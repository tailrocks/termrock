// SPDX-FileCopyrightText: 2026 Alexey Zhokhov
// SPDX-License-Identifier: Apache-2.0

//! FocusGraph — predictable focus for complex workbenches.
//!
//! Sole **public** focus-graph authority. [`super::InteractionScene`] still owns
//! layer Esc/hit registration; hosts may project scene elements into this graph
//! via [`FocusGraph::from_interaction`] or register explicitly.
//!
//! # Focus vs selection vs pointer
//!
//! - **Focus** (this type): which surface owns keyboard routing.
//! - **Selection**: collection-local cursor (`ListState`, table cursor, …).
//! - **Pointer**: [`Self::focus_at`] moves focus only; activation is a separate intent.
//! - **Roving**: a collection is one external focus target; internal Move stays on the widget.
use std::collections::VecDeque;

use ratatui_core::layout::{Position, Rect};

use crate::{
    input::{KeyCode, KeyEvent, KeyModifiers},
    interaction::{FocusRequest, InteractionScene, NavigationMove, UiIntent},
    style::{DesignSystem, PanelChrome, Role},
};

/// Result of a focus operation.
#[must_use]
#[derive(Debug, Clone, PartialEq, Eq)]
#[non_exhaustive]
pub enum FocusOutcome<Id> {
    /// Input or request did not apply.
    Ignored,
    /// Consumed without changing the focused id.
    Unchanged,
    /// Focus moved.
    Changed {
        /// Previous focus.
        from: Option<Id>,
        /// New focus.
        to: Option<Id>,
    },
}

impl<Id: PartialEq> FocusOutcome<Id> {
    /// Whether focus identity changed.
    #[must_use]
    pub fn changed(&self) -> bool {
        matches!(self, Self::Changed { .. })
    }
}

/// How arrow keys participate in focus movement.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
#[non_exhaustive]
pub enum FocusNavMode {
    /// Tab / BackTab only (arrows are widget intents).
    #[default]
    Linear,
    /// Arrows move to nearest neighbor by painted geometry.
    Spatial,
    /// Linear Tab; arrows spatial only when not inside a roving collection.
    Hybrid,
}

/// One focusable (or structural) node registered for the current frame.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FocusNode<Id> {
    /// Stable identity across frames.
    pub id: Id,
    /// Parent for scopes / traps / sibling reconcile.
    pub parent: Option<Id>,
    /// Named zone (sidebar, editor, dialog.footer, …).
    pub zone: Option<&'static str>,
    /// Painted geometry for spatial nav and pointer focus.
    pub area: Option<Rect>,
    /// Disabled targets are skipped by traversal.
    pub enabled: bool,
    /// When false, node is structural only (group / zone root).
    pub focusable: bool,
    /// Collection owns internal cursor; graph focuses this node once.
    pub roving: bool,
    /// Lower first; ties broken by registration order.
    pub tab_index: i32,
}

impl<Id> FocusNode<Id> {
    /// Focusable enabled leaf with no parent.
    #[must_use]
    pub fn leaf(id: Id, area: Rect) -> Self {
        Self {
            id,
            parent: None,
            zone: None,
            area: Some(area),
            enabled: true,
            focusable: true,
            roving: false,
            tab_index: 0,
        }
    }

    /// Roving collection surface (one external focus target).
    #[must_use]
    pub fn roving_collection(id: Id, area: Rect) -> Self {
        Self {
            id,
            parent: None,
            zone: None,
            area: Some(area),
            enabled: true,
            focusable: true,
            roving: true,
            tab_index: 0,
        }
    }

    /// Sets parent.
    #[must_use]
    pub fn parent(mut self, parent: Id) -> Self {
        self.parent = Some(parent);
        self
    }

    /// Sets zone label.
    #[must_use]
    pub const fn zone(mut self, zone: &'static str) -> Self {
        self.zone = Some(zone);
        self
    }

    /// Sets tab index.
    #[must_use]
    pub const fn tab_index(mut self, tab_index: i32) -> Self {
        self.tab_index = tab_index;
        self
    }

    /// Enabled flag.
    #[must_use]
    pub const fn enabled(mut self, enabled: bool) -> Self {
        self.enabled = enabled;
        self
    }

