//! Mapper 069 (FME-7 / Sunsoft 5B): 4 bancos de PRG de 8 KB, 8 de CHR de 1 KB, IRQ por contador,
//! e o áudio do 5B (3 canais quadrados do YM2149; `$C000` escolhe o registrador, `$E000` escreve).
use super::{CartData, Mapper};
use crate::cartridge::Mirror;

#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[derive(Clone)]
pub struct Fme7 {
    command: u8,
    prg_banks: [u8; 4],
    chr_banks: [u8; 8],
    /// Contador de 16 bits decrementado a cada ciclo de CPU (se `irq_count_enabled`);
    /// ao passar de $0000 para $FFFF dispara o IRQ (se `irq_enabled`).
    irq_counter: u16,
    irq_count_enabled: bool,
    irq_enabled: bool,
    irq_pending: bool,
    audio: Sunsoft5b,
}

/// YM2149 reduzido: 3 tons quadrados com período de 12 bits e volume logarítmico de 4 bits.
/// (Sem envelope nem ruído: os jogos do 5B usam praticamente só os tons.)
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[derive(Clone, Default)]
struct Sunsoft5b {
    reg_select: u8,
    regs: [u8; 16],
    /// Divisor de 8 ciclos de CPU (o tom vira a cada 8 × período).
    divider: u8,
    counters: [u16; 3],
    outputs: [bool; 3],
}

impl Sunsoft5b {
    fn write(&mut self, val: u8) {
        self.regs[(self.reg_select & 0x0F) as usize] = val;
    }

    fn clock(&mut self) {
        self.divider = self.divider.wrapping_add(1);
        if self.divider & 7 != 0 {
            return;
        }
        for ch in 0..3 {
            let period = (self.regs[ch * 2] as u16 | ((self.regs[ch * 2 + 1] as u16 & 0x0F) << 8)).max(1);
            self.counters[ch] += 1;
            if self.counters[ch] >= period {
                self.counters[ch] = 0;
                self.outputs[ch] = !self.outputs[ch];
            }
        }
    }

    fn output(&self) -> f32 {
        let mut sum = 0.0;
        for ch in 0..3 {
            let enabled = self.regs[7] & (1 << ch) == 0; // ativo em 0
            if !enabled || !self.outputs[ch] {
                continue;
            }
            let vol = (self.regs[8 + ch] & 0x0F) as usize;
            if vol > 0 {
                sum += VOLUME[vol];
            }
        }
        sum * 0.12
    }
}

/// ~3 dB por passo (AY/YM): amplitude relativa de cada nível de volume.
const VOLUME: [f32; 16] = [
    0.0, 0.0079, 0.0112, 0.0158, 0.0224, 0.0316, 0.0447, 0.0631, 0.0891, 0.1259, 0.1778, 0.2512, 0.3548,
    0.5012, 0.7079, 1.0,
];

impl Fme7 {
    pub fn new() -> Self {
        Fme7 {
            command: 0,
            prg_banks: [0; 4],
            chr_banks: [0; 8],
            irq_counter: 0,
            irq_count_enabled: false,
            irq_enabled: false,
            irq_pending: false,
            audio: Sunsoft5b::default(),
        }
    }
}

impl Default for Fme7 {
    fn default() -> Self {
        Self::new()
    }
}

impl Mapper for Fme7 {
    #[inline]
    fn prg_offset(&self, addr: u16, data: &CartData) -> Option<usize> {
        let bank = match addr {
            0x8000..=0x9FFF => (self.prg_banks[1] & 0x3F) as usize,
            0xA000..=0xBFFF => (self.prg_banks[2] & 0x3F) as usize,
            0xC000..=0xDFFF => (self.prg_banks[3] & 0x3F) as usize,
            0xE000..=0xFFFF => data.prg_8k() - 1,
            _ => return None,
        };
        Some(bank * 0x2000 + (addr & 0x1FFF) as usize)
    }

