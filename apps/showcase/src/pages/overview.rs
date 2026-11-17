use ratatui::buffer::Buffer;
use ratatui::layout::Rect;
use ratatui::style::{Color, Modifier, Style};

use crate::pages::layout::Sections;
use crate::pages::template::{Section, Template};
use crate::pages::{Cx, Hint, Page, PageEvent};
use junie_tui::core::event::Outcome;
use junie_tui::core::id::WidgetId;
use junie_tui::theme::{
    BadgeKind, ButtonKind, ColorLevel, CursorStyle, EnvKind, Glyphs, Insets, Kind, Ladder,
    LevelOverrides, Plane, StateModifiers, StatusKind, Strings, SyntaxTone, Theme, Tone,
    contrast_ratio,
};
use junie_tui::ui::ctx::{RenderCtx, VisualState, fill};
use junie_tui::ui::layout::StackChild;
use junie_tui::ui::specimen::{KindSample, SpecimenState, StateGrid};
use junie_tui::widgets::panel::Panel;
use junie_tui::widgets::props::{self, Prop};

const ID: WidgetId = WidgetId::of("overview");

const PLANES: [(Plane, &str); 10] = [
    (Plane::Canvas, "Canvas"),
    (Plane::Surface, "Surface"),
    (Plane::Card, "Card"),
    (Plane::Field, "Field"),
    (Plane::Raised, "Raised"),
    (Plane::Popover, "Popover"),
    (Plane::Highlight, "Highlight"),
    (Plane::HighlightDanger, "HighlightDanger"),
    (Plane::Accent, "Accent"),
    (Plane::Danger, "Danger"),
];

const KINDS: [(Kind, &str); 15] = [
    (Kind::Row, "Row"),
    (Kind::CursorRow(CursorStyle::Fill), "Cursor fill"),
    (Kind::CursorRow(CursorStyle::Bar), "Cursor bar"),
    (Kind::Button(ButtonKind::Primary), "Primary"),
    (Kind::Button(ButtonKind::Secondary), "Secondary"),
    (Kind::Button(ButtonKind::Subtle), "Subtle"),
    (Kind::Button(ButtonKind::Danger), "Danger"),
    (Kind::Button(ButtonKind::Toggle), "Toggle"),
    (Kind::Tab, "Tab"),
    (Kind::Title, "Title"),
    (Kind::Chip, "Chip"),
    (Kind::Close, "Close"),
    (Kind::Field, "Field"),
    (Kind::Handle, "Handle"),
    (Kind::Seam, "Seam"),
];

const DEFAULT_PLANE: usize = 2;
const DEFAULT_KIND: usize = 4;

const CARD_MIN: u16 = 50;

const GAP: u16 = 1;

pub struct OverviewPage {
    template: Template,
    sections: Sections,

    plane: usize,
    kind: usize,
    ladder: usize,
    mods: usize,
}

impl OverviewPage {
    pub fn new() -> Self {
        Self {
            template: Template::new(ID, states_grid(PLANES[DEFAULT_PLANE].0)),
            sections: Sections::new(ID.sub("cards")),
            plane: DEFAULT_PLANE,
            kind: DEFAULT_KIND,
            ladder: 0,
            mods: 0,
        }
    }

    fn explorer_theme(&self, t: &Theme) -> Theme {
        let plane = PLANES[self.plane].0;
        let mut th = *t;
        if self.ladder == 1 {
            th = th.with_ladder(
                plane,
                Ladder::new(t.accent_bg_subtle, t.accent_bg, t.accent_bg_hover),
            );
        }
        match self.mods {
            1 => th.with_state_modifiers(StateModifiers::COLOR),
            2 => th.with_state_modifiers(StateModifiers::ANSI16),
            3 => th.with_state_modifiers(StateModifiers::MONO),
            _ => th,
        }
    }

    fn bands(width: u16) -> (u16, u16, Vec<&'static [Card]>, Vec<u16>) {
        let cols = ((width + 1) / (CARD_MIN + 1)).clamp(1, 3);
        let card_w = (width.saturating_sub(GAP * (cols - 1))) / cols;
        let bands: Vec<&[Card]> = CARDS.chunks(cols as usize).collect();
        let heights: Vec<u16> = bands
            .iter()
            .map(|b| b.iter().map(|c| c.height(card_w)).max().unwrap_or(0))
            .collect();
        (cols, card_w, bands, heights)
    }

