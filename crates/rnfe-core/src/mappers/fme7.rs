//! Mapper 069 (FME-7 / Sunsoft 5B): 4 bancos de PRG de 8 KB, 8 de CHR de 1 KB, IRQ por contador.
use super::{CartData, Mapper};
use crate::cartridge::Mirror;

pub struct Fme7 {
    command: u8,
    prg_banks: [u8; 4],
    chr_banks: [u8; 8],
    /// Contador de 16 bits decrementado a cada ciclo de CPU (se `irq_count_enabled`);
    /// ao passar de $0000 para $FFFF dispara o IRQ (se `irq_enabled`).
    irq_counter: u16,
    irq_count_enabled: bool,
    irq_enabled: bool,
    irq_pending: bool,
}

impl Fme7 {
    pub fn new() -> Self {
        Fme7 {
            command: 0,
            prg_banks: [0; 4],
            chr_banks: [0; 8],
            irq_counter: 0,
            irq_count_enabled: false,
            irq_enabled: false,
            irq_pending: false,
        }
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
                    0xD => {
                        self.irq_enabled = val & 0x01 != 0;
                        self.irq_count_enabled = val & 0x80 != 0;
                        self.irq_pending = false; // ack
                    }
                    0xE => self.irq_counter = (self.irq_counter & 0xFF00) | val as u16,
                    _ => self.irq_counter = (self.irq_counter & 0x00FF) | ((val as u16) << 8),
                }
                true
            }
            _ => false,
        }
    }

    fn wants_cpu_clock(&self) -> bool {
        true
    }

    #[inline]
    fn cpu_clock(&mut self) {
        if self.irq_count_enabled {
            let (v, wrapped) = self.irq_counter.overflowing_sub(1);
            self.irq_counter = v;
            if wrapped && self.irq_enabled {
                self.irq_pending = true;
            }
        }
    }

    #[inline]
    fn irq_pending(&self) -> bool {
        self.irq_pending
    }

    fn manages_prg_ram(&self) -> bool {
        true
    }

    #[inline]
    fn chr_offset(&self, addr: u16) -> usize {
        self.chr_banks[(addr >> 10) as usize & 7] as usize * 0x0400 + (addr & 0x03FF) as usize
    }

    fn reset(&mut self, _data: &mut CartData) {
        self.command = 0;
        self.prg_banks = [0; 4];
        self.chr_banks = [0; 8];
        self.irq_counter = 0;
        self.irq_count_enabled = false;
        self.irq_enabled = false;
        self.irq_pending = false;
    }

    fn state_string(&self) -> String {
        format!(
            "  FME-7 cmd: {}  PRG: {:?}  CHR: {:?}  IRQ: counter=${:04X} count={} irq={} pending={}\n",
            self.command,
            self.prg_banks,
            self.chr_banks,
            self.irq_counter,
            self.irq_count_enabled,
            self.irq_enabled,
            self.irq_pending
        )
    }
}
