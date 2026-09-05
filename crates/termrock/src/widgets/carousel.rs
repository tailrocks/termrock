// SPDX-FileCopyrightText: 2026 Alexey Zhokhov
// SPDX-License-Identifier: Apache-2.0

//! **Carousel** — multi-slide panel with keyboard/mouse parity (shadcn Carousel peer).
//!
//! **Mission.** Host-projected slides (title + body lines); prev/next, wrap,
//! page indicators, and optional auto-advance tick (host-driven). No hover-only
//! controls — arrows always keyboard-reachable.
//!
//! Research: shadcn Carousel, terminal wizards, slide decks in TUI.
#![allow(unused_imports)] // test-module imports kept for unit tests; lib path may not use them
use ratatui_core::{buffer::Buffer, layout::Rect, style::Modifier};

use crate::{
    input::{
        KeyCode, KeyEvent, KeyEventKind, KeyModifiers, MouseButton, MouseEvent, MouseEventKind,
    },
    style::{DesignSystem, Role},
    text::{display_cols, take_display_cols},
};

// ── Domain ──────────────────────────────────────────────────────────────────

/// One carousel slide (host-projected).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CarouselSlide {
    /// Stable id.
    pub id: String,
    /// Title.
    pub title: String,
    /// Body lines.
    pub body: Vec<String>,
}

impl CarouselSlide {
    /// Construct.
    #[must_use]
    pub fn new(id: impl Into<String>, title: impl Into<String>) -> Self {
        Self {
            id: id.into(),
            title: title.into(),
            body: Vec::new(),
        }
    }

    /// Body lines.
    #[must_use]
    pub fn body(mut self, lines: impl IntoIterator<Item = impl Into<String>>) -> Self {
        self.body = lines.into_iter().map(Into::into).collect();
        self
    }
}

/// Outcomes.
#[derive(Debug, Clone, PartialEq, Eq)]
#[non_exhaustive]
pub enum CarouselOutcome {
    /// Ignored.
    Ignored,
    /// Index changed.
    Changed {
        /// New index.
        index: usize,
        /// Slide id.
        id: String,
    },
    /// Activated current slide (Enter).
    Activated {
        /// Index.
        index: usize,
        /// Id.
        id: String,
    },
    /// Esc.
    Cancelled,
}

// ── State ───────────────────────────────────────────────────────────────────

/// Carousel state.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CarouselState {
    index: usize,
    wrap: bool,
    focused: bool,
    accepts_input: bool,
    /// Host auto-advance period (ms); 0 = off. Host calls [`tick`].
    auto_ms: u64,
    elapsed_ms: u64,
}

impl Default for CarouselState {
    fn default() -> Self {
        Self::new()
    }
}

impl CarouselState {
    /// Fresh at first slide.
    #[must_use]
    pub fn new() -> Self {
        Self {
            index: 0,
            wrap: true,
            focused: true,
            accepts_input: true,
            auto_ms: 0,
            elapsed_ms: 0,
        }
    }

    /// Wrap at ends.
    pub fn set_wrap(&mut self, on: bool) {
        self.wrap = on;
    }

    /// Focus.
    pub fn set_focused(&mut self, on: bool) {
        self.focused = on;
    }

    /// Input.
    pub fn set_accepts_input(&mut self, on: bool) {
        self.accepts_input = on;
    }

    /// Auto-advance period in ms (0 disables).
    pub fn set_auto_ms(&mut self, ms: u64) {
        self.auto_ms = ms;
        self.elapsed_ms = 0;
    }

    /// Current index.
    #[must_use]
    pub const fn index(&self) -> usize {
        self.index
    }

    /// Clamp index to slide count.
    pub fn clamp_to(&mut self, n: usize) {
        if n == 0 {
            self.index = 0;
        } else {
            self.index = self.index.min(n - 1);
        }
    }

    /// Go to index.
    pub fn go_to(&mut self, i: usize, slides: &[CarouselSlide]) -> CarouselOutcome {
        if slides.is_empty() {
            return CarouselOutcome::Ignored;
        }
        let i = i.min(slides.len() - 1);
        if i == self.index {
            return CarouselOutcome::Ignored;
        }
        self.index = i;
        self.elapsed_ms = 0;
        CarouselOutcome::Changed {
            index: i,
            id: slides[i].id.clone(),
        }
    }

    /// Next.
    pub fn next(&mut self, slides: &[CarouselSlide]) -> CarouselOutcome {
        if slides.is_empty() {
            return CarouselOutcome::Ignored;
        }
        let n = slides.len();
        let next = if self.index + 1 < n {
            self.index + 1
        } else if self.wrap {
            0
        } else {
            return CarouselOutcome::Ignored;
        };
        self.go_to(next, slides)
    }

