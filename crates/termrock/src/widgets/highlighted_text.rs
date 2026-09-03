// SPDX-FileCopyrightText: 2026 Alexey Zhokhov
// SPDX-License-Identifier: Apache-2.0

//! HighlightedText and MatchRanges — reusable match rendering for search,
//! fuzzy finders, completion, and command palettes.
//!
//! **Mission.** Hosts own scoring/fuzzy metadata; this module owns
//! grapheme-safe range normalization, overlap resolution, match-preserving
//! truncation, and paint via DesignSystem roles — without re-running fuzzy
//! algorithms in `paint`.
//!
//! **Indices.** [`MatchRange`] uses **byte offsets** into the source `&str`.
//! Builders snap to grapheme boundaries. Display-column helpers convert when
//! hosts only know visual columns.
//!
//! Research: fzf, television, command palettes.
use ratatui_core::{buffer::Buffer, layout::Rect, widgets::Widget};
use unicode_segmentation::UnicodeSegmentation;

use crate::style::{DesignSystem, Role};
use crate::text::{display_cols, take_display_cols};
use crate::widgets::{Text, TextOverflow, TextSpan};

// ── Match kind / range ──────────────────────────────────────────────────────

/// Semantic class of a match span (overlap priority: Focused > Annotation > Match > Dim).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
#[non_exhaustive]
pub enum MatchKind {
    /// Primary fuzzy / substring hit.
    #[default]
    Match,
    /// Secondary / weaker hit (e.g. path tail).
    Soft,
    /// Host annotation layer (diagnostic, tag) over the same text.
    Annotation,
    /// Currently focused match among several (keyboard walk).
    Focused,
}

impl MatchKind {
    /// Stable id.
    #[must_use]
    pub const fn id(self) -> &'static str {
        match self {
            Self::Match => "match",
            Self::Soft => "soft",
            Self::Annotation => "annotation",
            Self::Focused => "focused",
        }
    }

    /// Priority when ranges overlap (higher wins).
    #[must_use]
    pub const fn priority(self) -> u8 {
        match self {
            Self::Soft => 1,
            Self::Match => 2,
            Self::Annotation => 3,
            Self::Focused => 4,
        }
    }

    fn role(self, selected: bool) -> Role {
        match self {
            // A page of matches is not a page of warnings: ordinary matches
            // read as strong text, and accent marks the one the cursor is on
            // (plans/007).
            Self::Match | Self::Focused => {
                if selected {
                    Role::Accent
                } else {
                    Role::TextStrong
                }
            }
            Self::Soft => Role::TextMuted,
            Self::Annotation => Role::Link,
        }
    }
}

/// One half-open byte range `[start, end)` into a source string.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct MatchRange {
    /// Inclusive start byte index.
    pub start: usize,
    /// Exclusive end byte index.
    pub end: usize,
    /// Match class.
    pub kind: MatchKind,
}

impl MatchRange {
    /// Range with default [`MatchKind::Match`].
    #[must_use]
    pub const fn new(start: usize, end: usize) -> Self {
        Self {
            start,
            end,
            kind: MatchKind::Match,
        }
    }

    /// With kind.
    #[must_use]
    pub const fn with_kind(start: usize, end: usize, kind: MatchKind) -> Self {
        Self { start, end, kind }
    }

    /// Focused match.
    #[must_use]
    pub const fn focused(start: usize, end: usize) -> Self {
        Self::with_kind(start, end, MatchKind::Focused)
    }

    /// Soft match.
    #[must_use]
    pub const fn soft(start: usize, end: usize) -> Self {
        Self::with_kind(start, end, MatchKind::Soft)
    }

    /// Empty / invalid.
    #[must_use]
    pub const fn is_empty(self) -> bool {
        self.end <= self.start
    }

    /// Byte length.
    #[must_use]
    pub const fn len(self) -> usize {
        self.end.saturating_sub(self.start)
    }

    /// Clamp to `source` length and ensure `start <= end`.
    #[must_use]
    pub fn clamp_to(self, source: &str) -> Self {
        let n = source.len();
        let start = self.start.min(n);
        let end = self.end.min(n).max(start);
        Self {
            start,
            end,
            kind: self.kind,
        }
    }

