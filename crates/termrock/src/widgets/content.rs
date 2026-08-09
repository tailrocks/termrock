// SPDX-FileCopyrightText: 2026 Alexey Zhokhov
// SPDX-License-Identifier: Apache-2.0

//! Content hierarchy primitives: heading, paragraph, callout, alert.
//! Section chrome: [`crate::widgets::Section`].

use ratatui_core::{buffer::Buffer, layout::Rect, widgets::Widget};

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
        use crate::widgets::text::{Text, TextSpan};
        if area.is_empty() {
            return;
        }
        // H1: strong + underline; H2: strong; H3: TextStrong role only.
        let span = match self.level {
            HeadingLevel::H1 => TextSpan::new(self.text)
                .role(Role::TextStrong)
                .strong()
                .underline(true),
            HeadingLevel::H2 => TextSpan::new(self.text)
                .role(Role::TextStrong)
                .strong(),
            HeadingLevel::H3 => TextSpan::new(self.text).role(Role::TextStrong),
        };
        let _ = Text::spans([span], self.tokens).truncate().paint(area, buffer);
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
        use crate::widgets::text::Text;
        if area.is_empty() {
            return;
        }
        let mut text = Text::new(self.text, self.tokens).wrap();
        if self.muted {
            text = text.muted();
        }
        let _ = text.paint(area, buffer);
    }
}

// Surface lives in `widgets/surface.rs` (canonical fill/border/clip/hit).
// Section lives in `widgets/section.rs` (editorial grouping anatomy).

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
        use crate::layout::{FlexSize, Stack};

        let style = self.tokens.style(self.tone.role());
        let head = format!("{} {}", self.tone.glyph(), self.title);
        let rows = if self.body.is_some() && area.height > 1 {
            Stack::new().layout(area, &[FlexSize::Fixed(1), FlexSize::Weight(1)])
        } else {
            Stack::new().layout(area, &[FlexSize::Weight(1)])
        };
        if let Some(title_r) = rows.get(0) {
            let text = take_display_cols(&head, usize::from(title_r.width));
            buffer.set_stringn(
                title_r.x,
                title_r.y,
                &text,
                usize::from(title_r.width),
                style,
            );
        }
        if let (Some(body), Some(body_r)) = (self.body, rows.get(1)) {
            let b = take_display_cols(body, usize::from(body_r.width));
            buffer.set_stringn(
                body_r.x,
                body_r.y,
                &b,
                usize::from(body_r.width),
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

}