    fn render_playground(&mut self, area: Rect, buf: &mut Buffer, ctx: &mut RenderCtx) {
        let t = ctx.theme;
        let gap = GAP;
        let (cols, card_w, bands, heights) = Self::bands(area.width);
        let children: Vec<StackChild> = heights.iter().map(|&h| StackChild::fixed(h)).collect();
        let l = self.sections.layout(area, &children, None, buf, ctx);
        let xt = self.explorer_theme(t);
        for (i, band) in bands.iter().enumerate() {
            let Some(slot) = l.slot(i) else { continue };
            for (k, card) in band.iter().enumerate() {
                let x = slot.x + k as u16 * (card_w + gap);
                let w = if k as u16 == cols - 1 {
                    slot.right().saturating_sub(x)
                } else {
                    card_w
                };
                let r = Rect::new(x, slot.y, w, card.height(card_w).min(slot.height));
                self.draw_card(*card, r, buf, t, &xt);
            }
        }
    }

    fn draw_card(&self, card: Card, area: Rect, buf: &mut Buffer, t: &Theme, xt: &Theme) {
        if card == Card::Roles {
            render_roles(area, buf, t);
            return;
        }
        let panel = Panel::card(Some(card.title()));
        let bg = panel.bg(t);
        let inner = panel.render(area, buf, t);
        if inner.is_empty() {
            return;
        }
        match card {
            Card::Planes => render_swatches(inner, buf, t, bg, &planes_and_text(t)),
            Card::Accents => render_swatches(inner, buf, t, bg, &accent_and_status(t)),
            Card::Glyphs => render_glyphs(inner, buf, t, bg),
            Card::Ladders => render_ladders(inner, buf, t, bg),
            Card::Contrast => render_contrast(inner, buf, t, bg),
            Card::Principles => render_principles(inner, buf, t, bg),
            Card::Explorer => self.render_explorer(inner, buf, t, xt, bg),
            Card::Mods => render_mods(inner, buf, t, bg),
            Card::Settings => render_settings(inner, buf, t, bg),
            Card::Legend => render_legend(inner, buf, t, bg),
            Card::Spacing => render_spacing(inner, buf, t, bg),
            Card::Roles => {}
        }
    }

    fn render_explorer(&self, area: Rect, buf: &mut Buffer, t: &Theme, xt: &Theme, bg: Color) {
        let (plane, plane_name) = PLANES[self.plane];
        let (kind, kind_name) = KINDS[self.kind];
        let lines = [
            "Theme::interactive(kind, state, plane) → Interactive".to_owned(),
            format!("plane {plane_name} · kind {kind_name}"),
            format!(
                "ladder {} · modifiers {}",
                ["theme", "custom (with_ladder)"][self.ladder],
                ["level", "COLOR", "ANSI16", "MONO"][self.mods]
            ),
        ];
        for (i, l) in lines.iter().enumerate() {
            let style = if i == 0 { t.faint() } else { t.secondary() };
            put(buf, area, i as u16, 0, l, style.bg(bg));
        }
        let sample = KindSample::new(ID.sub("explorer"), kind, plane, "Sample");
        let v = VisualState::default();
        let states: [(&str, VisualState); 6] = [
            ("rest", v),
            ("hover", VisualState { hovered: true, ..v }),
            (
                "pressed",
                VisualState {
                    hovered: true,
                    pressed: true,
                    ..v
                },
            ),
            (
                "selected",
                VisualState {
                    selected: true,
                    ..v
                },
            ),
            (
                "disabled",
                VisualState {
                    disabled: true,
                    ..v
                },
            ),
            ("error", VisualState { error: true, ..v }),
        ];
        for (i, (label, s)) in states.iter().enumerate() {
            let row = 4 + i as u16;
            if row >= area.height {
                break;
            }
            put(buf, area, row, 0, label, t.muted().bg(bg));
            let x = area.x + 10;
            let w = area.right().saturating_sub(x).min(14);
            sample.draw(xt, *s, Rect::new(x, area.y + row, w, 1), buf);
        }
    }

    fn set_plane(&mut self, i: usize) {
        self.plane = i.min(PLANES.len() - 1);
        self.template.grid = states_grid(PLANES[self.plane].0);
    }
}

