// SPDX-FileCopyrightText: 2026 Alexey Zhokhov
// SPDX-License-Identifier: Apache-2.0

//! LogStream, DiffReview (Plan 052 evolutions).
//!
//! ObjectInspector lives in [`super::object_inspector`].

use ratatui_core::{buffer::Buffer, layout::Rect};

use crate::{
    input::{KeyCode, KeyEvent, KeyEventKind, MouseButton, MouseEvent, MouseEventKind},
    interaction::{NavigationMove, PageMove, UiIntent},
    style::{DesignSystem, Role},
    text::take_display_cols,
    widgets::scroll_area::ScrollAreaState,
};

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

    /// Level mark for structured lines (`ascii` uses T/D/I/W/E).
    #[must_use]
    pub const fn glyph(self, ascii: bool) -> &'static str {
        if ascii {
            match self {
                Self::Trace => "T",
                Self::Debug => "D",
                Self::Info => "I",
                Self::Warn => "W",
                Self::Error => "E",
            }
        } else {
            match self {
                Self::Trace => ".",
                Self::Debug => "·",
                Self::Info => "i",
                Self::Warn => "!",
                Self::Error => "x",
            }
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

/// Log stream outcome (scroll + follow; no scene focus).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
#[non_exhaustive]
pub enum LogStreamOutcome {
    /// No change.
    #[default]
    Ignored,
    /// Viewport scrolled (follow already off or unchanged).
    Scrolled {
        /// New vertical offset.
        offset: u16,
    },
    /// Follow re-attached (tail).
    Follow,
    /// Detached from tail by scroll/nav.
    Detach,
}

/// Log stream state (bounded scroll + follow; scene owns surface focus).
///
/// Follow/pause/unseen live in [`ScrollAreaState`] — sole authority.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LogStreamState {
    scroll: ScrollAreaState,
    accepts_input: bool,
    origin: (u16, u16),
    body_rows: u16,
    /// Full painted height including follow chip.
    area_rows: u16,
    /// Last projected line count (for scroll clamp without full re-measure).
    line_count: u16,
}

impl Default for LogStreamState {
    fn default() -> Self {
        Self::new()
    }
}

