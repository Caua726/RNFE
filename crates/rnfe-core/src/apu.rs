//! APU do 2A03: 2 pulsos, triângulo, ruído, DMC, frame counter e mixer.
//!
//! Tudo aqui é contado em **ciclos de CPU** (`clock()` é chamado uma vez por ciclo pelo bus):
//! pulsos avançam a cada 2 ciclos, os demais timers a cada ciclo, o frame counter nos ciclos
//! 7457/14913/22371/29828-29830 (modo 0) e 37281/37282 (modo 1), como no nesdev.
//!
//! Efeitos com atraso de hardware que os testes do blargg medem: `$4017` só vale 3–4 ciclos
//! depois da escrita; halt e reload dos length counters valem no ciclo seguinte e o reload é
//! ignorado se o contador foi clockado nesse meio tempo; o DMC pede o próximo byte por DMA
//! (`dmc_dma_pending`) e a CPU para 3–4 ciclos para buscá-lo.

pub(crate) const LENGTH_TABLE: [u8; 32] = [
    10, 254, 20, 2, 40, 4, 80, 6, 160, 8, 60, 10, 14, 12, 26, 14, 12, 16, 24, 18, 48, 20, 96, 22, 192, 24,
    72, 26, 16, 28, 32, 30,
];

pub(crate) const DUTY_TABLE: [[u8; 8]; 4] =
    [[0, 1, 0, 0, 0, 0, 0, 0], [0, 1, 1, 0, 0, 0, 0, 0], [0, 1, 1, 1, 1, 0, 0, 0], [1, 0, 0, 1, 1, 1, 1, 1]];

const TRIANGLE_TABLE: [u8; 32] = [
    15, 14, 13, 12, 11, 10, 9, 8, 7, 6, 5, 4, 3, 2, 1, 0, 0, 1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12, 13, 14,
    15,
];

/// Clock da CPU NTSC (Hz).
const CPU_HZ: f64 = 1_789_773.0;

/// Clock da CPU PAL (Hz).
const CPU_HZ_PAL: f64 = 1_662_607.0;

/// Períodos do ruído em ciclos de CPU (NTSC).
const NOISE_PERIOD_TABLE: [u16; 16] =
    [4, 8, 16, 32, 64, 96, 128, 160, 202, 254, 380, 508, 762, 1016, 2034, 4068];

/// Períodos do ruído no PAL.
const NOISE_PERIOD_TABLE_PAL: [u16; 16] =
    [4, 8, 14, 30, 60, 88, 118, 148, 188, 236, 354, 472, 708, 944, 1890, 3778];

/// Períodos do DMC em ciclos de CPU (NTSC).
const DMC_RATE_TABLE: [u16; 16] =
    [428, 380, 340, 320, 286, 254, 226, 214, 190, 160, 142, 128, 106, 84, 72, 54];

/// Períodos do DMC no PAL.
const DMC_RATE_TABLE_PAL: [u16; 16] =
    [398, 354, 316, 298, 276, 236, 210, 198, 176, 148, 132, 118, 98, 78, 66, 50];

/// Ciclos (de CPU) em que cada passo do sequenciador dispara, por modo.
const FRAME_STEPS: [[u32; 6]; 2] =
    [[7457, 14913, 22371, 29828, 29829, 29830], [7457, 14913, 22371, 29829, 37281, 37282]];

/// Mesmos passos no PAL (o divisor do frame counter é outro).
const FRAME_STEPS_PAL: [[u32; 6]; 2] =
    [[8313, 16627, 24939, 33252, 33253, 33254], [8313, 16627, 24939, 33253, 41565, 41566]];

#[derive(Clone, Copy, PartialEq, Eq)]
enum FrameTick {
    None,
    Quarter,
    Half,
}

/// O que cada passo do sequenciador clocka (igual nos dois modos; só os instantes mudam).
const FRAME_TICKS: [FrameTick; 6] = [
    FrameTick::Quarter,
    FrameTick::Half,
    FrameTick::Quarter,
    FrameTick::None,
    FrameTick::Half,
    FrameTick::None,
];

// ------------------------------------------------------------------ peças comuns

