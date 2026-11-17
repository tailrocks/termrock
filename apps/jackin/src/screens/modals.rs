use termrock::core::event::{Key, Outcome};

use ratatui::buffer::Buffer;
use ratatui::crossterm::event::KeyCode;
use ratatui::layout::Rect;
use termrock::core::hit::Axis;
use termrock::core::id::WidgetId;
use termrock::core::intent::Phase;
use termrock::core::pointer::{MouseTarget, PointerEvent};
use termrock::ui::ctx::RenderCtx;
use termrock::ui::layer::{DismissReason, Layer, LayerCx, LayerOut, LayerSpec};
use termrock::widgets::keyhint::{Hint, hint};
use termrock::widgets::option_list::Item;
use termrock::widgets::picker::{Picker, PickerAction, PickerStatus};

use crate::backend::{FieldKind, OnePassword, OpError, OpListing, OpScope, Ticket};
use crate::domain::account::AccountId;
use crate::domain::agent::{Agent, Provider};
use crate::domain::onepassword::OpReference;
use crate::domain::workspace::{RoleEntry, Workspace};
use crate::world::{AgentOffer, World};

use super::ModalResult;
use crate::domain::docker::DockerRiskText;
use termrock::theme::StatusKind;
use termrock::ui::text::plural;
use termrock::widgets::button::Button;
use termrock::widgets::dialog::Dialog;
use termrock::widgets::props::Prop;

pub fn unsaved_dialog(id: WidgetId, changes: usize) -> Dialog {
    Dialog::confirm(
        id,
        "Unsaved changes",
        &format!(
            "Save changes before leaving? {} would be lost.",
            plural(changes.max(1), "change", "changes")
        ),
        "Save",
    )
    .actions(
        vec![
            Button::subtle(id.sub("exit_cancel"), "Cancel"),
            Button::danger(id.sub("exit_discard"), "Discard"),
            Button::primary(id.sub("exit_save"), "Save"),
        ],
        Some(0),
    )
}

pub fn trust_dialog(id: WidgetId, entry: &RoleEntry, w: &World) -> Dialog {
    Dialog::facts(
        id,
        "Trust role source",
        vec![
            Prop::new("Role", entry.full_name()),
            Prop::new("Repository", entry.source.label()),
        ],
        vec![
            "Trust this role source?".into(),
            "Dockerfile can run during image builds.".into(),
            w.surfaces.vocabulary.role_access.clone(),
        ],
        None,
        Button::primary(id.sub("ok"), "Trust & load"),
    )
}

/// The picker that asks which account an agent starts with: the accounts the
/// workspace offers, in the offer's order, and their ids in row order. The
/// offer's own choice, when it has one, carries the `preselected` tag.
pub fn account_picker(
    id: WidgetId,
    w: &World,
    agent: Agent,
    provider: Provider,
    ws: Option<&Workspace>,
    offer: &AgentOffer,
) -> (Picker, Vec<AccountId>) {
    let mut p = Picker::new(id, &format!("Account for {}", agent.label()));
    p.set_searchable(false)
        .expect("disabling picker search is valid");
    p.set_width(72);
    p.scope = Some("active for this Workspace".into());
    let mut items = vec![];
    let mut ids = vec![];
    for acc in offer.accounts.iter().filter_map(|id| w.accounts.get(id)) {
        let mut tags = vec![];
        if acc.default_for_provider {
            tags.push("global default");
        }
        if ws.is_some_and(|x| x.accounts.preferred.get(&provider) == Some(&acc.id)) {
            tags.push("Workspace preference");
        }
        if offer.preselected.as_ref() == Some(&acc.id) {
            tags.push("preselected");
        }
        items.push(
            Item::new(acc.display_name.clone())
                .detail(format!(
                    "{} · {}{}",
                    acc.source.origin_label(),
                    acc.status_word(),
                    if tags.is_empty() {
                        String::new()
                    } else {
                        format!(" · {}", tags.join(" · "))
                    }
                ))
                .glyph(if acc.default_for_provider { "★" } else { " " }),
        );
        ids.push(acc.id.clone());
    }
    p.set_items(items)
        .unwrap_or_else(|error| p.report_item_error(error));
    (p, ids)
}