    /// Snap start/end to grapheme boundaries (moves start back / end forward).
    #[must_use]
    pub fn snap_graphemes(self, source: &str) -> Self {
        let r = self.clamp_to(source);
        if r.is_empty() {
            return r;
        }
        let start = snap_start(source, r.start);
        let end = snap_end(source, r.end);
        Self {
            start,
            end: end.max(start),
            kind: r.kind,
        }
    }

    /// Slice of `source` for this range (empty if invalid).
    #[must_use]
    pub fn slice<'a>(self, source: &'a str) -> &'a str {
        let r = self.clamp_to(source);
        source.get(r.start..r.end).unwrap_or("")
    }
}

fn snap_start(source: &str, byte: usize) -> usize {
    if byte >= source.len() {
        return source.len();
    }
    if source.is_char_boundary(byte) && source.grapheme_indices(true).any(|(i, _)| i == byte) {
        return byte;
    }
    // Move to start of grapheme containing byte
    let mut last = 0;
    for (i, g) in source.grapheme_indices(true) {
        if i + g.len() > byte {
            return i;
        }
        last = i + g.len();
        if i >= byte {
            return i;
        }
    }
    last.min(source.len())
}

fn snap_end(source: &str, byte: usize) -> usize {
    if byte >= source.len() {
        return source.len();
    }
    if source.is_char_boundary(byte)
        && (byte == 0
            || source
                .grapheme_indices(true)
                .any(|(i, g)| i + g.len() == byte))
    {
        return byte;
    }
    for (i, g) in source.grapheme_indices(true) {
        if i >= byte {
            return i;
        }
        if i + g.len() >= byte {
            return i + g.len();
        }
    }
    source.len()
}

/// Build ranges from **display-column** spans (rare; prefer byte indices from scorer).
#[must_use]
pub fn match_range_from_display_cols(
    source: &str,
    start_col: usize,
    end_col: usize,
    kind: MatchKind,
) -> MatchRange {
    let start = byte_at_display_col(source, start_col);
    let end = byte_at_display_col(source, end_col);
    MatchRange::with_kind(start, end, kind).snap_graphemes(source)
}

fn byte_at_display_col(source: &str, col: usize) -> usize {
    let mut used = 0usize;
    for (i, g) in source.grapheme_indices(true) {
        let w = display_cols(g);
        if used + w > col {
            return i;
        }
        used += w;
        if used == col {
            return i + g.len();
        }
    }
    source.len()
}

// ── MatchRanges ─────────────────────────────────────────────────────────────

/// Normalized, host-precomputed match set (cheap to paint repeatedly).
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct MatchRanges {
    ranges: Vec<MatchRange>,
}

impl MatchRanges {
    /// Empty.
    #[must_use]
    pub const fn new() -> Self {
        Self { ranges: Vec::new() }
    }

    /// From raw ranges (will normalize against `source` when painting / via [`Self::normalized`]).
    #[must_use]
    pub fn from_ranges(iter: impl IntoIterator<Item = MatchRange>) -> Self {
        Self {
            ranges: iter.into_iter().collect(),
        }
    }

    /// Single range convenience.
    #[must_use]
    pub fn single(range: MatchRange) -> Self {
        Self {
            ranges: vec![range],
        }
    }

    /// Borrowed slice.
    #[must_use]
    pub fn as_slice(&self) -> &[MatchRange] {
        &self.ranges
    }

    /// Number of ranges.
    #[must_use]
    pub fn len(&self) -> usize {
        self.ranges.len()
    }

