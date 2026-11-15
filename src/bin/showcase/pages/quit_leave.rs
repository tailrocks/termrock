use ratatui::buffer::Buffer;
use ratatui::crossterm::event::{KeyCode, KeyModifiers};
use ratatui::layout::{Rect, Size};

use crate::app::PageId;
use crate::options::page_options;
use crate::pages::layout::{sheet_height, sheet_width};
use crate::pages::template::{Section, Template};
use crate::pages::{Cx, CxExt, Hint, Page, PageEvent};
use junie_tui::core::event::{Key, Outcome};
use junie_tui::core::focus::forward_blur;
use junie_tui::core::id::WidgetId;
use junie_tui::core::pointer::MouseTarget;
use junie_tui::theme::Tone;
use junie_tui::ui::ctx::RenderCtx;
use junie_tui::ui::leave::{
    KeyContext, Leave, LeaveGuard, LeaveKey, QuitKeys, Resolved, Stakes, letters_allowed,
};
use junie_tui::ui::measure::{Tile, Tiled, tiles};
use junie_tui::ui::specimen::{SpecimenState, StateGrid};
use junie_tui::ui::text;
use junie_tui::widgets::button::Button;
use junie_tui::widgets::input::{InputAction, TextInput};
use junie_tui::widgets::panel::Panel;
use junie_tui::widgets::props::{self, Prop};

const ID: WidgetId = WidgetId::of("quitleave");

pub const TITLE: WidgetId = WidgetId::of("quitleave.title");

pub const LEAVE: WidgetId = WidgetId::of("quitleave.leave");
pub const QUIT: WidgetId = WidgetId::of("quitleave.quit");
pub const EXPORT: WidgetId = WidgetId::of("quitleave.export");

pub const GUARD: WidgetId = WidgetId::of("quitleave.guard");

const SAVED_TITLE: &str = "Q3 settlement report";

pub struct QuitLeavePage {
    template: Template,
    title: TextInput,

    saved: String,
    leave: Button,
    quit: Button,
    export: Button,

    running: bool,
    guard: LeaveGuard<&'static str>,

    opts: Vec<usize>,

    last_request: String,
    last_resolved: String,
}

impl QuitLeavePage {
    pub fn new() -> Self {
        let mut page = Self {
            template: Template::new(ID, states_grid()),
            title: TextInput::new(TITLE, "Title").value(SAVED_TITLE),
            saved: SAVED_TITLE.to_owned(),
            leave: Button::secondary(LEAVE, "Leave for Overview"),
            quit: Button::secondary(QUIT, "Quit"),
            export: Button::secondary(EXPORT, "Run export"),
            running: false,
            guard: LeaveGuard::new(GUARD),
            opts: page_options(PageId::QuitLeave)
                .iter()
                .map(|o| o.default)
                .collect(),
            last_request: "—".into(),
            last_resolved: "—".into(),
        };
        page.rebuild();
        page
    }

