use ratatui_core::{buffer::Buffer, layout::Rect, widgets::StatefulWidget};
use unicode_segmentation::UnicodeSegmentation;
use unicode_width::UnicodeWidthStr;

use crate::{
    input::{KeyCode, KeyEvent, KeyEventKind, KeyModifiers},
    style::{Role, Theme},
    text::take_display_cols,
};

#[derive(Debug, Clone, PartialEq, Eq)]
/// Available `EditAction` choices.
pub enum EditAction {
    /// Selects the `Insert` behavior.
    Insert(char),
    /// Selects the `Backspace` behavior.
    Backspace,
    /// Selects the `Delete` behavior.
    Delete,
    /// Selects the `MoveLeft` behavior.
    MoveLeft,
    /// Selects the `MoveRight` behavior.
    MoveRight,
    /// Selects the `Home` behavior.
    Home,
    /// Selects the `End` behavior.
    End,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
/// Available `Validation` choices.
pub enum Validation<'a> {
    /// Selects the `Valid` behavior.
    Valid,
    /// Selects the `Invalid` behavior.
    Invalid(&'a str),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
/// Available `TextInputValidity` choices.
pub enum TextInputValidity {
    /// Selects the `Valid` behavior.
    Valid,
    /// Selects the `Empty` behavior.
    Empty,
    /// Selects the `Forbidden` behavior.
    Forbidden,
}

#[derive(Debug, Clone, PartialEq, Eq)]
#[non_exhaustive]
/// Available `TextInputOutcome` choices.
pub enum TextInputOutcome {
    /// Selects the `Ignored` behavior.
    Ignored,
    /// Selects the `Changed` behavior.
    Changed,
    /// Selects the `Submitted` behavior.
    Submitted(String),
    /// Selects the `Cancelled` behavior.
    Cancelled,
}

#[derive(Debug, Clone, PartialEq, Eq, Default)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
/// Runtime state for `TextInput`.
pub struct TextInputState {
    value: String,
    cursor: usize,
    viewport: usize,
    max_graphemes: Option<usize>,
    forbidden: Vec<String>,
    allow_empty: bool,
}

impl TextInputState {
    #[must_use]
    /// Creates a new value with canonical defaults.
    pub fn new(value: impl Into<String>) -> Self {
        let value = value.into();
        let cursor = value.len();
        Self {
            value,
            cursor,
            viewport: 0,
            max_graphemes: None,
            forbidden: Vec::new(),
            allow_empty: false,
        }
    }

    #[must_use]
    /// Returns this value with `max_graphemes` configured.
    pub fn with_max_graphemes(mut self, max_graphemes: usize) -> Self {
        self.max_graphemes = Some(max_graphemes);
        self
    }

    #[must_use]
    /// Returns this value with `forbidden` configured.
    pub fn with_forbidden(mut self, forbidden: impl IntoIterator<Item = String>) -> Self {
        self.forbidden = forbidden.into_iter().collect();
        self
    }

    #[must_use]
    /// Returns this value with `allow_empty` configured.
    pub const fn with_allow_empty(mut self, allow_empty: bool) -> Self {
        self.allow_empty = allow_empty;
        self
    }

    #[must_use]
    /// Performs the `value` operation.
    pub fn value(&self) -> &str {
        &self.value
    }

    #[must_use]
    /// Performs the `trimmed_value` operation.
    pub fn trimmed_value(&self) -> &str {
        self.value.trim()
    }

    #[must_use]
    /// Performs the `cursor_byte` operation.
    pub const fn cursor_byte(&self) -> usize {
        self.cursor
    }

    /// Moves the cursor to an externally owned byte offset when it is a
    /// grapheme boundary in the current value.
    ///
    /// Returns `false` without changing the state when the offset is out of
    /// bounds or would split a user-perceived character.
    pub fn set_cursor_byte(&mut self, cursor: usize) -> bool {
        let valid = cursor == self.value.len()
            || self
                .value
                .grapheme_indices(true)
                .any(|(index, _)| index == cursor);
        if valid {
            self.cursor = cursor;
        }
        valid
    }

    #[must_use]
    /// Performs the `validity` operation.
    pub fn validity(&self) -> TextInputValidity {
        let value = self.trimmed_value();
        if value.is_empty() && !self.allow_empty {
            TextInputValidity::Empty
        } else if !value.is_empty() && self.forbidden.iter().any(|item| item == value) {
            TextInputValidity::Forbidden
        } else {
            TextInputValidity::Valid
        }
    }

    #[must_use]
    /// Returns whether `valid`.
    pub fn is_valid(&self) -> bool {
        self.validity() == TextInputValidity::Valid
    }

    /// Handles the `handle_key` interaction.
    pub fn handle_key(&mut self, key: KeyEvent) -> TextInputOutcome {
        if key.kind == KeyEventKind::Release {
            return TextInputOutcome::Ignored;
        }
        match key.code {
            KeyCode::Enter => self.submit(),
            KeyCode::Char('m' | 'M') if key.modifiers.contains(KeyModifiers::CONTROL) => {
                self.submit()
            }
            KeyCode::Esc => TextInputOutcome::Cancelled,
            KeyCode::Backspace => self.edit(EditAction::Backspace),
            KeyCode::Delete => self.edit(EditAction::Delete),
            KeyCode::Left => self.edit(EditAction::MoveLeft),
            KeyCode::Right => self.edit(EditAction::MoveRight),
            KeyCode::Home => self.edit(EditAction::Home),
            KeyCode::End => self.edit(EditAction::End),
            KeyCode::Char(character)
                if !key.modifiers.contains(KeyModifiers::CONTROL)
                    && !key.modifiers.contains(KeyModifiers::ALT)
                    && !character.is_control() =>
            {
                self.edit(EditAction::Insert(character))
            }
            _ => TextInputOutcome::Ignored,
        }
    }

    /// Performs the `apply` operation.
    pub fn apply(&mut self, action: EditAction) -> bool {
        let before_cursor = self.cursor;
        let before_len = self.value.len();
        match action {
            EditAction::Insert(character) => {
                if character.is_control() {
                    return false;
                }
                let mut candidate = self.value.clone();
                candidate.insert(self.cursor, character);
                if self
                    .max_graphemes
                    .is_some_and(|max| candidate.graphemes(true).count() > max)
                {
                    return false;
                }
                self.value = candidate;
                self.cursor += character.len_utf8();
            }
            EditAction::Backspace => {
                if let Some((index, _)) =
                    self.value[..self.cursor].grapheme_indices(true).next_back()
                {
                    self.value.drain(index..self.cursor);
                    self.cursor = index;
                }
            }
            EditAction::Delete => {
                if let Some(grapheme) = self.value[self.cursor..].graphemes(true).next() {
                    self.value.drain(self.cursor..self.cursor + grapheme.len());
                }
            }
            EditAction::MoveLeft => {
                if let Some((index, _)) =
                    self.value[..self.cursor].grapheme_indices(true).next_back()
                {
                    self.cursor = index;
                }
            }
            EditAction::MoveRight => {
                if let Some(grapheme) = self.value[self.cursor..].graphemes(true).next() {
                    self.cursor += grapheme.len();
                }
            }
            EditAction::Home => self.cursor = 0,
            EditAction::End => self.cursor = self.value.len(),
        }
        before_cursor != self.cursor || before_len != self.value.len()
    }

    fn submit(&self) -> TextInputOutcome {
        if self.is_valid() {
            TextInputOutcome::Submitted(self.value.clone())
        } else {
            TextInputOutcome::Ignored
        }
    }

    fn edit(&mut self, action: EditAction) -> TextInputOutcome {
        if self.apply(action) {
            TextInputOutcome::Changed
        } else {
            TextInputOutcome::Ignored
        }
    }

    fn reveal_cursor(&mut self, width: usize) {
        self.viewport = self.viewport.min(self.cursor);
        while UnicodeWidthStr::width(&self.value[self.viewport..self.cursor]) >= width.max(1) {
            let Some(grapheme) = self.value[self.viewport..].graphemes(true).next() else {
                break;
            };
            self.viewport += grapheme.len();
        }
    }
}

#[derive(Debug, Clone, Copy)]
/// Data carried by `TextInput`.
pub struct TextInput<'a> {
    label: &'a str,
    placeholder: &'a str,
    validation: Validation<'a>,
    theme: &'a Theme,
}

impl<'a> TextInput<'a> {
    #[must_use]
    /// Creates a new value with canonical defaults.
    pub const fn new(label: &'a str, theme: &'a Theme) -> Self {
        Self {
            label,
            placeholder: "",
            validation: Validation::Valid,
            theme,
        }
    }

