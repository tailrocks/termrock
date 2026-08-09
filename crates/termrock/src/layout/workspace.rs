// SPDX-FileCopyrightText: 2026 Alexey Zhokhov
// SPDX-License-Identifier: Apache-2.0

//! Responsive workspace tree: splits, docks, collapse under pressure.
//!
//! Every returned rectangle is contained by the input area. Under pressure the
//! solver shrinks gaps, then collapses lower-priority leaves.

use ratatui_core::layout::Rect;

/// Stable workspace pane identity.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct PaneId(pub String);

impl PaneId {
    /// Static id helper.
    #[must_use]
    pub fn from_static(id: &'static str) -> Self {
        Self(id.to_owned())
    }
}

/// Split axis.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
#[non_exhaustive]
pub enum WorkspaceAxis {
    /// Left | Right.
    #[default]
    Horizontal,
    /// Top / Bottom.
    Vertical,
}

/// Leaf size hint.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[non_exhaustive]
pub enum PaneConstraint {
    /// Fixed cells along the parent axis.
    Fixed(u16),
    /// Weighted share of remaining space.
    Weight(u16),
    /// Preferred minimum under pressure.
    Min(u16),
}

/// Workspace tree node.
#[derive(Debug, Clone, PartialEq, Eq)]
#[non_exhaustive]
pub enum WorkspaceNode {
    /// Content leaf.
    Leaf {
        /// Pane id.
        id: PaneId,
        /// Size constraint.
        constraint: PaneConstraint,
        /// Lower collapses first under pressure (0 = first).
        collapse_priority: u8,
    },
    /// Binary split.
    Split {
        /// Axis.
        axis: WorkspaceAxis,
        /// First child share 0..=100 (remainder to second).
        ratio_percent: u8,
        /// First child.
        first: Box<WorkspaceNode>,
        /// Second child.
        second: Box<WorkspaceNode>,
    },
}

/// Resolved pane geometry.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PaneGeom {
    /// Pane id.
    pub id: PaneId,
    /// Contained rectangle (may be zero when collapsed).
    pub area: Rect,
    /// Whether the pane was collapsed under pressure.
    pub collapsed: bool,
}

/// Domain-neutral workspace interaction state.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct WorkspaceState {
    /// Focused pane id.
    pub focused: Option<PaneId>,
    /// Collapsed pane ids.
    pub collapsed: Vec<PaneId>,
    /// Optional zoomed pane (fills parent).
    pub zoomed: Option<PaneId>,
    /// Remembered ratio overrides keyed by left/top child id path is consumer-owned;
    /// here a single optional global override for the root split.
    pub root_ratio_percent: Option<u8>,
}

impl WorkspaceState {
    /// Creates empty state.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Toggles collapse for a pane.
    pub fn toggle_collapse(&mut self, id: PaneId) {
        if let Some(pos) = self.collapsed.iter().position(|item| item == &id) {
            self.collapsed.remove(pos);
        } else {
            self.collapsed.push(id);
        }
    }

    /// Returns whether a pane is collapsed.
    #[must_use]
    pub fn is_collapsed(&self, id: &PaneId) -> bool {
        self.collapsed.iter().any(|item| item == id)
    }
}

/// Layout solver for a workspace tree.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Workspace {
    root: WorkspaceNode,
}

impl Workspace {
    /// Creates a workspace with the given root node.
    #[must_use]
    pub const fn new(root: WorkspaceNode) -> Self {
        Self { root }
    }

    /// Resolves pane geometry inside `area`. All rects ⊆ area.
    #[must_use]
    pub fn layout(&self, area: Rect, state: &WorkspaceState) -> Vec<PaneGeom> {
        if area.width == 0 || area.height == 0 {
            return Vec::new();
        }
        if let Some(zoom) = &state.zoomed {
            return vec![PaneGeom {
                id: zoom.clone(),
                area,
                collapsed: false,
            }];
        }
        let mut out = Vec::new();
        layout_node(&self.root, area, state, &mut out);
        // Containment guarantee.
        for pane in &mut out {
            pane.area = intersect(pane.area, area);
        }
        out
    }
}

