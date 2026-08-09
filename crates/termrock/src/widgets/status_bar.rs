// SPDX-FileCopyrightText: 2026 Alexey Zhokhov
// SPDX-License-Identifier: Apache-2.0

//! StatusBar — low-noise status surface for mode, connection, selection,
//! context, shortcuts, and transient messages.
//!
//! **Regions:** left · center · right, each with priority-ordered slots.
//! **Recipes:** minimal · compact · rich filter which kinds paint.
//! **Semantics:** prefer glyph + role text over color-only meaning.
//! **Transient:** optional message occupies center overflow without removing
//! essential persistent slots.
//!
//! Behavioral references: Zellij mode bar, Vim/Helix status lines, btop footers.

use ratatui_core::{
    buffer::Buffer,
    layout::{Position, Rect},
    style::Style,
    widgets::StatefulWidget,
};

use crate::{
    interaction::{HitRegion, Outcome},
    style::{DesignSystem, GlyphSet, Role, RolePalette, faded},
    text::{display_cols, take_display_cols},
};

/// Which band a slot belongs to.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
#[non_exhaustive]
pub enum StatusRegion {
    /// Leading cluster (mode, primary context).
    #[default]
    Left,
    /// Center band (path, focus zone, transient).
    Center,
    /// Trailing cluster (selection, connection, clock, hints).
    Right,
}

impl StatusRegion {
    /// Stable id.
    #[must_use]
    pub const fn id(self) -> &'static str {
        match self {
            Self::Left => "left",
            Self::Center => "center",
            Self::Right => "right",
        }
    }
}

/// Semantic meaning of a slot (drives default glyph / role).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
#[non_exhaustive]
pub enum StatusKind {
    /// Generic text.
    #[default]
    Text,
    /// Editor / app mode (NORMAL, INSERT, …).
    Mode,
    /// Connection / sync state.
    Connection,
    /// Selection summary.
    Selection,
    /// Path / workspace context.
    Context,
    /// Key hint / shortcut strip fragment.
    Shortcut,
    /// Active focus zone label (which pane owns keys).
    FocusZone,
    /// Transient message (ephemeral; does not drop essentials).
    Transient,
}

impl StatusKind {
    /// Stable id.
    #[must_use]
    pub const fn id(self) -> &'static str {
        match self {
            Self::Text => "text",
            Self::Mode => "mode",
            Self::Connection => "connection",
            Self::Selection => "selection",
            Self::Context => "context",
            Self::Shortcut => "shortcut",
            Self::FocusZone => "focus-zone",
            Self::Transient => "transient",
        }
    }

    /// Default non-color glyph; empty means none. Resolves via semantic [`crate::style::Glyph`].
    #[must_use]
    pub const fn default_glyph(self, glyphs: GlyphSet) -> &'static str {
        use crate::style::Glyph;
        let g = match self {
            Self::Mode => Some(Glyph::ModeDot),
            Self::Connection => Some(Glyph::Connection),
            Self::Selection => Some(Glyph::SelectionMark),
            Self::Context => Some(Glyph::DisclosureClosed),
            Self::Shortcut | Self::Text => None,
            Self::FocusZone => Some(Glyph::FocusDiamond),
            Self::Transient => Some(Glyph::Ellipsis),
        };
        match g {
            Some(glyph) => glyphs.resolve(glyph).text,
            None => "",
        }
    }

    /// Default role when slot style is unset / default.
    #[must_use]
    pub const fn default_role(self) -> Role {
        match self {
            Self::Mode => Role::Accent,
            Self::Connection => Role::Success,
            Self::Selection => Role::Info,
            Self::Context => Role::TextMuted,
            Self::Shortcut => Role::HintKey,
            Self::FocusZone => Role::TextStrong,
            Self::Transient => Role::Warning,
            Self::Text => Role::StatusBar,
        }
    }

    /// Whether this kind is shown under a recipe.
    #[must_use]
    pub const fn allowed_in(self, recipe: StatusBarRecipe) -> bool {
        match recipe {
            StatusBarRecipe::Minimal => matches!(
                self,
                Self::Mode | Self::Connection | Self::FocusZone | Self::Transient | Self::Text
            ),
            StatusBarRecipe::Compact => !matches!(self, Self::Shortcut),
            StatusBarRecipe::Rich => true,
        }
    }
}

