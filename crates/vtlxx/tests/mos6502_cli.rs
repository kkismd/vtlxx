#![cfg(target_os = "linux")]

use std::fs;
use std::io::{Read, Write};
use std::path::PathBuf;
use std::process::{Child, Command, Output, Stdio};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::mpsc;
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

static NEXT_ID: AtomicU64 = AtomicU64::new(0);

struct TestDir(PathBuf);
impl TestDir {
    fn new() -> Self {
        let nonce = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let path = std::env::temp_dir().join(format!(
            "vtlxx-mos-cli-{}-{nonce}-{}",
            std::process::id(),
            NEXT_ID.fetch_add(1, Ordering::Relaxed)
        ));
        fs::create_dir(&path).unwrap();
        Self(path)
    }
    fn source(&self, source: &[u8]) -> PathBuf {
        let path = self.0.join("program.vtl");
        fs::write(&path, source).unwrap();
        path
    }
}
impl Drop for TestDir {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}

fn cli() -> Command {
    Command::new(env!("CARGO_BIN_EXE_vtlxx6502"))
}
fn run(temp: &TestDir, source: &[u8], input: &[u8], extra: &[&str]) -> Output {
    let source = temp.source(source);
    let input_path = temp.0.join("input.bin");
    fs::write(&input_path, input).unwrap();
    cli()
        .arg(source)
        .args([
            "--engine",
            "mos6502",
            "--cycle-limit",
            "200000000",
            "--runtime-input",
        ])
        .arg(input_path)
        .args(extra)
        .output()
        .unwrap()
}

#[test]
fn argument_errors_precede_source_read() {
    let help = cli().arg("--help").output().unwrap();
    assert!(help.status.success());
    let text = String::from_utf8_lossy(&help.stdout);
    assert!(text.contains("--engine") && text.contains("--raw-tty"));
    assert!(text.contains("mos6502") && text.contains("Linux"));
    for args in [
        vec!["missing.vtl", "--engine"],
        vec!["missing.vtl", "--engine", "bad"],
        vec!["missing.vtl", "--engine", "sim65", "--engine", "mos6502"],
        vec!["missing.vtl", "--raw-tty"],
        vec!["missing.vtl", "--raw-tty", "--raw-tty"],
        vec!["missing.vtl", "--interactive", "--raw-tty"],
        vec![
            "missing.vtl",
            "--engine",
            "sim65",
            "--interactive",
            "--raw-tty",
        ],
        vec!["missing.vtl", "--engine", "mos6502", "--raw-tty"],
        vec![
            "missing.vtl",
            "--engine",
            "mos6502",
            "--interactive",
            "--raw-tty",
        ],
    ] {
        let result = cli().args(&args).stdin(Stdio::null()).output().unwrap();
        let stderr = String::from_utf8_lossy(&result.stderr);
        assert_eq!(result.status.code(), Some(2), "{args:?}: {stderr}");
        assert!(result.stdout.is_empty(), "{args:?}");
        assert!(!stderr.contains("read source"), "{args:?}: {stderr}");
        assert!(
            stderr.contains("--engine") || stderr.contains("--raw-tty"),
            "{args:?}: {stderr}"
        );
    }
}

#[test]
fn oversized_source_is_rejected_before_build() {
    let temp = TestDir::new();
    let source = temp.source(&vec![b' '; 65536]);
    let result = cli()
        .arg(source)
        .args(["--engine", "mos6502"])
        .output()
        .unwrap();
    assert_eq!(result.status.code(), Some(2));
    assert!(result.stdout.is_empty());
    assert!(String::from_utf8_lossy(&result.stderr).contains("maximum supported length"));
}

#[test]
#[ignore = "requires ca65 and ld65; run with --ignored"]
fn batch_reads_binary_input_and_reports_guest_status() {
    let temp = TestDir::new();
    for (source, input, expected) in [
        (&b"?=65"[..], &b""[..], &b"65"[..]),
        (b"A=$ ?=A", b"A", b"65"),
        (b"?=` ?=` ?=$ ?=` ?=$", &[0, 255, 0, 65], b"025565-1-1"),
        (b"?=`", b"", b"-1"),
    ] {
        let result = run(&temp, source, input, &[]);
        assert!(result.status.success(), "{result:?}");
        assert_eq!(result.stdout, expected);
    }
    let syntax = run(&temp, b"A=", b"", &[]);
    assert_eq!(syntax.status.code(), Some(130), "{syntax:?}");
    assert!(syntax.stdout.is_empty());
    assert!(String::from_utf8_lossy(&syntax.stderr).contains("compilation failed"));
    let source = temp.source(b"?=65");
    let limited = cli()
        .arg(source)
        .args(["--engine", "mos6502", "--cycle-limit", "1"])
        .output()
        .unwrap();
    assert_eq!(limited.status.code(), Some(126), "{limited:?}");
    assert!(limited.stdout.is_empty());
}

#[test]
#[ignore = "requires ca65 and ld65; run with --ignored"]
fn fixed_seed_vector_matches_reference() {
    let temp = TestDir::new();
    let result = run(
        &temp,
        b"A=! B=! C=! D=! E=! F=! ?=A ?=B ?=C ?=D ?=E ?=F",
        b"",
        &["--seed", "1"],
    );
    assert!(result.status.success(), "{result:?}");
    assert_eq!(result.stdout, b"16576123042990058932324727857");
}

