use std::fs;
use std::path::PathBuf;
use std::process::{Command, Output};
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::{SystemTime, UNIX_EPOCH};

static NEXT_ID: AtomicU64 = AtomicU64::new(0);

struct TestDir(PathBuf);

impl TestDir {
    fn new() -> Self {
        let nonce = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let path = std::env::temp_dir().join(format!(
            "vtlxx-cli-test-{}-{nonce}-{}",
            std::process::id(),
            NEXT_ID.fetch_add(1, Ordering::Relaxed)
        ));
        fs::create_dir(&path).unwrap();
        Self(path)
    }
}

impl Drop for TestDir {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}

fn run_cli(args: &[&str]) -> Output {
    Command::new(env!("CARGO_BIN_EXE_ecvtl6502"))
        .args(args)
        .output()
        .unwrap()
}

#[test]
fn help_and_argument_errors_do_not_need_cc65() {
    let help = run_cli(&["--help"]);
    assert!(help.status.success());
    assert!(String::from_utf8_lossy(&help.stdout).contains("--runtime-input"));

    let invalid = run_cli(&["--unknown"]);
    assert!(!invalid.status.success());
    assert!(invalid.stdout.is_empty());
    assert!(String::from_utf8_lossy(&invalid.stderr).contains("unknown option"));
}

#[test]
#[ignore = "requires ca65, ld65, and sim65; run with --ignored"]
fn compiles_native_source_and_reads_binary_runtime_input() {
    let temp = TestDir::new();
    let source = temp.0.join("program.vtl");
    let input = temp.0.join("input.bin");
    fs::write(&source, b"A=65 ?=A").unwrap();
    fs::write(&input, [0, 0xff, 65]).unwrap();

    let output = Command::new(env!("CARGO_BIN_EXE_ecvtl6502"))
        .arg(&source)
        .arg("--runtime-input")
        .arg(&input)
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert_eq!(output.stdout, b"65");
}

#[test]
#[ignore = "requires ca65, ld65, and sim65; run with --ignored"]
fn template_runs_without_test_sentinel_and_compile_failure_is_nonzero() {
    let temp = TestDir::new();
    let source = temp.0.join("program.vtl");
    fs::write(&source, b"&=a{} [ {}=42 ] a{A}=() a{B}=() ?=A ?=B").unwrap();
    let success = Command::new(env!("CARGO_BIN_EXE_ecvtl6502"))
        .arg(&source)
        .output()
        .unwrap();
    assert!(
        success.status.success(),
        "{}",
        String::from_utf8_lossy(&success.stderr)
    );
    assert_eq!(success.stdout, b"4242");

    fs::write(&source, b"A=").unwrap();
    let failure = Command::new(env!("CARGO_BIN_EXE_ecvtl6502"))
        .arg(&source)
        .output()
        .unwrap();
    assert!(!failure.status.success());
    assert!(failure.stdout.is_empty());
    assert!(String::from_utf8_lossy(&failure.stderr).contains("compilation failed"));
}

#[test]
fn oversized_source_is_rejected_before_toolchain_start() {
    let temp = TestDir::new();
    let source = temp.0.join("oversized.vtl");
    fs::write(&source, vec![b' '; u16::MAX as usize + 1]).unwrap();
    let output = Command::new(env!("CARGO_BIN_EXE_ecvtl6502"))
        .arg(&source)
        .output()
        .unwrap();
    assert!(!output.status.success());
    assert!(output.stdout.is_empty());
    assert!(String::from_utf8_lossy(&output.stderr).contains("maximum supported length"));
}
