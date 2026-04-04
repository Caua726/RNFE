mod cpu6502;
mod bus;
mod ppu;
mod apu;
mod cartridge;
mod display;
mod font;
mod ui;
mod debug;
mod diagnostic;
mod nes;

use nes::Nes;
use cartridge::Cartridge;
use std::env;

fn load_rom(path: &str) -> Option<Box<Nes>> {
    match Cartridge::new(path) {
        Ok(cartridge) => {
            println!("ROM carregada: {}", path);
            let mut nes = Box::new(Nes::new());
            nes.insert_cartridge(cartridge);
            nes.reset();
            Some(nes)
        },
        Err(e) => {
            eprintln!("Erro ao carregar ROM '{}': {}", path, e);
            None
        }
    }
}

pub fn pick_rom() -> Option<String> {
    let file = rfd::FileDialog::new()
        .add_filter("NES ROM", &["nes"])
        .set_title("Abrir ROM")
        .pick_file()?;
    Some(file.to_string_lossy().to_string())
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args: Vec<String> = env::args().collect();

    if args.len() >= 2 {
        match load_rom(&args[1]) {
            Some(nes) => display::run_with_nes(nes)?,
            None => display::run()?,
        }
    } else {
        display::run()?;
    }

    Ok(())
}
