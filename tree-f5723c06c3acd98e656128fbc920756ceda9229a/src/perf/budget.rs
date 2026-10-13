// SPDX-FileCopyrightText: 2026 Alexey Zhokhov
// SPDX-License-Identifier: Apache-2.0

//! Explicit performance budgets for CI-gated hot paths.
//!
//! Budgets are **debug-profile tolerant** (see existing `tree_hot_path` 250 ms /
//! 100 samples). Release numbers are informational until measured on CI class.

use std::time::Duration;

/// Workload class for a surface.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[non_exhaustive]
pub enum PerfClass {
    /// Small chrome; negligible cost expected.
    Chrome,
    /// Typical interactive list/table viewport.
    InteractiveViewport,
    /// Streaming transcript / log follow.
    StreamingSurface,
    /// Large virtualized table/tree/grid.
    VirtualizedLarge,
    /// Overlay open/close / scene rebuild.
    OverlayScene,
    /// Full workbench composite frame.
    WorkbenchFrame,
}

/// What a budget measures.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[non_exhaustive]
pub enum BudgetKind {
    /// Wall time for N warmed paints (debug).
    WarmedPaintBatch {
        /// Samples in the batch.
        samples: u32,
        /// Max total wall time.
        max_total: Duration,
    },
    /// Steady-state allocator calls during warmed paint batch.
    ZeroAllocSteady {
        /// Samples.
        samples: u32,
    },
    /// Max logical rows scanned per paint (must be ≤ viewport + epsilon).
    MaxRowsTouched {
        /// Inclusive max.
        max: u32,
    },
    /// Max scene element registrations per frame for this surface.
    MaxSceneElements {
        /// Inclusive max.
        max: u32,
    },
    /// Idle animation: min period between redraws when only motion ticks.
    IdleRedrawMinPeriod {
        /// Minimum period.
        min: Duration,
    },
}

/// Named budget for one component or kit.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ComponentBudget {
    /// Stable id (e.g. `tree_viewport_10k`).
    pub id: &'static str,
    /// Component or surface name.
    pub component: &'static str,
    /// Class.
    pub class: PerfClass,
    /// Kind + limits.
    pub kind: BudgetKind,
    /// Human note / measurement context.
    pub notes: &'static str,
}

/// Canonical budgets (extend when adding hot_path tests).
#[must_use]
pub fn budgets() -> &'static [ComponentBudget] {
    &BUDGETS
}

/// Lookup by id.
#[must_use]
pub fn budget_for(id: &str) -> Option<&'static ComponentBudget> {
    budgets().iter().find(|b| b.id == id)
}

