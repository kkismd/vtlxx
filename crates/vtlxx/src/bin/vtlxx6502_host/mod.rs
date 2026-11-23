#![allow(dead_code)] // CLI selection and live host streams are the responsibility of #600.

//! Private runner for the stock sim6502 image format. This is deliberately
//! local to the `vtlxx6502` binary; CLI selection is added by a later issue.
mod image;
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
    software_stack_zp: u8,
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
        let loaded = image::load(bytes)?;
        let mut cpu = CPU::new(loaded.memory, Nmos6502);
        cpu.registers.program_counter = loaded.entry;
        cpu.registers.stack_pointer.0 = 0xff;
        Ok(Self {
            cpu,
            input,
            cycle_limit,
            software_stack_zp: loaded.software_stack_zp,
        })
    }

    pub(crate) fn run(mut self) -> Result<(HostOutcome, Vec<u8>, Vec<u8>), HostError> {
        let mut stdout = Vec::new();
        let mut stderr = Vec::new();
        let outcome = self.run_to(&mut stdout, &mut stderr)?;
        Ok((outcome, stdout, stderr))
    }

    pub(crate) fn run_to(
        &mut self,
        stdout: &mut dyn Write,
        stderr: &mut dyn Write,
    ) -> Result<HostOutcome, HostError> {
        self.run_with_callback(stdout, stderr, &mut |_| Ok(None))
    }

    #[cfg(target_os = "linux")]
    pub(crate) fn run_to_with_signals(
        &mut self,
        stdout: &mut dyn Write,
        stderr: &mut dyn Write,
        signal_fd: std::os::fd::RawFd,
        mut callback: impl FnMut(bool) -> std::io::Result<Option<i32>>,
    ) -> Result<HostOutcome, HostError> {
        self.input
            .set_signal_fd(signal_fd)
            .map_err(HostError::Signal)?;
        self.run_with_callback(stdout, stderr, &mut callback)
    }

    fn run_with_callback(
        &mut self,
        stdout: &mut dyn Write,
        stderr: &mut dyn Write,
        callback: &mut dyn FnMut(bool) -> std::io::Result<Option<i32>>,
    ) -> Result<HostOutcome, HostError> {
        let result = self.run_loop_with_callback(stdout, stderr, callback);
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

    fn run_loop(
        &mut self,
        stdout: &mut dyn Write,
        stderr: &mut dyn Write,
    ) -> Result<HostOutcome, HostError> {
        self.run_loop_with_callback(stdout, stderr, &mut |_| Ok(None))
    }

    fn run_loop_with_callback(
        &mut self,
        stdout: &mut dyn Write,
        stderr: &mut dyn Write,
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
            if pc == 0xffef || pc == 0xfff0 {
                woken = !services::input_service(&mut self.cpu, pc, &mut self.input);
            } else if (0xfff1..=0xfff9).contains(&pc) {
                let service = services::dispatch(
                    &mut self.cpu,
                    pc,
                    &mut self.input,
                    stdout,
                    stderr,
                    self.software_stack_zp,
                );
                if matches!(&service, Err(HostError::Input(e)) if e.kind() == std::io::ErrorKind::Interrupted)
                {
                    woken = true;
                    continue;
                }
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
    let mut stderr = std::io::stderr().lock();
    HostRunner::new(image, input.to_vec(), cycle_limit)?.run_to(writer, &mut stderr)
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
        let temp = Temp::new();
        let root = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
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
            command.args([OsStr::new("-t"), OsStr::new("sim6502")]);
            if name.ends_with("adapter.s") {
                command.args([OsStr::new("-D"), OsStr::new("SIM65_ENTROPY_UNAVAILABLE")]);
            }
            let status = command
                .arg(root.join(name))
                .arg("-o")
                .arg(&object)
                .status()
                .unwrap();
            assert!(status.success(), "ca65 failed for {name}");
            objects.push(object);
        }
        let image = temp.0.join("vtlxx.prg");
        let mut command = Command::new("ld65");
        command
            .args([OsStr::new("-t"), OsStr::new("sim6502"), OsStr::new("-o")])
            .arg(&image);
        command.args(&objects).arg("sim6502.lib");
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
            command.args(["-t", "sim6502"]);
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
                .args(["-t", "sim6502", "-o"])
                .arg(&image)
                .args(&objects)
                .arg("sim6502.lib")
                .status()
                .unwrap()
                .success()
        );
        (temp, image)
    }

    #[test]
    #[ignore = "requires ca65 and ld65"]
    fn mos_try_overflow_preserves_stack_eof_and_input() {
        for (case, wait) in [(16, false), (17, false), (16, true), (17, true)] {
            let (_temp, path) = compile_runtime_probe("runtime_try_capacity.s", case, wait);
            let image = std::fs::read(path).unwrap();
            let mut runner = HostRunner::new(&image, vec![65], Some(2_000_000)).unwrap();
            let mut out = Vec::new();
            let result = runner.run_to(&mut out, &mut Vec::new()).unwrap();
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
        let result = runner.run_loop(&mut out, &mut Vec::new()).unwrap();
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
            let result = runner.run_loop(&mut out, &mut Vec::new()).unwrap();
            assert!(matches!(result, HostOutcome::GuestExit { status: 6, .. }));
            assert!(out.is_empty());
            assert!(runner.input.finish().is_err());
        }
    }

    #[test]
    #[ignore = "requires ca65, ld65, and sim65; runs the stock image directly on mos6502"]
    fn stock_adapter_image_runs_source_compile_output_and_exit() {
        let (_temp, path) = compile_image(false);
        let image = std::fs::read(&path).unwrap();
        let input = crate::frame_input(b"?=65", b"").unwrap();
        let (outcome, stdout, _) = HostRunner::new(&image, input.clone(), Some(2_000_000))
            .unwrap()
            .run()
            .unwrap();
        assert_eq!(stdout, b"65");
        assert_eq!(
            outcome,
            HostOutcome::GuestExit {
                status: 0,
                cycles: 471_353
            }
        );

        let (limited, _, _) = HostRunner::new(&image, input.clone(), Some(471_353))
            .unwrap()
            .run()
            .unwrap();
        assert_eq!(limited, HostOutcome::CycleLimit { cycles: 471_353 });
        let (complete, _, _) = HostRunner::new(&image, input, Some(471_354))
            .unwrap()
            .run()
            .unwrap();
        assert_eq!(
            complete,
            HostOutcome::GuestExit {
                status: 0,
                cycles: 471_353
            }
        );

        let mut child = Command::new("sim65")
            .args(["-x", "2000000"])
            .arg(&path)
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
            let (result, output, _) = HostRunner::new(&image, framed.clone(), Some(2_000_000))
                .unwrap()
                .run()
                .unwrap();
            assert_eq!(output, b"");
            let HostOutcome::GuestExit { status, .. } = result else {
                panic!("expected frontend guest exit, got {result:?}");
            };
            assert_eq!(status, expected_status);
            let mut child = Command::new("sim65")
                .args(["-x", "2000000"])
                .arg(&path)
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
            let (result, output, _) = HostRunner::new(&image, framed.clone(), Some(2_000_000))
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
                .arg(&path)
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
        let (outcome, output, _) = HostRunner::new(&image, input, Some(2_000_000))
            .unwrap()
            .run()
            .unwrap();
        assert!(matches!(outcome, HostOutcome::GuestExit { status: 0, .. }));
        assert_eq!(output, b"025565-1-1");

        // Compilation failure cannot run or consume the runtime sentinel.
        let input = crate::frame_input(b"A=$+", b"Z").unwrap();
        let mut runner = HostRunner::new(&image, input, Some(2_000_000)).unwrap();
        let mut out = Vec::new();
        let outcome = runner.run_to(&mut out, &mut Vec::new()).unwrap();
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
            let (outcome, stdout, stderr) = HostRunner::new(&image, input.clone(), Some(2_000_000))
                .unwrap()
                .run()
                .unwrap();
            assert!(
                matches!(outcome, HostOutcome::GuestExit { status: 129, .. }),
                "input {input:?}: unexpected outcome: {outcome:?}"
            );
            assert!(stdout.is_empty());
            assert!(stderr.is_empty());
        }
    }

    #[cfg(target_os = "linux")]
    #[test]
    fn observed_worker_failure_takes_precedence_over_successful_guest_exit() {
        // LDA #0 / JMP $fff9: exit successfully after the worker has failed.
        let image = b"sim65\x02\x00\xf0\x00\x02\x00\x02\xa9\x00\x4c\xf9\xff";
        let mut runner = HostRunner::new(image, vec![], Some(100)).unwrap();
        let directory = std::fs::File::open(std::env::temp_dir()).unwrap();
        runner.input = input::HostInput::interactive(vec![], directory.into());
        assert_eq!(runner.input.wait_byte(), input::InputEvent::Fatal);
        let error = runner.run_to(&mut Vec::new(), &mut Vec::new()).unwrap_err();
        assert!(matches!(error, HostError::Input(_)));
        assert!(error.to_string().contains("guest input"));
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
        let (outcome, output, _) = result.expect("Try blocked on an open empty pipe").unwrap();
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
        let outcome = runner.run_to(&mut output, &mut Vec::new()).unwrap();
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
        // LDA #1 / LDX #0 / JSR $fff7 / LDA #0 / JMP $fff9, with a
        // one-byte write buffer at $0300 and the cc65 software stack at $200a.
        let mut body = vec![
            0xa9, 0x0a, 0x85, 0xf0, 0xa9, 0x20, 0x85, 0xf1, 0xa9, 0x00, 0x8d, 0x0a, 0x20, 0xa9,
            0x03, 0x8d, 0x0b, 0x20, 0xa9, 0x01, 0x8d, 0x0c, 0x20, 0xa9, 0x00, 0x8d, 0x0d, 0x20,
            0xa9, 0x01, 0xa2, 0x00, 0x20, 0xf7, 0xff, 0xa9, 0x00, 0x4c, 0xf9, 0xff,
        ];
        body.resize(0x100, 0xea);
        body.push(b'X');
        let mut image = b"sim65\x02\x00\xf0\x00\x02\x00\x02".to_vec();
        image.extend_from_slice(&body);
        let mut writer = FailOnWrite;
        let error = run_with_io(&image, &[], Some(100), &mut writer).unwrap_err();
        assert!(matches!(error, HostError::Output(_)));
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
        let mut body = vec![
            0xa9, 0x0a, 0x85, 0xf0, 0xa9, 0x20, 0x85, 0xf1, 0xa9, 0x00, 0x8d, 0x0a, 0x20, 0xa9,
            0x03, 0x8d, 0x0b, 0x20, 0xa9, 0x01, 0x8d, 0x0c, 0x20, 0xa9, 0x00, 0x8d, 0x0d, 0x20,
            0xa9, 0x01, 0xa2, 0x00, 0x20, 0xf7, 0xff, 0xa9, 0x00, 0x4c, 0xf9, 0xff,
        ];
        body.resize(0x100, 0xea);
        body.push(b'X');
        let mut image = b"sim65\x02\x00\xf0\x00\x02\x00\x02".to_vec();
        image.extend_from_slice(&body);
        let mut writer = FailOnFlush;
        let error = run_with_io(&image, &[], Some(100), &mut writer).unwrap_err();
        assert!(matches!(error, HostError::Output(_)));
    }
}
