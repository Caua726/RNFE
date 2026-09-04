//! RNFE no terminal.
//!
//! Cada caractere `▀` mostra dois pixels (cor de frente = de cima, fundo = de baixo), em ANSI
//! 24-bit. A imagem é reduzida por média para caber no terminal; só as células que mudaram
//! são reescritas a cada frame.
//!
//! Teclas: setas · Z=A · X=B · Enter=Start · Tab/C=Select · R=reset · Backspace=rewind ·
//! 1=salvar state · 2=carregar state · Q ou Ctrl-C=sair
//!
//! `rnfe-tty rom.nes [--headless] [--frames N] [--draw-every N] [--panic-test]`

use rnfe_core::{Buttons, Cartridge, Nes, SCREEN_H, SCREEN_W, Storage};
use rnfe_frontend::{FramePacer, FsStorage, InputState, NTSC_FPS, Rewind, SaveManager};
use std::io::{Read, Write};
use std::process::{Command, Stdio};
use std::sync::mpsc;
use std::time::{Duration, Instant};

const HOLD: Duration = Duration::from_millis(120);

struct Args {
    rom: String,
    headless: bool,
    frames: u32,
    draw_every: u32,
    panic_test: bool,
    /// `--frames` também encerra o modo interativo (smoke tests, bench do renderer).
    frames_limit: bool,
}

fn parse_args() -> Args {
    let mut a = Args {
        rom: String::new(),
        headless: false,
        frames: 600,
        draw_every: 2,
        panic_test: false,
        frames_limit: false,
    };
    let mut it = std::env::args().skip(1);
    while let Some(arg) = it.next() {
        match arg.as_str() {
            "--headless" => a.headless = true,
            "--frames" => {
                a.frames = it.next().and_then(|s| s.parse().ok()).unwrap_or(a.frames);
                a.frames_limit = true;
            }
            "--draw-every" => a.draw_every = it.next().and_then(|s| s.parse().ok()).unwrap_or(2).max(1),
            "--panic-test" => a.panic_test = true,
            "-h" | "--help" => {
                eprintln!("uso: rnfe-tty <rom.nes> [--headless --frames N] [--draw-every N] [--panic-test]");
                std::process::exit(0);
            }
            other if a.rom.is_empty() => a.rom = other.to_string(),
            other => {
                eprintln!("argumento desconhecido: {other}");
                std::process::exit(2);
            }
        }
    }
    if a.rom.is_empty() {
        eprintln!("uso: rnfe-tty <rom.nes> [--headless --frames N] [--draw-every N] [--panic-test]");
        std::process::exit(2);
    }
    a
}

// ------------------------------------------------------------------ terminal

/// Estado do terminal salvo por `stty -g`; restaurado no drop e no panic hook.
struct RawGuard {
    saved: String,
}

impl RawGuard {
    fn enter() -> Result<Self, String> {
        let out = Command::new("stty")
            .arg("-g")
            .stdin(Stdio::inherit())
            .output()
            .map_err(|e| format!("stty não encontrado ({e})"))?;
        if !out.status.success() {
            return Err("stdin não é um terminal (use --headless)".into());
        }
        let saved = String::from_utf8_lossy(&out.stdout).trim().to_string();
        Command::new("stty")
            .args(["raw", "-echo"])
            .stdin(Stdio::inherit())
            .status()
            .map_err(|e| e.to_string())?;
        // alt screen, cursor invisível, limpa
        print!("\x1b[?1049h\x1b[?25l\x1b[2J");
        let _ = std::io::stdout().flush();
        Ok(Self { saved })
    }

    fn restore(saved: &str) {
        print!("\x1b[0m\x1b[?25h\x1b[?1049l");
        let _ = std::io::stdout().flush();
        let _ = Command::new("stty").arg(saved).stdin(Stdio::inherit()).status();
    }
}

