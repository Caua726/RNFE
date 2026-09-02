//! Comparação instrução a instrução com o `nestest.log` de referência.

use super::list::{NESTEST_LOG, NESTEST_ROM};
use super::runner::roms_dir;
use crate::Cartridge;
use crate::bus::Bus;
use crate::cpu6502::Cpu6502;

pub const TOTAL_LINES: usize = 8991;

#[derive(PartialEq, Eq, Debug, Clone, Copy)]
pub struct St {
    pub pc: u16,
    pub a: u8,
    pub x: u8,
    pub y: u8,
    pub p: u8,
    pub sp: u8,
    pub cyc: u64,
}

impl St {
    // "C000  4C F5 C5  JMP $C5F5      A:00 X:00 Y:00 P:24 SP:FD PPU:  0, 21 CYC:7"
    pub fn parse(line: &str) -> St {
        let field = |key: &str| {
            let i = line.find(key).expect(key) + key.len();
            u8::from_str_radix(&line[i..i + 2], 16).unwrap()
        };
        St {
            pc: u16::from_str_radix(&line[0..4], 16).unwrap(),
            a: field("A:"),
            x: field("X:"),
            y: field("Y:"),
            p: field("P:"),
            sp: field("SP:"),
            cyc: line.rsplit("CYC:").next().unwrap().trim().parse().unwrap(),
        }
    }

    pub fn fmt(&self) -> String {
        format!(
            "{:04X} A:{:02X} X:{:02X} Y:{:02X} P:{:02X} SP:{:02X} CYC:{}",
            self.pc, self.a, self.x, self.y, self.p, self.sp, self.cyc
        )
    }
}

pub struct NestestResult {
    /// Linhas idênticas antes da primeira divergência (== TOTAL_LINES se bateu tudo).
    pub matched: usize,
    /// (índice 0-based, nosso estado) da primeira divergência.
    pub first_bad: Option<(usize, St)>,
    /// `$02`/`$03` ao fim (00/00 = nestest sem erro).
    pub result_bytes: (u8, u8),
    pub log: Vec<String>,
}

/// Roda o nestest no modo automático (`$C000`) e compara com o log. `Err` se as ROMs faltam.
pub fn compare() -> Result<NestestResult, String> {
    let dir = roms_dir();
    let rom = std::fs::read(dir.join(NESTEST_ROM))
        .map_err(|e| format!("{} ausente ({e}); rode scripts/fetch-roms.sh", NESTEST_ROM))?;
    let log = std::fs::read_to_string(dir.join(NESTEST_LOG))
        .map_err(|e| format!("{} ausente ({e})", NESTEST_LOG))?;
    let log: Vec<String> = log.lines().map(str::to_string).collect();
    if log.len() < TOTAL_LINES {
        return Err(format!("nestest.log tem {} linhas, esperava {TOTAL_LINES}", log.len()));
    }

    let cart = Cartridge::from_bytes(&rom).map_err(|e| e.to_string())?;
    let mut bus = Bus::new(cart);
    let mut cpu = Cpu6502::new();
    cpu.reset(&mut bus); // 7 ciclos, como o CYC:7 da primeira linha do log
    cpu.pc = 0xC000;
    cpu.status = 0x24;
    cpu.stkp = 0xFD;

    let mut first_bad = None;
    for (i, expected) in log.iter().enumerate().take(TOTAL_LINES) {
        let ours =
            St { pc: cpu.pc, a: cpu.a, x: cpu.x, y: cpu.y, p: cpu.status, sp: cpu.stkp, cyc: bus.cpu_cycles };
        if ours != St::parse(expected) {
            first_bad = Some((i, ours));
            break;
        }
        cpu.step(&mut bus);
    }
    Ok(NestestResult {
        matched: first_bad.map_or(TOTAL_LINES, |(i, _)| i),
        first_bad,
        result_bytes: (bus.cpu_read_debug(0x02), bus.cpu_read_debug(0x03)),
        log,
    })
}
