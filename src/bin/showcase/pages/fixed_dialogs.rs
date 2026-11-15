use ratatui::buffer::Buffer;
use ratatui::layout::Rect;

use crate::app::PageId;
use crate::options::{PageValues, page_options};
use crate::pages::template::{Section, Template};
use crate::pages::{Cx, Hint, Page, PageEvent, Report};
use junie_tui::core::event::Outcome;
use junie_tui::core::id::WidgetId;
use junie_tui::core::intent::Phase;
use junie_tui::core::pointer::MouseTarget;
use junie_tui::core::refusal::Reason;
use junie_tui::ui::ctx::RenderCtx;
use junie_tui::ui::layer::LayerTag;
use junie_tui::ui::layers::LayerResult;
use junie_tui::ui::specimen::StateGrid;
use junie_tui::widgets::button::{Button, row_layout};
use junie_tui::widgets::choice::{Checkbox, RadioGroup};
use junie_tui::widgets::choice_dialog::ChoiceDialog;
use junie_tui::widgets::file_browser::{FileBrowser, FileEntry, MemorySource};
use junie_tui::widgets::form_dialog::{FormAction, FormDialog, FormField};
use junie_tui::widgets::input::TextInput;
use junie_tui::widgets::panel::Panel;

const ID: WidgetId = WidgetId::of("fixeddialogs");
const CHOICE: WidgetId = ID.sub("choice");
const FORM: WidgetId = ID.sub("form");
const BROWSER: WidgetId = ID.sub("browser");

const OPENERS: [&str; 3] = ["Choice dialog…", "Form dialog…", "File browser…"];

const PLAIN: &str = "Not available · managed by policy";
const KEEP_AWAKE: &str = "Always off · the host decides";
const TYPED_KEY: &str = "Not available · managed by policy";
const READ_ONLY: &str = "Always read-only · the host decides";

pub fn page() -> Box<dyn Page> {
    Box::new(FixedDialogsPage::new())
}

pub struct FixedDialogsPage {
    template: Template,
    opts: PageValues,
    openers: Vec<Button>,
    last: Option<String>,
    changes: usize,
}

impl FixedDialogsPage {
    fn new() -> Self {
        Self {
            template: Template::new(ID, StateGrid::new(ID.sub("states"))),
            opts: PageValues::new(page_options(PageId::FixedDialogs)),
            openers: OPENERS
                .iter()
                .enumerate()
                .map(|(i, label)| Button::secondary(ID.child(i), label))
                .collect(),
            last: None,
            changes: 0,
        }
    }

    fn fixed(&self) -> bool {
        self.opts.value("fixeddialogs.state") == "fixed"
    }

    fn choice(&self) -> ChoiceDialog {
        let mut dialog = ChoiceDialog::new(
            CHOICE,
            "Destination",
            "Where the output goes",
            &["Project folder", "Edit destination…"],
            0,
        );
        dialog.radio.disabled = self.fixed();
        dialog
    }

    fn form(&self) -> FormDialog {
        let plain = Button::subtle(FORM.sub("plain"), "Plain text");
        let plain = if self.fixed() {
            plain.fixed(Reason::new(PLAIN).short("managed"))
        } else {
            plain
        };
        let keep_awake = Checkbox::new(FORM.sub("keep"), "Keep awake", false);
        let keep_awake = if self.fixed() {
            keep_awake.fixed(Reason::new(KEEP_AWAKE).short("managed"))
        } else {
            keep_awake
        };
        let source = RadioGroup::new(
            FORM.sub("source"),
            "Credential source",
            &["Local folder", "Password manager", "Typed key"],
            0,
        );
        let source = if self.fixed() {
            source.fixed_option(2, Reason::new(TYPED_KEY).short("managed"))
        } else {
            source
        };
        FormDialog::new(
            FORM,
            "Account",
            vec![
                FormField::input("name", TextInput::new(FORM.sub("name"), "Name")),
                FormField::check("keep", keep_awake),
                FormField::radio("source", source),
            ],
        )
        .action("plain", plain)
        .action("check", Button::subtle(FORM.sub("check"), "Check"))
    }

