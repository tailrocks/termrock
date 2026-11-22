use super::{
    Button, Cx, FormDialog, FormField, FormValues, LayerTag, ModalResult, Outcome, PlainKey,
    Select, TextInput, Ticket, Tone, WidgetId, World, choice_of, text_of,
};
use crate::usage_connections::{
    HubRow, LoginChoice, PRODUCTS, UsageAction, UsageEntry, UsageUpdate,
};

const ID: WidgetId = WidgetId::of("accounts.usage_connection");
const TAG: &str = "usage_connection";

#[derive(Default)]
pub(in crate::screens) struct Connections {
    ticket: Option<Ticket>,
    step: Step,
    target: Option<UsageEntry>,
    choice: Option<LoginChoice>,
    origin: Option<Step>,
    retry: Option<UsageAction>,
    hub_focus: Option<String>,
    mirror: Option<UsageEntry>,
    selected: Option<String>,
}

#[derive(Clone, Default)]
enum Step {
    #[default]
    Choose,
    Busy,
    Prompt(UsageUpdate),
    Accounts(Vec<UsageEntry>),
    Hubs(Vec<HubRow>),
    Hub(HubRow),
    HubEdit(Option<HubRow>),
    HubRemove(HubRow),
    Details(UsageEntry),
    Resets(UsageEntry),
    Confirm(UsageEntry, usize),
    Rename(UsageEntry),
    Remove(UsageEntry),
    Error(String),
}

fn regions(provider: &str) -> &'static [&'static str] {
    match provider {
        "mimo" => &["balance", "cn", "sgp", "ams"],
        "kimi" | "minimax" => &["global", "cn"],
        _ => &[],
    }
}

fn select(name: &str, label: &str, options: &[&str]) -> FormField {
    FormField::select(name, Select::new(ID.sub(name), label, options, 0))
}

fn note(lines: Vec<String>) -> FormField {
    FormField::note(
        "notice",
        lines.into_iter().map(|line| (line, Tone::Normal)).collect(),
    )
    .wrap(true)
}

fn input(label: &str, masked: bool, required: bool) -> FormField {
    let field = TextInput::new(ID.sub("input"), label).required(required);
    FormField::input("input", if masked { field.masked() } else { field })
}

impl Connections {
    pub(in crate::screens) fn owns(&self, ticket: Ticket) -> bool {
        self.ticket == Some(ticket)
    }

    pub(in crate::screens) fn add(&mut self, w: &mut World, cx: &mut Cx, provider: Option<&str>) {
        *self = Self::default();
        self.ticket = Some(w.next_ticket());
        self.target = None;
        self.choice = provider.map(|provider| LoginChoice {
            provider: provider.into(),
            device_login: None,
            region: None,
            replace: None,
        });
        self.step = Step::Choose;
        cx.open(self.form(), LayerTag::new(TAG));
    }

    pub(in crate::screens) fn show(&mut self, entry: UsageEntry, w: &mut World, cx: &mut Cx) {
        *self = Self::default();
        self.ticket = Some(w.next_ticket());
        self.target = None;
        self.selected = Some(entry.id.clone());
        self.step = Step::Details(entry);
        cx.open(self.form(), LayerTag::new(TAG));
    }

    pub(in crate::screens) fn refresh(&mut self, entry: UsageEntry, w: &mut World, cx: &mut Cx) {
        let id = entry.id.clone();
        self.show(entry, w, cx);
        self.pending(w, cx, UsageAction::Refresh(id));
    }

    pub(in crate::screens) fn list(&mut self, w: &mut World, cx: &mut Cx) {
        *self = Self::default();
        self.ticket = Some(w.next_ticket());
        self.target = None;
        self.step = Step::Busy;
        cx.open(self.form(), LayerTag::new(TAG));
        self.retry = Some(UsageAction::List);
        self.send(w, UsageAction::List);
    }

    fn send(&self, w: &World, action: UsageAction) {
        if let (Some(service), Some(ticket)) = (w.backend.usage_connections(), self.ticket) {
            service.request(ticket, action);
        }
    }

    fn redraw(&self, cx: &mut Cx) {
        let form = self.form();
        cx.focus.focus(form.initial_focus());
        cx.with_top::<FormDialog>(move |current| {
            if current.id == ID {
                *current = form;
            }
        });
    }

