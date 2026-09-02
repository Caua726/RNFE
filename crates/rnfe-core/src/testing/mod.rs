//! Infraestrutura de testes compartilhada por `tests/` e pelos binários `status`/`bench`.
//!
//! Não faz parte da API do emulador; fica no crate para que a tabela de ROMs exista em um só lugar.

pub mod list;
pub mod runner;

pub use list::{Expect, Style, TestRom, NESTEST_LOG, NESTEST_ROM, TESTS};
pub use runner::{fnv1a64, load, roms_dir, run, screen_text, write_ppm, Outcome};
