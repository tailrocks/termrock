use ratatui::buffer::Buffer;
use ratatui::crossterm::event::KeyCode;
use ratatui::layout::{Position, Rect};

use junie_tui::core::event::{Key, Outcome};
use junie_tui::core::hit::Axis;
use junie_tui::core::id::{ItemKey, Part, WidgetId};
use junie_tui::core::intent::Phase;
use junie_tui::core::pointer::{MouseTarget, PointerEvent};
use junie_tui::ui::ctx::RenderCtx;
use junie_tui::ui::layout::{Split, SplitDir};
use junie_tui::widgets::empty::EmptyState;
use junie_tui::widgets::field_common::{PointerField, route_click, route_drag, route_press};
use junie_tui::widgets::input::{InputAction, TextInput};
use junie_tui::widgets::keyhint::{Hint, hint};
use junie_tui::widgets::panel::Panel;
use junie_tui::widgets::splitter::Splitter;
use junie_tui::widgets::table::SortDir;
use junie_tui::widgets::tabs::{TabAction, TabItem, Tabs};
use junie_tui::widgets::tree::{TreeAction, TreeNode, TreeView};

use crate::app::Cx;
use crate::db::{Catalog, Connection, ObjectKind};
use crate::model::{History, HistoryEntry};
use crate::tabs::{HistoryCommand, HistoryTab, QueryTab, TableTab};

const ID: WidgetId = WidgetId::of("workbench");
pub const EXPLORER: WidgetId = ID.sub("explorer");

pub const EXPLORER_RAIL: WidgetId = ID.sub("explorer.rail");
pub const TABSTRIP: WidgetId = ID.sub("tabstrip");

#[allow(clippy::large_enum_variant)]
pub type PendingRun = (
    usize,
    Vec<(String, std::ops::Range<usize>)>,
    bool,
    Option<bool>,
);

#[allow(clippy::large_enum_variant)]
pub enum WorkTab {
    Table(TableTab),
    Query(QueryTab),
    History(HistoryTab),
}

impl WorkTab {
    pub fn label(&self) -> String {
        match self {
            WorkTab::Table(t) => t.label(),
            WorkTab::Query(q) => q.name.clone(),
            WorkTab::History(_) => "History".into(),
        }
    }
    pub fn is_editing(&self) -> bool {
        match self {
            WorkTab::Table(t) => t.is_editing(),
            WorkTab::Query(q) => q.is_editing(),
            WorkTab::History(h) => h.search.editing,
        }
    }
    pub fn dirty(&self) -> bool {
        match self {
            WorkTab::Table(t) => t.dirty_count() > 0,
            WorkTab::Query(q) => q.dirty(),
            WorkTab::History(_) => false,
        }
    }
}

pub struct Workbench {
    pub connection: Connection,
    /// The configured connection this session was opened from.
    pub connection_key: Option<junie_tui::core::id::ItemKey>,
    pub catalog: Catalog,
    pub schema: String,
    pub explorer: TreeView,
    pub explorer_filter: TextInput,
    pub explorer_visible: bool,

    pub rail: junie_tui::widgets::rail::SidebarRail,

    pub explorer_split: Split,
    pub explorer_sized: bool,
    pub explorer_seam: Splitter,
    explorer_split_area: Rect,
    pub maximized: bool,
    pub strip: Tabs,
    pub tabs: Vec<WorkTab>,
    pub active: usize,
    pub query_counter: usize,
    pending_loads: Vec<(Vec<usize>, u32)>,

    pub open_objects: Vec<(String, String)>,

    pub current_object: Option<(String, String)>,

    pub pending_run: Option<PendingRun>,
    pub closed: Vec<(String, String)>,
}

fn kind_glyph(k: ObjectKind) -> &'static str {
    match k {
        ObjectKind::Table => "T",
        ObjectKind::View => "V",
        ObjectKind::Function => "ƒ",
        ObjectKind::Sequence => "#",
    }
}

impl Workbench {
    pub fn new(connection: Connection, catalog: Catalog) -> Self {
        let mut wb = Self {
            connection,
            connection_key: None,
            catalog,
            schema: "public".into(),
            explorer: TreeView::new(EXPLORER, vec![]),
            explorer_filter: TextInput::new(EXPLORER.sub("filter"), "")
                .placeholder("Filter objects")
                .plain_label(),
            explorer_visible: true,
            rail: junie_tui::widgets::rail::SidebarRail::new(EXPLORER_RAIL, "Explorer"),
            explorer_split: Split::new(25, 20, 40),
            explorer_sized: false,
            explorer_seam: Splitter::new(EXPLORER.sub("seam"), SplitDir::Horizontal),
            explorer_split_area: Rect::ZERO,
            maximized: false,
            strip: Tabs::new(TABSTRIP, &[]),
            tabs: vec![],
            active: 0,
            query_counter: 0,
            pending_loads: vec![],
            open_objects: vec![],
            current_object: None,
            pending_run: None,
            closed: vec![],
        };
        wb.strip.allow_new = true;
        wb.build_explorer();
        wb
    }

    pub fn rebuild_for_schema(&mut self) {
        self.build_explorer();
        self.sync_strip();
    }

    fn build_explorer(&mut self) {
        let db = TreeNode::dir(
            &self.catalog.database.clone(),
            self.catalog
                .schemas
                .iter()
                .map(|s| {
                    if *s == self.schema {
                        TreeNode::dir(s, self.schema_children(s)).glyph("S")
                    } else {
                        TreeNode::lazy(s).glyph("S")
                    }
                })
                .collect(),
        )
        .glyph("D");
        self.explorer = TreeView::new(EXPLORER, vec![db]);
        self.explorer.expanded.insert(vec![0]);
        if let Some(i) = self.catalog.schemas.iter().position(|s| *s == self.schema) {
            self.explorer.expanded.insert(vec![0, i]);

            self.explorer.expanded.insert(vec![0, i, 0]);
        }
        self.explorer.flatten();
        self.explorer.cursor = 1;
    }

