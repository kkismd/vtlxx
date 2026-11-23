use std::{
    fs::{self, OpenOptions},
    io::{Read, Write},
    path::PathBuf,
    process::{Command, Output, Stdio},
    sync::atomic::{AtomicU64, Ordering},
};

#[cfg(unix)]
use std::process::{Child, ExitStatus};

static NEXT_FILE: AtomicU64 = AtomicU64::new(0);

struct SourceFile(PathBuf);

#[cfg(unix)]
struct ChildGuard(Child);

#[cfg(unix)]
impl std::ops::Deref for ChildGuard {
    type Target = Child;
    fn deref(&self) -> &Child {
        &self.0
    }
}

#[cfg(unix)]
impl std::ops::DerefMut for ChildGuard {
    fn deref_mut(&mut self) -> &mut Child {
        &mut self.0
    }
}

#[cfg(unix)]
impl Drop for ChildGuard {
    fn drop(&mut self) {
        let _ = self.0.kill();
        let _ = self.0.wait();
    }
}

#[cfg(unix)]
fn wait_timeout(child: &mut ChildGuard, timeout: std::time::Duration) -> ExitStatus {
    let deadline = std::time::Instant::now() + timeout;
    loop {
        if let Some(status) = child.try_wait().unwrap() {
            return status;
        }
        assert!(
            std::time::Instant::now() < deadline,
            "child process timed out"
        );
        std::thread::sleep(std::time::Duration::from_millis(10));
    }
}

#[cfg(unix)]
fn read_timeout<R: Read + std::os::fd::AsRawFd>(
    reader: &mut R,
    output: &mut [u8],
    timeout: std::time::Duration,
) {
    let deadline = std::time::Instant::now() + timeout;
    let mut offset = 0;
    while offset < output.len() {
        let remaining = deadline.saturating_duration_since(std::time::Instant::now());
        assert!(!remaining.is_zero(), "child stdout read timed out");
        let mut descriptor = libc::pollfd {
            fd: reader.as_raw_fd(),
            events: libc::POLLIN,
            revents: 0,
        };
        let ms = remaining.as_millis().min(i32::MAX as u128) as i32;
        let ready = unsafe { libc::poll(&mut descriptor, 1, ms.max(1)) };
        assert!(ready > 0, "child stdout read timed out");
        let count = reader.read(&mut output[offset..]).unwrap();
        assert_ne!(count, 0, "child stdout closed before expected output");
        offset += count;
    }
}

#[cfg(unix)]
fn read_to_end_timeout<R: Read + std::os::fd::AsRawFd>(
    reader: &mut R,
    timeout: std::time::Duration,
) -> Vec<u8> {
    let deadline = std::time::Instant::now() + timeout;
    let mut output = Vec::new();
    loop {
        let remaining = deadline.saturating_duration_since(std::time::Instant::now());
        assert!(!remaining.is_zero(), "child stdout read timed out");
        let mut descriptor = libc::pollfd {
            fd: reader.as_raw_fd(),
            events: libc::POLLIN,
            revents: 0,
        };
        let ms = remaining.as_millis().min(i32::MAX as u128) as i32;
        let ready = unsafe { libc::poll(&mut descriptor, 1, ms.max(1)) };
        assert!(ready > 0, "child stdout read timed out");
        let mut chunk = [0; 1024];
        let count = reader.read(&mut chunk).unwrap();
        if count == 0 {
            return output;
        }
        output.extend_from_slice(&chunk[..count]);
    }
}

impl SourceFile {
    fn new(contents: &[u8]) -> Self {
        let id = NEXT_FILE.fetch_add(1, Ordering::Relaxed);
        let path = std::env::temp_dir().join(format!("vtlxx-cli-{}-{id}.vtl", std::process::id()));
        let mut file = OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(&path)
            .unwrap();
        file.write_all(contents).unwrap();
        Self(path)
    }
}