const BUDGETS: [ComponentBudget; 14] = [
    ComponentBudget {
        id: "tree_viewport_10k",
        component: "Tree",
        class: PerfClass::VirtualizedLarge,
        kind: BudgetKind::WarmedPaintBatch {
            samples: 100,
            max_total: Duration::from_millis(250),
        },
        notes: "40-row viewport over 10k nodes; also zero-alloc steady (tree_hot_path)",
    },
    ComponentBudget {
        id: "tree_viewport_10k_alloc",
        component: "Tree",
        class: PerfClass::VirtualizedLarge,
        kind: BudgetKind::ZeroAllocSteady { samples: 100 },
        notes: "Warmed render loop must not allocate",
    },
    ComponentBudget {
        id: "table_viewport_10k",
        component: "Table",
        class: PerfClass::VirtualizedLarge,
        kind: BudgetKind::WarmedPaintBatch {
            samples: 100,
            max_total: Duration::from_millis(250),
        },
        notes: "table_hot_path",
    },
    ComponentBudget {
        id: "table_viewport_10k_alloc",
        component: "Table",
        class: PerfClass::VirtualizedLarge,
        kind: BudgetKind::ZeroAllocSteady { samples: 100 },
        notes: "warmed table paint must not allocate",
    },
    ComponentBudget {
        id: "log_append_follow",
        component: "LogPane",
        class: PerfClass::StreamingSurface,
        kind: BudgetKind::WarmedPaintBatch {
            samples: 100,
            max_total: Duration::from_millis(700),
        },
        notes: "follow-path paint; log_pane_hot_path; debug 100× over 10k history (raised 300→700ms for GitHub-hosted debug CI headroom)",
    },
    ComponentBudget {
        id: "log_append_follow_alloc",
        component: "LogPane",
        class: PerfClass::StreamingSurface,
        kind: BudgetKind::MaxRowsTouched { max: 64 },
        notes: "allocs per render × samples must stay under 64×samples (viewport-scale)",
    },
    ComponentBudget {
        id: "transcript_10k_blocks",
        component: "Transcript",
        class: PerfClass::StreamingSurface,
        kind: BudgetKind::WarmedPaintBatch {
            samples: 50,
            max_total: Duration::from_millis(300),
        },
        notes: "mixed-height blocks; measure cache hits on steady width",
    },
    ComponentBudget {
        id: "virtual_grid_million_window",
        component: "VirtualGrid",
        class: PerfClass::VirtualizedLarge,
        kind: BudgetKind::MaxRowsTouched { max: 48 },
        notes: "logical 1e6; paint only viewport rows",
    },
    ComponentBudget {
        id: "datatable_million_window",
        component: "DataTable",
        class: PerfClass::VirtualizedLarge,
        kind: BudgetKind::MaxRowsTouched { max: 48 },
        notes: "data_view VirtualWindow; project(start..end) only",
    },
    ComponentBudget {
        id: "scene_register_workbench",
        component: "InteractionScene",
        class: PerfClass::OverlayScene,
        kind: BudgetKind::MaxSceneElements { max: 256 },
        notes: "soft cap per frame for agent workbench; not a hard panic",
    },
    ComponentBudget {
        id: "overlay_open_close",
        component: "OverlayStack",
        class: PerfClass::OverlayScene,
        kind: BudgetKind::WarmedPaintBatch {
            samples: 200,
            max_total: Duration::from_millis(100),
        },
        notes: "open+reflow+dismiss loops without full app redraw cost in isolation",
    },
    ComponentBudget {
        id: "workbench_composite_frame",
        component: "AgentWorkbench",
        class: PerfClass::WorkbenchFrame,
        kind: BudgetKind::WarmedPaintBatch {
            samples: 30,
            max_total: Duration::from_millis(300),
        },
        notes: "rail+transcript+prompt; perceived 10 fps floor under load",
    },
    ComponentBudget {
        id: "idle_motion_cadence",
        component: "Motion",
        class: PerfClass::Chrome,
        kind: BudgetKind::IdleRedrawMinPeriod {
            min: Duration::from_millis(33),
        },
        notes: "Full motion ≤ ~30 Hz; Reduced slower; Off = no idle redraw",
    },
    ComponentBudget {
        id: "stream_coalesce_batch",
        component: "StreamCoalescer",
        class: PerfClass::StreamingSurface,
        kind: BudgetKind::WarmedPaintBatch {
            samples: 1_000,
            max_total: Duration::from_millis(50),
        },
        notes: "coalesce 1000 token deltas into batches; CPU only",
    },
];

/// Assert a wall-time batch budget (used by hot_path tests).
///
/// Returns `Err` with a message suitable for `assert!` / CI logs.
pub fn check_batch_budget(budget_id: &str, samples: u32, elapsed: Duration) -> Result<(), String> {
    let Some(b) = budget_for(budget_id) else {
        return Err(format!("unknown budget id {budget_id}"));
    };
    let BudgetKind::WarmedPaintBatch {
        samples: expected,
        max_total,
    } = b.kind
    else {
        return Err(format!("{budget_id} is not a WarmedPaintBatch budget"));
    };
    if samples != expected {
        return Err(format!(
            "{budget_id}: sample count {samples} != expected {expected}"
        ));
    }
    if elapsed > max_total {
        return Err(format!(
            "{budget_id}: {elapsed:?} exceeds max {max_total:?} ({}). \
             Re-measure on CI-class hardware before raising the budget.",
            b.notes
        ));
    }
    Ok(())
}

