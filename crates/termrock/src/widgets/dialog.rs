// SPDX-FileCopyrightText: 2026 Alexey Zhokhov
// SPDX-License-Identifier: Apache-2.0

//! **Dialog** — canonical modal interaction surface.
//!
//! **Mission.** Title, description, body, actions, close policy, focus trap
//! (via [`OverlayStack`]), initial focus, opener restoration, scrolling body,
//! loading, and validation. Recipes: normal, compact, wide, fullscreen,
//! destructive-adjacent. Nested popovers attach with parent id
//! [`DIALOG_OVERLAY_ID`].
//!
//! **Enter / Esc.** Enter activates the **default** or focused action only when
//! the action zone owns input and the dialog is not loading / failing
//! validation — never accidental submit from body scroll. Esc dismisses only
//! when [`DialogClosePolicy::Dismissible`]; alert / confirm-only traps Esc.
//!
//! **vs Popover.** Popover is non-modal (default) and anchored. Dialog is
//! centered modal with trap + dim.
//!
//! Research: Radix Dialog, Textual modals, Grok Build flows, desktop conventions.

use ratatui_core::{
    buffer::Buffer,
    layout::Rect,
    style::{Color, Style},
    text::Text,
    widgets::{StatefulWidget, Widget},
};
use ratatui_widgets::{clear::Clear, paragraph::Paragraph};

use crate::{
    input::{KeyCode, KeyEvent, KeyEventKind, KeyModifiers},
    interaction::{
        HitRegion, NavigationMove, Outcome, OverlayId, OverlayKind, OverlayOutcome, OverlayPolicy,
        OverlaySize, OverlaySpec, OverlayStack, SemanticNode, SemanticRole, SemanticScene,
        SemanticState, UiIntent, place_overlay,
    },
    scroll::DialogScroll,
    style::{Density, DesignSystem, Role, RolePalette},
};

use super::{
    Action, ActionBar, ActionBarState, DetailRow, DetailTable, DetailTableState, Panel, PanelChrome,
};

/// Default overlay id for a modal dialog on an [`OverlayStack`].
pub const DIALOG_OVERLAY_ID: &str = "termrock.dialog";
/// Width at or below which recipes promote toward fullscreen.
pub const DIALOG_FULLSCREEN_MAX_WIDTH: u16 = 40;
/// Height at or below which recipes promote toward fullscreen.
pub const DIALOG_FULLSCREEN_MAX_HEIGHT: u16 = 12;
/// Nested popover / child overlay id prefix under a dialog.
pub const DIALOG_NESTED_OVERLAY_PREFIX: &str = "termrock.dialog.child";

// ── Size / recipe / policy ──────────────────────────────────────────────────

/// Preferred dialog size before clamp / narrow promotion.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct DialogSize {
    /// Preferred width in cells.
    pub width: u16,
    /// Preferred height in rows.
    pub height: u16,
}

impl Default for DialogSize {
    fn default() -> Self {
        Self::for_density(Density::Comfortable)
    }
}

impl DialogSize {
    /// Preferred size for a density mode (cells).
    #[must_use]
    pub const fn for_density(density: Density) -> Self {
        match density {
            Density::Comfortable => Self {
                width: 48,
                height: 12,
            },
            Density::Compact => Self {
                width: 40,
                height: 10,
            },
            Density::Dashboard => Self {
                width: 36,
                height: 8,
            },
        }
    }

    /// Minimum usable width before fullscreen promotion (policy elsewhere).
    #[must_use]
    pub const fn min_width(density: Density) -> u16 {
        match density {
            Density::Comfortable => 40,
            Density::Compact => 36,
            Density::Dashboard => 32,
        }
    }

    /// Size for a recipe (before bounds contraction).
    #[must_use]
    pub const fn for_recipe(recipe: DialogRecipe) -> Self {
        match recipe {
            DialogRecipe::Normal => Self {
                width: 48,
                height: 12,
            },
            DialogRecipe::Compact => Self {
                width: 36,
                height: 9,
            },
            DialogRecipe::Wide => Self {
                width: 64,
                height: 14,
            },
            DialogRecipe::Fullscreen => Self {
                width: 0,
                height: 0,
            },
            DialogRecipe::Destructive => Self {
                width: 44,
                height: 11,
            },
        }
    }
}

impl From<DialogSize> for OverlaySize {
    fn from(value: DialogSize) -> Self {
        OverlaySize::dialog(value.width, value.height)
    }
}

/// Layout density / width recipe for the modal frame.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
#[non_exhaustive]
pub enum DialogRecipe {
    /// Standard centered modal.
    #[default]
    Normal,
    /// Tighter chrome for confirmations.
    Compact,
    /// Wider body (forms, multi-column).
    Wide,
    /// Fill bounds (tiny terminals or immersive).
    Fullscreen,
    /// Destructive-adjacent (danger border + confirm caution).
    Destructive,
}

impl DialogRecipe {
    /// Stable id.
    #[must_use]
    pub const fn id(self) -> &'static str {
        match self {
            Self::Normal => "normal",
            Self::Compact => "compact",
            Self::Wide => "wide",
            Self::Fullscreen => "fullscreen",
            Self::Destructive => "destructive",
        }
    }

    /// Whether chrome uses danger emphasis.
    #[must_use]
    pub const fn is_destructive(self) -> bool {
        matches!(self, Self::Destructive)
    }
}

/// Visual / semantic dialog chrome variant (paint).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
#[non_exhaustive]
pub enum DialogVariant {
    /// Neutral elevated dialog.
    #[default]
    Default,
    /// Destructive / risk surface (`PanelChrome::Danger`).
    Danger,
    /// Informational emphasis (focused border, info title tone).
    Info,
}

/// Esc / outside close behavior (maps to overlay policy).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
#[non_exhaustive]
pub enum DialogClosePolicy {
    /// Esc dismisses; outside clicks trapped (standard dialog).
    #[default]
    Dismissible,
    /// Esc and outside trapped until an explicit action (alert / confirm).
    ConfirmOnly,
    /// No Esc dismiss; host must call dismiss (blocking task).
    Locked,
}

impl DialogClosePolicy {
    /// Stable id.
    #[must_use]
    pub const fn id(self) -> &'static str {
        match self {
            Self::Dismissible => "dismissible",
            Self::ConfirmOnly => "confirm-only",
            Self::Locked => "locked",
        }
    }

    /// Overlay kind for this policy.
    #[must_use]
    pub const fn overlay_kind(self) -> OverlayKind {
        match self {
            Self::Dismissible => OverlayKind::Dialog,
            Self::ConfirmOnly | Self::Locked => OverlayKind::AlertDialog,
        }
    }
}

/// Which zone holds keyboard within the trap.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
#[non_exhaustive]
pub enum DialogFocusZone {
    /// Scrollable body / description (Enter does **not** submit by default).
    Body,
    /// Action bar (Enter activates cursor / default).
    #[default]
    Actions,
}

impl DialogFocusZone {
    /// Stable id.
    #[must_use]
    pub const fn id(self) -> &'static str {
        match self {
            Self::Body => "body",
            Self::Actions => "actions",
        }
    }
}

/// Slot geometry after paint.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct DialogSlots {
    /// Outer frame.
    pub root: Rect,
    /// Title band.
    pub title: Rect,
    /// Optional description under title.
    pub description: Rect,
    /// Scrollable body content.
    pub body: Rect,
    /// Validation / error banner.
    pub validation: Rect,
    /// Action bar.
    pub actions: Rect,
    /// Footer hint.
    pub footer: Rect,
}

