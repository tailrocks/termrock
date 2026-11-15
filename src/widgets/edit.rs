pub mod parsers;

use std::ops::Range;
use std::rc::Rc;

use ratatui::layout::Position;

use crate::core::edit::{EditKind, FieldKeys, TextCmd, UndoStack};
use crate::core::event::{Key, Outcome};
use crate::core::id::WidgetId;
use crate::core::keymap::KeyMap;
use crate::core::text::TextBuffer;
use crate::widgets::field_common::{
    Applied, EditAction, PointerField, PointerSelect, apply_edit, edit_key_in, unbound_key,
};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FieldError {
    message: String,
}

impl FieldError {
    pub fn new(message: impl Into<String>) -> Self {
        Self {
            message: message.into(),
        }
    }

    pub fn message(&self) -> &str {
        &self.message
    }

    pub fn into_message(self) -> String {
        self.message
    }
}

impl std::ops::Deref for FieldError {
    type Target = str;
    fn deref(&self) -> &str {
        &self.message
    }
}

impl FieldError {
    pub fn message_mut(&mut self) -> &mut String {
        &mut self.message
    }
}

impl std::fmt::Display for FieldError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.message)
    }
}

impl From<String> for FieldError {
    fn from(message: String) -> Self {
        Self { message }
    }
}

impl From<&str> for FieldError {
    fn from(message: &str) -> Self {
        Self::new(message)
    }
}

pub trait Validate {
    type Value;
    fn validate(&self, text: &str) -> Result<Self::Value, FieldError>;
}

pub type ValidateFn<V, C> = dyn Fn(&C, &str) -> Result<V, FieldError>;

pub struct Validator<V = (), C = ()>(Rc<ValidateFn<V, C>>);

impl<V, C> Clone for Validator<V, C> {
    fn clone(&self) -> Self {
        Self(self.0.clone())
    }
}

impl<V, C> std::fmt::Debug for Validator<V, C> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("Validator")
    }
}

impl<V: 'static> Validator<V> {
    pub fn new(f: impl Fn(&str) -> Result<V, FieldError> + 'static) -> Self {
        Self(Rc::new(move |_: &(), text: &str| f(text)))
    }
}

impl Validator {
    pub fn check(f: impl Fn(&str) -> Option<String> + 'static) -> Self {
        Self::new(move |text| f(text).map_or(Ok(()), |m| Err(FieldError::from(m))))
    }
}

impl<V: 'static, C: 'static> Validator<V, C> {
    pub fn with_context(f: impl Fn(&C, &str) -> Result<V, FieldError> + 'static) -> Self {
        Self(Rc::new(f))
    }

    pub fn validate_in(&self, cx: &C, text: &str) -> Result<V, FieldError> {
        (self.0)(cx, text)
    }

    pub fn bind(&self, cx: C) -> Validator<V> {
        let f = self.0.clone();
        Validator(Rc::new(move |_: &(), text: &str| f(&cx, text)))
    }
}

