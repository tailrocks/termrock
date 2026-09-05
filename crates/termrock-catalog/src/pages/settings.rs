// SPDX-FileCopyrightText: 2026 Alexey Zhokhov
// SPDX-License-Identifier: Apache-2.0
//
// Adapted from junie-tui src/bin/showcase/pages/settings.rs (MIT),
// https://github.com/donbeave/terminal-components-claude

//! Composed screen: project settings with tabs, a form, an editable
//! members table with a destructive dialog, and an environment list with a
//! prompt dialog.

use ratatui::buffer::Buffer;
use ratatui::layout::{Position, Rect};
use ratatui::style::{Color, Modifier};
use ratatui::text::{Line, Text};
use ratatui::widgets::StatefulWidget;
use termrock::input::{
    Event, KeyCode, KeyEventKind, KeyModifiers, MouseButton, MouseEvent, MouseEventKind,
};
use termrock::widgets::{
    Action, ActionVariant, ActivationOutcome, ButtonState, ButtonVariant, ColumnKind, ColumnModel,
    DataColumn, DataColumnWidth, DataTable, DataTableNavMode, DataTableOutcome, DataTableState,
    Dialog, DialogFocusZone, DialogOutcome, DialogState, List, ListClickPolicy, ListRow,
    ListSelectionMode, ListState, LoadState, RadioGroup, RadioOption, RadioOutcome, RadioState,
    SortSpec, Tab, Tabs, TabsOutcome, TabsState, TextArea, TextAreaOutcome, TextAreaState,
    TextInput, TextInputOutcome, TextInputState, Toggle, ToggleOutcome, ToggleState, ToggleValue,
    Validation,
};

use crate::ctx::RenderCtx;
use crate::id::WidgetId;
use crate::layout;
use crate::outcome::Route;
use crate::page::{Hint, Page, PageCtx, PageEvent};
use crate::tablepro::paint;
use crate::text;

const ID: WidgetId = WidgetId::of("settings");
const TABS: WidgetId = ID.sub("tabs");
const NAME: WidgetId = ID.sub("name");
const DESC: WidgetId = ID.sub("desc");
const VIS: WidgetId = ID.sub("vis");
const AUTO: WidgetId = ID.sub("automerge");
const PROTECT: WidgetId = ID.sub("protect");
const SAVE: WidgetId = ID.sub("save");
const MEMBERS: WidgetId = ID.sub("members");
const REMOVE: WidgetId = ID.sub("remove");
const INVITE: WidgetId = ID.sub("invite");
const ENV: WidgetId = ID.sub("env");
const ADD_VAR: WidgetId = ID.sub("addvar");
const RM_VARS: WidgetId = ID.sub("rmvars");
const ADD_NAME: WidgetId = ID.sub("add-name");

const VIS_LABELS: [&str; 3] = ["Private", "Internal", "Public"];
const MEMBER_COLS: [&str; 4] = ["name", "email", "role", "last"];

#[derive(Clone, Copy, PartialEq, Eq)]
enum Overlay {
    None,
    Remove,
    AddVar,
    Save,
}

struct Member {
    name: String,
    email: String,
    role: String,
    last: String,
}

struct EnvVar {
    key: String,
    meta: String,
}

fn role_error(col: &str, s: &str) -> Option<String> {
    match col {
        "role" if !matches!(s, "Owner" | "Admin" | "Member" | "Viewer") => {
            Some("Role: Owner, Admin, Member or Viewer".into())
        }
        "name" if s.trim().is_empty() => Some("Name required".into()),
        _ => None,
    }
}

fn var_error(s: &str) -> Option<&'static str> {
    if s.is_empty() {
        Some("Required")
    } else if !s
        .chars()
        .all(|c| c.is_ascii_uppercase() || c == '_' || c.is_ascii_digit())
    {
        Some("Use UPPER_SNAKE_CASE")
    } else {
        None
    }
}

fn mouse_down(pos: ratatui::layout::Position) -> MouseEvent {
    MouseEvent {
        kind: MouseEventKind::Down(MouseButton::Left),
        position: pos,
        modifiers: KeyModifiers::NONE,
    }
}

fn vis_opts() -> [RadioOption<'static, u8>; 3] {
    [
        RadioOption::new(0, VIS_LABELS[0]),
        RadioOption::new(1, VIS_LABELS[1]),
        RadioOption::new(2, VIS_LABELS[2]),
    ]
}

fn tab_defs() -> [Tab<'static, u8>; 3] {
    [
        Tab::new(0, "General"),
        Tab::new(1, "Members"),
        Tab::new(2, "Environment"),
    ]
}

fn env_rows(env: &[EnvVar]) -> Vec<ListRow<'_, usize>> {
    env.iter()
        .enumerate()
        .map(|(i, v)| {
            ListRow::item(i, Line::from(v.key.as_str())).secondary(Line::from(v.meta.as_str()))
        })
        .collect()
}

fn dialog_actions(ok: &'static str, danger: bool) -> [Action<'static, &'static str>; 2] {
    [
        Action {
            id: "cancel",
            label: "Cancel",
            enabled: true,
            variant: ActionVariant::Secondary,
        },
        Action {
            id: "ok",
            label: ok,
            enabled: true,
            variant: if danger {
                ActionVariant::Destructive
            } else {
                ActionVariant::Primary
            },
        },
    ]
}