impl Drop for RawGuard {
    fn drop(&mut self) {
        Self::restore(&self.saved);
    }
}

fn term_size() -> (usize, usize) {
    let out = Command::new("stty").arg("size").stdin(Stdio::inherit()).output().ok();
    let s = out.map(|o| String::from_utf8_lossy(&o.stdout).to_string()).unwrap_or_default();
    let mut it = s.split_whitespace().filter_map(|v| v.parse::<usize>().ok());
    match (it.next(), it.next()) {
        (Some(r), Some(c)) if r > 0 && c > 0 => (r, c),
        _ => (24, 80),
    }
}

// ------------------------------------------------------------------ input

enum Key {
    Button(Buttons),
    Reset,
    Quit,
    /// Backspace: volta no tempo (um state por toque; segurar repete)
    Rewind,
    SaveState,
    LoadState,
}

fn spawn_input() -> mpsc::Receiver<Key> {
    let (tx, rx) = mpsc::channel();
    std::thread::spawn(move || {
        let mut stdin = std::io::stdin().lock();
        let mut buf = [0u8; 1];
        let mut esc = Vec::new();
        while stdin.read(&mut buf).map(|n| n == 1).unwrap_or(false) {
            let b = buf[0];
            let key = if !esc.is_empty() {
                esc.push(b);
                match esc.as_slice() {
                    [0x1b, b'['] => continue,
                    [0x1b, b'[', b'A'] => Some(Key::Button(Buttons::UP)),
                    [0x1b, b'[', b'B'] => Some(Key::Button(Buttons::DOWN)),
                    [0x1b, b'[', b'C'] => Some(Key::Button(Buttons::RIGHT)),
                    [0x1b, b'[', b'D'] => Some(Key::Button(Buttons::LEFT)),
                    _ => None,
                }
                .inspect(|_| esc.clear())
                .or_else(|| {
                    esc.clear();
                    None
                })
            } else {
                match b {
                    0x1b => {
                        esc.push(b);
                        continue;
                    }
                    0x03 | b'q' | b'Q' => Some(Key::Quit),
                    b'r' | b'R' => Some(Key::Reset),
                    0x7F | 0x08 => Some(Key::Rewind),
                    b'1' => Some(Key::SaveState),
                    b'2' => Some(Key::LoadState),
                    b'z' | b'Z' => Some(Key::Button(Buttons::A)),
                    b'x' | b'X' => Some(Key::Button(Buttons::B)),
                    b'\r' | b'\n' => Some(Key::Button(Buttons::START)),
                    b'\t' | b'c' | b'C' => Some(Key::Button(Buttons::SELECT)),
                    b'w' | b'W' => Some(Key::Button(Buttons::UP)),
                    b's' | b'S' => Some(Key::Button(Buttons::DOWN)),
                    b'a' | b'A' => Some(Key::Button(Buttons::LEFT)),
                    b'd' | b'D' => Some(Key::Button(Buttons::RIGHT)),
                    _ => None,
                }
            };
            if let Some(k) = key {
                if tx.send(k).is_err() {
                    break;
                }
            }
        }
    });
    rx
}

// ------------------------------------------------------------------ render

struct Renderer {
    div: usize,
    cols: usize,
    rows: usize,
    /// (cor de cima, cor de baixo) por célula, do último frame desenhado
    prev: Vec<(u32, u32)>,
    out: String,
}

impl Renderer {
    fn new(term_rows: usize, term_cols: usize) -> Self {
        let usable_rows = term_rows.saturating_sub(1).max(1); // última linha = status
        let div = (SCREEN_W.div_ceil(term_cols)).max(SCREEN_H.div_ceil(usable_rows * 2)).max(1);
        let cols = SCREEN_W / div;
        let rows = (SCREEN_H / div).div_ceil(2);
        Self {
            div,
            cols,
            rows,
            prev: vec![(u32::MAX, u32::MAX); cols * rows],
            out: String::with_capacity(64 * 1024),
        }
    }

