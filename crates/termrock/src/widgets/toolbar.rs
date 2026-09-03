// SPDX-FileCopyrightText: 2026 Alexey Zhokhov
// SPDX-License-Identifier: Apache-2.0

//! Toolbar — roving-focus action strip for contextual commands.
//!
//! **Surface focus vs content selection.** When [`ToolbarState::surface_focused`]
//! is true the host has given keyboard ownership to the toolbar. The roving
//! **cursor** (active descendant) is independent of content-list selection in
//! the main pane. Content widgets keep their own selection models.
//!
//! Complements [`super::ActionBar`]: ActionBar is a simple paint + hit strip
//! (dialog footers). Toolbar adds item kinds, overflow, key hints, intents, and
//! integrated roving navigation.
//!
//! Behavioral references: desktop toolbars, Radix Toolbar (roving), adapted to
//! terminal cells and [`UiIntent`].
use ratatui_core::{buffer::Buffer, layout::Rect, widgets::StatefulWidget};

use crate::input::{KeyEvent, MouseButton, MouseEvent, MouseEventKind};
use crate::interaction::{
    HitRegion, RovingEntry, RovingFocusGroup, RovingOrientation, RovingOutcome, UiIntent,
    default_button_intent,
};
use crate::style::{ButtonRecipeVariant, ControlState, DesignSystem, GlyphSet, RecipeFamily, Role};
use crate::text::{display_cols, take_display_cols};

/// Layout orientation.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
#[non_exhaustive]
pub enum ToolbarOrientation {
    /// Left → right strip (default).
    #[default]
    Horizontal,
    /// Top → bottom compact strip.
    Vertical,
}

impl ToolbarOrientation {
    /// Stable id.
    #[must_use]
    pub const fn id(self) -> &'static str {
        match self {
            Self::Horizontal => "horizontal",
            Self::Vertical => "vertical",
        }
    }
}

/// Visual density recipe.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
#[non_exhaustive]
pub enum ToolbarVariant {
    /// Comfortable padding around labels.
    #[default]
    Default,
    /// Compact: tighter padding, prefer icons when present.
    Compact,
}

impl ToolbarVariant {
    /// Stable id.
    #[must_use]
    pub const fn id(self) -> &'static str {
        match self {
            Self::Default => "default",
            Self::Compact => "compact",
        }
    }
}

/// Kind of one toolbar slot.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
#[non_exhaustive]
pub enum ToolbarItemKind {
    /// Activatable command button (default).
    #[default]
    Action,
    /// Toggle button (host owns pressed state).
    Toggle {
        /// Whether currently pressed/on.
        pressed: bool,
    },
    /// Non-interactive visual separator.
    Separator,
    /// Non-interactive label / group title.
    Label,
}

/// One toolbar item (borrowed projection; stable id).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ToolbarItem<'a, Id> {
    /// Stable identity for activation / roving.
    pub id: Id,
    /// Visible label (may be empty when icon-only compact).
    pub label: &'a str,
    /// Optional leading icon/glyph.
    pub icon: Option<&'a str>,
    /// Optional key-hint text (e.g. "C-s").
    pub hint: Option<&'a str>,
    /// Whether activatable (separators/labels ignore).
    pub enabled: bool,
    /// Higher priority stays visible longer under contraction (default 50).
    pub priority: u8,
    /// Item kind.
    pub kind: ToolbarItemKind,
    /// Optional generated command id for host command catalogs.
    pub command: Option<&'a str>,
}

impl<'a, Id> ToolbarItem<'a, Id> {
    /// Enabled action with label.
    #[must_use]
    pub const fn action(id: Id, label: &'a str) -> Self {
        Self {
            id,
            label,
            icon: None,
            hint: None,
            enabled: true,
            priority: 50,
            kind: ToolbarItemKind::Action,
            command: None,
        }
    }

    /// Toggle item.
    #[must_use]
    pub const fn toggle(id: Id, label: &'a str, pressed: bool) -> Self {
        Self {
            id,
            label,
            icon: None,
            hint: None,
            enabled: true,
            priority: 50,
            kind: ToolbarItemKind::Toggle { pressed },
            command: None,
        }
    }

    /// Separator (non-interactive).
    #[must_use]
    pub const fn separator(id: Id) -> Self {
        Self {
            id,
            label: "",
            icon: None,
            hint: None,
            enabled: false,
            priority: 100,
            kind: ToolbarItemKind::Separator,
            command: None,
        }
    }
    /// Icon glyph.
    #[must_use]
    pub const fn icon(mut self, icon: &'a str) -> Self {
        self.icon = Some(icon);
        self
    }

    /// Key hint.
    #[must_use]
    pub const fn hint(mut self, hint: &'a str) -> Self {
        self.hint = Some(hint);
        self
    }

