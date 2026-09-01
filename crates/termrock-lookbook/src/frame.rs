// SPDX-FileCopyrightText: 2026 Alexey Zhokhov
// SPDX-License-Identifier: Apache-2.0

//! Pure terminal frame bridge for browser previews.
//!
//! Ratatui [`Buffer`] cells encode to JSON for a Ghostty-class web host;
//! browser key chords decode back into TermRock [`KeyEvent`]s so stories
//! paint through the same interactor path as the TUI lookbook.

use ratatui::{
    Terminal,
    backend::TestBackend,
    buffer::Buffer,
    layout::Rect,
    style::{Color, Modifier, Style},
    widgets::{Block, Clear},
};
use serde::{Deserialize, Serialize};
use termrock::{
    input::{KeyCode, KeyEvent, KeyEventKind, KeyModifiers},
    style::{DesignSystem, PREVIEW_CARD},
};

use crate::{
    demo::theme_id_for,
    stories::{Story, stories},
};

/// Uniform charcoal padding around story paint (matches SVG export).
pub const STORY_PAD: u16 = 1;

/// One terminal cell with true-color RGB (Ghostty-class 24-bit).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct FrameCell {
    /// Grapheme cluster / symbol drawn in the cell.
    pub ch: String,
    /// Foreground RGB.
    pub fg: [u8; 3],
    /// Background RGB.
    pub bg: [u8; 3],
    /// Bold modifier.
    #[serde(default)]
    pub bold: bool,
    /// Dim modifier.
    #[serde(default)]
    pub dim: bool,
    /// Underline modifier.
    #[serde(default)]
    pub underline: bool,
    /// Reverse video (already resolved into fg/bg when true; flag kept for hosts).
    #[serde(default)]
    pub reversed: bool,
}

/// Full terminal frame for a story paint.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct TerminalFrame {
    /// Story id (`list/selection`).
    pub story_id: String,
    /// Story title.
    pub title: String,
    /// Component name.
    pub component: String,
    /// Grid columns (including pad).
    pub cols: u16,
    /// Grid rows (including pad).
    pub rows: u16,
    /// Inner story width (no pad).
    pub story_cols: u16,
    /// Inner story height (no pad).
    pub story_rows: u16,
    /// Row-major cells, length `cols * rows`.
    pub cells: Vec<FrameCell>,
    /// Whether this story has a keyboard interactor.
    pub interactive: bool,
    /// Theme label.
    pub theme: String,
}

/// Browser → host key payload.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PreviewKey {
    /// `ArrowDown`, `Enter`, `a`, `Tab`, …
    pub key: String,
    /// `ctrl`, `alt`, `shift`, `meta` flags.
    #[serde(default)]
    pub ctrl: bool,
    /// Alt/Option modifier.
    #[serde(default)]
    pub alt: bool,
    /// Shift modifier.
    #[serde(default)]
    pub shift: bool,
    /// Meta/Super modifier.
    #[serde(default)]
    pub meta: bool,
}

/// Decode a browser key into a TermRock [`KeyEvent`].
#[must_use]
pub fn decode_preview_key(key: &PreviewKey) -> Option<KeyEvent> {
    let code = parse_key_code(&key.key)?;
    let mut mods = KeyModifiers::NONE;
    if key.ctrl {
        mods |= KeyModifiers::CONTROL;
    }
    if key.alt {
        mods |= KeyModifiers::ALT;
    }
    if key.shift {
        mods |= KeyModifiers::SHIFT;
    }
    if key.meta {
        // Super / Meta maps to none-of-the-above for TermRock; keep CONTROL when both.
        // Prefer CONTROL if ctrl also set; otherwise no TermRock meta flag exists — use ALT as host hint.
        if !key.ctrl {
            mods |= KeyModifiers::ALT;
        }
    }
    Some(KeyEvent {
        code,
        modifiers: mods,
        kind: KeyEventKind::Press,
        state: Default::default(),
    })
}

