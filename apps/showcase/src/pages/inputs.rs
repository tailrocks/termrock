use ratatui::buffer::Buffer;
use ratatui::layout::{Rect, Size};

use crate::app::PageId;
use crate::options::{PageValues, page_options};
use crate::pages::template::{Section, Template};
use crate::pages::{Cx, Hint, Page, PageEvent};
use termrock::core::edit::{EditEntry, FieldKeys, FocusSelect, TextCmd};
use termrock::core::event::Outcome;
use termrock::core::focus::BlurAction;
use termrock::core::id::WidgetId;
use termrock::core::intent::Phase;
use termrock::core::keymap::{KeyBinding, KeyMap};
use termrock::core::pointer::MouseTarget;
use termrock::ui::ctx::RenderCtx;
use termrock::ui::measure::{Measure, Tile, Tiled, tiles};
use termrock::ui::specimen::{SpecimenState, StateGrid};
use termrock::ui::text;
use termrock::widgets::field_common::PointerSelect;
use termrock::widgets::input::{InputAction, TextInput};
use termrock::widgets::panel::Panel;
use termrock::widgets::props::{self, Prop};

const ID: WidgetId = WidgetId::of("inputs");

fn email(s: &str) -> Option<String> {
    if !s.contains('@') || !s.contains('.') {
        Some("Enter a valid email address".into())
    } else {
        None
    }
}

const OWNER_ID: WidgetId = ID.child(2);
const OWNER_MAX_CHARS: usize = 24;

fn owner_length(s: &str) -> Option<String> {
    if s.chars().count() > OWNER_MAX_CHARS {
        Some(format!("Use at most {OWNER_MAX_CHARS} characters"))
    } else {
        None
    }
}

pub const TEXT_COMMANDS: [(TextCmd, &str); 14] = [
    (TextCmd::SelectAll, "Select all"),
    (TextCmd::Copy, "Copy"),
    (TextCmd::Cut, "Cut"),
    (TextCmd::Paste, "Paste"),
    (TextCmd::Undo, "Undo"),
    (TextCmd::Redo, "Redo"),
    (TextCmd::LineStart, "Line start"),
    (TextCmd::LineEnd, "Line end"),
    (TextCmd::DocStart, "Text start"),
    (TextCmd::DocEnd, "Text end"),
    (TextCmd::SelectToLineStart, "Select to line start"),
    (TextCmd::SelectToLineEnd, "Select to line end"),
    (TextCmd::SelectToDocStart, "Select to text start"),
    (TextCmd::SelectToDocEnd, "Select to text end"),
];

pub fn custom_text_keymap(keys: FieldKeys) -> KeyMap<TextCmd> {
    let mut map = vec![(KeyBinding::alt('a'), TextCmd::SelectAll)];
    map.extend(keys.text_keymap());
    map
}

fn keys_of(map: &KeyMap<TextCmd>, cmd: TextCmd) -> String {
    let keys: Vec<String> = map
        .iter()
        .filter(|(b, c)| {
            *c == cmd
                && !b
                    .mods
                    .contains(ratatui::crossterm::event::KeyModifiers::SUPER)
        })
        .map(|(b, _)| b.label())
        .fold(Vec::new(), |mut v, l| {
            if !v.contains(&l) {
                v.push(l);
            }
            v
        });
    if keys.is_empty() {
        "Cmd chord".into()
    } else {
        keys.join(" · ")
    }
}

fn edit_entry(opts: &PageValues, key: &str) -> Option<EditEntry> {
    match opts.value(key) {
        "immediate" => Some(EditEntry::Immediate),
        "type-to-edit" => Some(EditEntry::TypeToEdit),
        "explicit" => Some(EditEntry::Explicit),
        _ => None,
    }
}

pub fn focus_select(opts: &PageValues, key: &str) -> FocusSelect {
    match opts.value(key) {
        "caret-end" => FocusSelect::CaretEnd,
        "caret-start" => FocusSelect::CaretStart,
        _ => FocusSelect::SelectAll,
    }
}

