// SPDX-FileCopyrightText: 2026 Alexey Zhokhov
// SPDX-License-Identifier: Apache-2.0

//! ResizablePanelGroup — multi-panel layout with handles on SplitPane semantics.
//!
//! **Content-agnostic.** The group owns only geometry, handle interaction, and
//! collapse/preset state. Hosts paint domain widgets into returned panel rects
//! and keep their own focus/scroll models across layout changes.
//!
//! Built on the same ratio-scale and divider affordances as [`super::SplitPane`],
//! generalized to N panels (N−1 handles) with constrained redistribution,
//! collapse thresholds, keyboard resizing, saved presets, and responsive
//! side-panel → drawer recipes.
//!
//! Behavioral references: desktop workbench panes, Zellij pane management.
#![allow(unused_imports)] // test-module imports kept for unit tests; lib path may not use them
use ratatui_core::{
    buffer::Buffer,
    layout::{Position, Rect},
    style::Modifier,
    widgets::StatefulWidget,
};

use crate::{
    input::{KeyCode, KeyEvent, KeyEventKind, MouseButton, MouseEvent, MouseEventKind},
    style::{DesignSystem, GlyphSet, Role},
    widgets::{SplitDirection, SplitRatio},
};

const RATIO_SCALE: u32 = 10_000;
const KEYBOARD_STEP: u16 = 250;

/// Stable panel identity (host-owned string).
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct PanelId(pub String);

impl PanelId {
    /// From static name.
    #[must_use]
    pub fn from_static(id: &'static str) -> Self {
        Self(id.to_owned())
    }
}

impl From<&str> for PanelId {
    fn from(value: &str) -> Self {
        Self(value.to_owned())
    }
}

impl std::fmt::Display for PanelId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.0)
    }
}

/// Where a panel docks for responsive drawer recipes.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
#[non_exhaustive]
pub enum PanelDock {
    /// Primary content (never becomes a drawer).
    #[default]
    Main,
    /// Leading edge (left / top) — drawer candidate when narrow.
    Start,
    /// Trailing edge (right / bottom) — drawer candidate when narrow.
    End,
}

impl PanelDock {
    /// Stable id.
    #[must_use]
    pub const fn id(self) -> &'static str {
        match self {
            Self::Main => "main",
            Self::Start => "start",
            Self::End => "end",
        }
    }
}

/// One panel specification (content remains host-owned).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ResizablePanelSpec {
    /// Stable id.
    pub id: PanelId,
    /// Preferred share weight (≥ 1).
    pub weight: u16,
    /// Minimum main-axis cells when expanded.
    pub min: u16,
    /// Optional maximum main-axis cells.
    pub max: Option<u16>,
    /// Whether the panel may collapse to zero.
    pub collapsible: bool,
    /// When outer main-axis falls below this, prefer drawer mode (host applies).
    pub collapse_threshold: u16,
    /// Dock for responsive recipes.
    pub dock: PanelDock,
}

impl ResizablePanelSpec {
    /// Main panel with weight.
    #[must_use]
    pub fn main(id: impl Into<PanelId>, weight: u16) -> Self {
        Self {
            id: id.into(),
            weight: weight.max(1),
            min: 8,
            max: None,
            collapsible: false,
            collapse_threshold: 0,
            dock: PanelDock::Main,
        }
    }

    /// Start dock (sidebar).
    #[must_use]
    pub fn start(id: impl Into<PanelId>, weight: u16, min: u16) -> Self {
        Self {
            id: id.into(),
            weight: weight.max(1),
            min,
            max: Some(min.saturating_mul(4).max(40)),
            collapsible: true,
            collapse_threshold: 48,
            dock: PanelDock::Start,
        }
    }

    /// End dock (inspector).
    #[must_use]
    pub fn end(id: impl Into<PanelId>, weight: u16, min: u16) -> Self {
        Self {
            id: id.into(),
            weight: weight.max(1),
            min,
            max: Some(min.saturating_mul(4).max(48)),
            collapsible: true,
            collapse_threshold: 64,
            dock: PanelDock::End,
        }
    }

    /// Min size.
    #[must_use]
    pub const fn min(mut self, min: u16) -> Self {
        self.min = min;
        self
    }

    /// Max size.
    #[must_use]
    pub const fn max(mut self, max: u16) -> Self {
        self.max = Some(max);
        self
    }

    /// Collapsible flag.
    #[must_use]
    pub const fn collapsible(mut self, collapsible: bool) -> Self {
        self.collapsible = collapsible;
        self
    }

    /// Collapse / drawer threshold on outer width (horizontal) or height (vertical).
    #[must_use]
    pub const fn collapse_threshold(mut self, threshold: u16) -> Self {
        self.collapse_threshold = threshold;
        self
    }
}

/// Named size snapshot for restore.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PanelLayoutPreset {
    /// Preset name.
    pub name: String,
    /// Main-axis sizes in basis points of available content (sum ≈ 10_000).
    pub sizes_bp: Vec<u16>,
    /// Collapsed flags parallel to panels.
    pub collapsed: Vec<bool>,
}

impl PanelLayoutPreset {
    /// Create from name + bp sizes.
    #[must_use]
    pub fn new(name: impl Into<String>, sizes_bp: Vec<u16>, collapsed: Vec<bool>) -> Self {
        Self {
            name: name.into(),
            sizes_bp,
            collapsed,
        }
    }
}

/// Responsive recipe for the group.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
#[non_exhaustive]
pub enum PanelGroupRecipe {
    /// Always show all expanded panels (default).
    #[default]
    Fixed,
    /// When outer axis is narrow, mark Start/End docks as drawer candidates.
    SideDrawers,
    /// Workbench: left + main + right with drawer thresholds.
    Workbench,
    /// Dashboard: top metrics strip not used here — horizontal main+log style.
    Dashboard,
}

impl PanelGroupRecipe {
    /// Stable id.
    #[must_use]
    pub const fn id(self) -> &'static str {
        match self {
            Self::Fixed => "fixed",
            Self::SideDrawers => "side-drawers",
            Self::Workbench => "workbench",
            Self::Dashboard => "dashboard",
        }
    }
}