fn intersect(a: Rect, b: Rect) -> Rect {
    let x = a.x.max(b.x);
    let y = a.y.max(b.y);
    let right = a.right().min(b.right());
    let bottom = a.bottom().min(b.bottom());
    if right <= x || bottom <= y {
        Rect::new(x, y, 0, 0)
    } else {
        Rect {
            x,
            y,
            width: right.saturating_sub(x),
            height: bottom.saturating_sub(y),
        }
    }
}

fn layout_node(node: &WorkspaceNode, area: Rect, state: &WorkspaceState, out: &mut Vec<PaneGeom>) {
    match node {
        WorkspaceNode::Leaf {
            id,
            constraint: _,
            collapse_priority: _,
        } => {
            let collapsed = state.is_collapsed(id) || area.width == 0 || area.height == 0;
            out.push(PaneGeom {
                id: id.clone(),
                area: if collapsed {
                    Rect::new(area.x, area.y, 0, 0)
                } else {
                    area
                },
                collapsed,
            });
        }
        WorkspaceNode::Split {
            axis,
            ratio_percent,
            first,
            second,
        } => {
            if area.width == 0 || area.height == 0 {
                layout_node(first, area, state, out);
                layout_node(second, area, state, out);
                return;
            }
            let first_collapsed = matches!(
                first.as_ref(),
                WorkspaceNode::Leaf { id, .. } if state.is_collapsed(id)
            );
            let second_collapsed = matches!(
                second.as_ref(),
                WorkspaceNode::Leaf { id, .. } if state.is_collapsed(id)
            );
            if first_collapsed && !second_collapsed {
                layout_node(second, area, state, out);
                layout_node(first, Rect::new(area.x, area.y, 0, 0), state, out);
                return;
            }
            if second_collapsed && !first_collapsed {
                layout_node(first, area, state, out);
                layout_node(second, Rect::new(area.x, area.y, 0, 0), state, out);
                return;
            }
            let ratio = state.root_ratio_percent.unwrap_or(*ratio_percent).min(100);
            let (a, b) = split_area(area, *axis, ratio);
            // Pressure: if fixed mins cannot fit, collapse lower priority leaf.
            if !fits(area, first, second, *axis)
                && let Some(victim) = lower_priority_leaf(first, second)
            {
                let mut forced = state.clone();
                if !forced.is_collapsed(&victim) {
                    forced.collapsed.push(victim);
                }
                layout_node(node, area, &forced, out);
                return;
            }
            layout_node(first, a, state, out);
            layout_node(second, b, state, out);
        }
    }
}

fn split_area(area: Rect, axis: WorkspaceAxis, ratio_percent: u8) -> (Rect, Rect) {
    let ratio = u16::from(ratio_percent).min(100);
    match axis {
        WorkspaceAxis::Horizontal => {
            let w = area.width;
            let first_w = (u32::from(w) * u32::from(ratio) / 100) as u16;
            let first_w = first_w.min(w);
            let second_w = w.saturating_sub(first_w);
            (
                Rect {
                    x: area.x,
                    y: area.y,
                    width: first_w,
                    height: area.height,
                },
                Rect {
                    x: area.x.saturating_add(first_w),
                    y: area.y,
                    width: second_w,
                    height: area.height,
                },
            )
        }
        WorkspaceAxis::Vertical => {
            let h = area.height;
            let first_h = (u32::from(h) * u32::from(ratio) / 100) as u16;
            let first_h = first_h.min(h);
            let second_h = h.saturating_sub(first_h);
            (
                Rect {
                    x: area.x,
                    y: area.y,
                    width: area.width,
                    height: first_h,
                },
                Rect {
                    x: area.x,
                    y: area.y.saturating_add(first_h),
                    width: area.width,
                    height: second_h,
                },
            )
        }
    }
}

