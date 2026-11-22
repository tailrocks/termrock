use ratatui::buffer::Buffer;
use ratatui::crossterm::event::KeyCode;
use ratatui::layout::{Position, Rect};
use ratatui::style::{Modifier, Style};
use termrock_collections::list::ListBox;
use termrock_controls::button::{Button, row_layout_right, row_lines};
use termrock_controls::choice::{Checkbox, RadioGroup};
use termrock_controls::icon_button::IconButton;
use termrock_controls::keyhint::Hint;
use termrock_controls::scrollbar;
use termrock_core::event::{Key, Outcome};
use termrock_core::focus::{Focus, FocusChange, FocusRing};
use termrock_core::hints::{HasHints, HintCx};
use termrock_core::hit::Axis;
use termrock_core::id::WidgetId;
use termrock_core::intent::DismissReason;
use termrock_core::keymap::{Nav, NavKeys};
use termrock_core::pointer::PointerEvent;
use termrock_core::response::Response;
use termrock_core::scroll::{ScrollPane, Scrollable};
use termrock_fields::input::{InputAction, TextInput};
use termrock_fields::select::Select;
pub use termrock_overlays::layer::FormAction;
use termrock_overlays::layer::{
    Layer, LayerCx, LayerOut, LayerResult, LayerSpec, Overlay, layer_out,
};
use termrock_runtime::ctx::RenderCtx;
use termrock_runtime::measure::terminal_extent;
use termrock_runtime::popup::Dismiss;
use termrock_text::cells::truncate;
use termrock_theme::{Strings, Tone};

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
    List {
        list: Box<ListBox>,
        rows: u16,
    },
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
    pub wrap: bool,
}

impl FormField {
    pub fn input(name: &str, input: TextInput) -> Self {
        Self {
            name: name.into(),
            kind: FormFieldKind::Input(input),
            visible: true,
            wrap: false,
        }
    }
    pub fn select(name: &str, select: Select) -> Self {
        Self {
            name: name.into(),
            kind: FormFieldKind::Select(select),
            visible: true,
            wrap: false,
        }
    }
    pub fn check(name: &str, c: Checkbox) -> Self {
        Self {
            name: name.into(),
            kind: FormFieldKind::Check(c),
            visible: true,
            wrap: false,
        }
    }
    pub fn radio(name: &str, r: RadioGroup) -> Self {
        Self {
            name: name.into(),
            kind: FormFieldKind::Radio(r),
            visible: true,
            wrap: false,
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
            wrap: false,
        }
    }
    pub fn note(name: &str, lines: Vec<(String, Tone)>) -> Self {
        Self {
            name: name.into(),
            kind: FormFieldKind::Note(lines),
            visible: true,
            wrap: false,
        }
    }
    pub fn list(name: &str, list: ListBox, rows: u16) -> Self {
        Self {
            name: name.into(),
            kind: FormFieldKind::List {
                list: Box::new(list.skip_when_empty(true)),
                rows: rows.max(1),
            },
            visible: true,
            wrap: false,
        }
    }

    pub fn wrap(mut self, on: bool) -> Self {
        self.wrap = on;
        self
    }

    pub fn hidden(mut self) -> Self {
        self.visible = false;
        self
    }

    pub fn value(&self) -> FieldValue {
        match &self.kind {
            FormFieldKind::Input(i) => FieldValue::Text(i.text().to_owned()),
            FormFieldKind::Select(s) => FieldValue::Choice(s.selected),
            FormFieldKind::List { list, .. } => FieldValue::Choice(list.cursor),
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
            FormFieldKind::List { list, .. } => Some(list.id),
            FormFieldKind::Check(c) => Some(c.id),
            FormFieldKind::Radio(r) => Some(r.id),
            FormFieldKind::Chooser { button, .. } => Some(button.id),
            FormFieldKind::Note(_) => None,
        }
    }

