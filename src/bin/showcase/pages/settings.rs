use ratatui::buffer::Buffer;
use ratatui::layout::{Constraint, Rect, Size};

use crate::app::PageId;
use crate::options::{PageValues, page_options};
use crate::pages::template::{Section, Template};
use crate::pages::{Cx, CxExt, Hint, Page, PageEvent};
use junie_tui::core::event::Outcome;
use junie_tui::core::focus::forward_blur;
use junie_tui::core::id::{Part, WidgetId};
use junie_tui::core::intent::Phase;
use junie_tui::core::pointer::{MouseTarget, PointerEvent};
use junie_tui::ui::ctx::RenderCtx;
use junie_tui::ui::measure::Measure;
use junie_tui::ui::specimen::{SpecimenState, StateGrid};
use junie_tui::widgets::button::{Button, row_layout};
use junie_tui::widgets::choice::{RadioGroup, Toggle};
use junie_tui::widgets::dialog::{Dialog, DialogResult};
use junie_tui::widgets::input::{InputAction, TextInput};
use junie_tui::widgets::list::{ListBox, ListItem, SelectMode};
use junie_tui::widgets::panel::Panel;
use junie_tui::widgets::scrollbar;
use junie_tui::widgets::table::{Cell, Column, DataTable, TableAction, Tone};
use junie_tui::widgets::tabs::Tabs;
use junie_tui::widgets::textarea::TextArea;

const ID: WidgetId = WidgetId::of("settings");
const REMOVE_DLG: WidgetId = ID.sub("remove-dialog");
const ADD_DLG: WidgetId = ID.sub("add-dialog");
const SAVE_DLG: WidgetId = ID.sub("save-dialog");

fn role_validator(col: usize, s: &str) -> Option<String> {
    match col {
        2 if !matches!(s, "Owner" | "Admin" | "Member" | "Viewer") => {
            Some("Role: Owner, Admin, Member or Viewer".into())
        }
        0 if s.trim().is_empty() => Some("Name required".into()),
        _ => None,
    }
}

fn var_validator(s: &str) -> Option<String> {
    if !s
        .chars()
        .all(|c| c.is_ascii_uppercase() || c == '_' || c.is_ascii_digit())
    {
        Some("Use UPPER_SNAKE_CASE".into())
    } else {
        None
    }
}

pub struct SettingsPage {
    template: Template,
    opts: PageValues,
    tabs: Tabs,

    name: TextInput,
    description: TextArea,
    visibility: RadioGroup,
    auto_merge: Toggle,
    protect: Toggle,
    save: Button,
    dirty: bool,

    members: DataTable,
    remove: Button,
    invite: Button,

    env: ListBox,
    add_var: Button,
    remove_vars: Button,
}

fn member_rows() -> Vec<Vec<Cell>> {
    [
        ("Mira Okafor", "mira@acme.dev", "Owner", "today"),
        ("Jonas Weber", "jonas@acme.dev", "Admin", "2 h ago"),
        ("Ana Costa", "ana@acme.dev", "Member", "yesterday"),
        ("Kai Tanaka", "kai@acme.dev", "Member", "3 d ago"),
        ("Sofia Rossi", "sofia@acme.dev", "Viewer", "never"),
        ("deploy-bot", "bot@acme.dev", "Member", "1 m ago"),
    ]
    .iter()
    .map(|(n, e, r, l)| {
        vec![
            Cell::new(*n),
            Cell::new(*e).tone(Tone::Muted),
            Cell::new(*r),
            Cell::new(*l).tone(Tone::Muted),
        ]
    })
    .collect()
}

fn members_table(id: WidgetId, rows: Vec<Vec<Cell>>) -> DataTable {
    DataTable::try_new(
        id,
        vec![
            Column::new("Name", Constraint::Min(14)).editable(),
            Column::new("Email", Constraint::Length(18)),
            Column::new("Role", Constraint::Length(8)).editable(),
            Column::new("Last active", Constraint::Length(11)).right(),
        ],
        rows,
    )
    .expect("rectangular fixture")
    .validator(role_validator)
}

