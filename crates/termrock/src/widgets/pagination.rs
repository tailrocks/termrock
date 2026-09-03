// SPDX-FileCopyrightText: 2026 Alexey Zhokhov
// SPDX-License-Identifier: Apache-2.0

//! Page navigation for remote / bounded result sets — **not** scroll virtualization.
//!
//! **Mission.** API tables, search hits, and database clients need previous/next,
//! page numbers, unknown totals, page size, loading, and jump-to-page — while
//! the **application owns fetching**. TermRock only models the control and emits
//! typed [`PageRequest`]s.
//!
//! **vs virtualization ([`Virtualizer`](super::Virtualizer), lists, grids).**
//! Virtualization paints a window over an **already-local** (or progressively
//! streamed) collection. Pagination requests **discrete pages** from a remote
//! or expensive source. Prefer virtualization when all (or a large window of)
//! rows are in memory; prefer Pagination when the host must fetch page N.
//!
//! Research: shadcn Pagination, database clients, API result browsers.
use ratatui_core::{buffer::Buffer, layout::Rect, style::Modifier, widgets::StatefulWidget};

use crate::{
    input::{KeyCode, KeyEvent, KeyModifiers, MouseButton, MouseEvent, MouseEventKind},
    interaction::{SemanticNode, SemanticRole, SemanticScene, SemanticState, UiIntent},
    style::{DesignSystem, Role},
    text::{display_cols, take_display_cols},
};

/// Width under which control drops to compact summary.
pub const PAGINATION_COMPACT_MAX_WIDTH: u16 = 48;
/// Width under which only prev/next + short summary remain.
pub const PAGINATION_MINIMAL_MAX_WIDTH: u16 = 24;

// ── Model ───────────────────────────────────────────────────────────────────

/// Typed page request (host performs I/O).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct PageRequest {
    /// 1-based page index.
    pub page: u32,
    /// Page size (items per page).
    pub page_size: u32,
}

impl PageRequest {
    /// Construct (page clamped to ≥ 1, size ≥ 1).
    #[must_use]
    pub const fn new(page: u32, page_size: u32) -> Self {
        Self {
            page: if page == 0 { 1 } else { page },
            page_size: if page_size == 0 { 1 } else { page_size },
        }
    }

    /// 0-based item offset: `(page - 1) * page_size`.
    #[must_use]
    pub const fn offset(self) -> u64 {
        let p = self.page.saturating_sub(1) as u64;
        p.saturating_mul(self.page_size as u64)
    }

    /// Limit / page size as usize for host APIs.
    #[must_use]
    pub const fn limit(self) -> u32 {
        self.page_size
    }
}

/// Knowledge of total item count.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
#[non_exhaustive]
pub enum PageTotal {
    /// Total unknown (infinite scroll-style APIs with only next cursor).
    #[default]
    Unknown,
    /// Exact total item count.
    Known(u64),
    /// Lower bound (e.g. “at least N” from partial count).
    AtLeast(u64),
}

impl PageTotal {
    /// Stable id.
    #[must_use]
    pub const fn id(self) -> &'static str {
        match self {
            Self::Unknown => "unknown",
            Self::Known(_) => "known",
            Self::AtLeast(_) => "at-least",
        }
    }
}

/// Layout density for the control.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
#[non_exhaustive]
pub enum PaginationPresentation {
    /// First/prev, page numbers, next/last, size, summary.
    #[default]
    Full,
    /// Prev/next + page summary (+ optional jump).
    Compact,
    /// Prev/next + short “p/n” only.
    Minimal,
}

impl PaginationPresentation {
    /// Stable id.
    #[must_use]
    pub const fn id(self) -> &'static str {
        match self {
            Self::Full => "full",
            Self::Compact => "compact",
            Self::Minimal => "minimal",
        }
    }
}

/// Focusable part inside the control (single host Tab stop).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
#[non_exhaustive]
pub enum PaginationPart {
    /// First page.
    First,
    /// Previous page.
    #[default]
    Prev,
    /// A numbered page button (index into visible page number strip).
    PageButton(u8),
    /// Next page.
    Next,
    /// Last page.
    Last,
    /// Page-size cycle control.
    PageSize,
    /// Jump-to-page field.
    Jump,
}

impl PaginationPart {
    /// Stable id.
    #[must_use]
    pub fn id(self) -> String {
        match self {
            Self::First => "first".into(),
            Self::Prev => "prev".into(),
            Self::PageButton(i) => format!("page-{i}"),
            Self::Next => "next".into(),
            Self::Last => "last".into(),
            Self::PageSize => "page-size".into(),
            Self::Jump => "jump".into(),
        }
    }
}

/// Outcomes — **no fetch** inside TermRock.
#[derive(Debug, Clone, PartialEq, Eq)]
#[non_exhaustive]
pub enum PaginationOutcome {
    /// No effect.
    Ignored,
    /// Focus / draft / chrome.
    Changed,
    /// Host should load this page.
    PageRequested {
        /// Request.
        request: PageRequest,
    },
    /// Page size changed; host should reload with new size (usually page 1).
    PageSizeChanged {
        /// New size.
        page_size: u32,
        /// Follow-up request.
        request: PageRequest,
    },
    /// Jump entry activated.
    JumpStarted,
    /// Jump cancelled.
    JumpCancelled,
    /// Presentation auto-changed.
    PresentationChanged {
        /// Presentation.
        presentation: PaginationPresentation,
    },
}

// ── Guidance ────────────────────────────────────────────────────────────────

