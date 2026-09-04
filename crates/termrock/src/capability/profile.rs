// SPDX-FileCopyrightText: 2026 Alexey Zhokhov
// SPDX-License-Identifier: Apache-2.0

//! Named profiles and capability resolution.
use super::detect::{CapabilitySource, DetectionReport, detect_environment};
use super::set::CapabilitySet;
use crate::style::ColorCapability;

/// Named capability profile.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
#[non_exhaustive]
pub enum CapabilityProfile {
    /// Full modern terminal (Ghostty/Kitty/Wez class baseline).
    #[default]
    Modern,
    /// Safe defaults for multiplexers / SSH / mixed hosts.
    Compatible,
    /// Lowest common denominator (dumb-ish, mono, ascii, minimal IO).
    Minimal,
    /// No alternate screen; scrollback-friendly inline TUI.
    Inline,
    /// No interactive terminal (tests, batch, CI logs).
    Headless,
}

impl CapabilityProfile {
    /// Parse profile name (case-insensitive).
    #[must_use]
    pub fn parse(name: &str) -> Option<Self> {
        match name.trim().to_ascii_lowercase().as_str() {
            "modern" => Some(Self::Modern),
            "compatible" | "compat" => Some(Self::Compatible),
            "minimal" | "min" => Some(Self::Minimal),
            "inline" => Some(Self::Inline),
            "headless" | "none" => Some(Self::Headless),
            _ => None,
        }
    }

    /// Profile id.
    #[must_use]
    pub const fn id(self) -> &'static str {
        match self {
            Self::Modern => "modern",
            Self::Compatible => "compatible",
            Self::Minimal => "minimal",
            Self::Inline => "inline",
            Self::Headless => "headless",
        }
    }

    /// Human description.
    #[must_use]
    pub const fn description(self) -> &'static str {
        match self {
            Self::Modern => "Truecolor Unicode TUI with mouse, paste, alt-screen, optional images",
            Self::Compatible => "256/16 color, unicode, mouse optional, conservative OSC",
            Self::Minimal => "Mono/ASCII, keyboard-only, no alt-screen extras",
            Self::Inline => "Like Compatible but no alternate screen (inline/scrollback)",
            Self::Headless => "No interactive session; paint-to-buffer / tests only",
        }
    }

    /// Baseline set for this profile (before env clamp).
    #[must_use]
    pub const fn baseline(self) -> CapabilitySet {
        match self {
            Self::Modern => CapabilitySet {
                color: ColorCapability::Truecolor,
                mouse: true,
                bracketed_paste: true,
                hyperlinks: true,
                clipboard: true,
                enhanced_keyboard: false, // opt-in; not hard dep
                synchronized_output: false,
                image_protocols: true,
                text_sizing: false,
                alternate_screen: true,
                inline: false,
                keyboard: true,
                multiplexer: false,
                ssh: false,
                windows_conpty: false,
            },
            Self::Compatible => CapabilitySet {
                color: ColorCapability::Indexed256,
                mouse: true,
                bracketed_paste: true,
                hyperlinks: false,
                clipboard: false,
                enhanced_keyboard: false,
                synchronized_output: false,
                image_protocols: false,
                text_sizing: false,
                alternate_screen: true,
                inline: false,
                keyboard: true,
                multiplexer: false,
                ssh: false,
                windows_conpty: false,
            },
            Self::Minimal => CapabilitySet {
                color: ColorCapability::Monochrome,
                mouse: false,
                bracketed_paste: false,
                hyperlinks: false,
                clipboard: false,
                enhanced_keyboard: false,
                synchronized_output: false,
                image_protocols: false,
                text_sizing: false,
                alternate_screen: false,
                inline: true,
                keyboard: true,
                multiplexer: false,
                ssh: false,
                windows_conpty: false,
            },
            Self::Inline => CapabilitySet {
                color: ColorCapability::Indexed256,
                mouse: false,
                bracketed_paste: true,
                hyperlinks: false,
                clipboard: false,
                enhanced_keyboard: false,
                synchronized_output: false,
                image_protocols: false,
                text_sizing: false,
                alternate_screen: false,
                inline: true,
                keyboard: true,
                multiplexer: false,
                ssh: false,
                windows_conpty: false,
            },
            Self::Headless => CapabilitySet {
                color: ColorCapability::Truecolor, // buffer still has styles
                mouse: false,
                bracketed_paste: false,
                hyperlinks: false,
                clipboard: false,
                enhanced_keyboard: false,
                synchronized_output: false,
                image_protocols: false,
                text_sizing: false,
                alternate_screen: false,
                inline: true,
                keyboard: false,
                multiplexer: false,
                ssh: false,
                windows_conpty: false,
            },
        }
    }
}

