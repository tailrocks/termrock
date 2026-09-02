// SPDX-FileCopyrightText: 2026 Alexey Zhokhov
// SPDX-License-Identifier: Apache-2.0
//
// Adapted from junie-tui src/bin/showcase/pages/trees.rs (MIT).
// Demo tree copied from junie-tui src/bin/showcase/data.rs (MIT).

//! Indent carries hierarchy; the focus bar never moves.

use std::collections::HashSet;

use ratatui::buffer::Buffer;
use ratatui::layout::Rect;
use ratatui::text::Line;
use ratatui::widgets::StatefulWidget;
use termrock::input::KeyEventKind;
use termrock::widgets::{Tree, TreeNode, TreeOutcome, TreeState};

use crate::ctx::RenderCtx;
use crate::id::WidgetId;
use crate::layout;
use crate::outcome::Route;
use crate::page::{Hint, Page, PageCtx, PageEvent};
use crate::text;

const TREE: WidgetId = WidgetId::of("trees").sub("tree");

struct Node {
    label: &'static str,
    meta: Option<&'static str>,
    children: Vec<Node>,
}

impl Node {
    fn dir(label: &'static str, children: Vec<Node>) -> Self {
        Self {
            label,
            meta: None,
            children,
        }
    }
    fn leaf_meta(label: &'static str, meta: &'static str) -> Self {
        Self {
            label,
            meta: Some(meta),
            children: vec![],
        }
    }
}

fn project_tree() -> Vec<Node> {
    vec![
        Node::dir(
            "src",
            vec![
                Node::dir(
                    "api",
                    vec![
                        Node::leaf_meta("auth.rs", "2.1 KB"),
                        Node::leaf_meta("billing.rs", "6.4 KB"),
                        Node::leaf_meta("mod.rs", "312 B"),
                        Node::dir(
                            "webhooks",
                            vec![
                                Node::leaf_meta("dispatch.rs", "3.9 KB"),
                                Node::leaf_meta("retry.rs", "1.7 KB"),
                                Node::leaf_meta("mod.rs", "180 B"),
                            ],
                        ),
                    ],
                ),
                Node::dir(
                    "db",
                    vec![
                        Node::leaf_meta("migrations.rs", "9.2 KB"),
                        Node::leaf_meta("pool.rs", "1.1 KB"),
                        Node::leaf_meta("schema.rs", "14.8 KB"),
                    ],
                ),
                Node::dir(
                    "workers",
                    vec![
                        Node::leaf_meta("scheduler.rs", "4.6 KB"),
                        Node::leaf_meta("mailer.rs", "2.8 KB"),
                    ],
                ),
                Node::leaf_meta("config.rs", "1.9 KB"),
                Node::leaf_meta("lib.rs", "640 B"),
                Node::leaf_meta("main.rs", "1.2 KB"),
            ],
        ),
        Node::dir(
            "tests",
            vec![
                Node::leaf_meta("checkout.rs", "5.3 KB"),
                Node::leaf_meta("auth_flow.rs", "3.0 KB"),
                Node::dir(
                    "fixtures",
                    vec![
                        Node::leaf_meta("users.json", "18 KB"),
                        Node::leaf_meta("orders.json", "44 KB"),
                    ],
                ),
            ],
        ),
        Node::dir(
            "docs",
            vec![
                Node::leaf_meta("architecture.md", "7.7 KB"),
                Node::leaf_meta("webhooks.md", "2.2 KB"),
            ],
        ),
        Node::leaf_meta("Cargo.toml", "1.4 KB"),
        Node::leaf_meta("README.md", "3.5 KB"),
    ]
}

fn node_at<'a>(nodes: &'a [Node], path: &[usize]) -> Option<&'a Node> {
    let mut cur = nodes;
    let mut found = None;
    for &i in path {
        found = cur.get(i);
        cur = &found?.children;
    }
    found
}

