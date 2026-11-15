use ratatui::buffer::Buffer;
use ratatui::crossterm::event::KeyCode;
use ratatui::layout::{Constraint, Rect};

use crate::app::PageId;
use crate::options::{PageValues, page_options};
use crate::pages::row_menu::{MENU_HINTS, RowMenu};
use crate::pages::template::{Section, Template};
use crate::pages::{Cx, Hint, Page, PageEvent};
use junie_tui::core::event::{Key, Outcome};
use junie_tui::core::focus::forward_blur;
use junie_tui::core::id::WidgetId;
use junie_tui::core::intent::Phase;
use junie_tui::core::pointer::{MouseTarget, PointerEvent};
use junie_tui::theme::Tone;
use junie_tui::ui::context::ContextAnchor;
use junie_tui::ui::ctx::RenderCtx;
use junie_tui::ui::layer::LayerTag;
use junie_tui::ui::specimen::{SpecimenState, StateGrid};
use junie_tui::ui::text::fuzzy;
use junie_tui::widgets::button::Button;
use junie_tui::widgets::detail_card::DetailCard;
use junie_tui::widgets::empty::EmptyState;
use junie_tui::widgets::info_dialog::InfoDialog;
use junie_tui::widgets::input::{InputAction, TextInput};
use junie_tui::widgets::menu::{ContextMenu, MenuItem};
use junie_tui::widgets::panel::Panel;
use junie_tui::widgets::props::Prop;
use junie_tui::widgets::scrollbar;
use junie_tui::widgets::segments::{self, Segment};
use junie_tui::widgets::table::{Cell, Column, DataTable, TableAction};

const ID: WidgetId = WidgetId::of("finder");
const ALTS: WidgetId = ID.sub("alternatives-menu");
const WHY: WidgetId = ID.sub("why-dialog");
const SCOPES: &[&str] = &["Here", "Project", "Everywhere"];

const SCOPE_LINK: WidgetId = ID.sub("scope");

const CARD_MIN: u16 = 90;

struct Action {
    name: &'static str,
    group: &'static str,

    scope: usize,
    note: &'static str,
    runs: &'static str,
    risk: &'static str,
    alternatives: &'static [&'static str],

    recency: u32,
}

const ACTIONS: &[Action] = &[
    Action {
        name: "Pull",
        group: "git",
        scope: 0,
        note: "up to date",
        runs: "git pull --ff-only",
        risk: "mutating",
        alternatives: &["Fetch only", "Pull with merge", "Pull with rebase"],
        recency: 9,
    },
    Action {
        name: "Pull with rebase",
        group: "git",
        scope: 0,
        note: "rewrites local commits",
        runs: "git pull --rebase",
        risk: "mutating",
        alternatives: &["Pull", "Fetch only"],
        recency: 3,
    },
    Action {
        name: "Push",
        group: "git",
        scope: 0,
        note: "2 commits ahead",
        runs: "git push",
        risk: "remote",
        alternatives: &["Push with lease", "Push tags"],
        recency: 7,
    },
    Action {
        name: "Stash changes",
        group: "git",
        scope: 0,
        note: "3 files changed",
        runs: "git stash push",
        risk: "mutating",
        alternatives: &["Stash with message", "Stash untracked too"],
        recency: 4,
    },
    Action {
        name: "Build",
        group: "cargo",
        scope: 0,
        note: "debug · 14 crates",
        runs: "cargo build",
        risk: "safe",
        alternatives: &["Build release", "Check only"],
        recency: 8,
    },
    Action {
        name: "Test",
        group: "cargo",
        scope: 0,
        note: "nextest · 431 tests",
        runs: "cargo nextest run",
        risk: "safe",
        alternatives: &["Test this file", "Test with logs"],
        recency: 6,
    },
    Action {
        name: "Update dependencies",
        group: "cargo",
        scope: 1,
        note: "4 outdated",
        runs: "cargo update",
        risk: "mutating",
        alternatives: &["Dry run", "Update one crate"],
        recency: 2,
    },
    Action {
        name: "Compose up",
        group: "docker",
        scope: 1,
        note: "3 services",
        runs: "docker compose up -d",
        risk: "mutating",
        alternatives: &["Compose up (attached)", "Rebuild and up"],
        recency: 5,
    },
    Action {
        name: "Prune images",
        group: "docker",
        scope: 2,
        note: "4.2 GB reclaimable",
        runs: "docker image prune",
        risk: "destructive",
        alternatives: &["Prune all unused", "Show sizes"],
        recency: 1,
    },
    Action {
        name: "Open in IDEA",
        group: "files",
        scope: 1,
        note: "holla",
        runs: "idea .",
        risk: "safe",
        alternatives: &["Open in Zed", "Reveal in Finder"],
        recency: 4,
    },
    Action {
        name: "Clean target",
        group: "files",
        scope: 1,
        note: "9.8 GB",
        runs: "cargo clean",
        risk: "destructive",
        alternatives: &["Clean release only", "Show sizes"],
        recency: 1,
    },
    Action {
        name: "Upgrade Homebrew",
        group: "system",
        scope: 2,
        note: "12 formulae",
        runs: "brew upgrade",
        risk: "mutating",
        alternatives: &["List outdated", "Upgrade one"],
        recency: 2,
    },
];

