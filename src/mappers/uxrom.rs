// Mapper 002 (UxROM) - 16KB PRG switching, CHR RAM
use super::{Mapper, CartData};

pub struct Uxrom {
    bank: u8,
}

impl Uxrom {
    pub fn new() -> Self { Uxrom { bank: 0 } }
}

impl Mapper for Uxrom {
    fn cpu_read(&self, addr: u16, data: &CartData) -> Option<u8> {
        if addr >= 0xC000 {
            let offset = (data.prg_banks as usize - 1) * 0x4000 + (addr as usize - 0xC000);
            Some(data.prg[offset % data.prg.len()])
        } else if addr >= 0x8000 {
            let offset = self.bank as usize * 0x4000 + (addr as usize - 0x8000);
            Some(data.prg[offset % data.prg.len()])
        } else {
            None
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

    fn ppu_read(&mut self, addr: u16, data: &CartData) -> Option<u8> {
        if addr <= 0x1FFF {
            Some(data.chr[addr as usize])
        } else {
            None
        }
    }

    fn reset(&mut self, _prg_banks: u8) { self.bank = 0; }
}
