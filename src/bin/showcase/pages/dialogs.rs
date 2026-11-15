use ratatui::buffer::Buffer;
use ratatui::layout::{Margin, Rect, Size};

use crate::app::PageId;
use crate::options::{PageValues, page_options};
use crate::pages::layout::{sheet_height, sheet_width};
use crate::pages::template::{Section, Template};
use crate::pages::{Cx, CxExt, Hint, Page, PageEvent};
use junie_tui::core::edit::EditEntry;
use junie_tui::core::event::Outcome;
use junie_tui::core::id::WidgetId;
use junie_tui::core::intent::Phase;
use junie_tui::core::keymap::{KeyBinding, NavKeys};
use junie_tui::core::pointer::MouseTarget;
use junie_tui::theme::StatusKind;
use junie_tui::ui::ctx::{Backdrop, RenderCtx};
use junie_tui::ui::popup::{Dismiss, Placement};
use junie_tui::ui::specimen::{SpecimenState, StateGrid};
use junie_tui::widgets::button::{Button, ButtonRow, RowAlign, RowOverflow, row_layout};
use junie_tui::widgets::dialog::{
    AckEnter, Dialog, DialogBody, DialogCmd, DialogResult, OUTER_MARGIN,
};
use junie_tui::widgets::icon_button::IconButton;
use junie_tui::widgets::input::TextInput;
use junie_tui::widgets::panel::Panel;
use junie_tui::widgets::props::{self, Prop};

const ID: WidgetId = WidgetId::of("dialogs");
const CONFIRM: WidgetId = ID.sub("confirm");
const RENAME: WidgetId = ID.sub("rename");
const CHOICE: WidgetId = ID.sub("choice");
const DELETE: WidgetId = ID.sub("delete");
const VIEWER: WidgetId = ID.sub("viewer");
const GATE1: WidgetId = ID.sub("gate1");
const GATE2: WidgetId = ID.sub("gate2");
const PUSH: WidgetId = ID.sub("push");
const NOTES: WidgetId = ID.sub("notes");

const VARIANTS: &[(&str, &str)] = &[
    ("Confirm run", "Text body"),
    ("Rename task…", "Input body"),
    ("Unsaved changes…", "three actions"),
    ("Delete branch…", "▲ title status"),
    ("View config", "Facts · Close only"),
    ("Drop table…", "1 of 2 · 2 of 2"),
    ("Push failed…", "Retry · Force push"),
    ("Release notes", "body that scrolls"),
];

const RELEASE_NOTES: &str = "Version 4.2 moves every popup and modal onto one layer model. \
Dialogs, pickers, menus and selects now share their dismissal policy: Esc, an outside click \
and the close button all go through it, and a dismissal always resolves to the cancel \
action. Focus returns to the control that opened a layer when it closes. The footer hints \
of a dialog are built from its state, so they never disagree with the keys. Tall bodies \
scroll with the wheel, the arrows, PgUp/PgDn, Home/End and the scrollbar, and the dialog \
says how much is below. A typed acknowledgement arms its action only once the token \
matches. Buttons that do not fit wrap onto a second row instead of being cut. Mnemonics \
underline a letter of each action; they work only while the dialog has no text field, \
so typing never presses a button. Accelerators y and n can be bound through the key map. \
The backdrop keeps the footer live, because its hints belong to the dialog.";

fn rename_validator(s: &str) -> Option<String> {
    if s.trim().is_empty() {
        Some("Name cannot be empty".into())
    } else if s.len() > 40 {
        Some("Keep it under 40 characters".into())
    } else {
        None
    }
}

fn facts() -> Vec<Prop> {
    vec![
        Prop::new("Table", "public.orders"),
        Prop::new("Rows", "1,284,113"),
        Prop::new("Connection", "production · eu-west-1"),
    ]
}

pub struct DialogsPage {
    template: Template,
    opts: PageValues,
    pub buttons: Vec<Button>,
    pub history: Vec<String>,
    task_name: String,
}