/// When to prefer virtualization over Pagination.
///
/// Use **virtualization** when:
/// - Rows are already local or cheaply streamable into one buffer.
/// - User expects continuous scroll, not discrete “page 3 of 12”.
/// - Dataset is large but **one connection / one query window** is enough.
///
/// Use **Pagination** when:
/// - Each page is a distinct remote fetch (SQL `LIMIT/OFFSET`, REST `?page=`).
/// - Total may be unknown or expensive.
/// - Page size is a product control (10 / 25 / 50 / 100).
pub mod guidance {
    /// Handbook / Studio string.
    pub const WHEN_VIRTUALIZE: &str = "Prefer Virtualizer/lists when data is local or continuously windowed; \
         Pagination when the host must request discrete remote pages.";
}

// ── State ───────────────────────────────────────────────────────────────────

/// Pagination control state.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PaginationState {
    page: u32,
    page_size: u32,
    total: PageTotal,
    loading: bool,
    part: PaginationPart,
    jump_draft: String,
    jump_active: bool,
    focused: bool,
    enabled: bool,
    /// Cycle options for page size (default 10,25,50,100).
    page_size_options: Vec<u32>,
    presentation: PaginationPresentation,
    /// Visible page numbers from last paint (1-based).
    visible_pages: Vec<u32>,
    hits: Vec<(PaginationPart, Rect)>,
    root: Rect,
}

impl Default for PaginationState {
    fn default() -> Self {
        Self::new(1, 25, PageTotal::Unknown)
    }
}

impl PaginationState {
    /// Start on `page` (1-based) with `page_size`.
    #[must_use]
    pub fn new(page: u32, page_size: u32, total: PageTotal) -> Self {
        Self {
            page: page.max(1),
            page_size: page_size.max(1),
            total,
            loading: false,
            part: PaginationPart::Prev,
            jump_draft: String::new(),
            jump_active: false,
            focused: false,
            enabled: true,
            page_size_options: vec![10, 25, 50, 100],
            presentation: PaginationPresentation::Full,
            visible_pages: Vec::new(),
            hits: Vec::new(),
            root: Rect::default(),
        }
    }

    /// Page size options.
    #[must_use]
    pub fn with_page_sizes(mut self, sizes: impl IntoIterator<Item = u32>) -> Self {
        let mut v: Vec<u32> = sizes.into_iter().filter(|s| *s > 0).collect();
        if v.is_empty() {
            v = vec![10, 25, 50, 100];
        }
        self.page_size_options = v;
        self
    }

    /// Current page (1-based).
    #[must_use]
    pub const fn page(&self) -> u32 {
        self.page
    }

    /// Page size.
    #[must_use]
    pub const fn page_size(&self) -> u32 {
        self.page_size
    }

    /// Total knowledge.
    #[must_use]
    pub const fn total(&self) -> PageTotal {
        self.total
    }

    /// Loading.
    #[must_use]
    pub const fn is_loading(&self) -> bool {
        self.loading
    }

    /// Current request.
    #[must_use]
    pub const fn request(&self) -> PageRequest {
        PageRequest::new(self.page, self.page_size)
    }

    /// Total pages when computable.
    #[must_use]
    pub fn page_count(&self) -> Option<u32> {
        let items = match self.total {
            PageTotal::Known(n) => n,
            PageTotal::AtLeast(_) | PageTotal::Unknown => return None,
        };
        if self.page_size == 0 {
            return Some(1);
        }
        let pages = items.div_ceil(u64::from(self.page_size)).max(1);
        Some(u32::try_from(pages).unwrap_or(u32::MAX))
    }

    /// Whether previous is enabled.
    #[must_use]
    pub const fn can_prev(&self) -> bool {
        self.enabled && !self.loading && self.page > 1
    }

    /// Whether next is enabled.
    #[must_use]
    pub fn can_next(&self) -> bool {
        if !self.enabled || self.loading {
            return false;
        }
        match self.page_count() {
            Some(n) => self.page < n,
            None => true, // unknown: allow next until host disables via total/loading
        }
    }

    /// Whether first is enabled.
    #[must_use]
    pub const fn can_first(&self) -> bool {
        self.can_prev()
    }

    /// Whether last is enabled.
    #[must_use]
    pub fn can_last(&self) -> bool {
        self.page_count()
            .is_some_and(|n| self.enabled && !self.loading && self.page < n)
    }

    /// Presentation.
    #[must_use]
    pub const fn presentation(&self) -> PaginationPresentation {
        self.presentation
    }

    /// Focused part.
    #[must_use]
    pub const fn part(&self) -> PaginationPart {
        self.part
    }

    /// Focus control.
    pub fn set_focused(&mut self, on: bool) {
        self.focused = on;
        if !on {
            self.jump_active = false;
            self.jump_draft.clear();
        }
    }

    /// Enabled.
    pub fn set_enabled(&mut self, on: bool) {
        self.enabled = on;
    }

    /// Host projects total after fetch.
    pub fn set_total(&mut self, total: PageTotal) {
        self.total = total;
        self.clamp_page();
    }

    /// Host sets loading while fetch in flight.
    pub fn set_loading(&mut self, on: bool) {
        self.loading = on;
    }

    /// Host confirms page after successful fetch (or sets optimistically).
    pub fn set_page(&mut self, page: u32) {
        self.page = page.max(1);
        self.clamp_page();
    }
    fn clamp_page(&mut self) {
        if let Some(n) = self.page_count() {
            self.page = self.page.min(n).max(1);
        }
    }

    /// Presentation for width.
    #[must_use]
    pub fn presentation_for_width(width: u16) -> PaginationPresentation {
        if width < PAGINATION_MINIMAL_MAX_WIDTH {
            PaginationPresentation::Minimal
        } else if width < PAGINATION_COMPACT_MAX_WIDTH {
            PaginationPresentation::Compact
        } else {
            PaginationPresentation::Full
        }
    }