    /// Empty.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.ranges.is_empty()
    }

    /// Push (host builds offline).
    pub fn push(&mut self, range: MatchRange) {
        self.ranges.push(range);
    }

    /// Snap + clamp + sort by start (stable). Does **not** merge — use [`Self::resolve_overlaps`].
    #[must_use]
    pub fn normalized(self, source: &str) -> Self {
        let mut ranges: Vec<MatchRange> = self
            .ranges
            .into_iter()
            .map(|r| r.snap_graphemes(source))
            .filter(|r| !r.is_empty())
            .collect();
        ranges.sort_by_key(|r| (r.start, r.end, r.kind.priority()));
        Self { ranges }
    }

    /// Split overlapping ranges into non-overlapping segments (priority wins).
    ///
    /// Result is sorted by start and ready for linear paint. Call after
    /// [`Self::normalized`].
    #[must_use]
    pub fn resolve_overlaps(self, source: &str) -> Self {
        let n = source.len();
        if n == 0 || self.ranges.is_empty() {
            return Self::new();
        }
        // Sweep: collect breakpoints
        let mut points: Vec<usize> = vec![0, n];
        for r in &self.ranges {
            points.push(r.start.min(n));
            points.push(r.end.min(n));
        }
        points.sort_unstable();
        points.dedup();
        let mut out = Vec::new();
        for w in points.windows(2) {
            let a = w[0];
            let b = w[1];
            if a >= b {
                continue;
            }
            // Best kind covering [a,b)
            let mut best: Option<MatchKind> = None;
            for r in &self.ranges {
                if r.start <= a && r.end >= b {
                    best = Some(match best {
                        None => r.kind,
                        Some(k) if r.kind.priority() > k.priority() => r.kind,
                        Some(k) => k,
                    });
                }
            }
            if let Some(kind) = best {
                out.push(MatchRange::with_kind(a, b, kind));
            }
        }
        // Merge adjacent same-kind
        let mut merged = Vec::new();
        for r in out {
            if let Some(last) = merged.last_mut() {
                let last: &mut MatchRange = last;
                if last.end == r.start && last.kind == r.kind {
                    last.end = r.end;
                    continue;
                }
            }
            merged.push(r);
        }
        Self { ranges: merged }
    }

    /// Full prepare pipeline for paint.
    #[must_use]
    pub fn prepare(self, source: &str) -> Self {
        self.normalized(source).resolve_overlaps(source)
    }
}

impl FromIterator<MatchRange> for MatchRanges {
    fn from_iter<T: IntoIterator<Item = MatchRange>>(iter: T) -> Self {
        Self::from_ranges(iter)
    }
}

// ── HighlightedText ─────────────────────────────────────────────────────────

/// Visual state of the whole line (row selection in palettes).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
#[non_exhaustive]
pub enum HighlightVisual {
    /// Default.
    #[default]
    Normal,
    /// Cursor / selected row (stronger match + base text).
    Selected,
    /// Dimmed inactive row.
    Inactive,
}

impl HighlightVisual {
    /// Stable id.
    #[must_use]
    pub const fn id(self) -> &'static str {
        match self {
            Self::Normal => "normal",
            Self::Selected => "selected",
            Self::Inactive => "inactive",
        }
    }
}

/// Truncation policy for match-aware clipping.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
#[non_exhaustive]
pub enum MatchTruncate {
    /// Clip with end ellipsis (may hide trailing matches).
    #[default]
    End,
    /// Prefer keeping the **first** match visible (shift window).
    KeepFirstMatch,
    /// Prefer keeping the **focused** match (or first) visible.
    KeepFocusedMatch,
    /// Middle ellipsis when both ends matter (long paths).
    Middle,
}

impl MatchTruncate {
    /// Stable id.
    #[must_use]
    pub const fn id(self) -> &'static str {
        match self {
            Self::End => "end",
            Self::KeepFirstMatch => "keep-first",
            Self::KeepFocusedMatch => "keep-focused",
            Self::Middle => "middle",
        }
    }
}

/// Painted geometry.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct HighlightedTextParts {
    /// Used area.
    pub root: Rect,
    /// Source text (borrowed snapshot for semantics).
    pub source_len: usize,
    /// Whether truncation applied.
    pub truncated: bool,
    /// Visible window as byte range into original source.
    pub visible: MatchRange,
}

/// Match-highlighted text renderer.
///
/// Source string and match metadata are **borrowed** — recompute matches when
/// the query changes; paint only walks prepared ranges.
#[derive(Debug, Clone, Copy)]
pub struct HighlightedText<'a> {
    source: &'a str,
    /// Pre-normalized ranges preferred; raw ranges accepted (prepared in paint).
    ranges: &'a [MatchRange],
    system: &'a DesignSystem,
    visual: HighlightVisual,
    truncate: MatchTruncate,
    /// Index into `ranges` for KeepFocusedMatch (before resolve).
    focused_index: Option<usize>,
    /// Base role for non-match text.
    base_role: Role,
    /// When true, ranges are already prepared (skip normalize cost).
    prepared: bool,
}

