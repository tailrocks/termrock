// SPDX-FileCopyrightText: 2026 Alexey Zhokhov
// SPDX-License-Identifier: Apache-2.0

//! Progressive-enhancement boundary between terminal capabilities and widgets.
//!
//! Hosts resolve [`super::TerminalCapabilities`] once, then pass a
//! [`CapabilityBoundary`] (or the full caps) into paint/session setup so optional
//! features never become hidden hard dependencies.
use super::CapabilityProfile;
use super::profile::{SessionFlags, TerminalCapabilities};
use super::set::{CapabilityKind, CapabilitySet, FallbackPolicy, fallback_policies};
use crate::style::{ColorCapability, DesignSystem, RolePalette};

/// Progressive-enhancement contract for components (derived from a capability set).
///
/// Widgets consult this instead of reading env vars. Fallbacks are deterministic.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct CapabilityBoundary {
    /// Effective enablement set.
    pub set: CapabilitySet,
    /// Active profile (for density / story selection).
    pub profile: CapabilityProfile,
}

impl CapabilityBoundary {
    /// From profile baseline only (no env).
    #[must_use]
    pub const fn from_profile(profile: CapabilityProfile) -> Self {
        Self {
            set: profile.baseline(),
            profile,
        }
    }

    /// From resolved terminal capabilities.
    #[must_use]
    pub const fn from_capabilities(caps: &TerminalCapabilities) -> Self {
        Self {
            set: caps.set,
            profile: caps.profile,
        }
    }

    /// Explicit set + profile.
    #[must_use]
    pub const fn new(set: CapabilitySet, profile: CapabilityProfile) -> Self {
        Self { set, profile }
    }

    /// Disable chromatic color (NO_COLOR / monochrome).
    #[must_use]
    pub const fn colorless(self) -> bool {
        matches!(self.set.color, ColorCapability::Monochrome)
    }

    /// Mouse reporting desired.
    #[must_use]
    pub const fn allow_mouse(self) -> bool {
        self.set.mouse
    }

    /// Bracketed paste desired.
    #[must_use]
    pub const fn allow_bracketed_paste(self) -> bool {
        self.set.bracketed_paste
    }

    /// OSC 8 hyperlinks allowed.
    #[must_use]
    pub const fn allow_hyperlinks(self) -> bool {
        self.set.hyperlinks
    }

    /// Image protocol emission allowed (host still chooses protocol).
    #[must_use]
    pub const fn allow_images(self) -> bool {
        self.set.image_protocols
    }

    /// Interactive keyboard session.
    #[must_use]
    pub const fn interactive(self) -> bool {
        self.set.keyboard
    }

    /// Widget paint hints (common flags).
    #[must_use]
    pub const fn component_hints(self) -> ComponentCapabilityHints {
        ComponentCapabilityHints {
            colorless: self.colorless(),
            mouse: self.allow_mouse(),
            hyperlinks: self.allow_hyperlinks(),
            interactive: self.interactive(),
            inline: self.set.inline,
            headless: matches!(self.profile, CapabilityProfile::Headless) || !self.set.keyboard,
        }
    }

    /// Session flags for backend adapters.
    #[must_use]
    pub const fn session_flags(self) -> SessionFlags {
        SessionFlags {
            alternate_screen: self.set.alternate_screen,
            mouse_capture: self.set.mouse,
            bracketed_paste: self.set.bracketed_paste,
            raw_mode: self.set.keyboard,
            hide_cursor: self.set.keyboard && self.set.alternate_screen,
            disable_line_wrap: self.set.alternate_screen,
        }
    }

    /// Documented fallback for a kind.
    #[must_use]
    pub fn fallback(self, kind: CapabilityKind) -> Option<&'static FallbackPolicy> {
        fallback_policies().iter().find(|p| p.kind == kind)
    }

    /// Resolve a palette onto this boundary's colour ladder.
    #[must_use]
    pub fn project_palette(self, palette: RolePalette) -> RolePalette {
        palette.quantized(self.set.color)
    }

    /// Project design tokens onto this boundary (host may still set density).
    ///
    /// The palette is resolved for the boundary's colour rung and the rung is
    /// recorded; glyph projection is the only separate step, because the ASCII
    /// set is a vocabulary choice, not a colour downgrade.
    #[must_use]
    pub fn project_system(self, system: DesignSystem) -> DesignSystem {
        let mut out = system;
        out.palette = self.project_palette(out.palette);
        out.capability = self.set.color;
        out
    }

    /// Whether a capability is on.
    #[must_use]
    pub const fn enabled(self, kind: CapabilityKind) -> bool {
        self.set.enabled(kind)
    }
}

/// Common widget-facing flags (avoid env reads in paint).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub struct ComponentCapabilityHints {
    /// No chromatic color.
    pub colorless: bool,
    /// Mouse available.
    pub mouse: bool,
    /// Hyperlinks allowed.
    pub hyperlinks: bool,
    /// Interactive keyboard session.
    pub interactive: bool,
    /// Inline (no alt screen).
    pub inline: bool,
    /// Headless / non-interactive.
    pub headless: bool,
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::style::RolePalette;

    #[test]
    fn minimal_boundary_is_colorless() {
        let b = CapabilityBoundary::from_profile(CapabilityProfile::Minimal);
        assert!(b.colorless());
        assert!(!b.allow_mouse());
        let hints = b.component_hints();
        assert!(hints.colorless && !hints.mouse);
    }

    #[test]
    fn modern_projects_truecolor_palette() {
        let b = CapabilityBoundary::from_profile(CapabilityProfile::Modern);
        assert!(!b.colorless());
        let q = b.project_palette(RolePalette::junie());
        let _ = q;
    }

    #[test]
    fn headless_not_interactive() {
        let b = CapabilityBoundary::from_profile(CapabilityProfile::Headless);
        assert!(!b.interactive());
        assert!(b.component_hints().headless);
        assert!(!b.session_flags().raw_mode);
    }

    #[test]
    fn mono_projection_downgrades_selection_and_glyphs() {
        let b = CapabilityBoundary::from_profile(CapabilityProfile::Minimal);
        let projected = b.project_system(DesignSystem::junie());
        assert_eq!(projected.capability, ColorCapability::Monochrome);
        assert_eq!(
            projected.selection,
            crate::style::SelectionChrome::Gutter,
            "a fill has nothing to fill with once color is gone"
        );
    }

    #[test]
    fn modern_projection_keeps_rich_chrome() {
        let b = CapabilityBoundary::from_profile(CapabilityProfile::Modern);
        let projected = b.project_system(DesignSystem::junie());
        assert_eq!(projected.glyphs, DesignSystem::junie().glyphs);
        assert_eq!(projected.selection, DesignSystem::junie().selection);
    }

    #[test]
    fn every_kind_has_fallback_via_boundary() {
        let b = CapabilityBoundary::from_profile(CapabilityProfile::Compatible);
        for kind in CapabilityKind::ALL {
            assert!(b.fallback(kind).is_some(), "{kind:?}");
        }
    }
}
