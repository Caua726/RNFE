//! Desenho em software no overlay RGBA: texto (fontdue), barra de menu, botões, toque.
#![allow(clippy::too_many_arguments)]

use fontdue::Font;
use rnfe_core::Buttons;
use rnfe_frontend::touch::{Circle, Rect, TouchLayout};
use std::collections::HashMap;

const FONT_DATA: &[u8] = include_bytes!("../assets/NotoSans-Regular.ttf");

pub const MENUBAR_HEIGHT: i32 = 28;
const MENU_FONT_SIZE: f32 = 14.0;
const MENU_PAD_X: i32 = 12;
const DROPDOWN_ITEM_H: i32 = 26;
const DROPDOWN_PAD_X: i32 = 16;

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum MenuAction {
    None,
    OpenRom,
    Reset,
    SaveState,
    LoadState,
    Quit,
}

struct MenuItem {
    label: &'static str,
    items: &'static [(&'static str, MenuAction)],
}

const MENUS: &[MenuItem] = &[MenuItem {
    label: "File",
    items: &[
        ("Open ROM (O)", MenuAction::OpenRom),
        ("Reset (R)", MenuAction::Reset),
        ("Save state (F5)", MenuAction::SaveState),
        ("Load state (F7)", MenuAction::LoadState),
        ("Quit", MenuAction::Quit),
    ],
}];

/// Glifo rasterizado (cache por (char, tamanho×4)).
struct Glyph {
    metrics: fontdue::Metrics,
    bitmap: Vec<u8>,
}

pub struct Ui {
    font: Font,
    cache: HashMap<(char, u32), Glyph>,
    pub open_menu: Option<usize>,
}

impl Default for Ui {
    fn default() -> Self {
        Self::new()
    }
}

impl Ui {
    pub fn new() -> Self {
        let font = Font::from_bytes(FONT_DATA, fontdue::FontSettings::default()).expect("fonte embutida");
        Ui { font, cache: HashMap::new(), open_menu: None }
    }

    fn glyph(&mut self, ch: char, size: f32) -> &Glyph {
        let key = (ch, (size * 4.0) as u32);
        self.cache.entry(key).or_insert_with(|| {
            let (metrics, bitmap) = self.font.rasterize(ch, size);
            Glyph { metrics, bitmap }
        })
    }

    pub fn text_width(&mut self, text: &str, size: f32) -> i32 {
        text.chars().map(|c| self.glyph(c, size).metrics.advance_width as i32).sum()
    }

    pub fn draw_text(
        &mut self,
        fb: &mut [u8],
        w: u32,
        h: u32,
        text: &str,
        size: f32,
        x: i32,
        y: i32,
        color: [u8; 4],
    ) {
        let mut cursor_x = x;
        for ch in text.chars() {
            let g = self.glyph(ch, size);
            let m = g.metrics;
            let gx = cursor_x + m.xmin;
            let gy = y + (size as i32 - m.ymin - m.height as i32);
            for row in 0..m.height {
                let py = gy + row as i32;
                if py < 0 || py >= h as i32 {
                    continue;
                }
                for col in 0..m.width {
                    let alpha = g.bitmap[row * m.width + col];
                    if alpha == 0 {
                        continue;
                    }
                    let px = gx + col as i32;
                    if px < 0 || px >= w as i32 {
                        continue;
                    }
                    let idx = ((py as u32 * w + px as u32) * 4) as usize;
                    blend(&mut fb[idx..idx + 4], color, alpha);
                }
            }
            cursor_x += m.advance_width as i32;
        }
    }

    pub fn draw_text_centered(
        &mut self,
        fb: &mut [u8],
        w: u32,
        h: u32,
        text: &str,
        size: f32,
        y: i32,
        color: [u8; 4],
    ) {
        let tw = self.text_width(text, size);
        self.draw_text(fb, w, h, text, size, (w as i32 - tw) / 2, y, color);
    }

