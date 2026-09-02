/// Paleta NTSC de 64 cores (RGBA8); ênfase e cinza entram em F2-07.
const NES_PALETTE: [[u8; 4]; 64] = [
    [84, 84, 84, 255],
    [0, 30, 116, 255],
    [8, 16, 144, 255],
    [48, 0, 136, 255],
    [68, 0, 100, 255],
    [92, 0, 48, 255],
    [84, 4, 0, 255],
    [60, 24, 0, 255],
    [32, 42, 0, 255],
    [8, 58, 0, 255],
    [0, 64, 0, 255],
    [0, 60, 0, 255],
    [0, 50, 60, 255],
    [0, 0, 0, 255],
    [0, 0, 0, 255],
    [0, 0, 0, 255],
    [152, 150, 152, 255],
    [8, 76, 196, 255],
    [48, 50, 236, 255],
    [92, 30, 228, 255],
    [136, 20, 176, 255],
    [160, 20, 100, 255],
    [152, 34, 32, 255],
    [120, 60, 0, 255],
    [84, 90, 0, 255],
    [40, 114, 0, 255],
    [8, 124, 0, 255],
    [0, 118, 40, 255],
    [0, 102, 120, 255],
    [0, 0, 0, 255],
    [0, 0, 0, 255],
    [0, 0, 0, 255],
    [236, 238, 236, 255],
    [76, 154, 236, 255],
    [120, 124, 236, 255],
    [176, 98, 236, 255],
    [228, 84, 236, 255],
    [236, 88, 180, 255],
    [236, 106, 100, 255],
    [212, 136, 32, 255],
    [160, 170, 0, 255],
    [116, 196, 0, 255],
    [76, 208, 32, 255],
    [56, 204, 108, 255],
    [56, 180, 204, 255],
    [60, 60, 60, 255],
    [0, 0, 0, 255],
    [0, 0, 0, 255],
    [236, 238, 236, 255],
    [168, 204, 236, 255],
    [188, 188, 236, 255],
    [212, 178, 236, 255],
    [236, 174, 236, 255],
    [236, 174, 212, 255],
    [236, 180, 176, 255],
    [228, 196, 144, 255],
    [204, 210, 120, 255],
    [180, 222, 120, 255],
    [168, 226, 144, 255],
    [152, 226, 180, 255],
    [160, 214, 228, 255],
    [160, 162, 160, 255],
    [0, 0, 0, 255],
    [0, 0, 0, 255],
];

use crate::cartridge::{Cartridge, Mirror};

/// Dots entre a escrita em `$2001` e o render ligar/desligar de fato.
const RENDER_DELAY: u8 = 2;

pub struct Ppu {
    pub nametable: [[u8; 1024]; 2],
    pub palette_table: [u8; 32],

    // Status registers
    pub status: u8,
    pub mask: u8,
    pub control: u8,

    // Internal registers
    address_latch: u8,
    ppu_data_buffer: u8,
    pub vram_addr: u16, // v register - current VRAM address
    pub tram_addr: u16, // t register - temporary VRAM address
    fine_x: u8,

    // Background rendering
    bg_next_tile_id: u8,
    bg_next_tile_attr: u8,
    bg_next_tile_lsb: u8,
    bg_next_tile_msb: u8,
    bg_shifter_pattern_lo: u16,
    bg_shifter_pattern_hi: u16,
    bg_shifter_attr_lo: u16,
    bg_shifter_attr_hi: u16,

    // Sprite rendering
    pub oam: [u8; 256],
    oam_addr: u8,
    sprites_scanline: [ObjectAttributeEntry; 8],
    sprite_count: usize,
    sprite_shifter_pattern_lo: [u8; 8],
    sprite_shifter_pattern_hi: [u8; 8],
    sprite_zero_hit_possible: bool,
    sprite_zero_being_rendered: bool,

    // Screen (no heap pra nao estourar a stack)
    pub screen: Box<[[u8; 4]; 256 * 240]>,

    // Timing
    pub scanline: i16,
    pub cycle: i16,

    pub frame_complete: bool,

