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
            description: "Complete terminal design system: roles, recipes, presets, packages, capability ladders.".into(),
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
                    id: "presets".into(),
                    label: "Theme presets".into(),
                },
            ],
            semantic_roles: vec![SemanticRoleRef {
                id: "Role::*".into(),
            }],
            variants: vec![
                VariantRef {
                    id: "phosphor".into(),
                    description: "Phosphor Obsidian default".into(),
                },
                VariantRef {
                    id: "slate".into(),
                    description: "Cool gray".into(),
                },
                VariantRef {
                    id: "paper".into(),
                    description: "Light paper".into(),
                },
                VariantRef {
                    id: "ansi".into(),
                    description: "ANSI 16".into(),
                },
                VariantRef {
                    id: "high-contrast".into(),
                    description: "A11y high contrast".into(),
                },
                VariantRef {
                    id: "adaptive".into(),
                    description: "Env capability ladder".into(),
                },
            ],
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
            module: Some("termrock::widgets::connection_manager".into()),
            namespace: "termrock".into(),
            version: "0.13.0".into(),
            files: vec![file(
                "crates/termrock/src/widgets/connection_manager.rs",
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
            tests: vec!["widgets::connection_manager".into()],
            migration: Some("migrations/0239-v0.13.0-connection-manager.md".into()),
            provenance: prov("crates/termrock/src/widgets/connection_manager.rs"),
            source_hash: None,
            complete: true,
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

