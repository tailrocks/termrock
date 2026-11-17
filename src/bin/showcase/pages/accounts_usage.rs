use std::borrow::Cow;

use ratatui::buffer::Buffer;
use ratatui::layout::{Constraint, Rect, Size};

use crate::app::PageId;
use crate::options::{PageValues, page_options};
use crate::pages::template::{Section, Template};
use crate::pages::{Cx, Hint, Page, PageEvent};
use termrock::core::event::Outcome;
use termrock::core::id::WidgetId;
use termrock::core::intent::Phase;
use termrock::core::pointer::MouseTarget;
use termrock::theme::Tone;
use termrock::ui::ctx::RenderCtx;
use termrock::ui::layout::{Split, SplitDir, StackChild};
use termrock::ui::master_detail::{MasterDetail, Mode};
use termrock::ui::measure::Measure;
use termrock::ui::specimen::{SpecimenState, StateGrid};
use termrock::ui::text::truncate;
use termrock::widgets::button::{Button, ButtonRow};
use termrock::widgets::meter_list::{MeterList, MeterRow};
use termrock::widgets::notice::Notice;
use termrock::widgets::panel::Panel;
use termrock::widgets::progress::{Meter, MeterLevel, MeterTone, MeterVisual};
use termrock::widgets::props::{Prop, PropsAction, PropsList};
use termrock::widgets::splitter::Splitter;
use termrock::widgets::table::{Cell, Column, DataTable};
use termrock::widgets::tree::{TreeNode, TreeView};

const ID: WidgetId = WidgetId::of("accountsusage");
const TREE: WidgetId = ID.sub("tree");
const INSPECTOR: WidgetId = ID.sub("inspector");
const REFRESH: WidgetId = ID.sub("refresh");
const DEFAULT: WidgetId = ID.sub("default");
const USAGE: WidgetId = ID.sub("usage");

#[derive(Clone, Copy, PartialEq, Eq)]
enum Health {
    Ok,

    Warning,

    Broken,
    Disabled,
}

struct Account {
    name: &'static str,
    provider: &'static str,
    plan: &'static str,
    credential: &'static str,
    health: Health,

    quotas: &'static [(&'static str, Option<u8>, MeterTone, &'static str)],
    sessions: u32,
    tokens: &'static str,
    cost: &'static str,
}

const PROVIDERS: [&str; 3] = ["Anthropic", "OpenAI", "Google"];

const ACCOUNTS: [Account; 5] = [
    Account {
        name: "Work",
        provider: "Anthropic",
        plan: "Max 20x",
        credential: "1Password · Engineering › Anthropic · Work",
        health: Health::Warning,
        quotas: &[
            (
                "Session (5 h)",
                Some(38),
                MeterTone::Fixed(MeterLevel::Low),
                "resets in 2 h 14 m",
            ),
            ("Weekly", Some(72), MeterTone::Normal, "resets Mon 09:00"),
            (
                "Opus weekly",
                Some(86),
                MeterTone::Warning,
                "provider warns at 85 %",
            ),
        ],
        sessions: 41,
        tokens: "18.2 M",
        cost: "included",
    },
    Account {
        name: "Personal",
        provider: "Anthropic",
        plan: "Pro",
        credential: "local agent folder · ~/.claude",
        health: Health::Ok,
        quotas: &[
            (
                "Session (5 h)",
                Some(12),
                MeterTone::Fixed(MeterLevel::Low),
                "resets in 4 h 02 m",
            ),
            ("Weekly", Some(30), MeterTone::Normal, "resets Thu 18:00"),
        ],
        sessions: 9,
        tokens: "2.4 M",
        cost: "included",
    },
    Account {
        name: "Team",
        provider: "OpenAI",
        plan: "API · usage tier 4",
        credential: "API key · sk-…a91c (plain text, masked)",
        health: Health::Ok,
        quotas: &[(
            "Monthly spend",
            Some(64),
            MeterTone::Normal,
            "$640 of $1,000",
        )],
        sessions: 27,
        tokens: "9.7 M",
        cost: "$212.40",
    },
    Account {
        name: "Research",
        provider: "OpenAI",
        plan: "API · usage tier 2",
        credential: "API key · sk-…07de (1Password)",
        health: Health::Broken,
        quotas: &[(
            "Monthly spend",
            None,
            MeterTone::Unknown,
            "unknown until the key validates",
        )],
        sessions: 0,
        tokens: "—",
        cost: "$0.00",
    },
    Account {
        name: "Vertex",
        provider: "Google",
        plan: "Vertex AI · pay as you go",
        credential: "gcloud ADC · project acme-ml",
        health: Health::Disabled,
        quotas: &[],
        sessions: 0,
        tokens: "—",
        cost: "—",
    },
];

