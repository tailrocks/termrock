// SPDX-FileCopyrightText: 2026 Alexey Zhokhov
// SPDX-License-Identifier: Apache-2.0

//! Agent-era experience widgets: stream, tool cards, approvals, prompt, meters.

use ratatui_core::{
    buffer::Buffer,
    layout::Rect,
    widgets::{StatefulWidget, Widget},
};

use crate::{
    input::{
        KeyCode, KeyEvent, KeyEventKind, KeyModifiers, MouseButton, MouseEvent, MouseEventKind,
    },
    style::{Role, Theme},
    text::{display_cols, take_display_cols},
    widgets::{Panel, PanelEmphasis, TextArea, TextAreaOutcome, TextAreaState},
};

// ── Token meter ─────────────────────────────────────────────────────────────

/// Compact token/cost usage meter.
#[derive(Debug, Clone, Copy)]
pub struct TokenMeter<'a> {
    used: u64,
    limit: u64,
    label: &'a str,
    theme: &'a Theme,
}

impl<'a> TokenMeter<'a> {
    /// Creates a token meter.
    #[must_use]
    pub const fn new(used: u64, limit: u64, theme: &'a Theme) -> Self {
        Self {
            used,
            limit,
            label: "tokens",
            theme,
        }
    }

    /// Overrides the unit label.
    #[must_use]
    pub const fn label(mut self, label: &'a str) -> Self {
        self.label = label;
        self
    }
}

impl Widget for &TokenMeter<'_> {
    fn render(self, area: Rect, buffer: &mut Buffer) {
        if area.is_empty() {
            return;
        }
        let fraction = if self.limit == 0 {
            0.0
        } else {
            (self.used as f64 / self.limit as f64).clamp(0.0, 1.0)
        };
        let text = format!(
            "{} {}/{} ({:.0}%)",
            self.label,
            self.used,
            self.limit,
            fraction * 100.0
        );
        let role = if fraction >= 0.9 {
            Role::Danger
        } else if fraction >= 0.75 {
            Role::Warning
        } else {
            Role::TextMuted
        };
        let clipped = take_display_cols(&text, usize::from(area.width));
        buffer.set_stringn(
            area.x,
            area.y,
            &clipped,
            usize::from(area.width),
            self.theme.style(role),
        );
    }
}

impl Widget for TokenMeter<'_> {
    #[expect(
        clippy::needless_borrows_for_generic_args,
        reason = "explicitly delegate the owned contract to the borrowed renderer"
    )]
    fn render(self, area: Rect, buffer: &mut Buffer) {
        <&Self as Widget>::render(&self, area, buffer);
    }
}

// ── Thinking block ──────────────────────────────────────────────────────────

/// Collapsible thinking/reasoning chrome.
#[derive(Debug, Clone, Copy)]
pub struct ThinkingBlock<'a> {
    summary: &'a str,
    expanded: bool,
    body: &'a str,
    frame: &'a str,
    theme: &'a Theme,
}

impl<'a> ThinkingBlock<'a> {
    /// Creates a thinking block.
    #[must_use]
    pub const fn new(summary: &'a str, theme: &'a Theme) -> Self {
        Self {
            summary,
            expanded: false,
            body: "",
            frame: "·",
            theme,
        }
    }

    /// Expands body text.
    #[must_use]
    pub const fn expanded(mut self, expanded: bool) -> Self {
        self.expanded = expanded;
        self
    }

    /// Sets body text shown when expanded.
    #[must_use]
    pub const fn body(mut self, body: &'a str) -> Self {
        self.body = body;
        self
    }

    /// Spinner/status frame while thinking.
    #[must_use]
    pub const fn frame(mut self, frame: &'a str) -> Self {
        self.frame = frame;
        self
    }
}

impl Widget for &ThinkingBlock<'_> {
    fn render(self, area: Rect, buffer: &mut Buffer) {
        if area.is_empty() {
            return;
        }
        let marker = if self.expanded { "▾" } else { "▸" };
        let header = format!("{} {} {}", marker, self.frame, self.summary);
        let clipped = take_display_cols(&header, usize::from(area.width));
        buffer.set_stringn(
            area.x,
            area.y,
            &clipped,
            usize::from(area.width),
            self.theme.style(Role::TextMuted),
        );
        if self.expanded && area.height > 1 && !self.body.is_empty() {
            let body = take_display_cols(self.body, usize::from(area.width));
            buffer.set_stringn(
                area.x,
                area.y.saturating_add(1),
                &body,
                usize::from(area.width),
                self.theme.style(Role::TextDisabled),
            );
        }
    }
}