fn states_grid(plane: Plane) -> StateGrid {
    use SpecimenState::*;
    let id = ID.sub("states");
    let mut g = StateGrid::new(id)
        .rows(&[
            Rest,
            Hover,
            Pressed,
            Focused,
            FocusedHover,
            Selected,
            Disabled,
            Error,
        ])
        .label_width(14)
        .column_width(12);
    for (i, (kind, label)) in KINDS.iter().enumerate() {
        g = g.column(label, KindSample::new(id.child(i), *kind, plane, "Aa"));
    }
    g
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Card {
    Planes,
    Accents,
    Glyphs,
    Ladders,
    Contrast,
    Principles,
    Roles,
    Explorer,
    Mods,
    Settings,
    Legend,
    Spacing,
}

const CARDS: [Card; 12] = [
    Card::Planes,
    Card::Accents,
    Card::Glyphs,
    Card::Ladders,
    Card::Contrast,
    Card::Principles,
    Card::Roles,
    Card::Explorer,
    Card::Mods,
    Card::Settings,
    Card::Legend,
    Card::Spacing,
];

impl Card {
    fn title(self) -> &'static str {
        match self {
            Card::Planes => "Tokens · planes and text",
            Card::Accents => "Tokens · accent and status",
            Card::Glyphs => "Glyphs and strings",
            Card::Ladders => "Ladders · theme.ladders",
            Card::Contrast => "Contrast on the card, per level",
            Card::Principles => "Principles",
            Card::Roles => "Roles",
            Card::Explorer => "Explorer",
            Card::Mods => "State modifiers · theme.mods",
            Card::Settings => "Theme settings",
            Card::Legend => "State language",
            Card::Spacing => "Spacing · buttons · badges",
        }
    }

    fn height(self, width: u16) -> u16 {
        let body = match self {
            Card::Planes | Card::Accents => 20,
            Card::Glyphs => 14,
            Card::Ladders => 13,
            Card::Contrast => 12,
            Card::Principles => {
                let w = width.saturating_sub(2 * junie_tui::widgets::panel::CARD_PAD) as usize;
                PRINCIPLES
                    .iter()
                    .map(|(title, text)| {
                        junie_tui::ui::text::wrap(&format!("{title}. {text}"), w.max(1)).len()
                            as u16
                    })
                    .sum()
            }
            Card::Roles => return 12,
            Card::Explorer => 10,
            Card::Mods => 10,
            Card::Settings => 8,
            Card::Legend => 7,
            Card::Spacing => 9,
        };
        body + 3
    }
}

fn put(buf: &mut Buffer, area: Rect, row: u16, col: u16, text: &str, style: Style) {
    if row >= area.height || col >= area.width {
        return;
    }
    let room = (area.width - col) as usize;
    let text = junie_tui::ui::text::truncate(text, room);
    buf.set_string(area.x + col, area.y + row, text, style);
}

fn planes_and_text(t: &Theme) -> Vec<(&'static str, Color)> {
    vec![
        ("canvas", t.canvas),
        ("surface", t.surface),
        ("surface.elevated", t.surface_elevated),
        ("surface.overlay", t.surface_overlay),
        ("field", t.field),
        ("field.hover", t.field_hover),
        ("popover", t.popover),
        ("highlight", t.highlight),
        ("highlight.danger", t.highlight_danger),
        ("selection", t.selection_bg),
        ("border.subtle", t.border_subtle),
        ("border.strong", t.border_strong),
        ("scroll.track", t.scroll_track),
        ("text.primary", t.text_primary),
        ("text.secondary", t.text_secondary),
        ("text.muted", t.text_muted),
        ("text.faint", t.text_faint),
        ("text.ghost", t.text_ghost),
        ("text.on_accent", t.text_on_accent),
        ("disabled", t.disabled),
    ]
}

fn accent_and_status(t: &Theme) -> Vec<(&'static str, Color)> {
    vec![
        ("accent", t.accent),
        ("accent.hover", t.accent_hover),
        ("accent.pressed", t.accent_pressed),
        ("accent.bg", t.accent_bg),
        ("accent.bg.subtle", t.accent_bg_subtle),
        ("accent.bg.hover", t.accent_bg_hover),
        ("focus", t.focus),
        ("link", t.link),
        ("success", t.success),
        ("success.strong", t.success_strong),
        ("info", t.info),
        ("warning", t.warning),
        ("error", t.error),
        ("error.bg", t.error_bg),
        ("error.soft", t.error_soft),
        ("error.strong", t.error_strong),
        ("danger", t.danger_label),
        ("syntax.keyword", t.syntax_keyword),
        ("syntax.string", t.syntax_string),
        ("syntax.literal", t.syntax_literal),
    ]
}

