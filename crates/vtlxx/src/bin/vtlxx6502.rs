use std::ffi::{OsStr, OsString};
use std::fs;
#[cfg(unix)]
use std::io::Read;
use std::io::{self, Write};
#[cfg(unix)]
use std::os::fd::{AsRawFd, FromRawFd};
use std::path::{Path, PathBuf};
use std::process::{Command, Output, Stdio};
use std::sync::atomic::{AtomicU64, Ordering};
#[cfg(unix)]
use std::thread;
use std::time::{SystemTime, UNIX_EPOCH};

mod vtlxx6502_host;
#[cfg(target_os = "linux")]
mod vtlxx6502_tty;

const DEFAULT_CYCLE_LIMIT: u32 = 4_000_000;
const MAX_SOURCE_LEN: usize = u16::MAX as usize;
static NEXT_TEMP_ID: AtomicU64 = AtomicU64::new(0);

const USAGE: &str = "Usage: vtlxx6502 <source.vtl> [--runtime-input <path> | --interactive] [--cycle-limit <cycles>] [--seed <signed16>] [--engine sim65|mos6502] [--raw-tty]\n\
Compile VTLXX source with the NMOS 6502 target and run it on Linux.\n\
Options:\n\
  --engine <engine>       Select mos6502 (default, Linux only) or sim65\n\
  --raw-tty               Enable raw terminal input (mos6502 interactive only)\n\
  --runtime-input <path>  Supply raw runtime input bytes\n\
  --interactive           Connect runtime input and output to standard I/O\n\
  --cycle-limit <cycles>  Set guest cycle limit (default: 4000000; unlimited in interactive mode)\n\
  --seed <signed16>       Set the initial PRNG seed (default: OS entropy)\n\
  -h, --help              Show this help\n";

struct TempDir(PathBuf);

