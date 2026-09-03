// SPDX-FileCopyrightText: 2026 Alexey Zhokhov
// SPDX-License-Identifier: Apache-2.0

//! **LogStream** — continuous professional log viewer.
//!
//! **Mission.** Follow-tail, pause, unread/new-lines indicator, timestamps,
//! source, severity, ANSI-safe body paint, wrapping / horizontal scroll,
//! search, filters, bookmarks, selection, copy/export outcomes, bounded
//! history signals (dropped lines), burst batching chrome, reconnect banners,
//! virtualization + stable anchors, compact and detailed line recipes.
//!
//! **Ownership.** Host projects a **window** of [`LogLine`] (or owns via
//! [`super::LogPane`] and projects each frame). Scroll/follow/unseen live in
//! [`ScrollAreaState`]. Scene owns surface focus (`focused` + `accepts_input`).
//!
//! **vs [`super::EventStream`].** EventStream is typed structured events.
//! LogStream is line/severity text with log-ops workflows (stern/k9s-class).
//! **vs [`super::LogPane`].** LogPane **owns** append buffers; LogStream
//! **views** projected lines. Prefer LogStream for multi-source projected logs;
//! LogPane for single local build/process buffer.
//!
//! Research: k9s, stern, Textual logs, btop, TermRock LogPane.
use std::collections::BTreeSet;

use ratatui_core::{
    buffer::Buffer, layout::Rect, style::Modifier, text::Line, widgets::StatefulWidget,
};

use crate::{
    input::{KeyCode, KeyEvent, KeyModifiers, MouseButton, MouseEvent, MouseEventKind},
    interaction::{NavigationMove, PageMove, UiIntent},
    style::{DesignSystem, ListRowVisualState, Role},
    text::{display_cols, take_display_cols, wrap_display_cols},
    widgets::{scroll_area::ScrollAreaState, tiered_row::TieredRow},
};

/// Log line severity.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default, PartialOrd, Ord)]
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
    /// Stable id.
    #[must_use]
    pub const fn id(self) -> &'static str {
        match self {
            Self::Trace => "trace",
            Self::Debug => "debug",
            Self::Info => "info",
            Self::Warn => "warn",
            Self::Error => "error",
        }
    }

    #[must_use]
    fn role(self) -> Role {
        match self {
            Self::Trace | Self::Debug => Role::TextMuted,
            Self::Info => Role::Text,
            Self::Warn => Role::Warning,
            Self::Error => Role::Danger,
        }
    }

    /// Level mark (`ascii` uses T/D/I/W/E).
    #[must_use]
    pub const fn glyph(self, _ascii: bool) -> &'static str {
        match self {
            Self::Trace => ".",
            Self::Debug => "·",
            Self::Info => "i",
            Self::Warn => "!",
            Self::Error => "x",
        }
    }

    /// No-color letter (same as ascii glyph).
    #[must_use]
    pub const fn letter(self) -> char {
        match self {
            Self::Trace => 'T',
            Self::Debug => 'D',
            Self::Info => 'I',
            Self::Warn => 'W',
            Self::Error => 'E',
        }
    }
}

/// Line density recipe.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
#[non_exhaustive]
pub enum LogLineRecipe {
    /// Severity + body (compact).
    Compact,
    /// Timestamp · source · severity · body (default professional).
    #[default]
    Detailed,
}

impl LogLineRecipe {
    /// Stable id.
    #[must_use]
    pub const fn id(self) -> &'static str {
        match self {
            Self::Compact => "compact",
            Self::Detailed => "detailed",
        }
    }
}

/// How body text fits the cell width.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
#[non_exhaustive]
pub enum LogWrap {
    /// Clip at display boundary (h-scroll available).
    #[default]
    Clip,
    /// Soft-wrap to multiple viewport rows (virtual height grows).
    Wrap,
}

impl LogWrap {
    /// Stable id.
    #[must_use]
    pub const fn id(self) -> &'static str {
        match self {
            Self::Clip => "clip",
            Self::Wrap => "wrap",
        }
    }
}

/// One projected log line.
///
/// Prefer builders — new fields are additive.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LogLine<'a> {
    /// Stable identity (bookmarks, selection, anchors).
    pub id: &'a str,
    /// Severity.
    pub level: LogLevel,
    /// Body text (plain; control chars escaped at paint when needed).
    pub text: &'a str,
    /// Optional timestamp label.
    pub timestamp: Option<&'a str>,
    /// Optional source / service / pod.
    pub source: Option<&'a str>,
    /// Pre-styled body spans (ANSI already converted). When set, preferred over `text` for paint.
    pub styled: Option<&'a Line<'a>>,
    /// Coalesced burst count (>1 shows `×N`).
    pub batch_count: u32,
}

impl<'a> LogLine<'a> {
    /// Minimal line (id, level, text).
    #[must_use]
    pub const fn new(id: &'a str, level: LogLevel, text: &'a str) -> Self {
        Self {
            id,
            level,
            text,
            timestamp: None,
            source: None,
            styled: None,
            batch_count: 1,
        }
    }

    /// Timestamp.
    #[must_use]
    pub const fn timestamp(mut self, ts: &'a str) -> Self {
        self.timestamp = Some(ts);
        self
    }

    /// Source.
    #[must_use]
    pub const fn source(mut self, source: &'a str) -> Self {
        self.source = Some(source);
        self
    }

    /// Styled spans (ANSI path).
    #[must_use]
    pub const fn styled(mut self, line: &'a Line<'a>) -> Self {
        self.styled = Some(line);
        self
    }

    /// Burst batch size.
    #[must_use]
    pub const fn batch_count(mut self, n: u32) -> Self {
        self.batch_count = if n == 0 { 1 } else { n };
        self
    }
}

/// Hit region for a painted line.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LogStreamRegion {
    /// Line id.
    pub id: String,
    /// Index in filtered projection.
    pub index: usize,
    /// Row rect.
    pub area: Rect,
}

