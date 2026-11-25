use std::ffi::OsStr;
use std::fs;
use std::io::{Read, Write};
use std::path::Path;
use std::process::{Command, Output, Stdio};
use std::time::{Duration, Instant};

const EXAMPLES_DIR: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/examples");

#[allow(dead_code)]
#[derive(Clone, Copy, Debug)]
enum Disposition {
    Portable {
        expected_stdout: Option<&'static [u8]>,
        cycle_limit: Option<u32>,
        runtime_input: Option<&'static [u8]>,
    },
    Deferred {
        reason: &'static str,
        followup_issue: &'static str,
    },
}

#[derive(Clone, Copy, Debug)]
struct ExampleCase {
    name: &'static str,
    engine: &'static str,
    disposition: Disposition,
}

const EXAMPLES: &[ExampleCase] = &[
    ExampleCase {
        name: "1024.vtl",
        engine: "mos6502",
        disposition: Disposition::Portable {
            expected_stdout: None,
            cycle_limit: Some(256_000_000),
            runtime_input: Some(include_bytes!("fixtures/1024_quit.txt")),
        },
    },
    ExampleCase {
        name: "eight_queens.vtl",
        engine: "sim65",
        disposition: Disposition::Portable {
            expected_stdout: Some(include_bytes!("fixtures/eight_queens_example_output.txt")),
            cycle_limit: Some(256_000_000),
            runtime_input: None,
        },
    },
    ExampleCase {
        name: "eight_queens_storage.vtl",
        engine: "sim65",
        disposition: Disposition::Portable {
            expected_stdout: Some(include_bytes!("fixtures/eight_queens_example_output.txt")),
            cycle_limit: Some(256_000_000),
            runtime_input: None,
        },
    },
    ExampleCase {
        name: "interactive_input.vtl",
        engine: "sim65",
        disposition: Disposition::Portable {
            expected_stdout: Some(b"ENTER ONE CHARACTER: ASCII: 65\n"),
            cycle_limit: None,
            runtime_input: Some(include_bytes!("fixtures/interactive_input.txt")),
        },
    },
    ExampleCase {
        name: "mandelbrot.vtl",
        engine: "sim65",
        disposition: Disposition::Portable {
            expected_stdout: Some(include_bytes!("fixtures/mandelbrot_example_output.txt")),
            cycle_limit: Some(1_200_000_000),
            runtime_input: None,
        },
    },
    ExampleCase {
        name: "maze_dfs.vtl",
        engine: "sim65",
        disposition: Disposition::Portable {
            expected_stdout: Some(b"7\n"),
            cycle_limit: None,
            runtime_input: None,
        },
    },
    ExampleCase {
        name: "micro_trek.vtl",
        engine: "sim65",
        disposition: Disposition::Portable {
            expected_stdout: None,
            cycle_limit: Some(4_000_000_000),
            runtime_input: Some(include_bytes!("fixtures/micro_trek_exit.txt")),
        },
    },
    ExampleCase {
        name: "rpn_evaluator.vtl",
        engine: "sim65",
        disposition: Disposition::Portable {
            expected_stdout: Some(b"7\n"),
            cycle_limit: None,
            runtime_input: None,
        },
    },
    ExampleCase {
        name: "sim65_cli.vtl",
        engine: "sim65",
        disposition: Disposition::Portable {
            expected_stdout: Some(b"42"),
            cycle_limit: None,
            runtime_input: None,
        },
    },
];

#[test]
#[ignore = "requires ca65, ld65, sim65, and Linux mos6502; run with --ignored"]
fn registered_examples_match_the_rust_reference_when_portable() {
    let examples_dir = Path::new(EXAMPLES_DIR);
    let mut actual_names = fs::read_dir(examples_dir)
        .unwrap_or_else(|error| panic!("read {}: {error}", examples_dir.display()))
        .filter_map(|entry| {
            let entry = entry.expect("read examples directory entry");
            let path = entry.path();
            if path.is_file() && path.extension() == Some(OsStr::new("vtl")) {
                Some(path.file_name().unwrap().to_string_lossy().into_owned())
            } else {
                None
            }
        })
        .collect::<Vec<_>>();
    actual_names.sort();

    let mut manifest_names = EXAMPLES.iter().map(|case| case.name).collect::<Vec<_>>();
    manifest_names.sort_unstable();
    let actual_as_str = actual_names.iter().map(String::as_str).collect::<Vec<_>>();
    assert_eq!(
        actual_as_str, manifest_names,
        "examples/*.vtl and the conformance manifest must match in both directions"
    );

    assert!(
        manifest_names.windows(2).all(|pair| pair[0] != pair[1]),
        "duplicate manifest name"
    );

    let mut failures = Vec::new();
    for case in EXAMPLES {
        if let Disposition::Deferred {
            reason,
            followup_issue,
        } = case.disposition
        {
            assert!(!reason.trim().is_empty() && !followup_issue.trim().is_empty());
            println!(
                "{}: DEFERRED (not run): {reason}; {followup_issue}",
                case.name
            );
            continue;
        }
        if case.name == "1024.vtl" && !cfg!(target_os = "linux") {
            println!(
                "1024.vtl: not run: mos6502 host requires Linux{}",
                if cfg!(unix) {
                    ""
                } else {
                    "; Rust CLI Try input also requires Unix"
                }
            );
            continue;
        }
        if let Err(error) = check_example(case, examples_dir) {
            eprintln!("{error}");
            failures.push(error);
        }
    }
    if cfg!(target_os = "linux") {
        let moves = ExampleCase {
            name: "1024.vtl",
            engine: "mos6502",
            disposition: Disposition::Portable {
                expected_stdout: None,
                cycle_limit: Some(256_000_000),
                runtime_input: Some(include_bytes!("fixtures/1024_moves.txt")),
            },
        };
        if let Err(error) = check_example(&moves, examples_dir) {
            failures.push(error);
        }
    }
    assert!(
        failures.is_empty(),
        "{} example(s) failed conformance checks:\n{}",
        failures.len(),
        failures.join("\n")
    );
}

