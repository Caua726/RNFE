//! Mapper 000 (NROM): sem bank switching. NROM-128 (16 KB) espelha em `$C000` via a máscara.
use super::{CartData, Mapper};

#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[derive(Clone)]
pub struct Nrom;

impl Mapper for Nrom {
    #[inline]
    fn cpu_read(&self, addr: u16, data: &CartData) -> Option<u8> {
        if addr >= 0x8000 { Some(data.prg_at((addr - 0x8000) as usize)) } else { None }
    }

    fn cpu_write(&mut self, _addr: u16, _val: u8, _data: &mut CartData) -> bool {
        false
    }

    fn reset(&mut self, _data: &mut CartData) {}
}