fn env_items() -> Vec<ListItem> {
    [
        ("DATABASE_URL", "postgres://…"),
        ("REDIS_URL", "redis://…"),
        ("STRIPE_KEY", "sk_live_…"),
        ("LOG_LEVEL", "info"),
        ("FEATURE_FLAGS", "beta,otel"),
    ]
    .iter()
    .map(|(k, v)| ListItem::new(k).meta(v))
    .collect()
}

fn states_grid() -> StateGrid {
    use SpecimenState::*;
    let tabs = Tabs::new(ID.sub("states-tabs"), &["General", "Members", "Env"]);
    let mut tabs = tabs;
    tabs.update_item(0, |general| general.dirty = true);
    StateGrid::new(ID.sub("states"))
        .label_width(14)
        .column_width(26)
        .narrow_width(16)
        .rows(&[
            Rest, Hover, Pressed, Focused, Selected, Disabled, Error, Editing, Empty, Overflow,
            Narrow,
        ])
        .column("Section tabs", tabs)
        .column(
            "Members",
            members_table(ID.sub("states-members"), member_rows()).cell_nav(true),
        )
        .column(
            "Environment",
            ListBox::new(ID.sub("states-env"), env_items(), SelectMode::Multi)
                .empty_text("No variables defined"),
        )
}

impl SettingsPage {
    pub fn new() -> Self {
        let mut page = Self {
            template: Template::new(ID, states_grid()),
            opts: PageValues::new(page_options(PageId::Settings)),
            tabs: Tabs::new(ID.sub("tabs"), &["General", "Members", "Environment"]),
            name: TextInput::new(ID.sub("name"), "Project name")
                .value("payments-gateway")
                .required(true),
            description: TextArea::new(ID.sub("desc"), "Description", 3)
                .value("Handles checkout, invoicing and refunds for the storefront."),
            visibility: RadioGroup::new(
                ID.sub("vis"),
                "Visibility",
                &["Private", "Internal", "Public"],
                0,
            ),
            auto_merge: Toggle::new(ID.sub("automerge"), "Auto-merge approved PRs", false),
            protect: Toggle::new(ID.sub("protect"), "Protect main branch", true),
            save: Button::primary(ID.sub("save"), "Save changes"),
            dirty: false,
            members: members_table(ID.sub("members"), member_rows()),
            remove: Button::danger(ID.sub("remove"), "Remove…"),
            invite: Button::secondary(ID.sub("invite"), "Invite"),
            env: ListBox::new(ID.sub("env"), env_items(), SelectMode::Multi),
            add_var: Button::primary(ID.sub("addvar"), "Add variable…"),
            remove_vars: Button::danger(ID.sub("rmvars"), "Remove selected"),
        };
        page.apply();
        if let Some(i) = ["general", "members", "environment"]
            .iter()
            .position(|t| *t == page.opts.value("settings.tab"))
        {
            page.tabs.set_active(i);
            page.tabs.cursor = i;
        }
        page
    }

    fn apply(&mut self) {
        let o = &self.opts;
        self.members = self.members.clone().cell_nav(o.on("settings.cell_nav"));
        let mode = match o.value("settings.env_select") {
            "single" => SelectMode::Single,
            _ => SelectMode::Multi,
        };
        self.env = ListBox::new(ID.sub("env"), self.env.items.clone(), mode)
            .empty_text("No variables defined");
        self.description = TextArea::new(
            ID.sub("desc"),
            "Description",
            o.number("settings.desc_rows").unwrap_or(3),
        )
        .value(self.description.field.text());
    }

    fn save(&mut self, cx: &mut Cx) {
        if !self.dirty {
            cx.status("Nothing to save");
        } else if self.opts.on("settings.save_confirm") {
            cx.open_dialog(Dialog::confirm(
                SAVE_DLG,
                "Save settings?",
                "Changing visibility or branch protection applies to every open task.",
                "Save",
            ));
        } else {
            self.dirty = false;
            cx.status("Settings saved ✓");
        }
    }

    fn selected_member(&self) -> Option<usize> {
        if self.members.is_empty() {
            None
        } else {
            Some(self.members.source_row(self.members.cursor_row))
        }
    }
}

