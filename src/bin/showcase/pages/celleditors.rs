use ratatui::buffer::Buffer;
use ratatui::layout::{Constraint, Rect};

use crate::pages::layout::{sheet_height, sheet_width};
use crate::pages::tables::route_table;
use crate::pages::template::{Section, Template};
use crate::pages::{Cx, Hint, Page, PageEvent};
use junie_tui::core::event::Outcome;
use junie_tui::core::focus::BlurAction;
use junie_tui::core::id::WidgetId;
use junie_tui::core::intent::Phase;
use junie_tui::core::pointer::MouseTarget;
use junie_tui::theme::CursorStyle;
use junie_tui::ui::ctx::RenderCtx;
use junie_tui::ui::layout::StackChild;
use junie_tui::ui::popup::{Dismiss, Placement};
use junie_tui::ui::specimen::{SpecimenState, StateGrid};
use junie_tui::widgets::button::Button;
use junie_tui::widgets::option_editor::{
    Affordance, ChoiceEditor, ChoiceResult, ColumnEditor, EditorConfig, OptionItem, Options,
};
use junie_tui::widgets::panel::Panel;
use junie_tui::widgets::props::{self, Prop};
use junie_tui::widgets::table::{Cell, Column, DataTable, TableAction, Tone};

const ID: WidgetId = WidgetId::of("celleditors");

#[allow(dead_code)]
pub const ENV: usize = 2;
#[allow(dead_code)]
pub const REGION: usize = 3;
#[allow(dead_code)]
pub const ROLLOUT: usize = 4;
#[allow(dead_code)]
pub const OWNER: usize = 5;
#[allow(dead_code)]
pub const NOTIFY: usize = 6;
#[allow(dead_code)]
pub const LABELS: usize = 7;
#[allow(dead_code)]
pub const PRIORITY: usize = 8;

fn environments() -> Options {
    Options::new([
        OptionItem::new("dev").label("Development"),
        OptionItem::new("stage").label("Staging"),
        OptionItem::new("prod")
            .label("Production")
            .tone(Tone::Warning),
        OptionItem::new("sandbox").label("Sandbox"),
        OptionItem::new("preview").label("Preview"),
        OptionItem::new("perf").label("Performance"),
        OptionItem::new("dr")
            .label("Disaster recovery")
            .disabled("Frozen until Friday"),
    ])
}

fn regions() -> Options {
    let mut items: Vec<OptionItem> = [
        "us-east-1",
        "us-east-2",
        "us-west-1",
        "us-west-2",
        "ca-central-1",
        "sa-east-1",
        "eu-west-1",
        "eu-west-2",
        "eu-west-3",
        "eu-central-1",
        "eu-north-1",
        "ap-south-1",
        "ap-southeast-1",
        "ap-southeast-2",
        "ap-northeast-1",
        "ap-northeast-2",
        "me-south-1",
    ]
    .iter()
    .map(|r| OptionItem::new(r))
    .collect();
    items.push(OptionItem::new("af-south-1").disabled("Not enabled"));
    Options::new(items)
}

fn rollouts() -> Options {
    Options::new([
        OptionItem::new("off").label("Off").tone(Tone::Muted),
        OptionItem::new("canary").label("Canary"),
        OptionItem::new("half").label("50 %"),
        OptionItem::new("full").label("Full"),
    ])
}

fn owners() -> Options {
    Options::values(&[
        "Ana Lima",
        "Bea Costa",
        "Chen Wei",
        "Dara Okoye",
        "Eli Park",
        "Farah Aziz",
        "Gus Moreno",
        "Hana Sato",
        "Ivo Petrov",
        "Jonas Berg",
        "Kai Nakamura",
        "Lena Vogel",
        "Mira Okafor",
        "Omar Haddad",
        "Priya Nair",
        "Sofia Rossi",
        "Theo Grant",
    ])
}

fn labels() -> Options {
    Options::new([
        OptionItem::new("api"),
        OptionItem::new("web"),
        OptionItem::new("worker"),
        OptionItem::new("db"),
        OptionItem::new("cache"),
        OptionItem::new("edge"),
        OptionItem::new("legacy").disabled("Retired label"),
    ])
}

