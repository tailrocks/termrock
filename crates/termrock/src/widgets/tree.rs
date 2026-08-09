use ratatui_core::{
    buffer::Buffer,
    layout::{Position, Rect},
    style::Modifier,
    text::Line,
    widgets::StatefulWidget,
};

use crate::{
    input::KeyEvent,
    interaction::HitRegion,
    scroll::max_offset,
    style::{
        DesignSystem,
        ListRowVisualState,
        Role,
    },
};

use super::{ComposedRow, Selection};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[non_exhaustive]
/// Loading and error states associated with a tree node.
pub enum TreeNodeStatus {
    /// Node content is available for ordinary interaction.
    Ready,
    /// Node content is still being loaded.
    Loading,
    /// Node content could not be loaded.
    Error,
}

#[derive(Debug, Clone)]
/// A stable flattened tree row with hierarchy metadata.
pub struct TreeNode<'a, Id> {
    /// Stable identity used for selection and activation.
    pub id: Id,
    /// Caller-visible label.
    pub label: Line<'a>,
    /// Optional leading status/icon (composed leading).
    pub leading: Option<Line<'a>>,
    /// Optional secondary metadata (composed secondary).
    pub secondary: Option<Line<'a>>,
    /// Optional badge (composed badge; preferred over trailing when both set).
    pub badge: Option<Line<'a>>,
    /// Optional keyboard shortcut hint.
    pub shortcut: Option<&'a str>,
    /// Optional metadata aligned at the trailing edge (maps to badge when badge unset).
    pub trailing: Option<Line<'a>>,
    /// Zero-based hierarchy depth.
    pub depth: u16,
    /// Whether the node can request disclosure changes.
    pub branch: bool,
    /// Whether this item is expanded.
    pub expanded: bool,
    /// Whether this item is enabled.
    pub enabled: bool,
    /// Optional loading or error state.
    pub status: TreeNodeStatus,
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
            trailing: None,
            depth,
            branch: false,
            expanded: false,
            enabled: true,
            status: TreeNodeStatus::Ready,
        }
    }

    /// Creates a branch node (disclosure-capable).
    #[must_use]
    pub fn branch(mut self) -> Self {
        self.branch = true;
        self
    }

    /// Marks the branch expanded (consumer owns expansion policy).
    #[must_use]
    pub fn expanded(mut self) -> Self {
        self.expanded = true;
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

    /// Sets legacy trailing metadata (badge fallback).
    #[must_use]
    pub fn trailing(mut self, trailing: Line<'a>) -> Self {
        self.trailing = Some(trailing);
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

    /// Explicit status override.
    #[must_use]
    pub fn with_status(mut self, status: TreeNodeStatus) -> Self {
        self.status = status;
        self
    }

    /// Projects hierarchy chrome + label into shared composed anatomy.
    #[must_use]
    pub fn composed(&self) -> ComposedRow<'a, ()> {
        ComposedRow {
            id: (),
            leading: self.leading.clone(),
            primary: self.label.clone(),
            secondary: self.secondary.clone(),
            badge: self.badge.clone().or_else(|| self.trailing.clone()),
            shortcut: self.shortcut,
            enabled: self.enabled,
            loading: matches!(self.status, TreeNodeStatus::Loading),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
#[non_exhaustive]
/// Semantic results produced by tree interaction.
pub enum TreeOutcome<Id> {
    /// The event produced no tree-state change.
    Ignored,
    /// Navigation selected this stable node identity.
    SelectionChanged(Id),
    /// The identified branch requested disclosure inversion.
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
pub struct TreeState<Id> {
    selected: Option<Id>,
    hovered: Option<Id>,
    offset: usize,
    viewport_height: usize,
    follow_selection: bool,
    regions: Vec<HitRegion<Id>>,
    disclosure_regions: Vec<HitRegion<Id>>,
    selection: Option<Selection<Id>>,
    check_regions: Vec<HitRegion<Id>>,
    scrollbar_region: Option<Rect>,
}

impl<Id> Default for TreeState<Id> {
    fn default() -> Self {
        Self {
            selected: None,
            hovered: None,
            offset: 0,
            viewport_height: 0,
            follow_selection: false,
            regions: Vec::new(),
            disclosure_regions: Vec::new(),
            selection: None,
            check_regions: Vec::new(),
            scrollbar_region: None,
        }
    }
}

impl<Id> TreeState<Id> {
    #[must_use]
    /// Creates tree state with no selection, hover, expansion, or scroll.
    pub const fn new(selected: Option<Id>) -> Self {
        Self {
            selected,
            hovered: None,
            offset: 0,
            viewport_height: 0,
            follow_selection: true,
            regions: Vec::new(),
            disclosure_regions: Vec::new(),
            selection: None,
            check_regions: Vec::new(),
            scrollbar_region: None,
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

    /// Selects the item with the supplied stable identity.
    pub fn select(&mut self, selected: Option<Id>) {
        self.selected = selected;
        self.follow_selection = true;
    }

    /// Enables ordered multi-selection with an empty selection.
    pub fn enable_multi_select(&mut self) {
        self.selection.get_or_insert_with(Selection::new);
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
    /// Routes navigation, disclosure, checking, and activation keys.
    pub fn handle_key(&mut self, nodes: &[TreeNode<'_, Id>], key: KeyEvent) -> TreeOutcome<Id> {
        if let Some(intent) = crate::interaction::default_tree_intent(key) {
            return self.handle_intent(nodes, intent);
        }
        TreeOutcome::Ignored
    }

    /// Routes a semantic intent (keymap / scene adapter).
    pub fn handle_intent(
        &mut self,
        nodes: &[TreeNode<'_, Id>],
        intent: crate::interaction::UiIntent,
    ) -> TreeOutcome<Id> {
                use crate::interaction::{NavigationMove, PageMove, UiIntent};
        match intent {
            UiIntent::Move(NavigationMove::Previous) => self.move_selection(nodes, -1),
            UiIntent::Move(NavigationMove::Next) => self.move_selection(nodes, 1),
            UiIntent::Move(NavigationMove::First) => self.select_boundary(nodes, false),
            UiIntent::Move(NavigationMove::Last) => self.select_boundary(nodes, true),
            UiIntent::Page(PageMove::Backward) => self.page_selection(nodes, false),
            UiIntent::Page(PageMove::Forward) => self.page_selection(nodes, true),
            UiIntent::Collapse => self.collapse_or_parent(nodes),
            UiIntent::Expand => self.expand(nodes),
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
        self.follow_selection = true;
        TreeOutcome::SelectionChanged(node.id.clone())
    }

    fn collapse_or_parent(&mut self, nodes: &[TreeNode<'_, Id>]) -> TreeOutcome<Id> {
        let Some(index) = self.selected_index(nodes) else {
            return TreeOutcome::Ignored;
        };
        let node = &nodes[index];
        if node.enabled && node.branch && node.expanded {
            return TreeOutcome::Toggle(node.id.clone());
        }
        let parent = nodes[..index]
            .iter()
            .rposition(|candidate| candidate.enabled && candidate.depth < node.depth);
        self.select_index(nodes, parent)
    }

    fn expand(&self, nodes: &[TreeNode<'_, Id>]) -> TreeOutcome<Id> {
        self.selected_node(nodes)
            .map_or(TreeOutcome::Ignored, |node| {
                if node.branch && !node.expanded {
                    TreeOutcome::Toggle(node.id.clone())
                } else {
                    TreeOutcome::Ignored
                }
            })
    }
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

    /// Preferred paint root from [`DesignSystem`].
    #[must_use]
    pub const fn from_system(nodes: &'a [TreeNode<'a, Id>], system: &'a DesignSystem) -> Self {
        Self::new(nodes, system)
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
        state.viewport_height = usize::from(area.height);
        if area.is_empty() {
            state.offset = 0;
            return;
        }
        if self.nodes.is_empty() {
            state.offset = 0;
            if let Some(message) = self.empty_message {
                let style = self.tokens.style(Role::TextMuted);
                buffer.set_stringn(area.x, area.y, message, usize::from(area.width), style);
            }
            return;
        }

        if state.follow_selection
            && let Some(selected) = state.selected_index(self.nodes)
        {
            if selected < state.offset {
                state.offset = selected;
            } else if selected >= state.offset.saturating_add(usize::from(area.height)) {
                state.offset = selected.saturating_sub(usize::from(area.height).saturating_sub(1));
            }
        }
        state.follow_selection = false;
        state.offset = state
            .offset
            .min(max_offset(self.nodes.len(), usize::from(area.height)));
        let show_scrollbar =
            crate::scroll::is_scrollable(self.nodes.len(), usize::from(area.height))
                && area.width > 1;
        let content_area = Rect {
            width: area.width.saturating_sub(u16::from(show_scrollbar)),
            ..area
        };
        // Density indent; collapse under narrow pressure (tiny → 0).
        let indent_step = match content_area.width {
            0..=7 => 0,
            8..=11 => 1,
            _ => self.tokens.density.tree_indent().max(1),
        };
        for (visible, node) in self
            .nodes
            .iter()
            .skip(state.offset)
            .take(usize::from(area.height))
            .enumerate()
        {
            let y = area
                .y
                .saturating_add(u16::try_from(visible).unwrap_or(u16::MAX));
            let row = Rect::new(content_area.x, y, content_area.width, 1);
            let selected = state.selected.as_ref() == Some(&node.id);
            let hovered = state.hovered.as_ref() == Some(&node.id);
            let checked = state
                .selection
                .as_ref()
                .is_some_and(|selection| selection.is_checked(&node.id));
            let loading = matches!(node.status, TreeNodeStatus::Loading);
            let recipe = self.tokens.resolve_list_row(ListRowVisualState {
                selected,
                focused: self.focused && selected,
                hovered,
                enabled: node.enabled,
                loading,
                checked,
            });
            let mut style = match node.status {
                TreeNodeStatus::Ready if node.enabled => recipe.label,
                TreeNodeStatus::Ready => self.tokens.style(Role::TextDisabled),
                // Loading stays muted even when interaction-disabled.
                TreeNodeStatus::Loading => self.tokens.style(Role::TextMuted),
                TreeNodeStatus::Error => self.tokens.style(Role::Danger),
            };
            if !node.enabled {
                style = style.add_modifier(Modifier::DIM);
            }
            if selected && node.enabled {
                style = recipe.label;
                if self.focused {
                    style = style.add_modifier(Modifier::BOLD);
                    if recipe.show_focus_underline {
                        style = style.add_modifier(Modifier::UNDERLINED);
                    }
                } else {
                    style = style.add_modifier(Modifier::UNDERLINED);
                }
            } else if hovered && node.enabled {
                style = recipe.hover.add_modifier(Modifier::UNDERLINED);
            } else if checked && node.enabled {
                style = style.patch(self.tokens.style(Role::Accent));
            }
            if recipe.use_fill && selected {
                buffer.set_style(row, style);
            } else if recipe.hover_fill {
                buffer.set_style(row, recipe.hover);
            }

            // Quiet selection gutter (aligned with List) when Gutter chrome.
            let mut x_cursor = content_area.x;
            if let Some((gutter_glyph, gutter_style)) = recipe.gutter {
                buffer.set_stringn(x_cursor, y, gutter_glyph, 1, gutter_style);
                x_cursor = x_cursor.saturating_add(2);
            } else if recipe.show_gutter_slot
                && matches!(
                    self.tokens.selection,
                    crate::style::SelectionChrome::Gutter
                        | crate::style::SelectionChrome::Tint
                        | crate::style::SelectionChrome::Fill
                )
            {
                // Reserve gutter column only when selection chrome uses a slot.
                // For tree, reserve only if any selection likely — keep 2 cells when
                // tokens use Gutter default (DesignSystem::phosphor).
                if matches!(
                    self.tokens.selection,
                    crate::style::SelectionChrome::Gutter
                ) {
                    x_cursor = x_cursor.saturating_add(2);
                }
            }

            let max_indent = content_area
                .right()
                .saturating_sub(x_cursor)
                .saturating_sub(4);
            let indent = node
                .depth
                .saturating_mul(indent_step)
                .min(max_indent);
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
                        buffer.set_stringn(
                            check_x.saturating_add(paint_w),
                            y,
                            " ",
                            1,
                            style,
                        );
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
                // Fit-based: keep trailing badge whenever it still fits next to
                // a one-cell primary identity (mirrors ComposedRow budgets).
                let badge = node.badge.as_ref().or(node.trailing.as_ref());
                let badge_need = badge
                    .map(|b| {
                        u16::try_from(b.width())
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
                let mut budget = content_w.saturating_sub(1); // primary min
                let show_shortcut = node.shortcut.is_some() && budget >= shortcut_need;
                if show_shortcut {
                    budget = budget.saturating_sub(shortcut_need);
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
                let shortcut_w = if show_shortcut {
                    node.shortcut
                        .map(|s| u16::try_from(crate::text::display_cols(s)).unwrap_or(u16::MAX))
                        .unwrap_or(0)
                } else {
                    0
                };
                let right_edge = label_x.saturating_add(content_w);
                let reserve = badge_w
                    .saturating_add(shortcut_w)
                    .saturating_add(u16::from(badge_w > 0 && shortcut_w > 0));
                let mid_end = right_edge.saturating_sub(reserve);
                let primary_budget = mid_end.saturating_sub(x);
                if primary_budget > 0 {
                    buffer.set_line(x, y, &node.label, primary_budget);
                    x = x.saturating_add(
                        u16::try_from(node.label.width())
                            .unwrap_or(u16::MAX)
                            .min(primary_budget),
                    );
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
                        buffer.set_stringn(
                            cursor,
                            y,
                            shortcut,
                            usize::from(w),
                            recipe.shortcut,
                        );
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
            let scrollbar = Rect::new(area.right().saturating_sub(1), area.y, 1, area.height);
            state.scrollbar_region = Some(scrollbar);
            for y in scrollbar.top()..scrollbar.bottom() {
                buffer.set_string(
                    scrollbar.x,
                    y,
                    "│",
                    self.tokens.style(Role::ScrollTrack),
                );
            }
            if let Some(thumb) = crate::scroll::full_cell_thumb(
                self.nodes.len(),
                usize::from(area.height),
                area.height,
                state.offset,
            ) {
                for y in thumb.start..thumb.start.saturating_add(thumb.len) {
                    buffer.set_string(
                        scrollbar.x,
                        scrollbar.y.saturating_add(y),
                        "█",
                        self.tokens.style(Role::ScrollThumb),
                    );
                }
            }
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
    use crate::style::{
        DesignSystem,
        GlyphSet,
        SelectionChrome,
    };

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
    fn empty_message_and_from_system() {
        let system = DesignSystem::phosphor();
        let nodes: [TreeNode<'_, &str>; 0] = [];
        let mut state = TreeState::<&str>::default();
        let area = Rect::new(0, 0, 24, 2);
        let mut buffer = Buffer::empty(area);
        let tree = Tree::from_system(&nodes, &system).empty_message("No files");
        tree.render(area, &mut buffer, &mut state);
        assert_eq!(buffer[(0, 0)].symbol(), "N");
    }

    #[test]
    fn ascii_disclosure_and_gutter() {
        let tokens = DesignSystem::default()
            .glyphs(GlyphSet::Ascii)
            .selection(SelectionChrome::Gutter);
        let nodes = [
            TreeNode::new("a", Line::from("A"), 0).branch().expanded(),
            TreeNode::new("b", Line::from("B"), 1),
        ];
        let mut state = TreeState::new(Some("a"));
        let area = Rect::new(0, 0, 20, 2);
        let mut buffer = Buffer::empty(area);
        (&Tree::new(&nodes, &tokens)).render(area, &mut buffer, &mut state);
        // gutter + disclosure
        assert_eq!(buffer[(0, 0)].symbol(), ">");
        // disclosure open ascii after gutter slot (2) + indent 0
        assert_eq!(buffer[(2, 0)].symbol(), "v");
    }

    #[test]
    fn density_indent_dashboard_tighter() {
        let comfortable = DesignSystem::new(
            crate::style::RolePalette::default(),
            crate::style::Density::Comfortable,
        )
        .selection(SelectionChrome::Gutter);
        let dashboard = DesignSystem::new(
            crate::style::RolePalette::default(),
            crate::style::Density::Dashboard,
        )
        .selection(SelectionChrome::Gutter);
        let nodes = [TreeNode::new("c", Line::from("Child"), 2)];
        let mut state = TreeState::new(None);
        let area = Rect::new(0, 0, 40, 1);
        let mut buf_c = Buffer::empty(area);
        let mut buf_d = Buffer::empty(area);
        (&Tree::new(&nodes, &comfortable)).render(area, &mut buf_c, &mut state);
        (&Tree::new(&nodes, &dashboard)).render(area, &mut buf_d, &mut state);
        // Find first letter 'C' column — dashboard should paint earlier (less indent).
        let col = |buf: &Buffer| {
            (0..40)
                .find(|&x| buf[(x, 0)].symbol() == "C")
                .expect("label")
        };
        assert!(col(&buf_d) < col(&buf_c), "dashboard indent tighter");
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
}
