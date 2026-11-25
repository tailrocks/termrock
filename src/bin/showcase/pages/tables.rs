use ratatui::buffer::Buffer;
use ratatui::layout::{Constraint, Rect};

use crate::data::TaskStatus;
use crate::pages::row_menu::{MENU_HINTS, OPEN_HINT, RowMenu, plain};
use crate::pages::template::{Section, Template};
use crate::pages::{Cx, Hint, Page, PageEvent};
use junie_tui::core::data::{DataError, ShapeError, validate_rectangular};
use junie_tui::core::event::Outcome;
use junie_tui::core::id::WidgetId;
use junie_tui::core::intent::Phase;
use junie_tui::core::pointer::{MouseTarget, PointerEvent};
use junie_tui::core::response::Actions;
use junie_tui::core::selection::{SelectionMode, TriState};
use junie_tui::ui::context::ContextRequest;
use junie_tui::ui::ctx::RenderCtx;
use junie_tui::ui::layout::StackChild;
use junie_tui::ui::specimen::{SpecimenState, StateGrid};
use junie_tui::widgets::menu::{ContextMenu, MenuItem};
use junie_tui::widgets::panel::Panel;
use junie_tui::widgets::scrollbar;
use junie_tui::widgets::table::{Cell, Column, DataTable, SortDir, TableAction, Tone};
use ratatui::crossterm::event::KeyCode;

const ID: WidgetId = WidgetId::of("tables");
const MENU: WidgetId = WidgetId::of("tables.menu");
const HEADER_MENU: WidgetId = WidgetId::of("tables.header-menu");

pub fn task_columns() -> Vec<Column> {
    vec![
        Column::new("ID", Constraint::Length(5)),
        Column::new("Task", Constraint::Min(24)),
        Column::new("Owner", Constraint::Length(7)),
        Column::new("Status", Constraint::Length(9)),
        Column::new("Branch", Constraint::Length(20)),
        Column::new("Changes", Constraint::Length(9)).right(),
        Column::new("Duration", Constraint::Length(9)).right(),
    ]
}

pub fn status_cell(s: TaskStatus) -> Cell {
    let (tone, text) = match s {
        TaskStatus::Running => (Tone::Normal, "▸ Running"),
        TaskStatus::Failed => (Tone::Error, "Failed"),
        TaskStatus::Paused => (Tone::Warning, "Paused"),
        TaskStatus::Queued => (Tone::Muted, "Queued"),
        TaskStatus::Done => (Tone::Secondary, "Done"),
    };
    Cell::new(text).tone(tone)
}

pub fn task_rows() -> Vec<Vec<Cell>> {
    crate::data::tasks()
        .into_iter()
        .map(|r| {
            let dur = if r.duration_s == 0 {
                "0s".to_owned()
            } else if r.duration_s >= 60 {
                format!("{}m {:02}s", r.duration_s / 60, r.duration_s % 60)
            } else {
                format!("{}s", r.duration_s)
            };
            vec![
                Cell::new(format!("#{}", r.id)).tone(Tone::Muted),
                Cell::new(r.name),
                Cell::new(r.owner),
                status_cell(r.status),
                Cell::new(r.branch).tone(Tone::Muted),
                Cell::new(r.changes.to_string()).tone(if r.changes == 0 {
                    Tone::Muted
                } else {
                    Tone::Normal
                }),
                Cell::new(dur),
            ]
        })
        .collect()
}

const SELECTION_MODES: [SelectionMode; 3] = [
    SelectionMode::None,
    SelectionMode::Single,
    SelectionMode::Multi,
];

fn tasks_table(mode: SelectionMode) -> DataTable {
    DataTable::try_new(ID.sub("tasks"), task_columns(), task_rows())
        .expect("rectangular fixture")
        .numeric(&[0, 5, 6])
        .selection_mode(mode)
        .selection_column(mode == SelectionMode::Multi)
}