fn priorities() -> Options {
    Options::new([
        OptionItem::new("P1")
            .tone(Tone::Error)
            .description("Page on-call now"),
        OptionItem::new("P2")
            .tone(Tone::Warning)
            .description("Fix today"),
        OptionItem::new("P3").description("This sprint"),
        OptionItem::new("P4").disabled("Use P3 or close"),
    ])
}

fn status(s: &str) -> Cell {
    let tone = match s {
        "Down" => Tone::Error,
        "Degraded" => Tone::Warning,
        _ => Tone::Secondary,
    };
    Cell::new(s).tone(tone)
}

const KEYS: [&str; 10] = [
    "celleditors.dismiss",
    "celleditors.blur",
    "celleditors.max_rows",
    "celleditors.popup_width",
    "celleditors.cursor_style",
    "celleditors.type_ahead",
    "celleditors.affordance",
    "celleditors.flag_out_of_set",
    "celleditors.tag_separator",
    "celleditors.placement",
];

fn config(k: &[usize; 10]) -> EditorConfig {
    let [
        dismiss,
        blur,
        rows,
        width,
        cursor,
        type_ahead,
        affordance,
        flag,
        sep,
        place,
    ] = *k;
    EditorConfig {
        dismiss: [Dismiss::LIGHT, Dismiss::ESCAPE][dismiss],
        blur: [BlurAction::Cancel, BlurAction::Commit][blur],
        max_rows: [8, 4, 12][rows],
        popup_width: [(14, 48), (14, 24), (30, 60)][width],
        cursor_style: [CursorStyle::Fill, CursorStyle::Bar][cursor],
        type_ahead: type_ahead == 1,
        affordance: [Affordance::Focused, Affordance::Always, Affordance::Never][affordance],
        flag_out_of_set: flag == 1,
        tag_separator: [", ", " · "][sep],
        placement: [Placement::Below, Placement::Above][place],
        ..EditorConfig::default()
    }
}

fn states_grid() -> StateGrid {
    use SpecimenState::*;
    let id = ID.sub("states");
    let sample = |k: usize, affordance: Affordance| {
        let full = table();
        let keep = [1, ENV, ROLLOUT];
        let cols = keep.iter().map(|&c| full.columns[c].clone()).collect();
        let rows = full
            .rows
            .iter()
            .take(3)
            .map(|r| keep.iter().map(|&c| r[c].clone()).collect())
            .collect();
        let mut t = DataTable::new(id.child(k), cols, rows)
            .cell_nav(true)
            .editor_config(EditorConfig {
                affordance,
                ..EditorConfig::default()
            });
        t.cursor_col = 1;
        t
    };
    StateGrid::new(id)
        .rows(&[
            Rest, Hover, Focused, Editing, Error, Disabled, Empty, Overflow, Narrow,
        ])
        .column_width(46)
        .narrow_width(30)
        .column("▾ on focus and hover", sample(0, Affordance::Focused))
        .column("▾ always", sample(1, Affordance::Always))
        .column("▾ never", sample(2, Affordance::Never))
}

fn region_editor() -> ColumnEditor {
    ColumnEditor::Combo(regions())
}