pub fn docker_row_hints() -> [Hint; 2] {
    [hint("Space", "Toggle"), hint("i", "Risks")]
}

pub fn docker_risk_dialog(
    id: WidgetId,
    subject: &str,
    risks: &[DockerRiskText],
    confirm: bool,
) -> Dialog {
    let facts: Vec<Prop> = risks
        .iter()
        .map(|r| Prop::new(&r.label, r.text.clone()).wrap())
        .collect();
    if confirm {
        return Dialog::facts(
            id,
            &format!("Turn on Docker for {subject}?"),
            facts,
            vec![],
            None,
            Button::primary(id.sub("turn_on"), "Turn on"),
        )
        .title_status(StatusKind::Warn)
        .width(92);
    }
    let close = Button::primary(id.sub("close"), "Close");
    let focus = close.id;
    let mut d = Dialog::facts(
        id,
        &format!("Docker risks · {subject}"),
        facts,
        vec![],
        None,
        close.clone(),
    )
    .title_status(StatusKind::Warn)
    .width(92)
    .actions(vec![close], Some(0));
    d.initial_focus = focus;
    d
}

use termrock::widgets::file_browser::{Choose, FileCmd, UrlEntry};
pub use termrock::widgets::file_browser::{FileBrowser, FileBrowserResult as BrowserResult};

fn plain_token(text: &str) -> bool {
    !text.is_empty()
        && text
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || matches!(c, '.' | '_' | '-'))
}

fn repo_identity(text: &str) -> Option<(String, String)> {
    if text.starts_with('-') || text.chars().any(|c| c.is_whitespace() || c.is_control()) {
        return None;
    }
    let (authority, path) = match text.split_once("://") {
        Some((scheme, rest)) => {
            if !["https", "ssh", "git"]
                .iter()
                .any(|s| scheme.eq_ignore_ascii_case(s))
            {
                return None;
            }
            rest.split_once('/')?
        }
        None => match text.split_once(':') {
            Some((head, path)) if head.contains('@') => (head, path),
            _ => text.split_once('/')?,
        },
    };
    let host = match authority.split_once('@') {
        Some((user, host)) => plain_token(user).then_some(host)?,
        None => authority,
    };
    if !plain_token(host) {
        return None;
    }
    let host = host.to_ascii_lowercase();
    let path = path.trim_matches('/');
    let path = path.strip_suffix(".git").unwrap_or(path);
    if path.is_empty() {
        return None;
    }
    let path = if host == "github.com" {
        path.to_ascii_lowercase()
    } else {
        path.to_owned()
    };
    Some((host, path))
}

pub fn file_browser(
    id: WidgetId,
    title: &str,
    cwd: &str,
    with_readonly: bool,
    dirs_only: bool,
    w: &World,
) -> FileBrowser {
    let source = w.backend.file_source(w);
    let repos: Vec<(String, String)> = w
        .github
        .iter()
        .filter_map(|r| {
            repo_identity(&r.url).or_else(|| repo_identity(&format!("github.com/{}", r.full_name)))
        })
        .collect();
    let git_url = w.surfaces.fixed.as_ref().map(|f| f.git_url.clone());
    let url = UrlEntry::new("Git URL…", "Git URL")
        .back("Browse folders")
        .placeholder("github.com/org/repo")
        .help(&[
            w.surfaces.vocabulary.git_url_help[0].as_str(),
            w.surfaces.vocabulary.git_url_help[1].as_str(),
        ])
        .empty_error("Enter a repository URL")
        .validate(move |url| {
            if let Some(error) = &git_url {
                Err(error.clone())
            } else if repo_identity(url).is_some_and(|id| repos.contains(&id)) {
                Ok(())
            } else {
                Err(format!("Could not resolve {url}: repository not found"))
            }
        });
    let b = FileBrowser::new(id, title, source, cwd)
        .choose(if dirs_only { Choose::Dirs } else { Choose::Any })
        .keys(FileCmd::vim_keymap())
        .show_hidden(true)
        .hidden_toggle(false)
        .url_entry(url);
    if !with_readonly {
        return b;
    }
    let mut b = b.checkbox(&w.surfaces.vocabulary.read_only_label, false);
    if let Some(fixed) = &w.surfaces.fixed {
        b.checkbox = b.checkbox.take().map(|c| c.fixed(fixed.wizard_ro.clone()));
    }
    b
}

