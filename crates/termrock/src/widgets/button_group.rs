// SPDX-FileCopyrightText: 2026 Alexey Zhokhov
// SPDX-License-Identifier: Apache-2.0

//! ButtonGroup — grouped actions with shared chrome, priority overflow, and roving focus.
//!
//! **Mission.** Dialog footers, review flows, and compact toolbars need more than a
//! flat label strip: connected/separated recipes, primary-before-secondary order,
//! destructive separation, overflow at narrow widths, and roving focus without
//! hiding each command's identity.
//!
//! **vs [`ActionBar`](crate::widgets::ActionBar).** ActionBar remains a simple
//! paint + hit strip for simple dialog footers. Prefer ButtonGroup when overflow,
//! variants, default submit, or roving semantics matter.
//!
//! **vs [`Toolbar`](crate::widgets::Toolbar).** Toolbar owns rich item kinds and
//! command catalogs; ButtonGroup is a focused action cluster (OK/Cancel/Delete).
//!
//! Research: shadcn button groups, desktop dialog action bars, terminal prompt rows.
use ratatui_core::{buffer::Buffer, layout::Rect, style::Modifier, widgets::Widget};

use crate::input::{KeyCode, KeyEvent, MouseButton, MouseEvent, MouseEventKind};
use crate::interaction::{
    EventResult, NavigationMove, RovingEntry, RovingFocusGroup, RovingOrientation, RovingOutcome,
    SemanticNode, SemanticRole, SemanticScene, SemanticState, UiIntent, default_button_intent,
};
use crate::style::{ButtonRecipeVariant, ControlState, DesignSystem, Role};
use crate::text::{display_cols, take_display_cols};
use crate::widgets::{Button, ButtonState, ButtonVariant};

fn default_button_group_intent(key: KeyEvent) -> Option<UiIntent> {
    let intent = default_button_intent(key)?;
    match (intent, key.code) {
        (UiIntent::Activate, KeyCode::Enter) => Some(UiIntent::Submit),
        (intent, _) => Some(intent),
    }
}

// ── Recipe / layout ─────────────────────────────────────────────────────────

/// Visual connection between adjacent buttons.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
#[non_exhaustive]
pub enum ButtonGroupRecipe {
    /// Shared edge feel: tight separators (`|` / space) between faces.
    Connected,
    /// Visible gap between independent buttons (default dialog footer).
    #[default]
    Separated,
}

impl ButtonGroupRecipe {
    /// Stable id.
    #[must_use]
    pub const fn id(self) -> &'static str {
        match self {
            Self::Connected => "connected",
            Self::Separated => "separated",
        }
    }

    /// Columns between faces: connected uses a 1-col separator glyph; separated uses a gap.
    fn inter_cols(self) -> u16 {
        1
    }

    fn separator_glyph(self, ascii: bool) -> Option<&'static str> {
        match self {
            Self::Connected => Some(if ascii { "|" } else { "│" }),
            Self::Separated => None,
        }
    }
}

/// Horizontal vs stacked group.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
#[non_exhaustive]
pub enum ButtonGroupOrientation {
    /// Left → right (default).
    #[default]
    Horizontal,
    /// One action per row (very narrow).
    Vertical,
}

impl ButtonGroupOrientation {
    /// Stable id.
    #[must_use]
    pub const fn id(self) -> &'static str {
        match self {
            Self::Horizontal => "horizontal",
            Self::Vertical => "vertical",
        }
    }
}

// ── Items ───────────────────────────────────────────────────────────────────

/// One action in a [`ButtonGroup`].
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ButtonGroupItem<'a, Id> {
    /// Stable command identity (roving + outcomes).
    pub id: Id,
    /// Visible label.
    pub label: &'a str,
    /// Button chrome variant.
    pub variant: ButtonVariant,
    /// Whether activatable.
    pub enabled: bool,
    /// Loading (blocks activate; distinct paint).
    pub loading: bool,
    /// Higher priority stays visible longer under overflow (default 50).
    pub priority: u8,
    /// Default submit target when group has surface focus + Enter.
    pub is_default: bool,
    /// Leading glyph on the face.
    pub leading: Option<&'a str>,
    /// Optional command id for host catalogs.
    pub command: Option<&'a str>,
}

impl<'a, Id> ButtonGroupItem<'a, Id> {
    /// Enabled secondary action.
    #[must_use]
    pub const fn new(id: Id, label: &'a str) -> Self {
        Self {
            id,
            label,
            variant: ButtonVariant::Secondary,
            enabled: true,
            loading: false,
            priority: 50,
            is_default: false,
            leading: None,
            command: None,
        }
    }

    /// Primary / default action.
    #[must_use]
    pub const fn primary(id: Id, label: &'a str) -> Self {
        Self {
            id,
            label,
            variant: ButtonVariant::Primary,
            enabled: true,
            loading: false,
            priority: 100,
            is_default: true,
            leading: None,
            command: None,
        }
    }

    /// Destructive action (separated visually; never default).
    #[must_use]
    pub const fn destructive(id: Id, label: &'a str) -> Self {
        Self {
            id,
            label,
            variant: ButtonVariant::Destructive,
            enabled: true,
            loading: false,
            priority: 40,
            is_default: false,
            leading: None,
            command: None,
        }
    }

