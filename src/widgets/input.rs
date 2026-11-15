use ratatui::buffer::Buffer;
use ratatui::crossterm::event::KeyCode;
use ratatui::layout::{Position, Rect};
use ratatui::style::Modifier;

use crate::core::edit::{EditEntry, EditKind, FieldKeys, FocusSelect, TextCmd};
use crate::core::event::{Key, Outcome};
use crate::core::focus::{BlurAction, request_focus_next};
use crate::core::id::WidgetId;
use crate::ui::ctx::RenderCtx;
use crate::ui::edit_menu::{
    EditCaps, EditMenu, EditMenuHost, edit_menu_key, edit_menu_pointer, edit_menu_route,
    editable_target, point_editable,
};
use crate::ui::text::width;
use crate::widgets::edit::{EditField, Editable, FieldError, LineLook, ValidateWhen, Validator};
use crate::widgets::field_common::{EditAction, PointerField, PointerSelect, paste_clipboard};

impl crate::ui::measure::Measure for TextInput {
    fn measure(
        &self,
        _avail: ratatui::layout::Size,
        ctx: &crate::ui::ctx::RenderCtx,
    ) -> ratatui::layout::Size {
        let t = ctx.theme;
        let suffix = if self.required {
            2
        } else if self.plain_label || self.label.is_empty() {
            0
        } else {
            4 + width(t.strings.optional)
        };

        let label = if self.wrap_label {
            suffix
        } else {
            width(&self.label) + suffix
        };

        let value = width(self.field.buffer.text()).max(width(&self.placeholder)) + 3 + 1 + 4;
        let message = self
            .field
            .error
            .as_ref()
            .map(|e| width(&e.to_string()))
            .unwrap_or(0)
            .max(width(&self.help))
            + 2;
        let w = (label + Self::GUTTER as usize).max(value).max(message) as u16;
        ratatui::layout::Size::new(w, self.height_for(w))
    }
}

#[derive(Debug, Clone)]
pub struct TextInput {
    pub id: WidgetId,
    pub label: String,
    pub placeholder: String,

    pub field: EditField,
    pub disabled: bool,
    pub required: bool,

    pub required_message: Option<String>,

    required_text: &'static str,
    pub help: String,
    pub editing: bool,

    snapshot: String,

    snapshot_error: Option<FieldError>,
    pub area: Rect,

    text_area: Rect,

    pub plain_label: bool,

    pub masked: bool,

    pub reveal_tail: u8,

    pub wrap_label: bool,

    pub blur_action: BlurAction,

    pub edit_entry: Option<EditEntry>,

    entry: EditEntry,

    pub focus_select: FocusSelect,

    pub field_keys: Option<FieldKeys>,

    focus_seen: bool,

    focus_edit: bool,

    pub label_hit: bool,

    pub validate_on: ValidateOn,

    pub readout: String,

    pub readout_link: Option<WidgetId>,

    pub readout_area: Rect,

