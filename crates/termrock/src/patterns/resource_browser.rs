// SPDX-FileCopyrightText: 2026 Alexey Zhokhov
// SPDX-License-Identifier: Apache-2.0

//! Resource browser recipe: tree/list rail + detail + optional preview.
//!
//! Preview pane wires through [`CapabilityPreviewHost`] for generation-safe
//! placement planning. Consumers emit protocol bytes outside render.
//! Built on AppShell Workbench (rail=sidebar, preview=inspector).
//!
//! Teaches: how to compose a resource browser's geometry — a tree or list
//! rail, a detail pane, and an optional preview.
//!
//! [`crate::widgets::NavItem`], [`crate::widgets::SidebarOutcome`],
//! [`crate::widgets::SidebarState`].
//!
//! Copy-adapt: keep the widget composition and the focus routing;
//! replace the domain types, the wording, and the effects with your own.
use ratatui_core::layout::Rect;

use ratatui_core::{buffer::Buffer, widgets::StatefulWidget};

use crate::style::{CapabilityPreviewHost, DesignSystem, Role};
use crate::widgets::{
    DetailRow, DetailTable, DetailTableState, Panel, StatusBar, StatusBarState, StatusSlot, Tree,
    TreeNode, TreeState,
};

use super::app_shell::{AppShellConfig, AppShellRecipe, layout_app_shell};

/// Slots for a resource browser (file manager / k8s / DB class).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ResourceBrowserSlots {
    /// Navigation rail (tree or list).
    pub rail: Rect,
    /// Primary detail / table.
    pub detail: Rect,
    /// Optional preview pane (None when `preview_width == 0`).
    pub preview: Option<Rect>,
    /// Status / hints.
    pub status: Rect,
}

/// Layout knobs for [`layout_resource_browser`].
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ResourceBrowserLayout {
    /// Density.
    /// Left rail width.
    pub rail_width: u16,
    /// Right preview width; 0 hides preview.
    pub preview_width: u16,
    /// Status height.
    pub status_height: u16,
}

impl Default for ResourceBrowserLayout {
    fn default() -> Self {
        Self {
            rail_width: 28,
            preview_width: 32,
            status_height: 1,
        }
    }
}

/// Resolves resource browser rectangles.
#[must_use]
pub fn layout_resource_browser(area: Rect, config: ResourceBrowserLayout) -> ResourceBrowserSlots {
    let shell = layout_app_shell(
        area,
        AppShellConfig {
            recipe: AppShellRecipe::Workbench,
            header_height: 0,
            sidebar_width: config.rail_width.max(1),
            inspector_width: config.preview_width,
            footer_height: config.status_height.max(1),
            command_height: 0,
            metrics_height: 0,
            log_height: 0,
            lifecycle: Default::default(),
            inline: false,
        },
    );

    let status = shell.footer.unwrap_or(Rect {
        x: area.x,
        y: area.y.saturating_add(area.height.saturating_sub(1)),
        width: area.width,
        height: 1.min(area.height),
    });

    // When responsive collapses sidebar, fall back to full-width detail.
    let rail = shell.sidebar.unwrap_or(Rect {
        x: shell.main.x,
        y: shell.main.y,
        width: 0,
        height: shell.main.height,
    });
    let detail = shell.main;
    let preview = shell.inspector;

    ResourceBrowserSlots {
        rail,
        detail,
        preview,
        status,
    }
}

/// Syncs the resource-browser preview slot into a capability preview host.
///
/// Call after layout each frame when a resource is selected. Bumps generation
/// only when `resource_id` changes (caller tracks previous id).
pub fn wire_resource_preview(
    host: &mut CapabilityPreviewHost,
    slots: &ResourceBrowserSlots,
    resource_id: Option<&str>,
    pending: bool,
    selection_changed: bool,
) {
    host.begin_frame();
    if selection_changed {
        host.bump_generation();
    }
    if let (Some(area), Some(id)) = (slots.preview, resource_id) {
        host.place_resource_preview(area, id, pending);
    }
}

// ── Reference paint ─────────────────────────────────────────────────────────

/// Host-owned content for one resource browser frame.
#[derive(Debug, Clone, Copy)]
pub struct ResourceBrowserView<'a, NodeId, RowId> {
    /// Navigation nodes for the rail.
    pub nodes: &'a [TreeNode<'a, NodeId>],
    /// Detail rows for the selected resource.
    pub details: &'a [DetailRow<'a, RowId>],
    /// Preview body, when a preview pane is configured.
    pub preview: Option<&'a str>,
    /// Footer hints.
    pub hints: &'a [StatusSlot<'a, &'a str>],
}