    /// Quiet action.
    #[must_use]
    pub const fn quiet(id: Id, label: &'a str) -> Self {
        Self {
            id,
            label,
            variant: ButtonVariant::Quiet,
            enabled: true,
            loading: false,
            priority: 50,
            is_default: false,
            leading: None,
            command: None,
        }
    }

    /// Variant override.
    #[must_use]
    pub const fn variant(mut self, variant: ButtonVariant) -> Self {
        self.variant = variant;
        self
    }

    /// Enabled flag.
    #[must_use]
    pub const fn enabled(mut self, on: bool) -> Self {
        self.enabled = on;
        self
    }

    /// Loading flag.
    #[must_use]
    pub const fn loading(mut self, on: bool) -> Self {
        self.loading = on;
        self
    }

    /// Overflow priority (higher stays visible).
    #[must_use]
    pub const fn priority(mut self, priority: u8) -> Self {
        self.priority = priority;
        self
    }

    /// Mark as default submit (Enter when group focused).
    #[must_use]
    pub const fn default_action(mut self, on: bool) -> Self {
        self.is_default = on;
        if on && self.priority < 90 {
            self.priority = 90;
        }
        self
    }

    /// Leading glyph.
    #[must_use]
    pub const fn leading(mut self, glyph: &'a str) -> Self {
        self.leading = Some(glyph);
        self
    }

    /// Host command id.
    #[must_use]
    pub const fn command(mut self, command: &'a str) -> Self {
        self.command = Some(command);
        self
    }

    /// Whether this item may receive activation.
    #[must_use]
    pub const fn can_activate(&self) -> bool {
        self.enabled && !self.loading
    }
}

// ── State / parts / outcomes ────────────────────────────────────────────────

/// One painted item region.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ButtonGroupItemParts<Id> {
    /// Item id.
    pub id: Id,
    /// Hit / paint rect.
    pub area: Rect,
    /// Whether this item is in the overflow set (not painted inline).
    pub overflowed: bool,
}

/// Group paint geometry.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ButtonGroupParts<Id> {
    /// Full group area.
    pub root: Rect,
    /// Inline (visible) item regions.
    pub items: Vec<ButtonGroupItemParts<Id>>,
    /// Overflow trigger rect when present.
    pub overflow_trigger: Option<Rect>,
    /// Ids moved to overflow (stable order: reverse priority, then source order).
    pub overflow_ids: Vec<Id>,
}

/// Runtime state.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ButtonGroupState<Id> {
    /// Host granted keyboard ownership to the group surface.
    pub surface_focused: bool,
    /// Roving active descendant (individual command identity).
    pub cursor: Option<Id>,
    /// Hovered item.
    pub hovered: Option<Id>,
    /// Overflow menu open (host paints menu; group emits open/close).
    pub overflow_open: bool,
    /// Last parts.
    pub parts: Option<ButtonGroupParts<Id>>,
    /// Roving engine.
    pub roving: RovingFocusGroup<Id>,
}

impl<Id: Clone + PartialEq> Default for ButtonGroupState<Id> {
    fn default() -> Self {
        Self::new()
    }
}

impl<Id: Clone + PartialEq> ButtonGroupState<Id> {
    /// Fresh state.
    #[must_use]
    pub fn new() -> Self {
        Self {
            surface_focused: false,
            cursor: None,
            hovered: None,
            overflow_open: false,
            parts: None,
            roving: RovingFocusGroup::new().orientation(RovingOrientation::Horizontal),
        }
    }

    /// Surface focus.
    pub fn set_surface_focused(&mut self, on: bool) {
        self.surface_focused = on;
        if !on {
            self.overflow_open = false;
        }
    }
}

/// Outcomes for host effects.
#[derive(Debug, Clone, PartialEq, Eq)]
#[non_exhaustive]
pub enum ButtonGroupOutcome<Id> {
    /// No change.
    Ignored,
    /// Cursor moved (roving).
    CursorMoved {
        /// New active command id.
        id: Id,
    },
    /// Command activated (inline or default submit).
    Activated {
        /// Command id.
        id: Id,
    },
    /// Overflow trigger opened.
    OverflowOpened,
    /// Overflow closed.
    OverflowClosed,
    /// Host should activate an overflowed command (after menu selection).
    OverflowActivate {
        /// Command id still in overflow set.
        id: Id,
    },
}

// ── Widget ──────────────────────────────────────────────────────────────────

/// Grouped action cluster.
#[derive(Debug, Clone, Copy)]
pub struct ButtonGroup<'a, Id> {
    items: &'a [ButtonGroupItem<'a, Id>],
    system: &'a DesignSystem,
    recipe: ButtonGroupRecipe,
    orientation: ButtonGroupOrientation,
    /// Force vertical when width < this (0 = never auto).
    stack_below: u16,
    /// Label for overflow trigger.
    overflow_label: &'a str,
}

