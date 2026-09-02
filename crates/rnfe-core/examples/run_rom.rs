//! Roda uma ROM da tabela de testes e imprime o veredito com a mensagem completa.
//!
//! `cargo run -q -p rnfe-core --release --example run_rom -- <substring do path>`

use rnfe_core::testing::{TESTS, run};

fn main() {
    let filter = std::env::args().nth(1).unwrap_or_default();
    for t in TESTS.iter().filter(|t| t.path.contains(&filter)) {
        println!("{:<55} {:?}", t.path, run(t));
    }
}
