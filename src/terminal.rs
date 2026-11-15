use ratatui::buffer::{Buffer, CellWidth};
use ratatui::crossterm::event::{KeyCode, KeyModifiers};
use ratatui::layout::{Position, Rect};
use ratatui::style::{Color, Modifier, Style};
use ratatui::widgets::Widget;
use termpane::{DamageGrid, EraseDisplayPolicy, PassthroughEvent, ScrollOp};

use crate::core::event::Key;

mod forward;
mod guard;
mod viewport;

pub use forward::{ForwardPolicy, Paste};
pub use guard::is_spoofing_format;
pub use viewport::TerminalViewport;

pub const SCROLLBACK_ROWS: usize = 1_000;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct Modes {
    pub app_cursor: bool,
    pub bracketed_paste: bool,
    pub alt_screen: bool,
    pub hide_cursor: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TermEvent {
    Reply(Vec<u8>),
    Title(String),
    ClipboardWrite(String),
    Bell,
    Notification(String),
    Cwd(String),
    ScrollbackCleared,
}

impl TermEvent {
    fn from_passthrough(event: PassthroughEvent) -> Option<Self> {
        match event {
            PassthroughEvent::Reply(bytes) => Some(Self::Reply(bytes)),
            PassthroughEvent::TitleChanged(title) => Some(Self::Title(title)),
            PassthroughEvent::ClipboardWrite(payload) => Some(Self::ClipboardWrite(payload)),
            PassthroughEvent::Bell => Some(Self::Bell),
            PassthroughEvent::Notification(text) => Some(Self::Notification(text)),
            PassthroughEvent::CwdChanged(uri) => Some(Self::Cwd(uri)),
            PassthroughEvent::ScrollbackClear => Some(Self::ScrollbackCleared),
            PassthroughEvent::IconNameChanged(_)
            | PassthroughEvent::ApplicationCursorKeys(_)
            | PassthroughEvent::FocusEvents(_)
            | PassthroughEvent::BracketedPaste(_)
            | PassthroughEvent::Hyperlink { .. }
            | PassthroughEvent::UnhandledCsi(_)
            | PassthroughEvent::DroppedCsi(_) => None,
        }
    }
}

pub struct TerminalState {
    grid: DamageGrid,
    limit: usize,
    scrolled_off: u64,
    guard: guard::FeedGuard,
}

impl std::fmt::Debug for TerminalState {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let (rows, cols) = self.size();
        f.debug_struct("TerminalState")
            .field("rows", &rows)
            .field("cols", &cols)
            .finish_non_exhaustive()
    }
}

impl TerminalState {
    pub fn new(rows: u16, cols: u16) -> Self {
        Self::with_scrollback(rows, cols, SCROLLBACK_ROWS)
    }

    pub fn with_scrollback(rows: u16, cols: u16, limit: usize) -> Self {
        let mut grid = DamageGrid::new(rows.max(1), cols.max(1), limit);
        grid.set_erase_display_policy(EraseDisplayPolicy::Discard);
        Self {
            grid,
            limit,
            scrolled_off: 0,
            guard: guard::FeedGuard::new(),
        }
    }

    pub fn feed(&mut self, bytes: &[u8]) -> Vec<TermEvent> {
        let guard::Filtered { bytes, switches } = self.guard.filter(bytes);
        let mut start = 0;
        for end in switches.iter().copied().chain([bytes.len()]) {
            if end == start {
                continue;
            }
            let before = self.grid.scrollback_len();
            let alternate = self.grid.alternate_screen();
            self.grid.process(&bytes[start..end]);
            self.scrolled_off += pushed_since(&mut self.grid, self.limit, before, alternate);
            start = end;
        }
        self.grid
            .drain_passthrough()
            .into_iter()
            .filter_map(TermEvent::from_passthrough)
            .collect()
    }

    pub fn resize(&mut self, rows: u16, cols: u16) {
        self.grid.set_size(rows, cols);
    }

