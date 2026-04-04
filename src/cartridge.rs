use std::fs::File;
use std::io::Read;

pub struct Cartridge {
    prg_memory: Vec<u8>,
    chr_memory: Vec<u8>,
    mapper_id: u8,
    prg_banks: u8,
    chr_banks: u8,
    mirror: Mirror,
    
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
        
        println!("Cartridge loaded: PRG banks: {}, CHR banks: {}, Mapper: {}, Mirror: {:?}", 
                 prg_banks, chr_banks, mapper_id, mirror);
        println!("PRG ROM size: {} bytes, CHR ROM size: {} bytes", prg_size, chr_size);
        
        Ok(Cartridge {
            prg_memory,
            chr_memory,
            mapper_id,
            prg_banks,
            chr_banks,
            mirror,
            
            // Initialize MMC3 state
            mmc3_bank_select: 0,
            mmc3_prg_banks: [0, 1, (prg_banks * 2).wrapping_sub(2), (prg_banks * 2).wrapping_sub(1)],
            mmc3_chr_banks: [0, 1, 2, 3, 4, 5, 6, 7],
        })
    }
    
    pub fn cpu_read(&self, addr: u16) -> Option<u8> {
        match self.mapper_id {
            0 => self.mapper_000_cpu_read(addr),
            4 => self.mapper_004_cpu_read(addr),
            _ => {
                println!("Unsupported mapper: {}", self.mapper_id);
                None
            }
        }
    }
    
    pub fn cpu_write(&mut self, addr: u16, data: u8) -> bool {
        match self.mapper_id {
            0 => self.mapper_000_cpu_write(addr, data),
            4 => self.mapper_004_cpu_write(addr, data),
            _ => false,
        }
    }
    
    pub fn ppu_read(&self, addr: u16) -> Option<u8> {
        match self.mapper_id {
            0 => self.mapper_000_ppu_read(addr),
            _ => None,
        }
    }
    
    pub fn ppu_write(&mut self, addr: u16, data: u8) -> bool {
        match self.mapper_id {
            0 => self.mapper_000_ppu_write(addr, data),
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
    
    pub fn reset(&mut self) {
        // Reset any mapper-specific state
        if self.mapper_id == 4 {
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
}