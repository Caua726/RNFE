//! Mappers: o hardware dentro do cartucho que decide o que a CPU e a PPU enxergam.
//!
//! Cada mapper implementa [`Mapper`] sobre um [`CartData`] (ROM/RAM + metadados do header).
//! O despacho é por `match` em [`MapperKind`] (sem `dyn`, serializável em F3-06).
//!
//! Convenções:
//! - `cpu_read`/`cpu_write` devolvem `None`/`false` quando não tratam o endereço; o cartucho
//!   então oferece a PRG RAM padrão em `$6000-$7FFF` e o bus devolve open bus.
//! - CHR passa por `chr_offset` (offset físico) — a leitura/escrita real, com bounds e a
//!   distinção ROM/RAM, é do [`CartData`]. Só mappers com efeito colateral (MMC2) sobrescrevem
//!   `ppu_read`.
//! - IRQ é **nível**: `irq_pending()` fica alto até o jogo reconhecer no registrador do mapper.
//! - `a12_rise` é chamado pela PPU na borda de subida de A12 (filtrada) — o contador do MMC3.
//! - `cpu_clock` roda a cada ciclo de CPU, mas só se `wants_cpu_clock()` (FME-7, VRC, N163…).

pub mod axrom;
pub mod bnrom;
pub mod camerica;
pub mod cnrom;
pub mod colordreams;
pub mod dxrom;
pub mod fme7;
pub mod gxrom;
pub mod mapper227;
pub mod mmc1;
pub mod mmc2;
pub mod mmc3;
pub mod nrom;
pub mod uxrom;
pub mod vrc6;

use crate::cartridge::{Mirror, RomHeader};

/// Conteúdo do cartucho: ROMs, RAMs e o que o header diz sobre elas.
///
/// PRG e CHR são preenchidos até a próxima potência de 2 (repetindo o conteúdo), então todo
/// acesso é `offset & mask` — sem divisão e sem bounds check no caminho quente.
pub struct CartData {
    pub prg: Vec<u8>,
    pub chr: Vec<u8>,
    pub prg_ram: Vec<u8>,
    prg_mask: usize,
    chr_mask: usize,
    prg_ram_mask: usize,
    /// Tamanho real (antes do padding), em bancos de 16 KB / 8 KB.
    pub prg_banks: u16,
    pub chr_banks: u16,
    pub chr_is_ram: bool,
    /// Mirroring atual (o mapper muda; ignorado se `four_screen`).
    pub mirror: Mirror,
    pub four_screen: bool,
    pub battery: bool,
    pub mapper: u16,
    pub submapper: u8,
    /// Alguém escreveu na PRG RAM desde o último `take_prg_ram_dirty`.
    pub prg_ram_dirty: bool,
    /// Ciclo de CPU da escrita em curso (o bus atualiza antes de `cpu_write`).
    pub cpu_cycle: u64,
}

impl CartData {
    pub(crate) fn new(prg: Vec<u8>, chr: Vec<u8>, chr_is_ram: bool, hdr: &RomHeader) -> CartData {
        let prg_ram_len = hdr.prg_ram_len.min(64 * 1024);
        let prg_banks = (prg.len() / 16384) as u16;
        let chr_banks = if chr_is_ram { 0 } else { (chr.len() / 8192) as u16 };
        let prg = pad_pow2(prg);
        let chr = pad_pow2(chr);
        let prg_ram = vec![0u8; prg_ram_len.next_power_of_two().max(8192)];
        CartData {
            prg_mask: prg.len() - 1,
            chr_mask: chr.len() - 1,
            prg_ram_mask: prg_ram.len() - 1,
            prg,
            chr,
            prg_ram,
            prg_banks,
            chr_banks,
            chr_is_ram,
            mirror: hdr.mirror,
            four_screen: hdr.four_screen,
            battery: hdr.battery,
            mapper: hdr.mapper,
            submapper: hdr.submapper,
            prg_ram_dirty: false,
            cpu_cycle: 0,
        }
    }

    /// Byte de PRG ROM no offset físico (espelha se passar do fim).
    #[inline(always)]
    pub fn prg_at(&self, offset: usize) -> u8 {
        self.prg[offset & self.prg_mask]
    }

    /// Byte de CHR (ROM ou RAM) no offset físico.
    #[inline(always)]
    pub fn chr_at(&self, offset: usize) -> u8 {
        self.chr[offset & self.chr_mask]
    }

    /// Escrita em CHR: só tem efeito em CHR RAM.
    #[inline(always)]
    pub fn chr_set(&mut self, offset: usize, value: u8) {
        if self.chr_is_ram {
            self.chr[offset & self.chr_mask] = value;
        }
    }

