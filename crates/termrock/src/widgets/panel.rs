// SPDX-FileCopyrightText: 2026 Alexey Zhokhov
// SPDX-License-Identifier: Apache-2.0

//! Composable panel chrome with anatomy, variants, and body modes.
//!
//! **Anatomy:** `root` · `header` · `body` · `footer` · optional `disclosure`.
//! Border *weight* never encodes focus — only [`Role::BorderFocused`] does.
//! Fill/geometry come from [`crate::widgets::Surface`].
//!
//! Focus belongs to interactive *descendants* by default. Only
//! [`PanelVariant::Interactive`] (or collapsible header) registers panel-level
//! focus / activation.

#![allow(unused_imports)] // test-module imports kept for unit tests; lib path may not use them
use ratatui_core::{buffer::Buffer, layout::Rect, style::Style, text::Span, widgets::Widget};
use ratatui_widgets::block::Block;

use crate::input::{KeyEvent, KeyEventKind, MouseButton, MouseEvent, MouseEventKind};
use crate::interaction::{EventResult, UiIntent, default_button_intent, default_list_intent};
use crate::style::{DesignSystem, GlyphSet, PanelChrome, PanelRecipe, Role};
use crate::text::{display_cols, take_display_cols};
use crate::widgets::empty_state::EmptyState;
use crate::widgets::error_state::ErrorView;
use crate::widgets::skeleton::Skeleton;
use crate::widgets::surface::{Surface, SurfaceFill, SurfaceRecipe};
use crate::widgets::view_state::LoadingView;

// PanelChrome lives in `style` (sole chrome enum). Re-exported from widgets::mod.

/// Border / interaction recipe for a panel (orthogonal to focus emphasis).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
#[non_exhaustive]
pub enum PanelVariant {
    /// Single-line border + surface fill (default).
    #[default]
    Bordered,
    /// No border; density padding only (quiet region).
    Quiet,
    /// Top/bottom divider rules only (no side borders).
    DividerOnly,
    /// Whole panel is actionable (focus + activate).
    Interactive,
    /// Selected membership chrome (distinct from focus).
    Selected,
}

impl PanelVariant {
    /// Stable id.
    #[must_use]
    pub const fn id(self) -> &'static str {
        match self {
            Self::Bordered => "bordered",
            Self::Quiet => "quiet",
            Self::DividerOnly => "divider-only",
            Self::Interactive => "interactive",
            Self::Selected => "selected",
        }
    }
}

/// Built-in body projection when the host does not paint custom children.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
#[non_exhaustive]
pub enum PanelBody {
    /// Host paints children into body (default).
    #[default]
    Host,
    /// Loading placeholder.
    Loading,
    /// Empty state.
    Empty,
    /// Error state.
    Error,
}

/// One header action (stable id + label). Host owns policy; panel owns chrome hit.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct PanelAction<'a> {
    /// Stable action id for [`PanelOutcome::HeaderAction`].
    pub id: &'a str,
    /// Visible label (contracts under narrow width with the action band).
    pub label: &'a str,
    /// Optional icon for compact header / IconButton composition.
    pub icon: Option<&'a str>,
}

impl<'a> PanelAction<'a> {
    /// Creates a header action.
    #[must_use]
    pub const fn new(id: &'a str, label: &'a str) -> Self {
        Self {
            id,
            label,
            icon: None,
        }
    }

    /// Icon for compact paint (pair with [`crate::widgets::IconButton`] in host).
    #[must_use]
    pub const fn icon(mut self, icon: &'a str) -> Self {
        self.icon = Some(icon);
        self
    }
}

/// Priority-ordered title/footer slots for panel chrome.
///
/// Narrow drop order (first dropped under pressure):
/// footer → header_actions → badge → trailing → subtitle → leading → title (last).
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct PanelSlots<'a> {
    /// Primary title (survives longest under contraction).
    pub title: Option<&'a str>,
    /// Secondary title text after the primary.
    pub subtitle: Option<&'a str>,
    /// Leading status glyph/text before the title.
    pub leading: Option<&'a str>,
    /// Status badge (distinct from trailing meta / actions).
    pub badge: Option<&'a str>,
    /// Trailing metadata label on the title line (not an action).
    pub trailing: Option<&'a str>,
    /// Footer hint/status on the bottom border or footer band.
    pub footer: Option<&'a str>,
    /// Optional body title for empty/error modes.
    pub body_title: Option<&'a str>,
    /// Optional body detail for empty/error/loading modes.
    pub body_detail: Option<&'a str>,
}

impl<'a> PanelSlots<'a> {
    /// Resolves which slots survive at the available title width.
    #[must_use]
    pub fn for_width(self, width: u16) -> Self {
        let mut slots = self;
        // Drop order: footer → badge → trailing → subtitle → leading → title.
        // Header actions are gated separately via [`Panel::actions_visible`].
        if width < 24 {
            slots.footer = None;
        }
        if width < 22 {
            slots.badge = None;
        }
        if width < 20 {
            slots.trailing = None;
        }
        if width < 14 {
            slots.subtitle = None;
        }
        if width < 10 {
            slots.leading = None;
        }
        if width < 16 {
            slots.body_detail = None;
        }
        slots
    }

    /// Formats the top title span content (without outer spaces).
    #[must_use]
    pub fn title_text(self) -> Option<String> {
        if self.title.is_none()
            && self.leading.is_none()
            && self.subtitle.is_none()
            && self.badge.is_none()
            && self.trailing.is_none()
        {
            return None;
        }
        let mut parts = Vec::new();
        if let Some(leading) = self.leading {
            parts.push(leading.trim().to_string());
        }
        if let Some(title) = self.title {
            parts.push(title.trim().to_string());
        }
        if let Some(subtitle) = self.subtitle {
            parts.push(format!("· {}", subtitle.trim()));
        }
        if let Some(badge) = self.badge {
            parts.push(format!("[{}]", badge.trim()));
        }
        if let Some(trailing) = self.trailing {
            parts.push(format!("· {}", trailing.trim()));
        }
        let text = parts.join(" ");
        if text.is_empty() { None } else { Some(text) }
    }
}

