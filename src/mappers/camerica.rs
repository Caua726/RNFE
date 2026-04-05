// Mapper 071 (Camerica) - 16KB PRG switch at $8000, last bank fixed at $C000
use super::{Mapper, CartData};

pub struct Camerica {
    prg_bank: u8,
}

impl Camerica {
    pub fn new() -> Self { Camerica { prg_bank: 0 } }
}

impl Mapper for Camerica {
    fn cpu_read(&self, addr: u16, data: &CartData) -> Option<u8> {
        if addr >= 0xC000 {
            let bank = (data.prg_banks as usize).wrapping_sub(1);
            let offset = bank * 0x4000 + (addr as usize - 0xC000);
            Some(data.prg[offset % data.prg.len()])
        } else if addr >= 0x8000 {
            let offset = self.prg_bank as usize * 0x4000 + (addr as usize - 0x8000);
            Some(data.prg[offset % data.prg.len()])
        } else {
            None
        }
    }

    fn cpu_write(&mut self, addr: u16, val: u8, _data: &mut CartData) -> bool {
        match addr {
            0xC000..=0xFFFF => {
                self.prg_bank = val & 0x0F;
                true
            },
            _ => false,
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
