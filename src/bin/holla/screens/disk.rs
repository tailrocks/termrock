use crate::screens::Holla;
use junie_tui::shell::Page as ShellPage;
use junie_tui::widgets::page_header::Heading;
use std::collections::BTreeSet;

use junie_tui::core::event::{Key, Outcome};
use junie_tui::core::hit::Axis;
use junie_tui::core::id::{Part, WidgetId};
use junie_tui::core::intent::Phase;
use junie_tui::core::pointer::{MouseTarget, PointerEvent};
use junie_tui::core::scroll::ScrollState;
use junie_tui::core::selection::ClickPolicy;
use junie_tui::theme::Tone;
use junie_tui::ui::context::{ContextAnchor, ContextSource};
use junie_tui::ui::ctx::{RenderCtx, fill, legible_on_fill};
use junie_tui::ui::master_detail::MasterDetail;
use junie_tui::ui::text::{fit, width, wrap};
use junie_tui::widgets::empty::{self, EmptyState};
use junie_tui::widgets::keyhint::{Hint, hint};
use junie_tui::widgets::menu::MenuItem;
use junie_tui::widgets::meter_list::{MeterList, MeterRow};
use junie_tui::widgets::panel::Panel;
use junie_tui::widgets::progress::{
    Meter, MeterTone, MeterVisual, ProgressStatus, render_bar, render_spinner,
};
use junie_tui::widgets::props::Prop;
use junie_tui::widgets::scrollbar;
use junie_tui::widgets::statusbar::StatusItem;
use junie_tui::widgets::tree::{TreeAction, TreeNode, TreeView};
use ratatui::buffer::Buffer;
use ratatui::crossterm::event::{KeyCode, KeyModifiers};
use ratatui::layout::Rect;
use ratatui::style::{Modifier, Style};

use crate::domain::cleanup::{DeleteItem, DeletePlan, Mode, NOISE_NAMES};
use crate::domain::stack::Spotlight;
use crate::screens::{
    Cx, Go, ModalResult, Page, Screen, StatusBits, open_context, row_anchor, scroll_drag,
    scroll_press,
};
use crate::sim::fs::ScanNode;
use crate::sim::world::{ScanPhase, World};
use junie_tui::ui::layer::LayerTag;
use junie_tui::ui::text::plural;

pub const TREE: WidgetId = WidgetId::of("disk.tree");
pub const DETAIL: WidgetId = WidgetId::of("disk.detail");
pub const LIST: WidgetId = WidgetId::of("disk.list");

pub const FS_METER: MeterTone = MeterTone::Level { warn: 80, crit: 95 };

pub const ROW_MENU: WidgetId = WidgetId::of("disk.row.menu");

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Sort {
    Allocated,
    Apparent,
}

#[derive(Debug, Clone, PartialEq, Eq)]
enum View {
    Overview,

    Tree,

