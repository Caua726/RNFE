mod cpu6502;
mod bus;
mod ppu;
mod apu;
mod cartridge;
mod display;
mod font;
mod nes;

use nes::Nes;
use cartridge::Cartridge;
use std::env;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args: Vec<String> = env::args().collect();

    if args.len() < 2 {
        println!("Nenhuma ROM passada, abrindo janela...");
        display::run()?;
        return Ok(());
    }

    let rom_path = &args[1];

    let mut nes = Box::new(Nes::new());

    match Cartridge::new(rom_path) {
        Ok(cartridge) => {
            println!("ROM carregada: {}", rom_path);
            nes.insert_cartridge(cartridge);
            nes.reset();
            display::run_with_nes(nes)?
        },
        Err(e) => {
            eprintln!("Erro ao carregar ROM '{}': {}", rom_path, e);
            display::run()?
        }
    }

    Ok(())
}