    /// Summary string for paint / a11y.
    #[must_use]
    pub fn summary(&self) -> String {
        let start = self.request().offset().saturating_add(1);
        let end = self
            .request()
            .offset()
            .saturating_add(u64::from(self.page_size));
        match self.total {
            PageTotal::Known(n) => {
                let end = end.min(n);
                if n == 0 {
                    "0 items".into()
                } else {
                    format!("{start}–{end} of {n}")
                }
            }
            PageTotal::AtLeast(n) => format!("{start}–{end} of {n}+"),
            PageTotal::Unknown => {
                if let Some(pc) = self.page_count() {
                    format!("Page {}/{}", self.page, pc)
                } else {
                    format!("Page {}", self.page)
                }
            }
        }
    }

    /// Compact page label.
    #[must_use]
    pub fn page_label(&self) -> String {
        match self.page_count() {
            Some(n) => format!("{}/{}", self.page, n),
            None => format!("{}/?", self.page),
        }
    }

    fn emit_page(&mut self, page: u32) -> PaginationOutcome {
        if self.loading || !self.enabled {
            return PaginationOutcome::Ignored;
        }
        let page = page.max(1);
        if let Some(n) = self.page_count() {
            if page > n {
                return PaginationOutcome::Ignored;
            }
        }
        if page == self.page {
            return PaginationOutcome::Ignored;
        }
        self.page = page;
        PaginationOutcome::PageRequested {
            request: self.request(),
        }
    }

    /// Go to page (public).
    pub fn go_to(&mut self, page: u32) -> PaginationOutcome {
        self.emit_page(page)
    }

    /// Next.
    pub fn next(&mut self) -> PaginationOutcome {
        if !self.can_next() {
            return PaginationOutcome::Ignored;
        }
        self.emit_page(self.page.saturating_add(1))
    }

    /// Previous.
    pub fn prev(&mut self) -> PaginationOutcome {
        if !self.can_prev() {
            return PaginationOutcome::Ignored;
        }
        self.emit_page(self.page.saturating_sub(1))
    }

    /// First.
    pub fn first(&mut self) -> PaginationOutcome {
        if !self.can_first() {
            return PaginationOutcome::Ignored;
        }
        self.emit_page(1)
    }

    /// Last.
    pub fn last(&mut self) -> PaginationOutcome {
        let Some(n) = self.page_count() else {
            return PaginationOutcome::Ignored;
        };
        if !self.can_last() {
            return PaginationOutcome::Ignored;
        }
        self.emit_page(n)
    }

    /// Cycle page size.
    pub fn cycle_page_size(&mut self) -> PaginationOutcome {
        if !self.enabled || self.loading || self.page_size_options.is_empty() {
            return PaginationOutcome::Ignored;
        }
        let pos = self
            .page_size_options
            .iter()
            .position(|s| *s == self.page_size)
            .unwrap_or(0);
        let next = self.page_size_options[(pos + 1) % self.page_size_options.len()];
        self.page_size = next;
        self.page = 1;
        PaginationOutcome::PageSizeChanged {
            page_size: next,
            request: self.request(),
        }
    }

    /// Key adapter.
    pub fn handle_key(&mut self, key: KeyEvent) -> PaginationOutcome {
        if key.is_release() || !self.enabled {
            return PaginationOutcome::Ignored;
        }
        if !self.focused {
            return PaginationOutcome::Ignored;
        }

        // Jump entry mode
        if self.jump_active {
            if !key.is_press() && matches!(key.code, KeyCode::Esc | KeyCode::Enter) {
                return PaginationOutcome::Ignored;
            }
            match key.code {
                KeyCode::Esc => {
                    self.jump_active = false;
                    self.jump_draft.clear();
                    return PaginationOutcome::JumpCancelled;
                }
                KeyCode::Enter => {
                    let parsed = self.jump_draft.parse::<u32>().ok();
                    self.jump_active = false;
                    self.jump_draft.clear();
                    if let Some(p) = parsed {
                        return self.emit_page(p);
                    }
                    return PaginationOutcome::Changed;
                }
                KeyCode::Backspace => {
                    self.jump_draft.pop();
                    return PaginationOutcome::Changed;
                }
                KeyCode::Char(c) if c.is_ascii_digit() => {
                    if self.jump_draft.len() < 8 {
                        self.jump_draft.push(c);
                    }
                    return PaginationOutcome::Changed;
                }
                _ => return PaginationOutcome::Ignored,
            }
        }

        let one_shot = match key.code {
            KeyCode::Char('g' | 'G' | '/' | 's' | 'S' | ' ') | KeyCode::Enter => {
                key.modifiers.is_empty()
            }
            KeyCode::Tab => !key.modifiers.contains(KeyModifiers::SHIFT),
            KeyCode::BackTab => true,
            KeyCode::Char(c) if c.is_ascii_digit() => true,
            _ => false,
        };
        if !key.is_press() && one_shot {
            return PaginationOutcome::Ignored;
        }

        match key.code {
            KeyCode::Left | KeyCode::Char('h') | KeyCode::Char('p') if key.modifiers.is_empty() => {
                // move part or prev
                if matches!(self.part, PaginationPart::Prev | PaginationPart::First) {
                    self.prev()
                } else {
                    self.part = self.prev_part();
                    PaginationOutcome::Changed
                }
            }
            KeyCode::Right | KeyCode::Char('l') | KeyCode::Char('n')
                if key.modifiers.is_empty() =>
            {
                if matches!(self.part, PaginationPart::Next | PaginationPart::Last) {
                    self.next()
                } else {
                    self.part = self.next_part();
                    PaginationOutcome::Changed
                }
            }
            KeyCode::Home => self.first(),
            KeyCode::End => self.last(),
            KeyCode::Char('[') if key.modifiers.is_empty() => self.prev(),
            KeyCode::Char(']') if key.modifiers.is_empty() => self.next(),
            KeyCode::Char('<') if key.modifiers.contains(KeyModifiers::SHIFT) => self.first(),
            KeyCode::Char('>') if key.modifiers.contains(KeyModifiers::SHIFT) => self.last(),
            KeyCode::Char('g') | KeyCode::Char('G') | KeyCode::Char('/')
                if key.modifiers.is_empty() =>
            {
                self.jump_active = true;
                self.jump_draft.clear();
                self.part = PaginationPart::Jump;
                PaginationOutcome::JumpStarted
            }
            KeyCode::Char('s') | KeyCode::Char('S') if key.modifiers.is_empty() => {
                self.part = PaginationPart::PageSize;
                self.cycle_page_size()
            }
            KeyCode::Enter | KeyCode::Char(' ') if key.modifiers.is_empty() => self.activate_part(),
            KeyCode::Tab if !key.modifiers.contains(KeyModifiers::SHIFT) => {
                self.part = self.next_part();
                PaginationOutcome::Changed
            }
            KeyCode::BackTab => {
                self.part = self.prev_part();
                PaginationOutcome::Changed
            }
            KeyCode::Char(c) if c.is_ascii_digit() => {
                self.jump_active = true;
                self.jump_draft.clear();
                self.jump_draft.push(c);
                self.part = PaginationPart::Jump;
                PaginationOutcome::JumpStarted
            }
            KeyCode::Esc => PaginationOutcome::Ignored,
            _ => PaginationOutcome::Ignored,
        }
    }

