use ratatui::buffer::Buffer;
use ratatui::layout::{Rect, Size};

use crate::app::PageId;
use crate::options::{PageValues, page_options};
use crate::pages::template::{Section, Template};
use crate::pages::{Cx, CxExt, Hint, Page, PageEvent};
use termrock::core::event::Outcome;
use termrock::core::id::WidgetId;
use termrock::core::pointer::MouseTarget;
use termrock::theme::{StatusKind, Tone};
use termrock::ui::ctx::RenderCtx;
use termrock::ui::layout::{Split, SplitDir};
use termrock::ui::master_detail::{MasterDetail, Mode};
use termrock::ui::measure::Measure;
use termrock::ui::specimen::{SpecimenState, StateGrid};
use termrock::ui::text::{plural, truncate};
use termrock::widgets::button::{Button, row_layout};
use termrock::widgets::dialog::{Dialog, DialogResult};
use termrock::widgets::list::{ListBox, ListItem, SelectMode};
use termrock::widgets::panel::Panel;
use termrock::widgets::props::{self, Prop, PropsList};
use termrock::widgets::segments::{self, Segment};
use termrock::widgets::splitter::Splitter;
use termrock::widgets::tree::{TreeAction, TreeNode, TreeView};

const ID: WidgetId = WidgetId::of("workspaces");

pub const TREE: WidgetId = ID.sub("tree");

pub const DETAIL: WidgetId = ID.sub("detail");

pub const ROSTER: WidgetId = ID.sub("roster");

pub const PURGE: WidgetId = ID.sub("purge");

const LAUNCH_TICKS: u16 = 16;

const PREWARM_TICKS: u16 = 12;

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
enum State {
    Running,
    Starting,
    Preserved { dirty: bool },
    Failed,
}

#[derive(Clone, Debug)]
struct Instance {
    id: &'static str,
    role: &'static str,
    agent: &'static str,
    state: State,
    since: &'static str,
}

#[derive(Clone, Debug)]
struct Workspace {
    name: &'static str,
    dir: &'static str,
    mounts: &'static str,
    roles: &'static str,
    envs: &'static str,
    accounts: &'static str,
    instances: Vec<Instance>,
}

const fn inst(
    id: &'static str,
    role: &'static str,
    agent: &'static str,
    state: State,
    since: &'static str,
) -> Instance {
    Instance {
        id,
        role,
        agent,
        state,
        since,
    }
}

fn data() -> Vec<Workspace> {
    vec![
        Workspace {
            name: "payments-platform",
            dir: "/workspace/payments-platform",
            mounts: "~/src/payments-platform · rw worktree · ~/src/shared-libs · ro shared",
            roles: "the-architect ★ · allowed 3 of 46",
            envs: "5 vars · 2 [op]",
            accounts: "Claude · Personal ★ · Codex · Primary ★",
            instances: vec![
                inst(
                    "7f3a",
                    "the-architect",
                    "Claude Code",
                    State::Running,
                    "2 h 14 m",
                ),
                inst(
                    "c41e",
                    "reviewer",
                    "Codex",
                    State::Preserved { dirty: true },
                    "yesterday",
                ),
            ],
        },
        Workspace {
            name: "infra-control-plane",
            dir: "/workspace/infra",
            mounts: "~/src/infra · rw worktree",
            roles: "sre ★ · allowed 2 of 46",
            envs: "9 vars · 4 [op]",
            accounts: "Codex · Experiments",
            instances: vec![
                inst("9b02", "sre", "Codex", State::Running, "40 m"),
                inst(
                    "04d7",
                    "sre",
                    "Claude Code",
                    State::Preserved { dirty: false },
                    "3 days ago",
                ),
            ],
        },
        Workspace {
            name: "release-automation",
            dir: "/workspace/release",
            mounts: "~/src/release-tools · rw worktree",
            roles: "backend ★",
            envs: "3 vars",
            accounts: "Claude · Work",
            instances: vec![inst(
                "a1c0",
                "backend",
                "Claude Code",
                State::Failed,
                "restore failed",
            )],
        },
        Workspace {
            name: "customer-portal",
            dir: "/workspace/customer-portal",
            mounts: "~/src/customer-portal · rw worktree",
            roles: "frontend ★ · allowed 1 of 46",
            envs: "2 vars",
            accounts: "Grok · Team ★",
            instances: vec![],
        },
    ]
}

