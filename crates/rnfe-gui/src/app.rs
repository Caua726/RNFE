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
    RomLoaded { name: String, bytes: Vec<u8> },
    RomLoadFailed(String),
}

/// Velocidade do turbo.
const TURBO: f64 = 4.0;
/// Frames que "Voltar 5 s" desfaz.
const REWIND_FRAMES: u32 = 300;
/// Identificador do mouse nos menus (dedos usam o id do toque).
const MOUSE_ID: u64 = u64::MAX;

/// O que muda a aparência do overlay (menus, toque, debug, toast).
#[derive(Clone, PartialEq)]
struct OverlayKey {
    screen: Screen,
    touch: Buttons,
    touch_visible: bool,
    debug: Option<(u32, u32, usize)>,
    toast: Option<String>,
    cursor: (i32, i32),
    layout_gen: u64,
    pressed: Option<usize>,
    slider: Option<u32>,
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
    config: Config,
    recent: Vec<RecentRom>,
    screen: Screen,
    layout_cache: Option<Layout>,
    /// `Window::scale_factor()`: ~2,6 num celular, 1,0 num desktop comum.
    dpi: f32,
    /// Chave do último overlay desenhado: só se redesenha (e reenvia à GPU) quando muda.
    overlay_key: Option<OverlayKey>,
    overlay_size: (u32, u32),
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
    pad: Buttons,
    pad_stick: Buttons,
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
        let Launch { mut nes, rom_name, mut storage, picker, haptic } = launch;
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
            touch_layout: TouchLayout::for_size_scaled(768.0, 720.0, config.touch_scale),
            config,
            recent,
            screen,
            layout_cache: None,
            dpi: 1.0,
            overlay_key: None,
            overlay_size: (0, 0),
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
            pad: Buttons::NONE,
            pad_stick: Buttons::NONE,
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

    fn playing(&self) -> bool {
        self.screen == Screen::Playing && self.nes.is_some()
    }

    fn refresh_touch_layout(&mut self) {
        let (w, h) = self.size();
        let img_bottom =
            self.gpu.as_ref().map(|g| g.viewport.1 + g.viewport.3).unwrap_or(w as f32 * 240.0 / 256.0);
        self.touch_layout =
            TouchLayout::for_viewport(w as f32, h as f32, img_bottom, self.config.touch_scale);
    }

    fn set_screen(&mut self, s: Screen) {
        if s == Screen::Playing && self.nes.is_none() {
            self.screen = Screen::Start;
        } else {
            self.screen = s;
        }
        if s == Screen::States {
            self.refresh_slots();
        }
        self.confirm_reset = false;
        self.pressed = None;
        self.layout_cache = None;
        self.input.clear();
        self.touch.clear();
        self.rewinding = false;
        self.save_config();
        if self.screen == Screen::Playing {
            self.pacer.resync(self.now());
            // ROM vinda do SAF e o botão Voltar não passam por gesto de toque; só a web
            // exige um gesto para o áudio
            #[cfg(not(target_arch = "wasm32"))]
            self.ensure_audio();
        }
        self.redraw();
    }

    /// Grava os ajustes se mudaram (não a cada evento de arrasto do slider).
    fn save_config(&mut self) {
        if self.config_dirty {
            self.config_dirty = false;
            self.config.save(self.storage.as_mut());
        }
    }

    fn apply_config(&mut self) {
        if let Some(g) = self.gpu.as_mut() {
            g.set_integer_scale(self.config.integer_scale);
        }
        self.refresh_touch_layout();
        self.layout_cache = None;
        self.config_dirty = true;
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
        if let Some(a) = AudioOut::start() {
            if let Some(n) = self.nes.as_mut() {
                n.set_sample_rate(a.sample_rate);
            }
            self.audio = Some(a);
        }
    }

    fn flush_save(&mut self) {
        if let Some(n) = self.nes.as_mut() {
            if let Err(e) = self.save.flush(n, self.storage.as_mut()) {
                log::error!("erro ao gravar save: {e}");
            }
        }
    }

    fn install_nes(&mut self, mut nes: Box<Nes>, name: String) {
        self.flush_save();
        if let Some(a) = &self.audio {
            nes.set_sample_rate(a.sample_rate);
            a.ring.clear();
        }
        if let Some(old) = &self.nes {
            nes.debugger.trace_enabled = old.debugger.trace_enabled;
            nes.debugger.enabled = old.debugger.enabled;
        }
        self.save = SaveManager::new(&nes);
        if self.save.load(&mut nes, self.storage.as_mut()) {
            log::info!("save carregado: {}", self.save.key().unwrap_or(""));
        }
        self.nes = Some(nes);
        self.rom_name = name;
        self.play_frames = 0;
        self.rewind.clear();
        self.turbo = false;
        self.pacer.set_speed(1.0);
        if let Some(w) = &self.window {
            w.set_title(&format!("RNFE — {}", self.rom_name));
        }
        self.set_screen(Screen::Playing);
    }