/// Density / richness recipe for the bar.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
#[non_exhaustive]
pub enum StatusBarRecipe {
    /// Mode + connection + critical only.
    Minimal,
    /// Default: drop shortcut fragments under pressure first.
    #[default]
    Compact,
    /// Full meta including key hints.
    Rich,
}

impl StatusBarRecipe {
    /// Stable id.
    #[must_use]
    pub const fn id(self) -> &'static str {
        match self {
            Self::Minimal => "minimal",
            Self::Compact => "compact",
            Self::Rich => "rich",
        }
    }
}

/// A prioritized status-bar segment.
#[derive(Debug, Clone)]
pub struct StatusSlot<'a, Id> {
    /// Stable identity used for selection and activation.
    pub id: Id,
    /// Caller-visible content (text after optional glyph).
    pub content: &'a str,
    /// Higher-priority slots receive width before lower-priority slots.
    pub priority: u8,
    /// Minimum display columns required to keep the slot. Zero means all-or-nothing (full width).
    pub min_width: u16,
    /// Whether this item is enabled.
    pub enabled: bool,
    /// Explicit paint style (overrides kind role when non-default / always used if set via builder).
    pub style: Style,
    /// Optional style override while the slot is hovered.
    pub hover_style: Option<Style>,
    /// Region band (default Left for backward-compat constructors).
    pub region: StatusRegion,
    /// Semantic kind.
    pub kind: StatusKind,
    /// Optional non-color glyph (defaults from kind when None and kind has glyph).
    pub glyph: Option<&'a str>,
    /// When true, use `style` as-is; when false, merge kind role if style is empty-ish.
    pub style_explicit: bool,
}

impl<'a, Id> StatusSlot<'a, Id> {
    /// Text slot on the left with priority.
    #[must_use]
    pub fn new(id: Id, content: &'a str) -> Self {
        Self {
            id,
            content,
            priority: 50,
            min_width: 0,
            enabled: true,
            style: Style::new(),
            hover_style: None,
            region: StatusRegion::Left,
            kind: StatusKind::Text,
            glyph: None,
            style_explicit: false,
        }
    }

    /// Mode indicator.
    #[must_use]
    pub fn mode(id: Id, content: &'a str) -> Self {
        Self::new(id, content)
            .kind(StatusKind::Mode)
            .priority(100)
            .region(StatusRegion::Left)
            .min_width(3)
    }

    /// Connection status.
    #[must_use]
    pub fn connection(id: Id, content: &'a str) -> Self {
        Self::new(id, content)
            .kind(StatusKind::Connection)
            .priority(90)
            .region(StatusRegion::Right)
            .min_width(3)
    }

    /// Selection summary.
    #[must_use]
    pub fn selection(id: Id, content: &'a str) -> Self {
        Self::new(id, content)
            .kind(StatusKind::Selection)
            .priority(70)
            .region(StatusRegion::Right)
    }

    /// Context / path.
    #[must_use]
    pub fn context(id: Id, content: &'a str) -> Self {
        Self::new(id, content)
            .kind(StatusKind::Context)
            .priority(60)
            .region(StatusRegion::Center)
            .min_width(8)
    }

    /// Key hint fragment.
    #[must_use]
    pub fn shortcut(id: Id, content: &'a str) -> Self {
        Self::new(id, content)
            .kind(StatusKind::Shortcut)
            .priority(30)
            .region(StatusRegion::Right)
    }

    /// Active focus zone.
    #[must_use]
    pub fn focus_zone(id: Id, content: &'a str) -> Self {
        Self::new(id, content)
            .kind(StatusKind::FocusZone)
            .priority(85)
            .region(StatusRegion::Center)
            .min_width(4)
    }

    /// Region.
    #[must_use]
    pub const fn region(mut self, region: StatusRegion) -> Self {
        self.region = region;
        self
    }

    /// Kind.
    #[must_use]
    pub const fn kind(mut self, kind: StatusKind) -> Self {
        self.kind = kind;
        self
    }

    /// Priority.
    #[must_use]
    pub const fn priority(mut self, priority: u8) -> Self {
        self.priority = priority;
        self
    }

    /// Min width.
    #[must_use]
    pub const fn min_width(mut self, min_width: u16) -> Self {
        self.min_width = min_width;
        self
    }

    /// Enabled.
    #[must_use]
    pub const fn enabled(mut self, enabled: bool) -> Self {
        self.enabled = enabled;
        self
    }

