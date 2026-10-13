use ratatui_core::{buffer::Buffer, layout::Rect, style::Style, widgets::StatefulWidget};

use crate::style::{Role, Theme};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[non_exhaustive]
pub enum DiffKind {
    Context,
    Added,
    Removed,
}
#[derive(Debug, Clone, Copy)]
pub struct DiffLine<'a> {
    pub text: &'a str,
    pub kind: DiffKind,
}
#[derive(Debug, Clone, PartialEq, Eq, Default)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct DiffState {
    pub selected: Option<usize>,
    pub offset: usize,
}
#[derive(Debug, Clone, Copy)]
pub struct DiffView<'a> {
    lines: &'a [DiffLine<'a>],
    theme: &'a Theme,
}

impl<'a> DiffView<'a> {
    #[must_use]
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