    pub(in crate::screens) fn form_changed(&self, form: &mut FormDialog) {
        if !matches!(self.step, Step::Choose) {
            return;
        }
        let product = self.target.as_ref().map_or_else(
            || PRODUCTS.get(form.choice("provider")).map_or("", |x| x.0),
            |row| row.provider.as_str(),
        );
        form.set_visible("google_plan", product == "gemini");
        form.set_visible("mimo_region", product == "mimo");
        form.set_visible("auth", matches!(product, "kimi" | "minimax"));
        form.set_visible("region", matches!(product, "kimi" | "minimax"));
    }

    fn form(&self) -> FormDialog {
        let mut actions: Vec<(&str, &str)> = Vec::new();
        let (title, save, fields) = match &self.step {
            Step::Choose => {
                actions.push(("hubs", "CLIProxyAPI hubs…"));
                let names = PRODUCTS.map(|(_, label)| label);
                let selected = self
                    .choice
                    .as_ref()
                    .and_then(|choice| {
                        PRODUCTS
                            .iter()
                            .position(|(provider, _)| *provider == choice.provider)
                    })
                    .unwrap_or(0);
                let mut fields = vec![FormField::select(
                    "provider",
                    Select::new(ID.sub("provider"), "Provider", &names, selected),
                )];
                if let Some(target) = &self.target {
                    fields = vec![note(vec![format!("Sign in again to {}", target.label)])];
                }
                fields.push(
                    FormField::note(
                        "google_plan",
                        vec![("Code Assist requires an eligible Google Cloud project. For a personal Google AI Ultra subscription, open All Usage → Providers.".into(), Tone::Secondary)],
                    )
                    .wrap(true)
                    .hidden(),
                );
                let saved = self.target.as_ref();
                let region = self.choice.as_ref().map_or_else(
                    || saved.and_then(|a| a.region.as_deref()),
                    |c| c.region.as_deref(),
                );
                for (id, label, product) in [
                    ("mimo_region", "MiMo product / cluster", "mimo"),
                    ("region", "Account region", "kimi"),
                ] {
                    let options = regions(product);
                    let selected = options.iter().position(|r| Some(*r) == region).unwrap_or(0);
                    fields.push(
                        FormField::select(id, Select::new(ID.sub(id), label, options, selected))
                            .hidden(),
                    );
                }
                fields.push(
                    FormField::select(
                        "auth",
                        Select::new(
                            ID.sub("auth"),
                            "Sign-in method",
                            &["API key", "Device sign-in"],
                            usize::from(
                                self.choice
                                    .as_ref()
                                    .and_then(|c| c.device_login)
                                    .unwrap_or_else(|| saved.is_some_and(|a| a.device_login)),
                            ),
                        ),
                    )
                    .hidden(),
                );
                ("Add usage account", "Continue", fields)
            }
            Step::Busy => (
                "Usage accounts",
                "Working…",
                vec![note(vec!["Waiting for a response…".into()])],
            ),
            Step::Prompt(UsageUpdate::Key { url }) => {
                actions.extend([("open", "Open key page"), ("copy_url", "Copy URL")]);
                (
                    "Connect usage account",
                    "Validate and save",
                    vec![note(vec![url.clone()]), input("API key", true, true)],
                )
            }
            Step::Prompt(UsageUpdate::Browser { url, accepts_paste }) => {
                actions.extend([("open", "Open browser"), ("copy_url", "Copy URL")]);
                let mut fields = vec![note(vec![
                    "Finish signing in in your browser.".into(),
                    url.clone(),
                ])];
                if *accepts_paste {
                    fields.push(input("Paste the authorization response", true, false));
                }
                (
                    "Browser sign-in",
                    if *accepts_paste {
                        "Submit response"
                    } else {
                        "Waiting…"
                    },
                    fields,
                )
            }
            Step::Prompt(UsageUpdate::Device { url, code }) => {
                actions.extend([
                    ("open", "Open browser"),
                    ("copy_code", "Copy code"),
                    ("copy_url", "Copy URL"),
                ]);
                (
                    "Device sign-in",
                    "Waiting…",
                    vec![note(vec![format!("Enter code {code} at"), url.clone()])],
                )
            }
            Step::Prompt(UsageUpdate::Field {
                label,
                optional,
                masked,
            }) => {
                let label = if *optional {
                    format!("{label} (optional)")
                } else {
                    label.clone()
                };
                (
                    "Connect usage account",
                    "Continue",
                    vec![input(&label, *masked, !*optional)],
                )
            }
            Step::Prompt(UsageUpdate::Workspaces(names)) => {
                let labels: Vec<_> = names.iter().map(String::as_str).collect();
                (
                    "Select workspace",
                    "Continue",
                    vec![select("workspace", "Workspace", &labels)],
                )
            }
            Step::Accounts(rows) => {
                actions.push(("hubs", "CLIProxyAPI hubs…"));
                actions.push(("add", "Add account…"));
                let labels: Vec<String> = rows
                    .iter()
                    .map(|e| format!("{} · {}", e.label, super::usage_provider(&e.provider)))
                    .collect();
                let fields = if labels.is_empty() {
                    vec![note(vec!["No usage accounts connected.".into()])]
                } else {
                    vec![FormField::select(
                        "account",
                        Select::new(
                            ID.sub("account"),
                            "Account",
                            &labels.iter().map(String::as_str).collect::<Vec<_>>(),
                            rows.iter()
                                .position(|e| Some(&e.id) == self.selected.as_ref())
                                .unwrap_or(0),
                        ),
                    )]
                };
                ("Usage accounts", "View usage", fields)
            }
            Step::Hubs(rows) => {
                actions.push(("add_hub", "Add hub…"));
                let labels: Vec<&str> = rows.iter().map(|h| h.address.as_str()).collect();
                let fields = if labels.is_empty() {
                    vec![note(vec!["No hubs connected.".into()])]
                } else {
                    vec![select("hub", "Hub", &labels)]
                };
                ("CLIProxyAPI hubs", "Manage", fields)
            }
            Step::Hub(hub) => {
                actions.extend([
                    ("sync_hub", "Sync accounts"),
                    ("hub_key", "Replace key…"),
                    ("remove_hub", "Remove…"),
                ]);
                let status = if hub.refused {
                    "Management key refused"
                } else {
                    "Connected"
                };
                (
                    "CLIProxyAPI hub",
                    "All hubs",
                    vec![note(vec![hub.address.clone(), status.into()])],
                )
            }
            Step::HubEdit(hub) => {
                let address = match hub {
                    Some(h) => note(vec![h.address.clone()]),
                    None => FormField::input(
                        "address",
                        TextInput::new(ID.sub("address"), "Hub URL").required(true),
                    ),
                };
                (
                    "Connect CLIProxyAPI hub",
                    "Validate and save",
                    vec![address, input("Management key", true, true)],
                )
            }
            Step::HubRemove(h) => (
                "Remove CLIProxyAPI hub?",
                "Remove",
                vec![note(vec![
                    h.address.clone(),
                    "Remove the local connection and its mirrored accounts?".into(),
                ])],
            ),
            Step::Details(entry) => {
                actions.extend([
                    ("refresh", "Refresh"),
                    ("reconnect", "Sign in again…"),
                    ("rename", "Rename…"),
                    ("remove", "Remove…"),
                ]);
                if entry.hub.is_some() {
                    actions.retain(|(action, _)| !matches!(*action, "reconnect" | "remove"));
                    actions.push(("hubs", "Manage hub…"));
                }
                if !entry.resets.is_empty() {
                    actions.push(("resets", "Reset inventory…"));
                }
                let mut fields = vec![format!(
                    "{} · {}",
                    entry.label,
                    super::usage_provider(&entry.provider)
                )];
                fields.extend(entry.details.iter().cloned());
                ("Account usage", "All accounts", vec![note(fields)])
            }
            Step::Resets(row) => {
                let names: Vec<_> = row.resets.iter().map(|r| r.description.as_str()).collect();
                (
                    "Reset inventory",
                    "Review reset",
                    vec![select("reset", "Reset", &names)],
                )
            }
            Step::Confirm(row, i) => (
                "Spend a reset?",
                "Confirm redemption",
                vec![note(vec![
                    row.label.clone(),
                    row.resets[*i].description.clone(),
                    "Redeeming this reset consumes it immediately.".into(),
                ])],
            ),
            Step::Remove(e) => (
                "Remove usage account",
                "Remove",
                vec![note(vec![format!(
                    "Remove the local connection for {}?",
                    e.label
                )])],
            ),
            Step::Rename(e) => (
                "Rename usage account",
                "Save",
                vec![FormField::input(
                    "input",
                    TextInput::new(ID.sub("input"), "Account name").value(&e.label),
                )],
            ),
            Step::Error(error) => (
                "Usage accounts",
                if self.retry.is_some() {
                    "Retry"
                } else {
                    "Back"
                },
                vec![note(vec![error.clone()])],
            ),
            Step::Prompt(_) => unreachable!(),
        };
        let mut form = FormDialog::new(ID, title, fields)
            .width(78)
            .keep_open_on_save();
        form.save.label = save.into();
        if matches!(
            self.step,
            Step::Confirm(_, _) | Step::Remove(_) | Step::HubRemove(_)
        ) {
            form.save.kind = termrock::theme::ButtonKind::Danger;
        }
        form.save.disabled = matches!(
            self.step,
            Step::Busy
                | Step::Prompt(
                    UsageUpdate::Device { .. }
                        | UsageUpdate::Browser {
                            accepts_paste: false,
                            ..
                        }
                )
        ) || matches!(&self.step,Step::Accounts(rows) if rows.is_empty())
            || matches!(&self.step,Step::Hubs(rows) if rows.is_empty());
        if !matches!(self.step, Step::Choose | Step::Accounts(_) | Step::Error(_))
            || matches!(&self.step,Step::Error(_) if self.retry.is_some())
            || matches!(self.step, Step::Choose) && self.target.is_some()
        {
            actions.push(("back", "Back"));
        }
        for (name, label) in actions {
            form = form.action(name, Button::secondary(ID.sub(name), label));
        }
        self.form_changed(&mut form);
        form
    }

