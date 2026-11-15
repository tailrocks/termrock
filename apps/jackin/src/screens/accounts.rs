use crate::domain::vocabulary::Vocabulary;
use crate::screens::Jackin;
use junie_tui::shell::Page;
use std::collections::HashSet;

use junie_tui::core::event::{Key, Outcome};
use junie_tui::core::hit::Axis;
use junie_tui::core::id::WidgetId;
use junie_tui::core::intent::Phase;
use junie_tui::core::pointer::PointerEvent;
use junie_tui::core::scroll::{ScrollState, TrackClick};
use junie_tui::theme::{ButtonKind, StatusKind, Tone};
use junie_tui::ui::ctx::{RenderCtx, fill, legible_on_fill};
use junie_tui::ui::layout::{Split, SplitDir};
use junie_tui::ui::master_detail::{MasterDetail, Mode};
use junie_tui::ui::text::{fit, truncate, width};
use junie_tui::widgets::button::{Button, ButtonRow, RowOverflow};
use junie_tui::widgets::choice::RadioGroup;
use junie_tui::widgets::dialog::Dialog;
use junie_tui::widgets::empty::{self, EmptyState};
use junie_tui::widgets::input::TextInput;
use junie_tui::widgets::keyhint::{Hint, hint};
use junie_tui::widgets::meter_list::{MeterList, MeterRow};
use junie_tui::widgets::panel::{Panel, PanelFocus};
use junie_tui::widgets::progress::{Meter, MeterLevel, MeterTone, MeterVisual, spinner_frame};
use junie_tui::widgets::scrollbar;
use junie_tui::widgets::segments::Segment;
use junie_tui::widgets::select::Select;
use junie_tui::widgets::splitter::Splitter;
use junie_tui::widgets::viewport::{Span, render_spans, wrap_spans};
use ratatui::buffer::Buffer;
use ratatui::crossterm::event::KeyCode;
use ratatui::layout::{Position, Rect};
use ratatui::style::{Modifier, Style};

use super::modals::{BrowserResult, FormDialog, FormField, FormValues, OpFlow};
use super::row_meta::{ClockStamp, Moment, RowMeta, clocks};
use super::{Cx, Go, LayerTag, ModalResult, Screen};
use crate::backend::{
    CheckRow, Effect, FolderProbe, PlainKey, Ticket, Validation, ValidationOutcome,
    apply_validation,
};
use crate::domain::account::{
    Account, AccountId, AccountOrigin, AccountRegistry, CredentialSource, DetectedKind,
    DuplicateProbe, IssueCode, Lifecycle, SourceKind, UNSAVED_ID, ValidationState, fingerprint,
    masked, tail_of,
};
use crate::domain::agent::{Provider, UsageSurface};
use crate::domain::onepassword::OpReference;
use crate::domain::usage::{Freshness, OverallSummary, QuotaStatus, QuotaWindow, WindowUnit};
use crate::world::{Msg, World, job};
use junie_tui::ui::context::ContextAnchor;
use junie_tui::ui::text::plural;
use junie_tui::ui::text::thousands;
use junie_tui::widgets::menu::MenuItem;
use junie_tui::widgets::row_menu::RowMenu;

pub const TREE: WidgetId = WidgetId::of("accounts.tree");
pub const INSPECTOR: WidgetId = WidgetId::of("accounts.inspector");
pub const SEAM: WidgetId = WidgetId::of("accounts.seam");
pub const FORM: WidgetId = WidgetId::of("accounts.form");
pub const MENU: WidgetId = WidgetId::of("accounts.menu");

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Sel {
    Overview,
    Provider(UsageSurface),
    Account(AccountId),
    Add,
}

#[derive(Debug, Clone)]
struct Row {
    sel: Sel,
    depth: u16,
    star: bool,
    label: String,
    health: Option<(&'static str, Tone)>,
    meta: RowMeta,
    meta_tone: Tone,
    faint: bool,
    expandable: bool,
    expanded: bool,
}

/// What the cached rows were built from.
struct RowsStamp {
    accounts: u64,
    folded: HashSet<UsageSurface>,
    filter: Option<String>,
    selected: Sel,
}

struct FormCtx {
    serial: usize,
    editing: Option<AccountId>,
    op_ref: Option<OpReference>,
    op_masked: Option<String>,
    validation: Option<ValidationOutcome>,
    check: Option<Check>,
}

/// The form's one request to the backend that has not come back, and what
/// the form does with the reply.
struct Check {
    ticket: Ticket,
    /// What the form held when the request went out: a reply is the form's
    /// only while the form still holds it.
    request: FormValues,
    then: Then,
}

enum Then {
    Show,
    Save(FormValues),
    Probe {
        values: FormValues,
        outcome: Box<ValidationOutcome>,
    },
}

fn text_of(values: &FormValues, name: &str) -> String {
    values
        .iter()
        .find(|(n, _)| n == name)
        .map(|(_, v)| match v {
            super::modals::FieldValue::Text(s) => s.clone(),
            _ => String::new(),
        })
        .unwrap_or_default()
}

fn choice_of(values: &FormValues, name: &str) -> usize {
    values
        .iter()
        .find(|(n, _)| n == name)
        .map(|(_, v)| match v {
            super::modals::FieldValue::Choice(i) => *i,
            _ => 0,
        })
        .unwrap_or(0)
}

fn pending_note() -> Vec<(String, Tone)> {
    ["material", "identity", "quota   "]
        .iter()
        .map(|label| (format!("… {label}   checking"), Tone::Muted))
        .collect()
}

fn set_busy(form: &mut FormDialog, busy: bool) {
    if let Some((_, b)) = form.actions.iter_mut().find(|(n, _)| n == "validate") {
        b.busy = busy;
    }
}

fn begin_pending(form: &mut FormDialog) {
    set_busy(form, true);
    form.error = None;
    form.set_note("validation", pending_note());
    form.reveal("validation");
}

fn end_pending(form: &mut FormDialog) {
    set_busy(form, false);
    form.set_note("validation", vec![]);
}

const SAVE_TIMEOUT_SECS: i64 = 30;

/// What the screen does when the backend has stored the registry it sent.
struct Saving {
    status: String,
    select: Pick,
    unfold: Option<UsageSurface>,
    form: Option<usize>,
}

/// What a save selects once the backend has stored it.
enum Pick {
    Keep,
    Row(Sel),
    /// The account the backend added, which it names in its reply.
    Added,
}

pub struct AccountsScreen {
    pub selected: Sel,
    folded: HashSet<UsageSurface>,
    pub filter: Option<String>,
    rows: Vec<Row>,
    stamp: Option<RowsStamp>,
    clock: ClockStamp,
    scroll: ScrollState,
    inspector_scroll: ScrollState,
    tree_area: Rect,

    md: MasterDetail,
    actions: Vec<Button>,
    form: Option<FormCtx>,
    forms_opened: usize,
    pending_remove: Option<AccountId>,
    saving: Option<(Ticket, Saving)>,
    saving_since: i64,

    pub menu: RowMenu<Sel, &'static str>,
}

impl Default for AccountsScreen {
    fn default() -> Self {
        Self::new()
    }
}

fn chosen_provider(w: &World, index: usize) -> Provider {
    let list = &w.surfaces.account_form.providers;
    list[index.min(list.len() - 1)].provider
}

fn chosen_source(w: &World, index: usize) -> SourceKind {
    let list = &w.surfaces.account_form.sources;
    list[index.min(list.len() - 1)].kind
}

fn source_index(w: &World, kind: SourceKind) -> Option<usize> {
    w.surfaces
        .account_form
        .sources
        .iter()
        .position(|s| s.kind == kind)
}

impl AccountsScreen {
    pub fn new() -> Self {
        Self {
            selected: Sel::Overview,
            folded: HashSet::new(),
            filter: None,
            rows: vec![],
            stamp: None,
            clock: ClockStamp::default(),
            scroll: ScrollState::default(),
            inspector_scroll: ScrollState::default(),
            tree_area: Rect::ZERO,
            md: MasterDetail::new(TREE, INSPECTOR)
                .resizable(Split::new(34, 30, 40))
                .seam(Splitter::new(SEAM, SplitDir::Horizontal))
                .collapse_below(100)
                .summary_rows(6),
            actions: vec![],
            form: None,
            forms_opened: 0,
            pending_remove: None,
            saving: None,
            saving_since: 0,
            menu: RowMenu::new(MENU),
        }
    }

    pub fn select(&mut self, id: Option<AccountId>) {
        if let Some(id) = id {
            self.selected = Sel::Account(id);
        }
    }

    /// Builds the rows again only when something they are made of moved: the
    /// accounts epoch, the folded surfaces or the filter. Cursor, scroll,
    /// hover and the clock reuse them.
    fn ensure_rows(&mut self, w: &World) {
        let epoch = w.epochs().accounts;
        let fresh = self.stamp.as_ref().is_some_and(|stamp| {
            stamp.accounts == epoch && stamp.folded == self.folded && stamp.filter == self.filter
        });
        if fresh {
            if self
                .stamp
                .as_ref()
                .is_some_and(|s| s.selected != self.selected)
            {
                self.settle_selection(w);
                if let Some(stamp) = self.stamp.as_mut() {
                    stamp.selected = self.selected.clone();
                }
            }
            return;
        }
        self.build_rows(w);
        self.stamp = Some(RowsStamp {
            accounts: epoch,
            folded: self.folded.clone(),
            filter: self.filter.clone(),
            selected: self.selected.clone(),
        });
    }

    fn build_rows(&mut self, w: &World) {
        w.projection.work.count_accounts_build();
        let mut rows = vec![Row {
            sel: Sel::Overview,
            depth: 0,
            star: false,
            label: "Overview".into(),
            health: None,
            meta: RowMeta::plain(""),
            meta_tone: Tone::Muted,
            faint: false,
            expandable: false,
            expanded: false,
        }];
        let q = self
            .filter
            .as_ref()
            .map(|f| f.to_lowercase())
            .filter(|f| !f.is_empty());
        for surface in UsageSurface::ALL {
            let accounts: Vec<&Account> = w
                .surface_accounts(surface)
                .into_iter()
                .filter(|a| match &q {
                    Some(q) => {
                        a.display_name.to_lowercase().contains(q)
                            || a.provider.label().to_lowercase().contains(q)
                            || a.identity.label().to_lowercase().contains(q)
                            || a.status_word().contains(q.as_str())
                            || a.identity
                                .plan
                                .as_ref()
                                .is_some_and(|p| p.to_lowercase().contains(q))
                    }
                    None => true,
                })
                .collect();
            if surface == UsageSurface::Unsupported {
                if q.is_none() {
                    rows.push(Row {
                        sel: Sel::Provider(surface),
                        depth: 0,
                        star: false,
                        label: "Unsupported".into(),
                        health: None,
                        meta: RowMeta::plain("sentinel"),
                        meta_tone: Tone::Faint,
                        faint: true,
                        expandable: false,
                        expanded: false,
                    });
                }
                continue;
            }
            if accounts.is_empty()
                && (q.is_some()
                    || !surface
                        .provider()
                        .is_some_and(|p| p.agent().is_some_and(|a| a.registerable())))
            {
                if q.is_none() {
                    rows.push(Row {
                        sel: Sel::Provider(surface),
                        depth: 0,
                        star: false,
                        label: surface.surface_name().into(),
                        health: None,
                        meta: RowMeta::plain("not discovered"),
                        meta_tone: Tone::Faint,
                        faint: true,
                        expandable: false,
                        expanded: false,
                    });
                }
                continue;
            }
            let expanded = !self.folded.contains(&surface);
            let warn = accounts
                .iter()
                .filter(|a| {
                    a.enabled && matches!(a.usage.worst_status(), Some(QuotaStatus::Warning))
                })
                .count();
            let err = accounts
                .iter()
                .filter(|a| {
                    a.enabled
                        && (a.is_error_state()
                            || matches!(a.usage.worst_status(), Some(QuotaStatus::Exhausted)))
                })
                .count();
            let health = if err > 0 {
                Some(("!", Tone::Error))
            } else if warn > 0 {
                Some(("▲", Tone::Warning))
            } else {
                None
            };
            rows.push(Row {
                sel: Sel::Provider(surface),
                depth: 0,
                star: false,
                label: surface.surface_name().into(),
                health,
                meta: RowMeta::plain(if accounts.is_empty() {
                    "no accounts".into()
                } else {
                    accounts.len().to_string()
                }),
                meta_tone: Tone::Muted,
                faint: accounts.is_empty(),
                expandable: !accounts.is_empty(),
                expanded,
            });
            if expanded {
                for a in accounts {
                    let (meta, tone) = account_meta(a, w);
                    let health = if !a.enabled {
                        None
                    } else if a.is_error_state()
                        || matches!(a.usage.worst_status(), Some(QuotaStatus::Exhausted))
                    {
                        Some(("!", Tone::Error))
                    } else if matches!(a.usage.worst_status(), Some(QuotaStatus::Warning))
                        || a.usage.freshness.phase == Freshness::Stale
                    {
                        Some(("▲", Tone::Warning))
                    } else {
                        None
                    };
                    rows.push(Row {
                        sel: Sel::Account(a.id.clone()),
                        depth: 1,
                        star: a.default_for_provider,
                        label: if a.origin == AccountOrigin::Discovered
                            && a.display_name != "discovered"
                        {
                            format!("{} · discovered", a.display_name)
                        } else {
                            a.display_name.clone()
                        },
                        health,
                        meta,
                        meta_tone: tone,
                        faint: !a.enabled,
                        expandable: false,
                        expanded: false,
                    });
                }
            }
        }
        if q.is_none() {
            rows.push(Row {
                sel: Sel::Add,
                depth: 0,
                star: false,
                label: "+ Add account…".into(),
                health: None,
                meta: RowMeta::plain(""),
                meta_tone: Tone::Muted,
                faint: false,
                expandable: false,
                expanded: false,
            });
        }
        self.rows = rows;
        self.settle_selection(w);
        self.scroll.set_content(self.rows.len());
    }