    pub fn reset(&mut self) {
        let (rows, cols) = self.size();
        *self = Self::with_scrollback(rows, cols, self.limit);
    }

    pub fn size(&self) -> (u16, u16) {
        self.grid.size()
    }

    pub fn modes(&self) -> Modes {
        Modes {
            app_cursor: self.grid.application_cursor(),
            bracketed_paste: self.grid.bracketed_paste(),
            alt_screen: self.grid.alternate_screen(),
            hide_cursor: self.grid.hide_cursor(),
        }
    }

    pub fn scrollback_len(&self) -> usize {
        self.grid.scrollback_len()
    }

    /// Rows that left the top of the main screen since the start, counted
    /// whether or not the history keeps them.
    pub fn scrolled_off(&self) -> u64 {
        self.scrolled_off
    }

    pub fn scrollback(&self) -> usize {
        self.grid.scrollback()
    }

    /// Whether the program asked to be told when the pane gains or loses
    /// focus (`CSI ? 1004 h`).
    pub fn focus_events(&self) -> bool {
        self.grid.focus_events()
    }

    /// Whether the program asked for mouse reports (any of `CSI ? 1000 h`,
    /// `1002` and `1003`).
    pub fn mouse_reporting(&self) -> bool {
        self.grid.mouse_protocol_mode() != termpane::MouseProtocolMode::None
    }

    /// Drops the history and keeps the screen. The count of rows that left
    /// the top of the screen (`scrolled_off`) is a fact about the past and
    /// stays.
    pub fn clear_scrollback(&mut self) {
        self.grid.clear_scrollback();
    }

    pub fn set_scrollback(&mut self, rows: usize) {
        self.grid.set_scrollback(rows);
    }

    pub fn view(&self, offset: usize) -> TermView<'_> {
        let offset = if self.grid.alternate_screen() {
            0
        } else {
            offset
        };
        let (rows, _) = self.grid.size();
        TermView {
            grid: self.grid.scrollback_view(offset, rows),
        }
    }

    pub fn cursor(&self, area: Rect) -> Option<Position> {
        if self.grid.hide_cursor() || self.grid.scrollback() > 0 {
            return None;
        }
        let (row, col) = self.view(0).cursor();
        (row < area.height && col < area.width).then(|| Position::new(area.x + col, area.y + row))
    }
}

#[derive(Debug)]
pub struct TermView<'a> {
    grid: termpane::GridView<'a>,
}

impl<'a> TermView<'a> {
    pub fn rows(&self) -> u16 {
        self.grid.rows
    }

    pub fn cols(&self) -> u16 {
        self.grid.cols
    }

    pub fn cell(&self, row: u16, col: u16) -> Option<TermCell<'a>> {
        self.grid.cell(row, col).map(|cell| TermCell { cell })
    }

    pub fn row_wrap(&self, row: u16) -> Option<RowWrap> {
        self.grid.row_wrap(row).map(|wrap| match wrap {
            termpane::RowWrap::Hard => RowWrap::Hard,
            termpane::RowWrap::Soft => RowWrap::Soft,
        })
    }

    pub fn cursor(&self) -> (u16, u16) {
        let (row, col) = self.grid.cursor;
        (row, col.min(self.grid.cols.saturating_sub(1)))
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RowWrap {
    Hard,
    Soft,
}

#[derive(Debug, Clone, Copy)]
pub struct TermCell<'a> {
    cell: &'a termpane::Cell,
}

impl TermCell<'_> {
    pub fn text(&self) -> &str {
        self.cell.contents()
    }

    pub fn is_wide_continuation(&self) -> bool {
        self.cell.is_wide_continuation
    }
}

#[derive(Debug, Clone, Copy)]
pub struct TerminalView<'a> {
    state: &'a TerminalState,
    focused: bool,
    color: bool,
}

