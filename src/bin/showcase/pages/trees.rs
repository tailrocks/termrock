use ratatui::buffer::Buffer;
use ratatui::layout::Rect;

use crate::app::PageId;
use crate::options::page_options;
use crate::pages::row_menu::{MENU_HINTS, OPEN_HINT, RowMenu, plain};
use crate::pages::template::{Section, Template};
use crate::pages::{Cx, Hint, Page, PageEvent};
use junie_tui::core::event::Outcome;
use junie_tui::core::id::WidgetId;
use junie_tui::core::intent::Phase;
use junie_tui::core::keymap::{KeyBinding, NavKeys, hint_key};
use junie_tui::core::pointer::MouseTarget;
use junie_tui::core::scroll::TrackClick;
use junie_tui::core::selection::ClickPolicy;
use junie_tui::theme::Tone;
use junie_tui::ui::context::ContextRequest;
use junie_tui::ui::ctx::RenderCtx;
use junie_tui::ui::specimen::{SpecimenState, StateGrid};
use junie_tui::ui::text;
use junie_tui::widgets::menu::MenuItem;
use junie_tui::widgets::panel::Panel;
use junie_tui::widgets::scrollbar;
use junie_tui::widgets::tree::{BranchAction, TreeAction, TreeCmd, TreeNode, TreeView};
use ratatui::crossterm::event::KeyCode;

const ID: WidgetId = WidgetId::of("trees");
const MENU: WidgetId = WidgetId::of("trees.menu");

pub const PROJECT: WidgetId = ID.sub("tree");
pub const CONNECTIONS: WidgetId = ID.sub("connections");

const LOAD_TICKS: u8 = 4;

fn connections() -> Vec<TreeNode> {
    vec![
        TreeNode::dir(
            "production",
            vec![
                TreeNode::lazy("orders-db").meta("postgres"),
                TreeNode::lazy("billing-db").meta("postgres"),
                TreeNode::leaf_meta("replica-eu", "lag 4.2 s").label_tone(Tone::Warning),
            ],
        )
        .glyph("◆")
        .glyph_tone(Tone::Danger),
        TreeNode::dir(
            "staging",
            vec![TreeNode::lazy("orders-db").meta("postgres")],
        )
        .glyph("◇")
        .glyph_tone(Tone::Warning),
        TreeNode::dir("archive", vec![TreeNode::note("No connections")]).glyph("○"),
    ]
}

fn tables() -> Vec<TreeNode> {
    vec![
        TreeNode::leaf_meta("customers", "48k rows"),
        TreeNode::leaf_meta("orders", "1.2M rows"),
        TreeNode::leaf_meta("refunds", "3k rows"),
    ]
}

pub struct TreesPage {
    template: Template,
    tree: TreeView,
    conns: TreeView,

    loading: Option<(Vec<usize>, u8)>,

    last: String,

    menu: RowMenu<usize>,

    opts: Vec<usize>,
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum RowAction {
    Open,
    Fold,
    ExpandAll,
    CollapseAll,
}

impl TreesPage {
    pub fn new() -> Self {
        let mut page = Self {
            template: Template::new(ID, states_grid()),
            tree: TreeView::new(PROJECT, crate::data::project_tree()),
            conns: TreeView::new(CONNECTIONS, connections()),
            loading: None,
            last: "—".into(),
            menu: RowMenu::default(),
            opts: page_options(PageId::Trees)
                .iter()
                .map(|o| o.default)
                .collect(),
        };
        page.conns.expanded.insert(vec![0]);
        page.conns.flatten();
        page.rebuild();
        page
    }

    #[allow(dead_code)]
    pub fn conns(&self) -> &TreeView {
        &self.conns
    }

    fn opt(&self, key: &str) -> &'static str {
        let table = page_options(PageId::Trees);
        let i = table
            .iter()
            .position(|o| o.key == key)
            .unwrap_or_else(|| panic!("no option {key}"));
        table[i].values[self.opts[i]]
    }