    // Scanline callback (pra MMC3 IRQ)
    pub scanline_trigger: bool,

    // Frame par/ímpar
    odd_frame: bool,
    /// `$2002` lido um dot antes do VBL: a flag não é setada neste frame (e não há NMI).
    prevent_vbl: bool,
    /// Render ligado (bits 3/4 de `$2001`), com o atraso de `RENDER_DELAY` dots do hardware.
    rendering: bool,
    render_next: bool,
    render_delay: u8,
}

#[derive(Clone, Copy)]
struct ObjectAttributeEntry {
    y: u8,
    id: u8,
    attribute: u8,
    x: u8,
}

impl Default for Ppu {
    fn default() -> Self {
        Self::new()
    }
}

impl Ppu {
    pub fn new() -> Self {
        Ppu {
            nametable: [[0; 1024]; 2],
            palette_table: [0; 32],
            status: 0x80, // vblank flag setado no powerup
            mask: 0,
            control: 0,
            address_latch: 0,
            ppu_data_buffer: 0,
            vram_addr: 0,
            tram_addr: 0,
            fine_x: 0,
            bg_next_tile_id: 0,
            bg_next_tile_attr: 0,
            bg_next_tile_lsb: 0,
            bg_next_tile_msb: 0,
            bg_shifter_pattern_lo: 0,
            bg_shifter_pattern_hi: 0,
            bg_shifter_attr_lo: 0,
            bg_shifter_attr_hi: 0,
            oam: [0; 256],
            oam_addr: 0,
            sprites_scanline: [ObjectAttributeEntry { y: 0xFF, id: 0xFF, attribute: 0xFF, x: 0xFF }; 8],
            sprite_count: 0,
            sprite_shifter_pattern_lo: [0; 8],
            sprite_shifter_pattern_hi: [0; 8],
            sprite_zero_hit_possible: false,
            sprite_zero_being_rendered: false,
            screen: vec![[0u8; 4]; 256 * 240].into_boxed_slice().try_into().unwrap(),
            scanline: 241, // NES powerup: PPU começa em vblank
            cycle: 0,
            frame_complete: false,
            scanline_trigger: false,
            odd_frame: false,
            prevent_vbl: false,
            rendering: false,
            render_next: false,
            render_delay: 0,
        }
    }

    pub fn cpu_read_debug(&self, addr: u16) -> u8 {
        match addr {
            0x0002 => self.status,
            0x0004 => self.oam[self.oam_addr as usize],
            _ => 0,
        }
    }

    pub fn cpu_read(&mut self, addr: u16, cart: &mut Cartridge) -> u8 {
        match addr {
            0x0002 => {
                let data = (self.status & 0xE0) | (self.ppu_data_buffer & 0x1F);
                self.status &= 0x7F;
                self.address_latch = 0;
                // Lido um dot antes de (241,1): lê 0 e o VBL deste frame é suprimido
                if self.scanline == 241 && self.cycle == 1 {
                    self.prevent_vbl = true;
                }
                data
            }
            0x0004 => self.oam[self.oam_addr as usize],
            0x0007 => {
                let mut data = self.ppu_data_buffer;
                self.ppu_data_buffer = self.vram_read(self.vram_addr, cart);
                // Paletas retornam imediatamente, buffer recebe o nametable abaixo
                if (self.vram_addr & 0x3FFF) >= 0x3F00 {
                    data = self.ppu_data_buffer;
                    self.ppu_data_buffer = self.vram_read(self.vram_addr - 0x1000, cart);
                }
                self.vram_addr = self.vram_addr.wrapping_add(if self.control & 0x04 != 0 { 32 } else { 1 });
                data
            }
            _ => 0,
        }
    }