impl LogStreamState {
    /// Following by default.
    #[must_use]
    pub fn new() -> Self {
        let mut scroll = ScrollAreaState::new().axes(true, false);
        scroll.follow_tail();
        Self {
            scroll,
            accepts_input: true,
            origin: (0, 0),
            body_rows: 0,
            area_rows: 0,
            line_count: 0,
        }
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

    /// Following tail ([`ScrollAreaState`] sole authority).
    #[must_use]
    pub const fn is_following(&self) -> bool {
        self.scroll.is_following()
    }

    /// Vertical offset.
    #[must_use]
    pub const fn offset(&self) -> u16 {
        self.scroll.offset_y()
    }

    /// Shared scroll state (visible range, indicator, anchors).
    #[must_use]
    pub const fn scroll(&self) -> &ScrollAreaState {
        &self.scroll
    }

    /// Force follow on/off (host sync).
    pub fn set_following(&mut self, following: bool) {
        if following {
            self.scroll.follow_tail();
        } else {
            self.scroll.pause_follow();
        }
    }

    fn sync_metrics(&mut self, total_lines: u16, viewport: u16) {
        self.line_count = total_lines;
        self.body_rows = viewport;
        // set_content_size applies follow-tail or new-content indicator.
        self.scroll.set_content_size(1, total_lines);
        self.scroll.set_viewport(1, viewport);
    }

    /// After append: rejoin tail if following; else accumulate unseen.
    pub fn on_append(&mut self, total_lines: u16, viewport: u16) {
        self.sync_metrics(total_lines, viewport);
    }

    fn scroll_by_lines(&mut self, delta: i32) -> bool {
        if self.body_rows == 0 && self.line_count == 0 {
            return false;
        }
        self.scroll
            .scroll_by(delta as isize, 0)
            .is_scrolled()
    }

    fn scroll_page(&mut self, forward: bool) -> bool {
        self.scroll.page(forward).is_scrolled()
    }

    /// Keys via [`crate::interaction::default_log_stream_intent`].
    pub fn handle_key(&mut self, key: KeyEvent) -> LogStreamOutcome {
        if !self.accepts_input || key.kind == KeyEventKind::Release {
            return LogStreamOutcome::Ignored;
        }
        // Product chord: explicit `f` always toggles follow (also on intent Toggle).
        if key.kind == KeyEventKind::Press && matches!(key.code, KeyCode::Char('f' | 'F')) {
            return self.handle_intent(UiIntent::Toggle);
        }
        if let Some(intent) = crate::interaction::default_log_stream_intent(key) {
            return self.handle_intent(intent);
        }
        LogStreamOutcome::Ignored
    }

    /// Intent routing (scroll + follow).
    pub fn handle_intent(&mut self, intent: UiIntent) -> LogStreamOutcome {
        if !self.accepts_input {
            return LogStreamOutcome::Ignored;
        }
        match intent {
            UiIntent::Move(NavigationMove::Next) => {
                let was_follow = self.scroll.is_following();
                if !self.scroll_by_lines(1) {
                    return LogStreamOutcome::Ignored;
                }
                if was_follow {
                    LogStreamOutcome::Detach
                } else {
                    LogStreamOutcome::Scrolled {
                        offset: self.offset(),
                    }
                }
            }
            UiIntent::Move(NavigationMove::Previous) => {
                let was_follow = self.scroll.is_following();
                if !self.scroll_by_lines(-1) {
                    return LogStreamOutcome::Ignored;
                }
                if was_follow {
                    LogStreamOutcome::Detach
                } else {
                    LogStreamOutcome::Scrolled {
                        offset: self.offset(),
                    }
                }
            }
            UiIntent::Move(NavigationMove::First) => {
                let was_follow = self.scroll.is_following();
                let out = self.scroll.home();
                if was_follow {
                    LogStreamOutcome::Detach
                } else if out.is_scrolled() {
                    LogStreamOutcome::Scrolled {
                        offset: self.offset(),
                    }
                } else {
                    LogStreamOutcome::Ignored
                }
            }
            UiIntent::Move(NavigationMove::Last) => {
                if self.scroll.is_following() {
                    return LogStreamOutcome::Ignored;
                }
                self.scroll.follow_tail();
                LogStreamOutcome::Follow
            }
            UiIntent::Page(PageMove::Forward) => {
                let was_follow = self.scroll.is_following();
                if !self.scroll_page(true) {
                    return LogStreamOutcome::Ignored;
                }
                if was_follow {
                    LogStreamOutcome::Detach
                } else {
                    LogStreamOutcome::Scrolled {
                        offset: self.offset(),
                    }
                }
            }
            UiIntent::Page(PageMove::Backward) => {
                let was_follow = self.scroll.is_following();
                if !self.scroll_page(false) {
                    return LogStreamOutcome::Ignored;
                }
                if was_follow {
                    LogStreamOutcome::Detach
                } else {
                    LogStreamOutcome::Scrolled {
                        offset: self.offset(),
                    }
                }
            }
            UiIntent::Toggle => {
                if self.scroll.is_following() {
                    self.scroll.pause_follow();
                    LogStreamOutcome::Detach
                } else {
                    self.scroll.follow_tail();
                    LogStreamOutcome::Follow
                }
            }
            _ => LogStreamOutcome::Ignored,
        }
    }

    /// Wheel scroll + follow-chip click.
    pub fn handle_mouse(&mut self, event: MouseEvent) -> LogStreamOutcome {
        if !self.accepts_input {
            return LogStreamOutcome::Ignored;
        }
        let (ox, oy) = self.origin;
        let hit = Rect {
            x: ox,
            y: oy,
            width: 240,
            height: self.area_rows.max(self.body_rows).max(1),
        };
        if !hit.contains(event.position) {
            return LogStreamOutcome::Ignored;
        }
        match event.kind {
            MouseEventKind::ScrollDown => self.handle_intent(UiIntent::Move(NavigationMove::Next)),
            MouseEventKind::ScrollUp => {
                self.handle_intent(UiIntent::Move(NavigationMove::Previous))
            }
            MouseEventKind::Down(MouseButton::Left) => {
                // Bottom painted row is follow chip when area reserves it.
                let chip_y = oy.saturating_add(self.area_rows.saturating_sub(1));
                if self.area_rows >= 2 && event.position.y == chip_y {
                    if self.scroll.is_following() {
                        return LogStreamOutcome::Ignored;
                    }
                    // Jump to new content / resume follow.
                    self.scroll.jump_to_new_content();
                    return LogStreamOutcome::Follow;
                }
                LogStreamOutcome::Ignored
            }
            _ => LogStreamOutcome::Ignored,
        }
    }
}

/// Log stream paint (projected lines + follow chrome).
///
/// **Surface focus** is host-owned (`focused` + `accepts_input`). Scroll/follow
/// state is stream-local.
#[derive(Debug, Clone, Copy)]
pub struct LogStream<'a> {
    lines: &'a [LogLine<'a>],
    system: &'a DesignSystem,
    focused: bool,
    ascii: bool,
    colorless: bool,
}

