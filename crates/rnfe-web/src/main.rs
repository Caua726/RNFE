//! Entrada do build web (Trunk). Fora do wasm32 só avisa como construir.

fn main() {
    #[cfg(target_arch = "wasm32")]
    rnfe_gui::run_web();
    #[cfg(not(target_arch = "wasm32"))]
    eprintln!("rnfe-web só faz sentido em wasm32: `trunk build --release` na raiz do repo.");
}
