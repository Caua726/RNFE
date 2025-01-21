mod bus;
use bus::Bus;
use std::cell::RefCell;
use std::rc::Rc;

// Flags Olc6502
pub enum FLAGS6502{
     C = 1 << 0,   // Transportar Bit
     Z = 1 << 1,   // Zerar
     I = 1 << 2,   // Disabilitar Interrupção
     D = 1 << 3,   // Modo Decimal
     B = 1 << 4,   // Break
     U = 1 << 5,   // Não usado
     V = 1 << 6,   // Overflow
     N = 1 << 7,   // Negativo
}

pub struct Olc6502 {
    bus: Option<Rc<RefCell<Bus>>>,
    a: u8,  // Acccumulator Register
    x: u8,  // X Register
    y: u8,  // Y Register
    stkp: u8,    // Stack Pointer (mostra os pontos do bus)
    pc: u16,     // Program Counter
    status: u8,  // Status Register
}

pub struct Instruction {
    pub name: String,                                  // Nome da instrução
    pub operate: fn(&mut Olc6502) -> u8,              // Ponteiro para a função de operação
    pub addrmode: fn(&mut Olc6502) -> u8,             // Ponteiro para a função de modo de endereçamento
    pub cycles: u8,                                   // Ciclos necessários para a instrução
}
//Vector<Instruction> lookup

impl Olc6502 {
    pub fn new() -> Self {
        Olc6502 { 
            bus: None,
            a:0x00,
            x:0x00,
            y:0x00,
            stkp:0x00,
            pc:0x0000,
            status:0x00
        }
    }

    // Testing Refcell, so, i will disable this
    //  pub fn connectBus(&mut self, bus: Bus) {
    //           self.bus = Rc::new(RefCell::new(bus));
    //   }

    pub fn connect_bus(&mut self, bus: Rc<RefCell<Bus>>) {
        self.bus = Some(bus);
    }

    pub fn getFlag(&self, flag: FLAGS6502) -> u8 {
        self.status & flag as u8
    }
    pub fn setFlag(&mut self, flag: FLAGS6502, value: bool) {
        if value {
            self.status |= flag as u8;
        } else {
            self.status &= !(flag as u8);
        }
    }
    fn read(&self, addr: u16) -> u8 {
        self.bus.read(addr, false)
    }

    fn write(&mut self, addr: u16, data: u8) {
        self.bus.write(addr, data);
    }
    
    // Tem algumas instrucoes que somente alguns
    // Jogos ou nenhum usa, então nao irei emula-los
    // Mas pode ser que com o tempo eu adicione-os
    // Eu irei coloca-los como "???" então alguns
    // Roms modificadas podem nao funcionar
    pub fn instrucoes() -> Vec<Instruction> {
        vec![
            Instruction {
                name: "BRK",
                operate: Olc6502::BRK,
                addrmode: Olc6502::IMP,
                cycles: 7
            },
            Instruction {
                name: "ORA",
                operate: Olc6502::ORA,
                addrmode: Olc6502::IZX,
                cycles: 6
            },
            Instruction {
                name: "???"
            }
        ]
    }
    // Modos de enderecamento
    fn IMP() -> u8 {}   fn IMM() -> u8 {}
    fn ZP0() -> u8 {}   fn ZPX() -> u8 {}
    fn ZPY() -> u8 {}   fn REL() -> u8 {}
    fn ABS() -> u8 {}   fn ABX() -> u8 {}
    fn ABY() -> u8 {}   fn IND() -> u8 {}
    fn IZX() -> u8 {}   fn IZY() -> u8 {}

    // Opcodes
    fn ADC() -> u8 {}   fn AND() -> u8 {}
    fn ASL() -> u8 {}   fn BCC() -> u8 {}
    fn BCS() -> u8 {}   fn BEQ() -> u8 {}
    fn BIT() -> u8 {}   fn BMI() -> u8 {}
    fn BNE() -> u8 {}   fn BPL() -> u8 {}
    fn BRK() -> u8 {}   fn BVC() -> u8 {}
    fn BVS() -> u8 {}   fn CLC() -> u8 {}
    fn CLD() -> u8 {}   fn CLI() -> u8 {}
    fn CLV() -> u8 {}   fn CMP() -> u8 {}
    fn CPX() -> u8 {}   fn CPY() -> u8 {}
    fn DEC() -> u8 {}   fn DEX() -> u8 {}
    fn DEY() -> u8 {}   fn EOR() -> u8 {}
    fn INC() -> u8 {}   fn INX() -> u8 {}
    fn INY() -> u8 {}   fn JMP() -> u8 {}
    fn JSR() -> u8 {}   fn LDA() -> u8 {}
    fn LDX() -> u8 {}   fn LDY() -> u8 {}
    fn LSR() -> u8 {}   fn NOP() -> u8 {}
    fn XXX() -> u8 {}

    // Clock
    pub fn clock() {
        
    }
    // Reset
    pub fn reset() {

    }
    // Interruptiuon Request
    pub fn irq() {
    
    }
    // Non Maskable Interrupt
    pub fn nmi() {
    
    }

    // Fetch
    pub fn fetch() -> u8 {
        
    }
    // Fetched
    pub fn fetched () -> u8 {
        
    }
    // Addr Abs
    pub fn addr_abs() -> u16 {
        
    }
    // Addr Rel
    pub fn addr_rel() -> u16 {
        
    }
    // Opcode Var
    pub fn opcode() -> u8 {
        
    }
    //Cycles
    pub fn cycles() -> u8 {
        
    }
}
