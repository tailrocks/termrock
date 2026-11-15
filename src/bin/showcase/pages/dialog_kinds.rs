use ratatui::buffer::Buffer;
use ratatui::layout::{Rect, Size};

use crate::app::PageId;
use crate::options::{PageValues, page_options};
use crate::pages::layout::{sheet_height, sheet_width};
use crate::pages::template::{Section, Template};
use crate::pages::{Cx, Hint, Page, PageEvent, Report};
use junie_tui::core::event::Outcome;
use junie_tui::core::id::WidgetId;
use junie_tui::core::intent::Phase;
use junie_tui::core::keymap::{KeyBinding, NavKeys};
use junie_tui::core::pointer::MouseTarget;
use junie_tui::theme::Tone;
use junie_tui::ui::ctx::RenderCtx;
use junie_tui::ui::layer::LayerTag;
use junie_tui::ui::layers::LayerResult;
use junie_tui::ui::popup::Dismiss;
use junie_tui::ui::specimen::{SpecimenState, StateGrid};
use junie_tui::ui::text;
use junie_tui::widgets::button::{Button, ButtonRow, row_layout};
use junie_tui::widgets::choice::{Checkbox, RadioGroup};
use junie_tui::widgets::choice_dialog::ChoiceDialog;
use junie_tui::widgets::form_dialog::{
    FieldValue, FormAction, FormDialog, FormField, FormFieldKind,
};
use junie_tui::widgets::help::HelpOverlay;
use junie_tui::widgets::icon_button::IconButton;
use junie_tui::widgets::info_dialog::{InfoCmd, InfoDialog, InfoResult};
use junie_tui::widgets::input::TextInput;
use junie_tui::widgets::option_list::Item;
use junie_tui::widgets::panel::Panel;
use junie_tui::widgets::picker::{Picker, PickerAction};
use junie_tui::widgets::props::{self, Prop};
use junie_tui::widgets::select::Select;

const ID: WidgetId = WidgetId::of("dialogkinds");
const FORM: WidgetId = ID.sub("form");
const INFO: WidgetId = ID.sub("info");
const CHOICE: WidgetId = ID.sub("choice");
const HELP: WidgetId = ID.sub("help");
const KEYS: WidgetId = ID.sub("keys");

const DRIVERS: &[&str] = &["PostgreSQL", "MySQL", "SQLite"];
const SSL: &[&str] = &["Disable", "Require", "Verify full"];
const CHANNELS: &[&str] = &["Stable", "Beta", "Nightly (may break)"];
const KEYCHAIN_SECRET: &str = "keychain-rotated-0042";
const KEY_FILES: &[&str] = &["~/.ssh/id_ed25519", "~/.ssh/prod_rsa", "~/keys/staging.pem"];

const OPENERS: &[(&str, &str)] = &[
    ("Add connection…", "every field kind"),
    ("Server info", "copy rows with y"),
    ("Release channel…", "toned radios"),
    ("Keyboard help", "keys by section"),
];

pub struct DialogKindsPage {
    template: Template,
    opts: PageValues,
    pub buttons: Vec<Button>,

    open: Vec<&'static str>,

    pub log: Vec<String>,
    channel: usize,
}

impl DialogKindsPage {
    pub fn new() -> Self {
        let buttons = OPENERS
            .iter()
            .enumerate()
            .map(|(i, (label, _))| {
                if i == 0 {
                    Button::primary(ID.child(i), label)
                } else {
                    Button::secondary(ID.child(i), label)
                }
            })
            .collect();
        Self {
            template: Template::new(ID, states_grid()),
            opts: PageValues::new(page_options(PageId::DialogKinds)),
            buttons,
            open: vec![],
            log: vec![],
            channel: 0,
        }
    }

    fn width(&self, default: u16) -> u16 {
        match self.opts.value("dialogkinds.width") {
            "48" => 48,
            "72" => 72,
            _ => default,
        }
    }

