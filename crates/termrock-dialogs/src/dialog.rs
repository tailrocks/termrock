use ratatui::buffer::Buffer;
use ratatui::crossterm::event::KeyCode;
use ratatui::layout::{Constraint, Margin, Position, Rect};

use ratatui::style::Color;
use termrock_controls::button::{Button, ButtonRow, RowAlign, RowOverflow};
use termrock_controls::icon_button::IconButton;
use termrock_controls::keyhint::{Hint, hint};
use termrock_controls::scrollbar;
use termrock_core::event::{Key, Outcome};
use termrock_core::focus::FocusChange;
use termrock_core::hints::{HasHints, HintCx};
use termrock_core::hit::Axis;
use termrock_core::id::WidgetId;
use termrock_core::keymap::{Actions, KeyBinding, KeyMap, Nav, NavKeys, lookup};
use termrock_core::pointer::PointerEvent;
use termrock_core::response::Response;
use termrock_core::scroll::{HScroll, ScrollPane, ScrollState, Scrollable};
use termrock_fields::input::{InputAction, TextInput};
use termrock_overlays::layer::{
    DismissReason, Layer, LayerCx, LayerOut, LayerResult, LayerSpec, Overlay, layer_out,
};
pub use termrock_overlays::modal::{OUTER_MARGIN, body_bounds};
use termrock_runtime::ctx::{Backdrop, RenderCtx, fill};
use termrock_runtime::popup::{Dismiss, Place, Placement, place_with};
use termrock_runtime::specimen::{Specimen, SpecimenState};
use termrock_theme::{ButtonKind, StatusKind, Strings};
use termrock_viewport::panel::Panel;

#[derive(Debug, Clone)]
pub enum DialogBody {
    Text(String),
    Input(TextInput),

    Facts {
        facts: Vec<termrock_collections::props::Prop>,
        code: Vec<String>,
        ack: Option<AckInput>,
    },
}

#[derive(Debug, Clone)]
pub struct AckInput {
    pub input: TextInput,
    pub token: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DialogResult {
    Action(usize),
    Cancelled,
}

#[derive(Debug, Clone)]
pub struct Dialog {
    pub id: WidgetId,
    pub title: String,

    pub title_status: Option<StatusKind>,
    pub body: DialogBody,
    pub actions: Vec<Button>,

    pub cancel_index: Option<usize>,
    pub width: u16,

    pub keymap: KeyMap<DialogCmd>,

    pub nav: NavKeys,
    pub area: Rect,
    pub result: Option<DialogResult>,
    pub initial_focus: WidgetId,

    pub dismiss: Dismiss,

    pub close_button: termrock_controls::icon_button::IconButton,

    pub confirm_index: Option<usize>,

    pub body_scroll: bool,

    pub max_height: Option<u16>,

    pub outer_margin: Margin,

    pub placement: Placement,

    pub anchor: Rect,

    pub backdrop: Backdrop,

    pub chrome: (u16, u16),

    pub button_row: ButtonRow,

    gap: u16,

    pad: termrock_theme::Insets,

    pub scroll: ScrollPane,

    pub horizontal: Option<HScroll>,

    strings: Strings,

    pub ack_enter: AckEnter,

    pub mnemonics: bool,

    pristine: String,

    pub guard_dirty_prompt: bool,

    pub guard_destructive_dismiss: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum AckEnter {
    #[default]
    Submit,

    NextField,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DialogCmd {
    Confirm,

    Cancel,
}

impl Actions for DialogCmd {
    fn label(self) -> &'static str {
        match self {
            DialogCmd::Confirm => "Confirm",
            DialogCmd::Cancel => "Cancel",
        }
    }

    fn default_keymap() -> KeyMap<Self> {
        vec![]
    }
}

impl HasHints for Dialog {
    fn hints(&self, cx: &HintCx) -> Vec<Hint> {
        Dialog::hints(self, cx.focus)
    }
}

impl Dialog {
    pub fn width(mut self, width: u16) -> Self {
        self.width = width;
        self
    }

    pub fn accelerators(mut self, keys: Option<(char, char)>) -> Self {
        self.keymap = match keys {
            Some((yes, no)) => vec![
                (KeyBinding::char(yes), DialogCmd::Confirm),
                (KeyBinding::char(no), DialogCmd::Cancel),
            ],
            None => vec![],
        };
        self
    }

    pub fn keymap(mut self, keymap: KeyMap<DialogCmd>) -> Self {
        self.keymap = keymap;
        self
    }

    pub fn nav(mut self, nav: NavKeys) -> Self {
        self.nav = nav;
        self
    }

    pub fn ack_enter(mut self, ack_enter: AckEnter) -> Self {
        self.ack_enter = ack_enter;
        self
    }

    pub fn mnemonics(mut self, on: bool) -> Self {
        self.mnemonics = on;
        self
    }

    pub fn guard_dirty_prompt(mut self, on: bool) -> Self {
        self.guard_dirty_prompt = on;
        self
    }

    pub fn guard_destructive_dismiss(mut self, on: bool) -> Self {
        self.guard_destructive_dismiss = on;
        self
    }

    fn dismissal_is_destructive(&self) -> bool {
        self.cancel_index
            .and_then(|i| self.actions.get(i))
            .is_some_and(|b| b.kind == ButtonKind::Danger)
    }

    pub(crate) fn dismiss_in_force(&self) -> Dismiss {
        let mut d = self.dismiss;
        if self.guard_dirty_prompt && self.prompt_dirty() {
            d.outside_click = false;
        }
        if self.guard_destructive_dismiss && self.dismissal_is_destructive() {
            d.outside_click = false;
            d.focus_out = false;
        }
        d
    }

    fn has_text_field(&self) -> bool {
        matches!(
            self.body,
            DialogBody::Input(_) | DialogBody::Facts { ack: Some(_), .. }
        )
    }

    pub fn mnemonic_keys(&self) -> Vec<Option<(char, usize)>> {
        let mut taken: Vec<char> = vec![];
        self.actions
            .iter()
            .map(|b| {
                let m = b
                    .label
                    .chars()
                    .enumerate()
                    .filter(|(_, c)| c.is_alphanumeric())
                    .map(|(i, c)| (c.to_lowercase().next().unwrap_or(c), i))
                    .find(|(c, _)| !taken.contains(c));
                if let Some((c, _)) = m {
                    taken.push(c);
                }
                m
            })
            .collect()
    }

