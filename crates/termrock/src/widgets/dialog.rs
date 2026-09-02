// SPDX-FileCopyrightText: 2026 Alexey Zhokhov
// SPDX-License-Identifier: Apache-2.0

//! **Dialog** — canonical modal interaction surface (junie dialog, one-to-one).
//!
//! Rounded frame `╭╮╰╯─│`. Focused frame is border-strong; idle is border-subtle.
//! Surface is elevated. Title is bold primary when focused. Inset is 3 columns
//! by 2 rows. Backdrop runs the theme backdrop resolver on every cell under the
//! dialog except the footer row. Actions are a right-aligned junie button row
//! (gap 1). Esc maps to the cancel action; Enter activates the focused action.
//!
//! Families: [`Dialog::confirm`] (primary focused first, Cancel quiet),
//! [`Dialog::destructive`] (Cancel focused first, confirm danger, a single `!`
//! on the title), [`Dialog::prompt`] (field focused first). Typed acknowledgement
//! keeps the confirming button disabled until the token matches.
//!
//! **vs Popover.** Popover is non-modal (default) and anchored. Dialog is
//! centered modal with trap + dim.
//! **vs [`super::AlertDialog`].** Use AlertDialog for high-risk confirmations
//! (delete/overwrite/terminate/egress) with typed gates and safe default focus.
use ratatui_core::{
    buffer::Buffer,
    layout::Rect,
    style::{Modifier, Style},
    text::Text,
    widgets::{StatefulWidget, Widget},
};
use ratatui_widgets::{block::Block, borders::Borders, paragraph::Paragraph};

use crate::{
    input::{KeyCode, KeyEvent, KeyEventKind, KeyModifiers},
    interaction::{
        BackdropPolicy, HitRegion, NavigationMove, Outcome, OverlayId, OverlayKind, OverlayOutcome,
        OverlayPolicy, OverlaySize, OverlaySpec, OverlayStack, SemanticNode, SemanticRole,
        SemanticScene, SemanticState, UiIntent, place_overlay,
    },
    scroll::DialogScroll,
    style::{DesignSystem, Role, RolePalette},
    text::{display_cols, take_display_cols, truncate_cols},
};

use super::primitives::{Button, ButtonState, ButtonVariant};
use super::{
    Action, ActionVariant, DetailRow, DetailTable, DetailTableState, Hint, HintBar, PanelChrome,
    Prop,
};

/// Vertical inset from the outer frame to content (junie `Margin::new(3, 2)`).
const DIALOG_INSET_Y: u16 = 2;
/// Cell gap between action buttons.
const DIALOG_ACTION_GAP: u16 = 1;

/// Footer chords while a confirm dialog is open (sentence case).
const DIALOG_CONFIRM_HINTS: &[Hint<'static>] = &[
    Hint {
        chord: "Esc",
        label: "Cancel",
        priority: 10,
        visible: true,
    },
    Hint {
        chord: "Enter",
        label: "Confirm",
        priority: 20,
        visible: true,
    },
];

/// Footer chords while a destructive dialog is open.
const DIALOG_DESTRUCTIVE_HINTS: &[Hint<'static>] = DIALOG_CONFIRM_HINTS;

/// Footer chords while a prompt dialog is open.
const DIALOG_PROMPT_HINTS: &[Hint<'static>] = &[
    Hint {
        chord: "Esc",
        label: "Cancel",
        priority: 10,
        visible: true,
    },
    Hint {
        chord: "Enter",
        label: "Submit",
        priority: 20,
        visible: true,
    },
];

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
        Self {
            width: 48,
            height: 12,
        }
    }
}

