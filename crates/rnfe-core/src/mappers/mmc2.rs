//! Mapper 009 (MMC2, Punch-Out!!): CHR trocado por latches disparados pela leitura de tiles.
use super::{CartData, Mapper};
use crate::cartridge::Mirror;

#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[derive(Clone)]
pub struct Mmc2 {
    prg_bank: u8,
    chr_banks: [u8; 4],
    latch: [u8; 2],
}

impl Mmc2 {
    pub fn new() -> Self {
        Mmc2 { prg_bank: 0, chr_banks: [0; 4], latch: [0xFE, 0xFE] }
    }
}

impl Default for Mmc2 {
    fn default() -> Self {
        Self::new()
    }
}

impl Mapper for Mmc2 {
    #[inline]
    fn prg_offset(&self, addr: u16, data: &CartData) -> Option<usize> {
        if addr < 0x8000 {
            return None;
        }
        let bank = match addr {
            0x8000..=0x9FFF => self.prg_bank as usize,
            0xA000..=0xBFFF => data.prg_8k().saturating_sub(3),
            0xC000..=0xDFFF => data.prg_8k().saturating_sub(2),
            _ => data.prg_8k().saturating_sub(1),
        };
        Some(bank * 0x2000 + (addr & 0x1FFF) as usize)
    }

    fn cpu_read(&self, addr: u16, data: &CartData) -> Option<u8> {
        if addr < 0x8000 {
            return None;
        }
        let bank = match addr {
            0x8000..=0x9FFF => self.prg_bank as usize,
            0xA000..=0xBFFF => data.prg_8k().saturating_sub(3),
            0xC000..=0xDFFF => data.prg_8k().saturating_sub(2),
            _ => data.prg_8k().saturating_sub(1),
        };
        Some(data.prg_at(bank * 0x2000 + (addr & 0x1FFF) as usize))
    }

    fn cpu_write(&mut self, addr: u16, val: u8, data: &mut CartData) -> bool {
        match addr {
            0xA000..=0xAFFF => self.prg_bank = val & 0x0F,
            0xB000..=0xBFFF => self.chr_banks[0] = val & 0x1F,
            0xC000..=0xCFFF => self.chr_banks[1] = val & 0x1F,
            0xD000..=0xDFFF => self.chr_banks[2] = val & 0x1F,
            0xE000..=0xEFFF => self.chr_banks[3] = val & 0x1F,
            0xF000..=0xFFFF => {
                data.mirror = if val & 0x01 != 0 { Mirror::Horizontal } else { Mirror::Vertical }
            }
            _ => return false,
        }
        true
    }

    fn chr_dynamic(&self) -> bool {
        true
    }

    #[inline]
    fn chr_offset(&self, addr: u16) -> usize {
        let bank = if addr < 0x1000 {
            if self.latch[0] == 0xFD { self.chr_banks[0] } else { self.chr_banks[1] }
        } else if self.latch[1] == 0xFD {
            self.chr_banks[2]
        } else {
            self.chr_banks[3]
        };
        bank as usize * 0x1000 + (addr & 0x0FFF) as usize
    }

    fn ppu_read(&mut self, addr: u16, data: &CartData) -> u8 {
        let v = data.chr_at(self.chr_offset(addr));
        // Os latches mudam DEPOIS da leitura do tile $FD/$FE
        match addr {
            0x0FD8 => self.latch[0] = 0xFD,
            0x0FE8 => self.latch[0] = 0xFE,
            0x1FD8..=0x1FDF => self.latch[1] = 0xFD,
            0x1FE8..=0x1FEF => self.latch[1] = 0xFE,
            _ => {}
        }
        v
    }

    fn reset(&mut self, _data: &mut CartData) {
        self.prg_bank = 0;
        self.chr_banks = [0; 4];
        self.latch = [0xFE, 0xFE];
    }

    fn state_string(&self) -> String {
        format!("  MMC2 PRG: {}  CHR: {:?}  latches: {:02X?}\n", self.prg_bank, self.chr_banks, self.latch)
    }
}
