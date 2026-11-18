use ratatui::buffer::Buffer;
use ratatui::layout::Rect;
use ratatui::style::{Color, Modifier, Style};

use termrock_core::id::WidgetId;
use termrock_runtime::ctx::RenderCtx;
use termrock_runtime::measure::terminal_extent;
use termrock_text::cells::{truncate, wrap};
use termrock_theme::{Theme, Tone};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum NoticeKind {
    #[default]
    Error,

    Warning,

    Info,

    Success,
}

impl NoticeKind {
    fn glyph(self, t: &Theme) -> (&'static str, Style) {
        match self {
            NoticeKind::Error => ("!", t.error_fg().add_modifier(Modifier::BOLD)),
            NoticeKind::Warning => (t.glyphs.warning, Style::new().fg(t.warning)),
            NoticeKind::Info => (t.glyphs.info, Style::new().fg(t.info)),
            NoticeKind::Success => (t.glyphs.done, Style::new().fg(t.success)),
        }
    }

    pub fn message_tone(self) -> Tone {
        match self {
            NoticeKind::Error => Tone::Error,
            NoticeKind::Warning => Tone::Warning,
            NoticeKind::Info | NoticeKind::Success => Tone::Normal,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Notice {
    pub kind: NoticeKind,
    pub message: String,
    pub detail: Option<String>,
    pub hint: Option<String>,

    pub message_lines: usize,

    pub max_lines: usize,

    pub detail_tone: Tone,

    pub mark_cut: bool,
}

impl Notice {
    pub fn new(kind: NoticeKind, message: impl Into<String>) -> Self {
        Self {
            kind,
            message: message.into(),
            detail: None,
            hint: None,
            message_lines: 2,
            max_lines: 2,
            detail_tone: Tone::Secondary,
            mark_cut: true,
        }
    }
    pub fn error(message: impl Into<String>) -> Self {
        Self::new(NoticeKind::Error, message)
    }
    pub fn warning(message: impl Into<String>) -> Self {
        Self::new(NoticeKind::Warning, message)
    }
    pub fn info(message: impl Into<String>) -> Self {
        Self::new(NoticeKind::Info, message)
    }
    pub fn success(message: impl Into<String>) -> Self {
        Self::new(NoticeKind::Success, message)
    }

    pub fn kind(mut self, kind: NoticeKind) -> Self {
        self.kind = kind;
        self
    }

    pub fn detail(mut self, detail: impl Into<String>) -> Self {
        self.detail = Some(detail.into());
        self
    }

    pub fn hint(mut self, hint: impl Into<String>) -> Self {
        self.hint = Some(hint.into());
        self
    }

    pub fn max_lines(mut self, n: usize) -> Self {
        self.max_lines = n;
        self
    }

    pub fn message_lines(mut self, n: usize) -> Self {
        self.message_lines = n;
        self
    }

    pub fn detail_tone(mut self, tone: Tone) -> Self {
        self.detail_tone = tone;
        self
    }

    pub fn mark_cut(mut self, on: bool) -> Self {
        self.mark_cut = on;
        self
    }

    fn lines(&self, text: &str, w: usize, max: usize) -> Vec<String> {
        let wrapped = wrap(text, w.max(1));
        let cut = wrapped.len() > max;
        let mut lines: Vec<String> = wrapped.into_iter().take(max).collect();
        if cut
            && self.mark_cut
            && let Some(last) = lines.last_mut()
        {
            *last = truncate(&format!("{last} …"), w);
        }
        lines
    }

    /// Rows the notice needs at `width`, saturating at `u16::MAX`.
    pub fn height(&self, width: u16) -> u16 {
        let w = width.saturating_sub(2) as usize;
        let message = self.lines(&self.message, w, self.message_lines).len();
        let detail = self
            .detail
            .as_ref()
            .map_or(0, |d| self.lines(d, w, self.max_lines).len());
        terminal_extent(
            message
                .max(1)
                .saturating_add(detail)
                .saturating_add(usize::from(self.hint.is_some())),
        )
    }

    pub fn render(&self, area: Rect, buf: &mut Buffer, ctx: &mut RenderCtx, bg: Color) -> u16 {
        self.render_theme(area, buf, ctx.theme, bg)
    }

    pub fn render_theme(&self, area: Rect, buf: &mut Buffer, t: &Theme, bg: Color) -> u16 {
        let area = area.intersection(*buf.area());
        if area.is_empty() {
            return 0;
        }
        let (glyph, gs) = self.kind.glyph(t);
        let glyph_cell = Rect::new(area.x, area.y, area.width.min(2), 1);
        termrock_text::cells::paint_clipped(buf, glyph_cell, 0, glyph, gs.bg(bg));
        if area.width <= 2 {
            return 1;
        }
        let x = area.x + 2;
        let w = area.width.saturating_sub(2) as usize;
        let mut y = area.y;
        let mut put = |y: &mut u16, text: &str, style: Style| {
            if *y < area.bottom() {
                buf.set_string(x, *y, text, style.bg(bg));
                *y += 1;
            }
        };

        let reveal = |y: u16, text: &str, max: usize| {
            if max > 0 && y > area.y && y <= area.bottom() && wrap(text, w.max(1)).len() > max {
                termrock_runtime::reveal::queue_cut(Rect::new(x, y - 1, w as u16, 1), text);
            }
        };
        let message = Style::new().fg(t.tone(self.kind.message_tone()));
        let max = self.message_lines.max(1);
        let lines = self.lines(&self.message, w, max);
        if lines.is_empty() {
            y += 1;
        }
        for l in &lines {
            put(&mut y, l, message);
        }
        reveal(y, &self.message, max);
        if let Some(d) = &self.detail {
            let style = Style::new().fg(t.tone(self.detail_tone));
            for l in self.lines(d, w, self.max_lines) {
                put(&mut y, &l, style);
            }
            reveal(y, d, self.max_lines);
        }
        if let Some(h) = &self.hint {
            let row = Rect::new(x, y, w as u16, 1);
            put(&mut y, &termrock_runtime::reveal::cut(row, h), t.muted());
        }
        y.min(area.bottom()) - area.y
    }
}

impl termrock_runtime::measure::Measure for Notice {
    fn measure(&self, _avail: ratatui::layout::Size, _ctx: &RenderCtx) -> ratatui::layout::Size {
        let text = [
            Some(&self.message),
            self.detail.as_ref(),
            self.hint.as_ref(),
        ]
        .into_iter()
        .flatten()
        .map(|s| termrock_text::cells::width(s))
        .max()
        .unwrap_or(0);
        let width = terminal_extent(text.saturating_add(2));
        ratatui::layout::Size::new(width, self.height(width))
    }
}

const SAMPLE_ID: WidgetId = WidgetId::of("notice-sample");

impl termrock_runtime::specimen::Specimen for Notice {
    fn specimen_id(&self) -> WidgetId {
        SAMPLE_ID
    }

    fn supports(&self, state: termrock_runtime::specimen::SpecimenState) -> bool {
        use termrock_runtime::specimen::SpecimenState::*;
        matches!(state, Rest | Error | Overflow | Narrow)
    }

    fn specimen_height(&self, _state: termrock_runtime::specimen::SpecimenState) -> u16 {
        terminal_extent(
            self.message_lines
                .max(1)
                .saturating_add(if self.detail.is_some() {
                    self.max_lines
                } else {
                    0
                })
                .saturating_add(usize::from(self.hint.is_some())),
        )
    }

    fn specimen(
        &self,
        state: termrock_runtime::specimen::SpecimenState,
        area: Rect,
        buf: &mut Buffer,
        ctx: &mut RenderCtx,
        bg: Color,
    ) {
        use termrock_runtime::specimen::SpecimenState::*;
        let mut n = self.clone();
        match state {
            Error => n.kind = NoticeKind::Error,
            Overflow => {
                n.detail = Some(format!(
                    "{} and a cause far too long to fit in the lines it has",
                    n.detail.unwrap_or_default()
                ))
            }
            _ => {}
        }
        n.render(area, buf, ctx, bg);
    }
}