impl Widget for ThinkingBlock<'_> {
    #[expect(
        clippy::needless_borrows_for_generic_args,
        reason = "explicitly delegate the owned contract to the borrowed renderer"
    )]
    fn render(self, area: Rect, buffer: &mut Buffer) {
        <&Self as Widget>::render(&self, area, buffer);
    }
}

// ── Tool card ───────────────────────────────────────────────────────────────

/// Lifecycle status for a tool invocation card.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[non_exhaustive]
pub enum ToolStatus {
    /// Queued, not started.
    Pending,
    /// Currently executing.
    Running,
    /// Completed successfully.
    Done,
    /// Failed.
    Error,
    /// Cancelled by user or policy.
    Cancelled,
}

impl ToolStatus {
    /// Non-color status glyph.
    #[must_use]
    pub const fn glyph(self) -> &'static str {
        match self {
            Self::Pending => "…",
            Self::Running => "◉",
            Self::Done => "✓",
            Self::Error => "✗",
            Self::Cancelled => "⊘",
        }
    }

    /// Theme role for the status.
    #[must_use]
    pub const fn role(self) -> Role {
        match self {
            Self::Pending => Role::TextMuted,
            Self::Running => Role::Info,
            Self::Done => Role::Success,
            Self::Error => Role::Danger,
            Self::Cancelled => Role::TextDisabled,
        }
    }
}

/// Mutable streaming tool call card.
#[derive(Debug, Clone, Copy)]
pub struct ToolCard<'a> {
    name: &'a str,
    summary: &'a str,
    status: ToolStatus,
    detail: Option<&'a str>,
    expanded: bool,
    theme: &'a Theme,
}

impl<'a> ToolCard<'a> {
    /// Creates a tool card.
    #[must_use]
    pub const fn new(
        name: &'a str,
        summary: &'a str,
        status: ToolStatus,
        theme: &'a Theme,
    ) -> Self {
        Self {
            name,
            summary,
            status,
            detail: None,
            expanded: false,
            theme,
        }
    }

    /// Optional detail / stdout slice.
    #[must_use]
    pub const fn detail(mut self, detail: &'a str) -> Self {
        self.detail = Some(detail);
        self
    }

    /// Whether detail is shown.
    #[must_use]
    pub const fn expanded(mut self, expanded: bool) -> Self {
        self.expanded = expanded;
        self
    }
}

impl Widget for &ToolCard<'_> {
    fn render(self, area: Rect, buffer: &mut Buffer) {
        if area.is_empty() {
            return;
        }
        let panel = Panel::new(self.theme).emphasis(match self.status {
            ToolStatus::Error => PanelEmphasis::Danger,
            ToolStatus::Running => PanelEmphasis::Focused,
            _ => PanelEmphasis::Normal,
        });
        let inner = panel.inner(area);
        Widget::render(&panel, area, buffer);
        if inner.is_empty() {
            return;
        }
        let header = format!("{} {} — {}", self.status.glyph(), self.name, self.summary);
        let clipped = take_display_cols(&header, usize::from(inner.width));
        buffer.set_stringn(
            inner.x,
            inner.y,
            &clipped,
            usize::from(inner.width),
            self.theme.style(self.status.role()),
        );
        if self.expanded
            && let Some(detail) = self.detail
            && inner.height > 1
        {
            let body = take_display_cols(detail, usize::from(inner.width));
            buffer.set_stringn(
                inner.x,
                inner.y.saturating_add(1),
                &body,
                usize::from(inner.width),
                self.theme.style(Role::TextMuted),
            );
        }
    }
}

impl Widget for ToolCard<'_> {
    #[expect(
        clippy::needless_borrows_for_generic_args,
        reason = "explicitly delegate the owned contract to the borrowed renderer"
    )]
    fn render(self, area: Rect, buffer: &mut Buffer) {
        <&Self as Widget>::render(&self, area, buffer);
    }
}

// ── Approval card ───────────────────────────────────────────────────────────