    /// Explicit style.
    #[must_use]
    pub const fn style(mut self, style: Style) -> Self {
        self.style = style;
        self.style_explicit = true;
        self
    }

    /// Hover style.
    #[must_use]
    pub const fn hover_style(mut self, style: Style) -> Self {
        self.hover_style = Some(style);
        self
    }

    /// Glyph override.
    #[must_use]
    pub const fn glyph(mut self, glyph: &'a str) -> Self {
        self.glyph = Some(glyph);
        self
    }
}

/// Ephemeral center message (host owns lifetime / clock).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct TransientStatus<'a> {
    /// Message text.
    pub text: &'a str,
    /// Optional glyph.
    pub glyph: Option<&'a str>,
}

impl<'a> TransientStatus<'a> {
    /// Text-only transient.
    #[must_use]
    pub const fn new(text: &'a str) -> Self {
        Self { text, glyph: None }
    }

    /// With glyph.
    #[must_use]
    pub const fn glyph(mut self, glyph: &'a str) -> Self {
        self.glyph = Some(glyph);
        self
    }
}

/// Runtime state for `StatusBar`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StatusBarState<Id> {
    /// Whether this item is hovered.
    pub hovered: Option<Id>,
    /// Hit regions produced by the most recent render.
    pub regions: Vec<HitRegion<Id>>,
    /// Optional transient message (not a slot id — painted via center band).
    pub transient: Option<String>,
}

impl<Id> Default for StatusBarState<Id> {
    fn default() -> Self {
        Self {
            hovered: None,
            regions: Vec::new(),
            transient: None,
        }
    }
}

impl<Id: Clone> StatusBarState<Id> {
    /// Empty state.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Sets or clears transient text (host-owned lifetime).
    pub fn set_transient(&mut self, message: Option<impl Into<String>>) {
        self.transient = message.map(Into::into);
    }

    /// Updates hover state from the current pointer position and painted hit regions.
    pub fn hover(&mut self, position: Position) -> Option<&Id> {
        self.hovered = self
            .regions
            .iter()
            .find(|region| region.area.contains(position))
            .map(|region| region.id.clone());
        self.hovered.as_ref()
    }

    /// Maps a pointer position to the semantic outcome of the painted hit region.
    #[must_use]
    pub fn click(&mut self, position: Position) -> Outcome<Id> {
        self.regions
            .iter()
            .find(|region| region.area.contains(position))
            .map_or(Outcome::Ignored, |region| {
                Outcome::Activated(region.id.clone())
            })
    }
}

/// A one-row collection of prioritized status slots.
#[derive(Debug, Clone, Copy)]
pub struct StatusBar<'a, Id> {
    left: &'a [StatusSlot<'a, Id>],
    center: &'a [StatusSlot<'a, Id>],
    right: &'a [StatusSlot<'a, Id>],
    system: &'a DesignSystem,
    alpha: f32,
    recipe: StatusBarRecipe,
    /// Borrowed transient for this frame (preferred over state when set).
    transient: Option<&'a TransientStatus<'a>>,
}

