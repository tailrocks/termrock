use ratatui::buffer::Buffer;
use ratatui::layout::Rect;
use ratatui::style::{Modifier, Style};

use crate::core::id::WidgetId;
use crate::theme::{Kind, Plane, Tone};
use crate::ui::ctx::{RenderCtx, VisualState, fill};
use crate::ui::fitting::{removal_order, work};
use crate::ui::measure::terminal_extent;
use crate::ui::text::{truncate, width};
use crate::widgets::progress::{Meter, MeterTone, MeterVisual};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Emphasis {
    #[default]
    Plain,

    Strong,

    Chip,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StatusItem {
    pub text: String,
    pub tone: Tone,

    pub priority: u8,
    pub id: Option<WidgetId>,
    pub emphasis: Emphasis,

    pub meter: Option<(Option<u8>, MeterTone)>,

    pub meter_stale: bool,

    pub busy: bool,

    pub tip: Option<crate::ui::tooltip::Tip>,
}

pub const STATUS_METER_TRACK: u16 = 10;

impl StatusItem {
    pub fn new(text: impl Into<String>, tone: Tone) -> Self {
        Self {
            text: text.into(),
            tone,
            priority: 5,
            id: None,
            emphasis: Emphasis::Plain,
            meter: None,
            meter_stale: false,
            busy: false,
            tip: None,
        }
    }

    pub fn busy(mut self) -> Self {
        self.busy = true;
        self
    }

    pub fn meter(mut self, used_pct: Option<u8>, tone: MeterTone) -> Self {
        self.meter = Some((used_pct, tone));
        self
    }

    pub fn meter_stale(mut self, on: bool) -> Self {
        self.meter_stale = on;
        self
    }
    pub fn priority(mut self, p: u8) -> Self {
        self.priority = p;
        self
    }
    pub fn clickable(mut self, id: WidgetId) -> Self {
        self.id = Some(id);
        self
    }

    pub fn tooltip(mut self, tip: crate::ui::tooltip::Tip) -> Self {
        self.tip = Some(tip);
        self
    }
    pub fn strong(mut self) -> Self {
        self.emphasis = Emphasis::Strong;
        self
    }
    pub fn chip(mut self) -> Self {
        self.emphasis = Emphasis::Chip;
        self
    }

    /// Natural width of the item, saturating at `u16::MAX`.
    pub fn width(&self) -> u16 {
        terminal_extent(self.intrinsic_width())
    }

    pub(crate) fn intrinsic_width(&self) -> usize {
        let w = width(&self.text).saturating_add(if self.busy { 2 } else { 0 });
        let base = match self.emphasis {
            Emphasis::Chip => w.saturating_add(2),
            _ => w,
        };
        if self.meter.is_some() {
            base.saturating_add(1 + usize::from(STATUS_METER_TRACK) + 7)
        } else {
            base
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Group {
    Left,
    Center,
    Right,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Placed {
    pub group: Group,
    pub index: usize,
    pub x: u16,
    pub width: u16,

    pub text: String,
}

#[derive(Debug, Clone, Default)]
pub struct StatusBar {
    pub left: Vec<StatusItem>,
    pub center: Vec<StatusItem>,
    pub right: Vec<StatusItem>,

    pub reveal: Option<crate::ui::reveal::Reveal>,
}

impl crate::ui::measure::Measure for StatusBar {
    fn measure(
        &self,
        _avail: ratatui::layout::Size,
        _ctx: &crate::ui::ctx::RenderCtx,
    ) -> ratatui::layout::Size {
        ratatui::layout::Size::new(terminal_extent(self.intrinsic_width()), 1)
    }

    fn min_width(&self, _avail: ratatui::layout::Size, _ctx: &crate::ui::ctx::RenderCtx) -> u16 {
        FLEX_MIN
    }
}

const FLEX_MIN: u16 = 24;

pub fn reveal_id(group: Group, index: usize) -> WidgetId {
    WidgetId::of("statusbar.reveal").child(group as usize * 1000 + index)
}

const GAP: u16 = 3;
const EDGE: u16 = 1;

/// Running width and count of one group's kept items. Exact in `u128`, so
/// removing an item can never turn a saturated total falsely small.
#[derive(Clone, Copy, Default)]
struct Tally {
    sum: u128,
    count: usize,
}

impl Tally {
    fn add(&mut self, width: usize) {
        self.sum += width as u128;
        self.count += 1;
    }

    fn remove(&mut self, width: usize) {
        self.sum -= width as u128;
        self.count -= 1;
    }

    /// Item widths plus the gaps between them; an empty group takes no room.
    fn cells(self) -> u128 {
        match self.count {
            0 => 0,
            n => self.sum + (n as u128 - 1) * u128::from(GAP),
        }
    }
}

/// Full width of the kept items, group gaps, the optional More marker and the
/// edges, before any fit against an allocation. A gap separates groups that take
/// a positive width, so a lone empty label opens none.
fn needed(groups: &[Tally; 3], gone: bool) -> u128 {
    let cells = groups.map(Tally::cells);
    let present = cells.iter().filter(|&&c| c > 0).count();
    let gaps = present.saturating_sub(1) as u128 * u128::from(GAP);
    let more = if gone { 1 + u128::from(GAP) } else { 0 };
    cells.iter().sum::<u128>() + gaps + more + 2 * u128::from(EDGE)
}

impl StatusBar {
    pub fn new() -> Self {
        Self::default()
    }

    fn group(&self, g: Group) -> &[StatusItem] {
        match g {
            Group::Left => &self.left,
            Group::Center => &self.center,
            Group::Right => &self.right,
        }
    }

    /// Display width of every item, per group, measured once per layout.
    pub(crate) fn item_widths(&self) -> [Vec<usize>; 3] {
        [&self.left, &self.center, &self.right]
            .map(|items| items.iter().map(StatusItem::intrinsic_width).collect())
    }

    pub(crate) fn intrinsic_width(&self) -> usize {
        let tally = self.item_widths().map(|ws| {
            let mut t = Tally::default();
            ws.iter().for_each(|&w| t.add(w));
            t
        });
        usize::try_from(needed(&tally, false)).unwrap_or(usize::MAX)
    }

    pub fn layout(&self, area: Rect) -> Vec<Placed> {
        self.plan(area).0
    }

    /// Which items survive in `avail` cells. Items go in ascending priority; a
    /// tie removes Center before Right before Left, and the highest index first
    /// inside a group. The last Left item is never removed. One sort fixes that
    /// order and per-group running totals replace a rebuild of the survivors per
    /// removal.
    pub(crate) fn keep_masks(&self, widths: &[Vec<usize>; 3], avail: usize) -> [Vec<bool>; 3] {
        let mut keep = widths.clone().map(|ws| vec![true; ws.len()]);
        let mut tally = widths.clone().map(|ws| {
            let mut t = Tally::default();
            ws.iter().for_each(|&w| {
                work::visit();
                t.add(w);
            });
            t
        });
        let avail = avail as u128;
        if needed(&tally, false) <= avail {
            return keep;
        }
        let groups = [&self.left, &self.center, &self.right];
        let rank = [2u8, 0, 1];
        let mut victims: Vec<(usize, usize)> = Vec::new();
        let mut keys: Vec<(u8, u8, usize)> = Vec::new();
        for (g, items) in groups.iter().enumerate() {
            for (i, it) in items.iter().enumerate() {
                victims.push((g, i));
                keys.push((it.priority, rank[g], usize::MAX - i));
            }
        }
        for v in removal_order(&keys) {
            work::visit();
            let (g, i) = victims[v];
            if g == 0 && tally[0].count == 1 {
                continue;
            }
            keep[g][i] = false;
            tally[g].remove(widths[g][i]);
            if needed(&tally, true) <= avail {
                break;
            }
        }
        keep
    }

    fn plan(&self, area: Rect) -> (Vec<Placed>, Vec<(Group, usize)>, Option<u16>) {
        let total_w = area.width;
        let widths = self.item_widths();
        let keep = self.keep_masks(&widths, usize::from(total_w));
        let groups = [Group::Left, Group::Center, Group::Right];
        let mut placed = vec![];

        let mut x = area.x + EDGE;
        let left_budget = total_w.saturating_sub(2 * EDGE);
        for (i, it) in self.left.iter().enumerate() {
            if !keep[0][i] {
                continue;
            }
            let mut w = terminal_extent(widths[0][i]);
            let mut text = it.text.clone();
            let room = (area.x + EDGE + left_budget).saturating_sub(x);
            if widths[0][i] > usize::from(room) {
                let pad =
                    if it.emphasis == Emphasis::Chip { 2 } else { 0 } + if it.busy { 2 } else { 0 };
                text = truncate(&it.text, room.saturating_sub(pad) as usize);
                w = terminal_extent(width(&text) + usize::from(pad));
            }
            placed.push(Placed {
                group: Group::Left,
                index: i,
                x,
                width: w,
                text,
            });
            x = x.saturating_add(w).saturating_add(GAP);
        }
        let left_end = x.saturating_sub(GAP);

        let mut rx = area.right().saturating_sub(EDGE);
        let mut right_items: Vec<Placed> = vec![];
        for (i, it) in self.right.iter().enumerate().rev() {
            if !keep[2][i] {
                continue;
            }
            let w = terminal_extent(widths[2][i]);
            rx = rx.saturating_sub(w);
            right_items.push(Placed {
                group: Group::Right,
                index: i,
                x: rx,
                width: w,
                text: it.text.clone(),
            });
            rx = rx.saturating_sub(GAP);
        }
        let mut right_start = if right_items.is_empty() {
            area.right().saturating_sub(EDGE)
        } else {
            rx + GAP
        };

        let hidden: Vec<(Group, usize)> = groups
            .iter()
            .enumerate()
            .flat_map(|(g, grp)| {
                keep[g]
                    .iter()
                    .enumerate()
                    .filter(|(_, k)| !**k)
                    .map(move |(i, _)| (*grp, i))
            })
            .collect();
        let more_x = (!hidden.is_empty() && total_w > 2 * EDGE).then(|| {
            let at = if right_items.is_empty() {
                right_start.saturating_sub(1)
            } else {
                right_start.saturating_sub(GAP + 1)
            };
            right_start = at;
            at.max(left_end + u16::from(!placed.is_empty()))
                .min(area.right().saturating_sub(EDGE + 1))
        });

        let mut center = Tally::default();
        for (i, &w) in widths[1].iter().enumerate() {
            if keep[1][i] {
                center.add(w);
            }
        }
        let cw = terminal_extent(usize::try_from(center.cells()).unwrap_or(usize::MAX));
        if cw > 0 {
            let lo = left_end + GAP;
            let hi = right_start.saturating_sub(GAP);
            let free = hi.saturating_sub(lo);
            let mut cx = lo + free.saturating_sub(cw) / 2;
            for (i, it) in self.center.iter().enumerate() {
                if !keep[1][i] {
                    continue;
                }
                let w = terminal_extent(widths[1][i]);
                placed.push(Placed {
                    group: Group::Center,
                    index: i,
                    x: cx,
                    width: w,
                    text: it.text.clone(),
                });
                cx = cx.saturating_add(w).saturating_add(GAP);
            }
        }
        right_items.reverse();
        placed.extend(right_items);
        (placed, hidden, more_x)
    }

    pub fn render(&self, area: Rect, buf: &mut Buffer, ctx: &mut RenderCtx) {
        let area = area.intersection(*buf.area());
        if area.is_empty() {
            return;
        }
        let t = ctx.theme;
        let bg = t.surface_elevated;
        fill(buf, area, Style::new().bg(bg));
        let (placed, hidden, more_x) = self.plan(area);
        if let Some(x) = more_x {
            let text: Vec<&str> = hidden
                .iter()
                .map(|(g, i)| self.group(*g)[*i].text.as_str())
                .collect();
            crate::ui::reveal::overflow(
                ctx,
                buf,
                Rect::new(x, area.y, 1, 1),
                &text.join("   "),
                bg,
            );
        }
        for p in placed {
            let it = &self.group(p.group)[p.index];

            let kind = match it.emphasis {
                Emphasis::Chip => Some(Kind::Chip),
                _ => it.id.map(|_| Kind::Title),
            };
            let mut st = match (kind, it.id) {
                (Some(kind), Some(id)) => {
                    let mut s = ctx.state(id);
                    s.focused = false;
                    t.interactive(kind, s, Plane::Card).style
                }
                (Some(kind), None) => {
                    t.interactive(kind, VisualState::default(), Plane::Card)
                        .style
                }
                _ => Style::new().bg(bg),
            };
            st = st.fg(t.legible(t.tone(it.tone), st.bg.unwrap_or(bg)));
            if it.emphasis == Emphasis::Strong {
                st = st.add_modifier(Modifier::BOLD);
            }
            let text = if it.emphasis == Emphasis::Chip {
                format!(" {} ", p.text)
            } else {
                p.text.clone()
            };
            if it.busy {
                let (lead, rest) = match it.emphasis {
                    Emphasis::Chip => (" ", p.text.as_str()),
                    _ => ("", p.text.as_str()),
                };
                let spin = t.glyphs.spinner_frame(ctx.interaction.tick);
                let mut x = p.x;
                if !lead.is_empty() {
                    buf.set_string(x, area.y, lead, st);
                    x += 1;
                }

                let ink = t.legible_mark(t.accent, st.bg.unwrap_or(bg));
                buf.set_string(x, area.y, spin, st.fg(ink));
                buf.set_string(x + 2, area.y, rest, st);
                if it.emphasis == Emphasis::Chip {
                    buf.set_string(x + 2 + width(rest) as u16, area.y, " ", st);
                }
            } else {
                buf.set_string(p.x, area.y, &text, st);
            }
            if let Some((pct, tone)) = it.meter {
                let label_w = width(&text) as u16;
                let mx = p.x + label_w + 1;
                let mw = p.width.saturating_sub(label_w + 1);
                let value = pct
                    .map(|v| format!("{v}%"))
                    .unwrap_or_else(|| t.glyphs.none.into());
                Meter::new(pct)
                    .value(value)
                    .tone(tone)
                    .stale(it.meter_stale)
                    .visual(MeterVisual::Line)
                    .render(Rect::new(mx, area.y, mw, 1), buf, ctx, st.bg.unwrap_or(bg));
            }
            if let Some(id) = it.id {
                ctx.clickable(id, Rect::new(p.x, area.y, p.width, 1));
            }
            if let Some(tip) = &it.tip {
                let seg = Rect::new(p.x, area.y, p.width, 1);
                let id = it.id.unwrap_or_else(|| reveal_id(p.group, p.index));
                if it.id.is_none() && !ctx.inert {
                    ctx.hits.register_hover(id, seg);
                }
                crate::ui::tooltip::show(ctx, id, seg, tip, None);
            }
            if p.text != it.text {
                let lead = u16::from(it.emphasis == Emphasis::Chip) + if it.busy { 2 } else { 0 };
                let cut = Rect::new(p.x + lead, area.y, width(&p.text) as u16, 1);
                let id = it.id.unwrap_or_else(|| reveal_id(p.group, p.index));
                let cfg = self.reveal.unwrap_or(ctx.reveal);
                if it.id.is_none() && cfg.hover {
                    ctx.clickable(id, cut);
                }
                crate::ui::reveal::truncated(ctx, Some(cfg), id, cut, &it.text);
            }
        }
    }
}

impl crate::ui::specimen::Specimen for StatusItem {
    fn specimen_id(&self) -> WidgetId {
        self.id.unwrap_or(WidgetId::of("statusbar.specimen"))
    }

    fn supports(&self, state: crate::ui::specimen::SpecimenState) -> bool {
        use crate::ui::specimen::SpecimenState::*;
        match state {
            Rest | Busy | Narrow => true,
            Hover | Pressed => self.id.is_some(),
            _ => false,
        }
    }

    fn specimen(
        &self,
        state: crate::ui::specimen::SpecimenState,
        area: Rect,
        buf: &mut Buffer,
        ctx: &mut RenderCtx,
        _bg: ratatui::style::Color,
    ) {
        let mut item = self.clone();
        item.tip = None;
        item.busy |= state == crate::ui::specimen::SpecimenState::Busy;
        let mut bar = StatusBar::new();
        bar.left.push(item);
        bar.render(area, buf, ctx);
    }
}