fn parse_key_code(key: &str) -> Option<KeyCode> {
    match key {
        "ArrowUp" | "Up" => Some(KeyCode::Up),
        "ArrowDown" | "Down" => Some(KeyCode::Down),
        "ArrowLeft" | "Left" => Some(KeyCode::Left),
        "ArrowRight" | "Right" => Some(KeyCode::Right),
        "Enter" => Some(KeyCode::Enter),
        "Escape" | "Esc" => Some(KeyCode::Esc),
        "Tab" => Some(KeyCode::Tab),
        "Backspace" => Some(KeyCode::Backspace),
        "Delete" => Some(KeyCode::Delete),
        "Home" => Some(KeyCode::Home),
        "End" => Some(KeyCode::End),
        "PageUp" => Some(KeyCode::PageUp),
        "PageDown" => Some(KeyCode::PageDown),
        " " | "Space" => Some(KeyCode::Char(' ')),
        s if s.chars().count() == 1 => {
            let c = s.chars().next()?;
            Some(KeyCode::Char(c))
        }
        _ => None,
    }
}

/// Encode a Ratatui buffer into a flat cell grid (truecolor RGB).
#[must_use]
pub fn encode_buffer(buffer: &Buffer) -> (u16, u16, Vec<FrameCell>) {
    let area = buffer.area;
    let mut cells = Vec::with_capacity(usize::from(area.width) * usize::from(area.height));
    for y in 0..area.height {
        for x in 0..area.width {
            let cell = &buffer[(x, y)];
            let (fg, bg, bold, dim, underline, reversed) =
                resolve_cell_paint(cell.fg, cell.bg, cell.modifier);
            cells.push(FrameCell {
                ch: cell.symbol().to_string(),
                fg,
                bg,
                bold,
                dim,
                underline,
                reversed,
            });
        }
    }
    (area.width, area.height, cells)
}

fn resolve_cell_paint(
    fg: Color,
    bg: Color,
    modifier: Modifier,
) -> ([u8; 3], [u8; 3], bool, bool, bool, bool) {
    let mut fg_rgb = color_to_rgb(fg, true);
    let mut bg_rgb = color_to_rgb(bg, false);
    let reversed = modifier.contains(Modifier::REVERSED);
    if reversed {
        std::mem::swap(&mut fg_rgb, &mut bg_rgb);
    }
    if modifier.contains(Modifier::DIM) {
        fg_rgb = [
            (u16::from(fg_rgb[0]) * 6 / 10) as u8,
            (u16::from(fg_rgb[1]) * 6 / 10) as u8,
            (u16::from(fg_rgb[2]) * 6 / 10) as u8,
        ];
    }
    (
        fg_rgb,
        bg_rgb,
        modifier.contains(Modifier::BOLD),
        modifier.contains(Modifier::DIM),
        modifier.contains(Modifier::UNDERLINED),
        reversed,
    )
}

fn color_to_rgb(color: Color, is_fg: bool) -> [u8; 3] {
    match color {
        Color::Reset => {
            if is_fg {
                [0xff, 0xff, 0xff]
            } else {
                [0x00, 0x00, 0x00]
            }
        }
        Color::Black => [0x00, 0x00, 0x00],
        Color::Red => [0xff, 0x00, 0x00],
        Color::Green => [0x00, 0xff, 0x41],
        Color::Yellow => [0xff, 0xd8, 0x5e],
        Color::Blue => [0x00, 0x50, 0xb4],
        Color::Magenta => [0xff, 0x00, 0xff],
        Color::Cyan => [0x00, 0xff, 0xff],
        Color::Gray | Color::DarkGray => [0x80, 0x80, 0x80],
        Color::LightRed => [0xff, 0x5e, 0x7a],
        Color::LightGreen => [0x00, 0xff, 0x41],
        Color::LightYellow => [0xff, 0xd8, 0x5e],
        Color::LightBlue => [0x7a, 0xa2, 0xff],
        Color::LightMagenta => [0xff, 0x7a, 0xff],
        Color::LightCyan => [0x7a, 0xff, 0xff],
        Color::White => [0xff, 0xff, 0xff],
        Color::Rgb(r, g, b) => [r, g, b],
        Color::Indexed(index) => crate::palette256::xterm256_to_rgb(index),
    }
}

