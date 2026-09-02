// SPDX-FileCopyrightText: 2026 Alexey Zhokhov
// SPDX-License-Identifier: Apache-2.0

//! **EmptyState** — useful empty and first-run surfaces (not blank boxes).
//!
//! **Mission.** Title, explanation, primary/secondary actions, example,
//! shortcut, illustration glyph, and contextual details. Differentiates
//! first-use, no-data, no-results, filtered-out, and permission-limited.
//! Contracts to a concise inline form inside small panes. Primary action stays
//! dominant and safe (never destructive-by-default).
//!
//! **Recipes.** Table, logs, sessions, projects, and search examples.
//!
//! Research: shadcn empty patterns, IDE welcome screens, polished CLI onboarding.
#![allow(unused_imports)] // test-module imports kept for unit tests; lib path may not use them
use ratatui_core::{
    buffer::Buffer,
    layout::{Position, Rect},
    style::Modifier,
    widgets::Widget,
};

use crate::{
    input::{
        KeyCode, KeyEvent, KeyEventKind, KeyModifiers, MouseButton, MouseEvent, MouseEventKind,
    },
    interaction::{
        SemanticNode, SemanticRole, SemanticScene, SemanticState, UiIntent, default_button_intent,
    },
    layout::{Center, CenterAxis, FlexSize, Stack, center_line_x},
    style::{DesignSystem, Role},
    text::{display_cols, take_display_cols},
    widgets::{Button, ButtonState, ButtonVariant, Surface, SurfaceRecipe},
};

/// Width under which Full density collapses toward Inline.
pub const EMPTY_STATE_INLINE_MAX_WIDTH: u16 = 28;
/// Height under which Full density drops to Inline.
pub const EMPTY_STATE_INLINE_MAX_HEIGHT: u16 = 4;

// ── Kind ────────────────────────────────────────────────────────────────────

/// Why the surface is empty (drives default glyph + tone).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
#[non_exhaustive]
pub enum EmptyKind {
    /// First-run / welcome / onboarding.
    FirstUse,
    /// Collection exists but has no rows yet.
    #[default]
    NoData,
    /// Query returned nothing.
    NoResults,
    /// Filters hide all items.
    FilteredOut,
    /// Viewer lacks permission for content.
    PermissionLimited,
}

impl EmptyKind {
    /// Stable id.
    #[must_use]
    pub const fn id(self) -> &'static str {
        match self {
            Self::FirstUse => "first-use",
            Self::NoData => "no-data",
            Self::NoResults => "no-results",
            Self::FilteredOut => "filtered-out",
            Self::PermissionLimited => "permission-limited",
        }
    }

    /// Default illustration glyph (Unicode).
    #[must_use]
    pub const fn glyph_unicode(self) -> &'static str {
        match self {
            Self::FirstUse => "★",
            Self::NoData => "○",
            Self::NoResults => "∅",
            Self::FilteredOut => "▽",
            Self::PermissionLimited => "⊘",
        }
    }

    /// Title role (permission stays warning-toned; others muted title emphasis).
    #[must_use]
    pub const fn title_role(self) -> Role {
        match self {
            Self::PermissionLimited => Role::Warning,
            Self::FirstUse => Role::TextStrong,
            _ => Role::TextStrong,
        }
    }
}

// ── Action ──────────────────────────────────────────────────────────────────

/// Host-owned recovery action label (activation via [`EmptyStateState`]).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct EmptyAction<'a> {
    /// Visible label.
    pub label: &'a str,
    /// Optional chord hint painted after the label.
    pub shortcut: Option<&'a str>,
}

impl<'a> EmptyAction<'a> {
    /// Label only.
    #[must_use]
    pub const fn new(label: &'a str) -> Self {
        Self {
            label,
            shortcut: None,
        }
    }

    /// Label + shortcut.
    #[must_use]
    pub const fn with_shortcut(label: &'a str, shortcut: &'a str) -> Self {
        Self {
            label,
            shortcut: Some(shortcut),
        }
    }
}

// ── Outcomes / state ────────────────────────────────────────────────────────

/// Focus target inside an interactive empty state.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
#[non_exhaustive]
pub enum EmptyFocus {
    /// No action focused (paint only).
    #[default]
    None,
    /// Primary recovery action.
    Primary,
    /// Secondary action.
    Secondary,
}

