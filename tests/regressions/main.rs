//! Shared target for regression tests.
//!
//! Add a test as `<topic>.rs` next to this file and one `mod <topic>;` line
//! here. Never add a new top-level `tests/*.rs`: each target changes the
//! generated CI workflows (see `docs/ci.md`).

mod button_row_focus;
#[path = "../common/mod.rs"]
mod common;
mod form_dialog_geometry;
mod form_value;
mod grapheme_paint;
mod loading_label;
mod nav_list_width;
mod smoke;
