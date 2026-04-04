use std::fs::File;
use std::io::Read;

pub struct Cartridge {
    prg_memory: Vec<u8>,
    chr_memory: Vec<u8>,
    mapper_id: u8,
    prg_banks: u8,
    chr_banks: u8,
    mirror: Mirror,
    
    // MMC1 state
    mmc1_shift: u8,
    mmc1_shift_count: u8,
    mmc1_control: u8,
    mmc1_chr_bank0: u8,
    mmc1_chr_bank1: u8,
    mmc1_prg_bank: u8,

    // UxROM (mapper 2) state
    uxrom_bank: u8,

    // CNROM (mapper 3) state
    cnrom_chr_bank: u8,

    // AxROM (mapper 7) state
    axrom_prg_bank: u8,

    // GxROM (mapper 66) state
    gxrom_prg_bank: u8,
    gxrom_chr_bank: u8,

    // Color Dreams (mapper 11) state
    colordreams_prg_bank: u8,
    colordreams_chr_bank: u8,

    // PRG RAM (8KB, usado por MMC1, MMC3, etc)
    prg_ram: Vec<u8>,

    // MMC3 state
    mmc3_bank_select: u8,
    mmc3_prg_banks: [u8; 4],
    mmc3_chr_banks: [u8; 8],
    mmc3_irq_counter: u8,
    mmc3_irq_reload: u8,
    mmc3_irq_enabled: bool,
    mmc3_irq_pending: bool,

    // MMC2 (mapper 9) state
    mmc2_prg_bank: u8,
    mmc2_chr_banks: [u8; 4],
    mmc2_latch: [u8; 2],

    // BNROM (mapper 34) state
    bnrom_prg_bank: u8,

    // Camerica (mapper 71) state
    camerica_prg_bank: u8,

    // FME-7 (mapper 69) state
    fme7_command: u8,
    fme7_prg_banks: [u8; 4],
    fme7_chr_banks: [u8; 8],

    // Mapper 227 (multicart) state
    m227_reg: u16,

    // DxROM (mapper 206) state
    dxrom_bank_select: u8,
    dxrom_prg_banks: [u8; 4],
    dxrom_chr_banks: [u8; 8],
}

#[derive(Debug, Clone, Copy)]
pub enum Mirror {
    Horizontal,
    Vertical,
    OneScreenLo,
    OneScreenHi,
}

impl Cartridge {
    pub fn new(filename: &str) -> Result<Self, Box<dyn std::error::Error>> {
        let mut file = File::open(filename)?;
        let mut buffer = Vec::new();
        file.read_to_end(&mut buffer)?;
        
        // Parse iNES header
        if buffer.len() < 16 || &buffer[0..4] != b"NES\x1A" {
            return Err("Invalid NES ROM format".into());
        }
        
        let prg_banks = buffer[4];
        let chr_banks = buffer[5];
        let mapper1 = buffer[6];
        let mapper2 = buffer[7];
        let _prg_ram_size = buffer[8];
        
        // Skip trainer if present
        let mut file_offset = 16;
        if (mapper1 & 0x04) != 0 {
            file_offset += 512;
        }
        
        // Determine mapper ID
        let mapper_id = (mapper2 & 0xF0) | (mapper1 >> 4);
        
        // Determine mirroring
        // iNES: bit0=0 -> horizontal arrangement (vertical mirroring)
        //       bit0=1 -> vertical arrangement (horizontal mirroring)
        let mirror = if (mapper1 & 0x01) != 0 {
            Mirror::Horizontal
        } else {
            Mirror::Vertical
        };
        
        // Read PRG ROM
        let prg_size = prg_banks as usize * 16384;
        let prg_memory = buffer[file_offset..file_offset + prg_size].to_vec();
        file_offset += prg_size;
        
        // Read CHR ROM
        let chr_size = chr_banks as usize * 8192;
        let chr_memory = if chr_size > 0 {
            buffer[file_offset..file_offset + chr_size].to_vec()
        } else {
            // CHR RAM
            vec![0; 8192]
        };
        
        let supported = matches!(mapper_id, 0 | 1 | 2 | 3 | 4 | 7 | 9 | 11 | 34 | 66 | 69 | 71 | 206 | 227);
        println!("Cartridge loaded: PRG banks: {}, CHR banks: {}, Mapper: {}, Mirror: {:?}",
                 prg_banks, chr_banks, mapper_id, mirror);
        println!("PRG ROM size: {} bytes, CHR ROM size: {} bytes", prg_size, chr_size);
        if !supported {
            eprintln!("WARNING: Mapper {} not supported! Game may not work.", mapper_id);
        }
        
        Ok(Cartridge {
            prg_memory,
            chr_memory,
            mapper_id,
            prg_banks,
            chr_banks,
            mirror,
            
            // MMC1
            prg_ram: vec![0; 8192],

            mmc1_shift: 0x10,
            mmc1_shift_count: 0,
            mmc1_control: 0x0C, // PRG 16KB mode, fix last bank
            mmc1_chr_bank0: 0,
            mmc1_chr_bank1: 0,
            mmc1_prg_bank: 0,

            // UxROM
            uxrom_bank: 0,

            // CNROM
            cnrom_chr_bank: 0,

            // AxROM
            axrom_prg_bank: 0,

            // GxROM
            gxrom_prg_bank: 0,
            gxrom_chr_bank: 0,

            // Color Dreams
            colordreams_prg_bank: 0,
            colordreams_chr_bank: 0,

            // MMC3
            mmc3_bank_select: 0,
            mmc3_prg_banks: [0, 1, (prg_banks * 2).wrapping_sub(2), (prg_banks * 2).wrapping_sub(1)],
            mmc3_chr_banks: [0, 1, 2, 3, 4, 5, 6, 7],
            mmc3_irq_counter: 0,
            mmc3_irq_reload: 0,
            mmc3_irq_enabled: false,
            mmc3_irq_pending: false,

            // MMC2
            mmc2_prg_bank: 0,
            mmc2_chr_banks: [0; 4],
            mmc2_latch: [0xFE, 0xFE],

            // BNROM
            bnrom_prg_bank: 0,

            // Camerica
            camerica_prg_bank: 0,

            // FME-7
            fme7_command: 0,
            fme7_prg_banks: [0; 4],
            fme7_chr_banks: [0; 8],

            // Mapper 227
            m227_reg: 0,

            // DxROM
            dxrom_bank_select: 0,
            dxrom_prg_banks: [0, 1, (prg_banks * 2).wrapping_sub(2), (prg_banks * 2).wrapping_sub(1)],
            dxrom_chr_banks: [0, 1, 2, 3, 4, 5, 6, 7],
        })
    }
    