    fn schema_children(&self, schema: &str) -> Vec<TreeNode> {
        let mut sections = vec![];
        for (kind, label) in [
            (ObjectKind::Table, "Tables"),
            (ObjectKind::View, "Views"),
            (ObjectKind::Function, "Functions"),
            (ObjectKind::Sequence, "Sequences"),
        ] {
            let objs: Vec<&crate::db::Table> = self.catalog.tables_in(schema, kind).collect();
            if objs.is_empty() && kind != ObjectKind::Table {
                continue;
            }
            let children: Vec<TreeNode> = if objs.is_empty() {
                vec![TreeNode::note("No tables")]
            } else {
                objs.iter()
                    .map(|t| {
                        let meta = if t.row_count > 0 {
                            crate::sql::fmt_rows(t.row_count)
                        } else {
                            String::new()
                        };
                        if kind == ObjectKind::Table || kind == ObjectKind::View {
                            TreeNode::lazy(&t.name).glyph(kind_glyph(kind)).meta(&meta)
                        } else {
                            TreeNode::leaf(&t.name)
                                .glyph(kind_glyph(kind))
                                .meta(t.comment.as_deref().unwrap_or(""))
                        }
                    })
                    .collect()
            };
            let mut section = TreeNode::dir(label, children);
            section.meta = Some(objs.len().to_string());
            sections.push(section);
        }
        sections
    }

    fn table_children(&self, t: &crate::db::Table) -> Vec<TreeNode> {
        let mut out = vec![];
        let cols: Vec<TreeNode> = t
            .columns
            .iter()
            .map(|c| {
                let mut meta = c.ty.sql().to_owned();
                if c.primary {
                    meta.push_str(" · pk");
                } else if c.references.is_some() {
                    meta.push_str(" · fk");
                }
                TreeNode::leaf(&c.name).glyph("·").meta(&meta)
            })
            .collect();
        let mut cn = TreeNode::dir("Columns", cols);
        cn.meta = Some(t.columns.len().to_string());
        out.push(cn);
        let idx: Vec<TreeNode> = t
            .indexes
            .iter()
            .map(|i| {
                TreeNode::leaf(&i.name).meta(&format!(
                    "{}{}",
                    if i.unique { "unique · " } else { "" },
                    i.columns.join(", ")
                ))
            })
            .collect();
        let mut ix = TreeNode::dir(
            "Indexes",
            if idx.is_empty() {
                vec![TreeNode::note("No indexes")]
            } else {
                idx
            },
        );
        ix.meta = Some(t.indexes.len().to_string());
        out.push(ix);
        let keys: Vec<TreeNode> = t
            .constraints
            .iter()
            .filter(|k| k.kind.contains("KEY"))
            .map(|k| TreeNode::leaf(&k.name).meta(k.kind))
            .collect();
        let mut kn = TreeNode::dir(
            "Keys",
            if keys.is_empty() {
                vec![TreeNode::note("No keys")]
            } else {
                keys
            },
        );
        kn.meta = Some(
            t.constraints
                .iter()
                .filter(|k| k.kind.contains("KEY"))
                .count()
                .to_string(),
        );
        out.push(kn);
        let checks: Vec<TreeNode> = t
            .constraints
            .iter()
            .filter(|k| !k.kind.contains("KEY"))
            .map(|k| TreeNode::leaf(&k.name).meta(&k.definition))
            .collect();
        let mut chk = TreeNode::dir(
            "Constraints",
            if checks.is_empty() {
                vec![TreeNode::note("No constraints")]
            } else {
                checks
            },
        );
        chk.meta = Some(
            t.constraints
                .iter()
                .filter(|k| !k.kind.contains("KEY"))
                .count()
                .to_string(),
        );
        out.push(chk);
        let trg: Vec<TreeNode> = t
            .triggers
            .iter()
            .map(|tr| {
                let (n, rest) = tr.split_once(' ').unwrap_or((tr, ""));
                TreeNode::leaf(n).meta(rest)
            })
            .collect();
        let mut tn = TreeNode::dir(
            "Triggers",
            if trg.is_empty() {
                vec![TreeNode::note("No triggers")]
            } else {
                trg
            },
        );
        tn.meta = Some(t.triggers.len().to_string());
        out.push(tn);
        out
    }

    pub(crate) fn object_at(&self, path: &[usize]) -> Option<(String, String)> {
        if path.len() < 4 || path[0] != 0 {
            return None;
        }
        let schema = self.catalog.schemas.get(path[1])?.clone();
        let sections: Vec<ObjectKind> = [
            ObjectKind::Table,
            ObjectKind::View,
            ObjectKind::Function,
            ObjectKind::Sequence,
        ]
        .into_iter()
        .filter(|k| *k == ObjectKind::Table || self.catalog.tables_in(&schema, *k).next().is_some())
        .collect();
        let kind = *sections.get(path[2])?;
        let obj = self.catalog.tables_in(&schema, kind).nth(path[3])?;
        Some((schema, obj.name.clone()))
    }

    pub(crate) fn schema_at(&self, path: &[usize]) -> Option<String> {
        if path.len() == 2 && path[0] == 0 {
            self.catalog.schemas.get(path[1]).cloned()
        } else {
            None
        }
    }

    pub fn tick_explorer(&mut self) -> bool {
        let mut changed = false;
        let mut done = vec![];
        for (i, (_, ticks)) in self.pending_loads.iter_mut().enumerate() {
            *ticks = ticks.saturating_sub(1);
            if *ticks == 0 {
                done.push(i);
            }
        }
        for i in done.into_iter().rev() {
            let (path, _) = self.pending_loads.remove(i);
            let children = if let Some(schema) = self.schema_at(&path) {
                self.schema_children(&schema)
            } else if let Some((schema, name)) = self.object_at(&path) {
                match self.catalog.find(Some(&schema), &name) {
                    Some(t) => self.table_children(t),
                    None => vec![],
                }
            } else {
                vec![]
            };
            self.explorer.set_children(&path, children);
            changed = true;
        }
        changed
    }

    pub fn animating(&self) -> bool {
        !self.pending_loads.is_empty()
            || self
                .tabs
                .iter()
                .any(|t| matches!(t, WorkTab::Query(q) if q.is_running()))
    }