/// Look up a story by id.
#[must_use]
pub fn story_by_id(id: &str) -> Option<Story> {
    stories().into_iter().find(|s| s.id == id)
}

/// Paint a story once and return the unresolved [`Buffer`].
///
/// This is the full-fidelity seam: every modifier, including italic and
/// crossed-out, remains present here. [`encode_buffer`] and frame JSON below
/// this point are lossy.
#[must_use]
pub fn paint_story_buffer(
    story: Story,
    system: &DesignSystem,
    cols: Option<u16>,
    rows: Option<u16>,
) -> Buffer {
    let story_cols = cols.unwrap_or(story.width).max(1);
    let story_rows = rows.unwrap_or(story.height).max(1);
    let width = story_cols.saturating_add(STORY_PAD * 2);
    let height = story_rows.saturating_add(STORY_PAD * 2);
    let backend = TestBackend::new(width, height);
    let mut terminal = Terminal::new(backend).expect("test terminal");
    let mut interactor = story.mount();
    interactor.set_system(system.clone());
    terminal
        .draw(|frame| {
            let area = frame.area();
            frame.render_widget(
                Block::default().style(Style::default().bg(PREVIEW_CARD)),
                area,
            );
            let inner = Rect {
                x: STORY_PAD,
                y: STORY_PAD,
                width: story_cols,
                height: story_rows,
            };
            frame.render_widget(Clear, inner);
            // The story's ground is the palette's canvas (plans/011).
            frame
                .buffer_mut()
                .set_style(inner, system.style(termrock::style::Role::Canvas));
            interactor.render(frame, inner);
        })
        .expect("draw");
    terminal.backend().buffer().clone()
}

/// Paint a story once (static path) into a [`TerminalFrame`].
#[must_use]
pub fn paint_story_frame(
    story: Story,
    system: &DesignSystem,
    cols: Option<u16>,
    rows: Option<u16>,
) -> TerminalFrame {
    let story_cols = cols.unwrap_or(story.width).max(1);
    let story_rows = rows.unwrap_or(story.height).max(1);
    let buffer = paint_story_buffer(story, system, cols, rows);
    let (c, r, cells) = encode_buffer(&buffer);
    TerminalFrame {
        story_id: story.id.into(),
        title: story.title.into(),
        component: story.component().into(),
        cols: c,
        rows: r,
        story_cols,
        story_rows,
        cells,
        interactive: story.is_interactive(),
        theme: theme_id_for(system.palette()).into(),
    }
}

/// Paint after applying a sequence of keys through the real story interactor.
#[must_use]
pub fn paint_story_after_keys(
    story: Story,
    system: &DesignSystem,
    cols: Option<u16>,
    rows: Option<u16>,
    keys: &[PreviewKey],
) -> TerminalFrame {
    let story_cols = cols.unwrap_or(story.width).max(1);
    let story_rows = rows.unwrap_or(story.height).max(1);
    let width = story_cols.saturating_add(STORY_PAD * 2);
    let height = story_rows.saturating_add(STORY_PAD * 2);
    let mut interactor = story.mount();
    interactor.set_system(system.clone());
    for k in keys {
        if let Some(ev) = decode_preview_key(k) {
            let _ = interactor.handle_key(ev);
        }
    }
    let backend = TestBackend::new(width, height);
    let mut terminal = Terminal::new(backend).expect("test terminal");
    let inner = Rect {
        x: STORY_PAD,
        y: STORY_PAD,
        width: story_cols,
        height: story_rows,
    };
    terminal
        .draw(|frame| {
            let area = frame.area();
            frame.render_widget(
                Block::default().style(Style::default().bg(PREVIEW_CARD)),
                area,
            );
            frame.render_widget(Clear, inner);
            frame
                .buffer_mut()
                .set_style(inner, system.style(termrock::style::Role::Canvas));
            interactor.render(frame, inner);
        })
        .expect("draw");
    let buffer = terminal.backend().buffer().clone();
    let (c, r, cells) = encode_buffer(&buffer);
    TerminalFrame {
        story_id: story.id.into(),
        title: story.title.into(),
        component: story.component().into(),
        cols: c,
        rows: r,
        story_cols,
        story_rows,
        cells,
        interactive: story.is_interactive(),
        theme: theme_id_for(system.palette()).into(),
    }
}

