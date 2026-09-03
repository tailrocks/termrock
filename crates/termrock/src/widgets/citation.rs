// SPDX-FileCopyrightText: 2026 Alexey Zhokhov
// SPDX-License-Identifier: Apache-2.0

//! **SourceCitation** and **CitationList** — compact inline citations and
//! expandable source lists for agent output.
//!
//! **Mission.** Source title, type, path/URL, range, confidence/provenance,
//! open, preview, copy, unavailable state, and duplicate grouping. Keep **raw
//! destinations visible** for external or sensitive sources. Integrate with
//! Markdown [`SourceAnchor`](crate::widgets::SourceAnchor) and fullscreen
//! previews. Keyboard navigation without fragmenting reading flow. Support
//! no-hyperlink and offline states.
//!
//! Research: research assistants, IDE references, terminal hyperlink (OSC 8)
//! capabilities.
//!
//! **Ownership.** Host opens URLs / files and owns provider provenance. TermRock
//! paints chrome and emits typed outcomes — never writes OSC to the PTY
//! (compose with [`Link`](crate::widgets::Link) for OSC regions when online).
use std::collections::BTreeMap;

use ratatui_core::{buffer::Buffer, layout::Rect, style::Modifier};

use crate::{
    input::{KeyCode, KeyEvent, KeyModifiers, MouseButton, MouseEvent, MouseEventKind},
    style::{DesignSystem, ListRowVisualState, Role},
    text::{display_cols, take_display_cols},
    widgets::{
        link::DestinationDisplay, markdown::SourceAnchor, streaming_markdown::StreamCitation,
    },
};

/// Overlay id for fullscreen citation preview.
pub const CITATION_PREVIEW_OVERLAY_ID: &str = "termrock.citation_preview";

// ── Domain ──────────────────────────────────────────────────────────────────

/// Source kind (display + glyph).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
#[non_exhaustive]
pub enum CitationSourceType {
    /// Local or workspace file.
    #[default]
    File,
    /// HTTP(S) / remote URL.
    Url,
    /// Documentation / doc site.
    Docs,
    /// Issue / ticket / PR.
    Issue,
    /// Academic / paper / DOI.
    Paper,
    /// Chat / session message.
    Message,
    /// Other host type.
    Other,
}

impl CitationSourceType {
    /// Stable id.
    #[must_use]
    pub const fn id(self) -> &'static str {
        match self {
            Self::File => "file",
            Self::Url => "url",
            Self::Docs => "docs",
            Self::Issue => "issue",
            Self::Paper => "paper",
            Self::Message => "message",
            Self::Other => "other",
        }
    }

    /// ASCII letter.
    #[must_use]
    pub const fn letter(self) -> char {
        match self {
            Self::File => 'F',
            Self::Url => 'U',
            Self::Docs => 'D',
            Self::Issue => 'I',
            Self::Paper => 'P',
            Self::Message => 'M',
            Self::Other => '?',
        }
    }

    /// Glyph.
    #[must_use]
    pub const fn glyph(self) -> &'static str {
        match self {
            // One column each (plans/013).
            Self::File => "▤",
            Self::Url => "↗",
            Self::Docs => "▤",
            Self::Issue => "◉",
            Self::Paper => "§",
            Self::Message => "❝",
            Self::Other => "·",
        }
    }

    /// Infer from destination string (best-effort).
    #[must_use]
    pub fn infer(dest: &str) -> Self {
        let d = dest.to_ascii_lowercase();
        if d.starts_with("http://") || d.starts_with("https://") || d.starts_with("mailto:") {
            if d.contains("doi.org") || d.contains("/paper") {
                Self::Paper
            } else if d.contains("github.com") && (d.contains("/issues/") || d.contains("/pull/")) {
                Self::Issue
            } else if d.contains("/docs") || d.contains("readthedocs") {
                Self::Docs
            } else {
                Self::Url
            }
        } else if d.starts_with("file:") || d.contains('/') || d.contains('\\') {
            Self::File
        } else {
            Self::Other
        }
    }
}

/// Availability of a citation destination.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
#[non_exhaustive]
pub enum CitationAvailability {
    /// Openable.
    #[default]
    Available,
    /// Offline / no network for remote.
    Offline,
    /// Destination missing / 404.
    Unavailable,
    /// Sensitive — open restricted; still show raw dest.
    Restricted,
}

impl CitationAvailability {
    /// Stable id.
    #[must_use]
    pub const fn id(self) -> &'static str {
        match self {
            Self::Available => "available",
            Self::Offline => "offline",
            Self::Unavailable => "unavailable",
            Self::Restricted => "restricted",
        }
    }

    /// Whether open is allowed.
    #[must_use]
    pub const fn can_open(self) -> bool {
        matches!(self, Self::Available)
    }
}

/// Provenance / confidence for a citation (host-projected).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CitationProvenance {
    /// 0–100 confidence; None = unknown.
    pub confidence: Option<u8>,
    /// Short provenance label (`rag`, `tool:grep`, `user`).
    pub provenance: Option<String>,
    /// Extra note.
    pub note: Option<String>,
}