    fn sync_strip(&mut self) {
        let mut labels: Vec<String> = self.tabs.iter().map(|t| t.label()).collect();

        for i in 0..labels.len() {
            if labels.iter().filter(|l| **l == labels[i]).count() > 1
                && let WorkTab::Table(t) = &self.tabs[i]
            {
                labels[i] = t.qualified();
            }
        }
        let items: Vec<TabItem> = self
            .tabs
            .iter()
            .zip(labels)
            .map(|(t, label)| {
                let mut it = TabItem::new(&label).closable();
                match t {
                    WorkTab::Table(tt) => {
                        it.prefix = Some("T".into());
                        it.dirty = tt.dirty_count() > 0;
                    }
                    WorkTab::Query(q) => {
                        it.prefix = Some("≡".into());
                        it.busy = q.is_running();
                        it.dirty = q.dirty() && !q.is_running();
                        it.error = q.last_status.as_ref().is_some_and(|s| s.1)
                            && !q.is_running()
                            && q.unseen;
                    }
                    WorkTab::History(_) => {
                        it.prefix = Some("H".into());
                    }
                }
                it
            })
            .collect();
        let active = self.active.min(items.len().saturating_sub(1));

        self.strip.set_items(items);
        self.strip.allow_new = true;
        self.strip.set_active(active);
        self.current_object = match self.tabs.get(self.active) {
            Some(WorkTab::Table(t)) => Some((t.schema.clone(), t.name.clone())),
            _ => None,
        };
        self.open_objects = self
            .tabs
            .iter()
            .filter_map(|t| {
                if let WorkTab::Table(t) = t {
                    Some((t.schema.clone(), t.name.clone()))
                } else {
                    None
                }
            })
            .collect();
    }

    pub fn open_table(&mut self, schema: &str, name: &str, preview: bool) -> Option<usize> {
        if let Some(i) = self
            .tabs
            .iter()
            .position(|t| matches!(t, WorkTab::Table(tt) if tt.schema == schema && tt.name == name))
        {
            self.active = i;
            if let WorkTab::Table(tt) = &mut self.tabs[i]
                && !preview
            {
                tt.preview = false;
            }
            self.sync_strip();
            return Some(i);
        }
        let table = self.catalog.find(Some(schema), name)?.clone();
        let tab = TableTab::new(
            ID.sub("tab")
                .child(self.tabs.len() + self.query_counter + 1000),
            &self.catalog,
            &table,
            preview,
        );

        if preview
            && let Some(i) = self
                .tabs
                .iter()
                .position(|t| matches!(t, WorkTab::Table(tt) if tt.preview))
        {
            self.tabs[i] = WorkTab::Table(tab);
            self.active = i;
            self.sync_strip();
            return Some(i);
        }
        self.tabs.push(WorkTab::Table(tab));
        self.active = self.tabs.len() - 1;
        self.sync_strip();
        Some(self.active)
    }

    pub fn new_query(&mut self, text: &str) -> usize {
        self.query_counter += 1;
        let name = format!("Query {}", self.query_counter);
        let tab = QueryTab::new(ID.sub("query").child(self.query_counter), &name, text);
        self.tabs.push(WorkTab::Query(tab));
        self.active = self.tabs.len() - 1;
        self.sync_strip();
        self.active
    }

    pub fn open_history(&mut self, history: &History) -> usize {
        if let Some(i) = self
            .tabs
            .iter()
            .position(|t| matches!(t, WorkTab::History(_)))
        {
            self.active = i;
            if let WorkTab::History(h) = &mut self.tabs[i] {
                h.refresh(history, &self.connection.name);
            }
            self.sync_strip();
            return i;
        }
        let mut h = HistoryTab::new(ID.sub("history"));
        h.refresh(history, &self.connection.name);
        self.tabs.push(WorkTab::History(h));
        self.active = self.tabs.len() - 1;
        self.sync_strip();
        self.active
    }

    pub fn history_tab(&self, owner: WidgetId) -> Option<&HistoryTab> {
        self.tabs.iter().find_map(|t| match t {
            WorkTab::History(h) if h.id == owner => Some(h),
            _ => None,
        })
    }

    pub fn history_action(
        &self,
        owner: WidgetId,
        entry: ItemKey,
        cmd: HistoryCommand,
        history: &History,
        cx: &mut Cx,
    ) -> Outcome {
        match self.history_tab(owner) {
            Some(h) => h.run(entry, cmd, history, cx),
            None => {
                cx.status("The history tab is no longer available");
                Outcome::Changed
            }
        }
    }

    pub fn record_history(&mut self, history: &mut History, entries: Vec<HistoryEntry>) {
        for e in entries {
            history.push(e);
        }
        for t in self.tabs.iter_mut() {
            if let WorkTab::History(h) = t {
                h.refresh(history, &self.connection.name);
            }
        }
    }

    pub fn close_tab(&mut self, i: usize) {
        if i < self.tabs.len() {
            if let WorkTab::Table(t) = &self.tabs[i] {
                self.closed.push((t.schema.clone(), t.name.clone()));
            }
            self.tabs.remove(i);
            if self.active >= self.tabs.len() {
                self.active = self.tabs.len().saturating_sub(1);
            } else if self.active > i {
                self.active -= 1;
            }
            self.sync_strip();
        }
    }

    pub fn set_active(&mut self, i: usize) {
        if i < self.tabs.len() {
            self.active = i;
            if let WorkTab::Query(q) = &mut self.tabs[i] {
                q.unseen = false;
            }
            self.sync_strip();
        }
    }

    pub fn active_tab(&self) -> Option<&WorkTab> {
        self.tabs.get(self.active)
    }
    pub fn active_tab_mut(&mut self) -> Option<&mut WorkTab> {
        self.tabs.get_mut(self.active)
    }

    pub fn primary_focus(&self) -> Option<WidgetId> {
        match self.tabs.get(self.active)? {
            WorkTab::Table(t) => Some(t.grid.id),
            WorkTab::Query(q) => Some(q.editor.id),
            WorkTab::History(h) => Some(h.list.id),
        }
    }

    pub fn is_editing(&self) -> bool {
        self.explorer_filter.editing || self.active_tab().is_some_and(|t| t.is_editing())
    }

    pub fn pending_total(&self) -> usize {
        self.tabs
            .iter()
            .map(|t| {
                if let WorkTab::Table(t) = t {
                    t.dirty_count()
                } else {
                    0
                }
            })
            .sum()
    }

    pub fn unsaved_queries(&self) -> usize {
        self.tabs
            .iter()
            .filter(|t| matches!(t, WorkTab::Query(q) if q.dirty()))
            .count()
    }

    pub fn losses(&self) -> Option<String> {
        let (pending, queries) = (self.pending_total(), self.unsaved_queries());
        let mut parts = vec![];
        if pending > 0 {
            parts.push(format!(
                "{pending} pending row change{}",
                if pending == 1 { "" } else { "s" }
            ));
        }
        if queries > 0 {
            parts.push(format!(
                "{queries} unsaved quer{}",
                if queries == 1 { "y" } else { "ies" }
            ));
        }
        (!parts.is_empty()).then(|| parts.join(" and "))
    }