impl DialogsPage {
    pub fn new() -> Self {
        let buttons = VARIANTS
            .iter()
            .enumerate()
            .map(|(i, (label, _))| match i {
                0 => Button::primary(ID.child(i), label),
                3 => Button::danger(ID.child(i), label),
                _ => Button::secondary(ID.child(i), label),
            })
            .collect();
        Self {
            template: Template::new(ID, states_grid()),
            opts: PageValues::new(page_options(PageId::Dialogs)),
            buttons,
            history: vec![],
            task_name: "Migrate sessions table".into(),
        }
    }

    fn variant(&self, i: usize) -> Dialog {
        match i {
            0 => Dialog::confirm(
                CONFIRM,
                "Run task now?",
                "Junie will check out chore/uuid-sessions, apply the plan and run the test suite. You can pause at any step.",
                "Run",
            ),
            1 => {
                let input = TextInput::new(RENAME.sub("input"), "Task name")
                    .value(&self.task_name)
                    .required(true)
                    .required_message("Name cannot be empty")
                    .validator(rename_validator)
                    .help("Shown in the task list and PR title");
                Dialog::prompt(RENAME, "Rename task", input, "Rename")
            }
            2 => {
                let mut d = Dialog::confirm(
                    CHOICE,
                    "Unsaved changes",
                    "The description was edited. Save before leaving this page?",
                    "Save",
                )
                .actions(
                    vec![
                        Button::subtle(CHOICE.sub("cancel"), "Cancel"),
                        Button::secondary(CHOICE.sub("discard"), "Discard"),
                        Button::primary(CHOICE.sub("save"), "Save"),
                    ],
                    Some(0),
                );
                d.initial_focus = CHOICE.sub("save");
                d
            }
            3 => Dialog::destructive(
                DELETE,
                "Delete branch?",
                "feat/rate-limit has 14 commits that are not on main. This cannot be undone.",
                "Delete branch",
            )
            .title_status(StatusKind::Warn),
            4 => {
                let close = Button::primary(VIEWER.sub("close"), "Close");
                let mut d = Dialog::facts(
                    VIEWER,
                    "Connection config",
                    vec![
                        Prop::new("Name", "orders-prod"),
                        Prop::new("Driver", "postgres 16"),
                    ],
                    [
                        "{",
                        "  \"host\": \"db.eu-west-1.internal\",",
                        "  \"port\": 5432,",
                        "  \"ssl\": \"verify-full\",",
                        "  \"pool\": { \"min\": 2, \"max\": 16 },",
                        "  \"user\": \"orders_ro\",",
                        "  \"database\": \"orders\",",
                        "  \"statement_timeout\": \"30s\",",
                        "  \"application_name\": \"tablepro\"",
                        "}",
                    ]
                    .map(String::from)
                    .to_vec(),
                    None,
                    close,
                )
                .actions(vec![Button::primary(VIEWER.sub("close"), "Close")], Some(0));
                d.initial_focus = VIEWER.sub("close");
                d
            }
            5 => Dialog::facts(
                GATE1,
                "Drop table orders? · 1 of 2",
                facts(),
                vec!["DROP TABLE public.orders CASCADE;".into()],
                None,
                Button::danger(GATE1.sub("go"), "Continue"),
            )
            .title_status(StatusKind::Warn),
            6 => {
                let d = Dialog::destructive(
                    PUSH,
                    "Push rejected",
                    "The remote has 3 commits you do not have. Retry after a fetch, or force the push and drop them from the remote.",
                    "Force push",
                )
                .actions(
                    vec![
                        Button::subtle(PUSH.sub("cancel"), "Cancel"),
                        Button::secondary(PUSH.sub("retry"), "Retry"),
                        Button::danger(PUSH.sub("force"), "Force push"),
                    ],
                    Some(0),
                )
                .cancel_label("Keep local")
                .title_status(StatusKind::Fail);
                let mut d = d;
                d.initial_focus = PUSH.sub("retry");
                d
            }
            _ => {
                let mut d = Dialog::confirm(NOTES, "What's new in 4.2", RELEASE_NOTES, "Got it")
                    .actions(vec![Button::primary(NOTES.sub("ok"), "Got it")], Some(0));
                d.initial_focus = NOTES.sub("ok");
                d
            }
        }
    }

