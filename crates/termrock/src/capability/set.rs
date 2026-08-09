// SPDX-FileCopyrightText: 2026 Alexey Zhokhov
// SPDX-License-Identifier: Apache-2.0

//! Capability flags and fallback policy table.

use crate::style::{ColorCapability, GlyphSet};

/// Named optional terminal capability.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[non_exhaustive]
pub enum CapabilityKind {
    /// 24-bit RGB color.
    Truecolor,
    /// 256-color palette.
    Color256,
    /// 8/16 ANSI colors.
    AnsiColor,
    /// Chromatic color disabled (`NO_COLOR` / monochrome).
    NoColor,
    /// Unicode presentation (box drawing, wide glyphs).
    Unicode,
    /// Force ASCII-only glyphs.
    AsciiOnly,
    /// Conventional keyboard (raw mode key events).
    Keyboard,
    /// Enhanced keyboard protocol (e.g. kitty keyboard / modifyOtherKeys).
    EnhancedKeyboard,
    /// Mouse reporting.
    Mouse,
    /// Bracketed paste.
    BracketedPaste,
    /// OSC 8 hyperlinks.
    Hyperlinks,
    /// OSC 52 clipboard (or platform pasteboard via host).
    Clipboard,
    /// Synchronized output (DEC 2026 / mode 2026 family).
    SynchronizedOutput,
    /// Image protocols (Kitty / iTerm2 / Sixel — host-owned emission).
    ImageProtocols,
    /// Text-sizing extensions (experimental / host-owned).
    TextSizing,
    /// Alternate screen buffer.
    AlternateScreen,
    /// Inline rendering (no alt screen; scrollback-friendly).
    InlineRendering,
    /// Running under a multiplexer (tmux/screen/zellij).
    Multiplexer,
    /// Session over SSH.
    Ssh,
    /// Windows ConPTY host.
    WindowsConPty,
}

impl CapabilityKind {
    /// All kinds (for doctor iteration).
    pub const ALL: [Self; 20] = [
        Self::Truecolor,
        Self::Color256,
        Self::AnsiColor,
        Self::NoColor,
        Self::Unicode,
        Self::AsciiOnly,
        Self::Keyboard,
        Self::EnhancedKeyboard,
        Self::Mouse,
        Self::BracketedPaste,
        Self::Hyperlinks,
        Self::Clipboard,
        Self::SynchronizedOutput,
        Self::ImageProtocols,
        Self::TextSizing,
        Self::AlternateScreen,
        Self::InlineRendering,
        Self::Multiplexer,
        Self::Ssh,
        Self::WindowsConPty,
    ];

    /// Stable snake id for docs/env.
    #[must_use]
    pub const fn id(self) -> &'static str {
        match self {
            Self::Truecolor => "truecolor",
            Self::Color256 => "color256",
            Self::AnsiColor => "ansi_color",
            Self::NoColor => "no_color",
            Self::Unicode => "unicode",
            Self::AsciiOnly => "ascii_only",
            Self::Keyboard => "keyboard",
            Self::EnhancedKeyboard => "enhanced_keyboard",
            Self::Mouse => "mouse",
            Self::BracketedPaste => "bracketed_paste",
            Self::Hyperlinks => "hyperlinks",
            Self::Clipboard => "clipboard",
            Self::SynchronizedOutput => "synchronized_output",
            Self::ImageProtocols => "image_protocols",
            Self::TextSizing => "text_sizing",
            Self::AlternateScreen => "alternate_screen",
            Self::InlineRendering => "inline_rendering",
            Self::Multiplexer => "multiplexer",
            Self::Ssh => "ssh",
            Self::WindowsConPty => "windows_conpty",
        }
    }

    /// Human title.
    #[must_use]
    pub const fn title(self) -> &'static str {
        match self {
            Self::Truecolor => "Truecolor (24-bit)",
            Self::Color256 => "256 colors",
            Self::AnsiColor => "ANSI 8/16 colors",
            Self::NoColor => "No color",
            Self::Unicode => "Unicode glyphs",
            Self::AsciiOnly => "ASCII-only glyphs",
            Self::Keyboard => "Keyboard input",
            Self::EnhancedKeyboard => "Enhanced keyboard protocol",
            Self::Mouse => "Mouse",
            Self::BracketedPaste => "Bracketed paste",
            Self::Hyperlinks => "OSC 8 hyperlinks",
            Self::Clipboard => "Clipboard (OSC 52 / host)",
            Self::SynchronizedOutput => "Synchronized output",
            Self::ImageProtocols => "Image protocols",
            Self::TextSizing => "Text-sizing extensions",
            Self::AlternateScreen => "Alternate screen",
            Self::InlineRendering => "Inline rendering",
            Self::Multiplexer => "Terminal multiplexer",
            Self::Ssh => "SSH session",
            Self::WindowsConPty => "Windows ConPTY",
        }
    }
}

