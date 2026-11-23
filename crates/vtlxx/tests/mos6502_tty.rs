#![cfg(target_os = "linux")]

use std::{
    fs,
    io::{Read, Write},
    os::fd::{AsRawFd, FromRawFd, OwnedFd},
    process::{Child, Command, Stdio},
    time::{Duration, Instant},
};

struct Terminal {
    master: std::fs::File,
    slave: OwnedFd,
    initial: libc::termios,
    path: std::path::PathBuf,
}
impl Terminal {
    fn new() -> Self {
        let (mut master, mut slave) = (-1, -1);
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
        let master = unsafe { std::fs::File::from_raw_fd(master) };
        let slave = unsafe { OwnedFd::from_raw_fd(slave) };
        let initial = attributes(slave.as_raw_fd());
        let path = std::env::temp_dir().join(format!(
            "vtlxx-tty-{}-{:?}.vtl",
            std::process::id(),
            std::thread::current().id()
        ));
        Self {
            master,
            slave,
            initial,
            path,
        }
    }
    fn restored(&self) {
        let actual = attributes(self.slave.as_raw_fd());
        assert_eq!(self.initial.c_iflag, actual.c_iflag);
        assert_eq!(self.initial.c_oflag, actual.c_oflag);
        assert_eq!(self.initial.c_cflag, actual.c_cflag);
        assert_eq!(self.initial.c_lflag, actual.c_lflag);
        assert_eq!(self.initial.c_cc, actual.c_cc);
    }
    fn spawn(&self, source: &str) -> Child {
        self.spawn_mode(source, true)
    }
    fn spawn_mode(&self, source: &str, raw: bool) -> Child {
        self.spawn_with_engine(source, raw, true)
    }
    fn spawn_with_engine(&self, source: &str, raw: bool, explicit_engine: bool) -> Child {
        fs::write(&self.path, source).unwrap();
        let fd = unsafe { libc::dup(self.slave.as_raw_fd()) };
        assert!(fd >= 0);
        let mut command = Command::new(env!("CARGO_BIN_EXE_vtlxx6502"));
        command.arg(&self.path).arg("--interactive");
        if explicit_engine {
            command.args(["--engine", "mos6502"]);
        }
        if raw {
            command.arg("--raw-tty");
        }
        let child = command
            .stdin(unsafe { Stdio::from(OwnedFd::from_raw_fd(fd)) })
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()
            .unwrap();
        // The child still needs to read the source file.
        child
    }
}
#[test]
#[ignore = "requires ca65 and ld65 and a Linux PTY"]
fn default_engine_raw_terminal_restores_after_input() {
    let mut tty = Terminal::new();
    let mut child = tty.spawn_with_engine("?=\">\" A=$ ?=A", true, false);
    let output = read_prompt(&mut child);
    assert_eq!(
        attributes(tty.slave.as_raw_fd()).c_lflag & (libc::ICANON | libc::ECHO),
        0
    );
    tty.master.write_all(&[0, 65]).unwrap();
    for byte in b"65" {
        assert_eq!(output.recv_timeout(Duration::from_secs(20)).unwrap(), *byte);
    }
    assert!(wait(&mut child).success());
    tty.restored();
}
impl Drop for Terminal {
    fn drop(&mut self) {
        let _ = fs::remove_file(&self.path);
    }
}
fn attributes(fd: i32) -> libc::termios {
    let mut value = std::mem::MaybeUninit::uninit();
    assert_eq!(unsafe { libc::tcgetattr(fd, value.as_mut_ptr()) }, 0);
    unsafe { value.assume_init() }
}
fn read_prompt(child: &mut Child) -> std::sync::mpsc::Receiver<u8> {
    let stdout = child.stdout.take().unwrap();
    let (tx, rx) = std::sync::mpsc::channel();
    std::thread::spawn(move || {
        let mut stdout = stdout;
        let mut byte = [0];
        while stdout.read_exact(&mut byte).is_ok() {
            let _ = tx.send(byte[0]);
        }
    });
    assert_eq!(rx.recv_timeout(Duration::from_secs(20)).unwrap(), b'>');
    rx
}
fn wait(child: &mut Child) -> std::process::ExitStatus {
    let end = Instant::now() + Duration::from_secs(20);
    loop {
        if let Some(status) = child.try_wait().unwrap() {
            return status;
        }
        if Instant::now() > end {
            let _ = child.kill();
            panic!("child timed out")
        }
        std::thread::sleep(Duration::from_millis(10));
    }
}
#[test]
#[ignore = "requires ca65 and ld65 and a Linux PTY"]
fn raw_terminal_restores_after_byte_and_signals() {
    let mut tty = Terminal::new();
    let mut child = tty.spawn("?=\">\" A=$ ?=A");
    let output = read_prompt(&mut child);
    let raw = attributes(tty.slave.as_raw_fd());
    assert_eq!(raw.c_lflag & (libc::ICANON | libc::ECHO), 0);
    tty.master.write_all(&[0, 255]).unwrap();
    for byte in b"255" {
        assert_eq!(output.recv_timeout(Duration::from_secs(20)).unwrap(), *byte);
    }
    assert!(wait(&mut child).success());
    tty.restored();
    for signal in [libc::SIGINT, libc::SIGTERM, libc::SIGHUP, libc::SIGQUIT] {
        let mut child = tty.spawn("?=\">\" A=$");
        read_prompt(&mut child);
        assert_eq!(unsafe { libc::kill(child.id() as i32, signal) }, 0);
        assert_eq!(wait(&mut child).code(), Some(128 + signal));
        tty.restored();
    }
}
#[test]
#[ignore = "requires ca65 and ld65 and a Linux PTY"]
fn raw_terminal_restores_for_suspend_and_reapplies_on_continue() {
    let tty = Terminal::new();
    let mut child = tty.spawn("?=\">\" A=$");
    read_prompt(&mut child);
    assert_eq!(unsafe { libc::kill(child.id() as i32, libc::SIGTSTP) }, 0);
    let deadline = Instant::now() + Duration::from_secs(20);
    loop {
        let mut status = 0;
        let ret = unsafe {
            libc::waitpid(
                child.id() as i32,
                &mut status,
                libc::WNOHANG | libc::WUNTRACED,
            )
        };
        if ret > 0 && libc::WIFSTOPPED(status) {
            break;
        }
        if Instant::now() > deadline {
            let _ = child.kill();
            panic!("child did not suspend");
        }
        std::thread::sleep(Duration::from_millis(10));
    }
    tty.restored();
    assert_eq!(unsafe { libc::kill(child.id() as i32, libc::SIGCONT) }, 0);
    let deadline = Instant::now() + Duration::from_secs(20);
    while attributes(tty.slave.as_raw_fd()).c_lflag & libc::ICANON != 0 {
        assert!(
            Instant::now() < deadline,
            "raw mode was not restored after continue"
        );
        std::thread::sleep(Duration::from_millis(10));
    }
    assert_eq!(unsafe { libc::kill(child.id() as i32, libc::SIGTERM) }, 0);
    assert_eq!(wait(&mut child).code(), Some(128 + libc::SIGTERM));
    tty.restored();
}