    fn activate_part(&mut self) -> PaginationOutcome {
        match self.part {
            PaginationPart::First => self.first(),
            PaginationPart::Prev => self.prev(),
            PaginationPart::Next => self.next(),
            PaginationPart::Last => self.last(),
            PaginationPart::PageSize => self.cycle_page_size(),
            PaginationPart::Jump => {
                self.jump_active = true;
                self.jump_draft.clear();
                PaginationOutcome::JumpStarted
            }
            PaginationPart::PageButton(i) => {
                if let Some(p) = self.visible_pages.get(usize::from(i)).copied() {
                    self.emit_page(p)
                } else {
                    PaginationOutcome::Ignored
                }
            }
        }
    }

    fn next_part(&self) -> PaginationPart {
        match self.presentation {
            PaginationPresentation::Minimal => match self.part {
                PaginationPart::Prev => PaginationPart::Next,
                _ => PaginationPart::Prev,
            },
            PaginationPresentation::Compact => match self.part {
                PaginationPart::Prev => PaginationPart::Next,
                PaginationPart::Next => PaginationPart::Jump,
                PaginationPart::Jump => PaginationPart::PageSize,
                _ => PaginationPart::Prev,
            },
            PaginationPresentation::Full => match self.part {
                PaginationPart::First => PaginationPart::Prev,
                PaginationPart::Prev => {
                    if self.visible_pages.is_empty() {
                        PaginationPart::Next
                    } else {
                        PaginationPart::PageButton(0)
                    }
                }
                PaginationPart::PageButton(i) => {
                    let next = usize::from(i) + 1;
                    if next < self.visible_pages.len() {
                        PaginationPart::PageButton(i.saturating_add(1))
                    } else {
                        PaginationPart::Next
                    }
                }
                PaginationPart::Next => PaginationPart::Last,
                PaginationPart::Last => PaginationPart::PageSize,
                PaginationPart::PageSize => PaginationPart::Jump,
                PaginationPart::Jump => PaginationPart::First,
            },
        }
    }

    fn prev_part(&self) -> PaginationPart {
        match self.presentation {
            PaginationPresentation::Minimal => match self.part {
                PaginationPart::Next => PaginationPart::Prev,
                _ => PaginationPart::Next,
            },
            PaginationPresentation::Compact => match self.part {
                PaginationPart::PageSize => PaginationPart::Jump,
                PaginationPart::Jump => PaginationPart::Next,
                PaginationPart::Next => PaginationPart::Prev,
                _ => PaginationPart::PageSize,
            },
            PaginationPresentation::Full => match self.part {
                PaginationPart::Jump => PaginationPart::PageSize,
                PaginationPart::PageSize => PaginationPart::Last,
                PaginationPart::Last => PaginationPart::Next,
                PaginationPart::Next => {
                    if self.visible_pages.is_empty() {
                        PaginationPart::Prev
                    } else {
                        PaginationPart::PageButton(
                            (self.visible_pages.len().saturating_sub(1)) as u8,
                        )
                    }
                }
                PaginationPart::PageButton(0) => PaginationPart::Prev,
                PaginationPart::PageButton(i) => PaginationPart::PageButton(i.saturating_sub(1)),
                PaginationPart::Prev => PaginationPart::First,
                PaginationPart::First => PaginationPart::Jump,
            },
        }
    }

    /// Intent.
    pub fn handle_intent(&mut self, intent: UiIntent) -> PaginationOutcome {
        if !self.enabled || !self.focused {
            return PaginationOutcome::Ignored;
        }
        match intent {
            UiIntent::Activate | UiIntent::Submit => self.activate_part(),
            UiIntent::Page(p) => {
                use crate::interaction::PageMove;
                match p {
                    PageMove::Forward => self.next(),
                    PageMove::Backward => self.prev(),
                }
            }
            UiIntent::Move(m) => {
                use crate::interaction::NavigationMove;
                match m {
                    NavigationMove::Next => {
                        self.part = self.next_part();
                        PaginationOutcome::Changed
                    }
                    NavigationMove::Previous => {
                        self.part = self.prev_part();
                        PaginationOutcome::Changed
                    }
                    NavigationMove::First => self.first(),
                    NavigationMove::Last => self.last(),
                    _ => PaginationOutcome::Ignored,
                }
            }
            UiIntent::Cancel | UiIntent::Close => {
                if self.jump_active {
                    self.jump_active = false;
                    self.jump_draft.clear();
                    PaginationOutcome::JumpCancelled
                } else {
                    PaginationOutcome::Ignored
                }
            }
            _ => PaginationOutcome::Ignored,
        }
    }