pub struct SettingsPage {
    tabs: TabsState<u8>,
    name: TextInputState,
    description: TextAreaState,
    visibility: RadioState<u8>,
    auto_merge: ToggleState,
    protect: ToggleState,
    save: ButtonState,
    dirty: bool,
    members: Vec<Member>,
    member_cols: ColumnModel<&'static str>,
    members_state: DataTableState<usize, &'static str>,
    member_error: Option<String>,
    remove: ButtonState,
    invite: ButtonState,
    env: Vec<EnvVar>,
    env_state: ListState<usize>,
    add_var: ButtonState,
    remove_vars: ButtonState,
    overlay: Overlay,
    overlay_body: String,
    dialog: DialogState<&'static str>,
    add_name: TextInputState,
}

impl SettingsPage {
    #[must_use]
    pub fn new() -> Self {
        let members = [
            ("Mira Okafor", "mira@acme.dev", "Owner", "today"),
            ("Jonas Weber", "jonas@acme.dev", "Admin", "2 h ago"),
            ("Ana Costa", "ana@acme.dev", "Member", "yesterday"),
            ("Kai Tanaka", "kai@acme.dev", "Member", "3 d ago"),
            ("Sofia Rossi", "sofia@acme.dev", "Viewer", "never"),
            ("deploy-bot", "bot@acme.dev", "Member", "1 m ago"),
        ]
        .into_iter()
        .map(|(n, e, r, l)| Member {
            name: n.into(),
            email: e.into(),
            role: r.into(),
            last: l.into(),
        })
        .collect::<Vec<_>>();
        let env = [
            ("DATABASE_URL", "postgres://…"),
            ("REDIS_URL", "redis://…"),
            ("STRIPE_KEY", "sk_live_…"),
            ("LOG_LEVEL", "info"),
            ("FEATURE_FLAGS", "beta,otel"),
        ]
        .into_iter()
        .map(|(k, v)| EnvVar {
            key: k.into(),
            meta: v.into(),
        })
        .collect();
        let mut tabs = TabsState::new();
        tabs.set_selected(Some(0));
        let mut name = TextInputState::new("payments-gateway").with_allow_empty(false);
        name.set_editing(false);
        let mut description =
            TextAreaState::new("Handles checkout, invoicing and refunds for the storefront.");
        description.set_accepts_input(false);
        description.set_editing(false);
        let mut env_state = ListState::new(Some(0));
        env_state.set_selection_mode(ListSelectionMode::Multi);
        env_state.set_click_policy(ListClickPolicy::Select);
        let mut add_name = TextInputState::new("").with_allow_empty(false);
        add_name.set_editing(false);
        let member_cols = ColumnModel::new(vec![
            DataColumn::new("name", "Name", DataColumnWidth::Fixed(42))
                .editable()
                .sortable(),
            DataColumn::new("email", "Email", DataColumnWidth::Fixed(18)).sortable(),
            DataColumn::new("role", "Role", DataColumnWidth::Fixed(8))
                .editable()
                .sortable(),
            DataColumn::new("last", "Last active", DataColumnWidth::Fixed(11))
                .kind(ColumnKind::Numeric),
        ]);
        let n = members.len();
        let mut members_state = DataTableState::new();
        members_state.nav_mode = DataTableNavMode::Cell;
        members_state.striped = false;
        members_state.set_logical_rows(n as u64);
        members_state.load = LoadState::Ready { count: n as u64 };
        Self {
            tabs,
            name,
            description,
            visibility: RadioState::new(Some(0)),
            auto_merge: ToggleState::with_value(ToggleValue::from_pressed(false)),
            protect: ToggleState::with_value(ToggleValue::from_pressed(true)),
            save: ButtonState::new(),
            dirty: false,
            members,
            member_cols,
            members_state,
            member_error: None,
            remove: ButtonState::new(),
            invite: ButtonState::new(),
            env,
            env_state,
            add_var: ButtonState::new(),
            remove_vars: ButtonState::new(),
            overlay: Overlay::None,
            overlay_body: String::new(),
            dialog: DialogState::new(),
            add_name,
        }
    }

    fn selected_member(&self) -> Option<usize> {
        if self.members.is_empty() {
            None
        } else {
            Some(self.members_state.cursor_row.min(self.members.len() - 1))
        }
    }

    fn member_ids(&self) -> Vec<usize> {
        (0..self.members.len()).collect()
    }

    fn member_cell(&self, i: usize, col: &str) -> &str {
        let m = &self.members[i];
        match col {
            "name" => m.name.as_str(),
            "email" => m.email.as_str(),
            "role" => m.role.as_str(),
            "last" => m.last.as_str(),
            _ => "",
        }
    }

    fn begin_member_edit(&mut self) {
        let Some(i) = self.selected_member() else {
            return;
        };
        let col = MEMBER_COLS[self.members_state.cursor_col.min(3)];
        if !matches!(col, "name" | "role") {
            return;
        }
        self.members_state.editing = true;
        self.members_state.edit_draft = self.member_cell(i, col).to_owned();
        self.member_error = role_error(col, &self.members_state.edit_draft);
    }

    fn apply_member_edit(&mut self, i: usize, col: &'static str, text: String) -> Option<String> {
        if let Some(err) = role_error(col, &text) {
            self.member_error = Some(err.clone());
            return Some(err);
        }
        self.member_error = None;
        if let Some(m) = self.members.get_mut(i) {
            match col {
                "name" => m.name = text,
                "role" => m.role = text,
                _ => {}
            }
        }
        None
    }

    fn open_remove(&mut self) {
        let Some(i) = self.selected_member() else {
            return;
        };
        let name = self.members[i].name.clone();
        let role = self.members[i].role.clone();
        self.overlay_body =
            format!("{name} ({role}) will lose access to every task in this project immediately.");
        self.dialog = DialogState::destructive("ok", "cancel");
        self.overlay = Overlay::Remove;
    }

