/// Paleta NTSC base de 64 cores (RGBA8).
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
/// Frames até um bit do open bus da PPU decair (~600 ms).
const IO_DECAY_FRAMES: u8 = 36;

/// Paleta completa: índice de 9 bits = `ênfase (bits 6-8) | cor (bits 0-5)`, como o PPU grava
/// no framebuffer. A ênfase escurece os canais NÃO enfatizados em ~16 % por bit ligado.
pub static PALETTE_RGBA: [[u8; 4]; 512] = build_palette();

const fn build_palette() -> [[u8; 4]; 512] {
    let mut out = [[0u8; 4]; 512];
    let mut i = 0;
    while i < 512 {
        let base = NES_PALETTE[i & 0x3F];
        let emph = (i >> 6) & 7; // bit0 = vermelho, bit1 = verde, bit2 = azul
        let mut c = [base[0] as u32, base[1] as u32, base[2] as u32];
        if emph != 0 {
            let mut ch = 0;
            while ch < 3 {
                if emph & (1 << ch) == 0 {
                    c[ch] = c[ch] * 215 / 256;
                }
                ch += 1;
            }
        }
        out[i] = [c[0] as u8, c[1] as u8, c[2] as u8, 255];
        i += 1;
    }
    out
}

pub struct Ppu {
    pub nametable: [[u8; 1024]; 4],
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
    /// Dot em que a avaliação (com o bug do hardware) setaria a flag de overflow nesta linha.
    overflow_dot: Option<i16>,
    /// "OAM secundário": sprites avaliados para a linha seguinte, copiados no dot 257.
    next_sprites: [ObjectAttributeEntry; 8],
    next_sprite_count: usize,
    next_sprite_zero: bool,

