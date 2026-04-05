// Mapper 003 (CNROM) - 8KB CHR bank switching
use super::{Mapper, CartData};

pub struct Cnrom {
    chr_bank: u8,
}

impl Cnrom {
    pub fn new() -> Self { Cnrom { chr_bank: 0 } }
}

impl Mapper for Cnrom {
    fn cpu_read(&self, addr: u16, data: &CartData) -> Option<u8> {
        if addr >= 0x8000 {
            let masked = addr & if data.prg_banks > 1 { 0x7FFF } else { 0x3FFF };
            Some(data.prg[masked as usize % data.prg.len()])
        } else {
            None
        }
    }

    fn cpu_write(&mut self, addr: u16, val: u8, _data: &mut CartData) -> bool {
        if addr >= 0x8000 {
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

    fn reset(&mut self, _prg_banks: u8) { self.chr_bank = 0; }
}
