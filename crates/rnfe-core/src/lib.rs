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
pub mod ppu;

#[doc(hidden)]
#[cfg(not(target_arch = "wasm32"))]
pub mod testing;

pub use buttons::Buttons;
pub use cartridge::{Cartridge, Mirror, RomError};
pub use nes::Nes;

/// Largura da imagem gerada pela PPU, em pixels.
pub const SCREEN_W: usize = 256;
/// Altura da imagem gerada pela PPU, em pixels.
pub const SCREEN_H: usize = 240;
