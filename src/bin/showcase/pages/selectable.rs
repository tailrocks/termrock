use ratatui::buffer::Buffer;
use ratatui::crossterm::event::KeyCode;
use ratatui::layout::{Constraint, Rect};

use crate::pages::row_menu::{MENU_HINTS, OPEN_HINT, RowMenu};
use crate::pages::template::{Section, Template};
use crate::pages::{Cx, CxExt, Hint, Page, PageEvent};
use junie_tui::core::event::{ClientPlatform, Outcome};
use junie_tui::core::id::WidgetId;
use junie_tui::core::intent::Phase;
use junie_tui::core::pointer::{MouseTarget, PointerEvent};
use junie_tui::core::selection::{Activate, ClickPolicy, SelectionMode};
use junie_tui::ui::context::ContextRequest;
use junie_tui::ui::ctx::RenderCtx;
use junie_tui::ui::layout::StackChild;
use junie_tui::ui::specimen::{SpecimenState, StateGrid};
use junie_tui::widgets::button::{Button, row_layout};
use junie_tui::widgets::choice::Checkbox;
use junie_tui::widgets::dialog::{Dialog, DialogResult};
use junie_tui::widgets::empty::{self, EmptyState};
use junie_tui::widgets::input::{InputAction, TextInput};
use junie_tui::widgets::keyhint;
use junie_tui::widgets::menu::MenuItem;
use junie_tui::widgets::panel::Panel;
use junie_tui::widgets::scrollbar;
use junie_tui::widgets::select::Select;
use junie_tui::widgets::table::{Cell, Column, DataTable, TableAction, Tone};

const ID: WidgetId = WidgetId::of("selectable");
const DELETE: WidgetId = WidgetId::of("selectable.delete_dialog");
const DISCARD: WidgetId = WidgetId::of("selectable.discard_dialog");
const MENU: WidgetId = WidgetId::of("selectable.menu");

pub const MIXED: &str = "— mixed —";

const EDITOR_W: u16 = 44;

const EDITOR_H: u16 = 17;

const TABLE_ROWS: u16 = 10;

const ROLES: [&str; 3] = ["Admin", "Editor", "Viewer"];
const STATUSES: [&str; 3] = ["Active", "Invited", "Suspended"];
const REGIONS: [&str; 3] = ["EU", "US", "APAC"];

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Member {
    pub name: String,
    pub email: String,

    pub role: Option<usize>,
    pub status: usize,
    pub region: usize,
    pub on_call: bool,
    pub notes: String,
}

fn member(
    name: &str,
    email: &str,
    role: Option<usize>,
    status: usize,
    region: usize,
    on_call: bool,
    notes: &str,
) -> Member {
    Member {
        name: name.into(),
        email: email.into(),
        role,
        status,
        region,
        on_call,
        notes: notes.into(),
    }
}

pub fn members() -> Vec<Member> {
    vec![
        member(
            "Dana Whitfield",
            "dana@northwind.dev",
            None,
            0,
            0,
            true,
            "Workspace owner",
        ),
        member(
            "Ari Moreno",
            "ari@northwind.dev",
            Some(0),
            0,
            1,
            true,
            "Runs the release train",
        ),
        member(
            "Cleo Barnes",
            "cleo@northwind.dev",
            Some(1),
            0,
            0,
            false,
            "Docs and changelog",
        ),
        member("Bo Lindqvist", "bo@northwind.dev", Some(1), 1, 0, false, ""),
        member(
            "Eli Nakamura",
            "eli@contractor.io",
            Some(2),
            0,
            2,
            false,
            "Contract ends in March",
        ),
        member(
            "Fay Okafor",
            "fay@northwind.dev",
            Some(1),
            0,
            1,
            true,
            "On call this week",
        ),
        member(
            "Gus Petrov",
            "gus@northwind.dev",
            Some(2),
            2,
            0,
            false,
            "Left the team",
        ),
        member("Hana Sato", "hana@northwind.dev", Some(1), 0, 2, false, ""),
        member(
            "Ivo Brandt",
            "ivo@northwind.dev",
            Some(0),
            0,
            0,
            true,
            "Billing contact",
        ),
        member("June Park", "june@northwind.dev", Some(2), 1, 1, false, ""),
        member(
            "Kai Mensah",
            "kai@northwind.dev",
            Some(1),
            0,
            2,
            false,
            "Mobile app",
        ),
        member("Lena Varga", "lena@northwind.dev", Some(2), 0, 0, false, ""),
    ]
}

fn columns() -> Vec<Column> {
    vec![
        Column::new("Name", Constraint::Min(15)),
        Column::new("Role", Constraint::Length(6)),
        Column::new("Status", Constraint::Length(9)),
        Column::new("Region", Constraint::Length(6)),
        Column::new("On call", Constraint::Length(7)),
        Column::new("Email", Constraint::Min(20)),
        Column::new("Notes", Constraint::Min(22)),
    ]
}