    #[inline(always)]
    pub fn prg_ram_at(&self, offset: usize) -> u8 {
        self.prg_ram[offset & self.prg_ram_mask]
    }

    #[inline(always)]
    pub fn prg_ram_set(&mut self, offset: usize, value: u8) {
        self.prg_ram[offset & self.prg_ram_mask] = value;
        self.prg_ram_dirty = true;
    }

    /// Número de bancos de 16 KB de PRG (≥ 1).
    #[inline]
    pub fn prg_16k(&self) -> usize {
        (self.prg.len() / 16384).max(1)
    }

    /// Número de bancos de 8 KB de PRG (≥ 1).
    #[inline]
    pub fn prg_8k(&self) -> usize {
        (self.prg.len() / 8192).max(1)
    }
}

/// Preenche até a próxima potência de 2 repetindo o conteúdo (mirroring de ROM pequena).
fn pad_pow2(mut v: Vec<u8>) -> Vec<u8> {
    if v.is_empty() {
        return vec![0; 8192];
    }
    let target = v.len().next_power_of_two();
    while v.len() < target {
        let take = (target - v.len()).min(v.len());
        v.extend_from_within(..take);
    }
    v
}

pub trait Mapper {
    /// Leitura pela CPU (`$4020-$FFFF`). `None` = não mapeado (PRG RAM padrão ou open bus).
    fn cpu_read(&self, addr: u16, data: &CartData) -> Option<u8>;
    /// Escrita pela CPU. `false` = não tratada (cai na PRG RAM padrão em `$6000-$7FFF`).
    fn cpu_write(&mut self, addr: u16, val: u8, data: &mut CartData) -> bool;
    /// Offset físico em CHR para um endereço `$0000-$1FFF` da PPU.
    fn chr_offset(&self, addr: u16) -> usize {
        addr as usize
    }
    /// Leitura de CHR pela PPU (sobrescrever só quando a leitura tem efeito colateral).
    #[inline]
    fn ppu_read(&mut self, addr: u16, data: &CartData) -> u8 {
        data.chr_at(self.chr_offset(addr))
    }
    #[inline]
    fn ppu_write(&mut self, addr: u16, val: u8, data: &mut CartData) {
        data.chr_set(self.chr_offset(addr), val);
    }
    /// Borda de subida de A12 no barramento da PPU (já filtrada).
    fn a12_rise(&mut self) {}
    /// O mapper precisa de `cpu_clock` a cada ciclo de CPU?
    fn wants_cpu_clock(&self) -> bool {
        false
    }
    fn cpu_clock(&mut self) {}
    /// Nível da linha IRQ (fica alto até o jogo reconhecer no mapper).
    fn irq_pending(&self) -> bool {
        false
    }
    /// O mapper cuida de `$6000-$7FFF` sozinho (habilita/protege a PRG RAM): o cartucho não
    /// oferece a PRG RAM padrão quando `cpu_read` devolve `None`.
    fn manages_prg_ram(&self) -> bool {
        false
    }
    /// Saída de áudio de expansão (somada ao mix da APU), em [-1, 1].
    fn audio_output(&self) -> f32 {
        0.0
    }
    fn reset(&mut self, data: &mut CartData);
    fn state_string(&self) -> String {
        String::new()
    }
}

/// Todos os mappers suportados. `id` do iNES/NES 2.0 → variante.
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[derive(Clone)]
pub enum MapperKind {
    Nrom(nrom::Nrom),
    Mmc1(mmc1::Mmc1),
    Uxrom(uxrom::Uxrom),
    Cnrom(cnrom::Cnrom),
    Mmc3(mmc3::Mmc3),
    Axrom(axrom::Axrom),
    Mmc2(mmc2::Mmc2),
    ColorDreams(colordreams::ColorDreams),
    Bnrom(bnrom::Bnrom),
    Gxrom(gxrom::Gxrom),
    Fme7(fme7::Fme7),
    Camerica(camerica::Camerica),
    Dxrom(dxrom::Dxrom),
    Mapper227(mapper227::Mapper227),
    Vrc6(vrc6::Vrc6),
}

pub const SUPPORTED_MAPPERS: &[u16] = &[0, 1, 2, 3, 4, 7, 9, 11, 24, 26, 34, 66, 69, 71, 206, 227];

