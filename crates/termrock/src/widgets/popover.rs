// SPDX-FileCopyrightText: 2026 Alexey Zhokhov
// SPDX-License-Identifier: Apache-2.0

//! **Popover** — anchored interactive surface for settings, filters, pickers, details.
//!
//! **Mission.** Non-modal (default) or explicitly modal anchored chrome with
//! header / body / footer **slots**, collision placement via
//! [`OverlayStack`], outside/Esc dismissal, nested children, and opener
//! focus restoration. Geometry is never private to the component — hosts
//! open and reflow through the stack.
//!
//! **vs Dialog.** Dialog is centered modal blocking chrome. Popover is
//! anchor-relative; modality is opt-in via [`PopoverModality`].
//! **vs Tooltip.** Tooltip never owns input; Popover does (when open).
//! **vs Drawer.** Drawer is edge-attached; Popover may **contract** to drawer
//! or fullscreen when the preferred size cannot fit.
//!
//! Research: Radix Popover, terminal pickers, Textual overlays.

use ratatui_core::{buffer::Buffer, layout::Rect, style::Modifier, widgets::StatefulWidget};

use crate::{
    input::{KeyCode, KeyEvent, KeyEventKind},
    interaction::{
        BackdropPolicy, LayerDismissPolicy, NarrowFallback, OverlayId, OverlayKind, OverlayOutcome,
        OverlayPolicy, OverlaySize, OverlaySpec, OverlayStack, PlacementPrefer, SemanticNode,
        SemanticRole, SemanticScene, SemanticState, UiIntent, place_overlay,
    },
    style::{DesignSystem, Role},
    text::take_display_cols,
};

/// Default overlay id for popovers.
pub const POPOVER_OVERLAY_ID: &str = "termrock.popover";
/// Width at or below which anchored popovers contract toward drawer/fullscreen.
pub const POPOVER_CONTRACT_MAX_WIDTH: u16 = 40;
/// Height at or below which fullscreen contraction is preferred.
pub const POPOVER_CONTRACT_MAX_HEIGHT: u16 = 12;

// ── Modality & presentation ─────────────────────────────────────────────────

/// Explicit modal versus non-modal behavior (stack policy).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
#[non_exhaustive]
pub enum PopoverModality {
    /// Default: owns input, Esc/outside dismiss, **no** focus trap, no dim.
    /// Matches [`OverlayKind::Popover`] policy.
    #[default]
    NonModal,
    /// Blocking: focus trap, dim backdrop, outside clicks trapped (like dialog).
    /// Still anchor-aware for placement when size fits; may fullscreen on narrow.
    Modal,
}

impl PopoverModality {
    /// Stable id.
    #[must_use]
    pub const fn id(self) -> &'static str {
        match self {
            Self::NonModal => "non-modal",
            Self::Modal => "modal",
        }
    }

    /// Overlay policy for this modality (geometry kind stays Popover or Fullscreen).
    #[must_use]
    pub const fn policy(self) -> OverlayPolicy {
        match self {
            Self::NonModal => OverlayPolicy::for_kind(OverlayKind::Popover),
            Self::Modal => OverlayPolicy {
                esc: LayerDismissPolicy::Dismissible,
                outside: LayerDismissPolicy::Trap,
                owns_input: true,
                focus_trap: true,
                wheel_captures: true,
                backdrop: BackdropPolicy::Dim,
                prefer: PlacementPrefer::BelowStart,
                cover_anchor: false,
                narrow_fallback: NarrowFallback::Fullscreen,
                narrow_cols: POPOVER_CONTRACT_MAX_WIDTH,
            },
        }
    }
}

/// How the popover is presented after placement / contraction.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
#[non_exhaustive]
pub enum PopoverPresentation {
    /// Anchored under/near trigger (default).
    #[default]
    Anchored,
    /// Edge drawer when width is tight but height allows.
    Drawer,
    /// Full bounds when content cannot fit.
    Fullscreen,
}

impl PopoverPresentation {
    /// Stable id.
    #[must_use]
    pub const fn id(self) -> &'static str {
        match self {
            Self::Anchored => "anchored",
            Self::Drawer => "drawer",
            Self::Fullscreen => "fullscreen",
        }
    }
}