/// Semantic permission decision (message only — never executes policy).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[non_exhaustive]
pub enum ApprovalDecision {
    /// Allow once.
    AllowOnce,
    /// Allow for this session.
    AllowSession,
    /// Always allow this class of action.
    Always,
    /// Deny.
    Deny,
    /// Defer / ask later.
    Defer,
}

impl ApprovalDecision {
    /// Canonical navigation/render order for every decision.
    pub const ALL: [Self; 5] = [
        Self::AllowOnce,
        Self::AllowSession,
        Self::Always,
        Self::Deny,
        Self::Defer,
    ];

    /// Short chrome label.
    #[must_use]
    pub const fn label(self) -> &'static str {
        match self {
            Self::AllowOnce => "Once",
            Self::AllowSession => "Session",
            Self::Always => "Always",
            Self::Deny => "Deny",
            Self::Defer => "Defer",
        }
    }

    fn index(self) -> usize {
        Self::ALL
            .iter()
            .position(|&decision| decision == self)
            .unwrap_or(3)
    }
}

/// Risk tier for approval chrome.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[non_exhaustive]
pub enum ApprovalRisk {
    /// Low risk informational.
    Low,
    /// Caution.
    Medium,
    /// Destructive / high impact.
    High,
}

impl ApprovalRisk {
    /// Theme role for the risk tier.
    #[must_use]
    pub const fn role(self) -> Role {
        match self {
            Self::Low => Role::Info,
            Self::Medium => Role::Warning,
            Self::High => Role::Danger,
        }
    }

    /// Non-color marker.
    #[must_use]
    pub const fn glyph(self) -> &'static str {
        match self {
            Self::Low => "ℹ",
            Self::Medium => "!",
            Self::High => "⚠",
        }
    }
}

/// Typed result of approval interaction (no side effects).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[non_exhaustive]
pub enum ApprovalCardOutcome {
    /// Input did not apply.
    Ignored,
    /// Selected decision changed.
    SelectionChanged,
    /// User confirmed a decision (consumer applies policy).
    Confirmed(ApprovalDecision),
    /// User cancelled without confirming (Esc). Not a Deny decision.
    Cancelled,
}

/// Painted hit region for one decision control.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ApprovalDecisionRegion {
    /// Decision identity.
    pub decision: ApprovalDecision,
    /// Exact painted rectangle.
    pub area: Rect,
}

/// Fail-safe approval interaction state.
///
/// Default selection is [`ApprovalDecision::Deny`]. Untouched Enter never
/// approves.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ApprovalCardState {
    selected: ApprovalDecision,
    /// Exact decision hit regions from the latest render.
    pub decision_regions: Vec<ApprovalDecisionRegion>,
}

impl Default for ApprovalCardState {
    fn default() -> Self {
        Self::new()
    }
}

impl ApprovalCardState {
    /// Creates state with the safe default selection (`Deny`).
    #[must_use]
    pub fn new() -> Self {
        Self {
            selected: ApprovalDecision::Deny,
            decision_regions: Vec::new(),
        }
    }

    /// Creates state with an explicit initial selection.
    #[must_use]
    pub fn with_selected(selected: ApprovalDecision) -> Self {
        Self {
            selected,
            decision_regions: Vec::new(),
        }
    }

    /// Currently selected decision.
    #[must_use]
    pub const fn selected(&self) -> ApprovalDecision {
        self.selected
    }

    /// Sets the selected decision.
    pub fn set_selected(&mut self, selected: ApprovalDecision) {
        self.selected = selected;
    }

    fn move_selection(&mut self, delta: isize) -> ApprovalCardOutcome {
        let len = ApprovalDecision::ALL.len() as isize;
        let next = (self.selected.index() as isize + delta).rem_euclid(len) as usize;
        let next = ApprovalDecision::ALL[next];
        if next == self.selected {
            return ApprovalCardOutcome::Ignored;
        }
        self.selected = next;
        ApprovalCardOutcome::SelectionChanged
    }

