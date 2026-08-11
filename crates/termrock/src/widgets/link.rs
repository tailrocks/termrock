// SPDX-FileCopyrightText: 2026 Alexey Zhokhov
// SPDX-License-Identifier: Apache-2.0

//! Link and ActionLink — terminal-safe hyperlinks and inline actions.
//!
//! **Link** — navigation to an external URL (OSC 8 when capable) or an
//! application route. External destinations are **never hidden**: label paint
//! always keeps a visible URL fallback when hyperlinks are off or when the
//! host requests it.
//!
//! **ActionLink** — button-like inline action (no destination URL). Uses link
//! chrome (underline / Link role) but outcomes are application activate only.
//!
//! Consumers own OSC emission via [`crate::osc::encode_hyperlink_open`] /
//! [`crate::osc::Request::HyperlinkOpen`]. This widget produces regions and
//! typed outcomes; it never writes raw OSC bytes to the PTY itself.
//!
//! References: Rich hyperlinks, OSC 8, CLI docs conventions.

use ratatui_core::{buffer::Buffer, layout::Rect, style::Modifier, widgets::Widget};

use crate::input::{KeyEvent, KeyEventKind, MouseButton, MouseEvent, MouseEventKind};
use crate::interaction::{
    EventResult, SemanticNode, SemanticRole, SemanticScene, SemanticState, UiIntent,
    default_button_intent,
};
use crate::osc::{HyperlinkRegion, Request, encode_hyperlink_close, encode_hyperlink_open};
use crate::style::{DesignSystem, Role};
use crate::text::{display_cols, take_display_cols};

// ── Destination ─────────────────────────────────────────────────────────────

/// Where a navigation link goes.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[non_exhaustive]
pub enum LinkDestination<'a> {
    /// External URL (`http`/`https`/`mailto`/`file` — OSC validation is in encode).
    Url(&'a str),
    /// In-app route or resource id (never OSC 8).
    AppRoute(&'a str),
}

impl<'a> LinkDestination<'a> {
    /// Stable kind id.
    #[must_use]
    pub const fn kind_id(self) -> &'static str {
        match self {
            Self::Url(_) => "url",
            Self::AppRoute(_) => "app-route",
        }
    }

    /// Destination string for display / copy / outcome.
    #[must_use]
    pub const fn as_str(self) -> &'a str {
        match self {
            Self::Url(u) | Self::AppRoute(u) => u,
        }
    }

    /// External URL (risk-bearing).
    #[must_use]
    pub const fn is_external(self) -> bool {
        matches!(self, Self::Url(_))
    }

    /// Eligible for OSC 8 (external URL only).
    #[must_use]
    pub const fn osc8_eligible(self) -> bool {
        matches!(self, Self::Url(_))
    }
}

// ── Link ────────────────────────────────────────────────────────────────────

/// Visual density / chrome.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
#[non_exhaustive]
pub enum LinkVariant {
    /// Underlined link text (default).
    #[default]
    Underline,
    /// Brackets around label `[docs]`.
    Bracketed,
    /// Compact bare text (still Link role).
    Plain,
}

impl LinkVariant {
    /// Stable id.
    #[must_use]
    pub const fn id(self) -> &'static str {
        match self {
            Self::Underline => "underline",
            Self::Bracketed => "bracketed",
            Self::Plain => "plain",
        }
    }
}

/// How to show the destination string.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
#[non_exhaustive]
pub enum DestinationDisplay {
    /// External URLs always show destination (risk). App routes hide it.
    #[default]
    Auto,
    /// Always append destination string after the label.
    Always,
    /// Never append destination text; external URLs still get a risk cue.
    Never,
}

impl DestinationDisplay {
    /// Stable id.
    #[must_use]
    pub const fn id(self) -> &'static str {
        match self {
            Self::Auto => "auto",
            Self::Always => "always",
            Self::Never => "never",
        }
    }
}

/// Painted geometry + optional OSC region.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LinkParts {
    /// Full hit target.
    pub root: Rect,
    /// Label band.
    pub label: Rect,
    /// Destination/fallback band (may be empty).
    pub destination: Rect,
    /// Whether OSC 8 open would apply for this paint.
    pub osc8: bool,
}

