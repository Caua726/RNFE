//! O aplicativo: laço de eventos do winit, emulação com cadência própria, entrada
//! (teclado, toque, gamepad), saves, save states, rewind, menus de toque e ajustes.
//!
//! Fluxo por plataforma:
//! - desktop: `resumed` cria a janela e a GPU (bloqueando), `about_to_wait` emula os frames
//!   devidos e agenda `WaitUntil` no próximo — sem `sleep`.
//! - web: a GPU é inicializada num futuro e chega como `UserEvent::GpuReady`; a ROM chega por
//!   `UserEvent::RomLoaded` do seletor de arquivo; o áudio só nasce após o primeiro gesto.
//! - Android: igual à web, mas o seletor é o SAF (`Launch::picker`) e `suspended` solta a GPU.
//!
//! Os menus são o modelo puro de `rnfe_frontend::menu` (testado sem GPU); aqui só se desenha.

use crate::audio::AudioOut;
use crate::gpu::GpuState;
use crate::platform::{self, Instant};
use crate::ui::{self, Theme, Ui};
use crate::{Haptic, Launch, RomPicker};
use rnfe_core::{Buttons, Nes, Storage};
use rnfe_frontend::config::{self, Config, RecentRom};
use rnfe_frontend::menu::{self, Action, ItemKind, Layout, MenuState, Screen};
use rnfe_frontend::touch::{Rect, Special};
use rnfe_frontend::{FramePacer, InputState, NTSC_FPS, Rewind, SaveManager, TouchLayout, TouchState};
use std::sync::Arc;
use std::time::Duration;
use winit::application::ApplicationHandler;
use winit::event::{ElementState, MouseButton, TouchPhase, WindowEvent};
use winit::event_loop::{ActiveEventLoop, ControlFlow, EventLoopProxy};
use winit::keyboard::{KeyCode, PhysicalKey};
use winit::window::{Fullscreen, Window, WindowAttributes, WindowId};

pub enum UserEvent {
    GpuReady(Box<Result<GpuState, String>>),
    RomLoaded {
        name: String,
        bytes: Vec<u8>,
    },
    RomLoadFailed(String),
    /// Eixos do gamepad (Android: d-pad como hat ou analógico), −1..1.
    PadAxes {
        x: f32,
        y: f32,
    },
}

/// Mensagens de erro do núcleo em linguagem de gente.
fn friendly_rom_error(name: &str, e: &rnfe_core::RomError) -> String {
    use rnfe_core::RomError;
    match e {
        RomError::BadMagic => format!("{name} não é uma ROM de NES (.nes)"),
        RomError::Truncated { .. } => format!("{name} está incompleta (arquivo truncado)"),
        RomError::UnsupportedMapper(m) => format!("{name} usa o mapper {m}, que o RNFE ainda não emula"),
        RomError::BadHeader(why) => format!("{name}: cabeçalho inválido ({why})"),
    }
}

/// Tamanho da miniatura guardada com cada save state (tela ÷ 4).
const THUMB_W: usize = rnfe_core::SCREEN_W / 4;
const THUMB_H: usize = rnfe_core::SCREEN_H / 4;

/// Velocidade do turbo.
const TURBO: f64 = 4.0;
/// Frames que "Voltar 5 s" desfaz.
const REWIND_FRAMES: u32 = 300;
/// Identificador do mouse nos menus (dedos usam o id do toque).
const MOUSE_ID: u64 = u64::MAX;

/// Um arrasto num menu é rolagem **ou** slider, decidido pelo eixo do primeiro movimento:
/// antes disso tocar num slider não muda valor nenhum.
#[derive(Clone, Copy, PartialEq)]
enum Drag {
    Undecided,
    Slider,
    Scroll,
}

/// O que muda a aparência do overlay (menus, toque, debug, toast).
#[derive(Clone, PartialEq)]
struct OverlayKey {
    screen: Screen,
    touch: Buttons,
    touch_visible: bool,
    debug: Option<(u32, u32, usize)>,
    toast: Option<String>,
    hover: Option<usize>,
    layout_gen: u64,
    pressed: Option<usize>,
    scroll: i32,
    selected: Option<usize>,
    /// Selo no canto: turbo ou rebobinando.
    badge: Option<&'static str>,
    /// Mira do Zapper (posição e gatilho) quando o acessório está ligado.
    zapper: Option<(Option<(u16, u16)>, bool)>,
}

pub struct App {
    proxy: EventLoopProxy<UserEvent>,
    window: Option<Arc<Window>>,
    gpu: Option<GpuState>,
    gpu_error: Option<String>,
    nes: Option<Box<Nes>>,
    rom_name: String,
    storage: Box<dyn Storage>,
    picker: Option<RomPicker>,
    haptic: Option<Haptic>,
    keep_screen_on: Option<crate::KeepScreenOn>,
    notify: Option<crate::Notify>,
    config: Config,
    recent: Vec<RecentRom>,
    screen: Screen,
    layout_cache: Option<Layout>,
    /// `Window::scale_factor()`: ~2,6 num celular, 1,0 num desktop comum.
    dpi: f32,
    /// Chave do último overlay desenhado: só se redesenha (e reenvia à GPU) quando muda.
    overlay_key: Option<OverlayKey>,
    overlay_size: (u32, u32),
    /// Geração do layout de menu: sobe a cada invalidação (entra na chave do overlay).
    layout_gen: u64,
    /// Rolagem da tela de menu atual (px) e item selecionado por teclado/gamepad.
    scroll: f32,
    selected: Option<usize>,
    /// Última tentativa de abrir o áudio que falhou (não insistir a cada toque).
    audio_failed_at: Option<Instant>,
    /// Quando o seletor de ROM foi aberto (para destravar se nunca responder).
    loading_since: Option<Instant>,
    confirm_remove: Option<u64>,
    gesture_exclusion: Option<crate::GestureExclusion>,
    /// Item de menu pressionado: índice, dedo (ou `MOUSE_ID`) e onde começou; a ação vale ao soltar.
    pressed: Option<(usize, u64, f32, f32)>,
    /// Ajustes mudaram e ainda não foram gravados (grava ao soltar o slider / trocar de tela).
    config_dirty: bool,
    /// Tela de states: carregar ou salvar; quais slots existem.
    states_load: bool,
    slots: [bool; 3],
    confirm_reset: bool,
    /// Frames emulados nesta ROM (tempo de jogo).
    play_frames: u64,
    save: SaveManager,
    rewind: Rewind,
    audio: Option<AudioOut>,
    pacer: FramePacer,
    start: Instant,
    input: InputState,
    touch: TouchState,
    touch_layout: TouchLayout,
    #[cfg(feature = "gamepad")]
    gilrs: Option<gilrs::Gilrs>,
    /// Botões e analógico dos gamepads, por porta (0 = jogador 1).
    pad: [Buttons; 2],
    pad_stick: [Buttons; 2],
    /// Gamepads (gilrs) já vistos, em ordem: o índice é a porta.
    #[cfg(feature = "gamepad")]
    pad_ports: Vec<usize>,
    /// Teclado do 2º jogador (IJKL, U/O, vírgula/ponto).
    input2: InputState,
    /// Erro de gravação do save já avisado (uma vez por sessão).
    save_error_shown: bool,
    /// Esc na tela inicial aguardando o 2º Esc (desktop).
    confirm_esc: bool,
    /// O áudio estava mudo no frame anterior (rewind, turbo ou volume 0).
    was_muted: bool,
    /// Mira do Zapper em pixels do NES e frames restantes de gatilho apertado.
    zapper_aim: Option<(u16, u16)>,
    zapper_hold: u8,
    /// Faixas do overlay desenhadas no último frame de jogo (limpas no próximo).
    overlay_spans: Vec<(f32, f32)>,
    /// O último overlay desenhado foi um menu (tela inteira pintada).
    overlay_menu: bool,
    /// Para onde o arrasto atual está travado (decidido no primeiro movimento).
    drag: Drag,
    /// Navegação de menu pedida pelo gamepad, consumida em `about_to_wait`.
    nav_queue: Vec<i32>,
    last_touch_buttons: Buttons,
    cursor: (f64, f64),
    ui: Ui,
    overlay: Vec<u8>,
    debug_overlay: bool,
    rewinding: bool,
    turbo: bool,
    fps_counter: u32,
    skipped_frames: u32,
    fps_timer: Instant,
    fps_display: u32,
    toast_msg: String,
    toast_until: Instant,
    loading: bool,
}

impl App {
    pub fn new(launch: Launch, proxy: EventLoopProxy<UserEvent>) -> Self {
        let Launch {
            mut nes,
            rom_name,
            mut storage,
            picker,
            haptic,
            gesture_exclusion,
            keep_screen_on,
            notify,
        } = launch;
        let config = Config::load(storage.as_ref());
        let recent = config::load_recent(storage.as_ref());
        let mut save = match &nes {
            Some(n) => SaveManager::new(n),
            None => SaveManager::none(),
        };
        if let Some(n) = nes.as_mut() {
            if save.load(n, storage.as_mut()) {
                log::info!("save carregado: {}", save.key().unwrap_or(""));
            }
        }
        let now = Instant::now();
        let screen = if nes.is_some() { Screen::Playing } else { Screen::Start };
        App {
            proxy,
            window: None,
            gpu: None,
            gpu_error: None,
            nes,
            rom_name,
            storage,
            picker,
            haptic,
            keep_screen_on,
            notify,
            touch_layout: TouchLayout::for_size_scaled(768.0, 720.0, config.touch_scale),
            config,
            recent,
            screen,
            layout_cache: None,
            dpi: 1.0,
            overlay_key: None,
            overlay_size: (0, 0),
            layout_gen: 0,
            scroll: 0.0,
            selected: None,
            audio_failed_at: None,
            loading_since: None,
            confirm_remove: None,
            gesture_exclusion,
            pressed: None,
            config_dirty: false,
            states_load: false,
            slots: [false; 3],
            confirm_reset: false,
            play_frames: 0,
            save,
            rewind: Rewind::new(Rewind::DEFAULT_CAP),
            audio: None,
            pacer: FramePacer::new(NTSC_FPS),
            start: now,
            input: InputState::new(),
            touch: TouchState::new(),
            #[cfg(feature = "gamepad")]
            gilrs: None,
            pad: [Buttons::NONE; 2],
            pad_stick: [Buttons::NONE; 2],
            #[cfg(feature = "gamepad")]
            pad_ports: Vec::new(),
            input2: InputState::new(),
            save_error_shown: false,
            confirm_esc: false,
            was_muted: false,
            zapper_aim: None,
            zapper_hold: 0,
            overlay_spans: Vec::new(),
            overlay_menu: true,
            drag: Drag::Undecided,
            nav_queue: Vec::new(),
            last_touch_buttons: Buttons::NONE,
            cursor: (0.0, 0.0),
            ui: Ui::new(),
            overlay: Vec::new(),
            debug_overlay: false,
            rewinding: false,
            turbo: false,
            fps_counter: 0,
            skipped_frames: 0,
            fps_timer: now,
            fps_display: 0,
            toast_msg: String::new(),
            toast_until: now,
            loading: false,
        }
    }

