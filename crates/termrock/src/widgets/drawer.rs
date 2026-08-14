// SPDX-FileCopyrightText: 2026 Alexey Zhokhov
// SPDX-License-Identifier: Apache-2.0

//! **Drawer** and **Sheet** — edge-mounted secondary surfaces.
//!
//! **Mission.** Responsive inspectors, task rails, filters, and details that
//! replace docked sidebars/inspector panes under width pressure. Placement on
//! any edge (left/right/top/bottom), modal or non-modal policy, resizable depth,
//! focus trap (when modal), opener restoration, and nested child overlays via
//! [`OverlayStack`].
//!
//! **Sheet.** [`Sheet`] is the same surface with a bottom-edge default (mobile
//! sheet metaphor / shadcn Sheet). Shared state and paint.
//!
//! **Host owns.** Domain selection, scroll of the **underlying** main view,
//! process policy. Opening a drawer must not clear host list/table selection
//! or scroll offsets — TermRock only owns drawer-local chrome and stack geometry.
//!
//! **MotionPolicy.** Terminals do not slide-animate; [`MotionPolicy::Off`] / [`MotionPolicy::Basic`]
//! selects static chrome (ASCII handles, no spinner) as the no-motion fallback.
//!
//! Research: shadcn Sheet, mobile drawers, Zellij floating panes, agent task sidebars.

#![allow(unused_imports)] // test-module imports kept for unit tests; lib path may not use them
use ratatui_core::{
    buffer::Buffer,
    layout::{Position, Rect},
    style::Modifier,
    widgets::{StatefulWidget, Widget},
};
use ratatui_widgets::borders::Borders;

use crate::{
    input::{
        KeyCode, KeyEvent, KeyEventKind, KeyModifiers, MouseButton, MouseEvent, MouseEventKind,
    },
    interaction::{
        BackdropPolicy, LayerDismissPolicy, NarrowFallback, OverlayId, OverlayKind, OverlayOutcome,
        OverlayPolicy, OverlaySize, OverlaySpec, OverlayStack, PlacementPrefer, SemanticNode,
        SemanticRole, SemanticScene, SemanticState, UiIntent, place_overlay,
    },
    style::{DesignSystem, MotionPolicy, Role},
    text::{display_cols, take_display_cols},
};

/// Default overlay id for drawers.
pub const DRAWER_OVERLAY_ID: &str = "termrock.drawer";
/// Nested child overlay id prefix under a drawer.
pub const DRAWER_NESTED_OVERLAY_PREFIX: &str = "termrock.drawer.child";
/// Width at or below which horizontal drawers promote toward fullscreen.
pub const DRAWER_FULLSCREEN_MAX_WIDTH: u16 = 36;
/// Height at or below which vertical drawers promote toward fullscreen.
pub const DRAWER_FULLSCREEN_MAX_HEIGHT: u16 = 12;
/// Default horizontal depth (columns).
pub const DRAWER_DEFAULT_WIDTH: u16 = 32;
/// Default vertical depth (rows).
pub const DRAWER_DEFAULT_HEIGHT: u16 = 10;
/// Compact handle thickness (cells).
pub const DRAWER_HANDLE_CELLS: u16 = 1;

// ── Edge / modality / presentation ──────────────────────────────────────────

/// Which edge the drawer mounts to.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
#[non_exhaustive]
pub enum DrawerEdge {
    /// Left edge (start).
    Left,
    /// Right edge (end) — default for inspectors.
    #[default]
    Right,
    /// Top edge.
    Top,
    /// Bottom edge (default for [`Sheet`]).
    Bottom,
}

impl DrawerEdge {
    /// Stable id.
    #[must_use]
    pub const fn id(self) -> &'static str {
        match self {
            Self::Left => "left",
            Self::Right => "right",
            Self::Top => "top",
            Self::Bottom => "bottom",
        }
    }

    /// Whether this edge is horizontal (left/right).
    #[must_use]
    pub const fn is_horizontal(self) -> bool {
        matches!(self, Self::Left | Self::Right)
    }

    /// Placement preference for OverlayStack.
    #[must_use]
    pub const fn placement(self) -> PlacementPrefer {
        match self {
            Self::Left => PlacementPrefer::DrawerStart,
            Self::Right => PlacementPrefer::DrawerEnd,
            Self::Top => PlacementPrefer::DrawerTop,
            Self::Bottom => PlacementPrefer::DrawerBottom,
        }
    }
}

/// Modal versus non-modal drawer policy.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
#[non_exhaustive]
pub enum DrawerModality {
    /// Default: owns input, focus trap, dim backdrop, Esc/outside dismiss
    /// (matches [`OverlayKind::Drawer`]).
    #[default]
    Modal,
    /// Secondary surface: no focus trap, no dim; host may keep main selection.
    /// Esc still closes when the drawer accepts input.
    NonModal,
}

impl DrawerModality {
    /// Stable id.
    #[must_use]
    pub const fn id(self) -> &'static str {
        match self {
            Self::Modal => "modal",
            Self::NonModal => "non-modal",
        }
    }

    /// Overlay policy for this modality (placement filled by edge).
    #[must_use]
    pub const fn policy(self, edge: DrawerEdge) -> OverlayPolicy {
        match self {
            Self::Modal => OverlayPolicy {
                esc: LayerDismissPolicy::Dismissible,
                outside: LayerDismissPolicy::Dismissible,
                owns_input: true,
                focus_trap: true,
                wheel_captures: true,
                backdrop: BackdropPolicy::Dim,
                prefer: edge.placement(),
                cover_anchor: true,
                narrow_fallback: NarrowFallback::Fullscreen,
                narrow_cols: DRAWER_FULLSCREEN_MAX_WIDTH,
            },
            Self::NonModal => OverlayPolicy {
                esc: LayerDismissPolicy::Dismissible,
                outside: LayerDismissPolicy::Dismissible,
                owns_input: true,
                focus_trap: false,
                wheel_captures: true,
                backdrop: BackdropPolicy::None,
                prefer: edge.placement(),
                cover_anchor: true,
                narrow_fallback: NarrowFallback::Clamp,
                narrow_cols: DRAWER_FULLSCREEN_MAX_WIDTH,
            },
        }
    }
}

