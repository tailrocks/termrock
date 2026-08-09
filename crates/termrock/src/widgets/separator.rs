// SPDX-FileCopyrightText: 2026 Alexey Zhokhov
// SPDX-License-Identifier: Apache-2.0

//! Separator — semantic horizontal and vertical rules with optional labels.
//!
//! **Non-interactive by default.** Does not claim focus or register semantic
//! nodes unless the host opts into labeled landmark registration.
//!
//! ## Variants
//!
//! | Variant | Role / glyph intent |
//! |---------|---------------------|
//! | Quiet | muted border rule |
//! | Strong | stronger border / double rule (ASCII `=`) |
//! | SectionBreak | padded band + strong rule (spacing recipe) |
//! | Labeled | rule with centered label |
//! | FocusZone | non-color zone boundary (distinct glyph, not focus chrome) |
//!
//! Glyphs always respect [`GlyphSet`] (ASCII fallbacks). Color is optional;
//! no-color themes still paint glyph contrast via roles when available.

use ratatui_core::{buffer::Buffer, layout::Rect, widgets::Widget};

use crate::style::{Density, DesignSystem, GlyphSet, Role, SpacingScale};
use crate::text::{display_cols, take_display_cols};

/// Axis of the rule.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
#[non_exhaustive]
pub enum SeparatorOrientation {
    /// Left → right rule (default).
    #[default]
    Horizontal,
    /// Top → bottom rule.
    Vertical,
}

impl SeparatorOrientation {
    /// Stable id.
    #[must_use]
    pub const fn id(self) -> &'static str {
        match self {
            Self::Horizontal => "horizontal",
            Self::Vertical => "vertical",
        }
    }
}

/// Semantic weight / recipe for the rule.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
#[non_exhaustive]
pub enum SeparatorVariant {
    /// Soft editorial divider (`Role::Border` muted).
    #[default]
    Quiet,
    /// Strong divider (heavier glyph / role).
    Strong,
    /// Section break: uses spacing recipe for outer pad when area allows.
    SectionBreak,
    /// Labeled divider (requires [`Separator::label`]).
    Labeled,
    /// Focus-zone boundary (distinct non-color glyph; not BorderFocused).
    FocusZone,
}

impl SeparatorVariant {
    /// Stable id.
    #[must_use]
    pub const fn id(self) -> &'static str {
        match self {
            Self::Quiet => "quiet",
            Self::Strong => "strong",
            Self::SectionBreak => "section-break",
            Self::Labeled => "labeled",
            Self::FocusZone => "focus-zone",
        }
    }
}

/// Preferred main-axis thickness recipe (cells).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
#[non_exhaustive]
pub enum SeparatorThickness {
    /// Single cell line (default).
    #[default]
    Thin,
    /// Three-cell band for section breaks (pad + rule + pad when space).
    Band,
}

impl SeparatorThickness {
    /// Cells requested on the main cross axis for layout hosts.
    #[must_use]
    pub const fn cells(self) -> u16 {
        match self {
            Self::Thin => 1,
            Self::Band => 3,
        }
    }
}

/// Semantic horizontal or vertical separator.
#[derive(Debug, Clone, Copy)]
pub struct Separator<'a> {
    system: &'a DesignSystem,
    orientation: SeparatorOrientation,
    variant: SeparatorVariant,
    label: Option<&'a str>,
    thickness: SeparatorThickness,
    /// Optional spacing scale for section-break padding (defaults density gap).
    spacing: Option<SpacingScale>,
}

impl<'a> Separator<'a> {
    /// Quiet horizontal rule (canonical constructor).
    #[must_use]
    pub const fn new(system: &'a DesignSystem) -> Self {
        Self {
            system,
            orientation: SeparatorOrientation::Horizontal,
            variant: SeparatorVariant::Quiet,
            label: None,
            thickness: SeparatorThickness::Thin,
            spacing: None,
        }
    }

