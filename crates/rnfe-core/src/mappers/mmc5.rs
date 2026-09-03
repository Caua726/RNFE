//! Mapper 005 (Nintendo MMC5): PRG/CHR em vários tamanhos, PRG RAM até 64 KB, ExRAM de 1 KB
//! (nametable extra, atributos estendidos ou RAM), tile de preenchimento, multiplicador,
//! IRQ por scanline (detectada pelas 3 leituras iguais de nametable) e 2 pulsos + PCM.
//!
//! Não implementado ainda: a divisão vertical da tela (`$5200-$5202`).
use super::{CartData, Mapper};
use crate::cartridge::NtSource;

/// Pulso do MMC5: igual ao da 2A03 sem sweep.
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[derive(Clone, Default)]
struct Pulse {
    enabled: bool,
    duty: u8,
    halt: bool,
    constant: bool,
    volume: u8,
    period: u16,
    timer: u16,
    step: u8,
    length: u8,
    env_start: bool,
    env_divider: u8,
    env_decay: u8,
}

const DUTY: [[u8; 8]; 4] =
    [[0, 1, 0, 0, 0, 0, 0, 0], [0, 1, 1, 0, 0, 0, 0, 0], [0, 1, 1, 1, 1, 0, 0, 0], [1, 0, 0, 1, 1, 1, 1, 1]];
const LENGTH: [u8; 32] = [
    10, 254, 20, 2, 40, 4, 80, 6, 160, 8, 60, 10, 14, 12, 26, 14, 12, 16, 24, 18, 48, 20, 96, 22, 192, 24,
    72, 26, 16, 28, 32, 30,
];

impl Pulse {
    fn write(&mut self, reg: u16, val: u8) {
        match reg {
            0 => {
                self.duty = val >> 6;
                self.halt = val & 0x20 != 0;
                self.constant = val & 0x10 != 0;
                self.volume = val & 0x0F;
            }
            2 => self.period = (self.period & 0x0700) | val as u16,
            3 => {
                self.period = (self.period & 0x00FF) | ((val as u16 & 0x07) << 8);
                if self.enabled {
                    self.length = LENGTH[(val >> 3) as usize];
                }
                self.step = 0;
                self.env_start = true;
            }
            _ => {}
        }
    }

    fn clock_timer(&mut self) {
        if self.timer == 0 {
            self.timer = self.period;
            self.step = (self.step + 1) & 7;
        } else {
            self.timer -= 1;
        }
    }

    fn clock_quarter(&mut self) {
        if self.env_start {
            self.env_start = false;
            self.env_decay = 15;
            self.env_divider = self.volume;
        } else if self.env_divider == 0 {
            self.env_divider = self.volume;
            if self.env_decay > 0 {
                self.env_decay -= 1;
            } else if self.halt {
                self.env_decay = 15;
            }
        } else {
            self.env_divider -= 1;
        }
    }

    fn clock_half(&mut self) {
        if !self.halt && self.length > 0 {
            self.length -= 1;
        }
    }

    fn output(&self) -> u8 {
        if !self.enabled
            || self.length == 0
            || self.period < 8
            || DUTY[self.duty as usize][self.step as usize] == 0
        {
            return 0;
        }
        if self.constant { self.volume } else { self.env_decay }
    }
}

#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[derive(Clone)]
pub struct Mmc5 {
    prg_mode: u8,
    chr_mode: u8,
    prg_ram_protect: [u8; 2],
    exram_mode: u8,
    nt_map: u8,
    fill_tile: u8,
    fill_attr: u8,
    /// `$5113-$5117`: bit 7 = ROM (para $5114-$5116), bits 0-6 banco.
    prg_regs: [u8; 5],
    /// `$5120-$512B`: 12 registradores de 10 bits (com `$5130` nos bits 8-9).
    chr_regs: [u16; 12],
    chr_hi: u8,
    /// Último conjunto escrito (A = false, B = true): usado fora do modo 8×16.
    last_chr_b: bool,
    #[cfg_attr(feature = "serde", serde(with = "crate::state::bytes"))]
    exram: [u8; 1024],
    /// Byte da ExRAM do tile de fundo em curso (atributos estendidos).
    ext_tile: u8,
    multiplicand: u8,
    multiplier: u8,
    // Detecção de scanline
    last_nt_addr: u16,
    same_reads: u8,
    in_frame: bool,
    scanline: u8,
    idle_cycles: u16,
    irq_compare: u8,
    irq_enabled: bool,
    irq_pending: bool,
    // Áudio
    pulse1: Pulse,
    pulse2: Pulse,
    pcm: u8,
    pcm_read_mode: bool,
    frame_cycle: u16,
    frame_step: u8,
    cycle: u64,
}

