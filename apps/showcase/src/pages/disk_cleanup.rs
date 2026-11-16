use ratatui::buffer::Buffer;
use ratatui::layout::{Rect, Size};

use crate::app::PageId;
use crate::options::{PageValues, page_options};
use crate::pages::template::{Section, Template};
use crate::pages::{Cx, CxExt, Hint, Page, PageEvent};
use termrock::core::event::Outcome;
use termrock::core::id::WidgetId;
use termrock::core::pointer::MouseTarget;
use termrock::core::selection::{CheckClick, ClickPolicy};
use termrock::theme::{StatusKind, Tone};
use termrock::ui::ctx::RenderCtx;
use termrock::ui::layout::StackChild;
use termrock::ui::master_detail::MasterDetail;
use termrock::ui::measure::Measure;
use termrock::ui::specimen::{SpecimenState, StateGrid};
use termrock::ui::text::{bytes, plural, truncate};
use termrock::widgets::button::{Button, row_layout};
use termrock::widgets::dialog::{Dialog, DialogResult};
use termrock::widgets::list::{ListBox, ListItem, SelectMode};
use termrock::widgets::meter_list::{MeterList, MeterRow};
use termrock::widgets::notice::Notice;
use termrock::widgets::panel::Panel;
use termrock::widgets::progress::{Meter, MeterTone, MeterVisual};
use termrock::widgets::props::{self, Prop, PropsList};
use termrock::widgets::segments::{self, Segment};
use termrock::widgets::tree::{TreeAction, TreeNode, TreeView};

const ID: WidgetId = WidgetId::of("diskcleanup");

pub const TREE: WidgetId = ID.sub("tree");

pub const DETAILS: WidgetId = ID.sub("details");

pub const CATEGORIES: WidgetId = ID.sub("categories");

pub const GATE1: WidgetId = ID.sub("gate1");
pub const GATE2: WidgetId = ID.sub("gate2");

const GIB: u64 = 1 << 30;
const MIB: u64 = 1 << 20;

const SCAN_TICKS: u16 = 30;

const DISK_TONE: MeterTone = MeterTone::Level { warn: 80, crit: 95 };

struct Category {
    label: &'static str,
    size: u64,
    safety: &'static str,

    checked: bool,

    blocked: Option<&'static str>,
}

const fn cat(label: &'static str, size: u64, safety: &'static str, checked: bool) -> Category {
    Category {
        label,
        size,
        safety,
        checked,
        blocked: None,
    }
}

const CATEGORIES_DATA: [Category; 10] = [
    cat("Xcode DerivedData", 810 * MIB, "rebuilt on demand", true),
    cat(
        "Xcode device support",
        2 * GIB + 410 * MIB,
        "safe if old",
        true,
    ),
    cat("Simulator caches", 900 * MIB, "rebuilt on demand", true),
    cat(
        "Cargo registry cache",
        4 * GIB + 820 * MIB,
        "safe if old",
        true,
    ),
    cat("pnpm store", 2 * GIB + 100 * MIB, "safe if old", false),
    cat("Gradle caches", GIB + 512 * MIB, "safe if old", false),
    cat("npm cache", 201 * MIB, "rebuilt on demand", false),
    cat("Maven repository", 700 * MIB, "review first", false),
    cat("User caches", GIB + 920 * MIB, "review first", false),
    Category {
        label: "Docker build cache",
        size: 6 * GIB + 300 * MIB,
        safety: "in use",
        checked: false,
        blocked: Some("Docker is running"),
    },
];

