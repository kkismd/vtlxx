use super::HostError;
use mos6502::memory::{Bus, Memory};

const HEADER_LEN: usize = 12;
const SERVICE_START: usize = 0xffef;
const SIM65_V2_NMOS: u8 = 0;
const INITIAL_CC65_SP: u16 = 0xffef;

pub(super) struct LoadedImage {
    pub(super) memory: Memory,
    pub(super) entry: u16,
}

pub(super) fn load(bytes: &[u8]) -> Result<LoadedImage, HostError> {
    if bytes.len() < HEADER_LEN {
        return Err(HostError::Image(format!(
            "sim65 image header is truncated: {} bytes",
            bytes.len()
        )));
    }
    if &bytes[..6] != b"sim65\x02" || bytes[6] != SIM65_V2_NMOS {
        return Err(HostError::Image(
            "invalid sim65 v2 signature or CPU type".into(),
        ));
    }
    if bytes[7] > 0xfe {
        return Err(HostError::Image(format!(
            "invalid software stack zero-page address ${:02x}",
            bytes[7]
        )));
    }
    let load = u16::from_le_bytes([bytes[8], bytes[9]]);
    let entry = u16::from_le_bytes([bytes[10], bytes[11]]);
    let end = usize::from(load)
        .checked_add(bytes.len() - HEADER_LEN)
        .ok_or_else(|| HostError::Image("sim65 image address overflow".into()))?;
    if end > SERVICE_START {
        return Err(HostError::Image(
            "sim65 image overlaps reserved host services at $ffef".into(),
        ));
    }
    if usize::from(entry) < usize::from(load) || usize::from(entry) >= end {
        return Err(HostError::Image(format!(
            "entry ${entry:04x} is outside image ${load:04x}..${end:04x}"
        )));
    }
    let mut memory = Memory::new();
    for (offset, byte) in bytes[HEADER_LEN..].iter().copied().enumerate() {
        memory.set_byte((usize::from(load) + offset) as u16, byte);
    }
    memory.set_byte(u16::from(bytes[7]), INITIAL_CC65_SP as u8);
    memory.set_byte(u16::from(bytes[7]) + 1, (INITIAL_CC65_SP >> 8) as u8);
    Ok(LoadedImage { memory, entry })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn image(load: u16, entry: u16, body: &[u8]) -> Vec<u8> {
        let mut bytes = b"sim65\x02\x00\xf0".to_vec();
        bytes.extend_from_slice(&load.to_le_bytes());
        bytes.extend_from_slice(&entry.to_le_bytes());
        bytes.extend_from_slice(body);
        bytes
    }

    #[test]
    fn loader_accepts_nmos_image_and_initializes_cc65_stack() {
        use mos6502::memory::Bus;
        let mut loaded = load(&image(0x0200, 0x0200, &[0x60])).unwrap();
        assert_eq!(loaded.entry, 0x0200);
        assert_eq!(loaded.memory.get_byte(0xf0), 0xef);
        assert_eq!(loaded.memory.get_byte(0xf1), 0xff);
        assert_eq!(loaded.memory.get_byte(0x0200), 0x60);
    }

    #[test]
    fn loader_rejects_truncated_bad_cpu_entry_and_reserved_range() {
        assert!(load(b"sim65").is_err());
        let mut bad_cpu = image(0x0200, 0x0200, &[0x60]);
        bad_cpu[6] = 1;
        assert!(load(&bad_cpu).is_err());
        assert!(load(&image(0x0200, 0x0201, &[0x60])).is_err());
        assert!(load(&image(0xffee, 0xffee, &[0x60, 0x60])).is_err());
        assert!(load(&image(0x0200, 0x0200, &[])).is_err());
    }

    #[test]
    fn loader_rejects_invalid_stack_zero_page_pointer() {
        let mut bytes = image(0x0200, 0x0200, &[0x60]);
        bytes[7] = 0xff;
        assert!(load(&bytes).is_err());
    }
}
