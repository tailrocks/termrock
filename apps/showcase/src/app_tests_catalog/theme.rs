//! Lane `theme` of the #14 burn-down: this lane's callable scenarios.
//!
//! Each scenario builds real components, sends real input, reads typed
//! outcomes and renders to a `Buffer`. Register one `ScenarioCase` per
//! `ScenarioId` variant from the `lane:theme` block in `../coverage.rs`. Edit only
//! this file and `../coverage/lane_theme.rs`.

use std::time::Duration;

use ratatui::buffer::Buffer;
use ratatui::layout::{Position, Rect, Size};
use ratatui::style::{Color, Modifier, Style};
use termrock::core::edit::{EditEntry, FieldKeys};
use termrock::core::focus::FocusRing;
use termrock::core::hit::HitRegistry;
use termrock::core::id::WidgetId;
use termrock::core::scroll::ScrollState;
use termrock::theme::{
    self, BadgeKind, ButtonKind, Charset, ColorLevel, CursorRow, CursorStyle, EnvKind, Glyphs,
    HintAlign, HintBarStyle, InactiveSelection, Insets, Interactive, Kind, Ladder, Ladders,
    LevelOverrides, Plane, Spacing, StateModifiers, StatusKind, Strings, SyntaxTone, Theme, Tone,
    contrast_ratio,
};
use termrock::ui::content::{self, ContentState};
use termrock::ui::ctx::{Interaction, RenderCtx, VisualState};
use termrock::ui::fade;
use termrock::ui::measure::{self, Measure, Tile};
use termrock::ui::reveal::RevealState;
use termrock::ui::row::{MarkKind, RowLook, RowPaint, RowUi};
use termrock::ui::specimen::{KindSample, Specimen, SpecimenState, StateGrid};
use termrock::ui::text::{self, DurationStyle, WrapOpts};
use termrock::widgets::brand::Lockup;
use termrock::widgets::button::Button;
use termrock::widgets::choice::Checkbox;
use termrock::widgets::empty::{EmptyAction, EmptyKind, EmptyState};
use termrock::widgets::grid::RowState;
use termrock::widgets::input::TextInput;
use termrock::widgets::keyref::KeyReference;
use termrock::widgets::panel::CardHeading;
use termrock::widgets::segments::{self, Segment};

use super::{ScenarioCase, paint, paint_hits, rows, text_of};
use crate::coverage::ScenarioId;

#[rustfmt::skip]
pub const CASES: &[ScenarioCase] = &[
    ScenarioCase { id: ScenarioId::ThemeColorLevels, run: theme_color_levels },
    ScenarioCase { id: ScenarioId::ThemeCharsetGlyphs, run: theme_charset_glyphs },
    ScenarioCase { id: ScenarioId::ThemeInteractiveStates, run: theme_interactive_states },
    ScenarioCase { id: ScenarioId::ThemeInactiveSelection, run: theme_inactive_selection },
    ScenarioCase { id: ScenarioId::ThemeTonesAndStatus, run: theme_tones_and_status },
    ScenarioCase { id: ScenarioId::ThemeLegibility, run: theme_legibility },
    ScenarioCase { id: ScenarioId::ThemeStyleTokens, run: theme_style_tokens },
    ScenarioCase { id: ScenarioId::SpecimenStateGrid, run: specimen_state_grid },
    ScenarioCase { id: ScenarioId::TextUtilityFunctions, run: text_utility_functions },
    ScenarioCase { id: ScenarioId::RowPainting, run: row_painting },
    ScenarioCase { id: ScenarioId::ContentStates, run: content_states },
    ScenarioCase { id: ScenarioId::MeasureTiles, run: measure_tiles },
    ScenarioCase { id: ScenarioId::FadeScrollEdges, run: fade_scroll_edges },
    ScenarioCase { id: ScenarioId::StatusSegments, run: status_segments },
    ScenarioCase { id: ScenarioId::BrandLockup, run: brand_lockup },
    ScenarioCase { id: ScenarioId::KeyReferenceLayout, run: key_reference_layout },
];

/// Paints `draw` under `theme` and `interaction`, so a scenario can pick the
/// colour level and the pointer or focus state the widget sees.
fn paint_with(
    theme: &Theme,
    interaction: Interaction,
    area: Rect,
    draw: impl FnOnce(Rect, &mut Buffer, &mut RenderCtx),
) -> Buffer {
    let mut buf = Buffer::empty(area);
    let mut hits = HitRegistry::default();
    let mut ring = FocusRing::default();
    {
        let mut ctx = RenderCtx::new(theme, interaction, &mut hits, &mut ring);
        draw(area, &mut buf, &mut ctx);
        ctx.flush_overlays(&mut buf);
    }
    buf
}

fn state() -> VisualState {
    VisualState::default()
}

/// Colour levels, the per-level theme and its overrides, and the state
/// modifiers that carry hover and press where colour cannot.
fn theme_color_levels() {
    assert_eq!(
        ColorLevel::ALL,
        [
            ColorLevel::TrueColor,
            ColorLevel::Ansi256,
            ColorLevel::Ansi16,
            ColorLevel::Mono
        ]
    );
    let labels: Vec<&str> = ColorLevel::ALL.iter().map(|l| l.label()).collect();
    assert_eq!(labels, ["truecolor", "256 colors", "16 colors", "no color"]);
    assert!(
        ColorLevel::ALL.contains(&ColorLevel::detect()),
        "detect picks one of the four levels"
    );

    for level in ColorLevel::ALL {
        let t = Theme::for_level(level);
        assert_eq!(t.level, level);
        let expected = match level {
            ColorLevel::TrueColor | ColorLevel::Ansi256 => StateModifiers::COLOR,
            ColorLevel::Ansi16 => StateModifiers::ANSI16,
            ColorLevel::Mono => StateModifiers::MONO,
        };
        assert_eq!(t.mods, expected, "{level:?} carries its modifier set");
        let ratio = contrast_ratio(t.text_primary, t.canvas).expect("both are concrete colours");
        assert!(ratio >= 4.5, "{level:?} body text reads on the canvas");
    }
    assert_eq!(
        Theme::for_level(ColorLevel::TrueColor).canvas,
        Theme::junie().canvas
    );
    assert!(matches!(
        Theme::for_level(ColorLevel::Ansi256).canvas,
        Color::Indexed(_)
    ));
    assert_eq!(Theme::for_level(ColorLevel::Mono).canvas, Color::Black);
    // Mono keeps four greys: white text, light grey secondary text, dark grey
    // borders.
    let grey = Theme::for_level(ColorLevel::Mono);
    assert_eq!(grey.text_primary, Color::White);
    assert_eq!(grey.text_secondary, Color::Gray);
    assert_eq!(grey.border_subtle, Color::DarkGray);

    // Where colour cannot carry the state, a modifier does: a pressed button at
    // mono is reversed, at truecolor it differs by background only.
    let id = WidgetId::of("scenario.theme.button");
    let pressed_cell = |level: ColorLevel| {
        let theme = Theme::for_level(level);
        let interaction = Interaction {
            hover: Some(id),
            pressed: Some(id),
            ..Default::default()
        };
        let buf = paint_with(&theme, interaction, Rect::new(0, 0, 12, 1), |a, b, c| {
            Button::new(id, "Go", ButtonKind::Secondary).render(a, b, c, theme.surface);
        });
        assert!(text_of(&buf).contains("Go"));
        (buf[(3, 0)].modifier, theme)
    };
    let (mono, mono_theme) = pressed_cell(ColorLevel::Mono);
    assert!(mono.contains(Modifier::REVERSED), "mono press is reversed");
    assert!(mono.contains(mono_theme.mods.pressed));
    let (ansi16, _) = pressed_cell(ColorLevel::Ansi16);
    assert!(ansi16.contains(StateModifiers::ANSI16.pressed));
    let (truecolor, _) = pressed_cell(ColorLevel::TrueColor);
    assert!(!truecolor.contains(Modifier::REVERSED));
    assert_eq!(
        Theme::junie()
            .with_state_modifiers(StateModifiers::MONO)
            .mods,
        StateModifiers::MONO
    );
    assert_eq!(StateModifiers::COLOR.hover_flat, Modifier::UNDERLINED);
    assert_eq!(StateModifiers::MONO.disabled, Modifier::DIM);

    // Per-level overrides: ASCII at mono only, glyphs-only, or both.
    let mono_ascii = Theme::for_level_with(ColorLevel::Mono, &LevelOverrides::ASCII_MONO);
    assert_eq!(mono_ascii.glyphs, Glyphs::ASCII);
    assert_eq!(mono_ascii.strings, Strings::ASCII);
    let colour_keeps = Theme::for_level_with(ColorLevel::TrueColor, &LevelOverrides::ASCII_MONO);
    assert_eq!(colour_keeps.glyphs, Glyphs::UNICODE);
    let glyphs_only = LevelOverrides {
        ansi16: Some(theme::ascii_glyphs),
        ..LevelOverrides::default()
    };
    let t16 = Theme::for_level_with(ColorLevel::Ansi16, &glyphs_only);
    assert_eq!(t16.glyphs, Glyphs::ASCII);
    assert_eq!(t16.strings, Strings::DEFAULT);
    let both = LevelOverrides {
        truecolor: Some(theme::ascii),
        ..LevelOverrides::default()
    };
    let t = Theme::for_level_with(ColorLevel::TrueColor, &both);
    assert_eq!((t.glyphs, t.strings), (Glyphs::ASCII, Strings::ASCII));

    // contrast_ratio is the WCAG ratio and is symmetric; Reset has no colour.
    let max = contrast_ratio(Color::Black, Color::White).expect("concrete colours");
    assert!(
        (max - 21.0).abs() < 0.01,
        "black on white is 21:1, got {max}"
    );
    assert_eq!(
        contrast_ratio(Color::White, Color::Black),
        contrast_ratio(Color::Black, Color::White)
    );
    assert_eq!(contrast_ratio(Color::Reset, Color::Black), None);
}

fn env_of(pairs: &'static [(&'static str, &'static str)]) -> impl Fn(&str) -> Option<String> {
    move |key| {
        pairs
            .iter()
            .find(|(k, _)| *k == key)
            .map(|(_, v)| (*v).to_owned())
    }
}

