# Handoff

Everything a new session needs to continue. Written 2026-09-05; goal session
is stopping here.

## Where things stand

- `main` is the integration point. PR #50 (catalog + docs redesign, squash
  `cb61fc19`) and PR #51 (deploy-title CI fix, `3e906732`) are merged. PR #52
  adds `PROGRESS.md` and this file.
- Repo rules: squash-only merges, direct pushes to `main` rejected, bot review
  threads must be resolved before merging (GraphQL
  `resolveReviewThread` on the thread node id works).
- Required checks: `DCO` + `ci-required`. The `Docs (GitHub)` lane runs the
  full browser suite; its `Verify deployed documentation` job only runs on
  `main` and curls `https://termrock.tailrocks.com` for content markers.
- Playwright `webServer` can time out at 120 s when CI lanes start together.
  That failure mode is a flake: rerun the failed job before diagnosing.

## Runtime architecture (what replaced what)

- The only preview host is `CatalogSession` (crates/termrock-catalog) exposed
  through `crates/termrock-catalog-web` WASM. The old lookbook demo runtime
  and `crates/termrock-lookbook` are deleted.
- Browser contract worth knowing before editing specs:
  - Sessions mount at the CSS host size — 72x20 cells in the default docs
    layout — and the shell floor clamps mounts to 72x20 (`MIN_WIDTH`,
    `MIN_HEIGHT`), so narrow viewports keep 72 columns.
  - On the Buttons page grid, "Run task" spans x 23-34 of row 7; specs target
    cell (26, 7). Pointer specs must derive pixels from
    `data-preview-cols/rows`.
  - Plain Tab/Escape exit interaction mode; Shift+Enter aliases Tab and seeds
    focus; Shift+Escape aliases Escape while staying engaged.
  - Engagement clicks dispatch pointer input too (pointerDown has no
    engagement gate).
  - Outcomes are sticky status-line texts; hover feedback advances the
    semantic revision without recording an outcome.
- Catalog keeps one representative story per component, so the variant
  dropdown never renders; story transitions are cross-page poster loads.

## Deferred work (do not reopen blindly; root causes named)

1. Widget-session host over the termrock widgets (mirroring `CatalogSession`
   over pages) — unlocks the 10 `test.fixme` specs in
   `docs/tests/preview/live-components.spec.ts` and 4 in
   `docs/tests/visual/previews.spec.ts`.
2. `PatternSession` host over `crates/termrock/src/patterns` — unlocks the 7
   `test.fixme` specs in `docs/tests/patterns/live-patterns.spec.ts`.

## Verification commands (run from `docs/`)

```sh
bun run types:check
bun run check:snippets && bun run check:preview-posters && bun run check:preview-metrics
bun run check:contracts && bun run check:components && bun run check:patterns
bun run check:links && bun run check:content
bunx playwright test --workers=2
```

Rust (repo root): `cargo fmt --all --check && cargo clippy --workspace
--all-targets && cargo nextest run --workspace`.

## Workspaces that belong to another session

- `~/Projects/tailrocks/termrock-presentation` —
  `wip/junie-showcase-workingtree-2026-09-05`. Never delete.
- `/private/tmp/termrock-verify` — `wip/junie-showcase-loop-2026-09-05`, peer
  session workspace. It was removed once during cleanup and recreated by the
  peer; leave it alone.
- Peer session `termrock-presentation-f8` was asked not to push to the
  experimental branch; that branch no longer exists — `main` is canonical.

## Conventions

- `git commit -s` (DCO) plus `Co-authored-by: Claude Code
  <noreply@anthropic.com>` trailer.
- Research project: breaking changes preferred, no compatibility shims, no
  deprecation periods. Fix enabling conditions, not symptoms; name deferred
  root causes.
- Pristine-dirty worktree content that was removed during cleanup is backed
  up in `/tmp/termrock-worktree-backups/` (volatile; re-home if needed).
