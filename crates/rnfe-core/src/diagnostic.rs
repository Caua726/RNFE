// Diagnóstico do emulador - analisa o estado e identifica problemas
use crate::bus::Bus;
use crate::cpu6502::Cpu6502;
use std::fmt::Write;

/// Relatório completo do estado do console (CPU, PPU, nametables, paleta, OAM, mapper, tela).
pub fn diagnostic_report(cpu: &Cpu6502, bus: &Bus) -> String {
    let mut out = String::with_capacity(2048);
    let o = &mut out;

    let _ = writeln!(o, "\n========== DIAGNOSTICO DO EMULADOR ==========\n");

    // 1. Estado da CPU
    let _ = writeln!(o, "[CPU]");
    let _ = writeln!(
        o,
        "  PC: ${:04X}  A: ${:02X}  X: ${:02X}  Y: ${:02X}  SP: ${:02X}  P: ${:02X}",
        cpu.pc, cpu.a, cpu.x, cpu.y, cpu.stkp, cpu.status
    );
    let flags = format!(
        "{}{}{}{}{}{}{}{}",
        if cpu.status & 0x80 != 0 { "N" } else { "." },
        if cpu.status & 0x40 != 0 { "V" } else { "." },
        if cpu.status & 0x20 != 0 { "U" } else { "." },
        if cpu.status & 0x10 != 0 { "B" } else { "." },
        if cpu.status & 0x08 != 0 { "D" } else { "." },
        if cpu.status & 0x04 != 0 { "I" } else { "." },
        if cpu.status & 0x02 != 0 { "Z" } else { "." },
        if cpu.status & 0x01 != 0 { "C" } else { "." },
    );
    let _ = writeln!(o, "  Flags: {}", flags);

    let op = bus.cpu_read_debug(cpu.pc);
    let op1 = bus.cpu_read_debug(cpu.pc.wrapping_add(1));
    let op2 = bus.cpu_read_debug(cpu.pc.wrapping_add(2));
    let _ = writeln!(o, "  Next: {:02X} {:02X} {:02X}", op, op1, op2);

    // 2. PPU
    let _ = writeln!(o, "\n[PPU]");
    let _ = writeln!(
        o,
        "  CTRL: ${:02X}  MASK: ${:02X}  STATUS: ${:02X}",
        bus.ppu.control, bus.ppu.mask, bus.ppu.status
    );
    let _ = writeln!(o, "  Scanline: {}  Cycle: {}", bus.ppu.scanline, bus.ppu.cycle);
    let _ = writeln!(o, "  VRAM: ${:04X}  TRAM: ${:04X}", bus.ppu.vram_addr, bus.ppu.tram_addr);

    let nmi_enabled = bus.ppu.control & 0x80 != 0;
    let bg_enabled = bus.ppu.mask & 0x08 != 0;
    let spr_enabled = bus.ppu.mask & 0x10 != 0;
    let bg_table = if bus.ppu.control & 0x10 != 0 { "$1000" } else { "$0000" };
    let spr_table = if bus.ppu.control & 0x08 != 0 { "$1000" } else { "$0000" };
    let _ = writeln!(
        o,
        "  NMI: {}  BG: {}  Sprites: {}  BG table: {}  Sprite table: {}",
        nmi_enabled, bg_enabled, spr_enabled, bg_table, spr_table
    );

    // 3. Nametables
    let _ = writeln!(o, "\n[NAMETABLE]");
    let nz = |t: &[u8], r: std::ops::Range<usize>| t[r].iter().filter(|b| **b != 0).count();
    let _ = writeln!(
        o,
        "  NT0: {}/960 tiles nonzero, {}/64 attrs nonzero",
        nz(&bus.ppu.nametable[0], 0..960),
        nz(&bus.ppu.nametable[0], 960..1024)
    );
    let _ = writeln!(
        o,
        "  NT1: {}/960 tiles nonzero, {}/64 attrs nonzero",
        nz(&bus.ppu.nametable[1], 0..960),
        nz(&bus.ppu.nametable[1], 960..1024)
    );

    let mut tile_counts = [0u32; 256];
    for &t in &bus.ppu.nametable[0][..960] {
        tile_counts[t as usize] += 1;
    }
    let most_common = tile_counts.iter().enumerate().max_by_key(|(_, c)| **c).unwrap();
    let _ = writeln!(o, "  Most common tile in NT0: ${:02X} ({}x)", most_common.0, most_common.1);

    // 4. Pattern tables
    let _ = writeln!(o, "\n[PATTERN TABLE]");
    let cart_nonzero =
        (0..0x2000u16).filter_map(|a| bus.cartridge.cpu_read_chr_debug(a)).filter(|b| *b != 0).count();
    let _ = writeln!(o, "  Cartridge CHR (via mapper): {}/8192 bytes nonzero", cart_nonzero);

    // 5. Paleta
    let _ = writeln!(o, "\n[PALETTE]");
    let _ = write!(o, "  BG: ");
    for i in 0..16 {
        let _ = write!(o, "{:02X} ", bus.ppu.palette_table[i]);
        if i % 4 == 3 {
            let _ = write!(o, "| ");
        }
    }
    let _ = writeln!(o);
    let _ = write!(o, "  SP: ");
    for i in 16..32 {
        let _ = write!(o, "{:02X} ", bus.ppu.palette_table[i]);
        if i % 4 == 3 {
            let _ = write!(o, "| ");
        }
    }
    let _ = writeln!(o);
    if bus.ppu.palette_table.iter().all(|b| *b == 0) {
        let _ = writeln!(o, "  WARNING: Palette is completely empty!");
    }

    // 6. OAM
    let _ = writeln!(o, "\n[OAM]");
    let visible_sprites = (0..64).filter(|i| bus.ppu.oam[i * 4] < 240).count();
    let _ = writeln!(o, "  Visible sprites (Y < 240): {}/64", visible_sprites);
    if visible_sprites > 0 {
        let _ = writeln!(o, "  First 4 sprites:");
        for i in 0..4 {
            let y = bus.ppu.oam[i * 4];
            let tile = bus.ppu.oam[i * 4 + 1];
            let attr = bus.ppu.oam[i * 4 + 2];
            let x = bus.ppu.oam[i * 4 + 3];
            if y < 240 {
                let _ =
                    writeln!(o, "    Sprite {}: X={} Y={} Tile=${:02X} Attr=${:02X}", i, x, y, tile, attr);
            }
        }
    }

    // 7. Sprite 0 hit
    let _ = writeln!(o, "\n[SPRITE 0 HIT]");
    let spr0_y = bus.ppu.oam[0];
    let spr0_tile = bus.ppu.oam[1];
    let spr0_x = bus.ppu.oam[3];
    let _ = writeln!(o, "  Sprite 0: X={} Y={} Tile=${:02X}", spr0_x, spr0_y, spr0_tile);
    let _ = writeln!(o, "  Sprite 0 hit flag: {}", bus.ppu.status & 0x40 != 0);
    if spr0_y >= 240 {
        let _ = writeln!(o, "  WARNING: Sprite 0 is off-screen (Y >= 240), hit will never trigger!");
    }
    if !bg_enabled || !spr_enabled {
        let _ = writeln!(o, "  WARNING: BG or sprites disabled, sprite 0 hit cannot trigger!");
    }

    // 8. Mapper
    let _ = writeln!(o, "\n[MAPPER]");
    let _ = writeln!(o, "  Mirror: {:?}", bus.cartridge.get_mirror());
    let _ = write!(o, "{}", bus.cartridge.mapper_state());

    // 9. Tela
    let _ = writeln!(o, "\n[SCREEN]");
    let mut color_counts = std::collections::HashMap::new();
    for pixel in bus.ppu.screen.iter() {
        *color_counts.entry(*pixel).or_insert(0u32) += 1;
    }
    let total = (256 * 240) as u32;
    let mut sorted: Vec<_> = color_counts.iter().collect();
    sorted.sort_by(|a, b| b.1.cmp(a.1));
    let _ = writeln!(o, "  Unique colors: {}", sorted.len());
    for (i, (color, count)) in sorted.iter().take(5).enumerate() {
        let pct = **count as f32 / total as f32 * 100.0;
        let _ = writeln!(
            o,
            "  {}. RGB({},{},{}) = {} pixels ({:.1}%)",
            i + 1,
            color[0],
            color[1],
            color[2],
            count,
            pct
        );
    }
    if sorted.len() <= 2 {
        let _ = writeln!(o, "  WARNING: Screen has very few colors, rendering may be broken!");
    }

    let _ = writeln!(o, "\n========== FIM DIAGNOSTICO ==========\n");
    out
}