    fn build(&self, t: TreeView) -> TreeView {
        let mut t = t
            .branch_activate(match self.opt("trees.branch") {
                "activate" => BranchAction::Activate,
                "both" => BranchAction::Both,
                _ => BranchAction::Toggle,
            })
            .click(match self.opt("trees.click") {
                "single" => ClickPolicy::NAVIGATION,
                _ => ClickPolicy::DATA,
            })
            .indent(self.opt("trees.indent").parse().unwrap_or(2))
            .divider(self.opt("trees.divider") == "on")
            .alt_click_recursive(self.opt("trees.alt_click") == "on")
            .reserve_right(self.opt("trees.reserve_right").parse().unwrap_or(0))
            .show_meta(self.opt("trees.show_meta") == "on")
            .track_click(match self.opt("trees.track_click") {
                "jump" => TrackClick::Jump,
                _ => TrackClick::Page,
            })
            .nav(match self.opt("trees.nav") {
                "vim" => NavKeys::VIM,
                _ => NavKeys::default(),
            })
            .wrap(self.opt("trees.wrap") == "on")
            .type_ahead(self.opt("trees.type_ahead") == "on")
            .selected_marker(match self.opt("trees.marker") {
                "chevron" => Some("›"),
                _ => None,
            });
        if self.opt("trees.keymap") == "custom" {
            t = t.keymap(vec![
                (KeyBinding::char('e'), TreeCmd::ExpandAll),
                (KeyBinding::char('c'), TreeCmd::CollapseAll),
            ]);
        }
        t
    }

    fn rebuild(&mut self) {
        let keep = |old: &TreeView, new: TreeView| {
            let mut new = new;
            new.nodes = old.nodes.clone();
            new.expanded = old.expanded.clone();
            new.selected = old.selected.clone();
            new.cursor = old.cursor;
            new.flatten();
            new
        };
        let tree = self.build(TreeView::new(PROJECT, vec![]));
        self.tree = keep(&self.tree, tree);
        let mut conns = self.build(TreeView::new(CONNECTIONS, vec![]));
        conns.filter = match self.opt("trees.filter") {
            "orders" => Some("orders".to_owned()),
            _ => None,
        };
        self.conns = keep(&self.conns, conns);
    }

    fn tree_mut(&mut self, id: WidgetId) -> Option<&mut TreeView> {
        [&mut self.tree, &mut self.conns]
            .into_iter()
            .find(|t| t.id == id)
    }

    fn on_action(&mut self, id: WidgetId, a: Option<TreeAction>, cx: &mut Cx) {
        let Some(a) = a else { return };
        let t = if id == CONNECTIONS {
            &mut self.conns
        } else {
            &mut self.tree
        };
        let name =
            |t: &TreeView, p: &[usize]| t.node(p).map(|n| n.label.clone()).unwrap_or_default();
        match a {
            TreeAction::Expand(p) => {
                self.last = format!("Expand({})", name(t, &p));

                t.set_busy(&p, true);
                self.loading = Some((p, LOAD_TICKS));
            }
            TreeAction::Activate(p) => {
                self.last = format!("Activate({})", name(t, &p));
                cx.status(format!("Opened {}", name(t, &p)));
            }
        }
    }

    fn readout(&self) -> Vec<String> {
        let t = &self.tree;
        let keys = format!(
            "{} TreeCmd::ExpandAll · {} TreeCmd::CollapseAll",
            hint_key(&t.keymap, TreeCmd::ExpandAll).unwrap_or("none"),
            hint_key(&t.keymap, TreeCmd::CollapseAll).unwrap_or("none"),
        );
        vec![
            format!(
                "branch_activate {:?} · click {:?} · indent {} · divider {} · alt_click_recursive {} · reserve_right {} · show_meta {}",
                t.branch_activate,
                t.click.activate,
                t.indent,
                t.divider,
                t.alt_click_recursive,
                t.reserve_right,
                t.show_meta,
            ),
            format!(
                "nav {} · wrap {} · type-ahead {} · track_click {:?} · selected_marker {:?} · {keys}",
                if t.nav == NavKeys::VIM {
                    "vim"
                } else {
                    "default"
                },
                t.wrap,
                if t.type_ahead { "on" } else { "off" },
                t.scroll.track_click.unwrap_or_default(),
                t.selected_marker,
            ),
            format!(
                "last TreeAction: {}   (TreeAction::Expand on a lazy node · TreeAction::Activate on a leaf, or a branch per branch_activate)",
                self.last
            ),
        ]
    }

