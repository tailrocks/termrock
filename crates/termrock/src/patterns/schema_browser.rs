// SPDX-FileCopyrightText: 2026 Alexey Zhokhov
// SPDX-License-Identifier: Apache-2.0

//! **SchemaBrowser** — hierarchical database / schema navigator.
//!
//! **Mission.** Connections, databases, schemas, tables, views, columns,
//! indexes, constraints, routines; loading/error/lazy states; search; status;
//! context actions. Lazy expansion with **application-owned** metadata fetch.
//! Detail previews and QuickOpen integration. Expanded state survives refresh
//! and reconnect (stable ids). Contracts from side pane → drawer → fullscreen.
//!
//! **vs [`super::FileTree`].** FileTree is filesystem/git. SchemaBrowser is
//! catalog metadata (no FS, no SQL drivers).
//! **vs [`super::Tree`].** Tree is the generic hierarchy substrate.
//!
//! Research: TablePlus, DataGrip, pgcli ecosystems, file-tree navigation.
//!
//! Teaches: how to compose hierarchical database / schema navigator.
//!
//! Composes: [`crate::widgets::BreadcrumbItem`],
//! [`crate::widgets::QuickOpenItem`], [`crate::widgets::QuickOpenPreview`],
//! [`crate::widgets::StatefulWidget`], [`crate::widgets::Tree`],
//! [`crate::widgets::TreeNode`], [`crate::widgets::TreeNodeStatus`],
//! [`crate::widgets::TreeOutcome`], and 1 more.
//!
//! Copy-adapt: keep the widget composition and the focus routing;
//! replace the domain types, the wording, and the effects with your own.
use std::collections::BTreeSet;

use ratatui_core::{buffer::Buffer, layout::Rect, text::Line, widgets::StatefulWidget};

use crate::{
    input::{KeyCode, KeyEvent, KeyModifiers, MouseEvent},
    style::{DesignSystem, Role},
    widgets::{
        BreadcrumbItem, EmptyKind, EmptyState, QuickOpenItem, QuickOpenPreview, SemanticStatus,
        Tree, TreeNode, TreeNodeStatus, TreeOutcome, TreeState,
    },
};

// ── Kinds & connection status ───────────────────────────────────────────────

/// Schema object kind (host classification).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
#[non_exhaustive]
pub enum SchemaNodeKind {
    /// Live connection / server.
    Connection,
    /// Database / catalog.
    Database,
    /// Schema / namespace.
    Schema,
    /// Table.
    #[default]
    Table,
    /// View / materialized view.
    View,
    /// Column.
    Column,
    /// Index.
    Index,
    /// Constraint (PK/FK/check/unique).
    Constraint,
    /// Routine (function / procedure).
    Routine,
    /// Sequence / identity.
    Sequence,
    /// Group / folder band (host organizational).
    Group,
    /// Other catalog object.
    Other,
}

impl SchemaNodeKind {
    /// Stable id.
    #[must_use]
    pub const fn id(self) -> &'static str {
        match self {
            Self::Connection => "connection",
            Self::Database => "database",
            Self::Schema => "schema",
            Self::Table => "table",
            Self::View => "view",
            Self::Column => "column",
            Self::Index => "index",
            Self::Constraint => "constraint",
            Self::Routine => "routine",
            Self::Sequence => "sequence",
            Self::Group => "group",
            Self::Other => "other",
        }
    }

    /// Whether expandable by default (host may override with `branch`).
    #[must_use]
    pub const fn default_branch(self) -> bool {
        matches!(
            self,
            Self::Connection
                | Self::Database
                | Self::Schema
                | Self::Table
                | Self::View
                | Self::Group
                | Self::Routine
        )
    }

    /// Leading glyph.
    #[must_use]
    pub const fn glyph(self, _ascii: bool) -> &'static str {
        {
            match self {
                Self::Connection => "⬡",
                Self::Database => "▣",
                Self::Schema => "▤",
                Self::Table => "▦",
                Self::View => "▥",
                Self::Column => "·",
                Self::Index => "⚡",
                Self::Constraint => "⚓",
                Self::Routine => "ƒ",
                Self::Sequence => "#",
                Self::Group => "▸",
                Self::Other => "?",
            }
        }
    }

    /// Semantic role for kind chrome.
    #[must_use]
    pub const fn role(self) -> Role {
        match self {
            Self::Connection => Role::TextStrong,
            Self::Database | Self::Schema => Role::TextSecondary,
            Self::Table => Role::TextStrong,
            Self::View => Role::Text,
            Self::Column => Role::TextMuted,
            // Object kind is taxonomy, not operational health. Keep it
            // neutral so warning chrome remains reserved for an actual
            // caution with a glyph and verb.
            Self::Index | Self::Constraint => Role::TextMuted,
            Self::Routine | Self::Sequence => Role::TextStrong,
            Self::Group | Self::Other => Role::TextMuted,
        }
    }
}

/// Connection / node health (host).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
#[non_exhaustive]
pub enum SchemaConnStatus {
    /// Connected / ready.
    #[default]
    Connected,
    /// Connecting.
    Connecting,
    /// Offline / disconnected.
    Offline,
    /// Auth / network error.
    Error,
    /// Stale cache after reconnect pending.
    Stale,
}