/// Explicit user/app overrides (triple-state via Option).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub struct CapabilityOverrides {
    /// Force color ladder.
    pub color: Option<ColorCapability>,
    /// Force mouse on/off.
    pub mouse: Option<bool>,
    /// Force bracketed paste.
    pub bracketed_paste: Option<bool>,
    /// Force hyperlinks.
    pub hyperlinks: Option<bool>,
    /// Force clipboard.
    pub clipboard: Option<bool>,
    /// Force enhanced keyboard.
    pub enhanced_keyboard: Option<bool>,
    /// Force image protocols.
    pub image_protocols: Option<bool>,
    /// Force alternate screen.
    pub alternate_screen: Option<bool>,
    /// Force inline mode.
    pub inline: Option<bool>,
    /// Force profile (highest-level override).
    pub profile: Option<CapabilityProfile>,
}

impl CapabilityOverrides {
    /// Parse common env overrides into this struct (does not run full detect).
    #[must_use]
    pub fn from_env_keys(color: Option<&str>, profile: Option<&str>) -> Self {
        let mut o = Self::default();
        if let Some(p) = profile {
            o.profile = CapabilityProfile::parse(p);
        }
        if let Some(c) = color {
            o.color = match c.to_ascii_lowercase().as_str() {
                "truecolor" | "24bit" | "24" => Some(ColorCapability::Truecolor),
                "256" | "256color" => Some(ColorCapability::Indexed256),
                "16" | "ansi" | "ansi16" => Some(ColorCapability::Ansi16),
                "mono" | "monochrome" | "none" | "0" => Some(ColorCapability::Monochrome),
                _ => None,
            };
        }
        o
    }
}

/// Resolved terminal capabilities (detect + profile + overrides).
///
/// Primary public name for progressive enhancement. Also available as the
/// historical alias [`EffectiveCapabilities`].
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TerminalCapabilities {
    /// Active profile.
    pub profile: CapabilityProfile,
    /// Effective set.
    pub set: CapabilitySet,
    /// Detection used.
    pub detection: DetectionReport,
    /// Overrides applied.
    pub overrides: CapabilityOverrides,
    /// Color source.
    pub color_source: CapabilitySource,
    /// Profile source.
    pub profile_source: CapabilitySource,
}

/// Historical name for [`TerminalCapabilities`].
pub type EffectiveCapabilities = TerminalCapabilities;

/// Resolve profile + process env detection + overrides.
///
/// Order: profile baseline → clamp to detected environment → apply overrides.
#[must_use]
pub fn resolve_capabilities(
    preferred_profile: Option<CapabilityProfile>,
    overrides: CapabilityOverrides,
) -> TerminalCapabilities {
    resolve_from_detection(detect_environment(), preferred_profile, overrides)
}

