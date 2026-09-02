//! Frontend gráfico do RNFE: janela, GPU, áudio e menus.

pub mod app;
pub mod ui;

pub use app::{run, run_with_nes};

use rnfe_core::{Cartridge, Nes};

/// Abre um diálogo nativo para escolher uma ROM `.nes`.
pub fn pick_rom() -> Option<String> {
    let file = rfd::FileDialog::new().add_filter("NES ROM", &["nes"]).set_title("Abrir ROM").pick_file()?;
    Some(file.to_string_lossy().to_string())
}

/// Carrega uma ROM do disco e devolve um console pronto (já resetado).
pub fn load_rom(path: &str) -> Option<Box<Nes>> {
    let bytes = match std::fs::read(path) {
        Ok(b) => b,
        Err(e) => {
            eprintln!("Erro ao ler '{}': {}", path, e);
            return None;
        }
    };
    match Cartridge::from_bytes(&bytes) {
        Ok(cartridge) => {
            println!("ROM carregada: {} ({})", path, cartridge.describe());
            Some(Box::new(Nes::new(cartridge)))
        }
        Err(e) => {
            eprintln!("Erro ao carregar ROM '{}': {}", path, e);
            None
        }
    }
}
