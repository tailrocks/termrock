use ratatui_core::{
    buffer::Buffer,
    layout::Rect,
    text::{Line, Span},
    widgets::Widget,
};
use ratatui_widgets::paragraph::{Paragraph, Wrap};

#[derive(Debug, Clone, Copy)]
pub struct Hint<'a> {
    pub chord: &'a str,
    pub label: &'a str,
    pub priority: u8,
    pub visible: bool,
}

#[derive(Debug, Clone, Copy)]
pub struct HintBar<'a> {
    pub hints: &'a [Hint<'a>],
    pub separator: &'a str,
}

impl Widget for &HintBar<'_> {
    fn render(self, area: Rect, buffer: &mut Buffer) {
        let mut spans = Vec::new();
        for hint in self.hints.iter().filter(|hint| hint.visible) {
            if !spans.is_empty() {
                spans.push(Span::raw(self.separator));
            }
            spans.push(Span::styled(
                hint.chord,
                ratatui_core::style::Style::new().bold(),
            ));
            spans.push(Span::raw(" "));
            spans.push(Span::raw(hint.label));
        }
        Paragraph::new(Line::from(spans))
            .wrap(Wrap { trim: false })
            .render(area, buffer);
    }
}
