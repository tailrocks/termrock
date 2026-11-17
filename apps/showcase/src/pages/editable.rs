use ratatui::buffer::Buffer;
use ratatui::crossterm::event::KeyCode;
use ratatui::layout::{Constraint, Rect};

use crate::pages::layout::{sheet_height, sheet_width};
use crate::pages::tables::route_table;
use crate::pages::template::{Section, Template};
use crate::pages::{Cx, Hint, Page, PageEvent};
use termrock::core::event::Outcome;
use termrock::core::focus::BlurAction;
use termrock::core::id::WidgetId;
use termrock::core::keymap::{KeyBinding, KeyMap, NavKeys, hint_key};
use termrock::core::response::Actions;
use termrock::core::scroll::{HScroll, ScrollbarVisibility, TrackClick};
use termrock::ui::columns::HeaderLabels;
use termrock::ui::ctx::RenderCtx;
use termrock::ui::layout::StackChild;
use termrock::ui::reveal::Reveal;
use termrock::ui::specimen::{SpecimenState, StateGrid};
use termrock::widgets::empty::EmptyState;
use termrock::widgets::option_editor::{Affordance, ColumnEditor, EditorConfig, Options};
use termrock::widgets::panel::Panel;
use termrock::widgets::props::{self, Prop};
use termrock::widgets::scrollbar;
use termrock::widgets::table::{Align, Cell, Column, DataTable, TableAction, TableCmd, Tone};

const ID: WidgetId = WidgetId::of("editable");

fn validate(col: usize, s: &str) -> Option<String> {
    match col {
        1 if s.trim().is_empty() => Some("Task name cannot be empty".into()),
        4 if s.contains(' ') => Some("Branch names cannot contain spaces".into()),
        5 if s.parse::<u32>().is_err() => Some("Changes must be a whole number".into()),
        _ => None,
    }
}

fn columns() -> Vec<Column> {
    vec![
        Column::new("ID", Constraint::Length(5)),
        Column::new("Task", Constraint::Min(24)).editable(),
        Column::new("Owner", Constraint::Length(8)).editor(ColumnEditor::for_options(
            Options::values(&[
                "ana", "jonas", "kai", "lena", "mira", "omar", "priya", "sofia",
            ]),
        )),
        Column::new("Status", Constraint::Length(9)),
        Column::new("Branch", Constraint::Length(22)).editable(),
        Column::new("Changes", Constraint::Length(8))
            .right()
            .editable(),
    ]
}

fn rows(n: usize) -> Vec<Vec<Cell>> {
    crate::data::tasks()
        .into_iter()
        .take(n)
        .map(|r| {
            vec![
                Cell::new(format!("#{}", r.id)).tone(Tone::Muted),
                Cell::new(r.name),
                Cell::new(r.owner),
                crate::pages::tables::status_cell(r.status),
                Cell::new(r.branch).tone(Tone::Muted),
                Cell::new(r.changes.to_string()),
            ]
        })
        .collect()
}

const KEYS: [&str; 13] = [
    "editable.rows",
    "editable.type_ahead",
    "editable.affordance",
    "editable.overflow_marks",
    "editable.numeric",
    "editable.nav",
    "editable.keymap",
    "editable.horizontal",
    "editable.reveal",
    "editable.track_click",
    "editable.blur_action",
    "editable.header_labels",
    "editable.tab_walkthrough",
];

fn custom_keymap() -> KeyMap<TableCmd> {
    vec![
        (KeyBinding::char('o'), TableCmd::SortByColumn),
        (KeyBinding::ctrl('a'), TableCmd::SelectAll),
    ]
}