impl Default for CitationProvenance {
    fn default() -> Self {
        Self {
            confidence: None,
            provenance: None,
            note: None,
        }
    }
}

impl CitationProvenance {
    /// With confidence.
    #[must_use]
    pub fn confidence(mut self, c: u8) -> Self {
        self.confidence = Some(c.min(100));
        self
    }

    /// Provenance tag.
    #[must_use]
    pub fn provenance(mut self, p: impl Into<String>) -> Self {
        self.provenance = Some(p.into());
        self
    }
}

/// One source citation (domain model for inline + list).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CitationSource {
    /// Stable id (also used for duplicate grouping key when `group_key` unset).
    pub id: String,
    /// Display index label (`[1]`, `a`).
    pub index_label: String,
    /// Title / short name.
    pub title: String,
    /// Source type.
    pub kind: CitationSourceType,
    /// Raw path or URL — **always retained** for external/sensitive display.
    pub destination: String,
    /// Whether destination is external URL risk.
    pub external: bool,
    /// Line/byte range in source document.
    pub range: Option<SourceAnchor>,
    /// Markdown body anchor this citation supports.
    pub markdown_anchor: Option<SourceAnchor>,
    /// Confidence / provenance.
    pub provenance: CitationProvenance,
    /// Availability.
    pub availability: CitationAvailability,
    /// Group key for duplicates (canonical dest or DOI).
    pub group_key: Option<String>,
    /// Optional preview snippet (safe text).
    pub preview: Option<String>,
    /// Sensitive: still show dest but restrict open.
    pub sensitive: bool,
}

impl CitationSource {
    /// File citation.
    #[must_use]
    pub fn file(
        id: impl Into<String>,
        index: impl Into<String>,
        title: impl Into<String>,
        path: impl Into<String>,
    ) -> Self {
        let destination = path.into();
        Self {
            id: id.into(),
            index_label: index.into(),
            title: title.into(),
            kind: CitationSourceType::File,
            destination,
            external: false,
            range: None,
            markdown_anchor: None,
            provenance: CitationProvenance::default(),
            availability: CitationAvailability::Available,
            group_key: None,
            preview: None,
            sensitive: false,
        }
    }

    /// URL citation.
    #[must_use]
    pub fn url(
        id: impl Into<String>,
        index: impl Into<String>,
        title: impl Into<String>,
        url: impl Into<String>,
    ) -> Self {
        let destination = url.into();
        Self {
            id: id.into(),
            index_label: index.into(),
            title: title.into(),
            kind: CitationSourceType::Url,
            destination: destination.clone(),
            external: true,
            range: None,
            markdown_anchor: None,
            provenance: CitationProvenance::default(),
            availability: CitationAvailability::Available,
            group_key: Some(destination),
            preview: None,
            sensitive: false,
        }
    }

    /// Kind.
    #[must_use]
    pub const fn kind(mut self, k: CitationSourceType) -> Self {
        self.kind = k;
        self
    }

    /// Range.
    #[must_use]
    pub const fn range(mut self, r: SourceAnchor) -> Self {
        self.range = Some(r);
        self
    }

    /// Markdown anchor.
    #[must_use]
    pub const fn markdown_anchor(mut self, a: SourceAnchor) -> Self {
        self.markdown_anchor = Some(a);
        self
    }

    /// Provenance.
    #[must_use]
    pub fn provenance(mut self, p: CitationProvenance) -> Self {
        self.provenance = p;
        self
    }

    /// Availability.
    #[must_use]
    pub const fn availability(mut self, a: CitationAvailability) -> Self {
        self.availability = a;
        self
    }

    /// Group key.
    #[must_use]
    pub fn group_key(mut self, k: impl Into<String>) -> Self {
        self.group_key = Some(k.into());
        self
    }

    /// Preview.
    #[must_use]
    pub fn preview(mut self, p: impl Into<String>) -> Self {
        self.preview = Some(p.into());
        self
    }

    /// Sensitive.
    #[must_use]
    pub const fn sensitive(mut self, on: bool) -> Self {
        self.sensitive = on;
        self
    }

    /// Canonical group key.
    #[must_use]
    pub fn effective_group_key(&self) -> &str {
        self.group_key
            .as_deref()
            .unwrap_or(self.destination.as_str())
    }

    /// Whether the raw destination follows the label under `policy`.
    ///
    /// Single resolver for both the compact chip ([`SourceCitation`]) and the
    /// expanded list rows: `Auto` falls back to external / sensitive /
    /// no-hyperlink sources.
    #[must_use]
    pub fn shows_destination(&self, policy: DestinationDisplay, no_hyperlink: bool) -> bool {
        match policy {
            DestinationDisplay::Always => true,
            DestinationDisplay::Never => false,
            DestinationDisplay::Auto => self.external || self.sensitive || no_hyperlink,
        }
    }