#[test]
fn prompt_is_visible_before_input_and_output_is_not_duplicated() {
    let source = SourceFile::new(b"?=\"prompt\" A=$ $=A ?=() ");
    let mut child = Command::new(env!("CARGO_BIN_EXE_vtlxx"))
        .arg(&source.0)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .spawn()
        .unwrap();
    let mut stdout = child.stdout.take().unwrap();
    let mut prompt = [0; 6];
    stdout.read_exact(&mut prompt).unwrap();
    assert_eq!(&prompt, b"prompt");
    child.stdin.take().unwrap().write_all(b"Z").unwrap();
    let mut rest = Vec::new();
    stdout.read_to_end(&mut rest).unwrap();
    assert!(child.wait().unwrap().success());
    assert_eq!(rest, b"Z\n");
}

#[test]
fn empty_stdin_reports_eof() {
    let source = SourceFile::new(b"A=$ ?=A");
    let output = vtlxx(&[source.0.as_os_str()]);
    assert!(output.status.success());
    assert_eq!(output.stdout, b"-1");
}

#[cfg(unix)]
#[test]
fn regular_file_input_preserves_nul_ff_and_sticky_eof() {
    use std::fs::File;

    let source = SourceFile::new(b"A=$ B=$ C=$ ?=A ?=B ?=C");
    let input = SourceFile::new(&[0x00, 0xff]);
    let output = Command::new(env!("CARGO_BIN_EXE_vtlxx"))
        .arg(&source.0)
        .stdin(Stdio::from(File::open(&input.0).unwrap()))
        .output()
        .unwrap();
    assert!(output.status.success(), "{:?}", output.stderr);
    assert_eq!(output.stdout, b"255-1-1");

    let empty = SourceFile::new(b"");
    let output = Command::new(env!("CARGO_BIN_EXE_vtlxx"))
        .arg(&source.0)
        .stdin(Stdio::from(File::open(&empty.0).unwrap()))
        .output()
        .unwrap();
    assert!(output.status.success(), "{:?}", output.stderr);
    assert_eq!(output.stdout, b"-1-1-1");
}

#[cfg(unix)]
#[test]
fn nonblocking_read_and_waiting_read_share_pipe_bytes_and_eof() {
    let source = SourceFile::new(b"A=` ?=A B=$ ?=B C=$ ?=C D=` ?=D");
    let mut child = ChildGuard(
        Command::new(env!("CARGO_BIN_EXE_vtlxx"))
            .arg(&source.0)
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .spawn()
            .unwrap(),
    );
    let mut stdout = child.stdout.take().unwrap();
    let mut first = [0; 1];
    read_timeout(&mut stdout, &mut first, std::time::Duration::from_secs(3));
    assert_eq!(first, *b"0");
    let mut stdin = child.stdin.take().unwrap();
    stdin.write_all(b"Q").unwrap();
    let mut consumed = [0; 2];
    read_timeout(
        &mut stdout,
        &mut consumed,
        std::time::Duration::from_secs(3),
    );
    assert_eq!(&consumed, b"81");
    drop(stdin);
    let rest = read_to_end_timeout(&mut stdout, std::time::Duration::from_secs(3));
    assert!(wait_timeout(&mut child, std::time::Duration::from_secs(3)).success());
    assert_eq!(rest, b"-1-1");
}

#[cfg(unix)]
#[test]
fn raw_tty_requires_a_terminal_and_seed_can_be_combined_with_it() {
    let source = SourceFile::new(b"?=1");
    let output = Command::new(env!("CARGO_BIN_EXE_vtlxx"))
        .arg(&source.0)
        .arg("--raw-tty")
        .stdin(Stdio::null())
        .output()
        .unwrap();
    assert!(!output.status.success());
    assert!(output.stdout.is_empty());
    assert!(String::from_utf8_lossy(&output.stderr).contains("requires stdin to be a terminal"));

    let output = Command::new(env!("CARGO_BIN_EXE_vtlxx"))
        .arg(&source.0)
        .arg("--seed")
        .arg("2")
        .arg("--seed")
        .arg("3")
        .output()
        .unwrap();
    assert!(!output.status.success());
    assert!(output.stdout.is_empty());
}