/// The glyph and string sets, the charset that picks them and the themes that
/// carry them down to a rendered widget.
fn theme_charset_glyphs() {
    assert_eq!(Charset::default(), Charset::Unicode);
    assert_eq!(Charset::OVERRIDE_ENV, "JUNIE_TUI_GLYPHS");
    assert_eq!(
        Charset::ASCII_TERMS,
        ["linux", "vt", "cons25", "dumb", "pcansi"]
    );
    assert_eq!(Charset::Unicode.glyphs(), Glyphs::UNICODE);
    assert_eq!(Charset::Ascii.glyphs(), Glyphs::ASCII);
    assert_eq!(Charset::Unicode.strings(), Strings::DEFAULT);
    assert_eq!(Charset::Ascii.strings(), Strings::ASCII);
    let _ = Charset::detect();

    // from_env: the override wins over TERM, ASCII terminals fall back, and a
    // bare Windows console without a modern host is ASCII.
    let from = |pairs: &'static [(&str, &str)], windows| Charset::from_env(env_of(pairs), windows);
    assert_eq!(
        from(&[("JUNIE_TUI_GLYPHS", "ascii")], false),
        Charset::Ascii
    );
    assert_eq!(
        from(&[("JUNIE_TUI_GLYPHS", "UTF-8"), ("TERM", "linux")], false),
        Charset::Unicode
    );
    for term in ["linux", "vt100", "dumb", "cons25", "pcansi", "vt220-color"] {
        let pairs: &'static [(&str, &str)] = match term {
            "linux" => &[("TERM", "linux")],
            "vt100" => &[("TERM", "vt100")],
            "dumb" => &[("TERM", "dumb")],
            "cons25" => &[("TERM", "cons25")],
            "pcansi" => &[("TERM", "pcansi")],
            _ => &[("TERM", "vt220-color")],
        };
        assert_eq!(from(pairs, false), Charset::Ascii, "TERM={term}");
    }
    assert_eq!(from(&[("TERM", "xterm-256color")], false), Charset::Unicode);
    assert_eq!(
        from(&[("TERM", "vt-custom")], false),
        Charset::Unicode,
        "a bare `vt` prefix is not a VT terminal"
    );
    assert_eq!(from(&[], true), Charset::Ascii);
    assert_eq!(from(&[("WT_SESSION", "1")], true), Charset::Unicode);
    assert_eq!(from(&[], false), Charset::Unicode);

    // apply swaps glyphs and strings together; with_glyphs and with_strings
    // swap one each.
    let ascii = Charset::Ascii.apply(Theme::junie());
    assert_eq!(
        (ascii.glyphs, ascii.strings),
        (Glyphs::ASCII, Strings::ASCII)
    );
    let unicode = Charset::Unicode.apply(ascii);
    assert_eq!(unicode.glyphs, Glyphs::UNICODE);
    let only_glyphs = Theme::junie().with_glyphs(Glyphs::ASCII);
    assert_eq!(only_glyphs.strings, Strings::DEFAULT);
    let only_strings = Theme::junie().with_strings(Strings::ASCII);
    assert_eq!(only_strings.glyphs, Glyphs::UNICODE);

    // A checked box renders the theme's glyph, not a hardcoded one.
    let render_box = |theme: &Theme| {
        paint_with(
            theme,
            Interaction::default(),
            Rect::new(0, 0, 16, 1),
            |a, b, c| {
                Checkbox::new(WidgetId::of("scenario.theme.check"), "Opt", true).render(
                    a,
                    b,
                    c,
                    theme.canvas,
                );
            },
        )
    };
    assert!(text_of(&render_box(&Theme::junie())).contains(Glyphs::UNICODE.check_on));
    assert!(text_of(&render_box(&ascii)).contains(Glyphs::ASCII.check_on));
    assert_eq!(Glyphs::ASCII.check_on, "[x]");

    // Glyphs: env markers, key labels, spinner frames.
    assert_eq!(Glyphs::UNICODE.env(EnvKind::Production), "◆");
    assert_eq!(Glyphs::UNICODE.env(EnvKind::Staging), "◇");
    assert_eq!(Glyphs::UNICODE.env(EnvKind::Development), "·");
    assert_eq!(Glyphs::UNICODE.env(EnvKind::Local), "");
    assert_eq!(Glyphs::ASCII.env(EnvKind::Production), "#");
    assert_eq!(Glyphs::default(), Glyphs::UNICODE);
    assert!(matches!(
        Glyphs::UNICODE.key_label("↑↓ ⇧Tab"),
        std::borrow::Cow::Borrowed("↑↓ ⇧Tab")
    ));
    assert_eq!(Glyphs::ASCII.key_label("↑↓ ←→ ⇧Tab 1–3"), "^v <> STab 1-3");
    assert_eq!(Glyphs::UNICODE.spinner_frame(0), "⠋");
    assert_eq!(
        Glyphs::UNICODE.spinner_frame(10),
        "⠋",
        "the frame wraps at the end of the cycle"
    );
    assert_eq!(Glyphs::ASCII.spinner_frame(5), "/");
    let still = Glyphs {
        spinner: &[],
        ..Glyphs::UNICODE
    };
    assert_eq!(still.spinner_frame(3), " ", "no frames draws a blank");

    // Strings: fill, follow and the two sets.
    assert_eq!(Strings::default(), Strings::DEFAULT);
    assert_eq!(
        Strings::fill(Strings::DEFAULT.type_to_confirm, &[&"prod"]),
        "Type prod to confirm"
    );
    assert_eq!(Strings::fill("{} of {}", &[&1]), "1 of {}");
    assert_eq!(Strings::fill("{} of {}", &[&1, &2, &3]), "1 of 2");
    assert_eq!(Strings::DEFAULT.search_placeholder, "Type to search…");
    assert_eq!(Strings::ASCII.search_placeholder, "Type to search...");
    assert_eq!(
        Strings::ASCII.cancel,
        Strings::DEFAULT.cancel,
        "an ASCII set only replaces the strings that carry non-ASCII"
    );
    let mut text = String::from("Cancel");
    Strings::follow(&mut text, "Cancel", "Abort");
    assert_eq!(text, "Abort", "an untouched string follows the new default");
    Strings::follow(&mut text, "Cancel", "Stop");
    assert_eq!(text, "Abort", "a customised string is left alone");
    Strings::follow(&mut text, "Abort", "Abort");
    assert_eq!(text, "Abort");
}

const ALL_KINDS: [Kind; 13] = [
    Kind::Row,
    Kind::CursorRow(CursorStyle::Fill),
    Kind::CursorRow(CursorStyle::Bar),
    Kind::Button(ButtonKind::Primary),
    Kind::Button(ButtonKind::Secondary),
    Kind::Button(ButtonKind::Subtle),
    Kind::Button(ButtonKind::Danger),
    Kind::Button(ButtonKind::Toggle),
    Kind::Tab,
    Kind::Title,
    Kind::Chip,
    Kind::Close,
    Kind::Field,
];

