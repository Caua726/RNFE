//! Mapper 206 (DxROM / Namco 108): o antepassado do MMC3, sem IRQ e sem controle de mirroring.
use super::{CartData, Mapper};

#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[derive(Clone)]
pub struct Dxrom {
    bank_select: u8,
    prg_banks: [u8; 2],
    chr_banks: [u8; 8],
}

impl Dxrom {
    pub fn new(_data: &CartData) -> Self {
        Dxrom { bank_select: 0, prg_banks: [0, 1], chr_banks: [0, 1, 2, 3, 4, 5, 6, 7] }
    }
}

impl Mapper for Dxrom {
    #[inline]
    fn cpu_read(&self, addr: u16, data: &CartData) -> Option<u8> {
        if addr < 0x8000 {
            return None;
        }
        let bank = match addr {
            0x8000..=0x9FFF => self.prg_banks[0] as usize,
            0xA000..=0xBFFF => self.prg_banks[1] as usize,
            0xC000..=0xDFFF => data.prg_8k().saturating_sub(2),
            _ => data.prg_8k().saturating_sub(1),
        };
        Some(data.prg_at(bank * 0x2000 + (addr & 0x1FFF) as usize))
    }

    fn cpu_write(&mut self, addr: u16, val: u8, _data: &mut CartData) -> bool {
        if !(0x8000..=0x9FFF).contains(&addr) {
            return false;
        }
        if addr & 1 == 0 {
            self.bank_select = val & 0x07;
        } else {
            match self.bank_select {
                r @ (0 | 1) => {
                    self.chr_banks[r as usize * 2] = val & 0x3E;
                    self.chr_banks[r as usize * 2 + 1] = (val & 0x3E) | 1;
                }
                r @ 2..=5 => self.chr_banks[r as usize + 2] = val & 0x3F,
                6 => self.prg_banks[0] = val & 0x0F,
                _ => self.prg_banks[1] = val & 0x0F,
            }
        }
        true
    }

    #[inline]
    fn chr_offset(&self, addr: u16) -> usize {
        self.chr_banks[(addr >> 10) as usize & 7] as usize * 0x0400 + (addr & 0x03FF) as usize
    }

    fn reset(&mut self, _data: &mut CartData) {
        self.bank_select = 0;
        self.prg_banks = [0, 1];
        self.chr_banks = [0, 1, 2, 3, 4, 5, 6, 7];
    }
}