pub fn table() -> DataTable {
    let columns = vec![
        Column::new("ID", Constraint::Length(5)),
        Column::new("Service", Constraint::Min(14)),
        Column::new("Env", Constraint::Length(13)).editor(ColumnEditor::Select(environments())),
        Column::new("Region", Constraint::Length(15)).editor(ColumnEditor::for_options(regions())),
        Column::new("Rollout", Constraint::Length(8)).editor(ColumnEditor::for_options(rollouts())),
        Column::new("Owner", Constraint::Length(13)).editor(ColumnEditor::for_options(owners())),
        Column::new("Notify", Constraint::Length(6))
            .editor(ColumnEditor::for_options(Options::values(&["no", "yes"]))),
        Column::new("Labels", Constraint::Length(14)).editor(ColumnEditor::Tags(labels())),
        Column::new("Priority", Constraint::Length(8)).editor(ColumnEditor::Radio(priorities())),
        Column::new("Status", Constraint::Length(9)),
    ];
    let data: [[&str; 10]; 8] = [
        [
            "D-101",
            "checkout-api",
            "prod",
            "us-east-1",
            "canary",
            "Mira Okafor",
            "yes",
            "api, web",
            "P1",
            "Healthy",
        ],
        [
            "D-102",
            "search",
            "stage",
            "eu-west-1",
            "full",
            "Jonas Berg",
            "no",
            "api",
            "P3",
            "Healthy",
        ],
        [
            "D-103",
            "billing-worker",
            "prod",
            "eu-central-1",
            "half",
            "Sofia Rossi",
            "yes",
            "worker, db",
            "P2",
            "Degraded",
        ],
        [
            "D-104",
            "edge-cache",
            "preview",
            "ap-southeast-1",
            "off",
            "Kai Nakamura",
            "no",
            "cache, edge",
            "P3",
            "Healthy",
        ],
        [
            "D-105",
            "auth",
            "dev",
            "us-west-2",
            "full",
            "Ana Lima",
            "yes",
            "api",
            "P2",
            "Healthy",
        ],
        [
            "D-106",
            "reports",
            "qa-legacy",
            "mars-1",
            "canary",
            "Priya Nair",
            "no",
            "batch",
            "P3",
            "Down",
        ],
        [
            "D-107",
            "notifications",
            "sandbox",
            "sa-east-1",
            "off",
            "Omar Haddad",
            "yes",
            "worker",
            "P4",
            "Healthy",
        ],
        [
            "D-108",
            "media",
            "perf",
            "ap-northeast-1",
            "half",
            "Hana Sato",
            "no",
            "web, cache",
            "P3",
            "Healthy",
        ],
    ];
    let rows = data
        .iter()
        .map(|r| {
            let mut cells: Vec<Cell> = r[..9].iter().map(|v| Cell::new(*v)).collect();
            cells[0] = cells[0].clone().tone(Tone::Muted);
            cells.push(status(r[9]));
            cells
        })
        .collect();
    let mut t = DataTable::new(ID.sub("table"), columns, rows).cell_nav(true);
    t.cursor_col = ENV;
    t
}

pub struct CellEditorsPage {
    template: Template,

    knobs: [usize; 10],
    pub table: DataTable,
    edits: u32,

    pub pick: Button,
    pub region: String,
    pub chooser: Option<ChoiceEditor>,

    last: &'static str,
    sections: crate::pages::layout::Sections,
}

impl CellEditorsPage {
    pub fn new() -> Self {
        let knobs = KEYS.map(|k| crate::options::find(k).map_or(0, |o| o.default));
        let mut table = table().editor_config(config(&knobs));
        table.cursor_col = ENV;
        Self {
            template: Template::new(ID, states_grid()),
            knobs,
            table,
            edits: 0,
            pick: Button::secondary(ID.sub("pick"), "Default region"),
            region: "eu-west-1".into(),
            chooser: None,
            last: "—",
            sections: crate::pages::layout::Sections::new(ID.sub("page")),
        }
    }

    fn open_chooser(&mut self) {
        self.chooser = ChoiceEditor::open(
            ID.sub("pick-list"),
            &region_editor(),
            &self.region,
            &self.table.editor_config,
        );
    }

    fn on_choice(&mut self, r: Option<ChoiceResult>, cx: &mut Cx) {
        let Some(r) = r else {
            return;
        };
        self.chooser = None;
        match r {
            ChoiceResult::Commit(v) => {
                self.last = "Commit";
                cx.status(format!("Default region {v}"));
                self.region = v;
            }
            ChoiceResult::CommitAndTab { value, backward } => {
                self.last = "CommitAndTab";
                cx.status(format!("Default region {value}"));
                self.region = value;
                if backward {
                    cx.focus_prev();
                } else {
                    cx.focus_next();
                }
            }
            ChoiceResult::Cancel => {
                self.last = "Cancel";
                cx.status("Region unchanged");
            }
        }
    }