    pub edit_menu: EditMenu,
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub enum ValidateOn {
    #[default]
    Change,

    EveryCommit,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum InputAction {
    Committed,
    Cancelled,

    CommittedTab { backward: bool },
    Changed,
}

impl TextInput {
    pub fn new(id: WidgetId, label: &str) -> Self {
        Self {
            id,
            label: label.to_owned(),
            placeholder: String::new(),
            field: EditField::single(""),
            disabled: false,
            required: false,
            required_message: None,
            required_text: crate::theme::Strings::DEFAULT.required,
            help: String::new(),
            editing: false,
            snapshot: String::new(),
            snapshot_error: None,
            area: Rect::ZERO,
            text_area: Rect::ZERO,
            plain_label: false,
            masked: false,
            reveal_tail: 0,
            wrap_label: false,
            blur_action: BlurAction::Commit,
            edit_entry: None,
            entry: crate::theme::Theme::DEFAULT_EDIT_ENTRY,
            focus_select: FocusSelect::default(),
            field_keys: None,
            focus_seen: false,
            focus_edit: false,
            label_hit: true,
            validate_on: ValidateOn::Change,
            readout: String::new(),
            readout_link: None,
            readout_area: Rect::ZERO,
            edit_menu: EditMenu::default(),
        }
    }

    pub fn readout(mut self, text: &str) -> Self {
        self.readout = text.to_owned();
        self
    }

    pub fn set_readout(&mut self, text: &str) {
        if self.readout != text {
            self.readout = text.to_owned();
        }
    }

    pub fn readout_link(mut self, id: WidgetId) -> Self {
        self.readout_link = Some(id);
        self
    }

    pub fn placeholder(mut self, p: &str) -> Self {
        self.placeholder = p.to_owned();
        self
    }

    pub fn required_message(mut self, message: &str) -> Self {
        self.required_message = Some(message.to_owned());
        self
    }
    pub fn value(mut self, v: &str) -> Self {
        self.field.buffer.set_text(v);
        self
    }
    pub fn disabled(mut self, d: bool) -> Self {
        self.disabled = d;
        self
    }
    pub fn required(mut self, r: bool) -> Self {
        self.required = r;
        self
    }
    pub fn help(mut self, h: &str) -> Self {
        self.help = h.to_owned();
        self
    }
    pub fn plain_label(mut self) -> Self {
        self.plain_label = true;
        self
    }

    pub fn validator(mut self, v: impl Fn(&str) -> Option<String> + 'static) -> Self {
        self.field.validator = Some(Validator::check(v));
        self
    }

    pub fn validate_when(mut self, when: ValidateWhen) -> Self {
        self.field.validate_when = when;
        self
    }

    pub fn error(&self) -> Option<&str> {
        self.field.error_message()
    }

    pub fn set_error(&mut self, error: Option<String>) {
        self.field.set_error(error.map(FieldError::from));
    }
    pub fn masked(mut self) -> Self {
        self.masked = true;
        self.field.secret = true;
        self
    }

    pub fn context_menu(mut self, on: bool) -> Self {
        self.edit_menu.enabled = on;
        self
    }
    pub fn reveal_tail(mut self, n: u8) -> Self {
        self.reveal_tail = n;
        self
    }

    pub fn wrap_label(mut self, on: bool) -> Self {
        self.wrap_label = on;
        self
    }

    pub fn blur_action(mut self, action: BlurAction) -> Self {
        self.blur_action = action;
        self
    }

    pub fn validate_on(mut self, on: ValidateOn) -> Self {
        self.validate_on = on;
        self
    }

    pub fn edit_entry(mut self, entry: EditEntry) -> Self {
        self.set_edit_entry(entry);
        self
    }

    pub fn set_edit_entry(&mut self, entry: EditEntry) {
        self.edit_entry = Some(entry);
        self.entry = entry;
    }

    #[deprecated(note = "use `edit_entry(EditEntry::TypeToEdit)` or `EditEntry::Explicit`")]
    pub fn type_to_edit(self, on: bool) -> Self {
        self.edit_entry(if on {
            EditEntry::TypeToEdit
        } else {
            EditEntry::Explicit
        })
    }

    pub fn focus_select(mut self, select: FocusSelect) -> Self {
        self.focus_select = select;
        self
    }

    pub fn text_keymap(
        mut self,
        keymap: crate::core::keymap::KeyMap<crate::core::edit::TextCmd>,
    ) -> Self {
        self.field.text_keymap = Some(keymap);
        self
    }

    fn edit_action(&self, key: &Key) -> EditAction {
        self.field.action(key)
    }

    pub fn field_keys(mut self, keys: FieldKeys) -> Self {
        self.field_keys = Some(keys);
        self.field.keys = keys;
        self
    }

    pub fn entry(&self) -> EditEntry {
        self.edit_entry.unwrap_or(self.entry)
    }

    pub fn wants_text_input(&self) -> bool {
        !self.disabled && (self.editing || self.entry() != EditEntry::Explicit)
    }

    pub fn is_dirty(&self) -> bool {
        self.editing && self.field.buffer.text() != self.snapshot
    }

    pub fn can_undo(&self) -> bool {
        self.field.can_undo()
    }

    pub fn hints(&self) -> Vec<crate::widgets::keyhint::Hint> {
        use crate::widgets::keyhint::EDIT_FIELD;

        if let Some(m) = self.edit_menu.menu() {
            return crate::core::hints::HasHints::hints(m, &crate::core::hints::HintCx::new(None));
        }
        if self.disabled {
            Vec::new()
        } else if self.wants_text_input() {
            EDIT_FIELD.to_vec()
        } else {
            vec![crate::widgets::widget_hints::EDIT]
        }
    }

    pub fn pointer_select(mut self, sel: PointerSelect) -> Self {
        self.field.sel = sel;
        self
    }

    pub fn on_blur(&mut self) -> Option<InputAction> {
        if !self.editing {
            return None;
        }

        if self.entry() == EditEntry::Immediate && !self.is_dirty() {
            self.editing = false;
            self.field.buffer.clear_selection();
            return None;
        }
        match self.blur_action {
            BlurAction::Commit => {
                self.commit();
                Some(InputAction::Committed)
            }
            BlurAction::Cancel => {
                self.cancel();
                Some(InputAction::Cancelled)
            }
            BlurAction::Keep => None,
        }
    }

    pub fn clear(&mut self) {
        self.field.buffer.set_text("");
        self.snapshot.clear();
        self.snapshot_error = None;
        self.field.error = None;
        self.field.clear_history();
    }

    fn display_graphemes(&self, editing: bool, mask: &str) -> Vec<(usize, String, usize)> {
        let text = self.field.buffer.text();
        let gs: Vec<(usize, &str)> =
            unicode_segmentation::UnicodeSegmentation::grapheme_indices(text, true).collect();
        let n = gs.len();
        gs.into_iter()
            .enumerate()
            .map(|(i, (bi, g))| {
                let reveal = self.masked
                    && !editing
                    && self.reveal_tail > 0
                    && i + (self.reveal_tail as usize) >= n;
                if self.masked && !reveal {
                    (bi, mask.to_owned(), 1)
                } else {
                    (bi, g.to_owned(), width(g))
                }
            })
            .collect()
    }

    pub fn text(&self) -> &str {
        self.field.buffer.text()
    }

    pub fn begin_edit(&mut self) {
        if self.disabled || self.editing {
            return;
        }
        self.editing = true;
        self.focus_edit = false;
        self.snapshot = self.field.buffer.text().to_owned();
        self.snapshot_error = self.field.error.clone();
        self.field.buffer.clear_selection();
    }

    pub fn commit(&mut self) {
        self.editing = false;
        self.field.buffer.clear_selection();
        let changed = self.field.buffer.text() != self.snapshot;
        self.validate_ended_edit(changed);
    }

    pub fn cancel(&mut self) {
        self.editing = false;
        if self.field.buffer.text() != self.snapshot {
            self.field.record(EditKind::Other);
        }
        let snap = self.snapshot.clone();
        self.field.buffer.set_text(snap);

        let marked = self.snapshot_error.is_some() || self.field.error.is_some();
        self.validate_ended_edit(marked);
    }

    fn validate_ended_edit(&mut self, changed: bool) {
        if changed || self.field.error.is_some() || self.validate_on == ValidateOn::EveryCommit {
            self.validate();
        }
    }

    fn begin_focus_edit(&mut self) {
        if self.disabled || self.editing {
            return;
        }
        self.begin_edit();
        self.focus_select.apply(&mut self.field.buffer);
        self.focus_edit = true;
    }

    pub fn resume(&mut self) {
        if self.entry() == EditEntry::Immediate {
            self.begin_edit();
        }
    }

    pub fn validate(&mut self) -> bool {
        self.sync_required();
        self.field.validate()
    }

    fn sync_required(&mut self) {
        self.field.required = self.required.then(|| {
            self.required_message
                .clone()
                .unwrap_or_else(|| self.required_text.to_owned())
        });
    }

    pub const HEIGHT: u16 = 3;

    pub const GUTTER: u16 = 2;

    pub fn height(&self) -> u16 {
        2 + self.message_rows()
    }

    pub fn height_for(&self, cols: u16) -> u16 {
        self.label_lines(cols).len().max(1) as u16 + 1 + self.message_rows()
    }

    fn message_rows(&self) -> u16 {
        u16::from(self.field.error.is_some() || !self.help.is_empty())
    }

    fn label_lines(&self, cols: u16) -> Vec<String> {
        let room = cols.saturating_sub(Self::GUTTER) as usize;
        if self.wrap_label && room > 0 && width(&self.label) > room {
            crate::ui::text::wrap(&self.label, room)
        } else {
            vec![self.label.clone()]
        }
    }

    pub fn on_key(&mut self, key: &Key) -> (Outcome, Option<InputAction>) {
        if self.disabled {
            return (Outcome::Ignored, None);
        }

        if let Some(o) = edit_menu_key(self, key) {
            return (
                o,
                self.edit_menu.take_edited().then_some(InputAction::Changed),
            );
        }

        if key.ctrl_char('c') && (self.masked || self.field.buffer.selected_text().is_none()) {
            return (Outcome::Ignored, None);
        }
        let entry = self.entry();
        if !self.editing {
            match entry {
                EditEntry::Immediate if self.focus_seen => self.begin_edit(),
                EditEntry::Immediate => self.begin_focus_edit(),
                EditEntry::TypeToEdit | EditEntry::Explicit => {
                    if key.is(KeyCode::Enter) || key.is(KeyCode::F(2)) {
                        self.begin_edit();
                        return (Outcome::Changed, None);
                    }

                    if matches!(self.edit_action(key), EditAction::SelectAll) {
                        self.begin_edit();
                        self.field.buffer.select_all();
                        return (Outcome::Changed, None);
                    }

                    if matches!(self.edit_action(key), EditAction::Paste) {
                        let o = paste_clipboard(|t| self.on_paste(t));
                        return (o, (o == Outcome::Changed).then_some(InputAction::Changed));
                    }
                    if entry == EditEntry::TypeToEdit
                        && let KeyCode::Char(c) = key.code
                        && !key.chord()
                    {
                        self.begin_edit();
                        self.field.buffer.move_doc_end(false);
                        self.sync_required();
                        self.field.record(EditKind::Typing);
                        self.field.buffer.insert_char(c);
                        self.field.after_edit();
                        return (Outcome::Changed, Some(InputAction::Changed));
                    }
                    return (Outcome::Ignored, None);
                }
            }
        }
        let action = self.edit_action(key);
        match action {
            EditAction::Commit => {
                self.commit();
                self.resume();
                (Outcome::Changed, Some(InputAction::Committed))
            }
            EditAction::Cancel => {
                if entry == EditEntry::Immediate && !self.is_dirty() {
                    return (Outcome::Ignored, None);
                }
                self.cancel();
                self.resume();
                (Outcome::Changed, Some(InputAction::Cancelled))
            }
            EditAction::Tab { backward } => {
                self.commit();

                request_focus_next(backward);
                (
                    Outcome::Changed,
                    Some(InputAction::CommittedTab { backward }),
                )
            }

            EditAction::None if entry == EditEntry::Immediate => (Outcome::Ignored, None),

            EditAction::None => (crate::widgets::field_common::unbound_key(key), None),
            action => {
                self.sync_required();
                if self.field.apply(action, key).edited {
                    (Outcome::Changed, Some(InputAction::Changed))
                } else {
                    (Outcome::Changed, None)
                }
            }
        }
    }

    pub fn on_paste(&mut self, text: &str) -> Outcome {
        if self.disabled {
            return Outcome::Ignored;
        }
        if !self.editing {
            if self.entry() == EditEntry::Immediate {
                self.begin_focus_edit();
            } else {
                self.begin_edit();
            }
        }
        self.sync_required();
        self.field.paste(text);
        Outcome::Changed
    }

    fn offset_at(&self, pos: Position) -> usize {
        let glyphs = self.display_graphemes(true, crate::theme::Glyphs::UNICODE.mask);
        self.field.glyph_offset_at(pos, &glyphs)
    }

    pub fn label_hit(mut self, on: bool) -> Self {
        self.label_hit = on;
        self
    }

    fn on_label(&self, pos: Position) -> bool {
        self.label_hit && pos.y < self.area.y
    }

    pub fn on_click(&mut self, pos: Position) -> Outcome {
        self.on_click_count(pos, 1)
    }

    pub fn on_click_count(&mut self, pos: Position, count: u8) -> Outcome {
        PointerField::on_click_count(self, pos, count)
    }

    pub fn on_press(&mut self, pos: Position) -> Outcome {
        self.on_press_count(pos, 1)
    }

    pub fn on_press_count(&mut self, pos: Position, count: u8) -> Outcome {
        PointerField::on_press_count(self, pos, count)
    }

    pub fn on_drag(&mut self, pos: Position) -> Outcome {
        PointerField::on_drag(self, pos)
    }

    pub fn render(
        &mut self,
        area: Rect,
        buf: &mut Buffer,
        ctx: &mut RenderCtx,
        bg: ratatui::style::Color,
    ) {
        self.render_field(area, buf, ctx, bg);

        self.edit_menu.render(self.area, buf, ctx);
    }

    fn render_field(
        &mut self,
        area: Rect,
        buf: &mut Buffer,
        ctx: &mut RenderCtx,
        bg: ratatui::style::Color,
    ) {
        let required = ctx.theme.strings.required;
        if self.required_text != required {
            let old = std::mem::replace(&mut self.required_text, required);
            if self.required_message.is_none()
                && let Some(e) = self.field.error.as_mut()
            {
                crate::theme::Strings::follow(e.message_mut(), old, required);
            }
        }
        self.entry = self.edit_entry.unwrap_or(ctx.theme.edit_entry);
        self.field.keys = self.field_keys.or(ctx.theme.field_keys).unwrap_or_default();

        let has_focus = ctx.interaction.has_focus(self.id) && !ctx.inert;
        if has_focus && !self.focus_seen && self.entry == EditEntry::Immediate {
            self.begin_focus_edit();
        } else if !ctx.interaction.has_focus(self.id)
            && self.entry == EditEntry::Immediate
            && self.editing
            && self.focus_edit
            && !self.is_dirty()
        {
            self.editing = false;
            self.field.buffer.clear_selection();
        }
        self.focus_seen = has_focus;
        let area = area.intersection(*buf.area());
        self.area = Rect::ZERO;
        self.text_area = Rect::ZERO;
        if area.is_empty() {
            return;
        }
        let t = ctx.theme;
        let mut s = ctx.state(self.id);

        let menu_open = self.edit_menu.is_open();
        s.focused = ctx.interaction.focused_under(self.id, menu_open);
        s.disabled = self.disabled;
        s.editing = self.editing && (s.focused || (menu_open && has_focus));
        s.error = self.field.error.is_some();
        if self.disabled {
            s.hovered = false;
        }

        let wrapped = self.label_lines(area.width);
        let label_rows = wrapped.len().max(1) as u16;
        let label_x = area.x + Self::GUTTER.min(area.width);
        let label_style_for = |t: &crate::theme::Theme| {
            if self.disabled {
                t.faint().bg(bg)
            } else {
                t.label(s.focused).bg(bg)
            }
        };
        if label_rows > 1 {
            let st = label_style_for(t);
            for (i, line) in wrapped.iter().enumerate() {
                let y = area.y + i as u16;
                if y >= area.bottom() {
                    break;
                }
                buf.set_string(label_x, y, line, st);
            }
        }

        let mut label = self.label.clone();
        let name_w = width(&self.label);

        let show_optional = !self.required
            && !self.label.is_empty()
            && !self.plain_label
            && name_w + 4 + width(t.strings.optional) <= area.width as usize;
        if self.required {
            label.push_str(" *");
        } else if show_optional {
            label.push_str("  ");
            label.push_str(t.strings.optional);
        }
        let label_style = label_style_for(t);
        if label_rows == 1 {
            buf.set_string(
                label_x,
                area.y,
                crate::ui::text::fit(&label, area.width.saturating_sub(2) as usize),
                label_style,
            );
        }
        if label_rows > 1 {
        } else if self.required && !self.disabled && name_w + 4 <= area.width as usize {
            let ink = if self.field.error.is_some() {
                t.tone_on(crate::theme::Tone::Error, bg)
            } else {
                t.tone_on(crate::theme::Tone::Muted, bg)
            };
            buf.set_string(
                area.x + 2 + name_w as u16 + 1,
                area.y,
                "*",
                ratatui::style::Style::new().fg(ink).bg(bg),
            );
        } else if show_optional {
            buf.set_string(
                area.x + 2 + name_w as u16 + 2,
                area.y,
                t.strings.optional,
                t.faint().bg(bg),
            );
        }

        if area.height < label_rows + 1 {
            return;
        }
        let field = Rect::new(area.x, area.y + label_rows, area.width, 1);
        self.area = field;
        if self.label_hit && !self.label.is_empty() {
            let lw = if label_rows > 1 {
                wrapped.iter().map(|l| width(l)).max().unwrap_or(0)
            } else {
                width(&label)
            }
            .min(area.width.saturating_sub(Self::GUTTER) as usize) as u16;
            let rect = Rect::new(area.x, area.y, Self::GUTTER + lw, label_rows);
            ctx.clickable(self.id, rect.intersection(area));
        }
        let fs = t.field_style(s);
        crate::ui::ctx::fill(buf, field, fs);
        let gutter = t.gutter(s, fs.bg.unwrap_or(bg), false);
        buf.set_string(field.x, field.y, t.gutter_symbol(s), gutter);
        let trailing = if s.error { 2 } else { 0 };

        let readout_w = if self.readout.is_empty() {
            0
        } else {
            width(&self.readout) as u16 + 2
        };
        let readout_w = if field.width >= 3 + trailing + readout_w + 8 {
            readout_w
        } else {
            0
        };
        let inner = Rect::new(
            field.x + 2.min(field.width),
            field.y,
            field.width.saturating_sub(3 + trailing + readout_w),
            1,
        );
        self.readout_area = Rect::ZERO;
        self.text_area = inner;
        if !self.disabled && self.field.sel.drag {
            ctx.autoscroll(inner);
        }
        let text = self.field.buffer.text();
        if text.is_empty() {
            let p = crate::ui::reveal::fit(ctx, inner, &self.placeholder, s.focused);
            buf.set_string(inner.x, inner.y, &p, t.placeholder(s));
            if s.editing && inner.width > 0 {
                ctx.set_cursor(Position::new(inner.x, inner.y));
            }
        } else {
            let glyphs = self.display_graphemes(s.editing, t.glyphs.mask);
            let look = LineLook {
                text: fs,
                selection: t.selection(),
                ellipsis: fs.fg(t.text_muted),
                ellipsis_glyph: t.glyphs.ellipsis,
                underline: s.editing.then_some(t.accent),
                follow_caret: s.editing,
            };
            let caret = self.field.draw_glyphs(buf, inner, &look, &glyphs);
            if s.editing && inner.width > 0 {
                ctx.set_cursor(caret);
            }
        }
        if s.error && field.width >= 2 {
            buf.set_string(
                field.right() - 2,
                field.y,
                "!",
                fs.fg(t.error).add_modifier(Modifier::BOLD),
            );
        }
        ctx.control(self.id, field, self.disabled);

        if self.editing && !self.masked && self.field.buffer.has_selection() {
            ctx.selection(self.id);
        }
        if !self.disabled {
            ctx.pointer_shape(self.id, crate::core::shape::PointerShape::Text);
        }
        if readout_w > 0 {
            let rx = field.right().saturating_sub(trailing + readout_w);
            let r = Rect::new(rx, field.y, readout_w, 1);
            let fbg = fs.bg.unwrap_or(bg);

            let st = match self.readout_link {
                Some(id) if !self.disabled => ctx.title_style(id, fbg),
                _ => fs.fg(t.legible(t.text_muted, fbg)),
            };
            buf.set_string(rx, field.y, format!(" {} ", self.readout), st);
            self.readout_area = r;
            if let Some(id) = self.readout_link
                && !self.disabled
            {
                ctx.clickable(id, r);
            }
        }

        if area.height >= label_rows + 2 {
            let msg_y = field.y + 1;
            let msg = Rect::new(label_x, msg_y, area.width.saturating_sub(2), 1);
            if let Some(e) = &self.field.error {
                buf.set_string(
                    label_x,
                    msg_y,
                    crate::ui::reveal::fit(ctx, msg, e, s.focused),
                    t.error_fg().bg(bg),
                );
            } else if !self.help.is_empty() {
                buf.set_string(
                    label_x,
                    msg_y,
                    crate::ui::reveal::fit(ctx, msg, &self.help, s.focused),
                    t.muted().bg(bg),
                );
            }
        }
    }
}

impl crate::core::focus::BlurTarget for TextInput {
    fn blur_id(&self) -> WidgetId {
        self.id
    }
    fn blur(&mut self) -> Outcome {
        match self.on_blur() {
            Some(_) => Outcome::Changed,
            None => Outcome::Ignored,
        }
    }
}

impl Editable for TextInput {
    fn field(&mut self) -> &mut EditField {
        &mut self.field
    }
    fn edit_id(&self) -> WidgetId {
        self.id
    }
    fn offset_at(&self, pos: Position) -> usize {
        TextInput::offset_at(self, pos)
    }
    fn pointer_enabled(&self) -> bool {
        !self.disabled
    }
    fn whole_word(&self) -> bool {
        self.masked
    }
    fn on_label(&self, pos: Position) -> bool {
        TextInput::on_label(self, pos)
    }
    fn label_click(&mut self) {
        if !self.editing {
            self.begin_focus_edit();
        }
    }
    fn pointer_begin(&mut self) {
        self.begin_edit();
    }
    fn pointer_focus(&mut self) {
        self.focus_seen = true;
        self.focus_edit = false;
    }
}

impl crate::core::pointer::MouseTarget for TextInput {
    type Action = ();

    fn mouse_id(&self) -> WidgetId {
        self.id
    }

    fn pointer(
        &mut self,
        ev: &crate::core::pointer::PointerEvent,
    ) -> crate::core::response::Response<()> {
        use crate::core::intent::Phase;
        let pos = ev.pos();
        if let Some(o) = edit_menu_pointer(self, ev) {
            return o.into();
        }
        match ev.phase() {
            Some(Phase::Press) => self.on_press_count(pos, ev.count().max(1)).into(),
            Some(Phase::Drag) => self.on_drag(pos).into(),
            Some(Phase::Click) => self.on_click_count(pos, ev.count().max(1)).into(),
            _ => crate::core::response::Response::ignored(),
        }
    }

    fn route(
        &mut self,
        ev: &crate::core::pointer::PointerEvent,
    ) -> Option<crate::core::response::Response<()>> {
        edit_menu_route(self, ev)
    }
}

impl EditMenuHost for TextInput {
    fn edit_menu(&mut self) -> &mut EditMenu {
        &mut self.edit_menu
    }
    fn edit_owner(&self) -> WidgetId {
        self.id
    }
    fn edit_caps(&self) -> EditCaps {
        let mut caps = EditCaps::of(&self.field).secret(self.masked);
        caps.editable &= !self.disabled;
        caps
    }
    fn edit_shortcut(&self, cmd: TextCmd) -> Option<String> {
        self.field.shortcut(cmd)
    }
    fn edit_target(&self, ev: &crate::core::pointer::PointerEvent) -> bool {
        editable_target(self, ev)
    }
    fn edit_point(&mut self, pos: Position) {
        point_editable(self, pos);
    }
    fn edit_run(&mut self, cmd: TextCmd) -> bool {
        if self.masked && matches!(cmd, TextCmd::Cut | TextCmd::Copy) {
            return false;
        }
        if !self.editing {
            self.begin_edit();
        }
        self.sync_required();
        self.field.run(cmd).edited
    }
}

#[cfg(test)]
mod issue35_tests {
    use std::cell::Cell;
    use std::rc::Rc;

    use ratatui::buffer::Buffer;
    use ratatui::crossterm::event::KeyCode;
    use ratatui::layout::Rect;

    use super::*;
    use crate::core::event::Key;
    use crate::core::focus::FocusRing;
    use crate::core::hit::HitRegistry;
    use crate::theme::{ColorLevel, Theme};
    use crate::ui::ctx::Interaction;

    const REQUIRED: &str = "Name is required";

    fn key(code: KeyCode) -> Key {
        Key {
            code,
            mods: ratatui::crossterm::event::KeyModifiers::NONE,
        }
    }

    fn painted(input: &mut TextInput) -> String {
        let theme = Theme::for_level(ColorLevel::TrueColor);
        let mut buf = Buffer::empty(Rect::new(0, 0, 60, 4));
        let mut hits = HitRegistry::default();
        let mut ring = FocusRing::default();
        let interaction = Interaction {
            focus: Some(input.id),
            ..Default::default()
        };
        let mut ctx = RenderCtx::new(&theme, interaction, &mut hits, &mut ring);
        input.render(buf.area, &mut buf, &mut ctx, theme.canvas);
        (0..buf.area.height)
            .flat_map(|y| (0..buf.area.width).map(move |x| (x, y)))
            .map(|(x, y)| buf[(x, y)].symbol().to_owned())
            .collect()
    }

    #[test]
    fn issue35_input_required_error_recovers() {
        for when in [ValidateWhen::AfterError, ValidateWhen::Live] {
            let calls = Rc::new(Cell::new(0));
            let counter = calls.clone();
            let mut input = TextInput::new(WidgetId::of("issue35.name"), "Name")
                .edit_entry(EditEntry::Explicit)
                .required(true)
                .required_message(REQUIRED)
                .validate_on(ValidateOn::EveryCommit)
                .validate_when(when)
                .validator(move |text| {
                    counter.set(counter.get() + 1);
                    (text.len() > 40).then(|| "too long".to_owned())
                });

            input.on_key(&key(KeyCode::Enter));
            input.on_key(&key(KeyCode::Enter));
            assert_eq!(input.error(), Some(REQUIRED), "{when:?}");
            assert_eq!(
                calls.get(),
                0,
                "custom rule must not run on empty required text"
            );
            assert!(painted(&mut input).contains(REQUIRED));

            input.on_key(&key(KeyCode::Enter));
            input.on_key(&key(KeyCode::Char('a')));
            assert_eq!(
                input.error(),
                None,
                "{when:?} revalidates the corrected value"
            );
            assert_eq!(calls.get(), 1, "{when:?}");
            assert!(!painted(&mut input).contains(REQUIRED));
        }
    }

    #[test]
    fn issue35_required_after_error_uses_current_configuration() {
        let first = Rc::new(Cell::new(0));
        let first_calls = first.clone();
        let mut input = TextInput::new(WidgetId::of("issue35.current"), "Name")
            .edit_entry(EditEntry::Explicit)
            .required(true)
            .required_message("first message")
            .validate_on(ValidateOn::EveryCommit)
            .validator(move |_| {
                first_calls.set(first_calls.get() + 1);
                None
            });
        input.on_key(&key(KeyCode::Enter));
        input.on_key(&key(KeyCode::Enter));
        assert_eq!(input.error(), Some("first message"));

        let second = Rc::new(Cell::new(0));
        let second_calls = second.clone();
        input.required_message = Some("second message".to_owned());
        input.field.validator = Some(Validator::check(move |_| {
            second_calls.set(second_calls.get() + 1);
            None
        }));

        input.on_key(&key(KeyCode::Enter));
        input.on_key(&key(KeyCode::Char('a')));
        assert_eq!(
            input.error(),
            None,
            "the corrected value clears the prior error"
        );
        assert_eq!(
            second.get(),
            1,
            "the current validator runs on the corrected value"
        );

        input.on_key(&key(KeyCode::Backspace));
        input.on_key(&key(KeyCode::Enter));
        assert_eq!(input.error(), Some("second message"));
        assert_eq!(
            second.get(),
            1,
            "required fires before the current validator"
        );
    }
}