    fn mnemonics_live(&self) -> bool {
        self.mnemonics && !self.has_text_field()
    }

    fn mnemonic_action(&self, key: &Key) -> Option<usize> {
        let KeyCode::Char(c) = key.code else {
            return None;
        };
        if !self.mnemonics_live() || key.chord() {
            return None;
        }
        let c = c.to_lowercase().next().unwrap_or(c);
        self.mnemonic_keys()
            .iter()
            .position(|m| m.is_some_and(|(k, _)| k == c))
    }

    pub fn prompt_dirty(&self) -> bool {
        matches!(&self.body, DialogBody::Input(i) if i.text() != self.pristine)
    }

    fn gated_action(&self) -> Option<usize> {
        self.confirm_index.or_else(|| {
            self.actions
                .iter()
                .position(|b| matches!(b.kind, ButtonKind::Primary | ButtonKind::Danger))
        })
    }

    pub fn confirm(id: WidgetId, title: &str, text: &str, confirm: &str) -> Self {
        let cancel = Button::subtle(id.sub("cancel"), Strings::DEFAULT.cancel);
        let ok = Button::primary(id.sub("ok"), confirm);
        let ok_id = ok.id;
        Self {
            id,
            title: title.to_owned(),
            body: DialogBody::Text(text.to_owned()),
            actions: vec![cancel, ok],
            cancel_index: Some(0),
            width: 54,
            keymap: DialogCmd::default_keymap(),
            nav: NavKeys::default(),
            area: Rect::ZERO,
            result: None,
            initial_focus: ok_id,
            dismiss: Dismiss::LIGHT,
            close_button: termrock_controls::icon_button::IconButton::close(
                termrock_overlays::modal::CLOSE,
            ),
            title_status: None,
            confirm_index: None,
            body_scroll: true,
            max_height: None,
            outer_margin: OUTER_MARGIN,
            placement: Placement::Center,
            anchor: Rect::ZERO,
            backdrop: Backdrop::DEFAULT,
            chrome: (0, 0),
            button_row: ButtonRow::new()
                .align(RowAlign::End)
                .overflow(RowOverflow::Wrap),
            gap: termrock_theme::Spacing::DEFAULT.button_gap,
            pad: termrock_theme::Spacing::DEFAULT.dialog,
            scroll: ScrollPane::default(),
            horizontal: None,
            strings: Strings::DEFAULT,
            ack_enter: AckEnter::Submit,
            mnemonics: true,
            pristine: String::new(),
            guard_dirty_prompt: true,
            guard_destructive_dismiss: true,
        }
    }

    pub fn destructive(id: WidgetId, title: &str, text: &str, confirm: &str) -> Self {
        let cancel = Button::secondary(id.sub("cancel"), Strings::DEFAULT.cancel);
        let cancel_id = cancel.id;
        let del = Button::danger(id.sub("ok"), confirm);
        Self {
            id,
            title: title.to_owned(),
            body: DialogBody::Text(text.to_owned()),
            actions: vec![cancel, del],
            cancel_index: Some(0),
            width: 54,
            keymap: DialogCmd::default_keymap(),
            nav: NavKeys::default(),
            area: Rect::ZERO,
            result: None,
            initial_focus: cancel_id,
            dismiss: Dismiss::LIGHT,
            close_button: termrock_controls::icon_button::IconButton::close(
                termrock_overlays::modal::CLOSE,
            ),
            title_status: None,
            confirm_index: None,
            body_scroll: true,
            max_height: None,
            outer_margin: OUTER_MARGIN,
            placement: Placement::Center,
            anchor: Rect::ZERO,
            backdrop: Backdrop::DEFAULT,
            chrome: (0, 0),
            button_row: ButtonRow::new()
                .align(RowAlign::End)
                .overflow(RowOverflow::Wrap),
            gap: termrock_theme::Spacing::DEFAULT.button_gap,
            pad: termrock_theme::Spacing::DEFAULT.dialog,
            scroll: ScrollPane::default(),
            horizontal: None,
            strings: Strings::DEFAULT,
            ack_enter: AckEnter::Submit,
            mnemonics: true,
            pristine: String::new(),
            guard_dirty_prompt: true,
            guard_destructive_dismiss: true,
        }
    }

    pub fn prompt(id: WidgetId, title: &str, input: TextInput, confirm: &str) -> Self {
        let cancel = Button::subtle(id.sub("cancel"), Strings::DEFAULT.cancel);
        let ok = Button::primary(id.sub("ok"), confirm);
        let input_id = input.id;
        let pristine = input.text().to_owned();
        Self {
            id,
            title: title.to_owned(),
            body: DialogBody::Input(input),
            actions: vec![cancel, ok],
            cancel_index: Some(0),
            width: 54,
            keymap: DialogCmd::default_keymap(),
            nav: NavKeys::default(),
            area: Rect::ZERO,
            result: None,
            initial_focus: input_id,
            dismiss: Dismiss::LIGHT,
            close_button: termrock_controls::icon_button::IconButton::close(
                termrock_overlays::modal::CLOSE,
            ),
            title_status: None,
            confirm_index: None,
            body_scroll: true,
            max_height: None,
            outer_margin: OUTER_MARGIN,
            placement: Placement::Center,
            anchor: Rect::ZERO,
            backdrop: Backdrop::DEFAULT,
            chrome: (0, 0),
            button_row: ButtonRow::new()
                .align(RowAlign::End)
                .overflow(RowOverflow::Wrap),
            gap: termrock_theme::Spacing::DEFAULT.button_gap,
            pad: termrock_theme::Spacing::DEFAULT.dialog,
            scroll: ScrollPane::default(),
            horizontal: None,
            strings: Strings::DEFAULT,
            ack_enter: AckEnter::Submit,
            mnemonics: true,
            pristine,
            guard_dirty_prompt: true,
            guard_destructive_dismiss: true,
        }
    }

