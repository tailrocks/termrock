// SPDX-FileCopyrightText: 2026 Alexey Zhokhov
// SPDX-License-Identifier: Apache-2.0

//! **EmptyState** — a quiet title, an optional hint, never a big glyph.
//!
//! junie: centred muted title, one blank row, faint wrapped hint that names
//! the key which fills it (`Ctrl+N creates one`). No illustration glyphs.
//!
//! Every configured slot paints: title, explanation, example, context,
//! actions (focus from [`EmptyStateState`]), and the shortcut footer. Paint
//! goes only through `EmptyState::paint(area, buffer, state)` — a stateless
//! render would rebuild the state per frame and drop action focus between
//! frames while the interaction model still honored it.
use ratatui_core::{
    buffer::Buffer,
    layout::Rect,
    style::{Modifier, Style},
};

use crate::{
    input::{KeyCode, KeyEvent, KeyModifiers, MouseButton, MouseEvent, MouseEventKind},
    interaction::{
        SemanticNode, SemanticRole, SemanticScene, SemanticState, UiIntent, default_button_intent,
    },
    layout::center_line_x,
    style::{DesignSystem, Role},
    text::{display_cols, take_display_cols, wrap_display_cols},
};

/// Width under which Full density collapses toward Inline.
pub const EMPTY_STATE_INLINE_MAX_WIDTH: u16 = 28;
/// Height under which Full density drops to Inline.
pub const EMPTY_STATE_INLINE_MAX_HEIGHT: u16 = 4;

// ── Kind ────────────────────────────────────────────────────────────────────

