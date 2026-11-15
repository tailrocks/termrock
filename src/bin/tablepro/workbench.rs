use ratatui::buffer::Buffer;
use ratatui::crossterm::event::KeyCode;
use ratatui::layout::{Position, Rect};

use junie_tui::core::event::{Key, Outcome};
use junie_tui::core::hit::Axis;
use junie_tui::core::id::{ItemKey, Part, WidgetId};
use junie_tui::core::intent::Phase;
use junie_tui::core::pointer::{MouseTarget, PointerEvent};
use junie_tui::core::response::Response;
use junie_tui::ui::ctx::RenderCtx;
use junie_tui::ui::layout::{Split, SplitDir};
use junie_tui::widgets::document_workspace::{
    CloseDecision, CloseToken, DocumentWorkspace, OpenKind, OperationToken, WorkspaceAction,
    WorkspaceDocument,
};
use junie_tui::widgets::empty::EmptyState;
use junie_tui::widgets::field_common::{PointerField, route_click, route_drag, route_press};
use junie_tui::widgets::input::{InputAction, TextInput};
use junie_tui::widgets::keyhint::{Hint, hint};
use junie_tui::widgets::panel::Panel;
use junie_tui::widgets::splitter::Splitter;
use junie_tui::widgets::tabs::{TabAction, Tabs};
use junie_tui::widgets::tree::{TreeAction, TreeNode, TreeView};
use std::borrow::Cow;

use crate::app::{Cx, TableChange};
use crate::db::{Catalog, Connection, ObjectKind};
use crate::model::History;
use crate::store::{SessionStore, Source};
use crate::tabs::{HistoryTab, QueryTab, TableTab, WorkTabView};

const ID: WidgetId = WidgetId::of("workbench");
pub const EXPLORER: WidgetId = ID.sub("explorer");

pub const EXPLORER_RAIL: WidgetId = ID.sub("explorer.rail");
pub const TABSTRIP: WidgetId = ID.sub("tabstrip");

#[allow(clippy::large_enum_variant)]
pub type PendingRun = (
    ItemKey,
    Vec<(String, std::ops::Range<usize>)>,
    bool,
    Option<bool>,
);

/// Why a table that is being saved refuses to close, reload, sort or filter.
pub const SAVING_NOTICE: &str = "Saving · wait for the save to finish";

#[allow(clippy::large_enum_variant)]
pub enum WorkTab {
    Table(TableTab),
    Query(QueryTab),
    History(HistoryTab),
}

impl WorkTab {
    /// The tab's never-reused identity, allocated by the workspace. Unlike a
    /// tab's position or render `WidgetId`, it still names this tab after
    /// others close, open or reorder.
    pub fn key(&self) -> ItemKey {
        match self {
            WorkTab::Table(t) => t.key,
            WorkTab::Query(q) => q.key,
            WorkTab::History(h) => h.key,
        }
    }
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
    pub fn primary_focus(&self) -> WidgetId {
        match self {
            WorkTab::Table(t) => t.grid.id,
            WorkTab::Query(q) => q.editor.id,
            WorkTab::History(h) => h.list.id,
        }
    }
}

impl WorkspaceDocument for WorkTab {
    type View<'a> = WorkTabView<'a>;

    fn title(&self) -> Cow<'_, str> {
        match self {
            WorkTab::Query(q) => Cow::Borrowed(&q.name),
            WorkTab::History(_) => Cow::Borrowed("History"),
            WorkTab::Table(t) => Cow::Owned(t.label()),
        }
    }

    fn dirty(&self) -> bool {
        WorkTab::dirty(self)
    }

    fn focus_target(&self) -> Option<WidgetId> {
        Some(self.primary_focus())
    }

    fn view(&mut self) -> WorkTabView<'_> {
        WorkTabView::new(self)
    }
}

/// What a workspace response asks of the host, after the workbench has
/// updated what it owns (the strip, the recently closed list, unseen marks).
#[derive(Debug, Default)]
pub struct Settled {
    /// Dirty tabs that asked to close; the host presents each choice.
    pub close_requests: Vec<CloseToken>,
    /// Operations the workspace stopped; the host releases what they held.
    pub cancelled: Vec<OperationToken>,
}

pub struct Workbench {
    pub connection: Connection,
    /// The configured connection this session was opened from.
    pub connection_key: Option<junie_tui::core::id::ItemKey>,
    pub catalog: Catalog,
    /// The rows of this connection session. Every tab reads and saves through
    /// it, and it ends with the connection.
    pub store: SessionStore,
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
    /// A projection of the workspace: items and active index are rebuilt from
    /// the ordered keys, and a strip event is translated back to a key.
    pub strip: Tabs,
    pub ws: DocumentWorkspace<WorkTab>,
    pub query_counter: usize,
    pending_loads: Vec<(Vec<usize>, u32)>,

    pub open_objects: Vec<(String, String)>,

    pub current_object: Option<(String, String)>,

    pub pending_run: Option<PendingRun>,
    pub closed: Vec<(String, String)>,
}