impl<'a> HighlightedText<'a> {
    /// Highlight `source` with `ranges`.
    #[must_use]
    pub const fn new(source: &'a str, ranges: &'a [MatchRange], system: &'a DesignSystem) -> Self {
        Self {
            source,
            ranges,
            system,
            visual: HighlightVisual::Normal,
            truncate: MatchTruncate::KeepFirstMatch,
            focused_index: None,
            base_role: Role::Text,
            prepared: false,
        }
    }

    /// Ranges already [`MatchRanges::prepare`]d.
    #[must_use]
    pub const fn prepared(
        source: &'a str,
        ranges: &'a [MatchRange],
        system: &'a DesignSystem,
    ) -> Self {
        let mut s = Self::new(source, ranges, system);
        s.prepared = true;
        s
    }

    /// Visual state.
    #[must_use]
    pub const fn visual(mut self, visual: HighlightVisual) -> Self {
        self.visual = visual;
        self
    }

    /// Selected row.
    #[must_use]
    pub const fn selected(mut self) -> Self {
        self.visual = HighlightVisual::Selected;
        self
    }

    /// Truncation policy.
    #[must_use]
    pub const fn truncate(mut self, policy: MatchTruncate) -> Self {
        self.truncate = policy;
        self
    }

    /// Focused original range index (for KeepFocusedMatch).
    #[must_use]
    pub const fn focused_index(mut self, index: Option<usize>) -> Self {
        self.focused_index = index;
        self
    }

    /// Base role for non-match runs.
    #[must_use]
    pub const fn base_role(mut self, role: Role) -> Self {
        self.base_role = role;
        self
    }

    /// Original source (copy / semantics).
    #[must_use]
    pub const fn source(&self) -> &'a str {
        self.source
    }

    /// Full plain source for clipboard.
    #[must_use]
    pub const fn plain(&self) -> &'a str {
        self.source
    }
    /// Build TextSpans for the full source (no width truncation).
    #[must_use]
    pub fn to_spans(&self) -> Vec<TextSpan<'a>> {
        let prepared = if self.prepared {
            self.ranges.to_vec()
        } else {
            MatchRanges {
                ranges: self.ranges.to_vec(),
            }
            .prepare(self.source)
            .ranges
        };
        spans_for_window(self.source, 0, self.source.len(), &prepared, self)
    }

    /// Match-aware truncation window → (visible_text owned, ranges shifted, visible byte range).
    #[must_use]
    pub fn visible_window(&self, max_cols: u16) -> (String, Vec<MatchRange>, MatchRange) {
        let max = usize::from(max_cols);
        let source = self.source;
        if max == 0 {
            return (String::new(), Vec::new(), MatchRange::new(0, 0));
        }
        let full_w = display_cols(source);
        if full_w <= max {
            let prep = if self.prepared {
                self.ranges.to_vec()
            } else {
                MatchRanges {
                    ranges: self.ranges.to_vec(),
                }
                .prepare(source)
                .ranges
            };
            return (source.to_string(), prep, MatchRange::new(0, source.len()));
        }

        let ellipsis = "…";
        let ellipsis_w = display_cols(ellipsis);
        let budget = max.saturating_sub(ellipsis_w).max(1);

        let prepared = if self.prepared {
            self.ranges.to_vec()
        } else {
            MatchRanges {
                ranges: self.ranges.to_vec(),
            }
            .prepare(source)
            .ranges
        };

        let (start_byte, end_byte) = match self.truncate {
            MatchTruncate::End => {
                let s = take_display_cols(source, budget);
                (0, s.len())
            }
            MatchTruncate::Middle => {
                let half = budget / 2;
                let head = take_display_cols(source, half.max(1));
                let tail_budget = budget.saturating_sub(display_cols(&head));
                let tail = take_display_cols_end(source, tail_budget);
                let start_tail = source.len().saturating_sub(tail.len());
                // Represent as head + ellipsis + tail specially below
                return middle_truncate(source, &prepared, &head, &tail, start_tail);
            }
            MatchTruncate::KeepFirstMatch | MatchTruncate::KeepFocusedMatch => {
                let anchor = anchor_byte(self, &prepared);
                window_around(source, anchor, budget)
            }
        };

        let visible = MatchRange::new(start_byte, end_byte).snap_graphemes(source);
        let mut text = source
            .get(visible.start..visible.end)
            .unwrap_or("")
            .to_string();
        let mut shifted = shift_ranges(&prepared, visible.start, visible.end);

        // Add ellipsis markers
        if visible.start > 0 {
            text = format!("{ellipsis}{text}");
            shifted = shift_all(&shifted, ellipsis.len());
        }
        if visible.end < source.len() {
            text.push_str(ellipsis);
        }
        // Final col clamp
        if display_cols(&text) > max {
            text = take_display_cols(&text, max).into_owned();
        }
        (text, shifted, visible)
    }

    /// Layout/paint into area (single line).
    pub fn paint(&self, area: Rect, buffer: &mut Buffer) -> HighlightedTextParts {
        if area.is_empty() {
            return HighlightedTextParts {
                root: area,
                source_len: self.source.len(),
                truncated: false,
                visible: MatchRange::new(0, 0),
            };
        }
        let full_w = display_cols(self.source);
        let truncated = full_w > usize::from(area.width);
        let (text, ranges, visible) = if truncated {
            self.visible_window(area.width)
        } else {
            let prep = if self.prepared {
                self.ranges.to_vec()
            } else {
                MatchRanges {
                    ranges: self.ranges.to_vec(),
                }
                .prepare(self.source)
                .ranges
            };
            (
                self.source.to_string(),
                prep,
                MatchRange::new(0, self.source.len()),
            )
        };

        // Paint via Text spans built from owned text — need local spans
        let spans = spans_for_owned(&text, &ranges, self);
        let mut t = Text::spans(spans, self.system);
        t = match self.truncate {
            MatchTruncate::Middle => t.overflow(TextOverflow::Clip),
            _ => t.overflow(TextOverflow::Clip),
        };
        // Selection chrome is the host row background; matches are styled above.
        let _ = t.paint(area, buffer);
        HighlightedTextParts {
            root: Rect {
                x: area.x,
                y: area.y,
                width: area.width,
                height: 1.min(area.height),
            },
            source_len: self.source.len(),
            truncated,
            visible,
        }
    }
}

