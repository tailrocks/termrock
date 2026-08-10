// SPDX-FileCopyrightText: 2026 Alexey Zhokhov
// SPDX-License-Identifier: Apache-2.0

//! **DependencyGraph** — constrained graph viewer for package, service, schema,
//! and task dependencies.
//!
//! **Mission.** Nodes, edges, direction, status, selection, search, filtering,
//! grouping, details, and alternative **list/tree** representation. Does **not**
//! promise arbitrary graph-layout quality beyond terminal constraints.
//! Deterministic layered layouts, pan navigation, ASCII connectors. Falls back
//! to TreeTable-shaped projection when the graph is unreadable. Benchmark
//! moderate real-world graphs.
//!
//! **Ownership.** Host owns dependency resolution and package metadata.
//! TermRock owns layout, paint, selection chrome, and typed outcomes.
//!
//! Research: terminal graph tools, dependency trees, service maps, FTXUI canvases.

use std::collections::{BTreeMap, BTreeSet, VecDeque};

use ratatui_core::{
    buffer::Buffer,
    layout::Rect,
};

use crate::{
    input::{KeyCode, KeyEvent, KeyEventKind, KeyModifiers, MouseButton, MouseEvent, MouseEventKind},
    style::{DesignSystem, Role},
    text::take_display_cols,
    widgets::{
        data_view::{ColumnModel, DataColumn, DataColumnWidth},
        object_inspector::{InspectKind, InspectorField},
    },
};

/// Max nodes for graph view before auto tree fallback (host may override).
pub const DEP_GRAPH_AUTO_TREE_NODES: usize = 48;
/// Width at/below which graph falls back to tree.
pub const DEP_GRAPH_NARROW_MAX_WIDTH: u16 = 40;
/// Cell width for layered layout (chars).
pub const DEP_GRAPH_CELL_W: u16 = 14;
/// Cell height for layered layout (rows).
pub const DEP_GRAPH_CELL_H: u16 = 3;

// ── Domain ──────────────────────────────────────────────────────────────────

/// Node domain kind.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
#[non_exhaustive]
pub enum DepNodeKind {
    /// Package / crate.
    #[default]
    Package,
    /// Service / process.
    Service,
    /// Schema / namespace.
    Schema,
    /// Table / collection.
    Table,
    /// Task / job.
    Task,
    /// Module / library unit.
    Module,
    /// Other.
    Other,
}

impl DepNodeKind {
    /// Stable id.
    #[must_use]
    pub const fn id(self) -> &'static str {
        match self {
            Self::Package => "package",
            Self::Service => "service",
            Self::Schema => "schema",
            Self::Table => "table",
            Self::Task => "task",
            Self::Module => "module",
            Self::Other => "other",
        }
    }

    /// Glyph.
    #[must_use]
    pub const fn glyph(self, ascii: bool) -> &'static str {
        if ascii {
            match self {
                Self::Package => "P",
                Self::Service => "S",
                Self::Schema => "C",
                Self::Table => "T",
                Self::Task => "J",
                Self::Module => "M",
                Self::Other => "?",
            }
        } else {
            match self {
                Self::Package => "▣",
                Self::Service => "⬡",
                Self::Schema => "▤",
                Self::Table => "▦",
                Self::Task => "▸",
                Self::Module => "◇",
                Self::Other => "?",
            }
        }
    }
}

/// Node health / status.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default, PartialOrd, Ord)]
#[non_exhaustive]
pub enum DepNodeStatus {
    /// Healthy / resolved.
    #[default]
    Ok,
    /// Warning (outdated, deprecated).
    Warning,
    /// Error / conflict.
    Error,
    /// Missing dependency.
    Missing,
    /// Loading metadata.
    Loading,
}

impl DepNodeStatus {
    /// Stable id.
    #[must_use]
    pub const fn id(self) -> &'static str {
        match self {
            Self::Ok => "ok",
            Self::Warning => "warning",
            Self::Error => "error",
            Self::Missing => "missing",
            Self::Loading => "loading",
        }
    }

    /// Letter.
    #[must_use]
    pub const fn letter(self) -> char {
        match self {
            Self::Ok => '·',
            Self::Warning => '!',
            Self::Error => 'x',
            Self::Missing => '?',
            Self::Loading => '…',
        }
    }

    /// ASCII letter.
    #[must_use]
    pub const fn letter_ascii(self) -> char {
        match self {
            Self::Ok => '.',
            Self::Warning => '!',
            Self::Error => 'x',
            Self::Missing => '?',
            Self::Loading => '.',
        }
    }

    /// Role.
    #[must_use]
    pub const fn role(self) -> Role {
        match self {
            Self::Ok => Role::Success,
            Self::Warning | Self::Loading => Role::Warning,
            Self::Error | Self::Missing => Role::Danger,
        }
    }
}

/// Edge semantics.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
#[non_exhaustive]
pub enum DepEdgeKind {
    /// Depends on (default package edge).
    #[default]
    DependsOn,
    /// Imports / uses.
    Imports,
    /// Calls / invokes.
    Calls,
    /// Contains / owns.
    Contains,
    /// Blocks / waits on.
    Blocks,
}

impl DepEdgeKind {
    /// Stable id.
    #[must_use]
    pub const fn id(self) -> &'static str {
        match self {
            Self::DependsOn => "depends",
            Self::Imports => "imports",
            Self::Calls => "calls",
            Self::Contains => "contains",
            Self::Blocks => "blocks",
        }
    }

    /// Connector glyph preference.
    #[must_use]
    pub const fn arrow(self, ascii: bool) -> &'static str {
        if ascii {
            "->"
        } else {
            match self {
                Self::DependsOn | Self::Imports => "→",
                Self::Calls => "⇒",
                Self::Contains => "⊃",
                Self::Blocks => "↛",
            }
        }
    }
}

/// One graph node (host projection).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DepNode<'a> {
    /// Stable id.
    pub id: &'a str,
    /// Display label.
    pub label: &'a str,
    /// Kind.
    pub kind: DepNodeKind,
    /// Status.
    pub status: DepNodeStatus,
    /// Optional group (layering hint / filter).
    pub group: Option<&'a str>,
    /// Version / detail.
    pub detail: Option<&'a str>,
    /// Enabled.
    pub enabled: bool,
}

impl<'a> DepNode<'a> {
    /// Construct.
    #[must_use]
    pub const fn new(id: &'a str, label: &'a str) -> Self {
        Self {
            id,
            label,
            kind: DepNodeKind::Package,
            status: DepNodeStatus::Ok,
            group: None,
            detail: None,
            enabled: true,
        }
    }