impl<'a, Id> StatusBar<'a, Id> {
    /// Creates a status bar over left + right slots (center empty).
    ///
    /// Slots may also set [`StatusSlot::region`]; region on the slot wins when
    /// using [`Self::from_slots`].
    #[must_use]
    pub const fn new(
        left: &'a [StatusSlot<'a, Id>],
        right: &'a [StatusSlot<'a, Id>],
        system: &'a DesignSystem,
    ) -> Self {
        Self {
            left,
            center: &[],
            right,
            system,
            alpha: 1.0,
            recipe: StatusBarRecipe::Compact,
            transient: None,
        }
    }

    /// Three-region constructor (left · center · right).
    #[must_use]
    pub const fn with_center(
        left: &'a [StatusSlot<'a, Id>],
        center: &'a [StatusSlot<'a, Id>],
        right: &'a [StatusSlot<'a, Id>],
        system: &'a DesignSystem,
    ) -> Self {
        Self {
            left,
            center,
            right,
            system,
            alpha: 1.0,
            recipe: StatusBarRecipe::Compact,
            transient: None,
        }
    }

    /// Backdrop opacity.
    #[must_use]
    pub const fn alpha(mut self, alpha: f32) -> Self {
        self.alpha = alpha;
        self
    }

    /// Recipe filter.
    #[must_use]
    pub const fn recipe(mut self, recipe: StatusBarRecipe) -> Self {
        self.recipe = recipe;
        self
    }

    /// Minimal recipe.
    #[must_use]
    pub const fn minimal(mut self) -> Self {
        self.recipe = StatusBarRecipe::Minimal;
        self
    }

    /// Rich recipe.
    #[must_use]
    pub const fn rich(mut self) -> Self {
        self.recipe = StatusBarRecipe::Rich;
        self
    }

    /// Compact recipe (default).
    #[must_use]
    pub const fn compact(mut self) -> Self {
        self.recipe = StatusBarRecipe::Compact;
        self
    }

    /// Frame-local transient (takes precedence over state string when painting label).
    #[must_use]
    pub const fn transient(mut self, message: &'a TransientStatus<'a>) -> Self {
        self.transient = Some(message);
        self
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Side {
    Left,
    Center,
    Right,
}

#[derive(Debug, Clone)]
struct Allocation<Id> {
    id: Id,
    side: Side,
    index: usize,
    width: u16,
    full_width: u16,
    priority: u8,
    essential: bool,
}

#[derive(Debug, Clone)]
struct Placement<Id> {
    id: Id,
    side: Side,
    index: usize,
    area: Rect,
    is_transient: bool,
}

impl<Id: Clone> StatusBar<'_, Id> {
    /// Hit regions for the most recent geometry (without painting).
    #[must_use]
    pub fn regions(&self, area: Rect) -> Vec<HitRegion<Id>> {
        self.placements(area, None)
            .into_iter()
            .filter(|p| !p.is_transient)
            .map(|placement| HitRegion {
                id: placement.id,
                area: placement.area,
            })
            .collect()
    }

    fn slot_ref(&self, side: Side, index: usize) -> &StatusSlot<'_, Id> {
        match side {
            Side::Left => &self.left[index],
            Side::Center => &self.center[index],
            Side::Right => &self.right[index],
        }
    }

    fn placements(
        &self,
        area: Rect,
        state: Option<&StatusBarState<Id>>,
    ) -> Vec<Placement<Id>> {
        if area.is_empty() {
            return Vec::new();
        }

        let mut candidates = Vec::new();
        for (index, slot) in self.left.iter().enumerate() {
            if let Some(a) = allocation(slot, Side::Left, index, self.recipe, self.system) {
                candidates.push(a);
            }
        }
        for (index, slot) in self.center.iter().enumerate() {
            if let Some(a) = allocation(slot, Side::Center, index, self.recipe, self.system) {
                candidates.push(a);
            }
        }
        for (index, slot) in self.right.iter().enumerate() {
            if let Some(a) = allocation(slot, Side::Right, index, self.recipe, self.system) {
                candidates.push(a);
            }
        }

        // Reserve center budget for transient without dropping essentials.
        let has_transient = self
            .transient
            .map(|t| !t.text.is_empty())
            .or_else(|| state.map(|s| s.transient.as_ref().is_some_and(|t| !t.is_empty())))
            .unwrap_or(false);
        let transient_reserve: u16 = if has_transient {
            let text = self
                .transient
                .map(|t| t.text)
                .or_else(|| state.and_then(|s| s.transient.as_deref()))
                .unwrap_or("");
            let g = self
                .transient
                .and_then(|t| t.glyph)
                .unwrap_or_else(|| StatusKind::Transient.default_glyph(self.system.glyphs));
            let w = (display_cols(text) as u16)
                .saturating_add(if g.is_empty() {
                    0
                } else {
                    (display_cols(g) as u16).saturating_add(1)
                })
                .saturating_add(2);
            w.min(area.width / 2).max(4.min(area.width))
        } else {
            0
        };

        candidates.sort_by(|left, right| {
            right
                .priority
                .cmp(&left.priority)
                .then_with(|| side_rank(left.side).cmp(&side_rank(right.side)))
                .then_with(|| left.index.cmp(&right.index))
        });

        let mut remaining = area.width.saturating_sub(transient_reserve);

        let mut included = Vec::new();
        for mut candidate in candidates {
            let minimum = if candidate.width == 0 {
                candidate.full_width
            } else {
                candidate.width
            };
            if minimum == 0 {
                continue;
            }
            if minimum > remaining {
                // Essentials may still take remaining if anything left
                if candidate.essential && remaining > 0 {
                    candidate.width = remaining.min(candidate.full_width).max(1);
                    remaining = 0;
                    included.push(candidate);
                }
                continue;
            }
            candidate.width = minimum;
            remaining = remaining.saturating_sub(minimum);
            included.push(candidate);
        }
        // Grow toward full width
        for allocation in &mut included {
            let growth = allocation
                .full_width
                .saturating_sub(allocation.width)
                .min(remaining);
            allocation.width = allocation.width.saturating_add(growth);
            remaining = remaining.saturating_sub(growth);
        }

        // Place left LTR, right RTL, center in leftover middle.
        let mut placements = Vec::with_capacity(included.len() + 1);
        let mut left_x = area.x;
        let mut left = included
            .iter()
            .filter(|a| a.side == Side::Left)
            .collect::<Vec<_>>();
        left.sort_by_key(|a| a.index);
        for allocation in left {
            let width = allocation.width.min(area.right().saturating_sub(left_x));
            if width == 0 {
                continue;
            }
            placements.push(Placement {
                id: allocation.id.clone(),
                side: Side::Left,
                index: allocation.index,
                area: Rect::new(left_x, area.y, width, 1),
                is_transient: false,
            });
            left_x = left_x.saturating_add(width);
        }

        let mut right_x = area.right();
        let mut right = included
            .iter()
            .filter(|a| a.side == Side::Right)
            .collect::<Vec<_>>();
        right.sort_by_key(|a| std::cmp::Reverse(a.index));
        for allocation in right {
            let start = right_x.saturating_sub(allocation.width).max(left_x);
            if start >= right_x {
                continue;
            }
            placements.push(Placement {
                id: allocation.id.clone(),
                side: Side::Right,
                index: allocation.index,
                area: Rect::new(start, area.y, right_x.saturating_sub(start), 1),
                is_transient: false,
            });
            right_x = start;
        }

        // Center band between left_x and right_x (persistent center slots only).
        let center_start = left_x;
        let center_end = right_x;
        if center_end > center_start {
            let mut cx = center_start;
            let mut center = included
                .iter()
                .filter(|a| a.side == Side::Center)
                .collect::<Vec<_>>();
            center.sort_by_key(|a| a.index);
            for allocation in center {
                let avail = center_end.saturating_sub(cx);
                let width = allocation.width.min(avail);
                if width == 0 {
                    continue;
                }
                placements.push(Placement {
                    id: allocation.id.clone(),
                    side: Side::Center,
                    index: allocation.index,
                    area: Rect::new(cx, area.y, width, 1),
                    is_transient: false,
                });
                cx = cx.saturating_add(width);
            }
        }
        // Transient is painted separately in paint_transient (no hit region / no id).
        let _ = has_transient;

        placements
    }

    fn paint_transient(
        &self,
        area: Rect,
        buffer: &mut Buffer,
        placements: &[Placement<Id>],
        state: Option<&StatusBarState<Id>>,
    ) {
        let text = self
            .transient
            .map(|t| t.text)
            .or_else(|| state.and_then(|s| s.transient.as_deref()));
        let Some(text) = text.filter(|t| !t.is_empty()) else {
            return;
        };
        let glyph = self
            .transient
            .and_then(|t| t.glyph)
            .unwrap_or_else(|| StatusKind::Transient.default_glyph(self.system.glyphs));

        // Free span: between max left edge and min right edge
        let left_edge = placements
            .iter()
            .filter(|p| p.side == Side::Left)
            .map(|p| p.area.right())
            .max()
            .unwrap_or(area.x);
        let right_edge = placements
            .iter()
            .filter(|p| p.side == Side::Right)
            .map(|p| p.area.x)
            .min()
            .unwrap_or(area.right());
        // Also avoid permanent center slots
        let center_used_right = placements
            .iter()
            .filter(|p| p.side == Side::Center)
            .map(|p| p.area.right())
            .max()
            .unwrap_or(left_edge);
        let start = left_edge.max(center_used_right);
        if start >= right_edge {
            return;
        }
        let avail = right_edge.saturating_sub(start);
        let mut label = String::new();
        if !glyph.is_empty() {
            label.push_str(glyph);
            label.push(' ');
        }
        label.push_str(text);
        let need = (display_cols(&label) as u16 + 2).min(avail);
        if need == 0 {
            return;
        }
        let pad = avail.saturating_sub(need) / 2;
        let x = start.saturating_add(pad);
        let shown = take_display_cols(&label, usize::from(need));
        buffer.set_stringn(
            x,
            area.y,
            &shown,
            usize::from(need),
            fade_style(self.system.style(Role::Warning), self.alpha),
        );
    }
}

