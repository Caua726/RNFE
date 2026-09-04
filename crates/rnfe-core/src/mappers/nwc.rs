//! Mapper 105 (NES-EVENT, Nintendo World Championships 1990): um MMC1 com 256 KB de PRG em
//! duas metades, CHR RAM fixa e um contador de IRQ de 30 bits (o cronômetro do torneio).
//!
//! Registrador de CHR 0 (`$A000`): bit 4 = zera e desliga o contador (1) / conta (0); bit 3 =
//! modo de PRG (0: 32 KB dos bits 1-2 na 1ª metade de 128 KB; 1: MMC1 normal na 2ª metade).
//! No power-on os primeiros 32 KB ficam fixos até o jogo alternar o bit 4 (1→0→1).
use super::{CartData, Mapper};
use crate::cartridge::Mirror;

#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[derive(Clone)]
pub struct Nwc {
    shift: u8,
    shift_count: u8,
    control: u8,
    reg_a: u8,
    prg_bank: u8,
    /// 0 = power-on (bancos fixos), 1 = viu o bit 4 baixar, 2 = viu subir de novo (liberado).
    init: u8,
    irq_counter: u32,
    irq_pending: bool,
    /// Bit do contador que dispara o IRQ (chaves DIP: 25–29; o torneio usava 29).
    irq_bit: u8,
}

impl Nwc {
    pub fn new(_data: &CartData) -> Self {
        Nwc {
            shift: 0x10,
            shift_count: 0,
            control: 0x0C,
            reg_a: 0x10,
            prg_bank: 0,
            init: 0,
            irq_counter: 0,
            irq_pending: false,
            irq_bit: 29,
        }
    }

    fn prg_offset(&self, addr: u16, data: &CartData) -> usize {
        let off = (addr & 0x7FFF) as usize;
        if self.init < 2 {
            return off; // 32 KB iniciais
        }
        if self.reg_a & 0x08 == 0 {
            return ((self.reg_a >> 1) & 3) as usize * 0x8000 + off;
        }
        // MMC1 normal na 2ª metade (bancos de 16 KB 8-15)
        let base = 8usize;
        let bank = base + (self.prg_bank & 0x07) as usize;
        let last = base + 7;
        let _ = data;
        match (self.control >> 2) & 3 {
            0 | 1 => (base + (self.prg_bank & 0x06) as usize) * 0x4000 + off,
            2 => {
                if addr < 0xC000 {
                    base * 0x4000 + (off & 0x3FFF)
                } else {
                    bank * 0x4000 + (off & 0x3FFF)
                }
            }
            _ => {
                if addr < 0xC000 {
                    bank * 0x4000 + (off & 0x3FFF)
                } else {
                    last * 0x4000 + (off & 0x3FFF)
                }
            }
        }
    }
}

impl Mapper for Nwc {
    #[inline]
    fn cpu_read(&self, addr: u16, data: &CartData) -> Option<u8> {
        match addr {
            0x6000..=0x7FFF => Some(data.prg_ram_at((addr & 0x1FFF) as usize)),
            0x8000..=0xFFFF => Some(data.prg_at(self.prg_offset(addr, data))),
            _ => None,
        }
    }

    fn cpu_write(&mut self, addr: u16, val: u8, data: &mut CartData) -> bool {
        if (0x6000..=0x7FFF).contains(&addr) {
            data.prg_ram_set((addr & 0x1FFF) as usize, val);
            return true;
        }
        if addr < 0x8000 {
            return false;
        }
        if val & 0x80 != 0 {
            self.shift = 0x10;
            self.shift_count = 0;
            self.control |= 0x0C;
            return true;
        }
        self.shift >>= 1;
        self.shift |= (val & 1) << 4;
        self.shift_count += 1;
        if self.shift_count == 5 {
            let v = self.shift;
            match addr {
                0x8000..=0x9FFF => {
                    self.control = v;
                    data.mirror = match v & 3 {
                        0 => Mirror::OneScreenLo,
                        1 => Mirror::OneScreenHi,
                        2 => Mirror::Vertical,
                        _ => Mirror::Horizontal,
                    };
                }
                0xA000..=0xBFFF => {
                    let was = self.reg_a & 0x10 != 0;
                    let now = v & 0x10 != 0;
                    self.reg_a = v;
                    if was && !now && self.init == 0 {
                        self.init = 1;
                    } else if !was && now && self.init == 1 {
                        self.init = 2;
                    }
                    if now {
                        self.irq_counter = 0;
                        self.irq_pending = false;
                    }
                }
                0xC000..=0xDFFF => {}
                _ => self.prg_bank = v & 0x0F,
            }
            self.shift = 0x10;
            self.shift_count = 0;
        }
        true
    }

    fn manages_prg_ram(&self) -> bool {
        true
    }

    fn wants_cpu_clock(&self) -> bool {
        true
    }

    #[inline]
    fn cpu_clock(&mut self) {
        if self.reg_a & 0x10 == 0 {
            self.irq_counter = self.irq_counter.wrapping_add(1) & 0x3FFF_FFFF;
            if self.irq_counter & (1 << self.irq_bit) != 0 {
                self.irq_pending = true;
            }
        }
    }

    #[inline]
    fn irq_pending(&self) -> bool {
        self.irq_pending
    }

    fn reset(&mut self, data: &mut CartData) {
        *self = Nwc::new(data);
    }

    fn state_string(&self) -> String {
        format!(
            "  NES-EVENT ctrl ${:02X} regA ${:02X} PRG {} init {} IRQ counter={:#x} pending={}\n",
            self.control, self.reg_a, self.prg_bank, self.init, self.irq_counter, self.irq_pending
        )
    }
}
