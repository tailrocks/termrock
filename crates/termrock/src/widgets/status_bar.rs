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
    style::{DesignSystem, GlyphSet, Role},
    text::{display_cols, take_display_cols},
};

use super::semantic_status::SemanticStatus;

/// Footer status sentence lifetime. junie showcase default: 4 seconds.
pub const STATUS_DEFAULT_TTL_MS: u64 = 4_000;

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
            Self::Connection => Some(Glyph::ModeDot),
            Self::Selection => Some(Glyph::SelectionMarker),
            Self::Context => Some(Glyph::DisclosureClosed),
            Self::Shortcut | Self::Text => None,
            Self::FocusZone => Some(Glyph::DiamondFilled),
            Self::Transient => None,
        };
        match g {
            Some(glyph) => glyphs.resolve(glyph).text,
            None => "",
        }
    }

    /// Default role when slot style is unset / default.
    #[must_use]
    const fn default_role(self) -> Role {
        match self {
            // Mode is context, not the operator's current intent: accent is
            // spent on the one live thing, never on a permanent band
            // (plans/007).
            Self::Mode => Role::TextStrong,
            Self::Connection => Role::TextStrong,
            Self::Selection => Role::TextMuted,
            Self::Context => Role::TextMuted,
            Self::Shortcut => Role::HintKey,
            Self::FocusZone => Role::TextStrong,
            Self::Transient => Role::TextSecondary,
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
    /// Region band (default Left for backward-compat constructors).
    pub region: StatusRegion,
    /// Semantic kind.
    pub kind: StatusKind,
    /// Optional non-color glyph (defaults from kind when None and kind has glyph).
    pub glyph: Option<&'a str>,
    /// Optional typed lifecycle status. This owns semantic glyph and tone;
    /// callers cannot bypass the recipe with a raw paint style.
    semantic: Option<SemanticStatus>,
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
            region: StatusRegion::Left,
            kind: StatusKind::Text,
            glyph: None,
            semantic: None,
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
            .semantic(SemanticStatus::Online)
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

    /// Glyph override.
    #[must_use]
    pub const fn glyph(mut self, glyph: &'a str) -> Self {
        self.glyph = Some(glyph);
        self
    }

    /// Typed status projection. Its glyph and semantic role are recipe-owned.
    #[must_use]
    pub const fn semantic(mut self, semantic: SemanticStatus) -> Self {
        self.semantic = Some(semantic);
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
    /// The mode label the bar is leaving, and when the change started.
    ///
    /// A mode chip that swaps instantly is the one place a status bar can
    /// startle: `NORMAL` becoming `INSERT` in one frame reads as a flash. The
    /// bar cross-fades instead (plans/014 Step 3b).
    previous_mode: Option<String>,
    mode_changed_at_ms: u64,
    transient_set_at_ms: Option<u64>,
    /// Lifetime of a timed status sentence. Default [`STATUS_DEFAULT_TTL_MS`].
    pub transient_ttl_ms: u64,
}

impl<Id> StatusBarState<Id> {
    /// Records a mode change so the next frames can cross-fade it.
    pub fn set_mode(&mut self, mode: impl Into<String>, elapsed_ms: u64) {
        let mode = mode.into();
        if self.previous_mode.as_deref() == Some(mode.as_str()) {
            return;
        }
        self.previous_mode = Some(mode);
        self.mode_changed_at_ms = elapsed_ms;
    }

    /// How far the mode cross-fade has run at `elapsed_ms` (`1.0` settled).
    #[must_use]
    pub fn mode_fade(&self, elapsed_ms: u64, duration_ms: u64) -> f32 {
        if self.previous_mode.is_none() || duration_ms == 0 {
            return 1.0;
        }
        let since = elapsed_ms.saturating_sub(self.mode_changed_at_ms);
        if since >= duration_ms {
            return 1.0;
        }
        since as f32 / duration_ms as f32
    }
}

impl<Id> Default for StatusBarState<Id> {
    fn default() -> Self {
        Self {
            hovered: None,
            regions: Vec::new(),
            transient: None,
            previous_mode: None,
            mode_changed_at_ms: 0,
            transient_set_at_ms: None,
            transient_ttl_ms: STATUS_DEFAULT_TTL_MS,
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
        self.transient_set_at_ms = None;
    }

    /// Sets a timed status sentence. Past tense, no period: `Cell saved`.
    pub fn set_transient_at(&mut self, message: Option<impl Into<String>>, elapsed_ms: u64) {
        self.transient = message.map(Into::into);
        self.transient_set_at_ms = Some(elapsed_ms);
    }

    /// Drops an expired status sentence. Default TTL is 4 seconds.
    pub fn expire_transient(&mut self, elapsed_ms: u64) {
        if let Some(at) = self.transient_set_at_ms {
            if elapsed_ms.saturating_sub(at) >= self.transient_ttl_ms {
                self.transient = None;
                self.transient_set_at_ms = None;
            }
        }
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
    separator: bool,
}

#[derive(Debug, Clone)]
struct Placement<Id> {
    id: Id,
    side: Side,
    index: usize,
    area: Rect,
    is_transient: bool,
    separator: bool,
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

    fn placements(&self, area: Rect, state: Option<&StatusBarState<Id>>) -> Vec<Placement<Id>> {
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

        // Reserve the ` · ` divider's columns for every same-side slot past
        // the first (in paint order) while budget remains, so the separator
        // never eats a slot's content. Paint carves exactly what was
        // reserved here; under scarcity no separator paints.
        for side in [Side::Left, Side::Center, Side::Right] {
            let mut order = included
                .iter()
                .enumerate()
                .filter(|(_, a)| a.side == side)
                .map(|(position, _)| position)
                .collect::<Vec<_>>();
            match side {
                Side::Right => order.sort_by_key(|&p| std::cmp::Reverse(included[p].index)),
                Side::Left | Side::Center => order.sort_by_key(|&p| included[p].index),
            }
            for &p in order.iter().skip(1) {
                if remaining < 3 {
                    break;
                }
                included[p].width = included[p].width.saturating_add(3);
                included[p].separator = true;
                remaining -= 3;
            }
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
                separator: allocation.separator,
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
                separator: allocation.separator,
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
                    separator: allocation.separator,
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
        let glyph = self.transient.and_then(|t| t.glyph).unwrap_or("");

        // junie: status wins the right edge, text-secondary, `right - w - 1`.
        let mut label = String::new();
        if !glyph.is_empty() {
            label.push_str(glyph);
            label.push(' ');
        }
        label.push_str(text);
        let w = display_cols(&label) as u16;
        if area.width <= w + 2 {
            return;
        }
        let occupied_right = placements
            .iter()
            .filter(|p| p.side == Side::Right)
            .map(|p| p.area.x)
            .min()
            .unwrap_or(area.right());
        if occupied_right <= area.x + w + 1 {
            return;
        }
        let x = area.right().saturating_sub(w).saturating_sub(1);
        let shown = take_display_cols(&label, usize::from(w));
        buffer.set_stringn(
            x,
            area.y,
            &shown,
            usize::from(w),
            apply_alpha(
                self.system,
                self.system.junie_theme().secondary(),
                self.alpha,
            ),
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
            apply_alpha(self.system, self.system.style(Role::StatusBar), self.alpha),
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
            let mut content_area = placement.area;
            if placement.separator && placement.area.width > 3 {
                // Symmetric ` · `: the separator sits between two facts, so it
                // gets the same breathing space on both sides. Its columns
                // were reserved during allocation, never taken from content.
                let separator = self.system.glyphs.meta_separator();
                buffer.set_stringn(
                    placement.area.x.saturating_add(1),
                    placement.area.y,
                    separator,
                    1,
                    apply_alpha(self.system, self.system.style(Role::TextMuted), self.alpha),
                );
                content_area.x = content_area.x.saturating_add(3);
                content_area.width = content_area.width.saturating_sub(3);
            }
            crate::text::display_cols_slice_into(
                &painted,
                0,
                usize::from(content_area.width),
                &mut content,
            );
            // Slot words read as one band; the slot's own glyph carries its
            // state (plans/007).
            let body = if hovered {
                self.system
                    .style(Role::Focus)
                    .add_modifier(ratatui_core::style::Modifier::BOLD)
            } else {
                self.system.style(Role::StatusBar)
            };
            buffer.set_stringn(
                content_area.x,
                content_area.y,
                &content,
                usize::from(content_area.width),
                apply_alpha(self.system, body, self.alpha),
            );
            let glyph = slot
                .semantic
                .map(|status| status.glyph())
                .or(slot.glyph)
                .unwrap_or_else(|| slot.kind.default_glyph(self.system.glyphs));
            if !glyph.is_empty() {
                crate::widgets::row_chrome::paint_status_glyph(
                    buffer,
                    content_area,
                    0,
                    glyph,
                    apply_alpha(self.system, style, self.alpha),
                );
            }
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
        .semantic
        .map(|status| status.glyph())
        .or(slot.glyph)
        .unwrap_or_else(|| slot.kind.default_glyph(glyphs));
    if g.is_empty() {
        slot.content.to_string()
    } else {
        format!("{g} {}", slot.content)
    }
}

fn resolve_style<Id>(slot: &StatusSlot<'_, Id>, hovered: bool, system: &DesignSystem) -> Style {
    if hovered {
        return system
            .style(Role::Focus)
            .add_modifier(ratatui_core::style::Modifier::BOLD);
    }
    system.style(
        slot.semantic
            .map_or_else(|| slot.kind.default_role(), SemanticStatus::role),
    )
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
        separator: false,
    })
}

const fn side_rank(side: Side) -> u8 {
    match side {
        Side::Left => 0,
        Side::Center => 1,
        Side::Right => 2,
    }
}

/// junie has no blend vocabulary: the status bar paints at full strength and
/// carries state with glyphs and words, not opacity.
fn apply_alpha(_system: &DesignSystem, style: Style, _alpha: f32) -> Style {
    style
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::style::RolePalette;

    fn slot(
        id: &'static str,
        content: &'static str,
        priority: u8,
        min_width: u16,
    ) -> StatusSlot<'static, &'static str> {
        StatusSlot::new(id, content)
            .priority(priority)
            .min_width(min_width)
    }

    #[test]
    fn status_bar_paints_band() {
        let system = DesignSystem::default();
        let left = [slot("mode", "NORMAL", 1, 4)];
        let bar = StatusBar::new(&left, &[], &system);
        let area = Rect::new(0, 0, 20, 1);
        let mut state = StatusBarState::default();
        let mut buffer = Buffer::empty(area);
        (&bar).render(area, &mut buffer, &mut state);
        assert_eq!(
            buffer[(19, 0)].bg,
            system.style(Role::StatusBar).bg.unwrap()
        );
    }

    #[test]
    fn priority_and_minimum_width_control_narrow_layout() {
        let theme = RolePalette::default();
        let system = DesignSystem::new(theme.clone());
        let left = [slot("activity", " activity ", 10, 4)];
        let right = [
            slot("usage", " usage-long ", 1, 0),
            slot("run", " run ", 20, 0),
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
    fn same_side_separator_never_eats_slot_content() {
        let system = DesignSystem::default();
        let right = [
            StatusSlot::new("container", " 2y0t4aw6 "),
            StatusSlot::new("run", " jk-run-c46709 "),
        ];
        let bar = StatusBar::new(&[] as &[StatusSlot<'_, &str>], &right, &system);
        let area = Rect::new(0, 0, 90, 1);
        let mut state = StatusBarState::new();
        let mut buffer = Buffer::empty(area);
        (&bar).render(area, &mut buffer, &mut state);
        let row = (0..area.width)
            .map(|x| buffer[(x, 0)].symbol())
            .collect::<String>();
        assert!(
            row.contains("2y0t4aw6"),
            "container slot truncated: {row:?}"
        );
        assert!(row.contains("jk-run-c46709"), "run slot truncated: {row:?}");
        assert!(row.contains('·'), "separator should paint: {row:?}");
    }

    #[test]
    fn unicode_truncation_never_paints_half_a_wide_grapheme() {
        let left = [slot("wide", " 🧪🔬🧭 ", 1, 3)];
        let theme = RolePalette::default();
        let system = DesignSystem::new(theme.clone());
        let bar = StatusBar::new(&left, &[], &system);
        let area = Rect::new(0, 0, 3, 1);
        let mut state = StatusBarState::default();
        let mut buffer = Buffer::empty(area);
        (&bar).render(area, &mut buffer, &mut state);
        assert_eq!(state.regions[0].area.width, 3);
        assert_ne!(buffer[(2, 0)].symbol(), "\0");
    }

    #[test]
    fn resize_cjk_combining_and_ascii_safe() {
        let left = [slot("unicode", " 東京 Cafe\u{301} ", 10, 1)];
        let system = DesignSystem::default();
        let bar = StatusBar::new(&left, &[], &system);
        for width in [32, 12, 1, 0] {
            let area = Rect::new(0, 0, width, 1);
            let mut state = StatusBarState::default();
            let mut buffer = Buffer::empty(area);
            (&bar).render(area, &mut buffer, &mut state);
            if width == 32 {
                let text: String = buffer.content().iter().map(|cell| cell.symbol()).collect();
                assert!(text.contains('東'), "{text:?}");
                assert!(text.contains("Cafe\u{301}"), "{text:?}");
            }
        }
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
    fn semantic_status_owns_glyph_over_custom_slot_glyph() {
        let system = DesignSystem::default();
        let left = [StatusSlot::new("failure", "failed")
            .glyph("Z")
            .semantic(SemanticStatus::Failed)];
        let bar = StatusBar::new(&left, &[], &system);
        let area = Rect::new(0, 0, 20, 1);
        let mut state = StatusBarState::default();
        let mut buffer = Buffer::empty(area);
        (&bar).render(area, &mut buffer, &mut state);
        assert_eq!(buffer[(0, 0)].symbol(), "\u{2717}");
    }

    #[test]
    fn status_slot_public_api_has_no_raw_style_escape_hatch() {
        let public = include_str!("status_bar.rs")
            .split("#[cfg(test)]")
            .next()
            .expect("public source");
        for forbidden in [
            "pub style:",
            "pub hover_style:",
            "pub fn style(",
            "pub const fn style(",
            "style_explicit",
        ] {
            assert!(
                !public.contains(forbidden),
                "raw style API leaked: {forbidden}"
            );
        }
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
        let left = [
            StatusSlot::mode("m", "NOR"),
            StatusSlot::context("c", "path"),
        ];
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

    #[test]
    fn transient_is_right_edge_text_secondary() {
        let system = DesignSystem::default();
        let left = [StatusSlot::mode("m", "NOR")];
        let msg = TransientStatus::new("Cell saved");
        let bar = StatusBar::new(&left, &[], &system).transient(&msg);
        let mut state = StatusBarState::default();
        let area = Rect::new(0, 0, 40, 1);
        let mut buf = Buffer::empty(area);
        (&bar).render(area, &mut buf, &mut state);
        let row: String = (0..area.width).map(|x| buf[(x, 0)].symbol()).collect();
        assert!(row.contains("Cell saved"), "{row}");
        assert!(row.trim_end().ends_with("Cell saved"), "{row}");
        let start = row.find("Cell saved").unwrap() as u16;
        assert_eq!(
            buf[(start, 0)].fg,
            system.junie_theme().secondary().fg.unwrap()
        );
        assert!(!row.contains('.'), "{row}");
    }

    #[test]
    fn default_ttl_is_4s() {
        assert_eq!(STATUS_DEFAULT_TTL_MS, 4_000);
        let mut state = StatusBarState::<&str>::new();
        state.set_transient_at(Some("Cell saved"), 0);
        state.expire_transient(3_999);
        assert_eq!(state.transient.as_deref(), Some("Cell saved"));
        state.expire_transient(4_000);
        assert!(state.transient.is_none());
    }
}