    /// Keeps the selection on a row that exists: the provider of an account
    /// that went away, or Overview.
    fn settle_selection(&mut self, w: &World) {
        if !self.rows.iter().any(|r| r.sel == self.selected) {
            self.selected = match &self.selected {
                Sel::Account(id) => w
                    .accounts
                    .get(id)
                    .map(|a| Sel::Provider(a.surface))
                    .unwrap_or(Sel::Overview),
                _ => Sel::Overview,
            };
            if !self.rows.iter().any(|r| r.sel == self.selected) {
                self.selected = Sel::Overview;
            }
        }
    }

    /// Whether the clock moved since the last paint in a way the page shows:
    /// an age crossing a whole second, or a spinner frame. Nothing structural
    /// is rebuilt for it.
    fn clock_due(&self, w: &World) -> bool {
        let (rows_age, spinning) = clocks(
            self.scroll
                .visible_range()
                .filter_map(|i| self.rows.get(i))
                .map(|r| &r.meta),
        );
        let inspector_age = matches!(self.selected, Sel::Account(_));
        self.clock.due(w, rows_age || inspector_age, spinning)
    }

    fn cursor(&self) -> Option<usize> {
        self.rows.iter().position(|r| r.sel == self.selected)
    }

    fn move_cursor(&mut self, delta: isize) {
        let n = self.rows.len();
        if n == 0 {
            return;
        }
        let cur = self.cursor().unwrap_or(0) as isize;
        let next = (cur + delta).clamp(0, n as isize - 1) as usize;
        self.selected = self.rows[next].sel.clone();
        self.scroll.ensure_visible(next);
        self.inspector_scroll.jump_start();
    }

