//! Barramento da CPU: RAM, registradores da PPU/APU, controles, cartucho — e o relógio.
//!
//! A CPU chama [`Bus::tick_pre`]/[`Bus::tick_post`] em volta de **cada** acesso: um ciclo de CPU
//! são 3 dots de PPU (`DOTS_BEFORE_ACCESS` antes do acesso, o resto depois) e um clock de APU.
//! Assim PPU e APU nunca ficam nem à frente nem atrás da CPU por mais de um ciclo.

use crate::apu::Apu;
use crate::cartridge::Cartridge;
use crate::ppu::Ppu;

/// Dots de PPU executados antes do acesso da CPU dentro do ciclo (o restante, 3 − N, depois).
/// Fixa o alinhamento CPU/PPU visível em leituras de `$2002` perto do VBL.
const DOTS_BEFORE_ACCESS: u32 = 2;

/// Estado do Zapper (pistola de luz): onde o cano aponta e se o gatilho está apertado.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Zapper {
    pub x: u16,
    pub y: u16,
    pub trigger: bool,
}

pub struct Bus {
    pub ppu: Ppu,
    pub apu: Apu,
    pub cartridge: Cartridge,
    pub ram: [u8; 2048],
    /// Ciclos de CPU desde o power-on (o reset conta os seus 7).
    pub cpu_cycles: u64,
    /// O mapper quer `cpu_clock` a cada ciclo (lido uma vez do cartucho).
    mapper_clock: bool,
    /// O cartucho tem áudio próprio (some na média por ciclo em vez de amostrar por ponto).
    cart_audio: bool,
    /// Temporização do console (NTSC = 3 dots por ciclo; PAL = 3,2).
    region: crate::Region,
    /// Numerador acumulado dos dots que faltam dar (fração da região).
    dot_acc: u32,
    /// Dots que faltam dar depois do acesso deste ciclo.
    pending_dots: u32,
    /// Página pedida por `$4014`; a CPU consome no próximo ciclo de leitura.
    oam_dma_page: Option<u8>,
    /// Último valor no barramento de dados (lido em endereços não mapeados).
    open_bus: u8,
    // Controles. Bits: A B Select Start Up Down Left Right
    pub controller: [u8; 2],
    /// Zapper na porta 2 (`$4017`): posição de mira e gatilho.
    pub zapper: Option<Zapper>,
    controller_state: [u8; 2],
    controller_strobe: bool,
}

impl Bus {
    pub fn new(cartridge: Cartridge) -> Bus {
        Bus {
            mapper_clock: cartridge.wants_cpu_clock(),
            cart_audio: cartridge.has_audio(),
            region: crate::Region::Ntsc,
            dot_acc: 0,
            pending_dots: 1,
            ppu: Ppu::new(),
            apu: Apu::new(),
            cartridge,
            ram: [0u8; 2048],
            cpu_cycles: 0,
            oam_dma_page: None,
            open_bus: 0,
            controller: [0; 2],
            zapper: None,
            controller_state: [0; 2],
            controller_strobe: false,
        }
    }

    // ------------------------------------------------------------------ relógio

    /// Troca a temporização (NTSC/PAL). Zera o acumulador de dots.
    pub fn set_region(&mut self, region: crate::Region) {
        self.region = region;
        self.dot_acc = 0;
        self.ppu.set_region(region);
        self.apu.set_region(region);
    }

    pub fn region(&self) -> crate::Region {
        self.region
    }

    /// Quantos dots este ciclo de CPU vale (3 no NTSC; 3 ou 4 alternando no PAL, 16 a cada 5).
    #[inline]
    fn dots_this_cycle(&mut self) -> u32 {
        let (num, den) = self.region.dots_per_cycle();
        if den == 1 {
            return num;
        }
        self.dot_acc += num;
        let dots = self.dot_acc / den;
        self.dot_acc %= den;
        dots
    }