/// Assert zero allocations in a steady region (caller uses stats_alloc Region).
pub fn check_zero_alloc_steady(
    budget_id: &str,
    allocations: usize,
    reallocations: usize,
) -> Result<(), String> {
    let Some(b) = budget_for(budget_id) else {
        return Err(format!("unknown budget id {budget_id}"));
    };
    if !matches!(b.kind, BudgetKind::ZeroAllocSteady { .. }) {
        return Err(format!("{budget_id} is not a ZeroAllocSteady budget"));
    }
    if allocations != 0 || reallocations != 0 {
        return Err(format!(
            "{budget_id}: steady paint allocated (alloc={allocations}, realloc={reallocations})"
        ));
    }
    Ok(())
}

/// Assert max rows (or alloc units) touched/spent per paint batch.
///
/// Used for virtualization (`rows_touched ≤ viewport`) and for log-style
/// alloc-per-render caps stored as [`BudgetKind::MaxRowsTouched`].
pub fn check_max_rows_touched(budget_id: &str, touched: u32) -> Result<(), String> {
    let Some(b) = budget_for(budget_id) else {
        return Err(format!("unknown budget id {budget_id}"));
    };
    let BudgetKind::MaxRowsTouched { max } = b.kind else {
        return Err(format!("{budget_id} is not a MaxRowsTouched budget"));
    };
    if touched > max {
        return Err(format!(
            "{budget_id}: touched {touched} exceeds max {max} ({}).",
            b.notes
        ));
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn budgets_are_unique_and_tree_matches_hot_path() {
        let mut ids = std::collections::BTreeSet::new();
        for b in budgets() {
            assert!(ids.insert(b.id), "duplicate budget id {}", b.id);
        }
        let tree = budget_for("tree_viewport_10k").unwrap();
        match tree.kind {
            BudgetKind::WarmedPaintBatch { samples, max_total } => {
                assert_eq!(samples, 100);
                assert_eq!(max_total, Duration::from_millis(250));
            }
            _ => panic!("unexpected kind"),
        }
    }

    #[test]
    fn check_batch_budget_accepts_under_limit() {
        assert!(check_batch_budget("tree_viewport_10k", 100, Duration::from_millis(50)).is_ok());
        assert!(check_batch_budget("tree_viewport_10k", 100, Duration::from_millis(251)).is_err());
    }

    #[test]
    fn zero_alloc_check() {
        assert!(check_zero_alloc_steady("tree_viewport_10k_alloc", 0, 0).is_ok());
        assert!(check_zero_alloc_steady("tree_viewport_10k_alloc", 1, 0).is_err());
    }

    #[test]
    fn max_rows_touched_check() {
        assert!(check_max_rows_touched("datatable_million_window", 40).is_ok());
        assert!(check_max_rows_touched("datatable_million_window", 49).is_err());
    }

    #[test]
    fn stream_coalesce_budget_passes() {
        use crate::perf::{StreamCoalescer, UpdatePriority};
        use crate::runtime::FrameTick;
        use std::time::Instant;

        let mut c = StreamCoalescer::new().with_limits(64 * 1024, 256, Duration::from_millis(0));
        let started = Instant::now();
        let now = Instant::now();
        for i in 0..1_000 {
            c.push_text("x", UpdatePriority::Normal);
            if i % 10 == 0 {
                let _ = c.take_for_frame(FrameTick::manual(
                    now,
                    Duration::from_millis(i as u64),
                    Duration::from_millis(1),
                ));
            }
        }
        let _ = c.take_now();
        check_batch_budget("stream_coalesce_batch", 1_000, started.elapsed())
            .expect("coalesce batch budget");
    }
}
