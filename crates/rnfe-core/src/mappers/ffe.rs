//! Mappers 006, 008 e 017 (Front Far East: F4xxx, F3xxx e F8xxx).
//!
//! São os formatos dos copiadores da FFE, muito comuns em packs antigos: o "cartucho" é a RAM
//! do copiador e o jogo original vinha acompanhado de um *trainer* de 512 bytes em `$7000`
//! (carregado pelo [`crate::cartridge`]). Os registradores ficam na faixa `$4020-$5FFF`:
//!
//! - `$42FE` bit 4: nametable única (baixa/alta);
//! - `$42FF` bit 4: espelhamento horizontal (1) ou vertical (0);
//! - `$4501`: desliga o IRQ; `$4502`/`$4503`: 16 bits do contador e liga o IRQ.
//!
//! O contador de IRQ sobe a cada ciclo de CPU e dispara ao passar de `$FFFF`.
//!
//! O banco de PRG/CHR vem de uma escrita em `$8000-$FFFF` (6 e 8) ou de `$4504-$4507` e
//! `$4510-$4517` (17).
use super::{CartData, Mapper};
use crate::cartridge::Mirror;

/// Qual placa da FFE: muda só onde ficam os bancos.
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[derive(Clone, Copy, PartialEq, Eq)]
pub enum FfeKind {
    /// Mapper 6: PRG 16 KB em `$8000` (bits 2-7), CHR 8 KB (bits 0-1).
    F4,
    /// Mapper 8: PRG 16 KB em `$8000` (bits 3-7), CHR 8 KB (bits 0-2).
    F3,
    /// Mapper 17: PRG 4 × 8 KB e CHR 8 × 1 KB por registradores.
    F8,
}

#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[derive(Clone)]
pub struct Ffe {
    kind: FfeKind,
    /// Bancos de 8 KB de PRG (F4/F3 preenchem os dois primeiros a partir do banco de 16 KB).
    prg: [u8; 4],
    chr: [u8; 8],
    irq_counter: u16,
    irq_enabled: bool,
    irq_pending: bool,
}

impl Ffe {
    pub fn new(kind: FfeKind, data: &CartData) -> Self {
        // O copiador deixava os bancos prontos antes de soltar o jogo. F4/F3 têm o banco fixo
        // (o de índice 7, o último dos 128 KB do copiador); no F8 as quatro janelas comutam, e
        // os vetores das ROMs ficam no último banco de 8 KB.
        let last = data.prg_8k().saturating_sub(1) as u8;
        let prg = match kind {
            FfeKind::F8 => [0, 1, last.saturating_sub(1), last],
            _ => [0, 1, 0, 0],
        };
        Ffe {
            kind,
            prg,
            chr: [0, 1, 2, 3, 4, 5, 6, 7],
            irq_counter: 0,
            irq_enabled: false,
            irq_pending: false,
        }
    }

    pub fn name(&self) -> &'static str {
        match self.kind {
            FfeKind::F4 => "FFE F4xxx",
            FfeKind::F3 => "FFE F3xxx",
            FfeKind::F8 => "FFE F8xxx",
        }
    }

    /// Registradores comuns em `$4020-$5FFF`. Devolve `true` se consumiu a escrita.
    fn write_ffe(&mut self, addr: u16, val: u8, data: &mut CartData) -> bool {
        match addr {
            0x42FE => {
                data.mirror = if val & 0x10 != 0 { Mirror::OneScreenHi } else { Mirror::OneScreenLo };
            }
            0x42FF => {
                data.mirror = if val & 0x10 != 0 { Mirror::Horizontal } else { Mirror::Vertical };
            }
            0x4501 => {
                self.irq_enabled = false;
                self.irq_pending = false;
            }
            0x4502 => self.irq_counter = (self.irq_counter & 0xFF00) | val as u16,
            0x4503 => {
                self.irq_counter = (self.irq_counter & 0x00FF) | ((val as u16) << 8);
                self.irq_enabled = true;
                self.irq_pending = false;
            }
            0x4504..=0x4507 if self.kind == FfeKind::F8 => {
                self.prg[(addr - 0x4504) as usize] = val;
            }
            0x4510..=0x4517 if self.kind == FfeKind::F8 => {
                self.chr[(addr - 0x4510) as usize] = val;
            }
            _ => return false,
        }
        true
    }

    fn bank8(&self, addr: u16, data: &CartData) -> usize {
        let slot = ((addr >> 13) & 3) as usize;
        match self.kind {
            FfeKind::F8 => self.prg[slot] as usize,
            // 16 KB comutáveis em $8000; $C000 fixo no banco 7 (o último dos 128 KB do
            // copiador — é lá que ficam os vetores nessas imagens)
            _ => {
                if addr < 0xC000 {
                    self.prg[0] as usize * 2 + slot
                } else {
                    let fixed = 7usize.min(data.prg_16k().saturating_sub(1));
                    fixed * 2 + (slot & 1)
                }
            }
        }
    }
}

impl Mapper for Ffe {
    fn prg_offset(&self, addr: u16, data: &CartData) -> Option<usize> {
        (addr >= 0x8000).then(|| self.bank8(addr, data) * 0x2000 + (addr & 0x1FFF) as usize)
    }

    fn cpu_read(&self, addr: u16, data: &CartData) -> Option<u8> {
        match addr {
            0x6000..=0x7FFF => Some(data.prg_ram_at((addr & 0x1FFF) as usize)),
            0x8000..=0xFFFF => Some(data.prg_at(self.bank8(addr, data) * 0x2000 + (addr & 0x1FFF) as usize)),
            _ => None,
        }
    }

    fn cpu_write(&mut self, addr: u16, val: u8, data: &mut CartData) -> bool {
        if self.write_ffe(addr, val, data) {
            return true;
        }
        match addr {
            0x6000..=0x7FFF => {
                data.prg_ram_set((addr & 0x1FFF) as usize, val);
                true
            }
            0x8000..=0xFFFF => {
                match self.kind {
                    FfeKind::F4 => {
                        self.prg[0] = (val >> 2) & 0x3F;
                        let bank = (val & 0x03) as usize;
                        for (i, c) in self.chr.iter_mut().enumerate() {
                            *c = (bank * 8 + i) as u8;
                        }
                    }
                    FfeKind::F3 => {
                        self.prg[0] = val >> 3;
                        let bank = (val & 0x07) as usize;
                        for (i, c) in self.chr.iter_mut().enumerate() {
                            *c = (bank * 8 + i) as u8;
                        }
                    }
                    // No F8 a faixa de ROM não tem registrador
                    FfeKind::F8 => return false,
                }
                true
            }
            _ => false,
        }
    }

    #[inline]
    fn chr_offset(&self, addr: u16) -> usize {
        self.chr[(addr >> 10) as usize & 7] as usize * 0x0400 + (addr & 0x03FF) as usize
    }

    fn manages_prg_ram(&self) -> bool {
        true
    }

    fn wants_cpu_clock(&self) -> bool {
        true
    }

    #[inline]
    fn cpu_clock(&mut self) {
        if self.irq_enabled {
            let (next, carry) = self.irq_counter.overflowing_add(1);
            self.irq_counter = next;
            if carry {
                self.irq_pending = true;
                self.irq_enabled = false;
            }
        }
    }

    fn irq_pending(&self) -> bool {
        self.irq_pending
    }

    fn reset(&mut self, data: &mut CartData) {
        *self = Ffe::new(self.kind, data);
    }

    fn state_string(&self) -> String {
        format!("  {} PRG: {:?}  CHR: {:?}  IRQ: {}\n", self.name(), self.prg, self.chr, self.irq_counter)
    }
}
