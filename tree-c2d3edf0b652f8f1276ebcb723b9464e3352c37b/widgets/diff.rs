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
