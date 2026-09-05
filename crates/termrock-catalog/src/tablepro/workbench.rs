// SPDX-FileCopyrightText: 2026 Alexey Zhokhov
// SPDX-License-Identifier: Apache-2.0
//
// Adapted from junie-tui src/bin/tablepro/workbench.rs (MIT),
// https://github.com/donbeave/terminal-components-claude

//! Workbench: explorer pane + tab strip + tab bodies for one connection.

use std::collections::HashSet;
use std::ops::Range;

use ratatui::buffer::Buffer;
use ratatui::layout::{Position, Rect};
use ratatui::style::Style;
use ratatui::text::{Line, Span};
use ratatui::widgets::StatefulWidget;
use termrock::input::{KeyCode, KeyEventKind};
use termrock::style::{DesignSystem, PanelChrome};
use termrock::widgets::{
    Panel, PanelVariant, Tab, Tabs, TabsOutcome, TabsState, TextInput, TextInputState, Tree,
    TreeNode, TreeOutcome, TreeState,
};

use super::db::{Catalog, Connection, ObjectKind};
use super::model::History;
use super::tabs::{
    HistoryTab, PLAN, QueryTab, TableTab, handle_history, handle_query, handle_table, query_hints,
    render_history, render_query, render_table, table_hints,
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
    explorer_cursor: Option<Position>,
    pending_close: Option<usize>,
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
            explorer_cursor: None,
            pending_close: None,
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
    pub fn running_duration_ms(&self) -> Option<u32> {
        self.tabs.iter().find_map(|t| match t {
            WorkTab::Query(q) if q.is_running() => Some(q.running_duration_ms()),
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

    fn explorer_nodes(
        &self,
    ) -> Vec<(
        String,
        String,
        u16,
        bool,
        bool,
        &'static str,
        Option<String>,
    )> {
        let db = &self.catalog.database;
        let q = self.explorer_filter.trimmed_value().to_ascii_lowercase();
        let mut out = Vec::new();
        let db_exp = self.expanded.contains(db);
        out.push((db.clone(), db.clone(), 0, true, db_exp, "D", None));
        if !db_exp {
            return out;
        }
        for schema in &self.catalog.schemas {
            let sid = format!("{db}/{schema}");
            let exp = self.expanded.contains(&sid);
            out.push((sid.clone(), schema.clone(), 1, true, exp, "S", None));
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
                out.push((
                    kid.clone(),
                    label.to_owned(),
                    2,
                    true,
                    kexp,
                    "",
                    Some(objs.len().to_string()),
                ));
                if !kexp {
                    continue;
                }
                for t in objs {
                    if !q.is_empty() && !t.name.to_ascii_lowercase().contains(&q) {
                        continue;
                    }
                    let tid = format!("{kid}/{}", t.name);
                    let count = if t.row_count > 0 {
                        Some(super::sql::fmt_rows(t.row_count))
                    } else {
                        None
                    };
                    let branch = matches!(t.kind, ObjectKind::Table | ObjectKind::View);
                    out.push((
                        tid,
                        t.name.clone(),
                        3,
                        branch,
                        false,
                        kind_glyph(t.kind),
                        count,
                    ));
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

    pub fn render(
        &mut self,
        area: Rect,
        buf: &mut Buffer,
        ctx: &mut RenderCtx<'_>,
        history: &History,
    ) {
        self.explorer_cursor = None;
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
        ctx.clickable(TABSTRIP, strip);
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
            Some(WorkTab::Query(q)) => q
                .last_duration
                .map(super::tabs::duration_label)
                .unwrap_or_default(),
            Some(WorkTab::Table(tt)) => format!("{} cols", tt.grid.columns.len()),
            Some(WorkTab::History(_)) => format!("{} entries", history.entries.len()),
            None => String::new(),
        };
        // Source emphasizes the workbench pane only when focus is inside its
        // tab body/strip; explorer and filter own their own focus chrome.
        let structure_focused = ctx.interaction.focused(EXPLORER)
            && matches!(
                self.tabs.get(self.active),
                Some(WorkTab::Table(tab)) if tab.mode.selected == Some(1)
            );
        let focus_in_tab = ctx
            .interaction
            .focus
            .is_some_and(|f| (f != EXPLORER && f != FILTER && f != TABSTRIP) || structure_focused);
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
        panel = panel.trailing(&meta);
        panel.paint(main, buf, None);
        // The framed source pane leaves unpainted interior cells at the
        // canvas/default text tone; child widgets own their painted wells.
        let panel_interior = Rect::new(
            main.x.saturating_add(1),
            main.y.saturating_add(1),
            main.width.saturating_sub(2),
            main.height.saturating_sub(2),
        );
        if !panel_interior.is_empty() {
            buf.set_style(panel_interior, Style::new().fg(t.text_primary).bg(t.canvas));
        }
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
                let _ = h;
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
            let explorer_w = if self.explorer_visible {
                (area.width / 4).clamp(28, 40)
            } else {
                0
            };
            let body_x = if explorer_w > 0 {
                area.x + explorer_w + 1
            } else {
                area.x
            };
            let main = Rect::new(
                body_x,
                area.y + 2,
                area.width
                    .saturating_sub(if explorer_w > 0 { explorer_w + 1 } else { 0 }),
                area.height.saturating_sub(2),
            );
            let pane = Rect::new(
                main.x.saturating_add(1),
                main.y.saturating_add(1),
                main.width.saturating_sub(2),
                main.height.saturating_sub(2),
            );
            render_history(h, history, &self.connection.name, pane, buf, ctx);
        }
    }

    fn render_explorer(&mut self, area: Rect, buf: &mut Buffer, ctx: &mut RenderCtx<'_>) {
        let structure_active = matches!(
            self.tabs.get(self.active),
            Some(WorkTab::Table(tab)) if tab.mode.selected == Some(1)
        );
        let panel = Panel::new(ctx.system)
            .variant(PanelVariant::Bordered)
            .title("Explorer")
            .trailing(&self.schema)
            .emphasis(PanelChrome::for_focus(
                ctx.interaction.focused(EXPLORER) && !structure_active,
            ));
        panel.paint(area, buf, None);
        let panel_interior = Rect::new(
            area.x.saturating_add(1),
            area.y.saturating_add(1),
            area.width.saturating_sub(2),
            area.height.saturating_sub(2),
        );
        if !panel_interior.is_empty() {
            buf.set_style(
                panel_interior,
                Style::new().fg(ctx.theme.text_primary).bg(ctx.theme.canvas),
            );
        }
        let inner = Panel::new(ctx.system)
            .variant(PanelVariant::Bordered)
            .inner(area);
        self.explorer_filter
            .set_focused(ctx.interaction.focused(FILTER));
        // Source Input always occupies 2 rows: empty label, then ▎ field.
        let filter_well = Rect::new(
            inner.x.saturating_sub(1),
            inner.y,
            inner.width.saturating_add(1),
            2,
        );
        let filter = Rect::new(
            filter_well.x,
            filter_well.y.saturating_add(1),
            filter_well.width,
            1,
        );
        // The source frame reserves the label row and leaves its content band
        // on the secondary tier, even though this filter has no label text.
        let filter_label = Rect::new(
            filter_well.x.saturating_add(2),
            filter_well.y,
            filter_well.width.saturating_sub(2),
            1,
        );
        if !filter_label.is_empty() {
            buf.set_style(filter_label, ctx.theme.secondary().bg(ctx.theme.canvas));
        }
        let _ = TextInput::new("", ctx.system)
            .placeholder("Filter objects")
            .paint(filter, buf, &mut self.explorer_filter);
        ctx.control(FILTER, filter, false);
        let vis = self.explorer_nodes();
        let nodes: Vec<TreeNode<'_, String>> = vis
            .iter()
            .map(|(id, label, depth, branch, expanded, glyph, meta)| {
                let mut n = TreeNode::new(id.clone(), Line::from(label.as_str()), *depth);
                if !glyph.is_empty() {
                    n = n.leading(Line::from(*glyph));
                }
                if *branch {
                    n = n.branch();
                    if *expanded {
                        n = n.expanded();
                    }
                }
                if let Some(m) = meta
                    && (area.width >= 39 || crate::text::width(m) <= 3)
                {
                    n = n.badge(Line::from(m.as_str()));
                }
                n
            })
            .collect();
        let tree_area = Rect::new(
            inner.x.saturating_sub(1),
            inner.y.saturating_add(2),
            inner.width.saturating_add(1),
            inner.height.saturating_sub(2),
        );
        let tree_focused = ctx.interaction.focused(EXPLORER) && !structure_active;
        StatefulWidget::render(
            &Tree::new(&nodes, ctx.system)
                .focused(tree_focused)
                .background(ctx.theme.canvas)
                .focused_metadata_bold(tree_focused)
                .selection_visible(!structure_active),
            tree_area,
            buf,
            &mut self.explorer,
        );
        ctx.control(EXPLORER, tree_area, false);
        ctx.scrollable(EXPLORER, tree_area);
        if ctx.interaction.focused(EXPLORER)
            && let Some(selected) = self.explorer.selected()
            && let Some(row) = vis.iter().position(|(id, ..)| id == selected)
        {
            self.explorer_cursor = Some(Position::new(
                area.right().saturating_sub(3),
                tree_area
                    .y
                    .saturating_add(u16::try_from(row).unwrap_or(u16::MAX)),
            ));
        }
    }

    #[must_use]
    pub fn explorer_cursor(&self) -> Option<Position> {
        self.explorer_cursor
    }

    pub fn handle(
        &mut self,
        ev: &PageEvent,
        cx: &mut PageCtx<'_>,
        history: &History,
        system: &DesignSystem,
    ) -> Route {
        let _ = history;
        match ev {
            PageEvent::Key(key) if key.kind != KeyEventKind::Release => {
                let Some(f) = *cx.focus else {
                    return Route::Ignored;
                };
                if f == FILTER {
                    // TextInputState only edits once editing; an unmodified
                    // char begins editing so the filter is keyboard-reachable.
                    if !self.explorer_filter.is_editing()
                        && let KeyCode::Char(_) = key.code
                        && key.modifiers.is_empty()
                    {
                        self.explorer_filter.begin_edit();
                    }
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
                            let _ = self.request_close_tab(id);
                            return Route::Changed;
                        }
                        TabsOutcome::Ignored => return Route::Ignored,
                        _ => return Route::Changed,
                    }
                }
                match self.tabs.get(self.active) {
                    Some(WorkTab::Query(_)) => {
                        let cat = self.catalog.clone();
                        if let Some(WorkTab::Query(q)) = self.tabs.get_mut(self.active) {
                            return handle_query(q, ev, cx, &cat);
                        }
                    }
                    Some(WorkTab::Table(_)) => {
                        let cat = self.catalog.clone();
                        if let Some(WorkTab::Table(tt)) = self.tabs.get_mut(self.active) {
                            return handle_table(tt, ev, cx, &cat, system);
                        }
                    }
                    Some(WorkTab::History(_)) => {
                        if let Some(WorkTab::History(h)) = self.tabs.get_mut(self.active) {
                            return handle_history(h, ev, cx);
                        }
                    }
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
                match self.tabs.get(self.active) {
                    Some(WorkTab::Query(_)) => {
                        let cat = self.catalog.clone();
                        if let Some(WorkTab::Query(q)) = self.tabs.get_mut(self.active) {
                            return handle_query(q, ev, cx, &cat);
                        }
                    }
                    Some(WorkTab::Table(_)) => {
                        let cat = self.catalog.clone();
                        if let Some(WorkTab::Table(tt)) = self.tabs.get_mut(self.active) {
                            return handle_table(tt, ev, cx, &cat, system);
                        }
                    }
                    Some(WorkTab::History(_)) => {
                        if let Some(WorkTab::History(h)) = self.tabs.get_mut(self.active) {
                            return handle_history(h, ev, cx);
                        }
                    }
                    None => {}
                }
                Route::Ignored
            }
            PageEvent::Wheel { id, delta } if *id == EXPLORER => {
                let n = self.explorer_nodes().len();
                let _ = self.explorer.scroll_by(*delta as isize, n);
                Route::Changed
            }
            PageEvent::Paste(_) | PageEvent::Wheel { .. } => match self.tabs.get(self.active) {
                Some(WorkTab::Query(_)) => {
                    let cat = self.catalog.clone();
                    if let Some(WorkTab::Query(q)) = self.tabs.get_mut(self.active) {
                        return handle_query(q, ev, cx, &cat);
                    }
                    Route::Ignored
                }
                Some(WorkTab::Table(_)) => {
                    let cat = self.catalog.clone();
                    if let Some(WorkTab::Table(tt)) = self.tabs.get_mut(self.active) {
                        return handle_table(tt, ev, cx, &cat, system);
                    }
                    Route::Ignored
                }
                Some(WorkTab::History(_)) => {
                    if let Some(WorkTab::History(h)) = self.tabs.get_mut(self.active) {
                        return handle_history(h, ev, cx);
                    }
                    Route::Ignored
                }
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
            .map(|(id, label, depth, branch, expanded, glyph, meta)| {
                let mut n = TreeNode::new(id.clone(), Line::from(label.as_str()), *depth);
                if !glyph.is_empty() {
                    n = n.leading(Line::from(*glyph));
                }
                if *branch {
                    n = n.branch();
                    if *expanded {
                        n = n.expanded();
                    }
                }
                if let Some(m) = meta {
                    n = n.badge(Line::from(m.as_str()));
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
            self.explorer.select(Some(id.to_owned()));
            self.open_table(&schema, &name);
            cx.set_focus(super::tabs::TABLE_GRID);
            return Route::Changed;
        }
        if self.expanded.contains(id) {
            self.expanded.remove(id);
        } else {
            self.expanded.insert(id.to_owned());
        }
        Route::Changed
    }

    pub fn request_close_tab(&mut self, id: usize) -> bool {
        if id >= self.tabs.len() {
            return false;
        }
        if self.tabs[id].dirty() {
            self.pending_close = Some(id);
            true
        } else {
            self.close_tab(id);
            false
        }
    }

    pub fn take_close_request(&mut self) -> Option<usize> {
        self.pending_close.take()
    }

    pub fn close_tab(&mut self, id: usize) {
        if id >= self.tabs.len() {
            return;
        }
        self.tabs.remove(id);
        if self.active >= self.tabs.len() {
            self.active = self.tabs.len().saturating_sub(1);
        } else if self.active > id {
            self.active -= 1;
        }
        self.sync_strip();
    }

    #[must_use]
    pub fn hints(&self, focus: Option<WidgetId>) -> Vec<Hint> {
        if focus == Some(PLAN) {
            return vec![("↑ ↓", "Move")];
        }
        if focus == Some(TABSTRIP) {
            return vec![
                ("← →", "Switch"),
                ("Ctrl+T", "New query"),
                ("x", "Close"),
                ("Ctrl+G", "Tab list"),
                ("z", "Zoom"),
            ];
        }
        if focus == Some(EXPLORER) || focus == Some(FILTER) {
            if focus == Some(EXPLORER)
                && matches!(self.tabs.get(self.active), Some(WorkTab::Table(tab)) if tab.mode.selected == Some(1))
            {
                return vec![("↑ ↓", "Move"), ("Ctrl+D", "Structure")];
            }
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
            Some(WorkTab::Table(t)) => table_hints(t, focus),
            Some(WorkTab::History(_)) => vec![
                ("Enter", "Open in new tab"),
                ("r", "Rerun"),
                ("y", "Copy"),
                ("/", "Search"),
                ("c s", "Scope · Status"),
            ],
            None => vec![("Ctrl+T", "New query")],
        }
    }
}
