use ratatui_core::{buffer::Buffer, layout::Rect, text::Line, widgets::StatefulWidget};

use crate::{
    input::{KeyCode, KeyEvent, KeyEventKind},
    interaction::Outcome,
    scroll::{DialogScroll, TailScroll, max_offset},
    style::{Role, Theme},
    text::display_cols,
};

use super::Viewport;

#[derive(Debug, Clone, PartialEq, Eq)]
/// Runtime state for `LogPane`.
pub struct LogPaneState {
    lines: Vec<Line<'static>>,
    tail: TailScroll,
    follow: bool,
    max_lines: Option<usize>,
    viewport_height: usize,
}

impl Default for LogPaneState {
    fn default() -> Self {
        Self::new()
    }
}

impl LogPaneState {
    #[must_use]
    /// Creates a new value with canonical defaults.
    pub const fn new() -> Self {
        Self {
            lines: Vec::new(),
            tail: TailScroll::new(0),
            follow: true,
            max_lines: None,
            viewport_height: 0,
        }
    }

    #[must_use]
    /// Returns this value with `max_lines` configured.
    pub const fn with_max_lines(mut self, max_lines: usize) -> Self {
        self.max_lines = Some(max_lines);
        self
    }

    /// Performs the `append` operation.
    pub fn append(&mut self, line: impl Into<Line<'static>>) {
        self.lines.push(line.into());
        if !self.follow {
            self.tail.scroll_by(self.max_tail_offset(), 1);
        }
        if let Some(max_lines) = self.max_lines {
            let overflow = self.lines.len().saturating_sub(max_lines);
            if overflow > 0 {
                self.lines.drain(..overflow);
            }
        }
        self.clamp_tail();
    }

    /// Performs the `clear` operation.
    pub fn clear(&mut self) {
        self.lines.clear();
        self.tail = TailScroll::new(0);
        self.follow = true;
    }

    #[must_use]
    /// Performs the `lines` operation.
    pub fn lines(&self) -> &[Line<'static>] {
        &self.lines
    }

    #[must_use]
    /// Performs the `len` operation.
    pub fn len(&self) -> usize {
        self.lines.len()
    }

    #[must_use]
    /// Returns whether `empty`.
    pub fn is_empty(&self) -> bool {
        self.lines.is_empty()
    }

    #[must_use]
    /// Returns whether `following`.
    pub const fn is_following(&self) -> bool {
        self.follow
    }

    /// Handles the `handle_key` interaction.
    pub fn handle_key(&mut self, key: KeyEvent) -> Outcome<()> {
        if key.kind == KeyEventKind::Release {
            return Outcome::Ignored;
        }
        let max = self.max_tail_offset();
        let page = self.viewport_height.max(1);
        let old = (self.tail.offset(), self.follow);
        match key.code {
            KeyCode::Up | KeyCode::Char('k' | 'K') => {
                self.follow = false;
                self.tail.scroll_by(max, 1);
            }
            KeyCode::Down | KeyCode::Char('j' | 'J') => {
                self.follow = false;
                self.tail.scroll_by(max, -1);
            }
            KeyCode::PageUp => {
                self.follow = false;
                self.tail.scroll_by(max, page as isize);
            }
            KeyCode::PageDown => {
                self.follow = false;
                self.tail.scroll_by(max, -(page as isize));
            }
            KeyCode::End => {
                self.follow = true;
                self.tail = TailScroll::new(0);
            }
            _ => return Outcome::Ignored,
        }
        if old == (self.tail.offset(), self.follow) {
            Outcome::Ignored
        } else {
            Outcome::Changed
        }
    }

    fn max_tail_offset(&self) -> usize {
        max_offset(self.lines.len(), self.viewport_height)
    }

    fn clamp_tail(&mut self) {
        if self.follow {
            self.tail = TailScroll::new(0);
        } else {
            self.tail.clamp(self.max_tail_offset());
        }
    }
}

#[derive(Debug, Clone, Copy)]
/// Data carried by `LogPane`.
pub struct LogPane<'a> {
    title: Option<&'a str>,
    theme: &'a Theme,
}

impl<'a> LogPane<'a> {
    #[must_use]
    /// Creates a new value with canonical defaults.
    pub const fn new(theme: &'a Theme) -> Self {
        Self { title: None, theme }
    }

    #[must_use]
    /// Performs the `title` operation.
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
        let top = state
            .tail
            .to_top_offset(state.lines.len(), state.viewport_height);
        let mut scroll = DialogScroll {
            scroll_x: 0,
            scroll_y: u16::try_from(top).unwrap_or(u16::MAX),
            ..DialogScroll::default()
        };
        let viewport = Viewport::new(&state.lines, self.theme);
        let viewport = if let Some(title) = self.title {
            viewport.title(title)
        } else {
            viewport
        };
        (&viewport).render(area, buffer, &mut scroll);

        if state.follow && area.height > 0 {
            let indicator = " ⇣ following ";
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

    fn rendered(buffer: &Buffer) -> String {
        buffer.content().iter().map(|cell| cell.symbol()).collect()
    }
}
