//! Multi-line, grapheme-safe text editing with two-axis viewport ownership.

use ratatui_core::{
    buffer::Buffer,
    layout::{Position, Rect},
    widgets::{StatefulWidget, Widget},
};

use crate::{
    Theme,
    input::{Event, KeyCode, KeyEvent, KeyEventKind, KeyModifiers, MouseButton, MouseEventKind},
    scroll::DialogScroll,
    style::Role,
    text::{display_cols, display_cols_slice_into},
};

use super::{Panel, PanelEmphasis, edit_core};

#[derive(Debug, Clone, PartialEq, Eq)]
enum TextEditDelta {
    Line {
        line: usize,
        delta: edit_core::LineDelta,
    },
    Split {
        at: TextCursor,
    },
    Joined {
        inverse_split: JoinPoint,
    },
}

enum TextEditBatch {
    None,
    One(TextEditDelta),
    Many(Vec<TextEditDelta>),
}

impl TextEditBatch {
    fn discard(self) -> bool {
        match self {
            Self::None => false,
            Self::One(edit) => {
                drop(edit);
                true
            }
            Self::Many(edits) => {
                drop(edits);
                true
            }
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct JoinPoint {
    line: usize,
    byte: usize,
}

/// Stable normalized cursor coordinate.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct TextCursor {
    /// Zero-based logical line.
    pub line: usize,
    /// UTF-8 byte offset at an extended-grapheme boundary.
    pub byte: usize,
}

/// Semantic result of text-area interaction.
#[must_use]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[non_exhaustive]
pub enum TextAreaOutcome {
    /// Input was not applicable.
    Ignored,
    /// Text or cursor state changed.
    Changed,
    /// Editing requested cancellation.
    Cancelled,
}

/// Owned multi-line editor state.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TextAreaState {
    lines: Vec<String>,
    cursor: TextCursor,
    goal_column: Option<usize>,
    scroll: DialogScroll,
    focused: bool,
    viewport_width: usize,
    viewport_height: usize,
    max_width: usize,
    body: Rect,
    vertical_scrollbar: Option<Rect>,
    horizontal_scrollbar: Option<Rect>,
    scratch: String,
}

impl Default for TextAreaState {
    fn default() -> Self {
        Self::new("")
    }
}

impl TextAreaState {
    /// Creates state from text, normalizing CRLF, LF, and CR line endings.
    #[must_use]
    pub fn new(text: impl AsRef<str>) -> Self {
        let mut state = Self {
            lines: parse_lines(text.as_ref()),
            cursor: TextCursor::default(),
            goal_column: None,
            scroll: DialogScroll::new(),
            focused: false,
            viewport_width: 0,
            viewport_height: 0,
            max_width: 0,
            body: Rect::default(),
            vertical_scrollbar: None,
            horizontal_scrollbar: None,
            scratch: String::new(),
        };
        state.cursor.line = state.lines.len() - 1;
        state.cursor.byte = state.lines.last().map_or(0, String::len);
        state.measure();
        state
    }

    /// Replaces the document and places the cursor at its end.
    pub fn set_text(&mut self, text: &str) {
        self.lines = parse_lines(text);
        self.cursor.line = self.lines.len() - 1;
        self.cursor.byte = self.lines[self.cursor.line].len();
        self.goal_column = None;
        self.scroll = DialogScroll::new();
        self.measure();
    }

    /// Returns normalized logical lines.
    pub fn lines(&self) -> impl ExactSizeIterator<Item = &str> {
        self.lines.iter().map(String::as_str)
    }

    /// Extracts the normalized document with LF separators.
    #[must_use]
    pub fn text(&self) -> String {
        let end_line = self.lines.len() - 1;
        self.extract_range(
            TextCursor::default(),
            TextCursor {
                line: end_line,
                byte: self.lines[end_line].len(),
            },
        )
        .unwrap_or_default()
    }

    /// Returns the cursor coordinate.
    #[must_use]
    pub const fn cursor(&self) -> TextCursor {
        self.cursor
    }

    /// Sets a cursor only when it names an existing grapheme boundary.
    pub fn set_cursor(&mut self, cursor: TextCursor) -> bool {
        if self
            .lines
            .get(cursor.line)
            .is_some_and(|line| edit_core::is_boundary(line, cursor.byte))
        {
            self.cursor = cursor;
            self.goal_column = None;
            self.reveal();
            true
        } else {
            false
        }
    }

    /// Sets keyboard-focus ownership.
    pub const fn set_focused(&mut self, focused: bool) {
        self.focused = focused;
    }

