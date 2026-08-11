// SPDX-FileCopyrightText: 2026 Alexey Zhokhov
// SPDX-License-Identifier: Apache-2.0

//! Pure capability “PTY harness” tests — inject env without a real PTY.
//!
//! These lock progressive enhancement: NO_COLOR, profiles, SSH/tmux, and the
//! [`CapabilityBoundary`] contract used by widgets.

use termrock::capability::{
    CapabilityBoundary, CapabilityKind, CapabilityOverrides, CapabilityProfile, EnvHints,
    TerminalCapabilities, detect_from_hints, resolve_from_detection,
};
use termrock::style::ColorCapability;

#[test]
fn no_color_forces_monochrome_boundary() {
    let hints = EnvHints::fixture("xterm-256color", Some("truecolor"), true);
    let caps = TerminalCapabilities::with_hints(
        hints,
        Some(CapabilityProfile::Modern),
        CapabilityOverrides::default(),
    );
    assert!(matches!(caps.set.color, ColorCapability::Monochrome));
    let b = caps.boundary();
    assert!(b.colorless());
    assert!(!b.enabled(CapabilityKind::Truecolor));
}

#[test]
fn dumb_term_resolves_minimal_without_preferred() {
    let detection = detect_from_hints(EnvHints::fixture("dumb", None, false));
    let caps = resolve_from_detection(detection, None, CapabilityOverrides::default());
    assert_eq!(caps.profile, CapabilityProfile::Minimal);
    assert!(caps.boundary().ascii_glyphs());
}

#[test]
fn ssh_tmux_prefers_compatible() {
    let caps = TerminalCapabilities::with_hints(
        EnvHints::fixture_ssh_tmux(),
        None,
        CapabilityOverrides::default(),
    );
    assert_eq!(caps.profile, CapabilityProfile::Compatible);
    assert!(caps.set.ssh && caps.set.multiplexer);
}

#[test]
fn profile_override_beats_ssh_auto() {
    let caps = TerminalCapabilities::with_hints(
        EnvHints::fixture_ssh_tmux(),
        None,
        CapabilityOverrides {
            profile: Some(CapabilityProfile::Modern),
            ..CapabilityOverrides::default()
        },
    );
    assert_eq!(caps.profile, CapabilityProfile::Modern);
}

#[test]
fn all_profiles_have_session_and_boundary() {
    for p in [
        CapabilityProfile::Modern,
        CapabilityProfile::Compatible,
        CapabilityProfile::Minimal,
        CapabilityProfile::Inline,
        CapabilityProfile::Headless,
    ] {
        let caps = TerminalCapabilities::for_profile(p);
        let b = CapabilityBoundary::from_capabilities(&caps);
        let flags = b.session_flags();
        if p == CapabilityProfile::Headless {
            assert!(!flags.raw_mode);
        }
        if p == CapabilityProfile::Inline {
            assert!(!flags.alternate_screen);
            assert!(caps.set.inline);
        }
        for kind in CapabilityKind::ALL {
            let _ = b.fallback(kind);
        }
    }
}

#[test]
fn color_override_wins_over_no_color_env_when_explicit() {
    // Explicit override is the only way to force color under NO_COLOR.
    let hints = EnvHints::fixture("xterm", Some("truecolor"), true);
    let caps = TerminalCapabilities::with_hints(
        hints,
        Some(CapabilityProfile::Modern),
        CapabilityOverrides {
            color: Some(ColorCapability::Ansi16),
            ..CapabilityOverrides::default()
        },
    );
    assert!(matches!(caps.set.color, ColorCapability::Ansi16));
}