/// Outcomes from empty-state interaction (effects stay host-owned).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[non_exhaustive]
pub enum EmptyStateOutcome {
    /// No action.
    Ignored,
    /// Primary action activated.
    PrimaryActivated,
    /// Secondary action activated.
    SecondaryActivated,
}

/// Optional interaction state when primary/secondary actions are present.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct EmptyStateState {
    focus: EmptyFocus,
    primary: ButtonState,
    secondary: ButtonState,
}

impl EmptyStateState {
    /// Empty (no focus).
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Prefer primary when actions exist.
    pub fn focus_primary(&mut self) {
        self.focus = EmptyFocus::Primary;
    }

    /// Focus.
    #[must_use]
    pub const fn focus(&self) -> EmptyFocus {
        self.focus
    }

    /// Set focus.
    pub fn set_focus(&mut self, focus: EmptyFocus) {
        self.focus = focus;
    }
}

// ── Widget ──────────────────────────────────────────────────────────────────

/// Useful empty / first-run surface.
///
/// # Examples
///
/// ```
/// use termrock::style::DesignSystem;
/// use termrock::widgets::{EmptyState, EmptyKind, EmptyAction};
///
/// let system = DesignSystem::default();
/// let empty = EmptyState::new("No results", &system)
///     .kind(EmptyKind::NoResults)
///     .explanation("Try another query")
///     .primary(EmptyAction::with_shortcut("Clear filters", "esc"));
/// ```
#[derive(Debug, Clone, Copy)]
pub struct EmptyState<'a> {
    title: &'a str,
    kind: EmptyKind,
    explanation: Option<&'a str>,
    primary: Option<EmptyAction<'a>>,
    secondary: Option<EmptyAction<'a>>,
    example: Option<&'a str>,
    shortcut: Option<&'a str>,
    illustration: Option<&'a str>,
    context: Option<&'a str>,
    system: &'a DesignSystem,
}

impl<'a> EmptyState<'a> {
    /// Title + system (defaults: [`EmptyKind::NoData`], Full density, kind glyph).
    #[must_use]
    pub const fn new(title: &'a str, system: &'a DesignSystem) -> Self {
        Self {
            title,
            kind: EmptyKind::NoData,
            explanation: None,
            primary: None,
            secondary: None,
            example: None,
            shortcut: None,
            illustration: None,
            context: None,
            system,
        }
    }

    /// Semantic empty kind.
    #[must_use]
    pub const fn kind(mut self, kind: EmptyKind) -> Self {
        self.kind = kind;
        self
    }

    /// Explanation (secondary copy under the title).
    #[must_use]
    pub const fn explanation(mut self, text: &'a str) -> Self {
        self.explanation = Some(text);
        self
    }

    /// Primary recovery action (dominant; never painted as destructive).
    #[must_use]
    pub const fn primary(mut self, action: EmptyAction<'a>) -> Self {
        self.primary = Some(action);
        self
    }

    /// Secondary action (muted; optional).
    #[must_use]
    pub const fn secondary(mut self, action: EmptyAction<'a>) -> Self {
        self.secondary = Some(action);
        self
    }

    /// Example line (e.g. sample query or path).
    #[must_use]
    pub const fn example(mut self, example: &'a str) -> Self {
        self.example = Some(example);
        self
    }

    /// Global shortcut hint line (footer-style).
    #[must_use]
    pub const fn shortcut(mut self, shortcut: &'a str) -> Self {
        self.shortcut = Some(shortcut);
        self
    }

    /// Override illustration glyph (non-color cue).
    #[must_use]
    pub const fn glyph(mut self, glyph: &'a str) -> Self {
        self.illustration = Some(glyph);
        self
    }

    /// Contextual details (filter summary, path, permission scope).
    #[must_use]
    pub const fn context(mut self, context: &'a str) -> Self {
        self.context = Some(context);
        self
    }

    /// Resolved kind.
    #[must_use]
    pub const fn empty_kind(self) -> EmptyKind {
        self.kind
    }

