//! Mapper 019 (Namco 163): CHR/nametables de 1 KB por registrador (CIRAM ou CHR ROM),
//! IRQ por contador de 15 bits, e o áudio wavetable de até 8 canais multiplexados.
use super::{CartData, Mapper};
use crate::cartridge::{Mirror, NtSource};

#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[derive(Clone)]
pub struct N163 {
    /// R0-R7: CHR `$0000-$1FFF`; R8-R11: nametables `$2000-$2FFF`.
    banks: [u8; 12],
    prg: [u8; 3],
    /// `$E800` bits 6/7: valores ≥ $E0 em R0-R3 / R4-R7 NÃO viram CIRAM.
    chr_ram_disable: u8,
    irq_counter: u16,
    irq_enabled: bool,
    irq_pending: bool,
    /// `$F800`: endereço da RAM interna (bits 0-6) e auto-incremento (bit 7).
    ram_addr: u8,
    /// Bits de proteção de escrita da PRG RAM (um por 2 KB), do `$F800 = $4x`.
    ram_protect: u8,
    sound_disabled: bool,
    #[cfg_attr(feature = "serde", serde(with = "crate::state::bytes"))]
    ram: [u8; 128],
    /// Multiplexação: um canal atualizado a cada 15 ciclos, do 7 para baixo.
    audio_divider: u8,
    audio_channel: u8,
    outputs: [i16; 8],
}

fn default_banks(mirror: Mirror) -> [u8; 12] {
    let mut b = [0, 1, 2, 3, 4, 5, 6, 7, 0xE0, 0xE0, 0xE1, 0xE1];
    if mirror == Mirror::Vertical {
        b[9] = 0xE1;
        b[10] = 0xE0;
    }
    b
}

impl N163 {
    pub fn new(data: &CartData) -> Self {
        N163 {
            banks: default_banks(data.mirror),
            prg: [0, 1, 2],
            chr_ram_disable: 0,
            irq_counter: 0,
            irq_enabled: false,
            irq_pending: false,
            ram_addr: 0,
            ram_protect: 0,
            sound_disabled: false,
            ram: [0; 128],
            audio_divider: 0,
            audio_channel: 7,
            outputs: [0; 8],
        }
    }

    fn channels(&self) -> u8 {
        ((self.ram[0x7F] >> 4) & 7) + 1
    }

    fn clock_channel(&mut self, ch: usize) {
        let base = 0x40 + ch * 8;
        let freq = self.ram[base] as u32
            | (self.ram[base + 2] as u32) << 8
            | ((self.ram[base + 4] as u32 & 3) << 16);
        let length = 256 - (self.ram[base + 4] as u32 & 0xFC);
        let offset = self.ram[base + 6] as u32;
        let vol = (self.ram[base + 7] & 0x0F) as i16;
        let mut phase =
            self.ram[base + 1] as u32 | (self.ram[base + 3] as u32) << 8 | (self.ram[base + 5] as u32) << 16;
        phase = (phase + freq) % (length << 16).max(1);
        self.ram[base + 1] = phase as u8;
        self.ram[base + 3] = (phase >> 8) as u8;
        self.ram[base + 5] = (phase >> 16) as u8;
        let sample_addr = ((phase >> 16) + offset) as usize & 0xFF;
        let nibble = (self.ram[sample_addr >> 1] >> ((sample_addr & 1) * 4)) & 0x0F;
        self.outputs[ch] = (nibble as i16 - 8) * vol;
    }
}

impl Mapper for N163 {
    #[inline]
    fn cpu_read(&self, addr: u16, data: &CartData) -> Option<u8> {
        match addr {
            0x4800..=0x4FFF => Some(self.ram[(self.ram_addr & 0x7F) as usize]),
            0x5000..=0x57FF => Some(self.irq_counter as u8),
            0x5800..=0x5FFF => {
                Some(((self.irq_counter >> 8) as u8 & 0x7F) | if self.irq_enabled { 0x80 } else { 0 })
            }
            0x6000..=0x7FFF => Some(data.prg_ram_at((addr & 0x1FFF) as usize)),
            0x8000..=0xDFFF => {
                let bank = self.prg[((addr - 0x8000) >> 13) as usize] as usize & 0x3F;
                Some(data.prg_at(bank * 0x2000 + (addr & 0x1FFF) as usize))
            }
            0xE000..=0xFFFF => Some(data.prg_at((data.prg_8k() - 1) * 0x2000 + (addr & 0x1FFF) as usize)),
            _ => None,
        }
    }