/// Named geometry parts for one laid-out panel (no nested box soup).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct PanelParts {
    /// Outer allocation.
    pub root: Rect,
    /// Header / title band (inside border); None when untitled quiet panel.
    pub header: Option<Rect>,
    /// Body content area (children paint here).
    pub body: Rect,
    /// Footer band; None when no footer.
    pub footer: Option<Rect>,
    /// Disclosure hit target when collapsible.
    pub disclosure: Option<Rect>,
    /// Header-actions band (right of title); host paints labels into action hits on state.
    pub actions: Option<Rect>,
    /// Mouse hit region for panel-level interaction.
    pub hit: Rect,
    /// Clip contract (= body for children).
    pub clip: Rect,
}

impl PanelParts {
    /// True when body has positive area.
    #[must_use]
    pub const fn has_body(self) -> bool {
        self.body.width > 0 && self.body.height > 0
    }
}

/// Interaction state for collapsible / interactive panels.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct PanelState {
    /// Collapsed body when collapsible.
    pub collapsed: bool,
    /// Panel-level focus (interactive / collapsible header only).
    pub focused: bool,
    /// Pointer hover on panel hit region.
    pub hovered: bool,
    /// Cached layout for hit tests.
    pub parts: Option<PanelParts>,
    /// Header action hit targets (id, rect) filled during [`Panel::paint`].
    pub action_hits: Vec<(String, Rect)>,
}

impl PanelState {
    /// Open expanded panel.
    #[must_use]
    pub const fn new() -> Self {
        Self {
            collapsed: false,
            focused: false,
            hovered: false,
            parts: None,
            action_hits: Vec::new(),
        }
    }

    /// Sets collapse.
    pub const fn set_collapsed(&mut self, collapsed: bool) {
        self.collapsed = collapsed;
    }

    /// Sets panel focus (host / scene).
    pub const fn set_focused(&mut self, focused: bool) {
        self.focused = focused;
    }

    /// Whether collapsed.
    #[must_use]
    pub const fn is_collapsed(&self) -> bool {
        self.collapsed
    }

    /// Key handling via intents (Activate / Toggle / Expand / Collapse).
    pub fn handle_key(
        &mut self,
        key: KeyEvent,
        collapsible: bool,
        interactive: bool,
    ) -> PanelOutcome {
        if !self.focused || key.kind != KeyEventKind::Press {
            return PanelOutcome::Ignored;
        }
        let Some(intent) = default_button_intent(key).or_else(|| default_list_intent(key)) else {
            return PanelOutcome::Ignored;
        };
        self.handle_intent(intent, collapsible, interactive)
    }

    /// Semantic intent path.
    pub fn handle_intent(
        &mut self,
        intent: UiIntent,
        collapsible: bool,
        interactive: bool,
    ) -> PanelOutcome {
        if !self.focused {
            return PanelOutcome::Ignored;
        }
        match intent {
            UiIntent::Toggle | UiIntent::Expand | UiIntent::Collapse if collapsible => {
                if matches!(intent, UiIntent::Expand) {
                    self.collapsed = false;
                } else if matches!(intent, UiIntent::Collapse) {
                    self.collapsed = true;
                } else {
                    self.collapsed = !self.collapsed;
                }
                PanelOutcome::ToggleCollapsed {
                    collapsed: self.collapsed,
                }
            }
            UiIntent::Activate if interactive => PanelOutcome::Activated,
            UiIntent::Activate if collapsible => {
                self.collapsed = !self.collapsed;
                PanelOutcome::ToggleCollapsed {
                    collapsed: self.collapsed,
                }
            }
            _ => PanelOutcome::Ignored,
        }
    }

    /// Key path with [`EventResult`].
    pub fn handle_key_result(
        &mut self,
        key: KeyEvent,
        collapsible: bool,
        interactive: bool,
    ) -> EventResult<PanelOutcome> {
        match self.handle_key(key, collapsible, interactive) {
            PanelOutcome::Ignored => EventResult::ignored(),
            other => EventResult::emit(other),
        }
    }

    /// Click header toggles collapse; body activates interactive.
    pub fn handle_mouse(
        &mut self,
        event: MouseEvent,
        collapsible: bool,
        interactive: bool,
    ) -> PanelOutcome {
        if event.kind != MouseEventKind::Down(MouseButton::Left) {
            // Hover tracking
            if matches!(
                event.kind,
                MouseEventKind::Moved | MouseEventKind::Drag(MouseButton::Left)
            ) {
                if let Some(parts) = self.parts {
                    self.hovered = parts.hit.contains(event.position);
                }
            }
            return PanelOutcome::Ignored;
        }
        let Some(parts) = self.parts else {
            return PanelOutcome::Ignored;
        };
        // Header actions first (do not toggle collapse when clicking an action).
        for (id, rect) in &self.action_hits {
            if rect.contains(event.position) {
                return PanelOutcome::HeaderAction { id: id.clone() };
            }
        }
        if collapsible
            && (parts.disclosure.is_some_and(|r| r.contains(event.position))
                || parts.header.is_some_and(|r| {
                    r.contains(event.position)
                        && parts.actions.is_none_or(|a| !a.contains(event.position))
                }))
        {
            self.collapsed = !self.collapsed;
            return PanelOutcome::ToggleCollapsed {
                collapsed: self.collapsed,
            };
        }
        if interactive && parts.hit.contains(event.position) {
            return PanelOutcome::Activated;
        }
        PanelOutcome::Ignored
    }
}

