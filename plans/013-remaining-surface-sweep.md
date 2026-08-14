# Plan 013: The remaining surfaces — workbench patterns, cards, and the five orphan widgets join the language

> **Executor instructions**: Follow this plan step by step. Run every
> verification command and confirm the expected result before moving to the
> next step. If anything in the "STOP conditions" section occurs, stop and
> report — do not improvise. When done, update the status row for this plan
> in `plans/README.md`.
>
> **Drift check (run first)**: plans 006/007/009/010 DONE in
> `plans/README.md` (this plan reuses their helpers: row_chrome,
> status-split, HintBar adoption, destructive-confirm, EmptyState adoption).
> Re-locate every site with `rg` before editing.

## Status

- **Priority**: P2
- **Effort**: L
- **Risk**: MED (wide pattern snapshot churn)
- **Depends on**: plans/010 (idioms proven on the priority patterns)
- **Category**: design
- **Planned at**: commit `605217aa`, 2026-08-14

## Why this matters

The gap audit of the last 35 surfaces (5 widgets, 30 patterns) found the
same six root causes plus a few real breakages: whole-row status hue at 11
more pattern sites; multi-line `Accent+BOLD` selection slabs across six
plan_review panes; `system.glyphs` has ZERO pattern consumers (every
pattern's `ascii(true)` yields mixed ASCII/Unicode frames); `colorless` is
inert in 7 patterns and actively inverted in `terminal_run_card` (colorless
REMOVES the status glyph); every running `terminal_run_card` claims
`PanelChrome::Focused` so N running cards = N bright borders; three
patterns stamp destructive confirms over live content with identically
styled buttons; `query_editor` overpaints its own title and the editor's
first cell; a `🔒` emoji misaligns the approval queue; the approval queue's
safety banner is the dimmest text on its screen.

## Current state

Leads with file:line for every site are in the gap audit's findings
[GAP-01]…[GAP-30]; the authoritative per-site list is reproduced compactly
below. Verify each with `rg` before editing (files are large; line numbers
are leads, the described mechanism is the contract).

Cluster inventory:
- **Status hue on whole rows** [GAP-01]: `approval_queue.rs:1071-1079`,
  `integration_status.rs:1136-1143`, `plan_review.rs:1896-1904,2046-2050`,
  `result_grid.rs:1288-1293`, `terminal_run_card.rs:915-921`,
  `subagent_card.rs:1106-1120`, `background_task_panel.rs:1252-1259`,
  `activity_shelf.rs:1082-1093` (every chip a status hue),
  `error_recovery.rs:1243-1251` (color-only, no glyph),
  `query_editor.rs:1284-1290`, `working_state_card.rs:798-810`.
- **Accent selection slabs** [GAP-02]: `plan_review.rs:1739,1790-1793,1807,1899,1991,2048`,
  `approval_queue.rs:1066-1068`, `integration_status.rs:1136-1138`,
  `git_workbench.rs:1396` (current branch permanent accent row) [GAP-27].
- **Recipe adoption root** [GAP-03]: patterns never call
  `list_row_recipe`/`Role::SelectionTint` — rewire the row loops through
  plan 006's `row_chrome` helper.
- **Glyph catalog + ascii** [GAP-04, GAP-05]: zero `system.glyphs.` uses in
  `patterns/`; un-gated glyphs in `git_workbench.rs:1387-1392` (`↑↓`),
  `plan_review.rs:1798`, `integration_status.rs:1130` (`↗`),
  `session_picker.rs:1548` (curly quotes), `database_workbench` (`● ≈ ↵`),
  `schema_browser` (`⬡ ƒ ⚡ ⚓`), `subagent_card` (`↳ │ ◉ ▣`),
  `working_state_card` (`◇`), `task_rail`/`prompt_queue` (`▸`),
  `activity_shelf`/`background_task_panel` (`∅ ⚠ ×`),
  `agent_status_header` (`⌥`), `·` separators everywhere; emoji:
  `approval_queue.rs:111` (`🔒` in row prefix — column jitter),
  `git_workbench.rs:1532` + `observability_dashboard.rs:1119` (`🧪` fixtures).
- **Colorless plumbing** [GAP-06]: inert in `file_manager.rs:551`,
  `database_workbench`, `git_workbench`, `help_center`,
  `observability_dashboard`, `project_launcher`, `result_grid`; partial in
  `plan_review` (2 of 22 sites); order-inverted in `approval_queue.rs:1066-1075`,
  `integration_status.rs:1136-1141`; glyph-destroying in
  `terminal_run_card.rs:963-967`; conflated with ascii in `task_rail.rs:1409`.
- **Border/status conflation** [GAP-07, GAP-08, GAP-10]:
  `terminal_run_card.rs:953-961` (Running → Focused chrome);
  `Panelish` double borders (`file_manager.rs:1585-1594,1700`,
  `project_launcher.rs:1436-1445,1501`, `git_workbench.rs:1265-1278`,
  `database_workbench.rs:1395-1409`, `error_recovery.rs:1283`,
  `help_center.rs:1595`) with `preview_card.rs:1114-1119` pinned→
  BorderFocused compounding; focus-by-brightness-only panes
  (`result_grid.rs:1288-1293`, `schema_browser.rs:1094-1103`,
  `query_editor.rs:1232-1291,1387-1432`, `database_workbench.rs:1410-1478`).
- **Destructive confirms** [GAP-09]: `git_workbench.rs:1405-1438`,
  `prompt_queue.rs:905-936`, `session_picker.rs:1538-1600` → plan 010's
  AlertDialog-based `paint_destructive_confirm`.
- **Empty/loading/error** [GAP-11]: 24 parenthetical strings —
  `help_center.rs:1452,1508,1544,1567`, `integration_status.rs:1101,1304,1338,1363`,
  `plan_review.rs:1578,1860,2002`, `carousel.rs:276-283`,
  `approval_queue:1022`, `file_manager:1614`, `project_launcher:1402-1404`,
  `schema_browser:1136-1142`, `observability_dashboard:1059`,
  `session_picker:1480`, `background_task_panel:1184-1186`,
  `process_table:1298`, `activity_shelf:333`, `preview_card.rs:1221-1258`.
- **Hints** [GAP-12]: zero `Role::HintKey` uses in patterns;
  `carousel.rs:335-341`, `keybinding_recorder.rs:969-991`,
  `preview_card.rs:1279-1290` flat.
- **Rainbow rows** [GAP-13, GAP-22]: `agent_status_header.rs:960-1042`
  (5 hues; separators inherit status hue; narrow overflow drops the
  actionable `q:{n}` segment first — add priority sort),
  `session_picker.rs:1486-1521` (11-line 5-hue preview).
- **Dead API/statements** [GAP-14, GAP-16]: dead `role()` taxonomies
  `schema_browser.rs:137-149,208-215`, `result_grid.rs:97`; discards —
  `keybinding_recorder.rs:912-918,996` (Kbd chip built then thrown away),
  `result_grid.rs:731-739` (binary cells blank), `query_editor.rs:1215-1219`
  (success rows/duration discarded), `background_task_panel.rs:977,1343-1366`
  (ANSI passthrough dropped; dead `if let`), `task_rail.rs:1435`,
  `subagent_card.rs:1075`, `terminal_run_card.rs:1110`,
  `observability_dashboard.rs:551`, `database_workbench.rs:1146`,
  `help_center.rs:771`, `project_launcher.rs:1329`.
- **Motion honesty** [GAP-15]: `working_state_card.rs:804-810` Waiting ==
  static-Running glyph; shared `presence_glyph(status, motion, tick)`.
- **Point fixes** [GAP-17..29]: query_editor overlay/`›` stamp;
  dev-string leaks (`query_editor.rs:1424-1426`, `help_center.rs:1571-1575`,
  `integration_status.rs:1363`, `database_workbench.rs:1387`);
  hand-rolled tab strips (`database_workbench.rs:1421-1446` whole strip
  accent, `integration_status.rs:1190-1201`, `plan_review.rs:1735-1746`) →
  `widgets::Tabs` or marker+muted; `log_pane.rs:318-338` follow indicator
  accent→muted + catalog glyphs + ascii knob; approval banner
  (`approval_queue.rs:1000-1012`) → `!` Danger glyph + TextStrong;
  `background_task_panel.rs:1265-1287` trailing `·`;
  `ops_dashboard.rs:196-217` Tab cycles into no-op regions;
  carousel [GAP-23]; preview_card [GAP-24] (meta muted; pinned = `◆` +
  Border, never BorderFocused); image_surface [GAP-25] (catalog glyphs +
  lifecycle glyph); keybinding_recorder [GAP-26] (recording = `●` Accent +
  bold chord + Focused border, no REVERSED slab; render the Kbd chips).

## Commands

| Purpose | Command | Expected |
|---|---|---|
| Fast gate | `mise run check` | exit 0 |
| Full gate | `mise run gate` | exit 0 |

## Scope

**In scope**: `crates/termrock/src/widgets/{carousel,image_surface,
preview_card,keybinding_recorder,log_pane}.rs`; `crates/termrock/src/patterns/`
all files named above; `design_gate.rs` (extend `patterns_compose` to the
full pattern set); `migrations/0298-*.md` + `MIGRATING.md`.

**Out of scope**: reference paints for the four geometry-only recipes
(`ops_dashboard`, `resource_browser`, `studio_shell`, `agent_shell`) —
GAP-30 is recorded as a follow-up in `plans/README.md` (additive feature,
own design pass); the GAP-29 focus-cycle fix IS in scope (behavioral bug).

## Git workflow

`main`; commit per cluster; `git commit -s`.

## Steps

1. **Recipe + status-split adoption across remaining patterns** —
   [GAP-01,02,03,27]: rewire the row loops through plan 006's
   `row_chrome` and plan 007's status-split helpers; selection = gutter +
   tint + strong; status hue on glyph/letter cell only; colorless keeps
   REVERSED as the mono fallback.
   **Verify**: `rg -n "Role::Accent" crates/termrock/src/patterns/ | rg -v "gutter|cursor|Focused|live"` — review each survivor; extended `patterns_compose` gate green on the full pattern set.
2. **Glyph catalog + colorless plumbing** — [GAP-04,05,06]: patterns take
   glyphs from `system.glyphs`; per-pattern `ascii`/`colorless` become
   force-overrides seeded from the system (plan 008 Step 1 API); fix the
   inversion sites (`terminal_run_card` colorless KEEPS the glyph;
   `task_rail` unconflates the two flags); `🔒`→catalog 1-col glyph;
   `🧪` out of fixtures; `·` separators via catalog.
   **Verify**: `rg -n "[^\x00-\x7F]" crates/termrock/src/patterns/ -g '*.rs' | rg -v "glyphs\.|//|test"` — survivors reviewed (doc comments/tests only); colorless render of terminal_run_card shows status glyph (test).
3. **Border discipline** — [GAP-07,08,10]: terminal_run_card emphasis from
   `state.focused` only; shared `Panelish` helper with inner-owns-chrome
   mode (kills double borders); preview_card pinned = `◆` + `Border`;
   db-workbench family panes get real `Panel` focus borders; title lines
   fixed `TextStrong`.
   **Verify**: render db_workbench with focus in each pane → exactly one `BorderFocused` (test); three running cards → zero `BorderFocused` unless focused.
4. **Destructive confirms + empty/hints** — [GAP-09,11,12]: the three
   confirm overpaints → plan 010's AlertDialog helper; 24 empty strings →
   EmptyState/centered-muted; hint lines → HintBar/`HintKey` spans in the
   5 widget/pattern sites.
   **Verify**: `rg -n '"\(no |"\(select ' crates/termrock/src/{widgets,patterns}/` → 0.
5. **Rainbow + point fixes + dead code** — [GAP-13..29 remainder]: as
   inventoried above; priority-sorted segments in agent_status_header;
   finish-or-delete each `let _ =` discard (Kbd chips rendered; binary
   cells show `type_label`; query success shows rows+duration; ANSI
   passthrough wired or field removed with a doc note); ops_dashboard Tab
   skips non-interactive regions.
   **Verify**: `rg -n "let _ = " crates/termrock/src/patterns/ crates/termrock/src/widgets/{keybinding_recorder,preview_card,carousel,log_pane,image_surface}.rs` → 0 non-test matches; per-site tests.
6. **Gate + migration**: extend `design_gate.rs::patterns_compose` source
   scans to all patterns; `migrations/0299-*.md` (next free) +
   `MIGRATING.md` (visible changes per pattern/widget).
   **Verify**: `mise run gate` → exit 0.

## Test plan

Per-cluster verifies; churned snapshots re-blessed; new behavior tests
named in steps 2/3/5.

## Done criteria

- [ ] `mise run gate` exits 0.
- [ ] Full-pattern `patterns_compose` gate green.
- [ ] Colorless/ascii honest in every pattern (spot tests per Step 2).
- [ ] One focused border per scene across the workbench patterns (Step 3 tests).
- [ ] Migration + `MIGRATING.md`; `plans/README.md` updated.

## STOP conditions

- A pattern's row model genuinely can't route through `row_chrome`
  (non-list geometry) — apply the status-split rules directly and record it.
- Fixing `background_task_panel` ANSI passthrough requires new public API —
  report the API sketch instead of adding it ad hoc.
- `Panelish` consolidation changes public pattern surfaces consumed
  downstream — allowed (pre-1.0), but list every changed surface in the
  migration.

## Maintenance notes

- GAP-30 follow-up (reference paints for the 4 geometry-only recipes)
  recorded in `plans/README.md`.
- After this plan, every rendering surface in the crate is on the shared
  language; the design gates hold the line.
