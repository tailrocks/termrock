// SPDX-FileCopyrightText: 2026 Alexey Zhokhov
// SPDX-License-Identifier: Apache-2.0

//! **FileMention** and **EntityMention** — inline structured tokens for files,
//! symbols, agents, tools, sessions, and resources.
//!
//! **Mission.** Display label, canonical id/path, type glyph, validity,
//! stale/missing, preview, and removal. Integrate completion, keyboard
//! navigation, copy, and semantic descriptions. Cursor movement treats mention
//! tokens as **atomic** across text/token boundaries. Ambiguous names support
//! disambiguation lists. **No** provider-specific resource lookup — host
//! projects candidates and resolves validity.
//!
//! Research: editor mentions, chat mentions, agent file-reference syntax.
//!
//! **Composition**
//! - Inline paint: [`InlineMention`] (Tag chrome)
//! - Draft model: [`MentionDraft`] atomic segments
//! - Completion: [`mention_to_completion_candidate`] + [`CompletionMenu`]
//! - Chips: [`MentionRef::to_composer_label`] / PromptComposer mention chips
use ratatui_core::{buffer::Buffer, layout::Rect};

use crate::{
    input::{KeyCode, KeyEvent, KeyModifiers},
    style::{DesignSystem, Role},
    text::{contains_lower_all, take_display_cols},
    widgets::{
        completion_menu::CompletionCandidate,
        tag_chip::{Tag, TagOutcome, TagState, TokenPart, TokenParts, TokenStatus},
    },
};

/// Default `@` file/entity trigger.
pub const MENTION_TRIGGER_AT: char = '@';
/// Alternate symbol trigger (entity/symbol).
pub const MENTION_TRIGGER_HASH: char = '#';
/// Overlay id hint for file mention completion.
pub const FILE_MENTION_OVERLAY_ID: &str = "termrock.file_mention";
/// Overlay id hint for entity mention completion.
pub const ENTITY_MENTION_OVERLAY_ID: &str = "termrock.entity_mention";
/// Max disambiguation rows shown inline.
pub const MENTION_DISAMBIG_MAX: usize = 8;

// ── Domain ──────────────────────────────────────────────────────────────────

/// Which mention surface owns the token.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[non_exhaustive]
pub enum MentionFamily {
    /// File / path / symbol mentions (`FileMention`).
    File,
    /// Agent / tool / session / resource (`EntityMention`).
    Entity,
}

impl MentionFamily {
    /// Stable id.
    #[must_use]
    pub const fn id(self) -> &'static str {
        match self {
            Self::File => "file",
            Self::Entity => "entity",
        }
    }
}

/// Concrete mention type (glyph + completion kind).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
#[non_exhaustive]
pub enum MentionType {
    /// Workspace file path.
    #[default]
    File,
    /// Directory.
    Directory,
    /// Code symbol (fn, type, …).
    Symbol,
    /// Agent identity.
    Agent,
    /// Tool / capability.
    Tool,
    /// Session / conversation.
    Session,
    /// Generic resource handle.
    Resource,
    /// User / account.
    User,
    /// Other host kind.
    Other,
}

impl MentionType {
    /// Stable id.
    #[must_use]
    pub const fn id(self) -> &'static str {
        match self {
            Self::File => "file",
            Self::Directory => "directory",
            Self::Symbol => "symbol",
            Self::Agent => "agent",
            Self::Tool => "tool",
            Self::Session => "session",
            Self::Resource => "resource",
            Self::User => "user",
            Self::Other => "other",
        }
    }

    /// Family.
    #[must_use]
    pub const fn family(self) -> MentionFamily {
        match self {
            Self::File | Self::Directory | Self::Symbol => MentionFamily::File,
            Self::Agent
            | Self::Tool
            | Self::Session
            | Self::Resource
            | Self::User
            | Self::Other => MentionFamily::Entity,
        }
    }

    /// Glyph (emoji or letter).
    #[must_use]
    pub const fn glyph(self, ascii: bool) -> &'static str {
        if ascii {
            match self {
                Self::File => "F",
                Self::Directory => "D",
                Self::Symbol => "S",
                Self::Agent => "A",
                Self::Tool => "T",
                Self::Session => "H",
                Self::Resource => "R",
                Self::User => "U",
                Self::Other => "?",
            }
        } else {
            match self {
                Self::File => "▫",
                Self::Directory => "▪",
                Self::Symbol => "◇",
                Self::Agent => "◆",
                Self::Tool => "⚙",
                Self::Session => "◉",
                Self::Resource => "▣",
                Self::User => "◎",
                Self::Other => "·",
            }
        }
    }
}

/// Validity / resolution state (host-projected; no TermRock lookup).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
#[non_exhaustive]
pub enum MentionValidity {
    /// Resolved and current.
    #[default]
    Valid,
    /// Was valid; host marked stale (moved/renamed).
    Stale,
    /// Missing / not found.
    Missing,
    /// Multiple matches; needs disambiguation.
    Ambiguous,
    /// Not yet resolved.
    Unknown,
}

impl MentionValidity {
    /// Stable id.
    #[must_use]
    pub const fn id(self) -> &'static str {
        match self {
            Self::Valid => "valid",
            Self::Stale => "stale",
            Self::Missing => "missing",
            Self::Ambiguous => "ambiguous",
            Self::Unknown => "unknown",
        }
    }

    /// Map to token paint status.
    #[must_use]
    pub const fn token_status(self) -> TokenStatus {
        match self {
            Self::Missing => TokenStatus::Error,
            Self::Stale | Self::Ambiguous | Self::Unknown => TokenStatus::Loading,
            Self::Valid => TokenStatus::Default,
        }
    }

    /// Status mark for chrome.
    #[must_use]
    pub fn mark(self, ascii: bool) -> &'static str {
        match self {
            Self::Valid => "",
            Self::Stale => {
                if ascii {
                    "~"
                } else {
                    "≈"
                }
            }
            Self::Missing => {
                if ascii {
                    "!"
                } else {
                    "⚠"
                }
            }
            Self::Ambiguous => {
                if ascii {
                    "?"
                } else {
                    "¿"
                }
            }
            Self::Unknown => {
                if ascii {
                    "."
                } else {
                    "…"
                }
            }
        }
    }
}

/// One disambiguation choice (host-projected; same label, different canonical).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MentionDisambiguator {
    /// Stable choice id.
    pub id: String,
    /// Short label (basename, agent short name).
    pub label: String,
    /// Distinguishing detail (parent path, provider, …) — safe for display.
    pub detail: Option<String>,
}

impl MentionDisambiguator {
    /// Construct.
    #[must_use]
    pub fn new(id: impl Into<String>, label: impl Into<String>) -> Self {
        Self {
            id: id.into(),
            label: label.into(),
            detail: None,
        }
    }

