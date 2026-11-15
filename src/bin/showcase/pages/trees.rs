use ratatui::buffer::Buffer;
use ratatui::layout::Rect;

use crate::app::PageId;
use crate::options::page_options;
use crate::pages::row_menu::{MENU_HINTS, OPEN_HINT, RowMenu, plain};
use crate::pages::template::{Section, Template};
use crate::pages::{Cx, Hint, Page, PageEvent};
use junie_tui::core::event::Outcome;
use junie_tui::core::id::{ItemKey, Part, WidgetId};
use junie_tui::core::intent::Phase;
use junie_tui::core::keymap::{Actions as KeyActions, KeyBinding, NavKeys, hint_key};
use junie_tui::core::pointer::MouseTarget;
use junie_tui::core::response::{Actions, Response};
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
use junie_tui::widgets::tree::{
    BranchAction, KeyedTree, KeyedTreeNode, TreeAction, TreeCmd, TreeNode, TreeView,
};
use ratatui::crossterm::event::KeyCode;

fn push_step(last: &mut String, step: String) {
    if !last.is_empty() {
        last.push_str(" → ");
    }
    last.push_str(&step);
}

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

/// The project files as a keyed store: every node is identified by its path,
/// so the tree keeps its cursor, selection and folds across edits.
fn keyed_project() -> KeyedTree {
    fn add(tree: &mut KeyedTree, parent: Option<ItemKey>, prefix: &str, nodes: &[TreeNode]) {
        for (at, node) in nodes.iter().enumerate() {
            let path = format!("{prefix}{}", node.label);
            let key = ItemKey::text(&path);
            let mut leaf = KeyedTreeNode::new(&node.label);
            if let Some(meta) = &node.meta {
                leaf = leaf.meta(meta);
            }
            tree.insert(parent, at, key, leaf)
                .expect("the demo paths are unique");
            add(tree, Some(key), &format!("{path}/"), &node.children);
        }
    }
    let mut tree = KeyedTree::new();
    let files = crate::data::project_tree();
    add(&mut tree, None, "", &files);
    tree.insert(None, files.len(), TARGET, KeyedTreeNode::lazy("target"))
        .expect("the demo paths are unique");
    tree
}

const TARGET: ItemKey = ItemKey::text("target");
const MAIN: ItemKey = ItemKey::text("src/main.rs");
const NEW_FILE: ItemKey = ItemKey::text("src/new.rs");
const FIXTURES: ItemKey = ItemKey::text("tests/fixtures");

pub struct TreesPage {
    template: Template,
    tree: TreeView,
    conns: TreeView,

