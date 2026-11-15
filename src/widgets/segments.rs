use ratatui::buffer::Buffer;
use ratatui::layout::Rect;
use ratatui::style::{Color, Modifier};

use crate::core::id::WidgetId;
use crate::theme::Tone;
use crate::ui::ctx::RenderCtx;
use crate::ui::measure::terminal_extent;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Segment {
    pub text: String,
    pub tone: Tone,
    pub bold: bool,
    pub id: Option<WidgetId>,

    pub priority: u8,

    pub lead: Option<(String, Tone)>,
}

impl Segment {
    pub fn new(text: impl Into<String>, tone: Tone) -> Self {
        Self {
            text: text.into(),
            tone,
            bold: false,
            id: None,
            priority: 5,
            lead: None,
        }
    }
    pub fn bold(mut self) -> Self {
        self.bold = true;
        self
    }
    pub fn clickable(mut self, id: WidgetId) -> Self {
        self.id = Some(id);
        self
    }
    pub fn priority(mut self, p: u8) -> Self {
        self.priority = p;
        self
    }

    pub fn lead(mut self, glyph: impl Into<String>, tone: Tone) -> Self {
        self.lead = Some((glyph.into(), tone));
        self
    }

    fn shown(&self) -> String {
        match &self.lead {
            Some((g, _)) => format!("{g} {}", self.text),
            None => self.text.clone(),
        }
    }
}

const SEP: usize = 2;

/// Natural width of both segment groups, saturating at `u16::MAX`.
pub fn width(left: &[Segment], right: &[Segment], t: &crate::theme::Theme) -> u16 {
    terminal_extent(intrinsic_width(left, right, t))
}

fn intrinsic_width(left: &[Segment], right: &[Segment], t: &crate::theme::Theme) -> usize {
    left.iter()
        .chain(right)
        .map(|s| crate::ui::text::width(&s.shown()).saturating_add(SEP))
        .fold(0usize, usize::saturating_add)
        .saturating_sub(SEP)
        .saturating_add(2 * usize::from(t.spacing.chrome_edge))
}

/// Narrowest width that still shows the strongest segment, saturating at
/// `u16::MAX`.
pub fn min_width(left: &[Segment], right: &[Segment], t: &crate::theme::Theme) -> u16 {
    let strongest = left
        .iter()
        .chain(right)
        .max_by_key(|s| s.priority)
        .map_or(0, |s| crate::ui::text::width(&s.shown()));
    terminal_extent(
        strongest
            .saturating_add(2 + 1)
            .saturating_add(2 * usize::from(t.spacing.chrome_edge)),
    )
}