impl<'a> TerminalView<'a> {
    pub fn new(state: &'a TerminalState) -> Self {
        Self {
            state,
            focused: false,
            color: true,
        }
    }

    pub fn focused(mut self, focused: bool) -> Self {
        self.focused = focused;
        self
    }

    pub fn color(mut self, color: bool) -> Self {
        self.color = color;
        self
    }
}

#[derive(Debug, Clone, Copy)]
struct Defaults {
    fg: Color,
    bg: Color,
}

const TERMINAL_DEFAULTS: Defaults = Defaults {
    fg: Color::Reset,
    bg: Color::Reset,
};

fn pushed_since(grid: &mut DamageGrid, limit: usize, before: usize, alternate: bool) -> u64 {
    let ops = grid.drain_scroll_ops();
    let len = grid.scrollback_len();
    if alternate {
        0
    } else if limit > 0 && len < limit {
        (len - before.min(len)) as u64
    } else {
        ops.iter()
            .map(|op| match op {
                ScrollOp::Up { top: 0, rows, .. } => u64::from(*rows),
                _ => 0,
            })
            .sum()
    }
}

fn color(value: termpane::Color, default: Color) -> Color {
    match value {
        termpane::Color::Default => default,
        termpane::Color::Idx(index) => Color::Indexed(index),
        termpane::Color::Rgb(r, g, b) => Color::Rgb(r, g, b),
    }
}

fn style(cell: &termpane::Cell, with_color: bool, defaults: Defaults) -> Style {
    let mut style = Style::default();
    if with_color {
        style = style
            .fg(color(cell.fgcolor(), defaults.fg))
            .bg(color(cell.bgcolor(), defaults.bg));
    }
    for (on, modifier) in [
        (cell.bold(), Modifier::BOLD),
        (cell.dim(), Modifier::DIM),
        (cell.italic(), Modifier::ITALIC),
        (cell.underline(), Modifier::UNDERLINED),
        (cell.inverse(), Modifier::REVERSED),
        (cell.strikethrough(), Modifier::CROSSED_OUT),
    ] {
        if on {
            style = style.add_modifier(modifier);
        }
    }
    style
}

/// Paints `view` into `area`, keeping every emitted symbol inside the part of
/// `area` that overlaps `buf`. Source cell `(row, col)` always lands at
/// `area` origin plus `(col, row)`, so clipping on the left or top never
/// shifts the picture. A wide symbol that does not fit completely becomes a
/// blank cell with its own style.
fn paint_view(
    view: &TermView<'_>,
    area: Rect,
    buf: &mut Buffer,
    with_color: bool,
    defaults: Defaults,
) {
    let visible = area.intersection(*buf.area());
    if visible.is_empty() {
        return;
    }
    let rows = area.height.min(view.rows());
    let cols = area.width.min(view.cols());
    let right = visible.right().min(area.x + cols);
    let first_row = visible.y - area.y;
    let first_col = visible.x - area.x;
    for row in first_row..rows.min(visible.bottom() - area.y) {
        let y = area.y + row;
        for col in first_col..cols {
            let x = area.x + col;
            if x >= right {
                break;
            }
            let Some(cell) = view.cell(row, col) else {
                continue;
            };
            let text = cell.text();
            let symbol = if cell.is_wide_continuation() {
                if x > visible.x {
                    continue;
                }
                " "
            } else if text.is_empty() || !(1..=right - x).contains(&text.cell_width()) {
                " "
            } else {
                text
            };
            let target = &mut buf[(x, y)];
            target.set_symbol(symbol);
            target.set_style(style(cell.cell, with_color, defaults));
        }
    }
}

impl Widget for TerminalView<'_> {
    fn render(self, area: Rect, buf: &mut Buffer) {
        let view = self.state.view(self.state.scrollback());
        paint_view(&view, area, buf, self.color, TERMINAL_DEFAULTS);
        if self.focused
            && let Some(at) = self.state.cursor(area)
            && buf.area().contains(at)
        {
            let target = &mut buf[(at.x, at.y)];
            target.modifier.toggle(Modifier::REVERSED);
        }
    }
}