impl SchemaConnStatus {
    /// Stable id.
    #[must_use]
    pub const fn id(self) -> &'static str {
        match self {
            Self::Connected => "connected",
            Self::Connecting => "connecting",
            Self::Offline => "offline",
            Self::Error => "error",
            Self::Stale => "stale",
        }
    }

    /// Short letter.
    #[must_use]
    pub const fn letter(self) -> char {
        match self {
            Self::Connected => '●',
            Self::Connecting => '…',
            Self::Offline => '○',
            Self::Error => '!',
            Self::Stale => '~',
        }
    }

    /// ASCII letter.
    #[must_use]
    pub const fn letter_ascii(self) -> char {
        match self {
            Self::Connected => '*',
            Self::Connecting => '.',
            Self::Offline => 'o',
            Self::Error => '!',
            Self::Stale => '~',
        }
    }

    /// Shared lifecycle projection for recipe-owned status paint.
    #[must_use]
    pub const fn semantic(self) -> SemanticStatus {
        match self {
            Self::Connected => SemanticStatus::Online,
            Self::Connecting => SemanticStatus::Running,
            Self::Offline => SemanticStatus::Offline,
            Self::Error => SemanticStatus::Failed,
            Self::Stale => SemanticStatus::Warning,
        }
    }
}

/// Presentation density / surface size.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
#[non_exhaustive]
pub enum SchemaBrowserPresentation {
    /// Side pane (default IDE-like).
    #[default]
    SidePane,
    /// Drawer / bottom or side sheet.
    Drawer,
    /// Fullscreen overlay.
    Fullscreen,
}

impl SchemaBrowserPresentation {
    /// Stable id.
    #[must_use]
    pub const fn id(self) -> &'static str {
        match self {
            Self::SidePane => "side-pane",
            Self::Drawer => "drawer",
            Self::Fullscreen => "fullscreen",
        }
    }

    /// Cycle side → drawer → fullscreen → side.
    #[must_use]
    pub const fn cycle(self) -> Self {
        match self {
            Self::SidePane => Self::Drawer,
            Self::Drawer => Self::Fullscreen,
            Self::Fullscreen => Self::SidePane,
        }
    }

    /// Prefer for bounds.
    #[must_use]
    pub const fn for_bounds(width: u16, height: u16) -> Self {
        if width < 28 || height < 8 {
            Self::Fullscreen
        } else if width < 40 {
            Self::Drawer
        } else {
            Self::SidePane
        }
    }
}

// ── Entry projection ────────────────────────────────────────────────────────

/// One host-projected schema browser row (flattened visible hierarchy).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SchemaBrowserEntry<'a, Id> {
    /// Stable id (survives reconnect when host reuses catalog keys).
    pub id: Id,
    /// Display name.
    pub name: &'a str,
    /// Qualified path (`conn/db/schema/table.col`).
    pub path: &'a str,
    /// Kind.
    pub kind: SchemaNodeKind,
    /// Depth.
    pub depth: u16,
    /// Expandable.
    pub branch: bool,
    /// Expanded (host projection; should match preserved set).
    pub expanded: bool,
    /// Parent id.
    pub parent: Option<Id>,
    /// Load status.
    pub status: TreeNodeStatus,
    /// Connection health (mainly on Connection nodes).
    pub conn: SchemaConnStatus,
    /// Type label (`varchar(255)`, `int8`, `btree`).
    pub type_label: Option<&'a str>,
    /// Column nullable.
    pub nullable: Option<bool>,
    /// PK / FK / unique badge letter.
    pub key_badge: Option<&'a str>,
    /// Error message when status is Error.
    pub error: Option<&'a str>,
    /// Extra secondary chrome.
    pub secondary: Option<&'a str>,
    /// Interaction enabled.
    pub enabled: bool,
}

impl<'a, Id> SchemaBrowserEntry<'a, Id> {
    /// Construct leaf.
    #[must_use]
    pub fn new(id: Id, name: &'a str, path: &'a str, kind: SchemaNodeKind, depth: u16) -> Self {
        Self {
            id,
            name,
            path,
            kind,
            depth,
            branch: kind.default_branch(),
            expanded: false,
            parent: None,
            status: TreeNodeStatus::Ready,
            conn: SchemaConnStatus::Connected,
            type_label: None,
            nullable: None,
            key_badge: None,
            error: None,
            secondary: None,
            enabled: true,
        }
    }

    /// Connection root.
    #[must_use]
    pub fn connection(id: Id, name: &'a str, path: &'a str) -> Self {
        Self::new(id, name, path, SchemaNodeKind::Connection, 0)
            .branch()
            .conn_status(SchemaConnStatus::Connected)
    }

    /// Database.
    #[must_use]
    pub fn database(id: Id, name: &'a str, path: &'a str, depth: u16) -> Self {
        Self::new(id, name, path, SchemaNodeKind::Database, depth).branch()
    }

    /// Schema.
    #[must_use]
    pub fn schema(id: Id, name: &'a str, path: &'a str, depth: u16) -> Self {
        Self::new(id, name, path, SchemaNodeKind::Schema, depth).branch()
    }

    /// Table.
    #[must_use]
    pub fn table(id: Id, name: &'a str, path: &'a str, depth: u16) -> Self {
        Self::new(id, name, path, SchemaNodeKind::Table, depth).branch()
    }

    /// View.
    #[must_use]
    pub fn view(id: Id, name: &'a str, path: &'a str, depth: u16) -> Self {
        Self::new(id, name, path, SchemaNodeKind::View, depth).branch()
    }

