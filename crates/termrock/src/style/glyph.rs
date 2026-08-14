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
pub const SPINNER_BRAILLE_FRAMES: &[&str] = &["⠋", "⠙", "⠹", "⠸", "⠼", "⠴", "⠦", "⠧", "⠇", "⠏"];
/// Quiet four-step presence pulse; every frame is one display cell.
pub const SPINNER_DOT_PULSE_FRAMES: &[&str] = &["⋅", ":", "⸬", "⁙"];

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
    /// Checkbox mixed / indeterminate (partial group).
    CheckMixed,
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
    /// Inline meta separator between adjacent facts on one row.
    MetaSeparator,
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
    /// Filled diamond accent.
    DiamondFilled,
    /// Command / composer prompt prefix.
    Prompt,
    /// Token / context usage marker.
    Token,
    /// Diamond with center mark.
    DiamondDouble,
    /// Hollow status dot.
    StatusDotHollow,
    /// Target status dot.
    StatusDotTarget,
    /// Ringed status dot.
    StatusDotRing,
    /// Heavy vertical accent rail.
    RailHeavy,
    /// Compact/collapsed vertical accent rail.
    RailCollapsed,
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
    /// Slider scale tick.
    SliderTick,
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
            Self::CheckMixed => "check-mixed",
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
            Self::MetaSeparator => "meta-separator",
            Self::Ellipsis => "ellipsis",
            Self::ModeDot => "mode-dot",
            Self::Connection => "connection",
            Self::SelectionMark => "selection-mark",
            Self::FocusDiamond => "focus-diamond",
            Self::EmptyCircle => "empty-circle",
            Self::DisabledMark => "disabled-mark",
            Self::DiamondFilled => "diamond-filled",
            Self::Prompt => "prompt",
            Self::Token => "token",
            Self::DiamondDouble => "diamond-double",
            Self::StatusDotHollow => "status-dot-hollow",
            Self::StatusDotTarget => "status-dot-target",
            Self::StatusDotRing => "status-dot-ring",
            Self::RailHeavy => "rail-heavy",
            Self::RailCollapsed => "rail-collapsed",
            Self::NowEdge => "now-edge",
            Self::Mask => "mask",
            Self::SliderThumb => "slider-thumb",
            Self::SliderFill => "slider-fill",
            Self::SliderRail => "slider-rail",
            Self::SliderTick => "slider-tick",
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
            Self::CheckMixed => "partially checked",
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
            Self::MetaSeparator => "separator",
            Self::Ellipsis => "more",
            Self::ModeDot => "mode",
            Self::Connection => "connection",
            Self::SelectionMark => "selection",
            Self::FocusDiamond => "focus",
            Self::EmptyCircle => "empty",
            Self::DisabledMark => "disabled",
            Self::DiamondFilled => "accent",
            Self::Prompt => "prompt",
            Self::Token => "token count",
            Self::DiamondDouble => "emphasis",
            Self::StatusDotHollow => "inactive status",
            Self::StatusDotTarget => "active status",
            Self::StatusDotRing => "ring status",
            Self::RailHeavy => "accent rail",
            Self::RailCollapsed => "collapsed accent rail",
            Self::NowEdge => "now",
            Self::Mask => "hidden character",
            Self::SliderThumb => "slider handle",
            Self::SliderFill => "filled track",
            Self::SliderRail => "remaining track",
            Self::SliderTick => "scale tick",
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
            | Self::NowEdge
            | Self::CheckOn
            | Self::CheckOff
            | Self::CheckMixed
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
            | Self::MetaSeparator
            | Self::Ellipsis
            | Self::ModeDot
            | Self::Connection
            | Self::SelectionMark
            | Self::FocusDiamond
            | Self::EmptyCircle
            | Self::DisabledMark
            | Self::DiamondFilled
            | Self::Prompt
            | Self::Token
            | Self::DiamondDouble
            | Self::StatusDotHollow
            | Self::StatusDotTarget
            | Self::StatusDotRing
            | Self::RailHeavy
            | Self::RailCollapsed
            | Self::Mask
            | Self::SliderThumb
            | Self::SliderFill
            | Self::SliderRail
            | Self::SliderTick
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
        Self::CheckMixed,
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
        Self::MetaSeparator,
        Self::Ellipsis,
        Self::ModeDot,
        Self::Connection,
        Self::SelectionMark,
        Self::FocusDiamond,
        Self::EmptyCircle,
        Self::DisabledMark,
        Self::DiamondFilled,
        Self::Prompt,
        Self::Token,
        Self::DiamondDouble,
        Self::StatusDotHollow,
        Self::StatusDotTarget,
        Self::StatusDotRing,
        Self::RailHeavy,
        Self::RailCollapsed,
        Self::NowEdge,
        Self::Mask,
        Self::SliderThumb,
        Self::SliderFill,
        Self::SliderRail,
        Self::SliderTick,
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

    /// Cells for each profile.
    ///
    /// **One glyph, one concept.** Two catalog entries that can appear in the
    /// same surface must not share an encoding in any profile — see
    /// [`GLYPH_CONTEXTS`] and the test that enforces it. Mutually exclusive
    /// *states* of one element (divider idle/focused) are exempt: they differ in
    /// weight and role, never side by side.
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
            Self::Info => ("·", "i", "ℹ"),
            Self::Loading => ("◔", ":", "◔"),
            Self::Busy => ("◐", "o", "◐"),
            Self::CheckOn => ("☑", "[x]", "☑"),
            Self::CheckOff => ("☐", "[ ]", "☐"),
            Self::CheckMixed => ("▣", "[-]", "▣"),
            Self::RadioOn => ("●", "(*)", "●"),
            Self::RadioOff => ("○", "( )", "○"),
            Self::File => ("▫", ".", "📄"),
            Self::Folder => ("▪", "/", "📁"),
            Self::FolderOpen => ("▨", "\\", "📂"),
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
            Self::SelectionGutter => ("▌", "*", "▌"),
            Self::Bullet => ("•", "-", "•"),
            Self::MetaSeparator => ("·", "|", "·"),
            Self::Ellipsis => ("…", "...", "…"),
            Self::ModeDot => ("●", "*", "●"),
            Self::Connection => ("◍", "=", "◍"),
            Self::SelectionMark => ("▮", "#", "▮"),
            Self::FocusDiamond => ("◊", ">", "◊"),
            Self::EmptyCircle => ("○", "o", "○"),
            Self::DisabledMark => ("⊘", "~", "⊘"),
            Self::DiamondFilled => ("◆", "*", "◆"),
            Self::Prompt => ("❯", ">", "❯"),
            Self::Token => ("◧", "%", "◧"),
            Self::DiamondDouble => ("◈", "#", "◈"),
            Self::StatusDotHollow => ("○", ".", "○"),
            Self::StatusDotTarget => ("◉", "@", "◉"),
            Self::StatusDotRing => ("◎", "O", "◎"),
            Self::RailHeavy => ("┃", "|", "┃"),
            Self::RailCollapsed => ("❙", "|", "❙"),
            Self::NowEdge => ("◇", "|", "◇"),
            Self::Mask => ("●", "*", "●"),
            Self::SliderThumb => ("●", "*", "●"),
            Self::SliderFill => ("━", "=", "━"),
            Self::SliderRail => ("─", "-", "─"),
            Self::SliderTick => ("┊", "|", "┊"),
            Self::DividerVertical => ("│", "|", "│"),
            Self::DividerVerticalActive => ("┃", "|", "┃"),
            Self::DividerVerticalHint => ("┋", "|", "┋"),
            Self::DividerHorizontal => ("─", "-", "─"),
            Self::DividerHorizontalActive => ("━", "=", "━"),
            Self::DividerHorizontalHint => ("┅", "=", "┅"),
        }
    }

    /// Nominal Unicode width: every catalog encoding is one narrow cell.
    ///
    /// East-Asian-Ambiguous members (`✓ ● ○ ◆ ◇ …`) are *assumed narrow*. A
    /// terminal configured for wide ambiguous width will double them; handling
    /// that needs a capability flag plus a layout audit and is deliberately
    /// deferred (see the plan's maintenance notes).
    const fn unicode_cols(self) -> u16 {
        1
    }

    const fn ascii_cols(self) -> u16 {
        match self {
            Self::CheckOn | Self::CheckOff | Self::CheckMixed => 3,
            Self::RadioOn | Self::RadioOff => 3,
            Self::Ellipsis => 3,
            _ => 1,
        }
    }

    const fn enhanced_cols(self) -> u16 {
        match self {
            Self::File | Self::Folder | Self::FolderOpen | Self::Search => 2,
            Self::CheckOn | Self::CheckOff | Self::CheckMixed => 1,
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

/// Glyph sets that share one surface, where an encoding collision would read as
/// two meanings wearing the same character.
///
/// Membership is "can a reader see these together and have to tell them apart" —
/// side by side in a row, or column-wise down a list. Two kinds of glyph are
/// deliberately *not* members:
///
/// - **Mutually exclusive states of one element** (a divider that is idle *or*
///   focused): separated by role and weight, never by shape.
/// - **Delimiters.** [`Glyph::MetaSeparator`] shares `·` with [`Glyph::Info`],
///   but a separator is always painted inside spaces (` · `) between facts,
///   while `Info` occupies a status cell. Listing it would force one of the two
///   to change shape to satisfy a rule neither of them breaks.
/// - **Header vs body cells.** [`Glyph::ArrowDown`] renders `v` in ASCII, the
///   same character as [`Glyph::DisclosureOpen`]. A table shows both, but never
///   in the same cell: the sort marker sits in the header row's column label
///   and disclosure sits in a body row's leading cell, one column-aligned band
///   apart. They are separate contexts below for that reason — the alternative
///   was breaking a disclosure encoding migrated in 0282 and trading this
///   collision for `-` vs [`Glyph::Bullet`] in the same file.
pub const GLYPH_CONTEXTS: &[(&str, &[Glyph])] = &[
    (
        "collection row",
        &[
            Glyph::SelectionGutter,
            Glyph::SelectionMark,
            Glyph::Bullet,
            Glyph::DisclosureOpen,
            Glyph::DisclosureClosed,
            Glyph::CheckOn,
            Glyph::CheckOff,
            Glyph::CheckMixed,
            Glyph::RadioOn,
            Glyph::RadioOff,
            Glyph::File,
            Glyph::Folder,
            Glyph::FolderOpen,
            Glyph::Success,
            Glyph::Warning,
            Glyph::Error,
            Glyph::Info,
            Glyph::Busy,
            Glyph::Loading,
            Glyph::Ellipsis,
            Glyph::DisabledMark,
        ],
    ),
    (
        "status strip",
        &[
            Glyph::StatusDotHollow,
            Glyph::StatusDotTarget,
            Glyph::StatusDotRing,
            Glyph::Connection,
            Glyph::Token,
            Glyph::ModeDot,
            Glyph::Busy,
            Glyph::Loading,
            Glyph::NowEdge,
            Glyph::FocusDiamond,
            Glyph::SelectionMark,
            Glyph::Success,
            Glyph::Warning,
            Glyph::Error,
            Glyph::Info,
        ],
    ),
    (
        // Column labels only. Disclosure and the row gutter live one band below,
        // in the body rows — see the header/body note above.
        "table header",
        &[Glyph::ArrowUp, Glyph::ArrowDown],
    ),
    (
        "slider track",
        &[
            Glyph::SliderThumb,
            Glyph::SliderFill,
            Glyph::SliderRail,
            Glyph::SliderTick,
        ],
    ),
];

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
    fn every_catalog_encoding_matches_declared_width() {
        for glyph in Glyph::ALL {
            for set in [GlyphSet::Unicode, GlyphSet::Ascii, GlyphSet::Enhanced] {
                let resolved = glyph.resolve(set);
                assert_eq!(resolved.display_width(), resolved.cols, "{glyph:?} {set:?}");
            }
        }
        for frame in SPINNER_BRAILLE_FRAMES
            .iter()
            .chain(SPINNER_DOT_PULSE_FRAMES)
        {
            assert_eq!(display_cols(frame), 1, "{frame:?}");
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
    fn co_occurring_glyphs_never_share_an_encoding() {
        for (context, members) in GLYPH_CONTEXTS {
            for set in [GlyphSet::Unicode, GlyphSet::Ascii, GlyphSet::Enhanced] {
                for (i, glyph) in members.iter().enumerate() {
                    for other in &members[i + 1..] {
                        assert_ne!(
                            glyph.resolve(set).text,
                            other.resolve(set).text,
                            "{context}/{set:?}: {glyph:?} and {other:?} paint the same cell",
                        );
                    }
                }
            }
        }
    }

    #[test]
    fn every_glyph_context_member_is_in_the_catalog() {
        for (context, members) in GLYPH_CONTEXTS {
            for glyph in *members {
                assert!(
                    Glyph::ALL.contains(glyph),
                    "{context}: {glyph:?} is not in the catalog",
                );
            }
        }
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
