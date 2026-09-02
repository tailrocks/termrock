// SPDX-FileCopyrightText: 2026 Alexey Zhokhov
// SPDX-License-Identifier: Apache-2.0

//! Parse tmux `capture-pane -e` SGR streams into a cell grid.

use ratatui::buffer::Buffer;
use ratatui::layout::Rect;
use ratatui::style::{Color, Modifier, Style};

/// One resolved cell (tmux SGR or a Snapshot cell).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GridCell {
    pub ch: String,
    pub fg: [u8; 3],
    pub bg: [u8; 3],
    pub bold: bool,
    pub dim: bool,
    pub italic: bool,
    pub underline: bool,
    pub reverse: bool,
    pub strike: bool,
}

/// Row-major grid.
#[derive(Debug, Clone)]
pub struct Grid {
    pub cols: u16,
    pub rows: u16,
    pub cells: Vec<GridCell>,
}

impl Grid {
    #[must_use]
    pub fn at(&self, x: u16, y: u16) -> Option<&GridCell> {
        let i = usize::from(y) * usize::from(self.cols) + usize::from(x);
        self.cells.get(i)
    }

    /// ansi2html.py paints Reset as `#d0d0d0`; TestBackend Reset is `#ffffff`.
    /// Same canvas cell, different encoder default — not an app color.
    fn default_fg(c: [u8; 3]) -> bool {
        c == [0xd0, 0xd0, 0xd0] || c == [0xff, 0xff, 0xff]
    }

    fn cell_eq(a: &GridCell, b: &GridCell) -> bool {
        if a.ch != b.ch
            || a.bg != b.bg
            || a.bold != b.bold
            || a.dim != b.dim
            || a.italic != b.italic
            || a.underline != b.underline
            || a.reverse != b.reverse
            || a.strike != b.strike
        {
            return false;
        }
        if a.fg == b.fg {
            return true;
        }
        a.bg == [0, 0, 0] && Self::default_fg(a.fg) && Self::default_fg(b.fg)
    }

    /// First differing cell vs `other` (same geometry required).
    #[must_use]
    pub fn first_diff(&self, other: &Self) -> Option<(u16, u16, String)> {
        if self.cols != other.cols || self.rows != other.rows {
            return Some((
                0,
                0,
                format!(
                    "size {}x{} vs {}x{}",
                    self.cols, self.rows, other.cols, other.rows
                ),
            ));
        }
        for y in 0..self.rows {
            for x in 0..self.cols {
                let a = self.at(x, y).unwrap();
                let b = other.at(x, y).unwrap();
                if !Self::cell_eq(a, b) {
                    return Some((
                        x,
                        y,
                        format!(
                            "expected ch={:?} fg={:?} bg={:?} bold={} dim={} italic={} ul={} rev={} strike={} got ch={:?} fg={:?} bg={:?} bold={} dim={} italic={} ul={} rev={} strike={}",
                            a.ch,
                            a.fg,
                            a.bg,
                            a.bold,
                            a.dim,
                            a.italic,
                            a.underline,
                            a.reverse,
                            a.strike,
                            b.ch,
                            b.fg,
                            b.bg,
                            b.bold,
                            b.dim,
                            b.italic,
                            b.underline,
                            b.reverse,
                            b.strike
                        ),
                    ));
                }
            }
        }
        None
    }

    /// Paint into a ratatui buffer (for PNG raster of a source .ansi).
    #[must_use]
    pub fn to_buffer(&self) -> Buffer {
        let mut buf = Buffer::empty(Rect::new(0, 0, self.cols, self.rows));
        for y in 0..self.rows {
            for x in 0..self.cols {
                let Some(c) = self.at(x, y) else {
                    continue;
                };
                let mut modifier = Modifier::empty();
                if c.bold {
                    modifier |= Modifier::BOLD;
                }
                if c.dim {
                    modifier |= Modifier::DIM;
                }
                if c.italic {
                    modifier |= Modifier::ITALIC;
                }
                if c.underline {
                    modifier |= Modifier::UNDERLINED;
                }
                if c.reverse {
                    modifier |= Modifier::REVERSED;
                }
                if c.strike {
                    modifier |= Modifier::CROSSED_OUT;
                }
                let style = Style::new()
                    .fg(Color::Rgb(c.fg[0], c.fg[1], c.fg[2]))
                    .bg(Color::Rgb(c.bg[0], c.bg[1], c.bg[2]))
                    .add_modifier(modifier);
                buf[(x, y)].set_symbol(&c.ch).set_style(style);
            }
        }
        buf
    }
}