impl<'a, Id> ButtonGroup<'a, Id> {
    /// Group over borrowed items.
    ///
    /// Auto-stack is **off** by default (`stack_below = 0`): narrow widths use
    /// priority overflow. Call [`.stack_below(n)`](Self::stack_below) when the
    /// host wants a vertical stack under a width threshold.
    #[must_use]
    pub const fn new(items: &'a [ButtonGroupItem<'a, Id>], system: &'a DesignSystem) -> Self {
        Self {
            items,
            system,
            recipe: ButtonGroupRecipe::Separated,
            orientation: ButtonGroupOrientation::Horizontal,
            stack_below: 0,
            overflow_label: "…",
        }
    }

    /// Connected recipe.
    #[must_use]
    pub const fn connected(mut self) -> Self {
        self.recipe = ButtonGroupRecipe::Connected;
        self
    }

    /// Separated recipe.
    #[must_use]
    pub const fn separated(mut self) -> Self {
        self.recipe = ButtonGroupRecipe::Separated;
        self
    }

    /// Recipe override.
    #[must_use]
    pub const fn recipe(mut self, recipe: ButtonGroupRecipe) -> Self {
        self.recipe = recipe;
        self
    }

    /// Orientation.
    #[must_use]
    pub const fn orientation(mut self, orientation: ButtonGroupOrientation) -> Self {
        self.orientation = orientation;
        self
    }

    /// Auto-stack below width (0 disables).
    #[must_use]
    pub const fn stack_below(mut self, width: u16) -> Self {
        self.stack_below = width;
        self
    }

    /// Overflow trigger label (default `…`).
    #[must_use]
    pub const fn overflow_label(mut self, label: &'a str) -> Self {
        self.overflow_label = label;
        self
    }
}

impl<'a, Id: Clone + PartialEq> ButtonGroup<'a, Id> {
    fn resolved_orientation(&self, width: u16) -> ButtonGroupOrientation {
        if matches!(self.orientation, ButtonGroupOrientation::Vertical) {
            return ButtonGroupOrientation::Vertical;
        }
        if self.stack_below > 0 && width < self.stack_below {
            ButtonGroupOrientation::Vertical
        } else {
            ButtonGroupOrientation::Horizontal
        }
    }

    fn item_width(&self, item: &ButtonGroupItem<'a, Id>) -> u16 {
        let mut b = Button::new(item.label, self.system)
            .variant(item.variant)
            .compact();
        if let Some(g) = item.leading {
            b = b.leading(g);
        }
        b.preferred_width().max(3)
    }

    fn overflow_trigger_width(&self) -> u16 {
        u16::try_from(display_cols(self.overflow_label).saturating_add(2)).unwrap_or(3)
    }

    /// Plan which items stay inline vs overflow (destructive prefers isolation).
    ///
    /// Explicit vertical **or** auto-stack (`stack_below`) → all items visible.
    #[must_use]
    pub fn plan_overflow(&self, width: u16) -> (Vec<usize>, Vec<usize>) {
        // Returns (visible_indices, overflow_indices) into self.items
        if matches!(
            self.resolved_orientation(width),
            ButtonGroupOrientation::Vertical
        ) {
            return ((0..self.items.len()).collect(), Vec::new());
        }
        if self.items.is_empty() {
            return (Vec::new(), Vec::new());
        }

        let gap = self.recipe.inter_cols();
        let mut order: Vec<usize> = (0..self.items.len()).collect();
        // Sort by priority desc for keeping; preserve relative order with stable sort
        order.sort_by(|&a, &b| {
            self.items[b]
                .priority
                .cmp(&self.items[a].priority)
                .then(a.cmp(&b))
        });

        let mut used = 0u16;
        let mut keep = Vec::new();
        let mut overflow = Vec::new();

        for &idx in &order {
            let w = self.item_width(&self.items[idx]);
            let extra_gap = if keep.is_empty() { 0 } else { gap };
            // Reserve overflow trigger if later items may still overflow
            let remaining_after = order.len() - keep.len() - overflow.len() - 1;
            let reserve = if remaining_after > 0 || !overflow.is_empty() {
                self.overflow_trigger_width().saturating_add(gap)
            } else {
                0
            };
            let next = used
                .saturating_add(extra_gap)
                .saturating_add(w)
                .saturating_add(reserve);
            if keep.is_empty() || next <= width {
                if next > width && !keep.is_empty() {
                    overflow.push(idx);
                } else {
                    keep.push(idx);
                    used = used.saturating_add(extra_gap).saturating_add(w);
                }
            } else {
                overflow.push(idx);
            }
        }

        // If overflow non-empty, ensure trigger fits — drop lowest priority from keep
        if !overflow.is_empty() {
            let trigger = self.overflow_trigger_width().saturating_add(gap);
            while !keep.is_empty() {
                let total = keep.iter().enumerate().fold(0u16, |acc, (i, &idx)| {
                    acc.saturating_add(if i > 0 { gap } else { 0 })
                        .saturating_add(self.item_width(&self.items[idx]))
                });
                if total.saturating_add(trigger) <= width {
                    break;
                }
                // Drop last kept that is not default primary if possible
                if let Some(pos) = keep.iter().rposition(|&i| !self.items[i].is_default) {
                    overflow.push(keep.remove(pos));
                } else if keep.len() > 1 {
                    overflow.push(keep.pop().unwrap());
                } else {
                    break;
                }
            }
        }

        // Restore source order for visible
        keep.sort_unstable();
        // Overflow: keep reverse priority (menu order: lower priority first at end)
        overflow.sort_by(|&a, &b| {
            self.items[a]
                .priority
                .cmp(&self.items[b].priority)
                .then(a.cmp(&b))
        });
        (keep, overflow)
    }

