//! Mapper 001 (MMC1): registradores carregados bit a bit por uma porta serial.
use super::{CartData, Mapper};
use crate::cartridge::Mirror;

pub struct Mmc1 {
    shift: u8,
    shift_count: u8,
    control: u8,
    chr_bank0: u8,
    chr_bank1: u8,
    prg_bank: u8,
}

impl Mmc1 {
    pub fn new(_data: &CartData) -> Self {
        Mmc1 { shift: 0x10, shift_count: 0, control: 0x0C, chr_bank0: 0, chr_bank1: 0, prg_bank: 0 }
    }
}

impl Mapper for Mmc1 {
    #[inline]
    fn cpu_read(&self, addr: u16, data: &CartData) -> Option<u8> {
        if addr < 0x8000 {
            return None;
        }
        let offset = match (self.control >> 2) & 0x03 {
            0 | 1 => (self.prg_bank & 0x0E) as usize * 0x4000 + (addr & 0x7FFF) as usize,
            2 => {
                if addr < 0xC000 {
                    (addr & 0x3FFF) as usize
                } else {
                    (self.prg_bank & 0x0F) as usize * 0x4000 + (addr & 0x3FFF) as usize
                }
            }
            _ => {
                if addr < 0xC000 {
                    (self.prg_bank & 0x0F) as usize * 0x4000 + (addr & 0x3FFF) as usize
                } else {
                    (data.prg_16k() - 1) * 0x4000 + (addr & 0x3FFF) as usize
                }
            }
        };
        Some(data.prg_at(offset))
    }

    fn cpu_write(&mut self, addr: u16, val: u8, data: &mut CartData) -> bool {
        if addr < 0x8000 {
            return false;
        }
        if val & 0x80 != 0 {
            self.shift = 0x10;
            self.shift_count = 0;
            self.control |= 0x0C;
            return true;
        }
        self.shift >>= 1;
        self.shift |= (val & 0x01) << 4;
        self.shift_count += 1;
        if self.shift_count == 5 {
            let value = self.shift;
            match addr {
                0x8000..=0x9FFF => {
                    self.control = value;
                    data.mirror = match value & 0x03 {
                        0 => Mirror::OneScreenLo,
                        1 => Mirror::OneScreenHi,
                        2 => Mirror::Vertical,
                        _ => Mirror::Horizontal,
                    };
                }
                0xA000..=0xBFFF => self.chr_bank0 = value,
                0xC000..=0xDFFF => self.chr_bank1 = value,
                _ => self.prg_bank = value & 0x0F,
            }
            self.shift = 0x10;
            self.shift_count = 0;
        }
        true
    }

    #[inline]
    fn chr_offset(&self, addr: u16) -> usize {
        if self.control & 0x10 == 0 {
            (self.chr_bank0 & 0x1E) as usize * 0x1000 + addr as usize
        } else if addr < 0x1000 {
            self.chr_bank0 as usize * 0x1000 + addr as usize
        } else {
            self.chr_bank1 as usize * 0x1000 + (addr & 0x0FFF) as usize
        }
    }

    fn reset(&mut self, _data: &mut CartData) {
        self.shift = 0x10;
        self.shift_count = 0;
        self.control = 0x0C;
        self.chr_bank0 = 0;
        self.chr_bank1 = 0;
        self.prg_bank = 0;
    }

    fn state_string(&self) -> String {
        format!(
            "  MMC1 ctrl: ${:02X}  PRG bank: {}  CHR banks: {}/{}\n",
            self.control, self.prg_bank, self.chr_bank0, self.chr_bank1
        )
    }
}