impl MapperKind {
    /// Cria o mapper para o `id`; `None` se não suportado.
    pub fn create(id: u16, data: &CartData) -> Option<MapperKind> {
        Some(match id {
            0 => MapperKind::Nrom(nrom::Nrom),
            1 => MapperKind::Mmc1(mmc1::Mmc1::new(data)),
            2 => MapperKind::Uxrom(uxrom::Uxrom::new()),
            3 => MapperKind::Cnrom(cnrom::Cnrom::new()),
            4 => MapperKind::Mmc3(mmc3::Mmc3::new(data)),
            7 => MapperKind::Axrom(axrom::Axrom::new()),
            9 => MapperKind::Mmc2(mmc2::Mmc2::new()),
            11 => MapperKind::ColorDreams(colordreams::ColorDreams::new()),
            34 => MapperKind::Bnrom(bnrom::Bnrom::new()),
            66 => MapperKind::Gxrom(gxrom::Gxrom::new()),
            69 => MapperKind::Fme7(fme7::Fme7::new()),
            71 => MapperKind::Camerica(camerica::Camerica::new()),
            206 => MapperKind::Dxrom(dxrom::Dxrom::new(data)),
            227 => MapperKind::Mapper227(mapper227::Mapper227::new()),
            24 | 26 => MapperKind::Vrc6(vrc6::Vrc6::new(data)),
            _ => return None,
        })
    }

    pub fn name(&self) -> &'static str {
        match self {
            MapperKind::Nrom(_) => "NROM",
            MapperKind::Mmc1(_) => "MMC1",
            MapperKind::Uxrom(_) => "UxROM",
            MapperKind::Cnrom(_) => "CNROM",
            MapperKind::Mmc3(_) => "MMC3",
            MapperKind::Axrom(_) => "AxROM",
            MapperKind::Mmc2(_) => "MMC2",
            MapperKind::ColorDreams(_) => "Color Dreams",
            MapperKind::Bnrom(_) => "BNROM",
            MapperKind::Gxrom(_) => "GxROM",
            MapperKind::Fme7(_) => "FME-7",
            MapperKind::Camerica(_) => "Camerica",
            MapperKind::Dxrom(_) => "DxROM",
            MapperKind::Mapper227(_) => "Mapper 227",
            MapperKind::Vrc6(v) => {
                if v.is_26() {
                    "VRC6b"
                } else {
                    "VRC6a"
                }
            }
        }
    }
}

macro_rules! dispatch {
    ($self:expr, $m:ident => $e:expr) => {
        match $self {
            MapperKind::Nrom($m) => $e,
            MapperKind::Mmc1($m) => $e,
            MapperKind::Uxrom($m) => $e,
            MapperKind::Cnrom($m) => $e,
            MapperKind::Mmc3($m) => $e,
            MapperKind::Axrom($m) => $e,
            MapperKind::Mmc2($m) => $e,
            MapperKind::ColorDreams($m) => $e,
            MapperKind::Bnrom($m) => $e,
            MapperKind::Gxrom($m) => $e,
            MapperKind::Fme7($m) => $e,
            MapperKind::Camerica($m) => $e,
            MapperKind::Dxrom($m) => $e,
            MapperKind::Mapper227($m) => $e,
            MapperKind::Vrc6($m) => $e,
        }
    };
}

impl Mapper for MapperKind {
    #[inline]
    fn cpu_read(&self, addr: u16, data: &CartData) -> Option<u8> {
        dispatch!(self, m => m.cpu_read(addr, data))
    }
    #[inline]
    fn cpu_write(&mut self, addr: u16, val: u8, data: &mut CartData) -> bool {
        dispatch!(self, m => m.cpu_write(addr, val, data))
    }
    #[inline]
    fn chr_offset(&self, addr: u16) -> usize {
        dispatch!(self, m => m.chr_offset(addr))
    }
    #[inline]
    fn ppu_read(&mut self, addr: u16, data: &CartData) -> u8 {
        dispatch!(self, m => m.ppu_read(addr, data))
    }
    #[inline]
    fn ppu_write(&mut self, addr: u16, val: u8, data: &mut CartData) {
        dispatch!(self, m => m.ppu_write(addr, val, data))
    }
    #[inline]
    fn a12_rise(&mut self) {
        dispatch!(self, m => m.a12_rise())
    }
    fn wants_cpu_clock(&self) -> bool {
        dispatch!(self, m => m.wants_cpu_clock())
    }
    #[inline]
    fn cpu_clock(&mut self) {
        dispatch!(self, m => m.cpu_clock())
    }
    #[inline]
    fn irq_pending(&self) -> bool {
        dispatch!(self, m => m.irq_pending())
    }
    #[inline]
    fn audio_output(&self) -> f32 {
        dispatch!(self, m => m.audio_output())
    }
    fn manages_prg_ram(&self) -> bool {
        dispatch!(self, m => m.manages_prg_ram())
    }
    fn reset(&mut self, data: &mut CartData) {
        dispatch!(self, m => m.reset(data))
    }
    fn state_string(&self) -> String {
        dispatch!(self, m => m.state_string())
    }
}