    fn follow_strings(&mut self, s: Strings) {
        let old = std::mem::replace(&mut self.strings, s);
        if old == s {
            return;
        }
        if let Some(b) = self.cancel_index.and_then(|i| self.actions.get_mut(i)) {
            Strings::follow(&mut b.label, old.cancel, s.cancel);
        }
        if let DialogBody::Facts { ack: Some(a), .. } = &mut self.body {
            let tok: &dyn std::fmt::Display = &a.token;
            Strings::follow(
                &mut a.input.label,
                &Strings::fill(old.type_to_confirm, &[tok]),
                &Strings::fill(s.type_to_confirm, &[tok]),
            );
        }
    }

    pub fn facts(
        id: WidgetId,
        title: &str,
        facts: Vec<termrock_collections::props::Prop>,
        code: Vec<String>,
        token: Option<&str>,
        confirm: Button,
    ) -> Self {
        let cancel = Button::secondary(id.sub("cancel"), Strings::DEFAULT.cancel);
        let cancel_id = cancel.id;
        let ack = token.map(|tok| AckInput {
            input: TextInput::new(
                id.sub("ack"),
                &Strings::fill(Strings::DEFAULT.type_to_confirm, &[&tok]),
            )
            .plain_label()
            .wrap_label(true),
            token: tok.to_owned(),
        });
        let initial = ack.as_ref().map(|a| a.input.id).unwrap_or(cancel_id);
        Self {
            id,
            title: title.to_owned(),
            body: DialogBody::Facts { facts, code, ack },
            actions: vec![cancel, confirm],
            cancel_index: Some(0),
            width: 66,
            keymap: DialogCmd::default_keymap(),
            nav: NavKeys::default(),
            area: Rect::ZERO,
            result: None,
            initial_focus: initial,
            dismiss: if token.is_some() {
                Dismiss::ESCAPE
            } else {
                Dismiss::LIGHT
            },
            close_button: termrock_controls::icon_button::IconButton::close(
                termrock_overlays::modal::CLOSE,
            ),
            title_status: None,
            confirm_index: token.map(|_| 1),
            body_scroll: true,
            max_height: None,
            outer_margin: OUTER_MARGIN,
            placement: Placement::Center,
            anchor: Rect::ZERO,
            backdrop: Backdrop::DEFAULT,
            chrome: (0, 0),
            button_row: ButtonRow::new()
                .align(RowAlign::End)
                .overflow(RowOverflow::Wrap),
            gap: termrock_theme::Spacing::DEFAULT.button_gap,
            pad: termrock_theme::Spacing::DEFAULT.dialog,
            scroll: ScrollPane::default(),
            horizontal: None,
            strings: Strings::DEFAULT,
            ack_enter: AckEnter::Submit,
            mnemonics: true,
            pristine: String::new(),
            guard_dirty_prompt: true,
            guard_destructive_dismiss: true,
        }
    }

    pub fn armed(&self) -> bool {
        match &self.body {
            DialogBody::Facts { ack: Some(a), .. } => a.input.text().trim() == a.token,
            _ => true,
        }
    }

    pub fn hints(&self, focused: Option<WidgetId>) -> Vec<Hint> {
        let s = &self.strings;
        let cancel_label = self
            .cancel_index
            .and_then(|i| self.actions.get(i))
            .map(|b| b.label.clone());
        let dismiss_hint = || {
            self.dismiss.escape.then(|| {
                Hint::new(
                    "Esc",
                    cancel_label.clone().unwrap_or_else(|| s.hint_close.into()),
                )
            })
        };
        let mut v = Vec::new();
        let input = match &self.body {
            DialogBody::Input(i) => Some(i),
            DialogBody::Facts { ack: Some(a), .. } => Some(&a.input),
            _ => None,
        };
        if let Some(inp) = input
            && focused == Some(inp.id)
        {
            if let DialogBody::Facts { ack: Some(a), .. } = &self.body {
                let gated = self
                    .gated_action()
                    .and_then(|i| self.actions.get(i))
                    .map(|b| b.label.clone());
                match (self.ack_enter, self.armed(), gated) {
                    (AckEnter::Submit, true, Some(label)) => v.push(Hint::new("Enter", label)),
                    (AckEnter::Submit, _, _) => v.push(Hint::new(
                        "Type",
                        Strings::fill(s.type_to_confirm_hint, &[&a.token]),
                    )),
                    (AckEnter::NextField, ..) => {
                        v.push(Hint::new(
                            "Type",
                            Strings::fill(s.type_to_confirm_hint, &[&a.token]),
                        ));
                        v.push(hint("Enter", s.hint_next));
                    }
                }
            } else if inp.editing {
                let primary = self
                    .actions
                    .iter()
                    .find(|b| b.kind == ButtonKind::Primary)
                    .map(|b| b.label.clone());
                v.push(Hint::new(
                    "Enter",
                    primary.unwrap_or_else(|| s.hint_apply.into()),
                ));
            } else {
                v.push(hint("Enter", s.hint_edit));
                v.push(hint("Tab", s.hint_next));
            }
            if inp.editing {
                v.push(hint("Esc", s.hint_cancel));
            } else {
                v.extend(dismiss_hint());
            }
            return v;
        }

        let usable = |i: usize, b: &Button| {
            b.can_activate() && (Some(i) != self.confirm_index || self.armed())
        };
        let enabled = self
            .actions
            .iter()
            .enumerate()
            .filter(|(i, b)| usable(*i, b))
            .count();
        if enabled > 1 {
            v.push(hint("←→", s.hint_choose));
        }
        if let Some((_, b)) = self
            .actions
            .iter()
            .enumerate()
            .find(|(i, b)| Some(b.id) == focused && usable(*i, b))
        {
            v.push(Hint::new("Enter", b.label.clone()));
        }
        if self.scroll.overflows() {
            v.push(hint("↑↓", s.hint_scroll));
        }
        if self.scroll.h_cfg.enabled && self.scroll.h.overflows() {
            v.push(hint("⇧←→", s.hint_scroll_code));
        }
        v.extend(dismiss_hint());

        if matches!(self.body, DialogBody::Text(_)) {
            if let Some(k) = termrock_core::keymap::hint_key(&self.keymap, DialogCmd::Confirm)
                && let Some(p) = self.actions.iter().find(|b| {
                    matches!(b.kind, ButtonKind::Primary | ButtonKind::Danger) && b.can_activate()
                })
            {
                v.push(Hint::new(k, p.label.clone()));
            }
            if let Some(k) = termrock_core::keymap::hint_key(&self.keymap, DialogCmd::Cancel)
                && let Some(c) = cancel_label
            {
                v.push(Hint::new(k, c));
            }
        }
        v
    }

