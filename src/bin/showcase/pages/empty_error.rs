use ratatui::buffer::Buffer;
use ratatui::layout::{Rect, Size};

use crate::app::PageId;
use crate::options::{PageValues, page_options};
use crate::pages::row_menu::{MENU_HINTS, RowMenu};
use crate::pages::template::{Section, Template};
use crate::pages::{Cx, Hint, Page, PageEvent};
use junie_tui::core::event::Outcome;
use junie_tui::core::id::WidgetId;
use junie_tui::core::intent::Phase;
use junie_tui::core::pointer::MouseTarget;
use junie_tui::ui::ctx::RenderCtx;
use junie_tui::ui::layout::StackChild;
use junie_tui::ui::measure::Measure;
use junie_tui::ui::popup::Placement;
use junie_tui::ui::specimen::{SpecimenState, StateGrid};
use junie_tui::widgets::button::Button;
use junie_tui::widgets::empty::{self, EmptyAction, EmptyState};
use junie_tui::widgets::grid::{
    CellKind, CellValue, ColumnSpec, DataGrid, GridAction, GridRows, RowTotal,
};
use junie_tui::widgets::list::{ListBox, SelectMode};
use junie_tui::widgets::menu::{ContextMenu, MenuItem};
use junie_tui::widgets::panel::Panel;
use junie_tui::widgets::scrollbar;

const ID: WidgetId = WidgetId::of("emptyerror");
const NEW: WidgetId = WidgetId::of("emptyerror").sub("new");
const IMPORT: WidgetId = WidgetId::of("emptyerror").sub("import");

const CHILDREN: [StackChild; 3] = [
    StackChild::fixed(8),
    StackChild::fixed(10),
    StackChild::fixed(9),
];

const GRID_W: u16 = 60;
const GRID_NOTE: &str = "Enter or a typed key on a cell says why it cannot change";

fn saved_empty() -> EmptyState {
    EmptyState::new("No saved queries").hint("Queries you save appear here")
}

fn search_empty() -> EmptyState {
    EmptyState::new("Nothing matches “ordrs”")
}

pub fn page() -> Box<dyn Page> {
    Box::new(EmptyErrorPage::new())
}

pub struct EmptyErrorPage {
    template: Template,
    opts: PageValues,
    state: EmptyState,
    list: ListBox,
    grid: DataGrid,
    actions: Button,
    menu: RowMenu<()>,
    sections: crate::pages::layout::Sections,
}

impl EmptyErrorPage {
    pub fn new() -> Self {
        let columns = vec![
            ColumnSpec::new("order", CellKind::Number).type_label("integer"),
            ColumnSpec::new("customer", CellKind::Text).type_label("text"),
            ColumnSpec::new("total", CellKind::Number).type_label("numeric"),
        ];
        let mut grid = DataGrid::new(ID.sub("grid"), columns).editable(false);
        grid.set_rows(GridRows {
            rows: [(1041, "Acme Corp", 1204.5), (1042, "Globex", 88.0)]
                .into_iter()
                .map(|(o, c, t)| {
                    vec![
                        CellValue::Int(o),
                        CellValue::Text(c.into()),
                        CellValue::Num(t),
                    ]
                })
                .collect(),
            total: RowTotal::Exact(2),
            more: false,
        });
        let mut page = Self {
            template: Template::new(ID, states_grid()),
            opts: PageValues::new(page_options(PageId::EmptyError)),
            state: EmptyState::default(),
            list: ListBox::new(ID.sub("list"), vec![], SelectMode::Single).empty(saved_empty()),
            grid,
            actions: Button::secondary(ID.sub("actions"), "Row actions ▾"),
            menu: RowMenu::default(),
            sections: crate::pages::layout::Sections::new(ID.sub("page")),
        };
        page.rebuild();
        page
    }

    fn rebuild(&mut self) {
        let o = &self.opts;
        let error = o.value("emptyerror.kind") == "error";
        let mut e = if error {
            EmptyState::error("Could not load connections")
        } else {
            EmptyState::new("No connections yet")
        };
        if o.on("emptyerror.hint") {
            e = e.hint(if error {
                "The config file is unreadable: check its permissions"
            } else {
                "A connection holds a host, a user and a database"
            });
        }
        let first = if error {
            EmptyAction::primary(NEW, "Retry")
        } else {
            EmptyAction::primary(NEW, "New connection")
        };
        let actions = match o.value("emptyerror.actions") {
            "none" => vec![],
            "one" => vec![first],
            _ => vec![first, EmptyAction::secondary(IMPORT, "Import…")],
        };
        self.state = e.actions(actions);
        self.grid.read_only_reason = Some(self.reason().into());
    }

