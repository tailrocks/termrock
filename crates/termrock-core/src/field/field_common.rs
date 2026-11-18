use ratatui::crossterm::event::KeyCode;
use ratatui::layout::Position;

use crate::edit::{EditKind, FieldKeys, TextCmd, UndoStack, clipboard};
use crate::event::{Key, Outcome};
use crate::id::WidgetId;
use crate::keymap::{KeyBinding, lookup};
use termrock_text::buffer::TextBuffer;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct PointerSelect {
    pub drag: bool,

    pub word: bool,

    pub line: bool,

    pub drag_by_word: bool,
}

impl PointerSelect {
    pub const DEFAULT: Self = Self {
        drag: true,
        word: true,
        line: true,
        drag_by_word: true,
    };

    pub const OFF: Self = Self {
        drag: false,
        word: false,
        line: false,
        drag_by_word: false,
    };

    pub const fn drag(mut self, on: bool) -> Self {
        self.drag = on;
        self
    }
    pub const fn word(mut self, on: bool) -> Self {
        self.word = on;
        self
    }
    pub const fn line(mut self, on: bool) -> Self {
        self.line = on;
        self
    }
    pub const fn drag_by_word(mut self, on: bool) -> Self {
        self.drag_by_word = on;
        self
    }
}

impl Default for PointerSelect {
    fn default() -> Self {
        Self::DEFAULT
    }
}

pub trait PointerField {
    fn pointer_id(&self) -> WidgetId;

    fn on_press_count(&mut self, pos: Position, count: u8) -> Outcome;

    fn on_drag(&mut self, pos: Position) -> Outcome;

    fn on_click_count(&mut self, pos: Position, count: u8) -> Outcome;
}

pub fn route_press(
    fields: &mut [&mut dyn PointerField],
    id: WidgetId,
    pos: Position,
    count: u8,
) -> Option<Outcome> {
    fields
        .iter_mut()
        .find(|f| f.pointer_id() == id)
        .map(|f| f.on_press_count(pos, count))
}

pub fn route_drag(
    fields: &mut [&mut dyn PointerField],
    pressed: WidgetId,
    pos: Position,
) -> Option<Outcome> {
    fields
        .iter_mut()
        .find(|f| f.pointer_id() == pressed)
        .map(|f| f.on_drag(pos))
}

pub fn route_click(
    fields: &mut [&mut dyn PointerField],
    id: WidgetId,
    pos: Position,
    count: u8,
) -> Option<Outcome> {
    fields
        .iter_mut()
        .find(|f| f.pointer_id() == id)
        .map(|f| f.on_click_count(pos, count.max(1)))
}

#[derive(Clone, Copy)]
pub enum EditAction {
    Commit,
    Cancel,
    Tab { backward: bool },
    Apply(fn(&mut TextBuffer)),
    Insert(char),

    Undo,

    Redo,

    Copy,

    Cut,

    Paste,

    SelectAll,
    None,
}

pub fn unbound_key(key: &Key) -> Outcome {
    if key.chord() {
        Outcome::Ignored
    } else {
        Outcome::Consumed
    }
}

pub fn edit_key(key: &Key, multiline: bool) -> EditAction {
    edit_key_with(key, multiline, FieldKeys::default())
}

pub fn edit_key_with(key: &Key, multiline: bool, keys: FieldKeys) -> EditAction {
    edit_key_in(key, multiline, keys, &keys.text_keymap())
}

