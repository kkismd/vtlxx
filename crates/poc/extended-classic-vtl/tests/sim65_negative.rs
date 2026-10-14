use std::ffi::OsStr;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::{Command, Output, Stdio};
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::{SystemTime, UNIX_EPOCH};

const CYCLE_LIMIT: &str = "4000000";
static NEXT_TEMP_ID: AtomicU64 = AtomicU64::new(0);

struct TestDir(PathBuf);

impl TestDir {
    fn new() -> Self {
        let nonce = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let path = std::env::temp_dir().join(format!(
            "vtlxx-negative-sim65-{}-{nonce}-{}",
            std::process::id(),
            NEXT_TEMP_ID.fetch_add(1, Ordering::Relaxed)
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

fn run(tool: &str, args: &[&OsStr], input: &[u8]) -> Output {
    use std::io::Write;
    let mut child = Command::new(tool)
        .args(args)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .unwrap_or_else(|error| panic!("start {tool}: {error}"));
    child.stdin.take().unwrap().write_all(input).unwrap();
    child.wait_with_output().unwrap()
}

fn frame(source: &[u8]) -> Vec<u8> {
    let length = u16::try_from(source.len()).expect("fixture source length");
    let mut input = Vec::with_capacity(2 + source.len());
    input.extend_from_slice(&length.to_le_bytes());
    input.extend_from_slice(source);
    input
}

fn build_probe(temp: &TestDir) -> PathBuf {
    let root = Path::new(env!("CARGO_MANIFEST_DIR"));
    let sources = [
        "tests/fixtures/sim65/negative_compile_probe.s",
        "6502/frontend/basic.s",
        "6502/frontend/template.s",
        "6502/frontend/template_reader.s",
        "6502/frontend/source.s",
        "6502/compiler/arena.s",
        "6502/compiler/emitter.s",
        "6502/compiler/owner.s",
        "6502/compiler/binding.s",
        "6502/compiler/work_stack.s",
        "6502/runtime/state.s",
        "6502/runtime/primitives.s",
        "6502/runtime/arithmetic.s",
        "6502/target/sim65_adapter.s",
    ];
    let mut objects = Vec::with_capacity(sources.len());
    for (index, source) in sources.iter().enumerate() {
        let object = temp.0.join(format!("{index}.o"));
        let source_path = root.join(source);
        let output = Command::new("ca65")
            .args(["-t", "sim6502"])
            .arg(&source_path)
            .arg("-o")
            .arg(&object)
            .output()
            .unwrap_or_else(|error| panic!("start ca65: {error}"));
        assert!(output.status.success(), "assemble {source}: {output:?}");
        objects.push(object);
    }

    let executable = temp.0.join("negative-probe.prg");
    let mut link = Command::new("ld65");
    link.args(["-t", "sim6502", "-o"])
        .arg(&executable)
        .args(&objects)
        .arg("sim6502.lib");
    let output = link
        .output()
        .unwrap_or_else(|error| panic!("start ld65: {error}"));
    assert!(output.status.success(), "link probe: {output:?}");
    executable
}

fn run_probe(executable: &Path, source: &[u8]) -> Output {
    run(
        "sim65",
        &[
            OsStr::new("-x"),
            OsStr::new(CYCLE_LIMIT),
            executable.as_os_str(),
        ],
        &frame(source),
    )
}

fn cli(temp: &TestDir, name: &str, source: &[u8]) -> Output {
    let path = temp.0.join(format!("{name}.vtl"));
    fs::write(&path, source).unwrap();
    Command::new(env!("CARGO_BIN_EXE_ecvtl6502"))
        .arg(path)
        .args(["--cycle-limit", CYCLE_LIMIT])
        .output()
        .unwrap()
}

fn arena_source(count: usize) -> Vec<u8> {
    let mut source = b"?=42\n".to_vec();
    for _ in 0..count {
        source.extend_from_slice(b"A=1\n");
    }
    source
}

fn if_nesting_source(depth: usize) -> Vec<u8> {
    let mut source = b"?=42\n".to_vec();
    for _ in 0..depth {
        source.extend_from_slice(b"%=1 [ ");
    }
    source.extend_from_slice(b"?=9");
    for _ in 0..depth {
        source.extend_from_slice(b" ]");
    }
    source
}

fn labels_source(count: usize) -> Vec<u8> {
    let mut source = b"?=42\n".to_vec();
    for index in 0..count {
        if index > 0 {
            source.push(b' ');
        }
        source.extend_from_slice(format!("^={index}").as_bytes());
    }
    source
}

fn fixups_source(count: usize) -> Vec<u8> {
    let mut source = b"?=42\n".to_vec();
    let entries = (0..count)
        .map(|index| format!("#={index}"))
        .chain((0..count).map(|index| format!("^={index}")));
    for (index, entry) in entries.enumerate() {
        if index > 0 {
            source.push(b' ');
        }
        source.extend_from_slice(entry.as_bytes());
    }
    source
}

fn assert_cli_failure(temp: &TestDir, name: &str, source: &[u8], diagnostic: &str) {
    let output = cli(temp, name, source);
    assert!(!output.status.success(), "{name}: {output:?}");
    assert!(output.stdout.is_empty(), "{name}: {:?}", output.stdout);
    assert!(
        String::from_utf8_lossy(&output.stderr).contains(diagnostic),
        "{name}: {:?}",
        output.stderr
    );
    assert!(
        !String::from_utf8_lossy(&output.stderr).contains("cycle limit"),
        "{name}: {:?}",
        output.stderr
    );
}

#[test]
#[ignore = "requires ca65, ld65, and sim65; run with --ignored --nocapture"]
fn compile_failures_do_not_start_runtime_and_report_expected_causes() {
    let temp = TestDir::new();
    let probe = build_probe(&temp);

    let success = cli(&temp, "success-control", b"?=42\n");
    assert!(success.status.success(), "{success:?}");
    assert_eq!(success.stdout, b"42");

    // The IDs and byte lengths match docs/ecvtl-6502-negative-e2e-investigation.md.
    let cases: Vec<(&str, Vec<u8>, &str, i32)> = vec![
        ("syntax", b"?=42\nA=".to_vec(), "syntax error", 2),
        (
            "undefined-write",
            b"?=42\na=()\n".to_vec(),
            "compiler resource error",
            8,
        ),
        ("arena-170", arena_source(170), "compiler resource error", 1),
        ("if-depth-17", if_nesting_source(17), "syntax error", 2),
        ("labels-33", labels_source(33), "compiler resource error", 5),
        ("fixups-33", fixups_source(33), "compiler resource error", 6),
    ];

    for (name, source, diagnostic, expected_probe_status) in cases {
        let failure = cli(&temp, name, &source);
        assert!(!failure.status.success(), "{name}: {failure:?}");
        assert!(failure.stdout.is_empty(), "{name}: {:?}", failure.stdout);
        assert!(
            String::from_utf8_lossy(&failure.stderr).contains(diagnostic),
            "{name}: {:?}",
            failure.stderr
        );
        assert!(
            !String::from_utf8_lossy(&failure.stderr).contains("cycle limit"),
            "{name}: {:?}",
            failure.stderr
        );
        assert_eq!(
            run_probe(&probe, &source).status.code(),
            Some(expected_probe_status),
            "{name} (source length {}): expected internal status {expected_probe_status}",
            source.len()
        );
    }

    // Boundary controls establish that resource fixtures fail at the intended limit.
    for (name, source, expected) in [
        ("arena-169", arena_source(169), &b"42"[..]),
        ("parentheses-depth-16", parentheses_source(16), &b"42"[..]),
        ("if-depth-16", if_nesting_source(16), &b"429"[..]),
        ("while-depth-16", while_nesting_source(16), &b"42"[..]),
        ("labels-32", labels_source(32), &b"42"[..]),
        ("fixups-32", fixups_source(32), &b"42"[..]),
    ] {
        let output = cli(&temp, name, &source);
        assert!(output.status.success(), "{name}: {output:?}");
        assert_eq!(output.stdout, expected, "{name}");
    }

    assert_cli_failure(
        &temp,
        "while-depth-17",
        &while_nesting_source(17),
        "syntax error",
    );
    assert_cli_failure(
        &temp,
        "parentheses-depth-17",
        &parentheses_source(17),
        "syntax error",
    );
}

fn parentheses_source(depth: usize) -> Vec<u8> {
    let mut source = b"?=42\nA=".to_vec();
    source.extend(std::iter::repeat_n(b'(', depth));
    source.push(b'1');
    source.extend(std::iter::repeat_n(b')', depth));
    source
}

fn while_nesting_source(depth: usize) -> Vec<u8> {
    let mut source = b"?=42\n~=0 ".to_vec();
    for _ in 0..depth {
        source.extend_from_slice(b"*=() [ ~=0 ] [ ");
    }
    source.extend_from_slice(b"?=9");
    for _ in 0..depth {
        source.extend_from_slice(b" ]");
    }
    source
}