    fn gate2(&self) -> Dialog {
        Dialog::facts(
            GATE2,
            "Drop table orders? · 2 of 2",
            facts(),
            vec![],
            Some("orders"),
            Button::danger(GATE2.sub("go"), "Drop table"),
        )
        .title_status(StatusKind::Fail)
    }

    fn style(&self, mut d: Dialog, anchor: Rect) -> Dialog {
        let o = &self.opts;
        match o.value("dialogs.width") {
            "44" => d = d.width(44),
            "80" => d = d.width(80),
            _ => {}
        }
        let placement = match o.value("dialogs.placement") {
            "below" => Placement::Below,
            "above" => Placement::Above,
            "right" => Placement::Right,
            "left" => Placement::Left,
            _ => Placement::Center,
        };
        d = if placement == Placement::Center {
            d.placement(placement)
        } else {
            d.anchor(anchor, placement)
        };
        if o.value("dialogs.chrome") == "3-2" {
            d = d.chrome(3, 2);
        }
        d = d.outer_margin(match o.value("dialogs.outer_margin") {
            "0x0" => Margin::new(0, 0),
            "8x3" => Margin::new(8, 3),
            _ => OUTER_MARGIN,
        });
        d = d.max_height(match o.value("dialogs.max_height") {
            "14" => Some(14),
            "9" => Some(9),
            _ => None,
        });
        d = d.body_scroll(o.on("dialogs.body_scroll"));
        d = d.backdrop(match o.value("dialogs.backdrop") {
            "full" => Backdrop::FULL,
            "keep-top" => Backdrop::DEFAULT.keep_top(2),
            _ => Backdrop::DEFAULT,
        });
        match o.value("dialogs.dismiss") {
            "light" => d = d.dismiss(Dismiss::LIGHT),
            "escape" => d = d.dismiss(Dismiss::ESCAPE),
            "never" => d = d.dismiss(Dismiss::NEVER),
            _ => {}
        }
        if o.value("dialogs.close_button") == "danger" {
            d = d.close_button(
                IconButton::close(junie_tui::ui::modal::CLOSE)
                    .with_glyph("✕")
                    .danger_hover(true),
            );
        }
        if o.value("dialogs.accelerators") == "y-n" {
            d = d.accelerators(Some(('y', 'n')));
        }
        if o.value("dialogs.keymap") == "ctrl" {
            d = d.keymap(vec![
                (KeyBinding::ctrl('y'), DialogCmd::Confirm),
                (KeyBinding::ctrl('n'), DialogCmd::Cancel),
            ]);
        }
        d = d.mnemonics(o.on("dialogs.mnemonics"));
        d = d.nav(match o.value("dialogs.nav") {
            "vim" => NavKeys::VIM,
            _ => NavKeys::DESKTOP,
        });
        d = d.ack_enter(match o.value("dialogs.ack_enter") {
            "next-field" => AckEnter::NextField,
            _ => AckEnter::Submit,
        });
        d = d.guard_dirty_prompt(o.on("dialogs.guard_dirty"));
        d = d.guard_destructive_dismiss(o.on("dialogs.guard_destructive"));
        d = d.edit_entry(match o.value("dialogs.edit_entry") {
            "type-to-edit" => EditEntry::TypeToEdit,
            "explicit" => EditEntry::Explicit,
            _ => EditEntry::Immediate,
        });
        d = d.button_row(match o.value("dialogs.button_row") {
            "start-wrap" => ButtonRow::new()
                .align(RowAlign::Start)
                .overflow(RowOverflow::Wrap),
            "end-more" => ButtonRow::new()
                .align(RowAlign::End)
                .overflow(RowOverflow::More),
            _ => ButtonRow::new()
                .align(RowAlign::End)
                .overflow(RowOverflow::Wrap),
        });
        if o.value("dialogs.cancel_label") == "keep" {
            d = d.cancel_label("Keep");
        }
        if o.value("dialogs.confirm_index") == "nothing" {
            d = d.confirm_index(None);
        }
        d
    }

    fn open(&mut self, i: usize, cx: &mut Cx) {
        let anchor = self.buttons.get(i).map_or(Rect::ZERO, |b| b.area);
        cx.open_dialog(self.style(self.variant(i), anchor));
    }