fn swatch_note(c: Color) -> String {
    match c {
        Color::Rgb(r, g, b) => format!("#{r:02x}{g:02x}{b:02x}"),
        Color::Indexed(i) => format!("256: {i}"),
        other => format!("{other:?}"),
    }
}

fn render_swatches(area: Rect, buf: &mut Buffer, t: &Theme, bg: Color, all: &[(&str, Color)]) {
    for (i, (name, color)) in all.iter().enumerate() {
        let row = i as u16;
        if row >= area.height {
            break;
        }
        let y = area.y + row;
        fill(
            buf,
            Rect::new(area.x, y, 4.min(area.width), 1),
            Style::new().bg(*color),
        );
        put(buf, area, row, 4, "▏", t.faint().bg(bg));
        put(buf, area, row, 6, name, t.primary().bg(bg));
        let note = swatch_note(*color);
        let nw = junie_tui::ui::text::width(&note) as u16;
        let nx = area.width.saturating_sub(nw);
        if nx > 6 + junie_tui::ui::text::width(name) as u16 {
            put(buf, area, row, nx, &note, t.muted().bg(bg));
        }
    }
}

fn render_ladders(area: Rect, buf: &mut Buffer, t: &Theme, bg: Color) {
    put(
        buf,
        area,
        0,
        0,
        "Ladder: rest hover pressed · Plane::up",
        t.muted().bg(bg),
    );
    for (i, (plane, name)) in PLANES.iter().enumerate() {
        let row = 1 + i as u16;
        put(buf, area, row, 0, name, t.primary().bg(bg));
        let l = t.ladder(*plane);
        for (k, c) in [l.rest, l.hover, l.pressed].into_iter().enumerate() {
            let x = 16 + k as u16 * 5;
            if x + 4 <= area.width {
                fill(
                    buf,
                    Rect::new(area.x + x, area.y + row, 4, 1),
                    Style::new().bg(c),
                );
            }
        }
        let up = PLANES
            .iter()
            .find(|(p, _)| *p == plane.up())
            .map_or("", |(_, n)| n);
        put(buf, area, row, 32, &format!("→ {up}"), t.muted().bg(bg));
    }

    let inks = [
        ("handle", t.ladders.handle, t.glyphs.scroll_thumb),
        ("seam", t.ladders.seam, t.glyphs.divider),
    ];
    for (i, (name, l, g)) in inks.into_iter().enumerate() {
        let row = 11 + i as u16;
        put(buf, area, row, 0, name, t.primary().bg(bg));
        for (k, c) in [l.rest, l.hover, l.pressed].into_iter().enumerate() {
            put(
                buf,
                area,
                row,
                17 + k as u16 * 5,
                g,
                Style::new().fg(c).bg(bg),
            );
        }
    }
}

fn render_contrast(area: Rect, buf: &mut Buffer, t: &Theme, bg: Color) {
    type Ink = fn(&Theme) -> Color;
    let inks: [(&str, Ink); 10] = [
        ("text.primary", |t| t.text_primary),
        ("text.secondary", |t| t.text_secondary),
        ("text.muted", |t| t.text_muted),
        ("text.faint", |t| t.text_faint),
        ("accent", |t| t.accent),
        ("link", |t| t.link),
        ("success", |t| t.success),
        ("warning", |t| t.warning),
        ("error", |t| t.error),
        ("info", |t| t.info),
    ];
    let levels = [
        (ColorLevel::TrueColor, "true"),
        (ColorLevel::Ansi256, "256"),
        (ColorLevel::Ansi16, "16"),
        (ColorLevel::Mono, "mono"),
    ];
    let col = |k: usize| 15 + k as u16 * 8;
    put(buf, area, 0, 0, "contrast_ratio", t.muted().bg(bg));
    for (k, (_, name)) in levels.iter().enumerate() {
        put(buf, area, 0, col(k), name, t.muted().bg(bg));
    }
    let themes: Vec<Theme> = levels.iter().map(|(l, _)| Theme::for_level(*l)).collect();
    for (i, (name, ink)) in inks.iter().enumerate() {
        let row = 1 + i as u16;
        put(buf, area, row, 0, name, t.primary().bg(bg));
        for (k, th) in themes.iter().enumerate() {
            let card = th.surface_elevated;
            let fg = ink(th);
            let ratio = contrast_ratio(fg, card).unwrap_or(0.0);
            let text = if ratio >= 4.5 {
                ratio_label(ratio)
            } else {
                let up = contrast_ratio(th.legible(fg, card), card).unwrap_or(0.0);
                format!("{}→{}", ratio_label(ratio), ratio_label(up))
            };
            let style = if ratio >= 4.5 {
                t.secondary()
            } else {
                t.muted()
            };
            put(buf, area, row, col(k), &text, style.bg(bg));
        }
    }
    put(
        buf,
        area,
        11,
        0,
        "a→b: under 4.5:1, Theme::legible steps it up",
        t.faint().bg(bg),
    );
}

