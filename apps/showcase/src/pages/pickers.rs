use ratatui::buffer::Buffer;
use ratatui::layout::{Rect, Size};

use crate::app::PageId;
use crate::data::GRAPHEME_SAMPLES;
use crate::options::{PageValues, page_options};
use crate::pages::layout::{sheet_height, sheet_width};
use crate::pages::template::{Section, Template};
use crate::pages::{Cx, Hint, Page, PageEvent, Report};
use junie_tui::core::event::{Key, Outcome};
use junie_tui::core::id::{ItemKey, WidgetId};
use junie_tui::core::intent::Phase;
use junie_tui::core::keymap::NavKeys;
use junie_tui::core::pointer::MouseTarget;
use junie_tui::theme::CursorStyle;
use junie_tui::ui::ctx::{Backdrop, RenderCtx};
use junie_tui::ui::layer::LayerTag;
use junie_tui::ui::layers::LayerResult;
use junie_tui::ui::popup::{Dismiss, Place, Placement, VAlign};
use junie_tui::ui::specimen::{SpecimenState, StateGrid};
use junie_tui::ui::text::{self, fuzzy};
use junie_tui::widgets::button::{Button, row_layout};
use junie_tui::widgets::empty::EmptyState;
use junie_tui::widgets::option_list::Item;
use junie_tui::widgets::panel::Panel;
use junie_tui::widgets::picker::{
    Picker, PickerAction, PickerKeyProfile, PickerStatus, VisibleKey,
};
use junie_tui::widgets::props::{self, Prop};
use junie_tui::widgets::tree::TreeNode;
use ratatui::crossterm::event::KeyCode;

const ID: WidgetId = WidgetId::of("pickers");
const MULTI_ROWS: [&str; 4] = ["Row1", "Row2", "Row3", "Row4"];
const MULTI_SELECTION_GROUP: ItemKey = ItemKey::text("multi-provider");

fn multi_row_label(key: ItemKey) -> &'static str {
    MULTI_ROWS
        .iter()
        .copied()
        .find(|label| ItemKey::text(label) == key)
        .unwrap_or("Unknown row")
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Kind {
    Quick,
    Tabs,
    Level,
    Commands,
    Remote,
    Multi,
    MultiSearch,
}

impl Kind {
    const ALL: [Kind; 7] = [
        Kind::Quick,
        Kind::Tabs,
        Kind::Level,
        Kind::Commands,
        Kind::Remote,
        Kind::Multi,
        Kind::MultiSearch,
    ];
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Scope {
    All,
    Files,
    Tasks,
}

#[derive(Clone)]
struct Entry {
    label: String,
    detail: String,
    glyph: &'static str,
    group: &'static str,
}

fn flatten(nodes: &[TreeNode], path: &str, out: &mut Vec<Entry>) {
    for n in nodes {
        let p = if path.is_empty() {
            n.label.clone()
        } else {
            format!("{path}/{}", n.label)
        };
        if n.children.is_empty() {
            out.push(Entry {
                label: n.label.clone(),
                detail: p.clone(),
                glyph: "F",
                group: "Files",
            });
        } else {
            flatten(&n.children, &p, out);
        }
    }
}

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

const COMMANDS: &[(&str, &str, &str, Option<&str>)] = &[
    ("Run query", "▶", "run.query", None),
    ("Explain query", "?", "run.explain", None),
    (
        "Export results…",
        "⇩",
        "results.export",
        Some("Run a query first"),
    ),
    ("Format SQL", "≡", "edit.format", None),
    (
        "Commit transaction",
        "✓",
        "tx.commit",
        Some("No open transaction"),
    ),
    ("Reconnect", "↻", "conn.reconnect", None),
];

const BRANCHES: &[&str] = &[
    "main",
    "feat/rate-limit",
    "fix/session-expiry",
    "chore/uuid-sessions",
    "release/4.2",
];

const FETCH_TICKS: u32 = 3;

/// The most rows Alt+Down grows an open picker to.
const LIVE_ROWS_MAX: u16 = 24;

/// Rows the multi pickers open with checked when "Preselect" is on: Row1 and
/// Row3, by position.
const PRESELECTED: [usize; 2] = [0, 2];

fn international_entries() -> Vec<Entry> {
    GRAPHEME_SAMPLES
        .iter()
        .enumerate()
        .map(|(i, (kind, sample))| {
            if i < 3 {
                Entry {
                    label: format!("{sample}.md"),
                    detail: format!("docs/{sample}.md · {kind}"),
                    glyph: "F",
                    group: "Files",
                }
            } else {
                Entry {
                    label: format!("Review {sample}"),
                    detail: format!("#{} · {kind}", 201 + i),
                    glyph: "T",
                    group: "Tasks",
                }
            }
        })
        .collect()
}

pub struct PickersPage {
    template: Template,
    opts: PageValues,
    pub buttons: Vec<Button>,