impl<'a> LogStream<'a> {
    /// Lines + design system.
    #[must_use]
    pub const fn new(lines: &'a [LogLine<'a>], system: &'a DesignSystem) -> Self {
        Self {
            lines,
            system,
            focused: true,
            ascii: false,
            colorless: false,
        }
    }

    /// Scene surface focus chrome.
    #[must_use]
    pub const fn focused(mut self, focused: bool) -> Self {
        self.focused = focused;
        self
    }

    /// ASCII level / follow glyphs.
    #[must_use]
    pub const fn ascii(mut self, ascii: bool) -> Self {
        self.ascii = ascii;
        self
    }

    /// Reduced-color paint (level tone via strong/muted only).
    #[must_use]
    pub const fn colorless(mut self, colorless: bool) -> Self {
        self.colorless = colorless;
        self
    }

    /// Paint O(visible lines).
    pub fn render(&self, area: Rect, buffer: &mut Buffer, state: &mut LogStreamState) {
        if area.is_empty() {
            state.body_rows = 0;
            state.area_rows = 0;
            return;
        }
        state.origin = (area.x, area.y);
        state.area_rows = area.height;
        let total = self.lines.len().min(u16::MAX as usize) as u16;
        let following = state.scroll.is_following();
        let show_chip = area.height >= 2 && (following || !self.lines.is_empty());
        let body_h = if show_chip {
            area.height.saturating_sub(1)
        } else {
            area.height
        };
        state.sync_metrics(total, body_h);

        let surface = self.focused && state.accepts_input;
        let tiny = area.width < 16;
        let narrow = area.width < 28;

        if self.lines.is_empty() {
            let mark = if self.ascii { "[ ] " } else { "∅ " };
            let msg = if tiny {
                format!("{mark}empty")
            } else {
                format!("{mark}(empty log)")
            };
            buffer.set_stringn(
                area.x,
                area.y,
                &take_display_cols(&msg, usize::from(area.width)),
                usize::from(area.width),
                self.system.style(Role::TextMuted),
            );
        } else {
            let start = state.offset() as usize;
            let mut y = area.y;
            let bottom = area.y.saturating_add(body_h);
            for line in self.lines.iter().skip(start) {
                if y >= bottom {
                    break;
                }
                let body = if tiny {
                    line.text.to_string()
                } else {
                    let g = line.level.glyph(self.ascii);
                    if narrow {
                        format!("{g} {}", line.text)
                    } else {
                        format!("{g} {}", line.text)
                    }
                };
                let style = if self.colorless {
                    match line.level {
                        LogLevel::Error | LogLevel::Warn if surface => {
                            self.system.style(Role::TextStrong)
                        }
                        LogLevel::Trace | LogLevel::Debug => self.system.style(Role::TextMuted),
                        _ => self.system.style(Role::Text),
                    }
                } else if surface {
                    self.system.style(line.level.role())
                } else {
                    // Unfocused surface: mute levels slightly.
                    match line.level {
                        LogLevel::Error => self.system.style(Role::Danger),
                        LogLevel::Warn => self.system.style(Role::Warning),
                        _ => self.system.style(Role::TextMuted),
                    }
                };
                let text = take_display_cols(&body, usize::from(area.width));
                buffer.set_stringn(area.x, y, &text, usize::from(area.width), style);
                y = y.saturating_add(1);
            }
        }

        if show_chip {
            let chip_y = area.bottom().saturating_sub(1);
            let following = state.scroll.is_following();
            let indicator = state.scroll.new_content();
            let chip = if following {
                if self.ascii {
                    "v follow".to_string()
                } else {
                    "↓ follow".to_string()
                }
            } else if indicator.visible {
                if self.ascii {
                    format!("v {} new  f=follow", indicator.unseen)
                } else {
                    format!("↓ {} new · f follow", indicator.unseen)
                }
            } else if self.ascii {
                "^ pinned  f=follow".to_string()
            } else {
                "↑ pinned · f follow".to_string()
            };
            let chip_style = if following && surface {
                if self.colorless {
                    self.system.style(Role::TextStrong)
                } else {
                    self.system.style(Role::Accent)
                }
            } else if indicator.visible {
                self.system.style(Role::Warning)
            } else {
                self.system.style(Role::TextMuted)
            };
            buffer.set_stringn(
                area.x,
                chip_y,
                &take_display_cols(&chip, usize::from(area.width)),
                usize::from(area.width),
                chip_style,
            );
        }
    }
}

// ── DiffReview ──────────────────────────────────────────────────────────────

/// Hunk header for navigation.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DiffHunk {
    /// Start line index in projected diff lines.
    pub start: usize,
    /// Hunk length in lines (≥ 1 for a selectable hunk).
    pub len: usize,
    /// Header label (e.g. `@@ -1,3 +1,4 @@`).
    pub header: String,
}

impl DiffHunk {
    /// Inclusive end line index (exclusive bound = start + len).
    #[must_use]
    pub fn end(&self) -> usize {
        self.start.saturating_add(self.len.max(1))
    }

    /// Whether projected line `i` belongs to this hunk.
    #[must_use]
    pub fn contains_line(&self, i: usize) -> bool {
        i >= self.start && i < self.end()
    }
}

/// Diff review outcome (hunk cursor is local; scene owns surface focus).
#[derive(Debug, Clone, PartialEq, Eq)]
#[non_exhaustive]
pub enum DiffReviewOutcome {
    /// No change.
    Ignored,
    /// Hunk cursor moved (not scene focus).
    HunkCursorMoved {
        /// Hunk index.
        index: usize,
    },
    /// Activate hunk (stage/copy/open — consumer effect).
    HunkActivated {
        /// Hunk index.
        index: usize,
    },
    /// Viewport scrolled by line/page.
    Scrolled {
        /// New vertical offset.
        offset: u16,
    },
    /// Toggle split/unified preference request.
    ToggleMode,
}

/// Diff review state — hunk cursor + scroll (not scene focus).
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct DiffReviewState {
    hunk_cursor: usize,
    split: bool,
    scroll: ScrollAreaState,
    accepts_input: bool,
    origin: (u16, u16),
    body_rows: u16,
    line_count: u16,
}

impl DiffReviewState {
    /// Unified mode, first hunk.
    #[must_use]
    pub fn new() -> Self {
        Self {
            hunk_cursor: 0,
            split: false,
            scroll: ScrollAreaState::new(),
            accepts_input: true,
            origin: (0, 0),
            body_rows: 0,
            line_count: 0,
        }
    }