fn account_at(path: &[usize]) -> Option<&'static Account> {
    let [p, a] = path else { return None };
    ACCOUNTS
        .iter()
        .filter(|acc| acc.provider == PROVIDERS[*p])
        .nth(*a)
}

fn tree_nodes() -> Vec<TreeNode> {
    PROVIDERS
        .iter()
        .map(|p| {
            let accounts = ACCOUNTS
                .iter()
                .filter(|a| a.provider == *p)
                .enumerate()
                .map(|(i, a)| {
                    let node = TreeNode::leaf(a.name);
                    match a.health {
                        Health::Broken => {
                            node.meta("key revoked").glyph("!").glyph_tone(Tone::Error)
                        }
                        Health::Disabled => node.meta("disabled").label_tone(Tone::Muted),
                        _ if i == 0 => node.meta("★ default"),
                        _ => node,
                    }
                })
                .collect();
            TreeNode::dir(p, accounts)
        })
        .collect()
}

const INSPECTOR_MIN: u16 = 40;

pub fn page() -> Box<dyn Page> {
    Box::new(AccountsUsagePage::new())
}

pub struct AccountsUsagePage {
    template: Template,
    opts: PageValues,
    md: MasterDetail,
    tree: TreeView,
    facts: PropsList,
    refresh: Button,
    set_default: Button,
    usage: DataTable,

    shown: Vec<usize>,
    sections: crate::pages::layout::Sections,
}

impl AccountsUsagePage {
    pub fn new() -> Self {
        let mut tree = TreeView::new(TREE, tree_nodes());
        tree.expand_all();

        tree.cursor = 1;
        let mut page = Self {
            template: Template::new(ID, states_grid()),
            opts: PageValues::new(page_options(PageId::AccountsUsage)),
            md: MasterDetail::new(TREE, INSPECTOR),
            tree,
            facts: PropsList::new(INSPECTOR, vec![]),
            refresh: Button::secondary(REFRESH, "Refresh"),
            set_default: Button::secondary(DEFAULT, "Set default"),
            usage: usage_table(),
            shown: vec![usize::MAX],
            sections: crate::pages::layout::Sections::new(ID.sub("page")),
        };
        page.rebuild();
        page.sync_inspector();
        page
    }

    fn rebuild(&mut self) {
        self.md = MasterDetail::new(TREE, INSPECTOR)
            .collapse_below(80)
            .detail_owns(&[REFRESH, DEFAULT])
            .seam(Splitter::new(TREE.sub("seam"), SplitDir::Horizontal))
            .summary_rows(1)
            .resizable(Split::new(40, 26, 44));
    }

    fn frame_width(&self, avail: u16) -> u16 {
        self.opts
            .number("accountsusage.width")
            .map_or(avail, |w| w.min(avail))
    }

    fn offline(&self) -> bool {
        self.opts.on("accountsusage.offline")
    }

    fn cursor_path(&self) -> Vec<usize> {
        self.tree
            .cursor_path()
            .map(<[usize]>::to_vec)
            .unwrap_or_default()
    }

    fn sync_inspector(&mut self) {
        let path = self.cursor_path();
        if path == self.shown {
            return;
        }
        self.facts.set_props(match account_at(&path) {
            Some(a) => account_facts(a),
            None => provider_facts(path.first().copied().unwrap_or(0)),
        });
        self.shown = path;
    }

    fn health_line(&self) -> String {
        let attention = ACCOUNTS
            .iter()
            .filter(|a| a.health == Health::Broken)
            .count();
        let usage = if self.offline() {
            "usage offline · last-good 18 m ago"
        } else {
            "usage refreshed 2 m ago"
        };
        format!(
            "{} accounts · {} providers · {attention} needs attention · {usage}",
            ACCOUNTS.len(),
            PROVIDERS.len()
        )
    }

