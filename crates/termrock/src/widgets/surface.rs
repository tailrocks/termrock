// SPDX-FileCopyrightText: 2026 Alexey Zhokhov
// SPDX-License-Identifier: Apache-2.0

//! Lowest-level visual ownership: fill, padding, border, clip, and hit geometry.
//!
//! **One Surface per region.** Compose siblings (AppShell slots, stacks) rather
//! than nesting Surfaces inside Surfaces ("box soup"). [`Panel`] builds title
//! chrome on top of Surface geometry; focus weight stays on semantic borders
//! only ([`Role::BorderFocused`]), never glyph weight.
//!
//! ## Clipping
//!
//! Terminal buffers cannot hard-clip child paints. [`SurfaceParts::clip`] (and
//! [`SurfaceParts::content`]) is the **legal paint rect** for children. Hosts
//! and child widgets must not write outside it.
//!
//! ## Hit regions
//!
//! [`SurfaceParts::hit`] is the mouse/hit-test region. Interactive recipes use
//! the full outer rect (including border); passive fills use the content rect.

use ratatui_core::{
    buffer::Buffer,
    layout::Rect,
    style::{Color, Modifier, Style},
    widgets::Widget,
};
use ratatui_widgets::block::Block;

use crate::style::{ColorCapability, DesignSystem, Role};

/// Surface visual recipe (elevation + interactive/state chrome).
///
/// Recipes select fill + default border policy. Focus never uses double-line
/// borders — only [`Role::BorderFocused`] vs [`Role::Border`].
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
#[non_exhaustive]
pub enum SurfaceRecipe {
    /// Terminal underlay. Prefers terminal-default background (`Color::Reset`).
    Canvas,
    /// Recessed / ordinary component surface (default).
    #[default]
    Inset,
    /// Well recessed below the ordinary surface (input fields, code wells).
    Sunken,
    /// Raised card / preview body — one step above the ordinary surface.
    Raised,
    /// Overlay host (dialog, popover body) above backdrop.
    Overlay,
    /// Overlay host that owns interaction — elevated fill, focused border.
    OverlayFocused,
    /// Destructive overlay host — elevated fill, danger border.
    OverlayDanger,
    /// Interactive but unfocused chrome (hoverable card, clickable tile).
    Interactive,
    /// Interaction owner — uses [`Role::BorderFocused`].
    Focused,
    /// Selected / active membership chrome.
    Selected,
    /// Caution surface (non-destructive warning).
    Warning,
    /// Destructive / danger surface.
    Destructive,
}

impl SurfaceRecipe {
    /// Stable id for stories / contracts.
    #[must_use]
    pub const fn id(self) -> &'static str {
        match self {
            Self::Canvas => "canvas",
            Self::Inset => "inset",
            Self::Sunken => "sunken",
            Self::Raised => "raised",
            Self::Overlay => "overlay",
            Self::OverlayFocused => "overlay-focused",
            Self::OverlayDanger => "overlay-danger",
            Self::Interactive => "interactive",
            Self::Focused => "focused",
            Self::Selected => "selected",
            Self::Warning => "warning",
            Self::Destructive => "destructive",
        }
    }

    /// Whether this recipe is interactive chrome by default (border + full hit).
    #[must_use]
    pub const fn is_interactive(self) -> bool {
        matches!(
            self,
            Self::Interactive
                | Self::Focused
                | Self::Selected
                | Self::Warning
                | Self::Destructive
                | Self::Overlay
                | Self::OverlayFocused
                | Self::OverlayDanger
        )
    }

    /// Default border visibility for the recipe.
    #[must_use]
    pub const fn default_bordered(self) -> bool {
        matches!(
            self,
            Self::Raised
                | Self::Overlay
                | Self::OverlayFocused
                | Self::OverlayDanger
                | Self::Interactive
                | Self::Focused
                | Self::Selected
                | Self::Warning
                | Self::Destructive
        )
    }
}

