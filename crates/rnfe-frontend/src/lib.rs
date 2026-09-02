//! Peças compartilhadas por todos os frontends (tty, desktop, web, Android).
//!
//! Nada aqui conhece janela, GPU, áudio ou relógio de sistema: o tempo entra como
//! [`std::time::Duration`] medido pelo chamador, para funcionar igual em nativo e wasm.

pub mod input;
pub mod pacer;

pub use input::InputState;
pub use pacer::FramePacer;

/// Frequência de quadros do NTSC (Hz).
pub const NTSC_FPS: f64 = 60.098_814;
