// Mapper 001 (MMC1) - PRG/CHR bank switching via serial port
use super::{Mapper, CartData};
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
    pub fn new() -> Self {
        Mmc1 {
            shift: 0x10,
            shift_count: 0,
            control: 0x0C,
            chr_bank0: 0,
            chr_bank1: 0,
            prg_bank: 0,
        }
    }
}

impl Mapper for Mmc1 {
    fn cpu_read(&self, addr: u16, data: &CartData) -> Option<u8> {
        if addr >= 0x8000 {
            let prg_mode = (self.control >> 2) & 0x03;
            let bank = match prg_mode {
                0 | 1 => {
                    let b = (self.prg_bank & 0x0E) as usize;
                    b * 0x4000 + (addr as usize - 0x8000)
                },
                2 => {
                    if addr < 0xC000 {
                        addr as usize - 0x8000
                    } else {
                        (self.prg_bank & 0x0F) as usize * 0x4000 + (addr as usize - 0xC000)
                    }
                },
                3 | _ => {
                    if addr < 0xC000 {
                        (self.prg_bank & 0x0F) as usize * 0x4000 + (addr as usize - 0x8000)
                    } else {
                        (data.prg_banks as usize - 1) * 0x4000 + (addr as usize - 0xC000)
                    }
                },
            };
            if bank < data.prg.len() {
                Some(data.prg[bank])
            } else {
                Some(0)
            }
        } else if addr >= 0x6000 {
            Some(data.prg_ram[(addr - 0x6000) as usize])
        } else {
            None
        }
    }

    fn cpu_write(&mut self, addr: u16, val: u8, data: &mut CartData) -> bool {
        if addr >= 0x6000 && addr < 0x8000 {
            data.prg_ram[(addr - 0x6000) as usize] = val;
            return true;
        }
        if addr >= 0x8000 {
            if val & 0x80 != 0 {
                self.shift = 0x10;
                self.shift_count = 0;
                self.control |= 0x0C;
            } else {
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
                                3 => Mirror::Horizontal,
                                _ => data.mirror,
                            };
                        },
                        0xA000..=0xBFFF => self.chr_bank0 = value,
                        0xC000..=0xDFFF => self.chr_bank1 = value,
                        0xE000..=0xFFFF => self.prg_bank = value & 0x0F,
                        _ => {}
                    }
                    self.shift = 0x10;
                    self.shift_count = 0;
                }
            }
            true
        } else {
            false
        }
    }

    fn ppu_read(&mut self, addr: u16, data: &CartData) -> Option<u8> {
        if addr <= 0x1FFF {
            if data.chr_banks == 0 {
                Some(data.chr[addr as usize])
            } else {
                let chr_mode = (self.control >> 4) & 0x01;
                let bank_addr = if chr_mode == 0 {
                    let b = (self.chr_bank0 & 0x1E) as usize;
                    b * 0x1000 + addr as usize
                } else {
                    if addr < 0x1000 {
                        self.chr_bank0 as usize * 0x1000 + addr as usize
                    } else {
                        self.chr_bank1 as usize * 0x1000 + (addr as usize - 0x1000)
                    }
                };
                if bank_addr < data.chr.len() {
                    Some(data.chr[bank_addr])
                } else {
                    Some(0)
                }
            }
        } else {
            None
        }
    }

    fn reset(&mut self, _prg_banks: u8) {
        self.shift = 0x10;
        self.shift_count = 0;
        self.control = 0x0C;
        self.chr_bank0 = 0;
        self.chr_bank1 = 0;
        self.prg_bank = 0;
    }

    fn print_state(&self) {
        println!("  MMC1 ctrl: ${:02X}  PRG bank: {}  CHR banks: {}/{}",
            self.control, self.prg_bank, self.chr_bank0, self.chr_bank1);
    }
}