    /// Handles keyboard navigation and confirmation.
    ///
    /// Navigation accepts Press and Repeat. Confirmation and shortcuts are
    /// Press-only so held Enter cannot multi-fire.
    pub fn handle_key(&mut self, key: KeyEvent) -> ApprovalCardOutcome {
        match key.kind {
            KeyEventKind::Press | KeyEventKind::Repeat => {}
            KeyEventKind::Release => return ApprovalCardOutcome::Ignored,
        }
        let is_press = key.kind == KeyEventKind::Press;
        match key.code {
            KeyCode::Left | KeyCode::Up => self.move_selection(-1),
            KeyCode::Right | KeyCode::Down => self.move_selection(1),
            KeyCode::Tab if key.modifiers.contains(KeyModifiers::SHIFT) => self.move_selection(-1),
            KeyCode::Tab => self.move_selection(1),
            KeyCode::BackTab => self.move_selection(-1),
            KeyCode::Enter if is_press => ApprovalCardOutcome::Confirmed(self.selected),
            KeyCode::Enter => ApprovalCardOutcome::Ignored,
            KeyCode::Esc if is_press => ApprovalCardOutcome::Cancelled,
            KeyCode::Esc => ApprovalCardOutcome::Ignored,
            KeyCode::Char('n' | 'N') if is_press => {
                ApprovalCardOutcome::Confirmed(ApprovalDecision::Deny)
            }
            KeyCode::Char('y' | 'Y') if is_press => {
                ApprovalCardOutcome::Confirmed(ApprovalDecision::AllowOnce)
            }
            _ => ApprovalCardOutcome::Ignored,
        }
    }

    /// Handles pointer input against the latest decision hit regions.
    ///
    /// A left-button Down on a decision selects it and confirms once.
    pub fn handle_mouse(&mut self, event: MouseEvent) -> ApprovalCardOutcome {
        match event.kind {
            MouseEventKind::Down(MouseButton::Left) => {
                let Some(region) = self
                    .decision_regions
                    .iter()
                    .find(|region| region.area.contains(event.position))
                    .copied()
                else {
                    return ApprovalCardOutcome::Ignored;
                };
                self.selected = region.decision;
                ApprovalCardOutcome::Confirmed(region.decision)
            }
            MouseEventKind::Moved => {
                let Some(region) = self
                    .decision_regions
                    .iter()
                    .find(|region| region.area.contains(event.position))
                    .copied()
                else {
                    return ApprovalCardOutcome::Ignored;
                };
                if region.decision == self.selected {
                    return ApprovalCardOutcome::Ignored;
                }
                self.selected = region.decision;
                ApprovalCardOutcome::SelectionChanged
            }
            _ => ApprovalCardOutcome::Ignored,
        }
    }
}

/// Blocking permission card.
#[derive(Debug, Clone, Copy)]
pub struct ApprovalCard<'a> {
    title: &'a str,
    detail: &'a str,
    risk: ApprovalRisk,
    theme: &'a Theme,
}

impl<'a> ApprovalCard<'a> {
    /// Creates an approval card.
    #[must_use]
    pub const fn new(
        title: &'a str,
        detail: &'a str,
        risk: ApprovalRisk,
        theme: &'a Theme,
    ) -> Self {
        Self {
            title,
            detail,
            risk,
            theme,
        }
    }
}

fn decision_chip(decision: ApprovalDecision, selected: bool) -> String {
    let label = decision.label();
    if selected {
        format!("[{label}]")
    } else {
        format!(" {label} ")
    }
}