impl Mmc5 {
    pub fn new(_data: &CartData) -> Self {
        Mmc5 {
            prg_mode: 3,
            chr_mode: 3,
            prg_ram_protect: [0, 0],
            exram_mode: 0,
            nt_map: 0,
            fill_tile: 0,
            fill_attr: 0,
            prg_regs: [0, 0x80, 0x80, 0x80, 0xFF],
            chr_regs: [0, 1, 2, 3, 4, 5, 6, 7, 0, 1, 2, 3],
            chr_hi: 0,
            last_chr_b: false,
            exram: [0; 1024],
            ext_tile: 0,
            multiplicand: 0xFF,
            multiplier: 0xFF,
            last_nt_addr: 0,
            same_reads: 0,
            in_frame: false,
            scanline: 0,
            idle_cycles: 0,
            irq_compare: 0,
            irq_enabled: false,
            irq_pending: false,
            pulse1: Pulse::default(),
            pulse2: Pulse::default(),
            pcm: 0,
            pcm_read_mode: false,
            frame_cycle: 0,
            frame_step: 0,
            cycle: 0,
        }
    }

    fn ram_writable(&self) -> bool {
        self.prg_ram_protect[0] & 3 == 2 && self.prg_ram_protect[1] & 3 == 1
    }

    /// (é ROM?, banco de 8 KB) para o endereço `$6000-$FFFF`.
    fn prg_bank(&self, addr: u16) -> (bool, usize) {
        let r = |i: usize| (self.prg_regs[i] & 0x80 != 0 || i == 4, (self.prg_regs[i] & 0x7F) as usize);
        let slot = ((addr - 0x6000) >> 13) as usize; // 0 = $6000, 1 = $8000 … 4 = $E000
        if slot == 0 {
            return (false, (self.prg_regs[0] & 0x07) as usize);
        }
        match self.prg_mode {
            0 => {
                let (rom, b) = r(4);
                (rom, (b & !3) + (slot - 1))
            }
            1 => {
                let (rom, b) = if slot <= 2 { r(2) } else { r(4) };
                (rom, (b & !1) + ((slot - 1) & 1))
            }
            2 => match slot {
                1 | 2 => {
                    let (rom, b) = r(2);
                    (rom, (b & !1) + (slot - 1))
                }
                3 => r(3),
                _ => r(4),
            },
            _ => r(slot),
        }
    }

    fn chr_bank_offset(&self, addr: u16, data: &CartData) -> usize {
        let use_b = if data.ppu_sprites_16 { !data.ppu_sprite_fetch } else { self.last_chr_b };
        // Atributos estendidos: o fundo usa o banco de 4 KB do byte da ExRAM
        if self.exram_mode == 1 && !data.ppu_sprite_fetch {
            let bank = (self.ext_tile as usize & 0x3F) | ((self.chr_hi as usize & 3) << 6);
            return bank * 0x1000 + (addr & 0x0FFF) as usize;
        }
        let a = addr as usize & 0x1FFF;
        let reg = |i: usize| self.chr_regs[i] as usize;
        let (bank, size) = match (self.chr_mode, use_b) {
            (0, false) => (reg(7), 0x2000),
            (0, true) => (reg(11), 0x2000),
            (1, false) => (reg(if a < 0x1000 { 3 } else { 7 }), 0x1000),
            (1, true) => (reg(11), 0x1000),
            (2, false) => (reg([1, 3, 5, 7][a >> 11]), 0x0800),
            (2, true) => (reg([9, 11][(a >> 11) & 1]), 0x0800),
            (_, false) => (reg(a >> 10), 0x0400),
            (_, true) => (reg(8 + ((a >> 10) & 3)), 0x0400),
        };
        bank * size + (a & (size - 1))
    }

    fn detect_scanline(&mut self) {
        if self.in_frame {
            self.scanline = self.scanline.wrapping_add(1);
            if self.scanline == self.irq_compare && self.irq_compare != 0 {
                self.irq_pending = true;
            }
        } else {
            self.in_frame = true;
            self.scanline = 0;
        }
    }