impl State {
    fn glyph(self) -> (&'static str, Tone) {
        match self {
            State::Running | State::Starting => ("◉", Tone::Active),
            State::Preserved { .. } => ("◌", Tone::Muted),
            State::Failed => ("!", Tone::Error),
        }
    }

    fn short(self, since: &str) -> &str {
        match self {
            State::Running => since,
            State::Starting => "starting",
            State::Preserved { dirty: true } => "dirty •",
            State::Preserved { dirty: false } => "kept",
            State::Failed => "failed",
        }
    }
    fn label(self, since: &str) -> String {
        match self {
            State::Running => format!("running {since}"),
            State::Starting => "starting…".into(),
            State::Preserved { dirty: true } => "preserved · dirty •".into(),
            State::Preserved { dirty: false } => "preserved".into(),
            State::Failed => format!("! {since}"),
        }
    }
}

fn running(ws: &Workspace) -> usize {
    ws.instances
        .iter()
        .filter(|i| matches!(i.state, State::Running | State::Starting))
        .count()
}

fn ws_meta(ws: &Workspace) -> String {
    let n = running(ws);
    if ws.instances.iter().any(|i| i.state == State::Failed) {
        "! failed".into()
    } else if n > 0 {
        format!("{n} running")
    } else if ws.instances.is_empty() {
        "idle".into()
    } else {
        "preserved".into()
    }
}

const BESIDE: u16 = 84;

const ROSTER_MIN: u16 = 40;

fn facts_width(inner: u16) -> u16 {
    inner * 11 / 20
}

pub fn page() -> Box<dyn Page> {
    Box::new(WorkspacesPage::new())
}

pub struct WorkspacesPage {
    template: Template,
    opts: PageValues,
    data: Vec<Workspace>,
    md: MasterDetail,
    tree: TreeView,
    facts: PropsList,

    facts_for: Option<usize>,
    roster: ListBox,
    launch: Button,
    edit: Button,
    prewarm: Button,
    delete: Button,

    launching: Option<(usize, usize, u16)>,

    prewarming: u16,
}

impl WorkspacesPage {
    pub fn new() -> Self {
        let mut p = Self {
            template: Template::new(ID, states_grid()),
            opts: PageValues::new(page_options(PageId::Workspaces)),
            data: data(),
            md: MasterDetail::new(TREE, DETAIL),
            tree: TreeView::new(TREE, vec![]),
            facts: PropsList::new(DETAIL, vec![]),
            facts_for: None,
            roster: ListBox::new(ROSTER, vec![], SelectMode::Single),
            launch: Button::primary(ID.sub("launch"), "Launch"),
            edit: Button::secondary(ID.sub("edit"), "Edit"),
            prewarm: Button::secondary(ID.sub("prewarm"), "Prewarm"),
            delete: Button::danger(ID.sub("delete"), "Delete…"),
            launching: None,
            prewarming: 0,
        };
        p.tree.expanded.insert(vec![0]);
        p.sync_data();
        p.rebuild();
        p
    }

    fn rebuild(&mut self) {
        let o = &self.opts;
        let md = MasterDetail::new(TREE, DETAIL)
            .collapse_below(o.number("workspaces.collapse_below").unwrap_or(80))
            .esc_returns(o.on("workspaces.esc_returns"))
            .summary_rows(1)
            .detail_owns(&[
                ROSTER,
                self.launch.id,
                self.edit.id,
                self.prewarm.id,
                self.delete.id,
            ])
            .seam(Splitter::new(TREE.sub("seam"), SplitDir::Horizontal));
        self.md = match o.value("workspaces.size") {
            "share" => md.detail_share(62, 40, 90),
            _ => md.resizable(Split::new(46, 30, 40)),
        };
    }