    /// Kind.
    #[must_use]
    pub const fn kind(mut self, k: DepNodeKind) -> Self {
        self.kind = k;
        self
    }

    /// Status.
    #[must_use]
    pub const fn status(mut self, s: DepNodeStatus) -> Self {
        self.status = s;
        self
    }

    /// Group.
    #[must_use]
    pub const fn group(mut self, g: &'a str) -> Self {
        self.group = Some(g);
        self
    }

    /// Detail.
    #[must_use]
    pub const fn detail(mut self, d: &'a str) -> Self {
        self.detail = Some(d);
        self
    }
}

/// Directed or undirected edge.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DepEdge<'a> {
    /// Source id.
    pub from: &'a str,
    /// Target id.
    pub to: &'a str,
    /// Kind.
    pub kind: DepEdgeKind,
    /// Directed (true = arrow from→to).
    pub directed: bool,
}

impl<'a> DepEdge<'a> {
    /// Directed depends-on.
    #[must_use]
    pub const fn new(from: &'a str, to: &'a str) -> Self {
        Self {
            from,
            to,
            kind: DepEdgeKind::DependsOn,
            directed: true,
        }
    }

    /// Kind.
    #[must_use]
    pub const fn kind(mut self, k: DepEdgeKind) -> Self {
        self.kind = k;
        self
    }

    /// Undirected.
    #[must_use]
    pub const fn undirected(mut self) -> Self {
        self.directed = false;
        self
    }
}

// ── View / layout ───────────────────────────────────────────────────────────

/// Presentation mode.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
#[non_exhaustive]
pub enum DependencyGraphView {
    /// Layered graph (default when readable).
    #[default]
    Graph,
    /// TreeTable-shaped fallback / explicit tree.
    Tree,
    /// Flat adjacency list.
    List,
}

impl DependencyGraphView {
    /// Stable id.
    #[must_use]
    pub const fn id(self) -> &'static str {
        match self {
            Self::Graph => "graph",
            Self::Tree => "tree",
            Self::List => "list",
        }
    }

    /// Cycle Graph → Tree → List → Graph.
    #[must_use]
    pub const fn cycle(self) -> Self {
        match self {
            Self::Graph => Self::Tree,
            Self::Tree => Self::List,
            Self::List => Self::Graph,
        }
    }
}

/// Why graph view is considered unreadable.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[non_exhaustive]
pub enum GraphUnreadableReason {
    /// Too many nodes.
    TooManyNodes,
    /// Viewport too narrow.
    Narrow,
    /// Host forced tree.
    Forced,
}

impl GraphUnreadableReason {
    /// Stable id.
    #[must_use]
    pub const fn id(self) -> &'static str {
        match self {
            Self::TooManyNodes => "too-many-nodes",
            Self::Narrow => "narrow",
            Self::Forced => "forced",
        }
    }
}

/// Decide view: auto tree when unreadable unless host overrides.
#[must_use]
pub fn choose_dependency_view(
    preferred: DependencyGraphView,
    node_count: usize,
    width: u16,
    force_tree: bool,
) -> (DependencyGraphView, Option<GraphUnreadableReason>) {
    if force_tree {
        return (DependencyGraphView::Tree, Some(GraphUnreadableReason::Forced));
    }
    if matches!(preferred, DependencyGraphView::Tree | DependencyGraphView::List) {
        return (preferred, None);
    }
    if width <= DEP_GRAPH_NARROW_MAX_WIDTH {
        return (DependencyGraphView::Tree, Some(GraphUnreadableReason::Narrow));
    }
    if node_count > DEP_GRAPH_AUTO_TREE_NODES {
        return (
            DependencyGraphView::Tree,
            Some(GraphUnreadableReason::TooManyNodes),
        );
    }
    (DependencyGraphView::Graph, None)
}

/// Placed node in layered layout (cell coordinates).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DepLayoutNode {
    /// Node id.
    pub id: String,
    /// Layer index (0 = roots / sources).
    pub layer: usize,
    /// Index within layer.
    pub slot: usize,
    /// Character x (left of cell).
    pub x: u16,
    /// Character y (top of cell).
    pub y: u16,
}