    /// Detail.
    #[must_use]
    pub fn detail(mut self, d: impl Into<String>) -> Self {
        self.detail = Some(d.into());
        self
    }
}

/// Canonical mention reference (file or entity).
///
/// Host owns path resolution and provider lookups. TermRock never fetches.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MentionRef {
    /// Stable token id (draft identity).
    pub id: String,
    /// Type.
    pub kind: MentionType,
    /// Display label (short; grapheme-safe).
    pub label: String,
    /// Canonical id or path (may be redacted when sensitive).
    pub canonical: String,
    /// Validity.
    pub validity: MentionValidity,
    /// Disambiguation options when ambiguous.
    pub disambiguators: Vec<MentionDisambiguator>,
    /// Selected disambiguator index.
    pub disambiguation_index: Option<usize>,
    /// Optional short preview (file head, agent blurb) — never secrets.
    pub preview: Option<String>,
    /// Removable inline.
    pub removable: bool,
    /// Redact canonical in semantic summaries.
    pub sensitive: bool,
}

impl MentionRef {
    /// File mention.
    #[must_use]
    pub fn file(id: impl Into<String>, label: impl Into<String>, path: impl Into<String>) -> Self {
        Self {
            id: id.into(),
            kind: MentionType::File,
            label: label.into(),
            canonical: path.into(),
            validity: MentionValidity::Valid,
            disambiguators: Vec::new(),
            disambiguation_index: None,
            preview: None,
            removable: true,
            sensitive: false,
        }
    }

    /// Symbol mention.
    #[must_use]
    pub fn symbol(
        id: impl Into<String>,
        label: impl Into<String>,
        canonical: impl Into<String>,
    ) -> Self {
        Self {
            id: id.into(),
            kind: MentionType::Symbol,
            label: label.into(),
            canonical: canonical.into(),
            validity: MentionValidity::Valid,
            disambiguators: Vec::new(),
            disambiguation_index: None,
            preview: None,
            removable: true,
            sensitive: false,
        }
    }

    /// Entity (agent/tool/session/resource).
    #[must_use]
    pub fn entity(
        id: impl Into<String>,
        kind: MentionType,
        label: impl Into<String>,
        canonical: impl Into<String>,
    ) -> Self {
        debug_assert_eq!(kind.family(), MentionFamily::Entity);
        Self {
            id: id.into(),
            kind,
            label: label.into(),
            canonical: canonical.into(),
            validity: MentionValidity::Valid,
            disambiguators: Vec::new(),
            disambiguation_index: None,
            preview: None,
            removable: true,
            sensitive: false,
        }
    }

    /// Validity.
    #[must_use]
    pub const fn validity(mut self, v: MentionValidity) -> Self {
        self.validity = v;
        self
    }

    /// Disambiguators (marks validity Ambiguous when non-empty).
    #[must_use]
    pub fn with_disambiguators(mut self, list: Vec<MentionDisambiguator>) -> Self {
        if !list.is_empty() {
            self.validity = MentionValidity::Ambiguous;
        }
        self.disambiguators = list;
        self
    }

    /// Sensitive.
    #[must_use]
    pub const fn sensitive(mut self, on: bool) -> Self {
        self.sensitive = on;
        self
    }

    /// Family.
    #[must_use]
    pub const fn family(&self) -> MentionFamily {
        self.kind.family()
    }

    /// Compact display label for inline paint.
    #[must_use]
    pub fn display_label(&self, ascii: bool) -> String {
        let g = self.kind.glyph(ascii);
        let mark = self.validity.mark(ascii);
        let mut s = format!("{g}{}", self.label);
        if !mark.is_empty() {
            s.push(mark.chars().next().unwrap_or('?'));
        }
        if matches!(self.validity, MentionValidity::Ambiguous) && self.disambiguators.len() > 1 {
            s.push_str(&format!("×{}", self.disambiguators.len()));
        }
        s
    }

    /// Apply selected disambiguator (host confirms resolution).
    pub fn apply_disambiguation(&mut self, index: usize) -> bool {
        let Some(d) = self.disambiguators.get(index) else {
            return false;
        };
        self.label = d.label.clone();
        self.canonical = d.id.clone();
        if let Some(detail) = &d.detail {
            self.canonical = format!("{} · {detail}", d.id);
        }
        // Prefer id as canonical
        self.canonical = d.id.clone();
        self.disambiguation_index = Some(index);
        self.validity = MentionValidity::Valid;
        true
    }

    /// Serialize to a neutral inline form for plain-text export (not provider-specific).
    ///
    /// Format: `@[kind:id|label]` — host may reparse with [`parse_mention_markup`].
    #[must_use]
    pub fn to_markup(&self) -> String {
        format!(
            "@[{}:{}|{}]",
            self.kind.id(),
            escape_markup_part(&self.id),
            escape_markup_part(&self.label)
        )
    }
}

/// Semantic description for a11y / recordings — **no** full sensitive paths or secrets.
#[must_use]
pub fn mention_semantic_description(m: &MentionRef) -> String {
    let canon = if m.sensitive {
        redacted_canonical(&m.canonical)
    } else {
        m.canonical.as_str()
    };
    let mut s = format!(
        "mention {} {} label={} canonical={} {}",
        m.family().id(),
        m.kind.id(),
        m.label,
        canon,
        m.validity.id()
    );
    if matches!(m.validity, MentionValidity::Ambiguous) {
        s.push_str(&format!(" choices={}", m.disambiguators.len()));
    }
    s
}

// ── FileMention / EntityMention facades ─────────────────────────────────────

/// File / path / symbol mention token (family File).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FileMention {
    /// Shared ref.
    pub mention: MentionRef,
}

impl FileMention {
    /// Path file.
    #[must_use]
    pub fn path(id: impl Into<String>, label: impl Into<String>, path: impl Into<String>) -> Self {
        Self {
            mention: MentionRef::file(id, label, path),
        }
    }

    /// Missing path.
    #[must_use]
    pub fn missing(
        id: impl Into<String>,
        label: impl Into<String>,
        path: impl Into<String>,
    ) -> Self {
        Self {
            mention: MentionRef::file(id, label, path).validity(MentionValidity::Missing),
        }
    }

    /// Ambiguous basename with choices.
    #[must_use]
    pub fn ambiguous(
        id: impl Into<String>,
        label: impl Into<String>,
        choices: Vec<MentionDisambiguator>,
    ) -> Self {
        let label = label.into();
        Self {
            mention: MentionRef::file(id, label.clone(), label).with_disambiguators(choices),
        }
    }

