//! Mappers 024/026 (Konami VRC6): PRG 16 KB + 8 KB, CHR 8 × 1 KB, IRQ por scanline/ciclo e o
//! áudio de expansão (2 pulsos com duty de 16 passos + dente de serra).
//!
//! O mapper 26 é o mesmo chip com as linhas A0/A1 trocadas no barramento.
use super::{CartData, Mapper};
use crate::cartridge::Mirror;

#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[derive(Clone, Default)]
struct Pulse {
    volume: u8,
    duty: u8,
    /// Bit 7 do registrador de controle: saída constante (ignora o duty).
    mode: bool,
    period: u16,
    enabled: bool,
    timer: u16,
    step: u8,
}

impl Pulse {
    fn clock(&mut self, shift: u8) {
        if !self.enabled {
            return;
        }
        if self.timer == 0 {
            self.timer = self.period >> shift;
            self.step = (self.step + 1) & 15;
        } else {
            self.timer -= 1;
        }
    }

    fn output(&self) -> u8 {
        if !self.enabled {
            return 0;
        }
        if self.mode || self.step <= self.duty { self.volume } else { 0 }
    }
}

#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[derive(Clone, Default)]
struct Saw {
    rate: u8,
    period: u16,
    enabled: bool,
    timer: u16,
    step: u8,
    accumulator: u8,
}

impl Saw {
    fn clock(&mut self, shift: u8) {
        if !self.enabled {
            return;
        }
        if self.timer == 0 {
            self.timer = self.period >> shift;
            self.step += 1;
            if self.step & 1 == 0 {
                self.accumulator = self.accumulator.wrapping_add(self.rate);
            }
            if self.step >= 14 {
                self.step = 0;
                self.accumulator = 0;
            }
        } else {
            self.timer -= 1;
        }
    }

    fn output(&self) -> u8 {
        if self.enabled { self.accumulator >> 3 } else { 0 }
    }
}

#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[derive(Clone)]
pub struct Vrc6 {
    swap_a0_a1: bool,
    prg_16k: u8,
    prg_8k: u8,
    chr: [u8; 8],
    /// `$B003`: bits 0-1 modo de CHR, bit 5 estilo, bits 2-3 mirroring, bit 7 PRG RAM.
    ppu_ctrl: u8,
    irq_latch: u8,
    irq_counter: u8,
    irq_enabled: bool,
    irq_enable_after_ack: bool,
    irq_cycle_mode: bool,
    irq_pending: bool,
    /// Prescaler do modo scanline: 341 dots = 113⅔ ciclos (114, 114, 113).
    prescaler: i16,
    pulse1: Pulse,
    pulse2: Pulse,
    saw: Saw,
    /// `$9003`: bit 0 congela, bit 1 divide o período por 16, bit 2 por 256.
    halt: bool,
    freq_shift: u8,
}

impl Vrc6 {
    pub fn new(data: &CartData) -> Self {
        Vrc6 {
            swap_a0_a1: data.mapper == 26,
            prg_16k: 0,
            prg_8k: 0,
            chr: [0, 1, 2, 3, 4, 5, 6, 7],
            ppu_ctrl: 0,
            irq_latch: 0,
            irq_counter: 0,
            irq_enabled: false,
            irq_enable_after_ack: false,
            irq_cycle_mode: false,
            irq_pending: false,
            prescaler: 341,
            pulse1: Pulse::default(),
            pulse2: Pulse::default(),
            saw: Saw::default(),
            halt: false,
            freq_shift: 0,
        }
    }

    pub fn is_26(&self) -> bool {
        self.swap_a0_a1
    }

    fn prg_ram_enabled(&self) -> bool {
        self.ppu_ctrl & 0x80 != 0
    }

