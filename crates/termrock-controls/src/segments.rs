use ratatui::buffer::Buffer;
use ratatui::layout::Rect;
use ratatui::style::{Color, Modifier};

use termrock_core::id::WidgetId;
use termrock_runtime::ctx::RenderCtx;
use termrock_runtime::fitting::removal_order;
use termrock_runtime::measure::terminal_extent;
use termrock_theme::Tone;

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
pub fn width(left: &[Segment], right: &[Segment], t: &termrock_theme::Theme) -> u16 {
    terminal_extent(intrinsic_width(left, right, t))
}

fn intrinsic_width(left: &[Segment], right: &[Segment], t: &termrock_theme::Theme) -> usize {
    left.iter()
        .chain(right)
        .map(|s| termrock_text::cells::width(&s.shown()).saturating_add(SEP))
        .fold(0usize, usize::saturating_add)
        .saturating_sub(SEP)
        .saturating_add(2 * usize::from(t.spacing.chrome_edge))
}

/// Narrowest width that still shows the strongest segment, saturating at
/// `u16::MAX`.
pub fn min_width(left: &[Segment], right: &[Segment], t: &termrock_theme::Theme) -> u16 {
    let strongest = left
        .iter()
        .chain(right)
        .max_by_key(|s| s.priority)
        .map_or(0, |s| termrock_text::cells::width(&s.shown()));
    terminal_extent(
        strongest
            .saturating_add(2 + 1)
            .saturating_add(2 * usize::from(t.spacing.chrome_edge)),
    )
}

/// What a layout shows: each segment's display text and width, measured once,
/// and which ones survive. `left` entries come first, then `right`.
#[doc(hidden)]
pub struct Plan {
    pub shown: Vec<String>,
    pub widths: Vec<usize>,
    pub keep: Vec<bool>,
}

/// Drops the weakest segments until the rest fits `avail` cells.
///
/// Removal order is the lowest priority first. On a tie a right segment goes
/// before a left one, the rightmost right segment first and the leftmost left
/// segment first. One sort fixes that order and running totals replace a survivor
/// scan per removal. The totals are exact in `u128`.
#[doc(hidden)]
pub fn plan(left: &[Segment], right: &[Segment], edge: u16, avail: u16) -> Plan {
    let shown: Vec<String> = left.iter().chain(right).map(Segment::shown).collect();
    let widths: Vec<usize> = shown
        .iter()
        .map(|s| termrock_text::cells::width(s))
        .collect();
    let mut keep = vec![true; shown.len()];
    let edges = 2 * u128::from(edge);
    let avail = u128::from(avail);
    let need = |kept: u128, gone: bool| {
        let more = if gone { 1 + SEP as u128 } else { 0 };
        (kept + more).saturating_sub(SEP as u128) + edges
    };
    let mut kept: u128 = widths
        .iter()
        .map(|&w| {
            termrock_metrics::record_work!(FitVisits, 1usize);
            w as u128 + SEP as u128
        })
        .sum();
    if need(kept, false) <= avail {
        return Plan {
            shown,
            widths,
            keep,
        };
    }
    let keys: Vec<(u8, u8, usize)> = left
        .iter()
        .enumerate()
        .map(|(i, s)| (s.priority, 1, i))
        .chain(
            right
                .iter()
                .enumerate()
                .map(|(i, s)| (s.priority, 0, usize::MAX - i)),
        )
        .collect();
    for gone in removal_order(&keys) {
        termrock_metrics::record_work!(FitVisits, 1usize);
        keep[gone] = false;
        kept -= widths[gone] as u128 + SEP as u128;
        if need(kept, true) <= avail {
            break;
        }
    }
    Plan {
        shown,
        widths,
        keep,
    }
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
    let Plan {
        shown,
        widths,
        keep,
    } = plan(left, right, edge, area.width);
    let (keep_l, keep_r) = keep.split_at(left.len());
    let (shown_l, shown_r) = shown.split_at(left.len());
    let (width_l, width_r) = widths.split_at(left.len());
    let draw = |buf: &mut Buffer, ctx: &mut RenderCtx, x: u16, s: &Segment, shown: &str| {
        let mut st = match s.id {
            Some(id) => {
                let mut vs = ctx.state(id);
                vs.focused = false;
                t.interactive(termrock_theme::Kind::Title, vs, t.plane_of(bg))
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
            format!(" {shown} ")
        } else {
            shown.to_owned()
        };
        buf.set_string(x, area.y, &text, st);
        if let Some((g, tone)) = &s.lead {
            let ink = t.legible(t.tone(*tone), st.bg.unwrap_or(bg));
            buf.set_style(
                Rect::new(x + pad, area.y, termrock_text::cells::width(g) as u16, 1),
                ratatui::style::Style::new()
                    .fg(ink)
                    .remove_modifier(Modifier::BOLD),
            );
        }
        if let Some(id) = s.id {
            ctx.clickable(
                id,
                Rect::new(x, area.y, termrock_text::cells::width(&text) as u16, 1),
            );
        }
    };
    let mut x = area.x + edge;
    for (i, s) in left.iter().enumerate() {
        if !keep_l[i] {
            continue;
        }
        let sw = terminal_extent(width_l[i]) + if s.id.is_some() { 2 } else { 0 };
        let start = if s.id.is_some() {
            x.saturating_sub(1)
        } else {
            x
        };
        draw(buf, ctx, start, s, &shown_l[i]);
        x += sw.saturating_sub(if s.id.is_some() { 2 } else { 0 }) + sep;
    }
    let mut rx = area.right().saturating_sub(edge);
    for (i, s) in right.iter().enumerate().rev() {
        if !keep_r[i] {
            continue;
        }
        let sw = terminal_extent(width_r[i]);
        rx = rx.saturating_sub(sw);
        let start = if s.id.is_some() {
            rx.saturating_sub(1)
        } else {
            rx
        };
        draw(buf, ctx, start, s, &shown_r[i]);
        rx = rx.saturating_sub(sep);
    }

    let hidden: Vec<&str> = shown
        .iter()
        .zip(&keep)
        .filter(|(_, k)| !**k)
        .map(|(s, _)| s.as_str())
        .collect();
    if !hidden.is_empty() && area.width > 2 * edge {
        let at = if right.is_empty() {
            x.min(area.right().saturating_sub(edge + 1))
        } else {
            rx.saturating_sub(1).max(area.x + edge)
        };
        termrock_runtime::reveal::overflow(
            ctx,
            buf,
            Rect::new(at, area.y, 1, 1),
            &hidden.join("  "),
            bg,
        );
    }
}

impl termrock_runtime::specimen::Specimen for Segment {
    fn specimen_id(&self) -> WidgetId {
        self.id.unwrap_or(WidgetId::of("segments.specimen"))
    }

    fn supports(&self, state: termrock_runtime::specimen::SpecimenState) -> bool {
        use termrock_runtime::specimen::SpecimenState::*;
        match state {
            Rest | Narrow => true,
            Hover | Pressed => self.id.is_some(),
            _ => false,
        }
    }

    fn specimen(
        &self,
        _state: termrock_runtime::specimen::SpecimenState,
        area: Rect,
        buf: &mut Buffer,
        ctx: &mut RenderCtx,
        bg: Color,
    ) {
        termrock_runtime::ctx::fill(buf, area, ratatui::style::Style::new().bg(bg));
        render(area, buf, ctx, std::slice::from_ref(self), &[], bg);
    }
}