    /// Priority (higher survives overflow longer).
    #[must_use]
    pub const fn priority(mut self, priority: u8) -> Self {
        self.priority = priority;
        self
    }

    /// Enabled flag.
    #[must_use]
    pub const fn enabled(mut self, enabled: bool) -> Self {
        self.enabled = enabled;
        self
    }

    /// Command catalog id.
    #[must_use]
    pub const fn command(mut self, command: &'a str) -> Self {
        self.command = Some(command);
        self
    }

    /// Whether this item participates in roving / activation.
    #[must_use]
    pub const fn is_interactive(&self) -> bool {
        matches!(
            self.kind,
            ToolbarItemKind::Action | ToolbarItemKind::Toggle { .. }
        ) && self.enabled
    }
}

/// Typed outcomes (no side effects).
#[derive(Debug, Clone, PartialEq, Eq)]
#[non_exhaustive]
pub enum ToolbarOutcome<Id> {
    /// No change.
    Ignored,
    /// Action activated (Enter / click).
    Activated(Id),
    /// Toggle flipped (host applies pressed).
    Toggled {
        /// Item id.
        id: Id,
        /// Suggested new pressed state (inverse of projection).
        pressed: bool,
    },
    /// Roving cursor moved.
    CursorMoved {
        /// Previous active.
        from: Option<Id>,
        /// New active.
        to: Option<Id>,
    },
    /// Overflow menu requested open (host paints menu with overflowed ids).
    OverflowOpened,
    /// Overflow menu closed.
    OverflowClosed,
}

/// Runtime state: surface focus + roving cursor (not content selection).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ToolbarState<Id> {
    /// Host set true when the toolbar surface owns keyboard input.
    pub surface_focused: bool,
    /// Active-descendant cursor among interactive items.
    pub roving: RovingFocusGroup<Id>,
    /// Hit regions from last paint (interactive + overflow chip).
    pub regions: Vec<HitRegion<Id>>,
    /// Whether overflow chip is "open" (host menu).
    pub overflow_open: bool,
    /// Indices of items pushed to overflow last paint (for host menus).
    pub overflow_indices: Vec<usize>,
}

impl<Id> Default for ToolbarState<Id> {
    fn default() -> Self {
        Self::new()
    }
}

impl<Id> ToolbarState<Id> {
    /// Unfocused surface, empty roving, horizontal wrap.
    #[must_use]
    pub fn new() -> Self {
        Self {
            surface_focused: false,
            roving: RovingFocusGroup::new()
                .orientation(RovingOrientation::Horizontal)
                .wrap(true),
            regions: Vec::new(),
            overflow_open: false,
            overflow_indices: Vec::new(),
        }
    }

    /// Surface focus flag.
    pub const fn set_surface_focused(&mut self, focused: bool) {
        self.surface_focused = focused;
    }

    /// Active descendant (roving cursor).
    #[must_use]
    pub const fn cursor(&self) -> Option<&Id> {
        self.roving.active()
    }
}

impl<Id: Clone + PartialEq> ToolbarState<Id> {
    /// Sets roving active id.
    pub fn set_cursor(&mut self, id: Option<Id>) {
        self.roving.set_active(id);
    }

    /// Build roving entries for interactive items only (visible + overflow chip).
    fn roving_entries<'a>(
        items: &'a [ToolbarItem<'a, Id>],
        visible: &[usize],
        overflow_id: Option<&Id>,
        has_overflow: bool,
    ) -> Vec<RovingEntry<'a, Id>> {
        let mut out = Vec::new();
        for &i in visible {
            let item = &items[i];
            if item.is_interactive() {
                out.push(RovingEntry::new(item.id.clone(), item.label).enabled(true));
            }
        }
        if has_overflow {
            if let Some(oid) = overflow_id {
                out.push(RovingEntry::new(oid.clone(), "More").enabled(true));
            }
        }
        out
    }
}

/// Roving-focus toolbar strip.
#[derive(Debug, Clone)]
pub struct Toolbar<'a, Id> {
    items: &'a [ToolbarItem<'a, Id>],
    system: &'a DesignSystem,
    orientation: ToolbarOrientation,
    variant: ToolbarVariant,
    /// Synthetic id for the overflow "More" chip (required for overflow UX).
    overflow_id: Option<Id>,
}