    /// Borrow ref.
    #[must_use]
    pub const fn as_ref(&self) -> &MentionRef {
        &self.mention
    }

    /// Mut ref.
    pub const fn as_mut(&mut self) -> &mut MentionRef {
        &mut self.mention
    }
}

/// Agent / tool / session / resource mention token (family Entity).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EntityMention {
    /// Shared ref.
    pub mention: MentionRef,
}

impl EntityMention {
    /// Agent.
    #[must_use]
    pub fn agent(
        id: impl Into<String>,
        label: impl Into<String>,
        canonical: impl Into<String>,
    ) -> Self {
        Self {
            mention: MentionRef::entity(id, MentionType::Agent, label, canonical),
        }
    }

    /// Tool.
    #[must_use]
    pub fn tool(
        id: impl Into<String>,
        label: impl Into<String>,
        canonical: impl Into<String>,
    ) -> Self {
        Self {
            mention: MentionRef::entity(id, MentionType::Tool, label, canonical),
        }
    }

    /// Borrow.
    #[must_use]
    pub const fn as_ref(&self) -> &MentionRef {
        &self.mention
    }
}

// ── Completion query / candidates ───────────────────────────────────────────

/// Active mention completion query extracted from plain draft text.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MentionQuery {
    /// Family preference from trigger / host filter.
    pub family: MentionFamily,
    /// Trigger character.
    pub trigger: char,
    /// Text after trigger through cursor (no spaces).
    pub query: String,
    /// Byte offset of trigger in draft.
    pub trigger_byte: usize,
    /// Byte offset of cursor.
    pub cursor_byte: usize,
}

/// Host-ranked mention candidate for completion menus (no I/O).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MentionCandidate {
    /// Stable id.
    pub id: String,
    /// Display label.
    pub label: String,
    /// Type.
    pub kind: MentionType,
    /// Canonical path/id.
    pub canonical: String,
    /// Detail column.
    pub detail: Option<String>,
    /// Group header.
    pub group: Option<String>,
    /// Enabled.
    pub enabled: bool,
    /// Optional docs preview.
    pub documentation: Option<String>,
}

impl MentionCandidate {
    /// Construct.
    #[must_use]
    pub fn new(
        id: impl Into<String>,
        label: impl Into<String>,
        kind: MentionType,
        canonical: impl Into<String>,
    ) -> Self {
        Self {
            id: id.into(),
            label: label.into(),
            kind,
            canonical: canonical.into(),
            detail: None,
            group: None,
            enabled: true,
            documentation: None,
        }
    }

    /// Detail.
    #[must_use]
    pub fn detail(mut self, d: impl Into<String>) -> Self {
        self.detail = Some(d.into());
        self
    }

    /// Group.
    #[must_use]
    pub fn group(mut self, g: impl Into<String>) -> Self {
        self.group = Some(g.into());
        self
    }

    /// Convert to owned mention token on commit.
    #[must_use]
    pub fn to_mention_ref(&self) -> MentionRef {
        MentionRef {
            id: self.id.clone(),
            kind: self.kind,
            label: self.label.clone(),
            canonical: self.canonical.clone(),
            validity: MentionValidity::Valid,
            disambiguators: Vec::new(),
            disambiguation_index: None,
            preview: self.documentation.clone(),
            removable: true,
            sensitive: false,
        }
    }

    /// Insert text for plain-text drafts (markup form).
    #[must_use]
    pub fn insert_markup(&self) -> String {
        self.to_mention_ref().to_markup()
    }
}

/// Detect `@` / `#` mention query before cursor (pure; no I/O).
#[must_use]
pub fn detect_mention_query(
    text: &str,
    cursor_byte: usize,
    family: MentionFamily,
) -> Option<MentionQuery> {
    let abs = cursor_byte.min(text.len());
    let head = &text[..abs];
    let bytes = head.as_bytes();
    let mut i = bytes.len();
    while i > 0 {
        i -= 1;
        let b = bytes[i];
        if b == b'@' || b == b'#' {
            let at_start = i == 0;
            let prev_ok =
                at_start || matches!(bytes[i - 1], b' ' | b'\n' | b'\t' | b'(' | b'[' | b'{');
            if !prev_ok {
                continue;
            }
            let trigger = b as char;
            let query = head[i + 1..].to_string();
            if query.chars().any(char::is_whitespace) {
                return None;
            }
            // # prefers symbol/entity; @ used for both — family is host preference
            let fam = if trigger == MENTION_TRIGGER_HASH {
                // symbols often under file family; entity can still filter
                family
            } else {
                family
            };
            return Some(MentionQuery {
                family: fam,
                trigger,
                query,
                trigger_byte: i,
                cursor_byte: abs,
            });
        }
        if b == b' ' || b == b'\n' || b == b'\t' {
            break;
        }
    }
    None
}

/// File-family detect (`@` / `#` → FileMention surface).
#[must_use]
pub fn detect_file_mention_query(text: &str, cursor_byte: usize) -> Option<MentionQuery> {
    detect_mention_query(text, cursor_byte, MentionFamily::File)
}

/// Entity-family detect.
#[must_use]
pub fn detect_entity_mention_query(text: &str, cursor_byte: usize) -> Option<MentionQuery> {
    detect_mention_query(text, cursor_byte, MentionFamily::Entity)
}

/// Project mention candidates into completion menu rows (labels live in candidates).
#[must_use]
pub fn mention_to_completion_candidate(c: &MentionCandidate) -> CompletionCandidate<'_, String> {
    let mut cand = CompletionCandidate::new(c.id.clone(), c.label.as_str())
        .kind(c.kind.id())
        .kind_glyph(c.kind.glyph(false))
        .enabled(c.enabled);
    if let Some(d) = &c.detail {
        cand = cand.detail(d.as_str());
    } else {
        cand = cand.detail(c.canonical.as_str());
    }
    if let Some(g) = &c.group {
        cand = cand.group(g.as_str());
    }
    if let Some(docs) = &c.documentation {
        cand = cand.documentation(docs.as_str());
    }
    cand
}

/// Filter candidates by query (case-insensitive label/canonical/id contains).
#[must_use]
pub fn filter_mention_candidates<'a>(
    candidates: &'a [MentionCandidate],
    query: &str,
    family: Option<MentionFamily>,
) -> Vec<&'a MentionCandidate> {
    let q = query.trim().to_ascii_lowercase();
    candidates
        .iter()
        .filter(|c| {
            if let Some(f) = family {
                if c.kind.family() != f {
                    return false;
                }
            }
            if q.is_empty() {
                return true;
            }
            contains_lower_all(&[c.label.as_str(), c.canonical.as_str(), c.id.as_str()], &q)
        })
        .collect()
}

