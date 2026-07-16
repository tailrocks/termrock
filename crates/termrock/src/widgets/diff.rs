use ratatui_core::{buffer::Buffer, layout::Rect, style::Style, widgets::StatefulWidget};

use crate::style::{Role, Theme};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[non_exhaustive]
/// Available `DiffKind` choices.
pub enum DiffKind {
    /// Selects the `Context` behavior.
    Context,
    /// Selects the `Added` behavior.
    Added,
    /// Selects the `Removed` behavior.
    Removed,
}
#[derive(Debug, Clone, Copy)]
/// Data carried by `DiffLine`.
pub struct DiffLine<'a> {
    /// Documentation for `item`.
    pub text: &'a str,
    /// Documentation for `item`.
    pub kind: DiffKind,
}
#[derive(Debug, Clone, PartialEq, Eq, Default)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
/// Runtime state for `Diff`.
pub struct DiffState {
    /// Documentation for `item`.
    pub selected: Option<usize>,
    /// Documentation for `item`.
    pub offset: usize,
}
#[derive(Debug, Clone, Copy)]
/// Data carried by `DiffView`.
pub struct DiffView<'a> {
    lines: &'a [DiffLine<'a>],
    theme: &'a Theme,
}

impl<'a> DiffView<'a> {
    #[must_use]
    /// Creates a new value with canonical defaults.
    pub const fn new(lines: &'a [DiffLine<'a>], theme: &'a Theme) -> Self {
        Self { lines, theme }
    }
}

impl StatefulWidget for &DiffView<'_> {
    type State = DiffState;
    fn render(self, area: Rect, buffer: &mut Buffer, state: &mut Self::State) {
        state.offset = state.offset.min(crate::scroll::max_offset(
            self.lines.len(),
            usize::from(area.height),
        ));
        for (visible, line) in self
            .lines
            .iter()
            .skip(state.offset)
            .take(area.height as usize)
            .enumerate()
        {
            let style = match line.kind {
                DiffKind::Context => Style::new(),
                DiffKind::Added => self.theme.style(Role::DiffAdded),
                DiffKind::Removed => self.theme.style(Role::DiffRemoved),
            };
            buffer.set_stringn(
                area.x,
                area.y.saturating_add(visible as u16),
                line.text,
                area.width as usize,
                style,
            );
        }
    }
}

impl StatefulWidget for DiffView<'_> {
    type State = DiffState;

    fn render(self, area: Rect, buffer: &mut Buffer, state: &mut Self::State) {
        StatefulWidget::render(&self, area, buffer, state);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn lines() -> [DiffLine<'static>; 5] {
        [
            DiffLine {
                text: "zero",
                kind: DiffKind::Context,
            },
            DiffLine {
                text: "+one",
                kind: DiffKind::Added,
            },
            DiffLine {
                text: "-two",
                kind: DiffKind::Removed,
            },
            DiffLine {
                text: "three",
                kind: DiffKind::Context,
            },
            DiffLine {
                text: "four",
                kind: DiffKind::Context,
            },
        ]
    }

    #[test]
    fn renders_kind_styles_on_their_rows() {
        let lines = lines();
        let theme = Theme::default();
        let view = DiffView::new(&lines, &theme);
        let area = Rect::new(0, 0, 8, 3);
        let mut buffer = Buffer::empty(area);
        let mut state = DiffState::default();
        (&view).render(area, &mut buffer, &mut state);

        assert_eq!(buffer[(0, 0)].symbol(), "z");
        assert_eq!(buffer[(0, 1)].fg, theme.style(Role::DiffAdded).fg.unwrap());
        assert_eq!(
            buffer[(0, 2)].fg,
            theme.style(Role::DiffRemoved).fg.unwrap()
        );
    }

    #[test]
    fn clamps_over_scroll_to_the_last_full_window() {
        let lines = lines();
        let theme = Theme::default();
        let view = DiffView::new(&lines, &theme);
        let area = Rect::new(0, 0, 8, 3);
        let mut buffer = Buffer::empty(area);
        let mut state = DiffState {
            selected: None,
            offset: usize::MAX,
        };
        (&view).render(area, &mut buffer, &mut state);

        assert_eq!(state.offset, 2);
        assert_eq!(buffer[(0, 0)].symbol(), "-");
        assert_eq!(buffer[(0, 2)].symbol(), "f");
    }

    #[test]
    fn tiny_areas_and_control_text_do_not_panic() {
        let lines = [DiffLine {
            text: "a\u{7}b",
            kind: DiffKind::Context,
        }];
        let theme = Theme::default();
        let view = DiffView::new(&lines, &theme);
        let mut state = DiffState::default();
        (&view).render(
            Rect::new(0, 0, 0, 0),
            &mut Buffer::empty(Rect::new(0, 0, 0, 0)),
            &mut state,
        );

        let area = Rect::new(0, 0, 1, 1);
        let mut buffer = Buffer::empty(area);
        (&view).render(area, &mut buffer, &mut state);
        assert_eq!(buffer[(0, 0)].symbol(), "a");
    }
}
