// Mapper 206 (DxROM) - MMC3 simplificado, sem IRQ
use super::{Mapper, CartData};

pub struct Dxrom {
    bank_select: u8,
    prg_banks: [u8; 4],
    chr_banks: [u8; 8],
}

impl Dxrom {
    pub fn new(prg_banks: u8) -> Self {
        Dxrom {
            bank_select: 0,
            prg_banks: [0, 1, (prg_banks * 2).wrapping_sub(2), (prg_banks * 2).wrapping_sub(1)],
            chr_banks: [0, 1, 2, 3, 4, 5, 6, 7],
        }
    }
}

impl Mapper for Dxrom {
    fn cpu_read(&self, addr: u16, data: &CartData) -> Option<u8> {
        if addr >= 0x8000 {
            let bank = match addr {
                0x8000..=0x9FFF => self.prg_banks[0],
                0xA000..=0xBFFF => self.prg_banks[1],
                0xC000..=0xDFFF => self.prg_banks[2],
                0xE000..=0xFFFF => self.prg_banks[3],
                _ => 0,
            };
            let offset = bank as usize * 0x2000 + (addr & 0x1FFF) as usize;
            if offset < data.prg.len() {
                Some(data.prg[offset])
            } else {
                Some(0)
            }
        } else {
            None
        }
    }

    fn cpu_write(&mut self, addr: u16, val: u8, data: &mut CartData) -> bool {
        match addr {
            0x8000..=0x9FFF => {
                if addr % 2 == 0 {
                    self.bank_select = val & 0x07;
                } else {
                    let reg = self.bank_select & 0x07;
                    match reg {
                        0 | 1 => {
                            self.chr_banks[reg as usize * 2] = val & 0xFE;
                            self.chr_banks[reg as usize * 2 + 1] = (val & 0xFE) + 1;
                        },
                        2..=5 => {
                            self.chr_banks[reg as usize + 2] = val;
                        },
                        6 => {
                            self.prg_banks[0] = val;
                        },
                        7 => {
                            self.prg_banks[1] = val;
                        },
                        _ => {}
                    }
                    self.prg_banks[2] = (data.prg_banks * 2).wrapping_sub(2);
                    self.prg_banks[3] = (data.prg_banks * 2).wrapping_sub(1);
                }
                true
            },
            _ => false,
        }
    }

    fn ppu_read(&mut self, addr: u16, data: &CartData) -> Option<u8> {
        if addr <= 0x1FFF {
            let bank = match addr {
                0x0000..=0x03FF => self.chr_banks[0],
                0x0400..=0x07FF => self.chr_banks[1],
                0x0800..=0x0BFF => self.chr_banks[2],
                0x0C00..=0x0FFF => self.chr_banks[3],
                0x1000..=0x13FF => self.chr_banks[4],
                0x1400..=0x17FF => self.chr_banks[5],
                0x1800..=0x1BFF => self.chr_banks[6],
                0x1C00..=0x1FFF => self.chr_banks[7],
                _ => 0,
            };
            let offset = bank as usize * 0x0400 + (addr & 0x03FF) as usize;
            if offset < data.chr.len() {
                Some(data.chr[offset])
            } else {
                Some(0)
            }
        } else {
            None
        }
    }

    fn reset(&mut self, prg_banks: u8) {
        self.bank_select = 0;
        self.prg_banks = [0, 1, (prg_banks * 2).wrapping_sub(2), (prg_banks * 2).wrapping_sub(1)];
        self.chr_banks = [0, 1, 2, 3, 4, 5, 6, 7];
    }
}
