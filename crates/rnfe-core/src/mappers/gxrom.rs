// Mapper 066 (GxROM) - bits 4-5 = PRG bank, bits 0-1 = CHR bank
use super::{Mapper, CartData};

pub struct Gxrom {
    prg_bank: u8,
    chr_bank: u8,
}

impl Gxrom {
    pub fn new() -> Self { Gxrom { prg_bank: 0, chr_bank: 0 } }
}

impl Mapper for Gxrom {
    fn cpu_read(&self, addr: u16, data: &CartData) -> Option<u8> {
        if addr >= 0x8000 {
            let offset = self.prg_bank as usize * 0x8000 + (addr as usize - 0x8000);
            Some(data.prg[offset % data.prg.len()])
        } else {
            None
        }
    }

    fn cpu_write(&mut self, addr: u16, val: u8, _data: &mut CartData) -> bool {
        if addr >= 0x8000 {
            self.prg_bank = (val >> 4) & 0x03;
            self.chr_bank = val & 0x03;
            true
        } else {
            false
        }
    }

    fn ppu_read(&mut self, addr: u16, data: &CartData) -> Option<u8> {
        if addr <= 0x1FFF {
            let offset = self.chr_bank as usize * 0x2000 + addr as usize;
            Some(data.chr[offset % data.chr.len()])
        } else {
            None
        }
    }

    fn reset(&mut self, _prg_banks: u8) { self.prg_bank = 0; self.chr_bank = 0; }
}
