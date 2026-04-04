use fontdue::Font;

const FONT_DATA: &[u8] = include_bytes!("../assets/NotoSans-Regular.ttf");

pub const MENUBAR_HEIGHT: i32 = 28;
pub const SIDEBAR_WIDTH: i32 = 180;
const MENU_FONT_SIZE: f32 = 14.0;
const MENU_PAD_X: i32 = 12;
const DROPDOWN_ITEM_H: i32 = 26;
const DROPDOWN_PAD_X: i32 = 16;
const SIDEBAR_ITEM_H: i32 = 36;
const SIDEBAR_PAD_X: i32 = 18;
const SIDEBAR_FONT_SIZE: f32 = 14.0;

#[derive(Clone, Copy, PartialEq)]
pub enum MenuAction {
    None,
    OpenRom,
    Reset,
    Quit,
}

pub struct Ui {
    font: Font,
    pub open_menu: Option<usize>, // qual menu tá aberto (0=File, 1=Settings)
}

struct MenuItem {
    label: &'static str,
    items: &'static [(&'static str, MenuAction)],
}

const MENUS: &[MenuItem] = &[
    MenuItem {
        label: "File",
        items: &[
            ("Open ROM", MenuAction::OpenRom),
            ("Reset", MenuAction::Reset),
            ("Quit", MenuAction::Quit),
        ],
    },
];

impl Ui {
    pub fn new() -> Self {
        let font = Font::from_bytes(FONT_DATA, fontdue::FontSettings::default()).unwrap();
        Ui { font, open_menu: None }
    }

    pub fn draw_text_centered(&self, fb: &mut [u8], w: u32, h: u32, text: &str, size: f32, y: i32, color: [u8; 4]) {
        let tw = self.text_width(text, size);
        let x = (w as i32 - tw) / 2;
        self.draw_text(fb, w, h, text, size, x, y, color);
    }

    pub fn draw_text(&self, fb: &mut [u8], w: u32, h: u32, text: &str, size: f32, x: i32, y: i32, color: [u8; 4]) {
        let mut cursor_x = x;
        for ch in text.chars() {
            let (metrics, bitmap) = self.font.rasterize(ch, size);
            let gx = cursor_x + metrics.xmin;
            let gy = y + (size as i32 - metrics.ymin - metrics.height as i32);

            for row in 0..metrics.height {
                for col in 0..metrics.width {
                    let alpha = bitmap[row * metrics.width + col];
                    if alpha == 0 { continue; }
                    let px = gx + col as i32;
                    let py = gy + row as i32;
                    if px < 0 || py < 0 || px >= w as i32 || py >= h as i32 { continue; }
                    let idx = ((py as u32 * w + px as u32) * 4) as usize;
                    let a = alpha as f32 / 255.0;
                    fb[idx]     = (fb[idx] as f32 * (1.0 - a) + color[0] as f32 * a) as u8;
                    fb[idx + 1] = (fb[idx+1] as f32 * (1.0 - a) + color[1] as f32 * a) as u8;
                    fb[idx + 2] = (fb[idx+2] as f32 * (1.0 - a) + color[2] as f32 * a) as u8;
                    fb[idx + 3] = 255;
                }
            }
            cursor_x += metrics.advance_width as i32;
        }
    }

    pub fn text_width(&self, text: &str, size: f32) -> i32 {
        let mut w = 0;
        for ch in text.chars() {
            let (metrics, _) = self.font.rasterize(ch, size);
            w += metrics.advance_width as i32;
        }
        w
    }

    pub fn draw_button(&self, fb: &mut [u8], w: u32, h: u32, text: &str, size: f32, cx: i32, cy: i32, color: [u8; 4], border: [u8; 4]) {
        let tw = self.text_width(text, size);
        let pad_x = 20;
        let pad_y = 10;
        let bw = tw + pad_x * 2;
        let bh = size as i32 + pad_y * 2;
        let bx = cx - bw / 2;
        let by = cy - bh / 2;
        let w_i = w as i32;
        let h_i = h as i32;

        for px in bx..bx + bw {
            for &py in &[by, by + bh - 1] {
                if px >= 0 && py >= 0 && px < w_i && py < h_i {
                    let idx = ((py as u32 * w + px as u32) * 4) as usize;
                    fb[idx..idx + 4].copy_from_slice(&border);
                }
            }
        }
        for py in by..by + bh {
            for &px in &[bx, bx + bw - 1] {
                if px >= 0 && py >= 0 && px < w_i && py < h_i {
                    let idx = ((py as u32 * w + px as u32) * 4) as usize;
                    fb[idx..idx + 4].copy_from_slice(&border);
                }
            }
        }

        let tx = cx - tw / 2;
        let ty = by + pad_y;
        self.draw_text(fb, w, h, text, size, tx, ty, color);
    }

    pub fn button_rect(&self, text: &str, size: f32, cx: i32, cy: i32) -> (i32, i32, i32, i32) {
        let tw = self.text_width(text, size);
        let pad_x = 20;
        let pad_y = 10;
        let bw = tw + pad_x * 2;
        let bh = size as i32 + pad_y * 2;
        (cx - bw / 2, cy - bh / 2, bw, bh)
    }

