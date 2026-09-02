use std::fs::File;
use std::io::Read;
use crate::mappers::{self, CartData, Mapper};

pub struct Cartridge {
    pub data: CartData,
    mapper_id: u8,
    mapper: Box<dyn Mapper>,
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

        // Skip trainer if present
        let mut file_offset = 16;
        if (mapper1 & 0x04) != 0 {
            file_offset += 512;
        }

        let mapper_id = (mapper2 & 0xF0) | (mapper1 >> 4);

        // FIX: iNES bit0=1 -> vertical mirroring, bit0=0 -> horizontal mirroring
        let mirror = if (mapper1 & 0x01) != 0 {
            Mirror::Vertical
        } else {
            Mirror::Horizontal
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
            vec![0; 8192] // CHR RAM
        };

        let supported = matches!(mapper_id, 0 | 1 | 2 | 3 | 4 | 7 | 9 | 11 | 34 | 66 | 69 | 71 | 206 | 227);
        println!("Cartridge loaded: PRG banks: {}, CHR banks: {}, Mapper: {}, Mirror: {:?}",
                 prg_banks, chr_banks, mapper_id, mirror);
        println!("PRG ROM size: {} bytes, CHR ROM size: {} bytes", prg_size, chr_size);
        if !supported {
            eprintln!("WARNING: Mapper {} not supported! Game may not work.", mapper_id);
        }

        let mapper = mappers::create_mapper(mapper_id, prg_banks);

        Ok(Cartridge {
            data: CartData {
                prg: prg_memory,
                chr: chr_memory,
                prg_ram: vec![0; 8192],
                prg_banks,
                chr_banks,
                mirror,
            },
            mapper_id,
            mapper,
        })
    }

    pub fn cpu_read(&self, addr: u16) -> Option<u8> {
        self.mapper.cpu_read(addr, &self.data)
    }

    pub fn cpu_write(&mut self, addr: u16, data: u8) -> bool {
        self.mapper.cpu_write(addr, data, &mut self.data)
    }

    pub fn ppu_read(&mut self, addr: u16) -> Option<u8> {
        self.mapper.ppu_read(addr, &self.data)
    }

    pub fn ppu_write(&mut self, addr: u16, data: u8) -> bool {
        if addr <= 0x1FFF && self.data.chr_banks == 0 {
            self.data.chr[addr as usize] = data;
            true
        } else {
            false
        }
    }

    pub fn ppu_write_chr(&mut self, addr: u16, data: u8) {
        if addr <= 0x1FFF && self.data.chr_banks == 0 {
            let idx = addr as usize;
            if idx < self.data.chr.len() {
                self.data.chr[idx] = data;
            }
        }
    }

    pub fn get_mirror(&self) -> Mirror {
        self.data.mirror
    }

    pub fn get_chr_data(&self) -> &[u8] {
        &self.data.chr
    }

    pub fn clock_scanline(&mut self) {
        self.mapper.clock_scanline();
    }

    pub fn mapper_irq(&mut self) -> bool {
        self.mapper.mapper_irq()
    }

    pub fn reset(&mut self) {
        self.mapper.reset(self.data.prg_banks);
    }

    pub fn print_mapper_state(&self) {
        println!("  Mapper: {}  PRG banks: {}  CHR banks: {}", self.mapper_id, self.data.prg_banks, self.data.chr_banks);
        self.mapper.print_state();
    }

    // Debug: ler CHR sem side effects
    pub fn cpu_read_chr_debug(&self, addr: u16) -> Option<u8> {
        if addr <= 0x1FFF {
            if (addr as usize) < self.data.chr.len() {
                Some(self.data.chr[addr as usize])
            } else {
                Some(0)
            }
        } else {
            None
        }
    }
}