    fn write_pulse(p: &mut Pulse, reg: u16, val: u8) {
        match reg {
            0 => {
                p.volume = val & 0x0F;
                p.duty = (val >> 4) & 0x07;
                p.mode = val & 0x80 != 0;
            }
            1 => p.period = (p.period & 0x0F00) | val as u16,
            _ => {
                p.period = (p.period & 0x00FF) | ((val as u16 & 0x0F) << 8);
                p.enabled = val & 0x80 != 0;
                if !p.enabled {
                    p.step = 0;
                }
            }
        }
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

impl Mapper for Vrc6 {
    #[inline]
    fn cpu_read(&self, addr: u16, data: &CartData) -> Option<u8> {
        match addr {
            0x6000..=0x7FFF => {
                if self.prg_ram_enabled() {
                    Some(data.prg_ram_at((addr & 0x1FFF) as usize))
                } else {
                    None
                }
            }
            0x8000..=0xBFFF => Some(data.prg_at(self.prg_16k as usize * 0x4000 + (addr & 0x3FFF) as usize)),
            0xC000..=0xDFFF => Some(data.prg_at(self.prg_8k as usize * 0x2000 + (addr & 0x1FFF) as usize)),
            0xE000..=0xFFFF => Some(data.prg_at((data.prg_8k() - 1) * 0x2000 + (addr & 0x1FFF) as usize)),
            _ => None,
        }
    }

    fn cpu_write(&mut self, addr: u16, val: u8, data: &mut CartData) -> bool {
        if (0x6000..=0x7FFF).contains(&addr) {
            if self.prg_ram_enabled() {
                data.prg_ram_set((addr & 0x1FFF) as usize, val);
            }
            return true;
        }
        if addr < 0x8000 {
            return false;
        }
        let addr =
            if self.swap_a0_a1 { (addr & 0xFFFC) | ((addr & 1) << 1) | ((addr >> 1) & 1) } else { addr };
        let reg = addr & 0x0003;
        match addr & 0xF000 {
            0x8000 => self.prg_16k = val & 0x0F,
            0x9000 => {
                if reg == 3 {
                    self.halt = val & 0x01 != 0;
                    self.freq_shift = if val & 0x04 != 0 {
                        8
                    } else if val & 0x02 != 0 {
                        4
                    } else {
                        0
                    };
                } else {
                    Self::write_pulse(&mut self.pulse1, reg, val);
                }
            }
            0xA000 => Self::write_pulse(&mut self.pulse2, reg, val),
            0xB000 => match reg {
                0 => self.saw.rate = val & 0x3F,
                1 => self.saw.period = (self.saw.period & 0x0F00) | val as u16,
                2 => {
                    self.saw.period = (self.saw.period & 0x00FF) | ((val as u16 & 0x0F) << 8);
                    self.saw.enabled = val & 0x80 != 0;
                    if !self.saw.enabled {
                        self.saw.step = 0;
                        self.saw.accumulator = 0;
                    }
                }
                _ => {
                    self.ppu_ctrl = val;
                    data.mirror = match (val >> 2) & 0x03 {
                        0 => Mirror::Vertical,
                        1 => Mirror::Horizontal,
                        2 => Mirror::OneScreenLo,
                        _ => Mirror::OneScreenHi,
                    };
                }
            },
            0xC000 => self.prg_8k = val & 0x1F,
            0xD000 => self.chr[reg as usize] = val,
            0xE000 => self.chr[4 + reg as usize] = val,
            0xF000 => match reg {
                0 => self.irq_latch = val,
                1 => {
                    self.irq_enable_after_ack = val & 0x01 != 0;
                    self.irq_enabled = val & 0x02 != 0;
                    self.irq_cycle_mode = val & 0x04 != 0;
                    if self.irq_enabled {
                        self.irq_counter = self.irq_latch;
                        self.prescaler = 341;
                    }
                    self.irq_pending = false;
                }
                2 => {
                    self.irq_pending = false;
                    self.irq_enabled = self.irq_enable_after_ack;
                }
                _ => {}
            },
            _ => {}
        }
        true
    }

    fn manages_prg_ram(&self) -> bool {
        true
    }

    #[inline]
    fn chr_offset(&self, addr: u16) -> usize {
        let slot = (addr >> 10) as usize & 7;
        let bank = match self.ppu_ctrl & 0x03 {
            0 => self.chr[slot] as usize,
            1 => {
                // 4 bancos de 2 KB (R0-R3); o bit 5 escolhe se o registrador conta em 1 KB ou 2 KB
                let r = self.chr[slot / 2] as usize;
                if self.ppu_ctrl & 0x20 != 0 { (r & !1) | (slot & 1) } else { r * 2 + (slot & 1) }
            }
            _ => {
                if slot < 4 {
                    self.chr[slot] as usize
                } else {
                    let r = self.chr[4 + (slot - 4) / 2] as usize;
                    if self.ppu_ctrl & 0x20 != 0 { (r & !1) | (slot & 1) } else { r * 2 + (slot & 1) }
                }
            }
        };
        bank * 0x0400 + (addr & 0x03FF) as usize
    }

    fn wants_cpu_clock(&self) -> bool {
        true
    }

    #[inline]
    fn cpu_clock(&mut self) {
        if !self.halt {
            self.pulse1.clock(self.freq_shift);
            self.pulse2.clock(self.freq_shift);
            self.saw.clock(self.freq_shift);
        }
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

    #[inline]
    fn audio_output(&self) -> f32 {
        (self.pulse1.output() as f32 + self.pulse2.output() as f32) * 0.0095
            + self.saw.output() as f32 * 0.0066
    }

    fn reset(&mut self, data: &mut CartData) {
        let swap = self.swap_a0_a1;
        *self = Vrc6::new(data);
        self.swap_a0_a1 = swap;
    }

    fn state_string(&self) -> String {
        format!(
            "  VRC6 PRG: {}/{}  CHR: {:?}  $B003: ${:02X}  IRQ: latch={} counter={} en={} cycle={} pending={}\n",
            self.prg_16k,
            self.prg_8k,
            self.chr,
            self.ppu_ctrl,
            self.irq_latch,
            self.irq_counter,
            self.irq_enabled,
            self.irq_cycle_mode,
            self.irq_pending
        )
    }
}