    fn sync_data(&mut self) {
        let nodes = self
            .data
            .iter()
            .enumerate()
            .map(|(wi, ws)| {
                let children = ws
                    .instances
                    .iter()
                    .enumerate()
                    .map(|(ii, i)| {
                        let (g, tone) = i.state.glyph();
                        let mut n = TreeNode::leaf_meta(
                            &format!("{} {}", i.id, i.role),
                            i.state.short(i.since),
                        )
                        .glyph(g)
                        .glyph_tone(tone);
                        n.busy = self.launching.is_some_and(|(w, k, _)| (w, k) == (wi, ii));
                        if i.state == State::Failed {
                            n = n.label_tone(Tone::Error);
                        }
                        n
                    })
                    .collect::<Vec<_>>();
                if children.is_empty() {
                    TreeNode::dir(
                        ws.name,
                        vec![TreeNode::note("No instances · Launch starts one")],
                    )
                    .meta(&ws_meta(ws))
                } else {
                    TreeNode::dir(ws.name, children).meta(&ws_meta(ws))
                }
            })
            .collect();
        self.tree.nodes = nodes;
        self.tree.flatten();
        let items = self
            .data
            .iter()
            .flat_map(|ws| {
                ws.instances.iter().map(move |i| {
                    let (g, _) = i.state.glyph();
                    ListItem::new(&format!(
                        "{g} {}  {} · {} · {}",
                        i.id, ws.name, i.role, i.agent
                    ))
                    .meta(&i.state.label(i.since))
                    .meta_tone(match i.state {
                        State::Failed => Tone::Error,
                        State::Running | State::Starting => Tone::Secondary,
                        _ => Tone::Muted,
                    })
                })
            })
            .collect();
        self.roster.items = items;
        self.roster.cursor = self
            .roster
            .cursor
            .min(self.roster.items.len().saturating_sub(1));
        let n = self.roster.items.len();
        self.roster.scroll.set_content(n);
        self.facts_for = None;
    }

    fn current(&self) -> Option<usize> {
        self.tree.cursor_path().and_then(|p| p.first().copied())
    }

    fn facts(&self, wi: usize) -> Vec<Prop> {
        let ws = &self.data[wi];
        vec![
            Prop::new("Working dir", ws.dir).copyable(),
            Prop::new("Mounts", ws.mounts).wrap(),
            Prop::new("Roles", ws.roles).wrap(),
            Prop::new("Environments", ws.envs),
            Prop::new("Accounts", ws.accounts).wrap(),
            Prop::new(
                "Policies",
                "git pull enabled · keep awake on · dirty exit ask",
            )
            .wrap(),
            Prop::new(
                "Instances",
                format!(
                    "{} · {} running",
                    plural(ws.instances.len(), "instance", "instances"),
                    running(ws)
                ),
            ),
        ]
    }

    fn roster_on(&self) -> bool {
        self.opts.value("workspaces.roster") != "off"
    }

    fn detail_panel(&self, wi: usize) -> (Panel<'static>, String) {
        let ws = &self.data[wi];
        (
            Panel::framed(Some(ws.name)),
            format!("saved workspace · {}", ws_meta(ws)),
        )
    }

    fn detail_height(&self, wi: usize, width: u16) -> u16 {
        let (panel, _) = self.detail_panel(wi);
        let inner = panel.inner(Size::new(width, 0)).width;
        let facts = self.facts(wi);
        let roster = if self.roster_on() {
            self.roster.items.len() as u16 + 1
        } else {
            0
        };
        let body = if roster > 0 && inner >= BESIDE {
            props::measure(&facts, facts_width(inner)).max(roster)
        } else if roster > 0 {
            props::measure(&facts, inner) + 1 + roster
        } else {
            props::measure(&facts, inner)
        };
        panel.outer(Size::new(inner, body + 2)).height
    }