impl<Id: Clone + PartialEq> StatefulWidget for &StatusBar<'_, Id> {
    type State = StatusBarState<Id>;

    fn render(self, area: Rect, buffer: &mut Buffer, state: &mut Self::State) {
        if area.is_empty() {
            state.regions.clear();
            return;
        }
        buffer.set_style(
            area,
            fade_style(self.system.style(Role::StatusBar), self.alpha),
        );
        state.regions.clear();
        let placements = self.placements(area, Some(state));
        let mut content = String::new();
        for placement in &placements {
            if placement.is_transient {
                continue;
            }
            let slot = self.slot_ref(placement.side, placement.index);
            let hovered = state.hovered.as_ref() == Some(&slot.id);
            let style = resolve_style(slot, hovered, self.system);
            let painted = format_slot_content(slot, self.system.glyphs);
            crate::text::display_cols_slice_into(
                &painted,
                0,
                usize::from(placement.area.width),
                &mut content,
            );
            buffer.set_stringn(
                placement.area.x,
                placement.area.y,
                &content,
                usize::from(placement.area.width),
                fade_style(style, self.alpha),
            );
            state.regions.push(HitRegion {
                id: placement.id.clone(),
                area: placement.area,
            });
        }
        self.paint_transient(area, buffer, &placements, Some(state));
    }
}