    /// Inline label: `[1]` or `[1:title]`.
    #[must_use]
    pub fn inline_label(&self, show_title: bool) -> String {
        if show_title && !self.title.is_empty() {
            format!(
                "[{}:{}]",
                self.index_label.trim_matches(|c| c == '[' || c == ']'),
                self.title
            )
        } else if self.index_label.starts_with('[') {
            self.index_label.clone()
        } else {
            format!("[{}]", self.index_label)
        }
    }

    /// Compact meta line (type · range · conf · offline).
    #[must_use]
    pub fn meta_line(&self) -> String {
        let mut parts = Vec::new();
        parts.push(self.kind.glyph().to_string());
        if let Some(r) = self.range {
            if r.line_start == r.line_end {
                parts.push(format!("L{}", r.line_start));
            } else {
                parts.push(format!("L{}-{}", r.line_start, r.line_end));
            }
        }
        if let Some(c) = self.provenance.confidence {
            parts.push(format!("{c}%"));
        }
        if let Some(p) = &self.provenance.provenance {
            parts.push(p.clone());
        }
        if !matches!(self.availability, CitationAvailability::Available) {
            parts.push(self.availability.id().into());
        }
        parts.join(" · ")
    }

    /// Destination for display (never empty for external).
    #[must_use]
    pub fn destination_display(&self) -> &str {
        &self.destination
    }

    /// Text for copy (title + dest + range).
    #[must_use]
    pub fn copy_text(&self) -> String {
        let mut s = format!("{} — {}", self.title, self.destination);
        if let Some(r) = self.range {
            s.push_str(&format!(" (L{}-{})", r.line_start, r.line_end));
        }
        s
    }
}

/// Group duplicates by `effective_group_key`; keeps first as primary.
#[must_use]
pub fn group_citations(sources: &[CitationSource]) -> Vec<CitationGroup> {
    let mut map: BTreeMap<String, Vec<usize>> = BTreeMap::new();
    for (i, s) in sources.iter().enumerate() {
        map.entry(s.effective_group_key().to_string())
            .or_default()
            .push(i);
    }
    map.into_iter()
        .map(|(key, indices)| {
            let primary = indices[0];
            CitationGroup {
                key,
                primary,
                duplicates: indices[1..].to_vec(),
            }
        })
        .collect()
}

/// One duplicate group.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CitationGroup {
    /// Group key.
    pub key: String,
    /// Index of primary source in original slice.
    pub primary: usize,
    /// Additional indices.
    pub duplicates: Vec<usize>,
}

impl CitationGroup {
    /// Total count including primary.
    #[must_use]
    pub fn count(&self) -> usize {
        1 + self.duplicates.len()
    }
}

// ── Bridges ─────────────────────────────────────────────────────────────────

/// From StreamingMarkdown [`StreamCitation`].
#[must_use]
pub fn citation_from_stream(c: &StreamCitation, index: usize) -> CitationSource {
    let dest = c.href.clone().unwrap_or_default();
    let external = dest.starts_with("http://") || dest.starts_with("https://");
    let kind = if dest.is_empty() {
        CitationSourceType::Other
    } else {
        CitationSourceType::infer(&dest)
    };
    let group_key = if dest.is_empty() {
        Some(format!("idx:{index}"))
    } else {
        Some(dest.clone())
    };
    CitationSource {
        id: c.id.clone(),
        index_label: c.label.clone(),
        title: c.label.clone(),
        kind,
        destination: if dest.is_empty() {
            c.label.clone()
        } else {
            dest
        },
        external,
        range: c.source,
        markdown_anchor: c.source,
        provenance: CitationProvenance::default(),
        availability: CitationAvailability::Available,
        group_key,
        preview: None,
        sensitive: false,
    }
}

/// To StreamCitation for StreamingMarkdown footer.
#[must_use]
pub fn citation_to_stream(c: &CitationSource) -> StreamCitation {
    let mut s = StreamCitation::new(c.id.clone(), c.index_label.clone());
    if c.external || c.destination.starts_with("http") {
        s = s.href(c.destination.clone());
    }
    if let Some(a) = c.range {
        s = s.source(a);
    }
    s
}

// ── Outcomes ────────────────────────────────────────────────────────────────

/// Inline citation outcomes.
#[derive(Debug, Clone, PartialEq, Eq)]
#[non_exhaustive]
pub enum SourceCitationOutcome {
    /// Ignored.
    Ignored,
    /// Open destination (host).
    OpenRequested {
        /// Citation id.
        id: String,
        /// Raw destination.
        destination: String,
        /// External risk.
        external: bool,
    },
    /// Preview / fullscreen.
    PreviewRequested {
        /// Id.
        id: String,
    },
    /// Copy.
    CopyRequested {
        /// Id.
        id: String,
        /// Text (includes raw dest).
        text: String,
    },
    /// Focus / hover changed.
    Focused {
        /// Id.
        id: String,
    },
    /// Jump to markdown anchor in parent reader.
    JumpToAnchor {
        /// Citation id.
        id: String,
        /// Anchor.
        anchor: SourceAnchor,
    },
}

