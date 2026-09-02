// Diagnóstico do emulador - analisa o estado e identifica problemas
use crate::bus::Bus;
use crate::cpu6502::Cpu6502;

pub fn run_diagnostic(cpu: &Cpu6502, bus: &Bus) {
    println!("\n========== DIAGNOSTICO DO EMULADOR ==========\n");

    // 1. Estado da CPU
    println!("[CPU]");
    println!("  PC: ${:04X}  A: ${:02X}  X: ${:02X}  Y: ${:02X}  SP: ${:02X}  P: ${:02X}",
        cpu.pc, cpu.a, cpu.x, cpu.y, cpu.stkp, cpu.status);
    let flags = format!("{}{}{}{}{}{}{}{}",
        if cpu.status & 0x80 != 0 { "N" } else { "." },
        if cpu.status & 0x40 != 0 { "V" } else { "." },
        if cpu.status & 0x20 != 0 { "U" } else { "." },
        if cpu.status & 0x10 != 0 { "B" } else { "." },
        if cpu.status & 0x08 != 0 { "D" } else { "." },
        if cpu.status & 0x04 != 0 { "I" } else { "." },
        if cpu.status & 0x02 != 0 { "Z" } else { "." },
        if cpu.status & 0x01 != 0 { "C" } else { "." },
    );
    println!("  Flags: {}", flags);

    // Detectar stuck
    let op = bus.cpu_read_debug(cpu.pc);
    let op1 = bus.cpu_read_debug(cpu.pc.wrapping_add(1));
    let op2 = bus.cpu_read_debug(cpu.pc.wrapping_add(2));
    println!("  Next: {:02X} {:02X} {:02X}", op, op1, op2);

    // 2. PPU State
    println!("\n[PPU]");
    println!("  CTRL: ${:02X}  MASK: ${:02X}  STATUS: ${:02X}",
        bus.ppu.control, bus.ppu.mask, bus.ppu.status);
    println!("  Scanline: {}  Cycle: {}", bus.ppu.scanline, bus.ppu.cycle);
    println!("  VRAM: ${:04X}  TRAM: ${:04X}", bus.ppu.vram_addr, bus.ppu.tram_addr);

    let nmi_enabled = bus.ppu.control & 0x80 != 0;
    let bg_enabled = bus.ppu.mask & 0x08 != 0;
    let spr_enabled = bus.ppu.mask & 0x10 != 0;
    let bg_table = if bus.ppu.control & 0x10 != 0 { "$1000" } else { "$0000" };
    let spr_table = if bus.ppu.control & 0x08 != 0 { "$1000" } else { "$0000" };
    println!("  NMI: {}  BG: {}  Sprites: {}  BG table: {}  Sprite table: {}",
        nmi_enabled, bg_enabled, spr_enabled, bg_table, spr_table);

    // 3. Nametable content
    println!("\n[NAMETABLE]");
    let mut nt0_nonzero = 0u32;
    let mut nt1_nonzero = 0u32;
    for i in 0..960 { if bus.ppu.nametable[0][i] != 0 { nt0_nonzero += 1; } }
    for i in 0..960 { if bus.ppu.nametable[1][i] != 0 { nt1_nonzero += 1; } }
    let mut attr0_nonzero = 0u32;
    let mut attr1_nonzero = 0u32;
    for i in 960..1024 { if bus.ppu.nametable[0][i] != 0 { attr0_nonzero += 1; } }
    for i in 960..1024 { if bus.ppu.nametable[1][i] != 0 { attr1_nonzero += 1; } }
    println!("  NT0: {}/960 tiles nonzero, {}/64 attrs nonzero", nt0_nonzero, attr0_nonzero);
    println!("  NT1: {}/960 tiles nonzero, {}/64 attrs nonzero", nt1_nonzero, attr1_nonzero);

    // Tile ID distribution
    let mut tile_counts = [0u32; 256];
    for i in 0..960 { tile_counts[bus.ppu.nametable[0][i] as usize] += 1; }
    let most_common = tile_counts.iter().enumerate().max_by_key(|(_, c)| **c).unwrap();
    println!("  Most common tile in NT0: ${:02X} ({}x)", most_common.0, most_common.1);

    // 4. Pattern table content
    println!("\n[PATTERN TABLE]");
    let mut pt0_nonzero = 0u32;
    let mut pt1_nonzero = 0u32;
    for b in &bus.ppu.pattern_table[0] { if *b != 0 { pt0_nonzero += 1; } }
    for b in &bus.ppu.pattern_table[1] { if *b != 0 { pt1_nonzero += 1; } }
    println!("  PT0: {}/4096 bytes nonzero", pt0_nonzero);
    println!("  PT1: {}/4096 bytes nonzero", pt1_nonzero);

    // Verificar se CHR é lido do cartridge
    if let Some(ref cart) = bus.cartridge {
        // Tentar ler via cartridge
        let mut cart_nonzero = 0u32;
        for addr in 0..0x2000u16 {
            if let Some(b) = cart.cpu_read_chr_debug(addr) {
                if b != 0 { cart_nonzero += 1; }
            }
        }
        println!("  Cartridge CHR (via mapper): {}/8192 bytes nonzero", cart_nonzero);
    }

    // 5. Palette
    println!("\n[PALETTE]");
    print!("  BG: ");
    for i in 0..16 {
        print!("{:02X} ", bus.ppu.palette_table[i]);
        if i % 4 == 3 { print!("| "); }
    }
    println!();
    print!("  SP: ");
    for i in 16..32 {
        print!("{:02X} ", bus.ppu.palette_table[i]);
        if i % 4 == 3 { print!("| "); }
    }
    println!();

    let pal_nonzero = bus.ppu.palette_table.iter().filter(|b| **b != 0).count();
    if pal_nonzero == 0 {
        println!("  WARNING: Palette is completely empty!");
    }

    // 6. OAM / Sprites
    println!("\n[OAM]");
    let mut visible_sprites = 0;
    for i in 0..64 {
        let y = bus.ppu.oam[i * 4];
        if y < 240 { visible_sprites += 1; }
    }
    println!("  Visible sprites (Y < 240): {}/64", visible_sprites);
    if visible_sprites > 0 {
        println!("  First 4 sprites:");
        for i in 0..4.min(64) {
            let y = bus.ppu.oam[i * 4];
            let tile = bus.ppu.oam[i * 4 + 1];
            let attr = bus.ppu.oam[i * 4 + 2];
            let x = bus.ppu.oam[i * 4 + 3];
            if y < 240 {
                println!("    Sprite {}: X={} Y={} Tile=${:02X} Attr=${:02X}", i, x, y, tile, attr);
            }
        }
    }

    // 7. Sprite 0 Hit analysis
    println!("\n[SPRITE 0 HIT]");
    let spr0_y = bus.ppu.oam[0];
    let spr0_tile = bus.ppu.oam[1];
    let spr0_x = bus.ppu.oam[3];
    println!("  Sprite 0: X={} Y={} Tile=${:02X}", spr0_x, spr0_y, spr0_tile);
    let s0hit = bus.ppu.status & 0x40 != 0;
    println!("  Sprite 0 hit flag: {}", s0hit);
    if spr0_y >= 240 {
        println!("  WARNING: Sprite 0 is off-screen (Y >= 240), hit will never trigger!");
    }
    if !bg_enabled || !spr_enabled {
        println!("  WARNING: BG or sprites disabled, sprite 0 hit cannot trigger!");
    }

    // 8. Mapper info
    println!("\n[MAPPER]");
    if let Some(ref cart) = bus.cartridge {
        println!("  Mirror: {:?}", cart.get_mirror());
        cart.print_mapper_state();
    }

    // 9. Screen content analysis
    println!("\n[SCREEN]");
    let mut color_counts = std::collections::HashMap::new();
    for pixel in bus.ppu.screen.iter() {
        *color_counts.entry(*pixel).or_insert(0u32) += 1;
    }
    let total = (256 * 240) as u32;
    let mut sorted: Vec<_> = color_counts.iter().collect();
    sorted.sort_by(|a, b| b.1.cmp(a.1));
    println!("  Unique colors: {}", sorted.len());
    for (i, (color, count)) in sorted.iter().take(5).enumerate() {
        let pct = **count as f32 / total as f32 * 100.0;
        println!("  {}. RGB({},{},{}) = {} pixels ({:.1}%)", i+1, color[0], color[1], color[2], count, pct);
    }
    if sorted.len() <= 2 {
        println!("  WARNING: Screen has very few colors, rendering may be broken!");
    }

    println!("\n========== FIM DIAGNOSTICO ==========\n");
}