pub fn field_keys(opts: &PageValues, key: &str) -> Option<FieldKeys> {
    match opts.value(key) {
        "emacs" => Some(FieldKeys::Emacs),
        "desktop" => Some(FieldKeys::Desktop),
        _ => None,
    }
}

pub fn blur_action(opts: &PageValues, key: &str) -> BlurAction {
    match opts.value(key) {
        "cancel" => BlurAction::Cancel,
        "keep" => BlurAction::Keep,
        _ => BlurAction::Commit,
    }
}

pub fn pointer_select(opts: &PageValues, key: &str) -> PointerSelect {
    let d = PointerSelect::DEFAULT;
    match opts.value(key) {
        "no-drag" => d.drag(false),
        "no-word" => d.word(false),
        "no-line" => d.line(false),
        "by-word" => d.drag_by_word(true),
        "off" => PointerSelect::OFF,
        _ => d,
    }
}

pub struct InputsPage {
    template: Template,
    opts: PageValues,
    fields: Vec<TextInput>,
    sections: crate::pages::layout::Sections,
}

impl InputsPage {
    pub fn new() -> Self {
        let opts = PageValues::new(page_options(PageId::Inputs));
        let mut page = Self {
            template: Template::new(ID, states_grid()),
            opts,
            fields: Vec::new(),
            sections: crate::pages::layout::Sections::new(ID.sub("page")),
        };
        page.rebuild();
        page
    }

    fn rebuild(&mut self) {
        let o = &self.opts;
        let mut owner = TextInput::new(OWNER_ID, "Owner email")
            .value("mira@example")
            .required(true);
        owner = if o.value("inputs.owner_rule") == "max-length" {
            owner.validator(owner_length)
        } else {
            owner.validator(email)
        };
        owner.validate();
        let mut project = TextInput::new(ID.child(0), "Project name")
            .value("payments-gateway")
            .required(true)
            .help("Used as the working directory name");
        if o.value("inputs.required_message") == "custom" {
            project = project.required_message("Name the project to continue");
        }
        let mut fields = vec![
            project,
            TextInput::new(ID.child(1), "Branch to check out once the task is done")
                .placeholder("feat/…")
                .help("Leave empty to work on a detached checkout")
                .wrap_label(o.on("inputs.wrap_label")),
            owner,
            TextInput::new(ID.child(3), "API token")
                .value("jb_live_••••••••••••")
                .disabled(true)
                .help("Managed by the organization"),
            TextInput::new(ID.child(4), "Search files")
                .placeholder("Type a path or symbol…")
                .readout(if o.on("inputs.readout") {
                    "214 files"
                } else {
                    ""
                })
                .edit_entry(EditEntry::TypeToEdit)
                .help("Selection: Shift+←→  ·  words: Ctrl+←→  ·  clear: Ctrl+U"),
            TextInput::new(ID.child(5), "API key")
                .masked()
                .reveal_tail(4)
                .value("sk-live-0f3a91c2e7d4b6a8c1f2")
                .help("Masked while typing; the last four characters show once committed"),
        ];
        let entry = edit_entry(o, "inputs.edit_entry");
        let keys = field_keys(o, "inputs.field_keys");
        for (i, f) in fields.iter_mut().enumerate() {
            let mut g = f
                .clone()
                .focus_select(focus_select(o, "inputs.focus_select"))
                .blur_action(blur_action(o, "inputs.blur_action"))
                .pointer_select(pointer_select(o, "inputs.pointer_select"))
                .label_hit(o.on("inputs.label_hit"));
            if let Some(e) = entry {
                g = g.edit_entry(e);
            }
            if let Some(k) = keys {
                g = g.field_keys(k);
            }
            if o.value("inputs.text_keymap") == "custom" {
                g = g.text_keymap(custom_text_keymap(keys.unwrap_or_default()));
            }
            if o.on("inputs.plain_label") {
                g = g.plain_label();
            }

            if let Some(old) = self.fields.get(i)
                && !old.disabled
            {
                g.set_value(old.field.buffer.text());
                g.field.error = old.field.error.clone();
            }
            if g.id == OWNER_ID {
                g.validate();
            }
            *f = g;
        }
        self.fields = fields;
    }

