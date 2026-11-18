use ratatui::buffer::Buffer;
use ratatui::layout::Rect;
use ratatui::style::Color;

use termrock_controls::button::{Button, place_lines};
use termrock_core::id::WidgetId;
use termrock_runtime::ctx::RenderCtx;
use termrock_runtime::measure::terminal_extent;
use termrock_theme::Theme;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum EmptyKind {
    #[default]
    Empty,

    Error,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EmptyAction {
    pub id: WidgetId,
    pub label: String,

    pub primary: bool,
}

impl EmptyAction {
    pub fn primary(id: WidgetId, label: &str) -> Self {
        Self {
            id,
            label: label.to_owned(),
            primary: true,
        }
    }
    pub fn secondary(id: WidgetId, label: &str) -> Self {
        Self {
            id,
            label: label.to_owned(),
            primary: false,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct EmptyState {
    pub title: String,
    pub hint: Option<String>,
    pub kind: EmptyKind,

    pub actions: Vec<EmptyAction>,
}

impl EmptyState {
    pub fn new(title: &str) -> Self {
        Self {
            title: title.to_owned(),
            hint: None,
            kind: EmptyKind::Empty,
            actions: vec![],
        }
    }
    pub fn error(title: &str) -> Self {
        Self {
            title: title.to_owned(),
            hint: None,
            kind: EmptyKind::Error,
            actions: vec![],
        }
    }

    pub fn actions(mut self, actions: Vec<EmptyAction>) -> Self {
        self.actions = actions;
        self
    }

    pub fn on_click(&self, id: WidgetId) -> Option<&EmptyAction> {
        self.actions.iter().find(|a| a.id == id)
    }

    pub fn on_key(
        &self,
        focus: WidgetId,
        key: &termrock_core::event::Key,
    ) -> (termrock_core::event::Outcome, Option<&EmptyAction>) {
        let Some(a) = self.actions.iter().find(|a| a.id == focus) else {
            return (termrock_core::event::Outcome::Ignored, None);
        };
        let (o, act) = Button::secondary(a.id, &a.label).on_key(key);
        (o, act.then_some(a))
    }
    pub fn hint(mut self, h: &str) -> Self {
        self.hint = Some(h.to_owned());
        self
    }
}

pub fn render_ctx(area: Rect, buf: &mut Buffer, ctx: &mut RenderCtx, e: &EmptyState, bg: Color) {
    let area = area.intersection(*buf.area());
    if e.actions.is_empty() || area.height < 4 {
        render(area, buf, ctx.theme, e, bg);
        return;
    }

    let text = Rect::new(area.x, area.y, area.width, area.height.saturating_sub(2));
    render(text, buf, ctx.theme, e, bg);
    let hint_rows = e
        .hint
        .as_ref()
        .map(|h| {
            termrock_text::cells::wrap(h, area.width.saturating_sub(4).max(8) as usize).len() + 1
        })
        .unwrap_or(0);
    let total = terminal_extent(1usize.saturating_add(hint_rows));
    let y = text
        .y
        .saturating_add(text.height.saturating_sub(total) / 2)
        .saturating_add(total)
        .saturating_add(1);
    if y >= area.bottom() {
        return;
    }
    let mut buttons: Vec<Button> = e
        .actions
        .iter()
        .map(|a| {
            if a.primary {
                Button::primary(a.id, &a.label)
            } else {
                Button::secondary(a.id, &a.label)
            }
        })
        .collect();
    let gap = ctx.theme.spacing.button_gap;
    let widths: Vec<usize> = buttons.iter().map(Button::intrinsic_width).collect();
    let total_w = button_row_width(&widths, gap);
    let x = area.x + area.width.saturating_sub(terminal_extent(total_w)) / 2;
    let rects = place_lines(
        Rect::new(x, y, terminal_extent(total_w).min(area.width), 1),
        &widths,
        gap,
        false,
    );
    for (b, r) in buttons.iter_mut().zip(rects) {
        b.render(r, buf, ctx, bg);
    }
}

pub fn render(area: Rect, buf: &mut Buffer, t: &Theme, e: &EmptyState, bg: Color) {
    let area = area.intersection(*buf.area());
    if area.is_empty() {
        return;
    }
    let hint_lines: Vec<String> = e
        .hint
        .as_ref()
        .map(|h| termrock_text::cells::wrap(h, area.width.saturating_sub(4).max(8) as usize))
        .unwrap_or_default();
    let total = terminal_extent(
        1 + if hint_lines.is_empty() {
            0
        } else {
            hint_lines.len() + 1
        },
    );
    let y0 = area.y + area.height.saturating_sub(total) / 2;
    let put = |buf: &mut Buffer, y: u16, s: &str, style: ratatui::style::Style| {
        if y >= area.bottom() {
            return;
        }
        let s = termrock_text::cells::truncate(s, area.width as usize);
        let x = area.x
            + area
                .width
                .saturating_sub(termrock_text::cells::width(&s) as u16)
                / 2;
        buf.set_string(x, y, &s, style.bg(bg));
    };
    match e.kind {
        EmptyKind::Empty => put(buf, y0, &e.title, t.muted()),
        EmptyKind::Error => {
            let title = format!("! {}", e.title);
            put(buf, y0, &title, t.error_fg());

            let s = termrock_text::cells::truncate(&title, area.width as usize);
            let x = area.x
                + area
                    .width
                    .saturating_sub(termrock_text::cells::width(&s) as u16)
                    / 2;
            if y0 < area.bottom() {
                buf.set_string(
                    x,
                    y0,
                    "!",
                    t.error_fg()
                        .bg(bg)
                        .add_modifier(ratatui::style::Modifier::BOLD),
                );
            }
        }
    }
    for (i, l) in hint_lines.iter().enumerate() {
        put(
            buf,
            y0.saturating_add(2).saturating_add(terminal_extent(i)),
            l,
            t.faint(),
        );
    }
}

const SAMPLE_ID: WidgetId = WidgetId::of("empty-sample");

/// Rows the state needs at `width`, saturating at `u16::MAX`.
pub fn height(e: &EmptyState, width: u16) -> u16 {
    let hint = e
        .hint
        .as_ref()
        .map(|h| termrock_text::cells::wrap(h, width.saturating_sub(4).max(8) as usize).len() + 1)
        .unwrap_or(0);
    let actions = if e.actions.is_empty() { 0 } else { 2 };
    terminal_extent(hint.saturating_add(1 + actions))
}

fn button_row_width(widths: &[usize], gap: u16) -> usize {
    widths
        .iter()
        .fold(0usize, |a, w| a.saturating_add(*w))
        .saturating_add(usize::from(gap).saturating_mul(widths.len().saturating_sub(1)))
}

impl EmptyState {
    fn action_row_width(&self, gap: u16) -> usize {
        let widths: Vec<usize> = self
            .actions
            .iter()
            .map(|a| Button::secondary(a.id, &a.label).intrinsic_width())
            .collect();
        button_row_width(&widths, gap)
    }

    fn title_width(&self) -> usize {
        termrock_text::cells::width(&self.title).saturating_add(if self.kind == EmptyKind::Error {
            2
        } else {
            0
        })
    }
}

impl termrock_runtime::measure::Measure for EmptyState {
    fn measure(&self, _avail: ratatui::layout::Size, ctx: &RenderCtx) -> ratatui::layout::Size {
        let hint = self
            .hint
            .as_deref()
            .map(|h| termrock_text::cells::width(h).saturating_add(4))
            .unwrap_or(0);
        let buttons = self.action_row_width(ctx.theme.spacing.button_gap);
        let width = terminal_extent(self.title_width().max(hint).max(buttons));
        ratatui::layout::Size::new(width, height(self, width))
    }

    fn min_width(&self, avail: ratatui::layout::Size, ctx: &RenderCtx) -> u16 {
        let w = termrock_text::cells::width;
        let full = self.measure(avail, ctx).width;
        let hint = self.hint.as_deref().map_or(0, |h| {
            let word = h.split_whitespace().map(w).max().unwrap_or(0);
            (w(h).min(usize::from(HINT_MIN)))
                .max(word)
                .saturating_add(4)
        });
        let buttons = self.action_row_width(ctx.theme.spacing.button_gap);
        terminal_extent(self.title_width().max(hint).max(buttons)).min(full)
    }
}

const HINT_MIN: u16 = 24;

impl termrock_runtime::specimen::Specimen for EmptyState {
    fn specimen_id(&self) -> WidgetId {
        self.actions.first().map_or(SAMPLE_ID, |a| a.id)
    }

    fn supports(&self, state: termrock_runtime::specimen::SpecimenState) -> bool {
        use termrock_runtime::specimen::SpecimenState::*;
        match state {
            Rest | Error | Overflow | Narrow => true,
            Hover | Pressed | Focused | FocusedHover => !self.actions.is_empty(),
            _ => false,
        }
    }

    fn specimen_height(&self, _state: termrock_runtime::specimen::SpecimenState) -> u16 {
        height(self, 24) + u16::from(self.hint.is_some())
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
        let mut e = self.clone();
        match state {
            Error => e.kind = EmptyKind::Error,
            Overflow => e.title = format!("{} in a view far too narrow for it", e.title),
            _ => {}
        }
        render_ctx(area, buf, ctx, &e, bg);
    }
}