    open: Option<Kind>,
    scope: Scope,
    entries: Vec<Entry>,

    tabs: Vec<(String, Option<u32>)>,
    level: usize,
    chosen: Option<(String, String)>,
    pub last: String,
    opened: u32,

    fetching: Option<u32>,
    /// The row limit of the open picker, which Alt+Up and Alt+Down change live.
    live_rows: u16,
}

/// Checks the rows of `PRESELECTED` the way application code restores a saved
/// selection. A conflict or a disabled row is skipped, as it is for a click.
fn preselect(p: &mut Picker) {
    for row in PRESELECTED {
        p.toggle_checked(row);
    }
}

struct Rows {
    entries: Vec<Entry>,
    scope: Scope,
    tabs: Vec<(String, Option<u32>)>,
    level: usize,
    fetching: Option<u32>,
    remote_fails: bool,
}

impl Rows {
    fn fill(&self, kind: Kind, p: &mut Picker) {
        let q = p.query().to_owned();
        match kind {
            Kind::Quick => {
                let mut ranked: Vec<(u32, Item)> = self
                    .entries
                    .iter()
                    .filter(|e| match self.scope {
                        Scope::All => true,
                        Scope::Files => e.group == "Files",
                        Scope::Tasks => e.group == "Tasks",
                    })
                    .filter_map(|e| {
                        let (pen, matched) = fuzzy(&e.label, &q)?;
                        let mut it = Item::new(&e.label)
                            .detail(&e.detail)
                            .glyph(e.glyph)
                            .group(e.group);
                        it.matched = matched;
                        Some((pen + if e.group == "Files" { 0 } else { 5 }, it))
                    })
                    .collect();
                ranked.sort_by(|a, b| a.0.cmp(&b.0).then_with(|| a.1.label.cmp(&b.1.label)));
                p.scope = Some(format!(
                    "{} · Tab scope",
                    match self.scope {
                        Scope::All => "All",
                        Scope::Files => "Files",
                        Scope::Tasks => "Tasks",
                    }
                ));
                p.set_items(ranked.into_iter().map(|r| r.1).collect())
                    .unwrap_or_else(|error| p.report_item_error(error));
            }
            Kind::Tabs => {
                let items = self
                    .tabs
                    .iter()
                    .enumerate()
                    .filter_map(|(i, (t, run))| {
                        let (_, matched) = fuzzy(t, &q)?;
                        let mut it = Item::new(t)
                            .detail(match run {
                                Some(s) => format!("{s}s"),
                                None if i == 0 => "query".into(),
                                None => "public · data".into(),
                            })
                            .glyph(if run.is_some() { "◌" } else { "T" })
                            .group("Open tabs")
                            .key(ItemKey::text(t));
                        it.matched = matched;
                        it.tag = (i == 1).then_some("active".into());
                        Some(it)
                    })
                    .collect();
                let live = self.tabs.iter().filter(|t| t.1.is_some()).count();
                p.scope = Some(format!("{live} live"));

                p.replace_items(items)
                    .unwrap_or_else(|error| p.report_item_error(error));
            }
            Kind::Level => {
                p.set_items(
                    LEVELS
                        .iter()
                        .enumerate()
                        .map(|(i, (l, d))| {
                            let mut it = Item::new(*l).detail(*d);
                            it.tag = (i == self.level).then_some("current".into());
                            it
                        })
                        .collect(),
                )
                .unwrap_or_else(|error| p.report_item_error(error));
                p.set_cursor(self.level);
            }
            Kind::Commands => p
                .set_items(
                    COMMANDS
                        .iter()
                        .filter(|c| fuzzy(c.0, &q).is_some())
                        .map(|(l, g, k, why)| {
                            let it = Item::new(*l)
                                .glyph(*g)
                                .group("Commands")
                                .key(ItemKey::text(k))
                                .detail(why.unwrap_or(k));
                            it.disabled(why.is_some())
                        })
                        .collect(),
                )
                .unwrap_or_else(|error| p.report_item_error(error)),
            Kind::Multi | Kind::MultiSearch => {
                let items: Vec<Item> = MULTI_ROWS
                    .iter()
                    .enumerate()
                    .map(|(i, l)| {
                        let mut it = Item::new(*l)
                            .key(ItemKey::text(l))
                            .detail(format!("multi row {}", i + 1));
                        if matches!(i, 2 | 3) {
                            it = it.selection_group(MULTI_SELECTION_GROUP);
                        }
                        if i == 1 {
                            it = it.reason("Unavailable row");
                        }
                        it
                    })
                    .collect();
                if kind == Kind::MultiSearch {
                    p.replace_items(items.clone())
                        .unwrap_or_else(|error| p.report_item_error(error));
                    p.set_visible_keys(
                        items
                            .iter()
                            .filter(|item| fuzzy(&item.label, &q).is_some())
                            .filter_map(|item| item.key)
                            .map(|key| VisibleKey::new(key).matched(Vec::new()))
                            .collect(),
                    )
                    .unwrap_or_else(|error| p.report_item_error(error));
                } else {
                    p.set_items(items)
                        .unwrap_or_else(|error| p.report_item_error(error));
                }
            }
            Kind::Remote => match self.fetching {
                Some(_) => {
                    p.set_items(vec![])
                        .unwrap_or_else(|error| p.report_item_error(error));
                    p.status = PickerStatus::Loading("Fetching branches from origin…".into());
                }
                None if self.remote_fails => {
                    p.set_items(vec![])
                        .unwrap_or_else(|error| p.report_item_error(error));
                    p.status = PickerStatus::Error {
                        message: "Could not reach origin".into(),
                        detail: Some("r retries".into()),
                    };
                }
                None => {
                    p.status = PickerStatus::Ready;
                    p.set_items(
                        BRANCHES
                            .iter()
                            .map(|b| Item::new(*b).glyph("⎇").group("origin"))
                            .collect(),
                    )
                    .unwrap_or_else(|error| p.report_item_error(error));
                }
            },
        }
    }
}

impl PickersPage {
    pub fn new() -> Self {
        let mut entries = Vec::new();
        flatten(&crate::data::project_tree(), "", &mut entries);
        for t in crate::data::tasks().into_iter().take(12) {
            entries.push(Entry {
                label: t.name,
                detail: format!("#{} · {}", t.id, t.owner),
                glyph: "T",
                group: "Tasks",
            });
        }
        Self {
            template: Template::new(ID, states_grid()),
            opts: PageValues::new(page_options(PageId::Pickers)),
            buttons: vec![
                Button::primary(ID.sub("quick"), "Open quickly"),
                Button::secondary(ID.sub("tabs"), "Switch tab"),
                Button::secondary(ID.sub("level"), "Choose a level"),
                Button::secondary(ID.sub("commands"), "Run command"),
                Button::secondary(ID.sub("remote"), "Remote branch"),
                Button::secondary(ID.sub("multi"), "Multi select"),
                Button::secondary(ID.sub("multisearch"), "Multi search"),
            ],
            open: None,
            scope: Scope::All,
            entries,
            tabs: vec![
                ("Query 1".into(), Some(0)),
                ("orders".into(), None),
                ("order_items".into(), Some(4)),
                ("History".into(), None),
            ],
            level: 3,
            chosen: None,
            last: "none yet · QueryChanged · Chosen(i) · ChosenAlt(i) · Submit · Secondary(i) · NextScope · PrevScope · Back · Cancelled".into(),
            opened: 0,
            fetching: None,
            live_rows: 12,
        }
    }