    fn selected_account<'a>(&self, w: &'a World) -> Option<&'a Account> {
        match &self.selected {
            Sel::Account(id) => w.accounts.get(id),
            _ => None,
        }
    }

    fn refresh_scope(&mut self, w: &mut World, cx: &mut Cx) {
        let ids: Vec<AccountId> = match &self.selected {
            Sel::Account(id) => vec![id.clone()],
            Sel::Provider(s) => w
                .accounts
                .accounts
                .iter()
                .filter(|a| a.surface == *s && a.enabled)
                .map(|a| a.id.clone())
                .collect(),
            _ => w
                .accounts
                .accounts
                .iter()
                .filter(|a| a.enabled)
                .map(|a| a.id.clone())
                .collect(),
        };
        if ids.is_empty() {
            cx.status("Nothing to refresh");
            return;
        }
        let mut started = 0;
        for (i, id) in ids.iter().enumerate() {
            if w.refresh_account(id, i as i64 * 160).is_some() {
                started += 1;
            }
        }
        if started == 0 {
            cx.status("Refresh already running");
        } else {
            let scope = match &self.selected {
                Sel::Account(id) => w.accounts.get(id).map(|a| a.title()).unwrap_or_default(),
                Sel::Provider(s) => s.label().to_owned(),
                _ => "all".into(),
            };
            cx.status(format!(
                "Refreshing {scope} · {}",
                plural(started, "account", "accounts")
            ));
        }
    }

    fn open_form(&mut self, editing: Option<&Account>, w: &World, cx: &mut Cx) {
        let title = match editing {
            Some(a) => format!("Edit account · {}", a.title()),
            None => "New account".into(),
        };
        let data = &w.surfaces.account_form;
        let words = &w.surfaces.vocabulary.source;
        let provider_idx = editing
            .map(|a| {
                data.providers
                    .iter()
                    .position(|c| c.provider == a.provider)
                    .unwrap_or(0)
            })
            .unwrap_or(0);
        let typed_key =
            w.surfaces.fixed.as_ref().and_then(|f| {
                source_index(w, SourceKind::PlainApiKey).map(|at| (at, &f.typed_key))
            });
        let source_idx = match editing {
            Some(a) => a
                .source
                .kind()
                .and_then(|k| source_index(w, k))
                .filter(|at| typed_key.is_none_or(|(typed, _)| typed != *at))
                .or(data.default_source)
                .unwrap_or(0),
            None => data.default_source.unwrap_or(0),
        };
        let labels: Vec<&str> = data.providers.iter().map(|c| c.label.as_str()).collect();
        let source_labels: Vec<&str> = data.sources.iter().map(|c| c.label.as_str()).collect();
        let mut source_radio = RadioGroup::new(
            FORM.sub("source"),
            "Credential source",
            &source_labels,
            source_idx,
        );
        if let Some((at, reason)) = typed_key {
            source_radio = source_radio.fixed_option(at, reason.clone());
        }
        let mut fields = vec![
            FormField::input(
                "name",
                TextInput::new(FORM.sub("name"), "Display name")
                    .required(true)
                    .placeholder("Personal, Work, Team…")
                    .value(editing.map(|a| a.display_name.as_str()).unwrap_or("")),
            ),
            FormField::input(
                "purpose",
                TextInput::new(FORM.sub("purpose"), "Purpose label")
                    .placeholder("personal · work · experiments")
                    .value(editing.and_then(|a| a.purpose.as_deref()).unwrap_or("")),
            ),
            FormField::select(
                "provider",
                Select::new(FORM.sub("provider"), "Provider", &labels, provider_idx),
            ),
            FormField::note("link", vec![]),
            FormField::radio("source", source_radio),
            FormField::chooser(
                "op",
                FORM.sub("op"),
                "1Password reference",
                "not chosen",
                "Choose…",
            ),
            FormField::note(
                "op_note",
                data.op_note
                    .iter()
                    .map(|n| (n.clone(), Tone::Muted))
                    .collect(),
            ),
            FormField::input(
                "folder",
                TextInput::new(FORM.sub("folder"), "Local folder")
                    .placeholder("~/.claude")
                    .value(match editing.map(|a| &a.source) {
                        Some(CredentialSource::LocalFolder { path, .. }) => path.as_str(),
                        _ => "",
                    }),
            )
            .hidden(),
            FormField::chooser("browse", FORM.sub("browse"), "", "", "Browse…").hidden(),
            FormField::input(
                "env",
                TextInput::new(FORM.sub("env"), &words.env_label)
                    .required(true)
                    .placeholder(&words.env_placeholder)
                    .help(&words.env_help)
                    .value(match editing.map(|a| &a.source) {
                        Some(CredentialSource::HostEnv { var, .. }) => var.as_str(),
                        _ => "",
                    }),
            )
            .hidden(),
            FormField::input(
                "key",
                TextInput::new(FORM.sub("key"), "API key")
                    .masked()
                    .reveal_tail(4)
                    .placeholder("paste the key · shown masked")
                    .help("Typed once; only a fingerprint and a four-character tail are stored"),
            )
            .hidden(),
            FormField::input(
                "endpoint",
                TextInput::new(FORM.sub("endpoint"), "Endpoint / deployment").value("api.x.ai"),
            )
            .hidden(),
            FormField::note("validation", vec![]),
        ];

        if let Some((_, reason)) = typed_key
            && let Some(at) = fields.iter().position(|f| f.name == "source")
        {
            fields.insert(
                at + 1,
                FormField::note("typed_key", vec![(reason.text.clone(), Tone::Muted)]),
            );
        }

        self.forms_opened += 1;
        let serial = self.forms_opened;
        let mut ctx = FormCtx {
            serial,
            editing: editing.map(|a| a.id.clone()),
            op_ref: None,
            op_masked: None,
            validation: None,
            check: None,
        };
        if let Some(CredentialSource::OnePassword(r)) = editing.map(|a| &a.source) {
            ctx.op_ref = Some(r.clone());
            ctx.op_masked = w.op.describe(r).ok().map(|d| d.masked);
            if let Some(f) = fields.iter_mut().find(|f| f.name == "op")
                && let super::modals::FieldKindW::Chooser { value, .. } = &mut f.kind
            {
                *value = r.display_path();
            }
        }
        if let (Some(a), Some(e)) = (editing, editing.and_then(|a| a.endpoint.as_ref())) {
            let _ = a;
            if let Some(f) = fields.iter_mut().find(|f| f.name == "endpoint")
                && let super::modals::FieldKindW::Input(i) = &mut f.kind
            {
                i.set_value(&e.host);
            }
        }
        self.form = Some(ctx);
        let mut plain = Button::subtle(FORM.sub("plain"), "Enter plain text instead");
        if let Some((_, reason)) = typed_key {
            plain = plain.fixed(reason.clone());
        }
        let mut form = FormDialog::new(FORM, &title, fields)
            .meta(if editing.is_some() {
                "form · edits"
            } else {
                "form · unsaved"
            })
            .action("plain", plain)
            .action(
                "validate",
                Button::secondary(FORM.sub("validate"), "Validate"),
            )
            .width(70)
            .keep_open_on_save();
        Self::reveal(&mut form, w);
        cx.open(form, LayerTag::new("form").n(serial));
    }

    fn reveal(form: &mut FormDialog, w: &World) {
        let provider = chosen_provider(w, form.choice("provider"));
        let source = chosen_source(w, form.choice("source"));
        form.set_note(
            "link",
            vec![(
                format!(
                    "Agent runtime {} · provider {} · usage surface {}",
                    provider.agent().map(|a| a.label()).unwrap_or("—"),
                    provider.label(),
                    provider.usage_surface().surface_name()
                ),
                Tone::Muted,
            )],
        );
        form.set_visible("op", source == SourceKind::OnePassword);
        form.set_visible("op_note", source == SourceKind::OnePassword);
        form.set_visible("folder", source == SourceKind::LocalFolder);
        form.set_visible("browse", source == SourceKind::LocalFolder);
        form.set_visible("env", source == SourceKind::HostEnv);
        form.set_visible("key", source == SourceKind::PlainApiKey);
        form.set_visible(
            "endpoint",
            provider.supports_endpoint() && source != SourceKind::LocalFolder,
        );
        if let Some(f) = form.field_mut("folder")
            && let super::modals::FieldKindW::Input(i) = &mut f.kind
        {
            i.placeholder = match provider {
                Provider::Anthropic => "~/.claude".into(),
                Provider::OpenAi => "~/.codex  (CODEX_HOME)".into(),
                Provider::XAi => "~/.grok".into(),
                Provider::Gemini => "~/.gemini".into(),
                Provider::Cursor => "~/.cursor".into(),
                _ => "~/.local/share/opencode".into(),
            };
            i.label = provider.folder_label().to_owned();
        }
        if let Some(f) = form.field_mut("key")
            && let super::modals::FieldKindW::Input(i) = &mut f.kind
        {
            i.label = provider.plain_key_label().to_owned();
        }
        if let Some((_, b)) = form.actions.iter_mut().find(|(n, _)| n == "plain") {
            b.disabled = source != SourceKind::OnePassword;
        }
    }

    fn check_request(
        &self,
        values: &FormValues,
        w: &World,
    ) -> Result<(Provider, CredentialSource, Option<PlainKey>), String> {
        let provider = chosen_provider(w, choice_of(values, "provider"));
        let source = chosen_source(w, choice_of(values, "source"));
        let ctx = self.form.as_ref().ok_or("no form")?;
        match source {
            SourceKind::OnePassword => {
                let r = ctx
                    .op_ref
                    .clone()
                    .ok_or("Choose a 1Password item and field first")?;
                Ok((provider, CredentialSource::OnePassword(r), None))
            }
            SourceKind::LocalFolder => {
                let path = text_of(values, "folder");
                if path.trim().is_empty() {
                    return Err("Enter or browse to the local agent folder".into());
                }
                Ok((
                    provider,
                    CredentialSource::LocalFolder {
                        path: path.trim().to_owned(),
                        detected: DetectedKind::Unknown,
                    },
                    None,
                ))
            }
            SourceKind::HostEnv => {
                let var = text_of(values, "env");
                if var.trim().is_empty() {
                    return Err(w.surfaces.vocabulary.source.env_empty.clone());
                }
                Ok((
                    provider,
                    CredentialSource::HostEnv {
                        var: var.trim().to_owned(),
                        detected: DetectedKind::Unknown,
                    },
                    None,
                ))
            }
            SourceKind::PlainApiKey => {
                let key = PlainKey::new(&text_of(values, "key")).map_err(|e| e.to_string())?;
                Ok((
                    provider,
                    CredentialSource::PlainApiKey {
                        fingerprint: fingerprint(key.expose()),
                        tail: tail_of(key.expose()),
                    },
                    Some(key),
                ))
            }
        }
    }

    /// Asks the backend to check the source the form describes. The form
    /// shows the check as pending until the reply arrives; `then` says what
    /// the reply does.
    fn start_check(&mut self, values: &FormValues, then: Then, w: &mut World, cx: &mut Cx) {
        if let Some(ctx) = self.form.as_mut()
            && let Some(check) = ctx.check.as_mut()
        {
            if check.request == *values {
                if matches!(then, Then::Save(_)) && matches!(check.then, Then::Show) {
                    check.then = then;
                    cx.status("Saving as soon as the check finishes");
                }
                return;
            }
            ctx.check = None;
        }
        let (provider, source, key) = match self.check_request(values, w) {
            Ok(request) => request,
            Err(e) => {
                cx.with_top::<FormDialog>(move |f| f.error = Some(e));
                return;
            }
        };
        let ticket = w.next_ticket();
        if let Some(ctx) = self.form.as_mut() {
            ctx.check = Some(Check {
                ticket,
                request: values.clone(),
                then,
            });
        }
        w.perform(Effect::CheckSource {
            ticket,
            provider,
            source,
            key,
        });
        cx.progress(job::SOURCE_CHECK, "Validating the source…");
        cx.with_top::<FormDialog>(begin_pending);
    }

    /// The pending check, if `ticket` is the one the form waits for.
    fn take_check(&mut self, ticket: Ticket) -> Option<Check> {
        let ctx = self.form.as_mut()?;
        if ctx.check.as_ref()?.ticket != ticket {
            return None;
        }
        ctx.check.take()
    }

    /// Drops the pending check: its reply, when it comes, is not the form's.
    fn cancel_check(&mut self, cx: &mut Cx) {
        if let Some(ctx) = self.form.as_mut()
            && ctx.check.take().is_some()
        {
            cx.with_top::<FormDialog>(end_pending);
        }
    }

    fn source_checked(
        &mut self,
        ticket: Ticket,
        result: &Result<Box<ValidationOutcome>, String>,
        w: &mut World,
        cx: &mut Cx,
    ) -> Outcome {
        let Some(check) = self.take_check(ticket) else {
            return Outcome::Ignored;
        };
        let outcome = match result {
            Ok(outcome) => (**outcome).clone(),
            Err(error) => {
                let error = error.clone();
                cx.with_top::<FormDialog>(move |f| {
                    end_pending(f);
                    f.error = Some(error);
                });
                return Outcome::Changed;
            }
        };
        match check.then {
            Then::Show => {
                let note = Self::validation_note(&outcome, &w.surfaces.vocabulary);
                let ok = outcome.level.is_some();
                if let Some(ctx) = self.form.as_mut() {
                    ctx.validation = Some(outcome);
                }
                cx.with_top::<FormDialog>(move |f| {
                    set_busy(f, false);
                    f.set_note("validation", note);
                    f.reveal("validation");
                    f.error = None;
                });
                cx.status(if ok {
                    "Validated · Save stores the reference, never the secret"
                } else {
                    "Validation failed · fix the source and try again"
                });
            }
            Then::Save(values) => self.checked_for_save(values, outcome, w, cx),
            Then::Probe { .. } => {}
        }
        Outcome::Changed
    }

    fn folder_probed(
        &mut self,
        ticket: Ticket,
        result: &Result<FolderProbe, String>,
        w: &mut World,
        cx: &mut Cx,
    ) -> Outcome {
        let Some(Check {
            then: Then::Probe { values, outcome },
            ..
        }) = self.take_check(ticket)
        else {
            return Outcome::Ignored;
        };
        cx.with_top::<FormDialog>(end_pending);
        match result {
            Ok(FolderProbe::Found(kind)) => self.finish_save(values, *outcome, *kind, w, cx),
            Ok(_) => self.finish_save(values, *outcome, DetectedKind::Unknown, w, cx),
            Err(error) => {
                let error = error.clone();
                cx.with_top::<FormDialog>(move |f| f.error = Some(error));
            }
        }
        Outcome::Changed
    }

    fn validation_note(v: &ValidationOutcome, words: &Vocabulary) -> Vec<(String, Tone)> {
        let row = |label: &str, r: &CheckRow| match r {
            CheckRow::Ok(t) => (format!("✓ {label}   {}", words.found(t)), Tone::Secondary),
            CheckRow::Failed(t) => (format!("! {label}   {}", words.check(t)), Tone::Error),
            CheckRow::Skipped(t) => (format!("— {label}   {}", words.check(t)), Tone::Muted),
        };
        let mut lines = vec![
            row("material", &v.material),
            row("identity", &v.identity_row),
            row("quota   ", &v.quota_row),
        ];
        if let Some(i) = &v.issue
            && !i.is_informational()
        {
            lines.push((
                words.reason(&i.reason),
                if i.code == IssueCode::QuotaUnsupported {
                    Tone::Warning
                } else {
                    Tone::Error
                },
            ));
        }
        lines
    }

    fn save_form(&mut self, values: FormValues, w: &mut World, cx: &mut Cx) {
        let name = text_of(&values, "name").trim().to_owned();
        if name.is_empty() {
            cx.with_top::<FormDialog>(|f| f.error = Some("Display name is required".into()));
            return;
        }
        if name.chars().count() > 64 {
            cx.with_top::<FormDialog>(|f| {
                f.error = Some("Display name is too long (64 max)".into())
            });
            return;
        }
        self.start_check(&values, Then::Save(values.clone()), w, cx);
    }

    fn checked_for_save(
        &mut self,
        values: FormValues,
        outcome: ValidationOutcome,
        w: &mut World,
        cx: &mut Cx,
    ) {
        if outcome.level.is_none() {
            let msg = outcome
                .issue
                .as_ref()
                .map(|i| w.surfaces.vocabulary.reason(&i.reason))
                .unwrap_or("Validation failed".into());
            let note = Self::validation_note(&outcome, &w.surfaces.vocabulary);
            cx.with_top::<FormDialog>(move |f| {
                end_pending(f);
                f.error = Some(msg);
                f.set_note("validation", note);
                f.reveal("validation");
            });
            return;
        }
        if chosen_source(w, choice_of(&values, "source")) == SourceKind::LocalFolder {
            let ticket = w.next_ticket();
            let path = text_of(&values, "folder").trim().to_owned();
            if let Some(ctx) = self.form.as_mut() {
                ctx.check = Some(Check {
                    ticket,
                    request: values.clone(),
                    then: Then::Probe {
                        values,
                        outcome: Box::new(outcome),
                    },
                });
            }
            w.perform(Effect::ProbeFolder { ticket, path });
            return;
        }
        cx.with_top::<FormDialog>(end_pending);
        self.finish_save(values, outcome, DetectedKind::Unknown, w, cx);
    }

    fn finish_save(
        &mut self,
        values: FormValues,
        outcome: ValidationOutcome,
        detected: DetectedKind,
        w: &mut World,
        cx: &mut Cx,
    ) {
        let get = |name: &str| text_of(&values, name);
        let name = get("name").trim().to_owned();
        let provider = chosen_provider(w, choice_of(&values, "provider"));
        let source_kind = chosen_source(w, choice_of(&values, "source"));
        let Some(ctx) = self.form.as_ref() else {
            return;
        };
        let editing = ctx.editing.clone();
        let source = match source_kind {
            SourceKind::OnePassword => CredentialSource::OnePassword(ctx.op_ref.clone().unwrap()),
            SourceKind::LocalFolder => CredentialSource::LocalFolder {
                path: get("folder").trim().to_owned(),
                detected,
            },
            SourceKind::HostEnv => CredentialSource::HostEnv {
                var: get("env").trim().to_owned(),
                detected,
            },
            SourceKind::PlainApiKey => {
                let key = get("key");
                let key = key.trim();
                CredentialSource::PlainApiKey {
                    fingerprint: fingerprint(key),
                    tail: tail_of(key),
                }
            }
        };

        let probe = match &source {
            CredentialSource::OnePassword(r) => DuplicateProbe::OpReference {
                canonical: r.canonical(),
                account: r.account.clone(),
            },
            CredentialSource::LocalFolder { path, .. } => DuplicateProbe::Folder {
                provider,
                path: path.clone(),
            },
            CredentialSource::PlainApiKey { fingerprint, .. } => DuplicateProbe::KeyFingerprint {
                provider,
                fingerprint: fingerprint.clone(),
            },
            CredentialSource::HostEnv { var, .. } => DuplicateProbe::HostEnv {
                provider,
                var: var.clone(),
            },
        };
        if let Some(dup) = w.accounts.find_duplicate(&probe)
            && Some(&dup.id) != editing.as_ref()
        {
            let msg = format!("Already registered: this source is used by {}", dup.title());
            cx.with_top::<FormDialog>(move |f| f.error = Some(msg));
            return;
        }
        if let Some(subject) = &outcome.identity.subject
            && let Some(dup) = w.accounts.find_duplicate(&DuplicateProbe::Identity {
                surface: provider.usage_surface(),
                subject: subject.clone(),
            })
            && Some(&dup.id) != editing.as_ref()
            && dup.origin == AccountOrigin::Registered
            && matches!(source, CredentialSource::LocalFolder { .. })
        {
            let msg = format!(
                "Already registered: {} authenticates the same identity",
                dup.title()
            );
            cx.with_top::<FormDialog>(move |f| f.error = Some(msg));
            return;
        }
        let name_taken = w.accounts.name_taken(provider, &name, editing.as_deref());
        let now = w.now_secs();
        let id = editing.clone().unwrap_or_else(|| UNSAVED_ID.to_owned());
        let purpose = get("purpose").trim().to_owned();
        let endpoint = if provider.supports_endpoint() && source_kind != SourceKind::LocalFolder {
            let e = get("endpoint").trim().to_owned();
            if e.is_empty() { None } else { Some(e) }
        } else {
            None
        };
        let first_for_provider = w
            .accounts
            .by_provider(provider)
            .filter(|a| a.origin == AccountOrigin::Registered)
            .count()
            == 0;

        let mut twin = None;
        if let CredentialSource::LocalFolder { path, .. } = &source {
            twin = w
                .accounts
                .accounts
                .iter()
                .find(|a| a.origin == AccountOrigin::Discovered && matches!(&a.source, CredentialSource::LocalFolder { path: p, .. } if p == path))
                .map(|a| a.id.clone());
        }
        let mut account = match editing.as_ref().and_then(|id| w.accounts.get(id).cloned()) {
            Some(mut a) => {
                a.display_name = name.clone();
                a.provider = provider;
                a.surface = provider.usage_surface();
                a.agent = provider.agent();
                a.source = source;
                a
            }
            None => Account::registered(&id, &name, provider, source),
        };
        account.purpose = if purpose.is_empty() {
            None
        } else {
            Some(purpose)
        };
        account.endpoint = endpoint.map(|host| crate::domain::account::Endpoint {
            label: "Grok Build".into(),
            host,
        });
        if account.endpoint.is_some() && !provider.supports_endpoint() {
            account.endpoint = None;
        }
        apply_validation(&mut account, &outcome, now);
        if first_for_provider && editing.is_none() {
            account.default_for_provider = true;
        }
        let title = account.title();
        let mut registry = w.accounts.clone();
        if let Some(t) = twin {
            registry.remove(&t);
        }
        if editing.is_some() {
            if let Some(slot) = registry.get_mut(&id) {
                *slot = account;
            }
            registry.revision += 1;
        } else {
            registry.insert(account);
        }
        let status = if name_taken {
            format!("Saved {title} · name already used for {}", provider.short())
        } else {
            format!("Saved {title}")
        };
        self.save_registry(
            registry,
            Saving {
                status,
                select: match editing {
                    Some(id) => Pick::Row(Sel::Account(id)),
                    None => Pick::Added,
                },
                unfold: Some(provider.usage_surface()),
                form: self.form.as_ref().map(|c| c.serial),
            },
            w,
            cx,
        );
    }

    fn save_registry(
        &mut self,
        registry: AccountRegistry,
        saving: Saving,
        w: &mut World,
        cx: &mut Cx,
    ) {
        let now = w.now_secs();
        if self.saving.is_some() && now - self.saving_since < SAVE_TIMEOUT_SECS {
            let msg = "Saving the previous change… try again when it finishes".to_owned();
            if saving.form.is_some() {
                cx.with_top::<FormDialog>(move |f| f.error = Some(msg));
            } else {
                cx.status(msg);
            }
            return;
        }
        let ticket = w.next_ticket();
        self.saving = Some((ticket, saving));
        self.saving_since = now;
        w.perform(Effect::SaveAccounts {
            ticket,
            base: Box::new(w.accounts.clone()),
            registry: Box::new(registry),
        });
    }

    fn finish_saving(&mut self, saving: Saving, added: Option<AccountId>, cx: &mut Cx) {
        if let Some(serial) = saving.form
            && self.form.as_ref().is_some_and(|c| c.serial == serial)
        {
            self.form = None;
            cx.go(Go::CloseTop {
                tag: LayerTag::new("form").n(serial),
                then: Some(TREE),
            });
        }
        match (saving.select, added) {
            (Pick::Row(sel), _) => self.selected = sel,
            (Pick::Added, Some(id)) => self.selected = Sel::Account(id),
            (Pick::Added, None) | (Pick::Keep, _) => {}
        }
        if let Some(surface) = saving.unfold {
            self.folded.remove(&surface);
        }
        cx.status(saving.status);
    }

    fn remove(&mut self, id: &str, w: &World, cx: &mut Cx) {
        let Some(a) = w.accounts.get(id) else { return };
        if !a.mutations_allowed() {
            cx.status("Discovered accounts are read-only · nothing to remove");
            return;
        }
        let refs = w
            .workspaces
            .iter()
            .filter(|ws| {
                ws.accounts.enabled.contains(id)
                    || ws.accounts.preferred.values().any(|v| v == id)
                    || ws.accounts.role_preferred.values().any(|v| v == id)
            })
            .count();
        let body = format!(
            "{} is removed from the registry. {} Workspace or Role selections that point at it fall back to the provider default. The credential source itself is untouched.",
            a.title(),
            if refs == 0 {
                "No".to_owned()
            } else {
                refs.to_string()
            }
        );
        self.pending_remove = Some(id.to_owned());
        let d = Dialog::destructive(
            WidgetId::of("accounts.remove"),
            &format!("Remove account {}?", a.display_name),
            &body,
            "Remove",
        );
        cx.open(d, LayerTag::new("remove").key(id));
    }

    fn toggle_enabled(&mut self, id: &str, w: &mut World, cx: &mut Cx) {
        if let Some(fixed) = &w.surfaces.fixed {
            cx.refused(fixed.enable_global.clone());
            return;
        }
        let mut registry = w.accounts.clone();
        let Some(a) = registry.get_mut(id) else {
            return;
        };
        if !a.mutations_allowed() {
            cx.status("Discovered accounts are read-only");
            return;
        }
        a.enabled = !a.enabled;
        let (title, enabled, was_default) = (a.title(), a.enabled, a.default_for_provider);
        let status = if !enabled && was_default {
            a.default_for_provider = false;
            format!("Disabled {title} · provider default cleared")
        } else {
            format!("{} {title}", if enabled { "Enabled" } else { "Disabled" })
        };
        registry.revision += 1;
        self.save_registry(
            registry,
            Saving {
                status,
                select: Pick::Keep,
                unfold: None,
                form: None,
            },
            w,
            cx,
        );
    }

    fn set_default(&mut self, id: &str, w: &mut World, cx: &mut Cx) {
        let title = w.accounts.get(id).map(|a| a.title()).unwrap_or_default();
        let mut registry = w.accounts.clone();
        match registry.set_default(id) {
            Ok(()) => {
                let p = registry.get(id).map(|a| a.provider.short()).unwrap_or("");
                let status = format!("Default set: {p} → {title}");
                self.save_registry(
                    registry,
                    Saving {
                        status,
                        select: Pick::Keep,
                        unfold: None,
                        form: None,
                    },
                    w,
                    cx,
                );
            }
            Err(e) => cx.error(format!("Cannot set default: {e}")),
        }
    }

    fn validate_account(&mut self, id: &str, w: &mut World, cx: &mut Cx) {
        let Some(title) = w.accounts.get(id).map(Account::title) else {
            return;
        };
        if w.validate_account(id).is_some() {
            cx.progress(job::ACCOUNT_VALIDATE, format!("Validating {title}…"));
        }
    }

    fn show_validation(
        &mut self,
        id: &str,
        result: &Result<Validation, String>,
        w: &World,
        cx: &mut Cx,
    ) {
        let Some(a) = w.accounts.get(id) else { return };
        match result {
            Err(error) => cx.error(format!("{}: {error}", a.title())),
            Ok(validation) => match validation {
                Validation::Fingerprint { level } => {
                    let short = match &a.source {
                        CredentialSource::PlainApiKey { fingerprint, .. } => {
                            fingerprint.chars().take(8).collect::<String>()
                        }
                        _ => String::new(),
                    };
                    cx.status(format!(
                        "{} · fingerprint {short} matches · {}",
                        a.title(),
                        level.label()
                    ));
                }
                Validation::Checked(outcome) => {
                    let word = match outcome.level {
                        Some(l) => format!("Source validated · {}", l.label()),
                        None => outcome
                            .issue
                            .as_ref()
                            .map(|i| w.surfaces.vocabulary.reason(&i.reason))
                            .unwrap_or("Validation failed".into()),
                    };
                    if outcome.level.is_some() {
                        cx.status(format!("{} · {word}", a.title()));
                    } else {
                        cx.error(format!("{}: {word}", a.title()));
                    }
                }
            },
        }
    }

    fn rebuild_actions(&mut self, w: &World) {
        let mk = |n: &str, l: &str, k: ButtonKind| Button::new(INSPECTOR.sub(n), l, k);
        self.actions = match &self.selected {
            Sel::Account(id) => match w.accounts.get(id) {
                Some(a) if a.mutations_allowed() => {
                    let edit = mk("edit", "Edit…", ButtonKind::Secondary);
                    let edit = match &a.edit_blocked {
                        Some(why) => edit.fixed(why.reason()),
                        None => edit.disabled(!a.edit_allowed()),
                    };
                    let toggle = mk(
                        "toggle",
                        if a.enabled { "Disable" } else { "Enable" },
                        ButtonKind::Secondary,
                    );
                    let toggle = match &w.surfaces.fixed {
                        Some(fixed) => toggle.fixed(fixed.enable_global.clone()),
                        None => toggle,
                    };
                    vec![
                        mk("refresh", "Refresh", ButtonKind::Secondary),
                        mk("validate", "Validate", ButtonKind::Secondary),
                        edit,
                        mk("default", "Set default", ButtonKind::Secondary)
                            .disabled(a.default_for_provider || !a.enabled),
                        toggle,
                        mk("remove", "Remove…", ButtonKind::Danger),
                    ]
                }
                Some(_) => vec![mk("refresh", "Refresh", ButtonKind::Secondary)],
                None => vec![],
            },
            Sel::Provider(_) => vec![
                mk("refresh", "Refresh provider", ButtonKind::Secondary),
                mk("add", "Add account…", ButtonKind::Secondary),
            ],
            Sel::Overview => vec![
                mk("refresh", "Refresh all", ButtonKind::Secondary),
                mk("add", "Add account…", ButtonKind::Secondary),
            ],
            Sel::Add => vec![mk("add", "Add account…", ButtonKind::Primary)],
        };
    }

    fn fire(&mut self, name: &str, w: &mut World, cx: &mut Cx) -> Outcome {
        let id = match &self.selected {
            Sel::Account(id) => Some(id.clone()),
            _ => None,
        };
        match (name, id) {
            ("refresh", _) => self.refresh_scope(w, cx),
            ("validate", Some(id)) => self.validate_account(&id, w, cx),
            ("edit", Some(id)) => {
                if let Some(a) = w.accounts.get(&id).cloned() {
                    self.edit_account(&a, w, cx);
                }
            }
            ("default", Some(id)) => self.set_default(&id, w, cx),
            ("toggle", Some(id)) => self.toggle_enabled(&id, w, cx),
            ("remove", Some(id)) => self.remove(&id, w, cx),
            ("add", _) => self.open_form(None, w, cx),
            _ => return Outcome::Consumed,
        }
        Outcome::Changed
    }

    fn edit_account(&mut self, a: &Account, w: &World, cx: &mut Cx) {
        match (&a.edit_blocked, a.mutations_allowed()) {
            (_, false) => cx.status("Discovered accounts are read-only · register a copy with a"),
            (Some(why), true) => cx.refused(why.reason()),
            (None, true) => self.open_form(Some(a), w, cx),
        }
    }

    fn menu_items(&self, sel: &Sel, w: &World) -> (String, Vec<(MenuItem, &'static str)>) {
        let item = |label: &str, key: &'static str| MenuItem::new(label).shortcut(key);
        match sel {
            Sel::Account(id) => {
                let Some(a) = w.accounts.get(id) else {
                    return (String::new(), vec![]);
                };
                let read_only = "discovered accounts are read-only";
                let gate = |m: MenuItem| {
                    if a.mutations_allowed() {
                        m
                    } else {
                        m.reason(read_only)
                    }
                };
                let edit = match (&a.edit_blocked, a.mutations_allowed()) {
                    (Some(why), true) => item("Edit…", "e").reason(&why.short),
                    _ => gate(item("Edit…", "e")),
                };
                let toggle = gate(item(if a.enabled { "Disable" } else { "Enable" }, "t"));
                let toggle = match (&w.surfaces.fixed, a.mutations_allowed()) {
                    (Some(fixed), true) => toggle.reason(fixed.enable_global.short_or_text()),
                    _ => toggle,
                };
                let default = if !a.mutations_allowed() {
                    item("Set default", "Space").reason(read_only)
                } else if a.default_for_provider {
                    item("Set default", "Space").reason("already the provider default")
                } else if !a.enabled {
                    item("Set default", "Space").reason("the account is disabled")
                } else {
                    item("Set default", "Space")
                };
                (
                    a.display_name.clone(),
                    vec![
                        (item("Refresh", "r"), "refresh"),
                        (item("Validate", "v"), "validate"),
                        (item("Show usage", "m").separator(), "usage"),
                        (edit, "edit"),
                        (default, "default"),
                        (toggle.separator(), "toggle"),
                        (gate(item("Remove…", "d")).danger(), "remove"),
                    ],
                )
            }
            Sel::Provider(s) => {
                let expandable = self.rows.iter().any(|r| r.sel == *sel && r.expandable);
                let fold = item(
                    if self.folded.contains(s) {
                        "Expand"
                    } else {
                        "Collapse"
                    },
                    "Space",
                );
                (
                    s.surface_name().into(),
                    vec![
                        (item("Refresh provider", "r"), "refresh"),
                        (item("Add account…", "a"), "add"),
                        (
                            if expandable {
                                fold
                            } else {
                                fold.reason("no accounts")
                            },
                            "fold",
                        ),
                    ],
                )
            }
            Sel::Overview => (
                "Overview".into(),
                vec![
                    (item("Refresh all", "r"), "refresh"),
                    (item("Add account…", "a"), "add"),
                ],
            ),
            Sel::Add => (
                "New account".into(),
                vec![(item("Add account…", "Enter"), "add")],
            ),
        }
    }

    fn open_menu(&mut self, i: usize, anchor: ContextAnchor, w: &World) {
        let Some(sel) = self.rows.get(i).map(|r| r.sel.clone()) else {
            return;
        };
        let (title, items) = self.menu_items(&sel, w);
        if !items.is_empty() {
            self.menu.open(sel, anchor, title, items);
        }
    }

    fn run_menu(&mut self, target: Sel, cmd: &'static str, w: &mut World, cx: &mut Cx) -> Outcome {
        let prev = std::mem::replace(&mut self.selected, target.clone());
        let out = match (cmd, &target) {
            ("usage", Sel::Account(id)) => {
                cx.go(Go::Usage {
                    select: Some(id.clone()),
                });
                Outcome::Changed
            }
            ("fold", Sel::Provider(s)) => {
                if !self.folded.remove(s) {
                    self.folded.insert(*s);
                }
                self.ensure_rows(w);
                Outcome::Changed
            }
            _ => self.fire(cmd, w, cx),
        };

        if !self.menu.config.moves_cursor && self.selected == target {
            self.selected = prev;
        }
        out.or(Outcome::Changed)
    }

    fn draw_tree(&mut self, area: Rect, buf: &mut Buffer, ctx: &mut RenderCtx, w: &World) {
        let t = ctx.theme;
        let focused = ctx.interaction.focused(TREE);
        let summary = OverallSummary::compute(&w.accounts.accounts);
        let pos = scrollbar::position_label(&self.scroll);
        let mut meta = format!("{}", summary.counts.accounts);
        if summary.counts.warnings > 0 {
            meta.push_str(&format!(" · {} ▲", summary.counts.warnings));
        }
        if summary.counts.exhausted + summary.counts.failed > 0 {
            meta.push_str(&format!(
                " · {} !",
                summary.counts.exhausted + summary.counts.failed
            ));
        }
        if !pos.is_empty() {
            meta.push_str(&format!(" · {pos}"));
        }
        let title = match &self.filter {
            Some(f) if !f.is_empty() => format!("Accounts · filter {f}"),
            _ => "Accounts".into(),
        };
        let inner = Panel::framed(Some(&title))
            .focused(focused)
            .meta(&meta)
            .render(area, buf, t);
        self.tree_area = inner;
        let bg = t.canvas;
        self.scroll.set_viewport(inner.height as usize);
        if let Some(i) = self.cursor() {
            self.scroll.follow(i);
        }
        ctx.control(TREE, inner, false);
        ctx.scrollable(TREE, inner);
        if inner.is_empty() {
            return;
        }
        let has_sb = self.scroll.overflows();
        let row_w = inner.width.saturating_sub(u16::from(has_sb));
        let cursor = self.cursor();
        let show_meta = row_w >= 40;
        for (k, i) in self.scroll.visible_range().enumerate() {
            let y = inner.y + k as u16;
            let row = &self.rows[i];
            let rid = TREE.child(i);
            let mut s = ctx.state(rid);
            s.focused = focused && cursor == Some(i);
            s.selected = cursor == Some(i);
            s.disabled = false;
            let st = t.row(s, bg);
            let rect = Rect::new(inner.x - 1, y, row_w + 1, 1);
            fill(buf, rect, st);
            buf.set_string(
                rect.x,
                y,
                t.gutter_symbol(s),
                t.gutter(s, st.bg.unwrap_or(bg), false),
            );
            let mut x = rect.x + 2 + row.depth * 2;
            let glyph = if row.expandable {
                if row.expanded { "▾" } else { "▸" }
            } else if row.star {
                "★"
            } else {
                " "
            };
            let gs = if row.star {
                st.fg(if s.focused {
                    t.text_primary
                } else {
                    t.text_secondary
                })
            } else {
                st.fg(t.text_secondary).remove_modifier(Modifier::BOLD)
            };
            buf.set_string(x, y, glyph, gs);
            if row.expandable {
                ctx.clickable(TREE.child(i).sub("toggle"), Rect::new(x, y, 2, 1));
            }
            x += 2;
            let meta_text = if show_meta {
                row.meta.text(w)
            } else {
                String::new()
            };
            let meta_w = width(&meta_text) as u16;
            let avail = rect.right().saturating_sub(x + 1);
            let hw: u16 = if row.health.is_some() { 2 } else { 0 };
            let lw = avail.saturating_sub(if meta_w > 0 { meta_w + 2 } else { 0 } + hw);
            let label_style = if row.faint {
                st.fg(t.text_muted)
            } else if row.sel == Sel::Add {
                st.fg(if s.focused {
                    t.text_primary
                } else {
                    t.text_secondary
                })
            } else if s.selected {
                st.fg(t.accent)
            } else {
                st
            };
            buf.set_string(
                x,
                y,
                fit(&truncate(&row.label, lw as usize), lw as usize),
                label_style,
            );
            if let Some((g, tone)) = row.health {
                buf.set_string(x + lw, y, g, st.fg(t.tone(tone)));
            }
            if meta_w > 0 && meta_w + 4 < avail {
                let ms = if row.faint {
                    st.fg(t.text_muted)
                } else {
                    st.fg(t.tone(row.meta_tone))
                };
                buf.set_string(
                    rect.right().saturating_sub(meta_w + 1),
                    y,
                    &meta_text,
                    ms.remove_modifier(Modifier::BOLD),
                );
            }

            legible_on_fill(buf, rect, t, bg);
            ctx.clickable(rid, rect);
            if row.expandable {
                ctx.clickable(
                    TREE.child(i).sub("toggle"),
                    Rect::new(rect.x + 2 + row.depth * 2, y, 2, 1),
                );
            }
        }
        if has_sb {
            scrollbar::render_with_fade(
                Rect::new(inner.x, inner.y, inner.width, inner.height),
                buf,
                ctx,
                TREE,
                &self.scroll,
                focused,
            );
        }
    }

    fn inspector_lines(&self, w: &World, width_: u16) -> (String, String, Vec<Line>) {
        let mut lines: Vec<Line> = vec![];
        match &self.selected {
            Sel::Overview => {
                let s = OverallSummary::compute(&w.accounts.accounts);
                lines.push(Line::PropSpans("Health".into(), super::health_spans(&s)));
                lines.push(Line::Prop("Registry".into(), s.counts_line(), Tone::Normal));
                let registered = w.accounts.accounts.iter().filter(|a| a.origin == AccountOrigin::Registered).count();
                lines.push(Line::Prop(
                    "Sources".into(),
                    format!(
                        "{registered} registered · {} discovered · 1Password {}",
                        w.accounts.accounts.len() - registered,
                        match w.op.session() {
                            crate::backend::OpSession::SignedIn => "available",
                            crate::backend::OpSession::Locked => "locked",
                        }
                    ),
                    Tone::Normal,
                ));
                lines.push(Line::Blank);
                lines.push(Line::Heading("Comparable windows".into(), "identical provider, window and unit only".into()));
                if s.comparable.is_empty() {
                    lines.push(Line::Text("Nothing comparable: no provider has two accounts with the same window".into(), Tone::Muted));
                }
                for c in &s.comparable {
                    let mut text = format!("{} · {}   {} · {}–{}% remaining", c.surface.label(), c.label, plural(c.accounts, "account", "accounts"), c.min_remaining_pct, c.max_remaining_pct);
                    if let Some((u, l)) = c.summed {
                        text.push_str(&format!(" · {} / {} {}", junie_tui::ui::text::thousands(u as usize), junie_tui::ui::text::thousands(l as usize), c.unit.label()));
                    }
                    if c.last_good_count > 0 {
                        text.push_str(&format!(" ({} last good)", c.last_good_count));
                    }
                    lines.push(Line::Text(text, Tone::Secondary));
                }
                if !s.not_comparable.is_empty() {
                    lines.push(Line::Blank);
                    lines.push(Line::Heading("Not comparable".into(), String::new()));
                    let names: Vec<String> = s.not_comparable.iter().map(|n| format!("{} ({})", n.surface.label(), n.reason)).collect();
                    for l in junie_tui::ui::text::wrap(&names.join(" · "), width_ as usize) {
                        lines.push(Line::Text(l, Tone::Muted));
                    }
                }
                lines.push(Line::Blank);
                lines.push(Line::Heading("Warnings".into(), String::new()));
                let mut any = false;
                for a in w.sorted_accounts() {
                    if !a.enabled {
                        continue;
                    }
                    for win in &a.usage.windows {
                        match win.status {
                            QuotaStatus::Warning => {
                                any = true;
                                lines.push(Line::Status(StatusKind::Warn, vec![
                                    Span::new(format!("{}   ", a.title()), Tone::Normal),
                                    Span::new(format!("{} {}% used", win.label, win.used_pct.unwrap_or(0)), Tone::Warning),
                                    Span::new(win.reset_secs.map(|r| format!(" · {}", w.clock.reset_label(r))).unwrap_or_default(), Tone::Muted),
                                ]));
                            }
                            QuotaStatus::Exhausted => {
                                any = true;
                                lines.push(Line::Status(StatusKind::Fail, vec![
                                    Span::new(format!("{}   ", a.title()), Tone::Normal),
                                    Span::new(format!("{} exhausted", win.label), Tone::Error),
                                    Span::new(win.reset_secs.map(|r| format!(" · {}", w.clock.reset_label(r))).unwrap_or_default(), Tone::Muted),
                                ]));
                            }
                            _ => {}
                        }
                    }
                    if let Some(i) = &a.issue
                        && !i.is_informational()
                        && i.code != IssueCode::QuotaUnsupported
                    {
                        any = true;
                        lines.push(Line::Status(StatusKind::Fail, vec![
                            Span::new(format!("{}   ", a.title()), Tone::Normal),
                            Span::new(w.surfaces.vocabulary.reason(&i.reason), Tone::Error),
                        ]));
                    } else if a.usage.freshness.phase == Freshness::Stale {
                        any = true;
                        lines.push(Line::Status(StatusKind::Warn, vec![
                            Span::new(format!("{}   ", a.title()), Tone::Normal),
                            Span::new("stale", Tone::Warning),
                            Span::new(format!(" · last good {}", a.usage.freshness.last_good_secs.map(|s| w.clock.ago(s)).unwrap_or("?".into())), Tone::Muted),
                        ]));
                    }
                }
                if !any {
                    lines.push(Line::Text("No warnings".into(), Tone::Muted));
                }
                let unresolved: Vec<&Account> = w.accounts.accounts.iter().filter(|a| a.enabled && a.identity.subject.is_none()).collect();
                if !unresolved.is_empty() {
                    lines.push(Line::Blank);
                    lines.push(Line::Heading("Unresolved identity".into(), String::new()));
                    for a in unresolved {
                        lines.push(Line::Text(format!("{} · {} · {} — not an authenticated account", a.title(), a.source.origin_label().to_lowercase(), a.confidence.label()), Tone::Muted));
                    }
                }
                ("Overview".into(), format!("{} · {}", plural(w.accounts.accounts.len(), "account", "accounts"), plural(s.counts.providers, "provider", "providers")), lines)
            }
            Sel::Provider(surface) => {
                let accounts: Vec<&Account> = w.surface_accounts(*surface);
                let p = surface.provider();
                lines.push(Line::Prop("Provider".into(), p.map(|p| p.label()).unwrap_or("—").into(), Tone::Normal));
                lines.push(Line::Prop("Agent runtime".into(), p.and_then(|p| p.agent()).map(|a| a.label()).unwrap_or("none").into(), Tone::Normal));
                lines.push(Line::Prop("Usage surface".into(), surface.surface_name().into(), Tone::Normal));
                lines.push(Line::Prop(
                    "Registration".into(),
                    if p.and_then(|p| p.agent()).is_some_and(|a| a.registerable()) {
                        "manual accounts allowed · 1Password, local folder or plain-text key".into()
                    } else {
                        "discovered only · read-only projection".into()
                    },
                    Tone::Secondary,
                ));
                if let Some(d) = accounts.iter().find(|a| a.default_for_provider) {
                    lines.push(Line::Prop("Default".into(), format!("★ {}", d.display_name), Tone::Normal));
                }
                lines.push(Line::Blank);
                if *surface == UsageSurface::Unsupported {
                    lines.push(Line::Text("Unsupported is the registry's explicit sentinel: a capability with no provider adapter. It is never synthesized as zero usage.".into(), Tone::Muted));
                } else if accounts.is_empty() {
                    lines.push(Line::Text("No accounts for this provider".into(), Tone::Muted));
                }
                for a in accounts {
                    lines.push(Line::Heading(a.display_name.clone(), account_meta(a, w).0.text(w)));
                    for win in a.usage.windows.iter().take(2) {
                        if win.has_meter() {
                            lines.push(Line::Meter(win.label.clone(), win.used_pct.unwrap_or(0), meter_detail(win, w), meter_tone(win, a)));
                        } else {
                            lines.push(Line::Text(format!("{}   {}", win.label, win.value_label()), Tone::Muted));
                        }
                    }
                }
                (surface.surface_name().into(), format!("provider · {}", plural(w.accounts.by_provider(p.unwrap_or(Provider::Anthropic)).count(), "account", "accounts")), lines)
            }
            Sel::Account(id) => {
                let Some(a) = w.accounts.get(id) else {
                    return ("Account".into(), String::new(), lines);
                };
                let ws_users: Vec<String> = w
                    .workspaces
                    .iter()
                    .filter(|ws| ws.effective_accounts(&w.accounts).iter().any(|e| &e.id == id))
                    .map(|ws| ws.name.clone())
                    .collect();
                lines.push(Line::Prop("Provider".into(), a.provider.label().into(), Tone::Normal));
                lines.push(Line::Prop("Agent runtime".into(), a.agent.map(|x| x.label()).unwrap_or("none").into(), Tone::Normal));
                lines.push(Line::Prop("Usage surface".into(), a.surface.surface_name().into(), Tone::Normal));
                lines.push(Line::Prop("Identity".into(), a.identity.label(), if a.identity.subject.is_some() { Tone::Normal } else { Tone::Muted }));
                lines.push(Line::Prop("Plan".into(), a.identity.plan.clone().unwrap_or("unknown".into()), Tone::Normal));
                lines.push(Line::Prop("Credential".into(), format!("{} · {}", a.source.origin_label(), a.source.safe_detail()), Tone::Normal));
                match &a.source {
                    CredentialSource::OnePassword(r) => {
                        lines.push(Line::Prop(String::new(), format!("{} · {} · {}", r.account, r.canonical(), w.op.describe(r).map(|d| d.masked).unwrap_or("••••••••".into())), Tone::Muted));
                    }
                    CredentialSource::PlainApiKey { tail, .. } => lines.push(Line::Prop(String::new(), masked(tail), Tone::Muted)),
                    CredentialSource::LocalFolder { detected, .. } | CredentialSource::HostEnv { detected, .. } => {
                        lines.push(Line::Prop(String::new(), format!("detected {}", detected.label()), Tone::Muted));
                    }
                }
                if let Some(e) = &a.endpoint {
                    lines.push(Line::Prop("Endpoint".into(), format!("{} · {}", e.label, e.host), Tone::Normal));
                }
                lines.push(Line::Prop(
                    "Provenance".into(),
                    format!(
                        "{} · confidence {} · {}",
                        a.provenance.iter().map(|p| p.label()).collect::<Vec<_>>().join(", "),
                        a.confidence.label(),
                        a.origin_label()
                    ),
                    Tone::Secondary,
                ));
                lines.push(Line::Prop("Lifecycle".into(), a.lifecycle.label().into(), if a.lifecycle == Lifecycle::Available { Tone::Normal } else { Tone::Warning }));
                lines.push(Line::Prop(
                    "Default".into(),
                    if a.default_for_provider { format!("★ for {}", a.provider.short()) } else { "no".into() },
                    Tone::Normal,
                ));
                lines.push(Line::Prop("Enabled".into(), if a.enabled { "yes".into() } else { "no · disabled".into() }, if a.enabled { Tone::Normal } else { Tone::Muted }));
                if let Some(p) = &a.purpose {
                    lines.push(Line::Prop("Purpose".into(), p.clone(), Tone::Normal));
                }
                if !ws_users.is_empty() {
                    lines.push(Line::Prop("Used by".into(), format!("Workspace choice in {}", ws_users.join(", ")), Tone::Secondary));
                }
                lines.push(Line::Blank);
                let fresh = match a.usage.freshness.phase {
                    Freshness::Current => format!("current · refreshed {}", a.last_refresh_secs.map(|s| w.clock.ago(s)).unwrap_or("never".into())),
                    Freshness::Stale => format!("stale · last good {}", a.usage.freshness.last_good_secs.map(|s| w.clock.ago(s)).unwrap_or("?".into())),
                    Freshness::Refreshing => "refreshing…".into(),
                    Freshness::Failed => format!("failed · last good {}", a.usage.freshness.last_good_secs.map(|s| w.clock.ago(s)).unwrap_or("never".into())),
                };
                lines.push(Line::Heading("Quota".into(), fresh));
                if a.usage.windows.is_empty() {
                    lines.push(Line::Text("No quota windows · not started or unavailable".into(), Tone::Muted));
                }
                for win in &a.usage.windows {
                    if win.has_meter() {
                        lines.push(Line::Meter(win.label.clone(), win.used_pct.unwrap_or(0), meter_detail(win, w), meter_tone(win, a)));
                    } else {
                        let value = win.value_label();
                        let text = if value.to_lowercase().starts_with(&win.label.to_lowercase()) { value } else { format!("{}   {value}", win.label) };
                        lines.push(Line::Text(text, if win.status == QuotaStatus::Error { Tone::Error } else { Tone::Muted }));
                    }
                }
                lines.push(Line::Blank);
                let (m, i, q) = validation_marks(a);
                lines.push(Line::Prop("Validation".into(), format!("{m} material   {i} identity   {q} quota access · {}", a.validation.label(&w.surfaces.vocabulary)), Tone::Normal));
                if let Some(issue) = &a.issue {
                    let tone = if issue.is_informational() || issue.code == IssueCode::QuotaUnsupported { Tone::Warning } else { Tone::Error };
                    let mut text = w.surfaces.vocabulary.reason(&issue.reason);
                    if let Some(d) = &issue.detail {
                        text.push_str(&format!(" · {d}"));
                    }
                    if let Some(r) = issue.retry_secs {
                        text.push_str(&format!(" · retry {}", w.clock.reset_label(r).replace("resets", "")));
                    }
                    lines.push(Line::Prop("Status".into(), format!("{} · {}", text, issue.recoverability.label()), tone));
                }
                if a.origin == AccountOrigin::Discovered {
                    lines.push(Line::Text("read-only: discovered on host · refresh only".into(), Tone::Muted));
                }
                (a.title(), format!("account · {}", a.status_word()), lines)
            }
            Sel::Add => (
                "Add account".into(),
                "form".into(),
                vec![
                    Line::Text("Register a Claude Code, Codex, Grok Build or OpenCode account.".into(), Tone::Secondary),
                    Line::Text("1Password item/field references are the primary credential path; a local agent folder or a masked plain-text key are the alternatives.".into(), Tone::Muted),
                ],
            ),
        }
    }

    fn draw_inspector(
        &mut self,
        area: Rect,
        buf: &mut Buffer,
        ctx: &mut RenderCtx,
        w: &World,
        as_drawer: bool,
    ) {
        let t = ctx.theme;
        let focused = ctx.interaction.focused(INSPECTOR)
            || self.actions.iter().any(|b| ctx.interaction.focused(b.id));
        self.rebuild_actions(w);
        let (title, scope, lines) = self.inspector_lines(w, area.width.saturating_sub(6));
        let inner = if as_drawer {
            Panel::framed(Some(&title))
                .focused(focused)
                .meta(&scope)
                .render(area, buf, t)
        } else {
            Panel::card(Some(&title))
                .focused(focused)
                .focus_mark(if ctx.interaction.focused(INSPECTOR) {
                    PanelFocus::Bar
                } else {
                    PanelFocus::Title
                })
                .meta(&scope)
                .render(area, buf, t)
        };
        let bg = if as_drawer { t.canvas } else { t.surface };
        if inner.is_empty() {
            return;
        }

        let action_row = ButtonRow::new().overflow(RowOverflow::Wrap);
        let action_rows = action_row.rows(inner.width + 1, &self.actions, t);
        let body_h = inner.height.saturating_sub(1 + action_rows);
        let mut body = Rect::new(inner.x, inner.y, inner.width, body_h);

        let label_w: u16 = lines
            .iter()
            .map(|l| match l {
                Line::Prop(k, _, _) | Line::PropSpans(k, _) => width(k) as u16,
                Line::Meter(k, _, _, _) => width(k) as u16,
                _ => 0,
            })
            .max()
            .unwrap_or(13)
            .clamp(13, 22)
            + 1;
        let wrap_lines = |body: Rect| {
            let mut rendered: Vec<Line> = vec![];
            for l in lines.iter().cloned() {
                match l {
                    Line::Prop(k, v, tone) => {
                        let vw = body.width.saturating_sub(label_w) as usize;
                        let parts = junie_tui::ui::text::wrap(&v, vw.max(8));
                        for (i, p) in parts.into_iter().enumerate() {
                            rendered.push(Line::Prop(
                                if i == 0 { k.clone() } else { String::new() },
                                p,
                                tone,
                            ));
                        }
                    }
                    Line::Text(v, tone) => {
                        for p in junie_tui::ui::text::wrap(&v, body.width as usize) {
                            rendered.push(Line::Text(p, tone));
                        }
                    }
                    Line::PropSpans(k, spans) => {
                        let vw = body.width.saturating_sub(label_w) as usize;
                        for (i, p) in wrap_spans(&spans, vw.max(8)).into_iter().enumerate() {
                            rendered.push(Line::PropSpans(
                                if i == 0 { k.clone() } else { String::new() },
                                p,
                            ));
                        }
                    }
                    other => rendered.push(other),
                }
            }
            rendered
        };
        let mut rendered = wrap_lines(body);

        if rendered.len() > body_h as usize {
            body.width = inner
                .width
                .saturating_sub(scrollbar::reserve(scrollbar::GAP));
            rendered = wrap_lines(body);
        }
        self.inspector_scroll.set_content(rendered.len());
        self.inspector_scroll.set_viewport(body_h as usize);
        ctx.control(INSPECTOR, body, false);
        ctx.scrollable(INSPECTOR, body);
        let meter_w = if inner.width >= 70 {
            30
        } else if inner.width >= 50 {
            20
        } else {
            14
        };
        for (k, i) in self.inspector_scroll.visible_range().enumerate() {
            let y = body.y + k as u16;
            match &rendered[i] {
                Line::Prop(label, value, tone) => {
                    buf.set_string(body.x, y, label, t.muted().bg(bg));
                    buf.set_string(
                        body.x + label_w,
                        y,
                        junie_tui::ui::reveal::cut(
                            Rect::new(body.x + label_w, y, body.width.saturating_sub(label_w), 1),
                            value,
                        ),
                        Style::new().fg(t.tone(*tone)).bg(bg),
                    );
                }
                Line::Text(text, tone) => {
                    buf.set_string(
                        body.x,
                        y,
                        junie_tui::ui::reveal::cut(Rect::new(body.x, y, body.width, 1), text),
                        Style::new().fg(t.tone(*tone)).bg(bg),
                    );
                }
                Line::PropSpans(label, spans) => {
                    buf.set_string(body.x, y, label, t.muted().bg(bg));
                    render_spans(
                        buf,
                        body.x + label_w,
                        y,
                        body.width.saturating_sub(label_w),
                        t,
                        spans,
                        bg,
                    );
                }
                Line::Status(kind, spans) => {
                    draw_status_line(buf, body.x, y, body.width, t, *kind, spans, bg);
                }
                Line::Heading(h, meta) => {
                    buf.set_string(
                        body.x,
                        y,
                        h,
                        t.secondary().bg(bg).add_modifier(Modifier::BOLD),
                    );
                    let room = body.width.saturating_sub(width(h) as u16 + 3) as usize;
                    let meta = truncate(meta, room);
                    let mw = width(&meta) as u16;
                    if mw > 0 && room >= 8 {
                        buf.set_string(body.right() - mw, y, &meta, t.faint().bg(bg));
                    }
                }
                Line::Meter(label, pct, value, tone) => {
                    quota_row(label, *pct, value, *tone)
                        .label_width(label_w - 1)
                        .meter_width(meter_w + 6)
                        .reserve(2)
                        .render(Rect::new(body.x, y, body.width, 1), buf, ctx, bg);
                }
                Line::Blank => {}
            }
        }
        if self.inspector_scroll.overflows() {
            scrollbar::render_with_fade(
                Rect::new(inner.x, body.y, inner.width, body_h),
                buf,
                ctx,
                INSPECTOR,
                &self.inspector_scroll,
                ctx.interaction.focused(INSPECTOR),
            );
        }
        let ay = inner.bottom().saturating_sub(action_rows);
        action_row.render(
            INSPECTOR,
            Rect::new(inner.x - 1, ay, inner.width + 1, action_rows),
            &mut self.actions,
            buf,
            ctx,
            bg,
        );
    }

    fn draw_summary(&mut self, area: Rect, buf: &mut Buffer, ctx: &mut RenderCtx, w: &World) {
        let t = ctx.theme;
        let bg = t.surface;
        let (title, scope, lines) = self.inspector_lines(w, area.width.saturating_sub(4));
        let inner = Panel::card(Some(&title))
            .meta(&format!("{scope} · Tab details"))
            .render(area, buf, t);
        let mut y = inner.y;
        for l in lines.iter().take(inner.height as usize) {
            match l {
                Line::Prop(k, v, tone) if !k.is_empty() => {
                    buf.set_string(
                        inner.x,
                        y,
                        junie_tui::ui::reveal::cut(
                            Rect::new(inner.x, y, inner.width, 1),
                            &format!("{k}  {v}"),
                        ),
                        Style::new().fg(t.tone(*tone)).bg(bg),
                    );
                    y += 1;
                }
                Line::Meter(label, pct, value, tone) => {
                    quota_row(label, *pct, value, *tone)
                        .label_width(12)
                        .meter_width(22)
                        .render(Rect::new(inner.x, y, inner.width, 1), buf, ctx, bg);
                    y += 1;
                }
                Line::Text(v, tone) => {
                    buf.set_string(
                        inner.x,
                        y,
                        junie_tui::ui::reveal::cut(Rect::new(inner.x, y, inner.width, 1), v),
                        Style::new().fg(t.tone(*tone)).bg(bg),
                    );
                    y += 1;
                }
                Line::PropSpans(k, spans) => {
                    let lead = format!("{k}  ");
                    buf.set_string(
                        inner.x,
                        y,
                        junie_tui::ui::reveal::cut(Rect::new(inner.x, y, inner.width, 1), &lead),
                        t.primary().bg(bg),
                    );
                    let lw = width(&lead) as u16;
                    render_spans(
                        buf,
                        inner.x + lw,
                        y,
                        inner.width.saturating_sub(lw),
                        t,
                        spans,
                        bg,
                    );
                    y += 1;
                }
                Line::Status(kind, spans) => {
                    draw_status_line(buf, inner.x, y, inner.width, t, *kind, spans, bg);
                    y += 1;
                }
                _ => {}
            }
            if y >= inner.bottom() {
                break;
            }
        }
        ctx.clickable(INSPECTOR, area);
    }
}