fn ratio_label(r: f64) -> String {
    if r >= 10.0 {
        format!("{r:.0}")
    } else {
        format!("{r:.1}")
    }
}

fn render_principles(area: Rect, buf: &mut Buffer, t: &Theme, bg: Color) {
    let mut row = 0;
    for (title, text) in PRINCIPLES {
        let lines = junie_tui::ui::text::wrap(&format!("{title}. {text}"), area.width as usize);
        for (k, l) in lines.iter().enumerate() {
            put(buf, area, row, 0, l, t.secondary().bg(bg));
            if k == 0 {
                let head = format!("{title}.");
                let bold = t.primary().bg(bg).add_modifier(Modifier::BOLD);
                put(buf, area, row, 0, &head, bold);
            }
            row += 1;
        }
    }
}

fn render_mods(area: Rect, buf: &mut Buffer, t: &Theme, bg: Color) {
    let m = t.mods;
    put(
        buf,
        area,
        0,
        0,
        &format!("StateModifiers at {}", t.level.label()),
        t.muted().bg(bg),
    );
    let rows: [(&str, Modifier); 9] = [
        ("hover", m.hover),
        ("hover_flat", m.hover_flat),
        ("pressed", m.pressed),
        ("fill", m.fill),
        ("pressed_fill", m.pressed_fill),
        ("focused", m.focused),
        ("disabled", m.disabled),
        ("thumb_hover", m.thumb_hover),
        ("thumb_pressed", m.thumb_pressed),
    ];
    for (i, (name, md)) in rows.iter().enumerate() {
        let row = 1 + i as u16;
        put(buf, area, row, 0, name, t.primary().bg(bg));
        let value = if md.is_empty() {
            "—".to_owned()
        } else {
            format!("{md:?}")
        };
        put(buf, area, row, 15, &value, t.muted().bg(bg));
        let sample = t.primary().bg(bg).add_modifier(*md);
        put(
            buf,
            area,
            row,
            area.width.saturating_sub(6),
            "Sample",
            sample,
        );
    }
}

fn render_settings(area: Rect, buf: &mut Buffer, t: &Theme, bg: Color) {
    props::render(area, buf, t, &settings(t), bg);
}

fn settings(t: &Theme) -> Vec<Prop> {
    let h = t.hintbar;
    let on = |b: bool| if b { "on" } else { "off" };
    let mono = Theme::for_level(ColorLevel::Mono);
    let ascii = Theme::for_level_with(ColorLevel::Mono, &LevelOverrides::ASCII_MONO);
    let sample = |th: &Theme| {
        let g = th.glyphs;
        format!(
            "{} {} {} {}",
            g.check_on, g.chevron_closed, g.ellipsis, g.done
        )
    };
    vec![
        Prop::new("theme.level", t.level.label()),
        Prop::new("theme.tint_selection", on(t.tint_selection)),
        Prop::new(
            "theme.hintbar",
            format!(
                "{:?} · status ≥ {}% · compact {}",
                h.align,
                h.status_min_share,
                on(h.compact)
            ),
        ),
        Prop::new("pinned keys", h.pinned_keys.join(" ")),
        Prop::new("edit entry", format!("{:?}", t.edit_entry)),
        Prop::new("for_level(Mono)", sample(&mono)),
        Prop::new("+ ASCII_MONO", sample(&ascii)),
    ]
}

fn render_legend(area: Rect, buf: &mut Buffer, t: &Theme, bg: Color) {
    let legend: [(&str, &str, Style); 7] = [
        ("▏", "focus: the bar (depicted)", t.accent_fg()),
        ("░", "hover lifts the surface", t.secondary()),
        ("›", "current / chosen", t.primary()),
        ("✓", "checked", t.primary()),
        ("!", "error", t.error_fg()),
        ("▁", "editing: cursor + underline", t.primary()),
        ("○", "disabled: faint, no hover", t.faint()),
    ];
    for (i, (g, text, st)) in legend.iter().enumerate() {
        put(buf, area, i as u16, 0, g, st.bg(bg));
        put(buf, area, i as u16, 3, text, t.secondary().bg(bg));
    }
}

