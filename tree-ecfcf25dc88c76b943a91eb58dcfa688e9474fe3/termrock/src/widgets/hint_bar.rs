use crate::HintSpan;
use ratatui_core::{
    buffer::Buffer,
    layout::Rect,
    style::{Color, Modifier, Style},
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

/// Render the shared rich hint vocabulary centered in the supplied area.
pub fn render_hint_bar(
    frame: &mut ratatui_core::terminal::Frame<'_>,
    area: Rect,
    spans: &[HintSpan<'_>],
) {
    frame.render_widget(
        Paragraph::new(Line::from(styled_hint_spans(spans, |color| color)))
            .alignment(ratatui_core::layout::Alignment::Center),
        area,
    );
}

/// Convert rich hint spans into their canonical styled terminal spans.
pub fn styled_hint_spans(
    spans: &[HintSpan<'_>],
    remap: impl Fn(Color) -> Color,
) -> Vec<Span<'static>> {
    let key = Style::default()
        .fg(remap(crate::style::WHITE))
        .add_modifier(Modifier::BOLD);
    let text = Style::default().fg(remap(crate::style::PHOSPHOR_GREEN));
    let dim = Style::default().fg(remap(crate::style::PHOSPHOR_DIM));
    let sep = Style::default().fg(remap(crate::style::BORDER_GRAY));
    let mut out = Vec::with_capacity(spans.len());
    for span in spans {
        match span {
            HintSpan::Key(value) => out.push(Span::styled((*value).to_owned(), key)),
            HintSpan::DynKey(value) => out.push(Span::styled(value.clone(), key)),
            HintSpan::Text(value) => out.push(Span::styled(format!(" {value}"), text)),
            HintSpan::Dyn(value) => out.push(Span::styled(format!(" {value}"), dim)),
            HintSpan::Sep => out.push(Span::styled(" · ", sep)),
            HintSpan::GroupSep => out.push(Span::raw("   ")),
        }
    }
    out
}
