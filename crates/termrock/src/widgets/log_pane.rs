//! Append-oriented scrollback deliberately owns its buffered lines.
//!
//! Unlike projection widgets that borrow an application model each frame,
//! [`LogPaneState`] receives a stream over time. Owning that bounded history
//! keeps eviction, frozen-view offsets, and tail-follow transitions atomic.

use std::fmt::Write as _;

use ratatui_core::{buffer::Buffer, layout::Rect, text::Line, widgets::StatefulWidget};

use crate::{
    input::{KeyCode, KeyEvent, KeyEventKind},
    interaction::Outcome,
    scroll::{DialogScroll, TailScroll, max_offset},
    style::{Role, Theme},
    text::display_cols,
};

use super::Viewport;

/// Default maximum number of retained log lines.
pub const DEFAULT_LOG_HISTORY_LINES: usize = 10_000;

#[derive(Debug, Clone)]
/// Runtime state for `LogPane`.
pub struct LogPaneState {
    lines: Vec<Line<'static>>,
    history_start: usize,
    tail: TailScroll,
    follow: bool,
    pending_oldest: bool,
    max_lines: Option<usize>,
    viewport_height: usize,
    scroll_indicator: String,
}

impl Default for LogPaneState {
    fn default() -> Self {
        Self::new()
    }
}

impl LogPaneState {
    #[must_use]
    /// Creates an empty bounded log that follows the live tail.
    pub const fn new() -> Self {
        Self {
            lines: Vec::new(),
            history_start: 0,
            tail: TailScroll::new(0),
            follow: true,
            pending_oldest: false,
            max_lines: Some(DEFAULT_LOG_HISTORY_LINES),
            viewport_height: 0,
            scroll_indicator: String::new(),
        }
    }

    #[must_use]
    /// Returns this value with `max_lines` configured.
    pub const fn with_max_lines(mut self, max_lines: usize) -> Self {
        self.max_lines = Some(max_lines);
        self
    }

    /// Disables history eviction explicitly.
    #[must_use]
    pub fn unbounded(mut self) -> Self {
        self.compact_history();
        self.max_lines = None;
        self
    }

    /// Appends a line, evicting the oldest line when bounded history is full.
    pub fn append(&mut self, line: impl Into<Line<'static>>) {
        self.lines.push(line.into());
        if !self.follow {
            self.tail.scroll_by(self.max_tail_offset(), 1);
        }
        self.enforce_history_limit();
        self.clamp_tail();
    }

    /// Removes every checked identity.
    pub fn clear(&mut self) {
        self.lines.clear();
        self.history_start = 0;
        self.tail = TailScroll::new(0);
        self.follow = true;
        self.pending_oldest = false;
        self.scroll_indicator.clear();
    }

    #[must_use]
    /// Returns buffered log lines from oldest to newest.
    pub fn lines(&self) -> &[Line<'static>] {
        &self.lines[self.history_start..]
    }

    #[must_use]
    /// Returns the number of buffered log lines.
    pub fn len(&self) -> usize {
        self.lines.len().saturating_sub(self.history_start)
    }