    pub fn cpu_write(&mut self, addr: u16, data: u8, cart: &mut Cartridge) {
        match addr {
            0x0000 => {
                // NMI é nível (status.7 & control.7); a CPU detecta a borda
                self.control = data;
                self.tram_addr = (self.tram_addr & 0xF3FF) | ((data as u16 & 0x03) << 10);
            }
            0x0001 => {
                self.mask = data;
                self.render_next = data & 0x18 != 0;
                self.render_delay = RENDER_DELAY;
            }
            0x0002 => {}
            0x0003 => {
                self.oam_addr = data;
            }
            0x0004 => {
                self.oam[self.oam_addr as usize] = data;
                self.oam_addr = self.oam_addr.wrapping_add(1);
            }
            0x0005 => {
                if self.address_latch == 0 {
                    self.fine_x = data & 0x07;
                    self.tram_addr = (self.tram_addr & 0xFFE0) | ((data as u16) >> 3);
                    self.address_latch = 1;
                } else {
                    self.tram_addr = (self.tram_addr & 0x8C1F)
                        | ((data as u16 & 0x07) << 12)
                        | ((data as u16 & 0xF8) << 2);
                    self.address_latch = 0;
                }
            }
            0x0006 => {
                if self.address_latch == 0 {
                    self.tram_addr = ((data as u16 & 0x3F) << 8) | (self.tram_addr & 0x00FF);
                    self.address_latch = 1;
                } else {
                    self.tram_addr = (self.tram_addr & 0xFF00) | data as u16;
                    self.vram_addr = self.tram_addr; // t -> v on second write
                    self.address_latch = 0;
                }
            }
            0x0007 => {
                self.vram_write(self.vram_addr, data, cart);
                if (self.control & 0x04) != 0 {
                    self.vram_addr = self.vram_addr.wrapping_add(32);
                } else {
                    self.vram_addr = self.vram_addr.wrapping_add(1);
                }
            }
            _ => {}
        }
    }

    #[inline]
    fn mirror_nametable(addr: u16, mirror: Mirror) -> (usize, usize) {
        let addr = addr & 0x0FFF;
        let table = (addr >> 10) as usize; // 0-3
        let offset = (addr & 0x03FF) as usize;
        let nt = match mirror {
            Mirror::Vertical => table & 1,
            Mirror::Horizontal => table >> 1,
            Mirror::OneScreenLo => 0,
            Mirror::OneScreenHi => 1,
        };
        (nt, offset)
    }

    /// Leitura no barramento da PPU: CHR pelo mapper, nametables com o mirroring atual, paleta.
    #[inline]
    fn vram_read(&mut self, addr: u16, cart: &mut Cartridge) -> u8 {
        let addr = addr & 0x3FFF;
        if addr <= 0x1FFF {
            cart.chr_read(addr)
        } else if addr <= 0x3EFF {
            let (nt, offset) = Self::mirror_nametable(addr, cart.get_mirror());
            self.nametable[nt][offset]
        } else {
            let addr = Self::palette_index(addr);
            self.palette_table[addr] & if (self.mask & 0x01) != 0 { 0x30 } else { 0x3F }
        }
    }

    #[inline]
    fn palette_index(addr: u16) -> usize {
        let addr = addr & 0x001F;
        (if addr & 0x0013 == 0x0010 { addr & 0x000F } else { addr }) as usize
    }

    #[inline]
    fn vram_write(&mut self, addr: u16, data: u8, cart: &mut Cartridge) {
        let addr = addr & 0x3FFF;
        if addr <= 0x1FFF {
            cart.chr_write(addr, data);
        } else if addr <= 0x3EFF {
            let (nt, offset) = Self::mirror_nametable(addr, cart.get_mirror());
            self.nametable[nt][offset] = data;
        } else {
            self.palette_table[Self::palette_index(addr)] = data;
        }
    }

