//! Modelo dos menus de toque: telas, itens com retângulos, hit-test e os ajustes.
//!
//! Puro (sem desenho): o frontend gráfico pede o [`layout`] para o tamanho da janela, desenha
//! os itens e, num toque/clique, chama [`hit`]. Assim o comportamento é testável sem GPU.

use crate::config::{Config, RecentRom};
use crate::touch::Rect;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Screen {
    Start,
    Playing,
    Paused,
    Settings,
    Recents,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Setting {
    TouchScale,
    TouchOpacity,
    TouchAlways,
    TextScale,
    HighContrast,
    Haptics,
    IntegerScale,
    Volume,
}

impl Setting {
    pub const ALL: [Setting; 8] = [
        Setting::Volume,
        Setting::TouchScale,
        Setting::TouchOpacity,
        Setting::TouchAlways,
        Setting::TextScale,
        Setting::HighContrast,
        Setting::IntegerScale,
        Setting::Haptics,
    ];

    pub fn label(self) -> &'static str {
        match self {
            Setting::TouchScale => "Tamanho dos botões",
            Setting::TouchOpacity => "Opacidade dos botões",
            Setting::TouchAlways => "Botões sempre visíveis",
            Setting::TextScale => "Tamanho do texto",
            Setting::HighContrast => "Alto contraste",
            Setting::Haptics => "Vibrar ao tocar",
            Setting::IntegerScale => "Escala inteira",
            Setting::Volume => "Volume",
        }
    }

    pub fn is_bool(self) -> bool {
        matches!(
            self,
            Setting::TouchAlways | Setting::HighContrast | Setting::Haptics | Setting::IntegerScale
        )
    }

    /// Valor formatado para exibir.
    pub fn value(self, c: &Config) -> String {
        let pct = |v: f32| format!("{:.0}%", v * 100.0);
        let on = |b: bool| if b { "Sim" } else { "Não" }.to_string();
        match self {
            Setting::TouchScale => pct(c.touch_scale),
            Setting::TouchOpacity => pct(c.touch_opacity),
            Setting::TouchAlways => on(c.touch_always),
            Setting::TextScale => pct(c.text_scale),
            Setting::HighContrast => on(c.high_contrast),
            Setting::Haptics => on(c.haptics),
            Setting::IntegerScale => on(c.integer_scale),
            Setting::Volume => pct(c.volume),
        }
    }
}

/// Aplica `delta` passos (−1/+1; 0 = alterna booleanos) ao ajuste, com limites.
pub fn adjust(c: &mut Config, s: Setting, delta: i8) {
    let step =
        |v: f32, st: f32, lo: f32, hi: f32| (((v + st * delta as f32) * 100.0).round() / 100.0).clamp(lo, hi);
    match s {
        Setting::TouchScale => c.touch_scale = step(c.touch_scale, 0.1, 0.6, 1.6),
        Setting::TouchOpacity => c.touch_opacity = step(c.touch_opacity, 0.05, 0.2, 1.0),
        Setting::TextScale => c.text_scale = step(c.text_scale, 0.1, 0.8, 1.6),
        Setting::Volume => c.volume = step(c.volume, 0.1, 0.0, 1.0),
        Setting::TouchAlways => c.touch_always = !c.touch_always,
        Setting::HighContrast => c.high_contrast = !c.high_contrast,
        Setting::Haptics => c.haptics = !c.haptics,
        Setting::IntegerScale => c.integer_scale = !c.integer_scale,
    }
}

#[derive(Debug, Clone, PartialEq)]
pub enum Action {
    Resume,
    OpenRom,
    Recents,
    OpenRecent(u64),
    RemoveRecent(u64),
    Reset,
    SaveState,
    LoadState,
    /// Volta ~5 s.
    Rewind,
    ToggleTurbo,
    Settings,
    Back,
    Quit,
    Adjust(Setting, i8),
}

#[derive(Debug, Clone, PartialEq)]
pub struct Item {
    pub rect: Rect,
    pub label: String,
    /// Texto à direita (valor de ajuste, ou vazio).
    pub value: String,
    pub action: Action,
    /// Destaque (ex.: turbo ligado).
    pub active: bool,
}