    fn render_table(&mut self, card: Rect, buf: &mut Buffer, ctx: &mut RenderCtx) {
        let t = ctx.theme;
        let err = self.table.edit_error().map(str::to_owned);
        let panel =
            Panel::card(Some("Deployment targets")).focused(ctx.interaction.focused(self.table.id));
        let bg = panel.bg(t);
        let inner = panel.render(card, buf, t);
        self.table.render(inner, buf, ctx, bg);
        let flagged = self.table.out_of_set_cells().len();
        let meta = match (&err, flagged) {
            (Some(e), _) => e.clone(),
            (None, 0) => format!("{} edits", self.edits),
            (None, n) => format!(
                "{} edits · {n} value{} outside the set",
                self.edits,
                if n == 1 { "" } else { "s" }
            ),
        };
        panel.draw_meta(card, buf, t, &meta);
    }

    fn render_legend(&mut self, area: Rect, buf: &mut Buffer, ctx: &mut RenderCtx) {
        let t = ctx.theme;
        for (y, (glyph, name, what)) in (area.y..).zip(LEGEND) {
            if y >= area.bottom() {
                break;
            }
            let gs = match *name {
                "Outside the set" => t.error_fg(),
                "Read only" => t.muted(),
                _ => t.secondary(),
            };
            buf.set_string(area.x + 1, y, glyph, gs);
            buf.set_string(area.x + 6, y, name, t.primary());
            buf.set_string(
                area.x + 23,
                y,
                junie_tui::ui::text::truncate(what, area.width.saturating_sub(24) as usize),
                t.muted(),
            );
        }
    }

    fn option_sets(&self) -> Vec<Prop> {
        self.table
            .columns
            .iter()
            .filter_map(|c| {
                let opts = c.editor.options()?;
                let items: Vec<String> = opts
                    .0
                    .iter()
                    .map(|o| match (&o.disabled, &o.description) {
                        (Some(why), _) => format!("{} (off: {why})", o.label),
                        (None, Some(d)) => format!("{} ({d})", o.label),
                        _ => o.label.clone(),
                    })
                    .collect();
                Some(Prop::new(&c.title, items.join(" · ")))
            })
            .collect()
    }

    fn choice_line(&self) -> String {
        format!(
            "Enter or a click: Commit · Tab: CommitAndTab · Esc or outside: Cancel · last: {}",
            self.last
        )
    }

    fn heights(&self, width: u16) -> [u16; 4] {
        let sets: Vec<Prop> = self.option_sets().into_iter().map(Prop::wrap).collect();
        let choice = Panel::card(Some("A ChoiceEditor on its own"));
        let inner = choice.inner(ratatui::layout::Size::new(width, 0)).width;
        let lines =
            junie_tui::ui::text::wrap(&self.choice_line(), inner.max(1) as usize).len() as u16;
        [
            (self.table.len() as u16 + 5).max(8),
            LEGEND.len() as u16,
            sheet_height(&Panel::card(Some("Option sets")), &sets, width),
            choice
                .outer(ratatui::layout::Size::new(inner, 2 + lines))
                .height,
        ]
    }