    fn row_items(&self, i: usize) -> Vec<(RowAction, MenuItem)> {
        let row = &self.tree.rows()[i];
        let first = if row.has_children {
            let (label, key) = if row.expanded {
                ("Collapse", "←")
            } else {
                ("Expand", "→")
            };
            (RowAction::Fold, MenuItem::new(label).shortcut(key))
        } else {
            let open = MenuItem::new("Open").shortcut("Enter");
            let open = if row.note {
                open.reason("Not a file")
            } else if self.tree.selected.as_ref() == Some(&row.path) {
                open.reason("Already open")
            } else {
                open
            };
            (RowAction::Open, open)
        };
        let shortcut = |m: MenuItem, c| match hint_key(&self.tree.keymap, c) {
            Some(k) => m.shortcut(k),
            None => m,
        };
        let collapse = shortcut(MenuItem::new("Collapse all"), TreeCmd::CollapseAll);
        vec![
            (first.0, first.1.separator()),
            (
                RowAction::ExpandAll,
                shortcut(MenuItem::new("Expand all"), TreeCmd::ExpandAll),
            ),
            (
                RowAction::CollapseAll,
                if self.tree.expanded.is_empty() {
                    collapse.reason("All folded")
                } else {
                    collapse
                },
            ),
        ]
    }

    fn open_menu(&mut self, req: ContextRequest<usize>) {
        let row = req.target;
        let title = self.tree.rows()[row].label.clone();
        let items = self.row_items(row).into_iter().map(|(_, m)| m).collect();
        self.menu.open(req.menu(MENU, items).title(title), row);
    }

    fn choose(&mut self, row: usize, i: usize) -> Outcome {
        let Some(&(action, _)) = self.row_items(row).get(i) else {
            return Outcome::Consumed;
        };
        self.tree.cursor = row;
        match action {
            RowAction::Open => self.tree.on_key(&plain(KeyCode::Enter)).0,
            RowAction::Fold => self.tree.toggle(row).0,
            RowAction::ExpandAll => {
                self.tree.expand_all();
                Outcome::Changed
            }
            RowAction::CollapseAll => {
                self.tree.collapse_all();
                Outcome::Changed
            }
        }
    }

    fn conns_h(&self) -> u16 {
        self.conns.rows().len() as u16 + 3
    }

    fn top_h(&self) -> u16 {
        TREE_H.max(SELECTION_H + 1 + self.conns_h())
    }

    fn readout_lines(&self, width: u16) -> Vec<String> {
        self.readout()
            .iter()
            .flat_map(|l| text::wrap(l, width.max(1) as usize))
            .collect()
    }

