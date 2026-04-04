use crate::cpu6502::Cpu6502;
use crate::bus::Bus;
use crate::cartridge::Cartridge;
use crate::debug::Debugger;

pub struct Nes {
    pub cpu: Cpu6502,
    pub bus: Bus,
    pub debugger: Debugger,
    system_clock_counter: u32,
}

impl Nes {
    pub fn new() -> Nes {
        Nes {
            cpu: Cpu6502::new(),
            bus: Bus::new(),
            debugger: Debugger::new(),
            system_clock_counter: 0,
        }
    }

    pub fn insert_cartridge(&mut self, cartridge: Cartridge) {
        self.bus.insert_cartridge(cartridge);
        self.cpu.reset(&mut self.bus);
    }

    pub fn clock(&mut self) {
        self.bus.ppu.clock();

        // Atualizar CHR banks e mirroring durante pre-render (depois que NMI handler configurou tudo)
        if self.bus.ppu.scanline == -1 && self.bus.ppu.cycle == 0 {
            if let Some(ref mut cart) = self.bus.cartridge {
                self.bus.ppu.update_chr_from_cartridge(cart);
                self.bus.ppu.mirror_mode = match cart.get_mirror() {
                    crate::cartridge::Mirror::Vertical => 0,
                    crate::cartridge::Mirror::Horizontal => 1,
                    crate::cartridge::Mirror::OneScreenLo => 2,
                    crate::cartridge::Mirror::OneScreenHi => 3,
                };
            }
        }

        // MMC3 scanline IRQ
        if self.bus.ppu.scanline_trigger {
            self.bus.ppu.scanline_trigger = false;
            if let Some(ref mut cart) = self.bus.cartridge {
                cart.clock_scanline();
                if cart.mapper_irq() {
                    self.cpu.irq(&mut self.bus);
                }
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
                } else {
                    if self.system_clock_counter % 2 == 0 {
                        self.bus.dma_data = self.bus.cpu_read(
                            (self.bus.dma_page as u16) << 8 | self.bus.dma_addr as u16, false
                        );
                    } else {
                        self.bus.ppu.cpu_write(0x0004, self.bus.dma_data);
                        self.bus.dma_addr = self.bus.dma_addr.wrapping_add(1);
                        if self.bus.dma_addr == 0x00 {
                            self.bus.dma_transfer = false;
                            self.bus.dma_dummy = true;
                        }
                    }
                }
            } else {
                // Debug: trackear instrução antes de executar
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

    pub fn reset(&mut self) {
        self.bus.reset();
        self.cpu.reset(&mut self.bus);
        self.system_clock_counter = 0;
    }
}
