//! **Tree** — hierarchical collection for files, schemas, tasks, settings, objects.
//!
//! **Anatomy** (junie `TreeView` one-to-one): col 0 is always the focus bar
//! `▎` from [`RowChrome`] / [`DesignSystem::resolve_list_row`]; then
//! `depth × 2` indent; then a two-cell disclosure (`▾` open / `▸` closed /
//! spinner while loading / space for a leaf); optional kind glyph; label
//! (accent when selected, muted for notes); right-aligned meta. Disclosure
//! never occupies the membership-marker slot — that `›` belongs to lists.
//!
//! **Keys.** `↑↓`/`jk`; `→`/`l` expands or steps in; `←`/`h` collapses or
//! steps out; Enter toggles a folder or activates a leaf; Space checks when
//! multi-select is on, otherwise toggles a folder; `*` / `-` request expand-all
//! / collapse-all ([`TreeState::take_bulk_disclosure`]); `g`/`G` first/last.
//!
//! **Ownership.** Host owns hierarchy, expansion set, and lazy fetch. Tree owns
//! cursor/selection interaction, scroll, hit geometry, and typed outcomes.
//!
//! Research: junie TreeView, file explorers, broot, Yazi, VS Code trees.
#![allow(unused_imports)] // test-only imports retained
use ratatui_core::{
    buffer::Buffer,
    layout::{Position, Rect},
    style::{Color, Modifier, Style},
    text::Line,
    widgets::StatefulWidget,
};

use crate::{
    input::{KeyCode, KeyEvent, KeyEventKind, KeyModifiers},
    interaction::{CollectionItem, CollectionState, HitRegion, NavigationMove, PageMove, UiIntent},
    scroll::max_offset,
    style::{DesignSystem, Glyph, ListRowVisualState, Role, SPINNER_BRAILLE_FRAMES},
    text::{display_cols, display_cols_slice_into, take_display_cols},
};

use super::{ComposedRow, Selection, StickyRegion, Virtualizer, row_chrome::RowChrome};

/// Default overscan when using virtualized tree windows.
pub const TREE_DEFAULT_OVERSCAN: u16 = 4;

/// Semantic emphasis for a ready tree row.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
#[non_exhaustive]
pub enum ToneTier {
    /// Ordinary hierarchy content.
    #[default]
    Primary,
    /// Live or currently changing content.
    Live,
    /// Subdued live content that remains distinguishable from ordinary text.
    LiveDim,
}

impl ToneTier {
    const fn role(self) -> Role {
        match self {
            Self::Primary => Role::Text,
            Self::Live => Role::TextStrong,
            Self::LiveDim => Role::TextMuted,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
#[non_exhaustive]
/// Loading, error, and lazy-child states associated with a tree node.
pub enum TreeNodeStatus {
    /// Node content / children are available for ordinary interaction.
    #[default]
    Ready,
    /// Children (or node body) still loading.
    Loading,
    /// Children / content failed to load.
    Error,
    /// Branch not yet loaded (lazy); expand requests a fetch.
    Lazy,
}

impl TreeNodeStatus {
    /// Stable id.
    #[must_use]
    pub const fn id(self) -> &'static str {
        match self {
            Self::Ready => "ready",
            Self::Loading => "loading",
            Self::Error => "error",
            Self::Lazy => "lazy",
        }
    }

    /// Whether keyboard should skip this node.
    #[must_use]
    pub const fn skips_navigation(self) -> bool {
        matches!(self, Self::Loading)
    }
}

#[derive(Debug, Clone)]
/// A stable flattened tree row with hierarchy metadata.
///
/// Host projects only **visible** rows (expanded path). Optional virtual window
/// + [`TreeState::set_virtual_window`] for huge trees.
pub struct TreeNode<'a, Id> {
    /// Stable identity used for selection and activation.
    pub id: Id,
    /// Caller-visible label.
    pub label: Line<'a>,
    /// Optional leading status/icon (composed leading).
    pub leading: Option<Line<'a>>,
    /// Optional secondary metadata (composed secondary).
    pub secondary: Option<Line<'a>>,
    /// Optional badge aligned at the trailing edge.
    pub badge: Option<Line<'a>>,
    /// Optional keyboard shortcut hint.
    pub shortcut: Option<&'a str>,
    /// Optional context-action labels (display; host maps activation).
    pub actions: Option<Line<'a>>,
    /// Zero-based hierarchy depth.
    pub depth: u16,
    /// Whether the node can request disclosure changes.
    pub branch: bool,
    /// Whether this item is expanded.
    pub expanded: bool,
    /// Whether this item is enabled.
    pub enabled: bool,
    /// Optional loading, error, or lazy state.
    pub status: TreeNodeStatus,
    /// Semantic emphasis for ready content.
    pub tone: ToneTier,
    /// Parent id when known (enables filter ancestor retention without re-walk).
    pub parent: Option<Id>,
}

impl<'a, Id> TreeNode<'a, Id> {
    /// Creates a ready leaf/branch node with empty optional anatomy.
    #[must_use]
    pub fn new(id: Id, label: Line<'a>, depth: u16) -> Self {
        Self {
            id,
            label,
            leading: None,
            secondary: None,
            badge: None,
            shortcut: None,
            actions: None,
            depth,
            branch: false,
            expanded: false,
            enabled: true,
            status: TreeNodeStatus::Ready,
            tone: ToneTier::Primary,
            parent: None,
        }
    }

    /// Creates a branch node (disclosure-capable).
    #[must_use]
    pub fn branch(mut self) -> Self {
        self.branch = true;
        self
    }

    /// Lazy branch: expand requests load (status [`TreeNodeStatus::Lazy`]).
    #[must_use]
    pub fn lazy_branch(mut self) -> Self {
        self.branch = true;
        self.expanded = false;
        self.status = TreeNodeStatus::Lazy;
        self
    }

    /// Marks the branch expanded (consumer owns expansion policy).
    #[must_use]
    pub fn expanded(mut self) -> Self {
        self.expanded = true;
        self
    }

    /// Sets parent id (filter / collapse helpers).
    #[must_use]
    pub fn parent(mut self, parent: Id) -> Self {
        self.parent = Some(parent);
        self
    }

    /// Sets leading chrome.
    #[must_use]
    pub fn leading(mut self, leading: Line<'a>) -> Self {
        self.leading = Some(leading);
        self
    }

    /// Sets secondary metadata.
    #[must_use]
    pub fn secondary(mut self, secondary: Line<'a>) -> Self {
        self.secondary = Some(secondary);
        self
    }

    /// Sets badge chrome.
    #[must_use]
    pub fn badge(mut self, badge: Line<'a>) -> Self {
        self.badge = Some(badge);
        self
    }

    /// Sets keyboard shortcut hint.
    #[must_use]
    pub fn shortcut(mut self, shortcut: &'a str) -> Self {
        self.shortcut = Some(shortcut);
        self
    }

    /// Context action labels (display-only).
    #[must_use]
    pub fn actions(mut self, actions: Line<'a>) -> Self {
        self.actions = Some(actions);
        self
    }

    /// Marks the node disabled (skipped by keyboard, non-hittable).
    #[must_use]
    pub fn disabled(mut self) -> Self {
        self.enabled = false;
        self
    }

    /// Marks loading status (disabled interaction + status chrome).
    #[must_use]
    pub fn loading(mut self) -> Self {
        self.status = TreeNodeStatus::Loading;
        self.enabled = false;
        self
    }

    /// Marks error status (danger tone + status chrome).
    #[must_use]
    pub fn error(mut self) -> Self {
        self.status = TreeNodeStatus::Error;
        self
    }

    /// Lazy unloaded children.
    #[must_use]
    pub fn lazy(mut self) -> Self {
        self.status = TreeNodeStatus::Lazy;
        self.branch = true;
        self
    }

    /// Explicit status override.
    #[must_use]
    pub fn with_status(mut self, status: TreeNodeStatus) -> Self {
        self.status = status;
        self
    }

    /// Whether this node participates in navigation and pointer interaction.
    #[must_use]
    pub const fn is_interactive(&self) -> bool {
        self.enabled && !self.status.skips_navigation()
    }

    /// Sets semantic emphasis for this row without hardcoded color.
    #[must_use]
    pub const fn tone(mut self, tone: ToneTier) -> Self {
        self.tone = tone;
        self
    }

    /// Plain label for typeahead / filter.
    #[must_use]
    pub fn plain_label(&self) -> String {
        self.label
            .spans
            .iter()
            .map(|s| s.content.as_ref())
            .collect::<Vec<_>>()
            .join("")
    }

    /// Projects hierarchy chrome + label into shared composed anatomy.
    #[must_use]
    pub fn composed(&self) -> ComposedRow<'a, ()> {
        ComposedRow {
            id: (),
            leading: self.leading.clone(),
            primary: self.label.clone(),
            secondary: self.secondary.clone(),
            badge: self.badge.clone().or_else(|| self.actions.clone()),
            shortcut: self.shortcut,
            enabled: self.is_interactive(),
            loading: matches!(self.status, TreeNodeStatus::Loading | TreeNodeStatus::Lazy),
        }
    }
}

/// Keep nodes matching `query` (case-insensitive) **and** all their ancestors.
///
/// Uses `depth` (and optional `parent`) so filtered trees remain navigable.
#[must_use]
pub fn filter_tree_with_ancestors<'a, Id: Clone + PartialEq>(
    nodes: &'a [TreeNode<'a, Id>],
    query: &str,
) -> Vec<&'a TreeNode<'a, Id>> {
    let q = query.trim().to_ascii_lowercase();
    if q.is_empty() {
        return nodes.iter().collect();
    }
    let mut keep = vec![false; nodes.len()];
    for (i, n) in nodes.iter().enumerate() {
        if n.plain_label().to_ascii_lowercase().contains(&q) {
            keep[i] = true;
            // Mark ancestors by walking up depth.
            let mut depth = n.depth;
            let mut j = i;
            while depth > 0 && j > 0 {
                j -= 1;
                if nodes[j].depth < depth {
                    keep[j] = true;
                    depth = nodes[j].depth;
                }
            }
        }
    }
    nodes
        .iter()
        .enumerate()
        .filter(|(i, _)| keep[*i])
        .map(|(_, n)| n)
        .collect()
}

#[derive(Debug, Clone, PartialEq, Eq)]
#[non_exhaustive]
/// Semantic results produced by tree interaction.
pub enum TreeOutcome<Id> {
    /// The event produced no tree-state change.
    Ignored,
    /// Navigation moved the active cursor to this stable node identity.
    SelectionChanged(Id),
    /// The identified branch requested disclosure inversion (or lazy load).
    Toggle(Id),
    /// Multi-selection toggled this stable node identity.
    CheckToggled(Id),
    /// The identified enabled node requested activation.
    Activated(Id),
    /// Cancel/close intent (consumer maps Esc policy).
    Cancelled,
}

#[derive(Debug, Clone, PartialEq, Eq)]
/// Runtime state for `Tree`.
///
/// Navigation cursor and semantic selection are independent. The cursor is
/// the focused row; semantic selection is committed by activation or an
/// explicit [`TreeState::select`] call. Multi **check** state is optional
/// [`Self::selection`]. Expansion remains on the host projection
/// (`TreeNode.expanded`).
pub struct TreeState<Id> {
    cursor: Option<Id>,
    selected: Option<Id>,
    hovered: Option<Id>,
    offset: usize,
    viewport_height: usize,
    h_offset: u16,
    content_width: u16,
    viewport_width: u16,
    follow_selection: bool,
    regions: Vec<HitRegion<Id>>,
    disclosure_regions: Vec<HitRegion<Id>>,
    selection: Option<Selection<Id>>,
    check_regions: Vec<HitRegion<Id>>,
    scrollbar_region: Option<Rect>,
    /// Typeahead / collection model over the flat projection.
    collection: CollectionState<Id>,
    /// Search query chrome (`/` filter; host applies [`filter_tree_with_ancestors`]).
    filter_query: Option<String>,
    /// When non-zero, `nodes` is a window into a larger flat universe.
    virtual_total: usize,
    virtual_window_start: usize,
    /// Optional virtualizer for anchors / overscan diagnostics.
    virt: Virtualizer,
    /// `*` / `-` bulk disclosure: `Some(true)` expand-all, `Some(false)` collapse-all.
    bulk_disclosure: Option<bool>,
}