    TopFiles,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct OverviewRow {
    path: String,
    label: String,
    kind: &'static str,
    cached: Option<(u64, i64)>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct TopRow {
    path: String,
    bytes: u64,
    missing: bool,
}

pub struct DiskPage {
    pub path: String,
    view: View,
    tree: TreeView,

    paths: Vec<(Vec<usize>, String)>,

    folded: BTreeSet<String>,
    pub sort: Sort,
    pub fold: bool,
    pub selected: BTreeSet<String>,
    detail_scroll: ScrollState,
    list_scroll: ScrollState,
    detail_area: Rect,
    list_area: Rect,
    pub cursor: usize,
    overview: Vec<OverviewRow>,
    top: Vec<TopRow>,
    top_state: Option<String>,

    md: MasterDetail,
    last_tick: u64,
    last_revealed: usize,
    last_phase: Option<ScanPhase>,
    generation: u64,
    seeded_gen: u64,

    pub reviewed: Option<u64>,

    pub rebuilds: u32,
}

impl DiskPage {
    pub fn new(path: &str) -> Self {
        Self {
            path: path.into(),
            view: View::Tree,
            tree: TreeView::new(TREE, vec![]),
            paths: vec![],
            folded: BTreeSet::new(),
            sort: Sort::Allocated,
            fold: true,
            selected: BTreeSet::new(),
            detail_scroll: ScrollState::default(),
            list_scroll: ScrollState::default(),
            detail_area: Rect::ZERO,
            list_area: Rect::ZERO,
            cursor: 0,
            overview: vec![],
            top: vec![],
            top_state: None,
            md: MasterDetail::new(TREE, DETAIL).detail_share(38, 34, 50),
            last_tick: u64::MAX,
            last_revealed: usize::MAX,
            last_phase: None,
            generation: 0,
            seeded_gen: 0,
            reviewed: None,
            rebuilds: 0,
        }
    }

    pub fn overview() -> Self {
        let mut p = Self::new("");
        p.view = View::Overview;
        p
    }

    pub fn top_files() -> Self {
        let mut p = Self::new("");
        p.view = View::TopFiles;
        p
    }

    fn node_size(&self, n: &ScanNode) -> u64 {
        match self.sort {
            Sort::Allocated => n.allocated,
            Sort::Apparent => n.apparent,
        }
    }

    fn build_nodes(
        &mut self,
        n: &ScanNode,
        parent_bytes: u64,
        revealed: &dyn Fn(&str) -> bool,
        path: &mut Vec<usize>,
    ) -> TreeNode {
        let name = if n.path == "/" {
            "/".to_owned()
        } else {
            n.path.rsplit('/').next().unwrap_or(&n.path).to_owned()
        };
        let pct = if parent_bytes > 0 {
            (n.allocated as f64 / parent_bytes as f64 * 100.0).round() as u64
        } else {
            100
        };
        let foldable = self.fold && n.is_dir && NOISE_NAMES.contains(&name.as_str());
        let mut meta = format!(
            "{:>9} · {pct:>3}%",
            junie_tui::ui::text::bytes(self.node_size(n))
        );
        if let Some(e) = &n.error {
            meta = format!("{meta} · {}", short_error(e));
        }
        if n.link {
            meta = format!("{meta} · link");
        }
        if foldable {
            meta = format!("{meta} · folded");
        }
        let prefix = format!("{}/", n.path);
        let glyph: &'static str = if self.selected.contains(&n.path) {
            "✓"
        } else if self
            .selected
            .iter()
            .any(|s| n.path.starts_with(&format!("{s}/")))
        {
            "·"
        } else if self.selected.iter().any(|s| s.starts_with(&prefix)) {
            "◐"
        } else {
            " "
        };
        self.paths.push((path.clone(), n.path.clone()));
        if foldable {
            self.folded.insert(n.path.clone());
            return TreeNode::leaf(&name).meta(&meta).glyph(glyph);
        }
        if !n.is_dir || n.link {
            return TreeNode::leaf(&name).meta(&meta).glyph(glyph);
        }
        if !revealed(&n.path) {
            let mut t = TreeNode::lazy(&name)
                .meta(&format!("{:>9} · scanning", "…"))
                .glyph(glyph);
            t.busy = true;
            return t;
        }
        let mut children: Vec<&ScanNode> = n.children.iter().collect();
        match self.sort {
            Sort::Allocated => children.sort_by(|a, b| {
                b.allocated
                    .cmp(&a.allocated)
                    .then_with(|| a.path.cmp(&b.path))
            }),
            Sort::Apparent => children.sort_by(|a, b| {
                b.apparent
                    .cmp(&a.apparent)
                    .then_with(|| a.path.cmp(&b.path))
            }),
        }
        let mut kids = vec![];
        for (i, c) in children.iter().enumerate() {
            path.push(i);
            kids.push(self.build_nodes(c, n.allocated, revealed, path));
            path.pop();
        }
        if kids.is_empty() {
            let mut t = TreeNode::leaf(&name).meta(&meta).glyph(glyph);
            t.children = vec![TreeNode::note("empty")];
            return t;
        }
        TreeNode::dir(&name, kids).meta(&meta).glyph(glyph)
    }

    fn rebuild_tree(&mut self, w: &World) {
        let Some(scan) = &w.scan else {
            return;
        };
        if scan.root != self.path {
            return;
        }
        self.rebuilds += 1;
        let tick = w.tick;
        let focused = self.cursor_fs_path();
        let expanded_fs: Vec<String> = self
            .tree
            .expanded
            .iter()
            .filter_map(|p| {
                self.paths
                    .iter()
                    .find(|(pp, _)| pp == p)
                    .map(|(_, f)| f.clone())
            })
            .collect();
        self.paths.clear();
        self.folded.clear();
        let order = scan.order.clone();
        let n = scan.revealed(tick);
        let revealed_set: BTreeSet<&str> = order.iter().take(n).map(String::as_str).collect();
        let revealed = |p: &str| revealed_set.contains(p);
        let tree = scan.tree.clone();
        let mut path = vec![0];
        let root = self.build_nodes(&tree, tree.allocated, &revealed, &mut path);
        self.tree.nodes = vec![root];

        let mut expanded: std::collections::HashSet<Vec<usize>> = Default::default();
        expanded.insert(vec![0]);
        for f in expanded_fs {
            if let Some((pp, _)) = self.paths.iter().find(|(_, fp)| *fp == f) {
                expanded.insert(pp.clone());
            }
        }
        self.tree.expanded = expanded;
        self.tree.flatten();
        if let Some(f) = focused
            && let Some((pp, _)) = self.paths.iter().find(|(_, fp)| *fp == f)
        {
            self.tree.reveal(&pp.clone());
        }

        let live: BTreeSet<&str> = self.paths.iter().map(|(_, f)| f.as_str()).collect();
        self.selected.retain(|s| live.contains(s.as_str()));
    }

    fn cursor_fs_path(&self) -> Option<String> {
        let p = self.tree.cursor_path()?;
        self.paths
            .iter()
            .find(|(pp, _)| pp == p)
            .map(|(_, f)| f.clone())
    }

    fn fs_path_of(&self, positional: &[usize]) -> Option<String> {
        self.paths
            .iter()
            .find(|(pp, _)| pp == positional)
            .map(|(_, f)| f.clone())
    }

    fn scan_node<'a>(&self, w: &'a World, path: &str) -> Option<&'a ScanNode> {
        w.scan.as_ref().and_then(|s| s.tree.find(path))
    }

    fn seed_selection(&mut self, w: &World) {
        let Some(scan) = &w.scan else { return };
        if self.seeded_gen == scan.generation || !scan.completed() {
            return;
        }
        self.seeded_gen = scan.generation;
        for c in &w.disk.candidates {
            if c.default_selected()
                && c.path.starts_with(&self.path)
                && scan.tree.find(&c.path).is_some()
            {
                self.selected.insert(c.path.clone());
            }
        }
    }

    fn candidate_skip(&self, w: &World, path: &str) -> Option<String> {
        w.disk
            .candidates
            .iter()
            .find(|c| c.path == path)
            .and_then(|c| c.skip_reason())
    }

    fn toggle_select(&mut self, path: &str, w: &World, cx: &mut Cx) {
        if let Some(r) = self.candidate_skip(w, path) {
            cx.status(format!(
                "Cannot select {} · {r}",
                path.rsplit('/').next().unwrap_or(path)
            ));
            return;
        }
        if self
            .selected
            .iter()
            .any(|s| path.starts_with(&format!("{s}/")))
        {
            cx.status("Already covered by a selected ancestor");
            return;
        }
        if self.selected.remove(path) {
            cx.status(format!(
                "Unselected · {} selected · {}",
                self.selected.len(),
                junie_tui::ui::text::bytes(self.selected_bytes(w))
            ));
        } else {
            self.selected
                .retain(|s| !s.starts_with(&format!("{path}/")));
            self.selected.insert(path.to_owned());
            cx.status(format!(
                "{} selected · {} estimated",
                plural(self.selected.len(), "item", "items"),
                junie_tui::ui::text::bytes(self.selected_bytes(w))
            ));
        }
        self.reviewed = None;
        self.rebuild_tree(w);
    }

    pub fn selected_bytes(&self, w: &World) -> u64 {
        self.selected
            .iter()
            .filter_map(|p| {
                self.scan_node(w, p)
                    .map(|n| n.allocated)
                    .or_else(|| Some(w.fs.size_of(p)))
            })
            .sum()
    }

    fn build_plan(&self, w: &World, mode: Mode, dry_run: bool) -> DeletePlan {
        let items: Vec<DeleteItem> = self
            .selected
            .iter()
            .map(|p| DeleteItem {
                path: p.clone(),
                category: "disk.tree".into(),
                estimate: self
                    .scan_node(w, p)
                    .map(|n| n.allocated)
                    .unwrap_or_else(|| w.fs.size_of(p)),
                guard: None,
            })
            .collect();
        DeletePlan::new(&w.host.name, &self.path, items, mode, dry_run)
    }

    fn rebuild_overview(&mut self, w: &World) {
        let home = w.location.home.clone();
        let now = w.now_secs();
        let mut rows = vec![];
        if let Ok(children) = w.fs.list(&home) {
            let mut dirs: Vec<_> = children
                .iter()
                .filter(|c| c.is_dir() && !c.hidden())
                .collect();
            dirs.sort_by_key(|d| d.name().to_lowercase());
            for d in dirs {
                let cached = w
                    .size_cache
                    .hint(&d.path, d.mtime, now)
                    .map(|e| (e.allocated, e.recorded_secs));
                rows.push(OverviewRow {
                    path: d.path.clone(),
                    label: w.location.short(&d.path),
                    kind: "home folder",
                    cached,
                });
            }
        }
        for c in crate::domain::catalog::insight_candidates(w) {
            for r in c.category.roots {
                let p = format!("{home}/{r}");
                if w.fs.is_dir(&p) && !rows.iter().any(|x| x.path == p) {
                    let cached =
                        w.fs.get(&p)
                            .and_then(|n| w.size_cache.hint(&p, n.mtime, now))
                            .map(|e| (e.allocated, e.recorded_secs));
                    rows.push(OverviewRow {
                        path: p.clone(),
                        label: w.location.short(&p),
                        kind: c.category.label,
                        cached,
                    });
                }
            }
        }
        self.overview = rows;
        self.list_scroll.set_content(self.overview.len());
    }

    fn rebuild_top(&mut self, w: &World) {
        self.top.clear();
        self.top_state = None;
        match &w.platform.spotlight {
            Spotlight::Unavailable(r) => self.top_state = Some(format!("unavailable · {r}")),
            Spotlight::Timeout => self.top_state = Some(
                "unavailable · the Spotlight query did not finish within 5 s · use the tree scan"
                    .into(),
            ),
            Spotlight::Empty => self.top_state = Some("no files of 100 MiB or more".into()),
            Spotlight::Available(list) => {
                if w.host.os != crate::domain::context::Os::MacOs {
                    self.top_state =
                        Some("unavailable · Spotlight is macOS only · use the tree scan".into());
                } else {
                    let mut seen = BTreeSet::new();
                    let mut rows = vec![];
                    for (p, b) in list {
                        if !seen.insert(p.clone()) {
                            continue;
                        }

                        let (bytes, missing) = if w.fs.is_file(p) {
                            (w.fs.get(p).map(|n| n.allocated).unwrap_or(*b), false)
                        } else {
                            (*b, true)
                        };
                        if bytes >= 100 * 1024 * 1024 || missing {
                            rows.push(TopRow {
                                path: p.clone(),
                                bytes,
                                missing,
                            });
                        }
                    }
                    rows.sort_by(|a, b| b.bytes.cmp(&a.bytes).then_with(|| a.path.cmp(&b.path)));
                    rows.truncate(50);
                    if rows.is_empty() {
                        self.top_state = Some("no files of 100 MiB or more".into());
                    }
                    self.top = rows;
                }
            }
        }
        self.list_scroll.set_content(self.top.len());
    }

    fn detail_props(&self, w: &World) -> (String, String, Vec<Prop>) {
        match self.view {
            View::Tree => {
                let Some(path) = self.cursor_fs_path() else {
                    return ("Disk".into(), String::new(), vec![]);
                };
                let node = self.scan_node(w, &path);
                let fs_node = w.fs.get(&path);
                let mut v = vec![Prop::new("Path", w.location.short(&path)).wrap()];
                if let Some(n) = node {
                    v.push(Prop::new(
                        "Allocated",
                        junie_tui::ui::text::bytes(n.allocated),
                    ));
                    v.push(Prop::new(
                        "Apparent",
                        format!(
                            "{}{}",
                            junie_tui::ui::text::bytes(n.apparent),
                            if n.apparent > n.allocated {
                                " · sparse"
                            } else if n.apparent < n.allocated {
                                " · block rounding"
                            } else {
                                ""
                            }
                        ),
                    ));
                    v.push(Prop::new(
                        "Entries",
                        format!("{} · hardlinks counted once", n.entries),
                    ));
                    if let Some(e) = &n.error {
                        v.push(Prop::new("Error", e.message()).tone(Tone::Error).wrap());
                    }
                    if n.link {
                        v.push(Prop::new("Link", "symbolic link · never traversed · selecting it removes the link only").tone(Tone::Muted).wrap());
                    }
                }
                if let Some(f) = fs_node {
                    v.push(Prop::new("Modified", crate::scenario::CLOCK.stamp(f.mtime)));
                    if f.dataless {
                        v.push(
                            Prop::new(
                                "Dataless",
                                "evicted to iCloud · never materialised by the scan",
                            )
                            .tone(Tone::Warning)
                            .wrap(),
                        );
                    }
                }
                let now = w.now_secs();
                let live = w
                    .scan
                    .as_ref()
                    .is_some_and(|s| s.is_revealed(&path, w.tick));
                if !live
                    && let Some(f) = fs_node
                    && let Some(h) = w.size_cache.hint(&path, f.mtime, now)
                {
                    v.push(
                        Prop::new(
                            "Cached",
                            format!(
                                "{} · {} · hint only",
                                junie_tui::ui::text::bytes(h.allocated),
                                w.clock.ago(h.recorded_secs)
                            ),
                        )
                        .tone(Tone::Muted),
                    );
                }
                if let Some(scan) = &w.scan {
                    v.push(
                        Prop::new(
                            "Freshness",
                            if scan.cancelled() {
                                "partial · scan cancelled".to_owned()
                            } else if scan.completed() {
                                "live · complete".to_owned()
                            } else {
                                format!("live · {}%", (scan.progress(w.tick) * 100.0).round())
                            },
                        )
                        .tone(if scan.cancelled() {
                            Tone::Warning
                        } else {
                            Tone::Muted
                        }),
                    );
                }
                if let Some(c) = w.disk.candidates.iter().find(|c| c.path == path) {
                    v.push(
                        Prop::new(
                            "Candidate",
                            format!(
                                "{} · {} · {} · {} confidence",
                                c.family.label(),
                                c.age_label(),
                                c.method.label(),
                                c.confidence.label()
                            ),
                        )
                        .wrap(),
                    );
                    if let Some(r) = c.skip_reason() {
                        v.push(Prop::new("Skipped", r).tone(Tone::Error));
                    }
                    v.push(
                        Prop::new("Regenerate", c.regenerate.clone())
                            .tone(Tone::Secondary)
                            .wrap(),
                    );
                }
                if self.folded.contains(&path) {
                    v.push(
                        Prop::new(
                            "Folded",
                            "presentation only · the subtree was scanned in full · f unfolds",
                        )
                        .tone(Tone::Muted)
                        .wrap(),
                    );
                }
                let selected = self.selected.contains(&path);
                v.push(
                    Prop::new(
                        "Selection",
                        if selected {
                            "selected · Space unselects"
                        } else if self
                            .selected
                            .iter()
                            .any(|s| path.starts_with(&format!("{s}/")))
                        {
                            "covered by a selected ancestor"
                        } else {
                            "not selected · Space selects"
                        },
                    )
                    .tone(Tone::Muted),
                );
                (
                    path.rsplit('/').next().unwrap_or(&path).to_owned(),
                    if node.is_some_and(|n| n.is_dir) {
                        "directory".into()
                    } else {
                        "file".into()
                    },
                    v,
                )
            }
            View::Overview => {
                let Some(r) = self.overview.get(self.cursor) else {
                    return ("Disk".into(), String::new(), vec![]);
                };
                let mut v = vec![
                    Prop::new("Path", r.label.clone()).wrap(),
                    Prop::new("Kind", r.kind),
                ];
                match r.cached {
                    Some((b, at)) => v.push(
                        Prop::new(
                            "Cached",
                            format!(
                                "{} · {} · a hint, not a measurement",
                                junie_tui::ui::text::bytes(b),
                                w.clock.ago(at)
                            ),
                        )
                        .tone(Tone::Muted)
                        .wrap(),
                    ),
                    None => v.push(
                        Prop::new("Size", "unscanned · Enter starts a live analysis")
                            .tone(Tone::Muted),
                    ),
                }
                (
                    r.label.rsplit('/').next().unwrap_or("").to_owned(),
                    "root".into(),
                    v,
                )
            }
            View::TopFiles => {
                let Some(r) = self.top.get(self.cursor) else {
                    return (
                        "Top files".into(),
                        String::new(),
                        vec![
                            Prop::new("State", self.top_state.clone().unwrap_or("querying".into()))
                                .wrap(),
                        ],
                    );
                };
                (
                    r.path.rsplit('/').next().unwrap_or("").to_owned(),
                    "file".into(),
                    vec![
                        Prop::new("Path", w.location.short(&r.path)).wrap(),
                        Prop::new(
                            "Size",
                            if r.missing {
                                "stat failed · the file vanished".into()
                            } else {
                                format!("{} allocated", junie_tui::ui::text::bytes(r.bytes))
                            },
                        )
                        .tone(if r.missing {
                            Tone::Error
                        } else {
                            Tone::Normal
                        }),
                        Prop::new("Source", "Spotlight · kMDItemFSSize >= 104857600 · top 50")
                            .tone(Tone::Muted)
                            .wrap(),
                        Prop::new(
                            "Selection",
                            if self.selected.contains(&r.path) {
                                "selected · feeds the same cleanup gate"
                            } else {
                                "Space selects"
                            },
                        )
                        .tone(Tone::Muted),
                    ],
                )
            }
        }
    }

    fn render_detail(&mut self, area: Rect, buf: &mut Buffer, ctx: &mut RenderCtx, w: &World) {
        let t = ctx.theme;
        let focused = ctx.interaction.focused(DETAIL);
        let (title, meta, props) = self.detail_props(w);
        let panel = Panel::card(Some(&title)).focused(focused).meta(&meta);
        let bg = panel.bg(t);
        let inner = panel.render(area, buf, t);
        self.detail_area = inner;
        ctx.control(DETAIL, area, false);
        ctx.scrollable(DETAIL, inner);
        let label_w = props.iter().map(|p| width(&p.label)).max().unwrap_or(4) as u16 + 2;
        let vw = inner.width.saturating_sub(label_w) as usize;
        let mut flat: Vec<(String, String, Tone)> = vec![];
        for p in props {
            for (i, part) in wrap(&p.value, vw.max(8)).into_iter().enumerate() {
                flat.push((
                    if i == 0 {
                        p.label.clone()
                    } else {
                        String::new()
                    },
                    part,
                    p.tone,
                ));
            }
        }
        self.detail_scroll.set_content(flat.len());
        self.detail_scroll.set_viewport(inner.height as usize);
        for (k, i) in self.detail_scroll.visible_range().enumerate() {
            let y = inner.y + k as u16;
            let (l, v, tone) = &flat[i];
            buf.set_string(inner.x, y, l, t.muted().bg(bg));
            buf.set_string(
                inner.x + label_w,
                y,
                junie_tui::ui::reveal::cut(Rect::new(inner.x + label_w, y, (vw) as u16, 1), v),
                Style::new().fg(t.tone(*tone)).bg(bg),
            );
        }
        if self.detail_scroll.overflows() {
            scrollbar::render_with_fade(
                Rect::new(inner.x, inner.y, inner.width, inner.height),
                buf,
                ctx,
                DETAIL,
                &self.detail_scroll,
                focused,
            );
        }
    }

    fn render_list(&mut self, area: Rect, buf: &mut Buffer, ctx: &mut RenderCtx, w: &World) {
        let t = ctx.theme;
        let bg = t.canvas;
        let focused = ctx.interaction.focused(LIST);
        self.list_area = area;
        ctx.control(LIST, area, false);
        ctx.scrollable(LIST, area);
        let rows: Vec<(String, String, String, bool)> = match self.view {
            View::Overview => self
                .overview
                .iter()
                .map(|r| {
                    (
                        r.label.clone(),
                        match r.cached {
                            Some((b, at)) => {
                                format!(
                                    "{:>9} · cached {}",
                                    junie_tui::ui::text::bytes(b),
                                    w.clock.ago(at)
                                )
                            }
                            None => format!("{:>9} · unscanned", "–"),
                        },
                        r.kind.to_owned(),
                        false,
                    )
                })
                .collect(),
            View::TopFiles => self
                .top
                .iter()
                .map(|r| {
                    (
                        w.location.short(&r.path),
                        if r.missing {
                            "stat failed".to_owned()
                        } else {
                            format!("{:>9}", junie_tui::ui::text::bytes(r.bytes))
                        },
                        String::new(),
                        self.selected.contains(&r.path),
                    )
                })
                .collect(),
            View::Tree => vec![],
        };
        if rows.is_empty() {
            let (title, hint_text) = match self.view {
                View::TopFiles => (
                    self.top_state.clone().unwrap_or("Top files".into()),
                    "Esc goes back · the tree scan works everywhere",
                ),
                _ => ("Nothing to show".into(), "Esc goes back"),
            };
            empty::render(area, buf, t, &EmptyState::new(&title).hint(hint_text), bg);
            return;
        }
        self.list_scroll.set_content(rows.len());
        self.list_scroll.set_viewport(area.height as usize);
        let has_sb = self.list_scroll.overflows();
        let row_w = area.width.saturating_sub(u16::from(has_sb));
        let label_w = (row_w * 50 / 100).clamp(16, 56);
        for (k, i) in self.list_scroll.visible_range().enumerate() {
            let y = area.y + k as u16;
            let (label, size, kind, checked) = &rows[i];
            let rid = LIST.child(i);
            let mut s = ctx.state(rid);
            s.focused = focused && i == self.cursor;
            s.selected = i == self.cursor;
            let st = t.row(s, bg);
            let row = Rect::new(area.x, y, row_w, 1);
            fill(buf, row, st);
            buf.set_string(
                row.x,
                y,
                t.gutter_symbol(s),
                t.gutter(s, st.bg.unwrap_or(bg), false),
            );
            let plain = st.remove_modifier(Modifier::BOLD);
            let mut x = row.x + 1;
            if self.view == View::TopFiles {
                buf.set_string(
                    x,
                    y,
                    if *checked { "[✓]" } else { "[ ]" },
                    plain.fg(if *checked { t.accent } else { t.text_muted }),
                );
                x += 4;
            } else {
                x += 2;
            }
            buf.set_string(x, y, fit(label, label_w as usize), st);
            x += label_w + 2;
            let avail = row.right().saturating_sub(x + 1) as usize;
            if avail >= 8 {
                buf.set_string(
                    x,
                    y,
                    junie_tui::ui::reveal::cut(Rect::new(x, y, (avail) as u16, 1), size),
                    plain.fg(t.text_secondary),
                );
            }
            x += (width(size) as u16 + 2).min(row.right().saturating_sub(x));
            let avail = row.right().saturating_sub(x + 1) as usize;
            if avail >= 6 && !kind.is_empty() {
                buf.set_string(
                    x,
                    y,
                    junie_tui::ui::reveal::cut(Rect::new(x, y, (avail) as u16, 1), kind),
                    plain.fg(t.text_muted),
                );
            }
            legible_on_fill(buf, row, t, bg);
            ctx.clickable(rid, row);
        }
        if has_sb {
            scrollbar::render_with_fade(
                Rect::new(area.x, area.y, area.width, area.height),
                buf,
                ctx,
                LIST,
                &self.list_scroll,
                focused,
            );
        }
    }

    fn open_row(&mut self, w: &World, cx: &mut Cx) -> Outcome {
        match self.view {
            View::Overview => {
                if let Some(r) = self.overview.get(self.cursor) {
                    cx.go(Go::Analyze(r.path.clone()));
                }
                Outcome::Changed
            }
            View::TopFiles => {
                if let Some(r) = self.top.get(self.cursor).cloned() {
                    if r.missing {
                        cx.status("The file vanished · nothing to select");
                    } else {
                        self.toggle_select(&r.path, w, cx);
                    }
                }
                Outcome::Changed
            }
            View::Tree => Outcome::Ignored,
        }
    }
}