/// Link outcomes (host applies open / clipboard effects).
///
/// OSC 8 emit is **not** an outcome — use [`Link::osc_requests`] after paint
/// (or `encode_osc_open` / `encode_osc_close`) when the host owns the PTY.
#[derive(Debug, Clone, PartialEq, Eq)]
#[non_exhaustive]
pub enum LinkOutcome {
    /// No change.
    Ignored,
    /// Navigate / open destination.
    Activated {
        /// Destination kind + string.
        destination: String,
        /// External URL risk.
        external: bool,
    },
    /// Copy destination or label to clipboard (host emits OSC 52 if allowed).
    Copy {
        /// Text to copy.
        text: String,
    },
}

/// Interaction + session visited state.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct LinkState {
    /// Keyboard focus.
    pub focused: bool,
    /// Pointer hover.
    pub hovered: bool,
    /// Visited this session.
    pub visited: bool,
    /// Disabled.
    pub disabled: bool,
    /// Last painted parts.
    pub parts: Option<LinkParts>,
}

impl LinkState {
    /// Fresh state.
    #[must_use]
    pub const fn new() -> Self {
        Self {
            focused: false,
            hovered: false,
            visited: false,
            disabled: false,
            parts: None,
        }
    }

    /// Focus.
    pub const fn set_focused(&mut self, on: bool) {
        self.focused = on;
    }

    /// Hover.
    pub const fn set_hovered(&mut self, on: bool) {
        self.hovered = on;
    }

    /// Visited.
    pub const fn set_visited(&mut self, on: bool) {
        self.visited = on;
    }

    /// Disabled.
    pub const fn set_disabled(&mut self, on: bool) {
        self.disabled = on;
    }
}

/// Navigation link (URL or app route).
#[derive(Debug, Clone, Copy)]
pub struct Link<'a> {
    label: &'a str,
    destination: LinkDestination<'a>,
    system: &'a DesignSystem,
    variant: LinkVariant,
    destination_display: DestinationDisplay,
    /// Capability: OSC 8 available this session.
    hyperlinks: bool,
    /// Force external risk marker glyph even when URL shown.
    show_external_cue: bool,
    /// Optional OSC id.
    osc_id: Option<&'a str>,
    /// Max columns for paint (0 = area).
    max_cols: u16,
}

impl<'a> Link<'a> {
    /// Link with external URL destination.
    #[must_use]
    pub const fn url(label: &'a str, url: &'a str, system: &'a DesignSystem) -> Self {
        Self {
            label,
            destination: LinkDestination::Url(url),
            system,
            variant: LinkVariant::Underline,
            destination_display: DestinationDisplay::Auto,
            hyperlinks: false,
            show_external_cue: true,
            osc_id: None,
            max_cols: 0,
        }
    }

    /// Application-routed link (no OSC 8).
    #[must_use]
    pub const fn app_route(label: &'a str, route: &'a str, system: &'a DesignSystem) -> Self {
        Self {
            label,
            destination: LinkDestination::AppRoute(route),
            system,
            variant: LinkVariant::Underline,
            destination_display: DestinationDisplay::Never,
            hyperlinks: false,
            show_external_cue: false,
            osc_id: None,
            max_cols: 0,
        }
    }

    /// Variant chrome.
    #[must_use]
    pub const fn variant(mut self, variant: LinkVariant) -> Self {
        self.variant = variant;
        self
    }

    /// Destination display policy.
    #[must_use]
    pub const fn destination_display(mut self, display: DestinationDisplay) -> Self {
        self.destination_display = display;
        self
    }

    /// Always show destination text.
    #[must_use]
    pub const fn always_show_destination(mut self) -> Self {
        self.destination_display = DestinationDisplay::Always;
        self
    }

    /// Terminal supports OSC 8 this frame.
    #[must_use]
    pub const fn hyperlinks(mut self, on: bool) -> Self {
        self.hyperlinks = on;
        self
    }

    /// External risk cue (↗ / `^`).
    #[must_use]
    pub const fn external_cue(mut self, on: bool) -> Self {
        self.show_external_cue = on;
        self
    }

