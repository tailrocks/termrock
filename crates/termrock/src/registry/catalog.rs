// SPDX-FileCopyrightText: 2026 Alexey Zhokhov
// SPDX-License-Identifier: Apache-2.0

//! Official kernel inventory contracts (embedded catalog).

use super::contract::{
    AnatomyPartRef, CapabilityRequirements, ComponentContract, ContractDependencies, ContractFile,
    ContractFileRole, KernelRequirement, OutcomeRef, Provenance, RegistryItemKind, SemanticRoleRef,
    VariantRef, CONTRACT_SCHEMA,
};

fn prov(path: &str) -> Provenance {
    Provenance {
        origin: "https://github.com/tailrocks/termrock".into(),
        path: path.into(),
        revision: "main".into(),
        spdx: "Apache-2.0".into(),
        authors: vec!["Tailrocks contributors".into()],
    }
}

fn file(source: &str, role: ContractFileRole) -> ContractFile {
    ContractFile {
        source: source.into(),
        target: None,
        role,
        hash: None,
        optional: false,
    }
}

fn kernel_dep() -> ContractDependencies {
    ContractDependencies {
        kernel: Some(KernelRequirement {
            crate_name: "termrock".into(),
            min_version: "0.11.0".into(),
            features: vec![],
        }),
        registry: vec![],
        cargo: vec!["ratatui-core".into()],
    }
}

fn caps_basic() -> CapabilityRequirements {
    CapabilityRequirements {
        color: vec![
            "truecolor".into(),
            "256".into(),
            "16".into(),
            "mono".into(),
        ],
        glyphs: vec!["unicode".into(), "ascii".into()],
        responsive_surface: None,
        min_width: Some(8),
        min_height: Some(1),
        mouse_enhanced: false,
    }
}

