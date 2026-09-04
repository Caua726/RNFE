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

use crate::cartridge::Cartridge;

/// Dots entre a escrita em `$2001` e o render ligar/desligar de fato.
/// Dots que A12 precisa ficar baixo para a próxima subida clockar o MMC3 (~3 ciclos de M2).
const A12_FILTER_DOTS: u64 = 10;
const RENDER_DELAY: u8 = 2;
/// Frames até um bit do open bus da PPU decair (~600 ms).
const IO_DECAY_FRAMES: u8 = 36;

/// Paleta completa: índice de 9 bits = `ênfase (bits 6-8) | cor (bits 0-5)`, como o PPU grava
/// no framebuffer. A ênfase escurece os canais NÃO enfatizados em ~16 % por bit ligado.
pub static PALETTE_RGBA: [[u8; 4]; 512] = build_palette();

/// Paleta base (64 cores) que o console usa hoje: o padrão, ou uma escolhida pelo usuário.
/// A tabela de 512 entradas (com ênfase) é derivada dela.
pub fn palette_from_base(base: &[[u8; 3]; 64]) -> [[u8; 4]; 512] {
    let mut out = [[0u8; 4]; 512];
    for (i, slot) in out.iter_mut().enumerate() {
        let b = base[i & 0x3F];
        let emph = (i >> 6) & 7;
        let mut c = [b[0] as u32, b[1] as u32, b[2] as u32];
        for (ch, v) in c.iter_mut().enumerate() {
            if emph & !(1 << ch) & 7 != 0 {
                *v = *v * 191 / 256;
            }
        }
        *slot = [c[0] as u8, c[1] as u8, c[2] as u8, 255];
    }
    out
}

/// As 64 cores do padrão, para quem quiser partir delas.
pub fn default_base() -> [[u8; 3]; 64] {
    let mut out = [[0u8; 3]; 64];
    for (i, c) in out.iter_mut().enumerate() {
        let p = NES_PALETTE[i];
        *c = [p[0], p[1], p[2]];
    }
    out
}

fn sprite_limit_default() -> bool {
    true
}

const fn build_palette() -> [[u8; 4]; 512] {
    let mut out = [[0u8; 4]; 512];
    let mut i = 0;
    while i < 512 {
        let base = NES_PALETTE[i & 0x3F];
        let emph = (i >> 6) & 7; // bit0 = vermelho, bit1 = verde, bit2 = azul
        let mut c = [base[0] as u32, base[1] as u32, base[2] as u32];
        if emph != 0 {
            // Cada bit de ênfase realça o seu canal atenuando os OUTROS dois (~0,746). Com os
            // três ligados a imagem inteira escurece — truque usado por vários jogos.
            let mut ch = 0;
            while ch < 3 {
                if emph & !(1 << ch) & 7 != 0 {
                    c[ch] = c[ch] * 191 / 256;
                }
                ch += 1;
            }
        }
        out[i] = [c[0] as u8, c[1] as u8, c[2] as u8, 255];
        i += 1;
    }
    out
}

#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[derive(Clone)]
pub struct Ppu {
    #[cfg_attr(feature = "serde", serde(with = "crate::state::nt"))]
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
    #[cfg_attr(feature = "serde", serde(with = "crate::state::bytes"))]
    pub oam: [u8; 256],
    oam_addr: u8,
    sprites_scanline: [ObjectAttributeEntry; 8],
    sprite_count: usize,
    /// Pixels de sprite da linha atual, um byte por x: bits 0-1 cor, bits 2-3 paleta,
    /// bit 4 atrás do fundo, bit 5 é o sprite 0; 0 = transparente. Preenchido nos dots
    /// 257–320 (o primeiro sprite opaco de cada x vence) e lido no mux de pixel.
    #[cfg_attr(feature = "serde", serde(with = "crate::state::bytes"))]
    sprite_line: [u8; 256],
    sprite_zero_hit_possible: bool,
    /// Dot em que a avaliação (com o bug do hardware) setaria a flag de overflow nesta linha.
    overflow_dot: Option<i16>,
    /// "OAM secundário": sprites avaliados para a linha seguinte, copiados no dot 257.
    next_sprites: [ObjectAttributeEntry; 8],
    /// Limite de 8 sprites por linha (desligar tira o piscar, mas não é o hardware).
    #[cfg_attr(feature = "serde", serde(skip, default = "sprite_limit_default"))]
    sprite_limit: bool,
    /// Sprites além do 8º, quando o limite está desligado (fora do save state: são
    /// transitórios da scanline e o formato não muda).
    #[cfg_attr(feature = "serde", serde(skip))]
    extra_next: Vec<ObjectAttributeEntry>,
    #[cfg_attr(feature = "serde", serde(skip))]
    extra_cur: Vec<ObjectAttributeEntry>,
    next_sprite_count: usize,
    next_sprite_zero: bool,

