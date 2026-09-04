//! MOS 6502 do Ricoh 2A03 (sem modo decimal), emulado **por acesso ao barramento**.
//!
//! Cada leitura ou escrita da CPU custa exatamente um ciclo: `Bus::tick` avança PPU (3 dots) e
//! APU em volta do acesso, então tudo fica em lock-step sem contador de ciclos por instrução.
//! As leituras "inúteis" do hardware (dummy reads) são feitas nos mesmos endereços que o chip
//! real usa, porque registradores como `$2002`/`$2007`/`$4015` têm efeito colateral ao serem lidos.
//!
//! Interrupções seguem o modelo do nesdev: a linha NMI passa por um detector de borda e a IRQ é
//! amostrada por nível ao fim de cada ciclo; a decisão de atender usa o estado do **penúltimo**
//! ciclo da instrução. NMI pode "sequestrar" um BRK/IRQ já em andamento.

use crate::bus::Bus;

// Bits de P
pub const C: u8 = 1 << 0;
pub const Z: u8 = 1 << 1;
pub const I: u8 = 1 << 2;
pub const D: u8 = 1 << 3;
pub const B: u8 = 1 << 4;
pub const U: u8 = 1 << 5;
pub const V: u8 = 1 << 6;
pub const N: u8 = 1 << 7;

const NMI_VECTOR: u16 = 0xFFFA;
const RESET_VECTOR: u16 = 0xFFFC;
const IRQ_VECTOR: u16 = 0xFFFE;

/// Modo de endereçamento. Os sufixos `W` marcam instruções de escrita/RMW nos modos indexados:
/// nelas a leitura no endereço "não corrigido" acontece sempre, não só ao cruzar página.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Mode {
    Imp,
    Acc,
    Imm,
    Zp,
    ZpX,
    ZpY,
    Abs,
    AbsX,
    AbsXW,
    AbsY,
    AbsYW,
    Ind,
    IndX,
    IndY,
    IndYW,
    Rel,
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
#[rustfmt::skip]
pub enum Op {
    // oficiais
    Adc, And, Asl, Bcc, Bcs, Beq, Bit, Bmi, Bne, Bpl, Brk, Bvc, Bvs, Clc, Cld, Cli, Clv, Cmp, Cpx,
    Cpy, Dec, Dex, Dey, Eor, Inc, Inx, Iny, Jmp, Jsr, Lda, Ldx, Ldy, Lsr, Nop, Ora, Pha, Php, Pla,
    Plp, Rol, Ror, Rti, Rts, Sbc, Sec, Sed, Sei, Sta, Stx, Sty, Tax, Tay, Tsx, Txa, Txs, Tya,
    // não-oficiais
    Lax, Sax, Dcp, Isb, Slo, Rla, Sre, Rra, Anc, Alr, Arr, Xaa, Axs, Las, Sha, Shx, Shy, Tas, Jam,
}

impl Op {
    pub fn name(self) -> &'static str {
        use Op::*;
        match self {
            Adc => "ADC",
            And => "AND",
            Asl => "ASL",
            Bcc => "BCC",
            Bcs => "BCS",
            Beq => "BEQ",
            Bit => "BIT",
            Bmi => "BMI",
            Bne => "BNE",
            Bpl => "BPL",
            Brk => "BRK",
            Bvc => "BVC",
            Bvs => "BVS",
            Clc => "CLC",
            Cld => "CLD",
            Cli => "CLI",
            Clv => "CLV",
            Cmp => "CMP",
            Cpx => "CPX",
            Cpy => "CPY",
            Dec => "DEC",
            Dex => "DEX",
            Dey => "DEY",
            Eor => "EOR",
            Inc => "INC",
            Inx => "INX",
            Iny => "INY",
            Jmp => "JMP",
            Jsr => "JSR",
            Lda => "LDA",
            Ldx => "LDX",
            Ldy => "LDY",
            Lsr => "LSR",
            Nop => "NOP",
            Ora => "ORA",
            Pha => "PHA",
            Php => "PHP",
            Pla => "PLA",
            Plp => "PLP",
            Rol => "ROL",
            Ror => "ROR",
            Rti => "RTI",
            Rts => "RTS",
            Sbc => "SBC",
            Sec => "SEC",
            Sed => "SED",
            Sei => "SEI",
            Sta => "STA",
            Stx => "STX",
            Sty => "STY",
            Tax => "TAX",
            Tay => "TAY",
            Tsx => "TSX",
            Txa => "TXA",
            Txs => "TXS",
            Tya => "TYA",
            Lax => "LAX",
            Sax => "SAX",
            Dcp => "DCP",
            Isb => "ISB",
            Slo => "SLO",
            Rla => "RLA",
            Sre => "SRE",
            Rra => "RRA",
            Anc => "ANC",
            Alr => "ALR",
            Arr => "ARR",
            Xaa => "XAA",
            Axs => "AXS",
            Las => "LAS",
            Sha => "SHA",
            Shx => "SHX",
            Shy => "SHY",
            Tas => "TAS",
            Jam => "JAM",
        }
    }

    /// Opcode documentado pela MOS (os demais são "ilegais").
    pub fn is_official(self) -> bool {
        (self as u8) <= (Op::Tya as u8)
    }
}