/// Replace trigger..cursor span with mention markup.
#[must_use]
pub fn apply_mention_insert(draft: &str, query: &MentionQuery, insertion: &str) -> String {
    let start = query.trigger_byte.min(draft.len());
    let end = query.cursor_byte.min(draft.len()).max(start);
    let mut next = String::new();
    next.push_str(&draft[..start]);
    next.push_str(insertion);
    next.push_str(&draft[end..]);
    next
}

// ── Draft segments (atomic cursor) ──────────────────────────────────────────

/// One draft segment: plain text or atomic mention.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum MentionSegment {
    /// Free text.
    Text(String),
    /// Atomic mention token.
    Mention(MentionRef),
}

/// Cursor inside a mention draft.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MentionCursor {
    /// Within a text segment at byte offset.
    InText {
        /// Segment index.
        part: usize,
        /// Byte offset in text.
        byte: usize,
    },
    /// Caret is on the whole mention (atomic).
    OnMention {
        /// Segment index.
        part: usize,
    },
    /// After last segment (empty draft end).
    End,
}

impl Default for MentionCursor {
    fn default() -> Self {
        Self::End
    }
}

/// Mixed text + mention draft with atomic token navigation.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct MentionDraft {
    /// Segments in order.
    pub parts: Vec<MentionSegment>,
    /// Caret.
    pub cursor: MentionCursor,
}

impl MentionDraft {
    /// Empty.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// From plain text only.
    #[must_use]
    pub fn from_text(text: impl Into<String>) -> Self {
        let text = text.into();
        if text.is_empty() {
            return Self::new();
        }
        let len = text.len();
        Self {
            parts: vec![MentionSegment::Text(text)],
            cursor: MentionCursor::InText { part: 0, byte: len },
        }
    }
    /// Insert text at cursor (splits text segments; refuses inside mention).
    pub fn insert_text(&mut self, text: &str) {
        if text.is_empty() {
            return;
        }
        match self.cursor {
            MentionCursor::End => {
                if matches!(self.parts.last(), Some(MentionSegment::Text(_))) {
                    let part = self.parts.len() - 1;
                    if let Some(MentionSegment::Text(t)) = self.parts.get_mut(part) {
                        t.push_str(text);
                        self.cursor = MentionCursor::InText {
                            part,
                            byte: t.len(),
                        };
                    }
                } else {
                    self.parts.push(MentionSegment::Text(text.into()));
                    self.cursor = MentionCursor::InText {
                        part: self.parts.len() - 1,
                        byte: text.len(),
                    };
                }
            }
            MentionCursor::OnMention { part } => {
                // insert after token
                let insert_at = part + 1;
                self.parts
                    .insert(insert_at, MentionSegment::Text(text.into()));
                self.cursor = MentionCursor::InText {
                    part: insert_at,
                    byte: text.len(),
                };
            }
            MentionCursor::InText { part, byte } => {
                if let Some(MentionSegment::Text(t)) = self.parts.get_mut(part) {
                    let b = byte.min(t.len());
                    t.insert_str(b, text);
                    self.cursor = MentionCursor::InText {
                        part,
                        byte: b + text.len(),
                    };
                }
            }
        }
    }

    /// Insert mention at cursor (atomic unit).
    pub fn insert_mention(&mut self, mention: MentionRef) {
        match self.cursor {
            MentionCursor::End => {
                self.parts.push(MentionSegment::Mention(mention));
                self.cursor = MentionCursor::OnMention {
                    part: self.parts.len() - 1,
                };
            }
            MentionCursor::OnMention { part } => {
                let at = part + 1;
                self.parts.insert(at, MentionSegment::Mention(mention));
                self.cursor = MentionCursor::OnMention { part: at };
            }
            MentionCursor::InText { part, byte } => {
                let Some(MentionSegment::Text(t)) = self.parts.get(part) else {
                    return;
                };
                let t = t.clone();
                let b = byte.min(t.len());
                let left = t[..b].to_string();
                let right = t[b..].to_string();
                self.parts.remove(part);
                let mut idx = part;
                if !left.is_empty() {
                    self.parts.insert(idx, MentionSegment::Text(left));
                    idx += 1;
                }
                self.parts.insert(idx, MentionSegment::Mention(mention));
                let mention_part = idx;
                idx += 1;
                if !right.is_empty() {
                    self.parts.insert(idx, MentionSegment::Text(right));
                }
                self.cursor = MentionCursor::OnMention { part: mention_part };
            }
        }
    }

    /// Move caret left (atomic over mentions).
    pub fn move_left(&mut self) -> bool {
        match self.cursor {
            MentionCursor::End => {
                if self.parts.is_empty() {
                    return false;
                }
                let last = self.parts.len() - 1;
                match &self.parts[last] {
                    MentionSegment::Mention(_) => {
                        self.cursor = MentionCursor::OnMention { part: last };
                    }
                    MentionSegment::Text(t) => {
                        self.cursor = MentionCursor::InText {
                            part: last,
                            byte: t.len(),
                        };
                        return self.move_left();
                    }
                }
                true
            }
            MentionCursor::OnMention { part } => {
                if part == 0 {
                    // before first — stay or go to start of empty text
                    return false;
                }
                let prev = part - 1;
                match &self.parts[prev] {
                    MentionSegment::Mention(_) => {
                        self.cursor = MentionCursor::OnMention { part: prev };
                    }
                    MentionSegment::Text(t) => {
                        self.cursor = MentionCursor::InText {
                            part: prev,
                            byte: t.len(),
                        };
                    }
                }
                true
            }
            MentionCursor::InText { part, byte } => {
                if byte > 0 {
                    // step one char back
                    if let Some(MentionSegment::Text(t)) = self.parts.get(part) {
                        let b = prev_char_boundary(t, byte);
                        self.cursor = MentionCursor::InText { part, byte: b };
                        return true;
                    }
                }
                // leave text segment leftward
                if part == 0 {
                    return false;
                }
                let prev = part - 1;
                match &self.parts[prev] {
                    MentionSegment::Mention(_) => {
                        self.cursor = MentionCursor::OnMention { part: prev };
                    }
                    MentionSegment::Text(t) => {
                        self.cursor = MentionCursor::InText {
                            part: prev,
                            byte: t.len(),
                        };
                    }
                }
                true
            }
        }
    }