fn build(k: &[usize; 13]) -> DataTable {
    let [
        rows_k,
        type_ahead,
        affordance,
        marks,
        numeric,
        nav,
        keymap,
        horizontal,
        reveal,
        track,
        blur,
        labels,
        walkthrough,
    ] = *k;
    let data = if rows_k == 0 { rows(14) } else { Vec::new() };
    let mut table = DataTable::try_new(ID.sub("table"), columns(), data)
        .expect("rectangular fixture")
        .cell_nav(true)
        .numeric(if numeric == 1 { &[0, 5] } else { &[] })
        .validator(validate)
        .type_ahead(type_ahead > 0)
        .type_ahead_column((type_ahead == 2).then_some(1))
        .editor_config(EditorConfig {
            affordance: [Affordance::Focused, Affordance::Always, Affordance::Never][affordance],
            ..EditorConfig::default()
        })
        .overflow_marks(marks == 1)
        .nav([NavKeys::DEFAULT, NavKeys::VIM][nav])
        .empty(EmptyState::new("No tasks yet").hint("Tasks you create show up here"));
    if keymap == 1 {
        table = table.keymap(custom_keymap());
    }
    if horizontal == 1 {
        table = table.horizontal(HScroll {
            bar: ScrollbarVisibility::Never,
            ..HScroll::default()
        });
    }
    if reveal == 0 {
        table = table.reveal(Reveal::OFF);
    }
    if track > 0 {
        table = table.track_click([TrackClick::Page, TrackClick::Page, TrackClick::Jump][track]);
    }
    table.blur_action = [BlurAction::Commit, BlurAction::Cancel][blur];
    if labels == 1 {
        table.header_labels = HeaderLabels {
            sort_asc: "Sort A to Z",
            sort_desc: "Sort Z to A",
            clear_sort: "Unsort",
            autofit: "Fit Width",
            reset_widths: "Default Widths",
        };
    }
    if !table.rows().is_empty() {
        table.update_cell(2, 4, |cell| {
            cell.error = Some("Branch names cannot contain spaces".into());
            cell.text = "fix/checkout flake".into();
        });
    }
    table.cursor_col = if walkthrough == 0 { 1 } else { LAST_COL };
    table
}

/// Tab at the last editable cell hands focus on. The walkthrough starts there.
const LAST_COL: usize = 5;

/// What one pass through each walkthrough value shows, by option value.
const WALKTHROUGH: [Option<(&str, &str)>; 4] = [
    None,
    Some((
        "Enter, 7, Tab",
        "Committed then LeaveForward · the host saves, then focus moves on",
    )),
    Some((
        "Enter, Tab",
        "LeaveForward only · an unchanged value is a valid no-op commit",
    )),
    Some((
        "Enter, x, Tab",
        "No action · the editor stays open on its error and focus does not move",
    )),
];

fn action_name(a: &TableAction) -> &'static str {
    match a {
        TableAction::Committed { .. } => "Committed",
        TableAction::Cancelled => "Cancelled",
        TableAction::Activated(_) => "Activated",
        TableAction::SelectionChanged => "SelectionChanged",
        TableAction::LeaveForward => "LeaveForward",
        TableAction::LeaveBackward => "LeaveBackward",
    }
}

const EVENTS: &[(&str, &str)] = &[
    ("Enter / F2 / click twice", "Edit · Enter again Committed"),
    ("Esc", "Cancel the edit · Cancelled"),
    (
        "Tab at the last cell",
        "LeaveForward · Shift+Tab LeaveBackward",
    ),
    ("Enter on a row (Basic table)", "Activated"),
    ("Space (Selection)", "SelectionChanged"),
    (
        "s or a header click",
        "Sort ▴ ascending, again ▾ descending",
    ),
];

