//! Debugger opcional: cobertura de opcodes, trace da CPU e dumps de PPU. Só custa quando
//! `enabled` (ver `Nes::step_instruction`).

use std::collections::VecDeque;

use crate::bus::Bus;
use crate::cpu6502::{Cpu6502, LOOKUP, Mode, Op};

pub struct Debugger {
    /// `on_instruction` só roda quando ligado (custa ~10% do frame).
    pub enabled: bool,
    /// Quantas vezes cada opcode foi executado.
    pub opcode_count: [u64; 256],
    /// (opcode, PC) dos JAMs encontrados (a CPU trava neles).
    pub jams: Vec<(u8, u16)>,
    pub trace_enabled: bool,
    /// Últimas `trace_max` instruções no formato do nestest.
    pub trace_log: VecDeque<String>,
    pub trace_max: usize,
    pub total_instructions: u64,
    pub total_frames: u64,
}

impl Default for Debugger {
    fn default() -> Self {
        Self::new()
    }
}

/// Nome do opcode com o modo de endereçamento (`LDA abx`), como no disassembler.
pub fn opcode_name(op: u8) -> String {
    let (o, m) = LOOKUP[op as usize];
    let mode = match m {
        Mode::Imp => "",
        Mode::Acc => " acc",
        Mode::Imm => " imm",
        Mode::Zp => " zp",
        Mode::ZpX => " zpx",
        Mode::ZpY => " zpy",
        Mode::Abs => " abs",
        Mode::AbsX | Mode::AbsXW => " abx",
        Mode::AbsY | Mode::AbsYW => " aby",
        Mode::Ind => " ind",
        Mode::IndX => " izx",
        Mode::IndY | Mode::IndYW => " izy",
        Mode::Rel => " rel",
    };
    format!("{}{}", o.name(), mode)
}

fn is_official(op: u8) -> bool {
    LOOKUP[op as usize].0.is_official()
}

impl Debugger {
    pub fn new() -> Self {
        Debugger {
            enabled: false,
            opcode_count: [0; 256],
            jams: Vec::new(),
            trace_enabled: false,
            trace_log: VecDeque::new(),
            trace_max: 1000,
            total_instructions: 0,
            total_frames: 0,
        }
    }

    /// Chamado antes de cada instrução da CPU (só com `enabled`).
    pub fn on_instruction(&mut self, cpu: &Cpu6502, bus: &Bus) {
        let pc = cpu.pc;
        let opcode = bus.cpu_read_debug(pc);

        self.opcode_count[opcode as usize] += 1;
        self.total_instructions += 1;

        if LOOKUP[opcode as usize].0 == Op::Jam
            && self.jams.len() < 100
            && !self.jams.iter().any(|(op, _)| *op == opcode)
        {
            self.jams.push((opcode, pc));
            log::warn!("JAM 0x{:02X} em PC=0x{:04X}", opcode, pc);
        }

        if self.trace_enabled {
            let operand1 = bus.cpu_read_debug(pc.wrapping_add(1));
            let operand2 = bus.cpu_read_debug(pc.wrapping_add(2));
            let line = format!(
                "{:04X}  {:02X} {:02X} {:02X}  {:<10} A:{:02X} X:{:02X} Y:{:02X} P:{:02X} SP:{:02X}",
                pc,
                opcode,
                operand1,
                operand2,
                opcode_name(opcode),
                cpu.a,
                cpu.x,
                cpu.y,
                cpu.status,
                cpu.stkp
            );
            if self.trace_log.len() >= self.trace_max {
                self.trace_log.pop_front();
            }
            self.trace_log.push_back(line);
        }
    }

    /// Relatório de cobertura dos opcodes oficiais.
    pub fn coverage_report(&self) -> String {
        use std::fmt::Write;
        let mut report = String::new();
        let official: Vec<u8> = (0..=255u8).filter(|&op| is_official(op)).collect();
        let used = official.iter().filter(|&&op| self.opcode_count[op as usize] > 0).count();
        let _ = writeln!(report, "=== CPU Coverage: {}/{} opcodes usados ===", used, official.len());
        let _ = writeln!(report, "Total instrucoes: {}\n", self.total_instructions);

        let _ = writeln!(report, "Opcodes oficiais NAO usados:");
        for &op in &official {
            if self.opcode_count[op as usize] == 0 {
                let _ = writeln!(report, "  0x{:02X} {}", op, opcode_name(op));
            }
        }

        if !self.jams.is_empty() {
            let _ = writeln!(report, "\nJAMs encontrados ({}):", self.jams.len());
            for (op, pc) in &self.jams {
                let _ = writeln!(report, "  0x{:02X} em PC=0x{:04X}", op, pc);
            }
        }

        let _ = writeln!(report, "\nTop 10 opcodes mais executados:");
        let mut sorted: Vec<(usize, u64)> =
            self.opcode_count.iter().enumerate().filter(|(_, c)| **c > 0).map(|(i, c)| (i, *c)).collect();
        sorted.sort_by_key(|e| std::cmp::Reverse(e.1));
        for (i, (op, count)) in sorted.iter().take(10).enumerate() {
            let _ =
                writeln!(report, "  {}. 0x{:02X} {} = {} vezes", i + 1, op, opcode_name(*op as u8), count);
        }
        report
    }

    /// Nametable `nt` (0–3) como bytes de tile (sem a tabela de atributos).
    pub fn dump_nametable(&self, bus: &Bus, nt: usize) -> Vec<u8> {
        bus.ppu.nametable[nt & 3][..960].to_vec()
    }

    pub fn dump_palette(&self, bus: &Bus) -> [u8; 32] {
        bus.ppu.palette_table
    }

    /// OAM como (y, tile, atributos, x) por sprite.
    pub fn dump_oam(&self, bus: &Bus) -> Vec<(u8, u8, u8, u8)> {
        bus.ppu.oam.chunks_exact(4).map(|s| (s[0], s[1], s[2], s[3])).collect()
    }

    /// Detecta a CPU presa num `JMP`/branch para si mesma.
    pub fn detect_stuck(&self, cpu: &Cpu6502, bus: &Bus) -> Option<String> {
        let pc = cpu.pc;
        let op = bus.cpu_read_debug(pc);
        if op == 0x4C {
            let lo = bus.cpu_read_debug(pc.wrapping_add(1)) as u16;
            let hi = bus.cpu_read_debug(pc.wrapping_add(2)) as u16;
            if (hi << 8) | lo == pc {
                return Some(format!("CPU stuck: JMP to self at ${:04X}", pc));
            }
        }
        if matches!(op, 0x10 | 0x30 | 0x50 | 0x70 | 0x90 | 0xB0 | 0xD0 | 0xF0) {
            let offset = bus.cpu_read_debug(pc.wrapping_add(1)) as i8;
            if pc.wrapping_add(2).wrapping_add(offset as u16) == pc {
                return Some(format!("CPU stuck: branch to self at ${:04X} ({})", pc, opcode_name(op)));
            }
        }
        None
    }
}