fn render_spacing(area: Rect, buf: &mut Buffer, t: &Theme, bg: Color) {
    let s = t.spacing;
    let ins = |i: Insets| format!("Insets {{ x: {}, y: {} }}", i.x, i.y);
    let list = [
        Prop::new("theme.spacing", format!("button_gap {}", s.button_gap)),
        Prop::new("dialog", ins(s.dialog)),
        Prop::new("panel", ins(s.panel)),
        Prop::new("popup", ins(s.popup)),
        Prop::new("chrome_edge", s.chrome_edge.to_string()),
    ];
    let used = props::render(area, buf, t, &list, bg);
    let row = used + 1;
    put(buf, area, row, 0, "ButtonKind", t.muted().bg(bg));
    let mut x = 0;
    for (kind, label) in [
        (ButtonKind::Primary, "Primary"),
        (ButtonKind::Secondary, "Secondary"),
        (ButtonKind::Subtle, "Subtle"),
        (ButtonKind::Danger, "Danger"),
        (ButtonKind::Toggle, "Toggle"),
    ] {
        let text = format!(" {label} ");
        let w = junie_tui::ui::text::width(&text) as u16;
        if x + w > area.width {
            break;
        }
        let style = t.button(kind, VisualState::default(), bg);
        put(buf, area, row + 1, x, &text, style);
        x += w + s.button_gap;
    }
    put(buf, area, row + 2, 0, "BadgeKind::Edit", t.muted().bg(bg));
    put(buf, area, row + 2, 17, " EDIT ", t.badge(BadgeKind::Edit));
}

fn render_glyphs(area: Rect, buf: &mut Buffer, t: &Theme, bg: Color) {
    type Draw = fn(&Glyphs) -> String;
    let (u, a) = (Glyphs::UNICODE, Glyphs::ASCII);
    let col = |k: u16| 15 + k * 18;

    let cell = |buf: &mut Buffer, row: u16, k: u16, text: &str, style: Style| {
        put(
            buf,
            area,
            row,
            col(k),
            &junie_tui::ui::text::truncate(text, 17),
            style,
        );
    };
    put(buf, area, 0, 0, "Glyphs", t.muted().bg(bg));
    put(buf, area, 0, col(0), "UNICODE", t.muted().bg(bg));
    put(buf, area, 0, col(1), "ASCII", t.muted().bg(bg));
    let glyphs: [(&str, Draw); 7] = [
        ("checks", |g| {
            format!("{} {} {}", g.check_on, g.check_off, g.check_mixed)
        }),
        ("radio, toggle", |g| {
            format!(
                "{} {} {} {}",
                g.radio_on, g.radio_off, g.toggle_on, g.toggle_off
            )
        }),
        ("chevrons", |g| {
            format!(
                "{} {} {} {}",
                g.chevron_open, g.chevron_closed, g.disclosure_open, g.disclosure_closed
            )
        }),
        ("status", |g| {
            format!("{} {} {} {}", g.done, g.fail, g.warning, g.info)
        }),
        ("scrollbar", |g| {
            format!("{}{}{}", g.scroll_track, g.scroll_thumb, g.scroll_track)
        }),
        ("meter", |g| {
            format!("{}{}{}", g.bar_fill, g.bar_fill, g.bar_empty)
        }),
        ("more, close", |g| {
            format!("{} {} {}", g.ellipsis, g.more_below, g.close)
        }),
    ];
    for (i, (name, f)) in glyphs.iter().enumerate() {
        let row = 1 + i as u16;
        put(buf, area, row, 0, name, t.primary().bg(bg));
        cell(buf, row, 0, &f(&u), t.secondary().bg(bg));
        cell(buf, row, 1, &f(&a), t.secondary().bg(bg));
    }
    let (d, o) = (Strings::DEFAULT, Strings::ASCII);
    put(buf, area, 9, 0, "Strings", t.muted().bg(bg));
    put(buf, area, 9, col(0), "DEFAULT", t.muted().bg(bg));
    put(buf, area, 9, col(1), "override", t.muted().bg(bg));
    let strings: [(&str, &str, &str); 4] = [
        ("more_cut", d.more_cut, o.more_cut),
        ("search", d.search_placeholder, o.search_placeholder),
        ("empty_list", d.empty_list, o.empty_list),
        ("loading", d.loading_rows, o.loading_rows),
    ];
    for (i, (name, a, b)) in strings.iter().enumerate() {
        let row = 10 + i as u16;
        put(buf, area, row, 0, name, t.primary().bg(bg));
        cell(buf, row, 0, a, t.secondary().bg(bg));
        cell(buf, row, 1, b, t.secondary().bg(bg));
    }
}