    /// Focusable flag.
    #[must_use]
    pub const fn focusable(mut self, focusable: bool) -> Self {
        self.focusable = focusable;
        self
    }
}

/// Debug / Studio snapshot of one frame's focus graph.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FocusDebugSnapshot {
    /// Focused id display.
    pub focused: Option<String>,
    /// Trap root display when modal trap active.
    pub trap_root: Option<String>,
    /// Tab-order ids (display form).
    pub tab_order: Vec<String>,
    /// History newest-last (display).
    pub history: Vec<String>,
    /// Nav mode label.
    pub mode: &'static str,
    /// Eligible count this frame.
    pub eligible: usize,
}

impl FocusDebugSnapshot {
    /// Compact Studio lines.
    #[must_use]
    pub fn summary_lines(&self, max: usize) -> Vec<String> {
        let mut lines = vec![format!(
            "focus:{} trap:{} mode:{} n:{}",
            self.focused.as_deref().unwrap_or("—"),
            self.trap_root.as_deref().unwrap_or("—"),
            self.mode,
            self.eligible
        )];
        if !self.tab_order.is_empty() {
            let order = self.tab_order.iter().take(12).cloned().collect::<Vec<_>>();
            lines.push(format!("tab: {}", order.join(" → ")));
        }
        if !self.history.is_empty() {
            let hist = self
                .history
                .iter()
                .rev()
                .take(6)
                .cloned()
                .collect::<Vec<_>>();
            lines.push(format!("hist: {}", hist.join(" · ")));
        }
        lines.truncate(max);
        lines
    }
}

/// Predictable focus graph for workbenches and overlays.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FocusGraph<Id> {
    nodes: Vec<FocusNode<Id>>,
    focused: Option<Id>,
    trap_root: Option<Id>,
    /// Openers under traps (bottom → top).
    restore_stack: Vec<Option<Id>>,
    history: VecDeque<Id>,
    mode: FocusNavMode,
    max_history: usize,
}

impl<Id> Default for FocusGraph<Id> {
    fn default() -> Self {
        Self::new()
    }
}

impl<Id> FocusGraph<Id> {
    /// Empty graph, linear mode, history capacity 32.
    #[must_use]
    pub fn new() -> Self {
        Self {
            nodes: Vec::new(),
            focused: None,
            trap_root: None,
            restore_stack: Vec::new(),
            history: VecDeque::new(),
            mode: FocusNavMode::Linear,
            max_history: 32,
        }
    }

    /// Sets navigation mode.
    #[must_use]
    pub const fn mode(mut self, mode: FocusNavMode) -> Self {
        self.mode = mode;
        self
    }

    /// Clears per-frame nodes (keeps focus id, traps, history).
    pub fn begin_frame(&mut self) {
        self.nodes.clear();
    }

    /// Reserves capacity for virtualized registration windows.
    pub fn reserve(&mut self, additional: usize) {
        self.nodes.reserve(additional);
    }

    /// Registers one node (duplicate id ignored; first wins).
    pub fn register(&mut self, node: FocusNode<Id>)
    where
        Id: PartialEq,
    {
        if self.nodes.iter().any(|n| n.id == node.id) {
            return;
        }
        self.nodes.push(node);
    }

    /// Attaches/updates painted geometry after render.
    pub fn attach_area(&mut self, id: &Id, area: Rect) -> bool
    where
        Id: PartialEq,
    {
        let Some(node) = self.nodes.iter_mut().find(|n| &n.id == id) else {
            return false;
        };
        node.area = Some(area);
        true
    }

    /// Nav mode.
    #[must_use]
    pub const fn nav_mode(&self) -> FocusNavMode {
        self.mode
    }

    /// Currently focused id.
    #[must_use]
    pub const fn focused(&self) -> Option<&Id> {
        self.focused.as_ref()
    }

    /// Whether `id` owns keyboard focus.
    #[must_use]
    pub fn is_focused(&self, id: &Id) -> bool
    where
        Id: PartialEq,
    {
        self.focused.as_ref() == Some(id)
    }