pub use termrock::widgets::choice_dialog::ChoiceDialog;
pub use termrock::widgets::form_dialog::{
    FieldValue, FormAction, FormDialog, FormField, FormFieldKind as FieldKindW, FormValues,
};
pub use termrock::widgets::info_dialog::{InfoDialog, InfoResult};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum OpStep {
    Account,
    Vault,
    Item,
    Field,
}

pub struct OpFlow {
    pub dismiss: termrock::ui::popup::Dismiss,
    pub step: OpStep,
    pub account: Option<String>,
    pub vault: Option<(String, String)>,
    pub item: Option<(String, String)>,
    pub picker: Picker,
    pub result: Option<Option<OpReference>>,

    pub dismissed: bool,
    want: Option<OpScope>,
    in_flight: Option<Ticket>,
    listing: Option<OpListing>,
    ids: Vec<String>,
    sections: Vec<Option<String>>,
    pub last_error: Option<OpError>,

    pub concealed_only: bool,
}

impl OpFlow {
    pub fn new(id: WidgetId, op: &dyn OnePassword) -> Self {
        let mut f = Self {
            dismiss: termrock::ui::popup::Dismiss::LIGHT,
            step: OpStep::Account,
            account: None,
            vault: None,
            item: None,
            picker: Picker::new(id.sub("picker"), "1Password"),
            result: None,
            dismissed: false,
            want: None,
            in_flight: None,
            listing: None,
            ids: vec![],
            sections: vec![],
            last_error: None,
            concealed_only: true,
        };
        f.picker.set_width(64);

        if let [only] = op.accounts().as_slice() {
            f.account = Some(only.id.clone());
            f.step = OpStep::Vault;
        }
        f.begin_load();
        f
    }

    fn scope(&self) -> OpScope {
        let account = self.account.clone().unwrap_or_default();
        let vault = self.vault.as_ref().map(|v| v.0.clone()).unwrap_or_default();
        let item = self.item.as_ref().map(|i| i.0.clone()).unwrap_or_default();
        match self.step {
            OpStep::Account => OpScope::Accounts,
            OpStep::Vault => OpScope::Vaults { account },
            OpStep::Item => OpScope::Items { account, vault },
            OpStep::Field => OpScope::Fields {
                account,
                vault,
                item,
            },
        }
    }

    /// True from the moment a listing is wanted until the backend answers it.
    pub fn loading(&self) -> bool {
        self.want.is_some() || self.in_flight.is_some()
    }

    /// The listing the flow wants asked of the backend, once. The host sends
    /// it and tells the flow which ticket the reply will carry.
    pub fn take_request(&mut self) -> Option<OpScope> {
        self.want.take()
    }

    pub fn sent(&mut self, ticket: Ticket) {
        self.in_flight = Some(ticket);
    }

    /// Takes in the reply to the listing it sent. A reply to an earlier
    /// request, one the flow has since left behind, is not the flow's.
    pub fn loaded(&mut self, ticket: Ticket, result: &Result<OpListing, OpError>) -> bool {
        if self.in_flight != Some(ticket) {
            return false;
        }
        self.in_flight = None;
        match result {
            Ok(listing) => {
                self.listing = Some(listing.clone());
                self.refresh();
            }
            Err(e) => self.set_error(e.clone()),
        }
        true
    }

