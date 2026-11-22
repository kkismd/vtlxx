use super::HostError;
use mos6502::{
    cpu::CPU,
    instruction::Nmos6502,
    memory::{Bus, Memory},
};
use std::io::{Read, Write};

#[derive(Debug)]
pub(super) enum ServiceResult {
    Continue,
    Exit(u8),
}

fn word(memory: &mut Memory, address: u16) -> u16 {
    u16::from(memory.get_byte(address)) | (u16::from(memory.get_byte(address.wrapping_add(1))) << 8)
}

fn return_from_service(cpu: &mut CPU<Memory, Nmos6502>) {
    cpu.registers.stack_pointer.increment();
    let low = cpu.memory.get_byte(cpu.registers.stack_pointer.to_u16());
    cpu.registers.stack_pointer.increment();
    let high = cpu.memory.get_byte(cpu.registers.stack_pointer.to_u16());
    cpu.registers.program_counter = u16::from_le_bytes([low, high]).wrapping_add(1);
}

pub(super) fn dispatch(
    cpu: &mut CPU<Memory, Nmos6502>,
    pc: u16,
    input: &mut impl Read,
    stdout: &mut dyn Write,
    stderr: &mut dyn Write,
    software_stack_zp: u8,
) -> Result<ServiceResult, HostError> {
    if pc == 0xfff9 {
        return Ok(ServiceResult::Exit(cpu.registers.accumulator));
    }
    if pc != 0xfff6 && pc != 0xfff7 {
        return Err(HostError::Service(format!(
            "unsupported cc65 PC service ${pc:04x}"
        )));
    }
    let count = u16::from(cpu.registers.accumulator) | (u16::from(cpu.registers.index_x) << 8);
    let zp = u16::from(software_stack_zp);
    let sp = word(&mut cpu.memory, zp);
    let buffer = word(&mut cpu.memory, sp);
    let fd = word(
        &mut cpu.memory,
        sp.checked_add(2)
            .ok_or_else(|| HostError::Service("software stack address overflow".into()))?,
    );
    let after = sp
        .checked_add(4)
        .ok_or_else(|| HostError::Service("software stack address overflow".into()))?;
    if after > 0xffef {
        return Err(HostError::Service(
            "software stack pop crosses reserved host services".into(),
        ));
    }
    let end = usize::from(buffer)
        .checked_add(usize::from(count))
        .ok_or_else(|| HostError::Service("guest buffer address overflow".into()))?;
    if end > 0x1_0000 {
        return Err(HostError::Service(
            "guest buffer wraps address space".into(),
        ));
    }
    if usize::from(buffer) < 0x1_0000 && end > 0xffef {
        return Err(HostError::Service(
            "guest buffer overlaps reserved host services at $ffef".into(),
        ));
    }
    if (pc == 0xfff6 && fd != 0) || (pc == 0xfff7 && fd != 1 && fd != 2) {
        return Err(HostError::Service(format!(
            "unsupported fd {fd} for service ${pc:04x}"
        )));
    }
    let mut data = vec![0u8; usize::from(count)];
    let actual = if pc == 0xfff6 {
        let n = input.read(&mut data).map_err(HostError::Input)?;
        for (offset, byte) in data[..n].iter().copied().enumerate() {
            cpu.memory.set_byte(buffer + offset as u16, byte);
        }
        n
    } else {
        for (offset, byte) in data.iter_mut().enumerate() {
            *byte = cpu.memory.get_byte(buffer + offset as u16);
        }
        let result = match fd {
            1 => stdout.write_all(&data).and_then(|_| stdout.flush()),
            2 => stderr.write_all(&data).and_then(|_| stderr.flush()),
            _ => unreachable!(),
        };
        result.map_err(HostError::Output)?;
        usize::from(count)
    };
    cpu.memory.set_byte(zp, after as u8);
    cpu.memory.set_byte(zp + 1, (after >> 8) as u8);
    cpu.registers.accumulator = actual as u8;
    cpu.registers.index_x = (actual >> 8) as u8;
    return_from_service(cpu);
    Ok(ServiceResult::Continue)
}