    fn kind(&self) -> Option<Kind> {
        self.open
    }

    fn refill(&self, kind: Kind, cx: &mut Cx) {
        let rows = self.rows();
        cx.with_top::<Picker>(move |p| rows.fill(kind, p));
    }

    fn rows(&self) -> Rows {
        Rows {
            entries: if self.opts.on("pickers.international") {
                international_entries()
            } else {
                self.entries.clone()
            },
            scope: self.scope,
            tabs: self.tabs.clone(),
            level: self.level,
            fetching: self.fetching,
            remote_fails: self.opts.value("pickers.remote") == "fails",
        }
    }

    fn style(&self, p: Picker, anchor: Rect) -> Picker {
        let o = &self.opts;
        let mut p = p
            .key_profile(match o.value("pickers.nav") {
                "vim" => PickerKeyProfile::Vim,
                _ => PickerKeyProfile::Desktop,
            })
            .max_rows(o.value("pickers.max_rows").parse().unwrap_or(12))
            .fit(match o.value("pickers.fit") {
                "third" => Place::DEFAULT.v_align(VAlign::Third),
                "flush" => Place::FLUSH,
                _ => Place::DEFAULT,
            })
            .backdrop(match o.value("pickers.backdrop") {
                "full" => Backdrop::FULL,
                _ => Backdrop::DEFAULT,
            })
            .cursor_style(match o.value("pickers.cursor_style") {
                "bar" => CursorStyle::Bar,
                _ => CursorStyle::Fill,
            })
            .delete_is_secondary(o.on("pickers.delete_is_secondary"))
            .nav(match o.value("pickers.nav") {
                "vim" => NavKeys::VIM,
                _ => NavKeys::default(),
            })
            .dismiss(match o.value("pickers.dismiss") {
                "escape" => Dismiss::ESCAPE,
                _ => Dismiss::LIGHT,
            });
        match o.value("pickers.width") {
            "48" => p = p.width(48),
            "96" => p = p.width(96),
            _ => {}
        }
        p = if o.value("pickers.placement") == "below" {
            p.anchor(anchor, Placement::Below)
        } else {
            p.placement(Placement::Center)
        };
        p = if o.value("pickers.empty") == "state" {
            p.empty(EmptyState::new("Nothing matches").hint("Esc clears the search"))
        } else {
            p.empty_text("No matches — Esc clears the search")
        };
        if o.value("pickers.placeholder") == "custom" {
            p = p.placeholder("Type to narrow…");
        }
        if o.value("pickers.hints") == "short" {
            p = p.hints("Enter Choose · Esc Close");
        }
        p
    }