    fn now(&self) -> Duration {
        self.start.elapsed()
    }

    fn theme(&self) -> Theme {
        if self.config.high_contrast { Theme::high_contrast() } else { Theme::normal() }
    }

    fn toast(&mut self, msg: impl Into<String>) {
        self.toast_msg = msg.into();
        self.toast_until = Instant::now() + Duration::from_secs(2);
    }

    /// Erro: fica mais tempo na tela para dar para ler.
    fn toast_error(&mut self, msg: impl Into<String>) {
        self.toast_msg = msg.into();
        self.toast_until = Instant::now() + Duration::from_secs(6);
    }

    fn invalidate_layout(&mut self) {
        self.layout_cache = None;
        self.layout_gen = self.layout_gen.wrapping_add(1);
    }

    /// Limita a rolagem ao conteúdo da tela atual.
    fn clamp_scroll(&mut self) {
        let (_, h) = self.size();
        let max = self.layout().content_h - h as f32;
        self.scroll = self.scroll.clamp(0.0, max.max(0.0));
    }

    fn state_key(&self, slot: u8) -> Option<String> {
        self.nes.as_ref().map(|n| format!("state/{:016x}/{slot}.rnfs", n.cartridge().rom_hash()))
    }

    fn refresh_slots(&mut self) {
        for i in 0..3u8 {
            self.slots[i as usize] =
                self.state_key(i + 1).map(|k| self.storage.read(&k).is_some()).unwrap_or(false);
        }
    }

    fn redraw(&self) {
        if let Some(w) = &self.window {
            w.request_redraw();
        }
    }

    fn size(&self) -> (u32, u32) {
        self.gpu
            .as_ref()
            .map(|g| g.size())
            .or_else(|| self.window.as_ref().map(|w| (w.inner_size().width, w.inner_size().height)))
            .unwrap_or((768, 720))
    }

