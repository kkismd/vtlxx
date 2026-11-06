use std::fs;
use std::path::PathBuf;
use std::process::Command;
use std::sync::atomic::{AtomicUsize, Ordering};
use vtlxx::{CompileError, Machine, SourceError};

const SUCCESS: &[(&str, &str, &[u8])] = &[(
    "success",
    include_str!("fixtures/long_write/success.vtl"),
    b"abr8",
)];
const INVALID: &str = include_str!("fixtures/long_write/invalid.vtl");
const PREFIX: &str = "A=7 ?=\"ok\" &=persist [ B=~ ] ";

#[test]
fn rust_shared_success_and_stack() {
    for (name, source, expected) in SUCCESS {
        let mut machine = Machine::new();
        machine.execute_source(source).unwrap();
        assert_eq!(machine.output(), *expected, "{name}");
        assert!(machine.stack().is_empty(), "{name}");
    }
}

#[test]
fn rust_shared_failures_keep_completed_effects_and_bindings() {
    for source in INVALID.lines() {
        let mut machine = Machine::new();
        assert!(
            matches!(
                machine.execute_source(&format!("{PREFIX}{source}")),
                Err(SourceError::Compile(_))
            ),
            "{source}"
        );
        assert_eq!(machine.output(), b"ok", "{source}");
        assert_eq!(machine.register(0), Some(7), "{source}");
        machine.execute_source("persist=42").unwrap();
        assert_eq!(machine.register(1), Some(42), "{source}");
        if source.starts_with("&=open") {
            assert_eq!(
                machine.execute_source("open=()"),
                Err(SourceError::Compile(CompileError::UndefinedBinding))
            );
        }
    }
}

static NEXT: AtomicUsize = AtomicUsize::new(0);
struct SourceFile(PathBuf);
impl SourceFile {
    fn new(source: &str) -> Self {
        let path = std::env::temp_dir().join(format!(
            "vtlxx-conformance-{}-{}.vtl",
            std::process::id(),
            NEXT.fetch_add(1, Ordering::Relaxed)
        ));
        fs::write(&path, source).unwrap();
        Self(path)
    }
}
impl Drop for SourceFile {
    fn drop(&mut self) {
        let _ = fs::remove_file(&self.0);
    }
}

#[test]
#[ignore = "requires ca65, ld65, and sim65; run with --ignored"]
fn sim65_shared_sources_match_expected_bytes_and_failure_profile() {
    let run = |source: &str| {
        let file = SourceFile::new(source);
        Command::new(env!("CARGO_BIN_EXE_vtlxx6502"))
            .arg(&file.0)
            .args(["--cycle-limit", "200000000"])
            .output()
            .unwrap()
    };
    for (name, source, expected) in SUCCESS {
        let output = run(source);
        assert!(output.status.success(), "{name}: {output:?}");
        assert_eq!(output.stdout, *expected, "{name}");
    }
    for source in INVALID.lines() {
        let output = run(&format!("{PREFIX}{source}"));
        assert!(!output.status.success(), "{source}: {output:?}");
        assert!(output.stdout.is_empty(), "{source}: {output:?}");
        assert!(
            String::from_utf8_lossy(&output.stderr).contains("compilation failed"),
            "{source}: {output:?}"
        );
    }
}