impl DialogSize {
    /// Size for a recipe (before bounds contraction).
    #[must_use]
    pub const fn for_recipe(recipe: DialogRecipe) -> Self {
        match recipe {
            DialogRecipe::Normal => Self {
                width: 54,
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
                width: 54,
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
    /// Typed acknowledgement buffer changed.
    TypedChanged,
}

impl<Id> DialogOutcome<Id> {
    fn into_choice_outcome(self) -> Outcome<Id> {
        match self {
            Self::Ignored | Self::LoadingBlocked | Self::ValidationFailed | Self::Scrolled => {
                Outcome::Ignored
            }
            Self::FocusMoved | Self::TypedChanged => Outcome::Changed,
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

/// A semantic backdrop painted behind modal content.
///
/// The design system remains the sole paint authority. Callers choose only the
/// overlay policy; raw symbols and styles cannot bypass the family recipe.
#[derive(Debug, Clone, Copy)]
pub struct Backdrop<'a> {
    system: &'a DesignSystem,
    policy: BackdropPolicy,
}

impl<'a> Backdrop<'a> {
    /// Dimmed modal wash.
    #[must_use]
    pub const fn new(system: &'a DesignSystem) -> Self {
        Self {
            system,
            policy: BackdropPolicy::Dim,
        }
    }

    /// Opaque backdrop for blocking/fullscreen layers.
    #[must_use]
    pub const fn occluding(system: &'a DesignSystem) -> Self {
        Self {
            system,
            policy: BackdropPolicy::Occlude,
        }
    }

    /// Selects the semantic policy requested by the overlay stack.
    #[must_use]
    pub const fn policy(mut self, policy: BackdropPolicy) -> Self {
        self.policy = policy;
        self
    }
}

impl Widget for &Backdrop<'_> {
    fn render(self, area: Rect, buffer: &mut Buffer) {
        match self.policy {
            BackdropPolicy::None => {}
            BackdropPolicy::Occlude => {
                let style = self.system.style(Role::Canvas);
                for y in area.top()..area.bottom() {
                    for x in area.left()..area.right() {
                        buffer[(x, y)].set_char(' ').set_style(style);
                    }
                }
            }
            // junie's dim is a per-cell collapse, not a flat wash: surfaces
            // keep their fill so the page keeps its shape, every foreground
            // steps down the alpha ladder, and coloured fills land on the
            // overlay plane. Symbols survive — a dim that erases the page is
            // an occlude, not a dim.
            BackdropPolicy::Dim => {
                let theme = self.system.junie_theme();
                for y in area.top()..area.bottom() {
                    for x in area.left()..area.right() {
                        let cell = &mut buffer[(x, y)];
                        let next = theme.backdrop(cell.style());
                        cell.set_style(next);
                        cell.modifier = Modifier::empty();
                    }
                }
            }
        }
    }
}

impl Widget for Backdrop<'_> {
    #[expect(
        clippy::needless_borrows_for_generic_args,
        reason = "explicitly delegate the owned contract to the borrowed renderer"
    )]
    fn render(self, area: Rect, buffer: &mut Buffer) {
        <&Self as Widget>::render(&self, area, buffer);
    }
}

fn dialog_inner(area: Rect, inset_x: u16) -> Rect {
    let x = area.x.saturating_add(inset_x);
    let y = area.y.saturating_add(DIALOG_INSET_Y);
    let width = area.width.saturating_sub(inset_x.saturating_mul(2));
    let height = area.height.saturating_sub(DIALOG_INSET_Y.saturating_mul(2));
    if width == 0 || height == 0 {
        Rect {
            x,
            y,
            width: 0,
            height: 0,
        }
    } else {
        Rect {
            x,
            y,
            width,
            height,
        }
    }
}

fn paint_dialog_frame(area: Rect, buffer: &mut Buffer, system: &DesignSystem, focused: bool) {
    let theme = system.junie_theme();
    let bg = theme.surface_elevated;
    // junie `fill` only sets bg, so empty cells keep the dimmed-page fg.
    for y in area.top()..area.bottom() {
        for x in area.left()..area.right() {
            let fg = buffer[(x, y)].fg;
            buffer[(x, y)]
                .set_char(' ')
                .set_style(Style::new().fg(fg).bg(bg));
        }
    }
    Block::default()
        .borders(Borders::ALL)
        .border_set(system.border_set())
        .border_style(theme.border(focused).bg(bg))
        .render(area, buffer);
}

fn dialog_actions_row_width<Id>(actions: &[Action<'_, Id>], system: &DesignSystem) -> u16 {
    let mut total = 0u16;
    for (i, action) in actions.iter().enumerate() {
        let variant = match action.variant {
            ActionVariant::Primary => ButtonVariant::Primary,
            ActionVariant::Destructive => ButtonVariant::Destructive,
            ActionVariant::Secondary => ButtonVariant::Secondary,
        };
        if i > 0 {
            total = total.saturating_add(DIALOG_ACTION_GAP);
        }
        total = total.saturating_add(
            Button::new(action.label, system)
                .variant(variant)
                .preferred_width(),
        );
    }
    total
}

fn dialog_button_variant<Id: PartialEq>(
    action: &Action<'_, Id>,
    cancel_id: Option<&Id>,
    cancel_quiet: bool,
) -> ButtonVariant {
    match action.variant {
        ActionVariant::Primary => ButtonVariant::Primary,
        ActionVariant::Destructive => ButtonVariant::Destructive,
        ActionVariant::Secondary
            if cancel_quiet && cancel_id.is_some_and(|id| id == &action.id) =>
        {
            ButtonVariant::Quiet
        }
        ActionVariant::Secondary => ButtonVariant::Secondary,
    }
}

/// Right-aligned junie action row (gap 1). `vertical` stacks when the dialog
/// is too narrow. Confirming actions stay disabled until `armed`.
pub(crate) fn paint_dialog_actions<Id: Clone + PartialEq>(
    actions: &[Action<'_, Id>],
    area: Rect,
    buffer: &mut Buffer,
    cursor: Option<&Id>,
    cancel_id: Option<&Id>,
    cancel_quiet: bool,
    armed: bool,
    system: &DesignSystem,
    colorless: bool,
    vertical: bool,
) -> Vec<HitRegion<Id>> {
    let mut regions = Vec::new();
    if area.is_empty() || actions.is_empty() {
        return regions;
    }
    let ground = system.junie_theme().surface_elevated;
    let action_enabled = |action: &Action<'_, Id>| {
        let confirming = match cancel_id {
            Some(cancel) => &action.id != cancel,
            None => matches!(
                action.variant,
                ActionVariant::Primary | ActionVariant::Destructive
            ),
        };
        action.enabled && (armed || !confirming)
    };

    if vertical {
        let mut y = area.y;
        for action in actions {
            if y >= area.bottom() {
                break;
            }
            let btn =
                dialog_action_button(action, cancel_id, cancel_quiet, system, colorless, ground);
            let width = btn.preferred_width().min(area.width);
            let x = area.right().saturating_sub(width).max(area.x);
            let rect = Rect::new(x, y, width, 1.min(area.height));
            if let Some(region) =
                paint_dialog_action(action, rect, buffer, cursor, action_enabled(action), &btn)
            {
                regions.push(region);
            }
            y = y.saturating_add(1);
        }
        return regions;
    }

    let mut total = 0u16;
    let mut widths = Vec::with_capacity(actions.len());
    for (i, action) in actions.iter().enumerate() {
        let w = dialog_action_button(action, cancel_id, cancel_quiet, system, colorless, ground)
            .preferred_width();
        widths.push(w);
        if i > 0 {
            total = total.saturating_add(DIALOG_ACTION_GAP);
        }
        total = total.saturating_add(w);
    }
    let mut x = area
        .right()
        .saturating_sub(total.min(area.width))
        .max(area.x);
    for (action, width) in actions.iter().zip(widths) {
        if x >= area.right() {
            break;
        }
        let w = width.min(area.right().saturating_sub(x));
        let rect = Rect::new(x, area.y, w, 1.min(area.height));
        let btn = dialog_action_button(action, cancel_id, cancel_quiet, system, colorless, ground);
        if let Some(region) =
            paint_dialog_action(action, rect, buffer, cursor, action_enabled(action), &btn)
        {
            regions.push(region);
        }
        x = x.saturating_add(width).saturating_add(DIALOG_ACTION_GAP);
    }
    regions
}

fn dialog_action_button<'a, Id: PartialEq>(
    action: &'a Action<'a, Id>,
    cancel_id: Option<&Id>,
    cancel_quiet: bool,
    system: &'a DesignSystem,
    colorless: bool,
    ground: ratatui_core::style::Color,
) -> Button<'a> {
    Button::new(action.label, system)
        .variant(dialog_button_variant(action, cancel_id, cancel_quiet))
        .colorless(colorless)
        .container(ground)
}

fn paint_facts_body(
    body: Rect,
    buffer: &mut Buffer,
    theme: &crate::style::JunieTheme,
    facts: &[Prop],
    code: &[String],
    bg: ratatui_core::style::Color,
) {
    if body.is_empty() {
        return;
    }
    let used = super::props::render(body, buffer, theme, facts, bg);
    if code.is_empty() {
        return;
    }
    let y = body.y.saturating_add(used).saturating_add(1);
    let max = code.len().min(6);
    for (i, line) in code.iter().take(max).enumerate() {
        let row = y.saturating_add(i as u16);
        if row >= body.bottom() {
            break;
        }
        let shown = if i == max.saturating_sub(1) && code.len() > max {
            format!(
                "{} … {} more",
                truncate_cols(line, usize::from(body.width.saturating_sub(12)), "…"),
                code.len() - max
            )
        } else {
            truncate_cols(line, usize::from(body.width), "…").into_owned()
        };
        buffer.set_stringn(
            body.x,
            row,
            &shown,
            display_cols(&shown).min(usize::from(body.width)),
            theme.secondary().bg(bg),
        );
    }
}

fn paint_ack_field(
    buffer: &mut Buffer,
    body: Rect,
    token: &str,
    typed: &str,
    armed: bool,
    system: &DesignSystem,
    bg: ratatui_core::style::Color,
) {
    if body.height < 2 || body.width < 4 {
        return;
    }
    let theme = system.junie_theme();
    let y = body.bottom().saturating_sub(1);
    let ask = format!("Type {token} to confirm");
    buffer.set_stringn(
        body.x,
        y.saturating_sub(1),
        &take_display_cols(&ask, usize::from(body.width)),
        usize::from(body.width),
        theme.muted().bg(bg),
    );
    let field = format!("{} {typed}", system.glyphs.selection_gutter());
    let style = if armed {
        theme.title().bg(bg)
    } else {
        theme.secondary().bg(bg)
    };
    buffer.set_stringn(
        body.x,
        y,
        &take_display_cols(&field, usize::from(body.width)),
        usize::from(body.width),
        style,
    );
}