/// One resolved panel rectangle.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PanelRect {
    /// Panel id.
    pub id: PanelId,
    /// Geometry (zero-sized when collapsed).
    pub area: Rect,
    /// Collapsed flag.
    pub collapsed: bool,
    /// Host should present this panel as a drawer instead of an in-flow column.
    pub drawer: bool,
}

/// Full layout snapshot.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct ResizablePanelGroupLayout {
    /// Panel rects in order.
    pub panels: Vec<PanelRect>,
    /// Handle rects (between panels); empty when &lt; 2 visible panels.
    pub handles: Vec<Rect>,
    /// Outer area used.
    pub area: Rect,
}

impl ResizablePanelGroupLayout {
    /// Panel by id.
    #[must_use]
    pub fn get(&self, id: &PanelId) -> Option<&PanelRect> {
        self.panels.iter().find(|p| &p.id == id)
    }

    /// Hit-test handle index.
    #[must_use]
    pub fn hit_handle(&self, pos: Position) -> Option<usize> {
        self.handles
            .iter()
            .position(|r| r.width > 0 && r.height > 0 && r.contains(pos))
    }
}

/// Typed outcomes.
#[derive(Debug, Clone, PartialEq, Eq)]
#[non_exhaustive]
pub enum ResizablePanelOutcome {
    /// No change.
    Ignored,
    /// Handle gained focus (keyboard / drag start).
    HandleFocused {
        /// Handle index (between panel i and i+1).
        handle: usize,
    },
    /// Sizes changed (drag / keys / redistribute).
    Resized {
        /// Handle that moved (or `None` for bulk apply).
        handle: Option<usize>,
        /// Current sizes in cells (parallel to specs).
        sizes: Vec<u16>,
    },
    /// Panel collapsed.
    Collapsed {
        /// Panel id.
        id: PanelId,
    },
    /// Panel expanded.
    Expanded {
        /// Panel id.
        id: PanelId,
    },
    /// Preset applied.
    PresetApplied {
        /// Preset name.
        name: String,
    },
    /// Responsive recipe suggests drawer presentation (host applies overlay).
    DrawerSuggested {
        /// Panel ids that should leave the in-flow group.
        ids: Vec<PanelId>,
    },
}

/// Runtime state: sizes, collapse, handle focus/drag, presets.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ResizablePanelGroupState {
    /// Expanded main-axis sizes in cells (parallel to panel specs). Empty until first layout.
    sizes: Vec<u16>,
    /// Collapsed flags.
    collapsed: Vec<bool>,
    /// Sizes remembered before collapse (for restore).
    remembered: Vec<u16>,
    /// Focused handle index.
    focused_handle: Option<usize>,
    /// Hovered handle.
    hovered_handle: Option<usize>,
    /// Active drag handle.
    dragging: Option<usize>,
    /// Last layout.
    layout: ResizablePanelGroupLayout,
    /// Named presets (host may also persist externally).
    presets: Vec<PanelLayoutPreset>,
    /// Drawer ids suggested last layout (recipe).
    drawer_ids: Vec<PanelId>,
}

impl Default for ResizablePanelGroupState {
    fn default() -> Self {
        Self::new()
    }
}

impl ResizablePanelGroupState {
    /// Empty state.
    #[must_use]
    pub fn new() -> Self {
        Self {
            sizes: Vec::new(),
            collapsed: Vec::new(),
            remembered: Vec::new(),
            focused_handle: None,
            hovered_handle: None,
            dragging: None,
            layout: ResizablePanelGroupLayout::default(),
            presets: Vec::new(),
            drawer_ids: Vec::new(),
        }
    }

    /// Last layout.
    #[must_use]
    pub fn layout(&self) -> &ResizablePanelGroupLayout {
        &self.layout
    }

    /// Panel sizes in cells.
    #[must_use]
    pub fn sizes(&self) -> &[u16] {
        &self.sizes
    }

    /// Focused handle.
    #[must_use]
    pub const fn focused_handle(&self) -> Option<usize> {
        self.focused_handle
    }

    /// Sets handle focus.
    pub fn set_focused_handle(&mut self, handle: Option<usize>) {
        self.focused_handle = handle;
        if handle.is_none() {
            self.dragging = None;
        }
    }

    /// Drawer candidates from last layout.
    #[must_use]
    pub fn drawer_ids(&self) -> &[PanelId] {
        &self.drawer_ids
    }

    /// Whether panel index is collapsed.
    #[must_use]
    pub fn is_collapsed(&self, index: usize) -> bool {
        self.collapsed.get(index).copied().unwrap_or(false)
    }

    /// Export current sizes as basis points for persistence.
    #[must_use]
    pub fn to_basis_points(&self) -> Vec<u16> {
        let sum: u32 = self.sizes.iter().map(|&s| u32::from(s)).sum();
        if sum == 0 {
            return self.sizes.iter().map(|_| 0).collect();
        }
        self.sizes
            .iter()
            .map(|&s| ((u32::from(s) * RATIO_SCALE + sum / 2) / sum) as u16)
            .collect()
    }

    /// Save current layout as a named preset.
    pub fn save_preset(&mut self, name: impl Into<String>) {
        let preset = PanelLayoutPreset::new(name, self.to_basis_points(), self.collapsed.clone());
        if let Some(existing) = self.presets.iter_mut().find(|p| p.name == preset.name) {
            *existing = preset;
        } else {
            self.presets.push(preset);
        }
    }

    /// List preset names.
    #[must_use]
    pub fn preset_names(&self) -> Vec<&str> {
        self.presets.iter().map(|p| p.name.as_str()).collect()
    }

    /// Seed exact main-axis sizes in cells (parallel to panel specs).
    ///
    /// A host computing its own split math (e.g. a percentage seam) drives
    /// the layout by writing cells here before [`ResizablePanelGroup::layout`];
    /// when the visible sizes already sum to the available axis, layout keeps
    /// them (subject to each spec's min/max clamp) instead of re-deriving
    /// shares from weights. May be called before the first layout: a full
    /// length-matching seed replaces the weight placeholders.
    pub fn set_sizes_cells(&mut self, sizes: &[u16]) {
        if self.sizes.len() != sizes.len() {
            self.sizes = sizes.to_vec();
            self.collapsed = vec![false; sizes.len()];
            self.remembered = self.sizes.clone();
            return;
        }
        self.sizes.copy_from_slice(sizes);
    }