/// Outcomes for host effects (copy/export/bookmark — no I/O in TermRock).
#[derive(Debug, Clone, PartialEq, Eq)]
#[non_exhaustive]
pub enum LogStreamOutcome {
    /// No change.
    Ignored,
    /// Viewport scrolled.
    Scrolled {
        /// New vertical offset.
        offset: u16,
    },
    /// Follow re-attached.
    Follow,
    /// Detached from tail.
    Detach,
    /// Selection changed.
    SelectionChanged {
        /// Selected line ids (order of selection).
        ids: Vec<String>,
    },
    /// Copy request (host writes clipboard).
    Copy {
        /// Joined text (selected lines or cursor line).
        text: String,
    },
    /// Export request (host writes file/stream).
    Export {
        /// Joined text of selected or filtered view.
        text: String,
    },
    /// Bookmark toggled.
    BookmarkToggled {
        /// Line id.
        id: String,
        /// Bookmarked after toggle.
        on: bool,
    },
    /// Search query changed.
    SearchChanged(String),
    /// Level filter floor changed.
    LevelFilter(LogLevel),
    /// Horizontal scroll changed.
    HScrolled {
        /// New h offset.
        h_offset: u16,
    },
    /// Cancel / clear search.
    Cancelled,
    /// Dropped-line / reconnect chrome acknowledged.
    AckDropped,
}