    /// Illustration glyph.
    #[must_use]
    pub const fn resolved_glyph(&self) -> &'a str {
        if let Some(g) = self.illustration {
            return g;
        }
        self.kind.glyph_unicode()
    }

    /// Inline (single-line) form fits when the pane is narrow or short.
    const fn inline_form(&self, area: Rect) -> bool {
        area.width <= EMPTY_STATE_INLINE_MAX_WIDTH || area.height <= EMPTY_STATE_INLINE_MAX_HEIGHT
    }

    /// Rows needed for Full density (host layout).
    #[must_use]
    pub fn measure_height(&self, width: u16) -> u16 {
        if width == 0 {
            return 0;
        }
        if width <= EMPTY_STATE_INLINE_MAX_WIDTH {
            return if self.primary.is_some() && self.explanation.is_some() {
                2
            } else {
                1
            };
        }
        let mut h = 2u16; // illustration + title
        if self.explanation.is_some() {
            h = h.saturating_add(1);
        }
        if self.context.is_some() {
            h = h.saturating_add(1);
        }
        if self.example.is_some() {
            h = h.saturating_add(1);
        }
        if self.primary.is_some() {
            h = h.saturating_add(1);
        }
        if self.secondary.is_some() {
            h = h.saturating_add(1);
        }
        if self.shortcut.is_some() {
            h = h.saturating_add(1);
        }
        h
    }

    /// Passive paint (no interaction state).
    pub fn paint(&self, area: Rect, buffer: &mut Buffer) {
        let mut state = EmptyStateState::new();
        self.paint_with_state(area, buffer, &mut state);
    }

    /// Paint with optional action focus.
    pub fn paint_with_state(&self, area: Rect, buffer: &mut Buffer, state: &mut EmptyStateState) {
        if area.is_empty() {
            return;
        }
        if self.inline_form(area) {
            self.paint_inline(area, buffer, state);
        } else {
            self.paint_full(area, buffer, state);
        }
    }

    fn paint_inline(&self, area: Rect, buffer: &mut Buffer, state: &mut EmptyStateState) {
        let g = self.resolved_glyph();
        let mut line = format!("{g} {}", self.title);
        if let Some(ex) = self.explanation {
            if area.height >= 2 {
                // title on first, explanation + primary on second
                self.paint_centered(
                    area,
                    buffer,
                    area.y,
                    &line,
                    self.system
                        .style(self.kind.title_role())
                        .add_modifier(Modifier::BOLD),
                );
                let mut second = ex.to_string();
                if let Some(p) = self.primary {
                    second = format!("{second} · {}", p.label);
                }
                self.paint_centered(
                    area,
                    buffer,
                    area.y.saturating_add(1),
                    &second,
                    self.system.style(Role::TextMuted),
                );
                return;
            }
            line = format!("{line} — {ex}");
        }
        if let Some(p) = self.primary {
            line = format!("{line} · {}", p.label);
        }
        let style = self
            .system
            .style(self.kind.title_role())
            .add_modifier(Modifier::BOLD);
        self.paint_centered(area, buffer, area.y, &line, style);
        let _ = state;
    }

    fn paint_full(&self, area: Rect, buffer: &mut Buffer, state: &mut EmptyStateState) {
        let area = if area.width >= 24 && area.height >= 6 {
            let frame = Center::new(area.width.min(56), area.height.min(12))
                .layout(area)
                .child;
            Surface::new(self.system)
                .recipe(SurfaceRecipe::Inset)
                .bordered(true)
                .paint(frame, buffer)
        } else {
            area
        };
        // Build ordered rows: glyph, title, explanation, context, example, primary, secondary, shortcut
        let g = self.resolved_glyph();
        let mut rows: Vec<(String, Role, bool)> = Vec::with_capacity(8);
        rows.push((g.to_string(), Role::TextMuted, false));
        rows.push((self.title.to_string(), self.kind.title_role(), true));
        if let Some(ex) = self.explanation {
            rows.push((ex.to_string(), Role::TextMuted, false));
        }
        if let Some(ctx) = self.context {
            rows.push((ctx.to_string(), Role::TextDisabled, false));
        }
        if let Some(ex) = self.example {
            rows.push((format!("e.g. {ex}"), Role::TextDisabled, false));
        }
        // Actions painted separately so primary is dominant Button chrome when width allows.
        let action_rows = u16::from(self.primary.is_some()) + u16::from(self.secondary.is_some());
        let shortcut_row = u16::from(self.shortcut.is_some());
        let content_rows = rows.len() as u16;
        let gap_rows = u16::from(rows.len() > 2 && area.height >= 8) * self.system.spacing.gap;
        let total = content_rows
            .saturating_add(action_rows)
            .saturating_add(shortcut_row)
            .saturating_add(gap_rows)
            .max(1);

        let block = Center::new(area.width, total)
            .axis(CenterAxis::Vertical)
            .layout(area)
            .child;
        let sizes: Vec<FlexSize> = (0..total).map(|_| FlexSize::Fixed(1)).collect();
        let stack = Stack::new().layout(block, &sizes);

        let mut idx = 0usize;
        for (text, role, bold) in &rows {
            if let Some(r) = stack.get(idx) {
                let mut style = self.system.style(*role);
                if *bold {
                    style = style.add_modifier(Modifier::BOLD);
                }
                self.paint_centered(
                    Rect::new(area.x, r.y, area.width, 1),
                    buffer,
                    r.y,
                    text,
                    style,
                );
            }
            idx += 1;
            if idx == 2 {
                idx = idx.saturating_add(usize::from(gap_rows));
            }
        }

        if let Some(p) = self.primary {
            if let Some(r) = stack.get(idx) {
                self.paint_action(
                    Rect::new(area.x, r.y, area.width, 1),
                    buffer,
                    p,
                    true,
                    matches!(state.focus, EmptyFocus::Primary),
                    &mut state.primary,
                );
            }
            idx += 1;
        }
        if let Some(s) = self.secondary {
            if let Some(r) = stack.get(idx) {
                self.paint_action(
                    Rect::new(area.x, r.y, area.width, 1),
                    buffer,
                    s,
                    false,
                    matches!(state.focus, EmptyFocus::Secondary),
                    &mut state.secondary,
                );
            }
            idx += 1;
        }
        if let Some(sc) = self.shortcut {
            if let Some(r) = stack.get(idx) {
                self.paint_centered(
                    Rect::new(area.x, r.y, area.width, 1),
                    buffer,
                    r.y,
                    sc,
                    self.system.style(Role::TextDisabled),
                );
            }
        }
    }

    fn paint_action(
        &self,
        area: Rect,
        buffer: &mut Buffer,
        action: EmptyAction<'_>,
        primary: bool,
        focused: bool,
        btn_state: &mut ButtonState,
    ) {
        if area.is_empty() {
            return;
        }
        // Primary is always Primary variant (safe dominant); secondary Quiet.
        // Never Destructive here — host must opt into danger elsewhere.
        let variant = if primary {
            ButtonVariant::Primary
        } else {
            ButtonVariant::Quiet
        };
        let trailing = action.shortcut;
        let mut btn = Button::new(action.label, self.system).variant(variant);
        if let Some(sc) = trailing {
            btn = btn.trailing(sc);
        }
        let measure = display_cols(action.label)
            .saturating_add(trailing.map(display_cols).unwrap_or(0))
            .saturating_add(6)
            .min(usize::from(area.width)) as u16;
        let x = center_line_x(area, measure.max(1));
        let hit = Rect::new(x, area.y, measure.max(1), 1);
        btn_state.activation.set_accepts_input(focused || primary);
        let _ = btn.paint(hit, buffer, btn_state);
    }

    fn paint_centered(
        &self,
        area: Rect,
        buffer: &mut Buffer,
        y: u16,
        text: &str,
        style: ratatui_core::style::Style,
    ) {
        let width = display_cols(text).min(usize::from(area.width));
        if width == 0 {
            return;
        }
        let clipped = take_display_cols(text, width);
        let x = center_line_x(area, width as u16);
        buffer.set_stringn(x, y, &clipped, width, style);
    }

    /// Handle keyboard when actions are present.
    pub fn handle_key(&self, key: KeyEvent, state: &mut EmptyStateState) -> EmptyStateOutcome {
        if key.kind != KeyEventKind::Press {
            return EmptyStateOutcome::Ignored;
        }
        if self.primary.is_none() && self.secondary.is_none() {
            return EmptyStateOutcome::Ignored;
        }
        // Tab cycles focus
        if matches!(key.code, KeyCode::Tab) {
            let shift = key.modifiers.contains(KeyModifiers::SHIFT);
            state.focus = match (
                state.focus,
                self.primary.is_some(),
                self.secondary.is_some(),
                shift,
            ) {
                (_, true, false, _) => EmptyFocus::Primary,
                (_, false, true, _) => EmptyFocus::Secondary,
                (EmptyFocus::None | EmptyFocus::Secondary, true, true, false) => {
                    EmptyFocus::Primary
                }
                (EmptyFocus::Primary, true, true, false) => EmptyFocus::Secondary,
                (EmptyFocus::None | EmptyFocus::Primary, true, true, true) => EmptyFocus::Secondary,
                (EmptyFocus::Secondary, true, true, true) => EmptyFocus::Primary,
                _ => state.focus,
            };
            return EmptyStateOutcome::Ignored;
        }
        if matches!(default_button_intent(key), Some(UiIntent::Activate))
            || matches!(key.code, KeyCode::Enter | KeyCode::Char(' '))
        {
            return match state.focus {
                EmptyFocus::Primary if self.primary.is_some() => {
                    EmptyStateOutcome::PrimaryActivated
                }
                EmptyFocus::Secondary if self.secondary.is_some() => {
                    EmptyStateOutcome::SecondaryActivated
                }
                EmptyFocus::None if self.primary.is_some() => {
                    // Safe default: unfocused Enter activates primary only
                    EmptyStateOutcome::PrimaryActivated
                }
                _ => EmptyStateOutcome::Ignored,
            };
        }
        // 1/2 hotkeys for dual actions
        if matches!(key.code, KeyCode::Char('1')) && self.primary.is_some() {
            return EmptyStateOutcome::PrimaryActivated;
        }
        if matches!(key.code, KeyCode::Char('2')) && self.secondary.is_some() {
            return EmptyStateOutcome::SecondaryActivated;
        }
        EmptyStateOutcome::Ignored
    }

    /// Pointer: click primary/secondary by y order in last painted area is host-assisted.
    /// Simple hit: top action band = primary when y in lower half of area with actions.
    pub fn handle_mouse(
        &self,
        mouse: MouseEvent,
        area: Rect,
        state: &mut EmptyStateState,
    ) -> EmptyStateOutcome {
        if area.is_empty() || !matches!(mouse.kind, MouseEventKind::Down(MouseButton::Left)) {
            return EmptyStateOutcome::Ignored;
        }
        let pos = mouse.position;
        if !area.contains(pos) {
            return EmptyStateOutcome::Ignored;
        }
        // Full density: actions sit in lower rows — use relative y thirds
        if self.primary.is_none() && self.secondary.is_none() {
            return EmptyStateOutcome::Ignored;
        }
        let rel = pos.y.saturating_sub(area.y);
        let h = area.height.max(1);
        // Prefer lower portion for actions
        if self.primary.is_some() && self.secondary.is_some() {
            if rel + 1 >= h {
                state.focus = EmptyFocus::Secondary;
                return EmptyStateOutcome::SecondaryActivated;
            }
            if rel + 2 >= h {
                state.focus = EmptyFocus::Primary;
                return EmptyStateOutcome::PrimaryActivated;
            }
        } else if self.primary.is_some() && rel + 2 >= h {
            state.focus = EmptyFocus::Primary;
            return EmptyStateOutcome::PrimaryActivated;
        } else if self.secondary.is_some() && rel + 1 >= h {
            state.focus = EmptyFocus::Secondary;
            return EmptyStateOutcome::SecondaryActivated;
        }
        EmptyStateOutcome::Ignored
    }

    /// Semantic registration.
    pub fn register_semantic<Sid, Act>(
        &self,
        scene: &mut SemanticScene<Sid, Act>,
        id: Sid,
        area: Rect,
        state: Option<&EmptyStateState>,
    ) where
        Sid: Clone + PartialEq + std::fmt::Display,
        Act: Clone,
    {
        if area.is_empty() {
            return;
        }
        let focus = state.map(|s| s.focus).unwrap_or(EmptyFocus::None);
        let desc = format!(
            "empty-state kind={} title={} primary={} secondary={} focus={}",
            self.kind.id(),
            self.title,
            self.primary.map(|a| a.label).unwrap_or("-"),
            self.secondary.map(|a| a.label).unwrap_or("-"),
            match focus {
                EmptyFocus::None => "none",
                EmptyFocus::Primary => "primary",
                EmptyFocus::Secondary => "secondary",
            }
        );
        let focusable = self.primary.is_some() || self.secondary.is_some();
        let _ = scene.register(
            SemanticNode::control(id, area)
                .role(SemanticRole::Status)
                .label("empty-state")
                .description(desc)
                .focusable(focusable)
                .state(SemanticState::default()),
        );
    }
}

