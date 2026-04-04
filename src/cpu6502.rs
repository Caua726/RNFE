
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
    pub a: u8,          // Acccumulator Register
    pub x: u8,          // X Register
    pub y: u8,          // Y Register
    pub stkp: u8,       // Stack Pointer (mostra os pontos do bus)
    pub pc: u16,        // Program Counter
    pub status: u8,     // Status Register
    fetched: u8,    // Nao sei, depois vejo
    temp: u16,      // Variavel temporaria
    addr_abs: u16,  // Endereco Absoluto
    addr_rel: u16,  // Endereço Relativo
    opcode: u8,     // Variavel Opcode
    cycles: u8,      // Ciclo de clock
    lookup: Vec<Instruction>,
}

pub struct Instruction {
    pub name: &'static str,                           // Nome da instrução
    pub operate: fn(&mut Cpu6502, &mut crate::bus::Bus) -> u8,
    pub addrmode: fn(&mut Cpu6502, &mut crate::bus::Bus) -> u8,
    pub cycles: u8,                                   // Ciclos necessários para a instrução
}
//Vector<Instruction> lookup

impl Cpu6502 {
    pub fn new() -> Self {
        Cpu6502 {
            a:0x00,                         // Accumulator
            x:0x00,                         // X Register
            y:0x00,                         // Y Register
            stkp:0x00,                      // Stack Pointer
            pc:0x0000,                      // Program Counter
            status:0x00,                    // Status Register
            fetched:0x00,                   // Nao sei depois eu vejo
            temp:0x0000,                    // Variavel temporaria
            addr_abs:0x0000,                // Todo o endereço de memoria acaba aqui
            addr_rel:0x0000,                // O endereço absoluto da atual instrução
            opcode:0x00,                    // Byte de instrução
            cycles:0,                       // Contagem do numero de ciclo de clocks
            lookup: Cpu6502::instrucoes(),  // Lookup table para uinstrucoes da cpu
        }
    }

    pub fn getFlag(&self, flag: FLAGS6502) -> u8 {
        if self.status & flag as u8 != 0 {
            1
        } else {
            0
        }
    }
    pub fn setFlag(&mut self, flag: FLAGS6502, value: bool) {
        if value {
            self.status |= flag as u8;
        } else {
            self.status &= !(flag as u8);
        }
    }
    pub fn read(&self, bus: &mut crate::bus::Bus, addr: u16) -> u8 {
        bus.cpu_read(addr, false)
    }

    pub fn write(&mut self, bus: &mut crate::bus::Bus, addr: u16, data: u8) {
        bus.cpu_write(addr, data);
    }
    //==========================
    //# Modos de enderecamento #
    //==========================

    // IMP: Implied
    // Nao tem muita coisa pra falar dele
    // Ele Faz uma coisa muito simples, como, mudar o estado
    // De Bits, de qualquer jeito, o objetivo dele é
    // O Acumulador, para instrucoes como PHA
    pub fn IMP(&mut self, _bus: &mut crate::bus::Bus) -> u8 {
        0
    }

    // IMM: Imediato
    // A instrução espera até o proximo bit para ser usado
    // Como valor, o endereco de leitura deve apontar ao
    // Proximo valor
    pub fn IMM(&mut self, bus: &mut crate::bus::Bus) -> u8 {
        self.addr_abs = self.pc;
        self.pc += 1;
        0
    }