    #[must_use]
    /// Returns whether `empty`.
    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }

    #[must_use]
    /// Returns whether `following`.
    pub const fn is_following(&self) -> bool {
        self.follow
    }

    /// Resumes live-tail following.
    pub fn follow(&mut self) {
        self.follow = true;
        self.pending_oldest = false;
        self.tail = TailScroll::new(0);
        self.scroll_indicator.clear();
    }

    /// Moves through scrollback using top-relative wheel semantics.
    ///
    /// Negative deltas move toward older lines; positive deltas move toward
    /// the live tail. Reaching the tail does not resume following; call
    /// [`Self::follow`] or press End to opt back in.
    pub fn scroll_by(&mut self, delta: isize) -> bool {
        if delta == 0 {
            return false;
        }
        let before = self.tail.offset();
        self.pending_oldest = false;
        let maximum = self.max_tail_offset();
        if delta.is_negative() {
            let toward_oldest = isize::try_from(delta.unsigned_abs()).unwrap_or(isize::MAX);
            self.tail.scroll_by(maximum, toward_oldest);
        } else {
            self.tail.scroll_by(maximum, -delta);
        }
        let changed = before != self.tail.offset();
        if changed {
            self.follow = false;
            self.refresh_scroll_indicator();
        }
        changed
    }

    /// Freezes the view at the oldest available window.
    ///
    /// Calling this before the first render records the intent and resolves
    /// the exact offset when the widget learns its viewport height.
    pub fn scroll_to_oldest(&mut self) -> bool {
        let before = (self.tail.offset(), self.follow, self.pending_oldest);
        self.follow = false;
        if self.viewport_height == 0 {
            self.pending_oldest = true;
        } else {
            self.pending_oldest = false;
            self.tail = TailScroll::new(self.max_tail_offset());
        }
        self.refresh_scroll_indicator();
        before != (self.tail.offset(), self.follow, self.pending_oldest)
    }

    /// Handles the `handle_key` interaction.
    pub fn handle_key(&mut self, key: KeyEvent) -> Outcome<()> {
        if key.kind == KeyEventKind::Release {
            return Outcome::Ignored;
        }
        let page = self.viewport_height.max(1);
        let old = (self.tail.offset(), self.follow, self.pending_oldest);
        match key.code {
            KeyCode::Up | KeyCode::Char('k' | 'K') => {
                self.scroll_by(-1);
            }
            KeyCode::Down | KeyCode::Char('j' | 'J') => {
                self.scroll_by(1);
            }
            KeyCode::PageUp => {
                self.scroll_by(-isize::try_from(page).unwrap_or(isize::MAX));
            }
            KeyCode::PageDown => {
                self.scroll_by(isize::try_from(page).unwrap_or(isize::MAX));
            }
            KeyCode::Home => {
                self.scroll_to_oldest();
            }
            KeyCode::End => {
                self.follow();
            }
            _ => return Outcome::Ignored,
        }
        if old == (self.tail.offset(), self.follow, self.pending_oldest) {
            Outcome::Ignored
        } else {
            Outcome::Changed
        }
    }

    fn refresh_scroll_indicator(&mut self) {
        self.scroll_indicator.clear();
        if !self.follow {
            write!(&mut self.scroll_indicator, " ⇡ +{} ", self.tail.offset())
                .expect("writing to String cannot fail");
        }
    }

    fn max_tail_offset(&self) -> usize {
        max_offset(self.len(), self.viewport_height)
    }

    fn clamp_tail(&mut self) {
        if self.follow {
            self.tail = TailScroll::new(0);
            self.pending_oldest = false;
        } else if self.pending_oldest {
            self.tail = TailScroll::new(self.max_tail_offset());
            self.pending_oldest = false;
        } else {
            self.tail.clamp(self.max_tail_offset());
        }
        self.refresh_scroll_indicator();
    }

    fn enforce_history_limit(&mut self) {
        let Some(max_lines) = self.max_lines else {
            return;
        };
        let overflow = self.len().saturating_sub(max_lines);
        self.history_start = self.history_start.saturating_add(overflow);
        let threshold = max_lines.div_ceil(8).clamp(1, 1_024);
        if self.history_start >= threshold {
            self.compact_history();
        }
    }

    fn compact_history(&mut self) {
        if self.history_start > 0 {
            self.lines.drain(..self.history_start);
            self.history_start = 0;
        }
    }
}

impl PartialEq for LogPaneState {
    fn eq(&self, other: &Self) -> bool {
        self.lines() == other.lines()
            && self.tail == other.tail
            && self.follow == other.follow
            && self.pending_oldest == other.pending_oldest
            && self.max_lines == other.max_lines
            && self.viewport_height == other.viewport_height
            && self.scroll_indicator == other.scroll_indicator
    }
}

impl Eq for LogPaneState {}

#[derive(Debug, Clone, Copy)]
/// A bounded, scrollable log buffer with tail-follow behavior.
pub struct LogPane<'a> {
    title: Option<&'a str>,
    theme: &'a Theme,
}