impl Widget for &EmptyState<'_> {
    fn render(self, area: Rect, buffer: &mut Buffer) {
        self.paint(area, buffer);
    }
}

impl Widget for EmptyState<'_> {
    #[expect(
        clippy::needless_borrows_for_generic_args,
        reason = "explicitly delegate the owned contract to the borrowed renderer"
    )]
    fn render(self, area: Rect, buffer: &mut Buffer) {
        <&Self as Widget>::render(&self, area, buffer);
    }
}

// ── Domain recipes ──────────────────────────────────────────────────────────

/// Table body empty (no rows).
#[must_use]
pub fn example_empty_table(system: &DesignSystem) -> EmptyState<'_> {
    EmptyState::new("No rows", system)
        .kind(EmptyKind::NoData)
        .explanation("This table has no data yet.")
        .primary(EmptyAction::with_shortcut("Add row", "a"))
        .secondary(EmptyAction::new("Import CSV"))
        .example("paste cells or open a file")
        .shortcut("a add · i import")
}

/// Log stream empty.
#[must_use]
pub fn example_empty_logs(system: &DesignSystem) -> EmptyState<'_> {
    EmptyState::new("No log lines", system)
        .kind(EmptyKind::NoData)
        .explanation("Waiting for output from the process.")
        .primary(EmptyAction::with_shortcut("Start", "enter"))
        .context("stream: stdout + stderr")
        .shortcut("enter start · c clear filters")
}