    fn end_frame(&mut self) {
        self.in_frame = false;
        self.scanline = 0;
        self.same_reads = 0;
        self.last_nt_addr = 0;
    }
}

impl Mapper for Mmc5 {
    #[inline]
    fn cpu_read(&self, addr: u16, data: &CartData) -> Option<u8> {
        match addr {
            0x5015 => Some((self.pulse1.length > 0) as u8 | ((self.pulse2.length > 0) as u8) << 1),
            0x5204 => Some(((self.irq_pending as u8) << 7) | ((self.in_frame as u8) << 6)),
            0x5205 => Some((self.multiplicand as u16 * self.multiplier as u16) as u8),
            0x5206 => Some(((self.multiplicand as u16 * self.multiplier as u16) >> 8) as u8),
            0x5C00..=0x5FFF => {
                if self.exram_mode >= 2 {
                    Some(self.exram[(addr & 0x3FF) as usize])
                } else {
                    None
                }
            }
            0x6000..=0xFFFF => {
                let (rom, bank) = self.prg_bank(addr);
                let off = bank * 0x2000 + (addr & 0x1FFF) as usize;
                Some(if rom { data.prg_at(off) } else { data.prg_ram_at(off) })
            }
            _ => None,
        }
    }

    fn cpu_write(&mut self, addr: u16, val: u8, data: &mut CartData) -> bool {
        match addr {
            0x5000..=0x5003 => self.pulse1.write(addr & 3, val),
            0x5004..=0x5007 => self.pulse2.write(addr & 3, val),
            0x5010 => self.pcm_read_mode = val & 0x01 != 0,
            0x5011 => {
                if !self.pcm_read_mode {
                    self.pcm = val;
                }
            }
            0x5015 => {
                self.pulse1.enabled = val & 1 != 0;
                self.pulse2.enabled = val & 2 != 0;
                if !self.pulse1.enabled {
                    self.pulse1.length = 0;
                }
                if !self.pulse2.enabled {
                    self.pulse2.length = 0;
                }
            }
            0x5100 => self.prg_mode = val & 3,
            0x5101 => self.chr_mode = val & 3,
            0x5102 => self.prg_ram_protect[0] = val,
            0x5103 => self.prg_ram_protect[1] = val,
            0x5104 => self.exram_mode = val & 3,
            0x5105 => self.nt_map = val,
            0x5106 => self.fill_tile = val,
            0x5107 => self.fill_attr = val & 3,
            0x5113..=0x5117 => self.prg_regs[(addr - 0x5113) as usize] = val,
            0x5120..=0x512B => {
                let i = (addr - 0x5120) as usize;
                self.chr_regs[i] = val as u16 | ((self.chr_hi as u16 & 3) << 8);
                self.last_chr_b = i >= 8;
            }
            0x5130 => self.chr_hi = val & 3,
            0x5203 => self.irq_compare = val,
            0x5204 => self.irq_enabled = val & 0x80 != 0,
            0x5205 => self.multiplicand = val,
            0x5206 => self.multiplier = val,
            0x5C00..=0x5FFF => {
                if self.exram_mode != 3 {
                    self.exram[(addr & 0x3FF) as usize] = val;
                }
            }
            0x6000..=0xFFFF => {
                let (rom, bank) = self.prg_bank(addr);
                if !rom && self.ram_writable() {
                    data.prg_ram_set(bank * 0x2000 + (addr & 0x1FFF) as usize, val);
                }
            }
            _ => return false,
        }
        true
    }

    fn on_cpu_read(&mut self, addr: u16) {
        if addr == 0x5204 {
            self.irq_pending = false;
        }
    }

    fn chr_dynamic(&self) -> bool {
        true
    }

    fn nt_dynamic(&self) -> bool {
        true
    }

    fn manages_prg_ram(&self) -> bool {
        true
    }

    #[inline]
    fn chr_offset(&self, addr: u16) -> usize {
        // Sem contexto de sprite/fundo: modo 8 KB com o último conjunto (só para debug)
        let reg = self.chr_regs[if self.last_chr_b { 11 } else { 7 }] as usize;
        reg * 0x2000 + (addr & 0x1FFF) as usize
    }

