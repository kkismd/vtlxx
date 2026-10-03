use std::{
    fs::{self, OpenOptions},
    io::Write,
    path::PathBuf,
    process::{Command, Output},
    sync::atomic::{AtomicU64, Ordering},
};

static NEXT_FILE: AtomicU64 = AtomicU64::new(0);

struct SourceFile(PathBuf);

impl SourceFile {
    fn new(contents: &[u8]) -> Self {
        let id = NEXT_FILE.fetch_add(1, Ordering::Relaxed);
        let path = std::env::temp_dir().join(format!("ecvtl-cli-{}-{id}.vtl", std::process::id()));
        let mut file = OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(&path)
            .unwrap();
        file.write_all(contents).unwrap();
        Self(path)
    }
}

impl Drop for SourceFile {
    fn drop(&mut self) {
        fs::remove_file(&self.0).unwrap();
    }
}

fn ecvtl(args: &[&std::ffi::OsStr]) -> Output {
    Command::new(env!("CARGO_BIN_EXE_ecvtl"))
        .args(args)
        .output()
        .unwrap()
}

#[test]
fn eight_queens_output_is_exact() {
    let path = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/eight_queens.vtl");
    let output = ecvtl(&[path.as_os_str()]);
    assert!(output.status.success(), "{:?}", output.stderr);
    assert_eq!(output.stdout, b"92");
    assert!(output.stderr.is_empty());
}

#[test]
fn machine_newline_is_the_only_newline() {
    let source = SourceFile::new(b"?=\"hello\" ?=");
    let output = ecvtl(&[source.0.as_os_str()]);
    assert!(output.status.success(), "{:?}", output.stderr);
    assert_eq!(output.stdout, b"hello\n");
    assert!(output.stderr.is_empty());
}

#[test]
fn empty_machine_output_stays_empty() {
    let source = SourceFile::new(b"A=1");
    let output = ecvtl(&[source.0.as_os_str()]);
    assert!(output.status.success(), "{:?}", output.stderr);
    assert!(output.stdout.is_empty());
    assert!(output.stderr.is_empty());
}

#[test]
fn missing_or_extra_arguments_are_usage_errors() {
    let source = SourceFile::new(b"?=1");
    for args in [vec![], vec![source.0.as_os_str(), source.0.as_os_str()]] {
        let output = ecvtl(&args);
        assert!(!output.status.success());
        assert!(output.stdout.is_empty());
        assert!(!output.stderr.is_empty());
    }
}

#[test]
fn missing_file_and_invalid_utf8_are_input_errors() {
    let source = SourceFile::new(&[0xff]);
    let missing = source.0.with_extension("missing");
    for path in [&missing, &source.0] {
        let output = ecvtl(&[path.as_os_str()]);
        assert!(!output.status.success());
        assert!(output.stdout.is_empty());
        assert!(!output.stderr.is_empty());
    }
}

#[test]
fn compile_and_runtime_failures_report_errors_without_stdout() {
    let compile = SourceFile::new(b"A=1+");
    let runtime = SourceFile::new(b"?=\"prior\" A=1/0");
    for (path, category) in [(&compile.0, "Compile"), (&runtime.0, "Runtime")] {
        let output = ecvtl(&[path.as_os_str()]);
        assert!(!output.status.success());
        assert!(output.stdout.is_empty());
        assert!(String::from_utf8_lossy(&output.stderr).contains(category));
    }
}
