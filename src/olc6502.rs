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
    pub name: &'static str,                                  // Nome da instrução
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
    pub fn read(&self, addr: u16) -> u8 {
        self.bus.read(addr, false)
    }

    pub fn write(&mut self, addr: u16, data: u8) {
        self.bus.write(addr, data);
    }
    // ##############################
    // ##       Inicialização      ##
    // ##    de Algumas Coisas     ##
    // ##############################

    // Modos de enderecamento
    pub fn IMP(&mut self) -> u8 {0}   pub fn IMM(&mut self) -> u8 {0}
    pub fn ZP0(&mut self) -> u8 {0}   pub fn ZPX(&mut self) -> u8 {0}
    pub fn ZPY(&mut self) -> u8 {0}   pub fn REL(&mut self) -> u8 {0}
    pub fn ABS(&mut self) -> u8 {0}   pub fn ABX(&mut self) -> u8 {0}
    pub fn ABY(&mut self) -> u8 {0}   pub fn IND(&mut self) -> u8 {0}
    pub fn IZX(&mut self) -> u8 {0}   pub fn IZY(&mut self) -> u8 {0}

    // Opcodes
    pub fn ADC(&mut self) -> u8 {0}   pub fn AND(&mut self) -> u8 {0}
    pub fn ASL(&mut self) -> u8 {0}   pub fn BCC(&mut self) -> u8 {0}
    pub fn BCS(&mut self) -> u8 {0}   pub fn BEQ(&mut self) -> u8 {0}
    pub fn BIT(&mut self) -> u8 {0}   pub fn BMI(&mut self) -> u8 {0}
    pub fn BNE(&mut self) -> u8 {0}   pub fn BPL(&mut self) -> u8 {0}
    pub fn BRK(&mut self) -> u8 {0}   pub fn BVC(&mut self) -> u8 {0}
    pub fn BVS(&mut self) -> u8 {0}   pub fn CLC(&mut self) -> u8 {0}
    pub fn CLD(&mut self) -> u8 {0}   pub fn CLI(&mut self) -> u8 {0}
    pub fn CLV(&mut self) -> u8 {0}   pub fn CMP(&mut self) -> u8 {0}
    pub fn CPX(&mut self) -> u8 {0}   pub fn CPY(&mut self) -> u8 {0}
    pub fn DEC(&mut self) -> u8 {0}   pub fn DEX(&mut self) -> u8 {0}
    pub fn DEY(&mut self) -> u8 {0}   pub fn EOR(&mut self) -> u8 {0}
    pub fn INC(&mut self) -> u8 {0}   pub fn INX(&mut self) -> u8 {0}
    pub fn INY(&mut self) -> u8 {0}   pub fn JMP(&mut self) -> u8 {0}
    pub fn JSR(&mut self) -> u8 {0}   pub fn LDA(&mut self) -> u8 {0}
    pub fn LDX(&mut self) -> u8 {0}   pub fn LDY(&mut self) -> u8 {0}
    pub fn LSR(&mut self) -> u8 {0}   pub fn NOP(&mut self) -> u8 {0}
    pub fn XXX(&mut self) -> u8 {0}   // Nao Implementado

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

    // Tem algumas instrucoes que somente alguns
    // Jogos ou nenhum usa, então nao irei emula-los
    // Mas pode ser que com o tempo eu adicione-os
    // Eu irei coloca-los como "???"
    // Ou, quando for realmente em branco, mas alguns
    // Roms modificadas podem nao funcionar
    pub fn instrucoes() -> Vec<Instruction> {
        vec![
            // 0x00
            Instruction { name: "BRK", operate: Olc6502::BRK, addrmode: Olc6502::IMP, cycles: 7 },
            Instruction { name: "ORA", operate: Olc6502::ORA, addrmode: Olc6502::IZX, cycles: 6 },
            Instruction { name: "???", operate: Olc6502::XXX, addrmode: Olc6502::IMP, cycles: 2 },
            Instruction { name: "???", operate: Olc6502::XXX, addrmode: Olc6502::IMP, cycles: 8 },
            Instruction { name: "???", operate: Olc6502::NOP, addrmode: Olc6502::IMP, cycles: 3 },
            Instruction { name: "ORA", operate: Olc6502::ORA, addrmode: Olc6502::ZP0, cycles: 3 },
            Instruction { name: "ASL", operate: Olc6502::ASL, addrmode: Olc6502::ZP0, cycles: 5 },
            Instruction { name: "???", operate: Olc6502::XXX, addrmode: Olc6502::IMP, cycles: 5 },
            Instruction { name: "PHP", operate: Olc6502::PHP, addrmode: Olc6502::IMP, cycles: 3 },
            Instruction { name: "ORA", operate: Olc6502::ORA, addrmode: Olc6502::IMM, cycles: 2 },
            Instruction { name: "ASL", operate: Olc6502::ASL, addrmode: Olc6502::IMP, cycles: 2 },
            Instruction { name: "???", operate: Olc6502::XXX, addrmode: Olc6502::IMP, cycles: 2 },
            Instruction { name: "???", operate: Olc6502::NOP, addrmode: Olc6502::IMP, cycles: 4 },
            Instruction { name: "ORA", operate: Olc6502::ORA, addrmode: Olc6502::ABS, cycles: 4 },
            Instruction { name: "ASL", operate: Olc6502::ASL, addrmode: Olc6502::ABS, cycles: 6 },
            Instruction { name: "???", operate: Olc6502::XXX, addrmode: Olc6502::IMP, cycles: 6 },
    
            // 0x10
            Instruction { name: "BPL", operate: Olc6502::BPL, addrmode: Olc6502::REL, cycles: 2 },
            Instruction { name: "ORA", operate: Olc6502::ORA, addrmode: Olc6502::IZY, cycles: 5 },
            Instruction { name: "???", operate: Olc6502::XXX, addrmode: Olc6502::IMP, cycles: 2 },
            Instruction { name: "???", operate: Olc6502::XXX, addrmode: Olc6502::IMP, cycles: 8 },
            Instruction { name: "???", operate: Olc6502::NOP, addrmode: Olc6502::IMP, cycles: 4 },
            Instruction { name: "ORA", operate: Olc6502::ORA, addrmode: Olc6502::ZPX, cycles: 4 },
            Instruction { name: "ASL", operate: Olc6502::ASL, addrmode: Olc6502::ZPX, cycles: 6 },
            Instruction { name: "???", operate: Olc6502::XXX, addrmode: Olc6502::IMP, cycles: 6 },
            Instruction { name: "CLC", operate: Olc6502::CLC, addrmode: Olc6502::IMP, cycles: 2 },
            Instruction { name: "ORA", operate: Olc6502::ORA, addrmode: Olc6502::ABY, cycles: 4 },
            Instruction { name: "???", operate: Olc6502::NOP, addrmode: Olc6502::IMP, cycles: 2 },
            Instruction { name: "???", operate: Olc6502::XXX, addrmode: Olc6502::IMP, cycles: 7 },
            Instruction { name: "???", operate: Olc6502::NOP, addrmode: Olc6502::IMP, cycles: 4 },
            Instruction { name: "ORA", operate: Olc6502::ORA, addrmode: Olc6502::ABX, cycles: 4 },
            Instruction { name: "ASL", operate: Olc6502::ASL, addrmode: Olc6502::ABX, cycles: 7 },
            Instruction { name: "???", operate: Olc6502::XXX, addrmode: Olc6502::IMP, cycles: 7 },
    
            // 0x20
            Instruction { name: "JSR", operate: Olc6502::JSR, addrmode: Olc6502::ABS, cycles: 6 },
            Instruction { name: "AND", operate: Olc6502::AND, addrmode: Olc6502::IZX, cycles: 6 },
            Instruction { name: "???", operate: Olc6502::XXX, addrmode: Olc6502::IMP, cycles: 2 },
            Instruction { name: "???", operate: Olc6502::XXX, addrmode: Olc6502::IMP, cycles: 8 },
            Instruction { name: "BIT", operate: Olc6502::BIT, addrmode: Olc6502::ZP0, cycles: 3 },
            Instruction { name: "AND", operate: Olc6502::AND, addrmode: Olc6502::ZP0, cycles: 3 },
            Instruction { name: "ROL", operate: Olc6502::ROL, addrmode: Olc6502::ZP0, cycles: 5 },
            Instruction { name: "???", operate: Olc6502::XXX, addrmode: Olc6502::IMP, cycles: 5 },
            Instruction { name: "PLP", operate: Olc6502::PLP, addrmode: Olc6502::IMP, cycles: 4 },
            Instruction { name: "AND", operate: Olc6502::AND, addrmode: Olc6502::IMM, cycles: 2 },
            Instruction { name: "ROL", operate: Olc6502::ROL, addrmode: Olc6502::IMP, cycles: 2 },
            Instruction { name: "???", operate: Olc6502::XXX, addrmode: Olc6502::IMP, cycles: 2 },
            Instruction { name: "BIT", operate: Olc6502::BIT, addrmode: Olc6502::ABS, cycles: 4 },
            Instruction { name: "AND", operate: Olc6502::AND, addrmode: Olc6502::ABS, cycles: 4 },
            Instruction { name: "ROL", operate: Olc6502::ROL, addrmode: Olc6502::ABS, cycles: 6 },
            Instruction { name: "???", operate: Olc6502::XXX, addrmode: Olc6502::IMP, cycles: 6 },
    
            // 0x30
            Instruction { name: "BMI", operate: Olc6502::BMI, addrmode: Olc6502::REL, cycles: 2 },
            Instruction { name: "AND", operate: Olc6502::AND, addrmode: Olc6502::IZY, cycles: 5 },
            Instruction { name: "???", operate: Olc6502::XXX, addrmode: Olc6502::IMP, cycles: 2 },
            Instruction { name: "???", operate: Olc6502::XXX, addrmode: Olc6502::IMP, cycles: 8 },
            Instruction { name: "???", operate: Olc6502::NOP, addrmode: Olc6502::IMP, cycles: 4 },
            Instruction { name: "AND", operate: Olc6502::AND, addrmode: Olc6502::ZPX, cycles: 4 },
            Instruction { name: "ROL", operate: Olc6502::ROL, addrmode: Olc6502::ZPX, cycles: 6 },
            Instruction { name: "???", operate: Olc6502::XXX, addrmode: Olc6502::IMP, cycles: 6 },
            Instruction { name: "SEC", operate: Olc6502::SEC, addrmode: Olc6502::IMP, cycles: 2 },
            Instruction { name: "AND", operate: Olc6502::AND, addrmode: Olc6502::ABY, cycles: 4 },
            Instruction { name: "???", operate: Olc6502::NOP, addrmode: Olc6502::IMP, cycles: 2 },
            Instruction { name: "???", operate: Olc6502::XXX, addrmode: Olc6502::IMP, cycles: 7 },
            Instruction { name: "???", operate: Olc6502::NOP, addrmode: Olc6502::IMP, cycles: 4 },
            Instruction { name: "AND", operate: Olc6502::AND, addrmode: Olc6502::ABX, cycles: 4 },
            Instruction { name: "ROL", operate: Olc6502::ROL, addrmode: Olc6502::ABX, cycles: 7 },
            Instruction { name: "???", operate: Olc6502::XXX, addrmode: Olc6502::IMP, cycles: 7 },
    
            // 0x40
            Instruction { name: "RTI", operate: Olc6502::RTI, addrmode: Olc6502::IMP, cycles: 6 },
            Instruction { name: "EOR", operate: Olc6502::EOR, addrmode: Olc6502::IZX, cycles: 6 },
            Instruction { name: "???", operate: Olc6502::XXX, addrmode: Olc6502::IMP, cycles: 2 },
            Instruction { name: "???", operate: Olc6502::XXX, addrmode: Olc6502::IMP, cycles: 8 },
            Instruction { name: "???", operate: Olc6502::NOP, addrmode: Olc6502::IMP, cycles: 3 },
            Instruction { name: "EOR", operate: Olc6502::EOR, addrmode: Olc6502::ZP0, cycles: 3 },
            Instruction { name: "LSR", operate: Olc6502::LSR, addrmode: Olc6502::ZP0, cycles: 5 },
            Instruction { name: "???", operate: Olc6502::XXX, addrmode: Olc6502::IMP, cycles: 5 },
            Instruction { name: "PHA", operate: Olc6502::PHA, addrmode: Olc6502::IMP, cycles: 3 },
            Instruction { name: "EOR", operate: Olc6502::EOR, addrmode: Olc6502::IMM, cycles: 2 },
            Instruction { name: "LSR", operate: Olc6502::LSR, addrmode: Olc6502::IMP, cycles: 2 },
            Instruction { name: "???", operate: Olc6502::XXX, addrmode: Olc6502::IMP, cycles: 2 },
            Instruction { name: "JMP", operate: Olc6502::JMP, addrmode: Olc6502::ABS, cycles: 3 },
            Instruction { name: "EOR", operate: Olc6502::EOR, addrmode: Olc6502::ABS, cycles: 4 },
            Instruction { name: "LSR", operate: Olc6502::LSR, addrmode: Olc6502::ABS, cycles: 6 },
            Instruction { name: "???", operate: Olc6502::XXX, addrmode: Olc6502::IMP, cycles: 6 },
    
            // 0x50
            Instruction { name: "BVC", operate: Olc6502::BVC, addrmode: Olc6502::REL, cycles: 2 },
            Instruction { name: "EOR", operate: Olc6502::EOR, addrmode: Olc6502::IZY, cycles: 5 },
            Instruction { name: "???", operate: Olc6502::XXX, addrmode: Olc6502::IMP, cycles: 2 },
            Instruction { name: "???", operate: Olc6502::XXX, addrmode: Olc6502::IMP, cycles: 8 },
            Instruction { name: "???", operate: Olc6502::NOP, addrmode: Olc6502::IMP, cycles: 4 },
            Instruction { name: "EOR", operate: Olc6502::EOR, addrmode: Olc6502::ZPX, cycles: 4 },
            Instruction { name: "LSR", operate: Olc6502::LSR, addrmode: Olc6502::ZPX, cycles: 6 },
            Instruction { name: "???", operate: Olc6502::XXX, addrmode: Olc6502::IMP, cycles: 6 },
            Instruction { name: "CLI", operate: Olc6502::CLI, addrmode: Olc6502::IMP, cycles: 2 },
            Instruction { name: "EOR", operate: Olc6502::EOR, addrmode: Olc6502::ABY, cycles: 4 },
            Instruction { name: "???", operate: Olc6502::NOP, addrmode: Olc6502::IMP, cycles: 2 },
            Instruction { name: "???", operate: Olc6502::XXX, addrmode: Olc6502::IMP, cycles: 7 },
            Instruction { name: "???", operate: Olc6502::NOP, addrmode: Olc6502::IMP, cycles: 4 },
            Instruction { name: "EOR", operate: Olc6502::EOR, addrmode: Olc6502::ABX, cycles: 4 },
            Instruction { name: "LSR", operate: Olc6502::LSR, addrmode: Olc6502::ABX, cycles: 7 },
            Instruction { name: "???", operate: Olc6502::XXX, addrmode: Olc6502::IMP, cycles: 7 },
    
            // 0x60
            Instruction { name: "RTS", operate: Olc6502::RTS, addrmode: Olc6502::IMP, cycles: 6 },
            Instruction { name: "ADC", operate: Olc6502::ADC, addrmode: Olc6502::IZX, cycles: 6 },
            Instruction { name: "???", operate: Olc6502::XXX, addrmode: Olc6502::IMP, cycles: 2 },
            Instruction { name: "???", operate: Olc6502::XXX, addrmode: Olc6502::IMP, cycles: 8 },
            Instruction { name: "???", operate: Olc6502::NOP, addrmode: Olc6502::IMP, cycles: 3 },
            Instruction { name: "ADC", operate: Olc6502::ADC, addrmode: Olc6502::ZP0, cycles: 3 },
            Instruction { name: "ROR", operate: Olc6502::ROR, addrmode: Olc6502::ZP0, cycles: 5 },
            Instruction { name: "???", operate: Olc6502::XXX, addrmode: Olc6502::IMP, cycles: 5 },
            Instruction { name: "PLA", operate: Olc6502::PLA, addrmode: Olc6502::IMP, cycles: 4 },
            Instruction { name: "ADC", operate: Olc6502::ADC, addrmode: Olc6502::IMM, cycles: 2 },
            Instruction { name: "ROR", operate: Olc6502::ROR, addrmode: Olc6502::IMP, cycles: 2 },
            Instruction { name: "???", operate: Olc6502::XXX, addrmode: Olc6502::IMP, cycles: 2 },
            Instruction { name: "JMP", operate: Olc6502::JMP, addrmode: Olc6502::IND, cycles: 5 },
            Instruction { name: "ADC", operate: Olc6502::ADC, addrmode: Olc6502::ABS, cycles: 4 },
            Instruction { name: "ROR", operate: Olc6502::ROR, addrmode: Olc6502::ABS, cycles: 6 },
            Instruction { name: "???", operate: Olc6502::XXX, addrmode: Olc6502::IMP, cycles: 6 },
    
            // 0x70
            Instruction { name: "BVS", operate: Olc6502::BVS, addrmode: Olc6502::REL, cycles: 2 },
            Instruction { name: "ADC", operate: Olc6502::ADC, addrmode: Olc6502::IZY, cycles: 5 },
            Instruction { name: "???", operate: Olc6502::XXX, addrmode: Olc6502::IMP, cycles: 2 },
            Instruction { name: "???", operate: Olc6502::XXX, addrmode: Olc6502::IMP, cycles: 8 },
            Instruction { name: "???", operate: Olc6502::NOP, addrmode: Olc6502::IMP, cycles: 4 },
            Instruction { name: "ADC", operate: Olc6502::ADC, addrmode: Olc6502::ZPX, cycles: 4 },
            Instruction { name: "ROR", operate: Olc6502::ROR, addrmode: Olc6502::ZPX, cycles: 6 },
            Instruction { name: "???", operate: Olc6502::XXX, addrmode: Olc6502::IMP, cycles: 6 },
            Instruction { name: "SEI", operate: Olc6502::SEI, addrmode: Olc6502::IMP, cycles: 2 },
            Instruction { name: "ADC", operate: Olc6502::ADC, addrmode: Olc6502::ABY, cycles: 4 },
            Instruction { name: "???", operate: Olc6502::NOP, addrmode: Olc6502::IMP, cycles: 2 },
            Instruction { name: "???", operate: Olc6502::XXX, addrmode: Olc6502::IMP, cycles: 7 },
            Instruction { name: "???", operate: Olc6502::NOP, addrmode: Olc6502::IMP, cycles: 4 },
            Instruction { name: "ADC", operate: Olc6502::ADC, addrmode: Olc6502::ABX, cycles: 4 },
            Instruction { name: "ROR", operate: Olc6502::ROR, addrmode: Olc6502::ABX, cycles: 7 },
            Instruction { name: "???", operate: Olc6502::XXX, addrmode: Olc6502::IMP, cycles: 7 },
    
            // 0x80
            Instruction { name: "???", operate: Olc6502::NOP, addrmode: Olc6502::IMP, cycles: 2 },
            Instruction { name: "STA", operate: Olc6502::STA, addrmode: Olc6502::IZX, cycles: 6 },
            Instruction { name: "???", operate: Olc6502::NOP, addrmode: Olc6502::IMP, cycles: 2 },
            Instruction { name: "???", operate: Olc6502::XXX, addrmode: Olc6502::IMP, cycles: 6 },
            Instruction { name: "STY", operate: Olc6502::STY, addrmode: Olc6502::ZP0, cycles: 3 },
            Instruction { name: "STA", operate: Olc6502::STA, addrmode: Olc6502::ZP0, cycles: 3 },
            Instruction { name: "STX", operate: Olc6502::STX, addrmode: Olc6502::ZP0, cycles: 3 },
            Instruction { name: "???", operate: Olc6502::XXX, addrmode: Olc6502::IMP, cycles: 3 },
            Instruction { name: "DEY", operate: Olc6502::DEY, addrmode: Olc6502::IMP, cycles: 2 },
            Instruction { name: "???", operate: Olc6502::NOP, addrmode: Olc6502::IMP, cycles: 2 },
            Instruction { name: "TXA", operate: Olc6502::TXA, addrmode: Olc6502::IMP, cycles: 2 },
            Instruction { name: "???", operate: Olc6502::XXX, addrmode: Olc6502::IMP, cycles: 2 },
            Instruction { name: "STY", operate: Olc6502::STY, addrmode: Olc6502::ABS, cycles: 4 },
            Instruction { name: "STA", operate: Olc6502::STA, addrmode: Olc6502::ABS, cycles: 4 },
            Instruction { name: "STX", operate: Olc6502::STX, addrmode: Olc6502::ABS, cycles: 4 },
            Instruction { name: "???", operate: Olc6502::XXX, addrmode: Olc6502::IMP, cycles: 4 },
    
            // 0x90
            Instruction { name: "BCC", operate: Olc6502::BCC, addrmode: Olc6502::REL, cycles: 2 },
            Instruction { name: "STA", operate: Olc6502::STA, addrmode: Olc6502::IZY, cycles: 6 },
            Instruction { name: "???", operate: Olc6502::XXX, addrmode: Olc6502::IMP, cycles: 2 },
            Instruction { name: "???", operate: Olc6502::XXX, addrmode: Olc6502::IMP, cycles: 6 },
            Instruction { name: "STY", operate: Olc6502::STY, addrmode: Olc6502::ZPX, cycles: 4 },
            Instruction { name: "STA", operate: Olc6502::STA, addrmode: Olc6502::ZPX, cycles: 4 },
            Instruction { name: "STX", operate: Olc6502::STX, addrmode: Olc6502::ZPY, cycles: 4 },
            Instruction { name: "???", operate: Olc6502::XXX, addrmode: Olc6502::IMP, cycles: 4 },
            Instruction { name: "TYA", operate: Olc6502::TYA, addrmode: Olc6502::IMP, cycles: 2 },
            Instruction { name: "STA", operate: Olc6502::STA, addrmode: Olc6502::ABY, cycles: 5 },
            Instruction { name: "TXS", operate: Olc6502::TXS, addrmode: Olc6502::IMP, cycles: 2 },
            Instruction { name: "???", operate: Olc6502::XXX, addrmode: Olc6502::IMP, cycles: 5 },
            Instruction { name: "???", operate: Olc6502::NOP, addrmode: Olc6502::IMP, cycles: 5 },
            Instruction { name: "STA", operate: Olc6502::STA, addrmode: Olc6502::ABX, cycles: 5 },
            Instruction { name: "???", operate: Olc6502::XXX, addrmode: Olc6502::IMP, cycles: 5 },
            Instruction { name: "???", operate: Olc6502::XXX, addrmode: Olc6502::IMP, cycles: 5 },
    
            // 0xA0
            Instruction { name: "LDY", operate: Olc6502::LDY, addrmode: Olc6502::IMM, cycles: 2 },
            Instruction { name: "LDA", operate: Olc6502::LDA, addrmode: Olc6502::IZX, cycles: 6 },
            Instruction { name: "LDX", operate: Olc6502::LDX, addrmode: Olc6502::IMM, cycles: 2 },
            Instruction { name: "???", operate: Olc6502::XXX, addrmode: Olc6502::IMP, cycles: 6 },
            Instruction { name: "LDY", operate: Olc6502::LDY, addrmode: Olc6502::ZP0, cycles: 3 },
            Instruction { name: "LDA", operate: Olc6502::LDA, addrmode: Olc6502::ZP0, cycles: 3 },
            Instruction { name: "LDX", operate: Olc6502::LDX, addrmode: Olc6502::ZP0, cycles: 3 },
            Instruction { name: "???", operate: Olc6502::XXX, addrmode: Olc6502::IMP, cycles: 3 },
            Instruction { name: "TAY", operate: Olc6502::TAY, addrmode: Olc6502::IMP, cycles: 2 },
            Instruction { name: "LDA", operate: Olc6502::LDA, addrmode: Olc6502::IMM, cycles: 2 },
            Instruction { name: "TAX", operate: Olc6502::TAX, addrmode: Olc6502::IMP, cycles: 2 },
            Instruction { name: "???", operate: Olc6502::XXX, addrmode: Olc6502::IMP, cycles: 2 },
            Instruction { name: "LDY", operate: Olc6502::LDY, addrmode: Olc6502::ABS, cycles: 4 },
            Instruction { name: "LDA", operate: Olc6502::LDA, addrmode: Olc6502::ABS, cycles: 4 },
            Instruction { name: "LDX", operate: Olc6502::LDX, addrmode: Olc6502::ABS, cycles: 4 },
            Instruction { name: "???", operate: Olc6502::XXX, addrmode: Olc6502::IMP, cycles: 4 },
    
            // 0xB0
            Instruction { name: "BCS", operate: Olc6502::BCS, addrmode: Olc6502::REL, cycles: 2 },
            Instruction { name: "LDA", operate: Olc6502::LDA, addrmode: Olc6502::IZY, cycles: 5 },
            Instruction { name: "???", operate: Olc6502::XXX, addrmode: Olc6502::IMP, cycles: 2 },
            Instruction { name: "???", operate: Olc6502::XXX, addrmode: Olc6502::IMP, cycles: 5 },
            Instruction { name: "LDY", operate: Olc6502::LDY, addrmode: Olc6502::ZPX, cycles: 4 },
            Instruction { name: "LDA", operate: Olc6502::LDA, addrmode: Olc6502::ZPX, cycles: 4 },
            Instruction { name: "LDX", operate: Olc6502::LDX, addrmode: Olc6502::ZPY, cycles: 4 },
            Instruction { name: "???", operate: Olc6502::XXX, addrmode: Olc6502::IMP, cycles: 4 },
            Instruction { name: "CLV", operate: Olc6502::CLV, addrmode: Olc6502::IMP, cycles: 2 },
            Instruction { name: "LDA", operate: Olc6502::LDA, addrmode: Olc6502::ABY, cycles: 4 },
            Instruction { name: "TSX", operate: Olc6502::TSX, addrmode: Olc6502::IMP, cycles: 2 },
            Instruction { name: "???", operate: Olc6502::XXX, addrmode: Olc6502::IMP, cycles: 4 },
            Instruction { name: "LDY", operate: Olc6502::LDY, addrmode: Olc6502::ABX, cycles: 4 },
            Instruction { name: "LDA", operate: Olc6502::LDA, addrmode: Olc6502::ABX, cycles: 4 },
            Instruction { name: "LDX", operate: Olc6502::LDX, addrmode: Olc6502::ABY, cycles: 4 },
            Instruction { name: "???", operate: Olc6502::XXX, addrmode: Olc6502::IMP, cycles: 4 },
    
            // 0xC0
            Instruction { name: "CPY", operate: Olc6502::CPY, addrmode: Olc6502::IMM, cycles: 2 },
            Instruction { name: "CMP", operate: Olc6502::CMP, addrmode: Olc6502::IZX, cycles: 6 },
            Instruction { name: "???", operate: Olc6502::NOP, addrmode: Olc6502::IMP, cycles: 2 },
            Instruction { name: "???", operate: Olc6502::XXX, addrmode: Olc6502::IMP, cycles: 8 },
            Instruction { name: "CPY", operate: Olc6502::CPY, addrmode: Olc6502::ZP0, cycles: 3 },
            Instruction { name: "CMP", operate: Olc6502::CMP, addrmode: Olc6502::ZP0, cycles: 3 },
            Instruction { name: "DEC", operate: Olc6502::DEC, addrmode: Olc6502::ZP0, cycles: 5 },
            Instruction { name: "???", operate: Olc6502::XXX, addrmode: Olc6502::IMP, cycles: 5 },
            Instruction { name: "INY", operate: Olc6502::INY, addrmode: Olc6502::IMP, cycles: 2 },
            Instruction { name: "CMP", operate: Olc6502::CMP, addrmode: Olc6502::IMM, cycles: 2 },
            Instruction { name: "DEX", operate: Olc6502::DEX, addrmode: Olc6502::IMP, cycles: 2 },
            Instruction { name: "???", operate: Olc6502::XXX, addrmode: Olc6502::IMP, cycles: 2 },
            Instruction { name: "CPY", operate: Olc6502::CPY, addrmode: Olc6502::ABS, cycles: 4 },
            Instruction { name: "CMP", operate: Olc6502::CMP, addrmode: Olc6502::ABS, cycles: 4 },
            Instruction { name: "DEC", operate: Olc6502::DEC, addrmode: Olc6502::ABS, cycles: 6 },
            Instruction { name: "???", operate: Olc6502::XXX, addrmode: Olc6502::IMP, cycles: 6 },
    
            // 0xD0
            Instruction { name: "BNE", operate: Olc6502::BNE, addrmode: Olc6502::REL, cycles: 2 },
            Instruction { name: "CMP", operate: Olc6502::CMP, addrmode: Olc6502::IZY, cycles: 5 },
            Instruction { name: "???", operate: Olc6502::XXX, addrmode: Olc6502::IMP, cycles: 2 },
            Instruction { name: "???", operate: Olc6502::XXX, addrmode: Olc6502::IMP, cycles: 8 },
            Instruction { name: "???", operate: Olc6502::NOP, addrmode: Olc6502::IMP, cycles: 4 },
            Instruction { name: "CMP", operate: Olc6502::CMP, addrmode: Olc6502::ZPX, cycles: 4 },
            Instruction { name: "DEC", operate: Olc6502::DEC, addrmode: Olc6502::ZPX, cycles: 6 },
            Instruction { name: "???", operate: Olc6502::XXX, addrmode: Olc6502::IMP, cycles: 6 },
            Instruction { name: "CLD", operate: Olc6502::CLD, addrmode: Olc6502::IMP, cycles: 2 },
            Instruction { name: "CMP", operate: Olc6502::CMP, addrmode: Olc6502::ABY, cycles: 4 },
            Instruction { name: "NOP", operate: Olc6502::NOP, addrmode: Olc6502::IMP, cycles: 2 },
            Instruction { name: "???", operate: Olc6502::XXX, addrmode: Olc6502::IMP, cycles: 7 },
            Instruction { name: "???", operate: Olc6502::NOP, addrmode: Olc6502::IMP, cycles: 4 },
            Instruction { name: "CMP", operate: Olc6502::CMP, addrmode: Olc6502::ABX, cycles: 4 },
            Instruction { name: "DEC", operate: Olc6502::DEC, addrmode: Olc6502::ABX, cycles: 7 },
            Instruction { name: "???", operate: Olc6502::XXX, addrmode: Olc6502::IMP, cycles: 7 },
    
            // 0xE0
            Instruction { name: "CPX", operate: Olc6502::CPX, addrmode: Olc6502::IMM, cycles: 2 },
            Instruction { name: "SBC", operate: Olc6502::SBC, addrmode: Olc6502::IZX, cycles: 6 },
            Instruction { name: "???", operate: Olc6502::NOP, addrmode: Olc6502::IMP, cycles: 2 },
            Instruction { name: "???", operate: Olc6502::XXX, addrmode: Olc6502::IMP, cycles: 8 },
            Instruction { name: "CPX", operate: Olc6502::CPX, addrmode: Olc6502::ZP0, cycles: 3 },
            Instruction { name: "SBC", operate: Olc6502::SBC, addrmode: Olc6502::ZP0, cycles: 3 },
            Instruction { name: "INC", operate: Olc6502::INC, addrmode: Olc6502::ZP0, cycles: 5 },
            Instruction { name: "???", operate: Olc6502::XXX, addrmode: Olc6502::IMP, cycles: 5 },
            Instruction { name: "INX", operate: Olc6502::INX, addrmode: Olc6502::IMP, cycles: 2 },
            Instruction { name: "SBC", operate: Olc6502::SBC, addrmode: Olc6502::IMM, cycles: 2 },
            Instruction { name: "NOP", operate: Olc6502::NOP, addrmode: Olc6502::IMP, cycles: 2 },
            Instruction { name: "???", operate: Olc6502::SBC, addrmode: Olc6502::IMP, cycles: 2 },
            Instruction { name: "CPX", operate: Olc6502::CPX, addrmode: Olc6502::ABS, cycles: 4 },
            Instruction { name: "SBC", operate: Olc6502::SBC, addrmode: Olc6502::ABS, cycles: 4 },
            Instruction { name: "INC", operate: Olc6502::INC, addrmode: Olc6502::ABS, cycles: 6 },
            Instruction { name: "???", operate: Olc6502::XXX, addrmode: Olc6502::IMP, cycles: 6 },
    
            // 0xF0
            Instruction { name: "BEQ", operate: Olc6502::BEQ, addrmode: Olc6502::REL, cycles: 2 },
            Instruction { name: "SBC", operate: Olc6502::SBC, addrmode: Olc6502::IZY, cycles: 5 },
            Instruction { name: "???", operate: Olc6502::XXX, addrmode: Olc6502::IMP, cycles: 2 },
            Instruction { name: "???", operate: Olc6502::XXX, addrmode: Olc6502::IMP, cycles: 8 },
            Instruction { name: "???", operate: Olc6502::NOP, addrmode: Olc6502::IMP, cycles: 4 },
            Instruction { name: "SBC", operate: Olc6502::SBC, addrmode: Olc6502::ZPX, cycles: 4 },
            Instruction { name: "INC", operate: Olc6502::INC, addrmode: Olc6502::ZPX, cycles: 6 },
            Instruction { name: "???", operate: Olc6502::XXX, addrmode: Olc6502::IMP, cycles: 6 },
            Instruction { name: "SED", operate: Olc6502::SED, addrmode: Olc6502::IMP, cycles: 2 },
            Instruction { name: "SBC", operate: Olc6502::SBC, addrmode: Olc6502::ABY, cycles: 4 },
            Instruction { name: "NOP", operate: Olc6502::NOP, addrmode: Olc6502::IMP, cycles: 2 },
            Instruction { name: "???", operate: Olc6502::XXX, addrmode: Olc6502::IMP, cycles: 7 },
            Instruction { name: "???", operate: Olc6502::NOP, addrmode: Olc6502::IMP, cycles: 4 },
            Instruction { name: "SBC", operate: Olc6502::SBC, addrmode: Olc6502::ABX, cycles: 4 },
            Instruction { name: "INC", operate: Olc6502::INC, addrmode: Olc6502::ABX, cycles: 7 },
            Instruction { name: "???", operate: Olc6502::XXX, addrmode: Olc6502::IMP, cycles: 7 },
        ]
    }    
    }
}
