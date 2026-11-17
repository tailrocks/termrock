use ratatui::buffer::Buffer;
use ratatui::layout::Rect;

use crate::core::event::{Key, Outcome};
use crate::core::hints::{HasHints, HintCx};
use crate::core::id::WidgetId;
use crate::core::refusal::{Control, Reason};
use crate::theme::{ButtonKind, Kind};
use crate::ui::ctx::RenderCtx;
use crate::ui::measure::terminal_extent;
use crate::ui::text::{paint_clipped, width};
use crate::widgets::keyhint::Hint;
use ratatui::crossterm::event::KeyCode;
use unicode_segmentation::UnicodeSegmentation;

#[derive(Debug, Clone)]
pub struct Button {
    pub id: WidgetId,
    pub label: String,
    pub kind: ButtonKind,
    pub disabled: bool,
    pub control: Control,

    pub on: Option<bool>,
    pub busy: bool,
    pub area: Rect,

    pub focus_visible: crate::ui::ctx::FocusVisible,

    pub mnemonic: Option<usize>,

    pub tip: Option<crate::ui::tooltip::Tip>,

    pub focusable: bool,
}

impl Button {
    pub fn new(id: WidgetId, label: &str, kind: ButtonKind) -> Self {
        Self {
            id,
            label: label.to_owned(),
            kind,
            disabled: false,
            control: Control::Live,
            on: None,
            busy: false,
            area: Rect::ZERO,
            focus_visible: crate::ui::ctx::FocusVisible::Always,
            mnemonic: None,
            tip: None,
            focusable: true,
        }
    }

    pub fn focusable(mut self, on: bool) -> Self {
        self.focusable = on;
        self
    }

    pub fn tooltip(mut self, text: &str) -> Self {
        let key = self.tip.take().and_then(|t| t.shortcut);
        self.tip = Some(crate::ui::tooltip::Tip {
            text: text.to_owned(),
            shortcut: key,
        });
        self
    }

    pub fn shortcut(mut self, key: &str) -> Self {
        let mut tip = self.tip.take().unwrap_or_default();
        tip.shortcut = Some(key.to_owned());
        self.tip = Some(tip);
        self
    }

    pub fn focus_visible(mut self, mode: crate::ui::ctx::FocusVisible) -> Self {
        self.focus_visible = mode;
        self
    }

    pub fn primary(id: WidgetId, label: &str) -> Self {
        Self::new(id, label, ButtonKind::Primary)
    }
    pub fn secondary(id: WidgetId, label: &str) -> Self {
        Self::new(id, label, ButtonKind::Secondary)
    }
    pub fn subtle(id: WidgetId, label: &str) -> Self {
        Self::new(id, label, ButtonKind::Subtle)
    }
    pub fn danger(id: WidgetId, label: &str) -> Self {
        Self::new(id, label, ButtonKind::Danger)
    }
    pub fn toggle(id: WidgetId, label: &str, on: bool) -> Self {
        let mut b = Self::new(id, label, ButtonKind::Toggle);
        b.on = Some(on);
        b
    }
    pub fn disabled(mut self, disabled: bool) -> Self {
        self.disabled = disabled;
        self
    }

    pub fn control(mut self, control: Control) -> Self {
        self.control = control;
        self
    }

    pub fn fixed(self, reason: Reason) -> Self {
        self.control(Control::fixed(reason))
    }

    /// Natural width of the button, saturating at `u16::MAX`.
    pub fn width(&self) -> u16 {
        terminal_extent(self.intrinsic_width())
    }

    pub(crate) fn intrinsic_width(&self) -> usize {
        let marker = if self.on.is_some() || self.busy { 2 } else { 0 };
        width(&self.label).saturating_add(2 + marker)
    }

    pub fn hint(&self) -> crate::widgets::keyhint::Hint {
        if self.busy {
            return crate::widgets::keyhint::Hint::new("", "Working…")
                .priority(crate::widgets::keyhint::HintPriority::Normal);
        }
        crate::widgets::keyhint::Hint::new("Enter", self.label.clone())
    }

    pub fn can_activate(&self) -> bool {
        !self.disabled && !self.busy && !self.control.is_fixed()
    }

