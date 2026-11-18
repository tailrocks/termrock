use ratatui::buffer::Buffer;
use ratatui::crossterm::event::{KeyCode, KeyModifiers};
use std::collections::{HashMap, HashSet};

use ratatui::layout::Rect;
use ratatui::style::{Color, Modifier};

use crate::core::event::{Key, Outcome};
use crate::core::hints::{
    Binding, Bindings, HasHints, HintCx, actions_bindings, nav_bindings, nav_hints,
};
use crate::core::id::{ItemKey, Part, PartRef, WidgetId};
use crate::core::keyed::{KeyedSourceError, KeyedState};
use crate::core::keymap::{Actions, KeyBinding, KeyMap, Nav, NavKeys, lookup, unassigned_chord};
use crate::core::list::{
    Cursor, KeyStep, KeyedRowSource, Moved, Navigable, RowItem, RowSource, SourceStamp,
};
use crate::core::pointer::PointerEvent;
use crate::core::response::Response;
use crate::core::scroll::{ScrollPane, Scrollable};
use crate::core::selection::ClickPolicy;
use crate::theme::Tone;
use crate::ui::context::{ContextAnchor, ContextRequest, ContextSource, row_rect};
use crate::ui::ctx::RenderCtx;
use crate::ui::list_core::ListCore;
use crate::ui::row::{MarkKind, RowLook, RowUi};
use crate::widgets::keyhint::{Hint, hint};

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

/// One visible tree row as the shared engine reads it.
#[derive(Debug, Clone, Copy)]
struct RowRef<'a> {
    depth: usize,
    label: &'a str,
    meta: Option<&'a str>,
    glyph: Option<&'a str>,
    glyph_tone: Tone,
    label_tone: Option<Tone>,
    has_children: bool,
    expanded: bool,
    busy: bool,
    note: bool,
}

/// The visible rows the shared tree engine works over: owned flattened rows,
/// or a keyed projection that borrows labels from its source.
trait TreeRows: RowSource {
    fn row(&self, i: usize) -> RowRef<'_>;
    fn hit_key(&self, i: usize) -> ItemKey;
    /// Visible index of the row's parent, when it has one.
    fn parent(&self, i: usize) -> Option<usize>;
}

impl TreeRows for [FlatRow] {
    fn row(&self, i: usize) -> RowRef<'_> {
        let r = &self[i];
        RowRef {
            depth: r.depth,
            label: &r.label,
            meta: r.meta.as_deref(),
            glyph: r.glyph.as_deref(),
            glyph_tone: r.glyph_tone,
            label_tone: r.label_tone,
            has_children: r.has_children,
            expanded: r.expanded,
            busy: r.busy,
            note: r.note,
        }
    }

    fn hit_key(&self, i: usize) -> ItemKey {
        ItemKey::index(i)
    }

    fn parent(&self, i: usize) -> Option<usize> {
        let depth = self[i].depth;
        if depth == 0 {
            return None;
        }
        self[..i].iter().rposition(|r| r.depth < depth)
    }
}

/// What a key or click asks of the tree, before the owning storage applies it.
enum Plan {
    Done(Outcome),
    Command(TreeCmd),
    Toggle(usize),
    Recursive(usize),
    Branch(usize),
    Leaf(usize),
}

/// A tree action. Owned trees identify nodes by index [`Path`]; a keyed tree
/// identifies them by [`ItemKey`].
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TreeAction<K = Path> {
    Expand(K),

    Activate(K),
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

    keyed: KeyedState,
    projection: TreeProjection,
    layout_generation: u64,
    select_follows_cursor: bool,
    show_selection: bool,
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
    crate::record_work!(LabelsNormalized, 1usize);
    crate::record_work!(LabelBytesNormalized, lowered.len());
    #[cfg(test)]
    work::record_label_test(lowered.len());
    lowered.contains(query)
}

impl HasHints for TreeView {
    fn hints(&self, _: &HintCx) -> Vec<Hint> {
        let mut v = nav_hints(&self.nav, self.type_ahead, false);
        v.push(hint("←→", "Fold"));
        v
    }
}

impl Bindings for TreeView {
    fn bindings(&self) -> Vec<Binding> {
        let mut v = nav_bindings(&self.nav, self.type_ahead);
        v.push(Binding::group("←→", "Fold / unfold"));
        v.push(Binding::group("Enter", "Activate"));
        v.extend(actions_bindings(&self.keymap));
        v
    }
}

impl ContextSource for TreeView {
    type Target = usize;
    fn context_target(&self, ev: &PointerEvent) -> Option<usize> {
        self.row_at(ev)
    }
    fn context_select(&mut self, row: usize) {
        if let Some(i) = Navigable::cursor(self).select(row, self).index() {
            self.cursor = i;
        }
    }
    fn context_cursor(&self) -> Option<ContextRequest<usize>> {
        self.cursor_path()?;
        Some(ContextRequest {
            target: self.cursor,
            anchor: ContextAnchor::Control {
                id: self.row_id(self.cursor),
                fallback: row_rect(self.area, self.cursor, self.scroll.offset),
            },
        })
    }
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

#[derive(Clone, Copy, Debug, Default)]
struct MatchNode {
    own_match: bool,
    subtree_match: bool,
    subtree_end: usize,
}

fn project_matches(nodes: &[TreeNode], query: &str, out: &mut Vec<MatchNode>) -> bool {
    let mut any = false;
    for node in nodes {
        #[cfg(test)]
        work::record_projection_node();
        let ordinal = out.len();
        let own_match = label_matches(&node.label, query);
        out.push(MatchNode::default());
        let descendant = project_matches(&node.children, query, out);
        let subtree_match = own_match || descendant;
        out[ordinal] = MatchNode {
            own_match,
            subtree_match,
            subtree_end: out.len(),
        };
        any |= subtree_match;
    }
    any
}

fn flat_row(
    n: &TreeNode,
    path: &Path,
    depth: usize,
    has_children: bool,
    expanded: bool,
) -> FlatRow {
    FlatRow {
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
    }
}

fn walk(
    nodes: &[TreeNode],
    path: &mut Path,
    depth: usize,
    exp: &std::collections::HashSet<Path>,
    out: &mut Vec<FlatRow>,
) {
    for (i, n) in nodes.iter().enumerate() {
        path.push(i);
        let has_children = !n.children.is_empty() || (n.lazy && !n.note);
        let expanded = has_children && exp.contains(path);
        out.push(flat_row(n, path, depth, has_children, expanded));
        if expanded {
            walk(&n.children, path, depth + 1, exp, out);
        }
        path.pop();
    }
}

struct FilteredWalk<'a> {
    matches: &'a [MatchNode],
    exp: &'a std::collections::HashSet<Path>,
    ordinal: usize,
}

