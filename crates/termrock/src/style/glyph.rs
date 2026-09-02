// SPDX-FileCopyrightText: 2026 Alexey Zhokhov
// SPDX-License-Identifier: Apache-2.0

//! Semantic glyph catalog — names resolve to Unicode / ASCII / Enhanced cells.
//!
//! **Critical meaning.** Glyphs are never the only carrier of meaning: every
//! [`Glyph`] has a stable English [`Glyph::meaning`]. Prefer pairing paint with
//! [`crate::widgets::Icon::label`] or adjacent text for status that hosts act on.
//!
//! Inspired by Lucide's semantic naming; terminals constrain us to cell glyphs
//! (Unicode box/status, ASCII fallbacks, optional Enhanced emoji / richer set).
use crate::text::display_cols;

/// Shared vertical block ramp from empty through a full cell.
pub const BLOCK_RAMP: &[char; 9] = &[' ', '▁', '▂', '▃', '▄', '▅', '▆', '▇', '█'];
/// Shared horizontal (left-half) block ramp from empty through a full cell.
///
/// Powers sub-cell precision where the vertical ramp cannot: histogram bar
/// tops, slider thumbs between cells, and meter fills.
pub const LEFT_BLOCK_RAMP: &[char; 9] = &[' ', '▏', '▎', '▍', '▌', '▋', '▊', '▉', '█'];
/// Shared shade ramp for hatching and soft fills.
///
/// The colorless carrier for multi-series charts and backdrop stipple: density
/// separates series when hue cannot.
pub const SHADE_RAMP: &[char; 4] = &[' ', '░', '▒', '▓'];
/// Shared braille density ramp.
pub const BRAILLE_RAMP: &[char; 5] = &[' ', '⣀', '⣤', '⣶', '⣿'];
/// Cells painted for a fully masked secret.
///
/// Fixed width on purpose: a mask whose length tracked the secret would leak
/// it. Every masked field uses [`Glyph::Mask`] repeated this many times.
pub const MASK_CELLS: usize = 8;
/// Canonical deterministic braille spinner.
///
/// The ten-frame braille spinner, junie's single activity cadence at 80 ms.
pub const SPINNER_BRAILLE_FRAMES: &[&str] = &["⠋", "⠙", "⠹", "⠸", "⠼", "⠴", "⠦", "⠧", "⠇", "⠏"];
/// Semantic family for browsing and docs.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[non_exhaustive]
pub enum GlyphGroup {
    /// Chevrons, arrows, carets.
    Directional,
    /// Success / warning / error / checks.
    Status,
    /// Close, add, remove, …
    Action,
    /// Tree / collapsible disclosure.
    Disclosure,
    /// Rules, gutters, bullets, ellipsis chrome.
    Chrome,
}

impl GlyphGroup {
    /// Stable id.
    #[must_use]
    pub const fn id(self) -> &'static str {
        match self {
            Self::Directional => "directional",
            Self::Status => "status",
            Self::Action => "action",
            Self::Disclosure => "disclosure",
            Self::Chrome => "chrome",
        }
    }

    /// All groups in catalog order.
    pub const ALL: [Self; 5] = [
        Self::Directional,
        Self::Status,
        Self::Action,
        Self::Disclosure,
        Self::Chrome,
    ];
}

