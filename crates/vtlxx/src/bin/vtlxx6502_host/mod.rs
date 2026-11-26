#![allow(dead_code)] // CLI selection and live host streams are the responsibility of #600.

//! Private runner for fixed-address VTLXX raw images.
pub(crate) mod image;
mod input;
mod services;

use mos6502::{cpu::CPU, instruction::Nmos6502, memory::Memory};
use std::io::Write;

#[derive(Debug, PartialEq, Eq)]
pub(crate) enum HostOutcome {
    GuestExit { status: u8, cycles: u64 },
    CycleLimit { cycles: u64 },
    Interrupted { signal: i32 },
}

#[derive(Debug)]
pub(crate) enum HostError {
    Image(String),
    Service(String),
    Input(std::io::Error),
    Output(std::io::Error),
    Signal(std::io::Error),
}

impl std::fmt::Display for HostError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Image(message) | Self::Service(message) => f.write_str(message),
            Self::Input(error) => write!(f, "guest input: {error}"),
            Self::Output(error) => write!(f, "guest output: {error}"),
            Self::Signal(error) => write!(f, "signal handling: {error}"),
        }
    }
}

pub(crate) struct HostRunner {
    cpu: CPU<Memory, Nmos6502>,
    input: input::HostInput,
    cycle_limit: Option<u64>,
}

impl HostRunner {
    pub(crate) fn new(
        bytes: &[u8],
        input: Vec<u8>,
        cycle_limit: Option<u64>,
    ) -> Result<Self, HostError> {
        Self::with_input(bytes, input::HostInput::memory(input), cycle_limit)
    }

    #[cfg(target_os = "linux")]
    pub(crate) fn interactive(
        bytes: &[u8],
        prefix: Vec<u8>,
        fd: std::os::fd::OwnedFd,
        cycle_limit: Option<u64>,
    ) -> Result<Self, HostError> {
        Self::with_input(
            bytes,
            input::HostInput::interactive(prefix, fd),
            cycle_limit,
        )
    }

    fn with_input(
        bytes: &[u8],
        input: input::HostInput,
        cycle_limit: Option<u64>,
    ) -> Result<Self, HostError> {
        let memory = image::load(bytes)?;
        let mut cpu = CPU::new(memory, Nmos6502);
        cpu.registers.program_counter = image::LOAD;
        cpu.registers.stack_pointer.0 = 0xff;
        Ok(Self {
            cpu,
            input,
            cycle_limit,
        })
    }

    pub(crate) fn run(mut self) -> Result<(HostOutcome, Vec<u8>), HostError> {
        let mut stdout = Vec::new();
        let outcome = self.run_to(&mut stdout)?;
        Ok((outcome, stdout))
    }

    pub(crate) fn run_to(&mut self, stdout: &mut dyn Write) -> Result<HostOutcome, HostError> {
        self.run_with_callback(stdout, &mut |_| Ok(None))
    }

    #[cfg(target_os = "linux")]
    pub(crate) fn run_to_with_signals(
        &mut self,
        stdout: &mut dyn Write,
        signal_fd: std::os::fd::RawFd,
        mut callback: impl FnMut(bool) -> std::io::Result<Option<i32>>,
    ) -> Result<HostOutcome, HostError> {
        self.input
            .set_signal_fd(signal_fd)
            .map_err(HostError::Signal)?;
        self.run_with_callback(stdout, &mut callback)
    }

    fn run_with_callback(
        &mut self,
        stdout: &mut dyn Write,
        callback: &mut dyn FnMut(bool) -> std::io::Result<Option<i32>>,
    ) -> Result<HostOutcome, HostError> {
        let result = self.run_loop_with_callback(stdout, callback);
        let finished = self.input.finish().map_err(HostError::Input);
        match result {
            Ok(outcome @ HostOutcome::GuestExit { status: 6, .. }) => Ok(outcome),
            Ok(outcome) => {
                finished?;
                Ok(outcome)
            }
            Err(error) => Err(error),
        }
    }

    fn run_loop(&mut self, stdout: &mut dyn Write) -> Result<HostOutcome, HostError> {
        self.run_loop_with_callback(stdout, &mut |_| Ok(None))
    }