    /// Começo de um ciclo de CPU: os dots de PPU que antecedem o acesso.
    #[inline]
    pub fn tick_pre(&mut self) {
        // A APU avança antes do acesso: uma leitura de $4015 vê o mesmo estado que a linha IRQ
        // amostrada no fim deste ciclo.
        // O áudio do cartucho entra na mesma média por ciclo da 2A03: amostrar só no instante
        // da amostra (1 em ~37 ciclos) enchia de aliasing os canais do VRC6/N163/MMC5/5B.
        let exp = if self.cart_audio { self.cartridge.audio_output() } else { 0.0 };
        self.apu.clock(exp);
        // No PAL o ciclo vale 3 ou 4 dots; os 2 primeiros continuam antes do acesso.
        self.pending_dots = self.dots_this_cycle().saturating_sub(DOTS_BEFORE_ACCESS);
        for _ in 0..DOTS_BEFORE_ACCESS {
            self.ppu.step(&mut self.cartridge);
        }
    }

    /// Fim de um ciclo de CPU: dots restantes e mapper.
    #[inline]
    pub fn tick_post(&mut self) {
        for _ in 0..self.pending_dots {
            self.ppu.step(&mut self.cartridge);
        }
        if self.ppu.a12_rise {
            self.ppu.a12_rise = false;
            self.cartridge.a12_rise();
        }
        if self.mapper_clock {
            self.cartridge.cpu_clock();
        }
        self.cpu_cycles += 1;
    }

    /// Nível da linha /NMI (a CPU detecta a borda).
    #[inline]
    pub fn nmi_line(&self) -> bool {
        self.ppu.nmi_output()
    }

    /// Nível da linha /IRQ: APU (frame counter, DMC) ou mapper.
    #[inline]
    pub fn irq_line(&self) -> bool {
        self.apu.irq_line() || self.cartridge.irq_pending()
    }

    #[inline]
    pub fn take_oam_dma(&mut self) -> Option<u8> {
        self.oam_dma_page.take()
    }

    /// O DMC pediu um byte: a CPU faz o DMA (3–4 ciclos parada) e entrega com `dmc_feed`.
    #[inline]
    pub fn take_dmc_dma(&mut self) -> bool {
        self.apu.take_dmc_dma()
    }

    #[inline]
    pub fn dmc_address(&self) -> u16 {
        self.apu.dmc_address()
    }

    #[inline]
    pub fn dmc_feed(&mut self, data: u8) {
        self.apu.dmc_feed_sample(data);
    }

    // ------------------------------------------------------------------ acessos

    /// Leitura sem avançar o relógio (a CPU chama dentro de um ciclo; o DMC também).
    #[inline]
    pub fn read_raw(&mut self, addr: u16) -> u8 {
        let v = match addr {
            0x0000..=0x1FFF => self.ram[(addr & 0x07FF) as usize],
            0x2000..=0x3FFF => self.ppu.cpu_read(addr & 0x0007, &mut self.cartridge),
            // bit 5 de $4015 e bits 5-7 dos controles vêm do open bus
            0x4015 => self.apu.read_status() | (self.open_bus & 0x20),
            0x4016 => self.read_controller(0) | (self.open_bus & 0xE0),
            0x4017 => self.read_port2() | (self.open_bus & 0xE0),
            0x4000..=0x401F => self.open_bus,
            _ => self.cartridge.cpu_read_mut(addr).unwrap_or(self.open_bus),
        };
        self.open_bus = v;
        v
    }

    #[inline]
    pub fn write_raw(&mut self, addr: u16, data: u8) {
        self.open_bus = data;
        match addr {
            0x0000..=0x1FFF => self.ram[(addr & 0x07FF) as usize] = data,
            0x2000..=0x3FFF => self.ppu.cpu_write(addr & 0x0007, data, &mut self.cartridge),
            0x4014 => self.oam_dma_page = Some(data),
            0x4016 => {
                if data & 0x01 != 0 {
                    self.controller_strobe = true;
                } else {
                    if self.controller_strobe {
                        // Snapshot quando o strobe desliga
                        self.controller_state = self.controller;
                    }
                    self.controller_strobe = false;
                }
            }
            0x4000..=0x4013 | 0x4015 | 0x4017 => self.apu.cpu_write(addr, data),
            0x4018..=0x401F => {}
            _ => {
                self.cartridge.data.cpu_cycle = self.cpu_cycles;
                self.cartridge.cpu_write(addr, data);
            }
        }
    }

