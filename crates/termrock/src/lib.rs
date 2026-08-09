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
pub mod runtime;
pub mod scroll;
pub mod style;
pub mod text;
pub mod widgets;

#[cfg(feature = "crossterm")]
pub mod crossterm;

pub use interaction::{
    InteractionElement, InteractionLayer, InteractionOutcome, InteractionScene, LayerDismissPolicy,
    LayerKind, NavigationMove, PageMove, SceneError, SemanticElement, SemanticRole, SemanticScene,
    UiIntent, default_list_intent, dispatch_keymap_action,
};
pub use style::{
    Appearance, AppearanceThemeMap, ColorCapability, Density, DesignSystem, DesignTokens, GlyphSet,
    Motion, SelectionChrome, SpacingScale, Theme, theme_for_appearance,
};
