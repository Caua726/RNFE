//! Executa uma ROM de teste headless e interpreta o resultado.
//!
//! Dois protocolos existem nas ROMs da comunidade:
//! - **Mem** (blargg moderno): `$6001-$6003` = `DE B0 61`; `$6000` = `$80` rodando, `$81` pede reset,
//!   `$00` passou, outro valor = código de erro; texto em `$6004`.
//! - **Screen** (testes de 2005): o resultado é escrito na nametable como texto
//!   (`PASSED`, `FAILED #n`) ou como um código `$NN` em que `$01` significa "passou".

use super::list::{Style, TestRom};
use crate::{Cartridge, Nes};
use std::path::{Path, PathBuf};

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Outcome {
    Pass,
    Fail(String),
    Timeout(String),
    /// ROM ausente ou ilegível — não é resultado de emulação.
    Skip(String),
}

/// Diretório das ROMs: `$RNFE_TEST_ROMS` ou `<repo>/test-roms`.
pub fn roms_dir() -> PathBuf {
    std::env::var_os("RNFE_TEST_ROMS")
        .map(PathBuf::from)
        .unwrap_or_else(|| Path::new(env!("CARGO_MANIFEST_DIR")).join("../../test-roms"))
}

pub fn load(rel_path: &str) -> Result<Nes, String> {
    load_with(rel_path, 0)
}

/// Carrega ou pula o teste (`None`) se a ROM não estiver disponível — a menos que
/// `RNFE_REQUIRE_ROMS=1` (CI), quando a ausência é falha.
pub fn load_or_skip(rel_path: &str) -> Option<Nes> {
    match load(rel_path) {
        Ok(n) => Some(n),
        Err(e) if std::env::var_os("RNFE_REQUIRE_ROMS").is_some() => panic!("{e}"),
        Err(e) => {
            eprintln!("SKIP {rel_path}: {e}");
            None
        }
    }
}

/// Carrega a ROM; com `submapper > 0` reescreve o header como NES 2.0 com esse submapper.
pub fn load_with(rel_path: &str, submapper: u8) -> Result<Nes, String> {
    let full = roms_dir().join(rel_path);
    let mut bytes = std::fs::read(&full)
        .map_err(|e| format!("ROM ausente {} ({e}); rode scripts/fetch-roms.sh", full.display()))?;
    if submapper > 0 && bytes.len() >= 16 {
        bytes[7] = (bytes[7] & 0xF3) | 0x08;
        bytes[8] = submapper << 4;
        bytes[9..16].fill(0);
    }
    let cart = Cartridge::from_bytes(&bytes).map_err(|e| format!("{rel_path}: {e}"))?;
    Ok(Nes::new(cart))
}

/// Texto da nametable 0 (as ROMs de teste usam tiles = ASCII).
pub fn screen_text(nes: &Nes) -> String {
    let nt = &nes.bus.ppu.nametable[0];
    (0..30)
        .map(|row| {
            (0..32)
                .map(|col| {
                    let t = nt[row * 32 + col];
                    if (0x20..0x7F).contains(&t) { t as char } else { ' ' }
                })
                .collect::<String>()
                .trim_end()
                .to_string()
        })
        .filter(|l| !l.is_empty())
        .collect::<Vec<_>>()
        .join("\n")
}

/// Texto que a ROM escreveu em `$6004..` (protocolo Mem).
pub fn mem_text(nes: &Nes) -> String {
    (0x6004u16..0x7000).map(|a| nes.peek(a)).take_while(|&b| b != 0).map(|b| b as char).collect()
}

fn last_line(s: &str) -> String {
    s.lines().rev().find(|l| !l.trim().is_empty()).unwrap_or("").trim().to_string()
}