fn usage() -> Vec<TreeNode> {
    let node = |label: &str, size: u64, total: u64, children: Vec<TreeNode>| {
        let meta = format!("{:>9} · {:>3}%", bytes(size), size * 100 / total.max(1));
        if children.is_empty() {
            TreeNode::leaf_meta(label, &meta)
        } else {
            TreeNode::dir(label, children).meta(&meta)
        }
    };
    let total = 31 * GIB + 820 * MIB;
    vec![node(
        "work",
        total,
        total,
        vec![
            node(
                "backend",
                12 * GIB + 700 * MIB,
                total,
                vec![
                    node("target", 9 * GIB + 300 * MIB, total, vec![]),
                    node("node_modules", 2 * GIB + 100 * MIB, total, vec![]),
                    node("src", 410 * MIB, total, vec![]),
                ],
            ),
            node(
                "frontend",
                8 * GIB + 800 * MIB,
                total,
                vec![
                    node("node_modules", 6 * GIB + 200 * MIB, total, vec![]),
                    node(".next", 2 * GIB + 300 * MIB, total, vec![]),
                ],
            ),
            node(
                "dumps",
                6 * GIB + 300 * MIB,
                total,
                vec![node(
                    "prod-2026-09-30.sql.gz",
                    6 * GIB + 300 * MIB,
                    total,
                    vec![],
                )],
            ),
            node(
                "android",
                4 * GIB,
                total,
                vec![node("build", 3 * GIB + 900 * MIB, total, vec![])],
            ),
            node("backend-wt", 24 * 1024, total, vec![]),
        ],
    )]
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum Mode {
    Trash,
    Permanent,
    DryRun,
}

impl Mode {
    fn label(self) -> &'static str {
        match self {
            Mode::Trash => "Trash · space returns after the Trash is emptied",
            Mode::Permanent => "permanent delete · not recoverable",
            Mode::DryRun => "dry run · nothing is changed",
        }
    }
    fn verb(self) -> &'static str {
        match self {
            Mode::Trash => "Move to Trash",
            Mode::Permanent => "Delete permanently",
            Mode::DryRun => "Run dry run",
        }
    }

    fn phrase(self, n: usize) -> String {
        match self {
            Mode::Trash => format!("TRASH {n}"),
            Mode::Permanent => format!("DELETE {n}"),
            Mode::DryRun => String::new(),
        }
    }
}

pub fn page() -> Box<dyn Page> {
    Box::new(DiskCleanupPage::new())
}

pub struct DiskCleanupPage {
    template: Template,
    opts: PageValues,
    tree: TreeView,
    list: ListBox,

    cats: Vec<usize>,
    rescan: Button,
    review: Button,

    facts: PropsList,
    facts_for: Option<Vec<usize>>,

    usage_md: MasterDetail,
    clean_md: MasterDetail,

    scanning: u16,

    reclaimed: u64,
    sections: crate::pages::layout::Sections,
}

impl DiskCleanupPage {
    pub fn new() -> Self {
        let opts = PageValues::new(page_options(PageId::DiskCleanup));
        let mut tree = TreeView::new(TREE, usage());
        tree.expanded.insert(vec![0]);
        tree.flatten();
        let items = CATEGORIES_DATA
            .iter()
            .map(|c| {
                let meta = format!(
                    "{:>9}  {:<17}",
                    bytes(c.size),
                    c.blocked.unwrap_or(c.safety)
                );
                ListItem::new(c.label)
                    .meta(&meta)
                    .meta_tone(match c.safety {
                        "review first" => Tone::Warning,
                        _ => Tone::Muted,
                    })
                    .disabled(c.blocked.is_some())
            })
            .collect();
        let mut list = ListBox::new(CATEGORIES, items, SelectMode::Multi);
        for (i, c) in CATEGORIES_DATA.iter().enumerate() {
            if c.checked {
                list.set_checked(i, true);
            }
        }
        let mut p = Self {
            template: Template::new(ID, states_grid()),
            opts,
            tree,
            list,
            cats: (0..CATEGORIES_DATA.len()).collect(),
            rescan: Button::secondary(ID.sub("rescan"), "Rescan"),
            review: Button::primary(ID.sub("review"), "Review deletion…"),
            facts: PropsList::new(DETAILS, vec![]),
            facts_for: None,
            usage_md: MasterDetail::new(TREE, DETAILS),
            clean_md: MasterDetail::new(CATEGORIES, ID.sub("review")),
            scanning: 0,
            reclaimed: 0,
            sections: crate::pages::layout::Sections::new(ID.sub("page")),
        };
        p.rebuild();
        p
    }

