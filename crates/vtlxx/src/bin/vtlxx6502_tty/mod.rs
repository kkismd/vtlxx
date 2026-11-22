//! Linux-only terminal mode and signal handling for the private CLI.
use std::{
    io,
    os::fd::{AsRawFd, FromRawFd, OwnedFd, RawFd},
    sync::atomic::{AtomicI32, AtomicU32, Ordering},
};

static SIGNALS: AtomicU32 = AtomicU32::new(0);
static WAKE_FD: AtomicI32 = AtomicI32::new(-1);
const HANDLED: [i32; 5] = [
    libc::SIGINT,
    libc::SIGTERM,
    libc::SIGHUP,
    libc::SIGQUIT,
    libc::SIGTSTP,
];

extern "C" fn notify(signal: i32) {
    let errno = unsafe { *libc::__errno_location() };
    SIGNALS.fetch_or(1u32 << signal, Ordering::Relaxed);
    let fd = WAKE_FD.load(Ordering::Relaxed);
    if fd >= 0 {
        let byte = [0u8];
        // A full pipe is already readable. Only async-signal-safe operations run here.
        unsafe { libc::write(fd, byte.as_ptr().cast(), 1) };
    }
    unsafe { *libc::__errno_location() = errno };
}

pub(crate) struct RawTty {
    fd: RawFd,
    original: libc::termios,
    raw: libc::termios,
    read: OwnedFd,
    _write: OwnedFd,
    saved: Vec<(i32, libc::sigaction)>,
    active: bool,
}

impl RawTty {
    pub(crate) fn new(fd: RawFd) -> io::Result<Self> {
        let mut original = std::mem::MaybeUninit::<libc::termios>::uninit();
        if unsafe { libc::tcgetattr(fd, original.as_mut_ptr()) } != 0 {
            return Err(io::Error::last_os_error());
        }
        let original = unsafe { original.assume_init() };
        let mut raw = original;
        raw.c_iflag &= !(libc::IGNBRK
            | libc::BRKINT
            | libc::PARMRK
            | libc::ISTRIP
            | libc::INLCR
            | libc::IGNCR
            | libc::ICRNL
            | libc::IXON
            | libc::IXOFF
            | libc::IXANY
            | libc::INPCK);
        raw.c_lflag &= !(libc::ECHO | libc::ECHONL | libc::ICANON | libc::IEXTEN);
        raw.c_lflag |= libc::ISIG;
        raw.c_cflag &= !(libc::CSIZE | libc::PARENB);
        raw.c_cflag |= libc::CS8;
        raw.c_cc[libc::VMIN] = 1;
        raw.c_cc[libc::VTIME] = 0;
        let mut pipe = [-1; 2];
        if unsafe { libc::pipe2(pipe.as_mut_ptr(), libc::O_CLOEXEC | libc::O_NONBLOCK) } != 0 {
            return Err(io::Error::last_os_error());
        }
        let read = unsafe { OwnedFd::from_raw_fd(pipe[0]) };
        let write = unsafe { OwnedFd::from_raw_fd(pipe[1]) };
        if WAKE_FD
            .compare_exchange(-1, write.as_raw_fd(), Ordering::SeqCst, Ordering::SeqCst)
            .is_err()
        {
            return Err(io::Error::other(
                "raw terminal signal handlers already active",
            ));
        }
        SIGNALS.store(0, Ordering::SeqCst);
        let mut guard = Self {
            fd,
            original,
            raw,
            read,
            _write: write,
            saved: Vec::new(),
            active: false,
        };
        for signal in HANDLED {
            let mut action: libc::sigaction = unsafe { std::mem::zeroed() };
            action.sa_sigaction = notify as *const () as usize;
            unsafe { libc::sigemptyset(&mut action.sa_mask) };
            let mut old = std::mem::MaybeUninit::uninit();
            if unsafe { libc::sigaction(signal, &action, old.as_mut_ptr()) } != 0 {
                return Err(io::Error::last_os_error());
            }
            guard.saved.push((signal, unsafe { old.assume_init() }));
        }
        guard.set_raw()?;
        Ok(guard)
    }

    fn set_raw(&mut self) -> io::Result<()> {
        if unsafe { libc::tcsetattr(self.fd, libc::TCSANOW, &self.raw) } != 0 {
            return Err(io::Error::last_os_error());
        }
        self.active = true;
        Ok(())
    }