/// Ghostty-class default cell width in CSS pixels (matches SVG export).
pub const CELL_WIDTH_PX: u16 = 9;
/// Ghostty-class default cell height in CSS pixels (matches SVG export).
pub const CELL_HEIGHT_PX: u16 = 18;

/// Story-size presets exported for responsive host remapping (story cols×rows, no pad).
pub const RESPONSIVE_STORY_SIZES: &[(u16, u16)] = &[(28, 6), (40, 8), (56, 12), (72, 16), (80, 24)];

/// Suggest **total** terminal cols (including pad) from a CSS pixel width.
#[must_use]
pub fn cols_for_css_width(css_px: u16, cell_w: u16) -> u16 {
    let cell = cell_w.max(1);
    (css_px / cell).max(8)
}

/// Suggest **total** terminal rows (including pad) from a CSS pixel height.
#[must_use]
pub fn rows_for_css_height(css_px: u16, cell_h: u16) -> u16 {
    let cell = cell_h.max(1);
    (css_px / cell).max(4)
}

/// Map host CSS size → story paint size (inner, pad stripped).
#[must_use]
pub fn story_size_for_css_host(css_w: u16, css_h: u16) -> (u16, u16) {
    let total_c = cols_for_css_width(css_w, CELL_WIDTH_PX);
    let total_r = rows_for_css_height(css_h, CELL_HEIGHT_PX);
    let story_c = total_c.saturating_sub(STORY_PAD * 2).max(8);
    let story_r = total_r.saturating_sub(STORY_PAD * 2).max(4);
    (story_c, story_r)
}

/// Pick nearest exported size key (`40x8`) for a desired story cols×rows.
#[must_use]
pub fn pick_size_key(want_cols: u16, want_rows: u16) -> String {
    let (bc, br) = RESPONSIVE_STORY_SIZES
        .iter()
        .copied()
        .min_by_key(|(c, r)| {
            let dc = i32::from(*c) - i32::from(want_cols);
            let dr = i32::from(*r) - i32::from(want_rows);
            // Prefer not undershooting width when close.
            dc.unsigned_abs() * 2 + dr.unsigned_abs()
        })
        .unwrap_or((40, 8));
    format!("{bc}x{br}")
}

#[cfg(test)]
mod tests {
    use termrock::style::RolePalette;

    use super::*;

    #[test]
    fn encode_buffer_truecolor_rgb_cells() {
        let backend = TestBackend::new(2, 1);
        let mut terminal = Terminal::new(backend).unwrap();
        terminal
            .draw(|frame| {
                let area = frame.area();
                let buf = frame.buffer_mut();
                buf[(area.x, area.y)].set_symbol("A").set_style(
                    Style::default()
                        .fg(Color::Rgb(0x00, 0xff, 0x41))
                        .bg(Color::Rgb(0x10, 0x10, 0x10)),
                );
                buf[(area.x + 1, area.y)]
                    .set_symbol("B")
                    .set_style(Style::default().fg(Color::Green));
            })
            .unwrap();
        let buf = terminal.backend().buffer().clone();
        let (cols, rows, cells) = encode_buffer(&buf);
        assert_eq!((cols, rows), (2, 1));
        assert_eq!(cells.len(), 2);
        assert_eq!(cells[0].ch, "A");
        assert_eq!(cells[0].fg, [0x00, 0xff, 0x41]);
        assert_eq!(cells[0].bg, [0x10, 0x10, 0x10]);
        assert_eq!(cells[1].ch, "B");
        assert_eq!(cells[1].fg, [0x00, 0xff, 0x41]); // phosphor green named
    }

