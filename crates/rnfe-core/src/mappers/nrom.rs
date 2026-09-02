// Mapper 000 (NROM) - sem bank switching
use super::{Mapper, CartData};

pub struct Nrom;

impl Mapper for Nrom {
    fn cpu_read(&self, addr: u16, data: &CartData) -> Option<u8> {
        if addr >= 0x8000 {
            let masked = addr & if data.prg_banks > 1 { 0x7FFF } else { 0x3FFF };
            let index = (masked & 0x3FFF) as usize;
            Some(data.prg[index])
        } else {
            None
        }
    }

    fn cpu_write(&mut self, _addr: u16, _val: u8, _data: &mut CartData) -> bool {
        false
    }

    fn ppu_read(&mut self, addr: u16, data: &CartData) -> Option<u8> {
        if addr <= 0x1FFF {
            Some(data.chr[addr as usize])
        } else {
            None
        }
    }

    fn reset(&mut self, _prg_banks: u8) {}
}