const DEFAULT_FG: [u8; 3] = [0xd0, 0xd0, 0xd0];
const DEFAULT_BG: [u8; 3] = [0x00, 0x00, 0x00];

struct Sgr {
    fg: [u8; 3],
    bg: [u8; 3],
    bold: bool,
    dim: bool,
    italic: bool,
    underline: bool,
    reverse: bool,
    strike: bool,
}

impl Sgr {
    fn new() -> Self {
        Self {
            fg: DEFAULT_FG,
            bg: DEFAULT_BG,
            bold: false,
            dim: false,
            italic: false,
            underline: false,
            reverse: false,
            strike: false,
        }
    }

    fn apply(&mut self, params: &str) {
        if params.is_empty() {
            *self = Self::new();
            return;
        }
        let nums: Vec<u16> = params.split(';').map(|p| p.parse().unwrap_or(0)).collect();
        let mut i = 0;
        while i < nums.len() {
            match nums[i] {
                0 => *self = Self::new(),
                1 => self.bold = true,
                2 => self.dim = true,
                3 => self.italic = true,
                4 => self.underline = true,
                7 => self.reverse = true,
                9 => self.strike = true,
                22 => {
                    self.bold = false;
                    self.dim = false;
                }
                23 => self.italic = false,
                24 => self.underline = false,
                27 => self.reverse = false,
                29 => self.strike = false,
                39 => self.fg = DEFAULT_FG,
                49 => self.bg = DEFAULT_BG,
                38 if i + 4 < nums.len() && nums[i + 1] == 2 => {
                    self.fg = [nums[i + 2] as u8, nums[i + 3] as u8, nums[i + 4] as u8];
                    i += 4;
                }
                48 if i + 4 < nums.len() && nums[i + 1] == 2 => {
                    self.bg = [nums[i + 2] as u8, nums[i + 3] as u8, nums[i + 4] as u8];
                    i += 4;
                }
                _ => {}
            }
            i += 1;
        }
    }

    fn cell(&self, ch: String) -> GridCell {
        GridCell {
            ch,
            fg: self.fg,
            bg: self.bg,
            bold: self.bold,
            dim: self.dim,
            italic: self.italic,
            underline: self.underline,
            reverse: self.reverse,
            strike: self.strike,
        }
    }
}

/// Parse a tmux `-e` ANSI dump into a `cols × rows` grid (pad/truncate).
#[must_use]
pub fn parse_ansi(src: &str, cols: u16, rows: u16) -> Grid {
    let blank = Sgr::new().cell(" ".into());
    let mut cells = vec![blank.clone(); usize::from(cols) * usize::from(rows)];
    let mut sgr = Sgr::new();
    let mut x: u16 = 0;
    let mut y: u16 = 0;
    let chars: Vec<char> = src.chars().collect();
    let mut i = 0;
    while i < chars.len() {
        if chars[i] == '\u{1b}' && i + 1 < chars.len() && chars[i + 1] == '[' {
            i += 2;
            let start = i;
            while i < chars.len() && !chars[i].is_ascii_alphabetic() {
                i += 1;
            }
            if i >= chars.len() {
                break;
            }
            let cmd = chars[i];
            let params: String = chars[start..i].iter().collect();
            i += 1;
            if cmd == 'm' {
                sgr.apply(&params);
            }
            continue;
        }
        if chars[i] == '\n' {
            x = 0;
            y = y.saturating_add(1);
            i += 1;
            continue;
        }
        if chars[i] == '\r' {
            i += 1;
            continue;
        }
        if y < rows && x < cols {
            let idx = usize::from(y) * usize::from(cols) + usize::from(x);
            cells[idx] = sgr.cell(chars[i].to_string());
        }
        x = x.saturating_add(1);
        i += 1;
    }
    Grid { cols, rows, cells }
}

/// Snapshot cells → grid (for compare vs parsed source ANSI).
#[must_use]
pub fn from_snapshot(snap: &crate::snapshot::Snapshot) -> Grid {
    use termrock::style::color_to_rgb;
    let mut cells = Vec::with_capacity(snap.cells.len());
    for c in &snap.cells {
        let fg = color_to_rgb(c.fg, true);
        let bg = color_to_rgb(c.bg, false);
        cells.push(GridCell {
            ch: if c.glyph.is_empty() {
                " ".into()
            } else {
                c.glyph.clone()
            },
            fg,
            bg,
            bold: c.modifier.contains(Modifier::BOLD),
            dim: c.modifier.contains(Modifier::DIM),
            italic: c.modifier.contains(Modifier::ITALIC),
            underline: c.modifier.contains(Modifier::UNDERLINED),
            reverse: c.modifier.contains(Modifier::REVERSED),
            strike: c.modifier.contains(Modifier::CROSSED_OUT),
        });
    }
    Grid {
        cols: snap.cols,
        rows: snap.rows,
        cells,
    }
}