    fn opt(&self, key: &str) -> &'static str {
        let table = page_options(PageId::QuitLeave);
        let i = table
            .iter()
            .position(|o| o.key == key)
            .unwrap_or_else(|| panic!("no option {key}"));
        table[i].values[self.opts[i]]
    }

    fn rebuild(&mut self) {
        self.guard = LeaveGuard::new(GUARD).save(self.opt("quitleave.save") == "on");
    }

    fn quit_keys(&self) -> QuitKeys {
        let mut k = QuitKeys::DEFAULT
            .letter(match self.opt("quitleave.letter") {
                "x" => Some('x'),
                "none" => None,
                _ => Some('q'),
            })
            .esc_quits(self.opt("quitleave.esc_quits") == "on");
        k.ctrl_c = self.opt("quitleave.ctrl_c") == "on";
        k.ctrl_q = self.opt("quitleave.ctrl_q") == "on";
        k
    }

    fn unsaved(&self) -> Stakes {
        if self.title.text() == self.saved {
            Stakes::Clean
        } else {
            Stakes::Unsaved { changes: 1 }
        }
    }

    fn work(&self) -> Stakes {
        if self.running {
            Stakes::Running {
                what: "the export".into(),
            }
        } else {
            Stakes::Clean
        }
    }

    fn stakes(&self) -> Stakes {
        self.unsaved().max(self.work())
    }

    fn request(&mut self, to: Leave<&'static str>, cx: &mut Cx) {
        let stakes = self.stakes();
        let strings = junie_tui::theme::Strings::DEFAULT;
        match self.guard.request(to, &stakes, &strings) {
            Ok(to) => {
                self.last_request = format!("Ok({}): go now", leave_label(&to));
                cx.status(format!("Would {} now", go_label(&to)));
            }
            Err(d) => {
                self.last_request = format!("Err: {}", d.title);
                cx.open_dialog(*d);
            }
        }
    }

    fn resolve(&mut self, r: Resolved<&'static str>, cx: &mut Cx) {
        self.last_resolved = match &r {
            Resolved::Stay => "Resolved::Stay".to_owned(),
            Resolved::Save(to) => format!("Resolved::Save({})", leave_label(to)),
            Resolved::Discard(to) => format!("Resolved::Discard({})", leave_label(to)),
        };
        match r {
            Resolved::Stay => cx.status("Stayed"),
            Resolved::Save(to) => {
                self.saved = self.title.text().to_owned();
                cx.status(format!("Saved; would {} now", go_label(&to)));
            }
            Resolved::Discard(to) => {
                self.title.set_value(&self.saved);
                self.running = false;
                cx.status(format!("Discarded; would {} now", go_label(&to)));
            }
        }
    }

    fn facts(&self) -> Vec<Prop> {
        vec![
            Prop::new("Edits", stakes_label(&self.unsaved())).wrap(),
            Prop::new("Work", stakes_label(&self.work())).wrap(),
            Prop::new("Stakes::max", stakes_label(&self.stakes()))
                .tone(match self.stakes() {
                    Stakes::Clean => Tone::Secondary,
                    _ => Tone::Warning,
                })
                .wrap(),
            Prop::new(
                "LeaveGuard",
                if self.guard.save {
                    "save: Cancel Discard Save"
                } else {
                    "no save: Cancel Discard"
                },
            )
            .wrap(),
            Prop::new("Last request", self.last_request.clone()).wrap(),
            Prop::new("Last answer", self.last_resolved.clone()).wrap(),
        ]
    }

    fn notes(&self) -> String {
        let c = contexts();
        format!(
            "letters_allowed: clean {}, text focus {}, dirty {}. Quit goes through the \
             guard; Copy copies the selection instead. The leave buttons ask \
             Leave::To(Overview) and Leave::Quit; a dialog's answer resolves to \
             Resolved::Stay, Resolved::Save(to) or Resolved::Discard(to). The \
             showcase's own quit keys see this page's stakes: with edits or a running \
             export, Ctrl+Q asks first.",
            letters_allowed(c[0].1),
            letters_allowed(c[1].1),
            letters_allowed(c[2].1)
        )
    }

    fn tiled(&self, width: u16) -> Tiled {
        let doc = Panel::card(Some("Document")).meta("exporting");
        let facts = self.facts();

        let buttons = self.leave.width() + self.quit.width() + self.export.width() + 1 + 4;
        let doc_w = doc
            .outer(Size::new(buttons.max(FIELD_W), 0))
            .width
            .max(sheet_width(&doc, &facts));
        let fh = self.title.height();
        let doc_h = |w: u16| sheet_height(&doc, &facts, w) + fh + 3;
        let keys = Panel::card(Some("QuitKeys::classify")).meta("→ LeaveKey");
        let notes = self.notes();
        let keys_w = keys.outer(Size::new(LABEL_W + 4 * COL_W, 0)).width;
        let keys_h = |w: u16| {
            let inner = keys.inner(Size::new(w, 0)).width;
            let rows = 5 + 1 + text::wrap(&notes, inner as usize).len() as u16;
            keys.outer(Size::new(inner, rows)).height
        };
        tiles(
            width,
            &[&[Tile::new(doc_w, &doc_h), Tile::new(keys_w, &keys_h)]],
            (2, 1),
        )
    }

    fn render_playground(&mut self, area: Rect, buf: &mut Buffer, ctx: &mut RenderCtx) {
        let t = ctx.theme;
        let tiled = self.tiled(area.width);
        let (left, right) = (tiled.at(0, area), tiled.at(1, area));

        let meta = match self.stakes() {
            Stakes::Clean => "saved",
            Stakes::Unsaved { .. } => "edited",
            Stakes::Running { .. } => "exporting",
        };
        let panel = Panel::card(Some("Document")).meta(meta);
        let bg = panel.bg(t);
        let inner = panel.render(left, buf, t);
        let fh = self.title.height();
        self.title
            .render(Rect::new(inner.x, inner.y, inner.width, fh), buf, ctx, bg);
        let mut y = inner.y + fh + 1;
        self.export.label = if self.running {
            "Stop export".into()
        } else {
            "Run export".into()
        };
        let mut x = inner.x;
        for b in [&mut self.leave, &mut self.quit, &mut self.export] {
            let w = b.width();
            if x + w <= inner.right() {
                b.render(Rect::new(x, y, w, 1), buf, ctx, bg);
            }
            x += w + 2;
        }
        y += 2;
        let facts = self.facts();
        if y < inner.bottom() {
            props::render(
                Rect::new(inner.x, y, inner.width, inner.bottom() - y),
                buf,
                t,
                &facts,
                bg,
            );
        }

        let keys = self.quit_keys();
        let letter = keys.letter.map_or("letter".to_owned(), |c| c.to_string());
        let cases: [(String, Key); 4] = [
            (
                letter.clone(),
                plain(KeyCode::Char(keys.letter.unwrap_or('q'))),
            ),
            ("Ctrl+C".into(), ctrl('c')),
            ("Ctrl+Q".into(), ctrl('q')),
            ("Esc".into(), plain(KeyCode::Esc)),
        ];
        let contexts = contexts();

        let panel = Panel::card(Some("QuitKeys::classify")).meta("→ LeaveKey");
        let bg = panel.bg(t);
        let inner = panel.render(right, buf, t);
        let (label_w, col) = (LABEL_W, COL_W);
        let mut yy = inner.y;
        let mut x = inner.x + label_w;
        for (label, _) in &cases {
            if x < inner.right() {
                buf.set_string(x, yy, label, t.key_hint_key().bg(bg));
            }
            x += col;
        }
        yy += 1;
        for (name, cx) in &contexts {
            if yy >= inner.bottom() {
                break;
            }
            buf.set_string(inner.x, yy, *name, t.muted().bg(bg));
            let mut x = inner.x + label_w;
            for (_, key) in &cases {
                let (s, st) = match keys.classify(key, *cx) {
                    Some(LeaveKey::Quit) => ("Quit", t.secondary().bg(bg)),
                    Some(LeaveKey::Copy) => ("Copy", t.secondary().bg(bg)),
                    None => ("—", t.faint().bg(bg)),
                };
                if x < inner.right() {
                    buf.set_string(x, yy, s, st);
                }
                x += col;
            }
            yy += 1;
        }
        yy += 1;
        let notes = self.notes();
        for l in text::wrap(&notes, inner.width as usize) {
            buf.set_string(inner.x, yy, l, t.faint().bg(bg));
            yy += 1;
        }
    }
}

