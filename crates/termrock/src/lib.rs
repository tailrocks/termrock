//! termrock: domain-neutral TUI kernel for Ratatui.
//!
//! **Architecture:** one paint authority ([`style::DesignSystem`]), one focus/hit
//! authority ([`interaction::InteractionScene`]), one modal authority
//! ([`interaction::OverlayStack`]), plus [`runtime::run`] for the host loop.
//!
//! Import from modules (`termrock::style::…`, `termrock::widgets::…`). The crate
//! root does **not** re-export types (pre-1.0 Break A / migration 0060).

pub mod ansi_text;
pub mod capability;
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
mod paint_authority_policy {
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
}