/// The bytes a terminal program expects for `key`.
///
/// Navigation, function, Insert, Delete and Page keys with Ctrl or Shift use
/// the xterm modifier parameter (`CSI 1 ; m X`, `CSI n ; m ~`, m = 1 + Shift +
/// 2 Alt + 4 Ctrl). Alt alone keeps the older `ESC` prefix. Shift+Enter stays
/// `\r`: legacy terminals have no distinct code for it.
pub fn encode_key(key: &Key, modes: &Modes) -> Option<Vec<u8>> {
    let ctrl = key.mods.contains(KeyModifiers::CONTROL);
    let alt = key.mods.contains(KeyModifiers::ALT);
    let shift = key.mods.contains(KeyModifiers::SHIFT);
    let param =
        (ctrl || shift).then(|| 1 + u8::from(shift) + 2 * u8::from(alt) + 4 * u8::from(ctrl));
    let cursor = |letter: u8| match param {
        Some(m) => format!("\x1b[1;{m}{}", letter as char).into_bytes(),
        None if modes.app_cursor => vec![0x1b, b'O', letter],
        None => vec![0x1b, b'[', letter],
    };
    let tilde = |n: u8| match param {
        Some(m) => format!("\x1b[{n};{m}~").into_bytes(),
        None => format!("\x1b[{n}~").into_bytes(),
    };
    let function = |n: u8| match param {
        Some(m) => format!("\x1b[1;{m}{}", (b'P' + n - 1) as char).into_bytes(),
        None => vec![0x1b, b'O', b'P' + n - 1],
    };
    let modified_nav = param.is_some()
        && matches!(
            key.code,
            KeyCode::Up
                | KeyCode::Down
                | KeyCode::Right
                | KeyCode::Left
                | KeyCode::Home
                | KeyCode::End
                | KeyCode::PageUp
                | KeyCode::PageDown
                | KeyCode::Delete
                | KeyCode::Insert
                | KeyCode::F(1..=12)
        );
    let mut bytes = match key.code {
        KeyCode::Char(c) if ctrl && c.is_ascii_alphabetic() => {
            vec![c.to_ascii_lowercase() as u8 - b'a' + 1]
        }
        KeyCode::Char(' ') if ctrl => vec![0],
        KeyCode::Char(c @ ('@'..='_')) if ctrl => vec![c as u8 & 0x1f],
        KeyCode::Char('/') if ctrl => vec![0x1f],
        KeyCode::Char(c) => c.to_string().into_bytes(),
        KeyCode::Enter => vec![b'\r'],
        KeyCode::Backspace if ctrl => vec![0x08],
        KeyCode::Backspace => vec![0x7f],
        KeyCode::Tab => vec![b'\t'],
        KeyCode::BackTab => b"\x1b[Z".to_vec(),
        KeyCode::Esc => vec![0x1b],
        KeyCode::Up => cursor(b'A'),
        KeyCode::Down => cursor(b'B'),
        KeyCode::Right => cursor(b'C'),
        KeyCode::Left => cursor(b'D'),
        KeyCode::Home => cursor(b'H'),
        KeyCode::End => cursor(b'F'),
        KeyCode::PageUp => tilde(5),
        KeyCode::PageDown => tilde(6),
        KeyCode::Delete => tilde(3),
        KeyCode::Insert => tilde(2),
        KeyCode::F(n @ 1..=4) => function(n),
        KeyCode::F(n @ 5..=12) => {
            const CODES: [u8; 8] = [15, 17, 18, 19, 20, 21, 23, 24];
            tilde(CODES[usize::from(n - 5)])
        }
        _ => return None,
    };
    if alt && !modified_nav {
        bytes.insert(0, 0x1b);
    }
    Some(bytes)
}
