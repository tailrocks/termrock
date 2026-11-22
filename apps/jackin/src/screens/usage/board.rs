use crate::backend::{PlainKey, Ticket};
use crate::screens::modals::FieldKindW;
use crate::screens::modals::{FieldValue, FormDialog, FormField, FormValues};
use crate::screens::{Cx, LayerTag, ModalResult};
use crate::usage_connections::{
    BoardEdit, BoardMetric, OrderingScope, UsageAction, UsageBoard, UsageUpdate,
};
use crate::world::World;
use termrock::core::{event::Outcome, id::WidgetId};
use termrock::theme::Tone;
use termrock::widgets::input::TextInput;
use termrock::widgets::list::{ListBox, ListItem, SelectMode};
use termrock::widgets::{button::Button, choice::Checkbox, select::Select};

const ID: WidgetId = WidgetId::of("usage.connected_board");

#[derive(Default)]
pub(super) struct Board {
    ticket: Option<Ticket>,
    next_tick: i64,
    managing_account: bool,
    pub(super) account: super::super::accounts::connections::Connections,
    priority: bool,
    refresh_error: Option<String>,
    data: Option<UsageBoard>,
    view: View,
    row: usize,
    undo: Option<BoardEdit>,
}
#[derive(PartialEq, Eq)]
enum RowKey {
    Metric(String, Option<String>, String),
    Id(String),
}

#[derive(Default)]
enum View {
    #[default]
    Loading,
    Rows,
    Providers(usize),
    Provider(usize),
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
        self.refresh_error = None;
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

    pub(super) fn tick(&mut self, w: &mut World) {
        if w.now_ms() >= self.next_tick {
            self.next_tick = w.now_ms().saturating_add(30_000);
            self.refreshed(w);
        }
    }

    pub(super) fn account_closed(&mut self, w: &mut World) {
        self.managing_account = false;
        self.refreshed(w);
    }

