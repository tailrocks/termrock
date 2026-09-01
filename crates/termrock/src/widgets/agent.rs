// SPDX-FileCopyrightText: 2026 Alexey Zhokhov
// SPDX-License-Identifier: Apache-2.0

//! Agent-era experience widgets: tool cards, thinking, meters.
//! Timeline: [`super::timeline`].
//!
//! Conversation stream: [`crate::widgets::Transcript`] only (StreamView deleted).
//! Trust / prompt: [`crate::widgets::PermissionPrompt`] / [`crate::widgets::PromptComposer`].

use ratatui_core::{buffer::Buffer, layout::Rect, widgets::Widget};

use crate::{
    style::{DesignSystem, Role},
    text::take_display_cols,
    widgets::{PanelChrome, SemanticStatus},
};

use super::{accent_rail::AccentRail, status_indicator::StatusIndicator};

// ── Token meter ─────────────────────────────────────────────────────────────

/// Compact token/cost usage meter.
///
/// Prefer [`crate::widgets::ContextMeter`] for approximate precision, breakdown,
/// compaction thresholds, and non-token budgets (migration `0225`).
#[derive(Debug, Clone, Copy)]
pub struct TokenMeter<'a> {
    used: u64,
    limit: u64,
    label: &'a str,
    system: &'a DesignSystem,
}