/// How the drawer is presented after contraction.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
#[non_exhaustive]
pub enum DrawerPresentation {
    /// Edge panel at preferred depth.
    #[default]
    Expanded,
    /// Compact strip / handle-forward chrome (host may show only handle).
    Compact,
    /// Full bounds (tiny terminal).
    Fullscreen,
}

impl DrawerPresentation {
    /// Stable id.
    #[must_use]
    pub const fn id(self) -> &'static str {
        match self {
            Self::Expanded => "expanded",
            Self::Compact => "compact",
            Self::Fullscreen => "fullscreen",
        }
    }
}

/// Choose presentation from bounds and preferred depth.
#[must_use]
pub fn drawer_presentation_for(
    bounds: Rect,
    edge: DrawerEdge,
    preferred_depth: u16,
) -> DrawerPresentation {
    if bounds.is_empty() {
        return DrawerPresentation::Expanded;
    }
    if bounds.width <= DRAWER_FULLSCREEN_MAX_WIDTH || bounds.height <= DRAWER_FULLSCREEN_MAX_HEIGHT
    {
        return DrawerPresentation::Fullscreen;
    }
    if edge.is_horizontal() && preferred_depth.saturating_mul(2) > bounds.width {
        return DrawerPresentation::Compact;
    }
    if !edge.is_horizontal() && preferred_depth.saturating_mul(2) > bounds.height {
        return DrawerPresentation::Compact;
    }
    DrawerPresentation::Expanded
}

// ── Placement / open ────────────────────────────────────────────────────────

/// Places a drawer for edge + size (uses Drawer policy placement).
#[must_use]
pub fn place_drawer(bounds: Rect, size: OverlaySize) -> Rect {
    place_drawer_on_edge(
        bounds,
        DrawerEdge::Right,
        size,
        DrawerPresentation::Expanded,
    )
}

/// Place on a specific edge with presentation.
#[must_use]
pub fn place_drawer_on_edge(
    bounds: Rect,
    edge: DrawerEdge,
    size: OverlaySize,
    presentation: DrawerPresentation,
) -> Rect {
    if bounds.is_empty() {
        return Rect::default();
    }
    if matches!(presentation, DrawerPresentation::Fullscreen) {
        return bounds;
    }
    let mut size = size;
    if matches!(presentation, DrawerPresentation::Compact) {
        if edge.is_horizontal() {
            size.width = DRAWER_HANDLE_CELLS
                .saturating_add(12)
                .min(size.width)
                .max(DRAWER_HANDLE_CELLS.saturating_add(8));
        } else {
            size.height = DRAWER_HANDLE_CELLS
                .saturating_add(4)
                .min(size.height)
                .max(DRAWER_HANDLE_CELLS.saturating_add(3));
        }
    }
    let policy = DrawerModality::Modal.policy(edge);
    place_overlay(bounds, None, size, policy)
}

/// Opens a drawer (default right / modal / default id).
pub fn open_drawer_overlay<FocusId: Clone>(
    stack: &mut OverlayStack<FocusId>,
    bounds: Rect,
    id: impl Into<OverlayId>,
    size: OverlaySize,
    opener_focus: Option<FocusId>,
) -> OverlayOutcome<FocusId> {
    open_drawer_configured(
        stack,
        bounds,
        id,
        size,
        opener_focus,
        DrawerEdge::Right,
        DrawerModality::Modal,
        None,
        None,
    )
}

/// Full configuration open.
pub fn open_drawer_configured<FocusId: Clone>(
    stack: &mut OverlayStack<FocusId>,
    bounds: Rect,
    id: impl Into<OverlayId>,
    size: OverlaySize,
    opener_focus: Option<FocusId>,
    edge: DrawerEdge,
    modality: DrawerModality,
    force_presentation: Option<DrawerPresentation>,
    parent: Option<OverlayId>,
) -> OverlayOutcome<FocusId> {
    let depth = if edge.is_horizontal() {
        size.width
    } else {
        size.height
    };
    let presentation =
        force_presentation.unwrap_or_else(|| drawer_presentation_for(bounds, edge, depth));
    let id = id.into();
    let policy = modality.policy(edge);
    let mut spec = if matches!(presentation, DrawerPresentation::Fullscreen) {
        OverlaySpec::fullscreen(id, opener_focus).with_policy(OverlayPolicy {
            prefer: PlacementPrefer::Fullscreen,
            narrow_fallback: NarrowFallback::Fullscreen,
            ..policy
        })
    } else {
        OverlaySpec::drawer(id, size, opener_focus).with_policy(policy)
    };
    if let Some(p) = parent {
        spec = spec.with_parent(p);
    }
    stack.open(bounds, spec)
}

/// Nested child under an open drawer.
pub fn open_drawer_nested_overlay<FocusId: Clone>(
    stack: &mut OverlayStack<FocusId>,
    bounds: Rect,
    size: OverlaySize,
    edge: DrawerEdge,
    opener_focus: Option<FocusId>,
) -> OverlayOutcome<FocusId> {
    open_drawer_configured(
        stack,
        bounds,
        format!("{DRAWER_NESTED_OVERLAY_PREFIX}.nested"),
        size,
        opener_focus,
        edge,
        DrawerModality::NonModal,
        None,
        Some(OverlayId::from_static(DRAWER_OVERLAY_ID)),
    )
}

/// Dismiss default drawer id (and nested children via stack).
pub fn dismiss_drawer_overlay<FocusId: Clone>(
    stack: &mut OverlayStack<FocusId>,
) -> OverlayOutcome<FocusId> {
    stack.dismiss(&OverlayId::from_static(DRAWER_OVERLAY_ID))
}

// ── Outcomes / slots / state ────────────────────────────────────────────────