fn risk_tone(risk: &str) -> Tone {
    match risk {
        "safe" => Tone::Success,
        "remote" => Tone::Info,
        "destructive" => Tone::Danger,
        _ => Tone::Warning,
    }
}

fn rank(query: &str, scope: usize) -> Vec<usize> {
    let mut hits: Vec<(u32, u32, usize)> = ACTIONS
        .iter()
        .enumerate()
        .filter(|(_, a)| a.scope <= scope)
        .filter_map(|(i, a)| fuzzy(a.name, query.trim()).map(|(p, _)| (p, a.recency, i)))
        .collect();
    hits.sort_by(|a, b| a.0.cmp(&b.0).then(b.1.cmp(&a.1)));

    let mut groups: Vec<&str> = Vec::new();
    for (_, _, i) in &hits {
        if !groups.contains(&ACTIONS[*i].group) {
            groups.push(ACTIONS[*i].group);
        }
    }
    groups
        .iter()
        .flat_map(|g| {
            hits.iter()
                .filter(|h| ACTIONS[h.2].group == *g)
                .map(|h| h.2)
        })
        .collect()
}

pub fn page() -> Box<dyn Page> {
    Box::new(FinderPage::new())
}

pub struct FinderPage {
    template: Template,
    opts: PageValues,
    pub query: TextInput,

    pub scope: usize,
    pub results: DataTable,

    shown: Vec<usize>,
    pub alternatives: Button,
    pub why: Button,
    menu: RowMenu<usize>,
}

impl FinderPage {
    pub fn new() -> Self {
        let mut page = Self {
            template: Template::new(ID, states_grid()),
            opts: PageValues::new(page_options(PageId::Finder)),
            query: TextInput::new(ID.sub("query"), "")
                .placeholder("Search actions, files and tasks")
                .plain_label(),
            scope: 0,
            results: DataTable::new(ID.sub("results"), vec![], vec![]),
            shown: Vec::new(),
            alternatives: Button::secondary(ID.sub("alternatives"), "Alternatives…"),
            why: Button::subtle(ID.sub("why"), "Why?"),
            menu: RowMenu::default(),
        };
        page.rebuild();
        page
    }

    fn rebuild(&mut self) {
        let q = match self.opts.value("finder.query") {
            "empty" => "",
            "nomatch" => "zzq",
            _ => "pu",
        };
        self.query = TextInput::new(ID.sub("query"), "")
            .placeholder("Search actions, files and tasks")
            .plain_label()
            .readout_link(SCOPE_LINK)
            .value(q);
        let scope = match self.opts.value("finder.scope") {
            "project" => 1,
            "everywhere" => 2,
            _ => 0,
        };
        self.set_scope(scope);
    }

    pub fn set_scope(&mut self, scope: usize) {
        self.scope = scope.min(SCOPES.len() - 1);
        self.query
            .set_readout(&format!("scope ‹ {} ›", SCOPES[self.scope].to_lowercase()));
        self.rerank();
    }

    fn columns(&self) -> Vec<Column> {
        let mut cols = vec![
            Column::new("Action", Constraint::Min(18)),
            Column::new("Group", Constraint::Length(8)),
            Column::new("Note", Constraint::Min(14)),
        ];
        if self.opts.on("finder.risk_column") {
            cols.push(Column::new("Risk", Constraint::Length(12)));
        }
        cols
    }