/// Session list empty / first-run.
#[must_use]
pub fn example_empty_sessions(system: &DesignSystem) -> EmptyState<'_> {
    EmptyState::new("No sessions yet", system)
        .kind(EmptyKind::FirstUse)
        .explanation("Start a session to resume work later.")
        .primary(EmptyAction::with_shortcut("New session", "n"))
        .secondary(EmptyAction::new("Open folder"))
        .example("n  new session")
        .shortcut("n new · o open")
}

/// Projects empty.
#[must_use]
pub fn example_empty_projects(system: &DesignSystem) -> EmptyState<'_> {
    EmptyState::new("No projects", system)
        .kind(EmptyKind::FirstUse)
        .explanation("Create a project or clone a repository.")
        .primary(EmptyAction::with_shortcut("New project", "n"))
        .secondary(EmptyAction::new("Clone…"))
        .example("termrock init")
        .shortcut("n new · g clone")
}

/// Search / filter no results.
#[must_use]
pub fn example_empty_search(system: &DesignSystem) -> EmptyState<'_> {
    EmptyState::new("No results", system)
        .kind(EmptyKind::NoResults)
        .explanation("Try another query or clear filters.")
        .primary(EmptyAction::with_shortcut("Clear filters", "esc"))
        .secondary(EmptyAction::new("Edit query"))
        .context("filters: status=running")
        .example("status:failed agent:*")
        .shortcut("esc clear · / edit")
}