    pub fn refuse(&self) -> bool {
        !self.disabled && !self.busy && self.control.refuse()
    }

    pub fn activate(&mut self) -> (Outcome, bool) {
        if self.refuse() {
            (Outcome::Consumed, false)
        } else if self.can_activate() {
            self.toggle_if_needed();
            (Outcome::Changed, true)
        } else {
            (Outcome::Consumed, false)
        }
    }

    pub fn on_key(&mut self, key: &Key) -> (Outcome, bool) {
        if key.is(KeyCode::Enter) || key.is_char(' ') {
            self.activate()
        } else {
            (Outcome::Ignored, false)
        }
    }

    pub fn on_click(&mut self) -> bool {
        self.activate().1
    }

    fn toggle_if_needed(&mut self) {
        if let Some(on) = self.on.as_mut() {
            *on = !*on;
        }
    }

    pub fn render(
        &mut self,
        area: Rect,
        buf: &mut Buffer,
        ctx: &mut RenderCtx,
        bg: ratatui::style::Color,
    ) {
        let area = area.intersection(*buf.area());
        let w = self.width().min(area.width);
        let area = Rect::new(area.x, area.y, w, 1.min(area.height));
        self.area = area;
        if area.is_empty() {
            return;
        }
        let mut s = ctx.state(self.id);
        s.focused = ctx.interaction.focus_visible(self.id, self.focus_visible);
        s.fixed = self.control.is_fixed();
        s.disabled = self.disabled || s.fixed;
        s.busy = self.busy;
        s.selected = self.on.unwrap_or(false);
        if s.disabled {
            s.hovered = false;
            s.pressed = false;
        }
        if self.busy {
            s.pressed = false;
        }
        let t = ctx.theme;

        let it = t.interactive(Kind::Button(self.kind), s, t.plane_of(bg));
        let style = it.style;
        let filled = it.on_fill;
        let busy_ink = style.fg.unwrap_or(t.text_secondary);
        let gutter = it.gutter_style;
        let mut text = String::new();
        if self.busy {
            text.push_str(t.glyphs.spinner_frame(ctx.interaction.tick));
            text.push(' ');
        } else if let Some(on) = self.on {
            text.push_str(if on {
                t.glyphs.dot_on
            } else {
                t.glyphs.dot_off
            });
            text.push(' ');
        }
        text.push_str(&self.label);
        let content = Rect::new(
            area.x.saturating_add(1),
            area.y,
            w.saturating_sub(2),
            area.height,
        );
        let text = crate::ui::text::fit(&text, usize::from(content.width));
        paint_clipped(buf, area, 0, it.gutter, gutter);
        paint_clipped(buf, content, 0, &text, style);

        if let Some(on) = self.on
            && !s.disabled
        {
            let ms = style.fg(if on { t.accent } else { t.text_muted });
            let ms = if s.pressed { style } else { ms };
            paint_clipped(
                buf,
                content,
                0,
                if on {
                    t.glyphs.dot_on
                } else {
                    t.glyphs.dot_off
                },
                ms,
            );
        }
        if self.busy {
            paint_clipped(
                buf,
                content,
                0,
                t.glyphs.spinner_frame(ctx.interaction.tick),
                style.fg(if filled { busy_ink } else { t.accent }),
            );
        }

        if let Some(mi) = self.mnemonic
            && !self.busy
            && self.on.is_none()
            && text == self.label
        {
            let mut scalars = 0;
            let mut col = 0;
            for g in self.label.graphemes(true) {
                let cells = width(g);
                scalars += g.chars().count();
                if mi < scalars {
                    for dx in 0..cells {
                        let x = usize::from(content.x) + col + dx;
                        if let Ok(x) = u16::try_from(x)
                            && content.contains((x, content.y).into())
                            && let Some(cell) = buf.cell_mut((x, content.y))
                        {
                            cell.set_style(
                                style.add_modifier(ratatui::style::Modifier::UNDERLINED),
                            );
                        }
                    }
                    break;
                }
                col += cells;
            }
        }
        if w >= 2 {
            paint_clipped(
                buf,
                Rect::new(area.right() - 1, area.y, 1, 1),
                0,
                " ",
                style,
            );
        }
        if self.focusable {
            ctx.control(self.id, area, self.disabled);
        } else {
            ctx.clickable(self.id, area);
        }
        if let Some(tip) = &self.tip {
            crate::ui::tooltip::show(ctx, self.id, area, tip, None);
        }
    }
}