impl FilteredWalk<'_> {
    fn walk(&mut self, nodes: &[TreeNode], path: &mut Path, depth: usize, out: &mut Vec<FlatRow>) {
        for (i, n) in nodes.iter().enumerate() {
            let m = self.matches[self.ordinal];
            if !m.subtree_match {
                self.ordinal = m.subtree_end;
                continue;
            }
            self.ordinal += 1;
            path.push(i);
            let has_children = !n.children.is_empty() || (n.lazy && !n.note);
            let expanded = has_children
                && (self.exp.contains(path) || (!n.children.is_empty() && !m.own_match));
            out.push(flat_row(n, path, depth, has_children, expanded));
            if expanded {
                self.walk(&n.children, path, depth + 1, out);
                debug_assert_eq!(self.ordinal, m.subtree_end);
            }
            self.ordinal = m.subtree_end;
            path.pop();
        }
    }
}

fn build_rows(
    nodes: &[TreeNode],
    exp: &std::collections::HashSet<Path>,
    filter: Option<&str>,
) -> Vec<FlatRow> {
    let mut out = Vec::new();
    let Some(query) = filter else {
        walk(nodes, &mut Vec::new(), 0, exp, &mut out);
        return out;
    };
    let mut matches = Vec::new();
    project_matches(nodes, query, &mut matches);
    let mut filtered = FilteredWalk {
        matches: &matches,
        exp,
        ordinal: 0,
    };
    filtered.walk(nodes, &mut Vec::new(), 0, &mut out);
    debug_assert_eq!(filtered.ordinal, matches.len());
    out
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
            keyed: KeyedState::new(),
            projection: TreeProjection::default(),
            layout_generation: 0,
            select_follows_cursor: false,
            show_selection: true,
        };

        for (i, node) in tv.nodes.iter().enumerate() {
            if !(node.lazy && node.children.is_empty()) {
                tv.expanded.insert(vec![i]);
            }
        }
        tv.flatten();
        tv
    }

    /// In a tree driven through the `*_source` methods, selects the node the
    /// cursor lands on whenever it is a leaf. A branch under the cursor leaves
    /// the selection alone, and a filter that hides the selected node does not
    /// clear it.
    pub fn select_follows_cursor(mut self, on: bool) -> Self {
        self.select_follows_cursor = on;
        self
    }

    /// Whether a tree driven through the `*_source` methods paints its selected
    /// nodes. Off, the selection is tracked for the host but only the cursor
    /// row is drawn.
    pub fn show_selection(mut self, on: bool) -> Self {
        self.show_selection = on;
        self
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

    fn branch(&mut self, i: usize) -> Response<TreeAction> {
        let path = self.rows[i].path.clone();
        match self.branch_activate {
            BranchAction::Toggle => self.toggle(i),
            BranchAction::Activate => {
                self.selected = Some(path.clone());
                Response::emit(TreeAction::Activate(path))
            }
            BranchAction::Both => {
                let mut resp = self.toggle(i);
                self.selected = Some(path.clone());
                resp.push_action(TreeAction::Activate(path));
                resp
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
        let filter = self
            .filter
            .as_deref()
            .map(|f| f.trim().to_lowercase())
            .filter(|f| !f.is_empty());
        let out = build_rows(&self.nodes, &self.expanded, filter.as_deref());
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
        self.keyed.clear_cursor();
    }

    pub fn reveal(&mut self, path: &[usize]) {
        for k in 1..path.len() {
            self.expanded.insert(path[..k].to_vec());
        }
        self.flatten();
        if let Some(i) = self.rows.iter().position(|r| r.path == path) {
            self.set_cursor(self.rows.len(), i);
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

    fn set_cursor(&mut self, len: usize, i: usize) {
        self.core.cursor = i.min(len.saturating_sub(1));
        self.core.scroll.ensure_visible(self.core.cursor);
    }

    pub fn toggle(&mut self, i: usize) -> Response<TreeAction> {
        let Some(row) = self.rows.get(i) else {
            return Response::consumed();
        };
        if !row.has_children {
            return Response::consumed();
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
                return Response::from((Outcome::Changed, Some(TreeAction::Expand(path))));
            }
            return Response::changed();
        }
        self.refold(i);
        Response::changed()
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

    pub fn run(&mut self, cmd: TreeCmd) -> Response<TreeAction> {
        match cmd {
            TreeCmd::ExpandAll => self.expand_all(),
            TreeCmd::CollapseAll => self.collapse_all(),
        }
        Response::changed()
    }

    pub fn on_key(&mut self, key: &Key) -> Response<TreeAction> {
        let rows = std::mem::take(&mut self.rows);
        let plan = self.plan_key(&rows[..], key);
        self.rows = rows;
        self.apply_plan(plan)
    }

    fn apply_plan(&mut self, plan: Plan) -> Response<TreeAction> {
        match plan {
            Plan::Done(out) => Response::from(out),
            Plan::Command(cmd) => self.run(cmd),
            Plan::Toggle(i) => self.toggle(i),
            Plan::Recursive(i) => self.toggle_recursive(i),
            Plan::Branch(i) => self.branch(i),
            Plan::Leaf(i) => {
                let path = self.rows[i].path.clone();
                self.selected = Some(path.clone());
                Response::emit(TreeAction::Activate(path))
            }
        }
    }

    fn plan_key<R: TreeRows + ?Sized>(&mut self, rows: &R, key: &Key) -> Plan {
        if rows.len() == 0 {
            return Plan::Done(Outcome::Ignored);
        }
        if unassigned_chord(key, &self.keymap) {
            return Plan::Done(Outcome::Ignored);
        }
        if let Some(action) = lookup(&self.keymap, key) {
            return Plan::Command(action);
        }
        let (nav, moved) = match self.core.key_step(key, rows) {
            Some((KeyStep::Type(_), moved)) => {
                if let Moved::To(i) = moved {
                    self.set_cursor(rows.len(), i);
                }
                return Plan::Done(moved.outcome());
            }
            Some((KeyStep::Nav(nav), moved)) => (nav, moved),
            None => return self.plan_plain_key(rows, key),
        };
        let cursor = self.core.cursor;
        let out = match nav {
            Nav::Up | Nav::Down | Nav::PageUp | Nav::PageDown | Nav::Home | Nav::End => {
                if let Some(i) = moved.index() {
                    self.set_cursor(rows.len(), i);
                }
                Outcome::Changed
            }
            Nav::Right => {
                let row = rows.row(cursor);
                if row.has_children && !row.expanded {
                    return Plan::Toggle(cursor);
                } else if row.has_children {
                    self.set_cursor(rows.len(), cursor + 1);
                    Outcome::Changed
                } else {
                    Outcome::Consumed
                }
            }
            Nav::Left => {
                let row = rows.row(cursor);
                if row.has_children && row.expanded {
                    return Plan::Toggle(cursor);
                } else if row.depth > 0 {
                    if let Some(pi) = rows.parent(cursor) {
                        self.set_cursor(rows.len(), pi);
                    }
                    Outcome::Changed
                } else {
                    Outcome::Consumed
                }
            }
        };
        Plan::Done(out)
    }

    fn plan_plain_key<R: TreeRows + ?Sized>(&mut self, rows: &R, key: &Key) -> Plan {
        match key.code {
            KeyCode::Enter | KeyCode::Char(' ') => {
                let cursor = self.core.cursor;
                let row = rows.row(cursor);
                if row.note {
                    Plan::Done(Outcome::Consumed)
                } else if row.has_children {
                    Plan::Branch(cursor)
                } else {
                    Plan::Leaf(cursor)
                }
            }
            _ => Plan::Done(Outcome::Ignored),
        }
    }

    pub fn on_click_row(&mut self, i: usize) -> Response<TreeAction> {
        self.on_click_row_count(i, 1)
    }

    pub fn on_click_row_count(&mut self, i: usize, count: u8) -> Response<TreeAction> {
        let rows = std::mem::take(&mut self.rows);
        let plan = self.plan_click_row(&rows[..], i, count);
        self.rows = rows;
        self.apply_plan(plan)
    }

    fn plan_click_row<R: TreeRows + ?Sized>(&mut self, rows: &R, i: usize, count: u8) -> Plan {
        if i >= rows.len() {
            return Plan::Done(Outcome::Consumed);
        }
        let again = self.core.cursor == i;
        self.core.scroll.pointer_move();
        self.set_cursor(rows.len(), i);
        let row = rows.row(i);
        if row.note {
            return Plan::Done(Outcome::Consumed);
        }
        if !self.core.click.activates(count.max(1), again) {
            return Plan::Done(Outcome::Changed);
        }
        if row.has_children {
            Plan::Branch(i)
        } else {
            Plan::Leaf(i)
        }
    }

    pub fn on_click_toggle(&mut self, i: usize) -> Response<TreeAction> {
        self.core.scroll.pointer_move();
        self.set_cursor(self.rows.len(), i);
        self.toggle(i)
    }

    pub fn on_click_with(
        &mut self,
        i: usize,
        toggle: bool,
        mods: KeyModifiers,
    ) -> Response<TreeAction> {
        self.on_click_at(i, toggle, mods, 1)
    }

    pub fn on_click_at(
        &mut self,
        i: usize,
        toggle: bool,
        mods: KeyModifiers,
        count: u8,
    ) -> Response<TreeAction> {
        let rows = std::mem::take(&mut self.rows);
        let plan = self.plan_click(&rows[..], i, toggle, mods, count);
        self.rows = rows;
        self.apply_plan(plan)
    }

    fn plan_click<R: TreeRows + ?Sized>(
        &mut self,
        rows: &R,
        i: usize,
        toggle: bool,
        mods: KeyModifiers,
        count: u8,
    ) -> Plan {
        if self.alt_click_recursive
            && mods.contains(KeyModifiers::ALT)
            && i < rows.len()
            && rows.row(i).has_children
            && !rows.row(i).note
        {
            self.set_cursor(rows.len(), i);
            return Plan::Recursive(i);
        }
        if toggle {
            self.core.scroll.pointer_move();
            self.set_cursor(rows.len(), i);
            return Plan::Toggle(i);
        }
        self.plan_click_row(rows, i, count)
    }

    pub fn toggle_recursive(&mut self, i: usize) -> Response<TreeAction> {
        let Some(row) = self.rows.get(i) else {
            return Response::consumed();
        };
        if !row.has_children {
            return Response::consumed();
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
            return Response::from((Outcome::Changed, Some(TreeAction::Expand(path))));
        }
        Response::changed()
    }

    pub fn track_click(mut self, click: crate::core::scroll::TrackClick) -> Self {
        self.core.scroll.track_click = Some(click);
        self
    }

    pub fn render(&mut self, area: Rect, buf: &mut Buffer, ctx: &mut RenderCtx, bg: Color) {
        let rows = std::mem::take(&mut self.rows);
        let selected = self.selected.clone();
        self.render_in(
            &rows[..],
            |i| selected.as_deref() == Some(rows[i].path.as_slice()),
            area,
            buf,
            ctx,
            bg,
        );
        self.rows = rows;
    }

    fn render_in<R: TreeRows + ?Sized>(
        &mut self,
        rows: &R,
        is_selected: impl Fn(usize) -> bool,
        area: Rect,
        buf: &mut Buffer,
        ctx: &mut RenderCtx,
        bg: Color,
    ) {
        self.core.scroll.begin(ctx.track_click);
        let area = area.intersection(*buf.area());
        if area.is_empty() {
            return;
        }
        self.area = area;
        let t = ctx.theme;
        let focused = ctx.interaction.focused(self.id);
        self.core.scroll.set_viewport(area.height as usize);
        let n = rows.len();

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
                let row = rows.row(ri);
                let Some(m) = row.meta else { return true };
                let need = 1
                    + (row.depth as u16) * self.indent
                    + 2
                    + if row.glyph.is_some() { 2 } else { 0 }
                    + crate::ui::text::width(row.label) as u16
                    + 2
                    + crate::ui::text::width(m) as u16
                    + 1;
                need <= row_w
            });
        for (i, ri) in self.core.scroll.visible_range().enumerate() {
            crate::record_work!(VisibleRowsPainted, 1usize);
            let y = area.y + i as u16;
            let row = rows.row(ri);
            let hit = rows.hit_key(ri);
            let rect = Rect::new(area.x, y, row_w, 1);

            let full = Rect::new(area.x, y, full_w, 1);
            let rid = self.id.part(PartRef::item(Part::ROW, hit));
            let mut s = ctx.child_state(self.id, rid, ri == self.core.cursor);
            let tid = self.id.part(PartRef::item(Part::ICON, hit));

            s.hovered |= ctx.interaction.hovered(tid);
            s.pressed |= ctx.interaction.pressed(tid);

            let gid = self.id.part(PartRef::item(Part::MARKER, hit));
            s.hovered |= ctx.interaction.hovered(gid);
            s.pressed |= ctx.interaction.pressed(gid);
            if row.note {
                s.hovered = false;
                s.pressed = false;
            }
            s.focused = focused && ri == self.core.cursor;
            s.selected = is_selected(ri);
            s.inactive_selection = self.core.inactive_selection;
            let rp = RowUi::paint(ctx, buf, full, s, RowLook::Bar, bg);
            let st = rp.style();
            let indent = (row.depth as u16) * self.indent;
            let mut x = rect.x + 1 + indent;

            if !row.note {
                ctx.register_part(self.id, PartRef::item(Part::ROW, hit), full);
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
                    PartRef::item(Part::ICON, hit),
                    Rect::new(x, y, 2, 1),
                );
            } else if s.selected
                && !row.busy
                && let Some(m) = self.selected_marker
            {
                rp.mark(ctx, buf, x, y, MarkKind::Glyph(m), focused);
            }
            x += 2;
            let meta_w = row.meta.map(crate::ui::text::width).unwrap_or(0) as u16;
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
            if let Some(g) = row.glyph {
                if !row.note {
                    ctx.register_part(
                        self.id,
                        PartRef::item(Part::MARKER, hit),
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
                crate::ui::text::fit(row.label, (lw as usize).saturating_sub((lx - x) as usize)),
                label_style,
            );
            if let Some(m) = row.meta
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
                    PartRef::item(Part::ICON, hit),
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

impl RowItem for FlatRow {
    fn label(&self) -> &str {
        &self.label
    }
    fn enabled(&self) -> bool {
        !self.note
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
            Some((row, toggle)) => self.on_click_at(row, toggle, ev.mods(), ev.count()),
            None => crate::core::response::Response::ignored(),
        }
    }
}

/// One tree node borrowed from its provider. `children` are keys of nodes the
/// provider also serves through [`TreeSource::node`].
#[derive(Debug, Clone, Copy)]
pub struct TreeNodeRef<'a> {
    pub label: &'a str,
    pub meta: Option<&'a str>,
    pub glyph: Option<&'a str>,
    pub glyph_tone: Tone,
    pub label_tone: Option<Tone>,
    pub children: &'a [ItemKey],
    pub lazy: bool,
    pub busy: bool,
    pub note: bool,
}

const NO_NODE: TreeNodeRef<'static> = TreeNodeRef {
    label: "",
    meta: None,
    glyph: None,
    glyph_tone: Tone::Muted,
    label_tone: None,
    children: &[],
    lazy: false,
    busy: false,
    note: false,
};

/// A keyed provider of tree nodes. Every node must be reachable from exactly
/// one parent, keys must not be [`ItemKey::Index`], and the graph must be
/// acyclic; [`TreeView`] rejects a source that is not.
pub trait TreeSource {
    fn stamp(&self) -> SourceStamp;
    fn root_count(&self) -> usize;
    fn root_key(&self, index: usize) -> ItemKey;
    fn node(&self, key: ItemKey) -> Option<TreeNodeRef<'_>>;
}

#[derive(Debug, Clone, Copy)]
struct TreeRecord {
    key: ItemKey,
    depth: usize,
    parent: Option<usize>,
    has_children: bool,
    expanded: bool,
}

/// A validated source: preorder keys, key positions and parent positions.
type Validated = (Vec<ItemKey>, HashMap<ItemKey, usize>, Vec<Option<usize>>);

/// A flattening frame: child list (roots when `None`), next index, depth and
/// parent row.
type Frame<'a> = (Option<&'a [ItemKey]>, usize, usize, Option<usize>);

/// The keyed flattening of a [`TreeSource`]: only key, depth, parent and
/// flags per visible row, with labels borrowed from the source at paint.
#[derive(Debug, Clone, Default)]
struct TreeProjection {
    order: Vec<ItemKey>,
    positions: HashMap<ItemKey, usize>,
    parent_positions: Vec<Option<usize>>,
    own_match: Vec<bool>,
    subtree_match: Vec<bool>,
    rows: Vec<TreeRecord>,
    validated: Option<(ItemKey, u64)>,
    matched_for: Option<(ItemKey, u64, String)>,
    built: Option<(SourceStamp, u64, Option<String>)>,
    view: u64,
}

impl TreeProjection {
    /// Validates the source once per data revision: positional keys,
    /// duplicates (including a node with two parents), missing nodes and
    /// cycles. The previous projection survives an error.
    fn validate<S: TreeSource + ?Sized>(source: &S) -> Result<Validated, KeyedSourceError> {
        #[derive(Default)]
        struct Walk {
            order: Vec<ItemKey>,
            positions: HashMap<ItemKey, usize>,
            parents: Vec<Option<usize>>,
            on_stack: HashSet<ItemKey>,
        }
        impl Walk {
            fn enter<S: TreeSource + ?Sized>(
                &mut self,
                source: &S,
                key: ItemKey,
                parent: Option<usize>,
            ) -> Result<usize, KeyedSourceError> {
                if matches!(key, ItemKey::Index(_)) {
                    return Err(KeyedSourceError::PositionalKey {
                        index: self.order.len(),
                        authoritative: true,
                    });
                }
                if self.on_stack.contains(&key) {
                    return Err(KeyedSourceError::Cycle { key });
                }
                if self.positions.contains_key(&key) {
                    return Err(KeyedSourceError::DuplicateAuthoritative { key });
                }
                if source.node(key).is_none() {
                    return Err(KeyedSourceError::MissingNode { key });
                }
                let at = self.order.len();
                self.order.push(key);
                self.positions.insert(key, at);
                self.parents.push(parent);
                self.on_stack.insert(key);
                Ok(at)
            }
        }
        let mut walk = Walk::default();
        let mut stack: Vec<(&[ItemKey], usize, usize)> = Vec::new();
        for r in 0..source.root_count() {
            let root = source.root_key(r);
            let at = walk.enter(source, root, None)?;
            stack.push((source.node(root).map_or(&[][..], |n| n.children), 0, at));
            while let Some(frame) = stack.last_mut() {
                let (children, next, owner) = (frame.0, frame.1, frame.2);
                if next >= children.len() {
                    walk.on_stack.remove(&walk.order[owner]);
                    stack.pop();
                    continue;
                }
                frame.1 += 1;
                let child = children[next];
                let at = walk.enter(source, child, Some(owner))?;
                stack.push((source.node(child).map_or(&[][..], |n| n.children), 0, at));
            }
        }
        crate::record_work!(SourceRowsEnumerated, walk.order.len());
        Ok((walk.order, walk.positions, walk.parents))
    }

    fn refresh<S: TreeSource + ?Sized>(
        &mut self,
        source: &S,
        keyed: &KeyedState,
        filter: Option<&str>,
    ) -> Result<(), KeyedSourceError> {
        let stamp = source.stamp();
        let data_key = (stamp.instance, stamp.data);
        if self.validated != Some(data_key) {
            let (order, positions, parents) = Self::validate(source)?;
            self.order = order;
            self.positions = positions;
            self.parent_positions = parents;
            self.validated = Some(data_key);
            self.matched_for = None;
            self.built = None;
        }
        if let Some(query) = filter
            && self
                .matched_for
                .as_ref()
                .is_none_or(|m| (m.0, m.1, m.2.as_str()) != (stamp.instance, stamp.data, query))
        {
            self.compute_matches(source, query);
            self.matched_for = Some((stamp.instance, stamp.data, query.to_owned()));
            self.built = None;
        }
        let want = (stamp, keyed.expanded_revision(), filter.map(str::to_owned));
        if self.built.as_ref() == Some(&want) {
            return Ok(());
        }
        self.rebuild(source, keyed, filter);
        self.view += 1;
        self.built = Some(want);
        Ok(())
    }

    /// One postorder pass per (data revision, query): a node shows when its
    /// subtree holds a match.
    fn compute_matches<S: TreeSource + ?Sized>(&mut self, source: &S, query: &str) {
        self.own_match = self
            .order
            .iter()
            .map(|&key| label_matches(source.node(key).map_or("", |n| n.label), query))
            .collect();
        self.subtree_match = self.own_match.clone();
        for i in (0..self.order.len()).rev() {
            if self.subtree_match[i]
                && let Some(parent) = self.parent_positions[i]
            {
                self.subtree_match[parent] = true;
            }
        }
    }

    fn rebuild<S: TreeSource + ?Sized>(
        &mut self,
        source: &S,
        keyed: &KeyedState,
        filter: Option<&str>,
    ) {
        let mut rows = Vec::new();
        let mut stack: Vec<Frame<'_>> = vec![(None, 0, 0, None)];
        while let Some(frame) = stack.last_mut() {
            let (list, next, depth, parent) = (frame.0, frame.1, frame.2, frame.3);
            let key = match list {
                None if next < source.root_count() => source.root_key(next),
                Some(children) if next < children.len() => children[next],
                _ => {
                    stack.pop();
                    continue;
                }
            };
            frame.1 += 1;
            let at = self.positions[&key];
            if filter.is_some() && !self.subtree_match[at] {
                continue;
            }
            let node = source.node(key).unwrap_or(NO_NODE);
            let has_children = !node.children.is_empty() || (node.lazy && !node.note);
            let expanded = has_children
                && (keyed.is_expanded(key)
                    || (filter.is_some() && !node.children.is_empty() && !self.own_match[at]));
            rows.push(TreeRecord {
                key,
                depth,
                parent,
                has_children,
                expanded,
            });
            if expanded {
                stack.push((Some(node.children), 0, depth + 1, Some(rows.len() - 1)));
            }
        }
        self.rows = rows;
    }
}

/// The visible rows of a [`TreeSource`] projection, as the engine and the
/// keyed state read them.
struct KeyedTreeRows<'a, S: ?Sized> {
    source: &'a S,
    proj: &'a TreeProjection,
}

impl<S: TreeSource + ?Sized> KeyedTreeRows<'_, S> {
    fn node(&self, i: usize) -> TreeNodeRef<'_> {
        self.source.node(self.proj.rows[i].key).unwrap_or(NO_NODE)
    }
}