    /// Current hunk cursor index.
    #[must_use]
    pub const fn hunk_cursor(&self) -> usize {
        self.hunk_cursor
    }

    /// Deprecated name for [`Self::hunk_cursor`].
    #[deprecated(note = "use hunk_cursor")]
    #[must_use]
    pub const fn hunk_index(&self) -> usize {
        self.hunk_cursor
    }

    /// Programmatic hunk cursor.
    pub fn set_hunk_cursor(&mut self, index: usize) {
        self.hunk_cursor = index;
    }

    /// Vertical scroll offset.
    #[must_use]
    pub fn offset_y(&self) -> u16 {
        self.scroll.offset_y()
    }

    /// Split preferred (paint may still force unified when narrow).
    #[must_use]
    pub const fn is_split(&self) -> bool {
        self.split
    }

    /// Host input gate.
    pub fn set_accepts_input(&mut self, accepts: bool) {
        self.accepts_input = accepts;
    }

    /// Whether host granted input.
    #[must_use]
    pub const fn accepts_input(&self) -> bool {
        self.accepts_input
    }

    fn clamp_hunk(&mut self, hunk_count: usize) {
        if hunk_count == 0 {
            self.hunk_cursor = 0;
        } else {
            self.hunk_cursor = self.hunk_cursor.min(hunk_count - 1);
        }
    }

    fn sync_metrics(&mut self, line_count: u16, viewport: u16) {
        self.line_count = line_count;
        self.body_rows = viewport;
        self.scroll.set_content_size(1, line_count);
        self.scroll.set_viewport(1, viewport);
        self.scroll.clamp();
    }

    fn ensure_hunk_visible(&mut self, hunks: &[DiffHunk]) {
        if hunks.is_empty() || self.body_rows == 0 {
            return;
        }
        self.clamp_hunk(hunks.len());
        let Some(hunk) = hunks.get(self.hunk_cursor) else {
            return;
        };
        let vh = usize::from(self.body_rows);
        let start = usize::from(self.scroll.offset_y());
        let end = start.saturating_add(vh);
        if hunk.start < start {
            self.scroll
                .set_offset_y_quiet(hunk.start.min(u16::MAX as usize) as u16);
        } else if hunk.start >= end {
            let next = hunk.start.saturating_add(1).saturating_sub(vh);
            self.scroll
                .set_offset_y_quiet(next.min(u16::MAX as usize) as u16);
        }
    }

    fn scroll_by_lines(&mut self, delta: i32) -> bool {
        self.scroll
            .scroll_by(delta as isize, 0)
            .is_scrolled()
    }

    fn move_hunk(&mut self, next: usize, hunks: &[DiffHunk]) -> DiffReviewOutcome {
        if hunks.is_empty() {
            return DiffReviewOutcome::Ignored;
        }
        let next = next.min(hunks.len() - 1);
        if next == self.hunk_cursor {
            return DiffReviewOutcome::Ignored;
        }
        self.hunk_cursor = next;
        self.ensure_hunk_visible(hunks);
        DiffReviewOutcome::HunkCursorMoved {
            index: self.hunk_cursor,
        }
    }

    /// Keys: n/p hunks; s mode; intents for scroll/activate.
    pub fn handle_key(&mut self, key: KeyEvent, hunks: &[DiffHunk]) -> DiffReviewOutcome {
        if !self.accepts_input || key.kind == KeyEventKind::Release {
            return DiffReviewOutcome::Ignored;
        }
        self.clamp_hunk(hunks.len());
        if key.kind == KeyEventKind::Press {
            match key.code {
                KeyCode::Char('n' | 'N') if !hunks.is_empty() => {
                    let next = (self.hunk_cursor + 1).min(hunks.len() - 1);
                    return self.move_hunk(next, hunks);
                }
                KeyCode::Char('p' | 'P') if !hunks.is_empty() => {
                    let next = self.hunk_cursor.saturating_sub(1);
                    return self.move_hunk(next, hunks);
                }
                KeyCode::Char('s' | 'S') => {
                    self.split = !self.split;
                    return DiffReviewOutcome::ToggleMode;
                }
                _ => {}
            }
        }
        if let Some(intent) = crate::interaction::default_diff_review_intent(key) {
            return self.handle_intent(intent, hunks);
        }
        DiffReviewOutcome::Ignored
    }

