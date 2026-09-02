// NES APU - Audio Processing Unit
// Canais: 2 Pulse, 1 Triangle, 1 Noise, 1 DMC

const LENGTH_TABLE: [u8; 32] = [
    10, 254, 20, 2, 40, 4, 80, 6, 160, 8, 60, 10, 14, 12, 26, 14,
    12, 16, 24, 18, 48, 20, 96, 22, 192, 24, 72, 26, 16, 28, 32, 30,
];

const DUTY_TABLE: [[u8; 8]; 4] = [
    [0, 0, 0, 0, 0, 0, 0, 1],
    [0, 0, 0, 0, 0, 0, 1, 1],
    [0, 0, 0, 0, 1, 1, 1, 1],
    [1, 1, 1, 1, 1, 1, 0, 0],
];

const TRIANGLE_TABLE: [u8; 32] = [
    15, 14, 13, 12, 11, 10, 9, 8, 7, 6, 5, 4, 3, 2, 1, 0,
    0, 1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12, 13, 14, 15,
];

const NOISE_PERIOD_TABLE: [u16; 16] = [
    4, 8, 16, 32, 64, 96, 128, 160, 202, 254, 380, 508, 762, 1016, 2034, 4068,
];

struct Pulse {
    enabled: bool,
    duty: u8,
    duty_pos: u8,
    length_halt: bool,
    length_counter: u8,
    constant_volume: bool,
    volume: u8,
    envelope_start: bool,
    envelope_divider: u8,
    envelope_decay: u8,
    sweep_enabled: bool,
    sweep_period: u8,
    sweep_negate: bool,
    sweep_shift: u8,
    sweep_reload: bool,
    sweep_divider: u8,
    timer: u16,
    timer_period: u16,
    channel: u8, // 0 ou 1
}

impl Pulse {
    fn new(channel: u8) -> Self {
        Pulse {
            enabled: false, duty: 0, duty_pos: 0,
            length_halt: false, length_counter: 0,
            constant_volume: false, volume: 0,
            envelope_start: false, envelope_divider: 0, envelope_decay: 0,
            sweep_enabled: false, sweep_period: 0, sweep_negate: false,
            sweep_shift: 0, sweep_reload: false, sweep_divider: 0,
            timer: 0, timer_period: 0, channel,
        }
    }

    fn clock_timer(&mut self) {
        if self.timer == 0 {
            self.timer = self.timer_period;
            self.duty_pos = (self.duty_pos + 1) % 8;
        } else {
            self.timer -= 1;
        }
    }

    fn clock_envelope(&mut self) {
        if self.envelope_start {
            self.envelope_start = false;
            self.envelope_decay = 15;
            self.envelope_divider = self.volume;
        } else {
            if self.envelope_divider == 0 {
                self.envelope_divider = self.volume;
                if self.envelope_decay > 0 {
                    self.envelope_decay -= 1;
                } else if self.length_halt {
                    self.envelope_decay = 15;
                }
            } else {
                self.envelope_divider -= 1;
            }
        }
    }

    fn clock_length(&mut self) {
        if !self.length_halt && self.length_counter > 0 {
            self.length_counter -= 1;
        }
    }

    fn clock_sweep(&mut self) {
        let change = self.timer_period >> self.sweep_shift;
        let target = if self.sweep_negate {
            self.timer_period.wrapping_sub(change).wrapping_sub(if self.channel == 0 { 1 } else { 0 })
        } else {
            self.timer_period.wrapping_add(change)
        };

        if self.sweep_divider == 0 && self.sweep_enabled && self.sweep_shift > 0 && self.timer_period >= 8 && target <= 0x7FF {
            self.timer_period = target;
        }

        if self.sweep_divider == 0 || self.sweep_reload {
            self.sweep_divider = self.sweep_period;
            self.sweep_reload = false;
        } else {
            self.sweep_divider -= 1;
        }
    }

    fn output(&self) -> u8 {
        if !self.enabled || self.length_counter == 0 || self.timer_period < 8 || self.timer_period > 0x7FF {
            return 0;
        }
        if DUTY_TABLE[self.duty as usize][self.duty_pos as usize] == 0 {
            return 0;
        }
        if self.constant_volume { self.volume } else { self.envelope_decay }
    }
}