    /// Previous.
    pub fn prev(&mut self, slides: &[CarouselSlide]) -> CarouselOutcome {
        if slides.is_empty() {
            return CarouselOutcome::Ignored;
        }
        let n = slides.len();
        let next = if self.index > 0 {
            self.index - 1
        } else if self.wrap {
            n - 1
        } else {
            return CarouselOutcome::Ignored;
        };
        self.go_to(next, slides)
    }

    /// Host frame tick for auto-advance.
    pub fn tick(&mut self, delta_ms: u64, slides: &[CarouselSlide]) -> CarouselOutcome {
        if self.auto_ms == 0 || slides.len() < 2 {
            return CarouselOutcome::Ignored;
        }
        self.elapsed_ms = self.elapsed_ms.saturating_add(delta_ms);
        if self.elapsed_ms >= self.auto_ms {
            self.elapsed_ms = 0;
            return self.next(slides);
        }
        CarouselOutcome::Ignored
    }

    /// Keys.
    pub fn handle_key(&mut self, key: KeyEvent, slides: &[CarouselSlide]) -> CarouselOutcome {
        if !self.accepts_input || !self.focused || !key.is_press() {
            return CarouselOutcome::Ignored;
        }
        match key.code {
            KeyCode::Esc => CarouselOutcome::Cancelled,
            KeyCode::Left | KeyCode::Char('h') if key.modifiers.is_empty() => self.prev(slides),
            KeyCode::Right | KeyCode::Char('l') if key.modifiers.is_empty() => self.next(slides),
            KeyCode::Home => self.go_to(0, slides),
            KeyCode::End if !slides.is_empty() => self.go_to(slides.len() - 1, slides),
            KeyCode::Enter => {
                if slides.is_empty() {
                    return CarouselOutcome::Ignored;
                }
                let i = self.index.min(slides.len() - 1);
                CarouselOutcome::Activated {
                    index: i,
                    id: slides[i].id.clone(),
                }
            }
            KeyCode::Char(c) if c.is_ascii_digit() && key.modifiers.is_empty() => {
                let d = c.to_digit(10).unwrap_or(0) as usize;
                if d == 0 {
                    return CarouselOutcome::Ignored;
                }
                self.go_to(d - 1, slides)
            }
            _ => CarouselOutcome::Ignored,
        }
    }

    /// Mouse routing for the painted footer controls.
    ///
    /// Indicator cells select their slide; the painted left/right arrows use
    /// the same previous/next paths as the keyboard adapter.
    pub fn handle_mouse(
        &mut self,
        event: MouseEvent,
        area: Rect,
        slides: &[CarouselSlide],
    ) -> CarouselOutcome {
        if !self.accepts_input
            || area.is_empty()
            || slides.is_empty()
            || event.kind != MouseEventKind::Down(MouseButton::Left)
            || !area.contains(event.position)
            || event.position.y != area.bottom().saturating_sub(1)
        {
            return CarouselOutcome::Ignored;
        }

        let rel_x = usize::from(event.position.x.saturating_sub(area.x));
        for index in 0..slides.len() {
            if rel_x == index.saturating_mul(2) {
                self.focused = true;
                return self.go_to(index, slides);
            }
        }

        let current = self.index.min(slides.len() - 1);
        let indicator_width = slides.len().saturating_mul(2).saturating_sub(1);
        let counter = format!("  {}/{}  ", current + 1, slides.len());
        let previous_x = indicator_width.saturating_add(display_cols(&counter));
        if rel_x == previous_x {
            self.focused = true;
            return self.prev(slides);
        }
        if rel_x == previous_x.saturating_add(2) {
            self.focused = true;
            return self.next(slides);
        }
        CarouselOutcome::Ignored
    }
}

// ── Widget ──────────────────────────────────────────────────────────────────

/// Carousel paint.
#[derive(Debug, Clone, Copy)]
pub struct Carousel<'a> {
    slides: &'a [CarouselSlide],
    system: &'a DesignSystem,
}

impl<'a> Carousel<'a> {
    /// Slides + system.
    #[must_use]
    pub const fn new(slides: &'a [CarouselSlide], system: &'a DesignSystem) -> Self {
        Self { slides, system }
    }