/// Planes, ladders, kinds and the interactive style they resolve to.
fn theme_interactive_states() {
    let t = Theme::junie();

    // Planes: ten of them, four fills, and `up` lifts a plane one elevation.
    assert_eq!(Plane::ALL.len(), 10);
    let fills: Vec<Plane> = Plane::ALL.into_iter().filter(|p| p.is_fill()).collect();
    assert_eq!(
        fills,
        [
            Plane::Highlight,
            Plane::HighlightDanger,
            Plane::Accent,
            Plane::Danger
        ]
    );
    for plane in [Plane::Canvas, Plane::Surface, Plane::Card, Plane::Field] {
        assert_eq!(plane.up(), Plane::Raised);
    }
    assert_eq!(Plane::Raised.up(), Plane::Popover);
    assert_eq!(Plane::Popover.up(), Plane::Popover);
    assert_eq!(Plane::Accent.up(), Plane::Accent);
    for plane in Plane::ALL {
        assert_eq!(
            t.plane_of(t.ladder(plane).rest),
            plane,
            "{plane:?} is recognised from its resting colour"
        );
    }
    assert_eq!(t.plane_of(Color::Reset), Plane::Canvas);

    // Ladder::new and step: pressed beats hover beats rest.
    let ladder = Ladder::new(Color::Red, Color::Green, Color::Blue);
    assert_eq!(ladder.step(false, false), Color::Red);
    assert_eq!(ladder.step(true, false), Color::Green);
    assert_eq!(ladder.step(false, true), Color::Blue);
    assert_eq!(ladder.step(true, true), Color::Blue);
    let ladders: Ladders = t.ladders;
    assert_eq!(ladders.card, t.ladder(Plane::Card));
    assert_eq!(
        ladders.seam.pressed, t.accent,
        "a dragged seam is the accent"
    );
    let custom = t.with_ladder(Plane::Card, ladder);
    assert_eq!(custom.ladder(Plane::Card), ladder);
    assert_eq!(custom.ladder(Plane::Raised), t.ladder(Plane::Raised));
    let card = t.ladder(Plane::Card);
    assert_eq!(t.lift(card.rest), card.hover);
    assert_eq!(t.lift(card.hover), card.pressed);

    // Kind::is_glyph: only the resize handle and seam are glyph-only.
    assert!(Kind::Handle.is_glyph() && Kind::Seam.is_glyph());
    assert!(ALL_KINDS.iter().all(|k| !k.is_glyph()));

    // Every kind: rest, hover and pressed differ; focus shows the gutter bar
    // except where the fill itself is the cursor; a disabled kind ignores
    // hover and press.
    let hover = VisualState {
        hovered: true,
        ..state()
    };
    let press = VisualState {
        hovered: true,
        pressed: true,
        ..state()
    };
    let focus = VisualState {
        focused: true,
        ..state()
    };
    let dead = VisualState {
        disabled: true,
        ..state()
    };
    let dead_hover = VisualState {
        disabled: true,
        hovered: true,
        pressed: true,
        ..state()
    };
    for kind in ALL_KINDS.into_iter().chain([Kind::Handle, Kind::Seam]) {
        let rest = t.interactive(kind, state(), Plane::Surface);
        let hovered = t.interactive(kind, hover, Plane::Surface);
        let pressed = t.interactive(kind, press, Plane::Surface);
        assert_ne!(
            rest.style, hovered.style,
            "{kind:?} hover differs from rest"
        );
        assert_ne!(
            hovered.style, pressed.style,
            "{kind:?} press differs from hover"
        );
        assert_eq!(
            t.interactive(kind, dead, Plane::Surface).style,
            t.interactive(kind, dead_hover, Plane::Surface).style,
            "{kind:?} disabled does not react to hover or press"
        );
        if kind.is_glyph() {
            continue;
        }
        let focused: Interactive = t.interactive(kind, focus, Plane::Surface);
        let bar = !matches!(
            kind,
            Kind::CursorRow(CursorStyle::Fill) | Kind::Close | Kind::Field
        );
        assert_eq!(
            focused.gutter,
            if bar { t.glyphs.focus_bar } else { " " },
            "{kind:?} focus gutter"
        );
        assert_eq!(rest.gutter, " ");
        for s in [state(), hover, press, focus] {
            let look = t.interactive(kind, s, Plane::Surface);
            let (Some(fg), Some(bg)) = (look.style.fg, look.style.bg) else {
                panic!("{kind:?} sets both colours");
            };
            assert!(
                contrast_ratio(fg, bg).is_some_and(|r| r >= 4.5),
                "{kind:?} {s:?} text reads on its plane"
            );
        }
    }

    // Buttons: the accent fill, the danger press, the neutral raised body.
    let button = |kind, s| t.button(kind, s, t.surface);
    assert_eq!(
        button(ButtonKind::Primary, state()).bg,
        Some(t.ladder(Plane::Accent).rest)
    );
    assert_eq!(
        button(ButtonKind::Primary, state()).fg,
        Some(t.text_on_accent)
    );
    assert_eq!(
        button(ButtonKind::Danger, press).bg,
        Some(t.ladders.danger.pressed)
    );
    for kind in [
        ButtonKind::Secondary,
        ButtonKind::Subtle,
        ButtonKind::Toggle,
    ] {
        assert_ne!(button(kind, state()).bg, Some(t.ladder(Plane::Accent).rest));
    }
    assert_eq!(
        t.button(ButtonKind::Subtle, state(), t.surface).bg,
        Some(t.surface),
        "a subtle button keeps the plane it sits on"
    );

    // Rows and cursors: Fill paints the highlight, Bar keeps the plane and
    // draws the focus bar; an error cursor turns red.
    let focused_row = VisualState {
        focused: true,
        ..state()
    };
    let fill: CursorRow = t.cursor_row(focused_row, t.surface, CursorStyle::Fill);
    assert_eq!(fill.style.bg, Some(t.highlight));
    assert_eq!(fill.gutter, " ");
    assert!(fill.on_fill);
    let bar = t.cursor_row(focused_row, t.surface, CursorStyle::Bar);
    assert_eq!(bar.style.bg, Some(t.surface));
    assert_eq!(bar.gutter, t.glyphs.focus_bar);
    assert!(!bar.on_fill);
    let failing = VisualState {
        error: true,
        ..focused_row
    };
    assert_eq!(
        t.cursor_row(failing, t.surface, CursorStyle::Fill).style.bg,
        Some(t.highlight_danger)
    );
    assert_eq!(t.row_ink(&fill, Color::Red), t.text_primary);
    assert_eq!(t.row_ink(&bar, Color::Red), Color::Red);
    assert_eq!(
        t.row(hover, t.surface),
        t.interactive(Kind::Row, hover, Plane::Surface).style
    );
    assert_eq!(t.cell_cursor(false, false).bg, Some(t.highlight));
    assert_eq!(t.cell_cursor(true, false).bg, Some(t.highlight_danger));
    assert_eq!(
        t.cell_cursor(false, true).bg,
        Some(t.ladder(Plane::Highlight).pressed)
    );
    assert_eq!(
        t.cursor_in_selection(false, false).bg,
        Some(t.highlight_on_selection)
    );
    assert_eq!(
        t.cursor_in_selection(true, false),
        t.cell_cursor(true, false)
    );
    let mono = Theme::for_level(ColorLevel::Mono);
    let mono_cursor = mono.cursor_in_selection(false, false);
    assert!(mono_cursor.add_modifier.contains(Modifier::BOLD));
    assert!(!mono_cursor.add_modifier.contains(Modifier::REVERSED));

    // The gutter: only a focused enabled (or fixed) element draws the bar.
    assert_eq!(t.gutter_symbol(focus), t.glyphs.focus_bar);
    assert_eq!(t.gutter_symbol(state()), " ");
    let focused_dead = VisualState {
        disabled: true,
        focused: true,
        ..state()
    };
    assert_eq!(t.gutter_symbol(focused_dead), " ");
    assert_eq!(
        t.gutter_symbol(VisualState {
            fixed: true,
            ..focused_dead
        }),
        t.glyphs.focus_bar
    );
    assert_eq!(t.gutter(state(), t.surface, false).fg, Some(t.surface));
    assert_eq!(t.gutter(focus, t.surface, true).fg, Some(t.text_on_accent));
    assert_eq!(
        t.gutter(focus, t.surface, false).fg,
        Some(t.legible_mark(t.focus, t.surface))
    );

    // Fields: hover is suppressed while editing; a placeholder is muted and a
    // disabled one is the disabled ink; the close glyph and its danger plate.
    let editing = VisualState {
        editing: true,
        ..state()
    };
    assert_eq!(
        t.field_style(VisualState {
            hovered: true,
            ..editing
        }),
        t.field_style(editing)
    );
    assert_eq!(t.field_style(state()).bg, Some(t.field));
    assert_eq!(t.placeholder(state()).fg, Some(t.text_muted));
    assert_eq!(t.placeholder(dead).fg, Some(t.disabled));
    let host = t.on(t.surface);
    let close_rest = t.close_glyph(state(), host);
    assert!(!close_rest.add_modifier.contains(Modifier::BOLD));
    assert_ne!(t.close_glyph(hover, host), close_rest);
    assert_eq!(t.close_plate(state(), host, false), close_rest);
    assert_eq!(t.danger_plate(dead, host, true), host);
    assert_eq!(t.danger_plate(state(), host, false), host);
    let plate_hover = t.danger_plate(hover, host, true);
    assert_eq!(
        plate_hover.fg,
        Some(t.legible(t.danger_label, t.surface)),
        "a hovered danger plate turns the ink red"
    );
    assert_eq!(
        t.danger_plate(press, host, true).bg,
        Some(t.ladders.danger.pressed)
    );
    assert_eq!(
        t.close_plate(press, host, true).bg,
        Some(t.ladders.danger.pressed)
    );

    // Scrollbar thumb and track: one glyph, three greys; mono uses modifiers.
    assert_eq!(
        t.scrollbar_thumb(false, false).fg,
        Some(t.ladders.handle.rest)
    );
    assert_eq!(
        t.scrollbar_thumb(true, false).fg,
        Some(t.ladders.handle.hover)
    );
    assert_eq!(t.scrollbar_thumb(true, true).fg, Some(t.accent));
    assert_eq!(t.scrollbar_track().fg, Some(t.scroll_track));
    assert!(
        mono.scrollbar_thumb(true, false)
            .add_modifier
            .contains(mono.mods.thumb_hover)
    );
    assert!(
        mono.scrollbar_thumb(false, true)
            .add_modifier
            .contains(Modifier::REVERSED)
    );

    // Selection, current match, link and the disabled ink.
    assert_eq!(t.selection().bg, Some(t.selection_bg));
    for level in [ColorLevel::Ansi16, ColorLevel::Mono] {
        assert!(
            Theme::for_level(level)
                .selection()
                .add_modifier
                .contains(Modifier::REVERSED)
        );
    }
    let current = t.match_current();
    assert_eq!(current.bg, Some(t.accent));
    assert!(current.add_modifier.contains(Modifier::BOLD));
    assert!(
        mono.match_current()
            .add_modifier
            .contains(Modifier::REVERSED)
    );
    assert!(
        !t.link_style(false)
            .add_modifier
            .contains(Modifier::UNDERLINED)
    );
    assert!(
        t.link_style(true)
            .add_modifier
            .contains(Modifier::UNDERLINED)
    );
    assert!(
        mono.link_style(false)
            .add_modifier
            .contains(Modifier::UNDERLINED),
        "at mono a link is always underlined"
    );
    assert_eq!(t.disabled_style().fg, Some(t.disabled));
    assert!(mono.disabled_style().add_modifier.contains(Modifier::DIM));

    // At every colour level rest, hover and pressed stay apart, a disabled
    // kind ignores them, and no text takes its own background as ink.
    for level in ColorLevel::ALL {
        let lt = Theme::for_level(level);
        for kind in ALL_KINDS {
            for plane in [Plane::Canvas, Plane::Surface, Plane::Card] {
                let rest = lt.interactive(kind, state(), plane);
                let hovered = lt.interactive(kind, hover, plane);
                let pressed = lt.interactive(kind, press, plane);
                assert_ne!(rest.style, hovered.style, "{level:?} {kind:?} hover");
                assert_ne!(hovered.style, pressed.style, "{level:?} {kind:?} press");
                assert_ne!(
                    rest.style, pressed.style,
                    "{level:?} {kind:?} press vs rest"
                );
                assert_eq!(
                    lt.interactive(kind, dead, plane).style,
                    lt.interactive(kind, dead_hover, plane).style,
                    "{level:?} {kind:?} disabled"
                );
                for s in [state(), hover, press, focus, dead] {
                    let look = lt.interactive(kind, s, plane);
                    assert_ne!(
                        look.style.fg, look.style.bg,
                        "{level:?} {kind:?} {s:?} text is not its own background"
                    );
                    if matches!(level, ColorLevel::TrueColor | ColorLevel::Ansi256) {
                        let (Some(fg), Some(bg)) = (look.style.fg, look.style.bg) else {
                            continue;
                        };
                        assert!(
                            contrast_ratio(fg, bg).is_some_and(|r| r >= 4.5) || s.disabled,
                            "{level:?} {kind:?} {s:?} text reads on {plane:?}"
                        );
                    }
                }
            }
        }
    }

    // A real rest/hover/pressed button on a canvas paints three different
    // cells.
    let id = WidgetId::of("scenario.theme.states");
    let cells = [
        Interaction::default(),
        Interaction {
            hover: Some(id),
            ..Default::default()
        },
        Interaction {
            hover: Some(id),
            pressed: Some(id),
            ..Default::default()
        },
    ]
    .map(|interaction| {
        let buf = paint_with(&t, interaction, Rect::new(0, 0, 10, 1), |a, b, c| {
            Button::new(id, "Run", ButtonKind::Primary).render(a, b, c, t.canvas);
        });
        buf[(3, 0)].bg
    });
    assert!(cells[0] != cells[1] && cells[1] != cells[2] && cells[0] != cells[2]);
}

/// How a selected row looks once focus moves elsewhere, per policy.
fn theme_inactive_selection() {
    assert_eq!(
        InactiveSelection::ALL,
        [
            InactiveSelection::Accent,
            InactiveSelection::Neutral,
            InactiveSelection::None
        ]
    );
    assert_eq!(InactiveSelection::default(), InactiveSelection::Accent);
    let selected = VisualState {
        selected: true,
        ..state()
    };
    let t = Theme::junie();
    let bg_of = |t: &Theme, s: VisualState| t.row(s, t.surface).bg;

    // Owner decision 2026-10-17: the accent stays while focus is elsewhere.
    assert_eq!(t.inactive_selection, InactiveSelection::Accent);
    assert_eq!(bg_of(&t, selected), Some(t.accent_bg));
    let neutral = t.with_inactive_selection(InactiveSelection::Neutral);
    assert_eq!(bg_of(&neutral, selected), Some(t.selection_inactive));
    let none = t.with_inactive_selection(InactiveSelection::None);
    assert_eq!(bg_of(&none, selected), Some(t.ladder(Plane::Surface).rest));
    let focused = VisualState {
        focused: true,
        ..selected
    };
    assert_eq!(
        bg_of(&neutral, focused),
        Some(t.accent_bg),
        "the focused selection is always the accent"
    );

    // Under a modal or popup the accent selection goes neutral.
    let under_modal = VisualState {
        backdrop: true,
        ..selected
    };
    assert_eq!(t.inactive_look(selected), InactiveSelection::Accent);
    assert_eq!(t.inactive_look(under_modal), InactiveSelection::Neutral);
    assert_eq!(bg_of(&t, under_modal), Some(t.selection_inactive));
    // A per-state override beats the theme.
    let local = VisualState {
        inactive_selection: Some(InactiveSelection::None),
        ..selected
    };
    assert_eq!(t.inactive_look(local), InactiveSelection::None);

    // Chosen values follow the same policy.
    assert_eq!(t.chosen(true), t.accent);
    assert_eq!(t.chosen(false), t.text_primary);
    assert_eq!(t.chosen_in(selected), t.accent);
    assert_eq!(neutral.chosen_in(selected), t.text_primary);
    assert_eq!(neutral.chosen_in(focused), t.accent);
    assert_eq!(t.chosen_in(under_modal), t.text_primary);
}

