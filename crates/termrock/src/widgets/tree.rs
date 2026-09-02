//! **Tree** — hierarchical collection for files, schemas, tasks, settings, objects.
//!
//! **Mission.** Flattened projection with stable IDs, lazy children, loading/error
//! child state, expansion, active cursor, selection/check, icons, metadata,
//! context actions, and typeahead. Left/right define collapse / expand-or-enter
//! precisely. Filtering retains ancestor context. Large trees virtualize via
//! window + optional [`Virtualizer`]; scroll anchors preserve position. ASCII
//! disclosure/indent fallbacks come from the design system glyph set.
//!
//! **Ownership.** Host owns hierarchy, expansion set, and lazy fetch. Tree owns
//! cursor/selection interaction, scroll, hit geometry, and typed outcomes.
//!
//! Research: file explorers, broot, Yazi, VS Code trees, TermRock List/VirtualList.
#![allow(unused_imports)] // test-only imports retained
use ratatui_core::{
    buffer::Buffer,
    layout::{Position, Rect},
    style::Modifier,
    text::Line,
    widgets::StatefulWidget,
};

use crate::{
    input::{KeyCode, KeyEvent, KeyEventKind, KeyModifiers},
    interaction::{CollectionItem, CollectionState, HitRegion, NavigationMove, PageMove, UiIntent},
    scroll::max_offset,
    style::{DesignSystem, ListRowVisualState, Role},
    text::{display_cols, take_display_cols},
};

use super::{ComposedRow, Selection, StickyRegion, Virtualizer};

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
            enabled: self.enabled,
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
    /// Navigation selected this stable node identity (active cursor).
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
/// **Cursor** is [`Self::selected`]. Multi **check** state is optional
/// [`Self::selection`]. Expansion remains on the host projection (`TreeNode.expanded`).
pub struct TreeState<Id> {
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
}

impl<Id> Default for TreeState<Id> {
    fn default() -> Self {
        Self {
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
        }
    }
}

impl<Id> TreeState<Id> {
    #[must_use]
    /// Creates tree state with no hover/scroll; optional initial cursor.
    pub fn new(selected: Option<Id>) -> Self
    where
        Id: Clone + PartialEq,
    {
        let mut collection = CollectionState::new();
        collection.set_active(selected.clone());
        Self {
            selected,
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
        }
    }

    #[must_use]
    /// Returns the currently selected stable identity.
    pub const fn selected(&self) -> Option<&Id> {
        self.selected.as_ref()
    }

    #[must_use]
    /// Returns the stable identity currently under the pointer.
    pub const fn hovered(&self) -> Option<&Id> {
        self.hovered.as_ref()
    }

    #[must_use]
    /// Returns the zero-based first visible node index.
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

