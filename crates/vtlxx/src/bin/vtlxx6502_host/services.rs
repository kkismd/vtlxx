use super::HostError;
use super::input::{HostInput, InputEvent};
use mos6502::{
    cpu::CPU,
    instruction::Nmos6502,
    memory::{Bus, Memory},
};
use std::io::Write;

#[derive(Debug)]
pub(super) enum ServiceResult {
    Continue,
    Exit(u8),
}

fn return_from_service(cpu: &mut CPU<Memory, Nmos6502>) {
    cpu.registers.stack_pointer.increment();
    let low = cpu.memory.get_byte(cpu.registers.stack_pointer.to_u16());
    cpu.registers.stack_pointer.increment();
    let high = cpu.memory.get_byte(cpu.registers.stack_pointer.to_u16());
    cpu.registers.program_counter = u16::from_le_bytes([low, high]).wrapping_add(1);
}

pub(super) fn input_service(
    cpu: &mut CPU<Memory, Nmos6502>,
    pc: u16,
    input: &mut HostInput,
) -> bool {
    let event = if pc == 0xffef {
        input.try_byte()
    } else {
        input.wait_byte()
    };
    if event == InputEvent::Interrupted {
        return false;
    }
    cpu.registers.index_x = match event {
        InputEvent::Byte(byte) => {
            cpu.registers.accumulator = byte;
            0
        }
        InputEvent::NotReady => 1,
        InputEvent::Eof => 2,
        InputEvent::Fatal => 3,
        InputEvent::Interrupted => unreachable!(),
    };
    return_from_service(cpu);
    true
}

pub(super) fn dispatch(
    cpu: &mut CPU<Memory, Nmos6502>,
    pc: u16,
    stdout: &mut dyn Write,
) -> Result<ServiceResult, HostError> {
    match pc {
        0xfff1 => {
            stdout
                .write_all(&[cpu.registers.accumulator])
                .and_then(|()| stdout.flush())
                .map_err(HostError::Output)?;
            return_from_service(cpu);
            Ok(ServiceResult::Continue)
        }
        0xfff2 => Ok(ServiceResult::Exit(cpu.registers.accumulator)),
        _ => Err(HostError::Service(format!(
            "unsupported host service at ${pc:04x}"
        ))),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn cpu() -> CPU<Memory, Nmos6502> {
        let mut cpu = CPU::new(Memory::new(), Nmos6502);
        cpu.registers.stack_pointer.0 = 0xfd;
        cpu.memory.set_byte(0x01fe, 0x34);
        cpu.memory.set_byte(0x01ff, 0x12);
        cpu.registers.program_counter = 0xfff1;
        cpu
    }

    #[test]
    fn output_returns_through_hardware_stack_without_memory_changes() {
        let mut cpu = cpu();
        cpu.registers.accumulator = 0;
        cpu.registers.index_x = 0x56;
        cpu.registers.index_y = 0x78;
        cpu.memory.set_byte(0x2000, 0xaa);
        let mut output = Vec::new();
        assert!(matches!(
            dispatch(&mut cpu, 0xfff1, &mut output).unwrap(),
            ServiceResult::Continue
        ));
        assert_eq!(output, [0]);
        assert_eq!(cpu.registers.program_counter, 0x1235);
        assert_eq!(cpu.registers.stack_pointer.0, 0xff);
        assert_eq!(cpu.memory.get_byte(0x2000), 0xaa);
        assert_eq!(cpu.memory.get_byte(0x01fe), 0x34);
        assert_eq!(cpu.memory.get_byte(0x01ff), 0x12);
        assert_eq!(cpu.registers.index_x, 0x56);
        assert_eq!(cpu.registers.index_y, 0x78);
    }

    #[test]
    fn output_wraps_hardware_stack_and_return_address() {
        let mut cpu = cpu();
        cpu.registers.stack_pointer.0 = 0xfe;
        cpu.memory.set_byte(0x01ff, 0xff);
        cpu.memory.set_byte(0x0100, 0xff);
        cpu.registers.accumulator = 0xff;
        let mut output = Vec::new();
        dispatch(&mut cpu, 0xfff1, &mut output).unwrap();
        assert_eq!(output, [0xff]);
        assert_eq!(cpu.registers.stack_pointer.0, 0x00);
        assert_eq!(cpu.registers.program_counter, 0x0000);
    }

    #[test]
    fn halt_returns_status_without_popping_stack() {
        for pc in [0xfff2] {
            for status in [0, 2, 255] {
                let mut cpu = cpu();
                cpu.registers.program_counter = pc;
                cpu.registers.accumulator = status;
                assert!(
                    matches!(dispatch(&mut cpu, pc, &mut Vec::new()).unwrap(), ServiceResult::Exit(value) if value == status)
                );
                assert_eq!(cpu.registers.program_counter, pc);
                assert_eq!(cpu.registers.stack_pointer.0, 0xfd);
            }
        }
    }

    #[test]
    fn reserved_services_are_rejected_without_mutation() {
        for pc in 0xfff3..=0xffff {
            let mut cpu = cpu();
            cpu.registers.program_counter = pc;
            cpu.registers.accumulator = 0x42;
            let error = dispatch(&mut cpu, pc, &mut Vec::new()).unwrap_err();
            assert!(
                matches!(error, HostError::Service(message) if message.contains(&format!("${pc:04x}")))
            );
            assert_eq!(cpu.registers.program_counter, pc);
            assert_eq!(cpu.registers.stack_pointer.0, 0xfd);
            assert_eq!(cpu.registers.accumulator, 0x42);
        }
    }

    #[test]
    fn output_failures_preserve_prefix_and_do_not_return() {
        #[derive(Default)]
        struct FailingWriter {
            bytes: Vec<u8>,
            fail_flush: bool,
        }
        impl Write for FailingWriter {
            fn write(&mut self, bytes: &[u8]) -> std::io::Result<usize> {
                if self.bytes.len() == 1 && !self.fail_flush {
                    return Err(std::io::Error::other("injected write failure"));
                }
                self.bytes.extend_from_slice(bytes);
                Ok(bytes.len())
            }
            fn flush(&mut self) -> std::io::Result<()> {
                if self.fail_flush {
                    Err(std::io::Error::other("injected flush failure"))
                } else {
                    Ok(())
                }
            }
        }
        for fail_flush in [false, true] {
            let mut cpu = cpu();
            let mut writer = FailingWriter::default();
            cpu.registers.accumulator = b'a';
            dispatch(&mut cpu, 0xfff1, &mut writer).unwrap();
            cpu.registers.program_counter = 0xfff1;
            cpu.registers.stack_pointer.0 = 0xfd;
            cpu.registers.accumulator = b'b';
            writer.fail_flush = fail_flush;
            assert!(matches!(
                dispatch(&mut cpu, 0xfff1, &mut writer),
                Err(HostError::Output(_))
            ));
            assert_eq!(
                writer.bytes,
                if fail_flush { &b"ab"[..] } else { &b"a"[..] }
            );
            assert_eq!(cpu.registers.program_counter, 0xfff1);
            assert_eq!(cpu.registers.stack_pointer.0, 0xfd);
            assert_eq!(cpu.memory.get_byte(0x01fe), 0x34);
            assert_eq!(cpu.memory.get_byte(0x01ff), 0x12);
        }
    }
}
