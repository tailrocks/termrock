//! **ConfirmPrompt** — the last question before something irreversible.
//!
//! Three patterns had each grown their own version of this: a red sentence
//! over the pane's last two rows and a pair of bracket-wrapped words that were
//! not buttons. They disagreed on tone, on which side Cancel sat, and on
//! whether the destructive choice could be the resting focus.
//!
//! The prompt states one rule instead: the consequence is spelled out, Cancel
//! is the resting focus, the destructive action is a real
//! [`ButtonVariant::Destructive`] chip, and danger stays on that chip rather
//! than washing the whole surface (decision D1 in
//! `docs/design/termrock-component-audit-2026-08.md`, plans/009).

use ratatui_core::{buffer::Buffer, layout::Rect};

use crate::style::{DesignSystem, Glyph, Role};
use crate::text::take_display_cols;
use crate::widgets::primitives::{Button, ButtonState, ButtonVariant};
use crate::widgets::tiered_row::TieredRow;

/// Which side of the prompt owns focus.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
#[non_exhaustive]
pub enum ConfirmFocus {
    /// The safe way out — the resting focus for a destructive prompt.
    #[default]
    Cancel,
    /// The action being confirmed.
    Confirm,
}

impl ConfirmFocus {
    /// Moves focus to the other side.
    #[must_use]
    pub const fn toggled(self) -> Self {
        match self {
            Self::Cancel => Self::Confirm,
            Self::Confirm => Self::Cancel,
        }
    }

    /// Stable id.
    #[must_use]
    pub const fn id(self) -> &'static str {
        match self {
            Self::Cancel => "cancel",
            Self::Confirm => "confirm",
        }
    }
}

/// Painted hit regions, for hosts that route pointer input.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct ConfirmPromptHits {
    /// The cancel chip.
    pub cancel: Option<Rect>,
    /// The confirm chip.
    pub confirm: Option<Rect>,
}

impl ConfirmPromptHits {
    /// Which side a pointer position lands on, if any.
    #[must_use]
    pub fn hit(&self, position: ratatui_core::layout::Position) -> Option<ConfirmFocus> {
        if self.cancel.is_some_and(|r| r.contains(position)) {
            return Some(ConfirmFocus::Cancel);
        }
        if self.confirm.is_some_and(|r| r.contains(position)) {
            return Some(ConfirmFocus::Confirm);
        }
        None
    }
}

/// Rows a confirm prompt needs at the bottom of a pane.
pub const CONFIRM_PROMPT_ROWS: u16 = 2;

/// A two-row confirmation: the consequence, then the two ways out.
#[derive(Debug, Clone, Copy)]
pub struct ConfirmPrompt<'a> {
    system: &'a DesignSystem,
    message: &'a str,
    detail: Option<&'a str>,
    confirm_label: &'a str,
    cancel_label: &'a str,
    destructive: bool,
    focus: ConfirmFocus,
    colorless: bool,
}

impl<'a> ConfirmPrompt<'a> {
    /// A prompt asking whether to go ahead.
    #[must_use]
    pub const fn new(message: &'a str, confirm_label: &'a str, system: &'a DesignSystem) -> Self {
        Self {
            system,
            message,
            detail: None,
            confirm_label,
            cancel_label: "Cancel",
            destructive: true,
            focus: ConfirmFocus::Cancel,
            colorless: false,
        }
    }

    /// The consequence, stated after the message.
    #[must_use]
    pub const fn detail(mut self, detail: &'a str) -> Self {
        self.detail = Some(detail);
        self
    }

    /// Renames the safe way out (default `Cancel`).
    #[must_use]
    pub const fn cancel_label(mut self, label: &'a str) -> Self {
        self.cancel_label = label;
        self
    }

    /// Whether the confirmed action destroys something (default `true`).
    #[must_use]
    pub const fn destructive(mut self, destructive: bool) -> Self {
        self.destructive = destructive;
        self
    }

    /// Which side owns focus.
    #[must_use]
    pub const fn focus(mut self, focus: ConfirmFocus) -> Self {
        self.focus = focus;
        self
    }

    /// Drops hue for colorless terminals.
    #[must_use]
    pub const fn colorless(mut self, colorless: bool) -> Self {
        self.colorless = colorless;
        self
    }

