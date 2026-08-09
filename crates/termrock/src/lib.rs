//! termrock: shared TUI widgets, theme, and render helpers.
//!
//! **Architecture Invariant:** T1.
//! Entry point: [`Theme`] — shared TUI theme tokens.

pub mod ansi_text;
pub mod input;
pub mod interaction;
pub mod keymap;
pub mod layout;
pub mod osc;
pub mod patterns;
pub mod perf;
pub mod runtime;
pub mod scroll;
pub mod style;
pub mod text;
pub mod widgets;

#[cfg(feature = "crossterm")]
pub mod crossterm;

pub use interaction::{
    BackdropPolicy, InteractionElement, InteractionLayer, InteractionOutcome, InteractionScene,
    LayerDismissPolicy, LayerKind, NarrowFallback, NavigationMove, OverlayEntry, OverlayId,
    OverlayKind, OverlayOutcome, OverlayPolicy, OverlaySize, OverlaySpec, OverlayStack, PageMove,
    PlacementPrefer, SceneError, SemanticElement, SemanticRole, SemanticScene, UiIntent,
    default_list_intent, default_table_intent, default_tree_intent, dispatch_keymap_action,
    place_overlay,
};
pub use layout::{
    AdaptiveAnatomy, AnatomyPart, ContentPriority, ContractionStage, OverflowBehavior,
    ResponsiveSurface, SizeBudget, SurfaceResponsivePolicy, ViewportClass, WIDTH_LADDER,
    contract_parts, essential_survives,
};
pub use perf::{
    BackpressureSignal, BudgetKind, ComponentBudget, DirtyFlags, FollowMode, NewContentIndicator,
    PerfClass, ScrollAnchor, ScrollAnchorKind, StreamBatch, StreamCoalescer, UpdatePriority,
    apply_follow_after_append, budget_for, budgets, check_batch_budget, check_zero_alloc_steady,
    pause_follow_on_user_scroll,
};
pub use style::{
    Appearance, AppearanceThemeMap, CapabilityPreviewHost, ColorCapability, Density, DesignSystem,
    DesignTokens, GlyphSet, Motion, SelectionChrome, SpacingScale, Theme, theme_for_appearance,
};
