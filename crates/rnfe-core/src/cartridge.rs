//! Cartucho: header iNES / NES 2.0, ROMs, PRG RAM e o mapper.

use crate::mappers::{CartData, Mapper, MapperKind};
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
    /// Header pede algo que não faz sentido (ROM sem PRG, tamanho absurdo).
    BadHeader(&'static str),
}

impl fmt::Display for RomError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            RomError::BadMagic => write!(f, "arquivo não é uma ROM iNES (magic inválido)"),
            RomError::Truncated { expected, got } => {
                write!(f, "ROM truncada: header pede {} bytes, arquivo tem {}", expected, got)
            }
            RomError::UnsupportedMapper(id) => write!(f, "mapper {} não suportado", id),
            RomError::BadHeader(why) => write!(f, "header inválido: {}", why),
        }
    }
}

impl std::error::Error for RomError {}

#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Mirror {
    Horizontal,
    Vertical,
    OneScreenLo,
    OneScreenHi,
    /// 4 nametables físicas no cartucho (Gauntlet, Rad Racer II).
    FourScreen,
}

/// De onde vem um byte de nametable (`$2000-$2FFF`) — o mapper decide.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum NtSource {
    /// Página da VRAM interna da PPU (0-3).
    Ciram(u8),
    /// Offset físico em CHR (ROM ou RAM).
    Chr(usize),
    /// Valor pronto (tile de preenchimento, ExRAM…); escritas são ignoradas.
    Value(u8),
}

/// Página da VRAM (0-3) e offset para um endereço `$2000-$3EFF` com o mirroring dado.
#[inline]
pub fn mirror_nametable(addr: u16, mirror: Mirror) -> (usize, usize) {
    let addr = addr & 0x0FFF;
    let table = (addr >> 10) as usize;
    let offset = (addr & 0x03FF) as usize;
    let nt = match mirror {
        Mirror::Vertical => table & 1,
        Mirror::Horizontal => table >> 1,
        Mirror::OneScreenLo => 0,
        Mirror::OneScreenHi => 1,
        Mirror::FourScreen => table,
    };
    (nt, offset)
}

/// Limite de bom senso para cada ROM (o maior cartucho licenciado tem 1 MB; 64 MB cobre
/// qualquer multicart e o tamanho exponencial do NES 2.0 sem estourar o `usize` do wasm).
const MAX_ROM: usize = 64 << 20;
/// Quanto pode faltar no fim de um dump antes de recusar a ROM: um punhado de bytes (bit de
/// trainer setado por engano, corte no fim do arquivo) é comum nos packs; falta grande é ROM
/// errada.
const MAX_MISSING: usize = 64 << 10;

/// O que o header diz, já decodificado (iNES 1 ou NES 2.0).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RomHeader {
    pub nes2: bool,
    pub mapper: u16,
    pub submapper: u8,
    pub prg_len: usize,
    pub chr_len: usize,
    pub prg_ram_len: usize,
    pub chr_ram_len: usize,
    pub battery: bool,
    pub four_screen: bool,
    /// Região declarada no header (byte 9 do iNES 1, byte 12 do NES 2.0).
    pub region: crate::Region,
    pub trainer: bool,
    pub mirror: Mirror,
}

/// Headers que mentem: `(hash FNV-1a do arquivo inteiro, mapper certo, motivo)`.
/// Vale para dumps conhecidos em que o campo do header aponta uma placa impossível — sem isso o
/// jogo não abre ou abre em preto.
const HEADER_FIX: &[(u64, u16, &str)] = &[
    (0x4700_477f_dca1_009f, 34, "Mermaids of Atlantis (U): header diz FFE F3, a placa é NINA-001"),
    (0x031e_a7db_0647_ade7, 1, "Adventures in the Magic Kingdom (U) [a1]: escreve protocolo de MMC1"),
    (0xdf1a_ba35_afd2_a1f1, 71, "Fire Hawk (U) [a1]: header diz 15, a placa é Camerica"),
];