/// Tabela de decodificação: (operação, modo) por opcode.
#[rustfmt::skip]
pub static LOOKUP: [(Op, Mode); 256] = {
    use Mode::*;
    use Op::*;
    [
    // 0x00
    (Brk,Imm),(Ora,IndX),(Jam,Imp),(Slo,IndX),(Nop,Zp),(Ora,Zp),(Asl,Zp),(Slo,Zp),
    (Php,Imp),(Ora,Imm),(Asl,Acc),(Anc,Imm),(Nop,Abs),(Ora,Abs),(Asl,Abs),(Slo,Abs),
    // 0x10
    (Bpl,Rel),(Ora,IndY),(Jam,Imp),(Slo,IndYW),(Nop,ZpX),(Ora,ZpX),(Asl,ZpX),(Slo,ZpX),
    (Clc,Imp),(Ora,AbsY),(Nop,Imp),(Slo,AbsYW),(Nop,AbsX),(Ora,AbsX),(Asl,AbsXW),(Slo,AbsXW),
    // 0x20
    (Jsr,Abs),(And,IndX),(Jam,Imp),(Rla,IndX),(Bit,Zp),(And,Zp),(Rol,Zp),(Rla,Zp),
    (Plp,Imp),(And,Imm),(Rol,Acc),(Anc,Imm),(Bit,Abs),(And,Abs),(Rol,Abs),(Rla,Abs),
    // 0x30
    (Bmi,Rel),(And,IndY),(Jam,Imp),(Rla,IndYW),(Nop,ZpX),(And,ZpX),(Rol,ZpX),(Rla,ZpX),
    (Sec,Imp),(And,AbsY),(Nop,Imp),(Rla,AbsYW),(Nop,AbsX),(And,AbsX),(Rol,AbsXW),(Rla,AbsXW),
    // 0x40
    (Rti,Imp),(Eor,IndX),(Jam,Imp),(Sre,IndX),(Nop,Zp),(Eor,Zp),(Lsr,Zp),(Sre,Zp),
    (Pha,Imp),(Eor,Imm),(Lsr,Acc),(Alr,Imm),(Jmp,Abs),(Eor,Abs),(Lsr,Abs),(Sre,Abs),
    // 0x50
    (Bvc,Rel),(Eor,IndY),(Jam,Imp),(Sre,IndYW),(Nop,ZpX),(Eor,ZpX),(Lsr,ZpX),(Sre,ZpX),
    (Cli,Imp),(Eor,AbsY),(Nop,Imp),(Sre,AbsYW),(Nop,AbsX),(Eor,AbsX),(Lsr,AbsXW),(Sre,AbsXW),
    // 0x60
    (Rts,Imp),(Adc,IndX),(Jam,Imp),(Rra,IndX),(Nop,Zp),(Adc,Zp),(Ror,Zp),(Rra,Zp),
    (Pla,Imp),(Adc,Imm),(Ror,Acc),(Arr,Imm),(Jmp,Ind),(Adc,Abs),(Ror,Abs),(Rra,Abs),
    // 0x70
    (Bvs,Rel),(Adc,IndY),(Jam,Imp),(Rra,IndYW),(Nop,ZpX),(Adc,ZpX),(Ror,ZpX),(Rra,ZpX),
    (Sei,Imp),(Adc,AbsY),(Nop,Imp),(Rra,AbsYW),(Nop,AbsX),(Adc,AbsX),(Ror,AbsXW),(Rra,AbsXW),
    // 0x80
    (Nop,Imm),(Sta,IndX),(Nop,Imm),(Sax,IndX),(Sty,Zp),(Sta,Zp),(Stx,Zp),(Sax,Zp),
    (Dey,Imp),(Nop,Imm),(Txa,Imp),(Xaa,Imm),(Sty,Abs),(Sta,Abs),(Stx,Abs),(Sax,Abs),
    // 0x90
    (Bcc,Rel),(Sta,IndYW),(Jam,Imp),(Sha,IndYW),(Sty,ZpX),(Sta,ZpX),(Stx,ZpY),(Sax,ZpY),
    (Tya,Imp),(Sta,AbsYW),(Txs,Imp),(Tas,AbsYW),(Shy,AbsXW),(Sta,AbsXW),(Shx,AbsYW),(Sha,AbsYW),
    // 0xA0
    (Ldy,Imm),(Lda,IndX),(Ldx,Imm),(Lax,IndX),(Ldy,Zp),(Lda,Zp),(Ldx,Zp),(Lax,Zp),
    (Tay,Imp),(Lda,Imm),(Tax,Imp),(Lax,Imm),(Ldy,Abs),(Lda,Abs),(Ldx,Abs),(Lax,Abs),
    // 0xB0
    (Bcs,Rel),(Lda,IndY),(Jam,Imp),(Lax,IndY),(Ldy,ZpX),(Lda,ZpX),(Ldx,ZpY),(Lax,ZpY),
    (Clv,Imp),(Lda,AbsY),(Tsx,Imp),(Las,AbsY),(Ldy,AbsX),(Lda,AbsX),(Ldx,AbsY),(Lax,AbsY),
    // 0xC0
    (Cpy,Imm),(Cmp,IndX),(Nop,Imm),(Dcp,IndX),(Cpy,Zp),(Cmp,Zp),(Dec,Zp),(Dcp,Zp),
    (Iny,Imp),(Cmp,Imm),(Dex,Imp),(Axs,Imm),(Cpy,Abs),(Cmp,Abs),(Dec,Abs),(Dcp,Abs),
    // 0xD0
    (Bne,Rel),(Cmp,IndY),(Jam,Imp),(Dcp,IndYW),(Nop,ZpX),(Cmp,ZpX),(Dec,ZpX),(Dcp,ZpX),
    (Cld,Imp),(Cmp,AbsY),(Nop,Imp),(Dcp,AbsYW),(Nop,AbsX),(Cmp,AbsX),(Dec,AbsXW),(Dcp,AbsXW),
    // 0xE0
    (Cpx,Imm),(Sbc,IndX),(Nop,Imm),(Isb,IndX),(Cpx,Zp),(Sbc,Zp),(Inc,Zp),(Isb,Zp),
    (Inx,Imp),(Sbc,Imm),(Nop,Imp),(Sbc,Imm),(Cpx,Abs),(Sbc,Abs),(Inc,Abs),(Isb,Abs),
    // 0xF0
    (Beq,Rel),(Sbc,IndY),(Jam,Imp),(Isb,IndYW),(Nop,ZpX),(Sbc,ZpX),(Inc,ZpX),(Isb,ZpX),
    (Sed,Imp),(Sbc,AbsY),(Nop,Imp),(Isb,AbsYW),(Nop,AbsX),(Sbc,AbsX),(Inc,AbsXW),(Isb,AbsXW),
    ]
};