/// Pure resolve from an injected detection report (PTY / unit tests).
#[must_use]
pub fn resolve_from_detection(
    detection: DetectionReport,
    preferred_profile: Option<CapabilityProfile>,
    overrides: CapabilityOverrides,
) -> TerminalCapabilities {
    // Lowest → highest priority: default → auto-env → preferred arg → TERMROCK_PROFILE → overrides.
    let mut profile = CapabilityProfile::Modern;
    let mut profile_source = CapabilitySource::Default;

    if detection.env.term.as_deref() == Some("dumb") {
        profile = CapabilityProfile::Minimal;
        profile_source = CapabilitySource::Environment;
    } else if detection.env.ssh || detection.env.multiplexer.is_some() {
        profile = CapabilityProfile::Compatible;
        profile_source = CapabilitySource::Environment;
    }

    if let Some(p) = preferred_profile {
        profile = p;
        profile_source = CapabilitySource::Default;
    }

    if let Some(raw) = detection.env.profile_override.as_deref()
        && let Some(p) = CapabilityProfile::parse(raw)
    {
        profile = p;
        profile_source = CapabilitySource::Environment;
    }

    if let Some(p) = overrides.profile {
        profile = p;
        profile_source = CapabilitySource::Override;
    }

    let mut set = profile.baseline();
    // Environment facts
    set.multiplexer = detection.env.multiplexer.is_some();
    set.ssh = detection.env.ssh;
    set.windows_conpty = detection.env.windows_conpty;

    // Clamp color to what env can likely do (unless override later).
    let mut color_source = CapabilitySource::Profile;
    let detected = detection.env.color;
    set.color = min_color(set.color, detected);
    if set.color != profile.baseline().color {
        color_source = CapabilitySource::Environment;
    }

    // NO_COLOR always wins unless explicit color override
    if detection.env.no_color {
        set.color = ColorCapability::Monochrome;
        color_source = CapabilitySource::Environment;
    }

    // Apply overrides
    if let Some(c) = overrides.color {
        set.color = c;
        color_source = CapabilitySource::Override;
    } else if let Some(raw) = detection.env.color_override.as_deref() {
        let env_o = CapabilityOverrides::from_env_keys(Some(raw), None);
        if let Some(c) = env_o.color {
            set.color = c;
            color_source = CapabilitySource::Environment;
        }
    }

    if let Some(v) = overrides.mouse {
        set.mouse = v;
    }
    if let Some(v) = overrides.bracketed_paste {
        set.bracketed_paste = v;
    }
    if let Some(v) = overrides.hyperlinks {
        set.hyperlinks = v;
    }
    if let Some(v) = overrides.clipboard {
        set.clipboard = v;
    }
    if let Some(v) = overrides.enhanced_keyboard {
        set.enhanced_keyboard = v;
    }
    if let Some(v) = overrides.image_protocols {
        set.image_protocols = v;
    }
    if let Some(v) = overrides.alternate_screen {
        set.alternate_screen = v;
        set.inline = !v;
    }
    if let Some(v) = overrides.inline {
        set.inline = v;
        if v {
            set.alternate_screen = false;
        }
    }

    // Inline and alt-screen exclusive
    if set.inline {
        set.alternate_screen = false;
    }
    if profile == CapabilityProfile::Headless {
        set.keyboard = false;
        set.mouse = false;
    }

    TerminalCapabilities {
        profile,
        set,
        detection,
        overrides,
        color_source,
        profile_source,
    }
}

impl TerminalCapabilities {
    /// Profile baseline only — no environment (Studio / pure tests).
    #[must_use]
    pub fn for_profile(profile: CapabilityProfile) -> Self {
        resolve_from_detection(
            DetectionReport {
                env: super::detect::EnvHints::default(),
                warnings: Vec::new(),
            },
            Some(profile),
            CapabilityOverrides {
                profile: Some(profile),
                ..CapabilityOverrides::default()
            },
        )
    }

    /// Injected env hints (PTY / deterministic tests).
    #[must_use]
    pub fn with_hints(
        hints: super::detect::EnvHints,
        preferred_profile: Option<CapabilityProfile>,
        overrides: CapabilityOverrides,
    ) -> Self {
        use super::detect::detect_from_hints;
        resolve_from_detection(detect_from_hints(hints), preferred_profile, overrides)
    }

    /// Progressive-enhancement boundary for widgets.
    #[must_use]
    pub const fn boundary(&self) -> super::boundary::CapabilityBoundary {
        super::boundary::CapabilityBoundary::from_capabilities(self)
    }
}

/// Session mode flags derived from capabilities (backend-agnostic).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub struct SessionFlags {
    /// Alternate screen.
    pub alternate_screen: bool,
    /// Mouse capture.
    pub mouse_capture: bool,
    /// Bracketed paste.
    pub bracketed_paste: bool,
    /// Raw mode.
    pub raw_mode: bool,
    /// Hide cursor.
    pub hide_cursor: bool,
    /// Disable line wrap.
    pub disable_line_wrap: bool,
}

