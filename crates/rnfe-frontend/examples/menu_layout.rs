//! Mostra como os menus de toque ficam num tamanho de tela, em ASCII:
//! `cargo run -p rnfe-frontend --example menu_layout -- 1080 2340 paused [dpi]`
use rnfe_frontend::config::{Config, RecentRom};
use rnfe_frontend::menu::{MenuState, Screen, layout};

fn main() {
    let args: Vec<String> = std::env::args().collect();
    let w: f32 = args.get(1).and_then(|s| s.parse().ok()).unwrap_or(1080.0);
    let h: f32 = args.get(2).and_then(|s| s.parse().ok()).unwrap_or(2340.0);
    let screen = match args.get(3).map(|s| s.as_str()) {
        Some("start") => Screen::Start,
        Some("settings") => Screen::Settings,
        Some("recents") => Screen::Recents,
        Some("states") => Screen::States,
        _ => Screen::Paused,
    };
    let st = MenuState {
        has_rom: true,
        rom_name: "jogo.nes".into(),
        turbo: false,
        can_quit: true,
        recent: vec![
            RecentRom { hash: 1, name: "Blade Buster".into() },
            RecentRom { hash: 2, name: "nestest".into() },
        ],
        version: "0.2.0".into(),
        slots: [true, false, false],
        ..Default::default()
    };
    let dpi: f32 =
        args.get(4).and_then(|s| s.parse().ok()).unwrap_or(if w.min(h) >= 1000.0 { 2.6 } else { 1.0 });
    let l = layout(screen, w, h, &Config::default(), dpi, &st);
    println!(
        "{:?} em {w}x{h}: escala {:.2}, fonte {:.0}px, {} itens",
        screen,
        l.ui_scale,
        l.font,
        l.items.len()
    );
    let (cols, rows) = (64usize, 40usize);
    let mut grid = vec![vec![' '; cols]; rows];
    for item in &l.items {
        let x0 = (item.rect.x / w * cols as f32) as usize;
        let x1 = (((item.rect.x + item.rect.w) / w * cols as f32) as usize).min(cols - 1);
        let y0 = (item.rect.y / h * rows as f32) as usize;
        let y1 = (((item.rect.y + item.rect.h) / h * rows as f32) as usize).min(rows - 1);
        for (y, row) in grid.iter_mut().enumerate().take(y1 + 1).skip(y0) {
            for (x, cell) in row.iter_mut().enumerate().take(x1 + 1).skip(x0) {
                *cell = if y == y0 || y == y1 || x == x0 || x == x1 { '#' } else { '.' };
            }
        }
        let label: Vec<char> = item.label.chars().take(x1.saturating_sub(x0 + 1)).collect();
        let ly = (y0 + y1) / 2;
        for (i, c) in label.iter().enumerate() {
            grid[ly][x0 + 1 + i] = *c;
        }
    }
    for row in grid {
        println!("{}", row.into_iter().collect::<String>());
    }
}