    pub fn cpu_read(&self, addr: u16) -> Option<u8> {
        match self.mapper_id {
            0 => self.mapper_000_cpu_read(addr),
            1 => self.mapper_001_cpu_read(addr),
            2 => self.mapper_002_cpu_read(addr),
            3 => self.mapper_003_cpu_read(addr),
            4 => self.mapper_004_cpu_read(addr),
            7 => self.mapper_007_cpu_read(addr),
            11 => self.mapper_011_cpu_read(addr),
            66 => self.mapper_066_cpu_read(addr),
            9 => self.mapper_009_cpu_read(addr),
            34 => self.mapper_034_cpu_read(addr),
            69 => self.mapper_069_cpu_read(addr),
            71 => self.mapper_071_cpu_read(addr),
            206 => self.mapper_206_cpu_read(addr),
            227 => self.mapper_227_cpu_read(addr),
            _ => None,
        }
    }

    pub fn cpu_write(&mut self, addr: u16, data: u8) -> bool {
        match self.mapper_id {
            0 => self.mapper_000_cpu_write(addr, data),
            1 => self.mapper_001_cpu_write(addr, data),
            2 => self.mapper_002_cpu_write(addr, data),
            3 => self.mapper_003_cpu_write(addr, data),
            4 => self.mapper_004_cpu_write(addr, data),
            7 => self.mapper_007_cpu_write(addr, data),
            11 => self.mapper_011_cpu_write(addr, data),
            66 => self.mapper_066_cpu_write(addr, data),
            9 => self.mapper_009_cpu_write(addr, data),
            34 => self.mapper_034_cpu_write(addr, data),
            69 => self.mapper_069_cpu_write(addr, data),
            71 => self.mapper_071_cpu_write(addr, data),
            206 => self.mapper_206_cpu_write(addr, data),
            227 => self.mapper_227_cpu_write(addr, data),
            _ => false,
        }
    }

    pub fn ppu_read(&mut self, addr: u16) -> Option<u8> {
        match self.mapper_id {
            0 => self.mapper_000_ppu_read(addr),
            1 => self.mapper_001_ppu_read(addr),
            2 | 7 => self.mapper_002_ppu_read(addr), // CHR RAM simples
            3 => self.mapper_003_ppu_read(addr),
            4 => self.mapper_004_ppu_read(addr),
            11 | 66 => self.mapper_066_ppu_read(addr),
            9 => self.mapper_009_ppu_read(addr),
            34 | 71 | 227 => self.mapper_002_ppu_read(addr), // CHR RAM
            69 => self.mapper_069_ppu_read(addr),
            206 => self.mapper_206_ppu_read(addr),
            _ => None,
        }
    }

    pub fn ppu_write(&mut self, addr: u16, data: u8) -> bool {
        match self.mapper_id {
            0 => self.mapper_000_ppu_write(addr, data),
            1 | 2 | 7 => {
                if addr <= 0x1FFF && self.chr_banks == 0 {
                    self.chr_memory[addr as usize] = data;
                    true
                } else {
                    false
                }
            },
            3 | 11 | 66 | 9 => false, // CHR ROM, read-only
            4 | 206 => {
                if addr <= 0x1FFF && self.chr_banks == 0 {
                    self.chr_memory[addr as usize] = data;
                    true
                } else {
                    false
                }
            },
            34 | 71 | 227 => {
                if addr <= 0x1FFF && self.chr_banks == 0 {
                    self.chr_memory[addr as usize] = data;
                    true
                } else {
                    false
                }
            },
            69 => {
                if addr <= 0x1FFF && self.chr_banks == 0 {
                    self.chr_memory[addr as usize] = data;
                    true
                } else {
                    false
                }
            },
            _ => false,
        }
    }
    
    pub fn get_mirror(&self) -> Mirror {
        self.mirror
    }

    pub fn ppu_write_chr(&mut self, addr: u16, data: u8) {
        if addr <= 0x1FFF && self.chr_banks == 0 {
            // CHR RAM - escreve direto
            let idx = addr as usize;
            if idx < self.chr_memory.len() {
                self.chr_memory[idx] = data;
            }
        }
    }

    pub fn get_chr_data(&self) -> &[u8] {
        &self.chr_memory
    }

