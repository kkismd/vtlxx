use std::ffi::OsStr;
use std::fs;
use std::path::Path;
use std::process::{Command, Output};

const EXAMPLES_DIR: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/examples");

#[derive(Clone, Copy, Debug)]
enum Disposition {
    Portable {
        expected_stdout: Option<&'static [u8]>,
        cycle_limit: Option<u32>,
    },
}

#[derive(Clone, Copy, Debug)]
struct ExampleCase {
    name: &'static str,
    disposition: Disposition,
}

const EXAMPLES: &[ExampleCase] = &[
    ExampleCase {
        name: "eight_queens.vtl",
        disposition: Disposition::Portable {
            expected_stdout: Some(include_bytes!("fixtures/eight_queens_example_output.txt")),
            cycle_limit: Some(256_000_000),
        },
    },
    ExampleCase {
        name: "eight_queens_storage.vtl",
        disposition: Disposition::Portable {
            expected_stdout: Some(include_bytes!("fixtures/eight_queens_example_output.txt")),
            cycle_limit: Some(256_000_000),
        },
    },
    ExampleCase {
        name: "maze_dfs.vtl",
        disposition: Disposition::Portable {
            expected_stdout: Some(b"7\n"),
            cycle_limit: None,
        },
    },
    ExampleCase {
        name: "p0a_template_calibration.vtl",
        disposition: Disposition::Portable {
            expected_stdout: Some(b"58\n20\n40\n"),
            cycle_limit: None,
        },
    },
    ExampleCase {
        name: "rpn_evaluator.vtl",
        disposition: Disposition::Portable {
            expected_stdout: Some(b"7\n"),
            cycle_limit: None,
        },
    },
    ExampleCase {
        name: "sim65_cli.vtl",
        disposition: Disposition::Portable {
            expected_stdout: Some(b"42"),
            cycle_limit: None,
        },
    },
];

#[test]
#[ignore = "requires ca65, ld65, and sim65; run with --ignored"]
fn all_examples_are_portable_and_match_the_rust_reference() {
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

    let mut failures = Vec::new();
    for case in EXAMPLES {
        if let Err(error) = check_example(case, examples_dir) {
            eprintln!("{error}");
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
    let rust = run(env!("CARGO_BIN_EXE_ecvtl"), &source, None)
        .map_err(|error| format!("{}: {error}", case.name))?;
    if rust.status.code().is_none() {
        return Err(format!("{}: Rust ecvtl terminated by signal", case.name));
    }
    if !rust.status.success() {
        return Err(format!(
            "{}: Rust reference failed (exit {:?}): {}",
            case.name,
            rust.status.code(),
            String::from_utf8_lossy(&rust.stderr)
        ));
    }

    let cycle_limit = match case.disposition {
        Disposition::Portable { cycle_limit, .. } => cycle_limit,
    };
    let target = run(env!("CARGO_BIN_EXE_ecvtl6502"), &source, cycle_limit)
        .map_err(|error| format!("{}: {error}", case.name))?;
    if target.status.code().is_none() {
        return Err(format!(
            "{}: 6502 ecvtl6502 terminated by signal",
            case.name
        ));
    }

    match case.disposition {
        Disposition::Portable {
            expected_stdout, ..
        } => {
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
            println!(
                "{}: Rust exit=0, 6502 exit=0, disposition=portable success, stdout={} bytes",
                case.name,
                target.stdout.len()
            );
        }
    }
    Ok(())
}

fn run(binary: &str, source: &Path, cycle_limit: Option<u32>) -> Result<Output, String> {
    let mut command = Command::new(binary);
    command.arg(source);
    if let Some(cycle_limit) = cycle_limit {
        command.arg("--cycle-limit").arg(cycle_limit.to_string());
    }
    command
        .output()
        .map_err(|error| format!("start {} for {}: {error}", binary, source.display()))
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
