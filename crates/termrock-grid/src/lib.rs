//! The table and the editable grid, with their changesets and submissions.
//!
//! `termrock::widgets::{grid, table}` are the modules of the same name here; the
//! `junie-tui` package re-exports each at that path. `table` and `grid` call each
//! other, so they live in one crate.

#[cfg(test)]
mod collection_edit_tests;
pub mod grid;
#[cfg(test)]
mod grid_sort_tests;
mod grid_specimen;
#[cfg(test)]
mod grid_undo_tests;
pub mod table;
mod table_specimen;
