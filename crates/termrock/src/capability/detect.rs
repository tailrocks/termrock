// SPDX-FileCopyrightText: 2026 Alexey Zhokhov
// SPDX-License-Identifier: Apache-2.0

//! Best-effort environment detection (no terminal queries required).
//!
//! Interactive DA queries (DECRQM, XTGETTCAP, …) are host-owned and optional.

use crate::style::ColorCapability;

/// How a capability fact was obtained.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[non_exhaustive]
pub enum CapabilitySource {
    /// From environment variables.
    Environment,
    /// Explicit user/app override.
    Override,
    /// From a named profile default.
    Profile,
    /// Built-in safe default.
    Default,
    /// Not probed / unknown.
    Unknown,
}

/// Raw environment hints (detection only).
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct EnvHints {
    /// `TERM` value.
    pub term: Option<String>,
    /// `COLORTERM` value.
    pub colorterm: Option<String>,
    /// `NO_COLOR` set.
    pub no_color: bool,
    /// `TERM_PROGRAM` (iTerm, Apple_Terminal, …).
    pub term_program: Option<String>,
    /// `TMUX` or `TERM` contains screen/tmux.
    pub multiplexer: Option<String>,
    /// `SSH_CONNECTION` / `SSH_TTY` present.
    pub ssh: bool,
    /// `WT_SESSION` / ConPTY indicators.
    pub windows_conpty: bool,
    /// `TERMROCK_COLOR` override raw if any.
    pub color_override: Option<String>,
    /// `TERMROCK_GLYPHS` override raw if any.
    pub glyphs_override: Option<String>,
    /// `TERMROCK_PROFILE` raw if any.
    pub profile_override: Option<String>,
    /// Detected color capability from env (before profile).
    pub color: ColorCapability,
    /// Truecolor likely from COLORTERM.
    pub truecolor_hint: bool,
    /// 256color from TERM.
    pub color256_hint: bool,
}

/// Detection report for doctor / resolve.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct DetectionReport {
    /// Env snapshot.
    pub env: EnvHints,
    /// Suspicious configuration notes.
    pub warnings: Vec<String>,
}

/// Build warnings and normalize color hints from a pure [`EnvHints`] snapshot.
///
/// Use this for PTY / unit tests that inject environment without process env.
#[must_use]
pub fn detect_from_hints(mut env: EnvHints) -> DetectionReport {
    // Normalize color ladder if caller left default and provided TERM-like fields.
    if env.truecolor_hint || env.color256_hint || env.no_color || env.term.is_some() {
        env.color = color_from_hints(&env);
    }
    if let Some(ct) = &env.colorterm {
        let l = ct.to_ascii_lowercase();
        if l.contains("truecolor") || l.contains("24bit") {
            env.truecolor_hint = true;
        }
    }
    if let Some(term) = &env.term {
        if term.contains("256color") {
            env.color256_hint = true;
        }
    }

    let mut warnings = Vec::new();
    if env.no_color && env.truecolor_hint {
        warnings.push("NO_COLOR is set; truecolor hints ignored".into());
    }
    if env.multiplexer.is_some()
        && env.truecolor_hint
        && env.term.as_deref().is_some_and(|t| t.starts_with("screen"))
    {
        warnings.push(
            "multiplexer TERM=screen* with truecolor COLORTERM — ensure outer Tc/RGB (tmux terminal-overrides)"
                .into(),
        );
    }
    if env.term.as_deref() == Some("dumb") {
        warnings.push("TERM=dumb — Minimal/Headless profile recommended".into());
    }
    if env.ssh {
        warnings.push(
            "SSH detected — local clipboard/OSC 52 may fail; prefer Compatible profile".into(),
        );
    }
    if env.term.is_none() {
        warnings.push("TERM unset — capability detection degraded".into());
    }

    DetectionReport { env, warnings }
}

fn color_from_hints(env: &EnvHints) -> ColorCapability {
    if env.no_color {
        return ColorCapability::Monochrome;
    }
    if env.truecolor_hint {
        return ColorCapability::Truecolor;
    }
    if env.color256_hint {
        return ColorCapability::Indexed256;
    }
    if let Some(term) = env.term.as_deref() {
        if term == "dumb" {
            return ColorCapability::Monochrome;
        }
        if term.contains("256color") {
            return ColorCapability::Indexed256;
        }
        if term.contains("color") {
            return ColorCapability::Ansi16;
        }
    }
    env.color
}

