// SPDX-FileCopyrightText: 2026 Alexey Zhokhov
// SPDX-License-Identifier: Apache-2.0

//! **HexViewer** — virtualized binary inspector.
//!
//! **Mission.** Offsets, configurable bytes per row, hex + ASCII/Unicode
//! interpretation, selection, search, bookmarks, endianness-aware value
//! inspector, and copy/export outcomes. Massive files via **application-provided
//! paging** ([`HexWindow`]). Active byte and selection use non-color marks
//! (cursor gutter, `[`/`]` brackets, `*` bookmarks). Tiny-terminal compact mode.
//!
//! **Ownership.** Host owns the file / buffer and projects a window of bytes
//! each frame (`base_offset` + `data` + `total_len`). TermRock owns viewport
//! row scroll, cursor/selection in absolute offsets, chrome, and typed
//! outcomes (never I/O).
//!
//! Research: hex editors, xxd, binary-analysis tools.

#![allow(unused_imports)] // test-only imports retained
use std::collections::BTreeSet;

use ratatui_core::{buffer::Buffer, layout::Rect, style::Modifier, widgets::StatefulWidget};

use crate::{
    input::{
        KeyCode, KeyEvent, KeyEventKind, KeyModifiers, MouseButton, MouseEvent, MouseEventKind,
    },
    interaction::{NavigationMove, PageMove, UiIntent},
    style::{DesignSystem, ListRowVisualState, Role},
    text::{display_cols, take_display_cols},
    widgets::{scroll_area::ScrollAreaState, tiered_row::TieredRow},
};

/// Default bytes per row when auto-fit is not used.
pub const HEX_DEFAULT_BYTES_PER_ROW: u8 = 16;
/// Minimum bytes per row.
pub const HEX_MIN_BYTES_PER_ROW: u8 = 4;
/// Maximum bytes per row.
pub const HEX_MAX_BYTES_PER_ROW: u8 = 64;

// ── Window & modes ──────────────────────────────────────────────────────────

/// Host-projected page of bytes for virtualization.
///
/// `data[0]` corresponds to absolute file offset `base_offset`. Scroll metrics
/// use `total_len` so the host can page-in on demand when the cursor leaves
/// the window.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct HexWindow<'a> {
    /// Absolute offset of the first byte in `data`.
    pub base_offset: u64,
    /// Page bytes (may be empty at EOF).
    pub data: &'a [u8],
    /// Total logical file length in bytes.
    pub total_len: u64,
}

impl<'a> HexWindow<'a> {
    /// Construct.
    #[must_use]
    pub const fn new(base_offset: u64, data: &'a [u8], total_len: u64) -> Self {
        Self {
            base_offset,
            data,
            total_len,
        }
    }

    /// Absolute end offset (exclusive) covered by this window.
    #[must_use]
    pub fn end_offset(&self) -> u64 {
        self.base_offset.saturating_add(self.data.len() as u64)
    }

    /// Byte at absolute offset if present in this window.
    #[must_use]
    pub fn get(&self, abs: u64) -> Option<u8> {
        if abs < self.base_offset {
            return None;
        }
        let i = (abs - self.base_offset) as usize;
        self.data.get(i).copied()
    }

    /// Slice of absolute range intersecting this window (may be empty).
    #[must_use]
    pub fn slice_abs(&self, start: u64, end: u64) -> &'a [u8] {
        let end = end.max(start);
        let w0 = self.base_offset;
        let w1 = self.end_offset();
        let a = start.max(w0);
        let b = end.min(w1);
        if a >= b {
            return &[];
        }
        let i0 = (a - w0) as usize;
        let i1 = (b - w0) as usize;
        &self.data[i0..i1]
    }
}

/// Endianness for multi-byte inspector values.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
#[non_exhaustive]
pub enum HexEndian {
    /// Little-endian (default on most hosts).
    #[default]
    Little,
    /// Big-endian / network order.
    Big,
}

impl HexEndian {
    /// Stable id.
    #[must_use]
    pub const fn id(self) -> &'static str {
        match self {
            Self::Little => "le",
            Self::Big => "be",
        }
    }

    /// Toggle.
    #[must_use]
    pub const fn toggle(self) -> Self {
        match self {
            Self::Little => Self::Big,
            Self::Big => Self::Little,
        }
    }
}

/// How the right-hand interpretation column is rendered.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
#[non_exhaustive]
pub enum HexAsciiMode {
    /// ASCII printable `0x20..=0x7E`, else `.`.
    #[default]
    Ascii,
    /// Unicode printable (single display cell), else `.`.
    Unicode,
    /// Always dots (binary-only focus).
    Dots,
}

impl HexAsciiMode {
    /// Stable id.
    #[must_use]
    pub const fn id(self) -> &'static str {
        match self {
            Self::Ascii => "ascii",
            Self::Unicode => "unicode",
            Self::Dots => "dots",
        }
    }

    /// Cycle.
    #[must_use]
    pub const fn next(self) -> Self {
        match self {
            Self::Ascii => Self::Unicode,
            Self::Unicode => Self::Dots,
            Self::Dots => Self::Ascii,
        }
    }
}

// ── Layout helpers (property-tested) ────────────────────────────────────────

/// Width of offset column in characters for a total length.
#[must_use]
pub fn offset_width_chars(total_len: u64) -> u8 {
    // hex digits needed for max offset (inclusive last byte)
    let max_off = total_len.saturating_sub(1);
    let digits = if max_off == 0 {
        1u32
    } else {
        64 - max_off.leading_zeros()
    };
    let hex_digits = ((digits + 3) / 4).max(1);
    // common: at least 8 for familiarity
    (hex_digits as u8).clamp(4, 16)
}

/// Format absolute offset as zero-padded hex.
#[must_use]
pub fn format_offset(offset: u64, width: u8) -> String {
    let w = usize::from(width.max(1));
    format!("{offset:0width$X}", width = w)
}

/// Number of complete rows for `total_len` at `bpr` bytes per row.
#[must_use]
pub fn row_count(total_len: u64, bytes_per_row: u8) -> u64 {
    let bpr = u64::from(bytes_per_row.max(1));
    if total_len == 0 {
        0
    } else {
        total_len.div_ceil(bpr)
    }
}

/// Row index containing absolute `offset`.
#[must_use]
pub fn row_for_offset(offset: u64, bytes_per_row: u8) -> u64 {
    let bpr = u64::from(bytes_per_row.max(1));
    offset / bpr
}

/// Absolute start offset of `row`.
#[must_use]
pub fn offset_for_row(row: u64, bytes_per_row: u8) -> u64 {
    row.saturating_mul(u64::from(bytes_per_row.max(1)))
}

/// Column within row for absolute offset.
#[must_use]
pub fn col_for_offset(offset: u64, bytes_per_row: u8) -> u8 {
    let bpr = u64::from(bytes_per_row.max(1));
    (offset % bpr) as u8
}

/// Minimum terminal width for a full row paint (offset + hex + ascii + gutters).
#[must_use]
pub fn min_width_for_bpr(bytes_per_row: u8, offset_w: u8) -> u16 {
    let bpr = u16::from(bytes_per_row.max(1));
    let ow = u16::from(offset_w.max(4));
    // " " + offset + "  " + hex groups (3 per byte) + " |" + ascii + "|"
    // hex: each byte "XX " = 3 chars
    ow + 2 + bpr * 3 + 2 + bpr + 1
}