    fn run_loop_with_callback(
        &mut self,
        stdout: &mut dyn Write,
        callback: &mut dyn FnMut(bool) -> std::io::Result<Option<i32>>,
    ) -> Result<HostOutcome, HostError> {
        let mut woken = false;
        loop {
            if let Some(signal) = callback(woken).map_err(HostError::Signal)? {
                return Ok(HostOutcome::Interrupted { signal });
            }
            woken = false;
            if self
                .cycle_limit
                .is_some_and(|limit| self.cpu.cycles >= limit)
            {
                return Ok(HostOutcome::CycleLimit {
                    cycles: self.cpu.cycles,
                });
            }
            let pc = self.cpu.registers.program_counter;
            if pc == image::SERVICE_START || pc == image::SERVICE_START + 1 {
                woken = !services::input_service(&mut self.cpu, pc, &mut self.input);
            } else if pc >= 0xfff1 {
                let service = services::dispatch(&mut self.cpu, pc, stdout);
                match service? {
                    services::ServiceResult::Continue => continue,
                    services::ServiceResult::Exit(status) => {
                        return Ok(HostOutcome::GuestExit {
                            status,
                            cycles: self.cpu.cycles,
                        });
                    }
                }
            } else {
                self.cpu.single_step();
            }
        }
    }
}

