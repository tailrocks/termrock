// SPDX-FileCopyrightText: 2026 Alexey Zhokhov
// SPDX-License-Identifier: Apache-2.0

//! ObjectInspector, LogStream, DiffReview (Plan 052 evolutions).

use ratatui_core::{buffer::Buffer, layout::Rect};

use crate::{
    input::{
        KeyCode,
        KeyEvent,
        KeyEventKind,
    },
    style::{
        DesignTokens,
        Role,
    },
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

/// Inspector outcome.
#[derive(Debug, Clone, PartialEq, Eq)]
#[non_exhaustive]
pub enum ObjectInspectorOutcome {
    /// No change.
    Ignored,
    /// Focus moved.
    FocusChanged {
        /// Index.
        index: usize,
    },
    /// Activate field (copy/open — consumer effect).
    Activate {
        /// Index.
        index: usize,
    },
}

/// Inspector state.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct ObjectInspectorState {
    focus: usize,
    scroll: ScrollAreaState,
}

impl ObjectInspectorState {
    /// Fresh.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Keys.
    pub fn handle_key(&mut self, key: KeyEvent, field_count: usize) -> ObjectInspectorOutcome {
        if field_count == 0 || key.kind == KeyEventKind::Release {
            return ObjectInspectorOutcome::Ignored;
        }
        match key.code {
            KeyCode::Down => {
                self.focus = (self.focus + 1).min(field_count - 1);
                ObjectInspectorOutcome::FocusChanged { index: self.focus }
            }
            KeyCode::Up => {
                self.focus = self.focus.saturating_sub(1);
                ObjectInspectorOutcome::FocusChanged { index: self.focus }
            }
            KeyCode::Enter if key.kind == KeyEventKind::Press => {
                ObjectInspectorOutcome::Activate { index: self.focus }
            }
            _ => {
                let _ = self.scroll.handle_key(key);
                ObjectInspectorOutcome::Ignored
            }
        }
    }
}

/// Object inspector list.
#[derive(Debug, Clone, Copy)]
pub struct ObjectInspector<'a> {
    fields: &'a [InspectorField<'a>],
    tokens: &'a DesignTokens,
}

impl<'a> ObjectInspector<'a> {
    /// Fields.
    #[must_use]
    pub const fn new(fields: &'a [InspectorField<'a>], tokens: &'a DesignTokens) -> Self {
        Self { fields, tokens }
    }

    /// Paint.
    pub fn render(&self, area: Rect, buffer: &mut Buffer, state: &ObjectInspectorState) {
        if area.is_empty() {
            return;
        }
        let start = state.scroll.offset_y() as usize;
        let mut y = area.y;
        for (i, field) in self.fields.iter().enumerate().skip(start) {
            if y >= area.bottom() {
                break;
            }
            let pad = "  ".repeat(field.depth as usize);
            let line = format!("{pad}{}: {}", field.key, field.value);
            let style = if i == state.focus {
                self.tokens.theme.style(Role::Focus)
            } else {
                self.tokens.theme.style(Role::Text)
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
    tokens: &'a DesignTokens,
}

impl<'a> LogStream<'a> {
    /// Lines.
    #[must_use]
    pub const fn new(lines: &'a [LogLine<'a>], tokens: &'a DesignTokens) -> Self {
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
                self.tokens.theme.style(line.level.role()),
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
    tokens: &'a DesignTokens,
}

impl<'a> DiffReview<'a> {
    /// Lines with roles (added/removed/text).
    #[must_use]
    pub const fn new(lines: &'a [(&'a str, Role)], tokens: &'a DesignTokens) -> Self {
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
                self.tokens.theme.style(*role),
            );
            y = y.saturating_add(1);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::input::{KeyEvent, KeyModifiers};

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
    fn inspector_activate() {
        let mut state = ObjectInspectorState::new();
        assert!(matches!(
            state.handle_key(KeyEvent::new(KeyCode::Enter, KeyModifiers::NONE), 2),
            ObjectInspectorOutcome::Activate { index: 0 }
        ));
    }
}
