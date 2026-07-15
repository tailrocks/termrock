use ratatui_core::{buffer::Buffer, layout::Rect, style::Style, widgets::StatefulWidget};
use unicode_segmentation::UnicodeSegmentation;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum EditAction {
    Insert(char),
    Backspace,
    Delete,
    MoveLeft,
    MoveRight,
    Home,
    End,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Validation<'a> {
    Valid,
    Invalid(&'a str),
}

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct TextInputState {
    value: String,
    cursor: usize,
    viewport: usize,
}

impl TextInputState {
    #[must_use]
    pub fn new(value: impl Into<String>) -> Self {
        let value = value.into();
        let cursor = value.len();
        Self {
            value,
            cursor,
            viewport: 0,
        }
    }
    #[must_use]
    pub fn value(&self) -> &str {
        &self.value
    }
    #[must_use]
    pub const fn cursor_byte(&self) -> usize {
        self.cursor
    }
    pub fn apply(&mut self, action: EditAction) {
        match action {
            EditAction::Insert(c) => {
                self.value.insert(self.cursor, c);
                self.cursor += c.len_utf8();
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
    }
}

#[derive(Debug, Clone, Copy)]
pub struct TextInput<'a> {
    pub label: &'a str,
    pub placeholder: &'a str,
    pub validation: Validation<'a>,
    pub style: Style,
}

impl StatefulWidget for &TextInput<'_> {
    type State = TextInputState;
    fn render(self, area: Rect, buffer: &mut Buffer, state: &mut Self::State) {
        let shown = if state.value.is_empty() {
            self.placeholder
        } else {
            &state.value
        };
        buffer.set_stringn(area.x, area.y, shown, area.width as usize, self.style);
    }
}