    pub fn cancel_label(mut self, label: &str) -> Self {
        if let Some(b) = self.cancel_index.and_then(|i| self.actions.get_mut(i)) {
            b.label = label.to_owned();
        }
        self
    }

    pub fn title_status(mut self, kind: StatusKind) -> Self {
        self.title_status = Some(kind);
        self
    }

    pub fn dismiss(mut self, dismiss: Dismiss) -> Self {
        self.dismiss = dismiss;
        self
    }

    pub fn close_button(mut self, button: termrock_controls::icon_button::IconButton) -> Self {
        self.close_button = button;
        self
    }

    pub fn edit_entry(mut self, entry: termrock_core::edit::EditEntry) -> Self {
        if let Some(inp) = self.input_mut() {
            inp.set_edit_entry(entry);
        }
        self
    }

    pub(crate) fn dismissed(&mut self) -> Outcome {
        match self.cancel_index {
            Some(ci) => self.finish(DialogResult::Action(ci)),
            None => self.finish(DialogResult::Cancelled),
        }
    }

    pub fn actions(mut self, actions: Vec<Button>, cancel_index: Option<usize>) -> Self {
        self.actions = actions;
        self.cancel_index = cancel_index;
        self
    }

    #[deprecated(note = "renamed to `Dialog::actions`")]
    pub fn with_actions(mut self, actions: Vec<Button>, cancel_index: Option<usize>) -> Self {
        self.actions = actions;
        self.cancel_index = cancel_index;
        self
    }

    pub fn is_editing(&self) -> bool {
        match &self.body {
            DialogBody::Input(i) => i.editing,
            DialogBody::Facts { ack: Some(a), .. } => a.input.editing,
            _ => false,
        }
    }

    fn input_mut(&mut self) -> Option<&mut TextInput> {
        match &mut self.body {
            DialogBody::Input(i) => Some(i),
            DialogBody::Facts { ack: Some(a), .. } => Some(&mut a.input),
            _ => None,
        }
    }

    fn code_rows(code: &[String]) -> u16 {
        if code.is_empty() {
            0
        } else {
            code.len() as u16 + 1
        }
    }

    fn code_bar_rows(code: &[String], inner_w: u16, h_cfg: &HScroll) -> u16 {
        u16::from(!code.is_empty() && h_cfg.shows_bar(&Self::code_extent(code, inner_w)))
    }

    fn code_extent(code: &[String], inner_w: u16) -> ScrollState {
        let widest = code
            .iter()
            .map(|l| termrock_text::cells::width(l))
            .max()
            .unwrap_or(0);
        let mut s = ScrollState::new(widest);
        s.set_viewport(inner_w as usize);
        s
    }

    fn scroll_code(&mut self, delta: isize) -> Outcome {
        self.scroll.wheel(Axis::Horizontal, delta as i32).outcome()
    }

    pub fn on_drag(&mut self, pressed: WidgetId, pos: Position) -> Outcome {
        if let Some(inp) = self.input_mut()
            && inp.id == pressed
        {
            return inp.on_drag(pos);
        }
        if scrollbar::is_bar_of(pressed, self.id) {
            return match self.pane().bar_of(pressed) {
                Some(bar) => self.bar_input(&bar, pos, false).outcome(),
                None => Outcome::Consumed,
            };
        }
        Outcome::Consumed
    }

    fn ack_rows(&self, ack: Option<&AckInput>, width: u16) -> u16 {
        ack.map_or(0, |a| a.input.height_for(self.field_width(width)) + 1)
    }

    fn field_width(&self, width: u16) -> u16 {
        self.inner_width(width) + TextInput::GUTTER
    }

    fn inner_width(&self, width: u16) -> u16 {
        width.saturating_sub(2 * (1 + self.pad.x))
    }

    fn scroll_rows(&self, width: u16) -> u16 {
        let inner_w = self.inner_width(width);
        match &self.body {
            DialogBody::Text(t) => termrock_text::cells::wrap(t, inner_w as usize).len() as u16,
            DialogBody::Input(_) => 0,
            DialogBody::Facts { facts, code, .. } => {
                termrock_collections::props::measure(facts, inner_w) + Self::code_rows(code)
            }
        }
    }

    fn fixed_rows(&self, width: u16) -> u16 {
        match &self.body {
            DialogBody::Text(_) => 0,
            DialogBody::Input(i) => i.height_for(self.field_width(width)),
            DialogBody::Facts { code, ack, .. } => {
                Self::code_bar_rows(code, self.inner_width(width), &self.scroll.h_cfg)
                    + self.ack_rows(ack.as_ref(), width)
            }
        }
    }

    fn chrome_rows(&self) -> u16 {
        5 + 2 * self.pad.y
    }

    fn action_rows(&self, width: u16) -> u16 {
        self.button_row
            .rows_at_gap(self.inner_width(width), &self.actions, self.gap)
            .max(1)
    }

    pub fn height(&self, width: u16) -> u16 {
        self.chrome_rows()
            + self.action_rows(width)
            + self.scroll_rows(width)
            + self.fixed_rows(width)
    }

    pub fn button_row(mut self, row: ButtonRow) -> Self {
        self.button_row = row;
        self
    }

    pub fn body_scroll(mut self, on: bool) -> Self {
        self.body_scroll = on;
        self
    }

    pub fn max_height(mut self, rows: Option<u16>) -> Self {
        self.max_height = rows;
        self
    }

    pub fn outer_margin(mut self, margin: Margin) -> Self {
        self.outer_margin = margin;
        self
    }

    pub fn backdrop(mut self, backdrop: Backdrop) -> Self {
        self.backdrop = backdrop;
        self
    }

    pub fn chrome(mut self, top: u16, bottom: u16) -> Self {
        self.chrome = (top, bottom);
        self
    }

    pub fn placement(mut self, placement: Placement) -> Self {
        self.placement = placement;
        self
    }

    pub fn anchor(mut self, anchor: Rect, placement: Placement) -> Self {
        self.anchor = anchor;
        self.placement = placement;
        self
    }

