use crate::bus::Bus;
use crate::cpu6502::Cpu6502;

pub struct Debugger {
    // CPU instruction coverage
    pub opcode_count: [u64; 256],     // quantas vezes cada opcode foi executado
    pub opcode_names: [&'static str; 256],
    pub unknown_opcodes: Vec<(u8, u16)>, // (opcode, PC) quando bate num opcode desconhecido

    // CPU trace
    pub trace_enabled: bool,
    pub trace_log: Vec<String>,
    pub trace_max: usize,

    // Breakpoints
    pub breakpoints: Vec<u16>,
    pub hit_breakpoint: bool,

    // Memory watch
    pub watches: Vec<(u16, u8)>, // (addr, last_value)

    // Estatísticas
    pub total_instructions: u64,
    pub total_frames: u64,
}

impl Debugger {
    pub fn new() -> Self {
        let mut names = ["???"; 256];

        // Opcodes oficiais do 6502
        names[0x00] = "BRK"; names[0x01] = "ORA izx"; names[0x05] = "ORA zp";
        names[0x06] = "ASL zp"; names[0x08] = "PHP"; names[0x09] = "ORA imm";
        names[0x0A] = "ASL acc"; names[0x0D] = "ORA abs"; names[0x0E] = "ASL abs";
        names[0x10] = "BPL"; names[0x11] = "ORA izy"; names[0x15] = "ORA zpx";
        names[0x16] = "ASL zpx"; names[0x18] = "CLC"; names[0x19] = "ORA aby";
        names[0x1D] = "ORA abx"; names[0x1E] = "ASL abx";
        names[0x20] = "JSR"; names[0x21] = "AND izx"; names[0x24] = "BIT zp";
        names[0x25] = "AND zp"; names[0x26] = "ROL zp"; names[0x28] = "PLP";
        names[0x29] = "AND imm"; names[0x2A] = "ROL acc"; names[0x2C] = "BIT abs";
        names[0x2D] = "AND abs"; names[0x2E] = "ROL abs";
        names[0x30] = "BMI"; names[0x31] = "AND izy"; names[0x35] = "AND zpx";
        names[0x36] = "ROL zpx"; names[0x38] = "SEC"; names[0x39] = "AND aby";
        names[0x3D] = "AND abx"; names[0x3E] = "ROL abx";
        names[0x40] = "RTI"; names[0x41] = "EOR izx"; names[0x45] = "EOR zp";
        names[0x46] = "LSR zp"; names[0x48] = "PHA"; names[0x49] = "EOR imm";
        names[0x4A] = "LSR acc"; names[0x4C] = "JMP abs"; names[0x4D] = "EOR abs";
        names[0x4E] = "LSR abs";
        names[0x50] = "BVC"; names[0x51] = "EOR izy"; names[0x55] = "EOR zpx";
        names[0x56] = "LSR zpx"; names[0x58] = "CLI"; names[0x59] = "EOR aby";
        names[0x5D] = "EOR abx"; names[0x5E] = "LSR abx";
        names[0x60] = "RTS"; names[0x61] = "ADC izx"; names[0x65] = "ADC zp";
        names[0x66] = "ROR zp"; names[0x68] = "PLA"; names[0x69] = "ADC imm";
        names[0x6A] = "ROR acc"; names[0x6C] = "JMP ind"; names[0x6D] = "ADC abs";
        names[0x6E] = "ROR abs";
        names[0x70] = "BVS"; names[0x71] = "ADC izy"; names[0x75] = "ADC zpx";
        names[0x76] = "ROR zpx"; names[0x78] = "SEI"; names[0x79] = "ADC aby";
        names[0x7D] = "ADC abx"; names[0x7E] = "ROR abx";
        names[0x81] = "STA izx"; names[0x84] = "STY zp"; names[0x85] = "STA zp";
        names[0x86] = "STX zp"; names[0x88] = "DEY"; names[0x8A] = "TXA";
        names[0x8C] = "STY abs"; names[0x8D] = "STA abs"; names[0x8E] = "STX abs";
        names[0x90] = "BCC"; names[0x91] = "STA izy"; names[0x94] = "STY zpx";
        names[0x95] = "STA zpx"; names[0x96] = "STX zpy"; names[0x98] = "TYA";
        names[0x99] = "STA aby"; names[0x9A] = "TXS"; names[0x9D] = "STA abx";
        names[0xA0] = "LDY imm"; names[0xA1] = "LDA izx"; names[0xA2] = "LDX imm";
        names[0xA4] = "LDY zp"; names[0xA5] = "LDA zp"; names[0xA6] = "LDX zp";
        names[0xA8] = "TAY"; names[0xA9] = "LDA imm"; names[0xAA] = "TAX";
        names[0xAC] = "LDY abs"; names[0xAD] = "LDA abs"; names[0xAE] = "LDX abs";
        names[0xB0] = "BCS"; names[0xB1] = "LDA izy"; names[0xB4] = "LDY zpx";
        names[0xB5] = "LDA zpx"; names[0xB6] = "LDX zpy"; names[0xB8] = "CLV";
        names[0xB9] = "LDA aby"; names[0xBA] = "TSX"; names[0xBC] = "LDY abx";
        names[0xBD] = "LDA abx"; names[0xBE] = "LDX aby";
        names[0xC0] = "CPY imm"; names[0xC1] = "CMP izx"; names[0xC4] = "CPY zp";
        names[0xC5] = "CMP zp"; names[0xC6] = "DEC zp"; names[0xC8] = "INY";
        names[0xC9] = "CMP imm"; names[0xCA] = "DEX"; names[0xCC] = "CPY abs";
        names[0xCD] = "CMP abs"; names[0xCE] = "DEC abs";
        names[0xD0] = "BNE"; names[0xD1] = "CMP izy"; names[0xD5] = "CMP zpx";
        names[0xD6] = "DEC zpx"; names[0xD8] = "CLD"; names[0xD9] = "CMP aby";
        names[0xDD] = "CMP abx"; names[0xDE] = "DEC abx";
        names[0xE0] = "CPX imm"; names[0xE1] = "SBC izx"; names[0xE4] = "CPX zp";
        names[0xE5] = "SBC zp"; names[0xE6] = "INC zp"; names[0xE8] = "INX";
        names[0xE9] = "SBC imm"; names[0xEA] = "NOP"; names[0xEC] = "CPX abs";
        names[0xED] = "SBC abs"; names[0xEE] = "INC abs";
        names[0xF0] = "BEQ"; names[0xF1] = "SBC izy"; names[0xF5] = "SBC zpx";
        names[0xF6] = "INC zpx"; names[0xF8] = "SED"; names[0xF9] = "SBC aby";
        names[0xFD] = "SBC abx"; names[0xFE] = "INC abx";

        // NOPs ilegais comuns (usados por alguns jogos)
        for &op in &[0x04, 0x44, 0x64, 0x0C, 0x14, 0x34, 0x54, 0x74, 0xD4, 0xF4,
                     0x1A, 0x3A, 0x5A, 0x7A, 0xDA, 0xFA, 0x80,
                     0x1C, 0x3C, 0x5C, 0x7C, 0xDC, 0xFC] {
            if names[op as usize] == "???" {
                names[op as usize] = "NOP*";
            }
        }

        Debugger {
            opcode_count: [0; 256],
            opcode_names: names,
            unknown_opcodes: Vec::new(),
            trace_enabled: false,
            trace_log: Vec::new(),
            trace_max: 1000,
            breakpoints: Vec::new(),
            hit_breakpoint: false,
            watches: Vec::new(),
            total_instructions: 0,
            total_frames: 0,
        }
    }