impl StatefulWidget for &ApprovalCard<'_> {
    type State = ApprovalCardState;

    fn render(self, area: Rect, buffer: &mut Buffer, state: &mut Self::State) {
        state.decision_regions.clear();
        if area.is_empty() {
            return;
        }
        let emphasis = match self.risk {
            ApprovalRisk::High => PanelEmphasis::Danger,
            _ => PanelEmphasis::Focused,
        };
        let panel = Panel::new(self.theme).title(self.title).emphasis(emphasis);
        let inner = panel.inner(area);
        Widget::render(&panel, area, buffer);
        if inner.is_empty() {
            return;
        }
        let header = format!("{} {}", self.risk.glyph(), self.detail);
        buffer.set_stringn(
            inner.x,
            inner.y,
            take_display_cols(&header, usize::from(inner.width)),
            usize::from(inner.width),
            self.theme.style(self.risk.role()),
        );

        // Tiny height: selected decision + non-color nav cue on one row.
        if inner.height < 3 {
            if inner.height >= 2 {
                paint_selected_only_fallback(self.theme, inner, state, buffer);
            }
            return;
        }

        let chips: Vec<(ApprovalDecision, String)> = ApprovalDecision::ALL
            .iter()
            .copied()
            .map(|decision| {
                (
                    decision,
                    decision_chip(decision, decision == state.selected),
                )
            })
            .collect();
        let total_width: u16 = chips
            .iter()
            .map(|(_, text)| display_cols(text) as u16)
            .sum::<u16>()
            .saturating_add((chips.len().saturating_sub(1) as u16).saturating_mul(1));

        if total_width <= inner.width {
            // Wide: single horizontal row.
            let y = inner.bottom().saturating_sub(1);
            let mut x = inner.x;
            for (decision, text) in &chips {
                let width = display_cols(text) as u16;
                if x.saturating_add(width) > inner.right() {
                    break;
                }
                let style = if *decision == state.selected {
                    self.theme.style(Role::ActionFocused)
                } else {
                    self.theme.style(Role::TextMuted)
                };
                buffer.set_stringn(x, y, text, usize::from(width), style);
                state.decision_regions.push(ApprovalDecisionRegion {
                    decision: *decision,
                    area: Rect {
                        x,
                        y,
                        width,
                        height: 1,
                    },
                });
                x = x.saturating_add(width.saturating_add(1));
            }
            return;
        }

        // Medium: wrap decisions across available body rows (keep order).
        let body_top = inner.y.saturating_add(1);
        let body_bottom = inner.bottom();
        if body_top >= body_bottom {
            paint_selected_only_fallback(self.theme, inner, state, buffer);
            return;
        }
        let mut y = body_top;
        let mut x = inner.x;
        let mut painted_any = false;
        for (decision, text) in &chips {
            let width = (display_cols(text) as u16).max(1);
            if x > inner.x && x.saturating_add(width) > inner.right() {
                y = y.saturating_add(1);
                x = inner.x;
                if y >= body_bottom {
                    break;
                }
            }
            if width > inner.width {
                // Can't fit this chip at all on this width — selected-only.
                state.decision_regions.clear();
                paint_selected_only_fallback(self.theme, inner, state, buffer);
                return;
            }
            let style = if *decision == state.selected {
                self.theme.style(Role::ActionFocused)
            } else {
                self.theme.style(Role::TextMuted)
            };
            buffer.set_stringn(x, y, text, usize::from(width), style);
            state.decision_regions.push(ApprovalDecisionRegion {
                decision: *decision,
                area: Rect {
                    x,
                    y,
                    width,
                    height: 1,
                },
            });
            painted_any = true;
            x = x.saturating_add(width.saturating_add(1));
        }

        // If selected is missing from painted regions, force selected-only.
        let selected_visible = state
            .decision_regions
            .iter()
            .any(|region| region.decision == state.selected);
        if !painted_any || !selected_visible {
            state.decision_regions.clear();
            paint_selected_only_fallback(self.theme, inner, state, buffer);
        }
    }
}

fn paint_selected_only_fallback(
    theme: &Theme,
    inner: Rect,
    state: &mut ApprovalCardState,
    buffer: &mut Buffer,
) {
    let y = if inner.height >= 2 {
        inner.bottom().saturating_sub(1)
    } else {
        inner.y
    };
    // Non-color nav cue: ‹ [Deny] ›
    let core = decision_chip(state.selected, true);
    let text = format!("‹ {core} ›");
    let clipped = take_display_cols(&text, usize::from(inner.width));
    let width = (display_cols(&clipped) as u16).min(inner.width);
    buffer.set_stringn(
        inner.x,
        y,
        &clipped,
        usize::from(width),
        theme.style(Role::ActionFocused),
    );
    state.decision_regions.push(ApprovalDecisionRegion {
        decision: state.selected,
        area: Rect {
            x: inner.x,
            y,
            width: width.max(1),
            height: 1,
        },
    });
}

impl StatefulWidget for ApprovalCard<'_> {
    type State = ApprovalCardState;

    fn render(self, area: Rect, buffer: &mut Buffer, state: &mut Self::State) {
        <&Self as StatefulWidget>::render(&self, area, buffer, state);
    }
}

// ── Stream view ─────────────────────────────────────────────────────────────

/// Kind of stream turn/block.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[non_exhaustive]
pub enum StreamItemKind {
    /// User prompt.
    User,
    /// Assistant message.
    Assistant,
    /// Tool invocation summary line.
    Tool,
    /// System notice.
    System,
    /// Thinking/reasoning.
    Thinking,
}