impl SettingsPage {
    fn general_needs(&self, ctx: &RenderCtx) -> (u16, u16) {
        let w = |s: &str| junie_tui::ui::text::width(s) as u16;

        let left = self.description.min_width(Size::ZERO, ctx);
        let right = self
            .auto_merge
            .width()
            .max(self.protect.width())
            .max(w(&self.visibility.label) + 2)
            .max(
                self.visibility
                    .options
                    .iter()
                    .map(|o| w(o) + 6)
                    .max()
                    .unwrap_or(0),
            );
        (left, right)
    }

    fn general_columns(&self, inner: u16, ctx: &RenderCtx) -> Option<u16> {
        let (left, right) = self.general_needs(ctx);
        let half = (inner / 2).saturating_sub(2);
        if half >= left && inner.saturating_sub(half + 4) >= right {
            Some(half)
        } else if inner >= left + 4 + right {
            Some(inner - 4 - right)
        } else {
            None
        }
    }

    fn general_height(&self, inner: u16, ctx: &RenderCtx) -> u16 {
        if self.general_columns(inner, ctx).is_some() {
            return 16;
        }

        let rows = TextInput::HEIGHT + self.description.height() + 1 + 7 + 1 + 1;
        (rows + 3).max(16)
    }

    fn render_playground(&mut self, area: Rect, buf: &mut Buffer, ctx: &mut RenderCtx) {
        let t = ctx.theme;
        let (strip, body) = Tabs::layout(area);
        self.tabs.render(strip, buf, ctx, t.canvas);
        match self.tabs.active {
            0 => {
                let title = if self.dirty {
                    "General · unsaved"
                } else {
                    "General"
                };
                let panel = Panel::card(Some(title));
                let bg = panel.bg(t);
                let inner_w = panel.inner(Size::new(body.width, 0)).width;
                let card_h = self.general_height(inner_w, ctx);
                let inner = panel.render(
                    Rect::new(body.x, body.y, body.width, body.height.min(card_h)),
                    buf,
                    t,
                );
                let (l, r) = match self.general_columns(inner.width, ctx) {
                    Some(lw) => crate::pages::layout::columns(inner, lw, 4),
                    None => {
                        let top = TextInput::HEIGHT + self.description.height() + 1;
                        (
                            inner,
                            Rect::new(
                                inner.x,
                                inner.y + top.min(inner.height),
                                inner.width,
                                inner.height.saturating_sub(top),
                            ),
                        )
                    }
                };
                self.name.render(
                    Rect::new(l.x, l.y, l.width, TextInput::HEIGHT),
                    buf,
                    ctx,
                    bg,
                );
                self.description.render(
                    Rect::new(l.x, l.y + 3, l.width, self.description.height()),
                    buf,
                    ctx,
                    bg,
                );
                self.visibility.render(
                    Rect::new(r.x, r.y, r.width, self.visibility.height()),
                    buf,
                    ctx,
                    bg,
                );
                self.auto_merge
                    .render(Rect::new(r.x, r.y + 5, r.width, 1), buf, ctx, bg);
                self.protect
                    .render(Rect::new(r.x, r.y + 6, r.width, 1), buf, ctx, bg);
                let ay = inner.bottom().saturating_sub(1);
                self.save
                    .render(Rect::new(inner.x, ay, inner.width, 1), buf, ctx, bg);
                if !self.dirty {
                    buf.set_string(
                        inner.x + self.save.width() + 2,
                        ay,
                        "No changes",
                        t.faint().bg(bg),
                    );
                }
            }
            1 => {
                let panel =
                    Panel::card(Some("Members")).focused(ctx.interaction.focused(self.members.id));
                let bg = panel.bg(t);
                let th = (self.members.len() as u16 + 1).max(2);
                let card = Rect::new(body.x, body.y, body.width, (th + 5).min(body.height));
                let inner = panel.render(card, buf, t);
                let th = th.min(inner.height.saturating_sub(2));
                self.members
                    .render(Rect::new(inner.x, inner.y, inner.width, th), buf, ctx, bg);
                let pos = scrollbar::position_label(&self.members.scroll);
                let meta = match self.members.edit_error() {
                    Some(e) => e.to_owned(),
                    None if pos.is_empty() => format!("{} members", self.members.len()),
                    None => format!("{} members · {pos}", self.members.len()),
                };
                panel.draw_meta(card, buf, t, &meta);
                let ay = inner.bottom().saturating_sub(1);
                let rects = row_layout(
                    Rect::new(inner.x, ay, inner.width, 1),
                    &[self.invite.width(), self.remove.width()],
                    2,
                );
                self.invite.render(rects[0], buf, ctx, bg);
                self.remove.disabled = self.members.is_empty();
                self.remove.render(rects[1], buf, ctx, bg);
                if let Some(i) = self.selected_member() {
                    let s = format!("cursor: {}", self.members.rows()[i][0].text);
                    buf.set_string(rects[1].right() + 2, ay, &s, t.faint().bg(bg));
                }
            }
            _ => {
                let count = self.env.checked_count();
                let meta = format!("{count} selected");
                let panel = Panel::card(Some("Environment variables")).meta(&meta);
                let bg = panel.bg(t);
                let lh = (self.env.items.len() as u16).max(1);
                let card = Rect::new(body.x, body.y, body.width, (lh + 5).min(body.height));
                let inner = panel.render(card, buf, t);
                let lh = lh.min(inner.height.saturating_sub(2));
                self.env.render(
                    Rect::new(inner.x, inner.y, inner.width.min(60), lh),
                    buf,
                    ctx,
                    bg,
                );
                let ay = inner.bottom().saturating_sub(1);
                let rects = row_layout(
                    Rect::new(inner.x, ay, inner.width, 1),
                    &[self.add_var.width(), self.remove_vars.width()],
                    2,
                );
                self.add_var.render(rects[0], buf, ctx, bg);
                self.remove_vars.disabled = count == 0;
                self.remove_vars.render(rects[1], buf, ctx, bg);
            }
        }
    }
}