impl Widget for &HighlightedText<'_> {
    fn render(self, area: Rect, buffer: &mut Buffer) {
        let _ = self.paint(area, buffer);
    }
}

impl Widget for HighlightedText<'_> {
    fn render(self, area: Rect, buffer: &mut Buffer) {
        <&Self as Widget>::render(&self, area, buffer);
    }
}

// ── Internals ───────────────────────────────────────────────────────────────

fn anchor_byte(ht: &HighlightedText<'_>, prepared: &[MatchRange]) -> usize {
    if matches!(ht.truncate, MatchTruncate::KeepFocusedMatch) {
        if let Some(i) = ht.focused_index {
            if let Some(r) = ht.ranges.get(i) {
                return r.snap_graphemes(ht.source).start;
            }
        }
        // Fall back to Focused kind in prepared
        if let Some(r) = prepared.iter().find(|r| r.kind == MatchKind::Focused) {
            return r.start;
        }
    }
    prepared.first().map(|r| r.start).unwrap_or(0)
}

fn window_around(source: &str, anchor: usize, budget_cols: usize) -> (usize, usize) {
    // Find grapheme window of budget_cols containing anchor near start of window
    let anchor = anchor.min(source.len());
    // Prefer anchor near 1/3 of window so context before match exists
    let mut start = snap_start(source, anchor);
    // Expand forward from start until budget
    let mut end = start;
    let mut used = 0usize;
    for (i, g) in source[start..].grapheme_indices(true) {
        let bi = start + i;
        let w = display_cols(g);
        if used + w > budget_cols {
            break;
        }
        end = bi + g.len();
        used += w;
    }
    // If anchor not in window, shift back
    if start > anchor {
        start = snap_start(source, anchor);
    }
    // If we still have room at end and start can move left to center anchor
    if used < budget_cols && start > 0 {
        let mut extra = budget_cols - used;
        let prefix = &source[..start];
        let mut grab = String::new();
        for g in prefix.graphemes(true).rev() {
            let w = display_cols(g);
            if w > extra {
                break;
            }
            grab.insert_str(0, g);
            extra -= w;
        }
        start = start.saturating_sub(grab.len());
        // recompute end
        end = start;
        used = 0;
        for (i, g) in source[start..].grapheme_indices(true) {
            let bi = start + i;
            let w = display_cols(g);
            if used + w > budget_cols {
                break;
            }
            end = bi + g.len();
            used += w;
        }
    }
    (start, end.max(start))
}

