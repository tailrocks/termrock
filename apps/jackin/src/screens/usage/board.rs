use crate::backend::{PlainKey, Ticket};
use crate::screens::modals::FieldKindW;
use crate::screens::modals::{FieldValue, FormDialog, FormField, FormValues};
use crate::screens::{Cx, LayerTag, ModalResult};
use crate::usage_connections::{
    BoardEdit, BoardMetric, OrderingScope, UsageAction, UsageBoard, UsageUpdate,
};
use crate::world::World;
use junie_tui::core::{event::Outcome, id::WidgetId};
use junie_tui::theme::Tone;
use junie_tui::widgets::input::TextInput;
use junie_tui::widgets::list::{ListBox, ListItem, SelectMode};
use junie_tui::widgets::{button::Button, choice::Checkbox, select::Select};

const ID: WidgetId = WidgetId::of("usage.connected_board");

#[derive(Default)]
pub(super) struct Board {
    ticket: Option<Ticket>,
    next_tick: i64,
    managing_account: bool,
    pub(super) account: super::super::accounts::connections::Connections,
    priority: bool,
    data: Option<UsageBoard>,
    view: View,
    row: usize,
    undo: Option<BoardEdit>,
}
#[derive(Default)]
enum View {
    #[default]
    Loading,
    Rows,
    Details(usize),
    Member(usize, usize),
    Settings,
    Proxy,
    Error(String),
}

fn choose(values: &FormValues, name: &str) -> usize {
    values
        .iter()
        .find_map(|(key, v)| {
            if key == name
                && let FieldValue::Choice(index) = v
            {
                Some(*index)
            } else {
                None
            }
        })
        .unwrap_or(0)
}

fn note(lines: Vec<String>) -> FormField {
    FormField::note(
        "text",
        lines.into_iter().map(|s| (s, Tone::Muted)).collect(),
    )
    .wrap(true)
}

impl Board {
    pub(super) fn open(&mut self, w: &mut World, cx: &mut Cx, priority: Option<bool>) {
        self.next_tick = w.now_ms().saturating_add(30_000);
        self.managing_account = false;
        self.priority = priority.unwrap_or(true);
        self.ticket = Some(w.next_ticket());
        self.data = None;
        self.row = 0;
        self.view = View::Loading;
        cx.open(self.form(), LayerTag::new("usage_board"));
        self.request(
            w,
            if priority.is_none() {
                BoardEdit::DefaultTab
            } else {
                BoardEdit::Read
            },
        );
    }

    pub(super) fn tick(&mut self, w: &World) {
        if w.now_ms() >= self.next_tick {
            self.next_tick = w.now_ms().saturating_add(30_000);
            self.refreshed(w);
        }
    }

    pub(super) fn account_closed(&mut self, w: &World) {
        self.managing_account = false;
        self.refreshed(w);
    }

    pub(super) fn refreshed(&self, w: &World) {
        if !self.managing_account && matches!(self.view, View::Rows) && self.ticket.is_some() {
            self.request(w, BoardEdit::Read);
        }
    }

    fn request(&self, w: &World, edit: BoardEdit) {
        if let (Some(ticket), Some(svc)) = (self.ticket, w.backend.usage_connections()) {
            svc.request(
                ticket,
                UsageAction::Board {
                    priority: self.priority,
                    edit,
                },
            );
        }
    }

    fn redraw(&self, cx: &mut Cx, preserve: bool) {
        let mut form = self.form();
        if !preserve {
            cx.focus.focus(form.initial_focus());
        }
        cx.with_top::<FormDialog>(move |current| {
            if current.id == ID {
                if preserve {
                    form.scroll = current.scroll.clone();
                    for field in &mut form.fields {
                        if let FieldKindW::List { list, .. } = &mut field.kind
                            && let Some(old) =
                                current.fields.iter().find(|old| old.name == field.name)
                            && let FieldKindW::List { list: old, .. } = &old.kind
                        {
                            let label = old.items.get(old.cursor).map(|item| &item.label);
                            list.cursor = label
                                .and_then(|label| {
                                    list.items.iter().position(|item| &item.label == label)
                                })
                                .unwrap_or(old.cursor.min(list.items.len().saturating_sub(1)));
                            list.core.scroll = old.core.scroll.clone();
                        }
                    }
                }
                *current = form;
            }
        });
    }