    fn owns(&self, id: WidgetId) -> bool {
        match &self.kind {
            FormFieldKind::Input(i) => i.id == id,
            FormFieldKind::Select(s) => s.contains_id(id),
            FormFieldKind::List { list, .. } => list.id == id,
            FormFieldKind::Check(c) => c.id == id,
            FormFieldKind::Radio(r) => {
                r.id == id || (0..r.options.len()).any(|i| r.option_id(i) == id)
            }
            FormFieldKind::Chooser { button, .. } => button.id == id,
            FormFieldKind::Note(_) => false,
        }
    }

    /// Ends a hidden field's transient interaction. Returns true when an
    /// input's uncommitted edit survives as the field's value, so the caller
    /// reports it like any other committed change.
    fn end_interaction(&mut self) -> bool {
        match &mut self.kind {
            FormFieldKind::Input(i) if i.editing || i.field.pressed() || i.edit_menu.is_open() => {
                let typed = i.is_dirty().then(|| i.text().to_owned());
                let _ = i.on_blur();
                let text = i.text().to_owned();
                i.set_value(&text);
                typed.is_some_and(|typed| typed == text)
            }
            FormFieldKind::List { list, .. } => {
                list.scroll.release();
                false
            }
            FormFieldKind::Select(s) => {
                s.close();
                let _ = s.on_release(Position::ORIGIN);
                false
            }
            _ => false,
        }
    }

    fn tab_stop(&self) -> Option<WidgetId> {
        let disabled = match &self.kind {
            FormFieldKind::Input(i) => i.disabled,
            FormFieldKind::Select(s) => s.disabled,
            FormFieldKind::List { list, .. } => !list.has_rows(list.items.len()),
            FormFieldKind::Check(c) => c.disabled,
            FormFieldKind::Radio(r) => r.disabled,
            FormFieldKind::Chooser { button, .. } => button.disabled,
            FormFieldKind::Note(_) => true,
        };
        if disabled { None } else { self.focus_id() }
    }

    /// Forgets where the last frame drew the field, so a field the next frame
    /// does not draw leaves no hit region behind.
    fn clear_geometry(&mut self) {
        match &mut self.kind {
            FormFieldKind::Input(i) => i.area = Rect::ZERO,
            FormFieldKind::Select(s) => s.area = Rect::ZERO,
            FormFieldKind::List { list, .. } => list.area = Rect::ZERO,
            FormFieldKind::Check(c) => c.area = Rect::ZERO,
            FormFieldKind::Radio(r) => r.areas.clear(),
            FormFieldKind::Chooser { button, .. } => button.area = Rect::ZERO,
            FormFieldKind::Note(_) => {}
        }
    }
}

struct FieldLayout {
    index: usize,
    top: usize,
    height: usize,
    wrapped: Option<Vec<(String, Tone)>>,
}

struct FormLayout {
    fields: Vec<FieldLayout>,
    height: usize,
}

impl FormLayout {
    fn measure(fields: &[FormField], note_width: usize, available: usize) -> Self {
        let mut measured = Vec::with_capacity(fields.len());
        let mut fixed = 0;
        let mut lists = 0;
        for (index, field) in fields.iter().enumerate().filter(|(_, f)| f.visible) {
            let mut wrapped = None;
            let height = match &field.kind {
                FormFieldKind::Input(_) => usize::from(TextInput::HEIGHT),
                FormFieldKind::Select(_) => usize::from(Select::HEIGHT),
                FormFieldKind::List { rows, .. } => usize::from(*rows),
                FormFieldKind::Check(_) => 1,
                FormFieldKind::Radio(r) => r.options.len() + 1,
                FormFieldKind::Chooser { detail, .. } => 2 + usize::from(detail.is_some()),
                FormFieldKind::Note(lines) if !field.wrap => lines.len(),
                FormFieldKind::Note(lines) => {
                    let rows: Vec<_> = lines
                        .iter()
                        .flat_map(|(text, tone)| {
                            termrock_text::cells::wrap(text, note_width)
                                .into_iter()
                                .map(|row| (row, *tone))
                        })
                        .collect();
                    let height = rows.len();
                    wrapped = Some(rows);
                    height
                }
            };
            if matches!(field.kind, FormFieldKind::List { .. }) {
                lists += 1;
            } else {
                fixed += height;
            }
            measured.push(FieldLayout {
                index,
                top: 0,
                height,
                wrapped,
            });
        }
        let list_rows = (available.saturating_sub(fixed + measured.len()) / lists.max(1)).max(1);
        let mut height = 0;
        for field in &mut measured {
            field.top = height;
            if matches!(fields[field.index].kind, FormFieldKind::List { .. }) {
                field.height = field.height.min(list_rows);
            }
            height += field.height + 1;
        }
        Self {
            fields: measured,
            height,
        }
    }
}