impl<'a, Id> Toolbar<'a, Id> {
    /// Toolbar over borrowed items.
    #[must_use]
    pub const fn new(items: &'a [ToolbarItem<'a, Id>], system: &'a DesignSystem) -> Self {
        Self {
            items,
            system,
            orientation: ToolbarOrientation::Horizontal,
            variant: ToolbarVariant::Default,
            overflow_id: None,
            // Seeded from the system: a widget that defaults to false is
            // claiming the terminal has Unicode and colour before anyone
            // asked it. Builders below still force either way.
        }
    }

    /// Orientation.
    #[must_use]
    pub const fn orientation(mut self, orientation: ToolbarOrientation) -> Self {
        self.orientation = orientation;
        self
    }

    /// Vertical compact strip.
    #[must_use]
    pub const fn vertical(mut self) -> Self {
        self.orientation = ToolbarOrientation::Vertical;
        self
    }

    /// Variant recipe.
    #[must_use]
    pub const fn variant(mut self, variant: ToolbarVariant) -> Self {
        self.variant = variant;
        self
    }

    /// Compact density.
    #[must_use]
    pub const fn compact(mut self) -> Self {
        self.variant = ToolbarVariant::Compact;
        self
    }

    /// Overflow chip identity (when low-priority items do not fit).
    #[must_use]
    pub fn overflow_id(mut self, id: Id) -> Self {
        self.overflow_id = Some(id);
        self
    }
}

impl<Id: Clone + PartialEq> Toolbar<'_, Id> {
    /// Plan which items stay visible vs overflow under `area` budget.
    #[must_use]
    pub fn plan(&self, area: Rect) -> ToolbarPlan {
        plan_items(
            self.items,
            area,
            self.orientation,
            self.variant,
            self.overflow_id.is_some(),
            self.system.glyphs,
        )
    }

    /// Paint the toolbar into `buffer`.
    pub fn paint(&self, area: Rect, buffer: &mut Buffer, state: &mut ToolbarState<Id>) {
        StatefulWidget::render(self, area, buffer, state);
    }

    /// Key path: requires surface focus. Roving + Activate.
    pub fn handle_key(
        &self,
        state: &mut ToolbarState<Id>,
        key: KeyEvent,
        area: Rect,
    ) -> ToolbarOutcome<Id> {
        if !state.surface_focused || !key.is_press() {
            return ToolbarOutcome::Ignored;
        }
        let plan = self.plan(area);
        let entries = ToolbarState::roving_entries(
            self.items,
            &plan.visible,
            self.overflow_id.as_ref(),
            plan.has_overflow,
        );
        let _ = state.roving.reconcile(&entries);

        // Esc closes overflow
        if matches!(key.code, crate::input::KeyCode::Esc) && state.overflow_open {
            state.overflow_open = false;
            return ToolbarOutcome::OverflowClosed;
        }

        if let Some(intent) = default_button_intent(key) {
            if matches!(intent, UiIntent::Activate | UiIntent::Submit) {
                return self.activate_cursor(state);
            }
        }

        // Host should construct ToolbarState::horizontal/vertical to match orientation.
        match state.roving.handle_key(key, &entries) {
            RovingOutcome::Ignored => ToolbarOutcome::Ignored,
            RovingOutcome::ActiveChanged { from, to } => ToolbarOutcome::CursorMoved { from, to },
        }
    }

    /// Mouse down on painted regions.
    pub fn handle_mouse(
        &self,
        state: &mut ToolbarState<Id>,
        event: MouseEvent,
    ) -> ToolbarOutcome<Id> {
        if event.kind != MouseEventKind::Down(MouseButton::Left) {
            return ToolbarOutcome::Ignored;
        }
        for region in &state.regions {
            if region.area.contains(event.position) {
                // Overflow chip?
                if self.overflow_id.as_ref() == Some(&region.id) {
                    state.overflow_open = !state.overflow_open;
                    state.roving.set_active(Some(region.id.clone()));
                    return if state.overflow_open {
                        ToolbarOutcome::OverflowOpened
                    } else {
                        ToolbarOutcome::OverflowClosed
                    };
                }
                if let Some(item) = self.items.iter().find(|i| i.id == region.id) {
                    state.roving.set_active(Some(region.id.clone()));
                    return match item.kind {
                        ToolbarItemKind::Toggle { pressed } => ToolbarOutcome::Toggled {
                            id: region.id.clone(),
                            pressed: !pressed,
                        },
                        ToolbarItemKind::Action if item.enabled => {
                            ToolbarOutcome::Activated(region.id.clone())
                        }
                        _ => ToolbarOutcome::Ignored,
                    };
                }
            }
        }
        ToolbarOutcome::Ignored
    }

    fn activate_cursor(&self, state: &mut ToolbarState<Id>) -> ToolbarOutcome<Id> {
        let Some(active) = state.roving.active().cloned() else {
            return ToolbarOutcome::Ignored;
        };
        if self.overflow_id.as_ref() == Some(&active) {
            state.overflow_open = !state.overflow_open;
            return if state.overflow_open {
                ToolbarOutcome::OverflowOpened
            } else {
                ToolbarOutcome::OverflowClosed
            };
        }
        if let Some(item) = self.items.iter().find(|i| i.id == active) {
            if !item.is_interactive() {
                return ToolbarOutcome::Ignored;
            }
            return match item.kind {
                ToolbarItemKind::Toggle { pressed } => ToolbarOutcome::Toggled {
                    id: active,
                    pressed: !pressed,
                },
                ToolbarItemKind::Action => ToolbarOutcome::Activated(active),
                _ => ToolbarOutcome::Ignored,
            };
        }
        ToolbarOutcome::Ignored
    }
}

