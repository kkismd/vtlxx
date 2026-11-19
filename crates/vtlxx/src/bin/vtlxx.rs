use std::{env, ffi::OsString, fmt, fs, io, process::ExitCode};
use vtlxx::{Machine, SourceError};

#[derive(Debug)]
enum CliError {
    Usage,
    Input(io::Error),
    Source(SourceError),
    SeedRange,
    #[cfg(not(unix))]
    RawTtyUnsupported,
}

impl fmt::Display for CliError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Usage => write!(f, "usage: vtlxx <source-file> [--seed N] [--raw-tty]"),
            Self::Input(error) => write!(f, "cannot read source file: {error}"),
            Self::Source(error) => write!(f, "VTLXX error: {error:?}"),
            Self::SeedRange => write!(f, "--seed must be a signed 16-bit decimal integer"),
            #[cfg(not(unix))]
            Self::RawTtyUnsupported => write!(f, "--raw-tty is only supported on Unix terminals"),
        }
    }
}

struct Arguments {
    path: OsString,
    seed: Option<i16>,
    raw_tty: bool,
}

fn arguments() -> Result<Arguments, CliError> {
    let mut args = env::args_os();
    args.next();
    let path = args.next().ok_or(CliError::Usage)?;
    let (mut seed, mut raw_tty) = (None, false);
    while let Some(arg) = args.next() {
        if arg == "--raw-tty" && !raw_tty {
            raw_tty = true;
        } else if arg == "--seed" && seed.is_none() {
            let value = args.next().ok_or(CliError::Usage)?;
            let value = value.to_str().ok_or(CliError::SeedRange)?;
            let digits = value
                .strip_prefix('-')
                .or_else(|| value.strip_prefix('+'))
                .unwrap_or(value);
            if digits.is_empty() || !digits.bytes().all(|b| b.is_ascii_digit()) {
                return Err(CliError::SeedRange);
            }
            seed = Some(value.parse::<i16>().map_err(|_| CliError::SeedRange)?);
        } else {
            return Err(CliError::Usage);
        }
    }
    Ok(Arguments {
        path,
        seed,
        raw_tty,
    })
}

#[cfg(unix)]
fn run() -> Result<(), CliError> {
    let args = arguments()?;
    let source = fs::read_to_string(args.path).map_err(CliError::Input)?;
    let input = unix_input::UnixInput::new(args.raw_tty).map_err(CliError::Input)?;
    let mut machine = Machine::new();
    if let Some(seed) = args.seed {
        machine.set_entropy_source(move || Ok(seed));
    }
    machine.set_runtime_input(input);
    machine.set_output(io::stdout());
    machine.execute_source(&source).map_err(CliError::Source)
}

#[cfg(not(unix))]
fn run() -> Result<(), CliError> {
    let args = arguments()?;
    if args.raw_tty {
        return Err(CliError::RawTtyUnsupported);
    }
    let source = fs::read_to_string(args.path).map_err(CliError::Input)?;
    let mut machine = Machine::new();
    if let Some(seed) = args.seed {
        machine.set_entropy_source(move || Ok(seed));
    }
    machine.set_input(io::stdin());
    machine.set_output(io::stdout());
    machine.execute_source(&source).map_err(CliError::Source)
}

#[cfg(unix)]
mod unix_input {
    use std::{
        io,
        mem::MaybeUninit,
        os::fd::RawFd,
        sync::atomic::{AtomicBool, AtomicI32, Ordering},
    };
    use vtlxx::{InputEvent, ReadMode, RuntimeInput};

    static INTERRUPTED: AtomicBool = AtomicBool::new(false);
    static SIGNAL_WRITE_FD: AtomicI32 = AtomicI32::new(-1);
    extern "C" fn on_sigint(_: libc::c_int) {
        INTERRUPTED.store(true, Ordering::Relaxed);
        let fd = SIGNAL_WRITE_FD.load(Ordering::Relaxed);
        if fd >= 0 {
            let byte = 1u8;
            unsafe {
                libc::write(fd, (&byte as *const u8).cast(), 1);
            }
        }
    }

