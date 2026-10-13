# Jackin → TermRock parity and design verification

- **Status**: PLANNED
- **Slug**: jackin-termrock-parity
- **Created**: 2026-08-16 · **Updated**: 2026-08-16
- **Plan**: [plans/jackin-termrock-parity/](../../plans/jackin-termrock-parity/README.md)

## Intent

Upgrade jackin (`/Users/donbeave/Projects/tailrocks/jackin-project/jackin`)
from its old version of termrock to the new version. Analyze all of jackin's
widgets/components to verify all UI reusable widgets in the current termrock
are available to fully replace jackin's with the latest termrock. The current
termrock changed UI/UX of most of those components, so we need to list all
possible components from the jackin project and verify all their design by
comparing the current design and the design from the jackin project.

The goal is to eventually use termrock for all TUI experience for all terminal
applications in the tailrocks ecosystem.

When this ships: every jackin-used widget has a recorded per-component design
verdict applied in termrock, committed phosphor PNG baselines for all its
states rendered by the deterministic pure-Rust pipeline, and a bless-required
CI gate that fails any PR changing a rendered pixel without a committed,
reviewable image diff — making termrock provably ready for jackin's
migration.

## Vocabulary

- **Old rev**: termrock at jackin's pinned revision
  `5ff94ee117fd4a1b72fdd0d1b1847815055a93ac` (`=0.11.0`, 27 migrations) — the
  design-comparison reference. _Avoid_: "old termrock", "legacy version".
- **Baseline**: a committed PNG render of one widget state in the phosphor
  theme that CI compares regenerated renders against. _Avoid_: "golden"
  (already means the lookbook text goldens), "snapshot" (already means insta
  text snaps in jackin).
- **Bless**: committing regenerated PNGs in the same PR that changed the
  rendering, making the image diff reviewable there. _Avoid_: "update
  goldens".
- **Jackin-used subset**: the 16 widget families jackin imports (ActionBar,
  Backdrop, ChoiceDialog, DetailTable, Dialog, DiffView, HintBar, List,
  MessageDialog, Panel, Progress, StatusBar, Tabs, TextInput, Toast,
  Viewport — research ch. 03 Q6) plus the scroll, keymap/hint, and
  dialog-shell chrome those widgets rely on. _Avoid_: "~17 families",
  "jackin widgets".
- **Side harness**: a standalone binary outside the historical tree,
  compiled against old rev `5ff94ee`'s public widget constructors, emitting
  full-fidelity buffers for the comparison pairs. _Avoid_: "old-rev patch".

## Decisions

- 2026-08-16 — **Design conflicts resolved per-component.** When the current
  premium-overhaul rendering conflicts with the old jackin-era (rev `5ff94ee`)
  look, neither side wins wholesale: each widget is compared both ways and the
  user decides per widget which rendering survives. Because a blanket rule in
  either direction would discard deliberate improvements or break the
  jackin-era feel.
- 2026-08-16 — **Termrock-side scope only.** This item ends when termrock is
  proven ready for jackin: parity verified, per-component design decisions
  applied, PNG baseline + CI live. Jackin's own code migration is a separate
  item in the jackin repo. Because the repo boundary keeps ownership and
  tooling clean.
- 2026-08-16 — **Comparison baseline = old-rev per-widget renders.** The
  reference for design comparison is termrock checked out at jackin's pinned
  rev `5ff94ee`, rendering each jackin-used widget in its states, producing
  side-by-side old-vs-current pairs for the per-component verdicts. Because
  isolated per-widget pairs give precise, automatable verdict material.
- 2026-08-16 — **PNG baseline covers the jackin-used subset only.** Snapshot
  only the widgets jackin touches (~17 widget families plus scroll, hint-bar,
  and dialog chrome), in all their states — not the full 136-widget catalog.
  Because the user wants the snapshot footprint small; catalog-wide coverage
  is not a goal of this item.
- 2026-08-16 — **CI gate is bless-required.** CI regenerates affected PNGs on
  every PR; a mismatch against committed baselines fails the job unless the PR
  itself commits the regenerated PNGs, so the reviewer sees the before/after
  image diff in the same PR. Because intentional design changes stay one-PR
  while accidental ones cannot slip through.
- 2026-08-16 — **PNG baselines render the phosphor theme only.** Jackin uses
  the default phosphor theme unmodified (351 `Theme::default()` sites, zero
  custom themes), so one theme keeps the snapshot set small; other presets
  stay guarded by existing design_gate tests. Because footprint follows the
  subset-only coverage decision.
- 2026-08-16 — **Jackin custom components: classify all, promote generic.**
  The parity pass classifies every jackin-owned TUI component per the
  building-block-vs-composite law; generic capability gaps become termrock
  widgets, brand-specific pieces (digital rain, BrandHeader) stay in jackin.
  Because CLAUDE.md law assumes a visual capability belongs in TermRock
  unless provably product-specific.
