use ratatui::buffer::Buffer;
use ratatui::crossterm::event::KeyCode;
use ratatui::layout::{Position, Rect};
use ratatui::style::{Color, Modifier};

use crate::core::edit::{EditEntry, EditKind, FieldKeys, FocusSelect, TextCmd};
use crate::core::event::{Key, Outcome};
use crate::core::focus::{BlurAction, request_focus_next};
use crate::core::hit::Axis;
use crate::core::id::WidgetId;
use crate::core::keymap::{Nav, NavKeys};
use crate::core::scroll::{HScroll, ScrollPane, Scrollable};
use crate::core::text::CursorPos;
use crate::ui::ctx::RenderCtx;
use crate::ui::edit_menu::{
    EditCaps, EditMenu, EditMenuHost, edit_menu_key, edit_menu_pointer, edit_menu_route,
    editable_target, point_editable,
};
use crate::ui::text::width;
use crate::widgets::edit::{EditField, Editable};
use crate::widgets::field_common::{EditAction, PointerField, PointerSelect, paste_clipboard};
use crate::widgets::input::InputAction;
use crate::widgets::scrollbar;

impl crate::ui::measure::Measure for TextArea {
    fn measure(
        &self,
        _avail: ratatui::layout::Size,
        _ctx: &crate::ui::ctx::RenderCtx,
    ) -> ratatui::layout::Size {
        let text = self.field.buffer.text();
        let longest = text.split('\n').map(width).max().unwrap_or(0) as u16;
        let label = width(&self.label) as u16 + 2;
        ratatui::layout::Size::new((longest + 5).max(label), self.height())
    }

    fn min_width(&self, _avail: ratatui::layout::Size, _ctx: &crate::ui::ctx::RenderCtx) -> u16 {
        let message = self
            .field
            .error
            .as_ref()
            .map(|e| width(&e.to_string()))
            .unwrap_or(0)
            .max(width(&self.help));
        let help = if message == 0 {
            0
        } else {
            message as u16 + 2 + READOUT_W
        };
        (width(&self.label) as u16 + 2).max(help).max(MIN_W)
    }
}

const MIN_W: u16 = 24;

const READOUT_W: u16 = 14;

#[derive(Debug, Clone)]
pub struct TextArea {
    pub id: WidgetId,
    pub label: String,
    pub placeholder: String,

    pub field: EditField,
    pub disabled: bool,
    pub help: String,
    pub editing: bool,

    pub scroll: ScrollPane,
    pub area: Rect,
    text_area: Rect,
    hscroll: usize,
    last_cursor: Option<CursorPos>,
    follow_cursor: bool,

    pub rows: u16,

    pub blur_action: BlurAction,

    snapshot: String,

    pub nav: NavKeys,

    pub edit_entry: Option<EditEntry>,
    entry: EditEntry,

    pub focus_select: FocusSelect,

    pub esc_reverts: bool,

    pub field_keys: Option<FieldKeys>,
    focus_seen: bool,

    pub horizontal: Option<HScroll>,

    content_cols: usize,

    was_editing: bool,

    pub label_hit: bool,

    focus_edit: bool,

    pub edit_menu: EditMenu,
}

impl TextArea {
    pub fn new(id: WidgetId, label: &str, rows: u16) -> Self {
        Self {
            id,
            label: label.to_owned(),
            placeholder: String::new(),
            field: EditField::multi(""),
            disabled: false,
            help: String::new(),
            editing: false,
            scroll: ScrollPane::default(),
            area: Rect::ZERO,
            text_area: Rect::ZERO,
            hscroll: 0,
            last_cursor: None,
            follow_cursor: false,
            rows,
            blur_action: BlurAction::Commit,
            snapshot: String::new(),
            nav: NavKeys::default(),
            edit_entry: None,
            entry: crate::theme::Theme::DEFAULT_EDIT_ENTRY,
            focus_select: FocusSelect::CaretEnd,
            esc_reverts: false,
            field_keys: None,
            focus_seen: false,
            horizontal: None,
            content_cols: 0,
            was_editing: false,
            label_hit: true,
            focus_edit: false,
            edit_menu: EditMenu::default(),
        }
    }

    pub fn context_menu(mut self, on: bool) -> Self {
        self.edit_menu.enabled = on;
        self
    }