    pub struct UnixInput {
        fd: RawFd,
        original_flags: libc::c_int,
        original_termios: Option<libc::termios>,
        old_sigint: Option<libc::sigaction>,
        wake_read: Option<RawFd>,
        wake_write: Option<RawFd>,
        tty: bool,
        raw: bool,
    }

    impl UnixInput {
        pub fn new(raw: bool) -> io::Result<Self> {
            let fd = libc::STDIN_FILENO;
            let tty = unsafe { libc::isatty(fd) == 1 };
            if raw && !tty {
                return Err(io::Error::new(
                    io::ErrorKind::InvalidInput,
                    "--raw-tty requires stdin to be a terminal",
                ));
            }
            let original_flags = cvt(unsafe { libc::fcntl(fd, libc::F_GETFL) })?;
            let mut input = Self {
                fd,
                original_flags,
                original_termios: None,
                old_sigint: None,
                wake_read: None,
                wake_write: None,
                tty,
                raw,
            };
            if raw {
                let mut pipe_fds = [-1; 2];
                cvt(unsafe { libc::pipe(pipe_fds.as_mut_ptr()) })?;
                input.wake_read = Some(pipe_fds[0]);
                input.wake_write = Some(pipe_fds[1]);
                for pipe_fd in pipe_fds {
                    let flags = cvt(unsafe { libc::fcntl(pipe_fd, libc::F_GETFL) })?;
                    cvt(unsafe { libc::fcntl(pipe_fd, libc::F_SETFL, flags | libc::O_NONBLOCK) })?;
                    let flags = cvt(unsafe { libc::fcntl(pipe_fd, libc::F_GETFD) })?;
                    cvt(unsafe { libc::fcntl(pipe_fd, libc::F_SETFD, flags | libc::FD_CLOEXEC) })?;
                }
                INTERRUPTED.store(false, Ordering::Relaxed);
                SIGNAL_WRITE_FD.store(pipe_fds[1], Ordering::Relaxed);
                let mut action = unsafe { MaybeUninit::<libc::sigaction>::zeroed().assume_init() };
                action.sa_sigaction = on_sigint as *const () as libc::sighandler_t;
                unsafe {
                    libc::sigemptyset(&mut action.sa_mask);
                }
                action.sa_flags = 0;
                let mut previous = MaybeUninit::<libc::sigaction>::uninit();
                if unsafe { libc::sigaction(libc::SIGINT, &action, previous.as_mut_ptr()) } == -1 {
                    SIGNAL_WRITE_FD.store(-1, Ordering::Relaxed);
                    return Err(io::Error::last_os_error());
                }
                input.old_sigint = Some(unsafe { previous.assume_init() });
                let mut term = MaybeUninit::<libc::termios>::uninit();
                cvt(unsafe { libc::tcgetattr(fd, term.as_mut_ptr()) })?;
                let original = unsafe { term.assume_init() };
                let mut changed = original;
                changed.c_lflag &= !(libc::ICANON | libc::ECHO);
                changed.c_cc[libc::VMIN] = 1;
                changed.c_cc[libc::VTIME] = 0;
                input.original_termios = Some(original);
                cvt(unsafe { libc::tcsetattr(fd, libc::TCSANOW, &changed) })?;
            }
            cvt(unsafe { libc::fcntl(fd, libc::F_SETFL, original_flags | libc::O_NONBLOCK) })?;
            input.interrupted()?;
            Ok(input)
        }

        fn interrupted(&self) -> io::Result<()> {
            if self.raw && INTERRUPTED.load(Ordering::Relaxed) {
                Err(io::Error::new(
                    io::ErrorKind::Interrupted,
                    "interrupted by SIGINT",
                ))
            } else {
                Ok(())
            }
        }

