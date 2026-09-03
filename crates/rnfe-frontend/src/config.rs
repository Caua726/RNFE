//! Ajustes do usuário, persistidos no [`Storage`](rnfe_core::Storage) como texto `chave=valor`
//! (sem dependências; tolerante a chaves desconhecidas e valores inválidos).

use rnfe_core::Storage;

pub const CONFIG_KEY: &str = "config";

#[derive(Debug, Clone, PartialEq)]
pub struct Config {
    /// Escala dos controles de toque (0,6–1,6).
    pub touch_scale: f32,
    /// Opacidade dos controles de toque (0,2–1,0).
    pub touch_opacity: f32,
    /// Controles de toque sempre visíveis (senão só após o primeiro toque).
    pub touch_always: bool,
    /// Escala do texto dos menus (0,8–1,6).
    pub text_scale: f32,
    /// Alto contraste nos menus e controles.
    pub high_contrast: bool,
    /// Vibração ao apertar um botão de toque (onde a plataforma suporta).
    pub haptics: bool,
    /// Imagem em múltiplos inteiros do tamanho do NES (senão preenche a janela mantendo o aspecto).
    pub integer_scale: bool,
    /// Volume (0,0–1,0).
    pub volume: f32,
}

impl Default for Config {
    fn default() -> Self {
        Config {
            touch_scale: 1.0,
            touch_opacity: 0.45,
            touch_always: false,
            text_scale: 1.0,
            high_contrast: false,
            haptics: true,
            integer_scale: false,
            volume: 1.0,
        }
    }
}

impl Config {
    pub fn load(storage: &dyn Storage) -> Config {
        storage
            .read(CONFIG_KEY)
            .and_then(|b| String::from_utf8(b).ok())
            .map(|s| Config::parse(&s))
            .unwrap_or_default()
    }

    pub fn save(&self, storage: &mut dyn Storage) {
        if let Err(e) = storage.write(CONFIG_KEY, self.to_text().as_bytes()) {
            log::warn!("config: {e}");
        }
    }

    pub fn to_text(&self) -> String {
        format!(
            "touch_scale={}\ntouch_opacity={}\ntouch_always={}\ntext_scale={}\nhigh_contrast={}\nhaptics={}\ninteger_scale={}\nvolume={}\n",
            self.touch_scale,
            self.touch_opacity,
            self.touch_always,
            self.text_scale,
            self.high_contrast,
            self.haptics,
            self.integer_scale,
            self.volume
        )
    }

    pub fn parse(text: &str) -> Config {
        let mut c = Config::default();
        for line in text.lines() {
            let Some((k, v)) = line.split_once('=') else { continue };
            let (k, v) = (k.trim(), v.trim());
            let f = |cur: f32, lo: f32, hi: f32| v.parse::<f32>().map(|x| x.clamp(lo, hi)).unwrap_or(cur);
            let b = |cur: bool| match v {
                "true" | "1" => true,
                "false" | "0" => false,
                _ => cur,
            };
            match k {
                "touch_scale" => c.touch_scale = f(c.touch_scale, 0.6, 1.6),
                "touch_opacity" => c.touch_opacity = f(c.touch_opacity, 0.2, 1.0),
                "touch_always" => c.touch_always = b(c.touch_always),
                "text_scale" => c.text_scale = f(c.text_scale, 0.8, 1.6),
                "high_contrast" => c.high_contrast = b(c.high_contrast),
                "haptics" => c.haptics = b(c.haptics),
                "integer_scale" => c.integer_scale = b(c.integer_scale),
                "volume" => c.volume = f(c.volume, 0.0, 1.0),
                _ => {}
            }
        }
        c
    }
}

/// Lista de ROMs recentes: `hash\tnome` por linha, a mais recente primeiro; os bytes ficam
/// em `roms/<hash>.nes` para reabrir sem o seletor de arquivos.
pub const RECENT_KEY: &str = "recent";
pub const RECENT_MAX: usize = 8;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RecentRom {
    pub hash: u64,
    pub name: String,
}