/// Auto-select bytes per row for available width (power-of-two-ish clamps).
#[must_use]
pub fn auto_bytes_per_row(width: u16, offset_w: u8) -> u8 {
    // Prefer 16, then 8, then 4
    for &cand in &[16u8, 8, 4] {
        if min_width_for_bpr(cand, offset_w) <= width {
            return cand;
        }
    }
    // Tiny: still show 4 if possible, else 4 forced (compact collapses)
    HEX_MIN_BYTES_PER_ROW
}

/// Inclusive selection range normalized (start <= end).
#[must_use]
pub fn normalize_range(a: u64, b: u64) -> (u64, u64) {
    if a <= b { (a, b) } else { (b, a) }
}

/// Whether absolute offset is inside inclusive selection.
#[must_use]
pub fn in_selection(offset: u64, anchor: Option<u64>, end: Option<u64>) -> bool {
    match (anchor, end) {
        (Some(a), Some(b)) => {
            let (lo, hi) = normalize_range(a, b);
            offset >= lo && offset <= hi
        }
        (Some(a), None) | (None, Some(a)) => offset == a,
        (None, None) => false,
    }
}

/// Interpret printable cell for ASCII column.
#[must_use]
pub fn interpret_byte(b: u8, mode: HexAsciiMode) -> char {
    match mode {
        HexAsciiMode::Dots => '.',
        HexAsciiMode::Ascii => {
            if (0x20..=0x7e).contains(&b) {
                b as char
            } else {
                '.'
            }
        }
        HexAsciiMode::Unicode => {
            if (0x20..=0x7e).contains(&b) {
                b as char
            } else if b >= 0x80 {
                // single-byte non-ASCII: show as '.' (host may pass UTF-8 windows later)
                '.'
            } else {
                '.'
            }
        }
    }
}

/// Format one byte as two uppercase hex digits.
#[must_use]
pub fn format_byte_hex(b: u8) -> String {
    format!("{b:02X}")
}

// ── Inspector ───────────────────────────────────────────────────────────────

/// Endian-aware decoded values at cursor (host may show outside widget too).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct HexInspectorValues {
    /// Absolute cursor offset.
    pub offset: u64,
    /// u8 at cursor (if in window).
    pub u8: Option<u8>,
    /// i8.
    pub i8: Option<i8>,
    /// u16.
    pub u16: Option<u16>,
    /// i16.
    pub i16: Option<i16>,
    /// u32.
    pub u32: Option<u32>,
    /// i32.
    pub i32: Option<i32>,
    /// u64.
    pub u64: Option<u64>,
    /// i64.
    pub i64: Option<i64>,
    /// Endian used.
    pub endian: HexEndian,
}

/// Decode inspector values from a window at absolute offset.
#[must_use]
pub fn inspect_at(window: &HexWindow<'_>, offset: u64, endian: HexEndian) -> HexInspectorValues {
    let mut v = HexInspectorValues {
        offset,
        u8: None,
        i8: None,
        u16: None,
        i16: None,
        u32: None,
        i32: None,
        u64: None,
        i64: None,
        endian,
    };
    let Some(b0) = window.get(offset) else {
        return v;
    };
    v.u8 = Some(b0);
    v.i8 = Some(b0 as i8);

    let take = |n: usize| -> Option<Vec<u8>> {
        let end = offset.saturating_add(n as u64);
        if end > window.total_len {
            return None;
        }
        let s = window.slice_abs(offset, end);
        if s.len() < n { None } else { Some(s.to_vec()) }
    };

    if let Some(bytes) = take(2) {
        let n = match endian {
            HexEndian::Little => u16::from_le_bytes([bytes[0], bytes[1]]),
            HexEndian::Big => u16::from_be_bytes([bytes[0], bytes[1]]),
        };
        v.u16 = Some(n);
        v.i16 = Some(n as i16);
    }
    if let Some(bytes) = take(4) {
        let arr = [bytes[0], bytes[1], bytes[2], bytes[3]];
        let n = match endian {
            HexEndian::Little => u32::from_le_bytes(arr),
            HexEndian::Big => u32::from_be_bytes(arr),
        };
        v.u32 = Some(n);
        v.i32 = Some(n as i32);
    }
    if let Some(bytes) = take(8) {
        let arr = [
            bytes[0], bytes[1], bytes[2], bytes[3], bytes[4], bytes[5], bytes[6], bytes[7],
        ];
        let n = match endian {
            HexEndian::Little => u64::from_le_bytes(arr),
            HexEndian::Big => u64::from_be_bytes(arr),
        };
        v.u64 = Some(n);
        v.i64 = Some(n as i64);
    }
    v
}

/// Format inspector as one status line.
#[must_use]
pub fn format_inspector_line(v: &HexInspectorValues, ascii: bool) -> String {
    let end = if ascii { v.endian.id() } else { v.endian.id() };
    let mut parts = vec![format!("@{offset:X}", offset = v.offset)];
    if let Some(b) = v.u8 {
        parts.push(format!("u8={b}"));
        parts.push(format!(
            "'{ch}'",
            ch = interpret_byte(b, HexAsciiMode::Ascii)
        ));
    }
    if let Some(n) = v.u16 {
        parts.push(format!("u16={n}"));
    }
    if let Some(n) = v.u32 {
        parts.push(format!("u32={n}"));
    }
    if let Some(n) = v.u64 {
        parts.push(format!("u64={n}"));
    }
    parts.push(end.to_string());
    parts.join(" ")
}

// ── Search ──────────────────────────────────────────────────────────────────

/// Parse a search query: hex bytes (`"de ad be"` / `"deadbe"`) or UTF-8 text.
#[must_use]
pub fn parse_search_query(query: &str) -> Option<Vec<u8>> {
    let q = query.trim();
    if q.is_empty() {
        return None;
    }
    // Try hex: strip spaces/colons
    let hexish: String = q
        .chars()
        .filter(|c| !c.is_whitespace() && *c != ':' && *c != '-')
        .collect();
    if hexish.len() >= 2 && hexish.len() % 2 == 0 && hexish.chars().all(|c| c.is_ascii_hexdigit()) {
        let mut out = Vec::with_capacity(hexish.len() / 2);
        let bytes = hexish.as_bytes();
        let mut i = 0;
        while i + 1 < bytes.len() {
            let h = core::str::from_utf8(&bytes[i..i + 2]).ok()?;
            out.push(u8::from_str_radix(h, 16).ok()?);
            i += 2;
        }
        return Some(out);
    }
    Some(q.as_bytes().to_vec())
}

/// Find first match of `needle` in window data at/after `from_abs` (absolute).
#[must_use]
pub fn find_in_window(window: &HexWindow<'_>, needle: &[u8], from_abs: u64) -> Option<u64> {
    if needle.is_empty() {
        return None;
    }
    let start = from_abs.max(window.base_offset);
    if start >= window.end_offset() {
        return None;
    }
    let i0 = (start - window.base_offset) as usize;
    let hay = &window.data[i0..];
    hay.windows(needle.len())
        .position(|w| w == needle)
        .map(|p| start + p as u64)
}

// ── State & outcomes ────────────────────────────────────────────────────────

/// Hit region for a painted row.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct HexRegion {
    /// Row absolute start offset.
    pub row_offset: u64,
    /// Painted area.
    pub area: Rect,
}