impl<S: TreeSource + ?Sized> RowSource for KeyedTreeRows<'_, S> {
    fn len(&self) -> usize {
        self.proj.rows.len()
    }
    fn enabled(&self, i: usize) -> bool {
        !self.node(i).note
    }
    fn label(&self, i: usize) -> &str {
        self.node(i).label
    }
    fn key(&self, i: usize) -> ItemKey {
        self.proj.rows[i].key
    }
}

impl<S: TreeSource + ?Sized> KeyedRowSource for KeyedTreeRows<'_, S> {
    fn stamp(&self) -> SourceStamp {
        let stamp = self.source.stamp();
        SourceStamp {
            instance: stamp.instance,
            data: stamp.data,
            view: self.proj.view,
        }
    }
    fn source_len(&self) -> usize {
        self.proj.order.len()
    }
    fn source_key(&self, i: usize) -> ItemKey {
        self.proj.order[i]
    }
}

impl<S: TreeSource + ?Sized> TreeRows for KeyedTreeRows<'_, S> {
    fn row(&self, i: usize) -> RowRef<'_> {
        let node = self.node(i);
        let rec = &self.proj.rows[i];
        RowRef {
            depth: rec.depth,
            label: node.label,
            meta: node.meta,
            glyph: node.glyph,
            glyph_tone: node.glyph_tone,
            label_tone: node.label_tone,
            has_children: rec.has_children,
            expanded: rec.expanded,
            busy: node.busy,
            note: node.note,
        }
    }
    fn hit_key(&self, i: usize) -> ItemKey {
        self.proj.rows[i].key
    }
    fn parent(&self, i: usize) -> Option<usize> {
        self.proj.rows[i].parent
    }
}

