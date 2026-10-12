use std::ffi::OsStr;
use std::fs;
use std::path::{Path, PathBuf};
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
            reason: "6502 execution reaches the configured sim65 cycle limit; cause not isolated",
        },
    },
    ExampleCase {
        name: "eight_queens_storage.vtl",
        disposition: Disposition::KnownNonconformant {
            kind: FailureKind::FrontendSyntax,
            exit_code: 130,
            expected_rust_stdout: None,
            reason: "6502 frontend rejects syntax used by the example; exact construct not isolated",
        },
    },
    ExampleCase {
        name: "maze_dfs.vtl",
        disposition: Disposition::KnownNonconformant {
            kind: FailureKind::FrontendSyntax,
            exit_code: 130,
            expected_rust_stdout: Some(b"7\n"),
            reason: "%=~ is a candidate cause, not confirmed",
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

    for case in EXAMPLES {
        let source = examples_dir.join(case.name);
        let rust = run(env!("CARGO_BIN_EXE_ecvtl"), &source);
        assert_process_started(&rust, case.name, "Rust ecvtl");
        assert!(
            rust.status.success(),
            "{}: Rust reference failed (exit {:?}): {}",
            case.name,
            rust.status.code(),
            String::from_utf8_lossy(&rust.stderr)
        );

        let target = run(env!("CARGO_BIN_EXE_ecvtl6502"), &source);
        assert_process_started(&target, case.name, "6502 ecvtl6502");

        match case.disposition {
            Disposition::Portable { expected_stdout } => {
                assert!(
                    target.status.success(),
                    "{}: expected portable success, got exit {:?}: {}",
                    case.name,
                    target.status.code(),
                    String::from_utf8_lossy(&target.stderr)
                );
                assert_eq!(
                    target.stdout, rust.stdout,
                    "{}: Rust/6502 stdout bytes",
                    case.name
                );
                if let Some(expected) = expected_stdout {
                    assert_eq!(
                        rust.stdout, expected,
                        "{}: fixed expected stdout bytes",
                        case.name
                    );
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
                if let Some(expected) = expected_rust_stdout {
                    assert_eq!(
                        rust.stdout, expected,
                        "{}: fixed Rust reference bytes",
                        case.name
                    );
                }
                let observed_code = target.status.code().unwrap_or(-1);
                let observed_kind = classify_failure(&target);
                assert_eq!(
                    observed_code,
                    exit_code,
                    "{}: known failure status changed (expected {:?}, got {:?}); {}",
                    case.name,
                    exit_code,
                    observed_code,
                    String::from_utf8_lossy(&target.stderr)
                );
                assert_eq!(
                    observed_kind,
                    Some(kind),
                    "{}: known failure classification changed: {}",
                    case.name,
                    String::from_utf8_lossy(&target.stderr)
                );
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
    }
}

fn run(binary: &str, source: &PathBuf) -> Output {
    Command::new(binary)
        .arg(source)
        .output()
        .unwrap_or_else(|error| panic!("start {} for {}: {error}", binary, source.display()))
}

fn assert_process_started(output: &Output, name: &str, program: &str) {
    assert!(
        output.status.code().is_some(),
        "{name}: {program} did not exit normally: {:?}",
        output.status
    );
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