#[derive(Debug, Clone, PartialEq)]
pub struct Layout {
    pub title: String,
    pub subtitle: String,
    pub items: Vec<Item>,
    /// Fator de escala da interface (fonte, alturas), já com `Config::text_scale`.
    pub ui_scale: f32,
    pub font: f32,
}

/// O que o menu precisa saber do aplicativo.
#[derive(Debug, Clone, Default)]
pub struct MenuState {
    pub has_rom: bool,
    pub rom_name: String,
    pub turbo: bool,
    /// Há um botão "Sair" (desktop; web/Android não fecham por menu).
    pub can_quit: bool,
    pub recent: Vec<RecentRom>,
    pub version: String,
}

/// Escala da interface: pelo fator de DPI da janela (`Window::scale_factor`, ~2,6 num celular
/// de 1080 px; 1,0 num desktop comum), com um piso pela janela e o `text_scale` do usuário.
/// Resultado: fonte ≈ 16 dp e linhas ≥ 48 dp em qualquer tela.
pub fn ui_scale(w: f32, h: f32, config: &Config, dpi: f32) -> f32 {
    let dpi = if dpi.is_finite() && dpi > 0.0 { dpi } else { 1.0 };
    (0.9 * dpi).max(w.min(h) / 1000.0).clamp(0.7, 4.0) * config.text_scale
}