    /// Backspace: deletes one char or whole mention.
    pub fn delete_backward(&mut self) -> bool {
        match self.cursor {
            MentionCursor::End => {
                if self.parts.is_empty() {
                    return false;
                }
                // act as if at end of last
                let last = self.parts.len() - 1;
                match &self.parts[last] {
                    MentionSegment::Mention(_) => {
                        self.parts.remove(last);
                        self.cursor = if self.parts.is_empty() {
                            MentionCursor::End
                        } else {
                            MentionCursor::OnMention {
                                part: self.parts.len() - 1,
                            }
                        };
                        // fix: after remove, put cursor after previous
                        if self.parts.is_empty() {
                            self.cursor = MentionCursor::End;
                        } else {
                            let p = self.parts.len() - 1;
                            match &self.parts[p] {
                                MentionSegment::Text(t) => {
                                    self.cursor = MentionCursor::InText {
                                        part: p,
                                        byte: t.len(),
                                    };
                                }
                                MentionSegment::Mention(_) => {
                                    self.cursor = MentionCursor::OnMention { part: p };
                                }
                            }
                        }
                        true
                    }
                    MentionSegment::Text(_) => {
                        self.cursor = MentionCursor::InText {
                            part: last,
                            byte: match &self.parts[last] {
                                MentionSegment::Text(t) => t.len(),
                                _ => 0,
                            },
                        };
                        self.delete_backward()
                    }
                }
            }
            MentionCursor::OnMention { part } => {
                self.parts.remove(part);
                if self.parts.is_empty() {
                    self.cursor = MentionCursor::End;
                } else if part == 0 {
                    match &self.parts[0] {
                        MentionSegment::Text(_) => {
                            self.cursor = MentionCursor::InText { part: 0, byte: 0 };
                        }
                        MentionSegment::Mention(_) => {
                            self.cursor = MentionCursor::OnMention { part: 0 };
                        }
                    }
                } else {
                    let p = part - 1;
                    match &self.parts[p] {
                        MentionSegment::Text(t) => {
                            self.cursor = MentionCursor::InText {
                                part: p,
                                byte: t.len(),
                            };
                        }
                        MentionSegment::Mention(_) => {
                            self.cursor = MentionCursor::OnMention { part: p };
                        }
                    }
                }
                true
            }
            MentionCursor::InText { part, byte } => {
                if byte == 0 {
                    // delete previous mention whole
                    if part == 0 {
                        return false;
                    }
                    let prev = part - 1;
                    if matches!(self.parts.get(prev), Some(MentionSegment::Mention(_))) {
                        self.parts.remove(prev);
                        // part index shifts
                        self.cursor = MentionCursor::InText {
                            part: prev,
                            byte: 0,
                        };
                        // reindex: text was at part, now at prev if merged?
                        // simple: after remove, text segment index is part-1
                        if matches!(self.parts.get(prev), Some(MentionSegment::Text(_))) {
                            // if we had text at `part`, now at `prev` only if removed mention before it
                            self.cursor = MentionCursor::InText {
                                part: prev,
                                byte: 0,
                            };
                            // actually text is now at prev if mention was before...
                            // parts: [..., mention, text] remove mention -> text at prev
                            self.cursor = MentionCursor::InText {
                                part: prev,
                                byte: 0,
                            };
                        }
                        return true;
                    }
                    return false;
                }
                if let Some(MentionSegment::Text(t)) = self.parts.get_mut(part) {
                    let b = prev_char_boundary(t, byte);
                    t.replace_range(b..byte, "");
                    self.cursor = MentionCursor::InText { part, byte: b };
                    if t.is_empty() {
                        self.parts.remove(part);
                        self.cursor = if self.parts.is_empty() {
                            MentionCursor::End
                        } else if part == 0 {
                            MentionCursor::InText { part: 0, byte: 0 }
                        } else {
                            MentionCursor::OnMention { part: part - 1 }
                        };
                    }
                    return true;
                }
                false
            }
        }
    }

    /// Count mentions.
    #[must_use]
    pub fn mention_count(&self) -> usize {
        self.parts
            .iter()
            .filter(|p| matches!(p, MentionSegment::Mention(_)))
            .count()
    }
}

// ── Inline paint ────────────────────────────────────────────────────────────

/// Inline mention outcomes.
#[derive(Debug, Clone, PartialEq, Eq)]
#[non_exhaustive]
pub enum InlineMentionOutcome {
    /// Ignored.
    Ignored,
    /// Activated (open / preview intent).
    Activated {
        /// Id.
        id: String,
    },
    /// Remove token.
    Removed {
        /// Id.
        id: String,
    },
    /// Preview requested.
    PreviewRequested {
        /// Id.
        id: String,
    },
    /// Copy canonical/label (host copies; outcome has id only).
    CopyRequested {
        /// Id.
        id: String,
    },
    /// Open disambiguation.
    DisambiguateRequested {
        /// Id.
        id: String,
    },
    /// Disambiguation choice selected by index.
    DisambiguationSelected {
        /// Id.
        id: String,
        /// Choice index.
        index: usize,
    },
    /// Part focus.
    PartChanged(TokenPart),
}

/// Interaction state for one inline mention.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct InlineMentionState {
    /// Tag focus.
    pub tag: TagState,
    /// Disambiguation list open.
    pub disambiguation_open: bool,
    /// Cursor in disambiguation list.
    pub disambiguation_cursor: usize,
}

impl InlineMentionState {
    /// Fresh.
    #[must_use]
    pub const fn new() -> Self {
        Self {
            tag: TagState::new(),
            disambiguation_open: false,
            disambiguation_cursor: 0,
        }
    }

    /// Focus.
    pub const fn set_focused(&mut self, on: bool) {
        self.tag.set_focused(on);
        if !on {
            self.disambiguation_open = false;
        }
    }
}

/// Inline mention chrome (file or entity).
#[derive(Debug, Clone, Copy)]
pub struct InlineMention<'a> {
    mention: &'a MentionRef,
    system: &'a DesignSystem,
}

impl<'a> InlineMention<'a> {
    /// Mention + system.
    #[must_use]
    pub const fn new(mention: &'a MentionRef, system: &'a DesignSystem) -> Self {
        Self { mention, system }
    }

    /// ASCII glyphs.
    #[must_use]
    /// From file mention.
    pub const fn file(file: &'a FileMention, system: &'a DesignSystem) -> Self {
        Self::new(&file.mention, system)
    }