/// Choose presentation from terminal bounds and preferred content size.
#[must_use]
pub fn popover_presentation_for(bounds: Rect, preferred: OverlaySize) -> PopoverPresentation {
    if bounds.is_empty() {
        return PopoverPresentation::Anchored;
    }
    let need_w = preferred.width.max(preferred.min_width);
    let need_h = preferred.height.max(preferred.min_height);
    if bounds.width <= POPOVER_CONTRACT_MAX_WIDTH
        || bounds.height <= POPOVER_CONTRACT_MAX_HEIGHT
        || need_w > bounds.width.saturating_sub(2)
        || need_h > bounds.height.saturating_sub(2)
    {
        if bounds.width <= POPOVER_CONTRACT_MAX_WIDTH && bounds.height > POPOVER_CONTRACT_MAX_HEIGHT
        {
            return PopoverPresentation::Drawer;
        }
        return PopoverPresentation::Fullscreen;
    }
    PopoverPresentation::Anchored
}

// ── Placement / open helpers (OverlayStack only) ────────────────────────────

/// Places an anchored popover (flip/clamp via Popover policy).
#[must_use]
pub fn place_popover(bounds: Rect, anchor: Rect, size: OverlaySize) -> Rect {
    if bounds.is_empty() || size.width == 0 || size.height == 0 {
        return Rect::default();
    }
    place_overlay(
        bounds,
        Some(anchor),
        size,
        OverlayPolicy::for_kind(OverlayKind::Popover),
    )
}

/// Place using modality-specific policy.
#[must_use]
pub fn place_popover_with_modality(
    bounds: Rect,
    anchor: Option<Rect>,
    size: OverlaySize,
    modality: PopoverModality,
    presentation: PopoverPresentation,
) -> Rect {
    if bounds.is_empty() {
        return Rect::default();
    }
    match presentation {
        PopoverPresentation::Fullscreen => bounds,
        PopoverPresentation::Drawer => place_overlay(
            bounds,
            None,
            size,
            OverlayPolicy::for_kind(OverlayKind::Drawer),
        ),
        PopoverPresentation::Anchored => place_overlay(bounds, anchor, size, modality.policy()),
    }
}

/// Opens an anchored non-modal popover on the stack (default).
pub fn open_popover_overlay<FocusId: Clone>(
    stack: &mut OverlayStack<FocusId>,
    bounds: Rect,
    anchor: Rect,
    size: OverlaySize,
    opener_focus: Option<FocusId>,
) -> OverlayOutcome<FocusId> {
    open_popover_configured(
        stack,
        bounds,
        Some(anchor),
        size,
        opener_focus,
        PopoverModality::NonModal,
        None,
        None,
    )
}

/// Opens a modal popover (focus trap + dim; may fullscreen on narrow).
pub fn open_popover_modal_overlay<FocusId: Clone>(
    stack: &mut OverlayStack<FocusId>,
    bounds: Rect,
    anchor: Rect,
    size: OverlaySize,
    opener_focus: Option<FocusId>,
) -> OverlayOutcome<FocusId> {
    open_popover_configured(
        stack,
        bounds,
        Some(anchor),
        size,
        opener_focus,
        PopoverModality::Modal,
        None,
        None,
    )
}

/// Nested child popover under an open parent (cascade dismiss).
pub fn open_popover_nested_overlay<FocusId: Clone>(
    stack: &mut OverlayStack<FocusId>,
    bounds: Rect,
    anchor: Rect,
    size: OverlaySize,
    parent: impl Into<OverlayId>,
    opener_focus: Option<FocusId>,
) -> OverlayOutcome<FocusId> {
    open_popover_configured(
        stack,
        bounds,
        Some(anchor),
        size,
        opener_focus,
        PopoverModality::NonModal,
        Some(parent.into()),
        Some(format!("{}.nested", POPOVER_OVERLAY_ID)),
    )
}

/// Full configuration open (presentation auto-selected unless `force_presentation`).
pub fn open_popover_configured<FocusId: Clone>(
    stack: &mut OverlayStack<FocusId>,
    bounds: Rect,
    anchor: Option<Rect>,
    size: OverlaySize,
    opener_focus: Option<FocusId>,
    modality: PopoverModality,
    parent: Option<OverlayId>,
    id_override: Option<String>,
) -> OverlayOutcome<FocusId> {
    open_popover_with_presentation(
        stack,
        bounds,
        anchor,
        size,
        opener_focus,
        modality,
        parent,
        id_override,
        None,
    )
}