    fn quotas(&self, a: &Account) -> MeterList {
        let visual = match self.opts.value("accountsusage.visual") {
            "line" => MeterVisual::Line,
            _ => MeterVisual::Block,
        };
        let rows = a
            .quotas
            .iter()
            .map(|(label, pct, tone, detail)| {
                let mut m = Meter::new(*pct)
                    .value(pct.map(|p| format!("{p:>3}%")).unwrap_or_default())
                    .tone(*tone)
                    .visual(visual);
                if self.offline() {
                    m = m.stale(true);
                }
                MeterRow::new(*label, m).detail(*detail)
            })
            .collect();
        MeterList::new(rows).meter_width(16).min_meter(8)
    }

    fn warning(&self, a: &Account) -> Option<Notice> {
        if !self.opts.on("accountsusage.warnings") {
            return None;
        }
        match a.health {
            Health::Broken => Some(
                Notice::error("The API key was revoked")
                    .detail("OpenAI answered 401 invalid_api_key at 09:12. Launches with this account stop at Credentials.")
                    .hint("Edit the account to replace the key"),
            ),
            Health::Warning => Some(
                Notice::warning("Opus weekly is at 86 %")
                    .detail("Opus sessions pause at 100 % until Monday 09:00; other models keep working."),
            ),
            Health::Disabled => Some(Notice::info("This account is disabled").detail(
                "Workspaces skip it when they pick an account. Enable it to use it again.",
            )),
            Health::Ok => None,
        }
    }

    fn inspector_height(&self, account: Option<&'static Account>, width: u16) -> u16 {
        let w = width.saturating_sub(2);
        let mut h = 0;
        if let Some(n) = account.and_then(|a| self.warning(a)) {
            h += n.height(w) + 1;
        }
        h += self.facts.props.len() as u16 + 1;
        if let Some(a) = account {
            h += self.quotas(a).height() + 1;
        }
        h + 1
    }

    fn panes(&self, ctx: &RenderCtx) -> (Size, Panel<'static>, u16) {
        let tree = Panel::framed(Some("Accounts"))
            .meta("3 providers")
            .outer(self.tree.measure(Size::ZERO, ctx));
        let buttons = self.refresh.width() + 2 + self.set_default.width();
        let frame = Panel::framed(Some("Accounts")).meta("inspector");

        let inner = (buttons + 2).max(INSPECTOR_MIN);
        let detail = frame.outer(Size::new(inner, 0)).width;
        (tree, frame, detail)
    }

    fn accounts_height(&self, width: u16, ctx: &RenderCtx) -> u16 {
        let (tree, frame, _) = self.panes(ctx);
        let card = Panel::card(Some("Accounts"));
        let inner = card.inner(Size::new(self.frame_width(width), 0)).width;
        let account = account_at(&self.shown);
        let inspector = |w: u16| {
            let r = frame.inner(Size::new(w, 0)).width;
            frame
                .outer(Size::new(r, self.inspector_height(account, r)))
                .height
        };
        let body = match self.md.split_rects(Rect::new(0, 0, inner, 1)) {
            Some((_, d)) => tree.height.max(inspector(d.width)),
            None if self.md.collapsed_mode(&ctx.interaction) == Mode::Drawer => inspector(inner),
            None => tree.height + 2,
        };
        card.outer(Size::new(inner, body)).height.max(14)
    }

    fn render_playground(&mut self, area: Rect, buf: &mut Buffer, ctx: &mut RenderCtx) {
        let t = ctx.theme;
        let focus = if [TREE, INSPECTOR, REFRESH, DEFAULT]
            .iter()
            .any(|&id| ctx.interaction.has_focus(id))
        {
            Some(0)
        } else if ctx.interaction.has_focus(USAGE) {
            Some(1)
        } else {
            None
        };
        let usage_h = ACCOUNTS.len() as u16 + 4;
        let accounts_h = self.accounts_height(area.width, ctx);
        let l = self.sections.layout(
            area,
            &[
                StackChild::flex(accounts_h, accounts_h.max(22)),
                StackChild::fixed(usage_h),
            ],
            focus,
            buf,
            ctx,
        );
        match l.slot(0) {
            Some(r) => self.render_accounts(r, buf, ctx),
            None => {
                for id in [TREE, INSPECTOR, REFRESH, DEFAULT] {
                    ctx.focus_stop(id);
                }
            }
        }
        match l.slot(1) {
            Some(r) => {
                let panel = Panel::card(Some("Usage")).meta("last 7 days");
                let bg = panel.bg(t);
                let inner = panel.render(r, buf, t);
                self.usage.render(inner, buf, ctx, bg);
            }
            None => ctx.focus_stop(USAGE),
        }
    }