    /// OSC hyperlink id.
    #[must_use]
    pub const fn osc_id(mut self, id: &'a str) -> Self {
        self.osc_id = Some(id);
        self
    }

    /// Truncation budget.
    #[must_use]
    pub const fn max_cols(mut self, cols: u16) -> Self {
        self.max_cols = cols;
        self
    }

    /// Destination.
    #[must_use]
    pub const fn destination(&self) -> LinkDestination<'a> {
        self.destination
    }

    /// Whether destination string should paint this frame.
    #[must_use]
    pub fn shows_destination(&self) -> bool {
        match self.destination_display {
            DestinationDisplay::Always => true,
            DestinationDisplay::Never => {
                // External links must never hide risk entirely — still show cue.
                false
            }
            DestinationDisplay::Auto => {
                // When hyperlinks on, terminal may hide URL under OSC; we still
                // show destination for external risk unless Never (forced above).
                self.destination.is_external()
            }
        }
    }

    /// Visible painted string (label + optional destination + external cue).
    #[must_use]
    pub fn decorated(&self, state: &LinkState) -> String {
        let _ = state;
        let mut s = match self.variant {
            LinkVariant::Bracketed => format!("[{}]", self.label.trim()),
            LinkVariant::Underline | LinkVariant::Plain => self.label.trim().to_string(),
        };
        if self.shows_destination() {
            let dest = self.destination.as_str();
            if !dest.is_empty() && dest != self.label {
                s.push(' ');
                s.push('(');
                s.push_str(dest);
                s.push(')');
            }
        } else if self.destination.is_external() && self.show_external_cue {
            // Minimal risk cue when full URL hidden under OSC.
            let cue = if self.system.glyphs.is_ascii() {
                " ^"
            } else {
                " ↗"
            };
            s.push_str(cue);
        }
        if self.destination.is_external()
            && self.show_external_cue
            && self.shows_destination()
            && self.system.glyphs.is_ascii()
        {
            // Ensure ASCII risk marker when URL is shown.
            if !s.contains('^') {
                s.push_str(" ^");
            }
        }
        s
    }

    /// Plain text for copy (destination preferred for external).
    #[must_use]
    pub fn copy_text(&self) -> String {
        match self.destination {
            LinkDestination::Url(u) => u.to_string(),
            LinkDestination::AppRoute(r) => {
                if r.is_empty() {
                    self.label.to_string()
                } else {
                    r.to_string()
                }
            }
        }
    }

    /// A11y plain description.
    #[must_use]
    pub fn plain(&self) -> String {
        let kind = self.destination.kind_id();
        format!(
            "{} → {} ({kind}{})",
            self.label.trim(),
            self.destination.as_str(),
            if self.destination.is_external() {
                "; external"
            } else {
                ""
            }
        )
    }

    /// Measure width.
    #[must_use]
    pub fn measure_width(&self, state: &LinkState) -> u16 {
        u16::try_from(display_cols(&self.decorated(state)))
            .unwrap_or(1)
            .max(1)
    }

    fn style(&self, state: &LinkState) -> ratatui_core::style::Style {
        if state.disabled {
            return self.system.style(Role::TextDisabled);
        }
        let mut style = if state.hovered || state.focused {
            self.system.style(Role::LinkHover)
        } else if state.visited {
            // Visited: muted link — use Link with DIM.
            self.system.style(Role::Link).add_modifier(Modifier::DIM)
        } else {
            self.system.style(Role::Link)
        };
        if matches!(self.variant, LinkVariant::Underline) || state.focused {
            style = style.add_modifier(Modifier::UNDERLINED);
        }
        if state.focused {
            style = style.add_modifier(Modifier::BOLD);
        }
        style = ratatui_core::style::Style { bg: None, ..style };
        style
    }

    /// Whether OSC 8 should wrap this paint (host emits bytes).
    #[must_use]
    pub fn uses_osc8(&self, state: &LinkState) -> bool {
        self.hyperlinks
            && !state.disabled
            && self.destination.osc8_eligible()
            && !self.destination.as_str().is_empty()
    }

    /// Build OSC open/close request pair for host emission (empty open = rejected URL).
    #[must_use]
    pub fn osc_requests(&self, state: &LinkState) -> Option<(Request<'_>, Request<'_>)> {
        if !self.uses_osc8(state) {
            return None;
        }
        let LinkDestination::Url(url) = self.destination else {
            return None;
        };
        Some((
            Request::HyperlinkOpen {
                id: self.osc_id,
                url,
            },
            Request::HyperlinkClose,
        ))
    }

    /// Encode OSC open bytes (empty if rejected).
    #[must_use]
    pub fn encode_osc_open(&self, state: &LinkState) -> Vec<u8> {
        if !self.uses_osc8(state) {
            return Vec::new();
        }
        let LinkDestination::Url(url) = self.destination else {
            return Vec::new();
        };
        encode_hyperlink_open(self.osc_id, url)
    }

    /// Encode OSC close bytes.
    #[must_use]
    pub fn encode_osc_close() -> Vec<u8> {
        encode_hyperlink_close()
    }

    /// Hyperlink region for hit testing / host OSC region lists.
    #[must_use]
    pub fn hyperlink_region<Id: Clone>(
        &self,
        id: Id,
        state: &LinkState,
    ) -> Option<HyperlinkRegion<'_, Id>> {
        if !self.uses_osc8(state) {
            return None;
        }
        let LinkDestination::Url(url) = self.destination else {
            return None;
        };
        let area = state.parts.as_ref()?.root;
        Some(HyperlinkRegion { id, area, url })
    }

    /// Layout without paint.
    #[must_use]
    pub fn layout(&self, area: Rect, state: &LinkState) -> LinkParts {
        if area.is_empty() {
            return LinkParts {
                root: area,
                label: area,
                destination: Rect::default(),
                osc8: false,
            };
        }
        let text = self.decorated(state);
        let mut budget = usize::from(area.width);
        if self.max_cols > 0 {
            budget = budget.min(usize::from(self.max_cols));
        }
        let clipped = take_display_cols(&text, budget);
        let w = u16::try_from(display_cols(&clipped))
            .unwrap_or(0)
            .min(area.width);
        let root = Rect {
            x: area.x,
            y: area.y,
            width: w,
            height: 1.min(area.height),
        };
        LinkParts {
            root,
            label: root,
            destination: Rect::default(),
            osc8: self.uses_osc8(state),
        }
    }

    /// Paint link.
    pub fn paint(&self, area: Rect, buffer: &mut Buffer, state: &mut LinkState) -> LinkParts {
        let parts = self.layout(area, state);
        state.parts = Some(parts.clone());
        if parts.root.is_empty() {
            return parts;
        }
        let text = take_display_cols(&self.decorated(state), usize::from(parts.root.width));
        let style = self.style(state);
        buffer.set_stringn(
            parts.root.x,
            parts.root.y,
            &text,
            usize::from(parts.root.width),
            style,
        );
        parts
    }

    /// Activate (Enter / click).
    pub fn handle_key(&self, state: &mut LinkState, key: KeyEvent) -> LinkOutcome {
        if state.disabled || !state.focused || key.kind != KeyEventKind::Press {
            return LinkOutcome::Ignored;
        }
        if let Some(intent) = default_button_intent(key) {
            return self.handle_intent(state, intent);
        }
        // 'c' copy destination when focused (common CLI help convention)
        if matches!(key.code, crate::input::KeyCode::Char('c' | 'C')) && key.modifiers.is_empty() {
            return LinkOutcome::Copy {
                text: self.copy_text(),
            };
        }
        LinkOutcome::Ignored
    }

    /// Intent path.
    pub fn handle_intent(&self, state: &mut LinkState, intent: UiIntent) -> LinkOutcome {
        if state.disabled || !state.focused {
            return LinkOutcome::Ignored;
        }
        match intent {
            UiIntent::Activate | UiIntent::Submit => self.activate(state),
            _ => LinkOutcome::Ignored,
        }
    }

    fn activate(&self, state: &mut LinkState) -> LinkOutcome {
        state.visited = true;
        LinkOutcome::Activated {
            destination: self.destination.as_str().to_string(),
            external: self.destination.is_external(),
        }
    }

    /// Mouse: hover update + activate on click.
    pub fn handle_mouse(&self, state: &mut LinkState, event: MouseEvent) -> LinkOutcome {
        if state.disabled {
            return LinkOutcome::Ignored;
        }
        let Some(parts) = &state.parts else {
            return LinkOutcome::Ignored;
        };
        let hit = parts.root.contains(event.position);
        match event.kind {
            MouseEventKind::Moved | MouseEventKind::Drag(_) => {
                state.hovered = hit;
                LinkOutcome::Ignored
            }
            MouseEventKind::Down(MouseButton::Left) if hit => {
                state.focused = true;
                state.hovered = true;
                self.activate(state)
            }
            _ => LinkOutcome::Ignored,
        }
    }

    /// EventResult wrapper.
    pub fn handle_key_result(
        &self,
        state: &mut LinkState,
        key: KeyEvent,
    ) -> EventResult<LinkOutcome> {
        match self.handle_key(state, key) {
            LinkOutcome::Ignored => EventResult::ignored(),
            other => EventResult::emit(other),
        }
    }

    /// Semantic registration.
    pub fn register_semantic<Id, Action>(
        &self,
        scene: &mut SemanticScene<Id, Action>,
        id: Id,
        area: Rect,
        state: &LinkState,
    ) where
        Id: Clone + PartialEq + std::fmt::Display,
        Action: Clone,
    {
        let parts = self.layout(area, state);
        if parts.root.is_empty() {
            return;
        }
        let desc = self.plain();
        let _ = scene.register(
            SemanticNode::control(id, parts.root)
                .role(SemanticRole::Button)
                .label(self.label)
                .description(desc)
                .focusable(!state.disabled)
                .state(SemanticState {
                    selected: state.focused,
                    ..Default::default()
                }),
        );
    }
}