    /// Um dot de PPU.
    pub fn step(&mut self, cart: &mut Cartridge) {
        if self.render_delay > 0 {
            self.render_delay -= 1;
            if self.render_delay == 0 {
                self.rendering = self.render_next;
            }
        }
        // Frame ímpar com render ligado: (261,339) salta direto para (0,0).
        let skip_dot = self.scanline == -1 && self.cycle == 339 && self.odd_frame && self.rendering;

        // Background rendering logic
        if self.scanline >= -1 && self.scanline < 240 {
            if self.scanline == -1 && self.cycle == 1 {
                // Limpar vblank, sprite overflow, sprite zero hit
                self.status &= !(0x80 | 0x40 | 0x20);
                // Limpar sprite shifters
                for i in 0..8 {
                    self.sprite_shifter_pattern_lo[i] = 0;
                    self.sprite_shifter_pattern_hi[i] = 0;
                }
            }

            if (self.cycle >= 2 && self.cycle < 258) || (self.cycle >= 321 && self.cycle < 338) {
                self.update_shifters();

                match (self.cycle - 1) % 8 {
                    0 => {
                        self.load_background_shifters();
                        self.bg_next_tile_id = self.vram_read(0x2000 | (self.vram_addr & 0x0FFF), cart);
                    }
                    2 => {
                        self.bg_next_tile_attr = self.vram_read(
                            0x23C0
                                | (self.vram_addr & 0x0C00)
                                | ((self.vram_addr >> 4) & 0x38)
                                | ((self.vram_addr >> 2) & 0x07),
                            cart,
                        );
                        if (self.vram_addr & 0x0040) != 0 {
                            self.bg_next_tile_attr >>= 4;
                        }
                        if (self.vram_addr & 0x0002) != 0 {
                            self.bg_next_tile_attr >>= 2;
                        }
                        self.bg_next_tile_attr &= 0x03;
                    }
                    4 => {
                        self.bg_next_tile_lsb = self.vram_read(
                            ((self.control as u16 & 0x10) << 8)
                                + (self.bg_next_tile_id as u16 * 16)
                                + ((self.vram_addr >> 12) & 0x07),
                            cart,
                        );
                    }
                    6 => {
                        self.bg_next_tile_msb = self.vram_read(
                            ((self.control as u16 & 0x10) << 8)
                                + (self.bg_next_tile_id as u16 * 16)
                                + ((self.vram_addr >> 12) & 0x07)
                                + 8,
                            cart,
                        );
                    }
                    7 => {
                        self.increment_scroll_x();
                    }
                    _ => {}
                }
            }

            if self.cycle == 256 {
                self.increment_scroll_y();
            }

            // MMC3 scanline counter trigger (A12 rising edge)
            if self.cycle == 260 && self.rendering {
                self.scanline_trigger = true;
            }

            if self.cycle == 257 {
                self.transfer_address_x();
            }

            if self.cycle == 338 || self.cycle == 340 {
                self.bg_next_tile_id = self.vram_read(0x2000 | (self.vram_addr & 0x0FFF), cart);
            }

            if self.scanline == -1 && self.cycle >= 280 && self.cycle < 305 {
                self.transfer_address_y();
            }

            // Foreground rendering
            if self.cycle == 257 && self.scanline >= 0 {
                self.sprite_count = 0;
                for i in 0..8 {
                    self.sprites_scanline[i] =
                        ObjectAttributeEntry { y: 0xFF, id: 0xFF, attribute: 0xFF, x: 0xFF };
                }
                self.sprite_zero_hit_possible = false;

                let mut oam_entry = 0;
                while oam_entry < 64 && self.sprite_count < 9 {
                    let diff = self.scanline - self.oam[(oam_entry * 4) as usize] as i16;
                    if diff >= 0
                        && diff < if (self.control & 0x20) != 0 { 16 } else { 8 }
                        && self.sprite_count < 8
                        && self.sprite_count < 8
                    {
                        if oam_entry == 0 {
                            self.sprite_zero_hit_possible = true;
                        }

                        self.sprites_scanline[self.sprite_count] = ObjectAttributeEntry {
                            y: self.oam[(oam_entry * 4) as usize],
                            id: self.oam[(oam_entry * 4 + 1) as usize],
                            attribute: self.oam[(oam_entry * 4 + 2) as usize],
                            x: self.oam[(oam_entry * 4 + 3) as usize],
                        };
                        self.sprite_count += 1;
                    }
                    oam_entry += 1;
                }
                self.status |= if self.sprite_count >= 8 { 0x20 } else { 0 };
            }

            if self.cycle == 340 {
                for i in 0..self.sprite_count {
                    let mut sprite_pattern_bits_lo: u8;
                    let mut sprite_pattern_bits_hi: u8;
                    #[allow(clippy::needless_late_init)]
                    let sprite_pattern_addr_lo: u16;
                    #[allow(clippy::needless_late_init)]
                    let sprite_pattern_addr_hi: u16;

                    if (self.control & 0x20) == 0 {
                        if (self.sprites_scanline[i].attribute & 0x80) == 0 {
                            sprite_pattern_addr_lo = ((self.control as u16 & 0x08) << 9)
                                | (self.sprites_scanline[i].id as u16 * 16)
                                | ((self.scanline - self.sprites_scanline[i].y as i16) as u16);
                        } else {
                            sprite_pattern_addr_lo = ((self.control as u16 & 0x08) << 9)
                                | (self.sprites_scanline[i].id as u16 * 16)
                                | (7 - (self.scanline - self.sprites_scanline[i].y as i16)) as u16;
                        }
                    } else {
                        if (self.sprites_scanline[i].attribute & 0x80) == 0 {
                            if (self.scanline - self.sprites_scanline[i].y as i16) < 8 {
                                sprite_pattern_addr_lo = ((self.sprites_scanline[i].id as u16 & 0x01) << 12)
                                    | ((self.sprites_scanline[i].id as u16 & 0xFE) << 4)
                                    | ((self.scanline - self.sprites_scanline[i].y as i16) as u16 & 0x07);
                            } else {
                                sprite_pattern_addr_lo = ((self.sprites_scanline[i].id as u16 & 0x01) << 12)
                                    | (((self.sprites_scanline[i].id as u16 & 0xFE) + 1) << 4)
                                    | ((self.scanline - self.sprites_scanline[i].y as i16) as u16 & 0x07);
                            }
                        } else {
                            if (self.scanline - self.sprites_scanline[i].y as i16) < 8 {
                                sprite_pattern_addr_lo = ((self.sprites_scanline[i].id as u16 & 0x01) << 12)
                                    | (((self.sprites_scanline[i].id as u16 & 0xFE) + 1) << 4)
                                    | ((7 - (self.scanline - self.sprites_scanline[i].y as i16)) as u16
                                        & 0x07);
                            } else {
                                sprite_pattern_addr_lo = ((self.sprites_scanline[i].id as u16 & 0x01) << 12)
                                    | ((self.sprites_scanline[i].id as u16 & 0xFE) << 4)
                                    | ((7 - ((self.scanline - self.sprites_scanline[i].y as i16) & 0x07))
                                        as u16);
                            }
                        }
                    }

                    sprite_pattern_addr_hi = sprite_pattern_addr_lo + 8;
                    sprite_pattern_bits_lo = self.vram_read(sprite_pattern_addr_lo, cart);
                    sprite_pattern_bits_hi = self.vram_read(sprite_pattern_addr_hi, cart);

                    if (self.sprites_scanline[i].attribute & 0x40) != 0 {
                        fn flip_byte(b: u8) -> u8 {
                            let mut b = b;
                            b = (b & 0xF0) >> 4 | (b & 0x0F) << 4;
                            b = (b & 0xCC) >> 2 | (b & 0x33) << 2;
                            b = (b & 0xAA) >> 1 | (b & 0x55) << 1;
                            b
                        }

                        sprite_pattern_bits_lo = flip_byte(sprite_pattern_bits_lo);
                        sprite_pattern_bits_hi = flip_byte(sprite_pattern_bits_hi);
                    }

                    self.sprite_shifter_pattern_lo[i] = sprite_pattern_bits_lo;
                    self.sprite_shifter_pattern_hi[i] = sprite_pattern_bits_hi;
                }
            }
        }

        if self.scanline == 241 && self.cycle == 1 {
            if !self.prevent_vbl {
                self.status |= 0x80;
            }
            self.prevent_vbl = false;
        }

        // Mux de pixel, sprite 0 hit e escrita na tela: só na janela visível (240 linhas × dots 1..=256).
        let visible = self.scanline >= 0 && self.scanline < 240;
        if visible && self.cycle >= 1 && self.cycle <= 256 {
            self.render_pixel();
        }

        // Shifters de sprite avançam durante a linha (o x de cada sprite conta até zero)
        if visible && self.cycle >= 1 && self.cycle < 258 {
            for i in 0..self.sprite_count {
                if self.sprites_scanline[i].x > 0 {
                    self.sprites_scanline[i].x -= 1;
                } else {
                    self.sprite_shifter_pattern_lo[i] <<= 1;
                    self.sprite_shifter_pattern_hi[i] <<= 1;
                }
            }
        }

        self.cycle += 1;
        if skip_dot {
            self.cycle = 341;
        }
        if self.cycle >= 341 {
            self.cycle = 0;
            self.scanline += 1;
            if self.scanline >= 261 {
                self.scanline = -1;
                self.frame_complete = true;
                self.odd_frame = !self.odd_frame;
            }
        }
    }