    // Chamado antes de cada instrução da CPU
    pub fn on_instruction(&mut self, cpu: &Cpu6502, bus: &Bus) {
        let pc = cpu.pc;
        let opcode = bus.cpu_read_debug(pc);
        self.opcode_count[opcode as usize] += 1;
        self.total_instructions += 1;

        // Detectar opcodes desconhecidos
        if self.opcode_names[opcode as usize] == "???" {
            if self.unknown_opcodes.len() < 100 {
                let already = self.unknown_opcodes.iter().any(|(op, _)| *op == opcode);
                if !already {
                    self.unknown_opcodes.push((opcode, pc));
                    eprintln!("[DEBUG] Unknown opcode 0x{:02X} at PC=0x{:04X}", opcode, pc);
                }
            }
        }

        // CPU trace
        if self.trace_enabled {
            let operand1 = bus.cpu_read_debug(pc.wrapping_add(1));
            let operand2 = bus.cpu_read_debug(pc.wrapping_add(2));
            let name = self.opcode_names[opcode as usize];
            let line = format!(
                "{:04X}  {:02X} {:02X} {:02X}  {:<10} A:{:02X} X:{:02X} Y:{:02X} P:{:02X} SP:{:02X}",
                pc, opcode, operand1, operand2, name,
                cpu.a, cpu.x, cpu.y, cpu.status, cpu.stkp
            );
            self.trace_log.push(line);
            if self.trace_log.len() > self.trace_max {
                self.trace_log.remove(0);
            }
        }

        // Breakpoints
        if !self.breakpoints.is_empty() && self.breakpoints.contains(&pc) {
            self.hit_breakpoint = true;
        }

        // Memory watches
        for watch in &mut self.watches {
            let val = bus.cpu_read_debug(watch.0);
            if val != watch.1 {
                eprintln!("[WATCH] ${:04X}: {:02X} -> {:02X}", watch.0, watch.1, val);
                watch.1 = val;
            }
        }
    }