/// Which pane of the browser owns interaction this frame.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
#[non_exhaustive]
pub enum ResourceBrowserFocus {
    /// The navigation rail.
    #[default]
    Rail,
}

/// Paints a reference resource browser over [`layout_resource_browser`].
///
/// The rail is a [`Tree`], the detail pane a [`DetailTable`], the preview a
/// [`Panel`] body, and the footer a [`StatusBar`] — no chrome is invented
/// here. Hosts wanting a different assembly copy this and swap the widgets.
pub fn paint_resource_browser<NodeId: Clone + Eq, RowId: Clone + Eq>(
    area: Rect,
    buffer: &mut Buffer,
    system: &DesignSystem,
    config: ResourceBrowserLayout,
    view: ResourceBrowserView<'_, NodeId, RowId>,
    focus: ResourceBrowserFocus,
    tree_state: &mut TreeState<NodeId>,
    detail_state: &mut DetailTableState<RowId>,
) -> ResourceBrowserSlots {
    let slots = layout_resource_browser(area, config);

    if slots.rail.height > 0 {
        Tree::new(view.nodes, system)
            .focused(matches!(focus, ResourceBrowserFocus::Rail))
            .render(slots.rail, buffer, tree_state);
    }

    if slots.detail.height > 0 {
        DetailTable::new(view.details, system).render(slots.detail, buffer, detail_state);
    }

    if let Some(preview) = slots.preview
        && preview.height > 0
    {
        let body = Panel::new(system)
            .title("Preview")
            .paint(preview, buffer, None);
        if let Some(text) = view.preview {
            for (i, line) in text.lines().take(usize::from(body.height)).enumerate() {
                system.paint_row(
                    buffer,
                    Rect::new(
                        body.x,
                        body.y.saturating_add(u16::try_from(i).unwrap_or(0)),
                        body.width,
                        1,
                    ),
                    line,
                    system.style(Role::TextMuted),
                );
            }
        }
    }

    if slots.status.height > 0 {
        let mut status = StatusBarState::new();
        StatusBar::new(view.hints, &[], system).render(slots.status, buffer, &mut status);
    }

    slots
}

#[cfg(test)]
mod tests {

    #[test]
    fn reference_paint_fills_every_slot() {
        use crate::style::DesignSystem;
        use crate::widgets::{
            DetailCapability, DetailRow, DetailTableState, StatusSlot, TreeNode, TreeState,
        };
        use ratatui_core::buffer::Buffer;
        use ratatui_core::text::Line;

        let system = DesignSystem::default();
        let nodes = [
            TreeNode::new("ns", Line::from("namespaces"), 0),
            TreeNode::new("pods", Line::from("pods"), 1),
        ];
        let details = [DetailRow {
            id: "name",
            label: "Name",
            value: "api-7",
            href: None,
            capability: DetailCapability::Copy,
            emphasis: false,
            style: None,
        }];
        let hints = [StatusSlot::new("tab", "tab pane")];
        let view = ResourceBrowserView {
            nodes: &nodes,
            details: &details,
            preview: Some("apiVersion: v1"),
            hints: &hints,
        };
        let mut tree_state = TreeState::new(Some("ns"));
        let mut detail_state = DetailTableState::default();
        let area = Rect::new(0, 0, 90, 24);
        let mut buffer = Buffer::empty(area);
        let config = ResourceBrowserLayout {
            preview_width: 24,
            ..ResourceBrowserLayout::default()
        };
        let slots = paint_resource_browser(
            area,
            &mut buffer,
            &system,
            config,
            view,
            ResourceBrowserFocus::Rail,
            &mut tree_state,
            &mut detail_state,
        );

        let painted = |rect: Rect| {
            (rect.x..rect.right()).any(|x| {
                (rect.y..rect.bottom()).any(|y| !buffer[(x, y)].symbol().trim().is_empty())
            })
        };
        assert!(painted(slots.rail), "rail painted nothing");
        assert!(painted(slots.detail), "detail painted nothing");
        if let Some(preview) = slots.preview {
            assert!(painted(preview), "preview painted nothing");
        }
        assert!(painted(slots.status), "status painted nothing");
    }
    use super::*;
    use crate::style::DesignSystem;

    #[test]
    fn resource_browser_optional_preview() {
        let with_preview =
            layout_resource_browser(Rect::new(0, 0, 120, 40), ResourceBrowserLayout::default());
        assert!(with_preview.preview.is_some());
        let no_preview = layout_resource_browser(
            Rect::new(0, 0, 120, 40),
            ResourceBrowserLayout {
                preview_width: 0,
                ..ResourceBrowserLayout::default()
            },
        );
        assert!(no_preview.preview.is_none());
    }