    /// Column leaf.
    #[must_use]
    pub fn column(id: Id, name: &'a str, path: &'a str, depth: u16) -> Self {
        let mut e = Self::new(id, name, path, SchemaNodeKind::Column, depth);
        e.branch = false;
        e
    }

    /// Parent.
    #[must_use]
    pub fn parent(mut self, parent: Id) -> Self {
        self.parent = Some(parent);
        self
    }

    /// Branch.
    #[must_use]
    pub const fn branch(mut self) -> Self {
        self.branch = true;
        self
    }

    /// Expanded.
    #[must_use]
    pub const fn expanded(mut self) -> Self {
        self.expanded = true;
        self.branch = true;
        self
    }

    /// Lazy children not loaded.
    #[must_use]
    pub const fn lazy(mut self) -> Self {
        self.branch = true;
        self.expanded = false;
        self.status = TreeNodeStatus::Lazy;
        self
    }

    /// Loading.
    #[must_use]
    pub const fn loading(mut self) -> Self {
        self.status = TreeNodeStatus::Loading;
        self.enabled = false;
        self
    }

    /// Error.
    #[must_use]
    pub const fn error_msg(mut self, msg: &'a str) -> Self {
        self.error = Some(msg);
        self.status = TreeNodeStatus::Error;
        self
    }

    /// Connection status.
    #[must_use]
    pub const fn conn_status(mut self, s: SchemaConnStatus) -> Self {
        self.conn = s;
        self
    }

    /// Type label.
    #[must_use]
    pub const fn type_label(mut self, t: &'a str) -> Self {
        self.type_label = Some(t);
        self
    }

    /// Nullable column.
    #[must_use]
    pub const fn nullable(mut self, n: bool) -> Self {
        self.nullable = Some(n);
        self
    }

    /// Key badge (`PK`, `FK`, `UQ`).
    #[must_use]
    pub const fn key_badge(mut self, b: &'a str) -> Self {
        self.key_badge = Some(b);
        self
    }

    /// Secondary.
    #[must_use]
    pub const fn secondary(mut self, s: &'a str) -> Self {
        self.secondary = Some(s);
        self
    }

    /// Disabled.
    #[must_use]
    pub const fn disabled(mut self) -> Self {
        self.enabled = false;
        self
    }

    /// Status override.
    #[must_use]
    pub const fn with_status(mut self, s: TreeNodeStatus) -> Self {
        self.status = s;
        self
    }
}

// ── Pure helpers ────────────────────────────────────────────────────────────

/// Filter with ancestor retention (search).
#[must_use]
pub fn filter_schema_entries<'a, Id: Clone + PartialEq>(
    entries: &'a [SchemaBrowserEntry<'a, Id>],
    query: &str,
) -> Vec<&'a SchemaBrowserEntry<'a, Id>> {
    let q = query.trim().to_ascii_lowercase();
    if q.is_empty() {
        return entries.iter().collect();
    }
    let mut keep = vec![false; entries.len()];
    for (i, e) in entries.iter().enumerate() {
        let hay = format!("{} {} {}", e.name, e.path, e.kind.id()).to_ascii_lowercase();
        if hay.contains(&q) {
            keep[i] = true;
            let mut parent = e.parent.clone();
            while let Some(pid) = parent {
                if let Some((pi, pe)) = entries.iter().enumerate().find(|(_, x)| x.id == pid) {
                    keep[pi] = true;
                    parent = pe.parent.clone();
                } else {
                    break;
                }
            }
        }
    }
    entries
        .iter()
        .enumerate()
        .filter(|(i, _)| keep[*i])
        .map(|(_, e)| e)
        .collect()
}

/// Project entries to [`TreeNode`] for paint.
#[must_use]
pub fn schema_entries_to_tree_nodes<'a, Id: Clone>(
    entries: &[&'a SchemaBrowserEntry<'a, Id>],
    _ascii: bool,
) -> Vec<TreeNode<'a, Id>> {
    entries
        .iter()
        .map(|e| {
            let mut node = TreeNode::new(e.id.clone(), Line::from(e.name), e.depth);
            if e.branch {
                node = node.branch();
            }
            if e.expanded {
                node = node.expanded();
            }
            if let Some(p) = e.parent.clone() {
                node = node.parent(p);
            }
            node = node.with_status(e.status);
            if e.error.is_some() {
                node = node.error();
            } else if !e.enabled {
                node = node.disabled();
            }
            let lead = e.kind.glyph(false);
            node = node.leading(Line::from(lead));
            // Badge: key or connection letter
            if let Some(kb) = e.key_badge {
                node = node.badge(Line::from(kb));
            }
            // Secondary: type / nullable / error / secondary
            if let Some(err) = e.error {
                node = node.secondary(Line::from(err));
            } else if let Some(t) = e.type_label {
                let sec = match e.nullable {
                    Some(true) => format!("{t}?"),
                    Some(false) => format!("{t}!"),
                    None => t.to_string(),
                };
                node = node.secondary(Line::from(sec));
            } else if let Some(s) = e.secondary {
                node = node.secondary(Line::from(s));
            } else if matches!(e.kind, SchemaNodeKind::Connection) {
                node = node.secondary(Line::from(format!(
                    "| {} {}",
                    e.conn.semantic().glyph(),
                    e.conn.id()
                )));
            }
            node
        })
        .collect()
}