    /// Framebuffer por índice de paleta (`ênfase << 6 | cor`), 256×240. RGBA via `PALETTE_RGBA`.
    pub screen: Box<[u16; 256 * 240]>,

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
    /// Open bus da PPU ("decay register"): cada bit guarda o último valor que passou pelo
    /// barramento e decai para 0 depois de ~600 ms sem ser renovado.
    io_bus: u8,
    io_decay: [u8; 8],
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
            nametable: [[0; 1024]; 4],
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
            overflow_dot: None,
            next_sprites: [ObjectAttributeEntry { y: 0xFF, id: 0xFF, attribute: 0xFF, x: 0xFF }; 8],
            next_sprite_count: 0,
            next_sprite_zero: false,
            screen: vec![0u16; 256 * 240].into_boxed_slice().try_into().unwrap(),
            scanline: 241, // NES powerup: PPU começa em vblank
            cycle: 0,
            frame_complete: false,
            scanline_trigger: false,
            odd_frame: false,
            prevent_vbl: false,
            io_bus: 0,
            io_decay: [0; 8],
            rendering: false,
            render_next: false,
            render_delay: 0,
        }
    }

    pub fn cpu_read_debug(&self, addr: u16) -> u8 {
        match addr {
            0x0002 => self.status,
            0x0004 => {
                let v = self.oam[self.oam_addr as usize];
                // bits 2-4 do byte de atributo não existem no hardware
                if self.oam_addr & 3 == 2 { v & 0xE3 } else { v }
            }
            _ => 0,
        }
    }

    /// Renova os bits `mask` do open bus com `value`.
    #[inline]
    fn refresh_bus(&mut self, mask: u8, value: u8) {
        self.io_bus = (self.io_bus & !mask) | (value & mask);
        for bit in 0..8 {
            if mask & (1 << bit) != 0 {
                self.io_decay[bit] = IO_DECAY_FRAMES;
            }
        }
    }

    /// Chamado a cada frame: bits do open bus não renovados decaem para 0.
    fn decay_bus(&mut self) {
        for bit in 0..8 {
            if self.io_decay[bit] > 0 {
                self.io_decay[bit] -= 1;
                if self.io_decay[bit] == 0 {
                    self.io_bus &= !(1 << bit);
                }
            }
        }
    }

    pub fn cpu_read(&mut self, addr: u16, cart: &mut Cartridge) -> u8 {
        match addr {
            0x0002 => {
                let data = (self.status & 0xE0) | (self.io_bus & 0x1F);
                self.refresh_bus(0xE0, self.status);
                self.status &= 0x7F;
                self.address_latch = 0;
                // Lido um dot antes de (241,1): lê 0 e o VBL deste frame é suprimido
                if self.scanline == 241 && self.cycle == 1 {
                    self.prevent_vbl = true;
                }
                data
            }
            0x0004 => {
                let v = self.oam[self.oam_addr as usize];
                // bits 2-4 do byte de atributo não existem no hardware
                let v = if self.oam_addr & 3 == 2 { v & 0xE3 } else { v };
                self.refresh_bus(0xFF, v);
                v
            }
            0x0007 => {
                let data = if (self.vram_addr & 0x3FFF) >= 0x3F00 {
                    // Paleta: sai direto (6 bits + open bus nos 2 altos); o buffer recebe o
                    // nametable "embaixo" da paleta
                    let pal = self.vram_read(self.vram_addr, cart) & 0x3F;
                    self.ppu_data_buffer = self.vram_read(self.vram_addr - 0x1000, cart);
                    self.refresh_bus(0x3F, pal);
                    pal | (self.io_bus & 0xC0)
                } else {
                    let data = self.ppu_data_buffer;
                    self.ppu_data_buffer = self.vram_read(self.vram_addr, cart);
                    self.refresh_bus(0xFF, data);
                    data
                };
                self.increment_vram_addr();
                data
            }
            _ => self.io_bus,
        }
    }

    /// Incremento de `v` após `$2007`: +1/+32 fora do render; durante o render o hardware
    /// faz um incremento de X grosso e um de Y.
    #[inline]
    fn increment_vram_addr(&mut self) {
        if self.rendering && self.scanline < 240 {
            self.increment_scroll_x();
            self.increment_scroll_y();
        } else {
            self.vram_addr = self.vram_addr.wrapping_add(if self.control & 0x04 != 0 { 32 } else { 1 });
        }
    }

    pub fn cpu_write(&mut self, addr: u16, data: u8, cart: &mut Cartridge) {
        self.refresh_bus(0xFF, data);
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
                if self.rendering && self.scanline < 240 {
                    // durante o render a escrita é ignorada, mas OAMADDR sobe 4 (bits altos)
                    self.oam_addr = self.oam_addr.wrapping_add(4);
                } else {
                    self.oam[self.oam_addr as usize] = data;
                    self.oam_addr = self.oam_addr.wrapping_add(1);
                }
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
                self.increment_vram_addr();
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
            Mirror::FourScreen => table,
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

            // Avaliação de sprites da próxima linha: o hardware faz entre os dots 65 e ~256;
            // aqui é feita de uma vez no dot 65, com o mesmo algoritmo (inclusive o bug do
            // overflow) e o dot em que a flag de overflow seria setada.
            if self.cycle == 65 && self.scanline >= 0 && self.rendering {
                self.evaluate_sprites();
            }
            if self.cycle == 257 {
                self.sprites_scanline = self.next_sprites;
                self.sprite_count = self.next_sprite_count;
                self.sprite_zero_hit_possible = self.next_sprite_zero;
            }
            if self.scanline >= 0 && self.overflow_dot == Some(self.cycle) {
                self.status |= 0x20;
            }
            // OAMADDR é zerado durante as buscas de sprite
            if self.rendering && (257..=320).contains(&self.cycle) {
                self.oam_addr = 0;
            }

            // Busca dos padrões dos sprites da próxima linha (feita de uma vez no dot 340)
            if self.cycle == 340 && self.scanline >= 0 {
                let tall = (self.control & 0x20) != 0;
                let last = if tall { 15u16 } else { 7 };
                for i in 0..self.sprite_count {
                    let sp = self.sprites_scanline[i];
                    let mut row = (self.scanline - sp.y as i16) as u16 & last;
                    if sp.attribute & 0x80 != 0 {
                        row = last - row; // flip vertical
                    }
                    let addr = if tall {
                        ((sp.id as u16 & 0x01) << 12)
                            | ((sp.id as u16 & 0xFE) << 4)
                            | ((row & 8) << 1)
                            | (row & 7)
                    } else {
                        ((self.control as u16 & 0x08) << 9) | ((sp.id as u16) << 4) | row
                    };
                    let mut lo = self.vram_read(addr, cart);
                    let mut hi = self.vram_read(addr + 8, cart);
                    if sp.attribute & 0x40 != 0 {
                        lo = lo.reverse_bits(); // flip horizontal
                        hi = hi.reverse_bits();
                    }
                    self.sprite_shifter_pattern_lo[i] = lo;
                    self.sprite_shifter_pattern_hi[i] = hi;
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
        if visible && self.cycle >= 1 && self.cycle <= 256 {
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
                self.decay_bus();
            }
        }
    }

    /// Avaliação de sprites para a linha seguinte, a partir de `oam_addr` (desalinhamento
    /// incluído). Cada leitura de OAM custa 2 dots a partir do 65; depois do 8º sprite o
    /// hardware continua lendo com o índice `m` errado — é isso que gera a flag de overflow.
    fn evaluate_sprites(&mut self) {
        let height: i16 = if (self.control & 0x20) != 0 { 16 } else { 8 };
        let in_range = |y: u8| {
            let diff = self.scanline - y as i16;
            diff >= 0 && diff < height
        };
        self.next_sprite_count = 0;
        self.next_sprite_zero = false;
        self.overflow_dot = None;
        for e in self.next_sprites.iter_mut() {
            *e = ObjectAttributeEntry { y: 0xFF, id: 0xFF, attribute: 0xFF, x: 0xFF };
        }

        let start = self.oam_addr as usize;
        let mut reads: i16 = 0;
        let mut n = 0usize;
        // fase 1: até 8 sprites
        while n < 64 && self.next_sprite_count < 8 {
            let base = (start + n * 4) & 0xFF;
            let y = self.oam[base];
            reads += 1;
            if in_range(y) {
                reads += 3;
                if n == 0 {
                    self.next_sprite_zero = true;
                }
                self.next_sprites[self.next_sprite_count] = ObjectAttributeEntry {
                    y,
                    id: self.oam[(base + 1) & 0xFF],
                    attribute: self.oam[(base + 2) & 0xFF],
                    x: self.oam[(base + 3) & 0xFF],
                };
                self.next_sprite_count += 1;
            }
            n += 1;
        }
        // fase 2 (bug): lê OAM[n][m] como Y, incrementando m sem carry a cada erro
        if self.next_sprite_count == 8 {
            let mut m = 0usize;
            while n < 64 {
                let y = self.oam[(start + n * 4 + m) & 0xFF];
                reads += 1;
                if in_range(y) {
                    self.overflow_dot = Some(65 + 2 * (reads - 1));
                    break;
                }
                n += 1;
                m = (m + 1) & 3;
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
                // nunca no pixel 255; com clipping da coluna esquerda, só a partir do pixel 8
                let x = self.cycle - 1;
                let clipped = (self.mask & 0x02) == 0 || (self.mask & 0x04) == 0;
                if x != 255 && (!clipped || x >= 8) {
                    self.status |= 0x40;
                }
            }
        }

        let color = if !self.rendering && (self.vram_addr & 0x3F00) == 0x3F00 {
            // Render desligado com `v` apontando para a paleta: o backdrop mostra essa entrada
            // (é assim que demos como full_palette exibem as 512 cores)
            self.palette_table[Self::palette_index(self.vram_addr)]
        } else {
            self.color_from_palette_ram(palette, pixel)
        };
        let color = color & if self.mask & 0x01 != 0 { 0x30 } else { 0x3F };
        let x = (self.cycle - 1) as usize;
        let y = self.scanline as usize;
        self.screen[y * 256 + x] = ((self.mask as u16 & 0xE0) << 1) | color as u16;
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

    /// Cor (0-63) da paleta RAM para o par (paleta, pixel); pixel 0 é sempre o backdrop.
    #[inline]
    fn color_from_palette_ram(&self, palette: u8, pixel: u8) -> u8 {
        if pixel == 0 {
            return self.palette_table[0];
        }
        self.palette_table[Self::palette_index((palette as u16) << 2 | pixel as u16)]
    }

    /// Nível da saída /NMI da PPU: VBL ligado e NMI habilitado em `$2000`.
    #[inline]
    pub fn nmi_output(&self) -> bool {
        (self.status & 0x80) != 0 && (self.control & 0x80) != 0
    }
}
