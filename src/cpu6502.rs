use crate::bus::Bus;
use std::cell::RefCell;
use std::rc::Rc;

// Flags Cpu6502
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

pub struct Cpu6502 {
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
    pub operate: fn(&mut Cpu6502) -> u8,              // Ponteiro para a função de operação
    pub addrmode: fn(&mut Cpu6502) -> u8,             // Ponteiro para a função de modo de endereçamento
    pub cycles: u8,                                   // Ciclos necessários para a instrução
}
//Vector<Instruction> lookup

impl Cpu6502 {
    pub fn new() -> Self {
        Cpu6502 { 
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
            lookup: Cpu6502::instrucoes(),  // Lookup table para uinstrucoes da cpu
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
        self.addr_abs = self.pc;
        self.pc += 1;
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
        self.addr_abs = base.wrapping_add(self.x) as u16;
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
    // Memória para formar um en1dreço de 16 bits
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

    pub fn ACC(&mut self) -> u8 {
        self.addr_abs = self.pc;
        0
    }
    //==========================//
    //#      Instruções        #//
    //==========================//

    // Fetch
    pub fn fetch(&mut self) -> u8 {
        if self.lookup[self.opcode as usize].addrmode == Cpu6502::IMP {
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
    pub fn CLD(&mut self) -> u8 {
        self.setFlag(FLAGS6502::D, false);
        0
    }

    // CLI: Clear Interrupt Disable
    pub fn CLI(&mut self) -> u8 {
        self.setFlag(FLAGS6502::I, false);
        0
    }

    // CLV: Clear Overflow Flag
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
        self.temp as u16 = (self.a as u16) + (self.fetched as u16) + (self.getFlag(FLAGS6502::C) as u16);
        self.setFlag(FLAGS6502::C, self.temp > 255);
        self.setFlag(FLAGS6502::Z, (self.temp & 0x00FF) == 0);
        self.setFlag(FLAGS6502::N, self.temp & 0x0080 != 0);
        self.setFlag(FLAGS6502::V, ((self.a ^ self.fetched) & 0x80 != 0) && ((self.a ^ self.temp) & 0x80 != 0));
        self.a = (self.temp & 0x00FF) as u8;
        /*        self.temp = (self.a as u16).wrapping_add(self.fetched as u16); 
        if self.getFlag(FLAGS6502::C) {
            self.temp = self.temp.wrapping_add(1);
        }
        self.setFlag(FLAGS6502::C, self.temp > 0x00FF);
        self.setFlag(FLAGS6502::Z, (self.temp & 0x00FF) == 0);
        self.setFlag(FLAGS6502::N, self.temp & 0x0080 != 0);
        self.a = (self.temp & 0x00FF) as u8;
i dont remenber what i did (o fuck i murdered english) in that, so i will comment and do again
*/
        1
    }

    // SBC: Subtract with Carry
    // Subtrai um valor da memoria com o acumulador
    // Flags Out: C, Z, V, N
    pub fn SBC(&mut self) -> u8 {
        self.fetch();
        let value = (self.fetched as u16) ^ 0x00FF;
        let temp = (self.a as u16) + value + (self.getFlag(FLAGS6502::C) as u16);

        self.setFlag(FLAGS6502::C, temp > 0xFF);
        self.setFlag(FLAGS6502::Z, (temp & 0x00FF) == 0);
        self.setFlag(FLAGS6502::V, ((temp ^ (self.a as u16)) & (temp ^ value) & 0x80) != 0);
        self.setFlag(FLAGS6502::N, (temp & 0x80) != 0);

        self.a = (temp & 0x00FF) as u8;
        1
    }
    // PHA: Push Accumulator
    // Funcão: Coloca o valor do acumulador no stack
    pub fn PHA(&mut self) -> u8 {
        self.write(0x100 + self.stkp, self.a);
        self.stkp -= 1;
        0
    }

    // PLA: Pull Accumulator
    // Funcão: Pega o valor do stack e coloca no acumulador
    pub fn PLA(&mut self) -> u8 {
        self.stkp += 1;
        self.a = self.read(0x100 + self.stkp);
        self.setFlag(FLAGS6502::Z, self.a == 0x00);
        self.setFlag(FLAGS6502::N, self.a & 0x80 != 0);
        0
    }
    
    // RTI: Return from Interrupt
    pub fn RTI(&mut self) -> u8 {
        self.stkp += 1;
        self.status = self.read(0x0100 + self.stkp as u16);
        self.status &= !(FLAGS6502::B as u8);
        self.status &= !(FLAGS6502::U as u8);
        
        self.stkp += 1;
        let lo = self.read(0x0100 + self.stkp as u16);
        
        self.stkp += 1;
        let hi = self.read(0x0100 + self.stkp as u16);
        
        // Combine low and high bytes to form 16-bit program counter
        self.pc = ((hi as u16) << 8) | (lo as u16);
        
        0
    }
    //===========================================//
    //#         Opcodes Nao Implementados       #//
    //===========================================//
    pub fn ASL(&mut self) -> u8 {0}
    pub fn BIT(&mut self) -> u8 {0}
    pub fn BRK(&mut self) -> u8 {0}
    pub fn DEC(&mut self) -> u8 {0}
    pub fn DEX(&mut self) -> u8 {0}
    pub fn DEY(&mut self) -> u8 {0}
    pub fn EOR(&mut self) -> u8 {0}
    pub fn INC(&mut self) -> u8 {0}
    pub fn INX(&mut self) -> u8 {0}
    pub fn INY(&mut self) -> u8 {0}
    pub fn JMP(&mut self) -> u8 {0}
    pub fn JSR(&mut self) -> u8 {0}
    pub fn LDA(&mut self) -> u8 {0}
    pub fn LDX(&mut self) -> u8 {0}
    pub fn LDY(&mut self) -> u8 {0}
    pub fn LSR(&mut self) -> u8 {0}
    pub fn NOP(&mut self) -> u8 {0}
    pub fn ORA(&mut self) -> u8 {0}
    pub fn PHP(&mut self) -> u8 {0}
    pub fn ROL(&mut self) -> u8 {0}
    pub fn PLP(&mut self) -> u8 {0}
    pub fn SEC(&mut self) -> u8 {0}
    pub fn RTS(&mut self) -> u8 {0}
    pub fn SEI(&mut self) -> u8 {0}
    pub fn ROR(&mut self) -> u8 {0}
    pub fn STA(&mut self) -> u8 {0}
    pub fn STY(&mut self) -> u8 {0}
    pub fn STX(&mut self) -> u8 {0}
    pub fn TXS(&mut self) -> u8 {0}
    pub fn TAY(&mut self) -> u8 {0}
    pub fn TAX(&mut self) -> u8 {0}
    pub fn TXA(&mut self) -> u8 {0}
    pub fn TYA(&mut self) -> u8 {0}
    pub fn TSX(&mut self) -> u8 {0}
    pub fn SED(&mut self) -> u8 {0}
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
    pub fn reset(&mut self) {
        self.a = 0;
        self.x = 0;
        self.y = 0;
        self.stkp = 0xFD;
        self.status = 0x00 | FLAGS6502::U as u8;
        
        self.addr_abs = 0xFFFC;
        let lo = self.read(self.addr_abs + 0);
        let hi = self.read(self.addr_abs + 1);

        self.pc = ((hi as u16) << 8) | (lo as u16);

        self.addr_rel = 0x0000;
        self.addr_abs = 0x0000;
        self.fetched = 0x00;

        self.cycles = 8;
    }
    // Interruptiuon Request
    pub fn irq(&mut self) {
        if self.getFlag(FLAGS6502::I) == 0 {
            self.write(0x0100 + self.stkp, (self.pc >> 8) & 0x00FF);
            self.stkp -= 1;
            self.write(0x0100 + self.stkp, self.pc as u8);
            self.stkp -= 1;

            self.addr_abs = 0xFFFE;
            let lo = self.read(self.addr_abs + 0);
            let hi = self.read(self.addr_abs + 1);

            self.pc = ((hi as u16) << 8) | (lo as u16);
            self.cycles = 7;
        }
    }
    // Non Maskable Interrupt
    pub fn nmi(&mut self) {
        self.write(0x0100 + self.stkp as u16, (self.pc >> 8) & 0x00FF);
        self.stkp -= 1;
        self.write(0x0100 + self.stkp as u16, self.pc & 0x00FF);
        self.stkp -= 1;

        self.setFlag(FLAGS6502::B, false);
        self.setFlag(FLAGS6502::U, true);
        self.setFlag(FLAGS6502::I, true);
        self.write(0x0100 + self.stkp as u16, self.status);
        self.stkp -= 1;

        self.addr_abs = 0xFFFA;
        let lo = self.read(self.addr_abs + 0);
        let hi = self.read(self.addr_abs + 1);
        self.pc = ((hi as u16) << 8) | (lo as u16);

        self.cycles = 8;
    }
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
            Instruction { name: "BRK", operate: Cpu6502::BRK, addrmode: Cpu6502::IMP, cycles: 7 },
            Instruction { name: "ORA", operate: Cpu6502::ORA, addrmode: Cpu6502::IZX, cycles: 6 },
            Instruction { name: "???", operate: Cpu6502::XXX, addrmode: Cpu6502::IMP, cycles: 2 },
            Instruction { name: "???", operate: Cpu6502::XXX, addrmode: Cpu6502::IMP, cycles: 8 },
            Instruction { name: "???", operate: Cpu6502::NOP, addrmode: Cpu6502::IMP, cycles: 3 },
            Instruction { name: "ORA", operate: Cpu6502::ORA, addrmode: Cpu6502::ZP0, cycles: 3 },
            Instruction { name: "ASL", operate: Cpu6502::ASL, addrmode: Cpu6502::ZP0, cycles: 5 },
            Instruction { name: "???", operate: Cpu6502::XXX, addrmode: Cpu6502::IMP, cycles: 5 },
            Instruction { name: "PHP", operate: Cpu6502::PHP, addrmode: Cpu6502::IMP, cycles: 3 },
            Instruction { name: "ORA", operate: Cpu6502::ORA, addrmode: Cpu6502::IMM, cycles: 2 },
            Instruction { name: "ASL", operate: Cpu6502::ASL, addrmode: Cpu6502::IMP, cycles: 2 },
            Instruction { name: "???", operate: Cpu6502::XXX, addrmode: Cpu6502::IMP, cycles: 2 },
            Instruction { name: "???", operate: Cpu6502::NOP, addrmode: Cpu6502::IMP, cycles: 4 },
            Instruction { name: "ORA", operate: Cpu6502::ORA, addrmode: Cpu6502::ABS, cycles: 4 },
            Instruction { name: "ASL", operate: Cpu6502::ASL, addrmode: Cpu6502::ABS, cycles: 6 },
            Instruction { name: "???", operate: Cpu6502::XXX, addrmode: Cpu6502::IMP, cycles: 6 },
    
            // 0x10
            Instruction { name: "BPL", operate: Cpu6502::BPL, addrmode: Cpu6502::REL, cycles: 2 },
            Instruction { name: "ORA", operate: Cpu6502::ORA, addrmode: Cpu6502::IZY, cycles: 5 },
            Instruction { name: "???", operate: Cpu6502::XXX, addrmode: Cpu6502::IMP, cycles: 2 },
            Instruction { name: "???", operate: Cpu6502::XXX, addrmode: Cpu6502::IMP, cycles: 8 },
            Instruction { name: "???", operate: Cpu6502::NOP, addrmode: Cpu6502::IMP, cycles: 4 },
            Instruction { name: "ORA", operate: Cpu6502::ORA, addrmode: Cpu6502::ZPX, cycles: 4 },
            Instruction { name: "ASL", operate: Cpu6502::ASL, addrmode: Cpu6502::ZPX, cycles: 6 },
            Instruction { name: "???", operate: Cpu6502::XXX, addrmode: Cpu6502::IMP, cycles: 6 },
            Instruction { name: "CLC", operate: Cpu6502::CLC, addrmode: Cpu6502::IMP, cycles: 2 },
            Instruction { name: "ORA", operate: Cpu6502::ORA, addrmode: Cpu6502::ABY, cycles: 4 },
            Instruction { name: "???", operate: Cpu6502::NOP, addrmode: Cpu6502::IMP, cycles: 2 },
            Instruction { name: "???", operate: Cpu6502::XXX, addrmode: Cpu6502::IMP, cycles: 7 },
            Instruction { name: "???", operate: Cpu6502::NOP, addrmode: Cpu6502::IMP, cycles: 4 },
            Instruction { name: "ORA", operate: Cpu6502::ORA, addrmode: Cpu6502::ABX, cycles: 4 },
            Instruction { name: "ASL", operate: Cpu6502::ASL, addrmode: Cpu6502::ABX, cycles: 7 },
            Instruction { name: "???", operate: Cpu6502::XXX, addrmode: Cpu6502::IMP, cycles: 7 },
    
            // 0x20
            Instruction { name: "JSR", operate: Cpu6502::JSR, addrmode: Cpu6502::ABS, cycles: 6 },
            Instruction { name: "AND", operate: Cpu6502::AND, addrmode: Cpu6502::IZX, cycles: 6 },
            Instruction { name: "???", operate: Cpu6502::XXX, addrmode: Cpu6502::IMP, cycles: 2 },
            Instruction { name: "???", operate: Cpu6502::XXX, addrmode: Cpu6502::IMP, cycles: 8 },
            Instruction { name: "BIT", operate: Cpu6502::BIT, addrmode: Cpu6502::ZP0, cycles: 3 },
            Instruction { name: "AND", operate: Cpu6502::AND, addrmode: Cpu6502::ZP0, cycles: 3 },
            Instruction { name: "ROL", operate: Cpu6502::ROL, addrmode: Cpu6502::ZP0, cycles: 5 },
            Instruction { name: "???", operate: Cpu6502::XXX, addrmode: Cpu6502::IMP, cycles: 5 },
            Instruction { name: "PLP", operate: Cpu6502::PLP, addrmode: Cpu6502::IMP, cycles: 4 },
            Instruction { name: "AND", operate: Cpu6502::AND, addrmode: Cpu6502::IMM, cycles: 2 },
            Instruction { name: "ROL", operate: Cpu6502::ROL, addrmode: Cpu6502::IMP, cycles: 2 },
            Instruction { name: "???", operate: Cpu6502::XXX, addrmode: Cpu6502::IMP, cycles: 2 },
            Instruction { name: "BIT", operate: Cpu6502::BIT, addrmode: Cpu6502::ABS, cycles: 4 },
            Instruction { name: "AND", operate: Cpu6502::AND, addrmode: Cpu6502::ABS, cycles: 4 },
            Instruction { name: "ROL", operate: Cpu6502::ROL, addrmode: Cpu6502::ABS, cycles: 6 },
            Instruction { name: "???", operate: Cpu6502::XXX, addrmode: Cpu6502::IMP, cycles: 6 },
    
            // 0x30
            Instruction { name: "BMI", operate: Cpu6502::BMI, addrmode: Cpu6502::REL, cycles: 2 },
            Instruction { name: "AND", operate: Cpu6502::AND, addrmode: Cpu6502::IZY, cycles: 5 },
            Instruction { name: "???", operate: Cpu6502::XXX, addrmode: Cpu6502::IMP, cycles: 2 },
            Instruction { name: "???", operate: Cpu6502::XXX, addrmode: Cpu6502::IMP, cycles: 8 },
            Instruction { name: "???", operate: Cpu6502::NOP, addrmode: Cpu6502::IMP, cycles: 4 },
            Instruction { name: "AND", operate: Cpu6502::AND, addrmode: Cpu6502::ZPX, cycles: 4 },
            Instruction { name: "ROL", operate: Cpu6502::ROL, addrmode: Cpu6502::ZPX, cycles: 6 },
            Instruction { name: "???", operate: Cpu6502::XXX, addrmode: Cpu6502::IMP, cycles: 6 },
            Instruction { name: "SEC", operate: Cpu6502::SEC, addrmode: Cpu6502::IMP, cycles: 2 },
            Instruction { name: "AND", operate: Cpu6502::AND, addrmode: Cpu6502::ABY, cycles: 4 },
            Instruction { name: "???", operate: Cpu6502::NOP, addrmode: Cpu6502::IMP, cycles: 2 },
            Instruction { name: "???", operate: Cpu6502::XXX, addrmode: Cpu6502::IMP, cycles: 7 },
            Instruction { name: "???", operate: Cpu6502::NOP, addrmode: Cpu6502::IMP, cycles: 4 },
            Instruction { name: "AND", operate: Cpu6502::AND, addrmode: Cpu6502::ABX, cycles: 4 },
            Instruction { name: "ROL", operate: Cpu6502::ROL, addrmode: Cpu6502::ABX, cycles: 7 },
            Instruction { name: "???", operate: Cpu6502::XXX, addrmode: Cpu6502::IMP, cycles: 7 },
    
            // 0x40
            Instruction { name: "RTI", operate: Cpu6502::RTI, addrmode: Cpu6502::IMP, cycles: 6 },
            Instruction { name: "EOR", operate: Cpu6502::EOR, addrmode: Cpu6502::IZX, cycles: 6 },
            Instruction { name: "???", operate: Cpu6502::XXX, addrmode: Cpu6502::IMP, cycles: 2 },
            Instruction { name: "???", operate: Cpu6502::XXX, addrmode: Cpu6502::IMP, cycles: 8 },
            Instruction { name: "???", operate: Cpu6502::NOP, addrmode: Cpu6502::IMP, cycles: 3 },
            Instruction { name: "EOR", operate: Cpu6502::EOR, addrmode: Cpu6502::ZP0, cycles: 3 },
            Instruction { name: "LSR", operate: Cpu6502::LSR, addrmode: Cpu6502::ZP0, cycles: 5 },
            Instruction { name: "???", operate: Cpu6502::XXX, addrmode: Cpu6502::IMP, cycles: 5 },
            Instruction { name: "PHA", operate: Cpu6502::PHA, addrmode: Cpu6502::IMP, cycles: 3 },
            Instruction { name: "EOR", operate: Cpu6502::EOR, addrmode: Cpu6502::IMM, cycles: 2 },
            Instruction { name: "LSR", operate: Cpu6502::LSR, addrmode: Cpu6502::IMP, cycles: 2 },
            Instruction { name: "???", operate: Cpu6502::XXX, addrmode: Cpu6502::IMP, cycles: 2 },
            Instruction { name: "JMP", operate: Cpu6502::JMP, addrmode: Cpu6502::ABS, cycles: 3 },
            Instruction { name: "EOR", operate: Cpu6502::EOR, addrmode: Cpu6502::ABS, cycles: 4 },
            Instruction { name: "LSR", operate: Cpu6502::LSR, addrmode: Cpu6502::ABS, cycles: 6 },
            Instruction { name: "???", operate: Cpu6502::XXX, addrmode: Cpu6502::IMP, cycles: 6 },
    
            // 0x50
            Instruction { name: "BVC", operate: Cpu6502::BVC, addrmode: Cpu6502::REL, cycles: 2 },
            Instruction { name: "EOR", operate: Cpu6502::EOR, addrmode: Cpu6502::IZY, cycles: 5 },
            Instruction { name: "???", operate: Cpu6502::XXX, addrmode: Cpu6502::IMP, cycles: 2 },
            Instruction { name: "???", operate: Cpu6502::XXX, addrmode: Cpu6502::IMP, cycles: 8 },
            Instruction { name: "???", operate: Cpu6502::NOP, addrmode: Cpu6502::IMP, cycles: 4 },
            Instruction { name: "EOR", operate: Cpu6502::EOR, addrmode: Cpu6502::ZPX, cycles: 4 },
            Instruction { name: "LSR", operate: Cpu6502::LSR, addrmode: Cpu6502::ZPX, cycles: 6 },
            Instruction { name: "???", operate: Cpu6502::XXX, addrmode: Cpu6502::IMP, cycles: 6 },
            Instruction { name: "CLI", operate: Cpu6502::CLI, addrmode: Cpu6502::IMP, cycles: 2 },
            Instruction { name: "EOR", operate: Cpu6502::EOR, addrmode: Cpu6502::ABY, cycles: 4 },
            Instruction { name: "???", operate: Cpu6502::NOP, addrmode: Cpu6502::IMP, cycles: 2 },
            Instruction { name: "???", operate: Cpu6502::XXX, addrmode: Cpu6502::IMP, cycles: 7 },
            Instruction { name: "???", operate: Cpu6502::NOP, addrmode: Cpu6502::IMP, cycles: 4 },
            Instruction { name: "EOR", operate: Cpu6502::EOR, addrmode: Cpu6502::ABX, cycles: 4 },
            Instruction { name: "LSR", operate: Cpu6502::LSR, addrmode: Cpu6502::ABX, cycles: 7 },
            Instruction { name: "???", operate: Cpu6502::XXX, addrmode: Cpu6502::IMP, cycles: 7 },
    
            // 0x60
            Instruction { name: "RTS", operate: Cpu6502::RTS, addrmode: Cpu6502::IMP, cycles: 6 },
            Instruction { name: "ADC", operate: Cpu6502::ADC, addrmode: Cpu6502::IZX, cycles: 6 },
            Instruction { name: "???", operate: Cpu6502::XXX, addrmode: Cpu6502::IMP, cycles: 2 },
            Instruction { name: "???", operate: Cpu6502::XXX, addrmode: Cpu6502::IMP, cycles: 8 },
            Instruction { name: "???", operate: Cpu6502::NOP, addrmode: Cpu6502::IMP, cycles: 3 },
            Instruction { name: "ADC", operate: Cpu6502::ADC, addrmode: Cpu6502::ZP0, cycles: 3 },
            Instruction { name: "ROR", operate: Cpu6502::ROR, addrmode: Cpu6502::ZP0, cycles: 5 },
            Instruction { name: "???", operate: Cpu6502::XXX, addrmode: Cpu6502::IMP, cycles: 5 },
            Instruction { name: "PLA", operate: Cpu6502::PLA, addrmode: Cpu6502::IMP, cycles: 4 },
            Instruction { name: "ADC", operate: Cpu6502::ADC, addrmode: Cpu6502::IMM, cycles: 2 },
            Instruction { name: "ROR", operate: Cpu6502::ROR, addrmode: Cpu6502::IMP, cycles: 2 },
            Instruction { name: "???", operate: Cpu6502::XXX, addrmode: Cpu6502::IMP, cycles: 2 },
            Instruction { name: "JMP", operate: Cpu6502::JMP, addrmode: Cpu6502::IND, cycles: 5 },
            Instruction { name: "ADC", operate: Cpu6502::ADC, addrmode: Cpu6502::ABS, cycles: 4 },
            Instruction { name: "ROR", operate: Cpu6502::ROR, addrmode: Cpu6502::ABS, cycles: 6 },
            Instruction { name: "???", operate: Cpu6502::XXX, addrmode: Cpu6502::IMP, cycles: 6 },
    
            // 0x70
            Instruction { name: "BVS", operate: Cpu6502::BVS, addrmode: Cpu6502::REL, cycles: 2 },
            Instruction { name: "ADC", operate: Cpu6502::ADC, addrmode: Cpu6502::IZY, cycles: 5 },
            Instruction { name: "???", operate: Cpu6502::XXX, addrmode: Cpu6502::IMP, cycles: 2 },
            Instruction { name: "???", operate: Cpu6502::XXX, addrmode: Cpu6502::IMP, cycles: 8 },
            Instruction { name: "???", operate: Cpu6502::NOP, addrmode: Cpu6502::IMP, cycles: 4 },
            Instruction { name: "ADC", operate: Cpu6502::ADC, addrmode: Cpu6502::ZPX, cycles: 4 },
            Instruction { name: "ROR", operate: Cpu6502::ROR, addrmode: Cpu6502::ZPX, cycles: 6 },
            Instruction { name: "???", operate: Cpu6502::XXX, addrmode: Cpu6502::IMP, cycles: 6 },
            Instruction { name: "SEI", operate: Cpu6502::SEI, addrmode: Cpu6502::IMP, cycles: 2 },
            Instruction { name: "ADC", operate: Cpu6502::ADC, addrmode: Cpu6502::ABY, cycles: 4 },
            Instruction { name: "???", operate: Cpu6502::NOP, addrmode: Cpu6502::IMP, cycles: 2 },
            Instruction { name: "???", operate: Cpu6502::XXX, addrmode: Cpu6502::IMP, cycles: 7 },
            Instruction { name: "???", operate: Cpu6502::NOP, addrmode: Cpu6502::IMP, cycles: 4 },
            Instruction { name: "ADC", operate: Cpu6502::ADC, addrmode: Cpu6502::ABX, cycles: 4 },
            Instruction { name: "ROR", operate: Cpu6502::ROR, addrmode: Cpu6502::ABX, cycles: 7 },
            Instruction { name: "???", operate: Cpu6502::XXX, addrmode: Cpu6502::IMP, cycles: 7 },
    
            // 0x80
            Instruction { name: "???", operate: Cpu6502::NOP, addrmode: Cpu6502::IMP, cycles: 2 },
            Instruction { name: "STA", operate: Cpu6502::STA, addrmode: Cpu6502::IZX, cycles: 6 },
            Instruction { name: "???", operate: Cpu6502::NOP, addrmode: Cpu6502::IMP, cycles: 2 },
            Instruction { name: "???", operate: Cpu6502::XXX, addrmode: Cpu6502::IMP, cycles: 6 },
            Instruction { name: "STY", operate: Cpu6502::STY, addrmode: Cpu6502::ZP0, cycles: 3 },
            Instruction { name: "STA", operate: Cpu6502::STA, addrmode: Cpu6502::ZP0, cycles: 3 },
            Instruction { name: "STX", operate: Cpu6502::STX, addrmode: Cpu6502::ZP0, cycles: 3 },
            Instruction { name: "???", operate: Cpu6502::XXX, addrmode: Cpu6502::IMP, cycles: 3 },
            Instruction { name: "DEY", operate: Cpu6502::DEY, addrmode: Cpu6502::IMP, cycles: 2 },
            Instruction { name: "???", operate: Cpu6502::NOP, addrmode: Cpu6502::IMP, cycles: 2 },
            Instruction { name: "TXA", operate: Cpu6502::TXA, addrmode: Cpu6502::IMP, cycles: 2 },
            Instruction { name: "???", operate: Cpu6502::XXX, addrmode: Cpu6502::IMP, cycles: 2 },
            Instruction { name: "STY", operate: Cpu6502::STY, addrmode: Cpu6502::ABS, cycles: 4 },
            Instruction { name: "STA", operate: Cpu6502::STA, addrmode: Cpu6502::ABS, cycles: 4 },
            Instruction { name: "STX", operate: Cpu6502::STX, addrmode: Cpu6502::ABS, cycles: 4 },
            Instruction { name: "???", operate: Cpu6502::XXX, addrmode: Cpu6502::IMP, cycles: 4 },
    
            // 0x90
            Instruction { name: "BCC", operate: Cpu6502::BCC, addrmode: Cpu6502::REL, cycles: 2 },
            Instruction { name: "STA", operate: Cpu6502::STA, addrmode: Cpu6502::IZY, cycles: 6 },
            Instruction { name: "???", operate: Cpu6502::XXX, addrmode: Cpu6502::IMP, cycles: 2 },
            Instruction { name: "???", operate: Cpu6502::XXX, addrmode: Cpu6502::IMP, cycles: 6 },
            Instruction { name: "STY", operate: Cpu6502::STY, addrmode: Cpu6502::ZPX, cycles: 4 },
            Instruction { name: "STA", operate: Cpu6502::STA, addrmode: Cpu6502::ZPX, cycles: 4 },
            Instruction { name: "STX", operate: Cpu6502::STX, addrmode: Cpu6502::ZPY, cycles: 4 },
            Instruction { name: "???", operate: Cpu6502::XXX, addrmode: Cpu6502::IMP, cycles: 4 },
            Instruction { name: "TYA", operate: Cpu6502::TYA, addrmode: Cpu6502::IMP, cycles: 2 },
            Instruction { name: "STA", operate: Cpu6502::STA, addrmode: Cpu6502::ABY, cycles: 5 },
            Instruction { name: "TXS", operate: Cpu6502::TXS, addrmode: Cpu6502::IMP, cycles: 2 },
            Instruction { name: "???", operate: Cpu6502::XXX, addrmode: Cpu6502::IMP, cycles: 5 },
            Instruction { name: "???", operate: Cpu6502::NOP, addrmode: Cpu6502::IMP, cycles: 5 },
            Instruction { name: "STA", operate: Cpu6502::STA, addrmode: Cpu6502::ABX, cycles: 5 },
            Instruction { name: "???", operate: Cpu6502::XXX, addrmode: Cpu6502::IMP, cycles: 5 },
            Instruction { name: "???", operate: Cpu6502::XXX, addrmode: Cpu6502::IMP, cycles: 5 },
    
            // 0xA0
            Instruction { name: "LDY", operate: Cpu6502::LDY, addrmode: Cpu6502::IMM, cycles: 2 },
            Instruction { name: "LDA", operate: Cpu6502::LDA, addrmode: Cpu6502::IZX, cycles: 6 },
            Instruction { name: "LDX", operate: Cpu6502::LDX, addrmode: Cpu6502::IMM, cycles: 2 },
            Instruction { name: "???", operate: Cpu6502::XXX, addrmode: Cpu6502::IMP, cycles: 6 },
            Instruction { name: "LDY", operate: Cpu6502::LDY, addrmode: Cpu6502::ZP0, cycles: 3 },
            Instruction { name: "LDA", operate: Cpu6502::LDA, addrmode: Cpu6502::ZP0, cycles: 3 },
            Instruction { name: "LDX", operate: Cpu6502::LDX, addrmode: Cpu6502::ZP0, cycles: 3 },
            Instruction { name: "???", operate: Cpu6502::XXX, addrmode: Cpu6502::IMP, cycles: 3 },
            Instruction { name: "TAY", operate: Cpu6502::TAY, addrmode: Cpu6502::IMP, cycles: 2 },
            Instruction { name: "LDA", operate: Cpu6502::LDA, addrmode: Cpu6502::IMM, cycles: 2 },
            Instruction { name: "TAX", operate: Cpu6502::TAX, addrmode: Cpu6502::IMP, cycles: 2 },
            Instruction { name: "???", operate: Cpu6502::XXX, addrmode: Cpu6502::IMP, cycles: 2 },
            Instruction { name: "LDY", operate: Cpu6502::LDY, addrmode: Cpu6502::ABS, cycles: 4 },
            Instruction { name: "LDA", operate: Cpu6502::LDA, addrmode: Cpu6502::ABS, cycles: 4 },
            Instruction { name: "LDX", operate: Cpu6502::LDX, addrmode: Cpu6502::ABS, cycles: 4 },
            Instruction { name: "???", operate: Cpu6502::XXX, addrmode: Cpu6502::IMP, cycles: 4 },
    
            // 0xB0
            Instruction { name: "BCS", operate: Cpu6502::BCS, addrmode: Cpu6502::REL, cycles: 2 },
            Instruction { name: "LDA", operate: Cpu6502::LDA, addrmode: Cpu6502::IZY, cycles: 5 },
            Instruction { name: "???", operate: Cpu6502::XXX, addrmode: Cpu6502::IMP, cycles: 2 },
            Instruction { name: "???", operate: Cpu6502::XXX, addrmode: Cpu6502::IMP, cycles: 5 },
            Instruction { name: "LDY", operate: Cpu6502::LDY, addrmode: Cpu6502::ZPX, cycles: 4 },
            Instruction { name: "LDA", operate: Cpu6502::LDA, addrmode: Cpu6502::ZPX, cycles: 4 },
            Instruction { name: "LDX", operate: Cpu6502::LDX, addrmode: Cpu6502::ZPY, cycles: 4 },
            Instruction { name: "???", operate: Cpu6502::XXX, addrmode: Cpu6502::IMP, cycles: 4 },
            Instruction { name: "CLV", operate: Cpu6502::CLV, addrmode: Cpu6502::IMP, cycles: 2 },
            Instruction { name: "LDA", operate: Cpu6502::LDA, addrmode: Cpu6502::ABY, cycles: 4 },
            Instruction { name: "TSX", operate: Cpu6502::TSX, addrmode: Cpu6502::IMP, cycles: 2 },
            Instruction { name: "???", operate: Cpu6502::XXX, addrmode: Cpu6502::IMP, cycles: 4 },
            Instruction { name: "LDY", operate: Cpu6502::LDY, addrmode: Cpu6502::ABX, cycles: 4 },
            Instruction { name: "LDA", operate: Cpu6502::LDA, addrmode: Cpu6502::ABX, cycles: 4 },
            Instruction { name: "LDX", operate: Cpu6502::LDX, addrmode: Cpu6502::ABY, cycles: 4 },
            Instruction { name: "???", operate: Cpu6502::XXX, addrmode: Cpu6502::IMP, cycles: 4 },
    
            // 0xC0
            Instruction { name: "CPY", operate: Cpu6502::CPY, addrmode: Cpu6502::IMM, cycles: 2 },
            Instruction { name: "CMP", operate: Cpu6502::CMP, addrmode: Cpu6502::IZX, cycles: 6 },
            Instruction { name: "???", operate: Cpu6502::NOP, addrmode: Cpu6502::IMP, cycles: 2 },
            Instruction { name: "???", operate: Cpu6502::XXX, addrmode: Cpu6502::IMP, cycles: 8 },
            Instruction { name: "CPY", operate: Cpu6502::CPY, addrmode: Cpu6502::ZP0, cycles: 3 },
            Instruction { name: "CMP", operate: Cpu6502::CMP, addrmode: Cpu6502::ZP0, cycles: 3 },
            Instruction { name: "DEC", operate: Cpu6502::DEC, addrmode: Cpu6502::ZP0, cycles: 5 },
            Instruction { name: "???", operate: Cpu6502::XXX, addrmode: Cpu6502::IMP, cycles: 5 },
            Instruction { name: "INY", operate: Cpu6502::INY, addrmode: Cpu6502::IMP, cycles: 2 },
            Instruction { name: "CMP", operate: Cpu6502::CMP, addrmode: Cpu6502::IMM, cycles: 2 },
            Instruction { name: "DEX", operate: Cpu6502::DEX, addrmode: Cpu6502::IMP, cycles: 2 },
            Instruction { name: "???", operate: Cpu6502::XXX, addrmode: Cpu6502::IMP, cycles: 2 },
            Instruction { name: "CPY", operate: Cpu6502::CPY, addrmode: Cpu6502::ABS, cycles: 4 },
            Instruction { name: "CMP", operate: Cpu6502::CMP, addrmode: Cpu6502::ABS, cycles: 4 },
            Instruction { name: "DEC", operate: Cpu6502::DEC, addrmode: Cpu6502::ABS, cycles: 6 },
            Instruction { name: "???", operate: Cpu6502::XXX, addrmode: Cpu6502::IMP, cycles: 6 },
    
            // 0xD0
            Instruction { name: "BNE", operate: Cpu6502::BNE, addrmode: Cpu6502::REL, cycles: 2 },
            Instruction { name: "CMP", operate: Cpu6502::CMP, addrmode: Cpu6502::IZY, cycles: 5 },
            Instruction { name: "???", operate: Cpu6502::XXX, addrmode: Cpu6502::IMP, cycles: 2 },
            Instruction { name: "???", operate: Cpu6502::XXX, addrmode: Cpu6502::IMP, cycles: 8 },
            Instruction { name: "???", operate: Cpu6502::NOP, addrmode: Cpu6502::IMP, cycles: 4 },
            Instruction { name: "CMP", operate: Cpu6502::CMP, addrmode: Cpu6502::ZPX, cycles: 4 },
            Instruction { name: "DEC", operate: Cpu6502::DEC, addrmode: Cpu6502::ZPX, cycles: 6 },
            Instruction { name: "???", operate: Cpu6502::XXX, addrmode: Cpu6502::IMP, cycles: 6 },
            Instruction { name: "CLD", operate: Cpu6502::CLD, addrmode: Cpu6502::IMP, cycles: 2 },
            Instruction { name: "CMP", operate: Cpu6502::CMP, addrmode: Cpu6502::ABY, cycles: 4 },
            Instruction { name: "NOP", operate: Cpu6502::NOP, addrmode: Cpu6502::IMP, cycles: 2 },
            Instruction { name: "???", operate: Cpu6502::XXX, addrmode: Cpu6502::IMP, cycles: 7 },
            Instruction { name: "???", operate: Cpu6502::NOP, addrmode: Cpu6502::IMP, cycles: 4 },
            Instruction { name: "CMP", operate: Cpu6502::CMP, addrmode: Cpu6502::ABX, cycles: 4 },
            Instruction { name: "DEC", operate: Cpu6502::DEC, addrmode: Cpu6502::ABX, cycles: 7 },
            Instruction { name: "???", operate: Cpu6502::XXX, addrmode: Cpu6502::IMP, cycles: 7 },
    
            // 0xE0
            Instruction { name: "CPX", operate: Cpu6502::CPX, addrmode: Cpu6502::IMM, cycles: 2 },
            Instruction { name: "SBC", operate: Cpu6502::SBC, addrmode: Cpu6502::IZX, cycles: 6 },
            Instruction { name: "???", operate: Cpu6502::NOP, addrmode: Cpu6502::IMP, cycles: 2 },
            Instruction { name: "???", operate: Cpu6502::XXX, addrmode: Cpu6502::IMP, cycles: 8 },
            Instruction { name: "CPX", operate: Cpu6502::CPX, addrmode: Cpu6502::ZP0, cycles: 3 },
            Instruction { name: "SBC", operate: Cpu6502::SBC, addrmode: Cpu6502::ZP0, cycles: 3 },
            Instruction { name: "INC", operate: Cpu6502::INC, addrmode: Cpu6502::ZP0, cycles: 5 },
            Instruction { name: "???", operate: Cpu6502::XXX, addrmode: Cpu6502::IMP, cycles: 5 },
            Instruction { name: "INX", operate: Cpu6502::INX, addrmode: Cpu6502::IMP, cycles: 2 },
            Instruction { name: "SBC", operate: Cpu6502::SBC, addrmode: Cpu6502::IMM, cycles: 2 },
            Instruction { name: "NOP", operate: Cpu6502::NOP, addrmode: Cpu6502::IMP, cycles: 2 },
            Instruction { name: "???", operate: Cpu6502::SBC, addrmode: Cpu6502::IMP, cycles: 2 },
            Instruction { name: "CPX", operate: Cpu6502::CPX, addrmode: Cpu6502::ABS, cycles: 4 },
            Instruction { name: "SBC", operate: Cpu6502::SBC, addrmode: Cpu6502::ABS, cycles: 4 },
            Instruction { name: "INC", operate: Cpu6502::INC, addrmode: Cpu6502::ABS, cycles: 6 },
            Instruction { name: "???", operate: Cpu6502::XXX, addrmode: Cpu6502::IMP, cycles: 6 },
    
            // 0xF0
            Instruction { name: "BEQ", operate: Cpu6502::BEQ, addrmode: Cpu6502::REL, cycles: 2 },
            Instruction { name: "SBC", operate: Cpu6502::SBC, addrmode: Cpu6502::IZY, cycles: 5 },
            Instruction { name: "???", operate: Cpu6502::XXX, addrmode: Cpu6502::IMP, cycles: 2 },
            Instruction { name: "???", operate: Cpu6502::XXX, addrmode: Cpu6502::IMP, cycles: 8 },
            Instruction { name: "???", operate: Cpu6502::NOP, addrmode: Cpu6502::IMP, cycles: 4 },
            Instruction { name: "SBC", operate: Cpu6502::SBC, addrmode: Cpu6502::ZPX, cycles: 4 },
            Instruction { name: "INC", operate: Cpu6502::INC, addrmode: Cpu6502::ZPX, cycles: 6 },
            Instruction { name: "???", operate: Cpu6502::XXX, addrmode: Cpu6502::IMP, cycles: 6 },
            Instruction { name: "SED", operate: Cpu6502::SED, addrmode: Cpu6502::IMP, cycles: 2 },
            Instruction { name: "SBC", operate: Cpu6502::SBC, addrmode: Cpu6502::ABY, cycles: 4 },
            Instruction { name: "NOP", operate: Cpu6502::NOP, addrmode: Cpu6502::IMP, cycles: 2 },
            Instruction { name: "???", operate: Cpu6502::XXX, addrmode: Cpu6502::IMP, cycles: 7 },
            Instruction { name: "???", operate: Cpu6502::NOP, addrmode: Cpu6502::IMP, cycles: 4 },
            Instruction { name: "SBC", operate: Cpu6502::SBC, addrmode: Cpu6502::ABX, cycles: 4 },
            Instruction { name: "INC", operate: Cpu6502::INC, addrmode: Cpu6502::ABX, cycles: 7 },
            Instruction { name: "???", operate: Cpu6502::XXX, addrmode: Cpu6502::IMP, cycles: 7 },
        ]
    }
}