    #[inline]
    fn ppu_read(&mut self, addr: u16, data: &CartData) -> u8 {
        data.chr_at(self.chr_bank_offset(addr, data))
    }

    #[inline]
    fn ppu_write(&mut self, addr: u16, val: u8, data: &mut CartData) {
        let off = self.chr_bank_offset(addr, data);
        data.chr_set(off, val);
    }

    #[inline]
    fn nt_source(&mut self, addr: u16, data: &CartData) -> Option<NtSource> {
        let addr = addr & 0x0FFF;
        // Detecção de scanline: 3 leituras seguidas do mesmo endereço
        if addr == self.last_nt_addr {
            self.same_reads += 1;
            if self.same_reads == 2 {
                self.detect_scanline();
            }
        } else {
            self.same_reads = 0;
            self.last_nt_addr = addr;
        }
        self.idle_cycles = 0;
        let quadrant = (addr >> 10) as usize;
        let off = (addr & 0x03FF) as usize;
        let is_attr = off >= 0x3C0;
        // Atributos estendidos (modo 1): tile guarda o byte da ExRAM, atributo vem dele
        if self.exram_mode == 1 && !data.ppu_sprite_fetch {
            if !is_attr {
                self.ext_tile = self.exram[off];
            } else {
                let pal = self.ext_tile >> 6;
                return Some(NtSource::Value(pal * 0x55));
            }
        }
        Some(match (self.nt_map >> (quadrant * 2)) & 3 {
            0 => NtSource::Ciram(0),
            1 => NtSource::Ciram(1),
            2 => NtSource::Value(if self.exram_mode < 2 { self.exram[off] } else { 0 }),
            _ => NtSource::Value(if is_attr { self.fill_attr * 0x55 } else { self.fill_tile }),
        })
    }

    fn nt_write(&mut self, addr: u16, val: u8, _data: &mut CartData) -> bool {
        let addr = addr & 0x0FFF;
        let quadrant = (addr >> 10) as usize;
        if (self.nt_map >> (quadrant * 2)) & 3 == 2 {
            if self.exram_mode < 2 {
                self.exram[(addr & 0x03FF) as usize] = val;
            }
            return true;
        }
        false
    }

    fn wants_cpu_clock(&self) -> bool {
        true
    }

    #[inline]
    fn cpu_clock(&mut self) {
        // Sem leituras de nametable por ~3 scanlines: acabou o frame (vblank / render off)
        if self.in_frame {
            self.idle_cycles += 1;
            if self.idle_cycles > 3 * 114 {
                self.end_frame();
            }
        }
        // Áudio: timers a cada 2 ciclos, frame counter ~240 Hz
        self.cycle += 1;
        if self.cycle & 1 == 0 {
            self.pulse1.clock_timer();
            self.pulse2.clock_timer();
        }
        self.frame_cycle += 1;
        if self.frame_cycle >= 7457 {
            self.frame_cycle = 0;
            self.pulse1.clock_quarter();
            self.pulse2.clock_quarter();
            self.frame_step ^= 1;
            if self.frame_step == 0 {
                self.pulse1.clock_half();
                self.pulse2.clock_half();
            }
        }
    }

    #[inline]
    fn irq_pending(&self) -> bool {
        self.irq_pending && self.irq_enabled
    }

    #[inline]
    fn audio_output(&self) -> f32 {
        let p = (self.pulse1.output() + self.pulse2.output()) as f32;
        let pulse = if p > 0.0 { 95.88 / (8128.0 / p + 100.0) } else { 0.0 };
        pulse + self.pcm as f32 / 255.0 * 0.4
    }

    fn reset(&mut self, data: &mut CartData) {
        *self = Mmc5::new(data);
    }

    fn state_string(&self) -> String {
        format!(
            "  MMC5 PRG mode {} regs {:02X?}  CHR mode {} regs {:?} hi {}  NT ${:02X} ExRAM mode {}\n  MMC5 IRQ: compare={} scanline={} in_frame={} en={} pending={}\n",
            self.prg_mode,
            self.prg_regs,
            self.chr_mode,
            self.chr_regs,
            self.chr_hi,
            self.nt_map,
            self.exram_mode,
            self.irq_compare,
            self.scanline,
            self.in_frame,
            self.irq_enabled,
            self.irq_pending
        )
    }
}