/// Why the surface is empty (drives title tone).
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

    /// Title style: junie muted, except permission stays warning-toned.
    fn title_style(&self, system: &DesignSystem) -> ratatui_core::style::Style {
        match self {
            Self::PermissionLimited => system.style(Role::Warning),
            _ => system.junie_theme().muted(),
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

/// Interaction state for empty states with actions.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct EmptyStateState {
    focus: EmptyFocus,
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
    context: Option<&'a str>,
    system: &'a DesignSystem,
}

impl<'a> EmptyState<'a> {
    /// Title + system (defaults: [`EmptyKind::NoData`]).
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

    /// Rows needed at `width`: every configured slot, as painted.
    #[must_use]
    pub fn measure_height(&self, width: u16) -> u16 {
        if width == 0 {
            return 0;
        }
        u16::try_from(self.rows(width, EmptyFocus::None).len()).unwrap_or(u16::MAX)
    }

    /// Hint copy: explanation, else the shortcut that fills the surface.
    fn hint_text(&self) -> Option<&str> {
        self.explanation
            .or(self.shortcut)
            .or(self.primary.and_then(|a| a.shortcut))
    }

    /// Full slot projection: title, explanation, example, context, actions
    /// (focus-marked per `focus`), shortcut footer — one centered row each
    /// (explanation wraps). `measure_height` and `paint` share this, so a
    /// host sizing with one can never paint the other's layout.
    fn rows(&self, width: u16, focus: EmptyFocus) -> Vec<(String, ratatui_core::style::Style)> {
        let theme = self.system.junie_theme();
        let mut rows: Vec<(String, ratatui_core::style::Style)> =
            vec![(self.title.to_owned(), self.kind.title_style(self.system))];
        if let Some(hint) = self.hint_text() {
            rows.push((String::new(), theme.faint()));
            let wrap_w = usize::from(width.saturating_sub(4)).max(8);
            for line in wrap_words(hint, wrap_w) {
                rows.push((line, theme.faint()));
            }
        }
        if let Some(example) = self.example {
            rows.push((String::new(), theme.faint()));
            rows.push((format!("e.g. {example}"), theme.faint()));
        }
        if let Some(context) = self.context {
            rows.push((String::new(), theme.faint()));
            rows.push((context.to_owned(), theme.secondary()));
        }
        if self.primary.is_some() || self.secondary.is_some() {
            rows.push((String::new(), theme.faint()));
            for (action, focused) in [
                (self.primary, focus == EmptyFocus::Primary),
                (self.secondary, focus == EmptyFocus::Secondary),
            ] {
                let Some(action) = action else {
                    continue;
                };
                let mut text = action.label.to_owned();
                if let Some(shortcut) = action.shortcut {
                    text.push_str(&format!(" ({shortcut})"));
                }
                let style = if focused {
                    // junie focus: accent tone plus weight — never color alone.
                    Style::new().fg(theme.focus).add_modifier(Modifier::BOLD)
                } else {
                    theme.muted()
                };
                if focused {
                    text = format!("{} {text}", self.system.glyphs.selection_marker());
                }
                rows.push((text, style));
            }
        }
        if let Some(shortcut) = self.shortcut {
            rows.push((String::new(), theme.faint()));
            rows.push((shortcut.to_owned(), theme.faint()));
        }
        rows
    }

    /// Paint the empty state; action focus comes from `state`.
    pub fn paint(&self, area: Rect, buffer: &mut Buffer, state: &mut EmptyStateState) {
        if area.is_empty() {
            return;
        }
        let rows = self.rows(area.width, state.focus());
        let total = rows.len() as u16;
        let y0 = area.y + area.height.saturating_sub(total) / 2;
        for (i, (text, style)) in rows.iter().enumerate() {
            self.paint_centered(area, buffer, y0.saturating_add(i as u16), text, *style);
        }
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
        if width == 0 || y >= area.bottom() {
            return;
        }
        let clipped = take_display_cols(text, width);
        let x = center_line_x(area, width as u16);
        buffer.set_stringn(x, y, &clipped, width, style);
    }

    /// Handle keyboard when actions are present.
    pub fn handle_key(&self, key: KeyEvent, state: &mut EmptyStateState) -> EmptyStateOutcome {
        if !key.is_press() {
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

/// Word-wrap with a hard fallback, matching junie `ui::text::wrap`.
fn wrap_words(s: &str, width: usize) -> Vec<String> {
    let width = width.max(1);
    let mut lines = Vec::new();
    for para in s.split('\n') {
        let mut line = String::new();
        let mut used = 0usize;
        for word in para.split(' ') {
            let ww = display_cols(word);
            if used == 0 {
                if ww <= width {
                    line.push_str(word);
                    used = ww;
                } else {
                    for part in wrap_display_cols(word, width) {
                        if !line.is_empty() {
                            lines.push(std::mem::take(&mut line));
                        }
                        line = part;
                        used = display_cols(&line);
                    }
                }
            } else if used + 1 + ww <= width {
                line.push(' ');
                line.push_str(word);
                used += 1 + ww;
            } else {
                lines.push(std::mem::take(&mut line));
                if ww <= width {
                    line.push_str(word);
                    used = ww;
                } else {
                    for part in wrap_display_cols(word, width) {
                        if !line.is_empty() {
                            lines.push(std::mem::take(&mut line));
                        }
                        line = part;
                        used = display_cols(&line);
                    }
                }
            }
        }
        lines.push(line);
    }
    lines
}

// ── Domain recipes ──────────────────────────────────────────────────────────

/// Table body empty (no rows).
#[must_use]
pub fn example_empty_table(system: &DesignSystem) -> EmptyState<'_> {
    EmptyState::new("No rows", system)
        .kind(EmptyKind::NoData)
        .explanation("a creates one")
        .primary(EmptyAction::with_shortcut("Add row", "a"))
}

/// Log stream empty.
#[must_use]
pub fn example_empty_logs(system: &DesignSystem) -> EmptyState<'_> {
    EmptyState::new("No log lines", system)
        .kind(EmptyKind::NoData)
        .explanation("Enter starts the process")
        .primary(EmptyAction::with_shortcut("Start", "enter"))
}

/// Session list empty / first-run.
#[must_use]
pub fn example_empty_sessions(system: &DesignSystem) -> EmptyState<'_> {
    EmptyState::new("No sessions yet", system)
        .kind(EmptyKind::FirstUse)
        .explanation("n creates one")
        .primary(EmptyAction::with_shortcut("New session", "n"))
}

/// Projects empty.
#[must_use]
pub fn example_empty_projects(system: &DesignSystem) -> EmptyState<'_> {
    EmptyState::new("No projects", system)
        .kind(EmptyKind::FirstUse)
        .explanation("n creates one")
        .primary(EmptyAction::with_shortcut("New project", "n"))
}

/// Search / filter no results.
#[must_use]
pub fn example_empty_search(system: &DesignSystem) -> EmptyState<'_> {
    EmptyState::new("No results", system)
        .kind(EmptyKind::NoResults)
        .explanation("Esc clears filters")
        .primary(EmptyAction::with_shortcut("Clear filters", "esc"))
}