    #[test]
    fn decode_preview_key_arrows_and_modifiers() {
        let down = decode_preview_key(&PreviewKey {
            key: "ArrowDown".into(),
            ctrl: false,
            alt: false,
            shift: false,
            meta: false,
        })
        .expect("down");
        assert_eq!(down.code, KeyCode::Down);
        let enter = decode_preview_key(&PreviewKey {
            key: "Enter".into(),
            ctrl: false,
            alt: false,
            shift: false,
            meta: false,
        })
        .expect("enter");
        assert_eq!(enter.code, KeyCode::Enter);
        let ctrl_c = decode_preview_key(&PreviewKey {
            key: "c".into(),
            ctrl: true,
            alt: false,
            shift: false,
            meta: false,
        })
        .expect("ctrl-c");
        assert_eq!(ctrl_c.code, KeyCode::Char('c'));
        assert!(ctrl_c.modifiers.contains(KeyModifiers::CONTROL));
    }

    #[test]
    fn paint_story_frame_nonempty_for_list_selection() {
        let story = story_by_id("list/selection").expect("list/selection story");
        let system = crate::design::lookbook_system(RolePalette::default());
        let frame = paint_story_frame(story, &system, None, None);
        assert_eq!(frame.story_id, "list/selection");
        assert!(frame.cols >= story.width);
        assert!(!frame.cells.is_empty());
        // Substantial fill: most cells not empty after paint
        let nonempty = frame
            .cells
            .iter()
            .filter(|c| !c.ch.trim().is_empty())
            .count();
        assert!(
            nonempty > 10,
            "expected substantial terminal content, nonempty={nonempty}"
        );
    }

    #[test]
    fn story_specs_exclude_fixture_mount_divergence() {
        use crate::stories::StorySpec;

        let passive = story_by_id("center/both").expect("passive story");
        let mounted = story_by_id("list/selection").expect("mounted story");
        assert!(matches!(passive.spec(), StorySpec::Fixture(_)));
        assert!(matches!(mounted.spec(), StorySpec::Mounted(_)));
    }

    #[test]
    fn full_non_default_design_system_reaches_shared_frame_path() {
        use termrock::style::GlyphSet;

        let story = story_by_id("list/selection").expect("mounted list story");
        let unicode = crate::design::lookbook_system(RolePalette::default());
        let ascii = unicode.clone().glyphs(GlyphSet::Ascii);
        let unicode_frame = paint_story_buffer(story, &unicode, None, None);
        let ascii_frame = paint_story_buffer(story, &ascii, None, None);
        assert_ne!(unicode_frame, ascii_frame);
        assert!(
            unicode_frame
                .content()
                .iter()
                .any(|cell| cell.symbol() == unicode.glyphs.selection_gutter())
        );
        assert!(
            ascii_frame
                .content()
                .iter()
                .any(|cell| cell.symbol() == ascii.glyphs.selection_gutter())
        );
    }

    #[test]
    fn paint_after_keys_changes_list_selection_frame() {
        let story = story_by_id("list/selection").expect("list/selection");
        let system = crate::design::lookbook_system(RolePalette::default());
        let base = paint_story_after_keys(story, &system, None, None, &[]);
        let after = paint_story_after_keys(
            story,
            &system,
            None,
            None,
            &[PreviewKey {
                key: "ArrowDown".into(),
                ctrl: false,
                alt: false,
                shift: false,
                meta: false,
            }],
        );
        assert_ne!(
            base.cells, after.cells,
            "Down on list/selection must change the painted frame"
        );
    }

