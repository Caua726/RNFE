#![cfg(feature = "state")]
use rnfe_core::testing::runner::{fnv1a64, load};
use rnfe_frontend::Rewind;

#[test]
fn rewind_restores_middle_of_history() {
    let Ok(mut nes) = load("other/BladeBuster.nes") else {
        eprintln!("SKIP: ROM ausente");
        return;
    };
    let mut rw = Rewind::new(Rewind::DEFAULT_CAP);
    let mut hashes = Vec::new();
    for _ in 0..60 {
        nes.run_frame();
        rw.record(&nes);
        hashes.push(fnv1a64(nes.framebuffer()));
    }
    assert_eq!(rw.len(), 12, "um state a cada 5 frames");
    // states nos frames 5, 10, …, 60; 6 passos para trás restauram o do frame 35
    for _ in 0..6 {
        assert!(rw.step_back(&mut nes));
    }
    assert_eq!(rw.len(), 6);
    nes.run_frame(); // frame 36
    assert_eq!(fnv1a64(nes.framebuffer()), hashes[35]);
    nes.run_frame(); // frame 37
    assert_eq!(fnv1a64(nes.framebuffer()), hashes[36]);
    // esvazia
    while rw.step_back(&mut nes) {}
    assert!(rw.is_empty());
    assert!(!rw.step_back(&mut nes));
}

#[test]
fn rewind_respects_memory_cap() {
    let Ok(mut nes) = load("other/nestest.nes") else {
        eprintln!("SKIP: ROM ausente");
        return;
    };
    nes.run_frame();
    let one = nes.save_state().len();
    let mut rw = Rewind::new(one * 3 + 1);
    for _ in 0..100 {
        nes.run_frame();
        rw.record(&nes);
    }
    assert!(rw.len() <= 3, "{} states, cap de 3", rw.len());
    assert!(rw.bytes() <= one * 3 + 1);
}