impl<Id: Clone + PartialEq> StatefulWidget for StatusBar<'_, Id> {
    type State = StatusBarState<Id>;

    fn render(self, area: Rect, buffer: &mut Buffer, state: &mut Self::State) {
        <&Self as StatefulWidget>::render(&self, area, buffer, state);
    }
}

fn format_slot_content<Id>(slot: &StatusSlot<'_, Id>, glyphs: GlyphSet) -> String {
    let g = slot
        .glyph
        .unwrap_or_else(|| slot.kind.default_glyph(glyphs));
    if g.is_empty() {
        slot.content.to_string()
    } else {
        format!("{g} {}", slot.content)
    }
}

fn resolve_style<Id>(slot: &StatusSlot<'_, Id>, hovered: bool, system: &DesignSystem) -> Style {
    if hovered {
        if let Some(h) = slot.hover_style {
            return h;
        }
    }
    if slot.style_explicit {
        return slot.style;
    }
    // Prefer kind role; allow slot.style to add modifiers if any set
    let mut base = system.style(slot.kind.default_role());
    if slot.style.fg.is_some() {
        base.fg = slot.style.fg;
    }
    if slot.style.bg.is_some() {
        base.bg = slot.style.bg;
    }
    if slot.style.add_modifier != ratatui_core::style::Modifier::empty() {
        base = base.add_modifier(slot.style.add_modifier);
    }
    base
}

fn allocation<Id: Clone>(
    slot: &StatusSlot<'_, Id>,
    side: Side,
    index: usize,
    recipe: StatusBarRecipe,
    system: &DesignSystem,
) -> Option<Allocation<Id>> {
    if !slot.enabled {
        return None;
    }
    // Recipe filter: kind gate, with legacy high-priority Text still visible in minimal.
    let kind_ok = slot.kind.allowed_in(recipe)
        || (matches!(recipe, StatusBarRecipe::Minimal)
            && matches!(slot.kind, StatusKind::Text)
            && slot.priority >= 80);
    if !kind_ok {
        return None;
    }
    let painted = format_slot_content(slot, system.glyphs);
    let full_width = u16::try_from(display_cols(&painted)).unwrap_or(u16::MAX);
    if full_width == 0 {
        return None;
    }
    let essential = slot.priority >= 80
        || matches!(
            slot.kind,
            StatusKind::Mode | StatusKind::Connection | StatusKind::FocusZone
        );
    Some(Allocation {
        id: slot.id.clone(),
        side,
        index,
        width: slot.min_width.min(full_width),
        full_width,
        priority: slot.priority,
        essential,
    })
}

const fn side_rank(side: Side) -> u8 {
    match side {
        Side::Left => 0,
        Side::Center => 1,
        Side::Right => 2,
    }
}