    fn reason(&self) -> &'static str {
        match self.opts.value("emptyerror.reason") {
            "no-key" => "orders_view has no primary key, so rows cannot be told apart",
            "result" => "A query result is read-only: edit the table instead",
            _ => "orders_view is a view: views cannot be edited",
        }
    }

    fn short_reason(&self) -> &'static str {
        match self.opts.value("emptyerror.reason") {
            "no-key" => "no primary key",
            "result" => "query result",
            _ => "read-only view",
        }
    }

    fn render_playground(&mut self, area: Rect, buf: &mut Buffer, ctx: &mut RenderCtx) {
        let has = |id| ctx.interaction.has_focus(id);
        let focus = if has(self.list.id) {
            Some(0)
        } else if self.state.actions.iter().any(|a| has(a.id)) {
            Some(1)
        } else if has(self.grid.id) || has(self.actions.id) {
            Some(2)
        } else {
            None
        };
        let l = self.sections.layout(area, &CHILDREN, focus, buf, ctx);
        let t = ctx.theme;
        let (left, right) = match l.slot(0) {
            Some(r) => crate::pages::layout::columns(r, r.width / 2, 2),
            None => (Rect::ZERO, Rect::ZERO),
        };
        if l.slot(0).is_some() {
            let panel = Panel::card(Some("Saved queries")).meta("ListBox::empty");
            let bg = panel.bg(t);
            let inner = panel.render(left, buf, t);
            self.list.render(inner, buf, ctx, bg);
            let panel = Panel::card(Some("Search results")).meta("no hint, no action");
            let bg = panel.bg(t);
            let inner = panel.render(right, buf, t);
            empty::render(inner, buf, t, &search_empty(), bg);
        } else {
            ctx.focus_stop(self.list.id);
        }
        match l.slot(1) {
            Some(r) => {
                let panel = Panel::card(Some("Connections")).meta("EmptyState::actions");
                let bg = panel.bg(t);
                let inner = panel.render(r, buf, t);
                empty::render_ctx(inner, buf, ctx, &self.state, bg);
            }
            None => self.state.actions.iter().for_each(|a| ctx.focus_stop(a.id)),
        }
        match l.slot(2) {
            Some(r) => {
                let panel = Panel::card(Some("Orders")).meta("orders_view · read-only");
                let bg = panel.bg(t);
                let inner = panel.render(r, buf, t);
                let bw = self.actions.width();
                let grid = Rect::new(
                    inner.x,
                    inner.y,
                    inner.width.saturating_sub(bw + 2).min(GRID_W),
                    inner.height.saturating_sub(1),
                );
                self.grid.render(grid, buf, ctx, bg);
                self.actions
                    .render(Rect::new(grid.right() + 2, inner.y, bw, 1), buf, ctx, bg);
                let note = GRID_NOTE;
                buf.set_string(
                    inner.x,
                    inner.bottom().saturating_sub(1),
                    junie_tui::ui::text::truncate(note, inner.width as usize),
                    t.faint().bg(bg),
                );
            }
            None => {
                ctx.focus_stop(self.grid.id);
                ctx.focus_stop(self.actions.id);
            }
        }
        self.menu.render(area, buf, ctx);
    }

    fn open_menu(&mut self) {
        let reason = self.short_reason();
        let anchor = self.actions_area();
        let menu = ContextMenu::new(
            ID.sub("menu"),
            vec![
                MenuItem::new("Edit cell").disabled(true).reason(reason),
                MenuItem::new("Duplicate row").disabled(true).reason(reason),
                MenuItem::new("Delete row")
                    .danger()
                    .disabled(true)
                    .reason(reason)
                    .separator(),
                MenuItem::new("Copy row"),
                MenuItem::new("Copy as CSV"),
            ],
        )
        .anchor(anchor, Placement::Below);
        self.menu.open(menu, ());
    }

    fn actions_area(&self) -> Rect {
        self.actions.area
    }

    fn on_grid(&mut self, ev: Option<GridAction>, cx: &mut Cx) {
        match ev {
            Some(GridAction::ReadOnly { reason }) => cx.status(reason),
            Some(GridAction::Copy(s)) => cx.status(format!("Copied {} chars", s.len())),
            _ => {}
        }
    }
}

fn states_grid() -> StateGrid {
    use SpecimenState::*;
    let id = ID.sub("states");
    StateGrid::new(id)
        .label_width(14)
        .column_width(26)
        .rows(&[Rest, Hover, Pressed, Focused, Error, Overflow, Narrow])
        .column("Title", EmptyState::new("No rows"))
        .column(
            "Title + hint",
            EmptyState::new("No rows").hint("Run the query to fill it"),
        )
        .column(
            "With actions",
            EmptyState::new("No connections")
                .actions(vec![EmptyAction::primary(id.child(0), "New")]),
        )
}