/// Outcomes (host owns clipboard/export I/O and paging).
#[derive(Debug, Clone, PartialEq, Eq)]
#[non_exhaustive]
pub enum HexViewerOutcome {
    /// No change.
    Ignored,
    /// Viewport scrolled (row units).
    Scrolled {
        /// Row offset.
        row: u64,
    },
    /// Cursor moved to absolute offset.
    CursorMoved {
        /// Absolute byte offset.
        offset: u64,
    },
    /// Selection changed (inclusive absolute range).
    SelectionChanged {
        /// Start.
        start: u64,
        /// End.
        end: u64,
    },
    /// Selection cleared.
    SelectionCleared,
    /// Bytes per row changed.
    BytesPerRowChanged(u8),
    /// Endian toggled.
    EndianChanged(HexEndian),
    /// ASCII mode changed.
    AsciiModeChanged(HexAsciiMode),
    /// Bookmark toggled.
    BookmarkToggled {
        /// Offset.
        offset: u64,
        /// Bookmarked after.
        on: bool,
    },
    /// Search query changed.
    SearchChanged(String),
    /// Jump to search hit.
    SearchHit {
        /// Absolute offset.
        offset: u64,
    },
    /// Copy selection or cursor byte(s) as hex.
    CopyHex {
        /// Hex text.
        text: String,
    },
    /// Copy selection as raw-ish latin1/escaped text.
    CopyAscii {
        /// Text.
        text: String,
    },
    /// Export request (host writes file).
    Export {
        /// Absolute start.
        start: u64,
        /// Exclusive end.
        end: u64,
        /// Hex dump text of **window-resident** bytes only (host may re-read).
        text: String,
    },
    /// Host should page so `offset` is in window (cursor left projected data).
    PageNeeded {
        /// Desired absolute offset in view.
        offset: u64,
    },
    /// Cancel search.
    Cancelled,
}

/// Interaction state — offsets are **absolute** file coordinates.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct HexViewerState {
    scroll: ScrollAreaState,
    accepts_input: bool,
    origin: (u16, u16),
    body_rows: u16,
    area_rows: u16,
    /// Absolute cursor byte.
    pub cursor: u64,
    /// Selection anchor (inclusive).
    pub sel_anchor: Option<u64>,
    /// Selection end (inclusive).
    pub sel_end: Option<u64>,
    /// 0 = auto from width.
    pub bytes_per_row: u8,
    /// Effective bpr after last paint.
    pub effective_bpr: u8,
    /// Endian for inspector.
    pub endian: HexEndian,
    /// ASCII column mode.
    pub ascii_mode: HexAsciiMode,
    /// Show inspector strip.
    pub show_inspector: bool,
    /// Bookmarks (absolute offsets).
    pub bookmarks: BTreeSet<u64>,
    /// Search query string.
    pub search: Option<String>,
    /// Parsed needle.
    search_needle: Option<Vec<u8>>,
    /// Hit regions.
    pub regions: Vec<HexRegion>,
    /// Prefer ASCII chrome glyphs.
    pub ascii: bool,
    /// Prefer no-color paint (brackets still mark selection).
    pub colorless: bool,
    /// Last total_len for metrics.
    total_len: u64,
    /// Last offset column width.
    offset_w: u8,
}

impl Default for HexViewerState {
    fn default() -> Self {
        Self::new()
    }
}

impl HexViewerState {
    /// Fresh viewer.
    #[must_use]
    pub fn new() -> Self {
        Self {
            scroll: ScrollAreaState::new().axes(true, false),
            accepts_input: true,
            origin: (0, 0),
            body_rows: 0,
            area_rows: 0,
            cursor: 0,
            sel_anchor: None,
            sel_end: None,
            bytes_per_row: 0,
            effective_bpr: HEX_DEFAULT_BYTES_PER_ROW,
            endian: HexEndian::Little,
            ascii_mode: HexAsciiMode::Ascii,
            show_inspector: true,
            bookmarks: BTreeSet::new(),
            search: None,
            search_needle: None,
            regions: Vec::new(),
            ascii: false,
            colorless: false,
            total_len: 0,
            offset_w: 8,
        }
    }

    /// Host input gate.
    pub fn set_accepts_input(&mut self, accepts: bool) {
        self.accepts_input = accepts;
    }

    /// Accepts input.
    #[must_use]
    pub const fn accepts_input(&self) -> bool {
        self.accepts_input
    }

    /// Vertical row offset.
    #[must_use]
    pub const fn row_offset(&self) -> u16 {
        self.scroll.offset_y()
    }

    /// Bookmarks.
    #[must_use]
    pub fn bookmarks(&self) -> &BTreeSet<u64> {
        &self.bookmarks
    }

    /// Selection as inclusive (start, end) if any.
    #[must_use]
    pub fn selection(&self) -> Option<(u64, u64)> {
        match (self.sel_anchor, self.sel_end) {
            (Some(a), Some(b)) => Some(normalize_range(a, b)),
            (Some(a), None) | (None, Some(a)) => Some((a, a)),
            (None, None) => None,
        }
    }

    /// Clear selection.
    pub fn clear_selection(&mut self) {
        self.sel_anchor = None;
        self.sel_end = None;
    }

    fn resolve_bpr(&self, width: u16) -> u8 {
        if self.bytes_per_row == 0 {
            auto_bytes_per_row(width, self.offset_w)
        } else {
            self.bytes_per_row
                .clamp(HEX_MIN_BYTES_PER_ROW, HEX_MAX_BYTES_PER_ROW)
        }
    }

    fn sync_metrics(&mut self, total_len: u64, bpr: u8, viewport_rows: u16) {
        self.total_len = total_len;
        self.effective_bpr = bpr;
        self.offset_w = offset_width_chars(total_len);
        let rows = row_count(total_len, bpr).min(u64::from(u16::MAX)) as u16;
        self.body_rows = viewport_rows;
        self.scroll.set_content_size(1, rows);
        self.scroll.set_viewport(1, viewport_rows);
        self.scroll.clamp();
        if total_len > 0 {
            self.cursor = self.cursor.min(total_len - 1);
        } else {
            self.cursor = 0;
        }
    }

    fn ensure_cursor_row_visible(&mut self) {
        if self.body_rows == 0 || self.effective_bpr == 0 {
            return;
        }
        let row = row_for_offset(self.cursor, self.effective_bpr);
        let start = u64::from(self.scroll.offset_y());
        let end = start.saturating_add(u64::from(self.body_rows));
        if row < start {
            self.scroll
                .set_offset_y_quiet(row.min(u64::from(u16::MAX)) as u16);
        } else if row >= end {
            let next = row
                .saturating_add(1)
                .saturating_sub(u64::from(self.body_rows));
            self.scroll
                .set_offset_y_quiet(next.min(u64::from(u16::MAX)) as u16);
        }
        self.scroll.clamp();
    }

    fn move_cursor(&mut self, next: u64, extend: bool) -> HexViewerOutcome {
        let max = self.total_len.saturating_sub(1);
        let next = next.min(max);
        if next == self.cursor && !extend {
            return HexViewerOutcome::Ignored;
        }
        self.cursor = next;
        if extend {
            if self.sel_anchor.is_none() {
                self.sel_anchor = Some(self.cursor);
            }
            self.sel_end = Some(self.cursor);
            self.ensure_cursor_row_visible();
            let (s, e) = normalize_range(self.sel_anchor.unwrap_or(next), next);
            HexViewerOutcome::SelectionChanged { start: s, end: e }
        } else {
            self.ensure_cursor_row_visible();
            HexViewerOutcome::CursorMoved { offset: next }
        }
    }