/// Length counter com a latência de escrita do hardware (halt/reload valem no ciclo seguinte).
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[derive(Default, Clone)]
struct LengthCounter {
    enabled: bool,
    counter: u8,
    halt: bool,
    new_halt: bool,
    reload_value: u8,
    counter_before_reload: u8,
}

impl LengthCounter {
    fn set_halt(&mut self, halt: bool) {
        self.new_halt = halt;
    }

    fn load(&mut self, index: u8) {
        if self.enabled {
            self.reload_value = LENGTH_TABLE[index as usize];
            self.counter_before_reload = self.counter;
        }
    }

    /// Aplica halt/reload pendentes — depois do clock do frame counter deste ciclo.
    #[inline]
    fn apply_pending(&mut self) {
        if self.reload_value != 0 {
            if self.counter == self.counter_before_reload {
                self.counter = self.reload_value;
            }
            self.reload_value = 0;
        }
        self.halt = self.new_halt;
    }

    fn set_enabled(&mut self, on: bool) {
        self.enabled = on;
        if !on {
            self.counter = 0;
        }
    }

    fn clock(&mut self) {
        if self.counter > 0 && !self.halt {
            self.counter -= 1;
        }
    }

    #[inline]
    fn active(&self) -> bool {
        self.counter > 0
    }
}

#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[derive(Default, Clone)]
struct Envelope {
    start: bool,
    loop_flag: bool,
    constant: bool,
    volume: u8,
    divider: u8,
    decay: u8,
}

impl Envelope {
    fn write(&mut self, data: u8) {
        self.loop_flag = data & 0x20 != 0;
        self.constant = data & 0x10 != 0;
        self.volume = data & 0x0F;
    }

    fn clock(&mut self) {
        if self.start {
            self.start = false;
            self.decay = 15;
            self.divider = self.volume;
        } else if self.divider == 0 {
            self.divider = self.volume;
            if self.decay > 0 {
                self.decay -= 1;
            } else if self.loop_flag {
                self.decay = 15;
            }
        } else {
            self.divider -= 1;
        }
    }

    #[inline]
    fn output(&self) -> u8 {
        if self.constant { self.volume } else { self.decay }
    }
}

// ------------------------------------------------------------------ canais

#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[derive(Clone)]
struct Pulse {
    length: LengthCounter,
    envelope: Envelope,
    duty: u8,
    duty_pos: u8,
    sweep_enabled: bool,
    sweep_period: u8,
    sweep_negate: bool,
    sweep_shift: u8,
    sweep_reload: bool,
    sweep_divider: u8,
    sweep_target: u16,
    timer: u16,
    period: u16,
    /// Canal 1 subtrai 1 a mais no sweep negativo.
    is_first: bool,
}

impl Pulse {
    fn new(is_first: bool) -> Self {
        Pulse {
            length: LengthCounter::default(),
            envelope: Envelope::default(),
            duty: 0,
            duty_pos: 0,
            sweep_enabled: false,
            sweep_period: 0,
            sweep_negate: false,
            sweep_shift: 0,
            sweep_reload: false,
            sweep_divider: 0,
            sweep_target: 0,
            timer: 0,
            period: 0,
            is_first,
        }
    }

    fn update_target(&mut self) {
        let change = self.period >> self.sweep_shift;
        self.sweep_target = if self.sweep_negate {
            self.period.wrapping_sub(change).wrapping_sub(self.is_first as u16)
        } else {
            self.period + change
        };
    }

    fn set_period(&mut self, p: u16) {
        self.period = p;
        self.update_target();
    }

    /// A cada 2 ciclos de CPU.
    #[inline]
    fn clock_timer(&mut self) {
        if self.timer == 0 {
            self.timer = self.period;
            self.duty_pos = (self.duty_pos + 1) & 7;
        } else {
            self.timer -= 1;
        }
    }

    fn clock_sweep(&mut self) {
        if self.sweep_divider == 0 && self.sweep_enabled && self.sweep_shift > 0 && !self.muted() {
            self.set_period(self.sweep_target);
        }
        if self.sweep_divider == 0 || self.sweep_reload {
            self.sweep_divider = self.sweep_period;
            self.sweep_reload = false;
        } else {
            self.sweep_divider -= 1;
        }
    }

