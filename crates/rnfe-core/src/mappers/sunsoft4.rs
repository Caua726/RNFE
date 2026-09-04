//! Mapper 068 (Sunsoft-4): CHR em 4 bancos de 2 KB, nametables opcionalmente vindas da CHR ROM
//! (`$E000` bit 4, bancos de `$C000/$D000`), PRG de 16 KB em `$8000` com PRG RAM opcional.
use super::{CartData, Mapper};
use crate::cartridge::{Mirror, NtSource};

#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[derive(Clone)]
pub struct Sunsoft4 {
    chr: [u8; 4],
    nt: [u8; 2],
    /// `$E000`: bits 0-1 mirroring, bit 4 nametables na CHR ROM.
    nt_ctrl: u8,
    prg: u8,
    ram_enabled: bool,
}

impl Sunsoft4 {
    pub fn new(_data: &CartData) -> Self {
        Sunsoft4 { chr: [0, 1, 2, 3], nt: [0, 0], nt_ctrl: 0, prg: 0, ram_enabled: false }
    }

    fn chr_nametables(&self) -> bool {
        self.nt_ctrl & 0x10 != 0
    }
}

impl Mapper for Sunsoft4 {
    #[inline]
    fn cpu_read(&self, addr: u16, data: &CartData) -> Option<u8> {
        match addr {
            0x6000..=0x7FFF => {
                if self.ram_enabled {
                    Some(data.prg_ram_at((addr & 0x1FFF) as usize))
                } else {
                    None
                }
            }
            0x8000..=0xBFFF => Some(data.prg_at(self.prg as usize * 0x4000 + (addr & 0x3FFF) as usize)),
            0xC000..=0xFFFF => Some(data.prg_at((data.prg_16k() - 1) * 0x4000 + (addr & 0x3FFF) as usize)),
            _ => None,
        }
    }

    fn cpu_write(&mut self, addr: u16, val: u8, data: &mut CartData) -> bool {
        match addr {
            0x6000..=0x7FFF => {
                if self.ram_enabled {
                    data.prg_ram_set((addr & 0x1FFF) as usize, val);
                }
            }
            0x8000..=0xBFFF => self.chr[((addr - 0x8000) >> 12) as usize] = val,
            0xC000..=0xCFFF => self.nt[0] = val | 0x80,
            0xD000..=0xDFFF => self.nt[1] = val | 0x80,
            0xE000..=0xEFFF => {
                self.nt_ctrl = val;
                data.mirror = match val & 0x03 {
                    0 => Mirror::Vertical,
                    1 => Mirror::Horizontal,
                    2 => Mirror::OneScreenLo,
                    _ => Mirror::OneScreenHi,
                };
            }
            0xF000..=0xFFFF => {
                self.prg = val & 0x0F;
                self.ram_enabled = val & 0x10 != 0;
            }
            _ => return false,
        }
        true
    }

    fn manages_prg_ram(&self) -> bool {
        true
    }

    #[inline]
    fn chr_offset(&self, addr: u16) -> usize {
        self.chr[(addr >> 11) as usize & 3] as usize * 0x0800 + (addr & 0x07FF) as usize
    }

    #[inline]
    fn nt_source(&mut self, addr: u16, data: &CartData) -> Option<NtSource> {
        if !self.chr_nametables() {
            return None;
        }
        // qual das duas nametables (A/B) o quadrante usa, pelo mirroring atual
        let q = (addr >> 10) as usize & 3;
        let which = match data.mirror {
            Mirror::Vertical => q & 1,
            Mirror::Horizontal => q >> 1,
            Mirror::OneScreenLo => 0,
            _ => 1,
        };
        Some(NtSource::Chr(self.nt[which] as usize * 0x0400 + (addr & 0x03FF) as usize))
    }

    /// Mesmo mapa para escritas (aqui `nt_source` não tem efeito colateral).
    fn nt_dest(&self, addr: u16, data: &CartData) -> Option<NtSource> {
        if !self.chr_nametables() {
            return None;
        }
        // qual das duas nametables (A/B) o quadrante usa, pelo mirroring atual
        let q = (addr >> 10) as usize & 3;
        let which = match data.mirror {
            Mirror::Vertical => q & 1,
            Mirror::Horizontal => q >> 1,
            Mirror::OneScreenLo => 0,
            _ => 1,
        };
        Some(NtSource::Chr(self.nt[which] as usize * 0x0400 + (addr & 0x03FF) as usize))
    }

    fn reset(&mut self, data: &mut CartData) {
        *self = Sunsoft4::new(data);
    }

    fn state_string(&self) -> String {
        format!(
            "  Sunsoft-4 CHR {:?} NT {:02X?} ctrl ${:02X} PRG {} RAM {}\n",
            self.chr, self.nt, self.nt_ctrl, self.prg, self.ram_enabled
        )
    }
}
