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
    style::{DesignSystem, PREVIEW_CARD, RolePalette},
};

use crate::stories::{Story, stories};

/// Uniform charcoal padding around story paint (matches SVG export).
const STORY_PAD: u16 = 1;

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
    #[serde(default)]
    pub alt: bool,
    #[serde(default)]
    pub shift: bool,
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
        Color::Indexed(_) => {
            if is_fg {
                [0xff, 0xff, 0xff]
            } else {
                [0x00, 0x00, 0x00]
            }
        }
    }
}

/// Look up a story by id.
#[must_use]
pub fn story_by_id(id: &str) -> Option<Story> {
    stories().into_iter().find(|s| s.id == id)
}

/// Paint a story once (static path) into a [`TerminalFrame`].
#[must_use]
pub fn paint_story_frame(
    story: Story,
    theme: &RolePalette,
    cols: Option<u16>,
    rows: Option<u16>,
) -> TerminalFrame {
    let story_cols = cols.unwrap_or(story.width).max(1);
    let story_rows = rows.unwrap_or(story.height).max(1);
    let width = story_cols.saturating_add(STORY_PAD * 2);
    let height = story_rows.saturating_add(STORY_PAD * 2);
    let backend = TestBackend::new(width, height);
    let mut terminal = Terminal::new(backend).expect("test terminal");
    let system = DesignSystem::from_palette(theme.clone());
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
            story.render(frame, inner, &system);
        })
        .expect("draw");
    let buffer = terminal.backend().buffer().clone();
    let (c, r, cells) = encode_buffer(&buffer);
    TerminalFrame {
        story_id: story.id.into(),
        title: story.title.into(),
        component: story.component.into(),
        cols: c,
        rows: r,
        story_cols,
        story_rows,
        cells,
        interactive: probe_interactive(story),
        theme: "phosphor".into(),
    }
}

fn probe_interactive(story: Story) -> bool {
    let mut inter = story.make_interactor();
    inter.handle_key(KeyEvent {
        code: KeyCode::Down,
        modifiers: KeyModifiers::NONE,
        kind: KeyEventKind::Press,
        state: Default::default(),
    })
}

/// Paint after applying a sequence of keys through the real story interactor.
#[must_use]
pub fn paint_story_after_keys(
    story: Story,
    theme: &RolePalette,
    cols: Option<u16>,
    rows: Option<u16>,
    keys: &[PreviewKey],
) -> TerminalFrame {
    let story_cols = cols.unwrap_or(story.width).max(1);
    let story_rows = rows.unwrap_or(story.height).max(1);
    let width = story_cols.saturating_add(STORY_PAD * 2);
    let height = story_rows.saturating_add(STORY_PAD * 2);
    let mut interactor = story.make_interactor();
    interactor.set_theme(theme.clone());
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
            interactor.render(frame, inner);
        })
        .expect("draw");
    let buffer = terminal.backend().buffer().clone();
    let (c, r, cells) = encode_buffer(&buffer);
    TerminalFrame {
        story_id: story.id.into(),
        title: story.title.into(),
        component: story.component.into(),
        cols: c,
        rows: r,
        story_cols,
        story_rows,
        cells,
        interactive: true,
        theme: "phosphor".into(),
    }
}

/// Suggest terminal cols from a CSS pixel width (Ghostty-class ~9px cell).
#[must_use]
#[allow(dead_code)] // public bridge API for hosts / future live resize export
pub fn cols_for_css_width(css_px: u16, cell_w: u16) -> u16 {
    let cell = cell_w.max(1);
    (css_px / cell).max(8)
}

/// Suggest terminal rows from a CSS pixel height (Ghostty-class ~18px cell).
#[must_use]
#[allow(dead_code)] // public bridge API for hosts / future live resize export
pub fn rows_for_css_height(css_px: u16, cell_h: u16) -> u16 {
    let cell = cell_h.max(1);
    (css_px / cell).max(4)
}

#[cfg(test)]
mod tests {
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
        let frame = paint_story_frame(story, &RolePalette::default(), None, None);
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
    fn paint_after_keys_changes_list_selection_frame() {
        let story = story_by_id("list/selection").expect("list/selection");
        let theme = RolePalette::default();
        let base = paint_story_after_keys(story, &theme, None, None, &[]);
        let after = paint_story_after_keys(
            story,
            &theme,
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
    fn responsive_col_row_helpers() {
        assert_eq!(cols_for_css_width(360, 9), 40);
        assert_eq!(rows_for_css_height(180, 18), 10);
        assert!(cols_for_css_width(10, 9) >= 8);
    }

    #[test]
    fn agent_workbench_frame_paints() {
        let story = story_by_id("agent-workbench/basic").expect("agent-workbench/basic");
        let frame = paint_story_frame(story, &RolePalette::default(), Some(80), Some(24));
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
}