    /// `store`: guardar os bytes em `roms/<hash>.nes` (ROM nova); reabrir dos recentes só
    /// reordena a lista.
    fn load_rom_bytes(&mut self, name: String, bytes: Vec<u8>, store: bool) {
        match crate::load_rom_bytes(&bytes) {
            Ok(nes) => {
                let hash = nes.cartridge().rom_hash();
                self.recent =
                    config::push_recent(self.storage.as_mut(), hash, &name, store.then_some(&bytes[..]));
                self.install_nes(nes, name.clone());
                self.toast(name);
            }
            Err(e) => self.toast(format!("{name}: {e}")),
        }
    }

    fn open_rom(&mut self) {
        if self.loading {
            return;
        }
        self.loading = true;
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
                self.layout_cache = None;
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
            Err(e) => self.toast(format!("Erro: {e}")),
        }
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
            format!("Voltou {:.1} s", steps as f32 * Rewind::EVERY as f32 / 60.0)
        });
    }

    fn set_turbo(&mut self, on: bool) {
        self.turbo = on;
        self.pacer.set_speed(if on { TURBO } else { 1.0 });
    }

    fn menu_state(&self) -> MenuState {
        MenuState {
            has_rom: self.nes.is_some(),
            rom_name: self.rom_name.clone(),
            turbo: self.turbo,
            can_quit: cfg!(not(any(target_arch = "wasm32", target_os = "android"))),
            recent: self.recent.clone(),
            version: env!("CARGO_PKG_VERSION").into(),
            confirm_reset: self.confirm_reset,
            states_load: self.states_load,
            slots: self.slots,
            play_seconds: self.play_frames / 60,
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
            self.layout_cache = None;
        }
        match action {
            Action::Resume => self.set_screen(Screen::Playing),
            Action::OpenRom => self.open_rom(),
            Action::Recents => self.set_screen(Screen::Recents),
            Action::OpenRecent(h) => self.open_recent(h),
            Action::RemoveRecent(h) => {
                self.recent = config::remove_recent(self.storage.as_mut(), h);
                self.layout_cache = None;
                if self.recent.is_empty() {
                    self.set_screen(if self.nes.is_some() { Screen::Paused } else { Screen::Start });
                }
            }
            Action::Reset => {
                if self.confirm_reset {
                    self.reset();
                    self.set_screen(Screen::Playing);
                } else {
                    self.confirm_reset = true;
                    self.layout_cache = None;
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
                self.layout_cache = None;
            }
            Action::Settings => self.set_screen(Screen::Settings),
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
        let Some(g) = self.gilrs.as_mut() else { return };
        while let Some(ev) = g.next_event() {
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
            match ev.event {
                EventType::ButtonPressed(b, _) => self.pad |= map(b),
                EventType::ButtonReleased(b, _) => self.pad = self.pad.with(map(b), false),
                EventType::AxisChanged(axis, v, _) => {
                    let (neg, pos) = match axis {
                        Axis::LeftStickX => (Buttons::LEFT, Buttons::RIGHT),
                        Axis::LeftStickY => (Buttons::DOWN, Buttons::UP),
                        _ => continue,
                    };
                    self.pad_stick = self.pad_stick.with(neg, v < -0.5).with(pos, v > 0.5);
                }
                _ => {}
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
        let Some(nes) = self.nes.as_mut() else { return };
        let buttons = self.input.current(now) | self.touch.buttons() | self.pad | self.pad_stick;
        nes.set_controller(0, buttons);
        for _ in 0..due {
            if self.rewinding {
                if !self.rewind.step_back(nes) {
                    break;
                }
                continue;
            }
            nes.run_frame();
            self.play_frames += 1;
            self.rewind.record(nes);
            if let Err(e) = self.save.tick(nes, self.storage.as_mut()) {
                log::error!("erro ao gravar save: {e}");
            }
            self.fps_counter += 1;
        }
        if let Some(a) = &self.audio {
            if self.rewinding || self.turbo || self.config.volume <= 0.0 {
                nes.bus.apu.sample_buffer.clear();
            } else {
                if self.config.volume < 1.0 {
                    let v = self.config.volume;
                    for s in nes.bus.apu.sample_buffer.iter_mut() {
                        *s *= v;
                    }
                }
                a.ring.push(&nes.bus.apu.sample_buffer);
                nes.bus.apu.sample_buffer.clear();
                a.ring.trim_to(AudioOut::TARGET_QUEUE); // o anel é mono
            }
        } else {
            nes.bus.apu.sample_buffer.clear();
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
        let fb = std::mem::take(&mut self.overlay);
        let mut fb = fb;
        ui::clear(&mut fb, if self.screen == Screen::Paused { theme.bg } else { theme.panel });
        let s = layout.ui_scale;
        let (mx, my) = (self.cursor.0 as f32, self.cursor.1 as f32);
        let title_size = if self.screen == Screen::Start { 64.0 * s } else { 30.0 * s };
        let title_y =
            if self.screen == Screen::Start { (h as f32 * 0.17) as i32 } else { (h as f32 * 0.05) as i32 };
        if self.screen == Screen::Start {
            // marca: um cartucho estilizado atrás do título
            let cw = 150.0 * s;
            let ch = 90.0 * s;
            let cx = (w as f32 - cw) / 2.0;
            let cy = title_y as f32 - 12.0 * s;
            ui::fill_round_rect(
                &mut fb,
                w,
                h,
                &Rect { x: cx, y: cy + 4.0 * s, w: cw, h: ch },
                10.0 * s,
                [0, 0, 0, 110],
            );
            ui::fill_round_rect(&mut fb, w, h, &Rect { x: cx, y: cy, w: cw, h: ch }, 10.0 * s, theme.accent);
        }
        let title_color = if self.screen == Screen::Start { theme.on_accent } else { theme.text };
        self.ui.draw_text_centered(&mut fb, w, h, &layout.title, title_size, title_y, title_color);
        self.ui.draw_text_centered(
            &mut fb,
            w,
            h,
            &layout.subtitle,
            14.0 * s,
            title_y + title_size as i32 + (8.0 * s) as i32,
            theme.dim,
        );
        if let Some(e) = &self.gpu_error {
            let msg = e.clone();
            self.ui.draw_text_centered(
                &mut fb,
                w,
                h,
                &msg,
                12.0 * s,
                title_y + title_size as i32 + (30.0 * s) as i32,
                theme.accent,
            );
        }
        let pressed_idx = self.pressed.map(|p| p.0);
        for (i, item) in layout.items.iter().enumerate() {
            let hot = pressed_idx == Some(i) || (pressed_idx.is_none() && item.rect.contains(mx, my));
            self.ui.draw_item(&mut fb, w, h, item, &layout, hot, &theme);
        }
        if self.screen == Screen::Recents && layout.items.len() == 1 {
            let msg = "Abra uma ROM: ela aparece aqui";
            self.ui.draw_text_centered(&mut fb, w, h, msg, 14.0 * s, (h as f32 * 0.5) as i32, theme.dim);
        }
        if self.screen == Screen::Start {
            let hint = if self.picker.is_some() {
                "toque em Abrir ROM"
            } else {
                "tecle O ou clique em Abrir ROM · Esc sai"
            };
            self.ui.draw_text_centered(
                &mut fb,
                w,
                h,
                hint,
                12.0 * s,
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
        let key = OverlayKey {
            screen: self.screen,
            touch: self.touch.buttons(),
            touch_visible,
            debug: self.debug_overlay.then(|| (self.fps_display, self.skipped_frames, self.rewind.len())),
            toast: show_toast.then(|| self.toast_msg.clone()),
            cursor: if self.playing() { (0, 0) } else { (self.cursor.0 as i32, self.cursor.1 as i32) },
            layout_gen: self.layout_cache.as_ref().map(|l| l.items.len() as u64).unwrap_or(0),
            pressed: self.pressed.map(|p| p.0),
            slider: self.layout_cache.as_ref().map(|l| {
                l.items
                    .iter()
                    .filter_map(|i| {
                        if let ItemKind::Slider { fraction } = i.kind {
                            Some((fraction * 1000.0) as u32)
                        } else {
                            None
                        }
                    })
                    .sum()
            }),
        };
        let dirty = self.overlay_key.as_ref() != Some(&key)
            || self.overlay_size != (w, h)
            || self.layout_cache.is_none();
        let has_overlay = self.screen != Screen::Playing
            || self.nes.is_none()
            || touch_visible
            || self.debug_overlay
            || show_toast;
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
        } else {
            self.overlay.fill(0);
            if touch_visible {
                let pressed = self.touch.buttons();
                let (op, hc) = (self.config.touch_opacity, self.config.high_contrast);
                let layout = self.touch_layout.clone();
                self.ui.draw_touch_controls(&mut self.overlay, w, h, &layout, pressed, op, &theme, hc);
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
            self.ui.draw_toast(&mut self.overlay, w, h, &msg, 16.0 * s);
        }
        let Some(gpu) = self.gpu.as_mut() else { return };
        let fb = self.nes.as_mut().map(|n| n.framebuffer());
        let ov = if has_overlay { Some(self.overlay.as_slice()) } else { None };
        if !gpu.render(fb, has_overlay, ov) {
            self.redraw();
        }
    }

    fn handle_key(&mut self, key: KeyCode, pressed: bool, el: &ActiveEventLoop) {
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
                _ => None,
            };
            if let Some(b) = bit {
                self.input.set(b, pressed);
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
                Screen::Settings | Screen::Recents | Screen::States => self.act(Action::Back, el),
                Screen::Start => {
                    self.flush_save();
                    el.exit();
                }
            },
            KeyCode::KeyO => self.open_rom(),
            KeyCode::KeyR => self.reset(),
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
    fn menu_press(&mut self, id: u64, x: f32, y: f32, el: &ActiveEventLoop) {
        if self.pressed.is_some() {
            return; // já há um dedo num item: o segundo é ignorado
        }
        let layout = self.layout();
        self.pressed = menu::index_at(&layout, x, y).map(|i| (i, id, x, y));
        if let Some((i, ..)) = self.pressed {
            if matches!(layout.items[i].kind, ItemKind::Slider { .. }) {
                if let Some(a) = menu::hit(&layout, x, y) {
                    self.act(a, el);
                }
            }
        }
    }

    /// Arrasto com o dedo/mouse pressionado: sliders acompanham.
    fn menu_drag(&mut self, id: u64, x: f32, el: &ActiveEventLoop) {
        let Some((i, pid, _, _)) = self.pressed else { return };
        if pid != id {
            return;
        }
        let layout = self.layout();
        if let Some(a) = menu::slide(&layout, i, x) {
            self.act(a, el);
        }
    }

    /// Soltou: dispara a ação se ainda está sobre o mesmo item (senão, cancela).
    fn menu_release(&mut self, id: u64, x: f32, y: f32, el: &ActiveEventLoop) {
        if self.pressed.is_some_and(|p| p.1 != id) {
            return; // soltou outro dedo
        }
        let Some((i, ..)) = self.pressed.take() else { return };
        let layout = self.layout();
        if matches!(layout.items.get(i).map(|it| &it.kind), Some(ItemKind::Slider { .. })) {
            self.layout_cache = None;
            self.save_config();
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
            .with_inner_size(winit::dpi::PhysicalSize::new(768, 720));
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
        let window = Arc::new(el.create_window(attrs).expect("janela"));
        self.window = Some(window.clone());
        self.dpi = window.scale_factor() as f32;
        log::info!("janela {}x{} @{:.2}", window.inner_size().width, window.inner_size().height, self.dpi);
        self.layout_cache = None;
        self.overlay_key = None;
        #[cfg(feature = "gamepad")]
        {
            self.gilrs = gilrs::Gilrs::new().ok();
        }
        #[cfg(not(target_arch = "wasm32"))]
        {
            match pollster::block_on(GpuState::new(window.clone())) {
                Ok(mut g) => {
                    g.set_integer_scale(self.config.integer_scale);
                    self.gpu = Some(g);
                }
                Err(e) => {
                    log::error!("GPU: {e}");
                    self.gpu_error = Some(e);
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
        match ev {
            UserEvent::GpuReady(r) => match *r {
                Ok(mut g) => {
                    g.set_integer_scale(self.config.integer_scale);
                    self.gpu = Some(g);
                    if let Some(w) = &self.window {
                        let s = w.inner_size();
                        if let Some(g) = self.gpu.as_mut() {
                            g.resize(s.width, s.height);
                        }
                    }
                    self.refresh_touch_layout();
                    self.layout_cache = None;
                    self.pacer.resync(self.now());
                    self.redraw();
                }
                Err(e) => {
                    log::error!("GPU: {e}");
                    self.gpu_error = Some(e);
                }
            },
            UserEvent::RomLoaded { name, bytes } => {
                self.loading = false;
                self.load_rom_bytes(name, bytes, true);
                self.redraw();
            }
            UserEvent::RomLoadFailed(why) => {
                self.loading = false;
                if why != "cancelado" {
                    self.toast(why);
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
                self.layout_cache = None;
                self.redraw();
            }
            WindowEvent::ScaleFactorChanged { scale_factor, .. } => {
                self.dpi = scale_factor as f32;
                self.layout_cache = None;
                self.redraw();
            }
            WindowEvent::RedrawRequested => self.draw(),
            WindowEvent::Focused(false) => {
                self.input.clear();
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
                    if self.pressed.is_some() {
                        self.menu_drag(MOUSE_ID, position.x as f32, el);
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
                                let b = self.touch.down(&self.touch_layout, t.id, x, y);
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
                            self.menu_drag(t.id, x, el);
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
            self.layout_cache = None;
        }
    }

    fn about_to_wait(&mut self, el: &ActiveEventLoop) {
        self.advance();
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
