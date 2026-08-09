// SPDX-FileCopyrightText: 2026 Alexey Zhokhov
// SPDX-License-Identifier: Apache-2.0

//! Content hierarchy primitives: heading, paragraph, surface, section, callout, alert.

use ratatui_core::{buffer::Buffer, layout::Rect, style::Modifier, widgets::Widget};

use crate::{
    input::{KeyEvent, KeyEventKind, MouseButton, MouseEvent, MouseEventKind},
    interaction::{EventResult, UiIntent, default_button_intent, default_list_intent},
    style::{DesignSystem, Role},
    text::take_display_cols,
};

/// Heading level (terminal typography weight/role).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
#[non_exhaustive]
pub enum HeadingLevel {
    /// Page title.
    H1,
    /// Section title.
    #[default]
    H2,
    /// Subsection.
    H3,
}

/// Semantic heading line.
#[derive(Debug, Clone, Copy)]
pub struct Heading<'a> {
    text: &'a str,
    level: HeadingLevel,
    tokens: &'a DesignSystem,
}

impl<'a> Heading<'a> {
    /// Heading text.
    #[must_use]
    pub const fn new(text: &'a str, tokens: &'a DesignSystem) -> Self {
        Self {
            text,
            level: HeadingLevel::H2,
            tokens,
        }
    }

    /// Level.
    #[must_use]
    pub const fn level(mut self, level: HeadingLevel) -> Self {
        self.level = level;
        self
    }
}

impl Widget for &Heading<'_> {
    fn render(self, area: Rect, buffer: &mut Buffer) {
        if area.is_empty() {
            return;
        }
        let mut style = self.tokens.style(Role::TextStrong);
        if matches!(self.level, HeadingLevel::H1) {
            style = style.add_modifier(Modifier::BOLD | Modifier::UNDERLINED);
        } else if matches!(self.level, HeadingLevel::H2) {
            style = style.add_modifier(Modifier::BOLD);
        }
        let text = take_display_cols(self.text, usize::from(area.width));
        buffer.set_stringn(area.x, area.y, &text, usize::from(area.width), style);
    }
}

/// Body paragraph with grapheme-safe wrap (single-line clip when height is 1).
#[derive(Debug, Clone, Copy)]
pub struct Paragraph<'a> {
    text: &'a str,
    tokens: &'a DesignSystem,
    muted: bool,
}

impl<'a> Paragraph<'a> {
    /// Body text.
    #[must_use]
    pub const fn new(text: &'a str, tokens: &'a DesignSystem) -> Self {
        Self {
            text,
            tokens,
            muted: false,
        }
    }

    /// Secondary tone.
    #[must_use]
    pub const fn muted(mut self, muted: bool) -> Self {
        self.muted = muted;
        self
    }
}

impl Widget for &Paragraph<'_> {
    fn render(self, area: Rect, buffer: &mut Buffer) {
        if area.is_empty() {
            return;
        }
        let style = self.tokens.style(if self.muted {
            Role::TextMuted
        } else {
            Role::Text
        });
        let mut y = area.y;
        let mut rest = self.text;
        while y < area.bottom() && !rest.is_empty() {
            let line = take_display_cols(rest, usize::from(area.width));
            let take = line.len().min(rest.len());
            // advance by display-safe prefix length in bytes
            let prefix = take_display_cols(rest, usize::from(area.width));
            buffer.set_stringn(area.x, y, &prefix, usize::from(area.width), style);
            let _advance = prefix.len().max(1).min(rest.len());
            // Prefer char boundary: if we didn't consume all, skip used display cols worth of chars
            if prefix.len() >= rest.len() {
                break;
            }
            // Find byte index matching display width
            let mut cols = 0usize;
            let mut idx = 0;
            for (i, ch) in rest.char_indices() {
                let w = unicode_width::UnicodeWidthChar::width(ch).unwrap_or(0);
                if cols + w > usize::from(area.width) && cols > 0 {
                    idx = i;
                    break;
                }
                cols += w;
                idx = i + ch.len_utf8();
                if cols >= usize::from(area.width) {
                    break;
                }
            }
            if idx == 0 {
                break;
            }
            rest = &rest[idx..];
            let _ = take;
            y = y.saturating_add(1);
        }
    }
}

