//! Mapper 001 (MMC1): registradores carregados bit a bit por uma porta serial.
//!
//! Variantes por tamanho, sem submapper: SUROM/SOROM/SXROM usam o bit 4 do banco de CHR para
//! escolher a metade de 256 KB de PRG e os bits 2-3 para o banco de PRG RAM (16/32 KB).
use super::{CartData, Mapper};
use crate::cartridge::Mirror;

#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[derive(Clone)]
pub struct Mmc1 {
    shift: u8,
    shift_count: u8,
    control: u8,
    chr_bank0: u8,
    chr_bank1: u8,
    prg_bank: u8,
    /// Ciclo de CPU da última escrita em `$8000+`: a segunda escrita de um RMW (ciclo seguinte)
    /// é ignorada pelo hardware.
    last_write: u64,
    /// PRG maior que 256 KB: o bit 4 dos bancos de CHR escolhe a metade.
    big_prg: bool,
}

impl Mmc1 {
    pub fn new(data: &CartData) -> Self {
        Mmc1 {
            shift: 0x10,
            shift_count: 0,
            control: 0x0C,
            chr_bank0: 0,
            chr_bank1: 0,
            prg_bank: 0,
            last_write: u64::MAX,
            big_prg: data.prg.len() > 256 * 1024,
        }
    }

    #[inline]
    fn prg_ram_enabled(&self) -> bool {
        self.prg_bank & 0x10 == 0
    }

    /// Offset da PRG RAM (SOROM: bit 3 → 16 KB; SXROM: bits 2-3 → 32 KB).
    #[inline]
    fn prg_ram_offset(&self, addr: u16, data: &CartData) -> usize {
        let bank = match data.prg_ram.len() {
            0..=8192 => 0,
            8193..=16384 => (self.chr_bank0 >> 3) as usize & 1,
            _ => (self.chr_bank0 >> 2) as usize & 3,
        };
        bank * 0x2000 + (addr & 0x1FFF) as usize
    }
}

impl Mapper for Mmc1 {
    #[inline]
    fn prg_offset(&self, addr: u16, _data: &CartData) -> Option<usize> {
        if addr < 0x8000 {
            return None;
        }
        let mut offset = match (self.control >> 2) & 0x03 {
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
                    0x0F * 0x4000 + (addr & 0x3FFF) as usize
                }
            }
        };
        if self.big_prg {
            offset = (offset & 0x3FFFF) | ((self.chr_bank0 as usize & 0x10) << 14);
        }
        Some(offset)
    }

    fn cpu_read(&self, addr: u16, data: &CartData) -> Option<u8> {
        if addr < 0x8000 {
            return if addr >= 0x6000 && self.prg_ram_enabled() {
                Some(data.prg_ram_at(self.prg_ram_offset(addr, data)))
            } else {
                None
            };
        }
        let mut offset = match (self.control >> 2) & 0x03 {
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
                    0x0F * 0x4000 + (addr & 0x3FFF) as usize
                }
            }
        };
        if self.big_prg {
            offset = (offset & 0x3FFFF) | ((self.chr_bank0 as usize & 0x10) << 14);
        }
        Some(data.prg_at(offset))
    }

    fn cpu_write(&mut self, addr: u16, val: u8, data: &mut CartData) -> bool {
        if addr < 0x8000 {
            if addr >= 0x6000 {
                if self.prg_ram_enabled() {
                    let off = self.prg_ram_offset(addr, data);
                    data.prg_ram_set(off, val);
                }
                return true;
            }
            return false;
        }
        // Escritas em ciclos consecutivos (RMW: `INC $8000`) — só a primeira conta
        let consecutive = data.cpu_cycle.wrapping_sub(self.last_write) <= 1;
        self.last_write = data.cpu_cycle;
        if consecutive {
            return true;
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
                _ => self.prg_bank = value,
            }
            self.shift = 0x10;
            self.shift_count = 0;
        }
        true
    }

    fn manages_prg_ram(&self) -> bool {
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

    fn reset(&mut self, data: &mut CartData) {
        self.shift = 0x10;
        self.shift_count = 0;
        self.control = 0x0C;
        self.chr_bank0 = 0;
        self.chr_bank1 = 0;
        self.prg_bank = 0;
        self.last_write = u64::MAX;
        // o bit de mirroring do header vale até o jogo escrever no control
        data.mirror = if data.mirror == Mirror::Vertical { Mirror::Vertical } else { Mirror::Horizontal };
    }

    fn state_string(&self) -> String {
        format!(
            "  MMC1 ctrl: ${:02X}  PRG bank: ${:02X}  CHR banks: ${:02X}/${:02X}  RAM: {}\n",
            self.control,
            self.prg_bank,
            self.chr_bank0,
            self.chr_bank1,
            if self.prg_ram_enabled() { "on" } else { "off" }
        )
    }
}