impl RomHeader {
    /// Decodifica os 16 bytes do header.
    pub fn parse(h: &[u8]) -> Result<RomHeader, RomError> {
        if h.len() < 16 || &h[0..4] != b"NES\x1A" {
            return Err(RomError::BadMagic);
        }
        // Headers de dumps antigos têm lixo no byte 7 e passam por NES 2.0, o que produz
        // tamanhos absurdos (dezenas de MB) e faz a ROM ser recusada. Só aceitamos NES 2.0
        // quando o tamanho declarado cabe no arquivo.
        let mut nes2 = h[7] & 0x0C == 0x08;
        if nes2 && h.len() > 16 {
            let prg = nes2_rom_size(h[4], h[9] & 0x0F, 16384);
            let chr = nes2_rom_size(h[5], h[9] >> 4, 8192);
            match (prg, chr) {
                (Ok(p), Ok(c)) if 16 + p + c <= h.len() + (4 << 20) => {}
                _ => nes2 = false,
            }
        }
        let battery = h[6] & 0x02 != 0;
        let trainer = h[6] & 0x04 != 0;
        let four_screen = h[6] & 0x08 != 0;
        let mirror = if four_screen {
            Mirror::FourScreen
        } else if h[6] & 0x01 != 0 {
            Mirror::Vertical
        } else {
            Mirror::Horizontal
        };
        let mut mapper = ((h[7] & 0xF0) | (h[6] >> 4)) as u16;
        let mut submapper = 0;
        let (prg_len, chr_len, mut prg_ram_len, chr_ram_len);
        if nes2 {
            mapper |= ((h[8] & 0x0F) as u16) << 8;
            submapper = h[8] >> 4;
            prg_len = nes2_rom_size(h[4], h[9] & 0x0F, 16384)?;
            chr_len = nes2_rom_size(h[5], h[9] >> 4, 8192)?;
            // byte 10/11: nibble baixo = RAM volátil, alto = não volátil; 0 = ausente, senão 64 << n
            let shift = |n: u8| if n == 0 { 0 } else { 64usize << n };
            prg_ram_len = shift(h[10] & 0x0F).max(shift(h[10] >> 4));
            chr_ram_len = shift(h[11] & 0x0F).max(shift(h[11] >> 4));
        } else {
            // iNES 1: se os bytes 12-15 têm lixo, o "mapper alto" do byte 7 não é confiável
            if h[12..16].iter().any(|&b| b != 0) {
                mapper &= 0x0F;
            }
            prg_len = h[4] as usize * 16384;
            chr_len = h[5] as usize * 8192;
            prg_ram_len = if h[8] == 0 { 8192 } else { h[8] as usize * 8192 };
            // UNROM 512 (30) tem 32 KB de CHR RAM; o resto, 8 KB
            chr_ram_len = if chr_len == 0 {
                match mapper {
                    30 => 32768,         // UNROM 512
                    28 => 32768,         // Action 53
                    13 => 16384,         // CPROM: 4 páginas de 4 KB
                    6 | 8 | 17 => 32768, // copiadores FFE: CHR RAM em bancos
                    _ => 8192,
                }
            } else {
                0
            };
            // O byte 8 do iNES 1 quase nunca é preenchido: dimensiona a PRG RAM pela placa
            // (MMC1 SOROM/SXROM chega a 32 KB, MMC5 a 64 KB; sobra não atrapalha).
            if h[8] == 0 {
                prg_ram_len = match mapper {
                    5 => 64 * 1024,
                    1 => 32 * 1024,
                    _ => prg_ram_len,
                };
            }
        }
        // Região: NES 2.0 põe no byte 12 (bits 0-1); o iNES 1 tem o bit 0 do byte 9, que quase
        // ninguém preenche — o nome do arquivo é mais confiável, então isto é só o padrão.
        let region = if nes2 {
            match h[12] & 0x03 {
                1 => crate::Region::Pal,
                _ => crate::Region::Ntsc,
            }
        } else if h[9] & 0x01 != 0 {
            crate::Region::Pal
        } else {
            crate::Region::Ntsc
        };
        if prg_len == 0 {
            return Err(RomError::BadHeader("ROM sem PRG"));
        }
        if prg_len > MAX_ROM || chr_len > MAX_ROM {
            return Err(RomError::BadHeader("ROM maior que 64 MB"));
        }
        Ok(RomHeader {
            nes2,
            mapper,
            submapper,
            prg_len,
            chr_len,
            prg_ram_len,
            chr_ram_len,
            battery,
            four_screen,
            region,
            trainer,
            mirror,
        })
    }
}