    pub fn edit_entry(mut self, entry: EditEntry) -> Self {
        self.edit_entry = Some(entry);
        self.entry = entry;
        self
    }

    pub fn focus_select(mut self, select: FocusSelect) -> Self {
        self.focus_select = select;
        self
    }

    pub fn esc_reverts(mut self, on: bool) -> Self {
        self.esc_reverts = on;
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

    pub fn hints(&self) -> Vec<crate::widgets::keyhint::Hint> {
        use crate::widgets::keyhint::{EDIT_TEXT, hint};

        if let Some(m) = self.edit_menu.menu() {
            return crate::core::hints::HasHints::hints(m, &crate::core::hints::HintCx::new(None));
        }
        if self.disabled {
            Vec::new()
        } else if self.editing && self.esc_reverts {
            vec![hint("Tab", "Next field"), hint("Esc", "Cancel")]
        } else if self.editing {
            EDIT_TEXT.to_vec()
        } else {
            vec![crate::widgets::widget_hints::EDIT, hint("↑↓", "Scroll")]
        }
    }

    pub fn nav(mut self, nav: NavKeys) -> Self {
        self.nav = nav;
        self
    }

    pub fn pointer_select(mut self, sel: PointerSelect) -> Self {
        self.field.sel = sel;
        self
    }

    pub fn blur_action(mut self, action: BlurAction) -> Self {
        self.blur_action = action;
        self
    }

    pub fn on_blur(&mut self) -> Option<InputAction> {
        if !self.editing {
            return None;
        }
        if self.entry() == EditEntry::Immediate && !self.is_dirty() {
            self.commit();
            return None;
        }
        match self.blur_action {
            BlurAction::Commit => {
                self.commit();
                Some(InputAction::Committed)
            }
            BlurAction::Cancel => {
                self.editing = false;
                let snap = std::mem::take(&mut self.snapshot);
                self.field.buffer.set_text(snap);
                Some(InputAction::Cancelled)
            }
            BlurAction::Keep => None,
        }
    }

    pub fn value(mut self, v: &str) -> Self {
        self.field.set_text(v);
        self.field.buffer.move_doc_start(false);
        self
    }
    pub fn placeholder(mut self, p: &str) -> Self {
        self.placeholder = p.to_owned();
        self
    }
    pub fn disabled(mut self, d: bool) -> Self {
        self.disabled = d;
        self
    }
    pub fn error(mut self, e: Option<&str>) -> Self {
        self.field.error = e.map(crate::widgets::edit::FieldError::from);
        self
    }
    pub fn help(mut self, h: &str) -> Self {
        self.help = h.to_owned();
        self
    }

    pub fn height(&self) -> u16 {
        self.rows.saturating_add(2)
    }

    pub fn begin_edit(&mut self) {
        if !self.disabled {
            if !self.editing {
                self.snapshot = self.field.buffer.text().to_owned();
                self.focus_edit = false;
            }
            self.editing = true;
            self.follow_cursor = true;
        }
    }

    pub fn commit(&mut self) {
        self.editing = false;
        self.field.buffer.clear_selection();
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

        if key.ctrl_char('c') && self.field.buffer.selected_text().is_none() {
            return (Outcome::Ignored, None);
        }
        let entry = self.entry();
        if !self.editing {
            if entry == EditEntry::Immediate && !self.focus_seen {
                self.begin_edit();
                self.focus_select.apply(&mut self.field.buffer);
            } else if matches!(key.code, KeyCode::Enter | KeyCode::F(2)) && key.plain() {
                self.begin_edit();
                return (Outcome::Changed, None);
            } else if matches!(self.edit_action(key), EditAction::SelectAll) {
                self.begin_edit();
                self.field.buffer.select_all();
                return (Outcome::Changed, None);
            } else if matches!(self.edit_action(key), EditAction::Paste) {
                let o = paste_clipboard(|t| self.on_paste(t));
                return (o, (o == Outcome::Changed).then_some(InputAction::Changed));
            } else if entry != EditEntry::Explicit
                && let KeyCode::Char(c) = key.code
                && !key.chord()
            {
                self.begin_edit();
                self.field.record(EditKind::Typing);
                self.field.buffer.insert_char(c);
                return (Outcome::Changed, Some(InputAction::Changed));
            } else {
                let _moved = match self.nav.action(key) {
                    Some(Nav::Up) => self.scroll.scroll_by(-1),
                    Some(Nav::Down) => self.scroll.scroll_by(1),
                    Some(Nav::PageUp) => self.scroll.page_up(),
                    Some(Nav::PageDown) => self.scroll.page_down(),
                    Some(Nav::Home) => self.scroll.jump_start(),
                    Some(Nav::End) => self.scroll.jump_end(),
                    Some(Nav::Left | Nav::Right) | None => return (Outcome::Ignored, None),
                };
                return (Outcome::Changed, None);
            }
        }
        self.follow_cursor = true;
        match self.edit_action(key) {
            EditAction::Cancel if entry != EditEntry::Explicit && self.esc_reverts => {
                let dirty = self.is_dirty();
                self.revert();
                (
                    Outcome::Changed,
                    Some(if dirty {
                        InputAction::Cancelled
                    } else {
                        InputAction::Committed
                    }),
                )
            }
            EditAction::Commit | EditAction::Cancel => {
                self.commit();
                (Outcome::Changed, Some(InputAction::Committed))
            }
            EditAction::Tab { backward } => {
                self.commit();
                request_focus_next(backward);
                (
                    Outcome::Changed,
                    Some(InputAction::CommittedTab { backward }),
                )
            }
            EditAction::None => match key.code {
                KeyCode::PageUp if !key.chord() => {
                    for _ in 0..self.rows {
                        self.field.buffer.move_up(false);
                    }
                    (Outcome::Changed, None)
                }
                KeyCode::PageDown if !key.chord() => {
                    for _ in 0..self.rows {
                        self.field.buffer.move_down(false);
                    }
                    (Outcome::Changed, None)
                }

                _ => (crate::widgets::field_common::unbound_key(key), None),
            },
            action => {
                if self.field.apply(action, key).edited {
                    (Outcome::Changed, Some(InputAction::Changed))
                } else {
                    (Outcome::Changed, None)
                }
            }
        }
    }

    fn revert(&mut self) {
        if self.field.buffer.text() != self.snapshot {
            self.field.record(EditKind::Other);
        }
        self.editing = false;
        let snap = std::mem::take(&mut self.snapshot);
        self.field.buffer.set_text(snap);
    }

    pub fn on_paste(&mut self, text: &str) -> Outcome {
        if self.disabled || (!self.editing && self.entry() == EditEntry::Explicit) {
            return Outcome::Ignored;
        }
        self.begin_edit();
        self.field.paste(text);
        self.follow_cursor = true;
        Outcome::Changed
    }

    fn on_label(&self, pos: Position) -> bool {
        self.label_hit && pos.y < self.area.y
    }

    fn offset_at(&self, pos: Position) -> usize {
        let line = (self.scroll.offset as isize + (pos.y as isize - self.text_area.y as isize))
            .max(0) as usize;
        let col =
            (self.hscroll as isize + (pos.x as isize - self.text_area.x as isize)).max(0) as usize;
        let line = line.min(self.field.buffer.line_count().saturating_sub(1));
        self.field.buffer.offset_at(line, col)
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

    pub fn horizontal(mut self, h: HScroll) -> Self {
        self.horizontal = Some(h);
        self
    }

    pub fn label_hit(mut self, on: bool) -> Self {
        self.label_hit = on;
        self
    }

    pub fn render(&mut self, area: Rect, buf: &mut Buffer, ctx: &mut RenderCtx, bg: Color) {
        self.render_area(area, buf, ctx, bg);

        self.edit_menu.render(self.text_area, buf, ctx);
    }

    fn render_area(&mut self, area: Rect, buf: &mut Buffer, ctx: &mut RenderCtx, bg: Color) {
        self.entry = self.edit_entry.unwrap_or(ctx.theme.edit_entry);
        self.field.keys = self.field_keys.or(ctx.theme.field_keys).unwrap_or_default();
        self.scroll.h_cfg = self.horizontal.unwrap_or(ctx.hscroll);
        self.scroll.begin(ctx.track_click);

        let has_focus = ctx.interaction.has_focus(self.id) && !ctx.inert;
        if has_focus && !self.focus_seen && self.entry == EditEntry::Immediate && !self.editing {
            self.begin_edit();
            self.focus_select.apply(&mut self.field.buffer);
            self.focus_edit = true;
            if self.editing {
                ctx.note_render_mutation();
            }
        } else if !ctx.interaction.has_focus(self.id)
            && self.entry == EditEntry::Immediate
            && self.editing
            && self.focus_edit
            && !self.is_dirty()
        {
            self.commit();
            ctx.note_render_mutation();
        }
        self.focus_seen = has_focus;
        let area = area.intersection(*buf.area());
        if area.is_empty() {
            self.area = Rect::ZERO;
            self.text_area = Rect::ZERO;
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

        let label_style = if self.disabled {
            t.faint().bg(bg)
        } else {
            t.label(s.focused).bg(bg)
        };
        buf.set_string(
            area.x + 2.min(area.width),
            area.y,
            crate::ui::text::truncate(&self.label, area.width.saturating_sub(2) as usize),
            label_style,
        );

        let rows = self.rows.min(area.height.saturating_sub(2));
        if rows == 0 {
            self.area = Rect::ZERO;
            self.text_area = Rect::ZERO;
            return;
        }
        let body = Rect::new(area.x, area.y + 1, area.width, rows);
        let resized = body.width != self.area.width || body.height != self.area.height;
        self.area = body;
        if self.label_hit && !self.label.is_empty() {
            let lw = width(&self.label).min(area.width.saturating_sub(2) as usize) as u16;
            ctx.clickable(self.id, Rect::new(area.x, area.y, lw + 2, 1));
        }
        ctx.control(self.id, body, self.disabled);
        if self.editing && self.field.buffer.has_selection() {
            ctx.selection(self.id);
        }
        if !self.disabled {
            ctx.pointer_shape(self.id, crate::core::shape::PointerShape::Text);
        }
        ctx.scrollable(self.id, body);
        let fs = t.field_style(s);
        crate::ui::ctx::fill(buf, body, fs);
        let gutter = t.gutter(s, fs.bg.unwrap_or(bg), false);
        for y in body.top()..body.bottom() {
            buf.set_string(body.x, y, t.gutter_symbol(s), gutter);
        }
        let inner = Rect::new(
            body.x + 2.min(body.width),
            body.y,
            body.width.saturating_sub(4),
            rows,
        );
        self.text_area = inner;
        if !self.disabled && self.field.sel.drag {
            ctx.autoscroll(inner);
        }

        let text = self.field.buffer.text();
        let lines: Vec<&str> = text.split('\n').collect();
        self.content_cols = lines.iter().map(|l| width(l)).max().unwrap_or(0);
        self.scroll.set_content(lines.len());
        self.scroll.set_viewport(rows as usize);
        let cur = self.field.buffer.cursor_pos();
        if s.editing && (self.follow_cursor || self.last_cursor != Some(cur) || resized) {
            self.scroll.ensure_visible(cur.line);
            let width = inner.width as usize;
            if cur.col < self.hscroll {
                self.hscroll = cur.col;
            } else if width > 0 && cur.col >= self.hscroll.saturating_add(width) {
                self.hscroll = cur.col + 1 - width;
            }
        } else if !s.editing && self.was_editing {
            self.hscroll = 0;
        }
        self.was_editing = s.editing;
        if !s.editing {
            let max = self.content_cols.saturating_sub(inner.width as usize);
            self.hscroll = self.hscroll.min(max);
        }
        self.last_cursor = Some(cur);
        self.follow_cursor = false;
        if text.is_empty() {
            let w = inner.width as usize;
            let wrapped =
                crate::ui::text::wrap_with(&self.placeholder, w, crate::ui::text::WrapOpts::PLAIN);
            let n = (inner.height as usize).max(1).min(wrapped.len());
            for (i, line) in wrapped.iter().take(n).enumerate() {
                let line = if i + 1 == n && wrapped.len() > n {
                    crate::ui::text::truncate(&wrapped[i..].join(" "), w)
                } else {
                    line.clone()
                };
                buf.set_string(inner.x, inner.y + i as u16, &line, t.placeholder(s));
            }
            if s.editing && inner.width > 0 {
                ctx.set_cursor(Position::new(inner.x, inner.y));
            }
        } else {
            let sel = self.field.buffer.selection();
            let mut line_start = 0usize;
            for (li, line) in lines.iter().enumerate() {
                let visible = li >= self.scroll.offset && li < self.scroll.offset + rows as usize;
                if visible {
                    let y = inner.y + (li - self.scroll.offset) as u16;
                    let mut col = 0usize;
                    let clipped_right = width(line) > self.hscroll + inner.width as usize;
                    let available =
                        inner.width as usize - usize::from(clipped_right && inner.width > 0);
                    for (gi, g) in
                        unicode_segmentation::UnicodeSegmentation::grapheme_indices(*line, true)
                    {
                        let gw = width(g);
                        let start = col;
                        col += gw;
                        if col <= self.hscroll {
                            continue;
                        }
                        if start < self.hscroll {
                            if inner.width > 0 {
                                buf.set_string(inner.x, y, t.glyphs.ellipsis, fs.fg(t.text_muted));
                            }
                            continue;
                        }
                        let visible_col = start - self.hscroll;
                        if visible_col + gw > available {
                            break;
                        }
                        let mut st = fs;
                        if let Some(r) = &sel
                            && r.contains(&(line_start + gi))
                        {
                            st = t.selection();
                        }
                        buf.set_string(inner.x + visible_col as u16, y, g, st);
                    }
                    if clipped_right && inner.width > 0 {
                        let st = fs.fg(t.text_muted);
                        buf.set_string(inner.right() - 1, y, t.glyphs.ellipsis, st);
                    }
                    if s.editing && li == cur.line {
                        for xx in inner.x..inner.right() {
                            if let Some(c) = buf.cell_mut(Position::new(xx, y)) {
                                c.set_style(
                                    c.style()
                                        .add_modifier(Modifier::UNDERLINED)
                                        .underline_color(t.border_strong),
                                );
                            }
                        }
                    }
                }
                line_start += line.len() + 1;
            }
            if s.editing
                && self.scroll.visible_range().contains(&cur.line)
                && cur.col >= self.hscroll
                && cur.col - self.hscroll < inner.width as usize
            {
                let cy = inner.y + (cur.line - self.scroll.offset) as u16;
                let cx = inner.x + (cur.col - self.hscroll) as u16;
                ctx.set_cursor(Position::new(cx, cy));
            }
        }

        let rows_area = Rect::new(body.x, body.y, body.width, rows);
        self.scroll
            .draw_with_fade(rows_area, buf, ctx, self.id, s.focused, &[]);
        if self.field.error.is_some() && body.width >= 2 {
            buf.set_string(
                body.right() - 2,
                body.y,
                "!",
                fs.fg(t.error).add_modifier(Modifier::BOLD),
            );
        }

        let fy = body.bottom();
        if fy < area.bottom() {
            let pos = if s.editing {
                format!("ln {}/{}", cur.line + 1, lines.len())
            } else if self.scroll.overflows() {
                scrollbar::position_label(&self.scroll)
            } else {
                String::new()
            };
            let pos = crate::ui::text::truncate(&pos, area.width.saturating_sub(3) as usize);
            let pos_w = if pos.is_empty() {
                0
            } else {
                crate::ui::text::width(&pos) as u16 + 3
            };
            let msg_w = area.width.saturating_sub(2 + pos_w) as usize;
            if let Some(e) = &self.field.error {
                buf.set_string(
                    area.x + 2,
                    fy,
                    crate::ui::text::truncate(e, msg_w),
                    t.error_fg().bg(bg),
                );
            } else if !self.help.is_empty() {
                buf.set_string(
                    area.x + 2,
                    fy,
                    crate::ui::text::truncate(&self.help, msg_w),
                    t.muted().bg(bg),
                );
            }
            if !pos.is_empty() {
                let px = area
                    .right()
                    .saturating_sub(crate::ui::text::width(&pos) as u16 + 1);
                buf.set_string(px, fy, &pos, t.faint().bg(bg));
            }
        }
    }
}

impl crate::core::focus::BlurTarget for TextArea {
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

impl Editable for TextArea {
    fn field(&mut self) -> &mut EditField {
        &mut self.field
    }
    fn edit_id(&self) -> WidgetId {
        self.id
    }
    fn offset_at(&self, pos: Position) -> usize {
        TextArea::offset_at(self, pos)
    }
    fn pointer_enabled(&self) -> bool {
        !self.disabled
    }
    fn on_label(&self, pos: Position) -> bool {
        TextArea::on_label(self, pos)
    }
    fn label_click(&mut self) {
        if !self.editing {
            self.begin_edit();
            self.focus_select.apply(&mut self.field.buffer);
        }
    }
    fn pointer_begin(&mut self) {
        self.begin_edit();
    }
    fn pointer_focus(&mut self) {
        self.focus_seen = true;
        self.focus_edit = false;
    }
    fn pointer_after(&mut self) {
        self.follow_cursor = true;
    }
}

impl crate::ui::specimen::Specimen for TextArea {
    fn specimen_id(&self) -> WidgetId {
        self.id
    }

    fn supports(&self, state: crate::ui::specimen::SpecimenState) -> bool {
        use crate::ui::specimen::SpecimenState::*;
        match state {
            Rest | Hover | Focused | FocusedHover | Editing | Error | Disabled | Empty
            | Overflow | Narrow => true,
            Pressed | Selected | Busy | Loading => false,
        }
    }

    fn specimen_height(&self, _state: crate::ui::specimen::SpecimenState) -> u16 {
        self.height()
    }

    fn specimen(
        &self,
        state: crate::ui::specimen::SpecimenState,
        area: Rect,
        buf: &mut Buffer,
        ctx: &mut RenderCtx,
        bg: Color,
    ) {
        use crate::ui::specimen::SpecimenState;
        let mut a = self.clone();
        a.help.clear();
        let v = state.visual();
        a.disabled |= v.disabled;
        a.editing = v.editing;
        if v.error && a.field.error.is_none() {
            a.field.error = Some("Explain why, in the imperative".into());
        }
        match state {
            SpecimenState::Empty => {
                if a.placeholder.is_empty() {
                    a.placeholder = "Anything to add…".into();
                }
                a = a.value("");
            }
            SpecimenState::Overflow => {
                let long = format!(
                    "{} and a line longer than the area",
                    a.field.buffer.text().lines().next().unwrap_or_default()
                );
                a = a.value(&long);
            }
            _ => {}
        }
        a.render(area, buf, ctx, bg);
        if v.editing && a.text_area.width > 1 {
            let first = a.field.buffer.text().lines().next().unwrap_or_default();
            let x = (a.text_area.x + width(first) as u16).min(a.text_area.right() - 1);
            if let Some(c) = buf.cell_mut(Position::new(x, a.text_area.y)) {
                c.bg = ctx.theme.text_primary;
            }
        }
    }
}

impl Scrollable for TextArea {
    fn pane(&mut self) -> &mut ScrollPane {
        let width = self.text_area.width as usize;
        self.scroll.h.content_len = self.content_cols;
        self.scroll.h.viewport_len = width;
        self.scroll.h.offset = self.hscroll;
        self.scroll.h.clamp();
        &mut self.scroll
    }

    fn after_scroll(&mut self, axis: Axis, _moved: bool) -> bool {
        if axis == Axis::Horizontal {
            self.hscroll = self.scroll.h.offset;
        }
        false
    }
}

impl crate::core::pointer::MouseTarget for TextArea {
    type Action = ();

    fn mouse_id(&self) -> WidgetId {
        self.id
    }

    fn pointer(
        &mut self,
        ev: &crate::core::pointer::PointerEvent,
    ) -> crate::core::response::Response<()> {
        use crate::core::intent::Phase;
        if let Some(o) = edit_menu_pointer(self, ev) {
            return o.into();
        }
        if let Some(r) = crate::core::pointer::scroll_input(self, ev) {
            return r;
        }
        let pos = ev.pos();
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

impl EditMenuHost for TextArea {
    fn edit_menu(&mut self) -> &mut EditMenu {
        &mut self.edit_menu
    }
    fn edit_owner(&self) -> WidgetId {
        self.id
    }
    fn edit_caps(&self) -> EditCaps {
        let mut caps = EditCaps::of(&self.field);
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
        if !self.editing {
            self.begin_edit();
        }
        self.follow_cursor = true;
        self.field.run(cmd).edited
    }
}