    fn cpu_write(&mut self, addr: u16, val: u8, data: &mut CartData) -> bool {
        match addr {
            0x4800..=0x4FFF => {
                self.ram[(self.ram_addr & 0x7F) as usize] = val;
                if self.ram_addr & 0x80 != 0 {
                    self.ram_addr = 0x80 | (self.ram_addr.wrapping_add(1) & 0x7F);
                }
            }
            0x5000..=0x57FF => {
                self.irq_counter = (self.irq_counter & 0x7F00) | val as u16;
                self.irq_pending = false;
            }
            0x5800..=0x5FFF => {
                self.irq_counter = (self.irq_counter & 0x00FF) | ((val as u16 & 0x7F) << 8);
                self.irq_enabled = val & 0x80 != 0;
                self.irq_pending = false;
            }
            0x6000..=0x7FFF => {
                let block = ((addr - 0x6000) >> 11) as u8; // 2 KB
                if self.ram_protect & (1 << block) == 0 {
                    data.prg_ram_set((addr & 0x1FFF) as usize, val);
                }
            }
            0x8000..=0xDFFF => self.banks[((addr - 0x8000) >> 11) as usize] = val,
            0xE000..=0xE7FF => {
                self.prg[0] = val & 0x3F;
                self.sound_disabled = val & 0x40 != 0;
            }
            0xE800..=0xEFFF => {
                self.prg[1] = val & 0x3F;
                self.chr_ram_disable = val & 0xC0;
            }
            0xF000..=0xF7FF => self.prg[2] = val & 0x3F,
            0xF800..=0xFFFF => {
                self.ram_addr = val;
                // $F800 = $4x: bits 0-3 protegem cada bloco de 2 KB da PRG RAM
                self.ram_protect = if val & 0xF0 == 0x40 { val & 0x0F } else { 0 };
            }
            _ => return false,
        }
        true
    }

    fn chr_dynamic(&self) -> bool {
        true
    }

    fn manages_prg_ram(&self) -> bool {
        true
    }

    #[inline]
    fn chr_offset(&self, addr: u16) -> usize {
        let slot = (addr >> 10) as usize & 7;
        self.banks[slot] as usize * 0x0400 + (addr & 0x03FF) as usize
    }

    #[inline]
    fn ppu_read(&mut self, addr: u16, data: &CartData) -> u8 {
        let slot = (addr >> 10) as usize & 7;
        let bank = self.banks[slot];
        let disable = if slot < 4 { 0x40 } else { 0x80 };
        if bank >= 0xE0 && self.chr_ram_disable & disable == 0 {
            // CIRAM mapeada em $0000-$1FFF: não temos a VRAM aqui; devolve 0 (raro em jogos)
            0
        } else {
            data.chr_at(bank as usize * 0x0400 + (addr & 0x03FF) as usize)
        }
    }

    #[inline]
    fn nt_source(&mut self, addr: u16, _data: &CartData) -> Option<NtSource> {
        let r = self.banks[8 + ((addr >> 10) as usize & 3)];
        Some(if r >= 0xE0 {
            NtSource::Ciram(r & 1)
        } else {
            NtSource::Chr(r as usize * 0x0400 + (addr & 0x03FF) as usize)
        })
    }

    fn wants_cpu_clock(&self) -> bool {
        true
    }

    #[inline]
    fn cpu_clock(&mut self) {
        if self.irq_enabled && self.irq_counter < 0x7FFF {
            self.irq_counter += 1;
            if self.irq_counter == 0x7FFF {
                self.irq_pending = true;
            }
        }
        self.audio_divider += 1;
        if self.audio_divider >= 15 {
            self.audio_divider = 0;
            if !self.sound_disabled {
                let ch = self.audio_channel as usize;
                self.clock_channel(ch);
                let first = 8 - self.channels();
                self.audio_channel = if self.audio_channel <= first { 7 } else { self.audio_channel - 1 };
            }
        }
    }

    #[inline]
    fn irq_pending(&self) -> bool {
        self.irq_pending
    }

    #[inline]
    fn audio_output(&self) -> f32 {
        if self.sound_disabled {
            return 0.0;
        }
        let n = self.channels() as usize;
        let sum: i32 = self.outputs[8 - n..].iter().map(|&o| o as i32).sum();
        sum as f32 / n as f32 * 0.0028
    }

    fn reset(&mut self, data: &mut CartData) {
        *self = N163::new(data);
    }

    fn state_string(&self) -> String {
        format!(
            "  N163 PRG: {:?}  CHR/NT: {:02X?}  IRQ: counter=${:04X} en={} pending={}  canais={}\n",
            self.prg,
            self.banks,
            self.irq_counter,
            self.irq_enabled,
            self.irq_pending,
            self.channels()
        )
    }
}
