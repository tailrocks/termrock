// SPDX-FileCopyrightText: 2026 Alexey Zhokhov
// SPDX-License-Identifier: Apache-2.0

//! Composition recipes (shadcn-style blocks) built from TermRock widgets.
//!
//! Patterns are product-neutral layouts: consumers supply wording, domain
//! data, and effects. TermRock owns geometry and chrome roles.
//!
//! **Canonical shell:** [`layout_app_shell`] / [`AppShellConfig`]. Specialized
//! helpers (`layout_agent_shell`, `layout_studio_shell`, …) are thin recipe
//! wrappers over AppShell slots.

mod agent_shell;
mod agent_workbench;
mod app_shell;
mod ops_dashboard;
mod resource_browser;
mod studio_shell;

pub use agent_shell::{AgentShellLayout, AgentShellSlots, layout_agent_shell};
pub use agent_workbench::{
    AgentWorkbenchState, WorkbenchKeyOutcome, WorkbenchModals, WorkbenchPane, WorkbenchSurfaces,
    agent_workbench_layout, default_modes, empty_task_row, permission_modal_rect,
    register_workbench_scene, render_agent_workbench, sync_workbench_scene,
};
pub use app_shell::{
    app_shell_viewport, layout_app_shell, AppShellConfig, AppShellLifecycle, AppShellRecipe,
    AppShellSlots, AppShellZone,
};
pub use ops_dashboard::{OpsDashboardLayout, OpsDashboardSlots, layout_ops_dashboard};
pub use resource_browser::{
    ResourceBrowserLayout, ResourceBrowserSlots, layout_resource_browser, wire_resource_preview,
};
pub use studio_shell::{StudioShellLayout, StudioShellSlots, layout_studio_shell};