pub fn layout(screen: Screen, w: f32, h: f32, config: &Config, dpi: f32, st: &MenuState) -> Layout {
    let s = ui_scale(w, h, config, dpi);
    let font = 18.0 * s;
    let row_h = 52.0 * s;
    let gap = 10.0 * s;
    let bw = (w * 0.86).min(460.0 * s);
    let x = (w - bw) / 2.0;
    let mut items = Vec::new();
    let (title, subtitle): (String, String);
    let mut y;
    match screen {
        Screen::Start | Screen::Playing => {
            title = "RNFE".into();
            subtitle = if st.version.is_empty() {
                "Famicom / NES".into()
            } else {
                format!("Famicom / NES · v{}", st.version)
            };
            y = h * 0.34;
            let n = 2.0 + if st.recent.is_empty() { 0.0 } else { 1.0 } + if st.can_quit { 1.0 } else { 0.0 };
            let avail = h - y - gap;
            let rh = row_h.min((avail - gap * (n - 1.0)) / n).max(16.0 * s);
            let mut push = |label: &str, action: Action, y: &mut f32| {
                items.push(Item {
                    rect: Rect { x, y: *y, w: bw, h: rh },
                    label: label.into(),
                    value: String::new(),
                    action,
                    active: false,
                });
                *y += rh + gap;
            };
            push("Abrir ROM", Action::OpenRom, &mut y);
            if !st.recent.is_empty() {
                push(&format!("Recentes ({})", st.recent.len()), Action::Recents, &mut y);
            }
            push("Ajustes", Action::Settings, &mut y);
            if st.can_quit {
                push("Sair", Action::Quit, &mut y);
            }
        }
        Screen::Paused => {
            title = "Pausado".into();
            subtitle = st.rom_name.clone();
            y = (h * 0.16).max(row_h);
            let entries: Vec<(&str, Action, bool)> = vec![
                ("Continuar", Action::Resume, false),
                ("Salvar state", Action::SaveState, false),
                ("Carregar state", Action::LoadState, false),
                ("Voltar 5 s (rewind)", Action::Rewind, false),
                (if st.turbo { "Turbo: ligado" } else { "Turbo: desligado" }, Action::ToggleTurbo, st.turbo),
                ("Reset", Action::Reset, false),
                ("Abrir outra ROM", Action::OpenRom, false),
                ("Ajustes", Action::Settings, false),
            ];
            let n = entries.len() as f32 + if st.can_quit { 1.0 } else { 0.0 };
            // encolhe as linhas se não couber
            let avail = h - y - gap;
            let rh = row_h.min((avail - gap * (n - 1.0)) / n).max(16.0 * s);
            for (label, action, active) in entries {
                items.push(Item {
                    rect: Rect { x, y, w: bw, h: rh },
                    label: label.into(),
                    value: String::new(),
                    action,
                    active,
                });
                y += rh + gap;
            }
            if st.can_quit {
                items.push(Item {
                    rect: Rect { x, y, w: bw, h: rh },
                    label: "Sair".into(),
                    value: String::new(),
                    action: Action::Quit,
                    active: false,
                });
            }
        }
        Screen::Settings => {
            title = "Ajustes".into();
            subtitle = "toque em − / + ou na linha".into();
            y = (h * 0.14).max(row_h);
            let n = Setting::ALL.len() as f32 + 1.0;
            let avail = h - y - gap;
            let rh = row_h.min((avail - gap * (n - 1.0)) / n).max(16.0 * s);
            let side = rh; // largura dos botões − / +
            for set in Setting::ALL {
                let value = set.value(config);
                if set.is_bool() {
                    items.push(Item {
                        rect: Rect { x, y, w: bw, h: rh },
                        label: set.label().into(),
                        value,
                        action: Action::Adjust(set, 0),
                        active: false,
                    });
                } else {
                    items.push(Item {
                        rect: Rect { x, y, w: side, h: rh },
                        label: "−".into(),
                        value: String::new(),
                        action: Action::Adjust(set, -1),
                        active: false,
                    });
                    items.push(Item {
                        rect: Rect { x: x + side, y, w: bw - 2.0 * side, h: rh },
                        label: set.label().into(),
                        value,
                        action: Action::Adjust(set, 1),
                        active: false,
                    });
                    items.push(Item {
                        rect: Rect { x: x + bw - side, y, w: side, h: rh },
                        label: "+".into(),
                        value: String::new(),
                        action: Action::Adjust(set, 1),
                        active: false,
                    });
                }
                y += rh + gap;
            }
            items.push(Item {
                rect: Rect { x, y, w: bw, h: rh },
                label: "Voltar".into(),
                value: String::new(),
                action: Action::Back,
                active: false,
            });
        }
        Screen::Recents => {
            title = "Recentes".into();
            subtitle = "toque para abrir · × remove".into();
            y = (h * 0.14).max(row_h);
            let n = st.recent.len() as f32 + 1.0;
            let avail = h - y - gap;
            let rh = row_h.min((avail - gap * (n - 1.0)) / n).max(16.0 * s);
            let side = rh;
            for r in &st.recent {
                items.push(Item {
                    rect: Rect { x, y, w: bw - side, h: rh },
                    label: r.name.clone(),
                    value: String::new(),
                    action: Action::OpenRecent(r.hash),
                    active: false,
                });
                items.push(Item {
                    rect: Rect { x: x + bw - side, y, w: side, h: rh },
                    label: "×".into(),
                    value: String::new(),
                    action: Action::RemoveRecent(r.hash),
                    active: false,
                });
                y += rh + gap;
            }
            items.push(Item {
                rect: Rect { x, y, w: bw, h: rh },
                label: "Voltar".into(),
                value: String::new(),
                action: Action::Back,
                active: false,
            });
        }
    }
    Layout { title, subtitle, items, ui_scale: s, font }
}

