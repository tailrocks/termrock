// SPDX-FileCopyrightText: 2026 Alexey Zhokhov
// SPDX-License-Identifier: Apache-2.0

//! Canonical cell snapshot: one grid, then txt / ansi / cursor / html / png.

use ratatui::buffer::Buffer;
use ratatui::layout::{Position, Rect};
use ratatui::style::{Color, Modifier, Style};

fn export_rgb(color: Color, is_fg: bool) -> [u8; 3] {
    match color {
        // tmux starts a new line with the terminal default text color when no
        // SGR is present. Keep Reset distinct from explicit White.
        Color::Reset if is_fg => [0xd0, 0xd0, 0xd0],
        other => termrock::style::color_to_rgb(other, is_fg),
    }
}

fn append_modifiers(
    out: &mut String,
    bold: bool,
    dim: bool,
    italic: bool,
    underline: bool,
    strike: bool,
    reverse: bool,
) {
    if bold {
        out.push_str("\u{1b}[1m");
    }
    if dim {
        out.push_str("\u{1b}[2m");
    }
    if italic {
        out.push_str("\u{1b}[3m");
    }
    if underline {
        out.push_str("\u{1b}[4m");
    }
    if strike {
        out.push_str("\u{1b}[9m");
    }
    if reverse {
        out.push_str("\u{1b}[7m");
    }
}