    fn layout_at(&self, width: u16) -> (usize, [u16; 3]) {
        let per = ((width.saturating_sub(2) / 22).max(1) as usize).min(4);
        let lines = self.buttons.len().div_ceil(per) as u16 * 3 - 1;
        let applied = Panel::card(Some("Applied")).meta(APPLIED_META);
        let results = 2 + (self.history.len() as u16).max(1);
        (
            per,
            [
                lines + 3,
                sheet_height(&applied, &self.readout(), width),
                Panel::card(Some("Results"))
                    .outer(Size::new(0, results))
                    .height,
            ],
        )
    }

    fn render_playground(&mut self, area: Rect, buf: &mut Buffer, ctx: &mut RenderCtx) {
        let t = ctx.theme;

        let (per, [buttons_h, applied_h, _]) = self.layout_at(area.width);
        let notes = true;
        let rows = crate::pages::layout::rows(area, &[buttons_h, 1, 0]);
        let panel = Panel::card(Some("Open a dialog")).meta("Enter · click");
        let bg = panel.bg(t);
        let inner = panel.render(rows[0], buf, t);
        let step = if notes { 3 } else { 1 };
        for (r, chunk) in self.buttons.chunks_mut(per).enumerate() {
            let y = inner.y + r as u16 * step;
            if y >= inner.bottom() {
                break;
            }
            let widths: Vec<u16> = chunk.iter().map(|b| b.width().max(20)).collect();
            let rects = row_layout(Rect::new(inner.x, y, inner.width, 1), &widths, 2);
            for (k, (b, rect)) in chunk.iter_mut().zip(rects).enumerate() {
                let w = b.width().min(rect.width);
                b.render(Rect::new(rect.x, rect.y, w, 1), buf, ctx, bg);
                if notes && y + 1 < inner.bottom() {
                    buf.set_string(
                        rect.x,
                        y + 1,
                        junie_tui::ui::text::truncate(VARIANTS[r * per + k].1, rect.width as usize),
                        t.faint().bg(bg),
                    );
                }
            }
        }

        let rest = crate::pages::layout::rows(rows[2], &[applied_h, 1, 0]);
        let (left, right) = (rest[0], rest[2]);
        let panel = Panel::card(Some("Applied")).meta(APPLIED_META);
        let bg = panel.bg(t);
        let inner = panel.render(left, buf, t);
        props::render(inner, buf, t, &self.readout(), bg);

        let panel = Panel::card(Some("Results")).meta("DialogResult");
        let bg = panel.bg(t);
        let inner = panel.render(right, buf, t);
        buf.set_string(
            inner.x,
            inner.y,
            junie_tui::ui::text::truncate(
                &format!("Task: {} · Action(i) or Cancelled", self.task_name),
                inner.width as usize,
            ),
            t.secondary().bg(bg),
        );
        if self.history.is_empty() && inner.height > 2 {
            buf.set_string(inner.x, inner.y + 2, "Nothing yet", t.muted().bg(bg));
        }
        for (i, h) in self.history.iter().rev().enumerate() {
            let st = if i == 0 { t.primary() } else { t.muted() };
            buf.set_string(
                inner.x,
                inner.y + 2 + i as u16,
                junie_tui::ui::text::truncate(h, inner.width as usize),
                st.bg(bg),
            );
        }
    }