    /// Returns keyboard-focus ownership.
    #[must_use]
    pub const fn is_focused(&self) -> bool {
        self.focused
    }

    /// Returns two-axis viewport state.
    #[must_use]
    pub const fn scroll(&self) -> &DialogScroll {
        &self.scroll
    }

    /// Applies a bounded two-axis viewport delta.
    pub fn scroll_by(&mut self, delta_x: isize, delta_y: isize) -> bool {
        let before = (self.scroll.scroll_x, self.scroll.scroll_y);
        let delta_x =
            i16::try_from(delta_x).unwrap_or(if delta_x < 0 { i16::MIN } else { i16::MAX });
        let delta_y =
            i16::try_from(delta_y).unwrap_or(if delta_y < 0 { i16::MIN } else { i16::MAX });
        self.scroll.scroll_x = self.scroll.scroll_x.saturating_add_signed(delta_x);
        self.scroll.scroll_y = self.scroll.scroll_y.saturating_add_signed(delta_y);
        self.clamp_scroll();
        before != (self.scroll.scroll_x, self.scroll.scroll_y)
    }

    /// Maps a pointer on either painted scrollbar track to its content offset.
    pub fn scroll_to(&mut self, position: Position) -> bool {
        if let Some(area) = self
            .vertical_scrollbar
            .filter(|area| area.contains(position))
        {
            let before = self.scroll.scroll_y;
            self.scroll.scroll_y = u16::try_from(crate::scroll::offset_for_track_position(
                self.lines.len(),
                self.viewport_height,
                area.height,
                usize::from(position.y.saturating_sub(area.y)),
            ))
            .unwrap_or(u16::MAX);
            return before != self.scroll.scroll_y;
        }
        if let Some(area) = self
            .horizontal_scrollbar
            .filter(|area| area.contains(position))
        {
            let before = self.scroll.scroll_x;
            self.scroll.scroll_x = u16::try_from(crate::scroll::offset_for_track_position(
                self.max_width,
                self.viewport_width,
                area.width,
                usize::from(position.x.saturating_sub(area.x)),
            ))
            .unwrap_or(u16::MAX);
            return before != self.scroll.scroll_x;
        }
        false
    }

    /// Inserts normalized single- or multi-line text at the cursor.
    pub fn insert_text(&mut self, text: &str) -> TextAreaOutcome {
        let edits = self.insert_text_deltas(text);
        self.finish_edit(edits.discard())
    }

    /// Routes focused keyboard editing. Enter always inserts a newline.
    pub fn handle_key(&mut self, key: KeyEvent) -> TextAreaOutcome {
        if !self.focused || key.kind == KeyEventKind::Release {
            return TextAreaOutcome::Ignored;
        }
        if key.code == KeyCode::Esc {
            return TextAreaOutcome::Cancelled;
        }
        let plain = key.modifiers.is_empty() || key.modifiers == KeyModifiers::SHIFT;
        if !plain {
            return TextAreaOutcome::Ignored;
        }
        let vertical_delta = match key.code {
            KeyCode::Up => Some(-1),
            KeyCode::Down => Some(1),
            KeyCode::PageUp => {
                Some(-isize::try_from(self.viewport_height.max(1)).unwrap_or(isize::MAX))
            }
            KeyCode::PageDown => {
                Some(isize::try_from(self.viewport_height.max(1)).unwrap_or(isize::MAX))
            }
            _ => None,
        };
        if let Some(delta) = vertical_delta {
            if self.vertical(delta) {
                self.reveal();
                return TextAreaOutcome::Changed;
            }
            return TextAreaOutcome::Ignored;
        }
        let motion = match key.code {
            KeyCode::Left => Some(self.left()),
            KeyCode::Right => Some(self.right()),
            KeyCode::Home => Some(self.edge(false)),
            KeyCode::End => Some(self.edge(true)),
            _ => None,
        };
        if let Some(changed) = motion {
            if changed {
                self.reveal();
                return TextAreaOutcome::Changed;
            }
            return TextAreaOutcome::Ignored;
        }
        let changed = match key.code {
            KeyCode::Enter => self.newline().is_some(),
            KeyCode::Backspace => self.backspace().is_some(),
            KeyCode::Delete => self.delete().is_some(),
            KeyCode::Char(character) if !character.is_control() => {
                let line = self.cursor.line;
                edit_core::insert_char(&mut self.lines[line], &mut self.cursor.byte, character)
                    .map(|delta| TextEditDelta::Line { line, delta })
                    .is_some()
            }
            _ => false,
        };
        self.finish_edit(changed)
    }

