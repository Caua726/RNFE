use crate::olc6502::Olc6502;
use std::cell::RefCell;
use std::rc::Rc;

pub struct Bus {
    // cpu: Olc6502, // Talvez seria melhor sem, ou colocar o refcell
    ram: [u8; 64 * 1024], // FAKE RAM Temporally `_1`
}

impl Bus {
    pub fn new() -> Bus {
        Bus {
            // cpu: Olc6502::new(), // olhar linha 5
            ram: [0; 64 * 1024]
        }
    }

    // Write
    pub fn write(&mut self, addr: u16, data: u8) -> () {
        if (0x0000..=0xFFFF).contains(&addr) {
            self.ram[addr as usize] = data; // Write
        } else {
            println!("Address out of range: {:04X}", addr);
        }
    }

    // Read
    pub fn read(&self, addr: u16, read_only: bool) -> u8 {
        if (0x0000..=0xFFFF).contains(&addr) {
            self.ram[addr as usize] // Read
        } else {
            return 0x00;
        }
    }
}

fn main() {
    let mut bus = Bus::new();

    // Limpa a RAM
    for byte in bus.ram.iter_mut() {
        *byte = 0x00;
    }

    // Conecta a CPU ao barramento
    let mut cpu = Olc6502::new();
    cpu.connect_bus(Rc::new(RefCell::new(bus)));
}