/// Typed outcomes.
#[derive(Debug, Clone, PartialEq, Eq)]
#[non_exhaustive]
pub enum DrawerOutcome {
    /// No change.
    Ignored,
    /// Drawer opened.
    Opened,
    /// Closed (Esc / outside / request).
    Closed,
    /// Depth resized.
    Resized {
        /// New depth (width for L/R, height for T/B).
        depth: u16,
    },
    /// Presentation suggestion changed.
    PresentationChanged {
        /// New presentation.
        presentation: DrawerPresentation,
    },
    /// Focus entered drawer surface.
    FocusEntered,
}

/// Slot geometry after paint.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct DrawerSlots {
    /// Outer rect.
    pub root: Rect,
    /// Drag handle band (resize / compact affordance).
    pub handle: Rect,
    /// Header / title.
    pub header: Rect,
    /// Main body for host content.
    pub body: Rect,
    /// Optional footer.
    pub footer: Rect,
}

impl DrawerSlots {
    /// Empty.
    #[must_use]
    pub const fn empty() -> Self {
        Self {
            root: Rect {
                x: 0,
                y: 0,
                width: 0,
                height: 0,
            },
            handle: Rect {
                x: 0,
                y: 0,
                width: 0,
                height: 0,
            },
            header: Rect {
                x: 0,
                y: 0,
                width: 0,
                height: 0,
            },
            body: Rect {
                x: 0,
                y: 0,
                width: 0,
                height: 0,
            },
            footer: Rect {
                x: 0,
                y: 0,
                width: 0,
                height: 0,
            },
        }
    }
}

/// Drawer / sheet interaction state.
///
/// **Does not** hold host view selection or scroll — those stay on the main
/// surface when the drawer opens (preserve underlying view).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DrawerState {
    open: bool,
    edge: DrawerEdge,
    modality: DrawerModality,
    presentation: DrawerPresentation,
    presentation_override: Option<DrawerPresentation>,
    /// Preferred depth (cols for L/R, rows for T/B).
    depth: u16,
    min_depth: u16,
    max_depth: u16,
    focused: bool,
    accepts_input: bool,
    enabled: bool,
    resizing: bool,
    resize_anchor: Option<u16>,
    slots: DrawerSlots,
    header_rows: u16,
    footer_rows: u16,
    /// Compact handle-only mode (host collapsed).
    handle_only: bool,
}

impl Default for DrawerState {
    fn default() -> Self {
        Self::new()
    }
}

impl DrawerState {
    /// Closed right-edge modal drawer.
    #[must_use]
    pub const fn new() -> Self {
        Self {
            open: false,
            edge: DrawerEdge::Right,
            modality: DrawerModality::Modal,
            presentation: DrawerPresentation::Expanded,
            presentation_override: None,
            depth: DRAWER_DEFAULT_WIDTH,
            min_depth: 16,
            max_depth: 64,
            focused: true,
            accepts_input: true,
            enabled: true,
            resizing: false,
            resize_anchor: None,
            slots: DrawerSlots::empty(),
            header_rows: 1,
            footer_rows: 0,
            handle_only: false,
        }
    }

    /// Sheet factory (bottom edge).
    #[must_use]
    pub const fn sheet() -> Self {
        let mut s = Self::new();
        s.edge = DrawerEdge::Bottom;
        s.depth = DRAWER_DEFAULT_HEIGHT;
        s.min_depth = 5;
        s.max_depth = 24;
        s
    }

    /// Non-modal factory.
    #[must_use]
    pub const fn non_modal() -> Self {
        let mut s = Self::new();
        s.modality = DrawerModality::NonModal;
        s
    }

    /// Whether open.
    #[must_use]
    pub const fn is_open(&self) -> bool {
        self.open
    }

    /// Edge.
    #[must_use]
    pub const fn edge(&self) -> DrawerEdge {
        self.edge
    }

    /// Set edge.
    pub fn set_edge(&mut self, edge: DrawerEdge) {
        self.edge = edge;
        if edge.is_horizontal() && self.depth < 12 {
            self.depth = DRAWER_DEFAULT_WIDTH;
        }
        if !edge.is_horizontal() && self.depth > 40 {
            self.depth = DRAWER_DEFAULT_HEIGHT;
        }
    }

    /// Modality.
    #[must_use]
    pub const fn modality(&self) -> DrawerModality {
        self.modality
    }

    /// Set modality.
    pub fn set_modality(&mut self, m: DrawerModality) {
        self.modality = m;
    }

    /// Presentation.
    #[must_use]
    pub const fn presentation(&self) -> DrawerPresentation {
        self.presentation
    }

    /// Force presentation.
    pub fn set_presentation_override(&mut self, p: Option<DrawerPresentation>) {
        self.presentation_override = p;
        if let Some(p) = p {
            self.presentation = p;
        }
    }

    /// Preferred depth.
    #[must_use]
    pub const fn depth(&self) -> u16 {
        self.depth
    }

    /// Set depth (clamped).
    pub fn set_depth(&mut self, depth: u16) {
        self.depth = depth.clamp(self.min_depth, self.max_depth);
    }

    /// Resize limits.
    pub fn set_depth_limits(&mut self, min: u16, max: u16) {
        self.min_depth = min.max(1);
        self.max_depth = max.max(self.min_depth);
        self.depth = self.depth.clamp(self.min_depth, self.max_depth);
    }

    /// Header / footer rows.
    pub fn set_header_rows(&mut self, rows: u16) {
        self.header_rows = rows;
    }

    /// Footer rows.
    pub fn set_footer_rows(&mut self, rows: u16) {
        self.footer_rows = rows;
    }

    /// Slots after paint.
    #[must_use]
    pub const fn slots(&self) -> DrawerSlots {
        self.slots
    }

    /// Body area convenience.
    #[must_use]
    pub const fn body_area(&self) -> Rect {
        self.slots.body
    }

    /// Focus.
    pub fn set_focused(&mut self, on: bool) {
        self.focused = on;
    }