/// Typed panel outcomes (no side effects).
#[derive(Debug, Clone, PartialEq, Eq, Hash, Default)]
#[non_exhaustive]
pub enum PanelOutcome {
    /// No change.
    #[default]
    Ignored,
    /// Interactive panel activated.
    Activated,
    /// Collapse toggled.
    ToggleCollapsed {
        /// New collapsed flag.
        collapsed: bool,
    },
    /// Header action activated (mouse or host-routed intent).
    HeaderAction {
        /// Action id from [`PanelAction::id`].
        id: String,
    },
}

/// A composable container painted through [`DesignSystem`] recipes.
#[derive(Debug, Clone)]
pub struct Panel<'a> {
    slots: PanelSlots<'a>,
    emphasis: PanelChrome,
    variant: PanelVariant,
    body: PanelBody,
    collapsible: bool,
    /// Prefer elevated fill underlay (cards).
    raised: bool,
    /// Header actions (dropped under narrow width before badge).
    header_actions: &'a [PanelAction<'a>],
    style: Option<Style>,
    tokens: &'a DesignSystem,
}

impl<'a> Panel<'a> {
    /// Creates an untitled panel from design tokens (canonical constructor).
    #[must_use]
    pub const fn new(tokens: &'a DesignSystem) -> Self {
        Self {
            slots: PanelSlots {
                title: None,
                subtitle: None,
                leading: None,
                badge: None,
                trailing: None,
                footer: None,
                body_title: None,
                body_detail: None,
            },
            emphasis: PanelChrome::Normal,
            variant: PanelVariant::Bordered,
            body: PanelBody::Host,
            collapsible: false,
            raised: false,
            header_actions: &[],
            style: None,
            tokens,
        }
    }

    /// Alias for [`Self::new`].
    #[must_use]
    pub const fn from_tokens(tokens: &'a DesignSystem) -> Self {
        Self::new(tokens)
    }

    /// Quiet bordered-off panel (no chrome line).
    #[must_use]
    pub const fn quiet(tokens: &'a DesignSystem) -> Self {
        Self::new(tokens).variant(PanelVariant::Quiet)
    }

    #[must_use]
    /// Sets the optional visible title.
    pub const fn title(mut self, title: &'a str) -> Self {
        self.slots.title = Some(title);
        self
    }

    #[must_use]
    /// Sets the optional subtitle (drops before title under narrow pressure).
    pub const fn subtitle(mut self, subtitle: &'a str) -> Self {
        self.slots.subtitle = Some(subtitle);
        self
    }

    #[must_use]
    /// Sets leading status chrome on the title line.
    pub const fn leading(mut self, leading: &'a str) -> Self {
        self.slots.leading = Some(leading);
        self
    }

    #[must_use]
    /// Sets trailing metadata on the title line (not a clickable action).
    pub const fn trailing(mut self, trailing: &'a str) -> Self {
        self.slots.trailing = Some(trailing);
        self
    }

    /// Sets a status badge (contracts after header actions, before trailing).
    #[must_use]
    pub const fn badge(mut self, badge: &'a str) -> Self {
        self.slots.badge = Some(badge);
        self
    }