    /// Whether `id` should paint as keyboard owner (Panel chrome).
    #[must_use]
    pub fn owns_keyboard(&self, id: &Id) -> bool
    where
        Id: PartialEq,
    {
        self.is_focused(id)
    }

    /// Panel chrome helper for focus-visible borders.
    #[must_use]
    pub fn panel_chrome_for(&self, id: &Id) -> PanelChrome
    where
        Id: PartialEq,
    {
        if self.owns_keyboard(id) {
            PanelChrome::Focused
        } else {
            PanelChrome::Normal
        }
    }

    /// Active modal trap root.
    #[must_use]
    pub const fn trap_root(&self) -> Option<&Id> {
        self.trap_root.as_ref()
    }

    /// Registered nodes this frame.
    #[must_use]
    pub fn nodes(&self) -> &[FocusNode<Id>] {
        &self.nodes
    }

    /// Focus history (oldest → newest).
    #[must_use]
    pub fn history(&self) -> impl Iterator<Item = &Id> {
        self.history.iter()
    }
}

impl<Id: Clone + PartialEq> FocusGraph<Id> {
    /// Eligible focusable enabled ids in tab order (respects trap).
    #[must_use]
    pub fn tab_order(&self) -> Vec<&Id> {
        let mut nodes: Vec<&FocusNode<Id>> = self
            .nodes
            .iter()
            .filter(|n| n.focusable && n.enabled && self.in_trap(n))
            .collect();
        nodes.sort_by(|a, b| {
            a.tab_index.cmp(&b.tab_index).then_with(|| {
                // stable: registration index
                let ia = self.nodes.iter().position(|n| n.id == a.id).unwrap_or(0);
                let ib = self.nodes.iter().position(|n| n.id == b.id).unwrap_or(0);
                ia.cmp(&ib)
            })
        });
        nodes.into_iter().map(|n| &n.id).collect()
    }

    fn in_trap(&self, node: &FocusNode<Id>) -> bool {
        let Some(root) = &self.trap_root else {
            return true;
        };
        self.is_under_root(&node.id, root)
    }

    /// `id` is `root` or has `root` as an ancestor (walk parents).
    fn is_under_root(&self, id: &Id, root: &Id) -> bool {
        let mut walk = id.clone();
        for _ in 0..64 {
            if &walk == root {
                return true;
            }
            let Some(parent) = self
                .nodes
                .iter()
                .find(|n| n.id == walk)
                .and_then(|n| n.parent.clone())
            else {
                return false;
            };
            walk = parent;
        }
        false
    }

    fn push_history(&mut self, id: Id) {
        if self.history.back() == Some(&id) {
            return;
        }
        self.history.push_back(id);
        while self.history.len() > self.max_history {
            self.history.pop_front();
        }
    }

    fn set_focused(&mut self, id: Option<Id>) -> FocusOutcome<Id> {
        let from = self.focused.clone();
        if from == id {
            return FocusOutcome::Unchanged;
        }
        if let Some(to) = id.clone() {
            self.push_history(to);
        }
        self.focused = id.clone();
        FocusOutcome::Changed { from, to: id }
    }

    /// Reconciles focus when current id is missing/disabled.
    pub fn reconcile(&mut self) -> FocusOutcome<Id> {
        let order: Vec<Id> = self.tab_order().into_iter().cloned().collect();
        if order.is_empty() {
            return self.set_focused(None);
        }
        if let Some(cur) = &self.focused
            && order.iter().any(|id| id == cur)
        {
            return FocusOutcome::Unchanged;
        }
        // Prefer sibling under same parent, else first in tab order.
        let preferred = self
            .focused
            .as_ref()
            .and_then(|old| {
                let parent = self.nodes.iter().find(|n| &n.id == old)?.parent.clone();
                order.iter().find(|id| {
                    self.nodes
                        .iter()
                        .find(|n| &n.id == *id)
                        .is_some_and(|n| n.parent == parent)
                })
            })
            .cloned()
            .or_else(|| order.first().cloned());
        self.set_focused(preferred)
    }

    /// Tab forward.
    pub fn focus_next(&mut self) -> FocusOutcome<Id> {
        self.move_linear(false)
    }