#[cfg(unix)]
#[test]
fn raw_tty_reads_a_character_and_restores_terminal_settings() {
    use std::{
        fs::File,
        os::fd::FromRawFd,
        time::{Duration, Instant},
    };

    let source = SourceFile::new(b"A=$ ?=A");
    let (mut master, mut slave) = (0, 0);
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
    let mut original = std::mem::MaybeUninit::<libc::termios>::uninit();
    assert_eq!(unsafe { libc::tcgetattr(slave, original.as_mut_ptr()) }, 0);
    let original = unsafe { original.assume_init() };
    let child_stdin = unsafe { File::from_raw_fd(libc::dup(slave)) };
    let mut child = ChildGuard(
        Command::new(env!("CARGO_BIN_EXE_vtlxx"))
            .arg(&source.0)
            .arg("--raw-tty")
            .stdin(child_stdin)
            .stdout(Stdio::piped())
            .spawn()
            .unwrap(),
    );
    unsafe {
        libc::close(slave);
    }

    let deadline = Instant::now() + Duration::from_secs(3);
    let mut active = false;
    while Instant::now() < deadline {
        let mut current = std::mem::MaybeUninit::<libc::termios>::uninit();
        assert_eq!(unsafe { libc::tcgetattr(master, current.as_mut_ptr()) }, 0);
        let current = unsafe { current.assume_init() };
        if current.c_lflag & libc::ICANON == 0 && current.c_lflag & libc::ECHO == 0 {
            active = true;
            break;
        }
        std::thread::sleep(Duration::from_millis(10));
    }
    assert!(active, "child did not enter raw terminal mode");
    assert_eq!(unsafe { libc::write(master, b"R".as_ptr().cast(), 1) }, 1);
    let mut stdout = child.stdout.take().unwrap();
    let output = read_to_end_timeout(&mut stdout, Duration::from_secs(3));
    assert!(wait_timeout(&mut child, Duration::from_secs(3)).success());
    assert_eq!(output, b"82");

    let mut restored = std::mem::MaybeUninit::<libc::termios>::uninit();
    assert_eq!(unsafe { libc::tcgetattr(master, restored.as_mut_ptr()) }, 0);
    let restored = unsafe { restored.assume_init() };
    assert_eq!(restored.c_lflag, original.c_lflag);
    assert_eq!(restored.c_cc, original.c_cc);
    unsafe {
        libc::close(master);
    }
}

#[cfg(unix)]
#[test]
fn raw_tty_restores_terminal_settings_after_runtime_error() {
    use std::{
        fs::File,
        os::fd::FromRawFd,
        time::{Duration, Instant},
    };

    let source = SourceFile::new(b"A=$ ?=A B=1/0");
    let (mut master, mut slave) = (0, 0);
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
    let mut original = std::mem::MaybeUninit::<libc::termios>::uninit();
    assert_eq!(unsafe { libc::tcgetattr(slave, original.as_mut_ptr()) }, 0);
    let original = unsafe { original.assume_init() };
    let child_stdin = unsafe { File::from_raw_fd(libc::dup(slave)) };
    let mut child = ChildGuard(
        Command::new(env!("CARGO_BIN_EXE_vtlxx"))
            .arg(&source.0)
            .arg("--raw-tty")
            .stdin(child_stdin)
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()
            .unwrap(),
    );
    unsafe {
        libc::close(slave);
    }

    let deadline = Instant::now() + Duration::from_secs(3);
    let mut active = false;
    while Instant::now() < deadline {
        let mut current = std::mem::MaybeUninit::<libc::termios>::uninit();
        assert_eq!(unsafe { libc::tcgetattr(master, current.as_mut_ptr()) }, 0);
        if unsafe { current.assume_init() }.c_lflag & libc::ICANON == 0 {
            active = true;
            break;
        }
        std::thread::sleep(Duration::from_millis(10));
    }
    assert!(active, "child did not enter raw terminal mode");
    assert_eq!(unsafe { libc::write(master, b"R".as_ptr().cast(), 1) }, 1);
    let mut stdout = child.stdout.take().unwrap();
    let output = read_to_end_timeout(&mut stdout, Duration::from_secs(3));
    assert_eq!(output, b"82");
    assert!(!wait_timeout(&mut child, Duration::from_secs(3)).success());

    let mut restored = std::mem::MaybeUninit::<libc::termios>::uninit();
    assert_eq!(unsafe { libc::tcgetattr(master, restored.as_mut_ptr()) }, 0);
    let restored = unsafe { restored.assume_init() };
    assert_eq!(restored.c_lflag, original.c_lflag);
    assert_eq!(restored.c_cc, original.c_cc);
    unsafe {
        libc::close(master);
    }
}