    fn cpu_read(&self, addr: u16, data: &CartData) -> Option<u8> {
        let bank = match addr {
            0x6000..=0x7FFF => {
                let b = self.prg_banks[0];
                if b & 0x40 != 0 {
                    // RAM (bit 7 = habilitada)
                    return if b & 0x80 != 0 {
                        Some(data.prg_ram_at((addr & 0x1FFF) as usize))
                    } else {
                        None
                    };
                }
                (b & 0x3F) as usize
            }
            0x8000..=0x9FFF => (self.prg_banks[1] & 0x3F) as usize,
            0xA000..=0xBFFF => (self.prg_banks[2] & 0x3F) as usize,
            0xC000..=0xDFFF => (self.prg_banks[3] & 0x3F) as usize,
            0xE000..=0xFFFF => data.prg_8k() - 1,
            _ => return None,
        };
        Some(data.prg_at(bank * 0x2000 + (addr & 0x1FFF) as usize))
    }

    fn cpu_write(&mut self, addr: u16, val: u8, data: &mut CartData) -> bool {
        match addr {
            0x6000..=0x7FFF => {
                if self.prg_banks[0] & 0xC0 == 0xC0 {
                    data.prg_ram_set((addr & 0x1FFF) as usize, val);
                }
                true
            }
            0x8000..=0x9FFF => {
                self.command = val & 0x0F;
                true
            }
            0xA000..=0xBFFF => {
                match self.command {
                    c @ 0..=7 => self.chr_banks[c as usize] = val,
                    8 => self.prg_banks[0] = val,
                    9 => self.prg_banks[1] = val,
                    0xA => self.prg_banks[2] = val,
                    0xB => self.prg_banks[3] = val,
                    0xC => {
                        data.mirror = match val & 0x03 {
                            0 => Mirror::Vertical,
                            1 => Mirror::Horizontal,
                            2 => Mirror::OneScreenLo,
                            _ => Mirror::OneScreenHi,
                        };
                    }
                    0xD => {
                        self.irq_enabled = val & 0x01 != 0;
                        self.irq_count_enabled = val & 0x80 != 0;
                        self.irq_pending = false; // ack
                    }
                    0xE => self.irq_counter = (self.irq_counter & 0xFF00) | val as u16,
                    _ => self.irq_counter = (self.irq_counter & 0x00FF) | ((val as u16) << 8),
                }
                true
            }
            0xC000..=0xDFFF => {
                self.audio.reg_select = val;
                true
            }
            0xE000..=0xFFFF => {
                self.audio.write(val);
                true
            }
            _ => false,
        }
    }

    fn wants_cpu_clock(&self) -> bool {
        true
    }

    #[inline]
    fn has_audio(&self) -> bool {
        true
    }

    fn audio_output(&self) -> f32 {
        self.audio.output()
    }

    #[inline]
    fn cpu_clock(&mut self) {
        self.audio.clock();
        if self.irq_count_enabled {
            let (v, wrapped) = self.irq_counter.overflowing_sub(1);
            self.irq_counter = v;
            if wrapped && self.irq_enabled {
                self.irq_pending = true;
            }
        }
    }

    #[inline]
    fn irq_pending(&self) -> bool {
        self.irq_pending
    }

    fn manages_prg_ram(&self) -> bool {
        true
    }

    #[inline]
    fn chr_offset(&self, addr: u16) -> usize {
        self.chr_banks[(addr >> 10) as usize & 7] as usize * 0x0400 + (addr & 0x03FF) as usize
    }

    fn reset(&mut self, _data: &mut CartData) {
        self.command = 0;
        self.prg_banks = [0; 4];
        self.chr_banks = [0; 8];
        self.irq_counter = 0;
        self.irq_count_enabled = false;
        self.irq_enabled = false;
        self.irq_pending = false;
        self.audio = Sunsoft5b::default();
    }

    fn state_string(&self) -> String {
        format!(
            "  FME-7 cmd: {}  PRG: {:?}  CHR: {:?}  IRQ: counter=${:04X} count={} irq={} pending={}\n",
            self.command,
            self.prg_banks,
            self.chr_banks,
            self.irq_counter,
            self.irq_count_enabled,
            self.irq_enabled,
            self.irq_pending
        )
    }
}