    pub(in crate::screens) fn update(
        &mut self,
        ticket: Ticket,
        update: &UsageUpdate,
        cx: &mut Cx,
    ) -> Outcome {
        if Some(ticket) != self.ticket {
            return Outcome::Ignored;
        }
        if matches!(update, UsageUpdate::OpenFailed) {
            cx.with_top::<FormDialog>(|f| {
                f.error = Some("Could not open the browser. Copy the URL to continue.".into())
            });
            return Outcome::Changed;
        }
        self.step = match update {
            UsageUpdate::Accounts(rows) => Step::Accounts(rows.clone()),
            UsageUpdate::Hubs(rows) => {
                if let Some(id) = self.hub_focus.take() {
                    rows.iter()
                        .find(|h| h.id == id)
                        .cloned()
                        .map_or_else(|| Step::Hubs(rows.clone()), Step::Hub)
                } else {
                    Step::Hubs(rows.clone())
                }
            }
            UsageUpdate::Saved(row) => Step::Details(row.clone()),
            UsageUpdate::Failed(error) => Step::Error(error.clone()),
            prompt => Step::Prompt(prompt.clone()),
        };
        self.redraw(cx);
        Outcome::Changed
    }

    fn choose(&mut self, w: &mut World, cx: &mut Cx) {
        self.send(w, UsageAction::Cancel);
        self.ticket = Some(w.next_ticket());
        self.step = Step::Choose;
        self.redraw(cx);
    }

