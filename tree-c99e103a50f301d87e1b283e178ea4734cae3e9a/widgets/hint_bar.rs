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

/// Wrap semantic hint groups without splitting a key/label pair.
#[must_use]
pub fn wrapped_hint_lines(spans: &[HintSpan<'_>], width: u16) -> Vec<Line<'static>> {
    #[derive(Clone, Copy)]
    enum Separator {
        Group,
        Dot,
    }
    struct Chunk {
        spans: Vec<Span<'static>>,
        width: usize,
        separator: Separator,
    }

    let mut chunks = Vec::new();
    let mut current = Vec::new();
    let mut current_width = 0;
    let mut separator = Separator::Group;
    let flush = |chunks: &mut Vec<Chunk>,
                 current: &mut Vec<Span<'static>>,
                 current_width: &mut usize,
                 separator| {
        if !current.is_empty() {
            chunks.push(Chunk {
                spans: std::mem::take(current),
                width: *current_width,
                separator,
            });
            *current_width = 0;
        }
    };
    for span in spans {
        match span {
            HintSpan::Sep | HintSpan::GroupSep => {
                flush(&mut chunks, &mut current, &mut current_width, separator);
                separator = if matches!(span, HintSpan::Sep) {
                    Separator::Dot
                } else {
                    Separator::Group
                };
            }
            _ => {
                current_width += span.display_cols();
                current.extend(styled_hint_spans(std::slice::from_ref(span), |color| color));
            }
        }
    }
    flush(&mut chunks, &mut current, &mut current_width, separator);

    let mut lines = Vec::new();
    let mut row = Vec::new();
    let mut row_width: usize = 0;
    for chunk in chunks {
        let separator_width = usize::from(!row.is_empty()) * 3;
        if !row.is_empty()
            && row_width
                .saturating_add(separator_width)
                .saturating_add(chunk.width)
                > usize::from(width)
        {
            lines.push(Line::from(std::mem::take(&mut row)));
            row_width = 0;
        }
        if !row.is_empty() {
            match chunk.separator {
                Separator::Dot => {
                    row.extend(styled_hint_spans(&[HintSpan::Sep], |color| color));
                }
                Separator::Group => row.push(Span::raw("   ")),
            }
            row_width += 3;
        }
        row.extend(chunk.spans);
        row_width += chunk.width;
    }
    if !row.is_empty() {
        lines.push(Line::from(row));
    }
    if lines.is_empty() {
        lines.push(Line::raw(""));
    }
    lines
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn semantic_wrapping_keeps_key_and_label_together() {
        let spans = [
            HintSpan::Key("Enter"),
            HintSpan::Text("select"),
            HintSpan::GroupSep,
            HintSpan::Key("Esc"),
            HintSpan::Text("cancel"),
        ];
        let lines = wrapped_hint_lines(&spans, 15);
        assert_eq!(lines.len(), 2);
        assert_eq!(lines[0].to_string(), "Enter select");
        assert_eq!(lines[1].to_string(), "Esc cancel");
    }
}