    /// ASCII indicators.
    #[must_use]
    /// Paint.
    pub fn paint(&self, area: Rect, buffer: &mut Buffer, state: &CarouselState) {
        if area.is_empty() {
            return;
        }
        if self.slides.is_empty() {
            // Empty bodies speak one language: glyph + sentence, through the
            // widget that owns it (plans/009 Step 2).
            super::EmptyState::new("No slides", self.system).paint(area, buffer);
            return;
        }
        let i = state.index.min(self.slides.len() - 1);
        let slide = &self.slides[i];
        let mut y = area.y;
        let max_y = area.y.saturating_add(area.height);

        // Title
        let title_style = if state.focused {
            self.system
                .style(Role::TextStrong)
                .add_modifier(Modifier::BOLD)
        } else {
            self.system.style(Role::TextStrong)
        };
        buffer.set_stringn(
            area.x,
            y,
            take_display_cols(&slide.title, usize::from(area.width)),
            usize::from(area.width),
            title_style,
        );
        y = y.saturating_add(1);

        // Body
        for line in &slide.body {
            if y >= max_y.saturating_sub(1) {
                break;
            }
            buffer.set_stringn(
                area.x,
                y,
                take_display_cols(line, usize::from(area.width)),
                usize::from(area.width),
                self.system.style(Role::TextMuted),
            );
            y = y.saturating_add(1);
        }

        // Indicators footer
        if y < max_y {
            let mut dots = String::new();
            for (j, _) in self.slides.iter().enumerate() {
                {
                    dots.push(if j == i { '●' } else { '○' });
                }
                if j + 1 < self.slides.len() {
                    dots.push(' ');
                }
            }
            let hint = format!("{dots}  {}/{}  ← →", i + 1, self.slides.len());
            buffer.set_stringn(
                area.x,
                max_y.saturating_sub(1),
                take_display_cols(&hint, usize::from(area.width)),
                usize::from(area.width),
                self.system.style(Role::TextDisabled),
            );
        }
    }
}

/// Example slides.
#[must_use]
pub fn example_carousel_slides() -> Vec<CarouselSlide> {
    vec![
        CarouselSlide::new("s1", "Welcome").body(["TermRock carousel", "Keyboard: ← → or h l"]),
        CarouselSlide::new("s2", "Compose").body(["Build high-class TUI", "from public widgets"]),
        CarouselSlide::new("s3", "Ship")
            .body(["Stories · tests · migrations", "Ratatui paint engine"]),
    ]
}

#[cfg(test)]
mod tests {
    use super::*;

    fn press(code: KeyCode) -> KeyEvent {
        KeyEvent::new(code, KeyModifiers::NONE)
    }

    #[test]
    fn next_prev_wrap_and_activate() {
        let slides = example_carousel_slides();
        let mut st = CarouselState::new();
        st.set_wrap(true);
        let out = st.handle_key(press(KeyCode::Right), &slides);
        assert!(
            matches!(out, CarouselOutcome::Changed { index: 1, .. }),
            "{out:?}"
        );
        let out = st.handle_key(press(KeyCode::Left), &slides);
        assert!(matches!(out, CarouselOutcome::Changed { index: 0, .. }));
        // wrap from 0 left → last
        let out = st.handle_key(press(KeyCode::Left), &slides);
        assert!(
            matches!(out, CarouselOutcome::Changed { index: 2, .. }),
            "{out:?}"
        );
        let out = st.handle_key(press(KeyCode::Enter), &slides);
        assert!(
            matches!(out, CarouselOutcome::Activated { index: 2, ref id } if id == "s3"),
            "{out:?}"
        );
    }

    #[test]
    fn no_wrap_stops_at_end() {
        let slides = example_carousel_slides();
        let mut st = CarouselState::new();
        st.set_wrap(false);
        st.go_to(2, &slides);
        let out = st.handle_key(press(KeyCode::Right), &slides);
        assert!(matches!(out, CarouselOutcome::Ignored));
    }

    #[test]
    fn auto_tick_advances() {
        let slides = example_carousel_slides();
        let mut st = CarouselState::new();
        st.set_auto_ms(100);
        assert!(matches!(st.tick(50, &slides), CarouselOutcome::Ignored));
        let out = st.tick(60, &slides);
        assert!(matches!(out, CarouselOutcome::Changed { index: 1, .. }));
    }

    #[test]
    fn paint_smoke() {
        let system = DesignSystem::default();
        let slides = example_carousel_slides();
        let st = CarouselState::new();
        let area = Rect::new(0, 0, 40, 8);
        let mut buf = Buffer::empty(area);
        let _ = Carousel::new(&slides, &system).paint(area, &mut buf, &st);
    }

    #[test]
    fn mouse_footer_matches_keyboard_and_input_gate() {
        let slides = example_carousel_slides();
        let area = Rect::new(0, 0, 40, 8);
        let mut st = CarouselState::new();
        st.set_focused(false);

        let second_indicator = MouseEvent {
            kind: MouseEventKind::Down(MouseButton::Left),
            position: ratatui_core::layout::Position::new(2, area.bottom() - 1),
            modifiers: KeyModifiers::NONE,
        };
        assert!(matches!(
            st.handle_mouse(second_indicator, area, &slides),
            CarouselOutcome::Changed { index: 1, .. }
        ));
        assert!(st.focused, "pointer entry grants focus to the carousel");
        assert_eq!(
            st.handle_key(press(KeyCode::Esc), &slides),
            CarouselOutcome::Cancelled
        );

        st.set_accepts_input(false);
        assert_eq!(
            st.handle_mouse(second_indicator, area, &slides),
            CarouselOutcome::Ignored
        );
        assert_eq!(
            st.handle_key(press(KeyCode::Right), &slides),
            CarouselOutcome::Ignored
        );
    }
}