impl<Id> Default for TreeState<Id> {
    fn default() -> Self {
        Self {
            cursor: None,
            selected: None,
            hovered: None,
            offset: 0,
            viewport_height: 0,
            h_offset: 0,
            content_width: 0,
            viewport_width: 0,
            follow_selection: false,
            regions: Vec::new(),
            disclosure_regions: Vec::new(),
            selection: None,
            check_regions: Vec::new(),
            scrollbar_region: None,
            collection: CollectionState::new(),
            filter_query: None,
            virtual_total: 0,
            virtual_window_start: 0,
            virt: Virtualizer::fixed(1).with_overscan(TREE_DEFAULT_OVERSCAN),
            bulk_disclosure: None,
        }
    }
}

impl<Id> TreeState<Id> {
    #[must_use]
    /// Creates tree state with no hover/scroll; optional initial cursor.
    pub fn new(cursor: Option<Id>) -> Self
    where
        Id: Clone + PartialEq,
    {
        let mut collection = CollectionState::new();
        collection.set_active(cursor.clone());
        Self {
            cursor,
            selected: None,
            hovered: None,
            offset: 0,
            viewport_height: 0,
            h_offset: 0,
            content_width: 0,
            viewport_width: 0,
            follow_selection: true,
            regions: Vec::new(),
            disclosure_regions: Vec::new(),
            selection: None,
            check_regions: Vec::new(),
            scrollbar_region: None,
            collection,
            filter_query: None,
            virtual_total: 0,
            virtual_window_start: 0,
            virt: Virtualizer::fixed(1).with_overscan(TREE_DEFAULT_OVERSCAN),
            bulk_disclosure: None,
        }
    }

    #[must_use]
    /// Returns the current navigation cursor.
    ///
    /// This accessor retains its historical name for existing consumers;
    /// [`Self::cursor`] is the explicit spelling for new code.
    pub const fn selected(&self) -> Option<&Id> {
        self.cursor.as_ref()
    }

    /// Returns the current navigation cursor.
    #[must_use]
    pub const fn cursor(&self) -> Option<&Id> {
        self.cursor.as_ref()
    }

    /// Returns the committed semantic selection, if any.
    #[must_use]
    pub const fn semantic_selection(&self) -> Option<&Id> {
        self.selected.as_ref()
    }

    /// Sets the committed semantic selection without moving the cursor.
    pub fn set_semantic_selection(&mut self, selected: Option<Id>) {
        self.selected = selected;
    }

    /// `*` expand-all / `-` collapse-all request. Host owns expansion.
    #[must_use]
    pub const fn bulk_disclosure(&self) -> Option<bool> {
        self.bulk_disclosure
    }

    /// Takes the pending `*` / `-` command (`true` = expand all).
    pub fn take_bulk_disclosure(&mut self) -> Option<bool> {
        self.bulk_disclosure.take()
    }

    #[must_use]
    /// Returns the stable identity currently under the pointer.
    pub const fn hovered(&self) -> Option<&Id> {
        self.hovered.as_ref()
    }

    #[must_use]
    /// Returns the first visible logical node index.
    ///
    /// For a virtual projection this is the index in the host's full
    /// collection, not the first index in the resident `nodes` slice.
    pub const fn offset(&self) -> usize {
        self.offset
    }

    /// Horizontal label offset in display columns.
    #[must_use]
    pub const fn h_offset(&self) -> u16 {
        self.h_offset
    }

    /// Sets horizontal label scroll; clamped on the next paint.
    pub fn set_h_offset(&mut self, offset: u16) {
        self.h_offset = offset;
    }

    /// Scrolls the label region while disclosure and indentation remain pinned.
    pub fn scroll_horizontal(&mut self, delta: i16) -> bool {
        let max = self.content_width.saturating_sub(self.viewport_width);
        let next = if delta >= 0 {
            self.h_offset.saturating_add(delta as u16).min(max)
        } else {
            self.h_offset.saturating_sub((-delta) as u16)
        };
        let changed = next != self.h_offset;
        self.h_offset = next;
        changed
    }

    /// Selects the supplied stable identity and commits it as semantic selection.
    pub fn select(&mut self, selected: Option<Id>)
    where
        Id: Clone + PartialEq,
    {
        self.cursor = selected.clone();
        self.selected = selected.clone();
        self.collection.set_active(selected);
        self.follow_selection = true;
    }

    /// Enables ordered multi-selection with an empty selection.
    pub fn enable_multi_select(&mut self) {
        self.selection.get_or_insert_with(Selection::new);
    }

    /// Typeahead buffer (roving).
    #[must_use]
    pub fn typeahead_buffer(&self) -> &str {
        self.collection.roving().typeahead_buffer()
    }

    /// Clear typeahead.
    pub fn clear_typeahead(&mut self) {
        self.collection.clear_typeahead();
    }

    /// Filter query (`/` search chrome).
    #[must_use]
    pub fn filter_query(&self) -> Option<&str> {
        self.filter_query.as_deref()
    }

    /// Set filter query (empty clears).
    pub fn set_filter_query(&mut self, query: Option<String>) {
        self.filter_query = query.filter(|q| !q.is_empty());
    }

    /// Virtual window into a larger flat list (host projects only the window).
    ///
    /// `window_start` is the logical index of the first resident row. It may
    /// equal `total_len` when the host has an empty tail window. Reasserting
    /// the same window preserves the user's current scroll offset.
    pub fn set_virtual_window(&mut self, window_start: usize, total_len: usize) {
        if total_len == 0 {
            self.offset = 0;
            self.clear_virtual_geometry();
            // Keep active focus and typeahead while dropping only the
            // projection metadata.
            self.collection.clear_virtual_window();
            return;
        }

        let window_start = window_start.min(total_len);
        let same_window =
            self.virtual_total == total_len && self.virtual_window_start == window_start;
        self.virtual_total = total_len;
        // Keep the supplied resident origin independent from the scroll
        // offset. Virtualizer::set_offset clamps against the current
        // viewport, but the host's resident ids still begin at this origin.
        self.virtual_window_start = window_start;
        self.virt.set_len(total_len as u64);
        if !same_window {
            self.virt.set_offset(self.virtual_window_start as u64);
        }
        self.offset = self.virt.offset() as usize;
    }

    /// Borrow virtualizer (anchors / overscan).
    #[must_use]
    pub const fn virtualizer(&self) -> &Virtualizer {
        &self.virt
    }

    /// Mutable virtualizer.
    pub fn virtualizer_mut(&mut self) -> &mut Virtualizer {
        &mut self.virt
    }

    /// Sticky region for virtualized trees (headers).
    ///
    /// Tree does not synthesize or pin sticky rows. Hosts must project a
    /// contiguous resident body window, with `window_start` matching its
    /// first logical row; sticky rows, if shown, are painted by the host.
    /// The sticky counts still constrain virtual scroll bounds and anchors.
    pub fn set_sticky(&mut self, sticky: StickyRegion) {
        self.virt.set_sticky(sticky);
    }

    /// Capture index anchor for preserve-across-filter.
    pub fn capture_scroll_anchor(&mut self) {
        self.virt.set_offset(self.offset as u64);
        self.virt.capture_index_anchor();
    }

    /// Restore offset from virtualizer anchor after rebuild.
    pub fn restore_scroll_anchor(&mut self) {
        if let Some(a) = self.virt.anchor().cloned() {
            self.virt.apply_anchor(&a, |_| None);
            self.offset = self.virt.offset() as usize;
        }
    }

    fn clear_virtual_geometry(&mut self) {
        self.virtual_total = 0;
        self.virtual_window_start = 0;
        self.virt.set_len(0);
        self.virt.set_offset(0);
    }

    /// Disables multi-selection and discards checked identities.
    pub fn disable_multi_select(&mut self) {
        self.selection = None;
    }

    #[must_use]
    /// Returns the ordered multi-selection state, if enabled.
    pub const fn selection(&self) -> Option<&Selection<Id>> {
        self.selection.as_ref()
    }

    /// Returns mutable access to ordered multi-selection state, if enabled.
    pub fn selection_mut(&mut self) -> Option<&mut Selection<Id>> {
        self.selection.as_mut()
    }

    fn virtual_offset_max(&self) -> usize {
        if self.virtual_total == 0 {
            return 0;
        }
        usize::try_from(self.virt.max_offset()).unwrap_or(usize::MAX)
    }

    fn clamp_virtual_offset(&mut self) {
        let maximum = self.virtual_offset_max();
        self.offset = self.offset.min(maximum);
        self.virt.set_offset(self.offset as u64);
        self.offset = self.virt.offset() as usize;
    }

    /// Moves the scroll position by a signed delta and clamps it to valid content.
    pub fn scroll_by(&mut self, delta: isize, node_count: usize) -> bool {
        let before = self.offset;
        let content_len = if self.virtual_total > 0 {
            self.virtual_total
        } else {
            node_count
        };
        let viewport_height = if self.virtual_total > 0 {
            self.viewport_height.max(1)
        } else {
            self.viewport_height
        };
        let maximum = if self.virtual_total > 0 {
            self.virtual_offset_max()
        } else {
            max_offset(content_len, viewport_height)
        };
        self.offset = if delta.is_negative() {
            self.offset.saturating_sub(delta.unsigned_abs())
        } else {
            self.offset
                .saturating_add(delta.unsigned_abs())
                .min(maximum)
        };
        self.offset = self.offset.min(maximum);
        if self.virtual_total > 0 {
            self.virt.set_offset(self.offset as u64);
            self.offset = self.virt.offset() as usize;
        }
        self.follow_selection = false;
        before != self.offset
    }

    /// Scrolls toward a pointer position within the painted viewport.
    pub fn scroll_to_position(&mut self, position: Position, node_count: usize) -> bool {
        let Some(area) = self.scrollbar_region else {
            return false;
        };
        if !area.contains(position) {
            return false;
        }
        let content_len = if self.virtual_total > 0 {
            self.virtual_total
        } else {
            node_count
        };
        self.offset = crate::scroll::offset_for_track_position(
            content_len,
            self.viewport_height,
            area.height,
            usize::from(position.y.saturating_sub(area.y)),
        );
        if self.virtual_total > 0 {
            self.virt.set_offset(self.offset as u64);
            self.offset = self.virt.offset() as usize;
        }
        self.follow_selection = false;
        true
    }

    #[must_use]
    /// Returns the hit regions produced by the most recent render.
    pub fn regions(&self) -> &[HitRegion<Id>] {
        &self.regions
    }
}

impl<Id: Clone + PartialEq> TreeState<Id> {
    fn clear_virtual_projection(&mut self) {
        self.clear_virtual_geometry();
        // Do not call set_active/reconcile here: both can discard an active
        // typeahead buffer while the host changes from a resident window to a
        // full projection.
        self.collection.clear_virtual_window();
    }