/// Documented fallback when a capability is unavailable or disabled.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct FallbackPolicy {
    /// Capability.
    pub kind: CapabilityKind,
    /// What the host/widget does instead.
    pub fallback: &'static str,
    /// Studio story id (contract evidence).
    pub story: &'static str,
    /// Contract / test hint.
    pub contract_test: &'static str,
}

/// Canonical fallback table (every optional feature).
#[must_use]
pub fn fallback_policies() -> &'static [FallbackPolicy] {
    &FALLBACKS
}

#[cfg(test)]
mod fallback_tests {
    use super::*;

    #[test]
    fn every_capability_kind_has_fallback_policy() {
        let policies = fallback_policies();
        assert_eq!(policies.len(), CapabilityKind::ALL.len());
        for kind in CapabilityKind::ALL {
            assert!(
                policies.iter().any(|p| p.kind == kind),
                "missing FallbackPolicy for {:?}",
                kind
            );
            let p = policies.iter().find(|p| p.kind == kind).unwrap();
            assert!(!p.fallback.is_empty());
            assert!(p.story.starts_with("capability/"));
            assert!(!p.contract_test.is_empty());
        }
    }
}

const FALLBACKS: [FallbackPolicy; 20] = [
    FallbackPolicy {
        kind: CapabilityKind::Truecolor,
        fallback: "Quantize theme to 256 → 16 → mono via ColorCapability",
        story: "capability/color-ladder",
        contract_test: "quantize_palette keeps roles",
    },
    FallbackPolicy {
        kind: CapabilityKind::Color256,
        fallback: "Drop to Ansi16 or Monochrome",
        story: "capability/color-ladder",
        contract_test: "detect_from_env respects TERM",
    },
    FallbackPolicy {
        kind: CapabilityKind::AnsiColor,
        fallback: "Monochrome modifiers only",
        story: "capability/no-color",
        contract_test: "no_color monochrome",
    },
    FallbackPolicy {
        kind: CapabilityKind::NoColor,
        fallback: "N/A when chromatic color enabled; when set, Role modifiers + glyphs",
        story: "capability/no-color",
        contract_test: "NO_COLOR forces mono",
    },
    FallbackPolicy {
        kind: CapabilityKind::Unicode,
        fallback: "GlyphSet::Ascii substitutes",
        story: "capability/ascii-glyphs",
        contract_test: "glyphset ascii markers",
    },
    FallbackPolicy {
        kind: CapabilityKind::AsciiOnly,
        fallback: "Force GlyphSet::Ascii even if Unicode available",
        story: "capability/ascii-glyphs",
        contract_test: "ascii override",
    },
    FallbackPolicy {
        kind: CapabilityKind::Keyboard,
        fallback: "Headless/read-only mode; no interactive session",
        story: "capability/headless",
        contract_test: "session without raw mode",
    },
    FallbackPolicy {
        kind: CapabilityKind::EnhancedKeyboard,
        fallback: "Conventional crossterm KeyEvent mapping only",
        story: "capability/keyboard-basic",
        contract_test: "enhanced off by default",
    },
    FallbackPolicy {
        kind: CapabilityKind::Mouse,
        fallback: "Keyboard-only hit equivalents; SessionOptions.mouse_capture=false",
        story: "capability/no-mouse",
        contract_test: "mouse optional in SessionOptions",
    },
    FallbackPolicy {
        kind: CapabilityKind::BracketedPaste,
        fallback: "Treat paste as rapid key events or disable paste; SessionOptions flag",
        story: "capability/paste-fallback",
        contract_test: "bracketed paste optional",
    },
    FallbackPolicy {
        kind: CapabilityKind::Hyperlinks,
        fallback: "Paint link text only; no OSC 8 emission",
        story: "capability/hyperlink-off",
        contract_test: "hyperlink encode is consumer-owned",
    },
    FallbackPolicy {
        kind: CapabilityKind::Clipboard,
        fallback: "Copy outcomes only; host may no-op OSC 52",
        story: "capability/clipboard-off",
        contract_test: "clipboard not required for copy outcome",
    },
    FallbackPolicy {
        kind: CapabilityKind::SynchronizedOutput,
        fallback: "Normal frame writes without sync delimiters",
        story: "capability/sync-off",
        contract_test: "sync optional",
    },
    FallbackPolicy {
        kind: CapabilityKind::ImageProtocols,
        fallback: "CapabilityPreviewHost cell fallback / alt text",
        story: "capability/image-fallback",
        contract_test: "preview host CellFallback",
    },
    FallbackPolicy {
        kind: CapabilityKind::TextSizing,
        fallback: "Ignore size extensions; use cell grid only",
        story: "capability/text-sizing-off",
        contract_test: "no hard dep on text-size",
    },
    FallbackPolicy {
        kind: CapabilityKind::AlternateScreen,
        fallback: "Inline profile: draw in main buffer, preserve scrollback",
        story: "capability/inline",
        contract_test: "SessionOptions.alternate_screen=false",
    },
    FallbackPolicy {
        kind: CapabilityKind::InlineRendering,
        fallback: "Use alternate screen when preferred and available",
        story: "capability/inline",
        contract_test: "inline profile",
    },
    FallbackPolicy {
        kind: CapabilityKind::Multiplexer,
        fallback: "Conservative color; warn on truecolor-through-tmux without Tc",
        story: "capability/multiplexer",
        contract_test: "tmux detection",
    },
    FallbackPolicy {
        kind: CapabilityKind::Ssh,
        fallback: "Prefer Compatible profile; avoid assuming local clipboard",
        story: "capability/ssh",
        contract_test: "ssh detection",
    },
    FallbackPolicy {
        kind: CapabilityKind::WindowsConPty,
        fallback: "Compatible keyboard/mouse; ConPTY-specific host adapter",
        story: "capability/conpty",
        contract_test: "conpty flag",
    },
];

