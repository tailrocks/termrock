// SPDX-FileCopyrightText: 2026 Alexey Zhokhov
// SPDX-License-Identifier: Apache-2.0

//! Flagship Agent Workbench layout: TaskRail | Transcript + Prompt.

use ratatui_core::layout::Rect;

use crate::layout::{
    PaneConstraint, PaneGeom, PaneId, Workspace, WorkspaceAxis, WorkspaceNode, WorkspaceState,
};

/// Named panes of the default agent workbench.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[non_exhaustive]
pub enum WorkbenchPane {
    /// Task / subagent rail.
    TaskRail,
    /// Center transcript.
    Transcript,
    /// South prompt composer.
    Prompt,
    /// Status strip.
    Status,
}

impl WorkbenchPane {
    /// Stable pane id string.
    #[must_use]
    pub const fn id(self) -> &'static str {
        match self {
            Self::TaskRail => "task_rail",
            Self::Transcript => "transcript",
            Self::Prompt => "prompt",
            Self::Status => "status",
        }
    }
}

/// Resolves workbench geometry for the current area and collapse state.
#[must_use]
pub fn agent_workbench_layout(area: Rect, state: &WorkspaceState) -> Vec<PaneGeom> {
    let root = WorkspaceNode::Split {
        axis: WorkspaceAxis::Vertical,
        ratio_percent: 92,
        first: Box::new(WorkspaceNode::Split {
            axis: WorkspaceAxis::Horizontal,
            ratio_percent: 22,
            first: Box::new(WorkspaceNode::Leaf {
                id: PaneId::from_static(WorkbenchPane::TaskRail.id()),
                constraint: PaneConstraint::Min(12),
                collapse_priority: 0,
            }),
            second: Box::new(WorkspaceNode::Leaf {
                id: PaneId::from_static(WorkbenchPane::Transcript.id()),
                constraint: PaneConstraint::Weight(1),
                collapse_priority: 2,
            }),
        }),
        second: Box::new(WorkspaceNode::Split {
            axis: WorkspaceAxis::Vertical,
            ratio_percent: 70,
            first: Box::new(WorkspaceNode::Leaf {
                id: PaneId::from_static(WorkbenchPane::Prompt.id()),
                constraint: PaneConstraint::Min(3),
                collapse_priority: 1,
            }),
            second: Box::new(WorkspaceNode::Leaf {
                id: PaneId::from_static(WorkbenchPane::Status.id()),
                constraint: PaneConstraint::Fixed(1),
                collapse_priority: 3,
            }),
        }),
    };
    Workspace::new(root).layout(area, state)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn workbench_rects_are_contained() {
        let state = WorkspaceState::new();
        let area = Rect::new(0, 0, 80, 24);
        let panes = agent_workbench_layout(area, &state);
        assert!(!panes.is_empty());
        for pane in panes {
            assert!(pane.area.right() <= area.right());
            assert!(pane.area.bottom() <= area.bottom());
        }
    }
}
