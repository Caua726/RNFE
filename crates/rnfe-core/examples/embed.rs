//! Como embutir o núcleo num programa: carregar a ROM, rodar frames, ler o vídeo e o áudio,
//! salvar e restaurar um state. `cargo run -p rnfe-core --release --features serde --example embed -- rom.nes`
//!
//! Sem a feature `serde` o exemplo roda igual, só pula a parte do save state.
use rnfe_core::{Buttons, Cartridge, Nes};

fn main() {
    let path = std::env::args().nth(1).unwrap_or_else(|| {
        eprintln!("uso: embed <rom.nes>");
        std::process::exit(2);
    });
    let bytes = std::fs::read(&path).expect("ler ROM");
    let cart = Cartridge::from_bytes(&bytes).expect("ROM iNES/NES 2.0 válida");
    println!("{} — hash {:016x}, bateria: {}", cart.describe(), cart.rom_hash(), cart.has_battery());
    let mut nes = Nes::new(cart);
    // RNFE_REGION=pal força a temporização europeia nas ferramentas
    if std::env::var("RNFE_REGION").is_ok_and(|v| v.eq_ignore_ascii_case("pal")) {
        nes.set_region(rnfe_core::Region::Pal);
    }
    nes.set_sample_rate(48_000);

    // 2 segundos com START apertado no 1º segundo
    let mut audio = Vec::new();
    for frame in 0..120 {
        nes.set_controller(0, if frame < 60 { Buttons::START } else { Buttons::NONE });
        nes.run_frame();
        nes.drain_audio(&mut audio);
    }
    let fb = nes.framebuffer(); // RGBA8, 256×240
    let bright = fb.chunks(4).filter(|p| p[0] as u32 + p[1] as u32 + p[2] as u32 > 96).count();
    println!("120 frames: {} amostras de áudio, {bright} pixels claros, PC=${:04X}", audio.len(), nes.cpu.pc);

    #[cfg(feature = "serde")]
    {
        let state = nes.save_state();
        for _ in 0..60 {
            nes.run_frame();
        }
        let after = nes.cpu_cycles();
        nes.load_state(&state).expect("mesma ROM");
        println!("save state de {} bytes; restaurado: ciclos {} → {}", state.len(), after, nes.cpu_cycles());
    }
}