    /// Texto do selo de estado mostrado sobre o jogo (nada em jogo normal).
    fn status_badge(&self) -> Option<&'static str> {
        if !self.playing() {
            None
        } else if self.rewinding {
            Some("◀◀ voltando")
        } else if self.turbo {
            Some("TURBO 4x")
        } else {
            None
        }
    }

    fn playing(&self) -> bool {
        self.screen == Screen::Playing && self.nes.is_some()
    }

    fn refresh_touch_layout(&mut self) {
        let (w, h) = self.size();
        let img_bottom =
            self.gpu.as_ref().map(|g| g.viewport.1 + g.viewport.3).unwrap_or(w as f32 * 240.0 / 256.0);
        self.touch_layout =
            TouchLayout::for_viewport(w as f32, h as f32, img_bottom, self.config.touch_scale);
        self.update_gesture_exclusion();
    }

    /// Android: o gesto Voltar da borda não deve roubar o d-pad/A/B enquanto se joga.
    fn update_gesture_exclusion(&mut self) {
        let Some(f) = &self.gesture_exclusion else { return };
        let rects = if self.playing() && (self.touch.seen || self.config.touch_always) {
            self.touch_layout
                .gesture_exclusion()
                .map(|r| [r.x as i32, r.y as i32, (r.x + r.w) as i32, (r.y + r.h) as i32])
        } else {
            [[0; 4]; 2]
        };
        f(rects);
    }

    fn set_screen(&mut self, s: Screen) {
        if let Some(f) = &self.keep_screen_on {
            f(s == Screen::Playing);
        }
        if s == Screen::Playing && self.nes.is_none() {
            self.screen = Screen::Start;
        } else {
            self.screen = s;
        }
        if s == Screen::States {
            self.refresh_slots();
        }
        self.confirm_reset = false;
        self.confirm_remove = None;
        self.pressed = None;
        self.scroll = 0.0;
        self.selected = None;
        self.invalidate_layout();
        self.input.clear();
        self.input2.clear();
        self.touch.clear();
        self.rewinding = false;
        self.save_config();
        if self.screen == Screen::Playing {
            self.pacer.resync(self.now());
            // ROM vinda do SAF e o botão Voltar não passam por gesto de toque; só a web
            // exige um gesto para o áudio
            #[cfg(not(target_arch = "wasm32"))]
            self.ensure_audio();
            self.prime_audio();
        }
        self.update_gesture_exclusion();
        self.redraw();
    }

    /// Enche o anel com ~55 ms de silêncio: a partida/retomada absorve o jitter do laço.
    fn prime_audio(&self) {
        if let Some(a) = &self.audio {
            a.ring.clear();
            a.ring.prime(AudioOut::TARGET_QUEUE);
            // (o `clear` marca o ponto do descarte: o silêncio primado depois dele sobrevive)
        }
    }

    /// Grava os ajustes se mudaram (não a cada evento de arrasto do slider).
    fn save_config(&mut self) {
        if self.config_dirty {
            self.config_dirty = false;
            self.config.save(self.storage.as_mut());
        }
    }

    /// Aplica a paleta escolhida ao console (0 = a do núcleo).
    fn apply_palette(&mut self) {
        let idx = self.config.palette as usize;
        if let Some(nes) = self.nes.as_mut() {
            match rnfe_frontend::palettes::base(idx) {
                Some(p) => nes.set_palette(&p),
                None => nes.set_palette(&rnfe_core::ppu::default_base()),
            }
        }
    }

    fn apply_config(&mut self) {
        self.apply_palette();
        if let Some(nes) = self.nes.as_mut() {
            nes.set_sprite_limit(self.config.sprite_limit);
        }
        // região manual muda na hora (o "Automática" só vale ao abrir a ROM)
        if let Some(nes) = self.nes.as_mut() {
            let region = match self.config.region as u8 {
                1 => Some(rnfe_core::Region::Ntsc),
                2 => Some(rnfe_core::Region::Pal),
                _ => None,
            };
            if let Some(r) = region {
                if nes.region() != r {
                    nes.set_region(r);
                    self.pacer.set_fps(r.fps());
                }
            }
        }
        if let Some(g) = self.gpu.as_mut() {
            g.set_video(self.config.integer_scale, self.config.overscan, self.config.video_filter as u8);
        }
        self.refresh_touch_layout();
        self.invalidate_layout();
        self.config_dirty = true;
    }

    /// Converte um ponto da janela em pixel da tela do NES (`None` fora da imagem).
    fn nes_pixel(&self, x: f32, y: f32) -> Option<(u16, u16)> {
        let (vx, vy, vw, vh) = self.gpu.as_ref()?.viewport;
        if vw <= 0.0 || vh <= 0.0 || x < vx || y < vy || x >= vx + vw || y >= vy + vh {
            return None;
        }
        let lines = if self.config.overscan { 224.0 } else { 240.0 };
        let top = if self.config.overscan { 8.0 } else { 0.0 };
        let px = ((x - vx) / vw * rnfe_core::SCREEN_W as f32) as u16;
        let py = (top + (y - vy) / vh * lines) as u16;
        Some((px.min(255), py.min(239)))
    }

    /// Mira do Zapper: um toque/clique aponta e puxa o gatilho por alguns frames.
    fn zapper_shot(&mut self, x: f32, y: f32) {
        if let Some(p) = self.nes_pixel(x, y) {
            self.zapper_aim = Some(p);
            self.zapper_hold = 5;
        }
    }

    /// Sem GPU não há como desenhar nem a mensagem de erro: avisa por fora (Toast no Android,
    /// diálogo no desktop) para o app não ficar preto e calado.
    fn fail_gpu(&mut self, msg: String) {
        let text = format!("RNFE não conseguiu iniciar o vídeo: {msg}");
        eprintln!("{text}");
        if let Some(n) = &self.notify {
            n(&text);
        }
        #[cfg(all(not(target_arch = "wasm32"), not(target_os = "android")))]
        {
            rfd::MessageDialog::new().set_title("RNFE").set_description(&text).show();
        }
        self.gpu_error = Some(msg);
    }

    fn buzz(&self) {
        if self.config.haptics {
            if let Some(h) = &self.haptic {
                h();
            }
        }
    }

    /// Áudio nasce no primeiro gesto do usuário (exigência dos navegadores; inofensivo no desktop).
    fn ensure_audio(&mut self) {
        if self.audio.as_ref().is_some_and(|a| a.is_dead()) {
            log::warn!("áudio desconectado: recriando o stream");
            self.audio = None;
        }
        if self.audio.is_some() {
            return;
        }
        if self.audio_failed_at.is_some_and(|t| t.elapsed() < Duration::from_secs(2)) {
            return;
        }
        match AudioOut::start() {
            Some(a) => {
                if let Some(n) = self.nes.as_mut() {
                    n.set_sample_rate(a.sample_rate);
                }
                a.ring.prime(AudioOut::TARGET_QUEUE);
                self.audio = Some(a);
                self.audio_failed_at = None;
            }
            None => self.audio_failed_at = Some(Instant::now()),
        }
    }

    fn flush_save(&mut self) {
        self.save_auto_state();
        if let Some(n) = self.nes.as_mut() {
            if let Err(e) = self.save.flush(n, self.storage.as_mut()) {
                log::error!("erro ao gravar save: {e}");
            }
        }
    }

    /// Região a usar: o ajuste manual manda; em "Automática" vale o header e, como a maioria
    /// das ROMs europeias não marca nada, o nome do arquivo ((E), (Europe), PAL).
    fn pick_region(&self, nes: &Nes, name: &str) -> rnfe_core::Region {
        match self.config.region as u8 {
            1 => rnfe_core::Region::Ntsc,
            2 => rnfe_core::Region::Pal,
            _ => {
                if nes.region() == rnfe_core::Region::Pal {
                    return rnfe_core::Region::Pal;
                }
                let n = name.to_ascii_uppercase();
                let pal = ["(E)", "(EUROPE)", "(PAL)", "(EU)", "(A)", "(SW)", "(F)", "(G)", "(I)", "(S)"]
                    .iter()
                    .any(|m| n.contains(m));
                if pal { rnfe_core::Region::Pal } else { rnfe_core::Region::Ntsc }
            }
        }
    }

    fn install_nes(&mut self, mut nes: Box<Nes>, name: String) {
        self.flush_save();
        let region = self.pick_region(&nes, &name);
        nes.set_region(region);
        self.pacer.set_fps(region.fps());
        if let Some(a) = &self.audio {
            nes.set_sample_rate(a.sample_rate);
        }
        if let Some(old) = &self.nes {
            nes.debugger.trace_enabled = old.debugger.trace_enabled;
            nes.debugger.enabled = old.debugger.enabled;
        }
        self.save = SaveManager::new(&nes);
        if self.save.load(&mut nes, self.storage.as_mut()) {
            log::info!("save carregado: {}", self.save.key().unwrap_or(""));
        }
        // Continua de onde parou (auto-state de sair/suspender); state de outra versão é ignorado
        let resumed = self.storage.read(&Self::auto_key(&nes)).is_some_and(|d| nes.load_state(&d).is_ok());
        self.nes = Some(nes);
        self.rom_name = name;
        self.play_frames = 0;
        self.rewind.clear();
        self.turbo = false;
        self.pacer.set_speed(1.0);
        if let Some(w) = &self.window {
            let title: String = self.rom_name.chars().filter(|c| !c.is_control()).take(120).collect();
            w.set_title(&format!("RNFE — {title}"));
        }
        self.apply_palette();
        if let Some(nes) = self.nes.as_mut() {
            nes.set_sprite_limit(self.config.sprite_limit);
        }
        self.set_screen(Screen::Playing);
        if resumed {
            self.toast(format!("{} · continuando de onde parou", menu::display_name(&self.rom_name)));
        }
    }

    /// `store`: guardar os bytes em `roms/<hash>.nes` (ROM nova); reabrir dos recentes só
    /// reordena a lista.
    fn load_rom_bytes(&mut self, name: String, bytes: Vec<u8>, store: bool) {
        // Packs de ROM vêm zipados: abre o primeiro .nes de dentro em vez de recusar o arquivo
        let (name, bytes) = match rnfe_frontend::zip::extract_nes(&bytes) {
            Some((inner, data)) => (inner, data),
            None => (name, bytes),
        };
        match crate::load_rom_bytes(&bytes) {
            Ok(nes) => {
                let hash = nes.cartridge().rom_hash();
                self.recent =
                    config::push_recent(self.storage.as_mut(), hash, &name, store.then_some(&bytes[..]));
                self.toast(menu::display_name(&name).to_string());
                self.install_nes(nes, name);
            }
            Err(rnfe_core::RomError::BadMagic) if bytes.starts_with(rnfe_frontend::zip::MAGIC) => {
                self.toast_error(format!("{name} é um .zip sem nenhuma ROM .nes dentro"))
            }
            Err(e) => self.toast_error(friendly_rom_error(&name, &e)),
        }
    }

    /// Frame atual em PNG no Storage (`shots/<hash>-<frame>.png`).
    fn screenshot(&mut self) {
        let Some(nes) = self.nes.as_mut() else { return };
        let hash = nes.cartridge().rom_hash();
        let png = rnfe_core::png::encode(nes.framebuffer(), rnfe_core::SCREEN_W, rnfe_core::SCREEN_H);
        let key = format!("shots/{hash:016x}-{}.png", self.play_frames);
        match self.storage.write(&key, &png) {
            Ok(()) => self.toast(format!("Captura salva: {key}")),
            Err(e) => self.toast_error(format!("Não consegui salvar a captura: {e}")),
        }
    }

    /// Chave do auto-state (gravado ao sair/suspender; carregado ao abrir a ROM de novo).
    fn auto_key(nes: &Nes) -> String {
        format!("state/{:016x}/auto.rnfs", nes.cartridge().rom_hash())
    }

    fn save_auto_state(&mut self) {
        let Some(nes) = self.nes.as_ref() else { return };
        if self.play_frames == 0 {
            return; // nada jogado: não sobrescreve um auto-state anterior
        }
        let (key, data) = (Self::auto_key(nes), nes.save_state());
        if let Err(e) = self.storage.write(&key, &data) {
            log::warn!("auto-state: {e}");
        }
    }

    fn open_rom(&mut self) {
        if self.loading {
            return;
        }
        self.loading = true;
        self.loading_since = Some(Instant::now());
        self.invalidate_layout();
        match &self.picker {
            Some(p) => p(self.proxy.clone()),
            None => platform::pick_rom(self.proxy.clone()),
        }
    }

    fn open_recent(&mut self, hash: u64) {
        let name = self.recent.iter().find(|r| r.hash == hash).map(|r| r.name.clone()).unwrap_or_default();
        match self.storage.read(&RecentRom::rom_key(hash)) {
            Some(bytes) => self.load_rom_bytes(name, bytes, false),
            None => {
                self.recent = config::remove_recent(self.storage.as_mut(), hash);
                self.toast("ROM não está mais guardada");
                self.invalidate_layout();
            }
        }
    }

    fn reset(&mut self) {
        if let Some(n) = self.nes.as_mut() {
            n.reset();
            self.rewind.clear();
            self.toast("Reset");
        }
    }

    fn save_state(&mut self, slot: u8) {
        let Some(key) = self.state_key(slot) else { return };
        let data = self.nes.as_ref().map(|n| n.save_state()).unwrap_or_default();
        match self.storage.write(&key, &data) {
            Ok(()) => self.toast(format!("State salvo no slot {slot}")),
            Err(e) => {
                self.toast_error(format!("Não consegui salvar o state: {e}"));
                return; // sem state gravado, a miniatura mentiria sobre o slot
            }
        }
        // Miniatura ao lado do slot: a tela reduzida 4x, em índices de paleta (7,5 KB)
        if let Some(nes) = self.nes.as_ref() {
            let src = nes.framebuffer_indexed();
            let mut thumb = Vec::with_capacity(THUMB_W * THUMB_H * 2);
            for y in 0..THUMB_H {
                for x in 0..THUMB_W {
                    let i = (y * 4) * rnfe_core::SCREEN_W + x * 4;
                    thumb.extend_from_slice(&src[i].to_le_bytes());
                }
            }
            let _ = self.storage.write(&format!("{key}.thumb"), &thumb);
        }
    }

    /// Miniatura de um slot, em RGBA já pronto para desenhar.
    fn slot_thumb(&self, slot: u8) -> Option<Vec<u8>> {
        let key = format!("{}.thumb", self.state_key(slot)?);
        let raw = self.storage.read(&key)?;
        if raw.len() != THUMB_W * THUMB_H * 2 {
            return None;
        }
        let mut rgba = Vec::with_capacity(THUMB_W * THUMB_H * 4);
        for px in raw.chunks_exact(2) {
            let idx = u16::from_le_bytes([px[0], px[1]]) as usize;
            let pal = self.nes.as_ref().map_or(&rnfe_core::ppu::PALETTE_RGBA, |n| n.palette());
            rgba.extend_from_slice(&pal[idx & 0x1FF]);
        }
        Some(rgba)
    }

    fn load_state(&mut self, slot: u8) {
        let Some(key) = self.state_key(slot) else { return };
        let Some(data) = self.storage.read(&key) else {
            self.toast(format!("Slot {slot} vazio"));
            return;
        };
        let r = self.nes.as_mut().map(|n| n.load_state(&data));
        match r {
            Some(Ok(())) => {
                self.rewind.clear();
                self.toast(format!("State {slot} carregado"));
            }
            Some(Err(e)) => self.toast(format!("Erro: {e}")),
            None => {}
        }
    }

    fn rewind_5s(&mut self) {
        let Some(n) = self.nes.as_mut() else { return };
        let mut steps = 0;
        for _ in 0..(REWIND_FRAMES / Rewind::EVERY) {
            if !self.rewind.step_back(n) {
                break;
            }
            steps += 1;
        }
        self.toast(if steps == 0 {
            "Sem histórico".to_string()
        } else {
            format!("Voltou {:.1} s", steps as f32 * Rewind::EVERY as f32 / NTSC_FPS as f32)
        });
    }

    fn set_turbo(&mut self, on: bool) {
        if self.turbo && !on {
            self.prime_audio();
        }
        self.turbo = on;
        self.pacer.set_speed(if on { TURBO } else { 1.0 });
    }

    fn menu_state(&self) -> MenuState {
        MenuState {
            rom_name: self.rom_name.clone(),
            turbo: self.turbo,
            can_quit: cfg!(not(any(target_arch = "wasm32", target_os = "android"))),
            recent: self.recent.clone(),
            version: env!("CARGO_PKG_VERSION").into(),
            confirm_reset: self.confirm_reset,
            states_load: self.states_load,
            slots: self.slots,
            play_seconds: (self.play_frames as f64 / NTSC_FPS) as u64,
            loading: self.loading,
            confirm_remove: self.confirm_remove,
            touch_platform: cfg!(any(target_os = "android", target_arch = "wasm32")) || self.touch.seen,
            has_haptics: self.haptic.is_some(),
            can_screenshot: cfg!(not(any(target_os = "android", target_arch = "wasm32"))),
        }
    }

    fn layout(&mut self) -> Layout {
        if let Some(l) = &self.layout_cache {
            return l.clone();
        }
        let (w, h) = self.size();
        let l = menu::layout(self.screen, w as f32, h as f32, &self.config, self.dpi, &self.menu_state());
        self.layout_cache = Some(l.clone());
        l
    }

    fn act(&mut self, action: Action, el: &ActiveEventLoop) {
        if matches!(action, Action::None) {
            return;
        }
        if !matches!(action, Action::Slide(..)) {
            self.buzz();
        }
        if !matches!(action, Action::Reset) && self.confirm_reset {
            self.confirm_reset = false;
            self.invalidate_layout();
        }
        if !matches!(action, Action::RemoveRecent(_)) && self.confirm_remove.is_some() {
            self.confirm_remove = None;
            self.invalidate_layout();
        }
        match action {
            Action::Resume => self.set_screen(Screen::Playing),
            Action::OpenRom => self.open_rom(),
            Action::Recents => self.set_screen(Screen::Recents),
            Action::OpenRecent(h) => self.open_recent(h),
            Action::RemoveRecent(h) => {
                if self.confirm_remove == Some(h) {
                    self.confirm_remove = None;
                    self.recent = config::remove_recent(self.storage.as_mut(), h);
                    self.invalidate_layout();
                    if self.recent.is_empty() {
                        self.set_screen(if self.nes.is_some() { Screen::Paused } else { Screen::Start });
                    }
                } else {
                    self.confirm_remove = Some(h);
                    self.invalidate_layout();
                }
            }
            Action::Reset => {
                if self.confirm_reset {
                    self.reset();
                    self.set_screen(Screen::Playing);
                } else {
                    self.confirm_reset = true;
                    self.invalidate_layout();
                }
            }
            Action::States { load } => {
                self.states_load = load;
                self.set_screen(Screen::States);
            }
            Action::SaveSlot(n) => {
                self.save_state(n);
                self.set_screen(Screen::Playing);
            }
            Action::LoadSlot(n) => {
                self.load_state(n);
                self.set_screen(Screen::Playing);
            }
            Action::Slide(setting, f) => {
                let before = self.config.clone();
                menu::set_fraction(&mut self.config, setting, f);
                if self.config != before {
                    self.apply_config();
                }
            }
            Action::None => {}
            Action::Rewind => {
                self.rewind_5s();
                self.set_screen(Screen::Playing);
            }
            Action::ToggleTurbo => {
                let on = !self.turbo;
                self.set_turbo(on);
                self.invalidate_layout();
            }
            Action::Screenshot => {
                self.screenshot();
                self.set_screen(Screen::Playing);
            }
            Action::Settings => self.set_screen(Screen::Settings),
            Action::Controls => self.set_screen(Screen::Controls),
            Action::Back => self.set_screen(if self.nes.is_some() { Screen::Paused } else { Screen::Start }),
            Action::Quit => {
                self.flush_save();
                self.save_config();
                el.exit();
            }
            Action::Adjust(setting, delta) => {
                menu::adjust(&mut self.config, setting, delta);
                self.apply_config();
            }
        }
        self.redraw();
    }

    fn toggle_fullscreen(&self) {
        if let Some(w) = &self.window {
            if w.fullscreen().is_some() {
                w.set_fullscreen(None);
            } else {
                w.set_fullscreen(Some(Fullscreen::Borderless(None)));
            }
        }
    }

    #[cfg(feature = "gamepad")]
    fn poll_gamepad(&mut self) {
        use gilrs::{Axis, Button, EventType};
        let playing = self.playing();
        let Some(g) = self.gilrs.as_mut() else { return };
        let mut nav = Vec::new();
        let (mut pad, mut stick) = (self.pad, self.pad_stick);
        let mut ports = std::mem::take(&mut self.pad_ports);
        while let Some(ev) = g.next_event() {
            // 1º gamepad visto = jogador 1, 2º = jogador 2; os demais são ignorados
            let id: usize = ev.id.into();
            let port = match ports.iter().position(|&p| p == id) {
                Some(p) => p,
                None if ports.len() < 2 => {
                    ports.push(id);
                    ports.len() - 1
                }
                None => continue,
            };
            let map = |b: Button| match b {
                Button::South | Button::East => Buttons::A,
                Button::West | Button::North => Buttons::B,
                Button::Start => Buttons::START,
                Button::Select => Buttons::SELECT,
                Button::DPadUp => Buttons::UP,
                Button::DPadDown => Buttons::DOWN,
                Button::DPadLeft => Buttons::LEFT,
                Button::DPadRight => Buttons::RIGHT,
                _ => Buttons::NONE,
            };
            if !playing {
                match ev.event {
                    EventType::ButtonPressed(Button::DPadUp, _) => nav.push(-1),
                    EventType::ButtonPressed(Button::DPadDown, _) => nav.push(1),
                    EventType::ButtonPressed(Button::DPadLeft, _) => nav.push(-2),
                    EventType::ButtonPressed(Button::DPadRight, _) => nav.push(2),
                    EventType::ButtonPressed(Button::South | Button::Start, _) => nav.push(0),
                    EventType::ButtonPressed(Button::East | Button::Select, _) => nav.push(3),
                    EventType::ButtonPressed(Button::Mode, _) => nav.push(4),
                    _ => {}
                }
                continue;
            }
            match ev.event {
                EventType::ButtonPressed(Button::Mode, _) => nav.push(4),
                EventType::ButtonPressed(b, _) => pad[port] |= map(b),
                EventType::ButtonReleased(b, _) => pad[port] = pad[port].with(map(b), false),
                EventType::AxisChanged(axis, v, _) => {
                    let (neg, pos) = match axis {
                        Axis::LeftStickX => (Buttons::LEFT, Buttons::RIGHT),
                        Axis::LeftStickY => (Buttons::DOWN, Buttons::UP),
                        _ => continue,
                    };
                    stick[port] = stick[port].with(neg, v < -0.5).with(pos, v > 0.5);
                }
                _ => {}
            }
        }
        self.pad_ports = ports;
        self.pad = pad;
        self.pad_stick = stick;
        self.nav_queue.extend(nav);
    }

    /// Comandos de navegação vindos do gamepad (−1/1 cima/baixo, ±2 esquerda/direita,
    /// 0 ativar, 3 voltar, 4 menu), executados fora do `poll` para poder emprestar `self`.
    fn drain_nav(&mut self, el: &ActiveEventLoop) {
        let q = std::mem::take(&mut self.nav_queue);
        for cmd in q {
            match cmd {
                -1 | 1 => self.nav(cmd, el),
                -2 => self.nav_activate(-1, el),
                2 => self.nav_activate(1, el),
                0 => self.nav_activate(0, el),
                3 => {
                    if self.screen != Screen::Start {
                        self.act(Action::Back, el);
                    }
                }
                _ => {
                    if self.playing() {
                        self.set_screen(Screen::Paused);
                    } else if self.nes.is_some() {
                        self.set_screen(Screen::Playing);
                    }
                }
            }
        }
    }

    /// Emula os frames devidos desde a última chamada.
    fn advance(&mut self) {
        #[cfg(feature = "gamepad")]
        self.poll_gamepad();
        if !self.playing() || self.gpu.is_none() {
            return;
        }
        let now = self.now();
        let due = self.pacer.frames_due(now);
        if due == 0 {
            return;
        }
        // Frame skip adaptativo: quando o laço atrasa (due > 1), só o último frame emulado
        // vai para a tela — o redraw abaixo é um por chamada, não um por frame.
        self.skipped_frames += due.saturating_sub(1);
        let buttons = self.input.current(now) | self.touch.buttons() | self.pad[0] | self.pad_stick[0];
        let buttons2 = self.input2.current(now) | self.pad[1] | self.pad_stick[1];
        // START+SELECT juntos (gamepad, toque ou teclado) abrem o menu em qualquer plataforma
        let combo = Buttons::START.0 | Buttons::SELECT.0;
        if buttons.0 & combo == combo {
            self.input.clear();
            self.input2.clear();
            self.touch.clear();
            self.pad = [Buttons::NONE; 2];
            self.set_screen(Screen::Paused);
            return;
        }
        let Some(nes) = self.nes.as_mut() else { return };
        if self.config.zapper {
            let aim = self.zapper_aim.unwrap_or((0, 0));
            nes.set_zapper(Some((aim.0, aim.1, self.zapper_hold > 0)));
            // um decremento por frame emulado (com turbo, `due` é maior que 1)
            self.zapper_hold = self.zapper_hold.saturating_sub(due.min(255) as u8);
        } else if nes.has_zapper() {
            nes.set_zapper(None);
        }
        nes.set_controller(0, buttons);
        nes.set_controller(1, buttons2);
        let mut save_err: Option<String> = None;
        for _ in 0..due {
            if self.rewinding {
                if !self.rewind.step_back(nes) {
                    break;
                }
                nes.run_frame(); // mostra o frame do passado (o state não guarda a tela)
                continue;
            }
            nes.run_frame();
            self.play_frames += 1;
            self.rewind.record(nes);
            if let Err(e) = self.save.tick(nes, self.storage.as_mut()) {
                log::error!("erro ao gravar save: {e}");
                save_err = Some(e.to_string());
            }
            self.fps_counter += 1;
        }
        let muted = self.rewinding || self.turbo || self.config.volume <= 0.0;
        if self.was_muted && !muted {
            // voltar do rewind/turbo com a fila vazia dava engasgo: reenche com silêncio
            if let Some(a) = &self.audio {
                a.ring.clear();
                a.ring.prime(AudioOut::TARGET_QUEUE);
            }
        }
        self.was_muted = muted;
        let volume = self.config.volume;
        match &self.audio {
            Some(a) if !muted => {
                nes.take_audio(|samples| {
                    if volume < 1.0 {
                        for s in samples.iter_mut() {
                            *s *= volume;
                        }
                    }
                    a.ring.push_capped(samples, AudioOut::TARGET_QUEUE * 2);
                });
                // Controle fino da taxa: o relógio do DAC e o do pacer divergem um pouco.
                // Fila abaixo do alvo = produzir MAIS (taxa maior); acima = produzir menos.
                // (com o sinal trocado a realimentação vira positiva e a fila esvazia sempre)
                let err = a.ring.len() as f32 / AudioOut::TARGET_QUEUE as f32 - 1.0;
                let adj = if err.abs() < 0.05 { 1.0 } else { 1.0 - 0.005 * err.clamp(-1.0, 1.0) };
                nes.set_sample_rate((a.sample_rate as f32 * adj) as u32);
            }
            _ => nes.take_audio(|_| {}),
        }
        if let Some(e) = save_err.filter(|_| !self.save_error_shown) {
            self.save_error_shown = true;
            self.toast_error(format!("Não consegui gravar o save: {e}"));
        }
        if self.fps_timer.elapsed() >= Duration::from_secs(1) {
            self.fps_display = self.fps_counter;
            self.fps_counter = 0;
            self.fps_timer = Instant::now();
        }
        self.redraw();
    }

    fn draw_menu(&mut self, w: u32, h: u32) {
        let theme = self.theme();
        let layout = self.layout();
        let mut fb = std::mem::take(&mut self.overlay);
        ui::clear(&mut fb, if self.screen == Screen::Paused { theme.bg } else { theme.panel });
        let s = layout.ui_scale;
        let scroll = self.scroll;
        let (mx, my) = (self.cursor.0 as f32, self.cursor.1 as f32 + scroll);
        let title_size = layout.title_size;
        let title_y = layout.title_y as i32;
        let mut subtitle_y = layout.subtitle_y as i32;
        let cartridge = self.screen == Screen::Start;
        if cartridge {
            // marca: um cartucho estilizado atrás do título, do tamanho do texto
            let tw = self.ui.text_width(&layout.title, title_size) as f32;
            let cw = tw + 40.0 * s;
            let ch = title_size * 1.35;
            let cx = (w as f32 - cw) / 2.0;
            let cy = title_y as f32 - title_size * 0.12;
            ui::fill_round_rect(
                &mut fb,
                w,
                h,
                &Rect { x: cx, y: cy + 4.0 * s, w: cw, h: ch },
                10.0 * s,
                [0, 0, 0, 110],
            );
            ui::fill_round_rect(&mut fb, w, h, &Rect { x: cx, y: cy, w: cw, h: ch }, 10.0 * s, theme.accent);
            subtitle_y = (cy + ch + 10.0 * s) as i32;
        }
        let sub_size = layout.subtitle_size;
        let title_color = if self.screen == Screen::Start { theme.on_accent } else { theme.text };
        self.ui.draw_text_centered(&mut fb, w, h, &layout.title, title_size, title_y, title_color);
        self.ui.draw_text_centered(&mut fb, w, h, &layout.subtitle, sub_size, subtitle_y, theme.dim);
        if let Some(e) = &self.gpu_error {
            let msg = e.clone();
            self.ui.draw_text_centered(
                &mut fb,
                w,
                h,
                &msg,
                (13.0 * s).max(12.0),
                subtitle_y + (22.0 * s) as i32,
                theme.accent_hot,
            );
        }
        let pressed_idx = self.pressed.map(|p| p.0);
        for (i, item) in layout.items.iter().enumerate() {
            let hot = pressed_idx == Some(i)
                || self.selected == Some(i)
                || (pressed_idx.is_none() && self.selected.is_none() && item.rect.contains(mx, my));
            let mut it = item.clone();
            it.rect.y -= scroll;
            if it.rect.y + it.rect.h < 0.0 || it.rect.y > h as f32 {
                continue;
            }
            self.ui.draw_item(&mut fb, w, h, &it, &layout, hot, &theme);
            // Miniatura do slot à direita da linha (só na tela de states, slots preenchidos)
            if self.screen == Screen::States {
                if let ItemKind::Slot { filled: true } = it.kind {
                    let slot = match it.action {
                        Action::SaveSlot(n) | Action::LoadSlot(n) => n,
                        _ => continue,
                    };
                    if let Some(rgba) = self.slot_thumb(slot) {
                        let th = it.rect.h * 0.78;
                        let tw = th * THUMB_W as f32 / THUMB_H as f32;
                        let dst = Rect {
                            x: it.rect.x + it.rect.w - tw - layout.radius,
                            y: it.rect.y + (it.rect.h - th) / 2.0,
                            w: tw,
                            h: th,
                        };
                        ui::draw_image(&mut fb, w, h, &dst, &rgba, THUMB_W as u32, THUMB_H as u32);
                    }
                }
            }
        }
        if scroll > 0.0 {
            // conteúdo rolado passa por baixo do cabeçalho: faixa opaca + título de novo
            let base = if self.screen == Screen::Paused { theme.bg } else { theme.panel };
            let band = [base[0], base[1], base[2], 255];
            ui::fill_rect(&mut fb, w, h, 0, 0, w as i32, layout.header_h as i32, band);
            let tc = if cartridge { theme.on_accent } else { theme.text };
            self.ui.draw_text_centered(&mut fb, w, h, &layout.title, title_size, title_y, tc);
            self.ui.draw_text_centered(&mut fb, w, h, &layout.subtitle, sub_size, subtitle_y, theme.dim);
        }
        if layout.content_h > h as f32 {
            // barra de rolagem fina à direita, proporcional ao conteúdo
            let track_h = h as f32 - layout.header_h;
            let bar_h = (track_h * track_h / layout.content_h).max(24.0 * s);
            let max_scroll = (layout.content_h - h as f32).max(1.0);
            let by = layout.header_h + (track_h - bar_h) * (scroll / max_scroll).clamp(0.0, 1.0);
            let bx = w as f32 - 6.0 * s;
            let bar = Rect { x: bx, y: by, w: 4.0 * s, h: bar_h };
            ui::fill_round_rect(&mut fb, w, h, &bar, 2.0 * s, theme.border);
        }
        if self.screen == Screen::Recents && layout.items.len() == 1 {
            let msg = "Abra uma ROM: ela aparece aqui";
            self.ui.draw_text_centered(&mut fb, w, h, msg, sub_size, (h as f32 * 0.5) as i32, theme.dim);
        }
        if self.screen == Screen::Start {
            let hint = if self.touch.seen || cfg!(target_os = "android") {
                "toque em Abrir ROM"
            } else {
                "O abre uma ROM · arraste um .nes na janela · setas e Enter navegam"
            };
            self.ui.draw_text_centered(
                &mut fb,
                w,
                h,
                hint,
                (13.0 * s).max(12.0),
                h as i32 - (40.0 * s) as i32,
                theme.dim,
            );
        }
        self.overlay = fb;
    }

    /// Monta o overlay (menus, toque, debug, toast) e desenha o frame.
    fn draw(&mut self) {
        let Some((w, h)) = self.gpu.as_ref().map(|g| g.size()) else { return };
        let size = (w * h * 4) as usize;
        self.overlay.resize(size, 0);
        let show_toast = Instant::now() < self.toast_until;
        let touch_visible = self.touch.seen || self.config.touch_always;
        let hover = if self.playing() {
            None
        } else {
            let l = self.layout();
            menu::index_at(&l, self.cursor.0 as f32, self.cursor.1 as f32 + self.scroll)
        };
        let key = OverlayKey {
            screen: self.screen,
            touch: self.touch.buttons(),
            touch_visible,
            debug: self.debug_overlay.then(|| (self.fps_display, self.skipped_frames, self.rewind.len())),
            toast: show_toast.then(|| self.toast_msg.clone()),
            hover,
            layout_gen: self.layout_gen,
            pressed: self.pressed.map(|p| p.0),
            scroll: self.scroll as i32,
            selected: self.selected,
            badge: self.status_badge(),
            zapper: self.config.zapper.then_some((self.zapper_aim, self.zapper_hold > 0)),
        };
        let resized = self.overlay_size != (w, h);
        let dirty = self.overlay_key.as_ref() != Some(&key) || resized;
        let has_overlay = self.screen != Screen::Playing
            || self.nes.is_none()
            || touch_visible
            || self.debug_overlay
            || show_toast
            || self.status_badge().is_some()
            || (self.config.zapper && self.zapper_aim.is_some());
        if !dirty {
            let Some(gpu) = self.gpu.as_mut() else { return };
            let fb = self.nes.as_mut().map(|n| n.framebuffer());
            if !gpu.render(fb, has_overlay, None) {
                self.redraw();
            }
            return;
        }
        self.overlay_key = Some(key);
        self.overlay_size = (w, h);
        let theme = self.theme();
        let s = menu::ui_scale(w as f32, h as f32, &self.config, self.dpi);

        if self.screen != Screen::Playing || self.nes.is_none() {
            self.draw_menu(w, h);
            self.overlay_menu = true;
            self.overlay_spans.clear();
        } else {
            // Jogando, o overlay só tem controles de toque, selo, toast e debug: limpar a tela
            // toda (10 MB num celular) a cada aperto de botão era o grosso do custo.
            let mut spans: Vec<(f32, f32)> = Vec::new();
            if touch_visible {
                spans.push(self.touch_layout.vertical_span());
            }
            if self.status_badge().is_some() {
                let vy = self.gpu.as_ref().map_or(0.0, |g| g.viewport.1);
                spans.push((vy, vy + 40.0 * s));
            }
            if self.config.zapper {
                if let (Some((_, zy)), Some(g)) = (self.zapper_aim, self.gpu.as_ref()) {
                    let (_, vy, _, vh) = g.viewport;
                    let lines = if self.config.overscan { 224.0 } else { 240.0 };
                    let top = if self.config.overscan { 8.0 } else { 0.0 };
                    let cy = vy + (zy as f32 - top + 0.5) / lines * vh;
                    spans.push((cy - 16.0 * s, cy + 16.0 * s));
                }
            }
            if show_toast {
                // o toast jogando sai logo abaixo do botão MENU, não no rodapé
                let y = self.touch_layout.menu.y + self.touch_layout.menu.h + 12.0 * s;
                spans.push((y - 24.0 * s, y + 80.0 * s));
                spans.push((h as f32 - 90.0 * s, h as f32));
            }
            if self.debug_overlay {
                spans.push((0.0, 130.0 * s));
            }
            // Também limpa o que foi desenhado no frame anterior (toast que sumiu, controles que
            // mudaram de lugar); vindo de um menu, a tela inteira estava pintada.
            let mut to_clear = spans.clone();
            if self.overlay_menu || resized {
                // vindo de um menu (tela toda pintada) ou depois de girar/redimensionar (o
                // buffer redimensionado guarda o conteúdo antigo com o passo de linha errado)
                to_clear.push((0.0, h as f32));
                self.overlay_menu = false;
            } else {
                to_clear.extend(self.overlay_spans.iter().copied());
            }
            self.overlay_spans = spans;
            let row = (w * 4) as usize;
            for (y0, y1) in to_clear {
                let a = (y0.max(0.0) as u32).min(h) as usize * row;
                let b = (y1.max(0.0).ceil() as u32).min(h) as usize * row;
                if b > a {
                    self.overlay[a..b].fill(0);
                }
            }
            if touch_visible {
                let pressed = self.touch.buttons();
                let (op, hc) = (self.config.touch_opacity, self.config.high_contrast);
                let layout = self.touch_layout.clone();
                self.ui.draw_touch_controls(&mut self.overlay, w, h, &layout, pressed, op, &theme, hc);
            }
            // Mira do Zapper no último ponto apontado
            if self.config.zapper {
                if let (Some((zx, zy)), Some(g)) = (self.zapper_aim, self.gpu.as_ref()) {
                    let (vx, vy, vw, vh) = g.viewport;
                    let lines = if self.config.overscan { 224.0 } else { 240.0 };
                    let top = if self.config.overscan { 8.0 } else { 0.0 };
                    let cx = vx + (zx as f32 + 0.5) / rnfe_core::SCREEN_W as f32 * vw;
                    let cy = vy + (zy as f32 - top + 0.5) / lines * vh;
                    let r = 14.0 * s;
                    let hot = self.zapper_hold > 0;
                    let col = if hot { [255, 90, 60, 255] } else { [255, 255, 255, 190] };
                    ui::fill_rect(
                        &mut self.overlay,
                        w,
                        h,
                        (cx - r) as i32,
                        cy as i32,
                        (2.0 * r) as i32,
                        2,
                        col,
                    );
                    ui::fill_rect(
                        &mut self.overlay,
                        w,
                        h,
                        cx as i32,
                        (cy - r) as i32,
                        2,
                        (2.0 * r) as i32,
                        col,
                    );
                }
            }
            if let Some(text) = self.status_badge() {
                // canto superior direito da imagem, para não cobrir o HUD do jogo
                let size = 14.0 * s;
                let tw = self.ui.text_width(text, size) as f32;
                let pad = size * 0.5;
                let (vx, vy, vw, _) = self.gpu.as_ref().map_or((0.0, 0.0, w as f32, 0.0), |g| g.viewport);
                let r = Rect { x: vx + vw - tw - pad * 3.0, y: vy + pad, w: tw + pad * 2.0, h: size + pad };
                ui::fill_round_rect(&mut self.overlay, w, h, &r, size * 0.35, [0, 0, 0, 170]);
                let ty = self.ui.center_y(size, r.y + r.h * 0.5);
                self.ui.draw_text(
                    &mut self.overlay,
                    w,
                    h,
                    text,
                    size,
                    (r.x + pad) as i32,
                    ty,
                    theme.accent_hot,
                );
            }
            if self.debug_overlay {
                if let Some(nes) = self.nes.as_ref() {
                    let sz = 13.0 * s;
                    let mut y = 8;
                    ui::fill_rect(
                        &mut self.overlay,
                        w,
                        h,
                        4,
                        4,
                        (460.0 * s) as i32,
                        (80.0 * s) as i32,
                        [0, 0, 0, 160],
                    );
                    let l1 = format!(
                        "FPS {}  pulados {}  underruns {}  fila {} smp  rewind {} states / {} KB",
                        self.fps_display,
                        self.skipped_frames,
                        self.audio.as_ref().map(|a| a.ring.underruns()).unwrap_or(0),
                        self.audio.as_ref().map(|a| a.ring.len()).unwrap_or(0),
                        self.rewind.len(),
                        self.rewind.bytes() / 1024
                    );
                    self.ui.draw_text(&mut self.overlay, w, h, &l1, sz, 12, y, [0, 255, 80, 255]);
                    y += (18.0 * s) as i32;
                    let l2 = format!(
                        "PC:{:04X} A:{:02X} X:{:02X} Y:{:02X} SP:{:02X} P:{:02X}  ciclos {}",
                        nes.cpu.pc,
                        nes.cpu.a,
                        nes.cpu.x,
                        nes.cpu.y,
                        nes.cpu.stkp,
                        nes.cpu.status,
                        nes.cpu_cycles()
                    );
                    self.ui.draw_text(&mut self.overlay, w, h, &l2, sz, 12, y, [200, 200, 200, 255]);
                    y += (18.0 * s) as i32;
                    let l3 = format!(
                        "SL:{} CYC:{} CTRL:{:02X} MASK:{:02X} STAT:{:02X}  {}",
                        nes.bus.ppu.scanline,
                        nes.bus.ppu.cycle,
                        nes.bus.ppu.control,
                        nes.bus.ppu.mask,
                        nes.bus.ppu.status,
                        nes.cartridge().describe()
                    );
                    self.ui.draw_text(&mut self.overlay, w, h, &l3, sz, 12, y, [200, 200, 200, 255]);
                    y += (18.0 * s) as i32;
                    self.ui.draw_text(
                        &mut self.overlay,
                        w,
                        h,
                        "F3 debug  F4 cobertura  F5/F7 state  F6 diag  F9 trace  Bksp rewind  Espaço turbo",
                        sz,
                        12,
                        y,
                        [160, 160, 160, 255],
                    );
                }
            }
        }
        if show_toast {
            let msg = self.toast_msg.clone();
            let y = if self.playing() && self.touch_layout.portrait {
                Some(self.touch_layout.menu.y + self.touch_layout.menu.h + 12.0 * s)
            } else {
                None
            };
            self.ui.draw_toast(&mut self.overlay, w, h, &msg, 16.0 * s, y);
        }
        let Some(gpu) = self.gpu.as_mut() else { return };
        let fb = self.nes.as_mut().map(|n| n.framebuffer());
        let ov = if has_overlay { Some(self.overlay.as_slice()) } else { None };
        if !gpu.render(fb, has_overlay, ov) {
            self.redraw();
        }
    }

    /// Move a seleção de menu (teclado/gamepad) e rola para deixá-la visível.
    fn nav(&mut self, dir: i32, el: &ActiveEventLoop) {
        let layout = self.layout();
        self.selected = menu::next_selectable(&layout, self.selected, dir);
        if let Some(i) = self.selected {
            let r = layout.items[i].rect;
            let (_, h) = self.size();
            if r.y - self.scroll < 0.0 {
                self.scroll = r.y - 10.0;
            } else if r.y + r.h - self.scroll > h as f32 {
                self.scroll = r.y + r.h - h as f32 + 10.0;
            }
            self.clamp_scroll();
        }
        let _ = el;
        self.redraw();
    }

    fn nav_activate(&mut self, dir: i32, el: &ActiveEventLoop) {
        let layout = self.layout();
        // Sem seleção (mouse, ou tela recém-aberta): vale o primeiro item, que é o principal
        let Some(i) = self.selected.or_else(|| menu::next_selectable(&layout, None, 1)) else { return };
        self.selected = Some(i);
        if let Some(a) = menu::activate(&layout, i, dir) {
            self.act(a, el);
        }
    }

    fn handle_key(&mut self, key: KeyCode, pressed: bool, el: &ActiveEventLoop) {
        if !self.playing() && pressed {
            match key {
                KeyCode::ArrowUp => return self.nav(-1, el),
                KeyCode::ArrowDown | KeyCode::Tab => return self.nav(1, el),
                KeyCode::ArrowLeft => return self.nav_activate(-1, el),
                KeyCode::ArrowRight => return self.nav_activate(1, el),
                KeyCode::Enter | KeyCode::Space | KeyCode::KeyZ => return self.nav_activate(0, el),
                KeyCode::KeyX | KeyCode::Backspace if self.screen != Screen::Start => {
                    return self.act(Action::Back, el);
                }
                _ => {}
            }
        }
        if self.playing() {
            let bit = match key {
                KeyCode::KeyZ => Some(Buttons::A),
                KeyCode::KeyX => Some(Buttons::B),
                KeyCode::Tab | KeyCode::ShiftRight => Some(Buttons::SELECT),
                KeyCode::Enter => Some(Buttons::START),
                KeyCode::ArrowUp => Some(Buttons::UP),
                KeyCode::ArrowDown => Some(Buttons::DOWN),
                KeyCode::ArrowLeft => Some(Buttons::LEFT),
                KeyCode::ArrowRight => Some(Buttons::RIGHT),
                KeyCode::KeyW => Some(Buttons::UP),
                KeyCode::KeyS => Some(Buttons::DOWN),
                KeyCode::KeyA => Some(Buttons::LEFT),
                KeyCode::KeyD => Some(Buttons::RIGHT),
                _ => None,
            };
            if let Some(b) = bit {
                self.input.set(b, pressed);
            }
            // Jogador 2 no mesmo teclado
            let bit2 = match key {
                KeyCode::KeyO => Some(Buttons::A),
                KeyCode::KeyU => Some(Buttons::B),
                KeyCode::Comma => Some(Buttons::SELECT),
                KeyCode::Period => Some(Buttons::START),
                KeyCode::KeyI => Some(Buttons::UP),
                KeyCode::KeyK => Some(Buttons::DOWN),
                KeyCode::KeyJ => Some(Buttons::LEFT),
                KeyCode::KeyL => Some(Buttons::RIGHT),
                _ => None,
            };
            if let Some(b) = bit2 {
                self.input2.set(b, pressed);
            }
            match key {
                KeyCode::Backspace => self.rewinding = pressed,
                KeyCode::Space => self.set_turbo(pressed),
                _ => {}
            }
        }
        if !pressed && key == KeyCode::Space && self.turbo && !self.playing() {
            self.set_turbo(false); // soltou o turbo com o jogo pausado
        }
        if !pressed {
            return;
        }
        match key {
            KeyCode::Escape => match self.screen {
                Screen::Playing => self.set_screen(Screen::Paused),
                Screen::Paused => self.set_screen(Screen::Playing),
                Screen::Settings | Screen::Recents | Screen::States | Screen::Controls => {
                    self.act(Action::Back, el)
                }
                Screen::Start => {
                    if cfg!(not(any(target_arch = "wasm32", target_os = "android"))) || self.picker.is_some()
                    {
                        if self.confirm_esc && Instant::now() < self.toast_until {
                            self.flush_save();
                            self.save_config();
                            el.exit();
                        } else {
                            self.confirm_esc = true;
                            self.toast("Esc de novo para sair");
                        }
                    }
                }
            },
            KeyCode::KeyO if !self.playing() => self.open_rom(),
            KeyCode::KeyR => {
                if self.playing() {
                    // só vale enquanto o aviso está na tela: um R perdido não arma um reset eterno
                    if self.confirm_reset && Instant::now() < self.toast_until {
                        self.confirm_reset = false;
                        self.reset();
                    } else {
                        self.confirm_reset = true;
                        self.toast("R de novo para confirmar o reset");
                    }
                }
            }
            KeyCode::F3 => {
                self.debug_overlay = !self.debug_overlay;
                self.toast(if self.debug_overlay { "Debug ON" } else { "Debug OFF" });
            }
            KeyCode::F4 => {
                if let Some(n) = &self.nes {
                    log::info!("{}", n.debugger.coverage_report());
                    self.toast("Cobertura -> log");
                }
            }
            KeyCode::F5 => self.save_state(1),
            KeyCode::F12 if cfg!(not(any(target_os = "android", target_arch = "wasm32"))) => {
                self.screenshot()
            }
            KeyCode::F7 => self.load_state(1),
            KeyCode::F6 => {
                if let Some(n) = &self.nes {
                    log::info!("{}", rnfe_core::diagnostic::diagnostic_report(&n.cpu, &n.bus));
                    self.toast("Diagnóstico -> log");
                }
            }
            KeyCode::F9 => {
                if let Some(n) = self.nes.as_mut() {
                    n.debugger.trace_enabled = !n.debugger.trace_enabled;
                    n.debugger.enabled = n.debugger.trace_enabled;
                    if !n.debugger.trace_enabled {
                        for line in n.debugger.trace_log.iter().rev().take(20).rev() {
                            log::info!("{line}");
                        }
                    }
                    let on = n.debugger.trace_enabled;
                    self.toast(if on { "Trace ON" } else { "Trace OFF -> log" });
                }
            }
            KeyCode::F11 => self.toggle_fullscreen(),
            _ => {}
        }
    }

    /// Começo de um toque/clique num menu: marca o item pressionado (a ação vale ao soltar);
    /// sliders reagem já no toque.
    fn menu_press(&mut self, id: u64, x: f32, y: f32, _el: &ActiveEventLoop) {
        if self.pressed.is_some() {
            return; // já há um dedo num item: o segundo é ignorado
        }
        self.selected = None;
        let layout = self.layout();
        let y = y + self.scroll;
        // `usize::MAX` = dedo fora de qualquer item: só pode virar rolagem
        self.pressed = Some(menu::index_at(&layout, x, y).map_or((usize::MAX, id, x, y), |i| (i, id, x, y)));
        self.drag = Drag::Undecided;
    }

    /// Arrasto com o dedo/mouse pressionado: o eixo do primeiro movimento decide entre mexer no
    /// slider (horizontal) e rolar a lista (vertical).
    fn menu_drag(&mut self, id: u64, x: f32, y: f32, el: &ActiveEventLoop) {
        let Some((i, pid, x0, y0)) = self.pressed else { return };
        if pid != id {
            return;
        }
        let layout = self.layout();
        let (_, h) = self.size();
        let scrollable = layout.content_h > h as f32;
        let on_slider = layout.items.get(i).is_some_and(|it| matches!(it.kind, ItemKind::Slider { .. }));
        let (dx, dy) = (x - x0, (y + self.scroll) - y0);
        let threshold = 12.0 * layout.ui_scale;
        if self.drag == Drag::Undecided {
            if on_slider && dx.abs() > threshold && dx.abs() >= dy.abs() {
                self.drag = Drag::Slider;
            } else if dy.abs() > threshold && (scrollable || !on_slider) {
                self.drag = Drag::Scroll;
                if scrollable {
                    self.pressed = Some((usize::MAX, id, x0, y0));
                }
            }
        }
        match self.drag {
            Drag::Slider => {
                if let Some(a) = menu::slide(&layout, i, x) {
                    self.act(a, el);
                }
            }
            Drag::Scroll if scrollable => {
                // o ponto do conteúdo sob o dedo continua sob o dedo
                self.scroll = y0 - y;
                self.clamp_scroll();
                self.redraw();
            }
            _ => {}
        }
    }

    /// Soltou: dispara a ação se ainda está sobre o mesmo item (senão, cancela).
    fn menu_release(&mut self, id: u64, x: f32, y: f32, el: &ActiveEventLoop) {
        if self.pressed.is_some_and(|p| p.1 != id) {
            return; // soltou outro dedo
        }
        let Some((i, ..)) = self.pressed.take() else { return };
        if i == usize::MAX {
            self.redraw();
            return;
        }
        let y = y + self.scroll;
        let layout = self.layout();
        let drag = std::mem::replace(&mut self.drag, Drag::Undecided);
        if matches!(layout.items.get(i).map(|it| &it.kind), Some(ItemKind::Slider { .. })) {
            // toque sem arrasto: só vale se caiu na trilha (menu::hit devolve None fora dela)
            if drag == Drag::Undecided {
                if let Some(a) = menu::hit(&layout, x, y) {
                    self.act(a, el);
                }
            }
            self.invalidate_layout();
            self.save_config();
            self.redraw();
            return;
        }
        if drag == Drag::Scroll {
            self.redraw();
            return;
        }
        if menu::index_at(&layout, x, y) == Some(i) {
            let a = layout.items[i].action.clone();
            self.act(a, el);
        }
        self.redraw();
    }
}

