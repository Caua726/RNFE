//! Mapper 003 (CNROM): PRG fixo, CHR ROM comutável em blocos de 8 KB.
use super::{CartData, Mapper};

#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[derive(Clone, Default)]
pub struct Cnrom {
    chr_bank: u8,
}

impl Cnrom {
    pub fn new() -> Self {
        Cnrom::default()
    }
}

impl Mapper for Cnrom {
    #[inline]
    fn cpu_read(&self, addr: u16, data: &CartData) -> Option<u8> {
        if addr >= 0x8000 { Some(data.prg_at((addr - 0x8000) as usize)) } else { None }
    }

    fn cpu_write(&mut self, addr: u16, val: u8, _data: &mut CartData) -> bool {
        if addr >= 0x8000 {
            self.chr_bank = val; // a máscara do cartucho já limita (há ROMs com 64-128 KB)
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
        self.chr_bank = 0;
    }

    fn state_string(&self) -> String {
        format!("  CNROM CHR bank: {}\n", self.chr_bank)
    }
}