fn append_new_modifiers(
    out: &mut String,
    old_bold: bool,
    old_dim: bool,
    old_italic: bool,
    old_underline: bool,
    old_strike: bool,
    old_reverse: bool,
    bold: bool,
    dim: bool,
    italic: bool,
    underline: bool,
    strike: bool,
    reverse: bool,
) {
    if bold && !old_bold {
        out.push_str("\u{1b}[1m");
    }
    if dim && !old_dim {
        out.push_str("\u{1b}[2m");
    }
    if italic && !old_italic {
        out.push_str("\u{1b}[3m");
    }
    if underline && !old_underline {
        out.push_str("\u{1b}[4m");
    }
    if strike && !old_strike {
        out.push_str("\u{1b}[9m");
    }
    if reverse && !old_reverse {
        out.push_str("\u{1b}[7m");
    }
}

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

    /// Materialize explicit RGB styles for deterministic raster output.
    ///
    /// A live ratatui buffer may retain `Color::Reset`, whose terminal meaning
    /// is encoder-dependent. The source ANSI grid uses Junie's default text
    /// color (`#d0d0d0`) and black background; resolving here makes target PNG
    /// output use the same semantic cells as target ANSI/HTML artifacts.
    #[must_use]
    pub fn to_raster_buffer(&self) -> Buffer {
        let mut buffer = Buffer::empty(Rect::new(0, 0, self.cols, self.rows));
        for y in 0..self.rows {
            for x in 0..self.cols {
                let index = usize::from(y) * usize::from(self.cols) + usize::from(x);
                let Some(cell) = self.cells.get(index) else {
                    continue;
                };
                let fg = export_rgb(cell.fg, true);
                let bg = export_rgb(cell.bg, false);
                let style = Style::default()
                    .fg(Color::Rgb(fg[0], fg[1], fg[2]))
                    .bg(Color::Rgb(bg[0], bg[1], bg[2]))
                    .add_modifier(cell.modifier);
                buffer[(x, y)].set_symbol(&cell.glyph).set_style(style);
            }
        }
        buffer
    }

    /// Plain-text capture using tmux `capture-pane -p` line semantics.
    ///
    /// The source capture strips cells after the last painted cell on each
    /// line. Internal spaces remain significant; only the unpainted suffix is
    /// removed.
    #[must_use]
    pub fn to_txt(&self) -> String {
        self.to_txt_with_padding(false)
    }

    /// Full cell-grid text, including trailing cells preserved by a
    /// `TestBackend` frame. This is used for fixed-size headless fixtures;
    /// terminal `capture-pane -p` output uses [`Self::to_txt`] instead.
    #[must_use]
    pub fn to_txt_padded(&self) -> String {
        self.to_txt_with_padding(true)
    }

    fn to_txt_with_padding(&self, preserve_trailing: bool) -> String {
        let mut out = String::new();
        for y in 0..self.rows {
            let mut line = String::new();
            for x in 0..self.cols {
                let i = usize::from(y) * usize::from(self.cols) + usize::from(x);
                match self.cells.get(i) {
                    Some(c) if !c.glyph.is_empty() => line.push_str(&c.glyph),
                    _ => line.push(' '),
                }
            }
            if preserve_trailing {
                out.push_str(&line);
            } else {
                out.push_str(line.trim_end());
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
        let mut out = String::new();
        let mut prev: Option<([u8; 3], [u8; 3], bool, bool, bool, bool, bool, bool)> = None;
        for y in 0..self.rows {
            for x in 0..self.cols {
                let i = usize::from(y) * usize::from(self.cols) + usize::from(x);
                let Some(c) = self.cells.get(i) else {
                    out.push(' ');
                    continue;
                };
                let fg = export_rgb(c.fg, true);
                let bg = export_rgb(c.bg, false);
                let bold = c.modifier.contains(Modifier::BOLD);
                let dim = c.modifier.contains(Modifier::DIM);
                let ul = c.modifier.contains(Modifier::UNDERLINED);
                let italic = c.modifier.contains(Modifier::ITALIC);
                let strike = c.modifier.contains(Modifier::CROSSED_OUT);
                let reverse = c.modifier.contains(Modifier::REVERSED);
                let style = (fg, bg, bold, dim, ul, italic, strike, reverse);
                let modifiers_disabled = prev.is_some_and(|old| {
                    (old.2 && !bold)
                        || (old.3 && !dim)
                        || (old.4 && !ul)
                        || (old.5 && !italic)
                        || (old.6 && !strike)
                        || (old.7 && !reverse)
                });
                if modifiers_disabled {
                    // tmux resets the complete SGR state before reapplying
                    // colors when a decoration changes. Color-only changes
                    // below remain deltas, including across newlines.
                    out.push_str("\u{1b}[0m");
                    out.push_str(&format!("\u{1b}[38;2;{};{};{}m", fg[0], fg[1], fg[2]));
                    out.push_str(&format!("\u{1b}[48;2;{};{};{}m", bg[0], bg[1], bg[2]));
                    append_modifiers(&mut out, bold, dim, italic, ul, strike, reverse);
                } else if let Some(old) = prev {
                    append_new_modifiers(
                        &mut out, old.2, old.3, old.5, old.4, old.6, old.7, bold, dim, italic, ul,
                        strike, reverse,
                    );
                    if old.0 != fg {
                        out.push_str(&format!("\u{1b}[38;2;{};{};{}m", fg[0], fg[1], fg[2]));
                    }
                    if old.1 != bg {
                        out.push_str(&format!("\u{1b}[48;2;{};{};{}m", bg[0], bg[1], bg[2]));
                    }
                } else {
                    out.push_str(&format!("\u{1b}[38;2;{};{};{}m", fg[0], fg[1], fg[2]));
                    out.push_str(&format!("\u{1b}[48;2;{};{};{}m", bg[0], bg[1], bg[2]));
                    append_modifiers(&mut out, bold, dim, italic, ul, strike, reverse);
                }
                prev = Some(style);
                if c.glyph.is_empty() {
                    out.push(' ');
                } else {
                    out.push_str(&c.glyph);
                }
            }
            out.push('\n');
        }
        out
    }

    /// Standalone HTML preview of the same grid.
    #[must_use]
    pub fn to_html(&self) -> String {
        let mut lines = Vec::with_capacity(usize::from(self.rows));
        for y in 0..self.rows {
            let mut line = String::new();
            let mut run_style: Option<String> = None;
            let mut run_text = String::new();
            for x in 0..self.cols {
                let i = usize::from(y) * usize::from(self.cols) + usize::from(x);
                let Some(c) = self.cells.get(i) else {
                    continue;
                };
                let style = html_style(c);
                let glyph = html_escape(if c.glyph.is_empty() { " " } else { &c.glyph });
                if run_style.as_deref() != Some(style.as_str()) {
                    if let Some(old_style) = run_style.take() {
                        line.push_str(&format!("<span style=\"{old_style}\">{run_text}</span>"));
                        run_text.clear();
                    }
                    run_style = Some(style);
                }
                run_text.push_str(&glyph);
            }
            if let Some(style) = run_style {
                line.push_str(&format!("<span style=\"{style}\">{run_text}</span>"));
            }
            lines.push(line);
        }
        format!(
            "<!doctype html><html><head><meta charset=\"utf-8\"><title>capture</title>\n\
<style>\n\
html,body{{margin:0;background:#1a1a1a}}\n\
pre{{margin:16px;display:inline-block;font-family:\"JetBrainsMono Nerd Font Mono\",\"JetBrains Mono\",Menlo,monospace;font-size:14px;line-height:18px;white-space:pre;background:#000000}}\n\
span{{display:inline-block;height:18px;vertical-align:top}}\n\
</style></head><body><pre>{}</pre></body></html>",
            lines.join("\n")
        )
    }

    /// Trailing-space-stripped text, matching tmux `capture-pane -p`.
    #[must_use]
    pub fn to_txt_trimmed(&self) -> String {
        self.to_txt()
    }
}

fn html_escape(text: &str) -> String {
    text.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
        .replace('\'', "&#x27;")
}

fn html_style(cell: &Cell) -> String {
    let mut fg = export_rgb(cell.fg, true);
    let mut bg = export_rgb(cell.bg, false);
    if cell.modifier.contains(Modifier::REVERSED) {
        std::mem::swap(&mut fg, &mut bg);
    }
    let mut css = format!(
        "color:#{:02x}{:02x}{:02x};background:#{:02x}{:02x}{:02x}",
        fg[0], fg[1], fg[2], bg[0], bg[1], bg[2]
    );
    if cell.modifier.contains(Modifier::BOLD) {
        css.push_str(";font-weight:700");
    }
    if cell.modifier.contains(Modifier::DIM) {
        css.push_str(";opacity:.6");
    }
    if cell.modifier.contains(Modifier::ITALIC) {
        css.push_str(";font-style:italic");
    }
    let mut decoration = Vec::new();
    if cell.modifier.contains(Modifier::UNDERLINED) {
        decoration.push("underline");
    }
    if cell.modifier.contains(Modifier::CROSSED_OUT) {
        decoration.push("line-through");
    }
    if !decoration.is_empty() {
        css.push_str(";text-decoration:");
        css.push_str(&decoration.join(" "));
    }
    css
}

#[cfg(test)]
mod tests {
    use super::{Cell, Snapshot};
    use ratatui::layout::Position;
    use ratatui::style::{Color, Modifier};

    fn styled_snapshot(modifier: Modifier, cursor: Option<Position>) -> Snapshot {
        Snapshot {
            cols: 1,
            rows: 1,
            cells: vec![Cell {
                glyph: "A".to_owned(),
                fg: Color::Rgb(0x12, 0x34, 0x56),
                bg: Color::Rgb(0xa1, 0xb2, 0xc3),
                modifier,
            }],
            cursor,
            cursor_visible: cursor.is_some(),
        }
    }

    #[test]
    fn ansi_serializes_reverse_and_all_source_modifiers() {
        let modifier = Modifier::REVERSED
            | Modifier::BOLD
            | Modifier::DIM
            | Modifier::UNDERLINED
            | Modifier::ITALIC
            | Modifier::CROSSED_OUT;
        let ansi = styled_snapshot(modifier, None).to_ansi();

        assert!(ansi.contains("\u{1b}[38;2;18;52;86m"));
        assert!(ansi.contains("\u{1b}[48;2;161;178;195m"));
        for sgr in ["[1m", "[2m", "[3m", "[4m", "[9m", "[7m"] {
            assert!(ansi.contains(&format!("\u{1b}{sgr}")), "missing SGR {sgr}");
        }
        assert!(ansi.ends_with("A\n"));
    }

    #[test]
    fn html_resolves_reverse_and_keeps_both_decorations() {
        let modifier = Modifier::REVERSED
            | Modifier::BOLD
            | Modifier::DIM
            | Modifier::UNDERLINED
            | Modifier::ITALIC
            | Modifier::CROSSED_OUT;
        let html = styled_snapshot(modifier, None).to_html();

        assert!(html.contains("color:#a1b2c3;background:#123456"));
        assert!(html.contains("font-weight:700"));
        assert!(html.contains("font-style:italic"));
        assert!(html.contains("opacity:.6"));
        assert!(html.contains("text-decoration:underline line-through"));
    }

    #[test]
    fn ansi_and_html_dimensions_do_not_depend_on_cursor() {
        let without_cursor = styled_snapshot(Modifier::empty(), None);
        let with_cursor = styled_snapshot(Modifier::empty(), Some(Position::new(9, 7)));

        assert_eq!(without_cursor.to_ansi(), with_cursor.to_ansi());
        assert_eq!(without_cursor.to_html(), with_cursor.to_html());
    }

    #[test]
    fn non_reversed_ansi_output_remains_unchanged() {
        let snapshot = styled_snapshot(Modifier::BOLD, None);

        assert_eq!(
            snapshot.to_ansi(),
            "\u{1b}[38;2;18;52;86m\u{1b}[48;2;161;178;195m\u{1b}[1mA\n"
        );
    }
}
