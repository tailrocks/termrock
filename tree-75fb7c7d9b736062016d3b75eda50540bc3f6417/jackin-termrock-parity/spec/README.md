# Spec — jackin-termrock-parity

Contract between `roadmap/jackin-termrock-parity/README.md` (READY,
2026-08-16) and the plans. Plans implement these requirements, never raw item
prose. Ledger: `../coverage.md`.

## Capability index

| File | Ledger IDs | Area |
|------|-----------|------|
| [render-pipeline.md](render-pipeline.md) | F5(engine), B2, D9, N3 | Pure-Rust Buffer→PNG rasterizer, fonts, licensing, determinism |
| [baselines.md](baselines.md) | F5(set), B3, D4, D6, N2 | Committed PNG baseline set for the jackin-used subset |
| [ci-gate.md](ci-gate.md) | F6, W1, B4, D5, N2, N3 | Bless-required workspace-test gate |
| [parity-inventory.md](parity-inventory.md) | F1, F2, F7, D7 | Jackin inventory, API parity map, classification + promotion |
| [comparison-verdicts.md](comparison-verdicts.md) | F3, F4, F8, W2, B1, D1, D3, D8, D10, D12 | Side harness, comparison reports, verdict recording + application |

## Must-not registry

| ID | Statement | Reason | Enforced in plans |
|----|-----------|--------|-------------------|
| N1 | The repo MUST NOT ship any unreviewed visual divergence from the jackin-era look: every difference is restored, merged, or explicitly accepted by a recorded per-component verdict | item §Must not; nothing drifts silently | 004, 006, 008, 009, 010 |
| N2 | Baselines MUST NOT be stored in git-LFS | pointer-only PR diffs defeat the reviewer-sees-image-diff requirement (research ch. 04 §5) | 002, 003 |
| N3 | CI MUST NOT gate on PNG byte equality; the predicate is decoded-pixel equality at zero tolerance | encoder-version churn rewrites bytes without pixel change (research ch. 04 §2) | 001, 003 |

## Deferrals

None. Every `S#`/`F#`/`W#`/`N#`/`B#` ledger ID resolves to a capability file
(no `S#` exist — the item is headless by declaration).

## Notes binding all capability files

- Vocabulary is the item's: **Old rev** (`5ff94ee…`), **Baseline**,
  **Bless**, **Jackin-used subset** (16 widget families + scroll/keymap-hint/
  dialog-shell chrome), **Side harness**. Use these terms exactly.
- Evidence chapters live in `research/tui-png-baselines/` (all vetted
  2026-08-16); cited per requirement as `ch. NN`.
- Recorded cross-surface defect, in scope only as a note: dim is darkened
  0.6× in `frame.rs:184-189` and again 0.7× in `preview-metrics.ts:167-172`
  (ch. 06 §3). The rasterizer follows the Rust-side single 0.6 resolution;
  reconciling the web path is separate cleanup, not this item.