    /// Routes neutral keyboard, paste, and owned wheel events.
    pub fn handle_event(&mut self, event: Event) -> TextAreaOutcome {
        match event {
            Event::Key(key) => self.handle_key(key),
            Event::Paste(text) if self.focused => self.insert_text(&text),
            Event::Mouse(mouse)
                if matches!(
                    mouse.kind,
                    MouseEventKind::Down(MouseButton::Left)
                        | MouseEventKind::Drag(MouseButton::Left)
                ) =>
            {
                if self.scroll_to(mouse.position) {
                    TextAreaOutcome::Changed
                } else {
                    TextAreaOutcome::Ignored
                }
            }
            Event::Mouse(mouse) if self.body.contains(mouse.position) => {
                let changed = match mouse.kind {
                    MouseEventKind::ScrollUp => self.scroll_by(0, -1),
                    MouseEventKind::ScrollDown => self.scroll_by(0, 1),
                    _ => false,
                };
                if changed {
                    TextAreaOutcome::Changed
                } else {
                    TextAreaOutcome::Ignored
                }
            }
            _ => TextAreaOutcome::Ignored,
        }
    }

    fn newline(&mut self) -> Option<TextEditDelta> {
        let at = self.cursor;
        let suffix = self.lines[self.cursor.line].split_off(self.cursor.byte);
        self.cursor.line += 1;
        self.cursor.byte = 0;
        self.lines.insert(self.cursor.line, suffix);
        Some(TextEditDelta::Split { at })
    }
    fn insert_text_deltas(&mut self, text: &str) -> TextEditBatch {
        if !text.chars().any(|character| {
            matches!(character, '\r' | '\n') || crate::text::is_terminal_control_char(character)
        }) {
            let line = self.cursor.line;
            return edit_core::insert_inline(&mut self.lines[line], &mut self.cursor.byte, text)
                .map(|delta| TextEditBatch::One(TextEditDelta::Line { line, delta }))
                .unwrap_or(TextEditBatch::None);
        }
        let parts = parse_lines(text);
        let mut edits = Vec::with_capacity(parts.len().saturating_mul(2));
        if let Some(delta) = edit_core::insert_inline(
            &mut self.lines[self.cursor.line],
            &mut self.cursor.byte,
            &parts[0],
        ) {
            edits.push(TextEditDelta::Line {
                line: self.cursor.line,
                delta,
            });
        }
        for part in &parts[1..] {
            edits.push(self.newline().expect("newline always mutates"));
            if let Some(delta) = edit_core::insert_inline(
                &mut self.lines[self.cursor.line],
                &mut self.cursor.byte,
                part,
            ) {
                edits.push(TextEditDelta::Line {
                    line: self.cursor.line,
                    delta,
                });
            }
        }
        match edits.len() {
            0 => TextEditBatch::None,
            1 => TextEditBatch::One(edits.pop().expect("one edit exists")),
            _ => TextEditBatch::Many(edits),
        }
    }
    fn backspace(&mut self) -> Option<TextEditDelta> {
        let line = self.cursor.line;
        if let Some(delta) = edit_core::backspace(&mut self.lines[line], &mut self.cursor.byte) {
            return Some(TextEditDelta::Line { line, delta });
        }
        if self.cursor.line == 0 {
            return None;
        }
        let current = self.lines.remove(self.cursor.line);
        self.cursor.line -= 1;
        let seam = self.lines[self.cursor.line].len();
        self.lines[self.cursor.line].push_str(&current);
        self.cursor.byte = edit_core::boundary_at_or_after(&self.lines[self.cursor.line], seam);
        Some(TextEditDelta::Joined {
            inverse_split: JoinPoint {
                line: self.cursor.line,
                byte: seam,
            },
        })
    }
    fn delete(&mut self) -> Option<TextEditDelta> {
        let line = self.cursor.line;
        if let Some(delta) = edit_core::delete(&mut self.lines[line], self.cursor.byte) {
            return Some(TextEditDelta::Line { line, delta });
        }
        if self.cursor.line + 1 == self.lines.len() {
            return None;
        }
        let next = self.lines.remove(self.cursor.line + 1);
        let seam = self.cursor.byte;
        self.lines[self.cursor.line].push_str(&next);
        self.cursor.byte = edit_core::boundary_at_or_after(&self.lines[self.cursor.line], seam);
        Some(TextEditDelta::Joined {
            inverse_split: JoinPoint {
                line: self.cursor.line,
                byte: seam,
            },
        })
    }
    fn left(&mut self) -> bool {
        self.goal_column = None;
        if let Some(byte) =
            edit_core::previous_boundary(&self.lines[self.cursor.line], self.cursor.byte)
        {
            self.cursor.byte = byte;
            true
        } else if self.cursor.line > 0 {
            self.cursor.line -= 1;
            self.cursor.byte = self.lines[self.cursor.line].len();
            true
        } else {
            false
        }
    }
    fn right(&mut self) -> bool {
        self.goal_column = None;
        if let Some(byte) =
            edit_core::next_boundary(&self.lines[self.cursor.line], self.cursor.byte)
        {
            self.cursor.byte = byte;
            true
        } else if self.cursor.line + 1 < self.lines.len() {
            self.cursor.line += 1;
            self.cursor.byte = 0;
            true
        } else {
            false
        }
    }
    fn edge(&mut self, end: bool) -> bool {
        self.goal_column = None;
        let next = if end {
            self.lines[self.cursor.line].len()
        } else {
            0
        };
        let changed = next != self.cursor.byte;
        self.cursor.byte = next;
        changed
    }
    fn vertical(&mut self, delta: isize) -> bool {
        let before = self.cursor;
        let goal = *self
            .goal_column
            .get_or_insert_with(|| display_cols(&self.lines[self.cursor.line][..self.cursor.byte]));
        self.cursor.line = self
            .cursor
            .line
            .saturating_add_signed(delta)
            .min(self.lines.len() - 1);
        self.cursor.byte = edit_core::byte_at_display_column(&self.lines[self.cursor.line], goal);
        self.cursor != before
    }

