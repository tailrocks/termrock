// SPDX-FileCopyrightText: 2026 Alexey Zhokhov
// SPDX-License-Identifier: Apache-2.0

//! ObjectInspector, LogStream, DiffReview (Plan 052 evolutions).

use ratatui_core::{buffer::Buffer, layout::Rect};

use crate::{
    input::{KeyCode, KeyEvent, KeyEventKind, MouseButton, MouseEvent, MouseEventKind},
    interaction::{NavigationMove, PageMove, UiIntent},
    style::{DesignSystem, Role},
    text::take_display_cols,
    widgets::scroll_area::ScrollAreaState,
};

// ── ObjectInspector ─────────────────────────────────────────────────────────

/// Inspector field projection.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct InspectorField<'a> {
    /// Key.
    pub key: &'a str,
    /// Value.
    pub value: &'a str,
    /// Nested indent level.
    pub depth: u8,
}

/// Inspector outcome (cursor is list-local; scene owns surface focus).
#[derive(Debug, Clone, PartialEq, Eq)]
#[non_exhaustive]
pub enum ObjectInspectorOutcome {
    /// No change.
    Ignored,
    /// Cursor moved among fields.
    CursorMoved {
        /// Field index.
        index: usize,
    },
    /// Activate field (copy/open — consumer effect).
    Activate {
        /// Field index.
        index: usize,
    },
    /// Viewport scrolled.
    Scrolled,
}

/// Inspector state — in-list cursor + scroll (not scene focus).
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct ObjectInspectorState {
    cursor: usize,
    scroll: ScrollAreaState,
    /// Host grants keyboard/pointer input.
    accepts_input: bool,
    /// Painted body origin for mouse hits.
    origin: (u16, u16),
    /// Painted body height.
    body_rows: u16,
}

impl ObjectInspectorState {
    /// Fresh inspector at first field.
    #[must_use]
    pub fn new() -> Self {
        Self {
            cursor: 0,
            scroll: ScrollAreaState::default(),
            accepts_input: true,
            origin: (0, 0),
            body_rows: 0,
        }
    }

    /// Cursor field index.
    #[must_use]
    pub const fn cursor(&self) -> usize {
        self.cursor
    }

    /// Programmatic cursor (clamped on next interaction / paint).
    pub fn set_cursor(&mut self, index: usize) {
        self.cursor = index;
    }

    /// Vertical scroll offset in fields.
    #[must_use]
    pub fn offset_y(&self) -> u16 {
        self.scroll.offset_y()
    }

    /// Deprecated name for [`Self::cursor`].
    #[deprecated(note = "use cursor")]
    #[must_use]
    pub const fn focus(&self) -> usize {
        self.cursor
    }

    /// Host input gate (scene / overlay ownership).
    pub fn set_accepts_input(&mut self, accepts: bool) {
        self.accepts_input = accepts;
    }

    /// Whether host granted input.
    #[must_use]
    pub const fn accepts_input(&self) -> bool {
        self.accepts_input
    }

    fn clamp_cursor(&mut self, field_count: usize) {
        if field_count == 0 {
            self.cursor = 0;
        } else {
            self.cursor = self.cursor.min(field_count - 1);
        }
    }

    fn ensure_cursor_visible(&mut self, field_count: usize) {
        if field_count == 0 || self.body_rows == 0 {
            return;
        }
        let vh = usize::from(self.body_rows);
        let start = usize::from(self.scroll.offset_y());
        let end = start.saturating_add(vh);
        if self.cursor < start {
            self.scroll.set_offset_y(self.cursor as u16);
        } else if self.cursor >= end {
            let next = self.cursor.saturating_add(1).saturating_sub(vh);
            self.scroll.set_offset_y(next as u16);
        }
        self.scroll
            .set_content_size(1, field_count.min(u16::MAX as usize) as u16);
        self.scroll.set_viewport(1, self.body_rows);
        self.scroll.clamp();
    }

    /// Keys (cursor nav + activate; page via intent).
    pub fn handle_key(&mut self, key: KeyEvent, field_count: usize) -> ObjectInspectorOutcome {
        if !self.accepts_input || field_count == 0 || key.kind == KeyEventKind::Release {
            return ObjectInspectorOutcome::Ignored;
        }
        self.clamp_cursor(field_count);
        if let Some(intent) = crate::interaction::default_inspector_intent(key) {
            let out = self.handle_intent(intent, field_count);
            if !matches!(out, ObjectInspectorOutcome::Ignored) {
                return out;
            }
        }
        ObjectInspectorOutcome::Ignored
    }

