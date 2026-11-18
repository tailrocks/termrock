use ratatui::buffer::Buffer;
use ratatui::layout::Rect;
use ratatui::style::{Color, Style};

use termrock_runtime::measure::terminal_extent;
use termrock_text::cells::width;
use termrock_theme::{Theme, Tone};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PageHeader {
    pub title: String,

    pub status: Option<(String, Tone)>,

    pub right: Option<String>,

    pub right_gap: u16,

    pub subtitle: Option<(String, Tone)>,

    pub inset: u16,
}

impl PageHeader {
    pub fn new(title: &str) -> Self {
        Self {
            title: title.to_owned(),
            status: None,
            right: None,
            right_gap: 4,
            subtitle: None,
            inset: 1,
        }
    }

    pub fn status(mut self, text: &str, tone: Tone) -> Self {
        self.status = Some((text.to_owned(), tone));
        self
    }

    pub fn right(mut self, text: &str) -> Self {
        self.right = Some(text.to_owned());
        self
    }

    pub fn right_gap(mut self, cells: u16) -> Self {
        self.right_gap = cells;
        self
    }

    pub fn subtitle(mut self, text: &str, tone: Tone) -> Self {
        self.subtitle = Some((text.to_owned(), tone));
        self
    }

    pub fn inset(mut self, cells: u16) -> Self {
        self.inset = cells;
        self
    }

    pub fn height(&self) -> u16 {
        1 + u16::from(self.subtitle.is_some())
    }

    pub fn body(&self, area: Rect) -> Rect {
        let off = (self.height() + 1).min(area.height);
        Rect::new(area.x, area.y + off, area.width, area.height - off)
    }

    pub fn render(&self, area: Rect, buf: &mut Buffer, t: &Theme) -> u16 {
        let area = area.intersection(*buf.area());
        if area.is_empty() {
            return 0;
        }
        let x = area.x + self.inset.min(area.width);
        let inset = usize::from(self.inset);
        let title_w = width(&self.title);

        buf.set_stringn(
            x,
            area.y,
            &self.title,
            area.right().saturating_sub(x) as usize,
            t.title(),
        );
        let mut used = title_w;
        if let Some((status, tone)) = &self.status {
            let left = usize::from(area.width).saturating_sub(title_w.saturating_add(2 * inset));
            if left > 0 {
                let sx = x.saturating_add(terminal_extent(title_w));
                let s = termrock_runtime::reveal::cut(
                    Rect::new(sx, area.y, terminal_extent(left), 1),
                    status,
                );
                used = used.saturating_add(width(&s));
                buf.set_string(sx, area.y, s, ink(t, *tone));
            }
        }
        if let Some(right) = &self.right {
            let rw = width(right);
            if rw
                .saturating_add(used)
                .saturating_add(usize::from(self.right_gap))
                < usize::from(area.width)
            {
                buf.set_string(
                    area.right()
                        .saturating_sub(terminal_extent(rw.saturating_add(inset))),
                    area.y,
                    right,
                    t.muted(),
                );
            }
        }
        let Some((sub, tone)) = &self.subtitle else {
            return 1;
        };
        if area.height < 2 {
            return 1;
        }
        let row = Rect::new(x, area.y + 1, area.width.saturating_sub(2 * self.inset), 1);
        buf.set_string(
            x,
            row.y,
            termrock_runtime::reveal::cut(row, sub),
            ink(t, *tone),
        );
        2
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Heading {
    pub text: String,

    pub meta: Option<String>,

    pub tone: Tone,
}

impl Heading {
    pub fn new(text: &str) -> Self {
        Self {
            text: text.to_owned(),
            meta: None,
            tone: Tone::Muted,
        }
    }

    pub fn meta(mut self, meta: &str) -> Self {
        self.meta = Some(meta.to_owned());
        self
    }

    pub fn tone(mut self, tone: Tone) -> Self {
        self.tone = tone;
        self
    }

    pub fn render(&self, area: Rect, buf: &mut Buffer, t: &Theme, bg: Color) {
        let area = area.intersection(*buf.area());
        if area.is_empty() {
            return;
        }
        let ink = Style::new().fg(t.tone_on(self.tone, bg)).bg(bg);
        buf.set_string(
            area.x,
            area.y,
            termrock_runtime::reveal::cut(Rect::new(area.x, area.y, area.width, 1), &self.text),
            ink,
        );
        if let Some(meta) = &self.meta {
            let mw = width(meta);
            if width(&self.text).saturating_add(2).saturating_add(mw) <= usize::from(area.width) {
                buf.set_string(
                    area.right() - terminal_extent(mw),
                    area.y,
                    meta,
                    Style::new().fg(t.tone_on(Tone::Faint, bg)).bg(bg),
                );
            }
        }
    }
}

impl termrock_runtime::measure::Measure for PageHeader {
    fn measure(
        &self,
        _avail: ratatui::layout::Size,
        _ctx: &termrock_runtime::ctx::RenderCtx,
    ) -> ratatui::layout::Size {
        let inset = usize::from(self.inset);
        let used =
            width(&self.title).saturating_add(self.status.as_ref().map_or(0, |(s, _)| width(s)));
        let chrome = used.saturating_add(2 * inset);
        let row = match &self.right {
            Some(r) => used
                .saturating_add(usize::from(self.right_gap))
                .saturating_add(width(r))
                .saturating_add(1)
                .max(chrome),
            None => chrome,
        };
        let sub = self
            .subtitle
            .as_ref()
            .map_or(0, |(s, _)| width(s).saturating_add(2 * inset));
        ratatui::layout::Size::new(terminal_extent(row.max(sub)), self.height())
    }
}

impl termrock_runtime::measure::Measure for Heading {
    fn measure(
        &self,
        _avail: ratatui::layout::Size,
        _ctx: &termrock_runtime::ctx::RenderCtx,
    ) -> ratatui::layout::Size {
        let meta = self
            .meta
            .as_deref()
            .map_or(0, |m| width(m).saturating_add(2));
        ratatui::layout::Size::new(terminal_extent(width(&self.text).saturating_add(meta)), 1)
    }
}

fn ink(t: &Theme, tone: Tone) -> Style {
    Style::new().fg(t.tone(tone))
}
