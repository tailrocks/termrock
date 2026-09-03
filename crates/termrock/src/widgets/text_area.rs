//! Multi-line, grapheme-safe text editing with two-axis viewport ownership.
//!
//! **Mission.** Forms, notes, prompts, and comments need a multi-line editor
//! with selection, undo/redo, word movement, indent, optional soft wrap, line
//! numbers, ScrollArea chrome, and host hooks for external editor / fullscreen.
//!
//! **vs [`TextInput`](crate::widgets::TextInput).** Single-line field.
//! **vs [`PromptComposer`](crate::widgets::PromptComposer).** Product prompt shell
//! that embeds TextArea and may own additional undo/selection layers.
//!
//! Research: tui-textarea, prompt-toolkit, terminal editors, agent composers.
use ratatui_core::{
    buffer::Buffer,
    layout::{Position, Rect},
    style::{Modifier, Style},
    widgets::StatefulWidget,
};

use crate::{
    input::{Event, KeyCode, KeyEvent, KeyModifiers, MouseButton, MouseEventKind},
    interaction::{SemanticNode, SemanticRole, SemanticScene, SemanticState},
    style::{DesignSystem, VisualState},
    text::{display_cols, display_cols_slice_into, take_display_cols, truncate_cols},
};

use super::{ScrollAreaState, ScrollChain, edit_core};

/// Undo snapshot limit for standalone TextArea.
const UNDO_LIMIT: usize = 64;
/// Default indent unit (two spaces).
const DEFAULT_INDENT: &str = "  ";

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
#[derive(Debug, Clone, PartialEq, Eq)]
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
    /// Host should copy this text.
    ClipboardCopy {
        /// Selection or document slice.
        text: String,
    },
    /// Host should copy; text already removed.
    ClipboardCut {
        /// Cut text.
        text: String,
    },
    /// Host should resolve paste and call [`TextAreaState::insert_text`].
    ClipboardPasteRequest,
    /// Host should open external editor.
    ExternalEditorRequested,
    /// Host should promote to fullscreen overlay.
    FullscreenRequested,
}

/// Soft-wrap vs horizontal scroll.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
#[non_exhaustive]
pub enum TextWrap {
    /// Long lines scroll horizontally (default).
    #[default]
    None,
    /// Soft-wrap at viewport width (no horizontal scroll).
    Soft,
}

impl TextWrap {
    /// Stable id.
    #[must_use]
    pub const fn id(self) -> &'static str {
        match self {
            Self::None => "none",
            Self::Soft => "soft",
        }
    }
}

/// Visual / product recipe.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
#[non_exhaustive]
pub enum TextAreaVariant {
    /// Standard editor chrome.
    #[default]
    Editor,
    /// Review / comment: muted chrome.
    Review,
}

impl TextAreaVariant {
    /// Stable id.
    #[must_use]
    pub const fn id(self) -> &'static str {
        match self {
            Self::Editor => "editor",
            Self::Review => "review",
        }
    }
}