impl DiskPage {
    fn select_reason(&self, w: &World, path: &str) -> Option<String> {
        if let Some(r) = self.candidate_skip(w, path) {
            return Some(r);
        }
        self.selected
            .iter()
            .any(|s| path.starts_with(&format!("{s}/")))
            .then(|| "Already covered by a selected ancestor".to_owned())
    }

    fn row_menu(&self, row: usize, anchor: ContextAnchor, w: &World, cx: &mut Cx) -> Outcome {
        let copy = MenuItem::new("Copy path").shortcut("y");
        let mut review = MenuItem::new("Review deletion…").shortcut("d").danger();
        if self.selected.is_empty() {
            review = review.reason("Select at least one entry first");
        }
        let (path, items, keys): (String, Vec<MenuItem>, &str) = match self.view {
            View::Tree => {
                let Some(path) = self
                    .tree
                    .rows()
                    .get(row)
                    .and_then(|r| self.fs_path_of(&r.path))
                else {
                    return Outcome::Ignored;
                };
                let mut select = MenuItem::new(if self.selected.contains(&path) {
                    "Unselect"
                } else {
                    "Select"
                })
                .shortcut("Space");
                if let Some(r) = self.select_reason(w, &path) {
                    select = select.reason(r);
                }
                let mut clean = MenuItem::new("Build cleanup plan…")
                    .shortcut("c")
                    .separator();
                if !self
                    .selected
                    .iter()
                    .any(|p| w.disk.candidates.iter().any(|c| &c.path == p))
                {
                    clean = clean.reason("Select rebuildable artifacts first");
                }
                let mut cancel = MenuItem::new("Cancel scan").shortcut("x");
                if !w.scan.as_ref().is_some_and(|s| s.is_running()) {
                    cancel = cancel.reason("No scan is running");
                }
                let items = vec![
                    select,
                    copy,
                    self.md.menu_item().shortcut("p").separator(),
                    review,
                    clean,
                    MenuItem::new("Rescan").shortcut("r"),
                    cancel,
                ];
                (path, items, " ypdcrx")
            }
            View::Overview => {
                let Some(r) = self.overview.get(row) else {
                    return Outcome::Ignored;
                };
                let mut top = MenuItem::new("Top files").shortcut("t");
                if w.host.os != crate::domain::context::Os::MacOs {
                    top = top.reason("Spotlight is macOS only");
                }
                let items = vec![
                    MenuItem::new("Analyze").shortcut("Enter"),
                    copy.separator(),
                    top,
                ];
                (r.path.clone(), items, "\nyt")
            }
            View::TopFiles => {
                let Some(r) = self.top.get(row) else {
                    return Outcome::Ignored;
                };
                let mut select = MenuItem::new(if self.selected.contains(&r.path) {
                    "Unselect"
                } else {
                    "Select"
                })
                .shortcut("Space");
                if r.missing {
                    select = select.reason("The file vanished");
                } else if let Some(why) = self.select_reason(w, &r.path) {
                    select = select.reason(why);
                }
                (
                    r.path.clone(),
                    vec![select, copy.separator(), review],
                    " yd",
                )
            }
        };
        let title = w.location.short(&path);
        open_context(
            cx,
            ROW_MENU,
            anchor,
            title,
            items,
            LayerTag::new("disk-row").key(format!("{row}:{keys}")),
        )
    }
}