/// Tamanho NES 2.0: 12 bits normais, ou forma exponencial `2^E × (MM×2+1)` se o nibble alto é $F.
fn nes2_rom_size(lo: u8, hi: u8, unit: usize) -> Result<usize, RomError> {
    if hi == 0x0F {
        let exp = (lo >> 2) as u32;
        let mult = (lo & 0x03) as usize * 2 + 1;
        if exp > 26 {
            // acima de 64 MB nunca cabe (e 1 << exp estouraria o usize de 32 bits no wasm)
            return Err(RomError::BadHeader("tamanho exponencial absurdo"));
        }
        Ok((1usize << exp) * mult)
    } else {
        Ok((((hi as usize) << 8) | lo as usize) * unit)
    }
}

pub struct Cartridge {
    pub data: CartData,
    mapper: MapperKind,
    rom_hash: u64,
    wants_cpu_clock: bool,
    has_audio: bool,
    /// PRG RAM padrão em `$6000-$7FFF` quando o mapper não trata o endereço.
    prg_ram_fallback: bool,
    /// O mapper tem efeito colateral em leituras da CPU (MMC5 `$5204`, N163 `$4800`).
    read_hook: bool,
    /// Base física de cada banco de 1 KB de CHR (recalculada após cada escrita da CPU).
    chr_cache: [usize; 8],
    region: crate::Region,
    /// Base na PRG de cada janela de 8 KB de `$8000-$FFFF` (caminho rápido de leitura).
    prg_cache: [usize; 4],
    /// O mapper sabe dizer o banco sem efeito colateral e não tem gancho de leitura.
    prg_cached: bool,
    /// O mapper comuta bancos escrevendo em `$6000-$7FFF` (NINA-001).
    regs_in_prg_ram: bool,
    chr_dynamic: bool,
    nt_cache: [NtSource; 4],
    nt_dynamic: bool,
    /// Nível da linha IRQ do mapper, atualizado a cada chamada que pode mudá-lo.
    irq: bool,
}

