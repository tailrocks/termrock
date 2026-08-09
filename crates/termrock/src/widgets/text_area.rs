//! Multi-line, grapheme-safe text editing with two-axis viewport ownership.

use ratatui_core::{
    buffer::Buffer,
    layout::{Position, Rect},
    widgets::{StatefulWidget, Widget},
};

use crate::{
    input::{Event, KeyCode, KeyEvent, KeyEventKind, KeyModifiers, MouseButton, MouseEventKind},
    style::{Density, DesignSystem, Role, RolePalette},
    text::{display_cols, display_cols_slice_into},
};

use super::{
    Panel, PanelChrome, ScrollArea, ScrollAreaState, ScrollBarVisibility, ScrollChain, edit_core,
};

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
    /// Viewport scrolled without document change.
    Scrolled,
    /// Editing requested cancellation.
    Cancelled,
}

/// Owned multi-line editor state.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TextAreaState {
    lines: Vec<String>,
    cursor: TextCursor,
    goal_column: Option<usize>,
    /// Dual-axis viewport via canonical [`ScrollAreaState`] (native-feel input box).
    scroll: ScrollAreaState,
    accepts_input: bool,
    read_only: bool,
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
            scroll: ScrollAreaState::new()
                .axes(true, true)
                .chain(ScrollChain::Capture)
                .wheel_steps(3, 4),
            accepts_input: false,
            read_only: false,
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
        self.scroll = ScrollAreaState::new()
            .axes(true, true)
            .chain(ScrollChain::Capture)
            .wheel_steps(3, 4);
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

    /// Absolute UTF-8 byte offset of `cursor` in LF-joined document text.
    #[must_use]
    pub fn absolute_byte(&self, cursor: TextCursor) -> Option<usize> {
        if cursor.line >= self.lines.len()
            || !edit_core::is_boundary(&self.lines[cursor.line], cursor.byte)
        {
            return None;
        }
        let mut abs = 0usize;
        for (i, line) in self.lines.iter().enumerate() {
            if i == cursor.line {
                return Some(abs.saturating_add(cursor.byte.min(line.len())));
            }
            abs = abs.saturating_add(line.len()).saturating_add(1);
        }
        None
    }

    /// Cursor at absolute byte in LF-joined document (clamped to grapheme boundary).
    #[must_use]
    pub fn cursor_at_byte(&self, abs: usize) -> TextCursor {
        let mut remaining = abs;
        let last = self.lines.len().saturating_sub(1);
        for (i, line) in self.lines.iter().enumerate() {
            if remaining <= line.len() {
                let byte = edit_core::boundary_at_or_before(line, remaining);
                return TextCursor { line: i, byte };
            }
            if i == last {
                return TextCursor {
                    line: i,
                    byte: line.len(),
                };
            }
            remaining = remaining.saturating_sub(line.len().saturating_add(1));
        }
        TextCursor {
            line: last,
            byte: self.lines.last().map_or(0, String::len),
        }
    }

    /// Text between two cursors (order-independent). Empty range → empty string.
    #[must_use]
    pub fn extract_between(&self, a: TextCursor, b: TextCursor) -> Option<String> {
        let (start, end) = order_cursors(a, b);
        self.extract_range(start, end)
    }

    /// Replaces the span between two cursors with `replacement`, places cursor after it.
    pub fn replace_between(
        &mut self,
        a: TextCursor,
        b: TextCursor,
        replacement: &str,
    ) -> TextAreaOutcome {
        let Some(start_abs) = self.absolute_byte(a) else {
            return TextAreaOutcome::Ignored;
        };
        let Some(end_abs) = self.absolute_byte(b) else {
            return TextAreaOutcome::Ignored;
        };
        let (lo, hi) = if start_abs <= end_abs {
            (start_abs, end_abs)
        } else {
            (end_abs, start_abs)
        };
        let text = self.text();
        let lo = lo.min(text.len());
        let hi = hi.min(text.len()).max(lo);
        let mut next = String::with_capacity(text.len() - (hi - lo) + replacement.len());
        next.push_str(&text[..lo]);
        next.push_str(replacement);
        next.push_str(&text[hi..]);
        let cursor_abs = lo.saturating_add(replacement.len());
        self.set_text(&next);
        let cursor = self.cursor_at_byte(cursor_abs);
        let _ = self.set_cursor(cursor);
        TextAreaOutcome::Changed
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

    /// Host input gate (scene/overlay ownership). Does not clear buffer.
    pub const fn set_accepts_input(&mut self, accepts: bool) {
        self.accepts_input = accepts;
    }

    /// Whether host granted keyboard/pointer input.
    #[must_use]
    pub const fn accepts_input(&self) -> bool {
        self.accepts_input
    }

    /// When true, navigation/scroll allowed but edits ignored.
    pub const fn set_read_only(&mut self, read_only: bool) {
        self.read_only = read_only;
    }

    /// Read-only flag.
    #[must_use]
    pub const fn is_read_only(&self) -> bool {
        self.read_only
    }

    /// Deprecated name for [`Self::set_accepts_input`].
    #[deprecated(note = "use set_accepts_input")]
    pub const fn set_focused(&mut self, focused: bool) {
        self.accepts_input = focused;
    }

    /// Deprecated name for [`Self::accepts_input`].
    #[deprecated(note = "use accepts_input")]
    #[must_use]
    pub const fn is_focused(&self) -> bool {
        self.accepts_input
    }

    /// Two-axis viewport ([`ScrollAreaState`] — same engine as lists/logs).
    #[must_use]
    pub const fn scroll(&self) -> &ScrollAreaState {
        &self.scroll
    }

    /// Mutable scroll (hosts may overscan / chain when nested).
    pub fn scroll_mut(&mut self) -> &mut ScrollAreaState {
        &mut self.scroll
    }

    /// Applies a bounded two-axis viewport delta (y then x; native editor wheel).
    pub fn scroll_by(&mut self, delta_x: isize, delta_y: isize) -> bool {
        self.scroll.scroll_by(delta_y, delta_x).is_scrolled()
    }

    /// Maps a pointer on either painted scrollbar track to its content offset.
    pub fn scroll_to(&mut self, position: Position) -> bool {
        if let Some(area) = self
            .vertical_scrollbar
            .filter(|area| area.contains(position))
        {
            let before = self.scroll.offset_y();
            let y = u16::try_from(crate::scroll::offset_for_track_position(
                self.lines.len(),
                self.viewport_height,
                area.height,
                usize::from(position.y.saturating_sub(area.y)),
            ))
            .unwrap_or(u16::MAX);
            self.scroll.set_offset_y_quiet(y);
            return before != self.scroll.offset_y();
        }
        if let Some(area) = self
            .horizontal_scrollbar
            .filter(|area| area.contains(position))
        {
            let before = self.scroll.offset_x();
            let x = u16::try_from(crate::scroll::offset_for_track_position(
                self.max_width,
                self.viewport_width,
                area.width,
                usize::from(position.x.saturating_sub(area.x)),
            ))
            .unwrap_or(u16::MAX);
            self.scroll.set_offset_x(x);
            return before != self.scroll.offset_x();
        }
        false
    }

    /// Inserts normalized single- or multi-line text at the cursor.
    pub fn insert_text(&mut self, text: &str) -> TextAreaOutcome {
        let edits = self.insert_text_deltas(text);
        self.finish_edit(edits.discard())
    }

    /// Semantic navigation/cancel (chars still use [`Self::handle_key`]).
    pub fn handle_intent(&mut self, intent: crate::interaction::UiIntent) -> TextAreaOutcome {
        use crate::interaction::{NavigationMove, PageMove, UiIntent};
        if !self.accepts_input {
            return TextAreaOutcome::Ignored;
        }
        match intent {
            UiIntent::Cancel | UiIntent::Close => TextAreaOutcome::Cancelled,
            UiIntent::Move(NavigationMove::Previous) => {
                if self.left() {
                    self.reveal();
                    TextAreaOutcome::Changed
                } else {
                    TextAreaOutcome::Ignored
                }
            }
            UiIntent::Move(NavigationMove::Next) => {
                if self.right() {
                    self.reveal();
                    TextAreaOutcome::Changed
                } else {
                    TextAreaOutcome::Ignored
                }
            }
            UiIntent::Move(NavigationMove::First) => {
                if self.edge(false) {
                    self.reveal();
                    TextAreaOutcome::Changed
                } else {
                    TextAreaOutcome::Ignored
                }
            }
            UiIntent::Move(NavigationMove::Last) => {
                if self.edge(true) {
                    self.reveal();
                    TextAreaOutcome::Changed
                } else {
                    TextAreaOutcome::Ignored
                }
            }
            UiIntent::Page(PageMove::Backward) => {
                let step = -isize::try_from(self.viewport_height.max(1)).unwrap_or(isize::MAX);
                if self.vertical(step) {
                    self.reveal();
                    TextAreaOutcome::Changed
                } else {
                    TextAreaOutcome::Ignored
                }
            }
            UiIntent::Page(PageMove::Forward) => {
                let step = isize::try_from(self.viewport_height.max(1)).unwrap_or(isize::MAX);
                if self.vertical(step) {
                    self.reveal();
                    TextAreaOutcome::Changed
                } else {
                    TextAreaOutcome::Ignored
                }
            }
            // Up/Down as Move is ambiguous with Previous/Next; use Page for page.
            // Vertical line motion stays key-path (Up/Down).
            _ => TextAreaOutcome::Ignored,
        }
    }

    /// Routes keyboard editing when host granted input. Enter inserts a newline.
    pub fn handle_key(&mut self, key: KeyEvent) -> TextAreaOutcome {
        if !self.accepts_input || key.kind == KeyEventKind::Release {
            return TextAreaOutcome::Ignored;
        }
        if key.code == KeyCode::Esc {
            return TextAreaOutcome::Cancelled;
        }
        let plain = key.modifiers.is_empty() || key.modifiers == KeyModifiers::SHIFT;
        if !plain {
            return TextAreaOutcome::Ignored;
        }
        // Intent peel for Home/End/Page/Esc already; Up/Down stay local line moves.
        if key.modifiers.is_empty()
            && matches!(
                key.code,
                KeyCode::Home | KeyCode::End | KeyCode::PageUp | KeyCode::PageDown
            )
        {
            if let Some(intent) = crate::interaction::default_text_area_intent(key) {
                let out = self.handle_intent(intent);
                if !matches!(out, TextAreaOutcome::Ignored) {
                    return out;
                }
            }
        }
        let vertical_delta = match key.code {
            KeyCode::Up => Some(-1),
            KeyCode::Down => Some(1),
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
        if self.read_only {
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
            Event::Paste(text) if self.accepts_input && !self.read_only => self.insert_text(&text),
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
            Event::Mouse(mouse) if self.accepts_input && self.body.contains(mouse.position) => {
                // Wheel uses ScrollArea (multi-line steps) so input feels like a native editor.
                match mouse.kind {
                    MouseEventKind::ScrollUp
                    | MouseEventKind::ScrollDown
                    | MouseEventKind::ScrollLeft
                    | MouseEventKind::ScrollRight => {
                        if self.scroll.handle_mouse(mouse).is_scrolled() {
                            TextAreaOutcome::Scrolled
                        } else {
                            TextAreaOutcome::Ignored
                        }
                    }
                    _ => TextAreaOutcome::Ignored,
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
}

fn order_cursors(a: TextCursor, b: TextCursor) -> (TextCursor, TextCursor) {
    if a.line < b.line || (a.line == b.line && a.byte <= b.byte) {
        (a, b)
    } else {
        (b, a)
    }
}

impl TextAreaState {
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
    fn sync_scroll_metrics(&mut self) {
        let h = u16::try_from(self.lines.len().min(usize::from(u16::MAX))).unwrap_or(u16::MAX);
        let w = u16::try_from(self.max_width.min(usize::from(u16::MAX))).unwrap_or(u16::MAX);
        let vh = u16::try_from(self.viewport_height.min(usize::from(u16::MAX))).unwrap_or(1);
        let vw = u16::try_from(self.viewport_width.min(usize::from(u16::MAX))).unwrap_or(1);
        // Quiet content size so stream-like growth does not pause "follow" (N/A here).
        self.scroll.set_content_size(w, h);
        self.scroll.set_viewport(vw, vh.max(1));
    }

    fn clamp_scroll(&mut self) {
        self.sync_scroll_metrics();
        self.scroll.clamp();
    }

    /// Keep caret in view like a native multiline editor (no accidental follow-pause).
    fn reveal(&mut self) {
        self.sync_scroll_metrics();
        if self.viewport_height > 0 {
            let y = usize::from(self.scroll.offset_y());
            if self.cursor.line < y {
                self.scroll
                    .set_offset_y_quiet(u16::try_from(self.cursor.line).unwrap_or(u16::MAX));
            } else if self.cursor.line >= y + self.viewport_height {
                self.scroll.set_offset_y_quiet(
                    u16::try_from(self.cursor.line + 1 - self.viewport_height).unwrap_or(u16::MAX),
                );
            }
        }
        let col = display_cols(&self.lines[self.cursor.line][..self.cursor.byte]);
        let x = usize::from(self.scroll.offset_x());
        if col < x {
            self.scroll
                .set_offset_x(u16::try_from(col).unwrap_or(u16::MAX));
        } else if self.viewport_width > 0 && col >= x + self.viewport_width {
            self.scroll
                .set_offset_x(u16::try_from(col + 1 - self.viewport_width).unwrap_or(u16::MAX));
        }
        self.scroll.clamp();
    }
}

/// Themed multi-line text editor.
#[derive(Debug, Clone, Copy)]
pub struct TextArea<'a> {
    system: &'a DesignSystem,
    title: Option<&'a str>,
    placeholder: Option<&'a str>,
    ascii: bool,
    colorless: bool,
}

impl<'a> TextArea<'a> {
    /// Creates an untitled editor.
    #[must_use]
    pub const fn new(system: &'a DesignSystem) -> Self {
        Self {
            system,
            title: None,
            placeholder: None,
            ascii: false,
            colorless: false,
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

    /// ASCII scrollbar / empty cues.
    #[must_use]
    pub const fn ascii(mut self, ascii: bool) -> Self {
        self.ascii = ascii;
        self
    }

    /// Reduced-color caret/chrome.
    #[must_use]
    pub const fn colorless(mut self, colorless: bool) -> Self {
        self.colorless = colorless;
        self
    }
}

impl StatefulWidget for &TextArea<'_> {
    type State = TextAreaState;
    fn render(self, area: Rect, buffer: &mut Buffer, state: &mut Self::State) {
        let tokens = self.system.clone();
        let mut panel = Panel::new(&tokens).emphasis(if state.accepts_input {
            PanelChrome::Focused
        } else {
            PanelChrome::Normal
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
        state.sync_scroll_metrics();
        state.reveal();
        if body.is_empty() {
            return;
        }
        let first = usize::from(state.scroll.offset_y());
        let last = (first + state.viewport_height).min(state.lines.len());
        for (painted, line) in state.lines[first..last].iter().enumerate() {
            display_cols_slice_into(
                line,
                usize::from(state.scroll.offset_x()),
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
                self.system.style(if line.is_empty() {
                    Role::TextMuted
                } else {
                    Role::Text
                }),
            );
        }
        if state.accepts_input && state.cursor.line >= first && state.cursor.line < last {
            let col = display_cols(&state.lines[state.cursor.line][..state.cursor.byte])
                .saturating_sub(usize::from(state.scroll.offset_x()));
            let x = body
                .x
                .saturating_add(u16::try_from(col).unwrap_or(u16::MAX))
                .min(body.right().saturating_sub(1));
            let y = body.y + u16::try_from(state.cursor.line - first).unwrap_or(u16::MAX);
            let caret = if self.colorless {
                self.system.style(Role::TextStrong)
            } else {
                self.system.style(Role::Focus)
            };
            buffer.set_style(Rect::new(x, y, 1, 1), caret);
        }
        // Canonical scrollbar chrome (same glyphs/roles as ScrollArea surfaces).
        if show_vertical || show_horizontal {
            let sa = ScrollArea::new(self.system).bar(ScrollBarVisibility::Auto);
            sa.render_bars(inner, buffer, &state.scroll);
            if show_vertical && inner.width > 0 {
                state.vertical_scrollbar = Some(Rect::new(body.right(), inner.y, 1, body.height));
            }
            if show_horizontal && inner.height > 0 {
                state.horizontal_scrollbar =
                    Some(Rect::new(inner.x, body.bottom(), body.width, 1));
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
        state.set_accepts_input(true);
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
        state.set_accepts_input(true);
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
            state.set_accepts_input(true);
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
        state.set_accepts_input(true);
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
    fn tall_document_scroll_keeps_caret_visible_like_native_editor() {
        // Grok Build / Amp-style: multi-line draft taller than viewport; arrow
        // and page keep caret in view without allocating full paint.
        let mut lines = String::new();
        for i in 0..40 {
            lines.push_str(&format!("line {i:02} draft body\n"));
        }
        let mut state = TextAreaState::new(lines);
        state.set_accepts_input(true);
        state.viewport_width = 24;
        state.viewport_height = 6;
        state.sync_scroll_metrics();
        assert!(state.set_cursor(TextCursor {
            line: 30,
            byte: 0
        }));
        state.reveal();
        let y = state.scroll.offset_y() as usize;
        assert!(y <= 30);
        assert!(y + 6 > 30, "caret line must sit in viewport");
        // Wheel down via ScrollArea steps
        assert!(state.scroll_by(0, 3));
        assert!(state.scroll.offset_y() > y as u16);
    }

    #[test]
    fn scrollbars_stay_inside_panel_and_own_press_drag_geometry() {
        let theme = RolePalette::default();
        let system = crate::style::DesignSystem::from_palette(theme.clone());
        let mut state = TextAreaState::new("wide content beyond viewport\none\ntwo\nthree\nfour");
        state.set_accepts_input(true);
        assert!(state.set_cursor(c(0, 0)));
        let area = Rect::new(2, 3, 14, 6);
        let mut buffer = Buffer::empty(Rect::new(0, 0, 20, 12));
        (&TextArea::new(&system).title("Edit")).render(area, &mut buffer, &mut state);
        assert_eq!(buffer[(area.right() - 1, area.y)].symbol(), "┐");
        assert_eq!(buffer[(area.x, area.bottom() - 1)].symbol(), "└");
        let vertical = state.vertical_scrollbar.unwrap();
        let outcome = state.handle_event(Event::Mouse(crate::input::MouseEvent {
            kind: MouseEventKind::Down(MouseButton::Left),
            position: Position::new(vertical.x, vertical.bottom() - 1),
            modifiers: KeyModifiers::NONE,
        }));
        assert_eq!(outcome, TextAreaOutcome::Changed);
        assert!(state.scroll.offset_y() > 0);
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
    fn accepts_input_gate_and_read_only() {
        let mut state = TextAreaState::new("ab");
        assert_eq!(
            state.handle_key(KeyEvent::new(KeyCode::Char('x'), KeyModifiers::NONE)),
            TextAreaOutcome::Ignored
        );
        state.set_accepts_input(true);
        assert_eq!(
            state.handle_key(KeyEvent::new(KeyCode::Char('x'), KeyModifiers::NONE)),
            TextAreaOutcome::Changed
        );
        state.set_read_only(true);
        assert_eq!(
            state.handle_key(KeyEvent::new(KeyCode::Char('y'), KeyModifiers::NONE)),
            TextAreaOutcome::Ignored
        );
        let _ = state.handle_key(KeyEvent::new(KeyCode::Left, KeyModifiers::NONE));
    }

    #[test]
    fn measurement_invalidates_only_on_edits_and_tiny_control_input_is_safe() {
        let mut state = TextAreaState::new("ab");
        state.set_accepts_input(true);
        assert_eq!(state.max_width, 2);
        assert_eq!(state.insert_text("\u{7}東京"), TextAreaOutcome::Changed);
        assert_eq!(state.text(), "ab東京");
        assert_eq!(state.max_width, 6);
        let measured = state.max_width;
        let _ = state.handle_key(KeyEvent::new(KeyCode::Left, KeyModifiers::NONE));
        assert_eq!(state.max_width, measured);
        for area in [Rect::new(0, 0, 0, 0), Rect::new(2, 2, 1, 1)] {
            let mut buffer = Buffer::empty(area);
            (&TextArea::new(&crate::style::DesignSystem::default())).render(
                area,
                &mut buffer,
                &mut state,
            );
        }
    }
}
