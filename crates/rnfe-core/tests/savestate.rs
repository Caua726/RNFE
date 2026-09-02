//! Save states (feature `serde`): round-trip por mapper, idempotência, rejeições.
#![cfg(feature = "serde")]

use rnfe_core::testing::runner::{fnv1a64, load};
use rnfe_core::{Buttons, Nes, StateError};

fn frame_hash(nes: &mut Nes) -> u64 {
    fnv1a64(nes.framebuffer())
}

/// Roda `a` frames, salva, roda `b` frames (hash A); restaura, roda `b` frames (hash B).
fn round_trip(rel: &str, a: u32, b: u32) {
    let mut nes = match load(rel) {
        Ok(n) => n,
        Err(e) => {
            eprintln!("SKIP {rel}: {e}");
            return;
        }
    };
    nes.set_controller(0, Buttons::START);
    for _ in 0..a {
        nes.run_frame();
    }
    let state = nes.save_state();
    assert!(state.len() < 64 * 1024, "{rel}: state de {} bytes", state.len());
    for _ in 0..b {
        nes.run_frame();
    }
    let hash_a = frame_hash(&mut nes);
    let cyc_a = nes.cpu_cycles();

    nes.load_state(&state).unwrap();
    // idempotência: o que foi carregado serializa igual ao que foi salvo
    assert_eq!(nes.save_state(), state, "{rel}: save(load(s)) != s (campo faltando?)");
    for _ in 0..b {
        nes.run_frame();
    }
    assert_eq!(frame_hash(&mut nes), hash_a, "{rel}: frame diferente após restaurar");
    assert_eq!(nes.cpu_cycles(), cyc_a, "{rel}: ciclos diferentes após restaurar");

    // e num console novo com a mesma ROM
    let mut fresh = load(rel).unwrap();
    fresh.set_controller(0, Buttons::START);
    fresh.load_state(&state).unwrap();
    for _ in 0..b {
        fresh.run_frame();
    }
    assert_eq!(frame_hash(&mut fresh), hash_a, "{rel}: frame diferente num console novo");
}

#[test]
fn nrom_round_trip() {
    round_trip("other/nestest.nes", 30, 45);
}

#[test]
fn nrom_game_round_trip() {
    round_trip("other/BladeBuster.nes", 200, 90);
}

#[test]
fn mmc3_round_trip() {
    round_trip("mmc3_test_2/rom_singles/1-clocking.nes", 20, 40);
    round_trip("mmc3_irq_tests/4.Scanline_timing.nes", 20, 40);
}

#[test]
fn mmc1_chr_ram_round_trip() {
    round_trip("MMC1_A12/mmc1_a12.nes", 60, 30);
}

#[test]
fn apu_round_trip() {
    round_trip("apu_test/rom_singles/1-len_ctr.nes", 30, 30);
}

#[test]
fn rejects_wrong_rom_version_and_garbage() {
    let (Ok(mut a), Ok(mut b)) = (load("other/nestest.nes"), load("other/BladeBuster.nes")) else {
        eprintln!("SKIP: ROMs ausentes");
        return;
    };
    a.run_frame();
    let sa = a.save_state();
    assert!(matches!(b.load_state(&sa), Err(StateError::RomMismatch { .. })));
    assert_eq!(a.load_state(b"RNFX0000"), Err(StateError::BadMagic));
    assert_eq!(a.load_state(&sa[..10]), Err(StateError::BadMagic));
    let mut bad = sa.clone();
    bad[4] = 0xFF;
    assert!(matches!(a.load_state(&bad), Err(StateError::Version(_))));
    let mut trunc = sa.clone();
    trunc.truncate(200);
    assert!(matches!(a.load_state(&trunc), Err(StateError::Corrupt(_))));
    // o console continua utilizável depois das rejeições
    a.load_state(&sa).unwrap();
    a.run_frame();
}
