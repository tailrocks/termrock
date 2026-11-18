//! Proof tooling of the Termrock library: the declared-API catalog, the
//! generated documentation tables and the source gate of #14.
//!
//! `catalog` and `docs` are the public tooling surfaces; the facade re-exports
//! them at `termrock::{catalog, docs}`. `catalog_contract` and its helpers are
//! test code: they walk the source of every crate and never ship.

pub mod catalog;
pub mod docs;

#[cfg(test)]
mod catalog_contract;
#[cfg(test)]
mod inventory;
#[cfg(test)]
mod scenario_reach;