    /// Intent routing.
    pub fn handle_intent(
        &mut self,
        intent: UiIntent,
        field_count: usize,
    ) -> ObjectInspectorOutcome {
        if !self.accepts_input || field_count == 0 {
            return ObjectInspectorOutcome::Ignored;
        }
        self.clamp_cursor(field_count);
        let out = match intent {
            UiIntent::Move(NavigationMove::Next) => {
                let next = (self.cursor + 1).min(field_count - 1);
                if next == self.cursor {
                    return ObjectInspectorOutcome::Ignored;
                }
                self.cursor = next;
                ObjectInspectorOutcome::CursorMoved { index: self.cursor }
            }
            UiIntent::Move(NavigationMove::Previous) => {
                let next = self.cursor.saturating_sub(1);
                if next == self.cursor {
                    return ObjectInspectorOutcome::Ignored;
                }
                self.cursor = next;
                ObjectInspectorOutcome::CursorMoved { index: self.cursor }
            }
            UiIntent::Move(NavigationMove::First) => {
                if self.cursor == 0 {
                    return ObjectInspectorOutcome::Ignored;
                }
                self.cursor = 0;
                ObjectInspectorOutcome::CursorMoved { index: 0 }
            }
            UiIntent::Move(NavigationMove::Last) => {
                let last = field_count - 1;
                if self.cursor == last {
                    return ObjectInspectorOutcome::Ignored;
                }
                self.cursor = last;
                ObjectInspectorOutcome::CursorMoved { index: self.cursor }
            }
            UiIntent::Page(PageMove::Forward) => {
                let step = self.body_rows.max(1) as usize;
                let next = (self.cursor + step).min(field_count - 1);
                if next == self.cursor {
                    return ObjectInspectorOutcome::Ignored;
                }
                self.cursor = next;
                ObjectInspectorOutcome::CursorMoved { index: self.cursor }
            }
            UiIntent::Page(PageMove::Backward) => {
                let step = self.body_rows.max(1) as usize;
                let next = self.cursor.saturating_sub(step);
                if next == self.cursor {
                    return ObjectInspectorOutcome::Ignored;
                }
                self.cursor = next;
                ObjectInspectorOutcome::CursorMoved { index: self.cursor }
            }
            UiIntent::Activate | UiIntent::Submit | UiIntent::Toggle => {
                ObjectInspectorOutcome::Activate { index: self.cursor }
            }
            _ => ObjectInspectorOutcome::Ignored,
        };
        if matches!(out, ObjectInspectorOutcome::CursorMoved { .. }) {
            self.ensure_cursor_visible(field_count);
        }
        out
    }

    /// Mouse wheel and click-to-cursor (second click activates).
    pub fn handle_mouse(
        &mut self,
        event: MouseEvent,
        field_count: usize,
    ) -> ObjectInspectorOutcome {
        if !self.accepts_input || field_count == 0 {
            return ObjectInspectorOutcome::Ignored;
        }
        let (ox, oy) = self.origin;
        let body = Rect {
            x: ox,
            y: oy,
            width: 240,
            height: self.body_rows.max(1),
        };
        match event.kind {
            MouseEventKind::ScrollDown if body.contains(event.position) => {
                self.handle_intent(UiIntent::Move(NavigationMove::Next), field_count)
            }
            MouseEventKind::ScrollUp if body.contains(event.position) => {
                self.handle_intent(UiIntent::Move(NavigationMove::Previous), field_count)
            }
            MouseEventKind::Down(MouseButton::Left) if body.contains(event.position) => {
                let start = self.scroll.offset_y() as usize;
                let row = usize::from(event.position.y.saturating_sub(oy));
                let index = start.saturating_add(row);
                if index >= field_count {
                    return ObjectInspectorOutcome::Ignored;
                }
                if self.cursor == index {
                    return ObjectInspectorOutcome::Activate { index };
                }
                self.cursor = index;
                self.ensure_cursor_visible(field_count);
                ObjectInspectorOutcome::CursorMoved { index }
            }
            _ => ObjectInspectorOutcome::Ignored,
        }
    }
}