    // ZP0: Zero Paging Adress / Modo de paginamento zero
    // Isso é usado para economizar recursos, permitindo
    // Que você salve o local nos primeiros 0xFF bytes
    // De um endereço de memoria, e por isso, isto só
    // Requer um byte de memoria ao invez de 2
    pub fn ZP0(&mut self, bus: &mut crate::bus::Bus) -> u8 {
        self.addr_abs = self.read(bus, self.pc) as u16;
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
    pub fn ZPX(&mut self, bus: &mut crate::bus::Bus) -> u8 {
        let base = self.read(bus, self.pc) as u8;
        self.pc += 1;
        self.addr_abs = base.wrapping_add(self.x) as u16;
        0
    }
    // ZPY: Zero Paging Y
    // O Mesmo do ZPX mas com Y
    pub fn ZPY(&mut self, bus: &mut crate::bus::Bus) -> u8 {
        let base = self.read(bus, self.pc) as u8;
        self.pc += 1;
        self.addr_abs = base.wrapping_add(self.y) as u16;
        0
    }   
    // ABS: Absolute
    // O endereço absoluto é formado
    // Por dois bytes, o primeiro
    // Serve para o banco de memoória
    // O segundo para o endereço na
    // Memória para formar um en1dreço de 16 bits
    pub fn ABS(&mut self, bus: &mut crate::bus::Bus) -> u8 {
        let lo: u16 = self.read(bus, self.pc) as u16;
        self.pc += 1;
        let hi: u16 = self.read(bus, self.pc) as u16;
        self.pc += 1;

        self.addr_abs = (hi << 8) | lo;

        0
    }   
    // ABX: Absolute X
    // Endereço absoluto com valor X
    // Isto calcula o mesmo que o
    // ABS, mas adiciona o endereço X
    pub fn ABX(&mut self, bus: &mut crate::bus::Bus) -> u8 {
        let lo: u16 = self.read(bus, self.pc) as u16;
        self.pc += 1;
        let hi: u16 = self.read(bus, self.pc) as u16;
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
    pub fn ABY(&mut self, bus: &mut crate::bus::Bus) -> u8 {
        let lo: u16 = self.read(bus, self.pc) as u16;
        self.pc += 1;
        let hi: u16 = self.read(bus, self.pc) as u16;
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
    pub fn IND(&mut self, bus: &mut crate::bus::Bus) -> u8 {
        let ptr_lo: u16 = self.read(bus, self.pc) as u16;
        self.pc += 1;
        let ptr_hi: u16 = self.read(bus, self.pc) as u16;
        self.pc += 1;

        let ptr: u16 = (ptr_hi << 8) | ptr_lo;

        if ptr_lo == 0x00FF {
            self.addr_abs = ((self.read(bus, ptr & 0xFF00) as u16) << 8) | self.read(bus, ptr) as u16;
        } else {
            self.addr_abs = ((self.read(bus, ptr + 1) as u16) << 8) | self.read(bus, ptr) as u16;
        }
        0
    }
    // IZX: Indirect Adressing Zero Page X
    // Ele referencia um endereço na memória pagina
    // Zero a partir do offset X para ler o endereço
    // De 16 bits que precisamos para a instrução
    pub fn IZX(&mut self, bus: &mut crate::bus::Bus) -> u8 {
        let t: u16 = self.read(bus, self.pc) as u16;
        self.pc += 1;
        let lo: u16 = self.read(bus, (t + (self.x  as u16)) & 0x00FF) as u16;
        let hi: u16 = self.read(bus, (t + (self.x as u16) + 1) & 0x00FF) as u16;

        self.addr_abs = (hi << 8 | lo);

        0
    } 
    // IZY: Indirect Adressing Zero Page Y
    // Diferentemente dos outros, onde X == Y, nesse
    // O X !== Y, neste caso ele lê um ponteiro de 16 bits
    // Da pagina zero e usa o offset Y para formar um
    // Endereço final.
    pub fn IZY(&mut self, bus: &mut crate::bus::Bus) -> u8 {
        let t: u16 = self.read(bus, self.pc) as u16;
        self.pc += 1;
        let lo: u16 = self.read(bus, t & 0x00FF) as u16;
        let hi: u16 = self.read(bus, (t + 1) & 0x00FF) as u16;

        let base = ((hi << 8) | lo) as u16;
        self.addr_abs = base.wrapping_add(self.y as u16);

        if (self.addr_abs & 0xFF00) != (base & 0xFF00) {
            1
        } else {
            0
        }
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
    pub fn REL(&mut self, bus: &mut crate::bus::Bus) -> u8 {
        self.addr_rel = self.read(bus, self.pc) as u16;
        self.pc += 1;
        if (self.addr_rel & 0x80) != 0 {
            self.addr_rel |= 0xFF00;
        }
        0
    }

    pub fn ACC(&mut self, _bus: &mut crate::bus::Bus) -> u8 {
        0
    }
    //==========================//
    //#      Instruções        #//
    //==========================//

    // Fetch
    pub fn fetch(&mut self, bus: &mut crate::bus::Bus) -> u8 {
        let idx = self.opcode as usize;
        let am = self.lookup[idx].addrmode as usize;
    
        if am == Cpu6502::ACC as usize {
            self.fetched = self.a;
        } else if am != Cpu6502::IMP as usize {
            self.fetched = self.read(bus, self.addr_abs);
        }
        self.fetched
    }
    

    // AND: And
    pub fn AND(&mut self, bus: &mut crate::bus::Bus) -> u8 {
        self.fetch(bus);
        self.a = self.a & self.fetched;
        self.setFlag(FLAGS6502::Z, self.a == 0x00);
        self.setFlag(FLAGS6502::N, (self.a & 0x80) != 0);
        
        return 1;
    }

    // BCS: Branch Carry Set
    pub fn BCS(&mut self, _bus: &mut crate::bus::Bus) -> u8 {
        if self.getFlag(FLAGS6502::C) == 1 {
            self.cycles += 1;
            self.addr_abs = self.pc.wrapping_add(self.addr_rel);

            if (self.addr_abs & 0xFF00) != (self.pc & 0xFF00) {
                self.cycles += 1;
            }
            self.pc = self.addr_abs;
        }
        return 0;
    }
    
    // BCC: Branch Carry Clear
    pub fn BCC(&mut self, _bus: &mut crate::bus::Bus) -> u8 {
        if self.getFlag(FLAGS6502::C) == 0 {
            self.cycles += 1;
            self.addr_abs = self.pc.wrapping_add(self.addr_rel);
            if (self.addr_abs & 0xFF00) != (self.pc & 0xFF00) {
                self.cycles += 1;
            }
            self.pc = self.addr_abs;
        }
        return 0;
    }

    // BEQ: Branch Equal
    pub fn BEQ(&mut self, _bus: &mut crate::bus::Bus) -> u8 {
        if self.getFlag(FLAGS6502::Z) == 1 {
            self.cycles += 1;
            self.addr_abs = self.pc.wrapping_add(self.addr_rel);
            if (self.addr_abs & 0xFF00) != (self.pc & 0xFF00) {
                self.cycles += 1;
            }
            self.pc = self.addr_abs;
        }
        return 0;
    }

    // BMI: Branch Minus
    pub fn BMI(&mut self, _bus: &mut crate::bus::Bus) -> u8 {
        if self.getFlag(FLAGS6502::N) == 1 {
            self.cycles += 1;
            self.addr_abs = self.pc.wrapping_add(self.addr_rel);
            if (self.addr_abs & 0xFF00) != (self.pc & 0xFF00) {
                self.cycles += 1;
            }
            self.pc = self.addr_abs;
        }
        return 0;
    }

    // BME: Branch Not Equal
    pub fn BNE(&mut self, _bus: &mut crate::bus::Bus) -> u8 {
        if self.getFlag(FLAGS6502::Z) == 0 {
            self.cycles += 1;
            self.addr_abs = self.pc.wrapping_add(self.addr_rel);
            if (self.addr_abs & 0xFF00) != (self.pc & 0xFF00) {
                self.cycles += 1;
            }
            self.pc = self.addr_abs;
        }
        return 0;
    }

    // BPL: Branch Plus
    pub fn BPL(&mut self, _bus: &mut crate::bus::Bus) -> u8 {
        if self.getFlag(FLAGS6502::N) == 0 {
            self.cycles += 1;
            self.addr_abs = self.pc.wrapping_add(self.addr_rel);
            if (self.addr_abs & 0xFF00) != (self.pc & 0xFF00) {
                self.cycles += 1;
            }
            self.pc = self.addr_abs;
        }
        return 0;
    }

    // BVC: Branch Overflow
    pub fn BVC(&mut self, _bus: &mut crate::bus::Bus) -> u8 {
        if self.getFlag(FLAGS6502::V) == 0 {
            self.cycles += 1;
            self.addr_abs = self.pc.wrapping_add(self.addr_rel);
            if (self.addr_abs & 0xFF00) != (self.pc & 0xFF00) {
                self.cycles += 1;
            }
            self.pc = self.addr_abs;
        }
        return 0;
    }

    // BVS: Branch Not Overflow
    pub fn BVS(&mut self, _bus: &mut crate::bus::Bus) -> u8 {
        if self.getFlag(FLAGS6502::V) == 1 {
            self.cycles += 1;
            self.addr_abs = self.pc.wrapping_add(self.addr_rel);
            if (self.addr_abs & 0xFF00) != (self.pc & 0xFF00) {
                self.cycles += 1;
            }
            self.pc = self.addr_abs;
        }
        return 0;
    }

    // CLC: Clear Carry Bit
    pub fn CLC(&mut self, _bus: &mut crate::bus::Bus) -> u8 {
        self.setFlag(FLAGS6502::C, false);
        return 0;
    }
    // CLD: Clear Decimal Mode
    pub fn CLD(&mut self, _bus: &mut crate::bus::Bus) -> u8 {
        self.setFlag(FLAGS6502::D, false);
        0
    }

    // CLI: Clear Interrupt Disable
    pub fn CLI(&mut self, _bus: &mut crate::bus::Bus) -> u8 {
        self.setFlag(FLAGS6502::I, false);
        0
    }

    // CLV: Clear Overflow Flag
    pub fn CLV(&mut self, _bus: &mut crate::bus::Bus) -> u8 {
        self.setFlag(FLAGS6502::V, false);
        0
    }

    // CMP: Compare Accumulator
    // Function: Compare A with fetched value
    // Flags Out: C, Z, N
    pub fn CMP(&mut self, bus: &mut crate::bus::Bus) -> u8 {
        self.fetch(bus);
        self.temp = (self.a as u16).wrapping_sub(self.fetched as u16);
        self.setFlag(FLAGS6502::C, self.a >= self.fetched);
        self.setFlag(FLAGS6502::Z, (self.temp & 0x00FF) == 0);
        self.setFlag(FLAGS6502::N, self.temp & 0x0080 != 0);
        1
    }

    // CPX: Compare X Register
    // Function: Compare X with fetched value
    // Flags Out: C, Z, N
    pub fn CPX(&mut self, bus: &mut crate::bus::Bus) -> u8 {
        self.fetch(bus);
        self.temp = (self.x as u16).wrapping_sub(self.fetched as u16);
        self.setFlag(FLAGS6502::C, self.x >= self.fetched);
        self.setFlag(FLAGS6502::Z, (self.temp & 0x00FF) == 0);
        self.setFlag(FLAGS6502::N, self.temp & 0x0080 != 0);
        0
    }

    // CPY: Compare Y Register
    // Function: Compare Y with fetched value
    // Flags Out: C, Z, N
    pub fn CPY(&mut self, bus: &mut crate::bus::Bus) -> u8 {
        self.fetch(bus);
        self.temp = (self.y as u16).wrapping_sub(self.fetched as u16);
        self.setFlag(FLAGS6502::C, self.y >= self.fetched);
        self.setFlag(FLAGS6502::Z, (self.temp & 0x00FF) == 0);
        self.setFlag(FLAGS6502::N, self.temp & 0x0080 != 0);
        0
    }

    // ADC: Add with Carry
    // Function: Add memory value to accumulator with carry
    // Flags Out: C, Z, V, N
    pub fn ADC(&mut self, bus: &mut crate::bus::Bus) -> u8 {
        self.fetch(bus);
        self.temp = (self.a as u16) + (self.fetched as u16) + (self.getFlag(FLAGS6502::C) as u16);
    
        self.setFlag(FLAGS6502::C, self.temp > 0x00FF);
        self.setFlag(FLAGS6502::Z, (self.temp & 0x00FF) == 0);
    
        let r = (self.temp & 0x00FF) as u8; // resultado de 8 bits
        self.setFlag(FLAGS6502::N, (r & 0x80) != 0);
    
        // V: (~(A^M) & (A^R) & 0x80) != 0
        self.setFlag(
            FLAGS6502::V,
            (((!(self.a ^ self.fetched)) & (self.a ^ r) & 0x80) != 0)
        );
    
        self.a = r;
        1
    }
    

    // SBC: Subtract with Carry
    // Subtrai um valor da memoria com o acumulador
    // Flags Out: C, Z, V, N
    pub fn SBC(&mut self, bus: &mut crate::bus::Bus) -> u8 {
        self.fetch(bus);
        let value = (self.fetched as u16) ^ 0x00FF;
        let temp = (self.a as u16) + value + (self.getFlag(FLAGS6502::C) as u16);
    
        let r = (temp & 0x00FF) as u8;
    
        self.setFlag(FLAGS6502::C, (temp & 0xFF00) != 0);
        self.setFlag(FLAGS6502::Z, r == 0);
        self.setFlag(FLAGS6502::N, (r & 0x80) != 0);
    
        // V (SBC): ((A^R) & (A^M) & 0x80) != 0
        self.setFlag(
            FLAGS6502::V,
            (((self.a ^ r) & (self.a ^ self.fetched) & 0x80) != 0)
        );
    
        self.a = r;
        1
    }
    
    // PHA: Push Accumulator
    // Funcão: Coloca o valor do acumulador no stack
    pub fn PHA(&mut self, bus: &mut crate::bus::Bus) -> u8 {
        self.write(bus, 0x100 + self.stkp as u16, self.a);
        self.stkp -= 1;
        0
    }

    // PLA: Pull Accumulator
    // Funcão: Pega o valor do stack e coloca no acumulador
    pub fn PLA(&mut self, bus: &mut crate::bus::Bus) -> u8 {
        self.stkp += 1;
        self.a = self.read(bus, 0x100 + self.stkp as u16);
        self.setFlag(FLAGS6502::Z, self.a == 0x00);
        self.setFlag(FLAGS6502::N, self.a & 0x80 != 0);
        0
    }
    
    // RTI: Return from Interrupt
    pub fn RTI(&mut self, bus: &mut crate::bus::Bus) -> u8 {
        self.stkp = self.stkp.wrapping_add(1);
        self.status = self.read(bus, 0x0100 + self.stkp as u16);
        self.status &= !(FLAGS6502::B as u8);
        self.status |=  (FLAGS6502::U as u8);
    
        self.stkp = self.stkp.wrapping_add(1);
        let lo = self.read(bus, 0x0100 + self.stkp as u16) as u16;
        self.stkp = self.stkp.wrapping_add(1);
        let hi = self.read(bus, 0x0100 + self.stkp as u16) as u16;
    
        self.pc = (hi << 8) | lo;
        0
    }
    
    //===========================================//
    //#         Opcodes Nao Implementados       #//
    //===========================================//
    // ASL: Arithmetic Shift Left
    pub fn ASL(&mut self, bus: &mut crate::bus::Bus) -> u8 {
        self.fetch(bus);
        let temp = (self.fetched as u16) << 1;
        self.setFlag(FLAGS6502::C, temp > 0x00FF);
        self.setFlag(FLAGS6502::Z, (temp & 0x00FF) == 0);
        self.setFlag(FLAGS6502::N, (temp & 0x0080) != 0);
        if self.lookup[self.opcode as usize].addrmode as usize == Cpu6502::ACC as usize {
            self.a = (temp & 0x00FF) as u8;
        } else {
            self.write(bus, self.addr_abs, (temp & 0x00FF) as u8);
        }
        0
    }

    // BIT: Bit Test
    pub fn BIT(&mut self, bus: &mut crate::bus::Bus) -> u8 {
        self.fetch(bus);
        let temp = self.a & self.fetched;
        self.setFlag(FLAGS6502::Z, temp == 0);
        self.setFlag(FLAGS6502::N, (self.fetched & 0x80) != 0);
        self.setFlag(FLAGS6502::V, (self.fetched & 0x40) != 0);
        0
    }

    // BRK: Force Interrupt
    pub fn BRK(&mut self, bus: &mut crate::bus::Bus) -> u8 {
        self.pc += 1;
        
        self.setFlag(FLAGS6502::I, true);
        self.write(bus, 0x0100 + self.stkp as u16, ((self.pc >> 8) & 0x00FF) as u8);
        self.stkp = self.stkp.wrapping_sub(1);
        self.write(bus, 0x0100 + self.stkp as u16, (self.pc & 0x00FF) as u8);
        self.stkp = self.stkp.wrapping_sub(1);
        
        self.setFlag(FLAGS6502::B, true);
        self.write(bus, 0x0100 + self.stkp as u16, self.status);
        self.stkp = self.stkp.wrapping_sub(1);
        
        let lo = self.read(bus, 0xFFFE) as u16;
        let hi = self.read(bus, 0xFFFF) as u16;
        self.pc = (hi << 8) | lo;
        0
    }

    // DEC: Decrement Memory
    pub fn DEC(&mut self, bus: &mut crate::bus::Bus) -> u8 {
        self.fetch(bus);
        let temp = self.fetched.wrapping_sub(1);
        self.write(bus, self.addr_abs, temp);
        self.setFlag(FLAGS6502::Z, temp == 0);
        self.setFlag(FLAGS6502::N, (temp & 0x80) != 0);
        0
    }

    // DEX: Decrement X Register
    pub fn DEX(&mut self, _bus: &mut crate::bus::Bus) -> u8 {
        self.x = self.x.wrapping_sub(1);
        self.setFlag(FLAGS6502::Z, self.x == 0);
        self.setFlag(FLAGS6502::N, (self.x & 0x80) != 0);
        0
    }

    // DEY: Decrement Y Register
    pub fn DEY(&mut self, _bus: &mut crate::bus::Bus) -> u8 {
        self.y = self.y.wrapping_sub(1);
        self.setFlag(FLAGS6502::Z, self.y == 0);
        self.setFlag(FLAGS6502::N, (self.y & 0x80) != 0);
        0
    }

    // EOR: Exclusive OR
    pub fn EOR(&mut self, bus: &mut crate::bus::Bus) -> u8 {
        self.fetch(bus);
        self.a = self.a ^ self.fetched;
        self.setFlag(FLAGS6502::Z, self.a == 0);
        self.setFlag(FLAGS6502::N, (self.a & 0x80) != 0);
        1
    }

    // INC: Increment Memory
    pub fn INC(&mut self, bus: &mut crate::bus::Bus) -> u8 {
        self.fetch(bus);
        let temp = self.fetched.wrapping_add(1);
        self.write(bus, self.addr_abs, temp);
        self.setFlag(FLAGS6502::Z, temp == 0);
        self.setFlag(FLAGS6502::N, (temp & 0x80) != 0);
        0
    }

    // INX: Increment X Register
    pub fn INX(&mut self, _bus: &mut crate::bus::Bus) -> u8 {
        self.x = self.x.wrapping_add(1);
        self.setFlag(FLAGS6502::Z, self.x == 0);
        self.setFlag(FLAGS6502::N, (self.x & 0x80) != 0);
        0
    }

    // INY: Increment Y Register
    pub fn INY(&mut self, _bus: &mut crate::bus::Bus) -> u8 {
        self.y = self.y.wrapping_add(1);
        self.setFlag(FLAGS6502::Z, self.y == 0);
        self.setFlag(FLAGS6502::N, (self.y & 0x80) != 0);
        0
    }

    // JMP: Jump
    pub fn JMP(&mut self, _bus: &mut crate::bus::Bus) -> u8 {
        self.pc = self.addr_abs;
        0
    }

    // JSR: Jump to Subroutine
    pub fn JSR(&mut self, bus: &mut crate::bus::Bus) -> u8 {
        self.pc = self.pc.wrapping_sub(1);
        
        self.write(bus, 0x0100 + self.stkp as u16, ((self.pc >> 8) & 0x00FF) as u8);
        self.stkp = self.stkp.wrapping_sub(1);
        self.write(bus, 0x0100 + self.stkp as u16, (self.pc & 0x00FF) as u8);
        self.stkp = self.stkp.wrapping_sub(1);
        
        self.pc = self.addr_abs;
        0
    }

    // LDA: Load Accumulator
    pub fn LDA(&mut self, bus: &mut crate::bus::Bus) -> u8 {
        self.fetch(bus);
        self.a = self.fetched;
        self.setFlag(FLAGS6502::Z, self.a == 0);
        self.setFlag(FLAGS6502::N, (self.a & 0x80) != 0);
        1
    }

    // LDX: Load X Register
    pub fn LDX(&mut self, bus: &mut crate::bus::Bus) -> u8 {
        self.fetch(bus);
        self.x = self.fetched;
        self.setFlag(FLAGS6502::Z, self.x == 0);
        self.setFlag(FLAGS6502::N, (self.x & 0x80) != 0);
        1
    }

    // LDY: Load Y Register
    pub fn LDY(&mut self, bus: &mut crate::bus::Bus) -> u8 {
        self.fetch(bus);
        self.y = self.fetched;
        self.setFlag(FLAGS6502::Z, self.y == 0);
        self.setFlag(FLAGS6502::N, (self.y & 0x80) != 0);
        1
    }

    // LSR: Logical Shift Right
    pub fn LSR(&mut self, bus: &mut crate::bus::Bus) -> u8 {
        self.fetch(bus);
        self.setFlag(FLAGS6502::C, (self.fetched & 0x0001) != 0);
        let temp = self.fetched >> 1;
        self.setFlag(FLAGS6502::Z, temp == 0);
        self.setFlag(FLAGS6502::N, false);
        if self.lookup[self.opcode as usize].addrmode as usize == Cpu6502::ACC as usize {
            self.a = temp;
        } else {
            self.write(bus, self.addr_abs, temp);
        }
        0
    }

    // NOP: No Operation
    pub fn NOP(&mut self, _bus: &mut crate::bus::Bus) -> u8 {
        match self.opcode {
            0x1C | 0x3C | 0x5C | 0x7C | 0xDC | 0xFC => 1,
            _ => 0
        }
    }

    // ORA: Logical Inclusive OR
    pub fn ORA(&mut self, bus: &mut crate::bus::Bus) -> u8 {
        self.fetch(bus);
        self.a = self.a | self.fetched;
        self.setFlag(FLAGS6502::Z, self.a == 0);
        self.setFlag(FLAGS6502::N, (self.a & 0x80) != 0);
        1
    }

    // PHP: Push Processor Status
    pub fn PHP(&mut self, bus: &mut crate::bus::Bus) -> u8 {
        self.write(bus, 0x0100 + self.stkp as u16, self.status | FLAGS6502::B as u8 | FLAGS6502::U as u8);
        self.stkp = self.stkp.wrapping_sub(1);
        0
    }

    // PLP: Pull Processor Status
    pub fn PLP(&mut self, bus: &mut crate::bus::Bus) -> u8 {
        self.stkp = self.stkp.wrapping_add(1);
        self.status = self.read(bus, 0x0100 + self.stkp as u16);
        self.setFlag(FLAGS6502::U, true);
        self.setFlag(FLAGS6502::B, false);
        0
    }

    // ROL: Rotate Left
    pub fn ROL(&mut self, bus: &mut crate::bus::Bus) -> u8 {
        self.fetch(bus);
        let temp = ((self.fetched as u16) << 1) | (self.getFlag(FLAGS6502::C) as u16);
        self.setFlag(FLAGS6502::C, temp > 0x00FF);
        self.setFlag(FLAGS6502::Z, (temp & 0x00FF) == 0);
        self.setFlag(FLAGS6502::N, (temp & 0x0080) != 0);
        if self.lookup[self.opcode as usize].addrmode as usize == Cpu6502::ACC as usize {
            self.a = (temp & 0x00FF) as u8;
        } else {
            self.write(bus, self.addr_abs, (temp & 0x00FF) as u8);
        }
        0
    }

    // ROR: Rotate Right
    pub fn ROR(&mut self, bus: &mut crate::bus::Bus) -> u8 {
        self.fetch(bus);
        let temp = ((self.getFlag(FLAGS6502::C) as u16) << 7) | (self.fetched as u16 >> 1);
        self.setFlag(FLAGS6502::C, (self.fetched & 0x01) != 0);
        self.setFlag(FLAGS6502::Z, (temp & 0x00FF) == 0);
        self.setFlag(FLAGS6502::N, (temp & 0x0080) != 0);
        if self.lookup[self.opcode as usize].addrmode as usize == Cpu6502::ACC as usize {
            self.a = (temp & 0x00FF) as u8;
        } else {
            self.write(bus, self.addr_abs, (temp & 0x00FF) as u8);
        }
        0
    }

    // RTS: Return from Subroutine
    pub fn RTS(&mut self, bus: &mut crate::bus::Bus) -> u8 {
        self.stkp = self.stkp.wrapping_add(1);
        let lo = self.read(bus, 0x0100 + self.stkp as u16) as u16;
        self.stkp = self.stkp.wrapping_add(1);
        let hi = self.read(bus, 0x0100 + self.stkp as u16) as u16;
        
        self.pc = (hi << 8) | lo;
        self.pc = self.pc.wrapping_add(1);
        0
    }

    // SEC: Set Carry Flag
    pub fn SEC(&mut self, _bus: &mut crate::bus::Bus) -> u8 {
        self.setFlag(FLAGS6502::C, true);
        0
    }

    // SED: Set Decimal Mode
    pub fn SED(&mut self, _bus: &mut crate::bus::Bus) -> u8 {
        self.setFlag(FLAGS6502::D, true);
        0
    }

    // SEI: Set Interrupt Disable
    pub fn SEI(&mut self, _bus: &mut crate::bus::Bus) -> u8 {
        self.setFlag(FLAGS6502::I, true);
        0
    }

    // STA: Store Accumulator
    pub fn STA(&mut self, bus: &mut crate::bus::Bus) -> u8 {
        self.write(bus, self.addr_abs, self.a);
        0
    }

    // STX: Store X Register
    pub fn STX(&mut self, bus: &mut crate::bus::Bus) -> u8 {
        self.write(bus, self.addr_abs, self.x);
        0
    }

    // STY: Store Y Register
    pub fn STY(&mut self, bus: &mut crate::bus::Bus) -> u8 {
        self.write(bus, self.addr_abs, self.y);
        0
    }

    // TAX: Transfer Accumulator to X
    pub fn TAX(&mut self, _bus: &mut crate::bus::Bus) -> u8 {
        self.x = self.a;
        self.setFlag(FLAGS6502::Z, self.x == 0);
        self.setFlag(FLAGS6502::N, (self.x & 0x80) != 0);
        0
    }

    // TAY: Transfer Accumulator to Y
    pub fn TAY(&mut self, _bus: &mut crate::bus::Bus) -> u8 {
        self.y = self.a;
        self.setFlag(FLAGS6502::Z, self.y == 0);
        self.setFlag(FLAGS6502::N, (self.y & 0x80) != 0);
        0
    }

    // TSX: Transfer Stack Pointer to X
    pub fn TSX(&mut self, _bus: &mut crate::bus::Bus) -> u8 {
        self.x = self.stkp;
        self.setFlag(FLAGS6502::Z, self.x == 0);
        self.setFlag(FLAGS6502::N, (self.x & 0x80) != 0);
        0
    }

    // TXA: Transfer X to Accumulator
    pub fn TXA(&mut self, _bus: &mut crate::bus::Bus) -> u8 {
        self.a = self.x;
        self.setFlag(FLAGS6502::Z, self.a == 0);
        self.setFlag(FLAGS6502::N, (self.a & 0x80) != 0);
        0
    }

    // TXS: Transfer X to Stack Pointer
    pub fn TXS(&mut self, _bus: &mut crate::bus::Bus) -> u8 {
        self.stkp = self.x;
        0
    }

    // TYA: Transfer Y to Accumulator
    pub fn TYA(&mut self, _bus: &mut crate::bus::Bus) -> u8 {
        self.a = self.y;
        self.setFlag(FLAGS6502::Z, self.a == 0);
        self.setFlag(FLAGS6502::N, (self.a & 0x80) != 0);
        0
    }

    // XXX: Illegal Opcode
    pub fn XXX(&mut self, _bus: &mut crate::bus::Bus) -> u8 {0}

    // Clock
    pub fn clock(&mut self, bus: &mut crate::bus::Bus) {
        if self.cycles == 0 {
            self.opcode = self.read(bus, self.pc);
            self.pc = self.pc.wrapping_add(1); // só +1
    
            let (addrmode, operate, base_cycles) = {
                let ins = &self.lookup[self.opcode as usize];
                (ins.addrmode, ins.operate, ins.cycles)
            };
    
            self.cycles = base_cycles;
            let cycle0 = addrmode(self, bus);
            let cycle1 = operate(self, bus);
            self.cycles = self.cycles.wrapping_add((cycle0 & cycle1) as u8);
        }
        self.cycles = self.cycles.wrapping_sub(1);
    }
    
    
    // Reset
    pub fn reset(&mut self, bus: &mut crate::bus::Bus) {
        self.a = 0;
        self.x = 0;
        self.y = 0;
        self.stkp = 0xFD;
        self.status = 0x00 | FLAGS6502::U as u8;
        
        self.addr_abs = 0xFFFC;
        let lo = self.read(bus, self.addr_abs + 0);
        let hi = self.read(bus, self.addr_abs + 1);

        self.pc = ((hi as u16) << 8) | (lo as u16);

        self.addr_rel = 0x0000;
        self.addr_abs = 0x0000;
        self.fetched = 0x00;

        self.cycles = 8;
    }
    // Interruptiuon Request
    pub fn irq(&mut self, bus: &mut crate::bus::Bus) {
        if self.getFlag(FLAGS6502::I) == 0 {
            // push PC
            self.write(bus, 0x0100 + self.stkp as u16, ((self.pc >> 8) & 0x00FF) as u8);
            self.stkp = self.stkp.wrapping_sub(1);
            self.write(bus, 0x0100 + self.stkp as u16, (self.pc & 0x00FF) as u8);
            self.stkp = self.stkp.wrapping_sub(1);
    
            // push status (B=0, U=1) e seta I
            self.setFlag(FLAGS6502::B, false);
            self.setFlag(FLAGS6502::U, true);
            self.setFlag(FLAGS6502::I, true);
            self.write(bus, 0x0100 + self.stkp as u16, self.status);
            self.stkp = self.stkp.wrapping_sub(1);
    
            // vetor IRQ
            let lo = self.read(bus, 0xFFFE) as u16;
            let hi = self.read(bus, 0xFFFF) as u16;
            self.pc = (hi << 8) | lo;
    
            self.cycles = 7;
        }
    }
    
    pub fn nmi(&mut self, bus: &mut crate::bus::Bus) {
        self.write(bus, 0x0100 + self.stkp as u16, ((self.pc >> 8) & 0x00FF) as u8);
        self.stkp = self.stkp.wrapping_sub(1);
        self.write(bus, 0x0100 + self.stkp as u16, (self.pc & 0x00FF) as u8);
        self.stkp = self.stkp.wrapping_sub(1);
    
        self.setFlag(FLAGS6502::B, false);
        self.setFlag(FLAGS6502::U, true);
        self.setFlag(FLAGS6502::I, true);
        self.write(bus, 0x0100 + self.stkp as u16, self.status);
        self.stkp = self.stkp.wrapping_sub(1);
    
        let lo = self.read(bus, 0xFFFA) as u16;
        let hi = self.read(bus, 0xFFFB) as u16;
        self.pc = (hi << 8) | lo;
    
        self.cycles = 8;
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
            Instruction { name: "ASL", operate: Cpu6502::ASL, addrmode: Cpu6502::ACC, cycles: 2 },
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
            Instruction { name: "ROL", operate: Cpu6502::ROL, addrmode: Cpu6502::ACC, cycles: 2 },
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
            Instruction { name: "LSR", operate: Cpu6502::LSR, addrmode: Cpu6502::ACC, cycles: 2 },
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
            Instruction { name: "ROR", operate: Cpu6502::ROR, addrmode: Cpu6502::ACC, cycles: 2 },
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