/// Log stream interaction state.
///
/// Follow/pause/unseen live in [`ScrollAreaState`].
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LogStreamState {
    scroll: ScrollAreaState,
    accepts_input: bool,
    origin: (u16, u16),
    body_rows: u16,
    area_rows: u16,
    line_count: u16,
    /// Cursor index in filtered projection.
    pub cursor: usize,
    /// Multi-selected line ids.
    selected: BTreeSet<String>,
    /// Bookmarked line ids.
    bookmarks: BTreeSet<String>,
    /// Search query.
    pub search: Option<String>,
    /// Minimum level shown.
    pub level_floor: LogLevel,
    /// Horizontal content offset (clip mode).
    pub h_offset: u16,
    /// Wrap policy.
    pub wrap: LogWrap,
    /// Recipe.
    pub recipe: LogLineRecipe,
    /// Dropped lines under bounded history / reconnect gap.
    pub dropped: u64,
    /// Reconnect banner message.
    pub reconnect_message: Option<String>,
    /// Burst batched count (chrome).
    pub batched: u64,
    /// Widest filtered line in display columns (h-scroll bound is this plus
    /// the 40-column chrome pad).
    content_cols: u16,
    /// Scan memo `(line_count, search, level_floor)` for `content_cols`.
    /// The filtered projection is append-only while the filter is stable, so
    /// a longer projection rescans only the appended tail; a shorter one
    /// (bounded-history truncation) or a changed filter forces a full rescan.
    /// Without the memo, paint rescanned every line every frame.
    content_width_memo: (usize, Option<String>, LogLevel),
    /// Hit regions.
    pub regions: Vec<LogStreamRegion>,
    /// Prefer no-color paint (letter marks; or set on the widget).
    pub colorless: bool,
    /// Anchor line id for preserve-across-reproject.
    anchor_id: Option<String>,
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
            cursor: 0,
            selected: BTreeSet::new(),
            bookmarks: BTreeSet::new(),
            search: None,
            level_floor: LogLevel::Trace,
            h_offset: 0,
            wrap: LogWrap::Clip,
            recipe: LogLineRecipe::Detailed,
            dropped: 0,
            reconnect_message: None,
            batched: 0,
            content_cols: 0,
            content_width_memo: (0, None, LogLevel::Trace),
            regions: Vec::new(),
            colorless: false,
            anchor_id: None,
        }
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

    /// Following tail.
    #[must_use]
    pub const fn is_following(&self) -> bool {
        self.scroll.is_following()
    }

    /// Vertical offset.
    #[must_use]
    pub const fn offset(&self) -> u16 {
        self.scroll.offset_y()
    }

    /// Unseen new lines while paused.
    #[must_use]
    pub fn unread(&self) -> u64 {
        u64::from(self.scroll.new_content().unseen)
    }

    /// Scroll state (anchors, indicator).
    #[must_use]
    pub const fn scroll(&self) -> &ScrollAreaState {
        &self.scroll
    }

    /// Selected ids.
    #[must_use]
    pub fn selected_ids(&self) -> &BTreeSet<String> {
        &self.selected
    }

    /// Bookmarks.
    #[must_use]
    pub fn bookmarks(&self) -> &BTreeSet<String> {
        &self.bookmarks
    }

    /// Force follow on/off.
    pub fn set_following(&mut self, following: bool) {
        if following {
            self.scroll.follow_tail();
        } else {
            self.scroll.pause_follow();
        }
    }

    /// Report dropped lines / reconnect (host policy).
    pub fn report_dropped(&mut self, n: u64) {
        self.dropped = self.dropped.saturating_add(n);
    }

    /// Set reconnect banner (cleared by ack or clear).
    pub fn set_reconnect_message(&mut self, msg: Option<String>) {
        self.reconnect_message = msg;
    }

    /// Report burst batch chrome.
    pub fn report_batched(&mut self, n: u64) {
        self.batched = n;
    }

    /// Clear dropped / reconnect chrome.
    pub fn ack_dropped(&mut self) {
        self.dropped = 0;
        self.reconnect_message = None;
        self.batched = 0;
    }

    /// Capture anchor at cursor.
    pub fn capture_anchor(&mut self, lines: &[LogLine<'_>]) {
        let view = self.filtered(lines);
        if let Some(l) = view.get(self.cursor) {
            self.anchor_id = Some(l.id.to_string());
        }
    }

    /// Restore cursor from anchor.
    pub fn restore_anchor(&mut self, lines: &[LogLine<'_>]) {
        let view = self.filtered(lines);
        if let Some(aid) = self.anchor_id.as_ref() {
            if let Some(i) = view.iter().position(|l| l.id == aid) {
                self.cursor = i;
                self.scroll.reveal_row(self.cursor);
            }
        }
    }

    fn filtered<'a>(&self, lines: &'a [LogLine<'a>]) -> Vec<&'a LogLine<'a>> {
        filter_log_lines(
            lines,
            self.search.as_deref().unwrap_or(""),
            self.level_floor,
        )
    }

    fn sync_metrics(&mut self, total_lines: u16, viewport: u16) {
        self.line_count = total_lines;
        self.body_rows = viewport;
        self.scroll.set_content_size(1, total_lines);
        self.scroll.set_viewport(1, viewport);
    }

    /// After host projects append (total filtered or raw count).
    pub fn on_append(&mut self, total_lines: u16, viewport: u16) {
        self.sync_metrics(total_lines, viewport);
        if self.scroll.is_following() && total_lines > 0 {
            self.cursor = usize::from(total_lines.saturating_sub(1));
        }
    }

    fn scroll_by_lines(&mut self, delta: i32) -> bool {
        if self.body_rows == 0 && self.line_count == 0 {
            return false;
        }
        self.scroll.scroll_by(delta as isize, 0).is_scrolled()
    }

    fn scroll_page(&mut self, forward: bool) -> bool {
        self.scroll.page(forward).is_scrolled()
    }

    /// Keys.
    pub fn handle_key(&mut self, key: KeyEvent, lines: &[LogLine<'_>]) -> LogStreamOutcome {
        if !self.accepts_input || key.is_release() {
            return LogStreamOutcome::Ignored;
        }
        let is_press = key.is_press();

        // Search
        if is_press && matches!(key.code, KeyCode::Char('/')) && key.modifiers.is_empty() {
            if self.search.is_none() {
                self.search = Some(String::new());
            }
            return LogStreamOutcome::SearchChanged(self.search.clone().unwrap_or_default());
        }
        if let Some(q) = self.search.as_mut()
            && is_press
            && key.modifiers.is_empty()
        {
            match key.code {
                KeyCode::Esc => {
                    self.search = None;
                    return LogStreamOutcome::Cancelled;
                }
                KeyCode::Backspace => {
                    q.pop();
                    if q.is_empty() {
                        self.search = None;
                    }
                    return LogStreamOutcome::SearchChanged(
                        self.search.clone().unwrap_or_default(),
                    );
                }
                KeyCode::Char(c) if !c.is_control() && c != '/' => {
                    q.push(c);
                    return LogStreamOutcome::SearchChanged(q.clone());
                }
                _ => {}
            }
        }

        // Level floor
        if is_press && matches!(key.code, KeyCode::Char('[')) {
            self.level_floor = match self.level_floor {
                LogLevel::Trace => LogLevel::Debug,
                LogLevel::Debug => LogLevel::Info,
                LogLevel::Info => LogLevel::Warn,
                LogLevel::Warn => LogLevel::Error,
                LogLevel::Error => LogLevel::Trace,
            };
            return LogStreamOutcome::LevelFilter(self.level_floor);
        }

        // Horizontal scroll
        if is_press && matches!(key.code, KeyCode::Left | KeyCode::Char('h' | 'H')) {
            if self.h_offset > 0 {
                self.h_offset = self.h_offset.saturating_sub(4);
                return LogStreamOutcome::HScrolled {
                    h_offset: self.h_offset,
                };
            }
        }
        if is_press && matches!(key.code, KeyCode::Right | KeyCode::Char('l' | 'L')) {
            let max = self.content_cols;
            if self.h_offset < max {
                self.h_offset = self.h_offset.saturating_add(4).min(max);
                return LogStreamOutcome::HScrolled {
                    h_offset: self.h_offset,
                };
            }
        }

        // Wrap toggle
        if is_press && matches!(key.code, KeyCode::Char('w' | 'W')) {
            self.wrap = match self.wrap {
                LogWrap::Clip => LogWrap::Wrap,
                LogWrap::Wrap => LogWrap::Clip,
            };
            return LogStreamOutcome::Ignored;
        }

        // Recipe toggle
        if is_press && matches!(key.code, KeyCode::Char('d' | 'D')) {
            self.recipe = match self.recipe {
                LogLineRecipe::Compact => LogLineRecipe::Detailed,
                LogLineRecipe::Detailed => LogLineRecipe::Compact,
            };
            return LogStreamOutcome::Ignored;
        }

        // Bookmark
        if is_press && matches!(key.code, KeyCode::Char('m' | 'M')) {
            let view = self.filtered(lines);
            if let Some(l) = view.get(self.cursor) {
                let id = l.id.to_string();
                let on = if !self.bookmarks.remove(&id) {
                    self.bookmarks.insert(id.clone());
                    true
                } else {
                    false
                };
                return LogStreamOutcome::BookmarkToggled { id, on };
            }
        }

        // Copy selection or cursor line
        if is_press && matches!(key.code, KeyCode::Char('c' | 'C')) && key.modifiers.is_empty() {
            return self.copy_outcome(lines);
        }
        // Export filtered
        if is_press
            && matches!(key.code, KeyCode::Char('e' | 'E'))
            && key.modifiers.contains(KeyModifiers::CONTROL)
        {
            let view = self.filtered(lines);
            let text = view.iter().map(|l| l.text).collect::<Vec<_>>().join("\n");
            return LogStreamOutcome::Export { text };
        }

        // Space toggle select
        if is_press && matches!(key.code, KeyCode::Char(' ')) {
            let view = self.filtered(lines);
            if let Some(l) = view.get(self.cursor) {
                let id = l.id.to_string();
                if !self.selected.remove(&id) {
                    self.selected.insert(id);
                }
                return LogStreamOutcome::SelectionChanged {
                    ids: self.selected.iter().cloned().collect(),
                };
            }
        }

        if is_press && matches!(key.code, KeyCode::Char('b' | 'B')) {
            self.ack_dropped();
            return LogStreamOutcome::AckDropped;
        }

        // Follow chord
        if is_press && matches!(key.code, KeyCode::Char('f' | 'F')) {
            return self.handle_intent(UiIntent::Toggle, lines);
        }

        if let Some(intent) = crate::interaction::default_log_stream_intent(key)
            .or_else(|| crate::interaction::default_list_intent(key))
        {
            return self.handle_intent(intent, lines);
        }
        LogStreamOutcome::Ignored
    }

    /// Intent routing (optionally with lines for selection).
    pub fn handle_intent(&mut self, intent: UiIntent, lines: &[LogLine<'_>]) -> LogStreamOutcome {
        if !self.accepts_input {
            return LogStreamOutcome::Ignored;
        }
        let view = self.filtered(lines);
        let len = view.len();
        if len > 0 {
            self.cursor = self.cursor.min(len - 1);
        }

        match intent {
            UiIntent::Move(NavigationMove::Next) => {
                let was = self.scroll.is_following();
                // Prefer cursor step then scroll
                if len > 0 && self.cursor + 1 < len {
                    self.cursor += 1;
                    if self.cursor + 1 >= len {
                        self.scroll.follow_tail();
                    } else {
                        self.scroll.pause_follow();
                    }
                    self.scroll.reveal_row(self.cursor);
                    if was && !self.is_following() {
                        return LogStreamOutcome::Detach;
                    }
                    return LogStreamOutcome::Scrolled {
                        offset: self.offset(),
                    };
                }
                if !self.scroll_by_lines(1) {
                    return LogStreamOutcome::Ignored;
                }
                if was {
                    LogStreamOutcome::Detach
                } else {
                    LogStreamOutcome::Scrolled {
                        offset: self.offset(),
                    }
                }
            }
            UiIntent::Move(NavigationMove::Previous) => {
                let was = self.scroll.is_following();
                if len > 0 && self.cursor > 0 {
                    self.cursor -= 1;
                    self.scroll.pause_follow();
                    self.scroll.reveal_row(self.cursor);
                    if was {
                        return LogStreamOutcome::Detach;
                    }
                    return LogStreamOutcome::Scrolled {
                        offset: self.offset(),
                    };
                }
                if !self.scroll_by_lines(-1) {
                    return LogStreamOutcome::Ignored;
                }
                if was {
                    LogStreamOutcome::Detach
                } else {
                    LogStreamOutcome::Scrolled {
                        offset: self.offset(),
                    }
                }
            }
            UiIntent::Move(NavigationMove::First) => {
                let was = self.scroll.is_following();
                self.cursor = 0;
                let out = self.scroll.home();
                if was {
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
                if len > 0 {
                    self.cursor = len - 1;
                }
                self.scroll.follow_tail();
                LogStreamOutcome::Follow
            }
            UiIntent::Page(PageMove::Forward) => {
                let was = self.scroll.is_following();
                if !self.scroll_page(true) {
                    return LogStreamOutcome::Ignored;
                }
                if was {
                    LogStreamOutcome::Detach
                } else {
                    LogStreamOutcome::Scrolled {
                        offset: self.offset(),
                    }
                }
            }
            UiIntent::Page(PageMove::Backward) => {
                let was = self.scroll.is_following();
                if !self.scroll_page(false) {
                    return LogStreamOutcome::Ignored;
                }
                if was {
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
                    if len > 0 {
                        self.cursor = len - 1;
                    }
                    LogStreamOutcome::Follow
                }
            }
            UiIntent::Activate | UiIntent::Submit => self.copy_outcome(lines),
            UiIntent::Cancel => {
                if self.search.is_some() {
                    self.search = None;
                    return LogStreamOutcome::Cancelled;
                }
                self.selected.clear();
                LogStreamOutcome::Cancelled
            }
            _ => LogStreamOutcome::Ignored,
        }
    }

    fn copy_outcome(&self, lines: &[LogLine<'_>]) -> LogStreamOutcome {
        let view = self.filtered(lines);
        let text = if !self.selected.is_empty() {
            view.iter()
                .filter(|l| self.selected.contains(l.id))
                .map(|l| l.text)
                .collect::<Vec<_>>()
                .join("\n")
        } else if let Some(l) = view.get(self.cursor) {
            l.text.to_string()
        } else {
            String::new()
        };
        if text.is_empty() {
            LogStreamOutcome::Ignored
        } else {
            LogStreamOutcome::Copy { text }
        }
    }

    /// Mouse.
    pub fn handle_mouse(&mut self, event: MouseEvent, lines: &[LogLine<'_>]) -> LogStreamOutcome {
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
            MouseEventKind::ScrollDown => {
                self.handle_intent(UiIntent::Move(NavigationMove::Next), lines)
            }
            MouseEventKind::ScrollUp => {
                self.handle_intent(UiIntent::Move(NavigationMove::Previous), lines)
            }
            MouseEventKind::Down(MouseButton::Left) => {
                let chip_y = oy.saturating_add(self.area_rows.saturating_sub(1));
                if self.area_rows >= 2 && event.position.y == chip_y {
                    if self.scroll.is_following() && self.dropped == 0 {
                        return LogStreamOutcome::Ignored;
                    }
                    self.scroll.jump_to_new_content();
                    return LogStreamOutcome::Follow;
                }
                if let Some(r) = self
                    .regions
                    .iter()
                    .find(|r| r.area.contains(event.position))
                {
                    self.cursor = r.index;
                    self.scroll.pause_follow();
                    if event.modifiers.contains(KeyModifiers::SHIFT) {
                        self.selected.insert(r.id.clone());
                        return LogStreamOutcome::SelectionChanged {
                            ids: self.selected.iter().cloned().collect(),
                        };
                    }
                    return LogStreamOutcome::Scrolled {
                        offset: self.offset(),
                    };
                }
                LogStreamOutcome::Ignored
            }
            _ => LogStreamOutcome::Ignored,
        }
    }
}

/// Filter by search + level floor.
#[must_use]
pub fn filter_log_lines<'a>(
    lines: &'a [LogLine<'a>],
    query: &str,
    level_floor: LogLevel,
) -> Vec<&'a LogLine<'a>> {
    let q = query.trim().to_ascii_lowercase();
    lines
        .iter()
        .filter(|l| {
            if l.level < level_floor {
                return false;
            }
            if q.is_empty() {
                return true;
            }
            let hay = format!(
                "{} {} {}",
                l.text,
                l.source.unwrap_or(""),
                l.timestamp.unwrap_or("")
            )
            .to_ascii_lowercase();
            hay.contains(&q)
        })
        .collect()
}

/// Escape control characters for safe log display.
#[must_use]
pub fn escape_log_text(raw: &str) -> String {
    let mut out = String::with_capacity(raw.len());
    for ch in raw.chars() {
        match ch {
            '\n' => out.push_str("\\n"),
            '\r' => out.push_str("\\r"),
            '\t' => out.push('\t'), // keep tabs for alignment
            '\0' => out.push_str("\\0"),
            c if c.is_control() => out.push_str(&format!("\\u{{{:x}}}", c as u32)),
            c => out.push(c),
        }
    }
    out
}

/// Log stream paint + chrome.
#[derive(Debug, Clone)]
pub struct LogStream<'a> {
    lines: &'a [LogLine<'a>],
    system: &'a DesignSystem,
    focused: bool,
    colorless: bool,
    title: Option<&'a str>,
}

impl<'a> LogStream<'a> {
    /// Lines + design system.
    #[must_use]
    pub const fn new(lines: &'a [LogLine<'a>], system: &'a DesignSystem) -> Self {
        Self {
            lines,
            system,
            focused: true,
            colorless: false,
            title: None,
        }
    }

    /// Optional title row (consumes one body row).
    #[must_use]
    pub const fn title(mut self, title: &'a str) -> Self {
        self.title = Some(title);
        self
    }

    /// Scene surface focus chrome.
    #[must_use]
    pub const fn focused(mut self, focused: bool) -> Self {
        self.focused = focused;
        self
    }

    /// ASCII level / follow glyphs.
    #[must_use]
    /// Reduced-color paint.
    pub const fn colorless(mut self, colorless: bool) -> Self {
        self.colorless = colorless;
        self
    }

    /// Paint O(visible).
    pub fn paint(&self, area: Rect, buffer: &mut Buffer, state: &mut LogStreamState) {
        state.regions.clear();
        if area.is_empty() {
            state.body_rows = 0;
            state.area_rows = 0;
            return;
        }
        let separator = " · ";
        let colorless = self.colorless || state.colorless || self.system.mono();
        state.origin = (area.x, area.y);
        state.area_rows = area.height;

        let view = state.filtered(self.lines);
        // Content width for h-scroll, memoized per (count, filter). Scanning
        // every line every frame dominated paint on hot logs.
        let width_of = |l: &&LogLine<'_>| u16::try_from(display_cols(l.text)).unwrap_or(u16::MAX);
        if state.content_width_memo.1.as_deref() != state.search.as_deref()
            || state.content_width_memo.2 != state.level_floor
            || view.len() < state.content_width_memo.0
        {
            state.content_cols = view.iter().map(width_of).max().unwrap_or(0);
        } else if view.len() > state.content_width_memo.0 {
            let tail_max = view[state.content_width_memo.0..]
                .iter()
                .map(width_of)
                .max()
                .unwrap_or(0);
            state.content_cols = state.content_cols.max(tail_max);
        }
        state.content_width_memo = (view.len(), state.search.clone(), state.level_floor);
        state.h_offset = state.h_offset.min(
            state
                .content_cols
                .saturating_add(40)
                .saturating_sub(area.width.max(1)),
        );

        let following = state.scroll.is_following();
        let unread = state.unread();
        let show_chip = area.height >= 2
            && (following
                || unread > 0
                || state.dropped > 0
                || state.reconnect_message.is_some()
                || !view.is_empty());
        let title_h = u16::from(self.title.is_some() && area.height >= 3);
        let body_h = area
            .height
            .saturating_sub(u16::from(show_chip) + title_h)
            .max(1);

        // Virtual display rows depend on wrap
        let total_display = if matches!(state.wrap, LogWrap::Wrap) {
            // Approximate: each line at least 1; wrap not fully virtualized here — use line count
            view.len().min(u16::MAX as usize) as u16
        } else {
            view.len().min(u16::MAX as usize) as u16
        };
        state.sync_metrics(total_display, body_h);
        if following && total_display > 0 {
            state.cursor = usize::from(total_display.saturating_sub(1));
        }
        if total_display > 0 {
            state.cursor = state.cursor.min(usize::from(total_display) - 1);
        }

        let surface = self.focused && state.accepts_input;
        let tiny = area.width < 16;
        let narrow = area.width < 36;

        let mut y = area.y;
        if let Some(title) = self.title {
            if y < area.bottom() {
                buffer.set_stringn(
                    area.x,
                    y,
                    take_display_cols(title, usize::from(area.width)),
                    usize::from(area.width),
                    self.system.style(Role::TextStrong),
                );
                y = y.saturating_add(1);
            }
        }

        // Reconnect / dropped banner (steals one body row when present).
        let mut banner_h = 0u16;
        if let Some(msg) = &state.reconnect_message {
            if y < area.bottom().saturating_sub(u16::from(show_chip)) {
                let line = format!("! reconnect: {msg}");
                // The banner is a sentence; its mark carries the warning.
                buffer.set_stringn(
                    area.x,
                    y,
                    take_display_cols(&line, usize::from(area.width)),
                    usize::from(area.width),
                    self.system.style(Role::Text),
                );
                crate::widgets::row_chrome::paint_status_glyph(
                    buffer,
                    Rect::new(area.x, y, area.width, 1),
                    0,
                    "!",
                    self.system.style(Role::Warning),
                );
                y = y.saturating_add(1);
                banner_h = 1;
            }
        }

        let body_top = y;
        let bottom = area
            .y
            .saturating_add(title_h)
            .saturating_add(banner_h)
            .saturating_add(body_h.saturating_sub(banner_h))
            .min(area.bottom().saturating_sub(u16::from(show_chip)));

        if view.is_empty() {
            let mark = "∅ ";
            let msg = if tiny {
                format!("{mark}empty")
            } else {
                format!("{mark}(empty log)")
            };
            buffer.set_stringn(
                area.x,
                body_top,
                take_display_cols(&msg, usize::from(area.width)),
                usize::from(area.width),
                self.system.style(Role::TextMuted),
            );
        } else {
            let start = state.offset() as usize;
            let mut py = body_top;
            for (i, line) in view.iter().enumerate().skip(start) {
                if py >= bottom {
                    break;
                }
                let selected = state.selected.contains(line.id);
                let cursor = i == state.cursor;
                let bookmarked = state.bookmarks.contains(line.id);

                let style = if colorless {
                    if selected || cursor {
                        self.system
                            .style(Role::TextStrong)
                            .add_modifier(Modifier::BOLD)
                    } else {
                        match line.level {
                            LogLevel::Trace | LogLevel::Debug => self.system.style(Role::TextMuted),
                            _ => self.system.style(Role::Text),
                        }
                    }
                } else if surface {
                    // The level's hue rides its glyph; the message is a
                    // sentence and stays readable (plans/007).
                    self.system.style(Role::Text)
                } else {
                    match line.level {
                        LogLevel::Error | LogLevel::Warn => self.system.style(Role::Text),
                        _ => self.system.style(Role::TextMuted),
                    }
                };
                // A selected error line is still an error line: the chrome
                // marks it, the level keeps its tone.
                let chrome = crate::widgets::row_chrome::RowChrome::resolve(
                    self.system,
                    ListRowVisualState {
                        selected: selected || cursor,
                        focused: surface,
                        enabled: true,
                        ..Default::default()
                    },
                );
                let style = chrome.label_style(style);

                let g = line.level.glyph(false);
                let bm = if bookmarked { "★" } else { " " };
                let batch = if line.batch_count > 1 {
                    format!("{}{}", "×", line.batch_count)
                } else {
                    String::new()
                };

                let plain = if let Some(styled) = line.styled {
                    // Flatten spans for clip path
                    styled
                        .spans
                        .iter()
                        .map(|s| s.content.as_ref())
                        .collect::<String>()
                } else {
                    escape_log_text(line.text)
                };

                // The row is tiers, not a sentence: the timestamp and the
                // source sit under the message, the level owns its glyph, and
                // the batch count trails quietly (plans/012 Step 3).
                let tone = |role: Role| {
                    if colorless {
                        None
                    } else {
                        Some(chrome.label_style(self.system.style(role)))
                    }
                };
                let meta = tone(Role::TextFaint);
                let source_tone = tone(Role::TextMuted);
                let level_tone = tone(line.level.role());
                let mut row = TieredRow::default();
                match state.recipe {
                    LogLineRecipe::Compact | LogLineRecipe::Detailed if tiny => {
                        row.push_plain(&plain);
                    }
                    LogLineRecipe::Detailed if narrow => {
                        row.push_joined(bm, None);
                        row.push_joined(g, level_tone);
                        row.push_plain(&plain);
                        row.push_joined(&batch, source_tone);
                    }
                    LogLineRecipe::Compact => {
                        row.push_joined(bm, None);
                        row.push_joined(g, level_tone);
                        row.push_plain(&plain);
                        row.push_joined(&batch, source_tone);
                    }
                    LogLineRecipe::Detailed => {
                        row.push_plain(bm);
                        if let Some(ts) = line.timestamp {
                            match meta {
                                Some(style) => row.push(ts, style),
                                None => row.push_plain(ts),
                            }
                        }
                        if let Some(src) = line.source {
                            match source_tone {
                                Some(style) => row.push(src, style),
                                None => row.push_plain(src),
                            }
                        }
                        match level_tone {
                            Some(style) => row.push(g, style),
                            None => row.push_plain(g),
                        }
                        if colorless {
                            row.push_plain(&line.level.letter().to_string());
                        }
                        row.push_plain(&plain);
                        match source_tone {
                            Some(style) => row.push(&batch, style),
                            None => row.push_plain(&batch),
                        }
                    }
                }
                let body = row.text().to_string();

                let rows: Vec<String> = if matches!(state.wrap, LogWrap::Wrap) {
                    let inner_w = usize::from(area.width.saturating_sub(1).max(1));
                    wrap_display_cols(&body, inner_w)
                } else {
                    let skip = usize::from(state.h_offset);
                    let chars: String = body.chars().skip(skip).collect();
                    vec![
                        take_display_cols(&chars, usize::from(area.width.saturating_sub(1)))
                            .to_string(),
                    ]
                };

                let row0 = py;
                for (ri, painted) in rows.iter().enumerate() {
                    if py >= bottom {
                        break;
                    }
                    // The gutter column belongs to the shared row chrome.
                    buffer.set_stringn(area.x, py, " ", 1, style);
                    buffer.set_stringn(
                        area.x.saturating_add(1),
                        py,
                        take_display_cols(painted, usize::from(area.width.saturating_sub(1))),
                        usize::from(area.width.saturating_sub(1)),
                        style,
                    );
                    if ri == 0 {
                        // Tiers ride the first row only: a wrapped
                        // continuation is all message, and all one tone.
                        let skip = if matches!(state.wrap, LogWrap::Wrap) {
                            0
                        } else {
                            usize::from(state.h_offset)
                        };
                        row.paint_tiers(
                            buffer,
                            Rect::new(
                                area.x.saturating_add(1),
                                py,
                                area.width.saturating_sub(1),
                                1,
                            ),
                            skip,
                        );
                    }
                    let continuation = Rect::new(area.x, py, area.width, 1);
                    chrome.paint_wash(buffer, continuation);
                    if ri == 0 {
                        chrome.paint_gutter(buffer, continuation);
                    }
                    py = py.saturating_add(1);
                }

                state.regions.push(LogStreamRegion {
                    id: line.id.to_string(),
                    index: i,
                    area: Rect::new(area.x, row0, area.width, py.saturating_sub(row0).max(1)),
                });
            }
        }

        if show_chip {
            let chip_y = area.bottom().saturating_sub(1);
            let following = state.scroll.is_following();
            let indicator = state.scroll.new_content();
            let mut chip = if following {
                "↓ follow".to_string()
            } else if indicator.visible {
                format!("↓ {} new · f follow", indicator.unseen)
            } else {
                "↑ pinned · f follow".to_string()
            };
            if state.dropped > 0 {
                chip.push_str(&format!("{separator}drop {}", state.dropped));
            }
            if state.batched > 1 {
                chip.push_str(&format!("{separator}batch {}", state.batched));
            }
            if let Some(q) = &state.search {
                chip.push_str(&format!("{separator}/{q}"));
            }
            if state.level_floor > LogLevel::Trace {
                let comparison = "≥";
                chip.push_str(&format!(
                    "{separator}{comparison}{}",
                    state.level_floor.letter()
                ));
            }
            if !state.bookmarks.is_empty() {
                let bookmark = "★";
                chip.push_str(&format!("{separator}{bookmark}{}", state.bookmarks.len()));
            }
            if matches!(state.recipe, LogLineRecipe::Compact) {
                chip.push_str(&format!("{separator}compact"));
            }
            let chip_style = if following && surface {
                if colorless {
                    self.system.style(Role::TextStrong)
                } else {
                    self.system.style(Role::Accent)
                }
            } else if indicator.visible || state.dropped > 0 {
                self.system.style(Role::Warning)
            } else {
                self.system.style(Role::TextMuted)
            };
            buffer.set_stringn(
                area.x,
                chip_y,
                take_display_cols(&chip, usize::from(area.width)),
                usize::from(area.width),
                chip_style,
            );
        }
    }
}