fn fade_style(mut style: Style, alpha: f32) -> Style {
    if let Some(foreground) = style.fg {
        style = style.fg(faded(foreground, alpha));
    }
    if let Some(background) = style.bg {
        style = style.bg(faded(background, alpha));
    }
    if let Some(underline) = style.underline_color {
        style = style.underline_color(faded(underline, alpha));
    }
    style
}

#[cfg(test)]
mod tests {
    use super::*;
    use ratatui_core::style::Color;

    fn legacy_slot(
        id: &'static str,
        content: &'static str,
        priority: u8,
        min_width: u16,
    ) -> StatusSlot<'static, &'static str> {
        StatusSlot {
            id,
            content,
            priority,
            min_width,
            enabled: true,
            style: Style::new().fg(Color::Rgb(100, 50, 20)),
            hover_style: Some(Style::new().fg(Color::Rgb(200, 100, 40))),
            region: StatusRegion::Left,
            kind: StatusKind::Text,
            glyph: None,
            style_explicit: true,
        }
    }

    #[test]
    fn priority_and_minimum_width_control_narrow_layout() {
        let theme = RolePalette::default();
        let system = DesignSystem::from_palette(theme.clone());
        let left = [legacy_slot("activity", " activity ", 10, 4)];
        let right = [
            legacy_slot("usage", " usage-long ", 1, 0),
            legacy_slot("run", " run ", 20, 0),
        ];
        // Fix region on right slots for legacy constructor
        let mut right = right;
        right[0].region = StatusRegion::Right;
        right[1].region = StatusRegion::Right;
        let bar = StatusBar::new(&left, &right, &system);
        let regions = bar.regions(Rect::new(3, 2, 10, 1));
        assert!(regions.iter().any(|region| region.id == "run"));
        assert!(regions.iter().any(|region| region.id == "activity"));
        assert!(!regions.iter().any(|region| region.id == "usage"));
        assert!(regions.iter().all(|region| region.area.width > 0));
    }

    #[test]
    fn hover_and_activation_follow_only_painted_regions() {
        let left = [legacy_slot("activity", " activity ", 1, 4)];
        let theme = RolePalette::default()
            .with_role(Role::StatusBar, Style::new().bg(Color::Rgb(80, 80, 80)));
        let system = DesignSystem::from_palette(theme.clone());
        let bar = StatusBar::new(&left, &[], &system).alpha(0.5);
        let area = Rect::new(4, 3, 6, 1);
        let mut state = StatusBarState::default();
        let mut buffer = Buffer::empty(area);
        (&bar).render(area, &mut buffer, &mut state);
        assert_eq!(state.regions.len(), 1);
        let position = Position::new(area.x, area.y);
        assert_eq!(state.hover(position), Some(&"activity"));
        (&bar).render(area, &mut buffer, &mut state);
        assert_eq!(state.click(position), Outcome::Activated("activity"));
        assert_eq!(buffer[(area.x, area.y)].bg, Color::Rgb(40, 40, 40));
    }

    #[test]
    fn unicode_truncation_never_paints_half_a_wide_grapheme() {
        let left = [legacy_slot("wide", " 🧪🔬🧭 ", 1, 3)];
        let theme = RolePalette::default();
        let system = DesignSystem::from_palette(theme.clone());
        let bar = StatusBar::new(&left, &[], &system);
        let area = Rect::new(0, 0, 3, 1);
        let mut state = StatusBarState::default();
        let mut buffer = Buffer::empty(area);
        (&bar).render(area, &mut buffer, &mut state);
        assert_eq!(state.regions[0].area.width, 3);
        assert_ne!(buffer[(2, 0)].symbol(), "\0");
    }

    #[test]
    fn center_and_right_regions() {
        let system = DesignSystem::default();
        let left = [StatusSlot::mode("mode", "NOR")];
        let center = [StatusSlot::context("ctx", "src/main.rs")];
        let right = [StatusSlot::connection("conn", "ok")];
        let bar = StatusBar::with_center(&left, &center, &right, &system);
        let regions = bar.regions(Rect::new(0, 0, 60, 1));
        assert!(regions.iter().any(|r| r.id == "mode"));
        assert!(regions.iter().any(|r| r.id == "ctx"));
        assert!(regions.iter().any(|r| r.id == "conn"));
        // left before center before right
        let mode_x = regions.iter().find(|r| r.id == "mode").unwrap().area.x;
        let ctx_x = regions.iter().find(|r| r.id == "ctx").unwrap().area.x;
        let conn_x = regions.iter().find(|r| r.id == "conn").unwrap().area.x;
        assert!(mode_x <= ctx_x);
        assert!(ctx_x <= conn_x);
    }