/// Roda a ROM segundo o protocolo do estilo. Determinístico: só conta frames.
pub fn run(t: &TestRom) -> Outcome {
    let mut nes = match load_with(t.path, t.submapper) {
        Ok(n) => n,
        Err(e) => return Outcome::Skip(e),
    };
    let mut reset_at: Option<u32> = None;
    // depois de um reset, $6000 continua $81 até a ROM escrever outra coisa
    let mut just_reset = false;
    for frame in 0..t.max_frames {
        nes.run_frame();
        match t.style {
            Style::Mem => {
                let sig = [nes.peek(0x6001), nes.peek(0x6002), nes.peek(0x6003)] == [0xDE, 0xB0, 0x61];
                if !sig {
                    continue;
                }
                let status = nes.peek(0x6000);
                if status != 0x81 {
                    just_reset = false;
                }
                match status {
                    0x80 => {}
                    0x81 if just_reset => {}
                    0x81 => match reset_at {
                        // a ROM pede um reset depois de pelo menos 100 ms (pode pedir mais de um)
                        None => reset_at = Some(frame + 8),
                        Some(f) if frame >= f => {
                            nes.reset();
                            reset_at = None;
                            just_reset = true;
                        }
                        _ => {}
                    },
                    0x00 => return Outcome::Pass,
                    code => {
                        return Outcome::Fail(format!("código {code:#04x}: {}", last_line(&mem_text(&nes))));
                    }
                }
            }
            Style::Crc(expected) => {
                if frame % 10 != 9 {
                    continue;
                }
                let text = screen_text(&nes);
                let low = text.to_ascii_lowercase();
                if low.contains("passed") {
                    return Outcome::Pass;
                }
                if low.contains("failed") || low.contains("error") {
                    return Outcome::Fail(last_line(&text));
                }
                // primeira linha que é um CRC de 8 dígitos hex
                let is_hex8 = |l: &str| l.len() == 8 && l.bytes().all(|b| b.is_ascii_hexdigit());
                if let Some(crc) = text.lines().map(str::trim).find(|l| is_hex8(l)) {
                    return if expected.iter().any(|e| e.eq_ignore_ascii_case(crc)) {
                        Outcome::Pass
                    } else {
                        Outcome::Fail(format!("CRC {crc}, esperado {}", expected.join("/")))
                    };
                }
            }
            Style::Screen => {
                if frame % 10 != 9 {
                    continue;
                }
                let text = screen_text(&nes);
                let low = text.to_ascii_lowercase();
                if low.contains("passed") {
                    return Outcome::Pass;
                }
                if low.contains("failed") || low.contains("error") {
                    return Outcome::Fail(last_line(&text));
                }
                // código "$NN" numa linha própria: $01 = passou
                if let Some(code) = text
                    .lines()
                    .rev()
                    .filter_map(|l| l.trim().strip_prefix('$'))
                    .filter(|h| h.trim().len() == 2)
                    .find_map(|h| u8::from_str_radix(h.trim(), 16).ok())
                {
                    return if code == 1 {
                        Outcome::Pass
                    } else {
                        Outcome::Fail(format!("código ${code:02X}: {}", last_line(&text)))
                    };
                }
            }
        }
    }
    let text = if t.style == Style::Mem { mem_text(&nes) } else { screen_text(&nes) };
    Outcome::Timeout(format!("{} frames sem veredito; última linha: {}", t.max_frames, last_line(&text)))
}

/// FNV-1a de 64 bits — hash estável do framebuffer, sem dependências.
pub fn fnv1a64(bytes: &[u8]) -> u64 {
    bytes.iter().fold(0xcbf29ce484222325u64, |h, &b| (h ^ b as u64).wrapping_mul(0x100000001b3))
}

/// Grava o framebuffer RGBA 256×240 como PNG sem compressão (zlib "stored") — sem dependências,
/// para olhar um snapshot divergente em qualquer visualizador.
pub fn write_png(path: &Path, rgba: &[u8]) -> std::io::Result<()> {
    std::fs::write(path, crate::png::encode(rgba, crate::SCREEN_W, crate::SCREEN_H))
}
