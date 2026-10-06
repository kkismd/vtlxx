use std::ffi::{OsStr, OsString};
use std::fs;
use std::path::{Path, PathBuf};
use std::process::{Command, Output, Stdio};
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::{SystemTime, UNIX_EPOCH};

const CYCLE_LIMIT: &str = "2000000";
static NEXT_TEMP_ID: AtomicU64 = AtomicU64::new(0);

struct TempDir(PathBuf);

impl TempDir {
    fn new() -> Result<Self, String> {
        let nonce = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map_err(|error| format!("temporary directory clock: {error}"))?
            .as_nanos();
        let path = std::env::temp_dir().join(format!(
            "vtlxx-runtime-sim65-{}-{nonce}-{}",
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

fn build_and_run(fixture: &str, probe_case: Option<u8>) -> Result<Output, String> {
    let root = Path::new(env!("CARGO_MANIFEST_DIR"));
    let sources = [
        root.join("tests/fixtures/sim65")
            .join(format!("{fixture}.s")),
        root.join("6502/runtime/state.s"),
        root.join("6502/runtime/primitives.s"),
        root.join("6502/runtime/arithmetic.s"),
        root.join("6502/target/sim65_adapter.s"),
    ];
    let fixture_include = root.join("tests/fixtures/sim65");
    let case_define = probe_case.map(|case| OsString::from(format!("TEST_CASE={case}")));
    let temp = TempDir::new()?;
    let mut objects = Vec::new();
    for (index, source) in sources.iter().enumerate() {
        let object = temp.0.join(format!("{index}.o"));
        let mut args = vec![
            OsStr::new("-t"),
            OsStr::new("sim6502"),
            OsStr::new("-I"),
            fixture_include.as_os_str(),
        ];
        if probe_case.is_some() && index == 1 {
            args.extend([OsStr::new("-D"), OsStr::new("RT_TEST_PROBE")]);
        }
        if let Some(case_define) = &case_define
            && index == 0
        {
            args.extend([OsStr::new("-D"), case_define.as_os_str()]);
        }
        args.extend([source.as_os_str(), OsStr::new("-o"), object.as_os_str()]);
        let output = run_tool("ca65", "assemble", &args)?;
        require_success("assemble", "ca65", &output)?;
        objects.push(object);
    }

    let executable = temp.0.join("runtime.prg");
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

fn expect(fixture: &str, status: i32, stdout: &[u8]) {
    let output = build_and_run(fixture, None).unwrap();
    assert_eq!(output.status.code(), Some(status), "{fixture}: {output:?}");
    assert_eq!(output.stdout, stdout, "{fixture}: {output:?}");
}

fn expect_probe(case: u8, status: i32) {
    let output = build_and_run("runtime_probe", Some(case)).unwrap();
    assert_eq!(
        output.status.code(),
        Some(status),
        "case {case}: {output:?}"
    );
    assert_eq!(output.stdout, b"K", "case {case}: {output:?}");
}

#[test]
#[ignore = "requires ca65, ld65, and sim65; run with --ignored"]
fn runtime_state_and_ring_aliases() {
    expect(
        "runtime_state",
        0,
        b"0,0,0,0,4660,22136,4660,22136,22136,4660,22136,0,0,0,0",
    );
}

#[test]
#[ignore = "requires ca65, ld65, and sim65; run with --ignored"]
fn arithmetic_matches_signed_cell_edges() {
    expect(
        "runtime_arithmetic",
        0,
        b"-32768,32767,-32768,-2,-2,-1,1,-32768,0",
    );
}

#[test]
#[ignore = "requires ca65, ld65, and sim65; run with --ignored"]
fn comparisons_are_signed_and_normalized() {
    expect("runtime_comparisons", 0, b"1,0,1,0,1,0,0,1,0,1,0,1");
}

#[test]
#[ignore = "requires ca65, ld65, and sim65; run with --ignored"]
fn output_formats_numbers_and_low_byte_character() {
    expect("runtime_output", 0, b"0,-12,32767,-32768,A");
}

#[test]
#[ignore = "requires ca65, ld65, and sim65; run with --ignored"]
fn sixteen_stack_cells_fit() {
    expect(
        "runtime_stack_16",
        0,
        b"15,14,13,12,11,10,9,8,7,6,5,4,3,2,1,0",
    );
}

#[test]
#[ignore = "requires ca65, ld65, and sim65; run with --ignored"]
fn fatal_errors_halt_with_distinct_status() {
    for (fixture, status) in [
        ("runtime_underflow", 1),
        ("runtime_overflow", 2),
        ("runtime_div_zero", 3),
        ("runtime_rem_zero", 4),
    ] {
        expect(fixture, status, b"");
    }
}

#[test]
#[ignore = "requires ca65, ld65, and sim65; run with --ignored"]
fn fatal_helpers_preserve_committed_operands_and_depth() {
    for (case, status) in [(1, 1), (2, 2), (3, 3), (4, 4), (5, 1), (6, 1)] {
        expect_probe(case, status);
    }
}
