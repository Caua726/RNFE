//! Roda uma ROM sem frontend e imprime fps + hash do framebuffer.
//!
//! `cargo run -p rnfe-core --release --example headless -- rom.nes [frames]`
fn main() {
    let args: Vec<String> = std::env::args().collect();
    let Some(path) = args.get(1) else {
        eprintln!("uso: headless <rom.nes> [frames]");
        std::process::exit(2);
    };
    let frames: u32 = args.get(2).and_then(|s| s.parse().ok()).unwrap_or(600);
    let bytes = std::fs::read(path).expect("ler ROM");
    let cart = rnfe_core::Cartridge::from_bytes(&bytes).expect("ROM inválida");
    println!("{}", cart.describe());
    let mut nes = rnfe_core::Nes::new(cart);
    let t = std::time::Instant::now();
    for _ in 0..frames {
        nes.run_frame();
    }
    let dt = t.elapsed().as_secs_f64();
    let hash = nes
        .framebuffer()
        .iter()
        .fold(0xcbf29ce484222325u64, |h, &b| (h ^ b as u64).wrapping_mul(0x100000001b3));
    println!(
        "{frames} frames em {dt:.2}s = {:.0} fps | fb {hash:016x} | $6000={:02X} $02={:02X} $03={:02X}",
        frames as f64 / dt,
        nes.peek(0x6000),
        nes.peek(0x02),
        nes.peek(0x03)
    );
}