    fn extract_range(&self, start: TextCursor, end: TextCursor) -> Option<String> {
        if start.line > end.line
            || start.line >= self.lines.len()
            || end.line >= self.lines.len()
            || !edit_core::is_boundary(&self.lines[start.line], start.byte)
            || !edit_core::is_boundary(&self.lines[end.line], end.byte)
        {
            return None;
        }
        if start.line == end.line {
            return (start.byte <= end.byte)
                .then(|| self.lines[start.line][start.byte..end.byte].to_owned());
        }
        let mut out = self.lines[start.line][start.byte..].to_owned();
        for line in start.line + 1..end.line {
            out.push('\n');
            out.push_str(&self.lines[line]);
        }
        out.push('\n');
        out.push_str(&self.lines[end.line][..end.byte]);
        Some(out)
    }

    #[cfg(test)]
    fn apply_inverse(&mut self, edit: TextEditDelta) {
        match edit {
            TextEditDelta::Line {
                line,
                delta: edit_core::LineDelta::Inserted { range },
            } => {
                self.lines[line].replace_range(range, "");
            }
            TextEditDelta::Line {
                line,
                delta: edit_core::LineDelta::Deleted { at, text },
            } => {
                self.lines[line].insert_str(at, &text);
            }
            TextEditDelta::Split { at } => {
                let suffix = self.lines.remove(at.line + 1);
                self.lines[at.line].push_str(&suffix);
            }
            TextEditDelta::Joined { inverse_split } => {
                let suffix = self.lines[inverse_split.line].split_off(inverse_split.byte);
                self.lines.insert(inverse_split.line + 1, suffix);
            }
        }
        self.measure();
    }
    #[cfg(test)]
    fn apply_inverse_batch(&mut self, edits: TextEditBatch) {
        match edits {
            TextEditBatch::None => {}
            TextEditBatch::One(edit) => self.apply_inverse(edit),
            TextEditBatch::Many(edits) => {
                for edit in edits.into_iter().rev() {
                    self.apply_inverse(edit);
                }
            }
        }
    }
    fn finish_edit(&mut self, changed: bool) -> TextAreaOutcome {
        if !changed {
            return TextAreaOutcome::Ignored;
        }
        self.goal_column = None;
        self.measure();
        self.reveal();
        TextAreaOutcome::Changed
    }
    fn measure(&mut self) {
        self.max_width = self
            .lines
            .iter()
            .map(|line| display_cols(line))
            .max()
            .unwrap_or(0);
        self.clamp_scroll();
    }
    fn clamp_scroll(&mut self) {
        self.scroll.clamp(
            self.lines.len(),
            self.viewport_height,
            self.max_width,
            self.viewport_width,
        );
    }
    fn reveal(&mut self) {
        if self.viewport_height > 0 {
            let y = usize::from(self.scroll.scroll_y);
            if self.cursor.line < y {
                self.scroll.scroll_y = u16::try_from(self.cursor.line).unwrap_or(u16::MAX);
            } else if self.cursor.line >= y + self.viewport_height {
                self.scroll.scroll_y =
                    u16::try_from(self.cursor.line + 1 - self.viewport_height).unwrap_or(u16::MAX);
            }
        }
        let col = display_cols(&self.lines[self.cursor.line][..self.cursor.byte]);
        let x = usize::from(self.scroll.scroll_x);
        if col < x {
            self.scroll.scroll_x = u16::try_from(col).unwrap_or(u16::MAX);
        } else if self.viewport_width > 0 && col >= x + self.viewport_width {
            self.scroll.scroll_x = u16::try_from(col + 1 - self.viewport_width).unwrap_or(u16::MAX);
        }
        self.clamp_scroll();
    }
}

/// Themed multi-line text editor.
#[derive(Debug, Clone, Copy)]
pub struct TextArea<'a> {
    theme: &'a Theme,
    title: Option<&'a str>,
    placeholder: Option<&'a str>,
}