/// Official kernel-hosted contracts (subset of public inventory).
#[must_use]
pub fn official_kernel_contracts() -> Vec<ComponentContract> {
    vec![
        ComponentContract {
            schema: CONTRACT_SCHEMA,
            id: "Panel".into(),
            title: "Panel".into(),
            description: "Composable container: variants, body modes, collapsible/interactive; focus ≠ selection; Role::BorderFocused for focus.".into(),
            kind: RegistryItemKind::Primitive,
            license: "Apache-2.0".into(),
            module: Some("termrock::widgets::Panel".into()),
            namespace: "termrock".into(),
            version: "0.13.0".into(),
            files: vec![
                file("crates/termrock/src/widgets/panel.rs", ContractFileRole::Primary),
                file(
                    "docs/public/component-previews/panel-focused.svg",
                    ContractFileRole::Fixture,
                ),
            ],
            dependencies: {
                let mut d = kernel_dep();
                d.registry = vec!["termrock/Surface".into()];
                d
            },
            capabilities: caps_basic(),
            anatomy: vec![
                AnatomyPartRef {
                    id: "header".into(),
                    label: "Header".into(),
                },
                AnatomyPartRef {
                    id: "body".into(),
                    label: "Body".into(),
                },
                AnatomyPartRef {
                    id: "footer".into(),
                    label: "Footer".into(),
                },
            ],
            semantic_roles: vec![
                SemanticRoleRef {
                    id: "Role::Border".into(),
                },
                SemanticRoleRef {
                    id: "Role::BorderFocused".into(),
                },
            ],
            variants: vec![
                VariantRef {
                    id: "bordered".into(),
                    description: "Default box border".into(),
                },
                VariantRef {
                    id: "quiet".into(),
                    description: "No border".into(),
                },
                VariantRef {
                    id: "selected".into(),
                    description: "Selection chrome (not focus)".into(),
                },
            ],
            outcomes: vec![
                OutcomeRef {
                    id: "Activated".into(),
                },
                OutcomeRef {
                    id: "ToggleCollapsed".into(),
                },
            ],
            stories: vec![
                "panel/focused".into(),
                "panel/variants".into(),
                "panel/empty".into(),
                "panel/collapsible".into(),
                "panel/narrow".into(),
            ],
            tests: vec!["widgets::panel".into()],
            migration: Some("migrations/0096-v0.13.0-panel-card.md".into()),
            provenance: prov("crates/termrock/src/widgets/panel.rs"),
            source_hash: None,
            complete: false,
        },
        ComponentContract {
            schema: CONTRACT_SCHEMA,
            id: "ScrollArea".into(),
            title: "ScrollArea".into(),
            description: "Canonical scrolling primitive: axes, follow, anchors, chain."
                .into(),
            kind: RegistryItemKind::Primitive,
            license: "Apache-2.0".into(),
            module: Some("termrock::widgets::ScrollArea".into()),
            namespace: "termrock".into(),
            version: "0.13.0".into(),
            files: vec![file(
                "crates/termrock/src/widgets/scroll_area.rs",
                ContractFileRole::Primary,
            )],
            dependencies: kernel_dep(),
            capabilities: {
                let mut c = caps_basic();
                c.responsive_surface = Some("LogViewer".into());
                c
            },
            anatomy: vec![
                AnatomyPartRef {
                    id: "body".into(),
                    label: "Viewport body".into(),
                },
                AnatomyPartRef {
                    id: "scrollbar".into(),
                    label: "Scrollbar".into(),
                },
                AnatomyPartRef {
                    id: "new_content".into(),
                    label: "New content strip".into(),
                },
            ],
            semantic_roles: vec![
                SemanticRoleRef {
                    id: "Role::ScrollTrack".into(),
                },
                SemanticRoleRef {
                    id: "Role::ScrollThumb".into(),
                },
            ],
            variants: vec![],
            outcomes: vec![OutcomeRef {
                id: "ScrollOutcome".into(),
            }],
            stories: vec!["scroll-area/follow-paused".into()],
            tests: vec!["widgets::scroll_area".into()],
            migration: Some("migrations/0085-v0.13.0-scroll-area.md".into()),
            provenance: prov("crates/termrock/src/widgets/scroll_area.rs"),
            source_hash: None,
            complete: true,
        },
        ComponentContract {
            schema: CONTRACT_SCHEMA,
            id: "DismissableLayer".into(),
            title: "DismissableLayer".into(),
            description: "Reusable dismiss policy: Esc, outside press/release, traps."
                .into(),
            kind: RegistryItemKind::Behavior,
            license: "Apache-2.0".into(),
            module: Some("termrock::interaction::DismissableLayer".into()),
            namespace: "termrock".into(),
            version: "0.13.0".into(),
            files: vec![file(
                "crates/termrock/src/interaction/dismissable.rs",
                ContractFileRole::Primary,
            )],
            dependencies: kernel_dep(),
            capabilities: caps_basic(),
            anatomy: vec![],
            semantic_roles: vec![],
            variants: vec![
                VariantRef {
                    id: "dismissible".into(),
                    description: "DismissPolicy::dismissible".into(),
                },
                VariantRef {
                    id: "critical".into(),
                    description: "DismissPolicy::critical trap".into(),
                },
            ],
            outcomes: vec![OutcomeRef {
                id: "DismissDecision".into(),
            }],
            stories: vec!["dismissable/gestures".into()],
            tests: vec!["interaction::dismissable".into()],
            migration: Some("migrations/0089-v0.13.0-dismissable-layer.md".into()),
            provenance: prov("crates/termrock/src/interaction/dismissable.rs"),
            source_hash: None,
            complete: true,
        },
        ComponentContract {
            schema: CONTRACT_SCHEMA,
            id: "OverlayStack".into(),
            title: "OverlayStack".into(),
            description: "Sole overlay authority: z-order, placement, queue, Esc one layer."
                .into(),
            kind: RegistryItemKind::Behavior,
            license: "Apache-2.0".into(),
            module: Some("termrock::interaction::OverlayStack".into()),
            namespace: "termrock".into(),
            version: "0.13.0".into(),
            files: vec![file(
                "crates/termrock/src/interaction/overlay_stack.rs",
                ContractFileRole::Primary,
            )],
            dependencies: {
                let mut d = kernel_dep();
                d.registry = vec!["termrock/DismissableLayer".into()];
                d
            },
            capabilities: caps_basic(),
            anatomy: vec![],
            semantic_roles: vec![],
            variants: vec![],
            outcomes: vec![OutcomeRef {
                id: "OverlayOutcome".into(),
            }],
            stories: vec![
                "overlay/nested-escape".into(),
                "overlay/queued-dialogs".into(),
            ],
            tests: vec!["overlay_stack".into()],
            migration: Some("migrations/0087-v0.13.0-overlay-stack-premium.md".into()),
            provenance: prov("crates/termrock/src/interaction/overlay_stack.rs"),
            source_hash: None,
            complete: true,
        },
        ComponentContract {
            schema: CONTRACT_SCHEMA,
            id: "TerminalCapabilities".into(),
            title: "TerminalCapabilities".into(),
            description: "Detect/profile/override capabilities; CapabilityBoundary for widgets."
                .into(),
            kind: RegistryItemKind::Behavior,
            license: "Apache-2.0".into(),
            module: Some("termrock::capability::TerminalCapabilities".into()),
            namespace: "termrock".into(),
            version: "0.13.0".into(),
            files: vec![
                file(
                    "crates/termrock/src/capability/profile.rs",
                    ContractFileRole::Primary,
                ),
                file(
                    "crates/termrock/src/capability/boundary.rs",
                    ContractFileRole::Support,
                ),
            ],
            dependencies: kernel_dep(),
            capabilities: {
                let mut c = caps_basic();
                c.mouse_enhanced = true;
                c
            },
            anatomy: vec![],
            semantic_roles: vec![],
            variants: vec![
                VariantRef {
                    id: "modern".into(),
                    description: "CapabilityProfile::Modern".into(),
                },
                VariantRef {
                    id: "minimal".into(),
                    description: "CapabilityProfile::Minimal".into(),
                },
            ],
            outcomes: vec![],
            stories: vec!["capability/profiles".into()],
            tests: vec!["capability".into(), "capability_pty".into()],
            migration: Some("migrations/0091-v0.13.0-terminal-capabilities.md".into()),
            provenance: prov("crates/termrock/src/capability"),
            source_hash: None,
            complete: true,
        },
        ComponentContract {
            schema: CONTRACT_SCHEMA,
            id: "phosphor-theme".into(),
            title: "Phosphor theme".into(),
            description: "Default phosphor design language (RolePalette).".into(),
            kind: RegistryItemKind::Theme,
            license: "Apache-2.0".into(),
            module: Some("termrock::style::RolePalette".into()),
            namespace: "termrock".into(),
            version: "0.13.0".into(),
            files: vec![file(
                "crates/termrock/src/style/tokens.rs",
                ContractFileRole::Primary,
            )],
            dependencies: kernel_dep(),
            capabilities: caps_basic(),
            anatomy: vec![],
            semantic_roles: vec![SemanticRoleRef {
                id: "Role::*".into(),
            }],
            variants: vec![VariantRef {
                id: "tailrocks_phosphor".into(),
                description: "Default brand palette".into(),
            }],
            outcomes: vec![],
            stories: vec!["capability/color-ladder".into()],
            tests: vec!["style".into()],
            migration: None,
            provenance: prov("crates/termrock/src/style"),
            source_hash: None,
            complete: false,
        },
        ComponentContract {
            schema: CONTRACT_SCHEMA,
            id: "Grid".into(),
            title: "Grid".into(),
            description: "2D track grid: fixed/fr/minmax, gaps, spans, auto-flow, responsive templates, spatial neighbors.".into(),
            kind: RegistryItemKind::Primitive,
            license: "Apache-2.0".into(),
            module: Some("termrock::layout::Grid".into()),
            namespace: "termrock".into(),
            version: "0.13.0".into(),
            files: vec![file(
                "crates/termrock/src/layout/grid.rs",
                ContractFileRole::Primary,
            )],
            dependencies: {
                let mut d = kernel_dep();
                d.registry = vec!["termrock/Stack".into()];
                d
            },
            capabilities: caps_basic(),
            anatomy: vec![
                AnatomyPartRef {
                    id: "columns".into(),
                    label: "Column tracks".into(),
                },
                AnatomyPartRef {
                    id: "rows".into(),
                    label: "Row tracks".into(),
                },
                AnatomyPartRef {
                    id: "cells".into(),
                    label: "Placed cells".into(),
                },
            ],
            semantic_roles: vec![],
            variants: vec![
                VariantRef {
                    id: "columns-fr".into(),
                    description: "Equal fractional columns".into(),
                },
                VariantRef {
                    id: "form".into(),
                    description: "form_grid_template".into(),
                },
                VariantRef {
                    id: "dashboard".into(),
                    description: "dashboard_grid_template".into(),
                },
            ],
            outcomes: vec![],
            stories: vec![
                "grid/columns".into(),
                "grid/span".into(),
                "grid/dashboard".into(),
                "grid/form".into(),
                "grid/narrow".into(),
            ],
            tests: vec!["layout::grid".into()],
            migration: Some("migrations/0098-v0.13.0-grid.md".into()),
            provenance: prov("crates/termrock/src/layout/grid.rs"),
            source_hash: None,
            complete: false,
        },
        ComponentContract {
            schema: CONTRACT_SCHEMA,
            id: "Stack".into(),
            title: "Stack / Inline".into(),
            description: "Stateless vertical/horizontal packing: FlexSize, gap, align, justify, wrap, overflow.".into(),
            kind: RegistryItemKind::Primitive,
            license: "Apache-2.0".into(),
            module: Some("termrock::layout::Stack".into()),
            namespace: "termrock".into(),
            version: "0.13.0".into(),
            files: vec![file(
                "crates/termrock/src/layout/stack.rs",
                ContractFileRole::Primary,
            )],
            dependencies: kernel_dep(),
            capabilities: {
                let mut c = caps_basic();
                c.min_width = Some(1);
                c.min_height = Some(1);
                c
            },
            anatomy: vec![
                AnatomyPartRef {
                    id: "content".into(),
                    label: "Padded content".into(),
                },
                AnatomyPartRef {
                    id: "children".into(),
                    label: "Child rects".into(),
                },
            ],
            semantic_roles: vec![],
            variants: vec![
                VariantRef {
                    id: "vertical".into(),
                    description: "Stack".into(),
                },
                VariantRef {
                    id: "horizontal".into(),
                    description: "Inline".into(),
                },
                VariantRef {
                    id: "wrap".into(),
                    description: "Inline wrap".into(),
                },
            ],
            outcomes: vec![],
            stories: vec![
                "stack/vertical".into(),
                "stack/inline".into(),
                "stack/wrap".into(),
                "stack/responsive".into(),
            ],
            tests: vec!["layout::stack".into()],
            migration: Some("migrations/0097-v0.13.0-stack-inline.md".into()),
            provenance: prov("crates/termrock/src/layout/stack.rs"),
            source_hash: None,
            complete: false,
        },
        ComponentContract {
            schema: CONTRACT_SCHEMA,
            id: "Card".into(),
            title: "Card".into(),
            description: "Raised Panel composition with description band for metrics/tool cards.".into(),
            kind: RegistryItemKind::Primitive,
            license: "Apache-2.0".into(),
            module: Some("termrock::widgets::Card".into()),
            namespace: "termrock".into(),
            version: "0.13.0".into(),
            files: vec![
                file(
                    "crates/termrock/src/widgets/card.rs",
                    ContractFileRole::Primary,
                ),
                file(
                    "docs/public/component-previews/card-basic.svg",
                    ContractFileRole::Fixture,
                ),
            ],
            dependencies: {
                let mut d = kernel_dep();
                d.registry = vec!["termrock/Panel".into(), "termrock/Surface".into()];
                d
            },
            capabilities: caps_basic(),
            anatomy: vec![
                AnatomyPartRef {
                    id: "header".into(),
                    label: "Header".into(),
                },
                AnatomyPartRef {
                    id: "description".into(),
                    label: "Description".into(),
                },
                AnatomyPartRef {
                    id: "body".into(),
                    label: "Body".into(),
                },
            ],
            semantic_roles: vec![],
            variants: vec![],
            outcomes: vec![],
            stories: vec!["card/basic".into(), "card/tool".into()],
            tests: vec!["widgets::card".into()],
            migration: Some("migrations/0096-v0.13.0-panel-card.md".into()),
            provenance: prov("crates/termrock/src/widgets/card.rs"),
            source_hash: None,
            complete: false,
        },
        ComponentContract {
            schema: CONTRACT_SCHEMA,
            id: "Surface".into(),
            title: "Surface".into(),
            description: "Lowest-level visual ownership: fill, padding, border, clip, and hit geometry with canvas→destructive recipes.".into(),
            kind: RegistryItemKind::Primitive,
            license: "Apache-2.0".into(),
            module: Some("termrock::widgets::Surface".into()),
            namespace: "termrock".into(),
            version: "0.13.0".into(),
            files: vec![
                file(
                    "crates/termrock/src/widgets/surface.rs",
                    ContractFileRole::Primary,
                ),
                file(
                    "docs/public/component-previews/surface-ladder.svg",
                    ContractFileRole::Fixture,
                ),
            ],
            dependencies: kernel_dep(),
            capabilities: {
                let mut c = caps_basic();
                c.min_width = Some(1);
                c.min_height = Some(1);
                c
            },
            anatomy: vec![
                AnatomyPartRef {
                    id: "root".into(),
                    label: "Root".into(),
                },
                AnatomyPartRef {
                    id: "content".into(),
                    label: "Content slot".into(),
                },
                AnatomyPartRef {
                    id: "hit".into(),
                    label: "Hit region".into(),
                },
                AnatomyPartRef {
                    id: "clip".into(),
                    label: "Clip contract".into(),
                },
            ],
            semantic_roles: vec![
                SemanticRoleRef {
                    id: "Role::Canvas".into(),
                },
                SemanticRoleRef {
                    id: "Role::Surface".into(),
                },
                SemanticRoleRef {
                    id: "Role::Elevated".into(),
                },
                SemanticRoleRef {
                    id: "Role::BorderFocused".into(),
                },
            ],
            variants: vec![
                VariantRef {
                    id: "canvas".into(),
                    description: "Terminal underlay".into(),
                },
                VariantRef {
                    id: "inset".into(),
                    description: "Default surface".into(),
                },
                VariantRef {
                    id: "raised".into(),
                    description: "Card / elevated".into(),
                },
                VariantRef {
                    id: "focused".into(),
                    description: "Interaction owner border".into(),
                },
                VariantRef {
                    id: "destructive".into(),
                    description: "Danger chrome".into(),
                },
            ],
            outcomes: vec![],
            stories: vec![
                "surface/basic".into(),
                "surface/ladder".into(),
                "surface/focused".into(),
                "surface/terminal-default".into(),
            ],
            tests: vec!["widgets::surface".into()],
            migration: Some("migrations/0095-v0.13.0-surface.md".into()),
            provenance: prov("crates/termrock/src/widgets/surface.rs"),
            source_hash: None,
            complete: false,
        },
        ComponentContract {
            schema: CONTRACT_SCHEMA,
            id: "AppShell".into(),
            title: "AppShell".into(),
            description: "Canonical top-level composition: header, sidebar, main, inspector, footer, command, overlays; recipes + responsive collapse.".into(),
            kind: RegistryItemKind::Block,
            license: "Apache-2.0".into(),
            module: Some("termrock::patterns::layout_app_shell".into()),
            namespace: "termrock".into(),
            version: "0.13.0".into(),
            files: vec![
                file(
                    "crates/termrock/src/patterns/app_shell.rs",
                    ContractFileRole::Primary,
                ),
                file(
                    "docs/public/component-previews/app-shell-workbench.svg",
                    ContractFileRole::Fixture,
                ),
            ],
            dependencies: {
                let mut d = kernel_dep();
                d.registry = vec![
                    "termrock/WorkSurface".into(),
                    "termrock/ResponsiveSurface".into(),
                ];
                d
            },
            capabilities: {
                let mut c = caps_basic();
                c.responsive_surface = Some("AppShell".into());
                c.min_width = Some(20);
                c.min_height = Some(5);
                c
            },
            anatomy: vec![
                AnatomyPartRef {
                    id: "header".into(),
                    label: "Header".into(),
                },
                AnatomyPartRef {
                    id: "sidebar".into(),
                    label: "Sidebar".into(),
                },
                AnatomyPartRef {
                    id: "main".into(),
                    label: "Main workspace".into(),
                },
                AnatomyPartRef {
                    id: "inspector".into(),
                    label: "Inspector rail".into(),
                },
                AnatomyPartRef {
                    id: "footer".into(),
                    label: "Footer / status".into(),
                },
                AnatomyPartRef {
                    id: "command".into(),
                    label: "Command surface".into(),
                },
                AnatomyPartRef {
                    id: "overlay_bounds".into(),
                    label: "Overlay host bounds".into(),
                },
            ],
            semantic_roles: vec![],
            variants: vec![
                VariantRef {
                    id: "workbench".into(),
                    description: "IDE / agent multi-pane".into(),
                },
                VariantRef {
                    id: "dashboard".into(),
                    description: "Metrics + main + log".into(),
                },
                VariantRef {
                    id: "master-detail".into(),
                    description: "List + detail".into(),
                },
                VariantRef {
                    id: "minimal".into(),
                    description: "Main + footer only".into(),
                },
            ],
            outcomes: vec![],
            stories: vec![
                "app-shell/workbench".into(),
                "app-shell/dashboard".into(),
                "app-shell/master-detail".into(),
                "app-shell/minimal".into(),
                "app-shell/narrow-drawer".into(),
                "app-shell/offline".into(),
            ],
            tests: vec!["patterns::app_shell".into()],
            migration: Some("migrations/0094-v0.13.0-app-shell.md".into()),
            provenance: prov("crates/termrock/src/patterns/app_shell.rs"),
            source_hash: None,
            complete: false,
        },
        ComponentContract {
            schema: CONTRACT_SCHEMA,
            id: "agent-workbench".into(),
            title: "Agent Workbench".into(),
            description: "Flagship multi-pane agent shell block (composition).".into(),
            kind: RegistryItemKind::Block,
            license: "Apache-2.0".into(),
            module: Some("termrock::patterns::agent_workbench".into()),
            namespace: "termrock".into(),
            version: "0.13.0".into(),
            files: vec![file(
                "crates/termrock/src/patterns/agent_workbench.rs",
                ContractFileRole::Primary,
            )],
            dependencies: {
                let mut d = kernel_dep();
                d.registry = vec![
                    "termrock/AppShell".into(),
                    "termrock/ScrollArea".into(),
                    "termrock/OverlayStack".into(),
                    "termrock/Panel".into(),
                ];
                d
            },
            capabilities: {
                let mut c = caps_basic();
                c.responsive_surface = Some("AppShell".into());
                c.min_width = Some(40);
                c.min_height = Some(12);
                c
            },
            anatomy: vec![
                AnatomyPartRef {
                    id: "transcript".into(),
                    label: "Transcript".into(),
                },
                AnatomyPartRef {
                    id: "composer".into(),
                    label: "Prompt composer".into(),
                },
                AnatomyPartRef {
                    id: "rail".into(),
                    label: "Task rail".into(),
                },
            ],
            semantic_roles: vec![],
            variants: vec![],
            outcomes: vec![],
            stories: vec!["agent-workbench/basic".into()],
            tests: vec![],
            migration: None,
            provenance: prov("crates/termrock/src/patterns/agent_workbench.rs"),
            source_hash: None,
            complete: false,
        },
    ]
}

/// Lookup by id in the official kernel catalog.
#[must_use]
pub fn official_contract(id: &str) -> Option<ComponentContract> {
    official_kernel_contracts()
        .into_iter()
        .find(|c| c.id == id)
}

/// All official ids.
#[must_use]
pub fn official_ids() -> Vec<String> {
    official_kernel_contracts()
        .into_iter()
        .map(|c| c.id)
        .collect()
}