/// Semantic glyph name (Lucide-like), independent of encoding.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[non_exhaustive]
pub enum Glyph {
    /// Expanded disclosure / open branch.
    DisclosureOpen,
    /// Collapsed disclosure / closed branch.
    DisclosureClosed,
    /// Left chevron.
    ChevronLeft,
    /// Right chevron.
    ChevronRight,
    /// Up chevron.
    ChevronUp,
    /// Down chevron.
    ChevronDown,
    /// Right arrow.
    ArrowRight,
    /// Down arrow.
    ArrowDown,
    /// Success / ok.
    Success,
    /// Warning / caution.
    Warning,
    /// Error / failure.
    Error,
    /// Informational.
    Info,
    /// Loading / ellipsis busy.
    Loading,
    /// Checkbox on.
    CheckOn,
    /// Checkbox off.
    CheckOff,
    /// Radio selected.
    RadioOn,
    /// Radio unselected.
    RadioOff,
    /// Close / dismiss.
    Close,
    /// Add / plus.
    Add,
    /// Remove / minus.
    Remove,
    /// Horizontal rule unit.
    RuleH,
    /// Vertical rule unit.
    RuleV,
    /// Strong horizontal rule.
    RuleHStrong,
    /// Selection gutter bar.
    SelectionGutter,
    /// Selection marker triangle (classic `▸` cursor).
    SelectionMarker,
    /// List bullet.
    Bullet,
    /// Inline meta separator between adjacent facts on one row.
    MetaSeparator,
    /// More / overflow ellipsis.
    Ellipsis,
    /// Mode indicator dot.
    ModeDot,
    /// Empty / hollow circle.
    EmptyCircle,
    /// Filled diamond accent.
    DiamondFilled,
    /// Heavy vertical accent rail.
    RailHeavy,
    /// Live edge / checkpoint marker on a timeline.
    NowEdge,
    /// Masked (secret) character stand-in.
    Mask,
    /// Slider handle.
    SliderThumb,
    /// Filled part of a slider track.
    SliderFill,
    /// Empty part of a slider track.
    SliderRail,
    /// Idle vertical pane divider.
    DividerVertical,
    /// Focused vertical pane divider.
    DividerVerticalActive,
    /// Hovered (resizable) vertical pane divider.
    DividerVerticalHint,
    /// Idle horizontal pane divider.
    DividerHorizontal,
    /// Focused horizontal pane divider.
    DividerHorizontalActive,
    /// Hovered (resizable) horizontal pane divider.
    DividerHorizontalHint,
}