pub fn edit_key_in(
    key: &Key,
    multiline: bool,
    keys: FieldKeys,
    text_keymap: &[(KeyBinding, TextCmd)],
) -> EditAction {
    if let Some(cmd) = lookup(text_keymap, key) {
        return text_cmd_action(cmd, multiline);
    }
    let shift = key.shift();

    if key.code == KeyCode::Char('h') && key.ctrl() {
        return EditAction::Apply(|b| b.backspace());
    }
    if keys == FieldKeys::Desktop && key.ctrl() && !key.alt() && key.code == KeyCode::Char('a') {
        return EditAction::None;
    }
    match key.code {
        KeyCode::Esc => EditAction::Cancel,
        KeyCode::Enter if multiline && key.plain() => EditAction::Apply(|b| b.insert_char('\n')),
        KeyCode::Enter => EditAction::Commit,
        KeyCode::Tab => EditAction::Tab { backward: false },
        KeyCode::BackTab => EditAction::Tab { backward: true },
        KeyCode::Left if key.ctrl() || key.alt() => {
            if shift {
                EditAction::Apply(|b| b.move_word_left(true))
            } else {
                EditAction::Apply(|b| b.move_word_left(false))
            }
        }
        KeyCode::Right if key.ctrl() || key.alt() => {
            if shift {
                EditAction::Apply(|b| b.move_word_right(true))
            } else {
                EditAction::Apply(|b| b.move_word_right(false))
            }
        }
        KeyCode::Left => {
            if shift {
                EditAction::Apply(|b| b.move_left(true))
            } else {
                EditAction::Apply(|b| b.move_left(false))
            }
        }
        KeyCode::Right => {
            if shift {
                EditAction::Apply(|b| b.move_right(true))
            } else {
                EditAction::Apply(|b| b.move_right(false))
            }
        }
        KeyCode::Up if multiline => {
            if shift {
                EditAction::Apply(|b| {
                    b.move_up(true);
                })
            } else {
                EditAction::Apply(|b| {
                    b.move_up(false);
                })
            }
        }
        KeyCode::Down if multiline => {
            if shift {
                EditAction::Apply(|b| {
                    b.move_down(true);
                })
            } else {
                EditAction::Apply(|b| {
                    b.move_down(false);
                })
            }
        }
        KeyCode::Home if key.ctrl() => EditAction::Apply(|b| b.move_doc_start(false)),
        KeyCode::End if key.ctrl() => EditAction::Apply(|b| b.move_doc_end(false)),
        KeyCode::Home => {
            if shift {
                EditAction::Apply(|b| b.move_home(true))
            } else {
                EditAction::Apply(|b| b.move_home(false))
            }
        }
        KeyCode::End => {
            if shift {
                EditAction::Apply(|b| b.move_end(true))
            } else {
                EditAction::Apply(|b| b.move_end(false))
            }
        }
        KeyCode::Backspace if key.ctrl() || key.alt() => {
            EditAction::Apply(|b| b.delete_word_left())
        }
        KeyCode::Backspace => EditAction::Apply(|b| b.backspace()),
        KeyCode::Delete => EditAction::Apply(|b| b.delete()),
        KeyCode::Char('a') if key.ctrl() => EditAction::Apply(|b| b.move_home(false)),
        KeyCode::Char('e') if key.ctrl() => EditAction::Apply(|b| b.move_end(false)),
        KeyCode::Char('u') if key.ctrl() => EditAction::Apply(|b| b.delete_to_line_start()),
        KeyCode::Char('k') if key.ctrl() => EditAction::Apply(|b| b.delete_to_line_end()),
        KeyCode::Char('b') if key.alt() => EditAction::Apply(|b| b.move_word_left(false)),
        KeyCode::Char('f') if key.alt() => EditAction::Apply(|b| b.move_word_right(false)),
        KeyCode::Char(_) if key.chord() => EditAction::None,
        KeyCode::Char(c) => EditAction::Insert(c),
        _ => EditAction::None,
    }
}