/// Visibility plan for one paint.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct ToolbarPlan {
    /// Item indices kept on the strip (paint order).
    pub visible: Vec<usize>,
    /// Item indices moved to overflow.
    pub overflow: Vec<usize>,
    /// Whether overflow chip should show.
    pub has_overflow: bool,
}

fn plan_items<Id>(
    items: &[ToolbarItem<'_, Id>],
    area: Rect,
    orientation: ToolbarOrientation,
    variant: ToolbarVariant,
    overflow_enabled: bool,
    glyphs: GlyphSet,
) -> ToolbarPlan {
    if area.is_empty() || items.is_empty() {
        return ToolbarPlan::default();
    }
    let budget = match orientation {
        ToolbarOrientation::Horizontal => area.width,
        ToolbarOrientation::Vertical => area.height,
    };
    let gap: u16 = match variant {
        ToolbarVariant::Default => 1,
        ToolbarVariant::Compact => 0,
    };
    let overflow_w = overflow_chip_width(glyphs, variant);

    // Measure all
    let widths: Vec<u16> = items
        .iter()
        .map(|i| item_main_size(i, orientation, variant))
        .collect();

    // Try keep all
    let total: u16 = widths
        .iter()
        .copied()
        .enumerate()
        .map(|(i, w)| {
            if i + 1 < items.len() {
                w.saturating_add(gap)
            } else {
                w
            }
        })
        .sum();
    if total <= budget {
        return ToolbarPlan {
            visible: (0..items.len()).collect(),
            overflow: vec![],
            has_overflow: false,
        };
    }

    if !overflow_enabled {
        // Clip from end
        let mut used = 0u16;
        let mut visible = Vec::new();
        for (i, &w) in widths.iter().enumerate() {
            let need = if visible.is_empty() {
                w
            } else {
                w.saturating_add(gap)
            };
            if used.saturating_add(need) > budget {
                break;
            }
            used = used.saturating_add(need);
            visible.push(i);
        }
        return ToolbarPlan {
            overflow: (visible.len()..items.len()).collect(),
            has_overflow: false,
            visible,
        };
    }

    // Drop lowest priority first into overflow until rest + chip fit.
    let mut in_overflow: Vec<bool> = vec![false; items.len()];
    loop {
        let mut used = 0u16;
        let mut vis = Vec::new();
        for (i, &w) in widths.iter().enumerate() {
            if in_overflow[i] {
                continue;
            }
            let need = if vis.is_empty() {
                w
            } else {
                w.saturating_add(gap)
            };
            used = used.saturating_add(need);
            vis.push(i);
        }
        let overflow_count = in_overflow.iter().filter(|x| **x).count();
        let need_chip = overflow_count > 0;
        let chip = if need_chip {
            if vis.is_empty() {
                overflow_w
            } else {
                overflow_w.saturating_add(gap)
            }
        } else {
            0
        };
        if used.saturating_add(chip) <= budget || vis.is_empty() {
            return ToolbarPlan {
                visible: vis,
                overflow: in_overflow
                    .iter()
                    .enumerate()
                    .filter_map(|(i, o)| o.then_some(i))
                    .collect(),
                has_overflow: need_chip,
            };
        }
        // Drop lowest priority among remaining non-separators first; separators can drop too.
        let mut candidates: Vec<usize> = vis.clone();
        candidates.sort_by_key(|&i| (items[i].priority, i as u8));
        let Some(drop_i) = candidates.first().copied() else {
            break;
        };
        in_overflow[drop_i] = true;
        // safety: prevent infinite loop
        if in_overflow.iter().all(|x| *x) {
            break;
        }
    }
    ToolbarPlan {
        visible: vec![],
        overflow: (0..items.len()).collect(),
        has_overflow: overflow_enabled,
    }
}

fn overflow_chip_width(glyphs: GlyphSet, variant: ToolbarVariant) -> u16 {
    let g = glyphs.ellipsis();
    let pad = match variant {
        ToolbarVariant::Default => 2,
        ToolbarVariant::Compact => 0,
    };
    (display_cols(g) as u16).saturating_add(pad)
}

fn item_main_size<Id>(
    item: &ToolbarItem<'_, Id>,
    orientation: ToolbarOrientation,
    variant: ToolbarVariant,
) -> u16 {
    if matches!(item.kind, ToolbarItemKind::Separator) {
        return 1;
    }
    if matches!(orientation, ToolbarOrientation::Vertical) {
        return 1;
    }
    let mut parts = Vec::new();
    if let Some(icon) = item.icon {
        parts.push(icon);
    }
    let show_label = !(matches!(variant, ToolbarVariant::Compact) && item.icon.is_some());
    if show_label && !item.label.is_empty() {
        parts.push(item.label);
    }
    if let Some(hint) = item.hint {
        if !matches!(variant, ToolbarVariant::Compact) {
            parts.push(hint);
        }
    }
    let pad = match variant {
        ToolbarVariant::Default => 2u16,
        ToolbarVariant::Compact => 0,
    };
    let inner = if parts.is_empty() {
        1
    } else {
        parts
            .iter()
            .map(|p| display_cols(p) as u16)
            .sum::<u16>()
            .saturating_add((parts.len().saturating_sub(1) as u16).min(2))
    };
    // Mark (+ following space) already lives in format_label; do not add
    // checkbox-bracket extras on top.
    let toggle_extra = match item.kind {
        ToolbarItemKind::Toggle { pressed } => {
            u16::try_from(display_cols(toolbar_toggle_mark(pressed)).saturating_add(1)).unwrap_or(0)
        }
        _ => 0,
    };
    inner
        .saturating_add(pad)
        .saturating_add(toggle_extra)
        .max(1)
}

/// Junie switch face copied from standalone Toggle paint. Not `[inner]` wells.
fn toolbar_toggle_mark(pressed: bool) -> &'static str {
    if pressed { "──●" } else { "○──" }
}