impl Cartridge {
    /// Interpreta uma ROM iNES / NES 2.0 a partir dos bytes do arquivo.
    pub fn from_bytes(buffer: &[u8]) -> Result<Self, RomError> {
        let mut hdr = RomHeader::parse(buffer)?;
        // Correções de header conhecidas (por hash do arquivo) e o caso genérico do "mapper 0"
        // com tamanhos que a placa NROM não endereça — comum em dumps antigos.
        let file_hash = fnv1a(buffer, FNV_OFFSET);
        if let Some((_, mapper, why)) = HEADER_FIX.iter().find(|(h, ..)| *h == file_hash) {
            log::info!("header corrigido: {why}");
            hdr.mapper = *mapper;
            hdr.submapper = 0;
        } else if hdr.mapper == 0 && !hdr.nes2 {
            if hdr.prg_len > 32 * 1024 {
                log::info!("mapper 0 com {} KB de PRG: tratando como UxROM", hdr.prg_len / 1024);
                hdr.mapper = 2;
            } else if hdr.chr_len > 8 * 1024 {
                log::info!("mapper 0 com {} KB de CHR: tratando como CNROM", hdr.chr_len / 1024);
                hdr.mapper = 3;
            }
        }
        if MapperKind::create(hdr.mapper, &CartData::probe(&hdr)).is_none() {
            return Err(RomError::UnsupportedMapper(hdr.mapper));
        }
        let mut offset = 16 + if hdr.trainer { 512 } else { 0 };
        let expected = offset + hdr.prg_len + hdr.chr_len;
        let missing = expected.saturating_sub(buffer.len());
        // até 64 KB (ou 25 % do total) de cauda faltando: a maioria dos dumps ruins do pack só
        // perdeu o fim da CHR e roda com alguns tiles corrompidos, como nos outros emuladores
        if missing > MAX_MISSING || missing * 4 > expected {
            return Err(RomError::Truncated { expected, got: buffer.len() });
        }
        // Dumps a que faltam alguns bytes (ou com o bit de trainer setado por engano) são comuns
        // nos packs: completa com $FF em vez de recusar a ROM inteira.
        let mut rest = vec![0xFFu8; missing];
        if !rest.is_empty() {
            log::warn!("ROM truncada em {} bytes: completando com $FF", rest.len());
        }
        let mut full = buffer.to_vec();
        full.append(&mut rest);
        let buffer: &[u8] = &full;
        // Trainer: 512 bytes que o copiador punha em $7000-$71FF da PRG RAM
        let trainer = if hdr.trainer { Some(buffer[16..528].to_vec()) } else { None };
        let prg = buffer[offset..offset + hdr.prg_len].to_vec();
        offset += hdr.prg_len;
        let (chr, chr_is_ram) = if hdr.chr_len > 0 {
            (buffer[offset..offset + hdr.chr_len].to_vec(), false)
        } else {
            (vec![0u8; hdr.chr_ram_len.clamp(8192, 512 * 1024)], true)
        };
        let rom_hash = fnv1a(&prg, fnv1a(if chr_is_ram { &[] } else { &chr }, FNV_OFFSET));

        log::info!(
            "ROM: {} PRG {} KB, CHR {} KB{}, mapper {}.{}, {:?}{}",
            if hdr.nes2 { "NES 2.0" } else { "iNES" },
            hdr.prg_len / 1024,
            chr.len() / 1024,
            if chr_is_ram { " (RAM)" } else { "" },
            hdr.mapper,
            hdr.submapper,
            hdr.mirror,
            if hdr.battery { ", bateria" } else { "" }
        );

        let mut data = CartData::new(prg, chr, chr_is_ram, &hdr);
        if let Some(t) = trainer {
            let end = (0x1000 + t.len()).min(data.prg_ram.len());
            if end > 0x1000 {
                data.prg_ram[0x1000..end].copy_from_slice(&t[..end - 0x1000]);
            }
        }
        let mut mapper =
            MapperKind::create(hdr.mapper, &data).ok_or(RomError::UnsupportedMapper(hdr.mapper))?;
        mapper.reset(&mut data);
        let wants_cpu_clock = mapper.wants_cpu_clock();
        let has_audio = mapper.has_audio();
        let prg_ram_fallback = !mapper.manages_prg_ram();
        let read_hook = mapper.has_read_hook();
        let regs_in_prg_ram = mapper.regs_in_prg_ram();
        let mut cart = Cartridge {
            read_hook,
            chr_dynamic: mapper.chr_dynamic(),
            nt_dynamic: mapper.nt_dynamic(),
            data,
            mapper,
            rom_hash,
            wants_cpu_clock,
            has_audio,
            prg_ram_fallback,
            chr_cache: [0; 8],
            region: hdr.region,
            prg_cache: [0; 4],
            prg_cached: false,
            regs_in_prg_ram,
            nt_cache: [NtSource::Ciram(0); 4],
            irq: false,
        };
        cart.refresh_caches();
        Ok(cart)
    }

    pub fn mapper_id(&self) -> u16 {
        self.data.mapper
    }

    pub fn submapper(&self) -> u8 {
        self.data.submapper
    }

    /// FNV-1a de PRG+CHR ROM: identifica a ROM para saves e save states.
    pub fn rom_hash(&self) -> u64 {
        self.rom_hash
    }

    pub fn has_battery(&self) -> bool {
        self.data.battery
    }

    pub fn prg_ram(&self) -> &[u8] {
        &self.data.prg_ram
    }

    pub fn prg_ram_mut(&mut self) -> &mut [u8] {
        &mut self.data.prg_ram
    }

    /// `true` se houve escrita na PRG RAM desde a última chamada.
    pub fn take_prg_ram_dirty(&mut self) -> bool {
        std::mem::take(&mut self.data.prg_ram_dirty)
    }