fn check_example(case: &ExampleCase, examples_dir: &Path) -> Result<(), String> {
    let source = examples_dir.join(case.name);
    let Disposition::Portable {
        expected_stdout,
        cycle_limit,
        runtime_input,
    } = case.disposition
    else {
        unreachable!("deferred examples are not executed");
    };
    let rust = run(
        env!("CARGO_BIN_EXE_vtlxx"),
        &source,
        None,
        runtime_input,
        None,
    )
    .map_err(|error| format!("{}: {error}", case.name))?;
    if rust.status.code().is_none() {
        return Err(format!("{}: Rust vtlxx terminated by signal", case.name));
    }
    if !rust.status.success() {
        return Err(format!(
            "{}: Rust reference failed (exit {:?}): {}",
            case.name,
            rust.status.code(),
            String::from_utf8_lossy(&rust.stderr)
        ));
    }

    let target = run(
        env!("CARGO_BIN_EXE_vtlxx6502"),
        &source,
        cycle_limit,
        runtime_input,
        Some(case.engine),
    )
    .map_err(|error| format!("{}: {error}", case.name))?;
    if target.status.code().is_none() {
        return Err(format!(
            "{}: 6502 vtlxx6502 terminated by signal",
            case.name
        ));
    }

    if !target.status.success() {
        return Err(format!(
            "{}: expected portable success, got exit {:?}: {}",
            case.name,
            target.status.code(),
            String::from_utf8_lossy(&target.stderr)
        ));
    }
    if target.stdout != rust.stdout {
        return Err(format!(
            "{}: Rust/6502 stdout bytes differ (Rust {}, 6502 {})",
            case.name,
            describe_bytes(&rust.stdout),
            describe_bytes(&target.stdout)
        ));
    }
    if let Some(expected) = expected_stdout
        && rust.stdout != expected
    {
        return Err(format!(
            "{}: Rust stdout differs from fixed bytes (actual {}, expected {})",
            case.name,
            describe_bytes(&rust.stdout),
            describe_bytes(expected)
        ));
    }
    if matches!(case.name, "eight_queens.vtl" | "eight_queens_storage.vtl") {
        assert_eight_queens_output(case.name, &rust.stdout)?;
    }
    if case.name == "mandelbrot.vtl" {
        let lines = rust.stdout.split(|byte| *byte == b'\n').collect::<Vec<_>>();
        if rust.stdout.len() != 2000
            || lines.len() != 26
            || !lines[25].is_empty()
            || lines[..25].iter().any(|line| {
                line.len() != 79
                    || !line
                        .iter()
                        .all(|b| matches!(b, b' ' | b'0'..=b'9' | b'A'..=b'F'))
            })
        {
            return Err("mandelbrot.vtl: malformed 79 by 25 output".to_owned());
        }
    }
    println!(
        "{}: Rust exit=0, 6502 exit=0, disposition=portable success, stdout={} bytes",
        case.name,
        target.stdout.len()
    );
    Ok(())
}