/// Citation list outcomes.
#[derive(Debug, Clone, PartialEq, Eq)]
#[non_exhaustive]
pub enum CitationListOutcome {
    /// Ignored.
    Ignored,
    /// Cursor moved.
    SelectionChanged {
        /// Id.
        id: String,
    },
    /// Expanded / collapsed list chrome.
    ExpandChanged {
        /// Expanded.
        expanded: bool,
    },
    /// Nested citation outcome.
    Citation(SourceCitationOutcome),
    /// Group expanded to show duplicates.
    GroupExpanded {
        /// Group key.
        key: String,
    },
}

// ── SourceCitation (inline) ─────────────────────────────────────────────────

/// Inline citation chrome state.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct SourceCitationState {
    /// Focused in reading flow (Tab/roving from list or sequential).
    pub focused: bool,
    /// Hover.
    pub hovered: bool,
    /// Visited.
    pub visited: bool,
    /// Acknowledgement owed after a copy fired.
    pub copied: crate::style::ActionFlash,
    /// Last paint rect.
    pub area: Rect,
}

impl SourceCitationState {
    /// Fresh.
    #[must_use]
    pub const fn new() -> Self {
        Self {
            focused: false,
            hovered: false,
            visited: false,
            copied: crate::style::ActionFlash::new(),
            area: Rect {
                x: 0,
                y: 0,
                width: 0,
                height: 0,
            },
        }
    }

    /// Focus.
    pub const fn set_focused(&mut self, on: bool) {
        self.focused = on;
    }
}

/// Compact inline citation (`[1]` / chip).
#[derive(Debug, Clone, Copy)]
pub struct SourceCitation<'a> {
    source: &'a CitationSource,
    system: &'a DesignSystem,
    /// Force show raw destination after label.
    show_destination: DestinationDisplay,
    /// Disable OSC 8 (no-hyperlink terminal / offline).
    no_hyperlink: bool,
    /// Offline forces offline chrome even if Available.
    offline: bool,
}

impl<'a> SourceCitation<'a> {
    /// Source + system.
    #[must_use]
    pub const fn new(source: &'a CitationSource, system: &'a DesignSystem) -> Self {
        Self {
            source,
            system,
            show_destination: DestinationDisplay::Auto,
            no_hyperlink: false,
            offline: false,
        }
    }

    /// Destination display policy.
    #[must_use]
    pub const fn show_destination(mut self, d: DestinationDisplay) -> Self {
        self.show_destination = d;
        self
    }

    /// Offline session.
    #[must_use]
    pub const fn offline(mut self, on: bool) -> Self {
        self.offline = on;
        self
    }

    /// Decorated string for measure/paint.
    #[must_use]
    pub fn decorated(&self) -> String {
        let g = self.source.kind.glyph();
        let mut s = format!("{}{}", g, self.source.inline_label(false));
        let show_dest = self
            .source
            .shows_destination(self.show_destination, self.no_hyperlink);
        if show_dest && !self.source.destination.is_empty() {
            s.push(' ');
            s.push_str(&truncate_dest(
                &self.source.destination,
                24,
                self.system.glyphs.ellipsis(),
            ));
        }
        let avail = effective_availability(self.source, self.offline);
        if !matches!(avail, CitationAvailability::Available) {
            s.push(' ');
            s.push_str(avail.id());
        }
        s
    }

    /// Natural width.
    #[must_use]
    pub fn measure_width(&self) -> u16 {
        u16::try_from(display_cols(&self.decorated()))
            .unwrap_or(1)
            .max(1)
    }

    /// Whether open is allowed under current flags.
    #[must_use]
    pub fn can_open(&self) -> bool {
        effective_availability(self.source, self.offline).can_open() && !self.source.sensitive
    }

    /// Paint inline citation.
    pub fn paint(&self, area: Rect, buffer: &mut Buffer, state: &mut SourceCitationState) {
        if area.is_empty() {
            return;
        }
        state.area = area;
        let text = self.decorated();
        let avail = effective_availability(self.source, self.offline);
        let style = if !avail.can_open() || self.source.sensitive {
            self.system.style(Role::TextMuted)
        } else if state.focused || state.hovered {
            self.system
                .style(Role::Link)
                .add_modifier(Modifier::UNDERLINED | Modifier::BOLD)
        } else if state.visited {
            self.system
                .style(Role::TextMuted)
                .add_modifier(Modifier::UNDERLINED)
        } else {
            self.system
                .style(Role::Link)
                .add_modifier(Modifier::UNDERLINED)
        };
        buffer.set_stringn(
            area.x,
            area.y,
            take_display_cols(&text, usize::from(area.width)),
            usize::from(area.width),
            style,
        );
        // Same acknowledgement as every other copy site: one mark, one
        // duration, one tier rule (`style::ActionFlash`).
        let elapsed = self.system.elapsed_ms();
        if state.copied.is_lit(elapsed) {
            let mark = self
                .system
                .glyphs
                .resolve(crate::style::Glyph::Success)
                .text;
            let width = u16::try_from(display_cols(mark)).unwrap_or(1);
            if area.width > width {
                let mark_style = self.system.style(Role::Success);
                buffer.set_stringn(
                    area.right().saturating_sub(width),
                    area.y,
                    mark,
                    usize::from(width),
                    mark_style,
                );
            }
        }
    }