/// Open with optional forced [`PopoverPresentation`] (skips auto contraction when set).
pub fn open_popover_with_presentation<FocusId: Clone>(
    stack: &mut OverlayStack<FocusId>,
    bounds: Rect,
    anchor: Option<Rect>,
    size: OverlaySize,
    opener_focus: Option<FocusId>,
    modality: PopoverModality,
    parent: Option<OverlayId>,
    id_override: Option<String>,
    force_presentation: Option<PopoverPresentation>,
) -> OverlayOutcome<FocusId> {
    let presentation = force_presentation.unwrap_or_else(|| popover_presentation_for(bounds, size));
    let id = OverlayId(id_override.unwrap_or_else(|| POPOVER_OVERLAY_ID.to_string()));
    let mut spec = match presentation {
        PopoverPresentation::Fullscreen => {
            OverlaySpec::fullscreen(id, opener_focus).with_policy(modality.policy())
        }
        PopoverPresentation::Drawer => {
            let mut s = OverlaySpec::drawer(id, size, opener_focus);
            // Keep modality backdrop/esc when modal
            if matches!(modality, PopoverModality::Modal) {
                s = s.with_policy(modality.policy());
            }
            s
        }
        PopoverPresentation::Anchored => {
            let anchor = anchor.unwrap_or_else(|| {
                Rect::new(
                    bounds.x + bounds.width / 2,
                    bounds.y + bounds.height / 2,
                    1,
                    1,
                )
            });
            OverlaySpec::popover(id, anchor, size, opener_focus).with_policy(modality.policy())
        }
    };
    if let Some(p) = parent {
        spec = spec.with_parent(p);
    }
    stack.open(bounds, spec)
}

/// Dismiss default popover id (and nested children via stack).
pub fn dismiss_popover_overlay<FocusId: Clone>(
    stack: &mut OverlayStack<FocusId>,
) -> OverlayOutcome<FocusId> {
    stack.dismiss(&OverlayId::from_static(POPOVER_OVERLAY_ID))
}

// ── Slots ───────────────────────────────────────────────────────────────────

/// Slot geometry after paint (host paints content into these rects).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct PopoverSlots {
    /// Outer root (overlay rect).
    pub root: Rect,
    /// Optional header band.
    pub header: Rect,
    /// Main interactive body.
    pub body: Rect,
    /// Optional footer / actions.
    pub footer: Rect,
}

impl PopoverSlots {
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

// ── Outcomes / state ────────────────────────────────────────────────────────

/// Typed outcomes (host coordinates OverlayStack).
#[derive(Debug, Clone, PartialEq, Eq)]
#[non_exhaustive]
pub enum PopoverOutcome {
    /// No change.
    Ignored,
    /// Local open flag set (host should call open_*_overlay).
    OpenRequested {
        /// Suggested presentation.
        presentation: PopoverPresentation,
        /// Modality for stack policy.
        modality: PopoverModality,
    },
    /// Local close (host should dismiss stack entry → restores opener_focus).
    CloseRequested,
    /// Surface focus entered (first focusable inside — host moves focus).
    FocusEntered,
    /// Presentation suggestion changed (reflow stack).
    PresentationChanged {
        /// New presentation.
        presentation: PopoverPresentation,
    },
}

/// Local interaction state (geometry still owned by OverlayStack).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PopoverState {
    open: bool,
    modality: PopoverModality,
    presentation: PopoverPresentation,
    presentation_override: Option<PopoverPresentation>,
    /// Surface has keyboard ownership inside the popover.
    focused: bool,
    accepts_input: bool,
    enabled: bool,
    slots: PopoverSlots,
    /// Header / footer heights requested (0 = omit).
    header_rows: u16,
    footer_rows: u16,
}

impl Default for PopoverState {
    fn default() -> Self {
        Self::new()
    }
}

impl PopoverState {
    /// Closed non-modal popover.
    #[must_use]
    pub const fn new() -> Self {
        Self {
            open: false,
            modality: PopoverModality::NonModal,
            presentation: PopoverPresentation::Anchored,
            presentation_override: None,
            focused: false,
            accepts_input: true,
            enabled: true,
            slots: PopoverSlots::empty(),
            header_rows: 1,
            footer_rows: 0,
        }
    }

    /// Modal factory.
    #[must_use]
    pub const fn modal() -> Self {
        let mut s = Self::new();
        s.modality = PopoverModality::Modal;
        s
    }

    /// Whether open (local flag; stack is source of truth for geometry).
    #[must_use]
    pub const fn is_open(&self) -> bool {
        self.open
    }

    /// Force local open flag (lookbook / tests; host still owns OverlayStack).
    pub fn set_open(&mut self, on: bool) {
        self.open = on;
        if !on {
            self.focused = false;
        }
    }

    /// Modality.
    #[must_use]
    pub const fn modality(&self) -> PopoverModality {
        self.modality
    }

    /// Set modality.
    pub fn set_modality(&mut self, m: PopoverModality) {
        self.modality = m;
    }

    /// Presentation.
    #[must_use]
    pub const fn presentation(&self) -> PopoverPresentation {
        self.presentation
    }

    /// Force presentation.
    pub fn set_presentation_override(&mut self, p: Option<PopoverPresentation>) {
        self.presentation_override = p;
        if let Some(p) = p {
            self.presentation = p;
        }
    }

