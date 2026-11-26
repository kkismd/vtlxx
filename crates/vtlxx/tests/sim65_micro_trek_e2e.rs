//! Compare the canonical seed-0 game through both production CLIs.
use std::fs;
use std::io::{Read, Write};
#[cfg(unix)]
use std::os::unix::process::CommandExt;
use std::process::{Child, Command, Output, Stdio};
#[cfg(target_os = "linux")]
use std::sync::mpsc;
use std::thread;
use std::time::{Duration, Instant};

const SOURCE: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/examples/micro_trek.vtl");
const TIMEOUT: Duration = Duration::from_secs(120);
const CYCLES: &str = "4000000000";

struct Guard(Child);
impl Drop for Guard {
    fn drop(&mut self) {
        #[cfg(unix)]
        unsafe {
            libc::kill(-(self.0.id() as i32), libc::SIGKILL);
        }
        let _ = self.0.kill();
        let _ = self.0.wait();
    }
}
fn wait(child: &mut Child, deadline: Instant) -> std::process::ExitStatus {
    loop {
        if let Some(status) = child.try_wait().unwrap() {
            return status;
        }
        assert!(Instant::now() < deadline, "CLI timed out after {TIMEOUT:?}");
        thread::sleep(Duration::from_millis(10));
    }
}
fn run(target: bool, seed: i16, input: &[u8]) -> Output {
    let path = std::env::temp_dir().join(format!(
        "micro-trek-{}-{:?}.input",
        std::process::id(),
        thread::current().id()
    ));
    let mut command = Command::new(if target {
        env!("CARGO_BIN_EXE_vtlxx6502")
    } else {
        env!("CARGO_BIN_EXE_vtlxx")
    });
    command
        .arg(SOURCE)
        .arg("--seed")
        .arg(seed.to_string())
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());
    if target {
        fs::write(&path, input).unwrap();
        command
            .arg("--runtime-input")
            .arg(&path)
            .args(["--cycle-limit", CYCLES]);
    }
    #[cfg(unix)]
    command.process_group(0);
    let mut child = Guard(command.spawn().unwrap());
    let deadline = Instant::now() + TIMEOUT;
    let mut stdin = child.0.stdin.take().unwrap();
    let bytes = if target { Vec::new() } else { input.to_vec() };
    let writer = thread::spawn(move || stdin.write_all(&bytes));
    let mut stdout = child.0.stdout.take().unwrap();
    let reader = thread::spawn(move || {
        let mut b = Vec::new();
        stdout.read_to_end(&mut b).unwrap();
        b
    });
    let mut stderr = child.0.stderr.take().unwrap();
    let errors = thread::spawn(move || {
        let mut b = Vec::new();
        stderr.read_to_end(&mut b).unwrap();
        b
    });
    let status = wait(&mut child.0, deadline);
    writer.join().unwrap().unwrap();
    let output = Output {
        status,
        stdout: reader.join().unwrap(),
        stderr: errors.join().unwrap(),
    };
    if target {
        fs::remove_file(path).unwrap();
    }
    output
}
fn compare(input: &[u8]) {
    compare_seed(0, input);
}
fn compare_seed(seed: i16, input: &[u8]) {
    let rust = run(false, seed, input);
    let target = run(true, seed, input);
    let repeated = run(true, seed, input);
    assert!(rust.status.success(), "Rust: {rust:?}");
    assert!(target.status.success(), "6502: {target:?}");
    assert_eq!(target.status, repeated.status, "6502 exit, input {input:?}");
    assert_eq!(
        target.stdout, repeated.stdout,
        "6502 stdout, input {input:?}"
    );
    assert_eq!(
        target.stderr, repeated.stderr,
        "6502 stderr, input {input:?}"
    );
    assert_eq!(rust.status.code(), target.status.code());
    assert_eq!(rust.stdout, target.stdout, "input {input:?}");
    assert_eq!(rust.stderr, target.stderr, "input {input:?}");
}
#[test]
#[ignore = "requires ca65, ld65, and Linux mos6502"]
fn displays_then_x_without_turn_end() {
    compare(b"G\nL\nS\nR\n?\nX\n");
}
#[test]
#[ignore = "requires ca65, ld65, and Linux mos6502"]
fn invalid_numeric_retry_cancel_and_eof() {
    compare(b"P\nbad\n0\nX\n");
    compare(b"P\nbad\n");
}
#[test]
#[ignore = "requires ca65, ld65, and Linux mos6502"]
fn actions_and_outside_warp_follow_fixed_tracks() {
    compare(b"P\n10\nT\n0\nW\n1\n0\nR\nX\n");
    compare(b"W\n89\n0\nX\n");
}
#[test]
#[ignore = "requires ca65, ld65, and Linux mos6502"]
fn two_defeats_invalid_retry_yes_then_no() {
    let mut input = b"W\n1\n0\n".repeat(30);
    input.extend_from_slice(b"bad\nY\n");
    input.extend_from_slice(&b"W\n1\n0\n".repeat(30));
    input.extend_from_slice(b"N\n");
    compare(&input);
}
#[test]
#[ignore = "requires ca65, ld65, and Linux mos6502"]
fn initial_eof_is_normal_exit() {
    compare_seed(0, b"");
}
#[test]
#[ignore = "requires ca65, ld65, and Linux mos6502"]
fn seed_boundaries_match_rust_and_repeat_on_6502() {
    for seed in [0, 1, -1, -32768, 32767] {
        compare_seed(seed, b"X\n");
    }
}
#[test]
#[ignore = "requires ca65, ld65, and Linux mos6502"]
fn initial_eof_matches_after_startup() {
    compare_seed(0, b"");
}
#[cfg(target_os = "linux")]
#[test]
#[ignore = "requires ca65, ld65, and Linux mos6502"]
fn interactive_prompt_is_visible_before_input() {
    let mut command = Command::new(env!("CARGO_BIN_EXE_vtlxx6502"));
    command.process_group(0);
    let mut child = Guard(
        command
            .arg(SOURCE)
            .arg("--interactive")
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()
            .unwrap(),
    );
    let deadline = Instant::now() + TIMEOUT;
    let mut stdout = child.0.stdout.take().unwrap();
    let (tx, rx) = mpsc::channel();
    let reader = thread::spawn(move || {
        let mut b = [0];
        while let Ok(1) = stdout.read(&mut b) {
            if tx.send(b[0]).is_err() {
                break;
            }
        }
    });
    let mut stderr = child.0.stderr.take().unwrap();
    let errors = thread::spawn(move || {
        let mut b = Vec::new();
        stderr.read_to_end(&mut b).unwrap();
        b
    });
    let mut output = Vec::new();
    while !output.ends_with(b"COMMAND :") {
        output.push(
            rx.recv_timeout(deadline.saturating_duration_since(Instant::now()))
                .expect("prompt timeout"),
        );
    }
    child.0.stdin.take().unwrap().write_all(b"X\n").unwrap();
    assert!(wait(&mut child.0, deadline).success());
    output.extend(rx.iter());
    reader.join().unwrap();
    assert!(errors.join().unwrap().is_empty());
    assert!(output.starts_with(b"IT IS STARDATE : 3200\nOBJECTIVE :\nDESTROY "));
    assert!(output.ends_with(b"COMMAND :"));
}