impl<V> Validate for Validator<V> {
    type Value = V;
    fn validate(&self, text: &str) -> Result<V, FieldError> {
        (self.0)(&(), text)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum ValidateWhen {
    Commit,

    #[default]
    AfterError,

    Live,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EditExit {
    Commit,

    Cancel,

    Tab { backward: bool },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct EditResult {
    pub outcome: Outcome,

    pub edited: bool,

    pub exit: Option<EditExit>,

    pub unused: bool,
}

impl EditResult {
    fn changed(edited: bool) -> Self {
        Self {
            outcome: Outcome::Changed,
            edited,
            exit: None,
            unused: false,
        }
    }
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
struct PressState {
    at: Option<usize>,
    dragged: bool,

    word: Option<Range<usize>>,
}

#[derive(Debug, Clone)]
pub struct EditField {
    pub buffer: TextBuffer,

    pub sel: PointerSelect,
    press: PressState,
    undo: UndoStack,

    pub keys: FieldKeys,

    pub text_keymap: Option<KeyMap<TextCmd>>,

    pub validator: Option<Validator>,

    pub error: Option<FieldError>,

    pub validate_when: ValidateWhen,

    pub multiline: bool,

    pub read_only: bool,

    pub secret: bool,

    pub undo_runs: bool,

    pub required: Option<String>,

    line_off: usize,

    line_area: ratatui::layout::Rect,
}

impl EditField {
    pub fn single(text: impl Into<String>) -> Self {
        Self::with_buffer(TextBuffer::single(text), false)
    }

    pub fn multi(text: impl Into<String>) -> Self {
        Self::with_buffer(TextBuffer::multi(text), true)
    }

    fn with_buffer(buffer: TextBuffer, multiline: bool) -> Self {
        Self {
            buffer,
            sel: PointerSelect::DEFAULT,
            press: PressState::default(),
            undo: UndoStack::new(),
            keys: FieldKeys::default(),
            text_keymap: None,
            validator: None,
            error: None,
            validate_when: ValidateWhen::default(),
            multiline,
            read_only: false,
            secret: false,
            undo_runs: true,
            required: None,
            line_off: 0,
            line_area: ratatui::layout::Rect::ZERO,
        }
    }

    pub fn validator(mut self, v: Validator) -> Self {
        self.validator = Some(v);
        self
    }

    pub fn validate_when(mut self, when: ValidateWhen) -> Self {
        self.validate_when = when;
        self
    }

    pub fn keys(mut self, keys: FieldKeys) -> Self {
        self.keys = keys;
        self
    }

    pub fn text(&self) -> &str {
        self.buffer.text()
    }

    pub fn set_text(&mut self, text: impl Into<String>) {
        self.buffer.set_text(text);
        self.undo.clear();
    }

    pub fn action(&self, key: &Key) -> EditAction {
        if self.copy_first(key) {
            return EditAction::Copy;
        }
        match &self.text_keymap {
            Some(map) => edit_key_in(key, self.multiline, self.keys, map),
            None => edit_key_in(key, self.multiline, self.keys, &self.keys.text_keymap()),
        }
    }

    pub fn text_cmd(&self, key: &Key) -> Option<TextCmd> {
        if self.copy_first(key) {
            return Some(TextCmd::Copy);
        }
        self.bound_cmd(key)
    }

    fn bound_cmd(&self, key: &Key) -> Option<TextCmd> {
        match &self.text_keymap {
            Some(map) => crate::core::keymap::lookup(map, key),
            None => crate::core::keymap::lookup(&self.keys.text_keymap(), key),
        }
    }

    fn copy_first(&self, key: &Key) -> bool {
        key.ctrl_char('c') && self.buffer.has_selection() && self.bound_cmd(key).is_none()
    }

    pub fn on_key(&mut self, key: &Key) -> EditResult {
        let action = self.action(key);
        self.apply(action, key)
    }

    pub fn apply(&mut self, action: EditAction, key: &Key) -> EditResult {
        self.apply_as(action, key, self.read_only)
    }

    pub fn apply_as(&mut self, action: EditAction, key: &Key, read_only: bool) -> EditResult {
        let exit = match action {
            EditAction::Commit => Some(EditExit::Commit),
            EditAction::Cancel => Some(EditExit::Cancel),
            EditAction::Tab { backward } => Some(EditExit::Tab { backward }),
            EditAction::None => {
                return EditResult {
                    outcome: unbound_key(key),
                    edited: false,
                    exit: None,
                    unused: true,
                };
            }
            _ => None,
        };
        if exit.is_some() {
            return EditResult {
                outcome: Outcome::Changed,
                edited: false,
                exit,
                unused: false,
            };
        }
        let edited = apply_edit(
            &mut self.buffer,
            &mut self.undo,
            action,
            key.code,
            read_only,
            self.secret,
        ) == Some(Applied::Edited);
        if edited {
            if !self.undo_runs {
                self.undo.seal();
            }
            self.after_edit();
        }
        EditResult::changed(edited)
    }

    pub fn run(&mut self, cmd: TextCmd) -> EditResult {
        self.run_as(cmd, self.read_only)
    }

    pub fn run_as(&mut self, cmd: TextCmd, read_only: bool) -> EditResult {
        if cmd == TextCmd::Paste {
            let text = crate::core::edit::clipboard::text().unwrap_or_default();
            return EditResult::changed(!read_only && self.paste(&text));
        }
        let action = crate::widgets::field_common::text_cmd_action(cmd, self.multiline);
        let key = Key {
            code: ratatui::crossterm::event::KeyCode::Null,
            mods: ratatui::crossterm::event::KeyModifiers::NONE,
        };
        self.apply_as(action, &key, read_only)
    }

    pub fn shortcut(&self, cmd: TextCmd) -> Option<String> {
        match &self.text_keymap {
            Some(map) => crate::core::keymap::label_for(map, cmd),
            None => crate::core::keymap::label_for(&self.keys.text_keymap(), cmd),
        }
    }

    pub fn paste(&mut self, text: &str) -> bool {
        if self.read_only || text.is_empty() {
            return false;
        }
        let text = if self.multiline {
            text.replace("\r\n", "\n").replace('\r', "\n")
        } else {
            text.trim_end_matches(['\r', '\n'])
                .replace("\r\n", " ")
                .replace(['\r', '\n'], " ")
        };
        self.undo.record(&self.buffer, EditKind::Other);
        self.buffer.insert_str(&text);
        self.after_edit();
        true
    }

    pub fn record(&mut self, kind: EditKind) {
        self.undo.record(&self.buffer, kind);
    }

    pub fn seal(&mut self) {
        self.undo.seal();
    }

    pub fn clear_history(&mut self) {
        self.undo.clear();
    }

    pub fn can_undo(&self) -> bool {
        self.undo.can_undo()
    }

    pub fn can_redo(&self) -> bool {
        self.undo.can_redo()
    }

    pub fn after_edit(&mut self) {
        match self.validate_when {
            ValidateWhen::Live => {
                self.validate();
            }
            ValidateWhen::AfterError if self.error.is_some() => {
                self.validate();
            }
            _ => {}
        }
    }

    pub fn validate(&mut self) -> bool {
        self.error = match &self.validator {
            Some(v) => v.validate(self.buffer.text()).err(),
            None => self
                .required
                .as_ref()
                .filter(|_| self.buffer.is_empty())
                .map(|m| FieldError::new(m.clone())),
        };
        self.error.is_none()
    }

    pub fn set_error(&mut self, error: Option<FieldError>) {
        self.error = error;
    }

    pub fn error_message(&self) -> Option<&str> {
        self.error.as_ref().map(FieldError::message)
    }

    pub fn press(&mut self, offset: usize, count: u8, whole_word: bool) -> bool {
        self.press = PressState {
            at: Some(offset),
            dragged: false,
            word: None,
        };
        if count == 2 && self.sel.word && self.sel.drag_by_word && !whole_word {
            let w = self.buffer.word_range_at(offset);
            self.buffer.select_range(w.start, w.end);
            self.press.word = Some(w);
            return true;
        }
        false
    }

    pub fn drag(&mut self, offset: usize) -> bool {
        if !self.sel.drag {
            return false;
        }
        let anchor = self.press.at.unwrap_or_else(|| self.buffer.cursor_offset());
        self.press.at = Some(anchor);
        self.press.dragged = true;
        if let Some(w) = self.press.word.clone() {
            let to = self.buffer.word_range_at(offset);
            if to.start < w.start {
                self.buffer.select_range(w.end, to.start);
            } else {
                self.buffer.select_range(w.start, to.end.max(w.end));
            }
            return true;
        }
        self.buffer.select_range(anchor, offset);
        true
    }

    pub fn click(&mut self, offset: usize, count: u8, whole_word: bool) {
        let dragged = std::mem::take(&mut self.press.dragged);
        self.press.at = None;
        self.press.word = None;
        if dragged {
            return;
        }
        if count >= 3 && self.sel.line {
            self.buffer.select_line_at(offset);
        } else if count == 2 && self.sel.word {
            if whole_word {
                self.buffer.select_all();
            } else {
                self.buffer.select_word_at(offset);
            }
        } else {
            self.buffer.select_range(offset, offset);
            self.buffer.clear_selection();
        }
        self.undo.seal();
    }

    pub fn pressed(&self) -> bool {
        self.press.at.is_some()
    }

    pub fn draw_line(
        &mut self,
        buf: &mut ratatui::buffer::Buffer,
        area: ratatui::layout::Rect,
        look: &LineLook,
    ) -> Position {
        let glyphs = Self::graphemes(self.buffer.text());
        self.draw_glyphs(buf, area, look, &glyphs)
    }

    pub fn graphemes(text: &str) -> Vec<(usize, String, usize)> {
        unicode_segmentation::UnicodeSegmentation::grapheme_indices(text, true)
            .map(|(bi, g)| (bi, g.to_owned(), crate::ui::text::width(g)))
            .collect()
    }

    pub fn draw_glyphs(
        &mut self,
        buf: &mut ratatui::buffer::Buffer,
        area: ratatui::layout::Rect,
        look: &LineLook,
        glyphs: &[(usize, String, usize)],
    ) -> Position {
        let cursor_off = self.buffer.cursor_offset();
        let cursor_col: usize = glyphs
            .iter()
            .filter(|(bi, _, _)| *bi < cursor_off)
            .map(|(_, _, w)| *w)
            .sum();
        let total: usize = glyphs.iter().map(|(_, _, w)| *w).sum();
        let w = area.width as usize;
        if w > 0 {
            if cursor_col < self.line_off {
                self.line_off = cursor_col;
            } else if cursor_col >= self.line_off + w {
                self.line_off = cursor_col + 1 - w;
            }
            if !look.follow_caret {
                self.line_off = 0;
            }

            self.line_off = self.line_off.min(total.saturating_add(1).saturating_sub(w));
            let mut boundary = 0;
            for (_, _, gw) in glyphs {
                if boundary >= self.line_off {
                    break;
                }
                boundary += gw;
            }
            self.line_off = boundary.min(cursor_col);
        }
        self.line_area = area;
        let sel = self.buffer.selection();
        let mut col = 0usize;
        let clipped_right = total > self.line_off.saturating_add(w);
        let visible_width = w.saturating_sub(usize::from(clipped_right));
        for (bi, g, gw) in glyphs {
            let gw = *gw;
            if col + gw <= self.line_off {
                col += gw;
                continue;
            }
            let shown_col = col.saturating_sub(self.line_off);
            if shown_col + gw > visible_width {
                break;
            }
            let x = area.x + shown_col as u16;
            if shown_col == 0 && self.line_off > 0 && gw > 0 {
                buf.set_string(x, area.y, look.ellipsis_glyph, look.ellipsis);
                col += gw;
                continue;
            }
            let mut st = look.text;
            if let Some(r) = &sel
                && r.contains(bi)
            {
                st = look.selection;
            }
            if let Some(c) = look.underline {
                st = st
                    .add_modifier(ratatui::style::Modifier::UNDERLINED)
                    .underline_color(c);
            }
            buf.set_string(x, area.y, g, st);
            col += gw;
        }
        if clipped_right && area.width > 0 {
            buf.set_string(area.right() - 1, area.y, look.ellipsis_glyph, look.ellipsis);
        }
        let cx = area.x + cursor_col.saturating_sub(self.line_off) as u16;
        Position::new(cx.min(area.right().saturating_sub(1)), area.y)
    }

    pub fn line_offset_at(&self, pos: Position) -> usize {
        let col =
            (self.line_off as isize + (pos.x as isize - self.line_area.x as isize)).max(0) as usize;
        self.buffer.offset_at(0, col)
    }

    pub fn glyph_offset_at(&self, pos: Position, glyphs: &[(usize, String, usize)]) -> usize {
        let col =
            (self.line_off as isize + (pos.x as isize - self.line_area.x as isize)).max(0) as usize;
        let mut w = 0;
        for (bi, _, gw) in glyphs {
            if w + gw > col {
                return *bi;
            }
            w += gw;
        }
        self.buffer.text().len()
    }

    pub fn line_area(&self) -> ratatui::layout::Rect {
        self.line_area
    }

    pub fn line_scroll(&self) -> usize {
        self.line_off
    }
}

#[derive(Debug, Clone, Copy)]
pub struct LineLook {
    pub text: ratatui::style::Style,
    pub selection: ratatui::style::Style,
    pub ellipsis: ratatui::style::Style,
    pub ellipsis_glyph: &'static str,

    pub underline: Option<ratatui::style::Color>,

    pub follow_caret: bool,
}

impl Default for EditField {
    fn default() -> Self {
        Self::single("")
    }
}

pub trait Editable {
    fn field(&mut self) -> &mut EditField;

    fn edit_id(&self) -> WidgetId;

    fn offset_at(&self, pos: Position) -> usize;

    fn pointer_enabled(&self) -> bool {
        true
    }

    fn whole_word(&self) -> bool {
        false
    }

    fn on_label(&self, _pos: Position) -> bool {
        false
    }

    fn label_click(&mut self) {}

    fn pointer_begin(&mut self);

    fn pointer_focus(&mut self) {}

    fn pointer_after(&mut self) {}
}

impl<T: Editable> PointerField for T {
    fn pointer_id(&self) -> WidgetId {
        self.edit_id()
    }

    fn on_press_count(&mut self, pos: Position, count: u8) -> Outcome {
        if !self.pointer_enabled() || self.on_label(pos) {
            return Outcome::Consumed;
        }
        self.pointer_focus();
        let offset = self.offset_at(pos);
        let whole = self.whole_word();
        if self.field().press(offset, count, whole) {
            self.pointer_begin();
            self.pointer_after();
            return Outcome::Changed;
        }
        Outcome::Consumed
    }

    fn on_drag(&mut self, pos: Position) -> Outcome {
        if !self.pointer_enabled() || !self.field().sel.drag {
            return Outcome::Ignored;
        }
        let offset = self.offset_at(pos);
        self.pointer_begin();
        self.field().drag(offset);
        self.pointer_after();
        Outcome::Changed
    }

    fn on_click_count(&mut self, pos: Position, count: u8) -> Outcome {
        if !self.pointer_enabled() {
            return Outcome::Consumed;
        }
        if self.on_label(pos) {
            self.label_click();
            self.pointer_after();
            return Outcome::Changed;
        }
        self.pointer_begin();
        let offset = self.offset_at(pos);
        let whole = self.whole_word();
        self.field().click(offset, count.max(1), whole);
        self.pointer_after();
        Outcome::Changed
    }
}