#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[derive(Debug, Clone)]
pub struct Cpu6502 {
    pub a: u8,
    pub x: u8,
    pub y: u8,
    pub stkp: u8,
    pub pc: u16,
    pub status: u8,
    /// Opcode JAM executado: a CPU para até o reset.
    pub jammed: bool,
    /// Último opcode buscado (diagnóstico).
    pub opcode: u8,

    // Endereçamento indexado: byte alto ANTES da indexação e se cruzou página (SHA/SHX/SHY/TAS).
    base_hi: u8,
    crossed: bool,

    // Polling de interrupções (ver doc do módulo)
    nmi_level: bool,
    nmi_pending: bool,
    nmi_pending_prev: bool,
    irq_run: bool,
    irq_run_prev: bool,
}

impl Default for Cpu6502 {
    fn default() -> Self {
        Self::new()
    }
}

impl Cpu6502 {
    /// Estado de power-on. O `reset` (7 ciclos) leva SP a `$FD` e I=1.
    pub fn new() -> Self {
        Cpu6502 {
            a: 0,
            x: 0,
            y: 0,
            stkp: 0x00,
            pc: 0,
            status: I | U,
            jammed: false,
            opcode: 0,
            base_hi: 0,
            crossed: false,
            nmi_level: false,
            nmi_pending: false,
            nmi_pending_prev: false,
            irq_run: false,
            irq_run_prev: false,
        }
    }

    // ------------------------------------------------------------------ flags

    #[inline]
    pub fn flag(&self, f: u8) -> bool {
        self.status & f != 0
    }

    #[inline]
    pub fn set_flag(&mut self, f: u8, on: bool) {
        if on {
            self.status |= f;
        } else {
            self.status &= !f;
        }
    }