impl<'a> LogPane<'a> {
    #[must_use]
    /// Creates a log pane over mutable log state and a semantic theme.
    pub const fn new(theme: &'a Theme) -> Self {
        Self { title: None, theme }
    }

    #[must_use]
    /// Sets the optional visible title.
    pub const fn title(mut self, title: &'a str) -> Self {
        self.title = Some(title);
        self
    }
}

impl StatefulWidget for &LogPane<'_> {
    type State = LogPaneState;

    fn render(self, area: Rect, buffer: &mut Buffer, state: &mut Self::State) {
        state.viewport_height = usize::from(area.height.saturating_sub(2));
        state.clamp_tail();
        let top = state.tail.to_top_offset(state.len(), state.viewport_height);
        let mut scroll = DialogScroll {
            scroll_x: 0,
            scroll_y: u16::try_from(top).unwrap_or(u16::MAX),
            ..DialogScroll::default()
        };
        let viewport = Viewport::new(state.lines(), self.theme);
        let viewport = if let Some(title) = self.title {
            viewport.title(title)
        } else {
            viewport
        };
        (&viewport).render(area, buffer, &mut scroll);

        if area.height > 0 {
            let indicator = if state.follow {
                " ⇣ following "
            } else {
                &state.scroll_indicator
            };
            let width = u16::try_from(display_cols(indicator)).unwrap_or(u16::MAX);
            let indicator_x = area.right().saturating_sub(width.saturating_add(1));
            let title_end = self.title.map_or(area.x.saturating_add(1), |title| {
                area.x
                    .saturating_add(3)
                    .saturating_add(u16::try_from(display_cols(title.trim())).unwrap_or(u16::MAX))
            });
            if area.width >= width.saturating_add(2) && indicator_x > title_end {
                buffer.set_stringn(
                    indicator_x,
                    area.y,
                    indicator,
                    usize::from(width),
                    self.theme.style(Role::Accent),
                );
            }
        }
    }
}

impl StatefulWidget for LogPane<'_> {
    type State = LogPaneState;

    fn render(self, area: Rect, buffer: &mut Buffer, state: &mut Self::State) {
        StatefulWidget::render(&self, area, buffer, state);
    }
}

#[cfg(test)]
mod tests {
    use ratatui_core::widgets::StatefulWidget;

    use super::*;
    use crate::input::KeyModifiers;

    fn key(code: KeyCode) -> KeyEvent {
        KeyEvent::new(code, KeyModifiers::NONE)
    }

    #[test]
    fn append_follows_tail_until_user_scrolls() {
        let mut state = LogPaneState::new();
        state.viewport_height = 2;
        for line in ["one", "two", "three"] {
            state.append(line);
        }
        assert!(state.is_following());
        assert_eq!(state.tail.to_top_offset(state.len(), 2), 1);

        assert_eq!(state.handle_key(key(KeyCode::Up)), Outcome::Changed);
        assert!(!state.is_following());
        assert_eq!(state.tail.to_top_offset(state.len(), 2), 0);
        state.append("four");
        assert_eq!(state.tail.to_top_offset(state.len(), 2), 0);

        assert_eq!(state.handle_key(key(KeyCode::End)), Outcome::Changed);
        assert!(state.is_following());
        assert_eq!(state.tail.to_top_offset(state.len(), 2), 2);
    }

    #[test]
    fn bounded_history_evicts_oldest_lines() {
        let mut state = LogPaneState::new().with_max_lines(2);
        state.append("one");
        state.append("two");
        state.append("three");
        let text: Vec<_> = state.lines().iter().map(Line::to_string).collect();
        assert_eq!(text, ["two", "three"]);
    }

