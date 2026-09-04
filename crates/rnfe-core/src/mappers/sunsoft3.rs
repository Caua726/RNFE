//! Mapper 067 (Sunsoft-3): CHR em 4 bancos de 2 KB, PRG de 16 KB em `$8000` com o último fixo,
//! espelhamento por registrador e IRQ por contador de ciclos de CPU.
//!
//! O contador de 16 bits é escrito em **duas metades** por `$C800` (alto e depois baixo), com um
//! alternador que a escrita de `$D800` (liga/desliga) zera. Ele decresce a cada ciclo e dispara
//! ao passar de zero, desligando-se em seguida. Jogos: Fantasy Zone II, Mito Koumon, Ripple
//! Island, Nantettatte!! Baseball.
use super::{CartData, Mapper};
use crate::cartridge::Mirror;

#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[derive(Clone, Default)]
pub struct Sunsoft3 {
    chr: [u8; 4],
    prg: u8,
    irq_counter: u16,
    irq_enabled: bool,
    irq_pending: bool,
    /// Próxima escrita em `$C800` é a metade baixa.
    irq_low: bool,
}

impl Sunsoft3 {
    pub fn new() -> Self {
        Sunsoft3::default()
    }
}

impl Mapper for Sunsoft3 {
    fn prg_offset(&self, addr: u16, data: &CartData) -> Option<usize> {
        match addr {
            0x8000..=0xBFFF => Some(self.prg as usize * 0x4000 + (addr & 0x3FFF) as usize),
            0xC000..=0xFFFF => Some(data.prg_16k().saturating_sub(1) * 0x4000 + (addr & 0x3FFF) as usize),
            _ => None,
        }
    }

    fn cpu_read(&self, addr: u16, data: &CartData) -> Option<u8> {
        self.prg_offset(addr, data).map(|off| data.prg_at(off))
    }

    fn cpu_write(&mut self, addr: u16, val: u8, data: &mut CartData) -> bool {
        // Só os bits 11-14 do endereço são decodificados
        match addr & 0xF800 {
            0x8800 => self.chr[0] = val,
            0x9800 => self.chr[1] = val,
            0xA800 => self.chr[2] = val,
            0xB800 => self.chr[3] = val,
            0xC800 => {
                if self.irq_low {
                    self.irq_counter = (self.irq_counter & 0xFF00) | val as u16;
                } else {
                    self.irq_counter = (self.irq_counter & 0x00FF) | ((val as u16) << 8);
                }
                self.irq_low = !self.irq_low;
            }
            0xD800 => {
                self.irq_enabled = val & 0x10 != 0;
                self.irq_low = false;
                self.irq_pending = false;
            }
            0xE800 => {
                data.mirror = match val & 0x03 {
                    0 => Mirror::Vertical,
                    1 => Mirror::Horizontal,
                    2 => Mirror::OneScreenLo,
                    _ => Mirror::OneScreenHi,
                }
            }
            0xF800 => self.prg = val,
            _ => return false,
        }
        true
    }

    #[inline]
    fn chr_offset(&self, addr: u16) -> usize {
        self.chr[(addr >> 11) as usize & 3] as usize * 0x0800 + (addr & 0x07FF) as usize
    }

    fn wants_cpu_clock(&self) -> bool {
        true
    }

    #[inline]
    fn cpu_clock(&mut self) {
        if self.irq_enabled {
            let (next, borrow) = self.irq_counter.overflowing_sub(1);
            self.irq_counter = next;
            if borrow {
                self.irq_pending = true;
                self.irq_enabled = false;
            }
        }
    }

    fn irq_pending(&self) -> bool {
        self.irq_pending
    }

    fn reset(&mut self, _data: &mut CartData) {
        *self = Sunsoft3::new();
    }

    fn state_string(&self) -> String {
        format!("  Sunsoft-3 PRG: {}  CHR: {:?}  IRQ: {}\n", self.prg, self.chr, self.irq_counter)
    }
}