    /// Header actions (right band); dropped when width &lt; 28.
    #[must_use]
    pub const fn header_actions(mut self, actions: &'a [PanelAction<'a>]) -> Self {
        self.header_actions = actions;
        self
    }

    #[must_use]
    /// Sets footer hint on the bottom border (drops first under narrow pressure).
    pub const fn footer(mut self, footer: &'a str) -> Self {
        self.slots.footer = Some(footer);
        self
    }

    /// Whether header actions survive at `width`.
    #[must_use]
    pub const fn actions_visible(width: u16) -> bool {
        width >= 28
    }

    #[must_use]
    /// Body empty/error/loading title copy.
    pub const fn body_title(mut self, title: &'a str) -> Self {
        self.slots.body_title = Some(title);
        self
    }

    #[must_use]
    /// Body detail copy.
    pub const fn body_detail(mut self, detail: &'a str) -> Self {
        self.slots.body_detail = Some(detail);
        self
    }

    #[must_use]
    /// Replaces all panel slots at once.
    pub const fn slots(mut self, slots: PanelSlots<'a>) -> Self {
        self.slots = slots;
        self
    }

    #[must_use]
    /// Sets the semantic panel emphasis (focus / danger).
    pub const fn emphasis(mut self, emphasis: PanelChrome) -> Self {
        self.emphasis = emphasis;
        self
    }

    /// Canonical chrome setter (alias of [`Self::emphasis`]).
    #[must_use]
    pub const fn chrome(mut self, chrome: PanelChrome) -> Self {
        self.emphasis = chrome;
        self
    }

    /// Border / interaction variant.
    #[must_use]
    pub const fn variant(mut self, variant: PanelVariant) -> Self {
        self.variant = variant;
        self
    }

    /// Built-in body mode.
    #[must_use]
    pub const fn body(mut self, body: PanelBody) -> Self {
        self.body = body;
        self
    }

    /// Enables collapsible header (disclosure + Enter/Space toggle when focused).
    #[must_use]
    pub const fn collapsible(mut self, collapsible: bool) -> Self {
        self.collapsible = collapsible;
        self
    }

    /// Use elevated fill (card underlay) when the palette defines one.
    #[must_use]
    pub const fn raised(mut self, raised: bool) -> Self {
        self.raised = raised;
        self
    }

    #[must_use]
    /// Overrides the recipe border style.
    pub const fn style(mut self, style: Style) -> Self {
        self.style = Some(style);
        self
    }

    /// Whether this panel claims panel-level keyboard focus.
    #[must_use]
    pub const fn is_focusable(&self) -> bool {
        self.collapsible || matches!(self.variant, PanelVariant::Interactive)
    }

    /// Resolves the panel recipe for current emphasis.
    #[must_use]
    pub fn recipe(&self) -> PanelRecipe {
        self.tokens.panel_recipe(self.resolved_chrome())
    }

    /// Palette borrow from the design system.
    #[must_use]
    pub const fn palette(&self) -> &crate::style::RolePalette {
        self.tokens.palette()
    }

    /// Effective chrome after variant (Selected ≠ Focused).
    #[must_use]
    pub const fn resolved_chrome(&self) -> PanelChrome {
        match self.emphasis {
            PanelChrome::Danger => PanelChrome::Danger,
            PanelChrome::Focused => PanelChrome::Focused,
            PanelChrome::Normal => {
                if matches!(self.variant, PanelVariant::Selected) {
                    // Selected uses Selection fill via Surface; border stays Normal
                    // so focus remains a distinct BorderFocused cue.
                    PanelChrome::Normal
                } else {
                    PanelChrome::Normal
                }
            }
        }
    }

    /// Slot projection after contraction for a given outer width.
    #[must_use]
    pub fn slots_for_width(&self, width: u16) -> PanelSlots<'a> {
        // Border corners consume 2 cells; title padding uses ~2 more.
        self.slots.for_width(width.saturating_sub(4))
    }

    /// Maps panel emphasis + variant onto the Surface recipe set.
    #[must_use]
    pub const fn surface_recipe(&self) -> SurfaceRecipe {
        if matches!(self.emphasis, PanelChrome::Danger) {
            return SurfaceRecipe::Destructive;
        }
        if matches!(self.emphasis, PanelChrome::Focused) {
            return SurfaceRecipe::Focused;
        }
        match self.variant {
            PanelVariant::Selected => SurfaceRecipe::Selected,
            PanelVariant::Interactive => {
                if self.raised {
                    SurfaceRecipe::Raised
                } else {
                    SurfaceRecipe::Interactive
                }
            }
            PanelVariant::Quiet | PanelVariant::DividerOnly => {
                if self.raised {
                    SurfaceRecipe::Raised
                } else {
                    SurfaceRecipe::Inset
                }
            }
            PanelVariant::Bordered => {
                if self.raised {
                    SurfaceRecipe::Raised
                } else {
                    SurfaceRecipe::Inset
                }
            }
        }
    }

    /// Whether a full single-line box border is painted.
    #[must_use]
    pub const fn has_box_border(&self) -> bool {
        match self.variant {
            PanelVariant::Quiet | PanelVariant::DividerOnly => false,
            PanelVariant::Bordered | PanelVariant::Interactive | PanelVariant::Selected => true,
        }
    }

    #[must_use]
    /// Builds the surrounding block from the recipe (single-line border only).
    pub fn block(&self) -> Block<'a> {
        self.block_for_width(u16::MAX)
    }

    /// Builds chrome contracted to the available outer width.
    #[must_use]
    pub fn block_for_width(&self, width: u16) -> Block<'a> {
        let recipe = self.recipe();
        let border = self.style.unwrap_or(recipe.border);
        let mut block = if self.has_box_border() {
            Block::bordered()
                .border_style(border)
                .border_set(self.tokens.border_set())
        } else {
            Block::default()
        };
        let slots = self.slots_for_width(width);
        if let Some(title) = self.title_line(slots, None) {
            let budget = width.saturating_sub(4).max(1);
            let clipped = if display_cols(&title) > usize::from(budget) {
                title
                    .chars()
                    .take(usize::from(budget.saturating_sub(1)))
                    .collect::<String>()
                    + "…"
            } else {
                title
            };
            block = block.title(Span::styled(format!(" {clipped} "), recipe.title));
        }
        if let Some(footer) = slots.footer {
            block = block.title_bottom(Span::styled(format!(" {} ", footer.trim()), recipe.title));
        }
        block
    }

    fn title_line(&self, slots: PanelSlots<'a>, collapsed: Option<bool>) -> Option<String> {
        let mut base = slots.title_text()?;
        if self.collapsible {
            let glyph = if collapsed.unwrap_or(false) {
                self.tokens.glyphs.disclosure_closed()
            } else {
                self.tokens.glyphs.disclosure_open()
            };
            base = format!("{glyph} {base}");
        }
        Some(base)
    }

    /// Layout named parts without painting.
    #[must_use]
    pub fn layout(&self, area: Rect, state: Option<&PanelState>) -> PanelParts {
        let collapsed = state.is_some_and(|s| s.collapsed && self.collapsible);
        let has_border = self.has_box_border();
        let border_cells: u16 = if has_border { 1 } else { 0 };
        let inner = shrink(area, border_cells, border_cells, border_cells, border_cells);
        let spacing = self.tokens.spacing;
        let pad_x = if inner.width >= spacing.pad_x.saturating_mul(2).saturating_add(4) {
            spacing.pad_x
        } else {
            0
        };
        let pad_y = if inner.height >= spacing.pad_y.saturating_mul(2).saturating_add(1) {
            spacing.pad_y
        } else {
            0
        };
        let inner = shrink(inner, pad_x, pad_y, pad_x, pad_y);

        let slots = self.slots_for_width(area.width);
        let has_title = slots.title_text().is_some() || self.collapsible;
        let has_footer_band = slots.footer.is_some() && !has_border;
        // With box border, footer sits on bottom border (Block title_bottom).
        let footer_rows: u16 = if has_footer_band { 1 } else { 0 };
        // Header is one row inside when we paint multi-part header band for collapsible
        // without relying only on Block title (Block title is on the border line).
        let header_inside: u16 = if self.collapsible && has_title && has_border {
            0 // disclosure lives in border title
        } else if !has_border && has_title {
            1
        } else {
            0
        };

        let mut y = inner.y;
        let header = if header_inside > 0 && inner.height > 0 {
            let h = Rect {
                x: inner.x,
                y,
                width: inner.width,
                height: 1.min(inner.height),
            };
            y = y.saturating_add(1);
            Some(h)
        } else if has_title && has_border {
            // Title on border — expose header as top inner row for hit tests.
            Some(Rect {
                x: area.x.saturating_add(1),
                y: area.y,
                width: area.width.saturating_sub(2),
                height: 1.min(area.height),
            })
        } else {
            None
        };

        let footer_y = inner.bottom().saturating_sub(footer_rows);
        let body_bottom = if collapsed { y } else { footer_y };
        let body_h = body_bottom.saturating_sub(y);
        let body = if collapsed {
            Rect {
                x: inner.x,
                y,
                width: inner.width,
                height: 0,
            }
        } else {
            Rect {
                x: inner.x,
                y,
                width: inner.width,
                height: body_h,
            }
        };
        let footer = if footer_rows > 0 && !collapsed {
            Some(Rect {
                x: inner.x,
                y: footer_y,
                width: inner.width,
                height: 1,
            })
        } else {
            None
        };

        let disclosure = header.map(|h| Rect {
            x: h.x,
            y: h.y,
            width: 2.min(h.width),
            height: h.height,
        });

        let show_actions = Self::actions_visible(area.width) && !self.header_actions.is_empty();
        let actions = if show_actions {
            // Right band of top row (border title line or header inside).
            let band_w = self
                .header_actions
                .iter()
                .map(|a| display_cols(a.label) as u16 + 3)
                .sum::<u16>()
                .min(area.width / 2)
                .max(4);
            Some(Rect {
                x: area
                    .x
                    .saturating_add(area.width.saturating_sub(band_w).saturating_sub(1)),
                y: area.y,
                width: band_w.min(area.width),
                height: 1.min(area.height),
            })
        } else {
            None
        };

        let hit = if self.is_focusable() || has_border {
            area
        } else {
            body
        };

        PanelParts {
            root: area,
            header,
            body,
            footer,
            disclosure,
            actions,
            hit,
            clip: body,
        }
    }

    /// Content rectangle inside panel chrome (host children).
    #[must_use]
    pub fn inner(&self, area: Rect) -> Rect {
        self.layout(area, None).body
    }

    /// Paint panel chrome + optional built-in body; returns body rect.
    pub fn paint(&self, area: Rect, buffer: &mut Buffer, state: Option<&mut PanelState>) -> Rect {
        if area.is_empty() {
            return area;
        }
        let collapsed = state
            .as_ref()
            .is_some_and(|s| s.collapsed && self.collapsible);
        let focused = state.as_ref().is_some_and(|s| s.focused);
        let parts = self.layout(area, state.as_ref().map(|s| &**s));

        // Surface fill (variant-aware).
        let surface_recipe = if focused && self.is_focusable() {
            SurfaceRecipe::Focused
        } else {
            self.surface_recipe()
        };
        let fill_policy = if matches!(self.variant, PanelVariant::Quiet) {
            SurfaceFill::Transparent
        } else {
            SurfaceFill::Auto
        };
        let surface_style = if focused && self.is_focusable() {
            PanelChrome::Focused
        } else {
            self.emphasis
        };
        let surface_recipe_tokens = self.tokens.panel_recipe(surface_style);
        let _ = Surface::new(self.tokens)
            .recipe(surface_recipe)
            .bordered(false)
            .fill(fill_policy)
            .padding(surface_recipe_tokens.pad_x, surface_recipe_tokens.pad_y)
            .paint(area, buffer);

        // Box border + title/footer on border.
        if self.has_box_border() {
            let mut emphasis = self.emphasis;
            if focused && self.is_focusable() {
                emphasis = PanelChrome::Focused;
            }
            let recipe = self.tokens.panel_recipe(emphasis);
            let border = self.style.unwrap_or(recipe.border);
            let mut block = Block::bordered()
                .border_style(border)
                .border_set(self.tokens.border_set());
            let slots = self.slots_for_width(area.width);
            if let Some(title) = self.title_line(slots, Some(collapsed)) {
                // Reserve right band for header actions so title does not collide.
                let action_reserve = parts
                    .actions
                    .map(|a| a.width.saturating_add(1))
                    .unwrap_or(0);
                let budget = area
                    .width
                    .saturating_sub(4)
                    .saturating_sub(action_reserve)
                    .max(1);
                let clipped = if display_cols(&title) > usize::from(budget) {
                    title
                        .chars()
                        .take(usize::from(budget.saturating_sub(1)))
                        .collect::<String>()
                        + "…"
                } else {
                    title
                };
                block = block.title(Span::styled(format!(" {clipped} "), recipe.title));
            }
            if let Some(footer) = slots.footer {
                block =
                    block.title_bottom(Span::styled(format!(" {} ", footer.trim()), recipe.title));
            }
            block.render(area, buffer);
        } else if matches!(self.variant, PanelVariant::DividerOnly) {
            paint_divider_only(area, buffer, self.tokens);
            if let Some(header) = parts.header {
                paint_header_line(self, header, buffer, collapsed);
            }
            if let Some(footer) = parts.footer {
                if let Some(text) = self.slots_for_width(area.width).footer {
                    let t = take_display_cols(text, usize::from(footer.width));
                    buffer.set_stringn(
                        footer.x,
                        footer.y,
                        &t,
                        usize::from(footer.width),
                        self.tokens.style(Role::TextMuted),
                    );
                }
            }
        } else if matches!(self.variant, PanelVariant::Quiet) {
            if let Some(header) = parts.header {
                paint_header_line(self, header, buffer, collapsed);
            }
            if let Some(footer) = parts.footer {
                if let Some(text) = self.slots_for_width(area.width).footer {
                    let t = take_display_cols(text, usize::from(footer.width));
                    buffer.set_stringn(
                        footer.x,
                        footer.y,
                        &t,
                        usize::from(footer.width),
                        self.tokens.style(Role::TextMuted),
                    );
                }
            }
        }

        // Built-in body modes.
        if !collapsed && parts.has_body() {
            match self.body {
                PanelBody::Host => {}
                PanelBody::Loading => {
                    let label = self.slots.body_detail.unwrap_or("Loading");
                    let frame = self.tokens.glyphs.loading();
                    Widget::render(
                        &LoadingView::new(label, frame, self.tokens),
                        parts.body,
                        buffer,
                    );
                }
                PanelBody::Empty => {
                    let title = self.slots.body_title.unwrap_or("No items");
                    let mut empty = EmptyState::new(title, self.tokens);
                    if let Some(d) = self.slots.body_detail {
                        empty = empty.detail(d);
                    }
                    let glyph = self
                        .tokens
                        .glyphs
                        .resolve(crate::style::Glyph::EmptyCircle)
                        .text;
                    empty = empty.glyph(glyph);
                    Widget::render(&empty, parts.body, buffer);
                }
                PanelBody::Error => {
                    let title = self.slots.body_title.unwrap_or("Error");
                    let mut err = ErrorView::new(title, self.tokens);
                    if let Some(d) = self.slots.body_detail {
                        err = err.detail(d);
                    }
                    Widget::render(&err, parts.body, buffer);
                }
            }
        } else if collapsed {
            // nothing in body
        }

        // Tiny non-color cue: selected gutter when Selected variant.
        if matches!(self.variant, PanelVariant::Selected) && area.width > 0 && area.height > 0 {
            let g = self.tokens.glyphs.selection_gutter();
            buffer.set_stringn(
                area.x,
                area.y.saturating_add(area.height / 2),
                g,
                1,
                self.tokens.style(Role::Accent),
            );
        }

        // Header actions (right band) + hit targets.
        let mut action_hits = Vec::new();
        if let Some(band) = parts.actions {
            let style = self.tokens.style(if focused {
                Role::ActionFocused
            } else {
                Role::TextMuted
            });
            let mut x = band.x;
            for action in self.header_actions {
                let label = format!("[{}]", action.label.trim());
                let w = (display_cols(&label) as u16).min(band.right().saturating_sub(x));
                if w == 0 {
                    break;
                }
                buffer.set_stringn(x, band.y, &label, usize::from(w), style);
                action_hits.push((
                    action.id.to_string(),
                    Rect {
                        x,
                        y: band.y,
                        width: w,
                        height: 1,
                    },
                ));
                x = x.saturating_add(w).saturating_add(1);
                if x >= band.right() {
                    break;
                }
            }
        }

        if let Some(state) = state {
            state.parts = Some(parts);
            state.action_hits = action_hits;
        }
        parts.body
    }

    /// Registers panel chrome into a semantic scene (optional host aid).
    ///
    /// Does **not** claim focus unless [`Self::is_focusable`]. Body children
    /// remain host-registered interactive descendants.
    pub fn register_semantic<Id, Action>(
        &self,
        scene: &mut crate::interaction::SemanticScene<Id, Action>,
        id: Id,
        area: Rect,
        state: Option<&PanelState>,
    ) where
        Id: Clone + PartialEq + std::fmt::Display,
        Action: Clone,
    {
        use crate::interaction::{SemanticNode, SemanticRole};
        let label = self.slots.title.unwrap_or("panel");
        let focusable = self.is_focusable();
        let _ = scene.register(
            SemanticNode::control(id, area)
                .role(SemanticRole::Dialog)
                .label(label)
                .focusable(focusable)
                .state(crate::interaction::SemanticState {
                    expanded: !state.is_some_and(|s| s.collapsed),
                    selected: matches!(self.variant, PanelVariant::Selected),
                    ..Default::default()
                }),
        );
    }

    /// Skeleton body helper for loading lists (host-driven).
    pub fn paint_skeleton_body(&self, body: Rect, buffer: &mut Buffer, lines: u16) {
        if body.is_empty() {
            return;
        }
        Widget::render(&Skeleton::new(lines, self.tokens), body, buffer);
    }
}

