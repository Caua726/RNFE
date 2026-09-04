//! O leitor de zip contra um arquivo de verdade (feito com zlib, Huffman dinâmico): a ROM que
//! sai tem que ser byte a byte igual à que entrou.
use rnfe_frontend::zip;

/// Zip com uma ROM sintética de 24 KB comprimida em DEFLATE (gerado uma vez, versionado).
const ZIP: &[u8] = include_bytes!("data/jogo.zip");

fn rom_esperada() -> Vec<u8> {
    let prg: Vec<u8> = (0..16384u32).map(|i| ((i * 7 + (i >> 5)) & 0xFF) as u8).collect();
    let mut rom = b"NES\x1a".to_vec();
    rom.extend_from_slice(&[1, 1, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0]);
    rom.extend_from_slice(&prg);
    rom.extend_from_slice(&[0u8; 8192]);
    rom
}

#[test]
fn extrai_a_rom_do_zip() {
    let (nome, dados) = zip::extract_nes(ZIP).expect("achar o .nes");
    assert_eq!(nome, "jogo.nes");
    assert_eq!(dados, rom_esperada());
}

#[test]
fn lixo_nao_vira_rom() {
    assert!(zip::extract_nes(b"PK\x03\x04 truncado").is_none());
    assert!(zip::extract_nes(b"NES\x1a nem e zip").is_none());
}
