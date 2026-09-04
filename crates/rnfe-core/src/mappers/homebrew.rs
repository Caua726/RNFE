//! Mappers dos homebrews modernos: 030 (UNROM 512) e 028 (Action 53).
use super::{CartData, Mapper};
use crate::cartridge::Mirror;

/// Mapper 030 (UNROM 512): 16 KB comutáveis em `$8000` (bits 0-4), CHR RAM de 32 KB em
/// bancos de 8 KB (bits 5-6), e mirroring de uma tela pelo bit 7 (se o header pedir).
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[derive(Clone)]
pub struct Unrom512 {
    prg: u8,
    chr: u8,
    one_screen: bool,
}

impl Unrom512 {
    pub fn new(data: &CartData) -> Self {
        // iNES: four-screen (bit 3) marca o mirroring controlado pelo mapper (uma tela)
        Unrom512 { prg: 0, chr: 0, one_screen: data.four_screen }
    }
}

impl Mapper for Unrom512 {
    #[inline]
    fn prg_offset(&self, addr: u16, data: &CartData) -> Option<usize> {
        match addr {
            0x8000..=0xBFFF => Some(self.prg as usize * 0x4000 + (addr & 0x3FFF) as usize),
            0xC000..=0xFFFF => Some((data.prg_16k() - 1) * 0x4000 + (addr & 0x3FFF) as usize),
            _ => None,
        }
    }

    fn cpu_read(&self, addr: u16, data: &CartData) -> Option<u8> {
        match addr {
            0x8000..=0xBFFF => Some(data.prg_at(self.prg as usize * 0x4000 + (addr & 0x3FFF) as usize)),
            0xC000..=0xFFFF => Some(data.prg_at((data.prg_16k() - 1) * 0x4000 + (addr & 0x3FFF) as usize)),
            _ => None,
        }
    }

    fn cpu_write(&mut self, addr: u16, val: u8, data: &mut CartData) -> bool {
        if addr < 0x8000 {
            return false;
        }
        self.prg = val & 0x1F;
        self.chr = (val >> 5) & 0x03;
        if self.one_screen {
            data.mirror = if val & 0x80 != 0 { Mirror::OneScreenHi } else { Mirror::OneScreenLo };
            data.four_screen = false;
        }
        true
    }

    #[inline]
    fn chr_offset(&self, addr: u16) -> usize {
        self.chr as usize * 0x2000 + (addr & 0x1FFF) as usize
    }

    fn reset(&mut self, data: &mut CartData) {
        self.prg = 0;
        self.chr = 0;
        if self.one_screen {
            data.mirror = Mirror::OneScreenLo;
            data.four_screen = false;
        }
    }
}

/// Mapper 028 (Action 53): multicart de homebrews. Registrador escolhido em `$5000-$5FFF`
/// (bits 7 e 0), valor em `$8000-$FFFF`: 0 = CHR 8 KB + mirroring de 1 tela, 1 = banco de PRG
/// interno + mirroring, `$80` = modo (tamanho do "jogo", modo de PRG, mirroring), `$81` = banco
/// externo (o jogo). Tamanho do jogo: 32 KB × 2^S.
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[derive(Clone, Default)]
pub struct Action53 {
    select: u8,
    chr: u8,
    inner: u8,
    mode: u8,
    outer: u8,
}

impl Action53 {
    pub fn new() -> Self {
        Action53::default()
    }

    fn apply_mirror(&self, data: &mut CartData) {
        data.mirror = match self.mode & 0x03 {
            0 => Mirror::OneScreenLo,
            1 => Mirror::OneScreenHi,
            2 => Mirror::Vertical,
            _ => Mirror::Horizontal,
        };
    }

    /// Banco de 16 KB para a metade `hi` (`$C000`) ou baixa (`$8000`).
    fn prg_16k(&self, hi: bool) -> usize {
        let size = ((self.mode >> 4) & 3) as u32; // 0..3 → 32 KB × 2^size
        let prg_mode = (self.mode >> 2) & 3;
        let outer = self.outer as usize;
        let inner = self.inner as usize & 0x0F;
        let mask = (2usize << size) - 1; // bancos de 16 KB dentro do jogo
        let bank = match prg_mode {
            0 | 1 => (inner << 1) | hi as usize, // 32 KB: inner escolhe o par
            2 => {
                if hi { inner } else { 0 } // $8000 fixo no 1º do jogo
            }
            _ => {
                if hi { mask } else { inner } // $C000 fixo no último do jogo
            }
        };
        (outer << 1 & !mask) | (bank & mask)
    }
}

impl Mapper for Action53 {
    #[inline]
    fn prg_offset(&self, addr: u16, _data: &CartData) -> Option<usize> {
        if addr < 0x8000 {
            return None;
        }
        Some(self.prg_16k(addr >= 0xC000) * 0x4000 + (addr & 0x3FFF) as usize)
    }

    fn cpu_read(&self, addr: u16, data: &CartData) -> Option<u8> {
        if addr < 0x8000 {
            return None;
        }
        let bank = self.prg_16k(addr >= 0xC000);
        Some(data.prg_at(bank * 0x4000 + (addr & 0x3FFF) as usize))
    }

    fn cpu_write(&mut self, addr: u16, val: u8, data: &mut CartData) -> bool {
        match addr {
            0x5000..=0x5FFF => self.select = val & 0x81,
            0x8000..=0xFFFF => match self.select {
                0x00 => {
                    self.chr = val & 0x03;
                    if self.mode & 0x02 == 0 {
                        self.mode = (self.mode & !0x01) | ((val >> 4) & 0x01);
                        self.apply_mirror(data);
                    }
                }
                0x01 => {
                    self.inner = val & 0x0F;
                    if self.mode & 0x02 == 0 {
                        self.mode = (self.mode & !0x01) | ((val >> 4) & 0x01);
                        self.apply_mirror(data);
                    }
                }
                0x80 => {
                    self.mode = val;
                    self.apply_mirror(data);
                }
                _ => self.outer = val,
            },
            _ => return false,
        }
        true
    }

    #[inline]
    fn chr_offset(&self, addr: u16) -> usize {
        self.chr as usize * 0x2000 + (addr & 0x1FFF) as usize
    }

    fn reset(&mut self, data: &mut CartData) {
        *self = Action53 { outer: 0xFF, ..Default::default() };
        self.apply_mirror(data);
    }
}