    /// Mouse using last paint hits.
    pub fn handle_mouse(&mut self, event: MouseEvent) -> PaginationOutcome {
        if !self.enabled {
            return PaginationOutcome::Ignored;
        }
        if !matches!(event.kind, MouseEventKind::Down(MouseButton::Left)) {
            return PaginationOutcome::Ignored;
        }
        self.focused = true;
        for (part, rect) in &self.hits {
            if rect.contains(event.position) {
                self.part = *part;
                return self.activate_part();
            }
        }
        PaginationOutcome::Ignored
    }

    /// Build page number window around current (for Full presentation).
    fn compute_visible_pages(&self, max_buttons: usize) -> Vec<u32> {
        let Some(total_pages) = self.page_count() else {
            // unknown: show current only
            return vec![self.page];
        };
        if total_pages <= max_buttons as u32 {
            return (1..=total_pages).collect();
        }
        let max_buttons = max_buttons.max(3);
        let half = max_buttons / 2;
        let mut start = self.page.saturating_sub(half as u32).max(1);
        let mut end = start.saturating_add(max_buttons as u32).saturating_sub(1);
        if end > total_pages {
            end = total_pages;
            start = end
                .saturating_sub(max_buttons as u32)
                .saturating_add(1)
                .max(1);
        }
        (start..=end).collect()
    }
}

// ── Widget ──────────────────────────────────────────────────────────────────

/// Pagination chrome.
#[derive(Debug, Clone, Copy)]
pub struct Pagination<'a> {
    system: &'a DesignSystem,
    show_summary: bool,
    show_page_size: bool,
}

impl<'a> Pagination<'a> {
    /// Create.
    #[must_use]
    pub const fn new(system: &'a DesignSystem) -> Self {
        Self {
            system,
            show_summary: true,
            show_page_size: true,
        }
    }

    /// ASCII glyphs.
    #[must_use]
    /// Show item summary.
    pub const fn show_summary(mut self, on: bool) -> Self {
        self.show_summary = on;
        self
    }

    /// Show page-size control.
    #[must_use]
    pub const fn show_page_size(mut self, on: bool) -> Self {
        self.show_page_size = on;
        self
    }