    /// Porta 2: controle comum ou Zapper (bit 3 = gatilho, bit 4 = **ausência** de luz).
    #[inline]
    fn read_port2(&mut self) -> u8 {
        let Some(z) = self.zapper else { return self.read_controller(1) };
        // D3 = sensor de luz (0 = viu luz), D4 = gatilho (1 = apertado)
        let mut v = 0x08;
        if z.trigger {
            v |= 0x10;
        }
        if self.sees_light(&z) {
            v &= !0x08;
        }
        v
    }

    /// A fotocélula só enxerga o feixe recém-desenhado: pixel claro **e** a varredura passando
    /// pelas linhas logo abaixo da mira.
    fn sees_light(&self, z: &Zapper) -> bool {
        if z.x >= crate::SCREEN_W as u16 || z.y >= crate::SCREEN_H as u16 {
            return false;
        }
        let line = self.ppu.scanline;
        if line < z.y as i16 || line > z.y as i16 + 12 {
            return false;
        }
        let idx = self.ppu.screen[z.y as usize * crate::SCREEN_W + z.x as usize];
        let c = crate::ppu::PALETTE_RGBA[(idx & 0x1FF) as usize];
        // luminância aproximada; o Zapper responde a branco forte
        let lum = c[0] as u32 * 54 + c[1] as u32 * 183 + c[2] as u32 * 19;
        lum > 200 * 256
    }

    #[inline]
    fn read_controller(&mut self, port: usize) -> u8 {
        if self.controller_strobe {
            // Durante o strobe, sempre o botão A
            self.controller[port] >> 7
        } else {
            let data = (self.controller_state[port] & 0x80) >> 7;
            // depois dos 8 botões o controle padrão devolve 1 (alguns jogos/testes contam com isso)
            self.controller_state[port] = (self.controller_state[port] << 1) | 1;
            data
        }
    }

    /// Leitura sem efeitos colaterais (debug/testes).
    pub fn cpu_read_debug(&self, addr: u16) -> u8 {
        match addr {
            0x0000..=0x1FFF => self.ram[(addr & 0x07FF) as usize],
            0x2000..=0x3FFF => self.ppu.cpu_read_debug(addr & 0x0007),
            0x4015 => self.apu.peek_status(),
            0x4000..=0x401F => self.open_bus,
            _ => self.cartridge.cpu_read(addr).unwrap_or(self.open_bus),
        }
    }

    /// Estado do bus para save state (feature `serde`).
    #[cfg(feature = "serde")]
    pub fn state(&self) -> crate::state::BusState {
        crate::state::BusState {
            ram: self.ram,
            cpu_cycles: self.cpu_cycles,
            oam_dma_page: self.oam_dma_page,
            open_bus: self.open_bus,
            controller: self.controller,
            controller_state: self.controller_state,
            controller_strobe: self.controller_strobe,
        }
    }

    #[cfg(feature = "serde")]
    pub fn restore(&mut self, st: crate::state::BusState) {
        self.ram = st.ram;
        self.cpu_cycles = st.cpu_cycles;
        self.oam_dma_page = st.oam_dma_page;
        self.open_bus = st.open_bus;
        self.controller = st.controller;
        self.controller_state = st.controller_state;
        self.controller_strobe = st.controller_strobe;
    }

    /// Reset do console: RAM preservada (como no hardware), PPU/APU/mapper reiniciados.
    pub fn reset(&mut self) {
        self.cartridge.reset();
        // Reset por botão: só os registradores da PPU reiniciam; VRAM, paleta e OAM ficam
        let mut ppu = Ppu::new();
        ppu.nametable = self.ppu.nametable;
        ppu.palette_table = self.ppu.palette_table;
        ppu.oam = self.ppu.oam;
        self.ppu = ppu;
        self.apu.reset(true);
        self.oam_dma_page = None;
        self.open_bus = 0;
        self.controller = [0; 2];
        self.controller_state = [0; 2];
        self.controller_strobe = false;
    }
}