fn states_grid() -> StateGrid {
    use SpecimenState::*;
    let id = ID.sub("states");
    let sample = |k: usize, mode: SelectionMode| {
        let cols = vec![
            Column::new("ID", Constraint::Length(5)),
            Column::new("Task", Constraint::Min(10)),
            Column::new("Status", Constraint::Length(9)),
        ];
        let rows = task_rows()
            .into_iter()
            .take(3)
            .map(|r| vec![r[0].clone(), r[1].clone(), r[3].clone()])
            .collect();
        DataTable::try_new(id.child(k), cols, rows)
            .expect("rectangular fixture")
            .numeric(&[0])
            .selection_mode(mode)
            .selection_column(mode == SelectionMode::Multi)
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
            Overflow,
            Narrow,
        ])
        .column_width(38)
        .narrow_width(26)
        .column("Single selection", sample(0, SelectionMode::Single))
        .column("No selection", sample(1, SelectionMode::None))
        .column("Multi selection", sample(2, SelectionMode::Multi))
}

pub struct TablesPage {
    template: Template,

    selection: usize,
    malformed: usize,
    pub table: DataTable,

    pub reviewers: DataTable,
    empty: DataTable,
    sections: crate::pages::layout::Sections,

    menu: RowMenu<(usize, usize)>,

    header_menu: RowMenu<usize>,
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum CellAction {
    Select,
    Sort,
    Edit,
}

impl TablesPage {
    fn cell_items(&self, (row, col): (usize, usize)) -> Vec<(CellAction, MenuItem)> {
        let t = &self.table;
        let src = t.source_row(row);
        let select = MenuItem::new("Select").shortcut("Enter");
        let select = if t.is_disabled(src) {
            select.reason("Unavailable")
        } else if t.selected == Some(src) {
            select.reason("Already selected")
        } else {
            select
        };
        let column = &t.columns()[col];
        let name = column.title.to_lowercase();
        let sort = match t.sort {
            Some((c, SortDir::Asc)) if c == col => MenuItem::new(format!("Sort {name} descending")),
            Some((c, SortDir::Desc)) if c == col => MenuItem::new("Clear sort"),
            _ => MenuItem::new(format!("Sort by {name}")),
        };

        let sort = if !column.sortable {
            sort.reason("Not sortable")
        } else if col == t.cursor_col {
            sort.shortcut("s")
        } else {
            sort
        };
        let edit = MenuItem::new("Edit cell");

        let edit = if column.editable && t.cell_nav && col == t.cursor_col {
            edit.shortcut("F2")
        } else {
            edit.reason("Read-only")
        };
        vec![
            (CellAction::Select, select.separator()),
            (CellAction::Sort, sort.separator()),
            (CellAction::Edit, edit),
        ]
    }

    fn open_menu(&mut self, req: ContextRequest<(usize, usize)>) {
        let src = self.table.source_row(req.target.0);
        let title = self.table.rows()[src][1].text.clone();
        let items = self
            .cell_items(req.target)
            .into_iter()
            .map(|(_, m)| m)
            .collect();
        self.menu
            .open(req.menu(MENU, items).title(title), req.target);
    }