#[cfg(feature = "crossterm")]
impl From<SessionFlags> for crate::crossterm::SessionOptions {
    fn from(f: SessionFlags) -> Self {
        Self {
            alternate_screen: f.alternate_screen,
            mouse_capture: f.mouse_capture,
            bracketed_paste: f.bracketed_paste,
            raw_mode: f.raw_mode,
            hide_cursor: f.hide_cursor,
            disable_line_wrap: f.disable_line_wrap,
        }
    }
}

fn min_color(profile: ColorCapability, detected: ColorCapability) -> ColorCapability {
    use ColorCapability::{Ansi16, Indexed256, Monochrome, Truecolor};
    fn rank(c: ColorCapability) -> u8 {
        match c {
            Truecolor => 3,
            Indexed256 => 2,
            Ansi16 => 1,
            Monochrome => 0,
        }
    }
    if rank(detected) < rank(profile) {
        detected
    } else {
        profile
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn modern_baseline_has_truecolor_and_mouse() {
        let b = CapabilityProfile::Modern.baseline();
        assert!(matches!(b.color, ColorCapability::Truecolor));
        assert!(b.mouse && b.alternate_screen);
    }

    #[test]
    fn minimal_is_mono() {
        let b = CapabilityProfile::Minimal.baseline();
        assert!(matches!(b.color, ColorCapability::Monochrome));
        assert!(!b.mouse);
    }

    #[test]
    fn inline_disables_alt_screen() {
        let b = CapabilityProfile::Inline.baseline();
        assert!(b.inline && !b.alternate_screen);
    }

    #[test]
    fn headless_disables_keyboard() {
        let b = CapabilityProfile::Headless.baseline();
        assert!(!b.keyboard);
    }

    #[test]
    fn override_color_wins() {
        let o = CapabilityOverrides {
            color: Some(ColorCapability::Monochrome),
            profile: Some(CapabilityProfile::Modern),
            ..CapabilityOverrides::default()
        };
        let eff = resolve_capabilities(None, o);
        assert!(matches!(eff.set.color, ColorCapability::Monochrome));
        assert_eq!(eff.color_source, CapabilitySource::Override);
    }

    #[test]
    fn parse_profile_names() {
        assert_eq!(
            CapabilityProfile::parse("COMPAT"),
            Some(CapabilityProfile::Compatible)
        );
        assert_eq!(CapabilityProfile::parse("nope"), None);
    }

    #[test]
    fn resolve_never_panics() {
        let _ = resolve_capabilities(
            Some(CapabilityProfile::Compatible),
            CapabilityOverrides::default(),
        );
    }

    #[test]
    fn no_color_fixture_forces_monochrome() {
        use super::super::detect::{EnvHints, detect_from_hints};
        let detection =
            detect_from_hints(EnvHints::fixture("xterm-256color", Some("truecolor"), true));
        let caps = resolve_from_detection(
            detection,
            Some(CapabilityProfile::Modern),
            CapabilityOverrides::default(),
        );
        assert!(matches!(caps.set.color, ColorCapability::Monochrome));
        assert!(caps.boundary().colorless());
    }

    #[test]
    fn ssh_fixture_prefers_compatible_when_no_override() {
        use super::super::detect::{EnvHints, detect_from_hints};
        let detection = detect_from_hints(EnvHints::fixture_ssh_tmux());
        let caps = resolve_from_detection(detection, None, CapabilityOverrides::default());
        assert_eq!(caps.profile, CapabilityProfile::Compatible);
        assert!(caps.set.ssh && caps.set.multiplexer);
    }

    #[test]
    fn all_profiles_roundtrip_for_profile() {
        for p in [
            CapabilityProfile::Modern,
            CapabilityProfile::Compatible,
            CapabilityProfile::Minimal,
            CapabilityProfile::Inline,
            CapabilityProfile::Headless,
        ] {
            let caps = TerminalCapabilities::for_profile(p);
            assert_eq!(caps.profile, p);
            let _ = caps.boundary().session_flags();
        }
    }
}
