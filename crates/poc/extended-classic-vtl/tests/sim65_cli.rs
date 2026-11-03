use std::fs;
use std::io::{Read, Write};
use std::path::PathBuf;
use std::process::{Child, Command, Output, Stdio};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::mpsc;
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

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
    assert!(String::from_utf8_lossy(&help.stdout).contains("--interactive"));

    let conflict = run_cli(&["source.vtl", "--interactive", "--runtime-input", "input"]);
    assert_eq!(conflict.status.code(), Some(2));
    assert!(conflict.stdout.is_empty());
    assert!(!conflict.stderr.is_empty());

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

#[test]
#[ignore = "requires ca65, ld65, and sim65; run with --ignored"]
fn while_rhs_runs_in_sim65_and_rejects_legacy_syntax() {
    let temp = TestDir::new();
    let success = run_program_with_input(&temp, "while-rhs", b"A=0 *=A<3 [ ?=A A=A+1 ] ?=A", &[]);
    assert!(success.status.success(), "{:?}", success.stderr);
    assert_eq!(success.stdout, b"0123");

    let failure = run_program_with_input(&temp, "while-legacy", b"*=() [ ~=0 ] [ ?=9 ]", &[]);
    assert!(!failure.status.success());
    assert!(failure.stdout.is_empty());
    assert!(String::from_utf8_lossy(&failure.stderr).contains("compilation failed"));
}

#[test]
#[ignore = "requires ca65, ld65, and sim65; run with --ignored"]
fn compile_stack_201_byte_fixture_matches_reference() {
    let temp = TestDir::new();
    let source = format!(
        "%=0 [ {}A={}0+0{}{} ?=A",
        "%=1 [ ".repeat(15),
        "0+(".repeat(16),
        ")".repeat(16),
        " ]".repeat(16)
    );
    assert_eq!(source.len(), 201);
    let path = temp.0.join("stack-201.vtl");
    fs::write(&path, &source).unwrap();
    let reference = Command::new(env!("CARGO_BIN_EXE_ecvtl"))
        .arg(&path)
        .output()
        .unwrap();
    let target = run_program_with_input(&temp, "stack-201", source.as_bytes(), b"");
    assert_eq!(reference.status.code(), Some(0), "{reference:?}");
    assert_eq!(target.status.code(), reference.status.code(), "{target:?}");
    assert_eq!(reference.stdout, b"0");
    assert_eq!(target.stdout, reference.stdout);
}

#[test]
#[ignore = "requires ca65, ld65, and sim65; run with --ignored"]
fn literal_output_matches_reference_and_preserves_runtime_state() {
    let temp = TestDir::new();
    let cases: &[(&str, &str, &[u8])] = &[
        ("hello", "?=\"HELLO\"", b"HELLO"),
        ("empty", "?=\"\"", b""),
        ("contents", "?=\"A B;[ ]\\C\"", b"A B;[ ]\\C"),
        ("nul-tab", "?=\"A\0\tB\"", b"A\0\tB"),
        ("adjacent", "?=\"A\" ?=42 ?=() $=66", b"A42\nB"),
        ("stack", "~=7 ?=\"OK\" A=~ ?=A", b"OK7"),
        (
            "control",
            "?=\"T\" &=p [ ?=\"P\" ?=1 ] p=() %=1 [ ?=\"I\" ] A=0 *=A<2 [ ?=\"W\" A=A+1 ] p=() ?=() $=66",
            b"TP1IWWP1\nB",
        ),
        (
            "template-owner",
            "&=a{} [ {}=1 ] &=p [ ?=\"P\" ] a{A}=() ?=\"T\" p=() a{A}=() ?=A",
            b"TP1",
        ),
    ];
    for &(name, source, expected) in cases {
        let path = temp.0.join(format!("{name}-reference.vtl"));
        fs::write(&path, source).unwrap();
        let reference = Command::new(env!("CARGO_BIN_EXE_ecvtl"))
            .arg(&path)
            .output()
            .unwrap();
        let target = run_program_with_input(&temp, name, source.as_bytes(), b"");
        assert!(reference.status.success(), "{name}: {reference:?}");
        assert!(target.status.success(), "{name}: {target:?}");
        assert_eq!(reference.stdout, expected, "{name}");
        assert_eq!(target.stdout, reference.stdout, "{name}");
    }

    let input = run_program_with_input(&temp, "input", b"?=\"PROMPT: \" A=$ ?=A", b"A");
    assert!(input.status.success(), "{input:?}");
    assert_eq!(input.stdout, b"PROMPT: 65");
}