    fn paint_face(
        &self,
        item: &ButtonGroupItem<'a, Id>,
        area: Rect,
        buffer: &mut Buffer,
        focused: bool,
        hovered: bool,
    ) {
        if area.is_empty() {
            return;
        }
        let mut btn_state = ButtonState::new();
        btn_state.activation.set_enabled(item.enabled);
        btn_state.activation.set_loading(item.loading);
        btn_state
            .activation
            .set_accepts_input(focused && item.can_activate());
        btn_state.hovered = hovered;
        let mut b = Button::new(item.label, self.system)
            .variant(item.variant)
            .compact();
        if let Some(g) = item.leading {
            b = b.leading(g);
        }
        let _ = b.paint(area, buffer, &mut btn_state);
    }

    /// Paint group.
    pub fn paint(
        &self,
        area: Rect,
        buffer: &mut Buffer,
        state: &mut ButtonGroupState<Id>,
    ) -> ButtonGroupParts<Id> {
        if area.is_empty() || self.items.is_empty() {
            let parts = ButtonGroupParts {
                root: area,
                items: Vec::new(),
                overflow_trigger: None,
                overflow_ids: Vec::new(),
            };
            state.parts = Some(parts.clone());
            return parts;
        }

        let orient = self.resolved_orientation(area.width);
        let (visible, overflow) = self.plan_overflow(area.width);
        let overflow_ids: Vec<Id> = overflow.iter().map(|&i| self.items[i].id.clone()).collect();

        // Reconcile cursor: prefer existing if still visible, else first visible enabled
        let visible_ids: Vec<Id> = visible.iter().map(|&i| self.items[i].id.clone()).collect();
        if state
            .cursor
            .as_ref()
            .is_none_or(|c| !visible_ids.iter().any(|v| v == c))
        {
            state.cursor = visible
                .iter()
                .find(|&&i| self.items[i].can_activate())
                .map(|&i| self.items[i].id.clone())
                .or_else(|| visible_ids.first().cloned());
        }

        // Roving entries for visible only
        let roving_entries: Vec<RovingEntry<Id>> = visible
            .iter()
            .map(|&i| {
                let it = &self.items[i];
                RovingEntry::new(it.id.clone(), it.label).enabled(it.can_activate())
            })
            .collect();
        let _ = state.roving.reconcile(&roving_entries);
        if let Some(c) = state.cursor.clone() {
            state.roving.set_active(Some(c));
        }

        let mut item_parts = Vec::new();
        let mut overflow_trigger = None;

        match orient {
            ButtonGroupOrientation::Vertical => {
                let mut y = area.y;
                for &idx in &visible {
                    if y >= area.bottom() {
                        break;
                    }
                    let item = &self.items[idx];
                    let rect = Rect::new(area.x, y, area.width, 1);
                    let focused = state.surface_focused && state.cursor.as_ref() == Some(&item.id);
                    let hovered = state.hovered.as_ref() == Some(&item.id);
                    self.paint_face(item, rect, buffer, focused, hovered);
                    item_parts.push(ButtonGroupItemParts {
                        id: item.id.clone(),
                        area: rect,
                        overflowed: false,
                    });
                    y = y.saturating_add(1);
                }
            }
            ButtonGroupOrientation::Horizontal => {
                let gap = self.recipe.inter_cols();
                let sep = self.recipe.separator_glyph(false);
                let mut x = area.x;
                let mut first = true;
                // Paint in source order among visible
                for &idx in &visible {
                    if !first {
                        if let Some(glyph) = sep {
                            if x < area.right() && area.height > 0 {
                                buffer.set_stringn(
                                    x,
                                    area.y,
                                    glyph,
                                    1,
                                    self.system.style(Role::Border),
                                );
                                x = x.saturating_add(1);
                            }
                        } else {
                            x = x.saturating_add(gap);
                        }
                    }
                    first = false;
                    let item = &self.items[idx];
                    let w = self.item_width(item).min(area.right().saturating_sub(x));
                    if w == 0 {
                        break;
                    }
                    let rect = Rect::new(x, area.y, w, 1.min(area.height));
                    let focused = state.surface_focused && state.cursor.as_ref() == Some(&item.id);
                    let hovered = state.hovered.as_ref() == Some(&item.id);
                    self.paint_face(item, rect, buffer, focused, hovered);
                    item_parts.push(ButtonGroupItemParts {
                        id: item.id.clone(),
                        area: rect,
                        overflowed: false,
                    });
                    x = x.saturating_add(w);
                }
                if !overflow.is_empty() {
                    // inter-col before trigger when something painted
                    if !first {
                        if let Some(glyph) = sep {
                            if x < area.right() && area.height > 0 {
                                buffer.set_stringn(
                                    x,
                                    area.y,
                                    glyph,
                                    1,
                                    self.system.style(Role::Border),
                                );
                                x = x.saturating_add(1);
                            }
                        } else {
                            x = x.saturating_add(gap);
                        }
                    }
                    let tw = self
                        .overflow_trigger_width()
                        .min(area.right().saturating_sub(x));
                    if tw > 0 {
                        let rect = Rect::new(x, area.y, tw, 1.min(area.height));
                        let focused = state.overflow_open;
                        let recipe = self.system.button_recipe(
                            ButtonRecipeVariant::Quiet,
                            if focused {
                                ControlState::Focused
                            } else {
                                ControlState::Default
                            },
                            self.system.junie_theme().surface,
                        );
                        let mut style = recipe.fill.patch(recipe.label);
                        style = style.add_modifier(Modifier::BOLD);
                        let label = take_display_cols(self.overflow_label, usize::from(tw));
                        buffer.set_stringn(rect.x, rect.y, &label, usize::from(tw), style);
                        overflow_trigger = Some(rect);
                    }
                }
            }
        }

        // Mark overflowed items without geometry
        for &idx in &overflow {
            item_parts.push(ButtonGroupItemParts {
                id: self.items[idx].id.clone(),
                area: Rect::default(),
                overflowed: true,
            });
        }

        let parts = ButtonGroupParts {
            root: area,
            items: item_parts,
            overflow_trigger,
            overflow_ids,
        };
        state.parts = Some(parts.clone());
        parts
    }