fn flatten<'a>(
    nodes: &'a [Node],
    expanded: &HashSet<Vec<usize>>,
    parent: &[usize],
    depth: u16,
    out: &mut Vec<(Vec<usize>, &'a Node, u16, bool)>,
) {
    for (i, n) in nodes.iter().enumerate() {
        let mut path = parent.to_vec();
        path.push(i);
        let branch = !n.children.is_empty();
        let open = branch && expanded.contains(&path);
        out.push((path.clone(), n, depth, open));
        if open {
            flatten(&n.children, expanded, &path, depth.saturating_add(1), out);
        }
    }
}

fn expand_all(nodes: &[Node], parent: &mut Vec<usize>, expanded: &mut HashSet<Vec<usize>>) {
    for (i, n) in nodes.iter().enumerate() {
        parent.push(i);
        if !n.children.is_empty() {
            expanded.insert(parent.clone());
            expand_all(&n.children, parent, expanded);
        }
        parent.pop();
    }
}

fn position_label(offset: usize, view: usize, total: usize) -> String {
    if view == 0 || total <= view {
        return String::new();
    }
    let start = offset.saturating_add(1);
    let end = offset.saturating_add(view).min(total);
    format!("{start}–{end} of {total}")
}

pub struct TreesPage {
    nodes: Vec<Node>,
    expanded: HashSet<Vec<usize>>,
    tree: TreeState<Vec<usize>>,
    chosen: Option<Vec<usize>>,
}

impl TreesPage {
    #[must_use]
    pub fn new() -> Self {
        let nodes = project_tree();
        let mut expanded = HashSet::new();
        for i in 0..nodes.len() {
            expanded.insert(vec![i]);
        }
        Self {
            nodes,
            expanded,
            tree: TreeState::new(Some(vec![0])),
            chosen: None,
        }
    }

    fn visible(&self) -> Vec<(Vec<usize>, &Node, u16, bool)> {
        let mut out = Vec::new();
        flatten(&self.nodes, &self.expanded, &[], 0, &mut out);
        out
    }

    fn apply_outcome(&mut self, out: TreeOutcome<Vec<usize>>) -> Route {
        match &out {
            TreeOutcome::Toggle(path) => {
                if !self.expanded.remove(path) {
                    self.expanded.insert(path.clone());
                }
                Route::Changed
            }
            TreeOutcome::Activated(path) => {
                self.chosen = Some(path.clone());
                Route::Changed
            }
            TreeOutcome::SelectionChanged(_) => Route::Changed,
            TreeOutcome::Ignored | TreeOutcome::Cancelled => Route::Ignored,
            TreeOutcome::CheckToggled(_) => Route::Changed,
            _ => Route::Changed,
        }
    }
}

