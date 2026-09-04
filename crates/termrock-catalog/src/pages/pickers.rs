// SPDX-FileCopyrightText: 2026 Alexey Zhokhov
// SPDX-License-Identifier: Apache-2.0
//
// Adapted from junie-tui src/bin/showcase/pages/pickers.rs (MIT).

//! One modal list for files, tabs and levels: search, scope, tag, alternate action.

use ratatui::buffer::Buffer;
use ratatui::layout::{Margin, Position, Rect};
use ratatui::text::Line;
use ratatui::widgets::{StatefulWidget, Widget};
use termrock::input::{KeyCode, KeyEventKind, KeyModifiers};
use termrock::widgets::{
    Backdrop, Button, ButtonState, ButtonVariant, ListRow, Picker, PickerOutcome, PickerSize,
    PickerState, fuzzy_match_label, place_picker_modal,
};

use crate::ctx::RenderCtx;
use crate::id::WidgetId;
use crate::layout;
use crate::outcome::Route;
use crate::page::{Hint, Page, PageCtx, PageEvent};
use crate::text;

const ID: WidgetId = WidgetId::of("pickers");

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Kind {
    Quick,
    Tabs,
    Level,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Scope {
    All,
    Files,
    Tasks,
}

struct Node {
    label: &'static str,
    children: &'static [Node],
}

struct Entry {
    label: String,
    detail: String,
    glyph: &'static str,
    group: &'static str,
}

fn flatten(nodes: &[Node], path: &str, out: &mut Vec<Entry>) {
    for n in nodes {
        let p = if path.is_empty() {
            n.label.to_owned()
        } else {
            format!("{path}/{}", n.label)
        };
        if n.children.is_empty() {
            out.push(Entry {
                label: n.label.to_owned(),
                detail: p.clone(),
                glyph: "F",
                group: "Files",
            });
        } else {
            flatten(n.children, &p, out);
        }
    }
}

fn project_tree() -> &'static [Node] {
    const WEBHOOKS: &[Node] = &[
        Node {
            label: "dispatch.rs",
            children: &[],
        },
        Node {
            label: "retry.rs",
            children: &[],
        },
        Node {
            label: "mod.rs",
            children: &[],
        },
    ];
    const API: &[Node] = &[
        Node {
            label: "auth.rs",
            children: &[],
        },
        Node {
            label: "billing.rs",
            children: &[],
        },
        Node {
            label: "mod.rs",
            children: &[],
        },
        Node {
            label: "webhooks",
            children: WEBHOOKS,
        },
    ];
    const DB: &[Node] = &[
        Node {
            label: "migrations.rs",
            children: &[],
        },
        Node {
            label: "pool.rs",
            children: &[],
        },
        Node {
            label: "schema.rs",
            children: &[],
        },
    ];
    const WORKERS: &[Node] = &[
        Node {
            label: "scheduler.rs",
            children: &[],
        },
        Node {
            label: "mailer.rs",
            children: &[],
        },
    ];
    const SRC: &[Node] = &[
        Node {
            label: "api",
            children: API,
        },
        Node {
            label: "db",
            children: DB,
        },
        Node {
            label: "workers",
            children: WORKERS,
        },
        Node {
            label: "config.rs",
            children: &[],
        },
        Node {
            label: "lib.rs",
            children: &[],
        },
        Node {
            label: "main.rs",
            children: &[],
        },
    ];
    const FIXTURES: &[Node] = &[
        Node {
            label: "users.json",
            children: &[],
        },
        Node {
            label: "orders.json",
            children: &[],
        },
    ];
    const TESTS: &[Node] = &[
        Node {
            label: "checkout.rs",
            children: &[],
        },
        Node {
            label: "auth_flow.rs",
            children: &[],
        },
        Node {
            label: "fixtures",
            children: FIXTURES,
        },
    ];
    const DOCS: &[Node] = &[
        Node {
            label: "architecture.md",
            children: &[],
        },
        Node {
            label: "webhooks.md",
            children: &[],
        },
    ];
    const ROOT: &[Node] = &[
        Node {
            label: "src",
            children: SRC,
        },
        Node {
            label: "tests",
            children: TESTS,
        },
        Node {
            label: "docs",
            children: DOCS,
        },
        Node {
            label: "Cargo.toml",
            children: &[],
        },
        Node {
            label: "README.md",
            children: &[],
        },
    ];
    ROOT
}

