//! Desmontador mínimo: `cargo run -q -p rnfe-core --example disasm -- rom.nes C000 40`
//! (endereço e quantidade em hex; PRG mapeado como NROM em $8000-$FFFF).

use rnfe_core::cpu6502::{LOOKUP, Mode};

fn main() {
    let a: Vec<String> = std::env::args().skip(1).collect();
    let rom = std::fs::read(&a[0]).expect("rom");
    let start = u16::from_str_radix(&a[1], 16).unwrap();
    let count = usize::from_str_radix(a.get(2).map(String::as_str).unwrap_or("20"), 16).unwrap();
    let prg_banks = rom[4] as usize;
    let trainer = if rom[6] & 4 != 0 { 512 } else { 0 };
    let prg = &rom[16 + trainer..16 + trainer + prg_banks * 16384];
    let byte = |addr: u16| prg[(addr as usize - 0x8000) % prg.len()];
    let mut pc = start;
    for _ in 0..count {
        let op = byte(pc);
        let (name, mode) = LOOKUP[op as usize];
        let len = match mode {
            Mode::Imp | Mode::Acc => 1,
            Mode::Abs | Mode::AbsX | Mode::AbsXW | Mode::AbsY | Mode::AbsYW | Mode::Ind => 3,
            _ => 2,
        };
        let b1 = byte(pc.wrapping_add(1));
        let b2 = byte(pc.wrapping_add(2));
        let w = (b2 as u16) << 8 | b1 as u16;
        let operand = match mode {
            Mode::Imp => String::new(),
            Mode::Acc => "A".into(),
            Mode::Imm => format!("#${b1:02X}"),
            Mode::Zp => format!("${b1:02X}"),
            Mode::ZpX => format!("${b1:02X},X"),
            Mode::ZpY => format!("${b1:02X},Y"),
            Mode::Abs => format!("${w:04X}"),
            Mode::AbsX | Mode::AbsXW => format!("${w:04X},X"),
            Mode::AbsY | Mode::AbsYW => format!("${w:04X},Y"),
            Mode::Ind => format!("(${w:04X})"),
            Mode::IndX => format!("(${b1:02X},X)"),
            Mode::IndY | Mode::IndYW => format!("(${b1:02X}),Y"),
            Mode::Rel => format!("${:04X}", pc.wrapping_add(2).wrapping_add(b1 as i8 as i16 as u16)),
        };
        let bytes: Vec<String> = (0..len).map(|i| format!("{:02X}", byte(pc.wrapping_add(i)))).collect();
        println!("{pc:04X}  {:<9} {} {operand}", bytes.join(" "), name.name());
        pc = pc.wrapping_add(len);
    }
}
