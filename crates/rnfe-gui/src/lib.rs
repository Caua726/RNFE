//! Frontend gráfico do RNFE: janela, GPU, áudio, toque, gamepad e menus.
//!
//! O mesmo código roda em desktop (winit nativo) e na web (wasm32 + canvas). O que muda por
//! plataforma fica em [`platform`]: relógio, arquivos, armazenamento e como iniciar o laço.

pub mod app;
pub mod audio;
pub mod gpu;
pub mod platform;
pub mod ui;

use rnfe_core::{Cartridge, Nes, RomError, Storage};
use winit::event_loop::EventLoopProxy;

pub use app::{App, UserEvent};

/// Seletor de ROM da plataforma: recebe o proxy e, quando o usuário escolher, envia
/// `UserEvent::RomLoaded` (ou `RomLoadFailed`). Sem ele, o frontend usa o `rfd`.
pub type RomPicker = Box<dyn Fn(EventLoopProxy<UserEvent>) + Send + Sync>;

/// Vibração curta ao tocar num botão (Android via JNI; outras plataformas não têm).
pub type Haptic = Box<dyn Fn() + Send + Sync>;
/// Mantém a tela ligada (`true` enquanto joga; Android).
pub type KeepScreenOn = Box<dyn Fn(bool) + Send + Sync>;
/// Aviso fora da janela do jogo (Toast no Android): usado quando nem dá para desenhar.
pub type Notify = Box<dyn Fn(&str) + Send + Sync>;
/// Retângulos (l, t, r, b em px) onde o sistema não deve capturar gestos de borda (Android).
pub type GestureExclusion = Box<dyn Fn([[i32; 4]; 2]) + Send + Sync>;
/// Itens do menu atual entregues ao leitor de tela do sistema: rótulo e retângulo (l, t, r, b).
/// Sem isto, um app que desenha a própria interface é um retângulo mudo para o TalkBack.
pub type A11yNodes = Box<dyn Fn(Vec<(String, [i32; 4])>) + Send + Sync>;

/// O que o binário entrega ao frontend para começar.
pub struct Launch {
    /// Console já carregado (ou `None` para a tela inicial).
    pub nes: Option<Box<Nes>>,
    pub rom_name: String,
    /// Onde ficam `.sav`, save states, ajustes e ROMs recentes.
    pub storage: Box<dyn Storage>,
    /// Caminho dessa pasta no disco, quando faz sentido mostrar (desktop, Android).
    pub data_dir: Option<String>,
    /// Seletor de ROM próprio (Android usa o SAF por JNI).
    pub picker: Option<RomPicker>,
    pub haptic: Option<Haptic>,
    pub gesture_exclusion: Option<GestureExclusion>,
    pub keep_screen_on: Option<KeepScreenOn>,
    pub notify: Option<Notify>,
    /// Publica os itens do menu para o leitor de tela (Android).
    pub a11y: Option<A11yNodes>,
}

impl Launch {
    pub fn new(storage: Box<dyn Storage>) -> Launch {
        Launch {
            nes: None,
            rom_name: String::new(),
            storage,
            data_dir: None,
            picker: None,
            haptic: None,
            keep_screen_on: None,
            notify: None,
            a11y: None,
            gesture_exclusion: None,
        }
    }
}

/// Cria um console a partir dos bytes de uma ROM.
pub fn load_rom_bytes(bytes: &[u8]) -> Result<Box<Nes>, RomError> {
    let cartridge = Cartridge::from_bytes(bytes)?;
    log::info!("ROM: {}", cartridge.describe());
    Ok(Box::new(Nes::new(cartridge)))
}

/// Lê e carrega uma ROM do disco (desktop).
#[cfg(not(target_arch = "wasm32"))]
pub fn load_rom(path: &str) -> Option<Box<Nes>> {
    let bytes = match std::fs::read(path) {
        Ok(b) => b,
        Err(e) => {
            log::error!("erro ao ler '{path}': {e}");
            return None;
        }
    };
    match load_rom_bytes(&bytes) {
        Ok(nes) => Some(nes),
        Err(e) => {
            log::error!("erro ao carregar '{path}': {e}");
            None
        }
    }
}

/// Laço principal no desktop.
#[cfg(not(any(target_arch = "wasm32", target_os = "android")))]
pub fn run(launch: Launch) -> Result<(), winit::error::EventLoopError> {
    let el = winit::event_loop::EventLoop::<UserEvent>::with_user_event().build()?;
    let mut app = App::new(launch, el.create_proxy());
    el.run_app(&mut app)
}

/// Laço principal no Android: recebe o `AndroidApp` do `android_main`.
#[cfg(target_os = "android")]
pub fn run_android(
    android_app: winit::platform::android::activity::AndroidApp,
    launch: Launch,
    on_proxy: impl FnOnce(EventLoopProxy<UserEvent>),
) -> Result<(), winit::error::EventLoopError> {
    use winit::platform::android::EventLoopBuilderExtAndroid;
    let el =
        winit::event_loop::EventLoop::<UserEvent>::with_user_event().with_android_app(android_app).build()?;
    on_proxy(el.create_proxy());
    let mut app = App::new(launch, el.create_proxy());
    el.run_app(&mut app)
}

/// Laço principal na web: instala o panic hook e o log no console e entrega o controle ao
/// navegador (`spawn_app` não bloqueia — o wasm devolve ao JS).
#[cfg(target_arch = "wasm32")]
pub fn run_web() {
    use winit::platform::web::EventLoopExtWebSys;
    console_error_panic_hook::set_once();
    let _ = console_log::init_with_level(log::Level::Info);
    let el = winit::event_loop::EventLoop::<UserEvent>::with_user_event().build().expect("event loop");
    let launch = Launch::new(Box::new(platform::WebStorage::new()));
    let app = App::new(launch, el.create_proxy());
    el.spawn_app(app);
}