    /// Um dot visível: combina background e sprites, detecta sprite 0 hit e grava o pixel.
    #[inline]
    fn render_pixel(&mut self) {
        let mut bg_pixel = 0u8;
        let mut bg_palette = 0u8;

        // Temporariamente desabilitar background rendering para debug
        if (self.mask & 0x08) != 0 && ((self.mask & 0x02) != 0 || self.cycle >= 9) {
            let bit_mux = 0x8000 >> self.fine_x;
            let p0_pixel = if (self.bg_shifter_pattern_lo & bit_mux) > 0 { 1 } else { 0 };
            let p1_pixel = if (self.bg_shifter_pattern_hi & bit_mux) > 0 { 1 } else { 0 };
            bg_pixel = (p1_pixel << 1) | p0_pixel;

            let bg_pal0 = if (self.bg_shifter_attr_lo & bit_mux) > 0 { 1 } else { 0 };
            let bg_pal1 = if (self.bg_shifter_attr_hi & bit_mux) > 0 { 1 } else { 0 };
            bg_palette = (bg_pal1 << 1) | bg_pal0;
        }

        let mut fg_pixel = 0u8;
        let mut fg_palette = 0u8;
        let mut fg_priority = false;

        if (self.mask & 0x10) != 0 && ((self.mask & 0x04) != 0 || self.cycle >= 9) {
            self.sprite_zero_being_rendered = false;

            for i in 0..self.sprite_count {
                if self.sprites_scanline[i].x == 0 {
                    let fg_pixel_lo = if (self.sprite_shifter_pattern_lo[i] & 0x80) > 0 { 1 } else { 0 };
                    let fg_pixel_hi = if (self.sprite_shifter_pattern_hi[i] & 0x80) > 0 { 1 } else { 0 };
                    fg_pixel = (fg_pixel_hi << 1) | fg_pixel_lo;

                    fg_palette = (self.sprites_scanline[i].attribute & 0x03) + 0x04;
                    fg_priority = (self.sprites_scanline[i].attribute & 0x20) == 0;

                    if fg_pixel != 0 {
                        if i == 0 {
                            self.sprite_zero_being_rendered = true;
                        }
                        break;
                    }
                }
            }
        }

        let mut pixel = 0u8;
        let mut palette = 0u8;

        if bg_pixel == 0 && fg_pixel == 0 {
            pixel = 0x00;
            palette = 0x00;
        } else if bg_pixel == 0 && fg_pixel > 0 {
            pixel = fg_pixel;
            palette = fg_palette;
        } else if bg_pixel > 0 && fg_pixel == 0 {
            pixel = bg_pixel;
            palette = bg_palette;
        } else if bg_pixel > 0 && fg_pixel > 0 {
            if fg_priority {
                pixel = fg_pixel;
                palette = fg_palette;
            } else {
                pixel = bg_pixel;
                palette = bg_palette;
            }

            if self.sprite_zero_hit_possible
                && self.sprite_zero_being_rendered
                && (self.mask & 0x08) != 0
                && (self.mask & 0x10) != 0
            {
                if (self.mask & 0x02) == 0 || (self.mask & 0x04) == 0 {
                    // Left column clipping ativo, hit só a partir do pixel 8
                    if self.cycle >= 9 && self.cycle < 258 {
                        self.status |= 0x40;
                    }
                } else {
                    if self.cycle >= 1 && self.cycle < 258 {
                        self.status |= 0x40;
                    }
                }
            }
        }

        let color = self.get_color_from_palette_ram(palette, pixel);
        let x = (self.cycle - 1) as usize;
        let y = self.scanline as usize;
        self.screen[y * 256 + x] = color;
    }

