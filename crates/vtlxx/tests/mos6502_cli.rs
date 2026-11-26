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
        .args(["--cycle-limit", "200000000", "--runtime-input"])
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
    assert!(!text.contains("--engine") && text.contains("--raw-tty"));
    assert!(text.contains("6502") && text.contains("Linux"));
    for args in [
        vec!["missing.vtl", "--engine"],
        vec!["missing.vtl", "--engine", "bad"],
        vec!["missing.vtl", "--engine", "sim65"],
        vec!["missing.vtl", "--engine", "mos6502"],
        vec!["missing.vtl", "--raw-tty"],
        vec!["missing.vtl", "--raw-tty", "--raw-tty"],
        vec!["missing.vtl", "--interactive", "--raw-tty"],
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
    let result = cli().arg(source).output().unwrap();
    assert_eq!(result.status.code(), Some(2));
    assert!(result.stdout.is_empty());
    assert!(String::from_utf8_lossy(&result.stderr).contains("maximum supported length"));
}

#[test]
#[ignore = "requires Linux, ca65 and ld65; run with --ignored"]
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
        .args(["--cycle-limit", "1"])
        .output()
        .unwrap();
    assert_eq!(limited.status.code(), Some(126), "{limited:?}");
    assert!(limited.stdout.is_empty());
}

#[test]
#[ignore = "requires Linux, ca65 and ld65; run with --ignored"]
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
#[ignore = "requires Linux, ca65 and ld65; run with --ignored"]
fn native_runs_shared_programs() {
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
        {
            let engine = None::<&str>;
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
        }
    }
}