    /// Paint token.
    pub fn paint(
        &self,
        area: Rect,
        buffer: &mut Buffer,
        state: &mut InlineMentionState,
    ) -> TokenParts {
        let label = self.mention.display_label(false);
        let tag = if self.mention.removable {
            Tag::removable_tag(self.mention.id.as_str(), label.as_str(), self.system)
        } else {
            Tag::new(self.mention.id.as_str(), label.as_str(), self.system)
        }
        .status(self.mention.validity.token_status());
        let parts = tag.paint(area, buffer, &mut state.tag);
        if state.disambiguation_open && !self.mention.disambiguators.is_empty() {
            // host usually paints popover; optional one-line hint under token
            if area.height > 1 {
                let n = self.mention.disambiguators.len().min(MENTION_DISAMBIG_MAX);
                let hint = format!("?{} choices", n);
                buffer.set_stringn(
                    area.x,
                    area.y.saturating_add(1),
                    take_display_cols(&hint, usize::from(area.width)),
                    usize::from(area.width),
                    self.system.style(Role::TextMuted),
                );
            }
        }
        parts
    }
    /// Keys.
    pub fn handle_key(
        &self,
        state: &mut InlineMentionState,
        key: KeyEvent,
    ) -> InlineMentionOutcome {
        if key.is_release() {
            return InlineMentionOutcome::Ignored;
        }
        let is_press = key.is_press();
        if state.disambiguation_open {
            return self.handle_disambiguation_key(state, key);
        }
        if is_press
            && key.code == KeyCode::Char('c')
            && key.modifiers.contains(KeyModifiers::CONTROL)
        {
            return InlineMentionOutcome::CopyRequested {
                id: self.mention.id.clone(),
            };
        }
        if is_press
            && key.code == KeyCode::Char('p')
            && key.modifiers.contains(KeyModifiers::CONTROL)
        {
            return InlineMentionOutcome::PreviewRequested {
                id: self.mention.id.clone(),
            };
        }
        if is_press
            && key.code == KeyCode::Char('d')
            && key.modifiers.contains(KeyModifiers::CONTROL)
            && matches!(self.mention.validity, MentionValidity::Ambiguous)
        {
            state.disambiguation_open = true;
            state.disambiguation_cursor = self.mention.disambiguation_index.unwrap_or(0);
            return InlineMentionOutcome::DisambiguateRequested {
                id: self.mention.id.clone(),
            };
        }
        // Enter on ambiguous opens disambiguation
        if is_press
            && key.code == KeyCode::Enter
            && matches!(self.mention.validity, MentionValidity::Ambiguous)
            && state.tag.part == TokenPart::Body
        {
            state.disambiguation_open = true;
            return InlineMentionOutcome::DisambiguateRequested {
                id: self.mention.id.clone(),
            };
        }
        let label = self.mention.display_label(false);
        let tag = if self.mention.removable {
            Tag::removable_tag(self.mention.id.as_str(), label.as_str(), self.system)
        } else {
            Tag::new(self.mention.id.as_str(), label.as_str(), self.system)
        }
        .status(self.mention.validity.token_status());
        match tag.handle_key(&mut state.tag, key) {
            TagOutcome::Ignored => InlineMentionOutcome::Ignored,
            TagOutcome::Remove(id) => InlineMentionOutcome::Removed { id: id.to_string() },
            TagOutcome::PartChanged(p) => InlineMentionOutcome::PartChanged(p),
            TagOutcome::Activated(id) => InlineMentionOutcome::Activated { id: id.to_string() },
            TagOutcome::HoverChanged => InlineMentionOutcome::Ignored,
        }
    }

    fn handle_disambiguation_key(
        &self,
        state: &mut InlineMentionState,
        key: KeyEvent,
    ) -> InlineMentionOutcome {
        let n = self.mention.disambiguators.len().max(1);
        match key.code {
            KeyCode::Esc if key.is_press() => {
                state.disambiguation_open = false;
                InlineMentionOutcome::Ignored
            }
            KeyCode::Up | KeyCode::Char('k') => {
                state.disambiguation_cursor = state.disambiguation_cursor.saturating_sub(1);
                InlineMentionOutcome::Ignored
            }
            KeyCode::Down | KeyCode::Char('j') => {
                state.disambiguation_cursor = (state.disambiguation_cursor + 1).min(n - 1);
                InlineMentionOutcome::Ignored
            }
            KeyCode::Enter if key.is_press() => {
                let idx = state.disambiguation_cursor.min(n - 1);
                state.disambiguation_open = false;
                InlineMentionOutcome::DisambiguationSelected {
                    id: self.mention.id.clone(),
                    index: idx,
                }
            }
            _ => InlineMentionOutcome::Ignored,
        }
    }
}

// ── TokenStrip / semantic helpers ───────────────────────────────────────────

/// Parse `@[kind:id|label]` markup into a mention (best-effort).
#[must_use]
pub fn parse_mention_markup(s: &str) -> Option<MentionRef> {
    let s = s.strip_prefix("@[")?.strip_suffix(']')?;
    let (kind_id, rest) = s.split_once(':')?;
    let (id, label) = rest.split_once('|')?;
    let kind = match kind_id {
        "file" => MentionType::File,
        "directory" => MentionType::Directory,
        "symbol" => MentionType::Symbol,
        "agent" => MentionType::Agent,
        "tool" => MentionType::Tool,
        "session" => MentionType::Session,
        "resource" => MentionType::Resource,
        "user" => MentionType::User,
        _ => MentionType::Other,
    };
    Some(MentionRef {
        id: unescape_markup_part(id),
        kind,
        label: unescape_markup_part(label),
        canonical: unescape_markup_part(id),
        validity: MentionValidity::Valid,
        disambiguators: Vec::new(),
        disambiguation_index: None,
        preview: None,
        removable: true,
        sensitive: false,
    })
}

/// Parse mixed text with markup into a draft.
#[must_use]
pub fn parse_draft_with_mentions(text: &str) -> MentionDraft {
    let mut draft = MentionDraft::new();
    let mut rest = text;
    while !rest.is_empty() {
        if let Some(start) = rest.find("@[") {
            if start > 0 {
                draft.parts.push(MentionSegment::Text(rest[..start].into()));
            }
            if let Some(end_rel) = rest[start..].find(']') {
                let end = start + end_rel + 1;
                let chunk = &rest[start..end];
                if let Some(m) = parse_mention_markup(chunk) {
                    draft.parts.push(MentionSegment::Mention(m));
                } else {
                    draft.parts.push(MentionSegment::Text(chunk.into()));
                }
                rest = &rest[end..];
            } else {
                draft.parts.push(MentionSegment::Text(rest.into()));
                break;
            }
        } else {
            draft.parts.push(MentionSegment::Text(rest.into()));
            break;
        }
    }
    draft.cursor = MentionCursor::End;
    draft
}

// ── FileMention / EntityMention session helpers ─────────────────────────────

/// Lightweight open state for file mention completion (compose with CompletionMenu).
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct FileMentionState {
    /// Menu open.
    pub open: bool,
    /// Active query.
    pub query: Option<MentionQuery>,
    /// Accepts keyboard.
    pub accepts_input: bool,
}