    fn render_playground(&mut self, area: Rect, buf: &mut Buffer, ctx: &mut RenderCtx) {
        let t = ctx.theme;
        let total: usize = self.data.iter().map(running).sum();
        let rows = crate::pages::layout::rows(area, &[1, 1, area.height]);
        segments::render(
            rows[0],
            buf,
            ctx,
            &[
                Segment::new("Workspaces", Tone::Normal).bold().priority(9),
                Segment::new(format!("{} saved", self.data.len()), Tone::Muted).priority(3),
            ],
            &[Segment::new(format!("{total} running"), Tone::Secondary)
                .lead("●", Tone::Active)
                .priority(8)],
            t.canvas,
        );
        let l = self.md.layout(rows[2], buf, ctx, t.canvas);
        if !l.master.is_empty() {
            let meta = format!("{total} running");
            let panel = Panel::framed(Some("Workspaces"))
                .meta(&meta)
                .focused(ctx.interaction.focused(TREE));
            let bg = panel.bg(t);
            let inner = panel.render(l.master, buf, t);
            self.tree.render(inner, buf, ctx, bg);
        }
        let cur = self.current();
        if !l.detail.is_empty()
            && let Some(wi) = cur
        {
            self.render_detail(l.detail, wi, l.mode == Mode::Drawer, buf, ctx);
        }
        if !l.summary.is_empty()
            && let Some(wi) = cur
        {
            let ws = &self.data[wi];
            let line = format!("→ {} · {} · Tab shows details", ws.name, ws_meta(ws),);
            segments::render(
                l.summary,
                buf,
                ctx,
                &[Segment::new(
                    truncate(
                        &line,
                        l.summary.width.saturating_sub(2 * t.spacing.chrome_edge) as usize,
                    ),
                    Tone::Muted,
                )],
                &[],
                t.canvas,
            );
        }
        self.md.finish(ctx);
        if l.detail.is_empty() {
            for id in [
                ROSTER,
                self.launch.id,
                self.edit.id,
                self.prewarm.id,
                self.delete.id,
            ] {
                ctx.focus_stop(id);
            }
        }
    }

    fn render_detail(
        &mut self,
        area: Rect,
        wi: usize,
        drawer: bool,
        buf: &mut Buffer,
        ctx: &mut RenderCtx,
    ) {
        let t = ctx.theme;
        if self.facts_for != Some(wi) {
            self.facts.set_props(self.facts(wi));
            self.facts_for = Some(wi);
        }
        let ws = &self.data[wi];
        let meta = if drawer {
            "Esc back".to_owned()
        } else {
            format!("saved workspace · {}", ws_meta(ws))
        };
        let panel = Panel::framed(Some(ws.name))
            .meta(&meta)
            .focused(ctx.interaction.focused(DETAIL));
        let bg = panel.bg(t);
        let inner = panel.render(area, buf, t);
        let roster_on = self.roster_on();

        let beside = roster_on && inner.width >= BESIDE;
        let body = Rect::new(
            inner.x,
            inner.y,
            inner.width,
            inner.height.saturating_sub(2),
        );
        let (facts, roster) = if !roster_on {
            (body, Rect::ZERO)
        } else if beside {
            let fw = facts_width(inner.width);
            (
                Rect::new(body.x, body.y, fw, body.height),
                Rect::new(body.x + fw + 2, body.y, body.width - fw - 2, body.height),
            )
        } else {
            let fh = props::measure(&self.facts(wi), body.width)
                .min(body.height.saturating_sub(4).max(body.height / 2));
            (
                Rect::new(body.x, body.y, body.width, fh),
                Rect::new(
                    body.x,
                    body.y + fh + 1,
                    body.width,
                    body.height.saturating_sub(fh + 1),
                ),
            )
        };
        self.facts.render(facts, buf, ctx, bg);
        if roster.height >= 3 {
            segments::render(
                Rect::new(roster.x, roster.y, roster.width, 1),
                buf,
                ctx,
                &[Segment::new("Instances", Tone::Secondary).bold()],
                &[Segment::new("daemon · 3 s ago", Tone::Muted)],
                bg,
            );
            let list = Rect::new(roster.x, roster.y + 1, roster.width, roster.height - 1);
            self.roster.render(list, buf, ctx, bg);
        } else {
            ctx.focus_stop(ROSTER);
        }

        self.prewarm.busy = self.prewarming > 0;
        self.launch.busy = self.launching.is_some();
        let y = inner.bottom().saturating_sub(1);
        let rects = row_layout(
            Rect::new(inner.x, y, inner.width, 1),
            &[
                self.launch.width(),
                self.edit.width(),
                self.prewarm.width(),
                self.delete.width(),
            ],
            2,
        );
        self.launch.render(rects[0], buf, ctx, bg);
        self.edit.render(rects[1], buf, ctx, bg);
        self.prewarm.render(rects[2], buf, ctx, bg);
        self.delete.render(rects[3], buf, ctx, bg);
    }

