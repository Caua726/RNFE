//! Mappers 021/022/023/025 (Konami VRC2/VRC4): PRG 2 × 8 KB comutáveis, CHR 8 × 1 KB com
//! registradores de 4+4 bits, mirroring e (VRC4) IRQ igual ao do VRC6. As placas trocam as
//! linhas A0/A1 do registrador: cada número iNES tem sua permutação (e o submapper NES 2.0
//! desambigua VRC2 de VRC4 e as variantes).
use super::{CartData, Mapper};
use crate::cartridge::Mirror;

#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[derive(Clone)]
pub struct Vrc4 {
    /// Bits de endereço usados como A0 e A1 do registrador (ex.: VRC4a = A1,A2).
    a0: u16,
    a1: u16,
    /// VRC2 (sem IRQ e com o registrador de mirroring de 1 bit).
    vrc2: bool,
    /// VRC2a guarda o banco de CHR deslocado (a linha CHR A10 não é ligada).
    vrc2a: bool,
    prg: [u8; 2],
    swap: bool,
    chr: [u16; 8],
    irq_latch: u8,
    irq_counter: u8,
    irq_enabled: bool,
    irq_enable_after_ack: bool,
    irq_cycle_mode: bool,
    irq_pending: bool,
    prescaler: i16,
}

impl Vrc4 {
    pub fn new(data: &CartData) -> Self {
        // Cada placa liga o registrador a um par de linhas de endereço diferente. Com o
        // submapper (NES 2.0) sabemos qual é; sem ele, o jeito usado pelos emuladores é aceitar
        // os dois pares do mesmo número iNES (máscara com os dois bits): quem escreve num
        // endereço da outra variante acerta o mesmo registrador.
        let (a0, a1, vrc2) = match (data.mapper, data.submapper) {
            (21, 1) => (0x02, 0x04, false), // VRC4a: A1, A2
            (21, 2) => (0x40, 0x80, false), // VRC4c: A6, A7
            (21, _) => (0x02 | 0x40, 0x04 | 0x80, false),
            (22, _) => (0x02, 0x01, true),  // VRC2a: A1, A0 (e CHR em 2 KB)
            (23, 1) => (0x01, 0x02, false), // VRC4f: A0, A1
            (23, 2) => (0x04, 0x08, false), // VRC4e: A2, A3
            (23, 3) => (0x01, 0x02, true),  // VRC2b: A0, A1
            (23, _) => (0x01 | 0x04, 0x02 | 0x08, false),
            (25, 1) => (0x02, 0x01, false), // VRC4b: A1, A0
            (25, 2) => (0x08, 0x04, false), // VRC4d: A3, A2
            (25, 3) => (0x02, 0x01, true),  // VRC2c: A1, A0
            (25, _) => (0x02 | 0x08, 0x01 | 0x04, false),
            _ => (0x01, 0x02, false),
        };
        Vrc4 {
            a0,
            a1,
            vrc2,
            vrc2a: vrc2 && data.mapper == 22,
            prg: [0, 1],
            swap: false,
            chr: [0, 1, 2, 3, 4, 5, 6, 7],
            irq_latch: 0,
            irq_counter: 0,
            irq_enabled: false,
            irq_enable_after_ack: false,
            irq_cycle_mode: false,
            irq_pending: false,
            prescaler: 341,
        }
    }

    pub fn is_vrc2(&self) -> bool {
        self.vrc2
    }

    fn irq_clock(&mut self) {
        if self.irq_counter == 0xFF {
            self.irq_counter = self.irq_latch;
            self.irq_pending = true;
        } else {
            self.irq_counter += 1;
        }
    }
}

impl Mapper for Vrc4 {
    #[inline]
    fn prg_offset(&self, addr: u16, data: &CartData) -> Option<usize> {
        if addr < 0x8000 {
            return None;
        }
        let last = data.prg_8k().saturating_sub(1);
        let bank = match (addr >> 13) & 3 {
            0 => {
                if self.swap {
                    last.saturating_sub(1)
                } else {
                    self.prg[0] as usize
                }
            }
            1 => self.prg[1] as usize,
            2 => {
                if self.swap {
                    self.prg[0] as usize
                } else {
                    last.saturating_sub(1)
                }
            }
            _ => last,
        };
        Some(bank * 0x2000 + (addr & 0x1FFF) as usize)
    }