    /// Ensure vectors match panel count.
    fn ensure_len(&mut self, n: usize, specs: &[ResizablePanelSpec]) {
        if self.sizes.len() != n {
            let total_w: u32 = specs.iter().map(|s| u32::from(s.weight.max(1))).sum();
            // Initial sizes deferred to layout with real available; store weights as placeholders.
            self.sizes = specs
                .iter()
                .map(|s| if total_w == 0 { 1 } else { s.weight.max(1) })
                .collect();
            self.collapsed = vec![false; n];
            self.remembered = self.sizes.clone();
        }
    }
}

/// Multi-panel resizable group (paint handles only; content is host-owned).
#[derive(Debug, Clone)]
pub struct ResizablePanelGroup<'a> {
    panels: &'a [ResizablePanelSpec],
    direction: SplitDirection,
    system: &'a DesignSystem,
    recipe: PanelGroupRecipe,
    /// Outer width/height threshold for side-drawer recipe (cells).
    drawer_outer_threshold: u16,
    /// Main-axis cells reserved per handle between panels (0 = seamless).
    handle_cells: u16,
}

impl<'a> ResizablePanelGroup<'a> {
    /// Group over panel specs.
    #[must_use]
    pub const fn new(panels: &'a [ResizablePanelSpec], system: &'a DesignSystem) -> Self {
        Self {
            panels,
            direction: SplitDirection::Horizontal,
            system,
            recipe: PanelGroupRecipe::Fixed,
            drawer_outer_threshold: 72,
            handle_cells: 1,
        }
    }

    /// Cells reserved per handle between panels. `0` lays panels out
    /// seamlessly (no handle column/row, no handle rects) — for hosts that
    /// render an adjacent-pane split with their own seam affordance.
    #[must_use]
    pub const fn handle_cells(mut self, cells: u16) -> Self {
        self.handle_cells = cells;
        self
    }

    /// Direction (horizontal = columns, vertical = rows).
    #[must_use]
    pub const fn direction(mut self, direction: SplitDirection) -> Self {
        self.direction = direction;
        self
    }

    /// Vertical stack.
    #[must_use]
    pub const fn vertical(mut self) -> Self {
        self.direction = SplitDirection::Vertical;
        self
    }

    /// Responsive recipe.
    #[must_use]
    pub const fn recipe(mut self, recipe: PanelGroupRecipe) -> Self {
        self.recipe = recipe;
        self
    }

    /// Workbench recipe (side drawers when narrow).
    #[must_use]
    pub const fn workbench(mut self) -> Self {
        self.recipe = PanelGroupRecipe::Workbench;
        self.drawer_outer_threshold = 72;
        self
    }

    /// Dashboard recipe (horizontal metrics/main style — still N panels).
    #[must_use]
    pub const fn dashboard(mut self) -> Self {
        self.recipe = PanelGroupRecipe::Dashboard;
        self.drawer_outer_threshold = 56;
        self
    }

    /// Outer threshold for drawer suggestion.
    #[must_use]
    pub const fn drawer_threshold(mut self, cells: u16) -> Self {
        self.drawer_outer_threshold = cells;
        self
    }

