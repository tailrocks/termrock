// SPDX-FileCopyrightText: 2026 Alexey Zhokhov
// SPDX-License-Identifier: Apache-2.0
//
// Adapted from junie-tui src/bin/tablepro/workbench.rs (MIT),
// https://github.com/donbeave/terminal-components-claude

//! Workbench: explorer pane + tab strip + tab bodies for one connection.

use std::collections::HashSet;
use std::ops::Range;

use ratatui::buffer::Buffer;
use ratatui::layout::Rect;
use ratatui::text::{Line, Span};
use ratatui::widgets::StatefulWidget;
use termrock::input::{KeyCode, KeyEventKind};
use termrock::style::PanelChrome;
use termrock::widgets::{
    Panel, PanelVariant, Tab, Tabs, TabsOutcome, TabsState, TextInput, TextInputState, Tree,
    TreeNode, TreeOutcome, TreeState,
};

use super::db::{Catalog, Connection, ObjectKind};
use super::model::History;
use super::tabs::{
    HistoryTab, QueryTab, TableTab, handle_history, handle_query, handle_table, query_hints,
    render_history, render_query, render_table,
};
use crate::ctx::RenderCtx;
use crate::id::WidgetId;
use crate::layout;
use crate::outcome::Route;
use crate::page::{Hint, PageCtx, PageEvent};

const ID: WidgetId = WidgetId::of("workbench");
pub const EXPLORER: WidgetId = ID.sub("explorer");
pub const TABSTRIP: WidgetId = ID.sub("tabstrip");
const FILTER: WidgetId = ID.sub("filter");

pub type PendingRun = (usize, Vec<(String, Range<usize>)>, bool, Option<bool>);

pub enum WorkTab {
    Table(Box<TableTab>),
    Query(Box<QueryTab>),
    History(Box<HistoryTab>),
}

impl WorkTab {
    #[must_use]
    pub fn label(&self) -> String {
        match self {
            WorkTab::Table(t) => t.label(),
            WorkTab::Query(q) => q.name.clone(),
            WorkTab::History(_) => "History".into(),
        }
    }
    #[must_use]
    pub fn is_editing(&self) -> bool {
        match self {
            WorkTab::Table(t) => t.is_editing(),
            WorkTab::Query(q) => q.is_editing(),
            WorkTab::History(h) => h.search.is_editing(),
        }
    }
    #[must_use]
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
    pub catalog: Catalog,
    pub schema: String,
    pub explorer: TreeState<String>,
    pub explorer_filter: TextInputState,
    pub explorer_visible: bool,
    pub maximized: bool,
    pub expanded: HashSet<String>,
    pub strip: TabsState<usize>,
    pub tabs: Vec<WorkTab>,
    pub active: usize,
    pub query_counter: usize,
    pub pending_run: Option<PendingRun>,
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
    #[must_use]
    pub fn new(connection: Connection, catalog: Catalog) -> Self {
        let schema = "public".into();
        let mut expanded = HashSet::new();
        expanded.insert(catalog.database.clone());
        expanded.insert(format!("{}/public", catalog.database));
        expanded.insert(format!("{}/public/Tables", catalog.database));
        let mut filter = TextInputState::new("").with_allow_empty(true);
        filter.set_editing(false);
        let mut wb = Self {
            connection,
            catalog,
            schema,
            explorer: TreeState::new(None),
            explorer_filter: filter,
            explorer_visible: true,
            maximized: false,
            expanded,
            strip: TabsState::new(),
            tabs: vec![],
            active: 0,
            query_counter: 0,
            pending_run: None,
        };
        wb.explorer = TreeState::new(Some(format!("{}/public", wb.catalog.database)));
        wb
    }

    pub fn new_query(&mut self, sql: &str) {
        self.query_counter += 1;
        let name = format!("Query {}", self.query_counter);
        self.tabs
            .push(WorkTab::Query(Box::new(QueryTab::new(name, sql))));
        self.active = self.tabs.len() - 1;
        self.sync_strip();
    }

    pub fn open_table(&mut self, schema: &str, name: &str) {
        if let Some(i) = self
            .tabs
            .iter()
            .position(|t| matches!(t, WorkTab::Table(tt) if tt.schema == schema && tt.name == name))
        {
            self.active = i;
            self.sync_strip();
            return;
        }
        if let Some(table) = self.catalog.find(Some(schema), name).cloned() {
            self.tabs
                .push(WorkTab::Table(Box::new(TableTab::new(&table))));
            self.active = self.tabs.len() - 1;
            self.sync_strip();
        }
    }