    fn open(&mut self, kind: Kind, cx: &mut Cx) {
        let i = Kind::ALL.iter().position(|k| *k == kind).unwrap_or(0);
        let anchor = self.buttons.get(i).map_or(Rect::ZERO, |b| b.area);
        let mut p = match kind {
            Kind::Quick => Picker::new(ID.sub("picker.quick"), "Open quickly")
                .placeholder("Files and tasks…")
                .width(80),
            Kind::Tabs => Picker::new(ID.sub("picker.tabs"), "Open tabs")
                .placeholder("Filter tabs…")
                .width(48),
            Kind::Level => {
                let mut p =
                    Picker::new(ID.sub("picker.level"), "Safe Mode · this connection").width(112);
                p.set_searchable(false)
                    .expect("disabling picker search is valid");
                p
            }
            Kind::Commands => Picker::new(ID.sub("picker.commands"), "Run command")
                .placeholder("Commands…")
                .width(64),
            Kind::Remote => {
                let mut p =
                    Picker::new(ID.sub("picker.remote"), "Remote branches · origin").width(56);
                p.set_searchable(false)
                    .expect("disabling picker search is valid");
                p
            }
            Kind::Multi => {
                let mut p = Picker::new(ID.sub("picker.multi"), "Multi select")
                    .width(56)
                    .multi_select(true)
                    .expect("empty picker accepts multi-selection");
                p.set_searchable(false)
                    .expect("disabling picker search is valid");
                p
            }
            Kind::MultiSearch => Picker::new(ID.sub("picker.multisearch"), "Multi search")
                .width(56)
                .multi_select(true)
                .expect("empty picker accepts multi-selection"),
        };
        p = p.hints(match kind {
            Kind::Quick => {
                "↑↓ Move · Enter Open · Alt+Enter New tab · Tab Scope · Esc Clear / Close"
            }
            Kind::Tabs => "↑↓ Move · Enter Switch · Delete Close tab · Esc Close",
            Kind::Level => "↑↓ Move · Enter Set level · Esc Keep",
            Kind::Commands => "↑↓ Move · Enter Run · Esc Close",
            Kind::Remote => "↑↓ Move · Enter Check out · r Retry · Esc Close",
            Kind::Multi => "Enter Confirm · Esc Close",
            Kind::MultiSearch => "Type Filter · Enter Confirm · Esc Close",
        });
        let mut p = self.style(p, anchor);
        self.live_rows = p.list().max_rows;
        self.opened += 1;
        if kind == Kind::Remote {
            self.fetching = Some(FETCH_TICKS);
        }
        self.rows().fill(kind, &mut p);
        if self.opts.on("pickers.preselect") && matches!(kind, Kind::Multi | Kind::MultiSearch) {
            preselect(&mut p);
        }
        cx.open(p, LayerTag::new("picker").n(i));
        self.open = Some(kind);
    }