impl TreeView {
    /// The stable-identity state of a tree driven through the `*_source`
    /// methods: cursor, selection, expansion and pointer capture by key.
    pub fn keyed(&self) -> &KeyedState {
        &self.keyed
    }

    fn normalized_filter(&self) -> Option<String> {
        self.filter
            .as_deref()
            .map(|f| f.trim().to_lowercase())
            .filter(|f| !f.is_empty())
    }

    fn bind_tree<S: TreeSource + ?Sized>(&mut self, source: &S) -> Result<(), KeyedSourceError> {
        let first = self.keyed.stamp().is_none();
        let filter = self.normalized_filter();
        self.projection
            .refresh(source, &self.keyed, filter.as_deref())?;
        let old_cursor = self.keyed.cursor();
        let mut out = {
            let rows = KeyedTreeRows {
                source,
                proj: &self.projection,
            };
            self.keyed.reconcile(&rows)?
        };
        if first {
            for r in 0..source.root_count() {
                self.keyed.set_expanded(source.root_key(r), true);
            }
            self.projection
                .refresh(source, &self.keyed, filter.as_deref())?;
            let rows = KeyedTreeRows {
                source,
                proj: &self.projection,
            };
            let again = self.keyed.reconcile(&rows)?;
            out.view_changed |= again.view_changed;
            out.cursor_changed |= again.cursor_changed;
        }
        if out.cursor_changed
            && let Some(lost) = old_cursor
            && self.keyed.visible_index(lost).is_none()
            && let Some(&at) = self.projection.positions.get(&lost)
        {
            let mut cur = at;
            while let Some(parent) = self.projection.parent_positions[cur] {
                let key = self.projection.order[parent];
                if self.keyed.visible_index(key).is_some() {
                    self.keyed.set_cursor(key);
                    break;
                }
                cur = parent;
            }
        }
        if first || out.view_changed || out.data_changed || out.cursor_changed {
            let n = self.keyed.visible_len();
            self.core.cursor = self
                .keyed
                .cursor_view()
                .unwrap_or(0)
                .min(n.saturating_sub(1));
            self.core.scroll.set_content(n);
            self.core.scroll.ensure_visible(self.core.cursor);
        }
        Ok(())
    }