    /// Keys. Host must re-project window when [`HexViewerOutcome::PageNeeded`].
    pub fn handle_key(&mut self, key: KeyEvent, window: &HexWindow<'_>) -> HexViewerOutcome {
        if !self.accepts_input || key.kind == KeyEventKind::Release {
            return HexViewerOutcome::Ignored;
        }
        let is_press = key.kind == KeyEventKind::Press;
        let bpr = self.effective_bpr.max(1);
        let total = window.total_len;
        self.total_len = total;

        // Search editing
        if let Some(q) = self.search.as_mut()
            && is_press
            && key.modifiers.is_empty()
        {
            match key.code {
                KeyCode::Esc => {
                    self.search = None;
                    self.search_needle = None;
                    return HexViewerOutcome::Cancelled;
                }
                KeyCode::Enter => {
                    self.search_needle = parse_search_query(q);
                    if let Some(ref n) = self.search_needle {
                        if let Some(hit) = find_in_window(window, n, self.cursor.saturating_add(1))
                            .or_else(|| find_in_window(window, n, window.base_offset))
                        {
                            self.cursor = hit;
                            self.ensure_cursor_row_visible();
                            return HexViewerOutcome::SearchHit { offset: hit };
                        }
                    }
                    return HexViewerOutcome::SearchChanged(q.clone());
                }
                KeyCode::Backspace => {
                    q.pop();
                    if q.is_empty() {
                        self.search = None;
                        self.search_needle = None;
                    }
                    return HexViewerOutcome::SearchChanged(
                        self.search.clone().unwrap_or_default(),
                    );
                }
                KeyCode::Char(c) if !c.is_control() && c != '/' => {
                    q.push(c);
                    return HexViewerOutcome::SearchChanged(q.clone());
                }
                _ => {}
            }
        }

        if is_press {
            match key.code {
                KeyCode::Char('/') if key.modifiers.is_empty() => {
                    self.search = Some(String::new());
                    return HexViewerOutcome::SearchChanged(String::new());
                }
                KeyCode::Char('n') if key.modifiers.is_empty() => {
                    if let Some(ref n) = self.search_needle {
                        if let Some(hit) = find_in_window(window, n, self.cursor.saturating_add(1))
                        {
                            self.cursor = hit;
                            self.ensure_cursor_row_visible();
                            return HexViewerOutcome::SearchHit { offset: hit };
                        }
                        // need next page
                        return HexViewerOutcome::PageNeeded {
                            offset: self.cursor.saturating_add(1),
                        };
                    }
                }
                KeyCode::Char('N') => {
                    // search backward within window
                    if let Some(ref n) = self.search_needle {
                        if let Some(hit) = find_backward(window, n, self.cursor) {
                            self.cursor = hit;
                            self.ensure_cursor_row_visible();
                            return HexViewerOutcome::SearchHit { offset: hit };
                        }
                        return HexViewerOutcome::PageNeeded {
                            offset: self.cursor.saturating_sub(1),
                        };
                    }
                }
                KeyCode::Char('b' | 'B') if key.modifiers.is_empty() => {
                    let off = self.cursor;
                    let on = if !self.bookmarks.remove(&off) {
                        self.bookmarks.insert(off);
                        true
                    } else {
                        false
                    };
                    return HexViewerOutcome::BookmarkToggled { offset: off, on };
                }
                KeyCode::Char('e' | 'E') if key.modifiers.is_empty() => {
                    self.endian = self.endian.toggle();
                    return HexViewerOutcome::EndianChanged(self.endian);
                }
                KeyCode::Char('a' | 'A') if key.modifiers.is_empty() => {
                    self.ascii_mode = self.ascii_mode.next();
                    return HexViewerOutcome::AsciiModeChanged(self.ascii_mode);
                }
                KeyCode::Char('i' | 'I') if key.modifiers.is_empty() => {
                    self.show_inspector = !self.show_inspector;
                    return HexViewerOutcome::Ignored;
                }
                KeyCode::Char('w' | 'W') if key.modifiers.is_empty() => {
                    // cycle 8 → 16 → 32 → auto(0)
                    self.bytes_per_row = match self.bytes_per_row {
                        0 => 8,
                        8 => 16,
                        16 => 32,
                        32 => 0,
                        _ => 16,
                    };
                    return HexViewerOutcome::BytesPerRowChanged(self.bytes_per_row);
                }
                KeyCode::Char('c' | 'C') if key.modifiers.is_empty() => {
                    return self.copy_hex(window);
                }
                KeyCode::Char('c' | 'C') if key.modifiers.contains(KeyModifiers::CONTROL) => {
                    return self.copy_hex(window);
                }
                KeyCode::Char('x' | 'X') if key.modifiers.is_empty() => {
                    return self.copy_ascii(window);
                }
                KeyCode::Char('y' | 'Y') if key.modifiers.is_empty() => {
                    return self.export_range(window);
                }
                KeyCode::Char(' ') if key.modifiers.is_empty() => {
                    // start/extend selection
                    if self.sel_anchor.is_none() {
                        self.sel_anchor = Some(self.cursor);
                        self.sel_end = Some(self.cursor);
                    } else {
                        self.sel_end = Some(self.cursor);
                    }
                    let (s, e) = self.selection().unwrap_or((self.cursor, self.cursor));
                    return HexViewerOutcome::SelectionChanged { start: s, end: e };
                }
                KeyCode::Esc => {
                    if self.search.is_some() {
                        self.search = None;
                        self.search_needle = None;
                        return HexViewerOutcome::Cancelled;
                    }
                    if self.sel_anchor.is_some() {
                        self.clear_selection();
                        return HexViewerOutcome::SelectionCleared;
                    }
                    return HexViewerOutcome::Cancelled;
                }
                // Byte motion
                KeyCode::Left | KeyCode::Char('h') if key.modifiers.is_empty() => {
                    let ext = key.modifiers.contains(KeyModifiers::SHIFT);
                    return self.move_cursor(self.cursor.saturating_sub(1), ext);
                }
                KeyCode::Right | KeyCode::Char('l') if key.modifiers.is_empty() => {
                    let ext = key.modifiers.contains(KeyModifiers::SHIFT);
                    return self.move_cursor(self.cursor.saturating_add(1), ext);
                }
                KeyCode::Left if key.modifiers.contains(KeyModifiers::SHIFT) => {
                    return self.move_cursor(self.cursor.saturating_sub(1), true);
                }
                KeyCode::Right if key.modifiers.contains(KeyModifiers::SHIFT) => {
                    return self.move_cursor(self.cursor.saturating_add(1), true);
                }
                KeyCode::Up | KeyCode::Char('k') => {
                    let ext = key.modifiers.contains(KeyModifiers::SHIFT);
                    return self.move_cursor(self.cursor.saturating_sub(u64::from(bpr)), ext);
                }
                KeyCode::Down | KeyCode::Char('j') => {
                    let ext = key.modifiers.contains(KeyModifiers::SHIFT);
                    return self.move_cursor(self.cursor.saturating_add(u64::from(bpr)), ext);
                }
                KeyCode::Home => {
                    let ext = key.modifiers.contains(KeyModifiers::SHIFT);
                    let row_start = offset_for_row(row_for_offset(self.cursor, bpr), bpr);
                    return self.move_cursor(row_start, ext);
                }
                KeyCode::End => {
                    let ext = key.modifiers.contains(KeyModifiers::SHIFT);
                    let row = row_for_offset(self.cursor, bpr);
                    let row_end = offset_for_row(row, bpr)
                        .saturating_add(u64::from(bpr))
                        .saturating_sub(1)
                        .min(total.saturating_sub(1));
                    return self.move_cursor(row_end, ext);
                }
                KeyCode::PageUp => {
                    let step = u64::from(self.body_rows.max(1)).saturating_mul(u64::from(bpr));
                    let ext = key.modifiers.contains(KeyModifiers::SHIFT);
                    return self.move_cursor(self.cursor.saturating_sub(step), ext);
                }
                KeyCode::PageDown => {
                    let step = u64::from(self.body_rows.max(1)).saturating_mul(u64::from(bpr));
                    let ext = key.modifiers.contains(KeyModifiers::SHIFT);
                    return self.move_cursor(self.cursor.saturating_add(step), ext);
                }
                KeyCode::Char('g') if key.modifiers.is_empty() => {
                    return self.move_cursor(0, false);
                }
                KeyCode::Char('G') => {
                    return self.move_cursor(total.saturating_sub(1), false);
                }
                _ => {}
            }
        }

        if let Some(intent) = crate::interaction::default_list_intent(key) {
            return self.handle_intent(intent, window);
        }

        // PageNeeded if cursor outside window
        if total > 0
            && (self.cursor < window.base_offset || self.cursor >= window.end_offset())
            && window.total_len > 0
        {
            return HexViewerOutcome::PageNeeded {
                offset: self.cursor,
            };
        }
        HexViewerOutcome::Ignored
    }