    fn rerank(&mut self) {
        self.shown = rank(self.query.text(), self.scope);
        let risk = self.opts.on("finder.risk_column");
        let rows = self
            .shown
            .iter()
            .map(|&i| {
                let a = &ACTIONS[i];
                let mut row = vec![
                    Cell::new(a.name),
                    Cell::new(a.group).tone(Tone::Secondary),
                    Cell::new(a.note).tone(Tone::Muted),
                ];
                if risk {
                    row.push(Cell::new(a.risk).tone(risk_tone(a.risk)));
                }
                row
            })
            .collect();
        let total = ACTIONS.iter().filter(|a| a.scope <= self.scope).count();
        self.results = DataTable::new(ID.sub("results"), self.columns(), rows).empty(
            EmptyState::new("No matches").hint(&format!(
                "Nothing among {total} actions here matches; try Everywhere"
            )),
        );
        self.results.cursor_row = 0;
    }

    fn current(&self) -> Option<&'static Action> {
        if self.shown.is_empty() {
            return None;
        }
        let src = self.results.source_row(self.results.cursor_row);
        self.shown.get(src).map(|&i| &ACTIONS[i])
    }

    fn run(&mut self, what: &str, cx: &mut Cx) {
        cx.status(format!("✓ Ran {what}"));
    }

    fn open_alternatives(&mut self, anchor: ContextAnchor) {
        let Some(a) = self.current() else {
            return;
        };
        let items = a.alternatives.iter().map(|s| MenuItem::new(*s)).collect();
        let menu = anchor
            .apply(ContextMenu::new(ALTS, items))
            .title(format!("{} · alternatives", a.name));
        self.menu.open(menu, self.results.cursor_row);
    }

    fn button_anchor(&self) -> ContextAnchor {
        ContextAnchor::Control {
            id: self.alternatives.id,
            fallback: self.alternatives.area,
        }
    }

    fn open_why(&self, cx: &mut Cx) {
        let Some(a) = self.current() else {
            return;
        };
        let q = self.query.text().trim().to_owned();
        let how = match fuzzy(a.name, &q) {
            _ if q.is_empty() => "no query: recent first".to_owned(),
            Some((0, _)) => format!("\"{q}\" starts the name"),
            Some((p, _)) if p < 100 => format!("\"{q}\" is in the name"),
            Some(_) => format!("the letters of \"{q}\" appear in order"),
            None => "no match".to_owned(),
        };
        let d = InfoDialog::new(
            WHY,
            &format!("Why {}?", a.name),
            vec![
                Prop::new("Match", how),
                Prop::new("Recency", format!("used {} times this week", a.recency)),
                Prop::new("Scope", SCOPES[a.scope]),
                Prop::new("Risk", a.risk).tone(risk_tone(a.risk)),
            ],
        )
        .intro(vec![(
            "Ranked by match, then by how recently you ran it.".into(),
            Tone::Muted,
        )])
        .width(56);
        cx.open(d, LayerTag::new("why"));
    }

    fn card(&self, area: Rect) -> bool {
        self.opts.value("finder.preview") != "line" && area.width >= CARD_MIN
    }

    fn render_playground(&mut self, area: Rect, buf: &mut Buffer, ctx: &mut RenderCtx) {
        let t = ctx.theme;

        self.query
            .render(Rect::new(area.x, area.y, area.width, 2), buf, ctx, t.canvas);
        let top = 3;
        let body = Rect::new(
            area.x,
            area.y + top,
            area.width,
            area.height.saturating_sub(top),
        );
        if self.card(area) {
            let preview_w = (body.width * 2 / 5).max(34);
            let list = Rect::new(
                body.x,
                body.y,
                body.width.saturating_sub(preview_w + 2),
                body.height,
            );
            let prev = Rect::new(list.right() + 2, body.y, preview_w, body.height);
            self.render_results(list, buf, ctx);
            self.render_preview(prev, buf, ctx);
        } else {
            let list = Rect::new(body.x, body.y, body.width, body.height.saturating_sub(2));
            self.render_results(list, buf, ctx);
            self.render_line(
                Rect::new(body.x, body.bottom().saturating_sub(1), body.width, 1),
                buf,
                ctx,
            );
        }
        self.menu.render(area, buf, ctx);
    }

    fn render_results(&mut self, r: Rect, buf: &mut Buffer, ctx: &mut RenderCtx) {
        let t = ctx.theme;
        let count = format!("{}", self.shown.len());
        let panel = Panel::card(Some("Results"))
            .meta(&count)
            .focused(ctx.interaction.focused(self.results.id));
        let bg = panel.bg(t);
        if let Some(inner) = panel.try_render(r, buf, t) {
            self.results.render(inner, buf, ctx, bg);
        }
    }

    fn render_preview(&mut self, r: Rect, buf: &mut Buffer, ctx: &mut RenderCtx) {
        let t = ctx.theme;
        let Some(a) = self.current() else {
            let panel = Panel::card(Some("Preview"));
            let bg = panel.bg(t);
            if let Some(inner) = panel.try_render(r, buf, t) {
                DetailCard::empty(EmptyState::new("Nothing selected")).render(
                    inner,
                    buf,
                    ctx,
                    bg,
                    &mut [],
                );
            }
            return;
        };
        let meta = format!("{} · {}", a.group, SCOPES[a.scope].to_lowercase());
        let facts = vec![
            Prop::new("Runs", a.runs).copyable(),
            Prop::new("In", "~/work/holla"),
            Prop::new("Host", "mbp · local"),
            Prop::new("State", a.note),
            Prop::new("Risk", a.risk).tone(risk_tone(a.risk)),
            Prop::new(
                "Gate",
                if a.risk == "destructive" {
                    "asks first"
                } else {
                    "runs directly"
                },
            ),
            Prop::new("Data", "live · 2 s ago").tone(Tone::Muted),
        ];

        DetailCard::new(a.name)
            .meta(&meta)
            .props(facts)
            .fit(true)
            .reserve(2)
            .render(
                r,
                buf,
                ctx,
                t.canvas,
                &mut [&mut self.alternatives, &mut self.why],
            );
    }

    fn render_line(&mut self, r: Rect, buf: &mut Buffer, ctx: &mut RenderCtx) {
        let Some(a) = self.current() else {
            return;
        };
        let left = [
            Segment::new(a.runs, Tone::Secondary).lead("→", Tone::Muted),
            Segment::new(SCOPES[a.scope].to_lowercase(), Tone::Muted).priority(2),
            Segment::new(a.risk, risk_tone(a.risk)).priority(3),
        ];
        segments::render(r, buf, ctx, &left, &[], ctx.theme.canvas);
    }

    fn pointer(&mut self, ev: &PointerEvent, cx: &mut Cx) -> Outcome {
        let click = ev.is(Phase::Click);
        if let Some(r) = self.query.route(ev) {
            if click {
                cx.focus.focus(self.query.id);
            }
            return r.outcome();
        }

        if ev.id == SCOPE_LINK {
            if click {
                self.set_scope((self.scope + 1) % SCOPES.len());
                return Outcome::Changed;
            }
            return Outcome::Consumed;
        }
        if ev.is(Phase::Secondary)
            && let Some(req) = self.menu.config.on_secondary(&mut self.results, ev)
        {
            cx.focus.focus(self.results.id);
            self.open_alternatives(req.anchor);
            return Outcome::Changed;
        }
        if let Some(r) = self.results.route(ev) {
            if click && !scrollbar::is_bar_of(ev.id, self.results.id) {
                cx.focus.focus(self.results.id);
            }
            let (o, a) = r.into_parts();
            if let Some(TableAction::Activated(_)) = a
                && let Some(act) = self.current()
            {
                self.run(act.name, cx);
            }
            return o;
        }
        for (b, alt) in [(self.alternatives.id, true), (self.why.id, false)] {
            let button = if alt {
                &mut self.alternatives
            } else {
                &mut self.why
            };
            if let Some(r) = button.route(ev) {
                if r.into_action().is_some() {
                    cx.focus.focus(b);
                    if alt {
                        self.open_alternatives(self.button_anchor());
                    } else {
                        self.open_why(cx);
                    }
                }
                return Outcome::Changed;
            }
        }
        Outcome::Ignored
    }

    fn key(&mut self, key: &Key, cx: &mut Cx) -> Outcome {
        let Some(f) = cx.focus.current() else {
            return Outcome::Ignored;
        };
        if f == self.query.id {
            if key.alt() {
                match key.code {
                    KeyCode::Up => {
                        self.set_scope(self.scope + 1);
                        return Outcome::Changed;
                    }
                    KeyCode::Down => {
                        self.set_scope(self.scope.saturating_sub(1));
                        return Outcome::Changed;
                    }
                    _ => {}
                }
            }
            let (o, a) = self.query.on_key(key);
            match a {
                Some(InputAction::Changed) => self.rerank(),
                Some(InputAction::CommittedTab { backward: false }) => cx.focus_next(),
                Some(InputAction::CommittedTab { backward: true }) => cx.focus_prev(),
                _ => {}
            }
            return o;
        }
        if f == self.results.id {
            if let Some(req) = self.menu.config.on_key(&self.results, key) {
                self.open_alternatives(req.anchor);
                return Outcome::Changed;
            }
            let (o, a) = self.results.on_key(key);
            if let Some(TableAction::Activated(_)) = a
                && let Some(act) = self.current()
            {
                self.run(act.name, cx);
            }
            return o;
        }
        for alt in [true, false] {
            let button = if alt {
                &mut self.alternatives
            } else {
                &mut self.why
            };
            if button.id == f {
                let (o, act) = button.on_key(key);
                if act {
                    if alt {
                        self.open_alternatives(self.button_anchor());
                    } else {
                        self.open_why(cx);
                    }
                }
                return o;
            }
        }
        Outcome::Ignored
    }
}