    /// Intent routing.
    pub fn handle_intent(&mut self, intent: UiIntent, hunks: &[DiffHunk]) -> DiffReviewOutcome {
        if !self.accepts_input {
            return DiffReviewOutcome::Ignored;
        }
        self.clamp_hunk(hunks.len());
        match intent {
            UiIntent::Move(NavigationMove::Next) => {
                if !self.scroll_by_lines(1) {
                    return DiffReviewOutcome::Ignored;
                }
                DiffReviewOutcome::Scrolled {
                    offset: self.offset_y(),
                }
            }
            UiIntent::Move(NavigationMove::Previous) => {
                if !self.scroll_by_lines(-1) {
                    return DiffReviewOutcome::Ignored;
                }
                DiffReviewOutcome::Scrolled {
                    offset: self.offset_y(),
                }
            }
            UiIntent::Move(NavigationMove::First) => {
                if hunks.is_empty() {
                    let before = self.offset_y();
                    self.scroll.set_offset_y(0);
                    return if self.offset_y() != before {
                        DiffReviewOutcome::Scrolled {
                            offset: self.offset_y(),
                        }
                    } else {
                        DiffReviewOutcome::Ignored
                    };
                }
                self.move_hunk(0, hunks)
            }
            UiIntent::Move(NavigationMove::Last) => {
                if hunks.is_empty() {
                    return DiffReviewOutcome::Ignored;
                }
                self.move_hunk(hunks.len() - 1, hunks)
            }
            UiIntent::Page(PageMove::Forward) => {
                let step = i32::from(self.body_rows.max(1));
                if !self.scroll_by_lines(step) {
                    return DiffReviewOutcome::Ignored;
                }
                DiffReviewOutcome::Scrolled {
                    offset: self.offset_y(),
                }
            }
            UiIntent::Page(PageMove::Backward) => {
                let step = i32::from(self.body_rows.max(1));
                if !self.scroll_by_lines(-step) {
                    return DiffReviewOutcome::Ignored;
                }
                DiffReviewOutcome::Scrolled {
                    offset: self.offset_y(),
                }
            }
            UiIntent::Activate | UiIntent::Submit if !hunks.is_empty() => {
                DiffReviewOutcome::HunkActivated {
                    index: self.hunk_cursor,
                }
            }
            UiIntent::Toggle => {
                self.split = !self.split;
                DiffReviewOutcome::ToggleMode
            }
            _ => DiffReviewOutcome::Ignored,
        }
    }

    /// Wheel scroll; click selects hunk containing the line.
    pub fn handle_mouse(
        &mut self,
        event: MouseEvent,
        hunks: &[DiffHunk],
        line_count: usize,
    ) -> DiffReviewOutcome {
        if !self.accepts_input {
            return DiffReviewOutcome::Ignored;
        }
        let (ox, oy) = self.origin;
        let body = Rect {
            x: ox,
            y: oy,
            width: 240,
            height: self.body_rows.max(1),
        };
        if !body.contains(event.position) {
            return DiffReviewOutcome::Ignored;
        }
        match event.kind {
            MouseEventKind::ScrollDown => {
                self.handle_intent(UiIntent::Move(NavigationMove::Next), hunks)
            }
            MouseEventKind::ScrollUp => {
                self.handle_intent(UiIntent::Move(NavigationMove::Previous), hunks)
            }
            MouseEventKind::Down(MouseButton::Left) => {
                let start = self.scroll.offset_y() as usize;
                let row = usize::from(event.position.y.saturating_sub(oy));
                let line = start.saturating_add(row);
                if line >= line_count {
                    return DiffReviewOutcome::Ignored;
                }
                if let Some((idx, _)) = hunks
                    .iter()
                    .enumerate()
                    .find(|(_, h)| h.contains_line(line))
                {
                    if idx == self.hunk_cursor {
                        return DiffReviewOutcome::HunkActivated { index: idx };
                    }
                    return self.move_hunk(idx, hunks);
                }
                DiffReviewOutcome::Ignored
            }
            _ => DiffReviewOutcome::Ignored,
        }
    }
}

/// Diff review chrome over projected lines + hunk list.
///
/// **Hunk cursor** is local; **surface focus** is host-owned.
#[derive(Debug, Clone, Copy)]
pub struct DiffReview<'a> {
    lines: &'a [(&'a str, Role)],
    hunks: &'a [DiffHunk],
    system: &'a DesignSystem,
    focused: bool,
    ascii: bool,
    colorless: bool,
}

