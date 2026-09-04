//! Mede a velocidade do núcleo: `bench --rom x.nes [--frames N] [--profile]`.
//!
//! Imprime fps, ns/frame e o pico de RSS do processo (VmHWM, Linux/Android).
//! `--profile` roda também a PPU sozinha (mesmo número de dots) e a APU sozinha (mesmos
//! ciclos) para estimar quanto do frame é de cada subsistema — o resto é CPU + bus.

use std::time::Instant;

fn proc_status(key: &str) -> Option<u64> {
    let s = std::fs::read_to_string("/proc/self/status").ok()?;
    s.lines()
        .find(|l| l.starts_with(key))
        .and_then(|l| l.split_whitespace().nth(1))
        .and_then(|v| v.parse().ok())
}

fn main() {
    let args: Vec<String> = std::env::args().collect();
    let mut rom = None;
    let mut frames: u32 = 3000;
    let mut profile = false;
    let mut i = 1;
    while i < args.len() {
        match args[i].as_str() {
            "--rom" => {
                rom = args.get(i + 1).cloned();
                i += 1;
            }
            "--frames" => {
                frames = args.get(i + 1).and_then(|s| s.parse().ok()).unwrap_or(frames);
                i += 1;
            }
            "--profile" => profile = true,
            other => {
                eprintln!("argumento desconhecido: {other}\nuso: bench --rom <rom.nes> [--frames N]");
                std::process::exit(2);
            }
        }
        i += 1;
    }
    let Some(rom) = rom else {
        eprintln!("uso: bench --rom <rom.nes> [--frames N]");
        std::process::exit(2);
    };
    let bytes = std::fs::read(&rom).expect("ler ROM");
    let cart = rnfe_core::Cartridge::from_bytes(&bytes).expect("ROM inválida");
    let mut nes = rnfe_core::Nes::new(cart);
    let mut audio = Vec::with_capacity(4096);

    // aquecimento: tira o custo de página/alloc da medição
    for _ in 0..30 {
        nes.run_frame();
        audio.clear();
        nes.drain_audio(&mut audio);
    }
    let rss_start = proc_status("VmRSS:");

    let t = Instant::now();
    for _ in 0..frames {
        nes.run_frame();
        audio.clear();
        nes.drain_audio(&mut audio);
    }
    let wall = t.elapsed();

    let fps = frames as f64 / wall.as_secs_f64();
    let ns_frame = wall.as_nanos() as f64 / frames as f64;
    let name = std::path::Path::new(&rom).file_name().map(|s| s.to_string_lossy().to_string()).unwrap_or(rom);
    println!(
        "rom={name} frames={frames} wall={:.2}s fps={fps:.1} ns_frame={ns_frame:.0} ms_frame={:.3} realtime={:.1}x vmhwm_kb={} vmrss_kb={}→{} profile={}",
        wall.as_secs_f64(),
        ns_frame / 1e6,
        fps / rnfe_core::NTSC_FPS,
        proc_status("VmHWM:").map_or("n/a".into(), |v| v.to_string()),
        rss_start.map_or("n/a".into(), |v| v.to_string()),
        proc_status("VmRSS:").map_or("n/a".into(), |v| v.to_string()),
        if cfg!(debug_assertions) { "debug" } else { "release" },
    );

    if profile {
        // PPU sozinha: os mesmos 89 342 dots por frame, sem CPU
        let dots = frames as u64 * 89_342;
        let t = Instant::now();
        for _ in 0..dots {
            nes.bus.ppu.step(&mut nes.bus.cartridge);
        }
        let ppu = t.elapsed();
        // APU sozinha: os mesmos ~29 781 ciclos por frame
        let cycles = frames as u64 * 29_781;
        let t = Instant::now();
        for _ in 0..cycles {
            nes.bus.apu.clock(|| 0.0);
            if nes.bus.apu.sample_buffer.len() > 4096 {
                nes.bus.apu.sample_buffer.clear();
            }
        }
        let apu = t.elapsed();
        let ms = |d: std::time::Duration| d.as_secs_f64() * 1e3 / frames as f64;
        let rest = ns_frame / 1e6 - ms(ppu) - ms(apu);
        println!(
            "profile: ppu={:.3} ms/frame ({:.0}%)  apu={:.3} ms/frame ({:.0}%)  cpu+bus≈{:.3} ms/frame ({:.0}%)",
            ms(ppu),
            ms(ppu) / (ns_frame / 1e6) * 100.0,
            ms(apu),
            ms(apu) / (ns_frame / 1e6) * 100.0,
            rest,
            rest / (ns_frame / 1e6) * 100.0
        );
    }
}