    /// Header rows (0 hides header slot).
    pub fn set_header_rows(&mut self, rows: u16) {
        self.header_rows = rows;
    }

    /// Footer rows (0 hides footer slot).
    pub fn set_footer_rows(&mut self, rows: u16) {
        self.footer_rows = rows;
    }

    /// Slots after last paint.
    #[must_use]
    pub const fn slots(&self) -> PopoverSlots {
        self.slots
    }

    /// Body area convenience.
    #[must_use]
    pub const fn body_area(&self) -> Rect {
        self.slots.body
    }

    /// Focused inside popover.
    pub fn set_focused(&mut self, on: bool) {
        self.focused = on;
    }

    /// Focused?
    #[must_use]
    pub const fn is_focused(&self) -> bool {
        self.focused
    }

    /// Input gate (mirror stack top_owns_input).
    pub fn set_accepts_input(&mut self, on: bool) {
        self.accepts_input = on;
    }

    /// Enable.
    pub fn set_enabled(&mut self, on: bool) {
        self.enabled = on;
    }

    /// Request open — host places via OverlayStack.
    pub fn request_open(&mut self, bounds: Rect, preferred: OverlaySize) -> PopoverOutcome {
        if !self.enabled {
            return PopoverOutcome::Ignored;
        }
        let presentation = self
            .presentation_override
            .unwrap_or_else(|| popover_presentation_for(bounds, preferred));
        self.presentation = presentation;
        self.open = true;
        self.focused = true;
        PopoverOutcome::OpenRequested {
            presentation,
            modality: self.modality,
        }
    }

    /// Request close — host dismisses stack (opener restoration).
    pub fn request_close(&mut self) -> PopoverOutcome {
        if !self.open {
            return PopoverOutcome::Ignored;
        }
        self.open = false;
        self.focused = false;
        PopoverOutcome::CloseRequested
    }

    /// Sync local open with stack presence.
    pub fn sync_with_stack<F>(&mut self, stack: &OverlayStack<F>, id: &OverlayId) {
        let on_stack = stack.contains(id);
        self.open = on_stack;
        if on_stack {
            self.accepts_input = stack.top_owns_input() && stack.top().is_some_and(|t| &t.id == id);
        } else {
            self.focused = false;
            self.accepts_input = false;
        }
    }

    /// Sync presentation from bounds.
    pub fn sync_presentation(&mut self, bounds: Rect, preferred: OverlaySize) -> PopoverOutcome {
        if self.presentation_override.is_some() {
            return PopoverOutcome::Ignored;
        }
        let next = popover_presentation_for(bounds, preferred);
        if next != self.presentation {
            self.presentation = next;
            PopoverOutcome::PresentationChanged { presentation: next }
        } else {
            PopoverOutcome::Ignored
        }
    }

    /// Keyboard: Esc requests close when focused and non-ignored by host stack.
    ///
    /// Prefer stack `handle_escape` for actual dismiss; this is for hosts that
    /// route keys into the popover surface first.
    pub fn handle_key(&mut self, key: KeyEvent) -> PopoverOutcome {
        if !self.open || !self.enabled || !self.accepts_input {
            return PopoverOutcome::Ignored;
        }
        if key.kind == KeyEventKind::Release {
            return PopoverOutcome::Ignored;
        }
        if key.code == KeyCode::Esc && key.modifiers.is_empty() {
            return self.request_close();
        }
        PopoverOutcome::Ignored
    }

    /// Intent Cancel/Close → close request.
    pub fn handle_intent(&mut self, intent: UiIntent) -> PopoverOutcome {
        if !self.open || !self.enabled || !self.accepts_input {
            return PopoverOutcome::Ignored;
        }
        match intent {
            UiIntent::Cancel | UiIntent::Close => self.request_close(),
            _ => PopoverOutcome::Ignored,
        }
    }

    /// Mark focus entered (host called after Tab into popover).
    pub fn enter_focus(&mut self) -> PopoverOutcome {
        if !self.open {
            return PopoverOutcome::Ignored;
        }
        self.focused = true;
        PopoverOutcome::FocusEntered
    }

    /// Open on stack helper (honors modality + presentation override).
    pub fn open_on_stack<F: Clone>(
        &mut self,
        stack: &mut OverlayStack<F>,
        bounds: Rect,
        anchor: Rect,
        size: OverlaySize,
        opener_focus: Option<F>,
    ) -> OverlayOutcome<F> {
        let _ = self.request_open(bounds, size);
        open_popover_with_presentation(
            stack,
            bounds,
            Some(anchor),
            size,
            opener_focus,
            self.modality,
            None,
            None,
            self.presentation_override,
        )
    }