impl TempDir {
    fn new() -> Result<Self, String> {
        let nonce = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map_err(|error| format!("temporary directory clock: {error}"))?
            .as_nanos();
        let path = std::env::temp_dir().join(format!(
            "vtlxx-vtlxx6502-{}-{nonce}-{}",
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

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Engine {
    Sim65,
    Mos6502,
}

#[derive(Debug)]
struct Arguments {
    source: PathBuf,
    runtime_input: Option<PathBuf>,
    cycle_limit: Option<u32>,
    seed: Option<i16>,
    interactive: bool,
    engine: Engine,
    raw_tty: bool,
}

#[derive(Debug)]
enum ParsedArguments {
    Help,
    Run(Arguments),
}

fn parse_arguments(args: impl IntoIterator<Item = OsString>) -> Result<ParsedArguments, String> {
    let mut args = args.into_iter();
    let mut source = None;
    let mut runtime_input = None;
    let mut cycle_limit = None;
    let mut interactive = false;
    let mut cycle_limit_seen = false;
    let mut seed = None;
    let mut seed_seen = false;
    let mut engine = None;
    let mut raw_tty = false;
    while let Some(arg) = args.next() {
        if arg == "-h" || arg == "--help" {
            return Ok(ParsedArguments::Help);
        }
        if arg == "--engine" {
            if engine.is_some() {
                return Err("--engine may be specified only once".into());
            }
            let value = args
                .next()
                .ok_or_else(|| "--engine requires sim65 or mos6502".to_string())?;
            engine = Some(if value == "sim65" {
                Engine::Sim65
            } else if value == "mos6502" {
                Engine::Mos6502
            } else {
                return Err("--engine requires sim65 or mos6502".into());
            });
        } else if arg == "--raw-tty" {
            if raw_tty {
                return Err("--raw-tty may be specified only once".into());
            }
            raw_tty = true;
        } else if arg == "--interactive" {
            if interactive {
                return Err("--interactive may be specified only once".into());
            }
            interactive = true;
        } else if arg == "--runtime-input" {
            if runtime_input.is_some() {
                return Err("--runtime-input may be specified only once".into());
            }
            runtime_input =
                Some(PathBuf::from(args.next().ok_or_else(|| {
                    "--runtime-input requires a path".to_string()
                })?));
        } else if arg == "--cycle-limit" {
            if cycle_limit_seen {
                return Err("--cycle-limit may be specified only once".into());
            }
            cycle_limit_seen = true;
            let value = args
                .next()
                .ok_or_else(|| "--cycle-limit requires a positive decimal integer".to_string())?;
            let value = value
                .to_str()
                .ok_or_else(|| "--cycle-limit requires a positive decimal integer".to_string())?;
            if value.is_empty() || !value.bytes().all(|byte| byte.is_ascii_digit()) {
                return Err("--cycle-limit requires a positive decimal integer".into());
            }
            let parsed_limit = value
                .parse::<u32>()
                .map_err(|_| "--cycle-limit is outside the supported range".to_string())?;
            if parsed_limit == 0 {
                return Err("--cycle-limit must be greater than zero".into());
            }
            cycle_limit = Some(parsed_limit);
        } else if arg == "--seed" {
            if seed_seen {
                return Err("--seed may be specified only once".into());
            }
            seed_seen = true;
            let value = args
                .next()
                .ok_or_else(|| "--seed requires a signed 16-bit decimal integer".to_string())?;
            let value = value
                .to_str()
                .ok_or_else(|| "--seed requires a signed 16-bit decimal integer".to_string())?;
            let digits = value
                .strip_prefix('-')
                .or_else(|| value.strip_prefix('+'))
                .unwrap_or(value);
            if digits.is_empty() || !digits.bytes().all(|byte| byte.is_ascii_digit()) {
                return Err("--seed requires a signed 16-bit decimal integer".into());
            }
            seed = Some(
                value
                    .parse::<i16>()
                    .map_err(|_| "--seed is outside the signed 16-bit range".to_string())?,
            );
        } else if arg.to_string_lossy().starts_with('-') {
            return Err(format!("unknown option: {}", arg.to_string_lossy()));
        } else if source.replace(PathBuf::from(arg)).is_some() {
            return Err("exactly one source file is supported".into());
        }
    }
    if interactive && runtime_input.is_some() {
        return Err("--interactive and --runtime-input are mutually exclusive".into());
    }
    let engine = engine.unwrap_or(Engine::Mos6502);
    if raw_tty && (engine != Engine::Mos6502 || !interactive) {
        return Err("--raw-tty requires the mos6502 engine and --interactive".into());
    }
    #[cfg(not(target_os = "linux"))]
    if engine == Engine::Mos6502 {
        return Err("mos6502 engine is supported only on Linux".into());
    }
    #[cfg(target_os = "linux")]
    if raw_tty && unsafe { libc::isatty(libc::STDIN_FILENO) } != 1 {
        return Err("--raw-tty requires a TTY on stdin".into());
    }
    if !interactive && cycle_limit.is_none() {
        cycle_limit = Some(DEFAULT_CYCLE_LIMIT);
    }
    let source = source.ok_or_else(|| "a source file path is required".to_string())?;
    Ok(ParsedArguments::Run(Arguments {
        source,
        runtime_input,
        cycle_limit,
        seed,
        interactive,
        engine,
        raw_tty,
    }))
}

fn frame_input(source: &[u8], runtime_input: &[u8]) -> Result<Vec<u8>, String> {
    let length = u16::try_from(source.len()).map_err(|_| {
        format!(
            "source is {} bytes; maximum supported length is {MAX_SOURCE_LEN} bytes",
            source.len()
        )
    })?;
    let mut framed = Vec::with_capacity(2 + source.len() + runtime_input.len());
    framed.extend_from_slice(&length.to_le_bytes());
    framed.extend_from_slice(source);
    framed.extend_from_slice(runtime_input);
    Ok(framed)
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
        "{stage}: {tool} exited with {}\n{}",
        output.status,
        String::from_utf8_lossy(&output.stderr)
    ))
}

// Once the child exits, wake the input worker and wait for its final result.
// Readable input is handled before cancellation, so an already observed input
// failure cannot be lost when the child and input become ready together.
#[cfg(unix)]
fn transfer_input<R: Read + AsRawFd, W: Write>(
    mut input: R,
    mut output: W,
    frame: &[u8],
    cancel: io::PipeReader,
) -> Result<(), String> {
    fn write_bytes(output: &mut impl Write, bytes: &[u8]) -> Result<bool, String> {
        match output.write_all(bytes) {
            Ok(()) => Ok(true),
            Err(error) if error.kind() == io::ErrorKind::BrokenPipe => Ok(false),
            Err(error) => Err(format!("write sim65 stdin: {error}")),
        }
    }
    if !write_bytes(&mut output, frame)? {
        return Ok(());
    }
    let mut buffer = [0; 4096];
    loop {
        let mut fds = [
            libc::pollfd {
                fd: input.as_raw_fd(),
                events: libc::POLLIN,
                revents: 0,
            },
            libc::pollfd {
                fd: cancel.as_raw_fd(),
                events: libc::POLLIN,
                revents: 0,
            },
        ];
        // The descriptors remain owned and open for the duration of poll.
        let ready = unsafe { libc::poll(fds.as_mut_ptr(), fds.len() as libc::nfds_t, -1) };
        if ready < 0 {
            let error = io::Error::last_os_error();
            if error.kind() == io::ErrorKind::Interrupted {
                continue;
            }
            return Err(format!("poll stdin: {error}"));
        }
        if fds[0].revents != 0 {
            let count = input
                .read(&mut buffer)
                .map_err(|error| format!("read stdin: {error}"))?;
            if count == 0 || !write_bytes(&mut output, &buffer[..count])? {
                return Ok(());
            }
        }
        if fds[1].revents != 0 {
            return Ok(());
        }
    }
}

fn build_image(entropy_seed: Option<u16>, engine: Engine) -> Result<(TempDir, PathBuf), String> {
    let crate_root = Path::new(env!("CARGO_MANIFEST_DIR"));
    let sources = [
        "6502/target/sim65_cli.s",
        "6502/frontend/basic.s",
        "6502/frontend/source.s",
        "6502/compiler/arena.s",
        "6502/compiler/emitter.s",
        "6502/compiler/owner.s",
        "6502/compiler/binding.s",
        "6502/target/zp_layout.s",
        "6502/runtime/state.s",
        "6502/runtime/primitives.s",
        "6502/runtime/arithmetic.s",
        if engine == Engine::Sim65 {
            "6502/target/sim65_adapter.s"
        } else {
            "6502/target/mos6502_adapter.s"
        },
    ];
    let temp = TempDir::new()?;
    let mut objects = Vec::with_capacity(sources.len());
    for (index, source) in sources.iter().enumerate() {
        let source = crate_root.join(source);
        let object = temp.0.join(format!("{index}.o"));
        let mut assemble_args = vec![OsStr::new("-t"), OsStr::new("sim6502")];
        let entropy_define =
            entropy_seed.map(|seed| OsString::from(format!("SIM65_ENTROPY_SEED=${seed:04X}")));
        if source.ends_with("6502/target/sim65_adapter.s")
            || source.ends_with("6502/target/mos6502_adapter.s")
        {
            if let Some(define) = &entropy_define {
                assemble_args.extend([OsStr::new("-D"), define.as_os_str()]);
            } else {
                assemble_args.extend([OsStr::new("-D"), OsStr::new("SIM65_ENTROPY_UNAVAILABLE")]);
            }
        }
        assemble_args.extend([source.as_os_str(), OsStr::new("-o"), object.as_os_str()]);
        let output = run_tool("ca65", "assemble", &assemble_args, &[])?;
        require_success("assemble", "ca65", &output)?;
        objects.push(object);
    }

    let executable = temp.0.join("vtlxx6502.prg");
    let mut link_args = vec![
        OsStr::new("-t"),
        OsStr::new("sim6502"),
        OsStr::new("-o"),
        executable.as_os_str(),
    ];
    link_args.extend(objects.iter().map(|object| object.as_os_str()));
    link_args.push(OsStr::new("sim6502.lib"));
    let output = run_tool("ld65", "link", &link_args, &[])?;
    require_success("link", "ld65", &output)?;
    Ok((temp, executable))
}

fn build_and_run(
    input: &[u8],
    cycle_limit: Option<u32>,
    interactive: bool,
    entropy_seed: Option<u16>,
    engine: Engine,
    raw_tty: bool,
) -> Result<Output, String> {
    let (_temp, executable) = build_image(entropy_seed, engine)?;
    #[cfg(target_os = "linux")]
    if engine == Engine::Mos6502 {
        return execute_mos(&executable, input, cycle_limit, interactive, raw_tty);
    }
    #[cfg(not(target_os = "linux"))]
    let _ = raw_tty;
    let mut command = Command::new("sim65");
    if let Some(limit) = cycle_limit {
        command.arg("-x").arg(limit.to_string());
    }
    command.arg(&executable);
    if !interactive {
        let mut args = Vec::new();
        let limit = cycle_limit.map(|n| n.to_string());
        if let Some(ref limit) = limit {
            args.extend([OsStr::new("-x"), OsStr::new(limit)]);
        }
        args.push(executable.as_os_str());
        return run_tool("sim65", "execute", &args, input);
    }
    #[cfg(not(unix))]
    return Err("interactive mode requires a Unix host".into());
    #[cfg(unix)]
    {
        let (cancel, mut wake) =
            io::pipe().map_err(|error| format!("execute: create input cancellation: {error}"))?;
        let mut child = command
            .stdin(Stdio::piped())
            .stdout(Stdio::inherit())
            .stderr(Stdio::piped())
            .spawn()
            .map_err(|error| format!("execute: could not start sim65: {error}"))?;
        let child_stdin = child.stdin.take().expect("piped stdin");
        let frame = input.to_vec();
        let worker = thread::spawn(move || {
            // Poll and read the same unbuffered descriptor.
            let fd = unsafe { libc::dup(libc::STDIN_FILENO) };
            if fd < 0 {
                return Err(format!("read stdin: {}", io::Error::last_os_error()));
            }
            // dup returned a new owned descriptor.
            let input = unsafe { fs::File::from_raw_fd(fd) };
            transfer_input(input, child_stdin, &frame, cancel)
        });
        let output = child.wait_with_output();
        // Closing the wake endpoint also wakes poll if the write fails.
        let _ = wake.write_all(&[1]);
        drop(wake);
        let transfer = worker
            .join()
            .map_err(|_| "execute: stdin worker panicked".to_string())?;
        let output =
            output.map_err(|error| format!("execute: could not collect sim65 output: {error}"))?;
        if let Err(error) = transfer {
            let detail = if output.status.success() {
                String::from_utf8_lossy(&output.stderr).trim().to_string()
            } else {
                report_simulation_failure(&output).1
            };
            return Err(if detail.is_empty() {
                format!("execute: {error}")
            } else {
                format!("execute: {error}; {detail}")
            });
        }
        Ok(output)
    }
}

#[cfg(target_os = "linux")]
fn execute_mos(
    executable: &Path,
    input: &[u8],
    cycle_limit: Option<u32>,
    interactive: bool,
    raw_tty: bool,
) -> Result<Output, String> {
    use std::os::unix::process::ExitStatusExt;
    use vtlxx6502_host::{HostOutcome, HostRunner};
    let image = fs::read(executable).map_err(|error| format!("read image: {error}"))?;
    let limit = cycle_limit.map(u64::from);
    let mut runner = if interactive {
        let fd = unsafe { libc::dup(libc::STDIN_FILENO) };
        if fd < 0 {
            return Err(format!("duplicate stdin: {}", io::Error::last_os_error()));
        }
        let fd = unsafe { std::os::fd::OwnedFd::from_raw_fd(fd) };
        HostRunner::interactive(&image, input.to_vec(), fd, limit)
    } else {
        HostRunner::new(&image, input.to_vec(), limit)
    }
    .map_err(|error| format!("mos6502 host: {error}"))?;
    let mut stdout = Vec::new();
    let mut stderr = Vec::new();
    let outcome = if interactive {
        let mut out = io::stdout().lock();
        let mut err = io::stderr().lock();
        if raw_tty {
            let mut tty = vtlxx6502_tty::RawTty::new(libc::STDIN_FILENO)
                .map_err(|error| format!("raw tty: {error}"))?;
            let signal_fd = tty.signal_fd();
            let result = runner.run_to_with_signals(&mut out, &mut err, signal_fd, &mut |woken| {
                if woken || tty.pending() {
                    tty.process_signals()
                } else {
                    Ok(None)
                }
            });
            tty.restore()
                .map_err(|error| format!("restore tty: {error}"))?;
            result
        } else {
            runner.run_to(&mut out, &mut err)
        }
    } else {
        runner.run_to(&mut stdout, &mut stderr)
    }
    .map_err(|error| format!("mos6502 host: {error}"))?;
    if !interactive && !stderr.is_empty() {
        io::stderr()
            .write_all(&stderr)
            .and_then(|()| io::stderr().flush())
            .map_err(|error| format!("mos6502 host: write stderr: {error}"))?;
        stderr.clear();
    }
    let status = match outcome {
        HostOutcome::GuestExit { status, .. } => i32::from(status),
        HostOutcome::CycleLimit { .. } => {
            stderr.extend_from_slice(b"mos6502 cycle limit exceeded");
            126
        }
        HostOutcome::Interrupted { signal } => {
            stderr.extend_from_slice(format!("interrupted by signal {signal}").as_bytes());
            128 + signal
        }
    };
    Ok(Output {
        status: std::process::ExitStatus::from_raw(status << 8),
        stdout,
        stderr,
    })
}

fn os_entropy16() -> io::Result<u16> {
    let mut bytes = [0; 2];
    getrandom::fill(&mut bytes).map_err(|error| io::Error::other(error.to_string()))?;
    Ok(u16::from_le_bytes(bytes))
}

fn execute(arguments: Arguments) -> Result<Output, String> {
    execute_with_provider(arguments, os_entropy16)
}

fn select_entropy_seed(
    seed: Option<i16>,
    provider: impl FnOnce() -> io::Result<u16>,
) -> Option<u16> {
    seed.map(|seed| seed as u16).or_else(|| provider().ok())
}

fn execute_with_provider(
    arguments: Arguments,
    provider: impl FnOnce() -> io::Result<u16>,
) -> Result<Output, String> {
    // Explicit seeds bypass the host entropy provider entirely. For OS entropy
    // failure, link an unavailable adapter so unused `'` programs still run.
    let entropy_seed = select_entropy_seed(arguments.seed, provider);
    let source = fs::read(&arguments.source)
        .map_err(|error| format!("read source {}: {error}", arguments.source.display()))?;
    let runtime_input = match arguments.runtime_input {
        Some(path) => fs::read(&path)
            .map_err(|error| format!("read runtime input {}: {error}", path.display()))?,
        None => Vec::new(),
    };
    let input = frame_input(&source, &runtime_input)?;
    build_and_run(
        &input,
        arguments.cycle_limit,
        arguments.interactive,
        entropy_seed,
        arguments.engine,
        arguments.raw_tty,
    )
}

fn report_simulation_failure(output: &Output) -> (i32, String) {
    let stderr = String::from_utf8_lossy(&output.stderr);
    let lower = stderr.to_ascii_lowercase();
    let code = output.status.code().unwrap_or(1);
    let reason = if lower.contains("cycle") || lower.contains("timeout") {
        "sim65 cycle limit exceeded".to_string()
    } else if code & 0x80 != 0 {
        let frontend_status = code & 0x7f;
        let cause = match frontend_status {
            1 => "source or transport error",
            2 => "syntax error",
            3 => "literal range error",
            4 => "compiler resource error",
            _ => "unknown frontend error",
        };
        format!("6502 compilation failed ({cause}, frontend status {frontend_status})")
    } else {
        format!("6502 target halted with status {code}")
    };
    let detail = if stderr.trim().is_empty() {
        format!("{reason} (sim65 exit status {})", output.status)
    } else {
        format!(
            "{reason} (sim65 exit status {}): {}",
            output.status,
            stderr.trim()
        )
    };
    (code, detail)
}

fn main() {
    let parsed = match parse_arguments(std::env::args_os().skip(1)) {
        Ok(parsed) => parsed,
        Err(error) => {
            eprintln!("vtlxx6502: {error}\n{USAGE}");
            std::process::exit(2);
        }
    };
    match parsed {
        ParsedArguments::Help => print!("{USAGE}"),
        ParsedArguments::Run(arguments) => {
            let engine = arguments.engine;
            match execute(arguments) {
                Ok(output) => {
                    if let Err(error) = io::stdout().write_all(&output.stdout) {
                        eprintln!("vtlxx6502: write stdout: {error}");
                        std::process::exit(2);
                    }
                    if !output.status.success() {
                        let (status, message) = if engine == Engine::Sim65 {
                            report_simulation_failure(&output)
                        } else {
                            let status = output.status.code().unwrap_or(1);
                            let detail = String::from_utf8_lossy(&output.stderr);
                            (
                                status,
                                if detail.is_empty() && (129..=132).contains(&status) {
                                    format!(
                                        "6502 compilation failed (frontend status {})",
                                        status & 0x7f
                                    )
                                } else if detail.is_empty() {
                                    format!("mos6502 guest exited with status {status}")
                                } else {
                                    format!(
                                        "mos6502 guest exited with status {status}: {}",
                                        detail.trim()
                                    )
                                },
                            )
                        };
                        eprintln!("vtlxx6502: {message}");
                        std::process::exit(status);
                    }
                    if !output.stderr.is_empty() {
                        let _ = io::stderr().write_all(&output.stderr);
                    }
                }
                Err(error) => {
                    eprintln!("vtlxx6502: {error}");
                    std::process::exit(2);
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[cfg(unix)]
    #[test]
    fn child_exit_waits_for_an_in_flight_input_error() {
        use std::sync::mpsc;

        struct FailingInput {
            fd: io::PipeReader,
            entered: mpsc::Sender<()>,
            release: mpsc::Receiver<()>,
        }
        impl AsRawFd for FailingInput {
            fn as_raw_fd(&self) -> std::os::fd::RawFd {
                self.fd.as_raw_fd()
            }
        }
        impl Read for FailingInput {
            fn read(&mut self, _: &mut [u8]) -> io::Result<usize> {
                self.entered.send(()).unwrap();
                self.release.recv().unwrap();
                Err(io::Error::other("injected input failure"))
            }
        }
        let (input, mut ready) = io::pipe().unwrap();
        let (cancel, mut wake) = io::pipe().unwrap();
        let (entered_tx, entered_rx) = mpsc::channel();
        let (release_tx, release_rx) = mpsc::channel();
        let worker = thread::spawn(move || {
            transfer_input(
                FailingInput {
                    fd: input,
                    entered: entered_tx,
                    release: release_rx,
                },
                Vec::new(),
                b"frame",
                cancel,
            )
        });
        ready.write_all(b"x").unwrap();
        entered_rx.recv().unwrap();
        // Child exit is observed before the input failure is delivered.
        wake.write_all(b"x").unwrap();
        assert!(!worker.is_finished());
        release_tx.send(()).unwrap();
        assert!(
            worker
                .join()
                .unwrap()
                .unwrap_err()
                .contains("injected input failure")
        );
    }

    #[test]
    fn framing_preserves_source_and_binary_runtime_input() {
        assert_eq!(
            frame_input(b"A=1\n", &[0, 0xff]).unwrap(),
            [4, 0, b'A', b'=', b'1', b'\n', 0, 0xff]
        );
    }

    #[test]
    fn framing_accepts_u16_max_and_rejects_longer_source() {
        let max_source = vec![b' '; MAX_SOURCE_LEN];
        assert_eq!(
            frame_input(&max_source, &[]).unwrap().len(),
            MAX_SOURCE_LEN + 2
        );
        let oversized_source = vec![b' '; MAX_SOURCE_LEN + 1];
        assert!(frame_input(&oversized_source, &[]).is_err());
    }

    #[test]
    fn argument_errors_are_classified() {
        let parse = |args: Vec<OsString>| {
            parse_arguments(
                [OsString::from("--engine"), OsString::from("sim65")]
                    .into_iter()
                    .chain(args),
            )
        };
        assert!(
            parse(Vec::<OsString>::new())
                .unwrap_err()
                .contains("source file")
        );
        assert!(
            parse(vec!["source.vtl".into(), "--wat".into()])
                .unwrap_err()
                .contains("unknown option")
        );
        assert!(
            parse(vec!["one.vtl".into(), "two.vtl".into()])
                .unwrap_err()
                .contains("exactly one")
        );
        assert!(
            parse(vec!["source.vtl".into(), "--runtime-input".into()])
                .unwrap_err()
                .contains("requires a path")
        );
        for value in ["", "+", "1x", "32768", "-32769"] {
            assert!(
                parse(vec!["source.vtl".into(), "--seed".into(), value.into()]).is_err(),
                "accepted invalid seed {value:?}"
            );
        }
        for seed in ["0", "1", "+1", "-1", "-32768", "32767"] {
            assert!(matches!(
                parse(vec!["source.vtl".into(), "--seed".into(), seed.into()]),
                Ok(ParsedArguments::Run(Arguments { seed: Some(_), .. }))
            ));
        }
        assert!(parse(vec!["source.vtl".into(), "--seed".into()]).is_err());
        assert!(
            parse(vec![
                "source.vtl".into(),
                "--seed".into(),
                "1".into(),
                "--seed".into(),
                "2".into()
            ])
            .is_err()
        );
        match parse(vec![
            "source.vtl".into(),
            "--seed".into(),
            "-32768".into(),
            "--interactive".into(),
            "--cycle-limit".into(),
            "12".into(),
        ])
        .unwrap()
        {
            ParsedArguments::Run(arguments) => {
                assert_eq!(arguments.seed, Some(i16::MIN));
                assert!(arguments.interactive);
                assert_eq!(arguments.cycle_limit, Some(12));
            }
            ParsedArguments::Help => panic!("expected run arguments"),
        }
        for value in ["0", "-1", "", "12x", "4294967296"] {
            assert!(
                parse(vec![
                    "source.vtl".into(),
                    "--cycle-limit".into(),
                    value.into()
                ])
                .is_err(),
                "accepted invalid cycle limit {value:?}"
            );
        }
        assert!(
            parse(vec!["source.vtl".into(), "--cycle-limit".into()])
                .unwrap_err()
                .contains("requires")
        );
        assert!(
            parse(vec![
                "source.vtl".into(),
                "--cycle-limit".into(),
                "100".into(),
                "--cycle-limit".into(),
                "200".into(),
            ])
            .unwrap_err()
            .contains("only once")
        );
        match parse(vec![
            "source.vtl".into(),
            "--cycle-limit".into(),
            "256000000".into(),
            "--runtime-input".into(),
            "input.bin".into(),
        ])
        .unwrap()
        {
            ParsedArguments::Run(arguments) => {
                assert_eq!(arguments.cycle_limit, Some(256_000_000));
                assert_eq!(arguments.runtime_input, Some(PathBuf::from("input.bin")));
            }
            ParsedArguments::Help => panic!("expected run arguments"),
        }
        match parse(vec!["source.vtl".into()]).unwrap() {
            ParsedArguments::Run(arguments) => {
                assert_eq!(arguments.cycle_limit, Some(DEFAULT_CYCLE_LIMIT));
                assert_eq!(arguments.engine, Engine::Sim65);
            }
            ParsedArguments::Help => panic!("expected run arguments"),
        }
        match parse(vec!["source.vtl".into(), "--interactive".into()]).unwrap() {
            ParsedArguments::Run(arguments) => {
                assert!(arguments.interactive);
                assert_eq!(arguments.cycle_limit, None);
            }
            ParsedArguments::Help => panic!("expected run arguments"),
        }
        assert!(
            parse(vec![
                "source.vtl".into(),
                "--interactive".into(),
                "--runtime-input".into(),
                "input".into()
            ])
            .is_err()
        );
        assert!(
            parse(vec![
                "source.vtl".into(),
                "--interactive".into(),
                "--interactive".into()
            ])
            .is_err()
        );
    }

    #[test]
    #[cfg(target_os = "linux")]
    fn default_engine_is_mos6502() {
        match parse_arguments(["source.vtl".into()]).unwrap() {
            ParsedArguments::Run(arguments) => assert_eq!(arguments.engine, Engine::Mos6502),
            ParsedArguments::Help => panic!("expected run arguments"),
        }
    }

    #[test]
    #[cfg(not(target_os = "linux"))]
    fn mos_engines_are_rejected_before_source_access() {
        for args in [
            vec!["missing.vtl"],
            vec!["missing.vtl", "--engine", "mos6502"],
        ] {
            let error = parse_arguments(args.into_iter().map(OsString::from)).unwrap_err();
            assert!(error.contains("supported only on Linux"));
        }
        assert!(parse_arguments(["missing.vtl", "--engine", "sim65"].map(OsString::from)).is_ok());
    }

    #[test]
    fn explicit_seed_bypasses_entropy_provider() {
        let mut called = false;
        assert_eq!(
            select_entropy_seed(Some(-1), || {
                called = true;
                Err(io::Error::other("must not be called"))
            }),
            Some(u16::MAX)
        );
        assert!(!called);
        assert_eq!(select_entropy_seed(None, || Ok(0x80ff)), Some(0x80ff));
        assert_eq!(
            select_entropy_seed(None, || Err(io::Error::other("no entropy"))),
            None
        );
    }

    #[test]
    #[ignore = "requires ca65, ld65, and sim65"]
    fn failed_os_entropy_only_fails_when_bang_executes() {
        let temp = TempDir::new().unwrap();
        let run = |name: &str, source: &[u8]| {
            let path = temp.0.join(name);
            fs::write(&path, source).unwrap();
            execute_with_provider(
                Arguments {
                    source: path,
                    runtime_input: None,
                    cycle_limit: Some(200_000_000),
                    seed: None,
                    interactive: false,
                    engine: Engine::Sim65,
                    raw_tty: false,
                },
                || Err(io::Error::other("injected entropy failure")),
            )
            .unwrap()
        };
        assert!(run("unused.vtl", b"A=1").status.success());
        assert_eq!(run("used.vtl", b"A=! ?=A").status.code(), Some(5));
        assert_eq!(run("compile-error.vtl", b"A=").status.code(), Some(130));
    }

    #[test]
    fn missing_tool_error_includes_build_stage() {
        let error = run_tool("vtlxx-no-such-tool", "assemble", &[], &[]).unwrap_err();
        assert!(error.contains("assemble"));
        assert!(error.contains("vtlxx-no-such-tool"));
    }
}