    /// Keys: Enter open, `p` preview, `c` copy, `g` jump anchor.
    pub fn handle_key(
        &self,
        state: &mut SourceCitationState,
        key: KeyEvent,
    ) -> SourceCitationOutcome {
        if !state.focused || !key.is_press() {
            return SourceCitationOutcome::Ignored;
        }
        match key.code {
            KeyCode::Enter | KeyCode::Char('o') => self.open_outcome(state),
            KeyCode::Char('p') => SourceCitationOutcome::PreviewRequested {
                id: self.source.id.clone(),
            },
            KeyCode::Char('c')
                if key.modifiers.contains(KeyModifiers::CONTROL) || key.modifiers.is_empty() =>
            {
                // A citation that copies silently looks like a citation that
                // ignored the key.
                state.copied.fire(self.system.elapsed_ms());
                SourceCitationOutcome::CopyRequested {
                    id: self.source.id.clone(),
                    text: self.source.copy_text(),
                }
            }
            KeyCode::Char('g') => {
                if let Some(a) = self.source.markdown_anchor {
                    SourceCitationOutcome::JumpToAnchor {
                        id: self.source.id.clone(),
                        anchor: a,
                    }
                } else {
                    SourceCitationOutcome::Ignored
                }
            }
            _ => SourceCitationOutcome::Ignored,
        }
    }

    /// Mouse click.
    pub fn handle_mouse(
        &self,
        state: &mut SourceCitationState,
        event: MouseEvent,
    ) -> SourceCitationOutcome {
        if event.kind != MouseEventKind::Down(MouseButton::Left) {
            if event.kind == MouseEventKind::Moved && state.area.contains(event.position) {
                state.hovered = true;
                return SourceCitationOutcome::Ignored;
            }
            return SourceCitationOutcome::Ignored;
        }
        if !state.area.contains(event.position) && state.area.width > 0 {
            return SourceCitationOutcome::Ignored;
        }
        state.focused = true;
        self.open_outcome(state)
    }

    fn open_outcome(&self, state: &mut SourceCitationState) -> SourceCitationOutcome {
        if !self.can_open() {
            // still allow preview
            return SourceCitationOutcome::PreviewRequested {
                id: self.source.id.clone(),
            };
        }
        state.visited = true;
        SourceCitationOutcome::OpenRequested {
            id: self.source.id.clone(),
            destination: self.source.destination.clone(),
            external: self.source.external,
        }
    }
}

fn effective_availability(source: &CitationSource, offline: bool) -> CitationAvailability {
    if offline && source.external {
        return CitationAvailability::Offline;
    }
    if source.sensitive {
        return CitationAvailability::Restricted;
    }
    source.availability
}

fn truncate_dest(dest: &str, max_cols: usize, ellipsis: &str) -> String {
    crate::text::truncate_cols(dest, max_cols, ellipsis).into_owned()
}

// ── CitationList ────────────────────────────────────────────────────────────

/// Expandable list of sources.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CitationListState {
    /// Expanded (show rows) vs one-line summary.
    pub expanded: bool,
    /// Selected index into visible flat rows.
    pub cursor: usize,
    /// Focused.
    pub focused: bool,
    /// Accepts input.
    accepts_input: bool,
    /// Show duplicate children for group keys.
    pub expanded_groups: BTreeMap<String, bool>,
    /// Offline mode.
    pub offline: bool,
    /// No hyperlink capability.
    pub no_hyperlink: bool,
    /// Row hit regions from last paint.
    pub row_hits: Vec<(String, Rect)>,
}

impl Default for CitationListState {
    fn default() -> Self {
        Self::new()
    }
}

impl CitationListState {
    /// Collapsed summary.
    #[must_use]
    pub fn new() -> Self {
        Self {
            expanded: false,
            cursor: 0,
            focused: true,
            accepts_input: true,
            expanded_groups: BTreeMap::new(),
            offline: false,
            no_hyperlink: false,
            row_hits: Vec::new(),
        }
    }

    /// Gate.
    pub fn set_accepts_input(&mut self, on: bool) {
        self.accepts_input = on;
    }

    /// Expand list.
    pub fn expand(&mut self) {
        self.expanded = true;
    }

    /// Collapse.
    pub fn collapse(&mut self) {
        self.expanded = false;
    }

