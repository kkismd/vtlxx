use std::ffi::OsStr;
use std::fs;
use std::io::Write;
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
            "vtlxx-sim65-{}-{nonce}-{}",
            std::process::id(),
            NEXT_TEMP_ID.fetch_add(1, Ordering::Relaxed)
        ));
        fs::create_dir(&path).map_err(|error| {
            format!(
                "create temporary build directory {}: {error}",
                path.display()
            )
        })?;
        Ok(Self(path))
    }
}

impl Drop for TempDir {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}

fn run_tool(tool: &str, stage: &str, args: &[&OsStr], input: &[u8]) -> Result<Output, String> {
    let mut child = Command::new(tool)
        .args(args)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .map_err(|error| format!("{stage}: could not start {tool}: {error}"))?;
    if !input.is_empty() {
        child
            .stdin
            .take()
            .expect("piped stdin")
            .write_all(input)
            .map_err(|error| format!("{stage}: could not write stdin to {tool}: {error}"))?;
    }
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

fn build_and_run(fixture: &str, input: &[u8]) -> Result<Output, String> {
    let crate_root = Path::new(env!("CARGO_MANIFEST_DIR"));
    let adapter = crate_root.join("6502/target/sim65_adapter.s");
    let fixture = crate_root
        .join("tests/fixtures/sim65")
        .join(format!("{fixture}.s"));
    let frontend = crate_root.join("6502/frontend/source.s");
    let frontend_compile = crate_root.join("6502/frontend/compile.s");
    let compiler_sources = [
        crate_root.join("6502/compiler/arena.s"),
        crate_root.join("6502/compiler/binding.s"),
        crate_root.join("6502/compiler/emitter.s"),
        crate_root.join("6502/compiler/owner.s"),
        crate_root.join("6502/compiler/work_stack.s"),
        crate_root.join("6502/runtime/state.s"),
        crate_root.join("6502/runtime/arithmetic.s"),
        crate_root.join("6502/runtime/primitives.s"),
    ];
    let temp = TempDir::new()?;
    let adapter_object = temp.0.join("adapter.o");
    let fixture_object = temp.0.join("fixture.o");
    let frontend_object = temp.0.join("frontend.o");
    let compile_object = temp.0.join("compile.o");
    let mut compiler_objects = Vec::new();
    let executable = temp.0.join("smoke.prg");

    for (source, object) in [(&adapter, &adapter_object), (&fixture, &fixture_object)] {
        let output = run_tool(
            "ca65",
            "assemble",
            &[
                OsStr::new("-t"),
                OsStr::new("sim6502"),
                source.as_os_str(),
                OsStr::new("-o"),
                object.as_os_str(),
            ],
            &[],
        )?;
        require_success("assemble", "ca65", &output)?;
    }

    let mut objects = vec![fixture_object.as_os_str(), adapter_object.as_os_str()];
    if fixture.file_stem().and_then(|name| name.to_str()) == Some("frontend_framing") {
        let output = run_tool(
            "ca65",
            "assemble frontend",
            &[
                OsStr::new("-t"),
                OsStr::new("sim6502"),
                frontend.as_os_str(),
                OsStr::new("-o"),
                frontend_object.as_os_str(),
            ],
            &[],
        )?;
        require_success("assemble frontend", "ca65", &output)?;
        objects.push(frontend_object.as_os_str());
    }
    if fixture.file_stem().and_then(|name| name.to_str()) == Some("frontend_compile") {
        for (index, source) in compiler_sources.iter().enumerate() {
            let object = temp.0.join(format!("compiler-{index}.o"));
            let output = run_tool(
                "ca65",
                "assemble compiler",
                &[
                    OsStr::new("-t"),
                    OsStr::new("sim6502"),
                    source.as_os_str(),
                    OsStr::new("-o"),
                    object.as_os_str(),
                ],
                &[],
            )?;
            require_success("assemble compiler", "ca65", &output)?;
            compiler_objects.push(object);
        }
        for (source, object) in [
            (&frontend, &frontend_object),
            (&frontend_compile, &compile_object),
        ] {
            let output = run_tool(
                "ca65",
                "assemble frontend compiler",
                &[
                    OsStr::new("-t"),
                    OsStr::new("sim6502"),
                    source.as_os_str(),
                    OsStr::new("-o"),
                    object.as_os_str(),
                ],
                &[],
            )?;
            require_success("assemble frontend compiler", "ca65", &output)?;
        }
        objects.push(frontend_object.as_os_str());
        objects.push(compile_object.as_os_str());
        objects.extend(compiler_objects.iter().map(|path| path.as_os_str()));
    }

    let mut link_args = vec![
        OsStr::new("-t"),
        OsStr::new("sim6502"),
        OsStr::new("-o"),
        executable.as_os_str(),
    ];
    link_args.extend(objects);
    link_args.push(OsStr::new("sim6502.lib"));
    let output = run_tool("ld65", "link", &link_args, &[])?;
    require_success("link", "ld65", &output)?;

    run_tool(
        "sim65",
        "sim65 execute",
        &[
            OsStr::new("-x"),
            OsStr::new(CYCLE_LIMIT),
            executable.as_os_str(),
        ],
        input,
    )
}

fn validate_run(output: &Output, status: i32, stdout: &[u8]) -> Result<(), String> {
    if output.status.code() != Some(status) || output.stdout != stdout {
        return Err(format!(
            "sim65 execute / stdout/status validation: expected status {status} and stdout {stdout:?}; got status {}, stdout {:?}, stderr {}",
            output.status,
            output.stdout,
            String::from_utf8_lossy(&output.stderr)
        ));
    }
    Ok(())
}

#[test]
fn missing_tool_reports_start_failure_with_stage_and_tool() {
    let tool = "vtlxx-sim65-tool-that-does-not-exist";
    let error = run_tool(tool, "assemble", &[], &[]).unwrap_err();
    assert!(error.contains("assemble"), "{error}");
    assert!(error.contains(tool), "{error}");
    assert!(error.contains("could not start"), "{error}");
}

#[test]
#[ignore = "requires ca65, ld65, and sim65; run with --ignored"]
fn normal_halt_reports_status() {
    let output = build_and_run("normal_halt", &[]).unwrap();
    validate_run(&output, 7, b"").unwrap();
}

#[test]
#[ignore = "requires ca65, ld65, and sim65; run with --ignored"]
fn serial_out_is_exact() {
    let output = build_and_run("serial_out", &[]).unwrap();
    validate_run(&output, 0, b"OK").unwrap();
}

#[test]
#[ignore = "requires ca65, ld65, and sim65; run with --ignored"]
fn serial_input_round_trips() {
    let output = build_and_run("serial_echo", b"Z").unwrap();
    validate_run(&output, 0, b"Z").unwrap();
    let output = build_and_run("serial_echo", &[0xff]).unwrap();
    validate_run(&output, 0, &[0xff]).unwrap();
}

#[test]
#[ignore = "requires ca65, ld65, and sim65; run with --ignored"]
fn source_framing_stops_before_runtime_input() {
    let output = build_and_run("frontend_framing", &[3, 0, b'A', b'B', b'C', b'Z']).unwrap();
    validate_run(&output, 0, b"ABCZ").unwrap();
    let output = build_and_run("frontend_framing", &[0, 0, b'Z']).unwrap();
    validate_run(&output, 0, b"Z").unwrap();
}

#[test]
#[ignore = "requires ca65, ld65, and sim65; run with --ignored"]
fn source_compile_run_executes_native_output() {
    let source = b"A=40\nA=A+2\n?=A\n";
    let mut input = vec![source.len() as u8, 0];
    input.extend_from_slice(source);
    input.push(b'Z');
    let output = build_and_run("frontend_compile", &input).unwrap();
    validate_run(&output, 0, b"42").unwrap();
}

#[test]
#[ignore = "requires ca65, ld65, and sim65; run with --ignored"]
fn malformed_source_does_not_start_runtime() {
    let source = b"A=1+\n";
    let mut input = vec![source.len() as u8, 0];
    input.extend_from_slice(source);
    input.push(b'Z');
    let output = build_and_run("frontend_compile", &input).unwrap();
    validate_run(&output, 1, b"").unwrap();
}

#[test]
#[ignore = "requires ca65, ld65, and sim65; run with --ignored"]
fn source_framing_accepts_ff_and_256_byte_lengths() {
    for length in [255usize, 256] {
        let mut input = vec![length as u8, (length >> 8) as u8];
        input.extend(std::iter::repeat_n(b'A', length));
        input.push(b'Z');
        let output = build_and_run("frontend_framing", &input).unwrap();
        let mut expected = vec![b'A'; length];
        expected.push(b'Z');
        validate_run(&output, 0, &expected).unwrap();
    }
}

#[test]
#[ignore = "requires ca65, ld65, and sim65; run with --ignored"]
fn truncated_source_frame_fails_without_reading_past_eof() {
    let output = build_and_run("frontend_framing", &[3, 0, b'A', b'B']).unwrap();
    validate_run(&output, 1, b"AB").unwrap();
}

#[test]
#[ignore = "requires ca65, ld65, and sim65; run with --ignored"]
fn infinite_loop_is_bounded_by_cycle_limit() {
    let output = build_and_run("infinite_loop", &[]).unwrap();
    assert!(
        !output.status.success(),
        "sim65 execute: infinite loop unexpectedly succeeded"
    );
}