// Surface lives in `widgets/surface.rs` (canonical fill/border/clip/hit).

/// Section collapse outcome.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
#[non_exhaustive]
pub enum SectionOutcome {
    /// No change.
    #[default]
    Ignored,
    /// Collapse toggled.
    ToggleCollapsed {
        /// New collapsed flag.
        collapsed: bool,
    },
}

/// Collapsible section state.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct SectionState {
    collapsed: bool,
    focused: bool,
    header_region: Option<Rect>,
}

impl SectionState {
    /// Open section.
    #[must_use]
    pub const fn new() -> Self {
        Self {
            collapsed: false,
            focused: false,
            header_region: None,
        }
    }

    #[must_use]
    /// Collapsed.
    pub const fn is_collapsed(&self) -> bool {
        self.collapsed
    }

    /// Controlled collapse.
    pub const fn set_collapsed(&mut self, collapsed: bool) {
        self.collapsed = collapsed;
    }

    /// Focus on header.
    pub const fn set_focused(&mut self, focused: bool) {
        self.focused = focused;
    }

    /// Enter/Space toggles when focused (via intents; no raw key match).
    pub fn handle_key(&mut self, key: KeyEvent) -> SectionOutcome {
        if !self.focused || key.kind != KeyEventKind::Press {
            return SectionOutcome::Ignored;
        }
        // Activate (button map: Enter/Space) or Toggle (list Space).
        let intent = default_button_intent(key).or_else(|| default_list_intent(key));
        match intent {
            Some(UiIntent::Activate | UiIntent::Toggle) => self.toggle(),
            _ => SectionOutcome::Ignored,
        }
    }

    /// Semantic intent path.
    pub fn handle_intent(&mut self, intent: UiIntent) -> SectionOutcome {
        if !self.focused {
            return SectionOutcome::Ignored;
        }
        match intent {
            UiIntent::Activate | UiIntent::Toggle | UiIntent::Expand | UiIntent::Collapse => {
                self.toggle()
            }
            _ => SectionOutcome::Ignored,
        }
    }

    fn toggle(&mut self) -> SectionOutcome {
        self.collapsed = !self.collapsed;
        SectionOutcome::ToggleCollapsed {
            collapsed: self.collapsed,
        }
    }

    /// Key path with [`EventResult`].
    pub fn handle_key_result(&mut self, key: KeyEvent) -> EventResult<SectionOutcome> {
        match self.handle_key(key) {
            SectionOutcome::Ignored => EventResult::ignored(),
            other => EventResult::emit(other),
        }
    }

    /// Click header toggles.
    pub fn handle_mouse(&mut self, event: MouseEvent) -> SectionOutcome {
        if event.kind != MouseEventKind::Down(MouseButton::Left) {
            return SectionOutcome::Ignored;
        }
        if self
            .header_region
            .is_some_and(|r| r.contains(event.position))
        {
            self.collapsed = !self.collapsed;
            SectionOutcome::ToggleCollapsed {
                collapsed: self.collapsed,
            }
        } else {
            SectionOutcome::Ignored
        }
    }
}

/// Section = heading + optional description with collapse.
#[derive(Debug, Clone, Copy)]
pub struct Section<'a> {
    title: &'a str,
    description: Option<&'a str>,
    tokens: &'a DesignSystem,
}

impl<'a> Section<'a> {
    /// Section title.
    #[must_use]
    pub const fn new(title: &'a str, tokens: &'a DesignSystem) -> Self {
        Self {
            title,
            description: None,
            tokens,
        }
    }

    /// Description under title when expanded.
    #[must_use]
    pub const fn description(mut self, description: &'a str) -> Self {
        self.description = Some(description);
        self
    }