    /// Toggle.
    pub fn toggle_expanded(&mut self) -> CitationListOutcome {
        self.expanded = !self.expanded;
        CitationListOutcome::ExpandChanged {
            expanded: self.expanded,
        }
    }

    /// Visible row ids (primary only, or +duplicates when group expanded).
    #[must_use]
    pub fn visible_ids<'a>(&self, sources: &'a [CitationSource]) -> Vec<&'a str> {
        let groups = group_citations(sources);
        let mut ids = Vec::new();
        for g in &groups {
            if let Some(s) = sources.get(g.primary) {
                ids.push(s.id.as_str());
            }
            let open = self.expanded_groups.get(&g.key).copied().unwrap_or(false);
            if open {
                for &i in &g.duplicates {
                    if let Some(s) = sources.get(i) {
                        ids.push(s.id.as_str());
                    }
                }
            }
        }
        // if no grouping needed, preserve source order
        if groups.len() == sources.len() {
            return sources.iter().map(|s| s.id.as_str()).collect();
        }
        ids
    }

    /// Keys.
    pub fn handle_key(&mut self, key: KeyEvent, sources: &[CitationSource]) -> CitationListOutcome {
        if !self.accepts_input || !self.focused || !key.is_press() {
            return CitationListOutcome::Ignored;
        }
        if !self.expanded {
            match key.code {
                KeyCode::Enter | KeyCode::Char(' ') | KeyCode::Right | KeyCode::Char('l') => {
                    return self.toggle_expanded();
                }
                _ => return CitationListOutcome::Ignored,
            }
        }
        let ids = self.visible_ids(sources);
        if ids.is_empty() {
            return CitationListOutcome::Ignored;
        }
        match key.code {
            KeyCode::Left | KeyCode::Char('h') | KeyCode::Esc => self.toggle_expanded(),
            KeyCode::Down | KeyCode::Char('j') => {
                self.cursor = (self.cursor + 1).min(ids.len().saturating_sub(1));
                CitationListOutcome::SelectionChanged {
                    id: ids[self.cursor].to_string(),
                }
            }
            KeyCode::Up | KeyCode::Char('k') => {
                self.cursor = self.cursor.saturating_sub(1);
                CitationListOutcome::SelectionChanged {
                    id: ids[self.cursor].to_string(),
                }
            }
            KeyCode::Enter => {
                let id = ids[self.cursor.min(ids.len() - 1)];
                let Some(src) = sources.iter().find(|s| s.id == id) else {
                    return CitationListOutcome::Ignored;
                };
                let avail = effective_availability(src, self.offline);
                if avail.can_open() && !src.sensitive {
                    CitationListOutcome::Citation(SourceCitationOutcome::OpenRequested {
                        id: src.id.clone(),
                        destination: src.destination.clone(),
                        external: src.external,
                    })
                } else {
                    CitationListOutcome::Citation(SourceCitationOutcome::PreviewRequested {
                        id: src.id.clone(),
                    })
                }
            }
            KeyCode::Char('p') => {
                let id = ids[self.cursor.min(ids.len() - 1)];
                CitationListOutcome::Citation(SourceCitationOutcome::PreviewRequested {
                    id: id.to_string(),
                })
            }
            KeyCode::Char('c') => {
                let id = ids[self.cursor.min(ids.len() - 1)];
                if let Some(src) = sources.iter().find(|s| s.id == id) {
                    CitationListOutcome::Citation(SourceCitationOutcome::CopyRequested {
                        id: src.id.clone(),
                        text: src.copy_text(),
                    })
                } else {
                    CitationListOutcome::Ignored
                }
            }
            KeyCode::Char('d') => {
                // expand duplicates for current
                let id = ids[self.cursor.min(ids.len() - 1)];
                if let Some(src) = sources.iter().find(|s| s.id == id) {
                    let key = src.effective_group_key().to_string();
                    let cur = self.expanded_groups.get(&key).copied().unwrap_or(false);
                    self.expanded_groups.insert(key.clone(), !cur);
                    CitationListOutcome::GroupExpanded { key }
                } else {
                    CitationListOutcome::Ignored
                }
            }
            _ => CitationListOutcome::Ignored,
        }
    }
}

/// Citation list paint.
#[derive(Debug, Clone, Copy)]
pub struct CitationList<'a> {
    sources: &'a [CitationSource],
    system: &'a DesignSystem,
    title: Option<&'a str>,
    show_destination: DestinationDisplay,
}

impl<'a> CitationList<'a> {
    /// Sources + system.
    #[must_use]
    pub const fn new(sources: &'a [CitationSource], system: &'a DesignSystem) -> Self {
        Self {
            sources,
            system,
            title: None,
            show_destination: DestinationDisplay::Auto,
        }
    }

    /// Destination display policy for expanded rows.
    #[must_use]
    pub const fn show_destination(mut self, d: DestinationDisplay) -> Self {
        self.show_destination = d;
        self
    }

    /// Title.
    pub const fn title(mut self, t: &'a str) -> Self {
        self.title = Some(t);
        self
    }