impl FileMentionState {
    /// Fresh closed.
    #[must_use]
    pub fn new() -> Self {
        Self {
            open: false,
            query: None,
            accepts_input: true,
        }
    }

    /// Sync from draft text + cursor (host calls after edit).
    pub fn sync_from_draft(&mut self, text: &str, cursor_byte: usize) -> bool {
        if !self.accepts_input {
            return false;
        }
        match detect_file_mention_query(text, cursor_byte) {
            Some(q) => {
                let changed = self.query.as_ref() != Some(&q);
                self.query = Some(q);
                self.open = true;
                changed
            }
            None => {
                let was = self.open;
                self.open = false;
                self.query = None;
                was
            }
        }
    }
}

/// Entity mention completion session.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct EntityMentionState {
    /// Open.
    pub open: bool,
    /// Query.
    pub query: Option<MentionQuery>,
    /// Accepts input.
    pub accepts_input: bool,
}

// ── Helpers ─────────────────────────────────────────────────────────────────

fn escape_markup_part(s: &str) -> String {
    s.replace('\\', "\\\\")
        .replace('|', "\\|")
        .replace(']', "\\]")
}

fn unescape_markup_part(s: &str) -> String {
    let mut out = String::new();
    let mut chars = s.chars().peekable();
    while let Some(c) = chars.next() {
        if c == '\\' {
            if let Some(n) = chars.next() {
                out.push(n);
            }
        } else {
            out.push(c);
        }
    }
    out
}

fn redacted_canonical(c: &str) -> &str {
    c.rsplit(['/', '\\', ':']).next().unwrap_or("redacted")
}

fn prev_char_boundary(s: &str, byte: usize) -> usize {
    if byte == 0 {
        return 0;
    }
    let mut i = byte - 1;
    while i > 0 && !s.is_char_boundary(i) {
        i -= 1;
    }
    i
}

// ── Bench ───────────────────────────────────────────────────────────────────

/// Moderate mention draft / completion sizes.
pub mod bench {
    /// Mentions in a draft.
    pub const MENTION_COUNT: usize = 48;
    /// Candidates for filter.
    pub const CANDIDATE_COUNT: usize = 200;
    /// Paint frames.
    pub const PAINT_FRAMES: u32 = 24;
}