    /// Close on stack (restores opener_focus from stack entry).
    pub fn close_on_stack<F: Clone>(&mut self, stack: &mut OverlayStack<F>) -> OverlayOutcome<F> {
        let _ = self.request_close();
        dismiss_popover_overlay(stack)
    }
}

// ── Widget (slots, no forced Panel) ─────────────────────────────────────────

/// Popover chrome: elevated fill, optional border, **header/body/footer slots**.
///
/// Does **not** require [`crate::widgets::Panel`]. Hosts paint domain widgets
/// into [`PopoverState::slots`] after [`Self::paint`].
#[derive(Debug, Clone, Copy)]
pub struct Popover<'a> {
    system: &'a DesignSystem,
    /// Optional header title when header_rows ≥ 1.
    header: Option<&'a str>,
    /// Optional footer hint when footer_rows ≥ 1.
    footer: Option<&'a str>,
    /// Draw single-line border (focus uses BorderFocused role when focused).
    border: bool,
    ascii: bool,
    colorless: bool,
}

impl<'a> Popover<'a> {
    /// Creates a popover with empty header/footer slots.
    #[must_use]
    pub const fn new(system: &'a DesignSystem) -> Self {
        Self {
            system,
            header: None,
            footer: None,
            border: true,
            ascii: false,
            colorless: false,
        }
    }

    /// Header title text.
    #[must_use]
    pub const fn header(mut self, h: Option<&'a str>) -> Self {
        self.header = h;
        self
    }

    /// Footer text.
    #[must_use]
    pub const fn footer(mut self, f: Option<&'a str>) -> Self {
        self.footer = f;
        self
    }

    /// Border chrome.
    #[must_use]
    pub const fn border(mut self, on: bool) -> Self {
        self.border = on;
        self
    }

    /// ASCII border glyphs.
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

    /// Paint chrome and compute slots into `state`.
    ///
    /// `area` is the **overlay rect** from OverlayStack (not computed privately).
    pub fn paint(&self, area: Rect, buffer: &mut Buffer, state: &mut PopoverState) {
        if area.is_empty() {
            state.slots = PopoverSlots::empty();
            return;
        }
        state.slots.root = area;

        let recipe = if state.focused {
            super::SurfaceRecipe::OverlayFocused
        } else {
            super::SurfaceRecipe::Overlay
        };

        let colorless_system;
        let surface_system = if self.colorless {
            colorless_system = self
                .system
                .clone()
                .capability(crate::style::ColorCapability::Monochrome);
            &colorless_system
        } else {
            self.system
        };
        let inner = super::Surface::new(surface_system)
            .recipe(recipe)
            .bordered(self.border)
            .content_inset()
            .paint(area, buffer);

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
            if let Some(title) = self.header {
                crate::text::paint_text(
                    buffer,
                    Rect::new(inner.x, y, inner.width, 1),
                    title,
                    self.system
                        .style(Role::TextStrong)
                        .add_modifier(Modifier::BOLD),
                    self.system.glyphs.ellipsis(),
                );
            }
            // A header taller than its title spends its last row on a rule, so
            // the header reads as a band instead of a floating line. A
            // single-row header keeps every cell for the title.
            if header_h >= 2 && body_h > 0 {
                let rule_y = y.saturating_add(header_h.saturating_sub(1));
                let rule = self.system.glyphs.rule().repeat(usize::from(inner.width));
                buffer.set_stringn(
                    inner.x,
                    rule_y,
                    &rule,
                    usize::from(inner.width),
                    self.system.style(Role::Border),
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
    }

    /// Semantic registration.
    pub fn register_semantic<Id, Action>(
        &self,
        scene: &mut SemanticScene<Id, Action>,
        id: Id,
        area: Rect,
        state: &PopoverState,
    ) where
        Id: Clone + PartialEq + std::fmt::Display,
        Action: Clone,
    {
        if area.is_empty() || !state.open {
            return;
        }
        let desc = format!(
            "popover modality={} presentation={} focused={}",
            state.modality().id(),
            state.presentation().id(),
            state.is_focused()
        );
        let _ = scene.register(
            SemanticNode::control(id, area)
                .role(SemanticRole::Overlay)
                .label("popover")
                .description(desc)
                .focusable(state.enabled && state.accepts_input)
                .disabled(!state.enabled)
                .state(SemanticState {
                    selected: state.focused,
                    expanded: state.open,
                    ..Default::default()
                }),
        );
    }
}

impl StatefulWidget for &Popover<'_> {
    type State = PopoverState;

    fn render(self, area: Rect, buffer: &mut Buffer, state: &mut Self::State) {
        self.paint(area, buffer, state);
    }
}

impl StatefulWidget for Popover<'_> {
    type State = PopoverState;