    /// Tab backward.
    pub fn focus_previous(&mut self) -> FocusOutcome<Id> {
        self.move_linear(true)
    }

    fn move_linear(&mut self, reverse: bool) -> FocusOutcome<Id> {
        let order: Vec<Id> = self.tab_order().into_iter().cloned().collect();
        if order.is_empty() {
            return FocusOutcome::Unchanged;
        }
        let idx = self
            .focused
            .as_ref()
            .and_then(|f| order.iter().position(|id| id == f));
        let next = match (idx, reverse) {
            (Some(0) | None, true) => order.len() - 1,
            (Some(i), true) => i - 1,
            (Some(i), false) => (i + 1) % order.len(),
            (None, false) => 0,
        };
        self.set_focused(Some(order[next].clone()))
    }

    /// Spatial step using painted centers (requires areas).
    pub fn focus_spatial(&mut self, dir: NavigationMove) -> FocusOutcome<Id> {
        let Some(cur) = self.focused.clone() else {
            return self.focus_next();
        };
        let Some(cur_area) = self.nodes.iter().find(|n| n.id == cur).and_then(|n| n.area) else {
            return FocusOutcome::Ignored;
        };
        let origin = center(cur_area);
        let order: Vec<&FocusNode<Id>> = self
            .nodes
            .iter()
            .filter(|n| {
                n.focusable && n.enabled && n.id != cur && self.in_trap(n) && n.area.is_some()
            })
            .collect();
        let mut best: Option<(Id, i64)> = None;
        for n in order {
            let area = n.area.expect("filtered");
            let c = center(area);
            if !in_direction(origin, c, dir) {
                continue;
            }
            let dist = manhattan(origin, c);
            if best.as_ref().is_none_or(|(_, d)| dist < *d) {
                best = Some((n.id.clone(), dist));
            }
        }
        match best {
            Some((id, _)) => self.set_focused(Some(id)),
            None => FocusOutcome::Unchanged,
        }
    }

    /// Programmatic focus when eligible.
    pub fn request_focus(&mut self, id: Id) -> FocusOutcome<Id> {
        if !self
            .nodes
            .iter()
            .any(|n| n.id == id && n.focusable && n.enabled && self.in_trap(n))
        {
            return FocusOutcome::Ignored;
        }
        self.set_focused(Some(id))
    }

    /// Pointer focus: first eligible containing position (later registration wins).
    pub fn focus_at(&mut self, position: Position) -> FocusOutcome<Id> {
        let hit = self.nodes.iter().rev().find(|n| {
            n.focusable
                && n.enabled
                && self.in_trap(n)
                && n.area.is_some_and(|a| a.contains(position))
        });
        match hit {
            Some(n) => self.set_focused(Some(n.id.clone())),
            None => FocusOutcome::Ignored,
        }
    }

    /// Pushes a modal trap; only `root` subtree participates until [`Self::pop_trap`].
    pub fn push_trap(&mut self, root: Id, opener: Option<Id>) {
        self.restore_stack
            .push(opener.or_else(|| self.focused.clone()));
        self.trap_root = Some(root.clone());
        // Prefer focusing trap root if focusable, else first eligible under trap.
        if self
            .nodes
            .iter()
            .any(|n| n.id == root && n.focusable && n.enabled)
        {
            let _ = self.set_focused(Some(root));
        } else {
            let _ = self.reconcile();
        }
    }

    /// Pops trap and restores opener when possible.
    pub fn pop_trap(&mut self) -> FocusOutcome<Id> {
        let opener = self.restore_stack.pop().flatten();
        self.trap_root = if self.restore_stack.is_empty() {
            None
        } else {
            // Nested traps: keep prior trap if host re-pushed; simple model clears.
            None
        };
        if let Some(id) = opener {
            if self
                .nodes
                .iter()
                .any(|n| n.id == id && n.focusable && n.enabled)
            {
                return self.set_focused(Some(id));
            }
        }
        self.reconcile()
    }

    /// Applies [`FocusRequest`] from [`crate::interaction::EventResult`].
    pub fn apply_request(&mut self, request: FocusRequest<Id>) -> FocusOutcome<Id> {
        match request {
            FocusRequest::Set(id) => self.request_focus(id),
            FocusRequest::Clear => self.set_focused(None),
            FocusRequest::Next => self.focus_next(),
            FocusRequest::Previous => self.focus_previous(),
        }
    }