    fn readout(&self) -> Vec<Prop> {
        let d = self.style(self.variant(0), Rect::ZERO);
        let gate = self.style(self.gate2(), Rect::ZERO);
        let keys = |d: &Dialog| -> String {
            let mut parts: Vec<String> = d
                .keymap
                .iter()
                .map(|(k, c)| format!("{} {c:?}", k.label()))
                .collect();
            if d.mnemonics {
                parts.push("mnemonics".into());
            }
            if parts.is_empty() {
                "none".into()
            } else {
                parts.join(" · ")
            }
        };
        let dismiss = |s: Dismiss| -> String {
            let mut v = vec![];
            if s.escape {
                v.push("Esc");
            }
            if s.outside_click {
                v.push("outside click");
            }
            if s.shows_close() {
                v.push("close button");
            }
            if v.is_empty() {
                "actions only".into()
            } else {
                v.join(" · ")
            }
        };
        let body = |d: &Dialog| match &d.body {
            DialogBody::Text(_) => "Text",
            DialogBody::Input(_) => "Input",
            DialogBody::Facts { .. } => "Facts",
        };
        vec![
            Prop::new(
                "Size",
                format!(
                    "width {} · max height {} · body scroll {}",
                    d.width,
                    d.max_height.map_or("none".into(), |h| h.to_string()),
                    if d.body_scroll { "on" } else { "off" }
                ),
            ),
            Prop::new(
                "Place",
                format!(
                    "{:?}{} · chrome {}+{} · margin {}x{}",
                    d.placement,
                    if d.placement == Placement::Center {
                        ""
                    } else {
                        " the button"
                    },
                    d.chrome.0,
                    d.chrome.1,
                    d.outer_margin.horizontal,
                    d.outer_margin.vertical
                ),
            ),
            Prop::new(
                "Backdrop",
                format!(
                    "keeps {} top · {} bottom rows live",
                    d.backdrop.keep_top, d.backdrop.keep_bottom
                ),
            ),
            Prop::new(
                "Dismiss",
                format!(
                    "{} · typed token: {} · glyph {}",
                    dismiss(d.dismiss),
                    dismiss(gate.dismiss),
                    d.close_button.glyph(&junie_tui::theme::Theme::junie())
                ),
            ),
            Prop::new("Keys", keys(&d)),
            Prop::new(
                "Nav",
                if d.nav == NavKeys::VIM {
                    "vim"
                } else {
                    "desktop"
                },
            ),
            Prop::new(
                "Fields",
                format!(
                    "Enter in token {:?} · guard edited prompt {}",
                    gate.ack_enter,
                    if d.guard_dirty_prompt { "on" } else { "off" }
                ),
            ),
            Prop::new(
                "Edit entry",
                match &self.style(self.variant(1), Rect::ZERO).body {
                    DialogBody::Input(i) => format!(
                        "{:?} (the prompt's field)",
                        i.edit_entry.unwrap_or_default()
                    ),
                    _ => String::new(),
                },
            ),
            Prop::new(
                "Buttons",
                format!(
                    "{:?} {:?} · cancel “{}” · token gates {}",
                    d.button_row.align,
                    d.button_row.overflow,
                    d.actions[0].label,
                    gate.confirm_index
                        .and_then(|i| gate.actions.get(i))
                        .map_or("nothing".into(), |b| b.label.clone())
                ),
            ),
            Prop::new(
                "Bodies",
                format!(
                    "{} · {} · {}",
                    body(&d),
                    body(&self.variant(1)),
                    body(&gate)
                ),
            ),
        ]
    }

    fn result_line(&mut self, id: WidgetId, result: DialogResult, cx: &mut Cx) {
        let (label, msg) = match (id, result) {
            (d, DialogResult::Action(1)) if d == CONFIRM => ("Run", "Task started".to_owned()),
            (d, DialogResult::Action(1)) if d == DELETE => {
                ("Delete", "Branch feat/rate-limit deleted".to_owned())
            }
            (d, DialogResult::Action(1)) if d == RENAME => {
                ("Rename", format!("Renamed to “{}”", self.task_name))
            }
            (d, DialogResult::Action(2)) if d == CHOICE => ("Save", "Description saved".to_owned()),
            (d, DialogResult::Action(1)) if d == CHOICE => {
                ("Discard", "Changes discarded".to_owned())
            }
            (d, DialogResult::Action(0)) if d == VIEWER || d == NOTES => {
                ("Close", "Closed".to_owned())
            }
            (d, DialogResult::Action(1)) if d == GATE1 => {
                ("Continue", "Gate 1 of 2 passed".to_owned())
            }
            (d, DialogResult::Action(1)) if d == GATE2 => {
                ("Drop", "Table public.orders dropped".to_owned())
            }
            (d, DialogResult::Action(1)) if d == PUSH => ("Retry", "Retrying the push".to_owned()),
            (d, DialogResult::Action(2)) if d == PUSH => ("Force", "Forced the push".to_owned()),
            _ => ("Cancel", "Cancelled".to_owned()),
        };
        self.history.push(format!("{label:<8} {msg} · {result:?}"));
        cx.status(msg);
    }
}