    #[test]
    fn transient_does_not_drop_mode() {
        let system = DesignSystem::default();
        let left = [StatusSlot::mode("mode", "NOR")];
        let right = [StatusSlot::connection("c", "live")];
        let msg = TransientStatus::new("saved");
        let bar = StatusBar::new(&left, &right, &system).transient(&msg);
        let mut state = StatusBarState::default();
        let area = Rect::new(0, 0, 40, 1);
        let mut buf = Buffer::empty(area);
        (&bar).render(area, &mut buf, &mut state);
        assert!(state.regions.iter().any(|r| r.id == "mode"));
        // transient paints somewhere without a hit region for itself
        assert!(!state.regions.iter().any(|r| r.id == "saved"));
    }

    #[test]
    fn minimal_recipe_drops_shortcuts() {
        let system = DesignSystem::default();
        let left = [StatusSlot::mode("m", "INS")];
        let right = [
            StatusSlot::shortcut("h", "C-s save"),
            StatusSlot::connection("c", "ok"),
        ];
        let bar = StatusBar::new(&left, &right, &system).minimal();
        let regions = bar.regions(Rect::new(0, 0, 80, 1));
        assert!(regions.iter().any(|r| r.id == "m"));
        assert!(regions.iter().any(|r| r.id == "c"));
        assert!(!regions.iter().any(|r| r.id == "h"));
    }

    #[test]
    fn rich_keeps_shortcuts() {
        let system = DesignSystem::default();
        let left = [StatusSlot::mode("m", "NOR")];
        let right = [StatusSlot::shortcut("h", "?")];
        let bar = StatusBar::new(&left, &right, &system).rich();
        let regions = bar.regions(Rect::new(0, 0, 40, 1));
        assert!(regions.iter().any(|r| r.id == "h"));
    }

    #[test]
    fn state_transient_string() {
        let system = DesignSystem::default();
        let left = [StatusSlot::mode("m", "NOR")];
        let bar = StatusBar::new(&left, &[], &system);
        let mut state = StatusBarState::new();
        state.set_transient(Some("copied"));
        let area = Rect::new(0, 0, 30, 1);
        let mut buf = Buffer::empty(area);
        (&bar).render(area, &mut buf, &mut state);
        assert!(state.regions.iter().any(|r| r.id == "m"));
    }

    #[test]
    fn ascii_glyphs_for_mode() {
        let system = DesignSystem::default().glyphs(GlyphSet::Ascii);
        let left = [StatusSlot::mode("m", "NOR")];
        let bar = StatusBar::new(&left, &[], &system);
        let area = Rect::new(0, 0, 20, 1);
        let mut state = StatusBarState::default();
        let mut buf = Buffer::empty(area);
        (&bar).render(area, &mut buf, &mut state);
        assert_eq!(buf[(0, 0)].symbol(), "*");
    }

    #[test]
    fn empty_area_clears_regions() {
        let system = DesignSystem::default();
        let left = [StatusSlot::mode("m", "NOR")];
        let bar = StatusBar::new(&left, &[], &system);
        let mut state = StatusBarState::default();
        let mut buf = Buffer::empty(Rect::new(0, 0, 1, 1));
        (&bar).render(Rect::new(0, 0, 0, 0), &mut buf, &mut state);
        assert!(state.regions.is_empty());
    }

    #[test]
    fn layout_is_cheap() {
        let system = DesignSystem::default();
        let left = [StatusSlot::mode("m", "NOR"), StatusSlot::context("c", "path")];
        let center = [StatusSlot::focus_zone("f", "main")];
        let right = [
            StatusSlot::selection("s", "3 sel"),
            StatusSlot::connection("k", "ok"),
            StatusSlot::shortcut("h", "C-s"),
        ];
        let bar = StatusBar::with_center(&left, &center, &right, &system).rich();
        let area = Rect::new(0, 0, 100, 1);
        for _ in 0..20_000 {
            let _ = bar.regions(area);
        }
    }

    #[test]
    fn kind_ids_stable() {
        assert_eq!(StatusKind::FocusZone.id(), "focus-zone");
        assert_eq!(StatusBarRecipe::Minimal.id(), "minimal");
    }
}