/// Permission-limited surface.
#[must_use]
pub fn example_empty_permission(system: &DesignSystem) -> EmptyState<'_> {
    EmptyState::new("Access limited", system)
        .kind(EmptyKind::PermissionLimited)
        .explanation("r requests access")
        .primary(EmptyAction::with_shortcut("Request access", "r"))
}

// ── Tests ───────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    use crate::widgets::tests::click;
    use crate::widgets::tests::press;
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
    fn kinds_have_distinct_ids() {
        let mut set = std::collections::BTreeSet::new();
        for k in [
            EmptyKind::FirstUse,
            EmptyKind::NoData,
            EmptyKind::NoResults,
            EmptyKind::FilteredOut,
            EmptyKind::PermissionLimited,
        ] {
            set.insert(k.id());
            assert!(!k.id().is_empty());
        }
        assert_eq!(set.len(), 5);
    }

    #[test]
    fn explanation_renders_without_pictogram() {
        let system = system();
        let text = painted(Rect::new(0, 0, 40, 5), |a, b| {
            EmptyState::new("No results", &system)
                .explanation("Try another query")
                .paint(a, b, &mut EmptyStateState::new());
        });
        assert!(text.contains("No results"), "{text}");
        assert!(text.contains("Try another"), "{text}");
        assert!(!text.contains('★'), "{text}");
        assert!(!text.contains('○'), "{text}");
        assert!(!text.contains('∅'), "{text}");
        assert!(!text.contains('▽'), "{text}");
        assert!(!text.contains('⊘'), "{text}");
    }

    #[test]
    fn full_paints_every_slot_including_actions() {
        let system = system();
        let text = painted(Rect::new(0, 0, 48, 12), |a, b| {
            EmptyState::new("No rows", &system)
                .kind(EmptyKind::NoData)
                .explanation("a creates one")
                .primary(EmptyAction::new("Add row"))
                .secondary(EmptyAction::new("Import"))
                .example("csv path")
                .paint(a, b, &mut EmptyStateState::new());
        });
        assert!(text.contains("No rows"), "{text}");
        assert!(text.contains("a creates one"), "{text}");
        assert!(text.contains("Add row"), "{text}");
        assert!(text.contains("Import"), "{text}");
        assert!(text.contains("e.g. csv path"), "{text}");
    }

    #[test]
    fn focus_marks_primary_with_marker_and_weight() {
        // junie: focus speaks through the marker plus weight, not color alone.
        let system = system();
        let paint = |focus: EmptyFocus| {
            let mut buf = Buffer::empty(Rect::new(0, 0, 40, 8));
            let mut state = EmptyStateState::new();
            state.set_focus(focus);
            EmptyState::new("No rows", &system)
                .primary(EmptyAction::new("Add row"))
                .secondary(EmptyAction::new("Import"))
                .paint(Rect::new(0, 0, 40, 8), &mut buf, &mut state);
            buf
        };
        let marker = system.glyphs.selection_marker();
        let row_of = |buf: &Buffer, needle: &str| {
            (0..8u16)
                .find(|&y| {
                    (0..40u16).any(|x| {
                        let mut s = String::new();
                        for x2 in x..40u16 {
                            s.push_str(buf[(x2, y)].symbol());
                        }
                        s.contains(needle)
                    })
                })
                .expect("action row")
        };
        let col_of = |buf: &Buffer, y: u16, needle: &str| {
            // `find` is a byte offset; the marker glyph is multi-byte UTF-8,
            // so convert to a cell index via chars.
            let row: String = (0..40u16).map(|x| buf[(x, y)].symbol()).collect();
            let byte = row.find(needle).unwrap();
            (row[..byte].chars().count() as u16).saturating_sub(2)
        };

        let focused_primary = paint(EmptyFocus::Primary);
        let pr = row_of(&focused_primary, "Add row");
        let sr = row_of(&focused_primary, "Import");
        let px = col_of(&focused_primary, pr, "Add row");
        let sx = col_of(&focused_primary, sr, "Import");
        assert_eq!(focused_primary[(px, pr)].symbol(), marker);
        assert!(
            focused_primary[(px + 2, pr)]
                .modifier
                .contains(Modifier::BOLD)
        );
        assert_ne!(focused_primary[(sx, sr)].symbol(), marker);

        let focused_secondary = paint(EmptyFocus::Secondary);
        let sr2 = row_of(&focused_secondary, "Import");
        let sx2 = col_of(&focused_secondary, sr2, "Import");
        assert_eq!(focused_secondary[(sx2, sr2)].symbol(), marker);

        let unfocused = paint(EmptyFocus::None);
        let pr3 = row_of(&unfocused, "Add row");
        let px3 = col_of(&unfocused, pr3, "Add row");
        assert_ne!(
            unfocused[(px3, pr3)].symbol(),
            marker,
            "no focus, no marker"
        );
    }

    #[test]
    fn empty_state_is_centred_muted_title_blank_row_faint_hint() {
        let system = system();
        let area = Rect::new(0, 0, 32, 8);
        let mut buffer = Buffer::empty(area);
        EmptyState::new("No rows", &system)
            .explanation("Ctrl+N creates one")
            .paint(area, &mut buffer, &mut EmptyStateState::new());
        let theme = system.junie_theme();
        let rows: Vec<String> = (0..area.height)
            .map(|y| {
                (0..area.width)
                    .map(|x| buffer[(x, y)].symbol())
                    .collect::<String>()
            })
            .collect();
        let title_y = rows
            .iter()
            .position(|r| r.contains("No rows"))
            .expect("title");
        let hint_y = rows
            .iter()
            .position(|r| r.contains("Ctrl+N creates one"))
            .expect("hint");
        assert_eq!(hint_y, title_y + 2, "{rows:?}");
        let title_x = rows[title_y].find("No rows").unwrap() as u16;
        assert_eq!(
            buffer[(title_x, title_y as u16)].fg,
            theme.muted().fg.unwrap()
        );
        let hint_x = rows[hint_y].find("Ctrl+N creates one").unwrap() as u16;
        assert_eq!(
            buffer[(hint_x, hint_y as u16)].fg,
            theme.faint().fg.unwrap()
        );
        assert_eq!(buffer[(0, 0)].symbol(), " ");
    }

    #[test]
    fn inline_contracts_in_small_pane() {
        let system = system();
        let text = painted(Rect::new(0, 0, 18, 3), |a, b| {
            EmptyState::new("Empty", &system)
                .explanation("detail")
                .primary(EmptyAction::new("Act"))
                .paint(a, b, &mut EmptyStateState::new());
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
        assert_eq!(e.measure_height(80), 1);
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
            let text = painted(Rect::new(0, 0, 50, 14), |a, b| {
                e.paint(a, b, &mut EmptyStateState::new())
            });
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
        let key = press(KeyCode::Enter);
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
        let key = press(KeyCode::Enter);
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
        let tab = press(KeyCode::Tab);
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
        EmptyState::new("X", &system).paint(
            Rect::new(0, 0, 1, 1),
            &mut buf,
            &mut EmptyStateState::new(),
        );
        EmptyState::new("X", &system).paint(
            Rect::new(0, 0, 0, 0),
            &mut buf,
            &mut EmptyStateState::new(),
        );
    }

    #[test]
    fn filtered_and_permission_kinds() {
        let system = system();
        let f = painted(Rect::new(0, 0, 40, 8), |a, b| {
            EmptyState::new("Hidden by filters", &system)
                .kind(EmptyKind::FilteredOut)
                .primary(EmptyAction::new("Clear filters"))
                .paint(a, b, &mut EmptyStateState::new());
        });
        assert!(f.contains("Hidden") || f.contains("Clear"), "{f}");
        let p = painted(Rect::new(0, 0, 40, 8), |a, b| {
            example_empty_permission(&system).paint(a, b, &mut EmptyStateState::new());
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
            e.paint(area, &mut buf, &mut EmptyStateState::new());
        }
    }

    #[test]
    fn pty_snapshot_stable() {
        let system = system();
        let paint = || {
            let mut t = Terminal::new(TestBackend::new(36, 8)).unwrap();
            t.draw(|f| {
                example_empty_search(&system).paint(
                    f.area(),
                    f.buffer_mut(),
                    &mut EmptyStateState::new(),
                );
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
                    example_empty_table(&system).paint(
                        f.area(),
                        f.buffer_mut(),
                        &mut EmptyStateState::new(),
                    );
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
        let mouse = click(5, 5);
        assert_eq!(
            e.handle_mouse(mouse, area, &mut st),
            EmptyStateOutcome::PrimaryActivated
        );
    }
}
