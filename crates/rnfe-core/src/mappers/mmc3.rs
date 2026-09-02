//! Mapper 004 (MMC3): 8 registradores de banco + contador de scanline clockado por A12.
use super::{CartData, Mapper};
use crate::cartridge::Mirror;

pub struct Mmc3 {
    bank_select: u8,
    regs: [u8; 8],
    irq_counter: u8,
    irq_reload: u8,
    irq_enabled: bool,
    irq_pending: bool,
}

impl Mmc3 {
    pub fn new(_data: &CartData) -> Self {
        Mmc3 {
            bank_select: 0,
            regs: [0, 2, 4, 5, 6, 7, 0, 1],
            irq_counter: 0,
            irq_reload: 0,
            irq_enabled: false,
            irq_pending: false,
        }
    }
}

impl Mapper for Mmc3 {
    #[inline]
    fn cpu_read(&self, addr: u16, data: &CartData) -> Option<u8> {
        if addr < 0x8000 {
            return None;
        }
        let swap = self.bank_select & 0x40 != 0;
        let last = data.prg_8k() - 1;
        let bank = match (addr >> 13) & 3 {
            0 => {
                if swap {
                    last - 1
                } else {
                    self.regs[6] as usize
                }
            }
            1 => self.regs[7] as usize,
            2 => {
                if swap {
                    self.regs[6] as usize
                } else {
                    last - 1
                }
            }
            _ => last,
        };
        Some(data.prg_at(bank * 0x2000 + (addr & 0x1FFF) as usize))
    }

    fn cpu_write(&mut self, addr: u16, val: u8, data: &mut CartData) -> bool {
        if addr < 0x8000 {
            return false;
        }
        match (addr & 0xE001, addr & 1) {
            (0x8000, _) => self.bank_select = val,
            (0x8001, _) => {
                let r = (self.bank_select & 0x07) as usize;
                self.regs[r] = match r {
                    0 | 1 => val & 0xFE,
                    6 | 7 => val & 0x3F,
                    _ => val,
                };
            }
            (0xA000, _) => {
                data.mirror = if val & 0x01 != 0 { Mirror::Horizontal } else { Mirror::Vertical };
            }
            (0xA001, _) => {} // proteção de PRG RAM: F3-02
            (0xC000, _) => self.irq_reload = val,
            (0xC001, _) => self.irq_counter = 0,
            (0xE000, _) => {
                self.irq_enabled = false;
                self.irq_pending = false;
            }
            _ => self.irq_enabled = true,
        }
        true
    }

    #[inline]
    fn chr_offset(&self, addr: u16) -> usize {
        let a = addr ^ ((self.bank_select as u16 & 0x80) << 5); // inverte A12 se bit 7
        let bank = match a >> 10 {
            0 => self.regs[0] as usize,
            1 => self.regs[0] as usize + 1,
            2 => self.regs[1] as usize,
            3 => self.regs[1] as usize + 1,
            n => self.regs[n as usize - 2] as usize,
        };
        bank * 0x0400 + (addr & 0x03FF) as usize
    }

    fn a12_rise(&mut self) {
        if self.irq_counter == 0 {
            self.irq_counter = self.irq_reload;
        } else {
            self.irq_counter -= 1;
        }
        if self.irq_counter == 0 && self.irq_enabled {
            self.irq_pending = true;
        }
    }

    #[inline]
    fn irq_pending(&self) -> bool {
        self.irq_pending
    }

    fn reset(&mut self, _data: &mut CartData) {
        self.bank_select = 0;
        self.regs = [0, 2, 4, 5, 6, 7, 0, 1];
        self.irq_counter = 0;
        self.irq_reload = 0;
        self.irq_enabled = false;
        self.irq_pending = false;
    }

    fn state_string(&self) -> String {
        format!(
            "  MMC3 bank_select: ${:02X}  regs: {:?}\n  MMC3 IRQ: counter={} reload={} enabled={} pending={}\n",
            self.bank_select,
            self.regs,
            self.irq_counter,
            self.irq_reload,
            self.irq_enabled,
            self.irq_pending
        )
    }
}