    pub fn running(&self) -> Option<u32> {
        self.tabs.iter().find_map(|t| match t {
            WorkTab::Query(q) => q.running.as_ref().map(|r| r.started_ticks * 80),
            _ => None,
        })
    }

    pub fn hints(&self, focus: Option<WidgetId>) -> Vec<Hint> {
        if focus == Some(self.explorer.id) {
            return vec![
                hint("↑↓", "Move"),
                hint("Enter", "Open"),
                hint("→", "Expand"),
                hint("/", "Filter"),
                hint("Ctrl+O", "Quick open"),
                junie_tui::widgets::keyhint::CONTEXT_MENU,
            ];
        }
        if focus == Some(self.explorer_filter.id) {
            return vec![
                hint("Type", "Filter"),
                hint("↓", "Into tree"),
                hint("Esc", "Clear"),
            ];
        }
        if focus == Some(self.strip.id) {
            let mut v = vec![hint("←→", "Switch"), hint("Ctrl+T", "New query")];

            v.extend(
                junie_tui::core::keymap::hint_key(
                    &self.strip.keymap,
                    junie_tui::widgets::tabs::TabCmd::Close,
                )
                .map(|k| hint(k, "Close")),
            );
            v.extend([
                hint("Ctrl+G", "Tab list"),
                hint("z", "Zoom"),
                junie_tui::widgets::keyhint::CONTEXT_MENU,
            ]);
            return v;
        }
        match self.active_tab() {
            Some(WorkTab::Table(t)) => t.hints(focus),
            Some(WorkTab::Query(q)) => q.hints(focus),
            Some(WorkTab::History(h)) => h.hints(focus),
            None => vec![hint("Ctrl+T", "New query"), hint("0", "Explorer")],
        }
    }

    pub fn on_key(&mut self, key: &Key, cx: &mut Cx, history: &mut History) -> Outcome {
        let Some(f) = cx.focus.current() else {
            return Outcome::Ignored;
        };
        if f == self.explorer_filter.id {
            let (o, ev) = self.explorer_filter.on_key(key);
            match ev {
                Some(InputAction::Changed)
                | Some(InputAction::Committed)
                | Some(InputAction::Cancelled) => {
                    let q = self.explorer_filter.text().to_owned();
                    self.explorer
                        .set_filter(if q.is_empty() { None } else { Some(&q) });
                }
                Some(InputAction::CommittedTab { backward }) => {
                    if backward {
                        cx.focus_prev()
                    } else {
                        cx.focus_next()
                    }
                }
                None => {}
            }
            if !o.consumed() && key.is(KeyCode::Down) {
                cx.focus.focus(self.explorer.id);
                return Outcome::Changed;
            }
            return o;
        }
        if f == self.explorer.id {
            if key.is_char('/') {
                cx.focus.focus(self.explorer_filter.id);
                self.explorer_filter.begin_edit();
                return Outcome::Changed;
            }

            if key.is(KeyCode::Enter) {
                let path = self
                    .explorer
                    .rows()
                    .get(self.explorer.cursor)
                    .map(|r| r.path.clone());
                if let Some(path) = path {
                    if let Some((schema, name)) = self.object_at(&path) {
                        let obj = self.catalog.find(Some(&schema), &name).map(|t| t.kind);
                        if matches!(obj, Some(ObjectKind::Table | ObjectKind::View)) {
                            self.explorer.selected = Some(path);
                            self.open_table(&schema, &name, false);
                            if let Some(pf) = self.primary_focus() {
                                cx.focus.focus(pf);
                            }
                            return Outcome::Changed;
                        }
                    }
                    if let Some(schema) = self.schema_at(&path) {
                        self.schema = schema;
                        cx.status(format!("Schema {}", self.schema));
                    }
                }
            }
            if key.is(KeyCode::F(5)) || key.is_char('r') {
                self.build_explorer();
                cx.status("Explorer refreshed");
                return Outcome::Changed;
            }
            let (o, actions) = self.explorer.on_key(key).into_parts();
            for action in actions {
                if let TreeAction::Expand(path) = action {
                    self.pending_loads.push((path, 3));
                }
            }
            return o;
        }
        if f == self.strip.id {
            let (o, ev) = self.strip.on_key(key);
            match ev {
                Some(TabAction::Activated(i)) => self.set_active(i),
                Some(TabAction::Close(i)) => return self.request_close(i, cx),
                Some(TabAction::New) => {
                    self.new_query("");
                    if let Some(pf) = self.primary_focus() {
                        cx.focus.focus(pf);
                    }
                }
                None => {}
            }
            return o;
        }
        let cat = &self.catalog;
        let conn = self.connection.name.clone();
        let db = self.catalog.database.clone();
        match self.tabs.get_mut(self.active) {
            Some(WorkTab::Table(t)) => Self::table_key(t, key, f, cx, cat, &conn, &db, history),
            Some(WorkTab::Query(q)) => q.on_key(key, cx, cat),
            Some(WorkTab::History(h)) => Self::history_key(h, key, f, cx, history, &conn),
            None => Outcome::Ignored,
        }
    }