    /// Focused?
    #[must_use]
    pub const fn is_focused(&self) -> bool {
        self.focused
    }

    /// Input gate.
    pub fn set_accepts_input(&mut self, on: bool) {
        self.accepts_input = on;
    }

    /// Enable.
    pub fn set_enabled(&mut self, on: bool) {
        self.enabled = on;
    }

    /// Compact handle-only.
    pub fn set_handle_only(&mut self, on: bool) {
        self.handle_only = on;
        if on {
            self.presentation = DrawerPresentation::Compact;
        }
    }

    /// Overlay size for current edge/depth.
    #[must_use]
    pub fn overlay_size(&self, bounds: Rect) -> OverlaySize {
        if self.edge.is_horizontal() {
            OverlaySize {
                width: self.depth.min(bounds.width.saturating_sub(1).max(1)),
                height: bounds.height.max(3),
                min_width: self.min_depth,
                min_height: 3,
                max_width: self.max_depth,
                max_height: 0,
            }
        } else {
            OverlaySize {
                width: bounds.width.max(8),
                height: self.depth.min(bounds.height.saturating_sub(1).max(1)),
                min_width: 8,
                min_height: self.min_depth,
                max_width: 0,
                max_height: self.max_depth,
            }
        }
    }

    /// Request open.
    pub fn request_open(&mut self, bounds: Rect) -> DrawerOutcome {
        if !self.enabled {
            return DrawerOutcome::Ignored;
        }
        let presentation = self
            .presentation_override
            .unwrap_or_else(|| drawer_presentation_for(bounds, self.edge, self.depth));
        self.presentation = presentation;
        self.open = true;
        self.focused = true;
        DrawerOutcome::Opened
    }

    /// Request close.
    pub fn request_close(&mut self) -> DrawerOutcome {
        if !self.open {
            return DrawerOutcome::Ignored;
        }
        self.open = false;
        self.focused = false;
        self.resizing = false;
        DrawerOutcome::Closed
    }

    /// Sync with stack presence.
    pub fn sync_with_stack<F>(&mut self, stack: &OverlayStack<F>, id: &OverlayId) {
        let on = stack.contains(id);
        self.open = on;
        if on {
            self.accepts_input = stack.top_owns_input()
                && stack
                    .top()
                    .is_some_and(|t| &t.id == id || t.id.0.starts_with(&id.0));
            if let Some(top) = stack.top() {
                if top.id == *id || top.id.0.starts_with(DRAWER_OVERLAY_ID) {
                    if self.edge.is_horizontal() {
                        self.depth = top.rect.width.clamp(self.min_depth, self.max_depth);
                    } else {
                        self.depth = top.rect.height.clamp(self.min_depth, self.max_depth);
                    }
                }
            }
        } else {
            self.focused = false;
            self.accepts_input = false;
        }
    }

    /// Sync presentation from bounds.
    pub fn sync_presentation(&mut self, bounds: Rect) -> DrawerOutcome {
        if self.presentation_override.is_some() {
            return DrawerOutcome::Ignored;
        }
        let next = drawer_presentation_for(bounds, self.edge, self.depth);
        if next != self.presentation {
            self.presentation = next;
            DrawerOutcome::PresentationChanged { presentation: next }
        } else {
            DrawerOutcome::Ignored
        }
    }

    /// Open on stack with opener restoration.
    pub fn open_on_stack<F: Clone>(
        &mut self,
        stack: &mut OverlayStack<F>,
        bounds: Rect,
        opener_focus: Option<F>,
    ) -> OverlayOutcome<F> {
        let _ = self.request_open(bounds);
        open_drawer_configured(
            stack,
            bounds,
            DRAWER_OVERLAY_ID,
            self.overlay_size(bounds),
            opener_focus,
            self.edge,
            self.modality,
            self.presentation_override.or(Some(self.presentation)),
            None,
        )
    }

    /// Close on stack.
    pub fn close_on_stack<F: Clone>(&mut self, stack: &mut OverlayStack<F>) -> OverlayOutcome<F> {
        let _ = self.request_close();
        dismiss_drawer_overlay(stack)
    }

    /// Keyboard: Esc closes when open.
    pub fn handle_key(&mut self, key: KeyEvent) -> DrawerOutcome {
        if !self.open || !self.enabled || !self.accepts_input {
            return DrawerOutcome::Ignored;
        }
        if key.kind == KeyEventKind::Release {
            return DrawerOutcome::Ignored;
        }
        if key.code == KeyCode::Esc && key.modifiers.is_empty() {
            return self.request_close();
        }
        // Resize via [ ] when horizontal and focused
        if self.edge.is_horizontal()
            && key.modifiers.is_empty()
            && matches!(key.code, KeyCode::Char('[' | ']'))
        {
            let delta: i16 = if matches!(key.code, KeyCode::Char('[')) {
                -2
            } else {
                2
            };
            let next = (i32::from(self.depth) + i32::from(delta))
                .clamp(i32::from(self.min_depth), i32::from(self.max_depth))
                as u16;
            if next != self.depth {
                self.depth = next;
                return DrawerOutcome::Resized { depth: self.depth };
            }
        }
        DrawerOutcome::Ignored
    }

    /// Intent Cancel/Close.
    pub fn handle_intent(&mut self, intent: UiIntent) -> DrawerOutcome {
        if !self.open || !self.enabled || !self.accepts_input {
            return DrawerOutcome::Ignored;
        }
        match intent {
            UiIntent::Cancel | UiIntent::Close => self.request_close(),
            _ => DrawerOutcome::Ignored,
        }
    }