    /// Framebuffer por índice de paleta (`ênfase << 6 | cor`), 256×240. RGBA via `PALETTE_RGBA`.
    #[cfg_attr(feature = "serde", serde(skip, default = "blank_screen"))]
    pub screen: Box<[u16; 256 * 240]>,

    // Timing
    pub scanline: i16,
    pub cycle: i16,

    pub frame_complete: bool,

    /// Borda de subida de A12 no barramento da PPU (filtrada): o bus entrega ao mapper.
    pub a12_rise: bool,
    /// Dots desde o power-on (para o filtro de A12).
    dots: u64,
    /// Dot em que A12 ficou baixo pela última vez (`None` = está alto).
    a12_low_since: Option<u64>,
    /// Endereço e plano baixo do padrão do sprite sendo buscado (dots 257–320).
    sprite_fetch_addr: u16,
    sprite_fetch_lo: u8,

    // Frame par/ímpar
    odd_frame: bool,
    /// Temporização: muda a última linha, a linha do vblank e o dot pulado. Fora do save
    /// state — é ajuste do console, reaplicado ao carregar.
    #[cfg_attr(feature = "serde", serde(skip))]
    region: crate::Region,
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

#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[derive(Clone, Copy)]
struct ObjectAttributeEntry {
    y: u8,
    id: u8,
    attribute: u8,
    x: u8,
}

fn blank_screen() -> Box<[u16; 256 * 240]> {
    vec![0u16; 256 * 240].into_boxed_slice().try_into().unwrap()
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
            sprite_line: [0; 256],
            sprite_zero_hit_possible: false,
            overflow_dot: None,
            next_sprites: [ObjectAttributeEntry { y: 0xFF, id: 0xFF, attribute: 0xFF, x: 0xFF }; 8],
            sprite_limit: true,
            extra_next: Vec::new(),
            extra_cur: Vec::new(),
            next_sprite_count: 0,
            next_sprite_zero: false,
            screen: blank_screen(),
            scanline: 241, // NES powerup: PPU começa em vblank (ajustado em set_region)
            cycle: 0,
            frame_complete: false,
            a12_rise: false,
            dots: 0,
            a12_low_since: None,
            sprite_fetch_addr: 0,
            sprite_fetch_lo: 0,
            odd_frame: false,
            region: crate::Region::Ntsc,
            prevent_vbl: false,
            io_bus: 0,
            io_decay: [0; 8],
            rendering: false,
            render_next: false,
            render_delay: 0,
        }
    }

    /// Cópia do estado para save state, sem duplicar o framebuffer.
    #[cfg(feature = "serde")]
    pub(crate) fn clone_state(&self) -> Ppu {
        let mut p = Ppu { screen: blank_screen(), ..self.clone_without_screen() };
        p.frame_complete = false;
        p
    }

    #[cfg(feature = "serde")]
    fn clone_without_screen(&self) -> Ppu {
        // clone() copia o Box do framebuffer (123 KB) — aceitável para um save state
        self.clone()
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

    /// Liga/desliga o limite de 8 sprites por linha (desligado = sem piscar).
    pub fn set_sprite_limit(&mut self, on: bool) {
        self.sprite_limit = on;
    }

    /// Troca a temporização (NTSC/PAL): muda a última linha e a linha do vblank.
    pub fn set_region(&mut self, region: crate::Region) {
        self.region = region;
        if self.scanline > region.last_scanline() {
            self.scanline = region.vblank_scanline();
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
                if self.scanline == self.region.vblank_scanline() && self.cycle == 1 {
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
            self.v_changed();
        }
    }

    pub fn cpu_write(&mut self, addr: u16, data: u8, cart: &mut Cartridge) {
        self.refresh_bus(0xFF, data);
        match addr {
            0x0000 => {
                // NMI é nível (status.7 & control.7); a CPU detecta a borda
                self.control = data;
                self.tram_addr = (self.tram_addr & 0xF3FF) | ((data as u16 & 0x03) << 10);
                cart.data.ppu_sprites_16 = data & 0x20 != 0;
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
                    self.v_changed();
                }
            }
            0x0007 => {
                self.vram_write(self.vram_addr, data, cart);
                self.increment_vram_addr();
            }
            _ => {}
        }
    }

    /// Um endereço foi posto no barramento da PPU: detector de borda de A12 com o filtro do
    /// MMC3 (A12 precisa ter ficado baixo por alguns ciclos de M2 para a subida contar).
    #[inline]
    fn bus_addr(&mut self, addr: u16) {
        if addr & 0x1000 == 0 {
            if self.a12_low_since.is_none() {
                self.a12_low_since = Some(self.dots);
            }
        } else if let Some(t) = self.a12_low_since.take() {
            if self.dots - t >= A12_FILTER_DOTS {
                self.a12_rise = true;
            }
        }
    }

    /// Está buscando tiles (render ligado numa linha visível ou na pré-render)?
    #[inline]
    fn fetching(&self) -> bool {
        self.rendering && self.scanline < 240
    }

    /// `v` mudou fora do render: o barramento da PPU mostra `v`.
    #[inline]
    fn v_changed(&mut self) {
        if !self.fetching() {
            self.bus_addr(self.vram_addr);
        }
    }

    /// Leitura no barramento da PPU: CHR pelo mapper, nametables com o mirroring atual, paleta.
    #[inline]
    fn vram_read(&mut self, addr: u16, cart: &mut Cartridge) -> u8 {
        let addr = addr & 0x3FFF;
        self.bus_addr(addr);
        if addr <= 0x1FFF {
            cart.chr_read(addr)
        } else if addr <= 0x3EFF {
            cart.nt_read(addr, &self.nametable)
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
        self.bus_addr(addr);
        if addr <= 0x1FFF {
            cart.chr_write(addr, data);
        } else if addr <= 0x3EFF {
            cart.nt_write(addr, data, &mut self.nametable);
        } else {
            self.palette_table[Self::palette_index(addr)] = data;
        }
    }

    /// Um dot do pipeline de tiles do fundo (dots 2–257 e 321–337): avança os shifters e faz
    /// a busca deste passo (NT, atributo, plano baixo, plano alto, incremento de X).
    #[inline]
    fn fetch_tile(&mut self, cart: &mut Cartridge) {
        self.update_shifters();
        match (self.cycle - 1) & 7 {
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
                let addr = ((self.control as u16 & 0x10) << 8)
                    + (self.bg_next_tile_id as u16 * 16)
                    + ((self.vram_addr >> 12) & 0x07);
                self.bg_next_tile_lsb = self.vram_read(addr, cart);
            }
            6 => {
                let addr = ((self.control as u16 & 0x10) << 8)
                    + (self.bg_next_tile_id as u16 * 16)
                    + ((self.vram_addr >> 12) & 0x07)
                    + 8;
                self.bg_next_tile_msb = self.vram_read(addr, cart);
            }
            7 => self.increment_scroll_x(),
            _ => {}
        }
    }

    /// Escreve os 8 pixels do sprite do `slot` no buffer da linha (o primeiro opaco vence).
    fn draw_sprite_slot(&mut self, slot: usize, lo: u8, hi: u8) {
        let sp = self.sprites_scanline[slot];
        self.draw_entry(&sp, slot == 0, lo, hi);
    }

    /// Desenha um sprite na linha atual. `zero` marca o sprite 0 (para o hit).
    fn draw_entry(&mut self, sp: &ObjectAttributeEntry, zero: bool, lo: u8, hi: u8) {
        let (lo, hi) =
            if sp.attribute & 0x40 != 0 { (lo.reverse_bits(), hi.reverse_bits()) } else { (lo, hi) };
        let tag = ((sp.attribute & 0x03) << 2)
            | if sp.attribute & 0x20 != 0 { 0x10 } else { 0 }
            | if zero { 0x20 } else { 0 };
        for px in 0..8usize {
            let x = sp.x as usize + px;
            if x >= 256 {
                break;
            }
            let bit = 7 - px;
            let pixel = (((hi >> bit) & 1) << 1) | ((lo >> bit) & 1);
            if pixel != 0 && self.sprite_line[x] == 0 {
                self.sprite_line[x] = tag | pixel;
            }
        }
    }

    /// Endereço do padrão (plano baixo) do sprite no `slot` para a linha atual; slots vazios
    /// e a linha de pré-render usam o tile $FF, como o hardware.
    fn sprite_pattern_addr(&self, slot: usize) -> u16 {
        let tall = (self.control & 0x20) != 0;
        if slot >= self.sprite_count || self.scanline < 0 {
            return if tall { 0x1FE0 } else { ((self.control as u16 & 0x08) << 9) | 0x0FF0 };
        }
        let sp = self.sprites_scanline[slot];
        self.pattern_addr_for(&sp)
    }

    /// Endereço do padrão de um sprite na linha atual.
    fn pattern_addr_for(&self, sp: &ObjectAttributeEntry) -> u16 {
        let tall = (self.control & 0x20) != 0;
        let last = if tall { 15u16 } else { 7 };
        let mut row = (self.scanline - sp.y as i16) as u16 & last;
        if sp.attribute & 0x80 != 0 {
            row = last - row; // flip vertical
        }
        if tall {
            ((sp.id as u16 & 0x01) << 12) | ((sp.id as u16 & 0xFE) << 4) | ((row & 8) << 1) | (row & 7)
        } else {
            ((self.control as u16 & 0x08) << 9) | ((sp.id as u16) << 4) | row
        }
    }

    /// Um dot de PPU.
    pub fn step(&mut self, cart: &mut Cartridge) {
        self.dots += 1;
        #[cfg(feature = "profile-no-ppu")]
        {
            let _ = &cart;
            self.cycle += 1;
            if self.cycle >= 341 {
                self.cycle = 0;
                self.scanline += 1;
                if self.scanline >= self.region.last_scanline() {
                    self.scanline = -1;
                    self.frame_complete = true;
                }
            }
            return;
        }
        if self.render_delay > 0 {
            self.render_delay -= 1;
            if self.render_delay == 0 {
                self.rendering = self.render_next;
            }
        }
        // Frame ímpar com render ligado: (261,339) salta direto para (0,0).
        // O PAL não pula o dot em frame ímpar
        let skip_dot = self.scanline == -1
            && self.cycle == 339
            && self.odd_frame
            && self.rendering
            && self.region == crate::Region::Ntsc;

        // Background rendering logic
        if self.scanline >= -1 && self.scanline < 240 {
            let c = self.cycle;
            match c {
                1 => {
                    if self.scanline == -1 {
                        // Limpar vblank, sprite overflow, sprite zero hit
                        self.status &= !(0x80 | 0x40 | 0x20);
                        self.sprite_line = [0; 256];
                    }
                    // Busca de nametable do 1º tile (o mesmo endereço dos dots 337/339 — é
                    // assim que o MMC5 detecta o começo de uma scanline).
                    if self.rendering {
                        self.vram_read(0x2000 | (self.vram_addr & 0x0FFF), cart);
                    }
                    cart.data.ppu_sprite_fetch = false;
                }
                2..=257 => {
                    // Com o render desligado a PPU não busca nada no barramento: quem observa
                    // (MMC5 conta leituras iguais de nametable, MMC2/MMC4 têm latches) via
                    // eventos que no hardware não existem.
                    if self.rendering {
                        self.fetch_tile(cart);
                    }
                    if c == 65 {
                        if self.scanline >= 0 && self.rendering {
                            // Avaliação de sprites da próxima linha (o hardware faz entre os
                            // dots 65 e ~256; aqui de uma vez, com o mesmo algoritmo).
                            self.evaluate_sprites();
                        }
                    } else if c == 256 {
                        self.increment_scroll_y();
                    } else if c == 257 {
                        self.transfer_address_x();
                        cart.data.ppu_sprite_fetch = true;
                        self.sprite_line = [0; 256];
                        self.sprites_scanline = self.next_sprites;
                        std::mem::swap(&mut self.extra_cur, &mut self.extra_next);
                        self.sprite_count = self.next_sprite_count;
                        self.sprite_zero_hit_possible = self.next_sprite_zero;
                        if self.rendering {
                            self.oam_addr = 0;
                            self.bus_addr(0x2000 | (self.vram_addr & 0x0FFF));
                        }
                    }
                }
                258..=320 => {
                    if self.scanline == -1 && (280..305).contains(&c) {
                        self.transfer_address_y();
                    }
                    // Busca dos padrões dos sprites da próxima linha, nos dots 257–320 como no
                    // hardware (8 slots × 8 dots: 2 leituras de nametable descartadas, depois lo
                    // e hi do padrão). Slots sem sprite buscam o tile $FF — é o que clocka o
                    // MMC3 em toda linha. OAMADDR fica em zero durante as buscas.
                    if self.rendering {
                        self.oam_addr = 0;
                        let slot = ((c - 257) >> 3) as usize;
                        match (c - 257) & 7 {
                            0 | 2 => self.bus_addr(0x2000 | (self.vram_addr & 0x0FFF)),
                            4 => {
                                self.sprite_fetch_addr = self.sprite_pattern_addr(slot);
                                self.sprite_fetch_lo = self.vram_read(self.sprite_fetch_addr, cart);
                            }
                            6 => {
                                let hi = self.vram_read(self.sprite_fetch_addr + 8, cart);
                                if slot < self.sprite_count && self.scanline >= 0 {
                                    self.draw_sprite_slot(slot, self.sprite_fetch_lo, hi);
                                }
                            }
                            _ => {}
                        }
                        // Sprites além do 8º (limite desligado pelo jogador): não existem no
                        // hardware, então buscamos direto no cartucho, sem mexer no padrão de
                        // A12 que o MMC3 usa para contar scanlines.
                        if c == 320 && !self.extra_cur.is_empty() && self.scanline >= 0 {
                            let extras = std::mem::take(&mut self.extra_cur);
                            for sp in &extras {
                                let addr = self.pattern_addr_for(sp);
                                let lo = cart.chr_read(addr);
                                let hi = cart.chr_read(addr + 8);
                                self.draw_entry(sp, false, lo, hi);
                            }
                            self.extra_cur = extras;
                        }
                    }
                }
                321..=337 => {
                    if c == 321 {
                        cart.data.ppu_sprite_fetch = false;
                    }
                    if self.rendering {
                        self.fetch_tile(cart);
                    }
                }
                338 | 340 if self.rendering => {
                    self.bg_next_tile_id = self.vram_read(0x2000 | (self.vram_addr & 0x0FFF), cart);
                }
                _ => {}
            }
            if self.scanline >= 0 && self.overflow_dot == Some(c) {
                self.status |= 0x20;
            }
        }

        if self.scanline == self.region.vblank_scanline() && self.cycle == 1 {
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

        self.cycle += 1;
        if skip_dot {
            self.cycle = 341;
        }
        if self.cycle >= 341 {
            self.cycle = 0;
            self.scanline += 1;
            if self.scanline >= self.region.last_scanline() {
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
        // Sem o limite de 8 (opção do jogador): recolhe o resto da OAM em slots extras. A flag
        // de overflow e os 8 primeiros continuam idênticos ao hardware — só o desenho muda.
        self.extra_next.clear();
        if !self.sprite_limit {
            let mut extra = n;
            while extra < 64 {
                let base = (start + extra * 4) & 0xFF;
                let y = self.oam[base];
                if in_range(y) {
                    self.extra_next.push(ObjectAttributeEntry {
                        y,
                        id: self.oam[(base + 1) & 0xFF],
                        attribute: self.oam[(base + 2) & 0xFF],
                        x: self.oam[(base + 3) & 0xFF],
                    });
                }
                extra += 1;
            }
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
        let mut sprite_zero = false;

        if (self.mask & 0x10) != 0 && ((self.mask & 0x04) != 0 || self.cycle >= 9) {
            let e = self.sprite_line[(self.cycle - 1) as usize];
            fg_pixel = e & 0x03;
            fg_palette = ((e >> 2) & 0x03) + 0x04;
            fg_priority = e & 0x10 == 0;
            sprite_zero = e & 0x20 != 0;
        }

        let (pixel, palette) = match (bg_pixel, fg_pixel) {
            (0, 0) => (0, 0),
            (0, f) => (f, fg_palette),
            (b, 0) => (b, bg_palette),
            (b, f) => {
                if fg_priority {
                    (f, fg_palette)
                } else {
                    (b, bg_palette)
                }
            }
        };
        if bg_pixel != 0
            && fg_pixel != 0
            && self.sprite_zero_hit_possible
            && sprite_zero
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