    fn render_playground(&mut self, area: Rect, buf: &mut Buffer, ctx: &mut RenderCtx) {
        let t = ctx.theme;
        let sets: Vec<Prop> = self.option_sets().into_iter().map(Prop::wrap).collect();
        let heights = self.heights(area.width);
        let focus = if ctx.interaction.has_focus(self.table.id) {
            Some(0)
        } else if ctx.interaction.has_focus(self.pick.id) {
            Some(3)
        } else {
            None
        };
        let l = self
            .sections
            .layout(area, &heights.map(StackChild::fixed), focus, buf, ctx);
        match l.slot(0) {
            Some(r) => self.render_table(r, buf, ctx),
            None => ctx.focus_stop(self.table.id),
        }
        if let Some(r) = l.slot(1) {
            self.render_legend(r, buf, ctx);
        }
        if let Some(r) = l.slot(2) {
            let panel = Panel::card(Some("Option sets"));
            let bg = panel.bg(t);
            let inner = panel.render(r, buf, t);
            props::render(inner, buf, t, &sets, bg);
        }
        match l.slot(3) {
            Some(r) => {
                let panel = Panel::card(Some("A ChoiceEditor on its own"));
                let bg = panel.bg(t);
                let inner = panel.render(r, buf, t);
                self.pick.label = format!("Default region  {}  ▾", self.region);
                let w = self.pick.width().min(inner.width);
                let at = Rect::new(inner.x, inner.y, w, 1);
                self.pick.render(at, buf, ctx, bg);
                if let Some(c) = &mut self.chooser {
                    let cfg = self.table.editor_config.clone();
                    c.render(at, buf, ctx, &cfg);
                }
                let line = self.choice_line();
                for (i, l) in junie_tui::ui::text::wrap(&line, inner.width.max(1) as usize)
                    .iter()
                    .enumerate()
                {
                    crate::pages::layout::caption(inner.x, inner.y + 2 + i as u16, buf, t, l, bg);
                }
            }
            None => ctx.focus_stop(self.pick.id),
        }
    }

    fn on_event(&mut self, ev: Option<TableAction>, cx: &mut Cx) {
        match ev {
            Some(TableAction::Committed { col, .. }) => {
                self.edits += 1;
                let name = self.table.columns[col].title.clone();
                cx.status(format!("{name} saved"));
            }
            Some(TableAction::Cancelled) => cx.status("Edit cancelled"),
            Some(TableAction::LeaveForward) => cx.focus_next(),
            Some(TableAction::LeaveBackward) => cx.focus_prev(),
            _ => {}
        }
    }
}

const LEGEND: &[(&str, &str, &str)] = &[
    ("▾", "Select", "Env: 6–15 values in a dropdown"),
    (
        "/",
        "Combo",
        "Region, Owner: more than 15 values, type to filter",
    ),
    ("│", "Segmented", "Rollout: 2–5 short values on one strip"),
    ("[✓]", "Checkbox", "Notify: two values flip in place"),
    ("[✓]", "Tags", "Labels: several values from a set"),
    ("(●)", "Radio", "Priority: values that need a description"),
    (
        "!",
        "Outside the set",
        "imported values stay flagged until fixed",
    ),
    ("", "Read only", "ID, Service and the Status pill"),
];

impl CellEditorsPage {
    fn handle_chooser(&mut self, ev: &PageEvent, cx: &mut Cx) -> Option<Outcome> {
        let cfg = self.table.editor_config.clone();
        match ev {
            PageEvent::Blur(ch) if ch.from == self.pick.id && self.chooser.is_some() => {
                let value = self.chooser.as_ref().and_then(|c| c.blur_value(&cfg));
                self.chooser = None;
                if let Some(v) = value {
                    self.region = v;
                    self.last = "Commit";
                }
                Some(Outcome::Changed)
            }
            PageEvent::Key(key) if cx.focus.is(self.pick.id) => {
                if let Some(c) = &mut self.chooser {
                    let (o, r) = c.on_key(key, &cfg);
                    self.on_choice(r, cx);
                    return Some(o);
                }
                let (o, activated) = self.pick.on_key(key);
                if activated {
                    self.open_chooser();
                    return Some(Outcome::Changed);
                }
                Some(o)
            }
            PageEvent::Pointer(p) if self.chooser.as_ref().is_some_and(|c| p.on(c.id)) => {
                let c = self.chooser.as_mut()?;
                let (o, results) = c.pointer(p).into_parts();
                for r in results {
                    self.on_choice(Some(r), cx);
                }
                Some(o)
            }
            PageEvent::Pointer(p) if p.is(Phase::Click) && p.on(self.pick.id) => {
                cx.focus.focus(self.pick.id);
                if self.chooser.is_some() {
                    self.chooser = None;
                } else if self.pick.on_click() {
                    self.open_chooser();
                }
                Some(Outcome::Changed)
            }
            _ => None,
        }
    }
}