#[derive(Clone)]
enum Line {
    Prop(String, String, Tone),

    PropSpans(String, Vec<Span>),
    Text(String, Tone),

    Status(StatusKind, Vec<Span>),
    Heading(String, String),
    Meter(String, u8, String, QuotaTone),
    Blank,
}

#[allow(clippy::too_many_arguments)]
fn draw_status_line(
    buf: &mut Buffer,
    x: u16,
    y: u16,
    w: u16,
    t: &junie_tui::theme::Theme,
    kind: StatusKind,
    spans: &[Span],
    bg: ratatui::style::Color,
) {
    let (glyph, gs) = t.status(kind);
    let gw = width(glyph) as u16 + 1;
    if w <= gw {
        return;
    }
    buf.set_string(x, y, glyph, gs.bg(bg));
    render_spans(buf, x + gw, y, w - gw, t, spans, bg);
}

pub fn meter_detail(win: &QuotaWindow, w: &World) -> String {
    let mut parts: Vec<String> = vec![];
    if let (Some(u), Some(l)) = (win.used, win.limit)
        && win.unit != WindowUnit::Percent
    {
        parts.push(format!(
            "{} / {} {}",
            thousands(u as usize),
            thousands(l as usize),
            win.unit.label()
        ));
    }
    if let Some(r) = win.reset_secs {
        parts.push(w.clock.reset_label(r));
    }
    if let Some(s) = &win.spend_label {
        parts.push(s.clone());
    }
    if win.status == QuotaStatus::Exhausted {
        parts.push("exhausted".into());
    }
    parts.join(" · ")
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct QuotaTone {
    pub tone: MeterTone,
    pub stale: bool,
}

impl QuotaTone {
    pub fn on(self, m: Meter) -> Meter {
        m.tone(self.tone).stale(self.stale)
    }
}

pub(super) fn quota_row(label: &str, pct: u8, detail: &str, tone: QuotaTone) -> MeterList {
    MeterList::new(vec![
        MeterRow::new(
            label,
            tone.on(Meter::new(Some(pct)))
                .value(format!("{pct:>3}%"))
                .visual(MeterVisual::Block),
        )
        .detail(detail),
    ])
}

pub fn quota_tone(win: &QuotaWindow, phase: Freshness) -> QuotaTone {
    let tone = match (phase, win.status) {
        (Freshness::Refreshing, _) => MeterTone::Refreshing,
        (_, QuotaStatus::Exhausted) => MeterTone::Exhausted,
        (_, QuotaStatus::Warning) => MeterTone::Warning,
        (_, QuotaStatus::Available | QuotaStatus::NotStarted) => MeterTone::Fixed(MeterLevel::Low),
        _ => MeterTone::Normal,
    };
    QuotaTone {
        tone,
        stale: matches!(phase, Freshness::Stale | Freshness::Failed),
    }
}

pub(super) fn meter_tone(win: &QuotaWindow, a: &Account) -> QuotaTone {
    quota_tone(win, a.usage.freshness.phase)
}

fn validation_marks(a: &Account) -> (&'static str, &'static str, &'static str) {
    use crate::domain::account::ValidationLevel as L;
    match &a.validation {
        ValidationState::Valid(L::QuotaReadable) => ("✓", "✓", "✓"),
        ValidationState::Valid(L::IdentityAuthenticated) => (
            "✓",
            "✓",
            if a.issue
                .as_ref()
                .is_some_and(|i| i.code == IssueCode::QuotaUnsupported)
            {
                "—"
            } else {
                "▲"
            },
        ),
        ValidationState::Valid(L::MaterialDiscovered) => ("✓", "—", "—"),
        ValidationState::Invalid(_) => ("!", "—", "—"),
        ValidationState::Validating { .. } => ("⠋", "…", "…"),
        ValidationState::NeverValidated => ("—", "—", "—"),
    }
}

fn account_meta(a: &Account, _w: &World) -> (RowMeta, Tone) {
    if !a.enabled {
        return (RowMeta::plain("disabled"), Tone::Faint);
    }
    match a.usage.freshness.phase {
        Freshness::Refreshing => (RowMeta::spinner("", "refreshing"), Tone::Secondary),
        Freshness::Stale => (
            RowMeta::timed(
                "stale ",
                Moment::AgeOr(a.usage.freshness.last_good_secs, "?"),
                "",
            ),
            Tone::Warning,
        ),
        Freshness::Failed => (
            RowMeta::plain(match a.issue.as_ref().map(|i| i.code) {
                Some(IssueCode::RateLimited) => "rate limited",
                Some(IssueCode::ProviderUnavailable) => "unavailable",
                Some(IssueCode::CredentialFileMissing) => "needs secret",
                Some(IssueCode::Unauthorized) | Some(IssueCode::ApiKeyInvalid) => "unauthorized",
                Some(IssueCode::OpLocked) => "1Password locked",
                _ => "error",
            }),
            Tone::Error,
        ),
        Freshness::Current => match a.lifecycle {
            Lifecycle::Unsupported => (RowMeta::plain("unsupported"), Tone::Muted),
            Lifecycle::NeedsLogin => (RowMeta::plain("needs login"), Tone::Warning),
            Lifecycle::NeedsSecret => (RowMeta::plain("needs secret"), Tone::Warning),
            _ => (
                match a.last_refresh_secs {
                    Some(secs) => RowMeta::timed("current ", Moment::AgeBare(secs), ""),
                    None => RowMeta::plain("current now"),
                },
                Tone::Muted,
            ),
        },
    }
}

impl Account {
    fn origin_label(&self) -> &'static str {
        match self.origin {
            AccountOrigin::Registered => "registered",
            AccountOrigin::Discovered => "discovered",
        }
    }
}