fn short_error(e: &crate::sim::fs::FsError) -> &'static str {
    match e {
        crate::sim::fs::FsError::PermissionDenied(_) => "permission denied",
        crate::sim::fs::FsError::NotFound(_) => "missing",
        crate::sim::fs::FsError::Dataless(_) => "dataless",
        _ => "error",
    }
}

impl DiskPage {
    fn click(&mut self, ev: &PointerEvent, w: &mut World, cx: &mut Cx) -> Outcome {
        let (id, pos) = (ev.id, ev.pos());
        if id == scrollbar::id_for(TREE) {
            cx.focus.focus(TREE);
            return self.tree.pointer(ev).outcome();
        }
        if id == scrollbar::id_for(DETAIL) {
            cx.focus.focus(DETAIL);
            return scroll_press(self.detail_area, pos, &mut self.detail_scroll);
        }
        if id == scrollbar::id_for(LIST) {
            cx.focus.focus(LIST);
            return scroll_press(self.list_area, pos, &mut self.list_scroll);
        }
        if id == DETAIL {
            cx.focus.focus(DETAIL);
            return Outcome::Changed;
        }
        if self.view == View::Tree {
            if let Some(row) = self.tree.row_at(ev) {
                cx.focus.focus(TREE);

                if ev.index_of(Part::MARKER).is_some() {
                    self.tree.cursor = row;
                    if let Some(p) = self.fs_path_of(&self.tree.rows()[row].path.clone())
                        && p != self.path
                    {
                        self.toggle_select(&p, w, cx);
                    }
                    return Outcome::Changed;
                }
                return self.tree.pointer(ev).outcome().or(Outcome::Changed);
            }
            if id == TREE {
                cx.focus.focus(TREE);
                return Outcome::Changed;
            }
            return Outcome::Ignored;
        }
        if let Some(i) = self
            .list_scroll
            .visible_range()
            .find(|&i| LIST.child(i) == id)
        {
            self.cursor = i;
            cx.focus.focus(LIST);
            if self.view == View::TopFiles {
                return self.open_row(w, cx);
            }
            return Outcome::Changed;
        }
        if id == LIST {
            cx.focus.focus(LIST);
            return Outcome::Changed;
        }
        Outcome::Ignored
    }
}