impl ApplicationHandler<UserEvent> for App {
    fn resumed(&mut self, el: &ActiveEventLoop) {
        if self.window.is_some() {
            return;
        }
        let title =
            if self.rom_name.is_empty() { "RNFE".to_string() } else { format!("RNFE — {}", self.rom_name) };
        #[allow(unused_mut)]
        let mut attrs = WindowAttributes::default()
            .with_title(title)
            .with_inner_size(winit::dpi::PhysicalSize::new(878, 720));
        #[cfg(target_arch = "wasm32")]
        {
            use wasm_bindgen::JsCast;
            use winit::platform::web::WindowAttributesExtWebSys;
            let canvas = web_sys::window()
                .and_then(|w| w.document())
                .and_then(|d| d.get_element_by_id("rnfe"))
                .and_then(|e| e.dyn_into::<web_sys::HtmlCanvasElement>().ok());
            attrs = attrs.with_canvas(canvas).with_prevent_default(true).with_focusable(true);
        }
        let window = match el.create_window(attrs) {
            Ok(w) => Arc::new(w),
            Err(e) => {
                // sem janela não há o que fazer, mas morrer com panic/abort não ajuda ninguém
                log::error!("janela: {e}");
                self.gpu_error = Some(format!("não consegui abrir a janela: {e}"));
                el.exit();
                return;
            }
        };
        // Na web o winit grava a largura/altura em px no style do canvas, o que anula o
        // `100vw/100dvh` da página: tira o style inline e deixa o CSS mandar.
        #[cfg(target_arch = "wasm32")]
        {
            use wasm_bindgen::JsCast;
            if let Some(c) = web_sys::window()
                .and_then(|w| w.document())
                .and_then(|d| d.get_element_by_id("rnfe"))
                .and_then(|e| e.dyn_into::<web_sys::HtmlElement>().ok())
            {
                let _ = c.style().remove_property("width");
                let _ = c.style().remove_property("height");
            }
        }
        self.window = Some(window.clone());
        self.dpi = window.scale_factor() as f32;
        log::info!("janela {}x{} @{:.2}", window.inner_size().width, window.inner_size().height, self.dpi);
        self.invalidate_layout();
        self.overlay_key = None;
        #[cfg(feature = "gamepad")]
        {
            self.gilrs = gilrs::Gilrs::new().ok();
        }
        #[cfg(not(target_arch = "wasm32"))]
        {
            match pollster::block_on(GpuState::new(window.clone())) {
                Ok(mut g) => {
                    g.set_video(
                        self.config.integer_scale,
                        self.config.overscan,
                        self.config.video_filter as u8,
                    );
                    self.gpu = Some(g);
                }
                Err(e) => {
                    log::error!("GPU: {e}");
                    self.fail_gpu(e);
                }
            }
            self.refresh_touch_layout();
        }
        #[cfg(target_arch = "wasm32")]
        {
            let proxy = self.proxy.clone();
            platform::spawn_with(move || async move {
                let r = GpuState::new(window).await;
                let _ = proxy.send_event(UserEvent::GpuReady(Box::new(r)));
            });
        }
        self.pacer.resync(self.now());
        self.redraw();
    }

