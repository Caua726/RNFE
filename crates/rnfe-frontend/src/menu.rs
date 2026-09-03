//! Modelo dos menus de toque: telas, itens com retângulos, hit-test, sliders e os ajustes.
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
    /// Slots de save state (salvar ou carregar, conforme `MenuState::states_load`).
    States,
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
            Setting::IntegerScale => "Escala inteira (pixels quadrados)",
            Setting::Volume => "Volume",
        }
    }

    pub fn is_bool(self) -> bool {
        matches!(
            self,
            Setting::TouchAlways | Setting::HighContrast | Setting::Haptics | Setting::IntegerScale
        )
    }

    /// Faixa (mín, máx, passo) dos ajustes numéricos.
    pub fn range(self) -> (f32, f32, f32) {
        match self {
            Setting::TouchScale => (0.6, 1.6, 0.1),
            Setting::TouchOpacity => (0.2, 1.0, 0.05),
            Setting::TextScale => (0.8, 1.6, 0.1),
            Setting::Volume => (0.0, 1.0, 0.05),
            _ => (0.0, 1.0, 1.0),
        }
    }

    pub fn get(self, c: &Config) -> f32 {
        match self {
            Setting::TouchScale => c.touch_scale,
            Setting::TouchOpacity => c.touch_opacity,
            Setting::TextScale => c.text_scale,
            Setting::Volume => c.volume,
            Setting::TouchAlways => c.touch_always as u8 as f32,
            Setting::HighContrast => c.high_contrast as u8 as f32,
            Setting::Haptics => c.haptics as u8 as f32,
            Setting::IntegerScale => c.integer_scale as u8 as f32,
        }
    }

    /// Posição do slider (0–1) para o valor atual.
    pub fn fraction(self, c: &Config) -> f32 {
        let (lo, hi, _) = self.range();
        ((self.get(c) - lo) / (hi - lo)).clamp(0.0, 1.0)
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

fn set_value(c: &mut Config, s: Setting, v: f32) {
    let (lo, hi, step) = s.range();
    let v = ((v / step).round() * step).clamp(lo, hi);
    let v = (v * 1000.0).round() / 1000.0;
    match s {
        Setting::TouchScale => c.touch_scale = v,
        Setting::TouchOpacity => c.touch_opacity = v,
        Setting::TextScale => c.text_scale = v,
        Setting::Volume => c.volume = v,
        Setting::TouchAlways => c.touch_always = v >= 0.5,
        Setting::HighContrast => c.high_contrast = v >= 0.5,
        Setting::Haptics => c.haptics = v >= 0.5,
        Setting::IntegerScale => c.integer_scale = v >= 0.5,
    }
}

/// Aplica `delta` passos (−1/+1; 0 = alterna booleanos) ao ajuste, com limites.
pub fn adjust(c: &mut Config, s: Setting, delta: i8) {
    if s.is_bool() {
        set_value(c, s, if s.get(c) >= 0.5 { 0.0 } else { 1.0 });
    } else {
        let (_, _, step) = s.range();
        set_value(c, s, s.get(c) + step * delta as f32);
    }
}

/// Define o ajuste pela posição do slider (0–1).
pub fn set_fraction(c: &mut Config, s: Setting, f: f32) {
    let (lo, hi, _) = s.range();
    set_value(c, s, lo + (hi - lo) * f.clamp(0.0, 1.0));
}

#[derive(Debug, Clone, PartialEq)]
pub enum Action {
    Resume,
    OpenRom,
    Recents,
    OpenRecent(u64),
    RemoveRecent(u64),
    /// Primeiro toque pede confirmação; o segundo reseta.
    Reset,
    /// Abre a tela de slots para salvar (`false`) ou carregar (`true`).
    States {
        load: bool,
    },
    SaveSlot(u8),
    LoadSlot(u8),
    /// Volta ~5 s.
    Rewind,
    ToggleTurbo,
    Settings,
    Back,
    Quit,
    Adjust(Setting, i8),
    /// Slider: valor pela fração horizontal (0–1) do toque dentro da trilha.
    Slide(Setting, f32),
    /// Nada (título de seção).
    None,
}

#[derive(Debug, Clone, PartialEq)]
pub enum ItemKind {
    Button,
    /// Botão de destaque (Continuar, Abrir ROM).
    Primary,
    /// Botão perigoso (reset, remover).
    Danger,
    /// Linha com slider: `fraction` 0–1 preenchida.
    Slider {
        fraction: f32,
    },
    Toggle {
        on: bool,
    },
    /// Título de seção, sem ação.
    Header,
    /// Slot de state: `filled` diz se há algo salvo.
    Slot {
        filled: bool,
    },
}

#[derive(Debug, Clone, PartialEq)]
pub struct Item {
    pub rect: Rect,
    pub label: String,
    /// Texto à direita (valor de ajuste, ou vazio).
    pub value: String,
    pub action: Action,
    pub kind: ItemKind,
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
    /// Raio dos cantos e margem interna, em px.
    pub radius: f32,
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
    /// O reset está aguardando confirmação.
    pub confirm_reset: bool,
    /// Tela de states: carregar (true) ou salvar (false); e quais slots têm conteúdo.
    pub states_load: bool,
    pub slots: [bool; 3],
    /// Tempo de jogo nesta ROM, em segundos (mostrado na pausa).
    pub play_seconds: u64,
}

/// Escala da interface: pelo fator de DPI da janela (`Window::scale_factor`, ~2,6 num celular
/// de 1080 px; 1,0 num desktop comum), com um piso pela janela e o `text_scale` do usuário.
/// Resultado: fonte ≈ 16 dp e linhas ≥ 48 dp em qualquer tela.
pub fn ui_scale(w: f32, h: f32, config: &Config, dpi: f32) -> f32 {
    let dpi = if dpi.is_finite() && dpi > 0.0 { dpi } else { 1.0 };
    (0.9 * dpi).max(w.min(h) / 1000.0).clamp(0.7, 4.0) * config.text_scale
}

fn item(rect: Rect, label: impl Into<String>, action: Action, kind: ItemKind) -> Item {
    Item { rect, label: label.into(), value: String::new(), action, kind, active: false }
}

/// Monta os itens de uma coluna: encolhe as linhas para caber em `h` se precisar.
struct Column {
    x: f32,
    w: f32,
    y: f32,
    gap: f32,
    row_h: f32,
}

impl Column {
    fn fit(&mut self, rows: f32, h: f32, min_row: f32) {
        let avail = h - self.y - self.gap;
        self.row_h = self.row_h.min((avail - self.gap * (rows - 1.0).max(0.0)) / rows.max(1.0)).max(min_row);
    }

    fn push(
        &mut self,
        items: &mut Vec<Item>,
        label: impl Into<String>,
        action: Action,
        kind: ItemKind,
    ) -> usize {
        items.push(item(Rect { x: self.x, y: self.y, w: self.w, h: self.row_h }, label, action, kind));
        self.y += self.row_h + self.gap;
        items.len() - 1
    }

    /// Uma linha com `n` botões lado a lado.
    fn row(&mut self, items: &mut Vec<Item>, cells: Vec<(String, Action, ItemKind)>) {
        let n = cells.len() as f32;
        let cw = (self.w - self.gap * (n - 1.0)) / n;
        for (i, (label, action, kind)) in cells.into_iter().enumerate() {
            let x = self.x + i as f32 * (cw + self.gap);
            items.push(item(Rect { x, y: self.y, w: cw, h: self.row_h }, label, action, kind));
        }
        self.y += self.row_h + self.gap;
    }
}

pub fn layout(screen: Screen, w: f32, h: f32, config: &Config, dpi: f32, st: &MenuState) -> Layout {
    let s = ui_scale(w, h, config, dpi);
    let font = 18.0 * s;
    let row_h = 54.0 * s;
    let gap = 10.0 * s;
    let bw = (w * 0.88).min(480.0 * s);
    let x = (w - bw) / 2.0;
    let min_row = 16.0 * s;
    let mut items = Vec::new();
    let title: String;
    let subtitle: String;
    let mut col = Column { x, w: bw, y: 0.0, gap, row_h };
    match screen {
        Screen::Start | Screen::Playing => {
            title = "RNFE".into();
            subtitle = if st.version.is_empty() {
                "Famicom / NES".into()
            } else {
                format!("Famicom / NES · v{}", st.version)
            };
            col.y = h * 0.36;
            let rows = 2.0 + (!st.recent.is_empty()) as u8 as f32 + st.can_quit as u8 as f32;
            col.fit(rows, h, min_row);
            col.push(&mut items, "Abrir ROM", Action::OpenRom, ItemKind::Primary);
            if !st.recent.is_empty() {
                col.push(
                    &mut items,
                    format!("Recentes ({})", st.recent.len()),
                    Action::Recents,
                    ItemKind::Button,
                );
            }
            col.push(&mut items, "Ajustes", Action::Settings, ItemKind::Button);
            if st.can_quit {
                col.push(&mut items, "Sair", Action::Quit, ItemKind::Button);
            }
        }
        Screen::Paused => {
            title = "Pausado".into();
            let mins = st.play_seconds / 60;
            subtitle =
                if mins > 0 { format!("{} · {} min", st.rom_name, mins) } else { st.rom_name.clone() };
            col.y = (h * 0.15).max(row_h);
            let rows = 6.0 + st.can_quit as u8 as f32;
            col.fit(rows, h, min_row);
            col.push(&mut items, "Continuar", Action::Resume, ItemKind::Primary);
            col.row(
                &mut items,
                vec![
                    ("Salvar".into(), Action::States { load: false }, ItemKind::Button),
                    ("Carregar".into(), Action::States { load: true }, ItemKind::Button),
                    ("Voltar 5 s".into(), Action::Rewind, ItemKind::Button),
                ],
            );
            let i = col.push(
                &mut items,
                if st.turbo { "Turbo: ligado" } else { "Turbo: desligado" },
                Action::ToggleTurbo,
                ItemKind::Toggle { on: st.turbo },
            );
            items[i].active = st.turbo;
            col.push(&mut items, "Abrir outra ROM", Action::OpenRom, ItemKind::Button);
            col.push(&mut items, "Ajustes", Action::Settings, ItemKind::Button);
            let i = col.push(
                &mut items,
                if st.confirm_reset { "Confirmar reset?" } else { "Reset" },
                Action::Reset,
                ItemKind::Danger,
            );
            items[i].active = st.confirm_reset;
            if st.can_quit {
                col.push(&mut items, "Sair", Action::Quit, ItemKind::Button);
            }
        }
        Screen::Settings => {
            title = "Ajustes".into();
            subtitle = "arraste os controles deslizantes".into();
            col.y = (h * 0.13).max(row_h);
            // sliders são mais altos (trilha + rótulo)
            let rows = Setting::ALL.len() as f32 + 1.0;
            col.row_h = 62.0 * s;
            col.fit(rows, h, min_row);
            for set in Setting::ALL {
                let value = set.value(config);
                let i = if set.is_bool() {
                    col.push(
                        &mut items,
                        set.label(),
                        Action::Adjust(set, 0),
                        ItemKind::Toggle { on: set.get(config) >= 0.5 },
                    )
                } else {
                    col.push(
                        &mut items,
                        set.label(),
                        Action::Slide(set, set.fraction(config)),
                        ItemKind::Slider { fraction: set.fraction(config) },
                    )
                };
                items[i].value = value;
            }
            col.push(&mut items, "Voltar", Action::Back, ItemKind::Button);
        }
        Screen::Recents => {
            title = "Recentes".into();
            subtitle = if st.recent.is_empty() {
                "nenhuma ROM aberta ainda".into()
            } else {
                "toque para abrir · × remove".into()
            };
            col.y = (h * 0.13).max(row_h);
            col.fit(st.recent.len() as f32 + 1.0, h, min_row);
            let side = col.row_h;
            for r in &st.recent {
                let y = col.y;
                items.push(item(
                    Rect { x, y, w: bw - side - gap, h: col.row_h },
                    r.name.clone(),
                    Action::OpenRecent(r.hash),
                    ItemKind::Button,
                ));
                items.push(item(
                    Rect { x: x + bw - side, y, w: side, h: col.row_h },
                    "×",
                    Action::RemoveRecent(r.hash),
                    ItemKind::Danger,
                ));
                col.y += col.row_h + gap;
            }
            col.push(&mut items, "Voltar", Action::Back, ItemKind::Button);
        }
        Screen::States => {
            title = if st.states_load { "Carregar state" } else { "Salvar state" }.into();
            subtitle = st.rom_name.clone();
            col.y = (h * 0.13).max(row_h);
            col.fit(4.0, h, min_row);
            for (i, filled) in st.slots.iter().enumerate() {
                let n = i as u8 + 1;
                let label = format!("Slot {n} — {}", if *filled { "salvo" } else { "vazio" });
                let action = if st.states_load { Action::LoadSlot(n) } else { Action::SaveSlot(n) };
                let idx = col.push(&mut items, label, action, ItemKind::Slot { filled: *filled });
                if st.states_load && !filled {
                    items[idx].action = Action::None;
                }
            }
            col.push(&mut items, "Voltar", Action::Back, ItemKind::Button);
        }
    }
    Layout { title, subtitle, items, ui_scale: s, font, radius: 12.0 * s }
}

/// Ação do item sob o ponto, se houver. Para sliders, a fração vem da posição horizontal.
pub fn hit(layout: &Layout, x: f32, y: f32) -> Option<Action> {
    let it = layout.items.iter().find(|i| i.rect.contains(x, y))?;
    Some(match (&it.kind, &it.action) {
        (ItemKind::Slider { .. }, Action::Slide(s, _)) => {
            Action::Slide(*s, slider_fraction(&it.rect, x, layout))
        }
        (ItemKind::Header, _) => Action::None,
        _ => it.action.clone(),
    })
}

/// Trilha do slider dentro da linha (margens laterais), em px.
pub fn slider_track(rect: &Rect, layout: &Layout) -> Rect {
    let pad = layout.radius * 1.5;
    Rect { x: rect.x + pad, y: rect.y + rect.h * 0.62, w: rect.w - pad * 2.0, h: rect.h * 0.22 }
}

fn slider_fraction(rect: &Rect, x: f32, layout: &Layout) -> f32 {
    let t = slider_track(rect, layout);
    ((x - t.x) / t.w.max(1.0)).clamp(0.0, 1.0)
}

/// Arrasto sobre um slider já pressionado: a fração pela posição, mesmo fora da linha.
pub fn slide(layout: &Layout, index: usize, x: f32) -> Option<Action> {
    let it = layout.items.get(index)?;
    match (&it.kind, &it.action) {
        (ItemKind::Slider { .. }, Action::Slide(s, _)) => {
            Some(Action::Slide(*s, slider_fraction(&it.rect, x, layout)))
        }
        _ => None,
    }
}

/// Índice do item sob o ponto.
pub fn index_at(layout: &Layout, x: f32, y: f32) -> Option<usize> {
    layout.items.iter().position(|i| i.rect.contains(x, y))
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
            slots: [true, false, false],
            ..Default::default()
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
            for sc in [Screen::Start, Screen::Paused, Screen::Settings, Screen::Recents, Screen::States] {
                let l = layout(sc, w, h, &c, dpi, &state());
                assert!(!l.items.is_empty());
                fits(&l, w, h);
                // itens não se sobrepõem
                for (a, ia) in l.items.iter().enumerate() {
                    for ib in &l.items[a + 1..] {
                        let overlap = ia.rect.x < ib.rect.x + ib.rect.w - 0.5
                            && ib.rect.x < ia.rect.x + ia.rect.w - 0.5
                            && ia.rect.y < ib.rect.y + ib.rect.h - 0.5
                            && ib.rect.y < ia.rect.y + ia.rect.h - 0.5;
                        assert!(!overlap, "sobreposição em {sc:?} {w}x{h}: {} / {}", ia.label, ib.label);
                    }
                }
            }
        }
    }

    #[test]
    fn hit_returns_actions_and_touch_targets_are_big() {
        let c = Config::default();
        let l = layout(Screen::Paused, 1080.0, 2340.0, &c, 2.6, &state());
        let first = &l.items[0];
        assert_eq!(first.kind, ItemKind::Primary);
        assert_eq!(hit(&l, first.rect.x + 1.0, first.rect.y + 1.0), Some(Action::Resume));
        assert!(first.rect.h >= 48.0 * 2.6 * 0.9, "linha com pelo menos ~48 dp: {}", first.rect.h);
        assert!(l.font >= 16.0 * 2.6 * 0.85, "fonte de ~16 dp: {}", l.font);
        assert_eq!(hit(&l, 0.0, 0.0), None);
        let quick: Vec<_> =
            l.items.iter().filter(|i| matches!(i.action, Action::States { .. } | Action::Rewind)).collect();
        assert_eq!(quick.len(), 3, "linha de ações rápidas");
        assert!((quick[0].rect.y - quick[2].rect.y).abs() < 0.5, "na mesma linha");
        let l = layout(Screen::Recents, 1080.0, 2340.0, &c, 2.6, &state());
        let open = l.items.iter().find(|i| i.action == Action::OpenRecent(2)).unwrap();
        assert_eq!(hit(&l, open.rect.x + 2.0, open.rect.y + 2.0), Some(Action::OpenRecent(2)));
        let rm = l.items.iter().find(|i| i.action == Action::RemoveRecent(2)).unwrap();
        assert!(rm.rect.x > open.rect.x, "× à direita");
    }

    #[test]
    fn sliders_map_position_to_value() {
        let mut c = Config::default();
        let l = layout(Screen::Settings, 1080.0, 2340.0, &c, 2.6, &state());
        let vol = l.items.iter().position(|i| matches!(i.action, Action::Slide(Setting::Volume, _))).unwrap();
        let r = l.items[vol].rect;
        let t = slider_track(&r, &l);
        // toque no meio da trilha → 50 %
        match hit(&l, t.x + t.w / 2.0, r.y + r.h / 2.0) {
            Some(Action::Slide(Setting::Volume, f)) => {
                assert!((f - 0.5).abs() < 0.02, "{f}");
                set_fraction(&mut c, Setting::Volume, f);
                assert!((c.volume - 0.5).abs() < 0.03);
            }
            other => panic!("{other:?}"),
        }
        // arrasto além da borda satura
        match slide(&l, vol, 99999.0) {
            Some(Action::Slide(_, f)) => assert_eq!(f, 1.0),
            other => panic!("{other:?}"),
        }
        assert!(slide(&l, 0, 10.0).is_none() || matches!(slide(&l, 0, 10.0), Some(Action::Slide(..))));
        // tamanho dos botões: mínimo 60 %
        set_fraction(&mut c, Setting::TouchScale, 0.0);
        assert_eq!(c.touch_scale, 0.6);
        set_fraction(&mut c, Setting::TouchScale, 1.0);
        assert_eq!(c.touch_scale, 1.6);
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
        assert!((c.volume - 0.95).abs() < 1e-6);
        assert!(!c.high_contrast);
        adjust(&mut c, Setting::HighContrast, 0);
        assert!(c.high_contrast);
        assert_eq!(Setting::Volume.value(&c), "95%");
        assert_eq!(Setting::HighContrast.value(&c), "Sim");
    }

    #[test]
    fn states_screen_and_reset_confirmation() {
        let c = Config::default();
        let mut st = state();
        st.states_load = true;
        let l = layout(Screen::States, 1080.0, 2340.0, &c, 2.6, &st);
        assert_eq!(l.title, "Carregar state");
        let slot1 = l.items.iter().find(|i| i.action == Action::LoadSlot(1)).expect("slot 1 salvo carrega");
        assert_eq!(slot1.kind, ItemKind::Slot { filled: true });
        assert!(
            l.items.iter().any(|i| i.label.starts_with("Slot 2") && i.action == Action::None),
            "slot vazio não carrega"
        );
        st.states_load = false;
        let l = layout(Screen::States, 1080.0, 2340.0, &c, 2.6, &st);
        assert!(l.items.iter().any(|i| i.action == Action::SaveSlot(2)));
        st.confirm_reset = true;
        let l = layout(Screen::Paused, 1080.0, 2340.0, &c, 2.6, &st);
        let reset = l.items.iter().find(|i| i.action == Action::Reset).unwrap();
        assert!(reset.label.contains("Confirmar") && reset.active);
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
        assert_eq!(start.items[0].kind, ItemKind::Primary);
    }
}