    fn note_layout(&mut self, area: Rect, buf: &Buffer) {
        if area.intersection(*buf.area()) != self.area {
            self.layout_generation += 1;
            self.keyed.invalidate_layout(self.layout_generation);
        }
    }

    fn select_key(&mut self, key: ItemKey) {
        self.keyed.replace_selection([key]);
    }

    fn toggle_key<S: TreeSource + ?Sized>(
        &mut self,
        source: &S,
        key: ItemKey,
    ) -> Response<TreeAction<ItemKey>> {
        if self.keyed.is_expanded(key) {
            self.keyed.set_expanded(key, false);
            return Response::changed();
        }
        self.keyed.set_expanded(key, true);
        let lazy = source
            .node(key)
            .is_some_and(|n| n.lazy && n.children.is_empty());
        if lazy {
            return Response::from((Outcome::Changed, Some(TreeAction::Expand(key))));
        }
        Response::changed()
    }

    fn set_subtree_expanded<S: TreeSource + ?Sized>(
        &mut self,
        source: &S,
        root: ItemKey,
        open: bool,
    ) {
        let mut stack = vec![root];
        while let Some(key) = stack.pop() {
            let Some(node) = source.node(key) else {
                continue;
            };
            if !node.children.is_empty() || key == root {
                self.keyed.set_expanded(key, open);
            }
            stack.extend(node.children.iter().copied());
        }
    }