    fn user_event(&mut self, _el: &ActiveEventLoop, ev: UserEvent) {
        let _el: &ActiveEventLoop = _el;
        match ev {
            UserEvent::GpuReady(r) => match *r {
                Ok(mut g) => {
                    g.set_video(
                        self.config.integer_scale,
                        self.config.overscan,
                        self.config.video_filter as u8,
                    );
                    self.gpu = Some(g);
                    if let Some(w) = &self.window {
                        let s = w.inner_size();
                        if let Some(g) = self.gpu.as_mut() {
                            g.resize(s.width, s.height);
                        }
                    }
                    self.refresh_touch_layout();
                    self.invalidate_layout();
                    self.pacer.resync(self.now());
                    self.redraw();
                }
                Err(e) => {
                    log::error!("GPU: {e}");
                    self.fail_gpu(e);
                }
            },
            UserEvent::RomLoaded { name, bytes } => {
                self.loading = false;
                self.loading_since = None;
                self.invalidate_layout();
                self.load_rom_bytes(name, bytes, true);
                self.redraw();
            }
            UserEvent::PadAxes { x, y } => {
                let stick = Buttons::NONE
                    .with(Buttons::LEFT, x < -0.5)
                    .with(Buttons::RIGHT, x > 0.5)
                    .with(Buttons::UP, y < -0.5)
                    .with(Buttons::DOWN, y > 0.5);
                if !self.playing() {
                    // d-pad do controle navega os menus (só nas bordas de subida)
                    if stick.0 & !self.pad_stick[0].0 & Buttons::UP.0 != 0 {
                        self.nav(-1, _el);
                    } else if stick.0 & !self.pad_stick[0].0 & Buttons::DOWN.0 != 0 {
                        self.nav(1, _el);
                    } else if stick.0 & !self.pad_stick[0].0 & Buttons::LEFT.0 != 0 {
                        self.nav_activate(-1, _el);
                    } else if stick.0 & !self.pad_stick[0].0 & Buttons::RIGHT.0 != 0 {
                        self.nav_activate(1, _el);
                    }
                }
                self.pad_stick[0] = stick;
            }
            UserEvent::RomLoadFailed(why) => {
                self.loading = false;
                self.loading_since = None;
                self.invalidate_layout();
                if why != "cancelado" {
                    self.toast_error(why);
                }
                self.redraw();
            }
        }
    }