    fn choose(&mut self, cell: (usize, usize), i: usize, cx: &mut Cx) -> Outcome {
        let Some(&(action, _)) = self.cell_items(cell).get(i) else {
            return Outcome::Consumed;
        };
        match action {
            CellAction::Select => {
                self.table.cursor_row = cell.0;
                let (out, actions) = self.table.on_key(&plain(KeyCode::Enter)).into_parts();
                for tev in actions {
                    if let TableAction::Activated(row) = tev {
                        cx.status(format!("Selected {}", self.table.rows()[row][1].text));
                    }
                }
                out
            }
            // This page has no edit consumer, so the commit actions are dropped on purpose.
            CellAction::Sort => self.table.on_click_header(cell.1).erase().outcome(),
            CellAction::Edit => self.table.begin_edit().erase().outcome(),
        }
    }
    pub fn new() -> Self {
        let selection = 1;
        let table = tasks_table(SELECTION_MODES[selection]);

        let mut reviewers = DataTable::try_new(
            ID.sub("reviewers"),
            vec![
                Column::new("Reviewer", Constraint::Min(12)),
                Column::new("Team", Constraint::Length(10)),
                Column::new("Open reviews", Constraint::Length(12)).right(),
            ],
            [
                ("mira", "platform", "3"),
                ("jonas", "payments", "0"),
                ("ana", "platform", "5"),
                ("kai", "infra", "1"),
            ]
            .into_iter()
            .map(|(a, b, c)| vec![Cell::new(a), Cell::new(b).tone(Tone::Muted), Cell::new(c)])
            .collect(),
        )
        .expect("rectangular fixture")
        .numeric(&[2])
        .selection_mode(SelectionMode::Multi)
        .selection_column(true);
        reviewers.toggle_row(0);
        reviewers.toggle_row(2);
        let empty = checks_table(0);
        Self {
            template: Template::new(ID, states_grid()),
            selection,
            malformed: 0,
            table,
            reviewers,
            empty,
            sections: crate::pages::layout::Sections::new(ID.sub("page")),
            menu: RowMenu::default(),
            header_menu: RowMenu::default(),
        }
    }
}

fn checks_table(malformed: usize) -> DataTable {
    let mut table = DataTable::try_new(
        ID.sub("empty"),
        vec![
            Column::new("Check", Constraint::Min(12)),
            Column::new("Result", Constraint::Length(8)),
        ],
        vec![],
    )
    .expect("rectangular fixture")
    .empty_text("No checks have run yet");
    if let Err(e) = feed_malformed(&mut table, malformed) {
        table.empty_text = format!("Rows rejected · {e} ({})", kind_of(&e));
    }
    table
}

/// Offers the table rows a producer got wrong. Each call checks the rows with
/// `validate_rectangular` first, the way a caller would before it commits, and
/// the table then refuses them on its own as well.
fn feed_malformed(table: &mut DataTable, malformed: usize) -> Result<(), DataError> {
    let width = table.columns().len();
    let ok = || vec![Cell::new("lint"), Cell::new("ok")];
    match malformed {
        1 | 2 => {
            let mut cells = vec![Cell::new("lint")];
            if malformed == 2 {
                cells.extend([Cell::new("ok"), Cell::new("extra")]);
            }
            let rows = vec![cells];
            validate_rectangular(width, &rows)?;
            table.try_set_rows(rows)
        }
        3 => {
            let rows = vec![ok()];
            validate_rectangular(0, &rows)?;
            DataTable::try_new(ID.sub("no-columns"), Vec::new(), rows).map(|_| ())
        }
        4 => table.try_set_row(2, ok()),
        _ => Ok(()),
    }
}

fn kind_of(error: &DataError) -> String {
    match error {
        DataError::Shape(ShapeError { row, .. }) => format!("shape, row {row}"),
        DataError::EmptySchemaWithRows => "empty schema".to_owned(),
        DataError::MissingRow(row) => format!("missing row {row}"),
    }
}

pub fn route_table(
    t: &mut DataTable,
    ev: &PointerEvent,
    cx: &mut Cx,
) -> Option<(Outcome, Actions<TableAction>)> {
    let (out, action) = t.route(ev)?.into_parts();
    if ev.is(Phase::Click) && ev.bar.is_none() && out != Outcome::Ignored {
        cx.focus.focus(t.id);
    }
    Some((out, action))
}

impl TablesPage {
    fn pointer(&mut self, ev: &PointerEvent, cx: &mut Cx) -> Outcome {
        if ev.is(Phase::Secondary) {
            if let Some(c) = self.table.header_menu_at(ev) {
                cx.focus.focus(self.table.id);
                let items = self
                    .table
                    .header_menu(c)
                    .into_iter()
                    .map(|(_, m)| m)
                    .collect();
                let title = self.table.columns()[c].title.clone();
                let menu = ContextMenu::new(HEADER_MENU, items)
                    .at(ev.pos())
                    .title(title);
                self.header_menu.open(menu, c);
                return Outcome::Changed;
            }
            let cfg = self.menu.config;
            let Some(req) = cfg.on_secondary(&mut self.table, ev) else {
                return Outcome::Ignored;
            };
            cx.focus.focus(self.table.id);
            self.open_menu(req);
            return Outcome::Changed;
        }
        if let Some((out, tev)) = route_table(&mut self.table, ev, cx) {
            for action in tev {
                if let TableAction::Activated(r) = action {
                    cx.status(format!("Selected {}", self.table.rows()[r][1].text));
                }
            }
            return out;
        }
        if let Some((out, _)) = route_table(&mut self.reviewers, ev, cx) {
            return out;
        }
        if let Some((out, _)) = route_table(&mut self.empty, ev, cx) {
            return out;
        }
        Outcome::Ignored
    }