impl Widget for &Link<'_> {
    fn render(self, area: Rect, buffer: &mut Buffer) {
        let mut state = LinkState::new();
        let _ = self.paint(area, buffer, &mut state);
    }
}

impl Widget for Link<'_> {
    fn render(self, area: Rect, buffer: &mut Buffer) {
        <&Self as Widget>::render(&self, area, buffer);
    }
}

// ── ActionLink ──────────────────────────────────────────────────────────────

/// ActionLink outcomes (no external URL).
#[derive(Debug, Clone, PartialEq, Eq)]
#[non_exhaustive]
pub enum ActionLinkOutcome {
    /// Ignored.
    Ignored,
    /// Activated.
    Activated,
}

/// Lightweight inline action with link chrome (not a navigation destination).
#[derive(Debug, Clone, Copy)]
pub struct ActionLink<'a> {
    label: &'a str,
    system: &'a DesignSystem,
    variant: LinkVariant,
    /// Optional risk / scope note always visible when set (e.g. "runs cargo").
    risk_note: Option<&'a str>,
}

impl<'a> ActionLink<'a> {
    /// Inline action label.
    #[must_use]
    pub const fn new(label: &'a str, system: &'a DesignSystem) -> Self {
        Self {
            label,
            system,
            variant: LinkVariant::Underline,
            risk_note: None,
        }
    }