impl Page for TreesPage {
    fn title(&self) -> &'static str {
        "Trees"
    }
    fn blurb(&self) -> &'static str {
        "Indent carries hierarchy; the focus bar never moves"
    }

    fn render(&mut self, area: Rect, buf: &mut Buffer, ctx: &mut RenderCtx<'_>) {
        let t = ctx.theme;
        let (l, r) = layout::columns(area, (area.width * 3 / 5).max(30), 2);
        let visible = self.visible();
        let view_h = l.height.min(18).saturating_sub(2) as usize;
        let pos = position_label(self.tree.offset(), view_h, visible.len());
        let (inner, _) = layout::card(
            Rect::new(l.x, l.y, l.width, l.height.min(18)),
            buf,
            t,
            Some("Project"),
            Some(&pos),
            ctx.interaction.focused(TREE),
        );
        let rows: Vec<TreeNode<'_, Vec<usize>>> = visible
            .iter()
            .map(|(path, n, depth, open)| {
                let mut node = TreeNode::new(path.clone(), Line::from(n.label), *depth);
                if let Some(meta) = n.meta {
                    node = node.badge(Line::from(meta));
                }
                if !n.children.is_empty() {
                    node = node.branch();
                    if *open {
                        node = node.expanded();
                    }
                }
                node
            })
            .collect();
        Tree::new(&rows, ctx.system)
            .focused(ctx.interaction.focused(TREE))
            .render(inner, buf, &mut self.tree);
        ctx.control(TREE, inner, false);
        ctx.scrollable(TREE, inner);

        let (inner, bg) = layout::card(
            Rect::new(r.x, r.y, r.width, r.height.min(10)),
            buf,
            t,
            Some("Selection"),
            None,
            false,
        );
        let mut y = inner.y;
        match &self.chosen {
            Some(path) => {
                let mut parts = Vec::new();
                for k in 1..=path.len() {
                    if let Some(n) = node_at(&self.nodes, &path[..k]) {
                        parts.push(n.label);
                    }
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
                    format!("depth {}", path.len().saturating_sub(1)),
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
        let rows_now = self.visible();
        let cur_path = self.tree.selected().cloned();
        let cur_label = cur_path
            .as_ref()
            .and_then(|p| node_at(&self.nodes, p).map(|n| n.label))
            .unwrap_or("");
        if y < inner.bottom() {
            buf.set_string(inner.x, y, "cursor", t.faint().bg(bg));
            buf.set_string(
                inner.x + 8,
                y,
                text::truncate(cur_label, inner.width.saturating_sub(8) as usize),
                t.secondary().bg(bg),
            );
            y += 1;
        }
        if y < inner.bottom() {
            buf.set_string(inner.x, y, "visible", t.faint().bg(bg));
            buf.set_string(
                inner.x + 8,
                y,
                format!("{} rows", rows_now.len()),
                t.secondary().bg(bg),
            );
            y += 1;
        }
        if y < inner.bottom() {
            buf.set_string(inner.x, y, "open", t.faint().bg(bg));
            buf.set_string(
                inner.x + 8,
                y,
                format!("{} folders", self.expanded.len()),
                t.secondary().bg(bg),
            );
        }
    }

    fn handle(&mut self, ev: &PageEvent, cx: &mut PageCtx<'_>) -> Route {
        match ev {
            PageEvent::Key(key) if key.kind != KeyEventKind::Release => {
                if cx.focus_id() != Some(TREE) {
                    return Route::Ignored;
                }
                let visible = self.visible();
                let rows: Vec<TreeNode<'_, Vec<usize>>> = visible
                    .iter()
                    .map(|(path, n, depth, open)| {
                        let mut node = TreeNode::new(path.clone(), Line::from(n.label), *depth);
                        if !n.children.is_empty() {
                            node = node.branch();
                            if *open {
                                node = node.expanded();
                            }
                        }
                        node
                    })
                    .collect();
                let out = self.tree.handle_key(&rows, *key);
                if let Some(expand_all_flag) = self.tree.take_bulk_disclosure() {
                    if expand_all_flag {
                        expand_all(&self.nodes, &mut Vec::new(), &mut self.expanded);
                    } else {
                        self.expanded.clear();
                    }
                    return Route::Changed;
                }
                self.apply_outcome(out)
            }
            PageEvent::Click { id, pos } => {
                if *id != TREE {
                    return Route::Ignored;
                }
                cx.set_focus(TREE);
                let n = self.visible().len();
                if self.tree.scroll_to_position(*pos, n) {
                    return Route::Changed;
                }
                let out = self.tree.click(*pos);
                self.apply_outcome(out)
            }
            PageEvent::Drag { pressed, pos } if *pressed == TREE => {
                let n = self.visible().len();
                if self.tree.scroll_to_position(*pos, n) {
                    Route::Changed
                } else {
                    Route::Ignored
                }
            }
            PageEvent::Wheel { id, delta } if *id == TREE => {
                let n = self.visible().len();
                if self.tree.scroll_by(*delta as isize, n) {
                    Route::Changed
                } else {
                    Route::Ignored
                }
            }
            _ => Route::Ignored,
        }
    }

    fn hints(&self, _focus: Option<WidgetId>) -> Vec<Hint> {
        vec![
            ("↑ ↓", "Move"),
            ("← →", "Fold / unfold"),
            ("Enter", "Open"),
            ("*", "Expand all"),
        ]
    }
}