    fn update_shifters(&mut self) {
        if (self.mask & 0x08) != 0 {
            self.bg_shifter_pattern_lo <<= 1;
            self.bg_shifter_pattern_hi <<= 1;
            self.bg_shifter_attr_lo <<= 1;
            self.bg_shifter_attr_hi <<= 1;
        }
    }

    fn load_background_shifters(&mut self) {
        self.bg_shifter_pattern_lo = (self.bg_shifter_pattern_lo & 0xFF00) | self.bg_next_tile_lsb as u16;
        self.bg_shifter_pattern_hi = (self.bg_shifter_pattern_hi & 0xFF00) | self.bg_next_tile_msb as u16;

        let attr = if (self.bg_next_tile_attr & 0x01) != 0 { 0xFF } else { 0x00 };
        self.bg_shifter_attr_lo = (self.bg_shifter_attr_lo & 0xFF00) | attr;
        let attr = if (self.bg_next_tile_attr & 0x02) != 0 { 0xFF } else { 0x00 };
        self.bg_shifter_attr_hi = (self.bg_shifter_attr_hi & 0xFF00) | attr;
    }

    fn increment_scroll_x(&mut self) {
        if self.rendering {
            if (self.vram_addr & 0x001F) == 31 {
                self.vram_addr &= !0x001F;
                self.vram_addr ^= 0x0400;
            } else {
                self.vram_addr += 1;
            }
        }
    }