/// One stable-ID stream item.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct StreamItem<'a, Id> {
    /// Stable identity.
    pub id: Id,
    /// Kind chrome.
    pub kind: StreamItemKind,
    /// Primary visible text (single line or pre-wrapped).
    pub text: &'a str,
    /// Whether the block is folded.
    pub folded: bool,
}

/// Virtualized conversation stream.
#[derive(Debug, Clone, Copy)]
pub struct StreamView<'a, Id> {
    items: &'a [StreamItem<'a, Id>],
    first: usize,
    theme: &'a Theme,
}

impl<'a, Id> StreamView<'a, Id> {
    /// Creates a stream view.
    #[must_use]
    pub const fn new(items: &'a [StreamItem<'a, Id>], theme: &'a Theme) -> Self {
        Self {
            items,
            first: 0,
            theme,
        }
    }

    /// First visible item index.
    #[must_use]
    pub const fn first(mut self, first: usize) -> Self {
        self.first = first;
        self
    }
}

impl<Id> Widget for &StreamView<'_, Id> {
    fn render(self, area: Rect, buffer: &mut Buffer) {
        if area.is_empty() {
            return;
        }
        for row in 0..area.height {
            let index = self.first.saturating_add(usize::from(row));
            let Some(item) = self.items.get(index) else {
                break;
            };
            let (prefix, role) = match item.kind {
                StreamItemKind::User => ("› ", Role::TextStrong),
                StreamItemKind::Assistant => ("▍ ", Role::Text),
                StreamItemKind::Tool => ("⚙ ", Role::Info),
                StreamItemKind::System => ("· ", Role::TextMuted),
                StreamItemKind::Thinking => ("… ", Role::TextDisabled),
            };
            let fold = if item.folded { "▸ " } else { "" };
            let line = format!("{fold}{prefix}{}", item.text);
            buffer.set_stringn(
                area.x,
                area.y.saturating_add(row),
                take_display_cols(&line, usize::from(area.width)),
                usize::from(area.width),
                self.theme.style(role),
            );
        }
    }
}

impl<Id> Widget for StreamView<'_, Id> {
    fn render(self, area: Rect, buffer: &mut Buffer) {
        <&Self as Widget>::render(&self, area, buffer);
    }
}

// ── Timeline ────────────────────────────────────────────────────────────────

/// One timeline event.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct TimelineEvent<'a> {
    /// Time or sequence label.
    pub when: &'a str,
    /// Event summary.
    pub text: &'a str,
    /// Whether this is the active/current event.
    pub active: bool,
}

/// Vertical activity timeline.
#[derive(Debug, Clone, Copy)]
pub struct Timeline<'a> {
    events: &'a [TimelineEvent<'a>],
    theme: &'a Theme,
}

impl<'a> Timeline<'a> {
    /// Creates a timeline.
    #[must_use]
    pub const fn new(events: &'a [TimelineEvent<'a>], theme: &'a Theme) -> Self {
        Self { events, theme }
    }
}

impl Widget for &Timeline<'_> {
    fn render(self, area: Rect, buffer: &mut Buffer) {
        if area.is_empty() {
            return;
        }
        for (row, event) in self
            .events
            .iter()
            .enumerate()
            .take(usize::from(area.height))
        {
            let y = area.y.saturating_add(row as u16);
            let bullet = if event.active { "●" } else { "○" };
            let line = format!("{bullet} {}  {}", event.when, event.text);
            let role = if event.active {
                Role::Accent
            } else {
                Role::TextMuted
            };
            buffer.set_stringn(
                area.x,
                y,
                take_display_cols(&line, usize::from(area.width)),
                usize::from(area.width),
                self.theme.style(role),
            );
        }
    }
}

impl Widget for Timeline<'_> {
    #[expect(
        clippy::needless_borrows_for_generic_args,
        reason = "explicitly delegate the owned contract to the borrowed renderer"
    )]
    fn render(self, area: Rect, buffer: &mut Buffer) {
        <&Self as Widget>::render(&self, area, buffer);
    }
}

// ── Prompt box ──────────────────────────────────────────────────────────────