    #[allow(clippy::too_many_arguments)]
    fn table_key(
        t: &mut TableTab,
        key: &Key,
        f: WidgetId,
        cx: &mut Cx,
        cat: &Catalog,
        conn: &str,
        db: &str,
        history: &mut History,
    ) -> Outcome {
        if f == t.mode_tabs.id {
            let (o, _) = t.mode_tabs.on_key(key);
            return o;
        }
        if f == t.structure_tabs.id {
            let (o, ev) = t.structure_tabs.on_key(key);
            if ev.is_some() {
                t.structure_refresh(cat);
            }
            return o;
        }
        if f == t.structure.id {
            return t.structure.on_key(key).outcome();
        }
        if f == t.ddl.id {
            return t.ddl.on_key(key);
        }
        if f == t.chips.id {
            let (o, ev) = t.chips.on_key(key);
            match ev {
                Some(junie_tui::widgets::chips::ChipAction::Remove(i)) => {
                    t.filters.remove(i);
                    t.load(cat);
                    cx.status("Filter removed");
                }
                Some(junie_tui::widgets::chips::ChipAction::Toggle(i)) => {
                    t.filters[i].enabled = !t.filters[i].enabled;
                    t.load(cat);
                }
                Some(junie_tui::widgets::chips::ChipAction::Activate(i)) => {
                    cx.go(crate::app::Act::EditFilter(Some(i)));
                }
                Some(junie_tui::widgets::chips::ChipAction::Add) => {
                    cx.go(crate::app::Act::EditFilter(None))
                }
                Some(junie_tui::widgets::chips::ChipAction::ClearAll) => {
                    t.filters.clear();
                    t.load(cat);
                }
                Some(junie_tui::widgets::chips::ChipAction::Lead) => {
                    t.match_all = !t.match_all;
                    t.load(cat);
                }
                None => {}
            }
            return o;
        }
        for bid in t.grid.bar_ids() {
            if f == bid {
                let (o, ev) = t.grid.on_bar_key(bid, key);
                if let Some(ev) = ev {
                    return Self::grid_event(t, ev, cx, cat, conn, db, history).or(o);
                }
                return o;
            }
        }
        if f == t.grid.id {
            let (o, actions) = t.grid.on_key(key).into_parts();
            let mut out = o;
            for ev in actions {
                out = Self::grid_event(t, ev, cx, cat, conn, db, history).or(out);
            }
            return out;
        }
        Outcome::Ignored
    }

    fn grid_event(
        t: &mut TableTab,
        ev: junie_tui::widgets::grid::GridAction,
        cx: &mut Cx,
        cat: &Catalog,
        _conn: &str,
        _db: &str,
        _history: &mut History,
    ) -> Outcome {
        use junie_tui::widgets::grid::GridAction;
        match ev {
            GridAction::SortRequested(s) => {
                t.sort = s;
                t.load(cat);
                match s {
                    Some((c, d)) => cx.status(format!(
                        "Sorted by {} {}",
                        t.columns[c].0,
                        if d == SortDir::Asc {
                            "ascending"
                        } else {
                            "descending"
                        }
                    )),
                    None => cx.status("Sort cleared"),
                }
            }
            GridAction::FetchMore => {
                cx.status("Showing the first 500 rows · add a LIMIT or filter to narrow");
                t.grid.set_loading(false);
            }
            GridAction::Refresh => {
                if t.dirty_count() > 0 {
                    cx.go(crate::app::Act::ConfirmDiscard);
                } else {
                    t.load(cat);
                    cx.status("Refreshed");
                }
            }
            GridAction::CommitRequested => cx.go(crate::app::Act::CommitPending),
            GridAction::DiscardRequested => cx.go(crate::app::Act::ConfirmDiscard),
            GridAction::PreviewSql => cx.go(crate::app::Act::PreviewSql),
            GridAction::Copy(s) => cx.copy(s),
            GridAction::FilterOnCell { col, value } => {
                cx.go(crate::app::Act::FilterOnCell(col, value));
            }
            GridAction::OpenFilters => cx.go(crate::app::Act::EditFilter(None)),
            GridAction::ClearFilters => {
                t.filters.clear();
                t.load(cat);
                cx.status("Filters cleared");
            }
            GridAction::FollowReference { row, col } => {
                let name = t.columns[col].0.clone();
                let v = t.grid.value(row, col).text();
                if let Some(target) = cat
                    .find(Some(&t.schema), &name)
                    .and(None::<String>)
                    .or_else(|| t.grid.columns()[col].references.clone())
                {
                    cx.go(crate::app::Act::OpenTableFiltered(target, "id".into(), v));
                }
            }
            GridAction::OpenViewer { row, col } => {
                let v = t.grid.value(row, col).clone();
                cx.go(crate::app::Act::OpenViewer(t.columns[col].0.clone(), v));
            }
            GridAction::LeaveForward => cx.focus_next(),
            GridAction::LeaveBackward => cx.focus_prev(),
            GridAction::ReadOnly { reason } => cx.status(reason),
            GridAction::CellChanged { .. }
            | GridAction::Undone(_)
            | GridAction::RowInserted(_)
            | GridAction::RowDeleted(_)
            | GridAction::Activated(_) => {}
        }
        Outcome::Changed
    }

    fn history_key(
        h: &mut HistoryTab,
        key: &Key,
        f: WidgetId,
        cx: &mut Cx,
        history: &mut History,
        conn: &str,
    ) -> Outcome {
        if f == h.search.id {
            let (o, ev) = h.search.on_key(key);
            match ev {
                Some(InputAction::Changed)
                | Some(InputAction::Committed)
                | Some(InputAction::Cancelled) => h.refresh(history, conn),
                Some(InputAction::CommittedTab { backward }) => {
                    if backward {
                        cx.focus_prev()
                    } else {
                        cx.focus_next()
                    }
                }
                None => {}
            }
            if !o.consumed() && key.is(KeyCode::Down) {
                cx.focus.focus(h.list.id);
                return Outcome::Changed;
            }
            return o;
        }
        if f == h.list.id {
            match key.code {
                KeyCode::Char('/') => {
                    cx.focus.focus(h.search.id);
                    h.search.begin_edit();
                    return Outcome::Changed;
                }
                KeyCode::Char('c') => {
                    h.scope_all = !h.scope_all;
                    h.refresh(history, conn);
                    return Outcome::Changed;
                }
                KeyCode::Char('s') => {
                    h.failed_only = !h.failed_only;
                    h.refresh(history, conn);
                    return Outcome::Changed;
                }
                KeyCode::Enter => return h.run_selected(HistoryCommand::Open, history, cx),
                KeyCode::Char('r') => return h.run_selected(HistoryCommand::Rerun, history, cx),
                KeyCode::Char('y') => return h.run_selected(HistoryCommand::Copy, history, cx),
                _ => {}
            }
            let o = h.list.on_key(key);
            h.sync_detail(history);
            return o;
        }
        if f == h.detail.id {
            return h.detail.on_key(key).0;
        }
        let mut hit: Option<(Outcome, bool, HistoryCommand)> = None;
        for (b, cmd) in [
            (&mut h.open_btn, HistoryCommand::Open),
            (&mut h.rerun_btn, HistoryCommand::Rerun),
            (&mut h.copy_btn, HistoryCommand::Copy),
        ] {
            if f == b.id {
                let (o, act) = b.on_key(key);
                hit = Some((o, act, cmd));
                break;
            }
        }
        if let Some((o, act, cmd)) = hit {
            if act {
                h.run_selected(cmd, history, cx);
            }
            return o;
        }
        Outcome::Ignored
    }