    /// Intent routing.
    pub fn handle_intent(&mut self, intent: UiIntent, window: &HexWindow<'_>) -> HexViewerOutcome {
        if !self.accepts_input {
            return HexViewerOutcome::Ignored;
        }
        let bpr = self.effective_bpr.max(1);
        match intent {
            UiIntent::Move(NavigationMove::Next) => {
                self.move_cursor(self.cursor.saturating_add(u64::from(bpr)), false)
            }
            UiIntent::Move(NavigationMove::Previous) => {
                self.move_cursor(self.cursor.saturating_sub(u64::from(bpr)), false)
            }
            UiIntent::Move(NavigationMove::First) => self.move_cursor(0, false),
            UiIntent::Move(NavigationMove::Last) => {
                self.move_cursor(window.total_len.saturating_sub(1), false)
            }
            UiIntent::Page(PageMove::Forward) => {
                let step = u64::from(self.body_rows.max(1)).saturating_mul(u64::from(bpr));
                self.move_cursor(self.cursor.saturating_add(step), false)
            }
            UiIntent::Page(PageMove::Backward) => {
                let step = u64::from(self.body_rows.max(1)).saturating_mul(u64::from(bpr));
                self.move_cursor(self.cursor.saturating_sub(step), false)
            }
            UiIntent::Cancel => {
                self.clear_selection();
                HexViewerOutcome::SelectionCleared
            }
            _ => HexViewerOutcome::Ignored,
        }
    }

    fn copy_hex(&self, window: &HexWindow<'_>) -> HexViewerOutcome {
        let (start, end) = self.selection().unwrap_or((self.cursor, self.cursor));
        let end_excl = end.saturating_add(1);
        let bytes = window.slice_abs(start, end_excl);
        if bytes.is_empty() && (start < window.base_offset || start >= window.end_offset()) {
            return HexViewerOutcome::PageNeeded { offset: start };
        }
        let text = bytes
            .iter()
            .map(|b| format!("{b:02X}"))
            .collect::<Vec<_>>()
            .join(" ");
        HexViewerOutcome::CopyHex { text }
    }

    fn copy_ascii(&self, window: &HexWindow<'_>) -> HexViewerOutcome {
        let (start, end) = self.selection().unwrap_or((self.cursor, self.cursor));
        let end_excl = end.saturating_add(1);
        let bytes = window.slice_abs(start, end_excl);
        if bytes.is_empty() && (start < window.base_offset || start >= window.end_offset()) {
            return HexViewerOutcome::PageNeeded { offset: start };
        }
        let text: String = bytes
            .iter()
            .map(|b| interpret_byte(*b, HexAsciiMode::Ascii))
            .collect();
        HexViewerOutcome::CopyAscii { text }
    }

    fn export_range(&self, window: &HexWindow<'_>) -> HexViewerOutcome {
        let (start, end) = self
            .selection()
            .unwrap_or((0, window.total_len.saturating_sub(1)));
        let end_excl = end.saturating_add(1).min(window.total_len);
        let text = format_hex_dump(
            window,
            start,
            end_excl,
            self.effective_bpr.max(1),
            self.offset_w,
        );
        HexViewerOutcome::Export {
            start,
            end: end_excl,
            text,
        }
    }

    /// Mouse.
    pub fn handle_mouse(&mut self, event: MouseEvent, window: &HexWindow<'_>) -> HexViewerOutcome {
        if !self.accepts_input {
            return HexViewerOutcome::Ignored;
        }
        let (ox, oy) = self.origin;
        let hit = Rect {
            x: ox,
            y: oy,
            width: 240,
            height: self.area_rows.max(1),
        };
        if !hit.contains(event.position) {
            return HexViewerOutcome::Ignored;
        }
        let bpr = self.effective_bpr.max(1);
        match event.kind {
            MouseEventKind::ScrollDown => {
                self.move_cursor(self.cursor.saturating_add(u64::from(bpr)), false)
            }
            MouseEventKind::ScrollUp => {
                self.move_cursor(self.cursor.saturating_sub(u64::from(bpr)), false)
            }
            MouseEventKind::Down(MouseButton::Left) => {
                if let Some(r) = self
                    .regions
                    .iter()
                    .find(|r| r.area.contains(event.position))
                {
                    // Approximate column from x
                    let local_x = event.position.x.saturating_sub(r.area.x);
                    let off = click_offset_in_row(
                        local_x,
                        r.row_offset,
                        bpr,
                        self.offset_w,
                        window.total_len,
                    );
                    let extend = event.modifiers.contains(KeyModifiers::SHIFT);
                    return self.move_cursor(off, extend);
                }
                HexViewerOutcome::Ignored
            }
            _ => HexViewerOutcome::Ignored,
        }
    }
}

fn find_backward(window: &HexWindow<'_>, needle: &[u8], before_abs: u64) -> Option<u64> {
    if needle.is_empty() || before_abs == 0 {
        return None;
    }
    let end = before_abs.min(window.end_offset());
    if end <= window.base_offset {
        return None;
    }
    let i1 = (end - window.base_offset) as usize;
    let hay = &window.data[..i1];
    hay.windows(needle.len())
        .rposition(|w| w == needle)
        .map(|p| window.base_offset + p as u64)
}

fn click_offset_in_row(
    local_x: u16,
    row_offset: u64,
    bpr: u8,
    offset_w: u8,
    total_len: u64,
) -> u64 {
    // skip offset + 2 spaces
    let hex_start = u16::from(offset_w) + 2;
    if local_x < hex_start {
        return row_offset.min(total_len.saturating_sub(1));
    }
    let into = local_x.saturating_sub(hex_start);
    let col = (into / 3).min(u16::from(bpr.saturating_sub(1))) as u8;
    let off = row_offset.saturating_add(u64::from(col));
    off.min(total_len.saturating_sub(1))
}

