//! Mapper 065 (Irem H3001): 3 bancos de PRG de 8 KB, 8 de CHR de 1 KB e um contador de IRQ
//! de 16 bits decrementado por ciclo de CPU.
use super::{CartData, Mapper};
use crate::cartridge::Mirror;

#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[derive(Clone)]
pub struct H3001 {
    prg: [u8; 3],
    chr: [u8; 8],
    irq_latch: u16,
    irq_counter: u16,
    irq_enabled: bool,
    irq_pending: bool,
}

impl H3001 {
    pub fn new(_data: &CartData) -> Self {
        H3001 {
            prg: [0, 1, 0xFE],
            chr: [0, 1, 2, 3, 4, 5, 6, 7],
            irq_latch: 0,
            irq_counter: 0,
            irq_enabled: false,
            irq_pending: false,
        }
    }
}

impl Mapper for H3001 {
    #[inline]
    fn prg_offset(&self, addr: u16, data: &CartData) -> Option<usize> {
        if addr < 0x8000 {
            return None;
        }
        let bank = match (addr >> 13) & 3 {
            0 => self.prg[0] as usize,
            1 => self.prg[1] as usize,
            2 => self.prg[2] as usize,
            _ => data.prg_8k().saturating_sub(1),
        };
        Some(bank * 0x2000 + (addr & 0x1FFF) as usize)
    }

    fn cpu_read(&self, addr: u16, data: &CartData) -> Option<u8> {
        if addr < 0x8000 {
            return None;
        }
        let bank = match (addr >> 13) & 3 {
            0 => self.prg[0] as usize,
            1 => self.prg[1] as usize,
            2 => self.prg[2] as usize,
            _ => data.prg_8k().saturating_sub(1),
        };
        Some(data.prg_at(bank * 0x2000 + (addr & 0x1FFF) as usize))
    }

    fn cpu_write(&mut self, addr: u16, val: u8, data: &mut CartData) -> bool {
        match addr {
            0x8000 => self.prg[0] = val,
            0xA000 => self.prg[1] = val,
            0xC000 => self.prg[2] = val,
            0x9001 => data.mirror = if val & 0x80 != 0 { Mirror::Horizontal } else { Mirror::Vertical },
            0x9003 => {
                self.irq_enabled = val & 0x80 != 0;
                self.irq_pending = false;
            }
            0x9004 => {
                self.irq_counter = self.irq_latch;
                self.irq_pending = false;
            }
            0x9005 => self.irq_latch = (self.irq_latch & 0x00FF) | ((val as u16) << 8),
            0x9006 => self.irq_latch = (self.irq_latch & 0xFF00) | val as u16,
            0xB000..=0xB007 => self.chr[(addr & 7) as usize] = val,
            _ => return addr >= 0x8000,
        }
        true
    }

    #[inline]
    fn chr_offset(&self, addr: u16) -> usize {
        self.chr[(addr >> 10) as usize & 7] as usize * 0x0400 + (addr & 0x03FF) as usize
    }

    fn wants_cpu_clock(&self) -> bool {
        true
    }

    #[inline]
    fn cpu_clock(&mut self) {
        if self.irq_enabled && self.irq_counter > 0 {
            self.irq_counter -= 1;
            if self.irq_counter == 0 {
                self.irq_pending = true;
            }
        }
    }

    #[inline]
    fn irq_pending(&self) -> bool {
        self.irq_pending
    }

    fn reset(&mut self, data: &mut CartData) {
        *self = H3001::new(data);
    }

    fn state_string(&self) -> String {
        format!(
            "  H3001 PRG {:?} CHR {:?}  IRQ latch=${:04X} counter=${:04X} en={} pending={}\n",
            self.prg, self.chr, self.irq_latch, self.irq_counter, self.irq_enabled, self.irq_pending
        )
    }
}