    /// Média dos `div×div` pixels da célula (x, y) em coordenadas de célula.
    fn cell(&self, fb: &[u8], cx: usize, cy: usize) -> u32 {
        let (mut r, mut g, mut b, mut n) = (0u32, 0u32, 0u32, 0u32);
        for y in cy * self.div..((cy + 1) * self.div).min(SCREEN_H) {
            for x in cx * self.div..((cx + 1) * self.div).min(SCREEN_W) {
                let i = (y * SCREEN_W + x) * 4;
                r += fb[i] as u32;
                g += fb[i + 1] as u32;
                b += fb[i + 2] as u32;
                n += 1;
            }
        }
        if n == 0 {
            return 0;
        }
        ((r / n) << 16) | ((g / n) << 8) | (b / n)
    }

    fn draw(&mut self, fb: &[u8], status: &str, force: bool) {
        use std::fmt::Write as _;
        self.out.clear();
        let (mut cur_fg, mut cur_bg) = (u32::MAX, u32::MAX);
        for row in 0..self.rows {
            let mut cursor_placed = false;
            for col in 0..self.cols {
                let top = self.cell(fb, col, row * 2);
                let bot =
                    if (row * 2 + 1) * self.div < SCREEN_H { self.cell(fb, col, row * 2 + 1) } else { 0 };
                let idx = row * self.cols + col;
                if !force && self.prev[idx] == (top, bot) {
                    cursor_placed = false;
                    continue;
                }
                self.prev[idx] = (top, bot);
                if !cursor_placed {
                    let _ = write!(self.out, "\x1b[{};{}H", row + 1, col + 1);
                    cursor_placed = true;
                }
                if top != cur_fg {
                    let _ = write!(self.out, "\x1b[38;2;{};{};{}m", top >> 16, (top >> 8) & 255, top & 255);
                    cur_fg = top;
                }
                if bot != cur_bg {
                    let _ = write!(self.out, "\x1b[48;2;{};{};{}m", bot >> 16, (bot >> 8) & 255, bot & 255);
                    cur_bg = bot;
                }
                self.out.push('▀');
            }
        }
        let _ = write!(self.out, "\x1b[0m\x1b[{};1H\x1b[2K{}", self.rows + 1, status);
        let mut so = std::io::stdout().lock();
        let _ = so.write_all(self.out.as_bytes());
        let _ = so.flush();
    }
}

// ------------------------------------------------------------------ main

fn load(path: &str) -> Nes {
    let bytes = match std::fs::read(path) {
        Ok(b) => b,
        Err(e) => {
            eprintln!("não consegui ler {path}: {e}");
            std::process::exit(1);
        }
    };
    match Cartridge::from_bytes(&bytes) {
        Ok(c) => Nes::new(c),
        Err(e) => {
            eprintln!("{path}: {e}");
            std::process::exit(1);
        }
    }
}

fn headless(mut nes: Nes, frames: u32) {
    let t = Instant::now();
    let mut audio = Vec::new();
    for _ in 0..frames {
        nes.run_frame();
        audio.clear();
        nes.drain_audio(&mut audio);
    }
    let dt = t.elapsed().as_secs_f64();
    println!(
        "{frames} frames em {dt:.2}s = {:.1} fps ({:.1}x tempo real)",
        frames as f64 / dt,
        frames as f64 / dt / NTSC_FPS
    );
}

