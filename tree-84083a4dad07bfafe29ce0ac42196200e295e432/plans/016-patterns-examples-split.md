# Plan 016: Patterns become true examples — promote the buried primitives, then every composite only composes

> **Executor instructions**: Follow this plan step by step. Read
> `docs/design/building-block-vs-example-composite.md` (the boundary law),
> `docs/design/termrock-component-audit-2026-08.md` §2 Cluster I + §5, and
> `docs/design/tui-app-deep-analysis.md` §17 before starting. Run every
> verification command. STOP conditions binding. Update `plans/README.md`.
>
> **Drift check (run first)**: plans 004-009 DONE (this plan supplies
> primitives that plans 010/013 then consume — check README order). Re-locate
> every cited site with `rg`.

## Status

- **Priority**: P1 (the patterns-refactor goal's structural core)
- **Effort**: L
- **Risk**: MED (public pattern surfaces move — breaking is fine, migration mandatory)
- **Depends on**: plans/004-009; plans/010 and /013 consume this plan's primitives (see README order)
- **Category**: tech-debt / design / architecture
- **Planned at**: commit `d09bd2fe`, 2026-08-14

## Why this matters

The goal: **split components (widgets) from examples (how to use them)** —
today `patterns/` is neither. Its 36 files hand-roll paint that belongs in
widgets (4 identical private `Panelish` structs; bespoke tile chrome;
hand-stacked forms; 3 copies of a destructive-confirm overpaint; hand-built
status-segment rows), while 4 "patterns" ship geometry with no paint at all
— so the design language has no enforcement point and every consumer
re-invents chrome. An example that hand-rolls paint is not an example: it
teaches the wrong thing. After this plan: **widgets own every reusable
capability; each pattern is a short, beautiful, copyable recipe that only
composes public widgets** — and gates keep it that way.

## Decisions (recommendations applied; operator may overturn per step)

| ID | Decision | Recommendation |
|----|----------|----------------|
| E1 | Rename `termrock::patterns` → `termrock::examples`? | **Keep the path** (`patterns` is anchored in AGENTS.md law, registry provenance, docs); recharter the module docs as "copyable examples". Revisit a dedicated examples crate when registry distribution lands (law already permits it). |
| E2 | Consolidate overlapping dashboards? | Keep all named examples (they demo different assemblies); `app_dashboard` embeds `MetricsDashboard` (plan 010 already wires it) — no deletions. |
| E3 | Promotion homes | All promoted primitives are product-neutral → `widgets/` per the boundary checklist; none carry product nouns. |

## Step 1: Promote the buried primitives to `widgets/`

Run the boundary checklist (`building-block-vs-example-composite.md`) for
each; all pass as generic building blocks:

| New/target widget | Source evidence | API sketch |
|---|---|---|
| `Panel` inner-owns-chrome mode (kills `Panelish`) | 4 identical private structs: `error_recovery.rs:1283`, `help_center.rs:1595`, `project_launcher.rs:1501`, `file_manager.rs:1700`; double borders GAP-08 | `Panel::frameless_title(bool)` — title row + surface fill, no border, for wrapping widgets that draw their own chrome. Delete all four `Panelish` copies. |
| `PanelTitleSpec` (k9s title composition) | `tui-app-deep-analysis.md` §4/§17-8; today titles are ad-hoc strings | `Panel::title_spec(PanelTitleSpec { name, scope: Option, count: Option, filter: Option, live: Option })` → `Name(scope)[count] /filter` with per-segment roles (name strong, scope muted, count faint, filter accent-when-active). |
| `ConfirmPrompt` (destructive confirm) | 3 overpaint copies: `git_workbench.rs:1405-1438`, `prompt_queue.rs:905-936`, `session_picker.rs:1538-1600` (GAP-09); plan 010's helper graduates | Thin `AlertDialog` preset: message + Cancel/Confirm, DangerChrome Quiet (plan 009), focused button distinct, hit-region slots. Neutral trust chrome = widgets per the law's `PermissionPrompt` precedent. |
| `MetricTile` | `metrics_dashboard.rs:264-289,1358-1447` bespoke tiles; richness plan §6 names it | `MetricTile { title, value, unit, delta: Option<Delta>, spark: Option<&[f64]>, health }` — value `TextStrong`+BOLD, unit `TextMuted`, delta `▲/▼` glyph-before-color, spark `ChartSeries1`, health on the status letter only. |
| `StatusStrip` (segment row) | `agent_status_header.rs:960-1042` 5-hue rainbow + wrong drop order (GAP-13/22) | `StatusStrip { segments: Vec<Segment { text, role, priority }> }` — priority-sorted narrow dropping, separators `TextFaint`, ≤1 status hue + ≤1 accent enforced in-recipe. |
| Inline chrome row (filter/rename/confirm) | `file_tree.rs:1098-1126`, `tree_navigation.rs:908-918`, pickers (plan 007 helper graduates) | `ChromeRow { prefix_glyph, body, tone }` — muted body, glyph-carried tone, sunken option for query wells. |

Each promotion: implement in `widgets/`, export, story + contract entry per
repo law, consumers switched in the same commit (the 4 `Panelish` files, the
3 confirm sites, metrics tiles, agent_status_header).

**Verify**: `rg -n "struct Panelish" crates/` → 0;
`rg -n "MetricTile|StatusStrip|ConfirmPrompt|title_spec" crates/termrock/src/widgets/mod.rs` shows exports;
`mise run check` green.

## Step 2: Reference paints for the four geometry-only recipes (closes GAP-30)

`ops_dashboard`, `resource_browser`, `studio_shell`, `agent_shell` gain
`render_*` reference implementations composing public widgets over their
existing `layout_*` slots (Panel + StatusStrip + MetricTile + List/Tree +
StatusBar). Geometry-only entry points remain for hosts that paint
themselves; docs state both paths. Also fix `ops_dashboard.rs:196-217` Tab
cycling into no-op regions if plan 013 hasn't.

**Verify**: each of the four has a paint test rendering non-empty, budgeted
chrome (≤1 focused border, ≤2 accent regions).

## Step 3: The example charter — recharter and enforce

1. Rewrite `patterns/mod.rs` module docs as the **Example charter**: an
   example (a) composes public widgets only, (b) owns domain state +
   wording, (c) contains ZERO raw buffer paint (`set_stringn`/`cell_mut`)
   and ZERO `Role::` styling beyond mapping domain state → widget inputs,
   (d) teaches one assembly, named in its header doc.
2. Every pattern file gets a header doc block: *what this example teaches*,
   *widgets composed* (explicit list), *copy-adapt notes*.
3. Composition floor per composite (audit F8 + Cluster I): forms via
   `Form`/`FieldRow`; every shippable action = one `Button{Primary}`
   (Enter-triggered, omitted-not-greyed) — SetupWizard Continue/Finish,
   Settings Apply, AuthEntry Sign in are chord-only today
   (`form_wizard.rs:1145-1180`, `settings_screen.rs:472-478`,
   `auth_entry.rs:534-536`); footers = `StatusBar` + kbd chips via
   `HintBar`; empties via `EmptyState`; confirms via `ConfirmPrompt`;
   titles via `PanelTitleSpec`.
4. Sweep every pattern to the charter. Plans 010/013 already fixed paint
   defects; this step removes the remaining raw-paint glue by swapping in Step 1's
   primitives, then deletes now-dead local helpers.

**Verify**: `rg -n "set_stringn|cell_mut" crates/termrock/src/patterns/` → 0
matches; `rg -c "//! Teaches:" crates/termrock/src/patterns/*.rs` — every
rendering pattern has the header.

## Step 4: Gates — keep the split permanent

Extend `design_gate.rs`:
- `patterns_only_compose`: source scan — `patterns/` contains no
  `set_stringn`, no `cell_mut`, no `Modifier::` literals, no `Role::`
  outside `role()`-mapping fns (regex heuristic; whitelist file for
  documented exceptions, target empty).
- `patterns_have_charter_docs`: every `patterns/*.rs` with a render fn has
  the `//! Teaches:` header.
- `widgets_never_import_patterns` (law §6): scan `widgets/` for
  `crate::patterns` → 0.

**Verify**: gates green; corrupting one pattern with a `set_stringn` makes
the gate fail locally.

## Step 5: Docs, registry, lookbook alignment

- `docs/design/building-block-vs-example-composite.md`: add the promoted
  primitives to the positive-examples table; add the "zero raw paint"
  clause to the decision checklist.
- Registry/catalog provenance rows for composites still point at
  `patterns/…`; lookbook stories import blocks from widgets, composites
  from patterns (verify, fix drift).
- `AGENTS.md` building-block table: add `MetricTile`/`StatusStrip`/
  `ConfirmPrompt`/`ChromeRow` rows (building blocks) — same commit as the
  code per repo law.

**Verify**: `mise run contracts` + `mise run gate` green.

## Step 6: Migration

`migrations/` next free number + `MIGRATING.md`: promoted widget APIs, the
four `Panelish` deletions, pattern surface changes (new Buttons, footer
slots), the charter (documented consumer contract: patterns are copy-adapt
examples, pin-and-fork encouraged).

## Done criteria

- [ ] `mise run gate` exits 0 (incl. new gates).
- [ ] Zero raw paint in `patterns/`; zero `Panelish`.
- [ ] Six promotions exported from `widgets/` with stories + contracts.
- [ ] Four geometry-only recipes have reference paints.
- [ ] Charter docs on module + every rendering pattern.
- [ ] Composites ship real primary Buttons.
- [ ] Migration + `MIGRATING.md`; README row updated.

## STOP conditions

- A pattern's paint cannot be expressed through widgets without a NEW
  primitive not listed in Step 1 — report the gap (that's a widgets-plan
  item, not license to keep raw paint).
- `PanelTitleSpec` collides with plan 004's `PanelRecipe.title_prefix` —
  reconcile in favor of one title pipeline; report the merge.
- Registry provenance breaks (`mise run contracts` red) after moves —
  report rather than hand-editing generated files.

## Maintenance notes

- New pattern PRs must pass `patterns_only_compose` — reviewers reject raw
  paint on sight; if an example needs paint, the missing widget is the
  finding.
- Direction follow-up (recorded, not planned): a `WhichKey` overlay widget
  (yazi/helix steal, `tui-app-deep-analysis.md` §17-4) would let help/hint
  examples compose it; today `KeyboardHelp` approximates it.