/// Tones, status kinds, environments, syntax tones and badges, down to the
/// cells they paint.
fn theme_tones_and_status() {
    let t = Theme::junie();
    let want = [
        (Tone::Normal, t.text_primary),
        (Tone::Secondary, t.text_secondary),
        (Tone::Muted, t.text_muted),
        (Tone::Faint, t.text_faint),
        (Tone::Error, t.error),
        (Tone::Warning, t.warning),
        (Tone::Success, t.success),
        (Tone::Info, t.info),
        (Tone::Active, t.accent),
        (Tone::Danger, t.danger_label),
    ];
    assert_eq!(Tone::default(), Tone::Normal);
    for (tone, colour) in want {
        assert_eq!(t.tone(tone), colour, "{tone:?}");
        for level in ColorLevel::ALL {
            let theme = Theme::for_level(level);
            let ink = theme.tone_on(tone, theme.canvas);
            assert_eq!(ink, theme.legible(theme.tone(tone), theme.canvas));
        }
    }
    assert_eq!(t.tone_on(Tone::Normal, t.canvas), t.text_primary);
    // The palette itself keeps every text tone readable on the two planes
    // text mostly sits on, before any rescue by `legible`.
    for tone in [
        Tone::Normal,
        Tone::Secondary,
        Tone::Muted,
        Tone::Error,
        Tone::Warning,
        Tone::Success,
        Tone::Info,
        Tone::Active,
    ] {
        for bg in [t.canvas, t.surface] {
            assert!(
                contrast_ratio(t.tone(tone), bg).is_some_and(|r| r >= 4.5),
                "{tone:?} reads on {bg:?}"
            );
        }
    }

    // Status kinds: tone, glyph and style, with the spinner on a tick.
    assert_eq!(StatusKind::ALL.len(), 8);
    let tones = [
        (StatusKind::Ok, Tone::Success),
        (StatusKind::Fail, Tone::Error),
        (StatusKind::Warn, Tone::Warning),
        (StatusKind::Pending, Tone::Warning),
        (StatusKind::Running, Tone::Active),
        (StatusKind::Queued, Tone::Muted),
        (StatusKind::Info, Tone::Info),
        (StatusKind::Skipped, Tone::Faint),
    ];
    assert_eq!(tones.map(|(k, _)| k), StatusKind::ALL);
    let glyphs = ["✓", "✗", "▲", "•", "⠋", "○", "i", "–"];
    for ((kind, tone), glyph) in tones.into_iter().zip(glyphs) {
        assert_eq!(kind.tone(), tone, "{kind:?}");
        let (shown, style) = t.status(kind);
        assert_eq!(shown, glyph, "{kind:?} glyph");
        assert_eq!(style.fg, Some(t.tone(tone)));
        assert_eq!(
            style.add_modifier.contains(Modifier::BOLD),
            kind == StatusKind::Fail,
            "only a failure is bold"
        );
        let label = t.status_label(kind);
        let label_tone = if kind == StatusKind::Running {
            t.info
        } else {
            t.tone(tone)
        };
        assert_eq!(label.fg, Some(label_tone), "{kind:?} label ink");
    }
    assert_eq!(t.status_at(StatusKind::Running, 1).0, "⠙");
    assert_eq!(
        t.status_at(StatusKind::Running, 10).0,
        t.status_at(StatusKind::Running, 0).0
    );
    assert_eq!(
        Theme::junie()
            .with_glyphs(Glyphs::ASCII)
            .status(StatusKind::Ok)
            .0,
        "+"
    );

    // Environments: the production marker is bold and the loudest tone.
    let envs = [
        (EnvKind::Production, Tone::Danger, "◆"),
        (EnvKind::Staging, Tone::Warning, "◇"),
        (EnvKind::Development, Tone::Info, "·"),
        (EnvKind::Local, Tone::Muted, ""),
    ];
    for (env, tone, glyph) in envs {
        assert_eq!(env.tone(), tone);
        let (shown, style) = t.env(env);
        assert_eq!(shown, glyph);
        assert_eq!(style.fg, Some(t.tone(tone)));
        assert_eq!(
            style.add_modifier.contains(Modifier::BOLD),
            env == EnvKind::Production
        );
    }

    // Syntax tones: nine tones, six styles.
    assert_eq!(t.syntax(SyntaxTone::Keyword).fg, Some(t.syntax_keyword));
    assert!(
        t.syntax(SyntaxTone::Keyword)
            .add_modifier
            .contains(Modifier::BOLD)
    );
    assert_eq!(t.syntax(SyntaxTone::Ident), t.syntax(SyntaxTone::Plain));
    assert_eq!(t.syntax(SyntaxTone::Plain).fg, Some(t.text_primary));
    assert_eq!(t.syntax(SyntaxTone::Str).fg, Some(t.syntax_string));
    assert_eq!(t.syntax(SyntaxTone::Number), t.syntax(SyntaxTone::Literal));
    assert_eq!(t.syntax(SyntaxTone::Literal).fg, Some(t.syntax_literal));
    assert!(
        t.syntax(SyntaxTone::Literal)
            .add_modifier
            .contains(Modifier::ITALIC)
    );
    assert_eq!(t.syntax(SyntaxTone::Operator), t.syntax(SyntaxTone::Punct));
    assert_eq!(t.syntax(SyntaxTone::Punct).fg, Some(t.text_muted));
    let comment = t.syntax(SyntaxTone::Comment);
    assert_eq!(comment.fg, Some(t.text_faint));
    assert!(comment.add_modifier.contains(Modifier::ITALIC));

    // The edit badge is the accent plate with dark bold ink.
    let badge = t.badge(BadgeKind::Edit);
    assert_eq!(badge.bg, Some(t.accent));
    assert_eq!(badge.fg, Some(t.text_on_accent));
    assert!(badge.add_modifier.contains(Modifier::BOLD));

    // Row change marks in a grid: one glyph and ink per row state.
    let changes = [
        (RowState::Clean, " ", t.text_faint),
        (RowState::Modified, t.glyphs.dirty, t.warning),
        (RowState::Inserted, "+", t.text_secondary),
        (RowState::Deleted, t.glyphs.removed, t.text_muted),
        (RowState::Error, "!", t.error),
    ];
    for (row, glyph, ink) in changes {
        assert_eq!(t.change_glyph(row), (glyph, ink), "{row:?}");
    }

    // Painted into a buffer, the cells carry exactly those styles.
    let mut buf = Buffer::empty(Rect::new(0, 0, 12, 1));
    let (glyph, status) = t.status(StatusKind::Fail);
    buf.set_string(0, 0, glyph, status);
    buf.set_string(2, 0, "EDIT", badge);
    buf.set_string(7, 0, "SELECT", t.syntax(SyntaxTone::Keyword));
    assert_eq!(buf[(0, 0)].symbol(), "✗");
    assert_eq!(buf[(0, 0)].fg, t.error);
    assert_eq!(buf[(2, 0)].bg, t.accent);
    assert_eq!(buf[(7, 0)].fg, t.syntax_keyword);
}

/// Helpers that keep ink readable on any plane.
fn theme_legibility() {
    let t = Theme::junie();

    // Ink that already reads is returned untouched.
    assert_eq!(t.legible(t.text_primary, t.canvas), t.text_primary);
    assert_eq!(t.legible(Color::Reset, t.canvas), Color::Reset);
    // A ghost ink on the canvas does not read; legible picks one that does.
    let fixed = t.legible(t.text_ghost, t.canvas);
    assert_ne!(fixed, t.text_ghost);
    assert!(contrast_ratio(fixed, t.canvas).is_some_and(|r| r >= 4.5));
    // Error ink that falls short swaps to the strong red where that reads.
    let reads = |ink: Color, bg: Color| contrast_ratio(ink, bg).is_some_and(|r| r >= 4.5);
    let planes: Vec<Color> = Plane::ALL
        .into_iter()
        .flat_map(|p| {
            let l = t.ladder(p);
            [l.rest, l.hover, l.pressed]
        })
        .collect();
    let swapped: Vec<Color> = planes
        .iter()
        .copied()
        .filter(|&bg| !reads(t.error, bg) && reads(t.error_strong, bg))
        .collect();
    assert!(!swapped.is_empty(), "some plane needs the strong red");
    for bg in swapped {
        assert_eq!(t.legible(t.error, bg), t.error_strong);
    }
    // Where even the strong red fails, the best of the primary inks wins.
    let rescued = t.legible(t.error, t.highlight_danger);
    assert!(
        contrast_ratio(rescued, t.highlight_danger) > contrast_ratio(t.error, t.highlight_danger)
    );

    // Marks need only 3:1: the accent bar on black stays, on the accent
    // plate itself it flips to a ink that reads.
    assert_eq!(t.legible_mark(t.focus, t.canvas), t.focus);
    let on_accent = t.legible_mark(t.focus, t.accent);
    assert!(contrast_ratio(on_accent, t.accent).is_some_and(|r| r >= 3.0));
    assert_ne!(on_accent, t.focus);

    // beside_label: keeps a readable ink, steps up the grey ladder, or falls
    // back to the label colour.
    assert_eq!(
        t.beside_label(t.text_muted, t.canvas, t.text_primary),
        t.text_muted
    );
    let stepped = t.beside_label(t.text_faint, t.field, t.text_primary);
    assert!(contrast_ratio(stepped, t.field).is_some_and(|r| r >= 4.5));
    assert_eq!(
        t.beside_label(t.accent, t.accent, t.text_primary),
        t.text_primary,
        "an ink that is no grey rung falls back to the label"
    );

    // frame_on and rule_on: a frame is visible on every plane at every level.
    for level in ColorLevel::ALL {
        let theme = Theme::for_level(level);
        for bg in [
            theme.canvas,
            theme.surface,
            theme.surface_overlay,
            theme.popover,
        ] {
            let frame = theme.frame_on(bg);
            assert_ne!(frame, bg, "{level:?} frame is visible");
            assert_ne!(theme.rule_on(bg), bg, "{level:?} rule is visible");
        }
    }
    assert_eq!(t.rule_on(t.surface_overlay), t.border_strong);
    assert_eq!(t.rule_on(t.surface), t.border_subtle);

    // backdrop: dims what sits under a modal without erasing it.
    let ink = t.on(t.canvas);
    let dimmed = t.backdrop(ink);
    assert_eq!(dimmed.bg, Some(t.canvas));
    assert_eq!(dimmed.fg, Some(t.text_muted));
    let field = t.backdrop(t.on(t.field));
    assert_eq!(field.bg, Some(t.surface_elevated));
    let accent = t.backdrop(Style::new().fg(t.text_primary).bg(t.accent));
    assert_eq!(accent.bg, Some(t.surface_overlay));
    let blank = t.backdrop(Style::new().fg(t.canvas).bg(t.canvas));
    assert_eq!(blank.fg, blank.bg, "a blank cell stays blank");
    let mono = Theme::for_level(ColorLevel::Mono);
    assert!(
        mono.backdrop(mono.on(mono.canvas))
            .add_modifier
            .contains(Modifier::DIM)
    );
}