    pub fn button_rect(&mut self, text: &str, size: f32, cx: i32, cy: i32) -> (i32, i32, i32, i32) {
        let tw = self.text_width(text, size);
        let bw = tw + 40;
        let bh = size as i32 + 20;
        (cx - bw / 2, cy - bh / 2, bw, bh)
    }

    pub fn draw_button(
        &mut self,
        fb: &mut [u8],
        w: u32,
        h: u32,
        text: &str,
        size: f32,
        cx: i32,
        cy: i32,
        color: [u8; 4],
        border: [u8; 4],
    ) {
        let (bx, by, bw, bh) = self.button_rect(text, size, cx, cy);
        fill_rect(fb, w, h, bx, by, bw, 1, border);
        fill_rect(fb, w, h, bx, by + bh - 1, bw, 1, border);
        fill_rect(fb, w, h, bx, by, 1, bh, border);
        fill_rect(fb, w, h, bx + bw - 1, by, 1, bh, border);
        let tw = self.text_width(text, size);
        self.draw_text(fb, w, h, text, size, cx - tw / 2, by + 10, color);
    }

    /// Caixa de mensagem temporária, no rodapé.
    pub fn draw_toast(&mut self, fb: &mut [u8], w: u32, h: u32, msg: &str) {
        let tw = self.text_width(msg, 16.0);
        let tx = (w as i32 - tw) / 2;
        let ty = h as i32 - 50;
        fill_rect(fb, w, h, tx - 12, ty - 6, tw + 24, 28, [0, 0, 0, 180]);
        self.draw_text(fb, w, h, msg, 16.0, tx, ty, [255, 255, 255, 255]);
    }

    pub fn draw_menubar(&mut self, fb: &mut [u8], w: u32, h: u32, mx: i32, my: i32) {
        fill_rect(fb, w, h, 0, 0, w as i32, MENUBAR_HEIGHT, [22, 22, 28, 255]);
        fill_rect(fb, w, h, 0, MENUBAR_HEIGHT - 1, w as i32, 1, [40, 40, 50, 255]);
        let mut x = 0;
        for (i, menu) in MENUS.iter().enumerate() {
            let tw = self.text_width(menu.label, MENU_FONT_SIZE);
            let item_w = tw + MENU_PAD_X * 2;
            let hover = mx >= x && mx < x + item_w && (0..MENUBAR_HEIGHT).contains(&my);
            let active = self.open_menu == Some(i);
            if hover || active {
                fill_rect(fb, w, h, x, 0, item_w, MENUBAR_HEIGHT, [40, 40, 55, 255]);
            }
            let color = if hover || active { [255, 255, 255, 255] } else { [170, 170, 170, 255] };
            self.draw_text(fb, w, h, menu.label, MENU_FONT_SIZE, x + MENU_PAD_X, 7, color);
            if active {
                self.draw_dropdown(fb, w, h, x, menu.items, mx, my);
            }
            x += item_w;
        }
    }

    fn dropdown_width(&mut self, items: &[(&str, MenuAction)]) -> i32 {
        items.iter().map(|(l, _)| self.text_width(l, MENU_FONT_SIZE)).max().unwrap_or(0) + DROPDOWN_PAD_X * 2
    }