impl Page for OverviewPage {
    fn title(&self) -> &'static str {
        "Overview"
    }
    fn blurb(&self) -> &'static str {
        "Tokens and principles behind every component"
    }

    fn measure(&self, avail: ratatui::layout::Size, ctx: &RenderCtx) -> ratatui::layout::Size {
        let facts = settings(ctx.theme);
        let settings_w = Panel::card(Some(Card::Settings.title()))
            .outer(ratatui::layout::Size::new(props::width(&facts), 0))
            .width;
        let w = avail.width.max(CARD_MIN).max(settings_w);
        let (_, _, _, heights) = Self::bands(w);
        let h = heights.iter().sum::<u16>() + GAP * (heights.len() as u16).saturating_sub(1);
        ratatui::layout::Size::new(w, h)
    }

    fn render(&mut self, area: Rect, buf: &mut Buffer, ctx: &mut RenderCtx) {
        let body = self.template.render(area, buf, ctx);
        match self.template.section() {
            Section::Playground => self.render_playground(body, buf, ctx),
            Section::Options => self.template.render_options(body, buf, ctx),
            Section::States => self.template.render_states(body, buf, ctx),
        }
    }

    fn handle(&mut self, ev: &PageEvent, cx: &mut Cx) -> Outcome {
        if let Some(o) = self.template.handle(ev, cx) {
            return o;
        }
        if self.template.section() == Section::Playground
            && let Some(o) = self.sections.handle(ev)
        {
            return o;
        }
        Outcome::Ignored
    }

    fn hints(&self, focus: Option<WidgetId>) -> Vec<Hint> {
        self.template
            .hints(focus)
            .unwrap_or_else(|| vec![("[ ]", "Pages"), ("i", "Inspector")])
    }

    fn option_value(&self, key: &str) -> Option<usize> {
        let values = [self.plane, self.kind, self.ladder, self.mods];
        OPTION_KEYS
            .iter()
            .position(|k| *k == key)
            .map(|i| values[i])
    }

    fn set_option(&mut self, key: &str, value: usize) {
        match key {
            "overview.plane" => self.set_plane(value),
            "overview.kind" => self.kind = value.min(KINDS.len() - 1),
            "overview.ladder" => self.ladder = value.min(1),
            "overview.mods" => self.mods = value.min(3),
            _ => {}
        }
    }

    fn template(&self) -> Option<&Template> {
        Some(&self.template)
    }

    fn template_mut(&mut self) -> Option<&mut Template> {
        Some(&mut self.template)
    }
}

const OPTION_KEYS: [&str; 4] = [
    "overview.plane",
    "overview.kind",
    "overview.ladder",
    "overview.mods",
];

const PRINCIPLES: &[(&str, &str)] = &[
    (
        "Colour carries meaning",
        "One accent hue for focus and primary action; status hues only where they carry meaning; syntax hues only in code.",
    ),
    (
        "Never colour alone",
        "Every status has its glyph: ✓ done, ✗ failed, ▲ attention, • pending, i info.",
    ),
    (
        "Alpha ladder",
        "Text and borders step down in white opacity, never in arbitrary grays.",
    ),
    (
        "State is geometry",
        "Hover lifts the surface, focus adds a bar, selection adds a marker, editing shows the cursor.",
    ),
    (
        "Three planes",
        "Canvas, surface, elevated. Depth comes from lightness, not borders.",
    ),
    (
        "Quiet chrome",
        "Bold is reserved for the focused control. No box around a thing unless the box carries meaning.",
    ),
];