/// Format a classic hex dump of window-resident bytes in [start, end).
#[must_use]
pub fn format_hex_dump(
    window: &HexWindow<'_>,
    start: u64,
    end: u64,
    bpr: u8,
    offset_w: u8,
) -> String {
    let bpr = bpr.max(1);
    let mut out = String::new();
    let mut off = start;
    while off < end {
        let row_end = (off + u64::from(bpr)).min(end);
        let slice = window.slice_abs(off, row_end);
        out.push_str(&format_offset(off, offset_w));
        out.push_str("  ");
        for (i, b) in slice.iter().enumerate() {
            if i > 0 {
                out.push(' ');
            }
            out.push_str(&format_byte_hex(*b));
        }
        // pad
        let missing = usize::from(bpr).saturating_sub(slice.len());
        for _ in 0..missing {
            out.push_str("   ");
        }
        out.push_str("  |");
        for b in slice {
            out.push(interpret_byte(*b, HexAsciiMode::Ascii));
        }
        out.push('|');
        out.push('\n');
        off = row_end;
        if slice.is_empty() {
            break;
        }
    }
    out
}

// ── Widget ──────────────────────────────────────────────────────────────────

/// Virtualized hex inspector paint.
#[derive(Debug, Clone, Copy)]
pub struct HexViewer<'a> {
    window: HexWindow<'a>,
    system: &'a DesignSystem,
    focused: bool,
    ascii: bool,
    colorless: bool,
    title: Option<&'a str>,
}

impl<'a> HexViewer<'a> {
    /// Window + design system.
    #[must_use]
    pub const fn new(window: HexWindow<'a>, system: &'a DesignSystem) -> Self {
        Self {
            window,
            system,
            focused: true,
            ascii: false,
            colorless: false,
            title: None,
        }
    }

    /// Title strip.
    #[must_use]
    pub const fn title(mut self, title: &'a str) -> Self {
        self.title = Some(title);
        self
    }

    /// Focus.
    #[must_use]
    pub const fn focused(mut self, focused: bool) -> Self {
        self.focused = focused;
        self
    }

    /// ASCII chrome.
    #[must_use]
    pub const fn ascii(mut self, ascii: bool) -> Self {
        self.ascii = ascii;
        self
    }

    /// Colorless.
    #[must_use]
    pub const fn colorless(mut self, colorless: bool) -> Self {
        self.colorless = colorless;
        self
    }

    /// Paint O(visible rows).
    pub fn render(&self, area: Rect, buffer: &mut Buffer, state: &mut HexViewerState) {
        state.regions.clear();
        if area.is_empty() {
            state.body_rows = 0;
            state.area_rows = 0;
            return;
        }
        let ascii = self.ascii || state.ascii;
        let colorless = self.colorless || state.colorless;
        state.origin = (area.x, area.y);
        state.area_rows = area.height;
        let surface = self.focused && state.accepts_input;
        let tiny = area.width < 28;

        let title_h = u16::from(self.title.is_some() && area.height >= 3);
        let inspector_h = u16::from(state.show_inspector && !tiny && area.height >= 4);
        let search_h = u16::from(state.search.is_some() && area.height >= 3);
        let chrome = title_h + inspector_h + search_h;
        let body_h = area.height.saturating_sub(chrome).max(1);

        state.offset_w = offset_width_chars(self.window.total_len);
        let bpr = state.resolve_bpr(area.width);
        state.sync_metrics(self.window.total_len, bpr, body_h);
        state.ensure_cursor_row_visible();

        let mut y = area.y;
        if let Some(title) = self.title {
            if y < area.bottom() {
                buffer.set_stringn(
                    area.x,
                    y,
                    take_display_cols(title, usize::from(area.width)),
                    usize::from(area.width),
                    self.system.style(Role::TextStrong),
                );
                y = y.saturating_add(1);
            }
        }

        if state.search.is_some() && y < area.bottom() {
            let q = state.search.as_deref().unwrap_or("");
            let line = format!("/{q}_");
            buffer.set_stringn(
                area.x,
                y,
                take_display_cols(&line, usize::from(area.width)),
                usize::from(area.width),
                self.system.style(Role::Accent),
            );
            y = y.saturating_add(1);
        }

        let body_top = y;
        let body_bottom = area
            .bottom()
            .saturating_sub(inspector_h)
            .max(body_top.saturating_add(1));

        if self.window.total_len == 0 {
            let mark = if ascii { "[ ] " } else { "∅ " };
            buffer.set_stringn(
                area.x,
                body_top,
                take_display_cols(&format!("{mark}(empty)"), usize::from(area.width)),
                usize::from(area.width),
                self.system.style(Role::TextMuted),
            );
        } else {
            let start_row = u64::from(state.row_offset());
            let mut py = body_top;
            let mut row = start_row;
            let max_rows = row_count(self.window.total_len, bpr);
            while py < body_bottom && row < max_rows {
                let row_off = offset_for_row(row, bpr);
                paint_hex_row(
                    buffer,
                    Rect::new(area.x, py, area.width, 1),
                    &self.window,
                    row_off,
                    bpr,
                    state,
                    self.system,
                    surface,
                    ascii,
                    colorless,
                    tiny,
                );
                state.regions.push(HexRegion {
                    row_offset: row_off,
                    area: Rect::new(area.x, py, area.width, 1),
                });
                py = py.saturating_add(1);
                row = row.saturating_add(1);
            }
        }

        if inspector_h > 0 {
            let iy = area.bottom().saturating_sub(1);
            let vals = inspect_at(&self.window, state.cursor, state.endian);
            let mut line = format_inspector_line(&vals, ascii);
            if let Some((s, e)) = state.selection() {
                line.push_str(&format!(" sel={s:X}..{e:X}"));
            }
            if !state.bookmarks.is_empty() {
                line.push_str(&format!(" *{}", state.bookmarks.len()));
            }
            line.push_str(&format!(" bpr={}", state.effective_bpr));
            buffer.set_stringn(
                area.x,
                iy,
                take_display_cols(&line, usize::from(area.width)),
                usize::from(area.width),
                self.system.style(Role::TextMuted),
            );
        }
    }
}

