use crate::core::event::{Key, Outcome};
use crate::core::focus::{Focus, FocusRing};
use crate::core::hints::{HasHints, HintCx};
use crate::core::id::WidgetId;
use crate::core::intent::DismissReason;
use crate::core::keymap::{Nav, NavKeys};
use crate::core::scroll::{ScrollPane, Scrollable};
use crate::theme::{Strings, Tone};
use crate::ui::ctx::RenderCtx;
use crate::ui::text::truncate;
use crate::widgets::button::{Button, row_layout_right};
use crate::widgets::choice::{Checkbox, RadioGroup};
use crate::widgets::input::{InputAction, TextInput};
use crate::widgets::keyhint::Hint;
use crate::widgets::scrollbar;
use crate::widgets::select::Select;
use ratatui::buffer::Buffer;
use ratatui::crossterm::event::KeyCode;
use ratatui::layout::{Position, Rect};
use ratatui::style::{Modifier, Style};

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum FieldValue {
    Text(String),
    Choice(usize),
    Bool(bool),
}

pub type FormValues = Vec<(String, FieldValue)>;

pub enum FormFieldKind {
    Input(TextInput),
    Select(Select),
    Check(Checkbox),
    Radio(RadioGroup),

    Chooser {
        label: String,
        value: String,
        detail: Option<String>,
        button: Button,
    },

    Note(Vec<(String, Tone)>),
}

pub struct FormField {
    pub name: String,
    pub kind: FormFieldKind,
    pub visible: bool,
}

impl FormField {
    pub fn input(name: &str, input: TextInput) -> Self {
        Self {
            name: name.into(),
            kind: FormFieldKind::Input(input),
            visible: true,
        }
    }
    pub fn select(name: &str, select: Select) -> Self {
        Self {
            name: name.into(),
            kind: FormFieldKind::Select(select),
            visible: true,
        }
    }
    pub fn check(name: &str, c: Checkbox) -> Self {
        Self {
            name: name.into(),
            kind: FormFieldKind::Check(c),
            visible: true,
        }
    }
    pub fn radio(name: &str, r: RadioGroup) -> Self {
        Self {
            name: name.into(),
            kind: FormFieldKind::Radio(r),
            visible: true,
        }
    }
    pub fn chooser(name: &str, id: WidgetId, label: &str, value: &str, button: &str) -> Self {
        Self {
            name: name.into(),
            kind: FormFieldKind::Chooser {
                label: label.into(),
                value: value.into(),
                detail: None,
                button: Button::secondary(id, button),
            },
            visible: true,
        }
    }
    pub fn note(name: &str, lines: Vec<(String, Tone)>) -> Self {
        Self {
            name: name.into(),
            kind: FormFieldKind::Note(lines),
            visible: true,
        }
    }
    pub fn hidden(mut self) -> Self {
        self.visible = false;
        self
    }

    fn height(&self) -> u16 {
        match &self.kind {
            FormFieldKind::Input(_) => TextInput::HEIGHT,
            FormFieldKind::Select(_) => Select::HEIGHT,
            FormFieldKind::Check(_) => 1,
            FormFieldKind::Radio(r) => r.height(),
            FormFieldKind::Chooser { detail, .. } => 2 + u16::from(detail.is_some()),
            FormFieldKind::Note(l) => l.len() as u16,
        }
    }

    pub fn value(&self) -> FieldValue {
        match &self.kind {
            FormFieldKind::Input(i) => FieldValue::Text(i.text().to_owned()),
            FormFieldKind::Select(s) => FieldValue::Choice(s.selected),
            FormFieldKind::Check(c) => FieldValue::Bool(c.checked),
            FormFieldKind::Radio(r) => FieldValue::Choice(r.selected),
            FormFieldKind::Chooser { value, .. } => FieldValue::Text(value.clone()),
            FormFieldKind::Note(_) => FieldValue::Text(String::new()),
        }
    }