    fn apply_keyed_plan<S: TreeSource + ?Sized>(
        &mut self,
        source: &S,
        plan: Plan,
    ) -> Response<TreeAction<ItemKey>> {
        let key_at = |me: &Self, i: usize| me.keyed.visible_key(i);
        match plan {
            Plan::Done(out) => Response::from(out),
            Plan::Command(TreeCmd::ExpandAll) => {
                let keys: Vec<ItemKey> = self.projection.order.clone();
                for key in keys {
                    if source.node(key).is_some_and(|n| !n.children.is_empty()) {
                        self.keyed.set_expanded(key, true);
                    }
                }
                Response::changed()
            }
            Plan::Command(TreeCmd::CollapseAll) => {
                let open: Vec<ItemKey> = self.keyed.expanded().collect();
                for key in open {
                    self.keyed.set_expanded(key, false);
                }
                Response::changed()
            }
            Plan::Toggle(i) => match key_at(self, i) {
                Some(key) if self.projection.rows[i].has_children => self.toggle_key(source, key),
                _ => Response::consumed(),
            },
            Plan::Recursive(i) => {
                let Some(key) = key_at(self, i) else {
                    return Response::consumed();
                };
                let open = !self.projection.rows[i].expanded;
                let lazy = source
                    .node(key)
                    .is_some_and(|n| n.lazy && n.children.is_empty());
                self.set_subtree_expanded(source, key, open);
                if open && lazy {
                    return Response::from((Outcome::Changed, Some(TreeAction::Expand(key))));
                }
                Response::changed()
            }
            Plan::Branch(i) => {
                let Some(key) = key_at(self, i) else {
                    return Response::consumed();
                };
                match self.branch_activate {
                    BranchAction::Toggle => self.toggle_key(source, key),
                    BranchAction::Activate => {
                        self.select_key(key);
                        Response::emit(TreeAction::Activate(key))
                    }
                    BranchAction::Both => {
                        let mut resp = self.toggle_key(source, key);
                        self.select_key(key);
                        resp.push_action(TreeAction::Activate(key));
                        resp
                    }
                }
            }
            Plan::Leaf(i) => {
                let Some(key) = key_at(self, i) else {
                    return Response::consumed();
                };
                self.select_key(key);
                Response::emit(TreeAction::Activate(key))
            }
        }
    }

