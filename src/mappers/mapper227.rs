// Mapper 227 (multicart chinês) - encoding FCEUX
// A0: mirroring
// A2-A6 + A8: PRG bank (6 bits)
// A7: mode (0=32K, 1=16K)
// A9: L flag
use super::{Mapper, CartData};
use crate::cartridge::Mirror;

pub struct Mapper227 {
    reg: u16,
    fixed_bank: usize,
}

impl Mapper227 {
    pub fn new() -> Self {
        Mapper227 { reg: 0, fixed_bank: 0 }
    }
}

impl Mapper for Mapper227 {
    fn cpu_read(&self, addr: u16, data: &CartData) -> Option<u8> {
        if addr >= 0x8000 {
            let reg = self.reg;
            let p = (((reg >> 2) & 0x1F) | ((reg & 0x100) >> 3)) as usize;
            let mode_32k = (reg >> 7) & 1;

            let offset = if mode_32k != 0 {
                let base = (p & 0x3E) * 0x4000;
                base + (addr as usize - 0x8000)
            } else {
                if addr >= 0xC000 {
                    self.fixed_bank * 0x4000 + ((addr as usize - 0xC000) & 0x3FFF)
                } else {
                    p * 0x4000 + (addr as usize - 0x8000)
                }
            };

            Some(data.prg[offset % data.prg.len()])
        } else {
            None
        }
    }

    fn cpu_write(&mut self, addr: u16, _val: u8, data: &mut CartData) -> bool {
        if addr >= 0x8000 {
            self.reg = addr;
            data.mirror = if addr & 0x01 != 0 { Mirror::Horizontal } else { Mirror::Vertical };
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

    fn reset(&mut self, _prg_banks: u8) {
        self.reg = 0;
        self.fixed_bank = 0;
    }
}
