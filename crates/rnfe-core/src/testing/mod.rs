//! Infraestrutura de testes compartilhada por `tests/` e pelos binários `status`/`bench`.
//!
//! Não faz parte da API do emulador; fica no crate para que a tabela de ROMs exista em um só lugar.

pub mod list;
pub mod nestest;
pub mod runner;

pub use list::{Expect, NESTEST_LOG, NESTEST_ROM, Style, TESTS, TestRom};
pub use runner::{Outcome, fnv1a64, load, roms_dir, run, screen_text, write_png};