    /// Tab / BackTab / optional spatial arrows from raw keys.
    pub fn handle_key(&mut self, key: KeyEvent) -> FocusOutcome<Id> {
        if key.is_release() {
            return FocusOutcome::Ignored;
        }
        match key.code {
            KeyCode::Tab if key.modifiers.is_empty() => self.focus_next(),
            KeyCode::BackTab => self.focus_previous(),
            KeyCode::Tab if key.modifiers == KeyModifiers::SHIFT => self.focus_previous(),
            KeyCode::Up if self.spatial_arrows_active() => self.focus_spatial(NavigationMove::Up),
            KeyCode::Down if self.spatial_arrows_active() => {
                self.focus_spatial(NavigationMove::Down)
            }
            KeyCode::Left if self.spatial_arrows_active() => {
                self.focus_spatial(NavigationMove::Left)
            }
            KeyCode::Right if self.spatial_arrows_active() => {
                self.focus_spatial(NavigationMove::Right)
            }
            _ => FocusOutcome::Ignored,
        }
    }

    /// Routes focus-related [`UiIntent`]s.
    pub fn handle_intent(&mut self, intent: UiIntent) -> FocusOutcome<Id> {
        match intent {
            UiIntent::FocusNext => self.focus_next(),
            UiIntent::FocusPrevious => self.focus_previous(),
            UiIntent::Move(dir) if self.spatial_arrows_active() => self.focus_spatial(dir),
            _ => FocusOutcome::Ignored,
        }
    }

    fn spatial_arrows_active(&self) -> bool {
        match self.mode {
            FocusNavMode::Spatial => true,
            FocusNavMode::Linear => false,
            FocusNavMode::Hybrid => {
                // Inside roving collection: arrows belong to widget selection.
                !self.focused_is_roving()
            }
        }
    }

    fn focused_is_roving(&self) -> bool {
        self.focused.as_ref().is_some_and(|id| {
            self.nodes
                .iter()
                .find(|n| &n.id == id)
                .is_some_and(|n| n.roving)
        })
    }

    /// Whether the focused node is a roving collection (selection is internal).
    #[must_use]
    pub fn focused_roving(&self) -> bool {
        self.focused_is_roving()
    }

    /// Jump badge candidates: focusable visible areas.
    #[must_use]
    pub fn jump_regions(&self) -> Vec<crate::interaction::HitRegion<Id>> {
        self.nodes
            .iter()
            .filter(|n| n.focusable && n.enabled && self.in_trap(n))
            .filter_map(|n| {
                n.area.map(|area| crate::interaction::HitRegion {
                    id: n.id.clone(),
                    area,
                })
            })
            .collect()
    }

    /// Debug snapshot for Studio / Focus Lens.
    #[must_use]
    pub fn debug_snapshot(&self) -> FocusDebugSnapshot
    where
        Id: std::fmt::Display,
    {
        FocusDebugSnapshot {
            focused: self.focused.as_ref().map(ToString::to_string),
            trap_root: self.trap_root.as_ref().map(ToString::to_string),
            tab_order: self
                .tab_order()
                .into_iter()
                .map(ToString::to_string)
                .collect(),
            history: self.history.iter().map(ToString::to_string).collect(),
            mode: match self.mode {
                FocusNavMode::Linear => "linear",
                FocusNavMode::Spatial => "spatial",
                FocusNavMode::Hybrid => "hybrid",
            },
            eligible: self.tab_order().len(),
        }
    }

    /// Projects focusable interaction elements into a flat graph (no parents).
    #[must_use]
    pub fn from_interaction<LayerId, Action>(scene: &InteractionScene<Id, LayerId, Action>) -> Self
    where
        Id: Clone + PartialEq,
        LayerId: PartialEq,
        Action: Clone,
    {
        let mut g = Self::new();
        g.focused = scene.focused().cloned();
        for (i, el) in scene.elements().iter().enumerate() {
            if el.hidden {
                continue;
            }
            g.register(FocusNode {
                id: el.id.clone(),
                parent: None,
                zone: None,
                area: Some(el.area),
                enabled: el.enabled,
                focusable: el.focusable,
                roving: false,
                tab_index: i32::try_from(i).unwrap_or(i32::MAX),
            });
        }
        g
    }

