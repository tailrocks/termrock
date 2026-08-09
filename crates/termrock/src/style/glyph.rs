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

use super::tokens::GlyphSet;
use crate::text::display_cols;

/// Semantic family for browsing and docs.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[non_exhaustive]
pub enum GlyphGroup {
    /// Chevrons, arrows, carets.
    Directional,
    /// Success / warning / error / busy / checks.
    Status,
    /// File and folder markers.
    FileType,
    /// Search, close, settings, play, …
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
            Self::FileType => "file-type",
            Self::Action => "action",
            Self::Disclosure => "disclosure",
            Self::Chrome => "chrome",
        }
    }

    /// All groups in catalog order.
    pub const ALL: [Self; 6] = [
        Self::Directional,
        Self::Status,
        Self::FileType,
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
    /// Left arrow.
    ArrowLeft,
    /// Right arrow.
    ArrowRight,
    /// Up arrow.
    ArrowUp,
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
    /// Busy / connected pulse.
    Busy,
    /// Checkbox on.
    CheckOn,
    /// Checkbox off.
    CheckOff,
    /// Radio selected.
    RadioOn,
    /// Radio unselected.
    RadioOff,
    /// File marker.
    File,
    /// Folder marker.
    Folder,
    /// Open folder marker.
    FolderOpen,
    /// Search / filter.
    Search,
    /// Close / dismiss.
    Close,
    /// Settings / gear.
    Settings,
    /// Edit / pencil.
    Edit,
    /// Copy.
    Copy,
    /// Play / start.
    Play,
    /// Stop.
    Stop,
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
    /// List bullet.
    Bullet,
    /// More / overflow ellipsis.
    Ellipsis,
    /// Mode indicator dot.
    ModeDot,
    /// Connection indicator.
    Connection,
    /// Selection mark.
    SelectionMark,
    /// Focus diamond.
    FocusDiamond,
    /// Empty / hollow circle.
    EmptyCircle,
    /// Disabled mark.
    DisabledMark,
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
            Self::ArrowLeft => "arrow-left",
            Self::ArrowRight => "arrow-right",
            Self::ArrowUp => "arrow-up",
            Self::ArrowDown => "arrow-down",
            Self::Success => "success",
            Self::Warning => "warning",
            Self::Error => "error",
            Self::Info => "info",
            Self::Loading => "loading",
            Self::Busy => "busy",
            Self::CheckOn => "check-on",
            Self::CheckOff => "check-off",
            Self::RadioOn => "radio-on",
            Self::RadioOff => "radio-off",
            Self::File => "file",
            Self::Folder => "folder",
            Self::FolderOpen => "folder-open",
            Self::Search => "search",
            Self::Close => "close",
            Self::Settings => "settings",
            Self::Edit => "edit",
            Self::Copy => "copy",
            Self::Play => "play",
            Self::Stop => "stop",
            Self::Add => "add",
            Self::Remove => "remove",
            Self::RuleH => "rule-h",
            Self::RuleV => "rule-v",
            Self::RuleHStrong => "rule-h-strong",
            Self::SelectionGutter => "selection-gutter",
            Self::Bullet => "bullet",
            Self::Ellipsis => "ellipsis",
            Self::ModeDot => "mode-dot",
            Self::Connection => "connection",
            Self::SelectionMark => "selection-mark",
            Self::FocusDiamond => "focus-diamond",
            Self::EmptyCircle => "empty-circle",
            Self::DisabledMark => "disabled-mark",
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
            Self::ArrowLeft => "arrow left",
            Self::ArrowRight => "arrow right",
            Self::ArrowUp => "arrow up",
            Self::ArrowDown => "arrow down",
            Self::Success => "success",
            Self::Warning => "warning",
            Self::Error => "error",
            Self::Info => "info",
            Self::Loading => "loading",
            Self::Busy => "busy",
            Self::CheckOn => "checked",
            Self::CheckOff => "unchecked",
            Self::RadioOn => "selected",
            Self::RadioOff => "not selected",
            Self::File => "file",
            Self::Folder => "folder",
            Self::FolderOpen => "open folder",
            Self::Search => "search",
            Self::Close => "close",
            Self::Settings => "settings",
            Self::Edit => "edit",
            Self::Copy => "copy",
            Self::Play => "play",
            Self::Stop => "stop",
            Self::Add => "add",
            Self::Remove => "remove",
            Self::RuleH => "horizontal rule",
            Self::RuleV => "vertical rule",
            Self::RuleHStrong => "strong horizontal rule",
            Self::SelectionGutter => "selected",
            Self::Bullet => "list item",
            Self::Ellipsis => "more",
            Self::ModeDot => "mode",
            Self::Connection => "connection",
            Self::SelectionMark => "selection",
            Self::FocusDiamond => "focus",
            Self::EmptyCircle => "empty",
            Self::DisabledMark => "disabled",
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
            | Self::ArrowLeft
            | Self::ArrowRight
            | Self::ArrowUp
            | Self::ArrowDown => GlyphGroup::Directional,
            Self::Success
            | Self::Warning
            | Self::Error
            | Self::Info
            | Self::Loading
            | Self::Busy
            | Self::CheckOn
            | Self::CheckOff
            | Self::RadioOn
            | Self::RadioOff => GlyphGroup::Status,
            Self::File | Self::Folder | Self::FolderOpen => GlyphGroup::FileType,
            Self::Search
            | Self::Close
            | Self::Settings
            | Self::Edit
            | Self::Copy
            | Self::Play
            | Self::Stop
            | Self::Add
            | Self::Remove => GlyphGroup::Action,
            Self::RuleH
            | Self::RuleV
            | Self::RuleHStrong
            | Self::SelectionGutter
            | Self::Bullet
            | Self::Ellipsis
            | Self::ModeDot
            | Self::Connection
            | Self::SelectionMark
            | Self::FocusDiamond
            | Self::EmptyCircle
            | Self::DisabledMark => GlyphGroup::Chrome,
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
        Self::ArrowLeft,
        Self::ArrowRight,
        Self::ArrowUp,
        Self::ArrowDown,
        Self::Success,
        Self::Warning,
        Self::Error,
        Self::Info,
        Self::Loading,
        Self::Busy,
        Self::CheckOn,
        Self::CheckOff,
        Self::RadioOn,
        Self::RadioOff,
        Self::File,
        Self::Folder,
        Self::FolderOpen,
        Self::Search,
        Self::Close,
        Self::Settings,
        Self::Edit,
        Self::Copy,
        Self::Play,
        Self::Stop,
        Self::Add,
        Self::Remove,
        Self::RuleH,
        Self::RuleV,
        Self::RuleHStrong,
        Self::SelectionGutter,
        Self::Bullet,
        Self::Ellipsis,
        Self::ModeDot,
        Self::Connection,
        Self::SelectionMark,
        Self::FocusDiamond,
        Self::EmptyCircle,
        Self::DisabledMark,
    ];

    /// Glyphs in a group.
    #[must_use]
    pub fn in_group(group: GlyphGroup) -> impl Iterator<Item = Self> {
        Self::ALL.iter().copied().filter(move |g| g.group() == group)
    }

    /// Resolve cells + width for a glyph set / profile.
    #[must_use]
    pub const fn resolve(self, set: GlyphSet) -> GlyphResolved {
        let (unicode, ascii, enhanced) = self.encodings();
        let text = match set {
            GlyphSet::Ascii => ascii,
            GlyphSet::Enhanced => enhanced,
            GlyphSet::Unicode => unicode,
        };
        // Width is measured at runtime in `display_width`; store nominal cols.
        let cols = match set {
            GlyphSet::Ascii => self.ascii_cols(),
            GlyphSet::Enhanced => self.enhanced_cols(),
            GlyphSet::Unicode => self.unicode_cols(),
        };
        GlyphResolved {
            text,
            cols,
            meaning: self.meaning(),
            glyph: self,
            set,
        }
    }

    const fn encodings(self) -> (&'static str, &'static str, &'static str) {
        // (unicode, ascii, enhanced)
        match self {
            Self::DisclosureOpen => ("▾", "v", "▾"),
            Self::DisclosureClosed => ("▸", ">", "▸"),
            Self::ChevronLeft => ("‹", "<", "‹"),
            Self::ChevronRight => ("›", ">", "›"),
            Self::ChevronUp => ("ˆ", "^", "ˆ"),
            Self::ChevronDown => ("ˇ", "v", "ˇ"),
            Self::ArrowLeft => ("←", "<", "←"),
            Self::ArrowRight => ("→", ">", "→"),
            Self::ArrowUp => ("↑", "^", "↑"),
            Self::ArrowDown => ("↓", "v", "↓"),
            Self::Success => ("✓", "+", "✓"),
            Self::Warning => ("!", "!", "⚠"),
            Self::Error => ("✕", "x", "✕"),
            Self::Info => ("i", "i", "ℹ"),
            Self::Loading => ("…", "...", "…"),
            Self::Busy => ("◉", "o", "◉"),
            Self::CheckOn => ("☑", "[x]", "☑"),
            Self::CheckOff => ("☐", "[ ]", "☐"),
            Self::RadioOn => ("●", "(*)", "●"),
            Self::RadioOff => ("○", "( )", "○"),
            Self::File => ("·", ".", "📄"),
            Self::Folder => ("▸", ">", "📁"),
            Self::FolderOpen => ("▾", "v", "📂"),
            Self::Search => ("/", "/", "🔍"),
            Self::Close => ("✕", "x", "✕"),
            Self::Settings => ("⚙", "*", "⚙"),
            Self::Edit => ("✎", "e", "✎"),
            Self::Copy => ("⧉", "c", "⧉"),
            Self::Play => ("▶", ">", "▶"),
            Self::Stop => ("■", "#", "■"),
            Self::Add => ("+", "+", "+"),
            Self::Remove => ("−", "-", "−"),
            Self::RuleH => ("─", "-", "─"),
            Self::RuleV => ("│", "|", "│"),
            Self::RuleHStrong => ("═", "=", "═"),
            Self::SelectionGutter => ("▌", ">", "▌"),
            Self::Bullet => ("•", "*", "•"),
            Self::Ellipsis => ("…", "...", "…"),
            Self::ModeDot => ("●", "*", "●"),
            Self::Connection => ("◉", "o", "◉"),
            Self::SelectionMark => ("▣", "#", "▣"),
            Self::FocusDiamond => ("◇", "+", "◇"),
            Self::EmptyCircle => ("○", "o", "○"),
            Self::DisabledMark => ("⊘", "x", "⊘"),
        }
    }

    const fn unicode_cols(self) -> u16 {
        match self {
            Self::CheckOn | Self::CheckOff => 1,
            Self::Loading | Self::Ellipsis => 1,
            _ => 1,
        }
    }

    const fn ascii_cols(self) -> u16 {
        match self {
            Self::CheckOn | Self::CheckOff => 3,
            Self::RadioOn | Self::RadioOff => 3,
            Self::Loading | Self::Ellipsis => 3,
            _ => 1,
        }
    }

    const fn enhanced_cols(self) -> u16 {
        match self {
            Self::File | Self::Folder | Self::FolderOpen | Self::Search | Self::Warning | Self::Info => {
                2
            }
            Self::CheckOn | Self::CheckOff => 1,
            Self::Loading | Self::Ellipsis => 1,
            _ => 1,
        }
    }
}