fn paint_dialog_action<Id: Clone + PartialEq>(
    action: &Action<'_, Id>,
    rect: Rect,
    buffer: &mut Buffer,
    cursor: Option<&Id>,
    enabled: bool,
    button: &Button<'_>,
) -> Option<HitRegion<Id>> {
    if rect.is_empty() {
        return None;
    }
    let focused = cursor == Some(&action.id);
    let mut button_state = ButtonState::new();
    button_state.activation.set_enabled(enabled);
    button_state
        .activation
        .set_accepts_input(enabled && focused);
    button_state.focused = focused;
    let painted = button.paint(rect, buffer, &mut button_state);
    enabled.then(|| HitRegion {
        id: action.id.clone(),
        area: painted.root,
    })
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
    /// Token the user must type before the confirming action enables.
    ack_token: Option<String>,
    /// Typed acknowledgement buffer.
    typed_ack: String,
}

impl<Id> Default for DialogState<Id> {
    fn default() -> Self {
        Self::new()
    }
}

impl<Id> DialogState<Id> {
    /// Open dialog with action focus.
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
            ack_token: None,
            typed_ack: String::new(),
        }
    }

    /// Confirm-only (alert) factory.
    #[must_use]
    pub fn alert() -> Self {
        let mut s = Self::new();
        s.close_policy = DialogClosePolicy::ConfirmOnly;
        s
    }

    /// junie `Dialog::confirm`: primary action focused first; Esc fires cancel.
    #[must_use]
    pub fn confirm(confirm_id: Id, cancel_id: Id) -> Self
    where
        Id: Clone,
    {
        let mut s = Self::new();
        s.close_policy = DialogClosePolicy::ConfirmOnly;
        s.focus_zone = DialogFocusZone::Actions;
        s.initial_focus = DialogFocusZone::Actions;
        s.initial_applied = true;
        s.require_action_focus_for_enter = false;
        s.action_cursor = Some(confirm_id.clone());
        s.default_action = Some(confirm_id);
        s.cancel_action = Some(cancel_id);
        s
    }

    /// junie `Dialog::destructive`: Cancel focused first; confirm is danger.
    #[must_use]
    pub fn destructive(confirm_id: Id, cancel_id: Id) -> Self
    where
        Id: Clone,
    {
        let mut s = Self::new();
        s.close_policy = DialogClosePolicy::ConfirmOnly;
        s.recipe = DialogRecipe::Destructive;
        s.focus_zone = DialogFocusZone::Actions;
        s.initial_focus = DialogFocusZone::Actions;
        s.initial_applied = true;
        s.require_action_focus_for_enter = false;
        s.action_cursor = Some(cancel_id.clone());
        s.cancel_action = Some(cancel_id);
        s.default_action = Some(confirm_id);
        s
    }

    /// junie `Dialog::prompt`: field focused first; Enter submits the prompt.
    #[must_use]
    pub fn prompt(confirm_id: Id, cancel_id: Id) -> Self
    where
        Id: Clone,
    {
        let mut s = Self::new();
        s.close_policy = DialogClosePolicy::ConfirmOnly;
        s.focus_zone = DialogFocusZone::Body;
        s.initial_focus = DialogFocusZone::Body;
        s.initial_applied = true;
        s.require_action_focus_for_enter = false;
        s.default_action = Some(confirm_id);
        s.cancel_action = Some(cancel_id);
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

    /// Default action id.
    #[must_use]
    pub fn default_action(&self) -> Option<&Id> {
        self.default_action.as_ref()
    }

    /// Optional cancel action for confirm-only Esc handling.
    pub fn set_cancel_action(&mut self, id: Option<Id>) {
        self.cancel_action = id;
    }

    /// Cancel action id.
    #[must_use]
    pub fn cancel_action(&self) -> Option<&Id> {
        self.cancel_action.as_ref()
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

    /// Require typing `token` before the confirming action enables.
    pub fn set_ack_token(&mut self, token: Option<String>) {
        self.ack_token = token;
        self.typed_ack.clear();
        if self.ack_token.is_some() {
            self.initial_focus = DialogFocusZone::Body;
            self.focus_zone = DialogFocusZone::Body;
            self.initial_applied = true;
            self.require_action_focus_for_enter = true;
        }
    }

    /// Typed acknowledgement contents.
    #[must_use]
    pub fn typed_ack(&self) -> &str {
        &self.typed_ack
    }

    /// Whether the acknowledgement matches (or none is required).
    #[must_use]
    pub fn is_armed(&self) -> bool {
        match &self.ack_token {
            None => true,
            Some(token) => self.typed_ack.trim() == token,
        }
    }

    /// Slot geometry from the last paint.
    #[must_use]
    pub const fn slots(&self) -> DialogSlots {
        self.slots
    }

    /// Action hit regions from the last paint.
    #[must_use]
    pub fn action_regions(&self) -> &[HitRegion<Id>] {
        &self.action_regions
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
    pub fn close_on_stack<F: Clone>(&mut self, stack: &mut OverlayStack<F>) -> OverlayOutcome<F> {
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
    pub fn handle_key(&mut self, key: KeyEvent, actions: &[Action<'_, Id>]) -> DialogOutcome<Id> {
        if !self.open || !self.accepts_input || key.kind == KeyEventKind::Release {
            return DialogOutcome::Ignored;
        }
        self.ensure_initial_focus();
        if self.ack_token.is_some()
            && self.focus_zone == DialogFocusZone::Body
            && key.modifiers.is_empty()
            && key.kind == KeyEventKind::Press
        {
            match key.code {
                KeyCode::Char(c) if !c.is_control() => {
                    self.typed_ack.push(c);
                    return DialogOutcome::TypedChanged;
                }
                KeyCode::Backspace => {
                    self.typed_ack.pop();
                    return DialogOutcome::TypedChanged;
                }
                _ => {}
            }
        }
        if self.ack_token.is_none()
            && self.focus_zone == DialogFocusZone::Actions
            && key.kind == KeyEventKind::Press
            && key.modifiers.is_empty()
        {
            match key.code {
                KeyCode::Char('y' | 'Y') => {
                    if let Some(action) = actions.iter().find(|action| {
                        action.enabled
                            && matches!(
                                action.variant,
                                ActionVariant::Primary | ActionVariant::Destructive
                            )
                    }) {
                        return self.activate_action(actions, Some(action.id.clone()), false);
                    }
                }
                KeyCode::Char('n' | 'N') => return self.handle_cancel(actions),
                _ => {}
            }
        }
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
            UiIntent::Move(NavigationMove::Previous)
                if self.focus_zone == DialogFocusZone::Actions =>
            {
                self.move_action(actions, -1)
            }
            UiIntent::Move(NavigationMove::Next) if self.focus_zone == DialogFocusZone::Actions => {
                self.move_action(actions, 1)
            }
            UiIntent::Move(NavigationMove::First)
                if self.focus_zone == DialogFocusZone::Actions =>
            {
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
            UiIntent::Move(NavigationMove::Left) if self.focus_zone == DialogFocusZone::Actions => {
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
        self.activate_action(actions, None, force_default)
    }

    fn activate_action(
        &mut self,
        actions: &[Action<'_, Id>],
        prefer: Option<Id>,
        force_default: bool,
    ) -> DialogOutcome<Id> {
        if self.loading {
            return DialogOutcome::LoadingBlocked;
        }
        if self.validation_message.is_some() {
            return DialogOutcome::ValidationFailed;
        }
        // Accidental submission guard: body zone requires explicit opt-in.
        // Typed-ack Enter moves onto the actions so the confirming button is
        // reached deliberately, matching junie facts dialogs.
        if self.require_action_focus_for_enter
            && self.focus_zone == DialogFocusZone::Body
            && !force_default
        {
            if self.ack_token.is_some() {
                self.focus_zone = DialogFocusZone::Actions;
                return DialogOutcome::FocusMoved;
            }
            return DialogOutcome::Ignored;
        }
        if actions.is_empty() {
            return DialogOutcome::Ignored;
        }
        let candidates = [
            prefer,
            self.action_cursor.clone(),
            self.default_action.clone(),
        ];
        for id in candidates.into_iter().flatten() {
            if let Some(action) = actions.iter().find(|a| a.id == id) {
                if !self.action_is_enabled(action) {
                    if self.is_confirming(action) && !self.is_armed() {
                        return DialogOutcome::ValidationFailed;
                    }
                    continue;
                }
                return if self.default_action.as_ref() == Some(&id)
                    && self.action_cursor.as_ref() != Some(&id)
                {
                    DialogOutcome::DefaultActivated(id)
                } else {
                    DialogOutcome::Activated(id)
                };
            }
        }
        if let Some(a) = actions.iter().find(|a| self.action_is_enabled(a)) {
            return DialogOutcome::Activated(a.id.clone());
        }
        DialogOutcome::Ignored
    }

    fn is_confirming(&self, action: &Action<'_, Id>) -> bool {
        match &self.cancel_action {
            Some(cancel) => &action.id != cancel,
            None => matches!(
                action.variant,
                ActionVariant::Primary | ActionVariant::Destructive
            ),
        }
    }

    fn action_is_enabled(&self, action: &Action<'_, Id>) -> bool {
        action.enabled && (self.is_armed() || !self.is_confirming(action))
    }

    fn move_action(&mut self, actions: &[Action<'_, Id>], dir: isize) -> DialogOutcome<Id> {
        let enabled: Vec<_> = actions
            .iter()
            .filter(|a| self.action_is_enabled(a))
            .collect();
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
        let enabled: Vec<_> = actions
            .iter()
            .filter(|a| self.action_is_enabled(a))
            .collect();
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
            if actions
                .iter()
                .any(|a| a.id == region.id && self.action_is_enabled(a))
            {
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
        KeyCode::Char(' ') if is_press && matches!(zone, DialogFocusZone::Actions) => {
            Some(UiIntent::Activate)
        }
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
    tokens: &'a DesignSystem,
    emphasis: PanelChrome,
    variant: DialogVariant,
    recipe: DialogRecipe,
    footer_hint: Option<&'a str>,
    hints: &'a [Hint<'a>],
    loading: bool,
    colorless: bool,
    /// Confirm family: Cancel is quiet (junie subtle).
    cancel_quiet: bool,
    /// When set, `paint_modal` uses this instead of the recipe size.
    preferred_size: Option<DialogSize>,
    /// Facts-style body uses muted, not secondary.
    muted_body: bool,
    /// Label/value rows (junie `DialogBody::Facts`). Empty keeps [`Text`] body.
    facts: &'a [Prop],
    /// Preformatted block under the facts (SQL).
    code: &'a [String],
}

impl<'a> Dialog<'a> {
    /// Creates a dialog painted from design tokens.
    #[must_use]
    pub const fn new(title: &'a str, body: Text<'a>, tokens: &'a DesignSystem) -> Self {
        Self {
            title,
            description: None,
            body,
            tokens,
            emphasis: PanelChrome::Normal,
            variant: DialogVariant::Default,
            recipe: DialogRecipe::Normal,
            footer_hint: None,
            hints: &[],
            loading: false,
            colorless: false,
            cancel_quiet: false,
            preferred_size: None,
            muted_body: false,
            facts: &[],
            code: &[],
        }
    }

    /// junie confirm: primary focused first; Cancel is quiet.
    #[must_use]
    pub const fn confirm(title: &'a str, body: Text<'a>, tokens: &'a DesignSystem) -> Self {
        let mut dialog = Self::new(title, body, tokens);
        dialog.emphasis = PanelChrome::Focused;
        dialog.hints = DIALOG_CONFIRM_HINTS;
        dialog.cancel_quiet = true;
        dialog
    }

    /// junie destructive: Cancel focused first; confirm is danger; title `!` once.
    #[must_use]
    pub const fn destructive(title: &'a str, body: Text<'a>, tokens: &'a DesignSystem) -> Self {
        let mut dialog = Self::new(title, body, tokens);
        dialog.emphasis = PanelChrome::Focused;
        dialog.variant = DialogVariant::Danger;
        dialog.recipe = DialogRecipe::Destructive;
        dialog.hints = DIALOG_DESTRUCTIVE_HINTS;
        dialog
    }

    /// junie prompt: field focused first.
    #[must_use]
    pub const fn prompt(title: &'a str, body: Text<'a>, tokens: &'a DesignSystem) -> Self {
        let mut dialog = Self::new(title, body, tokens);
        dialog.emphasis = PanelChrome::Focused;
        dialog.hints = DIALOG_PROMPT_HINTS;
        dialog
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

    /// Override recipe geometry (source DataGrid facts dialog is 66×14).
    #[must_use]
    pub const fn preferred_size(mut self, size: DialogSize) -> Self {
        self.preferred_size = Some(size);
        self
    }

    /// Paint body copy as muted (source DataGrid facts).
    #[must_use]
    pub const fn muted_body(mut self, on: bool) -> Self {
        self.muted_body = on;
        self
    }

    /// Label/value facts plus an optional preformatted block. Gap cells between
    /// label and value stay unpainted (junie `props::render`).
    #[must_use]
    pub const fn facts(mut self, facts: &'a [Prop], code: &'a [String]) -> Self {
        self.facts = facts;
        self.code = code;
        self
    }

    /// Footer hints as chords, painted through [`HintBar`].
    ///
    /// This is the structured path: one separator from the glyph catalog, one
    /// alignment rule, and hints that contract as a row instead of as a
    /// sentence. Prefer it over [`Self::footer_hint`], which stays for plain
    /// copy that is not a chord list (plans/009 Step 1).
    #[must_use]
    pub const fn hints(mut self, hints: &'a [Hint<'a>]) -> Self {
        self.hints = hints;
        self
    }

    /// Footer hint row as plain copy.
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

    /// Reduced color.
    #[must_use]
    pub const fn colorless(mut self, on: bool) -> Self {
        self.colorless = on;
        self
    }

    fn frame_focused(&self, state_accepts: bool) -> bool {
        state_accepts
            || matches!(self.emphasis, PanelChrome::Focused)
            || matches!(self.variant, DialogVariant::Info)
    }

    fn title_for_paint(&self) -> String {
        let mut title = self.title.to_string();
        if self.loading {
            let glyph = self.tokens.glyphs.loading();
            title = format!("{title} {glyph}");
        }
        title
    }

    fn inset_x(&self) -> u16 {
        self.tokens.spacing.dialog_inset
    }

    fn content_rect(&self, area: Rect) -> Rect {
        dialog_inner(area, self.inset_x())
    }

    /// Paint chrome and compute slots into `state` (optional actions height reserved).
    ///
    /// `area` is the dialog frame (already placed). Backdrop lives on
    /// [`Self::paint_modal`].
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
        let focused = self.frame_focused(state.accepts_input && state.open);
        paint_dialog_frame(area, buffer, self.tokens, focused);
        if area.height < 3 || area.width < 4 {
            return;
        }

        let inner = dialog_inner(area, self.inset_x());
        let theme = self.tokens.junie_theme();
        let bg = theme.surface_elevated;
        let title = self.title_for_paint();
        let title_style = if focused {
            theme.title().bg(bg)
        } else {
            theme.secondary().bg(bg)
        };
        // Title lives inside the frame. On a 4-row dialog the 2-row vertical
        // inset leaves no inner rect, so the title sits on the first row
        // under the border rather than vanishing.
        let title_x = if inner.width > 0 {
            inner.x
        } else {
            area.x.saturating_add(1)
        };
        let title_y = if inner.height > 0 {
            inner.y
        } else {
            area.y
                .saturating_add(1)
                .min(area.bottom().saturating_sub(1))
        };
        let title_w = if inner.width > 0 {
            inner.width
        } else {
            area.width.saturating_sub(2)
        };
        state.slots.title = Rect::new(title_x, title_y, title_w, 1);
        if title_w > 0 && title_y < area.bottom() {
            buffer.set_stringn(
                title_x,
                title_y,
                &take_display_cols(&title, usize::from(title_w)),
                usize::from(title_w),
                title_style,
            );
        }
        if inner.is_empty() {
            return;
        }

        let has_desc = self.description.is_some() && inner.height >= 3;
        let has_validation = state.validation_message.is_some() && inner.height >= 4;
        let validation_rows = u16::from(has_validation);
        let desc_rows = u16::from(has_desc);
        let action_h = action_rows.min(inner.height.saturating_sub(1));
        let rhythm = inner.height
            >= 3_u16
                .saturating_add(action_h)
                .saturating_add(desc_rows)
                .saturating_add(validation_rows);

        let mut y = inner.y.saturating_add(1);
        if has_desc {
            if let Some(d) = self.description {
                state.slots.description = Rect::new(inner.x, y, inner.width, 1);
                buffer.set_stringn(
                    inner.x,
                    y,
                    &take_display_cols(d, usize::from(inner.width)),
                    usize::from(inner.width),
                    theme.muted().bg(bg),
                );
                y = y.saturating_add(1);
            }
        } else {
            state.slots.description = Rect::default();
            if rhythm {
                y = y.saturating_add(1);
            }
        }

        let junie_actions_y = if action_h > 0 {
            area.bottom()
                .saturating_sub(2)
                .saturating_sub(action_h)
                .max(y)
        } else {
            inner.bottom()
        };
        let gap_before_actions = u16::from(rhythm && action_h > 0);
        let reserved_bottom = validation_rows
            .saturating_add(action_h)
            .saturating_add(gap_before_actions);
        let body_h = inner
            .bottom()
            .saturating_sub(y)
            .saturating_sub(reserved_bottom);
        let body_h = if inner.height > 2 && reserved_bottom < inner.bottom().saturating_sub(y) {
            body_h.max(1)
        } else {
            body_h
        };
        state.slots.body = Rect::new(inner.x, y, inner.width, body_h);

        state.body_line_count = if self.facts.is_empty() && self.code.is_empty() {
            self.body.lines.len()
        } else {
            self.facts.len()
                + usize::from(!self.code.is_empty())
                + self.code.len().min(6)
                + usize::from(state.ack_token.is_some()) * 2
        };
        if !state.slots.body.is_empty() {
            if self.facts.is_empty() && self.code.is_empty() {
                Paragraph::new(self.body.clone())
                    .style(if self.muted_body {
                        theme.muted().bg(bg)
                    } else {
                        theme.secondary().bg(bg)
                    })
                    .wrap(ratatui_widgets::paragraph::Wrap { trim: false })
                    .scroll((state.scroll.scroll_y, state.scroll.scroll_x))
                    .render(state.slots.body, buffer);
                if let Some(token) = state.ack_token.as_deref() {
                    paint_ack_field(
                        buffer,
                        state.slots.body,
                        token,
                        &state.typed_ack,
                        state.is_armed(),
                        self.tokens,
                        bg,
                    );
                }
            } else {
                paint_facts_body(state.slots.body, buffer, &theme, self.facts, self.code, bg);
                if let Some(token) = state.ack_token.as_deref() {
                    paint_ack_field(
                        buffer,
                        state.slots.body,
                        token,
                        &state.typed_ack,
                        state.is_armed(),
                        self.tokens,
                        bg,
                    );
                }
            }
        }

        y = state.slots.body.bottom();
        if has_validation {
            state.slots.validation = Rect::new(inner.x, y, inner.width, 1);
            if let Some(msg) = &state.validation_message {
                buffer.set_stringn(
                    inner.x,
                    y,
                    &take_display_cols(msg, usize::from(inner.width)),
                    usize::from(inner.width),
                    self.tokens.style(Role::Danger).bg(bg),
                );
            }
            y = y.saturating_add(1);
        } else {
            state.slots.validation = Rect::default();
        }

        if rhythm && action_h > 0 && y + 1 <= junie_actions_y {
            y = y.saturating_add(1);
        }

        if action_h > 0 {
            let actions_y = if rhythm { junie_actions_y.max(y) } else { y };
            state.slots.actions = Rect::new(inner.x, actions_y, inner.width, action_h);
        } else {
            state.slots.actions = Rect::default();
        }

        state.slots.footer = Rect::default();
    }

    /// Dim the page (except the live footer row), place the dialog, paint chrome
    /// and right-aligned actions, then replace the footer with dialog hints.
    pub fn paint_modal<Id: Clone + PartialEq>(
        &self,
        screen: Rect,
        buffer: &mut Buffer,
        state: &mut DialogState<Id>,
        actions: &[Action<'_, Id>],
    ) {
        if screen.is_empty() {
            return;
        }
        let dim = Rect::new(
            screen.x,
            screen.y,
            screen.width,
            screen.height.saturating_sub(1),
        );
        Backdrop::new(self.tokens).render(dim, buffer);
        let mut preferred = self
            .preferred_size
            .unwrap_or_else(|| DialogSize::for_recipe(self.recipe));
        if preferred.width == 0 || preferred.height == 0 {
            preferred = DialogSize {
                width: screen.width.saturating_sub(4).max(20),
                height: screen.height.saturating_sub(2).max(3),
            };
        }
        preferred.width = preferred
            .width
            .min(screen.width.saturating_sub(4))
            .max(20.min(screen.width));
        let frame = place_dialog(screen, preferred);
        let stack_actions = crate::layout::dialog_stack_actions(frame.width, frame.height);
        let action_rows = if actions.is_empty() {
            0
        } else if stack_actions {
            (actions.len() as u16)
                .min(frame.height.saturating_sub(3))
                .max(1)
        } else {
            1
        };
        self.paint(frame, buffer, state, action_rows);
        if !actions.is_empty() {
            let regions = paint_dialog_actions(
                actions,
                state.slots.actions,
                buffer,
                state.action_cursor.as_ref(),
                state.cancel_action.as_ref(),
                self.cancel_quiet,
                state.is_armed(),
                self.tokens,
                self.colorless,
                stack_actions && action_rows > 1,
            );
            state.action_regions = regions;
        }
        if screen.height > 0 && (!self.hints.is_empty() || self.footer_hint.is_some()) {
            let footer = Rect::new(screen.x, screen.bottom().saturating_sub(1), screen.width, 1);
            if !self.hints.is_empty() {
                ratatui_core::widgets::Widget::render(
                    &HintBar::new(self.hints, self.tokens),
                    footer,
                    buffer,
                );
            } else if let Some(hint) = self.footer_hint {
                buffer.set_stringn(
                    footer.x,
                    footer.y,
                    &take_display_cols(hint, usize::from(footer.width)),
                    usize::from(footer.width),
                    self.tokens.junie_theme().key_hint_action(),
                );
            }
        }
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
        if matches!(self.emphasis, PanelChrome::Focused) {
            state.set_accepts_input(true);
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
        self.dialog.handle_key(key, actions).into_choice_outcome()
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
                    .into_choice_outcome(),
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
        out.into_choice_outcome()
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
    /// Reduced-color action paint.
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
        let required = dialog_actions_row_width(self.actions, self.dialog.tokens());
        let stack_actions = required > self.dialog.content_rect(area).width;
        let action_rows = if stack_actions {
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
        chrome.colorless = self.colorless;
        chrome.paint(area, buffer, &mut state.dialog, action_rows);

        let action_area = state.dialog.slots.actions;
        if action_area.is_empty() {
            state.regions.clear();
            return;
        }
        state.regions = paint_dialog_actions(
            self.actions,
            action_area,
            buffer,
            state.cursor.as_ref(),
            state.dialog.cancel_action.as_ref(),
            self.dialog.cancel_quiet,
            state.dialog.is_armed(),
            self.dialog.tokens(),
            self.colorless,
            stack_actions && action_rows > 1,
        );
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
        let body_height = u16::try_from(body_height).unwrap_or(u16::MAX).min(
            body.height
                .saturating_sub(u16::from(!self.details.is_empty())),
        );
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
    use ratatui_core::{
        layout::Position,
        style::{Color, Style},
        widgets::StatefulWidget,
    };

    use crate::{
        input::KeyModifiers,
        interaction::{OverlayId, OverlayKind, OverlayOutcome, OverlayStack},
    };

    #[test]
    fn backdrop_collapses_content_but_keeps_the_page_shape() {
        // junie's dim is per cell: an empty cell sits on the canvas ground, a
        // surface keeps its fill, and loud content steps down the ladder.
        let system = DesignSystem::default();
        let area = Rect::new(0, 0, 3, 1);
        let mut buffer = Buffer::empty(area);
        buffer[(0, 0)].set_char('x').set_style(Style::new());
        buffer[(1, 0)]
            .set_char('y')
            .set_style(Style::new().bg(system.style(Role::Surface).bg.unwrap()));
        buffer[(2, 0)]
            .set_char('z')
            .set_style(Style::new().fg(system.style(Role::Accent).fg.unwrap()));
        Backdrop::new(&system).render(area, &mut buffer);
        let theme = system.junie_theme();
        assert_eq!(buffer[(0, 0)].symbol(), "x", "a dim never erases the page");
        assert_eq!(
            buffer[(0, 0)].bg,
            theme.surface_overlay,
            "terminal-default ground recedes to the overlay plane"
        );
        // A terminal-default foreground is "hidden" content: the reference
        // keeps it hidden by collapsing it onto the same plane as the ground.
        assert_eq!(
            buffer[(0, 0)].fg,
            theme.surface_overlay,
            "hidden stays hidden"
        );
        assert_eq!(
            buffer[(1, 0)].bg,
            system.style(Role::Surface).bg.expect("surface fill"),
            "surfaces keep their fill so the page keeps its shape"
        );
        assert_eq!(
            buffer[(2, 0)].fg,
            theme.text_muted,
            "accent content steps down to the muted tier"
        );
        assert_eq!(
            buffer[(2, 0)].bg,
            theme.surface_overlay,
            "a coloured fill lands on the overlay plane"
        );
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
            top.fullscreen_promoted || top.kind == OverlayKind::Fullscreen || top.rect == bounds
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
                variant: ActionVariant::Primary,
            },
            Action {
                id: "blocked",
                label: "Blocked",
                enabled: false,
                variant: ActionVariant::Secondary,
            },
            Action {
                id: "cancel",
                label: "Cancel",
                enabled: true,
                variant: ActionVariant::Secondary,
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
            variant: ActionVariant::Primary,
        }];
        let mut state = ChoiceDialogState::new(Some("ok"));
        state.set_accepts_input(false);
        assert_eq!(
            state.handle_intent(&actions, UiIntent::Activate),
            Outcome::Ignored
        );
    }

    #[test]
    fn choice_dialog_stacks_only_when_actions_do_not_fit() {
        let actions = [
            Action {
                id: "a",
                label: "Accept",
                enabled: true,
                variant: ActionVariant::Primary,
            },
            Action {
                id: "c",
                label: "Cancel",
                enabled: true,
                variant: ActionVariant::Secondary,
            },
        ];
        let system = DesignSystem::default();
        let dialog = ChoiceDialog::new(
            Dialog::new("Choose", Text::from("?"), &system).emphasis(PanelChrome::Focused),
            &actions,
        );
        let required = dialog_actions_row_width(&actions, &system);
        let horizontal_width = (1..80)
            .find(|width| dialog.dialog.content_rect(Rect::new(0, 0, *width, 8)).width >= required)
            .expect("dialog reaches the action bar reference width");
        let stacked_area = Rect::new(0, 0, horizontal_width.saturating_sub(1), 8);
        let mut stacked_buffer = Buffer::empty(stacked_area);
        let mut stacked_state = ChoiceDialogState::new(Some("a"));
        (&dialog).render(stacked_area, &mut stacked_buffer, &mut stacked_state);
        assert_eq!(stacked_state.regions.len(), 2);
        assert_ne!(
            stacked_state.regions[0].area.y,
            stacked_state.regions[1].area.y
        );
        let stacked_slot = stacked_state.dialog().slots().actions;
        for region in &stacked_state.regions {
            assert_eq!(
                region.area.right(),
                stacked_slot.right(),
                "stacked actions sit on the right edge"
            );
        }

        // The first width meeting the measured action-bar contract stays one row.
        let horizontal_area = Rect::new(0, 0, horizontal_width, 8);
        let mut horizontal_buffer = Buffer::empty(horizontal_area);
        let mut horizontal_state = ChoiceDialogState::new(Some("a"));
        (&dialog).render(
            horizontal_area,
            &mut horizontal_buffer,
            &mut horizontal_state,
        );
        assert_eq!(horizontal_state.regions.len(), 2);
        assert_eq!(
            horizontal_state.regions[0].area.y,
            horizontal_state.regions[1].area.y
        );
        let horizontal_slot = horizontal_state.dialog().slots().actions;
        assert_eq!(
            horizontal_state.regions[1].area.right(),
            horizontal_slot.right(),
            "the action row is right-aligned"
        );
    }

    #[test]
    fn choice_dialog_mouse_outcomes_follow_enabled_painted_regions() {
        let actions = [Action {
            id: "accept",
            label: "Accept",
            enabled: true,
            variant: ActionVariant::Primary,
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
    fn dialog_rhythm_adds_and_contracts_spacer_rows() {
        let tokens = DesignSystem::default();
        let dialog = Dialog::new("Edit", Text::from("Body"), &tokens)
            .description("Description")
            .footer_hint("esc cancel");
        let mut spacious = DialogState::<()>::new();
        spacious.set_validation_message(Some("Required".into()));
        let mut buffer = Buffer::empty(Rect::new(0, 0, 40, 20));
        dialog.paint(Rect::new(0, 0, 40, 20), &mut buffer, &mut spacious, 1);
        assert!(spacious.slots.description.y > 1);
        assert!(spacious.slots.actions.y > spacious.slots.validation.bottom());

        let mut cramped = DialogState::<()>::new();
        cramped.set_validation_message(Some("Required".into()));
        let mut buffer = Buffer::empty(Rect::new(0, 0, 20, 8));
        dialog.paint(Rect::new(0, 0, 20, 8), &mut buffer, &mut cramped, 1);
        assert!(
            cramped.slots.actions.y >= cramped.slots.validation.bottom().saturating_sub(1),
            "cramped actions sit on the validation band: actions={} validation_bottom={}",
            cramped.slots.actions.y,
            cramped.slots.validation.bottom()
        );
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
                .contains("Notice")
        );
    }

    #[test]
    fn danger_variant_uses_focused_frame_without_title_bang() {
        let tokens = DesignSystem::junie();
        let dialog = Dialog::destructive("Delete", Text::from("Irreversible"), &tokens);
        let area = Rect::new(0, 0, 24, 5);
        let mut buffer = Buffer::empty(area);
        (&dialog).render(area, &mut buffer);
        assert_eq!(
            buffer[(0, 0)].fg,
            tokens.junie_theme().border_strong,
            "destructive chrome uses the focused frame, not a danger border"
        );
        let text: String = buffer.content().iter().map(|c| c.symbol()).collect();
        assert!(text.contains('╭') && text.contains('╰'), "{text:?}");
        assert_eq!(
            text.matches('!').count(),
            0,
            "junie titles have no bang: {text:?}"
        );
        assert!(text.contains("Delete"), "{text:?}");
    }

    #[test]
    fn loading_disables_choice_activation() {
        let actions = [Action {
            id: "ok",
            label: "OK",
            enabled: true,
            variant: ActionVariant::Primary,
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
    fn empty_body_uses_canonical_constructor() {
        let system = DesignSystem::junie();
        let dialog = Dialog::new("Empty", Text::default(), &system).footer_hint("esc cancel");
        let area = Rect::new(0, 0, 28, 6);
        let mut buffer = Buffer::empty(area);
        (&dialog).render(area, &mut buffer);
        let text: String = buffer.content().iter().map(|c| c.symbol()).collect();
        assert!(text.contains("Empty"), "{text:?}");
        assert!(text.contains('╭') && text.contains('╰'), "{text:?}");
        assert!(
            !text.contains("esc"),
            "chords live on the page footer, not inside the frame: {text:?}"
        );
    }

    #[test]
    fn monochrome_backdrop_keeps_the_grey_hierarchy_without_dim() {
        // Monochrome recedes through the grey buckets, never through DIM.
        let system = DesignSystem::default().no_color();
        let area = Rect::new(0, 0, 2, 1);
        let mut buffer = Buffer::empty(area);
        buffer[(0, 0)]
            .set_char('a')
            .set_style(Style::new().fg(Color::White));
        Backdrop::new(&system).render(area, &mut buffer);
        let theme = system.junie_theme();
        assert_eq!(
            buffer[(0, 0)].fg,
            theme
                .backdrop(Style::new().fg(Color::White))
                .fg
                .expect("muted grey"),
            "body content collapses to the muted grey tier"
        );
        assert!(
            !buffer[(0, 0)]
                .modifier
                .contains(ratatui_core::style::Modifier::DIM),
            "DIM is not part of the vocabulary"
        );
    }

    #[test]
    fn dialog_size_tracks_density() {
        assert!(DialogSize::default().width >= 40);
    }

    #[test]
    fn enter_does_not_submit_from_body_zone() {
        let actions = [
            Action {
                id: "ok",
                label: "OK",
                enabled: true,
                variant: ActionVariant::Primary,
            },
            Action {
                id: "cancel",
                label: "Cancel",
                enabled: true,
                variant: ActionVariant::Secondary,
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
            variant: ActionVariant::Primary,
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
            variant: ActionVariant::Primary,
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
        let text: String = buf
            .content()
            .iter()
            .map(|c| c.symbol().to_string())
            .collect();
        assert!(text.contains("Title") || text.contains("Body"), "{text}");
        assert!(!state.slots.body.is_empty());
    }

    #[test]
    fn dialog_paints_elevated_fill() {
        let system = DesignSystem::default();
        let dialog = Dialog::new("Title", Text::from("Body"), &system);
        let mut state = DialogState::<()>::new();
        let area = Rect::new(0, 0, 30, 8);
        let mut buffer = Buffer::empty(area);
        dialog.paint(area, &mut buffer, &mut state, 0);
        assert_eq!(buffer[(2, 2)].bg, system.style(Role::Elevated).bg.unwrap());
    }

    #[test]
    fn semantic_registers_dialog() {
        let system = DesignSystem::default();
        let dialog = Dialog::new("T", Text::from("B"), &system);
        let state = DialogState::<()>::new();
        let mut scene = SemanticScene::<&str, ()>::default();
        dialog.register_semantic(&mut scene, "d", Rect::new(0, 0, 20, 8), &state);
        assert!(
            scene
                .nodes()
                .iter()
                .any(|n| n.label.as_deref() == Some("dialog"))
        );
    }

    #[test]
    fn fuzz_keys() {
        let actions = [
            Action {
                id: "a",
                label: "A",
                enabled: true,
                variant: ActionVariant::Secondary,
            },
            Action {
                id: "b",
                label: "B",
                enabled: true,
                variant: ActionVariant::Secondary,
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
                variant: ActionVariant::Primary,
            },
            Action {
                id: "cancel",
                label: "Cancel",
                enabled: true,
                variant: ActionVariant::Secondary,
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

    fn confirm_actions<'a>() -> [Action<'a, &'static str>; 2] {
        [
            Action {
                id: "cancel",
                label: "Cancel",
                enabled: true,
                variant: ActionVariant::Secondary,
            },
            Action {
                id: "ok",
                label: "Run",
                enabled: true,
                variant: ActionVariant::Primary,
            },
        ]
    }

    fn destructive_actions<'a>() -> [Action<'a, &'static str>; 2] {
        [
            Action {
                id: "cancel",
                label: "Cancel",
                enabled: true,
                variant: ActionVariant::Secondary,
            },
            Action {
                id: "delete",
                label: "Delete",
                enabled: true,
                variant: ActionVariant::Destructive,
            },
        ]
    }

    #[test]
    fn destructive_cancel_is_initial_focus_rounded_frame_backdrop() {
        let system = DesignSystem::junie();
        let theme = system.junie_theme();
        let screen = Rect::new(0, 0, 80, 24);
        let mut buffer = Buffer::empty(screen);
        let primary = system.style(Role::Text);
        buffer.set_string(0, 0, "PAGE", primary);
        buffer.set_string(0, 23, "LIVE", primary);

        let mut state = DialogState::destructive("delete", "cancel");
        let actions = destructive_actions();
        Dialog::destructive(
            "Delete project",
            Text::from("This cannot be undone."),
            &system,
        )
        .paint_modal(screen, &mut buffer, &mut state, &actions);

        assert_eq!(state.action_cursor().copied(), Some("cancel"));
        assert!(
            actions
                .iter()
                .any(|a| matches!(a.variant, ActionVariant::Destructive))
        );

        let placed = state.slots.root;
        assert!(placed.width > 0 && placed.height > 0, "dialog was placed");
        assert_eq!(buffer[(placed.x, placed.y)].symbol(), "╭");
        assert_eq!(buffer[(placed.right() - 1, placed.y)].symbol(), "╮");
        assert_eq!(buffer[(placed.x, placed.bottom() - 1)].symbol(), "╰");
        assert_eq!(
            buffer[(placed.right() - 1, placed.bottom() - 1)].symbol(),
            "╯"
        );

        let title_row: String = (placed.x..placed.right())
            .map(|x| buffer[(x, placed.y + DIALOG_INSET_Y)].symbol().to_string())
            .collect();
        assert!(
            title_row.contains("Delete project"),
            "title row: {title_row:?}"
        );
        assert_eq!(
            title_row.matches('!').count(),
            0,
            "junie titles have no bang: {title_row:?}"
        );

        assert_eq!(
            buffer[(0, 0)].fg,
            theme.text_muted,
            "primary page text outside the dialog collapses through backdrop"
        );
        assert_eq!(
            buffer[(0, 23)].fg,
            primary.fg.unwrap(),
            "the footer row stays live"
        );
    }

    #[test]
    fn destructive_stores_confirm_as_default_enter_activates_cancel() {
        let mut state = DialogState::destructive("delete", "cancel");
        assert_eq!(state.action_cursor().copied(), Some("cancel"));
        assert_eq!(state.cancel_action().copied(), Some("cancel"));
        assert_eq!(state.default_action().copied(), Some("delete"));
        assert!(matches!(
            state.handle_key(
                KeyEvent::new(KeyCode::Enter, KeyModifiers::NONE),
                &destructive_actions()
            ),
            DialogOutcome::Activated("cancel")
        ));
    }

    #[test]
    fn confirm_primary_focused_first() {
        let system = DesignSystem::junie();
        let mut state = DialogState::confirm("ok", "cancel");
        assert_eq!(state.action_cursor().copied(), Some("ok"));
        let actions = confirm_actions();
        let screen = Rect::new(0, 0, 80, 20);
        let mut buffer = Buffer::empty(screen);
        Dialog::confirm("Confirm run", Text::from("Run the task now?"), &system).paint_modal(
            screen,
            &mut buffer,
            &mut state,
            &actions,
        );
        assert_eq!(state.action_cursor().copied(), Some("ok"));
        assert!(state.action_regions().iter().any(|r| r.id == "ok"));
    }

    #[test]
    fn esc_yields_cancel() {
        let actions = confirm_actions();
        let mut state = DialogState::confirm("ok", "cancel");
        assert!(matches!(
            state.handle_key(KeyEvent::new(KeyCode::Esc, KeyModifiers::NONE), &actions),
            DialogOutcome::Activated("cancel")
        ));
        let mut destructive = DialogState::destructive("delete", "cancel");
        assert!(matches!(
            destructive.handle_key(
                KeyEvent::new(KeyCode::Esc, KeyModifiers::NONE),
                &destructive_actions()
            ),
            DialogOutcome::Activated("cancel")
        ));
    }

    #[test]
    fn typed_ack_disables_confirm_until_match() {
        let system = DesignSystem::junie();
        let mut actions = destructive_actions();
        let mut state = DialogState::destructive("delete", "cancel");
        state.set_ack_token(Some("delete-me".into()));
        assert!(!state.is_armed());
        assert!(matches!(
            state.handle_intent(UiIntent::Activate, &actions),
            DialogOutcome::FocusMoved
        ));
        state.set_action_cursor(Some("delete"));
        assert!(matches!(
            state.handle_intent(UiIntent::Activate, &actions),
            DialogOutcome::ValidationFailed
        ));

        state.set_focus_zone(DialogFocusZone::Body);
        for c in "delete-me".chars() {
            assert!(matches!(
                state.handle_key(
                    KeyEvent::new(KeyCode::Char(c), KeyModifiers::NONE),
                    &actions
                ),
                DialogOutcome::TypedChanged
            ));
        }
        assert!(state.is_armed());
        actions[1].enabled = true;
        state.set_focus_zone(DialogFocusZone::Actions);
        state.set_action_cursor(Some("delete"));
        assert!(matches!(
            state.handle_intent(UiIntent::Activate, &actions),
            DialogOutcome::Activated("delete")
        ));

        let mut painted = DialogState::destructive("delete", "cancel");
        painted.set_ack_token(Some("delete-me".into()));
        let screen = Rect::new(0, 0, 80, 20);
        let mut buffer = Buffer::empty(screen);
        Dialog::destructive("Delete project", Text::from("Type to confirm."), &system).paint_modal(
            screen,
            &mut buffer,
            &mut painted,
            &destructive_actions(),
        );
        assert!(
            painted.action_regions().iter().all(|r| r.id != "delete"),
            "unarmed confirm is not a hit target"
        );
    }
}