    fn focus_id(&self) -> Option<WidgetId> {
        match &self.kind {
            FormFieldKind::Input(i) => Some(i.id),
            FormFieldKind::Select(s) => Some(s.id),
            FormFieldKind::Check(c) => Some(c.id),
            FormFieldKind::Radio(r) => Some(r.id),
            FormFieldKind::Chooser { button, .. } => Some(button.id),
            FormFieldKind::Note(_) => None,
        }
    }

    fn tab_stop(&self) -> Option<WidgetId> {
        let disabled = match &self.kind {
            FormFieldKind::Input(i) => i.disabled,
            FormFieldKind::Select(s) => s.disabled,
            FormFieldKind::Check(c) => c.disabled,
            FormFieldKind::Radio(r) => r.disabled,
            FormFieldKind::Chooser { button, .. } => button.disabled,
            FormFieldKind::Note(_) => true,
        };
        if disabled { None } else { self.focus_id() }
    }
}

impl HasHints for FormDialog {
    fn hints(&self, cx: &HintCx) -> Vec<Hint> {
        if self.is_editing() {
            return crate::widgets::keyhint::EDIT_FIELD.to_vec();
        }
        let mut v = vec![Hint::new("Tab", "Next field")];
        let button = self
            .actions
            .iter()
            .map(|(_, b)| b)
            .chain([&self.cancel, &self.save])
            .find(|b| cx.focus == Some(b.id));
        match button {
            Some(b) if b.can_activate() => v.push(b.hint()),
            Some(_) => {}
            None if self.save.can_activate() => v.push(self.save.hint()),
            None => {}
        }
        if self.dismiss.escape {
            v.push(Hint::new("Esc", self.cancel.label.clone()));
        }
        v
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum FormAction {
    Changed(String),

    Choose(String),

    Action(String),
    Save,
    Cancel,
}

pub struct FormDialog {
    pub close_button: crate::widgets::icon_button::IconButton,

    pub dismiss: crate::ui::popup::Dismiss,
    pub id: WidgetId,
    pub title: String,
    pub meta: Option<String>,
    pub fields: Vec<FormField>,

    pub actions: Vec<(String, Button)>,
    pub cancel: Button,
    pub save: Button,
    pub width: u16,
    pub error: Option<String>,

    pub scroll: ScrollPane,

    kept: Option<WidgetId>,
    reveal: Option<String>,
    pub area: Rect,
    pub events: Vec<FormAction>,
    pub dirty: bool,

    pub pop_on_save: bool,

    strings: Strings,

    pub nav: NavKeys,

    pub button_row: crate::widgets::button::ButtonRow,
}

impl FormDialog {
    pub fn button_row(mut self, row: crate::widgets::button::ButtonRow) -> Self {
        self.button_row = row;
        self
    }

    pub fn close_button(mut self, button: crate::widgets::icon_button::IconButton) -> Self {
        self.close_button = button;
        self
    }

    pub fn dismiss(mut self, dismiss: crate::ui::popup::Dismiss) -> Self {
        self.dismiss = dismiss;
        self
    }

    pub fn new(id: WidgetId, title: &str, fields: Vec<FormField>) -> Self {
        Self {
            dismiss: crate::ui::popup::Dismiss::ESCAPE,
            close_button: crate::widgets::icon_button::IconButton::close(crate::ui::modal::CLOSE),
            id,
            title: title.to_owned(),
            meta: None,
            fields,
            actions: vec![],
            cancel: Button::subtle(id.sub("cancel"), Strings::DEFAULT.cancel),
            save: Button::primary(id.sub("save"), Strings::DEFAULT.save),
            width: 66,
            error: None,
            scroll: ScrollPane::default(),
            kept: None,
            reveal: None,
            area: Rect::ZERO,
            events: vec![],
            dirty: false,
            pop_on_save: true,
            strings: Strings::DEFAULT,
            nav: NavKeys::NO_VIM,
            button_row: crate::widgets::button::ButtonRow::new(),
        }
    }

    pub fn keep_open_on_save(mut self) -> Self {
        self.pop_on_save = false;
        self
    }

    pub fn action(mut self, name: &str, button: Button) -> Self {
        self.actions.push((name.into(), button));
        self
    }

    pub fn width(mut self, w: u16) -> Self {
        self.width = w;
        self
    }

    pub fn meta(mut self, m: &str) -> Self {
        self.meta = Some(m.to_owned());
        self
    }

    pub fn field(&self, name: &str) -> Option<&FormField> {
        self.fields.iter().find(|f| f.name == name)
    }

    pub fn field_mut(&mut self, name: &str) -> Option<&mut FormField> {
        self.fields.iter_mut().find(|f| f.name == name)
    }

    fn changed_field(&mut self, name: String) {
        self.dirty = true;
        self.events.push(FormAction::Changed(name));
    }

    pub fn set_visible(&mut self, name: &str, v: bool) {
        if let Some(f) = self.field_mut(name) {
            f.visible = v;
        }
    }

    pub fn text(&self, name: &str) -> String {
        match self.field(name).map(|f| f.value()) {
            Some(FieldValue::Text(s)) => s,
            _ => String::new(),
        }
    }

    pub fn choice(&self, name: &str) -> usize {
        match self.field(name).map(|f| f.value()) {
            Some(FieldValue::Choice(i)) => i,
            _ => 0,
        }
    }

    pub fn set_text(&mut self, name: &str, v: &str) {
        if let Some(f) = self.field_mut(name) {
            match &mut f.kind {
                FormFieldKind::Input(i) => i.set_value(v),
                FormFieldKind::Chooser { value, .. } => *value = v.to_owned(),
                _ => {}
            }
        }
    }

    pub fn set_chooser(&mut self, name: &str, value: &str, detail: Option<&str>) {
        if let Some(f) = self.field_mut(name)
            && let FormFieldKind::Chooser {
                value: v,
                detail: d,
                ..
            } = &mut f.kind
        {
            *v = value.to_owned();
            *d = detail.map(str::to_owned);
        }
    }

    pub fn set_note(&mut self, name: &str, lines: Vec<(String, Tone)>) {
        if let Some(f) = self.field_mut(name)
            && let FormFieldKind::Note(l) = &mut f.kind
        {
            *l = lines;
        }
    }

    /// Scroll the named field into view on the next frame. Focus scrolls the
    /// body to the focused field; this is for an answer that arrives while
    /// focus stays on a pinned button, such as the note a check fills in. A
    /// form whose body fits does not move.
    pub fn reveal(&mut self, name: &str) {
        self.reveal = Some(name.to_owned());
    }

    pub fn values(&self) -> FormValues {
        self.fields
            .iter()
            .map(|f| (f.name.clone(), f.value()))
            .collect()
    }

    pub fn initial_focus(&self) -> WidgetId {
        self.fields
            .iter()
            .filter(|f| f.visible)
            .find_map(|f| f.focus_id())
            .unwrap_or(self.save.id)
    }

    pub fn is_editing(&self) -> bool {
        self.fields
            .iter()
            .any(|f| matches!(&f.kind, FormFieldKind::Input(i) if i.editing))
    }

    fn save_event(&self) -> FormAction {
        if self.pop_on_save {
            FormAction::Save
        } else {
            FormAction::Action("save".into())
        }
    }

    fn submit(&mut self, focus: &mut Focus) {
        let mut first_invalid = None;
        for f in self.fields.iter_mut().filter(|f| f.visible) {
            if let FormFieldKind::Input(i) = &mut f.kind
                && !i.disabled
                && !i.validate()
                && first_invalid.is_none()
            {
                first_invalid = Some(i.id);
            }
        }
        match first_invalid {
            Some(id) => focus.focus(id),
            None => self.events.push(self.save_event()),
        }
    }

    fn any_open_select(&self) -> bool {
        self.fields
            .iter()
            .any(|f| matches!(&f.kind, FormFieldKind::Select(s) if s.open))
    }

    fn dismiss_escape(&mut self) -> Outcome {
        crate::ui::layer::dismiss(self, DismissReason::Esc)
            .map_or(Outcome::Consumed, |response| response.outcome())
    }

    pub fn on_key(&mut self, key: &Key, focus: &mut Focus, ring: &FocusRing) -> Outcome {
        let cur = focus.current();
        let editing = self.is_editing();
        let open = self.any_open_select();
        if !editing && !open {
            match key.code {
                KeyCode::Esc => return self.dismiss_escape(),
                KeyCode::Tab => {
                    focus.next(ring);
                    return Outcome::Changed;
                }
                KeyCode::BackTab => {
                    focus.prev(ring);
                    return Outcome::Changed;
                }
                _ => {}
            }
        }
        for f in self.fields.iter_mut() {
            if !f.visible {
                continue;
            }
            let name = f.name.clone();
            match &mut f.kind {
                FormFieldKind::Input(i) if cur == Some(i.id) => {
                    let (o, ev) = i.on_key(key);
                    match ev {
                        Some(InputAction::CommittedTab { backward }) => {
                            if backward {
                                focus.prev(ring)
                            } else {
                                focus.next(ring)
                            }
                            self.dirty = true;
                            self.events.push(FormAction::Changed(name));
                            return Outcome::Changed;
                        }
                        Some(InputAction::Committed) => {
                            self.dirty = true;
                            self.events.push(FormAction::Changed(name));
                            return Outcome::Changed;
                        }
                        Some(InputAction::Changed) => {
                            self.dirty = true;
                        }
                        _ => {}
                    }
                    if o.consumed() {
                        return o;
                    }
                }
                FormFieldKind::Select(s) if cur == Some(s.id) => {
                    let (o, ev) = s.on_key(key);
                    if ev.is_some() {
                        self.dirty = true;
                        self.events.push(FormAction::Changed(name));
                    }
                    if o.consumed() {
                        return o;
                    }
                }
                FormFieldKind::Check(c) if cur == Some(c.id) => {
                    let before = (c.checked, c.mixed);
                    let o = c.on_key(key);
                    if (c.checked, c.mixed) != before {
                        self.changed_field(name);
                        return o;
                    }
                    if o.consumed() {
                        return o;
                    }
                }
                FormFieldKind::Radio(r) if cur == Some(r.id) => {
                    let before = r.selected;
                    let o = r.on_key(key);
                    if r.selected != before {
                        self.changed_field(name);
                        return o;
                    }
                    if o.consumed() {
                        return o;
                    }
                }
                FormFieldKind::Chooser { button, .. } if cur == Some(button.id) => {
                    let (o, fired) = button.on_key(key);
                    if fired {
                        self.events.push(FormAction::Choose(name));
                        return Outcome::Changed;
                    }
                    if o.consumed() {
                        return o;
                    }
                }
                _ => {}
            }
        }
        for (name, b) in self.actions.iter_mut() {
            if cur == Some(b.id) {
                let (o, fired) = b.on_key(key);
                if fired {
                    self.events.push(FormAction::Action(name.clone()));
                    return Outcome::Changed;
                }
                if o.consumed() {
                    return o;
                }
            }
        }
        if cur == Some(self.cancel.id) {
            let (o, fired) = self.cancel.on_key(key);
            if fired {
                self.events.push(FormAction::Cancel);
                return Outcome::Changed;
            }
            if o.consumed() {
                return o;
            }
        }
        if cur == Some(self.save.id) {
            let (o, fired) = self.save.on_key(key);
            if fired {
                self.submit(focus);
                return Outcome::Changed;
            }
            if o.consumed() {
                return o;
            }
        }
        match key.code {
            KeyCode::Enter if !editing => {
                if self.save.can_activate() {
                    self.submit(focus);
                } else {
                    focus.focus(self.initial_focus());
                }
                Outcome::Changed
            }
            _ if !editing && matches!(self.nav.action(key), Some(Nav::Left | Nav::Right)) => {
                let row = self
                    .actions
                    .iter()
                    .map(|(_, b)| b)
                    .chain([&self.cancel, &self.save]);
                let forward = self.nav.action(key) == Some(Nav::Right);
                if let Some(id) = self.button_row.step(row, cur, forward) {
                    focus.focus(id);
                }
                Outcome::Changed
            }

            KeyCode::Esc => self.dismiss_escape(),
            _ => Outcome::Consumed,
        }
    }

    fn inputs(&mut self) -> impl Iterator<Item = &mut TextInput> {
        self.fields.iter_mut().filter_map(|f| match &mut f.kind {
            FormFieldKind::Input(i) => Some(i),
            _ => None,
        })
    }

    pub fn on_press(&mut self, id: WidgetId, pos: Position, count: u8) -> Outcome {
        if let Some(s) = self.select_owning(id) {
            return s.on_press(id, pos);
        }
        self.inputs()
            .find(|i| i.id == id)
            .map_or(Outcome::Ignored, |i| i.on_press_count(pos, count))
    }

    pub fn on_drag(&mut self, pressed: WidgetId, pos: Position) -> Outcome {
        if scrollbar::is_bar_of(pressed, self.id) {
            return match self.pane().bar_of(pressed) {
                Some(bar) => self.bar_input(&bar, pos, false).outcome(),
                None => Outcome::Consumed,
            };
        }
        if let Some(s) = self.select_owning(pressed) {
            return s.on_drag(pressed, pos);
        }
        self.inputs()
            .find(|i| i.id == pressed)
            .map_or(Outcome::Ignored, |i| i.on_drag(pos))
    }

    pub fn on_release(&mut self, pressed: WidgetId, pos: Position) -> Outcome {
        for f in self.fields.iter_mut() {
            if let FormFieldKind::Select(s) = &mut f.kind
                && s.contains_id(pressed)
            {
                let (o, ev) = s.on_release(pos);
                if ev.is_some() {
                    self.dirty = true;
                    self.events.push(FormAction::Changed(f.name.clone()));
                }
                return o;
            }
        }
        Outcome::Ignored
    }

    fn select_owning(&mut self, id: WidgetId) -> Option<&mut Select> {
        self.fields.iter_mut().find_map(|f| match &mut f.kind {
            FormFieldKind::Select(s) if s.contains_id(id) => Some(s),
            _ => None,
        })
    }

    pub fn on_click(&mut self, id: WidgetId, pos: Position, focus: &mut Focus) -> Outcome {
        self.on_click_count(id, pos, 1, focus)
    }

    pub fn on_click_count(
        &mut self,
        id: WidgetId,
        pos: Position,
        count: u8,
        focus: &mut Focus,
    ) -> Outcome {
        if scrollbar::is_bar_of(id, self.id) {
            return self.bar_press(pos).outcome().or(Outcome::Consumed);
        }
        for f in self.fields.iter_mut() {
            let name = f.name.clone();
            match &mut f.kind {
                FormFieldKind::Input(i) if i.id == id => {
                    focus.focus(id);
                    return i.on_click_count(pos, count.max(1));
                }
                FormFieldKind::Select(s) if s.contains_id(id) => {
                    focus.focus(s.id);
                    let (o, ev) = s.on_click(id);
                    if ev.is_some() {
                        self.dirty = true;
                        self.events.push(FormAction::Changed(name));
                    }
                    return o.or(Outcome::Changed);
                }
                FormFieldKind::Check(c) if c.id == id => {
                    focus.focus(id);
                    let before = (c.checked, c.mixed);
                    let o = c.on_click();
                    if (c.checked, c.mixed) != before {
                        self.changed_field(name);
                    }
                    return o;
                }
                FormFieldKind::Radio(r) => {
                    for i in 0..r.options.len() {
                        if r.option_id(i) == id {
                            focus.focus(r.id);
                            let before = r.selected;
                            let o = r.on_click(i);
                            if r.selected != before {
                                self.changed_field(name);
                            }
                            return o;
                        }
                    }
                }
                FormFieldKind::Chooser { button, .. } if button.id == id => {
                    focus.focus(id);
                    if button.on_click() {
                        self.events.push(FormAction::Choose(name));
                    }
                    return Outcome::Changed;
                }
                _ => {}
            }
        }
        for (name, b) in self.actions.iter_mut() {
            if b.id == id {
                focus.focus(id);
                if b.on_click() {
                    self.events.push(FormAction::Action(name.clone()));
                }
                return Outcome::Changed;
            }
        }
        if id == self.cancel.id {
            focus.focus(id);
            if self.cancel.on_click() {
                self.events.push(FormAction::Cancel);
            }
            return Outcome::Changed;
        }
        if id == self.save.id {
            focus.focus(id);
            if self.save.on_click() {
                self.submit(focus);
            }
            return Outcome::Changed;
        }

        for f in self.fields.iter_mut() {
            if let FormFieldKind::Select(s) = &mut f.kind {
                s.close();
            }
        }
        Outcome::Consumed
    }

    pub fn on_blur(&mut self, ch: crate::core::focus::FocusChange) -> Outcome {
        for f in self.fields.iter_mut() {
            match &mut f.kind {
                FormFieldKind::Input(i) if i.id == ch.from => {
                    return match i.on_blur() {
                        Some(InputAction::Committed) => {
                            self.dirty = true;
                            self.events.push(FormAction::Changed(f.name.clone()));
                            Outcome::Changed
                        }
                        Some(_) => Outcome::Changed,
                        None => Outcome::Ignored,
                    };
                }
                FormFieldKind::Select(s) if s.id == ch.from => return s.on_blur(),
                _ => {}
            }
        }
        Outcome::Ignored
    }

    pub fn on_paste(&mut self, text: &str) -> Outcome {
        for f in self.fields.iter_mut() {
            if let FormFieldKind::Input(i) = &mut f.kind
                && i.editing
            {
                return i.on_paste(text);
            }
        }
        Outcome::Consumed
    }

    fn content_height(&self) -> u16 {
        let mut h = 0u16;
        for f in self.fields.iter().filter(|f| f.visible) {
            h += f.height() + 1;
        }
        h
    }

    pub fn nav(mut self, nav: NavKeys) -> Self {
        self.nav = nav;
        self
    }

    pub fn render(&mut self, screen: Rect, buf: &mut Buffer, ctx: &mut RenderCtx) {
        self.scroll.begin(ctx.track_click);
        let s = ctx.theme.strings;
        let old = std::mem::replace(&mut self.strings, s);
        Strings::follow(&mut self.cancel.label, old.cancel, s.cancel);
        Strings::follow(&mut self.save.label, old.save, s.save);
        let content = self.content_height();
        let err = u16::from(self.error.is_some());
        let h = 2 + 2 + content + err + 2;
        let (area, inner) = crate::ui::modal::Frame::new(self.width, h)
            .title(&self.title)
            .meta(self.meta.as_deref())
            .overlay(&*self)
            .render(screen, buf, ctx);
        self.area = area;
        let t = ctx.theme;
        let bg = t.surface_elevated;
        let body_h = inner.height.saturating_sub(3 + err);
        self.scroll.set_content(content as usize);
        self.scroll.set_viewport(body_h as usize);

        let cur = ctx.interaction.focus;
        let moved = cur != self.kept;
        self.kept = cur;
        let mut y_off = 0u16;
        for f in self.fields.iter().filter(|f| f.visible) {
            if moved && f.focus_id().is_some() && f.focus_id() == cur {
                let top = y_off as usize;
                let bottom = (y_off + f.height()) as usize;
                if top < self.scroll.offset {
                    self.scroll.scroll_to(top);
                } else if bottom > self.scroll.offset + body_h as usize {
                    self.scroll
                        .scroll_to(bottom.saturating_sub(body_h as usize));
                }
            }
            y_off += f.height() + 1;
        }
        if let Some(name) = self.reveal.take() {
            let mut top = 0usize;
            for f in self.fields.iter().filter(|f| f.visible) {
                let bottom = top + f.height() as usize + 1;
                if f.name == name {
                    if top < self.scroll.offset {
                        self.scroll.scroll_to(top);
                    } else if bottom > self.scroll.offset + body_h as usize {
                        self.scroll
                            .scroll_to(bottom.saturating_sub(body_h as usize).min(top));
                    }
                    break;
                }
                top = bottom;
            }
        }
        let body = Rect::new(inner.x, inner.y, inner.width, body_h);
        let mut y: i32 = body.y as i32 - self.scroll.offset as i32;
        let mut open_select: Option<usize> = None;
        for (fi, f) in self.fields.iter_mut().enumerate() {
            if !f.visible {
                continue;
            }
            let fh = f.height() as i32;
            let whole = y >= body.y as i32 && y < body.bottom() as i32;
            if !whole && let Some(id) = f.tab_stop() {
                ctx.focus_stop(id);
            }
            if y + fh > body.y as i32 && y < body.bottom() as i32 {
                let yy = y.max(body.y as i32) as u16;
                let clip_top = (body.y as i32 - y).max(0) as u16;
                let avail = (body.bottom() as i32 - y.max(body.y as i32)).max(0) as u16;
                let r = Rect::new(
                    inner.x - 1,
                    yy,
                    inner.width + 1,
                    avail.min(fh as u16 - clip_top),
                );
                if clip_top == 0 && !r.is_empty() {
                    match &mut f.kind {
                        FormFieldKind::Input(i) => i.render(r, buf, ctx, bg),
                        FormFieldKind::Select(s) => {
                            if s.open {
                                open_select = Some(fi);
                            } else {
                                s.render(r, buf, ctx, bg);
                            }
                        }
                        FormFieldKind::Check(c) => c.render(r, buf, ctx, bg),
                        FormFieldKind::Radio(rg) => rg.render(r, buf, ctx, bg),
                        FormFieldKind::Chooser {
                            label,
                            value,
                            detail,
                            button,
                        } => {
                            let focused = ctx.interaction.focused(button.id);
                            buf.set_string(r.x + 2, r.y, label.as_str(), t.label(focused).bg(bg));
                            if r.height >= 2 {
                                let bw = button.width();
                                let vw = r.width.saturating_sub(bw + 4) as usize;
                                buf.set_string(
                                    r.x + 2,
                                    r.y + 1,
                                    truncate(value, vw),
                                    t.primary().bg(bg),
                                );
                                button.render(
                                    Rect::new(r.right().saturating_sub(bw + 1), r.y + 1, bw, 1),
                                    buf,
                                    ctx,
                                    bg,
                                );
                            }
                            if r.height >= 3
                                && let Some(d) = detail
                            {
                                buf.set_string(
                                    r.x + 2,
                                    r.y + 2,
                                    truncate(d, r.width.saturating_sub(3) as usize),
                                    t.muted().bg(bg),
                                );
                            }
                        }
                        FormFieldKind::Note(lines) => {
                            for (i, (l, tone)) in lines.iter().enumerate() {
                                if (i as u16) < r.height {
                                    buf.set_string(
                                        r.x + 2,
                                        r.y + i as u16,
                                        truncate(l, r.width.saturating_sub(3) as usize),
                                        Style::new().fg(t.tone(*tone)).bg(bg),
                                    );
                                }
                            }
                        }
                    }
                }
            }
            y += fh + 1;
        }
        if self.scroll.overflows() {
            let rows = Rect::new(
                area.x,
                body.y,
                (area.right() - 1).saturating_sub(area.x),
                body.height,
            );
            self.scroll
                .draw_with_fade(rows, buf, ctx, self.id, true, &[]);
        }
        if let Some(e) = &self.error {
            let ey = inner.bottom().saturating_sub(2);
            buf.set_string(
                inner.x,
                ey,
                truncate(&format!("! {e}"), inner.width as usize),
                t.error_fg().bg(bg),
            );
            buf.set_string(
                inner.x,
                ey,
                "!",
                t.error_fg().bg(bg).add_modifier(Modifier::BOLD),
            );
        }
        let ay = inner.bottom().saturating_sub(1);
        let mut widths: Vec<u16> = self.actions.iter().map(|(_, b)| b.width()).collect();
        widths.push(self.cancel.width());
        widths.push(self.save.width());
        let rects = row_layout_right(Rect::new(inner.x, ay, inner.width, 1), &widths, 1);
        let n = self.actions.len();
        for (i, (_, b)) in self.actions.iter_mut().enumerate() {
            b.render(rects[i], buf, ctx, bg);
        }
        self.cancel.render(rects[n], buf, ctx, bg);
        self.save.render(rects[n + 1], buf, ctx, bg);

        for f in &self.fields {
            if !f.visible {
                continue;
            }
            match &f.kind {
                FormFieldKind::Input(i) => ctx.hits.register(i.id, i.area),
                FormFieldKind::Select(s) => ctx.hits.register(s.id, s.area),
                FormFieldKind::Check(c) => ctx.hits.register(c.id, c.area),
                FormFieldKind::Radio(r) => {
                    for (i, a) in r.areas.iter().enumerate() {
                        let part = RadioGroup::option_part(i);
                        ctx.hits.register_part_as(r.option_id(i), r.id, part, *a);
                    }
                }
                FormFieldKind::Chooser { button, .. } => ctx.hits.register(button.id, button.area),
                FormFieldKind::Note(_) => {}
            }
        }
        for (_, b) in &self.actions {
            ctx.hits.register(b.id, b.area);
        }
        ctx.hits.register(self.cancel.id, self.cancel.area);
        ctx.hits.register(self.save.id, self.save.area);

        if let Some(fi) = open_select {
            let mut y: i32 = body.y as i32 - self.scroll.offset as i32;
            for (i, f) in self.fields.iter_mut().enumerate() {
                if !f.visible {
                    continue;
                }
                if i == fi
                    && let FormFieldKind::Select(s) = &mut f.kind
                {
                    let r = Rect::new(
                        inner.x - 1,
                        y.max(body.y as i32) as u16,
                        inner.width + 1,
                        Select::HEIGHT,
                    );
                    s.render(r, buf, ctx, bg);
                    ctx.hits.register(s.id, s.area);
                    for k in 0..s.options.len() {
                        ctx.hits.raise(s.option_id(k));
                    }
                }
                y += f.height() as i32 + 1;
            }
        }
    }
}

impl Scrollable for FormDialog {
    fn pane(&mut self) -> &mut ScrollPane {
        &mut self.scroll
    }
}

impl FormDialog {
    pub fn pointer_in(
        &mut self,
        ev: &crate::core::pointer::PointerEvent,
        focus: &mut Focus,
    ) -> crate::core::response::Response<()> {
        use crate::core::intent::Phase;
        use crate::core::pointer::MouseTarget;
        use crate::core::response::Response;
        for f in self.fields.iter_mut() {
            match &mut f.kind {
                FormFieldKind::Select(s) => {
                    let Some(mut r) = s.route(ev) else {
                        continue;
                    };

                    if ev.wheel().is_some() && !r.is_consumed() {
                        break;
                    }
                    if ev.is(Phase::Click) {
                        focus.focus(s.id);
                    }
                    if r.take_action().is_some() {
                        self.dirty = true;
                        self.events.push(FormAction::Changed(f.name.clone()));
                    }
                    return r.erase();
                }
                FormFieldKind::Input(i) if ev.wheel().is_none() => {
                    let Some(r) = i.route(ev) else {
                        continue;
                    };
                    if ev.is(Phase::Click) {
                        focus.focus(i.id);
                    }
                    return r;
                }
                _ => {}
            }
        }
        if ev.wheel().is_some() && !ev.on(self.id) {
            return Response::ignored();
        }
        if ev.on(self.id)
            && let Some(r) = crate::core::pointer::scroll_input(self, ev)
        {
            return r;
        }
        match ev.phase() {
            Some(Phase::Click) => self
                .on_click_count(ev.id, ev.pos(), ev.count(), focus)
                .into(),
            _ => Response::ignored(),
        }
    }
}

#[cfg(test)]
mod tests;
