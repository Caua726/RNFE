//! O aplicativo: laço de eventos do winit, emulação com cadência própria, entrada
//! (teclado, toque, gamepad), saves, save states, rewind e overlays.
//!
//! Fluxo por plataforma:
//! - desktop: `resumed` cria a janela e a GPU (bloqueando), `about_to_wait` emula os frames
//!   devidos e agenda `WaitUntil` no próximo — sem `sleep`.
//! - web: a GPU é inicializada num futuro e chega como `UserEvent::GpuReady`; a ROM chega por
//!   `UserEvent::RomLoaded` do seletor de arquivo; o áudio só nasce após o primeiro gesto.

use crate::audio::AudioOut;
use crate::gpu::GpuState;
use crate::platform::{self, Instant};
use crate::ui::{self, MenuAction, Ui};
use crate::{Launch, RomPicker};
use rnfe_core::{Buttons, Nes, Storage};
use rnfe_frontend::touch::Special;
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

/// Velocidade do turbo (Espaço).
const TURBO: f64 = 4.0;

pub struct App {
    proxy: EventLoopProxy<UserEvent>,
    window: Option<Arc<Window>>,
    gpu: Option<GpuState>,
    gpu_error: Option<String>,
    nes: Option<Box<Nes>>,
    rom_name: String,
    storage: Box<dyn Storage>,
    picker: Option<RomPicker>,
    save: SaveManager,
    rewind: Rewind,
    audio: Option<AudioOut>,
    pacer: FramePacer,
    start: Instant,
    input: InputState,
    touch: TouchState,
    layout: TouchLayout,
    #[cfg(feature = "gamepad")]
    gilrs: Option<gilrs::Gilrs>,
    pad: Buttons,
    pad_stick: Buttons,
    cursor: (f64, f64),
    ui: Ui,
    overlay: Vec<u8>,
    paused: bool,
    debug_overlay: bool,
    rewinding: bool,
    turbo: bool,
    fps_counter: u32,
    fps_timer: Instant,
    fps_display: u32,
    toast_msg: String,
    toast_until: Instant,
    loading: bool,
}

