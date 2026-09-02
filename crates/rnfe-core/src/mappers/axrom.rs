// Mapper 007 (AxROM) - 32KB PRG switching + single screen mirroring
use super::{Mapper, CartData};
use crate::cartridge::Mirror;

pub struct Axrom {
    prg_bank: u8,
}

impl Axrom {
    pub fn new() -> Self { Axrom { prg_bank: 0 } }
}

impl Mapper for Axrom {
    fn cpu_read(&self, addr: u16, data: &CartData) -> Option<u8> {
        if addr >= 0x8000 {
            let offset = self.prg_bank as usize * 0x8000 + (addr as usize - 0x8000);
            Some(data.prg[offset % data.prg.len()])
        } else {
            None
        }
    }

    fn cpu_write(&mut self, addr: u16, val: u8, data: &mut CartData) -> bool {
        if addr >= 0x8000 {
            self.prg_bank = val & 0x07;
            data.mirror = if val & 0x10 != 0 { Mirror::OneScreenHi } else { Mirror::OneScreenLo };
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
