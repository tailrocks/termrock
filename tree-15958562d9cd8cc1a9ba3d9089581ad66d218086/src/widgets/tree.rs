use ratatui_core::{
    buffer::Buffer,
    layout::{Position, Rect},
    style::Modifier,
    text::Line,
    widgets::StatefulWidget,
};

use crate::{
    input::{KeyCode, KeyEvent, KeyEventKind},
    interaction::HitRegion,
    scroll::max_offset,
    style::{Role, Theme},
};

use super::Selection;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[non_exhaustive]
/// Available `TreeNodeStatus` choices.
pub enum TreeNodeStatus {
    /// Selects the `Ready` behavior.
    Ready,
    /// Selects the `Loading` behavior.
    Loading,
    /// Selects the `Error` behavior.
    Error,
}

#[derive(Debug, Clone)]
/// Data carried by `TreeNode`.
pub struct TreeNode<'a, Id> {
    /// Documentation for `item`.
    pub id: Id,
    /// Documentation for `item`.
    pub label: Line<'a>,
    /// Documentation for `item`.
    pub trailing: Option<Line<'a>>,
    /// Documentation for `item`.
    pub depth: u16,
    /// Documentation for `item`.
    pub branch: bool,
    /// Documentation for `item`.
    pub expanded: bool,
    /// Documentation for `item`.
    pub enabled: bool,
    /// Documentation for `item`.
    pub status: TreeNodeStatus,
}

#[derive(Debug, Clone, PartialEq, Eq)]
#[non_exhaustive]
/// Available `TreeOutcome` choices.
pub enum TreeOutcome<Id> {
    /// Selects the `Ignored` behavior.
    Ignored,
    /// Selects the `SelectionChanged` behavior.
    SelectionChanged(Id),
    /// Selects the `Toggle` behavior.
    Toggle(Id),
    /// Selects the `CheckToggled` behavior.
    CheckToggled(Id),
    /// Selects the `Activated` behavior.
    Activated(Id),
}