impl Page<Jackin> for AccountsScreen {
    fn enter(&mut self, w: &mut World, cx: &mut Cx) {
        self.ensure_rows(w);
        self.md.close_drawer(cx.focus);
    }

    fn primary_focus(&self) -> Option<WidgetId> {
        Some(TREE)
    }

    fn animating(&self, w: &World) -> bool {
        w.accounts.accounts.iter().any(|a| {
            a.usage.freshness.phase == Freshness::Refreshing
                || matches!(a.validation, ValidationState::Validating { .. })
        })
    }

    fn on_tick(&mut self, w: &mut World, _cx: &mut Cx) -> Outcome {
        self.ensure_rows(w);
        if self.animating(w) || self.clock_due(w) {
            Outcome::Changed
        } else {
            Outcome::Ignored
        }
    }

    fn on_msg(&mut self, msg: &Msg, w: &mut World, cx: &mut Cx) -> Outcome {
        match msg {
            Msg::AccountsSaved {
                ticket,
                added,
                result,
            } => {
                let current = match self.saving.take() {
                    Some((current, saving)) if current == *ticket => Some(saving),
                    other => {
                        self.saving = other;
                        None
                    }
                };
                match (result, current) {
                    (Ok(saved), current) => {
                        w.apply((*saved.snapshot).clone());
                        if let Some(saving) = current {
                            let status = saving.status.clone();
                            self.finish_saving(saving, added.clone(), cx);
                            if let Some(warning) = saved.warning {
                                cx.status(format!("{} · {status}", warning.status()));
                            }
                        }
                    }
                    (
                        Err(error),
                        Some(Saving {
                            form: Some(serial), ..
                        }),
                    ) if self.form.as_ref().is_some_and(|c| c.serial == serial) => {
                        let error = error.clone();
                        cx.with_top::<FormDialog>(move |f| f.error = Some(error));
                    }
                    (Err(error), Some(_)) => cx.error(error.clone()),
                    (Err(_), None) => {}
                }
                self.ensure_rows(w);
                Outcome::Changed
            }
            Msg::AccountRefreshed {
                account,
                source,
                result,
                ..
            } => {
                if let Some(a) = w.accounts.get(account).filter(|a| a.source == **source) {
                    match result {
                        Ok(refresh) => {
                            let n = a.usage.windows.iter().filter(|w| w.has_meter()).count();
                            cx.status(format!(
                                "Refreshed {} · {} · {}",
                                a.title(),
                                w.surfaces.vocabulary.refresh_outcome(refresh.outcome),
                                plural(n, "window", "windows")
                            ));
                        }
                        Err(error) => cx.error(error.clone()),
                    }
                }
                self.ensure_rows(w);
                Outcome::Changed
            }
            Msg::AccountValidated {
                ticket,
                account,
                result,
                ..
            } => {
                if w.validation_owns_status(account, *ticket) {
                    self.show_validation(account, result, w, cx);
                }
                self.ensure_rows(w);
                Outcome::Changed
            }
            Msg::SourceChecked { ticket, result } => self.source_checked(*ticket, result, w, cx),
            Msg::FolderProbed { ticket, result } => self.folder_probed(*ticket, result, w, cx),
            _ => Outcome::Ignored,
        }
    }