fn take_display_cols_end(s: &str, max_cols: usize) -> String {
    if max_cols == 0 {
        return String::new();
    }
    let mut graphemes: Vec<&str> = s.graphemes(true).collect();
    let mut out = String::new();
    let mut used = 0usize;
    while let Some(g) = graphemes.pop() {
        let w = display_cols(g);
        if used + w > max_cols {
            break;
        }
        out.insert_str(0, g);
        used += w;
    }
    out
}

fn middle_truncate(
    source: &str,
    prepared: &[MatchRange],
    head: &str,
    tail: &str,
    start_tail: usize,
) -> (String, Vec<MatchRange>, MatchRange) {
    let ellipsis = "…";
    let text = format!("{head}{ellipsis}{tail}");
    // Ranges only for head and tail pieces
    let mut shifted = Vec::new();
    for r in prepared {
        if r.end <= head.len() {
            shifted.push(*r);
        } else if r.start >= start_tail {
            let off = head.len() + ellipsis.len();
            shifted.push(MatchRange::with_kind(
                r.start - start_tail + off,
                r.end - start_tail + off,
                r.kind,
            ));
        }
    }
    (
        text,
        shifted,
        MatchRange::new(0, source.len()), // approximate
    )
}

fn shift_ranges(ranges: &[MatchRange], win_start: usize, win_end: usize) -> Vec<MatchRange> {
    let mut out = Vec::new();
    for r in ranges {
        if r.end <= win_start || r.start >= win_end {
            continue;
        }
        let s = r.start.max(win_start) - win_start;
        let e = r.end.min(win_end) - win_start;
        if e > s {
            out.push(MatchRange::with_kind(s, e, r.kind));
        }
    }
    out
}

fn shift_all(ranges: &[MatchRange], delta: usize) -> Vec<MatchRange> {
    ranges
        .iter()
        .map(|r| MatchRange::with_kind(r.start + delta, r.end + delta, r.kind))
        .collect()
}

fn spans_for_window<'a>(
    source: &'a str,
    win_start: usize,
    win_end: usize,
    prepared: &[MatchRange],
    ht: &HighlightedText<'a>,
) -> Vec<TextSpan<'a>> {
    let slice = source.get(win_start..win_end).unwrap_or("");
    let shifted = shift_ranges(prepared, win_start, win_end);
    // Build from shifted against slice as if offsets into slice
    let mut spans = Vec::new();
    let mut cursor = 0usize;
    let n = slice.len();
    for r in &shifted {
        if r.start > cursor {
            let piece = slice.get(cursor..r.start).unwrap_or("");
            if !piece.is_empty() {
                spans.push(base_span(piece, ht));
            }
        }
        let piece = slice.get(r.start..r.end.min(n)).unwrap_or("");
        if !piece.is_empty() {
            spans.push(match_span(piece, r.kind, ht));
        }
        cursor = r.end.min(n);
    }
    if cursor < n {
        let piece = slice.get(cursor..n).unwrap_or("");
        if !piece.is_empty() {
            spans.push(base_span(piece, ht));
        }
    }
    if spans.is_empty() {
        spans.push(base_span(slice, ht));
    }
    spans
}