    pub(super) fn refreshed(&mut self, w: &mut World) {
        if !self.managing_account
            && matches!(
                self.view,
                View::Rows | View::Providers(_) | View::Provider(_)
            )
            && self.ticket.is_some()
        {
            self.ticket = Some(w.next_ticket());
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

    fn metric_key(row: &BoardMetric) -> RowKey {
        RowKey::Metric(
            row.provider.clone(),
            row.account.clone(),
            row.metric.clone(),
        )
    }

    fn row_keys(&self) -> Vec<RowKey> {
        let Some(data) = &self.data else {
            return vec![];
        };
        match &self.view {
            View::Rows => data.rows.iter().map(Self::metric_key).collect(),
            View::Providers(_) => data
                .providers
                .iter()
                .map(|p| RowKey::Id(p.id.clone()))
                .collect(),
            View::Provider(index) => data
                .providers
                .get(*index)
                .into_iter()
                .flat_map(|p| &p.accounts)
                .map(|a| RowKey::Id(a.id.clone()))
                .collect(),
            _ => vec![],
        }
    }

    fn redraw(&self, cx: &mut Cx, previous_keys: Option<Vec<RowKey>>) {
        let mut form = self.form();
        let keys = previous_keys.map(|old| (old, self.row_keys()));
        let focus = keys.as_ref().and_then(|_| cx.focus.current());
        cx.focus.focus(form.restored_focus(focus));
        cx.with_top::<FormDialog>(move |current| {
            if current.id == ID {
                if let Some((old_keys, keys)) = keys {
                    form.scroll = current.scroll.clone();
                    for field in &mut form.fields {
                        if let FieldKindW::List { list, .. } = &mut field.kind
                            && let Some(old) =
                                current.fields.iter().find(|old| old.name == field.name)
                            && let FieldKindW::List { list: old, .. } = &old.kind
                        {
                            list.cursor = old_keys
                                .get(old.cursor)
                                .and_then(|selected| keys.iter().position(|key| key == selected))
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
            View::Providers(_) => "Usage providers",
            View::Provider(index) => self
                .data
                .as_ref()
                .and_then(|data| data.providers.get(index))
                .map_or("Provider", |provider| provider.name.as_str()),
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
                if !data.providers.is_empty() {
                    actions.push(("providers", "Providers…"));
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
            (Some(data), View::Providers(selected)) => {
                let items = data
                    .providers
                    .iter()
                    .map(|p| ListItem::new(&p.name).meta(&p.status()))
                    .collect();
                let mut list = ListBox::new(ID.sub("providers_list"), items, SelectMode::Single);
                list.cursor = (*selected).min(data.providers.len().saturating_sub(1));
                fields.push(FormField::list(
                    "providers_list",
                    list,
                    data.providers.len().clamp(1, 12) as u16,
                ));
                save = "View provider";
                actions.push(("back", "Back"));
            }
            (Some(data), View::Provider(index)) => {
                if let Some(provider) = data.providers.get(*index) {
                    if let Some(reason) = provider.connection_unavailable() {
                        fields.push(note(vec![reason.to_owned()]));
                    } else if provider.accounts.is_empty() {
                        fields.push(note(vec!["No usage accounts connected.".into()]));
                    }
                    if !provider.accounts.is_empty() {
                        let items = provider
                            .accounts
                            .iter()
                            .map(|account| {
                                let item = ListItem::new(&account.label);
                                if provider.accounts.iter().any(|other| {
                                    other.id != account.id && other.label == account.label
                                }) {
                                    item.meta(&account.id)
                                } else {
                                    item
                                }
                            })
                            .collect();
                        fields.push(FormField::list(
                            "provider_account",
                            ListBox::new(ID.sub("provider_account"), items, SelectMode::Single),
                            provider.accounts.len().clamp(1, 12) as u16,
                        ));
                        save = "View account";
                    } else if provider.connection_unavailable().is_none() {
                        save = "Add usage account…";
                    } else {
                        save = "Back";
                    }
                    if provider.connection_unavailable().is_none() && !provider.accounts.is_empty()
                    {
                        actions.push(("provider_add", "Add usage account…"));
                    }
                    if save != "Back" {
                        actions.push(("providers", "Back"));
                    }
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
        if let Some(error) = &self.refresh_error {
            form.error = Some(error.clone());
            form.save.label = "Retry".into();
            form.save.disabled = false;
        }
        for (key, label) in actions {
            form = form.action(key, Button::secondary(ID.sub(key), label));
        }
        form
    }

    pub(super) fn receive(&mut self, ticket: Ticket, update: &UsageUpdate, cx: &mut Cx) -> Outcome {
        if self.managing_account
            || self.ticket != Some(ticket)
            || !matches!(
                self.view,
                View::Loading | View::Rows | View::Providers(_) | View::Provider(_)
            )
        {
            return Outcome::Ignored;
        }
        let previous_keys = match update {
            UsageUpdate::Board(data) => {
                self.refresh_error = None;
                let keys = self.row_keys();
                let preserve = match self.view {
                    View::Provider(index) => {
                        let current = self.data.as_ref().and_then(|old| old.providers.get(index));
                        let next = current
                            .and_then(|p| data.providers.iter().position(|next| next.id == p.id));
                        if let Some(next) = next {
                            self.view = View::Provider(next);
                            true
                        } else {
                            self.view = if data.providers.is_empty() {
                                View::Rows
                            } else {
                                View::Providers(index.min(data.providers.len() - 1))
                            };
                            false
                        }
                    }
                    View::Providers(_) if data.providers.is_empty() => {
                        self.view = View::Rows;
                        false
                    }
                    View::Loading => {
                        self.view = View::Rows;
                        false
                    }
                    _ => true,
                };
                self.priority = data.priority;
                for alert in &data.alerts {
                    cx.status(alert.clone());
                }
                if let Some(selected) = self.data.as_ref().and_then(|old| old.rows.get(self.row)) {
                    self.row = data
                        .rows
                        .iter()
                        .position(|row| {
                            row.provider == selected.provider
                                && row.account == selected.account
                                && row.metric == selected.metric
                        })
                        .unwrap_or(self.row.min(data.rows.len().saturating_sub(1)));
                }
                self.data = Some(data.as_ref().clone());
                preserve.then_some(keys)
            }
            UsageUpdate::Failed(error) if !matches!(self.view, View::Loading) => {
                self.refresh_error = Some(error.clone());
                cx.focus.focus(ID.sub("save"));
                Some(self.row_keys())
            }
            UsageUpdate::Failed(error) => {
                self.view = View::Error(error.clone());
                None
            }
            _ => return Outcome::Ignored,
        };
        self.redraw(cx, previous_keys);
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
        if self.refresh_error.is_some() && key == "save" {
            self.ticket = Some(w.next_ticket());
            self.request(w, BoardEdit::Read);
            return Outcome::Changed;
        }
        let row = if let (Some(data), View::Details(i)) = (&self.data, &self.view) {
            data.rows.get(*i)
        } else {
            None
        };
        let action = match (key.as_str(), row) {
            ("providers", _) => {
                if matches!(self.view, View::Rows) {
                    self.row = choose(&values, "row");
                }
                self.view = View::Providers(match self.view {
                    View::Provider(index) => index,
                    _ => 0,
                });
                None
            }
            ("provider_add", _) => {
                if let View::Provider(index) = self.view
                    && let Some(provider) = self
                        .data
                        .as_ref()
                        .and_then(|data| data.providers.get(index))
                    && provider.connection_unavailable().is_none()
                {
                    self.managing_account = true;
                    self.account.add(w, cx, Some(&provider.id));
                }
                return Outcome::Changed;
            }
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
                View::Providers(_) => {
                    self.view = View::Provider(choose(&values, "providers_list"));
                    None
                }
                View::Provider(index) => {
                    if let Some(provider) = self
                        .data
                        .as_ref()
                        .and_then(|data| data.providers.get(*index))
                    {
                        if let Some(account) =
                            provider.accounts.get(choose(&values, "provider_account"))
                        {
                            self.managing_account = true;
                            self.account.show(account.clone(), w, cx);
                            return Outcome::Changed;
                        }
                        if provider.connection_unavailable().is_none() {
                            self.managing_account = true;
                            self.account.add(w, cx, Some(&provider.id));
                            return Outcome::Changed;
                        }
                    }
                    self.view = View::Providers(*index);
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
        self.refresh_error = None;
        if let Some(edit) = action {
            self.ticket = Some(w.next_ticket());
            self.view = View::Loading;
            self.request(w, edit);
        }
        self.redraw(cx, None);
        Outcome::Changed
    }
}