pub fn text_cmd_action(cmd: TextCmd, multiline: bool) -> EditAction {
    match cmd {
        TextCmd::SelectAll => EditAction::SelectAll,
        TextCmd::Copy => EditAction::Copy,
        TextCmd::Cut => EditAction::Cut,
        TextCmd::Paste => EditAction::Paste,
        TextCmd::Undo => EditAction::Undo,
        TextCmd::Redo => EditAction::Redo,
        TextCmd::LineStart => EditAction::Apply(|b| b.move_home(false)),
        TextCmd::LineEnd => EditAction::Apply(|b| b.move_end(false)),
        TextCmd::SelectToLineStart => EditAction::Apply(|b| b.move_home(true)),
        TextCmd::SelectToLineEnd => EditAction::Apply(|b| b.move_end(true)),
        TextCmd::DocStart
        | TextCmd::DocEnd
        | TextCmd::SelectToDocStart
        | TextCmd::SelectToDocEnd
            if !multiline =>
        {
            EditAction::None
        }
        TextCmd::DocStart => EditAction::Apply(|b| b.move_doc_start(false)),
        TextCmd::DocEnd => EditAction::Apply(|b| b.move_doc_end(false)),
        TextCmd::SelectToDocStart => EditAction::Apply(|b| b.move_doc_start(true)),
        TextCmd::SelectToDocEnd => EditAction::Apply(|b| b.move_doc_end(true)),
    }
}

pub fn paste_clipboard(on_paste: impl FnOnce(&str) -> Outcome) -> Outcome {
    match clipboard::text() {
        Some(t) => on_paste(&t),
        None => Outcome::Ignored,
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Applied {
    Edited,

    Moved,
}

pub fn apply_edit(
    buf: &mut TextBuffer,
    undo: &mut UndoStack,
    action: EditAction,
    key_code: KeyCode,
    read_only: bool,
    secret: bool,
) -> Option<Applied> {
    let before = buf.clone();
    let kind = match (action, key_code) {
        (EditAction::Insert(c), _) if c.is_whitespace() => EditKind::Other,
        (EditAction::Insert(_), _) => EditKind::Typing,
        (EditAction::Apply(_), KeyCode::Backspace | KeyCode::Delete) => EditKind::Deleting,
        _ => EditKind::Other,
    };
    match action {
        EditAction::Commit | EditAction::Cancel | EditAction::Tab { .. } | EditAction::None => {
            return None;
        }
        EditAction::Undo if !read_only => {
            return Some(if undo.undo(buf) {
                Applied::Edited
            } else {
                Applied::Moved
            });
        }
        EditAction::Redo if !read_only => {
            return Some(if undo.redo(buf) {
                Applied::Edited
            } else {
                Applied::Moved
            });
        }
        EditAction::Undo | EditAction::Redo => return Some(Applied::Moved),
        EditAction::SelectAll => {
            buf.select_all();
            undo.seal();
            return Some(Applied::Moved);
        }
        EditAction::Copy if secret => return Some(Applied::Moved),
        EditAction::Cut if secret && key_code != KeyCode::Char('w') => {
            return Some(Applied::Moved);
        }
        EditAction::Copy => {
            if let Some(t) = buf.selected_text() {
                clipboard::copy(t);
            }
            return Some(Applied::Moved);
        }
        EditAction::Cut => match buf.selected_text().map(str::to_owned) {
            Some(t) => {
                if !secret {
                    clipboard::copy(&t);
                }
                if !read_only {
                    buf.backspace();
                }
            }

            None if key_code == KeyCode::Char('w') && !read_only => buf.delete_word_left(),
            None => {}
        },
        EditAction::Paste => {
            if let Some(t) = clipboard::text()
                && !read_only
            {
                buf.insert_str(&t);
            }
        }
        EditAction::Insert(c) => {
            if !read_only {
                buf.insert_char(c);
            }
        }
        EditAction::Apply(f) => {
            let mut probe = buf.clone();
            f(&mut probe);
            if !read_only || probe.revision() == buf.revision() {
                *buf = probe;
            }
        }
    }
    if buf.revision() != before.revision() {
        undo.record(&before, kind);
        Some(Applied::Edited)
    } else {
        undo.seal();
        Some(Applied::Moved)
    }
}
