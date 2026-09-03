//! Varredura de compatibilidade: roda cada `.nes` de uma pasta por N frames, sem frontend, e
//! classifica. `cargo run -p rnfe-core --release --example sweep -- <pasta> [frames] [threads] > out.csv`
//!
//! Colunas: status;mapper;frames;ms;ciclos;cores;arquivo. Status: ok, blank (tela de uma cor
//! só), jam (CPU travada), stuck (PC parado num laço sem VBL), erro (não carregou), panic.
use rnfe_core::{Buttons, Cartridge, Nes};
use std::collections::BTreeSet;
use std::path::PathBuf;
use std::sync::{Arc, Mutex};
use std::time::Instant;

fn collect(dir: &std::path::Path, out: &mut Vec<PathBuf>) {
    if let Ok(rd) = std::fs::read_dir(dir) {
        for e in rd.flatten() {
            let p = e.path();
            if p.is_dir() {
                collect(&p, out);
            } else if p.extension().is_some_and(|x| x.eq_ignore_ascii_case("nes")) {
                out.push(p);
            }
        }
    }
}

fn run_one(path: &PathBuf, frames: u32) -> String {
    let name = path.display().to_string();
    let bytes = match std::fs::read(path) {
        Ok(b) => b,
        Err(e) => return format!("erro;-;0;0;0;0;{name};{e}"),
    };
    let cart = match Cartridge::from_bytes(&bytes) {
        Ok(c) => c,
        Err(e) => return format!("erro;-;0;0;0;0;{name};{e}"),
    };
    let mapper = cart.mapper_id();
    let r = std::panic::catch_unwind(move || {
        let mut nes = Nes::new(cart);
        let t = Instant::now();
        let mut colors = BTreeSet::new();
        for f in 0..frames {
            // START a cada 2 s por 10 frames, para sair de telas de título
            let b = if f % 120 < 10 && f > 60 { Buttons::START } else { Buttons::NONE };
            nes.set_controller(0, b);
            nes.run_frame();
            if nes.cpu.jammed {
                return ("jam".to_string(), f, t.elapsed().as_millis(), nes.cpu_cycles(), 0usize);
            }
        }
        for &c in nes.framebuffer_indexed().iter() {
            colors.insert(c & 0x3F);
        }
        let status = if colors.len() <= 1 { "blank" } else { "ok" };
        (status.to_string(), frames, t.elapsed().as_millis(), nes.cpu_cycles(), colors.len())
    });
    match r {
        Ok((status, f, ms, cyc, colors)) => format!("{status};{mapper};{f};{ms};{cyc};{colors};{name}"),
        Err(_) => format!("panic;{mapper};0;0;0;0;{name}"),
    }
}

fn main() {
    let args: Vec<String> = std::env::args().collect();
    let dir = PathBuf::from(args.get(1).cloned().unwrap_or_else(|| ".".into()));
    let frames: u32 = args.get(2).and_then(|s| s.parse().ok()).unwrap_or(300);
    let threads: usize = args.get(3).and_then(|s| s.parse().ok()).unwrap_or(4);
    let mut roms = Vec::new();
    collect(&dir, &mut roms);
    roms.sort();
    eprintln!("{} ROMs, {frames} frames, {threads} threads", roms.len());
    std::panic::set_hook(Box::new(|_| {})); // silencia o panic de cada ROM
    let queue = Arc::new(Mutex::new(roms.into_iter()));
    let results = Arc::new(Mutex::new(Vec::new()));
    let handles: Vec<_> = (0..threads)
        .map(|_| {
            let queue = queue.clone();
            let results = results.clone();
            std::thread::spawn(move || {
                loop {
                    let next = queue.lock().unwrap().next();
                    let Some(p) = next else { break };
                    let line = run_one(&p, frames);
                    let mut r = results.lock().unwrap();
                    r.push(line.clone());
                    if r.len() % 100 == 0 {
                        eprintln!("{}…", r.len());
                    }
                    println!("{line}");
                }
            })
        })
        .collect();
    for h in handles {
        let _ = h.join();
    }
    let r = results.lock().unwrap();
    let mut counts = std::collections::BTreeMap::new();
    for line in r.iter() {
        *counts.entry(line.split(';').next().unwrap_or("?").to_string()).or_insert(0) += 1;
    }
    eprintln!("resumo: {counts:?}");
}