fn states_grid() -> StateGrid {
    use SpecimenState::*;
    let id = ID.sub("states");
    let sample = |k: usize, owner: bool| {
        let mut cols = vec![
            Column::new("ID", Constraint::Length(5)),
            Column::new("Task", Constraint::Min(14)).editable(),
        ];
        let mut keep = vec![0, 1];
        if owner {
            cols.push(Column::new("Owner", Constraint::Length(8)).editor(
                ColumnEditor::for_options(Options::values(&["ana", "jonas", "kai", "mira"])),
            ));
            keep.push(2);
        } else {
            cols.push(
                Column::new("Changes", Constraint::Length(8))
                    .right()
                    .editable(),
            );
            keep.push(5);
        }
        let data = rows(3)
            .into_iter()
            .map(|r| keep.iter().map(|&c| r[c].clone()).collect())
            .collect();
        let mut t = DataTable::try_new(id.child(k), cols, data)
            .expect("rectangular fixture")
            .cell_nav(true)
            .numeric(&[0]);
        if owner {
            t = t.editor_config(EditorConfig {
                affordance: Affordance::Always,
                ..EditorConfig::default()
            });
        }
        t.cursor_col = 1;
        t
    };
    StateGrid::new(id)
        .rows(&[
            Rest, Hover, Pressed, Focused, Editing, Error, Disabled, Empty, Overflow, Narrow,
        ])
        .column_width(44)
        .narrow_width(28)
        .column("Text cells", sample(0, false))
        .column("Constrained (▾ always)", sample(1, true))
}

pub struct EditablePage {
    template: Template,
    knobs: [usize; 13],
    pub table: DataTable,
    sections: crate::pages::layout::Sections,
    edits: u32,
    last: String,
}

impl EditablePage {
    pub fn new() -> Self {
        let knobs = KEYS.map(|k| crate::options::find(k).map_or(0, |o| o.default));
        Self {
            template: Template::new(ID, states_grid()),
            table: build(&knobs),
            knobs,
            sections: crate::pages::layout::Sections::new(ID.sub("page")),
            edits: 0,
            last: "—".into(),
        }
    }

    fn column_props(&self) -> Vec<Prop> {
        self.table
            .columns()
            .iter()
            .map(|c| {
                let mut v = vec![match c.align {
                    Align::Left => "left",
                    Align::Right => "right",
                }];
                v.push(match &c.editor {
                    ColumnEditor::Text if c.editable => "editable · text",
                    ColumnEditor::Text => "read-only",
                    _ => "editable · ▾ Select",
                });
                Prop::new(&c.title, v.join(" · "))
            })
            .collect()
    }

    fn walkthrough(&self) -> usize {
        self.knobs[KEYS.len() - 1]
    }

    /// Hands every action of one input to the host in order. The readout keeps
    /// the batch, and a Tab that ended an edit without any action says so.
    fn on_actions(&mut self, actions: Actions<TableAction>, tab_in_edit: bool, cx: &mut Cx) {
        let names: Vec<&str> = actions.iter().map(action_name).collect();
        if !names.is_empty() {
            self.last = names.join(" → ");
        } else if tab_in_edit {
            self.last = "none".into();
        }
        for tev in actions {
            self.on_event(Some(tev), cx);
        }
    }

    fn on_event(&mut self, tev: Option<TableAction>, cx: &mut Cx) {
        match tev {
            Some(TableAction::Committed { .. }) => {
                self.edits += 1;
                cx.status("Cell saved");
            }
            Some(TableAction::Cancelled) => cx.status("Edit cancelled"),
            Some(TableAction::LeaveForward) => cx.focus_next(),
            Some(TableAction::LeaveBackward) => cx.focus_prev(),
            _ => {}
        }
    }

    fn legend(&self) -> Vec<Prop> {
        let mut rows = Vec::new();
        if let Some((keys, shows)) = WALKTHROUGH[self.walkthrough()] {
            rows.push(Prop::new("Try", keys).wrap());
            rows.push(Prop::new("Shows", shows).wrap());
        }
        rows.extend(legend());
        rows
    }

    fn heights(&self, width: u16) -> [u16; 4] {
        let cols: Vec<Prop> = self.column_props().into_iter().map(Prop::wrap).collect();
        let events = events();
        let legend = self.legend();
        [
            (self.table.len() as u16 + 4).max(8),
            props::measure(&legend, width),
            sheet_height(&Panel::card(Some("Columns")), &cols, width),
            sheet_height(&Panel::card(Some("Events")), &events, width),
        ]
    }

