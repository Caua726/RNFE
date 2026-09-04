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
    /// Lista de controles (teclado, gamepad, toque) — só leitura.
    Controls,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Setting {
    /// Zapper (pistola de luz) na porta 2.
    Zapper,
    TouchScale,
    TouchOpacity,
    TouchAlways,
    TextScale,
    HighContrast,
    Haptics,
    IntegerScale,
    Volume,
    /// Esconde as 8 linhas de cima e de baixo (as TVs não mostravam).
    Overscan,
}

/// Seções da tela de ajustes (título + itens).
pub const SECTIONS: &[(&str, &[Setting])] = &[
    ("Som", &[Setting::Volume]),
    ("Vídeo", &[Setting::IntegerScale, Setting::Overscan]),
    ("Controles", &[Setting::Zapper]),
    ("Toque", &[Setting::TouchScale, Setting::TouchOpacity, Setting::TouchAlways, Setting::Haptics]),
    ("Acessibilidade", &[Setting::TextScale, Setting::HighContrast]),
];

impl Setting {
    pub fn label(self) -> &'static str {
        match self {
            Setting::TouchScale => "Tamanho dos botões",
            Setting::TouchOpacity => "Opacidade dos botões",
            Setting::TouchAlways => "Botões sempre visíveis",
            Setting::TextScale => "Tamanho do texto",
            Setting::HighContrast => "Alto contraste",
            Setting::Haptics => "Vibrar ao tocar",
            Setting::Zapper => "Zapper (mira com o toque)",
            Setting::IntegerScale => "Escala inteira (pixels quadrados)",
            Setting::Volume => "Volume",
            Setting::Overscan => "Cortar bordas (overscan)",
        }
    }

    pub fn is_bool(self) -> bool {
        matches!(
            self,
            Setting::TouchAlways
                | Setting::HighContrast
                | Setting::Haptics
                | Setting::Zapper
                | Setting::IntegerScale
                | Setting::Overscan
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
            Setting::Zapper => c.zapper as u8 as f32,
            Setting::IntegerScale => c.integer_scale as u8 as f32,
            Setting::Overscan => c.overscan as u8 as f32,
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
            Setting::Zapper => on(c.zapper),
            Setting::IntegerScale => on(c.integer_scale),
            Setting::Volume => pct(c.volume),
            Setting::Overscan => on(c.overscan),
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
        Setting::Zapper => c.zapper = v >= 0.5,
        Setting::IntegerScale => c.integer_scale = v >= 0.5,
        Setting::Overscan => c.overscan = v >= 0.5,
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
    /// Abre a lista de controles.
    Controls,
    /// Grava o frame atual em PNG no Storage (`shots/`).
    Screenshot,
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
    /// Geometria do cabeçalho (px): onde o título e o subtítulo são desenhados e onde o
    /// conteúdo pode começar. Fica aqui para o desenho e o layout nunca discordarem.
    pub title_y: f32,
    pub title_size: f32,
    pub subtitle_y: f32,
    pub subtitle_size: f32,
    /// Primeira linha livre abaixo do cabeçalho.
    pub header_h: f32,
    /// Raio dos cantos e margem interna, em px.
    pub radius: f32,
    /// Altura total do conteúdo (px): se maior que a janela, a tela rola.
    pub content_h: f32,
}

/// O que o menu precisa saber do aplicativo.
#[derive(Debug, Clone, Default)]
pub struct MenuState {
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
    /// O seletor de ROM está aberto (o item "Abrir ROM" vira "Abrindo…").
    pub loading: bool,
    /// Remoção de recente aguardando confirmação (hash).
    pub confirm_remove: Option<u64>,
    /// A plataforma tem tela de toque (mostra a seção Toque dos ajustes).
    pub touch_platform: bool,
    /// Há vibração (senão o ajuste "Vibrar ao tocar" some).
    pub has_haptics: bool,
    /// Captura de tela para arquivo faz sentido (desktop).
    pub can_screenshot: bool,
}

/// Nome de exibição de uma ROM: sem a extensão `.nes`.
pub fn display_name(name: &str) -> &str {
    name.strip_suffix(".nes").or_else(|| name.strip_suffix(".NES")).unwrap_or(name)
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
    /// Encolhe as linhas para caber em `h`, mas nunca abaixo de `min_row` (≈ 2,4× a fonte):
    /// abaixo disso o texto não cabe — a tela rola em vez de espremer.
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
    let min_row = 2.4 * font;
    let mut items = Vec::new();
    let title: String;
    let subtitle: String;
    // Cabeçalho: na tela inicial o título mora dentro de um cartucho; nas outras é uma linha.
    let home = matches!(screen, Screen::Start | Screen::Playing);
    let title_size = if home { 64.0 * s } else { 30.0 * s };
    let title_y = if home { h * 0.17 } else { h * 0.05 };
    let subtitle_size = (15.0 * s).max(13.0);
    let subtitle_y = if home {
        title_y - title_size * 0.12 + title_size * 1.35 + 10.0 * s
    } else {
        title_y + title_size + 8.0 * s
    };
    let header_h = subtitle_y + subtitle_size * 1.4 + gap;
    let mut col = Column { x, w: bw, y: 0.0, gap, row_h };
    match screen {
        Screen::Start | Screen::Playing => {
            title = "RNFE".into();
            subtitle = if st.version.is_empty() {
                "Famicom / NES".into()
            } else {
                format!("Famicom / NES · v{}", st.version)
            };
            col.y = (h * 0.40).max(header_h);
            // Quem já jogou algo volta direto ao último jogo (o auto-state retoma de onde parou)
            let resume = if st.loading { None } else { st.recent.first() };
            let rows = 3.0
                + (!st.recent.is_empty()) as u8 as f32
                + resume.is_some() as u8 as f32
                + st.can_quit as u8 as f32;
            col.fit(rows, h, min_row);
            if let Some(r) = resume {
                col.push(
                    &mut items,
                    format!("Continuar · {}", display_name(&r.name)),
                    Action::OpenRecent(r.hash),
                    ItemKind::Primary,
                );
            }
            if st.loading {
                col.push(&mut items, "Abrindo…", Action::None, ItemKind::Button);
            } else {
                let kind = if resume.is_some() { ItemKind::Button } else { ItemKind::Primary };
                col.push(&mut items, "Abrir ROM", Action::OpenRom, kind);
            }
            if !st.recent.is_empty() {
                col.push(
                    &mut items,
                    format!("Recentes ({})", st.recent.len()),
                    Action::Recents,
                    ItemKind::Button,
                );
            }
            col.push(&mut items, "Ajustes", Action::Settings, ItemKind::Button);
            col.push(&mut items, "Controles", Action::Controls, ItemKind::Button);
            if st.can_quit {
                col.push(&mut items, "Sair", Action::Quit, ItemKind::Button);
            }
        }
        Screen::Paused => {
            title = "Pausado".into();
            let mins = st.play_seconds / 60;
            let name = display_name(&st.rom_name);
            subtitle = if mins > 0 { format!("{name} · {mins} min") } else { name.to_string() };
            col.y = header_h.max(h * 0.10);
            let rows = 7.0 + st.can_quit as u8 as f32 + st.can_screenshot as u8 as f32;
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
            if st.loading {
                col.push(&mut items, "Abrindo…", Action::None, ItemKind::Button);
            } else {
                col.push(&mut items, "Abrir outra ROM", Action::OpenRom, ItemKind::Button);
            }
            if st.can_screenshot {
                col.push(&mut items, "Captura de tela (F12)", Action::Screenshot, ItemKind::Button);
            }
            col.push(&mut items, "Ajustes", Action::Settings, ItemKind::Button);
            col.push(&mut items, "Controles", Action::Controls, ItemKind::Button);
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
            col.y = header_h;
            col.row_h = 54.0 * s;
            let sec_h = 30.0 * s;
            for (name, settings) in SECTIONS {
                if *name == "Toque" && !st.touch_platform {
                    continue;
                }
                items.push(item(
                    Rect { x, y: col.y, w: bw, h: sec_h },
                    *name,
                    Action::None,
                    ItemKind::Header,
                ));
                col.y += sec_h + gap * 0.5;
                for &set in *settings {
                    if set == Setting::Haptics && !st.has_haptics {
                        continue;
                    }
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
                col.y += gap;
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
            col.y = header_h;
            col.fit(st.recent.len() as f32 + 1.0, h, min_row);
            let side = col.row_h;
            for r in &st.recent {
                let y = col.y;
                let confirming = st.confirm_remove == Some(r.hash);
                let rm_w = if confirming { side * 2.2 } else { side };
                items.push(item(
                    Rect { x, y, w: bw - rm_w - gap, h: col.row_h },
                    display_name(&r.name).to_string(),
                    Action::OpenRecent(r.hash),
                    ItemKind::Button,
                ));
                let mut rm = item(
                    Rect { x: x + bw - rm_w, y, w: rm_w, h: col.row_h },
                    if confirming { "Apagar?" } else { "×" },
                    Action::RemoveRecent(r.hash),
                    ItemKind::Danger,
                );
                rm.active = confirming;
                items.push(rm);
                col.y += col.row_h + gap;
            }
            col.push(&mut items, "Voltar", Action::Back, ItemKind::Button);
        }
        Screen::Controls => {
            title = "Controles".into();
            subtitle = "toque em Voltar quando terminar".into();
            col.y = header_h;
            col.row_h = 46.0 * s;
            let rows: &[(&str, &str)] = if st.touch_platform {
                &[
                    ("D-pad", "canto inferior esquerdo"),
                    ("A / B", "círculos à direita"),
                    ("START / SELECT", "pílulas embaixo"),
                    ("Menu", "botão MENU ou START+SELECT"),
                    ("Gamepad", "Bluetooth: d-pad, A/B, Start/Select"),
                    ("Jogador 2", "2º gamepad pareado"),
                ]
            } else {
                &[
                    ("D-pad", "setas ou W A S D"),
                    ("A / B", "Z / X"),
                    ("START / SELECT", "Enter / Tab"),
                    ("Menu", "Esc (ou START+SELECT no controle)"),
                    ("Jogador 2", "I J K L · O / U · . / ,"),
                    ("State", "F5 salva · F7 carrega (slot 1)"),
                    ("Voltar 5 s", "Backspace (segurar)"),
                    ("Turbo", "Espaço (segurar)"),
                    ("Captura de tela", "F12"),
                    ("Abrir ROM", "O, ou arraste o arquivo"),
                    ("Reset", "R duas vezes"),
                    ("Tela cheia", "F11"),
                ]
            };
            col.fit(rows.len() as f32 + 1.0, h, min_row);
            for (what, how) in rows {
                let i = col.push(&mut items, *what, Action::None, ItemKind::Button);
                items[i].value = (*how).to_string();
            }
            col.push(&mut items, "Voltar", Action::Back, ItemKind::Button);
        }
        Screen::States => {
            title = if st.states_load { "Carregar state" } else { "Salvar state" }.into();
            subtitle = st.rom_name.clone();
            col.y = header_h;
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
    let content_h = items.iter().map(|i| i.rect.y + i.rect.h).fold(0.0, f32::max) + gap;
    Layout {
        title,
        subtitle,
        items,
        ui_scale: s,
        font,
        title_y,
        title_size,
        subtitle_y,
        subtitle_size,
        header_h,
        radius: 12.0 * s,
        content_h,
    }
}

/// Itens que podem ser selecionados por teclado/gamepad (têm ação).
pub fn selectable(layout: &Layout) -> Vec<usize> {
    layout
        .items
        .iter()
        .enumerate()
        .filter(|(_, i)| i.action != Action::None && i.kind != ItemKind::Header)
        .map(|(i, _)| i)
        .collect()
}

/// Próximo item selecionável na direção `dir` (+1 baixo, −1 cima), com volta nas pontas.
pub fn next_selectable(layout: &Layout, current: Option<usize>, dir: i32) -> Option<usize> {
    let sel = selectable(layout);
    if sel.is_empty() {
        return None;
    }
    let Some(cur) = current else { return Some(if dir >= 0 { sel[0] } else { sel[sel.len() - 1] }) };
    let pos = sel.iter().position(|&i| i == cur).unwrap_or(0) as i32;
    let n = sel.len() as i32;
    Some(sel[((pos + dir) % n + n) as usize % sel.len()])
}

/// Ação de "ativar" o item selecionado (Enter/A); para sliders, `dir` ajusta em passos.
pub fn activate(layout: &Layout, index: usize, dir: i32) -> Option<Action> {
    let it = layout.items.get(index)?;
    match (&it.kind, &it.action) {
        (ItemKind::Slider { .. }, Action::Slide(s, _)) => {
            if dir == 0 {
                None
            } else {
                Some(Action::Adjust(*s, dir.signum() as i8))
            }
        }
        (_, Action::None) => None,
        _ => {
            if dir == 0 {
                Some(it.action.clone())
            } else {
                None
            }
        }
    }
}

/// Ação do item sob o ponto, se houver. Para sliders, a fração vem da posição horizontal.
pub fn hit(layout: &Layout, x: f32, y: f32) -> Option<Action> {
    let it = layout.items.iter().find(|i| i.rect.contains(x, y))?;
    Some(match (&it.kind, &it.action) {
        (ItemKind::Slider { .. }, Action::Slide(s, _)) => {
            // só a trilha (com a folga do knob) muda o valor; o resto da linha é área de rolagem
            let t = slider_track(&it.rect, layout);
            let grab = Rect { x: t.x - t.h, y: t.y - t.h * 1.6, w: t.w + t.h * 2.0, h: t.h * 4.2 };
            if !grab.contains(x, y) {
                return Some(Action::None);
            }
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
            rom_name: "jogo.nes".into(),
            turbo: false,
            can_quit: true,
            recent: vec![RecentRom { hash: 1, name: "a".into() }, RecentRom { hash: 2, name: "b".into() }],
            version: "0.2.0".into(),
            slots: [true, false, false],
            touch_platform: true,
            has_haptics: true,
            can_screenshot: true,
            ..Default::default()
        }
    }

    fn fits(l: &Layout, w: f32, h: f32) {
        for i in &l.items {
            assert!(i.rect.x >= 0.0 && i.rect.y >= 0.0, "{:?}", i);
            assert!(i.rect.x + i.rect.w <= w + 0.5, "{} sai da tela {w}x{h}: {:?}", i.label, i.rect);
            // verticalmente pode passar: a tela rola (content_h diz quanto)
            assert!(i.rect.y + i.rect.h <= l.content_h + 0.5);
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

    /// O cabeçalho (título + subtítulo) não pode ser coberto pela primeira linha, em nenhum
    /// tamanho de tela nem com o texto ampliado.
    #[test]
    fn header_never_overlaps_items() {
        for scale in [0.8f32, 1.0, 1.6] {
            let c = Config { text_scale: scale, ..Config::default() };
            for (w, h, dpi) in [
                (1080.0, 2340.0, 2.6),
                (2340.0, 1080.0, 2.6),
                (768.0, 720.0, 1.0),
                (480.0, 320.0, 1.0),
                (1440.0, 3120.0, 3.5),
            ] {
                for sc in [Screen::Start, Screen::Paused, Screen::Settings, Screen::Recents, Screen::States] {
                    let l = layout(sc, w, h, &c, dpi, &state());
                    let top = l.items.iter().map(|i| i.rect.y).fold(f32::MAX, f32::min);
                    assert!(
                        top >= l.header_h - 0.5,
                        "{sc:?} {w}x{h} texto {scale}: item em {top} sob o cabeçalho ({})",
                        l.header_h
                    );
                }
            }
        }
    }

    /// Tocar no rótulo de um slider não muda o valor: só a trilha responde (o resto rola).
    #[test]
    fn slider_only_reacts_on_its_track() {
        let c = Config::default();
        let l = layout(Screen::Settings, 1080.0, 2340.0, &c, 2.6, &state());
        let (i, item) = l
            .items
            .iter()
            .enumerate()
            .find(|(_, it)| matches!(it.kind, ItemKind::Slider { .. }))
            .expect("um slider");
        let t = slider_track(&item.rect, &l);
        assert!(hit(&l, t.x + t.w * 0.5, t.y + t.h * 0.5).is_some());
        assert!(matches!(hit(&l, item.rect.x + 4.0, item.rect.y + 4.0), Some(Action::None)));
        // arrastar continua funcionando pelo índice do item
        assert!(matches!(slide(&l, i, t.x + t.w), Some(Action::Slide(..))));
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
    fn keyboard_navigation_and_scroll_content() {
        let c = Config::default();
        let l = layout(Screen::Settings, 2340.0, 1080.0, &c, 2.6, &state());
        assert!(l.content_h > 1080.0, "ajustes em paisagem rolam em vez de espremer: {}", l.content_h);
        for i in l.items.iter().filter(|i| i.kind != ItemKind::Header) {
            assert!(i.rect.h >= 2.0 * l.font, "linha alta o bastante para o texto: {}", i.label);
        }
        let first = next_selectable(&l, None, 1).unwrap();
        let second = next_selectable(&l, Some(first), 1).unwrap();
        assert!(second > first);
        assert_eq!(
            next_selectable(&l, Some(first), -1),
            Some(*selectable(&l).last().unwrap()),
            "volta nas pontas"
        );
        // slider: Enter não faz nada, ←/→ ajusta
        assert_eq!(activate(&l, first, 0), None);
        assert!(matches!(activate(&l, first, 1), Some(Action::Adjust(_, 1))));
        let back = l.items.iter().position(|i| i.action == Action::Back).unwrap();
        assert_eq!(activate(&l, back, 0), Some(Action::Back));
        // carregando: "Abrir ROM" vira inerte
        let mut st = state();
        st.loading = true;
        let l = layout(Screen::Start, 1080.0, 2340.0, &c, 2.6, &st);
        assert!(l.items.iter().any(|i| i.label == "Abrindo…" && i.action == Action::None));
        assert!(!selectable(&l).contains(&0));
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
