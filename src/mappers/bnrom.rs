// Mapper 034 (BNROM) - 32KB PRG switching, CHR RAM
use super::{Mapper, CartData};

pub struct Bnrom {
    prg_bank: u8,
}

impl Bnrom {
    pub fn new() -> Self { Bnrom { prg_bank: 0 } }
}

impl Mapper for Bnrom {
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
            self.prg_bank = val & 0x03;
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

    fn reset(&mut self, _prg_banks: u8) { self.prg_bank = 0; }
}