    #[inline]
    fn set_zn(&mut self, v: u8) {
        self.status = (self.status & !(Z | N)) | (v & N) | if v == 0 { Z } else { 0 };
    }

    // ------------------------------------------------------------- ciclos/bus

    /// Amostra as linhas de interrupção ao fim de um ciclo.
    #[inline]
    fn poll_interrupts(&mut self, bus: &Bus) {
        self.nmi_pending_prev = self.nmi_pending;
        let nmi = bus.nmi_line();
        if nmi && !self.nmi_level {
            self.nmi_pending = true;
        }
        self.nmi_level = nmi;
        self.irq_run_prev = self.irq_run;
        self.irq_run = bus.irq_line() && !self.flag(I);
    }

    /// Um ciclo de leitura. Se há OAM DMA pendente, a CPU é parada aqui (o DMA só começa num
    /// ciclo de leitura) e o acesso pedido acontece depois.
    #[inline]
    fn read(&mut self, bus: &mut Bus, addr: u16) -> u8 {
        if let Some(page) = bus.take_oam_dma() {
            self.oam_dma(bus, page, addr);
        }
        if bus.take_dmc_dma() {
            self.dmc_dma(bus, addr);
        }
        bus.tick_pre();
        let v = bus.read_raw(addr);
        bus.tick_post();
        self.poll_interrupts(bus);
        v
    }

    /// Um ciclo de escrita.
    #[inline]
    fn write(&mut self, bus: &mut Bus, addr: u16, v: u8) {
        bus.tick_pre();
        bus.write_raw(addr, v);
        bus.tick_post();
        self.poll_interrupts(bus);
    }

    /// Ciclo de leitura dentro do DMA (nunca reentra no DMA).
    #[inline]
    fn dma_read(&mut self, bus: &mut Bus, addr: u16) -> u8 {
        bus.tick_pre();
        let v = bus.read_raw(addr);
        bus.tick_post();
        self.poll_interrupts(bus);
        v
    }

    /// OAM DMA (`$4014`): 1 ciclo de parada (+1 de alinhamento se o ciclo é ímpar) e 256 pares
    /// leitura/escrita em `$2004` — 513 ou 514 ciclos. `addr` é o endereço que a CPU ia ler.
    fn oam_dma(&mut self, bus: &mut Bus, page: u8, addr: u16) {
        self.dma_read(bus, addr);
        if bus.cpu_cycles & 1 == 1 {
            self.dma_read(bus, addr);
        }
        let base = (page as u16) << 8;
        for i in 0..256u16 {
            // O DMC interrompe o DMA de OAM (na taxa mais alta o buffer esvaziaria antes)
            if bus.take_dmc_dma() {
                self.dma_read(bus, base | i);
                let v = self.dma_read(bus, bus.dmc_address());
                bus.dmc_feed(v);
            }
            let v = self.dma_read(bus, base | i);
            self.write(bus, 0x2004, v);
        }
    }

    /// DMA do DMC: parada (1) + dummy (1) + alinhamento (0–1) + leitura do byte. Os ciclos
    /// parados repetem a leitura de `addr` — é o que duplica leituras de `$2007`/`$4016`.
    fn dmc_dma(&mut self, bus: &mut Bus, addr: u16) {
        // Nas portas de controle ($4016/$4017) só o ciclo de parada repete a leitura
        // (hardware: 1 leitura extra); em $2007 todos os ciclos parados leem (2–3 extras).
        let repeat = !matches!(addr, 0x4016 | 0x4017);
        self.dma_read(bus, addr);
        self.dma_idle(bus, addr, repeat);
        if bus.cpu_cycles & 1 == 1 {
            self.dma_idle(bus, addr, repeat);
        }
        let v = self.dma_read(bus, bus.dmc_address());
        bus.dmc_feed(v);
    }

    /// Ciclo parado do DMA: repete a leitura de `addr` ou só deixa o relógio andar.
    #[inline]
    fn dma_idle(&mut self, bus: &mut Bus, addr: u16, repeat_read: bool) {
        if repeat_read {
            self.dma_read(bus, addr);
        } else {
            bus.tick_pre();
            bus.tick_post();
            self.poll_interrupts(bus);
        }
    }

    #[inline]
    fn fetch_byte(&mut self, bus: &mut Bus) -> u8 {
        let v = self.read(bus, self.pc);
        self.pc = self.pc.wrapping_add(1);
        v
    }

    #[inline]
    fn fetch_word(&mut self, bus: &mut Bus) -> u16 {
        let lo = self.fetch_byte(bus) as u16;
        let hi = self.fetch_byte(bus) as u16;
        (hi << 8) | lo
    }