    pub(in crate::screens) fn result(
        &mut self,
        result: ModalResult,
        w: &mut World,
        cx: &mut Cx,
    ) -> Outcome {
        let (name, values) = match result {
            ModalResult::FormAction(name, values) => (name, values),
            ModalResult::Form(None) | ModalResult::Cancelled => {
                self.send(w, UsageAction::Cancel);
                self.ticket = None;
                return Outcome::Changed;
            }
            _ => return Outcome::Ignored,
        };
        match name.as_str() {
            "copy_code" => {
                if let Step::Prompt(UsageUpdate::Device { code, .. }) = &self.step {
                    cx.copy_as(code.clone(), "sign-in code");
                }
            }
            "copy_url" => {
                if let Step::Prompt(
                    UsageUpdate::Device { url, .. }
                    | UsageUpdate::Browser { url, .. }
                    | UsageUpdate::Key { url },
                ) = &self.step
                {
                    cx.copy_as(url.clone(), "sign-in URL");
                }
            }
            "open" => self.send(w, UsageAction::OpenBrowser),
            "back" => self.back(w, cx),
            "add" => {
                self.choice = None;
                self.target = None;
                self.choose(w, cx);
            }
            "reconnect" => {
                if let Step::Details(row) = &self.step {
                    self.choice = None;
                    self.target = Some(row.clone());
                    self.choose(w, cx);
                }
            }
            "rename" => {
                if let Step::Details(row) = &self.step {
                    self.step = Step::Rename(row.clone());
                    self.redraw(cx);
                }
            }
            "hubs" => {
                if let Step::Details(row) = &self.step {
                    self.hub_focus.clone_from(&row.hub);
                    self.mirror = Some(row.clone());
                }
                self.pending(w, cx, UsageAction::Hubs);
            }
            "add_hub" => {
                self.step = Step::HubEdit(None);
                self.redraw(cx);
            }
            "hub_key" => {
                if let Step::Hub(h) = &self.step {
                    self.step = Step::HubEdit(Some(h.clone()));
                    self.redraw(cx);
                }
            }
            "remove_hub" => {
                if let Step::Hub(h) = &self.step {
                    self.step = Step::HubRemove(h.clone());
                    self.redraw(cx);
                }
            }
            "sync_hub" => {
                if let Step::Hub(h) = &self.step {
                    self.hub_focus = Some(h.id.clone());
                    self.pending(w, cx, UsageAction::SyncHub(h.id.clone()));
                }
            }
            "resets" => {
                if let Step::Details(row) = &self.step {
                    self.step = Step::Resets(row.clone());
                    self.redraw(cx);
                }
            }
            "remove" => {
                if let Step::Details(row) = &self.step {
                    self.step = Step::Remove(row.clone());
                    self.redraw(cx);
                }
            }
            "refresh" => {
                if let Step::Details(row) = &self.step {
                    let action = UsageAction::Refresh(row.id.clone());
                    self.pending(w, cx, action);
                }
            }
            "save" => self.submit(values, w, cx),
            _ => {}
        }
        Outcome::Changed
    }