    fn request_close(&mut self, i: usize, cx: &mut Cx) -> Outcome {
        if self.tabs.get(i).is_some_and(|t| t.dirty()) {
            cx.go(crate::app::Act::ConfirmCloseTab(i));
        } else {
            self.close_tab(i);
        }
        Outcome::Changed
    }

    fn pointer_fields(&mut self) -> Vec<&mut dyn PointerField> {
        let mut v: Vec<&mut dyn PointerField> = vec![&mut self.explorer_filter];
        if let Some(WorkTab::History(h)) = self.tabs.get_mut(self.active) {
            v.push(&mut h.search);
        }
        v
    }

    pub fn pointer(&mut self, ev: &PointerEvent, cx: &mut Cx, history: &mut History) -> Outcome {
        let facts = match self.tabs.get_mut(self.active) {
            Some(WorkTab::History(h)) => Some(&mut h.detail_view),
            Some(WorkTab::Query(q)) => Some(&mut q.result_facts),
            _ => None,
        };
        if let Some(r) = facts.and_then(|v| v.pointer(ev)) {
            return r.outcome();
        }
        if ev.wheel().is_some() {
            return self.wheel(ev);
        }
        match ev.phase() {
            Some(Phase::Press) => self.press(ev),
            Some(Phase::Click) => self.click(ev, cx, history),
            Some(Phase::Drag) => self.drag(ev),
            Some(Phase::Middle) => self.middle(ev, cx),
            _ => Outcome::Ignored,
        }
    }

    fn press(&mut self, ev: &PointerEvent) -> Outcome {
        let (id, pos) = (ev.id, ev.pos());
        if id == self.explorer_seam.id {
            let area = self.explorer_split_area;
            self.explorer_seam
                .on_press(&self.explorer_split, area, 1, pos);
            return Outcome::Consumed;
        }
        match self.tabs.get_mut(self.active) {
            Some(WorkTab::Query(q)) if id == q.seam.id => {
                q.seam.on_press(&q.split, q.split_area, 1, pos);
                return Outcome::Consumed;
            }
            Some(WorkTab::History(h)) if id == h.seam.id => {
                h.seam.on_press(&h.split, h.split_area, 2, pos);
                return Outcome::Consumed;
            }
            _ => {}
        }
        if let Some(o) = route_press(&mut self.pointer_fields(), id, pos, ev.count()) {
            return o;
        }
        if let Some(r) = self.explorer.route(ev) {
            return r.outcome();
        }
        match self.tabs.get_mut(self.active) {
            Some(WorkTab::Table(t)) => t
                .grid
                .route(ev)
                .map(|r| r.outcome())
                .or_else(|| t.structure.route(ev).map(|r| r.outcome()))
                .or_else(|| t.ddl.route(ev).map(|r| r.outcome()))
                .unwrap_or(Outcome::Ignored),
            Some(WorkTab::Query(q)) => q.press(ev),
            Some(WorkTab::History(h)) => h
                .list
                .route(ev)
                .map(|r| r.outcome())
                .or_else(|| h.detail.route(ev).map(|r| r.outcome()))
                .unwrap_or(Outcome::Ignored),
            None => Outcome::Ignored,
        }
    }

    fn middle(&mut self, ev: &PointerEvent, cx: &mut Cx) -> Outcome {
        if let Some(r) = self.strip.route(ev) {
            let (mut out, actions) = r.into_parts();
            for act in actions {
                if let TabAction::Close(i) = act {
                    out = self.request_close(i, cx);
                }
            }
            return out;
        }
        match self.tabs.get_mut(self.active) {
            Some(WorkTab::Query(q)) => q.middle(ev),
            _ => Outcome::Ignored,
        }
    }

    pub fn active_grid_any_mut(&mut self) -> Option<&mut junie_tui::widgets::grid::DataGrid> {
        match self.tabs.get_mut(self.active)? {
            WorkTab::Table(t) => Some(&mut t.grid),
            WorkTab::Query(q) => q.active_grid_mut(),
            WorkTab::History(_) => None,
        }
    }

    pub fn on_header_action(
        &mut self,
        col: usize,
        action: junie_tui::ui::columns::HeaderAction,
        cx: &mut Cx,
        history: &mut History,
    ) -> Outcome {
        let cat = &self.catalog;
        let conn = self.connection.name.clone();
        let db = self.catalog.database.clone();
        match self.tabs.get_mut(self.active) {
            Some(WorkTab::Table(t)) => {
                let (o, ev) = t.grid.on_header_action(col, action);
                match ev {
                    Some(ev) => Self::grid_event(t, ev, cx, cat, &conn, &db, history).or(o),
                    None => o,
                }
            }
            Some(WorkTab::Query(q)) => q
                .active_grid_mut()
                .map_or(Outcome::Ignored, |g| g.on_header_action(col, action).0),
            _ => Outcome::Ignored,
        }
    }