    fn form(&self) -> FormDialog {
        let mut actions = vec![];
        let mut fields = vec![];
        if matches!(self.view, View::Loading | View::Error(_)) {
            actions.push((
                "switch",
                if self.priority {
                    "All Usage"
                } else {
                    "Priority"
                },
            ));
        }
        let mut save = "Details";
        let title = match self.view {
            View::Settings => "Usage settings",
            View::Proxy => "Network proxy",
            _ if self.priority => "Priority",
            _ => "All Usage",
        };
        match (&self.data, &self.view) {
            (_, View::Error(e)) => {
                fields.push(note(vec![e.clone()]));
                save = "Retry";
            }
            (_, View::Loading) | (None, _) => {
                fields.push(note(vec!["Loading usage…".into()]));
                save = "Loading…";
            }
            (Some(data), View::Rows) => {
                let remaining = data
                    .settings
                    .flags
                    .iter()
                    .find(|(id, _, _)| *id == "remaining")
                    .is_none_or(|(_, _, enabled)| *enabled);
                let mut lines = vec![];
                if data.rows.iter().any(|row| {
                    row.value.contains('%')
                        && !["remaining", "used", "left"]
                            .iter()
                            .any(|word| row.value.contains(word))
                }) {
                    lines.push((
                        if remaining {
                            "Percentages show remaining limits"
                        } else {
                            "Percentages show used limits"
                        }
                        .into(),
                        Tone::Secondary,
                    ));
                }
                if data
                    .rows
                    .iter()
                    .any(|row| row.bar.is_some_and(|(_, pace)| pace.is_some()))
                {
                    lines.push(("│ expected pace".into(), Tone::Muted));
                }
                if !matches!(data.summary.as_str(), "Priority" | "All Usage") {
                    lines.push((data.summary.clone(), Tone::Normal));
                }
                lines.extend(data.notices.iter().cloned().map(|text| (text, Tone::Muted)));
                fields.push(FormField::note("text", lines).wrap(true));
                if !data.rows.is_empty() {
                    let items = data
                        .rows
                        .iter()
                        .map(|row| {
                            let mut value = row.value.clone();
                            if let Some((amount, pace)) = row.bar {
                                let bar: String = (0..5)
                                    .map(|index| {
                                        if pace.map(|pace| (pace / 20).min(4)) == Some(index) {
                                            '│'
                                        } else if index * 20 < amount {
                                            '█'
                                        } else {
                                            '░'
                                        }
                                    })
                                    .collect();
                                value.push_str(&format!(" {bar}"));
                            }
                            ListItem::new(&format!("{} · {}", row.group, row.title))
                                .meta(&value)
                                .meta_tone(row.severity)
                        })
                        .collect();
                    let mut list = ListBox::new(ID.sub("row"), items, SelectMode::Single);
                    list.cursor = self.row.min(data.rows.len().saturating_sub(1));
                    fields.push(FormField::list(
                        "row",
                        list,
                        data.rows.len().clamp(3, 32) as u16,
                    ));
                }
                if data.empty {
                    actions.push(("add", "Add account…"));
                }
                if !data.attention.is_empty() {
                    let labels: Vec<_> = data.attention.iter().map(|a| a.label.as_str()).collect();
                    fields.push(FormField::select(
                        "attention",
                        Select::new(ID.sub("attention"), "Needs attention", &labels, 0),
                    ));
                    actions.push(("fix", "Fix…"));
                }
                if self.priority && data.rows.is_empty() {
                    save = "All Usage";
                } else {
                    actions.push((
                        "switch",
                        if self.priority {
                            "All Usage"
                        } else {
                            "Priority"
                        },
                    ));
                }
                actions.extend([
                    ("settings", "Settings…"),
                    ("reload", "Refresh all"),
                    ("share", "Copy report"),
                ]);
                if self.undo.is_some() {
                    actions.push((
                        "undo",
                        if matches!(self.undo, Some(BoardEdit::Unified { .. })) {
                            "Undo change"
                        } else {
                            "Undo hide"
                        },
                    ));
                }
            }
            (Some(data), View::Details(i)) => {
                if let Some(row) = data.rows.get(*i) {
                    let mut lines = vec![row.group.clone(), row.title.clone(), row.value.clone()];
                    if let Some(reset) = &row.reset {
                        lines.push(reset.clone());
                    }
                    if let Some((amount, pace)) = row.bar {
                        let bar: String = (0..10)
                            .map(|index| {
                                if pace.map(|pace| (pace / 10).min(9)) == Some(index) {
                                    '│'
                                } else if index * 10 < amount {
                                    '█'
                                } else {
                                    '░'
                                }
                            })
                            .collect();
                        lines.push(if pace.is_some() {
                            format!("{bar} · │ expected pace")
                        } else {
                            bar
                        });
                    }
                    lines.extend(row.details.clone());
                    fields.push(note(lines));
                    save = "Back";
                    actions.push(("refresh_provider", "Refresh provider"));
                    if row.account.is_none() && !row.members.is_empty() {
                        let labels: Vec<_> = row
                            .members
                            .iter()
                            .map(|member| member.label.as_str())
                            .collect();
                        fields.push(FormField::select(
                            "member",
                            Select::new(ID.sub("member_choice"), "Account", &labels, 0),
                        ));
                        actions.push(("member", "Inspect account"));
                    }
                    if row.account.is_none() && !row.member_preferences.is_empty() {
                        actions.extend([
                            ("pin", if row.pinned { "Unpin" } else { "Pin" }),
                            ("hide", "Hide"),
                        ]);
                    }
                    if row.account.is_some() {
                        actions.extend([
                            ("pin", if row.pinned { "Unpin" } else { "Pin" }),
                            ("hide", "Hide"),
                            ("up", "Move up"),
                            ("down", "Move down"),
                        ]);
                        fields.push(FormField::select(
                            "order",
                            Select::new(
                                ID.sub("order"),
                                "Reorder",
                                &["Metric", "Account", "Provider"],
                                0,
                            ),
                        ));
                    }
                }
            }
            (Some(data), View::Member(row, member)) => {
                save = "Back";
                if let Some(entry) = data.rows.get(*row).and_then(|row| row.members.get(*member)) {
                    let mut lines = vec![entry.label.clone()];
                    lines.extend(entry.details.clone());
                    fields.push(note(lines));
                }
            }
            (Some(data), View::Proxy) => {
                save = "Set proxy";
                fields.push(note(vec![if data.settings.proxy {
                    "Proxy enabled".into()
                } else {
                    "Direct connection".into()
                }]));
                fields.push(FormField::input(
                    "proxy_url",
                    TextInput::new(ID.sub("proxy_url"), "Proxy URL")
                        .masked()
                        .required(true),
                ));
                if data.settings.proxy {
                    actions.push(("clear_proxy", "Disable proxy"));
                }
                actions.push(("settings", "Back"));
            }
            (Some(data), View::Settings) => {
                save = "Save settings";
                for (id, label, checked) in &data.settings.flags {
                    fields.push(FormField::check(
                        id,
                        Checkbox::new(ID.sub(id), label, *checked),
                    ));
                }
                let warnings: Vec<_> = (20..=70).step_by(5).map(|n| format!("{n}%")).collect();
                let critical: Vec<_> = (5..=25).step_by(5).map(|n| format!("{n}%")).collect();
                for (key, label, values, index) in [
                    (
                        "warning",
                        "Warning remaining",
                        warnings,
                        (data.settings.warning.saturating_sub(20)) / 5,
                    ),
                    (
                        "critical",
                        "Critical remaining",
                        critical,
                        (data.settings.critical.saturating_sub(5)) / 5,
                    ),
                ] {
                    fields.push(FormField::select(
                        key,
                        Select::new(
                            ID.sub(key),
                            label,
                            &values.iter().map(String::as_str).collect::<Vec<_>>(),
                            usize::from(index),
                        ),
                    ));
                }
                if !data.settings.hidden.is_empty() {
                    let hidden: Vec<_> = data
                        .settings
                        .hidden
                        .iter()
                        .map(|(_, _, label)| label.as_str())
                        .collect();
                    fields.push(FormField::select(
                        "hidden_metric",
                        Select::new(ID.sub("hidden_metric"), "Hidden metric", &hidden, 0),
                    ));
                    actions.push(("unhide", "Show metric"));
                }
                actions.push(("proxy", "Network proxy…"));
                actions.push(("back", "Back"));
            }
        }
        let mut form = FormDialog::new(ID, title, fields)
            .width(86)
            .keep_open_on_save();
        form.save.label = save.into();
        form.save.disabled = matches!(&self.view, View::Loading)
            || matches!((&self.view,&self.data),(View::Rows,Some(d)) if d.rows.is_empty() && !self.priority);
        for (key, label) in actions {
            form = form.action(key, Button::secondary(ID.sub(key), label));
        }
        form
    }