    pub fn open_history(&mut self) {
        if let Some(i) = self
            .tabs
            .iter()
            .position(|t| matches!(t, WorkTab::History(_)))
        {
            self.active = i;
        } else {
            self.tabs
                .push(WorkTab::History(Box::new(HistoryTab::new())));
            self.active = self.tabs.len() - 1;
        }
        self.sync_strip();
    }

    fn sync_strip(&mut self) {
        self.strip.set_selected(Some(self.active));
    }

    #[must_use]
    pub fn is_editing(&self) -> bool {
        self.explorer_filter.is_editing()
            || self.tabs.get(self.active).is_some_and(WorkTab::is_editing)
    }

    #[must_use]
    pub fn animating(&self) -> bool {
        self.tabs
            .iter()
            .any(|t| matches!(t, WorkTab::Query(q) if q.is_running()))
    }

    #[must_use]
    pub fn running(&self) -> Option<&str> {
        self.tabs.iter().find_map(|t| match t {
            WorkTab::Query(q) if q.is_running() => Some(q.name.as_str()),
            _ => None,
        })
    }

    #[must_use]
    pub fn pending_total(&self) -> usize {
        self.tabs
            .iter()
            .map(|t| match t {
                WorkTab::Table(tt) => tt.dirty_count(),
                _ => 0,
            })
            .sum()
    }

    pub fn active_query_mut(&mut self) -> Option<&mut QueryTab> {
        match self.tabs.get_mut(self.active) {
            Some(WorkTab::Query(q)) => Some(q.as_mut()),
            _ => None,
        }
    }

    #[must_use]
    pub fn active_tab(&self) -> Option<&WorkTab> {
        self.tabs.get(self.active)
    }

    fn explorer_nodes(&self) -> Vec<(String, String, u16, bool, bool, &'static str)> {
        let db = &self.catalog.database;
        let q = self.explorer_filter.trimmed_value().to_ascii_lowercase();
        let mut out = Vec::new();
        let db_exp = self.expanded.contains(db);
        out.push((db.clone(), db.clone(), 0, true, db_exp, "D"));
        if !db_exp {
            return out;
        }
        for schema in &self.catalog.schemas {
            let sid = format!("{db}/{schema}");
            let exp = self.expanded.contains(&sid);
            out.push((sid.clone(), schema.clone(), 1, true, exp, "S"));
            if !exp {
                continue;
            }
            for (kind, label) in [
                (ObjectKind::Table, "Tables"),
                (ObjectKind::View, "Views"),
                (ObjectKind::Function, "Functions"),
                (ObjectKind::Sequence, "Sequences"),
            ] {
                let objs: Vec<_> = self.catalog.tables_in(schema, kind).collect();
                if objs.is_empty() && kind != ObjectKind::Table {
                    continue;
                }
                let kid = format!("{sid}/{label}");
                let kexp = self.expanded.contains(&kid);
                out.push((kid.clone(), label.to_owned(), 2, true, kexp, "·"));
                if !kexp {
                    continue;
                }
                for t in objs {
                    if !q.is_empty() && !t.name.to_ascii_lowercase().contains(&q) {
                        continue;
                    }
                    let tid = format!("{kid}/{}", t.name);
                    out.push((tid, t.name.clone(), 3, false, false, kind_glyph(t.kind)));
                }
            }
        }
        out
    }

    pub fn tick(&mut self, history: &mut History) -> bool {
        let conn = self.connection.name.clone();
        let db = self.catalog.database.clone();
        let mut changed = false;
        for tab in &mut self.tabs {
            if let WorkTab::Query(q) = tab {
                changed |= q.tick(&self.catalog, &conn, &db, history);
            }
        }
        changed
    }

    pub fn primary_focus(&self) -> Option<WidgetId> {
        match self.tabs.get(self.active)? {
            WorkTab::Table(_) => Some(super::tabs::TABLE_GRID),
            WorkTab::Query(_) => Some(super::tabs::EDITOR),
            WorkTab::History(_) => Some(super::tabs::HIST_LIST),
        }
    }