    #[test]
    fn keyed_frames_match_sessions_for_mounted_and_passive_nondefault_systems() {
        use crate::demo::DemoSession;
        use termrock::input::Event;
        use termrock::style::SelectionChrome;

        let system =
            crate::design::lookbook_system(RolePalette::slate()).selection(SelectionChrome::Marker);
        let key = PreviewKey {
            key: "ArrowDown".into(),
            ctrl: false,
            alt: false,
            shift: false,
            meta: false,
        };
        for (story_id, interactive) in [("list/selection", true), ("center/both", false)] {
            let story = story_by_id(story_id).expect("registered story");
            let direct =
                paint_story_after_keys(story, &system, None, None, std::slice::from_ref(&key));
            let mut session = DemoSession::mount(story.id, None, None).expect("mounted session");
            session.set_system(system.clone());
            let event = decode_preview_key(&key).expect("preview key");
            let _ = session.dispatch_event(Event::Key(event));
            assert_eq!(direct, session.frame());
            assert_eq!(direct.interactive, interactive);
            assert_eq!(direct.theme, "slate");
        }
    }

    #[test]
    fn responsive_col_row_helpers() {
        assert_eq!(cols_for_css_width(360, CELL_WIDTH_PX), 40);
        assert_eq!(rows_for_css_height(180, CELL_HEIGHT_PX), 10);
        assert!(cols_for_css_width(10, CELL_WIDTH_PX) >= 8);
        let (sc, sr) = story_size_for_css_host(360, 180);
        // 360/9=40 total cols → 38 story; 180/18=10 total → 8 story
        assert_eq!((sc, sr), (38, 8));
        assert_eq!(pick_size_key(38, 8), "40x8");
        assert_eq!(pick_size_key(10, 4), "28x6");
        assert_eq!(pick_size_key(90, 30), "80x24");
    }

    #[test]
    fn paint_at_two_host_sizes_remaps_grid() {
        let story = story_by_id("list/selection").expect("list/selection");
        let system = crate::design::lookbook_system(RolePalette::default());
        let (c1, r1) = story_size_for_css_host(280, 120);
        let (c2, r2) = story_size_for_css_host(720, 320);
        assert!(
            c2 > c1 || r2 > r1,
            "wider host must request larger story size"
        );
        let f1 = paint_story_frame(story, &system, Some(c1), Some(r1));
        let f2 = paint_story_frame(story, &system, Some(c2), Some(r2));
        assert_eq!(f1.story_cols, c1);
        assert_eq!(f1.story_rows, r1);
        assert_eq!(f2.story_cols, c2);
        assert_eq!(f2.story_rows, r2);
        assert_ne!(f1.cols, f2.cols, "total cols must remap with host");
        assert_ne!(
            f1.cells.len(),
            f2.cells.len(),
            "cell grid length must track remapped size"
        );
        let k1 = pick_size_key(c1, r1);
        let k2 = pick_size_key(c2, r2);
        assert_ne!(k1, k2, "distinct hosts map to distinct size packs");
    }

    #[test]
    fn agent_workbench_frame_paints() {
        let story = story_by_id("agent-workbench/basic").expect("agent-workbench/basic");
        let system = crate::design::lookbook_system(RolePalette::default());
        let frame = paint_story_frame(story, &system, Some(80), Some(24));
        assert_eq!(frame.component, "AgentWorkbench");
        let nonempty = frame
            .cells
            .iter()
            .filter(|c| !c.ch.trim().is_empty())
            .count();
        assert!(
            nonempty > 40,
            "composite must fill terminal, nonempty={nonempty}"
        );
    }

    #[test]
    fn poster_frame_uses_the_same_mounted_demo_as_native_and_web_hosts() {
        let story = story_by_id("connection-manager/full").expect("pattern story");
        let system = crate::design::lookbook_system(RolePalette::default());
        let poster = paint_story_frame(story, &system, None, None);
        let mut session = crate::demo::DemoSession::mount(story.id, None, None).unwrap();
        session.set_system(system);
        assert_eq!(poster, session.frame());
    }
}