    fn render_accounts(&mut self, area: Rect, buf: &mut Buffer, ctx: &mut RenderCtx) {
        self.sync_inspector();
        let t = ctx.theme;
        let w = self.frame_width(area.width);
        let frame = Rect::new(area.x, area.y, w, area.height);
        let health = self.health_line();
        let panel = Panel::card(Some("Accounts")).meta(&health);
        let bg = panel.bg(t);
        let inner = panel.render(frame, buf, t);
        let l = self.md.layout(inner, buf, ctx, bg);
        if !l.master.is_empty() {
            let p = Panel::framed(Some("Accounts"))
                .meta("3 providers")
                .focused(ctx.interaction.focused(TREE));
            let pbg = p.bg(t);
            let r = p.render(l.master, buf, t);
            self.tree.render(r, buf, ctx, pbg);
        }
        let account = account_at(&self.shown);
        if !l.detail.is_empty() {
            let title = match account {
                Some(a) => a.name,
                None => PROVIDERS[self.shown.first().copied().unwrap_or(0).min(2)],
            };
            let p = Panel::framed(Some(title))
                .meta(if l.mode == Mode::Drawer {
                    "Esc back"
                } else {
                    "inspector"
                })
                .focused(ctx.interaction.focused(INSPECTOR));
            let pbg = p.bg(t);
            let r = p.render(l.detail, buf, t);
            self.render_inspector(r, account, buf, ctx, pbg);
        } else {
            for id in [REFRESH, DEFAULT] {
                ctx.focus_stop(id);
            }
        }
        if !l.summary.is_empty() {
            let line = match account {
                Some(a) => format!(
                    "→ {} · {} · {} · Tab shows details",
                    a.name, a.provider, a.plan
                ),
                None => "→ a provider · Tab shows details".to_owned(),
            };
            buf.set_string(
                l.summary.x + 1,
                l.summary.y,
                truncate(&line, l.summary.width.saturating_sub(1) as usize),
                t.muted().bg(bg),
            );
        }
        self.md.finish(ctx);
    }

    fn render_inspector(
        &mut self,
        r: Rect,
        account: Option<&'static Account>,
        buf: &mut Buffer,
        ctx: &mut RenderCtx,
        bg: ratatui::style::Color,
    ) {
        let t = ctx.theme;
        let mut y = r.y;
        let x = r.x + 1;
        let w = r.width.saturating_sub(2);
        if let Some(n) = account.and_then(|a| self.warning(a)) {
            let h = n.height(w).min(r.bottom().saturating_sub(y));
            n.render(Rect::new(x, y, w, h), buf, ctx, bg);
            y += h + 1;
        }
        let meters = account.map(|a| self.quotas(a));
        let meters_h = meters.as_ref().map_or(0, |m| m.height() + 1);

        let facts_h = (self.facts.props.len() as u16)
            .min(r.bottom().saturating_sub(y).saturating_sub(meters_h + 2));
        self.facts
            .render(Rect::new(r.x, y, r.width, facts_h), buf, ctx, bg);
        y += facts_h + 1;
        if let Some(m) = meters
            && y + m.height() < r.bottom()
        {
            m.render(Rect::new(x, y, w, m.height()), buf, ctx, bg);
            y += m.height() + 1;
        }
        self.set_default.disabled = account.is_none_or(|a| {
            a.health == Health::Disabled
                || ACCOUNTS.iter().position(|b| b.name == a.name) == Some(0)
        });
        self.refresh.busy = false;
        let row_y = r
            .bottom()
            .saturating_sub(1)
            .max(y.min(r.bottom().saturating_sub(1)));
        if r.height > 2 && row_y < r.bottom() {
            let buttons = [self.refresh.clone(), self.set_default.clone()];
            let lay = ButtonRow::new().layout(Rect::new(x, row_y, w, 1), &buttons, t);
            for (b, rect) in [&mut self.refresh, &mut self.set_default]
                .into_iter()
                .zip(lay.rects)
            {
                match rect {
                    Some(rect) => b.render(rect, buf, ctx, bg),
                    None => ctx.focus_stop(b.id),
                }
            }
        } else {
            ctx.focus_stop(REFRESH);
            ctx.focus_stop(DEFAULT);
        }
    }