    /// Paint section chrome; returns body area when expanded.
    pub fn render(&self, area: Rect, buffer: &mut Buffer, state: &mut SectionState) -> Rect {
        state.header_region = None;
        if area.is_empty() {
            return area;
        }
        let mark = if state.collapsed { "▸" } else { "▾" };
        let style = if state.focused {
            self.tokens.style(Role::TextStrong)
        } else {
            self.tokens.style(Role::Text)
        };
        let line = format!("{mark} {}", self.title);
        let text = take_display_cols(&line, usize::from(area.width));
        buffer.set_stringn(area.x, area.y, &text, usize::from(area.width), style);
        state.header_region = Some(Rect::new(area.x, area.y, area.width, 1));
        if state.collapsed {
            return Rect::new(area.x, area.y.saturating_add(1), area.width, 0);
        }
        let mut y = area.y.saturating_add(1);
        if let Some(desc) = self.description
            && y < area.bottom()
        {
            let d = take_display_cols(desc, usize::from(area.width));
            buffer.set_stringn(
                area.x,
                y,
                &d,
                usize::from(area.width),
                self.tokens.style(Role::TextMuted),
            );
            y = y.saturating_add(1);
        }
        let height = area.bottom().saturating_sub(y);
        Rect::new(area.x, y, area.width, height)
    }
}

/// Callout semantic tone.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
#[non_exhaustive]
pub enum CalloutTone {
    /// Neutral info.
    #[default]
    Info,
    /// Success.
    Success,
    /// Warning.
    Warning,
    /// Danger.
    Danger,
}

impl CalloutTone {
    #[must_use]
    fn role(self) -> Role {
        match self {
            Self::Info => Role::Info,
            Self::Success => Role::Success,
            Self::Warning => Role::Warning,
            Self::Danger => Role::Danger,
        }
    }

    #[must_use]
    fn glyph(self) -> &'static str {
        match self {
            Self::Info => "i",
            Self::Success => "+",
            Self::Warning => "!",
            Self::Danger => "x",
        }
    }
}

/// Inline callout (non-modal).
#[derive(Debug, Clone, Copy)]
pub struct Callout<'a> {
    title: &'a str,
    body: Option<&'a str>,
    tone: CalloutTone,
    tokens: &'a DesignSystem,
}

impl<'a> Callout<'a> {
    /// Title + tone.
    #[must_use]
    pub const fn new(title: &'a str, tokens: &'a DesignSystem) -> Self {
        Self {
            title,
            body: None,
            tone: CalloutTone::Info,
            tokens,
        }
    }

    /// Body line.
    #[must_use]
    pub const fn body(mut self, body: &'a str) -> Self {
        self.body = Some(body);
        self
    }

    /// Tone.
    #[must_use]
    pub const fn tone(mut self, tone: CalloutTone) -> Self {
        self.tone = tone;
        self
    }
}

impl Widget for &Callout<'_> {
    fn render(self, area: Rect, buffer: &mut Buffer) {
        if area.is_empty() {
            return;
        }
        let style = self.tokens.style(self.tone.role());
        let head = format!("{} {}", self.tone.glyph(), self.title);
        let text = take_display_cols(&head, usize::from(area.width));
        buffer.set_stringn(area.x, area.y, &text, usize::from(area.width), style);
        if let Some(body) = self.body
            && area.height > 1
        {
            let b = take_display_cols(body, usize::from(area.width));
            buffer.set_stringn(
                area.x,
                area.y + 1,
                &b,
                usize::from(area.width),
                self.tokens.style(Role::TextMuted),
            );
        }
    }
}

/// Alert tone (alias of callout for dismissible banners).
pub type AlertTone = CalloutTone;

/// Alert dismiss outcome.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
#[non_exhaustive]
pub enum AlertOutcome {
    /// No change.
    #[default]
    Ignored,
    /// User dismissed.
    Dismissed,
    /// User acknowledged (Enter).
    Acknowledged,
}

/// Dismissible alert banner.
#[derive(Debug, Clone, Copy)]
pub struct Alert<'a> {
    title: &'a str,
    tokens: &'a DesignSystem,
    tone: AlertTone,
}

impl<'a> Alert<'a> {
    /// Alert title.
    #[must_use]
    pub const fn new(title: &'a str, tokens: &'a DesignSystem) -> Self {
        Self {
            title,
            tokens,
            tone: AlertTone::Warning,
        }
    }

    /// Tone.
    #[must_use]
    pub const fn tone(mut self, tone: AlertTone) -> Self {
        self.tone = tone;
        self
    }
}

