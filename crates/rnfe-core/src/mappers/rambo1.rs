//! Mapper 064 (Tengen RAMBO-1): um MMC3 estendido — 3 bancos de PRG de 8 KB, CHR em 1 KB
//! opcional (mais dois registradores), e IRQ por scanline (A12) ou por ciclos de CPU (÷4).
use super::{CartData, Mapper};
use crate::cartridge::Mirror;

#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[derive(Clone)]
pub struct Rambo1 {
    /// `$8000`: bits 0-3 registrador, bit 5 CHR em 1 KB, bit 6 modo PRG, bit 7 inverte A12.
    select: u8,
    /// R0-R7 como no MMC3, R8/R9 os bancos de 1 KB extras, R15 o 3º banco de PRG.
    regs: [u8; 16],
    irq_latch: u8,
    irq_counter: u8,
    irq_reload: bool,
    irq_enabled: bool,
    irq_pending: bool,
    /// `$C001` bit 0: contar ciclos de CPU (÷4) em vez de subidas de A12.
    irq_cycle_mode: bool,
    prescaler: u8,
    /// O IRQ do RAMBO-1 sai alguns ciclos depois do clock: fila de atraso.
    irq_delay: u8,
}

impl Rambo1 {
    pub fn new(_data: &CartData) -> Self {
        Rambo1 {
            select: 0,
            regs: [0, 2, 4, 5, 6, 7, 0, 1, 0, 0, 0, 0, 0, 0, 0, 2],
            irq_latch: 0,
            irq_counter: 0,
            irq_reload: false,
            irq_enabled: false,
            irq_pending: false,
            irq_cycle_mode: false,
            prescaler: 0,
            irq_delay: 0,
        }
    }

    fn clock_irq(&mut self) {
        if self.irq_reload {
            self.irq_reload = false;
            // recarrega com latch (o hardware recarrega latch|1 no clock seguinte ao reload)
            self.irq_counter = self.irq_latch | if self.irq_latch == 0 { 0 } else { 1 };
        } else if self.irq_counter == 0 {
            self.irq_counter = self.irq_latch;
        } else {
            self.irq_counter -= 1;
        }
        if self.irq_counter == 0 && self.irq_enabled {
            self.irq_delay = 4;
        }
    }
}

impl Mapper for Rambo1 {
    #[inline]
    fn cpu_read(&self, addr: u16, data: &CartData) -> Option<u8> {
        if addr < 0x8000 {
            return None;
        }
        let p = self.select & 0x40 != 0;
        let last = data.prg_8k().saturating_sub(1);
        let bank = match (addr >> 13) & 3 {
            0 => (if p { self.regs[15] } else { self.regs[6] }) as usize,
            1 => (if p { self.regs[6] } else { self.regs[7] }) as usize,
            2 => (if p { self.regs[7] } else { self.regs[15] }) as usize,
            _ => last,
        };
        Some(data.prg_at(bank * 0x2000 + (addr & 0x1FFF) as usize))
    }

    fn cpu_write(&mut self, addr: u16, val: u8, data: &mut CartData) -> bool {
        if addr < 0x8000 {
            return false;
        }
        match (addr & 0xE000, addr & 1) {
            (0x8000, 0) => self.select = val,
            (0x8000, _) => self.regs[(self.select & 0x0F) as usize] = val,
            (0xA000, 0) => data.mirror = if val & 1 != 0 { Mirror::Horizontal } else { Mirror::Vertical },
            (0xC000, 0) => self.irq_latch = val,
            (0xC000, _) => {
                self.irq_cycle_mode = val & 1 != 0;
                self.irq_reload = true;
                self.prescaler = 0;
            }
            (0xE000, 0) => {
                self.irq_enabled = false;
                self.irq_pending = false;
                self.irq_delay = 0;
            }
            (0xE000, _) => self.irq_enabled = true,
            _ => {}
        }
        true
    }

    #[inline]
    fn chr_offset(&self, addr: u16) -> usize {
        let a = addr ^ ((self.select as u16 & 0x80) << 5); // inverte A12 se bit 7
        let slot = (a >> 10) as usize;
        let one_kb = self.select & 0x20 != 0;
        let bank = match slot {
            0..=3 if one_kb => [self.regs[0], self.regs[8], self.regs[1], self.regs[9]][slot] as usize,
            0 => self.regs[0] as usize & !1,
            1 => (self.regs[0] as usize & !1) + 1,
            2 => self.regs[1] as usize & !1,
            3 => (self.regs[1] as usize & !1) + 1,
            n => self.regs[n - 2] as usize,
        };
        bank * 0x0400 + (addr & 0x03FF) as usize
    }

    fn a12_rise(&mut self) {
        if !self.irq_cycle_mode {
            self.clock_irq();
        }
    }

    fn wants_cpu_clock(&self) -> bool {
        true
    }

    #[inline]
    fn cpu_clock(&mut self) {
        if self.irq_delay > 0 {
            self.irq_delay -= 1;
            if self.irq_delay == 0 {
                self.irq_pending = true;
            }
        }
        if self.irq_cycle_mode {
            self.prescaler = (self.prescaler + 1) & 3;
            if self.prescaler == 0 {
                self.clock_irq();
            }
        }
    }

    #[inline]
    fn irq_pending(&self) -> bool {
        self.irq_pending
    }

    fn reset(&mut self, data: &mut CartData) {
        *self = Rambo1::new(data);
    }

    fn state_string(&self) -> String {
        format!(
            "  RAMBO-1 select ${:02X} regs {:?}  IRQ latch={} counter={} en={} cycle={} pending={}\n",
            self.select,
            self.regs,
            self.irq_latch,
            self.irq_counter,
            self.irq_enabled,
            self.irq_cycle_mode,
            self.irq_pending
        )
    }
}