impl App {
    pub fn new(launch: Launch, proxy: EventLoopProxy<UserEvent>) -> Self {
        let Launch { mut nes, rom_name, mut storage, picker } = launch;
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
        App {
            proxy,
            window: None,
            gpu: None,
            gpu_error: None,
            nes,
            rom_name,
            storage,
            picker,
            save,
            rewind: Rewind::new(Rewind::DEFAULT_CAP),
            audio: None,
            pacer: FramePacer::new(NTSC_FPS),
            start: now,
            input: InputState::new(),
            touch: TouchState::new(),
            layout: TouchLayout::for_size(768.0, 720.0),
            #[cfg(feature = "gamepad")]
            gilrs: None,
            pad: Buttons::NONE,
            pad_stick: Buttons::NONE,
            cursor: (0.0, 0.0),
            ui: Ui::new(),
            overlay: Vec::new(),
            paused: false,
            debug_overlay: false,
            rewinding: false,
            turbo: false,
            fps_counter: 0,
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

    fn toast(&mut self, msg: impl Into<String>) {
        self.toast_msg = msg.into();
        self.toast_until = Instant::now() + Duration::from_secs(2);
    }

    fn state_key(&self) -> Option<String> {
        self.nes.as_ref().map(|n| format!("state/{:016x}/1.rnfs", n.cartridge().rom_hash()))
    }

    fn redraw(&self) {
        if let Some(w) = &self.window {
            w.request_redraw();
        }
    }

    /// Áudio nasce no primeiro gesto do usuário (exigência dos navegadores; inofensivo no desktop).
    fn ensure_audio(&mut self) {
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
        self.rewind.clear();
        self.input.clear();
        self.touch.clear();
        self.paused = false;
        self.pacer.resync(self.now());
        if let Some(w) = &self.window {
            w.set_title(&format!("RNFE — {}", self.rom_name));
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

    fn reset(&mut self) {
        if let Some(n) = self.nes.as_mut() {
            n.reset();
            self.rewind.clear();
            self.toast("Reset");
        }
    }

    fn save_state(&mut self) {
        let Some(key) = self.state_key() else { return };
        let data = self.nes.as_ref().map(|n| n.save_state()).unwrap_or_default();
        match self.storage.write(&key, &data) {
            Ok(()) => self.toast("State salvo (F5)"),
            Err(e) => self.toast(format!("Erro: {e}")),
        }
    }

    fn load_state(&mut self) {
        let Some(key) = self.state_key() else { return };
        let Some(data) = self.storage.read(&key) else {
            self.toast("Sem state salvo");
            return;
        };
        let r = self.nes.as_mut().map(|n| n.load_state(&data));
        match r {
            Some(Ok(())) => {
                self.rewind.clear();
                self.toast("State carregado (F7)");
            }
            Some(Err(e)) => self.toast(format!("Erro: {e}")),
            None => {}
        }
    }

    fn menu_action(&mut self, action: MenuAction, el: &ActiveEventLoop) {
        match action {
            MenuAction::OpenRom => {
                self.paused = false;
                self.open_rom();
            }
            MenuAction::Reset => {
                self.reset();
                self.paused = false;
            }
            MenuAction::SaveState => self.save_state(),
            MenuAction::LoadState => self.load_state(),
            MenuAction::Quit => {
                self.flush_save();
                el.exit();
            }
            MenuAction::None => {}
        }
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
        if self.paused || self.gpu.is_none() {
            return;
        }
        let now = self.now();
        let due = self.pacer.frames_due(now);
        if due == 0 {
            return;
        }
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
            self.rewind.record(nes);
            if let Err(e) = self.save.tick(nes, self.storage.as_mut()) {
                log::error!("erro ao gravar save: {e}");
            }
            self.fps_counter += 1;
        }
        if let Some(a) = &self.audio {
            if self.rewinding || self.turbo {
                nes.bus.apu.sample_buffer.clear();
            } else {
                a.ring.push(&nes.bus.apu.sample_buffer);
                nes.bus.apu.sample_buffer.clear();
                a.ring.trim_to(AudioOut::TARGET_QUEUE * a.channels.max(1));
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

    /// Monta o overlay (menus, toque, debug, toast) e desenha o frame.
    fn draw(&mut self) {
        let Some((w, h)) = self.gpu.as_ref().map(|g| g.size()) else { return };
        let size = (w * h * 4) as usize;
        self.overlay.resize(size, 0);
        let mut has_overlay = false;
        let (mx, my) = (self.cursor.0 as i32, self.cursor.1 as i32);
        let show_toast = Instant::now() < self.toast_until;

        if self.nes.is_none() {
            ui::clear(&mut self.overlay, [12, 12, 16, 255]);
            let title_y = (h as f32 * 0.30) as i32;
            self.ui.draw_text_centered(&mut self.overlay, w, h, "RNFE", 56.0, title_y, [220, 220, 220, 255]);
            self.ui.draw_text_centered(
                &mut self.overlay,
                w,
                h,
                "Famicom / NES",
                16.0,
                title_y + 65,
                [110, 110, 120, 255],
            );
            let (cx, by) = (w as i32 / 2, (h as f32 * 0.58) as i32);
            let (bx, byy, bw, bh) = self.ui.button_rect("Open ROM", 18.0, cx, by);
            let hover = mx >= bx && mx < bx + bw && my >= byy && my < byy + bh;
            let (c, b) = if hover {
                ([255, 255, 255, 255], [150, 150, 150, 255])
            } else {
                ([180, 180, 180, 255], [80, 80, 80, 255])
            };
            self.ui.draw_button(&mut self.overlay, w, h, "Open ROM", 18.0, cx, by, c, b);
            let hint = if let Some(e) = &self.gpu_error { e.clone() } else { "toque ou tecle O".to_string() };
            self.ui.draw_text_centered(&mut self.overlay, w, h, &hint, 12.0, by + 38, [90, 90, 100, 255]);
            self.ui.draw_menubar(&mut self.overlay, w, h, mx, my);
            has_overlay = true;
        } else if self.paused {
            ui::clear(&mut self.overlay, [8, 8, 14, 230]);
            let y = (h as f32 * 0.35) as i32;
            self.ui.draw_text_centered(&mut self.overlay, w, h, "PAUSED", 36.0, y, [200, 200, 200, 255]);
            self.ui.draw_text_centered(
                &mut self.overlay,
                w,
                h,
                "Esc / MENU para voltar",
                14.0,
                y + 48,
                [120, 120, 130, 255],
            );
            self.ui.draw_menubar(&mut self.overlay, w, h, mx, my);
            has_overlay = true;
        } else {
            self.overlay.fill(0);
            if self.touch.seen {
                let pressed = self.touch.buttons();
                self.ui.draw_touch_controls(&mut self.overlay, w, h, &self.layout, pressed);
                has_overlay = true;
            }
            if self.debug_overlay {
                if let Some(nes) = self.nes.as_ref() {
                    let sz = 13.0;
                    let mut y = 8;
                    ui::fill_rect(&mut self.overlay, w, h, 4, 4, 460, 80, [0, 0, 0, 160]);
                    let l1 = format!(
                        "FPS {}  áudio {}  rewind {} states / {} KB",
                        self.fps_display,
                        self.audio.as_ref().map(|a| a.ring.underruns()).unwrap_or(0),
                        self.rewind.len(),
                        self.rewind.bytes() / 1024
                    );
                    self.ui.draw_text(&mut self.overlay, w, h, &l1, sz, 12, y, [0, 255, 80, 255]);
                    y += 18;
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
                    y += 18;
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
                    y += 18;
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
                has_overlay = true;
            }
        }
        if show_toast {
            let msg = self.toast_msg.clone();
            self.ui.draw_toast(&mut self.overlay, w, h, &msg);
            has_overlay = true;
        }
        let Some(gpu) = self.gpu.as_mut() else { return };
        let fb = self.nes.as_mut().map(|n| n.framebuffer());
        let ov = if has_overlay { Some(self.overlay.as_slice()) } else { None };
        gpu.render(fb, ov);
    }

    fn handle_key(&mut self, key: KeyCode, pressed: bool, el: &ActiveEventLoop) {
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
            KeyCode::Backspace => self.rewinding = pressed && self.nes.is_some(),
            KeyCode::Space => {
                self.turbo = pressed;
                self.pacer.set_speed(if pressed { TURBO } else { 1.0 });
            }
            _ => {}
        }
        if !pressed {
            return;
        }
        match key {
            KeyCode::Escape => {
                if self.nes.is_some() {
                    self.paused = !self.paused;
                    self.input.clear();
                    if !self.paused {
                        self.pacer.resync(self.now());
                    }
                } else {
                    self.flush_save();
                    el.exit();
                }
            }
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
            KeyCode::F5 => self.save_state(),
            KeyCode::F7 => self.load_state(),
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

    fn handle_click(&mut self, x: f32, y: f32, el: &ActiveEventLoop) {
        let (mx, my) = (x as i32, y as i32);
        if self.nes.is_none() || self.paused {
            let mut action = self.ui.handle_click(mx, my);
            if action == MenuAction::None && self.nes.is_none() {
                if let Some((w, h)) = self.gpu.as_ref().map(|g| g.size()) {
                    let (bx, by, bw, bh) =
                        self.ui.button_rect("Open ROM", 18.0, w as i32 / 2, (h as f32 * 0.58) as i32);
                    if mx >= bx && mx < bx + bw && my >= by && my < by + bh {
                        action = MenuAction::OpenRom;
                    }
                }
            }
            self.menu_action(action, el);
        }
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
        let size = window.inner_size();
        self.layout = TouchLayout::for_size(size.width.max(1) as f32, size.height.max(1) as f32);
        #[cfg(feature = "gamepad")]
        {
            self.gilrs = gilrs::Gilrs::new().ok();
        }
        #[cfg(not(target_arch = "wasm32"))]
        {
            match pollster::block_on(GpuState::new(window.clone())) {
                Ok(g) => self.gpu = Some(g),
                Err(e) => {
                    log::error!("GPU: {e}");
                    self.gpu_error = Some(e);
                }
            }
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
                Ok(g) => {
                    self.gpu = Some(g);
                    if let Some(w) = &self.window {
                        let s = w.inner_size();
                        if let Some(g) = self.gpu.as_mut() {
                            g.resize(s.width, s.height);
                        }
                    }
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
                match crate::load_rom_bytes(&bytes) {
                    Ok(nes) => {
                        self.install_nes(nes, name.clone());
                        self.toast(name.clone());
                    }
                    Err(e) => self.toast(format!("{name}: {e}")),
                }
                self.redraw();
            }
            UserEvent::RomLoadFailed(why) => {
                self.loading = false;
                if why != "cancelado" {
                    self.toast(why);
                }
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
                el.exit();
            }
            WindowEvent::Resized(s) => {
                if let Some(g) = self.gpu.as_mut() {
                    g.resize(s.width, s.height);
                }
                self.layout = TouchLayout::for_size(s.width.max(1) as f32, s.height.max(1) as f32);
                self.redraw();
            }
            WindowEvent::RedrawRequested => self.draw(),
            WindowEvent::Focused(false) => {
                self.input.clear();
                self.touch.clear();
                self.rewinding = false;
            }
            WindowEvent::CursorMoved { position, .. } => {
                self.cursor = (position.x, position.y);
                if self.nes.is_none() || self.paused {
                    self.redraw();
                }
            }
            WindowEvent::MouseInput { state: ElementState::Pressed, button: MouseButton::Left, .. } => {
                self.ensure_audio();
                let (x, y) = (self.cursor.0 as f32, self.cursor.1 as f32);
                self.handle_click(x, y, el);
                self.redraw();
            }
            WindowEvent::Touch(t) => {
                let (x, y) = (t.location.x as f32, t.location.y as f32);
                match t.phase {
                    TouchPhase::Started => {
                        self.ensure_audio();
                        if self.nes.is_some() && !self.paused {
                            if self.layout.special(x, y) == Some(Special::Menu) {
                                self.paused = true;
                                self.input.clear();
                                self.touch.clear();
                            } else {
                                self.touch.down(&self.layout, t.id, x, y);
                            }
                        } else {
                            self.touch.seen = true;
                            self.cursor = (t.location.x, t.location.y);
                            if self.paused && self.layout.special(x, y) == Some(Special::Menu) {
                                self.paused = false;
                                self.pacer.resync(self.now());
                            } else {
                                self.handle_click(x, y, el);
                            }
                        }
                    }
                    TouchPhase::Moved => self.touch.moved(&self.layout, t.id, x, y),
                    TouchPhase::Ended | TouchPhase::Cancelled => self.touch.up(t.id),
                }
                self.redraw();
            }
            WindowEvent::KeyboardInput { event, .. } => {
                if let PhysicalKey::Code(code) = event.physical_key {
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
        self.gpu = None;
        self.window = None;
        if self.nes.is_some() {
            self.paused = true;
        }
    }

    fn about_to_wait(&mut self, el: &ActiveEventLoop) {
        self.advance();
        if self.nes.is_some() && !self.paused && self.gpu.is_some() {
            el.set_control_flow(ControlFlow::WaitUntil(self.start + self.pacer.next_deadline()));
        } else {
            el.set_control_flow(ControlFlow::Wait);
        }
    }
}