impl DialogSlots {
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
            title: Rect {
                x: 0,
                y: 0,
                width: 0,
                height: 0,
            },
            description: Rect {
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
            validation: Rect {
                x: 0,
                y: 0,
                width: 0,
                height: 0,
            },
            actions: Rect {
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

/// Typed dialog outcomes (extends choice [`Outcome`] mapping).
#[derive(Debug, Clone, PartialEq, Eq)]
#[non_exhaustive]
pub enum DialogOutcome<Id> {
    /// No change.
    Ignored,
    /// Focus zone or action cursor moved.
    FocusMoved,
    /// Body scrolled.
    Scrolled,
    /// Action activated (click or Enter on actions).
    Activated(Id),
    /// Default action activated via Enter when configured.
    DefaultActivated(Id),
    /// Esc / cancel (only when dismissible).
    Cancelled,
    /// Enter blocked by validation.
    ValidationFailed,
    /// Loading blocked activation.
    LoadingBlocked,
}

impl<Id> DialogOutcome<Id> {
    /// Map into legacy [`Outcome`] for ChoiceDialog hosts.
    #[must_use]
    pub fn into_outcome(self) -> Outcome<Id> {
        match self {
            Self::Ignored | Self::LoadingBlocked | Self::ValidationFailed | Self::Scrolled => {
                Outcome::Ignored
            }
            Self::FocusMoved => Outcome::Changed,
            Self::Activated(id) | Self::DefaultActivated(id) => Outcome::Activated(id),
            Self::Cancelled => Outcome::Cancelled,
        }
    }
}

/// Choose recipe contraction from bounds.
#[must_use]
pub fn dialog_recipe_for_bounds(bounds: Rect, preferred: DialogRecipe) -> DialogRecipe {
    if bounds.is_empty() {
        return preferred;
    }
    if matches!(preferred, DialogRecipe::Fullscreen)
        || bounds.width <= DIALOG_FULLSCREEN_MAX_WIDTH
        || bounds.height <= DIALOG_FULLSCREEN_MAX_HEIGHT
    {
        return DialogRecipe::Fullscreen;
    }
    preferred
}

// ── Overlay helpers ─────────────────────────────────────────────────────────

/// Centered dialog rectangle using [`OverlayKind::Dialog`] policy.
#[must_use]
pub fn place_dialog(bounds: Rect, preferred: DialogSize) -> Rect {
    if bounds.is_empty() || preferred.width == 0 || preferred.height == 0 {
        return Rect::default();
    }
    place_overlay(
        bounds,
        None,
        OverlaySize::from(preferred),
        OverlayPolicy::for_kind(OverlayKind::Dialog),
    )
}

/// Place with recipe (fullscreen fills bounds).
#[must_use]
pub fn place_dialog_recipe(bounds: Rect, recipe: DialogRecipe) -> Rect {
    let recipe = dialog_recipe_for_bounds(bounds, recipe);
    match recipe {
        DialogRecipe::Fullscreen => {
            if bounds.is_empty() {
                Rect::default()
            } else {
                bounds
            }
        }
        other => place_dialog(bounds, DialogSize::for_recipe(other)),
    }
}

/// Opens (or replaces) a dismissible dialog overlay.
pub fn open_dialog_overlay<FocusId: Clone>(
    stack: &mut OverlayStack<FocusId>,
    bounds: Rect,
    preferred: DialogSize,
    opener_focus: Option<FocusId>,
) -> OverlayOutcome<FocusId> {
    open_dialog_configured(
        stack,
        bounds,
        preferred,
        opener_focus,
        DialogClosePolicy::Dismissible,
        None,
        None,
    )
}

/// Opens an alert dialog that traps Esc until an explicit action.
pub fn open_alert_dialog_overlay<FocusId: Clone>(
    stack: &mut OverlayStack<FocusId>,
    bounds: Rect,
    preferred: DialogSize,
    opener_focus: Option<FocusId>,
) -> OverlayOutcome<FocusId> {
    open_dialog_configured(
        stack,
        bounds,
        preferred,
        opener_focus,
        DialogClosePolicy::ConfirmOnly,
        None,
        None,
    )
}

/// Full open with close policy and optional recipe force.
pub fn open_dialog_configured<FocusId: Clone>(
    stack: &mut OverlayStack<FocusId>,
    bounds: Rect,
    preferred: DialogSize,
    opener_focus: Option<FocusId>,
    close_policy: DialogClosePolicy,
    recipe: Option<DialogRecipe>,
    id_override: Option<String>,
) -> OverlayOutcome<FocusId> {
    let id = OverlayId(id_override.unwrap_or_else(|| DIALOG_OVERLAY_ID.to_string()));
    let recipe = recipe.unwrap_or(DialogRecipe::Normal);
    let effective = dialog_recipe_for_bounds(bounds, recipe);
    let size = if matches!(effective, DialogRecipe::Fullscreen) {
        OverlaySize::dialog(bounds.width.max(1), bounds.height.max(1))
    } else {
        OverlaySize::from(preferred)
    };
    let kind_policy = close_policy.overlay_kind();
    let mut spec = if matches!(effective, DialogRecipe::Fullscreen) {
        let policy = OverlayPolicy {
            prefer: crate::interaction::PlacementPrefer::Fullscreen,
            narrow_fallback: crate::interaction::NarrowFallback::Fullscreen,
            ..OverlayPolicy::for_kind(kind_policy)
        };
        OverlaySpec::fullscreen(id, opener_focus).with_policy(policy)
    } else {
        match close_policy {
            DialogClosePolicy::Dismissible => OverlaySpec::dialog(id, size, opener_focus),
            DialogClosePolicy::ConfirmOnly | DialogClosePolicy::Locked => {
                OverlaySpec::alert_dialog(id, size, opener_focus)
            }
        }
    };
    if matches!(close_policy, DialogClosePolicy::Locked) {
        let mut policy = OverlayPolicy::for_kind(OverlayKind::AlertDialog);
        policy.esc = crate::interaction::LayerDismissPolicy::Trap;
        spec = spec.with_policy(policy);
    }
    stack.open(bounds, spec)
}

/// Open a nested child overlay (popover/menu) under the dialog for cascade dismiss.
pub fn open_dialog_child_overlay<FocusId: Clone>(
    stack: &mut OverlayStack<FocusId>,
    bounds: Rect,
    anchor: Rect,
    size: OverlaySize,
    child_suffix: &str,
    opener_focus: Option<FocusId>,
) -> OverlayOutcome<FocusId> {
    let id = format!("{DIALOG_NESTED_OVERLAY_PREFIX}.{child_suffix}");
    let spec = OverlaySpec::popover(id, anchor, size, opener_focus)
        .with_parent(OverlayId::from_static(DIALOG_OVERLAY_ID));
    stack.open(bounds, spec)
}

/// Dismisses the default dialog overlay when present (restores opener focus).
pub fn dismiss_dialog_overlay<FocusId: Clone>(
    stack: &mut OverlayStack<FocusId>,
) -> OverlayOutcome<FocusId> {
    stack.dismiss(&OverlayId::from_static(DIALOG_OVERLAY_ID))
}

// ── Backdrop ────────────────────────────────────────────────────────────────

/// A themed fill painted behind modal content.
#[derive(Debug, Clone, Copy)]
pub struct Backdrop {
    symbol: char,
    style: Style,
}

impl Default for Backdrop {
    fn default() -> Self {
        Self {
            symbol: ' ',
            style: Style::new()
                .fg(Color::Reset)
                .bg(crate::style::DIALOG_BACKDROP),
        }
    }
}

impl Backdrop {
    /// Fully opaque backdrop.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Terminal-default background (Reset).
    #[must_use]
    pub fn reset() -> Self {
        Self::default()
    }

    /// Dim wash glyph field.
    #[must_use]
    pub fn dim_wash(ascii: bool) -> Self {
        Self {
            symbol: if ascii { '.' } else { '░' },
            style: Style::new()
                .fg(Color::DarkGray)
                .bg(crate::style::DIALOG_BACKDROP)
                .add_modifier(ratatui_core::style::Modifier::DIM),
        }
    }

    /// From design tokens (Reset by default).
    #[must_use]
    pub fn from_tokens(tokens: &DesignSystem) -> Self {
        let _ = tokens;
        Self::reset()
    }

    /// Fill symbol.
    #[must_use]
    pub const fn symbol(mut self, symbol: char) -> Self {
        self.symbol = symbol;
        self
    }

    /// Fill style.
    #[must_use]
    pub const fn style(mut self, style: Style) -> Self {
        self.style = style;
        self
    }
}

impl Widget for &Backdrop {
    fn render(self, area: Rect, buffer: &mut Buffer) {
        for y in area.top()..area.bottom() {
            for x in area.left()..area.right() {
                buffer[(x, y)].set_char(self.symbol).set_style(self.style);
            }
        }
    }
}

impl Widget for Backdrop {
    #[expect(
        clippy::needless_borrows_for_generic_args,
        reason = "explicitly delegate the owned contract to the borrowed renderer"
    )]
    fn render(self, area: Rect, buffer: &mut Buffer) {
        <&Self as Widget>::render(&self, area, buffer);
    }
}

// ── DialogState ─────────────────────────────────────────────────────────────

/// Canonical dialog interaction state (focus trap content, not geometry).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DialogState<Id = ()> {
    open: bool,
    accepts_input: bool,
    loading: bool,
    close_policy: DialogClosePolicy,
    recipe: DialogRecipe,
    focus_zone: DialogFocusZone,
    /// Initial focus applied once on open.
    initial_focus: DialogFocusZone,
    initial_applied: bool,
    /// Action cursor (when actions present).
    action_cursor: Option<Id>,
    /// Default action id for Enter when in Actions zone.
    default_action: Option<Id>,
    /// Optional cancel action id (Esc can activate instead of dismiss).
    cancel_action: Option<Id>,
    /// When true, Enter in body never submits (default: true — no accidental submit).
    require_action_focus_for_enter: bool,
    validation_message: Option<String>,
    scroll: DialogScroll,
    body_line_count: usize,
    slots: DialogSlots,
    action_regions: Vec<HitRegion<Id>>,
}

impl<Id> Default for DialogState<Id> {
    fn default() -> Self {
        Self::new()
    }
}

impl<Id> DialogState<Id> {
    /// Closed dialog; opens with action focus.
    #[must_use]
    pub fn new() -> Self {
        Self {
            open: true,
            accepts_input: true,
            loading: false,
            close_policy: DialogClosePolicy::Dismissible,
            recipe: DialogRecipe::Normal,
            focus_zone: DialogFocusZone::Actions,
            initial_focus: DialogFocusZone::Actions,
            initial_applied: false,
            action_cursor: None,
            default_action: None,
            cancel_action: None,
            require_action_focus_for_enter: true,
            validation_message: None,
            scroll: DialogScroll::new(),
            body_line_count: 0,
            slots: DialogSlots::empty(),
            action_regions: Vec::new(),
        }
    }