    fn window_event(&mut self, el: &ActiveEventLoop, id: WindowId, ev: WindowEvent) {
        let Some(window) = self.window.clone() else { return };
        if window.id() != id {
            return;
        }
        match ev {
            WindowEvent::CloseRequested => {
                self.flush_save();
                self.save_config();
                el.exit();
            }
            WindowEvent::Resized(s) => {
                if let Some(g) = self.gpu.as_mut() {
                    g.resize(s.width, s.height);
                }
                self.refresh_touch_layout();
                self.invalidate_layout();
                self.redraw();
            }
            WindowEvent::ScaleFactorChanged { scale_factor, .. } => {
                self.dpi = scale_factor as f32;
                self.invalidate_layout();
                self.redraw();
            }
            WindowEvent::DroppedFile(path) => {
                let name = path.file_name().map(|s| s.to_string_lossy().into_owned()).unwrap_or_default();
                match std::fs::read(&path) {
                    Ok(bytes) => self.load_rom_bytes(name, bytes, true),
                    Err(e) => self.toast_error(format!("{name}: {e}")),
                }
                self.redraw();
            }
            WindowEvent::RedrawRequested => self.draw(),
            WindowEvent::MouseWheel { delta, .. } => {
                if !self.playing() {
                    let dy = match delta {
                        winit::event::MouseScrollDelta::LineDelta(_, y) => y * 60.0,
                        winit::event::MouseScrollDelta::PixelDelta(p) => p.y as f32,
                    };
                    self.scroll -= dy;
                    self.clamp_scroll();
                    self.redraw();
                }
            }
            WindowEvent::Focused(false) => {
                self.input.clear();
                self.input2.clear();
                self.touch.clear();
                self.rewinding = false;
                // Cortina de notificações, diálogo, outro app: pausa em vez de correr às cegas
                if self.playing() {
                    self.set_screen(Screen::Paused);
                }
            }
            WindowEvent::CursorMoved { position, .. } => {
                self.cursor = (position.x, position.y);
                if !self.playing() {
                    if self.pressed.is_none() && self.selected.is_some() {
                        self.selected = None; // volta ao hover do mouse
                    }
                    if self.pressed.is_some() {
                        self.menu_drag(MOUSE_ID, position.x as f32, position.y as f32, el);
                    }
                    self.redraw();
                }
            }
            WindowEvent::MouseInput { state, button: MouseButton::Left, .. } => {
                let (x, y) = (self.cursor.0 as f32, self.cursor.1 as f32);
                if state == ElementState::Pressed {
                    self.ensure_audio();
                    if !self.playing() {
                        self.menu_press(MOUSE_ID, x, y, el);
                    } else if self.config.zapper {
                        self.zapper_shot(x, y);
                    }
                } else if !self.playing() {
                    self.menu_release(MOUSE_ID, x, y, el);
                }
                self.redraw();
            }
            WindowEvent::Touch(t) => {
                let (x, y) = (t.location.x as f32, t.location.y as f32);
                match t.phase {
                    TouchPhase::Started => {
                        self.ensure_audio();
                        if self.playing() {
                            if self.touch_layout.special(x, y) == Some(Special::Menu) {
                                self.buzz();
                                self.set_screen(Screen::Paused);
                            } else {
                                if self.config.zapper && self.nes_pixel(x, y).is_some() {
                                    self.zapper_shot(x, y); // mira dentro da imagem
                                }
                                let first = !self.touch.seen;
                                let b = self.touch.down(&self.touch_layout, t.id, x, y);
                                if first {
                                    self.update_gesture_exclusion();
                                }
                                if b != Buttons::NONE {
                                    self.buzz();
                                }
                            }
                        } else {
                            self.touch.seen = true;
                            self.cursor = (-1.0, -1.0);
                            self.menu_press(t.id, x, y, el);
                        }
                    }
                    TouchPhase::Moved => {
                        if self.playing() {
                            self.touch.moved(&self.touch_layout, t.id, x, y);
                            let b = self.touch.buttons();
                            if b != self.last_touch_buttons && b.0 & !self.last_touch_buttons.0 != 0 {
                                self.buzz();
                            }
                            self.last_touch_buttons = b;
                        } else {
                            self.menu_drag(t.id, x, y, el);
                        }
                    }
                    TouchPhase::Ended => {
                        if self.playing() {
                            self.touch.up(t.id);
                            self.last_touch_buttons = self.touch.buttons();
                        } else {
                            self.menu_release(t.id, x, y, el);
                        }
                        self.cursor = (-1.0, -1.0);
                    }
                    TouchPhase::Cancelled => {
                        self.touch.up(t.id);
                        self.last_touch_buttons = self.touch.buttons();
                        self.pressed = None;
                        self.cursor = (-1.0, -1.0);
                    }
                }
                self.redraw();
            }
            WindowEvent::KeyboardInput { event, .. } => {
                use winit::keyboard::{Key, NamedKey};
                // Botão/gesto Voltar do Android chega só como tecla lógica (sem código físico)
                if matches!(event.logical_key, Key::Named(NamedKey::BrowserBack | NamedKey::GoBack)) {
                    if event.state == ElementState::Pressed && !event.repeat {
                        self.handle_key(KeyCode::Escape, true, el);
                    }
                    self.redraw();
                    return;
                }
                let code = match event.physical_key {
                    PhysicalKey::Code(c) => Some(c),
                    // Android: KEYCODE_BUTTON_* de gamepads não têm código físico no winit
                    PhysicalKey::Unidentified(winit::keyboard::NativeKeyCode::Android(k)) => match k {
                        96 | 97 => Some(KeyCode::KeyZ),  // BUTTON_A (sul) / BUTTON_B (leste) → A
                        99 | 100 => Some(KeyCode::KeyX), // BUTTON_X / BUTTON_Y → B
                        108 => Some(KeyCode::Enter),     // BUTTON_START
                        109 => Some(KeyCode::Tab),       // BUTTON_SELECT
                        _ => None,
                    },
                    _ => None,
                };
                if let Some(code) = code {
                    if event.state == ElementState::Pressed && !event.repeat {
                        self.ensure_audio();
                    }
                    if !event.repeat || event.state == ElementState::Released {
                        self.handle_key(code, event.state == ElementState::Pressed, el);
                    }
                }
                self.redraw();
            }
            _ => {}
        }
    }