    /// Variant.
    #[must_use]
    pub const fn variant(mut self, variant: LinkVariant) -> Self {
        self.variant = variant;
        self
    }

    /// Visible risk / scope note (never hidden).
    #[must_use]
    pub const fn risk_note(mut self, note: &'a str) -> Self {
        self.risk_note = Some(note);
        self
    }

    /// Label.
    #[must_use]
    pub const fn label(&self) -> &'a str {
        self.label
    }

    fn decorated(&self) -> String {
        let mut s = match self.variant {
            LinkVariant::Bracketed => format!("[{}]", self.label.trim()),
            _ => self.label.trim().to_string(),
        };
        if let Some(note) = self.risk_note {
            if !note.is_empty() {
                s.push(' ');
                s.push('(');
                s.push_str(note);
                s.push(')');
            }
        }
        s
    }

    /// Plain a11y.
    #[must_use]
    pub fn plain(&self) -> String {
        match self.risk_note {
            Some(n) if !n.is_empty() => format!("action: {} ({n})", self.label),
            _ => format!("action: {}", self.label),
        }
    }

    /// Paint.
    pub fn paint(&self, area: Rect, buffer: &mut Buffer, state: &mut LinkState) -> LinkParts {
        // Action links never use OSC 8.
        let text = self.decorated();
        if area.is_empty() {
            let parts = LinkParts {
                root: area,
                label: area,
                destination: Rect::default(),
                osc8: false,
            };
            state.parts = Some(parts.clone());
            return parts;
        }
        let clipped = take_display_cols(&text, usize::from(area.width));
        let w = u16::try_from(display_cols(&clipped))
            .unwrap_or(0)
            .min(area.width);
        let root = Rect {
            x: area.x,
            y: area.y,
            width: w,
            height: 1.min(area.height),
        };
        let mut style = if state.disabled {
            self.system.style(Role::TextDisabled)
        } else if state.hovered || state.focused {
            self.system.style(Role::LinkHover)
        } else {
            self.system.style(Role::Link)
        };
        if matches!(self.variant, LinkVariant::Underline) || state.focused {
            style = style.add_modifier(Modifier::UNDERLINED);
        }
        if state.focused {
            style = style.add_modifier(Modifier::BOLD);
        }
        style = ratatui_core::style::Style { bg: None, ..style };
        buffer.set_stringn(root.x, root.y, &clipped, usize::from(root.width), style);
        let parts = LinkParts {
            root,
            label: root,
            destination: Rect::default(),
            osc8: false,
        };
        state.parts = Some(parts.clone());
        parts
    }

    /// Keys.
    pub fn handle_key(&self, state: &mut LinkState, key: KeyEvent) -> ActionLinkOutcome {
        if state.disabled || !state.focused || key.kind != KeyEventKind::Press {
            return ActionLinkOutcome::Ignored;
        }
        if default_button_intent(key)
            .is_some_and(|i| matches!(i, UiIntent::Activate | UiIntent::Submit))
        {
            return ActionLinkOutcome::Activated;
        }
        ActionLinkOutcome::Ignored
    }

    /// Mouse click.
    pub fn handle_mouse(&self, state: &mut LinkState, event: MouseEvent) -> ActionLinkOutcome {
        if state.disabled {
            return ActionLinkOutcome::Ignored;
        }
        let Some(parts) = &state.parts else {
            return ActionLinkOutcome::Ignored;
        };
        let hit = parts.root.contains(event.position);
        if matches!(event.kind, MouseEventKind::Moved | MouseEventKind::Drag(_)) {
            state.hovered = hit;
            return ActionLinkOutcome::Ignored;
        }
        if event.kind == MouseEventKind::Down(MouseButton::Left) && hit {
            state.focused = true;
            return ActionLinkOutcome::Activated;
        }
        ActionLinkOutcome::Ignored
    }

    /// Semantic.
    pub fn register_semantic<Id, Action>(
        &self,
        scene: &mut SemanticScene<Id, Action>,
        id: Id,
        area: Rect,
        state: &LinkState,
    ) where
        Id: Clone + PartialEq + std::fmt::Display,
        Action: Clone,
    {
        let text = self.decorated();
        let w = u16::try_from(display_cols(&text))
            .unwrap_or(1)
            .min(area.width);
        let root = Rect {
            x: area.x,
            y: area.y,
            width: w,
            height: 1.min(area.height),
        };
        if root.is_empty() {
            return;
        }
        let _ = scene.register(
            SemanticNode::control(id, root)
                .role(SemanticRole::Button)
                .label(self.label)
                .description(self.plain())
                .focusable(!state.disabled),
        );
    }
}