    fn click(&mut self, ev: &PointerEvent, cx: &mut Cx, history: &mut History) -> Outcome {
        let (id, pos) = (ev.id, ev.pos());
        if let Some(o) = route_click(&mut self.pointer_fields(), id, pos, ev.count()) {
            cx.focus.focus(id);
            return o;
        }
        if ev.on(self.explorer.id) {
            let row = ev.index_of(Part::ROW);
            if self.explorer.row_at(ev).is_some() {
                cx.focus.focus(self.explorer.id);
            }
            let path = row.and_then(|r| self.explorer.rows().get(r).map(|r| r.path.clone()));
            if let Some(row) = row
                && let Some(path) = &path
                && let Some((schema, name)) = self.object_at(path)
            {
                let kind = self.catalog.find(Some(&schema), &name).map(|t| t.kind);
                if matches!(kind, Some(ObjectKind::Table | ObjectKind::View)) {
                    let promote =
                        self.current_object.as_ref() == Some(&(schema.clone(), name.clone()));
                    self.explorer.cursor = row;
                    self.explorer.selected = Some(path.clone());
                    self.open_table(&schema, &name, !promote);
                    return Outcome::Changed;
                }
            }
            let (o, actions) = self.explorer.pointer(ev).into_parts();
            for act in actions {
                if let TreeAction::Expand(path) = act {
                    self.pending_loads.push((path, 3));
                }
            }
            return o;
        }
        if id == self.explorer_seam.id {
            if ev.count() == 2 {
                self.explorer_sized = false;
                return Outcome::Changed;
            }
            return Outcome::Consumed;
        }
        if let Some(r) = self.strip.route(ev) {
            cx.focus.focus(self.strip.id);
            let (o, actions) = r.into_parts();
            for act in actions {
                match act {
                    TabAction::Activated(i) => {
                        self.set_active(i);
                        if let Some(pf) = self.primary_focus() {
                            cx.focus.focus(pf);
                        }
                    }
                    TabAction::Close(i) => return self.request_close(i, cx),
                    TabAction::New => {
                        self.new_query("");
                        if let Some(pf) = self.primary_focus() {
                            cx.focus.focus(pf);
                        }
                    }
                }
            }
            return o;
        }
        let cat = &self.catalog;
        let conn = self.connection.name.clone();
        let db = self.catalog.database.clone();
        match self.tabs.get_mut(self.active) {
            Some(WorkTab::Table(t)) => {
                if let Some(r) = t.mode_tabs.route(ev) {
                    cx.focus.focus(t.mode_tabs.id);
                    return r.outcome();
                }
                if let Some(r) = t.structure_tabs.route(ev) {
                    cx.focus.focus(t.structure_tabs.id);
                    let (o, actions) = r.into_parts();
                    if !actions.is_empty() {
                        t.structure_refresh(cat);
                    }
                    return o;
                }
                if let Some(r) = t.structure.route(ev) {
                    cx.focus.focus(t.structure.id);
                    return r.outcome().or(Outcome::Changed);
                }
                if let Some(r) = t.ddl.route(ev) {
                    return r.outcome();
                }
                if let Some(r) = t.chips.route(ev) {
                    cx.focus.focus(t.chips.id);
                    let (o, actions) = r.into_parts();
                    for act in actions {
                        match act {
                            junie_tui::widgets::chips::ChipAction::Remove(i) => {
                                t.filters.remove(i);
                                t.load(cat);
                            }
                            junie_tui::widgets::chips::ChipAction::Activate(i) => {
                                cx.go(crate::app::Act::EditFilter(Some(i)))
                            }
                            junie_tui::widgets::chips::ChipAction::Add => {
                                cx.go(crate::app::Act::EditFilter(None))
                            }
                            junie_tui::widgets::chips::ChipAction::Lead => {
                                t.match_all = !t.match_all;
                                t.load(cat);
                            }
                            _ => {}
                        }
                    }
                    return o;
                }
                if !t.grid.bar_ids().contains(&id) && ev.on(t.grid.id) {
                    cx.focus.focus(t.grid.id);
                }
                if let Some(r) = t.grid.route(ev) {
                    let (o, actions) = r.into_parts();
                    let mut out = o;
                    for act in actions {
                        out = Self::grid_event(t, act, cx, cat, &conn, &db, history).or(out);
                    }
                    return out;
                }
                Outcome::Ignored
            }
            Some(WorkTab::Query(q)) => q.click(ev, cx, cat),
            Some(WorkTab::History(h)) => {
                if id == h.seam.id {
                    return h.seam.on_click(&mut h.split, ev.count());
                }
                if ev.on(h.list.id) {
                    if ev.index_of(Part::ROW).is_some() {
                        cx.focus.focus(h.list.id);
                    }
                    let o = h.list.pointer(ev).outcome();
                    h.sync_detail(history);
                    return o;
                }
                if ev.on(h.detail.id) {
                    if id == h.detail.id {
                        cx.focus.focus(h.detail.id);
                    }
                    return h.detail.pointer(ev).outcome();
                }
                let mut hit: Option<HistoryCommand> = None;
                for (b, cmd) in [
                    (&mut h.open_btn, HistoryCommand::Open),
                    (&mut h.rerun_btn, HistoryCommand::Rerun),
                    (&mut h.copy_btn, HistoryCommand::Copy),
                ] {
                    if b.id == id && b.on_click() {
                        hit = Some(cmd);
                        break;
                    }
                }
                match hit {
                    Some(cmd) => h.run_selected(cmd, history, cx),
                    None => Outcome::Ignored,
                }
            }
            None => Outcome::Ignored,
        }
    }

    fn drag(&mut self, ev: &PointerEvent) -> Outcome {
        let (pressed, pos) = (ev.id, ev.pos());
        if let Some(o) = route_drag(&mut self.pointer_fields(), pressed, pos) {
            return o;
        }
        if let Some(r) = self.explorer.route(ev) {
            return r.outcome();
        }
        if pressed == self.explorer_seam.id {
            self.explorer_sized = true;
            let area = self.explorer_split_area;
            return self
                .explorer_seam
                .on_drag(&mut self.explorer_split, area, 1, pos);
        }
        match self.tabs.get_mut(self.active) {
            Some(WorkTab::Table(t)) => t
                .grid
                .route(ev)
                .map(|r| r.outcome())
                .or_else(|| t.structure.route(ev).map(|r| r.outcome()))
                .or_else(|| t.ddl.route(ev).map(|r| r.outcome()))
                .unwrap_or(Outcome::Ignored),
            Some(WorkTab::Query(q)) => q.drag(ev),
            Some(WorkTab::History(h)) => {
                if pressed == h.seam.id {
                    let area = h.split_area;
                    return h.seam.on_drag(&mut h.split, area, 2, pos);
                }
                h.list
                    .route(ev)
                    .map(|r| r.outcome())
                    .or_else(|| h.detail.route(ev).map(|r| r.outcome()))
                    .unwrap_or(Outcome::Ignored)
            }
            None => Outcome::Ignored,
        }
    }

    fn wheel(&mut self, ev: &PointerEvent) -> Outcome {
        let horizontal = matches!(ev.wheel(), Some((Axis::Horizontal, _)));

        if let Some(r) = self.strip.route(ev) {
            return r.outcome();
        }

        if ev.on(self.explorer.id) {
            return if horizontal {
                Outcome::Consumed
            } else {
                self.explorer.pointer(ev).outcome()
            };
        }
        match self.tabs.get_mut(self.active) {
            Some(WorkTab::Table(t)) => t
                .grid
                .route(ev)
                .map(|r| r.outcome())
                .or_else(|| t.structure.route(ev).map(|r| r.outcome()))
                .or_else(|| t.ddl.route(ev).map(|r| r.outcome()))
                .unwrap_or(Outcome::Ignored),
            Some(WorkTab::Query(q)) => q.wheel(ev),
            Some(WorkTab::History(h)) => {
                if ev.on(h.list.id) {
                    return if horizontal {
                        Outcome::Consumed
                    } else {
                        h.list.pointer(ev).outcome()
                    };
                }
                h.detail
                    .route(ev)
                    .map(|r| r.outcome())
                    .unwrap_or(Outcome::Ignored)
            }
            None => Outcome::Ignored,
        }
    }