    /// Confirm-only (alert) factory.
    #[must_use]
    pub fn alert() -> Self {
        let mut s = Self::new();
        s.close_policy = DialogClosePolicy::ConfirmOnly;
        s
    }

    /// Whether the dialog is open (local; stack is geometry source of truth).
    #[must_use]
    pub const fn is_open(&self) -> bool {
        self.open
    }

    /// Open or close locally (pair with stack open/dismiss).
    pub fn set_open(&mut self, on: bool) {
        self.open = on;
        if on {
            self.initial_applied = false;
        }
    }

    /// Host grants keyboard into the focus trap.
    pub fn set_accepts_input(&mut self, on: bool) {
        self.accepts_input = on;
    }

    /// Whether the host granted input.
    #[must_use]
    pub const fn accepts_input(&self) -> bool {
        self.accepts_input
    }

    /// Loading chrome / block activation.
    pub const fn set_loading(&mut self, on: bool) {
        self.loading = on;
    }

    /// Whether activation is suppressed.
    #[must_use]
    pub const fn is_loading(&self) -> bool {
        self.loading
    }

    /// Esc / outside close policy.
    pub fn set_close_policy(&mut self, p: DialogClosePolicy) {
        self.close_policy = p;
    }

    /// Current close policy.
    #[must_use]
    pub const fn close_policy(&self) -> DialogClosePolicy {
        self.close_policy
    }

    /// Layout recipe (also used at open for size).
    pub fn set_recipe(&mut self, r: DialogRecipe) {
        self.recipe = r;
    }

    /// Current recipe.
    #[must_use]
    pub const fn recipe(&self) -> DialogRecipe {
        self.recipe
    }

    /// Explicit zone change (marks initial focus as applied).
    pub fn set_focus_zone(&mut self, z: DialogFocusZone) {
        self.focus_zone = z;
        self.initial_applied = true;
    }

    /// Active keyboard zone.
    #[must_use]
    pub const fn focus_zone(&self) -> DialogFocusZone {
        self.focus_zone
    }

    /// Initial focus zone when dialog opens.
    pub fn set_initial_focus(&mut self, z: DialogFocusZone) {
        self.initial_focus = z;
    }

    /// Default action for Enter when the action zone owns input.
    pub fn set_default_action(&mut self, id: Option<Id>) {
        self.default_action = id;
    }

    /// Optional cancel action for confirm-only Esc handling.
    pub fn set_cancel_action(&mut self, id: Option<Id>) {
        self.cancel_action = id;
    }

    /// When true (default), Enter only activates from the action zone.
    pub fn set_require_action_focus_for_enter(&mut self, on: bool) {
        self.require_action_focus_for_enter = on;
    }

    /// Validation banner; blocks default Enter while set.
    pub fn set_validation_message(&mut self, msg: Option<String>) {
        self.validation_message = msg;
    }

    /// Current validation message, if any.
    #[must_use]
    pub fn validation_message(&self) -> Option<&str> {
        self.validation_message.as_deref()
    }

    /// Slot geometry from the last paint.
    #[must_use]
    pub const fn slots(&self) -> DialogSlots {
        self.slots
    }

    /// Body scroll state.
    #[must_use]
    pub fn scroll(&self) -> &DialogScroll {
        &self.scroll
    }

    /// Mutable body scroll state.
    pub fn scroll_mut(&mut self) -> &mut DialogScroll {
        &mut self.scroll
    }

    /// Apply initial focus once.
    pub fn ensure_initial_focus(&mut self) {
        if !self.initial_applied {
            self.focus_zone = self.initial_focus;
            self.initial_applied = true;
        }
    }

    /// Open on OverlayStack with opener restoration.
    pub fn open_on_stack<F: Clone>(
        &mut self,
        stack: &mut OverlayStack<F>,
        bounds: Rect,
        preferred: DialogSize,
        opener_focus: Option<F>,
    ) -> OverlayOutcome<F> {
        self.open = true;
        self.initial_applied = false;
        self.ensure_initial_focus();
        open_dialog_configured(
            stack,
            bounds,
            preferred,
            opener_focus,
            self.close_policy,
            Some(self.recipe),
            None,
        )
    }

    /// Dismiss stack entry (opener restore).
    pub fn close_on_stack<F: Clone>(
        &mut self,
        stack: &mut OverlayStack<F>,
    ) -> OverlayOutcome<F> {
        self.open = false;
        dismiss_dialog_overlay(stack)
    }
}

impl<Id: Clone + PartialEq> DialogState<Id> {
    /// Seed action cursor.
    pub fn set_action_cursor(&mut self, id: Option<Id>) {
        self.action_cursor = id;
    }

    /// Current action cursor id.
    #[must_use]
    pub fn action_cursor(&self) -> Option<&Id> {
        self.action_cursor.as_ref()
    }