impl Widget for &ActionLink<'_> {
    fn render(self, area: Rect, buffer: &mut Buffer) {
        let mut state = LinkState::new();
        let _ = self.paint(area, buffer, &mut state);
    }
}

impl Widget for ActionLink<'_> {
    fn render(self, area: Rect, buffer: &mut Buffer) {
        <&Self as Widget>::render(&self, area, buffer);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::input::{KeyCode, KeyModifiers};
    use crate::style::GlyphSet;

    #[test]
    fn external_always_shows_destination_or_cue() {
        let system = DesignSystem::default();
        let link = Link::url("docs", "https://example.invalid/path", &system);
        let state = LinkState::new();
        let d = link.decorated(&state);
        assert!(
            d.contains("example.invalid") || d.contains('↗'),
            "must not hide external dest: {d}"
        );
    }

    #[test]
    fn no_hyperlink_shows_url() {
        let system = DesignSystem::default();
        let link = Link::url("docs", "https://example.invalid", &system).hyperlinks(false);
        let state = LinkState::new();
        assert!(link.decorated(&state).contains("example.invalid"));
        assert!(!link.uses_osc8(&state));
    }

    #[test]
    fn hyperlinks_on_still_shows_external_url_by_default() {
        let system = DesignSystem::default();
        let link = Link::url("docs", "https://example.invalid", &system).hyperlinks(true);
        let state = LinkState::new();
        // Auto policy still shows URL for external risk.
        assert!(link.shows_destination());
        assert!(link.uses_osc8(&state));
    }

    #[test]
    fn app_route_not_osc8() {
        let system = DesignSystem::default();
        let link = Link::app_route("Settings", "app://settings", &system).hyperlinks(true);
        let state = LinkState::new();
        assert!(!link.uses_osc8(&state));
        assert_eq!(link.encode_osc_open(&state).len(), 0);
    }

    #[test]
    fn osc_open_rejects_javascript() {
        let system = DesignSystem::default();
        let link = Link::url("x", "javascript:alert(1)", &system).hyperlinks(true);
        let state = LinkState::new();
        assert!(link.encode_osc_open(&state).is_empty());
    }

    #[test]
    fn osc_open_encodes_https() {
        let system = DesignSystem::default();
        let link = Link::url("x", "https://example.invalid", &system)
            .hyperlinks(true)
            .osc_id("docs");
        let state = LinkState::new();
        let bytes = link.encode_osc_open(&state);
        assert!(!bytes.is_empty());
        let s = String::from_utf8_lossy(&bytes);
        assert!(s.contains("https://example.invalid"));
    }

    #[test]
    fn activate_marks_visited() {
        let system = DesignSystem::default();
        let link = Link::url("x", "https://example.invalid", &system);
        let mut state = LinkState::new();
        state.set_focused(true);
        let out = link.handle_key(
            &mut state,
            KeyEvent::new(KeyCode::Enter, KeyModifiers::NONE),
        );
        assert!(matches!(out, LinkOutcome::Activated { external: true, .. }));
        assert!(state.visited);
    }

    #[test]
    fn copy_chord() {
        let system = DesignSystem::default();
        let link = Link::url("x", "https://example.invalid", &system);
        let mut state = LinkState::new();
        state.set_focused(true);
        let out = link.handle_key(
            &mut state,
            KeyEvent::new(KeyCode::Char('c'), KeyModifiers::NONE),
        );
        assert!(matches!(out, LinkOutcome::Copy { .. }));
    }

    #[test]
    fn disabled_ignores() {
        let system = DesignSystem::default();
        let link = Link::url("x", "https://example.invalid", &system);
        let mut state = LinkState::new();
        state.set_focused(true);
        state.set_disabled(true);
        assert_eq!(
            link.handle_key(
                &mut state,
                KeyEvent::new(KeyCode::Enter, KeyModifiers::NONE)
            ),
            LinkOutcome::Ignored
        );
    }

    #[test]
    fn action_link_not_navigation() {
        let system = DesignSystem::default();
        let action = ActionLink::new("Run tests", &system).risk_note("cargo test");
        assert!(action.plain().contains("action:"));
        assert!(action.decorated().contains("cargo test"));
        let mut state = LinkState::new();
        state.set_focused(true);
        assert_eq!(
            action.handle_key(
                &mut state,
                KeyEvent::new(KeyCode::Enter, KeyModifiers::NONE)
            ),
            ActionLinkOutcome::Activated
        );
    }

    #[test]
    fn ascii_external_cue() {
        let system = DesignSystem::default().glyphs(GlyphSet::Ascii);
        let link = Link::url("docs", "https://example.invalid", &system)
            .destination_display(DestinationDisplay::Never);
        let state = LinkState::new();
        let d = link.decorated(&state);
        assert!(d.contains('^'), "{d}");
    }

    #[test]
    fn no_color_still_underlines_when_focused() {
        let system = DesignSystem::default();
        let link = Link::url("x", "https://example.invalid", &system);
        let mut state = LinkState::new();
        state.set_focused(true);
        let style = link.style(&state);
        assert!(style.add_modifier.contains(Modifier::UNDERLINED));
    }

    #[test]
    fn paint_and_layout_cheap() {
        let system = DesignSystem::default();
        let link = Link::url("docs", "https://example.invalid/a/b", &system);
        let state = LinkState::new();
        for _ in 0..20_000 {
            let _ = link.layout(Rect::new(0, 0, 40, 1), &state);
            let _ = link.decorated(&state);
        }
    }

    #[test]
    fn empty_area_safe() {
        let system = DesignSystem::default();
        let mut buf = Buffer::empty(Rect::new(0, 0, 1, 1));
        let mut state = LinkState::new();
        let parts = Link::url("x", "https://example.invalid", &system).paint(
            Rect::new(0, 0, 0, 0),
            &mut buf,
            &mut state,
        );
        assert!(parts.root.is_empty());
    }

    #[test]
    fn hyperlink_region_when_osc8() {
        let system = DesignSystem::default();
        let link = Link::url("x", "https://example.invalid", &system).hyperlinks(true);
        let mut state = LinkState::new();
        let mut buf = Buffer::empty(Rect::new(0, 0, 40, 1));
        let _ = link.paint(Rect::new(0, 0, 40, 1), &mut buf, &mut state);
        let region = link.hyperlink_region("id", &state);
        assert!(region.is_some());
        assert_eq!(region.unwrap().url, "https://example.invalid");
    }

    #[test]
    fn mouse_click_activates() {
        use crate::input::{MouseButton, MouseEvent, MouseEventKind};
        use ratatui_core::layout::Position;

        let system = DesignSystem::default();
        let link = Link::url("docs", "https://example.invalid", &system);
        let mut state = LinkState::new();
        let mut buf = Buffer::empty(Rect::new(0, 0, 40, 1));
        let _ = link.paint(Rect::new(0, 0, 40, 1), &mut buf, &mut state);
        let out = link.handle_mouse(
            &mut state,
            MouseEvent {
                kind: MouseEventKind::Down(MouseButton::Left),
                position: Position { x: 1, y: 0 },
                modifiers: KeyModifiers::NONE,
            },
        );
        assert!(matches!(out, LinkOutcome::Activated { external: true, .. }));
        assert!(state.visited);
        assert!(state.focused);
    }

    #[test]
    fn narrow_truncates_without_panic() {
        let system = DesignSystem::default();
        let link = Link::url(
            "documentation",
            "https://example.invalid/long/path",
            &system,
        );
        let mut state = LinkState::new();
        let mut buf = Buffer::empty(Rect::new(0, 0, 12, 1));
        let parts = link.paint(Rect::new(0, 0, 8, 1), &mut buf, &mut state);
        assert!(parts.root.width <= 8);
    }

    #[test]
    fn unicode_label_measures() {
        let system = DesignSystem::default();
        let link = Link::url("文档 🔗", "https://example.invalid", &system);
        let state = LinkState::new();
        assert!(link.measure_width(&state) >= 2);
        let d = link.decorated(&state);
        assert!(d.contains('文') || d.contains("example"));
    }

    #[test]
    fn osc_requests_pair_when_eligible() {
        let system = DesignSystem::default();
        let link = Link::url("x", "https://example.invalid", &system)
            .hyperlinks(true)
            .osc_id("r1");
        let state = LinkState::new();
        let pair = link.osc_requests(&state).expect("pair");
        assert!(matches!(
            pair.0,
            Request::HyperlinkOpen {
                id: Some("r1"),
                url: "https://example.invalid"
            }
        ));
        assert_eq!(pair.1, Request::HyperlinkClose);
    }
}