    fn on_report(&mut self, kind: Kind, r: Report, cx: &mut Cx) {
        if r.closed.is_some() {
            self.open = None;
            if kind == Kind::Remote {
                self.fetching = None;
            }
        }
        let Some(LayerResult::Picker(ev)) = r.result else {
            return;
        };
        let picker = r.closed.as_ref().and_then(|e| e.get::<Picker>()).cloned();
        let conflict_feedback = match &ev {
            PickerAction::SelectionConflict {
                attempted,
                selected,
            } => Some(format!(
                "{} conflicts with selected {}",
                multi_row_label(*attempted),
                multi_row_label(*selected)
            )),
            _ => None,
        };
        self.last = match &ev {
            PickerAction::ChosenMany(keys) => {
                let positions: Vec<usize> = keys
                    .iter()
                    .filter_map(|key| {
                        let item = picker.as_ref()?.item_for_key(*key)?;
                        MULTI_ROWS
                            .iter()
                            .position(|label| *label == item.label.as_str())
                    })
                    .collect();
                format!("ChosenMany({positions:?})")
            }
            PickerAction::SelectionConflict { .. } => conflict_feedback
                .clone()
                .unwrap_or_else(|| "Selection conflict".into()),
            _ => format!("{ev:?}"),
        };
        if let Some(message) = conflict_feedback {
            cx.status(message);
        }
        match ev {
            PickerAction::Toggled => {}
            PickerAction::SelectionConflict { .. } => {}
            PickerAction::QueryChanged => self.refill(kind, cx),
            PickerAction::Chosen(i) | PickerAction::ChosenAlt(i) => {
                let alt = matches!(ev, PickerAction::ChosenAlt(_));
                if let Some(it) = picker
                    .as_ref()
                    .and_then(|p: &Picker| p.visible_item(i).map(|v| v.item))
                {
                    if kind == Kind::Level {
                        self.level = i;
                    }
                    self.chosen = Some((it.label.clone(), it.detail.clone()));
                    cx.status(format!(
                        "{} {}",
                        if alt { "Opened in a new tab:" } else { "Chose" },
                        it.label
                    ));
                }
            }
            PickerAction::ChosenMany(keys) => {
                let labels: Vec<String> = keys
                    .iter()
                    .map(|key| {
                        picker
                            .as_ref()
                            .and_then(|picker| picker.item_for_key(*key))
                            .map(|item| item.label.clone())
                            .unwrap_or_else(|| "Unavailable selection".into())
                    })
                    .collect();
                self.chosen = Some((labels.join(", "), format!("{} selected", keys.len())));
                cx.status(format!("ChoseMany: {}", labels.join(", ")));
            }
            PickerAction::Secondary(i) => {
                if kind == Kind::Tabs && i < self.tabs.len() && self.tabs.len() > 1 {
                    let (name, _) = self.tabs.remove(i);
                    cx.status(format!("Closed {name}"));
                    self.refill(kind, cx);
                }
            }

            PickerAction::Submit => {}
            PickerAction::NextScope | PickerAction::PrevScope => {
                if kind == Kind::Quick {
                    let back = ev == PickerAction::PrevScope;
                    self.scope = match (self.scope, back) {
                        (Scope::All, false) | (Scope::Tasks, true) => Scope::Files,
                        (Scope::Files, false) | (Scope::All, true) => Scope::Tasks,
                        (Scope::Tasks, false) | (Scope::Files, true) => Scope::All,
                    };
                    self.refill(kind, cx);
                }
            }
            PickerAction::Cancelled | PickerAction::Back => {
                if kind == Kind::Remote {
                    self.fetching = None;
                }
            }
        }
    }

