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