    pub(crate) fn restore(&mut self) -> io::Result<()> {
        if self.active {
            if unsafe { libc::tcsetattr(self.fd, libc::TCSANOW, &self.original) } != 0 {
                return Err(io::Error::last_os_error());
            }
            self.active = false;
        }
        Ok(())
    }

    pub(crate) fn pending(&self) -> bool {
        SIGNALS.load(Ordering::Relaxed) != 0
    }

    pub(crate) fn signal_fd(&self) -> RawFd {
        self.read.as_raw_fd()
    }

    pub(crate) fn process_signals(&mut self) -> io::Result<Option<i32>> {
        // Take flags first: a signal arriving while draining must remain
        // pending for the next instruction even if its byte is drained here.
        let signals = SIGNALS.swap(0, Ordering::SeqCst);
        let mut buf = [0u8; 64];
        loop {
            let n =
                unsafe { libc::read(self.read.as_raw_fd(), buf.as_mut_ptr().cast(), buf.len()) };
            if n > 0 {
                continue;
            }
            if n == 0 {
                break;
            }
            let e = io::Error::last_os_error();
            if e.kind() == io::ErrorKind::Interrupted {
                continue;
            }
            if e.kind() == io::ErrorKind::WouldBlock {
                break;
            }
            return Err(e);
        }
        for signal in [libc::SIGINT, libc::SIGTERM, libc::SIGHUP, libc::SIGQUIT] {
            if signals & (1u32 << signal) != 0 {
                return Ok(Some(signal));
            }
        }
        if signals & (1u32 << libc::SIGTSTP) != 0 {
            self.restore()?;
            // SIGSTOP cannot be ignored or recursively caught. The process resumes
            // here after SIGCONT, at which point terminal mode is re-established.
            if unsafe { libc::raise(libc::SIGSTOP) } != 0 {
                return Err(io::Error::last_os_error());
            }
            self.set_raw()?;
        }
        Ok(None)
    }
}

impl Drop for RawTty {
    fn drop(&mut self) {
        let _ = self.restore();
        for (signal, old) in self.saved.iter().rev() {
            unsafe { libc::sigaction(*signal, old, std::ptr::null_mut()) };
        }
        WAKE_FD.store(-1, Ordering::SeqCst);
        SIGNALS.store(0, Ordering::SeqCst);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn raw_mode_preserves_signals_and_restores_terminal() {
        let mut master = -1;
        let mut slave = -1;
        assert_eq!(
            unsafe {
                libc::openpty(
                    &mut master,
                    &mut slave,
                    std::ptr::null_mut(),
                    std::ptr::null(),
                    std::ptr::null(),
                )
            },
            0
        );
        let _master = unsafe { OwnedFd::from_raw_fd(master) };
        let slave = unsafe { OwnedFd::from_raw_fd(slave) };
        let mut before = std::mem::MaybeUninit::<libc::termios>::uninit();
        assert_eq!(
            unsafe { libc::tcgetattr(slave.as_raw_fd(), before.as_mut_ptr()) },
            0
        );
        let before = unsafe { before.assume_init() };
        {
            let mut guard = RawTty::new(slave.as_raw_fd()).unwrap();
            let mut current = std::mem::MaybeUninit::<libc::termios>::uninit();
            assert_eq!(
                unsafe { libc::tcgetattr(slave.as_raw_fd(), current.as_mut_ptr()) },
                0
            );
            let current = unsafe { current.assume_init() };
            assert_eq!(current.c_lflag & (libc::ICANON | libc::ECHO), 0);
            assert_ne!(current.c_lflag & libc::ISIG, 0);
            assert_eq!(
                current.c_iflag & (libc::ISTRIP | libc::ICRNL | libc::IXON),
                0
            );
            assert_eq!(current.c_cc[libc::VMIN], 1);
            assert_eq!(current.c_cc[libc::VTIME], 0);
            guard.restore().unwrap();
        }
        let mut after = std::mem::MaybeUninit::<libc::termios>::uninit();
        assert_eq!(
            unsafe { libc::tcgetattr(slave.as_raw_fd(), after.as_mut_ptr()) },
            0
        );
        let after = unsafe { after.assume_init() };
        assert_eq!(before.c_iflag, after.c_iflag);
        assert_eq!(before.c_oflag, after.c_oflag);
        assert_eq!(before.c_cflag, after.c_cflag);
        assert_eq!(before.c_lflag, after.c_lflag);
        assert_eq!(before.c_cc, after.c_cc);
    }
}