    #[must_use]
    /// Performs the `placeholder` operation.
    pub const fn placeholder(mut self, placeholder: &'a str) -> Self {
        self.placeholder = placeholder;
        self
    }

    #[must_use]
    /// Performs the `validation` operation.
    pub const fn validation(mut self, validation: Validation<'a>) -> Self {
        self.validation = validation;
        self
    }

    #[must_use]
    /// Performs the `label` operation.
    pub const fn label(&self) -> &'a str {
        self.label
    }
}

impl StatefulWidget for &TextInput<'_> {
    type State = TextInputState;

    fn render(self, area: Rect, buffer: &mut Buffer, state: &mut Self::State) {
        if area.is_empty() {
            return;
        }
        state.reveal_cursor(usize::from(area.width));
        let invalid = !state.is_valid() || matches!(self.validation, Validation::Invalid(_));
        let input_style = self.theme.style(if invalid {
            Role::InputInvalid
        } else {
            Role::Input
        });
        buffer.set_style(area, input_style);
        if state.value.is_empty() {
            buffer.set_stringn(
                area.x,
                area.y,
                take_display_cols(self.placeholder, usize::from(area.width)),
                usize::from(area.width),
                self.theme.style(Role::TextMuted),
            );
        } else {
            let visible =
                take_display_cols(&state.value[state.viewport..], usize::from(area.width));
            buffer.set_stringn(
                area.x,
                area.y,
                visible,
                usize::from(area.width),
                input_style,
            );
        }
        let cursor_column = UnicodeWidthStr::width(&state.value[state.viewport..state.cursor]);
        let cursor_x = area
            .x
            .saturating_add(u16::try_from(cursor_column).unwrap_or(u16::MAX))
            .min(area.right().saturating_sub(1));
        buffer.set_style(
            Rect::new(cursor_x, area.y, 1, 1),
            self.theme.style(Role::Focus),
        );
    }
}