const TASKS: &[(&str, &str)] = &[
    ("Add rate limiting to auth endpoints", "mira"),
    ("Migrate sessions table to UUID keys", "jonas"),
    ("Fix flaky checkout integration test", "ana"),
    ("Write release notes for 3.2", "mira"),
    ("Replace deprecated Vue mixins", "kai"),
    ("Upgrade Postgres driver to 0.9", "jonas"),
    ("Extract billing service module", "sofia"),
    ("Add OpenTelemetry tracing spans", "kai"),
    ("Remove legacy feature flags", "ana"),
    ("Generate API client from OpenAPI", "sofia"),
    ("Harden CSP headers", "mira"),
    ("Speed up cold start of worker", "jonas"),
];

const LEVELS: [(&str, &str); 6] = [
    (
        "Silent",
        "Writes run without asking. Destructive statements still confirm.",
    ),
    ("Alert", "Every write asks for confirmation before it runs."),
    (
        "Alert (Full)",
        "Every statement, reads included, asks for confirmation.",
    ),
    (
        "Safe Mode",
        "Writes ask for confirmation and a deliberate acknowledgement.",
    ),
    (
        "Safe Mode (Full)",
        "Every statement asks for confirmation and a deliberate acknowledgement.",
    ),
    (
        "Read-Only",
        "Writes are refused. Reads and exports still work.",
    ),
];

struct OpenPicker {
    kind: Kind,
    state: PickerState<usize>,
    width: u16,
    title: &'static str,
    placeholder: &'static str,
    searchable: bool,
}

pub struct PickersPage {
    quick: ButtonState,
    tabs_btn: ButtonState,
    level_btn: ButtonState,
    picker: Option<OpenPicker>,
    scope: Scope,
    entries: Vec<Entry>,
    tabs: Vec<String>,
    level: usize,
    chosen: Option<(String, String)>,
    opened: u32,
    capture_cursor: Option<Position>,
}

impl PickersPage {
    #[must_use]
    pub fn new() -> Self {
        let mut entries = Vec::new();
        flatten(project_tree(), "", &mut entries);
        for (i, (name, owner)) in TASKS.iter().enumerate() {
            entries.push(Entry {
                label: (*name).to_owned(),
                detail: format!("#{} · {}", 1040 + i as u32, owner),
                glyph: "T",
                group: "Tasks",
            });
        }
        Self {
            quick: ButtonState::new(),
            tabs_btn: ButtonState::new(),
            level_btn: ButtonState::new(),
            picker: None,
            scope: Scope::All,
            entries,
            tabs: vec![
                "Query 1".into(),
                "orders".into(),
                "order_items".into(),
                "History".into(),
            ],
            level: 3,
            chosen: None,
            opened: 0,
            capture_cursor: None,
        }
    }

    fn open(&mut self, kind: Kind) {
        self.opened += 1;
        let (title, placeholder, width, searchable, selected) = match kind {
            // Source quick-open keeps its default 64-cell modal even on a
            // 120-cell terminal; wider screens leave the side gutters open.
            Kind::Quick => ("Open quickly", "Files and tasks…", 64, true, None),
            Kind::Tabs => ("Open tabs", "Filter tabs…", 48, true, None),
            Kind::Level => (
                "Safe Mode · this connection",
                "",
                112,
                false,
                Some(self.level),
            ),
        };
        self.picker = Some(OpenPicker {
            kind,
            state: PickerState::new(selected),
            width,
            title,
            placeholder,
            searchable,
        });
    }

    fn ranked_items(
        &self,
        kind: Kind,
        query: &str,
    ) -> Vec<(
        usize,
        String,
        String,
        &'static str,
        Option<&'static str>,
        &'static str,
    )> {
        match kind {
            Kind::Quick => {
                let mut ranked: Vec<(u32, usize)> = self
                    .entries
                    .iter()
                    .enumerate()
                    .filter(|(_, e)| match self.scope {
                        Scope::All => true,
                        Scope::Files => e.group == "Files",
                        Scope::Tasks => e.group == "Tasks",
                    })
                    .filter_map(|(i, e)| {
                        let (pen, _) = fuzzy_match_label(query, &e.label)?;
                        Some((pen + if e.group == "Files" { 0 } else { 5 }, i))
                    })
                    .collect();
                ranked.sort_by(|a, b| {
                    a.0.cmp(&b.0)
                        .then_with(|| self.entries[a.1].label.cmp(&self.entries[b.1].label))
                });
                ranked
                    .into_iter()
                    .map(|(_, i)| {
                        let e = &self.entries[i];
                        (i, e.label.clone(), e.detail.clone(), e.glyph, None, e.group)
                    })
                    .collect()
            }
            Kind::Tabs => self
                .tabs
                .iter()
                .enumerate()
                .filter_map(|(i, t)| {
                    let _ = fuzzy_match_label(query, t)?;
                    Some((
                        i,
                        t.clone(),
                        if i == 0 {
                            "query".into()
                        } else {
                            "public · data".into()
                        },
                        if i == 0 { "≡" } else { "T" },
                        if i == 1 { Some("active") } else { None },
                        "Open tabs",
                    ))
                })
                .collect(),
            Kind::Level => LEVELS
                .iter()
                .enumerate()
                .map(|(i, (l, d))| {
                    (
                        i,
                        (*l).to_owned(),
                        (*d).to_owned(),
                        " ",
                        if i == self.level {
                            Some("current")
                        } else {
                            None
                        },
                        "",
                    )
                })
                .collect(),
        }
    }

