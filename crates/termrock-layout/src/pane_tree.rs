use ratatui::buffer::Buffer;
use ratatui::crossterm::event::{KeyCode, KeyModifiers};
use ratatui::layout::{Margin, Rect, Size};
use ratatui::style::Color;

use crate::layout::{Maximized, Split, SplitDir, SplitKeys};
use crate::splitter::Splitter;
use termrock_controls::keyhint::{Hint, hint};
use termrock_core::event::{Key, Outcome};
use termrock_core::id::WidgetId;
use termrock_core::intent::Phase;
use termrock_core::keymap::{Actions, KeyBinding, KeyMap, hint_key, lookup};
use termrock_core::pointer::PointerEvent;
use termrock_runtime::ctx::RenderCtx;

pub type PaneId = u64;

/// Cells between two panes of one split.
const SEAM: u16 = 1;

/// The smallest outer rectangle a subtree needs. Sums use `usize` so any
/// `min_size`, including `u16::MAX`, adds up without overflow.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct PaneMinimum {
    width: usize,
    height: usize,
}

impl PaneMinimum {
    fn of(size: (u16, u16)) -> Self {
        Self {
            width: size.0 as usize,
            height: size.1 as usize,
        }
    }

    /// The minimum of a split: sizes add along `dir` plus the seam and the larger one wins across it.
    fn join(dir: SplitDir, a: Self, b: Self) -> Self {
        let seam = SEAM as usize;
        match dir {
            SplitDir::Horizontal => Self {
                width: a.width.saturating_add(seam).saturating_add(b.width),
                height: a.height.max(b.height),
            },
            SplitDir::Vertical => Self {
                width: a.width.max(b.width),
                height: a.height.saturating_add(seam).saturating_add(b.height),
            },
        }
    }

    fn along(self, dir: SplitDir) -> usize {
        match dir {
            SplitDir::Horizontal => self.width,
            SplitDir::Vertical => self.height,
        }
    }

