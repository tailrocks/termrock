// SPDX-FileCopyrightText: 2026 Alexey Zhokhov
// SPDX-License-Identifier: Apache-2.0

//! Resource browser recipe: tree/list rail + detail + optional preview.
//!
//! Preview pane wires through [`CapabilityPreviewHost`] for generation-safe
//! placement planning. Consumers emit protocol bytes outside render.
//! Built on AppShell Workbench (rail=sidebar, preview=inspector).

use ratatui_core::layout::Rect;

use crate::style::{CapabilityPreviewHost, Density};

use super::app_shell::{layout_app_shell, AppShellConfig, AppShellRecipe};

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
    pub density: Density,
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
            density: Density::Compact,
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
            density: config.density,
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

#[cfg(test)]
mod tests {
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