impl Page for SettingsPage {
    fn title(&self) -> &'static str {
        "Project settings"
    }
    fn blurb(&self) -> &'static str {
        "Composed: tabs, form, editable table, list, dialogs"
    }

    fn measure(&self, avail: Size, ctx: &RenderCtx) -> Size {
        let top = Tabs::STRIP_H + 1;
        let (left, right) = self.general_needs(ctx);
        let panel = Panel::card(Some("General · unsaved"));
        let width = avail
            .width
            .max(panel.outer(Size::new(left.max(right), 0)).width);
        let inner = panel.inner(Size::new(width, 0)).width;
        let height = match self.tabs.active {
            0 => top + self.general_height(inner, ctx),
            1 => top + (self.members.len() as u16 + 1).max(2) + 5,
            _ => avail.height,
        };
        Size::new(width, height)
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
        let out = match self.template.handle(ev, cx) {
            Some(o) => o,

            None if self.template.section() == Section::Playground
                || matches!(ev, PageEvent::DialogClosed { .. } | PageEvent::Blur(_)) =>
            {
                self.handle_event(ev, cx)
            }
            None => Outcome::Ignored,
        };

        let dirty = self.dirty && self.opts.on("settings.tab_dirty");
        self.tabs.update_item(0, |general| general.dirty = dirty);
        out
    }

    fn editing(&self) -> bool {
        self.name.editing || self.description.editing || self.members.is_editing()
    }

    fn leave_stakes(&self) -> junie_tui::ui::leave::Stakes {
        if self.dirty {
            junie_tui::ui::leave::Stakes::Unsaved { changes: 0 }
        } else {
            junie_tui::ui::leave::Stakes::Clean
        }
    }

    fn hints(&self, focus: Option<WidgetId>) -> Vec<Hint> {
        if let Some(h) = self.template.hints(focus) {
            return h;
        }
        if self.members.is_editing() {
            return vec![("Enter", "Apply"), ("Esc", "Cancel"), ("Tab", "Next cell")];
        }
        if self.name.editing || self.description.editing {
            return vec![("Enter", "Apply"), ("Esc", "Cancel"), ("Tab", "Next")];
        }
        match focus {
            Some(f) if f == self.tabs.id => vec![("←→", "Switch tab"), ("1–3", "Jump")],
            Some(f) if f == self.members.id => {
                vec![
                    ("↑↓←→", "Cell"),
                    junie_tui::widgets::widget_hints::EDIT_PAIR,
                    ("s", "Sort"),
                ]
            }
            Some(f) if f == self.env.id => vec![("Space", "Toggle"), ("a", "All")],
            _ => vec![("Enter", "Edit / activate"), ("Ctrl+S", "Save")],
        }
    }

    fn option_value(&self, key: &str) -> Option<usize> {
        self.opts.get(key)
    }

    fn set_option(&mut self, key: &str, value: usize) {
        if self.opts.set(key, value) {
            if key == "settings.tab" {
                self.tabs.set_active(value);
                self.tabs.cursor = value;
            }
            self.apply();
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

impl SettingsPage {
    fn pointer(&mut self, ev: &PointerEvent, cx: &mut Cx) -> Outcome {
        let click = ev.is(Phase::Click);
        if let Some(r) = self.tabs.route(ev) {
            if click {
                cx.focus.focus(self.tabs.id);
            }
            return r.outcome();
        }
        if let Some(r) = self.name.route(ev) {
            if click {
                cx.focus.focus(self.name.id);
            }
            return r.outcome();
        }
        if let Some(r) = self.description.route(ev) {
            if click && ev.id == self.description.id {
                cx.focus.focus(self.description.id);
            }
            return r.outcome();
        }
        if let Some(r) = self.visibility.route(ev) {
            let o = r.outcome();
            if o != Outcome::Ignored {
                cx.focus.focus(self.visibility.id);
                self.dirty = true;
            }
            return o;
        }
        for t in [&mut self.auto_merge, &mut self.protect] {
            if let Some(r) = t.route(ev) {
                let o = r.outcome();
                if o != Outcome::Ignored {
                    self.dirty = true;
                }
                return o;
            }
        }
        if let Some(act) = clicked(&mut self.save, ev) {
            if act {
                self.save(cx);
            }
            return Outcome::Changed;
        }
        if let Some(r) = self.members.route(ev) {
            if click && !scrollbar::is_bar_of(ev.id, self.members.id) {
                cx.focus.focus(self.members.id);
            }
            return r.outcome();
        }
        if let Some(act) = clicked(&mut self.remove, ev) {
            if act {
                self.open_remove(cx);
            }
            return Outcome::Changed;
        }
        if let Some(act) = clicked(&mut self.invite, ev) {
            if act {
                cx.status("Invitations are sent from the web console");
            }
            return Outcome::Changed;
        }
        if let Some(r) = self.env.route(ev) {
            if click && ev.index_of(Part::ROW).is_some() {
                cx.focus.focus(self.env.id);
            }
            return r.outcome();
        }
        if let Some(act) = clicked(&mut self.add_var, ev) {
            if act {
                self.open_add(cx);
            }
            return Outcome::Changed;
        }
        if let Some(act) = clicked(&mut self.remove_vars, ev) {
            if act {
                self.remove_selected_vars(cx);
            }
            return Outcome::Changed;
        }
        Outcome::Ignored
    }
}

fn clicked(b: &mut Button, ev: &PointerEvent) -> Option<bool> {
    let r = b.route(ev)?;
    (r.outcome() != Outcome::Ignored).then(|| r.has_actions())
}

impl SettingsPage {
    fn handle_event(&mut self, ev: &PageEvent, cx: &mut Cx) -> Outcome {
        if let PageEvent::Blur(ch) = ev {
            return forward_blur(
                *ch,
                &mut [&mut self.name, &mut self.description, &mut self.members],
            );
        }
        match ev {
            PageEvent::Key(key) => {
                let Some(f) = cx.focus.current() else {
                    return Outcome::Ignored;
                };
                if f == self.tabs.id {
                    return self.tabs.on_key(key).0;
                }
                if key.ctrl_char('s') && self.tabs.active == 0 {
                    self.dirty = false;
                    cx.status("Settings saved ✓");
                    return Outcome::Changed;
                }
                let dirty = &mut self.dirty;
                macro_rules! input {
                    ($w:expr) => {{
                        let (o, iev) = $w.on_key(key);
                        match iev {
                            Some(InputAction::CommittedTab { backward: false }) => cx.focus_next(),
                            Some(InputAction::CommittedTab { backward: true }) => cx.focus_prev(),
                            Some(InputAction::Changed) => *dirty = true,
                            _ => {}
                        }
                        return o;
                    }};
                }
                if f == self.name.id {
                    input!(self.name);
                }
                if f == self.description.id {
                    input!(self.description);
                }
                macro_rules! simple {
                    ($w:expr) => {{
                        let o = $w.on_key(key);
                        if o == Outcome::Changed {
                            *dirty = true;
                        }
                        return o;
                    }};
                }
                if f == self.visibility.id {
                    simple!(self.visibility);
                }
                if f == self.auto_merge.id {
                    simple!(self.auto_merge);
                }
                if f == self.protect.id {
                    simple!(self.protect);
                }
                if f == self.save.id {
                    let (o, act) = self.save.on_key(key);
                    if act {
                        self.save(cx);
                    }
                    return o;
                }
                if f == self.members.id {
                    let (o, actions) = self.members.on_key(key).into_parts();
                    for tev in actions {
                        match tev {
                            TableAction::Committed { .. } => cx.status("Member updated"),
                            TableAction::LeaveForward => cx.focus_next(),
                            TableAction::LeaveBackward => cx.focus_prev(),
                            _ => {}
                        }
                    }
                    return o;
                }
                if f == self.remove.id {
                    let (o, act) = self.remove.on_key(key);
                    if act {
                        self.open_remove(cx);
                    }
                    return o;
                }
                if f == self.invite.id {
                    let (o, act) = self.invite.on_key(key);
                    if act {
                        cx.status("Invitations are sent from the web console");
                    }
                    return o;
                }
                if f == self.env.id {
                    return self.env.on_key(key);
                }
                if f == self.add_var.id {
                    let (o, act) = self.add_var.on_key(key);
                    if act {
                        self.open_add(cx);
                    }
                    return o;
                }
                if f == self.remove_vars.id {
                    let (o, act) = self.remove_vars.on_key(key);
                    if act {
                        self.remove_selected_vars(cx);
                    }
                    return o;
                }
                Outcome::Ignored
            }
            PageEvent::Paste(text) => {
                if self.name.editing {
                    return self.name.on_paste(text);
                }
                if self.description.editing {
                    return self.description.on_paste(text);
                }
                self.members.on_paste(text)
            }
            PageEvent::Pointer(ev) => self.pointer(ev, cx),
            PageEvent::DialogClosed { id, result, value } => {
                if *id == REMOVE_DLG {
                    if *result == DialogResult::Action(1) {
                        if let Some(i) = self.selected_member() {
                            let name = self.members.rows()[i][0].text.clone();
                            let mut rows = self.members.rows().to_vec();
                            rows.remove(i);
                            match self.members.try_set_rows(rows) {
                                Ok(()) => cx.status(format!("Removed {name}")),
                                Err(e) => cx.status(format!("Member not removed · {e}")),
                            }
                        }
                    } else {
                        cx.status("Kept member");
                    }
                    return Outcome::Changed;
                }
                if *id == ADD_DLG {
                    if *result == DialogResult::Action(1)
                        && let Some(name) = value.clone()
                    {
                        self.env.items.push(ListItem::new(&name).meta("(empty)"));
                        cx.status(format!("Added {name}"));
                    }
                    return Outcome::Changed;
                }
                if *id == SAVE_DLG {
                    if *result == DialogResult::Action(1) {
                        self.dirty = false;
                        cx.status("Settings saved ✓");
                    }
                    return Outcome::Changed;
                }
                Outcome::Ignored
            }
            _ => Outcome::Ignored,
        }
    }

    fn open_remove(&mut self, cx: &mut Cx) {
        let Some(i) = self.selected_member() else {
            return;
        };
        let name = self.members.rows()[i][0].text.clone();
        let role = self.members.rows()[i][2].text.clone();
        cx.open_dialog(Dialog::destructive(
            REMOVE_DLG,
            "Remove member?",
            &format!("{name} ({role}) will lose access to every task in this project immediately."),
            "Remove",
        ));
    }

    fn open_add(&mut self, cx: &mut Cx) {
        let input = TextInput::new(ADD_DLG.sub("name"), "Variable name")
            .placeholder("API_BASE_URL")
            .required(true)
            .validator(var_validator);
        cx.open_dialog(Dialog::prompt(ADD_DLG, "Add variable", input, "Add"));
    }

    fn remove_selected_vars(&mut self, cx: &mut Cx) {
        let rows = self.env.selection.to_vec();
        let removed = self.env.remove_rows(&rows);
        cx.status(format!("Removed {removed} variables"));
    }
}