    fn do_launch(&mut self, cx: &mut Cx) {
        let Some(wi) = self.current() else { return };
        if self.launching.is_some() {
            cx.status("A launch is already starting");
            return;
        }
        let ws = &mut self.data[wi];
        let role = ws.roles.split(' ').next().unwrap_or("agent");
        let role: &'static str = match role {
            "the-architect" => "the-architect",
            "sre" => "sre",
            "backend" => "backend",
            _ => "frontend",
        };
        ws.instances
            .push(inst("e5d9", role, "Claude Code", State::Starting, "now"));
        let ii = ws.instances.len() - 1;
        let name = ws.name;
        self.launching = Some((wi, ii, LAUNCH_TICKS));
        self.tree.expanded.insert(vec![wi]);
        self.sync_data();
        cx.status(format!("Launching {name}…"));
    }

    fn do_prewarm(&mut self, cx: &mut Cx) {
        if self.prewarming == 0 {
            self.prewarming = PREWARM_TICKS;
            cx.status("Prewarming the image…");
        }
    }

    fn purge_dialog(&self, wi: usize) -> Dialog {
        let ws = &self.data[wi];
        let token = self.opts.on("workspaces.typed").then_some(ws.name);
        Dialog::facts(
            PURGE,
            &format!("Delete {}?", ws.name),
            vec![
                Prop::new("Workspace", ws.name),
                Prop::new(
                    "Instances",
                    format!(
                        "{} stopped and purged",
                        plural(ws.instances.len(), "instance", "instances")
                    ),
                ),
                Prop::new("Mounts", "kept on disk · only the saved workspace goes").wrap(),
                Prop::new("Recoverable", "no"),
            ],
            vec![],
            token,
            Button::danger(PURGE.sub("go"), "Delete workspace"),
        )
        .title_status(StatusKind::Fail)
    }

    fn purge(&mut self, cx: &mut Cx) {
        let Some(wi) = self.current() else { return };
        let name = self.data[wi].name;
        self.data.remove(wi);
        if let Some((w, _, _)) = self.launching
            && w == wi
        {
            self.launching = None;
        }
        self.tree.expanded.clear();
        self.tree.cursor = 0;
        self.sync_data();
        cx.status(format!("Deleted {name}"));
    }

    fn on_tree(&mut self, a: Option<TreeAction>, cx: &mut Cx) {
        if let Some(TreeAction::Activate(_)) = a {
            self.md.open_drawer(cx.focus);
        }
    }

    fn button_key(&mut self, key: &termrock::core::event::Key, cx: &mut Cx) -> Option<Outcome> {
        let f = cx.focus.current()?;
        let (o, act) = if f == self.launch.id {
            self.launch.on_key(key)
        } else if f == self.edit.id {
            self.edit.on_key(key)
        } else if f == self.prewarm.id {
            self.prewarm.on_key(key)
        } else if f == self.delete.id {
            self.delete.on_key(key)
        } else {
            return None;
        };
        if act {
            self.act(f, cx);
        }
        Some(o)
    }

    fn act(&mut self, id: WidgetId, cx: &mut Cx) {
        if id == self.launch.id {
            self.do_launch(cx);
        } else if id == self.edit.id {
            if let Some(wi) = self.current() {
                cx.status(format!("Editing {}", self.data[wi].name));
            }
        } else if id == self.prewarm.id {
            self.do_prewarm(cx);
        } else if id == self.delete.id
            && let Some(wi) = self.current()
        {
            cx.open_dialog(self.purge_dialog(wi));
        }
    }
}

