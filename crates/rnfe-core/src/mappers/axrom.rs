//! Mapper 007 (AxROM): 32 KB de PRG comutáveis + mirroring de uma tela escolhido pelo jogo.
use super::{CartData, Mapper};
use crate::cartridge::Mirror;

#[derive(Default)]
pub struct Axrom {
    prg_bank: u8,
}

impl Axrom {
    pub fn new() -> Self {
        Axrom::default()
    }
}

impl Mapper for Axrom {
    #[inline]
    fn cpu_read(&self, addr: u16, data: &CartData) -> Option<u8> {
        if addr >= 0x8000 {
            Some(data.prg_at(self.prg_bank as usize * 0x8000 + (addr & 0x7FFF) as usize))
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

    fn reset(&mut self, data: &mut CartData) {
        self.prg_bank = 0;
        data.mirror = Mirror::OneScreenLo;
    }

    fn state_string(&self) -> String {
        format!("  AxROM PRG bank: {}\n", self.prg_bank)
    }
}