    #[inline]
    fn push(&mut self, bus: &mut Bus, v: u8) {
        self.write(bus, 0x0100 | self.stkp as u16, v);
        self.stkp = self.stkp.wrapping_sub(1);
    }

    #[inline]
    fn pull(&mut self, bus: &mut Bus) -> u8 {
        self.stkp = self.stkp.wrapping_add(1);
        self.read(bus, 0x0100 | self.stkp as u16)
    }

    /// Leitura descartada no topo da pilha (ciclo interno de PLA/RTS/RTI/JSR).
    #[inline]
    fn dummy_stack_read(&mut self, bus: &mut Bus) {
        self.read(bus, 0x0100 | self.stkp as u16);
    }

    // ----------------------------------------------------------- endereçamento

    /// Calcula o endereço efetivo, gastando os ciclos (e dummy reads) do modo.
    #[inline]
    fn operand(&mut self, bus: &mut Bus, mode: Mode) -> u16 {
        self.crossed = false;
        match mode {
            Mode::Imp | Mode::Acc => {
                self.read(bus, self.pc);
                0
            }
            Mode::Imm => {
                let a = self.pc;
                self.pc = self.pc.wrapping_add(1);
                a
            }
            Mode::Zp => self.fetch_byte(bus) as u16,
            Mode::ZpX => {
                let z = self.fetch_byte(bus);
                self.read(bus, z as u16);
                z.wrapping_add(self.x) as u16
            }
            Mode::ZpY => {
                let z = self.fetch_byte(bus);
                self.read(bus, z as u16);
                z.wrapping_add(self.y) as u16
            }
            Mode::Abs => self.fetch_word(bus),
            Mode::AbsX => {
                let base = self.fetch_word(bus);
                self.indexed(bus, base, self.x, false)
            }
            Mode::AbsXW => {
                let base = self.fetch_word(bus);
                self.indexed(bus, base, self.x, true)
            }
            Mode::AbsY => {
                let base = self.fetch_word(bus);
                self.indexed(bus, base, self.y, false)
            }
            Mode::AbsYW => {
                let base = self.fetch_word(bus);
                self.indexed(bus, base, self.y, true)
            }
            Mode::Ind => {
                let ptr = self.fetch_word(bus);
                let lo = self.read(bus, ptr) as u16;
                // bug do 6502: o byte alto não cruza a página
                let hi = self.read(bus, (ptr & 0xFF00) | (ptr.wrapping_add(1) & 0x00FF)) as u16;
                (hi << 8) | lo
            }
            Mode::IndX => {
                let z = self.fetch_byte(bus);
                self.read(bus, z as u16);
                let p = z.wrapping_add(self.x);
                let lo = self.read(bus, p as u16) as u16;
                let hi = self.read(bus, p.wrapping_add(1) as u16) as u16;
                (hi << 8) | lo
            }
            Mode::IndY | Mode::IndYW => {
                let z = self.fetch_byte(bus);
                let lo = self.read(bus, z as u16) as u16;
                let hi = self.read(bus, z.wrapping_add(1) as u16) as u16;
                self.indexed(bus, (hi << 8) | lo, self.y, mode == Mode::IndYW)
            }
            Mode::Rel => {
                let off = self.fetch_byte(bus) as i8;
                self.pc.wrapping_add(off as i16 as u16)
            }
        }
    }

    /// Indexação com a leitura no endereço "sem carry": sempre para escrita/RMW, só ao cruzar
    /// página para leitura.
    #[inline]
    fn indexed(&mut self, bus: &mut Bus, base: u16, index: u8, always_dummy: bool) -> u16 {
        let addr = base.wrapping_add(index as u16);
        self.base_hi = (base >> 8) as u8;
        self.crossed = (addr & 0xFF00) != (base & 0xFF00);
        if self.crossed || always_dummy {
            self.read(bus, (base & 0xFF00) | (addr & 0x00FF));
        }
        addr
    }

    // ----------------------------------------------------------------- execução

    /// Executa uma instrução completa (ou o atendimento de uma interrupção pendente ao fim dela).
    pub fn step(&mut self, bus: &mut Bus) {
        if self.jammed {
            bus.tick_pre();
            bus.tick_post();
            return;
        }
        let opcode = self.fetch_byte(bus);
        self.opcode = opcode;
        let (op, mode) = LOOKUP[opcode as usize];
        let addr = self.operand(bus, mode);
        self.execute(bus, op, mode, addr);

        if self.nmi_pending_prev || self.irq_run_prev {
            self.interrupt(bus);
        }
    }

