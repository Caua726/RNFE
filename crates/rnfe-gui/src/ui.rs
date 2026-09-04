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
    /// Bolinha do slider/toggle (precisa contrastar com a trilha e com o preenchimento).
    pub knob: [u8; 4],
    /// Contorno em todo item (alto contraste).
    pub outline: bool,
    pub touch: [u8; 4],
    pub touch_hot: [u8; 4],
}

impl Theme {
    pub fn normal() -> Theme {
        Theme {
            bg: [12, 12, 16, 200],
            panel: [18, 18, 26, 255],
            text: [225, 225, 230, 255],
            // contrastes conferidos sobre `button`/`accent` (WCAG AA, texto de 16 px)
            dim: [160, 160, 172, 255],
            button: [40, 40, 56, 255],
            button_hot: [72, 72, 100, 255],
            border: [70, 70, 90, 255],
            accent: [200, 56, 46, 255],
            accent_hot: [255, 120, 108, 255],
            on_accent: [255, 250, 245, 255],
            danger: [160, 40, 40, 255],
            danger_text: [255, 150, 140, 255],
            track: [60, 60, 76, 255],
            knob: [255, 250, 245, 255],
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
            danger: [200, 0, 0, 255],
            danger_text: [255, 120, 120, 255],
            track: [150, 150, 150, 255],
            knob: [255, 255, 255, 255],
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
    /// Altura da caixa alta ("H") em frações do tamanho da fonte: centraliza texto na vertical.
    cap: f32,
}

impl Default for Ui {
    fn default() -> Self {
        Self::new()
    }
}

impl Ui {
    pub fn new() -> Self {
        let font = Font::from_bytes(FONT_DATA, fontdue::FontSettings::default()).expect("fonte embutida");
        let cap = font.metrics('H', 100.0).height as f32 / 100.0;
        Ui { font, cache: HashMap::new(), cap }
    }

    /// `y` para `draw_text` de forma que a caixa alta fique centrada em `cy`.
    /// (a linha de base fica em `y + size`, então o topo do "H" é `y + size - cap`.)
    pub fn center_y(&self, size: f32, cy: f32) -> i32 {
        (cy - size + size * self.cap * 0.5) as i32
    }

    fn glyph(&mut self, ch: char, size: f32) -> &Glyph {
        let key = (ch, (size * 4.0) as u32);
        self.cache.entry(key).or_insert_with(|| {
            let (metrics, bitmap) = self.font.rasterize(ch, size);
            Glyph { metrics, bitmap }
        })
    }

    pub fn text_width(&mut self, text: &str, size: f32) -> i32 {
        let w: f32 = text.chars().map(|c| self.glyph(c, size).metrics.advance_width).sum();
        w.round() as i32
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
        self.draw_text_spaced(fb, w, h, text, size, x, y, color, 0.0);
    }

    /// `draw_text` com espaçamento extra entre letras (em px); devolve a largura desenhada.
    fn draw_text_spaced(
        &mut self,
        fb: &mut [u8],
        w: u32,
        h: u32,
        text: &str,
        size: f32,
        x: i32,
        y: i32,
        color: [u8; 4],
        tracking: f32,
    ) -> i32 {
        let mut cursor = x as f32;
        for ch in text.chars() {
            let g = self.glyph(ch, size);
            let m = g.metrics;
            let gx = cursor.round() as i32 + m.xmin;
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
            cursor += m.advance_width + tracking;
        }
        (cursor - x as f32).round() as i32
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

    /// Caixa de mensagem temporária: no rodapé, ou em `y` (px) se dado. Quebra em até 3 linhas.
    pub fn draw_toast(&mut self, fb: &mut [u8], w: u32, h: u32, msg: &str, size: f32, y: Option<f32>) {
        let max_w = w as f32 * 0.9 - size * 1.4;
        let mut lines: Vec<String> = Vec::new();
        let mut cur = String::new();
        for word in msg.split_whitespace() {
            let trial = if cur.is_empty() { word.to_string() } else { format!("{cur} {word}") };
            if self.text_width(&trial, size) as f32 <= max_w || cur.is_empty() {
                cur = trial;
            } else {
                lines.push(std::mem::take(&mut cur));
                cur = word.to_string();
                if lines.len() == 2 {
                    break;
                }
            }
        }
        if !cur.is_empty() && lines.len() < 3 {
            lines.push(cur);
        }
        if lines.is_empty() {
            return;
        }
        let pad = (size * 0.7) as i32;
        let lh = size * 1.3;
        let tw = lines.iter().map(|l| self.text_width(l, size)).max().unwrap_or(0).min(max_w as i32);
        let box_h = lh * lines.len() as f32 + pad as f32;
        let ty = y.map(|y| y as i32).unwrap_or(h as i32 - (size * 3.0) as i32 - (box_h - lh) as i32);
        let r = Rect {
            x: ((w as i32 - tw) / 2 - pad) as f32,
            y: (ty - pad / 2) as f32,
            w: (tw + pad * 2) as f32,
            h: box_h,
        };
        fill_round_rect(fb, w, h, &r, size * 0.4, [0, 0, 0, 210]);
        for (i, line) in lines.iter().enumerate() {
            let lw = self.text_width(line, size).min(tw);
            let lx = (w as i32 - lw) / 2;
            let ly = ty + (i as f32 * lh) as i32;
            self.draw_text_clipped(fb, w, h, line, size, lx, ly, tw, [255, 255, 255, 255]);
        }
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
        // Em retrato os controles ficam abaixo da imagem: translucidez só apagaria os botões.
        let opacity = if layout.portrait { 1.0 } else { opacity.clamp(0.1, 1.0) };
        let a = (opacity * 255.0) as u8;
        let base = [theme.touch[0], theme.touch[1], theme.touch[2], a];
        let hot = [theme.touch_hot[0], theme.touch_hot[1], theme.touch_hot[2], a.saturating_add(90)];
        let col = |b: Buttons| if pressed.0 & b.0 != 0 { hot } else { base };
        let d = &layout.dpad;
        let arm = d.r * 0.36;
        // disco tênue mostrando a área que responde ao d-pad
        fill_circle(
            fb,
            w,
            h,
            &Circle { cx: d.cx, cy: d.cy, r: d.r * 1.15 },
            [base[0], base[1], base[2], a / 4],
        );
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
        fill_round_rect(fb, w, h, &layout.start, layout.start.h / 2.0, col(Buttons::START));
        fill_round_rect(fb, w, h, &layout.select, layout.select.h / 2.0, col(Buttons::SELECT));
        fill_round_rect(fb, w, h, &layout.menu, layout.menu.h / 2.0, base);
        // rótulos escuros (o disco é claro), legíveis sobre céu azul ou fundo preto
        let label_color = [0, 0, 0, 230];
        let label = |ui: &mut Ui, fb: &mut [u8], text: &str, cx: f32, cy: f32, size: f32| {
            let tw = ui.text_width(text, size);
            let ty = ui.center_y(size, cy);
            ui.draw_text(fb, w, h, text, size, cx as i32 - tw / 2, ty, label_color);
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

fn fill_rect_f(fb: &mut [u8], w: u32, h: u32, x: f32, y: f32, rw: f32, rh: f32, color: [u8; 4]) {
    fill_rect(fb, w, h, x as i32, y as i32, rw as i32, rh as i32, color);
}

/// Círculo com 1 px de suavização na borda (cobertura pela distância ao raio).
fn fill_circle(fb: &mut [u8], w: u32, h: u32, c: &Circle, color: [u8; 4]) {
    let y0 = (c.cy - c.r - 1.0).max(0.0) as i32;
    let y1 = ((c.cy + c.r + 1.0) as i32 + 1).min(h as i32);
    let x0 = (c.cx - c.r - 1.0).max(0.0) as i32;
    let x1 = ((c.cx + c.r + 1.0) as i32 + 1).min(w as i32);
    for py in y0..y1 {
        let dy = py as f32 + 0.5 - c.cy;
        for px in x0..x1 {
            let dx = px as f32 + 0.5 - c.cx;
            let cov = (c.r - (dx * dx + dy * dy).sqrt() + 0.5).clamp(0.0, 1.0);
            if cov > 0.0 {
                let idx = ((py as u32 * w + px as u32) * 4) as usize;
                blend(&mut fb[idx..idx + 4], color, (cov * 255.0) as u8);
            }
        }
    }
}

/// Desenha uma imagem RGBA (`src`, `sw`×`sh`) esticada dentro de `dst`, com borda escura.
/// Vizinho mais próximo: são miniaturas de 64×60, ampliar suave só borraria.
pub fn draw_image(fb: &mut [u8], w: u32, h: u32, dst: &Rect, src: &[u8], sw: u32, sh: u32) {
    if src.len() < (sw * sh * 4) as usize || dst.w < 1.0 || dst.h < 1.0 {
        return;
    }
    let border = Rect { x: dst.x - 1.0, y: dst.y - 1.0, w: dst.w + 2.0, h: dst.h + 2.0 };
    fill_round_rect(fb, w, h, &border, 2.0, [0, 0, 0, 220]);
    let (x0, y0) = (dst.x.max(0.0) as i32, dst.y.max(0.0) as i32);
    let (x1, y1) = (((dst.x + dst.w) as i32).min(w as i32), ((dst.y + dst.h) as i32).min(h as i32));
    for py in y0..y1 {
        let v = ((py as f32 - dst.y) / dst.h * sh as f32) as u32;
        let v = v.min(sh - 1);
        for px in x0..x1 {
            let u = ((px as f32 - dst.x) / dst.w * sw as f32) as u32;
            let u = u.min(sw - 1);
            let si = ((v * sw + u) * 4) as usize;
            let di = ((py as u32 * w + px as u32) * 4) as usize;
            let c = [src[si], src[si + 1], src[si + 2], 255];
            fb[di..di + 4].copy_from_slice(&c);
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
        let (left, right) = (x0 + inset, x1 - inset);
        let sx = left.floor().max(0.0) as i32;
        let ex = (right.ceil() as i32).min(w as i32);
        for px in sx..ex {
            let cx = px as f32;
            // cobertura horizontal: as pontas da linha entram com alpha proporcional
            let cov = ((cx + 1.0).min(right) - cx.max(left)).clamp(0.0, 1.0);
            if cov <= 0.0 {
                continue;
            }
            let idx = ((py as u32 * w + px as u32) * 4) as usize;
            if color[3] == 255 && cov >= 1.0 {
                fb[idx..idx + 4].copy_from_slice(&color);
            } else {
                blend(&mut fb[idx..idx + 4], color, (cov * 255.0) as u8);
            }
        }
    }
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
            let size = font * 0.78;
            let label: String = it.label.to_uppercase();
            let cy = r.y + r.h * 0.5;
            let tx = r.x as i32 + pad as i32;
            let ty = self.center_y(size, cy);
            let tw = self.draw_text_spaced(fb, w, h, &label, size, tx, ty, theme.text, size * 0.08);
            // filete até a borda direita, separando a seção do que vem acima
            let line_x = tx + tw + (pad * 0.6) as i32;
            let line_w = (r.x + r.w) as i32 - line_x;
            if line_w > 0 {
                fill_rect(fb, w, h, line_x, cy as i32, line_w, 1.max((r.h * 0.03) as i32), theme.border);
            }
            return;
        }
        // itens sem ação (rótulo "Abrindo…", slot vazio ao carregar) não parecem clicáveis
        let inert = it.action == rnfe_frontend::menu::Action::None;
        if !inert {
            // sombra suave
            let shadow = Rect { x: r.x, y: r.y + 3.0 * layout.ui_scale, w: r.w, h: r.h };
            fill_round_rect(fb, w, h, &shadow, rad, [0, 0, 0, 90]);
        }
        let (fill, text_color) = match &it.kind {
            ItemKind::Primary => (if hot { theme.accent_hot } else { theme.accent }, theme.on_accent),
            ItemKind::Danger => (
                if hot || it.active { theme.danger } else { theme.button },
                if hot || it.active { [255, 255, 255, 255] } else { theme.danger_text },
            ),
            // "ativo" (turbo ligado) não muda o fundo: a pílula do toggle já mostra o estado
            _ => (if hot { theme.button_hot } else { theme.button }, theme.text),
        };
        let (fill, text_color) = if inert {
            (theme.panel, theme.dim)
        } else if theme.outline
            && hot
            && matches!(
                it.kind,
                ItemKind::Button | ItemKind::Slider { .. } | ItemKind::Toggle { .. } | ItemKind::Slot { .. }
            )
        {
            // alto contraste: fundo amarelo com texto branco é ilegível — o estado quente é o anel
            (theme.button, theme.text)
        } else {
            (fill, text_color)
        };
        if theme.outline || inert {
            // anel: retângulo externo na cor da borda, interno encolhido na cor do fundo
            let t = if theme.outline && hot { 4.0 } else { 2.0 } * layout.ui_scale.max(1.0);
            let ring = if inert {
                theme.border
            } else if hot {
                theme.accent
            } else {
                theme.border
            };
            fill_round_rect(fb, w, h, &r, rad, ring);
            let inner = Rect { x: r.x + t, y: r.y + t, w: r.w - 2.0 * t, h: r.h - 2.0 * t };
            fill_round_rect(fb, w, h, &inner, (rad - t).max(0.0), fill);
        } else {
            fill_round_rect(fb, w, h, &r, rad, fill);
        }
        let cap = self.cap;
        let ty = move |size: f32, cy: f32| (cy - size + size * cap * 0.5) as i32;
        match &it.kind {
            ItemKind::Slider { fraction } => {
                // rótulo em cima à esquerda, valor à direita, trilha embaixo (mesmo recuo)
                let cy = r.y + r.h * 0.30;
                let t = rnfe_frontend::menu::slider_track(&r, layout);
                let vw = self.text_width(&it.value, font);
                let max_w = (t.w - vw as f32 - pad) as i32;
                self.draw_text_clipped(
                    fb,
                    w,
                    h,
                    &it.label,
                    font,
                    t.x as i32,
                    ty(font, cy),
                    max_w,
                    text_color,
                );
                self.draw_text(
                    fb,
                    w,
                    h,
                    &it.value,
                    font,
                    (t.x + t.w - vw as f32) as i32,
                    ty(font, cy),
                    theme.accent_hot,
                );
                fill_round_rect(fb, w, h, &t, t.h / 2.0, theme.track);
                let filled = Rect { x: t.x, y: t.y, w: t.w * fraction, h: t.h };
                fill_round_rect(fb, w, h, &filled, t.h / 2.0, theme.accent);
                let kr = t.h * 1.1;
                let knob = Circle { cx: t.x + t.w * fraction, cy: t.y + t.h / 2.0, r: kr };
                fill_circle(
                    fb,
                    w,
                    h,
                    &Circle { cx: knob.cx, cy: knob.cy + 2.0 * layout.ui_scale, r: kr },
                    [0, 0, 0, 80],
                );
                fill_circle(fb, w, h, &knob, theme.knob);
                if theme.outline {
                    // alto contraste: anel escuro para o knob não sumir na trilha clara
                    fill_circle(fb, w, h, &Circle { cx: knob.cx, cy: knob.cy, r: kr * 0.62 }, [0, 0, 0, 255]);
                }
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
                fill_circle(fb, w, h, &Circle { cx: kx, cy, r: ph * 0.42 }, theme.knob);
                if theme.outline {
                    fill_circle(fb, w, h, &Circle { cx: kx, cy, r: ph * 0.26 }, [0, 0, 0, 255]);
                }
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