    #[inline]
    fn muted(&self) -> bool {
        self.period < 8 || (!self.sweep_negate && self.sweep_target > 0x7FF)
    }

    #[inline]
    fn output(&self) -> u8 {
        if !self.length.active()
            || self.muted()
            || DUTY_TABLE[self.duty as usize][self.duty_pos as usize] == 0
        {
            0
        } else {
            self.envelope.output()
        }
    }
}

#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[derive(Clone)]
struct Triangle {
    length: LengthCounter,
    linear_counter: u8,
    linear_reload_value: u8,
    linear_reload: bool,
    control: bool,
    timer: u16,
    period: u16,
    sequence_pos: u8,
}

impl Triangle {
    fn new() -> Self {
        Triangle {
            length: LengthCounter::default(),
            linear_counter: 0,
            linear_reload_value: 0,
            linear_reload: false,
            control: false,
            timer: 0,
            period: 0,
            sequence_pos: 0,
        }
    }

    /// A cada ciclo de CPU.
    #[inline]
    fn clock_timer(&mut self) {
        if self.timer == 0 {
            self.timer = self.period;
            // period < 2 daria uma onda ultrassônica: congela o sequenciador (a saída fica no
            // valor atual em vez de saltar para zero, que virava "pop")
            if self.length.active() && self.linear_counter > 0 && self.period >= 2 {
                self.sequence_pos = (self.sequence_pos + 1) & 31;
            }
        } else {
            self.timer -= 1;
        }
    }

    fn clock_linear(&mut self) {
        if self.linear_reload {
            self.linear_counter = self.linear_reload_value;
        } else if self.linear_counter > 0 {
            self.linear_counter -= 1;
        }
        if !self.control {
            self.linear_reload = false;
        }
    }

    #[inline]
    fn output(&self) -> u8 {
        // Período ultrassônico (< 2): o hardware congela o sequenciador. Zerar a saída daria um
        // degrau audível toda vez que um driver de música "cala" o triângulo assim.
        TRIANGLE_TABLE[self.sequence_pos as usize]
    }
}

#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[derive(Clone)]
struct Noise {
    length: LengthCounter,
    envelope: Envelope,
    mode: bool,
    timer: u16,
    period: u16,
    shift: u16,
}

impl Noise {
    fn new() -> Self {
        Noise {
            length: LengthCounter::default(),
            envelope: Envelope::default(),
            mode: false,
            timer: 0,
            period: NOISE_PERIOD_TABLE[0] - 1,
            shift: 1,
        }
    }

    /// A cada ciclo de CPU.
    #[inline]
    fn clock_timer(&mut self) {
        if self.timer == 0 {
            self.timer = self.period;
            let bit = if self.mode { 6 } else { 1 };
            let feedback = (self.shift & 1) ^ ((self.shift >> bit) & 1);
            self.shift = (self.shift >> 1) | (feedback << 14);
        } else {
            self.timer -= 1;
        }
    }

    #[inline]
    fn output(&self) -> u8 {
        if !self.length.active() || (self.shift & 1) != 0 { 0 } else { self.envelope.output() }
    }
}

#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[derive(Clone)]
struct Dmc {
    irq_enabled: bool,
    irq_flag: bool,
    loop_flag: bool,
    timer: u16,
    period: u16,
    output_level: u8,
    sample_addr: u16,
    sample_length: u16,
    current_addr: u16,
    bytes_remaining: u16,
    sample_buffer: Option<u8>,
    shift_register: u8,
    bits_remaining: u8,
    silence: bool,
    /// Pedido de DMA em aberto (a CPU o consome no próximo ciclo de leitura).
    dma_pending: bool,
}

impl Dmc {
    fn new() -> Self {
        Dmc {
            irq_enabled: false,
            irq_flag: false,
            loop_flag: false,
            timer: 0,
            period: DMC_RATE_TABLE[0] - 1,
            output_level: 0,
            sample_addr: 0xC000,
            sample_length: 1,
            current_addr: 0xC000,
            bytes_remaining: 0,
            sample_buffer: None,
            shift_register: 0,
            bits_remaining: 8,
            silence: true,
            dma_pending: false,
        }
    }