    fn nav(&self) -> NavKeys {
        match self.opts.value("dialogkinds.nav") {
            "vim" => NavKeys::VIM,
            _ => NavKeys::default(),
        }
    }

    fn close_button(&self) -> IconButton {
        let b = IconButton::close(junie_tui::ui::modal::CLOSE);
        if self.opts.value("dialogkinds.close_button") == "danger" {
            b.with_glyph("✕").danger_hover(true)
        } else {
            b
        }
    }

    fn dismiss(&self, default: Dismiss) -> Dismiss {
        match self.opts.value("dialogkinds.dismiss") {
            "light" => Dismiss::LIGHT,
            "escape" => Dismiss::ESCAPE,
            "never" => Dismiss::NEVER,
            _ => default,
        }
    }

    fn button_row(&self) -> ButtonRow {
        ButtonRow::new().wrap(self.opts.on("dialogkinds.button_wrap"))
    }

    pub fn form(&self) -> FormDialog {
        let mut f = FormDialog::new(
            FORM,
            "Add connection",
            vec![
                FormField::input(
                    "name",
                    TextInput::new(FORM.sub("name"), "Name").value("orders-prod"),
                ),
                FormField::select(
                    "driver",
                    Select::new(FORM.sub("driver"), "Driver", DRIVERS, 0),
                ),
                FormField::input(
                    "host",
                    TextInput::new(FORM.sub("host"), "Host").value("db.eu-west-1.internal"),
                ),
                FormField::input(
                    "password",
                    TextInput::new(FORM.sub("password"), "Password")
                        .value("hunter2-prod-7731")
                        .masked()
                        .reveal_tail(4),
                ),
                FormField::radio("ssl", RadioGroup::new(FORM.sub("ssl"), "SSL mode", SSL, 2)),
                FormField::check(
                    "tunnel",
                    Checkbox::new(FORM.sub("tunnel"), "Connect through an SSH tunnel", false),
                ),
                FormField::chooser("key", FORM.sub("key"), "SSH key", KEY_FILES[0], "Choose…")
                    .hidden(),
                FormField::note(
                    "preview",
                    preview_lines("orders-prod", "db.eu-west-1.internal", 0),
                ),
                FormField::note("result", vec![]),
            ],
        )
        .action(
            "test",
            Button::secondary(FORM.sub("test"), "Test connection"),
        )
        .width(self.width(62))
        .nav(self.nav())
        .dismiss(self.dismiss(Dismiss::ESCAPE))
        .close_button(self.close_button())
        .button_row(self.button_row());
        if self.opts.on("dialogkinds.form_meta") {
            f = f.meta("production");
        }
        if self.opts.on("dialogkinds.keep_open") {
            f = f.keep_open_on_save();
        }
        f
    }

    pub fn info(&self) -> InfoDialog {
        let mut d = InfoDialog::new(
            INFO,
            "Server info",
            vec![
                Prop::new("Host", "db.eu-west-1.internal").copyable(),
                Prop::new("Version", "PostgreSQL 16.4").copyable(),
                Prop::new("Uptime", "41 days"),
                Prop::new("Role", "read-only replica").tone(Tone::Warning),
            ],
        )
        .intro(vec![(
            "Rows marked ⧉ copy with y or a click.".into(),
            Tone::Muted,
        )])
        .detail(vec![
            "replication lag 120 ms".into(),
            "wal_level = logical".into(),
            "max_connections = 400".into(),
        ])
        .action(Button::secondary(INFO.sub("logs"), "Open logs"))
        .meta("read-only")
        .width(self.width(66))
        .nav(self.nav())
        .dismiss(self.dismiss(Dismiss::LIGHT))
        .close_button(self.close_button());
        if self.opts.on("dialogkinds.info_error") {
            d = d.error();
        }
        if self.opts.value("dialogkinds.info_keymap") == "ctrl" {
            d = d.keymap(vec![
                (KeyBinding::ctrl('w'), InfoCmd::Close),
                (KeyBinding::ctrl('y'), InfoCmd::Copy),
            ]);
        }
        d
    }