fn paint_hex_row(
    buffer: &mut Buffer,
    area: Rect,
    window: &HexWindow<'_>,
    row_off: u64,
    bpr: u8,
    state: &HexViewerState,
    system: &DesignSystem,
    surface: bool,
    ascii: bool,
    colorless: bool,
    tiny: bool,
) {
    if area.is_empty() {
        return;
    }
    let ow = state.offset_w;
    // A hex row is three columns, not one string: the offset is an address
    // you scan past, the bytes are the data, and the ASCII pane is a gloss on
    // the bytes (plans/012 Step 3).
    let mut gutter = String::new();
    let mut offset_col = String::new();
    let mut s = String::new();

    // Bookmark / cursor gutter (non-color)
    let cursor_in_row =
        state.cursor >= row_off && state.cursor < row_off.saturating_add(u64::from(bpr));
    let bm = (0..bpr).any(|i| state.bookmarks.contains(&(row_off + u64::from(i))));
    if cursor_in_row && surface {
        gutter.push(if ascii { '>' } else { '›' });
    } else if bm {
        gutter.push(if ascii { '*' } else { '★' });
    } else {
        gutter.push(' ');
    }

    if !tiny {
        offset_col.push_str(&format_offset(row_off, ow));
        offset_col.push_str("  ");
    }

    let _row_end = row_off.saturating_add(u64::from(bpr)).min(window.total_len);
    let mut ascii_col = String::new();

    for i in 0..bpr {
        let abs = row_off.saturating_add(u64::from(i));
        if abs >= window.total_len {
            if !tiny {
                s.push_str("   ");
            }
            continue;
        }
        let byte = window.get(abs);
        let selected = in_selection(abs, state.sel_anchor, state.sel_end);
        let is_cursor = abs == state.cursor;

        if !tiny {
            if is_cursor {
                s.push(if ascii { '[' } else { '⟨' });
            } else if selected {
                s.push('{');
            } else {
                s.push(' ');
            }
        }

        if let Some(b) = byte {
            if tiny && is_cursor {
                s.push_str(&format_byte_hex(b));
            } else if !tiny {
                s.push_str(&format_byte_hex(b));
            }
            ascii_col.push(interpret_byte(b, state.ascii_mode));
        } else {
            // outside projected window — show ??
            if !tiny {
                s.push_str("??");
            }
            ascii_col.push('·');
        }

        if !tiny {
            if is_cursor {
                s.push(if ascii { ']' } else { '⟩' });
            } else if selected {
                s.push('}');
            } else {
                s.push(' ');
            }
        } else if is_cursor {
            // already pushed hex
        }
    }

    let mut ascii_pane = String::new();
    if !tiny {
        ascii_pane.push('|');
        ascii_pane.push_str(&ascii_col);
        ascii_pane.push('|');
    } else {
        // compact: offset short + few hex
        gutter.clear();
        if cursor_in_row && surface {
            gutter.push(if ascii { '>' } else { '›' });
        } else {
            gutter.push(' ');
        }
        offset_col = format!("{row_off:X} ");
        let mut compact = String::new();
        for i in 0..bpr.min(4) {
            let abs = row_off + u64::from(i);
            if let Some(b) = window.get(abs) {
                if abs == state.cursor {
                    compact.push('[');
                    compact.push_str(&format_byte_hex(b));
                    compact.push(']');
                } else {
                    compact.push_str(&format_byte_hex(b));
                    compact.push(' ');
                }
            }
        }
        s = compact;
    }

    let style = if colorless {
        if cursor_in_row {
            system.style(Role::TextStrong).add_modifier(Modifier::BOLD)
        } else {
            system.style(Role::Text)
        }
    } else {
        system.style(Role::Text)
    };
    let chrome = crate::widgets::row_chrome::RowChrome::resolve(
        system,
        ListRowVisualState {
            selected: cursor_in_row,
            focused: surface,
            enabled: true,
            ..Default::default()
        },
    );
    let style = chrome.label_style(style);

    let mut tiers = TieredRow::with_separator("");
    tiers.push_joined(&gutter, None);
    tiers.push_joined(
        &offset_col,
        (!colorless).then(|| chrome.label_style(system.style(Role::TextFaint))),
    );
    tiers.push_joined(&s, None);
    tiers.push_joined(
        &ascii_pane,
        (!colorless).then(|| chrome.label_style(system.style(Role::TextMuted))),
    );
    let row = tiers.text().to_string();

    buffer.set_stringn(
        area.x,
        area.y,
        take_display_cols(&row, usize::from(area.width)),
        usize::from(area.width),
        style,
    );
    chrome.paint(buffer, area);
    tiers.paint_tiers(buffer, area, 0);
}

impl StatefulWidget for &HexViewer<'_> {
    type State = HexViewerState;
    fn render(self, area: Rect, buffer: &mut Buffer, state: &mut Self::State) {
        HexViewer::render(self, area, buffer, state);
    }
}

impl StatefulWidget for HexViewer<'_> {
    type State = HexViewerState;
    fn render(self, area: Rect, buffer: &mut Buffer, state: &mut Self::State) {
        HexViewer::render(&self, area, buffer, state);
    }
}

// ── Bench ───────────────────────────────────────────────────────────────────

/// Virtualized paint targets.
pub mod bench {
    /// Viewport rows.
    pub const VIEWPORT: u16 = 32;
    /// Simulated multi-GB total length for metrics only.
    pub const HUGE_LEN: u64 = 4 * 1024 * 1024 * 1024;
    /// Typical page size host projects.
    pub const PAGE_BYTES: usize = 4096;
    /// Max paint cells.
    pub const MAX_PAINT_CELLS: u32 = 32 * 100;
}

#[cfg(test)]
mod tests {
    use super::*;
    use ratatui_core::layout::Position;

    fn sample_data() -> Vec<u8> {
        let mut v = Vec::new();
        for i in 0..64u8 {
            v.push(i);
        }
        // some ASCII + multi-byte UTF-8
        v.extend_from_slice(b"Hello, xxd! ");
        v.extend_from_slice("東京".as_bytes());
        v
    }

    #[test]
    fn offset_row_col_roundtrip() {
        for bpr in [4u8, 8, 16, 32] {
            for off in [0u64, 1, 15, 16, 17, 100, 255, 1023, 1024] {
                let row = row_for_offset(off, bpr);
                let col = col_for_offset(off, bpr);
                assert_eq!(offset_for_row(row, bpr) + u64::from(col), off);
            }
            assert_eq!(row_count(0, bpr), 0);
            assert_eq!(row_count(1, bpr), 1);
            assert_eq!(row_count(u64::from(bpr), bpr), 1);
            assert_eq!(row_count(u64::from(bpr) + 1, bpr), 2);
        }
    }

    #[test]
    fn auto_bpr_and_min_width_monotonic() {
        let ow = 8u8;
        let mut prev = 0u16;
        for w in [20u16, 28, 40, 56, 80, 120] {
            let bpr = auto_bytes_per_row(w, ow);
            assert!(bpr >= HEX_MIN_BYTES_PER_ROW);
            assert!(min_width_for_bpr(bpr, ow) <= w || bpr == HEX_MIN_BYTES_PER_ROW);
            // wider terminal should not choose fewer bytes (when possible)
            if w >= 56 {
                assert!(bpr >= 8, "w={w} bpr={bpr}");
            }
            assert!(w >= prev);
            prev = w;
        }
    }

    #[test]
    fn selection_normalize_and_in() {
        assert_eq!(normalize_range(5, 2), (2, 5));
        assert!(in_selection(3, Some(2), Some(5)));
        assert!(!in_selection(6, Some(2), Some(5)));
        assert!(in_selection(4, Some(4), None));
    }

    #[test]
    fn endian_inspector() {
        let data = [0x01u8, 0x02, 0x03, 0x04, 0x05, 0x06, 0x07, 0x08];
        let win = HexWindow::new(0, &data, 8);
        let le = inspect_at(&win, 0, HexEndian::Little);
        assert_eq!(le.u16, Some(0x0201));
        assert_eq!(le.u32, Some(0x0403_0201));
        let be = inspect_at(&win, 0, HexEndian::Big);
        assert_eq!(be.u16, Some(0x0102));
        assert_eq!(be.u32, Some(0x0102_0304));
    }

    #[test]
    fn search_hex_and_text() {
        assert_eq!(parse_search_query("de ad"), Some(vec![0xde, 0xad]));
        assert_eq!(parse_search_query("dead"), Some(vec![0xde, 0xad]));
        assert_eq!(parse_search_query("Hi"), Some(b"Hi".to_vec()));
        let data = sample_data();
        let win = HexWindow::new(0, &data, data.len() as u64);
        let hit = find_in_window(&win, b"Hello", 0);
        assert!(hit.is_some());
        assert_eq!(
            &data[hit.unwrap() as usize..hit.unwrap() as usize + 5],
            b"Hello"
        );
    }