impl Page for CellEditorsPage {
    fn title(&self) -> &'static str {
        "Cell editors"
    }
    fn blurb(&self) -> &'static str {
        "Columns that only take values from a fixed set"
    }

    fn measure(&self, avail: ratatui::layout::Size, ctx: &RenderCtx) -> ratatui::layout::Size {
        use junie_tui::ui::measure::Measure;
        let sets: Vec<Prop> = self.option_sets().into_iter().map(Prop::wrap).collect();
        let legend = LEGEND
            .iter()
            .map(|(_, _, what)| 24 + junie_tui::ui::text::width(what) as u16)
            .max()
            .unwrap_or(0);
        let w = [
            self.table.min_width(ratatui::layout::Size::new(0, 0), ctx) + 4,
            legend,
            sheet_width(&Panel::card(Some("Option sets")), &sets),
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
        let cursor = (self.table.cursor_row, self.table.cursor_col);
        self.table = table().editor_config(config(&self.knobs));
        (self.table.cursor_row, self.table.cursor_col) = cursor;
        self.chooser = None;
    }

    fn template(&self) -> Option<&Template> {
        Some(&self.template)
    }

    fn template_mut(&mut self) -> Option<&mut Template> {
        Some(&mut self.template)
    }

    fn handle(&mut self, ev: &PageEvent, cx: &mut Cx) -> Outcome {
        if let Some(o) = self.template.handle(ev, cx) {
            return o;
        }
        if let Some(o) = self.handle_chooser(ev, cx) {
            return o;
        }
        if let Some(o) = self.sections.handle(ev) {
            return o;
        }
        if let PageEvent::Blur(ch) = ev {
            if ch.from != self.table.id {
                return Outcome::Ignored;
            }
            let (out, actions) = self.table.on_blur().into_parts();
            for tev in actions {
                self.on_event(Some(tev), cx);
            }
            return out;
        }
        match ev {
            PageEvent::Key(key) => {
                if !cx.focus.is(self.table.id) {
                    return Outcome::Ignored;
                }
                let (out, actions) = self.table.on_key(key).into_parts();
                for tev in actions {
                    self.on_event(Some(tev), cx);
                }
                out
            }
            PageEvent::Paste(text) => self.table.on_paste(text),
            PageEvent::Pointer(p) => {
                let Some((out, tev)) = route_table(&mut self.table, p, cx) else {
                    return Outcome::Ignored;
                };
                for tev in tev {
                    self.on_event(Some(tev), cx);
                }
                out
            }
            _ => Outcome::Ignored,
        }
    }

    fn editing(&self) -> bool {
        self.table.is_editing()
    }

    fn as_any(&self) -> Option<&dyn std::any::Any> {
        Some(self)
    }

    fn hints(&self, focus: Option<WidgetId>) -> Vec<Hint> {
        if let Some(h) = self.template.hints(focus) {
            return h;
        }
        if focus == Some(self.pick.id) {
            return if self.chooser.is_some() {
                vec![
                    ("↑↓", "Choose"),
                    ("Enter", "Apply"),
                    ("Tab", "Apply · next"),
                    ("Esc", "Cancel"),
                ]
            } else {
                vec![("Enter / Space", "Choose a region")]
            };
        }
        if self.table.choice.is_some() {
            vec![
                ("↑↓", "Choose"),
                ("Enter", "Apply"),
                ("Tab", "Apply · next"),
                ("Esc", "Cancel"),
            ]
        } else if self.table.is_editing() {
            vec![("Enter", "Apply"), ("Esc", "Cancel"), ("Tab", "Next cell")]
        } else {
            vec![
                ("↑↓←→", "Cell"),
                junie_tui::widgets::widget_hints::EDIT_PAIR,
                ("Alt+↓", "Open"),
                ("click twice", "Edit"),
            ]
        }
    }
}