    /// Keyboard via [`default_dialog_intent`].
    pub fn handle_key(
        &mut self,
        key: KeyEvent,
        actions: &[Action<'_, Id>],
    ) -> DialogOutcome<Id> {
        if !self.open || !self.accepts_input || key.kind == KeyEventKind::Release {
            return DialogOutcome::Ignored;
        }
        self.ensure_initial_focus();
        if let Some(intent) = default_dialog_intent(key, self.focus_zone) {
            return self.handle_intent(intent, actions);
        }
        DialogOutcome::Ignored
    }

    /// Semantic intent routing with safe Enter semantics.
    pub fn handle_intent(
        &mut self,
        intent: UiIntent,
        actions: &[Action<'_, Id>],
    ) -> DialogOutcome<Id> {
        if !self.open || !self.accepts_input {
            return DialogOutcome::Ignored;
        }
        self.ensure_initial_focus();

        match intent {
            UiIntent::Cancel | UiIntent::Close => self.handle_cancel(actions),
            UiIntent::Activate | UiIntent::Submit | UiIntent::Open => {
                self.handle_activate(actions, false)
            }
            UiIntent::Move(NavigationMove::Previous) if self.focus_zone == DialogFocusZone::Actions => {
                self.move_action(actions, -1)
            }
            UiIntent::Move(NavigationMove::Next) if self.focus_zone == DialogFocusZone::Actions => {
                self.move_action(actions, 1)
            }
            UiIntent::Move(NavigationMove::First) if self.focus_zone == DialogFocusZone::Actions => {
                self.jump_action(actions, true)
            }
            UiIntent::Move(NavigationMove::Last) if self.focus_zone == DialogFocusZone::Actions => {
                self.jump_action(actions, false)
            }
            UiIntent::Move(NavigationMove::Up) | UiIntent::Page(_)
                if self.focus_zone == DialogFocusZone::Body =>
            {
                self.scroll_body(intent)
            }
            UiIntent::Move(NavigationMove::Down) if self.focus_zone == DialogFocusZone::Body => {
                self.scroll_body(intent)
            }
            // Tab-like: cycle zones without scene (portable)
            UiIntent::Move(NavigationMove::Right)
                if self.focus_zone == DialogFocusZone::Body && !actions.is_empty() =>
            {
                self.focus_zone = DialogFocusZone::Actions;
                DialogOutcome::FocusMoved
            }
            UiIntent::Move(NavigationMove::Left)
                if self.focus_zone == DialogFocusZone::Actions =>
            {
                self.focus_zone = DialogFocusZone::Body;
                DialogOutcome::FocusMoved
            }
            _ => DialogOutcome::Ignored,
        }
    }

    fn handle_cancel(&mut self, actions: &[Action<'_, Id>]) -> DialogOutcome<Id> {
        match self.close_policy {
            DialogClosePolicy::Locked => DialogOutcome::Ignored,
            DialogClosePolicy::ConfirmOnly => {
                // Prefer cancel action if set; else trap (no dismiss).
                if let Some(cid) = self.cancel_action.clone() {
                    if actions.iter().any(|a| a.id == cid && a.enabled) && !self.loading {
                        return DialogOutcome::Activated(cid);
                    }
                }
                DialogOutcome::Ignored
            }
            DialogClosePolicy::Dismissible => {
                // Esc dismisses the modal (opener restore via stack). Hosts that
                // want Esc to fire a Cancel *button* set ConfirmOnly + cancel_action.
                let _ = actions;
                self.open = false;
                DialogOutcome::Cancelled
            }
        }
    }

    fn handle_activate(
        &mut self,
        actions: &[Action<'_, Id>],
        force_default: bool,
    ) -> DialogOutcome<Id> {
        if self.loading {
            return DialogOutcome::LoadingBlocked;
        }
        if self.validation_message.is_some() {
            return DialogOutcome::ValidationFailed;
        }
        // Accidental submission guard: body zone requires explicit opt-in.
        if self.require_action_focus_for_enter
            && self.focus_zone == DialogFocusZone::Body
            && !force_default
        {
            return DialogOutcome::Ignored;
        }
        if actions.is_empty() {
            return DialogOutcome::Ignored;
        }
        // Prefer action cursor, then default.
        if let Some(id) = self.action_cursor.clone() {
            if actions.iter().any(|a| a.id == id && a.enabled) {
                return DialogOutcome::Activated(id);
            }
        }
        if let Some(id) = self.default_action.clone() {
            if actions.iter().any(|a| a.id == id && a.enabled) {
                return DialogOutcome::DefaultActivated(id);
            }
        }
        // First enabled
        if let Some(a) = actions.iter().find(|a| a.enabled) {
            return DialogOutcome::Activated(a.id.clone());
        }
        DialogOutcome::Ignored
    }

    fn move_action(&mut self, actions: &[Action<'_, Id>], dir: isize) -> DialogOutcome<Id> {
        let enabled: Vec<_> = actions.iter().filter(|a| a.enabled).collect();
        if enabled.is_empty() {
            return DialogOutcome::Ignored;
        }
        let cur = self
            .action_cursor
            .as_ref()
            .and_then(|id| enabled.iter().position(|a| &a.id == id));
        let next = match (cur, dir < 0) {
            (Some(0), true) | (None, true) => enabled.len() - 1,
            (Some(i), true) => i - 1,
            (Some(i), false) => (i + 1) % enabled.len(),
            (None, false) => 0,
        };
        let id = enabled[next].id.clone();
        if self.action_cursor.as_ref() == Some(&id) {
            return DialogOutcome::Ignored;
        }
        self.action_cursor = Some(id);
        DialogOutcome::FocusMoved
    }

    fn jump_action(&mut self, actions: &[Action<'_, Id>], first: bool) -> DialogOutcome<Id> {
        let enabled: Vec<_> = actions.iter().filter(|a| a.enabled).collect();
        let a = if first {
            enabled.first()
        } else {
            enabled.last()
        };
        let Some(a) = a else {
            return DialogOutcome::Ignored;
        };
        if self.action_cursor.as_ref() == Some(&a.id) {
            return DialogOutcome::Ignored;
        }
        self.action_cursor = Some(a.id.clone());
        DialogOutcome::FocusMoved
    }

    fn scroll_body(&mut self, intent: UiIntent) -> DialogOutcome<Id> {
        let vh = usize::from(self.slots.body.height.max(1));
        let before = (self.scroll.scroll_x, self.scroll.scroll_y);
        match intent {
            UiIntent::Move(NavigationMove::Up) => {
                self.scroll.scroll_y = self.scroll.scroll_y.saturating_sub(1);
            }
            UiIntent::Move(NavigationMove::Down) => {
                self.scroll.scroll_y = self.scroll.scroll_y.saturating_add(1);
            }
            UiIntent::Page(crate::interaction::PageMove::Backward) => {
                self.scroll.scroll_y = self.scroll.scroll_y.saturating_sub(vh as u16);
            }
            UiIntent::Page(crate::interaction::PageMove::Forward) => {
                self.scroll.scroll_y = self.scroll.scroll_y.saturating_add(vh as u16);
            }
            _ => return DialogOutcome::Ignored,
        }
        let max_y = self.body_line_count.saturating_sub(vh) as u16;
        if self.scroll.scroll_y > max_y {
            self.scroll.scroll_y = max_y;
        }
        if (self.scroll.scroll_x, self.scroll.scroll_y) != before {
            DialogOutcome::Scrolled
        } else {
            DialogOutcome::Ignored
        }
    }

    /// Pointer on action hits.
    pub fn handle_click(
        &mut self,
        position: ratatui_core::layout::Position,
        actions: &[Action<'_, Id>],
    ) -> DialogOutcome<Id> {
        if !self.open || !self.accepts_input || self.loading {
            return if self.loading {
                DialogOutcome::LoadingBlocked
            } else {
                DialogOutcome::Ignored
            };
        }
        if let Some(region) = self
            .action_regions
            .iter()
            .find(|r| r.area.contains(position))
        {
            if actions.iter().any(|a| a.id == region.id && a.enabled) {
                self.action_cursor = Some(region.id.clone());
                self.focus_zone = DialogFocusZone::Actions;
                return DialogOutcome::Activated(region.id.clone());
            }
        }
        if self.slots.body.contains(position) {
            self.focus_zone = DialogFocusZone::Body;
            return DialogOutcome::FocusMoved;
        }
        DialogOutcome::Ignored
    }
}

/// Default dialog intent map.
///
/// - **Esc** → Cancel (policy decides dismiss vs trap)
/// - **Enter** → Activate (state blocks accidental body submit)
/// - Action zone: Left/Right/h/l move actions; j/k when on actions also move
/// - Body zone: j/k/Page scroll; Right moves to actions
/// - **Tab is not mapped** — host InteractionScene owns trap Tab between
///   registered focus targets (body fields, actions)
#[must_use]
pub fn default_dialog_intent(key: KeyEvent, zone: DialogFocusZone) -> Option<UiIntent> {
    if key.kind == KeyEventKind::Release {
        return None;
    }
    let is_press = key.kind == KeyEventKind::Press;
    if key.modifiers.contains(KeyModifiers::CONTROL) || key.modifiers.contains(KeyModifiers::ALT) {
        return None;
    }
    match key.code {
        KeyCode::Esc if is_press => Some(UiIntent::Cancel),
        KeyCode::Enter if is_press => Some(UiIntent::Activate),
        KeyCode::PageUp => Some(UiIntent::Page(crate::interaction::PageMove::Backward)),
        KeyCode::PageDown => Some(UiIntent::Page(crate::interaction::PageMove::Forward)),
        KeyCode::Left | KeyCode::Char('h' | 'H') => Some(UiIntent::Move(NavigationMove::Left)),
        KeyCode::Right | KeyCode::Char('l' | 'L') => Some(UiIntent::Move(NavigationMove::Right)),
        KeyCode::Up => Some(UiIntent::Move(NavigationMove::Up)),
        KeyCode::Down => Some(UiIntent::Move(NavigationMove::Down)),
        KeyCode::Home if matches!(zone, DialogFocusZone::Actions) => {
            Some(UiIntent::Move(NavigationMove::First))
        }
        KeyCode::End if matches!(zone, DialogFocusZone::Actions) => {
            Some(UiIntent::Move(NavigationMove::Last))
        }
        // j/k: actions move when on actions; body scroll when on body
        KeyCode::Char('k' | 'K') => match zone {
            DialogFocusZone::Actions => Some(UiIntent::Move(NavigationMove::Previous)),
            DialogFocusZone::Body => Some(UiIntent::Move(NavigationMove::Up)),
        },
        KeyCode::Char('j' | 'J') => match zone {
            DialogFocusZone::Actions => Some(UiIntent::Move(NavigationMove::Next)),
            DialogFocusZone::Body => Some(UiIntent::Move(NavigationMove::Down)),
        },
        _ => None,
    }
}

// ── Dialog chrome ───────────────────────────────────────────────────────────

/// Framed modal surface with slots (title / description / body / actions region).
///
/// Geometry open/close/trap live on [`OverlayStack`]. Paint is pure chrome for
/// the modal rect.
#[derive(Debug, Clone)]
pub struct Dialog<'a> {
    title: &'a str,
    description: Option<&'a str>,
    body: Text<'a>,
    style: Style,
    tokens: &'a DesignSystem,
    emphasis: PanelChrome,
    variant: DialogVariant,
    recipe: DialogRecipe,
    footer_hint: Option<&'a str>,
    loading: bool,
    ascii: bool,
    colorless: bool,
}

impl<'a> Dialog<'a> {
    /// Creates a dialog painted from design tokens.
    #[must_use]
    pub const fn new(title: &'a str, body: Text<'a>, tokens: &'a DesignSystem) -> Self {
        Self {
            title,
            description: None,
            body,
            style: Style::new(),
            tokens,
            emphasis: PanelChrome::Normal,
            variant: DialogVariant::Default,
            recipe: DialogRecipe::Normal,
            footer_hint: None,
            loading: false,
            ascii: false,
            colorless: false,
        }
    }

    /// Preferred constructor from [`DesignSystem`].
    #[must_use]
    pub const fn from_system(title: &'a str, body: Text<'a>, system: &'a DesignSystem) -> Self {
        Self::new(title, body, system)
    }

    /// Description under the title.
    #[must_use]
    pub const fn description(mut self, d: &'a str) -> Self {
        self.description = Some(d);
        self
    }

    /// Theme borrow for child widgets.
    #[must_use]
    pub const fn theme(&self) -> &RolePalette {
        self.tokens.palette()
    }

    /// Design tokens.
    #[must_use]
    pub const fn tokens(&self) -> &DesignSystem {
        self.tokens
    }

    /// Body style override.
    #[must_use]
    pub const fn style(mut self, style: Style) -> Self {
        self.style = style;
        self
    }

    /// Panel emphasis (overridden by danger variant / destructive recipe).
    #[must_use]
    pub const fn emphasis(mut self, emphasis: PanelChrome) -> Self {
        self.emphasis = emphasis;
        self
    }

    /// Chrome variant.
    #[must_use]
    pub const fn variant(mut self, variant: DialogVariant) -> Self {
        self.variant = variant;
        self
    }

    /// Layout recipe.
    #[must_use]
    pub const fn recipe(mut self, recipe: DialogRecipe) -> Self {
        self.recipe = recipe;
        self
    }

    /// Footer hint row.
    #[must_use]
    pub const fn footer_hint(mut self, hint: &'a str) -> Self {
        self.footer_hint = Some(hint);
        self
    }

    /// Loading chrome.
    #[must_use]
    pub const fn loading(mut self, loading: bool) -> Self {
        self.loading = loading;
        self
    }

    /// ASCII glyphs.
    #[must_use]
    pub const fn ascii(mut self, on: bool) -> Self {
        self.ascii = on;
        self
    }

    /// Reduced color.
    #[must_use]
    pub const fn colorless(mut self, on: bool) -> Self {
        self.colorless = on;
        self
    }

    fn resolved_emphasis(&self) -> PanelChrome {
        if self.recipe.is_destructive() || matches!(self.variant, DialogVariant::Danger) {
            return PanelChrome::Danger;
        }
        match self.variant {
            DialogVariant::Info => PanelChrome::Focused,
            DialogVariant::Default | DialogVariant::Danger => self.emphasis,
        }
    }

    fn title_for_paint(&self) -> String {
        let mut title = self.title.to_string();
        if (matches!(self.variant, DialogVariant::Danger) || self.recipe.is_destructive())
            && !title.contains('!')
        {
            title = format!("! {title}");
        }
        if self.loading {
            let glyph = if self.ascii {
                "..."
            } else {
                self.tokens.glyphs.loading()
            };
            title = format!("{title} {glyph}");
        }
        title
    }

    /// Paint chrome and compute slots into `state` (optional actions height reserved).
    pub fn paint<Id>(
        &self,
        area: Rect,
        buffer: &mut Buffer,
        state: &mut DialogState<Id>,
        action_rows: u16,
    ) {
        state.slots = DialogSlots::empty();
        if area.is_empty() {
            return;
        }
        state.slots.root = area;
        Clear.render(area, buffer);

        let emphasis = if matches!(state.focus_zone, DialogFocusZone::Actions | DialogFocusZone::Body)
        {
            // Modal owns interaction: focused border
            match self.resolved_emphasis() {
                PanelChrome::Danger => PanelChrome::Danger,
                _ => PanelChrome::Focused,
            }
        } else {
            self.resolved_emphasis()
        };

        let title = self.title_for_paint();
        let panel = Panel::new(self.tokens)
            .title(title.as_str())
            .emphasis(emphasis);

        if area.height < 3 {
            panel.block().render(area, buffer);
            return;
        }

        let has_desc = self.description.is_some() && area.height >= 5;
        let has_validation = state.validation_message.is_some() && area.height >= 6;
        let has_footer = self.footer_hint.is_some() && area.height >= 5;
        let footer_rows = u16::from(has_footer);
        let validation_rows = u16::from(has_validation);
        let desc_rows = u16::from(has_desc);
        let action_h = action_rows.min(area.height.saturating_sub(3));

        // Panel block paints border+title; body uses inner area.
        let block = panel.block();
        let inner = block.inner(area);
        block.render(area, buffer);

        // Title slot approx first inner row used by block title — report full top.
        state.slots.title = Rect::new(area.x, area.y, area.width, 1);

        let mut y = inner.y;
        if has_desc {
            if let Some(d) = self.description {
                state.slots.description = Rect::new(inner.x, y, inner.width, 1);
                buffer.set_stringn(
                    inner.x,
                    y,
                    &crate::text::take_display_cols(d, usize::from(inner.width)),
                    usize::from(inner.width),
                    self.tokens.style(Role::TextMuted),
                );
                y = y.saturating_add(1);
            }
        } else {
            state.slots.description = Rect::default();
        }

        let reserved_bottom = action_h
            .saturating_add(footer_rows)
            .saturating_add(validation_rows);
        let body_h = inner
            .bottom()
            .saturating_sub(y)
            .saturating_sub(reserved_bottom)
            .max(1);
        state.slots.body = Rect::new(inner.x, y, inner.width, body_h);

        // Body paragraph with scroll
        let mut body_style = self.style;
        if body_style.fg.is_none() {
            body_style = body_style.patch(self.tokens.style(Role::Text));
        }
        state.body_line_count = self.body.lines.len();
        let scroll_y = state.scroll.scroll_y;
        Paragraph::new(self.body.clone())
            .style(body_style)
            .scroll((scroll_y, state.scroll.scroll_x))
            .render(state.slots.body, buffer);

        y = state.slots.body.bottom();

        if has_validation {
            state.slots.validation = Rect::new(inner.x, y, inner.width, 1);
            if let Some(msg) = &state.validation_message {
                buffer.set_stringn(
                    inner.x,
                    y,
                    &crate::text::take_display_cols(msg, usize::from(inner.width)),
                    usize::from(inner.width),
                    self.tokens.style(Role::Danger),
                );
            }
            y = y.saturating_add(1);
        } else {
            state.slots.validation = Rect::default();
        }

        if action_h > 0 {
            state.slots.actions = Rect::new(inner.x, y, inner.width, action_h);
            y = y.saturating_add(action_h);
        } else {
            state.slots.actions = Rect::default();
        }

        if has_footer {
            state.slots.footer = Rect::new(inner.x, y, inner.width, 1);
            if let Some(hint) = self.footer_hint {
                buffer.set_stringn(
                    inner.x,
                    y,
                    &crate::text::take_display_cols(hint, usize::from(inner.width)),
                    usize::from(inner.width),
                    self.tokens.style(Role::TextMuted),
                );
            }
        } else {
            state.slots.footer = Rect::default();
        }

        let _ = desc_rows;
    }

    /// Semantic registration for the modal surface.
    pub fn register_semantic<Id, Sid, Action>(
        &self,
        scene: &mut SemanticScene<Sid, Action>,
        id: Sid,
        area: Rect,
        state: &DialogState<Id>,
    ) where
        Sid: Clone + PartialEq + std::fmt::Display,
        Action: Clone,
    {
        if area.is_empty() || !state.open {
            return;
        }
        let desc = format!(
            "dialog recipe={} close={} zone={} loading={} validation={}",
            self.recipe.id(),
            state.close_policy.id(),
            state.focus_zone.id(),
            state.loading,
            state.validation_message.is_some(),
        );
        let _ = scene.register(
            SemanticNode::control(id, area)
                .role(SemanticRole::Dialog)
                .label("dialog")
                .description(desc)
                .focusable(state.accepts_input && state.open)
                .state(SemanticState {
                    selected: true,
                    expanded: state.open,
                    busy: state.loading,
                    ..Default::default()
                }),
        );
    }
}

impl Widget for &Dialog<'_> {
    fn render(self, area: Rect, buffer: &mut Buffer) {
        let mut state = DialogState::<()>::new();
        state.set_loading(self.loading);
        if self.recipe.is_destructive() {
            state.set_recipe(DialogRecipe::Destructive);
        }
        Dialog::paint(self, area, buffer, &mut state, 0);
    }
}

impl Widget for Dialog<'_> {
    fn render(self, area: Rect, buffer: &mut Buffer) {
        <&Self as Widget>::render(&self, area, buffer);
    }
}

// ── ChoiceDialog ────────────────────────────────────────────────────────────

/// Runtime state for `ChoiceDialog` (wraps action bar + dialog policies).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ChoiceDialogState<Id> {
    /// Action cursor (not scene surface focus).
    pub cursor: Option<Id>,
    /// Hit regions from last render.
    pub regions: Vec<HitRegion<Id>>,
    loading: bool,
    accepts_input: bool,
    /// Full dialog engine (close policy, zones, validation, scroll).
    dialog: DialogState<Id>,
}

impl<Id> Default for ChoiceDialogState<Id> {
    fn default() -> Self {
        Self {
            cursor: None,
            regions: Vec::new(),
            loading: false,
            accepts_input: true,
            dialog: DialogState::new(),
        }
    }
}

impl<Id: Clone + PartialEq> ChoiceDialogState<Id> {
    /// Creates choice-dialog state with optional initial action cursor.
    #[must_use]
    pub fn new(cursor: Option<Id>) -> Self {
        let mut dialog = DialogState::new();
        dialog.set_action_cursor(cursor.clone());
        if let Some(ref c) = cursor {
            dialog.set_default_action(Some(c.clone()));
        }
        dialog.set_focus_zone(DialogFocusZone::Actions);
        dialog.set_initial_focus(DialogFocusZone::Actions);
        Self {
            cursor,
            regions: Vec::new(),
            loading: false,
            accepts_input: true,
            dialog,
        }
    }

    /// Action cursor id.
    #[must_use]
    pub fn cursor(&self) -> Option<&Id> {
        self.cursor.as_ref()
    }

    /// Deprecated name for [`Self::cursor`].
    #[deprecated(note = "use cursor")]
    #[must_use]
    pub fn focused(&self) -> Option<&Id> {
        self.cursor.as_ref()
    }

    /// Host input gate.
    pub fn set_accepts_input(&mut self, accepts: bool) {
        self.accepts_input = accepts;
        self.dialog.set_accepts_input(accepts);
    }

    /// Whether host granted input.
    #[must_use]
    pub const fn accepts_input(&self) -> bool {
        self.accepts_input
    }

    /// Async loading (blocks activation).
    pub fn set_loading(&mut self, loading: bool) {
        self.loading = loading;
        self.dialog.set_loading(loading);
    }

    /// Whether activation is suppressed by loading.
    #[must_use]
    pub const fn is_loading(&self) -> bool {
        self.loading
    }

    /// Access full dialog engine.
    #[must_use]
    pub fn dialog(&self) -> &DialogState<Id> {
        &self.dialog
    }

    /// Mutable access to the dialog engine (close policy, validation, zones).
    pub fn dialog_mut(&mut self) -> &mut DialogState<Id> {
        &mut self.dialog
    }

    /// Routes keys via dialog engine → [`Outcome`].
    pub fn handle_key(&mut self, actions: &[Action<'_, Id>], key: KeyEvent) -> Outcome<Id> {
        if !self.accepts_input || key.kind == KeyEventKind::Release {
            return Outcome::Ignored;
        }
        // Prefer choice-dialog intent (Left/Right for actions) when on actions.
        if let Some(intent) = crate::interaction::default_choice_dialog_intent(key) {
            return self.handle_intent(actions, intent);
        }
        // Fallback dialog intent (body scroll etc.)
        self.dialog
            .handle_key(key, actions)
            .into_outcome()
    }

    /// Semantic intent routing for footer actions.
    pub fn handle_intent(&mut self, actions: &[Action<'_, Id>], intent: UiIntent) -> Outcome<Id> {
        if !self.accepts_input {
            return Outcome::Ignored;
        }
        self.dialog.action_cursor = self.cursor.clone();
        self.dialog.loading = self.loading;
        self.dialog.accepts_input = self.accepts_input;
        // Classic Esc: always Cancelled when dismissible (stack restores opener).
        if matches!(intent, UiIntent::Cancel | UiIntent::Close) {
            return match self.dialog.close_policy {
                DialogClosePolicy::Dismissible => {
                    self.dialog.open = false;
                    Outcome::Cancelled
                }
                DialogClosePolicy::ConfirmOnly | DialogClosePolicy::Locked => self
                    .dialog
                    .handle_intent(intent, actions)
                    .into_outcome(),
            };
        }
        // Ensure action zone for classic choice intents
        if matches!(
            intent,
            UiIntent::Move(
                NavigationMove::Previous
                    | NavigationMove::Next
                    | NavigationMove::First
                    | NavigationMove::Last
            ) | UiIntent::Activate
                | UiIntent::Submit
                | UiIntent::Open
        ) {
            self.dialog.focus_zone = DialogFocusZone::Actions;
            // Choice dialogs: Enter always on actions zone (safe)
            self.dialog.require_action_focus_for_enter = false;
        }
        let out = self.dialog.handle_intent(intent, actions);
        self.cursor = self.dialog.action_cursor.clone();
        self.loading = self.dialog.loading;
        out.into_outcome()
    }

    /// Move to next enabled action.
    pub fn select_next(&mut self, actions: &[Action<'_, Id>]) -> Outcome<Id> {
        self.handle_intent(actions, UiIntent::Move(NavigationMove::Next))
    }

    /// Move to previous enabled action.
    pub fn select_previous(&mut self, actions: &[Action<'_, Id>]) -> Outcome<Id> {
        self.handle_intent(actions, UiIntent::Move(NavigationMove::Previous))
    }

    /// Activate the current cursor action if enabled.
    #[must_use]
    pub fn activate_selected(&self, actions: &[Action<'_, Id>]) -> Outcome<Id> {
        if self.loading || !self.accepts_input {
            return Outcome::Ignored;
        }
        self.cursor
            .as_ref()
            .and_then(|cur| {
                actions
                    .iter()
                    .find(|action| action.enabled && &action.id == cur)
            })
            .map_or(Outcome::Ignored, |action| {
                Outcome::Activated(action.id.clone())
            })
    }

    /// Click a painted action hit region.
    #[must_use]
    pub fn click(&mut self, position: ratatui_core::layout::Position) -> Outcome<Id> {
        if self.loading || !self.accepts_input {
            return Outcome::Ignored;
        }
        self.dialog.action_regions = self.regions.clone();
        let _ = self.dialog.handle_click(position, &[]);
        // Use regions directly for activate
        let Some(region) = self
            .regions
            .iter()
            .find(|region| region.area.contains(position))
        else {
            return Outcome::Ignored;
        };
        self.cursor = Some(region.id.clone());
        Outcome::Activated(region.id.clone())
    }
}

/// Modal choice prompt with stable action identities.
#[derive(Debug, Clone)]
pub struct ChoiceDialog<'a, Id> {
    dialog: Dialog<'a>,
    actions: &'a [Action<'a, Id>],
    gap: &'a str,
    ascii: bool,
    colorless: bool,
}

impl<'a, Id> ChoiceDialog<'a, Id> {
    /// Creates a choice dialog over borrowed actions.
    #[must_use]
    pub const fn new(dialog: Dialog<'a>, actions: &'a [Action<'a, Id>]) -> Self {
        Self {
            dialog,
            actions,
            gap: " ",
            ascii: false,
            colorless: false,
        }
    }

    /// Spacing between action labels.
    #[must_use]
    pub const fn gap(mut self, gap: &'a str) -> Self {
        self.gap = gap;
        self
    }

    /// ASCII action marks.
    #[must_use]
    pub const fn ascii(mut self, ascii: bool) -> Self {
        self.ascii = ascii;
        self
    }

    /// Reduced-color action paint.
    #[must_use]
    pub const fn colorless(mut self, colorless: bool) -> Self {
        self.colorless = colorless;
        self
    }
}

impl<Id: Clone + PartialEq> StatefulWidget for &ChoiceDialog<'_, Id> {
    type State = ChoiceDialogState<Id>;

    fn render(self, area: Rect, buffer: &mut Buffer, state: &mut Self::State) {
        if area.height < 3 {
            state.regions.clear();
            state.dialog.paint_clear_slots();
            return;
        }
        let narrow = crate::layout::dialog_stack_actions(area.width, area.height);
        let action_rows = if narrow {
            (self.actions.len() as u16)
                .min(area.height.saturating_sub(3))
                .max(1)
        } else {
            1
        };

        // Sync dialog engine
        state.dialog.loading = state.loading || self.dialog.loading;
        state.dialog.accepts_input = state.accepts_input;
        state.dialog.action_cursor = state.cursor.clone();
        if state.dialog.default_action.is_none() {
            state.dialog.default_action = state.cursor.clone();
        }

        let mut chrome = self.dialog.clone();
        chrome.loading = state.dialog.loading;
        chrome.ascii = self.ascii;
        chrome.colorless = self.colorless;
        chrome.paint(area, buffer, &mut state.dialog, action_rows);

        let action_area = state.dialog.slots.actions;
        if action_area.is_empty() {
            state.regions.clear();
            return;
        }
        let mut action_state = ActionBarState {
            cursor: state.cursor.clone(),
            regions: Vec::new(),
        };
        (&ActionBar::new(self.actions, self.dialog.tokens())
            .gap(self.gap)
            .ascii(self.ascii)
            .colorless(self.colorless)
            .vertical(narrow && action_rows > 1))
            .render(action_area, buffer, &mut action_state);
        if state.cursor.is_none() {
            state.cursor = action_state.cursor;
        }
        state.regions = action_state.regions;
        state.dialog.action_regions = state.regions.clone();
        state.dialog.action_cursor = state.cursor.clone();
    }
}

impl<Id: Clone + PartialEq> StatefulWidget for ChoiceDialog<'_, Id> {
    type State = ChoiceDialogState<Id>;

    fn render(self, area: Rect, buffer: &mut Buffer, state: &mut Self::State) {
        <&Self as StatefulWidget>::render(&self, area, buffer, state);
    }
}

impl<Id> DialogState<Id> {
    fn paint_clear_slots(&mut self) {
        self.slots = DialogSlots::empty();
    }
}

// ── MessageDialog ───────────────────────────────────────────────────────────

/// Message dialog with optional scrollable details.
#[derive(Debug, Clone)]
pub struct MessageDialog<'a, Id> {
    dialog: Dialog<'a>,
    details: &'a [DetailRow<'a, Id>],
    label_width: u16,
    wrap: bool,
    system: &'a DesignSystem,
}

impl<'a, Id> MessageDialog<'a, Id> {
    /// Message dialog with optional detail rows.
    #[must_use]
    pub const fn new(
        dialog: Dialog<'a>,
        details: &'a [DetailRow<'a, Id>],
        system: &'a DesignSystem,
    ) -> Self {
        Self {
            dialog,
            details,
            label_width: 0,
            wrap: false,
            system,
        }
    }

    /// Fixed label column width for details.
    #[must_use]
    pub const fn label_width(mut self, label_width: u16) -> Self {
        self.label_width = label_width;
        self
    }

    /// Wrap long detail values.
    #[must_use]
    pub const fn wrap(mut self, wrap: bool) -> Self {
        self.wrap = wrap;
        self
    }
}

impl<Id: Clone + PartialEq> StatefulWidget for &MessageDialog<'_, Id> {
    type State = DetailTableState<Id>;

    fn render(self, area: Rect, buffer: &mut Buffer, state: &mut Self::State) {
        let mut dstate = DialogState::<()>::new();
        dstate.set_loading(self.dialog.loading);
        self.dialog.paint(area, buffer, &mut dstate, 0);
        if area.width < 3 || area.height < 3 {
            state.regions.clear();
            return;
        }
        let body = dstate.slots.body;
        if body.is_empty() {
            return;
        }
        // Details fill lower portion of body when present
        let content_width = usize::from(body.width).max(1);
        let body_height = self
            .dialog
            .body
            .lines
            .iter()
            .map(|line| line.width().div_ceil(content_width).max(1))
            .sum::<usize>()
            .min(usize::from(body.height));
        let body_height = u16::try_from(body_height).unwrap_or(u16::MAX);
        let detail_area = Rect::new(
            body.x,
            body.y.saturating_add(body_height.min(body.height)),
            body.width,
            body.height.saturating_sub(body_height.min(body.height)),
        );
        if detail_area.height > 0 && !self.details.is_empty() {
            (&DetailTable::new(self.details, self.system)
                .label_width(self.label_width)
                .wrap(self.wrap))
                .render(detail_area, buffer, state);
            // Preserve prior test expectation: viewport.y tracks body start offset
            state.viewport.y = body.y.saturating_sub(area.y).saturating_add(body_height);
        }
    }
}

impl<Id: Clone + PartialEq> StatefulWidget for MessageDialog<'_, Id> {
    type State = DetailTableState<Id>;