    /// The keyed store behind the Project tree when its source is keyed.
    store: KeyedTree,
    was_keyed: bool,
    /// What the last store edit and the last jump reported.
    edit_note: String,
    jump_note: String,

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
            store: keyed_project(),
            was_keyed: false,
            edit_note: String::new(),
            jump_note: String::new(),
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
        let t = t
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
        t.keymap(if self.opt("trees.keymap") == "custom" {
            vec![
                (KeyBinding::char('e'), TreeCmd::ExpandAll),
                (KeyBinding::char('c'), TreeCmd::CollapseAll),
            ]
        } else {
            TreeCmd::default_keymap()
        })
    }

    /// Whether the Project tree runs over the keyed store.
    fn keyed(&self) -> bool {
        self.opt("trees.source") == "keyed"
    }

    fn rebuild(&mut self) {
        let keyed = self.keyed();
        let tree = if keyed && self.was_keyed {
            // The keyed state (cursor, folds, selection) lives in the view.
            let live = std::mem::replace(&mut self.tree, TreeView::new(PROJECT, vec![]));
            self.build(live)
        } else if keyed != self.was_keyed {
            let nodes = if keyed {
                vec![]
            } else {
                crate::data::project_tree()
            };
            self.build(TreeView::new(PROJECT, nodes))
        } else {
            let old = &self.tree;
            let mut new = self.build(TreeView::new(PROJECT, vec![]));
            new.nodes = old.nodes.clone();
            new.expanded = old.expanded.clone();
            new.selected = old.selected.clone();
            new.cursor = old.cursor;
            new.flatten();
            new
        };
        self.was_keyed = keyed;
        self.tree = tree
            .select_follows_cursor(self.opt("trees.follow") == "on")
            .show_selection(self.opt("trees.show_selection") == "on");
        let mut conns = self.build(TreeView::new(CONNECTIONS, vec![]));
        conns.filter = match self.opt("trees.filter") {
            "orders" => Some("orders".to_owned()),
            _ => None,
        };
        let old = &self.conns;
        conns.nodes = old.nodes.clone();
        conns.expanded = old.expanded.clone();
        conns.selected = old.selected.clone();
        conns.cursor = old.cursor;
        conns.flatten();
        self.conns = conns;
    }

    /// A keyed node's label, for the readout and the status line.
    fn label_of(&self, key: ItemKey) -> String {
        self.store
            .get(key)
            .map_or_else(|| format!("{key:?}"), |node| node.label.clone())
    }

    /// Puts the store back to the loaded files, then applies the chosen edit.
    /// The tree keeps its cursor, folds and selection across every step,
    /// because they are held by path key.
    fn apply_edit(&mut self) {
        self.store.remove(NEW_FILE);
        self.store.update(MAIN, |node| {
            node.label = "main.rs".to_owned();
            node.meta = Some("1.2 KB".to_owned());
        });
        if self.store.get(FIXTURES).is_none() {
            let tests = ItemKey::text("tests");
            let at = 2;
            self.store
                .insert(Some(tests), at, FIXTURES, KeyedTreeNode::new("fixtures"))
                .expect("the tests folder is in the store");
            for (i, (name, meta)) in [("users.json", "18 KB"), ("orders.json", "44 KB")]
                .into_iter()
                .enumerate()
            {
                let key = ItemKey::text(&format!("tests/fixtures/{name}"));
                self.store
                    .insert(Some(FIXTURES), i, key, KeyedTreeNode::new(name).meta(meta))
                    .expect("the fixtures are new keys");
            }
        }
        self.edit_note = match self.opt("trees.edit") {
            "insert before" => {
                let src = ItemKey::text("src");
                let node = KeyedTreeNode::new("new.rs").meta("just added").glyph("+");
                format!(
                    "insert(src, 0, new.rs) → {}",
                    self.store
                        .insert(Some(src), 0, NEW_FILE, node)
                        .map_or_else(|e| format!("Err({e:?})"), |()| "Ok".to_owned())
                )
            }
            "rename" => format!(
                "update(main.rs) → {}",
                self.store.update(MAIN, |node| {
                    node.label = "app.rs".to_owned();
                    node.meta = Some("renamed".to_owned());
                })
            ),
            "remove" => format!("remove(tests/fixtures) → {}", self.store.remove(FIXTURES)),
            _ => String::new(),
        };
    }

    /// Moves the keyed tree by identity: a cursor jump, a selection, or every
    /// folder open at once.
    fn apply_jump(&mut self) {
        const KEYED: &str = "the demo tree is a valid keyed store";
        self.jump_note = match self.opt("trees.jump") {
            "expand all" => {
                self.tree.expand_all_source(&self.store).expect(KEYED);
                "expand_all_source → every folder is open".to_owned()
            }
            "none" => String::new(),
            path => {
                let key = ItemKey::text(path);
                let moved = self.tree.set_cursor_source(key, &self.store).expect(KEYED);
                let picked = self.tree.select_source(key, &self.store).expect(KEYED);
                format!(
                    "set_cursor_source({path}) → {moved} · select_source → {picked}{}",
                    if moved {
                        ""
                    } else {
                        " (the cursor refuses a node inside a closed folder)"
                    }
                )
            }
        };
    }

    /// A lazy keyed folder loads its children when it opens.
    fn load_target(&mut self, key: ItemKey) {
        self.store.update(key, |node| node.lazy = false);
        for (i, (name, meta)) in [("debug", "212 files"), ("release", "96 files")]
            .into_iter()
            .enumerate()
        {
            let child = ItemKey::text(&format!("target/{name}"));
            self.store
                .insert(Some(key), i, child, KeyedTreeNode::new(name).meta(meta))
                .expect("the target folder was empty");
        }
    }

    /// Folds a keyed tree's actions into the readout and the status line.
    fn on_keyed_actions(
        &mut self,
        response: Result<Response<TreeAction<ItemKey>>, junie_tui::core::keyed::KeyedSourceError>,
        cx: &mut Cx,
    ) -> Outcome {
        let (out, actions) = response
            .expect("the demo tree is a valid keyed store")
            .into_parts();
        let mut steps = Vec::new();
        for action in actions {
            match action {
                TreeAction::Expand(key) => {
                    steps.push(format!("Expand({})", self.label_of(key)));
                    self.load_target(key);
                }
                TreeAction::Activate(key) => {
                    steps.push(format!("Activate({})", self.label_of(key)));
                    cx.status(format!("Opened {}", self.label_of(key)));
                }
            }
        }
        if !steps.is_empty() {
            self.last = steps.join(" → ");
        }
        out
    }

    fn tree_mut(&mut self, id: WidgetId) -> Option<&mut TreeView> {
        [&mut self.tree, &mut self.conns]
            .into_iter()
            .find(|t| t.id == id)
    }

    /// Handles one input's whole batch; the readout lists it in order, so
    /// `branch_activate: Both` shows `Expand(..) → Activate(..)`.
    fn on_actions(&mut self, id: WidgetId, actions: Actions<TreeAction>, cx: &mut Cx) {
        if actions.is_empty() {
            return;
        }
        self.last.clear();
        for a in actions {
            self.on_action(id, a, cx);
        }
    }

    fn on_action(&mut self, id: WidgetId, a: TreeAction, cx: &mut Cx) {
        let t = if id == CONNECTIONS {
            &mut self.conns
        } else {
            &mut self.tree
        };
        let name =
            |t: &TreeView, p: &[usize]| t.node(p).map(|n| n.label.clone()).unwrap_or_default();
        match a {
            TreeAction::Expand(p) => {
                push_step(&mut self.last, format!("Expand({})", name(t, &p)));

                t.set_busy(&p, true);
                self.loading = Some((p, LOAD_TICKS));
            }
            TreeAction::Activate(p) => {
                push_step(&mut self.last, format!("Activate({})", name(t, &p)));
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
        let mut lines = vec![
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
        ];
        if self.keyed() {
            let notes: Vec<&str> = [self.edit_note.as_str(), self.jump_note.as_str()]
                .into_iter()
                .filter(|note| !note.is_empty())
                .collect();
            lines.push(format!(
                "Project source keyed (TreeSource: KeyedTree, {} nodes, empty {}) · select_follows_cursor {} · show_selection {} · {}",
                self.store.len(),
                self.store.is_empty(),
                self.opt("trees.follow") == "on",
                self.opt("trees.show_selection") == "on",
                if notes.is_empty() { "no edit or jump".to_owned() } else { notes.join(" · ") },
            ));
        }
        lines
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
            RowAction::Open => self.tree.on_key(&plain(KeyCode::Enter)).outcome(),
            RowAction::Fold => self.tree.toggle(row).outcome(),
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
        if self.keyed() {
            self.tree
                .render_source(inner, &self.store, buf, ctx, bg)
                .expect("the demo tree is a valid keyed store");
        } else {
            self.tree.render(inner, buf, ctx, bg);
        }
        panel.draw_meta(card, buf, t, &scrollbar::position_label(&self.tree.scroll));

        let panel = Panel::card(Some("Selection"));
        let bg = panel.bg(t);
        let inner = panel.render(
            Rect::new(r.x, r.y, r.width, r.height.min(SELECTION_H)),
            buf,
            t,
        );
        let mut y = inner.y;
        let keyed = self.keyed();
        let selected: Option<(String, String)> = if keyed {
            self.tree.keyed().selected().next().map(|key| {
                (
                    self.label_of(key),
                    format!("by key · {} selected", self.tree.keyed().selected().count()),
                )
            })
        } else {
            self.tree.selected.clone().map(|path| {
                let mut nodes = &self.tree.nodes;
                let mut parts = Vec::new();
                for &i in &path {
                    parts.push(nodes[i].label.clone());
                    nodes = &nodes[i].children;
                }
                (parts.join("/"), format!("depth {}", path.len() - 1))
            })
        };
        match selected {
            Some((label, detail)) => {
                buf.set_string(
                    inner.x,
                    y,
                    text::truncate(&label, inner.width as usize),
                    t.primary().bg(bg),
                );
                y += 1;
                buf.set_string(inner.x, y, detail, t.muted().bg(bg));
            }
            None => {
                buf.set_string(inner.x, y, "Nothing selected", t.muted().bg(bg));
                y += 1;
                buf.set_string(inner.x, y, "Enter on a file selects it", t.faint().bg(bg));
            }
        }
        y += 2;
        let facts = if keyed {
            let state = self.tree.keyed();
            [
                (
                    "cursor",
                    state
                        .cursor()
                        .map_or_else(|| "none".to_owned(), |key| self.label_of(key)),
                ),
                ("visible", format!("{} rows", state.visible_len())),
                ("open", format!("{} folders", state.expanded().count())),
            ]
        } else {
            let rows = self.tree.rows();
            let cur = &rows[self.tree.cursor.min(rows.len().saturating_sub(1))];
            [
                ("cursor", cur.label.clone()),
                ("visible", format!("{} rows", rows.len())),
                ("open", format!("{} folders", self.tree.expanded.len())),
            ]
        };
        for (k, v) in facts {
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
                if self.keyed() && cx.focus.is(PROJECT) {
                    let response = self.tree.on_key_source(key, &self.store);
                    return self.on_keyed_actions(response, cx);
                }
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
                let (o, actions) = t.on_key(key).into_parts();
                self.on_actions(id, actions, cx);
                o
            }
            PageEvent::Pointer(ev) if ev.is(Phase::Secondary) && self.keyed() => {
                // The row menu edits the owned nodes; a borrowed source has none.
                Outcome::Ignored
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
            PageEvent::Pointer(ev) if self.keyed() && ev.on(PROJECT) => {
                let response = self.tree.on_pointer_source(ev, &self.store);
                if ev.is(Phase::Click) && ev.key_of(Part::ROW).is_some() {
                    cx.focus.focus(PROJECT);
                }
                self.on_keyed_actions(response, cx)
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
                    let (o, actions) = r.into_parts();
                    self.on_actions(tid, actions, cx);
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
        hints.push((
            "Enter",
            if matches!(t.branch_activate, BranchAction::Both) {
                "Expand, then open"
            } else {
                "Open"
            },
        ));
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
            if self.keyed() {
                match key {
                    "trees.edit" => self.apply_edit(),
                    "trees.jump" => self.apply_jump(),
                    "trees.source" => {
                        self.store = keyed_project();
                        self.apply_edit();
                        self.apply_jump();
                    }
                    _ => {}
                }
            }
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

#[cfg(test)]
mod tests {
    use super::*;
    use junie_tui::core::event::Key;
    use junie_tui::core::focus::{Focus, FocusRing};
    use ratatui::crossterm::event::KeyModifiers;

    /// With `branch_activate: both`, Enter on a collapsed lazy branch reports
    /// the expansion request and then the activation, each once, in order.
    #[test]
    fn branch_activate_both_lists_expand_then_activate() {
        let mut page = TreesPage::new();
        page.set_option("trees.branch", 2);
        let mut ring = FocusRing::default();
        ring.register(CONNECTIONS);
        let mut focus = Focus::default();
        focus.focus(CONNECTIONS);
        let mut cx = Cx::new(&mut focus, &ring);

        page.conns.cursor = 1;
        let enter = PageEvent::Key(Key {
            code: KeyCode::Enter,
            mods: KeyModifiers::NONE,
        });
        let _ = page.handle(&enter, &mut cx);

        assert_eq!(page.last, "Expand(orders-db) → Activate(orders-db)");
    }

    #[test]
    fn branch_activate_both_is_hinted_on_the_enter_key() {
        let mut page = TreesPage::new();
        let enter = |p: &TreesPage| {
            p.hints(Some(CONNECTIONS))
                .into_iter()
                .find(|(k, _)| *k == "Enter")
                .map(|(_, v)| v)
        };
        assert_eq!(enter(&page), Some("Open"));
        page.set_option("trees.branch", 2);
        assert_eq!(enter(&page), Some("Expand, then open"));
    }
}
