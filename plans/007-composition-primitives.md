# Plan 007: Composition primitives — FieldRow, AccentRail, TreeList, PanelStack, HintBar v2, DetailTable::measure

> **Executor instructions**: Follow step by step; verify each step; STOP
> conditions are binding. Update `plans/README.md` when done.
>
> **Drift check (run first)**: `git diff --stat 539e7d03..HEAD -- crates/termrock/src/widgets crates/termrock/src/layout`
> Heavy churn from plans 001–006 is expected; verify the *reference* APIs
> this plan builds on still exist (`Surface`, `resolve_list_row`,
> `ListRow` composed slots, motion kit from plan 006). On structural
> mismatch, STOP.

## Status

- **Priority**: P2
- **Effort**: L
- **Risk**: LOW–MED (new public API; minimal changes to existing widgets)
- **Depends on**: plans/001, 003, 006
- **Category**: tech-debt (missing building blocks)
- **Planned at**: commit `539e7d03`, 2026-08-12

## Why this matters

The primary consumer (Jackin) documents, in its own source comments, six
places where it rebuilt library-shaped functionality because TermRock lacks
the primitive: composed field rows ("form/table rows with labels, values,
disclosures, masked secrets, and action sentinels"), a mixed tree list
("cannot use the flat picker renderer"), a five-slot dialog shim ("TermRock
no longer ships a fixed five-slot dialog facade" — restored in plan 003),
content-sized panel stacks, a wrapping measured hint bar (3-pass reflow),
and `DetailTable` width measurement. Grok Build's premium block chrome is a
1-col animated accent rail. These are product-neutral building blocks; per
the repo's building-block law they belong in `termrock::widgets`/`layout`.

## Current state

APIs this plan composes (verify each exists before starting):

- `Surface` + `SurfaceRecipe` — `crates/termrock/src/widgets/surface.rs`.
- `resolve_list_row` / `ListRowRecipe` — `crates/termrock/src/style/tokens.rs:727`
  (verified at `539e7d03`; plan 001 added `hover_wash`).
- `ListRow` composed slots — `crates/termrock/src/widgets/list.rs:143-168`
  region (leading/secondary/status/badge/actions/trailing/custom — Jackin
  bypasses all of them today; FieldRow is the answer, not more ListRow
  slots).
- Motion kit (plan 006): `wave_brightness`, `blend_toward`,
  `effective_alpha`.
- Glyph catalog: disclosure glyphs already exist
  (`tokens.glyphs.disclosure_closed()/disclosure_open()` — verified used in
  `widgets/panel.rs:700-703`); gutter glyph `selection_gutter()`
  (`tokens.rs:747`).
- Existing tree widget: `crates/termrock/src/widgets/tree.rs` (uses
  `resolve_list_row` at ~line 1042); `tree_navigation.rs`, `tree_table.rs`
  exist — read their module docs before deciding TreeList's relationship
  (extend `Tree`, do NOT create a duplicate if `Tree` already supports
  disclosure + tone tiers after inspection; the gap analysis says it lacks
  per-row tone tiers, hover fill, and fixed-prefix horizontal scroll).
- Hint bar: `crates/termrock/src/widgets/hint_bar.rs` — single-line
  (`render_hint_bar`); a wrapped variant (`wrapped_hint_lines`) may already
  exist — grep `wrapped_hint_lines` in `crates/termrock/src`; if present,
  v2 = expose `measured_height(width)` + built-in leading-spacer option on
  the widget instead of consumer-side reflow.
- `DetailTable` — `crates/termrock/src/widgets/detail_table.rs`.
- Repo law: widgets never depend on `patterns`; all six primitives are
  product-neutral (no product nouns in APIs).

## Commands you will need

| Purpose | Command | Expected |
|---|---|---|
| Check | `mise run check` | exit 0 |
| Gate | `mise run gate` | exit 0 |
| Contracts/registry | `mise run contracts` | exit 0 |

## Scope

**In scope**:

- New: `crates/termrock/src/widgets/field_row.rs`, `accent_rail.rs`;
  `crates/termrock/src/layout/panel_stack.rs`
- Extend: `crates/termrock/src/widgets/tree.rs` (tone tiers, hover fill,
  h-scroll) — or new `tree_list.rs` ONLY if extending `Tree` is proven
  infeasible (record why in the commit message)
- Extend: `crates/termrock/src/widgets/hint_bar.rs`,
  `detail_table.rs`
- `crates/termrock/src/widgets/mod.rs` (exports), `layout/mod.rs`
- Lookbook: stories + knobs for every new primitive
- Registry/catalog entries + contract matrix rows (whatever
  `mise run contracts` and `docs-quality` tasks enforce — run them and
  follow errors)
- Generated component MDX, public API inventory, affected preview frames, and
  `artifacts/visual-qa/plan-007/`
- `migrations/0267-*.md` + `MIGRATING.md` (additive; migration documents new
  canonical paths replacing consumer workarounds)
- `plans/README.md`

**Out of scope**:

- Adopting these primitives inside existing widgets/patterns (plans 008–010).
- Dialog five-slot (landed in plan 003), truncate/motion (plan 006).

## Git workflow

`main`, `git commit -s`; one commit **per primitive** (each independently
buildable — repo law). Messages like `feat(widgets): FieldRow composed row primitive`.

## Steps

### Step 1: `FieldRow`

Anatomy (all slots optional except value):

```
[gutter 2col][marker Ncol][label → padded to label_cols][value][ annotation]
```

- `FieldRowValue` enum: `Plain(&str)`, `Masked { len: usize }` (paints `●`×len,
  ASCII `*`), `Composed(Line)` (caller-styled, e.g. breadcrumbs),
  `Unset { hint: &str }` (renders hint in `Role::Danger` when
  `required`, else `TextMuted`).
- Label column: `label_cols: u16` fixed width (content-derived helper:
  `FieldRow::label_cols_for<'a>(labels: impl Iterator<Item=&'a str>) -> u16`
  = max display cols, min 8).
- States via `resolve_list_row` (selected/hovered/disabled) — gutter glyph +
  tint wash come from the recipe, not reimplemented.
- Annotation slot: `TextMuted` + optional ITALIC.
- Roles only — zero hardcoded colors. Density-aware padding from
  `DesignSystem.spacing`.

**Verify**: unit tests — column math (label padding, wide-glyph labels via
`truncate_cols`), masked width, unset-required danger role;
`cargo nextest run -p termrock field_row` → pass.

### Step 2: `AccentRail`

Block chrome primitive: paints a 1-col vertical rail at the left edge of a
Rect with a semantic role; content area = rect minus rail+gap.

```rust
AccentRail::new(&system, Role::ActorAssistant)
    .glyph(Glyph::RailHeavy)          // `┃`, ASCII `|` — add to catalog if missing
    .active(bool)                      // active → wave animation
    .tick(u64)                         // from FrameTick; ignored unless active
    .layout(area) -> (rail: Rect, content: Rect)
    .paint(area, buf)
```

Active rail: per-row fg = `blend_toward(rail_color, canvas, 1.0 −
wave_brightness(tick, row, 32, speed))` with `effective_alpha` honoring
`Motion` reduction (reduced → static full-brightness rail). Collapsed-block
variant glyph `❙` (add to catalog, ASCII `|`).

**Verify**: tests — layout splits correctly at width 1..3; reduced motion →
identical frames for any tick; `cargo nextest run -p termrock accent_rail`
→ pass.

### Step 3: Tree tone tiers + hover + h-scroll

In `tree.rs` (after reading its current API): add per-row
`ToneTier { Primary, Live, LiveDim }` (maps to `Text` / `InfoStrong` /
`InfoDim`) on tree rows, hover wash via `ListRowRecipe.hover_wash`, and
fixed-prefix horizontal scroll (disclosure+indent prefix stays pinned while
label region scrolls; follow the h-scroll pattern used elsewhere — grep
`h_scroll\|horizontal_scroll` in `widgets/` for prior art; if none exists,
implement label-window slicing with `display_cols_slice_into` from
`crates/termrock/src/text/`, verified used in `status_bar.rs:812`).

**Verify**: `cargo nextest run -p termrock tree` → pass incl. new tier/hover
tests.

### Step 4: `PanelStack` layout

`layout/panel_stack.rs`:

```rust
pub struct PanelStackBlock {
    pub content_rows: u16,   // measured content height (no chrome)
    pub chrome_rows: u16,    // borders etc. (usually 2)
    pub min: u16, pub max: u16,
    pub visible: bool,       // false → omitted entirely (no gap)
}
pub fn panel_stack(area: Rect, blocks: &[PanelStackBlock], gap: u16) -> Vec<Option<Rect>>
```

Semantics (Jackin's proven behavior): each block gets
`(content_rows + chrome_rows).clamp(min, max)`; invisible blocks yield
`None` and consume nothing; overflow shrinks from the end (consistent with
`OverflowPolicy::ShrinkFromEnd` naming in `layout/stack.rs:245`, verified).

**Verify**: table-driven tests: empty blocks omitted, caps applied, overflow
shrinks last block first; `cargo nextest run -p termrock panel_stack` → pass.

### Step 5: HintBar v2 + DetailTable::measure

- `hint_bar.rs`: add `measured_height(&self, width: u16) -> u16` (wrapped
  line count + optional leading spacer row) and a
  `.leading_spacer(bool)` option that paints the spacer as an intentional
  band (same bg as the bar). Single-line behavior remains the default
  (`measured_height` == 1 for it).
- `detail_table.rs`: add `measure(&self) -> (u16, u16)` returning content
  (width, height) pre-render so hosts can size panels without painting
  (Jackin computes this by hand today).

**Verify**: hint wrap test at widths 20/40/80 (height monotonically
non-increasing); `DetailTable::measure` equals painted extent in an
existing-render test; `cargo nextest run -p termrock hint_bar detail_table`
→ pass.

### Step 6: Stories, registry, migration, gate

- Lookbook story per primitive (field-row states incl. masked + unset;
  accent-rail actors + active wave; tree tiers; panel-stack omission demo;
  hint-bar wrap knob). Deterministic ticks only.
- Registry/contract/catalog entries: run `mise run contracts` and
  `mise run docs-quality`, follow errors until green (every public widget
  must appear in the generated API inventory + contract matrix — repo law).
- `migrations/0267-v0.13.0-composition-primitives.md`: additive primitives,
  canonical replacements for known consumer workarounds (name them: five-slot
  dialog shim → plan 003 Dialog; tree renderer → Tree tiers; field rows →
  FieldRow; hint reflow → `measured_height`; width probe →
  `DetailTable::measure`). Link from `MIGRATING.md`.

**Verify**: `mise run check` → 0; `mise run gate` → 0. Commits per primitive.

## Test plan

Per-step tests named above (12+ new tests). Model widget tests on
`list.rs`/`panel.rs` test modules; layout tests on `stack.rs` tests
(table-driven style at `stack.rs:1272` region).

## Done criteria

- [ ] `mise run check` + `mise run gate` + `mise run contracts` exit 0
- [ ] `grep -n "pub struct FieldRow\|pub struct AccentRail" crates/termrock/src/widgets/*.rs` → both found
- [ ] `grep -n "panel_stack" crates/termrock/src/layout/` → found + exported
- [ ] `measured_height` on hint bar and `measure` on DetailTable exist with tests
- [ ] Every new primitive has a lookbook story and registry/contract entries
- [ ] `migrations/0267-*.md` exists, linked
- [ ] `plans/README.md` updated

## STOP conditions

- `Tree` extension collides with `tree_table.rs`/`tree_navigation.rs`
  responsibilities (unclear which owns row paint) → report the ownership map
  before writing code.
- Registry/contract generators reject a primitive for a structural reason
  (e.g. requires an interaction contract this plan didn't design) → report;
  don't stub contracts.
- Any primitive needs a product noun to be useful → it's a `patterns/`
  composite, not a widget; report the boundary question.

## Maintenance notes

- Plans 008/009 adopt these inside agent/data widgets — signature changes
  after this plan ripple; coordinate via plans/README.
- Reviewers: FieldRow + AccentRail must not duplicate `ListRow` slot logic —
  they compose `resolve_list_row`; flag any second styling path.

## Amendments

- 2026-08-12: Added generated component MDX/API/frame outputs and
  agent-browser review artifacts omitted from Scope. These primitives are
  public catalog surfaces; repo cross-surface law and the standing browser
  gate require their generated docs and responsive evidence in the same
  independently-green commits.
- 2026-08-12: Moved each primitive's story, contract, generated docs, preview,
  and browser review into that primitive's commit instead of deferring all
  cross-surface work to Step 6. The original order contradicts its own
  independently-green commit rule and repo law requiring every public widget
  to have catalog coverage. Deferral fails those authorities; one large commit
  discards the requested isolation; complete vertical slices satisfy both with
  the smallest process-only change. The shared migration/index remains in the
  final slice.
