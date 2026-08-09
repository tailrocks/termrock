// SPDX-FileCopyrightText: 2026 Alexey Zhokhov
// SPDX-License-Identifier: Apache-2.0

//! Composition recipes (shadcn-style blocks) built from TermRock widgets.
//!
//! Patterns are product-neutral layouts: consumers supply wording, domain
//! data, and effects. TermRock owns geometry and chrome roles.

mod agent_shell;
mod agent_workbench;
mod ops_dashboard;
mod resource_browser;

pub use agent_shell::{AgentShellLayout, AgentShellSlots, layout_agent_shell};
pub use agent_workbench::{WorkbenchPane, agent_workbench_layout};
pub use ops_dashboard::{OpsDashboardLayout, OpsDashboardSlots, layout_ops_dashboard};
pub use resource_browser::{ResourceBrowserLayout, ResourceBrowserSlots, layout_resource_browser};
