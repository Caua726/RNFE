//! Mapper 069 (FME-7 / Sunsoft 5B): 4 bancos de PRG de 8 KB, 8 de CHR de 1 KB, IRQ por contador.
use super::{CartData, Mapper};
use crate::cartridge::Mirror;

pub struct Fme7 {
    command: u8,
    prg_banks: [u8; 4],
    chr_banks: [u8; 8],
}

impl Fme7 {
    pub fn new() -> Self {
        Fme7 { command: 0, prg_banks: [0; 4], chr_banks: [0; 8] }
    }
}

impl Default for Fme7 {
    fn default() -> Self {
        Self::new()
    }
}

impl Mapper for Fme7 {
    #[inline]
    fn cpu_read(&self, addr: u16, data: &CartData) -> Option<u8> {
        let bank = match addr {
            0x6000..=0x7FFF => {
                let b = self.prg_banks[0];
                if b & 0x40 != 0 {
                    // RAM (bit 7 = habilitada)
                    return if b & 0x80 != 0 {
                        Some(data.prg_ram_at((addr & 0x1FFF) as usize))
                    } else {
                        None
                    };
                }
                (b & 0x3F) as usize
            }
            0x8000..=0x9FFF => (self.prg_banks[1] & 0x3F) as usize,
            0xA000..=0xBFFF => (self.prg_banks[2] & 0x3F) as usize,
            0xC000..=0xDFFF => (self.prg_banks[3] & 0x3F) as usize,
            0xE000..=0xFFFF => data.prg_8k() - 1,
            _ => return None,
        };
        Some(data.prg_at(bank * 0x2000 + (addr & 0x1FFF) as usize))
    }

    fn cpu_write(&mut self, addr: u16, val: u8, data: &mut CartData) -> bool {
        match addr {
            0x6000..=0x7FFF => {
                if self.prg_banks[0] & 0xC0 == 0xC0 {
                    data.prg_ram_set((addr & 0x1FFF) as usize, val);
                }
                true
            }
            0x8000..=0x9FFF => {
                self.command = val & 0x0F;
                true
            }
            0xA000..=0xBFFF => {
                match self.command {
                    c @ 0..=7 => self.chr_banks[c as usize] = val,
                    8 => self.prg_banks[0] = val,
                    9 => self.prg_banks[1] = val,
                    0xA => self.prg_banks[2] = val,
                    0xB => self.prg_banks[3] = val,
                    0xC => {
                        data.mirror = match val & 0x03 {
                            0 => Mirror::Vertical,
                            1 => Mirror::Horizontal,
                            2 => Mirror::OneScreenLo,
                            _ => Mirror::OneScreenHi,
                        };
                    }
                    _ => {} // IRQ: F3-04
                }
                true
            }
            _ => false,
        }
    }

    #[inline]
    fn chr_offset(&self, addr: u16) -> usize {
        self.chr_banks[(addr >> 10) as usize & 7] as usize * 0x0400 + (addr & 0x03FF) as usize
    }

    fn reset(&mut self, _data: &mut CartData) {
        self.command = 0;
        self.prg_banks = [0; 4];
        self.chr_banks = [0; 8];
    }

    fn state_string(&self) -> String {
        format!("  FME-7 cmd: {}  PRG: {:?}  CHR: {:?}\n", self.command, self.prg_banks, self.chr_banks)
    }
}