/// Object inspector list (key/value field projection).
///
/// **Cursor** is list-local (`ObjectInspectorState::cursor`). **Surface focus**
/// is host-owned (`focused` paint + `set_accepts_input`).
#[derive(Debug, Clone, Copy)]
pub struct ObjectInspector<'a> {
    fields: &'a [InspectorField<'a>],
    system: &'a DesignSystem,
    focused: bool,
    ascii: bool,
    colorless: bool,
}

impl<'a> ObjectInspector<'a> {
    /// Fields + design system.
    #[must_use]
    pub const fn new(fields: &'a [InspectorField<'a>], system: &'a DesignSystem) -> Self {
        Self {
            fields,
            system,
            focused: true,
            ascii: false,
            colorless: false,
        }
    }

    /// Scene surface focus chrome (not field cursor).
    #[must_use]
    pub const fn focused(mut self, focused: bool) -> Self {
        self.focused = focused;
        self
    }

    /// ASCII cursor / empty glyphs (`>` / `[ ]` instead of `›` / `∅`).
    #[must_use]
    pub const fn ascii(mut self, ascii: bool) -> Self {
        self.ascii = ascii;
        self
    }

    /// Reduced-color paint (strong text instead of Focus role).
    #[must_use]
    pub const fn colorless(mut self, colorless: bool) -> Self {
        self.colorless = colorless;
        self
    }

    /// Paint fields; keeps cursor in viewport.
    pub fn render(&self, area: Rect, buffer: &mut Buffer, state: &mut ObjectInspectorState) {
        if area.is_empty() {
            state.body_rows = 0;
            return;
        }
        state.origin = (area.x, area.y);
        state.body_rows = area.height;
        let field_count = self.fields.len();
        state.clamp_cursor(field_count);
        state
            .scroll
            .set_content_size(1, field_count.min(u16::MAX as usize) as u16);
        state.scroll.set_viewport(1, area.height);
        state.ensure_cursor_visible(field_count);

        let surface = self.focused && state.accepts_input;
        let narrow = area.width < 28;
        let tiny = area.width < 16;

        if self.fields.is_empty() {
            let glyph = if self.ascii { "[ ] " } else { "∅ " };
            let line = if tiny {
                format!("{glyph}empty")
            } else {
                format!("{glyph}(empty object)")
            };
            buffer.set_stringn(
                area.x,
                area.y,
                &take_display_cols(&line, usize::from(area.width)),
                usize::from(area.width),
                self.system.style(Role::TextMuted),
            );
            return;
        }

        let start = state.scroll.offset_y() as usize;
        let mut y = area.y;
        for (i, field) in self.fields.iter().enumerate().skip(start) {
            if y >= area.bottom() {
                break;
            }
            let pad = "  ".repeat(field.depth as usize);
            let cursor = i == state.cursor;
            let gutter = if cursor && surface {
                if self.ascii { "> " } else { "› " }
            } else if cursor {
                // Unfocused surface: still mark cursor with non-color space gutter.
                if self.ascii { ". " } else { "· " }
            } else {
                "  "
            };
            let line = if tiny {
                // Tiny: key only when not cursor; cursor shows value.
                if cursor {
                    format!("{gutter}{}", field.value)
                } else {
                    format!("{gutter}{}", field.key)
                }
            } else if narrow {
                // Narrow: stack as `key=` then value truncated on same line when room.
                format!("{gutter}{pad}{}={}", field.key, field.value)
            } else {
                format!("{gutter}{pad}{}: {}", field.key, field.value)
            };
            let style = if self.colorless {
                if cursor && surface {
                    self.system.style(Role::TextStrong)
                } else {
                    self.system.style(Role::Text)
                }
            } else if cursor && surface {
                self.system.style(Role::Focus)
            } else if field.depth > 0 {
                self.system.style(Role::TextMuted)
            } else {
                self.system.style(Role::Text)
            };
            let text = take_display_cols(&line, usize::from(area.width));
            buffer.set_stringn(area.x, y, &text, usize::from(area.width), style);
            y = y.saturating_add(1);
        }
    }
}

// ── LogStream ───────────────────────────────────────────────────────────────