impl<'a> TextArea<'a> {
    /// Creates an untitled editor.
    #[must_use]
    pub const fn new(theme: &'a Theme) -> Self {
        Self {
            theme,
            title: None,
            placeholder: None,
        }
    }
    /// Sets panel title.
    #[must_use]
    pub const fn title(mut self, title: &'a str) -> Self {
        self.title = Some(title);
        self
    }
    /// Sets empty-document placeholder.
    #[must_use]
    pub const fn placeholder(mut self, placeholder: &'a str) -> Self {
        self.placeholder = Some(placeholder);
        self
    }
}

impl StatefulWidget for &TextArea<'_> {
    type State = TextAreaState;
    fn render(self, area: Rect, buffer: &mut Buffer, state: &mut Self::State) {
        let mut panel = Panel::new(self.theme).emphasis(if state.focused {
            PanelEmphasis::Focused
        } else {
            PanelEmphasis::Normal
        });
        if let Some(title) = self.title {
            panel = panel.title(title);
        }
        let inner = panel.inner(area);
        panel.render(area, buffer);
        let mut show_vertical = false;
        let mut show_horizontal = false;
        for _ in 0..2 {
            let width = inner.width.saturating_sub(u16::from(show_vertical));
            let height = inner.height.saturating_sub(u16::from(show_horizontal));
            show_vertical = crate::scroll::is_scrollable(state.lines.len(), usize::from(height));
            show_horizontal = crate::scroll::is_scrollable(state.max_width, usize::from(width));
        }
        let body = Rect::new(
            inner.x,
            inner.y,
            inner.width.saturating_sub(u16::from(show_vertical)),
            inner.height.saturating_sub(u16::from(show_horizontal)),
        );
        state.body = body;
        state.vertical_scrollbar = None;
        state.horizontal_scrollbar = None;
        state.viewport_width = usize::from(body.width);
        state.viewport_height = usize::from(body.height);
        state.reveal();
        if body.is_empty() {
            return;
        }
        let first = usize::from(state.scroll.scroll_y);
        let last = (first + state.viewport_height).min(state.lines.len());
        for (painted, line) in state.lines[first..last].iter().enumerate() {
            display_cols_slice_into(
                line,
                usize::from(state.scroll.scroll_x),
                state.viewport_width,
                &mut state.scratch,
            );
            if line.is_empty()
                && state.lines.len() == 1
                && let Some(placeholder) = self.placeholder
            {
                display_cols_slice_into(placeholder, 0, state.viewport_width, &mut state.scratch);
            }
            buffer.set_stringn(
                body.x,
                body.y + u16::try_from(painted).unwrap_or(u16::MAX),
                &state.scratch,
                state.viewport_width,
                self.theme.style(if line.is_empty() {
                    Role::TextMuted
                } else {
                    Role::Text
                }),
            );
        }
        if state.focused && state.cursor.line >= first && state.cursor.line < last {
            let col = display_cols(&state.lines[state.cursor.line][..state.cursor.byte])
                .saturating_sub(usize::from(state.scroll.scroll_x));
            let x = body
                .x
                .saturating_add(u16::try_from(col).unwrap_or(u16::MAX))
                .min(body.right().saturating_sub(1));
            let y = body.y + u16::try_from(state.cursor.line - first).unwrap_or(u16::MAX);
            buffer.set_style(Rect::new(x, y, 1, 1), self.theme.style(Role::Focus));
        }
        if show_vertical && inner.width > 0 {
            let track = Rect::new(body.right(), inner.y, 1, body.height);
            state.vertical_scrollbar = Some(track);
            for y in track.top()..track.bottom() {
                buffer.set_string(track.x, y, "░", self.theme.style(Role::ScrollTrack));
            }
            if let Some(thumb) = crate::scroll::full_cell_thumb(
                state.lines.len(),
                state.viewport_height,
                track.height,
                usize::from(state.scroll.scroll_y),
            ) {
                for y in thumb.start..thumb.start.saturating_add(thumb.len) {
                    buffer.set_string(
                        track.x,
                        track.y + y,
                        "█",
                        self.theme.style(Role::ScrollThumb),
                    );
                }
            }
        }
        if show_horizontal && inner.height > 0 {
            let track = Rect::new(inner.x, body.bottom(), body.width, 1);
            state.horizontal_scrollbar = Some(track);
            for x in track.left()..track.right() {
                buffer.set_string(x, track.y, "░", self.theme.style(Role::ScrollTrack));
            }
            if let Some(thumb) = crate::scroll::full_cell_thumb(
                state.max_width,
                state.viewport_width,
                track.width,
                usize::from(state.scroll.scroll_x),
            ) {
                for x in thumb.start..thumb.start.saturating_add(thumb.len) {
                    buffer.set_string(
                        track.x + x,
                        track.y,
                        "█",
                        self.theme.style(Role::ScrollThumb),
                    );
                }
            }
        }
    }
}

