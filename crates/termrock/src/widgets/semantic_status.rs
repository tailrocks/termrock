// SPDX-FileCopyrightText: 2026 Alexey Zhokhov
// SPDX-License-Identifier: Apache-2.0

//! Shared **SemanticStatus** vocabulary (leaf module — no domain enum deps).
//!
//! Domain widgets map into this type; do not invent private status glyph sets.
//! See [`super::StatusIndicator`] for compact/labeled/elapsed paint.

use crate::style::{GlyphSet, HEARTBEAT_PERIOD_MS, MotionChannel, Role};

/// Canonical status kind for TermRock surfaces.
///
/// Prefer mapping domain enums into this type rather than painting ad-hoc glyphs.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
#[non_exhaustive]
pub enum SemanticStatus {
    /// Reachable / connected.
    Online,
    /// Disconnected / unreachable.
    Offline,
    /// Idle / idle presence (not offline).
    Idle,
    /// Queued / pending start.
    Queued,
    /// Actively working.
    Running,
    /// Blocked waiting (input, lock, network).
    Waiting,
    /// Successful outcome.
    Success,
    /// Caution / degraded.
    Warning,
    /// Failure.
    Failed,
    /// Paused by user/host.
    Paused,
    /// Unknown / indeterminate.
    #[default]
    Unknown,
}

impl SemanticStatus {
    /// What this status is *saying*, in the shared motion vocabulary.
    ///
    /// Terminal states — success, failure, offline, paused, queued — map to
    /// [`MotionChannel::Static`] on purpose. Gravity: a finished thing must not
    /// keep moving, and a state that costs frames forever is a bug, not polish.
    #[must_use]
    pub const fn channel(self) -> MotionChannel {
        match self {
            // Alive and working: a slow breathe says "still here", where a spin
            // would claim a rate this status does not know.
            Self::Running => MotionChannel::Live,
            // Blocked on something external: a quicker pulse, no progress claim.
            Self::Waiting => MotionChannel::Wait,
            // Presence, not progress — see `period_ms`, which slows it further.
            Self::Online => MotionChannel::Live,
            Self::Offline
            | Self::Idle
            | Self::Queued
            | Self::Success
            | Self::Warning
            | Self::Failed
            | Self::Paused
            | Self::Unknown => MotionChannel::Static,
        }
    }

    /// Loop period for this status (`0` when it does not animate).
    ///
    /// `Online` breathes on the [`HEARTBEAT_PERIOD_MS`] heartbeat rather than
    /// the channel's 2 s: presence should be barely perceptible, and anything
    /// faster reads as activity.
    #[must_use]
    pub const fn period_ms(self) -> u64 {
        match self {
            Self::Online => HEARTBEAT_PERIOD_MS,
            other => other.channel().period_ms(),
        }
    }

    /// Stable id.
    #[must_use]
    pub const fn id(self) -> &'static str {
        match self {
            Self::Online => "online",
            Self::Offline => "offline",
            Self::Idle => "idle",
            Self::Queued => "queued",
            Self::Running => "running",
            Self::Waiting => "waiting",
            Self::Success => "success",
            Self::Warning => "warning",
            Self::Failed => "failed",
            Self::Paused => "paused",
            Self::Unknown => "unknown",
        }
    }

    /// Default short label (always available as non-color cue with glyph).
    #[must_use]
    pub const fn default_label(self) -> &'static str {
        match self {
            Self::Online => "online",
            Self::Offline => "offline",
            Self::Idle => "idle",
            Self::Queued => "queued",
            Self::Running => "running",
            Self::Waiting => "waiting",
            Self::Success => "ok",
            Self::Warning => "warn",
            Self::Failed => "failed",
            Self::Paused => "paused",
            Self::Unknown => "unknown",
        }
    }

    /// Semantic paint role.
    #[must_use]
    pub const fn role(self) -> Role {
        match self {
            // Healthy and finished are the calm baseline. Green is reserved
            // for brand, focus, and the one primary action; the glyph + verb
            // carries success without turning every completed row green.
            Self::Online | Self::Success => Role::TextStrong,
            Self::Offline | Self::Idle | Self::Queued | Self::Paused | Self::Unknown => {
                Role::TextMuted
            }
            // Live work is information, not the brand: the reserved hue
            // belongs to the one thing the operator is acting on
            // (docs/design/termrock-design-language.md §3, plans/007).
            Self::Running => Role::InfoDim,
            Self::Waiting | Self::Warning => Role::Warning,
            Self::Failed => Role::Danger,
        }
    }

    /// Dot glyph (Unicode).
    #[must_use]
    pub const fn glyph_unicode(self) -> &'static str {
        match self {
            Self::Online => "●",
            Self::Offline => "○",
            Self::Idle => "◌",
            Self::Queued => "…",
            Self::Running => "◉",
            Self::Waiting => "◐",
            Self::Success => "✓",
            Self::Warning => "!",
            Self::Failed => "✗",
            Self::Paused => "‖",
            Self::Unknown => "?",
        }
    }

    /// Dot glyph (ASCII).
    #[must_use]
    pub const fn glyph_ascii(self) -> &'static str {
        match self {
            Self::Online => "*",
            Self::Offline => "o",
            Self::Idle => "~",
            Self::Queued => ".",
            Self::Running => ">",
            Self::Waiting => "=",
            Self::Success => "+",
            Self::Warning => "!",
            Self::Failed => "x",
            Self::Paused => "|",
            Self::Unknown => "?",
        }
    }

    /// Glyph for capability profile.
    #[must_use]
    pub const fn glyph(self, ascii: bool) -> &'static str {
        if ascii {
            self.glyph_ascii()
        } else {
            self.glyph_unicode()
        }
    }

    /// Glyph from [`GlyphSet`] on the design system.
    #[must_use]
    pub const fn glyph_for_set(self, glyphs: GlyphSet) -> &'static str {
        match glyphs {
            GlyphSet::Ascii => self.glyph_ascii(),
            GlyphSet::Unicode | GlyphSet::Enhanced => self.glyph_unicode(),
        }
    }

    /// Parse stable id.
    #[must_use]
    pub fn from_id(id: &str) -> Option<Self> {
        Some(match id {
            "online" => Self::Online,
            "offline" => Self::Offline,
            "idle" => Self::Idle,
            "queued" | "pending" => Self::Queued,
            "running" | "busy" => Self::Running,
            "waiting" => Self::Waiting,
            "away" => Self::Idle,
            "success" | "ok" | "done" | "complete" => Self::Success,
            "warning" | "warn" => Self::Warning,
            "failed" | "error" | "fail" => Self::Failed,
            "paused" => Self::Paused,
            "unknown" | "none" => Self::Unknown,
            _ => return None,
        })
    }

    /// All kinds (for tests / catalogs).
    pub const ALL: [Self; 11] = [
        Self::Online,
        Self::Offline,
        Self::Idle,
        Self::Queued,
        Self::Running,
        Self::Waiting,
        Self::Success,
        Self::Warning,
        Self::Failed,
        Self::Paused,
        Self::Unknown,
    ];
}