    /// Syncs focus id from an interaction scene after scene tab routing.
    pub fn sync_from_scene<LayerId, Action>(
        &mut self,
        scene: &InteractionScene<Id, LayerId, Action>,
    ) {
        self.focused = scene.focused().cloned();
    }

    /// Pushes focused id into a scene when the graph moved (host bridge).
    pub fn apply_to_scene<LayerId, Action>(&self, scene: &mut InteractionScene<Id, LayerId, Action>)
    where
        Id: Clone + PartialEq,
        LayerId: PartialEq,
    {
        if let Some(id) = self.focused.clone() {
            let _ = scene.focus(id);
        }
    }
}

fn center(area: Rect) -> (i32, i32) {
    (
        i32::from(area.x) + i32::from(area.width) / 2,
        i32::from(area.y) + i32::from(area.height) / 2,
    )
}

fn manhattan(a: (i32, i32), b: (i32, i32)) -> i64 {
    i64::from((a.0 - b.0).abs() + (a.1 - b.1).abs())
}

fn in_direction(from: (i32, i32), to: (i32, i32), dir: NavigationMove) -> bool {
    let dx = to.0 - from.0;
    let dy = to.1 - from.1;
    match dir {
        NavigationMove::Up | NavigationMove::Previous => dy < 0 && dy.abs() >= dx.abs(),
        NavigationMove::Down | NavigationMove::Next => dy > 0 && dy.abs() >= dx.abs(),
        NavigationMove::Left => dx < 0 && dx.abs() >= dy.abs(),
        NavigationMove::Right => dx > 0 && dx.abs() >= dy.abs(),
        NavigationMove::First | NavigationMove::Last => false,
    }
}

/// Focus Lens paint mode.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
#[non_exhaustive]
pub enum FocusLensMode {
    /// Tab-order indices only (default Studio debug).
    #[default]
    TabOrder,
    /// Focused outline marker only.
    FocusedOnly,
    /// Tab order + focused marker.
    Combined,
}

impl FocusLensMode {
    /// Stable id.
    #[must_use]
    pub const fn id(self) -> &'static str {
        match self {
            Self::TabOrder => "tab-order",
            Self::FocusedOnly => "focused-only",
            Self::Combined => "combined",
        }
    }
}

/// Focus Lens: paints tab-order markers and focused outline for Studio debug.
///
/// Complements JumpMode: lens is **inspection** (order / focus), jump is
/// **activation** (key labels). Neither mutates widgets beyond reading the
/// graph / semantic scene hosts already maintain.
#[derive(Debug, Clone, Copy)]
pub struct FocusLens<'a, Id> {
    graph: &'a FocusGraph<Id>,
    system: &'a DesignSystem,
    show_order: bool,
    mode: FocusLensMode,
    colorless: bool,
}

impl<'a, Id> FocusLens<'a, Id> {
    /// Creates a lens over a graph snapshot.
    #[must_use]
    pub const fn new(graph: &'a FocusGraph<Id>, system: &'a DesignSystem) -> Self {
        Self {
            graph,
            system,
            show_order: true,
            mode: FocusLensMode::Combined,
            colorless: false,
        }
    }

    /// Whether to paint tab-order indices.
    #[must_use]
    pub const fn show_order(mut self, show: bool) -> Self {
        self.show_order = show;
        self
    }

    /// Lens mode.
    #[must_use]
    pub const fn mode(mut self, mode: FocusLensMode) -> Self {
        self.mode = mode;
        self
    }

    /// Reduced-color roles (strong/muted only).
    #[must_use]
    pub const fn colorless(mut self, on: bool) -> Self {
        self.colorless = on;
        self
    }
}