    fn browser(&self) -> FileBrowser {
        let mut browser = FileBrowser::new(
            BROWSER,
            "Choose a folder",
            MemorySource::new(vec![
                FileEntry::dir("/work/app"),
                FileEntry::dir("/work/docs"),
                FileEntry::dir("/work/tools"),
            ])
            .home("/work"),
            "/work",
        )
        .checkbox("Read-only", false);
        if self.fixed() {
            browser.checkbox = browser
                .checkbox
                .take()
                .map(|c| c.fixed(Reason::new(READ_ONLY).short("managed")));
        }
        browser
    }

    fn open(&mut self, i: usize, cx: &mut Cx) {
        match i {
            0 => cx.open(self.choice(), LayerTag::new("choice")),
            1 => cx.open(self.form(), LayerTag::new("form")),
            _ => cx.open(self.browser(), LayerTag::new("browser")),
        }
    }
}

impl Page for FixedDialogsPage {
    fn title(&self) -> &'static str {
        "Fixed dialogs"
    }

    fn blurb(&self) -> &'static str {
        "A dialog keeps a fixed choice visible: a disabled radio is skipped, a fixed action is not offered, a fixed box refuses"
    }

    fn render(&mut self, area: Rect, buf: &mut Buffer, ctx: &mut RenderCtx) {
        let body = self.template.render(area, buf, ctx);
        match self.template.section() {
            Section::Playground => {
                let t = ctx.theme;
                let panel =
                    Panel::card(Some("Dialogs")).meta(if self.fixed() { "fixed" } else { "live" });
                let bg = panel.bg(t);
                let inner = panel.render(Rect::new(body.x, body.y, body.width.min(60), 6), buf, t);
                let widths: Vec<u16> = self.openers.iter().map(Button::width).collect();
                let rects = row_layout(Rect::new(inner.x, inner.y, inner.width, 1), &widths, 2);
                for (button, rect) in self.openers.iter_mut().zip(rects) {
                    button.render(rect, buf, ctx, bg);
                }
                if let Some(last) = &self.last {
                    let line = format!("{last} · {} form changes", self.changes);
                    buf.set_string(inner.x, inner.y + 2, line, t.secondary().bg(bg));
                }
            }
            Section::Options => self.template.render_options(body, buf, ctx),
            Section::States => self.template.render_states(body, buf, ctx),
        }
    }

    fn handle(&mut self, ev: &PageEvent, cx: &mut Cx) -> Outcome {
        if let Some(o) = self.template.handle(ev, cx) {
            return o;
        }
        if self.template.section() != Section::Playground {
            return Outcome::Ignored;
        }
        match ev {
            PageEvent::Key(key) => {
                for i in 0..self.openers.len() {
                    if cx.focus.is(self.openers[i].id) {
                        let (o, fired) = self.openers[i].on_key(key);
                        if fired {
                            self.open(i, cx);
                        }
                        return o;
                    }
                }
                Outcome::Ignored
            }
            PageEvent::Pointer(ev) => {
                for i in 0..self.openers.len() {
                    if let Some(r) = self.openers[i].route(ev) {
                        if ev.is(Phase::Click) && r.into_action().is_some() {
                            self.open(i, cx);
                        }
                        return Outcome::Changed;
                    }
                }
                Outcome::Ignored
            }
            _ => Outcome::Ignored,
        }
    }

    fn hints(&self, focus: Option<WidgetId>) -> Vec<Hint> {
        if let Some(h) = self.template.hints(focus) {
            return h;
        }
        vec![("Enter", "Open")]
    }

    fn on_layer(&mut self, tag: &LayerTag, report: Report, _cx: &mut Cx) -> Outcome {
        if let Some(LayerResult::Form(FormAction::Changed(_))) = &report.result {
            self.changes += 1;
        }
        if report.closed.is_some() {
            self.last = Some(format!("{} closed", tag.kind));
        }
        Outcome::Changed
    }

    fn option_value(&self, key: &str) -> Option<usize> {
        self.opts.get(key)
    }

    fn set_option(&mut self, key: &str, value: usize) {
        self.opts.set(key, value);
    }

    fn template(&self) -> Option<&Template> {
        Some(&self.template)
    }

    fn template_mut(&mut self) -> Option<&mut Template> {
        Some(&mut self.template)
    }
}
