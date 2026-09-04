//! Compatibilidade do formato de save state: um `.rnfs` gravado por uma versão anterior
//! (`tests/golden/nestest.rnfs`) precisa continuar carregando e produzindo a mesma imagem.
//! Se o formato mudar de propósito, suba `state::VERSION` e regrave com
//! `RNFE_UPDATE_GOLDEN=1 cargo test --features serde --test golden_state`.

#![cfg(feature = "serde")]

use rnfe_core::testing::{NESTEST_ROM, fnv1a64, load_or_skip};

const GOLDEN: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/tests/golden/nestest.rnfs");
const HASH: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/tests/golden/nestest.hash");
const FRAMES_BEFORE: usize = 120;
const FRAMES_AFTER: usize = 10;

#[test]
fn golden_state_loads_and_matches() {
    let Some(mut nes) = load_or_skip(NESTEST_ROM) else { return };
    if std::env::var_os("RNFE_UPDATE_GOLDEN").is_some() {
        for _ in 0..FRAMES_BEFORE {
            nes.run_frame();
        }
        let state = nes.save_state();
        for _ in 0..FRAMES_AFTER {
            nes.run_frame();
        }
        let hash = fnv1a64(nes.framebuffer());
        std::fs::write(GOLDEN, &state).unwrap();
        std::fs::write(HASH, format!("{hash:016x}\n")).unwrap();
        println!("golden regravado: {} bytes, hash {hash:016x}", state.len());
        return;
    }
    let state =
        std::fs::read(GOLDEN).expect("tests/golden/nestest.rnfs ausente (RNFE_UPDATE_GOLDEN=1 para gerar)");
    let want = std::fs::read_to_string(HASH).unwrap();
    nes.load_state(&state)
        .expect("save state antigo não carrega mais: suba state::VERSION e regrave o golden");
    for _ in 0..FRAMES_AFTER {
        nes.run_frame();
    }
    let got = fnv1a64(nes.framebuffer());
    assert_eq!(format!("{got:016x}"), want.trim(), "imagem após carregar o golden mudou");
}
