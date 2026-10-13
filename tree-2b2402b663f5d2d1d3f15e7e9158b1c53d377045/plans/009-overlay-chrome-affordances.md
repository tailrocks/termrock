# Plan 009: Overlays that float — shared headers, real hint bars, honest states, spec-true permission prompt

> **Executor instructions**: Follow this plan step by step. Run every
> verification command and confirm the expected result before moving to the
> next step. If anything in the "STOP conditions" section occurs, stop and
> report — do not improvise. When done, update the status row for this plan
> in `plans/README.md`.
>
> **Drift check (run first)**: plans 004/005/006/007 DONE in
> `plans/README.md`. Re-locate every site with `rg` before editing.

## Status

- **Priority**: P2
- **Effort**: L
- **Risk**: MED
- **Depends on**: plans/007
- **Category**: design
- **Planned at**: commit `605217aa`, 2026-08-14

## Why this matters

Plan 004 gave overlays elevation + backdrop; this plan makes them coherent:
today titles are placed four different ways, every overlay hand-rolls its
footer hints as one flat muted sentence (the `HintKey/HintText/HintDim/
HintSeparator` role quartet is consumed by 1 of 23 overlay widgets),
empty/loading/error bodies are ad-hoc parenthesised strings, tooltips paint
bare text over live content, the drawer double-borders against its host
pane, the completion menu claims the focused border while declaring itself
non-focusable, and the permission prompt — the highest-risk surface —
renders Critical requests with the same friendly phosphor border as benign
ones. Finish by flipping ON the neon-fill design gate.

## Current state (leads verified by audit at `605217aa`; dialog/panel anchors verified first-hand)

- Hints: canonical `hint_bar.rs:126-132`; hand-rolled flat strings:
  `question_flow.rs:1304`, `notification_center.rs:50,1322`,
  `command_palette.rs:1213`, `history_picker.rs:983`,
  `alert_dialog.rs:1037-1041`, `progress_steps.rs:817-824`,
  `jump_overlay.rs:848-851`, `dialog.rs:1408-1414` (`footer_hint: &str`).
  Separators differ (`·`, spaces, none).
- Titles: border-title via `Panel::block()` (`dialog.rs:1283-1286`) vs
  manual `set_stringn` over the border row (`command_palette.rs:1346-1354`)
  vs in-body first row (`drawer.rs:1063-1075`, `popover.rs:764-781` — both
  double-bold `TextStrong + BOLD`) vs none (`completion_menu.rs`).
- Empty/loading/error: `EmptyState`/`ErrorState` widgets have zero adopters
  among overlays; ad-hoc `"(empty)"` strings at `file_picker.rs:1512-1518`,
  `model_mode_selectors.rs:1150-1156`, `notification_center.rs:1104-1110`,
  `permission.rs:1449-1458`; `completion_menu.rs:1162-1177` writes empty
  message top-left while its own `paint_centered_msg` (`:1435-1447`) is used
  only for loading; loading treatments diverge (centered msg vs `… ` prefix
  vs title suffix vs "Checking…" text).
- Discarded affordances: `file_picker.rs:1394-1405` (`let _ = hint;` — no
  footer at all), `combobox.rs:1019-1029` (chevron computed, discarded),
  `combobox.rs:1000-1012` (corrected validation discarded),
  `notification_center.rs:1294-1313` (severity role discarded; identical
  ternary arms), `popover.rs:783-786` (header rule never painted),
  `alert_dialog.rs:1077` (`let _ = phrase;` — confirm phrase never shown),
  `drawer.rs:1097` (`let _ = display_cols;`), dead identical `colorless`
  arms `tooltip.rs:658-662`, `popover.rs:765-773`.
- Tooltip: `tooltip.rs:678-706` — Elevated fill only for `Rich`;
  `Plain`/`Shortcut` write text with no fill/border/padding.
- Drawer: `drawer.rs:932-968` — full 4-side border regardless of dock edge +
  handle column = triple vertical rules at the seam.
- Completion menu: `completion_menu.rs:1139-1143` — border =
  `BorderFocused` unless colorless, while `:1380` declares
  `.focusable(false)`.
- Permission (§5.11 spec): `permission.rs:1477-1486` — emphasis from
  `surface` only (risk ignored) + `AccentRail` + panel border double chrome;
  body = ~12 unaligned `TextMuted` lines (`:1495-1694`); egress warning
  color-only (`:1580-1586`).
- Jump overlay handled in plan 007 (accent) — here only the backdrop dim
  hookup if not done.
- Overlay pickers ragged columns: `history_picker.rs:1249-1296` (pin prefix
  shifts columns), `multi_select.rs:1155-1159` (desc concatenated into
  label, one style), `notification_center.rs:1277-1285` (raw `t{epoch}`),
  `model_mode_selectors.rs:1210-1216` (indent mismatch).
- Gate: `design_gate.rs::no_widget_paints_selection_fill_by_default` is
  `#[ignore]`d (plan 004 Step 7).

## Commands

| Purpose | Command | Expected |
|---|---|---|
| Fast gate | `mise run check` | exit 0 |
| Full gate | `mise run gate` | exit 0 |

## Scope

**In scope**: `dialog.rs`, `alert_dialog.rs`, `drawer.rs`, `popover.rs`,
`tooltip.rs`, `dropdown_menu.rs`, `command_palette.rs`, `completion_menu.rs`,
`slash_command_menu.rs`, `combobox.rs`, `multi_select.rs`,
`date_time_picker.rs` (hints only), `file_picker.rs`, `history_picker.rs`,
`jump_overlay.rs`, `question_flow.rs`, `permission.rs`, `form_wizard.rs`
(hints/nav), `progress_steps.rs`, `model_mode_selectors.rs`,
`notification_center.rs`, `mention.rs`, `menu_nav.rs`, `hint_bar.rs`,
`empty_state.rs`/`error_state.rs` (adoption seams), `card.rs`/`callout.rs`
(container language only if needed), `design_gate.rs`;
`migrations/0291-*.md` + `MIGRATING.md`.