    fn on_key(&mut self, key: &Key, w: &mut World, cx: &mut Cx) -> Outcome {
        self.ensure_rows(w);
        if self.menu.is_open() {
            let (o, chosen) = self.menu.on_key(key);
            match chosen {
                Some((target, cmd)) => return self.run_menu(target, cmd, w, cx),

                None if o == Outcome::Ignored => {}
                None => return o,
            }
        }
        let focus = cx.focus.current();
        let in_tree = focus == Some(TREE);
        if in_tree
            && self.menu.is_context_key(key)
            && let Some(i) = self.cursor()
        {
            let dy = i.saturating_sub(self.scroll.offset) as u16;
            let fallback = Rect::new(
                self.tree_area.x,
                self.tree_area.y + dy.min(self.tree_area.height.saturating_sub(1)),
                self.tree_area.width.max(1),
                1,
            );
            let anchor = ContextAnchor::Control {
                id: TREE.child(i),
                fallback,
            };
            self.open_menu(i, anchor, w);
            return Outcome::Changed;
        }
        let in_insp = focus == Some(INSPECTOR);
        let action_focus = self.actions.iter().position(|b| Some(b.id) == focus);
        if action_focus.is_none() {
            match key.code {
                KeyCode::Char('a') if key.plain() => {
                    self.open_form(None, w, cx);
                    return Outcome::Changed;
                }
                KeyCode::Char('r') if key.plain() => {
                    self.refresh_scope(w, cx);
                    return Outcome::Changed;
                }
                KeyCode::F(5) => {
                    let prev = self.selected.clone();
                    self.selected = Sel::Overview;
                    self.refresh_scope(w, cx);
                    self.selected = prev;
                    return Outcome::Changed;
                }
                KeyCode::Char('/') if key.plain() => {
                    let input = TextInput::new(WidgetId::of("accounts.filter.input"), "Filter")
                        .placeholder("name, provider, identity, status, plan")
                        .value(self.filter.as_deref().unwrap_or(""))
                        .plain_label();
                    let d = Dialog::prompt(
                        WidgetId::of("accounts.filter"),
                        "Filter accounts",
                        input,
                        "Apply",
                    );
                    cx.open(d, LayerTag::new("filter"));
                    return Outcome::Changed;
                }
                KeyCode::Char('e') if key.plain() => {
                    if let Some(a) = self.selected_account(w).cloned() {
                        self.edit_account(&a, w, cx);
                        return Outcome::Changed;
                    }
                }
                KeyCode::Char('t') if key.plain() => {
                    if let Sel::Account(id) = self.selected.clone() {
                        self.toggle_enabled(&id, w, cx);
                        return Outcome::Changed;
                    }
                }
                KeyCode::Char('d') | KeyCode::Delete if key.plain() => {
                    if let Sel::Account(id) = self.selected.clone() {
                        self.remove(&id, w, cx);
                        return Outcome::Changed;
                    }
                }
                KeyCode::Char('v') if key.plain() => {
                    if let Sel::Account(id) = self.selected.clone() {
                        self.validate_account(&id, w, cx);
                        return Outcome::Changed;
                    }
                }
                KeyCode::Char('m') if key.plain() => {
                    let sel = match &self.selected {
                        Sel::Account(id) => Some(id.clone()),
                        _ => None,
                    };
                    cx.go(Go::Usage { select: sel });
                    return Outcome::Changed;
                }
                _ => {}
            }
        }
        if in_tree {
            match key.code {
                KeyCode::Up | KeyCode::Char('k') => {
                    self.move_cursor(-1);
                    return Outcome::Changed;
                }
                KeyCode::Down | KeyCode::Char('j') => {
                    self.move_cursor(1);
                    return Outcome::Changed;
                }
                KeyCode::PageUp => {
                    self.move_cursor(-(self.scroll.viewport_len.max(1) as isize));
                    return Outcome::Changed;
                }
                KeyCode::PageDown => {
                    self.move_cursor(self.scroll.viewport_len.max(1) as isize);
                    return Outcome::Changed;
                }
                KeyCode::Home | KeyCode::Char('g') => {
                    self.move_cursor(-(self.rows.len() as isize));
                    return Outcome::Changed;
                }
                KeyCode::End | KeyCode::Char('G') => {
                    self.move_cursor(self.rows.len() as isize);
                    return Outcome::Changed;
                }
                KeyCode::Right | KeyCode::Char('l') => {
                    if let Sel::Provider(s) = self.selected {
                        if self.folded.remove(&s) {
                        } else {
                            self.move_cursor(1);
                        }
                        self.ensure_rows(w);
                    }
                    return Outcome::Changed;
                }
                KeyCode::Left | KeyCode::Char('h') => {
                    match self.selected.clone() {
                        Sel::Provider(s) => {
                            self.folded.insert(s);
                        }
                        Sel::Account(id) => {
                            if let Some(a) = w.accounts.get(&id) {
                                self.selected = Sel::Provider(a.surface);
                            }
                        }
                        _ => {}
                    }
                    self.ensure_rows(w);
                    return Outcome::Changed;
                }
                KeyCode::Char('*') => {
                    self.folded.clear();
                    self.ensure_rows(w);
                    return Outcome::Changed;
                }
                KeyCode::Char('-') => {
                    for s in UsageSurface::ALL {
                        self.folded.insert(s);
                    }
                    self.ensure_rows(w);
                    return Outcome::Changed;
                }
                KeyCode::Char(' ') => {
                    match self.selected.clone() {
                        Sel::Provider(s) => {
                            if !self.folded.remove(&s) {
                                self.folded.insert(s);
                            }
                            self.ensure_rows(w);
                        }
                        Sel::Account(id) => self.set_default(&id, w, cx),
                        _ => {}
                    }
                    return Outcome::Changed;
                }
                KeyCode::Enter => {
                    match self.selected.clone() {
                        Sel::Add => self.open_form(None, w, cx),
                        Sel::Provider(s) => {
                            if !self.folded.remove(&s) {
                                self.folded.insert(s);
                            }
                            self.ensure_rows(w);
                        }
                        _ => self.md.open_drawer(cx.focus),
                    }
                    return Outcome::Changed;
                }
                KeyCode::Tab => {
                    self.md.open_drawer(cx.focus);
                    return Outcome::Changed;
                }
                KeyCode::Esc => {
                    if self.filter.is_some() {
                        self.filter = None;
                        self.ensure_rows(w);
                        cx.status("Filter cleared");
                        return Outcome::Changed;
                    }
                    cx.go(Go::Manager);
                    return Outcome::Changed;
                }
                _ => {}
            }
        }
        if in_insp {
            match key.code {
                KeyCode::Up | KeyCode::Char('k') => {
                    self.inspector_scroll.scroll_by(-1);
                    return Outcome::Changed;
                }
                KeyCode::Down | KeyCode::Char('j') => {
                    self.inspector_scroll.scroll_by(1);
                    return Outcome::Changed;
                }
                KeyCode::PageUp => {
                    self.inspector_scroll.page_up();
                    return Outcome::Changed;
                }
                KeyCode::PageDown => {
                    self.inspector_scroll.page_down();
                    return Outcome::Changed;
                }
                KeyCode::Esc | KeyCode::Left | KeyCode::BackTab => {
                    self.md.close_drawer(cx.focus);
                    return Outcome::Changed;
                }
                KeyCode::Tab | KeyCode::Enter => {
                    if let Some(b) = self.actions.first() {
                        cx.focus.focus(b.id);
                    }
                    return Outcome::Changed;
                }
                _ => {}
            }
        }
        if let Some(i) = action_focus {
            let (o, fired) = self.actions[i].on_key(key);
            if fired {
                let id = self.actions[i].id;
                for n in [
                    "refresh", "validate", "edit", "default", "toggle", "remove", "add",
                ] {
                    if INSPECTOR.sub(n) == id {
                        return self.fire(n, w, cx);
                    }
                }
            }
            if o.consumed() {
                return o;
            }
            match key.code {
                KeyCode::Left | KeyCode::Char('h') => {
                    if i > 0 {
                        cx.focus.focus(self.actions[i - 1].id);
                    } else {
                        cx.focus.focus(INSPECTOR);
                    }
                    return Outcome::Changed;
                }
                KeyCode::Right | KeyCode::Char('l') => {
                    if i + 1 < self.actions.len() {
                        cx.focus.focus(self.actions[i + 1].id);
                    }
                    return Outcome::Changed;
                }
                KeyCode::Esc => {
                    self.md.close_drawer(cx.focus);
                    return Outcome::Changed;
                }
                _ => {}
            }
        }
        Outcome::Ignored
    }

