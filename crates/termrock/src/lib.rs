//! termrock: domain-neutral TUI kernel for Ratatui.
//!
//! **North star:** de facto base layer for modern Rust TUIs — simple defaults,
//! advanced power, modern APIs only, built **on Ratatui** (preferred session
//! adapter: optional `crossterm` feature). Breaking redesigns are always allowed;
//! see repository `AGENTS.md`.
//!
//! **Architecture:** one paint authority ([`style::DesignSystem`]), one focus/hit
//! authority ([`interaction::InteractionScene`]), one modal authority
//! ([`interaction::OverlayStack`]), per-frame coordination ([`context::UiContext`]),
//! plus [`runtime::run`] for the host loop.
//!
//! Import from modules (`termrock::style::…`, `termrock::widgets::…`). The crate
//! root does **not** re-export types (pre-1.0 Break A / migration 0060).

pub mod ansi_text;
pub mod capability;
pub mod context;
pub mod input;
pub mod interaction;
pub mod keymap;
pub mod layout;
pub mod osc;
pub mod patterns;
pub mod perf;
pub mod registry;
pub mod runtime;
pub mod scroll;
pub mod style;
pub mod text;
pub mod widgets;

#[cfg(feature = "crossterm")]
pub mod crossterm;

#[cfg(test)]
mod root_export_policy {
    /// Break A / migration 0060: crate root must not re-export types.
    #[test]
    fn root_reexports_are_forbidden() {
        let lib = include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/src/lib.rs"));
        for line in lib.lines() {
            let trimmed = line.trim_start();
            if trimmed.starts_with("//") || trimmed.is_empty() {
                continue;
            }
            assert!(
                !trimmed.starts_with("pub use "),
                "crate root must not re-export types (found: {trimmed}). \
                 Import from modules (style::, interaction::, widgets::, …)."
            );
        }
    }
}

#[cfg(test)]
mod focus_authority_policy {
    /// Break C0 / migration 0062: FocusRing must not be public.
    #[test]
    fn focus_ring_is_not_publicly_reexported() {
        let interaction = include_str!(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/src/interaction/mod.rs"
        ));
        assert!(
            !interaction.contains("pub use focus::{FocusOutcome, FocusRing, FocusTarget}"),
            "FocusRing must not be a public re-export (use InteractionScene)"
        );
        assert!(
            interaction.contains("pub use scene::{") && interaction.contains("InteractionScene"),
            "InteractionScene remains public"
        );
        assert!(
            interaction.contains("SemanticScene"),
            "SemanticScene is public frame-local semantic tree (migration 0079)"
        );
        assert!(
            interaction.contains("FocusGraph"),
            "FocusGraph is sole public focus graph (migration 0081)"
        );
    }
}

#[cfg(test)]
mod paint_authority_policy {
    use std::path::Path;

    fn production_source(source: &str) -> &str {
        source.split("#[cfg(test)]").next().unwrap_or(source)
    }

    fn assert_visual_authorities(dir: &Path) {
        for entry in std::fs::read_dir(dir).expect("widget source directory must be readable") {
            let path = entry.expect("widget source entry must be readable").path();
            if path.extension().and_then(|ext| ext.to_str()) != Some("rs") {
                continue;
            }
            if path.file_name().and_then(|name| name.to_str()) == Some("tests.rs") {
                continue;
            }
            let source = std::fs::read_to_string(&path).expect("widget source must be readable");
            let production = production_source(&source);
            assert!(
                !production.contains("bg = None"),
                "widget must not erase recipe backgrounds: {}",
                path.display()
            );
            assert!(
                !production.contains("\"┌\"") && !production.contains("\"╭\""),
                "widget must route box chrome through Surface: {}",
                path.display()
            );
        }
    }

    /// Break B / migration 0061: dual paint types must not re-enter public style API.
    #[test]
    fn dual_paint_types_are_gone() {
        let style_mod = include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/src/style/mod.rs"));
        let tokens = include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/src/style/tokens.rs"));
        let panel = include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/src/widgets/panel.rs"));
        assert!(
            !style_mod.contains("pub struct Theme"),
            "Theme must be RolePalette (Break B)"
        );
        assert!(
            style_mod.contains("pub struct RolePalette"),
            "RolePalette is the palette type"
        );
        assert!(
            !tokens.contains("pub struct DesignTokens"),
            "DesignTokens must be deleted (Break B)"
        );
        assert!(
            tokens.contains("pub struct DesignSystem"),
            "DesignSystem is sole paint authority"
        );
        assert!(
            !panel.contains("pub enum PanelEmphasis"),
            "PanelEmphasis must be PanelChrome only"
        );
    }

    /// Visual-richness series: widgets cannot fork shared fill or shell paint.
    #[test]
    fn widgets_use_shared_visual_authorities() {
        assert_visual_authorities(
            Path::new(env!("CARGO_MANIFEST_DIR"))
                .join("src/widgets")
                .as_path(),
        );
    }
}

#[cfg(test)]
mod overlay_authority_policy {
    #[test]
    fn modal_stack_is_not_publicly_reexported() {
        let interaction = include_str!(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/src/interaction/mod.rs"
        ));
        assert!(
            !interaction
                .lines()
                .any(|l| l.trim_start().starts_with("pub use modal::{") && l.contains("ModalStack")),
            "ModalStack must not be a public re-export (use OverlayStack)"
        );
        assert!(
            interaction.contains("pub use overlay_stack::{")
                && interaction.contains("OverlayStack"),
            "OverlayStack remains public"
        );
        // Dual private modules deleted
        assert!(
            !std::path::Path::new(concat!(
                env!("CARGO_MANIFEST_DIR"),
                "/src/interaction/overlay_controller.rs"
            ))
            .exists()
        );
        assert!(
            !std::path::Path::new(concat!(
                env!("CARGO_MANIFEST_DIR"),
                "/src/interaction/esc_cascade.rs"
            ))
            .exists()
        );
    }

    #[test]
    fn permission_overlay_trap_esc_does_not_peel_grant_path() {
        use crate::interaction::{
            OverlayId, OverlayKind, OverlayOutcome, OverlaySize, OverlaySpec, OverlayStack,
        };
        use ratatui_core::layout::Rect;
        let mut stack = OverlayStack::<&str>::new();
        let bounds = Rect::new(0, 0, 80, 24);
        let _ = stack.open(
            bounds,
            OverlaySpec {
                id: OverlayId::from_static("termrock.permission"),
                kind: OverlayKind::AlertDialog,
                parent: None,
                anchor: None,
                size: OverlaySize::dialog(40, 12),
                opener_focus: Some("prompt"),
                policy: None,
            },
        );
        // Alert traps Esc
        assert!(matches!(stack.handle_escape(), OverlayOutcome::Ignored));
        assert!(stack.contains(&OverlayId::from_static("termrock.permission")));
    }
}