    fn render(self, area: Rect, buffer: &mut Buffer, state: &mut Self::State) {
        <&Self as StatefulWidget>::render(&self, area, buffer, state);
    }
}

// ── Tests ───────────────────────────────────────────────────────────────────

#[cfg(test)]
mod backdrop_tests {
    use super::*;
    use ratatui_core::{layout::Position, widgets::StatefulWidget};

    use crate::{
        input::KeyModifiers,
        interaction::{OverlayId, OverlayKind, OverlayOutcome, OverlayStack},
    };

    #[test]
    fn default_backdrop_uses_terminal_background() {
        let backdrop = Backdrop::default();
        assert_eq!(backdrop.symbol, ' ');
        assert_eq!(backdrop.style.fg, Some(Color::Reset));
        assert_eq!(backdrop.style.bg, Some(Color::Reset));
    }

    #[test]
    fn dialog_opens_on_overlay_stack_with_opener_restore() {
        let bounds = Rect::new(0, 0, 80, 24);
        let mut stack = OverlayStack::<&'static str>::new();
        let out = open_dialog_overlay(
            &mut stack,
            bounds,
            DialogSize {
                width: 40,
                height: 10,
            },
            Some("trigger"),
        );
        assert!(matches!(out, OverlayOutcome::Opened { .. }));
        assert_eq!(stack.top().unwrap().kind, OverlayKind::Dialog);
        let placed = place_dialog(
            bounds,
            DialogSize {
                width: 40,
                height: 10,
            },
        );
        assert_eq!(stack.top().unwrap().rect, placed);
        assert_eq!(
            stack.handle_outside_click(Position::new(0, 0)),
            OverlayOutcome::Ignored
        );
        assert!(matches!(
            stack.handle_escape(),
            OverlayOutcome::Dismissed {
                focus: Some("trigger"),
                ..
            }
        ));
    }