    fn back(&mut self, w: &mut World, cx: &mut Cx) {
        self.send(w, UsageAction::Cancel);
        self.ticket = Some(w.next_ticket());
        if matches!(self.step, Step::Error(_) | Step::Busy) {
            self.step = self.origin.take().unwrap_or(Step::Choose);
        } else {
            self.step = match &self.step {
                Step::Remove(a) | Step::Rename(a) | Step::Resets(a) => Step::Details(a.clone()),
                Step::Confirm(a, _) => Step::Resets(a.clone()),
                Step::HubRemove(h) | Step::HubEdit(Some(h)) => Step::Hub(h.clone()),
                Step::Hub(_) | Step::HubEdit(None) => {
                    if let Some(mirror) = self.mirror.take() {
                        Step::Details(mirror)
                    } else {
                        self.pending(w, cx, UsageAction::Hubs);
                        return;
                    }
                }
                Step::Details(_) | Step::Hubs(_) => {
                    self.pending(w, cx, UsageAction::List);
                    return;
                }
                Step::Choose => self.target.take().map_or(Step::Choose, Step::Details),
                _ => Step::Choose,
            };
        }
        self.redraw(cx);
    }

    fn pending(&mut self, w: &World, cx: &mut Cx, action: UsageAction) {
        self.retry = match &action {
            UsageAction::Refresh(_)
            | UsageAction::SyncHub(_)
            | UsageAction::List
            | UsageAction::Hubs => Some(action.clone()),
            _ => None,
        };
        if !matches!(self.step, Step::Busy | Step::Error(_)) {
            self.origin = Some(match &self.step {
                Step::Prompt(_) => Step::Choose,
                other => other.clone(),
            });
        }
        self.step = Step::Busy;
        self.redraw(cx);
        self.send(w, action);
    }