    /// Paint.
    pub fn paint(&self, area: Rect, buffer: &mut Buffer, state: &mut PaginationState) {
        state.hits.clear();
        state.root = area;
        if area.is_empty() {
            return;
        }

        let pres = PaginationState::presentation_for_width(area.width);
        if pres != state.presentation {
            state.presentation = pres;
        }

        let max_buttons = match state.presentation {
            PaginationPresentation::Full => 7,
            PaginationPresentation::Compact => 1,
            PaginationPresentation::Minimal => 0,
        };
        state.visible_pages = state.compute_visible_pages(max_buttons);

        let first = { "«" };
        let prev = { "‹" };
        let next = { "›" };
        let last = { "»" };

        let mut x = area.x;
        let y = area.y;

        let put = |buffer: &mut Buffer,
                   x: &mut u16,
                   y: u16,
                   right: u16,
                   text: &str,
                   part: PaginationPart,
                   enabled: bool,
                   active: bool,
                   focused_part: bool,
                   system: &DesignSystem,
                   hits: &mut Vec<(PaginationPart, Rect)>| {
            let w = display_cols(text) as u16;
            if *x >= right || w == 0 {
                return;
            }
            let w = w.min(right.saturating_sub(*x));
            let rect = Rect::new(*x, y, w, 1);
            let style = if !enabled {
                system.style(Role::TextDisabled)
            } else if focused_part {
                // Focus is the accent tone and weight, never a reversal.
                system.style(Role::Focus).add_modifier(Modifier::BOLD)
            } else if active {
                // The current page is the bold one.
                system.style(Role::TextStrong).add_modifier(Modifier::BOLD)
            } else {
                system.style(Role::Text)
            };
            buffer.set_stringn(rect.x, rect.y, text, usize::from(rect.width), style);
            if enabled {
                hits.push((part, rect));
            }
            *x = (*x).saturating_add(w).saturating_add(1);
        };

        let loading = if state.loading { " …" } else { "" };

        match state.presentation {
            PaginationPresentation::Minimal => {
                put(
                    buffer,
                    &mut x,
                    y,
                    area.right(),
                    prev,
                    PaginationPart::Prev,
                    state.can_prev(),
                    false,
                    state.focused && matches!(state.part, PaginationPart::Prev),
                    self.system,
                    &mut state.hits,
                );
                let lab = format!("{}{loading}", state.page_label());
                put(
                    buffer,
                    &mut x,
                    y,
                    area.right(),
                    &lab,
                    PaginationPart::Jump,
                    state.enabled && !state.loading,
                    true,
                    state.focused && matches!(state.part, PaginationPart::Jump),
                    self.system,
                    &mut state.hits,
                );
                put(
                    buffer,
                    &mut x,
                    y,
                    area.right(),
                    next,
                    PaginationPart::Next,
                    state.can_next(),
                    false,
                    state.focused && matches!(state.part, PaginationPart::Next),
                    self.system,
                    &mut state.hits,
                );
            }
            PaginationPresentation::Compact => {
                put(
                    buffer,
                    &mut x,
                    y,
                    area.right(),
                    prev,
                    PaginationPart::Prev,
                    state.can_prev(),
                    false,
                    state.focused && matches!(state.part, PaginationPart::Prev),
                    self.system,
                    &mut state.hits,
                );
                let lab = if state.jump_active {
                    format!("[{}]{loading}", state.jump_draft)
                } else {
                    format!("Page {}{loading}", state.page_label())
                };
                put(
                    buffer,
                    &mut x,
                    y,
                    area.right(),
                    &lab,
                    PaginationPart::Jump,
                    state.enabled && !state.loading,
                    true,
                    state.focused && matches!(state.part, PaginationPart::Jump),
                    self.system,
                    &mut state.hits,
                );
                put(
                    buffer,
                    &mut x,
                    y,
                    area.right(),
                    next,
                    PaginationPart::Next,
                    state.can_next(),
                    false,
                    state.focused && matches!(state.part, PaginationPart::Next),
                    self.system,
                    &mut state.hits,
                );
                if self.show_page_size {
                    let sz = format!("×{}", state.page_size);
                    put(
                        buffer,
                        &mut x,
                        y,
                        area.right(),
                        &sz,
                        PaginationPart::PageSize,
                        state.enabled && !state.loading,
                        false,
                        state.focused && matches!(state.part, PaginationPart::PageSize),
                        self.system,
                        &mut state.hits,
                    );
                }
            }
            PaginationPresentation::Full => {
                put(
                    buffer,
                    &mut x,
                    y,
                    area.right(),
                    first,
                    PaginationPart::First,
                    state.can_first(),
                    false,
                    state.focused && matches!(state.part, PaginationPart::First),
                    self.system,
                    &mut state.hits,
                );
                put(
                    buffer,
                    &mut x,
                    y,
                    area.right(),
                    prev,
                    PaginationPart::Prev,
                    state.can_prev(),
                    false,
                    state.focused && matches!(state.part, PaginationPart::Prev),
                    self.system,
                    &mut state.hits,
                );
                for (i, p) in state.visible_pages.iter().copied().enumerate() {
                    let label = format!("{p}");
                    let active = p == state.page;
                    put(
                        buffer,
                        &mut x,
                        y,
                        area.right(),
                        &label,
                        PaginationPart::PageButton(i as u8),
                        state.enabled && !state.loading,
                        active,
                        state.focused
                            && matches!(state.part, PaginationPart::PageButton(j) if j as usize == i),
                        self.system,
                        &mut state.hits,
                    );
                }
                put(
                    buffer,
                    &mut x,
                    y,
                    area.right(),
                    next,
                    PaginationPart::Next,
                    state.can_next(),
                    false,
                    state.focused && matches!(state.part, PaginationPart::Next),
                    self.system,
                    &mut state.hits,
                );
                put(
                    buffer,
                    &mut x,
                    y,
                    area.right(),
                    last,
                    PaginationPart::Last,
                    state.can_last(),
                    false,
                    state.focused && matches!(state.part, PaginationPart::Last),
                    self.system,
                    &mut state.hits,
                );
                if self.show_page_size {
                    let sz = format!("×{}", state.page_size);
                    put(
                        buffer,
                        &mut x,
                        y,
                        area.right(),
                        &sz,
                        PaginationPart::PageSize,
                        state.enabled && !state.loading,
                        false,
                        state.focused && matches!(state.part, PaginationPart::PageSize),
                        self.system,
                        &mut state.hits,
                    );
                }
                if self.show_summary {
                    let sum = if state.jump_active {
                        format!(" go:{}{loading}", state.jump_draft)
                    } else {
                        format!(" {}{loading}", state.summary())
                    };
                    let w = display_cols(&sum) as u16;
                    let avail = area.right().saturating_sub(x);
                    if avail > 0 {
                        let w = w.min(avail);
                        let style = if state.focused && matches!(state.part, PaginationPart::Jump) {
                            self.system.style(Role::Focus).add_modifier(Modifier::BOLD)
                        } else {
                            self.system.style(Role::TextMuted)
                        };
                        buffer.set_stringn(
                            x,
                            y,
                            take_display_cols(&sum, usize::from(w)),
                            usize::from(w),
                            style,
                        );
                        let rect = Rect::new(x, y, w, 1);
                        state.hits.push((PaginationPart::Jump, rect));
                    }
                }
            }
        }
    }

    /// Semantic: one control.
    pub fn register_semantic<Id, Action>(
        &self,
        scene: &mut SemanticScene<Id, Action>,
        id: Id,
        area: Rect,
        state: &PaginationState,
    ) where
        Id: Clone + PartialEq + std::fmt::Display,
        Action: Clone,
    {
        if area.is_empty() {
            return;
        }
        let desc = format!(
            "pagination page={} size={} total={} loading={}",
            state.page(),
            state.page_size(),
            state.total().id(),
            state.is_loading()
        );
        let _ = scene.register(
            SemanticNode::control(id, area)
                .role(SemanticRole::Control)
                .label("pagination")
                .description(desc)
                .focusable(state.enabled)
                .disabled(!state.enabled)
                .state(SemanticState {
                    selected: state.focused,
                    busy: state.loading,
                    invalid: false,
                    expanded: state.jump_active,
                    ..Default::default()
                }),
        );
    }
}

impl StatefulWidget for &Pagination<'_> {
    type State = PaginationState;

    fn render(self, area: Rect, buffer: &mut Buffer, state: &mut Self::State) {
        self.paint(area, buffer, state);
    }
}