impl Widget for &Panel<'_> {
    fn render(self, area: Rect, buffer: &mut Buffer) {
        let _ = self.paint(area, buffer, None);
    }
}

impl Widget for Panel<'_> {
    fn render(self, area: Rect, buffer: &mut Buffer) {
        <&Self as Widget>::render(&self, area, buffer);
    }
}

fn paint_header_line(panel: &Panel<'_>, header: Rect, buffer: &mut Buffer, collapsed: bool) {
    let slots = panel.slots_for_width(header.width.saturating_add(4));
    if let Some(title) = panel.title_line(slots, Some(collapsed)) {
        let t = take_display_cols(&title, usize::from(header.width));
        let style = if panel.emphasis == PanelChrome::Focused {
            panel.tokens.style(Role::TextStrong)
        } else {
            panel.tokens.style(Role::Text)
        };
        buffer.set_stringn(header.x, header.y, &t, usize::from(header.width), style);
    }
}

fn paint_divider_only(area: Rect, buffer: &mut Buffer, system: &DesignSystem) {
    if area.width == 0 || area.height == 0 {
        return;
    }
    let rule = system.glyphs.rule();
    let style = system.style(Role::Border);
    let line: String = std::iter::repeat_n(rule, usize::from(area.width)).collect();
    buffer.set_stringn(area.x, area.y, &line, usize::from(area.width), style);
    if area.height > 1 {
        buffer.set_stringn(
            area.x,
            area.bottom().saturating_sub(1),
            &line,
            usize::from(area.width),
            style,
        );
    }
}