    // Debug: ler CHR sem side effects
    pub fn cpu_read_chr_debug(&self, addr: u16) -> Option<u8> {
        if addr <= 0x1FFF {
            // Ler via mapper sem &mut
            match self.mapper_id {
                0 => self.mapper_000_ppu_read(addr),
                4 => self.mapper_004_ppu_read(addr),
                _ => {
                    if (addr as usize) < self.chr_memory.len() {
                        Some(self.chr_memory[addr as usize])
                    } else {
                        Some(0)
                    }
                }
            }
        } else {
            None
        }
    }

    pub fn print_mapper_state(&self) {
        println!("  Mapper: {}  PRG banks: {}  CHR banks: {}", self.mapper_id, self.prg_banks, self.chr_banks);
        match self.mapper_id {
            4 => {
                println!("  MMC3 bank_select: ${:02X} (CHR_A12_inv={} PRG_mode={})",
                    self.mmc3_bank_select,
                    if self.mmc3_bank_select & 0x80 != 0 { "yes" } else { "no" },
                    if self.mmc3_bank_select & 0x40 != 0 { "swap" } else { "normal" });
                println!("  MMC3 PRG banks: [{}, {}, {}, {}]",
                    self.mmc3_prg_banks[0], self.mmc3_prg_banks[1],
                    self.mmc3_prg_banks[2], self.mmc3_prg_banks[3]);
                println!("  MMC3 CHR banks: [{}, {}, {}, {}, {}, {}, {}, {}]",
                    self.mmc3_chr_banks[0], self.mmc3_chr_banks[1],
                    self.mmc3_chr_banks[2], self.mmc3_chr_banks[3],
                    self.mmc3_chr_banks[4], self.mmc3_chr_banks[5],
                    self.mmc3_chr_banks[6], self.mmc3_chr_banks[7]);
                println!("  MMC3 IRQ: counter={} reload={} enabled={} pending={}",
                    self.mmc3_irq_counter, self.mmc3_irq_reload,
                    self.mmc3_irq_enabled, self.mmc3_irq_pending);
            },
            1 => {
                println!("  MMC1 ctrl: ${:02X}  PRG bank: {}  CHR banks: {}/{}",
                    self.mmc1_control, self.mmc1_prg_bank,
                    self.mmc1_chr_bank0, self.mmc1_chr_bank1);
            },
            _ => {}
        }
    }
    
    // Mapper 000 (NROM) implementation
    fn mapper_000_cpu_read(&self, addr: u16) -> Option<u8> {
        if addr >= 0x8000 && addr <= 0xFFFF {
            let masked_addr = addr & if self.prg_banks > 1 { 0x7FFF } else { 0x3FFF };
            let index = (masked_addr & 0x3FFF) as usize;
            Some(self.prg_memory[index])
        } else {
            None
        }
    }
    
    fn mapper_000_cpu_write(&mut self, addr: u16, _data: u8) -> bool {
        if addr >= 0x8000 && addr <= 0xFFFF {
            false // PRG ROM is read-only
        } else {
            false
        }
    }
    
    fn mapper_000_ppu_read(&self, addr: u16) -> Option<u8> {
        if addr >= 0x0000 && addr <= 0x1FFF {
            Some(self.chr_memory[addr as usize])
        } else {
            None
        }
    }
    
    fn mapper_000_ppu_write(&mut self, addr: u16, data: u8) -> bool {
        if addr >= 0x0000 && addr <= 0x1FFF {
            if self.chr_banks == 0 { // CHR RAM
                self.chr_memory[addr as usize] = data;
                true
            } else {
                false // CHR ROM is read-only
            }
        } else {
            false
        }
    }
    
    // Mapper 001 (MMC1)
    fn mapper_001_cpu_read(&self, addr: u16) -> Option<u8> {
        if addr >= 0x8000 {
            let prg_mode = (self.mmc1_control >> 2) & 0x03;
            let bank = match prg_mode {
                0 | 1 => {
                    // 32KB mode
                    let b = (self.mmc1_prg_bank & 0x0E) as usize;
                    b * 0x4000 + (addr as usize - 0x8000)
                },
                2 => {
                    // Fix first, switch second
                    if addr < 0xC000 {
                        addr as usize - 0x8000
                    } else {
                        (self.mmc1_prg_bank & 0x0F) as usize * 0x4000 + (addr as usize - 0xC000)
                    }
                },
                3 | _ => {
                    // Switch first, fix last
                    if addr < 0xC000 {
                        (self.mmc1_prg_bank & 0x0F) as usize * 0x4000 + (addr as usize - 0x8000)
                    } else {
                        (self.prg_banks as usize - 1) * 0x4000 + (addr as usize - 0xC000)
                    }
                },
            };
            if bank < self.prg_memory.len() {
                Some(self.prg_memory[bank])
            } else {
                Some(0)
            }
        } else if addr >= 0x6000 {
            Some(self.prg_ram[(addr - 0x6000) as usize])
        } else {
            None
        }
    }

