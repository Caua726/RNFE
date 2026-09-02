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

pub struct Bus {
    pub ppu: Ppu,
    pub apu: Apu,
    pub cartridge: Cartridge,
    pub ram: [u8; 2048],
    /// Ciclos de CPU desde o power-on (o reset conta os seus 7).
    pub cpu_cycles: u64,
    /// O mapper quer `cpu_clock` a cada ciclo (lido uma vez do cartucho).
    mapper_clock: bool,
    /// Página pedida por `$4014`; a CPU consome no próximo ciclo de leitura.
    oam_dma_page: Option<u8>,
    /// Último valor no barramento de dados (lido em endereços não mapeados).
    open_bus: u8,
    // Controles. Bits: A B Select Start Up Down Left Right
    pub controller: [u8; 2],
    controller_state: [u8; 2],
    controller_strobe: bool,
}

impl Bus {
    pub fn new(cartridge: Cartridge) -> Bus {
        Bus {
            mapper_clock: cartridge.wants_cpu_clock(),
            ppu: Ppu::new(),
            apu: Apu::new(),
            cartridge,
            ram: [0u8; 2048],
            cpu_cycles: 0,
            oam_dma_page: None,
            open_bus: 0,
            controller: [0; 2],
            controller_state: [0; 2],
            controller_strobe: false,
        }
    }

    // ------------------------------------------------------------------ relógio

    /// Começo de um ciclo de CPU: os dots de PPU que antecedem o acesso.
    #[inline]
    pub fn tick_pre(&mut self) {
        // A APU avança antes do acesso: uma leitura de $4015 vê o mesmo estado que a linha IRQ
        // amostrada no fim deste ciclo.
        self.apu.clock();
        for _ in 0..DOTS_BEFORE_ACCESS {
            self.ppu.step(&mut self.cartridge);
        }
    }

    /// Fim de um ciclo de CPU: dots restantes e mapper.
    #[inline]
    pub fn tick_post(&mut self) {
        for _ in DOTS_BEFORE_ACCESS..3 {
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
            0x4017 => self.read_controller(1) | (self.open_bus & 0xE0),
            0x4000..=0x401F => self.open_bus,
            _ => self.cartridge.cpu_read(addr).unwrap_or(self.open_bus),
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
                self.cartridge.cpu_write(addr, data);
            }
        }
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

    /// Reset do console: RAM preservada (como no hardware), PPU/APU/mapper reiniciados.
    pub fn reset(&mut self) {
        self.cartridge.reset();
        self.ppu = Ppu::new();
        self.apu.reset(true);
        self.oam_dma_page = None;
        self.open_bus = 0;
        self.controller = [0; 2];
        self.controller_state = [0; 2];
        self.controller_strobe = false;
    }
}