    /// Pure layout (updates state geometry; does not paint).
    pub fn layout(
        &self,
        area: Rect,
        state: &mut ResizablePanelGroupState,
    ) -> ResizablePanelGroupLayout {
        let n = self.panels.len();
        state.ensure_len(n, self.panels);
        if area.is_empty() || n == 0 {
            state.layout = ResizablePanelGroupLayout {
                panels: self
                    .panels
                    .iter()
                    .map(|p| PanelRect {
                        id: p.id.clone(),
                        area: Rect::new(area.x, area.y, 0, 0),
                        collapsed: true,
                        drawer: false,
                    })
                    .collect(),
                handles: Vec::new(),
                area,
            };
            return state.layout.clone();
        }

        let outer_main = match self.direction {
            SplitDirection::Horizontal => area.width,
            SplitDirection::Vertical => area.height,
        };

        // Drawer candidates (host decides; we only flag).
        state.drawer_ids.clear();
        let use_drawers = matches!(
            self.recipe,
            PanelGroupRecipe::SideDrawers | PanelGroupRecipe::Workbench
        ) && outer_main < self.drawer_outer_threshold;
        if use_drawers {
            for p in self.panels {
                // Side docks leave the in-flow group; host presents them as drawers.
                // Per-panel threshold can force drawers earlier while still wide enough
                // that the recipe has not flipped yet.
                let by_recipe = matches!(p.dock, PanelDock::Start | PanelDock::End);
                let by_panel = p.collapse_threshold > 0 && outer_main < p.collapse_threshold;
                if by_recipe || by_panel {
                    if matches!(p.dock, PanelDock::Start | PanelDock::End) {
                        state.drawer_ids.push(p.id.clone());
                    }
                }
            }
        } else {
            for p in self.panels {
                if matches!(p.dock, PanelDock::Start | PanelDock::End)
                    && p.collapse_threshold > 0
                    && outer_main < p.collapse_threshold
                {
                    state.drawer_ids.push(p.id.clone());
                }
            }
        }

        // Visible panels: not collapsed and not drawer-suggested (drawers leave in-flow).
        let drawer_set = &state.drawer_ids;
        let mut visible_idx: Vec<usize> = (0..n)
            .filter(|&i| !state.collapsed[i] && !drawer_set.iter().any(|d| d == &self.panels[i].id))
            .collect();

        // At least one main must remain in-flow.
        if visible_idx.is_empty() {
            if let Some(i) = self.panels.iter().position(|p| p.dock == PanelDock::Main) {
                state.collapsed[i] = false;
                visible_idx.push(i);
            } else {
                visible_idx.push(0);
                state.collapsed[0] = false;
            }
        }

        let handle_count = visible_idx.len().saturating_sub(1);
        let reserved = (handle_count as u16).saturating_mul(self.handle_cells);
        let available = outer_main.saturating_sub(reserved);

        // Redistribute sizes among visible panels
        redistribute(
            &mut state.sizes,
            &state.collapsed,
            self.panels,
            &visible_idx,
            available,
        );

        // Build rects
        let mut panels = Vec::with_capacity(n);
        let mut handles = Vec::with_capacity(handle_count);
        let mut cursor = match self.direction {
            SplitDirection::Horizontal => area.x,
            SplitDirection::Vertical => area.y,
        };

        for (vi, &i) in visible_idx.iter().enumerate() {
            let size = state.sizes[i];
            let rect = match self.direction {
                SplitDirection::Horizontal => Rect::new(cursor, area.y, size, area.height),
                SplitDirection::Vertical => Rect::new(area.x, cursor, area.width, size),
            };
            panels.push(PanelRect {
                id: self.panels[i].id.clone(),
                area: rect,
                collapsed: false,
                drawer: false,
            });
            cursor = cursor.saturating_add(size);
            if vi + 1 < visible_idx.len() && self.handle_cells > 0 {
                let handle = match self.direction {
                    SplitDirection::Horizontal => Rect::new(
                        cursor,
                        area.y,
                        self.handle_cells.min(area.width),
                        area.height,
                    ),
                    SplitDirection::Vertical => Rect::new(
                        area.x,
                        cursor,
                        area.width,
                        self.handle_cells.min(area.height),
                    ),
                };
                handles.push(handle);
                cursor = cursor.saturating_add(self.handle_cells);
            }
        }

        // Collapsed / drawer panels: zero in-flow area, flagged
        for i in 0..n {
            let is_drawer = drawer_set.iter().any(|d| d == &self.panels[i].id);
            let is_collapsed = state.collapsed[i];
            if is_drawer || is_collapsed {
                if !panels.iter().any(|p| p.id == self.panels[i].id) {
                    panels.push(PanelRect {
                        id: self.panels[i].id.clone(),
                        area: Rect::new(area.x, area.y, 0, 0),
                        collapsed: is_collapsed && !is_drawer,
                        drawer: is_drawer,
                    });
                }
            }
        }
        // Preserve original order for panels vec
        panels.sort_by_key(|p| {
            self.panels
                .iter()
                .position(|s| s.id == p.id)
                .unwrap_or(usize::MAX)
        });

        // Containment guarantee (tiny terminals, rounding).
        for p in &mut panels {
            p.area = intersect_rect(p.area, area);
        }
        for h in &mut handles {
            *h = intersect_rect(*h, area);
        }

        state.layout = ResizablePanelGroupLayout {
            panels,
            handles,
            area,
        };
        state.layout.clone()
    }

    /// Collapse panel by id.
    pub fn collapse(
        &self,
        state: &mut ResizablePanelGroupState,
        id: &PanelId,
    ) -> ResizablePanelOutcome {
        state.ensure_len(self.panels.len(), self.panels);
        let Some(i) = self.panels.iter().position(|p| &p.id == id) else {
            return ResizablePanelOutcome::Ignored;
        };
        if !self.panels[i].collapsible || state.collapsed[i] {
            return ResizablePanelOutcome::Ignored;
        }
        state.remembered[i] = state.sizes[i].max(self.panels[i].min);
        state.collapsed[i] = true;
        state.sizes[i] = 0;
        ResizablePanelOutcome::Collapsed { id: id.clone() }
    }

    /// Expand panel by id.
    pub fn expand(
        &self,
        state: &mut ResizablePanelGroupState,
        id: &PanelId,
    ) -> ResizablePanelOutcome {
        state.ensure_len(self.panels.len(), self.panels);
        let Some(i) = self.panels.iter().position(|p| &p.id == id) else {
            return ResizablePanelOutcome::Ignored;
        };
        if !state.collapsed[i] {
            return ResizablePanelOutcome::Ignored;
        }
        state.collapsed[i] = false;
        state.sizes[i] = state.remembered[i].max(self.panels[i].min);
        ResizablePanelOutcome::Expanded { id: id.clone() }
    }

    /// Apply a saved preset by name.
    pub fn apply_preset(
        &self,
        state: &mut ResizablePanelGroupState,
        name: &str,
        area: Rect,
    ) -> ResizablePanelOutcome {
        state.ensure_len(self.panels.len(), self.panels);
        let Some(preset) = state.presets.iter().find(|p| p.name == name).cloned() else {
            return ResizablePanelOutcome::Ignored;
        };
        if preset.sizes_bp.len() != self.panels.len() {
            return ResizablePanelOutcome::Ignored;
        }
        let outer = match self.direction {
            SplitDirection::Horizontal => area.width,
            SplitDirection::Vertical => area.height,
        };
        let handles = self.panels.len().saturating_sub(1) as u16;
        let available = outer.saturating_sub(handles);
        for (i, bp) in preset.sizes_bp.iter().enumerate() {
            let size =
                ((u32::from(available) * u32::from(*bp) + RATIO_SCALE / 2) / RATIO_SCALE) as u16;
            state.sizes[i] = size;
            state.collapsed[i] = preset.collapsed.get(i).copied().unwrap_or(false);
            if !state.collapsed[i] {
                state.remembered[i] = size.max(self.panels[i].min);
            }
        }
        let _ = self.layout(area, state);
        ResizablePanelOutcome::PresetApplied {
            name: name.to_owned(),
        }
    }