pub(crate) fn wrap_lines(
    width: usize,
    widths: &[usize],
    gap: usize,
    max_lines: usize,
) -> Vec<Vec<usize>> {
    let mut lines: Vec<Vec<usize>> = vec![vec![]];
    let mut used = 0usize;
    for (i, &w) in widths.iter().enumerate() {
        let line = lines.last_mut().expect("one line");
        let need = if line.is_empty() {
            w
        } else {
            used.saturating_add(gap).saturating_add(w)
        };
        if line.is_empty() || need <= width {
            line.push(i);
            used = need;
        } else if lines.len() < max_lines {
            lines.push(vec![i]);
            used = w;
        } else {
            break;
        }
    }
    lines
}

fn wide(widths: &[u16]) -> Vec<usize> {
    widths.iter().map(|&w| usize::from(w)).collect()
}

/// Number of lines the buttons wrap onto, saturating at `u16::MAX`.
pub fn row_lines(width: u16, widths: &[u16], gap: u16) -> u16 {
    if widths.is_empty() {
        return 1;
    }
    terminal_extent(
        wrap_lines(
            usize::from(width),
            &wide(widths),
            usize::from(gap),
            usize::MAX,
        )
        .len(),
    )
}

pub fn row_layout(area: Rect, widths: &[u16], gap: u16) -> Vec<Rect> {
    place_lines(area, &wide(widths), gap, false)
}

pub fn row_layout_right(area: Rect, widths: &[u16], gap: u16) -> Vec<Rect> {
    place_lines(area, &wide(widths), gap, true)
}

/// One-cell-high row `row_index` of `area`, or `None` once the allocation has
/// no such row. `area` must already be normalized to representable bounds.
fn row_at(area: Rect, row_index: usize) -> Option<Rect> {
    if area.width == 0 || row_index >= usize::from(area.bottom() - area.y) {
        return None;
    }
    Some(Rect::new(area.x, area.y + row_index as u16, area.width, 1))
}

/// Places `line` (index, intrinsic width) on row `y` of `area`, aligned to its
/// start or end. Every returned rectangle is inside `area` or `Rect::ZERO`.
fn place_line(area: Rect, y: u16, line: &[(usize, usize)], gap: u16, right: bool) -> Vec<Rect> {
    let total = line
        .iter()
        .map(|&(_, w)| w)
        .fold(0usize, usize::saturating_add)
        .saturating_add(usize::from(gap).saturating_mul(line.len().saturating_sub(1)));
    let total = terminal_extent(total).min(area.width);
    let mut x = if right { area.right() - total } else { area.x };
    line.iter()
        .map(|&(_, w)| {
            let w = terminal_extent(w.min(usize::from(area.right().saturating_sub(x))));
            let r = if w == 0 {
                Rect::ZERO
            } else {
                Rect::new(x, y, w, 1)
            };
            x = x.saturating_add(w).saturating_add(gap);
            r
        })
        .collect()
}