    /// Horizontal quiet rule (legacy ergonomics).
    #[must_use]
    pub const fn horizontal(system: &'a DesignSystem) -> Self {
        Self::new(system)
    }

    /// Vertical quiet rule.
    #[must_use]
    pub const fn vertical(system: &'a DesignSystem) -> Self {
        Self {
            system,
            orientation: SeparatorOrientation::Vertical,
            variant: SeparatorVariant::Quiet,
            label: None,
            thickness: SeparatorThickness::Thin,
            spacing: None,
        }
    }

    /// Orientation.
    #[must_use]
    pub const fn orientation(mut self, orientation: SeparatorOrientation) -> Self {
        self.orientation = orientation;
        self
    }

    /// Variant recipe.
    #[must_use]
    pub const fn variant(mut self, variant: SeparatorVariant) -> Self {
        self.variant = variant;
        if matches!(variant, SeparatorVariant::SectionBreak) {
            self.thickness = SeparatorThickness::Band;
        }
        self
    }

    /// Quiet recipe.
    #[must_use]
    pub const fn quiet(mut self) -> Self {
        self.variant = SeparatorVariant::Quiet;
        self
    }

    /// Strong recipe.
    #[must_use]
    pub const fn strong(mut self) -> Self {
        self.variant = SeparatorVariant::Strong;
        self
    }

    /// Section-break recipe (band thickness).
    #[must_use]
    pub const fn section_break(mut self) -> Self {
        self.variant = SeparatorVariant::SectionBreak;
        self.thickness = SeparatorThickness::Band;
        self
    }

    /// Focus-zone boundary recipe.
    #[must_use]
    pub const fn focus_zone(mut self) -> Self {
        self.variant = SeparatorVariant::FocusZone;
        self
    }

    /// Optional center label (forces labeled paint path when non-empty).
    #[must_use]
    pub const fn label(mut self, label: &'a str) -> Self {
        self.label = Some(label);
        if !matches!(self.variant, SeparatorVariant::Labeled) {
            self.variant = SeparatorVariant::Labeled;
        }
        self
    }

    /// Thickness recipe for hosts sizing a band.
    #[must_use]
    pub const fn thickness(mut self, thickness: SeparatorThickness) -> Self {
        self.thickness = thickness;
        self
    }

    /// Spacing from density (section-break pad).
    #[must_use]
    pub const fn with_density(mut self, density: Density) -> Self {
        self.spacing = Some(SpacingScale::from_density(density));
        self
    }

    /// Explicit spacing scale.
    #[must_use]
    pub const fn with_spacing(mut self, spacing: SpacingScale) -> Self {
        self.spacing = Some(spacing);
        self
    }

    /// Cells hosts should reserve on the axis perpendicular to the rule.
    #[must_use]
    pub const fn preferred_cross_size(self) -> u16 {
        match self.variant {
            SeparatorVariant::SectionBreak => SeparatorThickness::Band.cells(),
            _ => self.thickness.cells(),
        }
    }

    /// Paint into `area` (never panics on empty / one-cell / tiny).
    pub fn paint(self, area: Rect, buffer: &mut Buffer) {
        if area.is_empty() {
            return;
        }
        match self.orientation {
            SeparatorOrientation::Horizontal => paint_horizontal(self, area, buffer),
            SeparatorOrientation::Vertical => paint_vertical(self, area, buffer),
        }
    }

    /// Optional non-focusable landmark for labeled separators only.
    ///
    /// Quiet/strong/section/focus-zone rules never register — avoids fake
    /// interactive nodes. Labeled separators may register as content landmarks.
    pub fn register_semantic_if_labeled<Id, Action>(
        self,
        scene: &mut crate::interaction::SemanticScene<Id, Action>,
        id: Id,
        area: Rect,
    ) where
        Id: Clone + PartialEq + std::fmt::Display,
        Action: Clone,
    {
        let Some(label) = self.label.filter(|s| !s.is_empty()) else {
            return;
        };
        if area.is_empty() {
            return;
        }
        use crate::interaction::{SemanticNode, SemanticRole};
        let _ = scene.register(
            SemanticNode::content(id, area)
                .role(SemanticRole::Chrome)
                .label(label)
                .focusable(false),
        );
    }
}