    pub fn confirm_index(mut self, index: Option<usize>) -> Self {
        self.confirm_index = index;
        self
    }

    fn activate_action(&mut self, i: usize) -> Outcome {
        let (out, activated) = self.actions[i].activate();
        if activated {
            self.finish(DialogResult::Action(i))
        } else {
            out
        }
    }

    fn finish(&mut self, r: DialogResult) -> Outcome {
        if let DialogResult::Action(i) = r
            && self.actions.get(i).is_some_and(Button::refuse)
        {
            return Outcome::Consumed;
        }
        if let DialogResult::Action(i) = r
            && Some(i) == self.confirm_index
            && !self.armed()
        {
            return Outcome::Consumed;
        }
        if let DialogResult::Action(i) = r
            && let DialogBody::Input(inp) = &mut self.body
            && Some(i) != self.cancel_index
        {
            if inp.editing {
                inp.commit();
            }
            if !inp.validate() {
                inp.resume();
                return Outcome::Changed;
            }
        }
        self.result = Some(r);
        Outcome::Changed
    }

    pub fn on_key(
        &mut self,
        key: &Key,
        focus: &mut termrock_core::focus::Focus,
        ring: &termrock_core::focus::FocusRing,
    ) -> Outcome {
        let cur = focus.current();

        if key.is(KeyCode::Esc)
            && self.dismiss.escape
            && let Some(inp) = self.input_mut()
            && cur == Some(inp.id)
            && inp.entry() == termrock_core::edit::EditEntry::Immediate
        {
            return self.dismissed();
        }

        let is_facts = matches!(self.body, DialogBody::Facts { .. });
        if let Some(inp) = self.input_mut()
            && cur == Some(inp.id)
        {
            let (o, ev) = inp.on_key(key);
            match ev {
                Some(InputAction::CommittedTab { backward }) => {
                    if backward {
                        focus.prev(ring);
                    } else {
                        focus.next(ring);
                    }
                    return Outcome::Changed;
                }
                Some(InputAction::Committed) => {
                    if is_facts {
                        if self.ack_enter == AckEnter::NextField {
                            focus.next(ring);
                            return Outcome::Changed;
                        }
                        if self.armed()
                            && let Some(i) = self.gated_action()
                        {
                            return self.finish(DialogResult::Action(i));
                        }
                        let mismatch = self.strings.ack_mismatch;
                        if let Some(inp) = self.input_mut() {
                            inp.begin_edit();
                            inp.field.error = Some(mismatch.into());
                        }
                        return Outcome::Changed;
                    }
                    let primary = self
                        .actions
                        .iter()
                        .position(|b| b.kind == ButtonKind::Primary);
                    if let Some(p) = primary {
                        return self.finish(DialogResult::Action(p));
                    }
                    return o;
                }
                _ => {}
            }
            if o.consumed() {
                return o;
            }
        }
        for (i, b) in self.actions.iter_mut().enumerate() {
            if cur == Some(b.id) {
                let (o, activated) = b.on_key(key);
                if activated {
                    return self.finish(DialogResult::Action(i));
                }
                if o.consumed() {
                    return o;
                }
            }
        }
        if matches!(self.body, DialogBody::Text(_)) {
            match lookup(&self.keymap, key) {
                Some(DialogCmd::Confirm) => {
                    let primary = self.actions.iter().position(|b| {
                        matches!(b.kind, ButtonKind::Primary | ButtonKind::Danger) && !b.disabled
                    });
                    return match primary {
                        Some(p) => self.activate_action(p),
                        None => Outcome::Consumed,
                    };
                }
                Some(DialogCmd::Cancel) => {
                    return match self.cancel_index {
                        Some(ci) => self.finish(DialogResult::Action(ci)),
                        None => Outcome::Consumed,
                    };
                }
                None => {}
            }
        }

        if let Some(i) = self.mnemonic_action(key) {
            return if Some(i) != self.confirm_index || self.armed() {
                self.activate_action(i)
            } else {
                Outcome::Consumed
            };
        }
        let overflows = self.scroll.overflows();
        let arrow = !matches!(key.code, KeyCode::Char(_));
        match (self.nav.action(key), key.code) {
            (_, KeyCode::Esc) if self.dismiss.escape => self.dismissed(),
            (_, KeyCode::Tab) => {
                focus.next(ring);
                Outcome::Changed
            }
            (_, KeyCode::BackTab) => {
                focus.prev(ring);
                Outcome::Changed
            }

            (Some(Nav::Up), _) if overflows => self.wheel(Axis::Vertical, -1).outcome(),
            (Some(Nav::Down), _) if overflows => self.wheel(Axis::Vertical, 1).outcome(),
            (Some(Nav::PageUp), _) if overflows => {
                self.scroll.page_up();
                Outcome::Changed
            }
            (Some(Nav::PageDown), _) if overflows => {
                self.scroll.page_down();
                Outcome::Changed
            }
            (Some(Nav::Home), _) if overflows => {
                self.scroll.jump_start();
                Outcome::Changed
            }
            (Some(Nav::End), _) if overflows => {
                self.scroll.jump_end();
                Outcome::Changed
            }

            (Some(Nav::Left), _) if arrow && key.shift() && self.scroll.h.overflows() => {
                self.scroll_code(-1)
            }
            (Some(Nav::Right), _) if arrow && key.shift() && self.scroll.h.overflows() => {
                self.scroll_code(1)
            }
            (Some(nav @ (Nav::Left | Nav::Right)), _) => {
                if let Some(id) = self.button_row.step(
                    self.actions.iter().filter(|b| ring.contains(b.id)),
                    cur,
                    nav == Nav::Right,
                ) {
                    focus.focus(id);
                }
                Outcome::Changed
            }
            _ => Outcome::Consumed,
        }
    }

    pub fn on_blur(&mut self, change: termrock_core::focus::FocusChange) -> Outcome {
        match self.input_mut() {
            Some(inp) if inp.id == change.from => match inp.on_blur() {
                Some(_) => Outcome::Changed,
                None => Outcome::Ignored,
            },
            _ => Outcome::Ignored,
        }
    }

    pub fn on_paste(&mut self, text: &str) -> Outcome {
        match self.input_mut() {
            Some(inp) => inp.on_paste(text).or(Outcome::Consumed),
            None => Outcome::Consumed,
        }
    }