impl DiskPage {
    fn main_id(&self) -> WidgetId {
        if self.view == View::Tree { TREE } else { LIST }
    }
}

impl ShellPage<Holla> for DiskPage {
    fn on_key(&mut self, key: &Key, w: &mut World, cx: &mut Cx) -> Outcome {
        self.md.master = self.main_id();
        if let Some(o) = self.md.on_key(key, cx.focus) {
            return o;
        }
        if cx.focus.is(DETAIL) {
            match key.code {
                KeyCode::Up | KeyCode::Char('k') if key.plain() => {
                    self.detail_scroll.scroll_by(-1);
                    return Outcome::Changed;
                }
                KeyCode::Down | KeyCode::Char('j') if key.plain() => {
                    self.detail_scroll.scroll_by(1);
                    return Outcome::Changed;
                }
                _ => {}
            }
        }
        if matches!(key.code, KeyCode::Char(_)) && key.chord() {
            return Outcome::Ignored;
        }

        match key.code {
            KeyCode::Char('p') if key.plain() => {
                self.md.master = self.main_id();
                self.md.toggle_drawer(cx.focus);
                return Outcome::Changed;
            }
            KeyCode::Char('y') if key.plain() => {
                let p = match self.view {
                    View::Tree => self.cursor_fs_path(),
                    View::Overview => self.overview.get(self.cursor).map(|r| r.path.clone()),
                    View::TopFiles => self.top.get(self.cursor).map(|r| r.path.clone()),
                };
                if let Some(p) = p {
                    cx.go(Go::Osc52(p));
                }
                return Outcome::Changed;
            }
            KeyCode::Char('t') | KeyCode::Char('T') if self.view != View::TopFiles => {
                if w.host.os != crate::domain::context::Os::MacOs {
                    cx.status("Top files is unavailable on Linux · Spotlight is macOS only · use the tree scan");
                    return Outcome::Changed;
                }
                cx.go(Go::Push(Page::TopFiles));
                return Outcome::Changed;
            }
            KeyCode::Char('d') | KeyCode::Backspace
                if key.plain() && self.view != View::Overview =>
            {
                if self.selected.is_empty() {
                    cx.status("Select at least one entry first · Space selects");
                    return Outcome::Changed;
                }
                let plan = self.build_plan(w, Mode::Trash, false);
                self.reviewed = Some(plan.revision);
                cx.go(Go::Push(Page::CleanupGate { plan }));
                return Outcome::Changed;
            }
            _ => {}
        }
        match self.view {
            View::Tree => {
                match key.code {
                    KeyCode::Char(' ') => {
                        if let Some(p) = self.cursor_fs_path() {
                            self.toggle_select(&p, w, cx);
                        }
                        return Outcome::Changed;
                    }
                    KeyCode::Char('a') => {
                        let visible: Vec<String> = self
                            .tree
                            .rows()
                            .iter()
                            .filter_map(|r| self.fs_path_of(&r.path))
                            .filter(|p| p != &self.path)
                            .collect();
                        let eligible: Vec<String> = visible
                            .into_iter()
                            .filter(|p| self.candidate_skip(w, p).is_none())
                            .collect();
                        let all = eligible.iter().all(|p| {
                            self.selected.contains(p)
                                || self
                                    .selected
                                    .iter()
                                    .any(|s| p.starts_with(&format!("{s}/")))
                        });
                        if all {
                            self.selected.clear();
                            cx.status("Everything unselected");
                        } else {
                            for p in eligible {
                                if !self
                                    .selected
                                    .iter()
                                    .any(|s| p.starts_with(&format!("{s}/")))
                                {
                                    self.selected.retain(|s| !s.starts_with(&format!("{p}/")));
                                    self.selected.insert(p);
                                }
                            }
                            cx.status(format!(
                                "Every visible entry selected · {}",
                                junie_tui::ui::text::bytes(self.selected_bytes(w))
                            ));
                        }
                        self.reviewed = None;
                        self.rebuild_tree(w);
                        return Outcome::Changed;
                    }
                    KeyCode::Char('s') => {
                        self.sort = match self.sort {
                            Sort::Allocated => Sort::Apparent,
                            Sort::Apparent => Sort::Allocated,
                        };
                        cx.status(match self.sort {
                            Sort::Allocated => "Sorted by allocated size",
                            Sort::Apparent => {
                                "Sorted by apparent size · rows still show allocated bytes"
                            }
                        });
                        self.rebuild_tree(w);
                        return Outcome::Changed;
                    }
                    KeyCode::Char('f') => {
                        self.fold = !self.fold;
                        cx.status(if self.fold {
                            "Noise folders folded · sizes unchanged"
                        } else {
                            "Noise folders unfolded"
                        });
                        self.rebuild_tree(w);
                        return Outcome::Changed;
                    }
                    KeyCode::Char('r') => {
                        let root = self.path.clone();
                        w.start_scan(&root);
                        self.selected.clear();
                        self.reviewed = None;
                        self.tree.expanded.clear();
                        self.last_revealed = usize::MAX;
                        self.rebuild_tree(w);
                        cx.status("Rescanning · cached hints stay until live sizes arrive");
                        return Outcome::Changed;
                    }
                    KeyCode::Char('x') => {
                        if w.cancel_scan() {
                            cx.status(
                                "Scan cancelled · the partial tree stays and is labelled partial",
                            );
                        } else {
                            cx.status("No scan is running");
                        }
                        self.rebuild_tree(w);
                        return Outcome::Changed;
                    }
                    KeyCode::Char('c') => {
                        let sel: Vec<String> = self
                            .selected
                            .iter()
                            .filter(|p| w.disk.candidates.iter().any(|c| &c.path == *p))
                            .cloned()
                            .collect();
                        if sel.is_empty() {
                            cx.status("Select rebuildable artifacts first · d reviews any selection as a deletion");
                        } else {
                            cx.go(Go::CleanupPlan(sel));
                        }
                        return Outcome::Changed;
                    }
                    _ => {}
                }
                if !cx.focus.is(TREE) {
                    return Outcome::Ignored;
                }
                let (o, actions) = self.tree.on_key(key).into_parts();
                if actions
                    .into_iter()
                    .any(|a| matches!(a, TreeAction::Activate(_)))
                {
                    return Outcome::Changed;
                }
                if o.consumed() {
                    self.detail_scroll.jump_start();
                    return o;
                }
                Outcome::Ignored
            }
            View::Overview | View::TopFiles => {
                let n = if self.view == View::Overview {
                    self.overview.len()
                } else {
                    self.top.len()
                };
                match key.code {
                    KeyCode::Up | KeyCode::Char('k') => {
                        self.cursor = self.cursor.saturating_sub(1);
                        self.detail_scroll.jump_start();
                        self.list_scroll.ensure_visible(self.cursor);
                        Outcome::Changed
                    }
                    KeyCode::Down | KeyCode::Char('j') => {
                        self.cursor = (self.cursor + 1).min(n.saturating_sub(1));
                        self.detail_scroll.jump_start();
                        self.list_scroll.ensure_visible(self.cursor);
                        Outcome::Changed
                    }
                    KeyCode::Home | KeyCode::Char('g') => {
                        self.cursor = 0;
                        self.list_scroll.jump_start();
                        Outcome::Changed
                    }
                    KeyCode::End | KeyCode::Char('G') => {
                        self.cursor = n.saturating_sub(1);
                        self.list_scroll.ensure_visible(self.cursor);
                        Outcome::Changed
                    }
                    KeyCode::Enter => self.open_row(w, cx),
                    KeyCode::Char(' ') if self.view == View::TopFiles => self.open_row(w, cx),
                    KeyCode::Esc => {
                        cx.go(Go::Pop);
                        Outcome::Changed
                    }
                    _ => Outcome::Ignored,
                }
            }
        }
    }

