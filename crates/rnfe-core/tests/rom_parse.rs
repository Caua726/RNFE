use rnfe_core::{Buttons, Cartridge, Mirror, Nes, RomError, RomHeader};

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

// ---------------------------------------------------------------- header NES 2.0

fn header16(bytes: [u8; 12]) -> Vec<u8> {
    let mut v = b"NES\x1A".to_vec();
    v.extend_from_slice(&bytes);
    v
}

#[test]
fn nes2_mapper_submapper_and_sizes() {
    // flags7 bits 2-3 = 10 → NES 2.0; mapper = 4 | (byte8 & 0x0F) << 8; submapper = byte8 >> 4
    let h = header16([2, 1, 0x4B, 0x08, 0x40, 0x00, 0x77, 0x00, 0, 0, 0, 0]);
    let hdr = RomHeader::parse(&h).unwrap();
    assert!(hdr.nes2);
    assert_eq!(hdr.mapper, 4);
    assert_eq!(hdr.submapper, 4);
    assert_eq!(hdr.prg_len, 32768);
    assert_eq!(hdr.chr_len, 8192);
    assert!(hdr.battery);
    assert!(hdr.four_screen);
    assert_eq!(hdr.mirror, Mirror::FourScreen);
    // byte 10 = $77: 64 << 7 = 8 KB volátil e 8 KB NVRAM
    assert_eq!(hdr.prg_ram_len, 8192);
    assert_eq!(hdr.chr_ram_len, 0);
}

#[test]
fn nes2_high_mapper_and_exponent_size() {
    // mapper 0x1FF (não suportado, mas o header decodifica); PRG em forma exponencial 2^15 × 3
    let h = header16([0x3D, 0, 0xF0, 0xF8, 0x01, 0x0F, 0, 0, 0, 0, 0, 0]);
    let hdr = RomHeader::parse(&h).unwrap();
    assert_eq!(hdr.mapper, 0x1FF);
    assert_eq!(hdr.prg_len, (1 << 15) * 3);
    assert_eq!(Cartridge::from_bytes(&h).err(), Some(RomError::UnsupportedMapper(0x1FF)));
}

#[test]
fn ines1_ignores_mapper_high_nibble_when_padding_is_garbage() {
    // "DiskDude!" nos bytes 7-15 corrompe o nibble alto do mapper: cair para o nibble baixo
    let mut h = header16([1, 1, 0x10, 0x40, 0, 0, 0, 0, 0, 0, 0, 0]);
    h[12..16].copy_from_slice(b"ude!");
    assert_eq!(RomHeader::parse(&h).unwrap().mapper, 1);
    h[12..16].copy_from_slice(&[0; 4]);
    assert_eq!(RomHeader::parse(&h).unwrap().mapper, 0x41);
}

#[test]
fn ines1_prg_ram_default_and_battery() {
    let h = header16([1, 0, 0x02, 0x00, 0, 0, 0, 0, 0, 0, 0, 0]);
    let hdr = RomHeader::parse(&h).unwrap();
    assert!(!hdr.nes2);
    assert!(hdr.battery);
    assert_eq!(hdr.prg_ram_len, 8192);
    assert_eq!(hdr.chr_ram_len, 8192, "sem CHR ROM → 8 KB de CHR RAM");
    let h = header16([1, 0, 0x00, 0x00, 2, 0, 0, 0, 0, 0, 0, 0]);
    assert_eq!(RomHeader::parse(&h).unwrap().prg_ram_len, 16384);
}

#[test]
fn rom_without_prg_is_rejected() {
    let h = header16([0, 1, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0]);
    assert_eq!(RomHeader::parse(&h).err(), Some(RomError::BadHeader("ROM sem PRG")));
}

#[test]
fn small_prg_mirrors_and_hash_is_stable() {
    // NROM-128: $C000 espelha $8000; hash igual para o mesmo conteúdo, diferente se mudar 1 byte
    let mut rom = header(1, 0, 0, 0);
    let mut prg = vec![0u8; 16384];
    prg[0x1234] = 0xAB;
    rom.extend_from_slice(&prg);
    let cart = Cartridge::from_bytes(&rom).unwrap();
    assert_eq!(cart.cpu_read(0x9234), Some(0xAB));
    assert_eq!(cart.cpu_read(0xD234), Some(0xAB));
    assert_eq!(cart.cpu_read(0x5000), None, "abaixo de $6000 é open bus");
    let h1 = cart.rom_hash();
    assert_eq!(Cartridge::from_bytes(&rom).unwrap().rom_hash(), h1);
    rom[16 + 0x1234] = 0xAC;
    assert_ne!(Cartridge::from_bytes(&rom).unwrap().rom_hash(), h1);
}

#[test]
fn prg_ram_dirty_flag() {
    let mut rom = header(1, 0, 0x02, 0);
    rom.extend(std::iter::repeat_n(0xEA, 16384));
    let mut cart = Cartridge::from_bytes(&rom).unwrap();
    assert!(cart.has_battery());
    assert!(!cart.take_prg_ram_dirty());
    cart.cpu_write(0x6010, 0x55);
    assert_eq!(cart.cpu_read(0x6010), Some(0x55));
    assert_eq!(cart.prg_ram()[0x10], 0x55);
    assert!(cart.take_prg_ram_dirty());
    assert!(!cart.take_prg_ram_dirty());
}