    fn mapper_001_cpu_write(&mut self, addr: u16, data: u8) -> bool {
        if addr >= 0x6000 && addr < 0x8000 {
            self.prg_ram[(addr - 0x6000) as usize] = data;
            return true;
        }
        if addr >= 0x8000 {
            if data & 0x80 != 0 {
                // Reset shift register
                self.mmc1_shift = 0x10;
                self.mmc1_shift_count = 0;
                self.mmc1_control |= 0x0C;
            } else {
                self.mmc1_shift >>= 1;
                self.mmc1_shift |= (data & 0x01) << 4;
                self.mmc1_shift_count += 1;

                if self.mmc1_shift_count == 5 {
                    let value = self.mmc1_shift;
                    match addr {
                        0x8000..=0x9FFF => {
                            self.mmc1_control = value;
                            self.mirror = match value & 0x03 {
                                0 => Mirror::OneScreenLo,
                                1 => Mirror::OneScreenHi,
                                2 => Mirror::Vertical,
                                3 => Mirror::Horizontal,
                                _ => self.mirror,
                            };
                        },
                        0xA000..=0xBFFF => self.mmc1_chr_bank0 = value,
                        0xC000..=0xDFFF => self.mmc1_chr_bank1 = value,
                        0xE000..=0xFFFF => self.mmc1_prg_bank = value & 0x0F,
                        _ => {}
                    }
                    self.mmc1_shift = 0x10;
                    self.mmc1_shift_count = 0;
                }
            }
            true
        } else {
            false
        }
    }

    fn mapper_001_ppu_read(&self, addr: u16) -> Option<u8> {
        if addr <= 0x1FFF {
            if self.chr_banks == 0 {
                // CHR RAM
                Some(self.chr_memory[addr as usize])
            } else {
                let chr_mode = (self.mmc1_control >> 4) & 0x01;
                let bank_addr = if chr_mode == 0 {
                    // 8KB mode
                    let b = (self.mmc1_chr_bank0 & 0x1E) as usize;
                    b * 0x1000 + addr as usize
                } else {
                    // 4KB mode
                    if addr < 0x1000 {
                        self.mmc1_chr_bank0 as usize * 0x1000 + addr as usize
                    } else {
                        self.mmc1_chr_bank1 as usize * 0x1000 + (addr as usize - 0x1000)
                    }
                };
                if bank_addr < self.chr_memory.len() {
                    Some(self.chr_memory[bank_addr])
                } else {
                    Some(0)
                }
            }
        } else {
            None
        }
    }

    // Mapper 002 (UxROM)
    fn mapper_002_cpu_read(&self, addr: u16) -> Option<u8> {
        if addr >= 0xC000 {
            // Last bank fixed
            let offset = (self.prg_banks as usize - 1) * 0x4000 + (addr as usize - 0xC000);
            Some(self.prg_memory[offset % self.prg_memory.len()])
        } else if addr >= 0x8000 {
            // Switchable bank
            let offset = self.uxrom_bank as usize * 0x4000 + (addr as usize - 0x8000);
            Some(self.prg_memory[offset % self.prg_memory.len()])
        } else {
            None
        }
    }

    fn mapper_002_cpu_write(&mut self, addr: u16, data: u8) -> bool {
        if addr >= 0x8000 {
            self.uxrom_bank = data & 0x0F;
            true
        } else {
            false
        }
    }

    fn mapper_002_ppu_read(&self, addr: u16) -> Option<u8> {
        if addr <= 0x1FFF {
            Some(self.chr_memory[addr as usize])
        } else {
            None
        }
    }

    // Mapper 003 (CNROM) - CHR bank switching
    fn mapper_003_cpu_read(&self, addr: u16) -> Option<u8> {
        if addr >= 0x8000 {
            let masked = addr & if self.prg_banks > 1 { 0x7FFF } else { 0x3FFF };
            Some(self.prg_memory[masked as usize % self.prg_memory.len()])
        } else {
            None
        }
    }

    fn mapper_003_cpu_write(&mut self, addr: u16, data: u8) -> bool {
        if addr >= 0x8000 {
            self.cnrom_chr_bank = data & 0x03;
            true
        } else {
            false
        }
    }

    fn mapper_003_ppu_read(&self, addr: u16) -> Option<u8> {
        if addr <= 0x1FFF {
            let offset = self.cnrom_chr_bank as usize * 0x2000 + addr as usize;
            Some(self.chr_memory[offset % self.chr_memory.len()])
        } else {
            None
        }
    }

    // Mapper 007 (AxROM) - 32KB PRG switching + single screen mirroring
    fn mapper_007_cpu_read(&self, addr: u16) -> Option<u8> {
        if addr >= 0x8000 {
            let offset = self.axrom_prg_bank as usize * 0x8000 + (addr as usize - 0x8000);
            Some(self.prg_memory[offset % self.prg_memory.len()])
        } else {
            None
        }
    }

    fn mapper_007_cpu_write(&mut self, addr: u16, data: u8) -> bool {
        if addr >= 0x8000 {
            self.axrom_prg_bank = data & 0x07;
            self.mirror = if data & 0x10 != 0 { Mirror::OneScreenHi } else { Mirror::OneScreenLo };
            true
        } else {
            false
        }
    }

    // Mapper 227 (multicart chinês)
    // Mapper 227 (multicart chinês)
    // Registro = endereço escrito em $8000+
    // A0: mirroring (0=vert, 1=horiz)
    // A1: "last bank" flag - se 1, $C000-$FFFF mapeia o ultimo 16KB
    // A2-A6: PRG bank number (5 bits)
    // A7: mode (0=32KB, 1=16KB)
    fn mapper_227_cpu_read(&self, addr: u16) -> Option<u8> {
        if addr >= 0x8000 {
            let reg = self.m227_reg;
            let prg_bank = ((reg >> 2) & 0x1F) as usize;
            let mode_16k = (reg >> 7) & 1;
            let last_flag = (reg >> 1) & 1;

            let offset = if mode_16k != 0 {
                if addr >= 0xC000 {
                    // 16KB mode: $C000 = last bank
                    let last = self.prg_memory.len() - 0x4000;
                    last + (addr as usize - 0xC000)
                } else {
                    // $8000 = selected bank
                    prg_bank * 0x4000 + (addr as usize - 0x8000)
                }
            } else {
                // 32KB mode: $8000 = selected 16KB bank, $C000 = last 16KB (trampoline)
                if addr >= 0xC000 {
                    let last = self.prg_memory.len() - 0x4000;
                    last + (addr as usize - 0xC000)
                } else {
                    prg_bank * 0x4000 + (addr as usize - 0x8000)
                }
            };

            Some(self.prg_memory[offset % self.prg_memory.len()])
        } else {
            None
        }
    }