    fn pointer(&mut self, ev: &PointerEvent, w: &mut World, cx: &mut Cx) -> Outcome {
        let (id, pos) = (ev.id, ev.pos());
        if let Some((axis, delta)) = ev.wheel() {
            if axis != Axis::Vertical {
                return Outcome::Ignored;
            }
            if ev.on(TREE) {
                return self.tree.pointer(ev).outcome();
            }
            if id == LIST {
                self.list_scroll.scroll_by(delta as isize);
                return Outcome::Changed;
            }
            if id == DETAIL {
                self.detail_scroll.scroll_by(delta as isize);
                return Outcome::Changed;
            }
            return Outcome::Ignored;
        }
        match ev.phase() {
            Some(Phase::Press | Phase::Drag) => {
                let press = ev.is(Phase::Press);
                if id == scrollbar::id_for(TREE) {
                    return self.tree.pointer(ev).outcome();
                }
                let (area, scroll) = if id == scrollbar::id_for(DETAIL) {
                    (self.detail_area, &mut self.detail_scroll)
                } else if id == scrollbar::id_for(LIST) {
                    (self.list_area, &mut self.list_scroll)
                } else {
                    return Outcome::Ignored;
                };
                if press {
                    scroll_press(area, pos, scroll)
                } else {
                    scroll_drag(area, pos, scroll)
                }
            }
            Some(Phase::Click) => {
                if self.view == View::Overview
                    && let Some(i) = self
                        .list_scroll
                        .visible_range()
                        .find(|&i| LIST.child(i) == id)
                    && ClickPolicy::DATA.activates(ev.count(), self.cursor == i)
                {
                    self.cursor = i;
                    return self.open_row(w, cx);
                }
                self.click(ev, w, cx)
            }
            Some(Phase::Secondary) => {
                let row = if self.view == View::Tree {
                    let Some(req) = cx.context.on_secondary(&mut self.tree, ev) else {
                        return Outcome::Ignored;
                    };
                    cx.focus.focus(TREE);
                    req.target
                } else {
                    let Some(i) = self
                        .list_scroll
                        .visible_range()
                        .find(|&i| LIST.child(i) == id)
                    else {
                        return Outcome::Ignored;
                    };
                    if cx.context.moves_cursor {
                        self.cursor = i;
                    }
                    cx.focus.focus(LIST);
                    i
                };
                self.row_menu(row, ContextAnchor::Pointer(pos), w, cx)
            }
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
        if tag.kind == "disk-row"
            && let ModalResult::MenuChosen(k) = result
            && let Some((row, keys)) = tag.key.split_once(':')
            && let Ok(row) = row.parse::<usize>()
            && let Some(c) = keys.chars().nth(k)
        {
            if self.view == View::Tree {
                self.tree.context_select(row);
            } else {
                self.cursor = row;
            }
            let code = if c == '\n' {
                KeyCode::Enter
            } else {
                KeyCode::Char(c)
            };
            let key = Key {
                code,
                mods: KeyModifiers::NONE,
            };
            return self.on_key(&key, w, cx).or(Outcome::Changed);
        }
        Outcome::Changed
    }

    fn on_tick(&mut self, w: &mut World, _cx: &mut Cx) -> Outcome {
        if w.tick == self.last_tick {
            return Outcome::Ignored;
        }
        self.last_tick = w.tick;
        match self.view {
            View::Tree => {
                let Some(scan) = &w.scan else {
                    return Outcome::Ignored;
                };
                let revealed = scan.revealed(w.tick);
                let generation = scan.generation;
                let phase = scan.phase();
                if revealed != self.last_revealed
                    || generation != self.generation
                    || Some(phase) != self.last_phase
                {
                    self.last_revealed = revealed;
                    self.generation = generation;
                    self.last_phase = Some(phase);
                    self.seed_selection(w);
                    self.rebuild_tree(w);
                    return Outcome::Changed;
                }
                Outcome::Ignored
            }
            View::Overview => {
                self.rebuild_overview(w);
                Outcome::Ignored
            }
            View::TopFiles => Outcome::Ignored,
        }
    }

    fn enter(&mut self, w: &mut World, cx: &mut Cx) {
        match self.view {
            View::Tree => {
                if w.scan.as_ref().is_none_or(|s| s.root != self.path) {
                    let p = self.path.clone();
                    w.start_scan(&p);
                }
                self.seed_selection(w);
                self.rebuild_tree(w);
                cx.focus.focus(TREE);
            }
            View::Overview => {
                self.rebuild_overview(w);
                cx.focus.focus(LIST);
            }
            View::TopFiles => {
                self.rebuild_top(w);
                cx.focus.focus(LIST);
            }
        }
    }

    fn render(&mut self, area: Rect, buf: &mut Buffer, ctx: &mut RenderCtx, w: &World) {
        let t = ctx.theme;
        let title = match self.view {
            View::Tree => format!("Disk usage · {}", w.location.short(&self.path)),
            View::Overview => "Disk overview".to_owned(),
            View::TopFiles => "Top files · Spotlight".to_owned(),
        };
        buf.set_string(area.x + 1, area.y, &title, t.title());
        let y1 = area.y + 1;
        match self.view {
            View::Tree => {
                if let Some(scan) = &w.scan {
                    let errors = scan.tree.errors();
                    if scan.cancelled() {
                        let line = format!(
                            "▲ partial · scan cancelled at {}% · {} entries · {} selected ({})",
                            (scan.progress(w.tick) * 100.0).round(),
                            scan.revealed(w.tick),
                            self.selected.len(),
                            junie_tui::ui::text::bytes(self.selected_bytes(w))
                        );
                        buf.set_string(
                            area.x + 1,
                            y1,
                            junie_tui::ui::reveal::cut(
                                Rect::new(area.x + 1, y1, area.width.saturating_sub(2), 1),
                                &line,
                            ),
                            Style::new().fg(t.warning),
                        );
                    } else if scan.is_running() {
                        let label = format!(
                            "scanning · {} of {} entries",
                            scan.revealed(w.tick),
                            scan.order.len()
                        );
                        render_spinner(
                            Rect::new(area.x + 1, y1, area.width.saturating_sub(2), 1),
                            buf,
                            ctx,
                            &label,
                            t.canvas,
                        );
                        let bx = area.x + 4 + width(&label) as u16;
                        render_bar(
                            Rect::new(bx, y1, 32.min(area.width.saturating_sub(bx + 2)), 1),
                            buf,
                            ctx,
                            "",
                            scan.progress(w.tick),
                            ProgressStatus::Active,
                            t.canvas,
                        );
                    } else {
                        let mut line = format!(
                            "scan complete · {} allocated · {} entries · {} selected ({})",
                            junie_tui::ui::text::bytes(scan.tree.allocated),
                            scan.tree.entries,
                            self.selected.len(),
                            junie_tui::ui::text::bytes(self.selected_bytes(w))
                        );
                        if errors > 0 {
                            line = format!(
                                "▲ {} unreadable · grant Full Disk Access to include them · {line}",
                                errors
                            );
                        }
                        if let Some(r) = &w.disk.partial_reason {
                            line = format!("▲ partial · {r} · {line}");
                        }
                        buf.set_string(
                            area.x + 1,
                            y1,
                            junie_tui::ui::reveal::cut(
                                Rect::new(area.x + 1, y1, area.width.saturating_sub(2), 1),
                                &line,
                            ),
                            Style::new().fg(if errors > 0 || w.disk.partial_reason.is_some() {
                                t.warning
                            } else {
                                t.text_muted
                            }),
                        );
                    }
                }
            }
            View::Overview => {
                buf.set_string(area.x + 1, y1, junie_tui::ui::reveal::cut(Rect::new(area.x + 1, y1, area.width.saturating_sub(2), 1), "home folders alphabetically, then insight roots · cached sizes are hints with their age · Enter analyses live"), t.muted());
            }
            View::TopFiles => {
                buf.set_string(area.x + 1, y1, junie_tui::ui::reveal::cut(Rect::new(area.x + 1, y1, area.width.saturating_sub(2), 1), &format!("{} · regular files of 100 MiB or more · top 50 by allocated size · Space selects for the same cleanup gate", self.top_state.clone().unwrap_or(format!("{} files", self.top.len())))), t.muted());
            }
        }
        let mut body_y = area.y + 3;

        if self.view == View::Tree && !w.disk.filesystems.is_empty() {
            Heading::new("Filesystems").render(
                Rect::new(area.x + 3, body_y, area.width.saturating_sub(3), 1),
                buf,
                t,
                t.canvas,
            );
            body_y += 1;
            let meters = MeterList::new(
                w.disk
                    .filesystems
                    .iter()
                    .map(|f| {
                        MeterRow::new(
                            f.mount.clone(),
                            Meter::new(Some(f.pct()))
                                .value(format!("{}%", f.pct()))
                                .tone(FS_METER)
                                .visual(MeterVisual::Line),
                        )
                        .detail(format!("{} of {} GB", f.used_gb, f.total_gb))
                    })
                    .collect(),
            )
            .label_width(22)
            .label_tone(Tone::Secondary)
            .pad_label(true)
            .label_gap(2)
            .meter_width(24)
            .reserve(4)
            .min_meter(8);
            meters.render(
                Rect::new(
                    area.x + 3,
                    body_y,
                    area.width.saturating_sub(3),
                    meters.height(),
                ),
                buf,
                ctx,
                t.canvas,
            );
            body_y += meters.height() + 1;
        }
        let body = Rect::new(
            area.x,
            body_y,
            area.width,
            area.bottom().saturating_sub(body_y),
        );
        let render_main =
            |me: &mut Self, r: Rect, buf: &mut Buffer, ctx: &mut RenderCtx| match me.view {
                View::Tree => {
                    Heading::new(&format!(
                        "largest first · {} · {}",
                        if me.sort == Sort::Allocated {
                            "allocated"
                        } else {
                            "apparent"
                        },
                        if me.fold { "noise folded" } else { "unfolded" }
                    ))
                    .render(
                        Rect::new(r.x + 3, r.y, r.width.saturating_sub(3), 1),
                        buf,
                        ctx.theme,
                        ctx.theme.canvas,
                    );
                    me.tree.render(
                        Rect::new(r.x, r.y + 1, r.width, r.height.saturating_sub(1)),
                        buf,
                        ctx,
                        ctx.theme.canvas,
                    );
                }
                _ => me.render_list(r, buf, ctx, w),
            };

        self.md.master = self.main_id();
        let l = self.md.layout(body, buf, ctx, ctx.theme.canvas);
        if !l.master.is_empty() {
            render_main(self, l.master, buf, ctx);
        }
        if !l.detail.is_empty() {
            self.render_detail(l.detail, buf, ctx, w);
        }
        self.md.finish(ctx);
    }

    fn hints(&self, focus: Option<WidgetId>, _w: &World) -> Vec<Hint> {
        if focus == Some(DETAIL) {
            return vec![hint("↑↓", "Scroll"), hint("Esc", "Back to list")];
        }
        match self.view {
            View::Tree => vec![
                hint("↑↓", "Move"),
                hint("←→", "Fold / open"),
                hint("Space", "Select"),
                hint("s", "Sort"),
                hint("f", "Fold noise"),
                hint("d", "Delete…"),
                hint("c", "Artifact plan"),
                hint("r", "Rescan"),
                hint("x", "Cancel scan"),
                hint("t", "Top files"),
                hint("p", "Details"),
                hint("Esc", "Back"),
                junie_tui::widgets::keyhint::CONTEXT_MENU,
            ],
            View::Overview => vec![
                hint("↑↓", "Move"),
                hint("Enter", "Analyze"),
                hint("t", "Top files"),
                hint("p", "Details"),
                hint("Esc", "Back"),
                junie_tui::widgets::keyhint::CONTEXT_MENU,
            ],
            View::TopFiles => vec![
                hint("↑↓", "Move"),
                hint("Space", "Select"),
                hint("d", "Delete…"),
                hint("y", "Copy path"),
                hint("Esc", "Back"),
                junie_tui::widgets::keyhint::CONTEXT_MENU,
            ],
        }
    }

    fn crumb(&self, _w: &World) -> String {
        match self.view {
            View::Tree => "Disk › Usage".into(),
            View::Overview => "Disk › Overview".into(),
            View::TopFiles => "Disk › Top files".into(),
        }
    }

    fn animating(&self, w: &World) -> bool {
        self.view == View::Tree && w.scan.as_ref().is_some_and(|s| s.is_running())
    }

    fn primary_focus(&self) -> Option<WidgetId> {
        Some(if self.view == View::Tree { TREE } else { LIST })
    }
}

impl Screen for DiskPage {
    fn on_context_key(&mut self, w: &mut World, cx: &mut Cx) -> Outcome {
        if self.view == View::Tree {
            if !cx.focus.is(TREE) {
                return Outcome::Ignored;
            }
            let Some(req) = self.tree.context_cursor() else {
                return Outcome::Ignored;
            };
            return self.row_menu(req.target, req.anchor, w, cx);
        }
        if !cx.focus.is(LIST) {
            return Outcome::Ignored;
        }
        let anchor = row_anchor(LIST, self.cursor, self.list_area, self.list_scroll.offset);
        self.row_menu(self.cursor, anchor, w, cx)
    }