/// Parse source `ansi2html.py` output into a grid of the given size.
#[must_use]
pub fn parse_html(src: &str, cols: u16, rows: u16) -> Grid {
    let src = src
        .split_once("<pre")
        .and_then(|(_, rest)| rest.split_once('>').map(|(_, body)| body))
        .and_then(|body| body.split_once("</pre>").map(|(inner, _)| inner))
        .unwrap_or(src);
    let blank = Sgr::new().cell(" ".into());
    let mut cells = vec![blank.clone(); usize::from(cols) * usize::from(rows)];
    let mut x: u16 = 0;
    let mut y: u16 = 0;
    let bytes = src.as_bytes();
    let mut i = 0;
    while i < bytes.len() {
        if bytes[i] == b'<' {
            if src[i..].starts_with("<span") {
                let rest = &src[i..];
                let Some(end_tag) = rest.find('>') else {
                    break;
                };
                let tag = &rest[..=end_tag];
                let after = &rest[end_tag + 1..];
                let Some(close) = after.find("</span>") else {
                    break;
                };
                let body = &after[..close];
                let text = html_unescape(body);
                let fg = hex_color(tag, "color:#").unwrap_or(DEFAULT_FG);
                let bg = hex_color(tag, "background:#").unwrap_or(DEFAULT_BG);
                let bold = tag.contains("font-weight:700") || tag.contains("font-weight:bold");
                let italic = tag.contains("font-style:italic");
                let underline = tag.contains("underline");
                let strike = tag.contains("line-through");
                let dim = tag.contains("opacity");
                for ch in text.chars() {
                    if ch == '\n' {
                        x = 0;
                        y = y.saturating_add(1);
                        continue;
                    }
                    if y < rows && x < cols {
                        let idx = usize::from(y) * usize::from(cols) + usize::from(x);
                        cells[idx] = GridCell {
                            ch: ch.to_string(),
                            fg,
                            bg,
                            bold,
                            dim,
                            italic,
                            underline,
                            reverse: false,
                            strike,
                        };
                    }
                    x = x.saturating_add(1);
                }
                i += end_tag + 1 + close + "</span>".len();
                continue;
            }
            if let Some(gt) = src[i..].find('>') {
                i += gt + 1;
                continue;
            }
            break;
        }
        if bytes[i] == b'\n' {
            x = 0;
            y = y.saturating_add(1);
            i += 1;
            continue;
        }
        i += 1;
    }
    Grid { cols, rows, cells }
}

fn hex_color(tag: &str, key: &str) -> Option<[u8; 3]> {
    let i = tag.find(key)?;
    let h = tag.get(i + key.len()..i + key.len() + 6)?;
    let n = u32::from_str_radix(h, 16).ok()?;
    Some([
        ((n >> 16) & 0xff) as u8,
        ((n >> 8) & 0xff) as u8,
        (n & 0xff) as u8,
    ])
}

fn html_unescape(s: &str) -> String {
    s.replace("&amp;", "&")
        .replace("&lt;", "<")
        .replace("&gt;", ">")
        .replace("&quot;", "\"")
        .replace("&apos;", "'")
        .replace("&#39;", "'")
        .replace("&#x27;", "'")
}

/// Trailing-space-normalized txt lines (tmux `capture-pane -p`).
#[must_use]
pub fn norm_txt(s: &str) -> Vec<String> {
    s.lines().map(|l| l.trim_end().to_string()).collect()
}

/// First txt mismatch after trailing-space normalization.
#[must_use]
pub fn first_txt_diff(ours: &str, src: &str) -> Option<(u16, u16, char, char)> {
    let a = norm_txt(ours);
    let b = norm_txt(src);
    let rows = a.len().max(b.len());
    for y in 0..rows {
        let al = a.get(y).map(String::as_str).unwrap_or("");
        let bl = b.get(y).map(String::as_str).unwrap_or("");
        let ac: Vec<char> = al.chars().collect();
        let bc: Vec<char> = bl.chars().collect();
        let cols = ac.len().max(bc.len());
        for x in 0..cols {
            let ca = *ac.get(x).unwrap_or(&' ');
            let cb = *bc.get(x).unwrap_or(&' ');
            if ca != cb {
                return Some((x as u16, y as u16, cb, ca));
            }
        }
    }
    None
}