    pub fn on_press(&mut self, id: WidgetId, pos: Position, count: u8) -> Outcome {
        match self.input_mut() {
            Some(inp) if inp.id == id => inp.on_press_count(pos, count),
            _ => Outcome::Ignored,
        }
    }

    pub fn on_click(
        &mut self,
        id: WidgetId,
        pos: Position,
        focus: &mut termrock_core::focus::Focus,
    ) -> Outcome {
        self.on_click_count(id, pos, 1, focus)
    }

    pub fn on_click_count(
        &mut self,
        id: WidgetId,
        pos: Position,
        count: u8,
        focus: &mut termrock_core::focus::Focus,
    ) -> Outcome {
        if id == termrock_overlays::modal::CLOSE && self.dismiss.shows_close() {
            return self.dismissed();
        }
        if let Some(inp) = self.input_mut()
            && inp.id == id
        {
            focus.focus(id);
            return inp.on_click_count(pos, count.max(1));
        }
        if scrollbar::is_bar_of(id, self.id) {
            let o = self.bar_press(pos).outcome().or(Outcome::Consumed);
            if id == scrollbar::id_for(self.id) {
                self.scroll.v.release_track();
            }
            return o;
        }
        for i in 0..self.actions.len() {
            if self.actions[i].id == id {
                focus.focus(id);
                if self.actions[i].on_click() {
                    return self.finish(DialogResult::Action(i));
                }
                return Outcome::Changed;
            }
        }
        Outcome::Consumed
    }

    pub fn on_click_outside(&mut self) -> Outcome {
        if self.dismiss_in_force().outside_click {
            self.dismissed()
        } else {
            Outcome::Consumed
        }
    }

    pub fn render(&mut self, screen: Rect, buf: &mut Buffer, ctx: &mut RenderCtx) {
        let bounds = body_bounds(screen, self.chrome.0, self.chrome.1);
        self.render_in(screen, bounds, buf, ctx);
    }