impl StatefulWidget for TextArea<'_> {
    type State = TextAreaState;
    fn render(self, area: Rect, buffer: &mut Buffer, state: &mut Self::State) {
        (&self).render(area, buffer, state);
    }
}

fn parse_lines(text: &str) -> Vec<String> {
    let normalized = text.replace("\r\n", "\n").replace('\r', "\n");
    normalized
        .split('\n')
        .map(|line| {
            line.chars()
                .filter(|character| !crate::text::is_terminal_control_char(*character))
                .collect()
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn normalized_editing_and_goal_column_contract() {
        let mut state = TextAreaState::new("ab🧪\r\nx\r12345");
        state.set_focused(true);
        assert_eq!(state.lines().collect::<Vec<_>>(), ["ab🧪", "x", "12345"]);
        assert!(state.set_cursor(TextCursor { line: 2, byte: 4 }));
        assert_eq!(
            state.handle_key(KeyEvent::new(KeyCode::Up, KeyModifiers::NONE)),
            TextAreaOutcome::Changed
        );
        assert_eq!(state.cursor, TextCursor { line: 1, byte: 1 });
        assert_eq!(
            state.handle_key(KeyEvent::new(KeyCode::Up, KeyModifiers::NONE)),
            TextAreaOutcome::Changed
        );
        assert_eq!(state.cursor, TextCursor { line: 0, byte: 6 });
    }
    #[test]
    fn paste_split_join_and_invalid_cursor_are_safe() {
        let mut state = TextAreaState::new("e\u{301}x");
        state.set_focused(true);
        assert!(!state.set_cursor(TextCursor { line: 0, byte: 1 }));
        assert!(state.set_cursor(TextCursor { line: 0, byte: 3 }));
        assert_eq!(state.insert_text("A\r\nB\rC"), TextAreaOutcome::Changed);
        assert_eq!(state.text(), "e\u{301}A\nB\nCx");
    }

    #[test]
    fn edit_and_cursor_contract_table() {
        struct Case {
            name: &'static str,
            text: &'static str,
            cursor: TextCursor,
            key: KeyCode,
            expected: &'static str,
            expected_cursor: TextCursor,
            changed: bool,
        }
        let cases = [
            (
                "insert ascii",
                "",
                c(0, 0),
                KeyCode::Char('a'),
                "a",
                c(0, 1),
                true,
            ),
            (
                "insert cjk",
                "",
                c(0, 0),
                KeyCode::Char('東'),
                "東",
                c(0, 3),
                true,
            ),
            (
                "insert emoji",
                "",
                c(0, 0),
                KeyCode::Char('🧪'),
                "🧪",
                c(0, 4),
                true,
            ),
            (
                "newline middle",
                "ab",
                c(0, 1),
                KeyCode::Enter,
                "a\nb",
                c(1, 0),
                true,
            ),
            (
                "newline start",
                "ab",
                c(0, 0),
                KeyCode::Enter,
                "\nab",
                c(1, 0),
                true,
            ),
            (
                "newline end",
                "ab",
                c(0, 2),
                KeyCode::Enter,
                "ab\n",
                c(1, 0),
                true,
            ),
            (
                "backspace ascii",
                "ab",
                c(0, 2),
                KeyCode::Backspace,
                "a",
                c(0, 1),
                true,
            ),
            (
                "backspace cluster",
                "e\u{301}",
                c(0, 3),
                KeyCode::Backspace,
                "",
                c(0, 0),
                true,
            ),
            (
                "backspace join",
                "a\nb",
                c(1, 0),
                KeyCode::Backspace,
                "ab",
                c(0, 1),
                true,
            ),
            (
                "backspace start",
                "a",
                c(0, 0),
                KeyCode::Backspace,
                "a",
                c(0, 0),
                false,
            ),
            (
                "delete ascii",
                "ab",
                c(0, 0),
                KeyCode::Delete,
                "b",
                c(0, 0),
                true,
            ),
            (
                "delete emoji",
                "🧪x",
                c(0, 0),
                KeyCode::Delete,
                "x",
                c(0, 0),
                true,
            ),
            (
                "delete join",
                "a\nb",
                c(0, 1),
                KeyCode::Delete,
                "ab",
                c(0, 1),
                true,
            ),
            (
                "delete end",
                "a",
                c(0, 1),
                KeyCode::Delete,
                "a",
                c(0, 1),
                false,
            ),
            (
                "left cluster",
                "e\u{301}",
                c(0, 3),
                KeyCode::Left,
                "e\u{301}",
                c(0, 0),
                true,
            ),
            (
                "left line",
                "a\nb",
                c(1, 0),
                KeyCode::Left,
                "a\nb",
                c(0, 1),
                true,
            ),
            (
                "right emoji",
                "🧪x",
                c(0, 0),
                KeyCode::Right,
                "🧪x",
                c(0, 4),
                true,
            ),
            (
                "right line",
                "a\nb",
                c(0, 1),
                KeyCode::Right,
                "a\nb",
                c(1, 0),
                true,
            ),
            ("home", "abc", c(0, 2), KeyCode::Home, "abc", c(0, 0), true),
            ("end", "abc", c(0, 1), KeyCode::End, "abc", c(0, 3), true),
            (
                "up wide boundary",
                "a🧪\n123",
                c(1, 2),
                KeyCode::Up,
                "a🧪\n123",
                c(0, 1),
                true,
            ),
            (
                "down empty",
                "ab\n",
                c(0, 2),
                KeyCode::Down,
                "ab\n",
                c(1, 0),
                true,
            ),
            (
                "insert combining",
                "e",
                c(0, 1),
                KeyCode::Char('\u{301}'),
                "e\u{301}",
                c(0, 3),
                true,
            ),
            (
                "base before mark",
                "\u{301}x",
                c(0, 0),
                KeyCode::Char('e'),
                "e\u{301}x",
                c(0, 3),
                true,
            ),
            (
                "zwj join",
                "👩\u{200d}",
                c(0, 7),
                KeyCode::Char('💻'),
                "👩\u{200d}💻",
                c(0, 11),
                true,
            ),
            (
                "backspace combining join",
                "e\n\u{301}x",
                c(1, 0),
                KeyCode::Backspace,
                "e\u{301}x",
                c(0, 3),
                true,
            ),
            (
                "delete combining join",
                "e\n\u{301}x",
                c(0, 1),
                KeyCode::Delete,
                "e\u{301}x",
                c(0, 3),
                true,
            ),
            (
                "up empty",
                "\nab",
                c(1, 2),
                KeyCode::Up,
                "\nab",
                c(0, 0),
                true,
            ),
            (
                "page up clamp",
                "a\nb",
                c(1, 1),
                KeyCode::PageUp,
                "a\nb",
                c(0, 1),
                true,
            ),
            (
                "page down clamp",
                "a\nb",
                c(0, 1),
                KeyCode::PageDown,
                "a\nb",
                c(1, 1),
                true,
            ),
            (
                "left start",
                "a",
                c(0, 0),
                KeyCode::Left,
                "a",
                c(0, 0),
                false,
            ),
            (
                "right end",
                "a",
                c(0, 1),
                KeyCode::Right,
                "a",
                c(0, 1),
                false,
            ),
            (
                "home start",
                "a",
                c(0, 0),
                KeyCode::Home,
                "a",
                c(0, 0),
                false,
            ),
            ("end end", "a", c(0, 1), KeyCode::End, "a", c(0, 1), false),
        ]
        .map(
            |(name, text, cursor, key, expected, expected_cursor, changed)| Case {
                name,
                text,
                cursor,
                key,
                expected,
                expected_cursor,
                changed,
            },
        );
        for case in cases {
            let mut state = TextAreaState::new(case.text);
            state.set_focused(true);
            assert!(state.set_cursor(case.cursor), "{} cursor", case.name);
            let outcome = state.handle_key(KeyEvent::new(case.key, KeyModifiers::NONE));
            assert_eq!(
                outcome == TextAreaOutcome::Changed,
                case.changed,
                "{} outcome",
                case.name
            );
            assert_eq!(state.text(), case.expected, "{} text", case.name);
            assert_eq!(state.cursor(), case.expected_cursor, "{} cursor", case.name);
        }
    }

    const fn c(line: usize, byte: usize) -> TextCursor {
        TextCursor { line, byte }
    }

    #[test]
    fn multi_line_deltas_and_ranges_restore_without_document_snapshots() {
        let mut state = TextAreaState::new("alpha\nbeta\ngamma");
        assert_eq!(
            state.extract_range(c(0, 2), c(2, 2)).as_deref(),
            Some("pha\nbeta\nga")
        );
        state.set_cursor(c(1, 2));
        let split = state.newline().unwrap();
        assert_eq!(state.text(), "alpha\nbe\nta\ngamma");
        state.apply_inverse(split);
        assert_eq!(state.text(), "alpha\nbeta\ngamma");

        state.set_cursor(c(1, 0));
        let join = state.backspace().unwrap();
        assert_eq!(state.text(), "alphabeta\ngamma");
        state.apply_inverse(join);
        assert_eq!(state.text(), "alpha\nbeta\ngamma");

        state.set_cursor(c(0, 1));
        let inserted = state.insert_text_deltas("東京\r\nnext");
        assert_eq!(state.text(), "a東京\nnextlpha\nbeta\ngamma");
        state.apply_inverse_batch(inserted);
        assert_eq!(state.text(), "alpha\nbeta\ngamma");

        state.set_cursor(c(0, 5));
        let delete_join = state.delete().unwrap();
        assert_eq!(state.text(), "alphabeta\ngamma");
        state.apply_inverse(delete_join);
        assert_eq!(state.text(), "alpha\nbeta\ngamma");

        state.set_cursor(c(0, 5));
        let deleted = state.backspace().unwrap();
        assert_eq!(state.text(), "alph\nbeta\ngamma");
        state.apply_inverse(deleted);
        assert_eq!(state.text(), "alpha\nbeta\ngamma");
    }

    #[test]
    fn scrollbars_stay_inside_panel_and_own_press_drag_geometry() {
        let theme = Theme::default();
        let mut state = TextAreaState::new("wide content beyond viewport\none\ntwo\nthree\nfour");
        assert!(state.set_cursor(c(0, 0)));
        let area = Rect::new(2, 3, 14, 6);
        let mut buffer = Buffer::empty(Rect::new(0, 0, 20, 12));
        (&TextArea::new(&theme).title("Edit")).render(area, &mut buffer, &mut state);
        assert_eq!(buffer[(area.right() - 1, area.y)].symbol(), "┐");
        assert_eq!(buffer[(area.x, area.bottom() - 1)].symbol(), "└");
        let vertical = state.vertical_scrollbar.unwrap();
        let outcome = state.handle_event(Event::Mouse(crate::input::MouseEvent {
            kind: MouseEventKind::Down(MouseButton::Left),
            position: Position::new(vertical.x, vertical.bottom() - 1),
            modifiers: KeyModifiers::NONE,
        }));
        assert_eq!(outcome, TextAreaOutcome::Changed);
        assert!(state.scroll.scroll_y > 0);
        assert_eq!(
            state.handle_event(Event::Mouse(crate::input::MouseEvent {
                kind: MouseEventKind::Drag(MouseButton::Left),
                position: Position::new(vertical.x, vertical.bottom() - 1),
                modifiers: KeyModifiers::NONE,
            })),
            TextAreaOutcome::Ignored
        );
        assert_eq!(
            state.handle_event(Event::Mouse(crate::input::MouseEvent {
                kind: MouseEventKind::Drag(MouseButton::Left),
                position: Position::new(0, 0),
                modifiers: KeyModifiers::NONE,
            })),
            TextAreaOutcome::Ignored
        );
    }

    #[test]
    fn measurement_invalidates_only_on_edits_and_tiny_control_input_is_safe() {
        let mut state = TextAreaState::new("ab");
        assert_eq!(state.max_width, 2);
        state.set_focused(true);
        assert_eq!(state.insert_text("\u{7}東京"), TextAreaOutcome::Changed);
        assert_eq!(state.text(), "ab東京");
        assert_eq!(state.max_width, 6);
        let measured = state.max_width;
        let _ = state.handle_key(KeyEvent::new(KeyCode::Left, KeyModifiers::NONE));
        assert_eq!(state.max_width, measured);
        for area in [Rect::new(0, 0, 0, 0), Rect::new(2, 2, 1, 1)] {
            let mut buffer = Buffer::empty(area);
            (&TextArea::new(&Theme::default())).render(area, &mut buffer, &mut state);
        }
    }
}