#[test]
#[ignore = "requires ca65, ld65, and sim65; run with --ignored"]
fn product_engines_match_on_shared_programs() {
    let temp = TestDir::new();
    let input = temp.0.join("shared-input.bin");
    fs::write(&input, [0, 0, 65]).unwrap();
    for (source, expected_status, expected) in [
        ("?=65", 0, &b"65"[..]),
        ("A=$ ?=A", 0, &b"65"[..]),
        (".NUM:=4 &=p [ ?=.NUM ?=\"!\" ] p=()", 0, &b"4!"[..]),
        ("A=! B=! ?=A ?=B", 0, &b"1657612304"[..]),
        ("?=42 A=", 130, &b""[..]),
    ] {
        let path = temp.source(source.as_bytes());
        let mut outputs = Vec::new();
        for engine in [None, Some("sim65"), Some("mos6502")] {
            let mut command = cli();
            command
                .arg(&path)
                .args([
                    "--seed",
                    "1",
                    "--cycle-limit",
                    "200000000",
                    "--runtime-input",
                ])
                .arg(&input);
            if let Some(engine) = engine {
                command.args(["--engine", engine]);
            }
            let output = command.output().unwrap();
            assert_eq!(
                output.status.code(),
                Some(expected_status),
                "{source}: {engine:?}: {output:?}"
            );
            assert_eq!(output.stdout, expected, "{source}: {engine:?}");
            if expected_status == 0 {
                assert!(output.stderr.is_empty(), "{source}: {engine:?}: {output:?}");
            } else {
                assert!(String::from_utf8_lossy(&output.stderr).contains("compilation failed"));
            }
            outputs.push(output);
        }
        assert_eq!(outputs[0].stdout, outputs[2].stdout);
        assert_eq!(outputs[0].stderr, outputs[2].stderr);
    }
}

#[test]
#[ignore = "requires ca65 and ld65; run with --ignored"]
fn default_and_explicit_mos_match_on_cycle_limit() {
    let temp = TestDir::new();
    let source = temp.source(b".LOOP:=1 ^=.LOOP #=.LOOP");
    let mut outputs = Vec::new();
    for engine in [None, Some("mos6502")] {
        let mut command = cli();
        command.arg(&source).args(["--cycle-limit", "100000"]);
        if let Some(engine) = engine {
            command.args(["--engine", engine]);
        }
        outputs.push(command.output().unwrap());
    }
    assert_eq!(outputs[0].status.code(), Some(126));
    assert_eq!(outputs[0].status.code(), outputs[1].status.code());
    assert_eq!(outputs[0].stdout, outputs[1].stdout);
    assert_eq!(outputs[0].stderr, outputs[1].stderr);
}

#[test]
#[ignore = "requires ca65 and ld65; run with --ignored"]
fn busy_try_obeys_cycle_limit() {
    let temp = TestDir::new();
    let source = temp.source(b".LOOP:=1 ^=.LOOP A=` #=.LOOP");
    let mut child = TimedChild(
        cli()
            .arg(source)
            .args(["--interactive", "--cycle-limit", "2000000"])
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()
            .unwrap(),
    );
    assert_eq!(wait(&mut child.0).code(), Some(126));
}

struct TimedChild(Child);
impl Drop for TimedChild {
    fn drop(&mut self) {
        let _ = self.0.kill();
        let _ = self.0.wait();
    }
}
fn wait(child: &mut Child) -> std::process::ExitStatus {
    let deadline = Instant::now() + Duration::from_secs(20);
    loop {
        if let Some(status) = child.try_wait().unwrap() {
            return status;
        }
        assert!(Instant::now() < deadline, "interactive CLI timed out");
        std::thread::sleep(Duration::from_millis(10));
    }
}

#[test]
#[ignore = "requires ca65 and ld65; run with --ignored"]
fn interactive_try_then_wait_streams_before_input() {
    let temp = TestDir::new();
    let source = temp.source(b"?=` ?=\"> \" A=$ ?=A B=$ ?=B");
    let mut child = TimedChild(
        cli()
            .arg(source)
            .arg("--interactive")
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()
            .unwrap(),
    );
    let mut stdout = child.0.stdout.take().unwrap();
    let (tx, rx) = mpsc::channel();
    std::thread::spawn(move || {
        let mut byte = [0];
        while let Ok(1) = stdout.read(&mut byte) {
            if tx.send(byte[0]).is_err() {
                break;
            }
        }
    });
    for byte in b"0> " {
        assert_eq!(rx.recv_timeout(Duration::from_secs(20)).unwrap(), *byte);
    }
    assert!(rx.recv_timeout(Duration::from_millis(100)).is_err());
    let mut stdin = child.0.stdin.take().unwrap();
    stdin.write_all(&[0, 255]).unwrap();
    drop(stdin);
    for byte in b"255-1" {
        assert_eq!(rx.recv_timeout(Duration::from_secs(20)).unwrap(), *byte);
    }
    assert!(wait(&mut child.0).success());
    assert!(rx.recv_timeout(Duration::from_secs(2)).is_err());
}

#[test]
#[ignore = "requires ca65 and ld65; run with --ignored"]
fn interactive_compilation_failure_does_not_wait_for_stdin() {
    let temp = TestDir::new();
    let source = temp.source(b"A=$+");
    let mut child = TimedChild(
        cli()
            .arg(source)
            .args(["--engine", "mos6502", "--interactive"])
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()
            .unwrap(),
    );
    assert_eq!(wait(&mut child.0).code(), Some(130));
    let mut output = Vec::new();
    child
        .0
        .stdout
        .take()
        .unwrap()
        .read_to_end(&mut output)
        .unwrap();
    assert!(output.is_empty());
}