impl Page for EmptyErrorPage {
    fn title(&self) -> &'static str {
        "Empty & error"
    }
    fn blurb(&self) -> &'static str {
        "Empty and failed states with a way out, and read-only surfaces that say why"
    }

    fn measure(&self, _avail: Size, ctx: &RenderCtx) -> Size {
        let at = |e: &EmptyState| e.measure(Size::ZERO, ctx);
        let half = Panel::card(Some("Saved queries"))
            .meta("ListBox::empty")
            .outer(at(&saved_empty()))
            .width
            .max(
                Panel::card(Some("Search results"))
                    .meta("no hint, no action")
                    .outer(at(&search_empty()))
                    .width,
            );
        let grid =
            (GRID_W + 2 + self.actions.width()).max(junie_tui::ui::text::width(GRID_NOTE) as u16);
        let width = [
            half * 2 + 4,
            Panel::card(Some("Connections"))
                .meta("EmptyState::actions")
                .outer(at(&self.state))
                .width,
            Panel::card(Some("Orders"))
                .meta("orders_view · read-only")
                .outer(Size::new(grid, 0))
                .width,
        ]
        .into_iter()
        .max()
        .unwrap_or(0);
        Size::new(width, self.sections.stack.measure(&CHILDREN))
    }

    fn render(&mut self, area: Rect, buf: &mut Buffer, ctx: &mut RenderCtx) {
        let body = self.template.render(area, buf, ctx);
        match self.template.section() {
            Section::Playground => self.render_playground(body, buf, ctx),
            Section::Options => self.template.render_options(body, buf, ctx),
            Section::States => self.template.render_states(body, buf, ctx),
        }
    }

    fn layer_open(&self) -> bool {
        self.menu.hides_page_focus()
    }

    fn handle(&mut self, ev: &PageEvent, cx: &mut Cx) -> Outcome {
        if let Some((out, chosen)) = self.menu.handle(ev) {
            if let Some(((), i)) = chosen {
                cx.status(match i {
                    3 => "Copied row 1041",
                    _ => "Copied 2 rows as CSV",
                });
            }
            return out;
        }
        if let Some(o) = self.template.handle(ev, cx) {
            return o;
        }
        if self.template.section() != Section::Playground {
            return Outcome::Ignored;
        }
        if let Some(o) = self.sections.handle(ev) {
            return o;
        }
        match ev {
            PageEvent::Key(key) => {
                let Some(f) = cx.focus.current() else {
                    return Outcome::Ignored;
                };
                if f == self.grid.id {
                    let (o, actions) = self.grid.on_key(key).into_parts();
                    for gev in actions {
                        self.on_grid(Some(gev), cx);
                    }
                    return o;
                }
                if f == self.actions.id {
                    let (o, act) = self.actions.on_key(key);
                    if act {
                        self.open_menu();
                    }
                    return o;
                }
                if f == self.list.id {
                    return self.list.on_key(key);
                }
                let (o, chosen) = self.state.on_key(f, key);
                if let Some(a) = chosen {
                    cx.status(format!("{} chosen", a.label));
                }
                o
            }
            PageEvent::Pointer(ev) => {
                if ev.is(Phase::Click)
                    && let Some(a) = self.state.on_click(ev.id)
                {
                    cx.focus.focus(a.id);
                    cx.status(format!("{} chosen", a.label));
                    return Outcome::Changed;
                }
                if let Some(r) = self.actions.route(ev) {
                    if !ev.is(Phase::Click) {
                        return r.outcome();
                    }
                    cx.focus.focus(self.actions.id);
                    if r.has_actions() {
                        self.open_menu();
                    }
                    return Outcome::Changed;
                }
                if ev.is(Phase::Click)
                    && ev.on(self.grid.id)
                    && ev.id != scrollbar::id_for(self.grid.id)
                {
                    cx.focus.focus(self.grid.id);
                }
                if let Some(r) = self.grid.route(ev) {
                    let (o, actions) = r.into_parts();
                    for gev in actions {
                        self.on_grid(Some(gev), cx);
                    }
                    return o;
                }
                Outcome::Ignored
            }
            _ => Outcome::Ignored,
        }
    }

    fn hints(&self, focus: Option<WidgetId>) -> Vec<Hint> {
        if self.menu.is_open() {
            return MENU_HINTS.to_vec();
        }
        if let Some(h) = self.template.hints(focus) {
            return h;
        }
        if focus == Some(self.grid.id) {
            return vec![("↑↓←→", "Cell"), ("Ctrl+C", "Copy")];
        }
        vec![("Enter", "Activate")]
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