    fn draw_dropdown(
        &mut self,
        fb: &mut [u8],
        w: u32,
        h: u32,
        x: i32,
        items: &[(&str, MenuAction)],
        mx: i32,
        my: i32,
    ) {
        let dw = self.dropdown_width(items);
        let dh = items.len() as i32 * DROPDOWN_ITEM_H;
        let dy = MENUBAR_HEIGHT;
        fill_rect(fb, w, h, x, dy, dw, dh, [28, 28, 36, 255]);
        fill_rect(fb, w, h, x, dy, dw, 1, [50, 50, 60, 255]);
        fill_rect(fb, w, h, x, dy + dh - 1, dw, 1, [50, 50, 60, 255]);
        fill_rect(fb, w, h, x, dy, 1, dh, [50, 50, 60, 255]);
        fill_rect(fb, w, h, x + dw - 1, dy, 1, dh, [50, 50, 60, 255]);
        for (i, (label, _)) in items.iter().enumerate() {
            let iy = dy + i as i32 * DROPDOWN_ITEM_H;
            let hover = mx >= x && mx < x + dw && my >= iy && my < iy + DROPDOWN_ITEM_H;
            if hover {
                fill_rect(fb, w, h, x + 1, iy, dw - 2, DROPDOWN_ITEM_H, [50, 70, 120, 255]);
            }
            let color = if hover { [255, 255, 255, 255] } else { [170, 170, 170, 255] };
            self.draw_text(fb, w, h, label, MENU_FONT_SIZE, x + DROPDOWN_PAD_X, iy + 6, color);
        }
    }

    /// Clique/toque na barra de menu ou no dropdown aberto.
    pub fn handle_click(&mut self, mx: i32, my: i32) -> MenuAction {
        if (0..MENUBAR_HEIGHT).contains(&my) {
            let mut x = 0;
            for (i, menu) in MENUS.iter().enumerate() {
                let item_w = self.text_width(menu.label, MENU_FONT_SIZE) + MENU_PAD_X * 2;
                if mx >= x && mx < x + item_w {
                    self.open_menu = if self.open_menu == Some(i) { None } else { Some(i) };
                    return MenuAction::None;
                }
                x += item_w;
            }
        }
        if let Some(mi) = self.open_menu {
            let menu = &MENUS[mi];
            let mut x = 0;
            for m in &MENUS[..mi] {
                x += self.text_width(m.label, MENU_FONT_SIZE) + MENU_PAD_X * 2;
            }
            let dw = self.dropdown_width(menu.items);
            for (i, (_, action)) in menu.items.iter().enumerate() {
                let iy = MENUBAR_HEIGHT + i as i32 * DROPDOWN_ITEM_H;
                if mx >= x && mx < x + dw && my >= iy && my < iy + DROPDOWN_ITEM_H {
                    self.open_menu = None;
                    return *action;
                }
            }
            self.open_menu = None;
        }
        MenuAction::None
    }

    /// Controles de toque translúcidos; botões pressionados ficam mais claros.
    pub fn draw_touch_controls(
        &mut self,
        fb: &mut [u8],
        w: u32,
        h: u32,
        layout: &TouchLayout,
        pressed: Buttons,
    ) {
        let base = [255, 255, 255, 70];
        let hot = [255, 255, 255, 150];
        let col = |b: Buttons| if pressed.0 & b.0 != 0 { hot } else { base };
        // d-pad: cruz de dois retângulos + anel
        let d = &layout.dpad;
        let arm = d.r * 0.36;
        fill_rect_f(fb, w, h, d.cx - arm, d.cy - d.r, arm * 2.0, d.r * 2.0, col(Buttons::UP | Buttons::DOWN));
        fill_rect_f(
            fb,
            w,
            h,
            d.cx - d.r,
            d.cy - arm,
            d.r * 2.0,
            arm * 2.0,
            col(Buttons::LEFT | Buttons::RIGHT),
        );
        for (b, dx, dy) in [
            (Buttons::UP, 0.0, -0.7),
            (Buttons::DOWN, 0.0, 0.7),
            (Buttons::LEFT, -0.7, 0.0),
            (Buttons::RIGHT, 0.7, 0.0),
        ] {
            if pressed.0 & b.0 != 0 {
                fill_circle(
                    fb,
                    w,
                    h,
                    &Circle { cx: d.cx + dx * d.r, cy: d.cy + dy * d.r, r: arm * 0.8 },
                    hot,
                );
            }
        }
        fill_circle(fb, w, h, &layout.a, col(Buttons::A));
        fill_circle(fb, w, h, &layout.b, col(Buttons::B));
        fill_rect_r(fb, w, h, &layout.start, col(Buttons::START));
        fill_rect_r(fb, w, h, &layout.select, col(Buttons::SELECT));
        fill_rect_r(fb, w, h, &layout.menu, [255, 255, 255, 50]);
        let label = |ui: &mut Ui, fb: &mut [u8], text: &str, cx: f32, cy: f32, size: f32| {
            let tw = ui.text_width(text, size);
            ui.draw_text(
                fb,
                w,
                h,
                text,
                size,
                cx as i32 - tw / 2,
                cy as i32 - size as i32 / 2,
                [255, 255, 255, 200],
            );
        };
        let fs = (layout.a.r * 0.9).max(10.0);
        label(self, fb, "A", layout.a.cx, layout.a.cy, fs);
        label(self, fb, "B", layout.b.cx, layout.b.cy, fs);
        let ps = (layout.start.h * 0.6).max(9.0);
        label(
            self,
            fb,
            "START",
            layout.start.x + layout.start.w / 2.0,
            layout.start.y + layout.start.h / 2.0,
            ps,
        );
        label(
            self,
            fb,
            "SELECT",
            layout.select.x + layout.select.w / 2.0,
            layout.select.y + layout.select.h / 2.0,
            ps,
        );
        label(self, fb, "MENU", layout.menu.x + layout.menu.w / 2.0, layout.menu.y + layout.menu.h / 2.0, ps);
    }
}

