//! Controles de toque: layout por tamanho de tela e hit-test multi-toque.
//!
//! Coordenadas em pixels físicos da janela. O d-pad é um círculo dividido por ângulo com zona
//! morta no centro; A/B são círculos; Start/Select/Menu são pílulas. Retrato põe os
//! controles abaixo da imagem; paisagem põe nas laterais.

use rnfe_core::Buttons;

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Circle {
    pub cx: f32,
    pub cy: f32,
    pub r: f32,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Rect {
    pub x: f32,
    pub y: f32,
    pub w: f32,
    pub h: f32,
}

impl Rect {
    pub fn contains(&self, x: f32, y: f32) -> bool {
        x >= self.x && x < self.x + self.w && y >= self.y && y < self.y + self.h
    }
}

/// Um botão especial do overlay (não é do NES).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Special {
    Menu,
}

#[derive(Debug, Clone, PartialEq)]
pub struct TouchLayout {
    pub width: f32,
    pub height: f32,
    pub portrait: bool,
    pub dpad: Circle,
    /// Raio da zona morta do d-pad.
    pub dpad_dead: f32,
    pub a: Circle,
    pub b: Circle,
    pub start: Rect,
    pub select: Rect,
    pub menu: Rect,
}

impl TouchLayout {
    /// Layout para uma janela `w`×`h` (pixels físicos), com os controles no tamanho padrão.
    pub fn for_size(w: f32, h: f32) -> TouchLayout {
        Self::for_size_scaled(w, h, 1.0)
    }

    /// Como `for_size`, com os botões multiplicados por `scale` (0,6–1,6).
    pub fn for_size_scaled(w: f32, h: f32, scale: f32) -> TouchLayout {
        Self::for_viewport(w, h, w * 240.0 / 256.0, scale)
    }

    /// Layout sabendo onde a imagem do NES termina em retrato (`img_bottom`, px): os controles
    /// começam logo abaixo dela. Em paisagem `img_bottom` é ignorado.
    pub fn for_viewport(w: f32, h: f32, img_bottom: f32, scale: f32) -> TouchLayout {
        let portrait = h > w;
        let unit = if portrait { w } else { h }; // lado menor
        let scale = scale.clamp(0.5, 2.0);
        let dpad_r = unit * 0.17 * scale;
        let ab_r = unit * 0.085 * scale;
        let pill_w = unit * 0.16 * scale;
        let pill_h = unit * 0.06 * scale;
        let m = unit * 0.05; // margem
        // Faixa de gesto do sistema na borda inferior (Android): nada de botão ali
        let inset = (unit * 0.06).max(24.0);
        let (dpad, a, b, start, select, menu);
        if portrait {
            // Zona de controle: abaixo da imagem. MENU no topo da zona (fora do HUD do jogo),
            // START/SELECT logo acima da faixa de gesto, d-pad e A/B entre os dois.
            let img_h = img_bottom.clamp(0.0, h);
            let zone_top = img_h + m * 0.6;
            menu = Rect { x: w * 0.5 - pill_w * 0.5, y: zone_top, w: pill_w, h: pill_h };
            let py = (h - inset - pill_h).max(zone_top + pill_h * 2.0);
            select = Rect { x: w * 0.5 - pill_w - m * 0.5, y: py, w: pill_w, h: pill_h };
            start = Rect { x: w * 0.5 + m * 0.5, y: py, w: pill_w, h: pill_h };
            let top = menu.y + menu.h + m;
            let bottom = py - m;
            let cy = (top + (bottom - top) * 0.55).clamp(top + dpad_r, (bottom - dpad_r).max(top + dpad_r));
            dpad = Circle { cx: m + dpad_r, cy, r: dpad_r };
            a = Circle { cx: w - m - ab_r, cy: cy - ab_r * 0.6, r: ab_r };
            b = Circle { cx: w - m - ab_r * 3.4, cy: cy + ab_r * 0.6, r: ab_r };
        } else {
            // Paisagem: imagem no centro, controles nas calhas laterais; START/SELECT abaixo do
            // d-pad e de A/B (nunca sobre a imagem), MENU no canto superior esquerdo.
            let cy = h * 0.52;
            dpad = Circle { cx: m + dpad_r, cy, r: dpad_r };
            a = Circle { cx: w - m - ab_r, cy: cy - ab_r * 0.6, r: ab_r };
            b = Circle { cx: w - m - ab_r * 3.4, cy: cy + ab_r * 0.6, r: ab_r };
            let py = (cy + dpad_r + m * 0.6).min(h - inset - pill_h);
            select = Rect { x: dpad.cx - pill_w * 0.5, y: py, w: pill_w, h: pill_h };
            start = Rect { x: (a.cx + b.cx) * 0.5 - pill_w * 0.5, y: py, w: pill_w, h: pill_h };
            menu = Rect { x: m, y: m * 0.5, w: pill_w, h: pill_h };
        }
        TouchLayout {
            width: w,
            height: h,
            portrait,
            dpad,
            dpad_dead: dpad_r * 0.25,
            a,
            b,
            start,
            select,
            menu,
        }
    }