impl RecentRom {
    pub fn rom_key(hash: u64) -> String {
        format!("roms/{hash:016x}.nes")
    }
}

pub fn load_recent(storage: &dyn Storage) -> Vec<RecentRom> {
    let Some(text) = storage.read(RECENT_KEY).and_then(|b| String::from_utf8(b).ok()) else {
        return Vec::new();
    };
    text.lines()
        .filter_map(|l| {
            let (h, n) = l.split_once('\t')?;
            Some(RecentRom { hash: u64::from_str_radix(h, 16).ok()?, name: n.to_string() })
        })
        .take(RECENT_MAX)
        .collect()
}

/// Registra uma ROM aberta (guarda os bytes e põe no topo da lista). Erros de espaço só avisam.
pub fn push_recent(storage: &mut dyn Storage, hash: u64, name: &str, bytes: Option<&[u8]>) -> Vec<RecentRom> {
    let mut list = load_recent(storage);
    list.retain(|r| r.hash != hash);
    list.insert(0, RecentRom { hash, name: name.to_string() });
    list.truncate(RECENT_MAX);
    if let Some(bytes) = bytes {
        if let Err(e) = storage.write(&RecentRom::rom_key(hash), bytes) {
            log::warn!("recentes: não guardei a ROM: {e}");
        }
    }
    let text: String = list.iter().map(|r| format!("{:016x}\t{}\n", r.hash, r.name)).collect();
    if let Err(e) = storage.write(RECENT_KEY, text.as_bytes()) {
        log::warn!("recentes: {e}");
    }
    list
}

pub fn remove_recent(storage: &mut dyn Storage, hash: u64) -> Vec<RecentRom> {
    let mut list = load_recent(storage);
    list.retain(|r| r.hash != hash);
    let _ = storage.remove(&RecentRom::rom_key(hash));
    let text: String = list.iter().map(|r| format!("{:016x}\t{}\n", r.hash, r.name)).collect();
    let _ = storage.write(RECENT_KEY, text.as_bytes());
    list
}

#[cfg(test)]
mod tests {
    use super::*;
    use rnfe_core::MemoryStorage;

    #[test]
    fn config_round_trip_and_tolerance() {
        let mut st = MemoryStorage::new();
        assert_eq!(Config::load(&st), Config::default());
        let c = Config { touch_scale: 1.3, high_contrast: true, volume: 0.5, ..Config::default() };
        c.save(&mut st);
        assert_eq!(Config::load(&st), c);
        let p = Config::parse("touch_scale=9\nhigh_contrast=yes\nlixo\nvolume=abc\ntext_scale=1.2\n");
        assert_eq!(p.touch_scale, 1.6, "clamp");
        assert!(!p.high_contrast, "valor inválido mantém o padrão");
        assert_eq!(p.volume, 1.0);
        assert_eq!(p.text_scale, 1.2);
    }

    #[test]
    fn recent_list_orders_and_caps() {
        let mut st = MemoryStorage::new();
        assert!(load_recent(&st).is_empty());
        for i in 0..10u64 {
            push_recent(&mut st, i, &format!("jogo {i}"), Some(&[i as u8; 16]));
        }
        let l = load_recent(&st);
        assert_eq!(l.len(), RECENT_MAX);
        assert_eq!(l[0].hash, 9);
        assert_eq!(st.read(&RecentRom::rom_key(9)).unwrap(), vec![9u8; 16]);
        push_recent(&mut st, 5, "jogo 5", None);
        assert_eq!(load_recent(&st)[0].hash, 5, "reabrir sobe para o topo (sem regravar)");
        assert_eq!(st.read(&RecentRom::rom_key(5)).unwrap(), vec![5u8; 16]);
        let l = remove_recent(&mut st, 5);
        assert!(l.iter().all(|r| r.hash != 5));
        assert!(st.read(&RecentRom::rom_key(5)).is_none());
    }
}