    fn act(&mut self, id: WidgetId, cx: &mut Cx) {
        let path = self.shown.clone();
        let name = account_at(&path)
            .map(|a| a.name)
            .unwrap_or(PROVIDERS[path.first().copied().unwrap_or(0).min(2)]);
        match id {
            REFRESH if self.offline() => {
                cx.status_in("Offline: usage keeps its last-good values", Tone::Warning)
            }
            REFRESH => cx.status(format!("Refreshed usage for {name} ✓")),
            DEFAULT => cx.status(format!("{name} is now the default account")),
            _ => {}
        }
    }

    fn props_action(&mut self, a: Option<PropsAction>, cx: &mut Cx) {
        if let Some(PropsAction::Copy(i)) = a
            && let Some(p) = self.facts.props.get(i)
        {
            cx.copy_as(p.value.clone(), p.label.clone());
        }
    }
}

fn account_facts(a: &Account) -> Vec<Prop> {
    let (status, tone) = match a.health {
        Health::Ok => ("available · validated 2 m ago", Tone::Secondary),
        Health::Warning => ("available · a quota is near its limit", Tone::Warning),
        Health::Broken => ("validation failed · 401 invalid_api_key", Tone::Error),
        Health::Disabled => ("disabled", Tone::Muted),
    };
    vec![
        Prop::new("Provider", a.provider),
        Prop::new("Plan", a.plan),
        Prop::new("Credential", a.credential).copyable(),
        Prop::new("Status", status).tone(tone),
    ]
}

fn provider_facts(p: usize) -> Vec<Prop> {
    let name = PROVIDERS[p.min(2)];
    let n = ACCOUNTS.iter().filter(|a| a.provider == name).count();
    let default = ACCOUNTS
        .iter()
        .find(|a| a.provider == name)
        .map_or("—", |a| a.name);
    vec![
        Prop::new("Provider", name),
        Prop::new("Accounts", n.to_string()),
        Prop::new("Default", format!("★ {default}")),
        Prop::new(
            "Usage surface",
            match name {
                "Anthropic" => "plan windows (session, weekly)",
                "OpenAI" => "monthly spend",
                _ => "none reported",
            },
        ),
    ]
}

fn usage_table() -> DataTable {
    DataTable::try_new(
        USAGE,
        vec![
            Column::new("Account", Constraint::Min(12)),
            Column::new("Provider", Constraint::Length(10)),
            Column::new("Sessions", Constraint::Length(9)).right(),
            Column::new("Tokens", Constraint::Length(8)).right(),
            Column::new("Cost", Constraint::Length(10)).right(),
        ],
        ACCOUNTS
            .iter()
            .map(|a| {
                let muted = a.health == Health::Disabled;
                let tone = |c: Cell| if muted { c.tone(Tone::Muted) } else { c };
                vec![
                    tone(Cell::new(a.name)),
                    Cell::new(a.provider).tone(Tone::Muted),
                    tone(Cell::new(a.sessions.to_string())),
                    tone(Cell::new(a.tokens)),
                    tone(Cell::new(a.cost)),
                ]
            })
            .collect(),
    )
    .expect("rectangular fixture")
    .numeric(&[2, 3, 4])
}

fn states_grid() -> StateGrid {
    use SpecimenState::*;
    StateGrid::new(ID.sub("states"))
        .label_width(11)
        .column_width(34)
        .narrow_width(20)
        .rows(&[Rest, Loading, Error, Overflow, Narrow])
        .column(
            "Quota list",
            MeterList::new(vec![
                MeterRow::new(
                    "Session",
                    Meter::new(Some(38))
                        .value(" 38%")
                        .tone(MeterTone::Fixed(MeterLevel::Low))
                        .visual(MeterVisual::Block),
                )
                .detail("2 h"),
                MeterRow::new(
                    "Opus",
                    Meter::new(Some(86))
                        .value(" 86%")
                        .tone(MeterTone::Warning)
                        .visual(MeterVisual::Block),
                )
                .detail("Mon"),
            ])
            .meter_width(14),
        )
        .column(
            "Account warning",
            Notice::error("The API key was revoked").detail("401 invalid_api_key"),
        )
}

