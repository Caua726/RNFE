use crate::mappers::{self, CartData, Mapper};
use std::fmt;

/// Erro ao interpretar uma ROM iNES.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RomError {
    /// Os 4 primeiros bytes não são `NES\x1A`.
    BadMagic,
    /// O arquivo termina antes do que o header promete.
    Truncated { expected: usize, got: usize },
    /// Mapper sem implementação.
    UnsupportedMapper(u16),
}

impl fmt::Display for RomError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            RomError::BadMagic => write!(f, "arquivo não é uma ROM iNES (magic inválido)"),
            RomError::Truncated { expected, got } => {
                write!(f, "ROM truncada: header pede {} bytes, arquivo tem {}", expected, got)
            }
            RomError::UnsupportedMapper(id) => write!(f, "mapper {} não suportado", id),
        }
    }
}

impl std::error::Error for RomError {}

pub struct Cartridge {
    pub data: CartData,
    mapper_id: u8,
    mapper: Box<dyn Mapper>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Mirror {
    Horizontal,
    Vertical,
    OneScreenLo,
    OneScreenHi,
}

const SUPPORTED_MAPPERS: &[u8] = &[0, 1, 2, 3, 4, 7, 9, 11, 34, 66, 69, 71, 206, 227];

impl Cartridge {
    /// Interpreta uma ROM iNES a partir dos bytes do arquivo.
    pub fn from_bytes(buffer: &[u8]) -> Result<Self, RomError> {
        if buffer.len() < 16 || &buffer[0..4] != b"NES\x1A" {
            return Err(RomError::BadMagic);
        }

        let prg_banks = buffer[4];
        let chr_banks = buffer[5];
        let mapper1 = buffer[6];
        let mapper2 = buffer[7];

        // Trainer de 512 bytes, se presente
        let mut file_offset = 16;
        if (mapper1 & 0x04) != 0 {
            file_offset += 512;
        }

        let mapper_id = (mapper2 & 0xF0) | (mapper1 >> 4);

        // iNES bit0=1 -> vertical, bit0=0 -> horizontal
        let mirror = if (mapper1 & 0x01) != 0 { Mirror::Vertical } else { Mirror::Horizontal };

        let prg_size = prg_banks as usize * 16384;
        let chr_size = chr_banks as usize * 8192;
        let expected = file_offset + prg_size + chr_size;
        if buffer.len() < expected {
            return Err(RomError::Truncated { expected, got: buffer.len() });
        }

        let prg_memory = buffer[file_offset..file_offset + prg_size].to_vec();
        file_offset += prg_size;

        let chr_memory = if chr_size > 0 {
            buffer[file_offset..file_offset + chr_size].to_vec()
        } else {
            vec![0; 8192] // CHR RAM
        };

        if !SUPPORTED_MAPPERS.contains(&mapper_id) {
            return Err(RomError::UnsupportedMapper(mapper_id as u16));
        }

        log::info!(
            "ROM: PRG {} x 16K, CHR {} x 8K, mapper {}, mirror {:?}",
            prg_banks,
            chr_banks,
            mapper_id,
            mirror
        );

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

    pub fn mapper_id(&self) -> u8 {
        self.mapper_id
    }

    /// Resumo de uma linha (para logs e a tela inicial).
    pub fn describe(&self) -> String {
        format!(
            "PRG {}K, CHR {}{}, mapper {}, {:?}",
            self.data.prg_banks as usize * 16,
            if self.data.chr_banks == 0 { 8 } else { self.data.chr_banks as usize * 8 },
            if self.data.chr_banks == 0 { "K RAM" } else { "K" },
            self.mapper_id,
            self.data.mirror
        )
    }

    /// Leitura pela CPU. Mappers que não tratam `$6000-$7FFF` ganham os 8 KB de PRG RAM
    /// por padrão (muitos ROMs de teste e homebrews contam com WRAM mesmo sem bateria).
    #[inline]
    pub fn cpu_read(&self, addr: u16) -> Option<u8> {
        match self.mapper.cpu_read(addr, &self.data) {
            None if (0x6000..=0x7FFF).contains(&addr) => Some(self.data.prg_ram[(addr & 0x1FFF) as usize]),
            r => r,
        }
    }

    #[inline]
    pub fn cpu_write(&mut self, addr: u16, data: u8) -> bool {
        if self.mapper.cpu_write(addr, data, &mut self.data) {
            return true;
        }
        if (0x6000..=0x7FFF).contains(&addr) {
            self.data.prg_ram[(addr & 0x1FFF) as usize] = data;
            return true;
        }
        false
    }

    /// Leitura de CHR (`$0000-$1FFF` da PPU) pelo mapper.
    #[inline]
    pub fn chr_read(&mut self, addr: u16) -> u8 {
        self.mapper.ppu_read(addr, &self.data).unwrap_or(0)
    }

    /// Escrita em CHR RAM (ignorada em CHR ROM).
    #[inline]
    pub fn chr_write(&mut self, addr: u16, data: u8) {
        if self.data.chr_banks == 0 {
            let idx = (addr & 0x1FFF) as usize;
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

    /// Nível da linha IRQ do mapper.
    #[inline]
    pub fn irq_pending(&self) -> bool {
        self.mapper.irq_pending()
    }

    pub fn reset(&mut self) {
        self.mapper.reset(self.data.prg_banks);
    }

    /// Estado interno do mapper, em texto (diagnóstico).
    pub fn mapper_state(&self) -> String {
        let mut s = format!(
            "  Mapper: {}  PRG banks: {}  CHR banks: {}\n",
            self.mapper_id, self.data.prg_banks, self.data.chr_banks
        );
        s.push_str(&self.mapper.state_string());
        s
    }

    // Debug: ler CHR sem side effects
    pub fn cpu_read_chr_debug(&self, addr: u16) -> Option<u8> {
        if addr <= 0x1FFF {
            if (addr as usize) < self.data.chr.len() { Some(self.data.chr[addr as usize]) } else { Some(0) }
        } else {
            None
        }
    }
}