    /// Moves the cursor to `key` by identity and scrolls it into view. A key
    /// that is unknown, hidden by a filter or inside a collapsed branch is
    /// refused. Returns whether the cursor now rests on `key`.
    pub fn set_cursor_source<S: TreeSource + ?Sized>(
        &mut self,
        key: ItemKey,
        source: &S,
    ) -> Result<bool, KeyedSourceError> {
        self.bind_tree(source)?;
        self.keyed.set_cursor(key);
        if let Some(row) = self.keyed.cursor_view() {
            self.core.cursor = row;
            self.core.scroll.ensure_visible(row);
        }
        Ok(self.keyed.cursor() == Some(key))
    }

    /// Makes `key` the only selected node. Returns whether the selection
    /// changed; an unknown or disabled key is refused.
    pub fn select_source<S: TreeSource + ?Sized>(
        &mut self,
        key: ItemKey,
        source: &S,
    ) -> Result<bool, KeyedSourceError> {
        self.bind_tree(source)?;
        let selectable = self.keyed.is_authoritative(key)
            && source.node(key).is_some_and(|node| !node.note)
            && (self.keyed.visible_index(key).is_none() || self.keyed.is_enabled(key));
        if !selectable {
            return Ok(false);
        }
        Ok(self.keyed.replace_selection([key]))
    }

    fn follow_cursor(&mut self) {
        if !self.select_follows_cursor {
            return;
        }
        if let Some(row) = self.keyed.cursor_view()
            && !self.projection.rows[row].has_children
        {
            self.select_key(self.projection.rows[row].key);
        }
    }

    /// Expands every node of the provider that has children.
    pub fn expand_all_source<S: TreeSource + ?Sized>(
        &mut self,
        source: &S,
    ) -> Result<(), KeyedSourceError> {
        self.bind_tree(source)?;
        let keys: Vec<ItemKey> = self.projection.order.clone();
        for key in keys {
            if source.node(key).is_some_and(|n| !n.children.is_empty()) {
                self.keyed.set_expanded(key, true);
            }
        }
        Ok(())
    }

    /// Keyboard input over a borrowed keyed provider. Expansion, selection and
    /// the cursor are held by key, so inserting an earlier sibling moves none
    /// of them.
    pub fn on_key_source<S: TreeSource + ?Sized>(
        &mut self,
        key: &Key,
        source: &S,
    ) -> Result<Response<TreeAction<ItemKey>>, KeyedSourceError> {
        self.bind_tree(source)?;
        let proj = std::mem::take(&mut self.projection);
        let plan = self.plan_key(
            &KeyedTreeRows {
                source,
                proj: &proj,
            },
            key,
        );
        self.projection = proj;
        self.keyed.adopt_cursor_view(self.core.cursor);
        let response = self.apply_keyed_plan(source, plan);
        self.follow_cursor();
        Ok(response)
    }

    /// Pointer input over a borrowed keyed provider. Hit regions carry the
    /// node key, so a press can only complete on the node it began on.
    pub fn on_pointer_source<S: TreeSource + ?Sized>(
        &mut self,
        ev: &crate::core::pointer::PointerEvent,
        source: &S,
    ) -> Result<Response<TreeAction<ItemKey>>, KeyedSourceError> {
        use crate::core::intent::Phase;
        self.bind_tree(source)?;
        self.keyed.note_pointer(ev.phase());
        if let Some(r) = crate::core::pointer::scroll_input(self, ev) {
            return Ok(r.lift());
        }
        let hit = match (ev.key_of(Part::ICON), ev.key_of(Part::ROW)) {
            (Some(key), _) => Some((key, true)),
            (None, Some(key)) => Some((key, false)),
            _ => ev.key_of(Part::MARKER).map(|key| (key, false)),
        };
        let Some((key, toggle)) = hit.filter(|_| ev.on(self.id)) else {
            return Ok(Response::ignored());
        };
        match ev.phase() {
            Some(Phase::Press) => {
                self.keyed.capture_press(key, self.layout_generation);
                Ok(Response::ignored())
            }
            Some(Phase::Click) => {
                if !self.keyed.release_click(key, self.layout_generation) {
                    return Ok(Response::consumed());
                }
                let Some(row) = self.keyed.visible_index(key) else {
                    return Ok(Response::consumed());
                };
                let proj = std::mem::take(&mut self.projection);
                let plan = self.plan_click(
                    &KeyedTreeRows {
                        source,
                        proj: &proj,
                    },
                    row,
                    toggle,
                    ev.mods(),
                    ev.count(),
                );
                self.projection = proj;
                self.keyed.adopt_cursor_view(self.core.cursor);
                let response = self.apply_keyed_plan(source, plan);
                self.follow_cursor();
                Ok(response)
            }
            _ => Ok(Response::ignored()),
        }
    }

    /// Paints nodes borrowed from a keyed provider.
    pub fn render_source<S: TreeSource + ?Sized>(
        &mut self,
        area: Rect,
        source: &S,
        buf: &mut Buffer,
        ctx: &mut RenderCtx,
        bg: Color,
    ) -> Result<(), KeyedSourceError> {
        self.bind_tree(source)?;
        self.note_layout(area, buf);
        let proj = std::mem::take(&mut self.projection);
        let keyed = std::mem::take(&mut self.keyed);
        let show_selection = self.show_selection;
        let rows = KeyedTreeRows {
            source,
            proj: &proj,
        };
        self.render_in(
            &rows,
            |i| show_selection && keyed.is_selected(proj.rows[i].key),
            area,
            buf,
            ctx,
            bg,
        );
        self.projection = proj;
        self.keyed = keyed;
        Ok(())
    }
}