    fn tick(&mut self, cx: &mut Cx) -> bool {
        let Some(kind) = self.kind() else {
            return false;
        };
        match kind {
            Kind::Tabs => {
                for t in &mut self.tabs {
                    if let Some(s) = &mut t.1 {
                        *s += 1;
                    }
                }
                self.refill(kind, cx);
                true
            }
            Kind::Remote if self.opts.value("pickers.remote") == "stalls" => false,
            Kind::Remote => match self.fetching {
                Some(n) if n > 1 => {
                    self.fetching = Some(n - 1);
                    false
                }
                Some(_) => {
                    self.fetching = None;
                    self.refill(kind, cx);
                    true
                }
                None => false,
            },
            _ => false,
        }
    }

    fn readout(&self) -> Vec<Prop> {
        let p: Picker = self.style(Picker::new(ID.sub("picker.sample"), "Sample"), Rect::ZERO);
        let list = p.list();
        let on = |b: bool| if b { "on" } else { "off" };

        let commands = COMMANDS
            .iter()
            .enumerate()
            .filter(|(i, c)| *i == 0 || c.3.is_some())
            .map(|(_, (l, g, k, why))| match why {
                Some(w) => format!("{g} {l} ({k}, disabled: {w})"),
                None => format!("{g} {l} ({k})"),
            })
            .collect::<Vec<_>>()
            .join(" · ");
        vec![
            Prop::new(
                "Size",
                format!(
                    "width {} · max rows {} · {:?}{}",
                    list.width.1,
                    list.max_rows,
                    list.placement,
                    if list.placement == Placement::Center {
                        ""
                    } else {
                        " the button"
                    }
                ),
            ),
            Prop::new(
                "Fit",
                format!(
                    "margin {} · {:?} · backdrop keeps {} bottom rows",
                    list.fit.margin, list.fit.v_align, p.backdrop.keep_bottom
                ),
            ),
            Prop::new(
                "Rows",
                format!(
                    "cursor {:?} · Delete secondary {} · nav {}",
                    list.look,
                    on(p.delete_is_secondary),
                    self.opts.value("pickers.nav")
                ),
            ),
            Prop::new(
                "Close",
                format!(
                    "Esc {} · outside click {}",
                    on(list.dismiss.escape),
                    on(list.dismiss.outside_click)
                ),
            ),
            Prop::new(
                "Text",
                format!(
                    "placeholder “{}” · no matches: {} · hints “{}”",
                    p.placeholder,
                    match &p.empty {
                        Some(e) => format!("state “{}”", e.title),
                        None => format!("“{}”", p.empty_text),
                    },
                    if p.hints.is_empty() {
                        "per picker"
                    } else {
                        p.hints.as_str()
                    }
                ),
            ),
            Prop::new("Commands", commands),
            Prop::new(
                "Remote",
                match self.opts.value("pickers.remote") {
                    "fails" => "Loading, then Error (r retries)",
                    "stalls" => "Loading, and it stays Loading",
                    _ => "Loading, then Ready",
                },
            ),
        ]
    }

    fn button_lines(&self, room: u16) -> Vec<Vec<usize>> {
        let widths: Vec<u16> = self.buttons.iter().map(|b| b.width()).collect();
        let mut lines: Vec<Vec<usize>> = vec![vec![]];
        let mut used = 0u16;
        for (i, w) in widths.iter().enumerate() {
            let need = if used == 0 { *w } else { used + 2 + w };
            if need > room && used > 0 {
                lines.push(vec![]);
                used = *w;
            } else {
                used = need;
            }
            lines.last_mut().expect("a line").push(i);
        }
        lines
    }