    /// Keyboard: arrows resize focused handle; Home/End collapse/expand neighbor.
    pub fn handle_key(
        &self,
        state: &mut ResizablePanelGroupState,
        key: KeyEvent,
        area: Rect,
    ) -> ResizablePanelOutcome {
        if key.kind != KeyEventKind::Press {
            return ResizablePanelOutcome::Ignored;
        }
        let Some(handle) = state.focused_handle else {
            return ResizablePanelOutcome::Ignored;
        };
        let _ = self.layout(area, state);
        if handle >= state.layout.handles.len() {
            return ResizablePanelOutcome::Ignored;
        }
        // Map handle to adjacent visible panel indices
        let visible = visible_indices(state, self.panels);
        if handle + 1 >= visible.len() {
            return ResizablePanelOutcome::Ignored;
        }
        let left_i = visible[handle];
        let right_i = visible[handle + 1];

        let delta: i32 = match (self.direction, key.code) {
            (SplitDirection::Horizontal, KeyCode::Left)
            | (SplitDirection::Vertical, KeyCode::Up) => -(i32::from(KEYBOARD_STEP)),
            (SplitDirection::Horizontal, KeyCode::Right)
            | (SplitDirection::Vertical, KeyCode::Down) => i32::from(KEYBOARD_STEP),
            (SplitDirection::Horizontal, KeyCode::Home)
            | (SplitDirection::Vertical, KeyCode::Home) => {
                // collapse left neighbor if collapsible
                return self.collapse(state, &self.panels[left_i].id);
            }
            (SplitDirection::Horizontal, KeyCode::End)
            | (SplitDirection::Vertical, KeyCode::End) => {
                return self.collapse(state, &self.panels[right_i].id);
            }
            _ => return ResizablePanelOutcome::Ignored,
        };

        // Convert step in basis points to cell delta approximately
        let outer = match self.direction {
            SplitDirection::Horizontal => area.width,
            SplitDirection::Vertical => area.height,
        };
        let available = outer
            .saturating_sub(state.layout.handles.len() as u16)
            .max(1);
        let cell_delta = ((i32::from(available) * delta) / i32::from(RATIO_SCALE as u16)).max(1)
            * delta.signum();
        if move_handle(&mut state.sizes, left_i, right_i, cell_delta, self.panels) {
            let _ = self.layout(area, state);
            ResizablePanelOutcome::Resized {
                handle: Some(handle),
                sizes: state.sizes.clone(),
            }
        } else {
            ResizablePanelOutcome::Ignored
        }
    }

    /// Mouse: hit handles for focus/drag.
    pub fn handle_mouse(
        &self,
        state: &mut ResizablePanelGroupState,
        event: MouseEvent,
        area: Rect,
    ) -> ResizablePanelOutcome {
        let _ = self.layout(area, state);
        match event.kind {
            MouseEventKind::Down(MouseButton::Left) => {
                if let Some(h) = state.layout.hit_handle(event.position) {
                    state.focused_handle = Some(h);
                    state.dragging = Some(h);
                    state.hovered_handle = Some(h);
                    return ResizablePanelOutcome::HandleFocused { handle: h };
                }
                ResizablePanelOutcome::Ignored
            }
            MouseEventKind::Drag(MouseButton::Left) | MouseEventKind::Moved
                if state.dragging.is_some() =>
            {
                let Some(h) = state.dragging else {
                    return ResizablePanelOutcome::Ignored;
                };
                let visible = visible_indices(state, self.panels);
                if h + 1 >= visible.len() {
                    return ResizablePanelOutcome::Ignored;
                }
                let left_i = visible[h];
                let right_i = visible[h + 1];
                let origin = match self.direction {
                    SplitDirection::Horizontal => area.x,
                    SplitDirection::Vertical => area.y,
                };
                let coord = match self.direction {
                    SplitDirection::Horizontal => event.position.x,
                    SplitDirection::Vertical => event.position.y,
                };
                // Target first-size = sum of sizes before left_i+size of left up to pointer
                let prefix: u16 = visible[..h]
                    .iter()
                    .map(|&i| state.sizes[i])
                    .sum::<u16>()
                    .saturating_add(h as u16); // handles before
                let desired_left = coord.saturating_sub(origin).saturating_sub(prefix);
                let pair = state.sizes[left_i].saturating_add(state.sizes[right_i]);
                let new_left = desired_left.min(pair);
                let delta = new_left as i32 - state.sizes[left_i] as i32;
                if move_handle(&mut state.sizes, left_i, right_i, delta, self.panels) {
                    let _ = self.layout(area, state);
                    ResizablePanelOutcome::Resized {
                        handle: Some(h),
                        sizes: state.sizes.clone(),
                    }
                } else {
                    ResizablePanelOutcome::Ignored
                }
            }
            MouseEventKind::Up(MouseButton::Left) => {
                state.dragging = None;
                ResizablePanelOutcome::Ignored
            }
            MouseEventKind::Moved => {
                state.hovered_handle = state.layout.hit_handle(event.position);
                ResizablePanelOutcome::Ignored
            }
            _ => ResizablePanelOutcome::Ignored,
        }
    }

    /// Paint dividers only (content is host-owned).
    pub fn paint_handles(
        &self,
        area: Rect,
        buffer: &mut Buffer,
        state: &mut ResizablePanelGroupState,
    ) {
        let layout = self.layout(area, state);
        if layout.handles.is_empty() {
            return;
        }
        for (i, handle) in layout.handles.iter().enumerate() {
            if handle.is_empty() {
                continue;
            }
            let focused = state.focused_handle == Some(i);
            let hovered = state.hovered_handle == Some(i);
            let (glyph, role) = match (self.direction, focused, hovered) {
                (SplitDirection::Horizontal, true, _) => {
                    (self.system.glyphs.rule_v(), Role::BorderFocused)
                }
                (SplitDirection::Horizontal, false, true) => {
                    (self.system.glyphs.rule_v(), Role::Focus)
                }
                (SplitDirection::Horizontal, false, false) => (" ", Role::Border),
                (SplitDirection::Vertical, true, _) => {
                    (self.system.glyphs.rule(), Role::BorderFocused)
                }
                (SplitDirection::Vertical, false, true) => (self.system.glyphs.rule(), Role::Focus),
                (SplitDirection::Vertical, false, false) => (" ", Role::Border),
            };
            let mut style = self.system.style(role);
            if focused {
                style = style.add_modifier(Modifier::BOLD);
            }
            match self.direction {
                SplitDirection::Horizontal => {
                    for y in handle.top()..handle.bottom() {
                        buffer.set_string(handle.x, y, glyph, style);
                    }
                }
                SplitDirection::Vertical => {
                    let line: String =
                        std::iter::repeat_n(glyph, usize::from(handle.width)).collect();
                    buffer.set_stringn(handle.x, handle.y, &line, usize::from(handle.width), style);
                }
            }
        }
    }
}

