// Mapper 009 (MMC2) - Punch-Out!! (latch-based CHR switching)
use super::{Mapper, CartData};
use crate::cartridge::Mirror;

pub struct Mmc2 {
    prg_bank: u8,
    chr_banks: [u8; 4],
    latch: [u8; 2],
}

impl Mmc2 {
    pub fn new() -> Self {
        Mmc2 {
            prg_bank: 0,
            chr_banks: [0; 4],
            latch: [0xFE, 0xFE],
        }
    }
}

impl Mapper for Mmc2 {
    fn cpu_read(&self, addr: u16, data: &CartData) -> Option<u8> {
        match addr {
            0x8000..=0x9FFF => {
                let offset = self.prg_bank as usize * 0x2000 + (addr as usize - 0x8000);
                Some(data.prg[offset % data.prg.len()])
            },
            0xA000..=0xBFFF => {
                let bank = (data.prg_banks as usize * 2).wrapping_sub(3);
                let offset = bank * 0x2000 + (addr as usize - 0xA000);
                Some(data.prg[offset % data.prg.len()])
            },
            0xC000..=0xDFFF => {
                let bank = (data.prg_banks as usize * 2).wrapping_sub(2);
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
            0xA000..=0xAFFF => { self.prg_bank = val & 0x0F; true },
            0xB000..=0xBFFF => { self.chr_banks[0] = val & 0x1F; true },
            0xC000..=0xCFFF => { self.chr_banks[1] = val & 0x1F; true },
            0xD000..=0xDFFF => { self.chr_banks[2] = val & 0x1F; true },
            0xE000..=0xEFFF => { self.chr_banks[3] = val & 0x1F; true },
            0xF000..=0xFFFF => {
                data.mirror = if val & 0x01 != 0 { Mirror::Horizontal } else { Mirror::Vertical };
                true
            },
            _ => false,
        }
    }

    fn ppu_read(&mut self, addr: u16, data: &CartData) -> Option<u8> {
        if addr <= 0x1FFF {
            let bank = if addr < 0x1000 {
                if self.latch[0] == 0xFD {
                    self.chr_banks[0]
                } else {
                    self.chr_banks[1]
                }
            } else {
                if self.latch[1] == 0xFD {
                    self.chr_banks[2]
                } else {
                    self.chr_banks[3]
                }
            };
            let offset = bank as usize * 0x1000 + (addr & 0x0FFF) as usize;
            let result = if offset < data.chr.len() {
                Some(data.chr[offset])
            } else {
                Some(0)
            };
            // Atualizar latches baseado no tile lido
            match addr {
                0x0FD8 => self.latch[0] = 0xFD,
                0x0FE8 => self.latch[0] = 0xFE,
                0x1FD8..=0x1FDF => self.latch[1] = 0xFD,
                0x1FE8..=0x1FEF => self.latch[1] = 0xFE,
                _ => {}
            }
            result
        } else {
            None
        }
    }

    fn reset(&mut self, _prg_banks: u8) {
        self.prg_bank = 0;
        self.chr_banks = [0; 4];
        self.latch = [0xFE, 0xFE];
    }
}