    fn open_add(&mut self) {
        self.add_name = TextInputState::new("").with_allow_empty(false);
        self.add_name.set_editing(true);
        self.overlay_body.clear();
        self.dialog = DialogState::prompt("ok", "cancel");
        self.overlay = Overlay::AddVar;
    }

    fn open_save(&mut self) {
        self.overlay_body =
            "Changing visibility or branch protection applies to every open task.".into();
        self.dialog = DialogState::confirm("ok", "cancel");
        self.overlay = Overlay::Save;
    }

    fn apply_ok(&mut self, cx: &mut PageCtx<'_>) {
        match self.overlay {
            Overlay::Remove => {
                if let Some(i) = self.selected_member() {
                    let name = self.members[i].name.clone();
                    self.members.remove(i);
                    let n = self.members.len();
                    self.members_state = DataTableState::new();
                    self.members_state.nav_mode = DataTableNavMode::Cell;
                    self.members_state.striped = false;
                    self.members_state.set_logical_rows(n as u64);
                    self.members_state.load = LoadState::Ready { count: n as u64 };
                    if n > 0 {
                        self.members_state.cursor_row = i.min(n - 1);
                    }
                    cx.status(format!("Removed {name}"));
                }
            }
            Overlay::AddVar => {
                let name = self.add_name.trimmed_value().to_owned();
                if var_error(&name).is_none() {
                    self.env.push(EnvVar {
                        key: name.clone(),
                        meta: "(empty)".into(),
                    });
                    cx.status(format!("Added {name}"));
                }
            }
            Overlay::Save => {
                self.dirty = false;
                cx.status("Settings saved ✓");
            }
            Overlay::None => {}
        }
        self.overlay = Overlay::None;
        self.add_name.set_editing(false);
    }

    fn cancel_overlay(&mut self, cx: &mut PageCtx<'_>) {
        if self.overlay == Overlay::Remove {
            cx.status("Kept member");
        }
        self.overlay = Overlay::None;
        self.add_name.set_editing(false);
    }

    fn remove_selected_vars(&mut self, cx: &mut PageCtx<'_>) {
        let checked: Vec<usize> = self
            .env_state
            .selection()
            .map(|s| s.checked().to_vec())
            .unwrap_or_default();
        if checked.is_empty() {
            return;
        }
        let before = self.env.len();
        self.env = self
            .env
            .iter()
            .enumerate()
            .filter(|(i, _)| !checked.contains(i))
            .map(|(_, v)| EnvVar {
                key: v.key.clone(),
                meta: v.meta.clone(),
            })
            .collect();
        let n = self.env.len();
        self.env_state = ListState::new(if n == 0 { None } else { Some(0) });
        self.env_state.set_selection_mode(ListSelectionMode::Multi);
        self.env_state.set_click_policy(ListClickPolicy::Select);
        cx.status(format!("Removed {} variables", before - n));
    }

