//! Mapper 034 (BNROM): 32 KB de PRG comutáveis, CHR RAM.
use super::{CartData, Mapper};

#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[derive(Clone, Default)]
pub struct Bnrom {
    prg_bank: u8,
}

impl Bnrom {
    pub fn new() -> Self {
        Bnrom::default()
    }
}

impl Mapper for Bnrom {
    #[inline]
    fn cpu_read(&self, addr: u16, data: &CartData) -> Option<u8> {
        if addr >= 0x8000 {
            Some(data.prg_at(self.prg_bank as usize * 0x8000 + (addr & 0x7FFF) as usize))
        } else {
            None
        }
    }

    fn cpu_write(&mut self, addr: u16, val: u8, _data: &mut CartData) -> bool {
        if addr >= 0x8000 {
            self.prg_bank = val; // 256 KB existem (a máscara do cartucho limita)
            true
        } else {
            false
        }
    }

    fn reset(&mut self, _data: &mut CartData) {
        self.prg_bank = 0;
    }
}