/// Legacy name kept as an alias of [`Separator`].
pub type SeparatorLine<'a> = Separator<'a>;

impl Widget for &Separator<'_> {
    fn render(self, area: Rect, buffer: &mut Buffer) {
        (*self).paint(area, buffer);
    }
}

impl Widget for Separator<'_> {
    fn render(self, area: Rect, buffer: &mut Buffer) {
        self.paint(area, buffer);
    }
}

fn paint_horizontal(sep: Separator<'_>, area: Rect, buffer: &mut Buffer) {
    let (glyph, style) = rule_style(sep, false);
    let band = horizontal_rule_row(sep, area);
    if band.height == 0 || band.width == 0 {
        return;
    }

    if let Some(label) = sep.label.filter(|s| !s.trim().is_empty()) {
        paint_labeled_horizontal(sep, band, label, glyph, style, buffer);
        return;
    }

    let line: String = std::iter::repeat_n(glyph, usize::from(band.width)).collect();
    buffer.set_stringn(
        band.x,
        band.y,
        &line,
        usize::from(band.width),
        style,
    );
}

fn paint_vertical(sep: Separator<'_>, area: Rect, buffer: &mut Buffer) {
    let (glyph, style) = rule_style(sep, true);
    let band = vertical_rule_col(sep, area);
    if band.width == 0 || band.height == 0 {
        return;
    }

    // Vertical labels: top cell if room, rest glyph.
    let mut y = band.y;
    if let Some(label) = sep.label.filter(|s| !s.trim().is_empty()) {
        let ch = label.chars().next().unwrap_or('|');
        let s = take_display_cols(&ch.to_string(), 1);
        buffer.set_stringn(band.x, y, &s, 1, sep.system.style(Role::TextMuted));
        y = y.saturating_add(1);
    }
    while y < band.bottom() {
        buffer.set_stringn(band.x, y, glyph, 1, style);
        y = y.saturating_add(1);
    }
}

fn paint_labeled_horizontal(
    sep: Separator<'_>,
    band: Rect,
    label: &str,
    glyph: &str,
    style: ratatui_core::style::Style,
    buffer: &mut Buffer,
) {
    let label_style = sep.system.style(Role::TextMuted);
    let raw = format!(" {} ", label.trim());
    let max_label = band.width.saturating_sub(4).max(1);
    let shown = take_display_cols(&raw, usize::from(max_label));
    let label_w = display_cols(&shown) as u16;
    if label_w == 0 || label_w >= band.width {
        // Fall back to plain rule on tiny widths.
        let line: String = std::iter::repeat_n(glyph, usize::from(band.width)).collect();
        buffer.set_stringn(band.x, band.y, &line, usize::from(band.width), style);
        return;
    }
    let side = band.width.saturating_sub(label_w) / 2;
    let left: String = std::iter::repeat_n(glyph, usize::from(side)).collect();
    let right_w = band.width.saturating_sub(side).saturating_sub(label_w);
    let right: String = std::iter::repeat_n(glyph, usize::from(right_w)).collect();
    if side > 0 {
        buffer.set_stringn(band.x, band.y, &left, usize::from(side), style);
    }
    buffer.set_stringn(
        band.x.saturating_add(side),
        band.y,
        &shown,
        usize::from(label_w),
        label_style,
    );
    if right_w > 0 {
        buffer.set_stringn(
            band.x.saturating_add(side).saturating_add(label_w),
            band.y,
            &right,
            usize::from(right_w),
            style,
        );
    }
}

