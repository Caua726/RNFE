use rnfe_core::{Buttons, Cartridge, Nes, RomError};

fn header(prg: u8, chr: u8, flags6: u8, flags7: u8) -> Vec<u8> {
    let mut v = b"NES\x1A".to_vec();
    v.extend_from_slice(&[prg, chr, flags6, flags7, 0, 0, 0, 0, 0, 0, 0, 0]);
    v
}

#[test]
fn bad_magic() {
    assert_eq!(Cartridge::from_bytes(b"NOPE").err(), Some(RomError::BadMagic));
    assert_eq!(Cartridge::from_bytes(&[]).err(), Some(RomError::BadMagic));
}

#[test]
fn truncated_prg() {
    let mut rom = header(1, 0, 0, 0);
    rom.extend(std::iter::repeat_n(0xEA, 100));
    assert_eq!(
        Cartridge::from_bytes(&rom).err(),
        Some(RomError::Truncated { expected: 16 + 16384, got: 116 })
    );
}

#[test]
fn truncated_chr() {
    let mut rom = header(1, 1, 0, 0);
    rom.extend(std::iter::repeat_n(0xEA, 16384));
    assert_eq!(
        Cartridge::from_bytes(&rom).err(),
        Some(RomError::Truncated { expected: 16 + 16384 + 8192, got: 16 + 16384 })
    );
}

#[test]
fn unsupported_mapper() {
    let mut rom = header(1, 0, 0xF0, 0xF0); // mapper 255
    rom.extend(std::iter::repeat_n(0xEA, 16384));
    assert_eq!(Cartridge::from_bytes(&rom).err(), Some(RomError::UnsupportedMapper(255)));
}

#[test]
fn nrom_with_chr_ram_boots_and_runs() {
    // NROM-128 de NOPs com vetor de reset em $8000; CHR RAM (chr_banks = 0)
    let mut rom = header(1, 0, 0, 0);
    let mut prg = vec![0xEAu8; 16384];
    prg[0x3FFC] = 0x00; // reset vector -> $8000
    prg[0x3FFD] = 0x80;
    rom.extend_from_slice(&prg);
    let cart = Cartridge::from_bytes(&rom).expect("rom válida");
    assert_eq!(cart.mapper_id(), 0);
    let mut nes = Nes::new(cart);
    nes.set_controller(0, Buttons::A | Buttons::START);
    for _ in 0..3 {
        nes.run_frame();
    }
    assert_eq!(nes.framebuffer().len(), 256 * 240 * 4);
    assert!(nes.framebuffer().chunks(4).all(|p| p[3] == 255), "alpha sempre 255");
    assert_eq!(nes.peek(0x8000), 0xEA);
    assert!(nes.cpu.pc >= 0x8000, "CPU executando na ROM: PC={:04X}", nes.cpu.pc);
    let mut audio = Vec::new();
    nes.drain_audio(&mut audio);
    assert!(!audio.is_empty(), "APU gera amostras");
}