fn format_label<Id>(
    item: &ToolbarItem<'_, Id>,
    on_cursor: bool,
    surface_focused: bool,
    variant: ToolbarVariant,
) -> String {
    let mut s = String::new();
    if let Some(icon) = item.icon {
        s.push_str(icon);
        if !item.label.is_empty()
            && !(matches!(variant, ToolbarVariant::Compact) && item.icon.is_some())
        {
            s.push(' ');
        }
    }
    let show_label = !(matches!(variant, ToolbarVariant::Compact) && item.icon.is_some());
    if show_label {
        match item.kind {
            ToolbarItemKind::Toggle { pressed } => {
                s.push_str(toolbar_toggle_mark(pressed));
                s.push(' ');
                s.push_str(item.label);
            }
            _ => s.push_str(item.label),
        }
    }
    if let Some(hint) = item.hint {
        if !matches!(variant, ToolbarVariant::Compact) && !hint.is_empty() {
            s.push(' ');
            s.push('(');
            s.push_str(hint);
            s.push(')');
        }
    }
    let _ = (on_cursor, surface_focused);
    format!(" {s} ")
}

impl<Id: Clone + PartialEq> StatefulWidget for &Toolbar<'_, Id> {
    type State = ToolbarState<Id>;

    fn render(self, area: Rect, buffer: &mut Buffer, state: &mut Self::State) {
        state.regions.clear();
        state.overflow_indices.clear();
        if area.is_empty() {
            return;
        }
        // Orient roving without mem::take loss
        // RovingFocusGroup only allows orientation via builder - work around:
        // reconcile will still work; host may set orientation at construction.
        let plan = plan_items(
            self.items,
            area,
            self.orientation,
            self.variant,
            self.overflow_id.is_some(),
            self.system.glyphs,
        );
        state.overflow_indices = plan.overflow.clone();

        let gap: u16 = match self.variant {
            ToolbarVariant::Default => 1,
            ToolbarVariant::Compact => 0,
        };

        match self.orientation {
            ToolbarOrientation::Horizontal => {
                let mut x = area.x;
                for &i in &plan.visible {
                    if x >= area.right() {
                        break;
                    }
                    let item = &self.items[i];
                    paint_item(
                        self,
                        item,
                        Rect {
                            x,
                            y: area.y,
                            width: area.right().saturating_sub(x),
                            height: area.height.min(1),
                        },
                        buffer,
                        state,
                        true,
                    );
                    // re-measure used width from last region
                    if let Some(last) = state.regions.last() {
                        x = last.area.right().saturating_add(gap);
                    } else {
                        x = x.saturating_add(1);
                    }
                }
                if plan.has_overflow {
                    if let Some(oid) = self.overflow_id.as_ref() {
                        paint_overflow_chip(
                            self,
                            oid,
                            Rect {
                                x,
                                y: area.y,
                                width: area.right().saturating_sub(x),
                                height: 1.min(area.height),
                            },
                            buffer,
                            state,
                        );
                    }
                }
            }
            ToolbarOrientation::Vertical => {
                let mut y = area.y;
                for &i in &plan.visible {
                    if y >= area.bottom() {
                        break;
                    }
                    let item = &self.items[i];
                    paint_item(
                        self,
                        item,
                        Rect {
                            x: area.x,
                            y,
                            width: area.width,
                            height: 1,
                        },
                        buffer,
                        state,
                        false,
                    );
                    y = y.saturating_add(1);
                }
                if plan.has_overflow {
                    if let Some(oid) = self.overflow_id.as_ref() {
                        if y < area.bottom() {
                            paint_overflow_chip(
                                self,
                                oid,
                                Rect {
                                    x: area.x,
                                    y,
                                    width: area.width,
                                    height: 1,
                                },
                                buffer,
                                state,
                            );
                        }
                    }
                }
            }
        }

        // Reconcile roving after paint
        let entries = ToolbarState::roving_entries(
            self.items,
            &plan.visible,
            self.overflow_id.as_ref(),
            plan.has_overflow,
        );
        let _ = state.roving.reconcile(&entries);
    }
}