    fn render(self, area: Rect, buffer: &mut Buffer, state: &mut Self::State) {
        <&Self as StatefulWidget>::render(&self, area, buffer, state);
    }
}

// ── Tests ───────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use crate::input::KeyModifiers;

    #[test]
    fn modality_policies_differ() {
        let non = PopoverModality::NonModal.policy();
        let modal = PopoverModality::Modal.policy();
        assert!(non.owns_input);
        assert!(!non.focus_trap);
        assert!(modal.focus_trap);
        assert!(matches!(modal.backdrop, BackdropPolicy::Dim));
        assert!(matches!(modal.outside, LayerDismissPolicy::Trap));
    }

    #[test]
    fn presentation_contracts_on_narrow() {
        let size = OverlaySize {
            width: 30,
            height: 10,
            min_width: 20,
            min_height: 5,
            max_width: 0,
            max_height: 0,
        };
        assert_eq!(
            popover_presentation_for(Rect::new(0, 0, 30, 24), size),
            PopoverPresentation::Drawer
        );
        assert_eq!(
            popover_presentation_for(Rect::new(0, 0, 20, 10), size),
            PopoverPresentation::Fullscreen
        );
        assert_eq!(
            popover_presentation_for(Rect::new(0, 0, 80, 24), size),
            PopoverPresentation::Anchored
        );
    }

    #[test]
    fn open_close_restores_opener_focus() {
        let bounds = Rect::new(0, 0, 80, 24);
        let anchor = Rect::new(10, 5, 8, 1);
        let mut stack = OverlayStack::<&'static str>::new();
        let mut state = PopoverState::new();
        let size = OverlaySize::menu(28, 8);
        let out = state.open_on_stack(&mut stack, bounds, anchor, size, Some("trigger"));
        assert!(matches!(out, OverlayOutcome::Opened { .. }));
        assert!(state.is_open());
        assert_eq!(stack.top().unwrap().kind, OverlayKind::Popover);
        let placed = place_popover(bounds, anchor, size);
        assert_eq!(stack.top().unwrap().rect, placed);
        assert!(matches!(
            state.close_on_stack(&mut stack),
            OverlayOutcome::Dismissed {
                focus: Some("trigger"),
                ..
            }
        ));
        assert!(!state.is_open());
        assert!(stack.is_empty());
    }

    #[test]
    fn modal_open_uses_focus_trap_policy() {
        let bounds = Rect::new(0, 0, 80, 24);
        let anchor = Rect::new(5, 5, 4, 1);
        let mut stack = OverlayStack::<()>::new();
        let size = OverlaySize::menu(24, 6);
        let out = open_popover_modal_overlay(&mut stack, bounds, anchor, size, None);
        assert!(matches!(out, OverlayOutcome::Opened { .. }));
        assert!(stack.top().unwrap().policy.focus_trap);
    }

    #[test]
    fn nested_parent_cascade_dismiss() {
        let bounds = Rect::new(0, 0, 80, 24);
        let anchor = Rect::new(10, 5, 6, 1);
        let mut stack = OverlayStack::<&'static str>::new();
        let size = OverlaySize::menu(20, 5);
        let _ = open_popover_overlay(&mut stack, bounds, anchor, size, Some("root"));
        let sub = Rect::new(30, 8, 1, 1);
        let _ = open_popover_nested_overlay(
            &mut stack,
            bounds,
            sub,
            size,
            POPOVER_OVERLAY_ID,
            Some("root"),
        );
        assert_eq!(stack.entries().len(), 2);
        // Dismiss root removes children
        let _ = dismiss_popover_overlay(&mut stack);
        assert!(stack.is_empty());
    }

    #[test]
    fn esc_requests_close() {
        let mut state = PopoverState::new();
        state.open = true;
        state.focused = true;
        state.accepts_input = true;
        assert!(matches!(
            state.handle_key(KeyEvent::new(KeyCode::Esc, KeyModifiers::NONE)),
            PopoverOutcome::CloseRequested
        ));
        assert!(!state.is_open());
    }

    #[test]
    fn slots_header_body_footer() {
        let system = DesignSystem::default();
        let mut state = PopoverState::new();
        state.open = true;
        state.set_header_rows(1);
        state.set_footer_rows(1);
        let area = Rect::new(0, 0, 30, 10);
        let mut buf = Buffer::empty(area);
        Popover::new(&system)
            .header(Some("Settings"))
            .footer(Some("esc cancel"))
            .paint(area, &mut buf, &mut state);
        assert_eq!(state.slots.header.height, 1);
        assert_eq!(state.slots.footer.height, 1);
        assert!(state.slots.body.height >= 1);
        assert_eq!(
            state.slots.header.height + state.slots.body.height + state.slots.footer.height,
            area.height.saturating_sub(2), // border
        );
        let text: String = buf
            .content()
            .iter()
            .map(|c| c.symbol().to_string())
            .collect();
        assert!(text.contains("Settings"), "{text}");
    }

    #[test]
    fn no_panel_required_borderless() {
        let system = DesignSystem::default();
        let mut state = PopoverState::new();
        state.open = true;
        state.set_header_rows(0);
        state.set_footer_rows(0);
        let area = Rect::new(0, 0, 20, 6);
        let mut buf = Buffer::empty(area);
        Popover::new(&system)
            .border(false)
            .paint(area, &mut buf, &mut state);
        assert_eq!(state.slots.body, area);
    }

    #[test]
    fn colorless_focus_keeps_strong_overlay_outline() {
        let system = DesignSystem::default();
        let area = Rect::new(0, 0, 20, 6);
        let mut state = PopoverState::new();
        state.set_open(true);
        state.set_focused(true);
        let mut buffer = Buffer::empty(area);

        Popover::new(&system)
            .colorless(true)
            .paint(area, &mut buffer, &mut state);

        assert!(
            buffer[(0, 0)]
                .modifier
                .contains(ratatui_core::style::Modifier::BOLD),
            "focus must remain visible after chroma is removed"
        );
    }

    #[test]
    fn outside_click_dismisses_non_modal() {
        let bounds = Rect::new(0, 0, 80, 24);
        let anchor = Rect::new(10, 10, 4, 1);
        let mut stack = OverlayStack::<()>::new();
        let _ = open_popover_overlay(&mut stack, bounds, anchor, OverlaySize::menu(20, 5), None);
        assert!(matches!(
            stack.handle_outside_click(ratatui_core::layout::Position::new(0, 0)),
            OverlayOutcome::Dismissed { .. }
        ));
    }

    #[test]
    fn semantic_registers_when_open() {
        let system = DesignSystem::default();
        let mut state = PopoverState::new();
        state.open = true;
        let mut scene = SemanticScene::<&str, ()>::default();
        Popover::new(&system).header(Some("X")).register_semantic(
            &mut scene,
            "p",
            Rect::new(0, 0, 20, 5),
            &state,
        );
        assert!(
            scene
                .nodes()
                .iter()
                .any(|n| n.label.as_deref() == Some("popover"))
        );
    }

    #[test]
    fn disabled_popover_rejects_input_and_registers_disabled_semantics() {
        let system = DesignSystem::default();
        let mut state = PopoverState::new();
        state.set_open(true);
        state.set_focused(true);
        state.set_enabled(false);
        assert_eq!(
            state.handle_key(KeyEvent::new(KeyCode::Esc, KeyModifiers::NONE)),
            PopoverOutcome::Ignored
        );
        let mut scene = SemanticScene::<&str, ()>::default();
        Popover::new(&system).register_semantic(
            &mut scene,
            "popover",
            Rect::new(0, 0, 20, 5),
            &state,
        );
        let node = scene.nodes().first().expect("popover semantic node");
        assert!(node.disabled);
        assert!(!node.focusable);
    }

    #[test]
    fn fuzz_keys() {
        let mut state = PopoverState::new();
        state.open = true;
        state.accepts_input = true;
        state.focused = true;
        let keys = [
            KeyCode::Esc,
            KeyCode::Enter,
            KeyCode::Char('a'),
            KeyCode::Tab,
        ];
        let mut seed = 3u64;
        for _ in 0..100 {
            seed = seed.wrapping_mul(1103515245).wrapping_add(12345);
            let k = keys[(seed as usize) % keys.len()];
            if !state.is_open() {
                state.open = true;
                state.accepts_input = true;
            }
            let _ = state.handle_key(KeyEvent::new(k, KeyModifiers::NONE));
        }
    }

    #[test]
    fn focus_enter_outcome() {
        let mut state = PopoverState::new();
        assert!(matches!(state.enter_focus(), PopoverOutcome::Ignored));
        state.open = true;
        assert!(matches!(state.enter_focus(), PopoverOutcome::FocusEntered));
        assert!(state.is_focused());
    }

    #[test]
    fn modal_outside_click_traps() {
        let bounds = Rect::new(0, 0, 80, 24);
        let anchor = Rect::new(10, 10, 4, 1);
        let mut stack = OverlayStack::<()>::new();
        let _ =
            open_popover_modal_overlay(&mut stack, bounds, anchor, OverlaySize::menu(20, 5), None);
        // Trap policy: outside click is consumed; layer stays.
        assert!(matches!(
            stack.handle_outside_click(ratatui_core::layout::Position::new(0, 0)),
            OverlayOutcome::Ignored
        ));
        assert!(!stack.is_empty());
        assert!(stack.top().unwrap().policy.focus_trap);
        assert!(matches!(
            stack.top().unwrap().policy.outside,
            LayerDismissPolicy::Trap
        ));
    }

    #[test]
    fn intent_cancel_closes() {
        let mut state = PopoverState::new();
        state.open = true;
        state.accepts_input = true;
        assert!(matches!(
            state.handle_intent(UiIntent::Cancel),
            PopoverOutcome::CloseRequested
        ));
    }

    #[test]
    fn presentation_sync_and_override() {
        let size = OverlaySize::menu(30, 10);
        let mut state = PopoverState::new();
        assert!(matches!(
            state.sync_presentation(Rect::new(0, 0, 30, 24), size),
            PopoverOutcome::PresentationChanged {
                presentation: PopoverPresentation::Drawer
            }
        ));
        state.set_presentation_override(Some(PopoverPresentation::Anchored));
        assert!(matches!(
            state.sync_presentation(Rect::new(0, 0, 30, 24), size),
            PopoverOutcome::Ignored
        ));
        assert_eq!(state.presentation(), PopoverPresentation::Anchored);
    }

    #[test]
    fn force_presentation_on_open() {
        let bounds = Rect::new(0, 0, 20, 10); // would auto-fullscreen
        let anchor = Rect::new(2, 2, 4, 1);
        let mut stack = OverlayStack::<()>::new();
        let size = OverlaySize::menu(30, 10);
        let out = open_popover_with_presentation(
            &mut stack,
            bounds,
            Some(anchor),
            size,
            None,
            PopoverModality::NonModal,
            None,
            None,
            Some(PopoverPresentation::Anchored),
        );
        assert!(matches!(out, OverlayOutcome::Opened { .. }));
        assert_eq!(stack.top().unwrap().kind, OverlayKind::Popover);
    }

    #[test]
    fn paint_perf_smoke() {
        use ratatui_core::backend::TestBackend;
        use ratatui_core::terminal::Terminal;
        let system = DesignSystem::default();
        let mut state = PopoverState::new();
        state.open = true;
        state.set_header_rows(1);
        state.set_footer_rows(1);
        let mut terminal = Terminal::new(TestBackend::new(40, 12)).unwrap();
        let start = std::time::Instant::now();
        for _ in 0..300 {
            terminal
                .draw(|f| {
                    Popover::new(&system)
                        .header(Some("Settings"))
                        .footer(Some("esc"))
                        .paint(f.area(), f.buffer_mut(), &mut state);
                })
                .unwrap();
        }
        assert!(start.elapsed().as_millis() < 5_000);
    }

    #[test]
    fn pty_style_buffer_snapshot_stable() {
        use ratatui_core::backend::TestBackend;
        use ratatui_core::terminal::Terminal;
        let system = DesignSystem::default();
        let mut state = PopoverState::new();
        state.open = true;
        state.set_focused(true);
        state.set_header_rows(1);
        state.set_footer_rows(1);
        let mut terminal = Terminal::new(TestBackend::new(24, 8)).unwrap();
        terminal
            .draw(|f| {
                Popover::new(&system)
                    .header(Some("Filter"))
                    .footer(Some("esc"))
                    .paint(f.area(), f.buffer_mut(), &mut state);
            })
            .unwrap();
        let text: String = terminal
            .backend()
            .buffer()
            .content()
            .iter()
            .map(|c| c.symbol().to_string())
            .collect();
        assert!(text.contains("Filter"), "{text}");
        assert!(text.contains("esc"), "{text}");
        assert!(state.slots.body.width > 0);
        // Second frame identical chrome (deterministic paint).
        let mut terminal2 = Terminal::new(TestBackend::new(24, 8)).unwrap();
        let mut state2 = PopoverState::new();
        state2.open = true;
        state2.set_focused(true);
        state2.set_header_rows(1);
        state2.set_footer_rows(1);
        terminal2
            .draw(|f| {
                Popover::new(&system)
                    .header(Some("Filter"))
                    .footer(Some("esc"))
                    .paint(f.area(), f.buffer_mut(), &mut state2);
            })
            .unwrap();
        let text2: String = terminal2
            .backend()
            .buffer()
            .content()
            .iter()
            .map(|c| c.symbol().to_string())
            .collect();
        assert_eq!(text, text2);
    }
}