    fn rebuild(&mut self) {
        let tree = std::mem::replace(&mut self.tree, TreeView::new(TREE, vec![]));
        self.tree = tree.show_meta(self.opts.on("diskcleanup.sizes"));
        let list = std::mem::replace(
            &mut self.list,
            ListBox::new(CATEGORIES, vec![], SelectMode::Multi),
        );
        self.list = list.click(ClickPolicy {
            check: match self.opts.value("diskcleanup.check_click") {
                "row" => CheckClick::Row,
                _ => CheckClick::BoxOnly,
            },
            ..ClickPolicy::DATA
        });
        let collapse = self.opts.number("diskcleanup.collapse_below").unwrap_or(72);
        let md = |master, detail| {
            MasterDetail::new(master, detail)
                .detail_share(40, 30, 44)
                .collapse_below(collapse)
                .summary_rows(1)
        };
        self.usage_md = md(TREE, DETAILS);
        self.clean_md = md(CATEGORIES, self.review.id).detail_owns(&[self.rescan.id]);
        self.sync_scan();
    }

    fn mode(&self) -> Mode {
        match self.opts.value("diskcleanup.mode") {
            "permanent" => Mode::Permanent,
            "dry-run" => Mode::DryRun,
            _ => Mode::Trash,
        }
    }

    fn is_scanning(&self) -> bool {
        self.scanning > 0 || self.opts.value("diskcleanup.scan") == "scanning"
    }

    fn sync_scan(&mut self) {
        let n = self.tree.node(&[0]).map_or(0, |n| n.children.len());

        let done = if self.scanning > 0 {
            n * (SCAN_TICKS - self.scanning) as usize / SCAN_TICKS as usize
        } else if self.is_scanning() {
            2
        } else {
            n
        };
        for i in 0..n {
            self.tree.set_busy(&[0, i], i >= done);
        }
    }

    fn checked(&self) -> Vec<(usize, &'static Category)> {
        self.cats
            .iter()
            .enumerate()
            .filter(|(row, _)| self.list.is_checked(*row))
            .map(|(row, &c)| (row, &CATEGORIES_DATA[c]))
            .collect()
    }

    fn selected_bytes(&self) -> u64 {
        self.checked().iter().map(|(_, c)| c.size).sum()
    }

    fn filesystems(&self) -> MeterList {
        let visual = match self.opts.value("diskcleanup.meter_visual") {
            "block" => MeterVisual::Block,
            _ => MeterVisual::Line,
        };
        let total = 926 * GIB;
        let used = (842 * GIB).saturating_sub(self.reclaimed);
        let root = (used * 100 / total) as u8;
        let row = |label: &str, pct: u8, detail: String| {
            let mut m = Meter::new(Some(pct))
                .value(format!("{pct}%"))
                .tone(DISK_TONE)
                .visual(visual);
            if self.is_scanning() {
                m = m.tone(MeterTone::Refreshing);
            }
            MeterRow::new(label, m).detail(detail)
        };
        MeterList::new(vec![
            row("/", root, format!("{} of 926 GiB", used / GIB)),
            row("/Volumes/Backup", 71, "1420 of 2000 GB".into()),
            row("/Volumes/Scratch", 12, "61 of 512 GB".into()),
        ])
        .label_tone(Tone::Secondary)
        .meter_width(24)
        .reserve(4)
        .min_meter(8)
    }