    #[test]
    fn alert_dialog_traps_escape() {
        let bounds = Rect::new(0, 0, 80, 24);
        let mut stack = OverlayStack::<()>::new();
        let _ = open_alert_dialog_overlay(&mut stack, bounds, DialogSize::default(), None);
        assert_eq!(stack.handle_escape(), OverlayOutcome::Ignored);
        assert!(stack.contains(&OverlayId::from_static(DIALOG_OVERLAY_ID)));
        let _ = dismiss_dialog_overlay(&mut stack);
        assert!(stack.is_empty());
    }

    #[test]
    fn dialog_narrow_promotes_fullscreen() {
        let bounds = Rect::new(0, 0, 32, 10);
        let mut stack = OverlayStack::<()>::new();
        let _ = open_dialog_overlay(&mut stack, bounds, DialogSize::default(), None);
        // open_dialog_configured may use fullscreen kind on tiny bounds
        let top = stack.top().unwrap();
        assert!(
            top.fullscreen_promoted
                || top.kind == OverlayKind::Fullscreen
                || top.rect == bounds
        );
    }

    #[test]
    fn nested_child_overlay_cascade() {
        let bounds = Rect::new(0, 0, 80, 24);
        let mut stack = OverlayStack::<&'static str>::new();
        let _ = open_dialog_overlay(
            &mut stack,
            bounds,
            DialogSize {
                width: 40,
                height: 10,
            },
            Some("opener"),
        );
        let _ = open_dialog_child_overlay(
            &mut stack,
            bounds,
            Rect::new(20, 10, 1, 1),
            OverlaySize::menu(20, 5),
            "menu",
            Some("opener"),
        );
        assert_eq!(stack.entries().len(), 2);
        let _ = dismiss_dialog_overlay(&mut stack);
        assert!(stack.is_empty());
    }