    pub fn choice(&self) -> ChoiceDialog {
        let mut d = ChoiceDialog::new(
            CHOICE,
            "Release channel",
            "Updates come from",
            CHANNELS,
            self.channel,
        )
        .line("Switching applies at the next restart.", Tone::Secondary)
        .width(self.width(54))
        .nav(self.nav())
        .dismiss(self.dismiss(Dismiss::ESCAPE))
        .close_button(self.close_button())
        .button_row(self.button_row());
        if self.opts.on("dialogkinds.choice_tones") {
            d = d.option_tones(vec![Tone::Normal, Tone::Normal, Tone::Warning]);
        }
        if self.opts.on("dialogkinds.choice_stepper") {
            d = d.stepper("Step 2 of 3 · Updates");
        }
        if self.opts.value("dialogkinds.choice_buttons") == "three" {
            d = d.buttons(
                vec![
                    Button::subtle(CHOICE.sub("cancel"), "Cancel"),
                    Button::secondary(CHOICE.sub("skip"), "Skip"),
                    Button::primary(CHOICE.sub("ok"), "Switch"),
                ],
                0,
            );
        }
        d
    }

    pub fn help(&self) -> HelpOverlay {
        HelpOverlay::new(
            HELP,
            "Dialog kinds",
            vec![
                (
                    "Forms",
                    vec![("Tab", "Next field"), ("Enter", "Save"), ("Esc", "Cancel")],
                ),
                ("Info", vec![("y", "Copy"), ("q", "Close")]),
                ("Choices", vec![("↑↓", "Option"), ("←→", "Button")]),
            ],
        )
        .close_button(self.close_button())
    }

    fn open(&mut self, i: usize, cx: &mut Cx) {
        let kind = match i {
            0 => "form",
            1 => "info",
            2 => "choice",
            _ => "reference",
        };
        let tag = LayerTag::new(kind);
        match i {
            0 => cx.open(self.form(), tag),
            1 => cx.open(self.info(), tag),
            2 => cx.open(self.choice(), tag),
            _ => cx.open(self.help(), tag),
        }
        self.open.push(kind);
    }

    fn open_key_picker(&mut self, cx: &mut Cx) {
        let mut p = Picker::new(KEYS, "SSH key");
        p.set_searchable(false)
            .expect("disabling picker search is valid");
        p.set_width(48);
        p.set_items(KEY_FILES.iter().map(|k| Item::new(*k).glyph("⚿")).collect())
            .unwrap_or_else(|error| p.report_item_error(error));
        cx.open(p, LayerTag::new("keys"));
        self.open.push("keys");
    }