    fn increment_scroll_y(&mut self) {
        if self.rendering {
            if (self.vram_addr & 0x7000) != 0x7000 {
                self.vram_addr += 0x1000;
            } else {
                self.vram_addr &= !0x7000;
                let mut y = (self.vram_addr & 0x03E0) >> 5;
                if y == 29 {
                    y = 0;
                    self.vram_addr ^= 0x0800;
                } else if y == 31 {
                    y = 0;
                } else {
                    y += 1;
                }
                self.vram_addr = (self.vram_addr & !0x03E0) | (y << 5);
            }
        }
    }

    fn transfer_address_x(&mut self) {
        if self.rendering {
            // Copy horizontal bits from t to v
            self.vram_addr = (self.vram_addr & !0x041F) | (self.tram_addr & 0x041F);
        }
    }

    fn transfer_address_y(&mut self) {
        if self.rendering {
            // Copy vertical bits from t to v
            self.vram_addr = (self.vram_addr & !0x7BE0) | (self.tram_addr & 0x7BE0);
        }
    }

    fn get_color_from_palette_ram(&self, palette: u8, pixel: u8) -> [u8; 4] {
        // Se pixel é 0, usar sempre cor de background (palette 0, pixel 0)
        if pixel == 0 {
            let color_index = self.palette_table[0] & 0x3F;
            return self.get_nes_color(color_index);
        }

        let addr = (palette as u16 * 4 + pixel as u16) & 0x001F;
        let addr = if addr == 0x0010 {
            0x0000
        } else if addr == 0x0014 {
            0x0004
        } else if addr == 0x0018 {
            0x0008
        } else if addr == 0x001C {
            0x000C
        } else {
            addr
        };
        let color_index = self.palette_table[addr as usize] & 0x3F;
        self.get_nes_color(color_index)
    }

    #[inline]
    fn get_nes_color(&self, color_index: u8) -> [u8; 4] {
        NES_PALETTE[(color_index & 0x3F) as usize]
    }

    /// Nível da saída /NMI da PPU: VBL ligado e NMI habilitado em `$2000`.
    #[inline]
    pub fn nmi_output(&self) -> bool {
        (self.status & 0x80) != 0 && (self.control & 0x80) != 0
    }
}
