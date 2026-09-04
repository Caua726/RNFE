//! Peças compartilhadas por todos os frontends (tty, desktop, web, Android).
//!
//! Nada aqui conhece janela, GPU, áudio ou relógio de sistema: o tempo entra como
//! [`std::time::Duration`] medido pelo chamador, para funcionar igual em nativo e wasm.

pub mod audio_ring;
pub mod config;
pub mod fs_storage;
pub mod input;
pub mod menu;
pub mod pacer;
#[cfg(feature = "state")]
pub mod rewind;
pub mod save_manager;
pub mod touch;

pub use audio_ring::AudioRing;
pub use config::{Config, RecentRom};
pub use fs_storage::FsStorage;
pub use input::InputState;
pub use pacer::FramePacer;
#[cfg(feature = "state")]
pub use rewind::Rewind;
pub use save_manager::SaveManager;
pub use touch::{TouchLayout, TouchState};

/// Frequência de quadros do NTSC (Hz).
pub use rnfe_core::NTSC_FPS;
