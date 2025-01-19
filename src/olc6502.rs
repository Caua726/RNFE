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
    IMP: u8,    IMM: u8,
    ZP0: u8,    ZPX: u8,
    ZPY: u8,    REL: u8,
    ABS: u16,    ABX: u16,
    ABY: u16,    IND: u16,
    IZX: u16,    IZY: u16
}


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
}