    #[allow(dead_code)]
    pub fn field(&self, i: usize) -> &TextInput {
        &self.fields[i]
    }

    fn focused_index(&self, cx: &Cx) -> Option<usize> {
        let f = cx.focus.current()?;
        self.fields.iter().position(|i| i.id == f)
    }

    fn field_tiles(&self, width: u16, ctx: &RenderCtx) -> Tiled {
        let mins: Vec<u16> = self
            .fields
            .iter()
            .map(|f| f.measure(Size::new(0, 0), ctx).width)
            .collect();
        let heights: Vec<Box<dyn Fn(u16) -> u16 + '_>> = self
            .fields
            .iter()
            .map(|f| Box::new(move |w: u16| f.height_for(w)) as Box<dyn Fn(u16) -> u16>)
            .collect();
        let tiles_v: Vec<Tile> = heights
            .iter()
            .zip(&mins)
            .map(|(h, &m)| Tile::new(m, h.as_ref()))
            .collect();
        let rows: Vec<&[Tile]> = tiles_v.chunks(2).collect();
        tiles(width, &rows, (4, 0))
    }

    fn command_sheets(&self) -> [(&'static str, bool, Vec<Prop>); 2] {
        let custom = self.opts.value("inputs.text_keymap") == "custom";
        [
            (FieldKeys::Desktop, "Desktop keys"),
            (FieldKeys::Emacs, "Emacs keys"),
        ]
        .map(|(keys, name)| {
            let map = if custom {
                custom_text_keymap(keys)
            } else {
                keys.text_keymap()
            };
            let chosen = field_keys(&self.opts, "inputs.field_keys") == Some(keys);
            let list = TEXT_COMMANDS
                .iter()
                .map(|(cmd, label)| Prop::new(label, keys_of(&map, *cmd)).wrap())
                .collect();
            (name, chosen, list)
        })
    }

    fn command_tiles(&self, width: u16) -> Tiled {
        let sheets = self.command_sheets();
        let (a, b) = (&sheets[0].2, &sheets[1].2);
        let ha = |w: u16| 1 + props::measure(a, w);
        let hb = |w: u16| 1 + props::measure(b, w);
        tiles(
            width,
            &[&[
                Tile::new(props::width(a), &ha),
                Tile::new(props::width(b), &hb),
            ]],
            (4, 1),
        )
    }

    fn commands_panel(&self) -> Panel<'static> {
        let meta = if self.opts.value("inputs.text_keymap") == "custom" {
            "custom key map: Alt+A selects all"
        } else {
            "the preset's key map"
        };
        Panel::card(Some("Text commands")).meta(meta)
    }

    fn heights(&self, width: u16, ctx: &RenderCtx) -> [u16; 3] {
        let fp = Panel::card(Some("Playground")).meta(FIELDS_META);
        let inner = fp.inner(Size::new(width, 0)).width;
        let fields = fp
            .outer(Size::new(inner, self.field_tiles(inner, ctx).size.height))
            .height;
        let panel = self.commands_panel();
        let inner = panel.inner(Size::new(width, 0)).width;
        let commands = panel
            .outer(Size::new(inner, self.command_tiles(inner).size.height))
            .height;
        let undo = Panel::card(Some("Undo history"));
        let inner = undo.inner(Size::new(width, 0)).width;
        let rows = text::wrap(UNDO_NOTE, inner.max(1) as usize).len() as u16;
        [fields, commands, undo.outer(Size::new(inner, rows)).height]
    }