    fn mapper_227_cpu_write(&mut self, addr: u16, _data: u8) -> bool {
        if addr >= 0x8000 {
            self.m227_reg = addr;
            self.mirror = if addr & 0x01 != 0 { Mirror::Horizontal } else { Mirror::Vertical };
            true
        } else {
            false
        }
    }

    pub fn reset(&mut self) {
        if self.mapper_id == 1 {
            self.mmc1_shift = 0x10;
            self.mmc1_shift_count = 0;
            self.mmc1_control = 0x0C;
            self.mmc1_chr_bank0 = 0;
            self.mmc1_chr_bank1 = 0;
            self.mmc1_prg_bank = 0;
        } else if self.mapper_id == 2 {
            self.uxrom_bank = 0;
        } else if self.mapper_id == 3 {
            self.cnrom_chr_bank = 0;
        } else if self.mapper_id == 7 {
            self.axrom_prg_bank = 0;
        } else if self.mapper_id == 66 {
            self.gxrom_prg_bank = 0;
            self.gxrom_chr_bank = 0;
        } else if self.mapper_id == 11 {
            self.colordreams_prg_bank = 0;
            self.colordreams_chr_bank = 0;
        } else if self.mapper_id == 4 {
            self.mmc3_bank_select = 0;
            self.mmc3_prg_banks = [0, 1, (self.prg_banks * 2).wrapping_sub(2), (self.prg_banks * 2).wrapping_sub(1)];
            self.mmc3_chr_banks = [0, 1, 2, 3, 4, 5, 6, 7];
            self.mmc3_irq_counter = 0;
            self.mmc3_irq_reload = 0;
            self.mmc3_irq_enabled = false;
            self.mmc3_irq_pending = false;
        } else if self.mapper_id == 9 {
            self.mmc2_prg_bank = 0;
            self.mmc2_chr_banks = [0; 4];
            self.mmc2_latch = [0xFE, 0xFE];
        } else if self.mapper_id == 34 {
            self.bnrom_prg_bank = 0;
        } else if self.mapper_id == 69 {
            self.fme7_command = 0;
            self.fme7_prg_banks = [0; 4];
            self.fme7_chr_banks = [0; 8];
        } else if self.mapper_id == 71 {
            self.camerica_prg_bank = 0;
        } else if self.mapper_id == 206 {
            self.dxrom_bank_select = 0;
            self.dxrom_prg_banks = [0, 1, (self.prg_banks * 2).wrapping_sub(2), (self.prg_banks * 2).wrapping_sub(1)];
            self.dxrom_chr_banks = [0, 1, 2, 3, 4, 5, 6, 7];
        } else if self.mapper_id == 227 {
            // Bit 1 = last bank flag, mapeando ultimo 16KB em $C000
            self.m227_reg = 0x02;
        }
    }
    
    // MMC3 (Mapper 4) implementation
    fn mapper_004_cpu_read(&self, addr: u16) -> Option<u8> {
        if addr >= 0x6000 && addr < 0x8000 {
            return Some(self.prg_ram[(addr - 0x6000) as usize]);
        }
        if addr >= 0x8000 {
            let bank = match addr {
                0x8000..=0x9FFF => self.mmc3_prg_banks[0],
                0xA000..=0xBFFF => self.mmc3_prg_banks[1], 
                0xC000..=0xDFFF => self.mmc3_prg_banks[2],
                0xE000..=0xFFFF => self.mmc3_prg_banks[3],
                _ => 0
            };
            let bank_offset = (bank as usize) * 0x2000;
            let addr_offset = (addr & 0x1FFF) as usize;
            if bank_offset + addr_offset < self.prg_memory.len() {
                Some(self.prg_memory[bank_offset + addr_offset])
            } else {
                Some(0)
            }
        } else {
            None
        }
    }
    
