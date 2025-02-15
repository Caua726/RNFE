use crate::bus::Bus;
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
    a: u8,          // Acccumulator Register
    x: u8,          // X Register
    y: u8,          // Y Register
    stkp: u8,       // Stack Pointer (mostra os pontos do bus)
    pc: u16,        // Program Counter
    status: u8,     // Status Register
    fetched: u8,    // Nao sei, depois vejo
    addr_abs: u16,  // Endereco Absoluto
    addr_rel: u16,  // Endereço Relativo
    opcode: u8,     // Variavel Opcode
    cycles: u8,      // Ciclo de clock
    lookup: Vec<Instruction>,
}

pub struct Instruction {
    pub name: &'static str,                           // Nome da instrução
    pub operate: fn(&mut Olc6502) -> u8,              // Ponteiro para a função de operação
    pub addrmode: fn(&mut Olc6502) -> u8,             // Ponteiro para a função de modo de endereçamento
    pub cycles: u8,                                   // Ciclos necessários para a instrução
}
//Vector<Instruction> lookup

impl Olc6502 {
    pub fn new() -> Self {
        Olc6502 { 
            bus: None,                      // Barramento
            a:0x00,                         // Accumulator
            x:0x00,                         // X Register
            y:0x00,                         // Y Register
            stkp:0x00,                      // Stack Pointer
            pc:0x0000,                      // Program Counter
            status:0x00,                    // Status Register
            fetched:0x00,                   // Nao sei depois eu vejo
            addr_abs:0x0000,                // Todo o endereço de memoria acaba aqui
            addr_rel:0x0000,                // O endereço absoluto da atual instrução
            opcode:0x00,                    // Byte de instrução
            cycles:0,                       // Contagem do numero de ciclo de clocks
            lookup: Olc6502::instrucoes(),  // Lookup table para uinstrucoes da cpu
        }
    }

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
        self.bus
            .as_ref()
            .expect("Bus nao conectado")
            .borrow()
            .read(addr, false)
        // self.bus.read(addr, false)
    }

    pub fn write(&mut self, addr: u16, data: u8) {
        self.bus
            .as_ref()
            .expect("Bus nao conectado")
            .borrow_mut()
            .write(addr, data);
    }
    //==========================
    //# Modos de enderecamento #
    //==========================

    // IMP: Implied
    // Nao tem muita coisa pra falar dele
    // Ele Faz uma coisa muito simples, como, mudar o estado
    // De Bits, de qualquer jeito, o objetivo dele é
    // O Acumulador, para instrucoes como PHA
    pub fn IMP(&mut self) -> u8 {
        self.fetched = self.a;
        0
    } 

    // IMM: Imediato
    // A instrução espera até o proximo bit para ser usado
    // Como valor, o endereco de leitura deve apontar ao
    // Proximo valor
    pub fn IMM(&mut self) -> u8 {
        self.addr_abs = self.pc + 1;
        0
    }

    // ZP0: Zero Paging Adress / Modo de paginamento zero
    // Isso é usado para economizar recursos, permitindo
    // Que você salve o local nos primeiros 0xFF bytes
    // De um endereço de memoria, e por isso, isto só
    // Requer um byte de memoria ao invez de 2
    pub fn ZP0(&mut self) -> u8 {
        self.addr_abs = self.read(self.pc) as u16;
        self.pc += 1;
        self.addr_abs &= 0x00FF;
        0
    }
    // ZPX: Zero Paging X
    // O valor do X é adicionado ao valor lido
    // No endereço de memoria, e o resultado é
    // O endereço de memoria que o valor lido Apontava
    // Isso é util para interagir com areas da memória
    // Como um array em C
    pub fn ZPX(&mut self) -> u8 {
        self.addr_abs = (self.read(self.pc) + self.x) as u16;
        self.pc += 1;
        self.addr_abs &= 0x00FF;
        0
    }
    // ZPY: Zero Paging Y
    // O Mesmo do ZPX mas com Y
    pub fn ZPY(&mut self) -> u8 {
        self.addr_abs = (self.read(self.pc) + self.y) as u16;
        self.pc += 1;
        self.addr_abs &= 0x00FF;
        0
    }   
    // ABS: Absolute
    // O endereço absoluto é formado
    // Por dois bytes, o primeiro
    // Serve para o banco de memoória
    // O segundo para o endereço na
    // Memória para formar um endreço de 16 bits
    pub fn ABS(&mut self) -> u8 {
        let lo: u16 = self.read(self.pc) as u16;
        self.pc += 1;
        let hi: u16 = self.read(self.pc) as u16;
        self.pc += 1;

        self.addr_abs = (hi << 8) | lo;

        0
    }   
    // ABX: Absolute X
    // Endereço absoluto com valor X
    // Isto calcula o mesmo que o
    // ABS, mas adiciona o endereço X
    pub fn ABX(&mut self) -> u8 {
        let lo: u16 = self.read(self.pc) as u16;
        self.pc += 1;
        let hi: u16 = self.read(self.pc) as u16;
        self.pc += 1;

        self.addr_abs = (hi << 8) | lo;
        self.addr_abs += self.x as u16;

        if (self.addr_abs & 0xFF00) != (hi << 8) {
            1
        } else {
            0
        }

    }
    // ABY: Absolute Y
    // O mesmo que o ABX, mas
    // Adiciona o valor Y ao endereço
    pub fn ABY(&mut self) -> u8 {
        let lo: u16 = self.read(self.pc) as u16;
        self.pc += 1;
        let hi: u16 = self.read(self.pc) as u16;
        self.pc += 1;

        self.addr_abs = (hi << 8) | lo;
        self.addr_abs += self.y  as u16;

        if (self.addr_abs & 0xFF00) != (hi << 8) {
            1
        } else {
            0
        }

    }
    // IND: Indirect Adressing
    // Normalmente é reconhecido como um
    // Bug, mas para funcionar correctamente
    // É preciso ser emulador também, o que ele
    // Faz, é, apontar para o endereço 0xFF e ler
    // O byte mais alto, ele precisa atravesar
    // A page boudnary
    pub fn IND(&mut self) -> u8 {
        let ptr_lo: u16 = self.read(self.pc) as u16;
        self.pc += 1;
        let ptr_hi: u16 = self.read(self.pc) as u16;
        self.pc += 1;

        let ptr: u16 = (ptr_hi << 8) | ptr_lo;

        if ptr_lo == 0x00FF {
            self.addr_abs = ((self.read(ptr & 0xFF00) as u16) << 8) | self.read(ptr) as u16;
        } else {
            self.addr_abs = ((self.read(ptr + 1) as u16) << 8) | self.read(ptr) as u16;
        }
        0
    }
    // IZX: Indirect Adressing Zero Page X
    // Ele referencia um endereço na memória pagina
    // Zero a partir do offset X para ler o endereço
    // De 16 bits que precisamos para a instrução
    pub fn IZX(&mut self) -> u8 {
        let t: u16 = self.read(self.pc) as u16;
        self.pc += 1;
        let lo: u16 = self.read((t + (self.x  as u16)) & 0x00FF) as u16;
        let hi: u16 = self.read((t + (self.x as u16) + 1) & 0x00FF) as u16;

        self.addr_abs = (hi << 8 | lo);

        0
    } 
    // IZY: Indirect Adressing Zero Page Y
    // Diferentemente dos outros, onde X == Y, nesse
    // O X !== Y, neste caso ele lê um ponteiro de 16 bits
    // Da pagina zero e usa o offset Y para formar um
    // Endereço final.
    pub fn IZY(&mut self) -> u8 {
        let t: u16 = self.read(self.pc) as u16;
        self.pc += 1;
        let lo: u16 = self.read(t & 0x00FF) as u16;
        let hi: u16 = self.read((t + 1) & 0x00FF) as u16;

        self.addr_abs = (hi << 8 | lo);
        self.addr_abs += self.y as u16;

        if (self.addr_abs & 0xFF00) != (hi << 8) {
            return 1;
        } else {
            return 
        }

        0
    }   
    // REL: Relative.
    // Modo de Endereçamento Relativo, usado para
    // Instruções de branch, ele lê um byte
    // De memoria e o interpreta como deslocamento
    // Assinado de 8 bits
    // As intrucoes de branch não conseguem acessar qualquer
    // Localização da no adress range, eles so conseguem
    // Acessar um endereço relativo ao endereço atual
    // Num range de 127 localizações, pois é um endereço
    // Relativo, diferente dos demais
    pub fn REL(&mut self) -> u8 {
        self.addr_rel = self.read(self.pc) as u16;
        self.pc += 1;
        if (self.addr_rel & 0x80) != 0 {
            self.addr_rel |= 0xFF00;
        }
        0
    }
    //==========================//
    //#      Instruções        #//
    //==========================//

    // Fetch
    pub fn fetch(&mut self) -> u8 {
        if self.lookup[self.opcode as usize].addrmode == Olc6502::IMP {
            self.fetched = self.read(self.addr_abs);
        }
        self.fetched
    }

    // AND: And
    pub fn AND(&mut self) -> u8 {
        self.fetch();
        self.a = self.a & self.fetched;
        self.setFlag(FLAGS6502::Z, self.a == 0x00);
        self.setFlag(FLAGS6502::N, (self.a & 0x80) != 0);
        
        return 1;
    }

    // BCS: Branch Carry Set
    pub fn BCS(&mut self) -> u8 {
        if self.getFlag(FLAGS6502::C) == 1 {
            self.cycles += 1;
            self.addr_abs = self.pc + self.addr_rel;

            if (self.addr_abs & 0xFF00) != (self.pc & 0xFF00) {
                self.cycles += 1;
            }
            self.pc = self.addr_abs;
        }
        return 0;
    }
    
    // BCC: Branch Carry Clear
    pub fn BCC(&mut self) -> u8 {
        if self.getFlag(FLAGS6502::C) == 0 {
            self.cycles += 1;
            self.addr_abs = self.pc + self.addr_rel;
            if (self.addr_abs & 0xFF00) != (self.pc & 0xFF00) {
                self.cycles += 1;
            }
            self.pc = self.addr_abs;
        }
        return 0;
    }

    // BEQ: Branch Equal
    pub fn BEQ(&mut self) -> u8 {
        if self.getFlag(FLAGS6502::Z) == 1 {
            self.cycles += 1;
            self.addr_abs = self.pc + self.addr_rel;
            if (self.addr_abs & 0xFF00) != (self.pc & 0xFF00) {
                self.cycles += 1;
            }
            self.pc = self.addr_abs;
        }
        return 0;
    }

    // BMI: Branch Minus
    pub fn BMI(&mut self) -> u8 {
        if self.getFlag(FLAGS6502::N) == 1 {
            self.cycles += 1;
            self.addr_abs = self.pc + self.addr_rel;
            if (self.addr_abs & 0xFF00) != (self.pc & 0xFF00) {
                self.cycles += 1;
            }
            self.pc = self.addr_abs;
        }
        return 0;
    }

    // BME: Branch Not Equal
    pub fn BNE(&mut self) -> u8 {
        if self.getFlag(FLAGS6502::Z) == 0 {
            self.cycles += 1;
            self.addr_abs = self.pc + self.addr_rel;
            if (self.addr_abs & 0xFF00) != (self.pc & 0xFF00) {
                self.cycles += 1;
            }
            self.pc = self.addr_abs;
        }
        return 0;
    }

    // BPL: Branch Plus
    pub fn BPL(&mut self) -> u8 {
        if self.getFlag(FLAGS6502::N) == 0 {
            self.cycles += 1;
            self.addr_abs = self.pc + self.addr_rel;
            if (self.addr_abs & 0xFF00) != (self.pc & 0xFF00) {
                self.cycles += 1;
            }
            self.pc = self.addr_abs;
        }
        return 0;
    }

    // BVC: Branch Overflow
    pub fn BVC(&mut self) -> u8 {
        if self.getFlag(FLAGS6502::V) == 0 {
            self.cycles += 1;
            self.addr_abs = self.pc + self.addr_rel;
            if (self.addr_abs & 0xFF00) != (self.pc & 0xFF00) {
                self.cycles += 1;
            }
            self.pc = self.addr_abs;
        }
        return 0;
    }

    // BVS: Branch Not Overflow
    pub fn BVS(&mut self) -> u8 {
        if self.getFlag(FLAGS6502::V) == 1 {
            self.cycles += 1;
            self.addr_abs = self.pc + self.addr_rel;
            if (self.addr_abs & 0xFF00) != (self.pc & 0xFF00) {
                self.cycles += 1;
            }
            self.pc = self.addr_abs;
        }
        return 0;
    }

    // CLC: Clear Carry Bit
    pub fn CLC(&mut self) -> u8 {
        self.setFlag(FLAGS6502::C, false);
        return 0;
    }
    // CLD: Clear Decimal Mode
    // Function: D = 0
    pub fn CLD(&mut self) -> u8 {
        self.setFlag(FLAGS6502::D, false);
        0
    }

    // CLI: Clear Interrupt Disable
    // Function: I = 0
    pub fn CLI(&mut self) -> u8 {
        self.setFlag(FLAGS6502::I, false);
        0
    }

    // CLV: Clear Overflow Flag
    // Function: V = 0
    pub fn CLV(&mut self) -> u8 {
        self.setFlag(FLAGS6502::V, false);
        0
    }

    // CMP: Compare Accumulator
    // Function: Compare A with fetched value
    // Flags Out: C, Z, N
    pub fn CMP(&mut self) -> u8 {
        self.fetch();
        self.temp = (self.a as u16).wrapping_sub(self.fetched as u16);
        self.setFlag(FLAGS6502::C, self.a >= self.fetched);
        self.setFlag(FLAGS6502::Z, (self.temp & 0x00FF) == 0);
        self.setFlag(FLAGS6502::N, self.temp & 0x0080 != 0);
        1
    }

    // CPX: Compare X Register
    // Function: Compare X with fetched value
    // Flags Out: C, Z, N
    pub fn CPX(&mut self) -> u8 {
        self.fetch();
        self.temp = (self.x as u16).wrapping_sub(self.fetched as u16);
        self.setFlag(FLAGS6502::C, self.x >= self.fetched);
        self.setFlag(FLAGS6502::Z, (self.temp & 0x00FF) == 0);
        self.setFlag(FLAGS6502::N, self.temp & 0x0080 != 0);
        0
    }

    // CPY: Compare Y Register
    // Function: Compare Y with fetched value
    // Flags Out: C, Z, N
    pub fn CPY(&mut self) -> u8 {
        self.fetch();
        self.temp = (self.y as u16).wrapping_sub(self.fetched as u16);
        self.setFlag(FLAGS6502::C, self.y >= self.fetched);
        self.setFlag(FLAGS6502::Z, (self.temp & 0x00FF) == 0);
        self.setFlag(FLAGS6502::N, self.temp & 0x0080 != 0);
        0
    }

    // ADC: Add with Carry
    // Function: Add memory value to accumulator with carry
    // Flags Out: C, Z, V, N
    pub fn ADC(&mut self) -> u8 {
        self.fetch();
        self.temp = (self.a as u16).wrapping_add(self.fetched as u16);
        if self.getFlag(FLAGS6502::C) {
            self.temp = self.temp.wrapping_add(1);
        }
        self.setFlag(FLAGS6502::C, self.temp > 0x00FF);
        self.setFlag(FLAGS6502::Z, (self.temp & 0x00FF) == 0);
        self.setFlag(FLAGS6502::N, self.temp & 0x0080 != 0);
        self.a = (self.temp & 0x00FF) as u8;
        1
    }


    //==========================//
    //#         Opcodes        //#
    //==========================//
    pub fn ADC(&mut self) -> u8 {0}
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
    pub fn ORA(&mut self) -> u8 {0}   pub fn PHP(&mut self) -> u8 {0}
    pub fn ROL(&mut self) -> u8 {0}   pub fn PLP(&mut self) -> u8 {0}
    pub fn SEC(&mut self) -> u8 {0}   pub fn RTI(&mut self) -> u8 {0}
    pub fn PHA(&mut self) -> u8 {0}   pub fn RTS(&mut self) -> u8 {0}
    pub fn SEI(&mut self) -> u8 {0}   pub fn ROR(&mut self) -> u8 {0} 
    pub fn PLA(&mut self) -> u8 {0}   pub fn STA(&mut self) -> u8 {0} 
    pub fn STY(&mut self) -> u8 {0}   pub fn STX(&mut self) -> u8 {0} 
    pub fn TXS(&mut self) -> u8 {0}   pub fn TAY(&mut self) -> u8 {0} 
    pub fn TAX(&mut self) -> u8 {0}   pub fn SBC(&mut self) -> u8 {0}
    pub fn TXA(&mut self) -> u8 {0}   pub fn TYA(&mut self) -> u8 {0}
    pub fn TSX(&mut self) -> u8 {0}   pub fn SED(&mut self) -> u8 {0}
    pub fn XXX(&mut self) -> u8 {0}   // Nao Implementado

    // Clock
    pub fn clock(&mut self) {
        if self.cycles == 0 {
            self.opcode = self.read(self.pc);
            self.pc += 1;

            self.cycles = self.lookup[self.opcode as usize].cycles;
            let add_cycle0 = self.lookup[self.opcode as usize].addrmode;
            let add_cycle1 = self.lookup[self.opcode as usize].operate;
            
        }
        self.cycles -= 1;
    }
    // Reset
    pub fn reset() {

    }
    // Interruptiuon Request
    pub fn irq() {}
    // Non Maskable Interrupt
    pub fn nmi() {}
    // Temporary
    pub fn temp() -> u16 { 0x0000 } // Uma variavel pra usar pra qualquer coisa temporariamente


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