/// Background fill policy (terminal-default / no-color safe).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
#[non_exhaustive]
pub enum SurfaceFill {
    /// Recipe + capability driven (default).
    #[default]
    Auto,
    /// Always use terminal default background (`Color::Reset`).
    TerminalDefault,
    /// Never paint background (transparent over existing buffer cells).
    Transparent,
}

/// Resolved paint plan for one Surface frame (from [`DesignSystem::surface_recipe`]).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SurfacePaintPlan {
    /// Background style when fill is applied; `None` means skip fill.
    pub fill: Option<Style>,
    /// Border style when bordered; `None` means no border paint.
    pub border: Option<Style>,
    /// Horizontal content pad (cells).
    pub pad_x: u16,
    /// Vertical content pad (cells).
    pub pad_y: u16,
    /// Named recipe that produced this plan.
    pub recipe: SurfaceRecipe,
}

/// Named geometry parts for one laid-out Surface (no nested Surfaces).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct SurfaceParts {
    /// Outer allocation (full Surface area).
    pub root: Rect,
    /// Legal paint rect for children (inside border + padding).
    pub content: Rect,
    /// Mouse / hit-test region.
    pub hit: Rect,
    /// Clip rect (same as content; documented alias for hosts).
    pub clip: Rect,
    /// True when a single-line border consumes the outer ring.
    pub has_border: bool,
}

impl SurfaceParts {
    /// True when content has positive area.
    #[must_use]
    pub const fn has_content(self) -> bool {
        self.content.width > 0 && self.content.height > 0
    }

    /// Hit-test a cell.
    #[must_use]
    pub fn contains_hit(self, col: u16, row: u16) -> bool {
        self.hit
            .contains(ratatui_core::layout::Position { x: col, y: row })
    }
}

/// Elevation ladder kept as a thin map onto [`SurfaceRecipe`] for older call sites.
///
/// Prefer [`SurfaceRecipe`] directly. Mapping:
/// - [`SurfaceElevation::Canvas`] → [`SurfaceRecipe::Canvas`]
/// - [`SurfaceElevation::Base`] → [`SurfaceRecipe::Inset`]
/// - [`SurfaceElevation::Elevated`] → [`SurfaceRecipe::Raised`]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
#[non_exhaustive]
pub enum SurfaceElevation {
    /// Default surface ([`SurfaceRecipe::Inset`]).
    #[default]
    Base,
    /// Raised ([`SurfaceRecipe::Raised`]).
    Elevated,
    /// Canvas under panels ([`SurfaceRecipe::Canvas`]).
    Canvas,
}

impl SurfaceElevation {
    /// Maps elevation onto the modern recipe set.
    #[must_use]
    pub const fn recipe(self) -> SurfaceRecipe {
        match self {
            Self::Base => SurfaceRecipe::Inset,
            Self::Elevated => SurfaceRecipe::Raised,
            Self::Canvas => SurfaceRecipe::Canvas,
        }
    }
}

impl From<SurfaceElevation> for SurfaceRecipe {
    fn from(value: SurfaceElevation) -> Self {
        value.recipe()
    }
}

/// Lowest-level visual ownership primitive.
#[derive(Debug, Clone, Copy)]
pub struct Surface<'a> {
    system: &'a DesignSystem,
    recipe: SurfaceRecipe,
    bordered: Option<bool>,
    border_style: Option<Style>,
    fill: SurfaceFill,
    pad_x: Option<u16>,
    pad_y: Option<u16>,
    /// When true (default for interactive), hit uses root; else content.
    hit_full: Option<bool>,
}

impl<'a> Surface<'a> {
    /// Surface with default [`SurfaceRecipe::Inset`].
    #[must_use]
    pub const fn new(system: &'a DesignSystem) -> Self {
        Self {
            system,
            recipe: SurfaceRecipe::Inset,
            bordered: None,
            border_style: None,
            fill: SurfaceFill::Auto,
            pad_x: None,
            pad_y: None,
            hit_full: None,
        }
    }

    /// Recipe.
    #[must_use]
    pub const fn recipe(mut self, recipe: SurfaceRecipe) -> Self {
        self.recipe = recipe;
        self
    }