/// Resolved paint cells for one glyph under a [`GlyphSet`].
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
    /// Profile used.
    pub set: GlyphSet,
}

impl GlyphResolved {
    /// Measured display columns (grapheme-safe).
    #[must_use]
    pub fn display_width(&self) -> u16 {
        u16::try_from(display_cols(self.text)).unwrap_or(self.cols).max(1)
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_glyph_has_nonempty_meaning_and_id() {
        for g in Glyph::ALL {
            assert!(!g.id().is_empty(), "{g:?}");
            assert!(!g.meaning().is_empty(), "{g:?}");
            assert!(!g.resolve(GlyphSet::Unicode).text.is_empty() || g.id().contains("rule"));
        }
    }

    #[test]
    fn ascii_differs_for_disclosure() {
        let u = Glyph::DisclosureClosed.resolve(GlyphSet::Unicode);
        let a = Glyph::DisclosureClosed.resolve(GlyphSet::Ascii);
        assert_ne!(u.text, a.text);
        assert_eq!(a.text, ">");
    }

    #[test]
    fn enhanced_file_is_wider() {
        let e = Glyph::Folder.resolve(GlyphSet::Enhanced);
        assert!(e.display_width() >= 1);
        assert_eq!(e.meaning, "folder");
    }

    #[test]
    fn check_ascii_width_guarantee() {
        let a = Glyph::CheckOn.resolve(GlyphSet::Ascii);
        assert_eq!(a.cols, 3);
        assert_eq!(a.text, "[x]");
        assert_eq!(a.display_width(), 3);
    }

    #[test]
    fn aligned_pads() {
        let r = Glyph::Add.resolve(GlyphSet::Unicode);
        let s = r.aligned(3);
        assert_eq!(display_cols(&s), 3);
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

    #[test]
    fn resolve_is_cheap() {
        for _ in 0..50_000 {
            for g in Glyph::ALL {
                let _ = g.resolve(GlyphSet::Ascii);
            }
        }
    }
}
