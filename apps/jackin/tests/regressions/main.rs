//! Shared target for regression tests.
//!
//! Add a test as `<topic>.rs` next to this file and one `mod <topic>;` line
//! here. Never add a new top-level `tests/*.rs`: each target changes the
//! generated CI workflows (see `docs/ci.md`).

#![cfg(feature = "demo")]

#[path = "../common/mod.rs"]
mod common;
mod footer_wiring;
mod smoke;
mod tiny_terminals;
