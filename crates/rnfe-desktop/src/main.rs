//! `rnfe-desktop [rom.nes]` — janela com GPU, som e gamepad. Saves em `~/.local/share/rnfe`
//! (`$RNFE_DATA_DIR` para mudar). `RUST_LOG=info` mostra o log.

use rnfe_frontend::FsStorage;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    env_logger::Builder::from_env(env_logger::Env::default().default_filter_or("info")).init();
    let arg = std::env::args().nth(1);
    let (nes, rom_name) = match &arg {
        Some(path) => (
            rnfe_gui::load_rom(path),
            std::path::Path::new(path)
                .file_name()
                .map(|s| s.to_string_lossy().into_owned())
                .unwrap_or_default(),
        ),
        None => (None, String::new()),
    };
    let launch =
        rnfe_gui::Launch { nes, rom_name, storage: Box::new(FsStorage::new(FsStorage::default_dir())) };
    rnfe_gui::run(launch)?;
    Ok(())
}