fn page_hint(h: termrock::widgets::keyhint::Hint) -> Hint {
    let key = match h.key {
        Cow::Borrowed(key) => key,
        Cow::Owned(_) => "",
    };
    let action = match h.action {
        Cow::Borrowed(s) => s,
        Cow::Owned(_) => "",
    };
    (key, action)
}

impl Page for AccountsUsagePage {
    fn title(&self) -> &'static str {
        "Accounts & usage"
    }
    fn blurb(&self) -> &'static str {
        "Accounts by provider with an inspector: warnings, facts, quota meters, usage table"
    }

    fn measure(&self, avail: Size, ctx: &RenderCtx) -> Size {
        let (tree, _, detail) = self.panes(ctx);
        let card = Panel::card(Some("Accounts"));
        let frame = match self.opts.number("accountsusage.width") {
            Some(w) => w,
            None => {
                card.outer(Size::new(self.md.natural_width(tree.width, detail), 0))
                    .width
            }
        };
        let usage = Panel::card(Some("Usage"))
            .meta("last 7 days")
            .outer(Size::new(self.usage.min_width(avail, ctx), 0))
            .width;
        let width = avail.width.max(frame).max(usage);
        let usage_h = ACCOUNTS.len() as u16 + 4;
        let height = self.sections.stack.measure(&[
            StackChild::fixed(self.accounts_height(width, ctx)),
            StackChild::fixed(usage_h),
        ]);
        Size::new(width, height)
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
        if let Some(o) = self.sections.handle(ev) {
            return o;
        }
        let o = match ev {
            PageEvent::Key(key) => {
                if let Some(o) = self.md.on_key(key, cx.focus) {
                    return o;
                }
                let Some(f) = cx.focus.current() else {
                    return Outcome::Ignored;
                };
                match f {
                    TREE => self.tree.on_key(key).outcome(),
                    INSPECTOR => {
                        let (o, a) = self.facts.on_key(key);
                        self.props_action(a, cx);
                        o
                    }
                    USAGE => self.usage.on_key(key).outcome(),
                    REFRESH | DEFAULT => {
                        let b = if f == REFRESH {
                            &mut self.refresh
                        } else {
                            &mut self.set_default
                        };
                        let (o, act) = b.on_key(key);
                        if act {
                            self.act(f, cx);
                            return Outcome::Changed;
                        }
                        o
                    }
                    _ => Outcome::Ignored,
                }
            }
            PageEvent::Pointer(ev) => {
                if let Some(o) = self.md.on_pointer(ev) {
                    return o;
                }
                for id in [REFRESH, DEFAULT] {
                    let b = if id == REFRESH {
                        &mut self.refresh
                    } else {
                        &mut self.set_default
                    };
                    if let Some(r) = b.route(ev)
                        && r.outcome() != Outcome::Ignored
                    {
                        if r.has_actions() {
                            cx.focus.focus(id);
                            self.act(id, cx);
                        }
                        return Outcome::Changed;
                    }
                }
                if let Some(r) = self.tree.route(ev) {
                    if ev.is(Phase::Click) {
                        cx.focus.focus(TREE);
                    }
                    r.outcome()
                } else if let Some(r) = self.facts.route(ev) {
                    if ev.is(Phase::Click) {
                        cx.focus.focus(INSPECTOR);
                    }
                    let (o, actions) = r.into_parts();
                    for a in actions {
                        self.props_action(Some(a), cx);
                    }
                    o
                } else if let Some((o, _)) =
                    crate::pages::tables::route_table(&mut self.usage, ev, cx)
                {
                    o
                } else {
                    Outcome::Ignored
                }
            }
            _ => Outcome::Ignored,
        };
        self.sync_inspector();
        o
    }

    fn hints(&self, focus: Option<WidgetId>) -> Vec<Hint> {
        if let Some(h) = self.template.hints(focus) {
            return h;
        }
        let mut v: Vec<Hint> = match focus {
            Some(TREE) => vec![("↑↓", "Move"), ("←→", "Fold"), ("Tab", "Inspector")],
            Some(INSPECTOR) => vec![("↑↓", "Move"), ("y", "Copy")],
            Some(USAGE) => vec![("↑↓", "Move")],
            Some(REFRESH) | Some(DEFAULT) => vec![("Enter", "Activate"), ("Tab", "Next")],
            _ => Vec::new(),
        };
        v.extend(self.md.hints(focus).into_iter().map(page_hint));
        v
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