/// Ação do item sob o ponto, se houver.
pub fn hit(layout: &Layout, x: f32, y: f32) -> Option<Action> {
    layout.items.iter().find(|i| i.rect.contains(x, y)).map(|i| i.action.clone())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn state() -> MenuState {
        MenuState {
            has_rom: true,
            rom_name: "jogo.nes".into(),
            turbo: false,
            can_quit: true,
            recent: vec![RecentRom { hash: 1, name: "a".into() }, RecentRom { hash: 2, name: "b".into() }],
            version: "0.2.0".into(),
        }
    }

    fn fits(l: &Layout, w: f32, h: f32) {
        for i in &l.items {
            assert!(i.rect.x >= 0.0 && i.rect.y >= 0.0, "{:?}", i);
            assert!(
                i.rect.x + i.rect.w <= w + 0.5 && i.rect.y + i.rect.h <= h + 0.5,
                "{} sai da tela {w}x{h}: {:?}",
                i.label,
                i.rect
            );
        }
    }

    #[test]
    fn every_screen_fits_phone_and_desktop() {
        let c = Config::default();
        for (w, h, dpi) in [
            (1080.0, 2340.0, 2.6),
            (2340.0, 1080.0, 2.6),
            (768.0, 720.0, 1.0),
            (480.0, 320.0, 1.0),
            (1440.0, 3120.0, 3.5),
        ] {
            for sc in [Screen::Start, Screen::Paused, Screen::Settings, Screen::Recents] {
                let l = layout(sc, w, h, &c, dpi, &state());
                assert!(!l.items.is_empty());
                fits(&l, w, h);
                // sem sobreposição entre linhas
                let mut ys: Vec<(f32, f32)> =
                    l.items.iter().map(|i| (i.rect.y, i.rect.y + i.rect.h)).collect();
                ys.sort_by(|a, b| a.partial_cmp(b).unwrap());
                for p in ys.windows(2) {
                    assert!(
                        p[1].0 >= p[0].1 - 0.5 || (p[1].0 - p[0].0).abs() < 0.5,
                        "sobreposição em {sc:?} {w}x{h}"
                    );
                }
            }
        }
    }

    #[test]
    fn hit_returns_actions() {
        let c = Config::default();
        let l = layout(Screen::Paused, 1080.0, 2340.0, &c, 2.6, &state());
        let first = &l.items[0];
        assert!(first.rect.h >= 48.0 * 2.6 * 0.9, "linha com pelo menos ~48 dp: {}", first.rect.h);
        assert!(l.font >= 16.0 * 2.6 * 0.85, "fonte de ~16 dp: {}", l.font);
        assert_eq!(hit(&l, first.rect.x + 1.0, first.rect.y + 1.0), Some(Action::Resume));
        assert_eq!(hit(&l, 0.0, 0.0), None);
        let l = layout(Screen::Recents, 1080.0, 2340.0, &c, 2.6, &state());
        let open = l.items.iter().find(|i| i.action == Action::OpenRecent(2)).unwrap();
        assert_eq!(hit(&l, open.rect.x + 2.0, open.rect.y + 2.0), Some(Action::OpenRecent(2)));
        let rm = l.items.iter().find(|i| i.action == Action::RemoveRecent(2)).unwrap();
        assert!(rm.rect.x > open.rect.x, "× à direita");
    }

    #[test]
    fn settings_adjust_clamps_and_toggles() {
        let mut c = Config::default();
        for _ in 0..20 {
            adjust(&mut c, Setting::TouchScale, 1);
        }
        assert_eq!(c.touch_scale, 1.6);
        for _ in 0..30 {
            adjust(&mut c, Setting::TouchScale, -1);
        }
        assert_eq!(c.touch_scale, 0.6);
        adjust(&mut c, Setting::Volume, -1);
        assert!((c.volume - 0.9).abs() < 1e-6);
        assert!(!c.high_contrast);
        adjust(&mut c, Setting::HighContrast, 0);
        assert!(c.high_contrast);
        assert_eq!(Setting::Volume.value(&c), "90%");
        assert_eq!(Setting::HighContrast.value(&c), "Sim");
        let l = layout(Screen::Settings, 1080.0, 2340.0, &c, 2.6, &state());
        let minus = l.items.iter().filter(|i| i.label == "−").count();
        assert_eq!(minus, Setting::ALL.iter().filter(|s| !s.is_bool()).count());
    }

    #[test]
    fn text_scale_changes_ui_scale() {
        let mut c = Config::default();
        let a = ui_scale(1080.0, 2340.0, &c, 2.6);
        c.text_scale = 1.5;
        assert!((ui_scale(1080.0, 2340.0, &c, 2.6) - a * 1.5).abs() < 1e-4);
        assert!(ui_scale(768.0, 720.0, &Config::default(), 1.0) < a, "desktop menor que celular denso");
        assert_eq!(
            ui_scale(768.0, 720.0, &Config::default(), f32::NAN),
            ui_scale(768.0, 720.0, &Config::default(), 1.0)
        );
        let start = layout(Screen::Start, 1080.0, 2340.0, &c, 2.6, &MenuState::default());
        assert!(start.items.iter().all(|i| !matches!(i.action, Action::Recents)), "sem recentes, sem botão");
    }
}
