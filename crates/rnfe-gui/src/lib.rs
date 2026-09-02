//! Frontend gráfico do RNFE: janela, GPU, áudio e menus.

pub mod app;
pub mod ui;

pub use app::{run, run_with_nes};

use rnfe_core::{Cartridge, Nes};

/// Abre um diálogo nativo para escolher uma ROM `.nes`.
pub fn pick_rom() -> Option<String> {
    let file = rfd::FileDialog::new()
        .add_filter("NES ROM", &["nes"])
        .set_title("Abrir ROM")
        .pick_file()?;
    Some(file.to_string_lossy().to_string())
}

/// Carrega uma ROM do disco e devolve um console pronto (já resetado).
pub fn load_rom(path: &str) -> Option<Box<Nes>> {
    match Cartridge::new(path) {
        Ok(cartridge) => {
            println!("ROM carregada: {}", path);
            let mut nes = Box::new(Nes::new());
            nes.insert_cartridge(cartridge);
            nes.reset();
            Some(nes)
        }
        Err(e) => {
            eprintln!("Erro ao carregar ROM '{}': {}", path, e);
            None
        }
    }
}