    /// Android: a superfície some; solta a GPU e a janela, que `resumed` recria.
    fn suspended(&mut self, _el: &ActiveEventLoop) {
        self.flush_save();
        self.input.clear();
        self.input2.clear();
        self.touch.clear();
        self.rewinding = false;
        self.save_config();
        self.gpu = None;
        self.window = None;
        self.audio = None; // em background o stream pode ser desconectado; volta no próximo gesto
        self.overlay_key = None;
        self.pressed = None;
        self.confirm_reset = false;
        if self.playing() {
            self.set_screen(Screen::Paused);
        } else {
            self.invalidate_layout();
        }
    }

    fn about_to_wait(&mut self, el: &ActiveEventLoop) {
        self.advance();
        self.drain_nav(el);
        if self.loading && self.loading_since.is_some_and(|t| t.elapsed() > Duration::from_secs(90)) {
            self.loading = false;
            self.loading_since = None;
            self.invalidate_layout();
            self.redraw();
        }
        if self.playing() && self.gpu.is_some() {
            el.set_control_flow(ControlFlow::WaitUntil(self.start + self.pacer.next_deadline()));
        } else if Instant::now() < self.toast_until {
            // toast num menu: acorda na hora de apagá-lo
            el.set_control_flow(ControlFlow::WaitUntil(self.toast_until));
        } else {
            if self.overlay_key.as_ref().is_some_and(|k| k.toast.is_some()) {
                self.redraw(); // o último overlay ainda tem o toast
            }
            el.set_control_flow(ControlFlow::Wait);
        }
    }
}