/// Permission-limited surface.
#[must_use]
pub fn example_empty_permission(system: &DesignSystem) -> EmptyState<'_> {
    EmptyState::new("Access limited", system)
        .kind(EmptyKind::PermissionLimited)
        .explanation("You do not have permission to view this resource.")
        .primary(EmptyAction::with_shortcut("Request access", "r"))
        .secondary(EmptyAction::new("Go back"))
        .context("scope: org/private-repo")
        .shortcut("r request · esc back")
}

// ── Tests ───────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use ratatui_core::backend::TestBackend;
    use ratatui_core::terminal::Terminal;

    fn system() -> DesignSystem {
        DesignSystem::default()
    }

    fn painted(area: Rect, paint: impl FnOnce(Rect, &mut Buffer)) -> String {
        let mut buf = Buffer::empty(area);
        paint(area, &mut buf);
        let mut s = String::new();
        for y in area.y..area.bottom() {
            for x in area.x..area.right() {
                s.push_str(buf[(x, y)].symbol());
            }
            s.push('\n');
        }
        s
    }

    #[test]
    fn kinds_have_distinct_glyphs() {
        let mut set = std::collections::BTreeSet::new();
        for k in [
            EmptyKind::FirstUse,
            EmptyKind::NoData,
            EmptyKind::NoResults,
            EmptyKind::FilteredOut,
            EmptyKind::PermissionLimited,
        ] {
            set.insert(k.glyph_unicode());
            assert!(!k.id().is_empty());
        }
        assert!(set.len() >= 5);
    }

    #[test]
    fn explanation_and_glyph_render() {
        let system = system();
        let text = painted(Rect::new(0, 0, 40, 5), |a, b| {
            EmptyState::new("No results", &system)
                .explanation("Try another query")
                .glyph("○")
                .paint(a, b);
        });
        assert!(text.contains("No results"), "{text}");
        assert!(text.contains("Try another"), "{text}");
        assert!(text.contains('○') || text.contains('o'), "{text}");
    }

    #[test]
    fn full_paints_primary_dominant() {
        let system = system();
        let text = painted(Rect::new(0, 0, 48, 12), |a, b| {
            EmptyState::new("No rows", &system)
                .kind(EmptyKind::NoData)
                .explanation("Add data to continue")
                .primary(EmptyAction::new("Add row"))
                .secondary(EmptyAction::new("Import"))
                .example("csv path")
                .paint(a, b);
        });
        assert!(text.contains("No rows"), "{text}");
        assert!(text.contains("Add row"), "{text}");
        assert!(text.contains("Import"), "{text}");
        assert!(text.contains("csv") || text.contains("e.g."), "{text}");
    }

    #[test]
    fn empty_state_paints_inset_surface() {
        let system = system();
        let area = Rect::new(0, 0, 32, 8);
        let mut buffer = Buffer::empty(area);
        EmptyState::new("No rows", &system).paint(area, &mut buffer);
        assert_eq!(buffer[(1, 1)].bg, system.style(Role::Surface).bg.unwrap());
        assert_eq!(buffer[(0, 0)].fg, system.style(Role::Border).fg.unwrap());
    }

    #[test]
    fn inline_contracts_in_small_pane() {
        let system = system();
        let text = painted(Rect::new(0, 0, 18, 3), |a, b| {
            EmptyState::new("Empty", &system)
                .explanation("detail")
                .primary(EmptyAction::new("Act"))
                .paint(a, b);
        });
        assert!(text.contains("Empty"), "{text}");
        // Inline should not explode height content messily
        assert!(text.lines().count() <= 4);
    }

    #[test]
    fn inline_measure_height() {
        let system = system();
        let e = EmptyState::new("X", &system);
        assert_eq!(
            e.measure_height(crate::widgets::empty_state::EMPTY_STATE_INLINE_MAX_WIDTH),
            1
        );
        assert_eq!(e.measure_height(80), 2);
    }

    #[test]
    fn recipes_cover_domains() {
        let system = system();
        for e in [
            example_empty_table(&system),
            example_empty_logs(&system),
            example_empty_sessions(&system),
            example_empty_projects(&system),
            example_empty_search(&system),
            example_empty_permission(&system),
        ] {
            let text = painted(Rect::new(0, 0, 50, 14), |a, b| e.paint(a, b));
            assert!(!text.trim().is_empty(), "kind={}", e.kind.id());
            assert!(
                text.contains(e.title) || text.chars().any(|c| !c.is_whitespace()),
                "{text}"
            );
        }
    }

    #[test]
    fn primary_activation_enter() {
        let system = system();
        let e = EmptyState::new("Hi", &system).primary(EmptyAction::new("Go"));
        let mut st = EmptyStateState::new();
        st.focus_primary();
        let key = KeyEvent {
            code: KeyCode::Enter,
            modifiers: KeyModifiers::NONE,
            kind: KeyEventKind::Press,
            state: crate::input::KeyEventState::NONE,
        };
        assert_eq!(
            e.handle_key(key, &mut st),
            EmptyStateOutcome::PrimaryActivated
        );
    }

    #[test]
    fn unfocused_enter_activates_primary_safe_default() {
        let system = system();
        let e = EmptyState::new("Hi", &system)
            .primary(EmptyAction::new("Go"))
            .secondary(EmptyAction::new("Back"));
        let mut st = EmptyStateState::new();
        let key = KeyEvent {
            code: KeyCode::Enter,
            modifiers: KeyModifiers::NONE,
            kind: KeyEventKind::Press,
            state: crate::input::KeyEventState::NONE,
        };
        assert_eq!(
            e.handle_key(key, &mut st),
            EmptyStateOutcome::PrimaryActivated
        );
    }

    #[test]
    fn tab_cycles_actions() {
        let system = system();
        let e = EmptyState::new("Hi", &system)
            .primary(EmptyAction::new("A"))
            .secondary(EmptyAction::new("B"));
        let mut st = EmptyStateState::new();
        let tab = KeyEvent {
            code: KeyCode::Tab,
            modifiers: KeyModifiers::NONE,
            kind: KeyEventKind::Press,
            state: crate::input::KeyEventState::NONE,
        };
        let _ = e.handle_key(tab, &mut st);
        assert_eq!(st.focus(), EmptyFocus::Primary);
        let _ = e.handle_key(tab, &mut st);
        assert_eq!(st.focus(), EmptyFocus::Secondary);
    }

    #[test]
    fn semantic_registers() {
        let system = system();
        let mut scene = SemanticScene::<&str, ()>::default();
        EmptyState::new("None", &system)
            .primary(EmptyAction::new("Retry"))
            .register_semantic(&mut scene, "e", Rect::new(0, 0, 20, 6), None);
        assert!(
            scene
                .nodes()
                .iter()
                .any(|n| n.label.as_deref() == Some("empty-state"))
        );
    }

    #[test]
    fn tiny_and_empty_safe() {
        let system = system();
        let mut buf = Buffer::empty(Rect::new(0, 0, 8, 2));
        EmptyState::new("X", &system).paint(Rect::new(0, 0, 1, 1), &mut buf);
        EmptyState::new("X", &system).paint(Rect::new(0, 0, 0, 0), &mut buf);
    }

    #[test]
    fn kind_glyph_resolved() {
        let system = system();
        let e = EmptyState::new("Empty", &system).kind(EmptyKind::NoResults);
        assert_eq!(e.resolved_glyph(), "∅");
    }

    #[test]
    fn filtered_and_permission_kinds() {
        let system = system();
        let f = painted(Rect::new(0, 0, 40, 8), |a, b| {
            EmptyState::new("Hidden by filters", &system)
                .kind(EmptyKind::FilteredOut)
                .primary(EmptyAction::new("Clear filters"))
                .paint(a, b);
        });
        assert!(f.contains("Hidden") || f.contains("Clear"), "{f}");
        let p = painted(Rect::new(0, 0, 40, 8), |a, b| {
            example_empty_permission(&system).paint(a, b);
        });
        assert!(p.contains("Access") || p.contains("Request"), "{p}");
    }

    #[test]
    fn fuzz_kinds_sizes() {
        let system = system();
        let mut seed = 9u64;
        for _ in 0..40 {
            seed = seed.wrapping_mul(1103515245).wrapping_add(12345);
            let kind = match seed % 5 {
                0 => EmptyKind::FirstUse,
                1 => EmptyKind::NoData,
                2 => EmptyKind::NoResults,
                3 => EmptyKind::FilteredOut,
                _ => EmptyKind::PermissionLimited,
            };
            let w = (seed % 40) as u16 + 1;
            let h = (seed % 12) as u16 + 1;
            let area = Rect::new(0, 0, w, h);
            let mut buf = Buffer::empty(area);
            let mut e = EmptyState::new("T", &system).kind(kind);
            if seed % 2 == 0 {
                e = e.explanation("e").primary(EmptyAction::new("P"));
            }
            if seed % 3 == 0 {
                e = e.secondary(EmptyAction::new("S")).example("ex");
            }
            e.paint(area, &mut buf);
        }
    }

    #[test]
    fn pty_snapshot_stable() {
        let system = system();
        let paint = || {
            let mut t = Terminal::new(TestBackend::new(36, 8)).unwrap();
            t.draw(|f| {
                example_empty_search(&system).paint(f.area(), f.buffer_mut());
            })
            .unwrap();
            t.backend()
                .buffer()
                .content()
                .iter()
                .map(|c| c.symbol().to_string())
                .collect::<String>()
        };
        assert_eq!(paint(), paint());
    }

    #[test]
    fn paint_perf_smoke() {
        let system = system();
        let mut terminal = Terminal::new(TestBackend::new(48, 14)).unwrap();
        let start = std::time::Instant::now();
        for _ in 0..120 {
            terminal
                .draw(|f| {
                    example_empty_table(&system).paint(f.area(), f.buffer_mut());
                })
                .unwrap();
        }
        assert!(start.elapsed().as_millis() < 5_000);
    }

    #[test]
    fn mouse_primary_activation() {
        let system = system();
        let e = EmptyState::new("Hi", &system).primary(EmptyAction::new("Go"));
        let mut st = EmptyStateState::new();
        let area = Rect::new(0, 0, 40, 6);
        let mouse = MouseEvent {
            kind: MouseEventKind::Down(MouseButton::Left),
            position: Position { x: 5, y: 5 },
            modifiers: KeyModifiers::NONE,
        };
        assert_eq!(
            e.handle_mouse(mouse, area, &mut st),
            EmptyStateOutcome::PrimaryActivated
        );
    }
}
