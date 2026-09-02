use crate::bus::Bus;
use crate::buttons::Buttons;
use crate::cartridge::Cartridge;
use crate::cpu6502::Cpu6502;
use crate::debug::Debugger;

/// Um console completo: CPU, bus (PPU, APU, RAM, cartucho) e debugger.
pub struct Nes {
    pub cpu: Cpu6502,
    pub bus: Bus,
    pub debugger: Debugger,
    system_clock_counter: u32,
}

impl Nes {
    /// Cria o console com o cartucho inserido e já resetado.
    pub fn new(cartridge: Cartridge) -> Nes {
        let mut nes = Nes {
            cpu: Cpu6502::new(),
            bus: Bus::new(cartridge),
            debugger: Debugger::new(),
            system_clock_counter: 0,
        };
        nes.reset();
        nes
    }

    pub fn cartridge(&self) -> &Cartridge {
        &self.bus.cartridge
    }

    pub fn cartridge_mut(&mut self) -> &mut Cartridge {
        &mut self.bus.cartridge
    }

    /// Avança um ciclo de PPU (3 por ciclo de CPU).
    pub fn clock(&mut self) {
        // cart_ptr vale pelo clock inteiro (fetches da PPU e escritas de CHR RAM)
        self.bus.ppu.cart_ptr = Some(&mut self.bus.cartridge as *mut Cartridge);
        self.bus.ppu.clock(Some(&mut self.bus.cartridge));

        // Atualizar mirroring cada frame
        if self.bus.ppu.scanline == -1 && self.bus.ppu.cycle == 0 {
            self.bus.ppu.mirror_mode = Bus::mirror_code(self.bus.cartridge.get_mirror());
        }

        // MMC3 scanline IRQ
        if self.bus.ppu.scanline_trigger {
            self.bus.ppu.scanline_trigger = false;
            self.bus.cartridge.clock_scanline();
            if self.bus.cartridge.mapper_irq() {
                self.cpu.irq(&mut self.bus);
            }
        }

        if self.system_clock_counter % 3 == 0 {
            self.bus.apu.clock();
            if let Some(addr) = self.bus.apu.dmc_read_addr.take() {
                let data = self.bus.cpu_read(addr, false);
                self.bus.apu.dmc_feed_sample(data);
            }
            if self.bus.dma_transfer {
                if self.bus.dma_dummy {
                    if self.system_clock_counter % 2 == 1 {
                        self.bus.dma_dummy = false;
                    }
                } else if self.system_clock_counter % 2 == 0 {
                    self.bus.dma_data =
                        self.bus.cpu_read((self.bus.dma_page as u16) << 8 | self.bus.dma_addr as u16, false);
                } else {
                    self.bus.ppu.cpu_write(0x0004, self.bus.dma_data);
                    self.bus.dma_addr = self.bus.dma_addr.wrapping_add(1);
                    if self.bus.dma_addr == 0x00 {
                        self.bus.dma_transfer = false;
                        self.bus.dma_dummy = true;
                    }
                }
            } else {
                if self.cpu.is_instruction_start() {
                    self.debugger.on_instruction(&self.cpu, &self.bus);
                }
                self.cpu.clock(&mut self.bus);
            }
        }

        if self.bus.ppu.get_nmi() {
            self.cpu.nmi(&mut self.bus);
        }

        self.system_clock_counter += 1;
    }

    /// Executa uma instrução completa da CPU (com os ciclos de PPU/APU correspondentes).
    pub fn step_instruction(&mut self) {
        // Sai do estado "início de instrução" atual…
        loop {
            self.clock();
            if self.system_clock_counter % 3 == 0 && !self.cpu.is_instruction_start() {
                break;
            }
            if self.system_clock_counter % 3 == 0 && self.bus.dma_transfer {
                // DMA em andamento: a CPU está parada, conta como "passo"
                return;
            }
        }
        // …e roda até o próximo.
        while !(self.system_clock_counter % 3 == 0 && self.cpu.is_instruction_start()) {
            self.clock();
        }
    }

    /// Emula até o fim do frame atual (o `frame_complete` da PPU é consumido aqui).
    pub fn run_frame(&mut self) {
        loop {
            self.clock();
            if self.bus.ppu.frame_complete {
                self.bus.ppu.frame_complete = false;
                break;
            }
        }
        // Headless: ninguém drenou o áudio — não deixar crescer sem limite
        let buf = &mut self.bus.apu.sample_buffer;
        if buf.len() > 8192 {
            let excess = buf.len() - 8192;
            buf.drain(..excess);
        }
    }

    /// Imagem do frame atual, RGBA8, 256×240×4 bytes.
    pub fn framebuffer(&self) -> &[u8] {
        self.bus.ppu.screen.as_flattened()
    }

    /// Leitura de memória sem efeitos colaterais (RAM, PPU, cartucho).
    pub fn peek(&self, addr: u16) -> u8 {
        self.bus.cpu_read_debug(addr)
    }

    pub fn set_controller(&mut self, port: usize, buttons: Buttons) {
        if port < 2 {
            self.bus.controller[port] = buttons.0;
        }
    }

    pub fn set_sample_rate(&mut self, hz: u32) {
        self.bus.apu.set_sample_rate(hz as f32);
    }

    /// Move as amostras de áudio geradas desde a última chamada para `out`.
    pub fn drain_audio(&mut self, out: &mut Vec<f32>) {
        out.extend_from_slice(&self.bus.apu.sample_buffer);
        self.bus.apu.sample_buffer.clear();
    }

    pub fn reset(&mut self) {
        self.bus.reset();
        self.cpu.reset(&mut self.bus);
        self.system_clock_counter = 0;
    }
}
