//! Mapper 227 (multicart chinês; codificação do FCEUX).
//! A0: mirroring · A2-A6 + A8: banco de PRG (6 bits) · A7: modo (0 = 32 KB, 1 = 16 KB) · A9: flag L.
use super::{CartData, Mapper};
use crate::cartridge::Mirror;

#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[derive(Clone, Default)]
pub struct Mapper227 {
    reg: u16,
}

impl Mapper227 {
    pub fn new() -> Self {
        Mapper227::default()
    }
}

impl Mapper for Mapper227 {
    #[inline]
    fn cpu_read(&self, addr: u16, data: &CartData) -> Option<u8> {
        if addr < 0x8000 {
            return None;
        }
        let reg = self.reg;
        let p = (((reg >> 2) & 0x1F) | ((reg & 0x100) >> 3)) as usize;
        let mode_32k = (reg >> 7) & 1 != 0;
        let l = (reg >> 9) & 1 != 0;
        let offset = if mode_32k {
            (p & 0x3E) * 0x4000 + (addr & 0x7FFF) as usize
        } else if addr >= 0xC000 {
            // banco fixo: o mesmo banco (L=1) ou o último dos 8 do bloco (L=0)
            let fixed = if l { p } else { (p & 0x38) | 7 };
            fixed * 0x4000 + (addr & 0x3FFF) as usize
        } else {
            p * 0x4000 + (addr & 0x3FFF) as usize
        };
        Some(data.prg_at(offset))
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

    fn reset(&mut self, _data: &mut CartData) {
        self.reg = 0;
    }
}