fn states_grid() -> StateGrid {
    use SpecimenState::*;
    let id = ID.sub("states");
    StateGrid::new(id)
        .rows(&[Rest, Hover, Focused, Disabled, Editing, Overflow])
        .column_width(45)
        .column(
            "Confirm",
            Dialog::confirm(
                id.child(0),
                "Run task now?",
                "Apply the plan and run the tests.",
                "Run",
            )
            .width(44),
        )
        .column(
            "Typed token",
            Dialog::facts(
                id.child(1),
                "Drop table?",
                vec![],
                vec![],
                Some("orders"),
                Button::danger(id.child(1).sub("go"), "Drop"),
            )
            .width(44),
        )
}

const APPLIED_META: &str = "read back from the built dialog";

const MIN_W: u16 = 24;

impl Page for DialogsPage {
    fn title(&self) -> &'static str {
        "Dialogs"
    }
    fn blurb(&self) -> &'static str {
        "Focus is trapped, the page dims, Esc always cancels"
    }

    fn measure(&self, avail: Size, _ctx: &RenderCtx) -> Size {
        let applied = Panel::card(Some("Applied")).meta(APPLIED_META);
        let w = avail
            .width
            .max(sheet_width(&applied, &self.readout()))
            .max(MIN_W);
        let (_, [a, b, c]) = self.layout_at(w);
        Size::new(w, a + 1 + b + 1 + c)
    }

    fn render(&mut self, area: Rect, buf: &mut Buffer, ctx: &mut RenderCtx) {
        let body = self.template.render(area, buf, ctx);
        match self.template.section() {
            Section::Playground => self.render_playground(body, buf, ctx),
            Section::Options => self.template.render_options(body, buf, ctx),
            Section::States => self.template.render_states(body, buf, ctx),
        }
    }

    fn handle(&mut self, ev: &PageEvent, cx: &mut Cx) -> Outcome {
        if let PageEvent::DialogClosed { id, result, value } = ev {
            if *id == RENAME
                && *result == DialogResult::Action(1)
                && let Some(v) = value
            {
                self.task_name = v.clone();
            }
            self.result_line(*id, *result, cx);

            if *id == GATE1 && *result == DialogResult::Action(1) {
                let anchor = self.buttons[5].area;
                cx.open_dialog(self.style(self.gate2(), anchor));
            }
            return Outcome::Changed;
        }
        if let Some(o) = self.template.handle(ev, cx) {
            return o;
        }
        if self.template.section() != Section::Playground {
            return Outcome::Ignored;
        }
        match ev {
            PageEvent::Key(key) => {
                if key.is_char('d') {
                    self.open(3, cx);
                    return Outcome::Changed;
                }
                let Some(f) = cx.focus.current() else {
                    return Outcome::Ignored;
                };
                let Some(i) = self.buttons.iter().position(|b| b.id == f) else {
                    return Outcome::Ignored;
                };
                let (o, act) = self.buttons[i].on_key(key);
                if act {
                    self.open(i, cx);
                }
                o
            }
            PageEvent::Pointer(ev) if ev.is(Phase::Click) => {
                let Some(i) = self.buttons.iter().position(|b| ev.on(b.id)) else {
                    return Outcome::Ignored;
                };
                cx.focus.focus(ev.id);
                if self.buttons[i].route(ev).is_some_and(|r| r.has_actions()) {
                    self.open(i, cx);
                }
                Outcome::Changed
            }
            _ => Outcome::Ignored,
        }
    }

    fn hints(&self, focus: Option<WidgetId>) -> Vec<Hint> {
        if let Some(h) = self.template.hints(focus) {
            return h;
        }
        vec![("Enter", "Open"), ("d", "Delete branch")]
    }

    fn option_value(&self, key: &str) -> Option<usize> {
        self.opts.get(key)
    }

    fn set_option(&mut self, key: &str, value: usize) {
        self.opts.set(key, value);
    }

    fn as_any(&self) -> Option<&dyn std::any::Any> {
        Some(self)
    }

    fn template(&self) -> Option<&Template> {
        Some(&self.template)
    }

    fn template_mut(&mut self) -> Option<&mut Template> {
        Some(&mut self.template)
    }
}
