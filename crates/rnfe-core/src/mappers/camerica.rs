//! Mapper 071 (Camerica/Codemasters): 16 KB comutáveis em `$8000` (registrador em `$C000-$FFFF`),
//! último banco fixo em `$C000`.
use super::{CartData, Mapper};
use crate::cartridge::Mirror;

#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[derive(Clone, Default)]
pub struct Camerica {
    prg_bank: u8,
}

impl Camerica {
    pub fn new() -> Self {
        Camerica::default()
    }
}

impl Mapper for Camerica {
    #[inline]
    fn cpu_read(&self, addr: u16, data: &CartData) -> Option<u8> {
        match addr {
            0xC000..=0xFFFF => Some(data.prg_at((data.prg_16k() - 1) * 0x4000 + (addr & 0x3FFF) as usize)),
            0x8000..=0xBFFF => Some(data.prg_at(self.prg_bank as usize * 0x4000 + (addr & 0x3FFF) as usize)),
            _ => None,
        }
    }

    fn cpu_write(&mut self, addr: u16, val: u8, data: &mut CartData) -> bool {
        match addr {
            // BF9097 (Fire Hawk): mirroring de uma tela pelo bit 4
            0x9000..=0x9FFF => {
                data.mirror = if val & 0x10 != 0 { Mirror::OneScreenHi } else { Mirror::OneScreenLo };
                true
            }
            0xC000..=0xFFFF => {
                self.prg_bank = val & 0x0F;
                true
            }
            _ => false,
        }
    }

    fn reset(&mut self, _data: &mut CartData) {
        self.prg_bank = 0;
    }
}