    fn render_playground(&mut self, area: Rect, buf: &mut Buffer, ctx: &mut RenderCtx) {
        let t = ctx.theme;
        let cols: Vec<Prop> = self.column_props().into_iter().map(Prop::wrap).collect();
        let events = events();
        let focus = ctx.interaction.has_focus(self.table.id).then_some(0);
        let heights = self.heights(area.width);
        let l = self
            .sections
            .layout(area, &heights.map(StackChild::fixed), focus, buf, ctx);
        let err = self.table.edit_error().map(str::to_owned);
        match l.slot(0) {
            Some(card) => {
                let panel =
                    Panel::card(Some("Tasks")).focused(ctx.interaction.focused(self.table.id));
                let bg = panel.bg(t);
                let inner = panel.render(card, buf, t);
                self.table.render(inner, buf, ctx, bg);
                let pos = scrollbar::position_label(&self.table.scroll);
                let last = WALKTHROUGH[self.walkthrough()].map(|_| format!("last {}", self.last));
                let counts = if pos.is_empty() {
                    format!("{} edits", self.edits)
                } else {
                    format!("{} edits · {pos}", self.edits)
                };
                let meta = match (&err, &last) {
                    (Some(e), Some(last)) => format!("{e} · {last}"),
                    (Some(e), None) => e.clone(),
                    (None, Some(last)) => format!("{last} · {counts}"),
                    (None, None) => counts,
                };
                panel.draw_meta(card, buf, t, &meta);
            }
            None => ctx.focus_stop(self.table.id),
        }
        if let Some(r) = l.slot(1) {
            props::render(r, buf, t, &self.legend(), t.canvas);
        }
        for (slot, title, items) in [(2, "Columns", cols), (3, "Events", events)] {
            if let Some(r) = l.slot(slot) {
                let panel = Panel::card(Some(title));
                let bg = panel.bg(t);
                let inner = panel.render(r, buf, t);
                props::render(inner, buf, t, &items, bg);
            }
        }
    }
}

fn legend() -> Vec<Prop> {
    vec![
        Prop::new("reversed", "cell cursor (navigation)").wrap(),
        Prop::new("▁", "editing cursor + accent underline").wrap(),
        Prop::new("!", "validation error").wrap(),
        Prop::new("▾", "Owner: pick from a list (Enter, Space, Alt+↓)").wrap(),
    ]
}

fn events() -> Vec<Prop> {
    EVENTS
        .iter()
        .map(|(k, w)| Prop::new(k, *w).wrap())
        .collect()
}

impl Page for EditablePage {
    fn title(&self) -> &'static str {
        "Editable tables"
    }
    fn blurb(&self) -> &'static str {
        "Navigation is reversed cell; editing is a cursor. They never look alike."
    }

    fn measure(&self, avail: ratatui::layout::Size, ctx: &RenderCtx) -> ratatui::layout::Size {
        use termrock::ui::measure::Measure;
        let cols: Vec<Prop> = self.column_props().into_iter().map(Prop::wrap).collect();
        let w = [
            self.table.min_width(ratatui::layout::Size::new(0, 0), ctx) + 4,
            props::width(&self.legend()),
            sheet_width(&Panel::card(Some("Columns")), &cols),
            sheet_width(&Panel::card(Some("Events")), &events()),
        ]
        .into_iter()
        .fold(avail.width, u16::max);
        let h = self.heights(w).iter().sum::<u16>() + 3;
        ratatui::layout::Size::new(w, h)
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
        self.table = build(&self.knobs);
    }

    fn handle(&mut self, ev: &PageEvent, cx: &mut Cx) -> Outcome {
        if let Some(o) = self.template.handle(ev, cx) {
            return o;
        }
        if let PageEvent::Blur(ch) = ev {
            if ch.from != self.table.id {
                return Outcome::Ignored;
            }
            let (out, actions) = self.table.on_blur().into_parts();
            self.on_actions(actions, false, cx);
            return out;
        }
        if let Some(o) = self.sections.handle(ev) {
            return o;
        }
        match ev {
            PageEvent::Key(key) => {
                if !cx.focus.is(self.table.id) {
                    return Outcome::Ignored;
                }
                let tab_in_edit =
                    self.table.is_editing() && matches!(key.code, KeyCode::Tab | KeyCode::BackTab);
                let (out, actions) = self.table.on_key(key).into_parts();
                self.on_actions(actions, tab_in_edit, cx);
                out
            }
            PageEvent::Paste(text) => self.table.on_paste(text),
            PageEvent::Pointer(p) => {
                let Some((out, tev)) = route_table(&mut self.table, p, cx) else {
                    return Outcome::Ignored;
                };
                self.on_actions(tev, false, cx);
                out
            }
            _ => Outcome::Ignored,
        }
    }

    fn editing(&self) -> bool {
        self.table.is_editing()
    }

    fn hints(&self, focus: Option<WidgetId>) -> Vec<Hint> {
        if let Some(h) = self.template.hints(focus) {
            return h;
        }
        if self.table.is_editing() {
            let at_last = self.table.cursor_col == LAST_COL;
            let tab = match self.walkthrough() {
                1 if at_last => "Commit, then leave",
                2 if at_last => "Leave, nothing to save",
                3 if at_last => "Blocked by an error",
                _ => "Next cell",
            };
            return vec![("Enter", "Apply"), ("Esc", "Cancel"), ("Tab", tab)];
        }
        let mut h = vec![("↑↓←→", "Cell"), termrock::widgets::widget_hints::EDIT_PAIR];

        if let Some(k) = hint_key(&self.table.keymap, TableCmd::SortByColumn) {
            h.push((k, "Sort"));
        }
        h.push(("click twice", "Edit"));
        h
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
}