    fn result_facts(&self) -> Vec<Prop> {
        let (label, detail) = self
            .chosen
            .clone()
            .unwrap_or_else(|| ("nothing yet".into(), "—".into()));
        vec![
            Prop::new("Chosen", label).wrap(),
            Prop::new("Detail", detail).wrap(),
            Prop::new("Level", LEVELS[self.level].0),
            Prop::new(
                "Open tabs",
                self.tabs
                    .iter()
                    .map(|t| t.0.as_str())
                    .collect::<Vec<_>>()
                    .join(" · "),
            )
            .wrap(),
            Prop::new("Last action", self.last.clone()).wrap(),
            Prop::new("Pickers opened", self.opened.to_string()),
        ]
    }

    fn applied_facts(&self) -> Vec<Prop> {
        self.readout().into_iter().map(Prop::wrap).collect()
    }

    fn heights(&self, width: u16) -> [u16; 3] {
        let room = width.saturating_sub(4);
        let n = self.button_lines(room).len() as u16;
        let note = text::wrap(QUICK_NOTE, room.max(1) as usize).len() as u16;
        let applied = Panel::card(Some("Applied")).meta(APPLIED_META);
        let result = Panel::card(Some("Result")).meta("PickerAction");
        [
            n + 1 + note + 3,
            sheet_height(&applied, &self.applied_facts(), width),
            sheet_height(&result, &self.result_facts(), width),
        ]
    }