impl<Id: Clone + PartialEq + std::fmt::Display> ratatui_core::widgets::Widget
    for &FocusLens<'_, Id>
{
    fn render(self, _area: Rect, buffer: &mut ratatui_core::buffer::Buffer) {
        let accent = if self.colorless {
            // Monochrome states the lens with weight; a reversal reads as a
            // selection, and the lens is an overlay, not a selection.
            self.system
                .style(Role::TextStrong)
                .add_modifier(ratatui_core::style::Modifier::BOLD)
        } else {
            self.system.style(Role::BorderFocused)
        };
        let muted = self.system.style(Role::TextMuted);
        let order = self.graph.tab_order();
        let show_order = self.show_order
            && matches!(self.mode, FocusLensMode::TabOrder | FocusLensMode::Combined);
        let show_focus = matches!(
            self.mode,
            FocusLensMode::FocusedOnly | FocusLensMode::Combined
        );
        for (i, id) in order.iter().enumerate() {
            let Some(node) = self.graph.nodes().iter().find(|n| &n.id == *id) else {
                continue;
            };
            let Some(area) = node.area else {
                continue;
            };
            if area.width == 0 || area.height == 0 {
                continue;
            }
            let focused = self.graph.is_focused(id);
            let style = if focused { accent } else { muted };
            if show_order {
                let label = format!("{}", i + 1);
                buffer.set_stringn(area.x, area.y, &label, usize::from(area.width), style);
            }
            if show_focus && focused {
                let mark = "◈";
                // Prefer trailing corner when order digit already at origin.
                let mx = if show_order && area.width > 1 {
                    area.x.saturating_add(area.width.saturating_sub(1))
                } else {
                    area.x
                };
                buffer.set_stringn(mx, area.y, mark, 1, accent);
            }
        }
    }
}

