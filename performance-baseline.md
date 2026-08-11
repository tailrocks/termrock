# Performance baseline

> **Authoritative budgets (0.12.x):** `termrock::perf::budgets()` and
> [`docs/design/streaming-performance.md`](docs/design/streaming-performance.md).
>
> **CI hot paths (fail on regression):**
> - `tree_hot_path` → `tree_viewport_10k` + `tree_viewport_10k_alloc`
> - `table_hot_path` → `table_viewport_10k` + `table_viewport_10k_alloc`
> - `log_pane_hot_path` → `log_append_follow` + `log_append_follow_alloc`
> - `perf` unit → `stream_coalesce_batch`, `datatable_million_window` checks
>
> **Staleness note (2026-07-16 table below):** historical v0.6 donor comparison only.

Measured on 2026-07-15 on Linux aarch64 after jackin❯ parity, using Rust 1.97.0. These budgets compare against the frozen donor baseline; they guard behavior as well as speed.

| Measurement | Donor | v0.6.0 candidate | Verdict |
|---|---:|---:|---|
| clean default build | 2.276 s | 1.424 s | pass; dependency reduction, with render/interaction tests unchanged |
| clean all-feature build | 2.311 s | 1.298 s | pass; feature additivity remains tested |
| catalog render | 0.266 s | 0.098 s | pass; deterministic public-widget catalog |
| catalog size | 485,885 bytes / 29 donor compositions | 105,736 bytes / 12 neutral public-widget stories | pass; product stories intentionally remain with the consumer |
| first interactive frame observation | 0.517 s | 0.515 s | pass |

The candidate is faster because it excludes product crates, product stories, and default Crossterm coupling. Corrected Unicode wrapping and focused double-border glyphs are the only intended post-parity rendering changes; stable-ID, clipping, selection, focus, key-dispatch, and tiny-area tests pass. The terminal session is covered by partial-initialization, explicit restore, and drop-fallback tests rather than accepting the donor timeout transcript as a restoration guarantee.

Budgets for the `0.6.x` line: no more than 20% regression in clean build, catalog render, or first-frame wall time on equivalent hardware without an evidence-backed explanation; no interaction or rendering contract may regress in exchange for speed.