    #[test]
    fn wire_preview_registers_resource_surface() {
        let slots =
            layout_resource_browser(Rect::new(0, 0, 120, 40), ResourceBrowserLayout::default());
        let mut host = CapabilityPreviewHost::truecolor(DesignSystem::default());
        wire_resource_preview(&mut host, &slots, Some("readme.md"), false, true);
        assert_eq!(host.surfaces.len(), 1);
        assert_eq!(host.surfaces[0].resource_id.as_deref(), Some("readme.md"));
        // Stale async after reselection.
        let stale_gen = host.generation().saturating_sub(1);
        wire_resource_preview(&mut host, &slots, Some("other.md"), true, true);
        assert!(!host.complete_async(stale_gen, "readme.md"));
    }

    /// Every-frame wire with steady selection must not thrash session commands.
    #[test]
    fn wire_steady_state_same_resource_emits_no_session_commands() {
        let slots =
            layout_resource_browser(Rect::new(0, 0, 120, 40), ResourceBrowserLayout::default());
        let mut host =
            CapabilityPreviewHost::truecolor(DesignSystem::default()).protocols(true, false, false);
        // Frame 1: selection changed (open preview).
        wire_resource_preview(&mut host, &slots, Some("readme.md"), false, true);
        let cmds1 = host.session_commands();
        assert!(
            matches!(
                cmds1.as_slice(),
                [crate::style::MediaSessionCommand::Replace {
                    resource_id,
                    ..
                }] if resource_id == "readme.md"
            ),
            "frame1 wire should place: {cmds1:?}"
        );
        let placement_id = match &cmds1[0] {
            crate::style::MediaSessionCommand::Replace { placement_id, .. } => *placement_id,
            other => panic!("expected Replace, got {other:?}"),
        };

        // Frame 2: same selection, selection_changed=false — empty commands.
        wire_resource_preview(&mut host, &slots, Some("readme.md"), false, false);
        assert_eq!(host.surfaces[0].placement_id, placement_id);
        let cmds2 = host.session_commands();
        assert!(
            cmds2.is_empty(),
            "frame2 steady wire must not thrash: {cmds2:?}"
        );
    }
}

// ── Resource browser state machine (example composite) ───────────────────────

use crate::{
    input::KeyEvent,
    widgets::{NavItem, SidebarOutcome, SidebarState},
};

// ── ResourceBrowser ─────────────────────────────────────────────────────────

/// Resource browser outcomes.
#[derive(Debug, Clone, PartialEq, Eq)]
#[non_exhaustive]
pub enum ResourceBrowserOutcome<Id> {
    /// Sidebar selection.
    Sidebar(SidebarOutcome<Id>),
    /// Request load of selection (consumer).
    LoadRequested(Id),
}

/// Resource browser state.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ResourceBrowserState<Id: Clone + PartialEq> {
    /// Sidebar.
    pub sidebar: SidebarState<Id>,
    /// Generation for stale preview guard.
    pub selection_generation: u64,
}

impl<Id: Clone + PartialEq> ResourceBrowserState<Id> {
    /// Fresh.
    #[must_use]
    pub fn new() -> Self {
        Self {
            sidebar: SidebarState::new(None),
            selection_generation: 0,
        }
    }

    /// Keys.
    pub fn handle_key(
        &mut self,
        key: KeyEvent,
        items: &[NavItem<Id>],
    ) -> ResourceBrowserOutcome<Id> {
        let out = self.sidebar.handle_key(key, items);
        match out {
            SidebarOutcome::RouteChanged { id } => {
                self.selection_generation = self.selection_generation.saturating_add(1);
                ResourceBrowserOutcome::LoadRequested(id)
            }
            other => ResourceBrowserOutcome::Sidebar(other),
        }
    }
}

impl<Id: Clone + PartialEq> Default for ResourceBrowserState<Id> {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod state_tests {
    use super::*;
    use crate::input::{KeyCode, KeyEvent, KeyModifiers};
    use crate::widgets::NavItem;

    #[test]
    fn resource_load_on_select() {
        let mut state = ResourceBrowserState::new();
        let items = [NavItem::new("a", "A")];
        let out = state.handle_key(KeyEvent::new(KeyCode::Enter, KeyModifiers::NONE), &items);
        assert!(matches!(out, ResourceBrowserOutcome::LoadRequested("a")));
        assert_eq!(state.selection_generation, 1);
    }
}