impl Glyph {
    /// Stable kebab id.
    #[must_use]
    pub const fn id(self) -> &'static str {
        match self {
            Self::DisclosureOpen => "disclosure-open",
            Self::DisclosureClosed => "disclosure-closed",
            Self::ChevronLeft => "chevron-left",
            Self::ChevronRight => "chevron-right",
            Self::ChevronUp => "chevron-up",
            Self::ChevronDown => "chevron-down",
            Self::ArrowRight => "arrow-right",
            Self::ArrowDown => "arrow-down",
            Self::Success => "success",
            Self::Warning => "warning",
            Self::Error => "error",
            Self::Info => "info",
            Self::Loading => "loading",
            Self::CheckOn => "check-on",
            Self::CheckOff => "check-off",
            Self::RadioOn => "radio-on",
            Self::RadioOff => "radio-off",
            Self::Close => "close",
            Self::Add => "add",
            Self::Remove => "remove",
            Self::RuleH => "rule-h",
            Self::RuleV => "rule-v",
            Self::RuleHStrong => "rule-h-strong",
            Self::SelectionGutter => "selection-gutter",
            Self::SelectionMarker => "selection-marker",
            Self::Bullet => "bullet",
            Self::MetaSeparator => "meta-separator",
            Self::Ellipsis => "ellipsis",
            Self::ModeDot => "mode-dot",
            Self::EmptyCircle => "empty-circle",
            Self::DiamondFilled => "diamond-filled",
            Self::RailHeavy => "rail-heavy",
            Self::NowEdge => "now-edge",
            Self::Mask => "mask",
            Self::SliderThumb => "slider-thumb",
            Self::SliderFill => "slider-fill",
            Self::SliderRail => "slider-rail",
            Self::DividerVertical => "divider-vertical",
            Self::DividerVerticalActive => "divider-vertical-active",
            Self::DividerVerticalHint => "divider-vertical-hint",
            Self::DividerHorizontal => "divider-horizontal",
            Self::DividerHorizontalActive => "divider-horizontal-active",
            Self::DividerHorizontalHint => "divider-horizontal-hint",
        }
    }

    /// Accessible English meaning (never empty — glyph is not sole meaning).
    #[must_use]
    pub const fn meaning(self) -> &'static str {
        match self {
            Self::DisclosureOpen => "expanded",
            Self::DisclosureClosed => "collapsed",
            Self::ChevronLeft => "left",
            Self::ChevronRight => "right",
            Self::ChevronUp => "up",
            Self::ChevronDown => "down",
            Self::ArrowRight => "arrow right",
            Self::ArrowDown => "arrow down",
            Self::Success => "success",
            Self::Warning => "warning",
            Self::Error => "error",
            Self::Info => "info",
            Self::Loading => "loading",
            Self::CheckOn => "checked",
            Self::CheckOff => "unchecked",
            Self::RadioOn => "selected",
            Self::RadioOff => "not selected",
            Self::Close => "close",
            Self::Add => "add",
            Self::Remove => "remove",
            Self::RuleH => "horizontal rule",
            Self::RuleV => "vertical rule",
            Self::RuleHStrong => "strong horizontal rule",
            Self::SelectionGutter => "selected",
            Self::SelectionMarker => "selected",
            Self::Bullet => "list item",
            Self::MetaSeparator => "separator",
            Self::Ellipsis => "more",
            Self::ModeDot => "mode",
            Self::EmptyCircle => "empty",
            Self::DiamondFilled => "accent",
            Self::RailHeavy => "accent rail",
            Self::NowEdge => "now",
            Self::Mask => "hidden character",
            Self::SliderThumb => "slider handle",
            Self::SliderFill => "filled track",
            Self::SliderRail => "remaining track",
            Self::DividerVertical => "vertical divider",
            Self::DividerVerticalActive => "focused vertical divider",
            Self::DividerVerticalHint => "resizable vertical divider",
            Self::DividerHorizontal => "horizontal divider",
            Self::DividerHorizontalActive => "focused horizontal divider",
            Self::DividerHorizontalHint => "resizable horizontal divider",
        }
    }

    /// Catalog group.
    #[must_use]
    pub const fn group(self) -> GlyphGroup {
        match self {
            Self::DisclosureOpen | Self::DisclosureClosed => GlyphGroup::Disclosure,
            Self::ChevronLeft
            | Self::ChevronRight
            | Self::ChevronUp
            | Self::ChevronDown
            | Self::ArrowRight
            | Self::ArrowDown => GlyphGroup::Directional,
            Self::Success
            | Self::Warning
            | Self::Error
            | Self::Info
            | Self::Loading
            | Self::NowEdge
            | Self::CheckOn
            | Self::CheckOff
            | Self::RadioOn
            | Self::RadioOff => GlyphGroup::Status,
            Self::Close | Self::Add | Self::Remove => GlyphGroup::Action,
            Self::RuleH
            | Self::RuleV
            | Self::RuleHStrong
            | Self::SelectionGutter
            | Self::SelectionMarker
            | Self::Bullet
            | Self::MetaSeparator
            | Self::Ellipsis
            | Self::ModeDot
            | Self::EmptyCircle
            | Self::DiamondFilled
            | Self::RailHeavy
            | Self::Mask
            | Self::SliderThumb
            | Self::SliderFill
            | Self::SliderRail
            | Self::DividerVertical
            | Self::DividerVerticalActive
            | Self::DividerVerticalHint
            | Self::DividerHorizontal
            | Self::DividerHorizontalActive
            | Self::DividerHorizontalHint => GlyphGroup::Chrome,
        }
    }

    /// Full catalog in stable order (Studio browser).
    pub const ALL: &'static [Self] = &[
        Self::DisclosureOpen,
        Self::DisclosureClosed,
        Self::ChevronLeft,
        Self::ChevronRight,
        Self::ChevronUp,
        Self::ChevronDown,
        Self::ArrowRight,
        Self::ArrowDown,
        Self::Success,
        Self::Warning,
        Self::Error,
        Self::Info,
        Self::Loading,
        Self::CheckOn,
        Self::CheckOff,
        Self::RadioOn,
        Self::RadioOff,
        Self::Close,
        Self::Add,
        Self::Remove,
        Self::RuleH,
        Self::RuleV,
        Self::RuleHStrong,
        Self::SelectionGutter,
        Self::SelectionMarker,
        Self::Bullet,
        Self::MetaSeparator,
        Self::Ellipsis,
        Self::ModeDot,
        Self::EmptyCircle,
        Self::DiamondFilled,
        Self::RailHeavy,
        Self::NowEdge,
        Self::Mask,
        Self::SliderThumb,
        Self::SliderFill,
        Self::SliderRail,
        Self::DividerVertical,
        Self::DividerVerticalActive,
        Self::DividerVerticalHint,
        Self::DividerHorizontal,
        Self::DividerHorizontalActive,
        Self::DividerHorizontalHint,
    ];

    /// Glyphs in a group.
    #[must_use]
    pub fn in_group(group: GlyphGroup) -> impl Iterator<Item = Self> {
        Self::ALL
            .iter()
            .copied()
            .filter(move |g| g.group() == group)
    }

    /// Resolve cells + width for the one junie vocabulary.
    #[must_use]
    pub const fn resolve(self) -> GlyphResolved {
        GlyphResolved {
            text: self.encoding(),
            cols: self.cols(),
            meaning: self.meaning(),
            glyph: self,
        }
    }

    /// Cells for each profile.
    ///
    /// **One glyph, one concept.** Two catalog entries that can appear in the
    /// same surface must not share an encoding in any profile — see
    /// [`GLYPH_CONTEXTS`] and the test that enforces it. Mutually exclusive
    /// *states* of one element (divider idle/focused) are exempt: they differ in
    /// weight and role, never side by side.
    /// The junie encoding: one character, one vocabulary, no profile choice.
    ///
    /// Meaning comes from the role a glyph is painted in and the slot it
    /// occupies, never from a unique shape: junie deliberately reuses `•` for
    /// both bullets and warnings and `●`/`○` for dots, radios, and masks.
    const fn encoding(self) -> &'static str {
        match self {
            Self::DisclosureOpen => "▾",
            Self::DisclosureClosed => "▸",
            Self::ChevronLeft => "‹",
            Self::ChevronRight => "›",
            Self::ChevronUp => "▴",
            Self::ChevronDown => "▾",
            Self::ArrowRight => "→",
            Self::ArrowDown => "↓",
            Self::Success => "✓",
            Self::Warning => "•",
            Self::Error => "!",
            Self::Info => "·",
            Self::Loading => "⠋",
            Self::CheckOn => "[✓]",
            Self::CheckOff => "[ ]",
            Self::RadioOn => "●",
            Self::RadioOff => "○",
            Self::Close => "×",
            Self::Add => "+",
            Self::Remove => "−",
            Self::RuleH => "─",
            Self::RuleV => "│",
            Self::RuleHStrong => "━",
            Self::SelectionGutter => "▎",
            Self::SelectionMarker => "▸",
            Self::Bullet => "•",
            Self::MetaSeparator => "·",
            Self::Ellipsis => "…",
            Self::ModeDot => "●",
            Self::EmptyCircle => "○",
            Self::DiamondFilled => "◆",
            Self::NowEdge => "◇",
            Self::Mask => "●",
            Self::SliderThumb => "●",
            Self::SliderFill => "━",
            Self::SliderRail => "─",
            Self::DividerVertical => "│",
            Self::DividerVerticalActive => "┃",
            Self::DividerVerticalHint => "┃",
            Self::DividerHorizontal => "─",
            Self::DividerHorizontalActive => "━",
            Self::DividerHorizontalHint => "━",
            Self::RailHeavy => "┃",
        }
    }

    /// Nominal columns: every encoding is one narrow cell except the bracketed
    /// checkbox pair, which is three.
    const fn cols(self) -> u16 {
        match self {
            Self::CheckOn | Self::CheckOff => 3,
            _ => 1,
        }
    }
}

