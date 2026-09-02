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

pub use app::{App, UserEvent};

/// O que o binário entrega ao frontend para começar.
pub struct Launch {
    /// Console já carregado (ou `None` para a tela inicial).
    pub nes: Option<Box<Nes>>,
    pub rom_name: String,
    /// Onde ficam `.sav` e save states.
    pub storage: Box<dyn Storage>,
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
#[cfg(not(target_arch = "wasm32"))]
pub fn run(launch: Launch) -> Result<(), winit::error::EventLoopError> {
    let el = winit::event_loop::EventLoop::<UserEvent>::with_user_event().build()?;
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
    let launch =
        Launch { nes: None, rom_name: String::new(), storage: Box::new(platform::WebStorage::new()) };
    let app = App::new(launch, el.create_proxy());
    el.spawn_app(app);
}
