//! Roda ROMs da tabela de testes e imprime o veredito; com `-v`, também o texto completo
//! que a ROM deixou na memória (`$6004`) e na tela.
//!
//! `cargo run -q -p rnfe-core --release --example run_rom -- [-v] <substring do path>`

use rnfe_core::testing::{Outcome, TESTS, load, mem_text, run, screen_text};

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let verbose = args.iter().any(|a| a == "-v");
    let filter = args.iter().find(|a| !a.starts_with('-')).cloned().unwrap_or_default();
    for t in TESTS.iter().filter(|t| t.path.contains(&filter)) {
        let out = run(t);
        println!("{:<55} {:?}", t.path, out);
        if verbose && out != Outcome::Pass {
            if let Ok(mut nes) = load(t.path) {
                for _ in 0..t.max_frames.min(600) {
                    nes.run_frame();
                }
                let m = mem_text(&nes);
                let s = screen_text(&nes);
                println!(
                    "--- $6000={:02X} pc={:04X} jammed={} mem:\n{m}\n--- tela:\n{s}\n",
                    nes.peek(0x6000),
                    nes.cpu.pc,
                    nes.cpu.jammed
                );
            }
        }
    }
}