/// Wraps and places buttons from their wide intrinsic widths, so a button wider
/// than any `u16` allocation never looks like it fits. A button that has no row
/// or no width left is `Rect::ZERO`.
pub(crate) fn place_lines(area: Rect, wide: &[usize], gap: u16, right: bool) -> Vec<Rect> {
    let area = area.intersection(area);
    let mut out = vec![Rect::ZERO; wide.len()];
    for (k, line) in wrap_lines(
        usize::from(area.width),
        wide,
        usize::from(gap),
        usize::from(area.height),
    )
    .iter()
    .enumerate()
    {
        let Some(row) = row_at(area, k) else { break };
        let line: Vec<(usize, usize)> = line.iter().map(|&i| (i, wide[i])).collect();
        for (&(i, _), r) in line.iter().zip(place_line(area, row.y, &line, gap, right)) {
            out[i] = r;
        }
    }
    out
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum RowAlign {
    #[default]
    Start,

    End,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum RowOverflow {
    #[default]
    More,

    Wrap,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct ButtonRow {
    pub align: RowAlign,

    pub gap: Option<u16>,

    pub overflow: RowOverflow,

    pub wrap: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct RowLayout {
    pub rects: Vec<Option<Rect>>,

    pub more: Option<Rect>,

    pub hidden: Vec<usize>,
}

pub const MORE_LABEL: &str = "…";

impl ButtonRow {
    pub fn new() -> Self {
        Self::default()
    }
    pub fn align(mut self, align: RowAlign) -> Self {
        self.align = align;
        self
    }
    pub fn gap(mut self, gap: u16) -> Self {
        self.gap = Some(gap);
        self
    }
    pub fn overflow(mut self, overflow: RowOverflow) -> Self {
        self.overflow = overflow;
        self
    }

    pub fn wrap(mut self, on: bool) -> Self {
        self.wrap = on;
        self
    }

    pub fn step<'a>(
        &self,
        buttons: impl IntoIterator<Item = &'a Button>,
        cur: Option<WidgetId>,
        forward: bool,
    ) -> Option<WidgetId> {
        use crate::core::list::{Cursor, Moved, RowFn};
        let row: Vec<&Button> = buttons.into_iter().collect();
        let i = row.iter().position(|b| Some(b.id) == cur)?;
        let enabled = |j: usize| !row[j].disabled;
        let label = |j: usize| row[j].label.as_str();
        let src = RowFn {
            len: row.len(),
            enabled: &enabled,
            label: &label,
        };
        let mut c = Cursor::new(i)
            .axis(crate::core::hit::Axis::Horizontal)
            .wrap(self.wrap);
        match c.step(if forward { 1 } else { -1 }, &src) {
            Moved::To(j) => Some(row[j].id),
            _ => None,
        }
    }

    pub fn more_id(owner: WidgetId) -> WidgetId {
        owner.sub("more")
    }

    fn gap_for(&self, theme: &crate::theme::Theme) -> u16 {
        self.gap.unwrap_or(theme.spacing.button_gap)
    }

    fn primary(buttons: &[Button]) -> Option<usize> {
        buttons
            .iter()
            .position(|b| matches!(b.kind, ButtonKind::Primary | ButtonKind::Danger))
            .or_else(|| buttons.len().checked_sub(1))
    }

    fn line_count(width: u16, widths: &[usize], gap: u16) -> u16 {
        terminal_extent(wrap_lines(usize::from(width), widths, usize::from(gap), usize::MAX).len())
    }

    fn intrinsic_widths(buttons: &[Button]) -> Vec<usize> {
        buttons.iter().map(Button::intrinsic_width).collect()
    }

    /// Natural width of the whole row, saturating at `u16::MAX`.
    pub fn width(&self, buttons: &[Button], theme: &crate::theme::Theme) -> u16 {
        terminal_extent(
            Self::intrinsic_widths(buttons)
                .iter()
                .fold(0usize, |a, w| a.saturating_add(*w))
                .saturating_add(
                    usize::from(self.gap_for(theme))
                        .saturating_mul(buttons.len().saturating_sub(1)),
                ),
        )
    }

    pub fn rows(&self, width: u16, buttons: &[Button], theme: &crate::theme::Theme) -> u16 {
        match self.overflow {
            RowOverflow::More => 1,
            RowOverflow::Wrap => {
                let widths = Self::intrinsic_widths(buttons);
                Self::line_count(width, &widths, self.gap_for(theme))
            }
        }
    }

    pub fn rows_at_gap(&self, width: u16, buttons: &[Button], gap: u16) -> u16 {
        match self.overflow {
            RowOverflow::More => 1,
            RowOverflow::Wrap => {
                let widths = Self::intrinsic_widths(buttons);
                Self::line_count(width, &widths, self.gap.unwrap_or(gap))
            }
        }
    }

    pub fn layout(&self, area: Rect, buttons: &[Button], theme: &crate::theme::Theme) -> RowLayout {
        let gap = self.gap_for(theme);
        let widths = Self::intrinsic_widths(buttons);
        let mut out = RowLayout {
            rects: vec![None; buttons.len()],
            ..Default::default()
        };
        let area = area.intersection(area);
        if area.is_empty() || buttons.is_empty() {
            return out;
        }
        let right = self.align == RowAlign::End;
        let some = |r: Rect| Some(r).filter(|r| !r.is_empty());
        match self.overflow {
            RowOverflow::Wrap => {
                for (i, r) in place_lines(area, &widths, gap, right)
                    .into_iter()
                    .enumerate()
                {
                    out.rects[i] = some(r);
                }
            }
            RowOverflow::More => {
                let (gap_w, n) = (usize::from(gap), widths.len());
                let total = widths
                    .iter()
                    .fold(0usize, |a, w| a.saturating_add(*w))
                    .saturating_add(gap_w.saturating_mul(n - 1));
                let place = |line: &[(usize, usize)], out: &mut RowLayout| -> Vec<Rect> {
                    let rects = place_line(area, area.y, line, gap, right);
                    for (&(i, _), r) in line.iter().zip(&rects) {
                        if i < out.rects.len() {
                            out.rects[i] = some(*r);
                        }
                    }
                    rects
                };
                if total <= usize::from(area.width) {
                    let line: Vec<(usize, usize)> = widths.iter().copied().enumerate().collect();
                    place(&line, &mut out);
                    return out;
                }

                let primary = Self::primary(buttons).unwrap_or(0);
                let more_w = width(theme.glyphs.ellipsis) + 2;
                let mut budget = usize::from(area.width)
                    .saturating_sub(widths[primary].saturating_add(gap_w).saturating_add(more_w));
                let mut keep = vec![false; buttons.len()];
                keep[primary] = true;
                for (i, &w) in widths.iter().enumerate() {
                    if i == primary {
                        continue;
                    }
                    let need = w.saturating_add(gap_w);
                    if need <= budget {
                        budget -= need;
                        keep[i] = true;
                    } else {
                        break;
                    }
                }
                out.hidden = (0..buttons.len()).filter(|i| !keep[*i]).collect();

                let mut line: Vec<(usize, usize)> = Vec::new();
                for (i, &w) in widths.iter().enumerate() {
                    if keep[i] && i != primary {
                        line.push((i, w));
                    }
                }
                const MORE: usize = usize::MAX;
                if primary + 1 == buttons.len() {
                    line.push((MORE, more_w));
                    line.push((primary, widths[primary]));
                } else {
                    line.insert(
                        line.iter()
                            .position(|(i, _)| *i > primary)
                            .unwrap_or(line.len()),
                        (primary, widths[primary]),
                    );
                    line.push((MORE, more_w));
                }
                let rects = place(&line, &mut out);
                out.more = line
                    .iter()
                    .zip(rects)
                    .find(|((i, _), _)| *i == MORE)
                    .map(|(_, r)| r)
                    .filter(|r| !r.is_empty());
            }
        }
        out
    }

    pub fn render(
        &self,
        owner: WidgetId,
        area: Rect,
        buttons: &mut [Button],
        buf: &mut Buffer,
        ctx: &mut RenderCtx,
        bg: ratatui::style::Color,
    ) -> RowLayout {
        let layout = self.layout(area, buttons, ctx.theme);
        for (b, r) in buttons.iter_mut().zip(&layout.rects) {
            match r {
                Some(r) => b.render(*r, buf, ctx, bg),
                None => b.area = Rect::ZERO,
            }
        }
        if let Some(r) = layout.more {
            let mut more = Button::subtle(Self::more_id(owner), ctx.theme.glyphs.ellipsis);
            more.render(r, buf, ctx, bg);
        }
        layout
    }
}

impl crate::core::pointer::MouseTarget for Button {
    type Action = ();

    fn mouse_id(&self) -> WidgetId {
        self.id
    }

    fn pointer(
        &mut self,
        ev: &crate::core::pointer::PointerEvent,
    ) -> crate::core::response::Response<()> {
        use crate::core::response::Response;
        if !ev.is(crate::core::intent::Phase::Click) {
            return Response::ignored();
        }
        if self.on_click() {
            Response::emit(())
        } else {
            Response::consumed()
        }
    }
}

impl HasHints for Button {
    fn hints(&self, _: &HintCx) -> Vec<Hint> {
        if self.disabled {
            return vec![];
        }
        vec![self.hint()]
    }
}
