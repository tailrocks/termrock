mod dashboard;

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
    dashboard: dashboard::Dashboard,
    details_back: Option<usize>,
}
#[derive(PartialEq, Eq)]
enum RowKey {
    Metric(String, Option<String>, String),
    Id(String),
    Account(String, String),
}

#[derive(Default)]
enum View {
    #[default]
    Loading,
    Rows,
    Providers(usize),
    Provider(usize),
    Readings(usize),
    Details(usize),
    Order(usize),
    Actions,
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
        self.dashboard.reset();
        self.details_back = None;
        cx.focus.focus(self.dashboard.primary());
        self.request(
            w,
            if priority.is_none() {
                BoardEdit::DefaultTab
            } else {
                BoardEdit::Read
            },
        );
    }

    pub(super) fn primary_focus(&self) -> WidgetId {
        self.dashboard.primary()
    }

    pub(super) fn active(&self) -> bool {
        self.ticket.is_some()
    }

    pub(super) fn deactivate(&mut self) {
        self.ticket = None;
    }

    pub(super) fn render(
        &mut self,
        area: ratatui::layout::Rect,
        buf: &mut ratatui::buffer::Buffer,
        ctx: &mut termrock::ui::ctx::RenderCtx,
    ) {
        use dashboard::Status;

        let status = match &self.view {
            View::Loading if self.data.is_some() => Status::Updating,
            View::Loading => Status::Loading,
            View::Error(error) => Status::Error(error),
            _ => self
                .refresh_error
                .as_deref()
                .map_or(Status::Ready, Status::Error),
        };
        self.dashboard
            .render(area, buf, ctx, self.priority, self.data.as_ref(), status);
    }

    pub(super) fn key(
        &mut self,
        key: &termrock::core::event::Key,
        w: &mut World,
        cx: &mut Cx,
    ) -> Outcome {
        use ratatui::crossterm::event::KeyCode;

        let action = match key.code {
            KeyCode::Char('r') if key.plain() && self.data.is_some() => Some("reload"),
            KeyCode::Enter if key.plain() && cx.focus.is(dashboard::ROWS) => Some("save"),
            _ => None,
        };
        if let Some(action) = action {
            return self.action(action, vec![], w, cx);
        }
        if let Some((action, button)) = self
            .dashboard
            .buttons
            .iter_mut()
            .find(|(_, button)| cx.focus.is(button.id))
        {
            let (outcome, activate) = button.on_key(key);
            let action = *action;
            return if activate {
                self.action(action, vec![], w, cx)
            } else {
                outcome
            };
        }
        if cx.focus.is(dashboard::ROWS) {
            self.dashboard.list.on_key(key)
        } else {
            Outcome::Ignored
        }
    }

    pub(super) fn pointer(
        &mut self,
        ev: &termrock::core::pointer::PointerEvent,
        w: &mut World,
        cx: &mut Cx,
    ) -> Outcome {
        use termrock::core::pointer::MouseTarget;

        if let Some(action) = self.dashboard.button_pointer(ev) {
            return self.action(action, vec![], w, cx);
        }
        if ev.owner() == dashboard::ROWS {
            let response = self.dashboard.list.pointer(ev);
            if ev.is(termrock::core::intent::Phase::Click) {
                cx.focus.focus(dashboard::ROWS);
                if ev.count() >= 2 && ev.index_of(termrock::core::id::Part::ROW).is_some() {
                    return self.action("save", vec![], w, cx);
                }
            }
            return response.outcome();
        }
        Outcome::Ignored
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
                View::Rows | View::Actions | View::Providers(_) | View::Provider(_)
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
        let mut save = "Details";
        let reading_title;
        let title = match self.view {
            View::Providers(_) => "Usage providers",
            View::Provider(index) => self
                .data
                .as_ref()
                .and_then(|data| data.providers.get(index))
                .map_or("Provider", |provider| provider.name.as_str()),
            View::Actions => "Usage actions",
            View::Readings(index) => {
                reading_title = self
                    .dashboard
                    .entries
                    .get(index)
                    .map(|entry| format!("{} · {}", entry.label, entry.provider));
                reading_title.as_deref().unwrap_or("Readings")
            }
            View::Details(index) => self
                .data
                .as_ref()
                .and_then(|data| data.rows.get(index))
                .map_or("Metric details", |row| row.title.as_str()),
            View::Order(_) => "Reorder usage",
            View::Settings => "Usage settings",
            View::Proxy => "Network proxy",
            _ if self.priority => "Priority",
            _ => "All Usage",
        };
        match (&self.data, &self.view) {
            (None, _) | (_, View::Loading | View::Error(_)) => {}
            (Some(data), View::Actions) => {
                let mut lines = data.notices.clone();
                if !matches!(data.summary.as_str(), "Priority" | "All Usage") {
                    lines.insert(0, data.summary.clone());
                }
                fields.push(note(lines));
                if !data.attention.is_empty() {
                    let labels: Vec<_> = data.attention.iter().map(|a| a.label.as_str()).collect();
                    fields.push(FormField::select(
                        "attention",
                        Select::new(ID.sub("attention"), "Needs attention", &labels, 0),
                    ));
                    actions.push(("fix", "Fix…"));
                }
                actions.extend([("settings", "Settings…"), ("share", "Copy report")]);
                if self.undo.is_some() {
                    actions.push(("undo", "Undo change"));
                }
                save = "Back";
            }
            (Some(data), View::Readings(group)) => {
                if let Some(entry) = self.dashboard.entries.get(*group) {
                    let items = entry
                        .indices
                        .iter()
                        .filter_map(|index| data.rows.get(*index))
                        .map(|row| {
                            ListItem::new(&row.label)
                                .meta(&row.value)
                                .meta_tone(row.severity)
                        })
                        .collect();
                    let mut list = ListBox::new(ID.sub("readings"), items, SelectMode::Single);
                    list.cursor = entry
                        .indices
                        .iter()
                        .position(|&index| index == self.row)
                        .unwrap_or(0);
                    fields.push(FormField::list(
                        "readings",
                        list,
                        entry.indices.len().clamp(1, 14) as u16,
                    ));
                    save = "Details";
                }
            }
            (Some(_), View::Rows) => {}
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
                    let value = if row.exact_value.is_empty() {
                        &row.value
                    } else {
                        &row.exact_value
                    };
                    let mut lines = vec![(value.clone(), row.severity)];
                    if let Some(reset) = &row.reset {
                        lines.push((reset.clone(), Tone::Secondary));
                    }
                    if let Some((amount, pace)) = row.bounded_bar() {
                        let bar: String = dashboard::quota_bar(amount, pace, 10)
                            .map(|segment| match segment {
                                dashboard::BarSegment::Pace => '│',
                                dashboard::BarSegment::Fill => '█',
                                dashboard::BarSegment::Track => '░',
                            })
                            .collect();
                        let bar = if pace.is_some() {
                            format!("{bar} · │ expected pace")
                        } else {
                            bar
                        };
                        lines.push((bar, row.severity));
                    }
                    lines.extend(row.details.iter().cloned().map(|line| (line, Tone::Normal)));
                    fields.push(FormField::note("reading", lines).wrap(true));
                    save = "Back";
                    if row.provider != "antigravity" {
                        actions.push(("refresh_provider", "Refresh provider"));
                    }
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
                            ("order", "Reorder…"),
                        ]);
                    }
                }
            }
            (Some(data), View::Order(index)) => {
                if let Some(row) = data.rows.get(*index) {
                    fields.push(note(vec![row.title.clone()]));
                    fields.push(FormField::select(
                        "order",
                        Select::new(
                            ID.sub("scope"),
                            "Move",
                            &["Metric", "Account", "Provider"],
                            0,
                        ),
                    ));
                    actions.extend([("up", "Move up"), ("down", "Move down")]);
                    save = "Back";
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
            .width(74)
            .keep_open_on_save();
        form.save.label = save.into();
        if let (Some(data), View::Details(index)) = (&self.data, &self.view)
            && let Some(row) = data.rows.get(*index)
        {
            form.meta = data
                .providers
                .iter()
                .find(|provider| provider.id == row.provider)
                .map(|provider| provider.name.clone());
        }
        if matches!(self.view, View::Actions | View::Readings(_)) {
            form.cancel.label = "Close".into();
        }
        if self.retry_in_modal()
            && let Some(error) = &self.refresh_error
        {
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
                View::Loading
                    | View::Error(_)
                    | View::Rows
                    | View::Actions
                    | View::Providers(_)
                    | View::Provider(_)
            )
        {
            return Outcome::Ignored;
        }
        let was_modal = self.modal();
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
                    View::Loading | View::Error(_) => {
                        self.view = View::Rows;
                        self.data.is_some()
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
                self.dashboard.replace(data);
                self.data = Some(data.as_ref().clone());
                preserve.then_some(keys)
            }
            UsageUpdate::Failed(error) if self.data.is_some() => {
                if matches!(self.view, View::Loading) {
                    self.view = View::Rows;
                }
                self.refresh_error = Some(error.clone());
                if self.retry_in_modal() {
                    cx.focus.focus(ID.sub("save"));
                }
                Some(self.row_keys())
            }
            UsageUpdate::Failed(error) => {
                self.view = View::Error(error.clone());
                None
            }
            _ => return Outcome::Ignored,
        };
        if was_modal && self.modal() {
            self.redraw(cx, previous_keys);
        } else if was_modal {
            cx.close();
        } else if cx.focus.current().is_none_or(|id| {
            let Some(data) = &self.data else {
                return id == ID.sub("switch");
            };
            (id == ID.sub("switch") && previous_keys.is_none())
                || (id == ID.sub("providers") && data.providers.is_empty())
                || (id == ID.sub("add") && !data.empty)
                || ([dashboard::ROWS, ID.sub("save")].contains(&id)
                    && self.dashboard.entries.is_empty())
        }) {
            cx.focus.focus(self.dashboard.primary());
        }
        Outcome::Changed
    }

    pub(super) fn result(&mut self, result: ModalResult, w: &mut World, cx: &mut Cx) -> Outcome {
        match result {
            ModalResult::FormAction(key, values) => self.action(&key, values, w, cx),
            ModalResult::Form(None) | ModalResult::Cancelled => {
                self.view = if self.data.is_some() {
                    View::Rows
                } else {
                    View::Loading
                };
                self.details_back = None;
                Outcome::Changed
            }
            _ => Outcome::Ignored,
        }
    }

    fn modal(&self) -> bool {
        !matches!(self.view, View::Loading | View::Rows | View::Error(_))
    }

    fn retry_in_modal(&self) -> bool {
        matches!(self.view, View::Providers(_) | View::Provider(_))
    }

    fn action(&mut self, key: &str, values: FormValues, w: &mut World, cx: &mut Cx) -> Outcome {
        let was_modal = self.modal();
        if matches!(self.view, View::Loading) && key != "switch" {
            return Outcome::Consumed;
        }
        if self.refresh_error.is_some() && self.retry_in_modal() && key == "save" {
            self.ticket = Some(w.next_ticket());
            self.request(w, BoardEdit::Read);
            return Outcome::Changed;
        }
        let row = if let (Some(data), View::Details(i) | View::Order(i)) = (&self.data, &self.view)
        {
            data.rows.get(*i)
        } else {
            None
        };
        let action = match (key, row) {
            ("providers", _) => {
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
            ("more", _) => {
                self.view = View::Actions;
                None
            }
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
                self.dashboard.reset();
                self.view = View::Loading;
                Some(BoardEdit::Read)
            }
            ("reload", _) => Some(BoardEdit::Refresh(None)),
            ("refresh_provider", Some(row)) if row.provider != "antigravity" => {
                Some(BoardEdit::Refresh(Some(row.provider.clone())))
            }
            ("order", _) => {
                if let View::Details(index) = self.view {
                    self.view = View::Order(index);
                }
                None
            }
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
                    self.dashboard.reset();
                    self.view = View::Loading;
                    Some(BoardEdit::Read)
                }
                View::Rows => {
                    self.details_back = None;
                    let group = self.dashboard.cursor();
                    if let Some(entry) = self.dashboard.entries.get(group) {
                        if entry.indices.is_empty() {
                            if let Some(account) = &entry.account {
                                self.managing_account = true;
                                self.account.show(account.clone(), w, cx);
                            }
                            return Outcome::Changed;
                        } else if entry.indices.len() == 1 {
                            self.row = entry.indices[0];
                            self.view = View::Details(self.row);
                        } else {
                            self.row = entry.ranked[0];
                            self.view = View::Readings(group);
                        }
                    }
                    None
                }
                View::Readings(group) => {
                    if let Some(&index) = self
                        .dashboard
                        .entries
                        .get(*group)
                        .and_then(|entry| entry.indices.get(choose(&values, "readings")))
                    {
                        self.row = index;
                        self.details_back = Some(*group);
                        self.view = View::Details(index);
                    }
                    None
                }
                View::Actions => {
                    self.view = View::Rows;
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
                View::Order(index) => {
                    self.view = View::Details(*index);
                    None
                }
                View::Details(_) => {
                    self.view = self.details_back.take().map_or(View::Rows, View::Readings);
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
            self.refresh_error = None;
            self.ticket = Some(w.next_ticket());
            self.view = View::Loading;
            self.request(w, edit);
        }
        match (was_modal, self.modal()) {
            (true, true) => self.redraw(cx, None),
            (true, false) => cx.close(),
            (false, true) => cx.open(self.form(), LayerTag::new("usage_board")),
            (false, false) => {}
        }
        Outcome::Changed
    }
}