    fn fits(self, area: Rect) -> bool {
        self.width <= area.width as usize && self.height <= area.height as usize
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PaneNode {
    Leaf(PaneId),
    Split {
        dir: SplitDir,
        split: Split,
        first: Box<PaneNode>,
        second: Box<PaneNode>,
    },
}

impl PaneNode {
    pub fn leaves(&self) -> Vec<PaneId> {
        match self {
            PaneNode::Leaf(id) => vec![*id],
            PaneNode::Split { first, second, .. } => {
                let mut v = first.leaves();
                v.extend(second.leaves());
                v
            }
        }
    }

    pub fn layout(
        &self,
        area: Rect,
        gap: u16,
        out: &mut Vec<(PaneId, Rect)>,
        seams: &mut Vec<Seam>,
        path: &mut Vec<u8>,
    ) {
        match self {
            PaneNode::Leaf(id) => out.push((*id, area)),
            PaneNode::Split {
                dir,
                split,
                first,
                second,
            } => {
                let (a, b) = split.layout(*dir, area, gap);
                let handle = split.handle(*dir, area, gap);
                seams.push(Seam {
                    path: path.clone(),
                    dir: *dir,
                    container: area,
                    handle,
                });
                path.push(0);
                first.layout(a, gap, out, seams, path);
                path.pop();
                path.push(1);
                second.layout(b, gap, out, seams, path);
                path.pop();
            }
        }
    }

    pub fn natural_size(&self, gap: u16, pane: &dyn Fn(PaneId) -> Size) -> Size {
        match self {
            PaneNode::Leaf(id) => pane(*id),
            PaneNode::Split {
                dir,
                split,
                first,
                second,
            } => {
                let (a, b) = (
                    first.natural_size(gap, pane),
                    second.natural_size(gap, pane),
                );
                match dir {
                    SplitDir::Horizontal => Size::new(
                        split.natural_extent(a.width, b.width, gap),
                        a.height.max(b.height),
                    ),
                    SplitDir::Vertical => Size::new(
                        a.width.max(b.width),
                        split.natural_extent(a.height, b.height, gap),
                    ),
                }
            }
        }
    }

    pub fn node_at_mut(&mut self, path: &[u8]) -> Option<&mut PaneNode> {
        let mut cur = self;
        for p in path {
            match cur {
                PaneNode::Split { first, second, .. } => {
                    cur = if *p == 0 { first } else { second };
                }
                PaneNode::Leaf(_) => return None,
            }
        }
        Some(cur)
    }

    pub fn split_leaf(
        &mut self,
        target: PaneId,
        new: PaneId,
        dir: SplitDir,
        new_first: bool,
    ) -> bool {
        match self {
            PaneNode::Leaf(id) if *id == target => {
                let old = PaneNode::Leaf(*id);
                let fresh = PaneNode::Leaf(new);
                let (first, second) = if new_first {
                    (fresh, old)
                } else {
                    (old, fresh)
                };
                *self = PaneNode::Split {
                    dir,
                    split: Split::new(50, 1, 1),
                    first: Box::new(first),
                    second: Box::new(second),
                };
                true
            }
            PaneNode::Leaf(_) => false,
            PaneNode::Split { first, second, .. } => {
                first.split_leaf(target, new, dir, new_first)
                    || second.split_leaf(target, new, dir, new_first)
            }
        }
    }

    pub fn remove_leaf(&mut self, target: PaneId) -> bool {
        if let PaneNode::Split { first, second, .. } = self {
            if matches!(**first, PaneNode::Leaf(id) if id == target) {
                let s = std::mem::replace(&mut **second, PaneNode::Leaf(0));
                *self = s;
                return true;
            }
            if matches!(**second, PaneNode::Leaf(id) if id == target) {
                let f = std::mem::replace(&mut **first, PaneNode::Leaf(0));
                *self = f;
                return true;
            }
            return first.remove_leaf(target) || second.remove_leaf(target);
        }
        false
    }

    pub fn path_to(&self, target: PaneId, path: &mut Vec<u8>) -> bool {
        match self {
            PaneNode::Leaf(id) => *id == target,
            PaneNode::Split { first, second, .. } => {
                path.push(0);
                if first.path_to(target, path) {
                    return true;
                }
                path.pop();
                path.push(1);
                if second.path_to(target, path) {
                    return true;
                }
                path.pop();
                false
            }
        }
    }

    fn node_at(&self, path: &[u8]) -> Option<&PaneNode> {
        let mut cur = self;
        for p in path {
            match cur {
                PaneNode::Split { first, second, .. } => {
                    cur = if *p == 0 { first } else { second };
                }
                PaneNode::Leaf(_) => return None,
            }
        }
        Some(cur)
    }

    fn minimum(&self, leaf: PaneMinimum) -> PaneMinimum {
        match self {
            PaneNode::Leaf(_) => leaf,
            PaneNode::Split {
                dir, first, second, ..
            } => PaneMinimum::join(*dir, first.minimum(leaf), second.minimum(leaf)),
        }
    }

    /// Whether every leaf laid out in `area` keeps the outer minimum `leaf`.
    fn leaves_fit(&self, area: Rect, leaf: PaneMinimum) -> bool {
        match self {
            PaneNode::Leaf(_) => leaf.fits(area),
            PaneNode::Split {
                dir,
                split,
                first,
                second,
            } => {
                let (a, b) = split.layout(*dir, area, SEAM);
                first.leaves_fit(a, leaf) && second.leaves_fit(b, leaf)
            }
        }
    }

    fn unzoom(&mut self) {
        if let PaneNode::Split {
            split,
            first,
            second,
            ..
        } = self
        {
            split.maximized = Maximized::None;
            first.unzoom();
            second.unzoom();
        }
    }

    fn zoomed(&self) -> Option<PaneId> {
        match self {
            PaneNode::Leaf(id) => Some(*id),
            PaneNode::Split {
                split,
                first,
                second,
                ..
            } => match split.maximized {
                Maximized::First => first.zoomed(),
                Maximized::Second => second.zoomed(),
                Maximized::None => None,
            },
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Seam {
    pub path: Vec<u8>,
    pub dir: SplitDir,

    pub container: Rect,

    pub handle: Rect,
}

/// The seam a pointer gesture holds. `index` is the seam's position in the
/// layout that was current at press time, which is also the number its router
/// id carries. The rest identifies the split: its path, direction and leaves.
#[derive(Debug, Clone, PartialEq, Eq)]
struct SeamGrab {
    index: usize,
    path: Vec<u8>,
    dir: SplitDir,
    leaves: Vec<PaneId>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Direction {
    Left,
    Right,
    Up,
    Down,
}

pub fn nearest(leaves: &[(PaneId, Rect)], from: PaneId, dir: Direction) -> Option<PaneId> {
    let (_, fr) = leaves.iter().find(|(id, _)| *id == from)?;
    let (fx, fy) = (
        fr.x as i32 + fr.width as i32 / 2,
        fr.y as i32 + fr.height as i32 / 2,
    );
    leaves
        .iter()
        .filter(|(id, r)| {
            *id != from
                && match dir {
                    Direction::Left => r.right() as i32 <= fr.x as i32,
                    Direction::Right => r.x as i32 >= fr.right() as i32,
                    Direction::Up => r.bottom() as i32 <= fr.y as i32,
                    Direction::Down => r.y as i32 >= fr.bottom() as i32,
                }
        })
        .min_by_key(|(_, r)| {
            let cx = r.x as i32 + r.width as i32 / 2;
            let cy = r.y as i32 + r.height as i32 / 2;
            (cx - fx).abs() + (cy - fy).abs()
        })
        .map(|(id, _)| *id)
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PaneRefusal {
    ZoomedFocus,

    ZoomedResize,

    NoNeighbour,

    NothingToResize,

    /// The seam cannot move: a pane or the whole area is at its minimum size.
    AtMinimum,

    OnePane,
}

impl PaneRefusal {
    pub fn message(self) -> &'static str {
        match self {
            PaneRefusal::ZoomedFocus => "Unzoom to move focus",
            PaneRefusal::ZoomedResize => "Unzoom to resize",
            PaneRefusal::NoNeighbour => "No pane that way",
            PaneRefusal::NothingToResize => "Nothing to resize in that direction",
            PaneRefusal::AtMinimum => "Cannot resize: pane at minimum size",
            PaneRefusal::OnePane => "Nothing to zoom: one pane",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PaneCmd {
    Next,

    Prev,
    FocusLeft,
    FocusRight,
    FocusUp,
    FocusDown,

    Zoom,
}

impl Actions for PaneCmd {
    fn default_keymap() -> KeyMap<Self> {
        vec![
            (KeyBinding::key(KeyCode::F(6)), PaneCmd::Next),
            (
                KeyBinding::new(KeyCode::F(6), KeyModifiers::SHIFT),
                PaneCmd::Prev,
            ),
            (
                KeyBinding::new(KeyCode::Left, KeyModifiers::ALT),
                PaneCmd::FocusLeft,
            ),
            (
                KeyBinding::new(KeyCode::Right, KeyModifiers::ALT),
                PaneCmd::FocusRight,
            ),
            (
                KeyBinding::new(KeyCode::Up, KeyModifiers::ALT),
                PaneCmd::FocusUp,
            ),
            (
                KeyBinding::new(KeyCode::Down, KeyModifiers::ALT),
                PaneCmd::FocusDown,
            ),
            (KeyBinding::alt('z'), PaneCmd::Zoom),
            (
                KeyBinding::new(
                    KeyCode::F(12),
                    KeyModifiers::CONTROL.union(KeyModifiers::SHIFT),
                ),
                PaneCmd::Zoom,
            ),
        ]
    }

    fn label(self) -> &'static str {
        match self {
            PaneCmd::Next => "Next pane",
            PaneCmd::Prev => "Previous pane",
            PaneCmd::FocusLeft => "Pane to the left",
            PaneCmd::FocusRight => "Pane to the right",
            PaneCmd::FocusUp => "Pane above",
            PaneCmd::FocusDown => "Pane below",
            PaneCmd::Zoom => "Zoom pane",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Frames {
    #[default]
    WhenSplit,

    Always,

    Never,
}

pub type PaneResult = Result<Outcome, PaneRefusal>;

#[derive(Debug, Clone)]
pub struct PaneTree {
    pub id: WidgetId,
    pub root: PaneNode,

    pub focused: PaneId,

    pub keys: KeyMap<PaneCmd>,

    pub split_keys: SplitKeys,

    /// The smallest outer rectangle (columns, rows) a pane may be resized to.
    ///
    /// Keyboard resizing, seam dragging and a double-click reset share this
    /// limit and clamp to the boundary it implies, counting every pane and
    /// seam below a split. Frames are not added: a framed pane keeps
    /// `outer.saturating_sub(2)` cells of content. When the area is too small
    /// for the limits, or the 5% to 95% ratio cannot express the boundary,
    /// resizing is refused and the split stays as it was until the area grows.
    pub min_size: (u16, u16),

    pub frames: Frames,

    pub hit_pad: u16,
    area: Rect,
    rects: Vec<(PaneId, Rect)>,
    seams: Vec<Seam>,
    handles: Vec<Splitter>,

    grab: Option<SeamGrab>,

    dead: Option<WidgetId>,
}

impl PaneTree {
    pub fn new(id: WidgetId, first: PaneId) -> Self {
        Self {
            id,
            root: PaneNode::Leaf(first),
            focused: first,
            keys: PaneCmd::default_keymap(),
            split_keys: SplitKeys::default(),
            min_size: (12, 3),
            frames: Frames::default(),
            hit_pad: 1,
            area: Rect::ZERO,
            rects: Vec::new(),
            seams: Vec::new(),
            handles: Vec::new(),
            grab: None,
            dead: None,
        }
    }

    pub fn keys(mut self, keys: KeyMap<PaneCmd>) -> Self {
        self.keys = keys;
        self
    }

    pub fn split_keys(mut self, keys: SplitKeys) -> Self {
        self.split_keys = keys;
        self
    }

    pub fn min_size(mut self, cols: u16, rows: u16) -> Self {
        self.min_size = (cols, rows);
        self
    }

    pub fn frames(mut self, frames: Frames) -> Self {
        self.frames = frames;
        self
    }

    pub fn hit_pad(mut self, cells: u16) -> Self {
        self.hit_pad = cells;
        self
    }

    pub fn leaves(&self) -> Vec<PaneId> {
        self.root.leaves()
    }

    pub fn panes(&self) -> &[(PaneId, Rect)] {
        &self.rects
    }

    pub fn seams(&self) -> &[Seam] {
        &self.seams
    }

    pub fn zoomed(&self) -> Option<PaneId> {
        match self.root {
            PaneNode::Leaf(_) => None,
            _ => self.root.zoomed(),
        }
    }

    pub fn framed(&self) -> bool {
        match self.frames {
            Frames::Always => true,
            Frames::Never => false,
            Frames::WhenSplit => self.leaves().len() > 1 || self.zoomed().is_some(),
        }
    }

    pub fn split(&mut self, target: PaneId, new: PaneId, dir: SplitDir, new_first: bool) -> bool {
        self.root.unzoom();
        let done = self.root.split_leaf(target, new, dir, new_first);
        if done {
            self.focused = new;
            self.abandon_seam_drag();
        }
        done
    }

    pub fn close(&mut self, target: PaneId) -> Result<(), PaneRefusal> {
        if self.leaves().len() <= 1 {
            return Err(PaneRefusal::OnePane);
        }
        if self.zoomed() == Some(target) {
            self.root.unzoom();
        }
        if !self.root.remove_leaf(target) {
            return Err(PaneRefusal::NoNeighbour);
        }
        if self.focused == target {
            self.focused = self.leaves()[0];
        }
        self.abandon_seam_drag();
        Ok(())
    }

    pub fn toggle_zoom(&mut self) -> PaneResult {
        if self.zoomed().is_some() {
            self.root.unzoom();
            self.abandon_seam_drag();
            return Ok(Outcome::Changed);
        }
        if self.leaves().len() <= 1 {
            return Err(PaneRefusal::OnePane);
        }
        let mut path = vec![];
        if !self.root.path_to(self.focused, &mut path) {
            return Err(PaneRefusal::NoNeighbour);
        }
        for k in 0..path.len() {
            if let Some(PaneNode::Split { split, .. }) = self.root.node_at_mut(&path[..k]) {
                split.toggle_max(if path[k] == 0 {
                    Maximized::First
                } else {
                    Maximized::Second
                });
            }
        }
        self.abandon_seam_drag();
        Ok(Outcome::Changed)
    }

    fn seam_grab(&self, index: usize) -> Option<SeamGrab> {
        let seam = self.seams.get(index)?;
        let node = self.root.node_at(&seam.path)?;
        let PaneNode::Split { dir, .. } = node else {
            return None;
        };
        Some(SeamGrab {
            index,
            path: seam.path.clone(),
            dir: *dir,
            leaves: node.leaves(),
        })
    }

    /// Clears the held seam and every handle's grab. Returns whether a seam was held.
    fn cancel_seam_drag(&mut self) -> bool {
        let mut changed = self.grab.take().is_some();
        for h in &mut self.handles {
            changed |= h.cancel_drag();
        }
        changed
    }

    /// Cancels the held seam when the layout around it changed. The router keeps
    /// pressing the old seam id until the gesture ends, so that id stays dead
    /// until the next press or terminal phase. A new seam drawn under the same
    /// id is then neither resized nor drawn pressed.
    fn abandon_seam_drag(&mut self) {
        if let Some(index) = self.grab.as_ref().map(|g| g.index) {
            self.dead = Some(self.seam_id(index));
        }
        self.cancel_seam_drag();
    }

    fn release_stale_grab(&mut self) {
        let Some(grab) = &self.grab else {
            return;
        };
        if self.seam_grab(grab.index).as_ref() != Some(grab) {
            self.abandon_seam_drag();
        }
    }

    pub fn focus_toward(&mut self, dir: Direction) -> PaneResult {
        if self.zoomed().is_some() {
            return Err(PaneRefusal::ZoomedFocus);
        }
        let to = nearest(&self.rects, self.focused, dir).ok_or(PaneRefusal::NoNeighbour)?;
        self.focused = to;
        Ok(Outcome::Changed)
    }

    pub fn focus_next(&mut self, back: bool) -> PaneResult {
        if self.zoomed().is_some() {
            return Err(PaneRefusal::ZoomedFocus);
        }
        let leaves = self.leaves();
        if leaves.len() <= 1 {
            return Err(PaneRefusal::NoNeighbour);
        }
        let i = leaves.iter().position(|p| *p == self.focused).unwrap_or(0);
        let n = leaves.len();
        self.focused = leaves[if back { (i + n - 1) % n } else { (i + 1) % n }];
        Ok(Outcome::Changed)
    }

    pub fn resize(&mut self, dir: Direction, cells: u16) -> PaneResult {
        if self.zoomed().is_some() {
            return Err(PaneRefusal::ZoomedResize);
        }
        let mut path = vec![];
        if !self.root.path_to(self.focused, &mut path) {
            return Err(PaneRefusal::NothingToResize);
        }
        let want = match dir {
            Direction::Left | Direction::Right => SplitDir::Horizontal,
            Direction::Up | Direction::Down => SplitDir::Vertical,
        };
        let grows_first = matches!(dir, Direction::Right | Direction::Down);
        let mut chosen = None;
        for k in (0..path.len()).rev() {
            if let Some(PaneNode::Split { dir: sd, .. }) = self.root.node_at_mut(&path[..k])
                && *sd == want
            {
                let delta = cells.min(i16::MAX as u16) as i16;
                chosen = Some((path[..k].to_vec(), if grows_first { delta } else { -delta }));
                break;
            }
        }
        let Some((p, delta)) = chosen else {
            return Err(PaneRefusal::NothingToResize);
        };
        let Some(seam) = self.seams.iter().find(|s| s.path == p).cloned() else {
            return Err(PaneRefusal::NothingToResize);
        };
        let container = seam.container;
        let moved = self.commit_split_candidate(&p, container, |split, sd| {
            split.nudge(sd, container, SEAM, delta);
        });
        if moved {
            Ok(Outcome::Changed)
        } else {
            Err(PaneRefusal::AtMinimum)
        }
    }

    /// The one place a seam moves. `edit` runs on a copy of the split at `path`,
    /// clamped to the minimum extents of both subtrees, so a drag or key step
    /// lands on the nearest feasible boundary. The copy then returns to the
    /// stored minima and is committed only if it moved the seam and every leaf
    /// below still meets `min_size` when laid out from those stored values.
    /// Otherwise the tree is untouched, `precise` state included.
    fn commit_split_candidate(
        &mut self,
        path: &[u8],
        container: Rect,
        edit: impl FnOnce(&mut Split, SplitDir),
    ) -> bool {
        let leaf = PaneMinimum::of(self.min_size);
        if !self.root.minimum(leaf).fits(self.area) {
            return false;
        }
        let Some(PaneNode::Split {
            dir,
            split,
            first,
            second,
        }) = self.root.node_at(path)
        else {
            return false;
        };
        let dir = *dir;
        let (first_min, second_min) = (first.minimum(leaf), second.minimum(leaf));
        if !PaneMinimum::join(dir, first_min, second_min).fits(container) {
            return false;
        }
        let extent = |m: PaneMinimum| u16::try_from(m.along(dir)).unwrap_or(u16::MAX);
        let stored = *split;
        let mut candidate = stored;
        candidate.min_first = extent(first_min);
        candidate.min_second = extent(second_min);
        edit(&mut candidate, dir);
        candidate.min_first = stored.min_first;
        candidate.min_second = stored.min_second;

        let (a, b) = candidate.layout(dir, container, SEAM);
        let accepted = (a, b) != stored.layout(dir, container, SEAM)
            && first.leaves_fit(a, leaf)
            && second.leaves_fit(b, leaf);
        if accepted && let Some(PaneNode::Split { split, .. }) = self.root.node_at_mut(path) {
            *split = candidate;
        }
        accepted
    }

    pub fn natural_size(&self, content: &dyn Fn(PaneId) -> Size) -> Size {
        let frame = if self.framed() { 2 } else { 0 };
        let pane = |id: PaneId| {
            let c = content(id);
            Size::new(c.width + frame, c.height + frame)
        };
        match self.zoomed() {
            Some(z) => pane(z),
            None => self.root.natural_size(1, &pane),
        }
    }

    pub fn layout(&mut self, area: Rect) {
        self.area = area;
        self.rects.clear();
        self.seams.clear();
        let mut rects = Vec::new();
        let mut seams = Vec::new();
        self.root
            .layout(area, 1, &mut rects, &mut seams, &mut Vec::new());

        let zoomed = self.zoomed();
        self.rects = rects
            .into_iter()
            .filter(|(id, r)| zoomed.is_none_or(|z| z == *id) && !r.is_empty())
            .collect();
        if zoomed.is_none() {
            self.seams = seams.into_iter().filter(|s| !s.handle.is_empty()).collect();
        }
        self.release_stale_grab();
    }

    pub fn seam_id(&self, i: usize) -> WidgetId {
        self.id.sub("seam").child(i)
    }

    pub fn render(
        &mut self,
        area: Rect,
        buf: &mut Buffer,
        ctx: &mut RenderCtx,
        bg: Color,
        title: &dyn Fn(PaneId) -> String,
    ) -> Vec<(PaneId, Rect)> {
        self.layout(area);
        let t = ctx.theme;
        ctx.control(self.id, area, false);
        let tree_focused = ctx.interaction.focused(self.id);
        let framed = self.framed();

        let n = self.seams.len();
        self.handles.truncate(n);
        for i in self.handles.len()..n {
            let dir = self.seams[i].dir;
            self.handles
                .push(Splitter::new(self.seam_id(i), dir).hit_pad(self.hit_pad));
        }
        for i in 0..n {
            let s = &self.seams[i];
            let id = self.seam_id(i);
            let mut h = Splitter::new(id, s.dir).hit_pad(self.hit_pad);
            if self.handles[i].dir == s.dir {
                h = self.handles[i];
                h.hit_pad = self.hit_pad;
            }
            let held = self.grab.as_ref().is_some_and(|g| g.index == i);
            let saved = (ctx.interaction.pressed, ctx.interaction.hover);
            if self.dead == Some(id) {
                ctx.interaction.pressed = None;
                ctx.interaction.hover = None;
            }
            h.render_held(s.handle, buf, ctx, bg, held);
            (ctx.interaction.pressed, ctx.interaction.hover) = saved;
            self.handles[i] = h;
        }
        let mut inner = Vec::with_capacity(self.rects.len());
        let zoomed = self.zoomed().is_some();
        for &(pid, r) in &self.rects {
            let part = self.id.child(pid as usize);
            ctx.clickable(part, r);
            if !framed {
                inner.push((pid, r));
                continue;
            }
            let focused = tree_focused && pid == self.focused;
            let hovered = ctx.interaction.hovered(part);

            let name = title(pid);
            let meta = if zoomed { Some("zoomed") } else { None };
            let mut panel = termrock_viewport::panel::Panel::framed(Some(&name)).focused(focused);
            panel.bg_override = Some(bg);
            if let Some(m) = meta {
                panel = panel.meta(m);
            }
            panel.render(r, buf, t);
            if hovered && !focused && r.width > 4 {
                let w = termrock_text::cells::width(&name) as u16 + 2;
                for x in r.x + 2..(r.x + 2 + w).min(r.right().saturating_sub(2)) {
                    buf[(x, r.y)].set_style(t.primary().bg(bg));
                }
            }
            inner.push((pid, r.inner(Margin::new(1, 1))));
        }
        inner
    }

    pub fn on_key(&mut self, key: &Key) -> Option<PaneResult> {
        if let Some(cmd) = lookup(&self.keys, key) {
            return Some(match cmd {
                PaneCmd::Next => self.focus_next(false),
                PaneCmd::Prev => self.focus_next(true),
                PaneCmd::FocusLeft => self.focus_toward(Direction::Left),
                PaneCmd::FocusRight => self.focus_toward(Direction::Right),
                PaneCmd::FocusUp => self.focus_toward(Direction::Up),
                PaneCmd::FocusDown => self.focus_toward(Direction::Down),
                PaneCmd::Zoom => self.toggle_zoom(),
            });
        }
        let dir = [
            (SplitDir::Horizontal, Direction::Left, Direction::Right),
            (SplitDir::Vertical, Direction::Up, Direction::Down),
        ]
        .into_iter()
        .find_map(|(axis, back, fwd)| {
            self.split_keys
                .delta(key, axis)
                .map(|d| if d < 0 { back } else { fwd })
        })?;
        Some(self.resize(dir, self.split_keys.step))
    }

    pub fn on_pointer(&mut self, ev: &PointerEvent) -> Option<Outcome> {
        self.release_stale_grab();
        let seam = (0..self.seams.len()).find(|&i| self.seam_id(i) == ev.id);
        if seam.is_some() || self.dead == Some(ev.id) {
            return self.on_seam_pointer(ev, seam);
        }
        if ev.is(Phase::Click)
            && let Some(&(pid, _)) = self
                .rects
                .iter()
                .find(|(p, _)| self.id.child(*p as usize) == ev.id)
        {
            self.focused = pid;
            return Some(Outcome::Changed);
        }
        None
    }

    fn on_seam_pointer(&mut self, ev: &PointerEvent, seam: Option<usize>) -> Option<Outcome> {
        match ev.phase()? {
            Phase::Press => {
                let i = seam?;
                let grab = self.seam_grab(i)?;
                let container = self.seams[i].container;
                let mut h = self.handles.get(i).copied()?;
                let Some(PaneNode::Split { split, .. }) = self.root.node_at_mut(&grab.path) else {
                    return None;
                };
                self.dead = None;
                self.grab = Some(grab);
                h.on_press(split, container, SEAM, ev.pos());
                self.handles[i] = h;
                Some(Outcome::Consumed)
            }
            Phase::Drag => {
                let grabbed = seam.filter(|&i| self.grab.as_ref().is_some_and(|g| g.index == i));
                let Some(i) = grabbed else {
                    return Some(Outcome::Consumed);
                };
                let s = self.seams[i].clone();
                let mut h = self.handles.get(i).copied()?;
                let moved = self.commit_split_candidate(&s.path, s.container, |split, _| {
                    h.on_drag(split, s.container, SEAM, ev.pos());
                });
                self.handles[i] = h;
                Some(if moved {
                    Outcome::Changed
                } else {
                    Outcome::Consumed
                })
            }
            Phase::DragEnd => Some(self.end_seam_gesture()),
            Phase::Release => {
                self.dead = None;
                Some(self.end_seam_gesture())
            }
            Phase::Click => {
                let stale = self.dead.take() == Some(ev.id);
                let held = self.cancel_seam_drag();
                if stale {
                    return Some(if held {
                        Outcome::Changed
                    } else {
                        Outcome::Consumed
                    });
                }
                let i = seam?;
                let s = self.seams[i].clone();
                let mut h = self.handles.get(i).copied()?;
                let moved = self.commit_split_candidate(&s.path, s.container, |split, _| {
                    h.on_click(split, ev.count());
                });
                self.handles[i] = h;
                let o = if moved {
                    Outcome::Changed
                } else {
                    Outcome::Consumed
                };
                Some(if held && o == Outcome::Consumed {
                    Outcome::Changed
                } else {
                    o
                })
            }
            _ => None,
        }
    }

    fn end_seam_gesture(&mut self) -> Outcome {
        if self.cancel_seam_drag() {
            Outcome::Changed
        } else {
            Outcome::Consumed
        }
    }

    pub fn hints(&self) -> Vec<Hint> {
        let mut v = Vec::new();
        if self.zoomed().is_none() {
            if let Some(k) = hint_key(&self.keys, PaneCmd::Next) {
                v.push(hint(k, "Next pane"));
            }
            if let Some(h) = self.split_keys.hint(SplitDir::Horizontal) {
                v.push(h);
            }
        }
        if self.leaves().len() > 1
            && let Some(k) = hint_key(&self.keys, PaneCmd::Zoom)
        {
            v.push(hint(
                k,
                if self.zoomed().is_some() {
                    "Restore"
                } else {
                    "Zoom"
                },
            ));
        }
        v
    }
}

impl termrock_runtime::specimen::Specimen for PaneTree {
    fn specimen_id(&self) -> WidgetId {
        self.id
    }

    fn supports(&self, state: termrock_runtime::specimen::SpecimenState) -> bool {
        use termrock_runtime::specimen::SpecimenState::*;
        matches!(state, Rest | Hover | Focused | Selected)
    }

    fn specimen_height(&self, _state: termrock_runtime::specimen::SpecimenState) -> u16 {
        4
    }

    fn specimen(
        &self,
        state: termrock_runtime::specimen::SpecimenState,
        area: Rect,
        buf: &mut Buffer,
        ctx: &mut RenderCtx,
        bg: Color,
    ) {
        use termrock_runtime::specimen::SpecimenState;
        let mut tree = PaneTree::new(self.id, 1).min_size(4, 3);
        tree.split(1, 2, SplitDir::Horizontal, false);
        tree.focused = 1;
        if state == SpecimenState::Selected {
            let _ = tree.toggle_zoom();
        }
        let saved = ctx.interaction;
        match state {
            SpecimenState::Focused => ctx.interaction.focus = Some(self.id),
            SpecimenState::Hover => ctx.interaction.hover = Some(self.id.child(1)),
            _ => {}
        }
        let inert = std::mem::replace(&mut ctx.inert, true);
        let names = |p: PaneId| {
            if p == 1 {
                "shell".to_owned()
            } else {
                "logs".to_owned()
            }
        };
        tree.render(area, buf, ctx, bg, &names);
        ctx.inert = inert;
        ctx.interaction = saved;
    }
}