    fn scope_label(&self) -> &'static str {
        match self.scope {
            Scope::All => "All · Tab scope",
            Scope::Files => "Files · Tab scope",
            Scope::Tasks => "Tasks · Tab scope",
        }
    }
}

fn btn_width(label: &str) -> u16 {
    (text::width(label) + 2) as u16
}

fn paint_btn(
    label: &str,
    variant: ButtonVariant,
    id: WidgetId,
    area: Rect,
    buf: &mut Buffer,
    ctx: &mut RenderCtx<'_>,
    state: &mut ButtonState,
    bg: ratatui::style::Color,
    allow_focus: bool,
) {
    state.focused = allow_focus && ctx.interaction.focused(id);
    state.hovered = allow_focus && ctx.interaction.hovered(id);
    state.activation.set_accepts_input(true);
    state.activation.set_enabled(true);
    let _ = Button::new(label, ctx.system)
        .variant(variant)
        .container(bg)
        .paint(area, buf, state);
    ctx.control(id, area, false);
}

impl Page for PickersPage {
    fn title(&self) -> &'static str {
        "Pickers"
    }
    fn blurb(&self) -> &'static str {
        "One modal list for files, tabs and levels: search, scope, tag, alternate action"
    }

    fn render(&mut self, area: Rect, buf: &mut Buffer, ctx: &mut RenderCtx<'_>) {
        self.capture_cursor = None;
        let t = ctx.theme;
        let rows = layout::rows(area, &[7, 1, 0]);
        let (inner, bg) = layout::card(rows[0], buf, t, Some("Open a picker"), None, false);
        let labels = ["Open quickly", "Switch tab", "Choose a level"];
        let variants = [
            ButtonVariant::Primary,
            ButtonVariant::Secondary,
            ButtonVariant::Secondary,
        ];
        let ids = [ID.sub("quick"), ID.sub("tabs"), ID.sub("level")];
        let mut x = inner.x;
        let button_y = inner.y;
        let mut level_button_right = None;
        let allow_focus = self.picker.is_none();
        let states = [&mut self.quick, &mut self.tabs_btn, &mut self.level_btn];
        for i in 0..3 {
            let w = btn_width(labels[i]).min(inner.right().saturating_sub(x));
            let button_area = Rect::new(x, inner.y, w, 1);
            paint_btn(
                labels[i],
                variants[i],
                ids[i],
                button_area,
                buf,
                ctx,
                states[i],
                bg,
                allow_focus,
            );
            if i == 2 {
                level_button_right = Some(button_area.right());
            }
            x = x.saturating_add(w).saturating_add(2);
        }
        if inner.y + 2 < inner.bottom() {
            buf.set_string(
                inner.x,
                inner.y + 2,
                text::truncate(
                    "Quick: fuzzy over files and tasks, Tab cycles the scope, Alt+Enter is the alternate action · Tabs: Delete closes a row · Level: no search box",
                    inner.width as usize,
                ),
                t.muted().bg(bg),
            );
        }

        let (inner, _bg) = layout::card(
            Rect::new(rows[2].x, rows[2].y, rows[2].width, rows[2].height.min(8)),
            buf,
            t,
            Some("Result"),
            None,
            false,
        );
        let (label, detail) = self
            .chosen
            .clone()
            .unwrap_or_else(|| ("nothing yet".into(), "—".into()));
        let level = LEVELS[self.level].0;
        let tabs = self.tabs.join(" · ");
        let opened = self.opened.to_string();
        let pairs = [
            ("Chosen", label.as_str()),
            ("Detail", detail.as_str()),
            ("Level", level),
            ("Open tabs", tabs.as_str()),
            ("Pickers opened", opened.as_str()),
        ];
        let label_w = pairs.iter().map(|(k, _)| text::width(k)).max().unwrap_or(0) as u16 + 2;
        for (i, (k, v)) in pairs.iter().enumerate() {
            let y = inner.y + i as u16;
            if y >= inner.bottom() {
                break;
            }
            buf.set_string(inner.x, y, k, t.muted().bg(_bg));
            buf.set_string(
                inner.x + label_w,
                y,
                &text::truncate(v, inner.width.saturating_sub(label_w) as usize),
                t.primary().bg(_bg),
            );
        }

        if self.picker.is_some() {
            let (kind, title, placeholder, searchable, width, query) = {
                let p = self.picker.as_ref().expect("picker");
                (
                    p.kind,
                    p.title,
                    p.placeholder,
                    p.searchable,
                    p.width,
                    p.state.query_text().to_owned(),
                )
            };
            let ranked = self.ranked_items(kind, &query);
            let mut rows: Vec<ListRow<'_, usize>> = Vec::new();
            for (id, label, detail, glyph, tag, group) in &ranked {
                let mut row = ListRow::item(*id, Line::from(label.as_str()))
                    .leading(Line::from(*glyph))
                    .secondary(Line::from(detail.as_str()));
                if let Some(tag) = tag {
                    row = row.status(Line::from(*tag));
                }
                if !group.is_empty() {
                    row = row.badge(Line::from(*group));
                }
                rows.push(row);
            }
            self.picker.as_mut().expect("picker").state.reconcile(&rows);
            let hints = match kind {
                Kind::Quick => {
                    "↑↓ Move · Enter Open · Alt+Enter New tab · Tab Scope · Esc Clear / Close"
                }
                Kind::Tabs => "↑↓ Move · Enter Switch · Delete Close tab · Esc Close",
                Kind::Level => "↑↓ Move · Enter Set level · Esc Keep",
            };
            let screen = *buf.area();
            let query_rows = u16::from(searchable) * 2;
            let item_rows = u16::try_from(ranked.len().max(1)).unwrap_or(1).min(12);
            let height = (2 + 1 + query_rows + item_rows + 2).min(screen.height.saturating_sub(2));
            let width = width.min(screen.width.saturating_sub(4).max(1));
            let placed = place_picker_modal(screen, PickerSize { width, height });
            let scope = if kind == Kind::Quick {
                Some(self.scope_label())
            } else {
                None
            };
            let mut picker = Picker::new(&rows, ctx.system)
                .title(title)
                .placeholder(placeholder)
                .searchable(searchable)
                .hints(hints)
                .focused(true);
            if let Some(s) = scope {
                picker = picker.scope(s);
            }
            let dim = Rect::new(
                screen.x,
                screen.y,
                screen.width,
                screen.height.saturating_sub(1),
            );
            Widget::render(Backdrop::new(ctx.system), dim, buf);
            StatefulWidget::render(
                &picker,
                placed,
                buf,
                &mut self.picker.as_mut().expect("picker").state,
            );
            if searchable {
                let inner = placed.inner(Margin::new(2, 1));
                let qlen = text::width(&query) as u16;
                let cap = inner.width.saturating_sub(3);
                ctx.set_cursor(Position::new(
                    inner.x.saturating_add(2).saturating_add(qlen.min(cap)),
                    inner.y.saturating_add(1),
                ));
            } else if let Some(x) = level_button_right {
                // Native capture leaves the hidden cursor at the focused
                // level button while the fixed-choice modal is open.
                self.capture_cursor = Some(Position::new(x, button_y));
            }
            ctx.control(ID.sub("picker"), placed, false);
        }
    }

    fn handle(&mut self, ev: &PageEvent, cx: &mut PageCtx<'_>) -> Route {
        if self.picker.is_some() {
            return match ev {
                PageEvent::Key(key) if key.kind != KeyEventKind::Release => {
                    let kind = self.picker.as_ref().unwrap().kind;
                    if key.code == KeyCode::Tab && key.modifiers.is_empty() && kind == Kind::Quick {
                        self.scope = match self.scope {
                            Scope::All => Scope::Files,
                            Scope::Files => Scope::Tasks,
                            Scope::Tasks => Scope::All,
                        };
                        return Route::Changed;
                    }
                    if matches!(key.code, KeyCode::Delete | KeyCode::Backspace)
                        && kind == Kind::Tabs
                        && key.modifiers.is_empty()
                    {
                        let query = self.picker.as_ref().unwrap().state.query_text().to_owned();
                        let ranked = self.ranked_items(kind, &query);
                        if let Some(sel) = self
                            .picker
                            .as_ref()
                            .unwrap()
                            .state
                            .list()
                            .selected()
                            .copied()
                            && let Some(i) = ranked
                                .iter()
                                .find(|(id, ..)| *id == sel)
                                .map(|(id, ..)| *id)
                            && i < self.tabs.len()
                            && self.tabs.len() > 1
                        {
                            let name = self.tabs.remove(i);
                            cx.status(format!("Closed {name}"));
                            return Route::Changed;
                        }
                    }
                    let alt = key.modifiers.contains(KeyModifiers::ALT)
                        && matches!(key.code, KeyCode::Enter);
                    let query = self.picker.as_ref().unwrap().state.query_text().to_owned();
                    let ranked = self.ranked_items(kind, &query);
                    let rows: Vec<ListRow<'_, usize>> = ranked
                        .iter()
                        .map(|(id, label, ..)| ListRow::item(*id, Line::from(label.as_str())))
                        .collect();
                    let pev = self.picker.as_mut().unwrap().state.handle_key(&rows, *key);
                    match pev {
                        PickerOutcome::Cancelled => {
                            self.picker = None;
                            Route::Changed
                        }
                        PickerOutcome::QueryChanged | PickerOutcome::CursorMoved => Route::Changed,
                        PickerOutcome::Activated(i) => {
                            if kind == Kind::Level {
                                self.level = i;
                            }
                            if let Some((_, label, detail, ..)) =
                                ranked.iter().find(|(id, ..)| *id == i)
                            {
                                self.chosen = Some((label.clone(), detail.clone()));
                                cx.status(format!(
                                    "{} {}",
                                    if alt { "Opened in a new tab:" } else { "Chose" },
                                    label
                                ));
                            }
                            self.picker = None;
                            Route::Changed
                        }
                        PickerOutcome::Ignored if alt => Route::Changed,
                        PickerOutcome::Ignored => Route::Changed,
                        _ => Route::Changed,
                    }
                }
                PageEvent::Click { id, pos } => {
                    if *id != ID.sub("picker") {
                        self.picker = None;
                        return Route::Changed;
                    }
                    let kind = self.picker.as_ref().unwrap().kind;
                    let query = self.picker.as_ref().unwrap().state.query_text().to_owned();
                    let ranked = self.ranked_items(kind, &query);
                    let pev = self.picker.as_mut().unwrap().state.click(*pos);
                    if let PickerOutcome::Activated(i) = pev {
                        if kind == Kind::Level {
                            self.level = i;
                        }
                        if let Some((_, label, detail, ..)) =
                            ranked.iter().find(|(id, ..)| *id == i)
                        {
                            self.chosen = Some((label.clone(), detail.clone()));
                            cx.status(format!("Chose {label}"));
                        }
                        self.picker = None;
                    }
                    Route::Changed
                }
                PageEvent::Wheel { id, delta } if *id == ID.sub("picker") => {
                    let n = self
                        .picker
                        .as_ref()
                        .map(|p| self.ranked_items(p.kind, p.state.query_text()).len())
                        .unwrap_or(0);
                    let _ = self
                        .picker
                        .as_mut()
                        .unwrap()
                        .state
                        .scroll_by(*delta as isize, n);
                    Route::Changed
                }
                _ => Route::Ignored,
            };
        }
        match ev {
            PageEvent::Key(key)
                if key.kind != KeyEventKind::Release
                    && matches!(key.code, KeyCode::Enter | KeyCode::Char(' ')) =>
            {
                let Some(f) = *cx.focus else {
                    return Route::Ignored;
                };
                if f == ID.sub("quick") {
                    self.open(Kind::Quick);
                    return Route::Changed;
                }
                if f == ID.sub("tabs") {
                    self.open(Kind::Tabs);
                    return Route::Changed;
                }
                if f == ID.sub("level") {
                    self.open(Kind::Level);
                    return Route::Changed;
                }
                Route::Ignored
            }
            PageEvent::Click { id, .. } => {
                if *id == ID.sub("quick") {
                    self.open(Kind::Quick);
                    return Route::Changed;
                }
                if *id == ID.sub("tabs") {
                    self.open(Kind::Tabs);
                    return Route::Changed;
                }
                if *id == ID.sub("level") {
                    self.open(Kind::Level);
                    return Route::Changed;
                }
                Route::Ignored
            }
            _ => Route::Ignored,
        }
    }

    fn hints(&self, _focus: Option<WidgetId>) -> Vec<Hint> {
        if self.picker.is_some() {
            vec![("Esc", "Close")]
        } else {
            vec![("Enter", "Open"), ("Tab", "Next")]
        }
    }

    fn capture_cursor(&self) -> Option<Position> {
        self.capture_cursor
    }
}