    pub(super) fn receive(&mut self, ticket: Ticket, update: &UsageUpdate, cx: &mut Cx) -> Outcome {
        if self.ticket != Some(ticket) || !matches!(self.view, View::Loading | View::Rows) {
            return Outcome::Ignored;
        }
        let preserve = matches!(self.view, View::Rows);
        match update {
            UsageUpdate::Board(data) => {
                self.priority = data.priority;
                for alert in &data.alerts {
                    cx.status(alert.clone());
                }
                self.data = Some(data.as_ref().clone());
                self.view = View::Rows;
            }
            UsageUpdate::Failed(error) => self.view = View::Error(error.clone()),
            _ => return Outcome::Ignored,
        }
        if !self.managing_account {
            self.redraw(cx, preserve);
        }
        Outcome::Changed
    }

    pub(super) fn result(&mut self, result: ModalResult, w: &mut World, cx: &mut Cx) -> Outcome {
        let (key, values) = match result {
            ModalResult::FormAction(k, v) => (k, v),
            ModalResult::Form(None) | ModalResult::Cancelled => {
                self.ticket = None;
                return Outcome::Changed;
            }
            _ => return Outcome::Ignored,
        };
        let row = if let (Some(data), View::Details(i)) = (&self.data, &self.view) {
            data.rows.get(*i)
        } else {
            None
        };
        let action = match (key.as_str(), row) {
            ("add", _) => {
                self.managing_account = true;
                self.account.add(w, cx, None);
                return Outcome::Changed;
            }
            ("fix", _) => {
                if let Some(a) = self
                    .data
                    .as_ref()
                    .and_then(|d| d.attention.get(choose(&values, "attention")))
                {
                    self.managing_account = true;
                    self.account.show(a.clone(), w, cx);
                }
                return Outcome::Changed;
            }
            ("share", _) => {
                if let Some(data) = &self.data {
                    cx.copy_as(data.share.clone(), "redacted usage report");
                }
                return Outcome::Changed;
            }
            ("proxy", _) => {
                self.view = View::Proxy;
                None
            }
            ("clear_proxy", _) => Some(BoardEdit::Proxy(None)),
            ("unhide", _) => self
                .data
                .as_ref()
                .and_then(|data| data.settings.hidden.get(choose(&values, "hidden_metric")))
                .map(|(account, metric, _)| BoardEdit::Hide {
                    account: account.clone(),
                    metric: metric.clone(),
                    hidden: false,
                    restore_pin: false,
                }),
            ("settings", _) => {
                self.view = View::Settings;
                None
            }
            ("back", _) => {
                self.view = View::Rows;
                None
            }
            ("switch", _) => {
                self.priority = !self.priority;
                self.row = 0;
                self.ticket = Some(w.next_ticket());
                self.data = None;
                self.view = View::Loading;
                Some(BoardEdit::Read)
            }
            ("reload", _) => Some(BoardEdit::Refresh(None)),
            ("refresh_provider", Some(row)) => Some(BoardEdit::Refresh(Some(row.provider.clone()))),
            ("member", _) => {
                if let View::Details(i) = self.view {
                    self.view = View::Member(i, choose(&values, "member"));
                }
                None
            }
            ("undo", _) => self.undo.take(),
            (key @ ("pin" | "hide"), Some(row)) if row.account.is_none() => {
                row.unified_edit(key == "hide").map(|(edit, undo)| {
                    self.undo = Some(undo);
                    edit
                })
            }
            (
                "pin",
                Some(BoardMetric {
                    account: Some(a),
                    metric,
                    ..
                }),
            ) => Some(BoardEdit::Pin {
                account: a.clone(),
                metric: metric.clone(),
            }),
            (
                "hide",
                Some(BoardMetric {
                    account: Some(a),
                    metric,
                    pinned,
                    ..
                }),
            ) => {
                self.undo = Some(BoardEdit::Hide {
                    account: a.clone(),
                    metric: metric.clone(),
                    hidden: false,
                    restore_pin: *pinned,
                });
                Some(BoardEdit::Hide {
                    account: a.clone(),
                    metric: metric.clone(),
                    hidden: true,
                    restore_pin: false,
                })
            }
            (
                "up" | "down",
                Some(BoardMetric {
                    account: Some(a),
                    metric,
                    ..
                }),
            ) => Some(BoardEdit::Move {
                account: a.clone(),
                metric: metric.clone(),
                up: key == "up",
                scope: match choose(&values, "order") {
                    1 => OrderingScope::Account,
                    2 => OrderingScope::Provider,
                    _ => OrderingScope::Metric,
                },
            }),
            ("save", _) => match &self.view {
                View::Proxy => values.iter().find_map(|(key, v)| {
                    if key == "proxy_url"
                        && let FieldValue::Text(value) = v
                    {
                        PlainKey::new(value)
                            .ok()
                            .map(|url| BoardEdit::Proxy(Some(url)))
                    } else {
                        None
                    }
                }),
                View::Rows
                    if self.priority
                        && self.data.as_ref().is_some_and(|data| data.rows.is_empty()) =>
                {
                    self.priority = false;
                    self.ticket = Some(w.next_ticket());
                    self.data = None;
                    self.view = View::Loading;
                    Some(BoardEdit::Read)
                }
                View::Rows => {
                    self.row = choose(&values, "row");
                    self.view = View::Details(self.row);
                    None
                }
                View::Member(i, _) => {
                    self.view = View::Details(*i);
                    None
                }
                View::Details(_) => {
                    self.view = View::Rows;
                    None
                }
                View::Settings => {
                    let Some(data) = &self.data else {
                        return Outcome::Ignored;
                    };
                    let mut settings = data.settings.clone();
                    for (id, _, enabled) in &mut settings.flags {
                        if let Some(FieldValue::Bool(b)) =
                            values.iter().find(|(key, _)| key == id).map(|(_, v)| v)
                        {
                            *enabled = *b;
                        }
                    }
                    settings.warning = 20 + 5 * (choose(&values, "warning") as u8);
                    settings.critical = 5 + 5 * (choose(&values, "critical") as u8);
                    Some(BoardEdit::Settings(settings))
                }
                _ => Some(BoardEdit::Read),
            },
            _ => None,
        };
        if let Some(edit) = action {
            self.ticket = Some(w.next_ticket());
            self.view = View::Loading;
            self.request(w, edit);
        }
        self.redraw(cx, false);
        Outcome::Changed
    }
}