    fn submit(&mut self, values: FormValues, w: &mut World, cx: &mut Cx) {
        let action = match &self.step {
            Step::Choose => {
                let provider = self.target.as_ref().map_or_else(
                    || PRODUCTS[choice_of(&values, "provider")].0,
                    |r| r.provider.as_str(),
                );
                let region = regions(provider)
                    .get(choice_of(
                        &values,
                        if provider == "mimo" {
                            "mimo_region"
                        } else {
                            "region"
                        },
                    ))
                    .filter(|s| **s != "balance")
                    .map(|s| (*s).to_owned());
                let choice = LoginChoice {
                    provider: provider.into(),
                    device_login: matches!(provider, "kimi" | "minimax")
                        .then(|| choice_of(&values, "auth") == 1),
                    region,
                    replace: self.target.as_ref().map(|r| r.id.clone()),
                };
                self.choice = Some(choice.clone());
                UsageAction::Begin(choice)
            }
            Step::Prompt(
                UsageUpdate::Browser {
                    accepts_paste: true,
                    ..
                }
                | UsageUpdate::Key { .. },
            ) => match PlainKey::new(&text_of(&values, "input")) {
                Ok(key) => UsageAction::Submit(key),
                Err(_) => {
                    cx.with_top::<FormDialog>(|f| {
                        f.error = Some("Enter the requested credential.".into())
                    });
                    return;
                }
            },
            Step::Prompt(UsageUpdate::Field { optional, .. }) => {
                let text = text_of(&values, "input");
                if *optional && text.is_empty() {
                    UsageAction::Skip
                } else if let Ok(value) = PlainKey::new(&text) {
                    UsageAction::Submit(value)
                } else {
                    cx.with_top::<FormDialog>(|f| {
                        f.error = Some("Enter the requested value.".into())
                    });
                    return;
                }
            }
            Step::Prompt(UsageUpdate::Workspaces(_)) => {
                UsageAction::ChooseWorkspace(choice_of(&values, "workspace"))
            }
            Step::Accounts(rows) => {
                if let Some(row) = rows.get(choice_of(&values, "account")) {
                    self.selected = Some(row.id.clone());
                    self.step = Step::Details(row.clone());
                    self.redraw(cx);
                }
                return;
            }
            Step::Hub(_) => {
                self.mirror = None;
                UsageAction::Hubs
            }
            Step::Hubs(rows) => {
                if let Some(h) = rows.get(choice_of(&values, "hub")) {
                    self.step = Step::Hub(h.clone());
                    self.redraw(cx);
                }
                return;
            }
            Step::HubEdit(hub) => {
                let Ok(key) = PlainKey::new(&text_of(&values, "input")) else {
                    cx.with_top::<FormDialog>(|f| {
                        f.error = Some("Enter the management key.".into())
                    });
                    return;
                };
                UsageAction::ConnectHub {
                    address: hub
                        .as_ref()
                        .map_or_else(|| text_of(&values, "address"), |h| h.address.clone()),
                    key,
                    replace: hub.as_ref().map(|h| h.id.clone()),
                }
            }
            Step::HubRemove(h) => {
                self.mirror = None;
                UsageAction::RemoveHub(h.id.clone())
            }
            Step::Details(_) => UsageAction::List,
            Step::Resets(row) => {
                let i = choice_of(&values, "reset");
                if row.resets.get(i).is_some_and(|r| r.usable) {
                    self.step = Step::Confirm(row.clone(), i);
                    self.redraw(cx);
                } else {
                    cx.with_top::<FormDialog>(|f| {
                        f.error = Some("This reset cannot currently be redeemed.".into())
                    });
                }
                return;
            }
            Step::Confirm(row, i) => UsageAction::Redeem {
                account: row.id.clone(),
                index: row.resets[*i].index,
                identity: row.resets[*i].identity.clone(),
            },
            Step::Rename(row) => UsageAction::Rename {
                id: row.id.clone(),
                name: text_of(&values, "input"),
            },
            Step::Remove(row) => UsageAction::Remove(row.id.clone()),
            Step::Error(_) => {
                if let Some(action) = self.retry.clone() {
                    self.ticket = Some(w.next_ticket());
                    self.pending(w, cx, action);
                } else {
                    self.back(w, cx);
                }
                return;
            }
            Step::Busy | Step::Prompt(_) => return,
        };
        self.pending(w, cx, action);
    }
}