    pub fn render(&mut self, area: Rect, buf: &mut Buffer, ctx: &mut RenderCtx<'_>) {
        let t = ctx.theme;
        // Tab strip is full width; explorer sits in the remaining body.
        let strip = Rect::new(area.x, area.y, area.width, 2);
        let pane = Rect::new(
            area.x,
            area.y.saturating_add(2),
            area.width,
            area.height.saturating_sub(2),
        );
        let narrow = area.width < 100;
        let explorer_focused = ctx.interaction.focused(EXPLORER) || ctx.interaction.focused(FILTER);
        let show_explorer =
            self.explorer_visible && !self.maximized && (!narrow || explorer_focused);
        let explorer_w = (pane.width / 4).clamp(28, 40);
        let (ex, main) = if show_explorer && narrow {
            (pane, Rect::ZERO)
        } else if show_explorer {
            (
                Rect::new(pane.x, pane.y, explorer_w, pane.height),
                Rect::new(
                    pane.x.saturating_add(explorer_w).saturating_add(1),
                    pane.y,
                    pane.width.saturating_sub(explorer_w.saturating_add(1)),
                    pane.height,
                ),
            )
        } else {
            (Rect::ZERO, pane)
        };
        if !ex.is_empty() {
            self.render_explorer(ex, buf, ctx);
        }
        let labels: Vec<String> = self.tabs.iter().map(WorkTab::label).collect();
        let tab_defs: Vec<Tab<usize>> = self
            .tabs
            .iter()
            .zip(labels.iter())
            .enumerate()
            .map(|(i, (tab, label))| {
                let glyph = match tab {
                    WorkTab::Table(_) => "T",
                    WorkTab::Query(_) => "≡",
                    WorkTab::History(_) => "H",
                };
                Tab::new(i, label.as_str())
                    .closable(true)
                    .glyph(Span::raw(glyph))
            })
            .collect();
        self.strip.set_focused(ctx.interaction.focused(TABSTRIP));
        self.strip.set_selected(Some(self.active));
        if tab_defs.is_empty() {
            buf.set_string(strip.x, strip.y, "No tabs — Ctrl+T new query", t.muted());
        } else {
            Tabs::new(&tab_defs, ctx.system)
                .show_close(true)
                .paint(strip, buf, &mut self.strip);
            let plus = Rect::new(strip.right().saturating_sub(4), strip.y, 3, 1);
            buf.set_string(plus.x, plus.y, " + ", t.muted().bg(t.canvas));
            ctx.clickable(TABSTRIP.sub("new"), plus);
        }
        ctx.control(TABSTRIP, strip, false);
        if main.is_empty() {
            return;
        }
        let title = match self.tabs.get(self.active) {
            Some(WorkTab::Table(tt)) => format!("{} › {}", tt.schema, tt.name),
            Some(WorkTab::Query(q)) => q.name.clone(),
            Some(WorkTab::History(_)) => "Query history".into(),
            None => String::new(),
        };
        let meta = match self.tabs.get(self.active) {
            Some(WorkTab::Query(q)) => q.last_duration.map(super::tabs::duration_label),
            Some(WorkTab::Table(tt)) => Some(format!("{} cols", tt.grid.columns.len())),
            Some(WorkTab::History(_)) => Some(format!("{} entries", 0)),
            None => None,
        };
        let focus_in_tab = ctx
            .interaction
            .focus
            .is_some_and(|f| f != EXPLORER && f != FILTER && f != TABSTRIP);
        let mut panel = Panel::new(ctx.system)
            .variant(PanelVariant::Bordered)
            .emphasis(if focus_in_tab {
                PanelChrome::Focused
            } else {
                PanelChrome::Normal
            });
        if !title.is_empty() {
            panel = panel.title(&title);
        }
        if let Some(m) = meta.as_deref().filter(|s| !s.is_empty()) {
            panel = panel.trailing(m);
        }
        panel.paint(main, buf, None);
        let pane = Panel::new(ctx.system)
            .variant(PanelVariant::Bordered)
            .inner(main);
        match self.tabs.get_mut(self.active) {
            Some(WorkTab::Query(q)) => render_query(q, pane, buf, ctx),
            Some(WorkTab::Table(tt)) => {
                if let Some(table) = self.catalog.find(Some(&tt.schema), &tt.name).cloned() {
                    render_table(tt, &table, pane, buf, ctx);
                }
            }
            Some(WorkTab::History(h)) => {
                // history is borrowed from App; caller paints via render_history_with
                let _ = h;
                let (inner, bg) = layout::card(pane, buf, t, Some("History"), None, false);
                buf.set_string(inner.x, inner.y, "Open with Ctrl+Y", t.muted().bg(bg));
            }
            None => {
                let (inner, bg) = layout::card(pane, buf, t, Some("Empty"), None, false);
                buf.set_string(
                    inner.x,
                    inner.y,
                    "Ctrl+T new query · Enter on a table to browse",
                    t.muted().bg(bg),
                );
            }
        }
    }

