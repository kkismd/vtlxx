use std::fs;
use std::path::PathBuf;
use std::process::{Command, Output};
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::{SystemTime, UNIX_EPOCH};

static NEXT_ID: AtomicU64 = AtomicU64::new(0);
type RuntimeInputCase = (&'static str, &'static [u8], &'static [u8], &'static [u8]);

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

fn run_program_with_input(temp: &TestDir, name: &str, source: &[u8], input: &[u8]) -> Output {
    let source_path = temp.0.join(format!("{name}.vtl"));
    let input_path = temp.0.join(format!("{name}.bin"));
    fs::write(&source_path, source).unwrap();
    fs::write(&input_path, input).unwrap();

    Command::new(env!("CARGO_BIN_EXE_ecvtl6502"))
        .arg(&source_path)
        .arg("--runtime-input")
        .arg(&input_path)
        .output()
        .unwrap()
}

fn assert_runtime_input_output(
    temp: &TestDir,
    name: &str,
    source: &[u8],
    input: &[u8],
    expected: &[u8],
) {
    let output = run_program_with_input(temp, name, source, input);
    assert!(
        output.status.success(),
        "{name}: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert_eq!(output.stdout, expected, "{name}");
}

#[test]
fn help_and_argument_errors_do_not_need_cc65() {
    let help = run_cli(&["--help"]);
    assert!(help.status.success());
    assert!(String::from_utf8_lossy(&help.stdout).contains("--runtime-input"));
    assert!(String::from_utf8_lossy(&help.stdout).contains("--cycle-limit"));

    let invalid = run_cli(&["--unknown"]);
    assert!(!invalid.status.success());
    assert!(invalid.stdout.is_empty());
    assert!(String::from_utf8_lossy(&invalid.stderr).contains("unknown option"));
}

#[test]
fn invalid_cycle_limits_are_rejected_before_toolchain_start() {
    for args in [
        vec!["source.vtl", "--cycle-limit", "0"],
        vec!["source.vtl", "--cycle-limit", "-1"],
        vec!["source.vtl", "--cycle-limit", ""],
        vec!["source.vtl", "--cycle-limit", "12x"],
        vec!["source.vtl", "--cycle-limit", "4294967296"],
        vec!["source.vtl", "--cycle-limit"],
        vec!["source.vtl", "--cycle-limit", "10", "--cycle-limit", "20"],
    ] {
        let output = run_cli(&args);
        assert_eq!(output.status.code(), Some(2), "args={args:?}");
        assert!(output.stdout.is_empty(), "args={args:?}");
        let stderr = String::from_utf8_lossy(&output.stderr);
        assert!(stderr.contains("--cycle-limit"), "args={args:?}: {stderr}");
    }
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
fn cycle_limit_option_is_passed_to_sim65() {
    let temp = TestDir::new();
    let source = temp.0.join("cycle-limit.vtl");
    fs::write(&source, b"A=1").unwrap();

    let output = Command::new(env!("CARGO_BIN_EXE_ecvtl6502"))
        .arg(&source)
        .args(["--cycle-limit", "1"])
        .output()
        .unwrap();
    assert!(!output.status.success());
    assert!(output.stdout.is_empty());
    assert!(String::from_utf8_lossy(&output.stderr).contains("cycle limit"));
}

#[test]
#[ignore = "requires ca65, ld65, and sim65; run with --ignored"]
fn runtime_input_primary_read_matches_waiting_input_semantics() {
    let temp = TestDir::new();
    let cases: &[RuntimeInputCase] = &[
        ("basic", b"A=$ ?=A", &[0x41], b"65"),
        ("nul_then_basic", b"A=$ ?=A", &[0x00, 0x41], b"65"),
        (
            "repeated_nul_then_basic",
            b"A=$ ?=A",
            &[0x00, 0x00, 0x41],
            b"65",
        ),
        ("high_bytes", b"A=$ B=$ ?=A ?=B", &[0x80, 0xff], b"128255"),
        ("eof", b"A=$ ?=A", &[], b"-1"),
        ("repeated_eof", b"A=$ B=$ ?=A ?=B", &[], b"-1-1"),
        ("nul_then_eof", b"A=$ ?=A", &[0x00], b"-1"),
    ];

    for (name, source, input, expected) in cases {
        assert_runtime_input_output(&temp, name, source, input, expected);
    }

    // NUL is physically transported but discarded; 0xff remains a valid byte,
    // and only transport EOF becomes -1.
    assert_runtime_input_output(
        &temp,
        "nul_high_bytes_and_eof",
        b"A=$ B=$ C=$ D=$ ?=A ?=B ?=C ?=D",
        &[0x00, 0x7f, 0x80, 0xff],
        b"127128255-1",
    );
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
#[ignore = "requires ca65, ld65, and sim65; run with --ignored"]
fn p0a_template_calibration_example_matches_reference_output_bytes() {
    let example = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("examples/p0a_template_calibration.vtl");
    let output = run_cli(&[example.to_str().unwrap()]);
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert_eq!(output.stdout, b"58\n20\n40\n");
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