    fn on_report(&mut self, kind: &str, r: Report, cx: &mut Cx) {
        if r.closed.is_some() {
            self.open.pop();
        }
        let Some(result) = r.result.clone() else {
            return;
        };
        match result {
            LayerResult::Form(FormAction::Changed(field)) => {
                self.log.push(format!("Form · Changed({field:?})"));
                cx.with_top::<FormDialog>(move |f| {
                    if field == "driver" {
                        let remote = f.choice("driver") != 2;
                        f.set_visible("host", remote);
                        f.set_visible("password", remote);
                    }
                    if field == "tunnel" {
                        let on = matches!(
                            f.field("tunnel").map(|x| x.value()),
                            Some(FieldValue::Bool(true))
                        );
                        f.set_visible("key", on);
                    }
                    let lines = preview_lines(&f.text("name"), &f.text("host"), f.choice("driver"));
                    f.set_note("preview", lines);
                });
            }
            LayerResult::Form(FormAction::Choose(field)) => {
                self.log.push(format!(
                    "Form · Choose({field:?}) · a picker stacks over it"
                ));
                self.open_key_picker(cx);
            }
            LayerResult::Form(FormAction::Action(name)) => {
                if name == "test" {
                    cx.with_top::<FormDialog>(|f| {
                        let ok = !f.text("host").trim().is_empty();
                        let mut lines = vec![if ok {
                            ("✓ Connected in 42 ms".into(), Tone::Success)
                        } else {
                            ("✗ Host is required".into(), Tone::Error)
                        }];
                        if ok {
                            f.set_text("password", KEYCHAIN_SECRET);
                            let masked = matches!(
                                f.field("password").map(|x| &x.kind),
                                Some(FormFieldKind::Input(i)) if i.masked
                            );
                            lines.push((
                                format!(
                                    "Password refreshed from the keychain · {}",
                                    if masked { "still masked" } else { "revealed" }
                                ),
                                Tone::Muted,
                            ));
                        }
                        f.set_note("result", lines);
                    });
                }
                self.log.push(format!("Form · Action({name:?})"));

                if name == "save" && !r.text("name").trim().is_empty() {
                    cx.close();
                    self.open.pop();
                    self.log.push("Form · saved, closed by the page".into());
                }
            }
            LayerResult::Form(FormAction::Save) => {
                let name = r.text("name");
                self.log.push(format!("Form · Save · {name}"));
                cx.status(format!("Saved connection {name}"));
            }
            LayerResult::Form(FormAction::Cancel) => self.log.push("Form · Cancel".into()),
            LayerResult::Picker(PickerAction::Chosen(i)) if kind == "keys" => {
                cx.with_top::<FormDialog>(move |f| {
                    f.set_chooser("key", KEY_FILES[i.min(KEY_FILES.len() - 1)], None);
                });
                self.log
                    .push(format!("Picker · Chosen({i}) · back on the form"));
            }
            LayerResult::Picker(ev) => self.log.push(format!("Picker · {ev:?}")),
            LayerResult::Info(InfoResult::Copy(v)) => {
                self.log.push(format!("Info · Copy({v:?})"));
                cx.status(format!("Copied {v}"));
            }
            LayerResult::Info(ev) => self.log.push(format!("Info · {ev:?}")),
            LayerResult::Choice(c) => {
                if let Some(i) = c {
                    self.channel = i;
                }
                self.log.push(format!("Choice · {c:?}"));
            }
            LayerResult::Help => self.log.push("Help · closed".into()),
            other => self.log.push(format!("{other:?}")),
        }
    }

    fn readout(&self) -> Vec<Prop> {
        let f = self.form();
        let i = self.info();
        let c = self.choice();
        let kinds: Vec<&str> = f
            .fields
            .iter()
            .map(|x| {
                use junie_tui::widgets::form_dialog::FormFieldKind as K;
                match &x.kind {
                    K::Input(_) => "Input",
                    K::Select(_) => "Select",
                    K::Check(_) => "Check",
                    K::Radio(_) => "Radio",
                    K::Chooser { .. } => "Chooser",
                    K::Note(_) => "Note",
                }
            })
            .collect();
        let hidden: Vec<&str> = f
            .fields
            .iter()
            .filter(|x| !x.visible)
            .map(|x| x.name.as_str())
            .collect();
        let on = |b: bool| if b { "on" } else { "off" };
        let mut values: Vec<&str> = Vec::new();
        for (_, v) in f.values() {
            let name = match v {
                FieldValue::Text(_) => "Text",
                FieldValue::Choice(_) => "Choice",
                FieldValue::Bool(_) => "Bool",
            };
            if !values.contains(&name) {
                values.push(name);
            }
        }
        vec![
            Prop::new(
                "Form fields",
                format!("{} · values {}", kinds.join(" · "), values.join(" · ")),
            ),
            Prop::new(
                "Form",
                format!(
                    "width {} · meta {} · hidden {} · actions {} · Save {}",
                    f.width,
                    f.meta.clone().unwrap_or_else(|| "none".into()),
                    hidden.join(","),
                    f.actions
                        .iter()
                        .map(|(n, _)| n.as_str())
                        .collect::<Vec<_>>()
                        .join(","),
                    if f.pop_on_save {
                        "closes"
                    } else {
                        "keeps it open"
                    }
                ),
            ),
            Prop::new(
                "Info",
                format!(
                    "width {} · meta {} · intro · detail {} rows · actions {} · error title {}",
                    i.width,
                    i.meta.clone().unwrap_or_default(),
                    i.detail.len(),
                    i.actions.len(),
                    on(i.error_title)
                ),
            ),
            Prop::new(
                "Info keys",
                i.keymap
                    .iter()
                    .map(|(k, c)| format!("{} {c:?}", k.label()))
                    .collect::<Vec<_>>()
                    .join(" · "),
            ),
            Prop::new(
                "Choice",
                format!(
                    "width {} · line · stepper {} · tones {} · buttons {}",
                    c.width,
                    on(c.stepper.is_some()),
                    on(!c.option_tones.is_empty()),
                    c.buttons
                        .iter()
                        .map(|b| b.label.as_str())
                        .collect::<Vec<_>>()
                        .join("/")
                ),
            ),
            Prop::new(
                "All",
                format!(
                    "nav {} · × {} · button row wrap {}",
                    if self.opts.value("dialogkinds.nav") == "vim" {
                        "vim"
                    } else {
                        "desktop"
                    },
                    self.opts.value("dialogkinds.close_button"),
                    on(f.button_row.wrap)
                ),
            ),
        ]
    }

