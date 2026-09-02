// Mapper 004 (MMC3) - PRG/CHR bank switching + scanline IRQ
use super::{Mapper, CartData};
use crate::cartridge::Mirror;

pub struct Mmc3 {
    bank_select: u8,
    prg_banks: [u8; 4],
    chr_banks: [u8; 8],
    irq_counter: u8,
    irq_reload: u8,
    irq_enabled: bool,
    irq_pending: bool,
}

impl Mmc3 {
    pub fn new(prg_banks: u8) -> Self {
        Mmc3 {
            bank_select: 0,
            prg_banks: [0, 1, (prg_banks * 2).wrapping_sub(2), (prg_banks * 2).wrapping_sub(1)],
            chr_banks: [0, 1, 2, 3, 4, 5, 6, 7],
            irq_counter: 0,
            irq_reload: 0,
            irq_enabled: false,
            irq_pending: false,
        }
    }
}

impl Mapper for Mmc3 {
    fn cpu_read(&self, addr: u16, data: &CartData) -> Option<u8> {
        if addr >= 0x6000 && addr < 0x8000 {
            return Some(data.prg_ram[(addr - 0x6000) as usize]);
        }
        if addr >= 0x8000 {
            let bank = match addr {
                0x8000..=0x9FFF => self.prg_banks[0],
                0xA000..=0xBFFF => self.prg_banks[1],
                0xC000..=0xDFFF => self.prg_banks[2],
                0xE000..=0xFFFF => self.prg_banks[3],
                _ => 0
            };
            let bank_offset = (bank as usize) * 0x2000;
            let addr_offset = (addr & 0x1FFF) as usize;
            if bank_offset + addr_offset < data.prg.len() {
                Some(data.prg[bank_offset + addr_offset])
            } else {
                Some(0)
            }
        } else {
            None
        }
    }

    fn cpu_write(&mut self, addr: u16, val: u8, data: &mut CartData) -> bool {
        if addr >= 0x6000 && addr < 0x8000 {
            data.prg_ram[(addr - 0x6000) as usize] = val;
            return true;
        }
        match addr {
            0x8000..=0x9FFF => {
                if addr % 2 == 0 {
                    self.bank_select = val;
                } else {
                    let bank_register = self.bank_select & 0x07;
                    match bank_register {
                        0 | 1 => {
                            self.chr_banks[bank_register as usize * 2] = val & 0xFE;
                            self.chr_banks[bank_register as usize * 2 + 1] = (val & 0xFE) + 1;
                        },
                        2..=5 => {
                            self.chr_banks[bank_register as usize + 2] = val;
                        },
                        6 => {
                            self.prg_banks[if (self.bank_select & 0x40) != 0 { 2 } else { 0 }] = val;
                        },
                        7 => {
                            self.prg_banks[1] = val;
                        },
                        _ => {}
                    }
                }
                let last = (data.prg_banks * 2).wrapping_sub(1);
                let second_last = (data.prg_banks * 2).wrapping_sub(2);
                if (self.bank_select & 0x40) != 0 {
                    self.prg_banks[0] = second_last;
                } else {
                    self.prg_banks[2] = second_last;
                }
                self.prg_banks[3] = last;
                true
            },
            0xA000..=0xBFFF => {
                if addr % 2 == 0 {
                    data.mirror = if val & 0x01 != 0 { Mirror::Horizontal } else { Mirror::Vertical };
                }
                true
            },
            0xC000..=0xDFFF => {
                if addr % 2 == 0 {
                    self.irq_reload = val;
                } else {
                    self.irq_counter = 0;
                }
                true
            },
            0xE000..=0xFFFF => {
                if addr % 2 == 0 {
                    self.irq_enabled = false;
                    self.irq_pending = false;
                } else {
                    self.irq_enabled = true;
                }
                true
            },
            _ => false,
        }
    }

    fn ppu_read(&mut self, addr: u16, data: &CartData) -> Option<u8> {
        if addr <= 0x1FFF {
            let chr_mode = (self.bank_select & 0x80) != 0;
            let bank = if chr_mode {
                match addr {
                    0x0000..=0x03FF => self.chr_banks[4],
                    0x0400..=0x07FF => self.chr_banks[5],
                    0x0800..=0x0BFF => self.chr_banks[6],
                    0x0C00..=0x0FFF => self.chr_banks[7],
                    0x1000..=0x13FF => self.chr_banks[0],
                    0x1400..=0x17FF => self.chr_banks[1],
                    0x1800..=0x1BFF => self.chr_banks[2],
                    0x1C00..=0x1FFF => self.chr_banks[3],
                    _ => 0,
                }
            } else {
                match addr {
                    0x0000..=0x03FF => self.chr_banks[0],
                    0x0400..=0x07FF => self.chr_banks[1],
                    0x0800..=0x0BFF => self.chr_banks[2],
                    0x0C00..=0x0FFF => self.chr_banks[3],
                    0x1000..=0x13FF => self.chr_banks[4],
                    0x1400..=0x17FF => self.chr_banks[5],
                    0x1800..=0x1BFF => self.chr_banks[6],
                    0x1C00..=0x1FFF => self.chr_banks[7],
                    _ => 0,
                }
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

    fn clock_scanline(&mut self) {
        if self.irq_counter == 0 {
            self.irq_counter = self.irq_reload;
        } else {
            self.irq_counter -= 1;
        }
        if self.irq_counter == 0 && self.irq_enabled {
            self.irq_pending = true;
        }
    }

    fn mapper_irq(&mut self) -> bool {
        let pending = self.irq_pending;
        self.irq_pending = false;
        pending
    }

    fn reset(&mut self, prg_banks: u8) {
        self.bank_select = 0;
        self.prg_banks = [0, 1, (prg_banks * 2).wrapping_sub(2), (prg_banks * 2).wrapping_sub(1)];
        self.chr_banks = [0, 1, 2, 3, 4, 5, 6, 7];
        self.irq_counter = 0;
        self.irq_reload = 0;
        self.irq_enabled = false;
        self.irq_pending = false;
    }

    fn print_state(&self) {
        println!("  MMC3 bank_select: ${:02X} (CHR_A12_inv={} PRG_mode={})",
            self.bank_select,
            if self.bank_select & 0x80 != 0 { "yes" } else { "no" },
            if self.bank_select & 0x40 != 0 { "swap" } else { "normal" });
        println!("  MMC3 PRG banks: [{}, {}, {}, {}]",
            self.prg_banks[0], self.prg_banks[1], self.prg_banks[2], self.prg_banks[3]);
        println!("  MMC3 CHR banks: [{}, {}, {}, {}, {}, {}, {}, {}]",
            self.chr_banks[0], self.chr_banks[1], self.chr_banks[2], self.chr_banks[3],
            self.chr_banks[4], self.chr_banks[5], self.chr_banks[6], self.chr_banks[7]);
        println!("  MMC3 IRQ: counter={} reload={} enabled={} pending={}",
            self.irq_counter, self.irq_reload, self.irq_enabled, self.irq_pending);
    }
}