pub(crate) fn run_with_io(
    image: &[u8],
    input: &[u8],
    cycle_limit: Option<u64>,
    writer: &mut impl Write,
) -> Result<HostOutcome, HostError> {
    HostRunner::new(image, input.to_vec(), cycle_limit)?.run_to(writer)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::{
        ffi::OsStr,
        path::PathBuf,
        process::{Command, Stdio},
        sync::atomic::{AtomicU64, Ordering},
    };

    static NEXT: AtomicU64 = AtomicU64::new(0);

    struct Temp(PathBuf);
    impl Temp {
        fn new() -> Self {
            let path = std::env::temp_dir().join(format!(
                "vtlxx-mos-host-{}-{}",
                std::process::id(),
                NEXT.fetch_add(1, Ordering::Relaxed)
            ));
            std::fs::create_dir(&path).unwrap();
            Self(path)
        }
    }
    impl Drop for Temp {
        fn drop(&mut self) {
            let _ = std::fs::remove_dir_all(&self.0);
        }
    }

    fn compile_image(mos_adapter: bool) -> (Temp, PathBuf) {
        compile_image_with_guard(mos_adapter, None)
    }

    fn compile_image_with_guard(mos_adapter: bool, force_guard: Option<u8>) -> (Temp, PathBuf) {
        let temp = Temp::new();
        let root = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
        let sources = [
            "6502/target/sim65_cli.s",
            "6502/frontend/basic.s",
            "6502/frontend/source.s",
            "6502/compiler/session.s",
            "6502/compiler/arena.s",
            "6502/compiler/emitter.s",
            "6502/compiler/owner.s",
            "6502/compiler/binding.s",
            "6502/target/zp_layout.s",
            "6502/runtime/state.s",
            "6502/runtime/primitives.s",
            "6502/runtime/arithmetic.s",
            if mos_adapter {
                "6502/target/mos6502_adapter.s"
            } else {
                "6502/target/sim65_adapter.s"
            },
        ];
        let mut objects = Vec::new();
        for (index, name) in sources.iter().enumerate() {
            let object = temp.0.join(format!("{index}.o"));
            let mut command = Command::new("ca65");
            if mos_adapter {
                command.args([OsStr::new("-D"), OsStr::new("VTLXX_RAW")]);
            } else {
                command.args([OsStr::new("-t"), OsStr::new("sim6502")]);
            }
            if name.ends_with("adapter.s") {
                command.args([OsStr::new("-D"), OsStr::new("SIM65_ENTROPY_UNAVAILABLE")]);
            }
            let source = if let (Some(observed), "6502/frontend/basic.s") = (force_guard, *name) {
                let original = std::fs::read_to_string(root.join(name)).unwrap();
                assert!(original.contains("    cpx #25"));
                let path = temp.0.join("basic.s");
                std::fs::write(
                    &path,
                    original.replace("    cpx #25", &format!("    ldx #{observed}\n    cpx #25")),
                )
                .unwrap();
                path
            } else {
                root.join(name)
            };
            let status = command.arg(source).arg("-o").arg(&object).status().unwrap();
            assert!(status.success(), "ca65 failed for {name}");
            objects.push(object);
        }
        let image = temp.0.join("vtlxx.prg");
        let mut command = Command::new("ld65");
        if mos_adapter {
            command.args(crate::raw_link_args(
                &root.join("6502/target/mos6502.cfg"),
                &image,
                &objects,
            ));
        } else {
            command
                .args([OsStr::new("-t"), OsStr::new("sim6502"), OsStr::new("-o")])
                .arg(&image);
            command.args(&objects).arg("sim6502.lib");
        }
        assert!(command.status().unwrap().success(), "ld65 failed");
        (temp, image)
    }

    fn compile_runtime_probe(fixture: &str, case: u8, wait: bool) -> (Temp, PathBuf) {
        let temp = Temp::new();
        let root = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
        let original =
            std::fs::read_to_string(root.join("tests/fixtures/sim65").join(fixture)).unwrap();
        // The stock fixture checks scripted-adapter call counts. The host test
        // checks input position itself, while preserving its stack assertions.
        let mut source = original
            .replace("script_try_calls, ", "")
            .replace("    lda script_try_calls\n    cmp #1\n    bne @bad\n", "")
            .replace(
                "    lda script_try_calls\n    cmp #1\n    bne @probe_bad\n",
                "",
            )
            .replace("    lda script_try_calls\n    bne @probe_bad\n", "");
        if wait {
            source = source.replace("rt_try_read_char", "rt_read_char");
        }
        source = source.replace(
            ".segment \"CODE\"\n_main:",
            ".import VTLXX_LOAD\n.segment \"ENTRY\"\n_main:\n.assert _main = VTLXX_LOAD, lderror, \"entry must match load\"",
        );
        let fixture_path = temp.0.join("probe.s");
        std::fs::write(&fixture_path, source).unwrap();
        let sources = [
            fixture_path,
            root.join("6502/target/zp_layout.s"),
            root.join("6502/runtime/state.s"),
            root.join("6502/runtime/primitives.s"),
            root.join("6502/runtime/arithmetic.s"),
            root.join("6502/target/mos6502_adapter.s"),
        ];
        let mut objects = Vec::new();
        for (index, source) in sources.iter().enumerate() {
            let object = temp.0.join(format!("probe-{index}.o"));
            let mut command = Command::new("ca65");
            if index == 0 {
                command.args(["-D", &format!("TEST_CASE={case}")]);
            }
            if index == 2 {
                command.args(["-D", "RT_TEST_PROBE"]);
            }
            assert!(
                command
                    .arg(source)
                    .arg("-o")
                    .arg(&object)
                    .status()
                    .unwrap()
                    .success()
            );
            objects.push(object);
        }
        let image = temp.0.join("probe.prg");
        assert!(
            Command::new("ld65")
                .args(crate::raw_link_args(
                    &root.join("6502/target/mos6502.cfg"),
                    &image,
                    &objects,
                ))
                .status()
                .unwrap()
                .success()
        );
        (temp, image)
    }

    #[test]
    #[ignore = "requires ca65 and ld65"]
    fn raw_link_layout_and_rejected_placements() {
        use mos6502::memory::Bus;

        let temp = Temp::new();
        let root = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
        let config = root.join("6502/target/mos6502.cfg");
        let image = temp.0.join("layout.bin");
        let source = temp.0.join("layout.s");
        let object = temp.0.join("layout.o");
        let fixture = |entry_prefix: &str, bss: &str, zp: &str| {
            format!(
                r#"
.import VTLXX_LOAD
.segment "ENTRY"
{entry_prefix}
_main: rts
.assert _main = VTLXX_LOAD, lderror, "entry mismatch"
.segment "CODE"
.byte $11
.segment "RODATA"
.byte $22
.segment "DATA"
.byte $a5, $00, $5a, $ff
.segment "BSS"
.res {bss}
.segment "ZEROPAGE"
.res {zp}
"#
            )
        };
        let assemble = |text: &str| {
            std::fs::write(&source, text).unwrap();
            assert!(
                Command::new("ca65")
                    .arg(&source)
                    .arg("-o")
                    .arg(&object)
                    .status()
                    .unwrap()
                    .success()
            );
        };
        let link = |args: &[std::ffi::OsString]| {
            Command::new("ld65")
                .args(args)
                .output()
                .unwrap()
                .status
                .success()
        };
        assemble(&fixture("", "16", "8"));
        let args = crate::raw_link_args(&config, &image, std::slice::from_ref(&object));
        assert!(link(&args));
        let payload = std::fs::read(&image).unwrap();
        assert_eq!(payload, [0x60, 0x11, 0x22, 0xa5, 0x00, 0x5a, 0xff]);
        let mut loaded = image::load(&payload).unwrap();
        for address in 0..8 {
            assert_eq!(loaded.get_byte(address), 0);
        }
        for address in 0x0207..0x0217 {
            assert_eq!(loaded.get_byte(address), 0);
        }

        // Neither placement symbol has a default.
        for name in ["VTLXX_LOAD=", "VTLXX_SERVICE="] {
            let mut missing = args.clone();
            let index = missing
                .iter()
                .position(|arg| arg.to_string_lossy().starts_with(name))
                .unwrap();
            missing.drain(index - 1..=index);
            assert!(!link(&missing), "missing {name} unexpectedly linked");
        }
        // Shrinking the service limit also counts BSS in MAIN's capacity.
        let mut narrow = args.clone();
        let index = narrow
            .iter()
            .position(|arg| arg.to_string_lossy().starts_with("VTLXX_SERVICE="))
            .unwrap();
        narrow[index] = "VTLXX_SERVICE=0x0207".into();
        assert!(!link(&narrow));

        for (prefix, bss, zp) in [
            (".byte 0", "16", "8"),
            ("", "$ff00", "8"),
            ("", "16", "$101"),
        ] {
            assemble(&fixture(prefix, bss, zp));
            assert!(!link(&args), "invalid placement linked successfully");
        }
    }

    #[test]
    #[ignore = "requires ca65 and ld65"]
    fn mos_try_overflow_preserves_stack_eof_and_input() {
        for (case, wait) in [(16, false), (17, false), (16, true), (17, true)] {
            let (_temp, path) = compile_runtime_probe("runtime_try_capacity.s", case, wait);
            let image = std::fs::read(path).unwrap();
            let mut runner = HostRunner::new(&image, vec![65], Some(2_000_000)).unwrap();
            let mut out = Vec::new();
            let result = runner.run_to(&mut out).unwrap();
            assert!(matches!(result, HostOutcome::GuestExit { status: 2, .. }));
            assert_eq!(out, b"K");
            assert_eq!(runner.input.try_byte(), input::InputEvent::Byte(65));
        }
    }

    #[cfg(target_os = "linux")]
    #[test]
    #[ignore = "requires ca65 and ld65"]
    fn mos_try_fatal_preserves_stack_and_eof() {
        let (_temp, path) = compile_runtime_probe("runtime_try_fatal.s", 1, false);
        let image = std::fs::read(path).unwrap();
        let mut runner = HostRunner::new(&image, vec![], Some(2_000_000)).unwrap();
        // Linux poll/read on a directory produces a real input failure.
        let directory = std::fs::File::open(std::env::temp_dir()).unwrap();
        runner.input = input::HostInput::interactive(vec![], directory.into());
        assert_eq!(runner.input.wait_byte(), input::InputEvent::Fatal);
        let mut out = Vec::new();
        let result = runner.run_loop(&mut out).unwrap();
        assert!(matches!(result, HostOutcome::GuestExit { status: 6, .. }));
        assert_eq!(out, b"K");
        assert!(runner.input.finish().is_err());
    }

    #[cfg(target_os = "linux")]
    #[test]
    #[ignore = "requires ca65 and ld65"]
    fn mos_wait_fatal_stops_source_and_runtime() {
        let (_temp, path) = compile_image(true);
        let image = std::fs::read(path).unwrap();
        for prefix in [
            vec![5, 0, b'?', b'='],
            crate::frame_input(b"A=$ ?=A", b"").unwrap(),
        ] {
            let mut runner = HostRunner::new(&image, vec![], Some(2_000_000)).unwrap();
            let directory = std::fs::File::open(std::env::temp_dir()).unwrap();
            runner.input = input::HostInput::interactive(prefix, directory.into());
            let mut out = Vec::new();
            let result = runner.run_loop(&mut out).unwrap();
            assert!(matches!(result, HostOutcome::GuestExit { status: 6, .. }));
            assert!(out.is_empty());
            assert!(runner.input.finish().is_err());
        }
    }

    #[test]
    #[ignore = "requires ca65, ld65, and sim65"]
    fn mos_adapter_image_runs_source_compile_output_and_exit() {
        let (_temp, path) = compile_image(true);
        let (_stock_temp, stock_path) = compile_image(false);
        let image = std::fs::read(&path).unwrap();
        let input = crate::frame_input(b"?=65", b"").unwrap();
        let mut runner = HostRunner::new(&image, input.clone(), Some(2_000_000)).unwrap();
        let mut stdout = Vec::new();
        let outcome = runner.run_to(&mut stdout).unwrap();
        assert_eq!(runner.cpu.registers.program_counter, 0xfff2);
        assert_eq!(stdout, b"65");
        let HostOutcome::GuestExit {
            status: 0,
            cycles: exit_cycles,
        } = outcome
        else {
            panic!("expected successful guest exit, got {outcome:?}");
        };

        let (limited, _) = HostRunner::new(&image, input.clone(), Some(exit_cycles))
            .unwrap()
            .run()
            .unwrap();
        assert_eq!(
            limited,
            HostOutcome::CycleLimit {
                cycles: exit_cycles
            }
        );
        let (complete, _) = HostRunner::new(&image, input, Some(exit_cycles + 1))
            .unwrap()
            .run()
            .unwrap();
        assert_eq!(
            complete,
            HostOutcome::GuestExit {
                status: 0,
                cycles: exit_cycles
            }
        );

        let mut child = Command::new("sim65")
            .args(["-x", "2000000"])
            .arg(&stock_path)
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()
            .unwrap();
        child
            .stdin
            .take()
            .unwrap()
            .write_all(&crate::frame_input(b"?=65", b"").unwrap())
            .unwrap();
        let stock = child.wait_with_output().unwrap();
        assert!(stock.status.success());
        assert_eq!(stock.stdout, b"65");

        for (framed, expected_status) in [
            (vec![5, 0, b'?', b'=', b'6'], 129),
            (crate::frame_input(b"A=", b"").unwrap(), 130),
        ] {
            let mut runner = HostRunner::new(&image, framed.clone(), Some(2_000_000)).unwrap();
            let mut output = Vec::new();
            let result = runner.run_to(&mut output).unwrap();
            assert_eq!(runner.cpu.registers.program_counter, 0xfff2);
            assert_eq!(output, b"");
            let HostOutcome::GuestExit { status, .. } = result else {
                panic!("expected frontend guest exit, got {result:?}");
            };
            assert_eq!(status, expected_status);
            let mut child = Command::new("sim65")
                .args(["-x", "2000000"])
                .arg(&stock_path)
                .stdin(Stdio::piped())
                .stdout(Stdio::piped())
                .stderr(Stdio::piped())
                .spawn()
                .unwrap();
            child.stdin.take().unwrap().write_all(&framed).unwrap();
            let stock = child.wait_with_output().unwrap();
            assert_eq!(stock.status.code(), Some(i32::from(expected_status)));
            assert!(stock.stdout.is_empty());
        }

        for (runtime_bytes, expected_stdout) in [(&b"\0A"[..], &b"65"[..]), (&b""[..], &b"-1"[..])]
        {
            let framed = crate::frame_input(b"A=$ ?=A", runtime_bytes).unwrap();
            let (result, output) = HostRunner::new(&image, framed.clone(), Some(2_000_000))
                .unwrap()
                .run()
                .unwrap();
            let HostOutcome::GuestExit { status, .. } = result else {
                panic!("expected successful guest exit, got {result:?}");
            };
            assert_eq!(status, 0);
            assert_eq!(output, expected_stdout);

            let mut child = Command::new("sim65")
                .args(["-x", "2000000"])
                .arg(&stock_path)
                .stdin(Stdio::piped())
                .stdout(Stdio::piped())
                .stderr(Stdio::piped())
                .spawn()
                .unwrap();
            child.stdin.take().unwrap().write_all(&framed).unwrap();
            let stock = child.wait_with_output().unwrap();
            assert_eq!(stock.status.code(), Some(0));
            assert_eq!(stock.stdout, expected_stdout);
        }
    }

    #[test]
    #[ignore = "requires ca65 and ld65; runs the mos6502 adapter on the private host"]
    fn mos_adapter_shares_source_wait_try_and_sticky_eof() {
        let (_temp, path) = compile_image(true);
        let image = std::fs::read(path).unwrap();
        // Try consumes NUL and 0xff, Wait skips NUL and returns 'A', then
        // both operations observe EOF from the same runtime session.
        let input = crate::frame_input(b"?=` ?=` ?=$ ?=` ?=$", &[0, 255, 0, 65]).unwrap();
        let (outcome, output) = HostRunner::new(&image, input, Some(2_000_000))
            .unwrap()
            .run()
            .unwrap();
        assert!(matches!(outcome, HostOutcome::GuestExit { status: 0, .. }));
        assert_eq!(output, b"025565-1-1");

        // Compilation failure cannot run or consume the runtime sentinel.
        let input = crate::frame_input(b"A=$+", b"Z").unwrap();
        let mut runner = HostRunner::new(&image, input, Some(2_000_000)).unwrap();
        let mut out = Vec::new();
        let outcome = runner.run_to(&mut out).unwrap();
        assert!(matches!(
            outcome,
            HostOutcome::GuestExit { status: 130, .. }
        ));
        assert!(out.is_empty());
        assert_eq!(runner.input.try_byte(), input::InputEvent::Byte(b'Z'));
    }

    #[test]
    #[ignore = "requires ca65 and ld65"]
    fn mos_adapter_rejects_truncated_source_frames_as_transport_errors() {
        let (_temp, path) = compile_image(true);
        let image = std::fs::read(path).unwrap();
        for input in [vec![], vec![5], vec![5, 0, b'?', b'=', b'6']] {
            let (outcome, stdout) = HostRunner::new(&image, input.clone(), Some(2_000_000))
                .unwrap()
                .run()
                .unwrap();
            assert!(
                matches!(outcome, HostOutcome::GuestExit { status: 129, .. }),
                "input {input:?}: unexpected outcome: {outcome:?}"
            );
            assert!(stdout.is_empty());
        }
    }

    #[test]
    fn runner_initial_state_and_ram_are_fresh() {
        use mos6502::{memory::Bus, registers::Status};
        let mut first = HostRunner::new(&[0x60], vec![], None).unwrap();
        assert_eq!(first.cpu.registers.program_counter, image::LOAD);
        assert_eq!(first.cpu.registers.stack_pointer.0, 0xff);
        assert!(!first.cpu.registers.status.contains(Status::PS_DECIMAL_MODE));
        assert!(
            first
                .cpu
                .registers
                .status
                .contains(Status::PS_DISABLE_INTERRUPTS)
        );
        assert_eq!(first.cpu.cycles, 0);
        first.cpu.memory.set_byte(0x0300, 0xa5);
        let mut second = HostRunner::new(&[0x60], vec![], None).unwrap();
        assert_eq!(second.cpu.memory.get_byte(0x0300), 0);
    }

    #[cfg(target_os = "linux")]
    #[test]
    fn observed_worker_failure_takes_precedence_over_successful_guest_exit() {
        // LDA #0 / JMP $fff2: exit successfully after the worker has failed.
        let image = b"\xa9\x00\x4c\xf2\xff";
        let mut runner = HostRunner::new(image, vec![], Some(100)).unwrap();
        let directory = std::fs::File::open(std::env::temp_dir()).unwrap();
        runner.input = input::HostInput::interactive(vec![], directory.into());
        assert_eq!(runner.input.wait_byte(), input::InputEvent::Fatal);
        let error = runner.run_to(&mut Vec::new()).unwrap_err();
        assert!(matches!(error, HostError::Input(_)));
        assert!(error.to_string().contains("guest input"));
    }

    #[test]
    fn reserved_services_are_rejected_by_runner() {
        for pc in 0xfff3u16..=0xffff {
            let [lo, hi] = pc.to_le_bytes();
            let mut image = b"\xa9\x2a\x4c".to_vec();
            image.extend_from_slice(&[lo, hi]);
            let result = HostRunner::new(&image, vec![], Some(100)).unwrap().run();
            assert!(
                matches!(result, Err(HostError::Service(message)) if message.contains(&format!("${pc:04x}")))
            );
        }
    }

    #[cfg(target_os = "linux")]
    #[test]
    #[ignore = "requires ca65 and ld65"]
    fn mos_try_empty_pipe_returns_immediately_then_wait_reads_arriving_byte() {
        use std::{sync::mpsc, time::Duration};

        let (_temp, path) = compile_image(true);
        let image = std::fs::read(path).unwrap();
        let (reader, writer) = std::io::pipe().unwrap();
        let prefix = crate::frame_input(b"?=`", b"").unwrap();
        let (send, receive) = mpsc::channel();
        let image_copy = image.clone();
        let thread = std::thread::spawn(move || {
            let result =
                HostRunner::interactive(&image_copy, prefix, reader.into(), Some(2_000_000))
                    .unwrap()
                    .run();
            send.send(result).unwrap();
        });
        let result = receive.recv_timeout(Duration::from_secs(5));
        drop(writer); // Also unblocks a broken implementation before joining.
        thread.join().unwrap();
        let (outcome, output) = result.expect("Try blocked on an open empty pipe").unwrap();
        assert!(matches!(outcome, HostOutcome::GuestExit { status: 0, .. }));
        assert_eq!(output, b"0");

        struct SendAfterOutput {
            output: Vec<u8>,
            writer: std::io::PipeWriter,
        }
        impl Write for SendAfterOutput {
            fn write(&mut self, bytes: &[u8]) -> std::io::Result<usize> {
                if self.output.is_empty() {
                    assert_eq!(bytes, b"0");
                    self.writer.write_all(b"A")?;
                }
                self.output.extend_from_slice(bytes);
                Ok(bytes.len())
            }
            fn flush(&mut self) -> std::io::Result<()> {
                Ok(())
            }
        }
        let (reader, writer) = std::io::pipe().unwrap();
        let prefix = crate::frame_input(b"?=` ?=$", b"").unwrap();
        let mut runner =
            HostRunner::interactive(&image, prefix, reader.into(), Some(2_000_000)).unwrap();
        let mut output = SendAfterOutput {
            output: Vec::new(),
            writer,
        };
        let outcome = runner.run_to(&mut output).unwrap();
        assert!(matches!(outcome, HostOutcome::GuestExit { status: 0, .. }));
        assert_eq!(output.output, b"065");
    }

    #[test]
    fn guest_output_write_failure_is_a_host_error() {
        struct FailOnWrite;
        impl Write for FailOnWrite {
            fn write(&mut self, _: &[u8]) -> std::io::Result<usize> {
                Err(std::io::Error::other("injected output failure"))
            }
            fn flush(&mut self) -> std::io::Result<()> {
                Err(std::io::Error::other("injected flush failure"))
            }
        }
        // After output, the guest would write a sentinel and halt.
        let image = b"\xa9X\x20\xf1\xff\xa9\x42\x8d\x00\x03\x4c\xf2\xff";
        let mut writer = FailOnWrite;
        let mut runner = HostRunner::new(image, vec![], Some(100)).unwrap();
        let error = runner.run_to(&mut writer).unwrap_err();
        assert!(matches!(error, HostError::Output(_)));
        assert_output_stopped(&mut runner);
    }

    #[test]
    fn guest_output_flush_failure_is_a_host_error() {
        struct FailOnFlush;
        impl Write for FailOnFlush {
            fn write(&mut self, bytes: &[u8]) -> std::io::Result<usize> {
                Ok(bytes.len())
            }
            fn flush(&mut self) -> std::io::Result<()> {
                Err(std::io::Error::other("injected flush failure"))
            }
        }
        let image = b"\xa9X\x20\xf1\xff\xa9\x42\x8d\x00\x03\x4c\xf2\xff";
        let mut writer = FailOnFlush;
        let mut runner = HostRunner::new(image, vec![], Some(100)).unwrap();
        let error = runner.run_to(&mut writer).unwrap_err();
        assert!(matches!(error, HostError::Output(_)));
        assert_output_stopped(&mut runner);
    }

    fn assert_output_stopped(runner: &mut HostRunner) {
        use mos6502::memory::Bus;
        assert_eq!(runner.cpu.registers.program_counter, 0xfff1);
        assert_eq!(runner.cpu.registers.stack_pointer.0, 0xfd);
        assert_eq!(runner.cpu.memory.get_byte(0x0300), 0);
    }
    #[test]
    #[ignore = "requires ca65 and ld65"]
    fn mos_adapter_propagates_runtime_fatal_statuses() {
        let (_temp, path) = compile_image(true);
        let image = std::fs::read(path).unwrap();
        for (source, expected) in [
            (&b"?=~"[..], 1),
            (&b"?=1/0"[..], 3),
            (&b"?=1%0"[..], 4),
            (&b"?=!"[..], 5),
            (&b".DATA:=[1] A=.DATA(1)"[..], 7),
        ] {
            let input = crate::frame_input(source, b"").unwrap();
            let mut runner = HostRunner::new(&image, input, Some(2_000_000)).unwrap();
            let mut output = Vec::new();
            let outcome = runner.run_to(&mut output).unwrap();
            assert_eq!(runner.cpu.registers.program_counter, 0xfff2);
            assert!(
                matches!(outcome, HostOutcome::GuestExit { status, .. } if status == expected),
                "source {source:?}: {outcome:?}"
            );
            assert!(output.is_empty(), "source {source:?}: {output:?}");
        }
    }

    #[test]
    #[ignore = "requires ca65 and ld65"]
    fn mos_adapter_propagates_frontend_stack_guard_status() {
        for observed in [24, 25] {
            let (_temp, path) = compile_image_with_guard(true, Some(observed));
            let image = std::fs::read(path).unwrap();
            // Exercises value-first and stack-root operand paths, as in sim65_frontend.
            let input = crate::frame_input(b"~=40 ?=~+2", b"").unwrap();
            let mut runner = HostRunner::new(&image, input, Some(2_000_000)).unwrap();
            let mut output = Vec::new();
            let outcome = runner.run_to(&mut output).unwrap();
            assert_eq!(runner.cpu.registers.program_counter, 0xfff2);
            assert!(
                matches!(outcome, HostOutcome::GuestExit { status, .. } if status == if observed == 24 { 132 } else { 0 })
            );
            assert_eq!(
                output,
                if observed == 24 {
                    b"".as_slice()
                } else {
                    b"42".as_slice()
                }
            );
        }
        let source = format!(
            "%=0 [ {}A={}0+0{}{} ?=A",
            "%=1 [ ".repeat(15),
            "0+(".repeat(16),
            ")".repeat(16),
            " ]".repeat(16)
        );
        assert_eq!(source.len(), 201);
        let (_temp, path) = compile_image(true);
        let image = std::fs::read(path).unwrap();
        let input = crate::frame_input(source.as_bytes(), b"").unwrap();
        let (outcome, output) = HostRunner::new(&image, input, Some(2_000_000))
            .unwrap()
            .run()
            .unwrap();
        assert!(matches!(outcome, HostOutcome::GuestExit { status: 0, .. }));
        assert_eq!(output, b"0");
    }
}
