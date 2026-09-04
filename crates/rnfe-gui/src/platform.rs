//! O que muda entre desktop e web: relógio, diálogo de arquivo, armazenamento, futuros.

pub use web_time::Instant;

use crate::app::UserEvent;
use winit::event_loop::EventLoopProxy;

/// Abre o seletor de arquivo e entrega a ROM pelo laço de eventos (não bloqueia).
#[cfg(not(target_os = "android"))]
pub fn pick_rom(proxy: EventLoopProxy<UserEvent>) {
    spawn_with(move || async move {
        let dialog = rfd::AsyncFileDialog::new()
            .add_filter("ROM de NES", &["nes", "NES", "zip", "ZIP"])
            .add_filter("Todos os arquivos", &["*"])
            .set_title("Abrir ROM");
        let ev = match dialog.pick_file().await {
            Some(file) => {
                let name = file.file_name();
                let bytes = file.read().await;
                UserEvent::RomLoaded { name, bytes }
            }
            None => UserEvent::RomLoadFailed("cancelado".into()),
        };
        let _ = proxy.send_event(ev);
    });
}

/// Android sem `Launch::picker`: não há diálogo nativo.
#[cfg(target_os = "android")]
pub fn pick_rom(proxy: EventLoopProxy<UserEvent>) {
    let _ = proxy.send_event(UserEvent::RomLoadFailed("sem seletor de arquivos nesta plataforma".into()));
}

/// Constrói e roda um futuro até o fim: numa thread (desktop — só a closure precisa ser
/// `Send`, o futuro nasce dentro dela) ou no laço do navegador (web).
#[cfg(not(target_arch = "wasm32"))]
pub fn spawn_with<M, F>(make: M)
where
    M: FnOnce() -> F + Send + 'static,
    F: std::future::Future<Output = ()>,
{
    std::thread::spawn(move || pollster::block_on(make()));
}

#[cfg(target_arch = "wasm32")]
pub fn spawn_with<M, F>(make: M)
where
    M: FnOnce() -> F + 'static,
    F: std::future::Future<Output = ()> + 'static,
{
    wasm_bindgen_futures::spawn_local(make());
}

/// `localStorage` como [`Storage`]: valores em base64 (localStorage só guarda strings).
#[cfg(target_arch = "wasm32")]
pub struct WebStorage {
    local: Option<web_sys::Storage>,
}

#[cfg(target_arch = "wasm32")]
impl WebStorage {
    pub fn new() -> Self {
        let local = web_sys::window().and_then(|w| w.local_storage().ok().flatten());
        if local.is_none() {
            log::warn!("localStorage indisponível: saves não vão persistir");
        }
        WebStorage { local }
    }
}

#[cfg(target_arch = "wasm32")]
impl Default for WebStorage {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(target_arch = "wasm32")]
impl rnfe_core::Storage for WebStorage {
    fn read(&self, key: &str) -> Option<Vec<u8>> {
        let s = self.local.as_ref()?.get_item(&format!("rnfe/{key}")).ok()??;
        base64::decode(&s)
    }

    fn write(&mut self, key: &str, data: &[u8]) -> Result<(), rnfe_core::StorageError> {
        let local = self.local.as_ref().ok_or_else(|| rnfe_core::StorageError("sem localStorage".into()))?;
        local
            .set_item(&format!("rnfe/{key}"), &base64::encode(data))
            .map_err(|_| rnfe_core::StorageError("localStorage cheio ou bloqueado".into()))
    }

    fn remove(&mut self, key: &str) -> Result<(), rnfe_core::StorageError> {
        if let Some(local) = &self.local {
            let _ = local.remove_item(&format!("rnfe/{key}"));
        }
        Ok(())
    }
}

/// Base64 padrão (RFC 4648), sem dependência.
pub mod base64 {
    const TABLE: &[u8; 64] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";

    pub fn encode(data: &[u8]) -> String {
        let mut out = String::with_capacity(data.len().div_ceil(3) * 4);
        for chunk in data.chunks(3) {
            let b = [chunk[0], *chunk.get(1).unwrap_or(&0), *chunk.get(2).unwrap_or(&0)];
            let n = (b[0] as u32) << 16 | (b[1] as u32) << 8 | b[2] as u32;
            out.push(TABLE[(n >> 18) as usize & 63] as char);
            out.push(TABLE[(n >> 12) as usize & 63] as char);
            out.push(if chunk.len() > 1 { TABLE[(n >> 6) as usize & 63] as char } else { '=' });
            out.push(if chunk.len() > 2 { TABLE[n as usize & 63] as char } else { '=' });
        }
        out
    }

    pub fn decode(s: &str) -> Option<Vec<u8>> {
        fn val(c: u8) -> Option<u32> {
            Some(match c {
                b'A'..=b'Z' => (c - b'A') as u32,
                b'a'..=b'z' => (c - b'a') as u32 + 26,
                b'0'..=b'9' => (c - b'0') as u32 + 52,
                b'+' => 62,
                b'/' => 63,
                _ => return None,
            })
        }
        let bytes: Vec<u8> = s.bytes().filter(|b| !b.is_ascii_whitespace()).collect();
        if bytes.len() % 4 != 0 {
            return None;
        }
        let mut out = Vec::with_capacity(bytes.len() / 4 * 3);
        for chunk in bytes.chunks(4) {
            let pad = chunk.iter().filter(|&&c| c == b'=').count();
            let mut n = 0u32;
            for (i, &c) in chunk.iter().enumerate() {
                let v = if c == b'=' { 0 } else { val(c)? };
                n |= v << (18 - 6 * i);
            }
            out.push((n >> 16) as u8);
            if pad < 2 {
                out.push((n >> 8) as u8);
            }
            if pad < 1 {
                out.push(n as u8);
            }
        }
        Some(out)
    }

    #[cfg(test)]
    mod tests {
        use super::*;

        #[test]
        fn round_trip() {
            for len in 0..40 {
                let data: Vec<u8> = (0..len).map(|i| (i * 37 + 11) as u8).collect();
                let enc = encode(&data);
                assert_eq!(decode(&enc).unwrap(), data, "len {len}: {enc}");
            }
            assert_eq!(encode(b"Man"), "TWFu");
            assert_eq!(encode(b"Ma"), "TWE=");
            assert_eq!(encode(b"M"), "TQ==");
            assert_eq!(decode("abc"), None);
        }
    }
}