/// The base text styles and the theme's spacing and hint defaults.
fn theme_style_tokens() {
    let t = Theme::junie();
    assert_eq!(t.base().fg, Some(t.text_primary));
    assert_eq!(t.base().bg, Some(t.canvas));
    assert_eq!(t.on(t.field).bg, Some(t.field));
    assert_eq!(t.primary().fg, Some(t.text_primary));
    assert_eq!(t.secondary().fg, Some(t.text_secondary));
    assert_eq!(t.muted().fg, Some(t.text_muted));
    assert_eq!(t.faint().fg, Some(t.text_faint));
    assert_eq!(t.accent_fg().fg, Some(t.accent));
    assert_eq!(t.error_fg().fg, Some(t.error));
    assert!(t.title().add_modifier.contains(Modifier::BOLD));
    assert_eq!(t.label(true), t.title());
    assert_eq!(t.label(false), t.secondary());
    assert!(t.key_hint_key().add_modifier.contains(Modifier::BOLD));
    assert_eq!(t.key_hint_action().fg, Some(t.text_muted));
    assert_eq!(t.border(true).fg, Some(t.border_strong));
    assert_eq!(t.border(false).fg, Some(t.border_subtle));
    for plane in [t.canvas, t.surface] {
        assert_ne!(t.border_subtle, plane, "a subtle border shows on {plane:?}");
    }
    assert_ne!(t.border_strong, t.border_subtle);

    // Spacing, insets and the hint bar defaults ride on the theme.
    assert_eq!(Spacing::default(), Spacing::DEFAULT);
    assert_eq!(t.spacing, Spacing::DEFAULT);
    assert_eq!(Spacing::DEFAULT.button_gap, 2);
    assert_eq!(Spacing::DEFAULT.dialog, Insets::new(2, 1));
    assert_eq!(Spacing::DEFAULT.panel, Insets::new(2, 0));
    assert_eq!(Spacing::DEFAULT.popup, Insets::new(1, 0));
    assert_eq!(Insets::new(3, 4), Insets { x: 3, y: 4 });
    assert_eq!(HintBarStyle::default(), HintBarStyle::DEFAULT);
    assert_eq!(t.hintbar, HintBarStyle::DEFAULT);
    assert_eq!(HintBarStyle::DEFAULT.align, HintAlign::Start);
    assert!(HintBarStyle::DEFAULT.pinned_keys.contains(&"Esc"));
    assert_eq!(HintAlign::default(), HintAlign::Start);
    let centred = Theme::junie().centred_hints();
    assert_eq!(centred.hintbar.align, HintAlign::Center);
    assert_eq!(
        centred.hintbar.status_min_share,
        HintBarStyle::DEFAULT.status_min_share
    );

    // Editing and card defaults can be swapped per theme.
    assert_eq!(Theme::DEFAULT_EDIT_ENTRY, EditEntry::Immediate);
    assert_eq!(t.edit_entry, Theme::DEFAULT_EDIT_ENTRY);
    assert_eq!(
        t.with_edit_entry(EditEntry::TypeToEdit).edit_entry,
        EditEntry::TypeToEdit
    );
    assert_eq!(t.field_keys, None);
    assert_eq!(
        t.with_field_keys(FieldKeys::Emacs).field_keys,
        Some(FieldKeys::Emacs)
    );
    assert_eq!(t.card_heading, CardHeading::Rule);
    assert_eq!(
        t.with_card_heading(CardHeading::Band).card_heading,
        CardHeading::Band
    );
}

/// A state grid of specimens: one row per state, one column per specimen.
fn specimen_state_grid() {
    let t = Theme::junie();

    // The states: slugs round-trip, labels read well, `visual` maps each to the
    // flags the theme resolves.
    assert_eq!(SpecimenState::ALL.len(), 14);
    for state in SpecimenState::ALL {
        assert_eq!(SpecimenState::from_slug(state.slug()), Some(state));
    }
    assert_eq!(SpecimenState::from_slug("nope"), None);
    assert_eq!(SpecimenState::FocusedHover.slug(), "focused_hover");
    assert_eq!(SpecimenState::FocusedHover.label(), "focus + hover");
    assert_eq!(SpecimenState::Rest.label(), "rest");
    let visual = |s: SpecimenState| s.visual();
    assert_eq!(visual(SpecimenState::Rest), VisualState::default());
    assert!(visual(SpecimenState::Hover).hovered);
    let pressed = visual(SpecimenState::Pressed);
    assert!(pressed.pressed && pressed.hovered);
    assert!(visual(SpecimenState::Focused).focused);
    let both = visual(SpecimenState::FocusedHover);
    assert!(both.focused && both.hovered);
    assert!(visual(SpecimenState::Selected).selected);
    assert!(visual(SpecimenState::Disabled).disabled);
    assert!(visual(SpecimenState::Busy).busy);
    assert!(visual(SpecimenState::Error).error);
    let editing = visual(SpecimenState::Editing);
    assert!(editing.focused && editing.editing);
    for flat in [
        SpecimenState::Empty,
        SpecimenState::Loading,
        SpecimenState::Overflow,
        SpecimenState::Narrow,
    ] {
        assert_eq!(visual(flat), VisualState::default(), "{flat:?}");
    }

    // KindSample draws one resolved kind: the gutter, then the padded label.
    let sample = KindSample::new(
        WidgetId::of("scenario.specimen.kind"),
        Kind::Row,
        Plane::Surface,
        "Row",
    );
    assert_eq!(sample.plane, Plane::Surface);
    let sample = sample.plane(Plane::Card);
    assert_eq!(sample.plane, Plane::Card);
    let mut drawn = Buffer::empty(Rect::new(0, 0, 12, 1));
    sample.draw(
        &t,
        VisualState {
            focused: true,
            ..VisualState::default()
        },
        Rect::new(0, 0, 12, 1),
        &mut drawn,
    );
    assert_eq!(drawn[(0, 0)].symbol(), t.glyphs.focus_bar);
    assert!(text_of(&drawn).contains(" Row "));
    assert_eq!(drawn[(8, 0)].bg, t.ladder(Plane::Card).rest);

    // The Specimen trait: support, height and identity per state.
    let button_id = WidgetId::of("scenario.specimen.button");
    let button = Button::new(button_id, "Run", ButtonKind::Primary);
    assert_eq!(Specimen::specimen_id(&button), button_id);
    assert!(Specimen::supports(&button, SpecimenState::Busy));
    assert!(!Specimen::supports(&button, SpecimenState::Selected));
    assert!(!Specimen::supports(&sample, SpecimenState::Busy));
    assert_eq!(
        Specimen::specimen_height(&button, SpecimenState::Rest),
        1,
        "a specimen is one row unless it says otherwise"
    );
    let field = TextInput::new(WidgetId::of("scenario.specimen.input"), "Name");
    assert_eq!(Specimen::specimen_height(&field, SpecimenState::Rest), 2);
    assert_eq!(Specimen::specimen_height(&field, SpecimenState::Error), 3);

    // The grid: builders, size and per-row heights.
    let default_grid = StateGrid::new(WidgetId::of("scenario.specimen.default"));
    assert_eq!(default_grid.size(), (15, 1));
    assert_eq!(default_grid.mark(&t), "▏");
    assert_eq!(default_grid.mark(&t.with_glyphs(Glyphs::ASCII)), ":");
    let states = [
        SpecimenState::Rest,
        SpecimenState::Hover,
        SpecimenState::Pressed,
        SpecimenState::Focused,
        SpecimenState::Disabled,
    ];
    let mut grid = StateGrid::new(WidgetId::of("scenario.specimen.grid"))
        .rows(&states)
        .row(SpecimenState::Busy)
        .column("Button", button)
        .column("Kind", sample)
        .column("Input", field)
        .label_width(12)
        .column_width(14)
        .narrow_width(10)
        .focus_mark("|");
    assert_eq!(
        grid.column_labels().collect::<Vec<_>>(),
        ["Button", "Kind", "Input"]
    );
    assert_eq!(
        (grid.label_width, grid.column_width, grid.narrow_width),
        (12, 14, 10)
    );
    assert_eq!(grid.mark(&t), "|");
    assert_eq!(grid.rows.len(), 6);
    let heights = grid.row_heights();
    assert_eq!(
        heights,
        [2, 2, 1, 2, 2, 1],
        "a row is as tall as its tallest supporting specimen"
    );
    let (w, h) = grid.size();
    assert_eq!(w, 12 + 14 * 3);
    assert_eq!(h, 1 + heights.iter().sum::<u16>());

    // Render: header, a row per state, unsupported cells as a dash.
    let area = Rect::new(0, 0, w, h);
    let buf = paint_with(&t, Interaction::default(), area, |a, b, c| {
        grid.render(a, b, c, t.canvas);
    });
    let text = text_of(&buf);
    for label in [
        "Button", "Kind", "Input", "rest", "hover", "pressed", "focused", "disabled", "busy",
    ] {
        assert!(text.contains(label), "grid shows {label}");
    }
    assert_eq!(grid.rendered_rows().len(), 6);
    assert!(text.contains('–'), "an unsupported state shows a dash");
    let focused_y = 1 + heights[0] + heights[1] + heights[2];
    assert_eq!(
        buf[(12, focused_y)].symbol(),
        "|",
        "the live focus bar is replaced by the grid's mark"
    );
    let (rest_y, hover_y) = (1u16, 1 + heights[0]);
    assert_ne!(
        buf[(12 + 2, rest_y)].bg,
        buf[(12 + 2, hover_y)].bg,
        "a hovered primary button differs from a resting one"
    );

    // A Narrow row clamps every cell to narrow_width; other rows keep the
    // column width. TextInput supports both states and is two rows tall.
    let mut clamp = StateGrid::new(WidgetId::of("scenario.specimen.clamp"))
        .row(SpecimenState::Rest)
        .row(SpecimenState::Narrow)
        .column(
            "Input",
            TextInput::new(WidgetId::of("scenario.specimen.narrow"), "Name"),
        )
        .label_width(12)
        .column_width(24)
        .narrow_width(8);
    let (cw, ch) = clamp.size();
    let clamped = paint_with(
        &t,
        Interaction::default(),
        Rect::new(0, 0, cw, ch),
        |a, b, c| {
            clamp.render(a, b, c, t.canvas);
        },
    );
    let far = 12 + 16;
    let field_at = |ys: std::ops::Range<u16>| {
        let mut ys = ys;
        ys.any(|y| clamped[(far, y)].bg == t.field)
    };
    assert!(field_at(1..3), "a rest field spans the column");
    assert!(!field_at(3..5), "a narrow field stops at 8 cells");

    // A slice of rows, without the header.
    let mut part = StateGrid::new(WidgetId::of("scenario.specimen.part"))
        .rows(&states)
        .column(
            "Button",
            Button::new(
                WidgetId::of("scenario.specimen.pb"),
                "Run",
                ButtonKind::Secondary,
            ),
        );
    let body = Rect::new(0, 0, 30, 4);
    let sliced = paint_with(&t, Interaction::default(), body, |a, b, c| {
        part.render_rows(1..3, false, a, b, c, t.canvas);
    });
    assert_eq!(
        part.rendered_rows(),
        [SpecimenState::Hover, SpecimenState::Pressed]
    );
    assert!(
        !text_of(&sliced).contains("Button"),
        "no header was asked for"
    );
    assert!(text_of(&sliced).contains("hover"));

    // One colour level, and all four levels side by side or stacked.
    let level_area = Rect::new(0, 0, 100, 12);
    let level_buf = paint_with(&t, Interaction::default(), level_area, |a, b, c| {
        part.render_level(ColorLevel::Mono, a, b, c);
    });
    assert!(text_of(&level_buf).contains(ColorLevel::Mono.label()));
    assert_eq!(
        level_buf[(0, 1)].bg,
        Theme::for_level(ColorLevel::Mono).surface
    );
    assert_eq!(part.rendered_rows().len(), 5);
    let wide = part.levels_height(200);
    let narrow = part.levels_height(20);
    assert!(narrow > wide, "levels stack when the area is narrow");
    let levels_buf = paint_with(
        &t,
        Interaction::default(),
        Rect::new(0, 0, 200, wide),
        |a, b, c| {
            part.render_levels(a, b, c);
        },
    );
    let levels_text = text_of(&levels_buf);
    for level in ColorLevel::ALL {
        assert!(levels_text.contains(level.label()), "{level:?} is drawn");
    }
    assert_eq!(part.rendered_rows().len(), 5);
}