    fn layout_at(&self, width: u16) -> (usize, [u16; 3]) {
        let per = (width.saturating_sub(2) / 22).clamp(1, 4) as usize;
        let lines = self.buttons.len().div_ceil(per) as u16 * 3 - 1;
        let applied = Panel::card(Some("Applied")).meta(APPLIED_META);
        let returned = Panel::card(Some("Returned")).meta("LayerResult");
        let inner = returned.inner(Size::new(width, 0)).width;
        let rows = if self.log.is_empty() {
            text::wrap(NOTHING_YET, inner.max(1) as usize).len() as u16
        } else {
            self.log.len() as u16
        };
        (
            per,
            [
                lines + 3,
                sheet_height(&applied, &self.readout(), width),
                returned.outer(Size::new(inner, rows)).height,
            ],
        )
    }

    fn render_playground(&mut self, area: Rect, buf: &mut Buffer, ctx: &mut RenderCtx) {
        let t = ctx.theme;

        let (per, [buttons_h, applied_h, _]) = self.layout_at(area.width);
        let notes = true;
        let rows = crate::pages::layout::rows(area, &[buttons_h, 1, applied_h, 1, 0]);
        let panel = Panel::card(Some("Open a dialog")).meta("Enter · click");
        let bg = panel.bg(t);
        let inner = panel.render(rows[0], buf, t);
        for (r, chunk) in self.buttons.chunks_mut(per).enumerate() {
            let y = inner.y + r as u16 * 3;
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
                        junie_tui::ui::text::truncate(OPENERS[r * per + k].1, rect.width as usize),
                        t.faint().bg(bg),
                    );
                }
            }
        }

        let panel = Panel::card(Some("Applied")).meta(APPLIED_META);
        let bg = panel.bg(t);
        let inner = panel.render(rows[2], buf, t);
        props::render(inner, buf, t, &self.readout(), bg);

        let panel = Panel::card(Some("Returned")).meta("LayerResult");
        let bg = panel.bg(t);
        let inner = panel.render(rows[4], buf, t);
        if self.log.is_empty() {
            for (i, l) in text::wrap(NOTHING_YET, inner.width.max(1) as usize)
                .iter()
                .enumerate()
            {
                buf.set_string(inner.x, inner.y + i as u16, l, t.muted().bg(bg));
            }
        }
        for (k, line) in self.log.iter().rev().enumerate() {
            let st = if k == 0 { t.primary() } else { t.muted() };
            buf.set_string(
                inner.x,
                inner.y + k as u16,
                junie_tui::ui::text::truncate(line, inner.width as usize),
                st.bg(bg),
            );
        }
    }
}