    fn restart(&mut self) {
        self.current_addr = self.sample_addr;
        self.bytes_remaining = self.sample_length;
    }

    /// Pede o próximo byte se o buffer está vazio e ainda há amostra.
    #[inline]
    fn request_byte(&mut self) {
        if self.sample_buffer.is_none() && self.bytes_remaining > 0 {
            self.dma_pending = true;
        }
    }

    /// A cada ciclo de CPU.
    #[inline]
    fn clock_timer(&mut self) {
        if self.timer == 0 {
            self.timer = self.period;
            if !self.silence {
                if self.shift_register & 1 != 0 {
                    if self.output_level <= 125 {
                        self.output_level += 2;
                    }
                } else if self.output_level >= 2 {
                    self.output_level -= 2;
                }
                self.shift_register >>= 1;
            }
            self.bits_remaining -= 1;
            if self.bits_remaining == 0 {
                self.bits_remaining = 8;
                match self.sample_buffer.take() {
                    Some(b) => {
                        self.silence = false;
                        self.shift_register = b;
                        self.request_byte();
                    }
                    None => self.silence = true,
                }
            }
        } else {
            self.timer -= 1;
        }
    }

    /// Byte entregue pelo DMA.
    fn feed(&mut self, data: u8) {
        if self.bytes_remaining == 0 {
            return; // $4015 desligou o canal entre o pedido do DMA e a busca: descarta
        }
        self.sample_buffer = Some(data);
        self.current_addr = self.current_addr.wrapping_add(1) | 0x8000;
        self.bytes_remaining -= 1;
        if self.bytes_remaining == 0 {
            if self.loop_flag {
                self.restart();
            } else if self.irq_enabled {
                self.irq_flag = true;
            }
        }
    }
}

/// Coeficiente de um passa-alta RC de 1ª ordem (`y = α·(y₋₁ + x − x₋₁)`) com corte `fc`.
/// Passa-baixa de 1ª ordem: `alpha = dt / (RC + dt)`.
fn lp_alpha(fc: f64, fs: f64) -> f32 {
    let dt = 1.0 / fs.max(1.0);
    let rc = 1.0 / (2.0 * std::f64::consts::PI * fc);
    (dt / (rc + dt)) as f32
}

#[cfg(feature = "serde")]
fn default_lp_alpha() -> f32 {
    lp_alpha(14_000.0, 44100.0)
}

fn hp_alpha(fc: f64, fs: f64) -> f32 {
    let rc = 1.0 / (2.0 * std::f64::consts::PI * fc);
    let dt = 1.0 / fs.max(1.0);
    (rc / (rc + dt)) as f32
}

/// `95.88 / (8128 / n + 100)` para n = 0..=30 (soma dos dois pulsos).
pub(crate) const PULSE_TABLE: [f32; 31] = build_pulse_table();
/// `159.79 / (1 / (n / 22638) + 100)` para n = 3·tri + 2·noise + dmc (0..=202).
pub(crate) const TND_TABLE: [f32; 203] = build_tnd_table();

const fn build_pulse_table() -> [f32; 31] {
    let mut t = [0.0f32; 31];
    let mut n = 1;
    while n < 31 {
        t[n] = 95.88 / (8128.0 / n as f32 + 100.0);
        n += 1;
    }
    t
}

const fn build_tnd_table() -> [f32; 203] {
    let mut t = [0.0f32; 203];
    let mut n = 1;
    while n < 203 {
        t[n] = 159.79 / (1.0 / (n as f32 / 22638.0) + 100.0);
        n += 1;
    }
    t
}

/// Mix não linear dos dois pulsos (usado também pelo MMC5).
#[inline]
pub(crate) fn pulse_mix(sum: u8) -> f32 {
    PULSE_TABLE[(sum as usize).min(30)]
}

// ------------------------------------------------------------------ APU

#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[derive(Clone)]
pub struct Apu {
    pulse1: Pulse,
    pulse2: Pulse,
    triangle: Triangle,
    noise: Noise,
    dmc: Dmc,

