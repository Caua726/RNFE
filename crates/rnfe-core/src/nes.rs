use crate::bus::Bus;
use crate::buttons::Buttons;
use crate::cartridge::Cartridge;
use crate::cpu6502::Cpu6502;
use crate::debug::Debugger;

/// Um console completo: CPU, bus (PPU, APU, RAM, cartucho) e debugger.
///
/// A CPU comanda o relógio: cada acesso dela avança PPU e APU (ver [`Bus`]). A granularidade
/// externa é a instrução (`step_instruction`) ou o frame (`run_frame`).
pub struct Nes {
    pub cpu: Cpu6502,
    pub bus: Bus,
    pub debugger: Debugger,
    /// Cache RGBA do frame (convertido do framebuffer por índice só quando alguém pede).
    rgba: Box<[[u8; 4]; crate::SCREEN_W * crate::SCREEN_H]>,
    rgba_dirty: bool,
}

impl Nes {
    /// Cria o console com o cartucho inserido e já resetado.
    pub fn new(cartridge: Cartridge) -> Nes {
        let mut nes = Nes {
            cpu: Cpu6502::new(),
            bus: Bus::new(cartridge),
            debugger: Debugger::new(),
            rgba: vec![[0u8; 4]; crate::SCREEN_W * crate::SCREEN_H].into_boxed_slice().try_into().unwrap(),
            rgba_dirty: true,
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

    /// Executa uma instrução completa da CPU (com os ciclos de PPU/APU correspondentes).
    #[inline]
    pub fn step_instruction(&mut self) {
        if self.debugger.enabled {
            self.debugger.on_instruction(&self.cpu, &self.bus);
        }
        self.cpu.step(&mut self.bus);
    }

    /// Emula até o fim do frame atual (o `frame_complete` da PPU é consumido aqui).
    pub fn run_frame(&mut self) {
        while !self.bus.ppu.frame_complete {
            self.step_instruction();
        }
        self.bus.ppu.frame_complete = false;
        self.rgba_dirty = true;
        // Headless: ninguém drenou o áudio — não deixar crescer sem limite
        let buf = &mut self.bus.apu.sample_buffer;
        if buf.len() > 8192 {
            let excess = buf.len() - 8192;
            buf.drain(..excess);
        }
    }

    /// Ciclos de CPU desde o power-on.
    pub fn cpu_cycles(&self) -> u64 {
        self.bus.cpu_cycles
    }

    /// Imagem do frame atual, RGBA8, 256×240×4 bytes (convertida da paleta sob demanda).
    pub fn framebuffer(&mut self) -> &[u8] {
        if self.rgba_dirty {
            for (dst, &idx) in self.rgba.iter_mut().zip(self.bus.ppu.screen.iter()) {
                *dst = crate::ppu::PALETTE_RGBA[idx as usize];
            }
            self.rgba_dirty = false;
        }
        self.rgba.as_flattened()
    }

    /// Frame atual como índices de paleta de 9 bits (`ênfase << 6 | cor`), 256×240 — para
    /// frontends que aplicam a paleta na GPU (`ppu::PALETTE_RGBA`).
    pub fn framebuffer_indexed(&self) -> &[u16] {
        &self.bus.ppu.screen[..]
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
    }
}