impl StatefulWidget for TextInput<'_> {
    type State = TextInputState;

    fn render(self, area: Rect, buffer: &mut Buffer, state: &mut Self::State) {
        StatefulWidget::render(&self, area, buffer, state);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn keyboard_owns_edit_submit_cancel_and_validation() {
        let mut state = TextInputState::new("")
            .with_forbidden(["taken".to_owned()])
            .with_max_graphemes(5);
        assert_eq!(
            state.handle_key(KeyEvent::new(KeyCode::Enter, KeyModifiers::NONE)),
            TextInputOutcome::Ignored
        );
        for character in "taken!".chars() {
            state.handle_key(KeyEvent::new(KeyCode::Char(character), KeyModifiers::NONE));
        }
        assert_eq!(state.value(), "taken");
        assert_eq!(state.validity(), TextInputValidity::Forbidden);
        state.handle_key(KeyEvent::new(KeyCode::Backspace, KeyModifiers::NONE));
        assert_eq!(
            state.handle_key(KeyEvent::new(KeyCode::Enter, KeyModifiers::NONE)),
            TextInputOutcome::Submitted("take".to_owned())
        );
        assert_eq!(
            state.handle_key(KeyEvent::new(KeyCode::Esc, KeyModifiers::NONE)),
            TextInputOutcome::Cancelled
        );
    }

    #[test]
    fn render_reveals_wide_cursor_in_narrow_viewport() {
        let theme = Theme::default();
        let mut state = TextInputState::new("alpha🧪");
        let area = Rect::new(3, 2, 4, 1);
        let mut buffer = Buffer::empty(Rect::new(0, 0, 10, 5));
        (&TextInput::new("Name", &theme)).render(area, &mut buffer, &mut state);
        assert!(state.viewport > 0);
        assert!(state.cursor_byte() >= state.viewport);
    }

    #[test]
    fn external_cursor_accepts_grapheme_boundaries_and_rejects_splits() {
        let mut state = TextInputState::new("a👩‍💻🧪");

        assert!(state.set_cursor_byte(1));
        assert_eq!(state.cursor_byte(), 1);
        assert!(!state.set_cursor_byte(2));
        assert_eq!(state.cursor_byte(), 1);
        assert!(state.set_cursor_byte("a👩‍💻".len()));
        assert_eq!(state.cursor_byte(), "a👩‍💻".len());
        assert!(state.set_cursor_byte(state.value().len()));
    }
}