    // Frame counter
    frame_mode: u8,
    frame_cycle: u32,
    frame_step: usize,
    frame_irq_inhibit: bool,
    frame_irq: bool,
    /// Escrita em `$4017` esperando os 3–4 ciclos de atraso.
    frame_write: Option<(u8, u8)>,
    /// Evita dois clocks de quarter/half em ciclos consecutivos (reset + passo do sequenciador).
    frame_block: u8,

    /// Ciclos de CPU vistos pela APU.
    cycle: u64,
    /// Ciclos restantes em que `apply_pending` precisa rodar (após escrita em `$4000-$400F`).
    length_pending: u8,

    // Buffer de áudio
    #[cfg_attr(feature = "serde", serde(skip))]
    pub sample_buffer: Vec<f32>,
    pub sample_rate: f32,
    /// Temporização do console: muda o relógio e as tabelas de ruído/DMC/frame counter.
    /// Fora do save state, como na PPU.
    #[cfg_attr(feature = "serde", serde(skip))]
    region: crate::Region,
    /// Amostras por ciclo de CPU (`sample_rate / CPU_HZ`), pré-calculado.
    sample_step: f64,
    sample_clock: f64,
    /// Soma das saídas de cada ciclo desde a última amostra (média = filtro caixa: tira o
    /// aliasing dos pulsos agudos e do ruído).
    acc: f32,
    acc_n: u32,

    // Filtros high-pass (NES tem dois: 90 Hz e 440 Hz); coeficientes dependem da taxa
    hp1_alpha: f32,
    hp2_alpha: f32,
    /// Passa-baixa de 14 kHz da saída do console (estado transitório: fora do save state).
    /// Com `default` explícito: zero silenciaria a saída para sempre se alguém carregasse um
    /// state sem passar pelo `set_sample_rate`.
    #[cfg_attr(feature = "serde", serde(skip, default = "default_lp_alpha"))]
    lp_alpha: f32,
    #[cfg_attr(feature = "serde", serde(skip))]
    lp_prev: f32,
    hp1_prev_in: f32,
    hp1_prev_out: f32,
    hp2_prev_in: f32,
    hp2_prev_out: f32,
}

impl Default for Apu {
    fn default() -> Self {
        Self::new()
    }
}

impl Apu {
    pub fn new() -> Self {
        let mut apu = Apu {
            pulse1: Pulse::new(true),
            pulse2: Pulse::new(false),
            triangle: Triangle::new(),
            noise: Noise::new(),
            dmc: Dmc::new(),
            frame_mode: 0,
            frame_cycle: 0,
            frame_step: 0,
            frame_irq_inhibit: false,
            frame_irq: false,
            frame_write: None,
            frame_block: 0,
            cycle: 0,
            length_pending: 0,
            sample_buffer: Vec::with_capacity(1024),
            sample_rate: 44100.0,
            region: crate::Region::Ntsc,
            sample_step: 44100.0 / CPU_HZ,
            sample_clock: 0.0,
            acc: 0.0,
            acc_n: 0,
            hp1_alpha: hp_alpha(90.0, 44100.0),
            hp2_alpha: hp_alpha(440.0, 44100.0),
            lp_alpha: lp_alpha(14_000.0, 44100.0),
            lp_prev: 0.0,
            hp1_prev_in: 0.0,
            hp1_prev_out: 0.0,
            hp2_prev_in: 0.0,
            hp2_prev_out: 0.0,
        };
        apu.reset(false);
        apu
    }

    /// Reset: canais desligados, frame counter como se `$4017` fosse escrito com `$00`
    /// (ou com o modo anterior, num reset por botão) poucos ciclos antes da primeira instrução.
    pub fn reset(&mut self, soft: bool) {
        let keep_mode = if soft { self.frame_mode } else { 0 };
        self.pulse1 = Pulse::new(true);
        self.pulse2 = Pulse::new(false);
        // No reset por botão o length counter do triângulo não é tocado
        let tri_length = self.triangle.length.clone();
        self.triangle = Triangle::new();
        if soft {
            self.triangle.length = tri_length;
            self.triangle.length.enabled = false;
        }
        self.noise = Noise::new();
        self.dmc = Dmc::new();
        self.frame_mode = 0;
        self.frame_cycle = 0;
        self.frame_step = 0;
        self.frame_irq_inhibit = false;
        self.frame_irq = false;
        // Como se $4017 tivesse sido escrito ~10 ciclos antes da 1ª instrução: o efeito
        // (3 ciclos depois) já vale no começo dos 7 ciclos do reset da CPU.
        self.frame_write = None;
        self.frame_mode = keep_mode;
        self.frame_block = 0;
        self.sample_buffer.clear();
        self.sample_clock = 0.0;
        self.acc = 0.0;
        self.acc_n = 0;
        self.lp_prev = 0.0;
        self.hp1_prev_in = 0.0;
        self.hp1_prev_out = 0.0;
        self.hp2_prev_in = 0.0;
        self.hp2_prev_out = 0.0;
    }