pub fn render(
    area: Rect,
    buf: &mut Buffer,
    ctx: &mut RenderCtx,
    left: &[Segment],
    right: &[Segment],
    bg: Color,
) {
    let area = area.intersection(*buf.area());
    if area.is_empty() {
        return;
    }
    let t = ctx.theme;
    let sep = SEP as u16;

    let edge = t.spacing.chrome_edge;
    let w = |s: &Segment| crate::ui::text::width(&s.shown());

    let mut keep_l: Vec<bool> = vec![true; left.len()];
    let mut keep_r: Vec<bool> = vec![true; right.len()];
    let total = |kl: &[bool], kr: &[bool]| -> usize {
        let kept = |segs: &[Segment], keep: &[bool]| {
            segs.iter()
                .zip(keep)
                .filter(|(_, k)| **k)
                .map(|(s, _)| w(s).saturating_add(SEP))
                .fold(0usize, usize::saturating_add)
        };
        let gone = kl.iter().chain(kr).any(|k| !k);
        let more = if gone { 1 + SEP } else { 0 };

        kept(left, kl)
            .saturating_add(kept(right, kr))
            .saturating_add(more)
            .saturating_sub(SEP)
            .saturating_add(2 * usize::from(edge))
    };
    while total(&keep_l, &keep_r) > usize::from(area.width) {
        let mut best: Option<(u8, bool, usize)> = None;
        for (i, s) in left.iter().enumerate() {
            if keep_l[i] && best.is_none_or(|b| s.priority < b.0) {
                best = Some((s.priority, true, i));
            }
        }
        for (i, s) in right.iter().enumerate() {
            if keep_r[i] && best.is_none_or(|b| s.priority <= b.0) {
                best = Some((s.priority, false, i));
            }
        }
        match best {
            Some((_, true, i)) => keep_l[i] = false,
            Some((_, false, i)) => keep_r[i] = false,
            None => break,
        }
    }
    let draw = |buf: &mut Buffer, ctx: &mut RenderCtx, x: u16, s: &Segment| {
        let mut st = match s.id {
            Some(id) => {
                let mut vs = ctx.state(id);
                vs.focused = false;
                t.interactive(crate::theme::Kind::Title, vs, t.plane_of(bg))
                    .style
            }
            None => ratatui::style::Style::new().bg(bg),
        };
        st = st.fg(t.legible(t.tone(s.tone), st.bg.unwrap_or(bg)));
        if s.bold {
            st = st.add_modifier(Modifier::BOLD);
        }
        let pad = u16::from(s.id.is_some());
        let text = if s.id.is_some() {
            format!(" {} ", s.shown())
        } else {
            s.shown()
        };
        buf.set_string(x, area.y, &text, st);
        if let Some((g, tone)) = &s.lead {
            let ink = t.legible(t.tone(*tone), st.bg.unwrap_or(bg));
            buf.set_style(
                Rect::new(x + pad, area.y, crate::ui::text::width(g) as u16, 1),
                ratatui::style::Style::new()
                    .fg(ink)
                    .remove_modifier(Modifier::BOLD),
            );
        }
        if let Some(id) = s.id {
            ctx.clickable(
                id,
                Rect::new(x, area.y, crate::ui::text::width(&text) as u16, 1),
            );
        }
    };
    let mut x = area.x + edge;
    for (s, k) in left.iter().zip(&keep_l) {
        if !k {
            continue;
        }
        let sw = terminal_extent(w(s)) + if s.id.is_some() { 2 } else { 0 };
        let start = if s.id.is_some() {
            x.saturating_sub(1)
        } else {
            x
        };
        draw(buf, ctx, start, s);
        x += sw.saturating_sub(if s.id.is_some() { 2 } else { 0 }) + sep;
    }
    let mut rx = area.right().saturating_sub(edge);
    for (s, k) in right.iter().zip(&keep_r).rev() {
        if !k {
            continue;
        }
        let sw = terminal_extent(w(s));
        rx = rx.saturating_sub(sw);
        let start = if s.id.is_some() {
            rx.saturating_sub(1)
        } else {
            rx
        };
        draw(buf, ctx, start, s);
        rx = rx.saturating_sub(sep);
    }

    let hidden: Vec<String> = left
        .iter()
        .zip(&keep_l)
        .chain(right.iter().zip(&keep_r))
        .filter(|(_, k)| !**k)
        .map(|(s, _)| s.shown())
        .collect();
    if !hidden.is_empty() && area.width > 2 * edge {
        let at = if right.is_empty() {
            x.min(area.right().saturating_sub(edge + 1))
        } else {
            rx.saturating_sub(1).max(area.x + edge)
        };
        crate::ui::reveal::overflow(
            ctx,
            buf,
            Rect::new(at, area.y, 1, 1),
            &hidden.join("  "),
            bg,
        );
    }
}

impl crate::ui::specimen::Specimen for Segment {
    fn specimen_id(&self) -> WidgetId {
        self.id.unwrap_or(WidgetId::of("segments.specimen"))
    }

    fn supports(&self, state: crate::ui::specimen::SpecimenState) -> bool {
        use crate::ui::specimen::SpecimenState::*;
        match state {
            Rest | Narrow => true,
            Hover | Pressed => self.id.is_some(),
            _ => false,
        }
    }

    fn specimen(
        &self,
        _state: crate::ui::specimen::SpecimenState,
        area: Rect,
        buf: &mut Buffer,
        ctx: &mut RenderCtx,
        bg: Color,
    ) {
        crate::ui::ctx::fill(buf, area, ratatui::style::Style::new().bg(bg));
        render(area, buf, ctx, std::slice::from_ref(self), &[], bg);
    }
}