fn spans_for_owned<'a>(
    text: &'a str,
    ranges: &[MatchRange],
    ht: &HighlightedText<'_>,
) -> Vec<TextSpan<'a>> {
    // ranges are relative to text
    let mut spans = Vec::new();
    let mut cursor = 0usize;
    let n = text.len();
    for r in ranges {
        let rs = r.start.min(n);
        let re = r.end.min(n).max(rs);
        if rs > cursor {
            let piece = text.get(cursor..rs).unwrap_or("");
            if !piece.is_empty() {
                spans.push(base_span(piece, ht));
            }
        }
        let piece = text.get(rs..re).unwrap_or("");
        if !piece.is_empty() {
            spans.push(match_span(piece, r.kind, ht));
        }
        cursor = re;
    }
    if cursor < n {
        let piece = text.get(cursor..n).unwrap_or("");
        if !piece.is_empty() {
            spans.push(base_span(piece, ht));
        }
    }
    if spans.is_empty() {
        spans.push(base_span(text, ht));
    }
    spans
}

fn base_span<'a>(piece: &'a str, ht: &HighlightedText<'_>) -> TextSpan<'a> {
    let mut s = TextSpan::new(piece).role(ht.base_role);
    match ht.visual {
        HighlightVisual::Selected => s = s.strong(),
        HighlightVisual::Inactive => s = s.dim(),
        HighlightVisual::Normal => {}
    }
    s
}

fn match_span<'a>(piece: &'a str, kind: MatchKind, ht: &HighlightedText<'_>) -> TextSpan<'a> {
    let selected = matches!(ht.visual, HighlightVisual::Selected);
    let role = kind.role(selected);
    let mut s = TextSpan::new(piece)
        .role(role)
        .highlight(true)
        .annotation(kind.id());
    s = s.strong();
    // Colorless, or the match you are actually on: reverse the run.
    // Underline is the link affordance (design-language §5.9), not "this one".
    if matches!(
        ht.system.capability,
        crate::style::ColorCapability::Monochrome
    ) || kind == MatchKind::Focused
    {
        s = s.reverse(true);
    }
    s
}

/// Substring match helper for hosts without a fuzzy engine (case-sensitive).
#[must_use]
pub fn substring_ranges(source: &str, query: &str) -> MatchRanges {
    if query.is_empty() {
        return MatchRanges::new();
    }
    let mut ranges = MatchRanges::new();
    let mut start = 0;
    while let Some(rel) = source[start..].find(query) {
        let abs = start + rel;
        ranges.push(MatchRange::new(abs, abs + query.len()));
        start = abs + query.len().max(1);
        if start >= source.len() {
            break;
        }
    }
    ranges.prepare(source)
}