fn ctx_for<R>(theme: &Theme, f: impl FnOnce(&mut RenderCtx) -> R) -> R {
    let mut hits = HitRegistry::default();
    let mut ring = FocusRing::default();
    let mut ctx = RenderCtx::new(theme, Interaction::default(), &mut hits, &mut ring);
    f(&mut ctx)
}

/// The text helpers every widget measures, clips, wraps and formats with.
fn text_utility_functions() {
    // Width and the cheap width checks.
    assert_eq!(text::width("界a"), 3);
    assert!(!text::wider_than("hello", 5));
    assert!(text::wider_than("hello!", 5));
    assert_eq!(text::width_capped("ab", 4), 2);
    assert_eq!(
        text::width_capped("界界界", 4),
        5,
        "a capped width reports cap + 1"
    );
    assert_eq!(text::capitalize("hello"), "Hello");
    assert_eq!(text::capitalize("éa"), "Éa");
    assert_eq!(text::capitalize(""), "");

    // Clipping, truncating and fitting.
    assert_eq!(text::clip("hello", 3), "hel");
    assert_eq!(text::clip("界界", 3), "界");
    assert_eq!(text::truncate("hello world", 6), "hello…");
    assert_eq!(text::truncate("hi", 6), "hi");
    assert_eq!(text::truncate("abc", 0), "");
    assert_eq!(text::truncate("界界界", 5), "界界…");
    assert_eq!(text::fit("abc", 5), "abc  ");
    assert_eq!(text::fit("abcdef", 4), "abc…");
    assert_eq!(text::fit_right("abc", 5), "  abc");
    assert_eq!(text::truncate_middle("abcdefghij", 7), "abcd…ij");
    assert_eq!(text::truncate_middle("abc", 7), "abc");
    assert_eq!(text::truncate_middle("abcdef", 4), "abc…");
    assert_eq!(
        text::truncate_sep("alpha · beta · gamma", 14),
        "alpha · beta…",
        "a cut lands on a separator when one fits"
    );
    assert_eq!(text::truncate_sep("short", 14), "short");
    assert_eq!(
        text::truncate_sep("nosepatall long", 8),
        text::truncate("nosepatall long", 8)
    );
    let path = "Home › Settings › Network › Proxy";
    assert_eq!(text::crumb_fit(path, 40), path);
    assert_eq!(text::crumb_fit(path, 20), "Home › … › Proxy");
    let squeezed = text::crumb_fit("Home › A very long single name", 12);
    assert!(text::width(&squeezed) <= 12 && squeezed.contains('…'));
    assert_eq!(text::center_x(Rect::new(10, 0, 20, 1), "abcd"), 18);
    assert_eq!(text::center_x(Rect::new(10, 0, 3, 1), "abcdef"), 10);

    // Slicing a window out of a longer line marks both clipped edges.
    assert_eq!(text::slice_cells("hello world", 0, 5), "hell…");
    assert_eq!(text::slice_cells("hello world", 6, 5), "…orld");
    assert_eq!(text::slice_cells("hello", 0, 0), "");
    assert_eq!(text::slice_cells("hello", 0, 9), "hello");

    // Numbers and durations.
    assert_eq!(text::plural(1, "row", "rows"), "1 row");
    assert_eq!(text::plural(0, "row", "rows"), "0 rows");
    assert_eq!(text::plural(2, "row", "rows"), "2 rows");
    assert_eq!(text::thousands(0), "0");
    assert_eq!(text::thousands(999), "999");
    assert_eq!(text::thousands(1000), "1,000");
    assert_eq!(text::thousands(1_234_567), "1,234,567");
    assert_eq!(text::bytes(0), "0 B");
    assert_eq!(text::bytes(1023), "1023 B");
    assert_eq!(text::bytes(1024), "1.0 KiB");
    assert_eq!(text::bytes(1536), "1.5 KiB");
    assert_eq!(text::bytes(5 * 1024 * 1024 * 1024), "5.0 GiB");
    assert_eq!(DurationStyle::default(), DurationStyle::Spoken);
    let ms = |n| Duration::from_millis(n);
    assert_eq!(text::duration(ms(0), DurationStyle::Compact), "<1 ms");
    assert_eq!(text::duration(ms(250), DurationStyle::Compact), "250 ms");
    assert_eq!(text::duration(ms(1500), DurationStyle::Compact), "1.50 s");
    assert_eq!(text::duration(ms(125_000), DurationStyle::Compact), "2m 5s");
    let secs = |n| Duration::from_secs(n);
    assert_eq!(text::duration(secs(45), DurationStyle::Spoken), "45 s");
    assert_eq!(
        text::duration(secs(125), DurationStyle::Spoken),
        "2 min 5 s"
    );
    assert_eq!(text::duration(secs(3600), DurationStyle::Spoken), "1 h");
    assert_eq!(
        text::duration(secs(3660), DurationStyle::Spoken),
        "1 h 1 min"
    );
    assert_eq!(
        text::duration(secs(90_000), DurationStyle::Spoken),
        "1 d 1 h"
    );

    // Search: case-insensitive ranges in source bytes, and the fuzzy ranking.
    assert_eq!(
        text::find_ranges("Hello hello", "hello", false),
        [0..5, 6..11]
    );
    let exact = text::find_ranges("Hello hello", "hello", true);
    assert_eq!((exact.len(), exact[0].clone()), (1, 6..11));
    assert!(text::find_ranges("Hello", "", false).is_empty());
    assert_eq!(text::fuzzy("filter", ""), Some((0, vec![])));
    assert_eq!(text::fuzzy("filter", "fil"), Some((0, vec![0, 1, 2])));
    assert_eq!(text::fuzzy("my_filter", "fil"), Some((10, vec![3, 4, 5])));
    assert_eq!(text::fuzzy("myfilter", "fil"), Some((30, vec![2, 3, 4])));
    assert_eq!(text::fuzzy("filter_rows", "fr"), Some((65, vec![0, 5])));
    assert_eq!(text::fuzzy("abc", "z"), None);

    // Wrapping keeps indent and hangs continuation rows under the gap; PLAIN
    // drops both.
    assert_eq!(
        text::wrap("one two three four", 9),
        ["one two", "three", "four"]
    );
    assert_eq!(
        text::wrap("  indented text here", 12),
        ["  indented", "  text here"]
    );
    assert_eq!(
        text::wrap_with("  indented text here", 12, WrapOpts::PLAIN),
        ["indented", "text here"]
    );
    let hung = text::wrap("key   alpha beta gamma delta", 16);
    assert!(hung.len() >= 2);
    assert!(
        hung[1..].iter().all(|row| row.starts_with("      ")),
        "continuation rows hang under the description: {hung:?}"
    );
    let flat = text::wrap_with("key   alpha beta gamma delta", 16, WrapOpts::PLAIN);
    assert!(flat[1..].iter().all(|row| !row.starts_with(' ')));
    assert!(WrapOpts::default().keep_indent && !WrapOpts::PLAIN.keep_indent);

    // A fitted label painted into a buffer occupies exactly its cells.
    let mut buf = Buffer::empty(Rect::new(0, 0, 8, 1));
    buf.set_string(0, 0, text::fit_right("a界b", 8), Style::new());
    assert_eq!(buf[(4, 0)].symbol(), "a");
    assert_eq!(buf[(5, 0)].symbol(), "界");
    assert_eq!(buf[(7, 0)].symbol(), "b");
}

/// A row paint resolves the theme once, fills, draws the gutter and marks.
fn row_painting() {
    let theme = Theme::junie();
    let area = Rect::new(0, 0, 20, 1);
    let focused = VisualState {
        focused: true,
        ..VisualState::default()
    };
    let id = WidgetId::of("scenario.row.label");
    ctx_for(&theme, |ctx| {
        // Bar keeps the plane and shows the focus bar in the gutter.
        let mut buf = Buffer::empty(area);
        assert_eq!(RowLook::default(), RowLook::Bar);
        let bar: RowPaint = RowUi::paint(ctx, &mut buf, area, focused, RowLook::Bar, theme.surface);
        assert_eq!(buf[(0, 0)].symbol(), theme.glyphs.focus_bar);
        assert_eq!(bar.bg(), theme.surface);
        assert_eq!(bar.ink(ctx, Color::Red), Color::Red);
        assert_eq!(bar.area, area);
        assert_eq!(bar.plane, theme.surface);
        assert_eq!(bar.state, focused);

        // Fill is the cursor: the highlight, and ink forced to primary.
        let fill_look = RowLook::from(CursorStyle::Fill);
        assert_eq!(fill_look, RowLook::Fill(CursorStyle::Fill));
        let resolved = RowUi::resolve(ctx, area, focused, fill_look, theme.surface);
        assert_eq!(resolved.bg(), theme.highlight);
        assert_eq!(resolved.style().bg, Some(theme.highlight));
        assert_eq!(resolved.ink(ctx, Color::Red), theme.text_primary);
        assert_eq!(buf[(5, 0)].bg, theme.surface, "resolve paints nothing");
        resolved.draw(&mut buf);
        assert_eq!(buf[(5, 0)].bg, theme.highlight, "draw fills the area");
        assert_eq!(buf[(0, 0)].symbol(), " ", "a fill cursor has no gutter bar");

        // Marks: the glyph per kind, in the chosen colour; disabled rows keep
        // the row's own style.
        let selected = VisualState {
            selected: true,
            ..VisualState::default()
        };
        let row = RowUi::resolve(ctx, area, selected, RowLook::Bar, theme.surface);
        let mut marks = Buffer::empty(area);
        row.draw(&mut marks);
        row.mark(ctx, &mut marks, 2, 0, MarkKind::Current, false);
        row.mark(ctx, &mut marks, 4, 0, MarkKind::Tick, false);
        row.mark(ctx, &mut marks, 6, 0, MarkKind::Check, true);
        row.mark(ctx, &mut marks, 12, 0, MarkKind::Glyph("*"), false);
        assert_eq!(marks[(2, 0)].symbol(), theme.glyphs.current);
        assert_eq!(marks[(4, 0)].symbol(), theme.glyphs.tick);
        assert_eq!(marks[(6, 0)].symbol(), "[");
        assert_eq!(marks[(12, 0)].symbol(), "*");
        assert_eq!(
            marks[(2, 0)].fg,
            theme.legible(theme.accent, row.bg()),
            "an inactive selection keeps the accent mark"
        );
        let dead = RowUi::resolve(
            ctx,
            area,
            VisualState {
                disabled: true,
                ..selected
            },
            RowLook::Bar,
            theme.surface,
        );
        let mut off = Buffer::empty(area);
        dead.mark(ctx, &mut off, 2, 0, MarkKind::Tick, true);
        assert_eq!(off[(2, 0)].fg, dead.style().fg.expect("a foreground"));

        // Labels: cut text is reported so a reveal can offer the full text.
        let mut reveal = RevealState::default();
        let mut text = Buffer::empty(area);
        let cell = Rect::new(2, 0, 6, 1);
        assert!(row.label(
            ctx,
            &mut text,
            &mut reveal,
            id,
            cell,
            "a long label",
            row.style()
        ));
        assert!(text_of(&text).contains("a lon…"));
        assert!(!row.label(ctx, &mut text, &mut reveal, id, cell, "short", row.style()));
        assert!(!dead.label(
            ctx,
            &mut text,
            &mut reveal,
            id,
            cell,
            "a long label",
            dead.style()
        ));
    });
}

