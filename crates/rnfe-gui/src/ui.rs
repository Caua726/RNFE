//! Desenho em software no overlay RGBA: texto (fontdue), botões grandes, listas, toque.
#![allow(clippy::too_many_arguments)]

use fontdue::Font;
use rnfe_core::Buttons;
use rnfe_frontend::menu::{Item, Layout};
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
    pub accent_hot: [u8; 4],
    pub on_accent: [u8; 4],
    pub danger: [u8; 4],
    pub danger_text: [u8; 4],
    pub track: [u8; 4],
    /// Contorno em todo item (alto contraste).
    pub outline: bool,
    pub touch: [u8; 4],
    pub touch_hot: [u8; 4],
}

impl Theme {
    pub fn normal() -> Theme {
        Theme {
            bg: [12, 12, 16, 235],
            panel: [18, 18, 26, 255],
            text: [225, 225, 230, 255],
            dim: [130, 130, 140, 255],
            button: [40, 40, 56, 255],
            button_hot: [72, 72, 100, 255],
            border: [70, 70, 90, 255],
            accent: [232, 84, 74, 255],
            accent_hot: [255, 120, 108, 255],
            on_accent: [255, 250, 245, 255],
            danger: [160, 40, 40, 255],
            danger_text: [255, 150, 140, 255],
            track: [60, 60, 76, 255],
            outline: false,
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
            accent_hot: [255, 255, 160, 255],
            on_accent: [0, 0, 0, 255],
            danger: [255, 60, 60, 255],
            danger_text: [255, 120, 120, 255],
            track: [90, 90, 90, 255],
            outline: true,
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
    // alpha premultiplicado: o overlay é composto com PREMULTIPLIED_ALPHA_BLENDING na GPU
    dst[3] = (dst[3] as u32 * (255 - a) / 255 + a) as u8;
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

/// Fundo (tela inicial / pausa), já premultiplicado pelo alpha.
pub fn clear(fb: &mut [u8], color: [u8; 4]) {
    let a = color[3] as u32;
    let pm = [
        (color[0] as u32 * a / 255) as u8,
        (color[1] as u32 * a / 255) as u8,
        (color[2] as u32 * a / 255) as u8,
        color[3],
    ];
    for px in fb.chunks_exact_mut(4) {
        px.copy_from_slice(&pm);
    }
}

/// Retângulo com cantos arredondados (raio em px), com alpha.
pub fn fill_round_rect(fb: &mut [u8], w: u32, h: u32, r: &Rect, radius: f32, color: [u8; 4]) {
    let rad = radius.min(r.w / 2.0).min(r.h / 2.0).max(0.0);
    let (x0, y0, x1, y1) = (r.x, r.y, r.x + r.w, r.y + r.h);
    let py0 = y0.max(0.0) as i32;
    let py1 = (y1.ceil() as i32).min(h as i32);
    for py in py0..py1 {
        let cy = py as f32 + 0.5;
        // largura da linha nesta altura (cantos circulares)
        let inset = if cy < y0 + rad {
            let d = y0 + rad - cy;
            rad - (rad * rad - d * d).max(0.0).sqrt()
        } else if cy > y1 - rad {
            let d = cy - (y1 - rad);
            rad - (rad * rad - d * d).max(0.0).sqrt()
        } else {
            0.0
        };
        let sx = (x0 + inset).max(0.0) as i32;
        let ex = ((x1 - inset).ceil() as i32).min(w as i32);
        for px in sx..ex {
            let idx = ((py as u32 * w + px as u32) * 4) as usize;
            if color[3] == 255 {
                fb[idx..idx + 4].copy_from_slice(&color);
            } else {
                blend(&mut fb[idx..idx + 4], color, 255);
            }
        }
    }
}

/// Contorno arredondado (2 px) por diferença de dois retângulos.
pub fn stroke_round_rect(fb: &mut [u8], w: u32, h: u32, r: &Rect, radius: f32, color: [u8; 4], t: f32) {
    // desenha o anel em 4 fatias finas: simples e suficiente para bordas de 2 px
    let outer = *r;
    fill_round_rect(fb, w, h, &Rect { x: outer.x, y: outer.y, w: outer.w, h: t }, 0.0, color);
    fill_round_rect(fb, w, h, &Rect { x: outer.x, y: outer.y + outer.h - t, w: outer.w, h: t }, 0.0, color);
    fill_round_rect(
        fb,
        w,
        h,
        &Rect { x: outer.x, y: outer.y + radius, w: t, h: outer.h - radius * 2.0 },
        0.0,
        color,
    );
    fill_round_rect(
        fb,
        w,
        h,
        &Rect { x: outer.x + outer.w - t, y: outer.y + radius, w: t, h: outer.h - radius * 2.0 },
        0.0,
        color,
    );
}

impl Ui {
    /// Um item de menu, conforme o tipo: botão, destaque, perigo, slider, toggle, slot, título.
    pub fn draw_item(
        &mut self,
        fb: &mut [u8],
        w: u32,
        h: u32,
        it: &Item,
        layout: &Layout,
        hot: bool,
        theme: &Theme,
    ) {
        use rnfe_frontend::menu::ItemKind;
        let r = it.rect;
        let rad = layout.radius;
        let font = layout.font;
        let pad = rad;
        if matches!(it.kind, ItemKind::Header) {
            self.draw_text(
                fb,
                w,
                h,
                &it.label,
                font * 0.8,
                r.x as i32 + pad as i32,
                (r.y + r.h * 0.5 - font * 0.4) as i32,
                theme.dim,
            );
            return;
        }
        // sombra suave
        let shadow = Rect { x: r.x, y: r.y + 3.0 * layout.ui_scale, w: r.w, h: r.h };
        fill_round_rect(fb, w, h, &shadow, rad, [0, 0, 0, 90]);
        let (fill, text_color) = match &it.kind {
            ItemKind::Primary => (if hot { theme.accent_hot } else { theme.accent }, theme.on_accent),
            ItemKind::Danger => (
                if hot || it.active { theme.danger } else { theme.button },
                if hot || it.active { [255, 255, 255, 255] } else { theme.danger_text },
            ),
            _ => (if hot || it.active { theme.button_hot } else { theme.button }, theme.text),
        };
        fill_round_rect(fb, w, h, &r, rad, fill);
        if theme.outline {
            stroke_round_rect(fb, w, h, &r, rad, theme.border, 2.0 * layout.ui_scale.max(1.0));
        }
        let ty = |size: f32, cy: f32| (cy - size * 0.55) as i32;
        match &it.kind {
            ItemKind::Slider { fraction } => {
                // rótulo em cima à esquerda, valor à direita, trilha embaixo
                let cy = r.y + r.h * 0.30;
                let vw = self.text_width(&it.value, font * 0.9);
                self.draw_text_clipped(
                    fb,
                    w,
                    h,
                    &it.label,
                    font * 0.9,
                    (r.x + pad) as i32,
                    ty(font * 0.9, cy),
                    (r.w - pad * 3.0 - vw as f32) as i32,
                    text_color,
                );
                self.draw_text(
                    fb,
                    w,
                    h,
                    &it.value,
                    font * 0.9,
                    (r.x + r.w - pad - vw as f32) as i32,
                    ty(font * 0.9, cy),
                    theme.accent,
                );
                let t = rnfe_frontend::menu::slider_track(&r, layout);
                fill_round_rect(fb, w, h, &t, t.h / 2.0, theme.track);
                let filled = Rect { x: t.x, y: t.y, w: t.w * fraction, h: t.h };
                fill_round_rect(fb, w, h, &filled, t.h / 2.0, theme.accent);
                let kr = t.h * 1.1;
                let knob = Circle { cx: t.x + t.w * fraction, cy: t.y + t.h / 2.0, r: kr };
                fill_circle(fb, w, h, &Circle { cx: knob.cx, cy: knob.cy + 2.0, r: kr }, [0, 0, 0, 80]);
                fill_circle(fb, w, h, &knob, theme.on_accent);
            }
            ItemKind::Toggle { on } => {
                let cy = r.y + r.h / 2.0;
                // pílula do toggle à direita
                let pw = r.h * 0.9;
                let ph = r.h * 0.46;
                let px = r.x + r.w - pad - pw;
                let pill = Rect { x: px, y: cy - ph / 2.0, w: pw, h: ph };
                fill_round_rect(fb, w, h, &pill, ph / 2.0, if *on { theme.accent } else { theme.track });
                let kx = if *on { px + pw - ph / 2.0 } else { px + ph / 2.0 };
                fill_circle(fb, w, h, &Circle { cx: kx, cy, r: ph * 0.42 }, theme.on_accent);
                self.draw_text_clipped(
                    fb,
                    w,
                    h,
                    &it.label,
                    font,
                    (r.x + pad) as i32,
                    ty(font, cy),
                    (r.w - pad * 3.0 - pw) as i32,
                    text_color,
                );
            }
            ItemKind::Slot { filled } => {
                let cy = r.y + r.h / 2.0;
                let dot = Circle { cx: r.x + pad + font * 0.5, cy, r: font * 0.28 };
                fill_circle(fb, w, h, &dot, if *filled { theme.accent } else { theme.track });
                self.draw_text_clipped(
                    fb,
                    w,
                    h,
                    &it.label,
                    font,
                    (r.x + pad + font * 1.3) as i32,
                    ty(font, cy),
                    (r.w - pad * 2.0 - font * 1.3) as i32,
                    if *filled { text_color } else { theme.dim },
                );
            }
            _ => {
                let cy = r.y + r.h / 2.0;
                if it.value.is_empty() {
                    let tw = self.text_width(&it.label, font);
                    let tx =
                        if tw as f32 > r.w - pad * 2.0 { r.x + pad } else { r.x + (r.w - tw as f32) / 2.0 };
                    self.draw_text_clipped(
                        fb,
                        w,
                        h,
                        &it.label,
                        font,
                        tx as i32,
                        ty(font, cy),
                        (r.w - pad * 2.0) as i32,
                        text_color,
                    );
                } else {
                    let vw = self.text_width(&it.value, font);
                    self.draw_text_clipped(
                        fb,
                        w,
                        h,
                        &it.label,
                        font,
                        (r.x + pad) as i32,
                        ty(font, cy),
                        (r.w - pad * 3.0 - vw as f32) as i32,
                        text_color,
                    );
                    self.draw_text(
                        fb,
                        w,
                        h,
                        &it.value,
                        font,
                        (r.x + r.w - pad - vw as f32) as i32,
                        ty(font, cy),
                        theme.accent,
                    );
                }
            }
        }
    }
}