fn run(
    binary: &str,
    source: &Path,
    cycle_limit: Option<u32>,
    runtime_input: Option<&[u8]>,
    engine: Option<&str>,
) -> Result<Output, String> {
    let mut command = Command::new(binary);
    command.arg(source);
    if let Some(engine) = engine {
        command.args(["--engine", engine]);
    }
    if source
        .file_name()
        .is_some_and(|name| name == "micro_trek.vtl")
    {
        command.args(["--seed", "0"]);
    }
    if let Some(cycle_limit) = cycle_limit {
        command.arg("--cycle-limit").arg(cycle_limit.to_string());
    }
    if engine.is_some() && runtime_input.is_some() {
        let fixture = if runtime_input
            == Some(include_bytes!("fixtures/interactive_input.txt").as_slice())
        {
            "interactive_input.txt"
        } else if runtime_input == Some(include_bytes!("fixtures/1024_quit.txt").as_slice()) {
            "1024_quit.txt"
        } else if runtime_input == Some(include_bytes!("fixtures/1024_moves.txt").as_slice()) {
            "1024_moves.txt"
        } else if runtime_input == Some(include_bytes!("fixtures/micro_trek_exit.txt").as_slice()) {
            "micro_trek_exit.txt"
        } else {
            return Err("unknown runtime input fixture".to_owned());
        };
        command.arg("--runtime-input").arg(
            Path::new(env!("CARGO_MANIFEST_DIR"))
                .join("tests/fixtures")
                .join(fixture),
        );
    }
    if source.file_name().is_some_and(|name| name == "1024.vtl") {
        if engine.is_none() {
            let fixture = if runtime_input
                == Some(include_bytes!("fixtures/1024_quit.txt").as_slice())
            {
                "1024_quit.txt"
            } else if runtime_input == Some(include_bytes!("fixtures/1024_moves.txt").as_slice()) {
                "1024_moves.txt"
            } else {
                return Err("unknown 1024 input fixture".to_owned());
            };
            let path = Path::new(env!("CARGO_MANIFEST_DIR"))
                .join("tests/fixtures")
                .join(fixture);
            let input = fs::File::open(&path)
                .map_err(|error| format!("open {}: {error}", path.display()))?;
            command.stdin(Stdio::from(input));
        }
        return output_with_deadline(&mut command, binary);
    }
    if engine.is_none()
        && let Some(input) = runtime_input
    {
        let mut child = command
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()
            .map_err(|error| format!("start {binary} for {}: {error}", source.display()))?;
        child
            .stdin
            .take()
            .expect("piped stdin")
            .write_all(input)
            .map_err(|error| format!("write stdin for {}: {error}", source.display()))?;
        child
            .wait_with_output()
            .map_err(|error| format!("wait for {binary}: {error}"))
    } else {
        command
            .output()
            .map_err(|error| format!("start {} for {}: {error}", binary, source.display()))
    }
}

// Drain both pipes concurrently so a full pipe cannot prevent completion.
// Always reap the child, including on timeout or a wait error.
fn output_with_deadline(command: &mut Command, binary: &str) -> Result<Output, String> {
    let mut child = command
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .map_err(|error| format!("start {binary}: {error}"))?;
    let mut stdout = child.stdout.take().expect("piped stdout");
    let mut stderr = child.stderr.take().expect("piped stderr");
    let out = std::thread::spawn(move || {
        let mut bytes = Vec::new();
        stdout.read_to_end(&mut bytes).map(|_| bytes)
    });
    let err = std::thread::spawn(move || {
        let mut bytes = Vec::new();
        stderr.read_to_end(&mut bytes).map(|_| bytes)
    });
    let deadline = Instant::now() + Duration::from_secs(20);
    let status = loop {
        match child.try_wait() {
            Ok(Some(status)) => break Ok(status),
            Ok(None) if Instant::now() < deadline => std::thread::sleep(Duration::from_millis(10)),
            result => {
                let _ = child.kill();
                let _ = child.wait();
                break Err(match result {
                    Ok(None) => format!("{binary} timed out"),
                    Err(error) => format!("wait for {binary}: {error}"),
                    Ok(Some(_)) => unreachable!(),
                });
            }
        }
    };
    let stdout = out
        .join()
        .map_err(|_| "stdout reader panicked".to_owned())?
        .map_err(|error| format!("read stdout: {error}"))?;
    let stderr = err
        .join()
        .map_err(|_| "stderr reader panicked".to_owned())?
        .map_err(|error| format!("read stderr: {error}"))?;
    Ok(Output {
        status: status?,
        stdout,
        stderr,
    })
}

fn assert_eight_queens_output(name: &str, output: &[u8]) -> Result<(), String> {
    let lines = output.split(|byte| *byte == b'\n').collect::<Vec<_>>();
    if lines.len() != 94 || !lines.last().is_some_and(|line| line.is_empty()) || lines[92] != b"92"
    {
        return Err(format!(
            "{name}: expected 92 solutions and trailing count 92"
        ));
    }
    let solutions = &lines[..92];
    if solutions
        .iter()
        .any(|line| line.len() != 8 || !line.iter().all(u8::is_ascii_digit))
    {
        return Err(format!("{name}: malformed solution output"));
    }
    let unique = solutions
        .iter()
        .copied()
        .collect::<std::collections::HashSet<_>>();
    if unique.len() != 92 {
        return Err(format!(
            "{name}: expected 92 distinct solutions, found {}",
            unique.len()
        ));
    }
    Ok(())
}

fn describe_bytes(bytes: &[u8]) -> String {
    format!("{} bytes ({bytes:?})", bytes.len())
}