#[test]
#[ignore = "requires ca65 and ld65 and a Linux PTY"]
fn signals_during_execution_and_failure_paths_restore_terminal() {
    let tty = Terminal::new();
    for signal in [libc::SIGINT, libc::SIGTERM, libc::SIGHUP, libc::SIGQUIT] {
        let mut child = tty.spawn("?=\">\" .LOOP:=1 ^=.LOOP #=.LOOP");
        read_prompt(&mut child);
        assert_eq!(unsafe { libc::kill(child.id() as i32, signal) }, 0);
        assert_eq!(wait(&mut child).code(), Some(128 + signal));
        tty.restored();
    }
    let mut child = tty.spawn("A=");
    assert_eq!(wait(&mut child).code(), Some(130));
    tty.restored();
    let mut child = tty.spawn("?=1/0");
    let status = wait(&mut child);
    assert!(!status.success(), "division by zero must fail");
    tty.restored();

    let mut child = tty.spawn("?=\">\" A=$ ?=A");
    let mut stdout = child.stdout.take().unwrap();
    let mut prompt = [0];
    stdout.read_exact(&mut prompt).unwrap();
    assert_eq!(prompt, *b">");
    drop(stdout);
    let mut master = &tty.master;
    master.write_all(b"A").unwrap();
    assert_eq!(wait(&mut child).code(), Some(2));
    tty.restored();
}

#[test]
#[ignore = "requires ca65 and ld65 and a Linux PTY"]
fn ordinary_interactive_keeps_canonical_mode() {
    let tty = Terminal::new();
    let mut child = tty.spawn_mode("?=\">\" A=$", false);
    read_prompt(&mut child);
    tty.restored();
    let mut master = &tty.master;
    master.write_all(b"A\n").unwrap();
    assert!(wait(&mut child).success());
    tty.restored();
}
