use std::ffi::OsStr;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::{Command, Output, Stdio};
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::{SystemTime, UNIX_EPOCH};

const CYCLE_LIMIT: &str = "4000000";
static NEXT_TEMP_ID: AtomicU64 = AtomicU64::new(0);

struct TempDir(PathBuf);

impl TempDir {
    fn new() -> Result<Self, String> {
        let nonce = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map_err(|error| format!("temporary directory clock: {error}"))?
            .as_nanos();
        let path = std::env::temp_dir().join(format!(
            "vtlxx-backend-sim65-{}-{nonce}-{}",
            std::process::id(),
            NEXT_TEMP_ID.fetch_add(1, Ordering::Relaxed)
        ));
        fs::create_dir(&path)
            .map_err(|error| format!("create temporary directory {}: {error}", path.display()))?;
        Ok(Self(path))
    }
}

impl Drop for TempDir {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}

fn run_tool(tool: &str, stage: &str, args: &[&OsStr]) -> Result<Output, String> {
    let child = Command::new(tool)
        .args(args)
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .map_err(|error| format!("{stage}: could not start {tool}: {error}"))?;
    child
        .wait_with_output()
        .map_err(|error| format!("{stage}: could not collect {tool} output: {error}"))
}

fn require_success(stage: &str, tool: &str, output: &Output) -> Result<(), String> {
    if output.status.success() {
        return Ok(());
    }
    Err(format!(
        "{stage}: {tool} exited with {}\nstdout: {}\nstderr: {}",
        output.status,
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    ))
}

fn build_and_run(case: u8) -> Result<Output, String> {
    let root = Path::new(env!("CARGO_MANIFEST_DIR"));
    let sources = [
        root.join("tests/fixtures/sim65/backend_probe.s"),
        root.join("6502/compiler/arena.s"),
        root.join("6502/compiler/emitter.s"),
        root.join("6502/compiler/owner.s"),
        root.join("6502/compiler/binding.s"),
        root.join("6502/compiler/work_stack.s"),
        root.join("6502/runtime/state.s"),
        root.join("6502/runtime/primitives.s"),
        root.join("6502/runtime/arithmetic.s"),
        root.join("6502/target/sim65_adapter.s"),
    ];
    let fixture_include = root.join("tests/fixtures/sim65");
    let temp = TempDir::new()?;
    let mut objects = Vec::new();
    let case_define = format!("TEST_CASE={case}");
    for (index, source) in sources.iter().enumerate() {
        let object = temp.0.join(format!("{index}.o"));
        let mut args = vec![
            OsStr::new("-t"),
            OsStr::new("sim6502"),
            OsStr::new("-I"),
            fixture_include.as_os_str(),
        ];
        if index == 0 {
            args.extend([OsStr::new("-D"), OsStr::new(&case_define)]);
        }
        args.extend([source.as_os_str(), OsStr::new("-o"), object.as_os_str()]);
        let output = run_tool("ca65", "assemble", &args)?;
        require_success("assemble", "ca65", &output)?;
        objects.push(object);
    }

    let executable = temp.0.join("backend.prg");
    let mut args: Vec<&OsStr> = vec![
        OsStr::new("-t"),
        OsStr::new("sim6502"),
        OsStr::new("-o"),
        executable.as_os_str(),
    ];
    args.extend(objects.iter().map(|object| object.as_os_str()));
    args.push(OsStr::new("sim6502.lib"));
    let output = run_tool("ld65", "link", &args)?;
    require_success("link", "ld65", &output)?;

    run_tool(
        "sim65",
        "sim65 execute",
        &[
            OsStr::new("-x"),
            OsStr::new(CYCLE_LIMIT),
            executable.as_os_str(),
        ],
    )
}

fn expect(case: u8, stdout: &[u8]) {
    let output = build_and_run(case).unwrap();
    assert_eq!(output.status.code(), Some(0), "case {case}: {output:?}");
    assert_eq!(output.stdout, stdout, "case {case}: {output:?}");
}

#[test]
#[ignore = "requires ca65, ld65, and sim65; run with --ignored"]
fn emitted_native_code_executes_fixed_templates() {
    expect(1, b"42K");
    expect(2, b"BK");
    expect(3, b"ZNK");
}

#[test]
#[ignore = "requires ca65, ld65, and sim65; run with --ignored"]
fn patch_range_and_arena_boundaries() {
    for case in 4..=7 {
        expect(case, b"K");
    }
}

#[test]
#[ignore = "requires ca65, ld65, and sim65; run with --ignored"]
fn completed_owner_binding_and_owner_local_labels() {
    for case in 8..=9 {
        expect(case, b"K");
    }
    expect(10, b"LK");
    for case in 11..=13 {
        expect(case, b"K");
    }
    expect(15, b"K");
    expect(16, b"K");
    expect(17, b"QK");
}

#[test]
#[ignore = "requires ca65, ld65, and sim65; run with --ignored"]
fn source_work_stack_capacity_and_order() {
    expect(14, b"K");
}