fn render_roles(area: Rect, buf: &mut Buffer, t: &Theme) {
    if area.height < 4 {
        return;
    }
    let panel = Panel::card(Some("Roles"));
    let bg = panel.bg(t);
    let inner = panel.render(
        Rect::new(area.x, area.y, area.width, area.height.min(12)),
        buf,
        t,
    );
    let mut y = inner.y;
    let line =
        |buf: &mut Buffer, y: u16, label: &str, parts: &[(String, ratatui::style::Style)]| {
            if y >= inner.bottom() {
                return;
            }
            buf.set_string(inner.x, y, label, t.muted().bg(bg));
            let mut x = inner.x + 7;
            for (text, style) in parts {
                let w = junie_tui::ui::text::width(text) as u16;
                if x + w > inner.right() {
                    break;
                }
                buf.set_string(x, y, text, style.bg(bg));
                x += w;
            }
        };
    let gap = || (" ".to_owned(), t.primary());
    let status = |kinds: &[(StatusKind, &str)]| {
        let mut parts = Vec::new();
        for (kind, word) in kinds {
            let (glyph, style) = t.status(*kind);
            let label = t.status_label(*kind);
            let ink = t.legible(label.fg.unwrap_or(t.text_primary), bg);
            parts.push((glyph.to_owned(), style));
            parts.push((format!(" {word}"), label.fg(ink)));
            parts.push(gap());
        }
        parts
    };
    line(
        buf,
        y,
        "Status",
        &status(&[
            (StatusKind::Ok, "ok"),
            (StatusKind::Fail, "failed"),
            (StatusKind::Warn, "warn"),
            (StatusKind::Pending, "pending"),
        ]),
    );
    y += 1;
    line(
        buf,
        y,
        "",
        &status(&[
            (StatusKind::Running, "running"),
            (StatusKind::Queued, "queued"),
            (StatusKind::Info, "info"),
            (StatusKind::Skipped, "skipped"),
        ]),
    );
    y += 1;
    let env = |kinds: &[(EnvKind, &str)]| {
        let mut parts = Vec::new();
        for (kind, word) in kinds {
            let (_, style) = t.env(*kind);
            let glyph = t.glyphs.env(*kind);
            let text = if glyph.is_empty() {
                word.to_string()
            } else {
                format!("{glyph} {word}")
            };
            parts.push((text, style.fg(t.tone_on(kind.tone(), bg))));
            parts.push(gap());
        }
        parts
    };
    line(
        buf,
        y,
        "Env",
        &env(&[
            (EnvKind::Production, "production"),
            (EnvKind::Staging, "staging"),
        ]),
    );
    y += 1;
    line(
        buf,
        y,
        "",
        &env(&[
            (EnvKind::Development, "development"),
            (EnvKind::Local, "local"),
        ]),
    );
    y += 1;
    let tones = |list: &[(Tone, &str)]| {
        let mut parts = Vec::new();
        for (tone, word) in list {
            let fg = t.tone_on(*tone, bg);
            parts.push((word.to_string(), ratatui::style::Style::new().fg(fg)));
            parts.push(gap());
        }
        parts
    };
    line(
        buf,
        y,
        "Tone",
        &tones(&[
            (Tone::Normal, "normal"),
            (Tone::Secondary, "secondary"),
            (Tone::Muted, "muted"),
            (Tone::Faint, "faint"),
            (Tone::Active, "active"),
        ]),
    );
    y += 1;
    line(
        buf,
        y,
        "",
        &tones(&[
            (Tone::Info, "info"),
            (Tone::Success, "success"),
            (Tone::Warning, "warning"),
            (Tone::Error, "error"),
            (Tone::Danger, "danger"),
        ]),
    );
    y += 1;
    let code: Vec<(String, ratatui::style::Style)> = [
        ("let", SyntaxTone::Keyword),
        (" ", SyntaxTone::Plain),
        ("ok", SyntaxTone::Ident),
        (" = ", SyntaxTone::Operator),
        ("true", SyntaxTone::Literal),
        (", ", SyntaxTone::Punct),
        ("42", SyntaxTone::Number),
        (", ", SyntaxTone::Punct),
        ("\"text\"", SyntaxTone::Str),
        ("; ", SyntaxTone::Punct),
        ("// note", SyntaxTone::Comment),
    ]
    .into_iter()
    .map(|(text, tone)| (text.to_owned(), t.syntax(tone)))
    .collect();
    line(buf, y, "Code", &code);
    y += 1;
    line(
        buf,
        y,
        "Link",
        &[
            ("docs/colour.md".to_owned(), t.link_style(false)),
            ("  underlined on hover or focus".to_owned(), t.muted()),
        ],
    );
}