    fn on_members(
        &mut self,
        ev: DataTableOutcome<usize, &'static str>,
        cx: &mut PageCtx<'_>,
    ) -> Route {
        match ev {
            DataTableOutcome::Ignored => Route::Ignored,
            DataTableOutcome::EditStarted { .. } => {
                if self.members_state.edit_draft.is_empty() {
                    self.begin_member_edit();
                }
                Route::Changed
            }
            DataTableOutcome::EditCommitted { row, column, text } => {
                if self.apply_member_edit(row, column, text).is_none() {
                    cx.status("Member updated");
                }
                Route::Changed
            }
            DataTableOutcome::EditCancelled => {
                self.member_error = None;
                Route::Changed
            }
            DataTableOutcome::Activate(_) => {
                self.begin_member_edit();
                Route::Changed
            }
            DataTableOutcome::SortRequested(col)
            | DataTableOutcome::SortSpec(SortSpec { column: col, .. }) => {
                let asc = self
                    .members_state
                    .sort
                    .as_ref()
                    .map(|s| s.ascending)
                    .unwrap_or(true);
                self.members.sort_by(|a, b| {
                    let ord = match col {
                        "name" => a.name.cmp(&b.name),
                        "email" => a.email.cmp(&b.email),
                        "role" => a.role.cmp(&b.role),
                        "last" => a.last.cmp(&b.last),
                        _ => a.name.cmp(&b.name),
                    };
                    if asc { ord } else { ord.reverse() }
                });
                self.members_state.sort = Some(termrock::widgets::SortSpec {
                    column: col,
                    ascending: !self
                        .members_state
                        .sort
                        .as_ref()
                        .is_some_and(|s| s.column == col && s.ascending),
                });
                Route::Changed
            }
            DataTableOutcome::CursorMoved
            | DataTableOutcome::Scrolled
            | DataTableOutcome::HoverChanged
            | DataTableOutcome::SelectionChanged
            | DataTableOutcome::ToggleRow(_) => Route::Changed,
            _ => Route::Changed,
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

    fn render(&mut self, area: Rect, buf: &mut Buffer, ctx: &mut RenderCtx<'_>) {
        let overlay = self.overlay;
        let saved_inert = ctx.inert;
        ctx.inert = saved_inert || overlay != Overlay::None;

        let tabs = tab_defs();
        self.tabs.set_focused(ctx.interaction.focused(TABS));
        Tabs::new(&tabs, ctx.system).paint(
            Rect::new(area.x, area.y, area.width, 2),
            buf,
            &mut self.tabs,
        );
        ctx.control(TABS, Rect::new(area.x, area.y, area.width, 2), false);

        let body = Rect::new(
            area.x,
            area.y + 3,
            area.width,
            area.height.saturating_sub(3),
        );
        match self.tabs.selected {
            Some(1) => self.render_members(body, buf, ctx),
            Some(2) => self.render_env(body, buf, ctx),
            _ => self.render_general(body, buf, ctx),
        }

        ctx.inert = saved_inert;
        if overlay != Overlay::None {
            self.render_overlay(area, buf, ctx);
        }
    }

    fn handle(&mut self, ev: &PageEvent, cx: &mut PageCtx<'_>) -> Route {
        if self.overlay != Overlay::None {
            return self.handle_overlay(ev, cx);
        }
        match ev {
            PageEvent::Key(key) if key.kind != KeyEventKind::Release => {
                let tabs = tab_defs();
                if key.modifiers.contains(KeyModifiers::CONTROL)
                    && matches!(key.code, KeyCode::Char('s') | KeyCode::Char('S'))
                    && self.tabs.selected == Some(0)
                {
                    self.dirty = false;
                    cx.status("Settings saved ✓");
                    return Route::Changed;
                }
                if matches!(key.code, KeyCode::Tab | KeyCode::BackTab) {
                    if self.name.is_editing() {
                        self.name.commit();
                    }
                    if self.description.is_editing() {
                        self.description.set_editing(false);
                    }
                    if self.members_state.editing {
                        let col = MEMBER_COLS[self.members_state.cursor_col.min(3)];
                        if let Some(i) = self.selected_member() {
                            let text = self.members_state.edit_draft.clone();
                            if self.apply_member_edit(i, col, text).is_none() {
                                self.members_state.editing = false;
                                self.members_state.edit_draft.clear();
                                cx.status("Member updated");
                            }
                            return Route::Changed;
                        }
                    }
                    return Route::Ignored;
                }
                let Some(f) = cx.focus_id() else {
                    return Route::Ignored;
                };
                if f == TABS {
                    return match self.tabs.handle_key(*key, &tabs) {
                        TabsOutcome::Ignored => Route::Ignored,
                        _ => Route::Changed,
                    };
                }
                if f == NAME {
                    return match self.name.handle_key(*key) {
                        TextInputOutcome::Ignored => Route::Ignored,
                        TextInputOutcome::Changed | TextInputOutcome::Submitted(_) => {
                            self.dirty = true;
                            Route::Changed
                        }
                        _ => Route::Changed,
                    };
                }
                if f == DESC {
                    self.description.set_accepts_input(true);
                    return match self.description.handle_key(*key) {
                        TextAreaOutcome::Ignored => Route::Ignored,
                        TextAreaOutcome::Changed => {
                            self.dirty = true;
                            Route::Changed
                        }
                        _ => Route::Changed,
                    };
                }
                if f == VIS {
                    let vis = vis_opts();
                    let system = termrock::style::DesignSystem::junie();
                    let group = RadioGroup::new(&vis, &system);
                    return match group.handle_key(&mut self.visibility, *key) {
                        RadioOutcome::Ignored => Route::Ignored,
                        _ => {
                            self.dirty = true;
                            Route::Changed
                        }
                    };
                }
                if f == AUTO {
                    self.auto_merge.set_focused(true);
                    let system = termrock::style::DesignSystem::junie();
                    return match Toggle::new("Auto-merge approved PRs", &system)
                        .handle_key(&mut self.auto_merge, *key)
                    {
                        ToggleOutcome::Ignored => Route::Ignored,
                        ToggleOutcome::ValueChanged { .. } => {
                            self.dirty = true;
                            Route::Changed
                        }
                        _ => Route::Changed,
                    };
                }
                if f == PROTECT {
                    self.protect.set_focused(true);
                    let system = termrock::style::DesignSystem::junie();
                    return match Toggle::new("Protect main branch", &system)
                        .handle_key(&mut self.protect, *key)
                    {
                        ToggleOutcome::Ignored => Route::Ignored,
                        ToggleOutcome::ValueChanged { .. } => {
                            self.dirty = true;
                            Route::Changed
                        }
                        _ => Route::Changed,
                    };
                }
                if f == SAVE {
                    self.save.activation.set_accepts_input(true);
                    return match self.save.handle_key(*key) {
                        ActivationOutcome::Activated => {
                            if self.dirty {
                                self.open_save();
                            } else {
                                cx.status("Nothing to save");
                            }
                            Route::Changed
                        }
                        ActivationOutcome::Ignored => Route::Ignored,
                        _ => Route::Changed,
                    };
                }
                if f == MEMBERS {
                    if self.members_state.editing
                        && matches!(key.code, KeyCode::Tab | KeyCode::BackTab)
                    {
                        return Route::Changed;
                    }
                    let ids = self.member_ids();
                    let ev = self.members_state.handle_key(*key, &ids, &self.member_cols);
                    return self.on_members(ev, cx);
                }
                if f == REMOVE {
                    self.remove
                        .activation
                        .set_accepts_input(self.selected_member().is_some());
                    return match self.remove.handle_key(*key) {
                        ActivationOutcome::Activated => {
                            self.open_remove();
                            Route::Changed
                        }
                        ActivationOutcome::Ignored => Route::Ignored,
                        _ => Route::Changed,
                    };
                }
                if f == INVITE {
                    self.invite.activation.set_accepts_input(true);
                    return match self.invite.handle_key(*key) {
                        ActivationOutcome::Activated => {
                            cx.status("Invitations are sent from the web console");
                            Route::Changed
                        }
                        ActivationOutcome::Ignored => Route::Ignored,
                        _ => Route::Changed,
                    };
                }
                if f == ENV {
                    let rows = env_rows(&self.env);
                    return match self.env_state.handle_key(&rows, *key) {
                        termrock::widgets::Outcome::Ignored => Route::Ignored,
                        _ => Route::Changed,
                    };
                }
                if f == ADD_VAR {
                    self.add_var.activation.set_accepts_input(true);
                    return match self.add_var.handle_key(*key) {
                        ActivationOutcome::Activated => {
                            self.open_add();
                            Route::Changed
                        }
                        ActivationOutcome::Ignored => Route::Ignored,
                        _ => Route::Changed,
                    };
                }
                if f == RM_VARS {
                    self.remove_vars.activation.set_accepts_input(true);
                    return match self.remove_vars.handle_key(*key) {
                        ActivationOutcome::Activated => {
                            self.remove_selected_vars(cx);
                            Route::Changed
                        }
                        ActivationOutcome::Ignored => Route::Ignored,
                        _ => Route::Changed,
                    };
                }
                Route::Ignored
            }
            PageEvent::Paste(text) => {
                if self.name.is_editing() {
                    let _ = self.name.insert_str(text);
                    self.dirty = true;
                    return Route::Changed;
                }
                if self.description.is_editing() {
                    let _ = self.description.insert_text(text);
                    self.dirty = true;
                    return Route::Changed;
                }
                if self.members_state.editing {
                    self.members_state.edit_draft.push_str(text);
                    return Route::Changed;
                }
                Route::Ignored
            }
            PageEvent::Click { id, pos } => {
                if *id == TABS {
                    let tabs = tab_defs();
                    let _ = self.tabs.handle_mouse(mouse_down(*pos), &tabs);
                    cx.set_focus(TABS);
                    return Route::Changed;
                }
                if *id == NAME {
                    cx.set_focus(NAME);
                    self.name.set_focused(true);
                    if let Some(parts) = self.name.parts().cloned() {
                        let _ = self.name.handle_mouse(mouse_down(*pos), parts.field);
                    }
                    return Route::Changed;
                }
                if *id == DESC {
                    let was = cx.focus_id() == Some(DESC);
                    cx.set_focus(DESC);
                    self.description.set_accepts_input(true);
                    if was && !self.description.is_editing() {
                        self.description.set_editing(true);
                    }
                    let _ = self
                        .description
                        .handle_event(Event::Mouse(mouse_down(*pos)));
                    return Route::Changed;
                }
                if *id == VIS {
                    cx.set_focus(VIS);
                    self.visibility.set_surface_focused(true);
                    let vis = vis_opts();
                    let sys = termrock::style::DesignSystem::junie();
                    let _ = RadioGroup::new(&vis, &sys)
                        .handle_mouse(&mut self.visibility, mouse_down(*pos));
                    self.dirty = true;
                    return Route::Changed;
                }
                if *id == AUTO {
                    cx.set_focus(AUTO);
                    self.auto_merge.set_focused(true);
                    if self.auto_merge.enabled {
                        self.auto_merge.set_value(self.auto_merge.value.activate());
                        self.dirty = true;
                    }
                    return Route::Changed;
                }
                if *id == PROTECT {
                    cx.set_focus(PROTECT);
                    self.protect.set_focused(true);
                    if self.protect.enabled {
                        self.protect.set_value(self.protect.value.activate());
                        self.dirty = true;
                    }
                    return Route::Changed;
                }
                if *id == SAVE {
                    if self.dirty {
                        self.open_save();
                    } else {
                        cx.status("Nothing to save");
                    }
                    return Route::Changed;
                }
                if *id == MEMBERS {
                    cx.set_focus(MEMBERS);
                    let ids = self.member_ids();
                    let was_row = self.members_state.cursor_row;
                    let was_col = self.members_state.cursor_col;
                    let ev = self.members_state.handle_mouse(
                        mouse_down(*pos),
                        &ids,
                        &mut self.member_cols,
                    );
                    if matches!(ev, DataTableOutcome::CursorMoved)
                        && self.members_state.cursor_row == was_row
                        && self.members_state.cursor_col == was_col
                    {
                        self.begin_member_edit();
                        return Route::Changed;
                    }
                    return self.on_members(ev, cx);
                }
                if *id == REMOVE && self.selected_member().is_some() {
                    self.open_remove();
                    return Route::Changed;
                }
                if *id == INVITE {
                    cx.status("Invitations are sent from the web console");
                    return Route::Changed;
                }
                if *id == ENV {
                    cx.set_focus(ENV);
                    let _ = self.env_state.click(*pos);
                    return Route::Changed;
                }
                if *id == ADD_VAR {
                    self.open_add();
                    return Route::Changed;
                }
                if *id == RM_VARS {
                    self.remove_selected_vars(cx);
                    return Route::Changed;
                }
                Route::Ignored
            }
            PageEvent::Wheel { id, delta } => {
                if *id == MEMBERS {
                    let ids = self.member_ids();
                    let ev = self.members_state.handle_mouse(
                        MouseEvent {
                            kind: if *delta < 0 {
                                MouseEventKind::ScrollUp
                            } else {
                                MouseEventKind::ScrollDown
                            },
                            position: ratatui::layout::Position::default(),
                            modifiers: KeyModifiers::NONE,
                        },
                        &ids,
                        &mut self.member_cols,
                    );
                    return self.on_members(ev, cx);
                }
                if *id == ENV {
                    let n = self.env.len();
                    let _ = self.env_state.scroll_by(*delta as isize, n);
                    return Route::Changed;
                }
                if *id == DESC {
                    let _ = self.description.scroll_by(0, *delta as isize);
                    return Route::Changed;
                }
                Route::Ignored
            }
            _ => Route::Ignored,
        }
    }

    fn editing(&self) -> bool {
        self.name.is_editing()
            || self.description.is_editing()
            || self.members_state.editing
            || (self.overlay == Overlay::AddVar && self.add_name.is_editing())
    }

    fn hints(&self, focus: Option<WidgetId>) -> Vec<Hint> {
        if self.members_state.editing {
            return vec![("Enter", "Commit"), ("Esc", "Cancel"), ("Tab", "Next cell")];
        }
        if self.name.is_editing() || self.description.is_editing() {
            return vec![("Enter", "Commit"), ("Esc", "Cancel"), ("Tab", "Next")];
        }
        match focus {
            Some(f) if f == TABS => vec![("← →", "Switch tab"), ("1 2 3", "Jump")],
            Some(f) if f == MEMBERS => {
                vec![("↑ ↓ ← →", "Cell"), ("Enter", "Edit"), ("s", "Sort")]
            }
            Some(f) if f == ENV => vec![("Space", "Toggle"), ("a", "All")],
            _ => vec![("Enter", "Edit / activate"), ("Ctrl+S", "Save")],
        }
    }
}

impl SettingsPage {
    fn render_general(&mut self, body: Rect, buf: &mut Buffer, ctx: &mut RenderCtx<'_>) {
        let t = ctx.theme;
        let title = if self.dirty {
            "General · unsaved"
        } else {
            "General"
        };
        let card = Rect::new(body.x, body.y, body.width, body.height.min(16));
        let (inner, bg) = layout::card(card, buf, t, Some(title), None, false);
        let (l, r) = layout::columns(inner, inner.width / 2 - 2, 4);

        self.name.set_focused(ctx.interaction.focused(NAME));
        self.name.set_hovered(ctx.interaction.hovered(NAME));
        let name_r = Rect::new(l.x, l.y, l.width, 3);
        let parts = TextInput::new("Project name", ctx.system)
            .required(true)
            .paint(name_r, buf, &mut self.name);
        ctx.control(NAME, name_r, false);
        if let Some(c) = parts.cursor {
            ctx.set_cursor(c.as_position());
        }

        let desc_focus = ctx.interaction.focused(DESC);
        self.description.set_accepts_input(desc_focus);
        if !desc_focus {
            self.description.set_editing(false);
        }
        let desc_r = Rect::new(l.x, l.y + 3, l.width, 5);
        StatefulWidget::render(
            &TextArea::new(ctx.system).title("Description").rows(3),
            desc_r,
            buf,
            &mut self.description,
        );
        ctx.control(DESC, desc_r, false);
        ctx.scrollable(DESC, desc_r);
        if let Some(c) = self.description.cursor_cell() {
            ctx.set_cursor(c);
        }

        let vis = vis_opts();
        self.visibility
            .set_surface_focused(ctx.interaction.focused(VIS));
        let vis_h = 4.min(r.height);
        let vis_r = Rect::new(r.x, r.y, r.width, vis_h);
        RadioGroup::new(&vis, ctx.system)
            .legend("Visibility")
            .paint(vis_r, buf, &mut self.visibility);
        ctx.control(VIS, vis_r, false);

        self.auto_merge.set_focused(ctx.interaction.focused(AUTO));
        self.auto_merge.hovered = ctx.interaction.hovered(AUTO);
        let sw1 = Rect::new(r.x, r.y + 5, r.width, 1);
        Toggle::new("Auto-merge approved PRs", ctx.system).paint(sw1, buf, &mut self.auto_merge);
        ctx.control(AUTO, sw1, false);

        self.protect.set_focused(ctx.interaction.focused(PROTECT));
        self.protect.hovered = ctx.interaction.hovered(PROTECT);
        let sw2 = Rect::new(r.x, r.y + 6, r.width, 1);
        Toggle::new("Protect main branch", ctx.system).paint(sw2, buf, &mut self.protect);
        ctx.control(PROTECT, sw2, false);

        let ay = inner.bottom().saturating_sub(1);
        let save_r = Rect::new(inner.x, ay, paint::button_width("Save changes"), 1);
        paint::button(
            "Save changes",
            ButtonVariant::Primary,
            SAVE,
            save_r,
            buf,
            ctx,
            &mut self.save,
            false,
            bg,
        );
        if !self.dirty {
            buf.set_string(save_r.right() + 2, ay, "No changes", t.faint().bg(bg));
        }
    }

    fn render_members(&mut self, body: Rect, buf: &mut Buffer, ctx: &mut RenderCtx<'_>) {
        let t = ctx.theme;
        let pos = {
            let view = usize::from(body.height.saturating_sub(8).max(1));
            let total = self.members.len();
            if total <= view {
                String::new()
            } else {
                let start = self.members_state.window.offset as usize + 1;
                let end = (start + view - 1).min(total);
                format!("{start}–{end} of {total}")
            }
        };
        let meta = match &self.member_error {
            Some(e) => e.clone(),
            None if pos.is_empty() => format!("{} members", self.members.len()),
            None => format!("{} members · {pos}", self.members.len()),
        };
        let th = (self.members.len() as u16 + 1).max(2);
        let card = Rect::new(body.x, body.y, body.width, (th + 5).min(body.height));
        let (inner, bg) = layout::card(
            card,
            buf,
            t,
            Some("Members"),
            Some(&meta),
            ctx.interaction.focused(MEMBERS),
        );
        let owned: Vec<(usize, Vec<String>)> = self
            .members
            .iter()
            .enumerate()
            .map(|(i, m)| {
                (
                    i,
                    vec![
                        m.name.clone(),
                        m.email.clone(),
                        m.role.clone(),
                        m.last.clone(),
                    ],
                )
            })
            .collect();
        let refs: Vec<(usize, Vec<&str>)> = owned
            .iter()
            .map(|(i, c)| (*i, c.iter().map(String::as_str).collect()))
            .collect();
        let projected: Vec<(usize, &[&str])> =
            refs.iter().map(|(i, c)| (*i, c.as_slice())).collect();
        let table_h = th.min(inner.height.saturating_sub(2));
        let table_r = Rect::new(inner.x, inner.y, inner.width, table_h);
        self.members_state
            .set_accepts_input(ctx.interaction.focused(MEMBERS));
        StatefulWidget::render(
            &DataTable::new(ctx.system, &self.member_cols, &projected)
                .focused(ctx.interaction.focused(MEMBERS))
                .row_numbers(false),
            table_r,
            buf,
            &mut self.members_state,
        );
        if self.members_state.editing {
            if let Some(fg) = ctx.theme.muted().fg {
                for region in &self.members_state.header_regions {
                    for x in region.area.x..=region.area.right() {
                        buf[(x, region.area.y)].set_fg(fg);
                    }
                }
            }
        }
        if let Some(fg) = ctx.theme.muted().fg {
            for region in &self.members_state.cell_regions {
                if matches!(region.column, "email" | "last") {
                    for x in region.area.x..region.area.right() {
                        buf[(x, region.area.y)].set_fg(fg);
                    }
                }
            }
        }
        if self.members_state.editing {
            let selected_row = self.members_state.cursor_row;
            if let Some(region) = self
                .members_state
                .cell_regions
                .iter()
                .find(|region| region.row == selected_row)
            {
                for x in table_r.x..table_r.right() {
                    let cell = &mut buf[(x, region.area.y)];
                    cell.set_style(cell.style().add_modifier(Modifier::BOLD));
                }
            }
        }
        let editing_role = self.members_state.editing
            && self.members_state.cursor_column_id(&self.member_cols) == Some("role");
        if editing_role {
            let cursor_row = self.members_state.cursor_row;
            let text_width = text::width(&self.members_state.edit_draft);
            if let Some(region) = self
                .members_state
                .cell_regions
                .iter()
                .find(|region| region.row == cursor_row && region.column == "role")
            {
                let text_end = region.area.left().saturating_add(
                    u16::try_from(text_width)
                        .unwrap_or(region.area.width)
                        .min(region.area.width),
                );
                for x in region.area.left()..region.area.right() {
                    let cell = &mut buf[(x, region.area.y)];
                    let mut style = cell
                        .style()
                        .fg(Color::Rgb(255, 255, 255))
                        .bg(Color::Rgb(30, 30, 34))
                        .add_modifier(Modifier::BOLD)
                        .remove_modifier(Modifier::UNDERLINED | Modifier::REVERSED);
                    if x < text_end {
                        style = style.add_modifier(Modifier::UNDERLINED);
                    }
                    cell.set_style(style);
                }
            }
        }
        if editing_role
            && let Some(i) = self.selected_member()
            && let Some(region) = self
                .members_state
                .cell_regions
                .iter()
                .find(|region| region.row == i && region.column == "role")
        {
            let draft_width = u16::try_from(text::width(&self.members_state.edit_draft))
                .unwrap_or(u16::MAX)
                .min(region.area.width.saturating_sub(1));
            let cursor_x = region.area.x.saturating_add(draft_width);
            ctx.set_cursor(Position::new(cursor_x, region.area.y));
        }
        ctx.control(MEMBERS, table_r, false);
        ctx.scrollable(MEMBERS, table_r);

        let ay = inner.bottom().saturating_sub(1);
        let rects = layout::row_layout(
            Rect::new(inner.x, ay, inner.width, 1),
            &[
                paint::button_width("Invite"),
                paint::button_width("Remove…"),
            ],
            2,
        );
        paint::button(
            "Invite",
            ButtonVariant::Secondary,
            INVITE,
            rects[0],
            buf,
            ctx,
            &mut self.invite,
            false,
            bg,
        );
        paint::button(
            "Remove…",
            ButtonVariant::Destructive,
            REMOVE,
            rects[1],
            buf,
            ctx,
            &mut self.remove,
            self.members.is_empty(),
            bg,
        );
        if let Some(i) = self.selected_member() {
            let s = format!("cursor: {}", self.members[i].name);
            buf.set_string(rects[1].right() + 2, ay, &s, t.faint().bg(bg));
        }
    }

    fn render_env(&mut self, body: Rect, buf: &mut Buffer, ctx: &mut RenderCtx<'_>) {
        let t = ctx.theme;
        let count = self
            .env_state
            .selection()
            .map(|s| s.checked().len())
            .unwrap_or(0);
        let meta = format!("{count} selected");
        let lh = (self.env.len() as u16).max(1);
        let card = Rect::new(body.x, body.y, body.width, (lh + 5).min(body.height));
        let (inner, bg) = layout::card(
            card,
            buf,
            t,
            Some("Environment variables"),
            Some(&meta),
            false,
        );
        let rows = env_rows(&self.env);
        let list_h = lh.min(inner.height.saturating_sub(2));
        let list_r = Rect::new(inner.x, inner.y, inner.width.min(60), list_h);
        StatefulWidget::render(
            &List::new(&rows, ctx.system)
                .focused(ctx.interaction.focused(ENV))
                .empty_message(Line::from("No variables defined")),
            list_r,
            buf,
            &mut self.env_state,
        );
        ctx.control(ENV, list_r, false);
        ctx.scrollable(ENV, list_r);

        let ay = inner.bottom().saturating_sub(1);
        let rects = layout::row_layout(
            Rect::new(inner.x, ay, inner.width, 1),
            &[
                paint::button_width("Add variable…"),
                paint::button_width("Remove selected"),
            ],
            2,
        );
        paint::button(
            "Add variable…",
            ButtonVariant::Primary,
            ADD_VAR,
            rects[0],
            buf,
            ctx,
            &mut self.add_var,
            false,
            bg,
        );
        paint::button(
            "Remove selected",
            ButtonVariant::Destructive,
            RM_VARS,
            rects[1],
            buf,
            ctx,
            &mut self.remove_vars,
            count == 0,
            bg,
        );
    }

    fn render_overlay(&mut self, area: Rect, buf: &mut Buffer, ctx: &mut RenderCtx<'_>) {
        self.dialog.set_open(true);
        self.dialog.set_accepts_input(true);
        match self.overlay {
            Overlay::Remove => {
                let actions = dialog_actions("Remove", true);
                let body = self.overlay_body.clone();
                Dialog::destructive("Remove member?", Text::from(body), ctx.system).paint_modal(
                    area,
                    buf,
                    &mut self.dialog,
                    &actions,
                );
            }
            Overlay::Save => {
                let actions = dialog_actions("Save", false);
                let body = self.overlay_body.clone();
                Dialog::confirm("Save settings?", Text::from(body), ctx.system).paint_modal(
                    area,
                    buf,
                    &mut self.dialog,
                    &actions,
                );
            }
            Overlay::AddVar => {
                let actions = dialog_actions("Add", false);
                let err = var_error(self.add_name.trimmed_value());
                self.dialog
                    .set_validation_message(err.map(|e| e.to_owned()));
                Dialog::prompt("Add variable", Text::from(""), ctx.system).paint_modal(
                    area,
                    buf,
                    &mut self.dialog,
                    &actions,
                );
                let field = self.dialog.slots().body;
                if !field.is_empty() {
                    self.add_name.set_focused(true);
                    let mut input = TextInput::new("Variable name", ctx.system)
                        .required(true)
                        .placeholder("API_BASE_URL");
                    if let Some(e) = err {
                        input = input.validation(Validation::Invalid(e));
                    }
                    let parts = input.paint(field, buf, &mut self.add_name);
                    ctx.control(ADD_NAME, field, false);
                    if let Some(c) = parts.cursor {
                        ctx.set_cursor(c.as_position());
                    }
                }
            }
            Overlay::None => {}
        }
        ctx.control(ID.sub("modal"), area, false);
    }

    fn handle_overlay(&mut self, ev: &PageEvent, cx: &mut PageCtx<'_>) -> Route {
        let (ok, danger) = match self.overlay {
            Overlay::Remove => ("Remove", true),
            Overlay::AddVar => ("Add", false),
            Overlay::Save => ("Save", false),
            Overlay::None => return Route::Ignored,
        };
        let actions = dialog_actions(ok, danger);
        match ev {
            PageEvent::Key(key) if key.kind != KeyEventKind::Release => {
                if self.overlay == Overlay::AddVar
                    && self.dialog.focus_zone() == DialogFocusZone::Body
                    && !matches!(key.code, KeyCode::Esc)
                {
                    if matches!(key.code, KeyCode::Enter) {
                        if var_error(self.add_name.trimmed_value()).is_some() {
                            return Route::Changed;
                        }
                        self.apply_ok(cx);
                        return Route::Changed;
                    }
                    return match self.add_name.handle_key(*key) {
                        TextInputOutcome::Ignored => Route::Consumed,
                        TextInputOutcome::Submitted(_) => {
                            if var_error(self.add_name.trimmed_value()).is_none() {
                                self.apply_ok(cx);
                            }
                            Route::Changed
                        }
                        _ => Route::Changed,
                    };
                }
                let out = self.dialog.handle_key(*key, &actions);
                self.apply_dialog(out, cx)
            }
            PageEvent::Click { pos, .. } => {
                let out = self.dialog.handle_click(*pos, &actions);
                self.apply_dialog(out, cx)
            }
            PageEvent::Paste(text) if self.overlay == Overlay::AddVar => {
                let _ = self.add_name.insert_str(text);
                Route::Changed
            }
            _ => Route::Consumed,
        }
    }

    fn apply_dialog(&mut self, out: DialogOutcome<&'static str>, cx: &mut PageCtx<'_>) -> Route {
        match out {
            DialogOutcome::Ignored | DialogOutcome::LoadingBlocked => Route::Consumed,
            DialogOutcome::Activated("ok") | DialogOutcome::DefaultActivated("ok") => {
                if self.overlay == Overlay::AddVar
                    && var_error(self.add_name.trimmed_value()).is_some()
                {
                    return Route::Changed;
                }
                self.apply_ok(cx);
                Route::Changed
            }
            DialogOutcome::Activated(_) | DialogOutcome::Cancelled => {
                self.cancel_overlay(cx);
                Route::Changed
            }
            DialogOutcome::ValidationFailed => Route::Changed,
            _ => Route::Changed,
        }
    }
}
