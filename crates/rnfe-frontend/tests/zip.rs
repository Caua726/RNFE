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

/// Zip escrito em fluxo (bit 3 do flag, tamanhos só no descritor) não pode travar o leitor:
/// antes o iterador voltava sem avançar e girava para sempre.
#[test]
fn zip_em_fluxo_nao_trava() {
    let mut z = zip::MAGIC.to_vec();
    z.extend_from_slice(&20u16.to_le_bytes()); // versão
    z.extend_from_slice(&0x08u16.to_le_bytes()); // flags: data descriptor
    z.extend_from_slice(&0u16.to_le_bytes()); // método: stored
    z.extend_from_slice(&[0; 8]); // hora/data/crc
    z.extend_from_slice(&0u32.to_le_bytes()); // tamanho comprimido: desconhecido
    z.extend_from_slice(&0u32.to_le_bytes()); // tamanho original: desconhecido
    z.extend_from_slice(&(9u16).to_le_bytes()); // nome
    z.extend_from_slice(&0u16.to_le_bytes()); // extra
    z.extend_from_slice(b"leiame.txt".get(..9).unwrap());
    z.extend_from_slice(b"conteudo");
    assert!(zip::extract_nes(&z).is_none());
}

/// Um `.nes` com nome enorme ou com diretório vira um nome curto e sem caminho.
#[test]
fn nome_de_dentro_do_zip_e_higienizado() {
    let long: String = "a".repeat(400);
    let name = format!("../../etc/{long}.nes");
    let payload = b"NES\x1a rom";
    let mut z = zip::MAGIC.to_vec();
    z.extend_from_slice(&20u16.to_le_bytes());
    z.extend_from_slice(&0u16.to_le_bytes());
    z.extend_from_slice(&0u16.to_le_bytes());
    z.extend_from_slice(&[0; 8]);
    z.extend_from_slice(&(payload.len() as u32).to_le_bytes());
    z.extend_from_slice(&(payload.len() as u32).to_le_bytes());
    z.extend_from_slice(&(name.len() as u16).to_le_bytes());
    z.extend_from_slice(&0u16.to_le_bytes());
    z.extend_from_slice(name.as_bytes());
    z.extend_from_slice(payload);
    let (n, d) = zip::extract_nes(&z).expect("achar a ROM");
    assert!(!n.contains('/') && n.len() <= 120, "nome não higienizado: {}", n.len());
    assert_eq!(d, payload);
}
