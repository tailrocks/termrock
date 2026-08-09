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
