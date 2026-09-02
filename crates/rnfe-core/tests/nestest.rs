//! nestest: compara registradores e ciclos, instrução a instrução, com o log de referência.
//!
//! `VERIFIED_LINES` é o progresso executável: sobe conforme os opcodes não-oficiais são
//! implementados (meta: 8991 = o log inteiro). O teste falha tanto se uma linha já
//! verificada divergir quanto se o emulador passar a bater MAIS linhas sem a constante subir.

use rnfe_core::bus::Bus;
use rnfe_core::cpu6502::Cpu6502;
use rnfe_core::testing::{roms_dir, NESTEST_LOG, NESTEST_ROM};
use rnfe_core::Cartridge;

/// Linhas do log que DEVEM bater. Meta: 8991.
const VERIFIED_LINES: usize = 5004;
const TOTAL_LINES: usize = 8991;

#[derive(PartialEq, Eq, Debug, Clone, Copy)]
struct St {
    pc: u16,
    a: u8,
    x: u8,
    y: u8,
    p: u8,
    sp: u8,
    cyc: u64,
}

impl St {
    // "C000  4C F5 C5  JMP $C5F5      A:00 X:00 Y:00 P:24 SP:FD PPU:  0, 21 CYC:7"
    fn parse(line: &str) -> St {
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

    fn fmt(&self) -> String {
        format!(
            "{:04X} A:{:02X} X:{:02X} Y:{:02X} P:{:02X} SP:{:02X} CYC:{}",
            self.pc, self.a, self.x, self.y, self.p, self.sp, self.cyc
        )
    }
}

#[test]
fn nestest_matches_log() {
    let dir = roms_dir();
    let (Ok(rom), Ok(log)) = (
        std::fs::read(dir.join(NESTEST_ROM)),
        std::fs::read_to_string(dir.join(NESTEST_LOG)),
    ) else {
        if std::env::var_os("RNFE_REQUIRE_ROMS").is_some() {
            panic!("nestest.nes/nestest.log ausentes em {}", dir.display());
        }
        eprintln!("SKIP nestest: rode scripts/fetch-roms.sh");
        return;
    };
    let log: Vec<&str> = log.lines().collect();
    assert!(log.len() >= TOTAL_LINES, "nestest.log tem {} linhas", log.len());

    let cart = Cartridge::from_bytes(&rom).unwrap();
    let mut bus = Bus::new(cart);
    let mut cpu = Cpu6502::new();
    cpu.reset(&mut bus);
    // consome os ciclos do reset sem tocar no bus
    while !cpu.is_instruction_start() {
        cpu.clock(&mut bus);
    }
    // modo automático do nestest: começa em $C000 com P=$24, SP=$FD, CYC=7
    cpu.pc = 0xC000;
    cpu.status = 0x24;
    cpu.stkp = 0xFD;
    let mut cyc: u64 = 7;

    let mut first_bad: Option<(usize, St)> = None;
    for (i, expected) in log.iter().enumerate().take(TOTAL_LINES) {
        let ours = St { pc: cpu.pc, a: cpu.a, x: cpu.x, y: cpu.y, p: cpu.status, sp: cpu.stkp, cyc };
        if ours != St::parse(expected) {
            first_bad = Some((i, ours));
            break;
        }
        loop {
            cpu.clock(&mut bus);
            cyc += 1;
            if cpu.is_instruction_start() {
                break;
            }
        }
    }

    let matched = first_bad.map_or(TOTAL_LINES, |(i, _)| i);
    if let Some((i, ours)) = first_bad.filter(|(i, _)| *i < VERIFIED_LINES) {
        let ctx = (i.saturating_sub(3)..i)
            .map(|j| format!("      {:5} {}", j + 1, log[j].split("A:").next().unwrap().trim_end()))
            .collect::<Vec<_>>()
            .join("\n");
        panic!(
            "nestest divergiu na linha {} (verificadas: {VERIFIED_LINES})\n{ctx}\n  log: {}\n  nós: {}\n  instrução: {}",
            i + 1,
            St::parse(log[i]).fmt(),
            ours.fmt(),
            log[i].split("A:").next().unwrap().trim_end()
        );
    }
    assert!(
        matched == VERIFIED_LINES || matched == TOTAL_LINES,
        "nestest agora bate {matched} linhas (> VERIFIED_LINES = {VERIFIED_LINES}) — suba a constante em tests/nestest.rs"
    );
    if matched == TOTAL_LINES {
        assert_eq!(
            (bus.cpu_read_debug(0x02), bus.cpu_read_debug(0x03)),
            (0, 0),
            "nestest reportou erro em $02/$03"
        );
    }
}
