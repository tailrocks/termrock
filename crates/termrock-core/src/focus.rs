use super::id::WidgetId;

#[derive(Debug, Default, Clone)]
pub struct FocusRing {
    order: Vec<WidgetId>,
    barrier: Option<usize>,

    selections: Vec<WidgetId>,
}

impl FocusRing {
    pub fn register(&mut self, id: WidgetId) {
        self.order.push(id);
    }

    pub fn mark_selection(&mut self, id: WidgetId) {
        if !self.selections.contains(&id) {
            self.selections.push(id);
        }
    }

    pub fn has_selection(&self, id: WidgetId) -> bool {
        self.selections.contains(&id)
    }

    pub fn len(&self) -> usize {
        self.order.len()
    }
    pub fn is_empty(&self) -> bool {
        self.order.is_empty()
    }

    pub fn sort_from<K: Ord>(&mut self, start: usize, key: impl FnMut(&WidgetId) -> K) {
        if let Some(group) = self.order.get_mut(start..) {
            group.sort_by_key(key);
        }
    }
    pub fn push_barrier(&mut self) {
        self.barrier = Some(self.order.len());
    }

    pub fn reachable(&self) -> &[WidgetId] {
        match self.barrier {
            Some(start) => &self.order[start..],
            None => &self.order,
        }
    }

    pub fn contains(&self, id: WidgetId) -> bool {
        self.reachable().contains(&id)
    }

    pub fn first(&self) -> Option<WidgetId> {
        self.reachable().first().copied()
    }

    pub fn next(&self, current: Option<WidgetId>) -> Option<WidgetId> {
        let ring = self.reachable();
        if ring.is_empty() {
            return None;
        }
        match current.and_then(|c| ring.iter().position(|&id| id == c)) {
            Some(i) => Some(ring[(i + 1) % ring.len()]),
            None => ring.first().copied(),
        }
    }

    pub fn prev(&self, current: Option<WidgetId>) -> Option<WidgetId> {
        let ring = self.reachable();
        if ring.is_empty() {
            return None;
        }
        match current.and_then(|c| ring.iter().position(|&id| id == c)) {
            Some(0) | None => ring.last().copied(),
            Some(i) => Some(ring[i - 1]),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum FocusFallback {
    #[default]
    Nearest,

    First,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum BlurAction {
    #[default]
    Commit,

    Cancel,

    Keep,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct FocusChange {
    pub from: WidgetId,

    pub to: Option<WidgetId>,
}

pub trait BlurTarget {
    fn blur_id(&self) -> WidgetId;

    fn blur(&mut self) -> super::event::Outcome;
}

pub fn forward_blur(
    change: FocusChange,
    targets: &mut [&mut dyn BlurTarget],
) -> super::event::Outcome {
    targets
        .iter_mut()
        .find(|t| t.blur_id() == change.from)
        .map_or(super::event::Outcome::Ignored, |t| t.blur())
}

thread_local! {

    static REQUEST: std::cell::Cell<Option<bool>> = const { std::cell::Cell::new(None) };
}

pub fn request_focus_next(backward: bool) {
    REQUEST.with(|r| r.set(Some(backward)));
}

pub fn cancel_focus_request() {
    REQUEST.with(|r| r.set(None));
}

#[derive(Debug, Default, Clone)]
pub struct Focus {
    current: Option<WidgetId>,

    last_index: Option<usize>,

    last_valid: Option<WidgetId>,

    left: Vec<WidgetId>,

    end_edit: Option<WidgetId>,

    moves: u64,
}

impl Focus {
    pub fn current(&self) -> Option<WidgetId> {
        self.current
    }

    pub fn is(&self, id: WidgetId) -> bool {
        self.current == Some(id)
    }

    pub fn set(&mut self, id: Option<WidgetId>) {
        self.move_to(id);
    }

    pub fn focus(&mut self, id: WidgetId) {
        self.move_to(Some(id));
    }

    pub fn next(&mut self, ring: &FocusRing) {
        self.move_to(ring.next(self.current));
    }

    pub fn prev(&mut self, ring: &FocusRing) {
        self.move_to(ring.prev(self.current));
    }

    pub fn moves(&self) -> u64 {
        self.moves
    }

    fn move_to(&mut self, id: Option<WidgetId>) {
        self.moves = self.moves.wrapping_add(1);
        if self.current != id {
            cancel_focus_request();
        }
        if let Some(from) = self.current
            && self.current != id
            && !self.left.contains(&from)
        {
            self.left.push(from);
        }
        self.current = id;
    }

    pub fn apply_request(&mut self, ring: &FocusRing) -> bool {
        let Some(backward) = REQUEST.with(|r| r.take()) else {
            return false;
        };
        let before = self.current;
        if backward {
            self.prev(ring);
        } else {
            self.next(ring);
        }
        self.current != before
    }

    pub fn settle(&mut self, ring: &FocusRing) -> Vec<FocusChange> {
        self.apply_request(ring);
        self.take_changes()
    }

    pub fn end_edit(&mut self) {
        self.end_edit = self.current;
    }

    pub fn take_changes(&mut self) -> Vec<FocusChange> {
        let to = self.current;
        let mut out: Vec<FocusChange> = std::mem::take(&mut self.left)
            .into_iter()
            .filter(|&from| Some(from) != to)
            .map(|from| FocusChange { from, to })
            .collect();
        if let Some(from) = self.end_edit.take()
            && Some(from) == to
        {
            out.push(FocusChange { from, to });
        }
        out
    }

    pub fn ensure_valid(&mut self, ring: &FocusRing) {
        self.ensure_valid_with(ring, FocusFallback::First);
    }

    pub fn ensure_valid_with(&mut self, ring: &FocusRing, fallback: FocusFallback) {
        let reach = ring.reachable();
        if let Some(i) = self
            .current
            .and_then(|c| reach.iter().position(|&id| id == c))
        {
            self.last_index = Some(i);
            self.last_valid = self.current;
            return;
        }
        let stayed = self.last_valid.filter(|id| reach.contains(id));
        let next = match (fallback, self.last_index) {
            (FocusFallback::Nearest, _) if stayed.is_some() => stayed,
            (FocusFallback::Nearest, Some(i)) if !reach.is_empty() => {
                Some(reach[i.saturating_sub(1).min(reach.len() - 1)])
            }
            _ => ring.first(),
        };
        self.move_to(next);
        self.last_index = self
            .current
            .and_then(|c| reach.iter().position(|&id| id == c));
        self.last_valid = self.last_index.and(self.current);
    }
}