impl StatefulWidget for Pagination<'_> {
    type State = PaginationState;

    fn render(self, area: Rect, buffer: &mut Buffer, state: &mut Self::State) {
        <&Self as StatefulWidget>::render(&self, area, buffer, state);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::input::KeyEventKind;
    use crate::style::RolePalette;
    use crate::widgets::tests::{click, key_with_kind};

    #[test]
    fn page_request_offset() {
        let r = PageRequest::new(3, 25);
        assert_eq!(r.offset(), 50);
        assert_eq!(r.limit(), 25);
        assert_eq!(PageRequest::new(0, 0).page, 1);
    }

    #[test]
    fn next_prev_known_total() {
        let mut s = PaginationState::new(1, 10, PageTotal::Known(35));
        s.set_focused(true);
        assert_eq!(s.page_count(), Some(4));
        assert!(matches!(
            s.next(),
            PaginationOutcome::PageRequested {
                request: PageRequest {
                    page: 2,
                    page_size: 10
                }
            }
        ));
        assert!(
            matches!(s.prev(), PaginationOutcome::PageRequested { request } if request.page == 1)
        );
        assert!(matches!(s.first(), PaginationOutcome::Ignored)); // already 1 after prev
        s.set_page(2);
        assert!(matches!(
            s.last(),
            PaginationOutcome::PageRequested {
                request: PageRequest { page: 4, .. }
            }
        ));
    }

    #[test]
    fn unknown_total_allows_next() {
        let mut s = PaginationState::new(5, 20, PageTotal::Unknown);
        s.set_focused(true);
        assert!(s.can_next());
        assert!(matches!(
            s.next(),
            PaginationOutcome::PageRequested {
                request: PageRequest { page: 6, .. }
            }
        ));
        assert!(!s.can_last());
    }

    #[test]
    fn loading_disables_nav() {
        let mut s = PaginationState::new(2, 10, PageTotal::Known(100));
        s.set_focused(true);
        s.set_loading(true);
        assert!(!s.can_next());
        assert!(matches!(s.next(), PaginationOutcome::Ignored));
    }

    #[test]
    fn page_size_cycle() {
        let mut s =
            PaginationState::new(3, 25, PageTotal::Known(200)).with_page_sizes([10, 25, 50]);
        s.set_focused(true);
        assert!(matches!(
            s.cycle_page_size(),
            PaginationOutcome::PageSizeChanged {
                page_size: 50,
                request: PageRequest {
                    page: 1,
                    page_size: 50
                }
            }
        ));
    }

    #[test]
    fn jump_entry() {
        let mut s = PaginationState::new(1, 10, PageTotal::Known(100));
        s.set_focused(true);
        assert!(matches!(
            s.handle_key(KeyEvent::new(KeyCode::Char('g'), KeyModifiers::NONE)),
            PaginationOutcome::JumpStarted
        ));
        let _ = s.handle_key(KeyEvent::new(KeyCode::Char('3'), KeyModifiers::NONE));
        assert!(matches!(
            s.handle_key(KeyEvent::new(KeyCode::Enter, KeyModifiers::NONE)),
            PaginationOutcome::PageRequested {
                request: PageRequest { page: 3, .. }
            }
        ));
    }

    #[test]
    fn escape_closes_only_the_jump_entry_layer() {
        let mut state = PaginationState::new(4, 10, PageTotal::Known(100));
        state.set_focused(true);
        assert_eq!(
            state.handle_key(KeyEvent::new(KeyCode::Char('g'), KeyModifiers::NONE)),
            PaginationOutcome::JumpStarted
        );
        let _ = state.handle_key(KeyEvent::new(KeyCode::Char('8'), KeyModifiers::NONE));
        assert_eq!(
            state.handle_key(KeyEvent::new(KeyCode::Esc, KeyModifiers::NONE)),
            PaginationOutcome::JumpCancelled
        );
        assert_eq!(state.page(), 4);
    }

    #[test]
    fn repeated_pagination_lifecycle_actions_are_ignored() {
        let actions = [
            (KeyCode::Char('g'), KeyModifiers::NONE),
            (KeyCode::Char('/'), KeyModifiers::NONE),
            (KeyCode::Char('s'), KeyModifiers::NONE),
            (KeyCode::Enter, KeyModifiers::NONE),
            (KeyCode::Char(' '), KeyModifiers::NONE),
            (KeyCode::Tab, KeyModifiers::NONE),
            (KeyCode::BackTab, KeyModifiers::NONE),
            (KeyCode::Char('3'), KeyModifiers::NONE),
        ];
        for (code, modifiers) in actions {
            let mut state = PaginationState::new(2, 10, PageTotal::Known(100));
            state.set_focused(true);
            let before = state.clone();
            assert_eq!(
                state.handle_key(key_with_kind(code, modifiers, KeyEventKind::Repeat)),
                PaginationOutcome::Ignored,
                "repeat of {code:?} must not fire a pagination lifecycle action"
            );
            assert_eq!(state, before);
        }

        let mut jump = PaginationState::new(2, 10, PageTotal::Known(100));
        jump.set_focused(true);
        assert_eq!(
            jump.handle_key(KeyEvent::new(KeyCode::Char('g'), KeyModifiers::NONE)),
            PaginationOutcome::JumpStarted
        );
        let _ = jump.handle_key(KeyEvent::new(KeyCode::Char('3'), KeyModifiers::NONE));
        let before = jump.clone();
        for code in [KeyCode::Esc, KeyCode::Enter] {
            assert_eq!(
                jump.handle_key(key_with_kind(
                    code,
                    KeyModifiers::NONE,
                    KeyEventKind::Repeat
                )),
                PaginationOutcome::Ignored
            );
            assert_eq!(jump, before);
        }
    }

    #[test]
    fn disabled_state_rejects_keyboard_mouse_and_registers_semantics() {
        let system = DesignSystem::default();
        let area = Rect::new(0, 0, 72, 1);
        let mut buffer = Buffer::empty(area);
        let mut state = PaginationState::new(3, 10, PageTotal::Known(100));
        state.set_focused(true);
        Pagination::new(&system).paint(area, &mut buffer, &mut state);
        let next = state
            .hits
            .iter()
            .find(|(part, _)| matches!(part, PaginationPart::Next))
            .map(|(_, region)| *region)
            .expect("painted next region");

        state.set_enabled(false);
        assert_eq!(
            state.handle_key(KeyEvent::new(KeyCode::Right, KeyModifiers::NONE)),
            PaginationOutcome::Ignored
        );
        assert_eq!(
            state.handle_mouse(click(next.x, next.y)),
            PaginationOutcome::Ignored
        );

        let mut scene = SemanticScene::<&str, ()>::default();
        Pagination::new(&system).register_semantic(&mut scene, "pagination", area, &state);
        let node = scene.nodes().first().expect("pagination semantic node");
        assert!(node.disabled);
        assert!(!node.focusable);
    }

    #[test]
    fn keys_bracket_nav() {
        let mut s = PaginationState::new(2, 10, PageTotal::Known(50));
        s.set_focused(true);
        assert!(matches!(
            s.handle_key(KeyEvent::new(KeyCode::Char('['), KeyModifiers::NONE)),
            PaginationOutcome::PageRequested {
                request: PageRequest { page: 1, .. }
            }
        ));
    }

    #[test]
    fn summary_formats() {
        let s = PaginationState::new(2, 10, PageTotal::Known(35));
        assert!(s.summary().contains("of 35"));
        let u = PaginationState::new(3, 10, PageTotal::Unknown);
        assert_eq!(u.page_label(), "3/?");
        let a = PaginationState::new(1, 10, PageTotal::AtLeast(100));
        assert!(a.summary().contains('+'));
    }

    #[test]
    fn presentation_by_width() {
        assert_eq!(
            PaginationState::presentation_for_width(20),
            PaginationPresentation::Minimal
        );
        assert_eq!(
            PaginationState::presentation_for_width(36),
            PaginationPresentation::Compact
        );
        assert_eq!(
            PaginationState::presentation_for_width(80),
            PaginationPresentation::Full
        );
    }

    #[test]
    fn paint_full_and_mouse() {
        let system = DesignSystem::new(RolePalette::default());
        let mut state = PaginationState::new(3, 10, PageTotal::Known(100));
        state.set_focused(true);
        let area = Rect::new(0, 0, 72, 1);
        let mut buf = Buffer::empty(area);
        Pagination::new(&system).paint(area, &mut buf, &mut state);
        assert!(!state.hits.is_empty());
        // click next if present
        if let Some((_, rect)) = state
            .hits
            .iter()
            .find(|(p, _)| matches!(p, PaginationPart::Next))
        {
            assert!(matches!(
                state.handle_mouse(click(rect.x, rect.y)),
                PaginationOutcome::PageRequested {
                    request: PageRequest { page: 4, .. }
                }
            ));
        }
    }

    #[test]
    fn paint_minimal_loading() {
        let system = DesignSystem::default();
        let mut state = PaginationState::new(1, 25, PageTotal::Unknown);
        state.set_loading(true);
        state.set_focused(true);
        let area = Rect::new(0, 0, 18, 1);
        let mut buf = Buffer::empty(area);
        Pagination::new(&system).paint(area, &mut buf, &mut state);
        assert_eq!(state.presentation(), PaginationPresentation::Minimal);
    }

    #[test]
    fn fuzz_keys() {
        let mut s = PaginationState::new(2, 10, PageTotal::Known(80));
        s.set_focused(true);
        let keys = [
            KeyEvent::new(KeyCode::Left, KeyModifiers::NONE),
            KeyEvent::new(KeyCode::Right, KeyModifiers::NONE),
            KeyEvent::new(KeyCode::Enter, KeyModifiers::NONE),
            KeyEvent::new(KeyCode::Home, KeyModifiers::NONE),
            KeyEvent::new(KeyCode::End, KeyModifiers::NONE),
            KeyEvent::new(KeyCode::Char('s'), KeyModifiers::NONE),
            KeyEvent::new(KeyCode::Char('5'), KeyModifiers::NONE),
            KeyEvent::new(KeyCode::Esc, KeyModifiers::NONE),
        ];
        for key in keys.iter().cycle().take(40) {
            let _ = s.handle_key(*key);
            s.set_focused(true);
        }
    }

    #[test]
    fn paint_hot_path() {
        let system = DesignSystem::default();
        let mut state = PaginationState::new(5, 25, PageTotal::Known(1000));
        state.set_focused(true);
        let area = Rect::new(0, 0, 64, 1);
        let mut buf = Buffer::empty(area);
        let w = Pagination::new(&system);
        for _ in 0..50 {
            w.paint(area, &mut buf, &mut state);
        }
    }

    #[test]
    fn semantic() {
        let system = DesignSystem::default();
        let state = PaginationState::new(1, 10, PageTotal::Known(50));
        let mut scene = SemanticScene::<&str, ()>::default();
        Pagination::new(&system).register_semantic(
            &mut scene,
            "pg",
            Rect::new(0, 0, 40, 1),
            &state,
        );
        assert!(scene.get(&"pg").is_some());
    }

    #[test]
    fn guidance_mentions_virtualizer() {
        assert!(guidance::WHEN_VIRTUALIZE.contains("Virtualizer"));
    }
}
