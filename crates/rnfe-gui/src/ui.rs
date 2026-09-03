//! Desenho em software no overlay RGBA: texto (fontdue), botões grandes, listas, toque.
#![allow(clippy::too_many_arguments)]

use fontdue::Font;
use rnfe_core::Buttons;
use rnfe_frontend::touch::{Circle, Rect, TouchLayout};
use std::collections::HashMap;

const FONT_DATA: &[u8] = include_bytes!("../assets/NotoSans-Regular.ttf");

/// Cores de um tema (normal ou alto contraste).
#[derive(Clone, Copy)]
pub struct Theme {
    pub bg: [u8; 4],
    pub panel: [u8; 4],
    pub text: [u8; 4],
    pub dim: [u8; 4],
    pub button: [u8; 4],
    pub button_hot: [u8; 4],
    pub border: [u8; 4],
    pub accent: [u8; 4],
    pub touch: [u8; 4],
    pub touch_hot: [u8; 4],
}

impl Theme {
    pub fn normal() -> Theme {
        Theme {
            bg: [12, 12, 16, 235],
            panel: [22, 22, 30, 255],
            text: [225, 225, 230, 255],
            dim: [130, 130, 140, 255],
            button: [38, 38, 52, 255],
            button_hot: [70, 90, 140, 255],
            border: [70, 70, 90, 255],
            accent: [120, 170, 255, 255],
            touch: [255, 255, 255, 255],
            touch_hot: [255, 255, 255, 255],
        }
    }

    pub fn high_contrast() -> Theme {
        Theme {
            bg: [0, 0, 0, 255],
            panel: [0, 0, 0, 255],
            text: [255, 255, 255, 255],
            dim: [220, 220, 220, 255],
            button: [0, 0, 0, 255],
            button_hot: [255, 255, 0, 255],
            border: [255, 255, 255, 255],
            accent: [255, 255, 0, 255],
            touch: [255, 255, 0, 255],
            touch_hot: [255, 255, 255, 255],
        }
    }
}

struct Glyph {
    metrics: fontdue::Metrics,
    bitmap: Vec<u8>,
}

pub struct Ui {
    font: Font,
    cache: HashMap<(char, u32), Glyph>,
}

impl Default for Ui {
    fn default() -> Self {
        Self::new()
    }
}

impl Ui {
    pub fn new() -> Self {
        let font = Font::from_bytes(FONT_DATA, fontdue::FontSettings::default()).expect("fonte embutida");
        Ui { font, cache: HashMap::new() }
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

    /// Texto cortado com "…" para caber em `max_w`.
    pub fn draw_text_clipped(
        &mut self,
        fb: &mut [u8],
        w: u32,
        h: u32,
        text: &str,
        size: f32,
        x: i32,
        y: i32,
        max_w: i32,
        color: [u8; 4],
    ) {
        if self.text_width(text, size) <= max_w {
            self.draw_text(fb, w, h, text, size, x, y, color);
            return;
        }
        let mut s: String = text.chars().collect();
        while !s.is_empty() && self.text_width(&format!("{s}…"), size) > max_w {
            s.pop();
        }
        let s = format!("{s}…");
        self.draw_text(fb, w, h, &s, size, x, y, color);
    }

    /// Botão retangular grande com texto centralizado.
    pub fn draw_button_rect(
        &mut self,
        fb: &mut [u8],
        w: u32,
        h: u32,
        r: &Rect,
        text: &str,
        size: f32,
        hot: bool,
        theme: &Theme,
    ) {
        let (x, y, bw, bh) = (r.x as i32, r.y as i32, r.w as i32, r.h as i32);
        fill_rect(fb, w, h, x, y, bw, bh, if hot { theme.button_hot } else { theme.button });
        outline(fb, w, h, x, y, bw, bh, theme.border);
        let tw = self.text_width(text, size);
        self.draw_text(
            fb,
            w,
            h,
            text,
            size,
            x + (bw - tw) / 2,
            y + (bh - size as i32) / 2 - size as i32 / 8,
            theme.text,
        );
    }

    /// Caixa de mensagem temporária, no rodapé.
    pub fn draw_toast(&mut self, fb: &mut [u8], w: u32, h: u32, msg: &str, size: f32) {
        let tw = self.text_width(msg, size);
        let tx = (w as i32 - tw) / 2;
        let ty = h as i32 - (size * 3.0) as i32;
        fill_rect(fb, w, h, tx - 12, ty - 6, tw + 24, size as i32 + 14, [0, 0, 0, 200]);
        self.draw_text(fb, w, h, msg, size, tx, ty, [255, 255, 255, 255]);
    }

    /// Controles de toque translúcidos; botões pressionados ficam mais claros.
    pub fn draw_touch_controls(
        &mut self,
        fb: &mut [u8],
        w: u32,
        h: u32,
        layout: &TouchLayout,
        pressed: Buttons,
        opacity: f32,
        theme: &Theme,
        high_contrast: bool,
    ) {
        let a = (opacity.clamp(0.1, 1.0) * 255.0) as u8;
        let base = [theme.touch[0], theme.touch[1], theme.touch[2], a];
        let hot = [theme.touch_hot[0], theme.touch_hot[1], theme.touch_hot[2], a.saturating_add(90)];
        let col = |b: Buttons| if pressed.0 & b.0 != 0 { hot } else { base };
        let d = &layout.dpad;
        let arm = d.r * 0.36;
        if high_contrast {
            // contorno escuro para separar do jogo
            let dark = [0, 0, 0, 200];
            fill_rect_f(fb, w, h, d.cx - arm - 3.0, d.cy - d.r - 3.0, arm * 2.0 + 6.0, d.r * 2.0 + 6.0, dark);
            fill_rect_f(fb, w, h, d.cx - d.r - 3.0, d.cy - arm - 3.0, d.r * 2.0 + 6.0, arm * 2.0 + 6.0, dark);
            fill_circle(fb, w, h, &Circle { cx: layout.a.cx, cy: layout.a.cy, r: layout.a.r + 3.0 }, dark);
            fill_circle(fb, w, h, &Circle { cx: layout.b.cx, cy: layout.b.cy, r: layout.b.r + 3.0 }, dark);
        }
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
        fill_rect_r(fb, w, h, &layout.menu, [base[0], base[1], base[2], a / 2 + 20]);
        let label_color = if high_contrast { [0, 0, 0, 255] } else { [255, 255, 255, 220] };
        let label = |ui: &mut Ui, fb: &mut [u8], text: &str, cx: f32, cy: f32, size: f32| {
            let tw = ui.text_width(text, size);
            ui.draw_text(fb, w, h, text, size, cx as i32 - tw / 2, cy as i32 - size as i32 / 2, label_color);
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

pub fn outline(fb: &mut [u8], w: u32, h: u32, x: i32, y: i32, bw: i32, bh: i32, color: [u8; 4]) {
    fill_rect(fb, w, h, x, y, bw, 2, color);
    fill_rect(fb, w, h, x, y + bh - 2, bw, 2, color);
    fill_rect(fb, w, h, x, y, 2, bh, color);
    fill_rect(fb, w, h, x + bw - 2, y, 2, bh, color);
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

/// Fundo (tela inicial / pausa).
pub fn clear(fb: &mut [u8], color: [u8; 4]) {
    for px in fb.chunks_exact_mut(4) {
        px.copy_from_slice(&color);
    }
}