    /// Resumo de uma linha (para logs e a tela inicial).
    pub fn describe(&self) -> String {
        format!(
            "PRG {}K, CHR {}K{}, mapper {} ({}), {:?}{}",
            self.data.prg_banks as usize * 16,
            self.data.chr.len() / 1024,
            if self.data.chr_is_ram { " RAM" } else { "" },
            self.data.mapper,
            self.mapper.name(),
            self.get_mirror(),
            if self.data.battery { ", bateria" } else { "" }
        )
    }

    /// Leitura pela CPU. Mappers que não tratam `$6000-$7FFF` ganham a PRG RAM por padrão
    /// (muitos ROMs de teste e homebrews contam com WRAM mesmo sem bateria).
    #[inline]
    pub fn cpu_read(&self, addr: u16) -> Option<u8> {
        match self.mapper.cpu_read(addr, &self.data) {
            None if self.prg_ram_fallback && (0x6000..=0x7FFF).contains(&addr) => {
                Some(self.data.prg_ram_at((addr & 0x1FFF) as usize))
            }
            r => r,
        }
    }

    /// Recalcula os caches de CHR e nametable (o mapeamento só muda por escrita da CPU,
    /// exceto nos mappers `*_dynamic`, que são consultados a cada acesso).
    fn refresh_caches(&mut self) {
        // Tabela de bancos de PRG: evita o despacho do mapper em toda busca de instrução.
        // Os ganchos de leitura (MMC5 $5204, N163 $4800) só olham endereços abaixo de $8000,
        // então o caminho rápido pode ignorá-los.
        self.prg_cached = true;
        {
            for (i, base) in self.prg_cache.iter_mut().enumerate() {
                match self.mapper.prg_offset(0x8000 + (i as u16) * 0x2000, &self.data) {
                    Some(off) => *base = off,
                    None => {
                        self.prg_cached = false;
                        break;
                    }
                }
            }
        }
        if !self.chr_dynamic {
            for (i, base) in self.chr_cache.iter_mut().enumerate() {
                *base = self.mapper.chr_offset((i * 0x400) as u16);
            }
        }
        if !self.nt_dynamic {
            let mirror = self.get_mirror();
            for q in 0..4 {
                let addr = 0x2000 + (q as u16) * 0x400;
                self.nt_cache[q] = match self.mapper.nt_source(addr, &self.data) {
                    None => NtSource::Ciram(mirror_nametable(addr, mirror).0 as u8),
                    Some(NtSource::Ciram(p)) => NtSource::Ciram(p & 3),
                    Some(NtSource::Chr(o)) => NtSource::Chr(o & !0x3FF),
                    Some(v) => v,
                };
            }
        }
    }

    /// Leitura pela CPU com efeitos colaterais (o bus usa esta; o debugger usa `cpu_read`).
    #[inline]
    pub fn cpu_read_mut(&mut self, addr: u16) -> Option<u8> {
        if self.prg_cached && addr >= 0x8000 {
            let base = self.prg_cache[((addr >> 13) & 3) as usize];
            let v = self.data.prg_at(base + (addr & 0x1FFF) as usize);
            debug_assert_eq!(Some(v), self.cpu_read(addr), "cache de PRG divergiu em ${addr:04X}");
            return Some(v);
        }
        let v = self.cpu_read(addr);
        if self.read_hook {
            self.mapper.on_cpu_read(addr);
            self.irq = self.mapper.irq_pending();
        }
        v
    }

    #[inline]
    pub fn cpu_write(&mut self, addr: u16, data: u8) -> bool {
        if self.mapper.cpu_write(addr, data, &mut self.data) {
            if !(0x6000..0x8000).contains(&addr) || self.regs_in_prg_ram {
                self.refresh_caches();
                self.irq = self.mapper.irq_pending();
            }
            return true;
        }
        if self.prg_ram_fallback && (0x6000..=0x7FFF).contains(&addr) {
            self.data.prg_ram_set((addr & 0x1FFF) as usize, data);
            return true;
        }
        false
    }