    /// Troca a temporização; os períodos já carregados são reconvertidos na próxima escrita.
    pub fn set_region(&mut self, region: crate::Region) {
        self.region = region;
        self.set_sample_rate(self.sample_rate);
    }

    fn cpu_hz(&self) -> f64 {
        match self.region {
            crate::Region::Ntsc => CPU_HZ,
            crate::Region::Pal => CPU_HZ_PAL,
        }
    }

    pub fn set_sample_rate(&mut self, rate: f32) {
        self.sample_rate = rate;
        self.sample_step = rate as f64 / self.cpu_hz();
        self.hp1_alpha = hp_alpha(90.0, rate as f64);
        self.hp2_alpha = hp_alpha(440.0, rate as f64);
        self.lp_alpha = lp_alpha(14_000.0, rate as f64);
    }

    /// Nível da linha IRQ da APU (frame counter ou DMC).
    #[inline]
    pub fn irq_line(&self) -> bool {
        self.frame_irq || self.dmc.irq_flag
    }

    /// Há um byte do DMC para buscar? (a CPU consome e chama `dmc_feed_sample`)
    #[inline]
    pub fn take_dmc_dma(&mut self) -> bool {
        std::mem::take(&mut self.dmc.dma_pending)
    }

    #[inline]
    pub fn dmc_address(&self) -> u16 {
        self.dmc.current_addr
    }

    pub fn dmc_feed_sample(&mut self, data: u8) {
        self.dmc.feed(data);
    }

    // ------------------------------------------------------------ registradores