// ── Tests ───────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use crate::input::KeyEventKind;
    use crate::style::DesignSystem;
    use crate::widgets::remove_label;

    #[test]
    fn detect_file_mention_at() {
        let q = detect_file_mention_query("see @src/foo", 12).unwrap();
        assert_eq!(q.trigger, '@');
        assert_eq!(q.query, "src/foo");
        assert_eq!(q.family, MentionFamily::File);
    }

    #[test]
    fn detect_entity_hash() {
        let q = detect_entity_mention_query("use #agent-a", 12).unwrap();
        assert_eq!(q.trigger, '#');
        assert_eq!(q.query, "agent-a");
    }

    #[test]
    fn semantic_redacts_sensitive_path() {
        let m = FileMention::path("1", "key.pem", "/home/user/secret/key.pem")
            .mention
            .sensitive(true);
        let s = mention_semantic_description(&m);
        assert!(!s.contains("/home/user"));
        assert!(!s.contains("secret"));
        assert!(s.contains("key.pem") || s.contains("file"));
    }

    #[test]
    fn draft_atomic_cursor_skips_mention() {
        let mut d = MentionDraft::from_text("hi ");
        d.insert_mention(FileMention::path("f", "a.rs", "a.rs").mention);
        d.insert_text(" bye");
        // cursor at end of " bye"
        assert!(d.move_left()); // into text
        // walk left until on mention
        for _ in 0..10 {
            if matches!(d.cursor, MentionCursor::OnMention { .. }) {
                break;
            }
            assert!(d.move_left());
        }
        assert!(matches!(d.cursor, MentionCursor::OnMention { .. }));
        // one more left leaves mention
        assert!(d.move_left());
        assert!(!matches!(d.cursor, MentionCursor::OnMention { .. }));
    }

    #[test]
    fn delete_backward_removes_whole_mention() {
        let mut d = MentionDraft::new();
        d.insert_mention(EntityMention::agent("a1", "bot", "agent:bot").mention);
        d.cursor = MentionCursor::OnMention { part: 0 };
        assert!(d.delete_backward());
        assert_eq!(d.mention_count(), 0);
    }

    #[test]
    fn apply_insert_replaces_trigger_span() {
        let draft = "see @fo";
        let q = detect_file_mention_query(draft, draft.len()).unwrap();
        let m = MentionCandidate::new("id1", "foo.rs", MentionType::File, "src/foo.rs");
        let next = apply_mention_insert(draft, &q, &m.insert_markup());
        assert!(next.contains("@[file:"));
        assert!(next.starts_with("see "));
        assert!(!next.contains("@fo"));
    }

    #[test]
    fn filter_and_family() {
        let cands = vec![
            MentionCandidate::new("1", "main.rs", MentionType::File, "main.rs"),
            MentionCandidate::new("2", "bot", MentionType::Agent, "agent:bot"),
            MentionCandidate::new("3", "lib.rs", MentionType::File, "lib.rs"),
        ];
        let files = filter_mention_candidates(&cands, "rs", Some(MentionFamily::File));
        assert_eq!(files.len(), 2);
        let agents = filter_mention_candidates(&cands, "bot", Some(MentionFamily::Entity));
        assert_eq!(agents.len(), 1);
    }

    #[test]
    fn disambiguation_apply() {
        let mut f = FileMention::ambiguous(
            "x",
            "util.rs",
            vec![
                MentionDisambiguator::new("a/util.rs", "util.rs").detail("a/"),
                MentionDisambiguator::new("b/util.rs", "util.rs").detail("b/"),
            ],
        );
        assert_eq!(f.mention.validity, MentionValidity::Ambiguous);
        assert!(f.as_mut().apply_disambiguation(1));
        assert_eq!(f.mention.validity, MentionValidity::Valid);
        assert_eq!(f.mention.canonical, "b/util.rs");
    }

    #[test]
    fn markup_roundtrip() {
        let m = FileMention::path("id1", "lib.rs", "crates/x/lib.rs").mention;
        let s = m.to_markup();
        let p = parse_mention_markup(&s).unwrap();
        assert_eq!(p.id, "id1");
        assert_eq!(p.label, "lib.rs");
        assert_eq!(p.kind, MentionType::File);
    }

    #[test]
    fn parse_draft_mixed() {
        let d = parse_draft_with_mentions("see @[file:f1|a.rs] please");
        assert_eq!(d.parts.len(), 3);
        assert_eq!(d.mention_count(), 1);
    }

    #[test]
    fn file_mention_state_sync() {
        let mut st = FileMentionState::new();
        assert!(st.sync_from_draft("x @ab", 5));
        assert!(st.open);
        // trailing space ends query — close transition returns true
        assert!(st.sync_from_draft("x @ab ", 6));
        assert!(!st.open);
        // already closed
        assert!(!st.sync_from_draft("x @ab ", 6));
    }

    #[test]
    fn inline_paint_and_copy_outcome() {
        let system = DesignSystem::default();
        let file = FileMention::missing("m", "gone.rs", "gone.rs");
        let mut st = InlineMentionState::new();
        st.set_focused(true);
        let area = Rect::new(0, 0, 40, 2);
        let mut buf = Buffer::empty(area);
        InlineMention::file(&file, &system).paint(area, &mut buf, &mut st);
        let out = InlineMention::file(&file, &system).handle_key(
            &mut st,
            KeyEvent::new(KeyCode::Char('c'), KeyModifiers::CONTROL),
        );
        assert!(matches!(out, InlineMentionOutcome::CopyRequested { .. }));
        let desc = mention_semantic_description(file.as_ref());
        assert!(desc.contains("missing"));
    }

    #[test]
    fn repeated_direct_actions_and_disambiguation_lifecycle_are_ignored() {
        let system = DesignSystem::default();
        let file = FileMention::missing("m", "gone.rs", "gone.rs");
        let mention = InlineMention::file(&file, &system);
        let mut state = InlineMentionState::new();
        state.set_focused(true);

        for (code, modifiers) in [
            (KeyCode::Char('c'), KeyModifiers::CONTROL),
            (KeyCode::Char('p'), KeyModifiers::CONTROL),
        ] {
            let mut repeat = KeyEvent::new(code, modifiers);
            repeat.kind = KeyEventKind::Repeat;
            let before = state.clone();
            assert_eq!(
                mention.handle_key(&mut state, repeat),
                InlineMentionOutcome::Ignored
            );
            assert_eq!(state, before, "{code:?} repeat mutated mention state");
        }
        assert!(matches!(
            mention.handle_key(
                &mut state,
                KeyEvent::new(KeyCode::Char('c'), KeyModifiers::CONTROL)
            ),
            InlineMentionOutcome::CopyRequested { .. }
        ));

        let ambiguous = FileMention::ambiguous(
            "a",
            "same.rs",
            vec![
                MentionDisambiguator::new("one/same.rs", "same.rs"),
                MentionDisambiguator::new("two/same.rs", "same.rs"),
            ],
        );
        let ambiguous_mention = InlineMention::file(&ambiguous, &system);
        let mut repeat_disambiguate = KeyEvent::new(KeyCode::Char('d'), KeyModifiers::CONTROL);
        repeat_disambiguate.kind = KeyEventKind::Repeat;
        assert_eq!(
            ambiguous_mention.handle_key(&mut state, repeat_disambiguate),
            InlineMentionOutcome::Ignored
        );
        assert!(!state.disambiguation_open);
        assert!(matches!(
            ambiguous_mention.handle_key(
                &mut state,
                KeyEvent::new(KeyCode::Char('d'), KeyModifiers::CONTROL)
            ),
            InlineMentionOutcome::DisambiguateRequested { .. }
        ));

        let before = state.clone();
        let mut repeat_escape = KeyEvent::new(KeyCode::Esc, KeyModifiers::NONE);
        repeat_escape.kind = KeyEventKind::Repeat;
        assert_eq!(
            ambiguous_mention.handle_key(&mut state, repeat_escape),
            InlineMentionOutcome::Ignored
        );
        assert_eq!(state, before);

        let mut repeat_enter = KeyEvent::new(KeyCode::Enter, KeyModifiers::NONE);
        repeat_enter.kind = KeyEventKind::Repeat;
        let before = state.clone();
        assert_eq!(
            ambiguous_mention.handle_key(&mut state, repeat_enter),
            InlineMentionOutcome::Ignored
        );
        assert_eq!(state, before);
        assert!(matches!(
            ambiguous_mention.handle_key(
                &mut state,
                KeyEvent::new(KeyCode::Enter, KeyModifiers::NONE)
            ),
            InlineMentionOutcome::DisambiguationSelected { index: 0, .. }
        ));
    }

    #[test]
    fn never_looks_up_resources() {
        let src = include_str!("mention.rs");
        let body = src.split("#[cfg(test)]").next().unwrap_or(src);
        for forbidden in [
            "reqwest::",
            "std::fs::",
            "tokio::fs",
            "walkdir::",
            "ignore::",
            "lsp_types",
        ] {
            assert!(!body.contains(forbidden), "must not contain {forbidden}");
        }
    }

    #[test]
    fn completion_candidate_projection() {
        let c = MentionCandidate::new("1", "foo", MentionType::Tool, "tool:foo")
            .detail("run tool")
            .group("tools");
        let row = mention_to_completion_candidate(&c);
        assert_eq!(row.label, "foo");
        assert_eq!(row.kind, Some("tool"));
    }

    #[test]
    fn moderate_draft_bench() {
        let system = DesignSystem::default();
        let mut d = MentionDraft::from_text("start ");
        for i in 0..bench::MENTION_COUNT {
            d.insert_mention(
                FileMention::path(format!("f{i}"), format!("f{i}.rs"), format!("f{i}.rs")).mention,
            );
            d.insert_text(" ");
        }
        assert_eq!(d.mention_count(), bench::MENTION_COUNT);
        // navigate across tokens
        for _ in 0..bench::MENTION_COUNT * 2 {
            let _ = d.move_left();
        }
        let area = Rect::new(0, 0, 60, 1);
        let mut buf = Buffer::empty(area);
        for p in &d.parts {
            if let MentionSegment::Mention(m) = p {
                let mut st = InlineMentionState::new();
                for _ in 0..2 {
                    InlineMention::new(m, &system).paint(area, &mut buf, &mut st);
                }
            }
        }
        // filter candidates
        let cands: Vec<_> = (0..bench::CANDIDATE_COUNT)
            .map(|i| {
                MentionCandidate::new(
                    format!("c{i}"),
                    format!("item{i}"),
                    if i % 2 == 0 {
                        MentionType::File
                    } else {
                        MentionType::Agent
                    },
                    format!("can{i}"),
                )
            })
            .collect();
        let f = filter_mention_candidates(&cands, "item1", None);
        assert!(!f.is_empty());
    }

    #[test]
    fn remove_label_semantic() {
        let m = EntityMention::tool("t", "bash", "tool:bash");
        let label = m.as_ref().display_label(true);
        assert!(label.contains('T') || label.contains("bash"));
        let _ = remove_label(&m.as_ref().label);
    }
}
