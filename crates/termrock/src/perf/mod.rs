// SPDX-FileCopyrightText: 2026 Alexey Zhokhov
// SPDX-License-Identifier: Apache-2.0

//! High-frequency, streaming, and large-data performance kits.
//!
//! TermRock optimizes for **perceived** jank: frame time under load, allocation
//! on steady paint, and O(viewport) work — not micro-opts on cold paths.
//!
//! See `docs/design/streaming-performance.md`.
mod budget;
mod follow;
mod stream;

pub use budget::{
    BudgetKind, ComponentBudget, PerfClass, budget_for, budgets, check_batch_budget,
    check_max_rows_touched, check_zero_alloc_steady,
};
pub use follow::{
    FollowMode, NewContentIndicator, ScrollAnchor, ScrollAnchorKind, apply_follow_after_append,
    pause_follow_on_user_scroll,
};
pub use stream::{BackpressureSignal, DirtyFlags, StreamBatch, StreamCoalescer, UpdatePriority};