#[cfg(unix)]
#[test]
fn raw_tty_sigint_interrupts_wait_and_restores_terminal_settings() {
    use std::{
        fs::File,
        os::fd::FromRawFd,
        time::{Duration, Instant},
    };

    let source = SourceFile::new(b"A=$");
    let (mut master, mut slave) = (0, 0);
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
    let mut original = std::mem::MaybeUninit::<libc::termios>::uninit();
    assert_eq!(unsafe { libc::tcgetattr(slave, original.as_mut_ptr()) }, 0);
    let original = unsafe { original.assume_init() };
    let child_stdin = unsafe { File::from_raw_fd(libc::dup(slave)) };
    let mut child = ChildGuard(
        Command::new(env!("CARGO_BIN_EXE_vtlxx"))
            .arg(&source.0)
            .arg("--raw-tty")
            .stdin(child_stdin)
            .stdout(Stdio::null())
            .spawn()
            .unwrap(),
    );
    unsafe {
        libc::close(slave);
    }

    let deadline = Instant::now() + Duration::from_secs(3);
    let mut active = false;
    while Instant::now() < deadline {
        let mut current = std::mem::MaybeUninit::<libc::termios>::uninit();
        assert_eq!(unsafe { libc::tcgetattr(master, current.as_mut_ptr()) }, 0);
        if unsafe { current.assume_init() }.c_lflag & libc::ICANON == 0 {
            active = true;
            break;
        }
        std::thread::sleep(Duration::from_millis(10));
    }
    assert!(active, "child did not enter raw terminal mode");
    assert_eq!(
        unsafe { libc::kill(child.id() as libc::pid_t, libc::SIGINT) },
        0
    );
    assert!(!wait_timeout(&mut child, Duration::from_secs(3)).success());

    let mut restored = std::mem::MaybeUninit::<libc::termios>::uninit();
    assert_eq!(unsafe { libc::tcgetattr(master, restored.as_mut_ptr()) }, 0);
    assert_eq!(unsafe { restored.assume_init() }.c_lflag, original.c_lflag);
    unsafe {
        libc::close(master);
    }
}

impl Drop for SourceFile {
    fn drop(&mut self) {
        fs::remove_file(&self.0).unwrap();
    }
}

fn vtlxx(args: &[&std::ffi::OsStr]) -> Output {
    vtlxx_args(args)
}

fn vtlxx_args(args: &[&std::ffi::OsStr]) -> Output {
    Command::new(env!("CARGO_BIN_EXE_vtlxx"))
        .args(args.iter().copied())
        .output()
        .unwrap()
}

#[test]
fn eight_queens_output_is_exact() {
    let path = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/eight_queens.vtl");
    let output = vtlxx(&[path.as_os_str()]);
    assert!(output.status.success(), "{:?}", output.stderr);
    assert_eq!(output.stdout, b"92");
    assert!(output.stderr.is_empty());
}

#[test]
fn machine_newline_is_the_only_newline() {
    let source = SourceFile::new(b"?=\"hello\" ?=()");
    let output = vtlxx(&[source.0.as_os_str()]);
    assert!(output.status.success(), "{:?}", output.stderr);
    assert_eq!(output.stdout, b"hello\n");
    assert!(output.stderr.is_empty());
}

#[test]
fn empty_machine_output_stays_empty() {
    let source = SourceFile::new(b"A=1");
    let output = vtlxx(&[source.0.as_os_str()]);
    assert!(output.status.success(), "{:?}", output.stderr);
    assert!(output.stdout.is_empty());
    assert!(output.stderr.is_empty());
}