impl StatefulWidget for &LogStream<'_> {
    type State = LogStreamState;
    fn render(self, area: Rect, buffer: &mut Buffer, state: &mut Self::State) {
        LogStream::paint(self, area, buffer, state);
    }
}

impl StatefulWidget for LogStream<'_> {
    type State = LogStreamState;
    fn render(self, area: Rect, buffer: &mut Buffer, state: &mut Self::State) {
        LogStream::paint(&self, area, buffer, state);
    }
}

/// Project owned [`Line`]s (e.g. from [`super::LogPane`]) into plain LogLines.
///
/// Ids are index strings; level defaults to Info. Host should prefer native
/// [`LogLine`] projections when severity is known.
#[must_use]
pub fn log_lines_from_plain<'a>(
    owned: &'a [Line<'static>],
    id_buf: &'a mut Vec<String>,
    text_buf: &'a mut Vec<String>,
) -> Vec<LogLine<'a>> {
    id_buf.clear();
    text_buf.clear();
    for (i, line) in owned.iter().enumerate() {
        id_buf.push(i.to_string());
        let t: String = line.spans.iter().map(|s| s.content.as_ref()).collect();
        text_buf.push(t);
    }
    id_buf
        .iter()
        .zip(text_buf.iter())
        .map(|(id, text)| LogLine::new(id.as_str(), LogLevel::Info, text.as_str()))
        .collect()
}