        fn poll(&self, wait: bool) -> io::Result<libc::c_short> {
            let mut descriptors = [
                libc::pollfd {
                    fd: self.fd,
                    events: libc::POLLIN,
                    revents: 0,
                },
                libc::pollfd {
                    fd: self.wake_read.unwrap_or(-1),
                    events: libc::POLLIN,
                    revents: 0,
                },
            ];
            loop {
                self.interrupted()?;
                let result = unsafe {
                    libc::poll(
                        descriptors.as_mut_ptr(),
                        descriptors.len() as _,
                        if wait { -1 } else { 0 },
                    )
                };
                if result >= 0 {
                    if descriptors[1].revents != 0 {
                        self.drain_wakeup()?;
                        self.interrupted()?;
                    }
                    return Ok(descriptors[0].revents);
                }
                let error = io::Error::last_os_error();
                if error.kind() != io::ErrorKind::Interrupted {
                    return Err(error);
                }
                self.interrupted()?;
            }
        }

        fn drain_wakeup(&self) -> io::Result<()> {
            let Some(fd) = self.wake_read else {
                return Ok(());
            };
            let mut bytes = [0u8; 64];
            loop {
                let count = unsafe { libc::read(fd, bytes.as_mut_ptr().cast(), bytes.len()) };
                if count >= 0 {
                    if count == 0 {
                        return Ok(());
                    }
                    continue;
                }
                let error = io::Error::last_os_error();
                if error.kind() == io::ErrorKind::Interrupted {
                    continue;
                }
                if error.kind() == io::ErrorKind::WouldBlock {
                    return Ok(());
                }
                return Err(error);
            }
        }

        fn read_byte(&self, revents: libc::c_short) -> io::Result<InputEvent> {
            let mut byte = 0u8;
            loop {
                self.interrupted()?;
                let count = unsafe { libc::read(self.fd, (&mut byte as *mut u8).cast(), 1) };
                if count == 1 {
                    return Ok(InputEvent::Byte(byte));
                }
                if count == 0 {
                    if self.tty && self.raw && revents & (libc::POLLHUP | libc::POLLERR) == 0 {
                        return Ok(InputEvent::NotReady);
                    }
                    return Ok(InputEvent::Eof);
                }
                let error = io::Error::last_os_error();
                if error.kind() == io::ErrorKind::Interrupted {
                    self.interrupted()?;
                    continue;
                }
                if error.kind() == io::ErrorKind::WouldBlock {
                    return Ok(InputEvent::NotReady);
                }
                return Err(error);
            }
        }
    }

    impl RuntimeInput for UnixInput {
        fn read_event(&mut self, mode: ReadMode) -> io::Result<InputEvent> {
            let wait = mode == ReadMode::Wait;
            let events = self.poll(wait)?;
            if events == 0 {
                return Ok(InputEvent::NotReady);
            }
            if events & libc::POLLNVAL != 0 {
                return Err(io::Error::other("stdin fd is invalid"));
            }
            self.read_byte(events)
        }
    }

    impl Drop for UnixInput {
        fn drop(&mut self) {
            unsafe {
                libc::fcntl(self.fd, libc::F_SETFL, self.original_flags);
                if let Some(termios) = self.original_termios.as_ref() {
                    libc::tcsetattr(self.fd, libc::TCSANOW, termios);
                }
                if let Some(action) = self.old_sigint.as_ref() {
                    libc::sigaction(libc::SIGINT, action, std::ptr::null_mut());
                }
                SIGNAL_WRITE_FD.store(-1, Ordering::Relaxed);
                if let Some(fd) = self.wake_read {
                    libc::close(fd);
                }
                if let Some(fd) = self.wake_write {
                    libc::close(fd);
                }
            }
        }
    }

    fn cvt(result: libc::c_int) -> io::Result<libc::c_int> {
        if result == -1 {
            Err(io::Error::last_os_error())
        } else {
            Ok(result)
        }
    }
}

fn main() -> ExitCode {
    match run() {
        Ok(()) => ExitCode::SUCCESS,
        Err(error) => {
            eprintln!("{error}");
            ExitCode::FAILURE
        }
    }
}