    fn pointer(&mut self, ev: &PointerEvent, w: &mut World, cx: &mut Cx) -> Outcome {
        if let Some(o) = self.md.on_pointer(ev) {
            return o;
        }
        let (id, pos) = (ev.id, ev.pos());
        if let Some((axis, delta)) = ev.wheel() {
            return match axis {
                Axis::Vertical => self.wheel(id, delta),
                Axis::Horizontal => Outcome::Ignored,
            };
        }
        if let Some(o) =
            scrollbar::pointer(ev, TREE, &mut self.scroll, TrackClick::Jump).or_else(|| {
                scrollbar::pointer(ev, INSPECTOR, &mut self.inspector_scroll, TrackClick::Jump)
            })
        {
            return o;
        }
        match ev.phase() {
            Some(Phase::Click) => self.click(id, w, cx),
            Some(Phase::Secondary) => self.secondary(id, pos, w, cx),
            _ => Outcome::Ignored,
        }
    }

    fn on_layer_result(
        &mut self,
        tag: &LayerTag,
        result: ModalResult,
        w: &mut World,
        cx: &mut Cx,
    ) -> Outcome {
        match (tag.kind, result) {
            ("form", ModalResult::FormAction(name, values)) => {
                match name.as_str() {
                    "choose:op" => {
                        self.cancel_check(cx);
                        let flow = OpFlow::new(WidgetId::of("accounts.op"), w.op.as_ref());
                        cx.open(flow, LayerTag::new("op"));
                    }
                    "choose:browse" => {
                        self.cancel_check(cx);
                        let b = super::modals::file_browser(
                            WidgetId::of("accounts.browser"),
                            "Choose the local agent folder",
                            &w.home,
                            false,
                            true,
                            w,
                        );
                        cx.open(b, LayerTag::new("browse"));
                    }
                    "plain" => {
                        self.cancel_check(cx);
                        let plain = source_index(w, SourceKind::PlainApiKey).unwrap_or(0);
                        cx.with_top::<FormDialog>(move |f| {
                            if let Some(field) = f.field_mut("source")
                                && let super::modals::FieldKindW::Radio(r) = &mut field.kind
                            {
                                r.selected = plain;
                                r.cursor = plain;
                            }
                        });
                        cx.with_top::<FormDialog>(|f| {
                            f.set_visible("op", false);
                            f.set_visible("op_note", false);
                            f.set_visible("key", true);
                            f.set_visible("folder", false);
                            f.set_visible("browse", false);
                            if let Some((_, b)) = f.actions.iter_mut().find(|(n, _)| n == "plain") {
                                b.disabled = true;
                            }
                        });
                        cx.status("Plain text chosen · the key is masked and never stored");
                    }
                    "validate" => self.start_check(&values, Then::Show, w, cx),
                    "save" => self.save_form(values, w, cx),
                    _ => {}
                }
                Outcome::Changed
            }
            ("form", ModalResult::Form(None)) => {
                self.form = None;
                cx.status("Cancelled · nothing saved");
                cx.focus.focus(TREE);
                Outcome::Changed
            }
            ("form", ModalResult::Form(Some(values))) => {
                self.save_form(values, w, cx);
                Outcome::Changed
            }
            ("op", ModalResult::Op(Some(r))) => {
                let masked =
                    w.op.describe(&r)
                        .map(|d| d.masked)
                        .unwrap_or("••••••••".into());
                let path = r.display_path();
                let meta = format!("{} · {} · {}", r.account, r.canonical(), masked);
                if let Some(ctx) = self.form.as_mut() {
                    ctx.op_ref = Some(r.clone());
                    ctx.op_masked = Some(masked.clone());
                    ctx.validation = None;
                }
                let endpoint = w.op.endpoint_of(&r);
                let caveat = w.surfaces.account_form.op_note.clone();
                let saved_note = w.surfaces.vocabulary.op_reference_note.clone();
                cx.with_top::<FormDialog>(move |f| {
                    f.set_chooser("op", &path, None);
                    let mut lines = vec![(meta, Tone::Muted), (saved_note, Tone::Faint)];
                    lines.extend(caveat.map(|n| (n, Tone::Muted)));
                    f.set_note("op_note", lines);
                    f.set_note("validation", vec![]);
                    if let Some(e) = endpoint {
                        f.set_text(
                            "endpoint",
                            e.trim_start_matches("https://").trim_end_matches("/v1"),
                        );
                    }
                });
                cx.status("Reference chosen · Validate checks provider compatibility");
                Outcome::Changed
            }
            ("op", ModalResult::Op(None)) => Outcome::Changed,
            ("browse", ModalResult::Browser(BrowserResult::Chosen { path, .. })) => {
                let tilde = w.tilde(&path);
                cx.with_top::<FormDialog>(move |f| f.set_text("folder", &tilde));
                Outcome::Changed
            }
            (
                "remove",
                ModalResult::Dialog {
                    action: Some(1), ..
                },
            ) => {
                let mut registry = w.accounts.clone();
                if let Some(a) = registry.remove(&tag.key) {
                    self.save_registry(
                        registry,
                        Saving {
                            status: format!("Removed {}", a.title()),
                            select: Pick::Row(Sel::Provider(a.surface)),
                            unfold: None,
                            form: None,
                        },
                        w,
                        cx,
                    );
                }
                self.ensure_rows(w);
                Outcome::Changed
            }
            (
                "filter",
                ModalResult::Dialog {
                    action: Some(1),
                    text,
                },
            ) => {
                self.filter = text.filter(|t| !t.trim().is_empty());
                self.ensure_rows(w);
                Outcome::Changed
            }
            (_, ModalResult::Cancelled)
            | (
                _,
                ModalResult::Dialog {
                    action: Some(0), ..
                },
            ) => {
                if tag.kind == "remove" {
                    cx.status("Cancelled · account kept");
                }
                Outcome::Changed
            }
            _ => Outcome::Changed,
        }
    }