    pub fn render_in(&mut self, screen: Rect, bounds: Rect, buf: &mut Buffer, ctx: &mut RenderCtx) {
        self.follow_strings(ctx.theme.strings);
        let t = ctx.theme;
        self.scroll.h_cfg = self.horizontal.unwrap_or(ctx.hscroll);
        self.scroll.begin(ctx.track_click);
        self.gap = t.spacing.button_gap;

        self.pad = termrock_theme::Insets::new(t.spacing.dialog.x.max(1), t.spacing.dialog.y);

        ctx.modal_backdrop_with(buf, screen, self.backdrop);
        let bounds = bounds.intersection(screen);
        let bounds = if bounds.is_empty() { screen } else { bounds };
        let m = self.outer_margin;

        let width = self
            .width
            .min(bounds.width.saturating_sub(m.horizontal * 2))
            .max(20)
            .min(bounds.width);
        let room = bounds
            .height
            .saturating_sub(m.vertical * 2)
            .min(self.max_height.unwrap_or(u16::MAX));
        let height = self.height(width).min(room);
        let area = match self.placement {
            Placement::Center => {
                bounds.centered(Constraint::Length(width), Constraint::Length(height))
            }
            side => place_with(
                bounds,
                self.anchor,
                width,
                height,
                side,
                Place::DEFAULT
                    .margin(m.horizontal)
                    .vertical_margin(Some(m.vertical)),
            ),
        };
        self.area = area;
        let bg = t.surface_elevated;
        fill(buf, area, ratatui::style::Style::new().bg(bg));
        let panel = Panel::framed(None).focused(true);
        let block = ratatui::widgets::Block::new()
            .borders(ratatui::widgets::Borders::ALL)
            .border_set(t.glyphs.border)
            .border_style(termrock_overlays::modal::border_style(ctx, bg));
        ratatui::widgets::Widget::render(block, area, buf);
        let _ = panel;
        let close = if self.dismiss.shows_close() {
            termrock_overlays::modal::close_button_with(&self.close_button, area, buf, ctx, bg)
        } else {
            Rect::ZERO
        };
        let inner = area.inner(Margin::new(1 + self.pad.x, 1 + self.pad.y));
        if inner.is_empty() {
            return;
        }
        let mut title_x = inner.x;
        if let Some(kind) = self.title_status {
            let (glyph, st) = t.status(kind);
            let fg = t.legible(st.fg.unwrap_or(t.text_primary), bg);
            buf.set_string(title_x, inner.y, glyph, st.fg(fg).bg(bg));
            title_x += termrock_text::cells::width(glyph) as u16 + 1;
        }
        buf.set_string(
            title_x,
            inner.y,
            termrock_text::cells::truncate(
                &self.title,
                inner.right().saturating_sub(title_x) as usize,
            ),
            t.title().bg(bg),
        );
        let body_y = inner.y + 2;
        let action_rows = self.action_rows(width);
        let actions_y = area.bottom().saturating_sub(1 + self.pad.y + action_rows);

        let fixed = self.fixed_rows(width);
        let scroll_bottom = actions_y.saturating_sub(1 + fixed).max(body_y);
        let viewport = scroll_bottom - body_y;
        let content = self.scroll_rows(width);
        self.scroll.set_content(content as usize);
        self.scroll.set_viewport(viewport as usize);
        if !self.body_scroll {
            self.scroll.offset = 0;
        }
        self.scroll.clamp();
        let offset = self.scroll.offset as u16;

        let field_x = inner.x.saturating_sub(TextInput::GUTTER).max(area.x + 1);
        let field_w = inner.right() - field_x;
        match &mut self.body {
            DialogBody::Text(text) => {
                let lines = termrock_text::cells::wrap(text, inner.width as usize);
                for (i, l) in lines
                    .iter()
                    .skip(offset as usize)
                    .take(viewport as usize)
                    .enumerate()
                {
                    buf.set_string(inner.x, body_y + i as u16, l, t.secondary().bg(bg));
                }
            }
            DialogBody::Input(inp) => {
                let r = Rect::new(field_x, body_y, field_w, inp.height_for(field_w));
                inp.render(r, buf, ctx, bg);
            }
            DialogBody::Facts { facts, code, ack } => {
                let sheet_area = Rect::new(0, 0, inner.width, content);
                let mut sheet = termrock_runtime::ctx::scratch_buffer(sheet_area);
                fill(&mut sheet, sheet_area, ratatui::style::Style::new().bg(bg));
                termrock_collections::props::render(sheet_area, &mut sheet, t, facts, bg);
                let facts_h = termrock_collections::props::measure(facts, inner.width);
                let mut y = scroll_bottom;
                if !code.is_empty() {
                    let extent = Self::code_extent(code, inner.width);
                    let off = self.scroll.h.offset;
                    self.scroll.h.set_content(extent.content_len);
                    self.scroll.h.set_viewport(extent.viewport_len);
                    let off = if self.scroll.h_cfg.enabled {
                        off.min(self.scroll.h.max_offset())
                    } else {
                        0
                    };
                    self.scroll.h.offset = off;
                    let cut = |line: &str, w: u16| {
                        if off > 0 {
                            termrock_text::cells::slice_cells(line, off, w as usize)
                        } else {
                            termrock_text::cells::truncate(line, w as usize)
                        }
                    };
                    let code_y = facts_h + 1;
                    for (i, line) in code.iter().enumerate() {
                        sheet.set_string(
                            0,
                            code_y + i as u16,
                            cut(line, inner.width),
                            t.secondary().bg(bg),
                        );
                    }
                    if self.scroll.h_cfg.shows_bar(&self.scroll.h) {
                        let hb = Rect::new(inner.x, y, inner.width, 1);
                        if hb.y < actions_y {
                            self.scroll.draw_h(hb, buf, ctx, self.id, true);
                        }
                        y += 1;
                    }
                }
                blit_rows(
                    &sheet,
                    offset,
                    buf,
                    Rect::new(inner.x, body_y, inner.width, viewport),
                );
                if let Some(a) = ack {
                    y += 1;
                    let r = Rect::new(field_x, y, field_w, a.input.height_for(field_w));
                    a.input.render(r, buf, ctx, bg);
                }
            }
        }
        if self.scroll.overflows() && viewport > 0 {
            let track = Rect::new(area.right() - 2, body_y, 1, viewport);
            self.scroll.set_track(self.id, Axis::Vertical, track);
            let below = self.scroll.max_offset() - self.scroll.offset;

            let hb = self.scroll.h_track();
            let h_bar = !hb.is_empty() && hb.y == scroll_bottom;
            if below > 0 && scroll_bottom < actions_y && !h_bar {
                buf.set_string(
                    inner.x,
                    scroll_bottom,
                    termrock_text::cells::truncate(
                        &format!(
                            "{} {}",
                            t.glyphs.arrows[1],
                            Strings::fill(t.strings.more_count, &[&below])
                        ),
                        inner.width as usize,
                    ),
                    t.muted().bg(bg),
                );
            }
        }

        if let DialogBody::Facts { ack: Some(a), .. } = &self.body {
            let armed = a.input.text().trim() == a.token;
            if let Some(b) = self.confirm_index.and_then(|i| self.actions.get_mut(i)) {
                b.disabled = !armed;
            }
        }

        let live = self.mnemonics_live();
        let keys = self.mnemonic_keys();
        for (b, m) in self.actions.iter_mut().zip(keys) {
            b.mnemonic = m.filter(|_| live).map(|(_, i)| i);
        }

        self.button_row.render(
            self.id,
            Rect::new(inner.x, actions_y, inner.width, action_rows),
            &mut self.actions,
            buf,
            ctx,
            bg,
        );

        ctx.hits.register_surface(self.id, area);

        match &self.body {
            DialogBody::Input(inp) => ctx.hits.register(inp.id, inp.area),
            DialogBody::Facts { ack: Some(a), .. } => ctx.hits.register(a.input.id, a.input.area),
            _ => {}
        }
        for b in &self.actions {
            ctx.hits.register(b.id, b.area);
        }
        if !close.is_empty() {
            ctx.clickable(termrock_overlays::modal::CLOSE, close);
        }

        let track = self.scroll.v_track();
        if !track.is_empty() {
            self.scroll.draw_v(track, buf, ctx, self.id, true);
        }
        let htrack = self.scroll.h_track();
        if !htrack.is_empty() && self.scroll.h.overflows() {
            ctx.hits.register(scrollbar::h_id_for(self.id), htrack);
        }
    }
}

fn blit_rows(src: &Buffer, from: u16, dst: &mut Buffer, at: Rect) {
    let at = at.intersection(*dst.area());
    for dy in 0..at.height {
        let sy = from + dy;
        if sy >= src.area.bottom() {
            break;
        }
        for dx in 0..at.width.min(src.area.width) {
            if let (Some(c), Some(d)) = (
                src.cell((src.area.x + dx, sy)),
                dst.cell_mut((at.x + dx, at.y + dy)),
            ) {
                *d = c.clone();
            }
        }
    }
}

impl Scrollable for Dialog {
    fn pane(&mut self) -> &mut ScrollPane {
        &mut self.scroll
    }

    fn wheel(&mut self, axis: Axis, delta: i32) -> Response<()> {
        let r = self.scroll.wheel(axis, delta);
        if r.is_consumed() {
            r
        } else {
            Response::consumed()
        }
    }
}

impl Dialog {
    pub fn pointer_in(
        &mut self,
        ev: &termrock_core::pointer::PointerEvent,
        focus: &mut termrock_core::focus::Focus,
    ) -> Response<()> {
        use termrock_core::intent::Phase;
        use termrock_core::pointer::MouseTarget;
        if let Some(inp) = self.input_mut()
            && let Some(r) = inp.route(ev)
        {
            if ev.is(Phase::Click) {
                focus.focus(inp.id);
            }
            return r;
        }
        if ev.wheel().is_some() && !ev.on(self.id) {
            return Response::ignored();
        }
        if ev.on(self.id)
            && let Some(r) = termrock_core::pointer::scroll_input(self, ev)
        {
            return r;
        }
        let (id, pos) = (ev.id, ev.pos());
        match ev.phase() {
            Some(Phase::Press) => self.on_press(id, pos, ev.count()).into(),
            Some(Phase::Drag) => self.on_drag(id, pos).into(),
            Some(Phase::Click) => self.on_click_count(id, pos, ev.count(), focus).into(),
            _ => Response::ignored(),
        }
    }
}

impl Specimen for Dialog {
    fn specimen_id(&self) -> WidgetId {
        self.id
    }