fn states_grid() -> StateGrid {
    use SpecimenState::*;
    let id = ID.sub("states");
    let mut tree = TreeView::new(
        id.child(0),
        vec![
            TreeNode::dir(
                "payments",
                vec![
                    TreeNode::leaf_meta("7f3a  architect", "running")
                        .glyph("◉")
                        .glyph_tone(Tone::Active),
                    TreeNode::leaf_meta("c41e  reviewer", "dirty •")
                        .glyph("◌")
                        .glyph_tone(Tone::Muted),
                ],
            )
            .meta("1 running"),
            TreeNode::dir("release", vec![]).meta("! failed"),
        ],
    );
    tree.expanded.insert(vec![0]);
    tree.flatten();
    let roster = ListBox::new(
        id.child(1),
        vec![
            ListItem::new("◉ 7f3a  payments").meta("running"),
            ListItem::new("◌ c41e  payments").meta("preserved"),
            ListItem::new("! a1c0  release")
                .meta("failed")
                .meta_tone(Tone::Error),
        ],
        SelectMode::Single,
    );
    StateGrid::new(id)
        .label_width(10)
        .column_width(28)
        .rows(&[Rest, Hover, Focused, Selected, Busy, Narrow])
        .column("Workspace tree", tree)
        .column("Roster", roster)
        .column("Master-detail", MasterDetail::new(id.child(2), id.child(3)))
}

