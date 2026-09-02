// Mapper 000 (NROM) - sem bank switching.
// NROM-128: 16 KB espelhados em $8000-$FFFF. NROM-256: 32 KB lineares.
// $6000-$7FFF: PRG RAM (Family Basic e a maioria das ROMs de teste escrevem resultado aqui).
use super::{CartData, Mapper};

pub struct Nrom;

impl Mapper for Nrom {
    fn cpu_read(&self, addr: u16, data: &CartData) -> Option<u8> {
        match addr {
            0x6000..=0x7FFF => Some(data.prg_ram[(addr - 0x6000) as usize]),
            0x8000..=0xFFFF => {
                let mask = if data.prg_banks > 1 { 0x7FFF } else { 0x3FFF };
                Some(data.prg[(addr & mask) as usize])
            }
            _ => None,
        }
    }

    fn cpu_write(&mut self, addr: u16, val: u8, data: &mut CartData) -> bool {
        if (0x6000..=0x7FFF).contains(&addr) {
            data.prg_ram[(addr - 0x6000) as usize] = val;
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

    fn reset(&mut self, _prg_banks: u8) {}
}