    /// Elevation ladder (maps onto [`SurfaceRecipe`]).
    #[must_use]
    pub const fn elevation(mut self, elevation: SurfaceElevation) -> Self {
        self.recipe = elevation.recipe();
        self
    }

    /// Force border on/off (overrides recipe default).
    #[must_use]
    pub const fn bordered(mut self, bordered: bool) -> Self {
        self.bordered = Some(bordered);
        self
    }

    /// Override semantic border style while preserving shared geometry.
    #[must_use]
    pub const fn border_style(mut self, style: Style) -> Self {
        self.border_style = Some(style);
        self
    }

    /// Fill policy.
    #[must_use]
    pub const fn fill(mut self, fill: SurfaceFill) -> Self {
        self.fill = fill;
        self
    }

    /// Override padding (cells). Density spacing used when unset.
    #[must_use]
    pub const fn padding(mut self, pad_x: u16, pad_y: u16) -> Self {
        self.pad_x = Some(pad_x);
        self.pad_y = Some(pad_y);
        self
    }

    /// Reserves the border's content inset, and nothing else.
    ///
    /// Bordered chrome floors at one column at every density, so text never
    /// sits flush against a border glyph — the rule that
    /// [`crate::style::DesignSystem::content_inset`] owns. A borderless
    /// surface has no glyph to clear, so it keeps its full rect and lets the
    /// caller's own layout own the spacing. Prefer this over `padding(0, 0)`
    /// on any bordered overlay.
    #[must_use]
    pub const fn content_inset(mut self) -> Self {
        let bordered = match self.bordered {
            Some(bordered) => bordered,
            None => self.recipe.default_bordered(),
        };
        let inset = if bordered {
            self.system.content_inset(true)
        } else {
            crate::style::ContentInset { x: 0, y: 0 }
        };
        self.pad_x = Some(inset.x);
        self.pad_y = Some(inset.y);
        self
    }

    /// Hit region policy: full outer rect vs content only.
    #[must_use]
    pub const fn hit_full(mut self, hit_full: bool) -> Self {
        self.hit_full = Some(hit_full);
        self
    }

    /// Resolve paint plan from design system + overrides.
    #[must_use]
    pub fn plan(&self) -> SurfacePaintPlan {
        let mut plan = self.system.surface_recipe(self.recipe);
        if let Some(x) = self.pad_x {
            plan.pad_x = x;
        }
        if let Some(y) = self.pad_y {
            plan.pad_y = y;
        }
        plan.fill = match self.fill {
            SurfaceFill::Auto => plan.fill,
            SurfaceFill::TerminalDefault => Some(Style::default().bg(Color::Reset)),
            SurfaceFill::Transparent => None,
        };
        let bordered = self.bordered.unwrap_or(self.recipe.default_bordered());
        if !bordered {
            plan.border = None;
        } else if plan.border.is_none() {
            plan.border = Some(self.system.style(Role::Border));
        }
        if bordered && let Some(style) = self.border_style {
            plan.border = Some(style);
        }
        // No-color / monochrome: never rely on chromatic fill alone.
        if matches!(self.system.capability, ColorCapability::Monochrome)
            && matches!(self.fill, SurfaceFill::Auto)
        {
            plan.fill = match self.recipe {
                SurfaceRecipe::Canvas | SurfaceRecipe::Inset | SurfaceRecipe::Sunken => {
                    Some(Style::default().bg(Color::Reset))
                }
                // Keep modifier cues on border; skip solid fill soup.
                _ => None,
            };
            if let Some(border) = plan.border.as_mut()
                && matches!(
                    self.recipe,
                    SurfaceRecipe::Focused
                        | SurfaceRecipe::OverlayFocused
                        | SurfaceRecipe::Selected
                        | SurfaceRecipe::Destructive
                        | SurfaceRecipe::OverlayDanger
                )
            {
                *border = border.add_modifier(Modifier::BOLD);
            }
        }
        plan
    }