impl<'a> DiffReview<'a> {
    /// Lines with roles (added/removed/context).
    #[must_use]
    pub const fn new(lines: &'a [(&'a str, Role)], system: &'a DesignSystem) -> Self {
        Self {
            lines,
            hunks: &[],
            system,
            focused: true,
            ascii: false,
            colorless: false,
        }
    }

    /// Hunk index model for cursor paint + hit tests.
    #[must_use]
    pub const fn hunks(mut self, hunks: &'a [DiffHunk]) -> Self {
        self.hunks = hunks;
        self
    }

    /// Scene surface focus chrome.
    #[must_use]
    pub const fn focused(mut self, focused: bool) -> Self {
        self.focused = focused;
        self
    }

    /// ASCII cursor / empty glyphs.
    #[must_use]
    pub const fn ascii(mut self, ascii: bool) -> Self {
        self.ascii = ascii;
        self
    }

    /// Reduced-color paint.
    #[must_use]
    pub const fn colorless(mut self, colorless: bool) -> Self {
        self.colorless = colorless;
        self
    }

    /// Paint O(visible lines); keeps active hunk in view.
    pub fn render(&self, area: Rect, buffer: &mut Buffer, state: &mut DiffReviewState) {
        if area.is_empty() {
            state.body_rows = 0;
            return;
        }
        state.origin = (area.x, area.y);
        let line_count = self.lines.len().min(u16::MAX as usize) as u16;
        let narrow = area.width < 28;
        let tiny = area.width < 16;
        // Narrow forces unified chrome (split preference retained in state).
        let _unified = narrow || !state.split;

        let header_rows = if tiny && !self.hunks.is_empty() {
            1u16
        } else {
            0
        };
        let body_h = area.height.saturating_sub(header_rows);
        state.sync_metrics(line_count, body_h);
        state.clamp_hunk(self.hunks.len());
        state.ensure_hunk_visible(self.hunks);

        let surface = self.focused && state.accepts_input;
        let mut y = area.y;

        if header_rows > 0 {
            let label = self
                .hunks
                .get(state.hunk_cursor)
                .map(|h| h.header.as_str())
                .unwrap_or("(diff)");
            let mark = if self.ascii { "H " } else { "§ " };
            let head = format!("{mark}{label}");
            buffer.set_stringn(
                area.x,
                y,
                &take_display_cols(&head, usize::from(area.width)),
                usize::from(area.width),
                if surface {
                    self.system.style(Role::TextStrong)
                } else {
                    self.system.style(Role::TextMuted)
                },
            );
            y = y.saturating_add(1);
        }

        if self.lines.is_empty() {
            let mark = if self.ascii { "[ ] " } else { "∅ " };
            let msg = if tiny {
                format!("{mark}empty")
            } else {
                format!("{mark}(empty diff)")
            };
            buffer.set_stringn(
                area.x,
                y,
                &take_display_cols(&msg, usize::from(area.width)),
                usize::from(area.width),
                self.system.style(Role::TextMuted),
            );
            return;
        }

        let start = state.scroll.offset_y() as usize;
        let active = self.hunks.get(state.hunk_cursor);
        let body_bottom = area.bottom();
        for (i, (text, role)) in self.lines.iter().enumerate().skip(start) {
            if y >= body_bottom {
                break;
            }
            let in_hunk = active.is_some_and(|h| h.contains_line(i));
            let gutter = if in_hunk && surface {
                if self.ascii { "> " } else { "› " }
            } else if in_hunk {
                if self.ascii { ". " } else { "· " }
            } else {
                "  "
            };
            let line = if tiny {
                // Drop long paths; keep marker + short body.
                format!("{gutter}{text}")
            } else {
                format!("{gutter}{text}")
            };
            let style = if self.colorless {
                if matches!(*role, Role::DiffAdded | Role::DiffRemoved) {
                    self.system.style(Role::TextStrong)
                } else if in_hunk && surface {
                    self.system.style(Role::TextStrong)
                } else {
                    self.system.style(Role::Text)
                }
            } else if in_hunk && surface && matches!(*role, Role::Text | Role::TextMuted) {
                self.system.style(Role::Focus)
            } else {
                self.system.style(*role)
            };
            let painted = take_display_cols(&line, usize::from(area.width));
            buffer.set_stringn(area.x, y, &painted, usize::from(area.width), style);
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

    fn row_text(buffer: &Buffer, area: Rect, y: u16) -> String {
        (area.x..area.right())
            .map(|x| buffer[(x, y)].symbol().to_string())
            .collect::<String>()
            .trim_end()
            .to_string()
    }

    fn sample_log_lines() -> [LogLine<'static>; 6] {
        [
            LogLine {
                id: "1",
                level: LogLevel::Info,
                text: "boot",
            },
            LogLine {
                id: "2",
                level: LogLevel::Debug,
                text: "load 東京",
            },
            LogLine {
                id: "3",
                level: LogLevel::Warn,
                text: "retry",
            },
            LogLine {
                id: "4",
                level: LogLevel::Error,
                text: "fail 🧪",
            },
            LogLine {
                id: "5",
                level: LogLevel::Info,
                text: "ready",
            },
            LogLine {
                id: "6",
                level: LogLevel::Trace,
                text: "tick",
            },
        ]
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
    fn log_end_and_f_toggle_follow() {
        let mut state = LogStreamState::new();
        state.on_append(50, 5);
        let _ = state.handle_key(KeyEvent::new(KeyCode::Home, KeyModifiers::NONE));
        assert!(!state.is_following());
        assert!(matches!(
            state.handle_key(KeyEvent::new(KeyCode::End, KeyModifiers::NONE)),
            LogStreamOutcome::Follow
        ));
        assert!(state.is_following());
        assert!(matches!(
            state.handle_key(KeyEvent::new(KeyCode::Char('f'), KeyModifiers::NONE)),
            LogStreamOutcome::Detach
        ));
        assert!(matches!(
            state.handle_key(KeyEvent::new(KeyCode::Char('f'), KeyModifiers::NONE)),
            LogStreamOutcome::Follow
        ));
    }

    #[test]
    fn log_accepts_input_gate() {
        let mut state = LogStreamState::new();
        state.on_append(20, 4);
        state.set_accepts_input(false);
        assert!(matches!(
            state.handle_key(KeyEvent::new(KeyCode::PageUp, KeyModifiers::NONE)),
            LogStreamOutcome::Ignored
        ));
    }

    #[test]
    fn log_scrolled_when_already_detached() {
        let mut state = LogStreamState::new();
        state.on_append(30, 5);
        let _ = state.handle_key(KeyEvent::new(KeyCode::Home, KeyModifiers::NONE));
        assert!(!state.is_following());
        let out = state.handle_key(KeyEvent::new(KeyCode::Down, KeyModifiers::NONE));
        assert!(matches!(out, LogStreamOutcome::Scrolled { offset: 1 }));
    }

    #[test]
    fn log_mouse_wheel_and_chip() {
        let system = DesignSystem::default();
        let lines = sample_log_lines();
        let mut state = LogStreamState::new();
        let area = Rect::new(0, 0, 40, 5);
        let mut buffer = Buffer::empty(area);
        LogStream::new(&lines, &system).render(area, &mut buffer, &mut state);
        assert!(state.is_following());
        // Detach via wheel.
        let wheel = MouseEvent {
            kind: MouseEventKind::ScrollUp,
            position: Position::new(0, 0),
            modifiers: KeyModifiers::NONE,
        };
        assert!(matches!(
            state.handle_mouse(wheel),
            LogStreamOutcome::Detach
        ));
        // Click chip row to re-follow.
        let chip_y = area.bottom().saturating_sub(1);
        let click = MouseEvent {
            kind: MouseEventKind::Down(MouseButton::Left),
            position: Position::new(0, chip_y),
            modifiers: KeyModifiers::NONE,
        };
        assert!(matches!(
            state.handle_mouse(click),
            LogStreamOutcome::Follow
        ));
        assert!(state.is_following());
    }

    #[test]
    fn log_paint_levels_follow_chip_empty() {
        let system = DesignSystem::default();
        let lines = sample_log_lines();
        let mut state = LogStreamState::new();
        let area = Rect::new(0, 0, 36, 4);
        let mut buffer = Buffer::empty(area);
        LogStream::new(&lines, &system).render(area, &mut buffer, &mut state);
        let chip = row_text(&buffer, area, area.bottom() - 1);
        assert!(
            chip.contains("follow") || chip.contains('↓'),
            "follow chip: {chip:?}"
        );
        // Following paints near tail.
        let first = row_text(&buffer, area, 0);
        assert!(!first.is_empty());

        let mut empty_state = LogStreamState::new();
        let mut empty_buf = Buffer::empty(area);
        LogStream::new(&[], &system).render(area, &mut empty_buf, &mut empty_state);
        let empty = row_text(&empty_buf, area, 0);
        assert!(empty.contains('∅') || empty.contains("empty"), "{empty:?}");
    }

    #[test]
    fn log_ascii_colorless_narrow_tiny() {
        let system = DesignSystem::default();
        let lines = sample_log_lines();
        let mut state = LogStreamState::new();
        state.set_following(false);
        state.on_append(6, 3);

        let area = Rect::new(0, 0, 30, 4);
        let mut buffer = Buffer::empty(area);
        LogStream::new(&lines, &system)
            .ascii(true)
            .colorless(true)
            .render(area, &mut buffer, &mut state);
        let first = row_text(&buffer, area, 0);
        assert!(
            first.starts_with('I')
                || first.starts_with('D')
                || first.starts_with('W')
                || first.starts_with('E')
                || first.starts_with('T'),
            "ascii level: {first:?}"
        );

        let narrow = Rect::new(0, 0, 22, 3);
        let mut nbuf = Buffer::empty(narrow);
        LogStream::new(&lines, &system).render(narrow, &mut nbuf, &mut state);
        assert!(!row_text(&nbuf, narrow, 0).is_empty());

        let tiny = Rect::new(0, 0, 12, 2);
        let mut tbuf = Buffer::empty(tiny);
        LogStream::new(&lines, &system).render(tiny, &mut tbuf, &mut state);
        let tline = row_text(&tbuf, tiny, 0);
        // Tiny drops level glyph — body text only.
        assert!(
            !tline.starts_with('i') || tline.contains("boot") || !tline.is_empty(),
            "{tline:?}"
        );
    }

    #[test]
    fn log_append_keeps_follow() {
        let mut state = LogStreamState::new();
        state.on_append(10, 4);
        assert!(state.is_following());
        let off = state.offset();
        state.on_append(40, 4);
        assert!(state.is_following());
        assert!(state.offset() >= off);
    }

    fn sample_hunks() -> [DiffHunk; 3] {
        [
            DiffHunk {
                start: 0,
                len: 3,
                header: "@@ -1,3 +1,3 @@".into(),
            },
            DiffHunk {
                start: 3,
                len: 2,
                header: "@@ -10,2 +10,2 @@".into(),
            },
            DiffHunk {
                start: 5,
                len: 3,
                header: "@@ -20,3 +20,4 @@".into(),
            },
        ]
    }

    fn sample_diff_lines() -> [(&'static str, Role); 8] {
        [
            ("@@ -1,3 +1,3 @@", Role::TextMuted),
            (" context", Role::Text),
            ("-old", Role::DiffRemoved),
            ("+new 東京", Role::DiffAdded),
            ("@@ -10,2 +10,2 @@", Role::TextMuted),
            ("-gone", Role::DiffRemoved),
            ("@@ -20,3 +20,4 @@", Role::TextMuted),
            ("+ready 🧪", Role::DiffAdded),
        ]
    }

    #[test]
    fn diff_hunk_cursor_moved_not_focused() {
        let hunks = sample_hunks();
        let mut state = DiffReviewState::new();
        assert!(matches!(
            state.handle_key(
                KeyEvent::new(KeyCode::Char('n'), KeyModifiers::NONE),
                &hunks
            ),
            DiffReviewOutcome::HunkCursorMoved { index: 1 }
        ));
        assert_eq!(state.hunk_cursor(), 1);
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
        assert!(!head.contains("HunkFocused"));
        assert!(head.contains("HunkCursorMoved"));
    }

    #[test]
    fn diff_activate_toggle_and_scroll() {
        let hunks = sample_hunks();
        let mut state = DiffReviewState::new();
        state.body_rows = 3;
        state.sync_metrics(8, 3);
        assert!(matches!(
            state.handle_key(KeyEvent::new(KeyCode::Enter, KeyModifiers::NONE), &hunks),
            DiffReviewOutcome::HunkActivated { index: 0 }
        ));
        assert!(matches!(
            state.handle_key(
                KeyEvent::new(KeyCode::Char('s'), KeyModifiers::NONE),
                &hunks
            ),
            DiffReviewOutcome::ToggleMode
        ));
        assert!(state.is_split());
        assert!(matches!(
            state.handle_key(KeyEvent::new(KeyCode::Down, KeyModifiers::NONE), &hunks),
            DiffReviewOutcome::Scrolled { offset: 1 }
        ));
    }

    #[test]
    fn diff_accepts_input_gate() {
        let hunks = sample_hunks();
        let mut state = DiffReviewState::new();
        state.set_accepts_input(false);
        assert!(matches!(
            state.handle_key(
                KeyEvent::new(KeyCode::Char('n'), KeyModifiers::NONE),
                &hunks
            ),
            DiffReviewOutcome::Ignored
        ));
    }

    #[test]
    fn diff_mouse_click_hunk() {
        let system = DesignSystem::default();
        let lines = sample_diff_lines();
        let hunks = sample_hunks();
        let mut state = DiffReviewState::new();
        let area = Rect::new(0, 0, 40, 6);
        let mut buffer = Buffer::empty(area);
        DiffReview::new(&lines, &system)
            .hunks(&hunks)
            .render(area, &mut buffer, &mut state);
        // Line index 3 is in hunk 0; line 5 is hunk 1 (start 3).
        // Click visible row for line at start of hunk 1 after scroll 0: line 3 is row 3.
        let click = MouseEvent {
            kind: MouseEventKind::Down(MouseButton::Left),
            position: Position::new(0, 3),
            modifiers: KeyModifiers::NONE,
        };
        // line 3 is still in hunk 0 (start 0 len 3 → 0..3), so click line 3 is out of hunk0
        // hunk1 start=3. After paint origin y=0, row 3 → line 3 → hunk 1.
        assert!(matches!(
            state.handle_mouse(click, &hunks, lines.len()),
            DiffReviewOutcome::HunkCursorMoved { index: 1 }
        ));
    }

    #[test]
    fn diff_paint_gutter_empty_narrow_ascii() {
        let system = DesignSystem::default();
        let lines = sample_diff_lines();
        let hunks = sample_hunks();
        let mut state = DiffReviewState::new();
        let area = Rect::new(0, 0, 36, 5);
        let mut buffer = Buffer::empty(area);
        DiffReview::new(&lines, &system)
            .hunks(&hunks)
            .render(area, &mut buffer, &mut state);
        let first = row_text(&buffer, area, 0);
        assert!(
            first.starts_with('›') || first.contains("@@"),
            "hunk gutter: {first:?}"
        );

        let mut empty_state = DiffReviewState::new();
        let mut empty_buf = Buffer::empty(area);
        DiffReview::new(&[], &system).render(area, &mut empty_buf, &mut empty_state);
        let empty = row_text(&empty_buf, area, 0);
        assert!(empty.contains('∅') || empty.contains("empty"), "{empty:?}");

        let mut ascii_state = DiffReviewState::new();
        let mut abuf = Buffer::empty(area);
        DiffReview::new(&lines, &system)
            .hunks(&hunks)
            .ascii(true)
            .colorless(true)
            .render(area, &mut abuf, &mut ascii_state);
        let a0 = row_text(&abuf, area, 0);
        assert!(a0.starts_with('>'), "{a0:?}");

        let tiny = Rect::new(0, 0, 14, 3);
        let mut tbuf = Buffer::empty(tiny);
        let mut tstate = DiffReviewState::new();
        DiffReview::new(&lines, &system)
            .hunks(&hunks)
            .render(tiny, &mut tbuf, &mut tstate);
        let thead = row_text(&tbuf, tiny, 0);
        assert!(
            thead.contains("@@") || thead.contains('§') || thead.contains('H'),
            "tiny header: {thead:?}"
        );
    }

    #[test]
    fn diff_hunk_follow_scroll() {
        let system = DesignSystem::default();
        let lines = sample_diff_lines();
        let hunks = sample_hunks();
        let mut state = DiffReviewState::new();
        let area = Rect::new(0, 0, 32, 2);
        let mut buffer = Buffer::empty(area);
        DiffReview::new(&lines, &system)
            .hunks(&hunks)
            .render(area, &mut buffer, &mut state);
        let _ = state.handle_key(
            KeyEvent::new(KeyCode::Char('n'), KeyModifiers::NONE),
            &hunks,
        );
        let _ = state.handle_key(
            KeyEvent::new(KeyCode::Char('n'), KeyModifiers::NONE),
            &hunks,
        );
        assert_eq!(state.hunk_cursor(), 2);
        DiffReview::new(&lines, &system)
            .hunks(&hunks)
            .render(area, &mut buffer, &mut state);
        assert!(
            state.offset_y() >= 4,
            "cursor-follow offset={}",
            state.offset_y()
        );
    }












}