    fn render_playground(&mut self, area: Rect, buf: &mut Buffer, ctx: &mut RenderCtx) {
        let t = ctx.theme;

        let widths: Vec<u16> = self.buttons.iter().map(|b| b.width()).collect();
        let lines = self.button_lines(area.width.saturating_sub(4));
        let n = lines.len() as u16;
        let [top, applied_h, _] = self.heights(area.width);
        let rows = crate::pages::layout::rows(area, &[top, 1, applied_h, 1, 0]);
        let panel = Panel::card(Some("Open a picker")).meta(OPEN_META);
        let bg = panel.bg(t);
        let inner = panel.render(rows[0], buf, t);
        for (r, line) in lines.iter().enumerate() {
            let y = inner.y + r as u16;
            let ws: Vec<u16> = line.iter().map(|&i| widths[i]).collect();
            let rects = row_layout(Rect::new(inner.x, y, inner.width, 1), &ws, 2);
            for (&i, rect) in line.iter().zip(rects) {
                self.buttons[i].render(rect, buf, ctx, bg);
            }
        }
        for (i, l) in text::wrap(QUICK_NOTE, inner.width.max(1) as usize)
            .iter()
            .enumerate()
        {
            buf.set_string(inner.x, inner.y + n + 1 + i as u16, l, t.muted().bg(bg));
        }

        let panel = Panel::card(Some("Applied")).meta(APPLIED_META);
        let bg = panel.bg(t);
        let inner = panel.render(rows[2], buf, t);
        props::render(inner, buf, t, &self.applied_facts(), bg);

        let panel = Panel::card(Some("Result")).meta("PickerAction");
        let bg = panel.bg(t);
        let inner = panel.render(rows[4], buf, t);
        props::render(inner, buf, t, &self.result_facts(), bg);
    }
}

const QUICK_NOTE: &str = "Quick: fuzzy as you type, Tab scope, Alt+Enter alternate · Tabs: live, Delete closes · Level: no search box · Commands: disabled rows say why · Remote: loads, r retries";
const OPEN_META: &str = "Enter · click · Alt+↑↓ rows in an open picker";
const APPLIED_META: &str = "read back from a built picker";

fn states_grid() -> StateGrid {
    use SpecimenState::*;
    let id = ID.sub("states");
    let mut p = Picker::new(id.child(0), "Remote branches").width(44);
    p.set_searchable(false)
        .expect("disabling picker search is valid");
    p.set_items(
        ["main", "feat/rate-limit", "release/4.2"]
            .iter()
            .map(|b| Item::new(*b).glyph("⎇"))
            .collect(),
    )
    .unwrap_or_else(|error| p.report_item_error(error));
    StateGrid::new(id)
        .rows(&[Rest, Hover, Disabled, Empty, Loading, Error, Overflow])
        .column_width(50)
        .column("Branches", p)
}

impl Page for PickersPage {
    fn title(&self) -> &'static str {
        "Pickers"
    }
    fn blurb(&self) -> &'static str {
        "One modal list for files, tabs, levels, commands and branches: search, scope, status"
    }

    fn measure(&self, avail: Size, _ctx: &RenderCtx) -> Size {
        let applied = Panel::card(Some("Applied")).meta(APPLIED_META);
        let result = Panel::card(Some("Result")).meta("PickerAction");
        let widest = self.buttons.iter().map(|b| b.width()).max().unwrap_or(0) + 4;
        let w = avail
            .width
            .max(widest)
            .max(sheet_width(&applied, &self.applied_facts()))
            .max(sheet_width(&result, &self.result_facts()));
        let [a, b, c] = self.heights(w);
        Size::new(w, a + 1 + b + 1 + c)
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
        if let PageEvent::Tick = ev {
            return if self.tick(cx) {
                Outcome::Changed
            } else {
                Outcome::Ignored
            };
        }

        if self.open.is_some() {
            return Outcome::Ignored;
        }
        if let Some(o) = self.template.handle(ev, cx) {
            return o;
        }
        if self.template.section() != Section::Playground {
            return Outcome::Ignored;
        }
        match ev {
            PageEvent::Key(key) => {
                let Some(f) = cx.focus.current() else {
                    return Outcome::Ignored;
                };
                let Some(i) = self.buttons.iter().position(|b| b.id == f) else {
                    return Outcome::Ignored;
                };
                let (o, act) = self.buttons[i].on_key(key);
                if act {
                    self.open(Kind::ALL[i], cx);
                }
                o
            }
            PageEvent::Pointer(p) => {
                let hit = (self.buttons.iter_mut().enumerate())
                    .find_map(|(i, b)| b.route(p).map(|r| (i, r)));
                let Some((i, r)) = hit else {
                    return Outcome::Ignored;
                };
                if p.is(Phase::Click) {
                    cx.focus.focus(p.id);
                }
                if r.has_actions() {
                    self.open(Kind::ALL[i], cx);
                    return Outcome::Changed;
                }
                r.outcome()
            }
            _ => Outcome::Ignored,
        }
    }

    fn hints(&self, focus: Option<WidgetId>) -> Vec<Hint> {
        if self.open.is_some() {
            return vec![("Esc", "Close")];
        }
        if let Some(h) = self.template.hints(focus) {
            return h;
        }
        vec![("Enter", "Open"), ("Tab", "Next")]
    }

    fn animating(&self) -> bool {
        matches!(self.kind(), Some(Kind::Tabs)) || self.fetching.is_some()
    }

    fn on_layer_key(&mut self, key: &Key, cx: &mut Cx) -> Outcome {
        if self.open.is_some() && key.alt() && matches!(key.code, KeyCode::Up | KeyCode::Down) {
            let rows = if key.code == KeyCode::Down {
                (self.live_rows + 1).min(LIVE_ROWS_MAX)
            } else {
                self.live_rows.saturating_sub(1).max(1)
            };
            self.live_rows = rows;
            cx.with_top::<Picker>(move |p: &mut Picker| p.set_max_rows(rows));
            self.last = format!("set_max_rows({rows})");
            cx.status(format!("Max rows {rows}"));
            return Outcome::Changed;
        }
        let failed = self.fetching.is_none() && self.opts.value("pickers.remote") == "fails";
        if !(key.is_char('r') && self.kind() == Some(Kind::Remote) && failed) {
            return Outcome::Ignored;
        }
        self.fetching = Some(FETCH_TICKS);
        self.refill(Kind::Remote, cx);
        self.last = "retry".into();
        Outcome::Changed
    }

    fn on_layer(&mut self, tag: &LayerTag, report: Report, cx: &mut Cx) -> Outcome {
        self.on_report(Kind::ALL[tag.n], report, cx);
        Outcome::Changed
    }

    fn option_value(&self, key: &str) -> Option<usize> {
        self.opts.get(key)
    }

    fn set_option(&mut self, key: &str, value: usize) {
        self.opts.set(key, value);
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