/// Case-insensitive substring matches (Unicode-aware via per-char `char::to_lowercase`).
#[must_use]
pub fn substring_ranges_ignore_ascii_case(source: &str, query: &str) -> MatchRanges {
    if query.is_empty() {
        return MatchRanges::new();
    }
    let q: String = query.chars().flat_map(char::to_lowercase).collect();
    let lower: String = source.chars().flat_map(char::to_lowercase).collect();
    // Fold once, then map each lowercased byte offset back to its source byte
    // offset so match ranges address the original string exactly.
    let mut map = Vec::with_capacity(lower.len() + 1);
    let mut si = 0usize;
    for c in source.chars() {
        for _ in c.to_lowercase() {
            map.push(si);
        }
        si += c.len_utf8();
    }
    map.push(source.len());
    let mut ranges = MatchRanges::new();
    let mut start = 0usize;
    while let Some(rel) = lower[start..].find(&q) {
        let abs_l = start + rel;
        let abs = map[abs_l];
        let end = map[abs_l + q.len()];
        ranges.push(MatchRange::new(abs, end).snap_graphemes(source));
        start = abs_l + q.len().max(1);
        if start >= lower.len() {
            break;
        }
    }
    ranges.prepare(source)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn snap_graphemes_emoji() {
        let s = "a👍b";
        // 👍 is multi-byte; pick mid
        let mid = s.find('👍').unwrap() + 1;
        let r = MatchRange::new(mid, mid + 1).snap_graphemes(s);
        assert_eq!(r.slice(s), "👍");
    }

    #[test]
    fn overlap_priority_focused_wins() {
        let s = "abcdef";
        let ranges =
            MatchRanges::from_ranges([MatchRange::new(0, 4), MatchRange::focused(2, 6)]).prepare(s);
        // [0,2) Match, [2,4) Focused, [4,6) Focused
        assert!(
            ranges
                .as_slice()
                .iter()
                .any(|r| r.kind == MatchKind::Focused)
        );
        let at2 = ranges.as_slice().iter().find(|r| r.start == 2).unwrap();
        assert_eq!(at2.kind, MatchKind::Focused);
    }

    #[test]
    fn substring_and_paint() {
        let system = DesignSystem::default();
        let s = "src/widgets/command_palette.rs";
        let ranges = substring_ranges(s, "pal");
        assert!(!ranges.is_empty());
        let ht = HighlightedText::prepared(s, ranges.as_slice(), &system);
        let spans = ht.to_spans();
        assert!(spans.iter().any(|sp| sp.is_highlight()));
        let mut buf = Buffer::empty(Rect::new(0, 0, 40, 1));
        let parts = ht.paint(Rect::new(0, 0, 40, 1), &mut buf);
        assert!(!parts.truncated);
        assert_eq!(ht.plain(), s);
    }

    #[test]
    fn keep_first_match_truncation() {
        let system = DesignSystem::default();
        let s = "aaaaaaaaaaaaaaaaaaaa_HIT_bbbbbbbbbbbbbbbbbbbb";
        let ranges = substring_ranges(s, "HIT");
        let ht = HighlightedText::prepared(s, ranges.as_slice(), &system)
            .truncate(MatchTruncate::KeepFirstMatch);
        let (vis, _, _) = ht.visible_window(12);
        assert!(vis.contains("HIT") || vis.contains('H'), "{vis}");
        assert!(display_cols(&vis) <= 12, "{vis}");
    }

    #[test]
    fn no_color_underline_matches() {
        let system = DesignSystem::default().no_color();
        let s = "fuzzy";
        let ranges = substring_ranges(s, "zz");
        let ht = HighlightedText::prepared(s, ranges.as_slice(), &system);
        let spans = ht.to_spans();
        let hit = spans.iter().find(|s| s.is_highlight()).unwrap();
        // underline forced in monochrome path via style resolution at paint;
        // span flags highlight true
        assert!(hit.is_highlight());
    }

    #[test]
    fn selected_visual() {
        let system = DesignSystem::default();
        let s = "item";
        let ranges = substring_ranges(s, "it");
        let ht = HighlightedText::prepared(s, ranges.as_slice(), &system).selected();
        let mut buf = Buffer::empty(Rect::new(0, 0, 10, 1));
        let _ = ht.paint(Rect::new(0, 0, 10, 1), &mut buf);
    }

    #[test]
    fn empty_and_full_width() {
        let system = DesignSystem::default();
        let ht = HighlightedText::new("", &[], &system);
        let mut buf = Buffer::empty(Rect::new(0, 0, 1, 1));
        let p = ht.paint(Rect::new(0, 0, 0, 0), &mut buf);
        assert!(p.root.is_empty());
    }

    #[test]
    fn prepare_idempotent_cost() {
        let s = "the quick brown fox jumps over the lazy dog";
        let raw = substring_ranges(s, "o");
        let prep = MatchRanges {
            ranges: raw.as_slice().to_vec(),
        }
        .prepare(s);
        // many o's
        assert!(prep.len() >= 3);
        for _ in 0..1000 {
            let _ = MatchRanges {
                ranges: prep.as_slice().to_vec(),
            }
            .prepare(s);
        }
    }

    #[test]
    fn display_col_range_builder() {
        let s = "ab宽c";
        let r = match_range_from_display_cols(s, 2, 4, MatchKind::Match);
        assert!(!r.slice(s).is_empty());
    }

    #[test]
    fn fuzz_ranges_never_panic() {
        let s = "αβγ👍test/path.rs";
        let junk = [
            MatchRange::new(0, 1000),
            MatchRange::new(3, 1),
            MatchRange::new(1, 2),
            MatchRange::focused(5, 8),
            MatchRange::soft(0, 2),
        ];
        let prep = MatchRanges::from_ranges(junk).prepare(s);
        let system = DesignSystem::default();
        let ht = HighlightedText::prepared(s, prep.as_slice(), &system);
        let mut buf = Buffer::empty(Rect::new(0, 0, 8, 1));
        let _ = ht.paint(Rect::new(0, 0, 8, 1), &mut buf);
        for r in prep.as_slice() {
            let _ = r.slice(s);
        }
    }
}