    /// Paints the prompt into the last two rows of `area`.
    pub fn paint(&self, area: Rect, buffer: &mut Buffer) -> ConfirmPromptHits {
        let mut hits = ConfirmPromptHits::default();
        if area.width == 0 || area.height == 0 {
            return hits;
        }
        let system = self.system;
        let bar_y = area.bottom().saturating_sub(1);
        let message_y = bar_y.saturating_sub(1);
        let width = usize::from(area.width);

        if message_y >= area.y {
            // The risk glyph carries the severity so the warning survives a
            // colorless terminal; the sentence itself stays readable.
            let glyph = system.glyphs.resolve(Glyph::Warning).text;
            let mut row = TieredRow::with_separator(" ");
            row.push(
                glyph,
                system.style(if self.colorless {
                    Role::Text
                } else if self.destructive {
                    Role::Danger
                } else {
                    Role::Warning
                }),
            );
            row.push_plain(self.message);
            if let Some(detail) = self.detail {
                row.push_joined(" — ", Some(system.style(Role::TextFaint)));
                row.push_joined(detail, Some(system.style(Role::TextMuted)));
            }
            let line = row.text().to_string();
            buffer.set_stringn(
                area.x,
                message_y,
                take_display_cols(&line, width),
                width,
                system.style(Role::Text),
            );
            row.paint_tiers(buffer, Rect::new(area.x, message_y, area.width, 1), 0);
        }

        // Cancel first: the safe way out is the one you reach without aiming.
        let cancel = Button::new(self.cancel_label, system).variant(ButtonVariant::Secondary);
        let confirm = Button::new(self.confirm_label, system).variant(if self.destructive {
            ButtonVariant::Destructive
        } else {
            ButtonVariant::Primary
        });
        let cancel_w = cancel.preferred_width();
        let confirm_w = confirm.preferred_width();
        let gap = 2u16;
        if bar_y < area.y || cancel_w + gap + confirm_w > area.width {
            return hits;
        }

        // A button wears focus chrome when it accepts input, so exactly one
        // chip does — the prompt is stateless and the host owns activation.
        let mut cancel_state = ButtonState::new();
        cancel_state
            .activation
            .set_accepts_input(matches!(self.focus, ConfirmFocus::Cancel));
        let mut confirm_state = ButtonState::new();
        confirm_state
            .activation
            .set_accepts_input(matches!(self.focus, ConfirmFocus::Confirm));

        let cancel_rect = Rect::new(area.x, bar_y, cancel_w, 1);
        let confirm_rect = Rect::new(
            area.x.saturating_add(cancel_w).saturating_add(gap),
            bar_y,
            confirm_w,
            1,
        );
        hits.cancel = Some(cancel.paint(cancel_rect, buffer, &mut cancel_state).root);
        hits.confirm = Some(confirm.paint(confirm_rect, buffer, &mut confirm_state).root);
        hits
    }
}

#[cfg(test)]
mod tests {
    use ratatui_core::layout::Position;

    use super::*;

    #[test]
    fn cancel_sits_first_and_holds_the_resting_focus() {
        let system = DesignSystem::default();
        let area = Rect::new(0, 0, 48, 6);
        let mut buffer = Buffer::empty(area);
        let hits = ConfirmPrompt::new("Delete branch", "Delete", &system)
            .detail("history is not recoverable")
            .paint(area, &mut buffer);
        let cancel = hits.cancel.expect("cancel chip");
        let confirm = hits.confirm.expect("confirm chip");
        assert!(cancel.x < confirm.x, "cancel comes first");
        assert_eq!(
            hits.hit(Position::new(cancel.x, cancel.y)),
            Some(ConfirmFocus::Cancel)
        );
        assert_eq!(
            hits.hit(Position::new(confirm.x, confirm.y)),
            Some(ConfirmFocus::Confirm)
        );
    }

    #[test]
    fn danger_stays_on_the_confirm_chip() {
        let system = DesignSystem::default();
        let area = Rect::new(0, 0, 48, 6);
        let mut buffer = Buffer::empty(area);
        let hits = ConfirmPrompt::new("Delete branch", "Delete", &system).paint(area, &mut buffer);
        let confirm = hits.confirm.expect("confirm chip");
        let danger = system.style(Role::Danger).fg;

        let outside: usize = (area.x..area.right())
            .filter(|x| {
                let cell = &buffer[(*x, confirm.y)];
                !(confirm.x..confirm.right()).contains(x)
                    && !cell.symbol().trim().is_empty()
                    && Some(cell.fg) == danger
            })
            .count();
        assert_eq!(outside, 0, "danger belongs to the chip, not to the row");
    }

    #[test]
    fn a_prompt_too_narrow_for_its_chips_paints_no_half_button() {
        let system = DesignSystem::default();
        let area = Rect::new(0, 0, 6, 3);
        let mut buffer = Buffer::empty(area);
        let hits = ConfirmPrompt::new("Delete", "Delete", &system).paint(area, &mut buffer);
        assert!(hits.cancel.is_none() && hits.confirm.is_none());
    }
}