/// Deterministic layered layout from directed edges.
///
/// Roots = nodes with no incoming edges. Cycles get remaining nodes assigned
/// by stable id order after topo layers. Not a general graph drawing algorithm.
#[must_use]
pub fn layout_dependency_layers(
    nodes: &[DepNode<'_>],
    edges: &[DepEdge<'_>],
) -> Vec<DepLayoutNode> {
    let ids: Vec<&str> = nodes.iter().map(|n| n.id).collect();
    let id_set: BTreeSet<&str> = ids.iter().copied().collect();

    let mut indeg: BTreeMap<&str, usize> = BTreeMap::new();
    let mut outs: BTreeMap<&str, Vec<&str>> = BTreeMap::new();
    for id in &ids {
        indeg.entry(*id).or_insert(0);
        outs.entry(*id).or_default();
    }
    for e in edges {
        if !id_set.contains(e.from) || !id_set.contains(e.to) {
            continue;
        }
        if e.directed {
            *indeg.entry(e.to).or_insert(0) += 1;
            outs.entry(e.from).or_default().push(e.to);
        } else {
            outs.entry(e.from).or_default().push(e.to);
            outs.entry(e.to).or_default().push(e.from);
            *indeg.entry(e.to).or_insert(0) += 1;
        }
    }
    for v in outs.values_mut() {
        v.sort_unstable();
        v.dedup();
    }

    // Kahn topological layers (deterministic).
    let mut remaining = indeg.clone();
    let mut layer_of: BTreeMap<&str, usize> = BTreeMap::new();
    let mut q: VecDeque<&str> = remaining
        .iter()
        .filter(|(_, d)| **d == 0)
        .map(|(k, _)| *k)
        .collect();
    {
        let mut tmp: Vec<&str> = q.drain(..).collect();
        tmp.sort_unstable();
        q.extend(tmp);
    }
    let mut ordered: Vec<&str> = Vec::new();
    while let Some(u) = q.pop_front() {
        if layer_of.contains_key(u) {
            continue;
        }
        layer_of.insert(u, 0);
        ordered.push(u);
        // recompute layer as max(pred)+1 after full pass — assign after
        let mut nexts = outs.get(u).cloned().unwrap_or_default();
        nexts.sort_unstable();
        for v in nexts {
            let entry = remaining.entry(v).or_insert(0);
            *entry = entry.saturating_sub(1);
            if *entry == 0 {
                q.push_back(v);
            }
        }
    }
    // Assign layers properly: longest path from roots
    layer_of.clear();
    for id in &ids {
        if *indeg.get(id).unwrap_or(&0) == 0 {
            layer_of.insert(*id, 0);
        }
    }
    // relax |V| times
    for _ in 0..ids.len().max(1) {
        for e in edges {
            if !id_set.contains(e.from) || !id_set.contains(e.to) {
                continue;
            }
            if !e.directed {
                continue;
            }
            if let Some(&lf) = layer_of.get(e.from) {
                let entry = layer_of.entry(e.to).or_insert(lf + 1);
                if *entry < lf + 1 {
                    *entry = lf + 1;
                }
            }
        }
    }
    for id in &ids {
        layer_of.entry(*id).or_insert(0);
    }

    let mut by_layer: BTreeMap<usize, Vec<&str>> = BTreeMap::new();
    for id in &ids {
        let l = *layer_of.get(id).unwrap_or(&0);
        by_layer.entry(l).or_default().push(*id);
    }
    for v in by_layer.values_mut() {
        v.sort_unstable();
    }

    let mut out = Vec::with_capacity(nodes.len());
    for (layer, ids_in) in by_layer {
        for (slot, id) in ids_in.into_iter().enumerate() {
            out.push(DepLayoutNode {
                id: id.to_string(),
                layer,
                slot,
                x: (slot as u16).saturating_mul(DEP_GRAPH_CELL_W),
                y: (layer as u16).saturating_mul(DEP_GRAPH_CELL_H),
            });
        }
    }
    out.sort_by(|a, b| a.layer.cmp(&b.layer).then(a.slot.cmp(&b.slot)));
    out
}

/// Content size of layout in cells.
#[must_use]
pub fn layout_content_size(layout: &[DepLayoutNode]) -> (u16, u16) {
    let w = layout
        .iter()
        .map(|n| n.x.saturating_add(DEP_GRAPH_CELL_W))
        .max()
        .unwrap_or(0);
    let h = layout
        .iter()
        .map(|n| n.y.saturating_add(DEP_GRAPH_CELL_H))
        .max()
        .unwrap_or(0);
    (w, h)
}

/// Filter nodes by query (label/id/group/kind).
#[must_use]
pub fn filter_dep_nodes<'a>(
    nodes: &'a [DepNode<'a>],
    query: &str,
) -> Vec<&'a DepNode<'a>> {
    let q = query.trim().to_ascii_lowercase();
    if q.is_empty() {
        return nodes.iter().collect();
    }
    nodes
        .iter()
        .filter(|n| {
            let hay = format!(
                "{} {} {} {}",
                n.id,
                n.label,
                n.kind.id(),
                n.group.unwrap_or("")
            )
            .to_ascii_lowercase();
            hay.contains(&q)
        })
        .collect()
}

/// Edges where both ends pass the filtered id set.
#[must_use]
pub fn filter_dep_edges<'a>(
    edges: &'a [DepEdge<'a>],
    keep_ids: &BTreeSet<&str>,
) -> Vec<&'a DepEdge<'a>> {
    edges
        .iter()
        .filter(|e| keep_ids.contains(e.from) && keep_ids.contains(e.to))
        .collect()
}