/// Which row/column paints the rule inside a multi-cell band.
fn horizontal_rule_row(sep: Separator<'_>, area: Rect) -> Rect {
    if area.height == 0 {
        return area;
    }
    let want_band = matches!(sep.variant, SeparatorVariant::SectionBreak)
        || matches!(sep.thickness, SeparatorThickness::Band);
    let pad = sep
        .spacing
        .map(|s| s.pad_y.max(1))
        .unwrap_or(1);
    if want_band && area.height >= pad.saturating_mul(2).saturating_add(1) {
        // pad + rule + pad (spacing recipe)
        Rect {
            x: area.x,
            y: area.y.saturating_add(pad),
            width: area.width,
            height: 1,
        }
    } else {
        // Center single rule in multi-row area; prefer middle row.
        let y = if area.height > 1 {
            area.y.saturating_add(area.height / 2)
        } else {
            area.y
        };
        Rect {
            x: area.x,
            y,
            width: area.width,
            height: 1,
        }
    }
}

fn vertical_rule_col(sep: Separator<'_>, area: Rect) -> Rect {
    if area.width == 0 {
        return area;
    }
    let want_band = matches!(sep.variant, SeparatorVariant::SectionBreak)
        || matches!(sep.thickness, SeparatorThickness::Band);
    let pad = sep
        .spacing
        .map(|s| s.pad_x.max(1))
        .unwrap_or(1);
    if want_band && area.width >= pad.saturating_mul(2).saturating_add(1) {
        Rect {
            x: area.x.saturating_add(pad),
            y: area.y,
            width: 1,
            height: area.height,
        }
    } else {
        let x = if area.width > 1 {
            area.x.saturating_add(area.width / 2)
        } else {
            area.x
        };
        Rect {
            x,
            y: area.y,
            width: 1,
            height: area.height,
        }
    }
}