    pub fn fill_rect_pub(&self, fb: &mut [u8], w: u32, h: u32, rx: i32, ry: i32, rw: i32, rh: i32, color: [u8; 4]) {
        self.fill_rect(fb, w, h, rx, ry, rw, rh, color);
    }

    fn fill_rect(&self, fb: &mut [u8], w: u32, h: u32, rx: i32, ry: i32, rw: i32, rh: i32, color: [u8; 4]) {
        let w_i = w as i32;
        let h_i = h as i32;
        for py in ry.max(0)..((ry + rh).min(h_i)) {
            for px in rx.max(0)..((rx + rw).min(w_i)) {
                let idx = ((py as u32 * w + px as u32) * 4) as usize;
                fb[idx..idx + 4].copy_from_slice(&color);
            }
        }
    }

    // Desenha a barra de menu no topo
    pub fn draw_menubar(&self, fb: &mut [u8], w: u32, h: u32, mx: i32, my: i32) {
        // Fundo da barra
        self.fill_rect(fb, w, h, 0, 0, w as i32, MENUBAR_HEIGHT, [22, 22, 28, 255]);
        // Linha inferior
        self.fill_rect(fb, w, h, 0, MENUBAR_HEIGHT - 1, w as i32, 1, [40, 40, 50, 255]);

        let mut x = 0;
        for (i, menu) in MENUS.iter().enumerate() {
            let tw = self.text_width(menu.label, MENU_FONT_SIZE);
            let item_w = tw + MENU_PAD_X * 2;

            let hover = mx >= x && mx < x + item_w && my >= 0 && my < MENUBAR_HEIGHT;
            let active = self.open_menu == Some(i);

            if hover || active {
                self.fill_rect(fb, w, h, x, 0, item_w, MENUBAR_HEIGHT, [40, 40, 55, 255]);
            }

            let text_color = if hover || active { [255, 255, 255, 255] } else { [170, 170, 170, 255] };
            self.draw_text(fb, w, h, menu.label, MENU_FONT_SIZE, x + MENU_PAD_X, 7, text_color);

            // Dropdown
            if active {
                self.draw_dropdown(fb, w, h, x, menu.items, mx, my);
            }

            x += item_w;
        }
    }

    fn draw_dropdown(&self, fb: &mut [u8], w: u32, h: u32, x: i32, items: &[(&str, MenuAction)], mx: i32, my: i32) {
        let mut max_w = 0;
        for (label, _) in items {
            let tw = self.text_width(label, MENU_FONT_SIZE);
            if tw > max_w { max_w = tw; }
        }
        let dropdown_w = max_w + DROPDOWN_PAD_X * 2;
        let dropdown_h = items.len() as i32 * DROPDOWN_ITEM_H;
        let dy = MENUBAR_HEIGHT;

        // Fundo do dropdown
        self.fill_rect(fb, w, h, x, dy, dropdown_w, dropdown_h, [28, 28, 36, 255]);
        // Borda
        self.fill_rect(fb, w, h, x, dy, dropdown_w, 1, [50, 50, 60, 255]);
        self.fill_rect(fb, w, h, x, dy + dropdown_h - 1, dropdown_w, 1, [50, 50, 60, 255]);
        self.fill_rect(fb, w, h, x, dy, 1, dropdown_h, [50, 50, 60, 255]);
        self.fill_rect(fb, w, h, x + dropdown_w - 1, dy, 1, dropdown_h, [50, 50, 60, 255]);

        for (i, (label, _)) in items.iter().enumerate() {
            let iy = dy + i as i32 * DROPDOWN_ITEM_H;
            let hover = mx >= x && mx < x + dropdown_w && my >= iy && my < iy + DROPDOWN_ITEM_H;

            if hover {
                self.fill_rect(fb, w, h, x + 1, iy, dropdown_w - 2, DROPDOWN_ITEM_H, [50, 70, 120, 255]);
            }

            let color = if hover { [255, 255, 255, 255] } else { [170, 170, 170, 255] };
            self.draw_text(fb, w, h, label, MENU_FONT_SIZE, x + DROPDOWN_PAD_X, iy + 6, color);
        }
    }

    // Retorna a posição X de cada menu item na barra
    fn menu_item_x(&self, index: usize) -> (i32, i32) {
        let mut x = 0;
        for (i, menu) in MENUS.iter().enumerate() {
            let tw = self.text_width(menu.label, MENU_FONT_SIZE);
            let item_w = tw + MENU_PAD_X * 2;
            if i == index { return (x, item_w); }
            x += item_w;
        }
        (0, 0)
    }