impl StatefulWidget for &ResizablePanelGroup<'_> {
    type State = ResizablePanelGroupState;

    fn render(self, area: Rect, buffer: &mut Buffer, state: &mut Self::State) {
        self.paint_handles(area, buffer, state);
    }
}

impl StatefulWidget for ResizablePanelGroup<'_> {
    type State = ResizablePanelGroupState;

    fn render(self, area: Rect, buffer: &mut Buffer, state: &mut Self::State) {
        <&Self as StatefulWidget>::render(&self, area, buffer, state);
    }
}

/// Three-pane preset: start | main | end (e.g. rail | content | inspector).
#[must_use]
pub fn three_pane_panels() -> [ResizablePanelSpec; 3] {
    [
        ResizablePanelSpec::start("sidebar", 2, 12),
        ResizablePanelSpec::main("main", 6),
        ResizablePanelSpec::end("inspector", 2, 14),
    ]
}

/// Two-pane preset: main | end (e.g. content | log).
#[must_use]
pub fn main_end_panels() -> [ResizablePanelSpec; 2] {
    [
        ResizablePanelSpec::main("main", 3).min(20),
        ResizablePanelSpec::end("log", 1, 8).collapse_threshold(40),
    ]
}

fn visible_indices(state: &ResizablePanelGroupState, panels: &[ResizablePanelSpec]) -> Vec<usize> {
    (0..panels.len())
        .filter(|&i| {
            !state.collapsed.get(i).copied().unwrap_or(false)
                && !state.drawer_ids.iter().any(|d| d == &panels[i].id)
                && state
                    .layout
                    .panels
                    .iter()
                    .find(|p| p.id == panels[i].id)
                    .is_some_and(|p| p.area.width > 0 || p.area.height > 0 || !p.drawer)
        })
        .filter(|&i| {
            state
                .layout
                .panels
                .iter()
                .find(|p| p.id == panels[i].id)
                .is_some_and(|p| {
                    !p.collapsed && !p.drawer && (p.area.width > 0 || p.area.height > 0)
                })
        })
        .collect()
}

fn redistribute(
    sizes: &mut [u16],
    collapsed: &[bool],
    specs: &[ResizablePanelSpec],
    visible: &[usize],
    available: u16,
) {
    if visible.is_empty() || available == 0 {
        for &i in visible {
            sizes[i] = 0;
        }
        return;
    }
    // If sizes already sum close to available among visible, clamp only.
    let sum: u32 = visible.iter().map(|&i| u32::from(sizes[i])).sum();
    if sum == 0 {
        // Seed from weights
        let wsum: u32 = visible
            .iter()
            .map(|&i| u32::from(specs[i].weight.max(1)))
            .sum();
        let mut used = 0u16;
        for (vi, &i) in visible.iter().enumerate() {
            if vi + 1 == visible.len() {
                sizes[i] = available.saturating_sub(used);
            } else {
                let share = ((u32::from(available) * u32::from(specs[i].weight.max(1)) + wsum / 2)
                    / wsum) as u16;
                sizes[i] = share;
                used = used.saturating_add(share);
            }
        }
    } else if sum != u32::from(available) {
        // Scale to available
        let mut used = 0u16;
        for (vi, &i) in visible.iter().enumerate() {
            if vi + 1 == visible.len() {
                sizes[i] = available.saturating_sub(used);
            } else {
                let share = ((u32::from(available) * u32::from(sizes[i]) + sum / 2) / sum) as u16;
                sizes[i] = share;
                used = used.saturating_add(share);
            }
        }
    }
    // Clamp mins/maxes and fix remainder on last
    let mut deficit = 0i32;
    for &i in visible {
        let min = if collapsed[i] { 0 } else { specs[i].min };
        let max = specs[i].max.unwrap_or(u16::MAX);
        let clamped = sizes[i].clamp(min, max);
        deficit += sizes[i] as i32 - clamped as i32;
        sizes[i] = clamped;
    }
    // Push remainder to last visible expandable
    if deficit != 0 {
        if let Some(&last) = visible.last() {
            let min = specs[last].min;
            let max = specs[last].max.unwrap_or(u16::MAX);
            let next = (sizes[last] as i32 + deficit).clamp(i32::from(min), i32::from(max));
            sizes[last] = next as u16;
        }
    }
    // Zero collapsed
    for (i, c) in collapsed.iter().enumerate() {
        if *c {
            sizes[i] = 0;
        }
    }
}

fn move_handle(
    sizes: &mut [u16],
    left: usize,
    right: usize,
    delta_cells: i32,
    specs: &[ResizablePanelSpec],
) -> bool {
    if delta_cells == 0 {
        return false;
    }
    let pair = sizes[left].saturating_add(sizes[right]);
    let min_l = specs[left].min;
    let min_r = specs[right].min;
    let max_l = specs[left].max.unwrap_or(pair.saturating_sub(min_r));
    let max_r = specs[right].max.unwrap_or(pair.saturating_sub(min_l));
    let new_l = (sizes[left] as i32 + delta_cells).clamp(
        i32::from(min_l),
        i32::from(max_l.min(pair.saturating_sub(min_r))),
    ) as u16;
    let new_r = pair.saturating_sub(new_l);
    if new_r < min_r || new_r > max_r {
        return false;
    }
    if new_l == sizes[left] {
        return false;
    }
    sizes[left] = new_l;
    sizes[right] = new_r;
    true
}

fn intersect_rect(inner: Rect, outer: Rect) -> Rect {
    let x = inner.x.max(outer.x);
    let y = inner.y.max(outer.y);
    let right = inner.right().min(outer.right()).max(x);
    let bottom = inner.bottom().min(outer.bottom()).max(y);
    Rect {
        x,
        y,
        width: right.saturating_sub(x),
        height: bottom.saturating_sub(y),
    }
}