/// Project graph as TreeTable rows (dependency tree from roots, DFS).
///
/// `cell_bufs` must outlive returned rows — host stores strings.
pub fn project_dep_tree_rows(
    nodes: &[DepNode<'_>],
    edges: &[DepEdge<'_>],
    cell_bufs: &mut Vec<[String; 4]>,
) -> Vec<(String, u16, bool, bool, Option<String>, usize)> {
    // returns (id, depth, branch, expanded, parent, cell_buf_index)
    let id_set: BTreeSet<&str> = nodes.iter().map(|n| n.id).collect();
    let mut children: BTreeMap<&str, Vec<&str>> = BTreeMap::new();
    let mut indeg: BTreeMap<&str, usize> = BTreeMap::new();
    for n in nodes {
        indeg.entry(n.id).or_insert(0);
        children.entry(n.id).or_default();
    }
    for e in edges {
        if !id_set.contains(e.from) || !id_set.contains(e.to) {
            continue;
        }
        // tree: from depends on to → child edge from → to as nested under from? 
        // Convention: edge A→B means A depends on B, so B is child of A in tree view
        // (expand A shows deps).
        children.entry(e.from).or_default().push(e.to);
        *indeg.entry(e.to).or_insert(0) += 1;
    }
    for v in children.values_mut() {
        v.sort_unstable();
        v.dedup();
    }
    let mut roots: Vec<&str> = indeg
        .iter()
        .filter(|(_, d)| **d == 0)
        .map(|(k, _)| *k)
        .collect();
    roots.sort_unstable();
    if roots.is_empty() {
        roots = nodes.iter().map(|n| n.id).collect();
        roots.sort_unstable();
    }

    let node_by: BTreeMap<&str, &DepNode<'_>> = nodes.iter().map(|n| (n.id, n)).collect();
    cell_bufs.clear();
    let mut meta = Vec::new();
    let mut visited = BTreeSet::new();

    fn walk(
        id: &str,
        depth: u16,
        parent: Option<&str>,
        children: &BTreeMap<&str, Vec<&str>>,
        node_by: &BTreeMap<&str, &DepNode<'_>>,
        visited: &mut BTreeSet<String>,
        cell_bufs: &mut Vec<[String; 4]>,
        meta: &mut Vec<(String, u16, bool, bool, Option<String>, usize)>,
    ) {
        if !visited.insert(id.to_string()) {
            return;
        }
        let n = match node_by.get(id) {
            Some(n) => *n,
            None => return,
        };
        let kids = children.get(id).cloned().unwrap_or_default();
        let branch = !kids.is_empty();
        let idx = cell_bufs.len();
        cell_bufs.push([
            n.label.to_string(),
            n.kind.id().to_string(),
            n.status.id().to_string(),
            n.detail.unwrap_or("").to_string(),
        ]);
        meta.push((
            id.to_string(),
            depth,
            branch,
            branch, // expanded default for tree fallback
            parent.map(str::to_string),
            idx,
        ));
        for c in kids {
            walk(
                c,
                depth.saturating_add(1),
                Some(id),
                children,
                node_by,
                visited,
                cell_bufs,
                meta,
            );
        }
    }

    for r in roots {
        walk(
            r,
            0,
            None,
            &children,
            &node_by,
            &mut visited,
            cell_bufs,
            &mut meta,
        );
    }
    // orphans not reached
    for n in nodes {
        if !visited.contains(n.id) {
            walk(
                n.id,
                0,
                None,
                &children,
                &node_by,
                &mut visited,
                cell_bufs,
                &mut meta,
            );
        }
    }
    meta
}

/// Column model for tree/list fallback.
#[must_use]
pub fn dependency_tree_column_model() -> ColumnModel<&'static str> {
    ColumnModel::new(vec![
        DataColumn::new("name", "Name", DataColumnWidth::Min(12)).priority(100),
        DataColumn::new("kind", "Kind", DataColumnWidth::Fixed(8)).priority(80),
        DataColumn::new("status", "Status", DataColumnWidth::Fixed(8)).priority(70),
        DataColumn::new("detail", "Detail", DataColumnWidth::Min(8)).priority(40),
    ])
}

/// ObjectInspector fields for a node.
#[must_use]
pub fn dep_node_to_inspector_fields<'a>(node: &'a DepNode<'a>) -> Vec<InspectorField<'a>> {
    let mut f = vec![
        InspectorField::new("id", node.id).kind(InspectKind::String),
        InspectorField::new("label", node.label).kind(InspectKind::String),
        InspectorField::new("kind", node.kind.id()).kind(InspectKind::String),
        InspectorField::new("status", node.status.id()).kind(InspectKind::String),
    ];
    if let Some(g) = node.group {
        f.push(InspectorField::new("group", g).kind(InspectKind::String));
    }
    if let Some(d) = node.detail {
        f.push(InspectorField::new("detail", d).kind(InspectKind::String));
    }
    f
}

// ── Outcomes ────────────────────────────────────────────────────────────────

/// Typed outcomes — host owns resolution / open package.
#[derive(Debug, Clone, PartialEq, Eq)]
#[non_exhaustive]
pub enum DependencyGraphOutcome {
    /// No change.
    Ignored,
    /// Node selection.
    SelectionChanged {
        /// Node id.
        id: String,
    },
    /// Open details (inspector).
    DetailsRequested {
        /// Node id.
        id: String,
    },
    /// Edge focus (from→to).
    EdgeSelected {
        /// From.
        from: String,
        /// To.
        to: String,
    },
    /// View mode changed.
    ViewChanged(DependencyGraphView),
    /// Filter changed.
    FilterChanged(String),
    /// Pan changed.
    Panned {
        /// X offset.
        x: u16,
        /// Y offset.
        y: u16,
    },
    /// Cancel filter.
    Cancelled,
    /// Tree expand in tree view (host optional).
    ExpandToggled {
        /// Id.
        id: String,
    },
}

// ── State ───────────────────────────────────────────────────────────────────

/// Dependency graph state.
#[derive(Debug, Clone, PartialEq)]
pub struct DependencyGraphState {
    /// Preferred view (may auto-fallback).
    pub preferred_view: DependencyGraphView,
    /// Force tree regardless of size.
    pub force_tree: bool,
    /// Selected node id.
    selected: Option<String>,
    /// Horizontal pan offset for graph view (content scroll).
    pub pan_x: u16,
    /// Vertical pan offset for graph view (content scroll).
    pub pan_y: u16,
    /// Filter.
    pub filter: Option<String>,
    /// Cursor index in list/tree projection.
    pub cursor: usize,
    /// ASCII connectors.
    pub ascii: bool,
    /// Last effective view (set on paint).
    pub effective_view: DependencyGraphView,
    /// Unreadable reason if any.
    pub unreadable: Option<GraphUnreadableReason>,
    /// Node hit regions (id, rect).
    node_regions: Vec<(String, Rect)>,
    accepts_input: bool,
}

impl Default for DependencyGraphState {
    fn default() -> Self {
        Self::new()
    }
}

impl DependencyGraphState {
    /// Fresh.
    #[must_use]
    pub fn new() -> Self {
        Self {
            preferred_view: DependencyGraphView::Graph,
            force_tree: false,
            selected: None,
            pan_x: 0,
            pan_y: 0,
            filter: None,
            cursor: 0,
            ascii: false,
            effective_view: DependencyGraphView::Graph,
            unreadable: None,
            node_regions: Vec::new(),
            accepts_input: true,
        }
    }

    /// With selection.
    #[must_use]
    pub fn with_selected(id: impl Into<String>) -> Self {
        let mut s = Self::new();
        s.selected = Some(id.into());
        s
    }

    /// Host input gate.
    pub fn set_accepts_input(&mut self, on: bool) {
        self.accepts_input = on;
    }

    /// Accepts input.
    #[must_use]
    pub const fn accepts_input(&self) -> bool {
        self.accepts_input
    }

    /// Selected.
    #[must_use]
    pub fn selected(&self) -> Option<&str> {
        self.selected.as_deref()
    }

    /// Select.
    pub fn select(&mut self, id: Option<String>) {
        self.selected = id;
    }

    /// Keys.
    pub fn handle_key(
        &mut self,
        nodes: &[DepNode<'_>],
        edges: &[DepEdge<'_>],
        key: KeyEvent,
    ) -> DependencyGraphOutcome {
        if !self.accepts_input || key.kind != KeyEventKind::Press {
            return DependencyGraphOutcome::Ignored;
        }

        let filtered: Vec<&DepNode<'_>> =
            filter_dep_nodes(nodes, self.filter.as_deref().unwrap_or(""));
        let keep: BTreeSet<&str> = filtered.iter().map(|n| n.id).collect();
        let _edges = filter_dep_edges(edges, &keep);

        // Filter typing
        if let Some(q) = self.filter.as_mut()
            && key.modifiers.is_empty()
        {
            match key.code {
                KeyCode::Esc => {
                    self.filter = None;
                    return DependencyGraphOutcome::Cancelled;
                }
                KeyCode::Backspace => {
                    q.pop();
                    if q.is_empty() {
                        self.filter = None;
                    }
                    return DependencyGraphOutcome::FilterChanged(
                        self.filter.clone().unwrap_or_default(),
                    );
                }
                KeyCode::Char(c)
                    if !c.is_control()
                        && !matches!(c, 'j' | 'k' | 'h' | 'l' | 'J' | 'K' | 'H' | 'L') =>
                {
                    q.push(c);
                    return DependencyGraphOutcome::FilterChanged(q.clone());
                }
                _ => {}
            }
        }

        if key.modifiers.is_empty() && matches!(key.code, KeyCode::Char('/')) {
            self.filter = Some(String::new());
            return DependencyGraphOutcome::FilterChanged(String::new());
        }

        if key.modifiers.contains(KeyModifiers::CONTROL)
            && matches!(key.code, KeyCode::Char('v' | 'V'))
        {
            self.preferred_view = self.preferred_view.cycle();
            return DependencyGraphOutcome::ViewChanged(self.preferred_view);
        }

        if filtered.is_empty() {
            return DependencyGraphOutcome::Ignored;
        }

        // Graph pan
        if matches!(self.effective_view, DependencyGraphView::Graph) {
            match key.code {
                KeyCode::Left | KeyCode::Char('h')
                    if key.modifiers.contains(KeyModifiers::SHIFT)
                        || matches!(self.effective_view, DependencyGraphView::Graph)
                            && key.modifiers.contains(KeyModifiers::CONTROL) =>
                {
                    self.pan_x = self.pan_x.saturating_sub(4);
                    return DependencyGraphOutcome::Panned {
                        x: self.pan_x,
                        y: self.pan_y,
                    };
                }
                KeyCode::Right | KeyCode::Char('l')
                    if key.modifiers.contains(KeyModifiers::CONTROL) =>
                {
                    self.pan_x = self.pan_x.saturating_add(4);
                    return DependencyGraphOutcome::Panned {
                        x: self.pan_x,
                        y: self.pan_y,
                    };
                }
                KeyCode::Up | KeyCode::Char('k') if key.modifiers.contains(KeyModifiers::CONTROL) => {
                    self.pan_y = self.pan_y.saturating_sub(2);
                    return DependencyGraphOutcome::Panned {
                        x: self.pan_x,
                        y: self.pan_y,
                    };
                }
                KeyCode::Down | KeyCode::Char('j')
                    if key.modifiers.contains(KeyModifiers::CONTROL) =>
                {
                    self.pan_y = self.pan_y.saturating_add(2);
                    return DependencyGraphOutcome::Panned {
                        x: self.pan_x,
                        y: self.pan_y,
                    };
                }
                KeyCode::Char('[') => {
                    self.pan_x = self.pan_x.saturating_sub(4);
                    return DependencyGraphOutcome::Panned {
                        x: self.pan_x,
                        y: self.pan_y,
                    };
                }
                KeyCode::Char(']') => {
                    self.pan_x = self.pan_x.saturating_add(4);
                    return DependencyGraphOutcome::Panned {
                        x: self.pan_x,
                        y: self.pan_y,
                    };
                }
                _ => {}
            }
        }

        // Selection nav along filtered nodes (stable order)
        match key.code {
            KeyCode::Down | KeyCode::Char('j') if key.modifiers.is_empty() => {
                self.cursor = (self.cursor + 1).min(filtered.len() - 1);
                let id = filtered[self.cursor].id.to_string();
                self.selected = Some(id.clone());
                DependencyGraphOutcome::SelectionChanged { id }
            }
            KeyCode::Up | KeyCode::Char('k') if key.modifiers.is_empty() => {
                self.cursor = self.cursor.saturating_sub(1);
                let id = filtered[self.cursor].id.to_string();
                self.selected = Some(id.clone());
                DependencyGraphOutcome::SelectionChanged { id }
            }
            KeyCode::Home => {
                self.cursor = 0;
                let id = filtered[0].id.to_string();
                self.selected = Some(id.clone());
                DependencyGraphOutcome::SelectionChanged { id }
            }
            KeyCode::End => {
                self.cursor = filtered.len() - 1;
                let id = filtered[self.cursor].id.to_string();
                self.selected = Some(id.clone());
                DependencyGraphOutcome::SelectionChanged { id }
            }
            KeyCode::Enter | KeyCode::Char('i') if key.modifiers.is_empty() => {
                if let Some(id) = self.selected.clone().or_else(|| {
                    filtered.get(self.cursor).map(|n| n.id.to_string())
                }) {
                    DependencyGraphOutcome::DetailsRequested { id }
                } else {
                    DependencyGraphOutcome::Ignored
                }
            }
            KeyCode::Char('e') if key.modifiers.is_empty() => {
                // next outgoing edge from selection
                if let Some(sel) = self.selected.as_deref() {
                    if let Some(e) = edges.iter().find(|e| e.from == sel) {
                        return DependencyGraphOutcome::EdgeSelected {
                            from: e.from.to_string(),
                            to: e.to.to_string(),
                        };
                    }
                }
                DependencyGraphOutcome::Ignored
            }
            _ => DependencyGraphOutcome::Ignored,
        }
    }

    /// Mouse.
    pub fn handle_mouse(
        &mut self,
        _nodes: &[DepNode<'_>],
        event: MouseEvent,
    ) -> DependencyGraphOutcome {
        if !self.accepts_input {
            return DependencyGraphOutcome::Ignored;
        }
        match event.kind {
            MouseEventKind::ScrollDown if event.modifiers.contains(KeyModifiers::SHIFT) => {
                self.pan_x = self.pan_x.saturating_add(4);
                DependencyGraphOutcome::Panned {
                    x: self.pan_x,
                    y: self.pan_y,
                }
            }
            MouseEventKind::ScrollUp if event.modifiers.contains(KeyModifiers::SHIFT) => {
                self.pan_x = self.pan_x.saturating_sub(4);
                DependencyGraphOutcome::Panned {
                    x: self.pan_x,
                    y: self.pan_y,
                }
            }
            MouseEventKind::ScrollDown => {
                self.pan_y = self.pan_y.saturating_add(2);
                DependencyGraphOutcome::Panned {
                    x: self.pan_x,
                    y: self.pan_y,
                }
            }
            MouseEventKind::ScrollUp => {
                self.pan_y = self.pan_y.saturating_sub(2);
                DependencyGraphOutcome::Panned {
                    x: self.pan_x,
                    y: self.pan_y,
                }
            }
            MouseEventKind::Down(MouseButton::Left) => {
                let hit = self
                    .node_regions
                    .iter()
                    .find(|(_, r)| r.contains(event.position))
                    .map(|(id, _)| id.clone());
                if let Some(id) = hit {
                    self.selected = Some(id.clone());
                    DependencyGraphOutcome::SelectionChanged { id }
                } else {
                    DependencyGraphOutcome::Ignored
                }
            }
            _ => DependencyGraphOutcome::Ignored,
        }
    }
}

// ── Widget ──────────────────────────────────────────────────────────────────

/// Dependency graph paint.
#[derive(Debug, Clone, Copy)]
pub struct DependencyGraph<'a> {
    nodes: &'a [DepNode<'a>],
    edges: &'a [DepEdge<'a>],
    system: &'a DesignSystem,
    focused: bool,
    title: Option<&'a str>,
    ascii: bool,
}

impl<'a> DependencyGraph<'a> {
    /// Nodes + edges + system.
    #[must_use]
    pub const fn new(
        nodes: &'a [DepNode<'a>],
        edges: &'a [DepEdge<'a>],
        system: &'a DesignSystem,
    ) -> Self {
        Self {
            nodes,
            edges,
            system,
            focused: true,
            title: None,
            ascii: false,
        }
    }

    /// Title.
    #[must_use]
    pub const fn title(mut self, t: &'a str) -> Self {
        self.title = Some(t);
        self
    }

    /// Focus.
    #[must_use]
    pub const fn focused(mut self, on: bool) -> Self {
        self.focused = on;
        self
    }

    /// ASCII.
    #[must_use]
    pub const fn ascii(mut self, on: bool) -> Self {
        self.ascii = on;
        self
    }

    /// Paint.
    pub fn render(&self, area: Rect, buffer: &mut Buffer, state: &mut DependencyGraphState) {
        if area.is_empty() {
            return;
        }
        let ascii = self.ascii || state.ascii;
        state.node_regions.clear();

        let filtered = filter_dep_nodes(self.nodes, state.filter.as_deref().unwrap_or(""));
        let keep: BTreeSet<&str> = filtered.iter().map(|n| n.id).collect();
        let f_edges = filter_dep_edges(self.edges, &keep);
        // rebuild owned slices for layout
        let f_nodes: Vec<DepNode<'a>> = filtered.iter().map(|n| (*n).clone()).collect();
        let f_edge_owned: Vec<DepEdge<'a>> = f_edges.iter().map(|e| (*e).clone()).collect();

        let (view, reason) = choose_dependency_view(
            state.preferred_view,
            f_nodes.len(),
            area.width,
            state.force_tree,
        );
        state.effective_view = view;
        state.unreadable = reason;

        let mut y = area.y;
        let mut h = area.height;

        if h > 0 {
            let title = self.title.unwrap_or("deps");
            let note = reason.map(|r| r.id()).unwrap_or("-");
            let line = format!(
                "{title} · {} · {}n/{}e · pan {},{} · {note}",
                view.id(),
                f_nodes.len(),
                f_edge_owned.len(),
                state.pan_x,
                state.pan_y
            );
            buffer.set_stringn(
                area.x,
                y,
                take_display_cols(&line, usize::from(area.width)),
                usize::from(area.width),
                if self.focused {
                    self.system.style(Role::TextStrong)
                } else {
                    self.system.style(Role::TextMuted)
                },
            );
            y = y.saturating_add(1);
            h = h.saturating_sub(1);
        }

        if state.filter.is_some() && h > 0 {
            let q = state.filter.as_deref().unwrap_or("");
            buffer.set_stringn(
                area.x,
                y,
                take_display_cols(&format!("/{q}_"), usize::from(area.width)),
                usize::from(area.width),
                self.system.style(Role::Accent),
            );
            y = y.saturating_add(1);
            h = h.saturating_sub(1);
        }

        if h == 0 {
            return;
        }
        let body = Rect {
            x: area.x,
            y,
            width: area.width,
            height: h,
        };

        match view {
            DependencyGraphView::Graph => {
                paint_graph(
                    &f_nodes,
                    &f_edge_owned,
                    body,
                    buffer,
                    self.system,
                    state,
                    ascii,
                    self.focused,
                );
            }
            DependencyGraphView::Tree | DependencyGraphView::List => {
                paint_list_or_tree(
                    &f_nodes,
                    &f_edge_owned,
                    body,
                    buffer,
                    self.system,
                    state,
                    ascii,
                    self.focused,
                    matches!(view, DependencyGraphView::Tree),
                );
            }
        }
    }
}