fn paint_item<Id: Clone + PartialEq>(
    bar: &Toolbar<'_, Id>,
    item: &ToolbarItem<'_, Id>,
    slot: Rect,
    buffer: &mut Buffer,
    state: &mut ToolbarState<Id>,
    horizontal: bool,
) {
    if slot.is_empty() {
        return;
    }
    if matches!(item.kind, ToolbarItemKind::Separator) {
        let glyph = if horizontal {
            bar.system.glyphs.rule_v()
        } else {
            bar.system.glyphs.rule()
        };
        let w = if horizontal {
            1.min(slot.width)
        } else {
            slot.width
        };
        let h = if horizontal {
            slot.height
        } else {
            1.min(slot.height)
        };
        let rect = Rect {
            x: slot.x,
            y: slot.y,
            width: w,
            height: h,
        };
        if horizontal {
            buffer.set_stringn(rect.x, rect.y, glyph, 1, bar.system.style(Role::Border));
        } else {
            let line: String = std::iter::repeat_n(glyph, usize::from(rect.width)).collect();
            buffer.set_stringn(
                rect.x,
                rect.y,
                &line,
                usize::from(rect.width),
                bar.system.style(Role::Border),
            );
        }
        return;
    }

    let on_cursor = state.roving.active() == Some(&item.id);
    let label = format_label(item, on_cursor, state.surface_focused, bar.variant);
    let need = (display_cols(&label) as u16).min(slot.width).max(1);
    let rect = Rect {
        x: slot.x,
        y: slot.y,
        width: need,
        height: 1.min(slot.height),
    };
    let style = if matches!(item.kind, ToolbarItemKind::Label) {
        let contract = bar.system.family_recipe(RecipeFamily::Action);
        bar.system.style(contract.secondary)
    } else {
        let control_state = if !item.enabled {
            ControlState::Disabled
        } else if on_cursor && state.surface_focused {
            ControlState::Focused
        } else {
            ControlState::Default
        };
        let recipe = bar.system.button_recipe(
            ButtonRecipeVariant::Quiet,
            control_state,
            bar.system.junie_theme().surface,
        );
        let mut style = recipe.fill.patch(recipe.label);
        if matches!(item.kind, ToolbarItemKind::Toggle { pressed: true }) {
            style = style.add_modifier(ratatui_core::style::Modifier::BOLD);
        }
        style
    };
    let clipped = take_display_cols(&label, usize::from(rect.width));
    buffer.set_stringn(rect.x, rect.y, &clipped, usize::from(rect.width), style);
    if item.is_interactive() {
        state.regions.push(HitRegion {
            id: item.id.clone(),
            area: rect,
        });
    }
}

fn paint_overflow_chip<Id: Clone + PartialEq>(
    bar: &Toolbar<'_, Id>,
    id: &Id,
    slot: Rect,
    buffer: &mut Buffer,
    state: &mut ToolbarState<Id>,
) {
    if slot.is_empty() {
        return;
    }
    let g = bar.system.glyphs.ellipsis();
    let on = state.roving.active() == Some(id);
    let label = format!(" {g} ");
    let need = (display_cols(&label) as u16).min(slot.width).max(1);
    let rect = Rect {
        x: slot.x,
        y: slot.y,
        width: need,
        height: 1.min(slot.height),
    };
    let recipe = bar.system.button_recipe(
        ButtonRecipeVariant::Quiet,
        if on && state.surface_focused {
            ControlState::Focused
        } else {
            ControlState::Default
        },
        bar.system.junie_theme().surface,
    );
    let mut style = recipe.fill.patch(recipe.label);
    style = style.add_modifier(ratatui_core::style::Modifier::BOLD);
    buffer.set_stringn(
        rect.x,
        rect.y,
        take_display_cols(&label, usize::from(rect.width)).as_ref(),
        usize::from(rect.width),
        style,
    );
    state.regions.push(HitRegion {
        id: id.clone(),
        area: rect,
    });
}

impl<Id: Clone + PartialEq> StatefulWidget for Toolbar<'_, Id> {
    type State = ToolbarState<Id>;

    fn render(self, area: Rect, buffer: &mut Buffer, state: &mut Self::State) {
        <&Self as StatefulWidget>::render(&self, area, buffer, state);
    }
}