- 2026-08-16 — **Verdicts via comparison docs + dated decisions.** Subagents
  produce per-widget comparison reports (old-rev vs current renders side by
  side, diffs named) under `roadmap/jackin-termrock-parity/comparisons/`; the
  user reviews in batches and each verdict lands as a dated Decision in this
  item. Because verdicts must be auditable and resumable, never chat-only.
- 2026-08-16 — **PNG pipeline = pure-Rust in-process rasterizer (direction
  A).** Render PNGs from the ratatui buffer/frame JSON with a vendored pinned
  font, swash-class shaping, and tiny-skia-class rasterization (the
  agg-proven composition) — not libghostty, which has no rendering today
  (`research/tui-png-baselines/` ch. 01). Because it is the only direction
  with zero OS-text-stack inputs: strongest determinism, no GPU or browser or
  Zig in CI, and macOS bless matches Linux CI by construction. Revisit only
  if ghostty ships an embeddable CPU rendering pipeline.
- 2026-08-16 — **Old-rev capture via side harness.** Build a small standalone
  harness compiled against old rev `5ff94ee`'s public widget constructors,
  rendering full-fidelity buffers through the chosen pipeline for the
  comparison pairs. Because it gives honest modifier-level fidelity (old-rev
  SVGs are color-only) without touching or maintaining a patch on the
  historical revision.
- 2026-08-16 — **Per-component verdicts are the visual authority.** The
  original blanket must-not ("equal to the old termrock") is reworded: no
  unreviewed visual divergence — every difference from the jackin-era look is
  either restored or explicitly accepted by a recorded verdict. Because a
  literal "equal to old" would contradict the per-component-judgment
  decision; the protected property is that nothing drifts silently.
- 2026-08-16 — **Verdicts merge improvements, not just pick a side.** When
  the current widget version carries genuine improvements over the old
  jackin-era one (hover changes, interaction refinements, new state
  coverage), a verdict restoring the jackin-era design applies those
  improvements on top of it — the jackin-era look is the visual base, merged
  with the current widget's improvements. Pure "old as-is" or "current
  as-is" are the degenerate cases, not the expectation. Because the user
  wants the original design with all benefits of the termrock refactoring,
  not a rollback.

## Capabilities

- Verify all functionality jackin needs exists in the current termrock.
- List all possible components from the jackin project.
- Deeply compare all jackin-used components to their current state in termrock
  and deeply verify their design — for each verification, use subagents.
- Keep the original (jackin-era) design here, but with all benefits of the
  termrock refactoring — current-version improvements (hover changes,
  interaction refinements, new states) are merged onto the jackin-era visual
  base, never discarded by a restore.
- Render each jackin-used reusable component/widget in all of its states as
  PNG baselines stored in the git repository, ~~rendered via libghostty~~
  rendered via the pure-Rust rasterizer pipeline (pinned font + shaping +
  CPU raster; libghostty ruled out — no rendering exists, see Research) so
  they are as real as possible (scope and theme per the 2026-08-16
  decisions).
- On every future PR in this project, CI regenerates the affected widgets'
  renders and verifies their design was changed or not, to confirm that during
  the review process (bless-required semantics per the 2026-08-16 decision).
- Classify every jackin-owned custom TUI component per the
  building-block-vs-composite law and promote generic gaps into termrock
  widgets.
- Produce per-widget comparison reports under
  `roadmap/jackin-termrock-parity/comparisons/` for batch review.

## Screens

Headless by explicit declaration (2026-08-16): this item ships tooling, PNG
baselines, comparison documents, and CI gates — no interactive UI of its own.
Every capability is reachable through the two flows below or exists as a
committed artifact (baselines under the repo, comparison reports under
`roadmap/jackin-termrock-parity/comparisons/`).

## Flows

### PR design-verification flow

1. A PR is created in this project.
2. CI regenerates the PNG renders for affected widgets/components.
3. CI compares them to committed baselines; a mismatch fails the job unless
   the PR commits the regenerated PNGs (bless).
4. The reviewer confirms the before/after image diff on the PR.

Failure points: (a) affected-widget mapping misses a changed file — the
widget's baseline silently stays stale (mitigation: mapping gaps are a gate
failure, not a skip); (b) render non-determinism produces a mismatch with no
visual change — treated as a pipeline bug, never blessed over; (c) a PR
blesses a PNG the reviewer did not actually inspect — process risk carried
by review discipline, named here deliberately.

### Per-component verdict flow