    /// Routes navigation, disclosure, checking, activation, filter, and typeahead.
    ///
    /// Prefer intents via [`crate::interaction::default_tree_intent`]. Printable
    /// characters feed typeahead. `/` opens filter query chrome.
    pub fn handle_key(&mut self, nodes: &[TreeNode<'_, Id>], key: KeyEvent) -> TreeOutcome<Id> {
        if key.is_release() {
            return TreeOutcome::Ignored;
        }
        let intent = crate::interaction::default_tree_intent(key);
        if !matches!(intent, Some(UiIntent::Cancel | UiIntent::Close)) {
            self.reconcile_projection(nodes);
        }
        // Filter mode
        if key.is_press() && matches!(key.code, KeyCode::Char('/')) && key.modifiers.is_empty() {
            if self.filter_query.is_none() {
                self.filter_query = Some(String::new());
            }
            return TreeOutcome::Ignored; // host reprojects; treat as chrome change
        }
        if self.filter_query.is_some() && key.is_press() && key.modifiers.is_empty() {
            match key.code {
                KeyCode::Backspace => {
                    if let Some(q) = self.filter_query.as_mut() {
                        q.pop();
                        if q.is_empty() {
                            self.filter_query = None;
                        }
                    }
                    return TreeOutcome::Ignored;
                }
                KeyCode::Esc => {
                    self.filter_query = None;
                    return TreeOutcome::Cancelled;
                }
                KeyCode::Char(c) if !c.is_control() && c != '/' => {
                    if let Some(q) = self.filter_query.as_mut() {
                        q.push(c);
                    }
                    return TreeOutcome::Ignored;
                }
                _ => {}
            }
        }
        if key.is_press() && key.modifiers.is_empty() {
            match key.code {
                KeyCode::Char('*') => {
                    self.collection.clear_typeahead();
                    self.bulk_disclosure = Some(true);
                    return TreeOutcome::Ignored;
                }
                KeyCode::Char('-') => {
                    self.collection.clear_typeahead();
                    self.bulk_disclosure = Some(false);
                    return TreeOutcome::Ignored;
                }
                KeyCode::Char('g') => {
                    self.collection.clear_typeahead();
                    return self.select_boundary(nodes, false);
                }
                KeyCode::Char('G') => {
                    self.collection.clear_typeahead();
                    return self.select_boundary(nodes, true);
                }
                _ => {}
            }
        }
        if let Some(intent) = intent {
            self.collection.clear_typeahead();
            return self.handle_intent_reconciled(nodes, intent);
        }
        self.handle_typeahead(nodes, key)
    }

    fn handle_typeahead(&mut self, nodes: &[TreeNode<'_, Id>], key: KeyEvent) -> TreeOutcome<Id> {
        if !key.is_press() {
            return TreeOutcome::Ignored;
        }
        let KeyCode::Char(c) = key.code else {
            return TreeOutcome::Ignored;
        };
        if c.is_control()
            || key.modifiers.contains(KeyModifiers::CONTROL)
            || key.modifiers.contains(KeyModifiers::ALT)
        {
            return TreeOutcome::Ignored;
        }
        let items = collection_items_from_nodes(nodes);
        let out = self.collection.handle_key(key, &items);
        if out.active_changed() {
            if let Some(id) = self.collection.active().cloned() {
                self.cursor = Some(id.clone());
                self.follow_selection = true;
                return TreeOutcome::SelectionChanged(id);
            }
        }
        TreeOutcome::Ignored
    }

    fn reconcile_projection(&mut self, nodes: &[TreeNode<'_, Id>]) {
        let partial = self.virtual_total > nodes.len();
        let had_cursor = self.cursor.is_some();

        if partial {
            self.clamp_virtual_offset();
        }

        if self.collection.active() != self.cursor.as_ref() {
            self.collection
                .set_active_preserving_typeahead(self.cursor.clone());
        }

        if had_cursor {
            let cursor_node = self
                .cursor
                .as_ref()
                .and_then(|cursor| nodes.iter().find(|node| &node.id == cursor));
            if cursor_node.is_some_and(|node| !node.is_interactive())
                || (!partial && cursor_node.is_none())
            {
                let repaired = nodes
                    .iter()
                    .find(|node| node.is_interactive())
                    .map(|node| node.id.clone());
                self.cursor = repaired.clone();
                self.collection.set_active_preserving_typeahead(repaired);
                self.follow_selection = self.cursor.is_some();
            }
        }

        let keep_selected = self.selected.as_ref().is_some_and(|selected| {
            match nodes.iter().find(|node| &node.id == selected) {
                Some(node) => node.is_interactive(),
                None => partial,
            }
        });
        if !keep_selected {
            self.selected = None;
        }

        if partial {
            let items = collection_items_from_nodes(nodes);
            let _ = self.collection.reconcile_window(
                &items,
                self.virtual_window_start,
                self.virtual_total,
                self.viewport_height,
            );
            if self.cursor.is_none() {
                self.collection.set_active_preserving_typeahead(None);
            }
        } else {
            let had_virtual_projection = self.virtual_total != 0 || self.virtual_window_start != 0;
            if had_virtual_projection {
                self.clear_virtual_projection();
            } else {
                self.collection
                    .set_viewport(self.offset, self.viewport_height, nodes.len());
            }
        }
    }

    /// Routes a semantic intent (keymap / scene adapter).
    ///
    /// **Left (Collapse):** if expanded branch → toggle collapse; else move to parent.  
    /// **Right (Expand):** if collapsed/lazy branch → toggle expand/load; else if
    /// expanded with visible children → select first child; else ignored.
    pub fn handle_intent(
        &mut self,
        nodes: &[TreeNode<'_, Id>],
        intent: UiIntent,
    ) -> TreeOutcome<Id> {
        if !matches!(intent, UiIntent::Cancel | UiIntent::Close) {
            self.reconcile_projection(nodes);
        }
        self.handle_intent_reconciled(nodes, intent)
    }

    fn handle_intent_reconciled(
        &mut self,
        nodes: &[TreeNode<'_, Id>],
        intent: UiIntent,
    ) -> TreeOutcome<Id> {
        match intent {
            UiIntent::Move(NavigationMove::Previous) => self.move_selection(nodes, -1),
            UiIntent::Move(NavigationMove::Next) => self.move_selection(nodes, 1),
            UiIntent::Move(NavigationMove::First) => self.select_boundary(nodes, false),
            UiIntent::Move(NavigationMove::Last) => self.select_boundary(nodes, true),
            UiIntent::Page(PageMove::Backward) => self.page_selection(nodes, false),
            UiIntent::Page(PageMove::Forward) => self.page_selection(nodes, true),
            UiIntent::Collapse => self.collapse_or_parent(nodes),
            UiIntent::Expand => self.expand_or_enter(nodes),
            UiIntent::Activate => self.activate_or_toggle(nodes),
            UiIntent::Toggle => {
                if self.selection.is_some() {
                    self.toggle_selected(nodes)
                } else if self.cursor_node(nodes).is_some_and(|node| node.branch) {
                    self.activate_or_toggle(nodes)
                } else {
                    TreeOutcome::Ignored
                }
            }
            UiIntent::Cancel | UiIntent::Close => TreeOutcome::Cancelled,
            _ => TreeOutcome::Ignored,
        }
    }

    fn activate_or_toggle(&mut self, nodes: &[TreeNode<'_, Id>]) -> TreeOutcome<Id> {
        let Some(node) = self.cursor_node(nodes) else {
            return TreeOutcome::Ignored;
        };
        if node.branch {
            TreeOutcome::Toggle(node.id.clone())
        } else {
            self.selected = Some(node.id.clone());
            TreeOutcome::Activated(node.id.clone())
        }
    }

    fn toggle_selected(&mut self, nodes: &[TreeNode<'_, Id>]) -> TreeOutcome<Id> {
        let Some(selection) = self.selection.as_mut() else {
            return TreeOutcome::Ignored;
        };
        let Some(node) = self.cursor.as_ref().and_then(|cursor| {
            nodes
                .iter()
                .find(|node| node.is_interactive() && &node.id == cursor)
        }) else {
            return TreeOutcome::Ignored;
        };
        selection.toggle(&node.id);
        TreeOutcome::CheckToggled(node.id.clone())
    }

    /// Updates hover state from the current pointer position and painted hit regions.
    pub fn hover(&mut self, position: Position) -> Option<&Id> {
        self.hovered = self
            .regions
            .iter()
            .find(|region| region.area.contains(position))
            .map(|region| region.id.clone());
        self.hovered.as_ref()
    }

    fn focus_pointer(&mut self, id: Id) {
        self.collection.set_active(Some(id.clone()));
        self.cursor = Some(id);
        self.follow_selection = true;
    }

    /// Maps a pointer position to the semantic outcome of the painted hit region.
    pub fn click(&mut self, position: Position) -> TreeOutcome<Id> {
        if let Some(id) = self
            .disclosure_regions
            .iter()
            .find(|region| region.area.contains(position))
            .map(|region| region.id.clone())
        {
            self.focus_pointer(id.clone());
            return TreeOutcome::Toggle(id);
        }
        if let Some(id) = self
            .check_regions
            .iter()
            .find(|region| region.area.contains(position))
            .map(|region| region.id.clone())
        {
            self.focus_pointer(id.clone());
            if let Some(selection) = self.selection.as_mut() {
                selection.toggle(&id);
                return TreeOutcome::CheckToggled(id);
            }
        }
        let Some(id) = self
            .regions
            .iter()
            .find(|region| region.area.contains(position))
            .map(|region| region.id.clone())
        else {
            return TreeOutcome::Ignored;
        };
        let is_branch = self.disclosure_regions.iter().any(|region| region.id == id);
        if is_branch {
            self.focus_pointer(id.clone());
            return TreeOutcome::Toggle(id);
        }
        if self.cursor.as_ref() == Some(&id) {
            self.selected = Some(id.clone());
            TreeOutcome::Activated(id)
        } else {
            self.focus_pointer(id.clone());
            TreeOutcome::SelectionChanged(id)
        }
    }

    fn cursor_index(&self, nodes: &[TreeNode<'_, Id>]) -> Option<usize> {
        let cursor = self.cursor.as_ref()?;
        nodes.iter().position(|node| &node.id == cursor)
    }

    fn cursor_node<'a>(&self, nodes: &'a [TreeNode<'_, Id>]) -> Option<&'a TreeNode<'a, Id>> {
        let index = self.cursor_index(nodes)?;
        nodes.get(index).filter(|node| node.is_interactive())
    }

    fn move_selection(&mut self, nodes: &[TreeNode<'_, Id>], delta: i32) -> TreeOutcome<Id> {
        if self.cursor.is_none() {
            return self.select_boundary(nodes, delta < 0);
        }
        let start = self
            .cursor_index(nodes)
            .unwrap_or(if delta < 0 { nodes.len() } else { 0 });
        if self.virtual_total > nodes.len()
            && self.cursor.is_some()
            && self.cursor_index(nodes).is_none()
        {
            // The host only supplied a virtual slice. Relative movement cannot
            // infer the off-window cursor's adjacent row safely.
            return TreeOutcome::Ignored;
        }
        let candidate = if delta < 0 {
            nodes[..start]
                .iter()
                .rposition(|node| node.is_interactive())
        } else {
            nodes
                .iter()
                .enumerate()
                .skip(start.saturating_add(1))
                .find(|(_, node)| node.is_interactive())
                .map(|(index, _)| index)
        };
        self.set_cursor_index(nodes, candidate)
    }

    fn page_selection(&mut self, nodes: &[TreeNode<'_, Id>], forward: bool) -> TreeOutcome<Id> {
        if self.cursor.is_none() {
            return self.select_boundary(nodes, !forward);
        }
        let Some(start) = self.cursor_index(nodes) else {
            return TreeOutcome::Ignored;
        };
        let distance = self.viewport_height.max(1);
        let target = if forward {
            start
                .saturating_add(distance)
                .min(nodes.len().saturating_sub(1))
        } else {
            start.saturating_sub(distance)
        };
        let candidate = if forward {
            nodes
                .iter()
                .enumerate()
                .skip(target)
                .find(|(_, node)| node.is_interactive())
                .map(|(index, _)| index)
                .or_else(|| {
                    nodes[..target]
                        .iter()
                        .rposition(|node| node.is_interactive())
                })
        } else {
            nodes[..=target]
                .iter()
                .rposition(|node| node.is_interactive())
                .or_else(|| {
                    nodes
                        .iter()
                        .enumerate()
                        .skip(target.saturating_add(1))
                        .find(|(_, node)| node.is_interactive())
                        .map(|(index, _)| index)
                })
        };
        self.set_cursor_index(nodes, candidate)
    }

    fn select_boundary(&mut self, nodes: &[TreeNode<'_, Id>], from_end: bool) -> TreeOutcome<Id> {
        let candidate = if from_end {
            nodes.iter().rposition(|node| node.is_interactive())
        } else {
            nodes.iter().position(|node| node.is_interactive())
        };
        self.set_cursor_index(nodes, candidate)
    }

    fn set_cursor_index(
        &mut self,
        nodes: &[TreeNode<'_, Id>],
        index: Option<usize>,
    ) -> TreeOutcome<Id> {
        let Some(node) = index.and_then(|index| nodes.get(index)) else {
            return TreeOutcome::Ignored;
        };
        self.cursor = Some(node.id.clone());
        self.collection.set_active(Some(node.id.clone()));
        self.follow_selection = true;
        TreeOutcome::SelectionChanged(node.id.clone())
    }

    /// Left: collapse expanded branch, else select parent.
    fn collapse_or_parent(&mut self, nodes: &[TreeNode<'_, Id>]) -> TreeOutcome<Id> {
        let Some(index) = self.cursor_index(nodes) else {
            return TreeOutcome::Ignored;
        };
        let node = &nodes[index];
        if node.is_interactive() && node.branch && node.expanded {
            return TreeOutcome::Toggle(node.id.clone());
        }
        // Prefer explicit parent id when present.
        if let Some(ref pid) = node.parent {
            if let Some(pidx) = nodes
                .iter()
                .position(|n| n.is_interactive() && &n.id == pid)
            {
                return self.set_cursor_index(nodes, Some(pidx));
            }
        }
        let parent = nodes[..index]
            .iter()
            .rposition(|candidate| candidate.is_interactive() && candidate.depth < node.depth);
        self.set_cursor_index(nodes, parent)
    }

    /// Right: expand collapsed/lazy branch, else enter first visible child.
    fn expand_or_enter(&mut self, nodes: &[TreeNode<'_, Id>]) -> TreeOutcome<Id> {
        let Some(index) = self.cursor_index(nodes) else {
            return TreeOutcome::Ignored;
        };
        let node = &nodes[index];
        if !node.is_interactive() {
            return TreeOutcome::Ignored;
        }
        // Lazy or collapsed branch → request expand/load.
        if node.branch && (!node.expanded || matches!(node.status, TreeNodeStatus::Lazy)) {
            return TreeOutcome::Toggle(node.id.clone());
        }
        // Expanded: move to first enabled child in the flat projection.
        if node.branch && node.expanded {
            let child_depth = node.depth.saturating_add(1);
            let child = nodes
                .iter()
                .enumerate()
                .skip(index.saturating_add(1))
                .take_while(|(_, n)| n.depth >= child_depth)
                .find(|(_, n)| n.is_interactive() && n.depth == child_depth)
                .map(|(i, _)| i);
            return self.set_cursor_index(nodes, child);
        }
        TreeOutcome::Ignored
    }
}

fn collection_items_from_nodes<Id: Clone>(nodes: &[TreeNode<'_, Id>]) -> Vec<CollectionItem<Id>> {
    nodes
        .iter()
        .filter(|n| n.is_interactive())
        .map(|n| CollectionItem {
            id: n.id.clone(),
            enabled: true,
            label: n.plain_label(),
            parent: None,
        })
        .collect()
}

#[derive(Debug, Clone, Copy)]
/// A navigable hierarchical list with disclosure and multi-select support.
///
/// Consumer owns the flattened projection and expansion policy. Tree owns
/// selection, scroll, hover, multi-check, hit geometry, and typed outcomes.
pub struct Tree<'a, Id> {
    /// Host-supplied: surface owns keyboard focus this frame.
    focused: bool,
    nodes: &'a [TreeNode<'a, Id>],
    tokens: &'a DesignSystem,
    empty_message: Option<&'a str>,
    /// Spinner frame index for loading disclosure (junie `spinner_frame(tick)`).
    spinner_frame: usize,
    /// Surface beneath each row; defaults to the Junie surface plane.
    background: Option<Color>,
    /// Whether the active identity is painted as a semantic selection.
    selection_visible: bool,
    /// Whether focused-row metadata carries the row's weight cue.
    focused_metadata_bold: bool,
}

impl<'a, Id> Tree<'a, Id> {
    #[must_use]
    /// Creates a tree over borrowed flattened nodes and mutable tree state.
    pub const fn new(nodes: &'a [TreeNode<'a, Id>], tokens: &'a DesignSystem) -> Self {
        Self {
            focused: true,
            nodes,
            tokens,
            empty_message: None,
            spinner_frame: 0,
            background: None,
            selection_visible: true,
            focused_metadata_bold: false,
        }
    }

    /// Whether this surface owns keyboard focus this frame (host / scene).
    #[must_use]
    pub const fn focused(mut self, focused: bool) -> Self {
        self.focused = focused;
        self
    }

    /// Surface beneath rows, matching the host panel's fill.
    #[must_use]
    pub const fn background(mut self, background: Color) -> Self {
        self.background = Some(background);
        self
    }

    /// Controls whether navigation state receives selection paint.
    #[must_use]
    pub const fn selection_visible(mut self, visible: bool) -> Self {
        self.selection_visible = visible;
        self
    }

    /// Applies the focused-row weight to right-aligned metadata.
    #[must_use]
    pub const fn focused_metadata_bold(mut self, bold: bool) -> Self {
        self.focused_metadata_bold = bold;
        self
    }

    /// Message painted when `nodes` is empty.
    #[must_use]
    pub const fn empty_message(mut self, message: &'a str) -> Self {
        self.empty_message = Some(message);
        self
    }

    /// Busy-disclosure spinner frame (modulo the braille vocabulary).
    #[must_use]
    pub const fn spinner_frame(mut self, frame: usize) -> Self {
        self.spinner_frame = frame;
        self
    }

    /// Design tokens used for recipes and glyphs.
    #[must_use]
    pub const fn tokens(&self) -> &DesignSystem {
        self.tokens
    }
}

fn with_row_wash(style: Style, wash: Option<ratatui_core::style::Color>) -> Style {
    wash.map_or(style, |bg| style.bg(bg))
}

fn paint_tree_row<Id: Clone + PartialEq>(
    tokens: &DesignSystem,
    ground: Color,
    selection_visible: bool,
    focused: bool,
    focused_metadata_bold: bool,
    spinner_frame: usize,
    node: &TreeNode<'_, Id>,
    row: Rect,
    buffer: &mut Buffer,
    state: &mut TreeState<Id>,
    indent_step: u16,
) {
    if row.width == 0 {
        return;
    }
    let interactive = node.is_interactive();
    let selected = selection_visible && interactive && state.selected.as_ref() == Some(&node.id);
    let hovered = state.hovered.as_ref() == Some(&node.id);
    let checked = state
        .selection
        .as_ref()
        .is_some_and(|selection| selection.is_checked(&node.id));
    let busy = matches!(node.status, TreeNodeStatus::Loading);
    let row_focused = focused && state.cursor.as_ref() == Some(&node.id) && interactive;
    let visual = ListRowVisualState {
        selected,
        focused: row_focused,
        hovered: hovered && interactive,
        enabled: node.enabled,
        loading: busy,
        checked,
        error: matches!(node.status, TreeNodeStatus::Error),
        ..ListRowVisualState::default()
    };
    let chrome = RowChrome::resolve_on(tokens, visual, ground);
    let recipe = tokens.resolve_list_row_on(visual, ground);
    let mut body = match node.status {
        TreeNodeStatus::Ready if interactive => recipe.label.patch(tokens.style(node.tone.role())),
        TreeNodeStatus::Ready => tokens.style(Role::TextDisabled),
        TreeNodeStatus::Loading | TreeNodeStatus::Lazy => tokens.style(Role::TextSecondary),
        TreeNodeStatus::Error => tokens.style(Role::Danger),
    };
    if !node.enabled {
        body = tokens.style(Role::TextDisabled);
    }
    let body = chrome.label_style(body);
    buffer.set_style(row, body);
    chrome.paint_wash(buffer, row);
    chrome.paint_gutter(buffer, row);

    let y = row.y;
    let max_indent = row.width.saturating_sub(4);
    let indent = node.depth.saturating_mul(indent_step).min(max_indent);
    let mut x = row.x.saturating_add(1).saturating_add(indent);
    let wash = chrome.wash();
    if x.saturating_add(2) > row.right() {
        if interactive {
            state.regions.push(HitRegion {
                id: node.id.clone(),
                area: row,
            });
            if node.branch {
                state.disclosure_regions.push(HitRegion {
                    id: node.id.clone(),
                    area: Rect::new(row.x, y, 1, 1),
                });
            }
        }
        return;
    }

    let (glyph, glyph_style) = if busy {
        let frames = SPINNER_BRAILLE_FRAMES;
        let frame = frames[spinner_frame % frames.len()];
        (frame, with_row_wash(tokens.style(Role::Accent), wash))
    } else if node.branch {
        let glyph = if node.expanded {
            tokens.glyphs.disclosure_open()
        } else {
            tokens.glyphs.disclosure_closed()
        };
        (
            glyph,
            with_row_wash(tokens.style(Role::TextSecondary), wash),
        )
    } else {
        (" ", body)
    };
    buffer.set_stringn(x, y, glyph, 1, glyph_style);
    let disclosure_x = x;
    let disclosure_w = 2.min(row.right().saturating_sub(x));
    x = x.saturating_add(2);

    if state.selection.is_some() && x < row.right() {
        let marker = if checked {
            Glyph::Success.resolve().text
        } else {
            " "
        };
        let gw = u16::try_from(display_cols(marker)).unwrap_or(1).max(1);
        let paint_w = gw.min(row.right().saturating_sub(x));
        if paint_w > 0 {
            buffer.set_stringn(x, y, marker, usize::from(paint_w), body);
            if interactive {
                state.check_regions.push(HitRegion {
                    id: node.id.clone(),
                    area: Rect::new(x, y, paint_w, 1),
                });
            }
            x = x.saturating_add(paint_w);
            if x < row.right() {
                x = x.saturating_add(1);
            }
        }
    }

    if let Some(leading) = node.leading.as_ref()
        && x < row.right()
    {
        let lw = u16::try_from(leading.width())
            .unwrap_or(1)
            .min(row.right().saturating_sub(x))
            .max(1)
            .min(2);
        let muted = chrome
            .secondary_style(tokens.style(Role::TextMuted))
            .remove_modifier(Modifier::BOLD);
        buffer.set_line(x, y, leading, lw);
        buffer.set_style(Rect::new(x, y, lw, 1), muted);
        x = x.saturating_add(2);
    }

    let status = match node.status {
        TreeNodeStatus::Ready => None,
        TreeNodeStatus::Loading => Some(" loading"),
        TreeNodeStatus::Lazy => Some(" lazy"),
        TreeNodeStatus::Error => Some(" error"),
    };
    let status_w = status
        .map(display_cols)
        .and_then(|width| u16::try_from(width).ok())
        .unwrap_or(0);
    let badge = node.badge.as_ref();
    let raw_meta_w = badge
        .map(|meta| u16::try_from(meta.width()).unwrap_or(0))
        .unwrap_or(0);
    let avail = row.right().saturating_sub(x);
    // Hide meta rather than starve the label below one identity cell.
    let label_w = u16::try_from(node.label.width()).unwrap_or(u16::MAX);
    let meta_w = if raw_meta_w > 0 && avail.saturating_sub(raw_meta_w.saturating_add(2)) >= label_w
    {
        raw_meta_w
    } else {
        0
    };
    let shortcut_need = node
        .shortcut
        .map(|s| {
            u16::try_from(display_cols(s))
                .unwrap_or(0)
                .saturating_add(1)
        })
        .unwrap_or(0);
    let actions_need = node
        .actions
        .as_ref()
        .map(|a| u16::try_from(a.width()).unwrap_or(0).saturating_add(1))
        .unwrap_or(0);
    let secondary_need = node
        .secondary
        .as_ref()
        .map(|s| u16::try_from(s.width()).unwrap_or(0).saturating_add(1))
        .unwrap_or(0);

    let mut budget = avail
        .saturating_sub(status_w)
        .saturating_sub(if meta_w > 0 {
            meta_w.saturating_add(1)
        } else {
            0
        })
        .saturating_sub(1);
    let show_shortcut = node.shortcut.is_some() && budget >= shortcut_need;
    if show_shortcut {
        budget = budget.saturating_sub(shortcut_need);
    }
    let show_actions = node.actions.is_some() && budget >= actions_need;
    if show_actions {
        budget = budget.saturating_sub(actions_need);
    }
    let show_secondary = node.secondary.is_some() && budget >= secondary_need;

    let shortcut_w = if show_shortcut {
        node.shortcut
            .map(|s| u16::try_from(display_cols(s)).unwrap_or(0))
            .unwrap_or(0)
    } else {
        0
    };
    let actions_w = if show_actions {
        node.actions
            .as_ref()
            .map(|a| u16::try_from(a.width()).unwrap_or(0))
            .unwrap_or(0)
    } else {
        0
    };
    let extras = shortcut_w
        .saturating_add(actions_w)
        .saturating_add(u16::from(shortcut_w > 0 && actions_w > 0));
    let label_right = row
        .right()
        .saturating_sub(status_w)
        .saturating_sub(if meta_w > 0 {
            meta_w.saturating_add(1)
        } else {
            0
        })
        .saturating_sub(extras)
        .saturating_sub(u16::from(meta_w == 0));
    let label_w = label_right.saturating_sub(x);

    let mut label_style = if selected {
        chrome.label_style(tokens.style(Role::Accent))
    } else {
        body
    };
    if !interactive {
        label_style = chrome.label_style(tokens.style(Role::TextDisabled));
    }

    if label_w > 0 && x < row.right() {
        // A selected tree identity owns its label field, including padding
        // before right-aligned metadata. Keep the separator cell unstyled so
        // the metadata gap matches the source frame.
        let label_style_w = label_w.saturating_sub(u16::from(meta_w > 0));
        buffer.set_style(Rect::new(x, y, label_style_w, 1), label_style);
        if state.h_offset == 0 {
            buffer.set_line(x, y, &node.label, label_w);
            let painted = u16::try_from(node.label.width())
                .unwrap_or(u16::MAX)
                .min(label_w);
            x = x.saturating_add(painted);
        } else {
            let mut visible = String::new();
            display_cols_slice_into(
                &node.plain_label(),
                usize::from(state.h_offset),
                usize::from(label_w),
                &mut visible,
            );
            buffer.set_stringn(x, y, &visible, usize::from(label_w), label_style);
            x = x.saturating_add(
                u16::try_from(display_cols(&visible))
                    .unwrap_or(label_w)
                    .min(label_w),
            );
        }
    }

    if show_secondary && let Some(secondary) = node.secondary.as_ref() {
        let avail = label_right.saturating_sub(x);
        if avail > 2 {
            x = x.saturating_add(1);
            let sw = u16::try_from(secondary.width())
                .unwrap_or(u16::MAX)
                .min(label_right.saturating_sub(x));
            if sw > 0 {
                buffer.set_line(x, y, secondary, sw);
                let mut style = chrome.secondary_style(recipe.secondary);
                if row_focused && focused_metadata_bold {
                    style = style.add_modifier(Modifier::BOLD);
                }
                buffer.set_style(Rect::new(x, y, sw, 1), style);
            }
        }
    }

    let mut cursor = row.right().saturating_sub(status_w);
    if meta_w > 0
        && let Some(badge) = badge
    {
        cursor = cursor.saturating_sub(meta_w.saturating_add(1));
        buffer.set_line(cursor, y, badge, meta_w);
        let mut style = chrome.secondary_style(tokens.style(Role::TextMuted));
        if row_focused && focused_metadata_bold {
            style = style.add_modifier(Modifier::BOLD);
        }
        buffer.set_style(Rect::new(cursor, y, meta_w, 1), style);
    }
    if show_actions && let Some(act) = node.actions.as_ref() {
        let w = actions_w.min(cursor.saturating_sub(row.x));
        if w > 0 {
            cursor = cursor.saturating_sub(w.saturating_add(1));
            buffer.set_line(cursor, y, act, w);
            buffer.set_style(Rect::new(cursor, y, w, 1), recipe.shortcut);
        }
    }
    if show_shortcut && let Some(shortcut) = node.shortcut {
        let w = shortcut_w.min(cursor.saturating_sub(row.x));
        if w > 0 {
            cursor = cursor.saturating_sub(w.saturating_add(1));
            buffer.set_stringn(cursor, y, shortcut, usize::from(w), recipe.shortcut);
        }
    }
    if let Some(status) = status
        && status_w > 0
    {
        buffer.set_stringn(
            row.right().saturating_sub(status_w),
            y,
            status,
            usize::from(status_w),
            body,
        );
    }

    if interactive {
        state.regions.push(HitRegion {
            id: node.id.clone(),
            area: row,
        });
        if node.branch {
            state.disclosure_regions.push(HitRegion {
                id: node.id.clone(),
                area: Rect::new(disclosure_x, y, disclosure_w.max(1), 1),
            });
        }
    }
}

/// Converts a logical scrollbar geometry to the legacy `u16` painter range.
///
/// Tree offsets are `usize` because virtualized collections can be very large.
/// Scale the complete geometry before narrowing so a large offset keeps its
/// position on the track instead of saturating at `u16::MAX`.
fn scrollbar_geometry(total: usize, viewport: usize, offset: usize) -> (usize, usize, u16) {
    let max = usize::from(u16::MAX);
    if total <= max {
        let max_offset = total.saturating_sub(viewport.min(total));
        return (total, viewport, offset.min(max_offset) as u16);
    }

    let scaled_total = max;
    let scaled_viewport = ((viewport as u128 * max as u128) / total as u128)
        .max(1)
        .min(max as u128) as usize;
    let scaled_offset = ((offset as u128 * max as u128) / total as u128)
        .min(scaled_total.saturating_sub(scaled_viewport) as u128) as usize;
    (scaled_total, scaled_viewport, scaled_offset as u16)
}

impl<Id: Clone + PartialEq> StatefulWidget for &Tree<'_, Id> {
    type State = TreeState<Id>;

    fn render(self, area: Rect, buffer: &mut Buffer, state: &mut Self::State) {
        state.regions.clear();
        state.disclosure_regions.clear();
        state.check_regions.clear();
        state.scrollbar_region = None;
        if area.is_empty() {
            if state.virtual_total > 0 {
                state.clamp_virtual_offset();
            } else {
                state.offset = 0;
            }
            state.viewport_height = 0;
            state.reconcile_projection(self.nodes);
            state.hovered = None;
            return;
        }
        // Filter chrome strip
        let mut body_y = area.y;
        let mut body_h = area.height;
        if let Some(q) = state.filter_query.as_ref() {
            let strip = format!("/ {q}");
            buffer.set_stringn(
                area.x,
                area.y,
                &take_display_cols(&strip, usize::from(area.width)),
                usize::from(area.width),
                self.tokens.style(Role::TextSecondary),
            );
            body_y = area.y.saturating_add(1);
            body_h = area.height.saturating_sub(1);
        }
        let body = Rect::new(area.x, body_y, area.width, body_h);
        let ground = self
            .background
            .unwrap_or_else(|| self.tokens.junie_theme().surface);
        state.viewport_height = usize::from(body.height);
        state.virt.set_viewport_extent(body.height.max(1));
        state.reconcile_projection(self.nodes);
        if body.is_empty() {
            state.hovered = None;
            return;
        }
        if self.nodes.is_empty() {
            if state.virtual_total > 0 {
                state.clamp_virtual_offset();
            } else {
                state.offset = 0;
            }
            state.hovered = None;
            if let Some(message) = self.empty_message {
                let style = self.tokens.style(Role::TextMuted);
                buffer.set_stringn(body.x, body.y, message, usize::from(body.width), style);
            }
            return;
        }

        let virtualized = state.virtual_total > 0;
        let content_len = if virtualized {
            state.virtual_total
        } else {
            self.nodes.len()
        };
        if virtualized {
            state.virt.set_len(content_len as u64);
        }

        if state.follow_selection
            && let Some(selected) = state.cursor_index(self.nodes)
        {
            let selected = if virtualized {
                selected.saturating_add(state.virtual_window_start)
            } else {
                selected
            };
            if selected < state.offset {
                state.offset = selected;
            } else if selected >= state.offset.saturating_add(usize::from(body.height)) {
                state.offset = selected.saturating_sub(usize::from(body.height).saturating_sub(1));
            }
        }
        state.follow_selection = false;
        state.offset = state.offset.min(if virtualized {
            state.virtual_offset_max()
        } else {
            max_offset(content_len, usize::from(body.height))
        });
        if virtualized {
            state.virt.set_offset(state.offset as u64);
            state.offset = state.virt.offset() as usize;
        }
        let show_scrollbar =
            crate::scroll::is_scrollable(content_len, usize::from(body.height)) && body.width > 1;
        let content_area = Rect {
            x: body.x,
            y: body.y,
            width: body.width.saturating_sub(u16::from(show_scrollbar)),
            height: body.height,
        };
        let paint_offset = if state.virtual_total > 0 {
            // `offset` is absolute in the full logical list; resident nodes
            // begin at `virtual_window_start`.
            state.offset.saturating_sub(state.virtual_window_start)
        } else {
            state.offset
        };
        state.content_width = self
            .nodes
            .iter()
            .skip(paint_offset)
            .take(usize::from(body.height))
            .map(|node| u16::try_from(node.label.width()).unwrap_or(u16::MAX))
            .max()
            .unwrap_or(0);
        state.viewport_width = content_area.width.saturating_sub(3);
        state.h_offset = state
            .h_offset
            .min(state.content_width.saturating_sub(state.viewport_width));
        let indent_step = self.tokens.spacing.tree_indent.max(1);
        for (index, node) in self
            .nodes
            .iter()
            .skip(paint_offset)
            .take(usize::from(body.height))
            .enumerate()
        {
            let y = body
                .y
                .saturating_add(u16::try_from(index).unwrap_or(u16::MAX));
            let row = Rect::new(content_area.x, y, content_area.width, 1);
            paint_tree_row(
                self.tokens,
                ground,
                self.selection_visible,
                self.focused,
                self.focused_metadata_bold,
                self.spinner_frame,
                node,
                row,
                buffer,
                state,
                indent_step,
            );
        }

        if state
            .hovered
            .as_ref()
            .is_some_and(|hovered| !state.regions.iter().any(|region| &region.id == hovered))
        {
            state.hovered = None;
        }

        if show_scrollbar {
            let scrollbar = Rect::new(body.right().saturating_sub(1), body.y, 1, body.height);
            state.scrollbar_region = Some(scrollbar);
            let (scrollbar_total, scrollbar_viewport, scrollbar_offset) =
                scrollbar_geometry(content_len, usize::from(body.height), state.offset);
            crate::scroll::paint_overflow_scrollbar(
                buffer,
                scrollbar,
                scrollbar_total,
                scrollbar_viewport,
                scrollbar_offset,
                self.focused,
                self.tokens,
            );
        }
    }
}

impl<Id: Clone + PartialEq> StatefulWidget for Tree<'_, Id> {
    type State = TreeState<Id>;

    fn render(self, area: Rect, buffer: &mut Buffer, state: &mut Self::State) {
        <&Self as StatefulWidget>::render(&self, area, buffer, state);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::input::{KeyCode, KeyEvent, KeyModifiers};
    use crate::interaction::{NavigationMove, UiIntent};
    use crate::style::{DesignSystem, Role};

    fn sample() -> Vec<TreeNode<'static, &'static str>> {
        vec![
            TreeNode::new("root", Line::from("Workspace"), 0)
                .branch()
                .expanded(),
            TreeNode::new("loading", Line::from("Loading child"), 1)
                .branch()
                .loading(),
            TreeNode::new("leaf", Line::from("Wide 🧪"), 1)
                .secondary(Line::from("meta"))
                .badge(Line::from("ok")),
            TreeNode::new("err", Line::from("Broken"), 1).error(),
        ]
    }

    #[test]
    fn handle_intent_expands_collapses_and_cancels() {
        let nodes = sample();
        let mut state = TreeState::new(Some("root"));
        assert_eq!(
            state.handle_intent(&nodes, UiIntent::Collapse),
            TreeOutcome::Toggle("root")
        );
        // collapsed root: Collapse walks to parent (none) → Ignored or stays
        let mut collapsed = nodes.clone();
        collapsed[0].expanded = false;
        assert_eq!(
            state.handle_intent(&collapsed, UiIntent::Expand),
            TreeOutcome::Toggle("root")
        );
        assert_eq!(
            state.handle_intent(&nodes, UiIntent::Cancel),
            TreeOutcome::Cancelled
        );
        assert_eq!(
            state.handle_intent(&nodes, UiIntent::Move(NavigationMove::Next)),
            TreeOutcome::SelectionChanged("leaf")
        );
    }

    #[test]
    fn empty_message_and_canonical_constructor() {
        let system = DesignSystem::junie();
        let nodes: [TreeNode<'_, &str>; 0] = [];
        let mut state = TreeState::<&str>::default();
        let area = Rect::new(0, 0, 24, 2);
        let mut buffer = Buffer::empty(area);
        let tree = Tree::new(&nodes, &system).empty_message("No files");
        tree.render(area, &mut buffer, &mut state);
        assert_eq!(buffer[(0, 0)].symbol(), "N");
    }

    #[test]
    fn keyboard_skips_loading_disabled() {
        let nodes = sample();
        let mut state = TreeState::new(Some("root"));
        assert_eq!(
            state.handle_key(&nodes, KeyEvent::new(KeyCode::Down, KeyModifiers::NONE)),
            TreeOutcome::SelectionChanged("leaf")
        );
    }

    #[test]
    fn stress_visible_only() {
        let nodes: Vec<TreeNode<'_, usize>> = (0..5_000)
            .map(|i| TreeNode::new(i, Line::from(format!("n{i}")), (i % 5) as u16))
            .collect();
        let tokens = DesignSystem::default();
        let mut state = TreeState::new(Some(4_900));
        let area = Rect::new(0, 0, 40, 15);
        let mut buffer = Buffer::empty(area);
        (&Tree::new(&nodes, &tokens)).render(area, &mut buffer, &mut state);
        assert_eq!(state.regions().len(), 15);
    }

    #[test]
    fn right_expands_then_enters_child() {
        let mut nodes = vec![
            TreeNode::new("root", Line::from("Root"), 0).branch(),
            TreeNode::new("child", Line::from("Child"), 1).parent("root"),
        ];
        let mut state = TreeState::new(Some("root"));
        // collapsed → expand
        assert_eq!(
            state.handle_intent(&nodes, UiIntent::Expand),
            TreeOutcome::Toggle("root")
        );
        nodes[0].expanded = true;
        // expanded → first child
        assert_eq!(
            state.handle_intent(&nodes, UiIntent::Expand),
            TreeOutcome::SelectionChanged("child")
        );
    }

    #[test]
    fn left_collapses_or_parents() {
        let nodes = vec![
            TreeNode::new("root", Line::from("Root"), 0)
                .branch()
                .expanded(),
            TreeNode::new("child", Line::from("Child"), 1).parent("root"),
        ];
        let mut state = TreeState::new(Some("child"));
        assert_eq!(
            state.handle_intent(&nodes, UiIntent::Collapse),
            TreeOutcome::SelectionChanged("root")
        );
        assert_eq!(
            state.handle_intent(&nodes, UiIntent::Collapse),
            TreeOutcome::Toggle("root")
        );
    }

    #[test]
    fn lazy_branch_expand_requests_toggle() {
        let nodes = [TreeNode::new("lazy", Line::from("Dir"), 0).lazy_branch()];
        let mut state = TreeState::new(Some("lazy"));
        assert_eq!(
            state.handle_intent(&nodes, UiIntent::Expand),
            TreeOutcome::Toggle("lazy")
        );
    }

    #[test]
    fn typeahead_jumps_to_label() {
        let nodes = [
            TreeNode::new("a", Line::from("Alpha"), 0),
            TreeNode::new("b", Line::from("Beta"), 0),
            TreeNode::new("c", Line::from("Charlie"), 0),
        ];
        let mut state = TreeState::new(Some("a"));
        assert_eq!(
            state.handle_key(
                &nodes,
                KeyEvent::new(KeyCode::Char('b'), KeyModifiers::NONE)
            ),
            TreeOutcome::SelectionChanged("b")
        );
    }

    #[test]
    fn typeahead_buffer_survives_projection_reconciliation() {
        let nodes = [
            TreeNode::new("a", Line::from("Alpha"), 0),
            TreeNode::new("b", Line::from("Beta"), 0),
            TreeNode::new("c", Line::from("Bravo"), 0),
        ];
        let mut state = TreeState::new(Some("a"));

        assert_eq!(
            state.handle_key(
                &nodes,
                KeyEvent::new(KeyCode::Char('b'), KeyModifiers::NONE)
            ),
            TreeOutcome::SelectionChanged("b")
        );
        assert_eq!(
            state.handle_key(
                &nodes,
                KeyEvent::new(KeyCode::Char('r'), KeyModifiers::NONE)
            ),
            TreeOutcome::SelectionChanged("c")
        );
    }

    #[test]
    fn pointer_focus_syncs_typeahead_after_leaf_click() {
        let tokens = DesignSystem::junie();
        let nodes = [
            TreeNode::new("a", Line::from("Alpha"), 0),
            TreeNode::new("b", Line::from("Beta"), 0),
            TreeNode::new("c", Line::from("Bravo"), 0),
        ];
        let area = Rect::new(0, 0, 24, 3);
        let mut state = TreeState::new(Some("a"));
        let mut buffer = Buffer::empty(area);
        Tree::new(&nodes, &tokens).render(area, &mut buffer, &mut state);

        assert_eq!(
            state.click(Position::new(4, 1)),
            TreeOutcome::SelectionChanged("b")
        );
        assert_eq!(state.cursor(), Some(&"b"));
        assert_eq!(
            state.handle_key(
                &nodes,
                KeyEvent::new(KeyCode::Char('b'), KeyModifiers::NONE)
            ),
            TreeOutcome::SelectionChanged("c")
        );
    }

    #[test]
    fn loading_status_uses_one_interaction_predicate() {
        let tokens = DesignSystem::junie();
        let nodes = [
            TreeNode::new("loading", Line::from("Loading"), 0).with_status(TreeNodeStatus::Loading),
            TreeNode::new("ready", Line::from("Ready"), 0),
        ];
        assert!(nodes[0].enabled);
        assert!(!nodes[0].is_interactive());
        assert!(nodes[1].is_interactive());
        assert!(!nodes[0].composed().enabled);

        let mut keyboard = TreeState::new(Some("ready"));
        let navigation = [
            TreeNode::new("ready", Line::from("Ready"), 0),
            TreeNode::new("loading", Line::from("Loading"), 0).with_status(TreeNodeStatus::Loading),
            TreeNode::new("other", Line::from("Other"), 0),
        ];
        assert_eq!(
            keyboard.handle_intent(&navigation, UiIntent::Move(NavigationMove::Next)),
            TreeOutcome::SelectionChanged("other")
        );

        let mut state = TreeState::new(Some("loading"));
        assert_eq!(
            state.handle_intent(&nodes, UiIntent::Activate),
            TreeOutcome::Activated("ready")
        );
        let area = Rect::new(0, 0, 24, 2);
        let mut buffer = Buffer::empty(area);
        let mut painted = TreeState::new(Some("loading"));
        Tree::new(&nodes, &tokens).render(area, &mut buffer, &mut painted);
        assert_eq!(painted.regions().len(), 1);
        assert_eq!(painted.click(Position::new(10, 0)), TreeOutcome::Ignored);
        assert_eq!(
            painted.click(Position::new(10, 1)),
            TreeOutcome::Activated("ready")
        );
    }

    #[test]
    fn repaired_cursor_follows_into_view() {
        let tokens = DesignSystem::junie();
        let nodes = [
            TreeNode::new(0, Line::from("zero"), 0),
            TreeNode::new(1, Line::from("one"), 0),
            TreeNode::new(2, Line::from("two"), 0),
            TreeNode::new(3, Line::from("three"), 0),
            TreeNode::new(4, Line::from("four"), 0),
        ];
        let area = Rect::new(0, 0, 24, 2);
        let mut state = TreeState::new(Some(99));
        state.offset = 3;
        let mut buffer = Buffer::empty(area);

        Tree::new(&nodes, &tokens).render(area, &mut buffer, &mut state);

        assert_eq!(state.cursor(), Some(&0));
        assert_eq!(state.offset(), 0);
    }

    #[test]
    fn full_projection_clears_virtual_window_metadata() {
        let tokens = DesignSystem::junie();
        let partial = [
            TreeNode::new(2, Line::from("two"), 0),
            TreeNode::new(3, Line::from("three"), 0),
        ];
        let full = [
            TreeNode::new(0, Line::from("zero"), 0),
            TreeNode::new(1, Line::from("one"), 0),
            TreeNode::new(2, Line::from("two"), 0),
            TreeNode::new(3, Line::from("three"), 0),
        ];
        let area = Rect::new(0, 0, 24, 2);
        let mut state = TreeState::new(Some(2));
        state.set_virtual_window(2, 4);
        let mut buffer = Buffer::empty(area);

        Tree::new(&partial, &tokens).render(area, &mut buffer, &mut state);
        assert_eq!(state.virtual_total, 4);
        assert_eq!(state.virtual_window_start, 2);

        Tree::new(&full, &tokens).render(area, &mut buffer, &mut state);

        assert_eq!(state.virtual_total, 0);
        assert_eq!(state.virtual_window_start, 0);
        assert_eq!(state.virtualizer().logical_len(), 0);
    }

    #[test]
    fn entering_virtual_window_seeds_absolute_offset() {
        let mut state = TreeState::<usize>::new(Some(50));

        state.set_virtual_window(50, 100);

        assert_eq!(state.offset(), 50);
        assert_eq!(state.virtual_window_start, 50);
        assert_eq!(state.virtual_total, 100);
        assert_eq!(state.virtualizer().offset(), 50);
    }

    #[test]
    fn reasserting_same_virtual_window_preserves_user_scroll() {
        let mut state = TreeState::<usize>::new(Some(50));
        state.set_virtual_window(50, 100);
        assert!(state.scroll_by(7, 5));
        assert_eq!(state.offset(), 57);

        state.set_virtual_window(50, 100);

        assert_eq!(state.offset(), 57);
        assert_eq!(state.virtualizer().offset(), 57);
    }

    #[test]
    fn virtual_window_allows_empty_tail_origin() {
        let mut state = TreeState::<usize>::default();
        state.virt.set_viewport_extent(3);
        state.set_virtual_window(100, 100);

        assert_eq!(state.virtual_window_start, 100);
        assert_eq!(state.virtual_total, 100);
        assert_eq!(state.offset(), 97);
    }

    #[test]
    fn virtual_follow_selection_translates_projected_index_to_absolute_offset() {
        let tokens = DesignSystem::default();
        let nodes: Vec<_> = (50..55)
            .map(|id| TreeNode::new(id, Line::from(format!("node {id}")), 0))
            .collect();
        let area = Rect::new(0, 0, 24, 3);
        let mut state = TreeState::new(Some(54));
        state.set_virtual_window(50, 100);
        let mut buffer = Buffer::empty(area);

        Tree::new(&nodes, &tokens).render(area, &mut buffer, &mut state);

        assert_eq!(state.offset(), 52);
        assert_eq!(state.virtualizer().offset(), 52);
        let visible: Vec<_> = state.regions().iter().map(|region| region.id).collect();
        assert_eq!(visible, vec![52, 53, 54]);
        assert_eq!(state.click(Position::new(4, 2)), TreeOutcome::Activated(54));
    }

    #[test]
    fn virtual_origin_survives_clamp_with_sticky_leading_rows() {
        let tokens = DesignSystem::default();
        let nodes: Vec<_> = (95..100)
            .map(|id| TreeNode::new(id, Line::from(format!("node {id}")), 0))
            .collect();
        let area = Rect::new(0, 0, 24, 3);
        let mut state = TreeState::new(Some(99));
        // Simulate the previous frame's ten-row viewport before the window is
        // replaced. The requested origin must remain 95 even though that
        // viewport clamps the initial scroll offset to 90.
        state.virt.set_viewport_extent(10);
        state.set_sticky(StickyRegion {
            leading: 2,
            trailing: 0,
        });
        state.set_virtual_window(95, 100);
        assert_eq!(state.virtual_window_start, 95);
        assert_eq!(state.offset(), 90);

        let mut buffer = Buffer::empty(area);
        Tree::new(&nodes, &tokens).render(area, &mut buffer, &mut state);

        assert_eq!(state.virtual_window_start, 95);
        assert_eq!(state.offset(), 97);
        let visible: Vec<_> = state.regions().iter().map(|region| region.id).collect();
        assert_eq!(visible, vec![97, 98, 99]);
    }

    #[test]
    fn virtual_offset_clamps_against_virtual_total() {
        let tokens = DesignSystem::default();
        let nodes: Vec<_> = (50..55)
            .map(|id| TreeNode::new(id, Line::from(format!("node {id}")), 0))
            .collect();
        let area = Rect::new(0, 0, 24, 3);
        let mut state = TreeState::new(Some(50));
        state.set_virtual_window(50, 100);

        assert!(state.scroll_by(isize::MAX, nodes.len()));
        assert_eq!(state.offset(), 99);
        state.follow_selection = false;
        let mut buffer = Buffer::empty(area);

        Tree::new(&nodes, &tokens).render(area, &mut buffer, &mut state);

        let expected = max_offset(100, 3);
        assert_eq!(state.offset(), expected);
        assert_eq!(state.virtualizer().offset(), expected as u64);
    }

    #[test]
    fn virtual_scrollbar_uses_logical_content_length() {
        let tokens = DesignSystem::default();
        let nodes: Vec<_> = (50..55)
            .map(|id| TreeNode::new(id, Line::from(format!("node {id}")), 0))
            .collect();
        let area = Rect::new(0, 0, 24, 3);
        let mut state = TreeState::new(Some(50));
        state.set_virtual_window(50, 100);
        state.follow_selection = false;
        let mut buffer = Buffer::empty(area);

        Tree::new(&nodes, &tokens).render(area, &mut buffer, &mut state);

        let scrollbar = state.scrollbar_region.expect("virtual scrollbar");
        assert!(state.scroll_to_position(
            Position::new(scrollbar.x, scrollbar.bottom().saturating_sub(1)),
            nodes.len()
        ));
        assert!(state.offset() > nodes.len());
    }

    #[test]
    fn large_virtual_scrollbar_scales_offset_before_narrowing() {
        let tokens = DesignSystem::default();
        let nodes: Vec<_> = (900_000..900_005)
            .map(|id| TreeNode::new(id, Line::from(format!("node {id}")), 0))
            .collect();
        let area = Rect::new(0, 0, 24, 10);
        let mut state = TreeState::new(Some(900_000));
        state.set_virtual_window(900_000, 1_000_000);
        state.follow_selection = false;
        let mut buffer = Buffer::empty(area);

        Tree::new(&nodes, &tokens).render(area, &mut buffer, &mut state);

        let gutter = state.scrollbar_region.expect("large virtual scrollbar");
        let thumb = crate::scroll::ScrollbarStyle::Line.vertical_thumb();
        let thumb_rows: Vec<_> = (0..gutter.height)
            .filter(|row| buffer[(gutter.x, gutter.y + *row)].symbol() == thumb)
            .collect();
        let scaled_offset =
            scrollbar_geometry(1_000_000, usize::from(area.height), 900_000).2 as usize;
        let (start, len) =
            crate::scroll::overflow_thumb(65_535, 1, usize::from(gutter.height), scaled_offset)
                .expect("scaled geometry overflows");
        assert_eq!(thumb_rows.len(), len);
        assert_eq!(thumb_rows[0], start as u16);
        assert!((7..=9).contains(&thumb_rows[0]));
    }

    #[test]
    fn zero_virtual_total_clears_scroll_geometry() {
        let mut state = TreeState::<usize>::new(Some(50));
        state.set_sticky(StickyRegion {
            leading: 2,
            trailing: 0,
        });
        state.set_virtual_window(50, 100);
        assert!(state.scroll_by(7, 5));

        state.set_virtual_window(99, 0);

        assert_eq!(state.offset(), 0);
        assert_eq!(state.virtual_total, 0);
        assert_eq!(state.virtual_window_start, 0);
        assert_eq!(state.virtualizer().logical_len(), 0);
        assert_eq!(state.virtualizer().offset(), 0);
        assert_eq!(state.collection.total_len(), 0);
        assert_eq!(state.collection.offset(), 0);
    }

    #[test]
    fn returning_to_full_projection_clears_virtual_geometry_atomically() {
        let tokens = DesignSystem::default();
        let window: Vec<_> = (50..55)
            .map(|id| TreeNode::new(id, Line::from(format!("node {id}")), 0))
            .collect();
        let full: Vec<_> = (0..100)
            .map(|id| TreeNode::new(id, Line::from(format!("node {id}")), 0))
            .collect();
        let area = Rect::new(0, 0, 24, 3);
        let mut state = TreeState::new(Some(54));
        state.set_virtual_window(50, 100);
        let mut buffer = Buffer::empty(area);
        Tree::new(&window, &tokens).render(area, &mut buffer, &mut state);
        assert_eq!(state.offset(), 52);

        Tree::new(&full, &tokens).render(area, &mut buffer, &mut state);

        assert_eq!(state.virtual_total, 0);
        assert_eq!(state.virtual_window_start, 0);
        assert_eq!(state.collection.total_len(), 0);
        assert_eq!(state.collection.offset(), 0);
        assert_eq!(state.virtualizer().logical_len(), 0);
        assert_eq!(state.virtualizer().offset(), 0);
        assert_eq!(state.offset(), 52);
    }

    #[test]
    fn typeahead_survives_virtual_to_full_projection_transition() {
        let tokens = DesignSystem::default();
        let full = [
            TreeNode::new("a", Line::from("Alpha"), 0),
            TreeNode::new("b", Line::from("Beta"), 0),
            TreeNode::new("c", Line::from("Charlie"), 0),
        ];
        let partial = [
            TreeNode::new("b", Line::from("Beta"), 0),
            TreeNode::new("c", Line::from("Charlie"), 0),
        ];
        let area = Rect::new(0, 0, 24, 2);
        let mut state = TreeState::new(Some("a"));
        assert_eq!(
            state.handle_key(&full, KeyEvent::new(KeyCode::Char('b'), KeyModifiers::NONE)),
            TreeOutcome::SelectionChanged("b")
        );
        assert_eq!(state.typeahead_buffer(), "b");

        state.set_virtual_window(1, 3);
        let mut buffer = Buffer::empty(area);
        Tree::new(&partial, &tokens).render(area, &mut buffer, &mut state);
        Tree::new(&full, &tokens).render(area, &mut buffer, &mut state);

        assert_eq!(state.typeahead_buffer(), "b");
        assert_eq!(state.cursor(), Some(&"b"));
    }

    #[test]
    fn typeahead_survives_virtual_to_empty_projection_transition() {
        let tokens = DesignSystem::default();
        let full = [
            TreeNode::new("a", Line::from("Alpha"), 0),
            TreeNode::new("b", Line::from("Beta"), 0),
        ];
        let empty: [TreeNode<'_, &str>; 0] = [];
        let area = Rect::new(0, 0, 24, 2);
        let mut state = TreeState::new(Some("a"));
        assert_eq!(
            state.handle_key(&full, KeyEvent::new(KeyCode::Char('b'), KeyModifiers::NONE)),
            TreeOutcome::SelectionChanged("b")
        );
        assert_eq!(state.typeahead_buffer(), "b");

        state.set_virtual_window(1, 2);
        let mut buffer = Buffer::empty(area);
        Tree::new(&empty, &tokens).render(area, &mut buffer, &mut state);
        assert_eq!(state.typeahead_buffer(), "b");

        state.set_virtual_window(0, 0);
        Tree::new(&empty, &tokens).render(area, &mut buffer, &mut state);
        assert_eq!(state.typeahead_buffer(), "b");
        assert_eq!(state.cursor(), None);
    }

    #[test]
    fn cancel_and_close_preserve_stale_projection_state() {
        let nodes = [TreeNode::new("b", Line::from("Beta"), 0)];
        let mut state = TreeState::new(Some("a"));
        state.set_semantic_selection(Some("a"));

        assert_eq!(
            state.handle_intent(&nodes, UiIntent::Cancel),
            TreeOutcome::Cancelled
        );
        assert_eq!(state.cursor(), Some(&"a"));
        assert_eq!(state.semantic_selection(), Some(&"a"));
        assert_eq!(
            state.handle_intent(&nodes, UiIntent::Close),
            TreeOutcome::Cancelled
        );
        assert_eq!(state.cursor(), Some(&"a"));
        assert_eq!(state.semantic_selection(), Some(&"a"));

        let mut key_state = TreeState::new(Some("a"));
        key_state.set_semantic_selection(Some("a"));
        assert_eq!(
            key_state.handle_key(&nodes, KeyEvent::new(KeyCode::Esc, KeyModifiers::NONE)),
            TreeOutcome::Cancelled
        );
        assert_eq!(key_state.cursor(), Some(&"a"));
        assert_eq!(key_state.semantic_selection(), Some(&"a"));
    }

    #[test]
    fn input_reconciles_full_projection_before_activation() {
        let nodes = [
            TreeNode::new("b", Line::from("Beta"), 0),
            TreeNode::new("c", Line::from("Charlie"), 0),
        ];

        let mut intent_state = TreeState::new(Some("a"));
        intent_state.set_semantic_selection(Some("a"));
        assert_eq!(
            intent_state.handle_intent(&nodes, UiIntent::Activate),
            TreeOutcome::Activated("b")
        );
        assert_eq!(intent_state.cursor(), Some(&"b"));
        assert_eq!(intent_state.semantic_selection(), Some(&"b"));

        let mut key_state = TreeState::new(Some("a"));
        assert_eq!(
            key_state.handle_key(&nodes, KeyEvent::new(KeyCode::Enter, KeyModifiers::NONE)),
            TreeOutcome::Activated("b")
        );
        assert_eq!(key_state.cursor(), Some(&"b"));
    }

    #[test]
    fn full_projection_reconciles_removed_cursor_selection_and_hover() {
        let tokens = DesignSystem::junie();
        let first = [
            TreeNode::new("a", Line::from("A"), 0),
            TreeNode::new("b", Line::from("B"), 0),
        ];
        let second = [
            TreeNode::new("b", Line::from("B"), 0),
            TreeNode::new("c", Line::from("C"), 0),
        ];
        let area = Rect::new(0, 0, 24, 2);
        let mut state = TreeState::new(Some("a"));
        state.set_semantic_selection(Some("a"));
        let mut buffer = Buffer::empty(area);
        Tree::new(&first, &tokens).render(area, &mut buffer, &mut state);
        assert_eq!(state.hover(Position::new(10, 0)), Some(&"a"));

        Tree::new(&second, &tokens).render(area, &mut buffer, &mut state);

        assert_eq!(state.cursor(), Some(&"b"));
        assert_eq!(state.semantic_selection(), None);
        assert_eq!(state.hovered(), None);
        assert_eq!(
            state.handle_intent(&second, UiIntent::Move(NavigationMove::Next)),
            TreeOutcome::SelectionChanged("c")
        );
        assert_eq!(
            state.handle_intent(&second, UiIntent::Activate),
            TreeOutcome::Activated("c")
        );
    }

    #[test]
    fn partial_virtual_projection_preserves_off_window_identity() {
        let tokens = DesignSystem::junie();
        let nodes = [TreeNode::new("b", Line::from("B"), 0)];
        let area = Rect::new(0, 0, 24, 1);
        let mut state = TreeState::new(Some("a"));
        state.set_semantic_selection(Some("a"));
        state.set_virtual_window(1, 3);
        let mut buffer = Buffer::empty(area);

        Tree::new(&nodes, &tokens).render(area, &mut buffer, &mut state);

        assert_eq!(state.cursor(), Some(&"a"));
        assert_eq!(state.semantic_selection(), Some(&"a"));
        assert_eq!(
            state.handle_intent(&nodes, UiIntent::Move(NavigationMove::Next)),
            TreeOutcome::Ignored
        );
        assert_eq!(
            state.handle_intent(&nodes, UiIntent::Move(NavigationMove::Previous)),
            TreeOutcome::Ignored
        );
    }

    #[test]
    fn full_projection_without_cursor_preserves_no_focus() {
        let tokens = DesignSystem::junie();
        let nodes = [TreeNode::new("a", Line::from("A"), 0)];
        let area = Rect::new(0, 0, 24, 1);
        let mut state = TreeState::default();
        let mut buffer = Buffer::empty(area);

        Tree::new(&nodes, &tokens).render(area, &mut buffer, &mut state);

        assert_eq!(state.cursor(), None);
        assert_eq!(state.semantic_selection(), None);
        assert_eq!(state.regions().len(), 1);
    }

    #[test]
    fn filter_preserves_ancestors() {
        let nodes = [
            TreeNode::new("root", Line::from("Workspace"), 0)
                .branch()
                .expanded(),
            TreeNode::new("src", Line::from("src"), 1)
                .branch()
                .expanded()
                .parent("root"),
            TreeNode::new("lib", Line::from("lib.rs"), 2).parent("src"),
            TreeNode::new("tests", Line::from("tests"), 1).parent("root"),
        ];
        let filtered = filter_tree_with_ancestors(&nodes, "lib");
        let ids: Vec<_> = filtered.iter().map(|n| n.id).collect();
        assert!(ids.contains(&"lib"));
        assert!(ids.contains(&"src"));
        assert!(ids.contains(&"root"));
        assert!(!ids.contains(&"tests"));
    }

    #[test]
    fn virtual_window_and_anchor() {
        let mut state = TreeState::<usize>::new(Some(10));
        state.set_virtual_window(100, 10_000);
        assert_eq!(state.virtualizer().logical_len(), 10_000);
        state.capture_scroll_anchor();
        state.offset = 0;
        state.restore_scroll_anchor();
        // anchor restore sets virt offset
        assert!(state.virtualizer().offset() <= 10_000);
    }

    #[test]
    fn actions_and_filter_chrome_paint() {
        let nodes = [TreeNode::new("a", Line::from("File"), 0)
            .actions(Line::from("…"))
            .shortcut("f")];
        let tokens = DesignSystem::default();
        let mut state = TreeState::new(Some("a"));
        state.set_filter_query(Some("fi".into()));
        let area = Rect::new(0, 0, 40, 4);
        let mut buffer = Buffer::empty(area);
        (&Tree::new(&nodes, &tokens)).render(area, &mut buffer, &mut state);
        let mut s = String::new();
        for x in 0..40 {
            s.push_str(buffer[(x, 0)].symbol());
        }
        assert!(s.contains('/') || s.contains("fi"), "{s}");
    }

    fn row_text(buffer: &Buffer, y: u16, width: u16) -> String {
        (0..width)
            .map(|x| buffer[(x, y)].symbol().to_string())
            .collect()
    }

    #[test]
    fn anatomy_matches_junie_gutter_indent_disclosure_meta() {
        let tokens = DesignSystem::junie();
        let rows = [
            TreeNode::new("src", Line::from("src"), 0)
                .branch()
                .expanded(),
            TreeNode::new("api", Line::from("api"), 1).branch(),
            TreeNode::new("file", Line::from("config.rs"), 1).badge(Line::from("1.9 KB")),
        ];
        let area = Rect::new(0, 0, 40, 3);
        let mut buffer = Buffer::empty(area);
        let mut state = TreeState::new(Some("src"));
        Tree::new(&rows, &tokens).render(area, &mut buffer, &mut state);

        assert_eq!(buffer[(0, 0)].symbol(), tokens.glyphs.selection_gutter());
        assert_eq!(buffer[(0, 1)].symbol(), tokens.glyphs.selection_gutter());
        assert_eq!(buffer[(0, 2)].symbol(), tokens.glyphs.selection_gutter());
        assert_eq!(buffer[(1, 0)].symbol(), tokens.glyphs.disclosure_open());
        assert_eq!(buffer[(1, 1)].symbol(), " ");
        assert_eq!(buffer[(2, 1)].symbol(), " ");
        assert_eq!(buffer[(3, 1)].symbol(), tokens.glyphs.disclosure_closed());
        assert_eq!(buffer[(3, 2)].symbol(), " ");

        let row0 = row_text(&buffer, 0, 40);
        let row1 = row_text(&buffer, 1, 40);
        let row2 = row_text(&buffer, 2, 40);
        assert!(row0.contains("src"), "{row0:?}");
        assert!(row1.contains("api"), "{row1:?}");
        assert!(row2.contains("config.rs"), "{row2:?}");
        assert!(row2.contains("1.9 KB"), "{row2:?}");
        let meta_at = row2.find("1.9 KB").expect("meta");
        let label_at = row2.find("config.rs").expect("label");
        assert!(label_at < meta_at, "{row2:?}");
        assert!(
            !row0.contains(tokens.glyphs.selection_marker()),
            "tree must not steal › as a selection marker: {row0:?}"
        );
    }

    #[test]
    fn cursor_and_semantic_selection_paint_independently() {
        let tokens = DesignSystem::junie();
        let rows = [
            TreeNode::new("cursor", Line::from("cursor"), 0),
            TreeNode::new("selected", Line::from("selected"), 0),
        ];
        let area = Rect::new(0, 0, 24, 2);
        let mut state = TreeState::new(Some("cursor"));

        assert_eq!(state.cursor(), Some(&"cursor"));
        assert_eq!(state.selected(), Some(&"cursor"));
        assert_eq!(state.semantic_selection(), None);

        let mut buffer = Buffer::empty(area);
        Tree::new(&rows, &tokens)
            .focused(true)
            .render(area, &mut buffer, &mut state);
        assert_eq!(buffer[(0, 0)].fg, tokens.junie_theme().focus);
        assert!(buffer[(3, 0)].modifier.contains(Modifier::BOLD));
        assert_eq!(
            buffer[(3, 0)].bg,
            tokens.junie_theme().surface,
            "cursor focus alone must not paint selection tint"
        );
        assert!(!buffer[(3, 1)].modifier.contains(Modifier::BOLD));

        state.set_semantic_selection(Some("selected"));
        assert_eq!(state.cursor(), Some(&"cursor"));
        assert_eq!(state.semantic_selection(), Some(&"selected"));
        state.set_semantic_selection(Some("cursor"));
        let mut selected_buffer = Buffer::empty(area);
        Tree::new(&rows, &tokens)
            .focused(true)
            .render(area, &mut selected_buffer, &mut state);
        assert_eq!(state.cursor(), Some(&"cursor"));
        assert_eq!(state.semantic_selection(), Some(&"cursor"));
        assert_eq!(
            selected_buffer[(3, 0)].bg,
            tokens
                .style(Role::SelectionTint)
                .bg
                .expect("selection tint")
        );
    }

    #[test]
    fn selected_label_uses_accent_and_loading_paints_spinner() {
        let tokens = DesignSystem::junie();
        let rows = [
            TreeNode::new("src", Line::from("src"), 0)
                .branch()
                .expanded(),
            TreeNode::new("busy", Line::from("pending"), 1)
                .branch()
                .loading(),
        ];
        let area = Rect::new(0, 0, 24, 2);
        let mut buffer = Buffer::empty(area);
        let mut state = TreeState::new(Some("src"));
        state.set_semantic_selection(Some("src"));
        Tree::new(&rows, &tokens)
            .spinner_frame(0)
            .render(area, &mut buffer, &mut state);
        assert_eq!(buffer[(3, 0)].symbol(), "s");
        assert_eq!(
            buffer[(3, 0)].fg,
            tokens.style(Role::Accent).fg.expect("accent")
        );
        assert_eq!(buffer[(3, 1)].symbol(), SPINNER_BRAILLE_FRAMES[0]);
        assert_eq!(
            buffer[(3, 1)].fg,
            tokens.style(Role::Accent).fg.expect("accent")
        );
    }

    #[test]
    fn enter_toggles_folder_and_activates_leaf() {
        let nodes = [
            TreeNode::new("dir", Line::from("dir"), 0).branch(),
            TreeNode::new("file", Line::from("file"), 0),
        ];
        let mut state = TreeState::new(Some("dir"));
        assert_eq!(
            state.handle_key(&nodes, KeyEvent::new(KeyCode::Enter, KeyModifiers::NONE)),
            TreeOutcome::Toggle("dir")
        );
        state.select(Some("file"));
        assert_eq!(
            state.handle_key(&nodes, KeyEvent::new(KeyCode::Enter, KeyModifiers::NONE)),
            TreeOutcome::Activated("file")
        );
    }

    #[test]
    fn star_and_dash_request_bulk_disclosure() {
        let nodes = [TreeNode::new("dir", Line::from("dir"), 0).branch()];
        let mut state = TreeState::new(Some("dir"));
        assert_eq!(
            state.handle_key(
                &nodes,
                KeyEvent::new(KeyCode::Char('*'), KeyModifiers::NONE)
            ),
            TreeOutcome::Ignored
        );
        assert_eq!(state.take_bulk_disclosure(), Some(true));
        assert_eq!(
            state.handle_key(
                &nodes,
                KeyEvent::new(KeyCode::Char('-'), KeyModifiers::NONE)
            ),
            TreeOutcome::Ignored
        );
        assert_eq!(state.take_bulk_disclosure(), Some(false));
    }

    #[test]
    fn space_toggles_folder_when_multi_select_is_off() {
        let nodes = [TreeNode::new("dir", Line::from("dir"), 0).branch()];
        let mut state = TreeState::new(Some("dir"));
        assert_eq!(
            state.handle_key(
                &nodes,
                KeyEvent::new(KeyCode::Char(' '), KeyModifiers::NONE)
            ),
            TreeOutcome::Toggle("dir")
        );
    }

    #[test]
    fn overflowing_tree_uses_overflow_thumb() {
        let system = DesignSystem::default();
        let nodes: Vec<TreeNode<'_, usize>> = (0..24)
            .map(|i| TreeNode::new(i, Line::from(format!("n{i:02}")), 0))
            .collect();
        let mut state = TreeState::new(Some(0));
        let area = Rect::new(0, 0, 20, 8);
        let mut buffer = Buffer::empty(area);
        Tree::new(&nodes, &system).render(area, &mut buffer, &mut state);
        let thumb = crate::scroll::ScrollbarStyle::Line.vertical_thumb();
        let track = crate::scroll::SCROLLBAR_TRACK;
        let x = area.right().saturating_sub(1);
        let viewport = usize::from(area.height);
        let (start, len) = crate::scroll::overflow_thumb(24, viewport, viewport, 0)
            .expect("24 nodes overflow an 8-row viewport");
        let thumbs: Vec<u16> = (0..area.height)
            .filter(|y| buffer[(x, *y)].symbol() == thumb)
            .collect();
        assert_eq!(thumbs.len(), len);
        assert_eq!(thumbs[0], start as u16);
        assert_eq!(buffer[(x, len as u16)].symbol(), track);
    }
}