    /// Selects the item with the supplied stable identity (active cursor).
    pub fn select(&mut self, selected: Option<Id>)
    where
        Id: Clone + PartialEq,
    {
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
    pub fn set_virtual_window(&mut self, window_start: usize, total_len: usize) {
        self.virtual_window_start = window_start;
        self.virtual_total = total_len;
        self.virt.set_len(total_len as u64);
        self.virt.set_offset(window_start as u64);
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

    /// Moves the scroll position by a signed delta and clamps it to valid content.
    pub fn scroll_by(&mut self, delta: isize, node_count: usize) -> bool {
        let before = self.offset;
        let maximum = max_offset(node_count, self.viewport_height);
        self.offset = if delta.is_negative() {
            self.offset.saturating_sub(delta.unsigned_abs())
        } else {
            self.offset
                .saturating_add(delta.unsigned_abs())
                .min(maximum)
        };
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
        self.offset = crate::scroll::offset_for_track_position(
            node_count,
            self.viewport_height,
            area.height,
            usize::from(position.y.saturating_sub(area.y)),
        );
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
    /// Routes navigation, disclosure, checking, activation, filter, and typeahead.
    ///
    /// Prefer intents via [`crate::interaction::default_tree_intent`]. Printable
    /// characters feed typeahead. `/` opens filter query chrome.
    pub fn handle_key(&mut self, nodes: &[TreeNode<'_, Id>], key: KeyEvent) -> TreeOutcome<Id> {
        if key.kind == KeyEventKind::Release {
            return TreeOutcome::Ignored;
        }
        // Filter mode
        if key.kind == KeyEventKind::Press
            && matches!(key.code, KeyCode::Char('/'))
            && key.modifiers.is_empty()
        {
            if self.filter_query.is_none() {
                self.filter_query = Some(String::new());
            }
            return TreeOutcome::Ignored; // host reprojects; treat as chrome change
        }
        if self.filter_query.is_some()
            && key.kind == KeyEventKind::Press
            && key.modifiers.is_empty()
        {
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
        if let Some(intent) = crate::interaction::default_tree_intent(key) {
            self.collection.clear_typeahead();
            return self.handle_intent(nodes, intent);
        }
        self.handle_typeahead(nodes, key)
    }

    fn handle_typeahead(&mut self, nodes: &[TreeNode<'_, Id>], key: KeyEvent) -> TreeOutcome<Id> {
        if key.kind != KeyEventKind::Press {
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
                self.selected = Some(id.clone());
                self.follow_selection = true;
                return TreeOutcome::SelectionChanged(id);
            }
        }
        TreeOutcome::Ignored
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
        match intent {
            UiIntent::Move(NavigationMove::Previous) => self.move_selection(nodes, -1),
            UiIntent::Move(NavigationMove::Next) => self.move_selection(nodes, 1),
            UiIntent::Move(NavigationMove::First) => self.select_boundary(nodes, false),
            UiIntent::Move(NavigationMove::Last) => self.select_boundary(nodes, true),
            UiIntent::Page(PageMove::Backward) => self.page_selection(nodes, false),
            UiIntent::Page(PageMove::Forward) => self.page_selection(nodes, true),
            UiIntent::Collapse => self.collapse_or_parent(nodes),
            UiIntent::Expand => self.expand_or_enter(nodes),
            UiIntent::Activate => self
                .selected_node(nodes)
                .map_or(TreeOutcome::Ignored, |node| {
                    TreeOutcome::Activated(node.id.clone())
                }),
            UiIntent::Toggle => self.toggle_selected(nodes),
            UiIntent::Cancel | UiIntent::Close => TreeOutcome::Cancelled,
            _ => TreeOutcome::Ignored,
        }
    }

    fn toggle_selected(&mut self, nodes: &[TreeNode<'_, Id>]) -> TreeOutcome<Id> {
        let Some(selection) = self.selection.as_mut() else {
            return TreeOutcome::Ignored;
        };
        let Some(node) = self.selected.as_ref().and_then(|selected| {
            nodes
                .iter()
                .find(|node| node.enabled && &node.id == selected)
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

    /// Maps a pointer position to the semantic outcome of the painted hit region.
    pub fn click(&mut self, position: Position) -> TreeOutcome<Id> {
        if let Some(region) = self
            .disclosure_regions
            .iter()
            .find(|region| region.area.contains(position))
        {
            return TreeOutcome::Toggle(region.id.clone());
        }
        if let Some(id) = self
            .check_regions
            .iter()
            .find(|region| region.area.contains(position))
            .map(|region| region.id.clone())
        {
            self.selected = Some(id.clone());
            self.follow_selection = true;
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
        if self.selected.as_ref() == Some(&id) {
            TreeOutcome::Activated(id)
        } else {
            self.selected = Some(id.clone());
            self.follow_selection = true;
            TreeOutcome::SelectionChanged(id)
        }
    }

    fn selected_index(&self, nodes: &[TreeNode<'_, Id>]) -> Option<usize> {
        let selected = self.selected.as_ref()?;
        nodes.iter().position(|node| &node.id == selected)
    }

    fn selected_node<'a>(&self, nodes: &'a [TreeNode<'_, Id>]) -> Option<&'a TreeNode<'a, Id>> {
        let index = self.selected_index(nodes)?;
        nodes.get(index).filter(|node| node.enabled)
    }

    fn move_selection(&mut self, nodes: &[TreeNode<'_, Id>], delta: i32) -> TreeOutcome<Id> {
        if self.selected.is_none() {
            return self.select_boundary(nodes, delta < 0);
        }
        let start = self
            .selected_index(nodes)
            .unwrap_or(if delta < 0 { nodes.len() } else { 0 });
        let candidate = if delta < 0 {
            nodes[..start].iter().rposition(|node| node.enabled)
        } else {
            nodes
                .iter()
                .enumerate()
                .skip(start.saturating_add(1))
                .find(|(_, node)| node.enabled)
                .map(|(index, _)| index)
        };
        self.select_index(nodes, candidate)
    }

    fn page_selection(&mut self, nodes: &[TreeNode<'_, Id>], forward: bool) -> TreeOutcome<Id> {
        if self.selected.is_none() {
            return self.select_boundary(nodes, !forward);
        }
        let Some(start) = self.selected_index(nodes) else {
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
                .find(|(_, node)| node.enabled)
                .map(|(index, _)| index)
                .or_else(|| nodes[..target].iter().rposition(|node| node.enabled))
        } else {
            nodes[..=target]
                .iter()
                .rposition(|node| node.enabled)
                .or_else(|| {
                    nodes
                        .iter()
                        .enumerate()
                        .skip(target.saturating_add(1))
                        .find(|(_, node)| node.enabled)
                        .map(|(index, _)| index)
                })
        };
        self.select_index(nodes, candidate)
    }

    fn select_boundary(&mut self, nodes: &[TreeNode<'_, Id>], from_end: bool) -> TreeOutcome<Id> {
        let candidate = if from_end {
            nodes.iter().rposition(|node| node.enabled)
        } else {
            nodes.iter().position(|node| node.enabled)
        };
        self.select_index(nodes, candidate)
    }

    fn select_index(
        &mut self,
        nodes: &[TreeNode<'_, Id>],
        index: Option<usize>,
    ) -> TreeOutcome<Id> {
        let Some(node) = index.and_then(|index| nodes.get(index)) else {
            return TreeOutcome::Ignored;
        };
        self.selected = Some(node.id.clone());
        self.collection.set_active(Some(node.id.clone()));
        self.follow_selection = true;
        TreeOutcome::SelectionChanged(node.id.clone())
    }

    /// Left: collapse expanded branch, else select parent.
    fn collapse_or_parent(&mut self, nodes: &[TreeNode<'_, Id>]) -> TreeOutcome<Id> {
        let Some(index) = self.selected_index(nodes) else {
            return TreeOutcome::Ignored;
        };
        let node = &nodes[index];
        if node.enabled && node.branch && node.expanded {
            return TreeOutcome::Toggle(node.id.clone());
        }
        // Prefer explicit parent id when present.
        if let Some(ref pid) = node.parent {
            if let Some(pidx) = nodes.iter().position(|n| n.enabled && &n.id == pid) {
                return self.select_index(nodes, Some(pidx));
            }
        }
        let parent = nodes[..index]
            .iter()
            .rposition(|candidate| candidate.enabled && candidate.depth < node.depth);
        self.select_index(nodes, parent)
    }

    /// Right: expand collapsed/lazy branch, else enter first visible child.
    fn expand_or_enter(&mut self, nodes: &[TreeNode<'_, Id>]) -> TreeOutcome<Id> {
        let Some(index) = self.selected_index(nodes) else {
            return TreeOutcome::Ignored;
        };
        let node = &nodes[index];
        if !node.enabled {
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
                .find(|(_, n)| n.enabled && n.depth == child_depth)
                .map(|(i, _)| i);
            return self.select_index(nodes, child);
        }
        TreeOutcome::Ignored
    }
}

fn collection_items_from_nodes<Id: Clone>(nodes: &[TreeNode<'_, Id>]) -> Vec<CollectionItem<Id>> {
    nodes
        .iter()
        .filter(|n| n.enabled && !n.status.skips_navigation())
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
        }
    }

    /// Whether this surface owns keyboard focus this frame (host / scene).
    #[must_use]
    pub const fn focused(mut self, focused: bool) -> Self {
        self.focused = focused;
        self
    }

    /// Message painted when `nodes` is empty.
    #[must_use]
    pub const fn empty_message(mut self, message: &'a str) -> Self {
        self.empty_message = Some(message);
        self
    }

    /// Design tokens used for recipes and glyphs.
    #[must_use]
    pub const fn tokens(&self) -> &DesignSystem {
        self.tokens
    }
}

impl<Id: Clone + PartialEq> StatefulWidget for &Tree<'_, Id> {
    type State = TreeState<Id>;

    fn render(self, area: Rect, buffer: &mut Buffer, state: &mut Self::State) {
        state.regions.clear();
        state.disclosure_regions.clear();
        state.check_regions.clear();
        state.scrollbar_region = None;
        if area.is_empty() {
            state.offset = 0;
            state.viewport_height = 0;
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
        state.viewport_height = usize::from(body.height);
        state.virt.set_viewport_extent(body.height.max(1));
        if body.is_empty() {
            return;
        }
        if self.nodes.is_empty() {
            state.offset = 0;
            if let Some(message) = self.empty_message {
                let style = self.tokens.style(Role::TextMuted);
                buffer.set_stringn(body.x, body.y, message, usize::from(body.width), style);
            }
            return;
        }

        let node_count = if state.virtual_total > 0 {
            state.virtual_total
        } else {
            self.nodes.len()
        };
        if state.virtual_total > 0 {
            state.virt.set_len(state.virtual_total as u64);
        }

        if state.follow_selection
            && let Some(selected) = state.selected_index(self.nodes)
        {
            // selected is index in projected window
            if selected < state.offset {
                state.offset = selected;
            } else if selected >= state.offset.saturating_add(usize::from(body.height)) {
                state.offset = selected.saturating_sub(usize::from(body.height).saturating_sub(1));
            }
        }
        state.follow_selection = false;
        let paint_len = if state.virtual_total > 0 {
            self.nodes.len()
        } else {
            self.nodes.len()
        };
        state.offset = state
            .offset
            .min(max_offset(paint_len, usize::from(body.height)));
        let scroll_len = if state.virtual_total > 0 {
            state.virtual_total
        } else {
            self.nodes.len()
        };
        let _ = node_count;
        let show_scrollbar =
            crate::scroll::is_scrollable(scroll_len, usize::from(body.height)) && body.width > 1;
        let content_area = Rect {
            x: body.x,
            y: body.y,
            width: body.width.saturating_sub(u16::from(show_scrollbar)),
            height: body.height,
        };
        let paint_offset = if state.virtual_total > 0 {
            0
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
        state.viewport_width = content_area.width.saturating_sub(4);
        state.h_offset = state
            .h_offset
            .min(state.content_width.saturating_sub(state.viewport_width));
        // Density indent; collapse under narrow pressure (tiny → 0).
        let indent_step = match content_area.width {
            0..=7 => 0,
            8..=11 => 1,
            _ => self.tokens.spacing.tree_indent.max(1),
        };
        for (visible, node) in self
            .nodes
            .iter()
            .skip(paint_offset)
            .take(usize::from(body.height))
            .enumerate()
        {
            let y = body
                .y
                .saturating_add(u16::try_from(visible).unwrap_or(u16::MAX));
            let row = Rect::new(content_area.x, y, content_area.width, 1);
            let selected = state.selected.as_ref() == Some(&node.id);
            let hovered = state.hovered.as_ref() == Some(&node.id);
            let checked = state
                .selection
                .as_ref()
                .is_some_and(|selection| selection.is_checked(&node.id));
            let loading = matches!(node.status, TreeNodeStatus::Loading | TreeNodeStatus::Lazy);
            let recipe = self.tokens.resolve_list_row(ListRowVisualState {
                selected,
                focused: self.focused && selected,
                hovered,
                enabled: node.enabled,
                loading,
                checked,
                ..ListRowVisualState::default()
            });
            let mut style = match node.status {
                TreeNodeStatus::Ready if node.enabled => {
                    recipe.label.patch(self.tokens.style(node.tone.role()))
                }
                TreeNodeStatus::Ready => self.tokens.style(Role::TextDisabled),
                // Busy rows keep the body tone one plane down: secondary label
                // (junie `row()` busy law); the spinner owns the accent.
                TreeNodeStatus::Loading | TreeNodeStatus::Lazy => {
                    self.tokens.style(Role::TextSecondary)
                }
                TreeNodeStatus::Error => self.tokens.style(Role::Danger),
            };
            if !node.enabled {
                style = self.tokens.style(Role::TextDisabled);
            }
            if selected && node.enabled {
                style = recipe.label;
                if self.focused {
                    style = style.add_modifier(Modifier::BOLD);
                }
            } else if hovered && node.enabled {
                style = recipe.hover;
            } else if checked && node.enabled {
                style = style.patch(self.tokens.style(Role::Accent));
            }
            if recipe.use_fill && selected {
                buffer.set_style(row, style);
            } else if recipe.hover_fill {
                buffer.set_style(row, recipe.hover_wash);
            }

            // Quiet selection gutter (aligned with List) when Gutter chrome.
            let mut x_cursor = content_area.x;
            if let Some((gutter_glyph, gutter_style)) = recipe.gutter {
                buffer.set_stringn(x_cursor, y, gutter_glyph, 1, gutter_style);
                x_cursor = x_cursor.saturating_add(2);
            } else if recipe.show_gutter_slot
                && matches!(
                    self.tokens.selection,
                    crate::style::SelectionChrome::Gutter | crate::style::SelectionChrome::Tint
                )
            {
                // Reserve gutter column only when selection chrome uses a slot.
                // For tree, reserve only if any selection likely — keep 2 cells when
                // tokens use Gutter default (DesignSystem::phosphor).
                if matches!(self.tokens.selection, crate::style::SelectionChrome::Gutter) {
                    x_cursor = x_cursor.saturating_add(2);
                }
            }

            let max_indent = content_area
                .right()
                .saturating_sub(x_cursor)
                .saturating_sub(4);
            let indent = node.depth.saturating_mul(indent_step).min(max_indent);
            let disclosure_x = x_cursor.saturating_add(indent);
            let glyph = if node.branch {
                if node.expanded {
                    self.tokens.glyphs.disclosure_open()
                } else {
                    self.tokens.glyphs.disclosure_closed()
                }
            } else {
                " "
            };
            if disclosure_x < content_area.right() {
                buffer.set_stringn(disclosure_x, y, glyph, 1, style);
            }
            let check_x = disclosure_x.saturating_add(2);
            let mut check_w = 0u16;
            if state.selection.is_some() && check_x < content_area.right() {
                let marker = if checked {
                    recipe.check_on
                } else {
                    recipe.check_off
                };
                let gw = u16::try_from(crate::text::display_cols(marker)).unwrap_or(1);
                let available = content_area.right().saturating_sub(check_x);
                let paint_w = gw.min(available);
                if paint_w > 0 {
                    buffer.set_stringn(check_x, y, marker, usize::from(paint_w), style);
                    check_w = paint_w.saturating_add(u16::from(available > paint_w));
                    if available > paint_w {
                        buffer.set_stringn(check_x.saturating_add(paint_w), y, " ", 1, style);
                    }
                    if node.enabled {
                        state.check_regions.push(HitRegion {
                            id: node.id.clone(),
                            area: Rect::new(check_x, y, paint_w.max(1), 1),
                        });
                    }
                }
            }
            let label_x = check_x.saturating_add(check_w);
            // Colorless status suffixes; composed anatomy owns label/badge/shortcut.
            let status = match node.status {
                TreeNodeStatus::Ready => None,
                TreeNodeStatus::Loading => Some(" loading"),
                TreeNodeStatus::Lazy => Some(" lazy"),
                TreeNodeStatus::Error => Some(" error"),
            };
            let status_w = status
                .map(crate::text::display_cols)
                .and_then(|width| u16::try_from(width).ok())
                .unwrap_or(0);
            if label_x < content_area.right() {
                let content_w = content_area
                    .right()
                    .saturating_sub(label_x)
                    .saturating_sub(status_w);
                // Zero-copy contraction: borrow fields; no Line clones (hot path).
                // Fit-based: keep the badge whenever it still fits next to
                // a one-cell primary identity (mirrors ComposedRow budgets).
                let badge = node.badge.as_ref();
                let badge_need = badge
                    .map(|b| {
                        u16::try_from(b.width())
                            .unwrap_or(u16::MAX)
                            .saturating_add(2)
                    })
                    .unwrap_or(0);
                let actions_need = node
                    .actions
                    .as_ref()
                    .map(|a| {
                        u16::try_from(a.width())
                            .unwrap_or(u16::MAX)
                            .saturating_add(2)
                    })
                    .unwrap_or(0);
                let shortcut_need = node
                    .shortcut
                    .map(|s| {
                        u16::try_from(crate::text::display_cols(s))
                            .unwrap_or(u16::MAX)
                            .saturating_add(2)
                    })
                    .unwrap_or(0);
                let leading_need = node
                    .leading
                    .as_ref()
                    .map(|l| {
                        u16::try_from(l.width())
                            .unwrap_or(u16::MAX)
                            .saturating_add(1)
                    })
                    .unwrap_or(0);
                let secondary_need = node
                    .secondary
                    .as_ref()
                    .map(|s| {
                        u16::try_from(s.width())
                            .unwrap_or(u16::MAX)
                            .saturating_add(1)
                    })
                    .unwrap_or(0);
                // Drop: shortcut → actions → badge → secondary → leading → primary
                let mut budget = content_w.saturating_sub(1); // primary min
                let show_shortcut = node.shortcut.is_some() && budget >= shortcut_need;
                if show_shortcut {
                    budget = budget.saturating_sub(shortcut_need);
                }
                let show_actions = node.actions.is_some() && budget >= actions_need;
                if show_actions {
                    budget = budget.saturating_sub(actions_need);
                }
                let show_badge = badge.is_some() && budget >= badge_need;
                if show_badge {
                    budget = budget.saturating_sub(badge_need);
                }
                let show_secondary = node.secondary.is_some() && budget >= secondary_need;
                if show_secondary {
                    budget = budget.saturating_sub(secondary_need);
                }
                let show_leading = node.leading.is_some() && budget >= leading_need;
                let mut x = label_x;
                if show_leading && let Some(leading) = node.leading.as_ref() {
                    let lw = u16::try_from(leading.width())
                        .unwrap_or(u16::MAX)
                        .min(label_x.saturating_add(content_w).saturating_sub(x));
                    if lw > 0 {
                        buffer.set_line(x, y, leading, lw);
                        x = x.saturating_add(lw).saturating_add(1);
                    }
                }
                let badge_w = if show_badge {
                    badge
                        .map(|b| u16::try_from(b.width()).unwrap_or(u16::MAX))
                        .unwrap_or(0)
                } else {
                    0
                };
                let actions_w = if show_actions {
                    node.actions
                        .as_ref()
                        .map(|a| u16::try_from(a.width()).unwrap_or(u16::MAX))
                        .unwrap_or(0)
                } else {
                    0
                };
                let shortcut_w = if show_shortcut {
                    node.shortcut
                        .map(|s| u16::try_from(crate::text::display_cols(s)).unwrap_or(u16::MAX))
                        .unwrap_or(0)
                } else {
                    0
                };
                let right_edge = label_x.saturating_add(content_w);
                let reserve = badge_w
                    .saturating_add(actions_w)
                    .saturating_add(shortcut_w)
                    .saturating_add(u16::from(badge_w > 0 && (shortcut_w > 0 || actions_w > 0)))
                    .saturating_add(u16::from(actions_w > 0 && shortcut_w > 0));
                let mid_end = right_edge.saturating_sub(reserve);
                let primary_budget = mid_end.saturating_sub(x);
                if primary_budget > 0 {
                    let painted = if state.h_offset == 0 {
                        buffer.set_line(x, y, &node.label, primary_budget);
                        u16::try_from(node.label.width())
                            .unwrap_or(u16::MAX)
                            .min(primary_budget)
                    } else {
                        let mut visible = String::new();
                        crate::text::display_cols_slice_into(
                            &node.plain_label(),
                            usize::from(state.h_offset),
                            usize::from(primary_budget),
                            &mut visible,
                        );
                        let width = u16::try_from(crate::text::display_cols(&visible))
                            .unwrap_or(primary_budget)
                            .min(primary_budget);
                        buffer.set_stringn(x, y, &visible, usize::from(primary_budget), style);
                        width
                    };
                    x = x.saturating_add(painted);
                }
                if show_secondary && let Some(secondary) = node.secondary.as_ref() {
                    let avail = mid_end.saturating_sub(x);
                    if avail > 2 {
                        x = x.saturating_add(1);
                        let sw = u16::try_from(secondary.width())
                            .unwrap_or(u16::MAX)
                            .min(mid_end.saturating_sub(x));
                        if sw > 0 {
                            buffer.set_line(x, y, secondary, sw);
                            buffer.set_style(Rect::new(x, y, sw, 1), recipe.secondary);
                        }
                    }
                }
                let mut cursor = right_edge;
                if show_shortcut && let Some(shortcut) = node.shortcut {
                    let w = shortcut_w.min(cursor.saturating_sub(label_x));
                    if w > 0 {
                        cursor = cursor.saturating_sub(w);
                        buffer.set_stringn(cursor, y, shortcut, usize::from(w), recipe.shortcut);
                    }
                }
                if show_actions && let Some(act) = node.actions.as_ref() {
                    let w = actions_w.min(cursor.saturating_sub(label_x));
                    if w > 0 {
                        if show_shortcut {
                            cursor = cursor.saturating_sub(1);
                        }
                        cursor = cursor.saturating_sub(w);
                        buffer.set_line(cursor, y, act, w);
                        buffer.set_style(Rect::new(cursor, y, w, 1), recipe.shortcut);
                    }
                }
                if show_badge && let Some(badge) = badge {
                    let w = badge_w.min(cursor.saturating_sub(label_x));
                    if w > 0 {
                        if show_shortcut {
                            cursor = cursor.saturating_sub(1);
                        }
                        cursor = cursor.saturating_sub(w);
                        buffer.set_line(cursor, y, badge, w);
                        buffer.set_style(Rect::new(cursor, y, w, 1), recipe.trailing);
                    }
                }
                if let Some(status) = status
                    && status_w > 0
                {
                    buffer.set_stringn(
                        content_area.right().saturating_sub(status_w),
                        y,
                        status,
                        usize::from(status_w),
                        style,
                    );
                }
            }
            buffer.set_style(row, style);

            if node.enabled {
                state.regions.push(HitRegion {
                    id: node.id.clone(),
                    area: row,
                });
                if node.branch && indent < content_area.width {
                    state.disclosure_regions.push(HitRegion {
                        id: node.id.clone(),
                        area: Rect::new(disclosure_x, y, 1, 1),
                    });
                }
            }
        }

        if show_scrollbar {
            let scrollbar = Rect::new(body.right().saturating_sub(1), body.y, 1, body.height);
            state.scrollbar_region = Some(scrollbar);
            let thumb_total = if state.virtual_total > 0 {
                state.virtual_total
            } else {
                self.nodes.len()
            };
            crate::scroll::paint_scrolled_region(
                buffer,
                body,
                scrollbar,
                thumb_total,
                usize::from(body.height),
                u16::try_from(state.offset).unwrap_or(u16::MAX),
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
    use crate::style::{DesignSystem, GlyphSet, SelectionChrome};

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
}
