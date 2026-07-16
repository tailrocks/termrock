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

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TreeNodeStatus {
    Ready,
    Loading,
    Error,
}

#[derive(Debug, Clone)]
pub struct TreeNode<'a, Id> {
    pub id: Id,
    pub label: Line<'a>,
    pub depth: u16,
    pub branch: bool,
    pub expanded: bool,
    pub enabled: bool,
    pub status: TreeNodeStatus,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TreeOutcome<Id> {
    Ignored,
    SelectionChanged(Id),
    Toggle(Id),
    Activated(Id),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TreeState<Id> {
    selected: Option<Id>,
    hovered: Option<Id>,
    focused: bool,
    offset: usize,
    viewport_height: usize,
    follow_selection: bool,
    regions: Vec<HitRegion<Id>>,
    disclosure_regions: Vec<HitRegion<Id>>,
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
            scrollbar_region: None,
        }
    }
}

impl<Id> TreeState<Id> {
    #[must_use]
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
            scrollbar_region: None,
        }
    }

    #[must_use]
    pub const fn selected(&self) -> Option<&Id> {
        self.selected.as_ref()
    }

    #[must_use]
    pub const fn hovered(&self) -> Option<&Id> {
        self.hovered.as_ref()
    }

    #[must_use]
    pub const fn is_focused(&self) -> bool {
        self.focused
    }

    pub const fn set_focused(&mut self, focused: bool) {
        self.focused = focused;
    }

    #[must_use]
    pub const fn offset(&self) -> usize {
        self.offset
    }

    pub fn select(&mut self, selected: Option<Id>) {
        self.selected = selected;
        self.follow_selection = true;
    }

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
    pub fn regions(&self) -> &[HitRegion<Id>] {
        &self.regions
    }
}

impl<Id: Clone + PartialEq> TreeState<Id> {
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
            _ => TreeOutcome::Ignored,
        }
    }

    pub fn hover(&mut self, position: Position) -> Option<&Id> {
        self.hovered = self
            .regions
            .iter()
            .find(|region| region.area.contains(position))
            .map(|region| region.id.clone());
        self.hovered.as_ref()
    }

    pub fn click(&mut self, position: Position) -> TreeOutcome<Id> {
        if let Some(region) = self
            .disclosure_regions
            .iter()
            .find(|region| region.area.contains(position))
        {
            return TreeOutcome::Toggle(region.id.clone());
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
pub struct Tree<'a, Id> {
    pub nodes: &'a [TreeNode<'a, Id>],
    pub theme: &'a Theme,
}

impl<Id: Clone + PartialEq> StatefulWidget for &Tree<'_, Id> {
    type State = TreeState<Id>;

    fn render(self, area: Rect, buffer: &mut Buffer, state: &mut Self::State) {
        state.regions.clear();
        state.disclosure_regions.clear();
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
            let status = match node.status {
                TreeNodeStatus::Ready => None,
                TreeNodeStatus::Loading => Some(" loading"),
                TreeNodeStatus::Error => Some(" error"),
            };
            let status_width = status
                .map(crate::display_cols)
                .and_then(|width| u16::try_from(width).ok())
                .filter(|width| *width <= content_area.width)
                .unwrap_or(0);
            let label_x = disclosure_x.saturating_add(2);
            let used = label_x.saturating_sub(content_area.x);
            if used < content_area.width {
                buffer.set_line(
                    label_x,
                    y,
                    &node.label,
                    content_area
                        .width
                        .saturating_sub(used)
                        .saturating_sub(status_width),
                );
            }
            if let Some(status) = status
                && status_width > 0
            {
                buffer.set_stringn(
                    content_area.right().saturating_sub(status_width),
                    y,
                    status,
                    usize::from(status_width),
                    style,
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