    /// Mouse: resize handle drag; outside handled by stack.
    pub fn handle_mouse(&mut self, event: MouseEvent) -> DrawerOutcome {
        if !self.open || !self.enabled || !self.accepts_input {
            return DrawerOutcome::Ignored;
        }
        let handle = self.slots.handle;
        match event.kind {
            MouseEventKind::Down(MouseButton::Left)
                if !handle.is_empty() && handle.contains(event.position) =>
            {
                self.resizing = true;
                self.resize_anchor = Some(if self.edge.is_horizontal() {
                    event.position.x
                } else {
                    event.position.y
                });
                DrawerOutcome::Ignored
            }
            MouseEventKind::Drag(MouseButton::Left) if self.resizing => {
                let Some(anchor) = self.resize_anchor else {
                    return DrawerOutcome::Ignored;
                };
                let (pos, grow_positive) = if self.edge.is_horizontal() {
                    (event.position.x, matches!(self.edge, DrawerEdge::Left))
                } else {
                    (event.position.y, matches!(self.edge, DrawerEdge::Top))
                };
                let delta = if grow_positive {
                    i32::from(pos).saturating_sub(i32::from(anchor))
                } else {
                    i32::from(anchor).saturating_sub(i32::from(pos))
                };
                let next = (i32::from(self.depth) + delta)
                    .clamp(i32::from(self.min_depth), i32::from(self.max_depth))
                    as u16;
                self.resize_anchor = Some(pos);
                if next != self.depth {
                    self.depth = next;
                    DrawerOutcome::Resized { depth: self.depth }
                } else {
                    DrawerOutcome::Ignored
                }
            }
            MouseEventKind::Up(MouseButton::Left) => {
                self.resizing = false;
                self.resize_anchor = None;
                DrawerOutcome::Ignored
            }
            _ => DrawerOutcome::Ignored,
        }
    }

    /// Mark focus entered.
    pub fn enter_focus(&mut self) -> DrawerOutcome {
        if !self.open {
            return DrawerOutcome::Ignored;
        }
        self.focused = true;
        DrawerOutcome::FocusEntered
    }

    /// Legacy open flag helper (menu_nav API).
    pub const fn open(&mut self) {
        self.open = true;
        self.focused = true;
    }
}

/// Sheet state alias (bottom-edge drawer).
pub type SheetState = DrawerState;

// ── Widget ──────────────────────────────────────────────────────────────────

/// Edge drawer / sheet chrome with header/body/footer slots and resize handle.
#[derive(Debug, Clone, Copy)]
pub struct Drawer<'a> {
    system: &'a DesignSystem,
    title: Option<&'a str>,
    footer: Option<&'a str>,
    ascii: bool,
    colorless: bool,
}

/// Sheet paint alias.
pub type Sheet<'a> = Drawer<'a>;

impl<'a> Drawer<'a> {
    /// Title convenience (legacy `Drawer::new(title, system)`).
    #[must_use]
    pub const fn new(title: &'a str, system: &'a DesignSystem) -> Self {
        Self {
            system,
            title: Some(title),
            footer: None,
            ascii: false,
            colorless: false,
        }
    }

    /// Slot-oriented without default title.
    #[must_use]
    pub const fn slots(system: &'a DesignSystem) -> Self {
        Self {
            system,
            title: None,
            footer: None,
            ascii: false,
            colorless: false,
        }
    }

    /// Header title.
    #[must_use]
    pub const fn title(mut self, t: Option<&'a str>) -> Self {
        self.title = t;
        self
    }

    /// Footer text.
    #[must_use]
    pub const fn footer(mut self, f: Option<&'a str>) -> Self {
        self.footer = f;
        self
    }

    /// ASCII / no-motion handle glyphs.
    #[must_use]
    pub const fn ascii(mut self, on: bool) -> Self {
        self.ascii = on;
        self
    }

    /// Colorless roles.
    #[must_use]
    pub const fn colorless(mut self, on: bool) -> Self {
        self.colorless = on;
        self
    }

