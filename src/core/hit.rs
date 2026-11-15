use ratatui::layout::{Position, Rect};

use super::id::{ItemKey, Part, PartRef, WidgetId};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct HitRegion {
    pub id: WidgetId,

    pub owner: WidgetId,

    pub part: PartRef,
    pub area: Rect,

    pub scroll_only: bool,

    pub pass_through: bool,

    pub hover_only: bool,

    pub backdrop: bool,

    pub surface: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Axis {
    Vertical,
    Horizontal,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Bar {
    pub id: WidgetId,
    pub owner: WidgetId,
    pub axis: Axis,
    pub track: Rect,
}

#[derive(Debug, Default, Clone)]
pub struct HitRegistry {
    regions: Vec<HitRegion>,

    barriers: Vec<usize>,

    bars: Vec<Bar>,

    logical_bars: Vec<Bar>,

    shapes: Vec<(WidgetId, super::shape::PointerShape)>,

    autoscroll: Vec<(usize, Rect)>,

    releases: Vec<(WidgetId, ReleaseFrom)>,

    shifts: Vec<(WidgetId, (u16, u16))>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct HitMark {
    regions: usize,
    bars: usize,
    autoscroll: usize,
}

impl HitMark {
    pub(crate) fn regions_len(&self) -> usize {
        self.regions
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ReleaseFrom {
    Press(WidgetId),

    Secondary,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Hit {
    pub owner: WidgetId,
    pub part: PartRef,
}

impl HitRegion {
    fn plain(id: WidgetId, area: Rect) -> Self {
        Self {
            id,
            owner: id,
            part: PartRef::of(Part::CONTAINER),
            area,
            scroll_only: false,
            pass_through: false,
            hover_only: false,
            backdrop: false,
            surface: false,
        }
    }

    pub fn hit(&self) -> Hit {
        Hit {
            owner: self.owner,
            part: self.part,
        }
    }

    fn clickable(&self) -> bool {
        !self.scroll_only && !self.hover_only
    }
}

impl HitRegistry {
    pub fn register(&mut self, id: WidgetId, area: Rect) {
        if area.is_empty() {
            return;
        }
        self.regions.push(HitRegion::plain(id, area));
    }

    pub fn register_part(&mut self, owner: WidgetId, part: PartRef, area: Rect) -> WidgetId {
        let id = owner.part(part);
        self.register_part_as(id, owner, part, area);
        id
    }

    pub fn register_part_as(&mut self, id: WidgetId, owner: WidgetId, part: PartRef, area: Rect) {
        if area.is_empty() {
            return;
        }
        self.regions.push(HitRegion {
            owner,
            part,
            ..HitRegion::plain(id, area)
        });
    }

    pub fn resolve(&self, pos: Position) -> Option<Hit> {
        self.reachable()
            .iter()
            .rev()
            .find(|r| r.clickable() && r.area.contains(pos))
            .map(HitRegion::hit)
    }

    pub fn part_of(&self, id: WidgetId) -> Option<Hit> {
        self.regions
            .iter()
            .rev()
            .find(|r| r.id == id)
            .map(HitRegion::hit)
    }

    pub fn raise(&mut self, id: WidgetId) {
        if let Some(bar) = self.bar(id) {
            let parts: Vec<HitRegion> = self
                .regions
                .iter()
                .filter(|r| r.id == id && bar.track.contains(r.area.as_position()))
                .copied()
                .collect();
            let from = parts.iter().rposition(|r| r.area == bar.track).unwrap_or(0);
            self.regions.extend_from_slice(&parts[from..]);
            return;
        }
        if let Some(r) = self.regions.iter().rev().find(|r| r.id == id).copied() {
            self.regions.push(r);
        }
    }

    pub fn regions(&self) -> &[HitRegion] {
        &self.regions
    }

    pub fn register_under(&mut self, id: WidgetId, area: Rect) {
        if area.is_empty() {
            return;
        }
        let at = self.barriers.last().copied().unwrap_or(0);
        self.regions.insert(at, HitRegion::plain(id, area));
    }

    pub fn register_scroll_under(&mut self, id: WidgetId, area: Rect) {
        if area.is_empty() {
            return;
        }
        let at = self.barriers.last().copied().unwrap_or(0);
        self.regions.insert(
            at,
            HitRegion {
                scroll_only: true,
                ..HitRegion::plain(id, area)
            },
        );
    }

    pub fn register_surface(&mut self, id: WidgetId, area: Rect) {
        if area.is_empty() {
            return;
        }
        self.regions.push(HitRegion {
            surface: true,
            ..HitRegion::plain(id, area)
        });
    }

    pub fn is_surface(&self, id: WidgetId) -> bool {
        self.regions.iter().rev().any(|r| r.id == id && r.surface)
    }

    pub fn set_shape(&mut self, id: WidgetId, shape: super::shape::PointerShape) {
        self.shapes.retain(|(i, _)| *i != id);
        self.shapes.push((id, shape));
    }

    pub fn shape_of(&self, id: WidgetId) -> Option<super::shape::PointerShape> {
        self.shapes.iter().find(|(i, _)| *i == id).map(|(_, s)| *s)
    }

    pub fn register_bar(&mut self, bar: Bar) {
        if bar.track.is_empty() {
            return;
        }
        self.register_part_as(bar.id, bar.owner, PartRef::of(Part::TRACK), bar.track);
        self.bars.push(bar);
        self.logical_bars.push(bar);
    }

    pub fn register_thumb(&mut self, bar_id: WidgetId, owner: WidgetId, thumb: Rect) {
        if self.bar(bar_id).is_none() {
            return;
        }
        self.register_part_as(bar_id, owner, PartRef::of(Part::THUMB), thumb);
    }

    pub fn accept_release(&mut self, id: WidgetId, from: ReleaseFrom) {
        self.releases.push((id, from));
    }

    pub fn accepts_release(&self, id: WidgetId, from: ReleaseFrom) -> bool {
        self.releases.contains(&(id, from))
    }

    pub fn bars(&self) -> &[Bar] {
        &self.bars
    }

    pub fn scrollers(&self) -> Vec<(WidgetId, Rect)> {
        self.regions
            .iter()
            .filter(|r| r.scroll_only)
            .map(|r| (r.id, r.area))
            .collect()
    }

    pub fn bar(&self, id: WidgetId) -> Option<Bar> {
        self.bars.iter().rev().find(|b| b.id == id).copied()
    }

    pub(crate) fn logical_bar(&self, id: WidgetId) -> Option<Bar> {
        self.logical_bars.iter().rev().find(|b| b.id == id).copied()
    }

    pub fn register_backdrop(&mut self, id: WidgetId, area: Rect, pass_through: bool) {
        self.register_backdrop_of(id, id, area, pass_through);
    }

    pub fn register_backdrop_of(
        &mut self,
        id: WidgetId,
        owner: WidgetId,
        area: Rect,
        pass_through: bool,
    ) {
        if area.is_empty() {
            return;
        }
        self.regions.push(HitRegion {
            pass_through,
            owner,
            part: PartRef::of(Part::BACKDROP),
            backdrop: true,
            ..HitRegion::plain(id, area)
        });
    }

    pub fn register_scroll(&mut self, id: WidgetId, area: Rect) {
        if area.is_empty() {
            return;
        }
        self.regions.push(HitRegion {
            scroll_only: true,
            ..HitRegion::plain(id, area)
        });
    }

    pub fn register_hover(&mut self, id: WidgetId, area: Rect) {
        if area.is_empty() {
            return;
        }
        self.regions.push(HitRegion {
            hover_only: true,
            ..HitRegion::plain(id, area)
        });
    }

    pub fn register_autoscroll(&mut self, area: Rect) {
        if !area.is_empty() {
            self.autoscroll.push((self.barriers.len(), area));
        }
    }

    pub fn autoscroll_at(&self, pos: Position) -> Option<Rect> {
        let layer = self.barriers.len();
        self.autoscroll
            .iter()
            .rev()
            .find(|(at, area)| *at == layer && area.contains(pos))
            .map(|(_, area)| *area)
    }

    pub fn push_barrier(&mut self) {
        self.barriers.push(self.regions.len());
    }

    fn reachable(&self) -> &[HitRegion] {
        match self.barriers.last() {
            Some(&start) => &self.regions[start..],
            None => &self.regions,
        }
    }

    fn beneath(&self) -> &[HitRegion] {
        match self.barriers.as_slice() {
            [] => &[],
            [top] => &self.regions[..*top],
            [.., below, top] => &self.regions[*below..*top],
        }
    }

    pub fn pass_through_target(&self, id: WidgetId, pos: Position) -> Option<WidgetId> {
        let top = self
            .reachable()
            .iter()
            .rev()
            .find(|r| r.clickable() && r.area.contains(pos))?;
        if top.id != id || !top.pass_through {
            return None;
        }
        self.beneath()
            .iter()
            .rev()
            .find(|r| r.clickable() && r.area.contains(pos))
            .map(|r| r.id)
    }

    pub fn backdrop_at(&self, pos: Position) -> Option<(WidgetId, Option<WidgetId>)> {
        let top = self
            .reachable()
            .iter()
            .rev()
            .find(|r| r.clickable() && r.area.contains(pos))?;
        if !top.backdrop {
            return None;
        }
        let mut end = *self.barriers.last()?;
        for &start in self.barriers.iter().rev().skip(1) {
            let layer = &self.regions[start..end];
            if !layer.is_empty() {
                return Some((top.id, Self::top_clickable(layer, pos)));
            }
            end = start;
        }
        Some((top.id, Self::top_clickable(&self.regions[..end], pos)))
    }

    fn top_clickable(layer: &[HitRegion], pos: Position) -> Option<WidgetId> {
        layer
            .iter()
            .rev()
            .find(|r| r.clickable() && !r.backdrop && r.area.contains(pos))
            .map(|r| r.id)
    }

    pub fn hit(&self, pos: Position) -> Option<WidgetId> {
        self.reachable()
            .iter()
            .rev()
            .find(|r| r.clickable() && r.area.contains(pos))
            .map(|r| r.id)
    }

    pub fn hover_at(&self, pos: Position) -> Option<WidgetId> {
        self.reachable()
            .iter()
            .rev()
            .find(|r| !r.scroll_only && r.area.contains(pos))
            .map(|r| r.id)
    }

    pub fn hit_scroll(&self, pos: Position) -> Option<WidgetId> {
        self.reachable()
            .iter()
            .rev()
            .find(|r| !r.hover_only && r.area.contains(pos))
            .map(|r| r.id)
    }

    pub fn scroll_stack(&self, pos: Position) -> Vec<WidgetId> {
        let mut out: Vec<WidgetId> = Vec::new();
        for r in self.reachable().iter().rev() {
            if !r.hover_only && r.area.contains(pos) && !out.contains(&r.id) {
                out.push(r.id);
            }
        }
        out
    }

    pub fn mark(&self) -> HitMark {
        HitMark {
            regions: self.regions.len(),
            bars: self.bars.len(),
            autoscroll: self.autoscroll.len(),
        }
    }

    pub fn scroll_since(&mut self, mark: HitMark, content: Rect, window: Rect, to: Position) {
        let dx = window.x.saturating_sub(to.x);
        let dy = window.y.saturating_sub(to.y);
        let place = |r: Rect| -> Rect {
            let r = r.intersection(window);
            if r.is_empty() {
                return Rect::ZERO;
            }
            Rect::new(r.x - dx, r.y - dy, r.width, r.height)
        };
        let from = mark.regions.min(self.regions.len());
        let mut moved: Vec<WidgetId> = Vec::new();
        let mut kept = Vec::with_capacity(self.regions.len() - from);
        for mut r in self.regions.drain(from..) {
            if r.area.intersects(content) {
                if !moved.contains(&r.id) {
                    moved.push(r.id);
                }
                r.area = place(r.area);
                if r.area.is_empty() {
                    continue;
                }
            }
            kept.push(r);
        }
        self.regions.extend(kept);
        let from = mark.bars.min(self.bars.len());
        let bars: Vec<Bar> = self.bars.drain(from..).collect();
        for mut b in bars {
            if b.track.intersects(content) {
                if !moved.contains(&b.id) {
                    moved.push(b.id);
                }
                b.track = place(b.track);
                if b.track.is_empty() {
                    continue;
                }
            }
            self.bars.push(b);
        }
        let from = mark.autoscroll.min(self.autoscroll.len());
        let areas: Vec<(usize, Rect)> = self.autoscroll.drain(from..).collect();
        for (layer, area) in areas {
            let area = if area.intersects(content) {
                place(area)
            } else {
                area
            };
            if !area.is_empty() {
                self.autoscroll.push((layer, area));
            }
        }
        if dx == 0 && dy == 0 {
            return;
        }
        for id in moved {
            match self.shifts.iter_mut().find(|(i, _)| *i == id) {
                Some((_, (x, y))) => {
                    *x = x.saturating_add(dx);
                    *y = y.saturating_add(dy);
                }
                None => self.shifts.push((id, (dx, dy))),
            }
        }
    }

    pub(crate) fn clip_mark(&mut self, mark: HitMark, viewport: Rect) {
        self.clip_since(mark.regions, viewport);
        let from = mark.bars.min(self.bars.len());
        let bars: Vec<Bar> = self.bars.drain(from..).collect();
        for mut bar in bars {
            bar.track = bar.track.intersection(viewport);
            if !bar.track.is_empty() {
                self.bars.push(bar);
            }
        }
        let from = mark.autoscroll.min(self.autoscroll.len());
        let areas: Vec<(usize, Rect)> = self.autoscroll.drain(from..).collect();
        for (layer, area) in areas {
            let area = area.intersection(viewport);
            if !area.is_empty() {
                self.autoscroll.push((layer, area));
            }
        }
    }

    pub fn shift_of(&self, id: WidgetId) -> (u16, u16) {
        self.shifts
            .iter()
            .find(|(i, _)| *i == id)
            .map_or((0, 0), |&(_, s)| s)
    }

    pub fn local(&self, id: WidgetId, pos: Position) -> Position {
        let (dx, dy) = self.shift_of(id);
        Position::new(pos.x.saturating_add(dx), pos.y.saturating_add(dy))
    }

    pub fn len(&self) -> usize {
        self.regions.len()
    }

    pub fn is_empty(&self) -> bool {
        self.regions.is_empty()
    }

    pub fn clip_since(&mut self, from: usize, area: Rect) {
        let from = from.min(self.regions.len());
        let mut kept = Vec::with_capacity(self.regions.len() - from);
        for mut r in self.regions.drain(from..) {
            r.area = r.area.intersection(area);
            if !r.area.is_empty() {
                kept.push(r);
            }
        }
        self.regions.extend(kept);
    }

    pub fn area_of(&self, id: WidgetId) -> Option<Rect> {
        if let Some(bar) = self.bar(id) {
            return Some(bar.track);
        }
        self.regions
            .iter()
            .rev()
            .find(|r| r.id == id)
            .map(|r| r.area)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct PartEntry {
    id: WidgetId,
    part: PartRef,

    col: Option<usize>,
}

#[derive(Debug, Default, Clone, PartialEq, Eq)]
pub(crate) struct PartMap {
    entries: Vec<PartEntry>,
}

impl PartMap {
    pub fn clear(&mut self) {
        self.entries.clear();
    }

    pub fn record(&mut self, id: WidgetId, part: PartRef) {
        self.entries.push(PartEntry {
            id,
            part,
            col: None,
        });
    }

    pub fn record_cell(&mut self, id: WidgetId, part: PartRef, col: usize) {
        self.entries.push(PartEntry {
            id,
            part,
            col: Some(col),
        });
    }

    fn entry(&self, id: WidgetId) -> Option<&PartEntry> {
        self.entries.iter().rev().find(|e| e.id == id)
    }

    pub fn index(&self, id: WidgetId, part: Part) -> Option<usize> {
        self.entry(id)
            .filter(|e| e.part.part == part)
            .and_then(|e| e.part.item)
            .and_then(ItemKey::as_index)
    }

    pub fn cell(&self, id: WidgetId, part: Part) -> Option<(usize, usize)> {
        let e = self.entry(id).filter(|e| e.part.part == part)?;
        Some((e.part.item?.as_index()?, e.col?))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const OWNER: WidgetId = WidgetId::of("hit.tests.owner");
    const FIRST: WidgetId = WidgetId::of("hit.tests.first");
    const SECOND: WidgetId = WidgetId::of("hit.tests.second");

    fn vbar(id: WidgetId, track: Rect) -> Bar {
        Bar {
            id,
            owner: OWNER,
            axis: Axis::Vertical,
            track,
        }
    }

    fn scroll(hits: &mut HitRegistry, mark: HitMark, window: Rect, to: Position) {
        hits.scroll_since(mark, Rect::new(0, 0, 20, 60), window, to);
    }

    #[test]
    fn nested_scrollbar_zero_shift_keeps_both_records_equal() {
        let mut hits = HitRegistry::default();
        let mark = hits.mark();
        let bar = vbar(FIRST, Rect::new(5, 2, 1, 8));
        hits.register_bar(bar);
        scroll(
            &mut hits,
            mark,
            Rect::new(0, 0, 20, 20),
            Position::new(0, 0),
        );
        assert_eq!(hits.bar(FIRST), Some(bar));
        assert_eq!(hits.logical_bar(FIRST), Some(bar));
        assert_eq!(hits.shift_of(FIRST), (0, 0));
    }

    #[test]
    fn nested_scrollbar_clip_to_empty_keeps_logical_record_only() {
        let mut hits = HitRegistry::default();
        let mark = hits.mark();
        let bar = vbar(FIRST, Rect::new(5, 20, 1, 8));
        hits.register_bar(bar);
        scroll(
            &mut hits,
            mark,
            Rect::new(0, 0, 20, 10),
            Position::new(0, 0),
        );
        assert_eq!(hits.bar(FIRST), None, "no visible bar to hit");
        assert_eq!(hits.hit(Position::new(5, 5)), None);
        assert_eq!(hits.logical_bar(FIRST), Some(bar));
    }

    #[test]
    fn nested_scrollbar_thumb_and_track_shift_once() {
        let mut hits = HitRegistry::default();
        let mark = hits.mark();
        let bar = vbar(FIRST, Rect::new(5, 20, 1, 8));
        hits.register_bar(bar);
        hits.register_thumb(FIRST, OWNER, Rect::new(5, 20, 1, 2));
        hits.register_thumb(FIRST, OWNER, Rect::new(5, 20, 1, 2));
        scroll(
            &mut hits,
            mark,
            Rect::new(0, 15, 20, 10),
            Position::new(0, 0),
        );
        assert_eq!(hits.shift_of(FIRST), (0, 15), "one shift per scroll pass");
        assert_eq!(
            hits.bar(FIRST).map(|b| b.track),
            Some(Rect::new(5, 5, 1, 5))
        );
        assert_eq!(hits.logical_bar(FIRST), Some(bar));
        assert_eq!(
            hits.local(FIRST, Position::new(5, 7)),
            Position::new(5, 22),
            "screen pointer maps into the logical track's space"
        );
    }

    #[test]
    fn nested_scrollbar_pruned_sibling_keeps_pairing() {
        let mut hits = HitRegistry::default();
        let mark = hits.mark();
        let gone = vbar(FIRST, Rect::new(5, 40, 1, 8));
        let kept = vbar(SECOND, Rect::new(8, 20, 1, 8));
        hits.register_bar(gone);
        hits.register_bar(kept);
        scroll(
            &mut hits,
            mark,
            Rect::new(0, 15, 20, 10),
            Position::new(0, 0),
        );
        assert_eq!(hits.bar(FIRST), None);
        assert_eq!(
            hits.bar(SECOND).map(|b| b.track),
            Some(Rect::new(8, 5, 1, 5))
        );
        assert_eq!(hits.logical_bar(SECOND), Some(kept));
        assert_eq!(hits.logical_bar(FIRST), Some(gone));
        assert_eq!(hits.shift_of(SECOND), (0, 15));
    }

    #[test]
    fn nested_scrollbar_raise_does_not_register_a_second_logical_bar() {
        let mut hits = HitRegistry::default();
        let bar = vbar(FIRST, Rect::new(5, 2, 1, 8));
        hits.register_bar(bar);
        hits.register_thumb(FIRST, OWNER, Rect::new(5, 2, 1, 2));
        hits.raise(FIRST);
        assert_eq!(hits.logical_bars.len(), 1);
        assert_eq!(hits.logical_bar(FIRST), Some(bar));
    }

    #[test]
    fn clip_mark_clips_screen_geometry_and_keeps_logical_bars() {
        let mut hits = HitRegistry::default();
        hits.register(SECOND, Rect::new(0, 0, 40, 40));
        let mark = hits.mark();
        let bar = vbar(FIRST, Rect::new(5, 6, 1, 30));
        hits.register_bar(bar);
        hits.register_autoscroll(Rect::new(0, 0, 40, 40));
        hits.register(OWNER, Rect::new(2, 2, 3, 30));
        hits.register(OWNER, Rect::new(30, 30, 3, 3));
        hits.clip_mark(mark, Rect::new(0, 0, 20, 10));

        assert_eq!(hits.regions()[0].area, Rect::new(0, 0, 40, 40), "before the mark");
        let kept: Vec<Rect> = hits.regions()[1..].iter().map(|r| r.area).collect();
        assert_eq!(
            kept,
            vec![Rect::new(5, 6, 1, 4), Rect::new(2, 2, 3, 8)],
            "regions are clipped and fully hidden ones dropped"
        );
        assert_eq!(hits.bar(FIRST).map(|b| b.track), Some(Rect::new(5, 6, 1, 4)));
        assert_eq!(hits.logical_bar(FIRST), Some(bar), "the logical bar keeps its full track");
        assert_eq!(hits.shift_of(FIRST), (0, 0));
        assert_eq!(hits.autoscroll_at(Position::new(1, 1)), Some(Rect::new(0, 0, 20, 10)));
        assert_eq!(hits.autoscroll_at(Position::new(25, 25)), None);
    }

    #[test]
    fn clip_mark_drops_a_bar_with_no_visible_track() {
        let mut hits = HitRegistry::default();
        let mark = hits.mark();
        let bar = vbar(FIRST, Rect::new(5, 20, 1, 8));
        hits.register_bar(bar);
        hits.clip_mark(mark, Rect::new(0, 0, 20, 10));
        assert_eq!(hits.bar(FIRST), None);
        assert_eq!(hits.hit(Position::new(5, 21)), None);
        assert_eq!(hits.logical_bar(FIRST), Some(bar));
    }
}