#[test]
#[ignore = "requires ca65, ld65, and sim65; run with --ignored"]
fn literal_output_crosses_chunk_boundaries() {
    let temp = TestDir::new();
    for length in [255, 256, 511] {
        // Vary the bytes around chunk boundaries so omissions and repeats are visible.
        let literal: String = (0..length)
            .map(|i| (b'A' + (i % 26) as u8) as char)
            .collect();
        let source = format!("?=\"{literal}\"");
        let path = temp.0.join(format!("reference-{length}.vtl"));
        fs::write(&path, &source).unwrap();
        let reference = Command::new(env!("CARGO_BIN_EXE_ecvtl"))
            .arg(&path)
            .output()
            .unwrap();
        let target =
            run_program_with_input(&temp, &format!("chunk-{length}"), source.as_bytes(), b"");
        assert!(reference.status.success(), "{length}: {reference:?}");
        assert!(target.status.success(), "{length}: {target:?}");
        assert_eq!(reference.stdout, literal.as_bytes());
        assert_eq!(target.stdout, reference.stdout);
    }
}

#[test]
#[ignore = "requires ca65, ld65, and sim65; run with --ignored"]
fn invalid_or_oversized_literals_do_not_start_runtime() {
    let temp = TestDir::new();
    for (index, source) in [
        "?=\"unterminated",
        "?=\"line\nend\"",
        "?=\"line\rend\"",
        "?=\"A\"tail",
        "?=\"é\"",
        "?=\"A\"+1",
        "?=\"A\"?=2",
        "A=\"A\"",
    ]
    .iter()
    .enumerate()
    {
        let source = format!("?=42 {source}");
        let result =
            run_program_with_input(&temp, &format!("invalid-{index}"), source.as_bytes(), b"");
        assert!(!result.status.success(), "{source}: {result:?}");
        assert!(result.stdout.is_empty(), "{source}: {result:?}");
        assert!(
            String::from_utf8_lossy(&result.stderr).contains("syntax error"),
            "{source}: {result:?}"
        );
    }

    let source = format!(
        "&=a{{}} [ {{}}=1 ] &=p [ ?=\"{}\" ] ?=\"{}\" p=()",
        "A".repeat(1100),
        "B".repeat(1100)
    );
    let result = run_program_with_input(&temp, "arena-full", source.as_bytes(), b"");
    assert!(!result.status.success(), "{result:?}");
    assert!(result.stdout.is_empty(), "{result:?}");
    assert!(
        String::from_utf8_lossy(&result.stderr).contains("compilation failed"),
        "{result:?}"
    );
}

// A timeout must also clean up a stuck CLI, so failing streaming tests cannot
// leave a child running or wait forever for its output.
struct TimedChild(Child);

impl Drop for TimedChild {
    fn drop(&mut self) {
        let _ = self.0.kill();
        let _ = self.0.wait();
    }
}