    /// Layout named parts without painting.
    #[must_use]
    pub fn layout(&self, area: Rect) -> SurfaceParts {
        let plan = self.plan();
        let has_border = plan.border.is_some();
        let border_cells: u16 = if has_border { 1 } else { 0 };
        let inner = shrink_rect(area, border_cells, border_cells, border_cells, border_cells);
        let content = shrink_rect(inner, plan.pad_x, plan.pad_y, plan.pad_x, plan.pad_y);
        let hit_full = self
            .hit_full
            .unwrap_or(self.recipe.is_interactive() || has_border);
        let hit = if hit_full { area } else { content };
        SurfaceParts {
            root: area,
            content,
            hit,
            clip: content,
            has_border,
        }
    }

    /// Paint fill + optional border; returns content rect for children.
    pub fn paint(&self, area: Rect, buffer: &mut Buffer) -> Rect {
        let plan = self.plan();
        let parts = self.layout(area);
        if area.is_empty() {
            return parts.content;
        }
        if let Some(fill) = plan.fill {
            fill_rect(buffer, area, fill);
        }
        if let Some(border) = plan.border {
            Block::bordered()
                .border_style(border)
                .border_set(self.system.border_set())
                .render(area, buffer);
            // Re-fill content interior so border paint does not leave title gaps;
            // children own content cells.
            if let Some(fill) = plan.fill
                && parts.content.width > 0
                && parts.content.height > 0
            {
                fill_rect(buffer, parts.content, fill);
            }
        }
        parts.content
    }

    /// Hit-test helper after layout.
    #[must_use]
    pub fn hit_test(&self, area: Rect, col: u16, row: u16) -> bool {
        self.layout(area).contains_hit(col, row)
    }
}

impl Widget for &Surface<'_> {
    fn render(self, area: Rect, buffer: &mut Buffer) {
        let _ = self.paint(area, buffer);
    }
}

impl Widget for Surface<'_> {
    fn render(self, area: Rect, buffer: &mut Buffer) {
        let _ = self.paint(area, buffer);
    }
}

