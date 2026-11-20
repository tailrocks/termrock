use std::borrow::Cow;

use ratatui::buffer::Buffer;
use ratatui::layout::Rect;
use ratatui::style::Color;

use termrock_feedback::empty::{EmptyKind, EmptyState};
use termrock_runtime::ctx::RenderCtx;
use termrock_theme::Theme;

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub enum ContentState {
    #[default]
    Ready,

    Loading(Cow<'static, str>),

    Error(EmptyState),
}

impl ContentState {
    pub fn loading(label: impl Into<Cow<'static, str>>) -> Self {
        Self::Loading(label.into())
    }

    pub fn error(state: EmptyState) -> Self {
        Self::Error(EmptyState {
            kind: EmptyKind::Error,
            ..state
        })
    }

    pub fn is_ready(&self) -> bool {
        matches!(self, Self::Ready)
    }

    pub fn actions(&self) -> &[termrock_feedback::empty::EmptyAction] {
        match self {
            Self::Error(e) => &e.actions,
            _ => &[],
        }
    }
}

pub fn draw_line(area: Rect, buf: &mut Buffer, t: &Theme, text: &str, bg: Color) {
    let area = area.intersection(*buf.area());
    if area.is_empty() {
        return;
    }
    let msg = termrock_text::cells::truncate(text, area.width as usize);
    let y = area.y + area.height / 2;
    let x = area.x
        + area
            .width
            .saturating_sub(termrock_text::cells::width(&msg) as u16)
            / 2;
    buf.set_string(x, y, &msg, t.muted().bg(bg));
}

pub fn draw_loading(area: Rect, buf: &mut Buffer, ctx: &mut RenderCtx, label: &str, bg: Color) {
    let area = area.intersection(*buf.area());
    if area.is_empty() {
        return;
    }
    let label = if label.is_empty() {
        ctx.theme.strings.loading_rows
    } else {
        label
    };
    let glyph =
        termrock_text::cells::width(ctx.theme.glyphs.spinner_frame(ctx.interaction.tick)) as u16;
    let label =
        termrock_text::cells::truncate(label, area.width.saturating_sub(glyph + 1) as usize);
    let w = glyph + 1 + termrock_text::cells::width(&label) as u16;
    let x = area.x + area.width.saturating_sub(w) / 2;
    let y = area.y + area.height / 2;
    let owner = Rect::new(x, y, w, 1).intersection(area);
    termrock_feedback::progress::render_spinner(owner, buf, ctx, &label, bg);
}

pub fn draw_empty(area: Rect, buf: &mut Buffer, ctx: &mut RenderCtx, e: &EmptyState, bg: Color) {
    if e.kind == EmptyKind::Empty && e.hint.is_none() && e.actions.is_empty() {
        draw_line(area, buf, ctx.theme, &e.title, bg);
    } else {
        termrock_feedback::empty::render_ctx(area, buf, ctx, e, bg);
    }
}

pub fn specimen(state: termrock_runtime::specimen::SpecimenState) -> ContentState {
    use termrock_runtime::specimen::SpecimenState;
    match state {
        SpecimenState::Loading => ContentState::loading(""),
        SpecimenState::Error => ContentState::error(EmptyState::new("Could not load")),
        _ => ContentState::Ready,
    }
}