/// Outcome from the multi-line agent prompt.
#[derive(Debug, Clone, PartialEq, Eq)]
#[non_exhaustive]
pub enum PromptBoxOutcome {
    /// Ignored.
    Ignored,
    /// Draft text changed.
    Changed,
    /// Submit requested (caller reads draft).
    Submitted,
    /// Cancel / clear request.
    Cancelled,
}

/// Multi-line prompt chrome around [`TextArea`].
#[derive(Debug, Clone, Copy)]
pub struct PromptBox<'a> {
    placeholder: &'a str,
    theme: &'a Theme,
}

impl<'a> PromptBox<'a> {
    /// Creates a prompt box.
    #[must_use]
    pub const fn new(theme: &'a Theme) -> Self {
        Self {
            placeholder: "Message…",
            theme,
        }
    }

    /// Placeholder when empty.
    #[must_use]
    pub const fn placeholder(mut self, placeholder: &'a str) -> Self {
        self.placeholder = placeholder;
        self
    }
}

/// Prompt state (text area + focused flag).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PromptBoxState {
    /// Underlying multiline editor.
    pub editor: TextAreaState,
    focused: bool,
}

impl Default for PromptBoxState {
    fn default() -> Self {
        Self::new()
    }
}

impl PromptBoxState {
    /// Creates an empty prompt.
    #[must_use]
    pub fn new() -> Self {
        let mut editor = TextAreaState::default();
        editor.set_focused(true);
        Self {
            editor,
            focused: true,
        }
    }

    /// Draft text.
    #[must_use]
    pub fn text(&self) -> String {
        self.editor.text()
    }

    /// Whether the prompt owns keyboard focus.
    #[must_use]
    pub const fn is_focused(&self) -> bool {
        self.focused
    }

    /// Sets focus.
    pub fn set_focused(&mut self, focused: bool) {
        self.focused = focused;
        self.editor.set_focused(focused);
    }

    /// Handles a key. Enter submits when modifiers are empty and draft non-empty;
    /// plain Enter inserts newline when Shift is held? Convention: Enter submits,
    /// Ctrl/Shift+Enter inserts newline via TextArea when mapped by caller.
    /// Here: Enter submits; Alt+Enter inserts newline.
    pub fn handle_key(&mut self, key: KeyEvent) -> PromptBoxOutcome {
        if !self.focused || key.kind != KeyEventKind::Press {
            return PromptBoxOutcome::Ignored;
        }
        if key.code == KeyCode::Enter && key.modifiers.is_empty() {
            if self.text().trim().is_empty() {
                return PromptBoxOutcome::Ignored;
            }
            return PromptBoxOutcome::Submitted;
        }
        if key.code == KeyCode::Esc {
            return PromptBoxOutcome::Cancelled;
        }
        match self.editor.handle_key(key) {
            TextAreaOutcome::Changed => PromptBoxOutcome::Changed,
            TextAreaOutcome::Cancelled => PromptBoxOutcome::Cancelled,
            TextAreaOutcome::Ignored => PromptBoxOutcome::Ignored,
        }
    }
}

impl StatefulWidget for &PromptBox<'_> {
    type State = PromptBoxState;

    fn render(self, area: Rect, buffer: &mut Buffer, state: &mut Self::State) {
        if area.is_empty() {
            return;
        }
        StatefulWidget::render(
            &TextArea::new(self.theme).placeholder(self.placeholder),
            area,
            buffer,
            &mut state.editor,
        );
    }
}