    pub fn cpu_write(&mut self, addr: u16, data: u8) {
        if (0x4000..=0x400F).contains(&addr) {
            self.length_pending = 2;
        }
        match addr {
            0x4000 | 0x4004 => {
                let p = if addr == 0x4000 { &mut self.pulse1 } else { &mut self.pulse2 };
                p.duty = data >> 6;
                p.length.set_halt(data & 0x20 != 0);
                p.envelope.write(data);
            }
            0x4001 | 0x4005 => {
                let p = if addr == 0x4001 { &mut self.pulse1 } else { &mut self.pulse2 };
                p.sweep_enabled = data & 0x80 != 0;
                p.sweep_period = (data >> 4) & 0x07;
                p.sweep_negate = data & 0x08 != 0;
                p.sweep_shift = data & 0x07;
                p.sweep_reload = true;
                p.update_target();
            }
            0x4002 | 0x4006 => {
                let p = if addr == 0x4002 { &mut self.pulse1 } else { &mut self.pulse2 };
                p.set_period((p.period & 0x0700) | data as u16);
            }
            0x4003 | 0x4007 => {
                let p = if addr == 0x4003 { &mut self.pulse1 } else { &mut self.pulse2 };
                p.length.load(data >> 3);
                p.set_period((p.period & 0x00FF) | ((data as u16 & 0x07) << 8));
                p.duty_pos = 0;
                p.envelope.start = true;
            }

            0x4008 => {
                self.triangle.control = data & 0x80 != 0;
                self.triangle.length.set_halt(data & 0x80 != 0);
                self.triangle.linear_reload_value = data & 0x7F;
            }
            0x400A => self.triangle.period = (self.triangle.period & 0x0700) | data as u16,
            0x400B => {
                self.triangle.length.load(data >> 3);
                self.triangle.period = (self.triangle.period & 0x00FF) | ((data as u16 & 0x07) << 8);
                self.triangle.linear_reload = true;
            }

            0x400C => {
                self.noise.length.set_halt(data & 0x20 != 0);
                self.noise.envelope.write(data);
            }
            0x400E => {
                self.noise.mode = data & 0x80 != 0;
                let table = match self.region {
                    crate::Region::Ntsc => &NOISE_PERIOD_TABLE,
                    crate::Region::Pal => &NOISE_PERIOD_TABLE_PAL,
                };
                self.noise.period = table[(data & 0x0F) as usize] - 1;
            }
            0x400F => {
                self.noise.length.load(data >> 3);
                self.noise.envelope.start = true;
            }

            0x4010 => {
                self.dmc.irq_enabled = data & 0x80 != 0;
                if !self.dmc.irq_enabled {
                    self.dmc.irq_flag = false;
                }
                self.dmc.loop_flag = data & 0x40 != 0;
                let table = match self.region {
                    crate::Region::Ntsc => &DMC_RATE_TABLE,
                    crate::Region::Pal => &DMC_RATE_TABLE_PAL,
                };
                self.dmc.period = table[(data & 0x0F) as usize] - 1;
            }
            0x4011 => self.dmc.output_level = data & 0x7F,
            0x4012 => self.dmc.sample_addr = 0xC000 | ((data as u16) << 6),
            0x4013 => self.dmc.sample_length = ((data as u16) << 4) | 1,

            0x4015 => {
                self.pulse1.length.set_enabled(data & 0x01 != 0);
                self.pulse2.length.set_enabled(data & 0x02 != 0);
                self.triangle.length.set_enabled(data & 0x04 != 0);
                self.noise.length.set_enabled(data & 0x08 != 0);
                self.dmc.irq_flag = false;
                if data & 0x10 != 0 {
                    if self.dmc.bytes_remaining == 0 {
                        self.dmc.restart();
                        self.dmc.request_byte();
                    }
                } else {
                    self.dmc.bytes_remaining = 0;
                    self.dmc.dma_pending = false;
                }
            }

            0x4017 => {
                // Vale 3 ciclos depois se a escrita caiu num ciclo par da APU, 4 se ímpar
                let delay = if self.cycle & 1 == 1 { 4 } else { 3 };
                self.frame_write = Some((data, delay));
                self.frame_irq_inhibit = data & 0x40 != 0;
                if self.frame_irq_inhibit {
                    self.frame_irq = false;
                }
            }
            _ => {}
        }
    }

    /// Leitura de `$4015`: status dos canais e flags de IRQ; limpa a flag do frame counter.
    pub fn read_status(&mut self) -> u8 {
        let s = self.peek_status();
        self.frame_irq = false;
        s
    }

    /// `$4015` sem efeito colateral (debug).
    pub fn peek_status(&self) -> u8 {
        let mut s = 0u8;
        s |= self.pulse1.length.active() as u8;
        s |= (self.pulse2.length.active() as u8) << 1;
        s |= (self.triangle.length.active() as u8) << 2;
        s |= (self.noise.length.active() as u8) << 3;
        s |= ((self.dmc.bytes_remaining > 0) as u8) << 4;
        s |= (self.frame_irq as u8) << 6;
        s |= (self.dmc.irq_flag as u8) << 7;
        s
    }

    // ------------------------------------------------------------ relógio

    fn clock_quarter_frame(&mut self) {
        self.pulse1.envelope.clock();
        self.pulse2.envelope.clock();
        self.triangle.clock_linear();
        self.noise.envelope.clock();
    }

    fn clock_half_frame(&mut self) {
        self.pulse1.length.clock();
        self.pulse1.clock_sweep();
        self.pulse2.length.clock();
        self.pulse2.clock_sweep();
        self.triangle.length.clock();
        self.noise.length.clock();
    }

    fn frame_tick(&mut self, tick: FrameTick) {
        if self.frame_block > 0 {
            return;
        }
        match tick {
            FrameTick::None => return,
            FrameTick::Quarter => self.clock_quarter_frame(),
            FrameTick::Half => {
                self.clock_quarter_frame();
                self.clock_half_frame();
            }
        }
        self.frame_block = 2;
    }