    fn supports(&self, state: SpecimenState) -> bool {
        use SpecimenState::*;
        match state {
            Rest | Hover | Pressed | Focused | FocusedHover | Disabled | Overflow => {
                !self.actions.is_empty()
            }
            Editing => dialog_field(self).is_some(),
            Selected | Busy | Error | Empty | Loading | Narrow => false,
        }
    }

    fn specimen_height(&self, _state: SpecimenState) -> u16 {
        self.height(self.width)
    }

    fn specimen(
        &self,
        state: SpecimenState,
        area: Rect,
        buf: &mut Buffer,
        ctx: &mut RenderCtx,
        bg: Color,
    ) {
        let _ = bg;
        let mut d = self.clone();
        d.outer_margin = ratatui::layout::Margin::new(0, 0);
        d.placement = termrock_runtime::popup::Placement::Center;
        d.backdrop = termrock_runtime::ctx::Backdrop::FULL;
        d.width = area.width;
        let target = dialog_target(&d);
        let v = state.visual();
        if v.disabled
            && let Some(b) = d.actions.get_mut(target)
        {
            b.disabled = true;
        }
        if state == SpecimenState::Overflow {
            d.max_height = Some(self.height(self.width));
            if let DialogBody::Text(t) = &mut d.body {
                t.push_str(
                    " The rest of this text is longer than the dialog has room for, \
                     so the body scrolls and says how much is below.",
                );
            }
        }
        let field = dialog_field(&d);
        if v.editing
            && let DialogBody::Input(i)
            | DialogBody::Facts {
                ack: Some(AckInput { input: i, .. }),
                ..
            } = &mut d.body
        {
            i.editing = true;
        }
        let button = d.actions.get(target).map(|b| b.id);
        ctx.interaction.hover = if v.hovered { button } else { None };
        ctx.interaction.pressed = if v.pressed { button } else { None };
        ctx.interaction.focus = match (v.focused, v.editing) {
            (true, true) => field,
            (true, false) => button,
            _ => None,
        };
        d.render_in(area, area, buf, ctx);
    }
}

fn dialog_target(d: &Dialog) -> usize {
    d.confirm_index
        .or_else(|| {
            d.actions.iter().position(|b| {
                matches!(
                    b.kind,
                    termrock_theme::ButtonKind::Primary | termrock_theme::ButtonKind::Danger
                )
            })
        })
        .unwrap_or(d.actions.len().saturating_sub(1))
}

fn dialog_field(d: &Dialog) -> Option<WidgetId> {
    match &d.body {
        DialogBody::Input(i) => Some(i.id),
        DialogBody::Facts { ack: Some(a), .. } => Some(a.input.id),
        _ => None,
    }
}

impl Overlay for Dialog {
    type Action = DialogResult;

    fn dismiss(&self) -> Dismiss {
        self.dismiss_in_force()
    }
    fn spec(&self) -> LayerSpec {
        LayerSpec::modal(Overlay::dismiss(self)).initial_focus(self.initial_focus)
    }

    fn on_dismiss(&mut self, _: DismissReason) -> Response<DialogResult> {
        let _ = self.dismissed();
        Response::emit(self.result.unwrap_or(DialogResult::Cancelled))
    }
    fn close_button(&self) -> Option<&IconButton> {
        Some(&self.close_button)
    }
}

impl<R: From<LayerResult> + 'static, W: ?Sized + 'static> Layer<R, W> for Dialog {
    fn spec(&self) -> LayerSpec {
        Overlay::spec(self)
    }
    fn is_editing(&self) -> bool {
        Dialog::is_editing(self)
    }
    fn render(&mut self, screen: Rect, buf: &mut Buffer, ctx: &mut RenderCtx, _: &W) {
        Dialog::render(self, screen, buf, ctx);
    }
    fn on_key(&mut self, key: &Key, cx: &mut LayerCx<'_, W>) -> Outcome {
        Dialog::on_key(self, key, cx.focus, cx.ring).or(Outcome::Consumed)
    }
    fn pointer(&mut self, ev: &PointerEvent, cx: &mut LayerCx<'_, W>) -> Outcome {
        Dialog::pointer_in(self, ev, cx.focus).outcome()
    }
    fn on_click_count(
        &mut self,
        id: WidgetId,
        pos: Position,
        count: u8,
        cx: &mut LayerCx<'_, W>,
    ) -> Outcome {
        Dialog::on_click_count(self, id, pos, count, cx.focus)
    }
    fn on_click(&mut self, id: WidgetId, pos: Position, cx: &mut LayerCx<'_, W>) -> Outcome {
        Dialog::on_click_count(self, id, pos, 1, cx.focus)
    }
    fn on_press(&mut self, id: WidgetId, pos: Position, count: u8) -> Outcome {
        Dialog::on_press(self, id, pos, count)
    }
    fn on_drag(&mut self, pressed: WidgetId, pos: Position) -> Outcome {
        Dialog::on_drag(self, pressed, pos)
    }
    fn on_wheel(&mut self, delta: i32, _: Position) -> Outcome {
        Scrollable::wheel(self, Axis::Vertical, delta).outcome()
    }
    fn on_wheel_h(&mut self, delta: i32, _: Position) -> Outcome {
        Scrollable::wheel(self, Axis::Horizontal, delta).outcome()
    }
    fn on_paste(&mut self, text: &str) -> Outcome {
        Dialog::on_paste(self, text)
    }
    fn on_blur(&mut self, change: FocusChange) -> Outcome {
        Dialog::on_blur(self, change)
    }
    fn hints(&self, focused: Option<WidgetId>) -> Vec<Hint> {
        Dialog::hints(self, focused)
    }
    fn on_dismiss(&mut self, reason: DismissReason) {
        let _ = Overlay::on_dismiss(self, reason);
    }
    fn take_result(&mut self) -> Option<LayerOut<R>> {
        let action = match self.result.take()? {
            DialogResult::Action(i) => Some(i),
            DialogResult::Cancelled => None,
        };
        let text = match &self.body {
            DialogBody::Input(i) => Some(i.text().to_owned()),
            _ => None,
        };
        Some(layer_out(LayerResult::Dialog { action, text }, true))
    }
}