fn wait_bounded(child: &mut Child) -> std::process::ExitStatus {
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
#[ignore = "requires ca65, ld65, and sim65; run with --ignored"]
fn interactive_prompts_are_visible_before_each_input() {
    let temp = TestDir::new();
    let source = temp.0.join("prompt.vtl");
    fs::write(&source, b"$=62 $=32 A=$ ?=A $=62 $=32 B=$ ?=B").unwrap();
    let mut child = TimedChild(
        Command::new(env!("CARGO_BIN_EXE_ecvtl6502"))
            .arg(&source)
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
    let receive = |expected: &[u8]| {
        for &byte in expected {
            assert_eq!(rx.recv_timeout(Duration::from_secs(20)).unwrap(), byte);
        }
    };
    receive(b"> ");
    let mut stdin = child.0.stdin.take().unwrap();
    stdin.write_all(b"A").unwrap();
    receive(b"65> ");
    stdin.write_all(b"B").unwrap();
    receive(b"66");
    assert!(wait_bounded(&mut child.0).success());
    assert!(rx.recv_timeout(Duration::from_secs(2)).is_err());
}

#[test]
#[ignore = "requires ca65, ld65, and sim65; run with --ignored"]
fn interactive_literal_prompt_precedes_input() {
    let temp = TestDir::new();
    let source = temp.0.join("interactive-input.vtl");
    fs::write(
        &source,
        b"?=\"ENTER ONE CHARACTER: \"\nA=$\n?=\"ASCII: \"\n?=A\n?=()\n",
    )
    .unwrap();
    let mut child = TimedChild(
        Command::new(env!("CARGO_BIN_EXE_ecvtl6502"))
            .arg(&source)
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
    let receive = |expected: &[u8]| {
        for &byte in expected {
            assert_eq!(rx.recv_timeout(Duration::from_secs(20)).unwrap(), byte);
        }
    };
    receive(b"ENTER ONE CHARACTER: ");
    assert!(rx.recv_timeout(Duration::from_millis(100)).is_err());
    child.0.stdin.take().unwrap().write_all(b"A\n").unwrap();
    receive(b"ASCII: 65\n");
    assert!(wait_bounded(&mut child.0).success());
    assert!(rx.recv_timeout(Duration::from_secs(2)).is_err());
}

#[test]
#[ignore = "requires ca65, ld65, and sim65; run with --ignored"]
fn interactive_input_bytes_eof_and_output_match_batch() {
    let temp = TestDir::new();
    let source_bytes = b"A=$ B=$ C=$ D=$ ?=A ?=B ?=C ?=D";
    let input = [0, 0x7f, 0x80, 0xff];
    let batch = run_program_with_input(&temp, "batch-comparison", source_bytes, &input);
    assert!(batch.status.success(), "{:?}", batch.stderr);
    let source = temp.0.join("interactive-comparison.vtl");
    fs::write(&source, source_bytes).unwrap();
    let mut child = TimedChild(
        Command::new(env!("CARGO_BIN_EXE_ecvtl6502"))
            .arg(&source)
            .arg("--interactive")
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()
            .unwrap(),
    );
    child.0.stdin.take().unwrap().write_all(&input).unwrap();
    assert!(wait_bounded(&mut child.0).success());
    let mut actual = Vec::new();
    child
        .0
        .stdout
        .take()
        .unwrap()
        .read_to_end(&mut actual)
        .unwrap();
    assert_eq!(actual, batch.stdout);
    assert_eq!(actual, b"127128255-1");
}

#[test]
#[ignore = "requires ca65, ld65, and sim65; run with --ignored"]
fn interactive_early_exit_compile_failure_and_cycle_limit() {
    let temp = TestDir::new();
    let source = temp.0.join("early.vtl");
    fs::write(&source, b"?=42").unwrap();
    let mut child = TimedChild(
        Command::new(env!("CARGO_BIN_EXE_ecvtl6502"))
            .arg(&source)
            .arg("--interactive")
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()
            .unwrap(),
    );
    // Keep stdin open throughout: the child must not wait for an input EOF.
    assert!(wait_bounded(&mut child.0).success());
    let mut output = Vec::new();
    child
        .0
        .stdout
        .take()
        .unwrap()
        .read_to_end(&mut output)
        .unwrap();
    assert_eq!(output, b"42");

    let limited = Command::new(env!("CARGO_BIN_EXE_ecvtl6502"))
        .arg(&source)
        .args(["--interactive", "--cycle-limit", "1"])
        .stdin(Stdio::null())
        .output()
        .unwrap();
    assert!(!limited.status.success());
    assert!(limited.stdout.is_empty());
    assert!(String::from_utf8_lossy(&limited.stderr).contains("cycle limit"));

    fs::write(&source, b"A=").unwrap();
    let invalid = Command::new(env!("CARGO_BIN_EXE_ecvtl6502"))
        .arg(&source)
        .arg("--interactive")
        .stdin(Stdio::null())
        .output()
        .unwrap();
    assert!(!invalid.status.success());
    assert!(invalid.stdout.is_empty());
    assert!(String::from_utf8_lossy(&invalid.stderr).contains("compilation failed"));
}

#[cfg(unix)]
#[test]
#[ignore = "requires ca65, ld65, and sim65; run with --ignored"]
fn interactive_cycle_limit_arguments_are_forwarded_only_when_requested() {
    use std::os::unix::fs::PermissionsExt;

    let temp = TestDir::new();
    let source = temp.0.join("arguments.vtl");
    let log = temp.0.join("sim65-args");
    let wrapper = temp.0.join("sim65");
    fs::write(&source, b"A=1").unwrap();
    let real = Command::new("which").arg("sim65").output().unwrap();
    assert!(real.status.success());
    let real = String::from_utf8(real.stdout).unwrap();
    fs::write(
        &wrapper,
        format!(
            "#!/bin/sh\nprintf '%s\\n' \"$@\" > \"$SIM65_ARGS_LOG\"\nexec '{}' \"$@\"\n",
            real.trim().replace('\'', "'\\''")
        ),
    )
    .unwrap();
    fs::set_permissions(&wrapper, fs::Permissions::from_mode(0o755)).unwrap();
    let path = std::env::join_paths(
        std::iter::once(temp.0.clone())
            .chain(std::env::split_paths(&std::env::var_os("PATH").unwrap())),
    )
    .unwrap();
    for (extra, expected) in [
        (vec!["--interactive"], None),
        (
            vec!["--interactive", "--cycle-limit", "4000000"],
            Some("4000000"),
        ),
        (vec![], Some("4000000")),
    ] {
        let output = Command::new(env!("CARGO_BIN_EXE_ecvtl6502"))
            .arg(&source)
            .args(extra)
            .env("PATH", &path)
            .env("SIM65_ARGS_LOG", &log)
            .stdin(Stdio::null())
            .output()
            .unwrap();
        assert!(output.status.success(), "{:?}", output.stderr);
        let args = fs::read_to_string(&log).unwrap();
        let args: Vec<_> = args.lines().collect();
        match expected {
            Some(limit) => assert!(args.windows(2).any(|w| w == ["-x", limit]), "{args:?}"),
            None => assert!(!args.contains(&"-x"), "{args:?}"),
        }
    }
}