    fn natural_width(&self, ctx: &RenderCtx) -> u16 {
        let field = self
            .fields
            .iter()
            .map(|f| f.measure(Size::new(0, 0), ctx).width)
            .max()
            .unwrap_or(0);
        let fields_w = field.max(FIELDS_W);
        let sheets = self.command_sheets();
        let sheet = props::width(&sheets[0].2).max(props::width(&sheets[1].2));
        let card = |p: Panel, w: u16| p.outer(Size::new(w, 0)).width;
        card(Panel::card(Some("Playground")).meta(FIELDS_META), fields_w)
            .max(card(self.commands_panel(), sheet))
            .max(card(
                Panel::card(Some("Undo history")).meta(UNDO_META),
                UNDO_W,
            ))
    }

    fn render_playground(&mut self, area: Rect, buf: &mut Buffer, ctx: &mut RenderCtx) {
        let [fields, commands, undo] = self.heights(area.width, ctx);
        let r = Rect::new(area.x, area.y, area.width, fields);
        self.render_fields(r, buf, ctx);
        let r = Rect::new(area.x, r.bottom() + 1, area.width, commands);
        self.render_commands(r, buf, ctx);
        let r = Rect::new(area.x, r.bottom() + 1, area.width, undo);
        render_undo(r, buf, ctx);
    }

    fn render_fields(&mut self, area: Rect, buf: &mut Buffer, ctx: &mut RenderCtx) {
        let t = ctx.theme;
        let panel = Panel::card(Some("Playground")).meta(FIELDS_META);
        let bg = panel.bg(t);
        let inner = panel.render(area, buf, t);
        let tiled = self.field_tiles(inner.width, ctx);
        for (i, f) in self.fields.iter_mut().enumerate() {
            f.render(tiled.at(i, inner), buf, ctx, bg);
        }
    }

    fn render_commands(&self, area: Rect, buf: &mut Buffer, ctx: &mut RenderCtx) {
        let t = ctx.theme;
        let panel = self.commands_panel();
        let bg = panel.bg(t);
        let inner = panel.render(area, buf, t);
        let tiled = self.command_tiles(inner.width);
        for (i, (name, chosen, list)) in self.command_sheets().iter().enumerate() {
            let col = tiled.at(i, inner);
            let style = if *chosen {
                t.muted().fg(t.chosen(false))
            } else {
                t.muted()
            };
            buf.set_string(col.x, col.y, name, style.bg(bg));
            let body = Rect::new(col.x, col.y + 1, col.width, col.height.saturating_sub(1));
            props::render(body, buf, t, list, bg);
        }
    }
}

const FIELDS_W: u16 = 48;
const FIELDS_META: &str = "Type to edit · Enter Apply · Esc Cancel · Tab Next";

const UNDO_W: u16 = 30;
const UNDO_META: &str = "Ctrl+Z · Ctrl+Shift+Z";
const UNDO_NOTE: &str = "A run of typed characters undoes as one step (Typing), and so does \
     a run of deletions (Deleting); a space, a paste or a cut (Other) starts a new step. \
     Type two words in Project name, then Ctrl+Z.";

fn render_undo(area: Rect, buf: &mut Buffer, ctx: &mut RenderCtx) {
    let t = ctx.theme;
    let panel = Panel::card(Some("Undo history")).meta(UNDO_META);
    let bg = panel.bg(t);
    let inner = panel.render(area, buf, t);
    for (i, line) in text::wrap(UNDO_NOTE, inner.width.max(1) as usize)
        .iter()
        .enumerate()
    {
        let y = inner.y + i as u16;
        if y < inner.bottom() {
            buf.set_string(inner.x, y, line, t.secondary().bg(bg));
        }
    }
}

