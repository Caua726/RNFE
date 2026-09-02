//! Regressão visual: hash FNV-1a do framebuffer após N frames de ROMs livres.
//!
//! Não prova correção — prova que nada mudou sem querer. Uma correção de PPU que muda o
//! render regrava o `.hash` no mesmo commit: `RNFE_UPDATE_SNAPSHOTS=1 cargo test --test snapshots`.
//! Numa divergência, o frame é gravado em `target/snapshots/<nome>.ppm` para inspeção.

use rnfe_core::testing::{fnv1a64, load, write_ppm};
use rnfe_core::Buttons;
use std::path::Path;

/// (nome, ROM relativa a test-roms/, frames, [(frame, botões)])
const SNAPSHOTS: &[(&str, &str, u32, &[(u32, u8)])] = &[
    ("nestest_menu", "other/nestest.nes", 60, &[]),
    ("nestest_run", "other/nestest.nes", 240, &[(30, 0x10), (32, 0x00)]), // Start: roda os testes
    ("full_palette", "full_palette/full_palette.nes", 120, &[]),
    ("full_palette_smooth", "full_palette/full_palette_smooth.nes", 120, &[]),
    ("nmi_sync", "nmi_sync/demo_ntsc.nes", 120, &[]),
    ("scanline", "scanline/scanline.nes", 120, &[]),
    ("sprite_hit_basics", "sprite_hit_tests_2005.10.05/01.basics.nes", 120, &[]),
    ("blade_buster", "other/BladeBuster.nes", 400, &[(120, 0x10), (124, 0x00)]),
    ("raster_demo", "other/RasterDemo.NES", 120, &[]),
    ("blocks", "other/BLOCKS.NES", 120, &[]),
];

#[test]
fn framebuffer_snapshots() {
    let update = std::env::var_os("RNFE_UPDATE_SNAPSHOTS").is_some();
    let dir = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/snapshots");
    let dump_dir = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../target/snapshots");
    let mut errors = Vec::new();
    for (name, rom, frames, inputs) in SNAPSHOTS {
        let mut nes = match load(rom) {
            Ok(n) => n,
            Err(e) => {
                eprintln!("SKIP {name}: {e}");
                continue;
            }
        };
        for f in 0..*frames {
            if let Some((_, b)) = inputs.iter().find(|(at, _)| *at == f) {
                nes.set_controller(0, Buttons(*b));
            }
            nes.run_frame();
        }
        let hash = format!("{:016x}", fnv1a64(nes.framebuffer()));
        let file = dir.join(format!("{name}.hash"));
        if update {
            std::fs::write(&file, format!("{hash}\n")).unwrap();
            eprintln!("gravado {name} = {hash}");
            continue;
        }
        match std::fs::read_to_string(&file) {
            Err(_) => eprintln!("SKIP {name}: sem {} (RNFE_UPDATE_SNAPSHOTS=1 para criar)", file.display()),
            Ok(expected) if expected.trim() == hash => eprintln!("ok   {name} {hash}"),
            Ok(expected) => {
                let _ = std::fs::create_dir_all(&dump_dir);
                let ppm = dump_dir.join(format!("{name}.ppm"));
                let _ = write_ppm(&ppm, nes.framebuffer());
                errors.push(format!(
                    "{name}: esperado {} obtido {hash} (frame em {})",
                    expected.trim(),
                    ppm.display()
                ));
            }
        }
    }
    assert!(errors.is_empty(), "{} snapshot(s) divergente(s):\n  {}", errors.len(), errors.join("\n  "));
}
