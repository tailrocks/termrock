// SPDX-FileCopyrightText: 2026 Alexey Zhokhov
// SPDX-License-Identifier: Apache-2.0

//! **termrock-showcase** — the flagship workbench.
//!
//! A real terminal application built out of public TermRock only: transcript,
//! composer, trust gate, plan and diff review, task rail, status bar. The
//! agent behind it is scripted, so the demo is deterministic and needs no
//! provider, no network and no shell.
//!
//! Run it:
//!
//! ```sh
//! cargo run -p termrock-showcase
//! ```
//!
//! `Enter` submits, `^n` switches scenario, `Esc` peels one layer, `^q` quits.
//!
//! The application lives in a library so the scene gates in `tests/` exercise
//! the same code the binary runs, rather than a second copy of it.

pub mod app;
pub mod demo_runtime;
pub mod model;