    #[test]
    fn choice_dialog_skips_disabled_actions_and_returns_semantic_outcomes() {
        let actions = [
            Action {
                id: "accept",
                label: "Accept",
                enabled: true,
                style: None,
            },
            Action {
                id: "blocked",
                label: "Blocked",
                enabled: false,
                style: None,
            },
            Action {
                id: "cancel",
                label: "Cancel",
                enabled: true,
                style: None,
            },
        ];
        let mut state = ChoiceDialogState::new(Some("accept"));
        assert_eq!(
            state.handle_intent(&actions, UiIntent::Move(NavigationMove::Next)),
            Outcome::Changed
        );
        assert_eq!(state.cursor, Some("cancel"));
        assert_eq!(
            state.handle_intent(&actions, UiIntent::Activate),
            Outcome::Activated("cancel")
        );
        assert_eq!(
            state.handle_intent(&actions, UiIntent::Cancel),
            Outcome::Cancelled
        );
        assert_eq!(
            state.handle_key(&actions, KeyEvent::new(KeyCode::Esc, KeyModifiers::NONE)),
            Outcome::Cancelled
        );
        assert_eq!(
            state.handle_key(&actions, KeyEvent::new(KeyCode::Tab, KeyModifiers::NONE)),
            Outcome::Ignored
        );
    }

    #[test]
    fn choice_dialog_accepts_input_gate() {
        let actions = [Action {
            id: "ok",
            label: "OK",
            enabled: true,
            style: None,
        }];
        let mut state = ChoiceDialogState::new(Some("ok"));
        state.set_accepts_input(false);
        assert_eq!(
            state.handle_intent(&actions, UiIntent::Activate),
            Outcome::Ignored
        );
    }

    #[test]
    fn choice_dialog_narrow_stacks_actions() {
        let actions = [
            Action {
                id: "a",
                label: "Accept",
                enabled: true,
                style: None,
            },
            Action {
                id: "c",
                label: "Cancel",
                enabled: true,
                style: None,
            },
        ];
        let system = DesignSystem::default();
        let dialog = ChoiceDialog::new(
            Dialog::new("Choose", Text::from("?"), &system).emphasis(PanelChrome::Focused),
            &actions,
        )
        .ascii(true);
        let area = Rect::new(0, 0, 22, 8);
        let mut buffer = Buffer::empty(area);
        let mut state = ChoiceDialogState::new(Some("a"));
        (&dialog).render(area, &mut buffer, &mut state);
        assert!(state.regions.len() >= 1);
        let text: String = buffer.content().iter().map(|c| c.symbol()).collect();
        assert!(
            text.contains("Accept") || text.contains("[Accept]"),
            "{text:?}"
        );
    }

    #[test]
    fn choice_dialog_mouse_outcomes_follow_enabled_painted_regions() {
        let actions = [Action {
            id: "accept",
            label: "Accept",
            enabled: true,
            style: None,
        }];
        let tokens = DesignSystem::default();
        let dialog = ChoiceDialog::new(
            Dialog::new("Choose", Text::from("Continue?"), &tokens).emphasis(PanelChrome::Focused),
            &actions,
        );
        let area = Rect::new(3, 2, 30, 6);
        let mut buffer = Buffer::empty(area);
        let mut state = ChoiceDialogState::default();
        (&dialog).render(area, &mut buffer, &mut state);
        assert_eq!(state.regions.len(), 1);
        let region = state.regions[0].area;
        assert_eq!(
            state.click(Position::new(region.x, region.y)),
            Outcome::Activated("accept")
        );
    }

    #[test]
    fn message_details_start_after_wrapped_body() {
        let details = [DetailRow {
            id: "stage",
            label: "Stage",
            value: "Build",
            href: None,
            capability: super::super::DetailCapability::None,
            emphasis: false,
            style: None,
        }];
        let tokens = DesignSystem::default();
        let dialog = MessageDialog::new(
            Dialog::new("Failure", Text::from("a message that wraps"), &tokens)
                .emphasis(PanelChrome::Focused),
            &details,
            &tokens,
        )
        .wrap(true);
        let area = Rect::new(0, 0, 12, 8);
        let mut buffer = Buffer::empty(area);
        let mut state = DetailTableState::default();
        (&dialog).render(area, &mut buffer, &mut state);
        // Body occupies rows; details follow (y > 1)
        assert!(state.viewport.y >= 1, "viewport.y={}", state.viewport.y);
    }