    pub fn render_history_tab(
        &mut self,
        history: &History,
        area: Rect,
        buf: &mut Buffer,
        ctx: &mut RenderCtx<'_>,
    ) {
        if let Some(WorkTab::History(h)) = self.tabs.get_mut(self.active) {
            let explorer_w = if self.explorer_visible && area.width >= 90 {
                28
            } else if self.explorer_visible {
                22
            } else {
                0
            };
            let body_x = if explorer_w > 0 {
                area.x + explorer_w + 1
            } else {
                area.x
            };
            let pane = Rect::new(
                body_x,
                area.y + 3,
                area.width
                    .saturating_sub(if explorer_w > 0 { explorer_w + 1 } else { 0 }),
                area.height.saturating_sub(3),
            );
            render_history(h, history, pane, buf, ctx);
        }
    }

    fn render_explorer(&mut self, area: Rect, buf: &mut Buffer, ctx: &mut RenderCtx<'_>) {
        let t = ctx.theme;
        let rows = layout::rows(area, &[2, 0]);
        self.explorer_filter
            .set_focused(ctx.interaction.focused(FILTER));
        let _ = TextInput::new("", ctx.system)
            .placeholder("Filter objects")
            .paint(rows[0], buf, &mut self.explorer_filter);
        ctx.control(FILTER, rows[0], false);
        let vis = self.explorer_nodes();
        let nodes: Vec<TreeNode<'_, String>> = vis
            .iter()
            .map(|(id, label, depth, branch, expanded, glyph)| {
                let mut n = TreeNode::new(id.clone(), Line::from(label.as_str()), *depth)
                    .leading(Line::from(*glyph));
                if *branch {
                    n = n.branch();
                    if *expanded {
                        n = n.expanded();
                    }
                }
                n
            })
            .collect();
        let (inner, _bg) = layout::card(
            rows[1],
            buf,
            t,
            Some("Explorer"),
            None,
            ctx.interaction.focused(EXPLORER),
        );
        StatefulWidget::render(
            &Tree::new(&nodes, ctx.system).focused(ctx.interaction.focused(EXPLORER)),
            inner,
            buf,
            &mut self.explorer,
        );
        ctx.control(EXPLORER, inner, false);
        ctx.scrollable(EXPLORER, inner);
    }

    pub fn handle(&mut self, ev: &PageEvent, cx: &mut PageCtx<'_>, history: &History) -> Route {
        let _ = history;
        match ev {
            PageEvent::Key(key) if key.kind != KeyEventKind::Release => {
                let Some(f) = *cx.focus else {
                    return Route::Ignored;
                };
                if f == FILTER {
                    let o = self.explorer_filter.handle_key(*key);
                    return if matches!(o, termrock::widgets::TextInputOutcome::Ignored) {
                        Route::Ignored
                    } else {
                        Route::Changed
                    };
                }
                if f == EXPLORER {
                    return self.handle_explorer_key(*key, cx);
                }
                if f == TABSTRIP {
                    let labels: Vec<String> = self.tabs.iter().map(WorkTab::label).collect();
                    let tab_defs: Vec<Tab<usize>> = labels
                        .iter()
                        .enumerate()
                        .map(|(i, l)| Tab::new(i, l.as_str()).closable(true))
                        .collect();
                    match self.strip.handle_key(*key, &tab_defs) {
                        TabsOutcome::SelectionChanged { id } => {
                            self.active = id;
                            return Route::Changed;
                        }
                        TabsOutcome::CloseRequested { id } => {
                            self.close_tab(id);
                            return Route::Changed;
                        }
                        TabsOutcome::Ignored => return Route::Ignored,
                        _ => return Route::Changed,
                    }
                }
                match self.tabs.get_mut(self.active) {
                    Some(WorkTab::Query(q)) => return handle_query(q, ev, cx),
                    Some(WorkTab::Table(tt)) => return handle_table(tt, ev, cx),
                    Some(WorkTab::History(h)) => return handle_history(h, ev, cx),
                    None => {}
                }
                Route::Ignored
            }
            PageEvent::Click { id, pos } => {
                if *id == EXPLORER {
                    cx.set_focus(EXPLORER);
                    let o = self.explorer.click(*pos);
                    return self.apply_explorer(o, cx);
                }
                if *id == FILTER {
                    cx.set_focus(FILTER);
                    self.explorer_filter.begin_edit();
                    return Route::Changed;
                }
                if *id == TABSTRIP {
                    cx.set_focus(TABSTRIP);
                    return Route::Changed;
                }
                match self.tabs.get_mut(self.active) {
                    Some(WorkTab::Query(q)) => handle_query(q, ev, cx),
                    Some(WorkTab::Table(tt)) => handle_table(tt, ev, cx),
                    Some(WorkTab::History(h)) => handle_history(h, ev, cx),
                    None => Route::Ignored,
                }
            }
            PageEvent::Wheel { id, delta } if *id == EXPLORER => {
                let n = self.explorer_nodes().len();
                let _ = self.explorer.scroll_by(*delta as isize, n);
                Route::Changed
            }
            PageEvent::Paste(_) | PageEvent::Wheel { .. } => match self.tabs.get_mut(self.active) {
                Some(WorkTab::Query(q)) => handle_query(q, ev, cx),
                Some(WorkTab::Table(tt)) => handle_table(tt, ev, cx),
                Some(WorkTab::History(h)) => handle_history(h, ev, cx),
                None => Route::Ignored,
            },
            _ => Route::Ignored,
        }
    }