struct Triangle {
    enabled: bool,
    length_halt: bool,
    length_counter: u8,
    linear_counter: u8,
    linear_reload_value: u8,
    linear_reload: bool,
    timer: u16,
    timer_period: u16,
    sequence_pos: u8,
}

impl Triangle {
    fn new() -> Self {
        Triangle {
            enabled: false, length_halt: false, length_counter: 0,
            linear_counter: 0, linear_reload_value: 0, linear_reload: false,
            timer: 0, timer_period: 0, sequence_pos: 0,
        }
    }

    fn clock_timer(&mut self) {
        if self.timer == 0 {
            self.timer = self.timer_period;
            if self.length_counter > 0 && self.linear_counter > 0 {
                self.sequence_pos = (self.sequence_pos + 1) % 32;
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
        if !self.length_halt {
            self.linear_reload = false;
        }
    }

    fn clock_length(&mut self) {
        if !self.length_halt && self.length_counter > 0 {
            self.length_counter -= 1;
        }
    }

    fn output(&self) -> u8 {
        if !self.enabled || self.length_counter == 0 || self.linear_counter == 0 || self.timer_period < 2 {
            return 0;
        }
        TRIANGLE_TABLE[self.sequence_pos as usize]
    }
}

struct Noise {
    enabled: bool,
    length_halt: bool,
    length_counter: u8,
    constant_volume: bool,
    volume: u8,
    envelope_start: bool,
    envelope_divider: u8,
    envelope_decay: u8,
    mode: bool,
    timer: u16,
    timer_period: u16,
    shift: u16,
}

impl Noise {
    fn new() -> Self {
        Noise {
            enabled: false, length_halt: false, length_counter: 0,
            constant_volume: false, volume: 0,
            envelope_start: false, envelope_divider: 0, envelope_decay: 0,
            mode: false, timer: 0, timer_period: 0, shift: 1,
        }
    }

    fn clock_timer(&mut self) {
        if self.timer == 0 {
            self.timer = self.timer_period;
            let bit = if self.mode { 6 } else { 1 };
            let feedback = (self.shift & 1) ^ ((self.shift >> bit) & 1);
            self.shift >>= 1;
            self.shift |= feedback << 14;
        } else {
            self.timer -= 1;
        }
    }

    fn clock_envelope(&mut self) {
        if self.envelope_start {
            self.envelope_start = false;
            self.envelope_decay = 15;
            self.envelope_divider = self.volume;
        } else {
            if self.envelope_divider == 0 {
                self.envelope_divider = self.volume;
                if self.envelope_decay > 0 {
                    self.envelope_decay -= 1;
                } else if self.length_halt {
                    self.envelope_decay = 15;
                }
            } else {
                self.envelope_divider -= 1;
            }
        }
    }

    fn clock_length(&mut self) {
        if !self.length_halt && self.length_counter > 0 {
            self.length_counter -= 1;
        }
    }

    fn output(&self) -> u8 {
        if !self.enabled || self.length_counter == 0 || (self.shift & 1) != 0 {
            return 0;
        }
        if self.constant_volume { self.volume } else { self.envelope_decay }
    }
}

const DMC_RATE_TABLE: [u16; 16] = [
    428, 380, 340, 320, 286, 254, 226, 214, 190, 160, 142, 128, 106, 84, 72, 54,
];

struct Dmc {
    enabled: bool,
    irq_enabled: bool,
    loop_flag: bool,
    timer: u16,
    timer_period: u16,
    output_level: u8,
    sample_addr: u16,
    sample_length: u16,
    current_addr: u16,
    bytes_remaining: u16,
    sample_buffer: u8,
    sample_buffer_empty: bool,
    shift_register: u8,
    bits_remaining: u8,
    silence: bool,
}

impl Dmc {
    fn new() -> Self {
        Dmc {
            enabled: false, irq_enabled: false, loop_flag: false,
            timer: 0, timer_period: 0, output_level: 0,
            sample_addr: 0xC000, sample_length: 0, current_addr: 0xC000,
            bytes_remaining: 0, sample_buffer: 0, sample_buffer_empty: true,
            shift_register: 0, bits_remaining: 8, silence: true,
        }
    }

    fn clock_timer(&mut self) {
        if self.timer == 0 {
            self.timer = self.timer_period;

            if !self.silence {
                if self.shift_register & 1 != 0 {
                    if self.output_level <= 125 { self.output_level += 2; }
                } else {
                    if self.output_level >= 2 { self.output_level -= 2; }
                }
                self.shift_register >>= 1;
            }

            self.bits_remaining = self.bits_remaining.wrapping_sub(1);
            if self.bits_remaining == 0 {
                self.bits_remaining = 8;
                if self.sample_buffer_empty {
                    self.silence = true;
                } else {
                    self.silence = false;
                    self.shift_register = self.sample_buffer;
                    self.sample_buffer_empty = true;
                }
            }
        } else {
            self.timer -= 1;
        }
    }

    fn output(&self) -> u8 {
        self.output_level
    }
}

pub struct Apu {
    pulse1: Pulse,
    pulse2: Pulse,
    triangle: Triangle,
    noise: Noise,
    dmc: Dmc,
    frame_counter_mode: u8,
    frame_clock: u32,
    irq_inhibit: bool,
    cpu_clock: u64,

    // Buffer de audio
    pub sample_buffer: Vec<f32>,
    pub sample_rate: f32,
    sample_clock: f64,

    // Filtros high-pass (NES tem dois: 90Hz e 440Hz)
    hp1_prev_in: f32,
    hp1_prev_out: f32,
    hp2_prev_in: f32,
    hp2_prev_out: f32,

    // DMC precisa ler da memória da CPU
    pub dmc_read_addr: Option<u16>,
}

impl Apu {
    pub fn new() -> Self {
        Apu {
            pulse1: Pulse::new(0),
            pulse2: Pulse::new(1),
            triangle: Triangle::new(),
            noise: Noise::new(),
            dmc: Dmc::new(),
            frame_counter_mode: 0,
            frame_clock: 0,
            irq_inhibit: false,
            cpu_clock: 0,
            sample_buffer: Vec::with_capacity(1024),
            sample_rate: 44100.0,
            sample_clock: 0.0,
            hp1_prev_in: 0.0,
            hp1_prev_out: 0.0,
            hp2_prev_in: 0.0,
            hp2_prev_out: 0.0,
            dmc_read_addr: None,
        }
    }

    pub fn set_sample_rate(&mut self, rate: f32) {
        self.sample_rate = rate;
    }

    pub fn cpu_write(&mut self, addr: u16, data: u8) {
        match addr {
            // Pulse 1
            0x4000 => {
                self.pulse1.duty = (data >> 6) & 0x03;
                self.pulse1.length_halt = (data & 0x20) != 0;
                self.pulse1.constant_volume = (data & 0x10) != 0;
                self.pulse1.volume = data & 0x0F;
            },
            0x4001 => {
                self.pulse1.sweep_enabled = (data & 0x80) != 0;
                self.pulse1.sweep_period = (data >> 4) & 0x07;
                self.pulse1.sweep_negate = (data & 0x08) != 0;
                self.pulse1.sweep_shift = data & 0x07;
                self.pulse1.sweep_reload = true;
            },
            0x4002 => {
                self.pulse1.timer_period = (self.pulse1.timer_period & 0x0700) | data as u16;
            },
            0x4003 => {
                self.pulse1.timer_period = (self.pulse1.timer_period & 0x00FF) | ((data as u16 & 0x07) << 8);
                if self.pulse1.enabled {
                    self.pulse1.length_counter = LENGTH_TABLE[(data >> 3) as usize];
                }
                self.pulse1.duty_pos = 0;
                self.pulse1.envelope_start = true;
            },

            // Pulse 2
            0x4004 => {
                self.pulse2.duty = (data >> 6) & 0x03;
                self.pulse2.length_halt = (data & 0x20) != 0;
                self.pulse2.constant_volume = (data & 0x10) != 0;
                self.pulse2.volume = data & 0x0F;
            },
            0x4005 => {
                self.pulse2.sweep_enabled = (data & 0x80) != 0;
                self.pulse2.sweep_period = (data >> 4) & 0x07;
                self.pulse2.sweep_negate = (data & 0x08) != 0;
                self.pulse2.sweep_shift = data & 0x07;
                self.pulse2.sweep_reload = true;
            },
            0x4006 => {
                self.pulse2.timer_period = (self.pulse2.timer_period & 0x0700) | data as u16;
            },
            0x4007 => {
                self.pulse2.timer_period = (self.pulse2.timer_period & 0x00FF) | ((data as u16 & 0x07) << 8);
                if self.pulse2.enabled {
                    self.pulse2.length_counter = LENGTH_TABLE[(data >> 3) as usize];
                }
                self.pulse2.duty_pos = 0;
                self.pulse2.envelope_start = true;
            },

            // Triangle
            0x4008 => {
                self.triangle.length_halt = (data & 0x80) != 0;
                self.triangle.linear_reload_value = data & 0x7F;
            },
            0x400A => {
                self.triangle.timer_period = (self.triangle.timer_period & 0x0700) | data as u16;
            },
            0x400B => {
                self.triangle.timer_period = (self.triangle.timer_period & 0x00FF) | ((data as u16 & 0x07) << 8);
                if self.triangle.enabled {
                    self.triangle.length_counter = LENGTH_TABLE[(data >> 3) as usize];
                }
                self.triangle.linear_reload = true;
            },

            // Noise
            0x400C => {
                self.noise.length_halt = (data & 0x20) != 0;
                self.noise.constant_volume = (data & 0x10) != 0;
                self.noise.volume = data & 0x0F;
            },
            0x400E => {
                self.noise.mode = (data & 0x80) != 0;
                self.noise.timer_period = NOISE_PERIOD_TABLE[(data & 0x0F) as usize];
            },
            0x400F => {
                if self.noise.enabled {
                    self.noise.length_counter = LENGTH_TABLE[(data >> 3) as usize];
                }
                self.noise.envelope_start = true;
            },

            // DMC
            0x4010 => {
                self.dmc.irq_enabled = (data & 0x80) != 0;
                self.dmc.loop_flag = (data & 0x40) != 0;
                self.dmc.timer_period = DMC_RATE_TABLE[(data & 0x0F) as usize];
            },
            0x4011 => {
                self.dmc.output_level = data & 0x7F;
            },
            0x4012 => {
                self.dmc.sample_addr = 0xC000 | ((data as u16) << 6);
            },
            0x4013 => {
                self.dmc.sample_length = ((data as u16) << 4) | 1;
            },

            // Status
            0x4015 => {
                self.pulse1.enabled = (data & 0x01) != 0;
                self.pulse2.enabled = (data & 0x02) != 0;
                self.triangle.enabled = (data & 0x04) != 0;
                self.noise.enabled = (data & 0x08) != 0;
                self.dmc.enabled = (data & 0x10) != 0;
                if !self.pulse1.enabled { self.pulse1.length_counter = 0; }
                if !self.pulse2.enabled { self.pulse2.length_counter = 0; }
                if !self.triangle.enabled { self.triangle.length_counter = 0; }
                if !self.noise.enabled { self.noise.length_counter = 0; }
                if !self.dmc.enabled {
                    self.dmc.bytes_remaining = 0;
                } else if self.dmc.bytes_remaining == 0 {
                    self.dmc.current_addr = self.dmc.sample_addr;
                    self.dmc.bytes_remaining = self.dmc.sample_length;
                }
            },

            // Frame counter
            0x4017 => {
                self.frame_counter_mode = (data >> 7) & 1;
                self.irq_inhibit = (data & 0x40) != 0;
                if self.frame_counter_mode == 1 {
                    self.clock_quarter_frame();
                    self.clock_half_frame();
                }
            },

            _ => {}
        }
    }

    pub fn cpu_read(&self, addr: u16) -> u8 {
        if addr == 0x4015 {
            let mut status = 0u8;
            if self.pulse1.length_counter > 0 { status |= 0x01; }
            if self.pulse2.length_counter > 0 { status |= 0x02; }
            if self.triangle.length_counter > 0 { status |= 0x04; }
            if self.noise.length_counter > 0 { status |= 0x08; }
            if self.dmc.bytes_remaining > 0 { status |= 0x10; }
            status
        } else {
            0
        }
    }

    fn clock_quarter_frame(&mut self) {
        self.pulse1.clock_envelope();
        self.pulse2.clock_envelope();
        self.triangle.clock_linear();
        self.noise.clock_envelope();
    }

    fn clock_half_frame(&mut self) {
        self.pulse1.clock_length();
        self.pulse1.clock_sweep();
        self.pulse2.clock_length();
        self.pulse2.clock_sweep();
        self.triangle.clock_length();
        self.noise.clock_length();
    }

    // Chamado a cada CPU clock (~1.789MHz)
    pub fn clock(&mut self) {
        // Triangle cloca a cada CPU cycle
        self.triangle.clock_timer();

        // Pulse e Noise clockam a cada 2 CPU cycles
        if self.cpu_clock % 2 == 0 {
            self.pulse1.clock_timer();
            self.pulse2.clock_timer();
            self.noise.clock_timer();
            self.dmc.clock_timer();

            // Frame counter (~240Hz, a cada 3728.5 APU cycles)
            self.frame_clock += 1;
            match self.frame_counter_mode {
                0 => {
                    // 4-step
                    match self.frame_clock {
                        3729 => self.clock_quarter_frame(),
                        7457 => { self.clock_quarter_frame(); self.clock_half_frame(); },
                        11186 => self.clock_quarter_frame(),
                        14915 => {
                            self.clock_quarter_frame();
                            self.clock_half_frame();
                            self.frame_clock = 0;
                        },
                        _ => {}
                    }
                },
                1 => {
                    // 5-step
                    match self.frame_clock {
                        3729 => self.clock_quarter_frame(),
                        7457 => { self.clock_quarter_frame(); self.clock_half_frame(); },
                        11186 => self.clock_quarter_frame(),
                        14915 => { self.clock_quarter_frame(); self.clock_half_frame(); },
                        18641 => {
                            self.frame_clock = 0;
                        },
                        _ => {}
                    }
                },
                _ => {}
            }
        }

        // DMC precisa ler sample da CPU
        if self.dmc.sample_buffer_empty && self.dmc.bytes_remaining > 0 {
            self.dmc_read_addr = Some(self.dmc.current_addr);
            // O bus vai chamar dmc_feed_sample() com o byte lido
        }

        // Gerar sample na taxa certa
        self.sample_clock += self.sample_rate as f64 / 1789773.0;
        if self.sample_clock >= 1.0 {
            self.sample_clock -= 1.0;
            let raw = self.mix();
            // High-pass filter 1 (~90Hz, alpha ~0.999835)
            let alpha1: f32 = 0.999835;
            let hp1 = alpha1 * self.hp1_prev_out + raw - self.hp1_prev_in;
            self.hp1_prev_in = raw;
            self.hp1_prev_out = hp1;
            // High-pass filter 2 (~440Hz, alpha ~0.996)
            let alpha2: f32 = 0.996;
            let hp2 = alpha2 * self.hp2_prev_out + hp1 - self.hp2_prev_in;
            self.hp2_prev_in = hp1;
            self.hp2_prev_out = hp2;
            self.sample_buffer.push(hp2 * 0.8); // volume
        }

        self.cpu_clock += 1;
    }

    fn mix(&self) -> f32 {
        let p1 = self.pulse1.output() as f32;
        let p2 = self.pulse2.output() as f32;
        let tri = self.triangle.output() as f32;
        let noise = self.noise.output() as f32;
        let dmc = self.dmc.output() as f32;

        let pulse_out = if p1 + p2 > 0.0 {
            95.88 / (8128.0 / (p1 + p2) + 100.0)
        } else {
            0.0
        };

        let tnd_sum = tri / 8227.0 + noise / 12241.0 + dmc / 22638.0;
        let tnd_out = if tnd_sum > 0.0 {
            159.79 / (1.0 / tnd_sum + 100.0)
        } else {
            0.0
        };

        pulse_out + tnd_out
    }

    pub fn dmc_feed_sample(&mut self, data: u8) {
        self.dmc.sample_buffer = data;
        self.dmc.sample_buffer_empty = false;
        self.dmc.current_addr = self.dmc.current_addr.wrapping_add(1) | 0x8000;
        self.dmc.bytes_remaining -= 1;
        if self.dmc.bytes_remaining == 0 && self.dmc.loop_flag {
            self.dmc.current_addr = self.dmc.sample_addr;
            self.dmc.bytes_remaining = self.dmc.sample_length;
        }
    }

    pub fn reset(&mut self) {
        self.pulse1 = Pulse::new(0);
        self.pulse2 = Pulse::new(1);
        self.triangle = Triangle::new();
        self.noise = Noise::new();
        self.dmc = Dmc::new();
        self.frame_counter_mode = 0;
        self.frame_clock = 0;
        self.cpu_clock = 0;
        self.sample_buffer.clear();
        self.sample_clock = 0.0;
        self.hp1_prev_in = 0.0;
        self.hp1_prev_out = 0.0;
        self.hp2_prev_in = 0.0;
        self.hp2_prev_out = 0.0;
        self.dmc_read_addr = None;
    }
}