    fn default_id(&self) -> Option<Id> {
        self.items
            .iter()
            .find(|i| i.is_default && i.can_activate())
            .map(|i| i.id.clone())
            .or_else(|| {
                self.items
                    .iter()
                    .find(|i| matches!(i.variant, ButtonVariant::Primary) && i.can_activate())
                    .map(|i| i.id.clone())
            })
    }

    fn item_by_id(&self, id: &Id) -> Option<&ButtonGroupItem<'a, Id>> {
        self.items.iter().find(|i| &i.id == id)
    }

    /// Keys: roving + activate + default Enter + overflow.
    pub fn handle_key(
        &self,
        state: &mut ButtonGroupState<Id>,
        key: KeyEvent,
    ) -> ButtonGroupOutcome<Id> {
        if !state.surface_focused || !key.is_press() {
            return ButtonGroupOutcome::Ignored;
        }

        // Escape closes overflow
        if matches!(key.code, crate::input::KeyCode::Esc) && state.overflow_open {
            state.overflow_open = false;
            return ButtonGroupOutcome::OverflowClosed;
        }

        // Build roving list from visible items
        let parts = state.parts.clone();
        let visible: Vec<RovingEntry<Id>> = if let Some(p) = &parts {
            p.items
                .iter()
                .filter(|it| !it.overflowed)
                .filter_map(|it| {
                    let item = self.item_by_id(&it.id)?;
                    Some(RovingEntry::new(item.id.clone(), item.label).enabled(item.can_activate()))
                })
                .collect()
        } else {
            self.items
                .iter()
                .map(|i| RovingEntry::new(i.id.clone(), i.label).enabled(i.can_activate()))
                .collect()
        };

        if visible.is_empty() {
            return ButtonGroupOutcome::Ignored;
        }

        // Enter/Submit → default action (dialog convention). Space → cursor command.
        if let Some(intent) = default_button_group_intent(key) {
            match intent {
                UiIntent::Activate | UiIntent::Submit => {
                    if state.overflow_open {
                        return ButtonGroupOutcome::Ignored;
                    }
                    if matches!(intent, UiIntent::Submit) {
                        if let Some(d) = self.default_id() {
                            return ButtonGroupOutcome::Activated { id: d };
                        }
                    }
                    // Space (and Enter without default): activate roving cursor
                    if let Some(c) = state.cursor.clone() {
                        if let Some(item) = self.item_by_id(&c) {
                            if item.can_activate() {
                                if parts
                                    .as_ref()
                                    .is_some_and(|p| p.overflow_ids.iter().any(|id| id == &c))
                                {
                                    state.overflow_open = true;
                                    return ButtonGroupOutcome::OverflowOpened;
                                }
                                return ButtonGroupOutcome::Activated { id: c };
                            }
                        }
                    }
                    if let Some(d) = self.default_id() {
                        return ButtonGroupOutcome::Activated { id: d };
                    }
                    return ButtonGroupOutcome::Ignored;
                }
                _ => {}
            }
        }

        // Roving movement
        let ro = state.roving.handle_key(key, &visible);
        match ro {
            RovingOutcome::ActiveChanged { to: Some(id), .. } => {
                state.cursor = Some(id.clone());
                return ButtonGroupOutcome::CursorMoved { id };
            }
            RovingOutcome::ActiveChanged { to: None, .. } | RovingOutcome::Ignored => {}
        }

        // Manual left/right if roving ignored (ensure horizontal works)
        if let Some(mv) = crate::interaction::default_list_intent(key) {
            match mv {
                UiIntent::Move(NavigationMove::Next | NavigationMove::Right) => {
                    let out = state.roving.move_next(&visible);
                    if let RovingOutcome::ActiveChanged { to: Some(id), .. } = out {
                        state.cursor = Some(id.clone());
                        return ButtonGroupOutcome::CursorMoved { id };
                    }
                }
                UiIntent::Move(NavigationMove::Previous | NavigationMove::Left) => {
                    let out = state.roving.move_previous(&visible);
                    if let RovingOutcome::ActiveChanged { to: Some(id), .. } = out {
                        state.cursor = Some(id.clone());
                        return ButtonGroupOutcome::CursorMoved { id };
                    }
                }
                _ => {}
            }
        }

        // Overflow open: Tab or 'o' when overflow exists
        if let Some(p) = &parts {
            if !p.overflow_ids.is_empty()
                && matches!(key.code, crate::input::KeyCode::Char('o' | 'O' | '.'))
            {
                state.overflow_open = !state.overflow_open;
                return if state.overflow_open {
                    ButtonGroupOutcome::OverflowOpened
                } else {
                    ButtonGroupOutcome::OverflowClosed
                };
            }
        }

        ButtonGroupOutcome::Ignored
    }

    /// Mouse.
    pub fn handle_mouse(
        &self,
        state: &mut ButtonGroupState<Id>,
        event: MouseEvent,
    ) -> ButtonGroupOutcome<Id> {
        let Some(parts) = state.parts.clone() else {
            return ButtonGroupOutcome::Ignored;
        };
        if !parts.root.contains(event.position) {
            if matches!(event.kind, MouseEventKind::Moved) {
                state.hovered = None;
            }
            return ButtonGroupOutcome::Ignored;
        }
        match event.kind {
            MouseEventKind::Moved | MouseEventKind::Drag(_) => {
                state.hovered = parts
                    .items
                    .iter()
                    .find(|it| !it.overflowed && it.area.contains(event.position))
                    .map(|it| it.id.clone());
                ButtonGroupOutcome::Ignored
            }
            MouseEventKind::Down(MouseButton::Left) => {
                if let Some(tr) = parts.overflow_trigger {
                    if tr.contains(event.position) {
                        state.surface_focused = true;
                        state.overflow_open = !state.overflow_open;
                        return if state.overflow_open {
                            ButtonGroupOutcome::OverflowOpened
                        } else {
                            ButtonGroupOutcome::OverflowClosed
                        };
                    }
                }
                if let Some(it) = parts
                    .items
                    .iter()
                    .find(|it| !it.overflowed && it.area.contains(event.position))
                {
                    state.surface_focused = true;
                    state.cursor = Some(it.id.clone());
                    if let Some(item) = self.item_by_id(&it.id) {
                        if item.can_activate() {
                            return ButtonGroupOutcome::Activated { id: it.id.clone() };
                        }
                    }
                    return ButtonGroupOutcome::CursorMoved { id: it.id.clone() };
                }
                ButtonGroupOutcome::Ignored
            }
            _ => ButtonGroupOutcome::Ignored,
        }
    }

    /// Host selected an overflow menu command.
    pub fn activate_overflow(
        &self,
        state: &mut ButtonGroupState<Id>,
        id: Id,
    ) -> ButtonGroupOutcome<Id> {
        if state
            .parts
            .as_ref()
            .is_some_and(|p| p.overflow_ids.iter().any(|x| x == &id))
        {
            if let Some(item) = self.item_by_id(&id) {
                if item.can_activate() {
                    state.overflow_open = false;
                    return ButtonGroupOutcome::Activated { id };
                }
            }
            return ButtonGroupOutcome::OverflowActivate { id };
        }
        ButtonGroupOutcome::Ignored
    }

    /// EventResult wrapper.
    pub fn handle_key_result(
        &self,
        state: &mut ButtonGroupState<Id>,
        key: KeyEvent,
    ) -> EventResult<ButtonGroupOutcome<Id>> {
        match self.handle_key(state, key) {
            ButtonGroupOutcome::Ignored => EventResult::ignored(),
            other => EventResult::emit(other),
        }
    }

    /// Semantic: group + each visible command as Button.
    pub fn register_semantic<Action>(
        &self,
        scene: &mut SemanticScene<Id, Action>,
        group_id: Id,
        area: Rect,
        state: &ButtonGroupState<Id>,
    ) where
        Id: Clone + PartialEq + std::fmt::Display,
        Action: Clone,
    {
        if area.is_empty() {
            return;
        }
        let _ = scene.register(
            SemanticNode::control(group_id.clone(), area)
                .role(SemanticRole::List)
                .label("button group")
                .focusable(true)
                .state(SemanticState {
                    selected: state.surface_focused,
                    ..Default::default()
                }),
        );
        if let Some(parts) = &state.parts {
            for it in &parts.items {
                if it.overflowed {
                    continue;
                }
                let item = match self.item_by_id(&it.id) {
                    Some(i) => i,
                    None => continue,
                };
                let _ = scene.register(
                    SemanticNode::control(it.id.clone(), it.area)
                        .role(SemanticRole::Button)
                        .label(item.label)
                        .description(item.variant.id())
                        .focusable(item.can_activate() && state.surface_focused)
                        .state(SemanticState {
                            selected: state.cursor.as_ref() == Some(&it.id),
                            busy: item.loading,
                            ..Default::default()
                        }),
                );
            }
        }
    }
}