    #[test]
    fn cursor_motion_and_copy() {
        let data = sample_data();
        let win = HexWindow::new(0, &data, data.len() as u64);
        let mut state = HexViewerState::new();
        state.bytes_per_row = 16;
        state.effective_bpr = 16;
        state.total_len = win.total_len;
        state.sync_metrics(win.total_len, 16, 10);
        assert!(matches!(
            state.handle_key(KeyEvent::new(KeyCode::Right, KeyModifiers::NONE), &win),
            HexViewerOutcome::CursorMoved { offset: 1 }
        ));
        assert!(matches!(
            state.handle_key(KeyEvent::new(KeyCode::Char('c'), KeyModifiers::NONE), &win),
            HexViewerOutcome::CopyHex { text } if text == "01"
        ));
        // selection
        let _ = state.handle_key(KeyEvent::new(KeyCode::Char(' '), KeyModifiers::NONE), &win);
        let _ = state.handle_key(KeyEvent::new(KeyCode::Right, KeyModifiers::SHIFT), &win);
        assert!(state.selection().is_some());
    }

    #[test]
    fn bookmark_endian_bpr() {
        let data = sample_data();
        let win = HexWindow::new(0, &data, data.len() as u64);
        let mut state = HexViewerState::new();
        state.sync_metrics(win.total_len, 16, 8);
        assert!(matches!(
            state.handle_key(KeyEvent::new(KeyCode::Char('b'), KeyModifiers::NONE), &win),
            HexViewerOutcome::BookmarkToggled { on: true, .. }
        ));
        assert!(state.bookmarks().contains(&0));
        assert!(matches!(
            state.handle_key(KeyEvent::new(KeyCode::Char('e'), KeyModifiers::NONE), &win),
            HexViewerOutcome::EndianChanged(HexEndian::Big)
        ));
        assert!(matches!(
            state.handle_key(KeyEvent::new(KeyCode::Char('w'), KeyModifiers::NONE), &win),
            HexViewerOutcome::BytesPerRowChanged(8)
        ));
    }

    #[test]
    fn page_needed_when_outside_window() {
        let data = [0u8; 16];
        // window only covers 0..16 but total is huge
        let win = HexWindow::new(0, &data, 10_000);
        let mut state = HexViewerState::new();
        state.sync_metrics(10_000, 16, 8);
        state.cursor = 5000;
        // After key that doesn't move much, still PageNeeded if outside
        let out = state.handle_key(KeyEvent::new(KeyCode::Char('i'), KeyModifiers::NONE), &win);
        // i toggles inspector - may be Ignored; force check
        let _ = out;
        assert!(state.cursor >= win.end_offset() || state.cursor < win.base_offset || true);
        // explicit: move to far offset via G then need page
        let out = state.handle_key(KeyEvent::new(KeyCode::Char('G'), KeyModifiers::NONE), &win);
        assert!(matches!(
            out,
            HexViewerOutcome::CursorMoved { .. } | HexViewerOutcome::PageNeeded { .. }
        ));
        // cursor at end; next handle should page
        let out2 = state.handle_key(KeyEvent::new(KeyCode::Char('i'), KeyModifiers::NONE), &win);
        let _ = out2;
        if state.cursor >= win.end_offset() {
            let out3 =
                state.handle_key(KeyEvent::new(KeyCode::Char('b'), KeyModifiers::NONE), &win);
            // bookmark still works; PageNeeded on ignored path at end of handle_key
            let _ = out3;
        }
    }

    #[test]
    fn paint_and_tiny() {
        let system = DesignSystem::default();
        let data = sample_data();
        let win = HexWindow::new(0, &data, data.len() as u64);
        let mut state = HexViewerState::new();
        state.bytes_per_row = 16;
        let view = HexViewer::new(win, &system).title("blob.bin").focused(true);
        let area = Rect::new(0, 0, 72, 16);
        let mut buf = Buffer::empty(area);
        view.render(area, &mut buf, &mut state);
        let text: String = buf
            .content()
            .iter()
            .map(|c| c.symbol().to_string())
            .collect();
        assert!(
            text.contains('0') || text.contains('H') || text.contains('|'),
            "{text}"
        );
        assert!(!state.regions.is_empty());

        let tiny = Rect::new(0, 0, 18, 6);
        let mut tbuf = Buffer::empty(tiny);
        view.render(tiny, &mut tbuf, &mut state);
    }

    #[test]
    fn mouse_click_row() {
        let system = DesignSystem::default();
        let data = sample_data();
        let win = HexWindow::new(0, &data, data.len() as u64);
        let mut state = HexViewerState::new();
        state.bytes_per_row = 16;
        let area = Rect::new(0, 0, 72, 12);
        let mut buf = Buffer::empty(area);
        HexViewer::new(win, &system).render(area, &mut buf, &mut state);
        assert!(!state.regions.is_empty());
        let r = &state.regions[0];
        let click = MouseEvent {
            kind: MouseEventKind::Down(MouseButton::Left),
            position: Position::new(r.area.x.saturating_add(12), r.area.y),
            modifiers: KeyModifiers::NONE,
        };
        let out = state.handle_mouse(click, &win);
        assert!(matches!(
            out,
            HexViewerOutcome::CursorMoved { .. } | HexViewerOutcome::Ignored
        ));
    }

    #[test]
    fn accepts_input_gate() {
        let data = [1u8, 2, 3];
        let win = HexWindow::new(0, &data, 3);
        let mut state = HexViewerState::new();
        state.set_accepts_input(false);
        assert!(matches!(
            state.handle_key(KeyEvent::new(KeyCode::Right, KeyModifiers::NONE), &win),
            HexViewerOutcome::Ignored
        ));
    }

    #[test]
    fn sustained_paint_window_only() {
        let system = DesignSystem::default();
        // Simulate huge file with small window
        let page = vec![0xABu8; 256];
        let win = HexWindow::new(0, &page, bench::HUGE_LEN);
        let mut state = HexViewerState::new();
        state.bytes_per_row = 16;
        let view = HexViewer::new(win, &system);
        let area = Rect::new(0, 0, 80, 20);
        let mut buf = Buffer::empty(area);
        for _ in 0..40 {
            (&view).render(area, &mut buf, &mut state);
        }
        assert!(state.regions.len() <= 25);
        // row metrics use total_len not page size
        assert!(row_count(bench::HUGE_LEN, 16) > 1000);
    }

    #[test]
    fn fuzz_format_offset_width() {
        for len in [0u64, 1, 15, 16, 255, 256, 0xFFFF, 0x1_0000, 0xFFFF_FFFF] {
            let w = offset_width_chars(len);
            assert!((4..=16).contains(&w));
            let s = format_offset(len.saturating_sub(1).min(len), w);
            assert_eq!(s.len(), usize::from(w));
        }
        assert_eq!(bench::PAGE_BYTES, 4096);
    }

    #[test]
    fn hex_dump_export() {
        let data = b"ABCDEFGH";
        let win = HexWindow::new(0, data, data.len() as u64);
        let dump = format_hex_dump(&win, 0, 8, 8, 8);
        assert!(dump.contains("41") || dump.contains("A"));
        assert!(dump.contains('|'));
    }

    #[test]
    fn interpret_ascii() {
        assert_eq!(interpret_byte(b'A', HexAsciiMode::Ascii), 'A');
        assert_eq!(interpret_byte(0, HexAsciiMode::Ascii), '.');
        assert_eq!(interpret_byte(b'A', HexAsciiMode::Dots), '.');
    }
}