    fn handle_explorer_key(
        &mut self,
        key: termrock::input::KeyEvent,
        cx: &mut PageCtx<'_>,
    ) -> Route {
        if matches!(key.code, KeyCode::Enter) && key.modifiers.is_empty() {
            if let Some(id) = self.explorer.selected().cloned() {
                return self.activate_explorer(&id, cx);
            }
        }
        let vis = self.explorer_nodes();
        let nodes: Vec<TreeNode<'_, String>> = vis
            .iter()
            .map(|(id, label, depth, branch, expanded, glyph)| {
                let mut n = TreeNode::new(id.clone(), Line::from(label.as_str()), *depth)
                    .leading(Line::from(*glyph));
                if *branch {
                    n = n.branch();
                    if *expanded {
                        n = n.expanded();
                    }
                }
                n
            })
            .collect();
        let o = self.explorer.handle_key(&nodes, key);
        self.apply_explorer(o, cx)
    }

    fn apply_explorer(&mut self, o: TreeOutcome<String>, cx: &mut PageCtx<'_>) -> Route {
        match o {
            TreeOutcome::Ignored => Route::Ignored,
            TreeOutcome::Toggle(id) => {
                if self.expanded.contains(&id) {
                    self.expanded.remove(&id);
                } else {
                    self.expanded.insert(id);
                }
                Route::Changed
            }
            TreeOutcome::Activated(id) => self.activate_explorer(&id, cx),
            TreeOutcome::SelectionChanged(_) => Route::Changed,
            _ => Route::Changed,
        }
    }

    fn activate_explorer(&mut self, id: &str, cx: &mut PageCtx<'_>) -> Route {
        // id = db/schema/Tables/name
        let parts: Vec<&str> = id.split('/').collect();
        if parts.len() == 4 && matches!(parts[2], "Tables" | "Views") {
            let schema = parts[1].to_owned();
            let name = parts[3].to_owned();
            self.open_table(&schema, &name);
            cx.status(format!("Opened {schema}.{name}"));
            return Route::Changed;
        }
        if self.expanded.contains(id) {
            self.expanded.remove(id);
        } else {
            self.expanded.insert(id.to_owned());
        }
        Route::Changed
    }

    fn close_tab(&mut self, id: usize) {
        if id >= self.tabs.len() {
            return;
        }
        self.tabs.remove(id);
        if self.active >= self.tabs.len() {
            self.active = self.tabs.len().saturating_sub(1);
        }
        self.sync_strip();
    }

    #[must_use]
    pub fn hints(&self, focus: Option<WidgetId>) -> Vec<Hint> {
        if focus == Some(EXPLORER) || focus == Some(FILTER) {
            return vec![
                ("↑ ↓", "Move"),
                ("Enter", "Open"),
                ("→", "Expand"),
                ("/", "Filter"),
                ("Ctrl+O", "Quick open"),
            ];
        }
        match self.tabs.get(self.active) {
            Some(WorkTab::Query(q)) => query_hints(q),
            Some(WorkTab::Table(_)) => vec![("↑ ↓", "Rows"), ("s", "Structure")],
            Some(WorkTab::History(_)) => vec![("↑ ↓", "Move"), ("Enter", "Open")],
            None => vec![("Ctrl+T", "New query")],
        }
    }
}