/// Loading, error and empty content states and what they draw.
fn content_states() {
    let theme = Theme::junie();
    let area = Rect::new(0, 0, 24, 5);
    let ready = ContentState::default();
    assert!(ready.is_ready() && ready.actions().is_empty());
    assert!(matches!(ready, ContentState::Ready));

    let loading = ContentState::loading("Fetching");
    assert!(!loading.is_ready());
    assert!(matches!(&loading, ContentState::Loading(label) if label.as_ref() == "Fetching"));
    assert!(loading.actions().is_empty());

    let retry = WidgetId::of("scenario.content.retry");
    let failed = ContentState::error(
        EmptyState::new("Boom").actions(vec![EmptyAction::primary(retry, "Retry")]),
    );
    assert!(!failed.is_ready());
    let ContentState::Error(inner) = &failed else {
        panic!("an error state holds its empty state");
    };
    assert_eq!(
        inner.kind,
        EmptyKind::Error,
        "error() forces the error kind"
    );
    assert_eq!(failed.actions().len(), 1);
    assert_eq!(failed.actions()[0].id, retry);

    // The state a specimen row asks for.
    assert!(content::specimen(SpecimenState::Rest).is_ready());
    assert!(matches!(
        content::specimen(SpecimenState::Loading),
        ContentState::Loading(label) if label.is_empty()
    ));
    let ContentState::Error(shown) = content::specimen(SpecimenState::Error) else {
        panic!("the error specimen is an error state");
    };
    assert_eq!(shown.title, "Could not load");

    // draw_line centres a muted line and truncates it.
    let mut line = Buffer::empty(area);
    content::draw_line(area, &mut line, &theme, "Nothing", theme.canvas);
    assert_eq!(rows(&line)[2].trim(), "Nothing");
    assert_eq!(line[(8, 2)].symbol(), "N");
    assert_eq!(line[(8, 2)].fg, theme.text_muted);
    let mut cut = Buffer::empty(Rect::new(0, 0, 4, 1));
    content::draw_line(
        Rect::new(0, 0, 4, 1),
        &mut cut,
        &theme,
        "Nothing",
        theme.canvas,
    );
    assert_eq!(text_of(&cut), "Not…");

    // draw_loading shows the spinner and the label, or the theme's default.
    let loaded = paint(area, |a, b, c| {
        content::draw_loading(a, b, c, "Fetching", theme.canvas);
    });
    assert!(text_of(&loaded).contains("Fetching"));
    assert!(text_of(&loaded).contains("⠋"), "the first spinner frame");
    let default_label = paint(Rect::new(0, 0, 30, 3), |a, b, c| {
        content::draw_loading(a, b, c, "", theme.canvas);
    });
    assert!(text_of(&default_label).contains(Strings::DEFAULT.loading_rows));

    // draw_empty: a bare empty state is one line; a hint or an action uses the
    // full empty layout, with the action as a button.
    let plain = paint(area, |a, b, c| {
        content::draw_empty(a, b, c, &EmptyState::new("No rows"), theme.canvas);
    });
    assert_eq!(rows(&plain)[2].trim(), "No rows");
    let hinted = paint(Rect::new(0, 0, 40, 8), |a, b, c| {
        let e = EmptyState::new("No rows").hint("Run a query first");
        content::draw_empty(a, b, c, &e, theme.canvas);
    });
    assert!(text_of(&hinted).contains("No rows") && text_of(&hinted).contains("Run a query"));
    let errored = paint(Rect::new(0, 0, 40, 8), |a, b, c| {
        content::draw_empty(a, b, c, inner, theme.canvas);
    });
    assert!(text_of(&errored).contains("Boom") && text_of(&errored).contains("Retry"));
}