    fn crumb(&self) -> String {
        let mut parts = vec!["1Password".to_owned()];
        if let Some(a) = &self.account {
            parts.push(a.split('.').next().unwrap_or(a).to_owned());
        }
        if let Some((_, v)) = &self.vault {
            parts.push(v.clone());
        }
        if let Some((_, i)) = &self.item {
            parts.push(i.clone());
        }
        parts.join(" › ")
    }

    fn begin_load(&mut self) {
        self.picker.set_query("");
        self.picker.title = match self.step {
            OpStep::Account => "Choose 1Password account".into(),
            OpStep::Vault => "Choose vault".into(),
            OpStep::Item => "Choose item".into(),
            OpStep::Field => "Choose field".into(),
        };
        self.picker.scope = Some(self.crumb());
        self.picker.placeholder = match self.step {
            OpStep::Account => "Search accounts…".into(),
            OpStep::Vault => "Search vaults…".into(),
            OpStep::Item => "Search items…".into(),
            OpStep::Field => "Search fields…".into(),
        };
        self.picker
            .set_searchable(true)
            .unwrap_or_else(|error| self.picker.report_item_error(error));
        self.picker
            .set_items(vec![])
            .unwrap_or_else(|error| self.picker.report_item_error(error));
        if !matches!(self.picker.status, PickerStatus::Error { .. }) {
            self.picker.status = PickerStatus::Loading(match self.step {
                OpStep::Account => "loading accounts…".into(),
                OpStep::Vault => "loading vaults…".into(),
                OpStep::Item => format!(
                    "loading items from {}…",
                    self.vault.as_ref().map(|v| v.1.as_str()).unwrap_or("vault")
                ),
                OpStep::Field => format!(
                    "loading {}…",
                    self.item.as_ref().map(|i| i.1.as_str()).unwrap_or("item")
                ),
            });
        }
        self.ids.clear();
        self.sections.clear();
        self.listing = None;
        self.in_flight = None;
        self.want = Some(self.scope());
    }

    fn set_error(&mut self, e: OpError) {
        let (msg, detail) = match &e {
            OpError::Locked => (
                "1Password is locked".to_owned(),
                Some("Unlock it in the 1Password app, then press r to retry".to_owned()),
            ),
            OpError::NotAvailable => (
                "1Password is not available".to_owned(),
                Some("Install the 1Password CLI and sign in, then press r to retry".to_owned()),
            ),
            OpError::AuthorizationRequired { account } => (
                "Jackin is not authorized for 1Password CLI".to_owned(),
                Some(format!(
                    "Approve the request for {account} in 1Password, then press r to retry"
                )),
            ),
            OpError::PermissionDenied { vault } => (
                format!("No access to vault {vault}"),
                Some("Choose another vault".into()),
            ),
            other => (other.message(), None),
        };
        self.picker.status = PickerStatus::Error {
            message: msg,
            detail,
        };
        self.last_error = Some(e);
    }

