# Progress

Snapshot: 2026-09-05, branch `main` at `3e906732`.

## Merged to main

- PR #50 — component catalog and documentation redesign. Squash-merged as
  `cb61fc19` (repo only allows squash). The branch history
  (`experimental/component-catalog-docs-2026-09-02`) was deleted after the tree
  was verified identical to `main`.
- PR #51 — deploy verification accepts the redesigned landing title
  (`Terminal UI components for Rust — TermRock`); merged as `3e906732`.

## Runtime state

- Unified catalog runtime (`CatalogSession` over WASM) is the only preview
  host; the old lookbook runtime is deleted.
- Browser suites green: 41 passed, 21 `test.fixme`, 0 failed. Rust: 3534
  nextest tests, clippy and fmt clean.
- Docs checks green on main after PR #51 fixed the stale title check.

## Deferred work (named root causes)

- Widget-level demo lifecycles (text capture, sliders, split drags, tree
  collapse, toast expiry, alert/drawer/fullscreen triggers, key-value filter,
  permission decisions) need a widget-session host mirroring `CatalogSession`.
  All specs for them are `test.fixme` with the reason inline.
- Pattern pages need a `PatternSession` host over
  `crates/termrock/src/patterns`; the 7 pattern specs are `test.fixme`.
- Accent-rail passivity was a classification bug (Widget kind = paint
  contract); `stack/vertical` is the passive example now.

## Cleanup done

- All temp worktrees removed; dirty content backed up to
  `/tmp/termrock-worktree-backups/`.
- Merged local and remote branches deleted; `git fetch --prune` done.

## Active elsewhere (do not delete)

- `~/Projects/tailrocks/termrock-presentation` on
  `wip/junie-showcase-workingtree-2026-09-05`.
- `/private/tmp/termrock-verify` on `wip/junie-showcase-loop-2026-09-05`
  (peer session workspace).