    fn status(&self, w: &World) -> StatusBits {
        let (text, busy) = match (&self.view, &w.scan) {
            (View::Tree, Some(s)) if s.is_running() => (
                format!(
                    "scanning {} · {}%",
                    w.location.short(&self.path),
                    (s.progress(w.tick) * 100.0).round()
                ),
                true,
            ),
            (View::Tree, _) => (
                format!(
                    "{} selected · {}",
                    self.selected.len(),
                    junie_tui::ui::text::bytes(self.selected_bytes(w))
                ),
                false,
            ),
            (View::Overview, _) => (format!("{} roots", self.overview.len()), false),
            (View::TopFiles, _) => (
                format!(
                    "{} files · {} selected",
                    self.top.len(),
                    self.selected.len()
                ),
                false,
            ),
        };
        let mut item = StatusItem::new(text, Tone::Secondary).priority(6);
        if busy {
            item = item.busy();
        }
        StatusBits {
            center: Some(item),
            right: vec![],
        }
    }
}

#[cfg(test)]
mod scan_status_tests {
    use super::*;
    use crate::domain::fixtures::world_for;
    use crate::scenario::{Motion, Scenario};
    use crate::sim::world::scan_test_support::{run_to_completion, scan_done_count, scan_world};

    fn home_scan(scan_ticks: u64) -> World {
        let mut w = world_for(Scenario::DiskCleanup, Motion::Full);
        w.clock.running = true;
        w.disk.scan_ticks = scan_ticks;
        let home = w.location.home.clone();
        w.start_scan(&home);
        w
    }

    #[test]
    fn disk_page_animates_until_the_completing_tick() {
        let mut w = scan_world(4096, 1);
        let page = DiskPage::new("/work/big");
        w.tick();
        assert!(
            page.animating(&w),
            "the first batch leaves entries unscanned"
        );
        let msgs = run_to_completion(&mut w);
        assert_eq!(scan_done_count(&msgs), 1);
        assert!(!page.animating(&w));
    }

    #[test]
    fn completed_scan_preselects_candidates_and_cancelled_scan_does_not() {
        let mut w = home_scan(1);
        let home = w.location.home.clone();
        let mut page = DiskPage::new(&home);
        run_to_completion(&mut w);
        page.seed_selection(&w);
        assert!(
            !page.selected.is_empty(),
            "a completed scan seeds its default candidates"
        );

        let mut w = home_scan(50);
        let mut page = DiskPage::new(&home);
        w.tick();
        assert!(w.cancel_scan());
        page.seed_selection(&w);
        assert!(
            page.selected.is_empty(),
            "a cancelled partial scan never preselects"
        );
    }
}