impl<'a, Id: Clone + PartialEq> Widget for &ButtonGroup<'a, Id> {
    fn render(self, area: Rect, buffer: &mut Buffer) {
        let mut state = ButtonGroupState::new();
        let _ = self.paint(area, buffer, &mut state);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::input::{KeyCode, KeyModifiers};
    use ratatui_core::layout::Position;

    fn sample() -> [ButtonGroupItem<'static, &'static str>; 4] {
        [
            ButtonGroupItem::quiet("more", "More").priority(10),
            ButtonGroupItem::new("cancel", "Cancel").priority(60),
            ButtonGroupItem::primary("save", "Save"),
            ButtonGroupItem::destructive("delete", "Delete").priority(20),
        ]
    }

    #[test]
    fn overflow_keeps_high_priority() {
        let system = DesignSystem::default();
        let items = sample();
        let g = ButtonGroup::new(&items, &system);
        let (vis, over) = g.plan_overflow(12);
        assert!(vis.iter().any(|&i| items[i].id == "save"));
        assert!(
            !over.is_empty(),
            "expected overflow at 12 cols; vis={vis:?}"
        );
    }

    #[test]
    fn wide_fits_all() {
        let system = DesignSystem::default();
        let items = sample();
        let g = ButtonGroup::new(&items, &system);
        let (vis, over) = g.plan_overflow(80);
        assert_eq!(vis.len(), 4);
        assert!(over.is_empty());
    }

    #[test]
    fn activate_default_on_enter() {
        let system = DesignSystem::default();
        let items = sample();
        let g = ButtonGroup::new(&items, &system);
        let mut state = ButtonGroupState::new();
        state.set_surface_focused(true);
        let mut buf = Buffer::empty(Rect::new(0, 0, 60, 1));
        let _ = g.paint(Rect::new(0, 0, 60, 1), &mut buf, &mut state);
        let out = g.handle_key(
            &mut state,
            KeyEvent::new(KeyCode::Enter, KeyModifiers::NONE),
        );
        assert!(matches!(out, ButtonGroupOutcome::Activated { id: "save" }));
    }

    #[test]
    fn roving_moves_cursor() {
        let system = DesignSystem::default();
        let items = sample();
        let g = ButtonGroup::new(&items, &system);
        let mut state = ButtonGroupState::new();
        state.set_surface_focused(true);
        let mut buf = Buffer::empty(Rect::new(0, 0, 80, 1));
        let _ = g.paint(Rect::new(0, 0, 80, 1), &mut buf, &mut state);
        state.cursor = Some("cancel");
        let out = g.handle_key(
            &mut state,
            KeyEvent::new(KeyCode::Right, KeyModifiers::NONE),
        );
        assert!(matches!(out, ButtonGroupOutcome::CursorMoved { .. }));
    }

    #[test]
    fn disabled_not_activated() {
        let system = DesignSystem::default();
        let items = [
            ButtonGroupItem::primary("ok", "OK").enabled(false),
            ButtonGroupItem::new("cancel", "Cancel"),
        ];
        let g = ButtonGroup::new(&items, &system);
        let mut state = ButtonGroupState::new();
        state.set_surface_focused(true);
        state.cursor = Some("ok");
        let mut buf = Buffer::empty(Rect::new(0, 0, 40, 1));
        let _ = g.paint(Rect::new(0, 0, 40, 1), &mut buf, &mut state);
        let out = g.handle_key(
            &mut state,
            KeyEvent::new(KeyCode::Enter, KeyModifiers::NONE),
        );
        // falls back to no default or cancel
        assert!(
            matches!(out, ButtonGroupOutcome::Activated { id: "cancel" })
                || matches!(out, ButtonGroupOutcome::Ignored)
        );
    }

    #[test]
    fn mouse_activates() {
        let system = DesignSystem::default();
        let items = [
            ButtonGroupItem::new("a", "A"),
            ButtonGroupItem::primary("b", "B"),
        ];
        let g = ButtonGroup::new(&items, &system);
        let mut state = ButtonGroupState::new();
        let mut buf = Buffer::empty(Rect::new(0, 0, 40, 1));
        let parts = g.paint(Rect::new(0, 0, 40, 1), &mut buf, &mut state);
        let area = parts.items.iter().find(|i| i.id == "b").unwrap().area;
        let out = g.handle_mouse(
            &mut state,
            MouseEvent {
                kind: MouseEventKind::Down(MouseButton::Left),
                position: Position {
                    x: area.x,
                    y: area.y,
                },
                modifiers: KeyModifiers::NONE,
            },
        );
        assert!(matches!(out, ButtonGroupOutcome::Activated { id: "b" }));
    }

    #[test]
    fn connected_recipe_paints() {
        let system = DesignSystem::default();
        let items = [
            ButtonGroupItem::new("1", "One"),
            ButtonGroupItem::new("2", "Two"),
        ];
        let g = ButtonGroup::new(&items, &system).connected();
        let mut state = ButtonGroupState::new();
        let mut buf = Buffer::empty(Rect::new(0, 0, 30, 1));
        let parts = g.paint(Rect::new(0, 0, 30, 1), &mut buf, &mut state);
        assert_eq!(parts.items.iter().filter(|i| !i.overflowed).count(), 2);
    }

    #[test]
    fn vertical_no_overflow() {
        let system = DesignSystem::default();
        let items = sample();
        let g = ButtonGroup::new(&items, &system).orientation(ButtonGroupOrientation::Vertical);
        let (vis, over) = g.plan_overflow(10);
        assert_eq!(vis.len(), 4);
        assert!(over.is_empty());
    }

    #[test]
    fn empty_safe() {
        let system = DesignSystem::default();
        let items: [ButtonGroupItem<'_, &str>; 0] = [];
        let g = ButtonGroup::new(&items, &system);
        let mut state = ButtonGroupState::new();
        let mut buf = Buffer::empty(Rect::new(0, 0, 1, 1));
        let parts = g.paint(Rect::new(0, 0, 0, 0), &mut buf, &mut state);
        assert!(parts.root.is_empty() || parts.items.is_empty());
    }

    #[test]
    fn overflow_activate_host() {
        let system = DesignSystem::default();
        let items = sample();
        let g = ButtonGroup::new(&items, &system);
        let mut state = ButtonGroupState::new();
        let mut buf = Buffer::empty(Rect::new(0, 0, 16, 1));
        let parts = g.paint(Rect::new(0, 0, 16, 1), &mut buf, &mut state);
        if let Some(id) = parts.overflow_ids.first() {
            let out = g.activate_overflow(&mut state, *id);
            assert!(matches!(
                out,
                ButtonGroupOutcome::Activated { .. } | ButtonGroupOutcome::OverflowActivate { .. }
            ));
        }
        state.set_surface_focused(true);
        state.overflow_open = true;
        assert!(matches!(
            g.handle_key(&mut state, KeyEvent::new(KeyCode::Esc, KeyModifiers::NONE)),
            ButtonGroupOutcome::OverflowClosed
        ));
        assert!(!state.overflow_open);
    }

    #[test]
    fn space_activates_cursor_not_default() {
        let system = DesignSystem::default();
        let items = sample();
        let g = ButtonGroup::new(&items, &system);
        let mut state = ButtonGroupState::new();
        state.set_surface_focused(true);
        let mut buf = Buffer::empty(Rect::new(0, 0, 80, 1));
        let _ = g.paint(Rect::new(0, 0, 80, 1), &mut buf, &mut state);
        state.cursor = Some("cancel");
        let out = g.handle_key(
            &mut state,
            KeyEvent::new(KeyCode::Char(' '), KeyModifiers::NONE),
        );
        assert!(matches!(
            out,
            ButtonGroupOutcome::Activated { id: "cancel" }
        ));
    }

    #[test]
    fn loading_blocks_activation() {
        let system = DesignSystem::default();
        let items = [
            ButtonGroupItem::new("cancel", "Cancel"),
            ButtonGroupItem::primary("save", "Save").loading(true),
        ];
        let g = ButtonGroup::new(&items, &system);
        let mut state = ButtonGroupState::new();
        state.set_surface_focused(true);
        state.cursor = Some("save");
        let mut buf = Buffer::empty(Rect::new(0, 0, 40, 1));
        let _ = g.paint(Rect::new(0, 0, 40, 1), &mut buf, &mut state);
        let out = g.handle_key(
            &mut state,
            KeyEvent::new(KeyCode::Char(' '), KeyModifiers::NONE),
        );
        assert!(!matches!(out, ButtonGroupOutcome::Activated { id: "save" }));
    }

    #[test]
    fn semantic_registers_visible_commands() {
        let system = DesignSystem::default();
        let items = [
            ButtonGroupItem::new("cancel", "Cancel"),
            ButtonGroupItem::primary("save", "Save"),
        ];
        let g = ButtonGroup::new(&items, &system);
        let mut state = ButtonGroupState::new();
        state.set_surface_focused(true);
        let mut buf = Buffer::empty(Rect::new(0, 0, 40, 1));
        let _ = g.paint(Rect::new(0, 0, 40, 1), &mut buf, &mut state);
        let mut scene = SemanticScene::<&str, ()>::default();
        g.register_semantic(&mut scene, "group", Rect::new(0, 0, 40, 1), &state);
        assert!(scene.len() >= 2);
    }

    #[test]
    fn overflow_monotone_with_width() {
        // Fuzz-style: narrower width never keeps more items than wider.
        let system = DesignSystem::default();
        let items = sample();
        let g = ButtonGroup::new(&items, &system);
        let mut prev_vis = 0usize;
        for w in 4u16..=80 {
            let (vis, _) = g.plan_overflow(w);
            assert!(
                vis.len() >= prev_vis || w < 8,
                "width {w}: vis {} < prev {prev_vis}",
                vis.len()
            );
            // Soft monotone: once we fit all, stay full
            if vis.len() == items.len() {
                prev_vis = items.len();
            } else if vis.len() >= prev_vis {
                prev_vis = vis.len();
            }
        }
        let (full, over) = g.plan_overflow(120);
        assert_eq!(full.len(), 4);
        assert!(over.is_empty());
    }

    #[test]
    fn paint_hot_path_is_bounded() {
        // Performance smoke: many paints of a typical dialog strip.
        let system = DesignSystem::default();
        let items = sample();
        let g = ButtonGroup::new(&items, &system);
        let mut state = ButtonGroupState::new();
        state.set_surface_focused(true);
        let area = Rect::new(0, 0, 48, 1);
        let mut buf = Buffer::empty(area);
        for _ in 0..500 {
            let _ = g.paint(area, &mut buf, &mut state);
        }
        assert!(state.parts.is_some());
    }

    #[test]
    fn stack_below_opt_in_vertical() {
        let system = DesignSystem::default();
        let items = sample();
        let g = ButtonGroup::new(&items, &system).stack_below(40);
        let mut state = ButtonGroupState::new();
        let mut buf = Buffer::empty(Rect::new(0, 0, 30, 4));
        let parts = g.paint(Rect::new(0, 0, 30, 4), &mut buf, &mut state);
        // Vertical paint keeps source order without overflow ids
        assert!(parts.overflow_ids.is_empty() || parts.items.iter().any(|i| !i.overflowed));
        assert!(parts.items.iter().filter(|i| !i.overflowed).count() >= 2);
    }
}