impl<Id: Clone + PartialEq + std::fmt::Display> ratatui_core::widgets::Widget
    for FocusLens<'_, Id>
{
    fn render(self, area: Rect, buffer: &mut ratatui_core::buffer::Buffer) {
        ratatui_core::widgets::Widget::render(&self, area, buffer);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::interaction::{InteractionElement, InteractionLayer, LayerDismissPolicy, LayerKind};

    #[derive(Debug, Clone, Copy, PartialEq, Eq)]
    enum Id {
        Sidebar,
        List,
        Editor,
        Dialog,
        Ok,
        Cancel,
    }

    impl std::fmt::Display for Id {
        fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
            write!(f, "{self:?}")
        }
    }

    fn key(code: KeyCode) -> KeyEvent {
        KeyEvent::new(code, KeyModifiers::NONE)
    }

    fn sample_graph() -> FocusGraph<Id> {
        let mut g = FocusGraph::new().mode(FocusNavMode::Hybrid);
        g.register(
            FocusNode::leaf(Id::Sidebar, Rect::new(0, 0, 10, 5))
                .zone("sidebar")
                .tab_index(0),
        );
        g.register(
            FocusNode::roving_collection(Id::List, Rect::new(10, 0, 20, 10))
                .zone("main")
                .tab_index(1),
        );
        g.register(
            FocusNode::leaf(Id::Editor, Rect::new(10, 10, 20, 5))
                .zone("main")
                .tab_index(2),
        );
        let _ = g.reconcile();
        g
    }

    #[test]
    fn tab_order_and_wrap() {
        let mut g = sample_graph();
        assert_eq!(g.focused(), Some(&Id::Sidebar));
        assert!(g.focus_next().changed());
        assert_eq!(g.focused(), Some(&Id::List));
        assert!(g.focused_roving());
        let _ = g.focus_next();
        assert_eq!(g.focused(), Some(&Id::Editor));
        let _ = g.focus_next();
        assert_eq!(g.focused(), Some(&Id::Sidebar));
    }

    #[test]
    fn hybrid_skips_spatial_inside_roving() {
        let mut g = sample_graph();
        let _ = g.request_focus(Id::List);
        assert_eq!(g.handle_key(key(KeyCode::Down)), FocusOutcome::Ignored);
        let _ = g.request_focus(Id::Sidebar);
        // Spatial from sidebar toward list (right/down).
        let out = g.focus_spatial(NavigationMove::Right);
        assert!(out.changed() || matches!(out, FocusOutcome::Unchanged));
    }

    #[test]
    fn trap_restores_opener() {
        let mut g = FocusGraph::new();
        g.register(FocusNode::leaf(Id::Sidebar, Rect::new(0, 0, 10, 5)).tab_index(0));
        g.register(FocusNode::roving_collection(Id::List, Rect::new(10, 0, 20, 10)).tab_index(1));
        g.register(FocusNode::leaf(Id::Editor, Rect::new(10, 10, 20, 5)).tab_index(2));
        g.register(FocusNode::leaf(Id::Dialog, Rect::new(5, 5, 30, 8)).tab_index(10));
        g.register(
            FocusNode::leaf(Id::Ok, Rect::new(6, 10, 4, 1))
                .parent(Id::Dialog)
                .tab_index(11),
        );
        g.register(
            FocusNode::leaf(Id::Cancel, Rect::new(12, 10, 6, 1))
                .parent(Id::Dialog)
                .enabled(false)
                .tab_index(12),
        );
        let _ = g.request_focus(Id::Editor);
        g.push_trap(Id::Dialog, Some(Id::Editor));
        assert_eq!(g.focused(), Some(&Id::Dialog));
        // Tab stays inside trap (Dialog, Ok) — Cancel disabled
        let _ = g.focus_next();
        assert_eq!(g.focused(), Some(&Id::Ok));
        let _ = g.focus_next();
        assert!(matches!(g.focused(), Some(&Id::Dialog) | Some(&Id::Ok)));
        let _ = g.pop_trap();
        assert_eq!(g.focused(), Some(&Id::Editor));
    }

    #[test]
    fn reconcile_drops_disabled() {
        let mut g = FocusGraph::new();
        g.register(FocusNode::leaf(Id::Sidebar, Rect::new(0, 0, 1, 1)));
        g.register(FocusNode::leaf(Id::List, Rect::new(2, 0, 1, 1)));
        let _ = g.request_focus(Id::List);
        g.begin_frame();
        g.register(FocusNode::leaf(Id::Sidebar, Rect::new(0, 0, 1, 1)));
        g.register(FocusNode::leaf(Id::List, Rect::new(2, 0, 1, 1)).enabled(false));
        let out = g.reconcile();
        assert!(out.changed());
        assert_eq!(g.focused(), Some(&Id::Sidebar));
    }

    #[test]
    fn focus_at_pointer_does_not_require_activate() {
        let mut g = sample_graph();
        assert!(g.focus_at(Position::new(15, 2)).changed());
        assert_eq!(g.focused(), Some(&Id::List));
    }

    #[test]
    fn apply_request_and_event_result_bridge() {
        let mut g = sample_graph();
        assert!(g.apply_request(FocusRequest::Next).changed());
        assert_eq!(g.focused(), Some(&Id::List));
        assert!(g.apply_request(FocusRequest::Previous).changed());
    }

    #[test]
    fn from_interaction_adapter() {
        let mut scene = InteractionScene::<Id, u8, ()>::new();
        scene.ensure_root(InteractionLayer {
            id: 0,
            kind: LayerKind::Root,
            owns_input: true,
            esc: LayerDismissPolicy::Ignore,
            outside: LayerDismissPolicy::Ignore,
            focus_return: None,
        });
        scene
            .register(InteractionElement::control(
                Id::Sidebar,
                0,
                Rect::new(0, 0, 2, 2),
            ))
            .unwrap();
        scene
            .register(InteractionElement::control(
                Id::Editor,
                0,
                Rect::new(3, 0, 2, 2),
            ))
            .unwrap();
        scene.reconcile();
        let g = FocusGraph::from_interaction(&scene);
        assert_eq!(g.tab_order().len(), 2);
        assert_eq!(g.focused(), scene.focused());
    }

    #[test]
    fn debug_snapshot_lines() {
        let g = sample_graph();
        let snap = g.debug_snapshot();
        assert!(snap.summary_lines(4)[0].contains("focus:"));
        assert!(snap.eligible >= 1);
    }

    #[test]
    fn history_records_moves() {
        let mut g = sample_graph();
        let _ = g.focus_next();
        let _ = g.focus_next();
        assert!(g.history().count() >= 2);
    }
}