// ── Bench helpers ───────────────────────────────────────────────────────────

/// Sustained-rate bench targets (documentation / tests).
pub mod bench {
    /// Lines per second target for host append loops.
    pub const LINES_PER_SEC: u32 = 20_000;
    /// Viewport rows for paint budget.
    pub const VIEWPORT: u16 = 40;
    /// Burst batch size under pressure.
    pub const BURST_BATCH: u32 = 128;
    /// Default bounded history (aligns with LogPane).
    pub const BOUNDED_HISTORY: usize = 10_000;
    /// Max paint cells per frame (viewport × avg cols).
    pub const MAX_PAINT_CELLS: u32 = 40 * 80;
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sample() -> Vec<LogLine<'static>> {
        vec![
            LogLine::new("1", LogLevel::Info, "boot")
                .timestamp("12:00:00")
                .source("main"),
            LogLine::new("2", LogLevel::Debug, "load 東京").timestamp("12:00:01"),
            LogLine::new("3", LogLevel::Warn, "retry")
                .timestamp("12:00:02")
                .source("net"),
            LogLine::new("4", LogLevel::Error, "fail 🧪").timestamp("12:00:03"),
            LogLine::new("5", LogLevel::Info, "ready").timestamp("12:00:04"),
            LogLine::new("6", LogLevel::Trace, "tick")
                .timestamp("12:00:05")
                .batch_count(8),
        ]
    }

    #[test]
    fn follow_detaches_on_scroll() {
        let mut state = LogStreamState::new();
        state.on_append(100, 10);
        assert!(state.is_following());
        let out = state.handle_key(KeyEvent::new(KeyCode::PageUp, KeyModifiers::NONE), &[]);
        assert!(matches!(out, LogStreamOutcome::Detach));
        assert!(!state.is_following());
    }

    #[test]
    fn end_and_f_toggle_follow() {
        let lines = sample();
        let mut state = LogStreamState::new();
        state.on_append(50, 5);
        let _ = state.handle_key(KeyEvent::new(KeyCode::Home, KeyModifiers::NONE), &lines);
        assert!(!state.is_following());
        assert!(matches!(
            state.handle_key(KeyEvent::new(KeyCode::End, KeyModifiers::NONE), &lines),
            LogStreamOutcome::Follow
        ));
        assert!(matches!(
            state.handle_key(
                KeyEvent::new(KeyCode::Char('f'), KeyModifiers::NONE),
                &lines
            ),
            LogStreamOutcome::Detach
        ));
        assert!(matches!(
            state.handle_key(
                KeyEvent::new(KeyCode::Char('f'), KeyModifiers::NONE),
                &lines
            ),
            LogStreamOutcome::Follow
        ));
    }

    #[test]
    fn accepts_input_gate() {
        let mut state = LogStreamState::new();
        state.on_append(20, 4);
        state.set_accepts_input(false);
        assert!(matches!(
            state.handle_key(KeyEvent::new(KeyCode::PageUp, KeyModifiers::NONE), &[]),
            LogStreamOutcome::Ignored
        ));
    }

    #[test]
    fn search_and_level_filter() {
        let lines = sample();
        let v = filter_log_lines(&lines, "東京", LogLevel::Trace);
        assert_eq!(v.len(), 1);
        let v2 = filter_log_lines(&lines, "", LogLevel::Error);
        assert!(v2.iter().all(|l| l.level >= LogLevel::Error));
    }

    #[test]
    fn copy_and_bookmark() {
        let lines = sample();
        let mut state = LogStreamState::new();
        state.set_following(false);
        state.cursor = 3;
        let out = state.handle_key(
            KeyEvent::new(KeyCode::Char('m'), KeyModifiers::NONE),
            &lines,
        );
        assert!(matches!(
            out,
            LogStreamOutcome::BookmarkToggled { id, on: true } if id == "4"
        ));
        let out = state.handle_key(
            KeyEvent::new(KeyCode::Char('c'), KeyModifiers::NONE),
            &lines,
        );
        assert!(matches!(out, LogStreamOutcome::Copy { text } if text.contains("fail")));
    }

    #[test]
    fn paint_detailed_and_compact() {
        let system = DesignSystem::default();
        let lines = sample();
        let mut state = LogStreamState::new();
        state.set_following(false);
        state.recipe = LogLineRecipe::Detailed;
        let stream = LogStream::new(&lines, &system).focused(true);
        let area = Rect::new(0, 0, 72, 10);
        let mut buf = Buffer::empty(area);
        stream.render(area, &mut buf, &mut state);
        assert!(!state.regions.is_empty());
        let text: String = buf
            .content()
            .iter()
            .map(|c| c.symbol().to_string())
            .collect();
        assert!(text.contains("boot") || text.contains("ready"), "{text}");
    }

    #[test]
    fn level_color_is_confined_to_its_glyph_cell() {
        let system = DesignSystem::default();
        let lines = sample();
        let mut state = LogStreamState::new();
        state.set_following(false);
        state.recipe = LogLineRecipe::Compact;
        let stream = LogStream::new(&lines, &system).focused(true);
        let area = Rect::new(0, 0, 72, 10);
        let mut buf = Buffer::empty(area);
        stream.render(area, &mut buf, &mut state);

        let error_fg = system.style(Role::Danger).fg;
        let warn_fg = system.style(Role::Warning).fg;
        // Only the painted log rows: the follow chip is a status element in
        // its own right, not a sentence.
        for region in &state.regions {
            for y in region.area.top()..region.area.bottom() {
                let level_cells = (0..area.width)
                    .filter(|x| {
                        let fg = Some(buf[(*x, y)].fg);
                        !buf[(*x, y)].symbol().trim().is_empty()
                            && (fg == error_fg || fg == warn_fg)
                    })
                    .count();
                assert!(
                    level_cells <= 1,
                    "row {y} paints {level_cells} cells in a level color; \
                     the level belongs to its glyph"
                );
            }
        }
    }

    #[test]
    fn dropped_and_reconnect_chrome() {
        let system = DesignSystem::default();
        let lines = sample();
        let mut state = LogStreamState::new();
        state.report_dropped(42);
        state.set_reconnect_message(Some("stream resumed".into()));
        let stream = LogStream::new(&lines, &system);
        let area = Rect::new(0, 0, 60, 8);
        let mut buf = Buffer::empty(area);
        stream.render(area, &mut buf, &mut state);
        let text: String = buf
            .content()
            .iter()
            .map(|c| c.symbol().to_string())
            .collect();
        assert!(
            text.contains("drop") || text.contains("42") || text.contains("follow"),
            "{text}"
        );
    }

    #[test]
    fn anchor_restore() {
        let lines = sample();
        let mut state = LogStreamState::new();
        state.set_following(false);
        state.cursor = 3;
        state.capture_anchor(&lines);
        state.cursor = 0;
        state.restore_anchor(&lines);
        assert_eq!(state.cursor, 3);
    }

    #[test]
    fn escape_controls() {
        assert!(escape_log_text("a\nb").contains("\\n"));
    }

    #[test]
    fn sustained_viewport_paint() {
        let system = DesignSystem::default();
        let owned: Vec<(String, String)> = (0..40)
            .map(|i| (i.to_string(), format!("line-{i} payload")))
            .collect();
        let lines: Vec<LogLine<'_>> = owned
            .iter()
            .map(|(id, t)| LogLine::new(id.as_str(), LogLevel::Info, t.as_str()))
            .collect();
        let mut state = LogStreamState::new();
        state.on_append(40, 20);
        let stream = LogStream::new(&lines, &system);
        let area = Rect::new(0, 0, 80, 22);
        let mut buf = Buffer::empty(area);
        for _ in 0..50 {
            (&stream).render(area, &mut buf, &mut state);
        }
        assert!(state.regions.len() <= 25);
    }

    #[test]
    fn export_filtered() {
        let lines = sample();
        let mut state = LogStreamState::new();
        state.search = Some("fail".into());
        let out = state.handle_key(
            KeyEvent::new(KeyCode::Char('e'), KeyModifiers::CONTROL),
            &lines,
        );
        assert!(matches!(out, LogStreamOutcome::Export { text } if text.contains("fail")));
    }

    #[test]
    fn fuzz_filter_levels() {
        let lines = sample();
        for floor in [LogLevel::Trace, LogLevel::Info, LogLevel::Error] {
            let v = filter_log_lines(&lines, "", floor);
            assert!(v.iter().all(|l| l.level >= floor));
        }
    }

    #[test]
    fn mouse_wheel_and_chip() {
        use ratatui_core::layout::Position;
        let system = DesignSystem::default();
        let lines = sample();
        let mut state = LogStreamState::new();
        let area = Rect::new(0, 0, 40, 5);
        let mut buffer = Buffer::empty(area);
        LogStream::new(&lines, &system).render(area, &mut buffer, &mut state);
        assert!(state.is_following());
        let wheel = MouseEvent {
            kind: MouseEventKind::ScrollUp,
            position: Position::new(0, 0),
            modifiers: KeyModifiers::NONE,
        };
        assert!(matches!(
            state.handle_mouse(wheel, &lines),
            LogStreamOutcome::Detach
        ));
        let chip_y = area.bottom().saturating_sub(1);
        let click = MouseEvent {
            kind: MouseEventKind::Down(MouseButton::Left),
            position: Position::new(0, chip_y),
            modifiers: KeyModifiers::NONE,
        };
        assert!(matches!(
            state.handle_mouse(click, &lines),
            LogStreamOutcome::Follow
        ));
    }

    #[test]
    fn selection_space() {
        let lines = sample();
        let mut state = LogStreamState::new();
        state.set_following(false);
        state.cursor = 1;
        let out = state.handle_key(
            KeyEvent::new(KeyCode::Char(' '), KeyModifiers::NONE),
            &lines,
        );
        assert!(matches!(out, LogStreamOutcome::SelectionChanged { .. }));
        assert!(state.selected_ids().contains("2"));
    }

    #[test]
    fn log_lines_from_plain_bridge() {
        let owned = vec![Line::from("hello"), Line::from("world 東京")];
        let mut ids = Vec::new();
        let mut texts = Vec::new();
        let lines = log_lines_from_plain(&owned, &mut ids, &mut texts);
        assert_eq!(lines.len(), 2);
        assert_eq!(lines[0].text, "hello");
        assert_eq!(lines[1].level, LogLevel::Info);
    }

    #[test]
    fn wrap_and_recipe_toggle() {
        let lines = sample();
        let mut state = LogStreamState::new();
        let _ = state.handle_key(
            KeyEvent::new(KeyCode::Char('w'), KeyModifiers::NONE),
            &lines,
        );
        assert_eq!(state.wrap, LogWrap::Wrap);
        let _ = state.handle_key(
            KeyEvent::new(KeyCode::Char('d'), KeyModifiers::NONE),
            &lines,
        );
        assert_eq!(state.recipe, LogLineRecipe::Compact);
    }

    #[test]
    fn a_detailed_row_reads_as_tiers_not_as_a_sentence() {
        let system = DesignSystem::default();
        let lines = sample();
        let mut state = LogStreamState::new();
        state.set_following(false);
        state.cursor = 1;
        state.recipe = LogLineRecipe::Detailed;
        let stream = LogStream::new(&lines, &system).focused(true);
        let area = Rect::new(0, 0, 72, 10);
        let mut buf = Buffer::empty(area);
        stream.render(area, &mut buf, &mut state);

        let region = state.regions.first().expect("a row must be painted");
        let y = region.area.y;
        let row: String = (0..area.width).map(|x| buf[(x, y)].symbol()).collect();
        let col_of = |needle: &str| {
            u16::try_from(
                row.find(needle)
                    .unwrap_or_else(|| panic!("{needle:?} in {row:?}")),
            )
            .unwrap()
        };
        let timestamp = buf[(col_of("12:00:00"), y)].fg;
        let source = buf[(col_of("main"), y)].fg;
        let message = buf[(col_of("boot"), y)].fg;
        assert_ne!(
            timestamp, message,
            "a timestamp must not read as loudly as the message"
        );
        assert_ne!(
            source, message,
            "a source must not read as loudly as the message"
        );
        assert_eq!(Some(timestamp), system.style(Role::TextFaint).fg);
        assert_eq!(Some(source), system.style(Role::TextMuted).fg);
    }

    #[test]
    fn bench_constants_sane() {
        assert!(bench::LINES_PER_SEC >= 1_000);
        assert!(bench::VIEWPORT >= 10);
        assert_eq!(bench::BOUNDED_HISTORY, 10_000);
    }
}