    fn details(&self) -> (String, Vec<Prop>) {
        let rows = self.tree.rows();
        let Some(row) = rows.get(self.tree.cursor) else {
            return (String::new(), vec![]);
        };
        let path: Vec<String> = {
            let mut nodes = &self.tree.nodes;
            let mut parts = vec!["~".to_owned()];
            for &i in &row.path {
                parts.push(nodes[i].label.clone());
                nodes = &nodes[i].children;
            }
            parts
        };
        let size = row
            .meta
            .as_deref()
            .and_then(|m| m.split('·').next())
            .unwrap_or("")
            .trim()
            .to_owned();
        let kind = if row.has_children {
            "directory"
        } else {
            "file"
        };
        let fresh = if row.busy {
            Prop::new("Freshness", "scanning…").tone(Tone::Warning)
        } else {
            Prop::new("Freshness", "live · complete")
        };
        (
            kind.into(),
            vec![
                Prop::new("Path", path.join("/")),
                Prop::new("Allocated", size),
                fresh,
                Prop::new("Modified", "2026-09-07 08:41"),
                Prop::new("Rule", "hardlinks counted once").wrap(),
            ],
        )
    }

    fn reclaim_panel(&self) -> Panel<'static> {
        Panel::card(Some("Reclaim")).meta(match self.mode() {
            Mode::DryRun => "dry run",
            _ => "two gates",
        })
    }

    fn reclaim_facts(&self) -> Vec<Prop> {
        let mode = self.mode();
        let checked = self.checked();
        let gate = if mode == Mode::DryRun {
            "review only · nothing changes".to_owned()
        } else if self.opts.on("diskcleanup.typed") {
            format!("review, then type {}", mode.phrase(checked.len()))
        } else {
            "review, then confirm".to_owned()
        };
        vec![
            Prop::new("Selected", plural(checked.len(), "category", "categories")),
            Prop::new(
                "Reclaim",
                format!("{} estimated", bytes(self.selected_bytes())),
            ),
            Prop::new("Mode", mode.label())
                .tone(match mode {
                    Mode::Permanent => Tone::Danger,
                    _ => Tone::Secondary,
                })
                .wrap(),
            Prop::new("Gate", gate).wrap(),
        ]
    }

    fn skipped(&self) -> Option<Notice> {
        self.cats
            .iter()
            .filter_map(|&c| {
                CATEGORIES_DATA[c]
                    .blocked
                    .map(|b| (CATEGORIES_DATA[c].label, b))
            })
            .next()
            .map(|(label, why)| Notice::warning(format!("{label} is skipped")).detail(why))
    }

    fn fs_meta(&self) -> String {
        if self.is_scanning() {
            "scanning ~/work…".to_owned()
        } else {
            format!(
                "scan complete · 31.8 GiB in ~/work · {} reclaimed",
                bytes(self.reclaimed)
            )
        }
    }

    fn heights(&self, width: u16, fs: &MeterList) -> [u16; 3] {
        let (kind, facts) = self.details();
        let details = match self.usage_md.split_rects(Rect::new(0, 0, width, 1)) {
            Some((_, detail)) => crate::pages::layout::sheet_height(
                &Panel::card(Some("Details")).meta(&kind),
                &facts,
                detail.width,
            ),
            None => 0,
        };
        let usage = 10u16.max(details);
        let list = self.cats.len() as u16 + 3;
        let cleanup = match self.clean_md.split_rects(Rect::new(0, 0, width, 1)) {
            Some((_, detail)) => {
                let panel = self.reclaim_panel();
                let inner = panel.inner(Size::new(detail.width, 0)).width;
                let mut h = props::measure(&self.reclaim_facts(), inner) + 1;
                if let Some(n) = self.skipped() {
                    h += n.height(inner) + 1;
                }
                list.max(panel.outer(Size::new(inner, h + 1)).height)
            }
            None => list,
        };
        [fs.height() + 3, usage, cleanup]
    }

    fn render_playground(&mut self, area: Rect, buf: &mut Buffer, ctx: &mut RenderCtx) {
        let fs = self.filesystems();
        let focus = if [self.tree.id, DETAILS]
            .iter()
            .any(|&id| ctx.interaction.has_focus(id))
        {
            Some(1)
        } else if [self.list.id, self.rescan.id, self.review.id]
            .iter()
            .any(|&id| ctx.interaction.has_focus(id))
        {
            Some(2)
        } else {
            None
        };
        let l = self.sections.layout(
            area,
            &self.heights(area.width, &fs).map(StackChild::fixed),
            focus,
            buf,
            ctx,
        );
        if let Some(r) = l.slot(0) {
            self.render_filesystems(r, &fs, buf, ctx);
        }
        match l.slot(1) {
            Some(r) => self.render_usage(r, buf, ctx),
            None => {
                ctx.focus_stop(self.tree.id);
                ctx.focus_stop(DETAILS);
            }
        }
        match l.slot(2) {
            Some(r) => self.render_cleanup(r, buf, ctx),
            None => {
                ctx.focus_stop(self.list.id);
                ctx.focus_stop(self.review.id);
                ctx.focus_stop(self.rescan.id);
            }
        }
    }

    fn render_filesystems(
        &mut self,
        area: Rect,
        list: &MeterList,
        buf: &mut Buffer,
        ctx: &mut RenderCtx,
    ) {
        let t = ctx.theme;
        let meta = self.fs_meta();
        let panel = Panel::card(Some("Filesystems")).meta(&meta);
        let bg = panel.bg(t);
        let inner = panel.render(area, buf, t);
        list.render(inner, buf, ctx, bg);
    }

    fn render_usage(&mut self, area: Rect, buf: &mut Buffer, ctx: &mut RenderCtx) {
        let t = ctx.theme;
        let l = self.usage_md.layout(area, buf, ctx, t.canvas);
        if !l.master.is_empty() {
            let meta = if self.is_scanning() {
                let busy = self.tree.rows().iter().filter(|r| r.busy).count();
                format!("scanning · {busy} left")
            } else {
                "largest first · allocated".into()
            };
            let panel = Panel::card(Some("Usage · ~/work"))
                .meta(&meta)
                .focused(ctx.interaction.focused(self.tree.id));
            let bg = panel.bg(t);
            let inner = panel.render(l.master, buf, t);
            self.tree.render(inner, buf, ctx, bg);
        }
        let (kind, facts) = self.details();
        if !l.detail.is_empty() {
            let path = self.tree.cursor_path().map(<[usize]>::to_vec);
            if path != self.facts_for || self.is_scanning() {
                self.facts.set_props(facts.clone());
                self.facts_for = path;
            }
            let panel = Panel::card(Some("Details"))
                .meta(&kind)
                .focused(ctx.interaction.focused(DETAILS));
            let bg = panel.bg(t);
            let inner = panel.render(l.detail, buf, t);
            self.facts.render(inner, buf, ctx, bg);
        }
        if !l.summary.is_empty() {
            let line = format!(
                "→ {} · {} · Tab shows details",
                facts.first().map_or("", |p| p.value.as_str()),
                facts.get(1).map_or("", |p| p.value.as_str()),
            );
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
        self.usage_md.finish(ctx);
    }

    fn render_cleanup(&mut self, area: Rect, buf: &mut Buffer, ctx: &mut RenderCtx) {
        let t = ctx.theme;
        let md = self.clean_md.layout(area, buf, ctx, t.canvas);
        let (l, r) = (md.master, md.detail);
        let checked = self.checked();
        let meta = format!(
            "{} of {} · {}",
            checked.len(),
            self.cats.len(),
            bytes(self.selected_bytes())
        );
        let panel = Panel::card(Some("Cleanup categories"))
            .meta(&meta)
            .focused(ctx.interaction.focused(self.list.id));
        let bg = panel.bg(t);
        if !l.is_empty() {
            let inner = panel.render(l, buf, t);
            self.list.render(inner, buf, ctx, bg);
        }
        if !md.summary.is_empty() {
            let line = format!("→ {} · {} · Tab reviews", meta, self.mode().verb());
            segments::render(
                md.summary,
                buf,
                ctx,
                &[Segment::new(
                    truncate(
                        &line,
                        md.summary.width.saturating_sub(2 * t.spacing.chrome_edge) as usize,
                    ),
                    Tone::Muted,
                )],
                &[],
                t.canvas,
            );
        }
        if r.is_empty() {
            self.clean_md.finish(ctx);
            ctx.focus_stop(self.rescan.id);
            return;
        }
        let panel = self.reclaim_panel();
        let bg = panel.bg(t);
        let inner = panel.render(r, buf, t);
        let facts = self.reclaim_facts();
        let h = props::render(inner, buf, t, &facts, bg);
        let mut y = inner.y + h + 1;
        if let Some(n) = self.skipped() {
            let nh = n.height(inner.width);
            if y + nh + 2 <= inner.bottom() {
                n.render(Rect::new(inner.x, y, inner.width, nh), buf, ctx, bg);
                y += nh + 1;
            }
        }
        let y = y.min(inner.bottom().saturating_sub(1));
        self.rescan.busy = self.scanning > 0;
        self.review.disabled = checked.is_empty() || self.is_scanning();
        let rects = row_layout(
            Rect::new(inner.x, y, inner.width, 1),
            &[self.review.width(), self.rescan.width()],
            2,
        );
        self.review.render(rects[0], buf, ctx, bg);
        self.rescan.render(rects[1], buf, ctx, bg);
        self.clean_md.finish(ctx);
    }

    fn start_rescan(&mut self, cx: &mut Cx) {
        if self.is_scanning() {
            cx.status("A scan is already running");
            return;
        }
        self.scanning = SCAN_TICKS;
        self.sync_scan();
    }

    fn facts(&self) -> Vec<Prop> {
        let checked = self.checked();
        let mode = self.mode();
        let names: Vec<&str> = checked.iter().map(|(_, c)| c.label).collect();
        vec![
            Prop::new(
                "Action",
                format!(
                    "{} {}",
                    mode.verb(),
                    plural(checked.len(), "category", "categories")
                ),
            ),
            Prop::new("Host", "mbp · local"),
            Prop::new("Mode", mode.label()).wrap(),
            Prop::new(
                "Estimate",
                format!(
                    "{} allocated · APFS clones may overcount",
                    bytes(self.selected_bytes())
                ),
            )
            .wrap(),
            Prop::new("Paths", names.join(" · ")).wrap(),
            Prop::new(
                "Recoverable",
                match mode {
                    Mode::Trash => "yes · until the Trash is emptied",
                    Mode::Permanent => "no",
                    Mode::DryRun => "nothing changes",
                },
            ),
        ]
    }

    fn gate1(&self) -> Dialog {
        let mode = self.mode();
        let n = self.checked().len();
        let code = self
            .checked()
            .iter()
            .enumerate()
            .map(|(i, (_, c))| {
                format!(
                    "{:02}  {} ~/Library/Caches/{}  # {}",
                    i + 1,
                    match mode {
                        Mode::Trash => "trash",
                        Mode::Permanent => "rm -rf",
                        Mode::DryRun => "du -sh",
                    },
                    c.label.to_lowercase().replace(' ', "-"),
                    bytes(c.size)
                )
            })
            .collect();
        let two = mode != Mode::DryRun;
        let title = if two {
            format!(
                "Review · {} · gate 1 of 2",
                plural(n, "category", "categories")
            )
        } else {
            format!("Dry run · {}", plural(n, "category", "categories"))
        };
        let go = if two {
            Button::primary(GATE1.sub("go"), "Continue…")
        } else {
            Button::primary(GATE1.sub("go"), mode.verb())
        };
        let d = Dialog::facts(GATE1, &title, self.facts(), code, None, go).width(76);
        if mode == Mode::Permanent {
            d.title_status(StatusKind::Warn)
        } else {
            d
        }
    }

    fn gate2(&self) -> Dialog {
        let mode = self.mode();
        let n = self.checked().len();
        let go = match mode {
            Mode::Permanent => Button::danger(GATE2.sub("go"), mode.verb()),
            _ => Button::primary(GATE2.sub("go"), mode.verb()),
        };
        let phrase = mode.phrase(n);
        let token = self.opts.on("diskcleanup.typed").then_some(phrase.as_str());
        let d = Dialog::facts(
            GATE2,
            &format!("{} · gate 2 of 2", mode.verb()),
            self.facts(),
            vec![],
            token,
            go,
        )
        .width(76);
        if mode == Mode::Permanent {
            d.title_status(StatusKind::Fail)
        } else {
            d
        }
    }

    fn open_review(&mut self, cx: &mut Cx) {
        if self.checked().is_empty() {
            cx.status("Select a category first (Space)");
            return;
        }
        cx.open_dialog(self.gate1());
    }

    fn commit(&mut self, cx: &mut Cx) {
        let checked = self.checked();
        let n = checked.len();
        let size = self.selected_bytes();
        let mode = self.mode();
        if mode == Mode::DryRun {
            cx.status(format!(
                "Dry run: {} · {} would be freed",
                plural(n, "category", "categories"),
                bytes(size)
            ));
            return;
        }
        let rows: Vec<usize> = checked.iter().map(|(r, _)| *r).collect();
        for &r in rows.iter().rev() {
            self.cats.remove(r);
        }
        self.list.remove_rows(&rows);
        self.reclaimed += size;
        cx.status(format!(
            "{} · {} reclaimed",
            match mode {
                Mode::Trash =>
                    format!("Moved {} to the Trash", plural(n, "category", "categories")),
                _ => format!("Deleted {}", plural(n, "category", "categories")),
            },
            bytes(size)
        ));
    }

    fn on_tree(&mut self, a: Option<TreeAction>, cx: &mut Cx) {
        if let Some(TreeAction::Activate(p)) = a
            && let Some(n) = self.tree.node(&p)
        {
            cx.status(format!("Opened {} in Finder", n.label));
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
                "work",
                vec![
                    TreeNode::leaf_meta("backend", "12.7 GiB"),
                    TreeNode::leaf_meta("dumps", "6.3 GiB"),
                    TreeNode::lazy("android"),
                ],
            )
            .meta("31.8 GiB"),
        ],
    );
    tree.expanded.insert(vec![0]);
    tree.flatten();
    let mut list = ListBox::new(
        id.child(1),
        vec![
            ListItem::new("DerivedData").meta("810 MiB"),
            ListItem::new("Cargo cache").meta("4.8 GiB"),
            ListItem::new("Docker cache").meta("in use").disabled(true),
        ],
        SelectMode::Multi,
    );
    list.set_checked(0, true);
    StateGrid::new(id)
        .label_width(11)
        .column_width(26)
        .rows(&[Rest, Focused, Selected, Busy, Empty, Narrow])
        .column("Usage tree", tree)
        .column("Categories", list)
        .column(
            "Filesystems",
            MeterList::new(vec![
                MeterRow::new("/", Meter::new(Some(91)).value("91%").tone(DISK_TONE)),
                MeterRow::new("/Backup", Meter::new(Some(71)).value("71%").tone(DISK_TONE)),
            ])
            .label_tone(Tone::Secondary)
            .meter_width(12),
        )
}