#[cfg(test)]
mod tests {
    use super::*;
    use ratatui::crossterm::event::{KeyCode, KeyModifiers};
    use termrock::core::event::Key;
    use termrock::core::focus::{Focus, FocusRing};
    use termrock::shell::Request;

    fn key(code: KeyCode) -> PageEvent {
        PageEvent::Key(Key {
            code,
            mods: KeyModifiers::NONE,
        })
    }

    /// Editing the last column and pressing Tab commits the cell and hands focus
    /// on. The host must observe both actions, not just the leave.
    #[test]
    fn app_dispatch_drains_compound_response() {
        let mut page = EditablePage::new();
        let after = WidgetId::of("editable.after");
        let mut ring = FocusRing::default();
        ring.register(page.table.id);
        ring.register(after);
        let mut focus = Focus::default();
        focus.focus(page.table.id);
        let mut cx = Cx::new(&mut focus, &ring);

        page.table.cursor_col = 5;
        let _ = page.handle(&key(KeyCode::Enter), &mut cx);
        assert!(page.table.is_editing());
        let _ = page.handle(&key(KeyCode::Char('7')), &mut cx);
        let _ = page.handle(&key(KeyCode::Tab), &mut cx);

        assert_eq!(page.edits, 1, "the commit reached the host");
        assert!(
            cx.requests
                .iter()
                .any(|r| matches!(r, Request::Status(s, _) if s == "Cell saved")),
            "the saved indicator was raised"
        );
        assert_eq!(cx.focus.current(), Some(after), "focus left the table");
    }

    struct Rig {
        page: EditablePage,
        after: WidgetId,
        ring: FocusRing,
        focus: Focus,
    }

    fn rig() -> Rig {
        let page = EditablePage::new();
        let after = WidgetId::of("editable.after");
        let mut ring = FocusRing::default();
        ring.register(page.table.id);
        ring.register(after);
        let mut focus = Focus::default();
        focus.focus(page.table.id);
        Rig {
            page,
            after,
            ring,
            focus,
        }
    }

    fn saved(cx: &Cx) -> bool {
        cx.requests
            .iter()
            .any(|r| matches!(r, Request::Status(s, _) if s == "Cell saved"))
    }