    fn render_playground(&mut self, area: Rect, buf: &mut Buffer, ctx: &mut RenderCtx) {
        let t = ctx.theme;
        let (l, r) = crate::pages::layout::columns(area, (area.width * 3 / 5).max(30), 2);
        let panel = Panel::card(Some("Project")).focused(ctx.interaction.focused(self.tree.id));
        let bg = panel.bg(t);
        let card = Rect::new(l.x, l.y, l.width, l.height.min(TREE_H));
        let inner = panel.render(card, buf, t);
        self.tree.render(inner, buf, ctx, bg);
        panel.draw_meta(card, buf, t, &scrollbar::position_label(&self.tree.scroll));

        let panel = Panel::card(Some("Selection"));
        let bg = panel.bg(t);
        let inner = panel.render(
            Rect::new(r.x, r.y, r.width, r.height.min(SELECTION_H)),
            buf,
            t,
        );
        let mut y = inner.y;
        match self.tree.selected.clone() {
            Some(path) => {
                let mut nodes = &self.tree.nodes;
                let mut parts = Vec::new();
                for &i in &path {
                    parts.push(nodes[i].label.clone());
                    nodes = &nodes[i].children;
                }
                buf.set_string(
                    inner.x,
                    y,
                    text::truncate(&parts.join("/"), inner.width as usize),
                    t.primary().bg(bg),
                );
                y += 1;
                buf.set_string(
                    inner.x,
                    y,
                    format!("depth {}", path.len() - 1),
                    t.muted().bg(bg),
                );
            }
            None => {
                buf.set_string(inner.x, y, "Nothing selected", t.muted().bg(bg));
                y += 1;
                buf.set_string(inner.x, y, "Enter on a file selects it", t.faint().bg(bg));
            }
        }
        y += 2;
        let rows = self.tree.rows();
        let cur = &rows[self.tree.cursor.min(rows.len().saturating_sub(1))];
        for (k, v) in [
            ("cursor", cur.label.clone()),
            ("visible", format!("{} rows", rows.len())),
            ("open", format!("{} folders", self.tree.expanded.len())),
        ] {
            if y >= inner.bottom() {
                break;
            }
            buf.set_string(inner.x, y, k, t.faint().bg(bg));
            buf.set_string(
                inner.x + 8,
                y,
                text::truncate(&v, inner.width.saturating_sub(8) as usize),
                t.secondary().bg(bg),
            );
            y += 1;
        }

        let top = r.y + r.height.min(SELECTION_H) + 1;
        if top + 4 < r.bottom() {
            let meta = if self.loading.is_some() {
                "loading…"
            } else {
                "→ loads a database"
            };
            let panel = Panel::card(Some("Connections"))
                .meta(meta)
                .focused(ctx.interaction.focused(self.conns.id));
            let bg = panel.bg(t);
            let card = Rect::new(
                r.x,
                top,
                r.width,
                (card.bottom().saturating_sub(top)).max(self.conns_h()),
            );
            let inner = panel.render(card, buf, t);
            self.conns.render(inner, buf, ctx, bg);
        }

        for (y, l) in (area.y + self.top_h() + 1..).zip(self.readout_lines(area.width)) {
            buf.set_string(area.x, y, l, t.faint().bg(t.canvas));
        }
        self.menu.render(area, buf, ctx);
    }
}

const TREE_H: u16 = 18;

const SELECTION_H: u16 = 10;

const MIN_W: u16 = 76;

const ROW_CHROME: u16 = 12;

fn states_grid() -> StateGrid {
    use SpecimenState::*;
    let id = ID.sub("states");
    let mut files = TreeView::new(
        id.child(0),
        vec![
            TreeNode::dir(
                "src",
                vec![
                    TreeNode::leaf_meta("main.rs", "1.2 KB"),
                    TreeNode::leaf_meta("lib.rs", "640 B"),
                ],
            ),
            TreeNode::lazy("target"),
        ],
    );
    files.expanded.insert(vec![0]);
    files.flatten();
    let mut envs = TreeView::new(id.child(1), connections());
    envs.expanded.insert(vec![0]);
    envs.flatten();
    StateGrid::new(id)
        .rows(&[
            Rest,
            Hover,
            Pressed,
            Focused,
            FocusedHover,
            Selected,
            Busy,
            Loading,
            Error,
            Overflow,
            Narrow,
        ])
        .column_width(30)
        .column("Files", files)
        .column("Connections", envs)
}