/// Resolved paint cells for one glyph.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct GlyphResolved {
    /// Terminal cells to paint.
    pub text: &'static str,
    /// Nominal display columns (alignment budget).
    pub cols: u16,
    /// Accessible meaning (always non-empty).
    pub meaning: &'static str,
    /// Source semantic name.
    pub glyph: Glyph,
}

impl GlyphResolved {
    /// Measured display columns (grapheme-safe).
    #[must_use]
    pub fn display_width(&self) -> u16 {
        u16::try_from(display_cols(self.text))
            .unwrap_or(self.cols)
            .max(1)
    }

    /// Pad / clip text to `width` columns (left-aligned).
    #[must_use]
    pub fn aligned(&self, width: u16) -> String {
        use crate::text::take_display_cols;
        let w = usize::from(width.max(1));
        let mut s = take_display_cols(self.text, w);
        let used = display_cols(&s);
        if used < w {
            s.push_str(&" ".repeat(w - used));
        }
        s
    }
}

/// Lookup by stable id (`"disclosure-open"`).
#[must_use]
pub fn glyph_by_id(id: &str) -> Option<Glyph> {
    Glyph::ALL.iter().copied().find(|g| g.id() == id)
}

/// Surfaces where glyphs co-occur, stated under junie's context law.
///
/// junie does not give every concept a unique shape: `•` is both the bullet
/// and the warning mark, `●`/`○` serve dots, radios, and masks. Meaning comes
/// from the **role** a glyph is painted in and the **slot** it occupies — a
/// warning `•` wears [`crate::style::Role::Warning`] and sits in a state slot,
/// a bullet `•` wears the text tone and opens a row. This table records the
/// surfaces where several vocabulary members appear together, so a reader
/// auditing a surface knows which disambiguators are in play.
pub const GLYPH_CONTEXTS: &[(&str, &[Glyph])] = &[
    (
        "collection row (gutter col 0, marker col 1, content col 3)",
        &[
            Glyph::SelectionGutter,
            Glyph::Bullet,
            Glyph::DisclosureOpen,
            Glyph::DisclosureClosed,
            Glyph::CheckOn,
            Glyph::CheckOff,
            Glyph::RadioOn,
            Glyph::RadioOff,
            Glyph::Success,
            Glyph::Warning,
            Glyph::Error,
            Glyph::Info,
            Glyph::Loading,
            Glyph::Ellipsis,
        ],
    ),
    (
        "status strip (state slot + written label; the label is the cue)",
        &[
            Glyph::ModeDot,
            Glyph::Loading,
            Glyph::NowEdge,
            Glyph::Success,
            Glyph::Warning,
            Glyph::Error,
            Glyph::Info,
        ],
    ),
    (
        // Column labels only. Disclosure and the row gutter live one band
        // below, in the body rows.
        "table header (sort direction in the label column)",
        &[Glyph::ChevronUp, Glyph::ChevronDown],
    ),
    (
        "slider track (fill, thumb, and rail share one row)",
        &[Glyph::SliderThumb, Glyph::SliderFill, Glyph::SliderRail],
    ),
];

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_glyph_has_nonempty_meaning_id_and_encoding() {
        for g in Glyph::ALL {
            assert!(!g.id().is_empty(), "{g:?}");
            assert!(!g.meaning().is_empty(), "{g:?}");
            assert!(!g.resolve().text.is_empty(), "{g:?}");
        }
    }

    /// junie's vocabulary is fixed; these are the canonical spellings.
    #[test]
    fn encodings_are_the_junie_vocabulary() {
        let cases = [
            (Glyph::SelectionGutter, "▎"),
            (Glyph::RuleHStrong, "━"),
            (Glyph::Error, "!"),
            (Glyph::Warning, "•"),
            (Glyph::Success, "✓"),
            (Glyph::Close, "×"),
            (Glyph::CheckOn, "[✓]"),
            (Glyph::CheckOff, "[ ]"),
            (Glyph::ChevronUp, "▴"),
            (Glyph::ChevronDown, "▾"),
            (Glyph::DisclosureClosed, "▸"),
            (Glyph::DisclosureOpen, "▾"),
            (Glyph::DiamondFilled, "◆"),
            (Glyph::NowEdge, "◇"),
        ];
        for (glyph, text) in cases {
            assert_eq!(glyph.resolve().text, text, "{glyph:?}");
        }
        assert!(!Glyph::ALL.iter().any(|g| g.id() == "prompt"));
        for frame in SPINNER_BRAILLE_FRAMES {
            assert_eq!(display_cols(frame), 1, "{frame:?}");
        }
    }

    #[test]
    fn every_catalog_encoding_matches_declared_width() {
        for glyph in Glyph::ALL {
            let resolved = glyph.resolve();
            assert_eq!(resolved.display_width(), resolved.cols, "{glyph:?}");
        }
    }

    #[test]
    fn aligned_pads() {
        let s = Glyph::Add.resolve().aligned(3);
        assert_eq!(display_cols(&s), 3);
    }

    /// junie context law: shared encodings are legal, and the recorded
    /// contexts are exactly where a reader disambiguates by role and slot.
    #[test]
    fn contexts_are_catalog_members_and_share_encodings_by_design() {
        for (context, members) in GLYPH_CONTEXTS {
            assert!(!members.is_empty(), "{context}");
            for glyph in *members {
                assert!(
                    Glyph::ALL.contains(glyph),
                    "{context}: {glyph:?} is not in the catalog",
                );
            }
        }
        // The deliberate reuse: bullet and warning are both `•`, told apart
        // by role, not shape.
        assert_eq!(Glyph::Bullet.resolve().text, Glyph::Warning.resolve().text);
    }

    #[test]
    fn glyph_by_id_roundtrip() {
        for g in Glyph::ALL {
            assert_eq!(glyph_by_id(g.id()), Some(*g));
        }
        assert_eq!(glyph_by_id("nope"), None);
    }

    #[test]
    fn groups_partition_catalog() {
        let n: usize = GlyphGroup::ALL
            .iter()
            .map(|g| Glyph::in_group(*g).count())
            .sum();
        assert_eq!(n, Glyph::ALL.len());
    }
}
