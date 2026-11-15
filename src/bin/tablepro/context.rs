use ratatui::buffer::Buffer;
use ratatui::crossterm::event::{KeyCode, KeyModifiers};
use ratatui::layout::{Position, Rect};

use junie_tui::core::event::{Key, Outcome};
use junie_tui::core::id::{ItemKey, WidgetId};
use junie_tui::core::pointer::PointerEvent;
use junie_tui::shell::Menus;
use junie_tui::ui::columns::HeaderAction;
use junie_tui::ui::context::{ContextConfig, ContextRequest};
use junie_tui::ui::ctx::RenderCtx;
use junie_tui::ui::layer::{DismissReason, Layer, LayerCx, LayerOut, LayerSpec, Overlay};
use junie_tui::widgets::grid::DataGrid;
use junie_tui::widgets::keyhint::{Hint, hint};
use junie_tui::widgets::menu::{ContextMenu, MenuItem};
use junie_tui::widgets::table::SortDir;

use crate::app::ModalResult;
use crate::connections::ConnectionsScreen;
use crate::db::ObjectKind;
use crate::tabs::HistoryCommand;
use crate::workbench::{WorkTab, Workbench};

pub const MENU: WidgetId = WidgetId::of("context-menu");

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Target {
    Explorer(usize),

    Tab(usize),

    Grid(usize, usize),

    Results(usize, usize),

    History {
        owner: WidgetId,
        entry: ItemKey,
    },

    /// A connections-tree row and, for a connection row, the connection it showed
    /// when the menu opened.
    Connection(usize, Option<ItemKey>),

    Header(usize),

    App,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AppCommand {
    OpenTabs,

    Disconnect,

    Quit,
}

pub fn app_menu(connection: Option<&str>, pos: Position) -> CtxMenu {
    let mut items = vec![];
    if connection.is_some() {
        items.push((
            MenuItem::new("Open tabs…").shortcut("Ctrl+G"),
            AppCommand::OpenTabs,
        ));
        items.push((
            MenuItem::new("Disconnect").separator(),
            AppCommand::Disconnect,
        ));
    }
    items.push((
        MenuItem::new("Quit TablePro").shortcut("Ctrl+Q"),
        AppCommand::Quit,
    ));
    let keys = vec![None; items.len()];
    let title = connection.unwrap_or("TablePro").to_owned();
    let app = Menus::new().menu(title.clone(), items);
    CtxMenu {
        menu: ContextMenu::new(MENU, app.items(0)).at(pos).title(title),
        target: Target::App,
        owner: crate::workbench::EXPLORER,
        keys,
        header: Vec::new(),
        app,
    }
}

pub struct CtxMenu {
    pub menu: ContextMenu,
    pub target: Target,
    pub owner: WidgetId,
    pub keys: Vec<Option<Key>>,

    pub header: Vec<HeaderAction>,

    pub app: Menus<AppCommand>,
}

impl Layer<ModalResult> for CtxMenu {
    fn spec(&self) -> LayerSpec {
        Overlay::spec(&self.menu).initial_focus(self.owner)
    }
    fn render(&mut self, screen: Rect, buf: &mut Buffer, ctx: &mut RenderCtx, app: &()) {
        Layer::<ModalResult>::render(&mut self.menu, screen, buf, ctx, app);
    }
    fn on_key(&mut self, key: &Key, cx: &mut LayerCx<'_>) -> Outcome {
        Layer::<ModalResult>::on_key(&mut self.menu, key, cx)
    }
    fn pointer(
        &mut self,
        ev: &junie_tui::core::pointer::PointerEvent,
        cx: &mut LayerCx<'_>,
    ) -> Outcome {
        Layer::<ModalResult>::pointer(&mut self.menu, ev, cx)
    }
    fn hints(&self, _: Option<WidgetId>) -> Vec<Hint> {
        vec![
            hint("↑↓", "Move"),
            hint("Enter", "Choose"),
            hint("Esc", "Close"),
        ]
    }
    fn on_dismiss(&mut self, reason: DismissReason) {
        Layer::<ModalResult>::on_dismiss(&mut self.menu, reason);
    }
    fn take_result(&mut self) -> Option<LayerOut<ModalResult>> {
        Layer::<ModalResult>::take_result(&mut self.menu)
    }
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

fn alt(c: char) -> Key {
    Key {
        code: KeyCode::Char(c),
        mods: KeyModifiers::ALT,
    }
}

type Items = Vec<(MenuItem, Option<Key>)>;

fn cmd(items: &mut Items, item: MenuItem, key: Key, reason: Option<&str>) {
    match reason {
        Some(r) => items.push((item.reason(r), None)),
        None => items.push((item, Some(key))),
    }
}

fn group(items: &mut Items) {
    if let Some((it, _)) = items.last_mut() {
        it.separator_after = true;
    }
}

pub fn on_secondary(w: &mut Workbench, cfg: &ContextConfig, ev: &PointerEvent) -> Option<CtxMenu> {
    if let Some(req) = cfg.on_secondary(&mut w.explorer, ev) {
        return Some(build(w, req.map(Target::Explorer)));
    }
    if let Some(req) = cfg.on_secondary(&mut w.strip, ev) {
        return Some(build(w, req.map(Target::Tab)));
    }
    if let Some(g) = w.active_grid_any_mut()
        && let Some(col) = g.header_menu_at(ev)
    {
        let owner = g.id;
        let (header, items): (Vec<HeaderAction>, Vec<MenuItem>) =
            g.header_menu(col).into_iter().unzip();
        let keys = vec![None; items.len()];
        return Some(CtxMenu {
            menu: ContextMenu::new(MENU, items)
                .at(ev.pos())
                .title(g.columns[col].name.clone()),
            target: Target::Header(col),
            owner,
            keys,
            header,
            app: Menus::default(),
        });
    }
    let req = match w.active_tab_mut()? {
        WorkTab::Table(t) => cfg
            .on_secondary(&mut t.grid, ev)?
            .map(|(r, c)| Target::Grid(r, c)),
        WorkTab::Query(q) => cfg
            .on_secondary(q.active_grid_mut()?, ev)?
            .map(|(r, c)| Target::Results(r, c)),
        WorkTab::History(h) => {
            let req = cfg.on_secondary(&mut h.list, ev)?;
            let entry = h.filtered.get(req.target).copied()?;
            req.map(|_| Target::History { owner: h.id, entry })
        }
    };
    Some(build(w, req))
}

pub fn on_key(
    w: &Workbench,
    cfg: &ContextConfig,
    focus: Option<WidgetId>,
    key: &Key,
) -> Option<CtxMenu> {
    let f = focus?;
    if !cfg.keys.matches(key) {
        return None;
    }
    let req = if f == w.explorer.id {
        cfg.on_key(&w.explorer, key)?.map(Target::Explorer)
    } else if f == w.strip.id {
        cfg.on_key(&w.strip, key)?.map(Target::Tab)
    } else {
        match w.active_tab()? {
            WorkTab::Table(t) if f == t.grid.id && !t.grid.is_editing() => {
                cfg.on_key(&t.grid, key)?.map(|(r, c)| Target::Grid(r, c))
            }
            WorkTab::Query(q) => {
                let g = q.active_grid().filter(|g| g.id == f && !g.is_editing())?;
                cfg.on_key(g, key)?.map(|(r, c)| Target::Results(r, c))
            }
            WorkTab::History(h) if f == h.list.id => {
                let req = cfg.on_key(&h.list, key)?;
                let entry = h.filtered.get(req.target).copied()?;
                req.map(|_| Target::History { owner: h.id, entry })
            }
            _ => return None,
        }
    };
    Some(build(w, req))
}

pub fn select(w: &mut Workbench, target: Target) {
    use junie_tui::ui::context::ContextSource;
    match target {
        Target::Explorer(row) => w.explorer.context_select(row),
        Target::Tab(i) => w.strip.context_select(i),
        Target::Grid(r, c) => {
            if let Some(WorkTab::Table(t)) = w.active_tab_mut() {
                t.grid.context_select((r, c));
            }
        }
        Target::Results(r, c) => {
            if let Some(WorkTab::Query(q)) = w.active_tab_mut()
                && let Some(g) = q.active_grid_mut()
            {
                g.context_select((r, c));
            }
        }
        Target::History { .. } | Target::Connection(..) | Target::Header(_) | Target::App => {}
    }
}

pub fn on_secondary_connections(
    c: &mut ConnectionsScreen,
    cfg: &ContextConfig,
    ev: &PointerEvent,
) -> Option<CtxMenu> {
    let req = cfg.on_secondary(c.tree_mut(), ev)?;
    c.select_row(req.target);
    Some(build_connection(
        c,
        req.map(|row| connection_target(c, row)),
    ))
}

pub fn on_key_connections(
    c: &ConnectionsScreen,
    cfg: &ContextConfig,
    focus: Option<WidgetId>,
    key: &Key,
) -> Option<CtxMenu> {
    if focus? != c.tree().id {
        return None;
    }
    let req = cfg.on_key(c.tree(), key)?;
    Some(build_connection(
        c,
        req.map(|row| connection_target(c, row)),
    ))
}

fn connection_target(c: &ConnectionsScreen, row: usize) -> Target {
    Target::Connection(row, c.key_at_row(row))
}

fn build_connection(c: &ConnectionsScreen, req: ContextRequest<Target>) -> CtxMenu {
    let Target::Connection(row, _) = req.target else {
        unreachable!("a connections menu is about a connection row")
    };
    let (title, items) = connection_items(c, row);
    finish(req, title, c.tree().id, items)
}

fn connection_items(c: &ConnectionsScreen, row: usize) -> (String, Items) {
    let mut items = Items::new();
    let Some(r) = c.tree().rows().get(row) else {
        return (String::new(), items);
    };
    if c.key_at_row(row).is_some() {
        let busy = c.animating().then_some("A connection is in progress");
        cmd(
            &mut items,
            MenuItem::new("Connect").shortcut("Enter"),
            plain(KeyCode::Enter),
            busy,
        );
        group(&mut items);
        cmd(
            &mut items,
            MenuItem::new("Edit…").shortcut("e"),
            plain(KeyCode::Char('e')),
            None,
        );
        cmd(
            &mut items,
            MenuItem::new("Duplicate").shortcut("Alt+D"),
            alt('d'),
            None,
        );
        cmd(
            &mut items,
            MenuItem::new("Delete…").shortcut("d").danger(),
            plain(KeyCode::Char('d')),
            None,
        );
    } else if r.has_children {
        if r.expanded {
            cmd(
                &mut items,
                MenuItem::new("Collapse").shortcut("←"),
                plain(KeyCode::Left),
                None,
            );
        } else {
            cmd(
                &mut items,
                MenuItem::new("Expand").shortcut("→"),
                plain(KeyCode::Right),
                None,
            );
        }
    }
    group(&mut items);
    cmd(
        &mut items,
        MenuItem::new("New connection…").shortcut("Ctrl+N"),
        ctrl('n'),
        None,
    );
    cmd(
        &mut items,
        MenuItem::new("Filter").shortcut("/"),
        plain(KeyCode::Char('/')),
        None,
    );
    (r.label.clone(), items)
}

const HISTORY_ROW_COMMANDS: [(&str, &str, KeyCode, HistoryCommand); 3] = [
    (
        "Open in new tab",
        "Enter",
        KeyCode::Enter,
        HistoryCommand::Open,
    ),
    (
        "Run in new tab",
        "r",
        KeyCode::Char('r'),
        HistoryCommand::Rerun,
    ),
    ("Copy query", "y", KeyCode::Char('y'), HistoryCommand::Copy),
];

pub fn history_command(key: &Key) -> Option<HistoryCommand> {
    HISTORY_ROW_COMMANDS
        .iter()
        .find(|(_, _, code, _)| key.is(*code))
        .map(|(.., cmd)| *cmd)
}

fn history_items(w: &Workbench, owner: WidgetId, entry: ItemKey) -> (String, WidgetId, Items) {
    let mut items = Items::new();
    let Some(h) = w.history_tab(owner) else {
        return (String::new(), w.explorer.id, items);
    };
    let Some(it) = h.row_of(entry).and_then(|row| h.list.items.get(row)) else {
        return (String::new(), h.list.id, items);
    };
    for (title, shortcut, code, _) in HISTORY_ROW_COMMANDS {
        cmd(
            &mut items,
            MenuItem::new(title).shortcut(shortcut),
            plain(code),
            None,
        );
    }
    group(&mut items);
    cmd(
        &mut items,
        MenuItem::new("Search").shortcut("/"),
        plain(KeyCode::Char('/')),
        None,
    );
    let scope = if h.scope_all {
        "Show this connection only"
    } else {
        "Show all connections"
    };
    cmd(
        &mut items,
        MenuItem::new(scope).shortcut("c"),
        plain(KeyCode::Char('c')),
        None,
    );
    let status = if h.failed_only {
        "Show every status"
    } else {
        "Show failed only"
    };
    cmd(
        &mut items,
        MenuItem::new(status).shortcut("s"),
        plain(KeyCode::Char('s')),
        None,
    );
    (it.label.clone(), h.list.id, items)
}

fn build(w: &Workbench, req: ContextRequest<Target>) -> CtxMenu {
    let (title, owner, items) = match req.target {
        Target::Explorer(row) => explorer_items(w, row),
        Target::Tab(i) => tab_items(w, i),
        Target::Grid(r, c) => match w.active_tab() {
            Some(WorkTab::Table(t)) => grid_items(&t.grid, r, c, true),
            _ => (String::new(), w.explorer.id, vec![]),
        },
        Target::Results(r, c) => match w.active_tab() {
            Some(WorkTab::Query(q)) if q.active_grid().is_some() => {
                grid_items(q.active_grid().unwrap(), r, c, false)
            }
            _ => (String::new(), w.explorer.id, vec![]),
        },
        Target::History { owner, entry } => history_items(w, owner, entry),
        Target::Connection(..) | Target::Header(_) | Target::App => {
            (String::new(), w.explorer.id, vec![])
        }
    };
    finish(req, title, owner, items)
}

fn finish(req: ContextRequest<Target>, title: String, owner: WidgetId, items: Items) -> CtxMenu {
    let (items, keys): (Vec<MenuItem>, Vec<Option<Key>>) = items.into_iter().unzip();
    let mut menu = req.menu(MENU, items);
    if !title.is_empty() {
        menu = menu.title(title);
    }
    CtxMenu {
        menu,
        target: req.target,
        owner,
        keys,
        header: Vec::new(),
        app: Menus::default(),
    }
}

fn explorer_items(w: &Workbench, row: usize) -> (String, WidgetId, Items) {
    let mut items = Items::new();
    let Some(r) = w.explorer.rows().get(row) else {
        return (String::new(), w.explorer.id, items);
    };
    let enter = plain(KeyCode::Enter);
    if let Some((schema, name)) = w.object_at(&r.path) {
        let kind = w.catalog.find(Some(&schema), &name).map(|t| t.kind);
        let opens = matches!(kind, Some(ObjectKind::Table | ObjectKind::View));
        cmd(
            &mut items,
            MenuItem::new("Open").shortcut("Enter"),
            enter,
            (!opens).then_some("Tables and views only"),
        );
    } else if w.schema_at(&r.path).is_some() {
        cmd(
            &mut items,
            MenuItem::new("Use schema").shortcut("Enter"),
            enter,
            None,
        );
    }
    if r.has_children {
        if r.expanded {
            cmd(
                &mut items,
                MenuItem::new("Collapse").shortcut("←"),
                plain(KeyCode::Left),
                None,
            );
        } else {
            cmd(
                &mut items,
                MenuItem::new("Expand").shortcut("→"),
                plain(KeyCode::Right),
                None,
            );
        }
    }
    group(&mut items);
    cmd(
        &mut items,
        MenuItem::new("Filter objects…").shortcut("/"),
        plain(KeyCode::Char('/')),
        None,
    );
    cmd(
        &mut items,
        MenuItem::new("Refresh").shortcut("r"),
        plain(KeyCode::Char('r')),
        None,
    );
    (r.label.clone(), w.explorer.id, items)
}

fn tab_items(w: &Workbench, i: usize) -> (String, WidgetId, Items) {
    let mut items = Items::new();
    let title = w.tabs.get(i).map(|t| t.label()).unwrap_or_default();
    cmd(
        &mut items,
        MenuItem::new("Switch to tab").shortcut("Enter"),
        plain(KeyCode::Enter),
        (i == w.active).then_some("Current tab"),
    );
    cmd(
        &mut items,
        MenuItem::new("Close tab").shortcut("x"),
        plain(KeyCode::Char('x')),
        None,
    );
    group(&mut items);
    cmd(
        &mut items,
        MenuItem::new("New query tab").shortcut("Ctrl+T"),
        ctrl('t'),
        None,
    );
    cmd(
        &mut items,
        MenuItem::new("Tab list…").shortcut("Ctrl+G"),
        ctrl('g'),
        None,
    );
    (title, w.strip.id, items)
}

fn grid_items(g: &DataGrid, row: usize, col: usize, table: bool) -> (String, WidgetId, Items) {
    let mut items = Items::new();
    let Some(spec) = g.columns.get(col) else {
        return (String::new(), g.id, items);
    };
    if row >= g.len() {
        return (String::new(), g.id, items);
    }
    let src = g.source_row(row);
    let deleted = g
        .row_key(src)
        .is_some_and(|key| g.pending().is_deleted(key));
    let read_only = (!g.editable).then_some("Read-only rows");
    let cell_lock = read_only
        .or(spec.read_only.then_some("Read-only column"))
        .or(deleted.then_some("Row is deleted"));
    let title = format!("{} · row {}", spec.name, row + 1);

    cmd(
        &mut items,
        MenuItem::new("Copy cell").shortcut("y"),
        plain(KeyCode::Char('y')),
        None,
    );
    cmd(
        &mut items,
        MenuItem::new("Copy row").shortcut("Y"),
        plain(KeyCode::Char('Y')),
        None,
    );
    group(&mut items);
    cmd(
        &mut items,
        MenuItem::new("Edit cell").shortcut("Enter"),
        plain(KeyCode::Enter),
        cell_lock,
    );
    cmd(
        &mut items,
        MenuItem::new("Set NULL").shortcut("Del"),
        plain(KeyCode::Delete),
        cell_lock
            .or((!spec.nullable).then_some("NOT NULL column"))
            .or((!g.selection.is_empty()).then_some("Rows are marked")),
    );
    group(&mut items);
    cmd(
        &mut items,
        MenuItem::new("Filter by this value").shortcut("f"),
        plain(KeyCode::Char('f')),
        (!table).then_some("Table tabs only"),
    );
    let sort = match g.sort {
        Some((c, SortDir::Asc)) if c == col => "Sort descending",
        Some((c, SortDir::Desc)) if c == col => "Clear sort",
        _ => "Sort ascending",
    };
    cmd(
        &mut items,
        MenuItem::new(sort).shortcut("s"),
        plain(KeyCode::Char('s')),
        (!spec.sortable).then_some("Not sortable"),
    );
    if spec.references.is_some() {
        cmd(
            &mut items,
            MenuItem::new("Open referenced row").shortcut("Ctrl+]"),
            ctrl(']'),
            (!table).then_some("Table tabs only"),
        );
    }
    group(&mut items);
    cmd(
        &mut items,
        MenuItem::new("Insert row").shortcut("+"),
        plain(KeyCode::Char('+')),
        read_only,
    );
    cmd(
        &mut items,
        MenuItem::new("Duplicate row").shortcut("Alt+D"),
        alt('d'),
        read_only,
    );
    if deleted {
        cmd(
            &mut items,
            MenuItem::new("Restore row").shortcut("-"),
            plain(KeyCode::Char('-')),
            read_only,
        );
    } else {
        cmd(
            &mut items,
            MenuItem::new("Delete row").shortcut("-").danger(),
            plain(KeyCode::Char('-')),
            read_only,
        );
    }
    if table {
        group(&mut items);
        let none = g.pending().is_empty().then_some("No pending changes");
        cmd(
            &mut items,
            MenuItem::new("Preview SQL").shortcut("p"),
            plain(KeyCode::Char('p')),
            none,
        );
        cmd(
            &mut items,
            MenuItem::new("Save changes").shortcut("Ctrl+S"),
            ctrl('s'),
            none,
        );
        cmd(
            &mut items,
            MenuItem::new("Discard changes").shortcut("U").danger(),
            plain(KeyCode::Char('U')),
            none,
        );
        cmd(
            &mut items,
            MenuItem::new("Refresh").shortcut("r"),
            plain(KeyCode::Char('r')),
            None,
        );
    }
    (title, g.id, items)
}