impl Page for TreesPage {
    fn title(&self) -> &'static str {
        "Trees"
    }
    fn blurb(&self) -> &'static str {
        "Indent carries hierarchy; the focus bar never moves"
    }

    fn measure(&self, avail: ratatui::layout::Size, _ctx: &RenderCtx) -> ratatui::layout::Size {
        let row = self
            .conns
            .rows()
            .iter()
            .map(|r| {
                r.depth as u16 * 2
                    + text::width(&r.label) as u16
                    + r.meta.as_ref().map_or(0, |m| text::width(m) as u16 + 2)
            })
            .max()
            .unwrap_or(0)
            + ROW_CHROME;
        let w = avail.width.max(MIN_W).max((row + 2) * 5 / 2 + 1);
        let h = self.top_h() + 1 + self.readout_lines(w).len() as u16;
        ratatui::layout::Size::new(w, h)
    }

    fn render(&mut self, area: Rect, buf: &mut Buffer, ctx: &mut RenderCtx) {
        let body = self.template.render(area, buf, ctx);
        match self.template.section() {
            Section::Playground => self.render_playground(body, buf, ctx),
            Section::Options => self.template.render_options(body, buf, ctx),
            Section::States => self.template.render_states(body, buf, ctx),
        }
    }

    fn layer_open(&self) -> bool {
        self.menu.hides_page_focus() || self.template.popup_open()
    }

    fn animating(&self) -> bool {
        self.loading.is_some()
    }

    fn handle(&mut self, ev: &PageEvent, cx: &mut Cx) -> Outcome {
        if let PageEvent::Tick = ev
            && let Some((path, left)) = self.loading.as_mut()
        {
            *left = left.saturating_sub(1);
            if *left == 0 {
                let path = path.clone();
                self.loading = None;

                for t in [&mut self.tree, &mut self.conns] {
                    if t.node(&path).is_some_and(|n| n.busy) {
                        t.set_children(&path, tables());
                    }
                }
            }
            return Outcome::Changed;
        }
        if let Some((out, chosen)) = self.menu.handle(ev) {
            return match chosen {
                Some((row, i)) => self.choose(row, i).or(out),
                None => out,
            };
        }
        if let Some(o) = self.template.handle(ev, cx) {
            return o;
        }
        match ev {
            PageEvent::Key(key) => {
                if cx.focus.is(self.tree.id)
                    && let Some(req) = self.menu.config.on_key(&self.tree, key)
                {
                    self.open_menu(req);
                    return Outcome::Changed;
                }
                let Some(id) = cx.focus.current() else {
                    return Outcome::Ignored;
                };
                let Some(t) = self.tree_mut(id) else {
                    return Outcome::Ignored;
                };
                let (o, a) = t.on_key(key);
                self.on_action(id, a, cx);
                o
            }
            PageEvent::Pointer(ev) if ev.is(Phase::Secondary) => {
                let cfg = self.menu.config;
                let Some(req) = cfg.on_secondary(&mut self.tree, ev) else {
                    return Outcome::Ignored;
                };
                cx.focus.focus(self.tree.id);
                self.open_menu(req);
                Outcome::Changed
            }
            PageEvent::Pointer(ev) => {
                for tid in [PROJECT, CONNECTIONS] {
                    let Some(t) = self.tree_mut(tid) else {
                        continue;
                    };

                    let Some(r) = t.route(ev) else {
                        continue;
                    };
                    if ev.is(Phase::Click) && t.row_at(ev).is_some() {
                        cx.focus.focus(tid);
                    }
                    let (o, a) = r.into_parts();
                    self.on_action(tid, a, cx);
                    return o;
                }
                Outcome::Ignored
            }
            _ => Outcome::Ignored,
        }
    }

    fn hints(&self, focus: Option<WidgetId>) -> Vec<Hint> {
        if self.menu.is_open() {
            return MENU_HINTS.to_vec();
        }
        if let Some(h) = self.template.hints(focus) {
            return h;
        }

        let t = if focus == Some(self.conns.id) {
            &self.conns
        } else {
            &self.tree
        };
        let mut hints = Vec::new();
        if let Some(k) = t.nav.move_hint(t.type_ahead) {
            hints.push((k, "Move"));
        }
        if t.nav.arrows {
            hints.push(("←→", "Fold / unfold"));
        }
        hints.push(("Enter", "Open"));
        if let Some(k) = t.nav.ends_hint(t.type_ahead) {
            hints.push((k, "Ends"));
        }
        if let Some(k) = hint_key(&t.keymap, TreeCmd::ExpandAll) {
            hints.push((k, "Expand all"));
        }
        if focus == Some(self.tree.id) {
            hints.push(OPEN_HINT);
        }
        hints
    }

    fn option_value(&self, key: &str) -> Option<usize> {
        let i = page_options(PageId::Trees)
            .iter()
            .position(|o| o.key == key)?;
        Some(self.opts[i])
    }

    fn set_option(&mut self, key: &str, value: usize) {
        let table = page_options(PageId::Trees);
        if let Some(i) = table.iter().position(|o| o.key == key) {
            self.opts[i] = value.min(table[i].values.len() - 1);
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