impl StatefulWidget for PromptBox<'_> {
    type State = PromptBoxState;

    fn render(self, area: Rect, buffer: &mut Buffer, state: &mut Self::State) {
        <&Self as StatefulWidget>::render(&self, area, buffer, state);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::input::KeyModifiers;

    #[test]
    fn tool_status_glyphs_are_non_color() {
        assert_eq!(ToolStatus::Done.glyph(), "✓");
        assert_eq!(ToolStatus::Error.glyph(), "✗");
        assert_eq!(ToolStatus::Cancelled.glyph(), "⊘");
    }

    #[test]
    fn approval_default_enter_confirms_deny_never_allow() {
        let mut state = ApprovalCardState::new();
        assert_eq!(state.selected(), ApprovalDecision::Deny);
        assert_eq!(
            state.handle_key(KeyEvent::new(KeyCode::Enter, KeyModifiers::NONE)),
            ApprovalCardOutcome::Confirmed(ApprovalDecision::Deny)
        );
        assert_ne!(
            state.handle_key(KeyEvent::new(KeyCode::Enter, KeyModifiers::NONE)),
            ApprovalCardOutcome::Confirmed(ApprovalDecision::AllowOnce)
        );
    }

    #[test]
    fn approval_escape_is_cancelled_not_deny() {
        let mut state = ApprovalCardState::new();
        assert_eq!(
            state.handle_key(KeyEvent::new(KeyCode::Esc, KeyModifiers::NONE)),
            ApprovalCardOutcome::Cancelled
        );
    }

    #[test]
    fn approval_tab_and_backtab_wrap_full_decision_set() {
        let mut state = ApprovalCardState::new();
        assert_eq!(state.selected(), ApprovalDecision::Deny);
        assert_eq!(
            state.handle_key(KeyEvent::new(KeyCode::Tab, KeyModifiers::NONE)),
            ApprovalCardOutcome::SelectionChanged
        );
        assert_eq!(state.selected(), ApprovalDecision::Defer);
        assert_eq!(
            state.handle_key(KeyEvent::new(KeyCode::Tab, KeyModifiers::NONE)),
            ApprovalCardOutcome::SelectionChanged
        );
        assert_eq!(state.selected(), ApprovalDecision::AllowOnce);
        assert_eq!(
            state.handle_key(KeyEvent::new(KeyCode::BackTab, KeyModifiers::NONE)),
            ApprovalCardOutcome::SelectionChanged
        );
        assert_eq!(state.selected(), ApprovalDecision::Defer);
    }

    #[test]
    fn approval_enter_repeat_does_not_confirm() {
        let mut state = ApprovalCardState::with_selected(ApprovalDecision::AllowOnce);
        let mut key = KeyEvent::new(KeyCode::Enter, KeyModifiers::NONE);
        key.kind = KeyEventKind::Repeat;
        assert_eq!(state.handle_key(key), ApprovalCardOutcome::Ignored);
    }

    #[test]
    fn approval_y_n_shortcuts_are_explicit_confirms() {
        let mut state = ApprovalCardState::new();
        assert_eq!(
            state.handle_key(KeyEvent::new(KeyCode::Char('y'), KeyModifiers::NONE)),
            ApprovalCardOutcome::Confirmed(ApprovalDecision::AllowOnce)
        );
        assert_eq!(
            state.handle_key(KeyEvent::new(KeyCode::Char('n'), KeyModifiers::NONE)),
            ApprovalCardOutcome::Confirmed(ApprovalDecision::Deny)
        );
    }

    #[test]
    fn approval_selected_visible_at_every_non_empty_width() {
        use ratatui_core::{backend::TestBackend, terminal::Terminal};

        let theme = Theme::default();
        let card = ApprovalCard::new("Permission", "Run tool?", ApprovalRisk::High, &theme);
        for width in 0u16..=48 {
            let mut state = ApprovalCardState::new();
            let height = 6u16;
            let mut terminal = Terminal::new(TestBackend::new(width.max(1), height)).unwrap();
            let area = Rect::new(0, 0, width, height);
            terminal
                .draw(|frame| {
                    frame.render_stateful_widget(&card, area, &mut state);
                })
                .unwrap();
            if width == 0 {
                assert!(state.decision_regions.is_empty());
                continue;
            }
            // Selected must be published as a hit region whenever anything painted.
            if !state.decision_regions.is_empty() {
                assert!(
                    state
                        .decision_regions
                        .iter()
                        .any(|region| region.decision == state.selected()),
                    "width {width}: selected {:?} missing from {:?}",
                    state.selected(),
                    state.decision_regions
                );
            }
        }
    }

    #[test]
    fn prompt_submit_requires_non_empty_draft() {
        let mut state = PromptBoxState::new();
        assert_eq!(
            state.handle_key(KeyEvent::new(KeyCode::Enter, KeyModifiers::NONE)),
            PromptBoxOutcome::Ignored
        );
        state.editor = TextAreaState::new("hello");
        state.editor.set_focused(true);
        assert_eq!(
            state.handle_key(KeyEvent::new(KeyCode::Enter, KeyModifiers::NONE)),
            PromptBoxOutcome::Submitted
        );
    }
}
