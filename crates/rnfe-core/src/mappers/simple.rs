//! Mappers de discreto simples que só trocam bancos de 32 KB de PRG e 8 KB de CHR por um
//! registrador: NINA-03/06 (079/113), CPROM (013) e Camerica Quattro (232).
use super::{CartData, Mapper};
use crate::cartridge::Mirror;

/// Mappers 079 (NINA-03/06: AVE) e 113 (variante com mirroring e mais bancos): registrador em
/// `$4100-$5FFF` (A8 = 1, A14 = 0): bits 0-2 CHR 8 KB, bit 3 PRG 32 KB; no 113 os bits 3-5 são
/// o PRG, bit 6 o CHR alto e bit 7 o mirroring.
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[derive(Clone)]
pub struct Nina {
    variant113: bool,
    prg: u8,
    chr: u8,
}

impl Nina {
    pub fn new(data: &CartData) -> Self {
        Nina { variant113: data.mapper == 113, prg: 0, chr: 0 }
    }
}

impl Mapper for Nina {
    #[inline]
    fn cpu_read(&self, addr: u16, data: &CartData) -> Option<u8> {
        if addr >= 0x8000 {
            Some(data.prg_at(self.prg as usize * 0x8000 + (addr & 0x7FFF) as usize))
        } else {
            None
        }
    }

    fn cpu_write(&mut self, addr: u16, val: u8, data: &mut CartData) -> bool {
        if (0x4100..=0x5FFF).contains(&addr) && addr & 0x0100 != 0 {
            if self.variant113 {
                self.prg = (val >> 3) & 0x07;
                self.chr = (val & 0x07) | ((val >> 3) & 0x08);
                data.mirror = if val & 0x80 != 0 { Mirror::Vertical } else { Mirror::Horizontal };
            } else {
                self.prg = (val >> 3) & 0x01;
                self.chr = val & 0x07;
            }
            return true;
        }
        false
    }

    #[inline]
    fn chr_offset(&self, addr: u16) -> usize {
        self.chr as usize * 0x2000 + (addr & 0x1FFF) as usize
    }

    fn reset(&mut self, _data: &mut CartData) {
        self.prg = 0;
        self.chr = 0;
    }
}

/// Mapper 013 (CPROM, Videomation): 16 KB de CHR RAM, a metade de cima (`$1000-$1FFF`)
/// escolhida pelos bits 0-1 de qualquer escrita em `$8000+`.
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[derive(Clone, Default)]
pub struct Cprom {
    bank: u8,
}

impl Cprom {
    pub fn new() -> Self {
        Cprom::default()
    }
}

impl Mapper for Cprom {
    #[inline]
    fn cpu_read(&self, addr: u16, data: &CartData) -> Option<u8> {
        if addr >= 0x8000 { Some(data.prg_at((addr & 0x7FFF) as usize)) } else { None }
    }

    fn cpu_write(&mut self, addr: u16, val: u8, _data: &mut CartData) -> bool {
        if addr >= 0x8000 {
            self.bank = val & 0x03;
            return true;
        }
        false
    }

    #[inline]
    fn chr_offset(&self, addr: u16) -> usize {
        if addr < 0x1000 { addr as usize } else { self.bank as usize * 0x1000 + (addr & 0x0FFF) as usize }
    }

    fn reset(&mut self, _data: &mut CartData) {
        self.bank = 0;
    }
}

/// Mapper 232 (Camerica Quattro / Aladdin Deck Enhancer): 4 blocos de 64 KB (bits 3-4 de
/// `$8000-$BFFF`), dentro do bloco 16 KB comutáveis em `$8000` (bits 0-1 de `$C000-$FFFF`) e
/// o último 16 KB do bloco fixo em `$C000`.
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[derive(Clone, Default)]
pub struct Quattro {
    block: u8,
    bank: u8,
}

impl Quattro {
    pub fn new() -> Self {
        Quattro::default()
    }
}

impl Mapper for Quattro {
    #[inline]
    fn cpu_read(&self, addr: u16, data: &CartData) -> Option<u8> {
        let base = self.block as usize * 4;
        match addr {
            0x8000..=0xBFFF => {
                Some(data.prg_at((base + self.bank as usize) * 0x4000 + (addr & 0x3FFF) as usize))
            }
            0xC000..=0xFFFF => Some(data.prg_at((base + 3) * 0x4000 + (addr & 0x3FFF) as usize)),
            _ => None,
        }
    }

    fn cpu_write(&mut self, addr: u16, val: u8, _data: &mut CartData) -> bool {
        match addr {
            0x8000..=0xBFFF => self.block = (val >> 3) & 0x03,
            0xC000..=0xFFFF => self.bank = val & 0x03,
            _ => return false,
        }
        true
    }

    fn reset(&mut self, _data: &mut CartData) {
        self.block = 0;
        self.bank = 0;
    }
}

/// Mapper 034 (AVE NINA-001): PRG de 32 KB e CHR em duas metades de 4 KB, com os registradores
/// **na PRG RAM** (`$7FFD`/`$7FFE`/`$7FFF`). O mesmo número iNES serve à BNROM (CHR RAM): quem
/// tem CHR ROM é NINA-001.
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[derive(Clone, Default)]
pub struct Nina001 {
    prg: u8,
    chr: [u8; 2],
}

impl Nina001 {
    pub fn new() -> Self {
        Nina001::default()
    }
}

impl Mapper for Nina001 {
    #[inline]
    fn cpu_read(&self, addr: u16, data: &CartData) -> Option<u8> {
        if addr >= 0x8000 {
            Some(data.prg_at(self.prg as usize * 0x8000 + (addr & 0x7FFF) as usize))
        } else {
            None
        }
    }

    fn cpu_write(&mut self, addr: u16, val: u8, data: &mut CartData) -> bool {
        match addr {
            0x7FFD => self.prg = val & 0x01,
            0x7FFE => self.chr[0] = val & 0x0F,
            0x7FFF => self.chr[1] = val & 0x0F,
            _ => return false,
        }
        // os registradores também ficam na RAM (o jogo lê de volta)
        data.prg_ram_set((addr & 0x1FFF) as usize, val);
        true
    }

    #[inline]
    fn chr_offset(&self, addr: u16) -> usize {
        let half = (addr >> 12) as usize & 1;
        self.chr[half] as usize * 0x1000 + (addr & 0x0FFF) as usize
    }

    fn manages_prg_ram(&self) -> bool {
        false
    }

    fn reset(&mut self, _data: &mut CartData) {
        self.prg = 0;
        self.chr = [0, 1];
    }
}