    /// Summary line when collapsed.
    #[must_use]
    pub fn summary_text(&self, state: &CitationListState) -> String {
        let n = self.sources.len();
        let groups = group_citations(self.sources);
        let mark = if state.expanded { "▾" } else { "▸" };
        let title = self.title.unwrap_or("Sources");
        if groups.len() < n {
            format!("{mark} {title} ({n}, {} unique)", groups.len())
        } else {
            format!("{mark} {title} ({n})")
        }
    }

    /// Paint.
    pub fn paint(&self, area: Rect, buffer: &mut Buffer, state: &mut CitationListState) {
        state.row_hits.clear();
        if area.is_empty() {
            return;
        }
        let mut y = area.y;
        // summary
        let summary = self.summary_text(state);
        let style = if state.focused {
            self.system.style(Role::TextStrong)
        } else {
            self.system.style(Role::TextMuted)
        };
        buffer.set_stringn(
            area.x,
            y,
            take_display_cols(&summary, usize::from(area.width)),
            usize::from(area.width),
            style,
        );
        y = y.saturating_add(1);
        if !state.expanded || y >= area.bottom() {
            return;
        }

        let ids = state.visible_ids(self.sources);
        for (i, id) in ids.iter().enumerate() {
            if y >= area.bottom() {
                break;
            }
            let Some(src) = self.sources.iter().find(|s| s.id == *id) else {
                continue;
            };
            let selected = state.focused && i == state.cursor;
            let mark = " ";
            let g = src.kind.glyph();
            let mut line = format!("{mark}{} {} {}", src.inline_label(false), g, src.title);
            if src.shows_destination(self.show_destination, state.no_hyperlink) {
                line.push(' ');
                line.push_str(&truncate_dest(
                    &src.destination,
                    28,
                    self.system.glyphs.ellipsis(),
                ));
            }
            let meta = src.meta_line();
            if !meta.is_empty() && area.width > 40 {
                line.push_str(" · ");
                line.push_str(&meta);
            }
            let avail = effective_availability(src, state.offline);
            let base_style = if !avail.can_open() {
                self.system.style(Role::TextMuted)
            } else {
                self.system.style(Role::Text)
            };
            let chrome = crate::widgets::row_chrome::RowChrome::resolve(
                self.system,
                ListRowVisualState {
                    selected,
                    focused: selected,
                    enabled: avail.can_open(),
                    ..Default::default()
                },
            );
            let row_style = chrome.label_style(base_style);
            buffer.set_stringn(
                area.x,
                y,
                take_display_cols(&line, usize::from(area.width)),
                usize::from(area.width),
                row_style,
            );
            chrome.paint(buffer, Rect::new(area.x, y, area.width, 1));
            state.row_hits.push((
                src.id.clone(),
                Rect {
                    x: area.x,
                    y,
                    width: area.width,
                    height: 1,
                },
            ));
            y = y.saturating_add(1);
        }
    }
}

/// Example sources for stories/tests.
#[must_use]
pub fn example_citations() -> Vec<CitationSource> {
    vec![
        CitationSource::file(
            "c1",
            "1",
            "message_thread.rs",
            "crates/termrock/src/widgets/message_thread.rs",
        )
        .range(SourceAnchor::range(10, 40))
        .provenance(
            CitationProvenance::default()
                .confidence(92)
                .provenance("rag"),
        )
        .preview("project-to-lines over Transcript"),
        CitationSource::url(
            "c2",
            "2",
            "Glow docs",
            "https://github.com/charmbracelet/glow",
        )
        .kind(CitationSourceType::Docs)
        .provenance(CitationProvenance::default().confidence(70)),
        CitationSource::url(
            "c3",
            "3",
            "Glow mirror",
            "https://github.com/charmbracelet/glow",
        )
        .kind(CitationSourceType::Docs)
        .group_key("https://github.com/charmbracelet/glow"),
        CitationSource::file("c4", "4", "secret.env", "/home/user/secret.env")
            .sensitive(true)
            .availability(CitationAvailability::Restricted),
        CitationSource::url("c5", "5", "API ref", "https://api.example.com/v1")
            .availability(CitationAvailability::Offline),
    ]
}

// ── Bench ───────────────────────────────────────────────────────────────────

/// List sizes.
pub mod bench {
    /// Citations.
    pub const CITATION_COUNT: usize = 80;
    /// Paint frames.
    pub const PAINT_FRAMES: u32 = 24;
}

