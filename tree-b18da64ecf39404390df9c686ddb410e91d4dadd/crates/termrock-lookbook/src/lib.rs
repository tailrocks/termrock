// SPDX-FileCopyrightText: 2026 Alexey Zhokhov
// SPDX-License-Identifier: Apache-2.0

//! Shared, backend-neutral TermRock demo catalog and preview runtime.
//!
//! Native Lookbook and the documentation WASM host mount the same stories from
//! this crate. Host adapters own terminal/browser I/O; demo state and paint do
//! not depend on crossterm or the DOM.

pub mod demo;
pub mod frame;
pub mod interactors;
pub mod knobs;
pub mod stories;