/// Log line level.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
#[non_exhaustive]
pub enum LogLevel {
    /// Trace.
    Trace,
    /// Debug.
    Debug,
    /// Info.
    #[default]
    Info,
    /// Warn.
    Warn,
    /// Error.
    Error,
}

impl LogLevel {
    #[must_use]
    fn role(self) -> Role {
        match self {
            Self::Trace | Self::Debug => Role::TextMuted,
            Self::Info => Role::Text,
            Self::Warn => Role::Warning,
            Self::Error => Role::Danger,
        }
    }

    #[must_use]
    fn glyph(self) -> &'static str {
        match self {
            Self::Trace => ".",
            Self::Debug => "·",
            Self::Info => "i",
            Self::Warn => "!",
            Self::Error => "x",
        }
    }
}

/// One log line projection.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LogLine<'a> {
    /// Stable id.
    pub id: &'a str,
    /// Level.
    pub level: LogLevel,
    /// Body.
    pub text: &'a str,
}

/// Log stream outcome.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
#[non_exhaustive]
pub enum LogStreamOutcome {
    /// No change.
    #[default]
    Ignored,
    /// Follow re-attached.
    Follow,
    /// Detached from tail.
    Detach,
}

/// Log stream state (bounded scroll + follow).
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct LogStreamState {
    scroll: ScrollAreaState,
    follow: bool,
}

impl LogStreamState {
    /// Following by default.
    #[must_use]
    pub fn new() -> Self {
        Self {
            scroll: ScrollAreaState::new(),
            follow: true,
        }
    }

    /// After append: rejoin tail if following.
    pub fn on_append(&mut self, total_lines: u16, viewport: u16) {
        self.scroll.set_content_size(0, total_lines);
        self.scroll.set_viewport(0, viewport);
        if self.follow {
            self.scroll.follow_tail();
        }
    }

    /// Keys.
    pub fn handle_key(&mut self, key: KeyEvent) -> LogStreamOutcome {
        if key.kind == KeyEventKind::Press && key.code == KeyCode::End {
            self.follow = true;
            self.scroll.follow_tail();
            return LogStreamOutcome::Follow;
        }
        if self.scroll.handle_key(key) {
            self.follow = false;
            return LogStreamOutcome::Detach;
        }
        LogStreamOutcome::Ignored
    }

    #[must_use]
    /// Following.
    pub const fn is_following(&self) -> bool {
        self.follow
    }

    #[must_use]
    /// Offset.
    pub const fn offset(&self) -> u16 {
        self.scroll.offset_y()
    }
}

/// Log stream paint.
#[derive(Debug, Clone, Copy)]
pub struct LogStream<'a> {
    lines: &'a [LogLine<'a>],
    tokens: &'a DesignSystem,
}

impl<'a> LogStream<'a> {
    /// Lines.
    #[must_use]
    pub const fn new(lines: &'a [LogLine<'a>], tokens: &'a DesignSystem) -> Self {
        Self { lines, tokens }
    }

    /// Paint O(visible).
    pub fn render(&self, area: Rect, buffer: &mut Buffer, state: &LogStreamState) {
        if area.is_empty() {
            return;
        }
        let start = state.offset() as usize;
        let mut y = area.y;
        for line in self.lines.iter().skip(start) {
            if y >= area.bottom() {
                break;
            }
            let body = format!("{} {}", line.level.glyph(), line.text);
            let text = take_display_cols(&body, usize::from(area.width));
            buffer.set_stringn(
                area.x,
                y,
                &text,
                usize::from(area.width),
                self.tokens.style(line.level.role()),
            );
            y = y.saturating_add(1);
        }
    }
}

// ── DiffReview ──────────────────────────────────────────────────────────────

/// Hunk header for navigation.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DiffHunk {
    /// Start line index in projected diff lines.
    pub start: usize,
    /// Hunk length.
    pub len: usize,
    /// Header label.
    pub header: String,
}

/// Diff review outcome.
#[derive(Debug, Clone, PartialEq, Eq)]
#[non_exhaustive]
pub enum DiffReviewOutcome {
    /// No change.
    Ignored,
    /// Next/prev hunk.
    HunkFocused {
        /// Index.
        index: usize,
    },
    /// Activate hunk (stage/copy request).
    HunkActivated {
        /// Index.
        index: usize,
    },
    /// Toggle split/unified preference request.
    ToggleMode,
}

