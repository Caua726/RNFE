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
        let portrait = h > w;
        let unit = if portrait { w } else { h }; // lado menor
        let scale = scale.clamp(0.5, 2.0);
        let dpad_r = unit * 0.17 * scale;
        let ab_r = unit * 0.085 * scale;
        let pill_w = unit * 0.16 * scale;
        let pill_h = unit * 0.06 * scale;
        let m = unit * 0.05; // margem
        if portrait {
            // Zona de controle: abaixo da imagem (que ocupa w × w×240/256 no topo)
            let img_h = w * 240.0 / 256.0;
            let zone_top = img_h + m;
            let zone_h = (h - zone_top).max(dpad_r * 2.5);
            let cy = zone_top + zone_h * 0.45;
            let dpad = Circle { cx: m + dpad_r, cy, r: dpad_r };
            let a = Circle { cx: w - m - ab_r, cy: cy - ab_r * 0.6, r: ab_r };
            let b = Circle { cx: w - m - ab_r * 3.4, cy: cy + ab_r * 0.6, r: ab_r };
            let py = h - m - pill_h;
            let select = Rect { x: w * 0.5 - pill_w - m * 0.5, y: py, w: pill_w, h: pill_h };
            let start = Rect { x: w * 0.5 + m * 0.5, y: py, w: pill_w, h: pill_h };
            let menu = Rect { x: w - m - pill_w, y: m * 0.5, w: pill_w, h: pill_h };
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
        } else {
            let cy = h * 0.62;
            let dpad = Circle { cx: m + dpad_r, cy, r: dpad_r };
            let a = Circle { cx: w - m - ab_r, cy: cy - ab_r * 0.6, r: ab_r };
            let b = Circle { cx: w - m - ab_r * 3.4, cy: cy + ab_r * 0.6, r: ab_r };
            let py = h - m - pill_h;
            let select = Rect { x: w * 0.5 - pill_w - m * 0.5, y: py, w: pill_w, h: pill_h };
            let start = Rect { x: w * 0.5 + m * 0.5, y: py, w: pill_w, h: pill_h };
            let menu = Rect { x: w - m - pill_w, y: m * 0.5, w: pill_w, h: pill_h };
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
        assert!(l.start.y + l.start.h <= 2340.0);
        assert!(l.menu.y >= 0.0);
    }

    #[test]
    fn landscape_layout_puts_controls_on_sides() {
        let l = TouchLayout::for_size(2340.0, 1080.0);
        assert!(!l.portrait);
        assert!(l.dpad.cx < 2340.0 * 0.3);
        assert!(l.a.cx > 2340.0 * 0.7);
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