#[derive(Debug, Clone, PartialEq, Eq)]
/// Runtime state for `Tree`.
pub struct TreeState<Id> {
    selected: Option<Id>,
    hovered: Option<Id>,
    focused: bool,
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
            focused: false,
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
    /// Creates a new value with canonical defaults.
    pub const fn new(selected: Option<Id>) -> Self {
        Self {
            selected,
            hovered: None,
            focused: true,
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
    /// Performs the `selected` operation.
    pub const fn selected(&self) -> Option<&Id> {
        self.selected.as_ref()
    }

    #[must_use]
    /// Performs the `hovered` operation.
    pub const fn hovered(&self) -> Option<&Id> {
        self.hovered.as_ref()
    }

    #[must_use]
    /// Returns whether `focused`.
    pub const fn is_focused(&self) -> bool {
        self.focused
    }

    /// Sets `focused`.
    pub const fn set_focused(&mut self, focused: bool) {
        self.focused = focused;
    }

    #[must_use]
    /// Performs the `offset` operation.
    pub const fn offset(&self) -> usize {
        self.offset
    }

    /// Performs the `select` operation.
    pub fn select(&mut self, selected: Option<Id>) {
        self.selected = selected;
        self.follow_selection = true;
    }

    /// Performs the `enable_multi_select` operation.
    pub fn enable_multi_select(&mut self) {
        self.selection.get_or_insert_with(Selection::new);
    }

    /// Performs the `disable_multi_select` operation.
    pub fn disable_multi_select(&mut self) {
        self.selection = None;
    }

    #[must_use]
    /// Performs the `selection` operation.
    pub const fn selection(&self) -> Option<&Selection<Id>> {
        self.selection.as_ref()
    }

    /// Performs the `selection_mut` operation.
    pub fn selection_mut(&mut self) -> Option<&mut Selection<Id>> {
        self.selection.as_mut()
    }

    /// Performs the `scroll_by` operation.
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

    /// Performs the `scroll_to_position` operation.
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
    /// Performs the `regions` operation.
    pub fn regions(&self) -> &[HitRegion<Id>] {
        &self.regions
    }
}

impl<Id: Clone + PartialEq> TreeState<Id> {
    /// Handles the `handle_key` interaction.
    pub fn handle_key(&mut self, nodes: &[TreeNode<'_, Id>], key: KeyEvent) -> TreeOutcome<Id> {
        if !self.focused || key.kind == KeyEventKind::Release {
            return TreeOutcome::Ignored;
        }
        match key.code {
            KeyCode::Up => self.move_selection(nodes, -1),
            KeyCode::Down => self.move_selection(nodes, 1),
            KeyCode::Home => self.select_boundary(nodes, false),
            KeyCode::End => self.select_boundary(nodes, true),
            KeyCode::PageUp => self.page_selection(nodes, false),
            KeyCode::PageDown => self.page_selection(nodes, true),
            KeyCode::Left => self.collapse_or_parent(nodes),
            KeyCode::Right => self.expand(nodes),
            KeyCode::Enter => self
                .selected_node(nodes)
                .map_or(TreeOutcome::Ignored, |node| {
                    TreeOutcome::Activated(node.id.clone())
                }),
            KeyCode::Char(' ') => self.toggle_selected(nodes),
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

    /// Performs the `hover` operation.
    pub fn hover(&mut self, position: Position) -> Option<&Id> {
        self.hovered = self
            .regions
            .iter()
            .find(|region| region.area.contains(position))
            .map(|region| region.id.clone());
        self.hovered.as_ref()
    }

    /// Performs the `click` operation.
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
/// Data carried by `Tree`.
pub struct Tree<'a, Id> {
    nodes: &'a [TreeNode<'a, Id>],
    theme: &'a Theme,
}

impl<'a, Id> Tree<'a, Id> {
    #[must_use]
    /// Creates a new value with canonical defaults.
    pub const fn new(nodes: &'a [TreeNode<'a, Id>], theme: &'a Theme) -> Self {
        Self { nodes, theme }
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
        if area.is_empty() || self.nodes.is_empty() {
            state.offset = 0;
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
        let trailing_width = self
            .nodes
            .iter()
            .filter_map(|node| node.trailing.as_ref())
            .map(Line::width)
            .max()
            .and_then(|width| u16::try_from(width).ok())
            .unwrap_or(0)
            .min(content_area.width);
        let trailing_x = content_area.right().saturating_sub(trailing_width);

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
            let mut style = match node.status {
                TreeNodeStatus::Ready if node.enabled => self.theme.style(Role::Text),
                TreeNodeStatus::Ready => self.theme.style(Role::TextDisabled),
                TreeNodeStatus::Loading => self.theme.style(Role::TextMuted),
                TreeNodeStatus::Error => self.theme.style(Role::Danger),
            };
            if !node.enabled {
                style = style.add_modifier(Modifier::DIM);
            }
            if selected && node.enabled && state.focused {
                style = style
                    .patch(self.theme.style(Role::Selection))
                    .add_modifier(Modifier::BOLD);
            } else if selected && node.enabled {
                style = style
                    .patch(self.theme.style(Role::Accent))
                    .add_modifier(Modifier::UNDERLINED);
            } else if hovered && node.enabled {
                style = style
                    .patch(self.theme.style(Role::Focus))
                    .add_modifier(Modifier::UNDERLINED);
            } else if checked && node.enabled {
                style = style.patch(self.theme.style(Role::Accent));
            }

            let indent = node.depth.saturating_mul(2).min(content_area.width);
            let disclosure_x = content_area.x.saturating_add(indent);
            let glyph = if node.branch {
                if node.expanded { "▾" } else { "▸" }
            } else {
                " "
            };
            if indent < content_area.width {
                buffer.set_stringn(disclosure_x, y, glyph, 1, style);
            }
            let check_x = disclosure_x.saturating_add(2);
            if state.selection.is_some() && check_x < content_area.right() {
                buffer.set_stringn(
                    check_x,
                    y,
                    if checked { "[x] " } else { "[ ] " },
                    usize::from(content_area.right().saturating_sub(check_x).min(4)),
                    style,
                );
                if node.enabled && content_area.right().saturating_sub(check_x) >= 3 {
                    state.check_regions.push(HitRegion {
                        id: node.id.clone(),
                        area: Rect::new(check_x, y, 3, 1),
                    });
                }
            }
            let label_x = check_x.saturating_add(u16::from(state.selection.is_some()) * 4);
            let status = match node.status {
                TreeNodeStatus::Ready => None,
                TreeNodeStatus::Loading => Some(" loading"),
                TreeNodeStatus::Error => Some(" error"),
            };
            let metadata_gap = u16::from(trailing_width > 0);
            let status_end = trailing_x.saturating_sub(metadata_gap);
            let status_width = status
                .map(crate::text::display_cols)
                .and_then(|width| u16::try_from(width).ok())
                .filter(|width| status_end.saturating_sub(*width) >= label_x)
                .unwrap_or(0);
            let used = label_x.saturating_sub(content_area.x);
            if used < content_area.width {
                let label_end = status_end.saturating_sub(status_width);
                buffer.set_line(label_x, y, &node.label, label_end.saturating_sub(label_x));
            }
            if let Some(status) = status
                && status_width > 0
            {
                buffer.set_stringn(
                    status_end.saturating_sub(status_width),
                    y,
                    status,
                    usize::from(status_width),
                    style,
                );
            }
            if let Some(trailing) = node.trailing.as_ref()
                && trailing_width > 0
            {
                let width = u16::try_from(trailing.width())
                    .unwrap_or(u16::MAX)
                    .min(trailing_width);
                buffer.set_line(
                    content_area.right().saturating_sub(width),
                    y,
                    trailing,
                    width,
                );
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
                buffer.set_string(scrollbar.x, y, "│", self.theme.style(Role::ScrollTrack));
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
                        self.theme.style(Role::ScrollThumb),
                    );
                }
            }
        }
    }
}

impl<Id: Clone + PartialEq> StatefulWidget for Tree<'_, Id> {
    type State = TreeState<Id>;

    fn render(self, area: Rect, buffer: &mut Buffer, state: &mut Self::State) {
        StatefulWidget::render(&self, area, buffer, state);
    }
}