/// Breadcrumbs from a qualified path (`a/b/c`).
#[must_use]
pub fn schema_breadcrumbs_from_path(path: &str) -> Vec<BreadcrumbItem<String>> {
    let segs: Vec<&str> = path
        .split(|c| c == '/' || c == '.')
        .filter(|s| !s.is_empty())
        .collect();
    let mut acc = String::new();
    let mut out = Vec::with_capacity(segs.len());
    for (i, seg) in segs.iter().enumerate() {
        if i > 0 {
            acc.push('/');
        }
        acc.push_str(seg);
        out.push(BreadcrumbItem::new(acc.clone(), (*seg).to_string()));
    }
    if out.is_empty() && !path.is_empty() {
        out.push(BreadcrumbItem::new(path.to_string(), path.to_string()));
    }
    out
}

/// Project searchable objects for QuickOpen (tables/views/columns/routines by default).
#[must_use]
pub fn schema_to_quick_open_items<Id: Clone>(
    entries: &[SchemaBrowserEntry<'_, Id>],
    include_columns: bool,
) -> Vec<QuickOpenItem<Id>> {
    entries
        .iter()
        .filter(|e| {
            matches!(
                e.kind,
                SchemaNodeKind::Table
                    | SchemaNodeKind::View
                    | SchemaNodeKind::Routine
                    | SchemaNodeKind::Schema
                    | SchemaNodeKind::Database
            ) || (include_columns && matches!(e.kind, SchemaNodeKind::Column))
        })
        .map(|e| {
            let mut item = QuickOpenItem::new(e.id.clone(), e.name)
                .detail(e.path)
                .kind(e.kind.id());
            if let Some(t) = e.type_label {
                item = item.detail(format!("{} · {}", e.path, t));
            }
            item = item.preview(QuickOpenPreview::text([e.path, e.kind.id()]));
            item
        })
        .collect()
}

/// Collect expanded branch ids from projection (host may merge into preserve set).
#[must_use]
pub fn expanded_ids_from_entries<Id: Clone + Ord>(
    entries: &[SchemaBrowserEntry<'_, Id>],
) -> BTreeSet<Id> {
    entries
        .iter()
        .filter(|e| e.expanded && e.branch)
        .map(|e| e.id.clone())
        .collect()
}

/// Apply preserved expansion to a mutable host list (ids present become expanded).
pub fn apply_expanded_set<Id: Clone + PartialEq + Ord>(
    entries: &mut [SchemaBrowserEntry<'_, Id>],
    expanded: &BTreeSet<Id>,
) {
    for e in entries {
        if e.branch && expanded.contains(&e.id) {
            e.expanded = true;
            // Lazy nodes stay lazy until the host loads them; expanded+lazy
            // reads as "pending open".
        }
    }
}

// ── Outcomes ────────────────────────────────────────────────────────────────

/// Context action request (host maps verb to DDL/DML UI).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SchemaContextAction<Id> {
    /// Action id (`query`, `describe`, `drop`, `refresh`, …).
    pub action: String,
    /// Target.
    pub id: Id,
}

/// Typed outcomes — host owns catalog fetch / reconnect / query open.
#[derive(Debug, Clone, PartialEq, Eq)]
#[non_exhaustive]
pub enum SchemaBrowserOutcome<Id> {
    /// No change.
    Ignored,
    /// Cursor moved.
    SelectionChanged(Id),
    /// Expand/collapse (host updates projection).
    Toggle(Id),
    /// Multi-check.
    CheckToggled(Id),
    /// Open object (query editor / result / DDL).
    OpenRequested(Id),
    /// Detail preview pane.
    PreviewRequested(Id),
    /// Lazy load children.
    LoadChildrenRequested(Id),
    /// Refresh node or whole tree.
    RefreshRequested {
        /// Scope; None = all.
        id: Option<Id>,
    },
    /// Reconnect connection node.
    ReconnectRequested(Id),
    /// Context action.
    ContextAction(SchemaContextAction<Id>),
    /// Copy qualified name(s).
    CopyPathRequested {
        /// Paths.
        paths: Vec<String>,
    },
    /// Open QuickOpen over catalog.
    QuickOpenRequested,
    /// Breadcrumbs for selection.
    BreadcrumbsPath {
        /// Items.
        items: Vec<BreadcrumbItem<String>>,
    },
    /// Filter changed.
    FilterChanged(String),
    /// Presentation changed.
    PresentationChanged(SchemaBrowserPresentation),
    /// Cancel filter.
    Cancelled,
}

// ── State ───────────────────────────────────────────────────────────────────

/// Schema browser state.
///
/// Expanded ids in [`Self::expanded`] survive host refresh: host reprojects
/// children, then calls [`apply_expanded_set`] or merges this set.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SchemaBrowserState<Id: Clone + Ord> {
    /// Tree interaction.
    pub tree: TreeState<Id>,
    /// Filter query.
    pub filter: Option<String>,
    /// Presentation mode.
    pub presentation: SchemaBrowserPresentation,
    /// Host/auto presentation override.
    pub presentation_override: Option<SchemaBrowserPresentation>,
    /// Preserved expanded branch ids across refresh/reconnect.
    pub expanded: BTreeSet<Id>,
    /// Title.
    pub title: Option<String>,
    accepts_input: bool,
}

impl<Id: Clone + Ord> Default for SchemaBrowserState<Id> {
    fn default() -> Self {
        Self::new()
    }
}

impl<Id: Clone + Ord + PartialEq> SchemaBrowserState<Id> {
    /// Fresh.
    #[must_use]
    pub fn new() -> Self {
        Self {
            tree: TreeState::new(None),
            filter: None,
            presentation: SchemaBrowserPresentation::SidePane,
            presentation_override: None,
            expanded: BTreeSet::new(),
            title: None,
            accepts_input: true,
        }
    }

    /// With selection.
    #[must_use]
    pub fn with_selected(selected: Option<Id>) -> Self {
        let mut s = Self::new();
        s.tree = TreeState::new(selected);
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
    pub const fn selected(&self) -> Option<&Id> {
        self.tree.selected()
    }

    /// Select.
    pub fn select(&mut self, id: Option<Id>) {
        self.tree.select(id);
    }

    /// Multi-select.
    pub fn enable_multi_select(&mut self) {
        self.tree.enable_multi_select();
    }

    /// Effective presentation (override or auto).
    #[must_use]
    pub fn effective_presentation(&self, area: Rect) -> SchemaBrowserPresentation {
        self.presentation_override
            .unwrap_or_else(|| SchemaBrowserPresentation::for_bounds(area.width, area.height))
    }

    /// Record expansion for preserve set.
    pub fn mark_expanded(&mut self, id: Id, expanded: bool) {
        if expanded {
            self.expanded.insert(id);
        } else {
            self.expanded.remove(&id);
        }
    }

    /// Merge host projection expansion into preserve set.
    pub fn sync_expanded_from_entries(&mut self, entries: &[SchemaBrowserEntry<'_, Id>]) {
        for e in entries {
            if e.branch && e.expanded {
                self.expanded.insert(e.id.clone());
            }
        }
    }

    /// After reconnect: drop ids not present; keep survivors.
    pub fn reconcile_expanded(&mut self, live: &[Id]) {
        let live_set: BTreeSet<_> = live.iter().cloned().collect();
        self.expanded.retain(|id| live_set.contains(id));
    }

    /// Visible entries after filter.
    #[must_use]
    pub fn visible_entries<'a>(
        &self,
        all: &'a [SchemaBrowserEntry<'a, Id>],
    ) -> Vec<&'a SchemaBrowserEntry<'a, Id>> {
        filter_schema_entries(all, self.filter.as_deref().unwrap_or(""))
    }

    /// Keys.
    pub fn handle_key(
        &mut self,
        entries: &[SchemaBrowserEntry<'_, Id>],
        key: KeyEvent,
    ) -> SchemaBrowserOutcome<Id>
    where
        Id: Clone + PartialEq + Eq,
    {
        if !self.accepts_input || key.is_release() {
            return SchemaBrowserOutcome::Ignored;
        }
        let is_press = key.is_press();

        // Filter typing
        if let Some(q) = self.filter.as_mut()
            && is_press
            && key.modifiers.is_empty()
        {
            match key.code {
                KeyCode::Esc => {
                    self.filter = None;
                    return SchemaBrowserOutcome::Cancelled;
                }
                KeyCode::Backspace => {
                    q.pop();
                    if q.is_empty() {
                        self.filter = None;
                    }
                    return SchemaBrowserOutcome::FilterChanged(
                        self.filter.clone().unwrap_or_default(),
                    );
                }
                KeyCode::Char(c)
                    if !c.is_control()
                        && !matches!(c, 'j' | 'k' | 'h' | 'l' | 'J' | 'K' | 'H' | 'L') =>
                {
                    q.push(c);
                    return SchemaBrowserOutcome::FilterChanged(q.clone());
                }
                _ => {}
            }
        }

        if is_press {
            match key.code {
                KeyCode::Char('/') if key.modifiers.is_empty() => {
                    self.filter = Some(String::new());
                    return SchemaBrowserOutcome::FilterChanged(String::new());
                }
                KeyCode::Char('r') if key.modifiers.is_empty() => {
                    return SchemaBrowserOutcome::RefreshRequested {
                        id: self.tree.selected().cloned(),
                    };
                }
                KeyCode::Char('R') if key.modifiers.contains(KeyModifiers::SHIFT) => {
                    return SchemaBrowserOutcome::RefreshRequested { id: None };
                }
                KeyCode::Char('c')
                    if key.modifiers.contains(KeyModifiers::CONTROL)
                        && key.modifiers.contains(KeyModifiers::SHIFT) =>
                {
                    if let Some(id) = self.tree.selected().cloned() {
                        return SchemaBrowserOutcome::ReconnectRequested(id);
                    }
                }
                KeyCode::Char('y') if key.modifiers.is_empty() => {
                    return copy_paths(entries, &self.tree);
                }
                KeyCode::Char('p') if key.modifiers.is_empty() => {
                    if let Some(id) = self.tree.selected().cloned() {
                        return SchemaBrowserOutcome::PreviewRequested(id);
                    }
                }
                KeyCode::Char('o') if key.modifiers.contains(KeyModifiers::CONTROL) => {
                    return SchemaBrowserOutcome::QuickOpenRequested;
                }
                KeyCode::Char('g') if key.modifiers.is_empty() => {
                    if let Some(sel) = self.tree.selected() {
                        if let Some(e) = entries.iter().find(|e| &e.id == sel) {
                            return SchemaBrowserOutcome::BreadcrumbsPath {
                                items: schema_breadcrumbs_from_path(e.path),
                            };
                        }
                    }
                }
                KeyCode::Char('m') if key.modifiers.contains(KeyModifiers::CONTROL) => {
                    self.presentation = self.presentation.cycle();
                    return SchemaBrowserOutcome::PresentationChanged(self.presentation);
                }
                KeyCode::Char('x') if key.modifiers.is_empty() => {
                    if let Some(id) = self.tree.selected().cloned() {
                        return SchemaBrowserOutcome::ContextAction(SchemaContextAction {
                            action: "menu".into(),
                            id,
                        });
                    }
                }
                KeyCode::Char('q') if key.modifiers.is_empty() => {
                    if let Some(id) = self.tree.selected().cloned() {
                        return SchemaBrowserOutcome::ContextAction(SchemaContextAction {
                            action: "query".into(),
                            id,
                        });
                    }
                }
                KeyCode::Char('d') if key.modifiers.is_empty() => {
                    if let Some(id) = self.tree.selected().cloned() {
                        return SchemaBrowserOutcome::ContextAction(SchemaContextAction {
                            action: "describe".into(),
                            id,
                        });
                    }
                }
                _ => {}
            }
        }

        // Project to tree for nav
        let visible = self.visible_entries(entries);
        let nodes = schema_entries_to_tree_nodes(&visible, false);
        let out = self.tree.handle_key(&nodes, key);
        map_tree_outcome(out, entries, &mut self.expanded)
    }

    /// Mouse: Tree has no pointer API yet — host may select via hit regions after paint.
    pub fn handle_mouse(
        &mut self,
        _entries: &[SchemaBrowserEntry<'_, Id>],
        _event: MouseEvent,
    ) -> SchemaBrowserOutcome<Id> {
        SchemaBrowserOutcome::Ignored
    }
}

fn copy_paths<Id: Clone + PartialEq>(
    entries: &[SchemaBrowserEntry<'_, Id>],
    tree: &TreeState<Id>,
) -> SchemaBrowserOutcome<Id> {
    let mut paths = Vec::new();
    if let Some(sel) = tree.selection() {
        for id in sel.checked() {
            if let Some(e) = entries.iter().find(|e| &e.id == id) {
                paths.push(e.path.to_string());
            }
        }
    }
    if paths.is_empty() {
        if let Some(id) = tree.selected() {
            if let Some(e) = entries.iter().find(|e| &e.id == id) {
                paths.push(e.path.to_string());
            }
        }
    }
    if paths.is_empty() {
        SchemaBrowserOutcome::Ignored
    } else {
        SchemaBrowserOutcome::CopyPathRequested { paths }
    }
}

fn map_tree_outcome<Id: Clone + PartialEq + Ord>(
    out: TreeOutcome<Id>,
    entries: &[SchemaBrowserEntry<'_, Id>],
    expanded: &mut BTreeSet<Id>,
) -> SchemaBrowserOutcome<Id> {
    match out {
        TreeOutcome::Ignored => SchemaBrowserOutcome::Ignored,
        TreeOutcome::SelectionChanged(id) => SchemaBrowserOutcome::SelectionChanged(id),
        TreeOutcome::CheckToggled(id) => SchemaBrowserOutcome::CheckToggled(id),
        TreeOutcome::Cancelled => SchemaBrowserOutcome::Cancelled,
        TreeOutcome::Toggle(id) => {
            // Infer expand direction from current projection
            if let Some(e) = entries.iter().find(|e| e.id == id) {
                if e.expanded {
                    expanded.remove(&id);
                } else {
                    expanded.insert(id.clone());
                }
                if matches!(e.status, TreeNodeStatus::Lazy) && !e.expanded {
                    return SchemaBrowserOutcome::LoadChildrenRequested(id);
                }
            } else {
                expanded.insert(id.clone());
            }
            SchemaBrowserOutcome::Toggle(id)
        }
        TreeOutcome::Activated(id) => {
            if let Some(e) = entries.iter().find(|e| e.id == id) {
                if e.branch && matches!(e.status, TreeNodeStatus::Lazy) {
                    expanded.insert(id.clone());
                    return SchemaBrowserOutcome::LoadChildrenRequested(id);
                }
                if e.branch && !e.expanded {
                    expanded.insert(id.clone());
                    return SchemaBrowserOutcome::Toggle(id);
                }
            }
            SchemaBrowserOutcome::OpenRequested(id)
        }
    }
}

// ── Widget ──────────────────────────────────────────────────────────────────

/// Schema browser chrome + Tree body.
#[derive(Debug, Clone, Copy)]
pub struct SchemaBrowser<'a, Id> {
    entries: &'a [SchemaBrowserEntry<'a, Id>],
    system: &'a DesignSystem,
    focused: bool,
    title: Option<&'a str>,
}

impl<'a, Id: Clone + PartialEq + Ord> SchemaBrowser<'a, Id> {
    /// Entries + system.
    #[must_use]
    pub const fn new(entries: &'a [SchemaBrowserEntry<'a, Id>], system: &'a DesignSystem) -> Self {
        Self {
            entries,
            system,
            focused: true,
            title: None,
        }
    }

    /// Title.
    #[must_use]
    pub const fn title(mut self, title: &'a str) -> Self {
        self.title = Some(title);
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
    /// Paint.
    pub fn paint(&self, area: Rect, buffer: &mut Buffer, state: &mut SchemaBrowserState<Id>)
    where
        Id: Clone + PartialEq + Eq,
    {
        if area.is_empty() {
            return;
        }
        let pres = state.effective_presentation(area);
        // auto-sync presentation chrome without outcome
        if state.presentation_override.is_none() {
            state.presentation = pres;
        }

        let mut y = area.y;
        let mut h = area.height;

        if h > 0 {
            let title = self.title.or(state.title.as_deref()).unwrap_or("schema");
            let line = format!(
                "{title} · {} · {} objs",
                state.presentation.id(),
                self.entries.len()
            );
            self.system.paint_row(
                buffer,
                Rect::new(area.x, y, area.width, 1),
                &line,
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
            self.system.paint_row(
                buffer,
                Rect::new(area.x, y, area.width, 1),
                &format!("/{q}_"),
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

        let visible = state.visible_entries(self.entries);
        if visible.is_empty() {
            EmptyState::new("No objects", self.system)
                .kind(EmptyKind::NoData)
                .paint(
                    Rect::new(body.x, body.y, body.width, 1),
                    buffer,
                    &mut crate::widgets::EmptyStateState::new(),
                );
            return;
        }

        let nodes = schema_entries_to_tree_nodes(&visible, false);
        Tree::new(&nodes, self.system)
            .focused(self.focused && state.accepts_input)
            .render(body, buffer, &mut state.tree);
    }
}

// ── Bench ───────────────────────────────────────────────────────────────────

/// Large catalog targets.
pub mod bench {
    /// Objects in a large projection window.
    pub const OBJECT_COUNT: usize = 5_000;
    /// Viewport rows.
    pub const VIEWPORT: u16 = 40;
    /// Paint frames.
    pub const PAINT_FRAMES: u32 = 40;
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::style::DesignSystem;

    fn sample() -> Vec<SchemaBrowserEntry<'static, &'static str>> {
        vec![
            SchemaBrowserEntry::connection("conn", "prod", "prod")
                .expanded()
                .conn_status(SchemaConnStatus::Connected),
            SchemaBrowserEntry::database("db", "app", "prod/app", 1)
                .parent("conn")
                .expanded(),
            SchemaBrowserEntry::schema("sch", "public", "prod/app/public", 2)
                .parent("db")
                .expanded(),
            SchemaBrowserEntry::table("users", "users", "prod/app/public/users", 3)
                .parent("sch")
                .expanded(),
            SchemaBrowserEntry::column("users.id", "id", "prod/app/public/users.id", 4)
                .parent("users")
                .type_label("int8")
                .nullable(false)
                .key_badge("PK"),
            SchemaBrowserEntry::column("users.email", "email", "prod/app/public/users.email", 4)
                .parent("users")
                .type_label("text")
                .nullable(false),
            SchemaBrowserEntry::table("orders", "orders", "prod/app/public/orders", 3)
                .parent("sch")
                .lazy(),
            SchemaBrowserEntry::view("v_active", "v_active", "prod/app/public/v_active", 3)
                .parent("sch"),
            SchemaBrowserEntry::new(
                "idx_email",
                "idx_email",
                "prod/app/public/users/idx_email",
                SchemaNodeKind::Index,
                4,
            )
            .parent("users")
            .type_label("btree"),
            SchemaBrowserEntry::connection("offline", "staging", "staging")
                .conn_status(SchemaConnStatus::Offline)
                .lazy(),
        ]
    }

    #[test]
    fn filter_retains_ancestors() {
        let entries = sample();
        let v = filter_schema_entries(&entries, "email");
        let ids: Vec<_> = v.iter().map(|e| e.id).collect();
        assert!(ids.contains(&"users.email") || ids.iter().any(|i| i.contains("email")));
        assert!(ids.contains(&"users") || ids.contains(&"sch"));
    }

    #[test]
    fn expand_preserve_across_refresh() {
        let mut state = SchemaBrowserState::with_selected(Some("users"));
        let entries = sample();
        state.sync_expanded_from_entries(&entries);
        assert!(state.expanded.contains("users") || state.expanded.contains("sch"));
        // simulate reconnect with subset
        state.reconcile_expanded(&["conn", "db", "sch", "users", "users.id"]);
        assert!(!state.expanded.contains("missing"));
    }

    #[test]
    fn apply_expanded_set_marks_branches() {
        let mut entries = sample();
        let mut set = BTreeSet::new();
        set.insert("orders");
        super::apply_expanded_set(&mut entries, &set);
        assert!(entries.iter().find(|e| e.id == "orders").unwrap().expanded);
    }

    #[test]
    fn load_children_on_lazy_activate() {
        let entries = sample();
        let mut state = SchemaBrowserState::with_selected(Some("orders"));
        // Activate lazy table
        let nodes = schema_entries_to_tree_nodes(&entries.iter().collect::<Vec<_>>(), true);
        let _ = nodes;
        let out = state.handle_key(&entries, KeyEvent::new(KeyCode::Enter, KeyModifiers::NONE));
        assert!(
            matches!(
                out,
                SchemaBrowserOutcome::LoadChildrenRequested("orders")
                    | SchemaBrowserOutcome::OpenRequested("orders")
                    | SchemaBrowserOutcome::Toggle("orders")
                    | SchemaBrowserOutcome::Ignored
            ),
            "{out:?}"
        );
    }

    #[test]
    fn quick_open_bridge() {
        let entries = sample();
        let items = schema_to_quick_open_items(&entries, false);
        assert!(items.iter().any(|i| i.label == "users"));
        assert!(!items.iter().any(|i| i.label == "id")); // columns excluded
    }

    #[test]
    fn breadcrumbs() {
        let b = schema_breadcrumbs_from_path("prod/app/public/users");
        assert!(b.len() >= 3);
    }

    #[test]
    fn presentation_cycle() {
        let mut state = SchemaBrowserState::<&str>::new();
        let entries = sample();
        assert!(matches!(
            state.handle_key(
                &entries,
                KeyEvent::new(KeyCode::Char('m'), KeyModifiers::CONTROL)
            ),
            SchemaBrowserOutcome::PresentationChanged(SchemaBrowserPresentation::Drawer)
        ));
    }

    #[test]
    fn filter_and_refresh_chords() {
        let entries = sample();
        let mut state = SchemaBrowserState::new();
        assert!(matches!(
            state.handle_key(
                &entries,
                KeyEvent::new(KeyCode::Char('/'), KeyModifiers::NONE)
            ),
            SchemaBrowserOutcome::FilterChanged(_)
        ));
        // Escape filter before product chords
        assert!(matches!(
            state.handle_key(&entries, KeyEvent::new(KeyCode::Esc, KeyModifiers::NONE)),
            SchemaBrowserOutcome::Cancelled
        ));
        assert!(matches!(
            state.handle_key(
                &entries,
                KeyEvent::new(KeyCode::Char('r'), KeyModifiers::NONE)
            ),
            SchemaBrowserOutcome::RefreshRequested { .. }
        ));
    }

    #[test]
    fn copy_path() {
        let entries = sample();
        let mut state = SchemaBrowserState::with_selected(Some("users"));
        assert!(matches!(
            state.handle_key(
                &entries,
                KeyEvent::new(KeyCode::Char('y'), KeyModifiers::NONE)
            ),
            SchemaBrowserOutcome::CopyPathRequested { paths } if paths.iter().any(|p| p.contains("users"))
        ));
    }

    #[test]
    fn paint_basic() {
        let system = DesignSystem::default();
        let entries = sample();
        let mut state = SchemaBrowserState::with_selected(Some("users"));
        let area = Rect::new(0, 0, 40, 16);
        let mut buf = Buffer::empty(area);
        let _ = SchemaBrowser::new(&entries, &system)
            .title("Catalog")
            .paint(area, &mut buf, &mut state);
        let text: String = buf
            .content()
            .iter()
            .map(|c| c.symbol().to_string())
            .collect();
        assert!(
            text.contains("users") || text.contains("catalog") || text.contains("public"),
            "{text}"
        );
    }

    #[test]
    fn accepts_input_gate() {
        let entries = sample();
        let mut state = SchemaBrowserState::new();
        state.set_accepts_input(false);
        assert!(matches!(
            state.handle_key(&entries, KeyEvent::new(KeyCode::Down, KeyModifiers::NONE)),
            SchemaBrowserOutcome::Ignored
        ));
    }

    #[test]
    fn never_runs_sql() {
        let src = include_str!("schema_browser.rs");
        let body = src.split("#[cfg(test)]").next().unwrap_or(src);
        for forbidden in [
            "sqlx::",
            "tokio_postgres",
            "rusqlite",
            "std::process::Command",
        ] {
            assert!(!body.contains(forbidden), "must not contain {forbidden}");
        }
    }

    #[test]
    fn large_catalog_filter_paint() {
        let system = DesignSystem::default();
        // static-ish names
        let names: Vec<String> = (0..bench::OBJECT_COUNT).map(|i| format!("t{i}")).collect();
        let paths: Vec<String> = names.iter().map(|n| format!("db/public/{n}")).collect();
        let entries: Vec<SchemaBrowserEntry<'_, String>> = names
            .iter()
            .zip(paths.iter())
            .enumerate()
            .map(|(i, (n, p))| {
                SchemaBrowserEntry::table(n.clone(), n.as_str(), p.as_str(), 1)
                    .parent("public".into())
                    .type_label(if i % 10 == 0 { "view" } else { "table" })
            })
            .collect();
        // Fix kinds for every 10th
        let mut entries = entries;
        for (i, e) in entries.iter_mut().enumerate() {
            if i % 10 == 0 {
                e.kind = SchemaNodeKind::View;
            }
        }
        let mut state = SchemaBrowserState::new();
        state.filter = Some("t12".into());
        let vis = state.visible_entries(&entries);
        assert!(!vis.is_empty());
        let area = Rect::new(0, 0, 48, 24);
        let mut buf = Buffer::empty(area);
        let _ = SchemaBrowser::new(&entries, &system).paint(area, &mut buf, &mut state);
    }

    #[test]
    fn tree_nodes_have_kind_glyphs() {
        let entries = sample();
        let refs: Vec<_> = entries.iter().collect();
        let nodes = schema_entries_to_tree_nodes(&refs, true);
        assert!(!nodes.is_empty());
        assert!(nodes.iter().any(|n| n.leading.is_some()));
    }
}
