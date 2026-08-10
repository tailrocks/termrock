// SPDX-FileCopyrightText: 2026 Alexey Zhokhov
// SPDX-License-Identifier: Apache-2.0

//! Agent-era experience widgets: tool cards, thinking, meters, timeline.
//!
//! Conversation stream: [`crate::widgets::Transcript`] only (StreamView deleted).
//! Trust / prompt: [`crate::widgets::PermissionPrompt`] / [`crate::widgets::PromptComposer`].

use ratatui_core::{buffer::Buffer, layout::Rect, widgets::Widget};

use crate::{
    style::{DesignSystem, Role},
    text::take_display_cols,
    widgets::{PanelChrome, SemanticStatus},
};

// ── Token meter ─────────────────────────────────────────────────────────────

/// Compact token/cost usage meter.
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
            self.system.style(role),
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

        let show_body = self.expanded && !self.body.is_empty() && area.height > 1;
        let layout = if show_body {
            Stack::new().layout(area, &[FlexSize::Fixed(1), FlexSize::fill()])
        } else {
            Stack::new().layout(area, &[FlexSize::Fixed(1)])
        };
        let marker = if self.expanded {
            self.system.glyphs.disclosure_open()
        } else {
            self.system.glyphs.disclosure_closed()
        };
        let header = format!("{} {} {}", marker, self.frame, self.summary);
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
    /// Shared vocabulary projection.
    #[must_use]
    pub const fn semantic(self) -> SemanticStatus {
        match self {
            Self::Pending => SemanticStatus::Queued,
            Self::Running => SemanticStatus::Running,
            Self::Done => SemanticStatus::Success,
            Self::Error => SemanticStatus::Failed,
            Self::Cancelled => SemanticStatus::Paused,
        }
    }

    /// Non-color status glyph (shared [`SemanticStatus`] unicode set).
    #[must_use]
    pub const fn glyph(self) -> &'static str {
        self.semantic().glyph_unicode()
    }

    /// Theme role for the status (aligned with [`SemanticStatus`]).
    #[must_use]
    pub const fn role(self) -> Role {
        self.semantic().role()
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
        let status_label = match self.status {
            ToolStatus::Pending => "pending",
            ToolStatus::Running => "run",
            ToolStatus::Done => "done",
            ToolStatus::Error => "err",
            ToolStatus::Cancelled => "cancel",
        };
        // Chrome owns name / status badge / summary; body owns tool output only.
        let kind = self.status.semantic();
        let card = Card::new(self.system)
            .title(self.name)
            .leading(kind.glyph_for_set(self.system.glyphs))
            .badge(status_label)
            .subtitle(self.summary)
            .emphasis(match self.status {
                ToolStatus::Error => PanelChrome::Danger,
                ToolStatus::Running => PanelChrome::Focused,
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
                self.system.style(self.status.role()),
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
    system: &'a DesignSystem,
}

impl<'a> Timeline<'a> {
    /// Creates a timeline.
    #[must_use]
    pub const fn new(events: &'a [TimelineEvent<'a>], system: &'a DesignSystem) -> Self {
        Self { events, system }
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
                self.system.style(role),
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn tool_status_glyphs_are_non_color() {
        assert_eq!(ToolStatus::Done.glyph(), SemanticStatus::Success.glyph_unicode());
        assert_eq!(ToolStatus::Error.glyph(), SemanticStatus::Failed.glyph_unicode());
        assert_eq!(
            ToolStatus::Cancelled.glyph(),
            SemanticStatus::Paused.glyph_unicode()
        );
        assert_eq!(ToolStatus::Pending.semantic(), SemanticStatus::Queued);
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