    // Processa click do mouse, retorna a ação se clicou num item
    pub fn handle_click(&mut self, mx: i32, my: i32) -> MenuAction {
        // Click na barra?
        if my >= 0 && my < MENUBAR_HEIGHT {
            let mut x = 0;
            for (i, menu) in MENUS.iter().enumerate() {
                let tw = self.text_width(menu.label, MENU_FONT_SIZE);
                let item_w = tw + MENU_PAD_X * 2;
                if mx >= x && mx < x + item_w {
                    if self.open_menu == Some(i) {
                        self.open_menu = None;
                    } else {
                        self.open_menu = Some(i);
                    }
                    return MenuAction::None;
                }
                x += item_w;
            }
        }

        // Click no dropdown?
        if let Some(menu_idx) = self.open_menu {
            let menu = &MENUS[menu_idx];
            let (x, _) = self.menu_item_x(menu_idx);
            let mut max_w = 0;
            for (label, _) in menu.items {
                let tw = self.text_width(label, MENU_FONT_SIZE);
                if tw > max_w { max_w = tw; }
            }
            let dropdown_w = max_w + DROPDOWN_PAD_X * 2;
            let dy = MENUBAR_HEIGHT;

            for (i, (_, action)) in menu.items.iter().enumerate() {
                let iy = dy + i as i32 * DROPDOWN_ITEM_H;
                if mx >= x && mx < x + dropdown_w && my >= iy && my < iy + DROPDOWN_ITEM_H {
                    self.open_menu = None;
                    return *action;
                }
            }

            // Clicou fora, fechar
            self.open_menu = None;
        }

        MenuAction::None
    }

    pub fn draw_sidebar(&self, fb: &mut [u8], w: u32, h: u32, mx: i32, my: i32) {
        let sh = h as i32;

        // Fundo da sidebar
        self.fill_rect(fb, w, h, 0, MENUBAR_HEIGHT, SIDEBAR_WIDTH, sh - MENUBAR_HEIGHT, [18, 18, 24, 255]);
        // Borda direita
        self.fill_rect(fb, w, h, SIDEBAR_WIDTH - 1, MENUBAR_HEIGHT, 1, sh - MENUBAR_HEIGHT, [40, 40, 50, 255]);

        let items: &[(&str, &str, MenuAction)] = &[
            (">>",  "Open ROM",  MenuAction::OpenRom),
            ("↺",  "Reset",     MenuAction::Reset),
            ("×",  "Quit",      MenuAction::Quit),
        ];

        let mut y = MENUBAR_HEIGHT + 12;

        // Seção "File"
        self.draw_text(fb, w, h, "FILE", 11.0, SIDEBAR_PAD_X, y, [70, 70, 80, 255]);
        y += 22;

        for (icon, label, _) in items {
            let hover = mx >= 0 && mx < SIDEBAR_WIDTH && my >= y && my < y + SIDEBAR_ITEM_H;

            if hover {
                self.fill_rect(fb, w, h, 1, y, SIDEBAR_WIDTH - 2, SIDEBAR_ITEM_H, [35, 35, 50, 255]);
            }

            let color = if hover { [240, 240, 240, 255] } else { [160, 160, 160, 255] };
            let icon_color = if hover { [100, 160, 255, 255] } else { [80, 80, 100, 255] };

            self.draw_text(fb, w, h, icon, SIDEBAR_FONT_SIZE, SIDEBAR_PAD_X, y + 10, icon_color);
            self.draw_text(fb, w, h, label, SIDEBAR_FONT_SIZE, SIDEBAR_PAD_X + 28, y + 10, color);

            y += SIDEBAR_ITEM_H;
        }

        // Separador
        y += 6;
        self.fill_rect(fb, w, h, SIDEBAR_PAD_X, y, SIDEBAR_WIDTH - SIDEBAR_PAD_X * 2, 1, [40, 40, 50, 255]);
        y += 12;

        // Seção "Controls"
        self.draw_text(fb, w, h, "CONTROLS", 11.0, SIDEBAR_PAD_X, y, [70, 70, 80, 255]);
        y += 22;

        let controls = [
            ("Z", "A"),
            ("X", "B"),
            ("Tab", "Select"),
            ("Enter", "Start"),
            ("Arrows", "D-Pad"),
        ];

        for (key, action) in &controls {
            self.draw_text(fb, w, h, key, 12.0, SIDEBAR_PAD_X + 4, y + 4, [100, 160, 255, 255]);
            self.draw_text(fb, w, h, action, 12.0, SIDEBAR_PAD_X + 60, y + 4, [120, 120, 120, 255]);
            y += 22;
        }
    }

    pub fn handle_sidebar_click(&mut self, mx: i32, my: i32) -> MenuAction {
        if mx < 0 || mx >= SIDEBAR_WIDTH { return MenuAction::None; }

        let items: &[MenuAction] = &[
            MenuAction::OpenRom,
            MenuAction::Reset,
            MenuAction::Quit,
        ];

        let mut y = MENUBAR_HEIGHT + 12 + 22; // skip seção header
        for action in items {
            if my >= y && my < y + SIDEBAR_ITEM_H {
                return *action;
            }
            y += SIDEBAR_ITEM_H;
        }

        MenuAction::None
    }
}
