// SPDX-FileCopyrightText: 2026 Alexey Zhokhov
// SPDX-License-Identifier: Apache-2.0

//! Canonical cell snapshot: one grid, then txt / ansi / cursor / html / png.

use ratatui::buffer::Buffer;
use ratatui::layout::Position;
use ratatui::style::{Color, Modifier};

/// One terminal cell.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Cell {
    pub glyph: String,
    pub fg: Color,
    pub bg: Color,
    pub modifier: Modifier,
}

/// Canonical snapshot for capture and comparison.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Snapshot {
    pub cols: u16,
    pub rows: u16,
    pub cells: Vec<Cell>,
    pub cursor: Option<Position>,
    pub cursor_visible: bool,
}

impl Snapshot {
    /// Capture a ratatui buffer plus cursor.
    #[must_use]
    pub fn from_buffer(buf: &Buffer, cursor: Option<Position>, cursor_visible: bool) -> Self {
        let area = buf.area();
        let mut cells = Vec::with_capacity(usize::from(area.width) * usize::from(area.height));
        for y in area.y..area.bottom() {
            for x in area.x..area.right() {
                let c = &buf[(x, y)];
                cells.push(Cell {
                    glyph: c.symbol().to_owned(),
                    fg: c.fg,
                    bg: c.bg,
                    modifier: c.modifier,
                });
            }
        }
        Self {
            cols: area.width,
            rows: area.height,
            cells,
            cursor,
            cursor_visible,
        }
    }

    /// Plain-text grid, trailing cells preserved (spaces significant).
    #[must_use]
    pub fn to_txt(&self) -> String {
        let mut out = String::new();
        for y in 0..self.rows {
            for x in 0..self.cols {
                let i = usize::from(y) * usize::from(self.cols) + usize::from(x);
                match self.cells.get(i) {
                    Some(c) if !c.glyph.is_empty() => out.push_str(&c.glyph),
                    _ => out.push(' '),
                }
            }
            out.push('\n');
        }
        out
    }

    /// Source `.cursor` format: `x y flag` (`flag` 1 = visible).
    #[must_use]
    pub fn to_cursor(&self) -> String {
        let (x, y) = self.cursor.map(|p| (p.x, p.y)).unwrap_or((0, 0));
        let flag = u8::from(self.cursor_visible);
        format!("{x} {y} {flag}\n")
    }

    /// Truecolor SGR grid (tmux `capture-pane -e` shape).
    #[must_use]
    pub fn to_ansi(&self) -> String {
        use termrock::style::color_to_rgb;
        let mut out = String::new();
        let mut prev: Option<([u8; 3], [u8; 3], bool, bool, bool, bool, bool)> = None;
        for y in 0..self.rows {
            for x in 0..self.cols {
                let i = usize::from(y) * usize::from(self.cols) + usize::from(x);
                let Some(c) = self.cells.get(i) else {
                    out.push(' ');
                    continue;
                };
                let fg = color_to_rgb(c.fg, true);
                let bg = color_to_rgb(c.bg, false);
                let bold = c.modifier.contains(Modifier::BOLD);
                let dim = c.modifier.contains(Modifier::DIM);
                let ul = c.modifier.contains(Modifier::UNDERLINED);
                let italic = c.modifier.contains(Modifier::ITALIC);
                let strike = c.modifier.contains(Modifier::CROSSED_OUT);
                let style = (fg, bg, bold, dim, ul, italic, strike);
                if prev != Some(style) {
                    out.push_str("\u{1b}[0m");
                    out.push_str(&format!("\u{1b}[38;2;{};{};{}m", fg[0], fg[1], fg[2]));
                    out.push_str(&format!("\u{1b}[48;2;{};{};{}m", bg[0], bg[1], bg[2]));
                    if bold {
                        out.push_str("\u{1b}[1m");
                    }
                    if dim {
                        out.push_str("\u{1b}[2m");
                    }
                    if italic {
                        out.push_str("\u{1b}[3m");
                    }
                    if ul {
                        out.push_str("\u{1b}[4m");
                    }
                    if strike {
                        out.push_str("\u{1b}[9m");
                    }
                    prev = Some(style);
                }
                if c.glyph.is_empty() {
                    out.push(' ');
                } else {
                    out.push_str(&c.glyph);
                }
            }
            out.push_str("\u{1b}[0m\n");
            prev = None;
        }
        out
    }

    /// Standalone HTML preview of the same grid.
    #[must_use]
    pub fn to_html(&self) -> String {
        use termrock::style::color_to_rgb;
        let mut body = String::new();
        for y in 0..self.rows {
            for x in 0..self.cols {
                let i = usize::from(y) * usize::from(self.cols) + usize::from(x);
                let Some(c) = self.cells.get(i) else {
                    body.push(' ');
                    continue;
                };
                let fg = color_to_rgb(c.fg, true);
                let bg = color_to_rgb(c.bg, false);
                let glyph = if c.glyph.is_empty() {
                    " "
                } else {
                    c.glyph.as_str()
                };
                let escaped = glyph
                    .replace('&', "&amp;")
                    .replace('<', "&lt;")
                    .replace('>', "&gt;");
                let mut css = format!(
                    "color:#{:02x}{:02x}{:02x};background:#{:02x}{:02x}{:02x}",
                    fg[0], fg[1], fg[2], bg[0], bg[1], bg[2]
                );
                if c.modifier.contains(Modifier::BOLD) {
                    css.push_str(";font-weight:700");
                }
                if c.modifier.contains(Modifier::ITALIC) {
                    css.push_str(";font-style:italic");
                }
                if c.modifier.contains(Modifier::DIM) {
                    css.push_str(";opacity:0.6");
                }
                if c.modifier.contains(Modifier::UNDERLINED) {
                    css.push_str(";text-decoration:underline");
                }
                if c.modifier.contains(Modifier::CROSSED_OUT) {
                    css.push_str(";text-decoration:line-through");
                }
                body.push_str(&format!("<span style=\"{css}\">{escaped}</span>"));
            }
            body.push('\n');
        }
        format!(
            "<!doctype html><meta charset=utf-8><title>termrock-catalog</title>\
             <body style=\"background:#000;color:#fff;font:14px/18px ui-monospace,monospace\">\
             <pre style=\"margin:0\">{body}</pre></body>\n"
        )
    }

    /// Trailing-space-stripped text, matching tmux `capture-pane -p`.
    #[must_use]
    pub fn to_txt_trimmed(&self) -> String {
        self.to_txt()
            .lines()
            .map(str::trim_end)
            .collect::<Vec<_>>()
            .join("\n")
            + "\n"
    }
}
