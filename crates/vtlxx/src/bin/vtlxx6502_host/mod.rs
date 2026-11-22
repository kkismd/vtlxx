#![allow(dead_code)] // CLI selection and live host streams are the responsibility of #600.

//! Private runner for the stock sim6502 image format. This is deliberately
//! local to the `vtlxx6502` binary; CLI selection is added by a later issue.
mod image;
mod services;

use mos6502::{cpu::CPU, instruction::Nmos6502, memory::Memory};
use std::io::Write;

#[derive(Debug, PartialEq, Eq)]
pub(crate) enum HostOutcome {
    GuestExit { status: u8, cycles: u64 },
    CycleLimit { cycles: u64 },
}

#[derive(Debug)]
pub(crate) enum HostError {
    Image(String),
    Service(String),
    Input(std::io::Error),
    Output(std::io::Error),
}

impl std::fmt::Display for HostError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Image(message) | Self::Service(message) => f.write_str(message),
            Self::Input(error) => write!(f, "guest input: {error}"),
            Self::Output(error) => write!(f, "guest output: {error}"),
        }
    }
}

pub(crate) struct HostRunner {
    cpu: CPU<Memory, Nmos6502>,
    input: std::io::Cursor<Vec<u8>>,
    cycle_limit: Option<u64>,
    software_stack_zp: u8,
}

impl HostRunner {
    pub(crate) fn new(
        bytes: &[u8],
        input: Vec<u8>,
        cycle_limit: Option<u64>,
    ) -> Result<Self, HostError> {
        let loaded = image::load(bytes)?;
        let mut cpu = CPU::new(loaded.memory, Nmos6502);
        cpu.registers.program_counter = loaded.entry;
        cpu.registers.stack_pointer.0 = 0xff;
        Ok(Self {
            cpu,
            input: std::io::Cursor::new(input),
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

    fn run_to(
        &mut self,
        stdout: &mut dyn Write,
        stderr: &mut dyn Write,
    ) -> Result<HostOutcome, HostError> {
        loop {
            if self
                .cycle_limit
                .is_some_and(|limit| self.cpu.cycles >= limit)
            {
                return Ok(HostOutcome::CycleLimit {
                    cycles: self.cpu.cycles,
                });
            }
            let pc = self.cpu.registers.program_counter;
            if (0xfff1..=0xfff9).contains(&pc) {
                match services::dispatch(
                    &mut self.cpu,
                    pc,
                    &mut self.input,
                    stdout,
                    stderr,
                    self.software_stack_zp,
                )? {
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

    fn compile_image() -> (Temp, PathBuf) {
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
            "6502/target/sim65_adapter.s",
        ];
        let mut objects = Vec::new();
        for (index, name) in sources.iter().enumerate() {
            let object = temp.0.join(format!("{index}.o"));
            let mut command = Command::new("ca65");
            command.args([OsStr::new("-t"), OsStr::new("sim6502")]);
            if name.ends_with("sim65_adapter.s") {
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

    #[test]
    #[ignore = "requires ca65, ld65, and sim65; runs the stock image directly on mos6502"]
    fn stock_adapter_image_runs_source_compile_output_and_exit() {
        let (_temp, path) = compile_image();
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
                cycles: 466_289
            }
        );

        let (limited, _, _) = HostRunner::new(&image, input.clone(), Some(466_289))
            .unwrap()
            .run()
            .unwrap();
        assert_eq!(limited, HostOutcome::CycleLimit { cycles: 466_289 });
        let (complete, _, _) = HostRunner::new(&image, input, Some(466_290))
            .unwrap()
            .run()
            .unwrap();
        assert_eq!(
            complete,
            HostOutcome::GuestExit {
                status: 0,
                cycles: 466_289
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