    /// Sequência de 7 ciclos de IRQ/NMI. Se a NMI ficou pendente durante a sequência, ela
    /// "sequestra" o vetor.
    fn interrupt(&mut self, bus: &mut Bus) {
        self.read(bus, self.pc);
        self.read(bus, self.pc);
        self.interrupt_sequence(bus, false);
    }

    /// Ciclos 3–7 de BRK/IRQ/NMI: empilha PC e P (bit B só no BRK), seta I e busca o vetor.
    /// Um NMI pendente "sequestra" a sequência (vetor NMI no lugar do IRQ/BRK).
    /// Sem polling no fim: a 1ª instrução do handler sempre roda.
    fn interrupt_sequence(&mut self, bus: &mut Bus, brk: bool) {
        self.push(bus, (self.pc >> 8) as u8);
        self.push(bus, self.pc as u8);
        let nmi = self.nmi_pending;
        let p = if brk { self.status | B } else { self.status & !B };
        self.push(bus, p | U);
        self.status |= I;
        let vec = if nmi {
            self.nmi_pending = false;
            NMI_VECTOR
        } else {
            IRQ_VECTOR
        };
        let lo = self.read(bus, vec) as u16;
        let hi = self.read(bus, vec + 1) as u16;
        self.pc = (hi << 8) | lo;
        self.nmi_pending_prev = false;
        self.irq_run_prev = false;
    }

    #[inline]
    fn branch(&mut self, bus: &mut Bus, cond: bool, target: u16) {
        if !cond {
            return;
        }
        // Um branch tomado sem cruzar página não vê uma IRQ que subiu durante o ciclo do operando:
        // a instrução seguinte executa antes dela.
        if self.irq_run && !self.irq_run_prev {
            self.irq_run = false;
        }
        self.read(bus, self.pc);
        if (target & 0xFF00) != (self.pc & 0xFF00) {
            self.read(bus, (self.pc & 0xFF00) | (target & 0x00FF));
        }
        self.pc = target;
    }

    /// Read-modify-write: lê, reescreve o valor antigo, escreve o novo (ou opera em A).
    #[inline]
    fn rmw<F: FnOnce(&mut Self, u8) -> u8>(&mut self, bus: &mut Bus, mode: Mode, addr: u16, f: F) {
        if mode == Mode::Acc {
            self.a = f(self, self.a);
        } else {
            let v = self.read(bus, addr);
            self.write(bus, addr, v);
            let r = f(self, v);
            self.write(bus, addr, r);
        }
    }

    #[inline]
    fn adc(&mut self, m: u8) {
        let sum = self.a as u16 + m as u16 + self.flag(C) as u16;
        let r = sum as u8;
        self.set_flag(C, sum > 0xFF);
        self.set_flag(V, (!(self.a ^ m) & (self.a ^ r) & 0x80) != 0);
        self.a = r;
        self.set_zn(r);
    }

    #[inline]
    fn compare(&mut self, reg: u8, m: u8) {
        self.set_flag(C, reg >= m);
        self.set_zn(reg.wrapping_sub(m));
    }

    #[inline]
    fn asl(&mut self, v: u8) -> u8 {
        self.set_flag(C, v & 0x80 != 0);
        let r = v << 1;
        self.set_zn(r);
        r
    }

    #[inline]
    fn lsr(&mut self, v: u8) -> u8 {
        self.set_flag(C, v & 0x01 != 0);
        let r = v >> 1;
        self.set_zn(r);
        r
    }

    #[inline]
    fn rol(&mut self, v: u8) -> u8 {
        let r = (v << 1) | self.flag(C) as u8;
        self.set_flag(C, v & 0x80 != 0);
        self.set_zn(r);
        r
    }

    #[inline]
    fn ror(&mut self, v: u8) -> u8 {
        let r = (v >> 1) | ((self.flag(C) as u8) << 7);
        self.set_flag(C, v & 0x01 != 0);
        self.set_zn(r);
        r
    }

    /// Escrita `val & (H+1)` de SHA/SHX/SHY/TAS, com o byte alto corrompido ao cruzar página.
    fn store_high_and(&mut self, bus: &mut Bus, addr: u16, val: u8) {
        let v = val & self.base_hi.wrapping_add(1);
        let addr = if self.crossed { ((v as u16) << 8) | (addr & 0x00FF) } else { addr };
        self.write(bus, addr, v);
    }