    fn refresh(&mut self) {
        self.picker.status = PickerStatus::Ready;
        self.last_error = None;
        let q = self.picker.query().to_lowercase();
        let matches = |s: &str| q.is_empty() || s.to_lowercase().contains(&q);
        let rows: Vec<(String, Option<String>, Item)> = match &self.listing {
            Some(OpListing::Accounts(rows)) => rows
                .iter()
                .filter(|a| matches(&a.id) || matches(&a.email))
                .map(|a| {
                    let item = Item::new(a.id.clone())
                        .detail(a.email.clone())
                        .glyph("▪")
                        .tag(match a.state {
                            crate::backend::OpAccountState::Available => "signed in",
                            crate::backend::OpAccountState::Locked => "locked",
                            crate::backend::OpAccountState::AuthorizationRequired => "authorize",
                        });
                    (a.id.clone(), None, item)
                })
                .collect(),
            Some(OpListing::Vaults(rows)) => rows
                .iter()
                .filter(|v| matches(&v.name))
                .map(|v| {
                    let item = Item::new(v.name.clone())
                        .detail(format!("{} items", v.items))
                        .glyph("▪")
                        .tag(match v.access {
                            None | Some(crate::backend::VaultAccess::ReadWrite) => "",
                            Some(crate::backend::VaultAccess::ReadOnly) => "read-only",
                            Some(crate::backend::VaultAccess::Denied) => "no access",
                        });
                    (v.id.clone(), None, item)
                })
                .collect(),
            Some(OpListing::Items(rows)) => rows
                .iter()
                .filter(|i| matches(&i.title))
                .map(|i| {
                    let item = Item::new(i.title.clone())
                        .detail(match i.fields {
                            Some(n) => format!("{} · {n} fields", i.category),
                            None => i.category.clone(),
                        })
                        .glyph(" ");
                    (i.id.clone(), None, item)
                })
                .collect(),
            Some(OpListing::Fields(rows)) => rows
                .iter()
                .filter(|f| matches(&f.label))
                .map(|f| {
                    let item = Item::new(f.label.clone())
                        .detail(match &f.section {
                            Some(section) => format!("{section} · {}", f.kind.label()),
                            None => f.kind.label().to_owned(),
                        })
                        .glyph(if f.kind == FieldKind::Concealed {
                            "•"
                        } else {
                            " "
                        })
                        .disabled(self.concealed_only && f.kind != FieldKind::Concealed);
                    (f.id.clone(), f.section.clone(), item)
                })
                .collect(),
            None => return,
        };
        if rows.is_empty() {
            self.picker.empty_text = "No matches".into();
        }
        let mut ids = Vec::new();
        let mut sections = Vec::new();
        let mut items = Vec::new();
        for (id, section, item) in rows {
            ids.push(id);
            sections.push(section);
            items.push(item);
        }
        self.ids = ids;
        self.sections = sections;
        self.picker
            .set_items(items)
            .unwrap_or_else(|error| self.picker.report_item_error(error));
    }

    fn back(&mut self, op: &dyn OnePassword) -> Outcome {
        match self.step {
            OpStep::Account => {
                self.result = Some(None);
            }
            OpStep::Vault => {
                if op.accounts().len() > 1 {
                    self.step = OpStep::Account;
                    self.account = None;
                    self.begin_load();
                } else {
                    self.result = Some(None);
                }
            }
            OpStep::Item => {
                self.step = OpStep::Vault;
                self.vault = None;
                self.begin_load();
            }
            OpStep::Field => {
                self.step = OpStep::Item;
                self.item = None;
                self.begin_load();
            }
        }
        Outcome::Changed
    }

    fn choose(&mut self, i: usize, op: &dyn OnePassword) -> Outcome {
        let (Some(row), Some(id)) = (
            self.picker.visible_item(i).map(|v| v.item.clone()),
            self.ids.get(i).cloned(),
        ) else {
            return Outcome::Consumed;
        };
        let section = self.sections.get(i).cloned().flatten();
        match self.step {
            OpStep::Account => {
                self.account = Some(row.label);
                self.step = OpStep::Vault;
            }
            OpStep::Vault => {
                self.vault = Some((id, row.label));
                self.step = OpStep::Item;
            }
            OpStep::Item => {
                self.item = Some((id, row.label));
                self.step = OpStep::Field;
            }
            OpStep::Field => {
                let (vid, _) = self.vault.clone().unwrap_or_default();
                let (iid, _) = self.item.clone().unwrap_or_default();
                match op.reference(
                    self.account.as_deref().unwrap_or(""),
                    &vid,
                    &iid,
                    &id,
                    section.as_deref(),
                ) {
                    Ok(r) => match op.describe(&r) {
                        Ok(_) => self.result = Some(Some(r)),
                        Err(e) => self.set_error(e),
                    },
                    Err(e) => self.set_error(e),
                }
                return Outcome::Changed;
            }
        }
        self.begin_load();
        Outcome::Changed
    }

