use crate::ppu::Ppu;
use crate::apu::Apu;
use crate::cartridge::{Cartridge, Mirror};

pub struct Bus {
    pub ppu: Ppu,
    pub apu: Apu,
    pub cartridge: Cartridge,
    pub ram: [u8; 2048],
    pub dma_page: u8,
    pub dma_addr: u8,
    pub dma_data: u8,
    pub dma_transfer: bool,
    pub dma_dummy: bool,
    // Controllers
    // Bits: A B Select Start Up Down Left Right
    pub controller: [u8; 2],
    controller_state: [u8; 2],
    controller_strobe: bool,
}

impl Bus {
    pub fn new(cartridge: Cartridge) -> Bus {
        let mut ppu = Ppu::new();
        ppu.load_chr(cartridge.get_chr_data());
        ppu.mirror_mode = Self::mirror_code(cartridge.get_mirror());

        Bus {
            ppu,
            apu: Apu::new(),
            cartridge,
            ram: [0u8; 2048],
            dma_page: 0x00,
            dma_addr: 0x00,
            dma_data: 0x00,
            dma_transfer: false,
            dma_dummy: true,
            controller: [0; 2],
            controller_state: [0; 2],
            controller_strobe: false,
        }
    }

    /// Código de mirroring usado internamente pela PPU.
    pub fn mirror_code(mirror: Mirror) -> u8 {
        match mirror {
            Mirror::Vertical => 0,
            Mirror::Horizontal => 1,
            Mirror::OneScreenLo => 2,
            Mirror::OneScreenHi => 3,
        }
    }

    pub fn cpu_write(&mut self, addr: u16, data: u8) {
        if self.cartridge.cpu_write(addr, data) {
            return;
        }

        match addr {
            0x0000..=0x1FFF => {
                self.ram[(addr & 0x07FF) as usize] = data;
            },
            0x2000..=0x3FFF => {
                self.ppu.cpu_write(addr & 0x0007, data);
            },
            0x4000..=0x4013 | 0x4015 => {
                self.apu.cpu_write(addr, data);
            },
            0x4014 => {
                self.dma_page = data;
                self.dma_addr = 0x00;
                self.dma_transfer = true;
            },
            0x4016 => {
                if data & 0x01 != 0 {
                    self.controller_strobe = true;
                } else {
                    if self.controller_strobe {
                        // Snapshot quando strobe desliga
                        self.controller_state[0] = self.controller[0];
                        self.controller_state[1] = self.controller[1];
                    }
                    self.controller_strobe = false;
                }
            },
            0x4017 => {
                self.apu.cpu_write(addr, data);
            },
            _ => {}
        }
    }

    pub fn cpu_read(&mut self, addr: u16, _read_only: bool) -> u8 {
        if let Some(data) = self.cartridge.cpu_read(addr) {
            return data;
        }

        match addr {
            0x0000..=0x1FFF => {
                self.ram[(addr & 0x07FF) as usize]
            },
            0x2000..=0x3FFF => {
                self.ppu.cpu_read(addr & 0x0007, _read_only)
            },
            0x4000..=0x4013 | 0x4015 => self.apu.cpu_read(addr),
            0x4016 => {
                if self.controller_strobe {
                    // Durante strobe, retorna estado do botão A
                    self.controller[0] >> 7
                } else {
                    let data = (self.controller_state[0] & 0x80) >> 7;
                    self.controller_state[0] <<= 1;
                    data
                }
            },
            0x4017 => {
                if self.controller_strobe {
                    self.controller[1] >> 7
                } else {
                    let data = (self.controller_state[1] & 0x80) >> 7;
                    self.controller_state[1] <<= 1;
                    data
                }
            },
            _ => 0x00,
        }
    }

    // Read sem side effects (pra debug)
    pub fn cpu_read_debug(&self, addr: u16) -> u8 {
        if let Some(data) = self.cartridge.cpu_read(addr) {
            return data;
        }
        match addr {
            0x0000..=0x1FFF => self.ram[(addr & 0x07FF) as usize],
            0x2000..=0x3FFF => self.ppu.cpu_read_debug(addr & 0x0007),
            _ => 0x00,
        }
    }

    pub fn reset(&mut self) {
        self.cartridge.reset();
        let mirror = self.ppu.mirror_mode;
        self.ppu = Ppu::new();
        self.ppu.mirror_mode = mirror;
        self.ppu.load_chr(self.cartridge.get_chr_data());
        self.dma_page = 0x00;
        self.dma_addr = 0x00;
        self.dma_data = 0x00;
        self.dma_transfer = false;
        self.dma_dummy = true;
        self.apu.reset();
        self.controller = [0; 2];
        self.controller_state = [0; 2];
        self.controller_strobe = false;
    }
}