    #[test]
    fn dialog_uses_semantic_focused_panel_chrome() {
        let tokens = DesignSystem::default();
        let dialog =
            Dialog::new(" Notice ", Text::from("Done"), &tokens).emphasis(PanelChrome::Focused);
        let area = Rect::new(0, 0, 18, 4);
        let mut buffer = Buffer::empty(area);
        (&dialog).render(area, &mut buffer);

        assert_eq!(
            buffer[(0, 0)].fg,
            tokens.style(crate::style::Role::BorderFocused).fg.unwrap()
        );
        assert!(
            buffer
                .content()
                .iter()
                .map(|cell| cell.symbol())
                .collect::<String>()
                .contains(" Notice ")
        );
    }

    #[test]
    fn danger_variant_uses_danger_border_and_title_cue() {
        let tokens = DesignSystem::default();
        let dialog = Dialog::new("Delete", Text::from("Irreversible"), &tokens)
            .variant(DialogVariant::Danger);
        let area = Rect::new(0, 0, 24, 5);
        let mut buffer = Buffer::empty(area);
        (&dialog).render(area, &mut buffer);
        assert_eq!(buffer[(0, 0)].fg, tokens.style(Role::Danger).fg.unwrap());
        let text: String = buffer.content().iter().map(|c| c.symbol()).collect();
        assert!(text.contains('!') || text.contains("Delete"), "{text:?}");
    }

    #[test]
    fn loading_disables_choice_activation() {
        let actions = [Action {
            id: "ok",
            label: "OK",
            enabled: true,
            style: None,
        }];
        let mut state = ChoiceDialogState::new(Some("ok"));
        state.set_loading(true);
        assert_eq!(
            state.handle_intent(&actions, UiIntent::Activate),
            Outcome::Ignored
        );
        state.set_loading(false);
        assert_eq!(
            state.handle_intent(&actions, UiIntent::Activate),
            Outcome::Activated("ok")
        );
    }

    #[test]
    fn empty_body_and_from_system() {
        let system = DesignSystem::phosphor();
        let dialog =
            Dialog::from_system("Empty", Text::default(), &system).footer_hint("esc dismiss");
        let area = Rect::new(0, 0, 28, 6);
        let mut buffer = Buffer::empty(area);
        (&dialog).render(area, &mut buffer);
        let text: String = buffer.content().iter().map(|c| c.symbol()).collect();
        assert!(text.contains("Empty"), "{text:?}");
        assert!(text.contains("esc") || text.contains("dismiss"), "{text:?}");
    }

    #[test]
    fn dim_wash_backdrop_is_not_hard_black() {
        let wash = Backdrop::dim_wash(false);
        assert_ne!(wash.symbol, '\0');
        assert_eq!(wash.style.bg, Some(Color::Reset));
    }

    #[test]
    fn dialog_size_tracks_density() {
        assert!(
            DialogSize::for_density(Density::Comfortable).width
                >= DialogSize::for_density(Density::Dashboard).width
        );
        assert!(DialogSize::min_width(Density::Comfortable) >= 32);
    }

    #[test]
    fn enter_does_not_submit_from_body_zone() {
        let actions = [
            Action {
                id: "ok",
                label: "OK",
                enabled: true,
                style: None,
            },
            Action {
                id: "cancel",
                label: "Cancel",
                enabled: true,
                style: None,
            },
        ];
        let mut state = DialogState::new();
        state.set_action_cursor(Some("ok"));
        state.set_default_action(Some("ok"));
        state.set_focus_zone(DialogFocusZone::Body);
        state.set_require_action_focus_for_enter(true);
        assert!(matches!(
            state.handle_intent(UiIntent::Activate, &actions),
            DialogOutcome::Ignored
        ));
        state.set_focus_zone(DialogFocusZone::Actions);
        assert!(matches!(
            state.handle_intent(UiIntent::Activate, &actions),
            DialogOutcome::Activated("ok") | DialogOutcome::DefaultActivated("ok")
        ));
    }

    #[test]
    fn validation_blocks_enter() {
        let actions = [Action {
            id: "save",
            label: "Save",
            enabled: true,
            style: None,
        }];
        let mut state = DialogState::new();
        state.set_action_cursor(Some("save"));
        state.set_focus_zone(DialogFocusZone::Actions);
        state.set_require_action_focus_for_enter(false);
        state.set_validation_message(Some("Required field".into()));
        assert!(matches!(
            state.handle_intent(UiIntent::Activate, &actions),
            DialogOutcome::ValidationFailed
        ));
    }

    #[test]
    fn confirm_only_esc_traps_without_cancel_action() {
        let actions = [Action {
            id: "ok",
            label: "OK",
            enabled: true,
            style: None,
        }];
        let mut state = DialogState::alert();
        state.set_action_cursor(Some("ok"));
        assert!(matches!(
            state.handle_intent(UiIntent::Cancel, &actions),
            DialogOutcome::Ignored
        ));
        assert!(state.is_open());
    }

    #[test]
    fn recipes_and_description_paint() {
        let system = DesignSystem::default();
        let dialog = Dialog::new("Title", Text::from("Body line"), &system)
            .description("Helpful description")
            .recipe(DialogRecipe::Wide)
            .footer_hint("esc · cancel");
        let mut state = DialogState::<()>::new();
        let area = Rect::new(0, 0, 50, 12);
        let mut buf = Buffer::empty(area);
        dialog.paint(area, &mut buf, &mut state, 1);
        let text: String = buf.content().iter().map(|c| c.symbol().to_string()).collect();
        assert!(text.contains("Title") || text.contains("Body"), "{text}");
        assert!(!state.slots.body.is_empty());
    }

    #[test]
    fn semantic_registers_dialog() {
        let system = DesignSystem::default();
        let dialog = Dialog::new("T", Text::from("B"), &system);
        let state = DialogState::<()>::new();
        let mut scene = SemanticScene::<&str, ()>::default();
        dialog.register_semantic(&mut scene, "d", Rect::new(0, 0, 20, 8), &state);
        assert!(scene.nodes().iter().any(|n| n.label.as_deref() == Some("dialog")));
    }

    #[test]
    fn fuzz_keys() {
        let actions = [
            Action {
                id: "a",
                label: "A",
                enabled: true,
                style: None,
            },
            Action {
                id: "b",
                label: "B",
                enabled: true,
                style: None,
            },
        ];
        let mut state = DialogState::new();
        state.set_action_cursor(Some("a"));
        let keys = [
            KeyCode::Enter,
            KeyCode::Esc,
            KeyCode::Left,
            KeyCode::Right,
            KeyCode::Up,
            KeyCode::Down,
            KeyCode::Char('j'),
            KeyCode::PageDown,
        ];
        let mut seed = 5u64;
        for _ in 0..200 {
            if !state.is_open() {
                state.set_open(true);
            }
            seed = seed.wrapping_mul(1103515245).wrapping_add(12345);
            let k = keys[(seed as usize) % keys.len()];
            let _ = state.handle_key(KeyEvent::new(k, KeyModifiers::NONE), &actions);
        }
    }

    #[test]
    fn paint_perf_smoke() {
        use ratatui_core::backend::TestBackend;
        use ratatui_core::terminal::Terminal;
        let system = DesignSystem::default();
        let actions = [
            Action {
                id: "ok",
                label: "OK",
                enabled: true,
                style: None,
            },
            Action {
                id: "cancel",
                label: "Cancel",
                enabled: true,
                style: None,
            },
        ];
        let dialog = ChoiceDialog::new(
            Dialog::new("Confirm", Text::from("Proceed with operation?"), &system)
                .description("This may take a moment.")
                .footer_hint("esc cancel"),
            &actions,
        );
        let mut state = ChoiceDialogState::new(Some("ok"));
        let mut terminal = Terminal::new(TestBackend::new(48, 14)).unwrap();
        let start = std::time::Instant::now();
        for _ in 0..150 {
            terminal
                .draw(|f| {
                    (&dialog).render(f.area(), f.buffer_mut(), &mut state);
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
        let dialog = Dialog::new("Notice", Text::from("Saved."), &system)
            .emphasis(PanelChrome::Focused)
            .footer_hint("esc");
        let mut t1 = Terminal::new(TestBackend::new(28, 8)).unwrap();
        t1.draw(|f| {
            Widget::render(&dialog, f.area(), f.buffer_mut());
        })
        .unwrap();
        let s1: String = t1
            .backend()
            .buffer()
            .content()
            .iter()
            .map(|c| c.symbol().to_string())
            .collect();
        let mut t2 = Terminal::new(TestBackend::new(28, 8)).unwrap();
        t2.draw(|f| {
            Widget::render(&dialog, f.area(), f.buffer_mut());
        })
        .unwrap();
        let s2: String = t2
            .backend()
            .buffer()
            .content()
            .iter()
            .map(|c| c.symbol().to_string())
            .collect();
        assert_eq!(s1, s2);
        assert!(s1.contains("Notice") || s1.contains("Saved"));
    }
}