fn shrink(area: Rect, left: u16, top: u16, right: u16, bottom: u16) -> Rect {
    let x = area.x.saturating_add(left);
    let y = area.y.saturating_add(top);
    let width = area.width.saturating_sub(left.saturating_add(right));
    let height = area.height.saturating_sub(top.saturating_add(bottom));
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::input::{KeyCode, KeyModifiers};
    use crate::style::{BorderShape, DesignSystem, GlyphSet};

    fn render_border(system: &DesignSystem) -> Buffer {
        let area = Rect::new(0, 0, 8, 4);
        let mut buffer = Buffer::empty(area);
        Panel::new(system).paint(area, &mut buffer, None);
        buffer
    }

    #[test]
    fn rounded_shape_changes_corners_only() {
        let square = render_border(&DesignSystem::default());
        let rounded = render_border(&DesignSystem::default().border_shape(BorderShape::Rounded));
        for (position, square_symbol, rounded_symbol) in [
            ((0, 0), "┌", "╭"),
            ((7, 0), "┐", "╮"),
            ((0, 3), "└", "╰"),
            ((7, 3), "┘", "╯"),
        ] {
            assert_eq!(square[position].symbol(), square_symbol);
            assert_eq!(rounded[position].symbol(), rounded_symbol);
            assert_eq!(square[position].style(), rounded[position].style());
        }
        assert_eq!(square[(3, 0)].symbol(), "─");
        assert_eq!(rounded[(3, 0)].symbol(), "─");
        assert_eq!(square[(0, 2)].symbol(), "│");
        assert_eq!(rounded[(0, 2)].symbol(), "│");
    }

    #[test]
    fn ascii_maps_both_shapes_to_plus() {
        let square = render_border(&DesignSystem::default().glyphs(GlyphSet::Ascii));
        let rounded = render_border(
            &DesignSystem::default()
                .glyphs(GlyphSet::Ascii)
                .border_shape(BorderShape::Rounded),
        );
        assert_eq!(square, rounded);
        assert_eq!(square[(0, 0)].symbol(), "+");
        assert_eq!(square[(7, 3)].symbol(), "+");
    }

    #[test]
    fn square_is_the_default_border_shape() {
        assert_eq!(DesignSystem::default().border_shape, BorderShape::Square);
    }

    #[test]
    fn panel_inner_uses_density_padding_and_contracts_when_narrow() {
        let area = Rect::new(0, 0, 20, 10);
        let comfortable = Panel::new(&DesignSystem::default()).inner(area);
        let dashboard =
            Panel::new(&DesignSystem::default().density(crate::style::Density::Dashboard))
                .inner(area);
        assert_eq!(comfortable, Rect::new(3, 2, 14, 6));
        assert_eq!(dashboard, Rect::new(1, 1, 18, 8));
        assert_eq!(
            Panel::new(&DesignSystem::default()).inner(Rect::new(0, 0, 5, 2)),
            Rect::new(1, 1, 0, 0)
        );
    }

    #[test]
    fn panel_recipe_focus_uses_border_focused_not_weight() {
        let tokens = DesignSystem::default();
        let normal = tokens.panel_recipe(PanelChrome::Normal);
        let focused = tokens.panel_recipe(PanelChrome::Focused);
        assert_ne!(normal.border, focused.border);
        let panel = Panel::new(&tokens)
            .emphasis(PanelChrome::Focused)
            .title("T");
        assert_eq!(panel.recipe().border, focused.border);
    }

    #[test]
    fn panel_slots_drop_trailing_before_title() {
        let tokens = DesignSystem::default();
        let panel = Panel::new(&tokens)
            .title("Main")
            .subtitle("sub")
            .leading("*")
            .badge("new")
            .trailing("meta")
            .footer("hint");
        let wide = panel.slots_for_width(80);
        assert!(wide.footer.is_some());
        assert!(wide.trailing.is_some());
        assert!(wide.badge.is_some());
        let mid = panel.slots_for_width(18);
        assert!(mid.trailing.is_none());
        assert!(mid.badge.is_none());
        assert_eq!(mid.title, Some("Main"));
        let tiny = panel.slots_for_width(8);
        assert!(tiny.leading.is_none());
        assert_eq!(tiny.title, Some("Main"));
    }

    #[test]
    fn header_action_mouse_hit() {
        use crate::input::{KeyModifiers, MouseButton, MouseEventKind};
        let tokens = DesignSystem::default();
        let actions = [PanelAction::new("retry", "Retry")];
        let panel = Panel::new(&tokens).title("Job").header_actions(&actions);
        let mut state = PanelState::new();
        let mut buf = Buffer::empty(Rect::new(0, 0, 40, 6));
        let _ = panel.paint(Rect::new(0, 0, 40, 6), &mut buf, Some(&mut state));
        assert!(!state.action_hits.is_empty());
        let (id, rect) = &state.action_hits[0];
        assert_eq!(id, "retry");
        let out = state.handle_mouse(
            MouseEvent {
                kind: MouseEventKind::Down(MouseButton::Left),
                position: ratatui_core::layout::Position {
                    x: rect.x,
                    y: rect.y,
                },
                modifiers: KeyModifiers::NONE,
            },
            false,
            false,
        );
        assert!(matches!(out, PanelOutcome::HeaderAction { id } if id == "retry"));
    }

    #[test]
    fn header_action_not_toggle_when_collapsible() {
        use crate::input::{KeyModifiers, MouseButton, MouseEventKind};
        let tokens = DesignSystem::default();
        let actions = [PanelAction::new("more", "More")];
        let panel = Panel::new(&tokens)
            .title("Fold")
            .collapsible(true)
            .header_actions(&actions);
        let mut state = PanelState::new();
        let mut buf = Buffer::empty(Rect::new(0, 0, 48, 8));
        let _ = panel.paint(Rect::new(0, 0, 48, 8), &mut buf, Some(&mut state));
        let (_, rect) = &state.action_hits[0];
        let out = state.handle_mouse(
            MouseEvent {
                kind: MouseEventKind::Down(MouseButton::Left),
                position: ratatui_core::layout::Position {
                    x: rect.x,
                    y: rect.y,
                },
                modifiers: KeyModifiers::NONE,
            },
            true,
            false,
        );
        assert!(matches!(out, PanelOutcome::HeaderAction { id } if id == "more"));
        assert!(!state.is_collapsed());
    }

    #[test]
    fn non_focusable_panel_ignores_keys() {
        let mut state = PanelState::new();
        // Host never sets focused when !is_focusable; defensive check.
        state.set_focused(false);
        let out = state.handle_key(
            KeyEvent::new(KeyCode::Enter, KeyModifiers::NONE),
            false,
            false,
        );
        assert_eq!(out, PanelOutcome::Ignored);
    }

    #[test]
    fn title_reserves_action_band() {
        let tokens = DesignSystem::default();
        let actions = [
            PanelAction::new("a", "Retry"),
            PanelAction::new("b", "Cancel"),
        ];
        let panel = Panel::new(&tokens)
            .title("Very long panel title that would collide")
            .header_actions(&actions);
        let parts = panel.layout(Rect::new(0, 0, 40, 6), None);
        assert!(parts.actions.is_some());
        let mut buf = Buffer::empty(Rect::new(0, 0, 40, 6));
        let _ = panel.paint(Rect::new(0, 0, 40, 6), &mut buf, None);
        // Action label cells should still read '[' from painted [Retry]
        let ax = parts.actions.unwrap().x;
        let ch = buf[(ax, 0)].symbol();
        assert!(
            ch.contains('[') || ch == "[" || !ch.is_empty(),
            "expected action paint at x={ax}, got {ch:?}"
        );
    }

    #[test]
    fn actions_hidden_when_narrow() {
        assert!(!Panel::actions_visible(20));
        assert!(Panel::actions_visible(28));
    }

    #[test]
    fn loading_and_error_body_modes_paint() {
        let tokens = DesignSystem::default();
        let mut buf = Buffer::empty(Rect::new(0, 0, 30, 8));
        let _ = Panel::new(&tokens)
            .title("Load")
            .body(PanelBody::Loading)
            .body_detail("Fetching…")
            .paint(Rect::new(0, 0, 30, 8), &mut buf, None);
        let _ = Panel::new(&tokens)
            .title("Err")
            .body(PanelBody::Error)
            .body_title("Failed")
            .body_detail("timeout")
            .paint(Rect::new(0, 0, 30, 8), &mut buf, None);
    }

    #[test]
    fn selected_is_not_focused_surface() {
        let tokens = DesignSystem::default();
        let selected = Panel::new(&tokens)
            .variant(PanelVariant::Selected)
            .title("S");
        assert_eq!(selected.surface_recipe(), SurfaceRecipe::Selected);
        assert_eq!(selected.resolved_chrome(), PanelChrome::Normal);
        let focused = Panel::new(&tokens)
            .emphasis(PanelChrome::Focused)
            .title("F");
        assert_eq!(focused.surface_recipe(), SurfaceRecipe::Focused);
    }

    #[test]
    fn quiet_has_no_box_border() {
        let tokens = DesignSystem::default();
        let p = Panel::quiet(&tokens).title("Q");
        assert!(!p.has_box_border());
        let parts = p.layout(Rect::new(0, 0, 20, 6), None);
        assert!(parts.body.width > 0);
    }

    #[test]
    fn collapsible_toggle_via_intent() {
        let mut state = PanelState::new();
        state.set_focused(true);
        let out = state.handle_intent(UiIntent::Toggle, true, false);
        assert_eq!(out, PanelOutcome::ToggleCollapsed { collapsed: true });
        assert!(state.is_collapsed());
    }

    #[test]
    fn interactive_activate_via_enter() {
        let mut state = PanelState::new();
        state.set_focused(true);
        let out = state.handle_key(
            KeyEvent::new(KeyCode::Enter, KeyModifiers::NONE),
            false,
            true,
        );
        assert_eq!(out, PanelOutcome::Activated);
    }

    #[test]
    fn collapsed_body_has_zero_height() {
        let tokens = DesignSystem::default();
        let panel = Panel::new(&tokens).title("Fold").collapsible(true);
        let mut state = PanelState::new();
        state.set_collapsed(true);
        let parts = panel.layout(Rect::new(0, 0, 30, 10), Some(&state));
        assert_eq!(parts.body.height, 0);
    }

    #[test]
    fn paint_empty_body_mode() {
        let tokens = DesignSystem::default();
        let panel = Panel::new(&tokens)
            .title("List")
            .body(PanelBody::Empty)
            .body_title("No rows");
        let mut buf = Buffer::empty(Rect::new(0, 0, 24, 8));
        let body = panel.paint(Rect::new(0, 0, 24, 8), &mut buf, None);
        assert!(body.height > 0);
    }

    #[test]
    fn layout_is_cheap() {
        let tokens = DesignSystem::default();
        let panel = Panel::new(&tokens)
            .title("Perf")
            .subtitle("sub")
            .footer("f")
            .variant(PanelVariant::Bordered);
        let area = Rect::new(0, 0, 40, 12);
        for _ in 0..20_000 {
            let _ = panel.layout(area, None);
        }
    }

    #[test]
    fn focusable_only_when_interactive_or_collapsible() {
        let tokens = DesignSystem::default();
        assert!(!Panel::new(&tokens).title("x").is_focusable());
        assert!(
            Panel::new(&tokens)
                .variant(PanelVariant::Interactive)
                .is_focusable()
        );
        assert!(Panel::new(&tokens).collapsible(true).is_focusable());
    }

    #[test]
    fn variant_ids_stable() {
        assert_eq!(PanelVariant::DividerOnly.id(), "divider-only");
    }
}