impl<'a> TokenMeter<'a> {
    /// Creates a token meter.
    #[must_use]
    pub const fn new(used: u64, limit: u64, system: &'a DesignSystem) -> Self {
        Self {
            used,
            limit,
            label: "tokens",
            system,
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
        let warning = if fraction >= 0.9 {
            Some("critical")
        } else if fraction >= 0.75 {
            Some("warning")
        } else {
            None
        };
        if let Some(label) = warning {
            let status = StatusIndicator::new(SemanticStatus::Warning, self.system).label(label);
            let status_width = status.measure_width(None).min(area.width);
            Widget::render(&status, Rect::new(area.x, area.y, status_width, 1), buffer);
            let x = area.x.saturating_add(status_width.saturating_add(1));
            let width = area.right().saturating_sub(x);
            if width > 0 {
                buffer.set_stringn(
                    x,
                    area.y,
                    take_display_cols(&format!("· {text}"), usize::from(width)),
                    usize::from(width),
                    self.system.style(Role::TextMuted),
                );
            }
        } else {
            let clipped = take_display_cols(&text, usize::from(area.width));
            buffer.set_stringn(
                area.x,
                area.y,
                &clipped,
                usize::from(area.width),
                self.system.style(Role::TextMuted),
            );
        }
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
    system: &'a DesignSystem,
}

impl<'a> ThinkingBlock<'a> {
    /// Creates a thinking block.
    #[must_use]
    pub const fn new(summary: &'a str, system: &'a DesignSystem) -> Self {
        Self {
            summary,
            expanded: false,
            body: "",
            frame: "·",
            system,
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
        use crate::layout::{FlexSize, Stack};

        let content = AccentRail::new(self.system, Role::ActorAssistant)
            .active(true)
            .tick(self.system.elapsed_ms() / 80)
            .paint(area, buffer);
        if content.is_empty() {
            return;
        }
        let show_body = self.expanded && !self.body.is_empty() && content.height > 1;
        let layout = if show_body {
            Stack::new().layout(content, &[FlexSize::Fixed(1), FlexSize::fill()])
        } else {
            Stack::new().layout(content, &[FlexSize::Fixed(1)])
        };
        let marker = if self.expanded {
            self.system.glyphs.disclosure_open()
        } else {
            self.system.glyphs.disclosure_closed()
        };
        let frame = if self.system.motion.animate_spinners() && !self.frame.is_empty() {
            self.frame
        } else {
            SemanticStatus::Running.glyph_for_set(self.system.glyphs)
        };
        let header = format!("{marker} {frame} {}", self.summary);
        if let Some(header_r) = layout.get(0) {
            let clipped = take_display_cols(&header, usize::from(header_r.width));
            buffer.set_stringn(
                header_r.x,
                header_r.y,
                &clipped,
                usize::from(header_r.width),
                self.system.style(Role::TextMuted),
            );
        }
        if show_body
            && let Some(body_r) = layout.get(1)
            && body_r.height > 0
        {
            let body = take_display_cols(self.body, usize::from(body_r.width));
            buffer.set_stringn(
                body_r.x,
                body_r.y,
                &body,
                usize::from(body_r.width),
                self.system.style(Role::TextDisabled),
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
///
/// Elevated for [`crate::widgets::ToolCallCard`] (migration `0219`). Prefer these
/// names over historical Pending/Done/Error.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
#[non_exhaustive]
pub enum ToolStatus {
    /// Queued, not started.
    #[default]
    Queued,
    /// Preparing / resolving args.
    Preparing,
    /// Currently executing.
    Running,
    /// Waiting for host / user input.
    WaitingInput,
    /// Waiting for permission grant.
    WaitingPermission,
    /// Streaming tool output.
    Streaming,
    /// Completed successfully.
    Success,
    /// Completed with warning.
    Warning,
    /// Failed.
    Failed,
    /// Cancelled by user or policy.
    Cancelled,
    /// Detached / backgrounded (host still owns process).
    Detached,
}

impl ToolStatus {
    /// Stable id.
    #[must_use]
    pub const fn id(self) -> &'static str {
        match self {
            Self::Queued => "queued",
            Self::Preparing => "preparing",
            Self::Running => "running",
            Self::WaitingInput => "waiting-input",
            Self::WaitingPermission => "waiting-permission",
            Self::Streaming => "streaming",
            Self::Success => "success",
            Self::Warning => "warning",
            Self::Failed => "failed",
            Self::Cancelled => "cancelled",
            Self::Detached => "detached",
        }
    }

    /// Short badge label.
    #[must_use]
    pub const fn badge(self) -> &'static str {
        match self {
            Self::Queued => "queued",
            Self::Preparing => "prep",
            Self::Running => "run",
            Self::WaitingInput => "input",
            Self::WaitingPermission => "perm",
            Self::Streaming => "stream",
            Self::Success => "ok",
            Self::Warning => "warn",
            Self::Failed => "err",
            Self::Cancelled => "cancel",
            Self::Detached => "bg",
        }
    }

    /// Shared vocabulary projection.
    #[must_use]
    pub const fn semantic(self) -> SemanticStatus {
        match self {
            Self::Queued | Self::Preparing => SemanticStatus::Queued,
            Self::Running | Self::Streaming | Self::Detached => SemanticStatus::Running,
            Self::WaitingInput | Self::WaitingPermission => SemanticStatus::Paused,
            Self::Success => SemanticStatus::Success,
            Self::Warning => SemanticStatus::Warning,
            Self::Failed => SemanticStatus::Failed,
            Self::Cancelled => SemanticStatus::Paused,
        }
    }

    /// Non-color status glyph (shared [`SemanticStatus`] unicode set).
    #[must_use]
    pub const fn glyph(self) -> &'static str {
        self.semantic().glyph_unicode()
    }

    /// ASCII / letter fallback for colorless.
    #[must_use]
    pub const fn letter(self) -> char {
        match self {
            Self::Queued => 'Q',
            Self::Preparing => 'P',
            Self::Running => 'R',
            Self::WaitingInput => 'I',
            Self::WaitingPermission => 'A',
            Self::Streaming => 'S',
            Self::Success => '✓',
            Self::Warning => '!',
            Self::Failed => 'E',
            Self::Cancelled => 'X',
            Self::Detached => 'D',
        }
    }

    /// Whether cancel action is meaningful.
    #[must_use]
    pub const fn can_cancel(self) -> bool {
        matches!(
            self,
            Self::Queued
                | Self::Preparing
                | Self::Running
                | Self::Streaming
                | Self::WaitingInput
                | Self::WaitingPermission
                | Self::Detached
        )
    }

    /// Whether retry is meaningful.
    #[must_use]
    pub const fn can_retry(self) -> bool {
        matches!(self, Self::Failed | Self::Cancelled | Self::Warning)
    }
}

/// Mutable streaming tool call card.
///
/// For full command chrome (cwd, exit, follow, cancel **requests**), prefer
/// [`super::TerminalOutput`]. ToolCard stays the compact agent-tool summary.
#[derive(Debug, Clone, Copy)]
pub struct ToolCard<'a> {
    name: &'a str,
    summary: &'a str,
    status: ToolStatus,
    detail: Option<&'a str>,
    expanded: bool,
    system: &'a DesignSystem,
}

impl<'a> ToolCard<'a> {
    /// Creates a tool card.
    #[must_use]
    pub const fn new(
        name: &'a str,
        summary: &'a str,
        status: ToolStatus,
        system: &'a DesignSystem,
    ) -> Self {
        Self {
            name,
            summary,
            status,
            detail: None,
            expanded: false,
            system,
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
        use crate::widgets::Card;
        let status_label = self.status.badge();
        // Chrome owns name / status badge / summary; body owns tool output only.
        let kind = self.status.semantic();
        let card = Card::new(self.system)
            .title(self.name)
            .leading(kind.glyph_for_set(self.system.glyphs))
            .badge(status_label)
            .subtitle(self.summary)
            .emphasis(match self.status {
                ToolStatus::Failed => PanelChrome::Danger,
                ToolStatus::Running | ToolStatus::Streaming | ToolStatus::WaitingPermission => {
                    PanelChrome::Focused
                }
                ToolStatus::Warning => PanelChrome::Normal,
                _ => PanelChrome::Normal,
            });
        let body = card.paint(area, buffer, None);
        if body.is_empty() {
            return;
        }
        if self.expanded {
            if let Some(detail) = self.detail {
                let line = take_display_cols(detail, usize::from(body.width));
                buffer.set_stringn(
                    body.x,
                    body.y,
                    &line,
                    usize::from(body.width),
                    self.system.style(Role::TextMuted),
                );
            }
        } else {
            // Collapsed: one muted status line (non-color status already in badge).
            let line = take_display_cols(self.summary, usize::from(body.width));
            buffer.set_stringn(
                body.x,
                body.y,
                &line,
                usize::from(body.width),
                self.system.style(Role::TextMuted),
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

// Timeline lives in `timeline` module (re-exported from widgets root).

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn tool_status_glyphs_are_non_color() {
        assert_eq!(
            ToolStatus::Success.glyph(),
            SemanticStatus::Success.glyph_unicode()
        );
        assert_eq!(
            ToolStatus::Failed.glyph(),
            SemanticStatus::Failed.glyph_unicode()
        );
        assert_eq!(
            ToolStatus::Cancelled.glyph(),
            SemanticStatus::Paused.glyph_unicode()
        );
        assert_eq!(ToolStatus::Queued.semantic(), SemanticStatus::Queued);
    }

    #[test]
    fn warning_meter_spells_status_with_rail_and_glyph() {
        let system = DesignSystem::default();
        let area = Rect::new(0, 0, 48, 1);
        let mut buffer = Buffer::empty(area);
        Widget::render(TokenMeter::new(90, 100, &system), area, &mut buffer);
        let text = (0..area.width)
            .map(|x| buffer[(x, 0)].symbol())
            .collect::<String>();
        assert!(text.contains("┃ ! critical"), "{text:?}");
    }

    #[test]
    fn reduced_motion_thinking_block_uses_static_status_glyph() {
        let system = DesignSystem::default().motion(crate::style::MotionPolicy::Off);
        let area = Rect::new(0, 0, 32, 1);
        let mut buffer = Buffer::empty(area);
        Widget::render(
            ThinkingBlock::new("reviewing files", &system).frame("⠋"),
            area,
            &mut buffer,
        );
        let text = (0..area.width)
            .map(|x| buffer[(x, 0)].symbol())
            .collect::<String>();
        assert!(text.contains("◉ reviewing files"), "{text:?}");
        assert!(!text.contains('⠋'));
    }

    #[test]
    fn no_legacy_approval_or_prompt_box_types() {
        let src = include_str!("agent.rs");
        let code = src.split("#[cfg(test)]").next().unwrap_or(src);
        let a = ["pub struct Approv", "alCard"].concat();
        let b = ["pub struct Prompt", "Box"].concat();
        let c = ["pub enum Approval", "Decision"].concat();
        assert!(!code.contains(&a) && !code.contains(&b) && !code.contains(&c));
    }
}
