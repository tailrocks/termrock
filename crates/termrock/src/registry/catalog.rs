// SPDX-FileCopyrightText: 2026 Alexey Zhokhov
// SPDX-License-Identifier: Apache-2.0

//! Official kernel inventory contracts (embedded catalog).
use super::contract::{
    AnatomyPartRef, CONTRACT_SCHEMA, CapabilityRequirements, ComponentContract,
    ContractDependencies, ContractFile, ContractFileRole, KernelRequirement, OutcomeRef,
    Provenance, RegistryItemKind, SemanticRoleRef, VariantRef,
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
        color: vec!["truecolor".into(), "256".into(), "16".into(), "mono".into()],
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
                    "docs/public/preview-posters/panel-stack-omission.json",
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
            id: "junie-theme".into(),
            title: "Junie theme".into(),
            description: "Canonical junie design system (RolePalette).".into(),
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
                id: "junie".into(),
                description: "Canonical junie palette".into(),
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
            id: "SemanticScene".into(),
            title: "SemanticScene".into(),
            description: "Frame-local semantic tree: register, hit-test, jump, help, snapshots, FocusGraph projection.".into(),
            kind: RegistryItemKind::Primitive,
            license: "Apache-2.0".into(),
            module: Some("termrock::interaction::SemanticScene".into()),
            namespace: "termrock".into(),
            version: "0.13.0".into(),
            files: vec![file(
                "crates/termrock/src/interaction/scene.rs",
                ContractFileRole::Primary,
            )],
            dependencies: kernel_dep(),
            capabilities: caps_basic(),
            anatomy: vec![
                AnatomyPartRef {
                    id: "nodes".into(),
                    label: "Semantic nodes".into(),
                },
                AnatomyPartRef {
                    id: "diagnostics".into(),
                    label: "Collision diagnostics".into(),
                },
                AnatomyPartRef {
                    id: "snapshot".into(),
                    label: "Portable snapshot".into(),
                },
            ],
            semantic_roles: vec![SemanticRoleRef {
                id: "SemanticRole::*".into(),
            }],
            variants: vec![],
            outcomes: vec![],
            stories: vec![
                "semantic-scene/tree".into(),
                "semantic-scene/hit-jump".into(),
                "semantic-scene/snapshot".into(),
                "semantic-scene/virt-window".into(),
            ],
            tests: vec!["interaction::scene".into()],
            migration: Some("migrations/0102-v0.13.0-semantic-scene-premium.md".into()),
            provenance: prov("crates/termrock/src/interaction/scene.rs"),
            source_hash: None,
            complete: false,
        },
        ComponentContract {
            schema: CONTRACT_SCHEMA,
            id: "UiContext".into(),
            title: "UiContext".into(),
            description: "Per-frame coordination: design, caps, keymap, scene, focus, overlays, semantics, clock — no retained DOM.".into(),
            kind: RegistryItemKind::Primitive,
            license: "Apache-2.0".into(),
            module: Some("termrock::context::UiContext".into()),
            namespace: "termrock".into(),
            version: "0.13.0".into(),
            files: vec![file(
                "crates/termrock/src/context.rs",
                ContractFileRole::Primary,
            )],
            dependencies: {
                let mut d = kernel_dep();
                d.registry = vec![
                    "termrock/DesignSystem".into(),
                    "termrock/FocusGraph".into(),
                    "termrock/OverlayStack".into(),
                ];
                d
            },
            capabilities: caps_basic(),
            anatomy: vec![
                AnatomyPartRef {
                    id: "design".into(),
                    label: "DesignSystem".into(),
                },
                AnatomyPartRef {
                    id: "scene".into(),
                    label: "InteractionScene".into(),
                },
                AnatomyPartRef {
                    id: "focus".into(),
                    label: "FocusGraph".into(),
                },
                AnatomyPartRef {
                    id: "overlays".into(),
                    label: "OverlayStack".into(),
                },
                AnatomyPartRef {
                    id: "semantics".into(),
                    label: "SemanticScene".into(),
                },
                AnatomyPartRef {
                    id: "tick".into(),
                    label: "FrameTick".into(),
                },
            ],
            semantic_roles: vec![],
            variants: vec![],
            outcomes: vec![],
            stories: vec!["ui-context/frame".into(), "ui-context/nested".into()],
            tests: vec!["context".into()],
            migration: Some("migrations/0101-v0.13.0-ui-context.md".into()),
            provenance: prov("crates/termrock/src/context.rs"),
            source_hash: None,
            complete: false,
        },
        ComponentContract {
            schema: CONTRACT_SCHEMA,
            id: "DesignSystem".into(),
            title: "DesignSystem".into(),
            description: "Complete terminal design system: roles, recipes, one theme package, capability ladders.".into(),
            kind: RegistryItemKind::Theme,
            license: "Apache-2.0".into(),
            module: Some("termrock::style::DesignSystem".into()),
            namespace: "termrock".into(),
            version: "0.13.0".into(),
            files: vec![
                file(
                    "crates/termrock/src/style/tokens.rs",
                    ContractFileRole::Primary,
                ),
                file(
                    "crates/termrock/src/style/mod.rs",
                    ContractFileRole::Primary,
                ),
            ],
            dependencies: kernel_dep(),
            capabilities: caps_basic(),
            anatomy: vec![
                AnatomyPartRef {
                    id: "palette".into(),
                    label: "Role palette".into(),
                },
                AnatomyPartRef {
                    id: "recipes".into(),
                    label: "Component recipes".into(),
                },
                AnatomyPartRef {
                    id: "package".into(),
                    label: "Theme package".into(),
                },
            ],
            semantic_roles: vec![SemanticRoleRef {
                id: "Role::*".into(),
            }],
            variants: vec![VariantRef {
                id: "junie".into(),
                description: "Canonical junie system".into(),
            }],
            outcomes: vec![],
            stories: vec![
                "design-system/presets".into(),
                "design-system/no-color".into(),
                "design-system/button-recipes".into(),
            ],
            tests: vec!["style".into()],
            migration: Some("migrations/0100-v0.13.0-design-system-recipes.md".into()),
            provenance: prov("crates/termrock/src/style"),
            source_hash: None,
            complete: false,
        },
        ComponentContract {
            schema: CONTRACT_SCHEMA,
            id: "Center".into(),
            title: "Center".into(),
            description: "Axis-aware constrained centering; pure geometry, no fake focus node.".into(),
            kind: RegistryItemKind::Primitive,
            license: "Apache-2.0".into(),
            module: Some("termrock::layout::Center".into()),
            namespace: "termrock".into(),
            version: "0.13.0".into(),
            files: vec![file(
                "crates/termrock/src/layout/center.rs",
                ContractFileRole::Primary,
            )],
            dependencies: kernel_dep(),
            capabilities: caps_basic(),
            anatomy: vec![
                AnatomyPartRef {
                    id: "area".into(),
                    label: "Outer area".into(),
                },
                AnatomyPartRef {
                    id: "child".into(),
                    label: "Child rect".into(),
                },
            ],
            semantic_roles: vec![],
            variants: vec![
                VariantRef {
                    id: "both".into(),
                    description: "Both axes".into(),
                },
                VariantRef {
                    id: "dialog".into(),
                    description: "Safe margin dialog".into(),
                },
            ],
            outcomes: vec![],
            stories: vec![
                "center/both".into(),
                "center/dialog".into(),
                "center/tiny".into(),
            ],
            tests: vec!["layout::center".into()],
            migration: Some("migrations/0099-v0.13.0-center.md".into()),
            provenance: prov("crates/termrock/src/layout/center.rs"),
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
                VariantRef {
                    id: "settings".into(),
                    description: "settings_grid_template".into(),
                },
            ],
            outcomes: vec![],
            stories: vec![
                "grid/columns".into(),
                "grid/span".into(),
                "grid/dashboard".into(),
                "grid/form".into(),
                "grid/settings".into(),
                "grid/narrow".into(),
                "grid/overflow".into(),
                "grid/nav".into(),
            ],
            tests: vec!["layout::grid".into()],
            migration: Some("migrations/0105-v0.13.0-grid-premium.md".into()),
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
                    "docs/public/preview-posters/card-basic.json",
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
                    "docs/public/preview-posters/surface-ladder.json",
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
                    "docs/public/preview-posters/app-shell-workbench.json",
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
            description: "North-star application block composed from public TermRock APIs only."
                .into(),
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
                    "termrock/TaskRail".into(),
                    "termrock/ActivityShelf".into(),
                    "termrock/PromptComposer".into(),
                    "termrock/PermissionPrompt".into(),
                    "termrock/QuestionFlow".into(),
                    "termrock/PlanReview".into(),
                    "termrock/DiffReview".into(),
                    "termrock/SessionPicker".into(),
                    "termrock/WorkingStateCard".into(),
                    "termrock/Panel".into(),
                ];
                d
            },
            capabilities: {
                let mut c = caps_basic();
                c.responsive_surface = Some("Workspace".into());
                c.min_width = Some(30);
                c.min_height = Some(12);
                c
            },
            anatomy: vec![
                AnatomyPartRef {
                    id: "task_rail".into(),
                    label: "Task rail".into(),
                },
                AnatomyPartRef {
                    id: "transcript".into(),
                    label: "Transcript / message thread".into(),
                },
                AnatomyPartRef {
                    id: "activity".into(),
                    label: "Activity shelf".into(),
                },
                AnatomyPartRef {
                    id: "working".into(),
                    label: "Working state".into(),
                },
                AnatomyPartRef {
                    id: "composer".into(),
                    label: "Prompt composer".into(),
                },
                AnatomyPartRef {
                    id: "status".into(),
                    label: "Status strip".into(),
                },
            ],
            semantic_roles: vec![],
            variants: vec![],
            outcomes: vec![],
            stories: vec![
                "agent-workbench/basic".into(),
                "agent-workbench/tool-running".into(),
                "agent-workbench/permission".into(),
                "agent-workbench/plan".into(),
                "agent-workbench/diff".into(),
                "agent-workbench/session".into(),
                "agent-workbench/multi-agent".into(),
                "agent-workbench/narrow".into(),
                "agent-workbench/tiny".into(),
                "agent-workbench/ascii".into(),
                "agent-workbench/no-color".into(),
            ],
            tests: vec!["patterns::agent_workbench".into()],
            migration: Some("migrations/0236-v0.13.0-agent-workbench.md".into()),
            provenance: prov("crates/termrock/src/patterns/agent_workbench.rs"),
            source_hash: None,
            complete: true,
        },
        ComponentContract {
            schema: CONTRACT_SCHEMA,
            id: "settings-screen".into(),
            title: "Settings Screen".into(),
            description: "Searchable settings block: Sidebar, SearchInput, Form, theme, keybindings."
                .into(),
            kind: RegistryItemKind::Block,
            license: "Apache-2.0".into(),
            module: Some("termrock::patterns::settings_screen".into()),
            namespace: "termrock".into(),
            version: "0.13.0".into(),
            files: vec![file(
                "crates/termrock/src/patterns/settings_screen.rs",
                ContractFileRole::Primary,
            )],
            dependencies: {
                let mut d = kernel_dep();
                d.registry = vec![
                    "termrock/Sidebar".into(),
                    "termrock/SearchInput".into(),
                    "termrock/Form".into(),
                    "termrock/ThemePicker".into(),
                    "termrock/KeybindingRecorder".into(),
                    "termrock/KeyboardHelp".into(),
                    "termrock/Panel".into(),
                ];
                d
            },
            capabilities: {
                let mut c = caps_basic();
                c.responsive_surface = Some("SettingsDensity".into());
                c.min_width = Some(40);
                c.min_height = Some(12);
                c
            },
            anatomy: vec![
                AnatomyPartRef {
                    id: "search".into(),
                    label: "Search".into(),
                },
                AnatomyPartRef {
                    id: "nav".into(),
                    label: "Category sidebar".into(),
                },
                AnatomyPartRef {
                    id: "body".into(),
                    label: "Form / theme / keybinding body".into(),
                },
                AnatomyPartRef {
                    id: "footer".into(),
                    label: "Save / reset strip".into(),
                },
            ],
            semantic_roles: vec![],
            variants: vec![],
            outcomes: vec![],
            stories: vec![
                "settings-screen/basic".into(),
                "settings-screen/search".into(),
                "settings-screen/validation".into(),
                "settings-screen/conflicts".into(),
                "settings-screen/theme".into(),
                "settings-screen/keybinding".into(),
                "settings-screen/narrow".into(),
                "settings-screen/tiny".into(),
                "settings-screen/no-results".into(),
                "settings-screen/help".into(),
            ],
            tests: vec!["patterns::settings_screen".into()],
            migration: Some("migrations/0237-v0.13.0-settings-screen.md".into()),
            provenance: prov("crates/termrock/src/patterns/settings_screen.rs"),
            source_hash: None,
            complete: true,
        },
        ComponentContract {
            schema: CONTRACT_SCHEMA,
            id: "setup-wizard".into(),
            title: "Setup Wizard".into(),
            description:
                "Premium first-run / onboarding flow over FormWizard + Stepper with safe cancel."
                    .into(),
            kind: RegistryItemKind::Block,
            license: "Apache-2.0".into(),
            module: Some("termrock::patterns::setup_wizard".into()),
            namespace: "termrock".into(),
            version: "0.13.0".into(),
            files: vec![file(
                "crates/termrock/src/patterns/setup_wizard.rs",
                ContractFileRole::Primary,
            )],
            dependencies: {
                let mut d = kernel_dep();
                d.registry = vec![
                    "termrock/FormWizard".into(),
                    "termrock/Stepper".into(),
                    "termrock/Form".into(),
                    "termrock/EmptyState".into(),
                    "termrock/ThemePicker".into(),
                    "termrock/PermissionPrompt".into(),
                ];
                d
            },
            capabilities: {
                let mut c = caps_basic();
                c.responsive_surface = Some("FormWizard".into());
                c.min_width = Some(32);
                c.min_height = Some(10);
                c
            },
            anatomy: vec![
                AnatomyPartRef {
                    id: "stepper".into(),
                    label: "Stepper".into(),
                },
                AnatomyPartRef {
                    id: "body".into(),
                    label: "Step body".into(),
                },
                AnatomyPartRef {
                    id: "nav".into(),
                    label: "Back / next / cancel".into(),
                },
            ],
            semantic_roles: vec![],
            variants: vec![],
            outcomes: vec![],
            stories: vec![
                "setup-wizard/welcome".into(),
                "setup-wizard/capability".into(),
                "setup-wizard/account".into(),
                "setup-wizard/permission".into(),
                "setup-wizard/theme".into(),
                "setup-wizard/summary".into(),
                "setup-wizard/recovery".into(),
                "setup-wizard/inline".into(),
                "setup-wizard/resume".into(),
                "setup-wizard/cancel-confirm".into(),
            ],
            tests: vec!["patterns::setup_wizard".into()],
            migration: Some("migrations/0238-v0.13.0-setup-wizard.md".into()),
            provenance: prov("crates/termrock/src/patterns/setup_wizard.rs"),
            source_hash: None,
            complete: true,
        },
        ComponentContract {
            schema: CONTRACT_SCHEMA,
            id: "connection-manager".into(),
            title: "Connection Manager".into(),
            description: "Reusable DB/SSH/API/service connection inventory: list, status, search, groups, recent, favorites, add/edit/test, safe secrets, reconnect, gated delete; launcher + full views; OfflineState and diagnostic projection. Protocol and persistence host-owned.".into(),
            kind: RegistryItemKind::Block,
            license: "Apache-2.0".into(),
            module: Some("termrock::patterns::connection_manager".into()),
            namespace: "termrock".into(),
            version: "0.13.0".into(),
            files: vec![file(
                "crates/termrock/src/patterns/connection_manager.rs",
                ContractFileRole::Primary,
            )],
            dependencies: {
                let mut d = kernel_dep();
                d.registry = vec![
                    "termrock/Panel".into(),
                    "termrock/PasswordInput".into(),
                ];
                d
            },
            capabilities: {
                let mut c = caps_basic();
                c.responsive_surface = Some("List".into());
                c.min_width = Some(20);
                c.min_height = Some(8);
                c
            },
            anatomy: vec![
                AnatomyPartRef {
                    id: "search".into(),
                    label: "Search / view filter".into(),
                },
                AnatomyPartRef {
                    id: "list".into(),
                    label: "Connection list".into(),
                },
                AnatomyPartRef {
                    id: "detail".into(),
                    label: "Detail / diagnostics".into(),
                },
                AnatomyPartRef {
                    id: "form".into(),
                    label: "Add/edit form + secret".into(),
                },
                AnatomyPartRef {
                    id: "confirm".into(),
                    label: "Delete confirm".into(),
                },
            ],
            semantic_roles: vec![],
            variants: vec![
                VariantRef {
                    id: "launcher".into(),
                    description: "Compact launcher".into(),
                },
                VariantRef {
                    id: "full".into(),
                    description: "Full management".into(),
                },
            ],
            outcomes: vec![
                OutcomeRef {
                    id: "ConnectRequested".into(),
                },
                OutcomeRef {
                    id: "TestRequested".into(),
                },
                OutcomeRef {
                    id: "SaveRequested".into(),
                },
                OutcomeRef {
                    id: "DeleteRequested".into(),
                },
            ],
            stories: vec![
                "connection-manager/full".into(),
                "connection-manager/launcher".into(),
                "connection-manager/empty".into(),
                "connection-manager/error".into(),
                "connection-manager/secret".into(),
                "connection-manager/confirm".into(),
                "connection-manager/narrow".into(),
                "connection-manager/unicode".into(),
            ],
            tests: vec!["patterns::connection_manager".into()],
            migration: Some("migrations/0239-v0.13.0-connection-manager.md".into()),
            provenance: prov("crates/termrock/src/patterns/connection_manager.rs"),
            source_hash: None,
            complete: true,
        },
        ComponentContract {
            schema: CONTRACT_SCHEMA,
            id: "database-workbench".into(),
            title: "Database Workbench".into(),
            description: "Source-owned database application composition: ConnectionManager, SchemaBrowser, QueryEditor, ResultGrid, ObjectInspector, HistoryPicker, StatusBar, CommandPalette; focus zones, density collapse, typed run/cancel/export messages; host owns SQL I/O.".into(),
            kind: RegistryItemKind::Block,
            license: "Apache-2.0".into(),
            module: Some("termrock::patterns::database_workbench".into()),
            namespace: "termrock".into(),
            version: "0.13.0".into(),
            files: vec![file(
                "crates/termrock/src/patterns/database_workbench.rs",
                ContractFileRole::Primary,
            )],
            dependencies: {
                let mut d = kernel_dep();
                d.registry = vec![
                    "termrock/ConnectionManager".into(),
                    "termrock/Panel".into(),
                ];
                d
            },
            capabilities: {
                let mut c = caps_basic();
                c.responsive_surface = Some("Workbench".into());
                c.min_width = Some(30);
                c.min_height = Some(12);
                c
            },
            anatomy: vec![
                AnatomyPartRef {
                    id: "connections".into(),
                    label: "Connection inventory".into(),
                },
                AnatomyPartRef {
                    id: "schema".into(),
                    label: "Schema browser".into(),
                },
                AnatomyPartRef {
                    id: "query".into(),
                    label: "Query tabs/editor".into(),
                },
                AnatomyPartRef {
                    id: "results".into(),
                    label: "Result grid".into(),
                },
                AnatomyPartRef {
                    id: "inspector".into(),
                    label: "Object inspector".into(),
                },
                AnatomyPartRef {
                    id: "status".into(),
                    label: "Status bar".into(),
                },
            ],
            semantic_roles: vec![],
            variants: vec![
                VariantRef {
                    id: "normal".into(),
                    description: "Full multi-pane".into(),
                },
                VariantRef {
                    id: "narrow".into(),
                    description: "Inspector collapsed".into(),
                },
                VariantRef {
                    id: "tiny".into(),
                    description: "Query + results + status".into(),
                },
            ],
            outcomes: vec![
                OutcomeRef {
                    id: "RunRequested".into(),
                },
                OutcomeRef {
                    id: "CancelRequested".into(),
                },
                OutcomeRef {
                    id: "ExportRequested".into(),
                },
                OutcomeRef {
                    id: "RunBlocked".into(),
                },
            ],
            stories: vec![
                "database-workbench/basic".into(),
                "database-workbench/disconnected".into(),
                "database-workbench/error".into(),
                "database-workbench/running".into(),
                "database-workbench/narrow".into(),
                "database-workbench/unicode".into(),
            ],
            tests: vec!["patterns::database_workbench".into()],
            migration: Some("migrations/0240-v0.13.0-database-workbench.md".into()),
            provenance: prov("crates/termrock/src/patterns/database_workbench.rs"),
            source_hash: None,
            complete: true,
        },
        ComponentContract {
            schema: CONTRACT_SCHEMA,
            id: "git-workbench".into(),
            title: "Git Workbench".into(),
            description: "Source-owned Git workflow composition: FileTree, DiffReview, history timeline, branches, TerminalOutput, conflict diagnostics, StatusBar, KeyboardHelp; stage/discard confirms; fullscreen diff; host owns Git I/O.".into(),
            kind: RegistryItemKind::Block,
            license: "Apache-2.0".into(),
            module: Some("termrock::patterns::git_workbench".into()),
            namespace: "termrock".into(),
            version: "0.13.0".into(),
            files: vec![file(
                "crates/termrock/src/patterns/git_workbench.rs",
                ContractFileRole::Primary,
            )],
            dependencies: {
                let mut d = kernel_dep();
                d.registry = vec![
                    "termrock/Panel".into(),
                ];
                d
            },
            capabilities: {
                let mut c = caps_basic();
                c.responsive_surface = Some("Workbench".into());
                c.min_width = Some(30);
                c.min_height = Some(12);
                c
            },
            anatomy: vec![
                AnatomyPartRef {
                    id: "files".into(),
                    label: "File tree / status".into(),
                },
                AnatomyPartRef {
                    id: "diff".into(),
                    label: "Diff review".into(),
                },
                AnatomyPartRef {
                    id: "history".into(),
                    label: "Commit history".into(),
                },
                AnatomyPartRef {
                    id: "branches".into(),
                    label: "Branches".into(),
                },
                AnatomyPartRef {
                    id: "output".into(),
                    label: "Command output".into(),
                },
                AnatomyPartRef {
                    id: "status".into(),
                    label: "Status bar".into(),
                },
            ],
            semantic_roles: vec![],
            variants: vec![
                VariantRef {
                    id: "normal".into(),
                    description: "Full multi-pane".into(),
                },
                VariantRef {
                    id: "narrow".into(),
                    description: "Files + diff + output".into(),
                },
                VariantRef {
                    id: "fullscreen-diff".into(),
                    description: "Promoted diff".into(),
                },
            ],
            outcomes: vec![
                OutcomeRef {
                    id: "StageRequested".into(),
                },
                OutcomeRef {
                    id: "DiscardRequested".into(),
                },
                OutcomeRef {
                    id: "CheckoutRequested".into(),
                },
                OutcomeRef {
                    id: "FullscreenDiff".into(),
                },
            ],
            stories: vec![
                "git-workbench/basic".into(),
                "git-workbench/conflict".into(),
                "git-workbench/narrow".into(),
                "git-workbench/fullscreen-diff".into(),
                "git-workbench/unicode".into(),
                "git-workbench/clean".into(),
                "git-workbench/empty".into(),
            ],
            tests: vec!["patterns::git_workbench".into()],
            migration: Some("migrations/0241-v0.13.0-git-workbench.md".into()),
            provenance: prov("crates/termrock/src/patterns/git_workbench.rs"),
            source_hash: None,
            complete: true,
        },
        ComponentContract {
            schema: CONTRACT_SCHEMA,
            id: "observability-dashboard".into(),
            title: "Observability Dashboard".into(),
            description: "Logs and metrics operational monitoring composition: SearchInput, LogStream, EventStream, MetricsDashboard, ObjectInspector, StatusBar; live/pause, dropped/reconnect, bookmarks, drill-down; host owns acquisition.".into(),
            kind: RegistryItemKind::Block,
            license: "Apache-2.0".into(),
            module: Some("termrock::patterns::observability_dashboard".into()),
            namespace: "termrock".into(),
            version: "0.13.0".into(),
            files: vec![file(
                "crates/termrock/src/patterns/observability_dashboard.rs",
                ContractFileRole::Primary,
            )],
            dependencies: {
                let mut d = kernel_dep();
                d.registry = vec!["termrock/Panel".into()];
                d
            },
            capabilities: {
                let mut c = caps_basic();
                c.responsive_surface = Some("Workbench".into());
                c.min_width = Some(40);
                c.min_height = Some(12);
                c
            },
            anatomy: vec![
                AnatomyPartRef {
                    id: "search".into(),
                    label: "Query / filter".into(),
                },
                AnatomyPartRef {
                    id: "metrics".into(),
                    label: "Metrics + alerts".into(),
                },
                AnatomyPartRef {
                    id: "logs".into(),
                    label: "Log stream".into(),
                },
                AnatomyPartRef {
                    id: "events".into(),
                    label: "Event stream".into(),
                },
                AnatomyPartRef {
                    id: "inspector".into(),
                    label: "Details inspector".into(),
                },
                AnatomyPartRef {
                    id: "status".into(),
                    label: "Status summary".into(),
                },
            ],
            semantic_roles: vec![],
            variants: vec![
                VariantRef {
                    id: "normal".into(),
                    description: "Full multi-pane".into(),
                },
                VariantRef {
                    id: "narrow".into(),
                    description: "No inspector".into(),
                },
                VariantRef {
                    id: "tiny".into(),
                    description: "Search + logs + status".into(),
                },
            ],
            outcomes: vec![
                OutcomeRef {
                    id: "LiveToggled".into(),
                },
                OutcomeRef {
                    id: "BookmarkToggled".into(),
                },
                OutcomeRef {
                    id: "AckDropped".into(),
                },
                OutcomeRef {
                    id: "DrillDown".into(),
                },
            ],
            stories: vec![
                "observability-dashboard/basic".into(),
                "observability-dashboard/failure".into(),
                "observability-dashboard/narrow".into(),
                "observability-dashboard/unicode".into(),
            ],
            tests: vec!["patterns::observability_dashboard".into()],
            migration: Some("migrations/0242-v0.13.0-logs-observability-dashboard.md".into()),
            provenance: prov("crates/termrock/src/patterns/observability_dashboard.rs"),
            source_hash: None,
            complete: true,
        },
        ComponentContract {
            schema: CONTRACT_SCHEMA,
            id: "file-manager".into(),
            title: "File Manager".into(),
            description: "Source-owned file-management composition: Breadcrumbs, SearchInput, FileTree, PreviewCard, QuickOpen, operation queue, StatusBar, confirm/conflict dialogs; copy/move/delete/rename/new typed requests; host owns FS I/O.".into(),
            kind: RegistryItemKind::Block,
            license: "Apache-2.0".into(),
            module: Some("termrock::patterns::file_manager".into()),
            namespace: "termrock".into(),
            version: "0.13.0".into(),
            files: vec![file(
                "crates/termrock/src/patterns/file_manager.rs",
                ContractFileRole::Primary,
            )],
            dependencies: {
                let mut d = kernel_dep();
                d.registry = vec!["termrock/Panel".into()];
                d
            },
            capabilities: {
                let mut c = caps_basic();
                c.responsive_surface = Some("Workbench".into());
                c.min_width = Some(40);
                c.min_height = Some(12);
                c
            },
            anatomy: vec![
                AnatomyPartRef {
                    id: "breadcrumbs".into(),
                    label: "Path breadcrumbs".into(),
                },
                AnatomyPartRef {
                    id: "search".into(),
                    label: "Filter / search".into(),
                },
                AnatomyPartRef {
                    id: "tree".into(),
                    label: "File tree".into(),
                },
                AnatomyPartRef {
                    id: "preview".into(),
                    label: "Preview card".into(),
                },
                AnatomyPartRef {
                    id: "queue".into(),
                    label: "Operation queue".into(),
                },
                AnatomyPartRef {
                    id: "status".into(),
                    label: "Status bar".into(),
                },
            ],
            semantic_roles: vec![],
            variants: vec![
                VariantRef {
                    id: "normal".into(),
                    description: "Full multi-pane".into(),
                },
                VariantRef {
                    id: "narrow".into(),
                    description: "No queue; preview as drawer".into(),
                },
                VariantRef {
                    id: "tiny".into(),
                    description: "Search + tree + status".into(),
                },
            ],
            outcomes: vec![
                OutcomeRef {
                    id: "CopyRequested".into(),
                },
                OutcomeRef {
                    id: "MoveRequested".into(),
                },
                OutcomeRef {
                    id: "DeleteRequested".into(),
                },
                OutcomeRef {
                    id: "ConflictResolved".into(),
                },
            ],
            stories: vec![
                "file-manager/basic".into(),
                "file-manager/conflict".into(),
                "file-manager/narrow".into(),
                "file-manager/unicode".into(),
            ],
            tests: vec!["patterns::file_manager".into()],
            migration: Some("migrations/0243-v0.13.0-file-manager.md".into()),
            provenance: prov("crates/termrock/src/patterns/file_manager.rs"),
            source_hash: None,
            complete: true,
        },
        ComponentContract {
            schema: CONTRACT_SCHEMA,
            id: "project-launcher".into(),
            title: "Project Launcher".into(),
            description: "Fast project/session launcher: SearchInput, grouped projects List, SessionPicker, PreviewCard, QuickOpen, ConnectionStatus chrome, EmptyState onboarding; open/new/import/favorite/session typed requests; home + inline modes; host owns discovery/persistence.".into(),
            kind: RegistryItemKind::Block,
            license: "Apache-2.0".into(),
            module: Some("termrock::patterns::project_launcher".into()),
            namespace: "termrock".into(),
            version: "0.13.0".into(),
            files: vec![file(
                "crates/termrock/src/patterns/project_launcher.rs",
                ContractFileRole::Primary,
            )],
            dependencies: {
                let mut d = kernel_dep();
                d.registry = vec!["termrock/Panel".into()];
                d
            },
            capabilities: {
                let mut c = caps_basic();
                c.responsive_surface = Some("Workbench".into());
                c.min_width = Some(40);
                c.min_height = Some(12);
                c
            },
            anatomy: vec![
                AnatomyPartRef {
                    id: "search".into(),
                    label: "Filter / search".into(),
                },
                AnatomyPartRef {
                    id: "projects".into(),
                    label: "Project list".into(),
                },
                AnatomyPartRef {
                    id: "sessions".into(),
                    label: "Session picker".into(),
                },
                AnatomyPartRef {
                    id: "preview".into(),
                    label: "Preview card".into(),
                },
                AnatomyPartRef {
                    id: "onboarding".into(),
                    label: "Onboarding empty".into(),
                },
                AnatomyPartRef {
                    id: "status".into(),
                    label: "Status bar".into(),
                },
            ],
            semantic_roles: vec![],
            variants: vec![
                VariantRef {
                    id: "home".into(),
                    description: "Full-screen home".into(),
                },
                VariantRef {
                    id: "inline".into(),
                    description: "Compact quick launcher".into(),
                },
                VariantRef {
                    id: "narrow".into(),
                    description: "No preview".into(),
                },
            ],
            outcomes: vec![
                OutcomeRef {
                    id: "OpenRequested".into(),
                },
                OutcomeRef {
                    id: "NewRequested".into(),
                },
                OutcomeRef {
                    id: "ImportRequested".into(),
                },
                OutcomeRef {
                    id: "SessionResume".into(),
                },
            ],
            stories: vec![
                "project-launcher/basic".into(),
                "project-launcher/stale".into(),
                "project-launcher/narrow".into(),
                "project-launcher/inline".into(),
                "project-launcher/unicode".into(),
            ],
            tests: vec!["patterns::project_launcher".into()],
            migration: Some("migrations/0244-v0.13.0-project-launcher.md".into()),
            provenance: prov("crates/termrock/src/patterns/project_launcher.rs"),
            source_hash: None,
            complete: true,
        },
        ComponentContract {
            schema: CONTRACT_SCHEMA,
            id: "help-center".into(),
            title: "Help Center / Command Reference".into(),
            description: "Contextual product help: SearchInput, topic nav, KeyboardHelp (live HelpEntry/keymap SoT), command list from HelpEntry, MarkdownView body, DoctorReport diagnostics, registry inspect; compact overlay + full docs; host owns markdown and command execution.".into(),
            kind: RegistryItemKind::Block,
            license: "Apache-2.0".into(),
            module: Some("termrock::patterns::help_center".into()),
            namespace: "termrock".into(),
            version: "0.13.0".into(),
            files: vec![file(
                "crates/termrock/src/patterns/help_center.rs",
                ContractFileRole::Primary,
            )],
            dependencies: {
                let mut d = kernel_dep();
                d.registry = vec!["termrock/Panel".into()];
                d
            },
            capabilities: {
                let mut c = caps_basic();
                c.responsive_surface = Some("Workbench".into());
                c.min_width = Some(40);
                c.min_height = Some(12);
                c
            },
            anatomy: vec![
                AnatomyPartRef {
                    id: "search".into(),
                    label: "Search".into(),
                },
                AnatomyPartRef {
                    id: "nav".into(),
                    label: "Topic nav".into(),
                },
                AnatomyPartRef {
                    id: "keyboard".into(),
                    label: "Keyboard map".into(),
                },
                AnatomyPartRef {
                    id: "commands".into(),
                    label: "Command reference".into(),
                },
                AnatomyPartRef {
                    id: "body".into(),
                    label: "Markdown body".into(),
                },
                AnatomyPartRef {
                    id: "diagnostics".into(),
                    label: "Doctor diagnostics".into(),
                },
            ],
            semantic_roles: vec![],
            variants: vec![
                VariantRef {
                    id: "full".into(),
                    description: "Full documentation".into(),
                },
                VariantRef {
                    id: "compact".into(),
                    description: "Compact overlay".into(),
                },
                VariantRef {
                    id: "narrow".into(),
                    description: "No keyboard pane".into(),
                },
            ],
            outcomes: vec![
                OutcomeRef {
                    id: "TopicOpened".into(),
                },
                OutcomeRef {
                    id: "CommandRun".into(),
                },
                OutcomeRef {
                    id: "DoctorOpened".into(),
                },
                OutcomeRef {
                    id: "LinkFollowed".into(),
                },
            ],
            stories: vec![
                "help-center/basic".into(),
                "help-center/compact".into(),
                "help-center/narrow".into(),
                "help-center/doctor".into(),
                "help-center/unicode".into(),
            ],
            tests: vec!["patterns::help_center".into()],
            migration: Some("migrations/0245-v0.13.0-help-center-command-reference.md".into()),
            provenance: prov("crates/termrock/src/patterns/help_center.rs"),
            source_hash: None,
            complete: true,
        },
        ComponentContract {
            schema: CONTRACT_SCHEMA,
            id: "error-recovery".into(),
            title: "Error Recovery / Crash Report".into(),
            description: "Graceful recovery for serious failures: ErrorState summary, preserved work, recovery action list, redacted crash report (secret redaction on copy/report path), full and inline-fallback modes; host owns restart, session restore, logs, issue trackers, panic hooks.".into(),
            kind: RegistryItemKind::Block,
            license: "Apache-2.0".into(),
            module: Some("termrock::patterns::error_recovery".into()),
            namespace: "termrock".into(),
            version: "0.13.0".into(),
            files: vec![file(
                "crates/termrock/src/patterns/error_recovery.rs",
                ContractFileRole::Primary,
            )],
            dependencies: {
                let mut d = kernel_dep();
                d.registry = vec!["termrock/Panel".into()];
                d
            },
            capabilities: {
                let mut c = caps_basic();
                c.responsive_surface = Some("Workbench".into());
                c.min_width = Some(32);
                c.min_height = Some(8);
                c
            },
            anatomy: vec![
                AnatomyPartRef {
                    id: "summary".into(),
                    label: "Error summary".into(),
                },
                AnatomyPartRef {
                    id: "actions".into(),
                    label: "Recovery options".into(),
                },
                AnatomyPartRef {
                    id: "diagnostics".into(),
                    label: "Redacted report".into(),
                },
                AnatomyPartRef {
                    id: "preserved".into(),
                    label: "Preserved work".into(),
                },
                AnatomyPartRef {
                    id: "status".into(),
                    label: "Status bar".into(),
                },
            ],
            semantic_roles: vec![],
            variants: vec![
                VariantRef {
                    id: "full".into(),
                    description: "Full multi-pane recovery".into(),
                },
                VariantRef {
                    id: "inline-fallback".into(),
                    description: "Inline when full-screen compromised".into(),
                },
            ],
            outcomes: vec![
                OutcomeRef {
                    id: "RestartRequested".into(),
                },
                OutcomeRef {
                    id: "CopyDiagnostics".into(),
                },
                OutcomeRef {
                    id: "SafeQuit".into(),
                },
                OutcomeRef {
                    id: "ReportIssue".into(),
                },
            ],
            stories: vec![
                "error-recovery/basic".into(),
                "error-recovery/redacted".into(),
                "error-recovery/inline".into(),
                "error-recovery/unicode".into(),
            ],
            tests: vec!["patterns::error_recovery".into()],
            migration: Some("migrations/0246-v0.13.0-error-recovery-crash-report.md".into()),
            provenance: prov("crates/termrock/src/patterns/error_recovery.rs"),
            source_hash: None,
            complete: true,
        },
        ComponentContract {
            schema: CONTRACT_SCHEMA,
            id: "auth-entry".into(),
            title: "Auth Entry".into(),
            description: "Keyboard-first sign-up/sign-in/email-only gate: identity + password (+ confirm/terms) or passwordless request, validation, submit/cancel, forgot/oauth secondaries; host owns auth I/O and secrets.".into(),
            kind: RegistryItemKind::Block,
            license: "Apache-2.0".into(),
            module: Some("termrock::patterns::auth_entry".into()),
            namespace: "termrock".into(),
            version: "0.13.0".into(),
            files: vec![file(
                "crates/termrock/src/patterns/auth_entry.rs",
                ContractFileRole::Primary,
            )],
            dependencies: kernel_dep(),
            capabilities: caps_basic(),
            anatomy: vec![
                AnatomyPartRef {
                    id: "identity".into(),
                    label: "Identity field".into(),
                },
                AnatomyPartRef {
                    id: "password".into(),
                    label: "Password field".into(),
                },
                AnatomyPartRef {
                    id: "confirm".into(),
                    label: "Confirm password".into(),
                },
                AnatomyPartRef {
                    id: "terms".into(),
                    label: "Terms checkbox".into(),
                },
                AnatomyPartRef {
                    id: "aside".into(),
                    label: "Optional aside text".into(),
                },
            ],
            semantic_roles: vec![],
            variants: vec![
                VariantRef {
                    id: "sign-up".into(),
                    description: "Create account with confirm/terms".into(),
                },
                VariantRef {
                    id: "sign-in".into(),
                    description: "Existing account password login".into(),
                },
                VariantRef {
                    id: "email-only".into(),
                    description: "Passwordless magic-link request".into(),
                },
            ],
            outcomes: vec![
                OutcomeRef {
                    id: "Submitted".into(),
                },
                OutcomeRef {
                    id: "ValidationFailed".into(),
                },
                OutcomeRef {
                    id: "ModeSwitched".into(),
                },
                OutcomeRef {
                    id: "Cancelled".into(),
                },
                OutcomeRef {
                    id: "SecondaryAction".into(),
                },
            ],
            stories: vec![
                "auth-entry/basic".into(),
                "auth-entry/sign-in".into(),
                "auth-entry/email-only".into(),
            ],
            tests: vec!["patterns::auth_entry".into()],
            migration: Some("migrations/0249-v0.13.0-auth-entry-login-email-only.md".into()),
            provenance: prov("crates/termrock/src/patterns/auth_entry.rs"),
            source_hash: None,
            complete: true,
        },
        ComponentContract {
            schema: CONTRACT_SCHEMA,
            id: "input-otp".into(),
            title: "Input OTP".into(),
            description: "Fixed-slot OTP/PIN entry with auto-advance, paste fill, mask option; host owns verification.".into(),
            kind: RegistryItemKind::Component,
            license: "Apache-2.0".into(),
            module: Some("termrock::widgets::input_otp".into()),
            namespace: "termrock".into(),
            version: "0.13.0".into(),
            files: vec![file(
                "crates/termrock/src/widgets/input_otp.rs",
                ContractFileRole::Primary,
            )],
            dependencies: kernel_dep(),
            capabilities: caps_basic(),
            anatomy: vec![AnatomyPartRef {
                id: "slots".into(),
                label: "Digit slots".into(),
            }],
            semantic_roles: vec![],
            variants: vec![VariantRef {
                id: "digits".into(),
                description: "Numeric OTP".into(),
            }],
            outcomes: vec![OutcomeRef {
                id: "Completed".into(),
            }],
            stories: vec!["input-otp/basic".into()],
            tests: vec!["widgets::input_otp".into()],
            migration: Some(
                "migrations/0247-v0.13.0-shadcn-gap-input-otp-carousel-input-group.md".into(),
            ),
            provenance: prov("crates/termrock/src/widgets/input_otp.rs"),
            source_hash: None,
            complete: true,
        },
        ComponentContract {
            schema: CONTRACT_SCHEMA,
            id: "carousel".into(),
            title: "Carousel".into(),
            description: "Multi-slide panel with keyboard prev/next, wrap, indicators, host-driven auto tick.".into(),
            kind: RegistryItemKind::Component,
            license: "Apache-2.0".into(),
            module: Some("termrock::widgets::carousel".into()),
            namespace: "termrock".into(),
            version: "0.13.0".into(),
            files: vec![file(
                "crates/termrock/src/widgets/carousel.rs",
                ContractFileRole::Primary,
            )],
            dependencies: kernel_dep(),
            capabilities: caps_basic(),
            anatomy: vec![
                AnatomyPartRef {
                    id: "slide".into(),
                    label: "Active slide".into(),
                },
                AnatomyPartRef {
                    id: "indicators".into(),
                    label: "Page dots".into(),
                },
            ],
            semantic_roles: vec![],
            variants: vec![VariantRef {
                id: "wrap".into(),
                description: "Wrap ends".into(),
            }],
            outcomes: vec![OutcomeRef {
                id: "Changed".into(),
            }],
            stories: vec!["carousel/basic".into()],
            tests: vec!["widgets::carousel".into()],
            migration: Some(
                "migrations/0247-v0.13.0-shadcn-gap-input-otp-carousel-input-group.md".into(),
            ),
            provenance: prov("crates/termrock/src/widgets/carousel.rs"),
            source_hash: None,
            complete: true,
        },
        ComponentContract {
            schema: CONTRACT_SCHEMA,
            id: "input-group".into(),
            title: "Input Group".into(),
            description: "Prefix/suffix addons around TextInput; Alt+Enter activates suffix action.".into(),
            kind: RegistryItemKind::Component,
            license: "Apache-2.0".into(),
            module: Some("termrock::widgets::input_group".into()),
            namespace: "termrock".into(),
            version: "0.13.0".into(),
            files: vec![file(
                "crates/termrock/src/widgets/input_group.rs",
                ContractFileRole::Primary,
            )],
            dependencies: kernel_dep(),
            capabilities: caps_basic(),
            anatomy: vec![
                AnatomyPartRef {
                    id: "prefix".into(),
                    label: "Prefix addon".into(),
                },
                AnatomyPartRef {
                    id: "field".into(),
                    label: "Text field".into(),
                },
                AnatomyPartRef {
                    id: "suffix".into(),
                    label: "Suffix addon".into(),
                },
            ],
            semantic_roles: vec![],
            variants: vec![VariantRef {
                id: "url".into(),
                description: "URL scheme + submit".into(),
            }],
            outcomes: vec![OutcomeRef {
                id: "AddonActivated".into(),
            }],
            stories: vec!["input-group/basic".into()],
            tests: vec!["widgets::input_group".into()],
            migration: Some(
                "migrations/0247-v0.13.0-shadcn-gap-input-otp-carousel-input-group.md".into(),
            ),
            provenance: prov("crates/termrock/src/widgets/input_group.rs"),
            source_hash: None,
            complete: true,
        },
    ]
}

/// Lookup by id in the official kernel catalog.
#[must_use]
pub fn official_contract(id: &str) -> Option<ComponentContract> {
    official_kernel_contracts().into_iter().find(|c| c.id == id)
}

/// All official ids.
#[must_use]
pub fn official_ids() -> Vec<String> {
    official_kernel_contracts()
        .into_iter()
        .map(|c| c.id)
        .collect()
}