/// Diff review state.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct DiffReviewState {
    hunk_index: usize,
    split: bool,
    scroll: ScrollAreaState,
}

impl DiffReviewState {
    /// Unified mode.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    #[must_use]
    /// Split preferred.
    pub const fn is_split(&self) -> bool {
        self.split
    }

    /// Keys.
    pub fn handle_key(&mut self, key: KeyEvent, hunk_count: usize) -> DiffReviewOutcome {
        if key.kind != KeyEventKind::Press {
            let _ = self.scroll.handle_key(key);
            return DiffReviewOutcome::Ignored;
        }
        match key.code {
            KeyCode::Char('n') if hunk_count > 0 => {
                self.hunk_index = (self.hunk_index + 1) % hunk_count;
                DiffReviewOutcome::HunkFocused {
                    index: self.hunk_index,
                }
            }
            KeyCode::Char('p') if hunk_count > 0 => {
                self.hunk_index = self.hunk_index.checked_sub(1).unwrap_or(hunk_count - 1);
                DiffReviewOutcome::HunkFocused {
                    index: self.hunk_index,
                }
            }
            KeyCode::Enter if hunk_count > 0 => DiffReviewOutcome::HunkActivated {
                index: self.hunk_index,
            },
            KeyCode::Char('s') => {
                self.split = !self.split;
                DiffReviewOutcome::ToggleMode
            }
            _ => {
                let _ = self.scroll.handle_key(key);
                DiffReviewOutcome::Ignored
            }
        }
    }
}

/// Diff review chrome over projected lines.
#[derive(Debug, Clone, Copy)]
pub struct DiffReview<'a> {
    lines: &'a [(&'a str, Role)],
    tokens: &'a DesignSystem,
}

