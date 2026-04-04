use crate::ppu::Ppu;
use crate::cartridge::Cartridge;

pub struct Bus {
    pub ppu: Ppu,
    pub cartridge: Option<Cartridge>,
    pub ram: [u8; 2048],
    pub dma_page: u8,
    pub dma_addr: u8,
    pub dma_data: u8,
    pub dma_transfer: bool,
    pub dma_dummy: bool,
}

impl Bus {
    pub fn new() -> Bus {
        Bus {
            ppu: Ppu::new(),
            cartridge: None,
            ram: [0; 2048],
            dma_page: 0x00,
            dma_addr: 0x00,
            dma_data: 0x00,
            dma_transfer: false,
            dma_dummy: true,
        }
    }

    pub fn insert_cartridge(&mut self, cartridge: Cartridge) {
        self.ppu.load_chr(cartridge.get_chr_data());
        self.ppu.mirror_horizontal = matches!(cartridge.get_mirror(), crate::cartridge::Mirror::Horizontal);
        self.cartridge = Some(cartridge);
    }

    pub fn cpu_write(&mut self, addr: u16, data: u8) {
        if let Some(ref mut cartridge) = self.cartridge {
            if cartridge.cpu_write(addr, data) {
                return;
            }
        }

        match addr {
            0x0000..=0x1FFF => {
                self.ram[(addr & 0x07FF) as usize] = data;
            },
            0x2000..=0x3FFF => {
                self.ppu.cpu_write(addr & 0x0007, data);
            },
            0x4000..=0x4013 | 0x4015 => {},
            0x4014 => {
                self.dma_page = data;
                self.dma_addr = 0x00;
                self.dma_transfer = true;
            },
            0x4016 | 0x4017 => {},
            _ => {}
        }
    }

    pub fn cpu_read(&mut self, addr: u16, _read_only: bool) -> u8 {
        if let Some(ref cartridge) = self.cartridge {
            if let Some(data) = cartridge.cpu_read(addr) {
                return data;
            }
        }

        match addr {
            0x0000..=0x1FFF => {
                self.ram[(addr & 0x07FF) as usize]
            },
            0x2000..=0x3FFF => {
                self.ppu.cpu_read(addr & 0x0007, _read_only)
            },
            0x4000..=0x4013 | 0x4015 => 0x00,
            0x4016 | 0x4017 => 0x00,
            _ => 0x00,
        }
    }

    pub fn reset(&mut self) {
        if let Some(ref mut cartridge) = self.cartridge {
            cartridge.reset();
        }
        let mirror = self.ppu.mirror_horizontal;
        self.ppu = Ppu::new();
        self.ppu.mirror_horizontal = mirror;
        if let Some(ref cartridge) = self.cartridge {
            self.ppu.load_chr(cartridge.get_chr_data());
        }
        self.dma_page = 0x00;
        self.dma_addr = 0x00;
        self.dma_data = 0x00;
        self.dma_transfer = false;
        self.dma_dummy = true;
    }
}
