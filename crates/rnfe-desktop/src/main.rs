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
    let data_dir = FsStorage::default_dir();
    let mut launch = rnfe_gui::Launch::new(Box::new(FsStorage::new(data_dir.clone())));
    launch.data_dir = Some(data_dir.display().to_string());
    launch.nes = nes;
    launch.rom_name = rom_name;
    rnfe_gui::run(launch)?;
    Ok(())
}