#[test]
#[ignore = "requires Linux, ca65 and ld65; run with --ignored"]
fn static_data_fixture_and_failures_match_reference() {
    #[derive(Clone, Copy, Debug)]
    enum Expected {
        Success(&'static [u8]),
        CompileError,
        BoundsError,
    }
    use Expected::{BoundsError, CompileError, Success};

    let temp = TestDir::new();
    let cases: &[(&[u8], Expected)] = &[
        (
            include_bytes!("fixtures/static_levels.vtl"),
            Success(include_bytes!("fixtures/static_levels.txt")),
        ),
        (b"A=.MISSING(0)", CompileError),
        (b".DATA:=[1] .DATA:=2", CompileError),
        (b".DATA:=1 A=.DATA(0)", CompileError),
        (b".DATA:=[1] #=.DATA", CompileError),
        (b".DATA:=[256]", CompileError),
        (b".DATA:=[1", CompileError),
        (".DATA:=[\"é\"]".as_bytes(), CompileError),
        (b".DATA:=[1,,2]", CompileError),
        (b".DATA:=[1] A=.DATA(-1)", BoundsError),
        (b".DATA:=[1] A=.DATA(1)", BoundsError),
        (b".DATA:=[] A=.DATA(0)", BoundsError),
    ];
    for &(source, expected) in cases {
        let path = temp.source(source);
        let reference = Command::new(env!("CARGO_BIN_EXE_vtlxx"))
            .arg(&path)
            .output()
            .unwrap();
        let reference_error = String::from_utf8_lossy(&reference.stderr);
        match expected {
            Success(stdout) => {
                assert!(reference.status.success(), "{source:?}: {reference:?}");
                assert_eq!(reference.stdout, stdout, "{source:?}");
            }
            CompileError | BoundsError => {
                assert!(!reference.status.success(), "{source:?}: {reference:?}");
                assert!(reference.stdout.is_empty(), "{source:?}: {reference:?}");
                let diagnostic = match expected {
                    CompileError => "Compile(",
                    BoundsError => "Runtime(StaticDataOutOfBounds)",
                    Success(_) => unreachable!(),
                };
                assert!(
                    reference_error.contains(diagnostic),
                    "{source:?}: {reference:?}"
                );
            }
        }
        {
            let engine = "native";
            let output = cli()
                .arg(&path)
                .args(["--cycle-limit", "200000000"])
                .output()
                .unwrap();
            let diagnostic = String::from_utf8_lossy(&output.stderr);
            match expected {
                Success(stdout) => {
                    assert!(output.status.success(), "{engine}: {source:?}: {output:?}");
                    assert_eq!(output.stdout, stdout, "{engine}: {source:?}");
                }
                CompileError => {
                    assert!(!output.status.success(), "{engine}: {source:?}: {output:?}");
                    assert!(output.stdout.is_empty(), "{engine}: {source:?}: {output:?}");
                    assert!(
                        diagnostic.contains("compilation failed"),
                        "{engine}: {source:?}: {output:?}"
                    );
                }
                BoundsError => {
                    assert_eq!(
                        output.status.code(),
                        Some(7),
                        "{engine}: {source:?}: {output:?}"
                    );
                    assert!(output.stdout.is_empty(), "{engine}: {source:?}: {output:?}");
                    assert!(
                        diagnostic.contains("status 7"),
                        "{engine}: {source:?}: {output:?}"
                    );
                }
            }
        }
    }
    // Whole-program compilation must suppress even an earlier output statement.
    {
        let engine = "native";
        let path = temp.source(b"?=42 .DATA:=[256]");
        let output = cli().arg(path).output().unwrap();
        assert!(!output.status.success(), "{engine}: {output:?}");
        assert!(output.stdout.is_empty(), "{engine}: {output:?}");
        assert!(
            String::from_utf8_lossy(&output.stderr).contains("compilation failed"),
            "{engine}: {output:?}"
        );
    }
}

#[test]
#[ignore = "requires Linux, ca65 and ld65; run with --ignored"]
fn constant_expressions_conform_to_reference() {
    // Expected values are calculated independently of the reference implementation.
    let successes: &[(&str, &[u8], &[u8])] = &[
        ("layout", b".STATE:=1 .NEXT:=.STATE+1 .WIDTH:=8 .HEIGHT:=8 .SIZE:=.WIDTH*.HEIGHT .BASE:=.SIZE+7 .ALIAS:=.BASE ?=.NEXT ?=() ?=.SIZE ?=() ?=.BASE ?=() ?=.ALIAS", b"2\n64\n71\n71"),
        ("long-names", b".ABCDEFGHIJKLMNOP:=2 .QRSTUVWXYZABCDEF:=3 .RESULT:=.ABCDEFGHIJKLMNOP+.QRSTUVWXYZABCDEF*4 ?=.RESULT", b"20"),
        ("wrap-and-sign", b".AA:=32767+1 .BB:=-32768-1 .CC:=30000*3 .DD:=-32768*-1 .EE:=1--2 .FF:=1+-2 ?=.AA ?=() ?=.BB ?=() ?=.CC ?=() ?=.DD ?=() ?=.EE ?=() ?=.FF", b"-32768\n32767\n24464\n-32768\n3\n-1"),
        ("forms", b"; comment\r\n\t.AA \t:=\t1+0; comment\r\n.AA2:=.AA+1 *=0 [ A=2 ] ?=.AA2", b"2"),
        ("procedure-label-data", b".ONE:=1+0 .TWO:=.ONE+1 .DATA:=[10,20,30] &=p [ ?=.DATA(.TWO) ] A=0 ^=.ONE A=A+1 %=A<.TWO [ #=.ONE ] p=() ?=A", b"302"),
    ];
    let failures: &[(&str, &[u8], Option<i32>)] = &[
        ("undefined", b".AA:=.MISSING", None),
        ("self", b".AA:=.AA", None),
        ("forward", b".AA:=.BB .BB:=1", None),
        ("duplicate", b".AA:=1 .AA:=2", None),
        ("mixed-duplicate", b".AA:=[1] .AA:=2", None),
        ("data-name", b".DATA:=[1] .AA:=.DATA", None),
        ("data-index", b".DATA:=[1] .AA:=.DATA(0)", None),
        ("literal-high", b".AA:=32768", Some(131)),
        ("literal-low", b".AA:=-32769", Some(131)),
        ("intermediate-literal", b".AA:=1+32768-1", Some(131)),
        ("adjacent-number", b".AA:=1.TWO", None),
        ("adjacent-name", b".BB:=1 .AA:=.BB.BB", None),
        ("incomplete", b".AA:=1+", None),
        ("double-plus", b".AA:=1++2", None),
        ("space-before", b".AA:=1 +2", None),
        ("space-after", b".AA:=1+ 2", None),
        ("tab", b".AA:=1+\t2", None),
        ("newline", b".AA:=1+\n2", None),
        ("crlf", b".AA:=1+\r\n2", None),
        ("minus-name", b".BB:=1 .AA:=-.BB", None),
        ("parentheses", b".AA:=(1+2)", None),
        ("division", b".AA:=1/2", None),
        ("remainder", b".AA:=1%2", None),
        ("comparison", b".AA:=1==2", None),
        ("character", b".AA:='A'", None),
        ("register", b".AA:=A", None),
        ("input", b".AA:=$", None),
        ("output", b".AA:=?", None),
        ("random", b".AA:=!", None),
        ("stack", b".AA:=~", None),
        ("comma", b".AA:=1,2", None),
        ("block", b"%=1 [ .AA:=1 ]", None),
        ("byte-expression", b".DATA:=[1+2]", None),
        ("byte-constant", b".AA:=1 .DATA:=[.AA]", None),
        ("label-range", b".AA:=0-1 ^=.AA", None),
    ];
    let temp = TestDir::new();
    for (name, source, expected) in successes {
        let path = temp.source(source);
        for is_reference in [true, false] {
            let mut command = if is_reference {
                Command::new(env!("CARGO_BIN_EXE_vtlxx"))
            } else {
                cli()
            };
            command.arg(&path).stdin(Stdio::null());
            if !is_reference {
                command.args(["--cycle-limit", "200000000"]);
            }
            let output = command.output().unwrap();
            assert_eq!(
                output.status.code(),
                Some(0),
                "{name} reference={is_reference}: {output:?}"
            );
            assert_eq!(
                &output.stdout, expected,
                "{name} reference={is_reference}: {output:?}"
            );
            assert!(
                output.stderr.is_empty(),
                "{name} reference={is_reference}: {output:?}"
            );
        }
    }
    for (name, source, status) in failures.iter().copied().chain(std::iter::once((
        "flush",
        &b"?=42 &=p [ A=1 ] .GOOD:=1 .BAD:=.GOOD+2+.MISSING"[..],
        None,
    ))) {
        let path = temp.source(source);
        for is_reference in [true, false] {
            let mut command = if is_reference {
                Command::new(env!("CARGO_BIN_EXE_vtlxx"))
            } else {
                cli()
            };
            command.arg(&path).stdin(Stdio::null());
            if !is_reference {
                command.args(["--cycle-limit", "200000000"]);
            }
            let output = command.output().unwrap();
            let diagnostic = String::from_utf8_lossy(&output.stderr);
            if is_reference {
                assert!(
                    !output.status.success() && diagnostic.contains("Compile("),
                    "{name} reference={is_reference}: {output:?}"
                );
                assert_eq!(
                    output.stdout,
                    if name == "flush" {
                        &b"42"[..]
                    } else {
                        &b""[..]
                    },
                    "{name} reference={is_reference}: {output:?}"
                );
            } else {
                assert!(
                    matches!(output.status.code(), Some(130 | 131)),
                    "{name} reference={is_reference}: {output:?}"
                );
                if let Some(code) = status {
                    assert_eq!(
                        output.status.code(),
                        Some(code),
                        "{name} reference={is_reference}: {output:?}"
                    );
                }
                assert!(
                    output.stdout.is_empty() && diagnostic.contains("compilation failed"),
                    "{name} reference={is_reference}: {output:?}"
                );
            }
        }
    }
}

#[test]
#[ignore = "requires Linux, ca65 and ld65; run with --ignored"]
fn batch_output_arrives_while_guest_is_running() {
    let temp = TestDir::new();
    let source = temp.source(b"?=\"prefix\" .LOOP:=1 ^=.LOOP #=.LOOP");
    {
        let engine = None::<&str>;
        let mut command = cli();
        command.arg(&source).args(["--cycle-limit", "1200000000"]);
        let mut child = TimedChild(
            command
                .stdin(Stdio::null())
                .stdout(Stdio::piped())
                .stderr(Stdio::piped())
                .spawn()
                .unwrap(),
        );
        let mut stdout = child.0.stdout.take().unwrap();
        let (tx, rx) = mpsc::channel();
        let reader = std::thread::spawn(move || {
            let mut prefix = [0; 6];
            let result = stdout.read_exact(&mut prefix);
            let _ = tx.send(result.map(|()| prefix));
        });
        let received = rx.recv_timeout(Duration::from_secs(20));
        let running = child.0.try_wait().unwrap().is_none();
        let _ = child.0.kill();
        let _ = child.0.wait();
        reader.join().unwrap();
        assert_eq!(received.unwrap().unwrap(), *b"prefix", "{engine:?}");
        assert!(
            running,
            "guest exited before prefix was observed: {engine:?}"
        );
    }
}

#[test]
#[ignore = "requires Linux, ca65 and ld65; run with --ignored"]
fn batch_output_and_failures_preserve_prefix_and_diagnostics() {
    let temp = TestDir::new();
    for (source, limit, status, stdout, diagnostic) in [
        (
            &b"?=\"first\" ?=\"second\""[..],
            "200000000",
            0,
            &b"firstsecond"[..],
            "",
        ),
        (b"?=\"prefix\" ?=1/0", "200000000", 3, b"prefix", "status 3"),
        (
            b"?=\"prefix\" .LOOP:=1 ^=.LOOP #=.LOOP",
            "2000000",
            126,
            b"prefix",
            "cycle limit exceeded",
        ),
        (
            b"?=\"prefix\" A=",
            "200000000",
            130,
            b"",
            "compilation failed",
        ),
    ] {
        let path = temp.source(source);
        let output = cli()
            .arg(path)
            .args(["--cycle-limit", limit])
            .output()
            .unwrap();
        assert_eq!(output.status.code(), Some(status), "{output:?}");
        assert_eq!(output.stdout, stdout, "{output:?}");
        let stderr = String::from_utf8_lossy(&output.stderr);
        if diagnostic.is_empty() {
            assert!(stderr.is_empty(), "{output:?}");
        } else {
            assert_eq!(stderr.matches(diagnostic).count(), 1, "{output:?}");
        }
    }
}

#[test]
#[ignore = "requires Linux, ca65 and ld65; run with --ignored"]
fn batch_output_failure_stops_guest_before_cycle_limit() {
    let temp = TestDir::new();
    let source = temp.source(b"?=\"prefix\" .LOOP:=1 ^=.LOOP #=.LOOP");
    let output = cli()
        .arg(source)
        .args(["--cycle-limit", "2000000"])
        .stdout(Stdio::from(
            fs::OpenOptions::new()
                .write(true)
                .open("/dev/full")
                .unwrap(),
        ))
        .output()
        .unwrap();
    assert_eq!(output.status.code(), Some(2), "{output:?}");
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(stderr.contains("mos6502 host: guest output:"), "{output:?}");
    assert!(!stderr.contains("cycle limit exceeded"), "{output:?}");
}

#[test]
#[ignore = "requires Linux, ca65 and ld65; run with --ignored"]
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
#[ignore = "requires Linux, ca65 and ld65; run with --ignored"]
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
#[ignore = "requires Linux, ca65 and ld65; run with --ignored"]
fn interactive_compilation_failure_does_not_wait_for_stdin() {
    let temp = TestDir::new();
    let source = temp.source(b"A=$+");
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

// Exercise the real example in the native CI path, with deadlines even if input
// handling regresses into an endless polling loop.
#[test]
#[ignore = "requires Linux, ca65 and ld65; run with --ignored"]
fn game_1024_batch_matches_reference() {
    let temp = TestDir::new();
    let input = temp.0.join("1024-input.bin");
    fs::write(&input, b"hJkLrQ").unwrap();
    let source = concat!(env!("CARGO_MANIFEST_DIR"), "/examples/1024.vtl");
    let mut reference = Command::new(env!("CARGO_BIN_EXE_vtlxx"));
    reference
        .arg(source)
        .stdin(Stdio::from(fs::File::open(&input).unwrap()));
    let mut native = cli();
    native
        .arg(source)
        .args(["--cycle-limit", "200000000", "--runtime-input"])
        .arg(&input);
    let reference = output_with_deadline(&mut reference);
    let native = output_with_deadline(&mut native);
    assert!(reference.status.success(), "{reference:?}");
    assert!(native.status.success(), "{native:?}");
    assert_eq!(reference.stdout, native.stdout);
}

#[test]
#[ignore = "requires Linux, ca65 and ld65; run with --ignored"]
fn cycle_defaults_and_explicit_limits_apply_to_finite_workload() {
    let temp = TestDir::new();
    let input = temp.0.join("1024-input.bin");
    fs::write(&input, b"hJkLrQ").unwrap();
    let source = concat!(env!("CARGO_MANIFEST_DIR"), "/examples/1024.vtl");
    let mut outputs = Vec::new();
    for (interactive, limit) in [
        (false, None),
        (true, Some("4000000")),
        (false, Some("200000000")),
        (true, None),
    ] {
        let mut command = cli();
        command.arg(source).args(["--seed", "4660"]);
        if interactive {
            command.arg("--interactive").stdin(Stdio::piped());
        } else {
            command.arg("--runtime-input").arg(&input);
        }
        if let Some(limit) = limit {
            command.args(["--cycle-limit", limit]);
        }
        outputs.push(output_with_input_and_deadline(
            &mut command,
            interactive.then_some(&b"hJkLrQ"[..]),
        ));
    }
    assert_eq!(outputs[0].status.code(), Some(126), "{:?}", outputs[0]);
    assert_eq!(outputs[1].status.code(), Some(126), "{:?}", outputs[1]);
    assert!(outputs[2].status.success(), "{:?}", outputs[2]);
    assert!(outputs[3].status.success(), "{:?}", outputs[3]);
    assert_eq!(outputs[2].stdout, outputs[3].stdout);
}

fn output_with_deadline(command: &mut Command) -> Output {
    output_with_input_and_deadline(command, None)
}

fn output_with_input_and_deadline(command: &mut Command, input: Option<&[u8]>) -> Output {
    let mut child = TimedChild(
        command
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()
            .unwrap(),
    );
    if let Some(input) = input {
        child.0.stdin.take().unwrap().write_all(input).unwrap();
    }
    let mut stdout = child.0.stdout.take().unwrap();
    let mut stderr = child.0.stderr.take().unwrap();
    let out = std::thread::spawn(move || {
        let mut bytes = Vec::new();
        stdout.read_to_end(&mut bytes).unwrap();
        bytes
    });
    let err = std::thread::spawn(move || {
        let mut bytes = Vec::new();
        stderr.read_to_end(&mut bytes).unwrap();
        bytes
    });
    let status = wait(&mut child.0);
    Output {
        status,
        stdout: out.join().unwrap(),
        stderr: err.join().unwrap(),
    }
}