const FIELD_W: u16 = 40;

const LABEL_W: u16 = 12;
const COL_W: u16 = 8;

fn contexts() -> [(&'static str, KeyContext); 4] {
    [
        ("clean", KeyContext::default()),
        (
            "text focus",
            KeyContext {
                text_focus: true,
                ..KeyContext::default()
            },
        ),
        (
            "dirty",
            KeyContext {
                dirty: true,
                ..KeyContext::default()
            },
        ),
        (
            "selection",
            KeyContext {
                selection: true,
                ..KeyContext::default()
            },
        ),
    ]
}

fn plain(code: KeyCode) -> Key {
    Key {
        code,
        mods: KeyModifiers::NONE,
    }
}

fn ctrl(c: char) -> Key {
    Key {
        code: KeyCode::Char(c),
        mods: KeyModifiers::CONTROL,
    }
}

fn leave_label(to: &Leave<&'static str>) -> String {
    match to {
        Leave::Quit => "Leave::Quit".to_owned(),
        Leave::To(r) => format!("Leave::To({r})"),
    }
}

fn go_label(to: &Leave<&'static str>) -> String {
    match to {
        Leave::Quit => "quit".to_owned(),
        Leave::To(r) => format!("go to {r}"),
    }
}

fn stakes_label(s: &Stakes) -> String {
    match s {
        Stakes::Clean => "Clean".to_owned(),
        Stakes::Unsaved { changes } => format!("Unsaved · {changes} change(s)"),
        Stakes::Running { what } => format!("Running · {what}"),
    }
}

fn states_grid() -> StateGrid {
    use SpecimenState::*;
    let id = ID.sub("states");
    StateGrid::new(id)
        .rows(&[Rest, Hover, Pressed, Focused, Disabled])
        .column_width(14)
        .column("Cancel", Button::subtle(id.child(0), "Cancel"))
        .column("Discard", Button::danger(id.child(1), "Discard"))
        .column("Save", Button::primary(id.child(2), "Save"))
        .column("Stay", Button::secondary(id.child(3), "Stay"))
}

impl Page for QuitLeavePage {
    fn title(&self) -> &'static str {
        "Quit & leave"
    }

    fn blurb(&self) -> &'static str {
        "QuitKeys, the leave guard with Clean, Unsaved and Running stakes, and its dialogs"
    }

    fn editing(&self) -> bool {
        self.title.editing
    }

    fn leave_stakes(&self) -> Stakes {
        self.stakes()
    }

    fn measure(&self, avail: Size, _ctx: &RenderCtx) -> Size {
        self.tiled(avail.width).size
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
        if let PageEvent::Blur(ch) = ev {
            return forward_blur(*ch, &mut [&mut self.title]);
        }
        if let PageEvent::DialogClosed { id, result, .. } = ev
            && *id == self.guard.id
        {
            let r = self.guard.resolve(*result);
            self.resolve(r, cx);
            return Outcome::Changed;
        }
        if let Some(o) = self.template.handle(ev, cx) {
            return o;
        }
        let act = match ev {
            PageEvent::Key(key) => {
                let Some(f) = cx.focus.current() else {
                    return Outcome::Ignored;
                };
                if f == TITLE {
                    let (o, iev) = self.title.on_key(key);
                    match iev {
                        Some(InputAction::CommittedTab { backward: false }) => cx.focus_next(),
                        Some(InputAction::CommittedTab { backward: true }) => cx.focus_prev(),
                        _ => {}
                    }
                    return o;
                }
                let b = match f {
                    LEAVE => &mut self.leave,
                    QUIT => &mut self.quit,
                    EXPORT => &mut self.export,
                    _ => return Outcome::Ignored,
                };
                let (o, act) = b.on_key(key);
                if !act {
                    return o;
                }
                f
            }
            PageEvent::Paste(text) if cx.focus.is(TITLE) => return self.title.on_paste(text),
            PageEvent::Pointer(ev) => {
                if let Some(r) = self.title.route(ev) {
                    let o = r.outcome();
                    if o != Outcome::Ignored {
                        cx.focus.focus(TITLE);
                    }
                    return o;
                }
                let (id, b) = match ev.owner() {
                    LEAVE => (LEAVE, &mut self.leave),
                    QUIT => (QUIT, &mut self.quit),
                    EXPORT => (EXPORT, &mut self.export),
                    _ => return Outcome::Ignored,
                };
                match b.route(ev) {
                    Some(r) if r.outcome() != Outcome::Ignored => {
                        if r.into_action().is_none() {
                            return Outcome::Changed;
                        }
                        id
                    }
                    _ => return Outcome::Ignored,
                }
            }
            _ => return Outcome::Ignored,
        };
        match act {
            LEAVE => self.request(Leave::To("Overview"), cx),
            QUIT => self.request(Leave::Quit, cx),
            _ => {
                self.running = !self.running;
                cx.status(if self.running {
                    "Export running"
                } else {
                    "Export stopped"
                });
            }
        }
        Outcome::Changed
    }

    fn hints(&self, focus: Option<WidgetId>) -> Vec<Hint> {
        if let Some(h) = self.template.hints(focus) {
            return h;
        }
        match focus {
            Some(TITLE) => vec![("Type", "Edit the title")],
            Some(LEAVE | QUIT | EXPORT) => vec![("Enter", "Press")],
            _ => Vec::new(),
        }
    }

    fn option_value(&self, key: &str) -> Option<usize> {
        let i = page_options(PageId::QuitLeave)
            .iter()
            .position(|o| o.key == key)?;
        Some(self.opts[i])
    }

    fn set_option(&mut self, key: &str, value: usize) {
        let table = page_options(PageId::QuitLeave);
        if let Some(i) = table.iter().position(|o| o.key == key) {
            self.opts[i] = value.min(table[i].values.len() - 1);
            if key == "quitleave.stakes" {
                let v = table[i].values[self.opts[i]];
                self.title.set_value(if v == "unsaved" {
                    "Q3 settlement report (draft)"
                } else {
                    SAVED_TITLE
                });
                self.saved = SAVED_TITLE.to_owned();
                self.running = v == "running";
            }
            self.rebuild();
        }
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
    Box::new(QuitLeavePage::new())
}