fn shrink_rect(area: Rect, left: u16, top: u16, right: u16, bottom: u16) -> Rect {
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

fn fill_rect(buffer: &mut Buffer, area: Rect, style: Style) {
    if area.width == 0 || area.height == 0 {
        return;
    }
    for y in area.y..area.bottom() {
        for x in area.x..area.right() {
            if let Some(cell) = buffer.cell_mut((x, y)) {
                cell.set_style(style);
            }
        }
    }
}

/// Skip empty styles so phosphor terminal-default surfaces do not force Reset soup.
fn nonempty_fill(style: Style) -> Option<Style> {
    if style.bg.is_some() || style.fg.is_some() || style.add_modifier != Modifier::empty() {
        Some(style)
    } else {
        None
    }
}

/// DesignSystem surface recipe resolution.
impl DesignSystem {
    /// Resolves fill, border, and density padding for a [`SurfaceRecipe`].
    #[must_use]
    pub fn surface_recipe(&self, recipe: SurfaceRecipe) -> SurfacePaintPlan {
        let (fill_role, border_role, bordered) = match recipe {
            SurfaceRecipe::Canvas => (None, None, false),
            SurfaceRecipe::Inset => (Some(Role::Surface), None, false),
            SurfaceRecipe::Sunken => (Some(Role::Sunken), None, false),
            // The ladder only reads as a ladder if each rung has its own role:
            // in-flow cards sit on `Raised`, overlays keep `Elevated`.
            SurfaceRecipe::Raised => (Some(Role::Raised), Some(Role::Border), true),
            SurfaceRecipe::Overlay => (Some(Role::Elevated), Some(Role::Border), true),
            SurfaceRecipe::OverlayFocused => {
                (Some(Role::Elevated), Some(Role::BorderFocused), true)
            }
            SurfaceRecipe::OverlayDanger => (Some(Role::Elevated), Some(Role::Danger), true),
            SurfaceRecipe::Interactive => (Some(Role::Surface), Some(Role::Border), true),
            SurfaceRecipe::Focused => (Some(Role::Surface), Some(Role::BorderFocused), true),
            // Membership is a wash, not a slab: `Role::Selection` is a saturated
            // fill meant for one row, never for a whole panel.
            SurfaceRecipe::Selected => (Some(Role::SelectionTint), Some(Role::Border), true),
            SurfaceRecipe::Warning => (Some(Role::Surface), Some(Role::Warning), true),
            SurfaceRecipe::Destructive => (Some(Role::Surface), Some(Role::Danger), true),
        };
        let fill = match recipe {
            SurfaceRecipe::Canvas => Some(Style::default().bg(Color::Reset)),
            _ => fill_role.map(|r| self.style(r)).and_then(nonempty_fill),
        };
        let border = if bordered {
            border_role.map(|r| self.style(r))
        } else {
            None
        };
        SurfacePaintPlan {
            fill,
            border,
            pad_x: self.spacing.pad_x,
            pad_y: self.spacing.pad_y,
            recipe,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::style::{ColorCapability, DesignSystem};

    #[test]
    fn canvas_uses_terminal_default_fill() {
        let system = DesignSystem::default();
        let plan = Surface::new(&system).recipe(SurfaceRecipe::Canvas).plan();
        assert_eq!(plan.fill, Some(Style::default().bg(Color::Reset)));
        assert!(plan.border.is_none());
    }

    #[test]
    fn focused_uses_border_focused_role() {
        let system = DesignSystem::default();
        let focused = system.surface_recipe(SurfaceRecipe::Focused);
        let normal = system.surface_recipe(SurfaceRecipe::Interactive);
        assert_ne!(focused.border, normal.border);
        assert_eq!(focused.border, Some(system.style(Role::BorderFocused)));
    }

    #[test]
    fn layout_content_inside_border_and_pad() {
        let system = DesignSystem::default().density(crate::style::Density::Comfortable);
        let s = Surface::new(&system)
            .recipe(SurfaceRecipe::Raised)
            .padding(1, 1);
        let parts = s.layout(Rect::new(0, 0, 20, 10));
        assert!(parts.has_border);
        assert_eq!(parts.root, Rect::new(0, 0, 20, 10));
        // border 1 + pad 1 each side → content inset by 2
        assert_eq!(parts.content, Rect::new(2, 2, 16, 6));
        assert_eq!(parts.clip, parts.content);
        assert_eq!(parts.hit, parts.root);
    }

    #[test]
    fn inset_hit_is_content_when_not_interactive() {
        let system = DesignSystem::default();
        let s = Surface::new(&system)
            .recipe(SurfaceRecipe::Inset)
            .padding(1, 0);
        let parts = s.layout(Rect::new(0, 0, 10, 5));
        assert!(!parts.has_border);
        assert_eq!(parts.hit, parts.content);
        assert!(parts.contains_hit(parts.content.x, parts.content.y));
    }

    #[test]
    fn transparent_skips_fill() {
        let system = DesignSystem::default();
        let plan = Surface::new(&system)
            .recipe(SurfaceRecipe::Raised)
            .fill(SurfaceFill::Transparent)
            .plan();
        assert!(plan.fill.is_none());
        assert!(plan.border.is_some());
    }

    #[test]
    fn monochrome_avoids_chromatic_fill_soup() {
        let system = DesignSystem::default().capability(ColorCapability::Monochrome);
        let plan = Surface::new(&system).recipe(SurfaceRecipe::Raised).plan();
        assert!(plan.fill.is_none());
        assert!(plan.border.is_some());
    }

    #[test]
    fn elevation_maps_to_recipe() {
        assert_eq!(SurfaceElevation::Base.recipe(), SurfaceRecipe::Inset);
        assert_eq!(SurfaceElevation::Elevated.recipe(), SurfaceRecipe::Raised);
        assert_eq!(SurfaceElevation::Canvas.recipe(), SurfaceRecipe::Canvas);
    }

    #[test]
    fn paint_does_not_panic_on_tiny() {
        let system = DesignSystem::default();
        let mut buf = Buffer::empty(Rect::new(0, 0, 2, 1));
        let content = Surface::new(&system)
            .recipe(SurfaceRecipe::Focused)
            .paint(Rect::new(0, 0, 2, 1), &mut buf);
        // Tiny: content may be empty after border.
        let _ = content;
    }

    #[test]
    fn recipe_ids_stable() {
        assert_eq!(SurfaceRecipe::Focused.id(), "focused");
        assert_eq!(SurfaceRecipe::Destructive.id(), "destructive");
    }

    #[test]
    fn all_recipes_resolve() {
        let system = DesignSystem::default();
        for recipe in [
            SurfaceRecipe::Canvas,
            SurfaceRecipe::Inset,
            SurfaceRecipe::Raised,
            SurfaceRecipe::Overlay,
            SurfaceRecipe::Interactive,
            SurfaceRecipe::Focused,
            SurfaceRecipe::Selected,
            SurfaceRecipe::Warning,
            SurfaceRecipe::Destructive,
        ] {
            let plan = system.surface_recipe(recipe);
            assert_eq!(plan.recipe, recipe);
            let parts = Surface::new(&system)
                .recipe(recipe)
                .layout(Rect::new(0, 0, 40, 12));
            assert_eq!(parts.root.width, 40);
        }
    }

    #[test]
    fn layout_is_cheap() {
        let system = DesignSystem::default();
        let s = Surface::new(&system).recipe(SurfaceRecipe::Focused);
        let area = Rect::new(0, 0, 80, 24);
        for _ in 0..50_000 {
            let _ = s.layout(area);
        }
    }

    #[test]
    fn paint_with_explicit_fill_sets_bg() {
        let system = DesignSystem::from_palette(crate::style::RolePalette::slate());
        let mut buf = Buffer::empty(Rect::new(0, 0, 8, 4));
        Surface::new(&system)
            .recipe(SurfaceRecipe::Raised)
            .bordered(false)
            .padding(0, 0)
            .paint(Rect::new(0, 0, 8, 4), &mut buf);
        let raised = system.style(Role::Raised);
        assert!(raised.bg.is_some(), "slate Raised must carry bg");
        assert_eq!(buf[(3, 1)].style().bg, raised.bg);
    }

    #[test]
    fn phosphor_surface_ladder_is_populated() {
        let system = DesignSystem::default();
        for role in [
            Role::Canvas,
            Role::Surface,
            Role::Raised,
            Role::Elevated,
            Role::Sunken,
            Role::StatusBar,
            Role::SelectionTint,
            Role::HoverTint,
        ] {
            assert!(system.style(role).bg.is_some(), "{role:?} must carry bg");
        }
    }

    #[test]
    fn phosphor_raised_fill_is_painted() {
        let system = DesignSystem::default();
        let plan = system.surface_recipe(SurfaceRecipe::Raised);
        assert_eq!(plan.fill, Some(system.style(Role::Raised)));
        assert!(plan.border.is_some());

        // Overlays own the top rung, and a focused overlay keeps it.
        let overlay = system.surface_recipe(SurfaceRecipe::Overlay);
        let focused_overlay = system.surface_recipe(SurfaceRecipe::OverlayFocused);
        assert_eq!(overlay.fill, Some(system.style(Role::Elevated)));
        assert_eq!(focused_overlay.fill, overlay.fill);
        assert_eq!(
            focused_overlay.border,
            Some(system.style(Role::BorderFocused))
        );
        assert_ne!(plan.fill, overlay.fill);

        // Wells recess: a sunken surface fills below the ordinary surface.
        let sunken = system.surface_recipe(SurfaceRecipe::Sunken);
        assert_eq!(sunken.fill, Some(system.style(Role::Sunken)));
        assert!(sunken.border.is_none());

        // Membership washes; it never slabs the panel with the selection fill.
        let selected = system.surface_recipe(SurfaceRecipe::Selected);
        assert_eq!(selected.fill, Some(system.style(Role::SelectionTint)));
    }
}