const APPLIED_META: &str = "read back from the built dialogs";
const NOTHING_YET: &str = "Nothing yet · Form(Save | Cancel | Changed | Choose | Action) · Info(Closed | Copy | Action) · Choice(Some(i) | None)";

fn preview_lines(name: &str, host: &str, driver: usize) -> Vec<(String, Tone)> {
    let scheme = ["postgres", "mysql", "sqlite"][driver.min(2)];
    vec![(format!("Preview  {scheme}://{host}/{name}"), Tone::Muted)]
}

fn states_grid() -> StateGrid {
    use SpecimenState::*;
    let id = ID.sub("states");
    StateGrid::new(id)
        .rows(&[Rest, Hover, Focused, Selected, Disabled])
        .column_width(50)
        .column(
            "Choice",
            ChoiceDialog::new(id.child(0), "Split", "Direction", &["Right", "Down"], 0).width(42),
        )
        .column(
            "Toned",
            ChoiceDialog::new(
                id.child(1),
                "Release channel",
                "Updates come from",
                &["Stable", "Nightly"],
                0,
            )
            .option_tones(vec![Tone::Normal, Tone::Warning])
            .width(42),
        )
}

impl Page for DialogKindsPage {
    fn title(&self) -> &'static str {
        "Dialog kinds"
    }
    fn blurb(&self) -> &'static str {
        "Forms, info, choices and help: the purpose-built modals on one layer stack"
    }

    fn measure(&self, avail: Size, _ctx: &RenderCtx) -> Size {
        let applied = Panel::card(Some("Applied")).meta(APPLIED_META);
        let w = avail
            .width
            .max(sheet_width(&applied, &self.readout()))
            .max(24);
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
        if !self.open.is_empty() {
            return Outcome::Ignored;
        }
        if let Some(o) = self.template.handle(ev, cx) {
            return o;
        }
        if self.template.section() != Section::Playground {
            return Outcome::Ignored;
        }
        match ev {
            PageEvent::Key(key) => {
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
            PageEvent::Pointer(p) => {
                let hit = (self.buttons.iter_mut().enumerate())
                    .find_map(|(i, b)| b.route(p).map(|r| (i, r)));
                let Some((i, r)) = hit else {
                    return Outcome::Ignored;
                };
                if p.is(Phase::Click) {
                    cx.focus.focus(p.id);
                }
                if r.action().is_some() {
                    self.open(i, cx);
                    return Outcome::Changed;
                }
                r.outcome()
            }
            _ => Outcome::Ignored,
        }
    }

    fn hints(&self, focus: Option<WidgetId>) -> Vec<Hint> {
        if let Some(kind) = self.open.last() {
            return match *kind {
                "form" => vec![("Tab", "Next field"), ("Enter", "Save"), ("Esc", "Cancel")],
                "info" if self.opts.value("dialogkinds.info_keymap") == "ctrl" => {
                    vec![("Ctrl+Y", "Copy"), ("Ctrl+W", "Close")]
                }
                "info" => vec![("y", "Copy"), ("q / Esc", "Close")],
                "choice" => vec![("↑↓", "Option"), ("Enter", "Choose"), ("Esc", "Cancel")],
                "keys" => vec![("↑↓", "Move"), ("Enter", "Choose"), ("Esc", "Back")],
                _ => vec![("↑↓", "Scroll"), ("Esc", "Close")],
            };
        }
        if let Some(h) = self.template.hints(focus) {
            return h;
        }
        vec![("Enter", "Open")]
    }

    fn on_layer(&mut self, tag: &LayerTag, report: Report, cx: &mut Cx) -> Outcome {
        self.on_report(tag.kind, report, cx);
        Outcome::Changed
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

pub fn page() -> Box<dyn Page> {
    Box::new(DialogKindsPage::new())
}
