use ratatui_core::{buffer::Buffer, layout::Rect, style::Style, widgets::StatefulWidget};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
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
pub struct DiffState {
    pub selected: Option<usize>,
    pub offset: usize,
}
#[derive(Debug, Clone, Copy)]
pub struct DiffView<'a> {
    pub lines: &'a [DiffLine<'a>],
    pub added_style: Style,
    pub removed_style: Style,
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
                DiffKind::Added => self.added_style,
                DiffKind::Removed => self.removed_style,
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