    // Relatório de coverage
    pub fn coverage_report(&self) -> String {
        let mut report = String::new();
        let total_official = self.opcode_names.iter().filter(|n| **n != "???" && **n != "NOP*").count();
        let used_official = self.opcode_count.iter().enumerate()
            .filter(|(i, c)| **c > 0 && self.opcode_names[*i] != "???" && self.opcode_names[*i] != "NOP*")
            .count();

        report.push_str(&format!("=== CPU Coverage: {}/{} opcodes usados ===\n", used_official, total_official));
        report.push_str(&format!("Total instrucoes: {}\n\n", self.total_instructions));

        // Opcodes nunca executados
        report.push_str("Opcodes oficiais NAO usados:\n");
        for (i, name) in self.opcode_names.iter().enumerate() {
            if *name != "???" && *name != "NOP*" && self.opcode_count[i] == 0 {
                report.push_str(&format!("  0x{:02X} {}\n", i, name));
            }
        }

        // Opcodes desconhecidos encontrados
        if !self.unknown_opcodes.is_empty() {
            report.push_str(&format!("\nOpcodes DESCONHECIDOS encontrados ({}):\n", self.unknown_opcodes.len()));
            for (op, pc) in &self.unknown_opcodes {
                report.push_str(&format!("  0x{:02X} at PC=0x{:04X}\n", op, pc));
            }
        }

        // Top 10 opcodes mais usados
        report.push_str("\nTop 10 opcodes mais executados:\n");
        let mut sorted: Vec<(usize, u64)> = self.opcode_count.iter().enumerate()
            .filter(|(_, c)| **c > 0)
            .map(|(i, c)| (i, *c))
            .collect();
        sorted.sort_by(|a, b| b.1.cmp(&a.1));
        for (i, (op, count)) in sorted.iter().take(10).enumerate() {
            report.push_str(&format!("  {}. 0x{:02X} {} = {} vezes\n", i + 1, op, self.opcode_names[*op], count));
        }

        report
    }

    // Dump de nametable como texto
    pub fn dump_nametable(&self, bus: &Bus, nt: usize) -> Vec<u8> {
        let mut data = vec![0u8; 960];
        for i in 0..960 {
            data[i] = bus.ppu.nametable[nt][i];
        }
        data
    }

    // Dump de paleta
    pub fn dump_palette(&self, bus: &Bus) -> [u8; 32] {
        bus.ppu.palette_table
    }

    // Dump de OAM
    pub fn dump_oam(&self, bus: &Bus) -> Vec<(u8, u8, u8, u8)> {
        let mut sprites = Vec::with_capacity(64);
        for i in 0..64 {
            let y = bus.ppu.oam[i * 4];
            let tile = bus.ppu.oam[i * 4 + 1];
            let attr = bus.ppu.oam[i * 4 + 2];
            let x = bus.ppu.oam[i * 4 + 3];
            sprites.push((y, tile, attr, x));
        }
        sprites
    }

    // Verifica se a CPU está presa num loop
    pub fn detect_stuck(&self, cpu: &Cpu6502, bus: &Bus) -> Option<String> {
        // Ler as próximas instruções e ver se é um loop de 2-3 bytes
        let pc = cpu.pc;
        let op = bus.cpu_read_debug(pc);

        // Detectar JMP pra si mesmo
        if op == 0x4C { // JMP abs
            let lo = bus.cpu_read_debug(pc.wrapping_add(1)) as u16;
            let hi = bus.cpu_read_debug(pc.wrapping_add(2)) as u16;
            let target = (hi << 8) | lo;
            if target == pc {
                return Some(format!("CPU stuck: JMP to self at ${:04X}", pc));
            }
        }

        // Detectar branch loop infinito (2 bytes que volta pra PC)
        if matches!(op, 0x10 | 0x30 | 0x50 | 0x70 | 0x90 | 0xB0 | 0xD0 | 0xF0) {
            let offset = bus.cpu_read_debug(pc.wrapping_add(1)) as i8;
            let target = pc.wrapping_add(2).wrapping_add(offset as u16);
            if target == pc {
                return Some(format!("CPU stuck: branch to self at ${:04X} ({})", pc, self.opcode_names[op as usize]));
            }
        }

        None
    }
}