/// Tiling: natural sizes in, rectangles out, wrapping when a row is too narrow.
fn measure_tiles() {
    struct Label(&'static str);
    impl Measure for Label {
        fn measure(&self, _avail: Size, _ctx: &RenderCtx) -> Size {
            Size::new(text::width(self.0) as u16, 1)
        }
    }
    let theme = Theme::junie();
    ctx_for(&theme, |ctx| {
        let label = Label("abcde");
        assert_eq!(label.measure(Size::new(80, 24), ctx), Size::new(5, 1));
        assert_eq!(
            label.min_width(Size::new(80, 24), ctx),
            5,
            "min_width defaults to the natural width"
        );
    });
    assert_eq!(
        measure::at_least(Size::new(3, 5), Size::new(10, 2)),
        Size::new(10, 5)
    );

    let one = |_w: u16| 1u16;
    let by_width = |w: u16| if w >= 18 { 1u16 } else { 2 };

    // Growing tiles share the spare width; the last takes the remainder.
    let (a, b) = (Tile::new(5, &one), Tile::new(5, &one));
    let grown: measure::Tiled = measure::tiles(20, &[&[a, b]], (2, 1));
    assert_eq!(grown.rects, [Rect::new(0, 0, 9, 1), Rect::new(11, 0, 9, 1)]);
    assert_eq!(grown.size, Size::new(20, 1));

    // An odd spare width goes to the last growing tile.
    let (a, b) = (Tile::new(5, &one), Tile::new(5, &one));
    let odd = measure::tiles(21, &[&[a, b]], (2, 1));
    assert_eq!(odd.rects, [Rect::new(0, 0, 9, 1), Rect::new(11, 0, 10, 1)]);

    // A fixed tile keeps its width.
    let (fixed, grow) = (Tile::new(6, &one).fixed(), Tile::new(4, &one));
    assert!(!fixed.grow && grow.grow);
    let mixed = measure::tiles(20, &[&[fixed, grow]], (1, 0));
    assert_eq!(mixed.rects, [Rect::new(0, 0, 6, 1), Rect::new(7, 0, 13, 1)]);

    // Too narrow: the row wraps into a column, heights come from the width.
    let (a, b) = (Tile::new(8, &by_width), Tile::new(8, &by_width));
    let stacked = measure::tiles(10, &[&[a, b]], (2, 1));
    assert_eq!(
        stacked.rects,
        [Rect::new(0, 0, 10, 2), Rect::new(0, 3, 10, 2)]
    );
    assert_eq!(stacked.size, Size::new(10, 5));

    // Wide enough, the same tiles take the short height; two rows get the row
    // gap between them, and an empty row is skipped.
    let (a, b, c) = (
        Tile::new(8, &by_width),
        Tile::new(8, &by_width),
        Tile::new(4, &one),
    );
    let two_rows = measure::tiles(40, &[&[a, b], &[], &[c]], (2, 1));
    assert_eq!(two_rows.rects.len(), 3);
    assert_eq!(two_rows.rects[0].height, 1);
    assert_eq!(two_rows.rects[2].y, 2, "row height 1 plus gap 1");
    assert_eq!(two_rows.size.height, 3);

    // Tiled::at moves a tile to where its container sits; a missing tile is an
    // empty rect at the origin.
    let origin = Rect::new(4, 3, 40, 10);
    assert_eq!(grown.at(1, origin), Rect::new(15, 3, 9, 1));
    assert_eq!(grown.at(9, origin), Rect::new(4, 3, 0, 0));

    // Painted at their rects, the tiles land side by side.
    let mut buf = Buffer::empty(Rect::new(0, 0, 48, 14));
    for (i, name) in ["A", "B"].into_iter().enumerate() {
        let r = grown.at(i, origin);
        buf.set_string(r.x, r.y, name, Style::new());
    }
    assert_eq!(buf[(4, 3)].symbol(), "A");
    assert_eq!(buf[(15, 3)].symbol(), "B");
}

fn fg_of(buf: &Buffer, x: u16, y: u16) -> Color {
    buf[(x, y)].fg
}

/// The scroll-edge fade: outer rows dim more than inner ones, and only where
/// there is more content in that direction.
fn fade_scroll_edges() {
    let theme = Theme::junie();
    let ink = Color::Rgb(200, 200, 200);
    let canvas = Color::Rgb(0, 0, 0);
    let filled = |height: u16| {
        let area = Rect::new(0, 0, 10, height);
        let mut buf = Buffer::empty(area);
        for pos in area.positions() {
            buf[pos].set_fg(ink).set_bg(canvas);
        }
        (area, buf)
    };
    let mixed = |keep: f32| {
        Color::Rgb(
            (200.0 * keep).round() as u8,
            (200.0 * keep).round() as u8,
            (200.0 * keep).round() as u8,
        )
    };
    let scrolled = |len: usize, view: usize, offset: usize| {
        let mut s = ScrollState::new(len);
        s.set_viewport(view);
        s.offset = offset;
        s
    };

    // A tall area scrolled into the middle fades two rows at each edge.
    let height = fade::DEEP_FROM;
    let (area, mut buf) = filled(height);
    ctx_for(&theme, |ctx| {
        fade::scroll_edges(&mut buf, ctx, area, &scrolled(100, height as usize, 10));
    });
    assert_eq!(fg_of(&buf, 3, 0), mixed(fade::OUTER_KEEP));
    assert_eq!(fg_of(&buf, 3, 1), mixed(fade::INNER_KEEP));
    assert_eq!(fg_of(&buf, 3, 2), ink, "the middle is untouched");
    assert_eq!(fg_of(&buf, 3, height - 2), mixed(fade::INNER_KEEP));
    assert_eq!(fg_of(&buf, 3, height - 1), mixed(fade::OUTER_KEEP));

    // A short area fades one row per edge; below MIN_ROWS nothing fades.
    let (area, mut short) = filled(fade::DEEP_FROM - 1);
    ctx_for(&theme, |ctx| {
        fade::scroll_edges(&mut short, ctx, area, &scrolled(100, 11, 10));
    });
    assert_eq!(fg_of(&short, 0, 0), mixed(fade::OUTER_KEEP));
    assert_eq!(fg_of(&short, 0, 1), ink);
    let (area, mut tiny) = filled(fade::MIN_ROWS - 1);
    ctx_for(&theme, |ctx| {
        fade::scroll_edges(&mut tiny, ctx, area, &scrolled(100, 3, 10));
    });
    assert!(area.positions().all(|p| tiny[p].fg == ink));

    // Only the side with more content fades; a fitting list fades nothing.
    let (area, mut top) = filled(height);
    ctx_for(&theme, |ctx| {
        fade::scroll_edges(&mut top, ctx, area, &scrolled(100, height as usize, 0));
    });
    assert_eq!(fg_of(&top, 0, 0), ink);
    assert_eq!(fg_of(&top, 0, height - 1), mixed(fade::OUTER_KEEP));
    let (area, mut fits) = filled(height);
    ctx_for(&theme, |ctx| {
        fade::scroll_edges(&mut fits, ctx, area, &scrolled(5, height as usize, 0));
    });
    assert!(area.positions().all(|p| fits[p].fg == ink));

    // `scroll_edges_except` leaves the kept rows and the cursor row alone.
    let (area, mut kept) = filled(height);
    ctx_for(&theme, |ctx| {
        fade::scroll_edges_except(
            &mut kept,
            ctx,
            area,
            &scrolled(100, height as usize, 10),
            &[0],
        );
    });
    assert_eq!(fg_of(&kept, 0, 0), ink, "a kept row is not faded");
    assert_eq!(fg_of(&kept, 0, height - 1), mixed(fade::OUTER_KEEP));
    let (area, mut cursor) = filled(height);
    let mut hits = HitRegistry::default();
    let mut ring = FocusRing::default();
    let mut ctx = RenderCtx::new(&theme, Interaction::default(), &mut hits, &mut ring);
    ctx.cursor = Some(Position::new(2, height - 1));
    fade::scroll_edges(&mut cursor, &ctx, area, &scrolled(100, height as usize, 10));
    assert_eq!(
        fg_of(&cursor, 0, height - 1),
        ink,
        "the cursor row stays crisp"
    );
    assert_eq!(fg_of(&cursor, 0, 0), mixed(fade::OUTER_KEEP));

    // Without true colour the outer rows dim by modifier instead.
    let indexed = Theme::for_level(ColorLevel::Ansi256);
    let area = Rect::new(0, 0, 10, height);
    let mut dim = Buffer::empty(area);
    for pos in area.positions() {
        dim[pos]
            .set_fg(Color::Indexed(231))
            .set_bg(Color::Indexed(16));
    }
    ctx_for(&indexed, |ctx| {
        fade::scroll_edges(&mut dim, ctx, area, &scrolled(100, height as usize, 10));
    });
    assert!(dim[(0, 0)].modifier.contains(Modifier::DIM));
    assert!(!dim[(0, 1)].modifier.contains(Modifier::DIM));
    assert!(!dim[(0, 2)].modifier.contains(Modifier::DIM));
}

/// A status bar's segments: widths, priorities, dropping and click regions.
fn status_segments() {
    let theme = Theme::junie();
    let link = WidgetId::of("scenario.segments.link");
    let left = [
        Segment::new("main", Tone::Normal)
            .bold()
            .lead("●", Tone::Success),
        Segment::new("clickable", Tone::Info)
            .clickable(link)
            .priority(9),
    ];
    let right = [Segment::new("12:30", Tone::Muted).priority(1)];
    assert_eq!(
        left[0].priority, 5,
        "the default priority is the middle one"
    );
    assert!(left[0].bold && left[0].id.is_none());
    assert_eq!(left[0].lead, Some(("●".to_owned(), Tone::Success)));
    assert_eq!(left[1].id, Some(link));
    assert_eq!(left[1].priority, 9);

    // 6 + 9 + 5 cells, two gaps of 2, and the chrome edge twice.
    assert_eq!(
        segments::width(&left, &right, &theme),
        6 + 2 + 9 + 2 + 5 + 4
    );
    assert_eq!(
        segments::min_width(&left, &right, &theme),
        9 + 3 + 4,
        "the narrowest bar still shows the strongest segment"
    );

    // Wide enough, everything shows and the clickable one registers a region.
    let wide = paint_hits(Rect::new(0, 0, 40, 1), |a, b, c| {
        segments::render(a, b, c, &left, &right, theme.surface);
    });
    let text = text_of(&wide.buf);
    assert!(text.contains("● main") && text.contains("clickable") && text.contains("12:30"));
    let region = wide
        .hits
        .regions()
        .iter()
        .find(|r| r.id == link)
        .expect("a clickable segment registers a hit region");
    assert!(region.area.width >= 9);
    let main_x = (0..40)
        .find(|&x| wide.buf[(x, 0)].symbol() == "m")
        .expect("main is drawn");
    assert!(
        wide.buf[(main_x, 0)].modifier.contains(Modifier::BOLD),
        "a bold segment is bold"
    );
    assert!(
        !wide.buf[(main_x - 2, 0)].modifier.contains(Modifier::BOLD),
        "its lead glyph is not"
    );
    assert_eq!(
        wide.buf[(main_x - 2, 0)].fg,
        theme.legible(theme.tone(Tone::Success), theme.surface)
    );

    // Squeezed, the lowest priority goes first and the rest keep their order.
    let squeezed = paint_hits(Rect::new(0, 0, 26, 1), |a, b, c| {
        segments::render(a, b, c, &left, &right, theme.surface);
    });
    let text = text_of(&squeezed.buf);
    assert!(!text.contains("12:30"), "the weakest segment is dropped");
    assert!(text.contains("clickable"), "the strongest survives");

    // On equal priority the rightmost right segment goes first.
    let tie_left = [Segment::new("keep", Tone::Normal).priority(9)];
    let tie_right = [
        Segment::new("r1", Tone::Muted).priority(3),
        Segment::new("r2", Tone::Muted).priority(3),
    ];
    let full = segments::width(&tie_left, &tie_right, &theme);
    let tight = paint_hits(Rect::new(0, 0, full - 1, 1), |a, b, c| {
        segments::render(a, b, c, &tie_left, &tie_right, theme.surface);
    });
    let text = text_of(&tight.buf);
    assert!(
        text.contains("keep") && text.contains("r1") && !text.contains("r2"),
        "{text:?}"
    );
}

/// The brand lockup, plain and as a click target.
fn brand_lockup() {
    let theme = Theme::junie();
    let full = Lockup::new("junie");
    let compact = Lockup::compact("junie");
    assert!(!full.compact && compact.compact);
    assert_eq!(full.width(), 7, "a full lockup pads both sides");
    assert_eq!(compact.width(), 5);
    let style = Lockup::style(&theme);
    assert_eq!(style.bg, Some(theme.accent));
    assert_eq!(style.fg, Some(theme.text_on_accent));
    assert!(style.add_modifier.contains(Modifier::BOLD));

    let mut buf = Buffer::empty(Rect::new(0, 0, 20, 2));
    assert_eq!(full.render(1, 0, &mut buf, &theme), 7);
    assert_eq!(compact.render(1, 1, &mut buf, &theme), 5);
    assert_eq!(buf[(2, 0)].symbol(), "j");
    assert_eq!(
        buf[(1, 0)].bg,
        theme.accent,
        "the padding is part of the plate"
    );
    assert_eq!(buf[(1, 1)].symbol(), "j");

    // Clickable: registers its region and follows hover and press.
    let id = WidgetId::of("scenario.brand.lockup");
    let draw = |interaction: Interaction| {
        let mut buf = Buffer::empty(Rect::new(0, 0, 20, 1));
        let mut hits = HitRegistry::default();
        let mut ring = FocusRing::default();
        let w = {
            let mut ctx = RenderCtx::new(&theme, interaction, &mut hits, &mut ring);
            full.render_clickable(2, 0, &mut buf, &mut ctx, id)
        };
        (buf, hits, w)
    };
    let (rest, hits, w) = draw(Interaction::default());
    assert_eq!(w, 7);
    assert!(
        hits.regions()
            .iter()
            .any(|r| r.id == id && r.area == Rect::new(2, 0, 7, 1))
    );
    let (hover, _, _) = draw(Interaction {
        hover: Some(id),
        ..Default::default()
    });
    let (press, _, _) = draw(Interaction {
        hover: Some(id),
        pressed: Some(id),
        ..Default::default()
    });
    assert_eq!(rest[(3, 0)].bg, theme.ladders.accent.rest);
    assert_eq!(hover[(3, 0)].bg, theme.ladders.accent.hover);
    assert_eq!(press[(3, 0)].bg, theme.ladders.accent.pressed);
    let (focused, _, _) = draw(Interaction {
        focus: Some(id),
        ..Default::default()
    });
    assert_eq!(
        focused[(2, 0)].symbol(),
        " ",
        "the lockup is a brand mark, not a focus stop"
    );
}

/// The key reference table: sections, keys, gap and the key column's share.
fn key_reference_layout() {
    let r = KeyReference::new()
        .section("Edit")
        .key("Ctrl+C", "Copy")
        .key("Ctrl+Shift+V", "Paste plain")
        .section("View")
        .key("F11", "Full screen");
    assert_eq!((r.gap, r.key_max_share), (3, 40));
    let line = |keys: &str, action: &str| format!("{keys:<12}   {action}");
    let expected = [
        "Edit".to_owned(),
        line("Ctrl+C", "Copy"),
        line("Ctrl+Shift+V", "Paste plain"),
        String::new(),
        "View".to_owned(),
        line("F11", "Full screen"),
    ];
    assert_eq!(r.text(), expected.join("\n"));
    assert_eq!(r.lines(80), expected, "at a wide width nothing wraps");

    // The gap has a floor of three cells and grows past it.
    assert_eq!(KeyReference::new().key("A", "x").gap(1).text(), "A   x");
    assert_eq!(
        KeyReference::new().key("A", "x").gap(6).text(),
        format!("A{}x", " ".repeat(6))
    );
    assert_eq!(r.clone().gap(6).gap, 6);

    // The key column takes at most its share of the width: at a 40% share the
    // keys stay aligned, at 20% they are cut loose and no longer pad.
    assert_eq!(r.clone().key_max_share(200).key_max_share, 100);
    assert_eq!(r.lines(30)[1], line("Ctrl+C", "Copy"));
    assert_eq!(r.clone().key_max_share(20).lines(30)[1], "Ctrl+C   Copy");
    let tight = r.clone().key_max_share(0).lines(24);
    assert!(
        tight.iter().all(|l| text::width(l) <= 24),
        "a zero share still lays out inside the width"
    );

    // A long action wraps onto a row hung under the action column.
    let long = KeyReference::new().key("Ctrl+Shift+V", "Paste without formatting");
    let wrapped = long.lines(30);
    assert_eq!(wrapped.len(), 2, "{wrapped:?}");
    assert!(wrapped[0].starts_with("Ctrl+Shift+V   Paste"));
    assert_eq!(wrapped[1], format!("{}formatting", " ".repeat(15)));

    // Painted, the lines land one per row.
    let mut buf = Buffer::empty(Rect::new(0, 0, 40, 6));
    for (y, l) in r.lines(40).iter().enumerate() {
        buf.set_string(0, y as u16, l, Style::new());
    }
    assert_eq!(rows(&buf)[1].trim_end(), expected[1]);
    assert_eq!(rows(&buf)[4].trim_end(), "View");
}