#[inline]
fn blend(dst: &mut [u8], color: [u8; 4], alpha: u8) {
    let a = alpha as u32 * color[3] as u32 / 255;
    if a == 0 {
        return;
    }
    for i in 0..3 {
        dst[i] = ((dst[i] as u32 * (255 - a) + color[i] as u32 * a) / 255) as u8;
    }
    dst[3] = dst[3].max(a as u8);
}

pub fn fill_rect(fb: &mut [u8], w: u32, h: u32, rx: i32, ry: i32, rw: i32, rh: i32, color: [u8; 4]) {
    for py in ry.max(0)..(ry + rh).min(h as i32) {
        for px in rx.max(0)..(rx + rw).min(w as i32) {
            let idx = ((py as u32 * w + px as u32) * 4) as usize;
            if color[3] == 255 {
                fb[idx..idx + 4].copy_from_slice(&color);
            } else {
                blend(&mut fb[idx..idx + 4], color, 255);
            }
        }
    }
}

fn fill_rect_f(fb: &mut [u8], w: u32, h: u32, x: f32, y: f32, rw: f32, rh: f32, color: [u8; 4]) {
    fill_rect(fb, w, h, x as i32, y as i32, rw as i32, rh as i32, color);
}

fn fill_rect_r(fb: &mut [u8], w: u32, h: u32, r: &Rect, color: [u8; 4]) {
    fill_rect_f(fb, w, h, r.x, r.y, r.w, r.h, color);
}

fn fill_circle(fb: &mut [u8], w: u32, h: u32, c: &Circle, color: [u8; 4]) {
    let r2 = c.r * c.r;
    let y0 = (c.cy - c.r).max(0.0) as i32;
    let y1 = ((c.cy + c.r) as i32 + 1).min(h as i32);
    let x0 = (c.cx - c.r).max(0.0) as i32;
    let x1 = ((c.cx + c.r) as i32 + 1).min(w as i32);
    for py in y0..y1 {
        let dy = py as f32 + 0.5 - c.cy;
        for px in x0..x1 {
            let dx = px as f32 + 0.5 - c.cx;
            if dx * dx + dy * dy <= r2 {
                let idx = ((py as u32 * w + px as u32) * 4) as usize;
                blend(&mut fb[idx..idx + 4], color, 255);
            }
        }
    }
}

/// Fundo opaco (tela inicial / pausa).
pub fn clear(fb: &mut [u8], color: [u8; 4]) {
    for px in fb.chunks_exact_mut(4) {
        px.copy_from_slice(&color);
    }
}
