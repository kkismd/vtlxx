use std::{
    fs::{self, OpenOptions},
    io::{Read, Write},
    path::PathBuf,
    process::{Command, Output, Stdio},
    sync::atomic::{AtomicU64, Ordering},
};

static NEXT_FILE: AtomicU64 = AtomicU64::new(0);

struct SourceFile(PathBuf);

impl SourceFile {
    fn new(contents: &[u8]) -> Self {
        let id = NEXT_FILE.fetch_add(1, Ordering::Relaxed);
        let path = std::env::temp_dir().join(format!("vtlxx-cli-{}-{id}.vtl", std::process::id()));
        let mut file = OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(&path)
            .unwrap();
        file.write_all(contents).unwrap();
        Self(path)
    }
}

#[test]
fn prompt_is_visible_before_input_and_output_is_not_duplicated() {
    let source = SourceFile::new(b"?=\"prompt\" A=$ $=A ?=() ");
    let mut child = Command::new(env!("CARGO_BIN_EXE_vtlxx"))
        .arg(&source.0)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .spawn()
        .unwrap();
    let mut stdout = child.stdout.take().unwrap();
    let mut prompt = [0; 6];
    stdout.read_exact(&mut prompt).unwrap();
    assert_eq!(&prompt, b"prompt");
    child.stdin.take().unwrap().write_all(b"Z").unwrap();
    let mut rest = Vec::new();
    stdout.read_to_end(&mut rest).unwrap();
    assert!(child.wait().unwrap().success());
    assert_eq!(rest, b"Z\n");
}

#[test]
fn empty_stdin_reports_eof() {
    let source = SourceFile::new(b"A=$ ?=A");
    let output = vtlxx(&[source.0.as_os_str()]);
    assert!(output.status.success());
    assert_eq!(output.stdout, b"-1");
}

impl Drop for SourceFile {
    fn drop(&mut self) {
        fs::remove_file(&self.0).unwrap();
    }
}

fn vtlxx(args: &[&std::ffi::OsStr]) -> Output {
    vtlxx_args(args)
}

fn vtlxx_args(args: &[&std::ffi::OsStr]) -> Output {
    Command::new(env!("CARGO_BIN_EXE_vtlxx"))
        .args(args.iter().copied())
        .output()
        .unwrap()
}

#[test]
fn eight_queens_output_is_exact() {
    let path = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/eight_queens.vtl");
    let output = vtlxx(&[path.as_os_str()]);
    assert!(output.status.success(), "{:?}", output.stderr);
    assert_eq!(output.stdout, b"92");
    assert!(output.stderr.is_empty());
}

#[test]
fn machine_newline_is_the_only_newline() {
    let source = SourceFile::new(b"?=\"hello\" ?=()");
    let output = vtlxx(&[source.0.as_os_str()]);
    assert!(output.status.success(), "{:?}", output.stderr);
    assert_eq!(output.stdout, b"hello\n");
    assert!(output.stderr.is_empty());
}

#[test]
fn empty_machine_output_stays_empty() {
    let source = SourceFile::new(b"A=1");
    let output = vtlxx(&[source.0.as_os_str()]);
    assert!(output.status.success(), "{:?}", output.stderr);
    assert!(output.stdout.is_empty());
    assert!(output.stderr.is_empty());
}

#[test]
fn fixed_seed_is_used_for_each_entropy_read() {
    let source = SourceFile::new(b"A=' B=' ?=A ?=B");
    for seed in [-32768, -1, 0, 1, 32767] {
        let seed_arg = seed.to_string();
        let output = vtlxx_args(&[
            source.0.as_os_str(),
            std::ffi::OsStr::new("--seed"),
            std::ffi::OsStr::new(&seed_arg),
        ]);
        assert!(output.status.success(), "{seed}: {:?}", output.stderr);
        assert_eq!(output.stdout, format!("{seed}{seed}").as_bytes());
        assert!(output.stderr.is_empty());
    }
}

#[test]
fn invalid_seed_options_fail_before_running_source() {
    let source = SourceFile::new(b"?=\"started\"");
    let invalid: Vec<Vec<&std::ffi::OsStr>> = vec![
        vec![source.0.as_os_str(), std::ffi::OsStr::new("--seed")],
        vec![
            source.0.as_os_str(),
            std::ffi::OsStr::new("--seed"),
            std::ffi::OsStr::new("1"),
            std::ffi::OsStr::new("--seed"),
            std::ffi::OsStr::new("2"),
        ],
        vec![
            source.0.as_os_str(),
            std::ffi::OsStr::new("--seed"),
            std::ffi::OsStr::new("32768"),
        ],
        vec![source.0.as_os_str(), std::ffi::OsStr::new("--unknown")],
    ];
    for args in invalid {
        let output = vtlxx_args(&args);
        assert!(!output.status.success());
        assert!(output.stdout.is_empty());
    }
}

#[test]
fn missing_or_extra_arguments_are_usage_errors() {
    let source = SourceFile::new(b"?=1");
    for args in [vec![], vec![source.0.as_os_str(), source.0.as_os_str()]] {
        let output = vtlxx(&args);
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
        let output = vtlxx(&[path.as_os_str()]);
        assert!(!output.status.success());
        assert!(output.stdout.is_empty());
        assert!(!output.stderr.is_empty());
    }
}

#[test]
fn compile_and_runtime_failures_report_errors() {
    let compile = SourceFile::new(b"A=1+");
    let runtime = SourceFile::new(b"?=\"prior\" A=1/0");
    for (path, category) in [(&compile.0, "Compile"), (&runtime.0, "Runtime")] {
        let output = vtlxx(&[path.as_os_str()]);
        assert!(!output.status.success());
        assert_eq!(
            output.stdout,
            if category == "Runtime" {
                b"prior".as_slice()
            } else {
                b""
            }
        );
        assert!(String::from_utf8_lossy(&output.stderr).contains(category));
    }
}

#[test]
fn while_rhs_rechecks_condition_and_rejects_old_syntax() {
    let source = SourceFile::new(b"A=0 *=A<3 [ ?=A A=A+1 ] ?=A");
    let output = vtlxx(&[source.0.as_os_str()]);
    assert!(output.status.success(), "{:?}", output.stderr);
    assert_eq!(output.stdout, b"0123");
    assert!(output.stderr.is_empty());

    let source = SourceFile::new(b"*=() [ ~=0 ] [ A=1 ]");
    let output = vtlxx(&[source.0.as_os_str()]);
    assert!(!output.status.success());
    assert!(output.stdout.is_empty());
    assert!(String::from_utf8_lossy(&output.stderr).contains("Compile"));
}