fn paint_graph(
    nodes: &[DepNode<'_>],
    edges: &[DepEdge<'_>],
    area: Rect,
    buffer: &mut Buffer,
    system: &DesignSystem,
    state: &mut DependencyGraphState,
    ascii: bool,
    focused: bool,
) {
    if nodes.is_empty() {
        buffer.set_stringn(
            area.x,
            area.y,
            take_display_cols("(no nodes)", usize::from(area.width)),
            usize::from(area.width),
            system.style(Role::TextMuted),
        );
        return;
    }
    let layout = layout_dependency_layers(nodes, edges);
    let by_id: BTreeMap<&str, &DepLayoutNode> =
        layout.iter().map(|n| (n.id.as_str(), n)).collect();
    let node_by: BTreeMap<&str, &DepNode<'_>> = nodes.iter().map(|n| (n.id, n)).collect();

    // Draw edges first (under nodes)
    let h_line = if ascii { "-" } else { "─" };
    let v_line = if ascii { "|" } else { "│" };
    let corner = if ascii { "+" } else { "┼" };

    for e in edges {
        let Some(a) = by_id.get(e.from) else {
            continue;
        };
        let Some(b) = by_id.get(e.to) else {
            continue;
        };
        // center of cells relative to pan
        let ax = a.x.saturating_add(DEP_GRAPH_CELL_W / 2).saturating_sub(state.pan_x);
        let ay = a.y.saturating_add(1).saturating_sub(state.pan_y);
        let bx = b.x.saturating_add(DEP_GRAPH_CELL_W / 2).saturating_sub(state.pan_x);
        let by = b.y.saturating_add(1).saturating_sub(state.pan_y);
        // simple L connector
        let x0 = area.x.saturating_add(ax.min(bx));
        let x1 = area.x.saturating_add(ax.max(bx));
        let y0 = area.y.saturating_add(ay.min(by));
        let y1 = area.y.saturating_add(ay.max(by));
        let style = system.style(Role::TextMuted);
        // vertical then horizontal toward target
        let mid_y = area.y.saturating_add(ay);
        let mid_x = area.x.saturating_add(bx);
        if mid_y >= area.y && mid_y < area.bottom() {
            let left = x0.max(area.x);
            let right = x1.min(area.right().saturating_sub(1));
            for x in left..=right {
                put_sym(buffer, x, mid_y, h_line, style);
            }
        }
        let top = y0.max(area.y);
        let bot = y1.min(area.bottom().saturating_sub(1));
        if mid_x >= area.x && mid_x < area.right() {
            for y in top..=bot {
                put_sym(buffer, mid_x, y, v_line, style);
            }
        }
        if mid_x >= area.x && mid_x < area.right() && mid_y >= area.y && mid_y < area.bottom() {
            put_sym(buffer, mid_x, mid_y, corner, style);
        }
        let _ = e.kind.arrow(ascii);
    }

    // Draw nodes
    for ln in &layout {
        let Some(n) = node_by.get(ln.id.as_str()) else {
            continue;
        };
        let x = area.x.saturating_add(ln.x.saturating_sub(state.pan_x));
        let y = area.y.saturating_add(ln.y.saturating_sub(state.pan_y));
        if x >= area.right() || y >= area.bottom() {
            continue;
        }
        let w = DEP_GRAPH_CELL_W
            .saturating_sub(1)
            .min(area.right().saturating_sub(x));
        let h = 2u16.min(area.bottom().saturating_sub(y));
        if w == 0 || h == 0 {
            continue;
        }
        let selected = state.selected.as_deref() == Some(n.id);
        let letter = if ascii {
            n.status.letter_ascii()
        } else {
            n.status.letter()
        };
        let mark = if selected {
            if ascii { "*" } else { "›" }
        } else {
            " "
        };
        let label = format!(
            "{mark}{}{} {}",
            n.kind.glyph(ascii),
            letter,
            take_display_cols(n.label, usize::from(w.saturating_sub(4)))
        );
        let style = if selected && focused {
            system.style(Role::Focus)
        } else {
            system.style(n.status.role())
        };
        buffer.set_stringn(
            x,
            y,
            take_display_cols(&label, usize::from(w)),
            usize::from(w),
            style,
        );
        if h > 1 {
            if let Some(d) = n.detail {
                buffer.set_stringn(
                    x,
                    y.saturating_add(1),
                    take_display_cols(d, usize::from(w)),
                    usize::from(w),
                    system.style(Role::TextMuted),
                );
            }
        }
        state.node_regions.push((
            n.id.to_string(),
            Rect {
                x,
                y,
                width: w,
                height: h,
            },
        ));
    }
}

fn put_sym(
    buffer: &mut Buffer,
    x: u16,
    y: u16,
    sym: &str,
    style: ratatui_core::style::Style,
) {
    if let Some(cell) = buffer.cell_mut((x, y)) {
        // don't overwrite node labels heavily — only empty-ish
        let cur = cell.symbol();
        if cur == " " || cur.is_empty() || cur == "─" || cur == "-" || cur == "│" || cur == "|" {
            cell.set_symbol(sym);
            cell.set_style(style);
        }
    }
}

fn paint_list_or_tree(
    nodes: &[DepNode<'_>],
    edges: &[DepEdge<'_>],
    area: Rect,
    buffer: &mut Buffer,
    system: &DesignSystem,
    state: &mut DependencyGraphState,
    ascii: bool,
    focused: bool,
    tree: bool,
) {
    if nodes.is_empty() {
        buffer.set_stringn(
            area.x,
            area.y,
            take_display_cols("(no nodes)", usize::from(area.width)),
            usize::from(area.width),
            system.style(Role::TextMuted),
        );
        return;
    }

    let mut cell_bufs = Vec::new();
    let meta = if tree {
        project_dep_tree_rows(nodes, edges, &mut cell_bufs)
    } else {
        // flat list
        cell_bufs.clear();
        nodes
            .iter()
            .enumerate()
            .map(|(i, n)| {
                cell_bufs.push([
                    n.label.to_string(),
                    n.kind.id().to_string(),
                    n.status.id().to_string(),
                    n.detail.unwrap_or("").to_string(),
                ]);
                (n.id.to_string(), 0u16, false, false, None, i)
            })
            .collect()
    };

    // header
    let mut y = area.y;
    buffer.set_stringn(
        area.x,
        y,
        take_display_cols(
            if tree {
                "NAME                 KIND     STATUS"
            } else {
                "NAME                 KIND     STATUS   (list)"
            },
            usize::from(area.width),
        ),
        usize::from(area.width),
        system.style(Role::TextMuted),
    );
    y = y.saturating_add(1);

    let start = state.cursor.saturating_sub(usize::from(area.height.saturating_sub(2)) / 2);
    for (i, m) in meta.iter().enumerate().skip(start) {
        if y >= area.bottom() {
            break;
        }
        let cells = &cell_bufs[m.5];
        let selected = state.selected.as_deref() == Some(m.0.as_str()) || i == state.cursor;
        let indent = if tree {
            "  ".repeat(usize::from(m.1))
        } else {
            String::new()
        };
        let disc = if tree && m.2 {
            if m.3 {
                if ascii { "v " } else { "▾ " }
            } else if ascii {
                "> "
            } else {
                "▸ "
            }
        } else {
            "  "
        };
        let mark = if selected {
            if ascii { ">" } else { "›" }
        } else {
            " "
        };
        let line = format!(
            "{mark}{indent}{disc}{:<16} {:<8} {}",
            take_display_cols(&cells[0], 16),
            cells[1],
            cells[2]
        );
        buffer.set_stringn(
            area.x,
            y,
            take_display_cols(&line, usize::from(area.width)),
            usize::from(area.width),
            if selected && focused {
                system.style(Role::Focus)
            } else {
                system.style(Role::Text)
            },
        );
        state.node_regions.push((
            m.0.clone(),
            Rect {
                x: area.x,
                y,
                width: area.width,
                height: 1,
            },
        ));
        y = y.saturating_add(1);
    }
}

// ── Bench ───────────────────────────────────────────────────────────────────

/// Moderate real-world graph sizes.
pub mod bench {
    /// Nodes (moderate service map).
    pub const NODE_COUNT: usize = 80;
    /// Edges.
    pub const EDGE_COUNT: usize = 160;
    /// Paint frames.
    pub const PAINT_FRAMES: u32 = 40;
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::style::DesignSystem;

    fn sample() -> (Vec<DepNode<'static>>, Vec<DepEdge<'static>>) {
        let nodes = vec![
            DepNode::new("app", "app").kind(DepNodeKind::Package).detail("0.1.0"),
            DepNode::new("termrock", "termrock")
                .kind(DepNodeKind::Package)
                .detail("0.11"),
            DepNode::new("ratatui", "ratatui")
                .kind(DepNodeKind::Package)
                .status(DepNodeStatus::Ok),
            DepNode::new("serde", "serde").kind(DepNodeKind::Package),
            DepNode::new("api", "api-svc")
                .kind(DepNodeKind::Service)
                .group("runtime"),
            DepNode::new("db", "postgres")
                .kind(DepNodeKind::Service)
                .status(DepNodeStatus::Warning),
            DepNode::new("missing", "lost-crate")
                .kind(DepNodeKind::Package)
                .status(DepNodeStatus::Missing),
        ];
        let edges = vec![
            DepEdge::new("app", "termrock"),
            DepEdge::new("app", "serde"),
            DepEdge::new("termrock", "ratatui"),
            DepEdge::new("api", "db").kind(DepEdgeKind::Calls),
            DepEdge::new("app", "api").kind(DepEdgeKind::DependsOn),
            DepEdge::new("app", "missing"),
        ];
        (nodes, edges)
    }

    #[test]
    fn layered_layout_deterministic() {
        let (nodes, edges) = sample();
        let a = layout_dependency_layers(&nodes, &edges);
        let b = layout_dependency_layers(&nodes, &edges);
        assert_eq!(a, b);
        assert!(!a.is_empty());
        assert!(a.iter().any(|n| n.id == "app"));
    }

    #[test]
    fn choose_view_narrow_and_large() {
        let (v, r) = choose_dependency_view(DependencyGraphView::Graph, 10, 30, false);
        assert_eq!(v, DependencyGraphView::Tree);
        assert_eq!(r, Some(GraphUnreadableReason::Narrow));
        let (v, r) = choose_dependency_view(
            DependencyGraphView::Graph,
            DEP_GRAPH_AUTO_TREE_NODES + 1,
            80,
            false,
        );
        assert_eq!(v, DependencyGraphView::Tree);
        assert_eq!(r, Some(GraphUnreadableReason::TooManyNodes));
        let (v, _) = choose_dependency_view(DependencyGraphView::Graph, 5, 80, false);
        assert_eq!(v, DependencyGraphView::Graph);
    }

    #[test]
    fn filter_nodes() {
        let (nodes, _) = sample();
        let v = filter_dep_nodes(&nodes, "serde");
        assert_eq!(v.len(), 1);
    }

    #[test]
    fn tree_projection() {
        let (nodes, edges) = sample();
        let mut bufs = Vec::new();
        let meta = project_dep_tree_rows(&nodes, &edges, &mut bufs);
        assert!(!meta.is_empty());
        assert_eq!(bufs.len(), meta.len());
    }

    #[test]
    fn nav_and_view_cycle() {
        let (nodes, edges) = sample();
        let mut state = DependencyGraphState::with_selected("app");
        state.effective_view = DependencyGraphView::List;
        assert!(matches!(
            state.handle_key(
                &nodes,
                &edges,
                KeyEvent::new(KeyCode::Down, KeyModifiers::NONE)
            ),
            DependencyGraphOutcome::SelectionChanged { .. }
        ));
        assert!(matches!(
            state.handle_key(
                &nodes,
                &edges,
                KeyEvent::new(KeyCode::Char('v'), KeyModifiers::CONTROL)
            ),
            DependencyGraphOutcome::ViewChanged(_)
        ));
    }

    #[test]
    fn paint_graph_and_tree() {
        let system = DesignSystem::default();
        let (nodes, edges) = sample();
        let mut state = DependencyGraphState::with_selected("termrock");
        let area = Rect::new(0, 0, 72, 16);
        let mut buf = Buffer::empty(area);
        DependencyGraph::new(&nodes, &edges, &system)
            .title("crates")
            .render(area, &mut buf, &mut state);
        assert_eq!(state.effective_view, DependencyGraphView::Graph);
        let text: String = buf
            .content()
            .iter()
            .map(|c| c.symbol().to_string())
            .collect();
        assert!(
            text.contains("termrock") || text.contains("crates") || text.contains("app"),
            "{text}"
        );

        state.force_tree = true;
        DependencyGraph::new(&nodes, &edges, &system).render(area, &mut buf, &mut state);
        assert_eq!(state.effective_view, DependencyGraphView::Tree);
    }

    #[test]
    fn inspector_bridge() {
        let (nodes, _) = sample();
        let f = dep_node_to_inspector_fields(&nodes[0]);
        assert!(f.iter().any(|x| x.key == "id"));
    }

    #[test]
    fn accepts_input_gate() {
        let (nodes, edges) = sample();
        let mut state = DependencyGraphState::new();
        state.set_accepts_input(false);
        assert!(matches!(
            state.handle_key(
                &nodes,
                &edges,
                KeyEvent::new(KeyCode::Down, KeyModifiers::NONE)
            ),
            DependencyGraphOutcome::Ignored
        ));
    }

    #[test]
    fn moderate_graph_bench() {
        let system = DesignSystem::default();
        let ids: Vec<String> = (0..bench::NODE_COUNT).map(|i| format!("n{i}")).collect();
        let labels: Vec<String> = (0..bench::NODE_COUNT).map(|i| format!("node-{i}")).collect();
        let nodes: Vec<DepNode<'_>> = (0..bench::NODE_COUNT)
            .map(|i| {
                DepNode::new(&ids[i], &labels[i]).kind(if i % 3 == 0 {
                    DepNodeKind::Service
                } else {
                    DepNodeKind::Package
                })
            })
            .collect();
        let edges: Vec<DepEdge<'_>> = (0..bench::EDGE_COUNT)
            .map(|i| {
                let a = i % bench::NODE_COUNT;
                let b = (i * 7 + 1) % bench::NODE_COUNT;
                DepEdge::new(&ids[a], &ids[b])
            })
            .collect();
        let layout = layout_dependency_layers(&nodes, &edges);
        assert_eq!(layout.len(), bench::NODE_COUNT);
        let mut state = DependencyGraphState::new();
        // large → auto tree
        let area = Rect::new(0, 0, 100, 30);
        let mut buf = Buffer::empty(area);
        for _ in 0..8 {
            DependencyGraph::new(&nodes, &edges, &system).render(area, &mut buf, &mut state);
            let _ = state.handle_key(
                &nodes,
                &edges,
                KeyEvent::new(KeyCode::Down, KeyModifiers::NONE),
            );
        }
        assert_eq!(state.effective_view, DependencyGraphView::Tree);
    }

    #[test]
    fn never_resolves_packages() {
        let src = include_str!("dependency_graph.rs");
        let body = src.split("#[cfg(test)]").next().unwrap_or(src);
        for forbidden in ["cargo metadata", "std::process::Command", "reqwest::"] {
            assert!(!body.contains(forbidden), "must not contain {forbidden}");
        }
    }
}