fn cells(m: &Member) -> Vec<Cell> {
    let status = Cell::new(STATUSES[m.status]).tone(match m.status {
        1 => Tone::Secondary,
        2 => Tone::Warning,
        _ => Tone::Normal,
    });
    vec![
        Cell::new(m.name.clone()),
        Cell::new(m.role.map_or("Owner", |r| ROLES[r])),
        status,
        Cell::new(REGIONS[m.region]),
        Cell::new(if m.on_call { "Yes" } else { "—" }).tone(if m.on_call {
            Tone::Normal
        } else {
            Tone::Faint
        }),
        Cell::new(m.email.clone()),
        Cell::new(if m.notes.is_empty() { "—" } else { &m.notes }),
    ]
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
struct FormValues {
    name: Option<String>,
    email: Option<String>,
    role: Option<usize>,
    status: Option<usize>,
    region: Option<usize>,
    on_call: Option<bool>,
    notes: Option<String>,
}

fn name_rule(s: &str) -> Option<String> {
    s.trim().is_empty().then(|| "Enter a name".to_owned())
}

fn email_rule(s: &str) -> Option<String> {
    (!s.contains('@') || !s.contains('.')).then(|| "Enter a valid email address".to_owned())
}

fn shared<T: PartialEq + Clone>(mut it: impl Iterator<Item = T>) -> Option<T> {
    let first = it.next()?;
    it.all(|v| v == first).then_some(first)
}

const KEYS: [&str; 5] = [
    "selection.esc_clears",
    "selection.toggle_mods",
    "selection.disabled_rows",
    "selection.check_column",
    "selection.activate",
];

const ACTIVATE: [Activate; 4] = [
    Activate::Double,
    Activate::ClickAgain,
    Activate::Single,
    Activate::Never,
];

fn members_table(members: &[Member], k: &[usize; 5], platform: ClientPlatform) -> DataTable {
    use ratatui::crossterm::event::KeyModifiers;
    let [esc, mods, disabled, checks, activate] = *k;
    let toggle = match mods {
        1 => KeyModifiers::CONTROL,
        2 => KeyModifiers::ALT,
        _ => DataTable::toggle_mods_for(platform),
    };
    DataTable::new(
        ID.sub("members"),
        columns(),
        members.iter().map(cells).collect(),
    )
    .selection_mode(SelectionMode::Multi)
    .selection_column(checks == 1)
    .disabled_rows(if disabled == 0 { &[0] } else { &[] })
    .esc_clears(esc == 1)
    .toggle_mods(toggle)
    .click(ClickPolicy::DATA.activate(ACTIVATE[activate.min(3)]))
    .empty_text("No members yet")
}

fn states_grid() -> StateGrid {
    use SpecimenState::*;
    let id = ID.sub("states");
    let sample = |k: usize, checks: bool| {
        let cols = vec![
            Column::new("Name", Constraint::Min(12)),
            Column::new("Role", Constraint::Length(7)),
        ];
        let rows = members()
            .iter()
            .take(3)
            .map(|m| {
                let c = cells(m);
                vec![c[0].clone(), c[1].clone()]
            })
            .collect();
        DataTable::new(id.child(k), cols, rows)
            .selection_mode(SelectionMode::Multi)
            .selection_column(checks)
    };
    StateGrid::new(id)
        .rows(&[
            Rest,
            Hover,
            Pressed,
            Focused,
            FocusedHover,
            Selected,
            Disabled,
            Empty,
            Narrow,
        ])
        .column_width(36)
        .narrow_width(24)
        .column("Checkbox column", sample(0, true))
        .column("Rows only", sample(1, false))
}

pub struct SelectablePage {
    template: Template,

    knobs: [usize; 5],
    pub members: Vec<Member>,
    pub table: DataTable,
    pub name: TextInput,
    pub email: TextInput,
    pub role: Select,
    pub status: Select,
    pub region: Select,
    pub on_call: Checkbox,
    pub notes: TextInput,
    pub apply: Button,
    pub revert: Button,
    pub delete: Button,

    loaded: Vec<usize>,

    base: FormValues,

    notes_mixed: bool,

    pub feedback: Option<String>,

    pub leave_edits: LeaveEdits,

    pub platform: ClientPlatform,

    menu: RowMenu<(usize, usize)>,

    pending: Option<(junie_tui::core::selection::RowSelection, usize)>,
    sections: crate::pages::layout::Sections,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum LeaveEdits {
    #[default]
    Commit,

    #[allow(dead_code)]
    Ask,

    Discard,
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum RowAction {
    Edit,
    Delete,
}

impl SelectablePage {
    pub fn new() -> Self {
        Self::for_platform(ClientPlatform::current())
    }

    pub fn for_platform(platform: ClientPlatform) -> Self {
        let members = members();
        let knobs = KEYS.map(|k| crate::options::find(k).map_or(0, |o| o.default));
        let table = members_table(&members, &knobs, platform);
        let mut page = Self {
            template: Template::new(ID, states_grid()),
            knobs,
            members,
            table,
            name: TextInput::new(ID.sub("name"), "Name")
                .required(true)
                .required_message("Enter a name")
                .validator(name_rule),
            email: TextInput::new(ID.sub("email"), "Email")
                .required(true)
                .required_message("Enter a valid email address")
                .validator(email_rule),
            role: Select::new(ID.sub("role"), "Role", &ROLES, 0),
            status: Select::new(ID.sub("status"), "Status", &STATUSES, 0),
            region: Select::new(ID.sub("region"), "Region", &REGIONS, 0),
            on_call: Checkbox::new(ID.sub("oncall"), "On call", false),
            notes: TextInput::new(ID.sub("notes"), "Notes").plain_label(),
            apply: Button::primary(ID.sub("apply"), "Apply"),
            revert: Button::subtle(ID.sub("revert"), "Revert"),
            delete: Button::danger(ID.sub("delete"), "Delete…"),
            loaded: vec![],
            base: FormValues::default(),
            notes_mixed: false,
            feedback: None,
            leave_edits: LeaveEdits::default(),
            platform,
            menu: RowMenu::default(),
            pending: None,
            sections: crate::pages::layout::Sections::new(ID.sub("page")),
        };
        page.load();
        page
    }

    pub fn load(&mut self) {
        self.loaded = self.table.selected_rows();
        let sel: Vec<&Member> = self.loaded.iter().map(|&r| &self.members[r]).collect();
        let single = sel.len() == 1;
        for input in [&mut self.name, &mut self.email, &mut self.notes] {
            input.editing = false;
            input.field.error = None;
        }
        let text = |f: fn(&Member) -> &str| {
            if single {
                f(sel[0]).to_owned()
            } else {
                String::new()
            }
        };
        self.name.field.buffer.set_text(text(|m| &m.name));
        self.email.field.buffer.set_text(text(|m| &m.email));
        let pick = |sel_w: &mut Select, v: Option<usize>| {
            sel_w.open = false;
            match v {
                Some(i) => {
                    sel_w.selected = i;
                    sel_w.set_mixed(None);
                }
                None => sel_w.set_mixed(Some(MIXED)),
            }
        };
        pick(
            &mut self.role,
            shared(sel.iter().map(|m| m.role.unwrap_or(0))),
        );
        pick(&mut self.status, shared(sel.iter().map(|m| m.status)));
        pick(&mut self.region, shared(sel.iter().map(|m| m.region)));
        match shared(sel.iter().map(|m| m.on_call)) {
            Some(on) => {
                self.on_call.checked = on;
                self.on_call.mixed = false;
            }
            None => {
                self.on_call.checked = false;
                self.on_call.mixed = true;
            }
        }
        let notes = shared(sel.iter().map(|m| m.notes.clone()));
        self.notes_mixed = notes.is_none() && !sel.is_empty();
        self.notes.field.buffer.set_text(notes.unwrap_or_default());
        self.notes.placeholder = if self.notes_mixed {
            MIXED.to_owned()
        } else {
            "Add a note".to_owned()
        };
        self.base = self.values();
    }

    fn values(&self) -> FormValues {
        let single = self.loaded.len() == 1;
        let choice = |s: &Select| s.mixed.is_none().then_some(s.selected);
        FormValues {
            name: single.then(|| self.name.text().to_owned()),
            email: single.then(|| self.email.text().to_owned()),
            role: choice(&self.role),
            status: choice(&self.status),
            region: choice(&self.region),
            on_call: (!self.on_call.mixed).then_some(self.on_call.checked),
            notes: (!(self.notes_mixed && self.notes.text().is_empty()))
                .then(|| self.notes.text().to_owned()),
        }
    }

    pub fn dirty(&self) -> bool {
        !self.loaded.is_empty() && self.values() != self.base
    }

    fn editors(&self) -> [WidgetId; 9] {
        [
            self.name.id,
            self.email.id,
            self.role.id,
            self.status.id,
            self.region.id,
            self.on_call.id,
            self.notes.id,
            self.apply.id,
            self.revert.id,
        ]
    }

    fn selection_changed(&mut self) {
        self.feedback = None;
        self.load();
    }

    fn do_apply(&mut self, cx: &mut Cx) {
        for input in [&mut self.name, &mut self.email, &mut self.notes] {
            if input.editing {
                input.commit();
            }
        }
        if !self.dirty() {
            return;
        }
        if self.loaded.len() == 1 {
            let a = self.name.validate();
            let b = self.email.validate();
            if !(a && b) {
                cx.status("Fix the highlighted fields");
                let first = if self.name.field.error.is_some() {
                    self.name.id
                } else {
                    self.email.id
                };
                cx.focus.focus(first);
                return;
            }
        }
        let now = self.values();
        let base = self.base.clone();
        for &r in &self.loaded {
            let m = &mut self.members[r];
            if now.name != base.name
                && let Some(v) = &now.name
            {
                m.name = v.trim().to_owned();
            }
            if now.email != base.email
                && let Some(v) = &now.email
            {
                m.email = v.trim().to_owned();
            }
            if now.role != base.role
                && let Some(v) = now.role
            {
                m.role = Some(v);
            }
            if now.status != base.status
                && let Some(v) = now.status
            {
                m.status = v;
            }
            if now.region != base.region
                && let Some(v) = now.region
            {
                m.region = v;
            }
            if now.on_call != base.on_call
                && let Some(v) = now.on_call
            {
                m.on_call = v;
            }
            if now.notes != base.notes
                && let Some(v) = &now.notes
            {
                m.notes = v.trim().to_owned();
            }
            self.table.set_row(r, cells(m));
        }
        let n = self.loaded.len();
        let msg = if n == 1 {
            format!("Saved {}", self.members[self.loaded[0]].name)
        } else {
            format!("Updated {n} members")
        };
        self.load();
        cx.status(msg.clone());
        self.feedback = Some(msg);
    }

    fn do_revert(&mut self, cx: &mut Cx) {
        self.load();
        self.feedback = None;
        cx.status("Changes reverted");
    }

    fn ask_delete(&mut self, cx: &mut Cx) {
        let n = self.table.selected_rows().len();
        if n == 0 {
            return;
        }
        let noun = if n == 1 { "member" } else { "members" };
        let d = Dialog::destructive(
            DELETE,
            &format!("Delete {n} {noun}?"),
            "They lose access to the workspace right away. This can't be undone.",
            &format!("Delete {n} {noun}"),
        );
        cx.open_dialog(d);
    }

    fn do_delete(&mut self, cx: &mut Cx) {
        let rows = self.table.selected_rows();
        let n = rows.len();
        self.table.remove_rows(&rows);
        let mut i = 0;
        self.members.retain(|_| {
            let keep = !rows.contains(&i);
            i += 1;
            keep
        });
        self.table.clear_selection();
        self.load();
        let msg = format!("Deleted {n} {}", if n == 1 { "member" } else { "members" });
        cx.status(msg.clone());
        self.feedback = Some(msg);
        cx.focus.focus(self.table.id);
    }

    fn edit_row(&mut self, src: usize, cx: &mut Cx) {
        if !self.table.selection.contains(src) {
            let disabled = self.table.disabled.clone();
            self.table
                .selection
                .select_only(src, |r| !disabled.contains(&r));
            self.selection_changed();
        }
        if self.loaded.is_empty() {
            return;
        }
        let first = if self.loaded.len() == 1 {
            self.name.id
        } else {
            self.role.id
        };
        cx.focus.focus(first);
    }

    fn edits_valid(&mut self) -> bool {
        for input in [&mut self.name, &mut self.email, &mut self.notes] {
            if input.editing {
                input.commit();
            }
        }
        if self.loaded.len() != 1 {
            return true;
        }
        let a = self.name.validate();
        let b = self.email.validate();
        a && b
    }

    fn table_input(
        &mut self,
        cx: &mut Cx,
        f: impl FnOnce(&mut DataTable) -> (Outcome, Option<TableAction>),
    ) -> Outcome {
        let guard = (self.leave_edits != LeaveEdits::Discard && self.dirty())
            .then(|| (self.table.selection.clone(), self.table.cursor_row));
        let (out, tev) = f(&mut self.table);
        if let (Some((before, cursor)), Some(TableAction::SelectionChanged)) = (guard, &tev) {
            if self.leave_edits == LeaveEdits::Commit && self.edits_valid() {
                self.do_apply(cx);
                return Outcome::Changed;
            }
            let after = std::mem::replace(&mut self.table.selection, before);
            let to = std::mem::replace(&mut self.table.cursor_row, cursor);
            self.pending = Some((after, to));
            let invalid = self.loaded.len() == 1
                && (self.name.field.error.is_some() || self.email.field.error.is_some());
            let text = if invalid {
                "Some fields need fixing before they can be saved. Changing the selection without saving drops the edits."
            } else {
                "The editor has changes that were not applied. Changing the selection without saving drops them."
            };
            let mut d = Dialog::confirm(DISCARD, "Save changes?", text, "Save").actions(
                vec![
                    Button::subtle(DISCARD.sub("cancel"), "Cancel"),
                    Button::secondary(DISCARD.sub("discard"), "Don't Save"),
                    Button::primary(DISCARD.sub("save"), "Save"),
                ],
                Some(0),
            );
            d.initial_focus = DISCARD.sub("save");
            cx.open_dialog(d);
            return Outcome::Changed;
        }
        self.on_table_event(tev, cx);
        out
    }

    fn on_table_event(&mut self, ev: Option<TableAction>, cx: &mut Cx) {
        match ev {
            Some(TableAction::SelectionChanged) => self.selection_changed(),
            Some(TableAction::Activated(src)) => self.edit_row(src, cx),
            _ => {}
        }
    }

    fn row_items(&self, row: usize) -> Vec<(RowAction, MenuItem)> {
        let src = self.table.source_row(row);
        let locked = self.table.is_disabled(src);
        let item = |label: &str, key: &'static str| {
            let m = MenuItem::new(label);
            if locked {
                m.reason("Unavailable")
            } else {
                m.shortcut(key)
            }
        };
        vec![
            (RowAction::Edit, item("Edit", "Enter").separator()),
            (RowAction::Delete, item("Delete…", "d")),
        ]
    }

    fn open_menu(&mut self, req: ContextRequest<(usize, usize)>) {
        let src = self.table.source_row(req.target.0);
        let title = self.members[src].name.clone();
        let items = self
            .row_items(req.target.0)
            .into_iter()
            .map(|(_, m)| m)
            .collect();
        self.menu
            .open(req.menu(MENU, items).title(title), req.target);
    }

    fn choose(&mut self, cell: (usize, usize), i: usize, cx: &mut Cx) -> Outcome {
        let Some(&(action, _)) = self.row_items(cell.0).get(i) else {
            return Outcome::Consumed;
        };
        let src = self.table.source_row(cell.0);
        match action {
            RowAction::Edit => self.edit_row(src, cx),
            RowAction::Delete => {
                self.table_input(cx, |t| {
                    let changed = t.context_selection(cell.0);
                    (
                        Outcome::Changed,
                        changed.then_some(TableAction::SelectionChanged),
                    )
                });

                if self.pending.is_none() {
                    self.ask_delete(cx);
                }
            }
        }
        Outcome::Changed
    }

    fn selects(&mut self) -> [&mut Select; 3] {
        [&mut self.role, &mut self.status, &mut self.region]
    }

    fn render_table(&mut self, area: Rect, buf: &mut Buffer, ctx: &mut RenderCtx) {
        let t = ctx.theme;
        let panel = Panel::card(Some("Members")).focused(ctx.interaction.focused(self.table.id));
        let bg = panel.bg(t);
        let inner = panel.render(area, buf, t);
        self.table.render(inner, buf, ctx, bg);
        let pos = scrollbar::position_label(&self.table.scroll);
        let meta = [self.table.selection_label(), pos]
            .into_iter()
            .filter(|s| !s.is_empty())
            .collect::<Vec<_>>()
            .join(" · ");
        panel.draw_meta(area, buf, t, &meta);
    }

    fn render_editor(&mut self, area: Rect, buf: &mut Buffer, ctx: &mut RenderCtx) {
        let t = ctx.theme;
        let n = self.loaded.len();
        let title = match n {
            0 => "Details".to_owned(),
            1 => "Edit member".to_owned(),
            n => format!("Edit {n} members"),
        };
        let panel = Panel::card(Some(&title));
        let bg = panel.bg(t);
        let inner = panel.render(area, buf, t);
        if n == 0 {
            let e = EmptyState::new("No member selected").hint(
                "Select rows in the table to edit them here. Space selects, \
                 Shift+↑↓ extends, Ctrl+A selects all.",
            );
            empty::render(inner, buf, t, &e, bg);
            return;
        }
        let bottom = inner.bottom();
        let fits = |y: u16, h: u16| y + h <= bottom;
        let mut y = inner.y;
        let gap = 2;
        let half = inner.width.saturating_sub(gap) / 2;
        let left = |y: u16, h: u16| Rect::new(inner.x, y, half, h);
        let right =
            |y: u16, h: u16| Rect::new(inner.x + half + gap, y, inner.width - half - gap, h);

        let heading = if n == 1 {
            "Applies to this member".to_owned()
        } else {
            format!("Applies to all {n} · names and emails stay")
        };
        buf.set_string(
            inner.x + 2,
            y,
            junie_tui::ui::text::truncate(&heading, inner.width.saturating_sub(2) as usize),
            t.faint().bg(bg),
        );
        y += 1;
        if n == 1 && fits(y, 3) {
            self.name.render(left(y, 3), buf, ctx, bg);
            self.email.render(right(y, 3), buf, ctx, bg);
            y += 3;
        }
        if fits(y, 3) {
            self.role.render(left(y, Select::HEIGHT), buf, ctx, bg);
            self.status.render(right(y, Select::HEIGHT), buf, ctx, bg);
            y += 3;
        }
        if fits(y, 3) {
            self.region.render(left(y, Select::HEIGHT), buf, ctx, bg);
            self.on_call.render(right(y + 1, 1), buf, ctx, bg);
            y += 3;
        }
        if fits(y, 3) {
            self.notes
                .render(Rect::new(inner.x, y, inner.width, 3), buf, ctx, bg);
        }

        let dirty = self.dirty();
        self.apply.disabled = !dirty;
        self.revert.disabled = !dirty;
        let ay = bottom.saturating_sub(1);
        if ay > y {
            let widths = [self.apply.width(), self.revert.width(), self.delete.width()];
            let rects = row_layout(Rect::new(inner.x, ay, inner.width, 1), &widths, 2);
            self.apply.render(rects[0], buf, ctx, bg);
            self.revert.render(rects[1], buf, ctx, bg);
            self.delete.render(rects[2], buf, ctx, bg);
            if let Some(msg) = &self.feedback
                && !dirty
                && ay > inner.y
            {
                let msg = format!("{msg} ✓");
                buf.set_string(
                    inner.x + 2,
                    ay - 1,
                    junie_tui::ui::text::truncate(&msg, inner.width.saturating_sub(2) as usize),
                    t.accent_fg().bg(bg),
                );
            } else if dirty && ay > inner.y {
                buf.set_string(
                    inner.x + 2,
                    ay - 1,
                    junie_tui::ui::text::truncate(
                        "Unsaved changes",
                        inner.width.saturating_sub(2) as usize,
                    ),
                    t.secondary().bg(bg),
                );
            }
        }
    }

    fn pointer(&mut self, ev: &PointerEvent, cx: &mut Cx) -> Outcome {
        if ev.is(Phase::Secondary) {
            let cfg = self.menu.config;

            let mut req = None;
            self.table_input(cx, |t| {
                let before = t.selection.clone();
                req = cfg.on_secondary(t, ev);
                let changed = t.selection != before;
                (
                    Outcome::Changed,
                    changed.then_some(TableAction::SelectionChanged),
                )
            });
            let Some(req) = req else {
                return Outcome::Ignored;
            };
            cx.focus.focus(self.table.id);
            if self.pending.is_none() {
                self.open_menu(req);
            }
            return Outcome::Changed;
        }
        let click = ev.is(Phase::Click);
        let mut routed = false;
        let before = cx.focus.current();
        let out = self.table_input(cx, |t| match t.route(ev) {
            Some(r) => {
                routed = true;
                r.into_parts()
            }
            None => (Outcome::Ignored, None),
        });
        if routed {
            if click && ev.bar.is_none() && out != Outcome::Ignored && cx.focus.current() == before
            {
                cx.focus.focus(self.table.id);
            }
            return out;
        }
        for input in [&mut self.name, &mut self.email, &mut self.notes] {
            if let Some(r) = input.route(ev) {
                if click {
                    cx.focus.focus(input.id);
                }
                return r.outcome();
            }
        }
        for sel in self.selects() {
            if let Some(r) = sel.route(ev) {
                if (click || ev.is(Phase::Press)) && ev.id == sel.id {
                    cx.focus.focus(sel.id);
                }

                if r.outcome() == Outcome::Ignored && ev.wheel().is_some() {
                    break;
                }
                return r.outcome();
            }
        }
        if let Some(r) = self.on_call.route(ev) {
            return r.outcome();
        }
        let buttons = [
            self.apply.route(ev),
            self.revert.route(ev),
            self.delete.route(ev),
        ];
        for (i, r) in buttons.into_iter().enumerate() {
            let Some(r) = r else {
                continue;
            };
            if r.into_action().is_some() {
                match i {
                    0 => self.do_apply(cx),
                    1 => self.do_revert(cx),
                    _ => self.ask_delete(cx),
                }
            }
            return if click {
                Outcome::Changed
            } else {
                Outcome::Ignored
            };
        }
        Outcome::Ignored
    }

    fn route_input(
        input: &mut TextInput,
        key: &junie_tui::core::event::Key,
        cx: &mut Cx,
    ) -> Outcome {
        let (out, ev) = input.on_key(key);
        match ev {
            Some(InputAction::CommittedTab { backward: false }) => cx.focus_next(),
            Some(InputAction::CommittedTab { backward: true }) => cx.focus_prev(),
            _ => {}
        }
        out
    }
}

impl SelectablePage {
    fn render_playground(&mut self, area: Rect, buf: &mut Buffer, ctx: &mut RenderCtx) {
        if area.width > 50 + EDITOR_W {
            let tw = area.width - EDITOR_W - 1;
            self.render_table(Rect::new(area.x, area.y, tw, area.height), buf, ctx);
            let eh = area.height.min(EDITOR_H);
            self.render_editor(Rect::new(area.x + tw + 1, area.y, EDITOR_W, eh), buf, ctx);
            self.menu.render(area, buf, ctx);
            return;
        }

        let focus = if ctx.interaction.has_focus(self.table.id) {
            Some(0)
        } else if self
            .editors()
            .iter()
            .any(|&id| ctx.interaction.has_focus(id))
            || ctx.interaction.has_focus(self.delete.id)
        {
            Some(1)
        } else {
            None
        };
        let l = self.sections.layout(
            area,
            &[
                StackChild::flex(8, area.height.saturating_sub(EDITOR_H + 1).max(8)),
                StackChild::fixed(EDITOR_H),
            ],
            focus,
            buf,
            ctx,
        );
        match l.slot(0) {
            Some(r) => self.render_table(r, buf, ctx),
            None => ctx.focus_stop(self.table.id),
        }
        if let Some(r) = l.slot(1) {
            self.render_editor(r, buf, ctx);
        }
        self.menu.render(area, buf, ctx);
    }
}

impl Page for SelectablePage {
    fn measure(&self, avail: ratatui::layout::Size, ctx: &RenderCtx) -> ratatui::layout::Size {
        use junie_tui::ui::measure::Measure;
        let any = ratatui::layout::Size::new(0, 0);

        let table_h = self.table.measure(any, ctx).height.min(1 + TABLE_ROWS) + 4;
        let min = self.table.min_width(any, ctx) + 4;
        if avail.width > 50 + EDITOR_W {
            ratatui::layout::Size::new(avail.width, table_h.max(EDITOR_H))
        } else {
            ratatui::layout::Size::new(avail.width.max(min).max(EDITOR_W), table_h + 1 + EDITOR_H)
        }
    }
    fn title(&self) -> &'static str {
        "Selectable table"
    }
    fn blurb(&self) -> &'static str {
        "Select rows, edit one or many together, delete with a confirmation"
    }

    fn render(&mut self, area: Rect, buf: &mut Buffer, ctx: &mut RenderCtx) {
        let body = self.template.render(area, buf, ctx);
        match self.template.section() {
            Section::Playground => self.render_playground(body, buf, ctx),
            Section::Options => self.template.render_options(body, buf, ctx),
            Section::States => self.template.render_states(body, buf, ctx),
        }
    }

    fn option_value(&self, key: &str) -> Option<usize> {
        KEYS.iter().position(|k| *k == key).map(|i| self.knobs[i])
    }

    fn set_option(&mut self, key: &str, value: usize) {
        let Some(i) = KEYS.iter().position(|k| *k == key) else {
            return;
        };
        let n = crate::options::find(key).map_or(1, |o| o.values.len());
        self.knobs[i] = value.min(n - 1);
        self.table = members_table(&self.members, &self.knobs, self.platform);
        self.load();
    }

    fn template(&self) -> Option<&Template> {
        Some(&self.template)
    }

    fn template_mut(&mut self) -> Option<&mut Template> {
        Some(&mut self.template)
    }

    fn as_any(&self) -> Option<&dyn std::any::Any> {
        Some(self)
    }

    fn layer_open(&self) -> bool {
        self.menu.hides_page_focus()
    }

    fn handle(&mut self, ev: &PageEvent, cx: &mut Cx) -> Outcome {
        if let Some(o) = self.template.handle(ev, cx) {
            return o;
        }
        if let Some((out, chosen)) = self.menu.handle(ev) {
            return match chosen {
                Some((cell, i)) => self.choose(cell, i, cx).or(out),
                None => out,
            };
        }
        if let Some(o) = self.sections.handle(ev) {
            return o;
        }
        match ev {
            PageEvent::Key(key) => {
                if key.ctrl_char('s') && !self.editing() {
                    self.do_apply(cx);
                    return Outcome::Changed;
                }
                let Some(f) = cx.focus.current() else {
                    return Outcome::Ignored;
                };
                if f == self.table.id {
                    if !self.table.is_editing()
                        && let Some(req) = self.menu.config.on_key(&self.table, key)
                    {
                        self.open_menu(req);
                        return Outcome::Changed;
                    }
                    if !self.table.is_editing()
                        && (key.is_char('d') || key.is(KeyCode::Delete))
                        && !self.table.selected_rows().is_empty()
                    {
                        self.ask_delete(cx);
                        return Outcome::Changed;
                    }
                    return self.table_input(cx, |t| t.on_key(key));
                }
                if f == self.name.id {
                    return Self::route_input(&mut self.name, key, cx);
                }
                if f == self.email.id {
                    return Self::route_input(&mut self.email, key, cx);
                }
                if f == self.notes.id {
                    return Self::route_input(&mut self.notes, key, cx);
                }
                for sel in self.selects() {
                    if sel.id == f {
                        return sel.on_key(key).0;
                    }
                }
                if f == self.on_call.id {
                    return self.on_call.on_key(key);
                }
                if f == self.apply.id {
                    let (out, act) = self.apply.on_key(key);
                    if act {
                        self.do_apply(cx);
                    }
                    return out;
                }
                if f == self.revert.id {
                    let (out, act) = self.revert.on_key(key);
                    if act {
                        self.do_revert(cx);
                    }
                    return out;
                }
                if f == self.delete.id {
                    let (out, act) = self.delete.on_key(key);
                    if act {
                        self.ask_delete(cx);
                    }
                    return out;
                }
                Outcome::Ignored
            }
            PageEvent::Paste(text) => {
                for input in [&mut self.name, &mut self.email, &mut self.notes] {
                    if input.editing {
                        return input.on_paste(text);
                    }
                }
                self.table.on_paste(text)
            }
            PageEvent::Pointer(p) => self.pointer(p, cx),
            PageEvent::DialogClosed { id, result, .. } if *id == DISCARD => {
                let Some((sel, cursor)) = self.pending.take() else {
                    return Outcome::Changed;
                };
                let go = |page: &mut Self| {
                    page.table.selection = sel;
                    page.table.cursor_row = cursor;
                    page.selection_changed();
                };
                match result {
                    DialogResult::Action(1) => {
                        go(self);
                        cx.status("Changes discarded");
                    }

                    DialogResult::Action(2) => {
                        self.do_apply(cx);
                        if !self.dirty() {
                            go(self);
                        }
                    }
                    _ => {}
                }
                Outcome::Changed
            }
            PageEvent::DialogClosed { id, result, .. } if *id == DELETE => {
                if *result == DialogResult::Action(1) {
                    self.do_delete(cx);
                } else {
                    cx.status("Nothing deleted");
                }
                Outcome::Changed
            }
            _ => Outcome::Ignored,
        }
    }

    fn leave_stakes(&self) -> junie_tui::ui::leave::Stakes {
        if self.dirty() {
            junie_tui::ui::leave::Stakes::Unsaved { changes: 0 }
        } else {
            junie_tui::ui::leave::Stakes::Clean
        }
    }

    fn editing(&self) -> bool {
        self.name.editing || self.email.editing || self.notes.editing || self.table.is_editing()
    }

    fn hints(&self, focus: Option<WidgetId>) -> Vec<Hint> {
        if let Some(h) = self.template.hints(focus) {
            return h;
        }
        if self.editing() {
            return vec![("Enter", "Apply"), ("Esc", "Cancel"), ("Tab", "Next field")];
        }
        if self.menu.is_open() {
            return MENU_HINTS.to_vec();
        }
        let selected = !self.table.selected_rows().is_empty();
        let save = ("Ctrl+S", "Apply changes");
        match focus {
            Some(f) if f == self.table.id => {
                let mut h = vec![
                    ("Space", "Select"),
                    ("Shift+↑↓", "Extend"),
                    ("Ctrl+A", "Select all"),
                ];
                if selected {
                    if let Some(click) =
                        keyhint::modifier_click(self.table.toggle_mods, self.platform)
                    {
                        h.insert(2, (click, "Toggle row"));
                    }
                    h.push(junie_tui::widgets::widget_hints::EDIT_PAIR);
                    h.push(("d", "Delete…"));

                    if self.table.esc_clears {
                        h.push(("Esc", "Clear selection"));
                    }
                } else {
                    h.push(("↑↓", "Move"));
                    h.push(("s", "Sort column"));
                }
                if self.dirty() {
                    h.push(save);
                }
                h.push(OPEN_HINT);
                h
            }
            Some(f) if f == self.name.id || f == self.email.id || f == self.notes.id => {
                vec![
                    junie_tui::widgets::widget_hints::EDIT_PAIR,
                    ("Tab", "Next field"),
                    save,
                ]
            }
            Some(f) if f == self.role.id || f == self.status.id || f == self.region.id => {
                vec![("Enter", "Open"), ("↑↓", "Choose"), save]
            }
            Some(f) if f == self.on_call.id => vec![("Space", "Toggle on call"), save],
            Some(f) if f == self.apply.id => vec![("Enter", "Apply"), ("Tab", "Next")],
            Some(f) if f == self.revert.id => vec![("Enter", "Revert"), ("Tab", "Next")],
            Some(f) if f == self.delete.id => vec![("Enter", "Delete…"), ("Tab", "Next")],
            _ => vec![("Tab", "Members table")],
        }
    }
}