impl Page for DiskCleanupPage {
    fn title(&self) -> &'static str {
        "Disk cleanup"
    }
    fn blurb(&self) -> &'static str {
        "Usage tree, filesystem meters, a cleanup checklist and a two-gate review"
    }

    fn measure(&self, avail: Size, ctx: &RenderCtx) -> Size {
        let fs = self.filesystems();
        let meta = self.fs_meta();
        let fs_w = Panel::card(Some("Filesystems"))
            .meta(&meta)
            .outer(fs.measure(avail, ctx))
            .width;
        let tree = Panel::card(Some("Usage · ~/work"))
            .meta("largest first · allocated")
            .outer(self.tree.measure(avail, ctx))
            .width;
        let (kind, facts) = self.details();
        let details =
            crate::pages::layout::sheet_width(&Panel::card(Some("Details")).meta(&kind), &facts);
        let cats = Panel::card(Some("Cleanup categories"))
            .meta("10 of 10 · 99.9 GiB")
            .outer(self.list.measure(avail, ctx))
            .width;
        let width = fs_w
            .max(self.usage_md.panes_width(tree, details))
            .max(self.clean_md.panes_width(cats, 0))
            .max(avail.width);
        let height = self
            .sections
            .stack
            .measure(&self.heights(width, &fs).map(StackChild::fixed));
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

    fn animating(&self) -> bool {
        self.scanning > 0
    }

    fn handle(&mut self, ev: &PageEvent, cx: &mut Cx) -> Outcome {
        if let PageEvent::Tick = ev {
            if self.scanning == 0 {
                return Outcome::Ignored;
            }
            self.scanning -= 1;
            self.sync_scan();
            if self.scanning == 0 {
                cx.status("Scan complete · 31.8 GiB in ~/work");
            }
            return Outcome::Changed;
        }
        if let PageEvent::DialogClosed { id, result, .. } = ev {
            match (*id, *result) {
                (GATE1, DialogResult::Action(1)) if self.mode() == Mode::DryRun => self.commit(cx),
                (GATE1, DialogResult::Action(1)) => cx.open_dialog(self.gate2()),
                (GATE2, DialogResult::Action(1)) => self.commit(cx),
                (GATE1 | GATE2, _) => cx.status("Cleanup cancelled · nothing changed"),
                _ => return Outcome::Ignored,
            }
            return Outcome::Changed;
        }
        if let Some(o) = self.template.handle(ev, cx) {
            return o;
        }
        if self.template.section() != Section::Playground {
            return Outcome::Ignored;
        }
        if let Some(o) = self.sections.handle(ev) {
            return o;
        }
        match ev {
            PageEvent::Key(key) => {
                if let Some(o) = self.usage_md.on_key(key, cx.focus) {
                    return o;
                }
                if let Some(o) = self.clean_md.on_key(key, cx.focus) {
                    return o;
                }
                if cx.focus.is(DETAILS) {
                    return self.facts.on_key(key).0;
                }
                if cx.focus.is(self.tree.id) {
                    let (o, actions) = self.tree.on_key(key).into_parts();
                    for a in actions {
                        self.on_tree(Some(a), cx);
                    }
                    return o;
                }
                if cx.focus.is(self.list.id) {
                    if key.is_char('d') {
                        self.open_review(cx);
                        return Outcome::Changed;
                    }
                    return self.list.on_key(key);
                }
                if cx.focus.is(self.review.id) {
                    let (o, act) = self.review.on_key(key);
                    if act {
                        self.open_review(cx);
                    }
                    return o;
                }
                if cx.focus.is(self.rescan.id) {
                    let (o, act) = self.rescan.on_key(key);
                    if act {
                        self.start_rescan(cx);
                    }
                    return o;
                }
                Outcome::Ignored
            }
            PageEvent::Pointer(ev) => {
                if let Some(r) = self.facts.route(ev) {
                    return r.into_parts().0;
                }
                if let Some(r) = self.tree.route(ev) {
                    let (o, actions) = r.into_parts();
                    for a in actions {
                        self.on_tree(Some(a), cx);
                    }
                    return o;
                }
                if let Some(r) = self.list.route(ev) {
                    return r.outcome();
                }
                if let Some(r) = self.review.route(ev)
                    && r.outcome() != Outcome::Ignored
                {
                    if r.has_actions() {
                        self.open_review(cx);
                    }
                    return Outcome::Changed;
                }
                if let Some(r) = self.rescan.route(ev)
                    && r.outcome() != Outcome::Ignored
                {
                    if r.has_actions() {
                        self.start_rescan(cx);
                    }
                    return Outcome::Changed;
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
        if focus == Some(self.tree.id) {
            return vec![("↑↓", "Move"), ("←→", "Fold / open"), ("Enter", "Open")];
        }
        if focus == Some(self.list.id) {
            return vec![
                ("↑↓", "Move"),
                ("Space", "Select"),
                ("a", "All"),
                ("d", "Review deletion…"),
            ];
        }
        vec![("Enter", "Activate")]
    }

    fn option_value(&self, key: &str) -> Option<usize> {
        self.opts.get(key)
    }

    fn set_option(&mut self, key: &str, value: usize) {
        if self.opts.set(key, value) {
            self.rebuild();
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