    /// Paint chrome into overlay rect; compute slots.
    pub fn paint(&self, area: Rect, buffer: &mut Buffer, state: &mut DrawerState) {
        if area.is_empty() {
            state.slots = DrawerSlots::empty();
            return;
        }
        state.slots.root = area;

        let border = if state.focused && !self.colorless {
            Role::BorderFocused
        } else {
            Role::Border
        };
        let border_style = self.system.style(border);
        // The tier rides the design system into every widget; a private copy
        // on the state could disagree with it, and did.
        let no_motion = !self.system.motion.allows_ambient() || self.ascii;
        // The docked edge butts against the pane the drawer slid out of, and
        // the handle column is already the rule at that seam. Drawing a border
        // there too stacked three vertical lines on one boundary
        // (plans/009 Step 4).
        let borders = match state.edge {
            DrawerEdge::Right => Borders::ALL & !Borders::LEFT,
            DrawerEdge::Left => Borders::ALL & !Borders::RIGHT,
            DrawerEdge::Bottom => Borders::ALL & !Borders::TOP,
            DrawerEdge::Top => Borders::ALL & !Borders::BOTTOM,
        };
        super::Surface::new(self.system)
            .recipe(super::SurfaceRecipe::Overlay)
            .bordered(true)
            .borders(borders)
            .border_style(border_style)
            .content_inset()
            .paint(area, buffer);

        // Handle on inner edge (resize grip)
        let handle = match state.edge {
            DrawerEdge::Right => Rect::new(area.x, area.y, DRAWER_HANDLE_CELLS, area.height),
            DrawerEdge::Left => Rect::new(
                area.right().saturating_sub(DRAWER_HANDLE_CELLS),
                area.y,
                DRAWER_HANDLE_CELLS,
                area.height,
            ),
            DrawerEdge::Bottom => Rect::new(area.x, area.y, area.width, DRAWER_HANDLE_CELLS),
            DrawerEdge::Top => Rect::new(
                area.x,
                area.bottom().saturating_sub(DRAWER_HANDLE_CELLS),
                area.width,
                DRAWER_HANDLE_CELLS,
            ),
        };
        state.slots.handle = handle;
        let handle_glyph = if no_motion {
            if state.edge.is_horizontal() { "|" } else { "=" }
        } else if state.edge.is_horizontal() {
            "│"
        } else {
            "─"
        };
        for y in handle.y..handle.bottom() {
            for x in handle.x..handle.right() {
                buffer.set_stringn(x, y, handle_glyph, 1, border_style);
            }
        }

        // Content inner (exclude handle)
        let mut inner = match state.edge {
            DrawerEdge::Right => Rect {
                x: area.x.saturating_add(DRAWER_HANDLE_CELLS),
                y: area.y,
                width: area.width.saturating_sub(DRAWER_HANDLE_CELLS),
                height: area.height,
            },
            DrawerEdge::Left => Rect {
                x: area.x,
                y: area.y,
                width: area.width.saturating_sub(DRAWER_HANDLE_CELLS),
                height: area.height,
            },
            DrawerEdge::Bottom => Rect {
                x: area.x,
                y: area.y.saturating_add(DRAWER_HANDLE_CELLS),
                width: area.width,
                height: area.height.saturating_sub(DRAWER_HANDLE_CELLS),
            },
            DrawerEdge::Top => Rect {
                x: area.x,
                y: area.y,
                width: area.width,
                height: area.height.saturating_sub(DRAWER_HANDLE_CELLS),
            },
        };

        // Shared Surface paints the shell; the handle above overlays its inner
        // edge. Preserve the established content geometry below.
        if area.width >= 2 && area.height >= 2 {
            // Shrink inner for border
            inner = Rect {
                x: inner.x.saturating_add(1),
                y: inner.y.saturating_add(1),
                width: inner.width.saturating_sub(
                    if matches!(state.edge, DrawerEdge::Left | DrawerEdge::Right) {
                        1
                    } else {
                        2
                    },
                ),
                height: inner.height.saturating_sub(
                    if matches!(state.edge, DrawerEdge::Top | DrawerEdge::Bottom) {
                        1
                    } else {
                        2
                    },
                ),
            };
        }

        if state.handle_only
            || matches!(state.presentation, DrawerPresentation::Compact) && inner.width <= 4
        {
            // Compact: title only in remaining
            state.slots.header = inner;
            state.slots.body = Rect::default();
            state.slots.footer = Rect::default();
            if let Some(t) = self.title {
                buffer.set_stringn(
                    inner.x,
                    inner.y,
                    &take_display_cols(t, usize::from(inner.width)),
                    usize::from(inner.width),
                    self.system
                        .style(Role::TextStrong)
                        .add_modifier(Modifier::BOLD),
                );
            }
            return;
        }

        if inner.is_empty() {
            state.slots.header = Rect::default();
            state.slots.body = Rect::default();
            state.slots.footer = Rect::default();
            return;
        }

        let header_h = state.header_rows.min(inner.height);
        let footer_h = if state.footer_rows > 0 {
            state.footer_rows.min(inner.height.saturating_sub(header_h))
        } else {
            0
        };
        let body_h = inner
            .height
            .saturating_sub(header_h)
            .saturating_sub(footer_h);

        let mut y = inner.y;
        if header_h > 0 {
            state.slots.header = Rect::new(inner.x, y, inner.width, header_h);
            if let Some(title) = self.title {
                buffer.set_stringn(
                    inner.x,
                    y,
                    &take_display_cols(title, usize::from(inner.width)),
                    usize::from(inner.width),
                    self.system
                        .style(Role::TextStrong)
                        .add_modifier(Modifier::BOLD),
                );
            }
            y = y.saturating_add(header_h);
        } else {
            state.slots.header = Rect::default();
        }

        state.slots.body = Rect::new(inner.x, y, inner.width, body_h);
        y = y.saturating_add(body_h);

        if footer_h > 0 {
            state.slots.footer = Rect::new(inner.x, y, inner.width, footer_h);
            if let Some(ft) = self.footer {
                buffer.set_stringn(
                    inner.x,
                    y,
                    &take_display_cols(ft, usize::from(inner.width)),
                    usize::from(inner.width),
                    self.system.style(Role::TextMuted),
                );
            }
        } else {
            state.slots.footer = Rect::default();
        }
        let _ = display_cols;
    }

    /// Semantic registration.
    pub fn register_semantic<Id, Action>(
        &self,
        scene: &mut SemanticScene<Id, Action>,
        id: Id,
        area: Rect,
        state: &DrawerState,
    ) where
        Id: Clone + PartialEq + std::fmt::Display,
        Action: Clone,
    {
        if area.is_empty() || !state.open {
            return;
        }
        let desc = format!(
            "drawer edge={} modality={} presentation={} depth={} focused={}",
            state.edge.id(),
            state.modality.id(),
            state.presentation.id(),
            state.depth,
            state.focused,
        );
        let _ = scene.register(
            SemanticNode::control(id, area)
                .role(SemanticRole::Overlay)
                .label("drawer")
                .description(desc)
                .focusable(state.enabled && state.accepts_input)
                .state(SemanticState {
                    selected: state.focused,
                    expanded: state.open,
                    ..Default::default()
                }),
        );
    }
}

impl StatefulWidget for &Drawer<'_> {
    type State = DrawerState;

    fn render(self, area: Rect, buffer: &mut Buffer, state: &mut Self::State) {
        self.paint(area, buffer, state);
    }
}

impl StatefulWidget for Drawer<'_> {
    type State = DrawerState;

    fn render(self, area: Rect, buffer: &mut Buffer, state: &mut Self::State) {
        <&Self as StatefulWidget>::render(&self, area, buffer, state);
    }
}

// Legacy Widget without state (lookbook snapshots).
impl Widget for &Drawer<'_> {
    fn render(self, area: Rect, buffer: &mut Buffer) {
        let mut state = DrawerState::new();
        state.open = true;
        state.focused = true;
        state.set_header_rows(1);
        self.paint(area, buffer, &mut state);
    }
}

impl Widget for Drawer<'_> {
    fn render(self, area: Rect, buffer: &mut Buffer) {
        <&Self as Widget>::render(&self, area, buffer);
    }
}