    /// Leitura de nametable (`$2000-$3EFF` da PPU): o mapper pode redirecionar para CHR,
    /// para uma página específica da VRAM ou devolver um valor próprio.
    #[inline]
    pub fn nt_read(&mut self, addr: u16, ciram: &[[u8; 1024]; 4]) -> u8 {
        if !self.nt_dynamic {
            let off = (addr & 0x03FF) as usize;
            return match self.nt_cache[(addr >> 10) as usize & 3] {
                NtSource::Ciram(p) => ciram[p as usize][off],
                NtSource::Chr(base) => self.data.chr_at(base + off),
                NtSource::Value(v) => v,
            };
        }
        self.nt_read_dynamic(addr, ciram)
    }

    #[inline(never)]
    fn nt_read_dynamic(&mut self, addr: u16, ciram: &[[u8; 1024]; 4]) -> u8 {
        let src = self.mapper.nt_source(addr, &self.data);
        self.irq = self.mapper.irq_pending(); // MMC5 detecta scanlines (e dispara IRQ) aqui
        match src {
            None => {
                let (nt, off) = mirror_nametable(addr, self.get_mirror());
                ciram[nt][off]
            }
            Some(NtSource::Ciram(p)) => ciram[(p & 3) as usize][(addr & 0x03FF) as usize],
            Some(NtSource::Chr(o)) => self.data.chr_at(o),
            Some(NtSource::Value(v)) => v,
        }
    }

    #[inline]
    pub fn nt_write(&mut self, addr: u16, val: u8, ciram: &mut [[u8; 1024]; 4]) {
        if !self.nt_dynamic {
            let off = (addr & 0x03FF) as usize;
            match self.nt_cache[(addr >> 10) as usize & 3] {
                NtSource::Ciram(p) => ciram[p as usize][off] = val,
                NtSource::Chr(base) => self.data.chr_set(base + off, val),
                NtSource::Value(_) => {}
            }
            return;
        }
        if self.mapper.nt_write(addr, val, &mut self.data) {
            return;
        }
        match self.mapper.nt_dest(addr, &self.data) {
            None => {
                let (nt, off) = mirror_nametable(addr, self.get_mirror());
                ciram[nt][off] = val;
            }
            Some(NtSource::Ciram(p)) => ciram[(p & 3) as usize][(addr & 0x03FF) as usize] = val,
            Some(NtSource::Chr(o)) => self.data.chr_set(o, val),
            Some(NtSource::Value(_)) => {}
        }
    }

    /// Leitura de CHR (`$0000-$1FFF` da PPU) pelo mapper.
    #[inline]
    pub fn chr_read(&mut self, addr: u16) -> u8 {
        if self.chr_dynamic {
            self.chr_read_dynamic(addr)
        } else {
            self.data.chr_at(self.chr_cache[(addr >> 10) as usize & 7] + (addr & 0x3FF) as usize)
        }
    }

    /// Fora de linha de propósito: aqui o despacho dos 28 mappers é inlinado, e deixá-lo no
    /// corpo de `chr_read` (37 mil chamadas por frame) inchava a função e derrubava o I-cache.
    #[inline(never)]
    fn chr_read_dynamic(&mut self, addr: u16) -> u8 {
        self.mapper.ppu_read(addr, &self.data)
    }

    /// Escrita em CHR (só tem efeito em CHR RAM).
    #[inline]
    pub fn chr_write(&mut self, addr: u16, data: u8) {
        if self.chr_dynamic {
            self.chr_write_dynamic(addr, data);
        } else {
            let off = self.chr_cache[(addr >> 10) as usize & 7] + (addr & 0x3FF) as usize;
            self.data.chr_set(off, data);
        }
    }

    #[inline]
    pub fn get_mirror(&self) -> Mirror {
        if self.data.four_screen { Mirror::FourScreen } else { self.data.mirror }
    }

    #[inline(never)]
    fn chr_write_dynamic(&mut self, addr: u16, data: u8) {
        self.mapper.ppu_write(addr, data, &mut self.data);
    }

    /// Borda de subida de A12 na PPU (contador de scanline do MMC3). Fora de linha: aqui mora o
    /// despacho dos mappers, e inlinar isso em `Bus::tick_post` (todo ciclo) só incha o laço.
    #[inline(never)]
    pub fn a12_rise(&mut self) {
        self.mapper.a12_rise();
        self.irq = self.mapper.irq_pending();
    }