1. Subagents render each jackin-used widget at the old rev (via the side
   harness against `5ff94ee`'s public constructors) and at HEAD, both through
   the pure-Rust rasterizer pipeline.
2. A comparison report per widget (side-by-side pairs, diffs named) lands in
   `roadmap/jackin-termrock-parity/comparisons/`.
3. The user reviews in batches; each verdict becomes a dated Decision here.
4. Each verdict is one of: **merge** (jackin-era visual base + current
   improvements such as hover states applied on top — the expected default),
   **restore** (old look as-is), or **accept** (current look recorded as
   accepted divergence). Merge and restore verdicts become termrock design
   changes.

Failure points: (a) an old-rev state has no story or constructor path — the
side harness renders it from public constructors, and a state that cannot be
reconstructed is reported in the comparison doc as an uncomparable state,
never silently skipped (only 25 of the subset's 87 current stories have
old-rev story counterparts — research ch. 03 Q6); (b) old-vs-new diffs are
dominated by global theme drift rather than widget behavior — comparison
docs must separate palette-level from widget-level differences.

## Data & integrations

- PNG design snapshots of widgets/components, stored in plain git (LFS ruled
  out — pointer-only PR diffs; see Research).
- ~~libghostty — used to render the PNGs so they are as real as possible.~~
  Superseded 2026-08-16: pure-Rust rasterizer pipeline (vendored pinned font,
  swash-class shaping, tiny-skia-class raster); pixel-compare, not
  byte-compare, as the CI predicate.
- CI — regenerates and verifies affected widget renders on each PR.

## References

- `/Users/donbeave/Projects/tailrocks/jackin-project/jackin` — jackin project
  on the old termrock version; source of the original component list and the
  design baseline (colors, visual representation) to preserve.
- Current termrock repository — the target: latest widgets, refactored UI/UX.
- ~~libghostty — terminal rendering engine for realistic PNG snapshots.~~
  Ruled out 2026-08-16: no rendering capability exists in libghostty
  (`research/tui-png-baselines/` ch. 01).

### Looked-up facts (2026-08-16)

- Jackin pins termrock `rev = 5ff94ee117fd4a1b72fdd0d1b1847815055a93ac`
  (`=0.11.0`, features `crossterm, serde`) — jackin `Cargo.toml:118`. That rev
  had 27 migrations; termrock HEAD has 326 (`migrations/`), so jackin is 299
  migrations behind, including the `feat(design)!` premium TUI overhaul
  (`2856f718`).
- Jackin termrock usage: 1067 `termrock::` references across 137 files;
  modules used: `widgets` (Action/ActionBar, Backdrop, ChoiceDialog, Dialog,
  MessageDialog, DetailTable, DiffView, HintSpan/render_hint_bar, List, Panel,
  Progress, StatusBar, Tabs, TextInput, Toast, Viewport), `scroll` (~20 fns,
  DialogScroll/TailScroll), `style` (20 Role variants), `layout`
  (render_dialog_shell ×14), `keymap`, `input`, `interaction`
  (FocusRing/ModalStack/HitRegion/classify_click), `osc`, `text`, `ansi_text`,
  plus 351 `Theme::default()` sites.
- Jackin theme: default phosphor unmodified; zero custom theme literals; only
  3 targeted `with_role` overrides (Role::StatusBar debug chip rows) and one
  Diff role remap (jackin-launch `run.rs:987-1001`). Brand colors live in
  `jackin-brand` (`PHOSPHOR_GREEN(0,255,65)`, rain, cyan, amber), separate
  from the termrock theme.
- Jackin custom widgets (own `Widget` impls): BrandHeader, capsule chrome
  (StatusBarWidget, PaneBorderWidget, BottomChromeWidget), PaneBodyWidget
  (custom cell-grid blit), launch digital-rain, progress rail, prompt dialogs,
  command palette, plus ~40 function-style components across
  console/capsule/launch/tui/oppicker crates.
- Current termrock: 136 public widgets (`docs/scripts/check-catalog.ts:27`),
  36 patterns, 63 theme roles (`style/mod.rs:249`), `DesignSystem` sole paint
  authority (`style/tokens.rs:666`), default `RolePalette::tailrocks_phosphor()`.
- Existing snapshot infra, termrock: lookbook text goldens — 15 flagship
  stories, grapheme-only, no color (`termrock-lookbook/tests/goldens.rs`);
  SVG export with documented fidelity gaps (`svg.rs`,
  `plans/011-lookbook-catalog-truth.md:44-52`); JSON `FrameCell` dumps with
  full fg/bg/attrs, doc-comment says built for a "Ghostty-class web host"
  (`termrock-lookbook/src/frame.rs:5-8`); 1066 registered lookbook stories.
- Existing snapshot infra, jackin: `insta =1.48.0`, 18 plain-text snaps
  (symbol-only, no styles) in console + capsule; 138 `TestBackend` substring
  assertions; no image tooling.
- No PNG, libghostty, vt100, or image-render tooling exists in either repo
  today (Cargo.lock verified). No CI visual-regression job; `docs.yml` only
  self-diffs a render for determinism (`docs.yml:115-119`).

## Research

- [`research/tui-png-baselines/`](../../research/tui-png-baselines/README.md) —
  answers the PNG/libghostty questions: libghostty cannot render PNGs today
  (VT state only); candidate render pipelines A–D with trade-offs; plain git
  over LFS; pixel-compare over byte-compare; old-rev `5ff94ee` builds and
  renders deterministically but with color-only SVG fidelity.

## Must not

- ~~MUST NOT let the overall TUI experience diverge — colors and visual
  representation must be consistent and equal to the old termrock from the
  jackin project.~~ Reworded 2026-08-16 (see Decisions): MUST NOT allow any
  unreviewed visual divergence — every difference from the old jackin-era
  look is either restored or explicitly accepted by a recorded per-component
  verdict; nothing drifts silently.
- MUST NOT store baselines in git-LFS — GitHub shows only the pointer file
  in PR diffs, defeating the reviewer-sees-image-diff requirement.
- MUST NOT gate CI on PNG byte equality — encoder-version churn rewrites
  bytes without pixel changes; the predicate is decoded-pixel equality at
  zero tolerance.

## Quality bar

- Overall TUI experience matches the recorded per-component verdicts: every
  jackin-used widget's rendering is restored to the jackin-era look, merged
  (jackin-era base + current improvements), or carries an explicitly accepted
  divergence — zero unreviewed differences, zero lost improvements.
- PNG snapshots are as real as possible: real font shaping and rasterization
  of the true cell grid (pure-Rust pipeline; libghostty superseded).
- Every jackin-used reusable component/widget is rendered in all of its
  states (scoped by the 2026-08-16 coverage decision).
- CI verification on PRs makes us confident we will never easily break the
  TUI look-and-feel experience.

## Open questions

- ~~Which PNG render pipeline?~~ Decided 2026-08-16: direction A, pure-Rust
  in-process rasterizer (see Decisions).
- ~~Old-rev capture fidelity for comparison pairs?~~ Decided 2026-08-16:
  side harness (see Decisions).

## Open research questions

Answered 2026-08-16 by `research/tui-png-baselines/` (see its README for the
struck originals): git storage → plain git, not LFS; libghostty rendering →
impossible today; FrameCell-feeds-libghostty → moot for rendering, JSON has
named fidelity gaps; byte-determinism → pure-Rust stack + pixel-compare.
Remaining:

- Cross-arch bit-identity of the pure-Rust raster stack (swash/tiny-skia
  class) — empirical two-platform render test at plan time; now load-bearing
  for the chosen direction A.
- ~~resvg SVG→PNG byte-identity across platforms — only if direction D
  chosen.~~ Moot 2026-08-16: direction A chosen.
- `tailrocks/velnor-actions` ci-code.yml extensibility for a PNG job
  (container support, runner fleet OS/arch) — read that repo at plan time.

## Deferred

## Log

- 2026-08-16 — tailrocks-idea — created (DRAFT).
- 2026-08-16 — tailrocks-brainstorm — first touch; fact lookup (jackin + termrock inventories) recorded; SHAPING.
- 2026-08-16 — tailrocks-brainstorm — grilling session: 7 decisions recorded (design authority, scope, baseline, coverage, CI gate, theme scope, promotion, verdict flow); research questions sharpened.
- 2026-08-16 — tailrocks-research — topic `tui-png-baselines` researched and vetted; original research questions answered (libghostty renderer ruled out, plain git, pixel-compare); 2 new decision questions and 3 remaining research questions recorded.
- 2026-08-16 — tailrocks-record-decision — two decisions recorded and propagated: PNG pipeline = pure-Rust rasterizer (direction A); old-rev capture = side harness. libghostty references struck across sections; Open questions cleared.
- 2026-08-16 — tailrocks-finalize — closing interview: must-not reworded to verdict authority (decision), destination sentence added, headless Screens declaration, flow failure points, subset + side-harness vocabulary; readiness checklist passed in full; READY.
- 2026-08-16 — tailrocks-finalize — user refinement during close: verdicts merge current improvements (hover etc.) onto the jackin-era base rather than binary pick; verdict flow, quality bar, capabilities updated; gate re-checked, READY stands.
- 2026-08-16 — tailrocks-plan — package written: coverage ledger, 5-capability spec, 10 plans (all cold-reviewed, findings fixed), research chapters 05–06 added (Q1 closed empirically, Q2 closed), GOAL.md; PLANNED.
