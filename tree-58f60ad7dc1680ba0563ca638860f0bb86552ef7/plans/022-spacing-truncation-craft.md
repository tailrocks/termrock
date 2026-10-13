# Plan 022: The craft pass — insets, rhythm, alignment, honest truncation, one scrollbar

> **Executor instructions**: Follow this plan step by step. Run every
> verification command. STOP conditions binding. Update `plans/README.md`.
>
> **Drift check (run first)**: plans 002/004/015/017 DONE preferred (density
> tokens + budgets exist). Re-locate every cited site with `rg` — line
> numbers are leads; the described mechanism is the contract.

## Status

- **Priority**: P1 (designer micro-detail directive: padding + small details)
- **Effort**: XL (helpers first make the sweeps cheap; steps are independent commits)
- **Risk**: MED (1-cell geometry shifts ripple through hit rects + goldens)
- **Depends on**: plans/002 (glyph ellipsis token), 003 (catalog), 015 (breathing rows coordinate), 017 (budgets)
- **Category**: design / craft
- **Planned at**: commit `d09bd2fe`, 2026-08-14

## Why this matters

The pixel-craft audit: text sits flush against border glyphs across the
whole bordered-overlay family (9 widgets; CompletionMenu alone insets
correctly); the spacing scale is consumed by 5 files and hardcoded ~50
times; the library truncates with a hard cut 800+ times and with an
ellipsis 7 times — Panel truncates its own title by CHARS (CJK/emoji
overrun + mid-grapheme splits) and never truncates its footer; paths
end-truncate so `src/widgets/quick_o…` loses exactly the discriminating
token; process_table's header is one cell right of every data column;
DataTable has no cell alignment concept while sibling Table has the full
mechanism; four scrollbar implementations with two visual languages; the
menu/picker family scrolls with NO indicator at all; nine `.take(N)` clips
drop content silently (Form shows 3 of 5 validation errors with no hint).

## Current state — anchors verified first-hand at `d09bd2fe`

- `widgets/panel.rs:679-694` — title: `chars().take(budget-1) + "…"`
  (char-counted, grapheme-unsafe, hardcoded `…`); footer:
  `format!(" {} ", footer.trim())` with NO truncation; duplicate logic at
  `:908-913`; Quiet/DividerOnly footers hard-cut.
- Counts: `take_display_cols` ≈ 870 call sites in widgets+patterns;
  `truncate_cols(` = 7; `truncate_display_cols` (End/Start/Middle) = 1
  consumer (`text.rs:610`); canonical scrollbar `scroll/render.rs:16,35`
  (`·` track / `┃` thumb) has ONE widget caller (`list.rs:1441`).
- `patterns/process_table.rs:1270-1272` header `{:<4}` for a 1-char status
  vs row `{mark}{:<2}` — every column misaligned by one.
- `widgets/drawer.rs:936`, `dropdown_menu.rs:1185` — bordered Surface
  `.padding(0, 0)` + paints at `inner.x` (family: notification_center,
  preview_card, popover, menu_bar, fullscreen_viewer, image_surface,
  callout — callout also paints its rail at `inner.x` → `││` double rule).
- `widgets/key_value_list.rs:414-417` separator `"  "` vs
  `detail_table.rs:19` `" : "` vs `key_value_table.rs:43` `GUTTER=2`.
- Full finding inventory: micro-craft audit CRAFT-01..34 (this plan's
  step lists reproduce every site; re-locate each with the given rg).

## Scope