    fn render_playground(&mut self, area: Rect, buf: &mut Buffer, ctx: &mut RenderCtx) {
        let t = ctx.theme;

        let focus = if ctx.interaction.has_focus(self.table.id) {
            Some(0)
        } else if ctx.interaction.has_focus(self.reviewers.id) {
            Some(1)
        } else if ctx.interaction.has_focus(self.empty.id) {
            Some(2)
        } else {
            None
        };
        let l = self.sections.layout(
            area,
            &[
                StackChild::fixed(TASKS_H),
                StackChild::fixed(REVIEWERS_H),
                StackChild::fixed(CHECKS_H),
            ],
            focus,
            buf,
            ctx,
        );
        let sort = match self.table.sort {
            Some((c, d)) => format!(
                "sorted by {} {}",
                self.table.columns()[c].title.to_lowercase(),
                if d == junie_tui::widgets::table::SortDir::Asc {
                    "▴"
                } else {
                    "▾"
                }
            ),
            None => "unsorted".to_owned(),
        };
        let panel = Panel::card(Some("Tasks")).focused(ctx.interaction.focused(self.table.id));
        let bg = panel.bg(t);
        match l.slot(0) {
            Some(r) => {
                let inner = panel.render(r, buf, t);
                self.table.render(inner, buf, ctx, bg);
                let pos = scrollbar::position_label(&self.table.scroll);
                let meta = if pos.is_empty() {
                    sort
                } else {
                    format!("{sort} · {pos}")
                };
                panel.draw_meta(r, buf, t, &meta);
            }
            None => ctx.focus_stop(self.table.id),
        }

        let panel =
            Panel::card(Some("Reviewers")).focused(ctx.interaction.focused(self.reviewers.id));
        let bg = panel.bg(t);
        match l.slot(1) {
            Some(r) => {
                let inner = panel.render(r, buf, t);
                self.reviewers.render(inner, buf, ctx, bg);
                let state = match self.reviewers.selection_state() {
                    TriState::None => "none",
                    TriState::Mixed => "mixed",
                    TriState::All => "all",
                };
                let meta = format!("{} · header {state}", self.reviewers.selection_label());
                panel.draw_meta(r, buf, t, &meta);
            }
            None => ctx.focus_stop(self.reviewers.id),
        }

        let panel = Panel::card(Some("Checks"));
        let bg = panel.bg(t);
        match l.slot(2) {
            Some(r) => {
                let inner = panel.render(r, buf, t);
                self.empty.render(inner, buf, ctx, bg);
            }
            None => ctx.focus_stop(self.empty.id),
        }
        self.menu.render(area, buf, ctx);
        self.header_menu.render(area, buf, ctx);
    }
}

const TASKS_H: u16 = 3 + 1 + 20 + 1;
const REVIEWERS_H: u16 = 8;
const CHECKS_H: u16 = 6;

impl Page for TablesPage {
    fn title(&self) -> &'static str {
        "Tables"
    }
    fn blurb(&self) -> &'static str {
        "Sort by header, hover rows, select with Enter, overflow scrolls"
    }

    fn measure(&self, avail: ratatui::layout::Size, ctx: &RenderCtx) -> ratatui::layout::Size {
        use junie_tui::ui::measure::Measure;
        let any = ratatui::layout::Size::new(0, 0);
        let w = [&self.table, &self.reviewers, &self.empty]
            .iter()
            .map(|t| t.min_width(any, ctx) + 4)
            .fold(avail.width, u16::max);
        ratatui::layout::Size::new(w, TASKS_H + REVIEWERS_H + CHECKS_H + 2)
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
        self.menu.hides_page_focus() || self.header_menu.hides_page_focus()
    }

    fn option_value(&self, key: &str) -> Option<usize> {
        match key {
            "basictable.selection" => Some(self.selection),
            "basictable.malformed" => Some(self.malformed),
            _ => None,
        }
    }

    fn set_option(&mut self, key: &str, value: usize) {
        match key {
            "basictable.selection" => {
                self.selection = value.min(SELECTION_MODES.len() - 1);
                self.table = tasks_table(SELECTION_MODES[self.selection]);
            }
            "basictable.malformed" => {
                self.malformed = value.min(4);
                self.empty = checks_table(self.malformed);
            }
            _ => {}
        }
    }

    fn template(&self) -> Option<&Template> {
        Some(&self.template)
    }

    fn as_any(&self) -> Option<&dyn std::any::Any> {
        Some(self)
    }

    fn template_mut(&mut self) -> Option<&mut Template> {
        Some(&mut self.template)
    }

    fn handle(&mut self, ev: &PageEvent, cx: &mut Cx) -> Outcome {
        if let Some(o) = self.template.handle(ev, cx) {
            return o;
        }
        if let Some((out, chosen)) = self.header_menu.handle(ev) {
            return match chosen.and_then(|(c, i)| Some((c, self.table.header_menu(c).get(i)?.0))) {
                Some((c, action)) => self.table.on_header_action(c, action).or(out),
                None => out,
            };
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
                if cx.focus.is(self.table.id) {
                    if !self.table.is_editing()
                        && let Some(req) = self.menu.config.on_key(&self.table, key)
                    {
                        self.open_menu(req);
                        return Outcome::Changed;
                    }
                    let (out, actions) = self.table.on_key(key).into_parts();
                    for tev in actions {
                        if let TableAction::Activated(row) = tev {
                            cx.status(format!("Selected {}", self.table.rows()[row][1].text));
                        }
                    }
                    return out;
                }
                if cx.focus.is(self.reviewers.id) {
                    return self.reviewers.on_key(key).outcome();
                }
                if cx.focus.is(self.empty.id) {
                    return self.empty.on_key(key).outcome();
                }
                Outcome::Ignored
            }
            PageEvent::Pointer(p) => self.pointer(p, cx),
            _ => Outcome::Ignored,
        }
    }

    fn hints(&self, focus: Option<WidgetId>) -> Vec<Hint> {
        if let Some(h) = self.template.hints(focus) {
            return h;
        }
        if self.menu.is_open() {
            return MENU_HINTS.to_vec();
        }
        let multi = |t: &DataTable| t.selection_mode == SelectionMode::Multi;
        if focus == Some(self.reviewers.id) || (focus == Some(self.table.id) && multi(&self.table))
        {
            return vec![
                ("↑↓", "Move"),
                ("Space", "Toggle row"),
                ("Shift+↑↓", "Extend"),
            ];
        }
        vec![
            ("↑↓", "Move"),
            ("←→", "Columns"),
            ("s", "Sort column"),
            ("Enter", "Select"),
            OPEN_HINT,
        ]
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::pages::page_text;

    fn with_malformed(value: usize) -> String {
        let mut page = TablesPage::new();
        page.set_option("basictable.malformed", value);
        page_text(&mut page, 120, 70)
    }

    #[test]
    fn issue42_malformed_input_option_is_off_by_default() {
        let page = TablesPage::new();
        assert_eq!(page.option_value("basictable.malformed"), Some(0));
        let text = page_text(&mut TablesPage::new(), 120, 70);
        assert!(text.contains("No checks have run yet"), "{text}");
    }

    #[test]
    fn issue42_short_and_long_rows_show_the_data_error() {
        let short = with_malformed(1);
        assert!(
            short.contains("Rows rejected · row 0 has 1 cells, expected 2 (shape, row 0)"),
            "{short}"
        );
        let long = with_malformed(2);
        assert!(
            long.contains("Rows rejected · row 0 has 3 cells, expected 2 (shape, row 0)"),
            "{long}"
        );
        assert!(!long.contains("No checks have run yet"));
    }

    #[test]
    fn issue301_empty_schema_and_missing_row_show_the_data_error() {
        let empty = with_malformed(3);
        assert!(
            empty.contains(
                "Rows rejected · rows supplied for a schema with no columns (empty schema)"
            ),
            "{empty}"
        );
        let missing = with_malformed(4);
        assert!(
            missing.contains("Rows rejected · row 2 does not exist (missing row 2)"),
            "{missing}"
        );
    }
}