    fn execute(&mut self, bus: &mut Bus, op: Op, mode: Mode, addr: u16) {
        use Op::*;
        match op {
            // ---- cargas e armazenamentos
            Lda => {
                let v = self.read(bus, addr);
                self.a = v;
                self.set_zn(v);
            }
            Ldx => {
                let v = self.read(bus, addr);
                self.x = v;
                self.set_zn(v);
            }
            Ldy => {
                let v = self.read(bus, addr);
                self.y = v;
                self.set_zn(v);
            }
            Sta => self.write(bus, addr, self.a),
            Stx => self.write(bus, addr, self.x),
            Sty => self.write(bus, addr, self.y),

            // ---- transferências
            Tax => {
                self.x = self.a;
                self.set_zn(self.x);
            }
            Tay => {
                self.y = self.a;
                self.set_zn(self.y);
            }
            Txa => {
                self.a = self.x;
                self.set_zn(self.a);
            }
            Tya => {
                self.a = self.y;
                self.set_zn(self.a);
            }
            Tsx => {
                self.x = self.stkp;
                self.set_zn(self.x);
            }
            Txs => self.stkp = self.x,

            // ---- pilha
            Pha => self.push(bus, self.a),
            Php => self.push(bus, self.status | B | U),
            Pla => {
                self.dummy_stack_read(bus);
                self.a = self.pull(bus);
                self.set_zn(self.a);
            }
            Plp => {
                self.dummy_stack_read(bus);
                self.status = (self.pull(bus) & !B) | U;
            }

            // ---- aritmética e lógica
            Adc => {
                let m = self.read(bus, addr);
                self.adc(m);
            }
            Sbc => {
                let m = self.read(bus, addr);
                self.adc(!m);
            }
            And => {
                self.a &= self.read(bus, addr);
                self.set_zn(self.a);
            }
            Ora => {
                self.a |= self.read(bus, addr);
                self.set_zn(self.a);
            }
            Eor => {
                self.a ^= self.read(bus, addr);
                self.set_zn(self.a);
            }
            Cmp => {
                let m = self.read(bus, addr);
                self.compare(self.a, m);
            }
            Cpx => {
                let m = self.read(bus, addr);
                self.compare(self.x, m);
            }
            Cpy => {
                let m = self.read(bus, addr);
                self.compare(self.y, m);
            }
            Bit => {
                let m = self.read(bus, addr);
                self.set_flag(Z, self.a & m == 0);
                self.set_flag(N, m & 0x80 != 0);
                self.set_flag(V, m & 0x40 != 0);
            }

            // ---- incrementos e deslocamentos
            Inc => self.rmw(bus, mode, addr, |c, v| {
                let r = v.wrapping_add(1);
                c.set_zn(r);
                r
            }),
            Dec => self.rmw(bus, mode, addr, |c, v| {
                let r = v.wrapping_sub(1);
                c.set_zn(r);
                r
            }),
            Inx => {
                self.x = self.x.wrapping_add(1);
                self.set_zn(self.x);
            }
            Iny => {
                self.y = self.y.wrapping_add(1);
                self.set_zn(self.y);
            }
            Dex => {
                self.x = self.x.wrapping_sub(1);
                self.set_zn(self.x);
            }
            Dey => {
                self.y = self.y.wrapping_sub(1);
                self.set_zn(self.y);
            }
            Asl => self.rmw(bus, mode, addr, Self::asl),
            Lsr => self.rmw(bus, mode, addr, Self::lsr),
            Rol => self.rmw(bus, mode, addr, Self::rol),
            Ror => self.rmw(bus, mode, addr, Self::ror),

            // ---- flags
            Clc => self.set_flag(C, false),
            Sec => self.set_flag(C, true),
            Cli => self.set_flag(I, false),
            Sei => self.set_flag(I, true),
            Cld => self.set_flag(D, false),
            Sed => self.set_flag(D, true),
            Clv => self.set_flag(V, false),

            // ---- desvios
            Bcc => self.branch(bus, !self.flag(C), addr),
            Bcs => self.branch(bus, self.flag(C), addr),
            Bne => self.branch(bus, !self.flag(Z), addr),
            Beq => self.branch(bus, self.flag(Z), addr),
            Bpl => self.branch(bus, !self.flag(N), addr),
            Bmi => self.branch(bus, self.flag(N), addr),
            Bvc => self.branch(bus, !self.flag(V), addr),
            Bvs => self.branch(bus, self.flag(V), addr),
            Jmp => self.pc = addr,
            Jsr => {
                self.dummy_stack_read(bus);
                let ret = self.pc.wrapping_sub(1);
                self.push(bus, (ret >> 8) as u8);
                self.push(bus, ret as u8);
                self.pc = addr;
            }
            Rts => {
                self.dummy_stack_read(bus);
                let lo = self.pull(bus) as u16;
                let hi = self.pull(bus) as u16;
                self.pc = (hi << 8) | lo;
                self.read(bus, self.pc);
                self.pc = self.pc.wrapping_add(1);
            }
            Rti => {
                self.dummy_stack_read(bus);
                self.status = (self.pull(bus) & !B) | U;
                let lo = self.pull(bus) as u16;
                let hi = self.pull(bus) as u16;
                self.pc = (hi << 8) | lo;
            }
            Brk => {
                self.read(bus, addr); // byte de padding
                self.interrupt_sequence(bus, true);
            }
            Nop => {
                if mode != Mode::Imp {
                    self.read(bus, addr);
                }
            }

            // ---- não-oficiais
            Lax => {
                let v = self.read(bus, addr);
                self.a = v;
                self.x = v;
                self.set_zn(v);
            }
            Sax => self.write(bus, addr, self.a & self.x),
            Dcp => self.rmw(bus, mode, addr, |c, v| {
                let r = v.wrapping_sub(1);
                c.compare(c.a, r);
                r
            }),
            Isb => self.rmw(bus, mode, addr, |c, v| {
                let r = v.wrapping_add(1);
                c.adc(!r);
                r
            }),
            Slo => self.rmw(bus, mode, addr, |c, v| {
                let r = c.asl(v);
                c.a |= r;
                c.set_zn(c.a);
                r
            }),
            Rla => self.rmw(bus, mode, addr, |c, v| {
                let r = c.rol(v);
                c.a &= r;
                c.set_zn(c.a);
                r
            }),
            Sre => self.rmw(bus, mode, addr, |c, v| {
                let r = c.lsr(v);
                c.a ^= r;
                c.set_zn(c.a);
                r
            }),
            Rra => self.rmw(bus, mode, addr, |c, v| {
                let r = c.ror(v);
                c.adc(r);
                r
            }),
            Anc => {
                self.a &= self.read(bus, addr);
                self.set_zn(self.a);
                self.set_flag(C, self.a & 0x80 != 0);
            }
            Alr => {
                self.a &= self.read(bus, addr);
                self.a = self.lsr(self.a);
            }
            Arr => {
                self.a &= self.read(bus, addr);
                self.a = (self.a >> 1) | ((self.flag(C) as u8) << 7);
                self.set_zn(self.a);
                self.set_flag(C, self.a & 0x40 != 0);
                self.set_flag(V, ((self.a >> 6) ^ (self.a >> 5)) & 1 != 0);
            }
            Xaa => {
                // instável no hardware; constante mágica $EE
                self.a = (self.a | 0xEE) & self.x & self.read(bus, addr);
                self.set_zn(self.a);
            }
            Axs => {
                let m = self.read(bus, addr);
                let ax = self.a & self.x;
                self.set_flag(C, ax >= m);
                self.x = ax.wrapping_sub(m);
                self.set_zn(self.x);
            }
            Las => {
                let v = self.read(bus, addr) & self.stkp;
                self.a = v;
                self.x = v;
                self.stkp = v;
                self.set_zn(v);
            }
            Sha => self.store_high_and(bus, addr, self.a & self.x),
            Shx => self.store_high_and(bus, addr, self.x),
            Shy => self.store_high_and(bus, addr, self.y),
            Tas => {
                self.stkp = self.a & self.x;
                self.store_high_and(bus, addr, self.stkp);
            }
            Jam => {
                self.jammed = true;
                self.pc = self.pc.wrapping_sub(1);
            }
        }
    }

    // ------------------------------------------------------------------ reset

    /// Reset (7 ciclos): A/X/Y e os demais bits de P são preservados; I=1; SP desce 3 (os três
    /// "pushes" do reset são leituras); PC vem de `$FFFC`.
    pub fn reset(&mut self, bus: &mut Bus) {
        self.stkp = self.stkp.wrapping_sub(3);
        self.status |= I | U;
        self.jammed = false;
        self.nmi_pending = false;
        self.nmi_pending_prev = false;
        self.irq_run = false;
        self.irq_run_prev = false;
        for _ in 0..5 {
            bus.tick_pre();
            bus.tick_post();
        }
        let lo = self.read(bus, RESET_VECTOR) as u16;
        let hi = self.read(bus, RESET_VECTOR + 1) as u16;
        self.pc = (hi << 8) | lo;
        // sem borda espúria de NMI no power-on
        self.nmi_level = bus.nmi_line();
        self.nmi_pending = false;
        self.nmi_pending_prev = false;
        self.irq_run = false;
        self.irq_run_prev = false;
    }
}