/// Flat enablement set after detection + overrides + profile.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub struct CapabilitySet {
    /// Color ladder (mutually exclusive preference; NoColor → Monochrome).
    pub color: ColorCapability,
    /// Glyph set.
    pub glyphs: GlyphSet,
    /// Mouse reporting desired.
    pub mouse: bool,
    /// Bracketed paste desired.
    pub bracketed_paste: bool,
    /// OSC 8 emission allowed.
    pub hyperlinks: bool,
    /// OSC 52 / host clipboard allowed.
    pub clipboard: bool,
    /// Enhanced keyboard protocol negotiation allowed.
    pub enhanced_keyboard: bool,
    /// Synchronized output delimiters allowed.
    pub synchronized_output: bool,
    /// Image protocol emission allowed (host still chooses protocol).
    pub image_protocols: bool,
    /// Text-sizing extensions allowed.
    pub text_sizing: bool,
    /// Enter alternate screen.
    pub alternate_screen: bool,
    /// Prefer inline (implies !alternate_screen when exclusive).
    pub inline: bool,
    /// Raw mode / keyboard session.
    pub keyboard: bool,
    /// Environment hints (informational).
    pub multiplexer: bool,
    /// SSH session hint.
    pub ssh: bool,
    /// Windows ConPTY hint.
    pub windows_conpty: bool,
}

impl CapabilitySet {
    /// Whether chromatic color is available.
    #[must_use]
    pub const fn has_color(self) -> bool {
        !matches!(self.color, ColorCapability::Monochrome)
    }

    /// Whether Unicode glyphs are preferred.
    #[must_use]
    pub const fn unicode_glyphs(self) -> bool {
        matches!(self.glyphs, GlyphSet::Unicode)
    }

    /// Query a kind (environment + feature enablement).
    #[must_use]
    pub const fn enabled(self, kind: CapabilityKind) -> bool {
        match kind {
            CapabilityKind::Truecolor => matches!(self.color, ColorCapability::Truecolor),
            CapabilityKind::Color256 => matches!(
                self.color,
                ColorCapability::Truecolor | ColorCapability::Indexed256
            ),
            CapabilityKind::AnsiColor => self.has_color(),
            CapabilityKind::NoColor => !self.has_color(),
            CapabilityKind::Unicode => self.unicode_glyphs(),
            CapabilityKind::AsciiOnly => !self.unicode_glyphs(),
            CapabilityKind::Keyboard => self.keyboard,
            CapabilityKind::EnhancedKeyboard => self.enhanced_keyboard,
            CapabilityKind::Mouse => self.mouse,
            CapabilityKind::BracketedPaste => self.bracketed_paste,
            CapabilityKind::Hyperlinks => self.hyperlinks,
            CapabilityKind::Clipboard => self.clipboard,
            CapabilityKind::SynchronizedOutput => self.synchronized_output,
            CapabilityKind::ImageProtocols => self.image_protocols,
            CapabilityKind::TextSizing => self.text_sizing,
            CapabilityKind::AlternateScreen => self.alternate_screen,
            CapabilityKind::InlineRendering => self.inline,
            CapabilityKind::Multiplexer => self.multiplexer,
            CapabilityKind::Ssh => self.ssh,
            CapabilityKind::WindowsConPty => self.windows_conpty,
        }
    }
}