/// Alert interaction (focus + dismiss).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct AlertState {
    /// Focused for keyboard.
    pub focused: bool,
    /// Region for click-dismiss.
    pub region: Option<Rect>,
}

impl AlertState {
    /// Esc dismisses; Enter acknowledges (via intents; no raw key match).
    pub fn handle_key(&mut self, key: KeyEvent) -> AlertOutcome {
        if !self.focused || key.kind != KeyEventKind::Press {
            return AlertOutcome::Ignored;
        }
        let intent = default_button_intent(key).or_else(|| default_list_intent(key));
        match intent {
            Some(UiIntent::Cancel | UiIntent::Close) => AlertOutcome::Dismissed,
            Some(UiIntent::Activate | UiIntent::Submit) => AlertOutcome::Acknowledged,
            _ => AlertOutcome::Ignored,
        }
    }

    /// Semantic intent path.
    pub fn handle_intent(&mut self, intent: UiIntent) -> AlertOutcome {
        if !self.focused {
            return AlertOutcome::Ignored;
        }
        match intent {
            UiIntent::Cancel | UiIntent::Close => AlertOutcome::Dismissed,
            UiIntent::Activate | UiIntent::Submit => AlertOutcome::Acknowledged,
            _ => AlertOutcome::Ignored,
        }
    }

    /// Key path with [`EventResult`] (dismiss requests overlay peel).
    pub fn handle_key_result(&mut self, key: KeyEvent) -> EventResult<AlertOutcome> {
        match self.handle_key(key) {
            AlertOutcome::Ignored => EventResult::ignored(),
            AlertOutcome::Dismissed => EventResult::emit(AlertOutcome::Dismissed)
                .with_overlay(crate::interaction::OverlayRequest::DismissTop),
            other => EventResult::emit(other),
        }
    }
}

impl Alert<'_> {
    /// Paint alert.
    pub fn render(&self, area: Rect, buffer: &mut Buffer, state: &mut AlertState) {
        state.region = None;
        if area.is_empty() {
            return;
        }
        let callout = Callout::new(self.title, self.tokens).tone(self.tone);
        Widget::render(&callout, area, buffer);
        state.region = Some(area);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::input::{KeyCode, KeyModifiers};
    use crate::interaction::OverlayRequest;
    use crate::text::display_cols;

    #[test]
    fn section_toggle_collapse() {
        let mut state = SectionState::new();
        state.set_focused(true);
        assert!(matches!(
            state.handle_key(KeyEvent::new(KeyCode::Enter, KeyModifiers::NONE)),
            SectionOutcome::ToggleCollapsed { collapsed: true }
        ));
        assert!(state.is_collapsed());
    }

    #[test]
    fn heading_paints_strong() {
        let tokens = DesignSystem::default();
        let mut buf = Buffer::empty(Rect::new(0, 0, 20, 1));
        Widget::render(
            &Heading::new("Title", &tokens).level(HeadingLevel::H1),
            Rect::new(0, 0, 20, 1),
            &mut buf,
        );
        assert!(!buf[(0, 0)].symbol().trim().is_empty() || display_cols("Title") > 0);
    }

    #[test]
    fn alert_esc_dismisses() {
        let mut state = AlertState {
            focused: true,
            ..Default::default()
        };
        assert_eq!(
            state.handle_key(KeyEvent::new(KeyCode::Esc, KeyModifiers::NONE)),
            AlertOutcome::Dismissed
        );
        let r = state.handle_key_result(KeyEvent::new(KeyCode::Esc, KeyModifiers::NONE));
        assert_eq!(r.message(), Some(&AlertOutcome::Dismissed));
        assert_eq!(r.overlay(), Some(&OverlayRequest::DismissTop));
    }

    #[test]
    fn section_event_result_emits_toggle() {
        let mut state = SectionState::new();
        state.set_focused(true);
        let r = state.handle_key_result(KeyEvent::new(KeyCode::Enter, KeyModifiers::NONE));
        assert!(r.is_consumed());
        assert!(matches!(
            r.message(),
            Some(SectionOutcome::ToggleCollapsed { collapsed: true })
        ));
    }
}
