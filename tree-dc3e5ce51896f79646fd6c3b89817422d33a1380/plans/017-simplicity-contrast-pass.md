# Plan 017: The designer pass — information budgets and a contrast floor

> **Executor instructions**: Follow this plan step by step. Read
> `docs/design/web-premium-tui-law.md` §2.1 (radical calm), P1/P6/P8,
> rules 11/15 before starting. Run every verification command. STOP
> conditions binding. Update `plans/README.md` when done.
>
> **Drift check (run first)**: plans 002/007 DONE (ladder values + status
> quieting land first; this plan measures and prunes on top of them).
> Re-locate cited sites with `rg`.

## Status

- **Priority**: P1 (operator directive 2026-08-14: "too much information …
  simplicity and color contrast" — designer look-and-feel first)
- **Effort**: L
- **Risk**: MED (removes default-visible information — migration documents every removal)
- **Depends on**: plans/002, plans/007; runs alongside 010/013 (their sweeps
  respect the budgets set here)
- **Category**: design
- **Planned at**: commit `d09bd2fe`, 2026-08-14

## Why this matters

Two designer-visible defect classes remain unowned by 001-016:

1. **Information overload.** Patterns show everything at once: an
   `agent_status_header` packs 8 segments in 5 hues into one permanent row;
   `session_picker`'s preview is 11 lines in 5 hues; the permission prompt
   is ~12 unaligned meta lines; `connection_manager` rows carry
   name+target+env+protocol+status in one line. Premium calm = few things,
   quiet, with everything else one keypress away (law §2.1 "radical calm",
   P6 hover-revealed, P8 contextual collapse, rule 11 disclosure).
2. **No contrast floor.** Contrast bugs keep appearing case-by-case
   (TextDisabled 2.43:1 on Canvas; ladder steps at 1.07:1; black-on-tint
   focused overflow label; Border-colored badge text) because nothing
   enforces a minimum. One computed floor + one test ends the class.

## Part A — Contrast floor (structural fix first)

### Step A1: Relative-luminance contrast in `style/`

Add `fn contrast_ratio(a: Rgb, b: Rgb) -> f32` (WCAG relative luminance) in
`style/palette.rs` (test-usable, `pub(crate)`).

### Step A2: The floor table, tested per preset

| Pair (fg on bg) | Floor |
|---|---|
| `Text` on Canvas/Surface/Raised/Elevated/Sunken | ≥ 7.0 |
| `TextStrong` on same | ≥ 7.0 |
| `TextMuted` on Canvas/Surface | ≥ 4.5 |
| `TextFaint` on Canvas/Surface | ≥ 3.0 |
| `TextDisabled` on Canvas/Surface | ≥ 2.5 AND ratio distinct from TextFaint by ≥ 0.4 |
| `Danger/Warning/Info/Success` fg on Canvas/Surface and on their tint bgs | ≥ 4.5 |
| `Text`/`TextStrong` on `SelectionTint`/`HoverTint` | ≥ 4.5 |
| Any recipe label fg on its recipe fill bg (button/chip/action) | ≥ 4.5 |
| `Border` vs its surface | ≥ 1.3 (visible hairline) |
| Adjacent ladder surfaces (Canvas→Surface→Raised→Elevated) | pairwise ≥ 1.15 |

Test `contrast_floor_holds` runs the table against phosphor, slate, paper,
high_contrast (HC floors: all text ≥ 7.0). Fix every violation by adjusting
the palette VALUE (not the consumer) — plan 002's targets are the starting
point; where a 002 target itself fails (e.g. `TextFaint #4a574a` on Surface
`#121612` ≈ 2.9 — measure), tune the value and update 002's pin tests in the
same commit.

### Step A3: Recipe-level sweep

Add `recipe_pairs_pass_floor` test: resolve button/input/list-row/chip/panel
recipes in all states under phosphor; every (label fg, effective bg) pair
passes its row above. This mechanically catches the
`button_group`-class bugs (fg patched over an unrelated bg).

**Verify**: `cargo nextest run -p termrock style:: contrast` → green across
presets; `mise run check` green.

## Part B — Information budget (the simplification law)

### Step B1: Write the budget into the law doc

Append to `docs/design/web-premium-tui-law.md` §4 (one rule, same commit as
code): **"Default-frame information budget: ≤3 content zones visible; ≤1
hint row; metadata ≤⅓ of visible rows; every further detail is one
keypress away (focus/expand/overlay) and its affordance is visible.
Removing information from the default frame requires providing the
keypress path in the same commit."**

### Step B2: Per-surface reductions (apply the budget)

Default-frame content diets — detail moves behind focus/expand/overlay,
never deleted:

| Surface | Default frame after | Behind one keypress |
|---|---|---|
| `agent_status_header` | work-state glyph+verb, model, ONE actionable (queue) — ≤3 segments, ≤1 hue | full segment sheet as overlay/expando (`s`) |
| `session_picker` preview | 5 quiet lines: title, relative time, model, status glyph+word, one-line summary | full metadata sheet (`i`) |
| `permission.rs` prompt | 4-line KV core: actor · operation · target · risk (+ decision list) | provenance/expectations/prior grants (`d details`, expands in place) |
| `connection_manager` rows | name + env chip + status glyph | target/protocol/latency in the detail pane (already exists) |
| `metrics_dashboard` tile | title, value+unit, delta glyph, spark | thresholds/history on tile focus |
| `plan_review` | ONE pane bright; marks column faint; counts into `PanelTitleSpec` | other panes quiet until focused |
| `database_workbench` / `observability_dashboard` | ≤3 panes default (nav + main + status) | inspector/logs as tabs/drawer |
| `help_center` / `integration_status` lists | first N rows + `+12 more` faint | full list on expand |
| `setup_wizard` summary | only user-changed values | full config dump behind `a all` |
| ALL patterns | ONE footer hint row (HintBar, ≤5 hints); counts/filters live in `PanelTitleSpec`, not body rows; timestamps relative + `TextFaint`; ids/hashes middle-truncated | KeyboardHelp overlay for the full map |

Each row = one commit-sized edit; each removal listed in the migration with
its keypress path.

### Step B3: Enforce what's testable

`design_gate.rs` additions:
- `pattern_hint_budget`: each rendering pattern's default frame contains ≤1
  hint row (scan for HintBar paints / bottom-row hint content).
- `pattern_style_diversity`: default frame of each priority pattern renders
  ≤8 distinct fg colors (buffer scan) — the "too much information" proxy
  that catches hue soup regressions.
- Extend `accent_budget` (plan 007) to the four workbench patterns.

**Verify**: gates green; before/after row-count + distinct-color metrics for
the 10 surfaces above recorded in the migration file (honest deltas).

### Step B4: Migration

`migrations/` next free + `MIGRATING.md`: every default-frame removal, its
keypress path, contrast value changes with old→new ratios.

## Done criteria

- [ ] `mise run gate` exits 0.
- [ ] `contrast_floor_holds` + `recipe_pairs_pass_floor` green on 4 presets.
- [ ] The 10 surface diets applied; each removal has a visible affordance.
- [ ] Law doc carries the information-budget rule.
- [ ] Gates (`pattern_hint_budget`, `pattern_style_diversity`) green.
- [ ] Migration + `MIGRATING.md`; README row updated.

## STOP conditions

- A floor can't be met without leaving the phosphor family (hue shift) —
  report the pair + candidate values; palette identity is a design call.
- A content diet removes something a pattern's outcome contract needs
  visible (e.g. permission risk) — the budget bends, the safety info stays;
  report the exception for the law doc.
- `pattern_style_diversity` can't reach ≤8 on a chart-heavy surface —
  charts are exempt (series colors are data); encode the exemption, don't
  raise the global budget.

## Maintenance notes

- The floor table is the permanent regression barrier for "colors contrast";
  new roles/pairs must be added to it in the same commit.
- New patterns must state their default-frame budget in the `//! Teaches:`
  header (plan 016 charter gains one line — coordinate).