**Out of scope**: patterns (plan 010), lookbook (plan 011), status colors
(done in 007), elevation/backdrop plumbing (done in 004).

## Git workflow

`main`; commits per step; `git commit -s`.

## Steps

### Step 1: `OverlayHeader` + hint unification

- One header helper: border-title placement (the `dialog.rs` way) with
  optional `!`/loading prefix; adopt in command_palette, drawer, popover,
  notification_center, permission, date_time_picker; delete manual title
  writes + double-bold.
- `Dialog::footer_hint` accepts hints (`&[(key, label)]`-shaped) rendered
  through `HintBar`; convert the eight hand-rolled hint strings; one
  separator (`·` from glyph catalog).

**Verify**: `rg -n "HintKey" crates/termrock/src/widgets/ | wc -l` ≥ 8 files; overlay tests updated.

### Step 2: Empty/loading/error adoption

Route the ad-hoc `"(empty)"`/`"(no …)"` bodies through `EmptyState`
(centered, muted, glyph); completion_menu empty path uses
`paint_centered_msg`; loading = title-suffix treatment everywhere a title
exists, centered message otherwise.

**Verify**: per-widget tests; no `"(empty)"`-style literals:
`rg -n '"\((empty|no )' crates/termrock/src/widgets/` → 0.

### Step 3: Restore the discarded affordances

file_picker footer (reserve a row; HintBar), combobox chevron (reserved
right cell) + validation pass-through, notification_center severity glyph
cell (separately styled leading cell), popover header rule, alert_dialog
confirm phrase display, delete `let _ =` leftovers + dead colorless arms.

**Verify**: `rg -n "let _ = (hint|phrase|display_cols|validation|item\.kind)" crates/termrock/src/widgets/` → 0.

### Step 4: Tooltip + drawer + completion border

Tooltip: all variants = Overlay fill + 1-cell pad + quiet Border outline.
Drawer: three-sided border (open edge borderless; handle column is the
inner rule); header rule row. Completion menu: `focused(bool)` builder
default false → quiet Border (slash_command_menu inherits).

**Verify**: drawer render shows single rule at seam (test); completion menu
border role == Border by default.

### Step 5: Permission + danger chrome — QUIET default (supersedes the old red-border spec)

Per `termrock-component-audit-2026-08.md` D1/F8 and `web-premium-tui-law.md`
(shadcn/Amp/Linear/Grok consensus): danger lives on the **confirm button
only**, not the container chrome. `DangerChrome::{Quiet (default), Loud
(opt-in)}`:
- Quiet = neutral `Border` container, `!` + word carry danger in the title,
  red solid chip ONLY on the destructive confirm action.
- Loud (irreversible-of-irreversible, explicit opt-in) = `Danger` border too.
Permission prompt: Quiet chrome + `!` title prefix scaled by risk; drop the
AccentRail; provenance/scope block via `KeyValueList` (aligned label column,
values `Text`, labels `TextMuted`); egress line gains `!` glyph; decision
list stays identity-based (verify default for High = Deny — report if not,
do not change semantics); footer hints via HintBar. Same Quiet default
applies to ChoiceDialog/AlertDialog danger variants (plan 004's
`PanelChrome::Danger` + `title_prefix` mechanism stays — its USE becomes
opt-in Loud).

**Verify**: permission snapshot: Critical shows danger border + `!`;
`rg -n "AccentRail" crates/termrock/src/widgets/permission.rs` → 0.

### Step 6: Picker column discipline

Fixed-width leading slots (gutter 2 / glyph 2 / pin 2 always reserved);
descriptions/meta in `recipe.secondary`; relative timestamps
(`notification_center` formats `t{epoch}` → `3m ago` given a now-input —
if no clock input exists, display the raw seconds as `…s` and record the
API gap); model_mode indent fix.

**Verify**: pinned + unpinned history rows start text at the same column
(test asserts column).

### Step 7: Flip the gate + migration

- Un-`#[ignore]` `no_widget_paints_selection_fill_by_default` (plan 004
  Step 7); it must pass now.
- `migrations/0292-*.md` (next free) + `MIGRATING.md`: header/hint/empty
  unification, tooltip/drawer chrome, completion border default, permission
  chrome change, picker column changes.
- `mise run gate` → exit 0.

## Test plan

Per-step verifies; the un-ignored gate; churned snapshots re-blessed.

## Done criteria

- [ ] `mise run gate` exits 0.
- [ ] Neon-fill gate ACTIVE and green.
- [ ] `rg -n "footer_hint: Option<&" crates/termrock/src/widgets/dialog.rs` reflects hint-slice API (or equivalent) — no flat-string hints remain in scoped overlays.
- [ ] Permission Critical render: danger border, `!`, aligned KV block.
- [ ] Migration + `MIGRATING.md`; `plans/README.md` updated.

## STOP conditions

- `HintBar` cannot express a widget's hint (e.g. mid-line hints) — report
  rather than half-adopting.
- Drawer three-sided border breaks Block-based layout math twice — report.
- Permission decision-default semantics differ from spec — REPORT ONLY
  (semantics are out of scope; this plan is chrome).

## Maintenance notes

- New overlays must use OverlayHeader + HintBar + EmptyState; reviewers
  reject manual title/hint writes (the design gate's source scans can grow
  a check later).
- Notification relative-time needs a clock input if absent — recorded gap.