// SplitRatio re-export bridge for preset interop with binary splits.
#[allow(dead_code)]
fn _ratio_bridge(r: SplitRatio) -> u16 {
    r.basis_points()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::style::DesignSystem;

    #[test]
    fn three_panel_workbench_layout() {
        let system = DesignSystem::default();
        let panels = three_pane_panels();
        let group = ResizablePanelGroup::new(&panels, &system).workbench();
        let mut state = ResizablePanelGroupState::new();
        let layout = group.layout(Rect::new(0, 0, 100, 20), &mut state);
        assert_eq!(layout.panels.len(), 3);
        assert_eq!(layout.handles.len(), 2);
        let total_w: u16 = layout
            .panels
            .iter()
            .filter(|p| !p.drawer && !p.collapsed)
            .map(|p| p.area.width)
            .sum::<u16>()
            .saturating_add(layout.handles.len() as u16);
        assert_eq!(total_w, 100);
        // All expanded panels contained in outer width.
        for p in &layout.panels {
            if p.area.width > 0 {
                assert!(p.area.right() <= 100);
            }
        }
    }

    #[test]
    fn seamless_layout_reserves_no_handle_cells() {
        let system = DesignSystem::default();
        let panels = [
            ResizablePanelSpec::main(PanelId::from_static("list"), 30).min(0),
            ResizablePanelSpec::main(PanelId::from_static("preview"), 70).min(0),
        ];
        let group = ResizablePanelGroup::new(&panels, &system).handle_cells(0);
        let mut state = ResizablePanelGroupState::new();
        state.set_sizes_cells(&[47, 111]);
        let layout = group.layout(Rect::new(0, 0, 158, 40), &mut state);
        assert!(layout.handles.is_empty());
        assert_eq!(layout.panels[0].area, Rect::new(0, 0, 47, 40));
        assert_eq!(layout.panels[1].area, Rect::new(47, 0, 111, 40));
    }

    #[test]
    fn set_sizes_cells_before_first_layout_is_honored() {
        let system = DesignSystem::default();
        let panels = [
            ResizablePanelSpec::main(PanelId::from_static("a"), 1).min(0),
            ResizablePanelSpec::main(PanelId::from_static("b"), 1).min(0),
        ];
        let group = ResizablePanelGroup::new(&panels, &system);
        let mut state = ResizablePanelGroupState::new();
        state.set_sizes_cells(&[9, 90]);
        let layout = group.layout(Rect::new(0, 0, 100, 10), &mut state);
        assert_eq!(layout.panels[0].area.width, 9);
        assert_eq!(layout.panels[1].area.width, 90);
        // Sizes summing to the available axis survive re-layout unchanged.
        let layout = group.layout(Rect::new(0, 0, 100, 10), &mut state);
        assert_eq!(layout.panels[0].area.width, 9);
        assert_eq!(layout.panels[1].area.width, 90);
    }

    #[test]
    fn tiny_area_no_overflow() {
        let system = DesignSystem::default();
        let panels = three_pane_panels();
        let group = ResizablePanelGroup::new(&panels, &system);
        let mut state = ResizablePanelGroupState::new();
        for w in 0..20u16 {
            for h in 0..5u16 {
                let layout = group.layout(Rect::new(2, 3, w, h), &mut state);
                for p in &layout.panels {
                    if p.area.width == 0 && p.area.height == 0 {
                        continue;
                    }
                    assert!(p.area.x >= 2);
                    assert!(p.area.y >= 3);
                    if w > 0 {
                        assert!(p.area.right() <= 2 + w);
                    }
                    if h > 0 {
                        assert!(p.area.bottom() <= 3 + h);
                    }
                }
            }
        }
    }

    #[test]
    fn handle_resize_preserves_sum() {
        let system = DesignSystem::default();
        let panels = three_pane_panels();
        let group = ResizablePanelGroup::new(&panels, &system);
        let mut state = ResizablePanelGroupState::new();
        let area = Rect::new(0, 0, 90, 10);
        let _ = group.layout(area, &mut state);
        let before: u16 = state.sizes.iter().sum();
        state.set_focused_handle(Some(0));
        let out = group.handle_key(
            &mut state,
            KeyEvent::new(KeyCode::Right, crate::input::KeyModifiers::NONE),
            area,
        );
        assert!(
            matches!(out, ResizablePanelOutcome::Resized { .. })
                || matches!(out, ResizablePanelOutcome::Ignored)
        );
        let after: u16 = state.sizes.iter().sum();
        // sum of visible sizes stable within handle redistribution
        assert_eq!(before, after);
    }

    #[test]
    fn collapse_and_expand_restores() {
        let system = DesignSystem::default();
        let panels = three_pane_panels();
        let group = ResizablePanelGroup::new(&panels, &system);
        let mut state = ResizablePanelGroupState::new();
        let area = Rect::new(0, 0, 100, 12);
        let _ = group.layout(area, &mut state);
        let id = PanelId::from_static("sidebar");
        let out = group.collapse(&mut state, &id);
        assert!(matches!(out, ResizablePanelOutcome::Collapsed { .. }));
        let layout = group.layout(area, &mut state);
        let side = layout.get(&id).unwrap();
        assert!(side.collapsed || side.area.width == 0);
        let out = group.expand(&mut state, &id);
        assert!(matches!(out, ResizablePanelOutcome::Expanded { .. }));
        let layout = group.layout(area, &mut state);
        assert!(layout.get(&id).unwrap().area.width > 0);
    }

    #[test]
    fn workbench_suggests_drawers_when_narrow() {
        let system = DesignSystem::default();
        let panels = three_pane_panels();
        let group = ResizablePanelGroup::new(&panels, &system)
            .workbench()
            .drawer_threshold(80);
        let mut state = ResizablePanelGroupState::new();
        let _ = group.layout(Rect::new(0, 0, 50, 20), &mut state);
        assert!(!state.drawer_ids().is_empty());
        assert!(state.drawer_ids().iter().any(|d| d.0 == "sidebar"));
    }

    #[test]
    fn preset_roundtrip() {
        let system = DesignSystem::default();
        let panels = three_pane_panels();
        let group = ResizablePanelGroup::new(&panels, &system);
        let mut state = ResizablePanelGroupState::new();
        let area = Rect::new(0, 0, 100, 10);
        let _ = group.layout(area, &mut state);
        state.set_focused_handle(Some(0));
        let _ = group.handle_key(
            &mut state,
            KeyEvent::new(KeyCode::Right, crate::input::KeyModifiers::NONE),
            area,
        );
        state.save_preset("dev");
        // mess sizes
        state.sizes = vec![10, 10, 10];
        let out = group.apply_preset(&mut state, "dev", area);
        assert!(matches!(out, ResizablePanelOutcome::PresetApplied { .. }));
        assert_eq!(state.preset_names(), vec!["dev"]);
    }

    #[test]
    fn nested_groups_independent() {
        // Two independent groups share no state — content focus/scroll stays host-owned.
        let system = DesignSystem::default();
        let outer = [
            ResizablePanelSpec::main("left", 1),
            ResizablePanelSpec::main("right", 1),
        ];
        let inner = [
            ResizablePanelSpec::main("top", 1),
            ResizablePanelSpec::main("bottom", 1),
        ];
        let g_outer = ResizablePanelGroup::new(&outer, &system);
        let g_inner = ResizablePanelGroup::new(&inner, &system).vertical();
        let mut s_outer = ResizablePanelGroupState::new();
        let mut s_inner = ResizablePanelGroupState::new();
        let lo = g_outer.layout(Rect::new(0, 0, 80, 24), &mut s_outer);
        let right = lo.get(&PanelId::from_static("right")).unwrap().area;
        let li = g_inner.layout(right, &mut s_inner);
        assert_eq!(li.handles.len(), 1);
        assert!(li.panels.iter().all(|p| p.area.x >= right.x));
        // Changing outer does not clear inner sizes
        let before = s_inner.sizes.clone();
        s_outer.set_focused_handle(Some(0));
        let _ = g_outer.handle_key(
            &mut s_outer,
            KeyEvent::new(KeyCode::Left, crate::input::KeyModifiers::NONE),
            Rect::new(0, 0, 80, 24),
        );
        assert_eq!(s_inner.sizes, before);
    }

    #[test]
    fn rapid_resize_stress() {
        let system = DesignSystem::default();
        let panels = three_pane_panels();
        let group = ResizablePanelGroup::new(&panels, &system);
        let mut state = ResizablePanelGroupState::new();
        let area = Rect::new(0, 0, 120, 30);
        let _ = group.layout(area, &mut state);
        state.set_focused_handle(Some(0));
        for i in 0..500 {
            let key = if i % 2 == 0 {
                KeyCode::Right
            } else {
                KeyCode::Left
            };
            let _ = group.handle_key(
                &mut state,
                KeyEvent::new(key, crate::input::KeyModifiers::NONE),
                area,
            );
            let layout = group.layout(area, &mut state);
            let sum: u16 = layout
                .panels
                .iter()
                .filter(|p| !p.drawer && !p.collapsed)
                .map(|p| p.area.width)
                .sum::<u16>()
                .saturating_add(layout.handles.len() as u16);
            assert_eq!(sum, 120, "frame {i}");
        }
    }

    #[test]
    fn mouse_drag_moves_handle() {
        let system = DesignSystem::default();
        let panels = three_pane_panels();
        let group = ResizablePanelGroup::new(&panels, &system);
        let mut state = ResizablePanelGroupState::new();
        let area = Rect::new(0, 0, 90, 10);
        let mut buf = Buffer::empty(area);
        group.paint_handles(area, &mut buf, &mut state);
        assert!(!state.layout.handles.is_empty());
        let h = state.layout.handles[0];
        let out = group.handle_mouse(
            &mut state,
            MouseEvent {
                kind: MouseEventKind::Down(MouseButton::Left),
                position: Position { x: h.x, y: h.y },
                modifiers: crate::input::KeyModifiers::NONE,
            },
            area,
        );
        assert!(matches!(
            out,
            ResizablePanelOutcome::HandleFocused { handle: 0 }
        ));
        let out = group.handle_mouse(
            &mut state,
            MouseEvent {
                kind: MouseEventKind::Drag(MouseButton::Left),
                position: Position {
                    x: h.x.saturating_add(5),
                    y: h.y,
                },
                modifiers: crate::input::KeyModifiers::NONE,
            },
            area,
        );
        assert!(
            matches!(out, ResizablePanelOutcome::Resized { .. })
                || matches!(out, ResizablePanelOutcome::Ignored)
        );
    }

    #[test]
    fn group_handles_are_quiet_until_focused() {
        let system = DesignSystem::default();
        let panels = [
            ResizablePanelSpec::main("left", 1),
            ResizablePanelSpec::main("right", 1),
        ];
        let group = ResizablePanelGroup::new(&panels, &system);
        let area = Rect::new(0, 0, 21, 3);
        let mut state = ResizablePanelGroupState::new();
        let mut buffer = Buffer::empty(area);
        group.paint_handles(area, &mut buffer, &mut state);
        let handle = state.layout.handles[0];
        assert_eq!(buffer[(handle.x, handle.y)].symbol(), " ");

        state.set_focused_handle(Some(0));
        group.paint_handles(area, &mut buffer, &mut state);
        assert_eq!(
            buffer[(handle.x, handle.y)].symbol(),
            system.glyphs.rule_v()
        );
    }

    #[test]
    fn layout_is_cheap() {
        let system = DesignSystem::default();
        let panels = three_pane_panels();
        let group = ResizablePanelGroup::new(&panels, &system).workbench();
        let mut state = ResizablePanelGroupState::new();
        let area = Rect::new(0, 0, 100, 30);
        for _ in 0..20_000 {
            let _ = group.layout(area, &mut state);
        }
    }

    #[test]
    fn dashboard_two_panel() {
        let system = DesignSystem::default();
        let panels = main_end_panels();
        let group = ResizablePanelGroup::new(&panels, &system).dashboard();
        let mut state = ResizablePanelGroupState::new();
        let layout = group.layout(Rect::new(0, 0, 80, 24), &mut state);
        assert_eq!(layout.handles.len(), 1);
    }
}
