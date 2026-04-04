// fonte 5x7 mínima (H,E,L,O, ,W,R,D). 1 bit = 1 pixel; 5 col x 7 lin.
fn fonte(c: char) -> [u8; 7] {
    match c {
        'H' => [0b10001,0b10001,0b11111,0b10001,0b10001,0b10001,0b10001],
        'E' => [0b11111,0b10000,0b11110,0b10000,0b10000,0b10000,0b11111],
        'L' => [0b10000,0b10000,0b10000,0b10000,0b10000,0b10000,0b11111],
        'O' => [0b01110,0b10001,0b10001,0b10001,0b10001,0b10001,0b01110],
        'W' => [0b10001,0b10001,0b10101,0b10101,0b10101,0b01010,0b01010],
        'R' => [0b11110,0b10001,0b10001,0b11110,0b10100,0b10010,0b10001],
        'D' => [0b11110,0b10001,0b10001,0b10001,0b10001,0b10001,0b11110],
        ' ' => [0,0,0,0,0,0,0],
        _   => [0,0,0,0,0,0,0],
    }
}

pub fn draw_str(fb: &mut [u8], w: u32, h: u32, s: &str, scale: i32, rgba: [u8; 4]) {
    let (w_i, h_i) = (w as i32, h as i32);
    let char_w = 5 * scale;
    let char_h = 7 * scale;
    let spacing = 1 * scale;
    let text_w = (s.len() as i32) * (char_w + spacing) - spacing;
    let x0 = (w_i - text_w) / 2;
    let y0 = (h_i - char_h) / 2;

    let mut x = x0;
    for ch in s.chars() {
        let g = fonte(ch.to_ascii_uppercase());
        for (row, bits) in g.iter().enumerate() {
            for col in 0..5 {
                if (bits >> (4 - col)) & 1 == 1 {
                    for dy in 0..scale {
                        for dx in 0..scale {
                            let px = x + col as i32 * scale + dx;
                            let py = y0 + row as i32 * scale + dy;
                            if px >= 0 && py >= 0 && px < w_i && py < h_i {
                                let idx = ((py as u32 * w + px as u32) * 4) as usize;
                                fb[idx..idx + 4].copy_from_slice(&rgba);
                            }
                        }
                    }
                }
            }
        }
        x += char_w + spacing;
    }
}
