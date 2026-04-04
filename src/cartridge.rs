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

    // MMC3 state
    mmc3_bank_select: u8,
    mmc3_prg_banks: [u8; 4],
    mmc3_chr_banks: [u8; 8],
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
        
        let supported = matches!(mapper_id, 0 | 1 | 2 | 3 | 4 | 7 | 11 | 66);
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
            _ => false,
        }
    }

    pub fn ppu_read(&self, addr: u16) -> Option<u8> {
        match self.mapper_id {
            0 => self.mapper_000_ppu_read(addr),
            1 => self.mapper_001_ppu_read(addr),
            2 | 7 => self.mapper_002_ppu_read(addr), // CHR RAM simples
            3 => self.mapper_003_ppu_read(addr),
            11 | 66 => self.mapper_066_ppu_read(addr),
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
            3 | 11 | 66 => false, // CHR ROM, read-only
            _ => false,
        }
    }
    
    pub fn get_mirror(&self) -> Mirror {
        self.mirror
    }

    pub fn get_chr_data(&self) -> &[u8] {
        &self.chr_memory
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
            // PRG RAM (not implemented, return 0)
            Some(0)
        } else {
            None
        }
    }

    fn mapper_001_cpu_write(&mut self, addr: u16, data: u8) -> bool {
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
        }
    }
    
    // MMC3 (Mapper 4) implementation
    fn mapper_004_cpu_read(&self, addr: u16) -> Option<u8> {
        if addr >= 0x8000 && addr <= 0xFFFF {
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
        match addr {
            0x8000..=0x9FFE if addr % 2 == 0 => {
                // Bank select
                self.mmc3_bank_select = data;
                true
            },
            0x8001..=0x9FFF if addr % 2 == 1 => {
                // Bank data
                let bank_register = self.mmc3_bank_select & 0x07;
                match bank_register {
                    0 | 1 => {
                        // CHR banks 
                        self.mmc3_chr_banks[bank_register as usize * 2] = data & 0xFE;
                        self.mmc3_chr_banks[bank_register as usize * 2 + 1] = (data & 0xFE) + 1;
                    },
                    2..=5 => {
                        // CHR banks
                        self.mmc3_chr_banks[bank_register as usize + 2] = data;
                    },
                    6 => {
                        // PRG bank
                        if (self.mmc3_bank_select & 0x40) != 0 {
                            self.mmc3_prg_banks[2] = data;
                        } else {
                            self.mmc3_prg_banks[0] = data;
                        }
                    },
                    7 => {
                        // PRG bank
                        self.mmc3_prg_banks[1] = data;
                    },
                    _ => {}
                }
                true
            },
            _ => false
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
}