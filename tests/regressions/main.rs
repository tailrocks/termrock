//! Shared target for regression tests.
//!
//! Add a test as `<topic>.rs` next to this file and one `mod <topic>;` line
//! here. Never add a new top-level `tests/*.rs`: each target changes the
//! generated CI workflows (see `docs/ci.md`).

#[path = "../common/mod.rs"]
mod common;
mod collection_edit_lifecycle;
mod data_shape;
mod document_workspace;
mod focus_edit;
mod form_value;
mod grapheme_paint;
mod grid_parsers;
mod grid_validation;
mod keyed_collections;
mod keyed_costs;
mod response_batches;
mod smoke;