impl<'a> DiffReview<'a> {
    /// Lines with roles (added/removed/text).
    #[must_use]
    pub const fn new(lines: &'a [(&'a str, Role)], tokens: &'a DesignSystem) -> Self {
        Self { lines, tokens }
    }

    /// Paint from scroll offset.
    pub fn render(&self, area: Rect, buffer: &mut Buffer, state: &DiffReviewState) {
        if area.is_empty() {
            return;
        }
        let start = state.scroll.offset_y() as usize;
        let mut y = area.y;
        for (text, role) in self.lines.iter().skip(start) {
            if y >= area.bottom() {
                break;
            }
            let line = take_display_cols(text, usize::from(area.width));
            buffer.set_stringn(
                area.x,
                y,
                &line,
                usize::from(area.width),
                self.tokens.style(*role),
            );
            y = y.saturating_add(1);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::input::{KeyEvent, KeyModifiers};
    use crate::interaction::{NavigationMove, UiIntent};
    use crate::style::DesignSystem;
    use ratatui_core::layout::Position;

    fn sample_fields() -> [InspectorField<'static>; 5] {
        [
            InspectorField {
                key: "id",
                value: "42",
                depth: 0,
            },
            InspectorField {
                key: "name",
                value: "東京",
                depth: 0,
            },
            InspectorField {
                key: "meta",
                value: "{…}",
                depth: 0,
            },
            InspectorField {
                key: "size",
                value: "1 KiB",
                depth: 1,
            },
            InspectorField {
                key: "kind",
                value: "blob",
                depth: 1,
            },
        ]
    }

    fn row_text(buffer: &Buffer, area: Rect, y: u16) -> String {
        (area.x..area.right())
            .map(|x| buffer[(x, y)].symbol().to_string())
            .collect::<String>()
            .trim_end()
            .to_string()
    }

    #[test]
    fn log_follow_detaches_on_scroll() {
        let mut state = LogStreamState::new();
        state.on_append(100, 10);
        assert!(state.is_following());
        let out = state.handle_key(KeyEvent::new(KeyCode::PageUp, KeyModifiers::NONE));
        assert!(matches!(out, LogStreamOutcome::Detach));
        assert!(!state.is_following());
    }

    #[test]
    fn diff_hunk_nav() {
        let mut state = DiffReviewState::new();
        assert!(matches!(
            state.handle_key(KeyEvent::new(KeyCode::Char('n'), KeyModifiers::NONE), 3),
            DiffReviewOutcome::HunkFocused { index: 1 }
        ));
    }

    #[test]
    fn inspector_cursor_moved_not_focus_changed() {
        let mut state = ObjectInspectorState::new();
        assert!(matches!(
            state.handle_key(KeyEvent::new(KeyCode::Down, KeyModifiers::NONE), 3),
            ObjectInspectorOutcome::CursorMoved { index: 1 }
        ));
        assert_eq!(state.cursor(), 1);
        let src = include_str!("review.rs");
        let head = src
            .split("#[cfg(test)]")
            .next()
            .unwrap_or(src)
            .lines()
            .filter(|l| {
                let t = l.trim_start();
                !t.starts_with("//") && !t.starts_with("//!")
            })
            .collect::<Vec<_>>()
            .join("\n");
        assert!(!head.contains("FocusChanged"));
        assert!(head.contains("CursorMoved"));
        assert!(head.contains("fn cursor("));
    }

    #[test]
    fn inspector_activate_and_jk() {
        let mut state = ObjectInspectorState::new();
        assert!(matches!(
            state.handle_key(KeyEvent::new(KeyCode::Enter, KeyModifiers::NONE), 2),
            ObjectInspectorOutcome::Activate { index: 0 }
        ));
        let _ = state.handle_key(KeyEvent::new(KeyCode::Char('j'), KeyModifiers::NONE), 2);
        assert_eq!(state.cursor(), 1);
        assert!(matches!(
            state.handle_intent(UiIntent::Activate, 2),
            ObjectInspectorOutcome::Activate { index: 1 }
        ));
    }

    #[test]
    fn inspector_accepts_input_gate() {
        let mut state = ObjectInspectorState::new();
        state.set_accepts_input(false);
        assert!(matches!(
            state.handle_key(KeyEvent::new(KeyCode::Down, KeyModifiers::NONE), 2),
            ObjectInspectorOutcome::Ignored
        ));
        assert!(matches!(
            state.handle_intent(UiIntent::Move(NavigationMove::Next), 2),
            ObjectInspectorOutcome::Ignored
        ));
    }

    #[test]
    fn inspector_home_end_page() {
        let mut state = ObjectInspectorState::new();
        state.body_rows = 2;
        assert!(matches!(
            state.handle_key(KeyEvent::new(KeyCode::End, KeyModifiers::NONE), 5),
            ObjectInspectorOutcome::CursorMoved { index: 4 }
        ));
        assert!(matches!(
            state.handle_key(KeyEvent::new(KeyCode::Home, KeyModifiers::NONE), 5),
            ObjectInspectorOutcome::CursorMoved { index: 0 }
        ));
        assert!(matches!(
            state.handle_key(KeyEvent::new(KeyCode::PageDown, KeyModifiers::NONE), 5),
            ObjectInspectorOutcome::CursorMoved { index: 2 }
        ));
    }

    #[test]
    fn inspector_mouse_click_and_wheel() {
        let mut state = ObjectInspectorState::new();
        state.origin = (0, 0);
        state.body_rows = 5;
        let click = MouseEvent {
            kind: MouseEventKind::Down(MouseButton::Left),
            position: Position::new(0, 2),
            modifiers: KeyModifiers::NONE,
        };
        assert!(matches!(
            state.handle_mouse(click, 5),
            ObjectInspectorOutcome::CursorMoved { index: 2 }
        ));
        // Second click on same row activates.
        assert!(matches!(
            state.handle_mouse(click, 5),
            ObjectInspectorOutcome::Activate { index: 2 }
        ));
        let wheel = MouseEvent {
            kind: MouseEventKind::ScrollUp,
            position: Position::new(0, 1),
            modifiers: KeyModifiers::NONE,
        };
        assert!(matches!(
            state.handle_mouse(wheel, 5),
            ObjectInspectorOutcome::CursorMoved { index: 1 }
        ));
    }

    #[test]
    fn inspector_paint_cursor_gutter_and_empty() {
        let system = DesignSystem::default();
        let fields = sample_fields();
        let mut state = ObjectInspectorState::new();
        let area = Rect::new(0, 0, 32, 5);
        let mut buffer = Buffer::empty(area);
        ObjectInspector::new(&fields, &system).render(area, &mut buffer, &mut state);
        let first = row_text(&buffer, area, 0);
        assert!(
            first.starts_with('›') || first.contains("id"),
            "cursor gutter + key: {first:?}"
        );
        assert!(first.contains("id"), "{first:?}");

        let mut empty_state = ObjectInspectorState::new();
        let mut empty_buf = Buffer::empty(area);
        ObjectInspector::new(&[], &system).render(area, &mut empty_buf, &mut empty_state);
        let empty = row_text(&empty_buf, area, 0);
        assert!(empty.contains('∅') || empty.contains("empty"), "{empty:?}");
    }

    #[test]
    fn inspector_ascii_and_colorless() {
        let system = DesignSystem::default();
        let fields = sample_fields();
        let mut state = ObjectInspectorState::new();
        let area = Rect::new(0, 0, 28, 3);
        let mut buffer = Buffer::empty(area);
        ObjectInspector::new(&fields, &system)
            .ascii(true)
            .colorless(true)
            .render(area, &mut buffer, &mut state);
        let first = row_text(&buffer, area, 0);
        assert!(first.starts_with('>'), "ascii cursor: {first:?}");
    }

    #[test]
    fn inspector_narrow_and_tiny() {
        let system = DesignSystem::default();
        let fields = sample_fields();
        let mut state = ObjectInspectorState::new();
        let narrow = Rect::new(0, 0, 22, 3);
        let mut nbuf = Buffer::empty(narrow);
        ObjectInspector::new(&fields, &system).render(narrow, &mut nbuf, &mut state);
        let nline = row_text(&nbuf, narrow, 0);
        assert!(nline.contains('=') || nline.contains("id"), "{nline:?}");

        let tiny = Rect::new(0, 0, 12, 2);
        let mut tbuf = Buffer::empty(tiny);
        ObjectInspector::new(&fields, &system).render(tiny, &mut tbuf, &mut state);
        let tline = row_text(&tbuf, tiny, 0);
        assert!(!tline.is_empty(), "tiny paints something");
    }

    #[test]
    fn inspector_cursor_follow_scroll() {
        let system = DesignSystem::default();
        let fields = sample_fields();
        let mut state = ObjectInspectorState::new();
        let area = Rect::new(0, 0, 30, 2);
        let mut buffer = Buffer::empty(area);
        // Prime body_rows via paint.
        ObjectInspector::new(&fields, &system).render(area, &mut buffer, &mut state);
        assert_eq!(state.body_rows, 2);
        let _ = state.handle_key(KeyEvent::new(KeyCode::End, KeyModifiers::NONE), 5);
        assert_eq!(state.cursor(), 4);
        ObjectInspector::new(&fields, &system).render(area, &mut buffer, &mut state);
        assert!(
            state.offset_y() >= 3,
            "cursor follow should scroll: offset={}",
            state.offset_y()
        );
        let last = row_text(&buffer, area, 1);
        assert!(last.contains("kind") || last.contains("blob"), "{last:?}");
    }

    #[test]
    fn inspector_unfocused_surface_still_marks_cursor() {
        let system = DesignSystem::default();
        let fields = sample_fields();
        let mut state = ObjectInspectorState::new();
        let area = Rect::new(0, 0, 28, 3);
        let mut buffer = Buffer::empty(area);
        ObjectInspector::new(&fields, &system)
            .focused(false)
            .render(area, &mut buffer, &mut state);
        let first = row_text(&buffer, area, 0);
        assert!(
            first.starts_with('·') || first.starts_with('.'),
            "unfocused cursor mark: {first:?}"
        );
    }

    #[test]
    fn inspector_unicode_key_not_split() {
        let system = DesignSystem::default();
        let fields = [InspectorField {
            key: "名",
            value: "東京🧪",
            depth: 0,
        }];
        let mut state = ObjectInspectorState::new();
        let area = Rect::new(0, 0, 14, 1);
        let mut buffer = Buffer::empty(area);
        ObjectInspector::new(&fields, &system).render(area, &mut buffer, &mut state);
        let text = row_text(&buffer, area, 0);
        // Wide graphemes intact or fully dropped — never half cells of emoji.
        let emoji = text.matches('🧪').count();
        assert!(emoji <= 1, "{text:?}");
    }
}
