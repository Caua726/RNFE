// Mapper 069 (FME-7/Sunsoft-5B)
use super::{Mapper, CartData};
use crate::cartridge::Mirror;

pub struct Fme7 {
    command: u8,
    prg_banks: [u8; 4],
    chr_banks: [u8; 8],
}

impl Fme7 {
    pub fn new() -> Self {
        Fme7 {
            command: 0,
            prg_banks: [0; 4],
            chr_banks: [0; 8],
        }
    }
}

impl Mapper for Fme7 {
    fn cpu_read(&self, addr: u16, data: &CartData) -> Option<u8> {
        match addr {
            0x6000..=0x7FFF => {
                let bank = self.prg_banks[0] as usize & 0x3F;
                let offset = bank * 0x2000 + (addr as usize - 0x6000);
                if offset < data.prg.len() {
                    Some(data.prg[offset])
                } else {
                    Some(0)
                }
            },
            0x8000..=0x9FFF => {
                let bank = self.prg_banks[1] as usize & 0x3F;
                let offset = bank * 0x2000 + (addr as usize - 0x8000);
                Some(data.prg[offset % data.prg.len()])
            },
            0xA000..=0xBFFF => {
                let bank = self.prg_banks[2] as usize & 0x3F;
                let offset = bank * 0x2000 + (addr as usize - 0xA000);
                Some(data.prg[offset % data.prg.len()])
            },
            0xC000..=0xDFFF => {
                let bank = self.prg_banks[3] as usize & 0x3F;
                let offset = bank * 0x2000 + (addr as usize - 0xC000);
                Some(data.prg[offset % data.prg.len()])
            },
            0xE000..=0xFFFF => {
                let bank = (data.prg_banks as usize * 2).wrapping_sub(1);
                let offset = bank * 0x2000 + (addr as usize - 0xE000);
                Some(data.prg[offset % data.prg.len()])
            },
            _ => None,
        }
    }

    fn cpu_write(&mut self, addr: u16, val: u8, data: &mut CartData) -> bool {
        match addr {
            0x8000..=0x9FFF => {
                self.command = val & 0x0F;
                true
            },
            0xA000..=0xBFFF => {
                match self.command {
                    0..=7 => {
                        self.chr_banks[self.command as usize] = val;
                    },
                    8 => self.prg_banks[0] = val,
                    9 => self.prg_banks[1] = val,
                    0xA => self.prg_banks[2] = val,
                    0xB => self.prg_banks[3] = val,
                    0xC => {
                        data.mirror = match val & 0x03 {
                            0 => Mirror::Vertical,
                            1 => Mirror::Horizontal,
                            2 => Mirror::OneScreenLo,
                            3 => Mirror::OneScreenHi,
                            _ => data.mirror,
                        };
                    },
                    _ => {}
                }
                true
            },
            _ => false,
        }
    }

    fn ppu_read(&mut self, addr: u16, data: &CartData) -> Option<u8> {
        if addr <= 0x1FFF {
            let bank_index = (addr / 0x0400) as usize;
            let bank = self.chr_banks[bank_index] as usize;
            let offset = bank * 0x0400 + (addr & 0x03FF) as usize;
            if offset < data.chr.len() {
                Some(data.chr[offset])
            } else {
                Some(0)
            }
        } else {
            None
        }
    }

    fn reset(&mut self, _prg_banks: u8) {
        self.command = 0;
        self.prg_banks = [0; 4];
        self.chr_banks = [0; 8];
    }
}