    /// Botões do NES sob o ponto (d-pad com 8 direções por ângulo; fora do círculo o d-pad
    /// ainda responde até 1,6× o raio, para o dedo poder deslizar).
    pub fn hit(&self, x: f32, y: f32) -> Buttons {
        let mut b = Buttons::NONE;
        let dx = x - self.dpad.cx;
        let dy = y - self.dpad.cy;
        let d2 = dx * dx + dy * dy;
        let reach = self.dpad.r * 1.6;
        if d2 <= reach * reach && d2 >= self.dpad_dead * self.dpad_dead {
            // ângulo em graus, 0 = direita, 90 = cima (y cresce para baixo)
            let ang = (-dy).atan2(dx).to_degrees();
            let ang = if ang < 0.0 { ang + 360.0 } else { ang };
            // setores de 45° centrados nas 8 direções; diagonais entre 22,5° e 67,5°
            if !(112.5..=247.5).contains(&ang) && (ang <= 67.5 || ang >= 292.5) {
                b |= Buttons::RIGHT;
            }
            if (112.5..=247.5).contains(&ang) {
                b |= Buttons::LEFT;
            }
            if (22.5..=157.5).contains(&ang) {
                b |= Buttons::UP;
            }
            if (202.5..=337.5).contains(&ang) {
                b |= Buttons::DOWN;
            }
        }
        if inside(&self.a, x, y, 1.3) {
            b |= Buttons::A;
        }
        if inside(&self.b, x, y, 1.3) {
            b |= Buttons::B;
        }
        if self.start.contains(x, y) {
            b |= Buttons::START;
        }
        if self.select.contains(x, y) {
            b |= Buttons::SELECT;
        }
        b
    }

    pub fn special(&self, x: f32, y: f32) -> Option<Special> {
        self.menu.contains(x, y).then_some(Special::Menu)
    }

    /// Retângulos onde o sistema não deve capturar gestos de borda (Android): d-pad e A/B.
    pub fn gesture_exclusion(&self) -> [Rect; 2] {
        let d = &self.dpad;
        let reach = d.r * 1.3;
        let ab_x0 = (self.b.cx - self.b.r * 1.3).min(self.a.cx - self.a.r * 1.3);
        let ab_x1 = (self.a.cx + self.a.r * 1.3).max(self.b.cx + self.b.r * 1.3);
        let ab_y0 = (self.a.cy - self.a.r * 1.3).min(self.b.cy - self.b.r * 1.3);
        let ab_y1 = (self.a.cy + self.a.r * 1.3).max(self.b.cy + self.b.r * 1.3);
        [
            Rect { x: (d.cx - reach).max(0.0), y: d.cy - reach, w: reach * 2.0, h: reach * 2.0 },
            Rect { x: ab_x0, y: ab_y0, w: ab_x1 - ab_x0, h: ab_y1 - ab_y0 },
        ]
    }
}

fn inside(c: &Circle, x: f32, y: f32, slack: f32) -> bool {
    let dx = x - c.cx;
    let dy = y - c.cy;
    dx * dx + dy * dy <= (c.r * slack) * (c.r * slack)
}

/// Toques ativos → botões (união de todos os dedos).
#[derive(Debug, Default, Clone)]
pub struct TouchState {
    touches: Vec<(u64, Buttons)>,
    /// Já houve algum toque: mostrar o overlay.
    pub seen: bool,
}

impl TouchState {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn down(&mut self, layout: &TouchLayout, id: u64, x: f32, y: f32) -> Buttons {
        self.seen = true;
        let b = layout.hit(x, y);
        self.touches.retain(|(i, _)| *i != id);
        self.touches.push((id, b));
        b
    }

    pub fn moved(&mut self, layout: &TouchLayout, id: u64, x: f32, y: f32) {
        let b = layout.hit(x, y);
        if let Some(t) = self.touches.iter_mut().find(|(i, _)| *i == id) {
            t.1 = b;
        }
    }

    pub fn up(&mut self, id: u64) {
        self.touches.retain(|(i, _)| *i != id);
    }

    pub fn clear(&mut self) {
        self.touches.clear();
    }

    pub fn buttons(&self) -> Buttons {
        self.touches.iter().fold(Buttons::NONE, |acc, (_, b)| acc | *b)
    }