    fn render(&mut self, area: Rect, buf: &mut Buffer, ctx: &mut RenderCtx, w: &World) {
        self.ensure_rows(w);
        self.clock.painted(w);
        let t = ctx.theme;
        self.md.detail_owns = self.actions.iter().map(|b| b.id).collect();
        let l = self.md.layout(area, buf, ctx, t.canvas);
        if !l.master.is_empty() {
            self.draw_tree(l.master, buf, ctx, w);
        }
        if l.mode != Mode::Split {
            if l.mode == Mode::Drawer {
                self.draw_inspector(l.detail, buf, ctx, w, true);
            } else {
                self.draw_summary(l.summary, buf, ctx, w);
            }
            self.md.finish(ctx);
            self.menu.render(area, buf, ctx);
            return;
        }
        let right = l.detail;
        self.draw_inspector(right, buf, ctx, w, false);
        if w.accounts.accounts.is_empty() && matches!(self.selected, Sel::Overview) {
            let e = EmptyState::new("No accounts registered").hint("a adds one · 1Password is the default credential source · discovered sources appear after a refresh");
            let inner = Rect::new(
                right.x + 2,
                right.y + 3,
                right.width.saturating_sub(4),
                right.height.saturating_sub(6),
            );
            fill(buf, inner, Style::new().bg(t.surface));
            empty::render(inner, buf, t, &e, t.surface);
        }

        self.menu.render(area, buf, ctx);
    }

    fn hints(&self, focus: Option<WidgetId>, w: &World) -> Vec<Hint> {
        if self.menu.is_open() {
            return junie_tui::widgets::row_menu::hints();
        }
        if focus == Some(INSPECTOR) {
            return vec![
                hint("↑↓", "Scroll"),
                hint("Tab", "Actions"),
                hint("Esc", "Back"),
            ];
        }
        if let Some(b) = self.actions.iter().find(|b| Some(b.id) == focus) {
            let mut v = vec![hint("←→", "Choose")];
            if b.can_activate() {
                v.push(b.hint());
            }
            v.push(hint("Esc", "Back"));
            return v;
        }
        let mut v = vec![];
        match &self.selected {
            Sel::Account(id) => {
                let account = w.accounts.get(id);
                let reg = account.is_some_and(|a| a.mutations_allowed());
                let editable = account.is_some_and(|a| a.edit_allowed());
                let enabled = account.is_some_and(|a| a.enabled);
                v.push(hint("Enter", "Details"));
                v.push(hint("r", "Refresh"));
                if editable {
                    v.push(hint("e", "Edit…"));
                }
                if reg {
                    v.push(hint("Space", "Default"));
                    if w.surfaces.fixed.is_none() {
                        v.push(junie_tui::widgets::keyhint::toggle_hint(
                            "t", enabled, "Disable", "Enable",
                        ));
                    }
                    v.push(hint("v", "Validate"));
                    v.push(hint("d", "Remove…"));
                }
            }
            Sel::Provider(_) => {
                v.push(hint("Space", "Fold"));
                v.push(hint("r", "Refresh provider"));
            }
            Sel::Overview => {
                v.push(hint("Enter", "Details"));
                v.push(hint("r", "Refresh all"));
            }
            Sel::Add => v.push(hint("Enter", "Add account…")),
        }
        v.push(hint("a", "Add…"));
        v.push(hint("/", "Filter"));
        v.push(hint("m", "Usage"));
        v.push(hint("Esc", "Back"));
        v
    }

    fn crumb(&self, w: &World) -> String {
        match &self.selected {
            Sel::Overview => "Accounts › Overview".into(),
            Sel::Provider(s) => format!("Accounts › {}", s.surface_name()),
            Sel::Account(id) => match w.accounts.get(id) {
                Some(a) => format!(
                    "Accounts › {} › {}",
                    a.surface.surface_name(),
                    a.display_name
                ),
                None => "Accounts".into(),
            },
            Sel::Add => "Accounts › new account".into(),
        }
    }

    fn is_editing(&self) -> bool {
        false
    }
}

impl Screen for AccountsScreen {
    fn layer_open(&self) -> bool {
        self.menu.hides_page_focus()
    }

    fn form_changed(&mut self, tag: &LayerTag, form: &mut FormDialog, w: &World) {
        if tag.kind == "form" {
            Self::reveal(form, w);
            form.error = None;
            if let Some(ctx) = self.form.as_mut() {
                ctx.validation = None;
                ctx.check = None;
            }
            end_pending(form);
        }
    }

    fn reply_arrives(&mut self, tag: &LayerTag, form: &mut FormDialog, m: &Msg) {
        let (Msg::SourceChecked { ticket, .. } | Msg::FolderProbed { ticket, .. }) = m else {
            return;
        };
        if tag.kind != "form" {
            return;
        }
        if let Some(ctx) = self.form.as_mut()
            && ctx
                .check
                .as_ref()
                .is_some_and(|c| c.ticket == *ticket && c.request != form.values())
        {
            ctx.check = None;
            end_pending(form);
        }
    }

    fn strip_right(&self, w: &World) -> Vec<Segment> {
        let n = w
            .accounts
            .accounts
            .iter()
            .filter(|a| a.usage.freshness.phase == Freshness::Refreshing)
            .count();
        if n > 0 {
            vec![
                Segment::new(
                    format!("{} refreshing {n}", spinner_frame(w.now_ms() as u64 / 80)),
                    Tone::Secondary,
                )
                .priority(6),
            ]
        } else {
            vec![]
        }
    }
}

impl AccountsScreen {
    fn click(&mut self, id: WidgetId, w: &mut World, cx: &mut Cx) -> Outcome {
        self.ensure_rows(w);
        if self.menu.is_open() {
            let (o, chosen) = self.menu.on_click(id);
            return match chosen {
                Some((target, cmd)) => self.run_menu(target, cmd, w, cx),
                None => o,
            };
        }
        for i in self.scroll.visible_range() {
            if TREE.child(i).sub("toggle") == id {
                if let Sel::Provider(s) = self.rows[i].sel {
                    self.selected = Sel::Provider(s);
                    if !self.folded.remove(&s) {
                        self.folded.insert(s);
                    }
                    self.ensure_rows(w);
                }
                cx.focus.focus(TREE);
                return Outcome::Changed;
            }
            if TREE.child(i) == id {
                let same = self.selected == self.rows[i].sel;
                self.selected = self.rows[i].sel.clone();
                self.inspector_scroll.jump_start();
                self.md.close_drawer(cx.focus);
                if same && self.selected == Sel::Add {
                    self.open_form(None, w, cx);
                }
                return Outcome::Changed;
            }
        }
        if id == INSPECTOR {
            self.md.open_drawer(cx.focus);
            return Outcome::Changed;
        }
        for i in 0..self.actions.len() {
            if self.actions[i].id == id {
                cx.focus.focus(id);
                if self.actions[i].on_click() {
                    for n in [
                        "refresh", "validate", "edit", "default", "toggle", "remove", "add",
                    ] {
                        if INSPECTOR.sub(n) == id {
                            return self.fire(n, w, cx);
                        }
                    }
                }
                return Outcome::Changed;
            }
        }
        Outcome::Ignored
    }

    fn secondary(&mut self, id: WidgetId, pos: Position, w: &mut World, cx: &mut Cx) -> Outcome {
        self.ensure_rows(w);
        let Some(i) = self
            .scroll
            .visible_range()
            .find(|&i| TREE.child(i) == id || TREE.child(i).sub("toggle") == id)
        else {
            return Outcome::Ignored;
        };
        if self.menu.config.moves_cursor && self.selected != self.rows[i].sel {
            self.selected = self.rows[i].sel.clone();
            self.inspector_scroll.jump_start();
        }
        self.md.close_drawer(cx.focus);
        self.open_menu(i, ContextAnchor::Pointer(pos), w);
        Outcome::Changed
    }

    fn wheel(&mut self, id: WidgetId, delta: i32) -> Outcome {
        if id == TREE || id == scrollbar::id_for(TREE) {
            self.scroll.scroll_by(delta as isize);
            return Outcome::Changed;
        }
        if id == INSPECTOR || id == scrollbar::id_for(INSPECTOR) {
            self.inspector_scroll.scroll_by(delta as isize);
            return Outcome::Changed;
        }
        Outcome::Ignored
    }
}