// ── Tests ───────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use crate::style::DesignSystem;

    #[test]
    fn raw_destination_always_in_copy() {
        let c = CitationSource::url("u", "1", "Title", "https://evil.example/path?q=1");
        let t = c.copy_text();
        assert!(t.contains("https://evil.example/path?q=1"));
        assert!(t.contains("Title"));
    }

    #[test]
    fn external_shows_dest_in_decorated_auto() {
        let c = CitationSource::url("u", "1", "T", "https://example.com/x");
        let system = DesignSystem::default();
        let sc = SourceCitation::new(&c, &system).show_destination(DestinationDisplay::Auto);
        let d = sc.decorated();
        assert!(d.contains("example.com") || d.contains("https"));
    }

    #[test]
    fn offline_blocks_open() {
        let c = CitationSource::url("u", "1", "T", "https://example.com");
        let system = DesignSystem::default();
        let sc = SourceCitation::new(&c, &system).offline(true);
        assert!(!sc.can_open());
        let mut st = SourceCitationState::new();
        st.focused = true;
        let out = sc.handle_key(&mut st, KeyEvent::new(KeyCode::Enter, KeyModifiers::NONE));
        assert!(matches!(
            out,
            SourceCitationOutcome::PreviewRequested { .. }
        ));
    }

    #[test]
    fn group_duplicates() {
        let src = example_citations();
        let g = group_citations(&src);
        // two glow urls share group
        let glow = g
            .iter()
            .find(|x| x.key.contains("glow"))
            .expect("glow group");
        assert!(glow.count() >= 2);
    }

    #[test]
    fn list_nav_and_copy() {
        let src = example_citations();
        let mut st = CitationListState::new();
        st.expand();
        let out = st.handle_key(KeyEvent::new(KeyCode::Down, KeyModifiers::NONE), &src);
        assert!(matches!(out, CitationListOutcome::SelectionChanged { .. }));
        let out = st.handle_key(KeyEvent::new(KeyCode::Char('c'), KeyModifiers::NONE), &src);
        assert!(matches!(
            out,
            CitationListOutcome::Citation(SourceCitationOutcome::CopyRequested { ref text, .. })
                if text.contains("http") || text.contains("message_thread")
        ));
    }

    #[test]
    fn stream_bridge_roundtrip() {
        let sc = StreamCitation::new("x", "[9]").href("https://a.example");
        let c = citation_from_stream(&sc, 0);
        assert!(c.external);
        let back = citation_to_stream(&c);
        assert_eq!(back.id, "x");
    }

    #[test]
    fn jump_to_markdown_anchor() {
        let c = CitationSource::file("f", "1", "lib.rs", "lib.rs")
            .markdown_anchor(SourceAnchor::range(3, 5));
        let system = DesignSystem::default();
        let mut st = SourceCitationState::new();
        st.focused = true;
        let out = SourceCitation::new(&c, &system).handle_key(
            &mut st,
            KeyEvent::new(KeyCode::Char('g'), KeyModifiers::NONE),
        );
        assert!(matches!(
            out,
            SourceCitationOutcome::JumpToAnchor {
                anchor: SourceAnchor {
                    line_start: 3,
                    line_end: 5
                },
                ..
            }
        ));
    }

    #[test]
    fn paint_list_and_inline() {
        let system = DesignSystem::default();
        let src = example_citations();
        let mut list = CitationListState::new();
        list.expand();
        let area = Rect::new(0, 0, 60, 12);
        let mut buf = Buffer::empty(area);
        for _ in 0..bench::PAINT_FRAMES {
            CitationList::new(&src, &system)
                .title("Sources")
                .paint(area, &mut buf, &mut list);
        }
        assert!(!list.row_hits.is_empty());
        let mut ist = SourceCitationState::new();
        ist.focused = true;
        SourceCitation::new(&src[0], &system).paint(Rect::new(0, 0, 20, 1), &mut buf, &mut ist);
    }

    #[test]
    fn large_list_bench() {
        let system = DesignSystem::default();
        let mut src = Vec::with_capacity(bench::CITATION_COUNT);
        for i in 0..bench::CITATION_COUNT {
            src.push(
                CitationSource::file(
                    format!("c{i}"),
                    format!("{i}"),
                    format!("file{i}.rs"),
                    format!("src/f{i}.rs"),
                )
                .range(SourceAnchor::line(i as u32 + 1)),
            );
        }
        let mut st = CitationListState::new();
        st.expand();
        let area = Rect::new(0, 0, 48, 20);
        let mut buf = Buffer::empty(area);
        CitationList::new(&src, &system).paint(area, &mut buf, &mut st);
        assert_eq!(group_citations(&src).len(), bench::CITATION_COUNT);
    }

    #[test]
    fn never_writes_osc() {
        let src = include_str!("citation.rs");
        let body = src.split("#[cfg(test)]").next().unwrap_or(src);
        // may import encode helpers for composition docs but must not call PTY write
        for forbidden in ["std::io::Write", "stdout()", "write_all"] {
            assert!(!body.contains(forbidden), "must not {forbidden}");
        }
    }

    #[test]
    fn accepts_input_gate() {
        let src = example_citations();
        let mut st = CitationListState::new();
        st.set_accepts_input(false);
        st.expand();
        assert!(matches!(
            st.handle_key(KeyEvent::new(KeyCode::Down, KeyModifiers::NONE), &src),
            CitationListOutcome::Ignored
        ));
    }
}