fn states_grid() -> StateGrid {
    use SpecimenState::*;
    let id = ID.sub("states");
    let rows = ACTIONS[..3]
        .iter()
        .map(|a| vec![Cell::new(a.name), Cell::new(a.group).tone(Tone::Secondary)])
        .collect();
    let table = DataTable::new(
        id.child(1),
        vec![
            Column::new("Action", Constraint::Min(12)),
            Column::new("Group", Constraint::Length(6)),
        ],
        rows,
    )
    .empty(EmptyState::new("No matches"));
    StateGrid::new(id)
        .rows(&[Rest, Hover, Focused, Selected, Empty, Overflow, Narrow])
        .column_width(30)
        .column(
            "Scope",
            TextInput::new(id.child(0), "")
                .placeholder("Search actions")
                .plain_label()
                .readout("scope ‹ here ›")
                .readout_link(id.child(2)),
        )
        .column("Results", table)
}

impl Page for FinderPage {
    fn title(&self) -> &'static str {
        "Finder"
    }
    fn blurb(&self) -> &'static str {
        "Typing-hot query, scope, ranked results, preview, alternatives and why"
    }

    fn render(&mut self, area: Rect, buf: &mut Buffer, ctx: &mut RenderCtx) {
        let body = self.template.render(area, buf, ctx);
        match self.template.section() {
            Section::Playground => self.render_playground(body, buf, ctx),
            Section::Options => self.template.render_options(body, buf, ctx),
            Section::States => self.template.render_states(body, buf, ctx),
        }
    }

    fn handle(&mut self, ev: &PageEvent, cx: &mut Cx) -> Outcome {
        if let Some(o) = self.template.handle(ev, cx) {
            return o;
        }
        if self.template.section() != Section::Playground {
            return Outcome::Ignored;
        }
        if let Some((out, chosen)) = self.menu.handle(ev) {
            if let Some((_, i)) = chosen
                && let Some(alt) = self.current().and_then(|a| a.alternatives.get(i))
            {
                self.run(alt, cx);
            }
            return out;
        }
        match ev {
            PageEvent::Blur(ch) => forward_blur(*ch, &mut [&mut self.query]),
            PageEvent::Key(key) => self.key(key, cx),
            PageEvent::Paste(text) if cx.focus.is(self.query.id) => {
                let o = self.query.on_paste(text);
                self.rerank();
                o
            }
            PageEvent::Pointer(p) => self.pointer(p, cx),
            _ => Outcome::Ignored,
        }
    }

    fn layer_open(&self) -> bool {
        self.menu.hides_page_focus()
    }

    fn editing(&self) -> bool {
        self.query.editing
    }

    fn hints(&self, focus: Option<WidgetId>) -> Vec<Hint> {
        if let Some(h) = self.template.hints(focus) {
            return h;
        }
        if self.menu.is_open() {
            return MENU_HINTS.to_vec();
        }
        match focus {
            Some(f) if f == self.results.id => {
                vec![
                    ("↑↓", "Move"),
                    ("Enter", "Run"),
                    crate::pages::row_menu::OPEN_HINT,
                ]
            }
            _ => vec![("Type", "Search"), ("Alt+↑↓", "Scope"), ("Tab", "Results")],
        }
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

#[cfg(all(test, feature = "work-metrics"))]
pub(crate) const ACTION_COUNT: usize = ACTIONS.len();