    #[test]
    fn default_history_is_bounded_and_unbounded_is_explicit() {
        let mut bounded = LogPaneState::new();
        for index in 0..=DEFAULT_LOG_HISTORY_LINES {
            bounded.append(index.to_string());
        }
        assert_eq!(bounded.len(), DEFAULT_LOG_HISTORY_LINES);
        assert_eq!(bounded.lines()[0].to_string(), "1");

        let mut unbounded = LogPaneState::new().unbounded();
        for index in 0..=DEFAULT_LOG_HISTORY_LINES {
            unbounded.append(index.to_string());
        }
        assert_eq!(unbounded.len(), DEFAULT_LOG_HISTORY_LINES + 1);
    }

    #[test]
    fn wheel_home_and_follow_cover_the_full_scrollback() {
        let mut state = LogPaneState::new();
        state.viewport_height = 2;
        for line in ["one", "two", "three", "four"] {
            state.append(line);
        }

        assert!(state.scroll_by(-1));
        assert!(!state.is_following());
        assert_eq!(state.tail.to_top_offset(state.len(), 2), 1);
        assert!(state.scroll_by(1));
        assert_eq!(state.tail.to_top_offset(state.len(), 2), 2);

        assert!(state.scroll_by(isize::MIN));
        assert_eq!(state.tail.to_top_offset(state.len(), 2), 0);
        assert!(!state.scroll_by(isize::MIN));
        assert!(state.scroll_by(isize::MAX));
        assert_eq!(state.tail.to_top_offset(state.len(), 2), 2);
        assert!(!state.scroll_by(isize::MAX));

        assert_eq!(state.handle_key(key(KeyCode::Home)), Outcome::Changed);
        assert_eq!(state.tail.to_top_offset(state.len(), 2), 0);
        state.follow();
        assert!(state.is_following());
        assert_eq!(state.tail.to_top_offset(state.len(), 2), 2);
    }

    #[test]
    fn rendering_is_deterministic_and_shows_follow_state() {
        let theme = Theme::default();
        let pane = LogPane::new(&theme).title("Build");
        let area = Rect::new(0, 0, 24, 4);
        let mut state = LogPaneState::new();
        state.append("compile ✓");
        let mut first = Buffer::empty(area);
        let mut second = Buffer::empty(area);
        (&pane).render(area, &mut first, &mut state);
        (&pane).render(area, &mut second, &mut state);
        assert_eq!(first, second);
        let text: String = first.content().iter().map(|cell| cell.symbol()).collect();
        assert!(text.contains("⇣ following"));
        (&pane).render(Rect::new(0, 0, 1, 1), &mut first, &mut state);
    }

    #[test]
    fn follow_indicator_preserves_borders_and_long_titles() {
        let theme = Theme::default();
        let mut state = LogPaneState::new();
        let exact_area = Rect::new(0, 0, 14, 3);
        let mut exact = Buffer::empty(exact_area);
        (&LogPane::new(&theme)).render(exact_area, &mut exact, &mut state);
        assert_eq!(exact[(0, 0)].symbol(), "┌");
        assert_eq!(exact[(13, 0)].symbol(), "┐");
        assert!(!rendered(&exact).contains("following"));

        let titled_area = Rect::new(0, 0, 28, 3);
        let mut titled = Buffer::empty(titled_area);
        (&LogPane::new(&theme).title("A deliberately long title")).render(
            titled_area,
            &mut titled,
            &mut state,
        );
        assert!(!rendered(&titled).contains("following"));
        assert_eq!(titled[(27, 0)].symbol(), "┐");
    }

    #[test]
    fn scrolled_back_indicator_reports_lines_below_view() {
        let theme = Theme::default();
        let area = Rect::new(0, 0, 32, 4);
        let mut state = LogPaneState::new();
        for line in ["one", "two", "three", "four"] {
            state.append(line);
        }
        let pane = LogPane::new(&theme).title("Build");
        let mut buffer = Buffer::empty(area);
        (&pane).render(area, &mut buffer, &mut state);
        assert!(state.scroll_by(-1));
        (&pane).render(area, &mut buffer, &mut state);
        assert!(rendered(&buffer).contains("⇡ +1"));
    }

    fn rendered(buffer: &Buffer) -> String {
        buffer.content().iter().map(|cell| cell.symbol()).collect()
    }
}