impl Page for WorkspacesPage {
    fn title(&self) -> &'static str {
        "Workspaces"
    }
    fn blurb(&self) -> &'static str {
        "A workspace tree beside its detail card and instance roster; narrow, a drawer"
    }

    fn measure(&self, avail: Size, ctx: &RenderCtx) -> Size {
        let total: usize = self.data.iter().map(running).sum();
        let meta = format!("{total} running");
        let tree = Panel::framed(Some("Workspaces"))
            .meta(&meta)
            .outer(self.tree.measure(avail, ctx));
        let Some(wi) = self.current() else {
            return Size::new(avail.width.max(tree.width), 2 + tree.height);
        };
        let buttons = self.launch.width()
            + self.edit.width()
            + self.prewarm.width()
            + self.delete.width()
            + 3 * 2;
        let roster = if self.roster_on() { ROSTER_MIN } else { 0 };
        let (panel, card_meta) = self.detail_panel(wi);
        let detail = panel
            .meta(&card_meta)
            .outer(Size::new(
                props::width(&self.facts(wi)).max(buttons).max(roster),
                0,
            ))
            .width;
        let panes = self.md.panes_width(tree.width, detail);
        let width = if avail.width < self.md.collapse_below && panes < self.md.collapse_below {
            avail.width.max(tree.width).max(detail)
        } else {
            avail.width.max(panes)
        };
        let height = match self.md.split_rects(Rect::new(0, 0, width, 1)) {
            Some((_, d)) => tree.height.max(self.detail_height(wi, d.width)),

            None if self.md.collapsed_mode(&ctx.interaction) == Mode::Drawer => {
                self.detail_height(wi, width)
            }
            None => tree.height + 2,
        };
        Size::new(width, 2 + height)
    }

    fn render(&mut self, area: Rect, buf: &mut Buffer, ctx: &mut RenderCtx) {
        let body = self.template.render(area, buf, ctx);
        match self.template.section() {
            Section::Playground => self.render_playground(body, buf, ctx),
            Section::Options => self.template.render_options(body, buf, ctx),
            Section::States => self.template.render_states(body, buf, ctx),
        }
    }

    fn animating(&self) -> bool {
        self.launching.is_some() || self.prewarming > 0
    }

    fn handle(&mut self, ev: &PageEvent, cx: &mut Cx) -> Outcome {
        if let PageEvent::Tick = ev {
            let mut changed = false;
            if self.prewarming > 0 {
                self.prewarming -= 1;
                if self.prewarming == 0 {
                    cx.status("Image prewarmed · next launch starts warm");
                }
                changed = true;
            }
            if let Some((wi, ii, left)) = self.launching.as_mut() {
                *left = left.saturating_sub(1);
                if *left == 0 {
                    let (wi, ii) = (*wi, *ii);
                    self.launching = None;
                    if let Some(i) = self.data.get_mut(wi).and_then(|w| w.instances.get_mut(ii)) {
                        i.state = State::Running;
                        i.since = "just now";
                    }
                    self.sync_data();
                    cx.status("Instance e5d9 is running");
                }
                changed = true;
            }
            return if changed {
                Outcome::Changed
            } else {
                Outcome::Ignored
            };
        }
        if let PageEvent::DialogClosed { id, result, .. } = ev {
            if *id != PURGE {
                return Outcome::Ignored;
            }
            if *result == DialogResult::Action(1) {
                self.purge(cx);
            } else {
                cx.status("Nothing deleted");
            }
            return Outcome::Changed;
        }
        if let Some(o) = self.template.handle(ev, cx) {
            return o;
        }
        if self.template.section() != Section::Playground {
            return Outcome::Ignored;
        }
        match ev {
            PageEvent::Key(key) => {
                if let Some(o) = self.md.on_key(key, cx.focus) {
                    return o;
                }
                if cx.focus.is(TREE) {
                    let (o, actions) = self.tree.on_key(key).into_parts();
                    for a in actions {
                        self.on_tree(Some(a), cx);
                    }
                    return o;
                }
                if cx.focus.is(DETAIL) {
                    return self.facts.on_key(key).0;
                }
                if cx.focus.is(ROSTER) {
                    return self.roster.on_key(key);
                }
                self.button_key(key, cx).unwrap_or(Outcome::Ignored)
            }
            PageEvent::Pointer(ev) => {
                if let Some(o) = self.md.on_pointer(ev) {
                    return o;
                }
                if let Some(r) = self.tree.route(ev) {
                    let (o, actions) = r.into_parts();
                    for a in actions {
                        self.on_tree(Some(a), cx);
                    }
                    return o;
                }
                if let Some(r) = self.facts.route(ev) {
                    return r.outcome();
                }
                if let Some(r) = self.roster.route(ev) {
                    return r.outcome();
                }
                for id in [
                    self.launch.id,
                    self.edit.id,
                    self.prewarm.id,
                    self.delete.id,
                ] {
                    let b = [
                        &mut self.launch,
                        &mut self.edit,
                        &mut self.prewarm,
                        &mut self.delete,
                    ]
                    .into_iter()
                    .find(|b| b.id == id);
                    let Some(b) = b else { continue };
                    if let Some(r) = b.route(ev)
                        && r.outcome() != Outcome::Ignored
                    {
                        if r.has_actions() {
                            self.act(id, cx);
                        }
                        return Outcome::Changed;
                    }
                }
                Outcome::Ignored
            }
            _ => Outcome::Ignored,
        }
    }

    fn hints(&self, focus: Option<WidgetId>) -> Vec<Hint> {
        if let Some(h) = self.template.hints(focus) {
            return h;
        }
        let mut v: Vec<Hint> = match focus {
            Some(TREE) => vec![
                ("↑↓", "Move"),
                ("←→", "Fold / open"),
                ("Enter", "Details"),
                ("Tab", "Details"),
            ],
            Some(DETAIL) | Some(ROSTER) => vec![("↑↓", "Move"), ("Tab", "Actions")],
            _ => vec![("Enter", "Activate")],
        };
        for h in self.md.hints(focus) {
            if let (std::borrow::Cow::Borrowed(key), std::borrow::Cow::Borrowed(action)) =
                (h.key, h.action)
            {
                v.push((key, action));
            }
        }
        v
    }

    fn option_value(&self, key: &str) -> Option<usize> {
        self.opts.get(key)
    }

    fn set_option(&mut self, key: &str, value: usize) {
        if self.opts.set(key, value) {
            self.rebuild();
            self.facts_for = None;
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