    fn cpu_read(&self, addr: u16, data: &CartData) -> Option<u8> {
        if addr < 0x8000 {
            return None;
        }
        let last = data.prg_8k().saturating_sub(1);
        let bank = match (addr >> 13) & 3 {
            0 => {
                if self.swap {
                    last.saturating_sub(1)
                } else {
                    self.prg[0] as usize
                }
            }
            1 => self.prg[1] as usize,
            2 => {
                if self.swap {
                    self.prg[0] as usize
                } else {
                    last.saturating_sub(1)
                }
            }
            _ => last,
        };
        Some(data.prg_at(bank * 0x2000 + (addr & 0x1FFF) as usize))
    }

    fn cpu_write(&mut self, addr: u16, val: u8, data: &mut CartData) -> bool {
        if addr < 0x8000 {
            return false;
        }
        let r = ((addr & self.a0 != 0) as u16) | (((addr & self.a1 != 0) as u16) << 1);
        match addr & 0xF000 {
            0x8000 => self.prg[0] = val & 0x1F,
            0x9000 => {
                if r == 2 && !self.vrc2 {
                    self.swap = val & 0x02 != 0;
                } else if r < 2 || self.vrc2 {
                    data.mirror = match val & 0x03 {
                        0 => Mirror::Vertical,
                        1 => Mirror::Horizontal,
                        2 => Mirror::OneScreenLo,
                        _ => Mirror::OneScreenHi,
                    };
                }
            }
            0xA000 => self.prg[1] = val & 0x1F,
            0xB000..=0xE000 => {
                let slot = (((addr >> 12) - 0xB) * 2 + (r >> 1)) as usize;
                let cur = self.chr[slot];
                self.chr[slot] = if r & 1 == 0 {
                    (cur & 0x1F0) | (val as u16 & 0x0F)
                } else {
                    (cur & 0x00F) | ((val as u16 & 0x1F) << 4)
                };
            }
            0xF000 => match r {
                0 => self.irq_latch = (self.irq_latch & 0xF0) | (val & 0x0F),
                1 => self.irq_latch = (self.irq_latch & 0x0F) | (val << 4),
                2 => {
                    self.irq_enable_after_ack = val & 0x01 != 0;
                    self.irq_enabled = val & 0x02 != 0;
                    self.irq_cycle_mode = val & 0x04 != 0;
                    if self.irq_enabled {
                        self.irq_counter = self.irq_latch;
                        self.prescaler = 341;
                    }
                    self.irq_pending = false;
                }
                _ => {
                    self.irq_pending = false;
                    self.irq_enabled = self.irq_enable_after_ack;
                }
            },
            _ => {}
        }
        true
    }

    #[inline]
    fn chr_offset(&self, addr: u16) -> usize {
        let bank = self.chr[(addr >> 10) as usize & 7] as usize;
        let bank = if self.vrc2a { bank >> 1 } else { bank };
        bank * 0x0400 + (addr & 0x03FF) as usize
    }

    fn wants_cpu_clock(&self) -> bool {
        !self.vrc2
    }

    #[inline]
    fn cpu_clock(&mut self) {
        if self.irq_enabled {
            if self.irq_cycle_mode {
                self.irq_clock();
            } else {
                self.prescaler -= 3;
                if self.prescaler <= 0 {
                    self.prescaler += 341;
                    self.irq_clock();
                }
            }
        }
    }

    #[inline]
    fn irq_pending(&self) -> bool {
        self.irq_pending
    }

    fn reset(&mut self, data: &mut CartData) {
        *self = Vrc4::new(data);
    }

    fn state_string(&self) -> String {
        format!(
            "  VRC2/4 PRG {:?} swap {} CHR {:?}  IRQ latch={} counter={} en={} pending={}\n",
            self.prg,
            self.swap,
            self.chr,
            self.irq_latch,
            self.irq_counter,
            self.irq_enabled,
            self.irq_pending
        )
    }
}
