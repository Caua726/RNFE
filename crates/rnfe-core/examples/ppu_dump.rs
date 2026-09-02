//! Roda uma ROM N frames e imprime o estado da PPU (registradores, paleta, scroll) e grava o
//! frame em PNG: `cargo run -q -p rnfe-core --release --example ppu_dump -- rom.nes 120 out.png`

use rnfe_core::testing::write_png;

fn main() {
    let a: Vec<String> = std::env::args().skip(1).collect();
    let bytes = std::fs::read(&a[0]).expect("rom");
    let frames: u32 = a.get(1).and_then(|s| s.parse().ok()).unwrap_or(60);
    let cart = rnfe_core::Cartridge::from_bytes(&bytes).expect("ROM inválida");
    let mut nes = rnfe_core::Nes::new(cart);
    for _ in 0..frames {
        nes.run_frame();
    }
    let ppu = &nes.bus.ppu;
    println!(
        "frame {frames}: ctrl=${:02X} mask=${:02X} status=${:02X} v=${:04X} t=${:04X}",
        ppu.control, ppu.mask, ppu.status, ppu.vram_addr, ppu.tram_addr
    );
    println!("paleta: {:02X?}", ppu.palette_table);
    println!("oam[0..32]: {:02X?}", &ppu.oam[..32]);
    let idx = nes.framebuffer_indexed();
    let mut hist = std::collections::BTreeMap::new();
    for &p in idx {
        *hist.entry(p).or_insert(0u32) += 1;
    }
    println!("cores no frame (índice 9 bits → pixels): {hist:?}");
    if let Some(out) = a.get(2) {
        write_png(std::path::Path::new(out), nes.framebuffer()).unwrap();
    }
}
