use fontdue::Font;

const FONT_DATA: &[u8] = include_bytes!("../assets/NotoSans-Regular.ttf");

pub struct Ui {
    font: Font,
}

impl Ui {
    pub fn new() -> Self {
        let font = Font::from_bytes(FONT_DATA, fontdue::FontSettings::default()).unwrap();
        Ui { font }
    }

    // Desenha texto centralizado horizontalmente no framebuffer RGBA
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
                    // Alpha blending
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

        // Só bordas, sem preenchimento
        for px in bx..bx + bw {
            // Borda superior e inferior
            for t in 0..1 {
                let py = by + t;
                if px >= 0 && py >= 0 && px < w_i && py < h_i {
                    let idx = ((py as u32 * w + px as u32) * 4) as usize;
                    fb[idx..idx + 4].copy_from_slice(&border);
                }
                let py = by + bh - 1 + t;
                if px >= 0 && py >= 0 && px < w_i && py < h_i {
                    let idx = ((py as u32 * w + px as u32) * 4) as usize;
                    fb[idx..idx + 4].copy_from_slice(&border);
                }
            }
        }
        for py in by..by + bh {
            // Borda esquerda e direita
            for t in 0..1 {
                let px = bx + t;
                if px >= 0 && py >= 0 && px < w_i && py < h_i {
                    let idx = ((py as u32 * w + px as u32) * 4) as usize;
                    fb[idx..idx + 4].copy_from_slice(&border);
                }
                let px = bx + bw - 1 + t;
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

    // Retorna (x, y, w, h) do botão pra hit testing
    pub fn button_rect(&self, text: &str, size: f32, cx: i32, cy: i32) -> (i32, i32, i32, i32) {
        let tw = self.text_width(text, size);
        let pad_x = 20;
        let pad_y = 10;
        let bw = tw + pad_x * 2;
        let bh = size as i32 + pad_y * 2;
        (cx - bw / 2, cy - bh / 2, bw, bh)
    }
}