/// Owned multi-line editor state.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TextAreaState {
    lines: Vec<String>,
    cursor: TextCursor,
    /// Selection anchor when selecting.
    select_anchor: Option<TextCursor>,
    goal_column: Option<usize>,
    /// Dual-axis viewport via canonical [`ScrollAreaState`] (native-feel input box).
    scroll: ScrollAreaState,
    accepts_input: bool,
    /// Two-mode like junie TextInput: focused-idle vs editing.
    editing: bool,
    read_only: bool,
    wrap: TextWrap,
    indent: String,
    undo: Vec<String>,
    redo: Vec<String>,
    viewport_width: usize,
    viewport_height: usize,
    max_width: usize,
    /// Content height for scroll (visual rows when soft-wrapping).
    content_height: usize,
    /// When true, recompute `max_width` on next measure (edits only).
    metrics_dirty: bool,
    /// Last viewport width used for soft-wrap content height.
    soft_wrap_width: usize,
    body: Rect,
    vertical_scrollbar: Option<Rect>,
    horizontal_scrollbar: Option<Rect>,
    gutter_width: u16,
    scratch: String,
    selecting_mouse: bool,
    /// Hardware caret cell while editing (host applies `set_cursor_position`).
    hardware_cursor: Option<Position>,
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
            select_anchor: None,
            goal_column: None,
            scroll: ScrollAreaState::new()
                .axes(true, true)
                .chain(ScrollChain::Capture)
                .wheel_steps(3, 4),
            accepts_input: false,
            editing: false,
            read_only: false,
            wrap: TextWrap::None,
            indent: DEFAULT_INDENT.to_owned(),
            undo: Vec::new(),
            redo: Vec::new(),
            viewport_width: 0,
            viewport_height: 0,
            max_width: 0,
            content_height: 0,
            metrics_dirty: true,
            soft_wrap_width: 0,
            body: Rect::default(),
            vertical_scrollbar: None,
            horizontal_scrollbar: None,
            gutter_width: 0,
            scratch: String::new(),
            selecting_mouse: false,
            hardware_cursor: None,
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
        self.select_anchor = None;
        self.goal_column = None;
        self.scroll = ScrollAreaState::new()
            .axes(true, true)
            .chain(ScrollChain::Capture)
            .wheel_steps(3, 4);
        self.metrics_dirty = true;
        self.measure();
    }

    /// Soft wrap policy.
    pub const fn set_wrap(&mut self, wrap: TextWrap) {
        self.wrap = wrap;
        // Soft content height depends on wrap mode; recompute next measure.
        self.soft_wrap_width = 0;
    }

    /// Wrap policy.
    #[must_use]
    pub const fn wrap(&self) -> TextWrap {
        self.wrap
    }
    /// Selection anchor.
    #[must_use]
    pub const fn selection_anchor(&self) -> Option<TextCursor> {
        self.select_anchor
    }

    /// Active selection ordered range.
    #[must_use]
    pub fn selection_range(&self) -> Option<(TextCursor, TextCursor)> {
        let a = self.select_anchor?;
        if a == self.cursor {
            return None;
        }
        Some(order_cursors(a, self.cursor))
    }

    /// Selected text if any.
    #[must_use]
    pub fn selected_text(&self) -> Option<String> {
        let (a, b) = self.selection_range()?;
        self.extract_range(a, b)
    }

    /// Select entire document.
    pub fn select_all(&mut self) {
        self.select_anchor = Some(TextCursor::default());
        let last = self.lines.len() - 1;
        self.cursor = TextCursor {
            line: last,
            byte: self.lines[last].len(),
        };
        self.goal_column = None;
    }

    /// Clear selection without moving caret.
    pub fn clear_selection(&mut self) {
        self.select_anchor = None;
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
            if !self.selecting_mouse {
                self.select_anchor = None;
            }
            self.goal_column = None;
            self.reveal();
            true
        } else {
            false
        }
    }

    fn push_undo(&mut self) {
        let snap = self.text();
        if self.undo.last().is_some_and(|s| s == &snap) {
            return;
        }
        self.undo.push(snap);
        if self.undo.len() > UNDO_LIMIT {
            self.undo.remove(0);
        }
        self.redo.clear();
    }

    /// Undo last document mutation.
    pub fn undo(&mut self) -> TextAreaOutcome {
        if self.read_only || !self.accepts_input {
            return TextAreaOutcome::Ignored;
        }
        let Some(prev) = self.undo.pop() else {
            return TextAreaOutcome::Ignored;
        };
        self.redo.push(self.text());
        self.set_text(&prev);
        TextAreaOutcome::Changed
    }

    /// Redo.
    pub fn redo(&mut self) -> TextAreaOutcome {
        if self.read_only || !self.accepts_input {
            return TextAreaOutcome::Ignored;
        }
        let Some(next) = self.redo.pop() else {
            return TextAreaOutcome::Ignored;
        };
        self.undo.push(self.text());
        self.set_text(&next);
        TextAreaOutcome::Changed
    }

    fn delete_selection(&mut self) -> bool {
        let Some(anchor) = self.select_anchor.take() else {
            return false;
        };
        if anchor == self.cursor {
            return false;
        }
        let _ = self.replace_between(anchor, self.cursor, "");
        true
    }

    /// Host input gate (scene/overlay ownership). Does not clear buffer.
    /// Does not enter editing — host or Enter/F2 does.
    pub const fn set_accepts_input(&mut self, accepts: bool) {
        self.accepts_input = accepts;
        if !accepts {
            self.editing = false;
        }
    }

    /// Begin/leave editing. No-op when read-only.
    pub const fn set_editing(&mut self, editing: bool) {
        self.editing = editing && !self.read_only && self.accepts_input;
    }

    /// Whether the document currently accepts inserts (not merely focused).
    #[must_use]
    pub const fn is_editing(&self) -> bool {
        self.editing && self.accepts_input && !self.read_only
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

    /// Two-axis viewport ([`ScrollAreaState`] — same engine as lists/logs).
    #[must_use]
    pub const fn scroll(&self) -> &ScrollAreaState {
        &self.scroll
    }

    /// Last painted editor body, excluding label, gutter, and scrollbar.
    #[must_use]
    pub const fn body_area(&self) -> Rect {
        self.body
    }

    /// Hardware caret cell while editing; `None` when not editing or off-screen.
    #[must_use]
    pub const fn cursor_cell(&self) -> Option<Position> {
        self.hardware_cursor
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
            let content_h = match self.wrap {
                TextWrap::Soft => self.content_height.max(self.lines.len()),
                TextWrap::None => self.lines.len(),
            };
            let y = u16::try_from(crate::scroll::offset_for_track_position(
                content_h,
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
        if self.read_only || !self.accepts_input {
            return TextAreaOutcome::Ignored;
        }
        if text.is_empty() && self.selection_range().is_none() {
            return TextAreaOutcome::Ignored;
        }
        self.push_undo();
        let had_sel = self.delete_selection();
        let edits = self.insert_text_deltas(text);
        let changed = had_sel || edits.discard();
        if !changed {
            let _ = self.undo.pop();
        }
        self.finish_edit(changed)
    }

    /// Semantic navigation/cancel (chars still use [`Self::handle_key`]).
    pub fn handle_intent(&mut self, intent: crate::interaction::UiIntent) -> TextAreaOutcome {
        use crate::interaction::{NavigationMove, PageMove, UiIntent};
        if !self.accepts_input {
            return TextAreaOutcome::Ignored;
        }
        match intent {
            UiIntent::Cancel | UiIntent::Close => {
                if self.editing {
                    self.editing = false;
                    self.select_anchor = None;
                    TextAreaOutcome::Changed
                } else {
                    TextAreaOutcome::Cancelled
                }
            }
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

    /// Routes keyboard. Idle (focused, not editing): Enter/F2 begin edit,
    /// j/k and arrows scroll. Editing: Enter inserts a newline; Esc commits.
    pub fn handle_key(&mut self, key: KeyEvent) -> TextAreaOutcome {
        if !self.accepts_input || key.is_release() {
            return TextAreaOutcome::Ignored;
        }
        let is_press = key.is_press();
        let plain = key.modifiers.is_empty();
        if !self.editing || self.read_only {
            return match key.code {
                KeyCode::Enter if plain && !self.read_only && is_press => {
                    self.editing = true;
                    TextAreaOutcome::Changed
                }
                KeyCode::Up | KeyCode::Char('k') if plain => {
                    if self.scroll_by(0, -1) {
                        TextAreaOutcome::Changed
                    } else {
                        TextAreaOutcome::Ignored
                    }
                }
                KeyCode::Down | KeyCode::Char('j') if plain => {
                    if self.scroll_by(0, 1) {
                        TextAreaOutcome::Changed
                    } else {
                        TextAreaOutcome::Ignored
                    }
                }
                KeyCode::PageUp if plain => {
                    let step = -isize::try_from(self.viewport_height.max(1)).unwrap_or(isize::MAX);
                    if self.vertical(step) {
                        TextAreaOutcome::Changed
                    } else {
                        TextAreaOutcome::Ignored
                    }
                }
                KeyCode::PageDown if plain => {
                    let step = isize::try_from(self.viewport_height.max(1)).unwrap_or(isize::MAX);
                    if self.vertical(step) {
                        TextAreaOutcome::Changed
                    } else {
                        TextAreaOutcome::Ignored
                    }
                }
                _ => TextAreaOutcome::Ignored,
            };
        }
        if key.code == KeyCode::Esc {
            if !key.is_press()
                || !(key.modifiers.is_empty() || key.modifiers == KeyModifiers::SHIFT)
            {
                return TextAreaOutcome::Ignored;
            }
            // junie: Esc finishes editing and keeps the document.
            self.editing = false;
            self.select_anchor = None;
            return TextAreaOutcome::Changed;
        }

        let ctrl = key.modifiers.contains(KeyModifiers::CONTROL);
        let alt = key.modifiers.contains(KeyModifiers::ALT);
        let shift = key.modifiers.contains(KeyModifiers::SHIFT);

        // Host-facing clipboard/editor/fullscreen actions are physical
        // one-shots. Keep ordinary text and caret repeats available.
        let host_one_shot = (ctrl
            && !alt
            && matches!(
                key.code,
                KeyCode::Char('c' | 'C' | 'x' | 'X' | 'v' | 'V' | 'e' | 'E')
            ))
            || (ctrl && shift && !alt && matches!(key.code, KeyCode::Char('f' | 'F')));
        if !is_press && host_one_shot {
            return TextAreaOutcome::Ignored;
        }

        // Host hooks / clipboard / undo (Emacs-style default adapter)
        if ctrl && !alt {
            match key.code {
                KeyCode::Char('z' | 'Z') if !shift => return self.undo(),
                KeyCode::Char('z' | 'Z') if shift => return self.redo(),
                KeyCode::Char('y' | 'Y') => return self.redo(),
                KeyCode::Char('a' | 'A') => {
                    self.select_all();
                    return TextAreaOutcome::Changed;
                }
                KeyCode::Char('c' | 'C') => {
                    let text = self.selected_text().unwrap_or_else(|| self.text());
                    if text.is_empty() {
                        return TextAreaOutcome::Ignored;
                    }
                    return TextAreaOutcome::ClipboardCopy { text };
                }
                KeyCode::Char('x' | 'X') if !self.read_only => {
                    let Some(text) = self.selected_text() else {
                        return TextAreaOutcome::Ignored;
                    };
                    self.push_undo();
                    let _ = self.delete_selection();
                    self.measure();
                    self.reveal();
                    return TextAreaOutcome::ClipboardCut { text };
                }
                KeyCode::Char('v' | 'V') if !self.read_only => {
                    return TextAreaOutcome::ClipboardPasteRequest;
                }
                KeyCode::Char('e' | 'E') => return TextAreaOutcome::ExternalEditorRequested,
                KeyCode::Char('f' | 'F') if shift => {
                    return TextAreaOutcome::FullscreenRequested;
                }
                _ => {}
            }
        }

        // Word motion
        if (ctrl || alt) && matches!(key.code, KeyCode::Left | KeyCode::Right) {
            self.begin_select(shift);
            let changed = match key.code {
                KeyCode::Left => self.word_left(),
                KeyCode::Right => self.word_right(),
                _ => false,
            };
            if changed {
                self.reveal();
                return TextAreaOutcome::Changed;
            }
            return TextAreaOutcome::Ignored;
        }

        // Intent peel for Home/End/Page
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

        // Shift+Home/End select
        if shift && matches!(key.code, KeyCode::Home | KeyCode::End) {
            self.begin_select(true);
            let changed = self.edge(matches!(key.code, KeyCode::End));
            if changed {
                self.reveal();
                return TextAreaOutcome::Changed;
            }
            return TextAreaOutcome::Ignored;
        }

        let plain = key.modifiers.is_empty() || key.modifiers == KeyModifiers::SHIFT;
        if !plain && !matches!(key.code, KeyCode::Tab | KeyCode::BackTab) {
            return TextAreaOutcome::Ignored;
        }

        // Vertical / horizontal motion with optional select
        let vertical_delta = match key.code {
            KeyCode::Up => Some(-1),
            KeyCode::Down => Some(1),
            _ => None,
        };
        if let Some(delta) = vertical_delta {
            self.begin_select(shift);
            if self.vertical(delta) {
                self.reveal();
                return TextAreaOutcome::Changed;
            }
            return TextAreaOutcome::Ignored;
        }
        let motion = match key.code {
            KeyCode::Left => {
                self.begin_select(shift);
                Some(self.left())
            }
            KeyCode::Right => {
                self.begin_select(shift);
                Some(self.right())
            }
            KeyCode::Home => {
                self.begin_select(shift);
                Some(self.edge(false))
            }
            KeyCode::End => {
                self.begin_select(shift);
                Some(self.edge(true))
            }
            _ => None,
        };
        if let Some(changed) = motion {
            if changed {
                if !shift {
                    self.select_anchor = None;
                }
                self.reveal();
                return TextAreaOutcome::Changed;
            }
            return TextAreaOutcome::Ignored;
        }

        if self.read_only {
            return TextAreaOutcome::Ignored;
        }

        // Indent
        if matches!(key.code, KeyCode::Tab) && !shift {
            self.push_undo();
            let had_sel = self.delete_selection();
            let indent = self.indent.clone();
            let inserted = self.insert_text_deltas(&indent).discard();
            if !(had_sel || inserted) {
                let _ = self.undo.pop();
                return TextAreaOutcome::Ignored;
            }
            return self.finish_edit(true);
        }
        if matches!(key.code, KeyCode::BackTab) || (matches!(key.code, KeyCode::Tab) && shift) {
            return self.outdent_line();
        }

        let muting = matches!(
            key.code,
            KeyCode::Enter | KeyCode::Backspace | KeyCode::Delete | KeyCode::Char(_)
        );
        if !muting {
            return TextAreaOutcome::Ignored;
        }

        // Selection + Backspace/Delete: only remove selection.
        if matches!(key.code, KeyCode::Backspace | KeyCode::Delete)
            && self.selection_range().is_some()
        {
            self.push_undo();
            if self.delete_selection() {
                return self.finish_edit(true);
            }
            let _ = self.undo.pop();
            return TextAreaOutcome::Ignored;
        }

        self.push_undo();
        let had_sel = self.delete_selection();
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
        let changed = had_sel || changed;
        if !changed {
            let _ = self.undo.pop();
        }
        self.finish_edit(changed)
    }

    fn begin_select(&mut self, select: bool) {
        if select {
            if self.select_anchor.is_none() {
                self.select_anchor = Some(self.cursor);
            }
        } else {
            self.select_anchor = None;
        }
    }

    fn word_left(&mut self) -> bool {
        self.goal_column = None;
        let line = &self.lines[self.cursor.line];
        if self.cursor.byte > 0 {
            let next = edit_core::previous_word_boundary(line, self.cursor.byte);
            if next != self.cursor.byte {
                self.cursor.byte = next;
                return true;
            }
        }
        if self.cursor.line > 0 {
            self.cursor.line -= 1;
            self.cursor.byte = self.lines[self.cursor.line].len();
            true
        } else {
            false
        }
    }

    fn word_right(&mut self) -> bool {
        self.goal_column = None;
        let line = &self.lines[self.cursor.line];
        if self.cursor.byte < line.len() {
            let next = edit_core::next_word_boundary(line, self.cursor.byte);
            if next != self.cursor.byte {
                self.cursor.byte = next;
                return true;
            }
        }
        if self.cursor.line + 1 < self.lines.len() {
            self.cursor.line += 1;
            self.cursor.byte = 0;
            true
        } else {
            false
        }
    }

    fn outdent_line(&mut self) -> TextAreaOutcome {
        self.push_undo();
        let line = &mut self.lines[self.cursor.line];
        let indent = self.indent.clone();
        let removed = if line.starts_with(&indent) {
            line.drain(..indent.len());
            indent.len()
        } else if line.starts_with('\t') {
            line.remove(0);
            1
        } else if line.starts_with(' ') {
            let n = line
                .chars()
                .take_while(|c| *c == ' ')
                .take(indent.len())
                .count();
            line.drain(..n);
            n
        } else {
            0
        };
        if removed == 0 {
            let _ = self.undo.pop();
            return TextAreaOutcome::Ignored;
        }
        self.cursor.byte = self.cursor.byte.saturating_sub(removed);
        self.cursor.byte =
            edit_core::boundary_at_or_before(&self.lines[self.cursor.line], self.cursor.byte);
        self.finish_edit(true)
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
                    return TextAreaOutcome::Changed;
                }
                if self.accepts_input && self.body.contains(mouse.position) {
                    if let Some(cur) = self.cursor_from_position(mouse.position) {
                        if matches!(mouse.kind, MouseEventKind::Down(MouseButton::Left)) {
                            self.selecting_mouse = true;
                            self.select_anchor = Some(cur);
                            let _ = self.set_cursor(cur);
                            self.select_anchor = Some(cur);
                            return TextAreaOutcome::Changed;
                        }
                        if matches!(mouse.kind, MouseEventKind::Drag(MouseButton::Left))
                            && self.selecting_mouse
                        {
                            if self.select_anchor.is_none() {
                                self.select_anchor = Some(self.cursor);
                            }
                            self.selecting_mouse = true;
                            let before = self.cursor;
                            self.cursor = cur;
                            self.goal_column = None;
                            self.reveal();
                            if before != self.cursor {
                                return TextAreaOutcome::Changed;
                            }
                        }
                    }
                }
                TextAreaOutcome::Ignored
            }
            Event::Mouse(mouse) if matches!(mouse.kind, MouseEventKind::Up(MouseButton::Left)) => {
                self.selecting_mouse = false;
                if self.select_anchor == Some(self.cursor) {
                    self.select_anchor = None;
                }
                TextAreaOutcome::Ignored
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
        self.metrics_dirty = true;
        self.measure();
        self.reveal();
        TextAreaOutcome::Changed
    }
    fn measure(&mut self) {
        if self.metrics_dirty {
            self.max_width = self
                .lines
                .iter()
                .map(|line| display_cols(line))
                .max()
                .unwrap_or(0);
            self.metrics_dirty = false;
            // Soft-wrap height invalid when line widths change.
            self.soft_wrap_width = 0;
        }
        self.content_height = match self.wrap {
            TextWrap::None => self.lines.len(),
            TextWrap::Soft => {
                let w = self.viewport_width.max(1);
                if self.soft_wrap_width == w && self.content_height > 0 {
                    // Reuse cached visual row total for this width.
                    self.content_height
                } else {
                    let h = self
                        .lines
                        .iter()
                        .map(|line| {
                            let cols = display_cols(line).max(1);
                            cols.div_ceil(w).max(1)
                        })
                        .sum();
                    self.soft_wrap_width = w;
                    h
                }
            }
        };
        self.clamp_scroll();
    }
    fn sync_scroll_metrics(&mut self) {
        let h = match self.wrap {
            TextWrap::None => self.lines.len(),
            TextWrap::Soft => self.content_height.max(self.lines.len()),
        };
        let h = u16::try_from(h.min(usize::from(u16::MAX))).unwrap_or(u16::MAX);
        let content_w = match self.wrap {
            TextWrap::Soft => self.viewport_width.max(1),
            TextWrap::None => self.max_width,
        };
        let w = u16::try_from(content_w.min(usize::from(u16::MAX))).unwrap_or(u16::MAX);
        let vh = u16::try_from(self.viewport_height.min(usize::from(u16::MAX))).unwrap_or(1);
        let vw = u16::try_from(self.viewport_width.min(usize::from(u16::MAX))).unwrap_or(1);
        // Quiet content size so stream-like growth does not pause "follow" (N/A here).
        self.scroll.set_content_size(w, h);
        self.scroll.set_viewport(vw, vh.max(1));
    }

    /// Visual row index of caret (for soft wrap scroll).
    fn visual_row_of_cursor(&self) -> usize {
        match self.wrap {
            TextWrap::None => self.cursor.line,
            TextWrap::Soft => {
                let w = self.viewport_width.max(1);
                let mut row = 0usize;
                for (i, line) in self.lines.iter().enumerate() {
                    let cols = display_cols(line).max(1);
                    let rows = cols.div_ceil(w).max(1);
                    if i < self.cursor.line {
                        row = row.saturating_add(rows);
                    } else if i == self.cursor.line {
                        let col = display_cols(&line[..self.cursor.byte.min(line.len())]);
                        row = row.saturating_add(col / w);
                        break;
                    }
                }
                row
            }
        }
    }

    fn clamp_scroll(&mut self) {
        self.sync_scroll_metrics();
        self.scroll.clamp();
    }

    /// Keep caret in view like a native multiline editor (no accidental follow-pause).
    fn reveal(&mut self) {
        self.sync_scroll_metrics();
        if self.viewport_height > 0 {
            let caret_row = self.visual_row_of_cursor();
            let y = usize::from(self.scroll.offset_y());
            if caret_row < y {
                self.scroll
                    .set_offset_y_quiet(u16::try_from(caret_row).unwrap_or(u16::MAX));
            } else if caret_row >= y + self.viewport_height {
                self.scroll.set_offset_y_quiet(
                    u16::try_from(caret_row + 1 - self.viewport_height).unwrap_or(u16::MAX),
                );
            }
        }
        if matches!(self.wrap, TextWrap::None) {
            let col = display_cols(&self.lines[self.cursor.line][..self.cursor.byte]);
            let x = usize::from(self.scroll.offset_x());
            if col < x {
                self.scroll
                    .set_offset_x(u16::try_from(col).unwrap_or(u16::MAX));
            } else if self.viewport_width > 0 && col >= x + self.viewport_width {
                self.scroll
                    .set_offset_x(u16::try_from(col + 1 - self.viewport_width).unwrap_or(u16::MAX));
            }
        } else {
            self.scroll.set_offset_x(0);
        }
        self.scroll.clamp();
    }

    /// Place caret from body-local cell (mouse).
    pub fn cursor_from_position(&self, pos: Position) -> Option<TextCursor> {
        if self.body.is_empty() || !self.body.contains(pos) {
            return None;
        }
        let row = usize::from(pos.y.saturating_sub(self.body.y));
        // Body origin already excludes the line-number gutter.
        let col = usize::from(pos.x.saturating_sub(self.body.x));
        let first = usize::from(self.scroll.offset_y());
        match self.wrap {
            TextWrap::None => {
                let line = first
                    .saturating_add(row)
                    .min(self.lines.len().saturating_sub(1));
                let abs_col = col.saturating_add(usize::from(self.scroll.offset_x()));
                let byte = edit_core::byte_at_display_column(&self.lines[line], abs_col);
                Some(TextCursor { line, byte })
            }
            TextWrap::Soft => {
                let w = self.viewport_width.max(1);
                let mut visual = 0usize;
                let target = first.saturating_add(row);
                for (i, line) in self.lines.iter().enumerate() {
                    let cols = display_cols(line).max(1);
                    let rows = cols.div_ceil(w).max(1);
                    if visual.saturating_add(rows) > target {
                        let within = target.saturating_sub(visual);
                        let abs_col = within.saturating_mul(w).saturating_add(col);
                        let byte = edit_core::byte_at_display_column(line, abs_col);
                        return Some(TextCursor { line: i, byte });
                    }
                    visual = visual.saturating_add(rows);
                }
                let last = self.lines.len().saturating_sub(1);
                Some(TextCursor {
                    line: last,
                    byte: self.lines[last].len(),
                })
            }
        }
    }
}

/// Themed multi-line text editor.
#[derive(Debug, Clone, Copy)]
pub struct TextArea<'a> {
    system: &'a DesignSystem,
    title: Option<&'a str>,
    placeholder: Option<&'a str>,
    help: Option<&'a str>,
    error: Option<&'a str>,
    colorless: bool,
    line_numbers: bool,
    wrap: TextWrap,
    variant: TextAreaVariant,
    /// Visible text rows. Height is `rows + 2` (label + footer). `0` fills `area`.
    rows: u16,
}

impl<'a> TextArea<'a> {
    /// Creates an untitled editor.
    #[must_use]
    pub const fn new(system: &'a DesignSystem) -> Self {
        Self {
            system,
            title: None,
            placeholder: None,
            help: None,
            error: None,
            // Seeded from the system: a widget that defaults to false is
            // claiming the terminal has Unicode and colour before anyone
            // asked it. Builders below still force either way.
            colorless: system.mono(),
            line_numbers: false,
            wrap: TextWrap::None,
            variant: TextAreaVariant::Editor,
            rows: 0,
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

    /// Footer help (yields to [`Self::error`]).
    #[must_use]
    pub const fn help(mut self, help: &'a str) -> Self {
        self.help = Some(help);
        self
    }

    /// Footer error; paints `!` in the field and replaces help.
    #[must_use]
    pub const fn error(mut self, error: &'a str) -> Self {
        self.error = Some(error);
        self
    }

    /// Visible text rows. Painted height is `rows + 2`.
    #[must_use]
    pub const fn rows(mut self, rows: u16) -> Self {
        self.rows = rows;
        self
    }

    /// Label + body rows + footer.
    #[must_use]
    pub const fn height(&self) -> u16 {
        self.rows.saturating_add(2)
    }

    /// ASCII scrollbar / empty cues.
    #[must_use]
    /// Reduced-color caret/chrome.
    pub const fn colorless(mut self, colorless: bool) -> Self {
        self.colorless = colorless;
        self
    }

    /// Gutter line numbers.
    #[must_use]
    pub const fn line_numbers(mut self, on: bool) -> Self {
        self.line_numbers = on;
        self
    }

    /// Soft wrap policy (synced into state on paint).
    #[must_use]
    pub const fn wrap(mut self, wrap: TextWrap) -> Self {
        self.wrap = wrap;
        self
    }

    /// Soft wrap convenience.
    #[must_use]
    pub const fn soft_wrap(mut self) -> Self {
        self.wrap = TextWrap::Soft;
        self
    }

    /// Variant recipe.
    #[must_use]
    pub const fn variant(mut self, variant: TextAreaVariant) -> Self {
        self.variant = variant;
        self
    }

    /// Review / comment chrome.
    #[must_use]
    pub const fn review(mut self) -> Self {
        self.variant = TextAreaVariant::Review;
        self
    }

    /// Semantic registration for a11y / intent hosts.
    pub fn register_semantic<Id, Action>(
        &self,
        scene: &mut SemanticScene<Id, Action>,
        id: Id,
        area: Rect,
        state: &TextAreaState,
    ) where
        Id: Clone + PartialEq + std::fmt::Display,
        Action: Clone,
    {
        if area.is_empty() {
            return;
        }
        let _ = scene.register(
            SemanticNode::control(id, area)
                .role(SemanticRole::Input)
                .label(self.title.unwrap_or("text area"))
                .description(self.variant.id())
                .focusable(!state.is_read_only())
                .disabled(false)
                .state(SemanticState {
                    selected: state.accepts_input,
                    ..Default::default()
                }),
        );
    }
}

impl StatefulWidget for &TextArea<'_> {
    type State = TextAreaState;
    fn render(self, area: Rect, buffer: &mut Buffer, state: &mut Self::State) {
        state.wrap = self.wrap;
        state.hardware_cursor = None;
        state.vertical_scrollbar = None;
        state.horizontal_scrollbar = None;
        if area.is_empty() {
            state.body = Rect::default();
            return;
        }

        let theme = self.system.junie_theme();
        let focused = state.accepts_input;
        let editing = state.editing && focused && !state.read_only;
        let invalid = self.error.is_some();
        let visual = VisualState {
            focused,
            disabled: state.read_only,
            editing,
            error: invalid,
            ..VisualState::default()
        };
        let fs = theme.field_style(visual);
        let field_bg = fs.bg.unwrap_or(theme.field);
        let canvas = theme.canvas;

        // Label row (always reserved: height = rows + 2).
        let label_style = if state.read_only {
            theme.faint().bg(canvas)
        } else if matches!(self.variant, TextAreaVariant::Review) && !focused {
            theme.muted().bg(canvas)
        } else {
            theme.label(focused).bg(canvas)
        };
        if let Some(title) = self.title {
            let text = take_display_cols(title, usize::from(area.width.saturating_sub(2)));
            buffer.set_stringn(
                area.x.saturating_add(2),
                area.y,
                text,
                usize::from(area.width.saturating_sub(2)),
                label_style,
            );
        }

        let max_rows = area.height.saturating_sub(2);
        let rows = if self.rows == 0 {
            max_rows
        } else {
            self.rows.min(max_rows)
        };
        if rows == 0 {
            state.body = Rect::default();
            return;
        }

        let field = Rect::new(area.x, area.y.saturating_add(1), area.width, rows);
        buffer.set_style(field, fs);
        let gutter = self.system.gutter(visual, field_bg, false);
        for y in field.top()..field.bottom() {
            buffer.set_stringn(field.x, y, self.system.glyphs.selection_gutter(), 1, gutter);
        }

        let number_w = if self.line_numbers {
            let digits = state.lines.len().max(1).ilog10() as u16 + 1;
            digits.saturating_add(1).min(6)
        } else {
            0
        };
        // Two-cell inset (bar + space) plus optional line numbers; right 2
        // cells are scrollbar + pad, matching junie inner = width − 4.
        state.gutter_width = 2u16.saturating_add(number_w);
        let inner = Rect::new(
            field.x.saturating_add(2),
            field.y,
            field.width.saturating_sub(4),
            rows,
        );
        let text = Rect::new(
            inner.x.saturating_add(number_w),
            inner.y,
            inner.width.saturating_sub(number_w),
            inner.height,
        );
        state.body = text;
        state.viewport_width = usize::from(text.width);
        state.viewport_height = usize::from(text.height);
        state.measure();
        // Junie only follows the cursor while editing. Idle view stays put
        // so a committed document is not scrolled to EOF.
        if editing {
            state.reveal();
        }
        if text.is_empty() {
            paint_textarea_footer(self, area, field, state, buffer, theme, editing);
            return;
        }

        if number_w > 0 {
            let first = usize::from(state.scroll.offset_y());
            let nstyle = theme.faint().bg(field_bg);
            for row in 0..state.viewport_height {
                let line_no = first.saturating_add(row).saturating_add(1);
                if line_no > state.lines.len() && matches!(state.wrap, TextWrap::None) {
                    break;
                }
                let label = format!(
                    "{line_no:>width$}",
                    width = usize::from(number_w.saturating_sub(1))
                );
                buffer.set_stringn(
                    inner.x,
                    text.y.saturating_add(u16::try_from(row).unwrap_or(0)),
                    take_display_cols(&label, usize::from(number_w)),
                    usize::from(number_w),
                    nstyle,
                );
            }
        }

        let first = usize::from(state.scroll.offset_y());
        let text_style = fs;
        let placeholder_style = theme.placeholder(visual);
        let empty_doc = state.lines.len() == 1 && state.lines[0].is_empty();

        match state.wrap {
            TextWrap::None => {
                let last = (first + state.viewport_height).min(state.lines.len());
                for (painted, line) in state.lines[first..last].iter().enumerate() {
                    let y = text.y + u16::try_from(painted).unwrap_or(u16::MAX);
                    let offset_x = usize::from(state.scroll.offset_x());
                    if offset_x > 0 && !line.is_empty() {
                        buffer.set_stringn(text.x, y, "…", 1, fs.fg(theme.text_muted));
                    }
                    display_cols_slice_into(
                        line,
                        offset_x,
                        state.viewport_width,
                        &mut state.scratch,
                    );
                    if empty_doc
                        && painted == 0
                        && let Some(placeholder) = self.placeholder
                    {
                        write_placeholder(
                            &mut state.scratch,
                            placeholder,
                            state.viewport_width,
                            self.system.glyphs.ellipsis(),
                        );
                    }
                    let style = if empty_doc {
                        placeholder_style
                    } else {
                        text_style
                    };
                    buffer.set_stringn(text.x, y, &state.scratch, state.viewport_width, style);
                    if display_cols(line).saturating_sub(offset_x) > state.viewport_width
                        && text.width > 0
                    {
                        buffer.set_stringn(
                            text.right().saturating_sub(1),
                            y,
                            "…",
                            1,
                            fs.fg(theme.text_muted),
                        );
                    }
                    if let Some((a, b)) = state.selection_range() {
                        paint_selection_line(
                            buffer,
                            text,
                            line,
                            first + painted,
                            SelectionWindow {
                                a,
                                b,
                                offset_x,
                                viewport_width: state.viewport_width,
                            },
                            y,
                            self.system.selected_text(),
                        );
                    }
                }
            }
            TextWrap::Soft => {
                let w = state.viewport_width.max(1);
                let mut visual = 0usize;
                let mut painted = 0usize;
                let sel = state.selection_range();
                for (line_idx, line) in state.lines.iter().enumerate() {
                    let cols = display_cols(line).max(1);
                    let wrap_rows = cols.div_ceil(w).max(1);
                    for r in 0..wrap_rows {
                        if visual + r < first {
                            continue;
                        }
                        if painted >= state.viewport_height {
                            break;
                        }
                        let start_col = r.saturating_mul(w);
                        display_cols_slice_into(line, start_col, w, &mut state.scratch);
                        if empty_doc
                            && r == 0
                            && let Some(placeholder) = self.placeholder
                        {
                            write_placeholder(
                                &mut state.scratch,
                                placeholder,
                                w,
                                self.system.glyphs.ellipsis(),
                            );
                        }
                        let y = text.y + u16::try_from(painted).unwrap_or(0);
                        buffer.set_stringn(
                            text.x,
                            y,
                            &state.scratch,
                            w,
                            if empty_doc {
                                placeholder_style
                            } else {
                                text_style
                            },
                        );
                        if let Some((a, b)) = sel
                            && line_idx >= a.line
                            && line_idx <= b.line
                        {
                            buffer.set_style(
                                Rect::new(text.x, y, text.width.min(w as u16), 1),
                                self.system.selected_text(),
                            );
                        }
                        painted += 1;
                    }
                    visual = visual.saturating_add(wrap_rows);
                    if painted >= state.viewport_height {
                        break;
                    }
                }
            }
        }

        // Current line: border-strong underline (not accent — that is
        // single-line editing). Hardware cursor; no reverse cell caret.
        if editing {
            let (cy, cx_off) = match state.wrap {
                TextWrap::None => {
                    let col = display_cols(&state.lines[state.cursor.line][..state.cursor.byte])
                        .saturating_sub(usize::from(state.scroll.offset_x()));
                    let y = text.y
                        + u16::try_from(state.cursor.line.saturating_sub(first))
                            .unwrap_or(u16::MAX);
                    (y, col)
                }
                TextWrap::Soft => {
                    let crow = state.visual_row_of_cursor();
                    let line = &state.lines[state.cursor.line];
                    let col = display_cols(&line[..state.cursor.byte.min(line.len())]);
                    let w = state.viewport_width.max(1);
                    let y = text.y + u16::try_from(crow.saturating_sub(first)).unwrap_or(0);
                    (y, col % w)
                }
            };
            if cy >= text.y && cy < text.bottom() {
                underline_row(buffer, inner, cy, theme.border_strong);
                let x = text
                    .x
                    .saturating_add(u16::try_from(cx_off).unwrap_or(u16::MAX))
                    .min(text.right());
                state.hardware_cursor = Some(Position { x, y: cy });
            }
        }

        if invalid {
            buffer.set_stringn(
                field.right().saturating_sub(2),
                field.y,
                "!",
                1,
                fs.fg(theme.error).add_modifier(Modifier::BOLD),
            );
        }

        let content_h = match state.wrap {
            TextWrap::Soft => state.content_height.max(state.lines.len()),
            TextWrap::None => state.lines.len(),
        };
        let sb = Rect::new(field.right().saturating_sub(1), field.y, 1, rows);
        state.vertical_scrollbar = Some(sb);
        crate::scroll::paint_overflow_scrollbar(
            buffer,
            sb,
            content_h,
            state.viewport_height,
            state.scroll.offset_y(),
            focused,
            self.system,
        );

        paint_textarea_footer(self, area, field, state, buffer, theme, editing);
        let _ = self.colorless;
    }
}

fn write_placeholder(scratch: &mut String, placeholder: &str, width: usize, ellipsis: &str) {
    scratch.clear();
    scratch.push_str(truncate_cols(placeholder, width, ellipsis).as_ref());
}

fn underline_row(buffer: &mut Buffer, inner: Rect, y: u16, color: ratatui_core::style::Color) {
    for x in inner.x..inner.right() {
        if let Some(cell) = buffer.cell_mut((x, y)) {
            cell.set_style(
                cell.style()
                    .add_modifier(Modifier::UNDERLINED)
                    .underline_color(color),
            );
        }
    }
}

fn paint_textarea_footer(
    widget: &TextArea<'_>,
    area: Rect,
    field: Rect,
    state: &mut TextAreaState,
    buffer: &mut Buffer,
    theme: crate::style::JunieTheme,
    editing: bool,
) {
    let fy = field.bottom();
    if fy >= area.bottom() {
        return;
    }
    let n = state.lines.len().max(1);
    let first = usize::from(state.scroll.offset_y());
    let last = (first + state.viewport_height.max(1)).min(n);
    state.scratch.clear();
    if editing {
        use std::fmt::Write as _;
        let _ = write!(
            state.scratch,
            "ln {}/{n}",
            state.cursor.line.saturating_add(1)
        );
    } else if crate::scroll::is_scrollable(n, state.viewport_height.max(1)) {
        use std::fmt::Write as _;
        let _ = write!(state.scratch, "{}–{last} of {n}", first.saturating_add(1));
    }
    let pos_w = if state.scratch.is_empty() {
        0
    } else {
        display_cols(&state.scratch) as u16 + 3
    };
    let msg_w = usize::from(area.width.saturating_sub(2 + pos_w));
    if let Some(err) = widget.error {
        buffer.set_stringn(
            area.x.saturating_add(2),
            fy,
            crate::text::truncate_cols(err, msg_w, widget.system.glyphs.ellipsis()).as_ref(),
            msg_w,
            theme.error_fg().bg(theme.canvas),
        );
    } else if let Some(help) = widget.help {
        buffer.set_stringn(
            area.x.saturating_add(2),
            fy,
            crate::text::truncate_cols(help, msg_w, widget.system.glyphs.ellipsis()).as_ref(),
            msg_w,
            theme.muted().bg(theme.canvas),
        );
    }
    if !state.scratch.is_empty() {
        let w = display_cols(&state.scratch);
        let px = area
            .right()
            .saturating_sub(u16::try_from(w).unwrap_or(0).saturating_add(1));
        buffer.set_stringn(px, fy, &state.scratch, w, theme.faint().bg(theme.canvas));
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct SelectionWindow {
    a: TextCursor,
    b: TextCursor,
    offset_x: usize,
    viewport_width: usize,
}

fn paint_selection_line(
    buffer: &mut Buffer,
    body: Rect,
    line: &str,
    line_idx: usize,
    window: SelectionWindow,
    y: u16,
    cursor_style: Style,
) {
    if line_idx < window.a.line || line_idx > window.b.line {
        return;
    }
    let start_byte = if line_idx == window.a.line {
        window.a.byte
    } else {
        0
    };
    let end_byte = if line_idx == window.b.line {
        window.b.byte.min(line.len())
    } else {
        line.len()
    };
    if start_byte >= end_byte {
        return;
    }
    let start_col = display_cols(&line[..start_byte]).saturating_sub(window.offset_x);
    let end_col = display_cols(&line[..end_byte]).saturating_sub(window.offset_x);
    let sx = body
        .x
        .saturating_add(u16::try_from(start_col.min(window.viewport_width)).unwrap_or(0));
    let ex = body
        .x
        .saturating_add(u16::try_from(end_col.min(window.viewport_width)).unwrap_or(0))
        .min(body.right());
    if ex > sx {
        // D8: selected text is one explicit pair, applied as a whole —
        // never a reversal of whatever the cell already carried.
        buffer.set_style(Rect::new(sx, y, ex.saturating_sub(sx), 1), cursor_style);
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
    use crate::input::{KeyEventKind, KeyEventState};
    use crate::style::RolePalette;
    use crate::widgets::tests::mouse;
    #[test]
    fn normalized_editing_and_goal_column_contract() {
        let mut state = TextAreaState::new("ab🧪\r\nx\r12345");
        state.set_accepts_input(true);
        state.set_editing(true);
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
        state.set_editing(true);
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
            state.set_editing(true);
            state.set_editing(true);
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
        state.set_editing(true);
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
        state.set_editing(true);
        state.viewport_width = 24;
        state.viewport_height = 6;
        state.sync_scroll_metrics();
        assert!(state.set_cursor(TextCursor { line: 30, byte: 0 }));
        state.reveal();
        let y = state.scroll.offset_y() as usize;
        assert!(y <= 30);
        assert!(y + 6 > 30, "caret line must sit in viewport");
        // Wheel down via ScrollArea steps
        assert!(state.scroll_by(0, 3));
        assert!(state.scroll.offset_y() > y as u16);
    }

    #[test]
    fn scrollbars_stay_inside_field_and_own_press_drag_geometry() {
        let theme = RolePalette::default();
        let system = crate::style::DesignSystem::new(theme.clone());
        let mut state = TextAreaState::new("wide content beyond viewport\none\ntwo\nthree\nfour");
        state.set_accepts_input(true);
        state.set_editing(true);
        assert!(state.set_cursor(c(0, 0)));
        let area = Rect::new(2, 2, 14, 8);
        let mut buffer = Buffer::empty(Rect::new(0, 0, 20, 12));
        (&TextArea::new(&system).title("Edit").rows(3)).render(area, &mut buffer, &mut state);
        // junie: ▎ on every body row; scrollbar occupies the field's last column.
        let field_y = area.y + 1;
        assert_eq!(buffer[(area.x, field_y)].symbol(), "▎");
        let vertical = state.vertical_scrollbar.unwrap();
        assert_eq!(vertical.x, area.right() - 1);
        assert!(vertical.y >= field_y);
        assert!(vertical.bottom() <= area.bottom());
        let outcome = state.handle_event(Event::Mouse(mouse(
            MouseEventKind::Down(MouseButton::Left),
            vertical.x,
            vertical.bottom() - 1,
        )));
        assert_eq!(outcome, TextAreaOutcome::Changed);
        assert!(state.scroll.offset_y() > 0);
        assert_eq!(
            state.handle_event(Event::Mouse(mouse(
                MouseEventKind::Drag(MouseButton::Left),
                vertical.x,
                vertical.bottom() - 1
            ))),
            TextAreaOutcome::Ignored
        );
        assert_eq!(
            state.handle_event(Event::Mouse(mouse(
                MouseEventKind::Drag(MouseButton::Left),
                0,
                0
            ))),
            TextAreaOutcome::Ignored
        );
    }

    #[test]
    fn height_is_rows_plus_two_and_body_fills_between_label_and_footer() {
        let system = crate::style::DesignSystem::default();
        let mut state = TextAreaState::new("Explain this module");
        state.set_accepts_input(true);
        state.set_editing(true);
        let area = Rect::new(0, 0, 22, 5);
        let mut buffer = Buffer::empty(area);

        let widget = TextArea::new(&system).rows(3);
        assert_eq!(widget.height(), 5);
        (&widget).render(area, &mut buffer, &mut state);

        // area 5 = label + 3 body rows + footer
        assert_eq!(state.body.height, 3);
        assert!(state.horizontal_scrollbar.is_none());
        let painted: String = (state.body.x..state.body.right())
            .map(|x| buffer[(x, state.body.y)].symbol())
            .collect();
        assert!(painted.chars().any(|ch| !ch.is_whitespace()), "{painted:?}");
    }

    #[test]
    fn accepts_input_gate_and_read_only() {
        let mut state = TextAreaState::new("ab");
        assert_eq!(
            state.handle_key(KeyEvent::new(KeyCode::Char('x'), KeyModifiers::NONE)),
            TextAreaOutcome::Ignored
        );
        state.set_accepts_input(true);
        state.set_editing(true);
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
    fn escape_requires_a_press_and_supported_modifiers() {
        struct Case {
            name: &'static str,
            kind: KeyEventKind,
            modifiers: KeyModifiers,
            expected_outcome: TextAreaOutcome,
            expected_editing: bool,
            expected_anchor: Option<TextCursor>,
        }

        let cases = [
            Case {
                name: "bare press",
                kind: KeyEventKind::Press,
                modifiers: KeyModifiers::NONE,
                expected_outcome: TextAreaOutcome::Changed,
                expected_editing: false,
                expected_anchor: None,
            },
            Case {
                name: "shift press",
                kind: KeyEventKind::Press,
                modifiers: KeyModifiers::SHIFT,
                expected_outcome: TextAreaOutcome::Changed,
                expected_editing: false,
                expected_anchor: None,
            },
            Case {
                name: "repeat",
                kind: KeyEventKind::Repeat,
                modifiers: KeyModifiers::NONE,
                expected_outcome: TextAreaOutcome::Ignored,
                expected_editing: true,
                expected_anchor: Some(c(0, 0)),
            },
            Case {
                name: "release",
                kind: KeyEventKind::Release,
                modifiers: KeyModifiers::NONE,
                expected_outcome: TextAreaOutcome::Ignored,
                expected_editing: true,
                expected_anchor: Some(c(0, 0)),
            },
            Case {
                name: "control press",
                kind: KeyEventKind::Press,
                modifiers: KeyModifiers::CONTROL,
                expected_outcome: TextAreaOutcome::Ignored,
                expected_editing: true,
                expected_anchor: Some(c(0, 0)),
            },
            Case {
                name: "alt press",
                kind: KeyEventKind::Press,
                modifiers: KeyModifiers::ALT,
                expected_outcome: TextAreaOutcome::Ignored,
                expected_editing: true,
                expected_anchor: Some(c(0, 0)),
            },
        ];

        for case in cases {
            let mut state = TextAreaState::new("one\ntwo");
            state.set_accepts_input(true);
            state.set_editing(true);
            state.select_all();
            let initial_cursor = state.cursor();
            let initial_text = state.text();
            let initial_range = state.selection_range();

            let outcome = state.handle_key(KeyEvent {
                code: KeyCode::Esc,
                modifiers: case.modifiers,
                kind: case.kind,
                state: KeyEventState::NONE,
            });

            assert_eq!(outcome, case.expected_outcome, "{} outcome", case.name);
            assert_eq!(
                state.is_editing(),
                case.expected_editing,
                "{} editing",
                case.name
            );
            assert_eq!(state.text(), initial_text, "{} text", case.name);
            assert_eq!(state.cursor(), initial_cursor, "{} cursor", case.name);
            assert_eq!(
                state.selection_anchor(),
                case.expected_anchor,
                "{} selection anchor",
                case.name
            );
            if case.expected_editing {
                assert_eq!(
                    state.selection_range(),
                    initial_range,
                    "{} selection",
                    case.name
                );
            } else {
                assert!(
                    state.selection_range().is_none(),
                    "{} selection cleared",
                    case.name
                );
            }
        }
    }

    #[test]
    fn repeated_escape_does_not_finish_editing() {
        let mut state = TextAreaState::new("one\ntwo");
        state.set_accepts_input(true);
        state.set_editing(true);
        let mut repeat = KeyEvent::new(KeyCode::Esc, KeyModifiers::NONE);
        repeat.kind = KeyEventKind::Repeat;

        assert_eq!(state.handle_key(repeat), TextAreaOutcome::Ignored);
        assert!(state.is_editing());
        assert_eq!(state.text(), "one\ntwo");

        let mut release = KeyEvent::new(KeyCode::Esc, KeyModifiers::NONE);
        release.kind = KeyEventKind::Release;
        assert_eq!(state.handle_key(release), TextAreaOutcome::Ignored);
        assert!(state.is_editing());
    }

    #[test]
    fn measurement_invalidates_only_on_edits_and_tiny_control_input_is_safe() {
        let mut state = TextAreaState::new("ab");
        state.set_accepts_input(true);
        state.set_editing(true);
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

    #[test]
    fn selection_shift_and_select_all_and_delete() {
        let mut state = TextAreaState::new("hello world");
        state.set_accepts_input(true);
        state.set_editing(true);
        assert!(state.set_cursor(c(0, 0)));
        assert_eq!(
            state.handle_key(KeyEvent::new(KeyCode::Right, KeyModifiers::SHIFT)),
            TextAreaOutcome::Changed
        );
        assert_eq!(
            state.handle_key(KeyEvent::new(KeyCode::Right, KeyModifiers::SHIFT)),
            TextAreaOutcome::Changed
        );
        assert_eq!(state.selected_text().as_deref(), Some("he"));
        assert_eq!(
            state.handle_key(KeyEvent::new(KeyCode::Backspace, KeyModifiers::NONE)),
            TextAreaOutcome::Changed
        );
        assert_eq!(state.text(), "llo world");
        assert!(state.selection_range().is_none());
        state.select_all();
        assert_eq!(state.selected_text().as_deref(), Some("llo world"));
    }

    #[test]
    fn undo_redo_and_clipboard_outcomes() {
        let mut state = TextAreaState::new("ab");
        state.set_accepts_input(true);
        state.set_editing(true);
        assert_eq!(
            state.handle_key(KeyEvent::new(KeyCode::Char('c'), KeyModifiers::NONE)),
            TextAreaOutcome::Changed
        );
        assert_eq!(state.text(), "abc");
        assert_eq!(
            state.handle_key(KeyEvent::new(KeyCode::Char('z'), KeyModifiers::CONTROL)),
            TextAreaOutcome::Changed
        );
        assert_eq!(state.text(), "ab");
        assert_eq!(
            state.handle_key(KeyEvent::new(KeyCode::Char('y'), KeyModifiers::CONTROL)),
            TextAreaOutcome::Changed
        );
        assert_eq!(state.text(), "abc");
        state.select_all();
        assert_eq!(
            state.handle_key(KeyEvent::new(KeyCode::Char('c'), KeyModifiers::CONTROL)),
            TextAreaOutcome::ClipboardCopy { text: "abc".into() }
        );
        assert_eq!(
            state.handle_key(KeyEvent::new(KeyCode::Char('x'), KeyModifiers::CONTROL)),
            TextAreaOutcome::ClipboardCut { text: "abc".into() }
        );
        assert_eq!(state.text(), "");
        assert_eq!(
            state.handle_key(KeyEvent::new(KeyCode::Char('v'), KeyModifiers::CONTROL)),
            TextAreaOutcome::ClipboardPasteRequest
        );
        assert_eq!(
            state.handle_key(KeyEvent::new(KeyCode::Char('e'), KeyModifiers::CONTROL)),
            TextAreaOutcome::ExternalEditorRequested
        );
        assert_eq!(
            state.handle_key(KeyEvent::new(
                KeyCode::Char('f'),
                KeyModifiers::CONTROL | KeyModifiers::SHIFT
            )),
            TextAreaOutcome::FullscreenRequested
        );
    }

    #[test]
    fn repeated_idle_entry_and_host_actions_are_ignored() {
        let repeat = |code, modifiers| {
            let mut key = KeyEvent::new(code, modifiers);
            key.kind = KeyEventKind::Repeat;
            key
        };

        let mut idle = TextAreaState::new("ab");
        idle.set_accepts_input(true);
        let before = idle.clone();
        assert_eq!(
            idle.handle_key(repeat(KeyCode::Enter, KeyModifiers::NONE)),
            TextAreaOutcome::Ignored
        );
        assert_eq!(idle, before, "repeated idle Enter must not begin editing");

        for (code, modifiers) in [
            (KeyCode::Char('c'), KeyModifiers::CONTROL),
            (KeyCode::Char('x'), KeyModifiers::CONTROL),
            (KeyCode::Char('v'), KeyModifiers::CONTROL),
            (KeyCode::Char('e'), KeyModifiers::CONTROL),
            (
                KeyCode::Char('f'),
                KeyModifiers::CONTROL | KeyModifiers::SHIFT,
            ),
        ] {
            let mut state = TextAreaState::new("abc");
            state.set_accepts_input(true);
            state.set_editing(true);
            state.select_all();
            let before = state.clone();
            assert_eq!(
                state.handle_key(repeat(code, modifiers)),
                TextAreaOutcome::Ignored,
                "repeat of {code:?} with {modifiers:?} must be ignored"
            );
            assert_eq!(state, before);
        }
    }

    #[test]
    fn word_motion_and_indent() {
        let mut state = TextAreaState::new("foo bar");
        state.set_accepts_input(true);
        state.set_editing(true);
        assert!(state.set_cursor(c(0, 0)));
        assert_eq!(
            state.handle_key(KeyEvent::new(KeyCode::Right, KeyModifiers::CONTROL)),
            TextAreaOutcome::Changed
        );
        assert_eq!(state.cursor().byte, 3);
        assert_eq!(
            state.handle_key(KeyEvent::new(KeyCode::Right, KeyModifiers::CONTROL)),
            TextAreaOutcome::Changed
        );
        assert_eq!(state.cursor().byte, 7);
        assert_eq!(
            state.handle_key(KeyEvent::new(KeyCode::Left, KeyModifiers::ALT)),
            TextAreaOutcome::Changed
        );
        assert_eq!(state.cursor().byte, 4);
        assert!(state.set_cursor(c(0, 0)));
        assert_eq!(
            state.handle_key(KeyEvent::new(KeyCode::Tab, KeyModifiers::NONE)),
            TextAreaOutcome::Changed
        );
        assert_eq!(state.text(), "  foo bar");
        assert_eq!(
            state.handle_key(KeyEvent::new(KeyCode::BackTab, KeyModifiers::NONE)),
            TextAreaOutcome::Changed
        );
        assert_eq!(state.text(), "foo bar");
    }

    #[test]
    fn soft_wrap_preserves_caret_row_on_reflow() {
        let mut state = TextAreaState::new("abcdefghijklmnopqrstuvwxyz");
        state.set_accepts_input(true);
        state.set_editing(true);
        state.set_wrap(TextWrap::Soft);
        state.viewport_width = 10;
        state.viewport_height = 4;
        state.measure();
        assert!(state.set_cursor(c(0, 20)));
        state.reveal();
        let row = state.visual_row_of_cursor();
        assert!(row >= 1, "caret should wrap to later visual row");
        // Resize: reflow keeps absolute caret; scroll re-reveals
        state.viewport_width = 5;
        state.measure();
        state.reveal();
        assert_eq!(state.cursor().byte, 20);
        assert!(state.visual_row_of_cursor() >= row);
    }

    #[test]
    fn line_numbers_gutter_and_review_paint() {
        let system = crate::style::DesignSystem::default();
        let mut state = TextAreaState::new("a\nb\nc");
        state.set_accepts_input(true);
        state.set_editing(true);
        let area = Rect::new(0, 0, 24, 8);
        let mut buffer = Buffer::empty(area);
        (&TextArea::new(&system)
            .title("Notes")
            .line_numbers(true)
            .review())
            .render(area, &mut buffer, &mut state);
        assert!(state.gutter_width >= 2);
        assert!(!state.body.is_empty());
        // junie: ▎ at the field origin; line numbers sit in the two-cell inset.
        assert_eq!(buffer[(area.x, state.body.y)].symbol(), "▎");
        let number_cell = &buffer[(area.x + 2, state.body.y)];
        assert!(
            number_cell.symbol().chars().any(|c| c.is_ascii_digit())
                || !number_cell.symbol().trim().is_empty(),
            "expected line number gutter cell, got {:?}",
            number_cell.symbol()
        );
    }

    #[test]
    fn large_document_key_nav_stays_bounded() {
        let text = (0..2_000)
            .map(|i| format!("line {i:04} body"))
            .collect::<Vec<_>>()
            .join("\n");
        let mut state = TextAreaState::new(text);
        state.set_accepts_input(true);
        state.set_editing(true);
        state.viewport_width = 40;
        state.viewport_height = 12;
        state.sync_scroll_metrics();
        assert!(state.set_cursor(c(1_500, 0)));
        state.reveal();
        for _ in 0..50 {
            let _ = state.handle_key(KeyEvent::new(KeyCode::Down, KeyModifiers::NONE));
        }
        assert!(state.cursor().line >= 1_500);
        assert!(usize::from(state.scroll.offset_y()) + state.viewport_height > state.cursor().line);
    }

    #[test]
    fn unicode_fuzz_random_ops_keep_boundary() {
        let samples = [
            "a\n東京",
            "e\u{301}\ncafe",
            "👩‍🔬 line",
            "a\u{200d}b\nx",
            "café\n\nend",
        ];
        let keys = [
            KeyEvent::new(KeyCode::Char('z'), KeyModifiers::NONE),
            KeyEvent::new(KeyCode::Char('あ'), KeyModifiers::NONE),
            KeyEvent::new(KeyCode::Backspace, KeyModifiers::NONE),
            KeyEvent::new(KeyCode::Delete, KeyModifiers::NONE),
            KeyEvent::new(KeyCode::Left, KeyModifiers::NONE),
            KeyEvent::new(KeyCode::Right, KeyModifiers::SHIFT),
            KeyEvent::new(KeyCode::Left, KeyModifiers::CONTROL),
            KeyEvent::new(KeyCode::Right, KeyModifiers::ALT),
            KeyEvent::new(KeyCode::Home, KeyModifiers::NONE),
            KeyEvent::new(KeyCode::End, KeyModifiers::SHIFT),
            KeyEvent::new(KeyCode::Enter, KeyModifiers::NONE),
            KeyEvent::new(KeyCode::Up, KeyModifiers::NONE),
            KeyEvent::new(KeyCode::Down, KeyModifiers::NONE),
            KeyEvent::new(KeyCode::Char('a'), KeyModifiers::CONTROL),
            KeyEvent::new(KeyCode::Tab, KeyModifiers::NONE),
        ];
        for seed in samples {
            let mut state = TextAreaState::new(seed);
            state.set_accepts_input(true);
            state.set_editing(true);
            state.set_editing(true);
            state.viewport_width = 12;
            state.viewport_height = 4;
            for (i, key) in keys.iter().cycle().take(48).enumerate() {
                let _ = state.handle_key(*key);
                let line = &state.lines[state.cursor.line];
                assert!(
                    edit_core::is_boundary(line, state.cursor.byte),
                    "seed={seed:?} step={i} cursor={:?}",
                    state.cursor
                );
            }
        }
    }

    #[test]
    fn idle_jk_scrolls_without_moving_caret() {
        let system = crate::style::DesignSystem::junie();
        let mut state = TextAreaState::new("a\nb\nc\nd\ne\nf\ng\nh");
        state.set_accepts_input(true);
        let area = Rect::new(0, 0, 24, 5);
        let mut buffer = Buffer::empty(area);
        (&TextArea::new(&system).rows(2)).render(area, &mut buffer, &mut state);
        let caret = state.cursor();
        assert_eq!(
            state.handle_key(KeyEvent::new(KeyCode::Char('j'), KeyModifiers::NONE)),
            TextAreaOutcome::Changed
        );
        assert_eq!(state.cursor(), caret, "idle j must not move the caret");
        assert!(!state.is_editing());
    }

    #[test]
    fn handle_intent_esc_commits_edit() {
        use crate::interaction::UiIntent;
        let mut state = TextAreaState::new("ab");
        state.set_accepts_input(true);
        state.set_editing(true);
        assert_eq!(
            state.handle_intent(UiIntent::Cancel),
            TextAreaOutcome::Changed
        );
        assert!(!state.is_editing());
        assert_eq!(state.text(), "ab");
    }

    #[test]
    fn enter_begins_edit_when_idle_without_inserting() {
        let mut state = TextAreaState::new("ab");
        state.set_accepts_input(true);
        assert!(!state.is_editing());
        assert_eq!(
            state.handle_key(KeyEvent::new(KeyCode::Enter, KeyModifiers::NONE)),
            TextAreaOutcome::Changed
        );
        assert!(state.is_editing());
        assert_eq!(state.text(), "ab");
        assert_eq!(
            state.handle_key(KeyEvent::new(KeyCode::Enter, KeyModifiers::NONE)),
            TextAreaOutcome::Changed
        );
        assert_eq!(state.text(), "ab\n");
    }

    #[test]
    fn semantic_registration() {
        let system = crate::style::DesignSystem::default();
        let widget = TextArea::new(&system).title("Notes").review();
        let state = TextAreaState::new("x");
        let mut scene = SemanticScene::<&str, ()>::default();
        widget.register_semantic(&mut scene, "ta", Rect::new(0, 0, 20, 5), &state);
        assert!(scene.get(&"ta").is_some());
    }

    #[test]
    fn editing_uses_field_plane_border_strong_underline_and_hardware_cursor() {
        let system = crate::style::DesignSystem::junie();
        let theme = system.junie_theme();
        let mut state = TextAreaState::new("hello\nworld");
        state.set_accepts_input(true);
        state.set_editing(true);
        assert!(state.set_cursor(c(0, 2)));
        let area = Rect::new(0, 0, 28, 6);
        let mut buffer = Buffer::empty(area);
        (&TextArea::new(&system)
            .title("Notes")
            .help("Enter inserts a newline")
            .rows(3))
            .render(area, &mut buffer, &mut state);

        assert_eq!(TextArea::new(&system).rows(3).height(), 5);
        let field_y = area.y + 1;
        let cell = &buffer[(area.x + 4, field_y)];
        assert_eq!(cell.bg, theme.field, "field plane is #1e1e22");
        assert!(
            cell.style().add_modifier.contains(Modifier::UNDERLINED),
            "current line is underlined"
        );
        assert_eq!(cell.underline_color, theme.border_strong);
        let cursor = state.cursor_cell().expect("hardware caret while editing");
        assert_eq!(cursor.y, field_y);
        assert_eq!(
            state.handle_key(KeyEvent::new(KeyCode::Esc, KeyModifiers::NONE)),
            TextAreaOutcome::Changed
        );
        assert!(!state.is_editing());
        assert_eq!(state.text(), "hello\nworld");
    }

    #[test]
    fn enter_inserts_newline_while_editing() {
        let mut state = TextAreaState::new("ab");
        state.set_accepts_input(true);
        state.set_editing(true);
        assert!(state.set_cursor(c(0, 1)));
        assert_eq!(
            state.handle_key(KeyEvent::new(KeyCode::Enter, KeyModifiers::NONE)),
            TextAreaOutcome::Changed
        );
        assert_eq!(state.text(), "a\nb");
    }

    #[test]
    fn overflow_placeholder_uses_ellipsis_not_hard_clip() {
        let system = crate::style::DesignSystem::junie();
        let mut state = TextAreaState::new("");
        let area = Rect::new(0, 0, 20, 3);
        let mut buffer = Buffer::empty(area);
        (&TextArea::new(&system)
            .placeholder("What should Junie do, and what does done look like?")
            .rows(1))
            .render(area, &mut buffer, &mut state);
        let y = area.y + 1;
        let line: String = (0..area.width)
            .map(|x| buffer[(x, y)].symbol().to_string())
            .collect();
        assert!(
            line.contains(system.glyphs.ellipsis()),
            "overflow placeholder must mark the cut, got {line:?}"
        );
        assert!(
            !line.contains("look"),
            "overflow placeholder must not hard-clip the tail, got {line:?}"
        );
    }

    #[test]
    fn overflowing_text_area_uses_overflow_thumb() {
        let system = DesignSystem::default();
        let body: String = (0..24)
            .map(|i| format!("line-{i:02}"))
            .collect::<Vec<_>>()
            .join("\n");
        let mut state = TextAreaState::new(body);
        state.set_accepts_input(true);
        let area = Rect::new(0, 0, 28, 12);
        let mut buffer = Buffer::empty(area);
        (&TextArea::new(&system).rows(8)).render(area, &mut buffer, &mut state);
        let thumb = crate::scroll::ScrollbarStyle::Line.vertical_thumb();
        let mut sb_x = None;
        let mut track_ys = Vec::new();
        for y in 0..area.height {
            for x in 0..area.width {
                if buffer[(x, y)].symbol() == thumb {
                    sb_x = Some(x);
                }
            }
        }
        let sb_x = sb_x.expect("overflowing text area paints a thumb");
        let track = crate::scroll::SCROLLBAR_TRACK;
        for y in 0..area.height {
            let symbol = buffer[(sb_x, y)].symbol();
            if symbol == thumb || symbol == track {
                track_ys.push(y);
            }
        }
        let viewport = track_ys.len();
        let (start, len) = crate::scroll::overflow_thumb(24, viewport, viewport, 0)
            .expect("24 lines overflow the field viewport");
        let thumbs: Vec<u16> = track_ys
            .iter()
            .copied()
            .filter(|y| buffer[(sb_x, *y)].symbol() == thumb)
            .collect();
        assert_eq!(thumbs.len(), len);
        assert_eq!(thumbs[0], track_ys[start]);
    }
}