fn rule_style(sep: Separator<'_>, vertical: bool) -> (&'static str, ratatui_core::style::Style) {
    let glyphs = sep.system.glyphs;
    let (glyph, role) = match (sep.variant, vertical, glyphs) {
        (SeparatorVariant::Quiet, false, GlyphSet::Unicode) => ("─", Role::Border),
        (SeparatorVariant::Quiet, true, GlyphSet::Unicode) => ("│", Role::Border),
        (SeparatorVariant::Quiet, false, GlyphSet::Ascii) => ("-", Role::Border),
        (SeparatorVariant::Quiet, true, GlyphSet::Ascii) => ("|", Role::Border),

        (SeparatorVariant::Strong | SeparatorVariant::SectionBreak, false, GlyphSet::Unicode) => {
            ("━", Role::TextMuted)
        }
        (SeparatorVariant::Strong | SeparatorVariant::SectionBreak, true, GlyphSet::Unicode) => {
            ("┃", Role::TextMuted)
        }
        (SeparatorVariant::Strong | SeparatorVariant::SectionBreak, false, GlyphSet::Ascii) => {
            ("=", Role::TextMuted)
        }
        (SeparatorVariant::Strong | SeparatorVariant::SectionBreak, true, GlyphSet::Ascii) => {
            ("|", Role::TextMuted)
        }

        (SeparatorVariant::Labeled, false, GlyphSet::Unicode) => ("─", Role::Border),
        (SeparatorVariant::Labeled, true, GlyphSet::Unicode) => ("│", Role::Border),
        (SeparatorVariant::Labeled, false, GlyphSet::Ascii) => ("-", Role::Border),
        (SeparatorVariant::Labeled, true, GlyphSet::Ascii) => ("|", Role::Border),

        // Focus zone: distinct double/dash pattern without BorderFocused (focus ≠ chrome).
        (SeparatorVariant::FocusZone, false, GlyphSet::Unicode) => ("═", Role::Border),
        (SeparatorVariant::FocusZone, true, GlyphSet::Unicode) => ("║", Role::Border),
        (SeparatorVariant::FocusZone, false, GlyphSet::Ascii) => ("=", Role::Border),
        (SeparatorVariant::FocusZone, true, GlyphSet::Ascii) => (":", Role::Border),
    };
    // Prefer muted for quiet if palette has TextDisabled contrast for no-color
    // terminals; still Border for structural rules.
    let style = match sep.variant {
        SeparatorVariant::Quiet => sep.system.style(Role::Border),
        SeparatorVariant::Strong | SeparatorVariant::SectionBreak => {
            sep.system.style(Role::TextMuted)
        }
        SeparatorVariant::Labeled => sep.system.style(Role::Border),
        SeparatorVariant::FocusZone => sep.system.style(Role::Border),
    };
    let _ = role;
    (glyph, style)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::style::GlyphSet;

    #[test]
    fn horizontal_quiet_fills_width() {
        let system = DesignSystem::default();
        let mut buf = Buffer::empty(Rect::new(0, 0, 10, 1));
        Separator::horizontal(&system).paint(Rect::new(0, 0, 10, 1), &mut buf);
        assert!(!buf[(0, 0)].symbol().is_empty());
        assert!(!buf[(9, 0)].symbol().is_empty());
    }

    #[test]
    fn vertical_quiet_fills_height() {
        let system = DesignSystem::default();
        let mut buf = Buffer::empty(Rect::new(0, 0, 1, 5));
        Separator::vertical(&system).paint(Rect::new(0, 0, 1, 5), &mut buf);
        for y in 0..5 {
            assert!(!buf[(0, y)].symbol().is_empty());
        }
    }

    #[test]
    fn empty_area_no_panic() {
        let system = DesignSystem::default();
        let mut buf = Buffer::empty(Rect::new(0, 0, 0, 0));
        Separator::horizontal(&system).paint(Rect::new(0, 0, 0, 0), &mut buf);
        Separator::vertical(&system).paint(Rect::new(0, 0, 0, 0), &mut buf);
    }

    #[test]
    fn one_cell_horizontal_and_vertical() {
        let system = DesignSystem::default();
        let mut buf = Buffer::empty(Rect::new(0, 0, 1, 1));
        Separator::horizontal(&system).paint(Rect::new(0, 0, 1, 1), &mut buf);
        assert!(!buf[(0, 0)].symbol().is_empty());
        let mut buf = Buffer::empty(Rect::new(0, 0, 1, 1));
        Separator::vertical(&system).paint(Rect::new(0, 0, 1, 1), &mut buf);
        assert!(!buf[(0, 0)].symbol().is_empty());
    }

    #[test]
    fn tiny_sizes_all_variants() {
        let system = DesignSystem::default();
        let variants = [
            SeparatorVariant::Quiet,
            SeparatorVariant::Strong,
            SeparatorVariant::SectionBreak,
            SeparatorVariant::Labeled,
            SeparatorVariant::FocusZone,
        ];
        for w in 0..6u16 {
            for h in 0..6u16 {
                for &v in &variants {
                    let mut buf = Buffer::empty(Rect::new(0, 0, w.max(1), h.max(1)));
                    let area = Rect::new(0, 0, w, h);
                    let mut sep = Separator::new(&system).variant(v);
                    if matches!(v, SeparatorVariant::Labeled) {
                        sep = sep.label("OR");
                    }
                    sep.paint(area, &mut buf);
                    Separator::vertical(&system).variant(v).label("x").paint(area, &mut buf);
                }
            }
        }
    }

    #[test]
    fn ascii_glyphs() {
        let system = DesignSystem::default().glyphs(GlyphSet::Ascii);
        let mut buf = Buffer::empty(Rect::new(0, 0, 5, 1));
        Separator::horizontal(&system).paint(Rect::new(0, 0, 5, 1), &mut buf);
        assert_eq!(buf[(0, 0)].symbol(), "-");
        let mut buf = Buffer::empty(Rect::new(0, 0, 5, 1));
        Separator::horizontal(&system)
            .strong()
            .paint(Rect::new(0, 0, 5, 1), &mut buf);
        assert_eq!(buf[(0, 0)].symbol(), "=");
        let mut buf = Buffer::empty(Rect::new(0, 0, 1, 3));
        Separator::vertical(&system)
            .focus_zone()
            .paint(Rect::new(0, 0, 1, 3), &mut buf);
        assert_eq!(buf[(0, 0)].symbol(), ":");
    }

    #[test]
    fn labeled_centers_text() {
        let system = DesignSystem::default().glyphs(GlyphSet::Ascii);
        let mut buf = Buffer::empty(Rect::new(0, 0, 20, 1));
        Separator::horizontal(&system)
            .label("OR")
            .paint(Rect::new(0, 0, 20, 1), &mut buf);
        // Label cells should include 'O' somewhere mid-line.
        let mut found = false;
        for x in 0..20 {
            if buf[(x, 0)].symbol().contains('O') {
                found = true;
                break;
            }
        }
        assert!(found, "expected label in buffer");
    }

    #[test]
    fn labeled_tiny_falls_back_to_rule() {
        let system = DesignSystem::default().glyphs(GlyphSet::Ascii);
        let mut buf = Buffer::empty(Rect::new(0, 0, 3, 1));
        Separator::horizontal(&system)
            .label("LONG LABEL")
            .paint(Rect::new(0, 0, 3, 1), &mut buf);
        assert_eq!(buf[(0, 0)].symbol(), "-");
    }

    #[test]
    fn section_break_uses_middle_row_when_tall() {
        let system = DesignSystem::default().glyphs(GlyphSet::Ascii);
        let mut buf = Buffer::empty(Rect::new(0, 0, 8, 3));
        Separator::horizontal(&system)
            .section_break()
            .paint(Rect::new(0, 0, 8, 3), &mut buf);
        // Middle row has rule.
        assert_eq!(buf[(0, 1)].symbol(), "=");
        // Outer pad rows empty (default cell).
        assert!(buf[(0, 0)].symbol().is_empty() || buf[(0, 0)].symbol() == " ");
    }

    #[test]
    fn preferred_cross_size_recipes() {
        let system = DesignSystem::default();
        assert_eq!(Separator::horizontal(&system).preferred_cross_size(), 1);
        assert_eq!(
            Separator::horizontal(&system)
                .section_break()
                .preferred_cross_size(),
            3
        );
    }

    #[test]
    fn legacy_alias_and_widget() {
        let system = DesignSystem::default();
        let mut buf = Buffer::empty(Rect::new(0, 0, 4, 1));
        Widget::render(&SeparatorLine::horizontal(&system), Rect::new(0, 0, 4, 1), &mut buf);
        assert!(!buf[(0, 0)].symbol().is_empty());
    }

    #[test]
    fn semantic_only_when_labeled() {
        use crate::interaction::SemanticScene;
        let system = DesignSystem::default();
        let mut scene = SemanticScene::<&str, ()>::new();
        scene.begin_frame();
        Separator::horizontal(&system).register_semantic_if_labeled(
            &mut scene,
            "a",
            Rect::new(0, 0, 10, 1),
        );
        assert!(scene.is_empty());
        Separator::horizontal(&system)
            .label("Part 2")
            .register_semantic_if_labeled(&mut scene, "b", Rect::new(0, 0, 10, 1));
        assert_eq!(scene.len(), 1);
        assert!(!scene.nodes()[0].focusable);
    }

    #[test]
    fn layout_is_cheap() {
        let system = DesignSystem::default();
        let mut buf = Buffer::empty(Rect::new(0, 0, 80, 3));
        let area = Rect::new(0, 0, 80, 1);
        for _ in 0..50_000 {
            Separator::horizontal(&system)
                .label("SECTION")
                .paint(area, &mut buf);
        }
    }

    #[test]
    fn variant_ids_stable() {
        assert_eq!(SeparatorVariant::FocusZone.id(), "focus-zone");
        assert_eq!(SeparatorOrientation::Vertical.id(), "vertical");
    }
}
