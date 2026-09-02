use std::env;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args: Vec<String> = env::args().collect();

    if args.len() >= 2 {
        match rnfe_gui::load_rom(&args[1]) {
            Some(nes) => rnfe_gui::run_with_nes(nes)?,
            None => rnfe_gui::run()?,
        }
    } else {
        rnfe_gui::run()?;
    }

    Ok(())
}