fn states_grid() -> StateGrid {
    use SpecimenState::*;
    let id = ID.sub("states");
    StateGrid::new(id)
        .column_width(34)
        .rows(&[
            Rest, Hover, Focused, Editing, Error, Disabled, Empty, Overflow, Narrow,
        ])
        .column(
            "Text",
            TextInput::new(id.child(0), "Project name")
                .value("payments-gateway")
                .placeholder("(feat/…)")
                .plain_label(),
        )
        .column(
            "Masked",
            TextInput::new(id.child(1), "API key")
                .masked()
                .reveal_tail(4)
                .value("sk-live-0f3a91c2e7d4b6a8c1f2")
                .plain_label(),
        )
}

impl Page for InputsPage {
    fn title(&self) -> &'static str {
        "Inputs"
    }
    fn blurb(&self) -> &'static str {
        "Focus is a bar; editing is a cursor. Enter to edit, Esc to revert."
    }

    fn measure(&self, avail: Size, ctx: &RenderCtx) -> Size {
        let w = avail.width.max(self.natural_width(ctx));
        let h = self.heights(w, ctx).iter().sum::<u16>() + 2;
        Size::new(w, h)
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
        if let Some(o) = self.template.handle(ev, cx) {
            return o;
        }
        if let PageEvent::Blur(ch) = ev {
            let Some(i) = self.fields.iter().position(|f| f.id == ch.from) else {
                return Outcome::Ignored;
            };
            return match self.fields[i].on_blur() {
                Some(InputAction::Committed) => {
                    cx.status(format!("{} saved", self.fields[i].label));
                    Outcome::Changed
                }
                Some(InputAction::Cancelled) => {
                    cx.status("Reverted");
                    Outcome::Changed
                }
                _ => Outcome::Ignored,
            };
        }
        if self.template.section() == Section::Playground
            && let Some(o) = self.sections.handle(ev)
        {
            return o;
        }
        match ev {
            PageEvent::Key(key) => {
                let Some(i) = self.focused_index(cx) else {
                    return Outcome::Ignored;
                };
                let (out, iev) = self.fields[i].on_key(key);
                match iev {
                    Some(InputAction::CommittedTab { backward: false }) => cx.focus_next(),
                    Some(InputAction::CommittedTab { backward: true }) => cx.focus_prev(),
                    Some(InputAction::Committed) => {
                        cx.status(format!("{} saved", self.fields[i].label))
                    }
                    Some(InputAction::Cancelled) => cx.status("Reverted"),
                    _ => {}
                }
                out
            }
            PageEvent::Paste(text) => {
                let Some(i) = self.focused_index(cx) else {
                    return Outcome::Ignored;
                };
                self.fields[i].on_paste(text)
            }
            PageEvent::Pointer(ev) => {
                for f in &mut self.fields {
                    if let Some(r) = f.route(ev) {
                        if ev.is(Phase::Click) {
                            cx.focus.focus(f.id);
                        }
                        return r.outcome();
                    }
                }
                Outcome::Ignored
            }
            _ => Outcome::Ignored,
        }
    }

    fn editing(&self) -> bool {
        self.fields.iter().any(|f| f.editing)
    }

    fn hints(&self, focus: Option<WidgetId>) -> Vec<Hint> {
        if let Some(h) = self.template.hints(focus) {
            return h;
        }
        let editing = focus
            .and_then(|f| self.fields.iter().find(|i| i.id == f))
            .map(|i| i.editing)
            .unwrap_or(false);
        if editing {
            vec![
                ("Enter", "Apply"),
                ("Esc", "Cancel"),
                ("Shift+←→", "Select"),
                ("Ctrl+U", "Clear"),
            ]
        } else {
            vec![termrock::widgets::widget_hints::EDIT_PAIR]
        }
    }

    fn option_value(&self, key: &str) -> Option<usize> {
        self.opts.get(key)
    }

    fn set_option(&mut self, key: &str, value: usize) {
        if self.opts.set(key, value) {
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