/// One node of a [`KeyedTree`].
#[derive(Debug, Clone)]
pub struct KeyedTreeNode {
    pub label: String,
    pub meta: Option<String>,
    pub glyph: Option<std::borrow::Cow<'static, str>>,
    pub glyph_tone: Tone,
    pub label_tone: Option<Tone>,
    pub lazy: bool,
    pub busy: bool,
    pub note: bool,
    children: Vec<ItemKey>,
}

impl KeyedTreeNode {
    pub fn new(label: &str) -> Self {
        Self {
            label: label.to_owned(),
            meta: None,
            glyph: None,
            glyph_tone: Tone::Muted,
            label_tone: None,
            lazy: false,
            busy: false,
            note: false,
            children: Vec::new(),
        }
    }

    pub fn lazy(label: &str) -> Self {
        Self {
            lazy: true,
            ..Self::new(label)
        }
    }

    pub fn meta(mut self, meta: &str) -> Self {
        self.meta = Some(meta.to_owned());
        self
    }

    pub fn glyph(mut self, glyph: impl Into<std::borrow::Cow<'static, str>>) -> Self {
        self.glyph = Some(glyph.into());
        self
    }

    pub fn children(&self) -> &[ItemKey] {
        &self.children
    }
}

/// An owned keyed tree store that advances its own revisions. A `Clone` is an
/// independent store with a fresh instance.
#[derive(Debug)]
pub struct KeyedTree {
    instance: ItemKey,
    data: u64,
    view: u64,
    roots: Vec<ItemKey>,
    nodes: HashMap<ItemKey, KeyedTreeNode>,
    parents: HashMap<ItemKey, Option<ItemKey>>,
}

impl Clone for KeyedTree {
    fn clone(&self) -> Self {
        Self {
            instance: crate::core::keyed::fresh_instance(),
            data: self.data,
            view: self.view,
            roots: self.roots.clone(),
            nodes: self.nodes.clone(),
            parents: self.parents.clone(),
        }
    }
}

impl Default for KeyedTree {
    fn default() -> Self {
        Self::new()
    }
}

impl KeyedTree {
    pub fn new() -> Self {
        Self {
            instance: crate::core::keyed::fresh_instance(),
            data: 0,
            view: 0,
            roots: Vec::new(),
            nodes: HashMap::new(),
            parents: HashMap::new(),
        }
    }

    pub fn get(&self, key: ItemKey) -> Option<&KeyedTreeNode> {
        self.nodes.get(&key)
    }

    pub fn len(&self) -> usize {
        self.nodes.len()
    }

    pub fn is_empty(&self) -> bool {
        self.nodes.is_empty()
    }

    /// Inserts `node` under `parent` (a root when `None`) before `position`.
    pub fn insert(
        &mut self,
        parent: Option<ItemKey>,
        position: usize,
        key: ItemKey,
        node: KeyedTreeNode,
    ) -> Result<(), KeyedSourceError> {
        if matches!(key, ItemKey::Index(_)) {
            return Err(KeyedSourceError::PositionalKey {
                index: position,
                authoritative: true,
            });
        }
        if self.nodes.contains_key(&key) {
            return Err(KeyedSourceError::DuplicateAuthoritative { key });
        }
        if let Some(parent) = parent
            && !self.nodes.contains_key(&parent)
        {
            return Err(KeyedSourceError::MissingNode { key: parent });
        }
        let siblings = match parent {
            Some(parent) => &mut self.nodes.get_mut(&parent).expect("checked").children,
            None => &mut self.roots,
        };
        let at = position.min(siblings.len());
        siblings.insert(at, key);
        self.nodes.insert(
            key,
            KeyedTreeNode {
                children: Vec::new(),
                ..node
            },
        );
        self.parents.insert(key, parent);
        self.data += 1;
        self.view += 1;
        Ok(())
    }

    /// Removes a node and its whole subtree.
    pub fn remove(&mut self, key: ItemKey) -> bool {
        let Some(parent) = self.parents.remove(&key) else {
            return false;
        };
        let siblings = match parent {
            Some(parent) => self.nodes.get_mut(&parent).map(|n| &mut n.children),
            None => Some(&mut self.roots),
        };
        if let Some(siblings) = siblings {
            siblings.retain(|k| *k != key);
        }
        let mut stack = vec![key];
        while let Some(next) = stack.pop() {
            if let Some(node) = self.nodes.remove(&next) {
                for child in node.children {
                    self.parents.remove(&child);
                    stack.push(child);
                }
            }
        }
        self.data += 1;
        self.view += 1;
        true
    }

    /// Edits one node's own fields. Its `children` are not reachable here, so
    /// the closure cannot break the tree shape.
    pub fn update(&mut self, key: ItemKey, edit: impl FnOnce(&mut KeyedTreeNode)) -> bool {
        let Some(node) = self.nodes.get_mut(&key) else {
            return false;
        };
        let children = std::mem::take(&mut node.children);
        edit(node);
        node.children = children;
        self.data += 1;
        self.view += 1;
        true
    }
}

impl TreeSource for KeyedTree {
    fn stamp(&self) -> SourceStamp {
        SourceStamp {
            instance: self.instance,
            data: self.data,
            view: self.view,
        }
    }

    fn root_count(&self) -> usize {
        self.roots.len()
    }

    fn root_key(&self, index: usize) -> ItemKey {
        self.roots[index]
    }

    fn node(&self, key: ItemKey) -> Option<TreeNodeRef<'_>> {
        let n = self.nodes.get(&key)?;
        Some(TreeNodeRef {
            label: &n.label,
            meta: n.meta.as_deref(),
            glyph: n.glyph.as_deref(),
            glyph_tone: n.glyph_tone,
            label_tone: n.label_tone,
            children: &n.children,
            lazy: n.lazy,
            busy: n.busy,
            note: n.note,
        })
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
        let _ = tv.toggle(5 * 5);
        assert_eq!(tv.rows().len(), 246);
        assert_eq!(tv.rows_built - before, 0, "collapse rebuilds no rows");
        let _ = tv.toggle(5 * 5);
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
