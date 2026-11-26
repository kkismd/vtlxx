use super::HostError;
use mos6502::memory::{Bus, Memory};

pub(crate) const LOAD: u16 = 0x0200;
pub(crate) const SERVICE_START: u16 = 0xffef;
const MAX_PAYLOAD: usize = SERVICE_START as usize - LOAD as usize;

pub(super) fn load(bytes: &[u8]) -> Result<Memory, HostError> {
    if bytes.is_empty() || bytes.len() > MAX_PAYLOAD {
        return Err(HostError::Image(format!(
            "raw image length must be 1..={MAX_PAYLOAD} bytes"
        )));
    }
    let mut memory = Memory::new();
    for (offset, byte) in bytes.iter().copied().enumerate() {
        memory.set_byte((usize::from(LOAD) + offset) as u16, byte);
    }
    Ok(memory)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn raw_payload_bounds_and_zeroed_memory() {
        assert!(load(&[]).is_err());
        assert!(load(&vec![0; MAX_PAYLOAD + 1]).is_err());
        let mut bytes = vec![0x5a; MAX_PAYLOAD];
        bytes[0] = 0xa5;
        let mut memory = load(&bytes).unwrap();
        assert_eq!(memory.get_byte(LOAD), 0xa5);
        assert_eq!(memory.get_byte(SERVICE_START - 1), 0x5a);
        for address in [0, 0xff, 0x100, 0x1ff, SERVICE_START, 0xffff] {
            assert_eq!(memory.get_byte(address), 0);
        }
        let mut short = load(&[0x42]).unwrap();
        assert_eq!(short.get_byte(LOAD), 0x42);
        assert_eq!(short.get_byte(LOAD + 1), 0);
    }
}