**In scope**: `text/mod.rs` (helpers), `style/tokens.rs` (+`ContentInset`,
`SpacerBand`, `KvSeparator` tokens), `scroll/render.rs` (spec API),
`style/glyph.rs` (tee glyphs `├ ┤` + ASCII), the widget/pattern files named
per step, `design_gate.rs`, migrations + `MIGRATING.md`.
**Out of scope**: hover/pressed states (021), motion (014), microcopy
wording (020 — but ellipsis PLUMBING is here, ellipsis LITERALS in copy are
020's; coordinate), content diets (017).

## Steps (helpers first — they turn the sweeps into call-site swaps)

### Step 1: The four shared helpers

a. `text::paint_text(buffer, rect, text, style, ellipsis)` — grapheme-safe
   `truncate_cols` + ellipsis from `glyphs.ellipsis()`; and lift
   `table.rs:1617-1680` span-aware `render_line_overflow` into a shared
   `paint_line_overflow` (styled `Line` + ellipsis + alignment-aware
   direction: Right-aligned uses `TruncateMode::Start` with LEADING
   ellipsis — fixes CRAFT-23's `1234…`-instead-of-`…5678`).
b. `text::truncate_path(text, budget, ellipsis)` — drop leading segments
   (`…/dir/file.rs`), fall back to Middle mode.
c. `DesignSystem::content_inset(bordered: bool) -> (u16, u16)` — density-
   resolved, floor 1-cell horizontal on bordered chrome at ALL widths
   (kills `panel.rs:719-728` narrow collapse); `SpacingScale::band()` (rows)
   + a shared drop-band-first contraction helper (unifies the four spacer
   implementations: dialog rhythm flag, kv_group_gap, markdown breathing,
   hint_bar leading_spacer — hint_bar default flips to ON).
d. `scroll::ScrollbarSpec` + `paint_list_scrollbar(buffer, gutter_rect,
   total, viewport, offset, system)` wrapping the canonical renderer.

**Verify**: helper unit tests (CJK title truncation, ZWJ family emoji,
path middle-drop, right-aligned leading ellipsis); `mise run check` green.

### Step 2: "Reserve the gutters" — ONE geometry batch (per audit sequencing note)

Same widgets, same thresholds, one pass: bordered-overlay 1-cell inset
(drawer, dropdown_menu, notification_center, preview_card, popover,
menu_bar, fullscreen_viewer, image_surface, callout — rail moves to
`inner.x+1`); list trailing anchor stable (always reserve the scrollbar
column, `list.rs:1139`); menu/picker family scrollbar gutter + indicator
(select, completion_menu, quick_open, multi_select, command_palette,
history_picker, dropdown_menu, menu_bar, notification_center, drawer,
event_stream, object_inspector — spare-column fallback: `n/m` counter in
the status strip); table column separator gutter (`value │ value` 3-cell
or 2-space — decide WITH the KvSeparator token, one family answer);
horizontal-clip chevrons `‹ ›` in data_table/tree_table edge columns;
menu separator rules meet borders with new `├ ┤` tee glyphs. Update every
affected hit-rect/slots export + narrow thresholds once.

**Verify**: per-widget geometry tests updated in the same commit; gate
render: dropdown + completion menu side-by-side have identical insets.

### Step 3: Truncation honesty sweep

Tier 1 (titles/labels/paths — the ~60 visible sites): Panel title+footer
(all four variants via one `chrome_label` fn), composed_row/list primary
via `paint_line_overflow`, quick_open + fullscreen_viewer + file-picker
paths via `truncate_path`, data_table sort marker reserved-then-truncate
(never cut the arrow; also space it: `CPU ▲` matching Table's tested form),
menu_bar/dropdown shortcut kept over label under pressure,
grapheme-unsafe fixes (`history_picker.rs:265-278` MaskMiddle → helper +
catalog mask glyph; `search_input.rs:961`), the ~25 hardcoded `"…"` →
`glyphs.ellipsis()` (incl. `breadcrumbs.rs:41` const). Tier 2 stays
hard-cut where content is pre-measured — document the rule on the helper.
`+N more` rows at the nine silent `.take(N)` clips (template:
`multi_select.rs:961-995`); `form.rs:822` first (3-of-5 errors bug),
`metrics_dashboard.rs:1226` second; fix `connectivity.rs:947` to match its
own `:938`.

**Verify**: `rg -n '\.chars\(\)\.take' crates/termrock/src/widgets/` → 0
in paint paths; `rg -c 'truncate_cols|paint_text|paint_line_overflow'`
grows past 60; form story with 5 errors shows `+2 more`.

### Step 4: Rhythm + alignment

Section spacer band before title (drop-first under height pressure, the
dialog contraction pattern); Form fieldset gap (measure + paint both);
popover/drawer header rule (kill the dead `if` at `popover.rs:783-786`);
process_table header offset fix + byte-offset regression test;
`CellAlignment` lifted shared → `DataColumn`/`ResultColumn` + right-align
math after clip; empty states: list/tree_navigation/dependency_graph/
completion_menu route through `EmptyState` (and rename `paint_centered_msg`
or make it center); HintBar: one alignment path (widget setter,
`render_hint_bar` delegates), inter-span space moved to join;
StatusBar separator symmetric `" · "`; GroupSep gets a real token;
bullet vocabulary → `glyphs.bullet()` + one indent (`·` reserved for
inline meta separator); `keyboard_help.rs:984` ASCII sep `" | "` → catalog;
breadcrumb chevron spaced form everywhere; `select.rs:1026` drop `.min(64)`;
KvSeparator token adopted by the three KV widgets.

**Verify**: process_table alignment test green; kv three-widget render uses
one separator; section/form stories show bands.

### Step 5: One scrollbar

Route `scroll_area` (incl. horizontal glyphs), `viewport`, `tree`, `form`,
`text_area` through the canonical renderer via `ScrollbarSpec`; delete the
duplicated thumb math; keep hit-rect bookkeeping at call sites
(text_area drag preserved — coordinate with plan 021 Step 4's thumb-drag).

**Verify**: `rg -n 'SCROLLBAR_TRACK|"·"' crates/termrock/src/widgets/` —
no local track constants; all scroll surfaces render the `·`/`┃` language;
text_area drag test green.

### Step 6: Spacing scale alive + gates + migration

Migrate the ~50 hardcoded inset/gap literals onto `content_inset`/
`SpacingScale` file-by-file (overlay family done in Step 2; this finishes
list gutters, form COLUMN_GAP, kv GUTTER, callout gutter_w, completion +1s
etc.). Gates: `design_gate.rs::text_never_touches_borders` (render the
bordered family; assert no non-border glyph in border-adjacent columns),
`::truncation_has_ellipsis` (flagship stories: any clipped title/label row
ends in the ellipsis glyph), `::one_scrollbar_language` (source scan).
Tiny-terminal spot-checks: flagship widgets render usable at 20×5 or
return their documented LineMode (adds the missing S24 evidence — one test
over List/TextInput/Panel/Dialog/StatusBar). Resize safety: one fuzz test
resizing 200 random rects over the overlay family (S49). Migration (next
free) + `MIGRATING.md`: geometry shifts, hint_bar spacer default, kbd
keycap padding decision (`[ C-s ]` per P13 — measure+paint together),
separator/ scrollbar unifications.

**Verify**: `mise run gate` exit 0; all three new gates green.

## Done criteria

- [ ] `mise run gate` exits 0; gates `text_never_touches_borders`,
      `truncation_has_ellipsis`, `one_scrollbar_language` green.
- [ ] `rg -n "padding\(0, 0\)" crates/termrock/src/widgets/` → only
      borderless/documented sites.
- [ ] Panel CJK title renders inside its border (test).
- [ ] Paths middle-truncate in quick_open (test asserts filename visible).
- [ ] One scrollbar language; menu/picker family shows position.
- [ ] `+N more` at all nine clip sites; process_table aligned.
- [ ] Migration + `MIGRATING.md`; README row updated.

## STOP conditions

- A 1-cell inset makes a widget unusable at its documented minimum size —
  report the widget + minimum; the floor rule may need a per-widget
  exception recorded in the law.
- Span-aware truncation cannot preserve styles at a boundary — report with
  the failing Line shape; do not ship color-bleeding tails.
- Scrollbar-gutter reservation conflicts with plan 021's thumb-drag rects
  mid-flight — land whichever is first, adapt the second; note in both.
- The kbd keycap padding decision contradicts a landed plan-015 chip recipe
  — the recipe wins; update law P13 wording instead.

## Maintenance notes

- `paint_text`/`paint_line_overflow` are the only sanctioned label
  painters going forward; new `take_display_cols` in title/label positions
  should trip review (extend the source-scan gate if it recurs).
- Geometry batch (Step 2) is the churn spike — schedule its golden
  re-bless as one review.