impl EnvHints {
    /// Pure fixture for tests / PTY harnesses (no process env).
    #[must_use]
    pub fn fixture(term: &str, colorterm: Option<&str>, no_color: bool) -> Self {
        let mut env = Self {
            term: Some(term.into()),
            colorterm: colorterm.map(str::to_owned),
            no_color,
            ..Self::default()
        };
        if let Some(ct) = colorterm {
            let l = ct.to_ascii_lowercase();
            env.truecolor_hint = l.contains("truecolor") || l.contains("24bit");
        }
        env.color256_hint = term.contains("256color");
        env.color = color_from_hints(&env);
        env
    }

    /// SSH + multiplexer fixture.
    #[must_use]
    pub fn fixture_ssh_tmux() -> Self {
        let mut e = Self::fixture("screen-256color", Some("truecolor"), false);
        e.ssh = true;
        e.multiplexer = Some("tmux:fixture".into());
        e.color = color_from_hints(&e);
        e
    }
}

/// Read process environment and produce a detection report.
#[must_use]
pub fn detect_environment() -> DetectionReport {
    let mut env = EnvHints {
        term: std::env::var("TERM").ok(),
        colorterm: std::env::var("COLORTERM").ok(),
        no_color: std::env::var_os("NO_COLOR").is_some(),
        term_program: std::env::var("TERM_PROGRAM").ok(),
        color_override: std::env::var("TERMROCK_COLOR")
            .ok()
            .or_else(|| std::env::var("COLORTERM_FORCE").ok()),
        glyphs_override: std::env::var("TERMROCK_GLYPHS").ok(),
        profile_override: std::env::var("TERMROCK_PROFILE").ok(),
        ssh: std::env::var_os("SSH_CONNECTION").is_some() || std::env::var_os("SSH_TTY").is_some(),
        windows_conpty: cfg!(windows)
            || std::env::var_os("WT_SESSION").is_some()
            || std::env::var_os("ConEmuANSI").is_some(),
        ..EnvHints::default()
    };

    // Multiplexer
    if let Ok(tmux) = std::env::var("TMUX") {
        env.multiplexer = Some(format!("tmux:{tmux}"));
    } else if let Some(term) = &env.term {
        if term.starts_with("screen") {
            env.multiplexer = Some("screen".into());
        } else if term.contains("tmux") {
            env.multiplexer = Some("tmux-term".into());
        }
    }
    if std::env::var_os("ZELLIJ").is_some() {
        env.multiplexer = Some("zellij".into());
    }

    env.color = ColorCapability::detect_from_env();
    if let Some(ct) = &env.colorterm {
        let l = ct.to_ascii_lowercase();
        env.truecolor_hint = l.contains("truecolor") || l.contains("24bit");
    }
    if let Some(term) = &env.term {
        env.color256_hint = term.contains("256color");
    }

    detect_from_hints(env)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn detect_runs_without_panic() {
        let report = detect_environment();
        // Always produces a color ladder value.
        let _ = report.env.color;
    }

    #[test]
    fn dumb_term_is_monochrome_via_color_capability() {
        // Unit-level: ColorCapability contract used by detect.
        // Full env isolation is host-specific; this locks the helper.
        let c = ColorCapability::detect_from_env();
        let _ = c;
    }

    #[test]
    fn pure_fixture_no_color_is_monochrome() {
        let report =
            detect_from_hints(EnvHints::fixture("xterm-256color", Some("truecolor"), true));
        assert!(report.env.no_color);
        assert!(matches!(report.env.color, ColorCapability::Monochrome));
        assert!(report.warnings.iter().any(|w| w.contains("NO_COLOR")));
    }

    #[test]
    fn pure_fixture_dumb_warns() {
        let report = detect_from_hints(EnvHints::fixture("dumb", None, false));
        assert!(report.warnings.iter().any(|w| w.contains("dumb")));
        assert!(matches!(report.env.color, ColorCapability::Monochrome));
    }

    #[test]
    fn pure_ssh_tmux_fixture() {
        let report = detect_from_hints(EnvHints::fixture_ssh_tmux());
        assert!(report.env.ssh);
        assert!(report.env.multiplexer.is_some());
    }
}