    pub fn on_blur(&mut self, ch: junie_tui::core::focus::FocusChange) -> Outcome {
        use junie_tui::core::focus::forward_blur;
        let mut out = forward_blur(ch, &mut [&mut self.explorer_filter]);
        for tab in &mut self.tabs {
            out = out.or(match tab {
                WorkTab::Table(t) => forward_blur(ch, &mut [&mut t.grid, &mut t.structure]),
                WorkTab::History(h) => forward_blur(ch, &mut [&mut h.search]),
                WorkTab::Query(_) => Outcome::Ignored,
            });
        }
        out
    }

    pub fn on_paste(&mut self, text: &str) -> Outcome {
        if self.explorer_filter.editing {
            return self.explorer_filter.on_paste(text);
        }
        match self.tabs.get_mut(self.active) {
            Some(WorkTab::Table(t)) => t.grid.on_paste(text),
            Some(WorkTab::Query(q)) => q.on_paste(text),
            Some(WorkTab::History(h)) => h.search.on_paste(text),
            None => Outcome::Ignored,
        }
    }

    pub fn render(&mut self, area: Rect, buf: &mut Buffer, ctx: &mut RenderCtx, history: &History) {
        let t = ctx.theme;

        let (strip, body) = Tabs::layout_gap(area, 0);
        self.strip.render(strip, buf, ctx, t.canvas);

        let narrow = area.width < 100;
        let explorer_focused = ctx.interaction.focused(self.explorer.id)
            || ctx.interaction.focused(self.explorer_filter.id);
        let show_explorer =
            self.explorer_visible && !self.maximized && (!narrow || explorer_focused);

        let body = if !show_explorer && !self.maximized && self.rail.width() > 0 {
            self.rail.render_rail(body, buf, ctx, t.canvas);
            let w = self.rail.width().min(body.width);
            Rect::new(body.x + w, body.y, body.width - w, body.height)
        } else {
            body
        };
        let explorer_w = (body.width / 4).clamp(28, 40);
        if !self.explorer_sized {
            self.explorer_split.drag_to(
                SplitDir::Horizontal,
                body,
                1,
                Position::new(body.x + explorer_w, body.y),
            );
            self.explorer_split.default_percent = self.explorer_split.percent;
        }
        self.explorer_split_area = body;
        let (ex, main) = if show_explorer && narrow {
            (body, Rect::ZERO)
        } else if show_explorer {
            let (ex, main) = self.explorer_split.horizontal(body, 1);
            let handle = self.explorer_split.handle(SplitDir::Horizontal, body, 1);
            self.explorer_seam.render(handle, buf, ctx, t.canvas);
            (ex, main)
        } else {
            (Rect::ZERO, body)
        };
        if !ex.is_empty() {
            let ef = ctx.interaction.focused(self.explorer.id)
                || ctx.interaction.focused(self.explorer_filter.id);

            let meta = format!("{}    ", self.schema);
            let panel = Panel::framed(Some("Explorer")).focused(ef).meta(&meta);
            let bg = panel.bg(t);
            let inner = panel.render(ex, buf, t);
            self.rail.render_collapse(ex, buf, ctx, t.canvas);
            self.explorer_filter.render(
                Rect::new(inner.x.saturating_sub(1), inner.y, inner.width + 1, 2),
                buf,
                ctx,
                bg,
            );
            let tree_area = Rect::new(
                inner.x.saturating_sub(1),
                inner.y + 2,
                inner.width + 1,
                inner.height.saturating_sub(2),
            );

            self.explorer.render(tree_area, buf, ctx, bg);
        } else if self.explorer_visible && !self.maximized {
            ctx.control(self.explorer.id, Rect::ZERO, false);
        }

        if main.is_empty() {
            if let Some(pf) = self.primary_focus() {
                ctx.control(pf, Rect::ZERO, false);
            }
            return;
        }
        let Some(active) = self.tabs.get(self.active) else {
            let panel = Panel::framed(None);
            let inner = panel.render(main, buf, t);
            junie_tui::widgets::empty::render(inner, buf, t, &EmptyState::new("No open tabs").hint("Enter on a table in the explorer opens it · Ctrl+T starts a query · Ctrl+O opens anything"), t.canvas);
            return;
        };
        let title = match active {
            WorkTab::Table(tt) => format!("{} › {}", tt.schema, tt.name),
            WorkTab::Query(q) => q.name.clone(),
            WorkTab::History(_) => "Query history".into(),
        };
        let focus_in_tab = ctx.interaction.shown_focus().is_some_and(|f| {
            f != self.explorer.id && f != self.explorer_filter.id && f != self.strip.id
        });
        let meta = match active {
            WorkTab::Table(tt) => format!(
                "{} · {} cols",
                if tt.preview { "preview" } else { "" },
                tt.columns.len()
            )
            .trim_start_matches(" · ")
            .to_owned(),
            WorkTab::Query(q) => q
                .last_duration
                .map(|ms| {
                    junie_tui::ui::text::duration(
                        std::time::Duration::from_millis(u64::from(ms)),
                        junie_tui::ui::text::DurationStyle::Compact,
                    )
                })
                .unwrap_or_default(),
            WorkTab::History(_) => format!("{} entries", history.entries.len()),
        };
        let panel = Panel::framed(Some(&title))
            .focused(focus_in_tab)
            .meta(&meta);
        let bg = panel.bg(t);
        let inner = panel.render(main, buf, t);
        let conn = self.connection.name.clone();
        match self.tabs.get_mut(self.active) {
            Some(WorkTab::Table(tt)) => tt.render(inner, buf, ctx, bg),
            Some(WorkTab::Query(q)) => q.render(inner, buf, ctx, bg),
            Some(WorkTab::History(h)) => h.render(inner, buf, ctx, bg, history, &conn),
            None => {}
        }
    }
}