    fn mapper_004_cpu_write(&mut self, addr: u16, data: u8) -> bool {
        if addr >= 0x6000 && addr < 0x8000 {
            self.prg_ram[(addr - 0x6000) as usize] = data;
            return true;
        }
        match addr {
            0x8000..=0x9FFF => {
                if addr % 2 == 0 {
                    // Bank select
                    self.mmc3_bank_select = data;
                } else {
                    // Bank data
                    let bank_register = self.mmc3_bank_select & 0x07;
                    match bank_register {
                        0 | 1 => {
                            self.mmc3_chr_banks[bank_register as usize * 2] = data & 0xFE;
                            self.mmc3_chr_banks[bank_register as usize * 2 + 1] = (data & 0xFE) + 1;
                        },
                        2..=5 => {
                            self.mmc3_chr_banks[bank_register as usize + 2] = data;
                        },
                        6 => {
                            self.mmc3_prg_banks[if (self.mmc3_bank_select & 0x40) != 0 { 2 } else { 0 }] = data;
                        },
                        7 => {
                            self.mmc3_prg_banks[1] = data;
                        },
                        _ => {}
                    }
                }
                // Manter fixed banks corretos
                let last = (self.prg_banks * 2).wrapping_sub(1);
                let second_last = (self.prg_banks * 2).wrapping_sub(2);
                if (self.mmc3_bank_select & 0x40) != 0 {
                    self.mmc3_prg_banks[0] = second_last;
                } else {
                    self.mmc3_prg_banks[2] = second_last;
                }
                self.mmc3_prg_banks[3] = last;
                true
            },
            0xA000..=0xBFFF => {
                if addr % 2 == 0 {
                    // Mirroring
                    self.mirror = if data & 0x01 != 0 { Mirror::Horizontal } else { Mirror::Vertical };
                }
                // Odd: PRG RAM protect (not implemented)
                true
            },
            0xC000..=0xDFFF => {
                if addr % 2 == 0 {
                    // IRQ latch
                    self.mmc3_irq_reload = data;
                } else {
                    // IRQ reload
                    self.mmc3_irq_counter = 0;
                }
                true
            },
            0xE000..=0xFFFF => {
                if addr % 2 == 0 {
                    // IRQ disable
                    self.mmc3_irq_enabled = false;
                    self.mmc3_irq_pending = false;
                } else {
                    // IRQ enable
                    self.mmc3_irq_enabled = true;
                }
                true
            },
            _ => false,
        }
    }

    fn mapper_004_ppu_read(&self, addr: u16) -> Option<u8> {
        if addr <= 0x1FFF {
            let chr_mode = (self.mmc3_bank_select & 0x80) != 0;
            let bank = if chr_mode {
                match addr {
                    0x0000..=0x03FF => self.mmc3_chr_banks[4],
                    0x0400..=0x07FF => self.mmc3_chr_banks[5],
                    0x0800..=0x0BFF => self.mmc3_chr_banks[6],
                    0x0C00..=0x0FFF => self.mmc3_chr_banks[7],
                    0x1000..=0x13FF => self.mmc3_chr_banks[0],
                    0x1400..=0x17FF => self.mmc3_chr_banks[1],
                    0x1800..=0x1BFF => self.mmc3_chr_banks[2],
                    0x1C00..=0x1FFF => self.mmc3_chr_banks[3],
                    _ => 0,
                }
            } else {
                match addr {
                    0x0000..=0x03FF => self.mmc3_chr_banks[0],
                    0x0400..=0x07FF => self.mmc3_chr_banks[1],
                    0x0800..=0x0BFF => self.mmc3_chr_banks[2],
                    0x0C00..=0x0FFF => self.mmc3_chr_banks[3],
                    0x1000..=0x13FF => self.mmc3_chr_banks[4],
                    0x1400..=0x17FF => self.mmc3_chr_banks[5],
                    0x1800..=0x1BFF => self.mmc3_chr_banks[6],
                    0x1C00..=0x1FFF => self.mmc3_chr_banks[7],
                    _ => 0,
                }
            };
            let offset = bank as usize * 0x0400 + (addr & 0x03FF) as usize;
            if offset < self.chr_memory.len() {
                Some(self.chr_memory[offset])
            } else {
                Some(0)
            }
        } else {
            None
        }
    }

    pub fn mapper_irq(&mut self) -> bool {
        let pending = self.mmc3_irq_pending;
        self.mmc3_irq_pending = false;
        pending
    }

    pub fn clock_scanline(&mut self) {
        if self.mmc3_irq_counter == 0 {
            self.mmc3_irq_counter = self.mmc3_irq_reload;
        } else {
            self.mmc3_irq_counter -= 1;
        }
        if self.mmc3_irq_counter == 0 && self.mmc3_irq_enabled {
            self.mmc3_irq_pending = true;
        }
    }

    // Mapper 066 (GxROM) - 32KB PRG + 8KB CHR switching
    // Bits 4-5 = PRG bank, bits 0-1 = CHR bank
    fn mapper_066_cpu_read(&self, addr: u16) -> Option<u8> {
        if addr >= 0x8000 {
            let offset = self.gxrom_prg_bank as usize * 0x8000 + (addr as usize - 0x8000);
            Some(self.prg_memory[offset % self.prg_memory.len()])
        } else {
            None
        }
    }

    fn mapper_066_cpu_write(&mut self, addr: u16, data: u8) -> bool {
        if addr >= 0x8000 {
            self.gxrom_prg_bank = (data >> 4) & 0x03;
            self.gxrom_chr_bank = data & 0x03;
            true
        } else {
            false
        }
    }

    fn mapper_066_ppu_read(&self, addr: u16) -> Option<u8> {
        if addr <= 0x1FFF {
            let bank = if self.mapper_id == 11 { self.colordreams_chr_bank } else { self.gxrom_chr_bank };
            let offset = bank as usize * 0x2000 + addr as usize;
            Some(self.chr_memory[offset % self.chr_memory.len()])
        } else {
            None
        }
    }

    // Mapper 011 (Color Dreams) - same as GxROM but bits reversed
    // Bits 0-1 = PRG bank, bits 4-5 = CHR bank
    fn mapper_011_cpu_read(&self, addr: u16) -> Option<u8> {
        if addr >= 0x8000 {
            let offset = self.colordreams_prg_bank as usize * 0x8000 + (addr as usize - 0x8000);
            Some(self.prg_memory[offset % self.prg_memory.len()])
        } else {
            None
        }
    }

    fn mapper_011_cpu_write(&mut self, addr: u16, data: u8) -> bool {
        if addr >= 0x8000 {
            self.colordreams_prg_bank = data & 0x03;
            self.colordreams_chr_bank = (data >> 4) & 0x03;
            true
        } else {
            false
        }
    }