    pub fn active(&self) -> usize {
        self.touches.len()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn portrait_layout_fits_screen() {
        let l = TouchLayout::for_size(1080.0, 2340.0);
        assert!(l.portrait);
        assert!(l.dpad.cy - l.dpad.r > 1080.0 * 240.0 / 256.0, "d-pad abaixo da imagem");
        assert!(l.a.cx + l.a.r <= 1080.0 && l.b.cx - l.b.r >= 0.0);
        assert!(l.start.y + l.start.h <= 2340.0 - 24.0, "START acima da faixa de gesto");
        assert!(l.menu.y > 1080.0 * 240.0 / 256.0, "MENU fora da imagem em retrato");
        assert!(l.dpad.cy - l.dpad.r > l.menu.y + l.menu.h, "d-pad abaixo do MENU");
        assert!(l.dpad.cy + l.dpad.r < l.start.y, "d-pad acima de START/SELECT");
        let ex = l.gesture_exclusion();
        assert!(ex[0].x <= 1.0 && ex[0].contains(l.dpad.cx, l.dpad.cy));
        assert!(ex[1].contains(l.a.cx, l.a.cy) && ex[1].contains(l.b.cx, l.b.cy));
    }

    #[test]
    fn viewport_moves_controls_under_the_image() {
        let a = TouchLayout::for_viewport(1080.0, 2340.0, 886.0, 1.0);
        let b = TouchLayout::for_viewport(1080.0, 2340.0, 1012.0, 1.0);
        assert!(a.dpad.cy < b.dpad.cy, "imagem mais baixa (8:7) → d-pad mais alto");
        assert!(a.dpad.cy - a.dpad.r >= 886.0, "não invade a imagem");
        let big = TouchLayout::for_viewport(1080.0, 2340.0, 886.0, 1.5);
        assert!(big.dpad.r > a.dpad.r * 1.4);
    }

    #[test]
    fn landscape_layout_puts_controls_on_sides() {
        let l = TouchLayout::for_size(2340.0, 1080.0);
        assert!(!l.portrait);
        assert!(l.dpad.cx < 2340.0 * 0.3);
        assert!(l.a.cx > 2340.0 * 0.7);
        // imagem 8:7 centralizada: x 512..1828; START/SELECT/d-pad/A/B ficam nas calhas
        let (img_x0, img_x1) = (512.0, 1828.0);
        assert!(l.select.x + l.select.w < img_x0, "SELECT na calha esquerda");
        assert!(l.start.x > img_x1, "START na calha direita");
        assert!(l.dpad.cx + l.dpad.r < img_x0 && l.b.cx - l.b.r > img_x1);
        assert!(l.start.y + l.start.h <= 1080.0 - 24.0);
        assert!(l.menu.x + l.menu.w < img_x0);
    }

    #[test]
    fn dpad_angles_and_dead_zone() {
        let l = TouchLayout::for_size(2340.0, 1080.0);
        let (cx, cy, r) = (l.dpad.cx, l.dpad.cy, l.dpad.r);
        assert_eq!(l.hit(cx, cy), Buttons::NONE, "zona morta");
        assert_eq!(l.hit(cx + r * 0.8, cy), Buttons::RIGHT);
        assert_eq!(l.hit(cx - r * 0.8, cy), Buttons::LEFT);
        assert_eq!(l.hit(cx, cy - r * 0.8), Buttons::UP);
        assert_eq!(l.hit(cx, cy + r * 0.8), Buttons::DOWN);
        assert_eq!(l.hit(cx + r * 0.6, cy - r * 0.6), Buttons::UP | Buttons::RIGHT);
        assert_eq!(l.hit(cx - r * 0.6, cy + r * 0.6), Buttons::DOWN | Buttons::LEFT);
        assert_eq!(l.hit(cx + r * 1.5, cy), Buttons::RIGHT, "deslizou para fora, ainda vale");
        assert_eq!(l.hit(cx + r * 3.0, cy), Buttons::NONE);
    }

    #[test]
    fn buttons_and_multitouch() {
        let l = TouchLayout::for_size(1080.0, 2340.0);
        assert_eq!(l.hit(l.a.cx, l.a.cy), Buttons::A);
        assert_eq!(l.hit(l.b.cx, l.b.cy), Buttons::B);
        assert_eq!(l.hit(l.start.x + 1.0, l.start.y + 1.0), Buttons::START);
        assert_eq!(l.hit(l.select.x + 1.0, l.select.y + 1.0), Buttons::SELECT);
        assert_eq!(l.special(l.menu.x + 1.0, l.menu.y + 1.0), Some(Special::Menu));
        assert_eq!(l.hit(l.width * 0.5, 10.0), Buttons::NONE, "imagem não é botão");

        let mut t = TouchState::new();
        assert!(!t.seen);
        t.down(&l, 1, l.dpad.cx + l.dpad.r * 0.8, l.dpad.cy);
        t.down(&l, 2, l.a.cx, l.a.cy);
        assert_eq!(t.buttons(), Buttons::RIGHT | Buttons::A);
        t.moved(&l, 1, l.dpad.cx, l.dpad.cy - l.dpad.r * 0.8);
        assert_eq!(t.buttons(), Buttons::UP | Buttons::A);
        t.up(2);
        assert_eq!(t.buttons(), Buttons::UP);
        t.up(1);
        assert_eq!(t.buttons(), Buttons::NONE);
        assert!(t.seen);
    }
}
