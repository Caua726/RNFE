//! Núcleo do RNFE — emulação pura, sem I/O, sem dependências.
//!
//! O frontend (desktop, web, tty, Android) só precisa de [`Nes`] e [`Cartridge`].

#![forbid(unsafe_code)]

pub mod apu;
pub mod bus;
pub mod buttons;
pub mod cartridge;
pub mod cpu6502;
pub mod debug;
pub mod diagnostic;
pub mod mappers;
pub mod nes;
pub mod png;
pub mod ppu;
#[cfg(feature = "serde")]
pub mod state;
pub mod storage;

#[doc(hidden)]
#[cfg(not(target_arch = "wasm32"))]
pub mod testing;

/// Frames por segundo do NTSC (PPU a 5,369 MHz, 89 341,5 dots por frame).
pub const NTSC_FPS: f64 = 60.098_814;
/// Frames por segundo do PAL (312 linhas de 341 dots, PPU a 5,320 MHz).
pub const PAL_FPS: f64 = 50.006_98;

/// Temporização do console: muda o relógio da CPU, o número de linhas e as tabelas da APU.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub enum Region {
    #[default]
    Ntsc,
    Pal,
}

impl Region {
    /// Frames por segundo desta região.
    pub fn fps(self) -> f64 {
        match self {
            Region::Ntsc => NTSC_FPS,
            Region::Pal => PAL_FPS,
        }
    }

    /// Dezesseis vezes os dots de PPU por ciclo de CPU (48/16 = 3 no NTSC, 16/5 = 3,2 no PAL).
    /// Guardado como fração para o bus acumular sem ponto flutuante.
    pub(crate) fn dots_per_cycle(self) -> (u32, u32) {
        match self {
            Region::Ntsc => (3, 1),
            Region::Pal => (16, 5),
        }
    }

    /// Última scanline antes da pré-render (260 no NTSC, 310 no PAL).
    pub fn last_scanline(self) -> i16 {
        match self {
            Region::Ntsc => 261,
            Region::Pal => 311,
        }
    }

    /// Linha em que o flag de vblank sobe (o PAL tem duas linhas de pós-render).
    pub fn vblank_scanline(self) -> i16 {
        match self {
            Region::Ntsc => 241,
            Region::Pal => 242,
        }
    }

    pub fn name(self) -> &'static str {
        match self {
            Region::Ntsc => "NTSC",
            Region::Pal => "PAL",
        }
    }
}

pub use buttons::Buttons;
pub use cartridge::{Cartridge, Mirror, RomError, RomHeader};
pub use nes::Nes;
#[cfg(feature = "serde")]
pub use state::StateError;
pub use storage::{MemoryStorage, Storage, StorageError};

/// Largura da imagem gerada pela PPU, em pixels.
pub const SCREEN_W: usize = 256;
/// Altura da imagem gerada pela PPU, em pixels.
pub const SCREEN_H: usize = 240;
