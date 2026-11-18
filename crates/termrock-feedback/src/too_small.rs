use ratatui::buffer::Buffer;
use ratatui::layout::Rect;
use ratatui::style::Style;

use termrock_text::cells::{paint_clipped, truncate, width};
use termrock_theme::Theme;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TooSmall<'a> {
    brand: &'a str,
    brand_style: Option<Style>,
    min: (u16, u16),
    quit_hint: &'a str,
}

impl<'a> TooSmall<'a> {
    pub const MIN_WIDTH: u16 = 72;
    pub const MIN_HEIGHT: u16 = 20;

    pub const fn new(brand: &'a str) -> Self {
        Self {
            brand,
            brand_style: None,
            min: (Self::MIN_WIDTH, Self::MIN_HEIGHT),
            quit_hint: "q Quit",
        }
    }

    pub const fn brand_style(mut self, style: Style) -> Self {
        self.brand_style = Some(style);
        self
    }

    pub const fn min(mut self, width: u16, height: u16) -> Self {
        self.min = (width, height);
        self
    }

    pub const fn quit_hint(mut self, hint: &'a str) -> Self {
        self.quit_hint = hint;
        self
    }

    pub fn applies(&self, area: Rect) -> bool {
        area.width < self.min.0 || area.height < self.min.1
    }

    pub fn render(&self, area: Rect, buf: &mut Buffer, t: &Theme) {
        let canvas = |s: Style| s.bg(t.canvas);
        let need = format!(
            "Need {}×{}, have {}×{}",
            self.min.0, self.min.1, area.width, area.height
        );
        let lines = [
            (
                self.brand,
                self.brand_style.unwrap_or_else(|| canvas(t.title())),
            ),
            ("Terminal too small", canvas(t.secondary())),
            (need.as_str(), canvas(t.muted())),
            (self.quit_hint, canvas(t.faint())),
        ];
        let owner = area.intersection(*buf.area());
        let y0 = usize::from(owner.height.saturating_sub(5)) / 2;
        for (i, (text, style)) in lines.into_iter().enumerate() {
            let row = y0 + i + usize::from(i == 3);
            let Some(row) = u16::try_from(row).ok().filter(|&r| r < owner.height) else {
                continue;
            };
            let text = truncate(text, usize::from(owner.width));
            let x = usize::from(owner.width).saturating_sub(width(&text)) / 2;
            let line = Rect::new(owner.x, owner.y + row, owner.width, 1);
            paint_clipped(buf, line, x, &text, style);
        }
    }
}