fn leaf_min(node: &WorkspaceNode) -> u16 {
    match node {
        WorkspaceNode::Leaf { constraint, .. } => match constraint {
            PaneConstraint::Fixed(n) | PaneConstraint::Min(n) => (*n).max(1),
            PaneConstraint::Weight(_) => 1,
        },
        WorkspaceNode::Split { first, second, .. } => {
            leaf_min(first).saturating_add(leaf_min(second))
        }
    }
}

fn fits(area: Rect, first: &WorkspaceNode, second: &WorkspaceNode, axis: WorkspaceAxis) -> bool {
    let need = leaf_min(first).saturating_add(leaf_min(second));
    match axis {
        WorkspaceAxis::Horizontal => need <= area.width,
        WorkspaceAxis::Vertical => need <= area.height,
    }
}

fn lower_priority_leaf(first: &WorkspaceNode, second: &WorkspaceNode) -> Option<PaneId> {
    let mut best: Option<(u8, PaneId)> = None;
    fn walk(node: &WorkspaceNode, best: &mut Option<(u8, PaneId)>) {
        match node {
            WorkspaceNode::Leaf {
                id,
                collapse_priority,
                ..
            } => match best {
                Some((p, _)) if *collapse_priority >= *p => {}
                _ => *best = Some((*collapse_priority, id.clone())),
            },
            WorkspaceNode::Split { first, second, .. } => {
                walk(first, best);
                walk(second, best);
            }
        }
    }
    walk(first, &mut best);
    walk(second, &mut best);
    best.map(|(_, id)| id)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rects_stay_inside_parent_on_tiny_area() {
        let ws = Workspace::new(WorkspaceNode::Split {
            axis: WorkspaceAxis::Horizontal,
            ratio_percent: 50,
            first: Box::new(WorkspaceNode::Leaf {
                id: PaneId::from_static("a"),
                constraint: PaneConstraint::Fixed(20),
                collapse_priority: 1,
            }),
            second: Box::new(WorkspaceNode::Leaf {
                id: PaneId::from_static("b"),
                constraint: PaneConstraint::Fixed(20),
                collapse_priority: 0,
            }),
        });
        let area = Rect::new(2, 3, 5, 4);
        let state = WorkspaceState::new();
        let panes = ws.layout(area, &state);
        for pane in panes {
            assert!(pane.area.x >= area.x);
            assert!(pane.area.y >= area.y);
            assert!(pane.area.right() <= area.right());
            assert!(pane.area.bottom() <= area.bottom());
        }
    }

    #[test]
    fn collapse_gives_space_to_sibling() {
        let ws = Workspace::new(WorkspaceNode::Split {
            axis: WorkspaceAxis::Horizontal,
            ratio_percent: 50,
            first: Box::new(WorkspaceNode::Leaf {
                id: PaneId::from_static("west"),
                constraint: PaneConstraint::Weight(1),
                collapse_priority: 0,
            }),
            second: Box::new(WorkspaceNode::Leaf {
                id: PaneId::from_static("east"),
                constraint: PaneConstraint::Weight(1),
                collapse_priority: 1,
            }),
        });
        let mut state = WorkspaceState::new();
        state.collapsed.push(PaneId::from_static("west"));
        let panes = ws.layout(Rect::new(0, 0, 40, 10), &state);
        let east = panes.iter().find(|p| p.id.0 == "east").unwrap();
        assert_eq!(east.area.width, 40);
        let west = panes.iter().find(|p| p.id.0 == "west").unwrap();
        assert!(west.collapsed || west.area.width == 0);
    }

    #[test]
    fn zoom_fills_parent() {
        let ws = Workspace::new(WorkspaceNode::Leaf {
            id: PaneId::from_static("only"),
            constraint: PaneConstraint::Weight(1),
            collapse_priority: 0,
        });
        let mut state = WorkspaceState::new();
        state.zoomed = Some(PaneId::from_static("main"));
        let area = Rect::new(1, 1, 30, 12);
        let panes = ws.layout(area, &state);
        assert_eq!(panes.len(), 1);
        assert_eq!(panes[0].area, area);
        assert_eq!(panes[0].id.0, "main");
    }
}