    // Mapper 009 (MMC2) - Punch-Out!!
    fn mapper_009_cpu_read(&self, addr: u16) -> Option<u8> {
        match addr {
            0x8000..=0x9FFF => {
                let offset = self.mmc2_prg_bank as usize * 0x2000 + (addr as usize - 0x8000);
                Some(self.prg_memory[offset % self.prg_memory.len()])
            },
            0xA000..=0xBFFF => {
                let bank = (self.prg_banks as usize * 2).wrapping_sub(3);
                let offset = bank * 0x2000 + (addr as usize - 0xA000);
                Some(self.prg_memory[offset % self.prg_memory.len()])
            },
            0xC000..=0xDFFF => {
                let bank = (self.prg_banks as usize * 2).wrapping_sub(2);
                let offset = bank * 0x2000 + (addr as usize - 0xC000);
                Some(self.prg_memory[offset % self.prg_memory.len()])
            },
            0xE000..=0xFFFF => {
                let bank = (self.prg_banks as usize * 2).wrapping_sub(1);
                let offset = bank * 0x2000 + (addr as usize - 0xE000);
                Some(self.prg_memory[offset % self.prg_memory.len()])
            },
            _ => None,
        }
    }

    fn mapper_009_cpu_write(&mut self, addr: u16, data: u8) -> bool {
        match addr {
            0xA000..=0xAFFF => { self.mmc2_prg_bank = data & 0x0F; true },
            0xB000..=0xBFFF => { self.mmc2_chr_banks[0] = data & 0x1F; true },
            0xC000..=0xCFFF => { self.mmc2_chr_banks[1] = data & 0x1F; true },
            0xD000..=0xDFFF => { self.mmc2_chr_banks[2] = data & 0x1F; true },
            0xE000..=0xEFFF => { self.mmc2_chr_banks[3] = data & 0x1F; true },
            0xF000..=0xFFFF => {
                self.mirror = if data & 0x01 != 0 { Mirror::Horizontal } else { Mirror::Vertical };
                true
            },
            _ => false,
        }
    }

    fn mapper_009_ppu_read(&mut self, addr: u16) -> Option<u8> {
        if addr <= 0x1FFF {
            let bank = if addr < 0x1000 {
                // Low 4KB: select based on latch[0]
                if self.mmc2_latch[0] == 0xFD {
                    self.mmc2_chr_banks[0]
                } else {
                    self.mmc2_chr_banks[1]
                }
            } else {
                // High 4KB: select based on latch[1]
                if self.mmc2_latch[1] == 0xFD {
                    self.mmc2_chr_banks[2]
                } else {
                    self.mmc2_chr_banks[3]
                }
            };
            let offset = bank as usize * 0x1000 + (addr & 0x0FFF) as usize;
            let result = if offset < self.chr_memory.len() {
                Some(self.chr_memory[offset])
            } else {
                Some(0)
            };
            // Update latches based on tile fetched
            match addr {
                0x0FD8 => self.mmc2_latch[0] = 0xFD,
                0x0FE8 => self.mmc2_latch[0] = 0xFE,
                0x1FD8..=0x1FDF => self.mmc2_latch[1] = 0xFD,
                0x1FE8..=0x1FEF => self.mmc2_latch[1] = 0xFE,
                _ => {}
            }
            result
        } else {
            None
        }
    }

    // Mapper 034 (BNROM) - 32KB PRG switching, CHR RAM
    fn mapper_034_cpu_read(&self, addr: u16) -> Option<u8> {
        if addr >= 0x8000 {
            let offset = self.bnrom_prg_bank as usize * 0x8000 + (addr as usize - 0x8000);
            Some(self.prg_memory[offset % self.prg_memory.len()])
        } else {
            None
        }
    }

    fn mapper_034_cpu_write(&mut self, addr: u16, data: u8) -> bool {
        if addr >= 0x8000 {
            self.bnrom_prg_bank = data & 0x03;
            true
        } else {
            false
        }
    }

    // Mapper 069 (FME-7/Sunsoft-5B)
    fn mapper_069_cpu_read(&self, addr: u16) -> Option<u8> {
        match addr {
            0x6000..=0x7FFF => {
                // PRG bank 0 at $6000 (can be RAM or ROM)
                let bank = self.fme7_prg_banks[0] as usize & 0x3F;
                let offset = bank * 0x2000 + (addr as usize - 0x6000);
                if offset < self.prg_memory.len() {
                    Some(self.prg_memory[offset])
                } else {
                    Some(0)
                }
            },
            0x8000..=0x9FFF => {
                let bank = self.fme7_prg_banks[1] as usize & 0x3F;
                let offset = bank * 0x2000 + (addr as usize - 0x8000);
                Some(self.prg_memory[offset % self.prg_memory.len()])
            },
            0xA000..=0xBFFF => {
                let bank = self.fme7_prg_banks[2] as usize & 0x3F;
                let offset = bank * 0x2000 + (addr as usize - 0xA000);
                Some(self.prg_memory[offset % self.prg_memory.len()])
            },
            0xC000..=0xDFFF => {
                let bank = self.fme7_prg_banks[3] as usize & 0x3F;
                let offset = bank * 0x2000 + (addr as usize - 0xC000);
                Some(self.prg_memory[offset % self.prg_memory.len()])
            },
            0xE000..=0xFFFF => {
                // Last bank fixed
                let bank = (self.prg_banks as usize * 2).wrapping_sub(1);
                let offset = bank * 0x2000 + (addr as usize - 0xE000);
                Some(self.prg_memory[offset % self.prg_memory.len()])
            },
            _ => None,
        }
    }