/// The render id of one tab's `role`, scoped by the workspace id and the tab's
/// key, never by its position.
fn tab_id(role: &str, key: ItemKey) -> WidgetId {
    ID.sub("tab")
        .sub(role)
        .part(junie_tui::core::id::PartRef::item(Part::ROW, key))
}

fn kind_glyph(k: ObjectKind) -> &'static str {
    match k {
        ObjectKind::Table => "T",
        ObjectKind::View => "V",
        ObjectKind::Function => "ƒ",
        ObjectKind::Sequence => "#",
    }
}

fn row_count_meta(src: Source, table: &crate::db::Table) -> String {
    let rows = src.row_count(table);
    if rows > 0 {
        crate::sql::fmt_rows(rows)
    } else {
        String::new()
    }
}

impl Workbench {
    pub fn new(connection: Connection, catalog: Catalog, store: SessionStore) -> Self {
        let mut wb = Self {
            connection,
            connection_key: None,
            catalog,
            store,
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
            ws: DocumentWorkspace::new(ID.sub("tab")),
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
                        let meta = self.row_count_meta(t);
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

    fn row_count_meta(&self, table: &crate::db::Table) -> String {
        row_count_meta(
            Source {
                catalog: &self.catalog,
                store: &self.store,
            },
            table,
        )
    }

    /// Puts the live row counts on the explorer's loaded tables, leaving its
    /// expansion and cursor alone.
    fn refresh_explorer_counts(&mut self) {
        let src = Source {
            catalog: &self.catalog,
            store: &self.store,
        };
        if let Some(db) = self.explorer.nodes.first_mut() {
            for (schema, schema_node) in src.catalog.schemas.iter().zip(&mut db.children) {
                for node in schema_node
                    .children
                    .iter_mut()
                    .flat_map(|section| section.children.iter_mut())
                {
                    if let Some(table) = src.catalog.find(Some(schema), &node.label)
                        && matches!(table.kind, ObjectKind::Table | ObjectKind::View)
                    {
                        node.meta = Some(row_count_meta(src, table));
                    }
                }
            }
        }
        self.explorer.flatten();
    }

    /// Brings every other view of a saved table up to date. A view with no
    /// draft and no open editor reads the new rows. Any other keeps its draft
    /// and editor exactly as they are and is marked, so its own save fails
    /// as a conflict instead of overwriting the saved rows. Query results are
    /// snapshots and are only marked. Returns how many table views were marked.
    pub fn table_saved(&mut self, origin: ItemKey, schema: &str, name: &str) -> usize {
        let Some(table) = self.catalog.find(Some(schema), name) else {
            return 0;
        };
        let id = Source {
            catalog: &self.catalog,
            store: &self.store,
        }
        .table_id(table);
        let revision = self.store.revision(&id);
        let keys: Vec<ItemKey> = self
            .ws
            .iter()
            .filter(|(key, _)| *key != origin)
            .map(|(key, _)| key)
            .collect();
        let mut marked = 0;
        for key in keys {
            let same_table = |tab: &WorkTab| matches!(tab, WorkTab::Table(t) if t.schema == schema && t.name == name);
            if !self
                .ws
                .get(key)
                .is_some_and(|tab| same_table(tab) || matches!(tab, WorkTab::Query(_)))
            {
                continue;
            }
            let src = Source {
                catalog: &self.catalog,
                store: &self.store,
            };
            let updated = self.ws.update(key, |tab| match tab {
                WorkTab::Table(t) if t.schema == schema && t.name == name => {
                    let idle = t.dirty_count() == 0 && !t.is_editing() && !t.saving();
                    let view = t.view().clone();
                    let refreshed = idle
                        && t.prepare_load(src, &view)
                            .ok()
                            .is_some_and(|loaded| t.install_load(loaded, view).is_ok());
                    if !refreshed {
                        t.mark_source_changed();
                    }
                    !refreshed
                }
                WorkTab::Query(q) => {
                    q.table_saved(&id, revision);
                    false
                }
                _ => false,
            });
            match updated {
                Ok((was_marked, resp)) => {
                    let _ = self.settle(resp);
                    if was_marked {
                        marked += 1;
                    }
                }
                Err(_) if self.ws.get(key).is_some_and(same_table) => marked += 1,
                Err(_) => {}
            }
        }
        self.refresh_explorer_counts();
        self.sync_strip();
        marked
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
                .tabs()
                .any(|t| matches!(t, WorkTab::Query(q) if q.is_running()))
    }

    /// Every tab in strip order.
    pub fn tabs(&self) -> impl Iterator<Item = &WorkTab> + '_ {
        self.ws.iter().map(|(_, tab)| tab)
    }

    pub fn tab_count(&self) -> usize {
        self.ws.len()
    }

    pub fn tab(&self, key: ItemKey) -> Option<&WorkTab> {
        self.ws.get(key)
    }

    pub fn tab_key_at(&self, index: usize) -> Option<ItemKey> {
        self.ws.key_at(index)
    }

    pub fn tab_index(&self, key: ItemKey) -> Option<usize> {
        self.ws.index_of(key)
    }

    pub fn active_key(&self) -> Option<ItemKey> {
        self.ws.active_key()
    }

    pub fn active_tab(&self) -> Option<&WorkTab> {
        self.ws.get(self.ws.active_key()?)
    }

    /// Whether `key` is the one replaceable preview tab.
    pub fn is_preview(&self, key: ItemKey) -> bool {
        self.ws.preview_key() == Some(key)
    }

    /// The table tab `key` names, read-only.
    pub fn table(&self, key: ItemKey) -> Option<&TableTab> {
        match self.ws.get(key)? {
            WorkTab::Table(t) => Some(t),
            _ => None,
        }
    }

    /// The content mutation gateway for one tab. Nothing outside it can hold a
    /// mutable tab: the workspace advances the tab's content revision, promotes
    /// a preview that became dirty, and refuses the call while a close-save
    /// owns the tab (`None`).
    pub fn modify<R>(&mut self, key: ItemKey, f: impl FnOnce(&mut WorkTab) -> R) -> Option<R> {
        let (out, resp) = self.ws.update(key, f).ok()?;
        let _ = self.settle(resp);
        Some(out)
    }

    /// [`Self::modify`] for a table, with the catalog and store its rows are read
    /// from.
    pub fn modify_table<R>(
        &mut self,
        key: ItemKey,
        f: impl FnOnce(&mut TableTab, Source<'_>) -> R,
    ) -> Option<R> {
        let src = Source {
            catalog: &self.catalog,
            store: &self.store,
        };
        let (out, resp) = self
            .ws
            .update(key, |tab| match tab {
                WorkTab::Table(t) => Some(f(t, src)),
                _ => None,
            })
            .ok()?;
        let _ = self.settle(resp);
        out
    }

    pub fn modify_active<R>(&mut self, f: impl FnOnce(&mut WorkTab) -> R) -> Option<R> {
        let key = self.ws.active_key()?;
        self.modify(key, f)
    }

    /// Handles a workspace response in order: refreshes the strip, remembers
    /// closed tables, clears a query's unseen mark when it becomes active, and
    /// hands back what only the host can do.
    pub fn settle(&mut self, resp: Response<WorkspaceAction<WorkTab>>) -> Settled {
        let mut settled = Settled::default();
        let changed_tabs = resp.has_actions();
        for action in resp.into_actions() {
            match action {
                WorkspaceAction::Activated(key) => {
                    if let Ok(mut view) = self.ws.view_mut(key) {
                        view.mark_seen();
                    }
                }
                WorkspaceAction::PreviewPromoted(_) => {}
                WorkspaceAction::CloseRequested(token) => settled.close_requests.push(token),
                WorkspaceAction::CancelOperation(token) => settled.cancelled.push(token),
                WorkspaceAction::Closed { document, .. } => {
                    if let WorkTab::Table(t) = &document {
                        self.closed.push((t.schema.clone(), t.name.clone()));
                    }
                }
            }
        }
        if changed_tabs {
            self.sync_strip();
        }
        settled
    }

    /// Rebuilds the tab strip from the workspace, so its dirty and busy marks
    /// match their drafts again.
    pub fn sync_strip(&mut self) {
        self.ws.sync_tabs_with(&mut self.strip, |tab, mut item| {
            item.busy = false;
            match tab {
                WorkTab::Table(tt) => {
                    item.prefix = Some("T".into());
                    item.dirty = tt.dirty_count() > 0;
                    if tt.source_changed() {
                        item.suffix = Some("source changed".into());
                    }
                }
                WorkTab::Query(q) => {
                    item.prefix = Some("≡".into());
                    item.busy = q.is_running();
                    item.dirty = q.dirty() && !q.is_running();
                    item.error =
                        q.last_status.as_ref().is_some_and(|s| s.1) && !q.is_running() && q.unseen;
                }
                WorkTab::History(_) => {
                    item.prefix = Some("H".into());
                    item.dirty = false;
                }
            }
            item
        });
        let labels: Vec<String> = self.strip.items.iter().map(|i| i.label.clone()).collect();
        let qualified: Vec<Option<String>> = self
            .tabs()
            .enumerate()
            .map(|(i, tab)| match tab {
                WorkTab::Table(t) if labels.iter().filter(|l| **l == labels[i]).count() > 1 => {
                    Some(t.qualified())
                }
                _ => None,
            })
            .collect();
        for (item, label) in self.strip.items.iter_mut().zip(qualified) {
            if let Some(label) = label {
                item.label = label;
            }
        }
        self.strip.allow_new = true;
        self.current_object = match self.active_tab() {
            Some(WorkTab::Table(t)) => Some((t.schema.clone(), t.name.clone())),
            _ => None,
        };
        self.open_objects = self
            .tabs()
            .filter_map(|t| {
                if let WorkTab::Table(t) = t {
                    Some((t.schema.clone(), t.name.clone()))
                } else {
                    None
                }
            })
            .collect();
    }

    /// Opens (or re-activates) the table's tab and returns its key. A preview
    /// open replaces a clean idle preview; a preview that holds a draft, a
    /// running operation, an open editor or a captured save is pinned first.
    pub fn open_table(&mut self, schema: &str, name: &str, preview: bool) -> Option<ItemKey> {
        let existing = self.ws.iter().find_map(|(key, tab)| {
            matches!(tab, WorkTab::Table(tt) if tt.schema == schema && tt.name == name)
                .then_some(key)
        });
        if let Some(key) = existing {
            let mut resp = self.ws.activate(key);
            if !preview {
                resp |= self.ws.promote(key);
            }
            let _ = self.settle(resp);
            self.sync_strip();
            return Some(key);
        }
        let table = self.catalog.find(Some(schema), name)?.clone();
        if preview && let Some(old) = self.ws.preview_key() {
            let keep = matches!(
                self.ws.get(old),
                Some(WorkTab::Table(t)) if t.is_editing() || t.saving()
            );
            if keep {
                let resp = self.ws.promote(old);
                let _ = self.settle(resp);
            }
        }
        let src = Source {
            catalog: &self.catalog,
            store: &self.store,
        };
        let (key, resp) = self.ws.open_with(
            |key| WorkTab::Table(TableTab::new(key, tab_id("table", key), src, &table)),
            if preview {
                OpenKind::Preview
            } else {
                OpenKind::Pinned
            },
        );
        let _ = self.settle(resp);
        Some(key)
    }

    pub fn new_query(&mut self, text: &str) -> ItemKey {
        self.query_counter += 1;
        let name = format!("Query {}", self.query_counter);
        let (key, resp) = self.ws.open_with(
            |key| WorkTab::Query(QueryTab::new(key, tab_id("query", key), &name, text)),
            OpenKind::Pinned,
        );
        let _ = self.settle(resp);
        key
    }

    pub fn open_history(&mut self, history: &History) -> ItemKey {
        let existing = self
            .ws
            .iter()
            .find_map(|(key, tab)| matches!(tab, WorkTab::History(_)).then_some(key));
        if let Some(key) = existing {
            let resp = self.ws.activate(key);
            let connection = self.connection.name.clone();
            let _ = self.modify(key, |tab| {
                if let WorkTab::History(h) = tab {
                    h.refresh(history, &connection);
                }
            });
            let _ = self.settle(resp);
            self.sync_strip();
            return key;
        }
        let connection = self.connection.name.clone();
        let (key, resp) = self.ws.open_with(
            |key| {
                let mut h = HistoryTab::new(key, ID.sub("history"));
                h.refresh(history, &connection);
                WorkTab::History(h)
            },
            OpenKind::Pinned,
        );
        let _ = self.settle(resp);
        key
    }

    /// Makes `key` the active tab. A query's unseen mark clears even when the
    /// tab was already active.
    pub fn activate(&mut self, key: ItemKey) -> bool {
        if !self.ws.contains(key) {
            return false;
        }
        let resp = self.ws.activate(key);
        if let Ok(mut view) = self.ws.view_mut(key) {
            view.mark_seen();
        }
        let _ = self.settle(resp);
        self.sync_strip();
        true
    }

    /// Activates the tab at strip position `index`. The position came from the
    /// strip's current projection; it is translated to a key before anything
    /// changes.
    pub fn activate_index(&mut self, index: usize) -> bool {
        self.ws.key_at(index).is_some_and(|key| self.activate(key))
    }

    /// A second table tab on the table, standing in for another view of the
    /// same rows. The explorer opens one tab per table, so only tests get here.
    #[cfg(test)]
    pub fn open_second_view(&mut self, schema: &str, name: &str) -> Option<ItemKey> {
        let table = self.catalog.find(Some(schema), name)?.clone();
        let src = Source {
            catalog: &self.catalog,
            store: &self.store,
        };
        let (key, resp) = self.ws.open_with(
            |key| WorkTab::Table(TableTab::new(key, tab_id("table", key), src, &table)),
            OpenKind::Pinned,
        );
        let _ = self.settle(resp);
        Some(key)
    }

    /// Asks to close tab `key`. A clean tab closes at once, and a query that
    /// is only running is discarded, which cancels the run. A dirty tab stays
    /// and its close token comes back for the host to confirm. A table that is
    /// saving refuses. A live editor is finished first, so its edit is part of
    /// the draft before the request captures the content revision, and a
    /// rejected input keeps the tab open with no request.
    pub fn request_close(&mut self, key: ItemKey) -> Result<Vec<CloseToken>, String> {
        if matches!(self.ws.get(key), Some(WorkTab::Table(t)) if t.saving()) {
            return Err(SAVING_NOTICE.to_owned());
        }
        let finished = self.modify(key, |tab| match tab {
            WorkTab::Table(t) => t
                .grid
                .finish_edit_session()
                .map(|_| ())
                .map_err(|e| e.to_string()),
            _ => Ok(()),
        });
        if let Some(Err(message)) = finished {
            return Err(format!("Fix the open cell first: {message}"));
        }
        let resp = self.ws.request_close(key);
        let settled = self.settle(resp);
        let mut confirm = vec![];
        for token in settled.close_requests {
            if self.ws.get(token.document).is_some_and(WorkTab::dirty) {
                confirm.push(token);
            } else if let Ok(resp) = self.ws.resolve_close(token, CloseDecision::Discard) {
                let _ = self.settle(resp);
            }
        }
        Ok(confirm)
    }

    /// [`Self::request_close`] for a handler that owns a `Cx`: the answer goes
    /// out as a confirmation request or a status.
    fn close_via(&mut self, key: ItemKey, cx: &mut Cx) -> Outcome {
        match self.request_close(key) {
            Ok(tokens) => {
                for token in tokens {
                    cx.go(crate::app::Act::ConfirmClose(token));
                }
            }
            Err(notice) => cx.status(notice),
        }
        Outcome::Changed
    }

    /// Refreshes every history tab against `history`.
    pub fn refresh_history(&mut self, history: &History) {
        let connection = self.connection.name.clone();
        let keys: Vec<ItemKey> = self
            .ws
            .iter()
            .filter(|(_, tab)| matches!(tab, WorkTab::History(_)))
            .map(|(key, _)| key)
            .collect();
        for key in keys {
            let _ = self.modify(key, |tab| {
                if let WorkTab::History(h) = tab {
                    h.refresh(history, &connection);
                }
            });
        }
    }

    /// Polls every running query tab and returns the history entries the
    /// statements made. A run that ends here ends its workspace operation.
    /// Polling goes through the view facet: it never advances a tab's content
    /// revision, so a close prompt that is open stays valid.
    pub fn tick_queries(&mut self) -> Vec<crate::model::HistoryEntry> {
        let conn = self.connection.name.clone();
        let db = self.catalog.database.clone();
        let running: Vec<ItemKey> = self
            .ws
            .iter()
            .filter(|(_, tab)| matches!(tab, WorkTab::Query(q) if q.is_running()))
            .map(|(key, _)| key)
            .collect();
        let mut entries = vec![];
        for key in running {
            let src = Source {
                catalog: &self.catalog,
                store: &self.store,
            };
            let Ok((made, still_running)) = self
                .ws
                .view_mut(key)
                .map(|mut view| view.tick(src, &conn, &db))
            else {
                continue;
            };
            entries.extend(made);
            if !still_running && let Some(token) = self.ws.running(key) {
                let _ = self.ws.end_operation(token, |_| ());
            }
        }
        entries
    }

    /// Starts a query run in tab `key` under a workspace operation, so closing
    /// or replacing the tab names the run it cancels.
    pub fn start_query(
        &mut self,
        key: ItemKey,
        statements: Vec<(String, std::ops::Range<usize>)>,
        all: bool,
        explain: Option<bool>,
    ) -> bool {
        let Ok((_, resp)) = self.ws.begin_operation(key) else {
            return false;
        };
        let _ = self.settle(resp);
        let started = self.modify(key, |tab| {
            if let WorkTab::Query(q) = tab {
                q.start(statements, all, explain);
            }
        });
        if started.is_none() {
            let _ = self.ws.cancel_operation(key);
            return false;
        }
        true
    }

    /// Cancels the first running query, if any.
    pub fn cancel_running_query(&mut self) -> bool {
        let keys: Vec<ItemKey> = self
            .ws
            .iter()
            .filter(|(_, tab)| matches!(tab, WorkTab::Query(q) if q.is_running()))
            .map(|(key, _)| key)
            .collect();
        keys.into_iter().any(|key| self.cancel_query(key))
    }

    /// Cancels the running query of tab `key`, if any.
    pub fn cancel_query(&mut self, key: ItemKey) -> bool {
        let cancelled = self
            .modify(key, |tab| match tab {
                WorkTab::Query(q) => q.cancel(),
                _ => false,
            })
            .unwrap_or(false);
        if cancelled {
            let _ = self.ws.cancel_operation(key);
        }
        cancelled
    }

    pub fn primary_focus(&self) -> Option<WidgetId> {
        self.active_tab().map(WorkTab::primary_focus)
    }

    pub fn is_editing(&self) -> bool {
        self.explorer_filter.editing || self.active_tab().is_some_and(|t| t.is_editing())
    }

    pub fn pending_total(&self) -> usize {
        self.tabs()
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
        self.tabs()
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
        self.tabs().find_map(|t| match t {
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
        self.dispatch_key(key, cx, history)
    }

    fn dispatch_key(&mut self, key: &Key, cx: &mut Cx, history: &mut History) -> Outcome {
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
                Some(TabAction::Activated(i)) => {
                    self.activate_index(i);
                }
                Some(TabAction::Close(i)) => {
                    return match self.ws.key_at(i) {
                        Some(tab) => self.close_via(tab, cx),
                        None => Outcome::Ignored,
                    };
                }
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
        let Some(active) = self.ws.active_key() else {
            return Outcome::Ignored;
        };
        let cat = &self.catalog;
        let conn = self.connection.name.clone();
        let db = self.catalog.database.clone();
        let edited = self.ws.update(active, |tab| match tab {
            WorkTab::Table(t) => Self::table_key(t, key, f, cx, cat, &conn, &db, history),
            WorkTab::Query(q) => q.on_key(
                key,
                cx,
                Source {
                    catalog: cat,
                    store: &self.store,
                },
            ),
            WorkTab::History(h) => Self::history_key(h, key, f, cx, history, &conn),
        });
        match edited {
            Ok((out, resp)) => {
                let _ = self.settle(resp);
                out
            }
            Err(_) => self.navigate_locked(active, key, cx),
        }
    }

    /// A close-save owns the tab: mutation keys are refused with the saving
    /// notice, while view-only navigation still reaches the tab.
    fn navigate_locked(&mut self, active: ItemKey, key: &Key, cx: &mut Cx) -> Outcome {
        let Ok(mut view) = self.ws.view_mut(active) else {
            return Outcome::Ignored;
        };
        if view.navigate(key, cx.focus.current()) {
            return Outcome::Changed;
        }
        cx.status(SAVING_NOTICE);
        Outcome::Consumed
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
            if let Some(ev) = ev {
                Self::chip_event(t, ev, cx);
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

    /// Asks the app to change the table's rows or controls. The grid and chips
    /// may have moved ahead of the change, so they go back to the shown view;
    /// installing the change moves them together with the rows.
    fn request_change(t: &mut TableTab, cx: &mut Cx, change: TableChange) {
        t.restore_view_controls();
        cx.go(crate::app::Act::TableChange(change));
    }

    /// The one handler for a filter chip gesture, from the keyboard or the pointer.
    fn chip_event(t: &mut TableTab, ev: junie_tui::widgets::chips::ChipAction, cx: &mut Cx) {
        use junie_tui::widgets::chips::ChipAction;
        match ev {
            ChipAction::Activate(i) => cx.go(crate::app::Act::EditFilter(Some(i))),
            ChipAction::Add => cx.go(crate::app::Act::EditFilter(None)),
            other => {
                if let Some(change) = TableChange::from_chip(t, &other) {
                    Self::request_change(t, cx, change);
                }
            }
        }
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
                let change = TableChange::sort(t, s);
                Self::request_change(t, cx, change);
            }
            GridAction::FetchMore => {
                cx.status("Showing the first 500 rows · add a LIMIT or filter to narrow");
                t.grid.set_loading(false);
            }
            GridAction::Refresh => {
                let change = TableChange::refresh(t, "Refreshed");
                Self::request_change(t, cx, change);
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
                let mut next = t.view().clone();
                next.filters.clear();
                let change = TableChange::filters(t, next, Some("Filters cleared".to_owned()));
                Self::request_change(t, cx, change);
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
                KeyCode::Enter => {
                    if let Some(e) = h.current_entry(history) {
                        cx.go(crate::app::Act::OpenQuery(e.sql.clone(), false));
                    }
                    return Outcome::Changed;
                }
                KeyCode::Char('r') => {
                    if let Some(e) = h.current_entry(history) {
                        cx.go(crate::app::Act::OpenQuery(e.sql.clone(), true));
                    }
                    return Outcome::Changed;
                }
                KeyCode::Char('y') => {
                    if let Some(e) = h.current_entry(history) {
                        cx.copy_as(e.sql.clone(), "query");
                    }
                    return Outcome::Changed;
                }
                _ => {}
            }
            let o = h.list.on_key(key);
            h.sync_detail_public(history);
            return o;
        }
        if f == h.detail.id {
            return h.detail.on_key(key).0;
        }
        let mut hit: Option<(Outcome, bool, bool)> = None;
        for (b, run) in [(&mut h.open_btn, false), (&mut h.rerun_btn, true)] {
            if f == b.id {
                let (o, act) = b.on_key(key);
                hit = Some((o, act, run));
                break;
            }
        }
        if let Some((o, act, run)) = hit {
            if act && let Some(e) = h.current_entry(history) {
                cx.go(crate::app::Act::OpenQuery(e.sql.clone(), run));
            }
            return o;
        }
        if f == h.copy_btn.id {
            let (o, act) = h.copy_btn.on_key(key);
            if act && let Some(e) = h.current_entry(history) {
                cx.copy_as(e.sql.clone(), "query");
            }
            return o;
        }
        Outcome::Ignored
    }

    /// Runs `f` with the pointer fields a press, click or drag may reach: the
    /// explorer filter, and the history search when the active tab is one.
    fn with_pointer_fields<R>(&mut self, f: impl FnOnce(&mut [&mut dyn PointerField]) -> R) -> R {
        let search = self.ws.active_key().filter(|key| {
            !self.ws.save_locked(*key) && matches!(self.ws.get(*key), Some(WorkTab::History(_)))
        });
        let filter = &mut self.explorer_filter;
        let Some(key) = search else {
            return f(&mut [filter]);
        };
        let (out, resp) = self
            .ws
            .update(key, |tab| match tab {
                WorkTab::History(h) => f(&mut [filter, &mut h.search]),
                _ => f(&mut [filter]),
            })
            .expect("an open tab that no save holds");
        let _ = self.settle(resp);
        out
    }

    pub fn pointer(&mut self, ev: &PointerEvent, cx: &mut Cx, history: &mut History) -> Outcome {
        self.dispatch_pointer(ev, cx, history)
    }

    fn dispatch_pointer(
        &mut self,
        ev: &PointerEvent,
        cx: &mut Cx,
        history: &mut History,
    ) -> Outcome {
        if let Some(key) = self.ws.active_key()
            && let Ok(mut view) = self.ws.view_mut(key)
            && let Some(o) = view.facts_pointer(ev)
        {
            return o;
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

    /// The active tab's pointer handling for `ev`, through the workspace's
    /// mutation gateway. A tab held by a close-save ignores it.
    fn edit_active_pointer(
        &mut self,
        f: impl FnOnce(&mut WorkTab, &Catalog) -> Outcome,
    ) -> Outcome {
        let Some(key) = self.ws.active_key() else {
            return Outcome::Ignored;
        };
        let cat = &self.catalog;
        match self.ws.update(key, |tab| f(tab, cat)) {
            Ok((out, resp)) => {
                let _ = self.settle(resp);
                out
            }
            Err(_) => Outcome::Consumed,
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
        if let Some(key) = self.ws.active_key()
            && let Ok(mut view) = self.ws.view_mut(key)
            && view.seam_press(id, pos)
        {
            return Outcome::Consumed;
        }
        let count = ev.count();
        if let Some(o) = self.with_pointer_fields(|fields| route_press(fields, id, pos, count)) {
            return o;
        }
        if let Some(r) = self.explorer.route(ev) {
            return r.outcome();
        }
        self.edit_active_pointer(|tab, _| match tab {
            WorkTab::Table(t) => t
                .grid
                .route(ev)
                .map(|r| r.outcome())
                .or_else(|| t.structure.route(ev).map(|r| r.outcome()))
                .or_else(|| t.ddl.route(ev).map(|r| r.outcome()))
                .unwrap_or(Outcome::Ignored),
            WorkTab::Query(q) => q.press(ev),
            WorkTab::History(h) => h
                .list
                .route(ev)
                .map(|r| r.outcome())
                .or_else(|| h.detail.route(ev).map(|r| r.outcome()))
                .unwrap_or(Outcome::Ignored),
        })
    }

    fn middle(&mut self, ev: &PointerEvent, cx: &mut Cx) -> Outcome {
        if let Some(r) = self.strip.route(ev) {
            let (mut out, actions) = r.into_parts();
            for act in actions {
                if let TabAction::Close(i) = act
                    && let Some(key) = self.ws.key_at(i)
                {
                    out = self.close_via(key, cx);
                }
            }
            return out;
        }
        self.edit_active_pointer(|tab, _| match tab {
            WorkTab::Query(q) => q.middle(ev),
            _ => Outcome::Ignored,
        })
    }

    /// Runs `f` on the grid the active tab shows (a table's rows or a query's
    /// active result), through the mutation gateway.
    pub fn modify_active_grid<R>(
        &mut self,
        f: impl FnOnce(&mut junie_tui::widgets::grid::DataGrid) -> R,
    ) -> Option<R> {
        self.modify_active(|tab| match tab {
            WorkTab::Table(t) => Some(f(&mut t.grid)),
            WorkTab::Query(q) => q.active_grid_mut().map(f),
            WorkTab::History(_) => None,
        })
        .flatten()
    }

    pub fn on_header_action(
        &mut self,
        col: usize,
        action: junie_tui::ui::columns::HeaderAction,
        cx: &mut Cx,
        history: &mut History,
    ) -> Outcome {
        self.header_action(col, action, cx, history)
    }

    fn header_action(
        &mut self,
        col: usize,
        action: junie_tui::ui::columns::HeaderAction,
        cx: &mut Cx,
        history: &mut History,
    ) -> Outcome {
        let conn = self.connection.name.clone();
        let db = self.catalog.database.clone();
        self.edit_active_pointer(|tab, cat| match tab {
            WorkTab::Table(t) => {
                let (o, ev) = t.grid.on_header_action(col, action);
                match ev {
                    Some(ev) => Self::grid_event(t, ev, cx, cat, &conn, &db, history).or(o),
                    None => o,
                }
            }
            WorkTab::Query(q) => q
                .active_grid_mut()
                .map_or(Outcome::Ignored, |g| g.on_header_action(col, action).0),
            WorkTab::History(_) => Outcome::Ignored,
        })
    }

    fn click(&mut self, ev: &PointerEvent, cx: &mut Cx, history: &mut History) -> Outcome {
        let (id, pos) = (ev.id, ev.pos());
        let count = ev.count();
        if let Some(o) = self.with_pointer_fields(|fields| route_click(fields, id, pos, count)) {
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
                        self.activate_index(i);
                        if let Some(pf) = self.primary_focus() {
                            cx.focus.focus(pf);
                        }
                    }
                    TabAction::Close(i) => {
                        return match self.ws.key_at(i) {
                            Some(tab) => self.close_via(tab, cx),
                            None => Outcome::Ignored,
                        };
                    }
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
        let conn = self.connection.name.clone();
        let db = self.catalog.database.clone();
        self.edit_active_pointer(|tab, cat| match tab {
            WorkTab::Table(t) => {
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
                        Self::chip_event(t, act, cx);
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
            WorkTab::Query(q) => q.click(ev, cx, cat),
            WorkTab::History(h) => {
                if id == h.seam.id {
                    return h.seam.on_click(&mut h.split, ev.count());
                }
                if ev.on(h.list.id) {
                    if ev.index_of(Part::ROW).is_some() {
                        cx.focus.focus(h.list.id);
                    }
                    let o = h.list.pointer(ev).outcome();
                    h.sync_detail_public(history);
                    return o;
                }
                if ev.on(h.detail.id) {
                    if id == h.detail.id {
                        cx.focus.focus(h.detail.id);
                    }
                    return h.detail.pointer(ev).outcome();
                }
                let mut hit: Option<bool> = None;
                for (b, run) in [(&mut h.open_btn, false), (&mut h.rerun_btn, true)] {
                    if b.id == id && b.on_click() {
                        hit = Some(run);
                        break;
                    }
                }
                if let Some(run) = hit {
                    if let Some(e) = h.current_entry(history) {
                        cx.go(crate::app::Act::OpenQuery(e.sql.clone(), run));
                    }
                    return Outcome::Changed;
                }
                if h.copy_btn.id == id && h.copy_btn.on_click() {
                    if let Some(e) = h.current_entry(history) {
                        cx.copy_as(e.sql.clone(), "query");
                    }
                    return Outcome::Changed;
                }
                Outcome::Ignored
            }
        })
    }

    fn drag(&mut self, ev: &PointerEvent) -> Outcome {
        let (pressed, pos) = (ev.id, ev.pos());
        if let Some(o) = self.with_pointer_fields(|fields| route_drag(fields, pressed, pos)) {
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
        if let Some(key) = self.ws.active_key()
            && let Ok(mut view) = self.ws.view_mut(key)
            && let Some(o) = view.seam_drag(pressed, pos)
        {
            return o;
        }
        self.edit_active_pointer(|tab, _| match tab {
            WorkTab::Table(t) => t
                .grid
                .route(ev)
                .map(|r| r.outcome())
                .or_else(|| t.structure.route(ev).map(|r| r.outcome()))
                .or_else(|| t.ddl.route(ev).map(|r| r.outcome()))
                .unwrap_or(Outcome::Ignored),
            WorkTab::Query(q) => q.drag(ev),
            WorkTab::History(h) => h
                .list
                .route(ev)
                .map(|r| r.outcome())
                .or_else(|| h.detail.route(ev).map(|r| r.outcome()))
                .unwrap_or(Outcome::Ignored),
        })
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
        let Some(key) = self.ws.active_key() else {
            return Outcome::Ignored;
        };
        match self.ws.view_mut(key) {
            Ok(mut view) => view.wheel(ev),
            Err(_) => Outcome::Ignored,
        }
    }

    pub fn on_blur(&mut self, ch: junie_tui::core::focus::FocusChange) -> Outcome {
        use junie_tui::core::focus::forward_blur;
        let mut out = forward_blur(ch, &mut [&mut self.explorer_filter]);
        for key in self.ws.ordered_keys().to_vec() {
            if !self.ws.get(key).is_some_and(WorkTab::is_editing) {
                continue;
            }
            let blurred = self.modify(key, |tab| match tab {
                WorkTab::Table(t) => forward_blur(ch, &mut [&mut t.grid, &mut t.structure]),
                WorkTab::History(h) => forward_blur(ch, &mut [&mut h.search]),
                WorkTab::Query(_) => Outcome::Ignored,
            });
            out = out.or(blurred.unwrap_or(Outcome::Ignored));
        }
        out
    }

    pub fn on_paste(&mut self, text: &str, cx: &mut Cx) -> Outcome {
        if self.explorer_filter.editing {
            return self.explorer_filter.on_paste(text);
        }
        self.edit_active_pointer(|tab, _| match tab {
            WorkTab::Table(t) => match t.grid.on_paste(text) {
                Outcome::Ignored if cx.focus.is(t.grid.id) => {
                    let (out, actions) = t.grid.paste_into_cell(text).into_parts();
                    for ev in actions {
                        if let junie_tui::widgets::grid::GridAction::ReadOnly { reason } = ev {
                            cx.status(reason);
                        }
                    }
                    out
                }
                out => out,
            },
            WorkTab::Query(q) => q.on_paste(text),
            WorkTab::History(h) => h.search.on_paste(text),
        })
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
        let Some(active_key) = self.ws.active_key() else {
            let panel = Panel::framed(None);
            let inner = panel.render(main, buf, t);
            junie_tui::widgets::empty::render(inner, buf, t, &EmptyState::new("No open tabs").hint("Enter on a table in the explorer opens it · Ctrl+T starts a query · Ctrl+O opens anything"), t.canvas);
            return;
        };
        let Some(active) = self.ws.get(active_key) else {
            return;
        };
        let preview = self.is_preview(active_key);
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
                if preview { "preview" } else { "" },
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
        if let Ok(mut view) = self.ws.view_mut(active_key) {
            view.render(inner, buf, ctx, bg, history, &conn);
        }
    }
}

impl HistoryTab {
    pub fn sync_detail_public(&mut self, history: &History) {
        let text = self
            .current_entry(history)
            .map(|e| e.sql.clone())
            .unwrap_or_default();
        if self.detail.text() != text {
            self.detail.set_text(&text);
        }
    }
}