#[test]
fn fixed_seed_initializes_a_reproducible_xorshift_sequence() {
    let source = SourceFile::new(b"A=! B=! C=! D=! E=! F=! ?=A ?=B ?=C ?=D ?=E ?=F");
    let expected = [
        (-32768, "2460826648200102368722344753"),
        (-1, "16319122392551518160439927244"),
        (0, "270153093029703269729846343"),
        (1, "16576123042990058932324727857"),
        (32767, "24479183911164967751802326781"),
    ];
    for (seed, sequence) in expected {
        let seed_arg = seed.to_string();
        let output = vtlxx_args(&[
            source.0.as_os_str(),
            std::ffi::OsStr::new("--seed"),
            std::ffi::OsStr::new(&seed_arg),
        ]);
        assert!(output.status.success(), "{seed}: {:?}", output.stderr);
        assert_eq!(output.stdout, sequence.as_bytes());
        assert!(output.stderr.is_empty());
    }
}

#[test]
fn entropy_read_works_without_a_fixed_seed_and_stays_in_range() {
    let source = SourceFile::new(b"A=! ?=A");
    let output = vtlxx(&[source.0.as_os_str()]);
    assert!(output.status.success(), "{:?}", output.stderr);
    let value = std::str::from_utf8(&output.stdout)
        .unwrap()
        .parse::<i32>()
        .unwrap();
    assert!((0..=32767).contains(&value));
    assert!(output.stderr.is_empty());
}

#[test]
fn invalid_seed_options_fail_before_running_source() {
    let source = SourceFile::new(b"?=\"started\"");
    let invalid: Vec<Vec<&std::ffi::OsStr>> = vec![
        vec![source.0.as_os_str(), std::ffi::OsStr::new("--seed")],
        vec![
            source.0.as_os_str(),
            std::ffi::OsStr::new("--seed"),
            std::ffi::OsStr::new("1"),
            std::ffi::OsStr::new("--seed"),
            std::ffi::OsStr::new("2"),
        ],
        vec![
            source.0.as_os_str(),
            std::ffi::OsStr::new("--seed"),
            std::ffi::OsStr::new("32768"),
        ],
        vec![source.0.as_os_str(), std::ffi::OsStr::new("--unknown")],
    ];
    for args in invalid {
        let output = vtlxx_args(&args);
        assert!(!output.status.success());
        assert!(output.stdout.is_empty());
    }
}

#[test]
fn missing_or_extra_arguments_are_usage_errors() {
    let source = SourceFile::new(b"?=1");
    for args in [vec![], vec![source.0.as_os_str(), source.0.as_os_str()]] {
        let output = vtlxx(&args);
        assert!(!output.status.success());
        assert!(output.stdout.is_empty());
        assert!(!output.stderr.is_empty());
    }
}

#[test]
fn missing_file_and_invalid_utf8_are_input_errors() {
    let source = SourceFile::new(&[0xff]);
    let missing = source.0.with_extension("missing");
    for path in [&missing, &source.0] {
        let output = vtlxx(&[path.as_os_str()]);
        assert!(!output.status.success());
        assert!(output.stdout.is_empty());
        assert!(!output.stderr.is_empty());
    }
}

#[test]
fn compile_and_runtime_failures_report_errors() {
    let compile = SourceFile::new(b"A=1+");
    let runtime = SourceFile::new(b"?=\"prior\" A=1/0");
    for (path, category) in [(&compile.0, "Compile"), (&runtime.0, "Runtime")] {
        let output = vtlxx(&[path.as_os_str()]);
        assert!(!output.status.success());
        assert_eq!(
            output.stdout,
            if category == "Runtime" {
                b"prior".as_slice()
            } else {
                b""
            }
        );
        assert!(String::from_utf8_lossy(&output.stderr).contains(category));
    }
}

#[test]
fn while_rhs_rechecks_condition_and_rejects_old_syntax() {
    let source = SourceFile::new(b"A=0 *=A<3 [ ?=A A=A+1 ] ?=A");
    let output = vtlxx(&[source.0.as_os_str()]);
    assert!(output.status.success(), "{:?}", output.stderr);
    assert_eq!(output.stdout, b"0123");
    assert!(output.stderr.is_empty());

    let source = SourceFile::new(b"*=() [ ~=0 ] [ A=1 ]");
    let output = vtlxx(&[source.0.as_os_str()]);
    assert!(!output.status.success());
    assert!(output.stdout.is_empty());
    assert!(String::from_utf8_lossy(&output.stderr).contains("Compile"));
}