fn main() {
    let args = parse_args();
    let mut nes = load(&args.rom);
    if args.headless {
        headless(nes, args.frames);
        return;
    }
    // Save com bateria (.sav) na pasta de dados; ~/.local/share/rnfe por padrão
    let mut storage = FsStorage::new(FsStorage::default_dir());
    let mut save = SaveManager::new(&nes);
    if save.load(&mut nes, &storage) {
        eprintln!("save carregado: {}", storage.dir().join(save.key().unwrap_or("")).display());
    }
    let mut rewind = Rewind::new(Rewind::DEFAULT_CAP);
    let state_key = format!("state/{:016x}/1.rnfs", nes.cartridge().rom_hash());
    let mut msg = String::new();

    let guard = match RawGuard::enter() {
        Ok(g) => g,
        Err(e) => {
            eprintln!("{e}");
            std::process::exit(1);
        }
    };
    let saved = guard.saved.clone();
    let prev_hook = std::panic::take_hook();
    std::panic::set_hook(Box::new(move |info| {
        RawGuard::restore(&saved);
        prev_hook(info);
    }));

    let (rows, cols) = term_size();
    let mut renderer = Renderer::new(rows, cols);
    let keys = spawn_input();
    let mut input = InputState::new();
    // a região vem do header da ROM (PAL roda a 50 Hz)
    let mut pacer = FramePacer::new(nes.region().fps());
    let start = Instant::now();
    let now = || start.elapsed();

    let mut frame_count: u64 = 0;
    let mut audio = Vec::new();
    let mut fps_frames = 0u32;
    let mut fps_t = now();
    let mut fps = 0.0;
    let mut force = true;

    'main: loop {
        while let Ok(k) = keys.try_recv() {
            match k {
                Key::Quit => break 'main,
                Key::Reset => {
                    nes.reset();
                    rewind.clear();
                }
                Key::Rewind => {
                    if rewind.step_back(&mut nes) {
                        force = true;
                    }
                }
                Key::SaveState => {
                    msg = match storage.write(&state_key, &nes.save_state()) {
                        Ok(()) => "state salvo (1)".into(),
                        Err(e) => format!("erro: {e}"),
                    };
                }
                Key::LoadState => {
                    msg = match storage.read(&state_key) {
                        Some(d) => match nes.load_state(&d) {
                            Ok(()) => {
                                rewind.clear();
                                force = true;
                                "state carregado (2)".into()
                            }
                            Err(e) => format!("erro: {e}"),
                        },
                        None => "sem state salvo".into(),
                    };
                }
                Key::Button(b) => input.pulse(b, now(), HOLD),
            }
        }
        let due = pacer.frames_due(now());
        for _ in 0..due {
            nes.set_controller(0, input.current(now()));
            nes.run_frame();
            rewind.record(&nes);
            if let Err(e) = save.tick(&mut nes, &mut storage) {
                eprintln!("erro ao gravar save: {e}");
            }
            audio.clear();
            nes.drain_audio(&mut audio);
            frame_count += 1;
            fps_frames += 1;
            if args.panic_test && frame_count == 30 {
                panic!("--panic-test: o terminal deve voltar ao normal");
            }
            if args.frames_limit && frame_count >= args.frames as u64 {
                break 'main;
            }
        }
        if due > 0 && frame_count % args.draw_every as u64 == 0 {
            let el = now() - fps_t;
            if el >= Duration::from_secs(1) {
                fps = fps_frames as f64 / el.as_secs_f64();
                fps_frames = 0;
                fps_t = now();
            }
            let status = format!(
                "RNFE tty {}x{} frame {} {:.0} fps | Z X Enter Tab R Bksp=rewind 1/2=state Q {}",
                renderer.cols,
                renderer.rows * 2,
                frame_count,
                fps,
                msg
            );
            renderer.draw(nes.framebuffer(), &status, force);
            force = false;
        }
        let wait = pacer.next_deadline().saturating_sub(now());
        if wait > Duration::from_millis(1) {
            std::thread::sleep(wait.min(Duration::from_millis(16)));
        }
    }
    drop(guard);
    match save.flush(&mut nes, &mut storage) {
        Ok(true) => eprintln!("save gravado em {}", storage.dir().join(save.key().unwrap_or("")).display()),
        Ok(false) => {}
        Err(e) => eprintln!("erro ao gravar save: {e}"),
    }
}