    fn mapper_069_cpu_write(&mut self, addr: u16, data: u8) -> bool {
        match addr {
            0x8000..=0x9FFF => {
                self.fme7_command = data & 0x0F;
                true
            },
            0xA000..=0xBFFF => {
                match self.fme7_command {
                    0..=7 => {
                        self.fme7_chr_banks[self.fme7_command as usize] = data;
                    },
                    8 => self.fme7_prg_banks[0] = data,
                    9 => self.fme7_prg_banks[1] = data,
                    0xA => self.fme7_prg_banks[2] = data,
                    0xB => self.fme7_prg_banks[3] = data,
                    0xC => {
                        self.mirror = match data & 0x03 {
                            0 => Mirror::Vertical,
                            1 => Mirror::Horizontal,
                            2 => Mirror::OneScreenLo,
                            3 => Mirror::OneScreenHi,
                            _ => self.mirror,
                        };
                    },
                    _ => {}
                }
                true
            },
            _ => false,
        }
    }

    fn mapper_069_ppu_read(&self, addr: u16) -> Option<u8> {
        if addr <= 0x1FFF {
            let bank_index = (addr / 0x0400) as usize;
            let bank = self.fme7_chr_banks[bank_index] as usize;
            let offset = bank * 0x0400 + (addr & 0x03FF) as usize;
            if offset < self.chr_memory.len() {
                Some(self.chr_memory[offset])
            } else {
                Some(0)
            }
        } else {
            None
        }
    }

    // Mapper 071 (Camerica) - 16KB PRG switch at $8000, last bank fixed at $C000
    fn mapper_071_cpu_read(&self, addr: u16) -> Option<u8> {
        if addr >= 0xC000 {
            let bank = (self.prg_banks as usize).wrapping_sub(1);
            let offset = bank * 0x4000 + (addr as usize - 0xC000);
            Some(self.prg_memory[offset % self.prg_memory.len()])
        } else if addr >= 0x8000 {
            let offset = self.camerica_prg_bank as usize * 0x4000 + (addr as usize - 0x8000);
            Some(self.prg_memory[offset % self.prg_memory.len()])
        } else {
            None
        }
    }

    fn mapper_071_cpu_write(&mut self, addr: u16, data: u8) -> bool {
        match addr {
            0xC000..=0xFFFF => {
                self.camerica_prg_bank = data & 0x0F;
                true
            },
            _ => false,
        }
    }

    // Mapper 206 (DxROM) - simplified MMC3, no IRQ
    fn mapper_206_cpu_read(&self, addr: u16) -> Option<u8> {
        if addr >= 0x8000 {
            let bank = match addr {
                0x8000..=0x9FFF => self.dxrom_prg_banks[0],
                0xA000..=0xBFFF => self.dxrom_prg_banks[1],
                0xC000..=0xDFFF => self.dxrom_prg_banks[2],
                0xE000..=0xFFFF => self.dxrom_prg_banks[3],
                _ => 0,
            };
            let offset = bank as usize * 0x2000 + (addr & 0x1FFF) as usize;
            if offset < self.prg_memory.len() {
                Some(self.prg_memory[offset])
            } else {
                Some(0)
            }
        } else {
            None
        }
    }

    fn mapper_206_cpu_write(&mut self, addr: u16, data: u8) -> bool {
        match addr {
            0x8000..=0x9FFF => {
                if addr % 2 == 0 {
                    self.dxrom_bank_select = data & 0x07;
                } else {
                    let reg = self.dxrom_bank_select & 0x07;
                    match reg {
                        0 | 1 => {
                            self.dxrom_chr_banks[reg as usize * 2] = data & 0xFE;
                            self.dxrom_chr_banks[reg as usize * 2 + 1] = (data & 0xFE) + 1;
                        },
                        2..=5 => {
                            self.dxrom_chr_banks[reg as usize + 2] = data;
                        },
                        6 => {
                            self.dxrom_prg_banks[0] = data;
                        },
                        7 => {
                            self.dxrom_prg_banks[1] = data;
                        },
                        _ => {}
                    }
                    // Fixed banks
                    self.dxrom_prg_banks[2] = (self.prg_banks * 2).wrapping_sub(2);
                    self.dxrom_prg_banks[3] = (self.prg_banks * 2).wrapping_sub(1);
                }
                true
            },
            _ => false,
        }
    }

    fn mapper_206_ppu_read(&self, addr: u16) -> Option<u8> {
        if addr <= 0x1FFF {
            let bank = match addr {
                0x0000..=0x03FF => self.dxrom_chr_banks[0],
                0x0400..=0x07FF => self.dxrom_chr_banks[1],
                0x0800..=0x0BFF => self.dxrom_chr_banks[2],
                0x0C00..=0x0FFF => self.dxrom_chr_banks[3],
                0x1000..=0x13FF => self.dxrom_chr_banks[4],
                0x1400..=0x17FF => self.dxrom_chr_banks[5],
                0x1800..=0x1BFF => self.dxrom_chr_banks[6],
                0x1C00..=0x1FFF => self.dxrom_chr_banks[7],
                _ => 0,
            };
            let offset = bank as usize * 0x0400 + (addr & 0x03FF) as usize;
            if offset < self.chr_memory.len() {
                Some(self.chr_memory[offset])
            } else {
                Some(0)
            }
        } else {
            None
        }
    }
}