#[cfg(test)]
mod tests {
    use super::*;
    use mos6502::memory::Bus;

    fn cpu() -> CPU<Memory, Nmos6502> {
        let mut cpu = CPU::new(Memory::new(), Nmos6502);
        cpu.memory.set_byte(0xf0, 0x00);
        cpu.memory.set_byte(0xf1, 0x20);
        cpu.memory.set_byte(0x2000, 0x00);
        cpu.memory.set_byte(0x2001, 0x30);
        cpu.memory.set_byte(0x2002, 0x00);
        cpu.memory.set_byte(0x2003, 0x00);
        cpu.registers.stack_pointer.0 = 0xfd;
        cpu.memory.set_byte(0x01fe, 0x34);
        cpu.memory.set_byte(0x01ff, 0x12);
        cpu.registers.program_counter = 0xfff6;
        cpu.registers.accumulator = 3;
        cpu
    }

    #[test]
    fn pvread_copies_bytes_pops_software_stack_and_returns_through_hardware_stack() {
        let mut cpu = cpu();
        let mut input = &b"xy"[..];
        let (mut stdout, mut stderr) = (Vec::new(), Vec::new());
        assert!(matches!(
            dispatch(&mut cpu, 0xfff6, &mut input, &mut stdout, &mut stderr, 0xf0).unwrap(),
            ServiceResult::Continue
        ));
        assert_eq!(
            [cpu.memory.get_byte(0x3000), cpu.memory.get_byte(0x3001)],
            *b"xy"
        );
        assert_eq!(cpu.registers.accumulator, 2);
        assert_eq!(cpu.registers.index_x, 0);
        assert_eq!(cpu.registers.program_counter, 0x1235);
        assert_eq!(cpu.memory.get_byte(0xf0), 4);
        assert_eq!(cpu.memory.get_byte(0xf1), 0x20);
    }

    #[test]
    fn invalid_service_fd_and_buffer_fail_before_stack_or_register_mutation() {
        let mut cpu = cpu();
        cpu.memory.set_byte(0x2002, 9);
        let mut input = &b"x"[..];
        let (mut stdout, mut stderr) = (Vec::new(), Vec::new());
        assert!(dispatch(&mut cpu, 0xfff6, &mut input, &mut stdout, &mut stderr, 0xf0).is_err());
        assert_eq!(cpu.memory.get_byte(0xf0), 0);
        assert_eq!(cpu.registers.accumulator, 3);
        cpu.memory.set_byte(0x2002, 0);
        cpu.memory.set_byte(0x2000, 0xee);
        cpu.memory.set_byte(0x2001, 0xff);
        assert!(dispatch(&mut cpu, 0xfff6, &mut input, &mut stdout, &mut stderr, 0xf0).is_err());
        assert_eq!(cpu.memory.get_byte(0xf0), 0);
        assert!(dispatch(&mut cpu, 0xfff1, &mut input, &mut stdout, &mut stderr, 0xf0).is_err());
    }

    #[test]
    fn pvread_transport_error_does_not_become_guest_eof_or_partial_success() {
        struct FailingReader;
        impl Read for FailingReader {
            fn read(&mut self, _: &mut [u8]) -> std::io::Result<usize> {
                Err(std::io::Error::other("injected input failure"))
            }
        }
        let mut cpu = cpu();
        let (mut stdout, mut stderr) = (Vec::new(), Vec::new());
        let error = dispatch(
            &mut cpu,
            0xfff6,
            &mut FailingReader,
            &mut stdout,
            &mut stderr,
            0xf0,
        )
        .unwrap_err();
        assert!(matches!(error, HostError::Input(_)));
        assert_eq!(cpu.registers.accumulator, 3);
        assert_eq!(cpu.memory.get_byte(0xf0), 0);
        assert_eq!(cpu.memory.get_byte(0x3000), 0);
        assert_eq!(cpu.registers.program_counter, 0xfff6);
    }
}
