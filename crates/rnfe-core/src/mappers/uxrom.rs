//! Mapper 002 (UxROM): 16 KB comutáveis em `$8000`, último banco fixo em `$C000`, CHR RAM.
use super::{CartData, Mapper};

#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[derive(Clone, Default)]
pub struct Uxrom {
    bank: u8,
}

impl Uxrom {
    pub fn new() -> Self {
        Uxrom::default()
    }
}

impl Mapper for Uxrom {
    #[inline]
    fn prg_offset(&self, addr: u16, data: &CartData) -> Option<usize> {
        match addr {
            0xC000..=0xFFFF => Some((data.prg_16k() - 1) * 0x4000 + (addr & 0x3FFF) as usize),
            0x8000..=0xBFFF => Some(self.bank as usize * 0x4000 + (addr & 0x3FFF) as usize),
            _ => None,
        }
    }

    fn cpu_read(&self, addr: u16, data: &CartData) -> Option<u8> {
        match addr {
            0xC000..=0xFFFF => Some(data.prg_at((data.prg_16k() - 1) * 0x4000 + (addr & 0x3FFF) as usize)),
            0x8000..=0xBFFF => Some(data.prg_at(self.bank as usize * 0x4000 + (addr & 0x3FFF) as usize)),
            _ => None,
        }
    }

    fn cpu_write(&mut self, addr: u16, val: u8, _data: &mut CartData) -> bool {
        if addr >= 0x8000 {
            self.bank = val & 0x0F;
            true
        } else {
            false
        }
    }

    fn reset(&mut self, _data: &mut CartData) {
        self.bank = 0;
    }

    fn state_string(&self) -> String {
        format!("  UxROM bank: {}\n", self.bank)
    }
}