// Note: Roving orientation: set at ToolbarState::new as Horizontal. For vertical
// toolbars, hosts should rebuild state or we set on first paint via a flag.
// Expose helper:

impl<Id> ToolbarState<Id> {
    /// Horizontal roving (default).
    #[must_use]
    pub fn horizontal() -> Self {
        Self {
            surface_focused: false,
            roving: RovingFocusGroup::new()
                .orientation(RovingOrientation::Horizontal)
                .wrap(true),
            regions: Vec::new(),
            overflow_open: false,
            overflow_indices: Vec::new(),
        }
    }

    /// Vertical roving for compact toolbars.
    #[must_use]
    pub fn vertical() -> Self {
        Self {
            surface_focused: false,
            roving: RovingFocusGroup::new()
                .orientation(RovingOrientation::Vertical)
                .wrap(true),
            regions: Vec::new(),
            overflow_open: false,
            overflow_indices: Vec::new(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::input::{KeyCode, KeyModifiers};
    use crate::style::DesignSystem;
    use crate::widgets::tests::click;

    fn sample_items() -> Vec<ToolbarItem<'static, &'static str>> {
        vec![
            ToolbarItem::action("save", "Save")
                .hint("C-s")
                .command("file.save")
                .priority(90),
            ToolbarItem::action("open", "Open").priority(40),
            ToolbarItem::separator("sep1"),
            ToolbarItem::toggle("wrap", "Wrap", false).priority(30),
            ToolbarItem::action("find", "Find").priority(20),
            ToolbarItem::action("help", "Help").priority(10),
        ]
    }

    #[test]
    fn plan_keeps_all_when_wide() {
        let system = DesignSystem::default();
        let items = sample_items();
        let tb = Toolbar::new(&items, &system).overflow_id("more");
        let plan = tb.plan(Rect::new(0, 0, 120, 1));
        assert_eq!(plan.visible.len(), items.len());
        assert!(!plan.has_overflow);
    }

    #[test]
    fn plan_overflows_low_priority() {
        let system = DesignSystem::default();
        let items = sample_items();
        let tb = Toolbar::new(&items, &system).overflow_id("more");
        let plan = tb.plan(Rect::new(0, 0, 28, 1));
        assert!(plan.has_overflow || plan.visible.len() < items.len());
        // high priority save should stay if anything stays
        if !plan.visible.is_empty() {
            assert!(plan.visible.contains(&0), "save kept: {:?}", plan.visible);
        }
    }

    #[test]
    fn roving_activate_requires_surface_focus() {
        let system = DesignSystem::default();
        let items = [ToolbarItem::action("a", "A"), ToolbarItem::action("b", "B")];
        let tb = Toolbar::new(&items, &system);
        let mut state = ToolbarState::horizontal();
        state.set_cursor(Some("a"));
        let area = Rect::new(0, 0, 40, 1);
        let mut buf = Buffer::empty(area);
        StatefulWidget::render(&tb, area, &mut buf, &mut state);
        // no surface focus
        let out = tb.handle_key(
            &mut state,
            KeyEvent::new(KeyCode::Enter, KeyModifiers::NONE),
            area,
        );
        assert!(matches!(out, ToolbarOutcome::Ignored));
        state.set_surface_focused(true);
        let out = tb.handle_key(
            &mut state,
            KeyEvent::new(KeyCode::Enter, KeyModifiers::NONE),
            area,
        );
        assert!(matches!(out, ToolbarOutcome::Activated("a")));
    }

    #[test]
    fn arrows_move_cursor_when_focused() {
        let system = DesignSystem::default();
        let items = [ToolbarItem::action("a", "A"), ToolbarItem::action("b", "B")];
        let tb = Toolbar::new(&items, &system);
        let mut state = ToolbarState::horizontal();
        state.set_surface_focused(true);
        state.set_cursor(Some("a"));
        let area = Rect::new(0, 0, 40, 1);
        let mut buf = Buffer::empty(area);
        StatefulWidget::render(&tb, area, &mut buf, &mut state);
        let out = tb.handle_key(
            &mut state,
            KeyEvent::new(KeyCode::Right, KeyModifiers::NONE),
            area,
        );
        assert!(matches!(
            out,
            ToolbarOutcome::CursorMoved { to: Some("b"), .. }
        ));
    }

    #[test]
    fn mouse_activates_hit() {
        let system = DesignSystem::default();
        let items = [ToolbarItem::action("a", "A"), ToolbarItem::action("b", "B")];
        let tb = Toolbar::new(&items, &system);
        let mut state = ToolbarState::horizontal();
        let area = Rect::new(0, 0, 40, 1);
        let mut buf = Buffer::empty(area);
        StatefulWidget::render(&tb, area, &mut buf, &mut state);
        assert!(!state.regions.is_empty());
        let r = state.regions[0].area;
        let out = tb.handle_mouse(&mut state, click(r.x, r.y));
        assert!(matches!(out, ToolbarOutcome::Activated("a")));
    }

    #[test]
    fn toggle_outcome() {
        let system = DesignSystem::default();
        let items = [ToolbarItem::toggle("w", "Wrap", false)];
        let tb = Toolbar::new(&items, &system);
        let mut state = ToolbarState::horizontal();
        state.set_surface_focused(true);
        state.set_cursor(Some("w"));
        let area = Rect::new(0, 0, 20, 1);
        let mut buf = Buffer::empty(area);
        StatefulWidget::render(&tb, area, &mut buf, &mut state);
        let out = tb.handle_key(
            &mut state,
            KeyEvent::new(KeyCode::Enter, KeyModifiers::NONE),
            area,
        );
        assert!(matches!(
            out,
            ToolbarOutcome::Toggled {
                id: "w",
                pressed: true
            }
        ));
    }

    #[test]
    fn toggle_paints_switch_glyphs_not_checkbox_wells() {
        let system = DesignSystem::junie();
        let items = [
            ToolbarItem::toggle("on", "Wrap", true),
            ToolbarItem::toggle("off", "Line", false),
        ];
        let tb = Toolbar::new(&items, &system);
        let mut state = ToolbarState::horizontal();
        let area = Rect::new(0, 0, 48, 1);
        let mut buf = Buffer::empty(area);
        StatefulWidget::render(&tb, area, &mut buf, &mut state);
        let text: String = (0..area.width)
            .map(|x| buf[(x, 0)].symbol().to_string())
            .collect();
        assert!(
            !text.contains("[✓]") && !text.contains("[ ]"),
            "checkbox wells leaked: {text:?}"
        );
        assert!(text.contains("──●"), "pressed switch missing: {text:?}");
        assert!(text.contains("○──"), "idle switch missing: {text:?}");
    }

    #[test]
    fn vertical_paints() {
        let system = DesignSystem::default();
        let items = [ToolbarItem::action("a", "A"), ToolbarItem::action("b", "B")];
        let tb = Toolbar::new(&items, &system).vertical().compact();
        let mut state = ToolbarState::vertical();
        let area = Rect::new(0, 0, 8, 4);
        let mut buf = Buffer::empty(area);
        StatefulWidget::render(&tb, area, &mut buf, &mut state);
        assert_eq!(state.regions.len(), 2);
    }

    #[test]
    fn overflow_chip_activates() {
        let system = DesignSystem::default();
        let items = sample_items();
        let tb = Toolbar::new(&items, &system).overflow_id("more");
        let mut state = ToolbarState::horizontal();
        state.set_surface_focused(true);
        let area = Rect::new(0, 0, 22, 1);
        let mut buf = Buffer::empty(area);
        StatefulWidget::render(&tb, area, &mut buf, &mut state);
        if state.overflow_indices.is_empty() {
            // still ok if everything fit
            return;
        }
        assert!(state.regions.iter().any(|r| r.id == "more"));
        state.set_cursor(Some("more"));
        let out = tb.handle_key(
            &mut state,
            KeyEvent::new(KeyCode::Enter, KeyModifiers::NONE),
            area,
        );
        assert!(matches!(out, ToolbarOutcome::OverflowOpened));
        assert!(state.overflow_open);
        assert!(matches!(
            tb.handle_key(
                &mut state,
                KeyEvent::new(KeyCode::Esc, KeyModifiers::NONE),
                area,
            ),
            ToolbarOutcome::OverflowClosed
        ));
        assert!(!state.overflow_open);
    }

    #[test]
    fn layout_is_cheap() {
        let system = DesignSystem::default();
        let items = sample_items();
        let tb = Toolbar::new(&items, &system).overflow_id("more");
        let area = Rect::new(0, 0, 60, 1);
        for _ in 0..20_000 {
            let _ = tb.plan(area);
        }
    }

    #[test]
    fn content_selection_independent_of_cursor() {
        // Document contract: surface_focused false means keys ignored even if cursor set.
        let mut state = ToolbarState::horizontal();
        state.set_cursor(Some("a"));
        // content selection would live elsewhere — toolbar does not clear it.
        assert!(!state.surface_focused);
        assert_eq!(state.cursor(), Some(&"a"));
    }

    #[test]
    fn empty_toolbar_is_safe_and_publishes_no_targets() {
        let system = DesignSystem::default();
        let items: [ToolbarItem<'_, &str>; 0] = [];
        let mut state = ToolbarState::<&str>::horizontal();
        let area = Rect::new(0, 0, 1, 1);
        let mut buffer = Buffer::empty(area);

        StatefulWidget::render(Toolbar::new(&items, &system), area, &mut buffer, &mut state);

        assert!(state.regions.is_empty());
        assert!(state.overflow_indices.is_empty());
    }
}