    #[inline]
    pub fn wants_cpu_clock(&self) -> bool {
        self.wants_cpu_clock
    }

    #[inline(never)]
    pub fn cpu_clock(&mut self) {
        self.mapper.cpu_clock();
        self.irq = self.mapper.irq_pending();
    }

    /// Nível da linha IRQ do mapper (cache atualizado em `cpu_clock`, `a12_rise`,
    /// `cpu_write`, `cpu_read_mut`, `reset` e `restore`).
    #[inline]
    pub fn irq_pending(&self) -> bool {
        self.irq
    }

    /// Região declarada no header da ROM.
    pub fn region(&self) -> crate::Region {
        self.region
    }

    /// O cartucho tem canal de áudio próprio.
    pub fn has_audio(&self) -> bool {
        self.has_audio
    }

    /// Áudio de expansão do cartucho, em [-1, 1].
    #[inline]
    pub fn audio_output(&self) -> f32 {
        self.mapper.audio_output()
    }

    pub fn reset(&mut self) {
        self.mapper.reset(&mut self.data);
        self.refresh_caches();
        self.irq = self.mapper.irq_pending();
    }

    /// Estado interno do mapper, em texto (diagnóstico).
    pub fn mapper_state(&self) -> String {
        let mut s = format!(
            "  Mapper: {} ({})  PRG banks: {}  CHR banks: {}\n",
            self.data.mapper,
            self.mapper.name(),
            self.data.prg_banks,
            self.data.chr_banks
        );
        s.push_str(&self.mapper.state_string());
        s
    }

    /// Estado sem a ROM (feature `serde`).
    #[cfg(feature = "serde")]
    pub fn state(&self) -> crate::state::CartState {
        crate::state::CartState {
            prg_ram: self.data.prg_ram.clone(),
            chr_ram: self.data.chr_is_ram.then(|| self.data.chr.clone()),
            chr_ram_extra: self.data.chr_ram.clone(),
            mirror: self.data.mirror,
            mapper: self.mapper.clone(),
        }
    }

    #[cfg(feature = "serde")]
    pub fn restore(&mut self, st: crate::state::CartState) -> Result<(), crate::state::StateError> {
        use crate::state::StateError;
        if st.prg_ram.len() != self.data.prg_ram.len() {
            return Err(StateError::Corrupt(format!(
                "PRG RAM de {} bytes, cartucho tem {}",
                st.prg_ram.len(),
                self.data.prg_ram.len()
            )));
        }
        match (&st.chr_ram, self.data.chr_is_ram) {
            (Some(c), true) if c.len() == self.data.chr.len() => {}
            (None, false) => {}
            _ => return Err(StateError::Corrupt("CHR RAM não bate com o cartucho".into())),
        }
        if core::mem::discriminant(&st.mapper) != core::mem::discriminant(&self.mapper) {
            return Err(StateError::Corrupt("mapper diferente".into()));
        }
        self.data.prg_ram = st.prg_ram;
        if let Some(c) = st.chr_ram {
            self.data.chr = c;
        }
        if st.chr_ram_extra.len() == self.data.chr_ram.len() {
            self.data.chr_ram = st.chr_ram_extra;
        }
        self.data.mirror = st.mirror;
        self.data.prg_ram_dirty = true;
        self.mapper = st.mapper;
        self.refresh_caches();
        self.irq = self.mapper.irq_pending();
        Ok(())
    }

    /// CHR pelo mapeamento atual, sem efeitos colaterais (debug).
    pub fn cpu_read_chr_debug(&self, addr: u16) -> Option<u8> {
        if addr <= 0x1FFF { Some(self.data.chr_at(self.mapper.chr_offset(addr))) } else { None }
    }
}

const FNV_OFFSET: u64 = 0xcbf29ce484222325;

fn fnv1a(bytes: &[u8], mut h: u64) -> u64 {
    for &b in bytes {
        h ^= b as u64;
        h = h.wrapping_mul(0x100000001b3);
    }
    h
}