    /// Um ciclo de CPU.
    #[inline]
    /// Um ciclo de CPU. `expansion` é chamado só quando a APU gera uma amostra (~44 kHz) e
    /// devolve o áudio do cartucho (VRC6, N163, MMC5, 5B…), já na escala do mix da 2A03.
    pub fn clock(&mut self, expansion: f32) {
        self.cycle += 1;

        // --- frame counter
        self.frame_cycle += 1;
        let mode = self.frame_mode as usize;
        let steps = match self.region {
            crate::Region::Ntsc => &FRAME_STEPS,
            crate::Region::Pal => &FRAME_STEPS_PAL,
        };
        if self.frame_cycle >= steps[mode][self.frame_step] {
            if mode == 0 && self.frame_step >= 3 && !self.frame_irq_inhibit {
                self.frame_irq = true;
            }
            let tick = FRAME_TICKS[self.frame_step];
            self.frame_tick(tick);
            self.frame_step += 1;
            if self.frame_step == 6 {
                self.frame_step = 0;
                self.frame_cycle = 0;
            }
        }
        if let Some((data, delay)) = self.frame_write {
            if delay <= 1 {
                self.frame_write = None;
                self.frame_mode = data >> 7;
                self.frame_step = 0;
                self.frame_cycle = 0;
                if self.frame_mode == 1 {
                    // Modo 5 passos: clock imediato de quarter e half
                    self.frame_tick(FrameTick::Half);
                }
            } else {
                self.frame_write = Some((data, delay - 1));
            }
        }
        if self.frame_block > 0 {
            self.frame_block -= 1;
        }

        // --- halt/reload dos length counters (depois do clock do frame counter); só nos
        // ciclos seguintes a uma escrita em $4000-$400F
        if self.length_pending > 0 {
            self.length_pending -= 1;
            self.pulse1.length.apply_pending();
            self.pulse2.length.apply_pending();
            self.triangle.length.apply_pending();
            self.noise.length.apply_pending();
        }

        // --- timers
        if self.cycle & 1 == 0 {
            self.pulse1.clock_timer();
            self.pulse2.clock_timer();
        }
        self.triangle.clock_timer();
        self.noise.clock_timer();
        self.dmc.clock_timer();

        // --- amostragem: média das saídas de todos os ciclos desde a última amostra
        self.acc += self.mix() + expansion;
        self.acc_n += 1;
        self.sample_clock += self.sample_step;
        if self.sample_clock >= 1.0 {
            self.sample_clock -= 1.0;
            let raw = self.acc / self.acc_n as f32;
            self.acc = 0.0;
            self.acc_n = 0;
            // High-pass 1 (90 Hz)
            let hp1 = self.hp1_alpha * (self.hp1_prev_out + raw - self.hp1_prev_in);
            self.hp1_prev_in = raw;
            self.hp1_prev_out = hp1;
            // High-pass 2 (440 Hz)
            let hp2 = self.hp2_alpha * (self.hp2_prev_out + hp1 - self.hp2_prev_in);
            self.hp2_prev_in = hp1;
            self.hp2_prev_out = hp2;
            // Passa-baixa de 14 kHz: o console tem os três filtros; sem este o som sai mais
            // áspero e o lixo acima de 14 kHz passa inteiro.
            self.lp_prev += self.lp_alpha * (hp2 - self.lp_prev);
            // Ganho: o pico real de um jogo fica em torno de −12 dBFS com 0,8; 2,0 aproxima o
            // nível dos outros emuladores sem estourar (o clamp protege picos raros).
            // Compressão suave no lugar do corte duro: com áudio de expansão o pico passa de
            // 1,0 (MMC5 chega a 1,67) e o `clamp` distorcia justamente esses jogos.
            let v = self.lp_prev * 2.0;
            self.sample_buffer.push(v / (1.0 + 0.35 * v.abs()));
        }
    }

    /// Saída instantânea da 2A03 (mix não linear por tabelas).
    #[inline]
    fn mix(&self) -> f32 {
        let p = (self.pulse1.output() + self.pulse2.output()) as usize;
        let t = self.triangle.output() as usize * 3
            + self.noise.output() as usize * 2
            + self.dmc.output_level as usize;
        PULSE_TABLE[p] + TND_TABLE[t]
    }
}