impl HasHints for FormDialog {
    fn hints(&self, cx: &HintCx) -> Vec<Hint> {
        if self.is_editing() {
            return termrock_controls::keyhint::EDIT_FIELD.to_vec();
        }
        if let Some(select) = self
            .fields
            .iter()
            .filter(|field| field.visible)
            .find_map(|field| match &field.kind {
                FormFieldKind::Select(select) if cx.focus == Some(select.id) => Some(select),
                _ => None,
            })
        {
            return if select.open {
                vec![
                    Hint::new("↑↓", "Choose"),
                    Hint::new("Enter", "Select"),
                    Hint::new("Esc", "Close options"),
                ]
            } else {
                vec![
                    Hint::new("Tab", "Next field"),
                    Hint::new("Enter", "Open options"),
                    Hint::new("Esc", self.cancel.label.clone()),
                ]
            };
        }
        if let Some(list) = self.focused_list(cx.focus) {
            let mut hints = vec![Hint::new("↑↓", "Browse"), Hint::new("PgUp/PgDn", "Scroll")];
            if self.save.can_activate()
                && list.has_rows(list.items.len())
                && list
                    .items
                    .get(list.cursor)
                    .is_some_and(|row| !row.disabled && !row.control.is_fixed())
            {
                hints.push(self.save.hint());
            }
            hints.extend([
                Hint::new("Tab", "Actions"),
                Hint::new("Esc", self.cancel.label.clone()),
            ]);
            return hints;
        }
        let mut v = vec![Hint::new("Tab", "Next field")];
        if self.scroll.overflows() {
            v.push(Hint::new("PgUp/PgDn", "Scroll"));
        }
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

pub struct FormDialog {
    pub close_button: termrock_controls::icon_button::IconButton,

    pub dismiss: termrock_runtime::popup::Dismiss,
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

    pub button_row: termrock_controls::button::ButtonRow,
}

impl FormDialog {
    pub fn button_row(mut self, row: termrock_controls::button::ButtonRow) -> Self {
        self.button_row = row;
        self
    }

    pub fn close_button(mut self, button: termrock_controls::icon_button::IconButton) -> Self {
        self.close_button = button;
        self
    }

    pub fn dismiss(mut self, dismiss: termrock_runtime::popup::Dismiss) -> Self {
        self.dismiss = dismiss;
        self
    }

    pub fn new(id: WidgetId, title: &str, fields: Vec<FormField>) -> Self {
        Self {
            dismiss: termrock_runtime::popup::Dismiss::ESCAPE,
            close_button: termrock_controls::icon_button::IconButton::close(
                termrock_overlays::modal::CLOSE,
            ),
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
            button_row: termrock_controls::button::ButtonRow::new(),
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

    /// Shows or hides a field. Hiding ends the field's transient interaction
    /// at once; the field's value stays in [`FormValues`] and reveal restores
    /// the value and configuration, never an old edit or popup session.
    ///
    /// Hiding a dirty [`TextInput`] mid-edit is a blur: its
    /// [`BlurAction`](termrock_core::focus::BlurAction) decides the outcome.
    /// `Cancel` reverts to the committed text and reports nothing; `Commit`
    /// and `Keep` retain the typed text and the form reports
    /// [`FormAction::Changed`] once, because the form's values changed under
    /// a consumer that has not been told. This differs from the first
    /// contract of #87 (retain the text with no `Changed`); see
    /// `docs/decisions.md`, 2026-11-18.
    pub fn set_visible(&mut self, name: &str, v: bool) {
        if let Some(f) = self.field_mut(name) {
            f.visible = v;
        }
        self.reconcile_hidden_interactions();
    }

    fn reconcile_hidden_interactions(&mut self) {
        let mut changed = Vec::new();
        for f in self.fields.iter_mut().filter(|f| !f.visible) {
            if f.end_interaction() {
                changed.push(f.name.clone());
            }
        }
        for name in changed {
            self.changed_field(name);
        }
        let hidden = |id: WidgetId| {
            self.fields
                .iter()
                .any(|f| !f.visible && f.focus_id() == Some(id))
        };
        if self.kept.is_some_and(hidden) {
            self.kept = None;
        }
        if let Some(name) = &self.reveal
            && self.field(name).is_some_and(|f| !f.visible)
        {
            self.reveal = None;
        }
    }

    fn resolve_hidden_focus(&self, focus: &mut Focus) {
        let Some(cur) = focus.current() else {
            return;
        };
        let Some(at) = self.fields.iter().position(|f| !f.visible && f.owns(cur)) else {
            return;
        };
        let next = self.fields[at + 1..]
            .iter()
            .filter(|f| f.visible)
            .find_map(FormField::tab_stop)
            .unwrap_or_else(|| self.default_button_focus());
        focus.focus(next);
    }

    fn is_hidden_target(&self, id: WidgetId) -> bool {
        self.fields.iter().any(|f| !f.visible && f.owns(id))
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

    fn focused_list(&self, focus: Option<WidgetId>) -> Option<&ListBox> {
        self.fields
            .iter()
            .filter(|field| field.visible)
            .find_map(|field| match &field.kind {
                FormFieldKind::List { list, .. } if focus == Some(list.id) => Some(list.as_ref()),
                _ => None,
            })
    }

    pub fn initial_focus(&self) -> WidgetId {
        self.fields
            .iter()
            .filter(|f| f.visible)
            .find_map(FormField::tab_stop)
            .unwrap_or_else(|| self.default_button_focus())
    }

    fn default_button_focus(&self) -> WidgetId {
        if self.save.can_activate() {
            return self.save.id;
        }
        self.actions
            .iter()
            .map(|(_, button)| button)
            .find(|button| button.can_activate())
            .map_or(self.cancel.id, |button| button.id)
    }

    pub fn is_editing(&self) -> bool {
        self.fields
            .iter()
            .any(|f| f.visible && matches!(&f.kind, FormFieldKind::Input(i) if i.editing))
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
            .any(|f| f.visible && matches!(&f.kind, FormFieldKind::Select(s) if s.open))
    }

    fn dismiss_escape(&mut self) -> Outcome {
        termrock_overlays::layer::dismiss(self, DismissReason::Esc)
            .map_or(Outcome::Consumed, |response| response.outcome())
    }

    pub fn on_key(&mut self, key: &Key, focus: &mut Focus, ring: &FocusRing) -> Outcome {
        self.reconcile_hidden_interactions();
        self.resolve_hidden_focus(focus);
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
        if !editing && !open && self.focused_list(cur).is_none() && self.scroll.overflows() {
            match key.code {
                KeyCode::PageUp => {
                    self.scroll.page_up();
                    return Outcome::Changed;
                }
                KeyCode::PageDown => {
                    self.scroll.page_down();
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
                FormFieldKind::List { list, .. } if cur == Some(list.id) => {
                    if !list.has_rows(list.items.len()) {
                        return Outcome::Consumed;
                    }
                    if key.is(KeyCode::Enter) {
                        if !self.save.can_activate() {
                            self.save.refuse();
                            return Outcome::Consumed;
                        }
                        let outcome = list.activate(list.cursor);
                        if outcome == Outcome::Changed {
                            self.submit(focus);
                        }
                        return outcome;
                    }
                    let before = list.cursor;
                    let outcome = list.on_key(key);
                    if list.cursor != before {
                        self.dirty = true;
                        self.events.push(FormAction::Changed(name));
                    }
                    if outcome.consumed() {
                        return outcome;
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
                    .chain([&self.cancel, &self.save])
                    .filter(|b| ring.contains(b.id));
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
        self.fields
            .iter_mut()
            .filter(|f| f.visible)
            .filter_map(|f| match &mut f.kind {
                FormFieldKind::Input(i) => Some(i),
                _ => None,
            })
    }

    pub fn on_press(&mut self, id: WidgetId, pos: Position, count: u8) -> Outcome {
        self.reconcile_hidden_interactions();
        if let Some(s) = self.select_owning(id) {
            return s.on_press(id, pos);
        }
        self.inputs()
            .find(|i| i.id == id)
            .map_or(Outcome::Ignored, |i| i.on_press_count(pos, count))
    }

    pub fn on_drag(&mut self, pressed: WidgetId, pos: Position) -> Outcome {
        self.reconcile_hidden_interactions();
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
        self.reconcile_hidden_interactions();
        for f in self.fields.iter_mut().filter(|f| f.visible) {
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
        self.fields
            .iter_mut()
            .filter(|f| f.visible)
            .find_map(|f| match &mut f.kind {
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
        self.reconcile_hidden_interactions();
        self.resolve_hidden_focus(focus);
        if scrollbar::is_bar_of(id, self.id) {
            return self.bar_press(pos).outcome().or(Outcome::Consumed);
        }
        if self.is_hidden_target(id) {
            return Outcome::Consumed;
        }
        for f in self.fields.iter_mut().filter(|f| f.visible) {
            let name = f.name.clone();
            match &mut f.kind {
                FormFieldKind::Input(i) if i.id == id => {
                    focus.focus(id);
                    return i.on_click_count(pos, count.max(1));
                }
                FormFieldKind::List { list, .. } => {
                    if list.area.contains(pos)
                        && let Some(index) = list
                            .scroll
                            .visible_range()
                            .find(|index| list.row_id(*index) == id)
                    {
                        focus.focus(list.id);
                        let before = list.cursor;
                        let outcome = list.on_click_count(index, count);
                        if list.cursor != before {
                            self.dirty = true;
                            self.events.push(FormAction::Changed(name));
                        }
                        return outcome;
                    }
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

    pub fn on_blur(&mut self, ch: termrock_core::focus::FocusChange) -> Outcome {
        self.reconcile_hidden_interactions();
        for f in self.fields.iter_mut().filter(|f| f.visible) {
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

    pub fn on_paste(&mut self, text: &str, focus: &Focus) -> Outcome {
        self.reconcile_hidden_interactions();
        let cur = focus.current();
        for f in self.fields.iter_mut().filter(|f| f.visible) {
            if let FormFieldKind::Input(i) = &mut f.kind
                && i.editing
                && cur == Some(i.id)
            {
                return i.on_paste(text);
            }
        }
        Outcome::Consumed
    }

    pub fn nav(mut self, nav: NavKeys) -> Self {
        self.nav = nav;
        self
    }

    pub fn render(&mut self, screen: Rect, buf: &mut Buffer, ctx: &mut RenderCtx) {
        self.reconcile_hidden_interactions();
        self.scroll.begin(ctx.track_click);
        let s = ctx.theme.strings;
        let old = std::mem::replace(&mut self.strings, s);
        Strings::follow(&mut self.cancel.label, old.cancel, s.cancel);
        Strings::follow(&mut self.save.label, old.save, s.save);
        let frame = termrock_overlays::modal::Frame::new(self.width, 1);
        let inner_width = frame.inner_width(screen);
        let note_width = inner_width.saturating_sub(2);
        let err = u16::from(self.error.is_some());
        let mut widths: Vec<u16> = self.actions.iter().map(|(_, b)| b.width()).collect();
        widths.push(self.cancel.width());
        widths.push(self.save.width());
        let footer_rows = row_lines(inner_width, &widths, 1);
        let maximum_inner = termrock_overlays::modal::Frame::new(self.width, u16::MAX)
            .area(screen)
            .inner(frame.margin)
            .height;
        let reserved = 2 + usize::from(footer_rows) + usize::from(err);
        let layout = FormLayout::measure(
            &self.fields,
            usize::from(note_width),
            usize::from(maximum_inner).saturating_sub(reserved),
        );
        let h = terminal_extent(3 + reserved + layout.height);
        let (area, inner) = termrock_overlays::modal::Frame::new(self.width, h)
            .title(&self.title)
            .meta(self.meta.as_deref())
            .overlay(&*self)
            .render(screen, buf, ctx);
        self.area = area;
        let t = ctx.theme;
        let bg = t.surface_elevated;
        let footer_h = footer_rows.min(inner.height);
        let footer = Rect::new(inner.x, inner.bottom() - footer_h, inner.width, footer_h);
        let body_h = inner.height.saturating_sub(terminal_extent(reserved));
        self.scroll.set_content(layout.height);
        self.scroll.set_viewport(body_h as usize);

        let cur = ctx.interaction.focus;
        let moved = cur != self.kept;
        self.kept = cur;
        if moved
            && let Some(focused) = cur
            && let Some(field) = layout
                .fields
                .iter()
                .find(|f| self.fields[f.index].focus_id() == Some(focused))
        {
            let bottom = field.top + field.height;
            if field.top < self.scroll.offset {
                self.scroll.scroll_to(field.top);
            } else if bottom > self.scroll.offset + usize::from(body_h) {
                self.scroll
                    .scroll_to(bottom.saturating_sub(usize::from(body_h)));
            }
        }
        if let Some(name) = self.reveal.take()
            && let Some(field) = layout
                .fields
                .iter()
                .find(|f| self.fields[f.index].name == name)
        {
            let bottom = field.top + field.height + 1;
            if field.top < self.scroll.offset {
                self.scroll.scroll_to(field.top);
            } else if bottom > self.scroll.offset + usize::from(body_h) {
                self.scroll
                    .scroll_to(bottom.saturating_sub(usize::from(body_h)).min(field.top));
            }
        }
        let body = Rect::new(inner.x, inner.y, inner.width, body_h);
        let viewport = self.scroll.offset..self.scroll.offset + usize::from(body.height);
        let mut open_select = None;
        for field in &layout.fields {
            let f = &mut self.fields[field.index];
            f.clear_geometry();
            if body_h > 0
                && !viewport.contains(&field.top)
                && let Some(id) = f.tab_stop()
            {
                ctx.focus_stop(id);
            }
            let first = field.top.max(viewport.start);
            let end = (field.top + field.height).min(viewport.end);
            if first < end && body.width > 0 {
                let clip_top = first - field.top;
                let r = Rect::new(
                    inner.x.saturating_sub(1),
                    body.y + terminal_extent(first - viewport.start),
                    inner.width.saturating_add(1),
                    terminal_extent(end - first),
                );
                if (clip_top == 0 || matches!(&f.kind, FormFieldKind::Note(_))) && !r.is_empty() {
                    match &mut f.kind {
                        FormFieldKind::Input(i) => i.render(r, buf, ctx, bg),
                        FormFieldKind::List { list, .. } => {
                            list.render(r, buf, ctx, bg);
                        }
                        FormFieldKind::Select(s) => {
                            if s.open {
                                open_select = Some((field.index, r));
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
                            buf.set_stringn(
                                r.x.saturating_add(2),
                                r.y,
                                label,
                                usize::from(r.width.saturating_sub(2)),
                                t.label(focused).bg(bg),
                            );
                            if r.height >= 2 {
                                let bw = button.width().min(r.width.saturating_sub(1));
                                let vw = r.width.saturating_sub(bw.saturating_add(4)) as usize;
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
                            let rows = field.wrapped.as_deref().unwrap_or(lines);
                            for (i, (line, tone)) in rows
                                [clip_top..clip_top + usize::from(r.height)]
                                .iter()
                                .enumerate()
                            {
                                buf.set_string(
                                    r.x.saturating_add(2),
                                    r.y + terminal_extent(i),
                                    truncate(line, usize::from(note_width)),
                                    Style::new().fg(t.tone(*tone)).bg(bg),
                                );
                            }
                        }
                    }
                }
            }
        }
        if self.scroll.overflows() {
            let rows = Rect::new(area.x, body.y, area.width.saturating_sub(1), body.height);
            self.scroll
                .draw_with_fade(rows, buf, ctx, self.id, true, &[]);
        }
        if let Some(e) = &self.error
            && footer.y > inner.y
        {
            let ey = footer.y - 1;
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
        let rects = row_layout_right(footer, &widths, 1);
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
                FormFieldKind::List { .. } => {}
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

        if let Some((index, mut area)) = open_select
            && let FormFieldKind::Select(s) = &mut self.fields[index].kind
        {
            area.height = Select::HEIGHT;
            s.render(area, buf, ctx, bg);
            ctx.hits.register(s.id, s.area);
            for k in 0..s.options.len() {
                ctx.hits.raise(s.option_id(k));
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
        ev: &termrock_core::pointer::PointerEvent,
        focus: &mut Focus,
    ) -> termrock_core::response::Response<()> {
        use termrock_core::intent::Phase;
        use termrock_core::pointer::MouseTarget;
        use termrock_core::response::Response;
        self.reconcile_hidden_interactions();
        self.resolve_hidden_focus(focus);
        for f in self.fields.iter_mut().filter(|f| f.visible) {
            match &mut f.kind {
                FormFieldKind::List { list, .. } => {
                    let before = list.cursor;
                    let Some(response) = list.route(ev) else {
                        continue;
                    };
                    if ev.is(Phase::Click) {
                        focus.focus(list.id);
                    }
                    if list.cursor != before {
                        self.dirty = true;
                        self.events.push(FormAction::Changed(f.name.clone()));
                    }
                    return response;
                }
                FormFieldKind::Select(s) => {
                    let Some(r) = s.route(ev) else {
                        continue;
                    };

                    if ev.wheel().is_some() && !r.is_consumed() {
                        break;
                    }
                    if ev.is(Phase::Click) {
                        focus.focus(s.id);
                    }
                    if r.has_actions() {
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
            && let Some(r) = termrock_core::pointer::scroll_input(self, ev)
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

impl Overlay for FormDialog {
    type Action = FormAction;
    fn dismiss(&self) -> Dismiss {
        self.dismiss
    }
    fn spec(&self) -> LayerSpec {
        LayerSpec::modal(self.dismiss).initial_focus(self.initial_focus())
    }
    fn on_dismiss(&mut self, _: DismissReason) -> Response<FormAction> {
        self.events.push(FormAction::Cancel);
        Response::emit(FormAction::Cancel)
    }
    fn close_button(&self) -> Option<&IconButton> {
        Some(&self.close_button)
    }
}

impl<R: From<LayerResult> + 'static, W: ?Sized + 'static> Layer<R, W> for FormDialog {
    fn hints(&self, focused: Option<WidgetId>) -> Vec<Hint> {
        HasHints::hints(self, &HintCx::new(focused))
    }
    fn spec(&self) -> LayerSpec {
        Overlay::spec(self)
    }
    fn is_editing(&self) -> bool {
        FormDialog::is_editing(self)
    }
    fn render(&mut self, screen: Rect, buf: &mut Buffer, ctx: &mut RenderCtx, _: &W) {
        FormDialog::render(self, screen, buf, ctx);
    }
    fn on_key(&mut self, key: &Key, cx: &mut LayerCx<'_, W>) -> Outcome {
        FormDialog::on_key(self, key, cx.focus, cx.ring).or(Outcome::Consumed)
    }

    fn pointer(&mut self, ev: &PointerEvent, cx: &mut LayerCx<'_, W>) -> Outcome {
        FormDialog::pointer_in(self, ev, cx.focus).outcome()
    }
    fn on_click_count(
        &mut self,
        id: WidgetId,
        pos: Position,
        count: u8,
        cx: &mut LayerCx<'_, W>,
    ) -> Outcome {
        FormDialog::on_click_count(self, id, pos, count, cx.focus)
    }
    fn on_click(&mut self, id: WidgetId, pos: Position, cx: &mut LayerCx<'_, W>) -> Outcome {
        FormDialog::on_click_count(self, id, pos, 1, cx.focus)
    }
    fn on_press(&mut self, id: WidgetId, pos: Position, count: u8) -> Outcome {
        FormDialog::on_press(self, id, pos, count)
    }
    fn on_drag(&mut self, pressed: WidgetId, pos: Position) -> Outcome {
        FormDialog::on_drag(self, pressed, pos)
    }
    fn on_release(&mut self, pressed: WidgetId, pos: Position) -> Outcome {
        FormDialog::on_release(self, pressed, pos)
    }
    fn on_wheel(&mut self, delta: i32, _: Position) -> Outcome {
        Scrollable::wheel(self, Axis::Vertical, delta).outcome()
    }
    fn on_paste(&mut self, _: &str) -> Outcome {
        Outcome::Consumed
    }
    fn on_paste_focused(&mut self, text: &str, focus: &Focus) -> Outcome {
        FormDialog::on_paste(self, text, focus)
    }
    fn on_blur(&mut self, change: FocusChange) -> Outcome {
        FormDialog::on_blur(self, change)
    }
    fn on_dismiss(&mut self, reason: DismissReason) {
        let _ = Overlay::on_dismiss(self, reason);
    }
    fn take_result(&mut self) -> Option<LayerOut<R>> {
        if self.events.is_empty() {
            return None;
        }
        let ev = self.events.remove(0);
        let closes = matches!(ev, FormAction::Save | FormAction::Cancel);
        Some(layer_out(LayerResult::Form(ev), closes))
    }
}

#[cfg(test)]
mod tests;

#[cfg(test)]
mod issue35_tests {
    use std::cell::Cell;
    use std::rc::Rc;

    use ratatui::crossterm::event::{KeyCode, KeyModifiers};

    use super::*;
    use termrock_core::event::Key;
    use termrock_core::focus::{Focus, FocusRing};
    use termrock_core::id::WidgetId;
    use termrock_fields::input::TextInput;

    #[test]
    fn issue35_form_save_blocked_by_required_before_custom() {
        let id = WidgetId::of("issue35.form");
        let calls = Rc::new(Cell::new(0));
        let counter = calls.clone();
        let name = TextInput::new(id.sub("name"), "Name")
            .required(true)
            .required_message("Name is required")
            .validator(move |_| {
                counter.set(counter.get() + 1);
                None
            });
        let mut form = FormDialog::new(id, "Account", vec![FormField::input("name", name)]);
        let mut focus = Focus::default();
        focus.set(Some(form.save.id));
        let enter = Key {
            code: KeyCode::Enter,
            mods: KeyModifiers::NONE,
        };

        form.on_key(&enter, &mut focus, &FocusRing::default());

        assert!(!form.events.iter().any(|e| matches!(e, FormAction::Save)));
        assert_eq!(focus.current(), Some(id.sub("name")));
        assert_eq!(calls.get(), 0);
    }
}