    /// An unchanged valid edit at the last cell is a no-op commit: focus still
    /// moves on, but the host must not be told a cell was saved.
    #[test]
    fn edge_tab_without_a_change_leaves_without_saving() {
        let mut r = rig();
        let mut cx = Cx::new(&mut r.focus, &r.ring);
        r.page.table.cursor_col = 5;
        let _ = r.page.handle(&key(KeyCode::Enter), &mut cx);
        assert!(r.page.table.is_editing());
        let _ = r.page.handle(&key(KeyCode::Tab), &mut cx);

        assert_eq!(r.page.edits, 0, "nothing was committed");
        assert!(!saved(&cx), "no Cell saved indicator");
        assert!(!r.page.table.is_editing());
        assert_eq!(cx.focus.current(), Some(r.after), "focus left the table");
    }

    /// An invalid edit blocks Tab: the editor stays open on its error, nothing
    /// is saved and focus does not move.
    #[test]
    fn invalid_tab_keeps_editing_and_reports_nothing() {
        let mut r = rig();
        let table_id = r.page.table.id;
        let mut cx = Cx::new(&mut r.focus, &r.ring);
        r.page.table.cursor_col = 4;
        let _ = r.page.handle(&key(KeyCode::Enter), &mut cx);
        let _ = r.page.handle(&key(KeyCode::Char(' ')), &mut cx);
        let _ = r.page.handle(&key(KeyCode::Tab), &mut cx);

        assert!(r.page.table.is_editing(), "the editor stays open");
        assert!(r.page.table.edit_error().is_some(), "the error is shown");
        assert_eq!(r.page.edits, 0);
        assert!(!saved(&cx), "no Cell saved indicator");
        assert_eq!(cx.focus.current(), Some(table_id), "focus did not move");
    }
    fn walk(scenario: usize, typed: Option<char>) -> (Rig, String, bool) {
        let mut r = rig();
        r.page.set_option("editable.tab_walkthrough", scenario);
        let mut cx = Cx::new(&mut r.focus, &r.ring);
        assert_eq!(r.page.table.cursor_col, LAST_COL, "starts on the last cell");
        let _ = r.page.handle(&key(KeyCode::Enter), &mut cx);
        if let Some(c) = typed {
            let _ = r.page.handle(&key(KeyCode::Char(c)), &mut cx);
        }
        let _ = r.page.handle(&key(KeyCode::Tab), &mut cx);
        let left = cx.focus.current() == Some(r.after);
        let last = r.page.last.clone();
        (r, last, left)
    }

    #[test]
    fn walkthrough_edge_tab_lists_commit_then_leave() {
        let (r, last, left) = walk(1, Some('7'));
        assert_eq!(last, "Committed → LeaveForward");
        assert_eq!(r.page.edits, 1);
        assert!(left, "focus left the table");
    }

    #[test]
    fn walkthrough_no_op_tab_lists_leave_only() {
        let (r, last, left) = walk(2, None);
        assert_eq!(last, "LeaveForward");
        assert_eq!(r.page.edits, 0);
        assert!(left, "focus left the table");
    }

    #[test]
    fn walkthrough_invalid_tab_lists_no_action_and_stays() {
        let (r, last, left) = walk(3, Some('x'));
        assert_eq!(last, "none");
        assert!(r.page.table.is_editing(), "the editor stays open");
        assert!(r.page.table.edit_error().is_some());
        assert!(!left, "focus did not move");
    }

    #[test]
    fn walkthrough_is_listed_on_the_options_tab_and_hinted() {
        let opt = crate::options::find("editable.tab_walkthrough").expect("option");
        assert_eq!(opt.values, &["free", "commit", "no-op", "invalid"]);
        let mut page = EditablePage::new();
        assert_eq!(page.table.cursor_col, 1, "free keeps the default cell");
        page.set_option("editable.tab_walkthrough", 1);
        let _ = page.table.begin_edit();
        let hints = page.hints(None);
        assert!(hints.contains(&("Tab", "Commit, then leave")), "{hints:?}");
        page.table.cursor_col = 1;
        let hints = page.hints(None);
        assert!(hints.contains(&("Tab", "Next cell")), "{hints:?}");
    }
}
