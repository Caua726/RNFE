//! Mapper 011 (Color Dreams): bits 0-1 = banco de PRG (32 KB), bits 4-5 = banco de CHR (8 KB).
use super::{CartData, Mapper};

#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[derive(Clone, Default)]
pub struct ColorDreams {
    prg_bank: u8,
    chr_bank: u8,
}

impl ColorDreams {
    pub fn new() -> Self {
        ColorDreams::default()
    }
}

impl Mapper for ColorDreams {
    #[inline]
    fn prg_offset(&self, addr: u16, _data: &CartData) -> Option<usize> {
        (addr >= 0x8000).then_some(self.prg_bank as usize * 0x8000 + (addr & 0x7FFF) as usize)
    }

    fn cpu_read(&self, addr: u16, data: &CartData) -> Option<u8> {
        if addr >= 0x8000 {
            Some(data.prg_at(self.prg_bank as usize * 0x8000 + (addr & 0x7FFF) as usize))
        } else {
            None
        }
    }

    fn cpu_write(&mut self, addr: u16, val: u8, _data: &mut CartData) -> bool {
        if addr >= 0x8000 {
            self.prg_bank = val & 0x03;
            self.chr_bank = (val >> 4) & 0x0F;
            true
        } else {
            false
        }
    }

    #[inline]
    fn chr_offset(&self, addr: u16) -> usize {
        self.chr_bank as usize * 0x2000 + addr as usize
    }

    fn reset(&mut self, _data: &mut CartData) {
        self.prg_bank = 0;
        self.chr_bank = 0;
    }
}
