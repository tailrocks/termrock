use ratatui_core::{buffer::Buffer, layout::Rect, style::Style, text::Line, widgets::Widget};
use ratatui_widgets::{block::Block, clear::Clear, paragraph::Paragraph};

use super::Action;

#[derive(Debug, Clone, Copy)]
pub struct Backdrop {
    pub symbol: char,
    pub style: Style,
}
impl Widget for &Backdrop {
    fn render(self, area: Rect, buffer: &mut Buffer) {
        for y in area.top()..area.bottom() {
            for x in area.left()..area.right() {
                buffer[(x, y)].set_char(self.symbol).set_style(self.style);
            }
        }
    }
}

#[derive(Debug, Clone)]
pub struct Dialog<'a> {
    pub title: &'a str,
    pub body: Line<'a>,
    pub style: Style,
}
impl Widget for &Dialog<'_> {
    fn render(self, area: Rect, buffer: &mut Buffer) {
        Clear.render(area, buffer);
        Paragraph::new(self.body.clone())
            .block(Block::bordered().title(self.title))
            .style(self.style)
            .render(area, buffer);
    }
}

#[derive(Debug, Clone)]
pub struct DialogAction<'a, Id> {
    pub action: Action<'a, Id>,
    pub destructive: bool,
}
#[derive(Debug, Clone)]
pub struct ChoiceDialog<'a, Id> {
    pub dialog: Dialog<'a>,
    pub actions: &'a [DialogAction<'a, Id>],
}
impl<Id> Widget for &ChoiceDialog<'_, Id> {
    fn render(self, area: Rect, buffer: &mut Buffer) {
        (&self.dialog).render(area, buffer);
        if area.height < 3 {
            return;
        }
        let mut x = area.x.saturating_add(1);
        let y = area.bottom().saturating_sub(2);
        for item in self.actions {
            let label = item.action.label;
            let width = label
                .chars()
                .count()
                .saturating_add(2)
                .min(u16::MAX as usize) as u16;
            let rect = Rect::new(x, y, width.min(area.right().saturating_sub(x)), 1);
            let style = item.action.style.unwrap_or_else(|| {
                if item.destructive {
                    Style::new().bold()
                } else {
                    Style::new()
                }
            });
            buffer.set_stringn(
                rect.x,
                rect.y,
                format!(" {label} "),
                rect.width as usize,
                style,
            );
            x = x.saturating_add(width).saturating_add(1);
            if x >= area.right() {
                break;
            }
        }
    }
}
#[derive(Debug, Clone)]
pub struct MessageDialog<'a, Id> {
    pub dialog: Dialog<'a>,
    pub details: &'a [super::DetailRow<'a, Id>],
}
impl<Id> Widget for &MessageDialog<'_, Id> {
    fn render(self, area: Rect, buffer: &mut Buffer) {
        (&self.dialog).render(area, buffer);
        if area.width < 3 || area.height < 3 {
            return;
        }
        let inner = Rect::new(area.x + 1, area.y + 2, area.width - 2, area.height - 3);
        for (index, row) in self.details.iter().take(inner.height as usize).enumerate() {
            let y = inner.y.saturating_add(index as u16);
            buffer.set_stringn(
                inner.x,
                y,
                row.label,
                inner.width as usize,
                Style::new().dim(),
            );
            let value_x = inner.x.saturating_add(inner.width.min(14));
            buffer.set_stringn(
                value_x,
                y,
                row.value,
                inner.right().saturating_sub(value_x) as usize,
                Style::new(),
            );
        }
    }
}
