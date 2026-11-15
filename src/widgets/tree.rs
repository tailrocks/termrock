use ratatui::buffer::Buffer;
use ratatui::crossterm::event::{KeyCode, KeyModifiers};
use ratatui::layout::Rect;
use ratatui::style::{Color, Modifier};

use crate::core::event::{Key, Outcome};
use crate::core::id::{ItemKey, Part, PartRef, WidgetId};
use crate::core::keymap::{Actions, KeyBinding, KeyMap, Nav, NavKeys, lookup, unassigned_chord};
use crate::core::list::{Cursor, KeyStep, Moved, Navigable, RowSource};
use crate::core::scroll::{ScrollPane, Scrollable};
use crate::core::selection::ClickPolicy;
use crate::theme::Tone;
use crate::ui::ctx::RenderCtx;
use crate::ui::list_core::ListCore;
use crate::ui::row::{MarkKind, RowLook, RowUi};

#[derive(Debug, Clone)]
pub struct TreeNode {
    pub label: String,
    pub children: Vec<TreeNode>,
    pub meta: Option<String>,

    pub glyph: Option<std::borrow::Cow<'static, str>>,

    pub glyph_tone: Tone,

    pub label_tone: Option<Tone>,

    pub lazy: bool,

    pub busy: bool,

    pub note: bool,
}

impl TreeNode {
    pub fn leaf(label: &str) -> Self {
        Self {
            label: label.to_owned(),
            children: vec![],
            meta: None,
            glyph: None,
            glyph_tone: Tone::Muted,
            label_tone: None,
            lazy: false,
            busy: false,
            note: false,
        }
    }

    pub fn leaf_meta(label: &str, meta: &str) -> Self {
        Self {
            meta: Some(meta.to_owned()),
            ..Self::leaf(label)
        }
    }

    pub fn dir(label: &str, children: Vec<TreeNode>) -> Self {
        Self {
            children,
            ..Self::leaf(label)
        }
    }

    pub fn lazy(label: &str) -> Self {
        Self {
            lazy: true,
            ..Self::leaf(label)
        }
    }

    pub fn glyph(mut self, g: impl Into<std::borrow::Cow<'static, str>>) -> Self {
        self.glyph = Some(g.into());
        self
    }

    pub fn glyph_tone(mut self, tone: Tone) -> Self {
        self.glyph_tone = tone;
        self
    }

    pub fn label_tone(mut self, tone: Tone) -> Self {
        self.label_tone = Some(tone);
        self
    }

    pub fn note(label: &str) -> Self {
        Self {
            note: true,
            ..Self::leaf(label)
        }
    }

    pub fn meta(mut self, m: &str) -> Self {
        self.meta = Some(m.to_owned());
        self
    }
}

pub type Path = Vec<usize>;