    pub fn on_key(&mut self, key: &Key, op: &dyn OnePassword) -> Outcome {
        if key.is_char('r')
            && self.picker.status != PickerStatus::Ready
            && self.picker.query().is_empty()
        {
            self.begin_load();
            return Outcome::Changed;
        }
        if self.loading() && !key.is(KeyCode::Esc) {
            return Outcome::Consumed;
        }
        let (o, ev) = self.picker.on_key(key);
        match ev {
            Some(PickerAction::QueryChanged) => {
                if (self.picker.status == PickerStatus::Ready
                    || matches!(self.picker.status, PickerStatus::Error { .. }))
                    && self.last_error.is_none()
                {
                    self.refresh();
                }
                Outcome::Changed
            }
            Some(PickerAction::Chosen(i)) | Some(PickerAction::ChosenAlt(i)) => self.choose(i, op),
            Some(PickerAction::Back) | Some(PickerAction::Cancelled) => self.back(op),
            _ => o.or(Outcome::Consumed),
        }
    }

    pub fn pointer(&mut self, ev: &PointerEvent, op: &dyn OnePassword) -> Outcome {
        let Some(r) = self.picker.route(ev) else {
            return match ev.wheel() {
                Some((Axis::Vertical, delta)) => self.picker.wheel(Axis::Vertical, delta),
                _ if ev.is(Phase::Click) => Outcome::Consumed,
                _ => Outcome::Ignored,
            };
        };
        let (o, actions) = r.into_parts();
        let mut result = if ev.is(Phase::Click) {
            o.or(Outcome::Consumed)
        } else {
            o
        };
        for act in actions {
            result = result.or(match act {
                PickerAction::Chosen(i) => self.choose(i, op),
                PickerAction::Cancelled if self.dismiss.outside_click => {
                    self.dismissed = true;
                    Outcome::Changed
                }
                _ => Outcome::Consumed,
            });
        }
        result
    }

    pub fn hints(&self) -> Vec<Hint> {
        match (self.step, &self.picker.status) {
            (_, PickerStatus::Error { .. }) => vec![hint("r", "Retry"), hint("Esc", "Back")],
            (OpStep::Field, _) => vec![
                hint("↑↓", "Move"),
                hint("Enter", "Save reference"),
                hint("Esc", "Back to items"),
            ],
            (OpStep::Account, _) => vec![
                hint("↑↓", "Move"),
                hint("Enter", "Choose"),
                hint("Esc", "Cancel"),
            ],
            _ => vec![
                hint("↑↓", "Move"),
                hint("Enter", "Choose"),
                hint("Backspace", "Back"),
                hint("Esc", "Back"),
            ],
        }
    }

    pub fn render(&mut self, screen: Rect, buf: &mut Buffer, ctx: &mut RenderCtx) {
        self.picker.draw(screen, buf, ctx);
    }
}

impl Layer<ModalResult, World> for OpFlow {
    fn animating(&self) -> bool {
        true
    }
    fn spec(&self) -> LayerSpec {
        LayerSpec::modal(self.dismiss).initial_focus(self.picker.id)
    }
    fn render(&mut self, screen: Rect, buf: &mut Buffer, ctx: &mut RenderCtx, _: &World) {
        OpFlow::render(self, screen, buf, ctx);
    }
    fn on_key(&mut self, key: &Key, cx: &mut LayerCx<'_, World>) -> Outcome {
        OpFlow::on_key(self, key, cx.app.op.as_ref()).or(Outcome::Consumed)
    }
    fn pointer(&mut self, ev: &PointerEvent, cx: &mut LayerCx<'_, World>) -> Outcome {
        OpFlow::pointer(self, ev, cx.app.op.as_ref())
    }
    fn hints(&self, _: Option<WidgetId>) -> Vec<Hint> {
        OpFlow::hints(self)
    }
    fn on_dismiss(&mut self, _: DismissReason) {
        self.dismissed = true;
    }
    fn take_result(&mut self) -> Option<LayerOut<ModalResult>> {
        if std::mem::take(&mut self.dismissed) {
            return Some(LayerOut::Close(ModalResult::Cancelled));
        }
        self.result
            .take()
            .map(|r| LayerOut::Close(ModalResult::Op(r)))
    }
}

pub use termrock::widgets::help::HelpOverlay;
