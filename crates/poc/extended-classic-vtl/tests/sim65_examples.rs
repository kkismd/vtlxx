use std::ffi::OsStr;
use std::fs;
use std::path::Path;
use std::process::{Command, Output};

const EXAMPLES_DIR: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/examples");

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum FailureKind {
    FrontendSyntax,
    CycleLimit,
}

#[derive(Clone, Copy, Debug)]
enum Disposition {
    Portable {
        expected_stdout: Option<&'static [u8]>,
    },
    KnownNonconformant {
        kind: FailureKind,
        exit_code: i32,
        expected_rust_stdout: Option<&'static [u8]>,
        reason: &'static str,
    },
}

#[derive(Clone, Copy, Debug)]
struct ExampleCase {
    name: &'static str,
    disposition: Disposition,
}

// Initial classifications are based on ecvtl/ecvtl6502 runs on 2026-10-12.
// A known failure moving to success intentionally fails until this table changes.
const EXAMPLES: &[ExampleCase] = &[
    ExampleCase {
        name: "eight_queens.vtl",
        disposition: Disposition::KnownNonconformant {
            kind: FailureKind::CycleLimit,
            exit_code: 126,
            expected_rust_stdout: None,
            reason: "observed sim65 cycle-limit termination; this does not establish syntax conformance",
        },
    },
    ExampleCase {
        name: "eight_queens_storage.vtl",
        disposition: Disposition::KnownNonconformant {
            kind: FailureKind::CycleLimit,
            exit_code: 126,
            expected_rust_stdout: None,
            reason: "6502 frontend accepts the source, but sim65 reaches its cycle limit before completion",
        },
    },
    ExampleCase {
        name: "maze_dfs.vtl",
        disposition: Disposition::Portable {
            expected_stdout: Some(b"7\n"),
        },
    },
    ExampleCase {
        name: "p0a_template_calibration.vtl",
        disposition: Disposition::Portable {
            expected_stdout: Some(b"58\n20\n40\n"),
        },
    },
    ExampleCase {
        name: "rpn_evaluator.vtl",
        disposition: Disposition::Portable {
            expected_stdout: Some(b"7\n"),
        },
    },
    ExampleCase {
        name: "sim65_cli.vtl",
        disposition: Disposition::Portable {
            expected_stdout: Some(b"42"),
        },
    },
];

#[test]
#[ignore = "requires ca65, ld65, and sim65; run with --ignored"]
fn examples_match_the_rust_reference_or_the_measured_6502_disposition() {
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
    let rust = run(env!("CARGO_BIN_EXE_ecvtl"), &source)
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

    let target = run(env!("CARGO_BIN_EXE_ecvtl6502"), &source)
        .map_err(|error| format!("{}: {error}", case.name))?;
    if target.status.code().is_none() {
        return Err(format!(
            "{}: 6502 ecvtl6502 terminated by signal",
            case.name
        ));
    }

    match case.disposition {
        Disposition::Portable { expected_stdout } => {
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
            println!(
                "{}: Rust exit=0, 6502 exit=0, disposition=portable success, stdout={} bytes",
                case.name,
                target.stdout.len()
            );
        }
        Disposition::KnownNonconformant {
            kind,
            exit_code,
            expected_rust_stdout,
            reason,
        } => {
            if let Some(expected) = expected_rust_stdout
                && rust.stdout != expected
            {
                return Err(format!(
                    "{}: fixed Rust reference bytes differ (actual {}, expected {})",
                    case.name,
                    describe_bytes(&rust.stdout),
                    describe_bytes(expected)
                ));
            }
            let observed_code = target.status.code().unwrap_or(-1);
            let observed_kind = classify_failure(&target);
            if observed_code != exit_code || observed_kind != Some(kind) {
                return Err(format!(
                    "{}: known failure changed (expected {:?}/{:?}, got {:?}/{:?}): {}",
                    case.name,
                    kind,
                    exit_code,
                    observed_kind,
                    observed_code,
                    String::from_utf8_lossy(&target.stderr)
                ));
            }
            println!(
                "{}: Rust exit=0, 6502 exit={}, disposition=known {:?}, stdout={} bytes; {}",
                case.name,
                observed_code,
                kind,
                target.stdout.len(),
                reason
            );
        }
    }
    Ok(())
}

fn run(binary: &str, source: &Path) -> Result<Output, String> {
    Command::new(binary)
        .arg(source)
        .output()
        .map_err(|error| format!("start {} for {}: {error}", binary, source.display()))
}

fn describe_bytes(bytes: &[u8]) -> String {
    format!("{} bytes ({bytes:?})", bytes.len())
}

fn classify_failure(output: &Output) -> Option<FailureKind> {
    let code = output.status.code()?;
    let stderr = String::from_utf8_lossy(&output.stderr);
    if stderr.to_ascii_lowercase().contains("cycle limit") {
        Some(FailureKind::CycleLimit)
    } else if code & 0x80 != 0 && code & 0x7f == 2 {
        Some(FailureKind::FrontendSyntax)
    } else {
        None
    }
}