#[derive(Debug, Clone)]
pub struct FlatRow {
    pub path: Path,
    pub depth: usize,
    pub label: String,
    pub meta: Option<String>,
    pub has_children: bool,
    pub expanded: bool,
    pub glyph: Option<std::borrow::Cow<'static, str>>,
    pub glyph_tone: Tone,
    pub label_tone: Option<Tone>,
    pub busy: bool,
    pub note: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TreeAction {
    Expand(Path),

    Activate(Path),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum BranchAction {
    #[default]
    Toggle,

    Activate,

    Both,
}

#[derive(Debug, Clone)]
pub struct TreeView {
    pub id: WidgetId,
    pub nodes: Vec<TreeNode>,
    pub expanded: std::collections::HashSet<Path>,

    pub core: ListCore,
    pub selected: Option<Path>,
    pub area: Rect,
    rows: Vec<FlatRow>,

    pub filter: Option<String>,

    pub reserve_right: u16,

    pub show_meta: bool,

    trailing: Rect,

    pub branch_activate: BranchAction,

    pub indent: u16,

    rows_built: usize,

    pub keymap: KeyMap<TreeCmd>,

    pub divider: bool,

    pub selected_marker: Option<&'static str>,

    pub alt_click_recursive: bool,
}

impl std::ops::Deref for TreeView {
    type Target = ListCore;
    fn deref(&self) -> &ListCore {
        &self.core
    }
}

impl std::ops::DerefMut for TreeView {
    fn deref_mut(&mut self) -> &mut ListCore {
        &mut self.core
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TreeCmd {
    ExpandAll,

    CollapseAll,
}

impl Actions for TreeCmd {
    fn label(self) -> &'static str {
        match self {
            TreeCmd::ExpandAll => "Expand all",
            TreeCmd::CollapseAll => "Collapse all",
        }
    }

    fn default_keymap() -> KeyMap<Self> {
        vec![
            (KeyBinding::char('*'), TreeCmd::ExpandAll),
            (KeyBinding::char('-'), TreeCmd::CollapseAll),
        ]
    }
}

fn label_matches(label: &str, query: &str) -> bool {
    let lowered = label.to_lowercase();
    #[cfg(test)]
    work::record_label_test(lowered.len());
    lowered.contains(query)
}

#[cfg(test)]
mod work {
    use std::cell::Cell;

    #[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
    pub(super) struct MatchWork {
        pub label_tests: usize,
        pub label_bytes: usize,
        pub projection_nodes: usize,
    }

    thread_local! {
        static WORK: Cell<MatchWork> = const {
            Cell::new(MatchWork { label_tests: 0, label_bytes: 0, projection_nodes: 0 })
        };
    }

    pub(super) fn take() -> MatchWork {
        WORK.with(Cell::take)
    }

    pub(super) fn record_label_test(bytes: usize) {
        WORK.with(|w| {
            let mut v = w.get();
            v.label_tests += 1;
            v.label_bytes += bytes;
            w.set(v);
        });
    }

    pub(super) fn record_projection_node() {
        WORK.with(|w| {
            let mut v = w.get();
            v.projection_nodes += 1;
            w.set(v);
        });
    }
}

fn matches(n: &TreeNode, q: &str) -> bool {
    label_matches(&n.label, q) || n.children.iter().any(|c| matches(c, q))
}

fn walk(
    nodes: &[TreeNode],
    path: &mut Path,
    depth: usize,
    exp: &std::collections::HashSet<Path>,
    filter: Option<&str>,
    out: &mut Vec<FlatRow>,
) {
    for (i, n) in nodes.iter().enumerate() {
        if let Some(q) = filter
            && !matches(n, q)
        {
            continue;
        }
        path.push(i);
        let has_children = !n.children.is_empty() || (n.lazy && !n.note);

        let expanded = has_children
            && (exp.contains(path)
                || filter.is_some_and(|q| !n.children.is_empty() && !label_matches(&n.label, q)));
        out.push(FlatRow {
            path: path.clone(),
            depth,
            label: n.label.clone(),
            meta: n.meta.clone(),
            has_children,
            expanded,
            glyph: n.glyph.clone(),
            glyph_tone: n.glyph_tone,
            label_tone: n.label_tone,
            busy: n.busy,
            note: n.note,
        });
        if expanded {
            walk(&n.children, path, depth + 1, exp, filter, out);
        }
        path.pop();
    }
}

impl TreeView {
    pub fn new(id: WidgetId, nodes: Vec<TreeNode>) -> Self {
        let mut tv = Self {
            id,
            nodes,
            expanded: Default::default(),
            core: ListCore::new(0),
            selected: None,
            area: Rect::ZERO,
            rows: vec![],
            filter: None,
            reserve_right: 0,
            show_meta: true,
            trailing: Rect::ZERO,
            branch_activate: BranchAction::Toggle,
            indent: 2,
            rows_built: 0,
            keymap: TreeCmd::default_keymap(),
            divider: false,
            selected_marker: None,
            alt_click_recursive: true,
        };

        for i in 0..tv.nodes.len() {
            tv.expanded.insert(vec![i]);
        }
        tv.flatten();
        tv
    }

    pub fn branch_activate(mut self, action: BranchAction) -> Self {
        self.branch_activate = action;
        self
    }

    pub fn click(mut self, policy: ClickPolicy) -> Self {
        self.core.click = policy;
        self
    }

    pub fn wrap(mut self, on: bool) -> Self {
        self.core.wrap = on;
        self
    }

    pub fn nav(mut self, nav: NavKeys) -> Self {
        self.core.nav = nav;
        self
    }

    pub fn keymap(mut self, keymap: KeyMap<TreeCmd>) -> Self {
        self.keymap = keymap;
        self
    }

    pub fn indent(mut self, cols: u16) -> Self {
        self.indent = cols;
        self
    }

    pub fn type_ahead_timeout(mut self, timeout: std::time::Duration) -> Self {
        self.core.typeahead.timeout = timeout;
        self
    }

    pub fn type_ahead(mut self, on: bool) -> Self {
        self.core.type_ahead = on;
        self
    }

    pub fn type_ahead_precedence(
        mut self,
        precedence: crate::core::list::TypeAheadPrecedence,
    ) -> Self {
        self.core.type_ahead_precedence = Some(precedence);
        self
    }

    pub fn content(mut self, content: crate::ui::content::ContentState) -> Self {
        self.core.content = content;
        self
    }

    pub fn empty(mut self, state: crate::widgets::empty::EmptyState) -> Self {
        self.core.empty = Some(state);
        self
    }

    pub fn skip_when_empty(mut self, on: bool) -> Self {
        self.core.skip_when_empty = on;
        self
    }

    pub fn divider(mut self, on: bool) -> Self {
        self.divider = on;
        self
    }

    pub fn inactive_selection(mut self, look: crate::theme::InactiveSelection) -> Self {
        self.core.inactive_selection = Some(look);
        self
    }

    pub fn selected_marker(mut self, marker: Option<&'static str>) -> Self {
        self.selected_marker = marker;
        self
    }

    pub fn alt_click_recursive(mut self, on: bool) -> Self {
        self.alt_click_recursive = on;
        self
    }

    fn branch(&mut self, i: usize) -> (Outcome, Option<TreeAction>) {
        let path = self.rows[i].path.clone();
        match self.branch_activate {
            BranchAction::Toggle => self.toggle(i),
            BranchAction::Activate => {
                self.selected = Some(path.clone());
                (Outcome::Changed, Some(TreeAction::Activate(path)))
            }
            BranchAction::Both => {
                let (o, ev) = self.toggle(i);
                self.selected = Some(path.clone());
                (
                    o.or(Outcome::Changed),
                    ev.or(Some(TreeAction::Activate(path))),
                )
            }
        }
    }

    pub fn rows(&self) -> &[FlatRow] {
        &self.rows
    }

    pub fn reserve_right(mut self, cells: u16) -> Self {
        self.reserve_right = cells;
        self
    }

    pub fn show_meta(mut self, on: bool) -> Self {
        self.show_meta = on;
        self
    }

    pub fn trailing(&self) -> Rect {
        self.trailing
    }

    pub const MIN_LABEL: u16 = 12;

    pub fn flatten(&mut self) {
        let focused: Option<Path> = self.rows.get(self.core.cursor).map(|r| r.path.clone());
        let before: Option<Path> = self
            .cursor
            .checked_sub(1)
            .and_then(|i| self.rows.get(i))
            .map(|r| r.path.clone());
        let after: Option<Path> = self.rows.get(self.core.cursor + 1).map(|r| r.path.clone());
        let mut out = Vec::new();
        let filter = self
            .filter
            .as_deref()
            .map(|f| f.trim().to_lowercase())
            .filter(|f| !f.is_empty());
        walk(
            &self.nodes,
            &mut Vec::new(),
            0,
            &self.expanded,
            filter.as_deref(),
            &mut out,
        );
        self.rows_built += out.len();
        self.rows = out;
        self.core.cursor = self
            .relocate(focused.as_deref(), before.as_deref(), after.as_deref())
            .unwrap_or(self.core.cursor)
            .min(self.rows.len().saturating_sub(1));
        self.core.scroll.set_content(self.rows.len());
    }

    fn relocate(
        &self,
        focused: Option<&[usize]>,
        before: Option<&[usize]>,
        after: Option<&[usize]>,
    ) -> Option<usize> {
        let find = |p: &[usize]| self.rows.iter().position(|r| r.path == p);
        let focused = focused?;
        if let Some(i) = find(focused) {
            return Some(i);
        }
        for k in (1..focused.len()).rev() {
            if let Some(i) = find(&focused[..k]) {
                return Some(i);
            }
        }
        after.and_then(find).or_else(|| before.and_then(find))
    }

    pub fn cursor_path(&self) -> Option<&[usize]> {
        self.rows.get(self.core.cursor).map(|r| r.path.as_slice())
    }

    pub fn node(&self, path: &[usize]) -> Option<&TreeNode> {
        let mut nodes = &self.nodes;
        let mut cur: Option<&TreeNode> = None;
        for &i in path {
            cur = nodes.get(i);
            nodes = &cur?.children;
        }
        cur
    }

    pub fn node_mut(&mut self, path: &[usize]) -> Option<&mut TreeNode> {
        let (last, parents) = path.split_last()?;
        let mut nodes = &mut self.nodes;
        for &i in parents {
            nodes = &mut nodes.get_mut(i)?.children;
        }
        nodes.get_mut(*last)
    }

    pub fn set_children(&mut self, path: &[usize], children: Vec<TreeNode>) {
        if let Some(n) = self.node_mut(path) {
            n.children = children;
            n.lazy = false;
            n.busy = false;
        }
        self.expanded.insert(path.to_vec());
        self.flatten();
    }

    pub fn set_busy(&mut self, path: &[usize], busy: bool) {
        if let Some(n) = self.node_mut(path) {
            n.busy = busy;
        }
        self.flatten();
    }

    pub fn set_filter(&mut self, q: Option<&str>) {
        self.filter = q.map(str::to_owned);
        self.flatten();
        self.core.scroll.jump_start();
        self.core.cursor = 0;
    }

    pub fn reveal(&mut self, path: &[usize]) {
        for k in 1..path.len() {
            self.expanded.insert(path[..k].to_vec());
        }
        self.flatten();
        if let Some(i) = self.rows.iter().position(|r| r.path == path) {
            self.set_cursor(i);
        }
    }

    pub fn row_id(&self, i: usize) -> WidgetId {
        self.id.part(PartRef::item(Part::ROW, ItemKey::index(i)))
    }

    pub fn toggle_id(&self, i: usize) -> WidgetId {
        self.id.part(PartRef::item(Part::ICON, ItemKey::index(i)))
    }

    pub fn row_at(&self, ev: &crate::core::pointer::PointerEvent) -> Option<usize> {
        if !ev.on(self.id) {
            return None;
        }
        ev.index_of(Part::ROW)
            .or_else(|| ev.index_of(Part::ICON))
            .or_else(|| ev.index_of(Part::MARKER))
    }

    pub fn glyph_id(&self, i: usize) -> WidgetId {
        self.id.part(PartRef::item(Part::MARKER, ItemKey::index(i)))
    }

    fn nav_cursor(&self) -> Cursor {
        self.core.cursor_model()
    }

    fn set_cursor(&mut self, i: usize) {
        self.core.cursor = i.min(self.rows.len().saturating_sub(1));
        self.core.scroll.ensure_visible(self.core.cursor);
    }

    pub fn toggle(&mut self, i: usize) -> (Outcome, Option<TreeAction>) {
        let Some(row) = self.rows.get(i) else {
            return (Outcome::Consumed, None);
        };
        if !row.has_children {
            return (Outcome::Consumed, None);
        }
        let path = row.path.clone();
        if !self.expanded.remove(&path) {
            self.expanded.insert(path.clone());
            let lazy = self
                .node(&path)
                .is_some_and(|n| n.lazy && n.children.is_empty());
            if lazy && let Some(n) = self.node_mut(&path) {
                n.busy = true;
            }
            self.refold(i);
            if lazy {
                return (Outcome::Changed, Some(TreeAction::Expand(path)));
            }
            return (Outcome::Changed, None);
        }
        self.refold(i);
        (Outcome::Changed, None)
    }

    fn refold(&mut self, i: usize) {
        if self.filter.as_deref().is_some_and(|f| !f.trim().is_empty()) {
            self.flatten();
            return;
        }
        let depth = self.rows[i].depth;

        let end = self.rows[i + 1..]
            .iter()
            .position(|r| r.depth <= depth)
            .map_or(self.rows.len(), |k| i + 1 + k);
        let mut path = self.rows[i].path.clone();
        let Some(node) = self.node(&path) else {
            self.flatten();
            return;
        };
        let has_children = !node.children.is_empty() || (node.lazy && !node.note);
        let open = has_children && self.expanded.contains(&path);
        let busy = node.busy;
        let mut shown = Vec::new();
        if open {
            walk(
                &node.children,
                &mut path,
                depth + 1,
                &self.expanded,
                None,
                &mut shown,
            );
        }
        self.rows_built += shown.len();
        let row = &mut self.rows[i];
        row.expanded = open;
        row.has_children = has_children;
        row.busy = busy;
        let removed = end - (i + 1);
        let added = shown.len();
        self.rows.splice(i + 1..end, shown);

        if self.core.cursor > i {
            self.core.cursor = if self.core.cursor < end {
                if removed > 0 { i } else { self.core.cursor }
            } else {
                self.core.cursor - removed + added
            };
        }
        self.core.cursor = self.core.cursor.min(self.rows.len().saturating_sub(1));
        self.core.scroll.set_content(self.rows.len());
    }

    pub fn expand_all(&mut self) {
        fn walk(nodes: &[TreeNode], path: &mut Path, exp: &mut std::collections::HashSet<Path>) {
            for (i, n) in nodes.iter().enumerate() {
                path.push(i);
                if !n.children.is_empty() {
                    exp.insert(path.clone());
                    walk(&n.children, path, exp);
                }
                path.pop();
            }
        }
        walk(&self.nodes, &mut Vec::new(), &mut self.expanded);
        self.flatten();
    }

    pub fn collapse_all(&mut self) {
        self.expanded.clear();
        self.flatten();
    }

    fn all_expanded(&self) -> bool {
        fn walk(
            nodes: &[TreeNode],
            path: &mut Path,
            exp: &std::collections::HashSet<Path>,
        ) -> bool {
            nodes.iter().enumerate().all(|(i, n)| {
                path.push(i);
                let ok =
                    n.children.is_empty() || (exp.contains(path) && walk(&n.children, path, exp));
                path.pop();
                ok
            })
        }
        walk(&self.nodes, &mut Vec::new(), &self.expanded)
    }

    pub fn context_items(&self) -> Vec<(TreeCmd, crate::widgets::menu::MenuItem)> {
        use crate::ui::context::{ContextCmd, command_items};
        let cmds = [
            ContextCmd::new(TreeCmd::ExpandAll)
                .unless(self.all_expanded().then_some("Every branch is open")),
            ContextCmd::new(TreeCmd::CollapseAll)
                .unless(self.expanded.is_empty().then_some("Every branch is closed")),
        ];
        command_items(&self.keymap, &cmds)
    }

    pub fn run(&mut self, cmd: TreeCmd) -> (Outcome, Option<TreeAction>) {
        match cmd {
            TreeCmd::ExpandAll => self.expand_all(),
            TreeCmd::CollapseAll => self.collapse_all(),
        }
        (Outcome::Changed, None)
    }

    pub fn on_key(&mut self, key: &Key) -> (Outcome, Option<TreeAction>) {
        if self.rows.is_empty() {
            return (Outcome::Ignored, None);
        }
        if unassigned_chord(key, &self.keymap) {
            return (Outcome::Ignored, None);
        }
        if let Some(action) = lookup(&self.keymap, key) {
            return self.run(action);
        }
        let rows = &self.rows[..];
        let nav = match self.core.key_step(key, rows) {
            Some((KeyStep::Type(_), moved)) => {
                if let Moved::To(i) = moved {
                    self.set_cursor(i);
                }
                return (moved.outcome(), None);
            }
            Some((KeyStep::Nav(nav), moved)) => (nav, moved),
            None => return self.on_plain_key(key),
        };
        let (nav, moved) = nav;
        let out = match nav {
            Nav::Up | Nav::Down | Nav::PageUp | Nav::PageDown | Nav::Home | Nav::End => {
                if let Some(i) = moved.index() {
                    self.set_cursor(i);
                }
                Outcome::Changed
            }
            Nav::Right => {
                let row = &self.rows[self.core.cursor];
                if row.has_children && !row.expanded {
                    return self.toggle(self.core.cursor);
                } else if row.has_children {
                    self.set_cursor(self.core.cursor + 1);
                    Outcome::Changed
                } else {
                    Outcome::Consumed
                }
            }
            Nav::Left => {
                let row = &self.rows[self.core.cursor];
                if row.has_children && row.expanded {
                    return self.toggle(self.core.cursor);
                } else if row.depth > 0 {
                    let depth = row.depth;
                    if let Some(pi) = self.rows[..self.core.cursor]
                        .iter()
                        .rposition(|r| r.depth < depth)
                    {
                        self.set_cursor(pi);
                    }
                    Outcome::Changed
                } else {
                    Outcome::Consumed
                }
            }
        };
        (out, None)
    }

    fn on_plain_key(&mut self, key: &Key) -> (Outcome, Option<TreeAction>) {
        let out = match key.code {
            KeyCode::Enter | KeyCode::Char(' ') => {
                let row = &self.rows[self.core.cursor];
                if row.note {
                    Outcome::Consumed
                } else if row.has_children {
                    return self.branch(self.core.cursor);
                } else {
                    self.selected = Some(row.path.clone());
                    return (
                        Outcome::Changed,
                        Some(TreeAction::Activate(row.path.clone())),
                    );
                }
            }
            _ => Outcome::Ignored,
        };
        (out, None)
    }

    pub fn on_click_row(&mut self, i: usize) -> (Outcome, Option<TreeAction>) {
        self.on_click_row_count(i, 1)
    }

    pub fn on_click_row_count(&mut self, i: usize, count: u8) -> (Outcome, Option<TreeAction>) {
        if i >= self.rows.len() {
            return (Outcome::Consumed, None);
        }
        let again = self.core.cursor == i;
        self.core.scroll.pointer_move();
        self.set_cursor(i);
        if self.rows[i].note {
            return (Outcome::Consumed, None);
        }
        if !self.core.click.activates(count.max(1), again) {
            return (Outcome::Changed, None);
        }
        if self.rows[i].has_children {
            self.branch(i)
        } else {
            self.selected = Some(self.rows[i].path.clone());
            (
                Outcome::Changed,
                Some(TreeAction::Activate(self.rows[i].path.clone())),
            )
        }
    }

    pub fn on_click_toggle(&mut self, i: usize) -> (Outcome, Option<TreeAction>) {
        self.core.scroll.pointer_move();
        self.set_cursor(i);
        self.toggle(i)
    }

    pub fn on_click_with(
        &mut self,
        i: usize,
        toggle: bool,
        mods: KeyModifiers,
    ) -> (Outcome, Option<TreeAction>) {
        self.on_click_at(i, toggle, mods, 1)
    }

    pub fn on_click_at(
        &mut self,
        i: usize,
        toggle: bool,
        mods: KeyModifiers,
        count: u8,
    ) -> (Outcome, Option<TreeAction>) {
        if self.alt_click_recursive
            && mods.contains(KeyModifiers::ALT)
            && self.rows.get(i).is_some_and(|r| r.has_children && !r.note)
        {
            self.set_cursor(i);
            return self.toggle_recursive(i);
        }
        if toggle {
            self.on_click_toggle(i)
        } else {
            self.on_click_row_count(i, count)
        }
    }

    pub fn toggle_recursive(&mut self, i: usize) -> (Outcome, Option<TreeAction>) {
        let Some(row) = self.rows.get(i) else {
            return (Outcome::Consumed, None);
        };
        if !row.has_children {
            return (Outcome::Consumed, None);
        }
        let path = row.path.clone();
        let open = !row.expanded;
        fn walk(
            nodes: &[TreeNode],
            path: &mut Path,
            open: bool,
            exp: &mut std::collections::HashSet<Path>,
        ) {
            for (k, n) in nodes.iter().enumerate() {
                path.push(k);
                if !n.children.is_empty() {
                    if open {
                        exp.insert(path.clone());
                    } else {
                        exp.remove(path);
                    }
                    walk(&n.children, path, open, exp);
                }
                path.pop();
            }
        }
        let lazy = self
            .node(&path)
            .is_some_and(|n| n.lazy && n.children.is_empty());
        if let Some(children) = self.node(&path).map(|n| n.children.clone()) {
            walk(&children, &mut path.clone(), open, &mut self.expanded);
        }
        if open {
            self.expanded.insert(path.clone());
            if lazy && let Some(n) = self.node_mut(&path) {
                n.busy = true;
            }
        } else {
            self.expanded.remove(&path);
        }
        self.refold(i);
        if open && lazy {
            return (Outcome::Changed, Some(TreeAction::Expand(path)));
        }
        (Outcome::Changed, None)
    }

    pub fn track_click(mut self, click: crate::core::scroll::TrackClick) -> Self {
        self.core.scroll.track_click = Some(click);
        self
    }

    pub fn render(&mut self, area: Rect, buf: &mut Buffer, ctx: &mut RenderCtx, bg: Color) {
        self.core.scroll.begin(ctx.track_click);
        let area = area.intersection(*buf.area());
        if area.is_empty() {
            return;
        }
        self.area = area;
        let t = ctx.theme;
        let focused = ctx.interaction.focused(self.id);
        self.core.scroll.set_viewport(area.height as usize);
        let n = self.rows.len();

        if self.core.is_focus_stop(n, focused) {
            ctx.control(self.id, area, false);
            ctx.scrollable(self.id, area);
        }

        if (!self.core.content.is_ready() || self.core.empty.is_some())
            && self.core.draw_content(n, area, buf, ctx, bg, None)
        {
            return;
        }
        let has_sb = self.core.scroll.overflows();
        let full_w = area
            .width
            .saturating_sub(if has_sb || self.divider { 1 } else { 0 });

        let reserve = if self.reserve_right > 0 && full_w >= self.reserve_right + Self::MIN_LABEL {
            self.reserve_right
        } else {
            0
        };
        self.trailing = if reserve > 0 {
            Rect::new(area.x + full_w - reserve, area.y, reserve, area.height)
        } else {
            Rect::ZERO
        };
        let row_w = full_w - reserve;

        let show_meta = self.show_meta
            && self.core.scroll.visible_range().all(|ri| {
                let row = &self.rows[ri];
                let Some(m) = &row.meta else { return true };
                let need = 1
                    + (row.depth as u16) * self.indent
                    + 2
                    + if row.glyph.is_some() { 2 } else { 0 }
                    + crate::ui::text::width(&row.label) as u16
                    + 2
                    + crate::ui::text::width(m) as u16
                    + 1;
                need <= row_w
            });
        for (i, ri) in self.core.scroll.visible_range().enumerate() {
            let y = area.y + i as u16;
            let row = &self.rows[ri];
            let rect = Rect::new(area.x, y, row_w, 1);

            let full = Rect::new(area.x, y, full_w, 1);
            let rid = self.row_id(ri);
            let mut s = ctx.child_state(self.id, rid, ri == self.core.cursor);
            let tid = self.toggle_id(ri);

            s.hovered |= ctx.interaction.hovered(tid);
            s.pressed |= ctx.interaction.pressed(tid);

            let gid = self.glyph_id(ri);
            s.hovered |= ctx.interaction.hovered(gid);
            s.pressed |= ctx.interaction.pressed(gid);
            if row.note {
                s.hovered = false;
                s.pressed = false;
            }
            s.focused = focused && ri == self.core.cursor;
            s.selected = self.selected.as_ref() == Some(&row.path);
            s.inactive_selection = self.core.inactive_selection;
            let rp = RowUi::paint(ctx, buf, full, s, RowLook::Bar, bg);
            let st = rp.style();
            let indent = (row.depth as u16) * self.indent;
            let mut x = rect.x + 1 + indent;

            if !row.note {
                ctx.register_part(self.id, PartRef::item(Part::ROW, ItemKey::index(ri)), full);
            }
            if x + 2 >= rect.right() {
                continue;
            }
            let glyph = if row.busy {
                t.glyphs.spinner_frame(ctx.interaction.tick)
            } else if row.has_children {
                if row.expanded {
                    t.glyphs.disclosure_open
                } else {
                    t.glyphs.disclosure_closed
                }
            } else {
                " "
            };
            let gs = if row.busy {
                st.fg(t.legible_mark(t.accent, st.bg.unwrap_or(bg)))
            } else if row.has_children {
                st.fg(t.text_secondary)
            } else {
                st
            };
            buf.set_string(x, y, glyph, gs);
            if row.has_children {
                ctx.register_part(
                    self.id,
                    PartRef::item(Part::ICON, ItemKey::index(ri)),
                    Rect::new(x, y, 2, 1),
                );
            } else if s.selected
                && !row.busy
                && let Some(m) = self.selected_marker
            {
                rp.mark(ctx, buf, x, y, MarkKind::Glyph(m), focused);
            }
            x += 2;
            let meta_w = row
                .meta
                .as_ref()
                .map(|m| crate::ui::text::width(m))
                .unwrap_or(0) as u16;
            let avail = rect.right().saturating_sub(x);

            let meta_w = if !show_meta || avail.saturating_sub(meta_w + 2) < 10 {
                0
            } else {
                meta_w
            };
            let lw = avail.saturating_sub(if meta_w > 0 { meta_w + 2 } else { 1 });

            let mut label_style = match (s.selected, focused) {
                (true, true) => st.fg(t.chosen(true)),
                (true, false) => st
                    .fg(t.legible(t.chosen_in(s), st.bg.unwrap_or(bg)))
                    .add_modifier(Modifier::BOLD),
                _ => match row.label_tone {
                    Some(tone) => st.fg(t.tone_on(tone, st.bg.unwrap_or(t.canvas))),
                    None => st,
                },
            };
            if row.note {
                label_style = st.fg(t.text_muted).remove_modifier(Modifier::BOLD);
            }
            let mut lx = x;
            if let Some(g) = row.glyph.as_deref() {
                if !row.note {
                    ctx.register_part(
                        self.id,
                        PartRef::item(Part::MARKER, ItemKey::index(ri)),
                        Rect::new(lx, y, 2.min(rect.right().saturating_sub(lx)), 1),
                    );
                }
                buf.set_string(
                    lx,
                    y,
                    g,
                    st.fg(if row.glyph_tone == Tone::Muted {
                        t.text_muted
                    } else {
                        t.tone_on(row.glyph_tone, st.bg.unwrap_or(t.canvas))
                    })
                    .remove_modifier(Modifier::BOLD),
                );
                lx += 2;
            }
            buf.set_string(
                lx,
                y,
                crate::ui::text::fit(&row.label, (lw as usize).saturating_sub((lx - x) as usize)),
                label_style,
            );
            if let Some(m) = &row.meta
                && meta_w > 0
                && meta_w + 4 < avail
            {
                buf.set_string(
                    rect.right().saturating_sub(meta_w + 1),
                    y,
                    m,
                    st.fg(t.text_muted),
                );
            }

            if row.has_children {
                ctx.register_part(
                    self.id,
                    PartRef::item(Part::ICON, ItemKey::index(ri)),
                    Rect::new(rect.x + 1 + indent, y, 2, 1),
                );
            }
        }

        ctx.scrollable(self.id, area);
        if self.divider && !has_sb {
            let sb = Rect::new(area.right() - 1, area.y, 1, area.height);
            self.core
                .scroll
                .draw_divider(sb, buf, ctx, self.id, focused);
        }
        if has_sb {
            crate::ui::fade::scroll_edges_except(
                buf,
                ctx,
                Rect::new(
                    area.x,
                    area.y,
                    (area.right() - 1).saturating_sub(area.x),
                    area.height,
                ),
                &self.core.scroll,
                &self
                    .scroll
                    .visible_range()
                    .position(|i| i == self.core.cursor)
                    .map(|k| area.y + k as u16)
                    .into_iter()
                    .collect::<Vec<_>>(),
            );
            let sb = Rect::new(area.right() - 1, area.y, 1, area.height);
            self.core.scroll.draw_v(sb, buf, ctx, self.id, focused);
        }
    }
}

impl crate::ui::measure::Measure for TreeView {
    fn measure(&self, avail: ratatui::layout::Size, ctx: &RenderCtx) -> ratatui::layout::Size {
        if self.rows.is_empty() || !self.core.content.is_ready() {
            return match &self.core.empty {
                Some(e) if self.rows.is_empty() => e.measure(avail, ctx),
                _ => ratatui::layout::Size::new(0, self.rows.len() as u16),
            };
        }
        let w = |s: &str| crate::ui::text::width(s) as u16;
        let width = self
            .rows
            .iter()
            .map(|row| {
                let lead = 1 + (row.depth as u16) * self.indent + 2;
                let label = if row.glyph.is_some() { 2 } else { 0 } + w(&row.label);
                let rest = match row.meta.as_deref().filter(|_| self.show_meta) {
                    Some(m) => label.max(10) + 2 + w(m) + 1,
                    None => label + 1,
                };
                lead + rest
            })
            .max()
            .unwrap_or(0);
        ratatui::layout::Size::new(
            width + u16::from(self.divider) + self.reserve_right,
            self.rows.len() as u16,
        )
    }
}

impl Scrollable for TreeView {
    fn pane(&mut self) -> &mut ScrollPane {
        &mut self.core.scroll
    }
}

impl RowSource for [FlatRow] {
    fn len(&self) -> usize {
        <[FlatRow]>::len(self)
    }
    fn enabled(&self, i: usize) -> bool {
        !self[i].note
    }
    fn label(&self, i: usize) -> &str {
        &self[i].label
    }
}

impl RowSource for TreeView {
    fn len(&self) -> usize {
        self.rows.len()
    }
    fn enabled(&self, i: usize) -> bool {
        self.rows[..].enabled(i)
    }
    fn label(&self, i: usize) -> &str {
        &self.rows[i].label
    }
}

impl Navigable for TreeView {
    fn cursor(&self) -> Cursor {
        self.nav_cursor()
    }
}

const SPECIMEN_ROWS: usize = 4;

impl crate::ui::specimen::Specimen for TreeView {
    fn specimen_id(&self) -> WidgetId {
        self.id
    }

    fn supports(&self, state: crate::ui::specimen::SpecimenState) -> bool {
        use crate::ui::specimen::SpecimenState::*;
        matches!(
            state,
            Rest | Hover
                | Pressed
                | Focused
                | FocusedHover
                | Selected
                | Busy
                | Loading
                | Error
                | Overflow
                | Narrow
        )
    }

    fn specimen_height(&self, state: crate::ui::specimen::SpecimenState) -> u16 {
        use crate::ui::specimen::SpecimenState::*;
        if matches!(state, Loading | Error) {
            return 1;
        }
        self.rows.len().clamp(1, SPECIMEN_ROWS) as u16
    }

    fn specimen(
        &self,
        state: crate::ui::specimen::SpecimenState,
        area: Rect,
        buf: &mut Buffer,
        ctx: &mut RenderCtx,
        bg: Color,
    ) {
        use crate::ui::specimen::SpecimenState::*;
        let mut t = self.clone();
        t.flatten();
        t.cursor = 1.min(t.rows.len().saturating_sub(1));
        let path = t.rows.get(t.cursor).map(|r| r.path.clone());
        match (state, path) {
            (Selected, Some(p)) => t.selected = Some(p),
            (Overflow, Some(p)) => {
                if let Some(n) = t.node_mut(&p) {
                    n.label = format!("{} and a label far longer than its row", n.label);
                }
                t.flatten();
            }
            (Busy, _) => {
                if let Some(p) = t
                    .rows
                    .iter()
                    .find(|r| r.has_children)
                    .map(|r| r.path.clone())
                {
                    t.set_busy(&p, true);
                }
            }
            (Loading | Error, _) => t.core.content = crate::ui::content::specimen(state),
            _ => {}
        }
        let v = state.visual();
        if v.hovered && t.has_rows(t.rows.len()) {
            ctx.interaction.hover = Some(t.row_id(t.cursor));
            if v.pressed {
                ctx.interaction.pressed = Some(t.row_id(t.cursor));
            }
        }
        crate::ui::ctx::fill(buf, area, ratatui::style::Style::new().bg(bg));
        t.render(area, buf, ctx, bg);
    }
}

impl crate::core::pointer::MouseTarget for TreeView {
    type Action = TreeAction;

    fn mouse_id(&self) -> WidgetId {
        self.id
    }

    fn pointer(
        &mut self,
        ev: &crate::core::pointer::PointerEvent,
    ) -> crate::core::response::Response<TreeAction> {
        use crate::core::intent::Phase;
        if let Some(r) = crate::core::pointer::scroll_input(self, ev) {
            return r.lift();
        }
        if !ev.is(Phase::Click) {
            return crate::core::response::Response::ignored();
        }

        let hit = match (ev.index_of(Part::ICON), ev.index_of(Part::ROW)) {
            (Some(row), _) => Some((row, true)),
            (None, Some(row)) => Some((row, false)),
            _ => ev.index_of(Part::MARKER).map(|row| (row, false)),
        };
        match hit {
            Some((row, toggle)) => self.on_click_at(row, toggle, ev.mods(), ev.count()).into(),
            None => crate::core::response::Response::ignored(),
        }
    }
}

#[cfg(test)]
mod tests {
    use std::collections::HashSet;

    use super::*;

    const ID: WidgetId = WidgetId::of("tree.filter.tests");

    fn ref_matches(n: &TreeNode, q: &str) -> bool {
        n.label.to_lowercase().contains(q) || n.children.iter().any(|c| ref_matches(c, q))
    }

    fn ref_walk(
        nodes: &[TreeNode],
        path: &mut Path,
        depth: usize,
        exp: &HashSet<Path>,
        filter: Option<&str>,
        out: &mut Vec<FlatRow>,
    ) {
        for (i, n) in nodes.iter().enumerate() {
            if let Some(q) = filter
                && !ref_matches(n, q)
            {
                continue;
            }
            path.push(i);
            let has_children = !n.children.is_empty() || (n.lazy && !n.note);
            let expanded = has_children
                && (exp.contains(path)
                    || filter.is_some_and(|q| {
                        !n.children.is_empty() && !n.label.to_lowercase().contains(q)
                    }));
            out.push(FlatRow {
                path: path.clone(),
                depth,
                label: n.label.clone(),
                meta: n.meta.clone(),
                has_children,
                expanded,
                glyph: n.glyph.clone(),
                glyph_tone: n.glyph_tone,
                label_tone: n.label_tone,
                busy: n.busy,
                note: n.note,
            });
            if expanded {
                ref_walk(&n.children, path, depth + 1, exp, filter, out);
            }
            path.pop();
        }
    }

    fn reference_rows(nodes: &[TreeNode], exp: &HashSet<Path>, query: Option<&str>) -> Vec<String> {
        let filter = query
            .map(|f| f.trim().to_lowercase())
            .filter(|f| !f.is_empty());
        let mut out = Vec::new();
        ref_walk(nodes, &mut Vec::new(), 0, exp, filter.as_deref(), &mut out);
        out.iter().map(|r| format!("{r:?}")).collect()
    }

    fn actual_rows(tv: &TreeView) -> Vec<String> {
        tv.rows().iter().map(|r| format!("{r:?}")).collect()
    }

    fn count_nodes(nodes: &[TreeNode]) -> usize {
        nodes.iter().map(|n| 1 + count_nodes(&n.children)).sum()
    }

    fn all_paths(nodes: &[TreeNode], prefix: &mut Path, out: &mut Vec<Path>) {
        for (i, n) in nodes.iter().enumerate() {
            prefix.push(i);
            out.push(prefix.clone());
            all_paths(&n.children, prefix, out);
            prefix.pop();
        }
    }

    fn chain(depth: usize, last: &str) -> Vec<TreeNode> {
        let mut node = TreeNode::leaf(last);
        for level in (0..depth - 1).rev() {
            node = TreeNode::dir(&format!("level {level}"), vec![node]);
        }
        vec![node]
    }

    fn balanced(depth: usize, fanout: usize, tag: &str) -> Vec<TreeNode> {
        (0..fanout)
            .map(|i| {
                let label = format!("{tag}{i}");
                if depth <= 1 {
                    TreeNode::leaf(&label)
                } else {
                    TreeNode::dir(&label, balanced(depth - 1, fanout, &format!("{tag}{i}.")))
                }
            })
            .collect()
    }

    fn filtered(nodes: Vec<TreeNode>, query: &str) -> TreeView {
        let mut tv = TreeView::new(ID, nodes);
        tv.filter = Some(query.to_owned());
        work::take();
        tv.flatten();
        tv
    }

    #[test]
    fn filtered_tree_tests_each_label_once() {
        for depth in [32usize, 64, 128, 256] {
            let mut tv = TreeView::new(ID, chain(depth, "needle"));
            tv.filter = Some("needle".into());
            work::take();
            tv.flatten();
            let spent = work::take();

            assert_eq!(tv.rows().len(), depth, "depth {depth}: every ancestor row");
            for (k, row) in tv.rows().iter().enumerate() {
                assert_eq!(row.path, vec![0; k + 1], "depth {depth}: path of row {k}");
                assert_eq!(row.depth, k);
            }
            assert_eq!(
                spent.label_tests,
                depth,
                "depth {depth}: one label test per node, not {} (triangular)",
                depth * (depth + 1) / 2
            );
            assert_eq!(
                spent.projection_nodes, depth,
                "depth {depth}: one projection entry per node"
            );
        }
    }

    #[test]
    fn filtered_tree_tests_each_label_once_on_wide_deep_forest() {
        let nodes = balanced(5, 3, "n");
        let total = count_nodes(&nodes);
        let tv = filtered(nodes, "n2.2.2.2.2");
        let spent = work::take();

        assert_eq!(spent.label_tests, total);
        assert_eq!(spent.projection_nodes, total);
        assert_eq!(tv.rows().len(), 5, "only the matching chain is emitted");
        assert_eq!(
            tv.rows().last().map(|r| r.label.as_str()),
            Some("n2.2.2.2.2")
        );
    }

    #[test]
    fn filtered_tree_collapsed_own_match_reports_full_projection_cost() {
        let hidden: Vec<TreeNode> = (0..40)
            .map(|i| TreeNode::leaf(&format!("child {i}")))
            .collect();
        let nodes = vec![TreeNode::dir(
            "root",
            vec![TreeNode::dir("pkg-core", hidden), TreeNode::leaf("other")],
        )];
        let total = count_nodes(&nodes);
        let tv = filtered(nodes, "pkg");
        let spent = work::take();

        let labels: Vec<&str> = tv.rows().iter().map(|r| r.label.as_str()).collect();
        assert_eq!(labels, ["root", "pkg-core"], "own match stays collapsed");
        assert!(!tv.rows()[1].expanded);
        assert_eq!(
            spent.label_tests, total,
            "projection scans the collapsed subtree too"
        );
        assert_eq!(spent.projection_nodes, total);
        assert!(spent.label_tests > tv.rows().len());
    }

    #[test]
    fn filtered_tree_own_match_respects_expansion() {
        let nodes = vec![TreeNode::dir(
            "root",
            vec![
                TreeNode::dir(
                    "pkg-core",
                    vec![TreeNode::leaf("inner"), TreeNode::leaf("pkg-inner")],
                ),
                TreeNode::dir(
                    "lib",
                    vec![TreeNode::leaf("pkg-util"), TreeNode::leaf("skip")],
                ),
            ],
        )];
        let mut tv = TreeView::new(ID, nodes);
        tv.filter = Some("PKG".into());
        tv.flatten();
        let shape = |tv: &TreeView| -> Vec<(String, bool)> {
            tv.rows()
                .iter()
                .map(|r| (r.label.clone(), r.expanded))
                .collect()
        };
        assert_eq!(
            shape(&tv),
            [
                ("root".to_owned(), true),
                ("pkg-core".to_owned(), false),
                ("lib".to_owned(), true),
                ("pkg-util".to_owned(), false),
            ],
            "own match collapsed, descendant-only parent auto-expanded"
        );

        tv.expanded.insert(vec![0, 0]);
        tv.expanded.insert(vec![0, 1]);
        tv.flatten();
        assert_eq!(
            shape(&tv),
            [
                ("root".to_owned(), true),
                ("pkg-core".to_owned(), true),
                ("pkg-inner".to_owned(), false),
                ("lib".to_owned(), true),
                ("pkg-util".to_owned(), false),
            ],
            "explicit expansion stays active and children stay filtered"
        );
    }

    #[test]
    fn filtered_tree_query_normalization_and_cursor() {
        let nodes = vec![TreeNode::dir(
            "src",
            vec![
                TreeNode::leaf("main.rs"),
                TreeNode::leaf("ÄPFEL.rs"),
                TreeNode::dir(
                    "util",
                    vec![TreeNode::leaf("helper.rs"), TreeNode::leaf("mod.rs")],
                ),
            ],
        )];
        let mut tv = TreeView::new(ID, nodes);
        let unfiltered = actual_rows(&tv);

        tv.filter = Some("   ".into());
        work::take();
        tv.flatten();
        assert_eq!(
            actual_rows(&tv),
            unfiltered,
            "whitespace query is no filter"
        );
        assert_eq!(
            work::take(),
            work::MatchWork::default(),
            "no matching work without a filter"
        );

        tv.filter = Some("  äPFEL ".into());
        tv.flatten();
        let labels: Vec<&str> = tv.rows().iter().map(|r| r.label.as_str()).collect();
        assert_eq!(
            labels,
            ["src", "ÄPFEL.rs"],
            "Unicode lowercase and trimming"
        );

        tv.filter = None;
        tv.expanded.insert(vec![0, 2]);
        tv.flatten();
        tv.core.cursor = tv
            .rows()
            .iter()
            .position(|r| r.label == "helper.rs")
            .unwrap();
        tv.filter = Some("main".into());
        tv.flatten();
        assert_eq!(
            tv.cursor_path(),
            Some(&[0usize][..]),
            "falls back to the nearest visible ancestor"
        );
    }

    #[test]
    fn filtered_tree_cursor_relocates_to_next_then_previous_sibling() {
        let build = |labels: [&str; 5]| {
            let mut tv = TreeView::new(ID, labels.map(TreeNode::leaf).to_vec());
            tv.core.cursor = 2;
            tv.filter = Some("x-".into());
            tv.flatten();
            tv
        };
        let next = build(["x-a", "x-b", "y-c", "x-d", "x-e"]);
        assert_eq!(next.rows().len(), 4);
        assert_eq!(
            next.cursor_path(),
            Some(&[3usize][..]),
            "relocates to the row after"
        );
        assert_eq!(next.cursor, 2);

        let previous = build(["x-a", "x-b", "y-c", "y-d", "x-e"]);
        assert_eq!(previous.rows().len(), 3);
        assert_eq!(
            previous.cursor_path(),
            Some(&[1usize][..]),
            "falls back to the row before"
        );
    }

    #[test]
    fn unfiltered_refold_keeps_branch_local_work() {
        let nodes: Vec<TreeNode> = (0..50)
            .map(|i| {
                TreeNode::dir(
                    &format!("branch {i}"),
                    (0..4)
                        .map(|j| TreeNode::leaf(&format!("leaf {i}.{j}")))
                        .collect(),
                )
            })
            .collect();
        let mut tv = TreeView::new(ID, nodes);
        assert_eq!(tv.rows().len(), 250);

        let before = tv.rows_built;
        work::take();
        tv.toggle(5 * 5);
        assert_eq!(tv.rows().len(), 246);
        assert_eq!(tv.rows_built - before, 0, "collapse rebuilds no rows");
        tv.toggle(5 * 5);
        assert_eq!(tv.rows().len(), 250);
        assert_eq!(
            tv.rows_built - before,
            4,
            "expand rebuilds only that branch"
        );
        assert_eq!(work::take(), work::MatchWork::default());
    }

    struct Lcg(u64);

    impl Lcg {
        fn below(&mut self, n: u64) -> u64 {
            self.0 = self
                .0
                .wrapping_mul(6364136223846793005)
                .wrapping_add(1442695040888963407);
            (self.0 >> 33) % n
        }
    }

    fn random_forest(rng: &mut Lcg, depth: usize, max_children: u64) -> Vec<TreeNode> {
        const LABELS: [&str; 10] = [
            "Alpha",
            "beta",
            "GAMMA",
            "ÄPFEL",
            "straße",
            "İstanbul",
            "ΣΑΣ",
            "alpha-beta",
            "node",
            "",
        ];
        (0..rng.below(max_children + 1))
            .map(|_| {
                let mut n = TreeNode::leaf(LABELS[rng.below(LABELS.len() as u64) as usize]);
                match rng.below(8) {
                    0 => n.lazy = true,
                    1 => n.note = true,
                    2 => {
                        n.lazy = true;
                        n.note = true;
                    }
                    _ => {}
                }
                if depth > 1 && rng.below(3) != 0 {
                    n.children = random_forest(rng, depth - 1, max_children);
                }
                n
            })
            .collect()
    }

    #[test]
    fn filtered_tree_projection_matches_reference() {
        const QUERIES: [&str; 14] = [
            "a", "alpha", "BETA", "pha-", "äpfel", "ß", "i\u{307}", "σας", "node", "zzz", " ", "",
            "  Gamma ", "e",
        ];
        let mut rng = Lcg(0x5eed);
        let mut compared = 0usize;
        for _ in 0..60 {
            let nodes = random_forest(&mut rng, 4, 3);
            let mut paths = Vec::new();
            all_paths(&nodes, &mut Vec::new(), &mut paths);
            let expansions: [HashSet<Path>; 4] = [
                TreeView::new(ID, nodes.clone()).expanded,
                paths.iter().cloned().collect(),
                paths.iter().step_by(3).cloned().collect(),
                HashSet::new(),
            ];
            for exp in &expansions {
                for query in QUERIES.iter().map(|q| Some(*q)).chain([None]) {
                    let mut tv = TreeView::new(ID, nodes.clone());
                    tv.expanded = exp.clone();
                    tv.filter = query.map(str::to_owned);
                    tv.flatten();
                    assert_eq!(
                        actual_rows(&tv),
                        reference_rows(&nodes, exp, query),
                        "query {query:?} over {nodes:?} with expanded {exp:?}"
                    );
                    compared += 1;
                }
            }
        }
        assert!(compared > 3000);
    }
}