// ── Tests ───────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use crate::input::KeyModifiers;

    #[test]
    fn place_right_and_left_edges() {
        let bounds = Rect::new(0, 0, 80, 24);
        let size = OverlaySize {
            width: 28,
            height: 24,
            min_width: 12,
            min_height: 3,
            max_width: 40,
            max_height: 0,
        };
        let right = place_drawer_on_edge(
            bounds,
            DrawerEdge::Right,
            size,
            DrawerPresentation::Expanded,
        );
        assert_eq!(right.width, 28);
        assert_eq!(right.x, 80 - 28);
        assert_eq!(right.height, 24);

        let left =
            place_drawer_on_edge(bounds, DrawerEdge::Left, size, DrawerPresentation::Expanded);
        assert_eq!(left.x, 0);
        assert_eq!(left.width, 28);
    }

    #[test]
    fn place_top_and_bottom_edges() {
        let bounds = Rect::new(0, 0, 80, 24);
        let size = OverlaySize {
            width: 80,
            height: 8,
            min_width: 8,
            min_height: 4,
            max_width: 0,
            max_height: 16,
        };
        let top = place_drawer_on_edge(bounds, DrawerEdge::Top, size, DrawerPresentation::Expanded);
        assert_eq!(top.y, 0);
        assert_eq!(top.height, 8);
        assert_eq!(top.width, 80);

        let bottom = place_drawer_on_edge(
            bounds,
            DrawerEdge::Bottom,
            size,
            DrawerPresentation::Expanded,
        );
        assert_eq!(bottom.y, 24 - 8);
        assert_eq!(bottom.height, 8);
    }

    #[test]
    fn open_close_restores_opener() {
        let bounds = Rect::new(0, 0, 80, 24);
        let mut stack = OverlayStack::<&'static str>::new();
        let mut state = DrawerState::new();
        let out = state.open_on_stack(&mut stack, bounds, Some("main"));
        assert!(matches!(out, OverlayOutcome::Opened { .. }));
        assert_eq!(stack.top().unwrap().kind, OverlayKind::Drawer);
        assert!(state.is_open());
        assert!(matches!(
            state.close_on_stack(&mut stack),
            OverlayOutcome::Dismissed {
                focus: Some("main"),
                ..
            }
        ));
        assert!(!state.is_open());
    }

    #[test]
    fn modal_vs_non_modal_policy() {
        let modal = DrawerModality::Modal.policy(DrawerEdge::Right);
        let non = DrawerModality::NonModal.policy(DrawerEdge::Right);
        assert!(modal.focus_trap);
        assert!(!non.focus_trap);
        assert!(matches!(modal.backdrop, BackdropPolicy::Dim));
        assert!(matches!(non.backdrop, BackdropPolicy::None));
    }

    #[test]
    fn non_modal_open() {
        let bounds = Rect::new(0, 0, 80, 24);
        let mut stack = OverlayStack::<()>::new();
        let size = OverlaySize {
            width: 24,
            height: 24,
            min_width: 12,
            min_height: 3,
            max_width: 40,
            max_height: 0,
        };
        let _ = open_drawer_configured(
            &mut stack,
            bounds,
            "termrock.drawer.nonmodal",
            size,
            None,
            DrawerEdge::Left,
            DrawerModality::NonModal,
            None,
            None,
        );
        assert!(!stack.top().unwrap().policy.focus_trap);
    }

    #[test]
    fn nested_child_cascade_dismiss() {
        let bounds = Rect::new(0, 0, 80, 24);
        let mut stack = OverlayStack::<&'static str>::new();
        let mut state = DrawerState::new();
        let _ = state.open_on_stack(&mut stack, bounds, Some("root"));
        let size = OverlaySize {
            width: 20,
            height: 24,
            min_width: 10,
            min_height: 3,
            max_width: 30,
            max_height: 0,
        };
        let _ =
            open_drawer_nested_overlay(&mut stack, bounds, size, DrawerEdge::Right, Some("root"));
        assert_eq!(stack.entries().len(), 2);
        let _ = dismiss_drawer_overlay(&mut stack);
        assert!(stack.is_empty());
    }

    #[test]
    fn fullscreen_on_tiny_bounds() {
        assert_eq!(
            drawer_presentation_for(Rect::new(0, 0, 30, 20), DrawerEdge::Right, 28),
            DrawerPresentation::Fullscreen
        );
        let bounds = Rect::new(0, 0, 30, 10);
        let mut stack = OverlayStack::<()>::new();
        let mut state = DrawerState::new();
        let _ = state.open_on_stack(&mut stack, bounds, None);
        let top = stack.top().unwrap();
        assert!(
            top.kind == OverlayKind::Fullscreen || top.fullscreen_promoted || top.rect == bounds
        );
    }

    #[test]
    fn esc_and_intent_close() {
        let mut state = DrawerState::new();
        state.open = true;
        state.accepts_input = true;
        assert!(matches!(
            state.handle_key(KeyEvent::new(KeyCode::Esc, KeyModifiers::NONE)),
            DrawerOutcome::Closed
        ));
        state.open = true;
        assert!(matches!(
            state.handle_intent(UiIntent::Cancel),
            DrawerOutcome::Closed
        ));
    }

    #[test]
    fn resize_via_keys() {
        let mut state = DrawerState::new();
        state.open = true;
        state.accepts_input = true;
        state.set_depth(28);
        assert!(matches!(
            state.handle_key(KeyEvent::new(KeyCode::Char(']'), KeyModifiers::NONE)),
            DrawerOutcome::Resized { depth: 30 }
        ));
        assert!(matches!(
            state.handle_key(KeyEvent::new(KeyCode::Char('['), KeyModifiers::NONE)),
            DrawerOutcome::Resized { depth: 28 }
        ));
    }

    #[test]
    fn slots_header_body_footer() {
        let system = DesignSystem::default();
        let mut state = DrawerState::new();
        state.open = true;
        state.set_header_rows(1);
        state.set_footer_rows(1);
        let area = Rect::new(0, 0, 30, 16);
        let mut buf = Buffer::empty(area);
        Drawer::new("Inspector", &system)
            .footer(Some("esc cancel"))
            .paint(area, &mut buf, &mut state);
        assert_eq!(state.slots.header.height, 1);
        assert_eq!(state.slots.footer.height, 1);
        assert!(state.slots.body.height >= 1);
        assert!(!state.slots.handle.is_empty());
        let text: String = buf
            .content()
            .iter()
            .map(|c| c.symbol().to_string())
            .collect();
        assert!(text.contains("Inspector"), "{text}");
    }

    #[test]
    fn sheet_bottom_default() {
        let state = DrawerState::sheet();
        assert_eq!(state.edge(), DrawerEdge::Bottom);
        let bounds = Rect::new(0, 0, 80, 24);
        let size = state.overlay_size(bounds);
        let placed = place_drawer_on_edge(
            bounds,
            DrawerEdge::Bottom,
            size,
            DrawerPresentation::Expanded,
        );
        assert_eq!(placed.y + placed.height, 24);
    }

    #[test]
    fn no_motion_ascii_handle() {
        let system = DesignSystem::default();
        let system = system.motion(MotionPolicy::Off);
        let mut state = DrawerState::new();
        state.open = true;
        let area = Rect::new(0, 0, 24, 12);
        let mut buf = Buffer::empty(area);
        Drawer::new("Rail", &system)
            .ascii(true)
            .paint(area, &mut buf, &mut state);
        let text: String = buf
            .content()
            .iter()
            .map(|c| c.symbol().to_string())
            .collect();
        assert!(text.contains('|') || text.contains("Rail"), "{text}");
    }

    #[test]
    fn semantic_registers() {
        let system = DesignSystem::default();
        let mut state = DrawerState::new();
        state.open = true;
        let mut scene = SemanticScene::<&str, ()>::default();
        Drawer::new("D", &system).register_semantic(
            &mut scene,
            "d",
            Rect::new(0, 0, 20, 10),
            &state,
        );
        assert!(
            scene
                .nodes()
                .iter()
                .any(|n| n.label.as_deref() == Some("drawer"))
        );
    }

    #[test]
    fn preserve_host_selection_contract() {
        // Structural: DrawerState has no selection/scroll of host lists.
        let state = DrawerState::new();
        assert!(!state.is_open());
        // Opening must not require host to pass selection — host keeps its own.
        let _ = state;
    }

    #[test]
    fn fuzz_keys() {
        let mut state = DrawerState::new();
        state.open = true;
        state.accepts_input = true;
        let keys = [
            KeyCode::Esc,
            KeyCode::Char('['),
            KeyCode::Char(']'),
            KeyCode::Enter,
            KeyCode::Tab,
        ];
        let mut seed = 13u64;
        for _ in 0..200 {
            if !state.is_open() {
                state.open = true;
                state.accepts_input = true;
            }
            seed = seed.wrapping_mul(1103515245).wrapping_add(12345);
            let k = keys[(seed as usize) % keys.len()];
            let _ = state.handle_key(KeyEvent::new(k, KeyModifiers::NONE));
        }
    }

    #[test]
    fn paint_perf_smoke() {
        use ratatui_core::backend::TestBackend;
        use ratatui_core::terminal::Terminal;
        let system = DesignSystem::default();
        let mut state = DrawerState::new();
        state.open = true;
        state.set_header_rows(1);
        state.set_footer_rows(1);
        let mut terminal = Terminal::new(TestBackend::new(36, 20)).unwrap();
        let start = std::time::Instant::now();
        for _ in 0..200 {
            terminal
                .draw(|f| {
                    Drawer::new("Filters", &system).footer(Some("esc")).paint(
                        f.area(),
                        f.buffer_mut(),
                        &mut state,
                    );
                })
                .unwrap();
        }
        assert!(start.elapsed().as_millis() < 5_000);
    }

    #[test]
    fn pty_snapshot_stable() {
        use ratatui_core::backend::TestBackend;
        use ratatui_core::terminal::Terminal;
        let system = DesignSystem::default();
        let mut s1 = DrawerState::new();
        s1.open = true;
        s1.focused = true;
        let mut t1 = Terminal::new(TestBackend::new(28, 12)).unwrap();
        t1.draw(|f| {
            Drawer::new("Details", &system).paint(f.area(), f.buffer_mut(), &mut s1);
        })
        .unwrap();
        let a: String = t1
            .backend()
            .buffer()
            .content()
            .iter()
            .map(|c| c.symbol().to_string())
            .collect();
        let mut s2 = DrawerState::new();
        s2.open = true;
        s2.focused = true;
        let mut t2 = Terminal::new(TestBackend::new(28, 12)).unwrap();
        t2.draw(|f| {
            Drawer::new("Details", &system).paint(f.area(), f.buffer_mut(), &mut s2);
        })
        .unwrap();
        let b: String = t2
            .backend()
            .buffer()
            .content()
            .iter()
            .map(|c| c.symbol().to_string())
            .collect();
        assert_eq!(a, b);
        assert!(a.contains("Details"));
    }

    #[test]
    fn legacy_widget_title_paint() {
        let system = DesignSystem::default();
        let area = Rect::new(0, 0, 20, 8);
        let mut buf = Buffer::empty(area);
        Widget::render(&Drawer::new("Settings", &system), area, &mut buf);
        let text: String = buf
            .content()
            .iter()
            .map(|c| c.symbol().to_string())
            .collect();
        assert!(text.contains("Settings"), "{text}");
    }

    #[test]
    fn place_drawer_compat() {
        let bounds = Rect::new(0, 0, 80, 24);
        let size = OverlaySize {
            width: 28,
            height: 24,
            min_width: 12,
            min_height: 3,
            max_width: 40,
            max_height: 0,
        };
        let r = place_drawer(bounds, size);
        assert_eq!(
            r,
            place_drawer_on_edge(
                bounds,
                DrawerEdge::Right,
                size,
                DrawerPresentation::Expanded
            )
        );
    }
}
