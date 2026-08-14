# Plan 007: Status lives in the glyph; accent budget enforced — de-neon the data, feedback, and agent surfaces

> **Executor instructions**: Follow this plan step by step. Run every
> verification command and confirm the expected result before moving to the
> next step. If anything in the "STOP conditions" section occurs, stop and
> report — do not improvise. When done, update the status row for this plan
> in `plans/README.md`.
>
> **Drift check (run first)**: plans 002/004/005/006 DONE in `plans/README.md`.
> Re-locate every site with the given `rg` before editing; vanished sites get
> recorded, not improvised.

## Status

- **Priority**: P1
- **Effort**: L
- **Risk**: MED
- **Depends on**: plans/006
- **Category**: design
- **Planned at**: commit `605217aa`, 2026-08-14

## Why this matters

The binding rules (`docs/design/termrock-design-language.md` §3): status =
glyph + word first, color reinforces, confined to the glyph cell; secondary/
meta always muted; ≤2 accent-forward regions per viewport. Today whole rows,
bars, rails, and footers take saturated status/accent paint: every OK metric
tile is mint, every running row is brand green, a trace waterfall is a wall
of saturated blocks, a default status bar is a 4-color band, selected
transcript blocks flood accent. This plan is the "quiet canvas" sweep.

## The rules (apply mechanically)

1. Status color → the status glyph/letter cell ONLY. Body = `Text`/`TextStrong`;
   meta = `TextMuted`/`TextFaint`.
2. `Role::Accent` = current intent only (focused owner already has the
   border; one live/primary element max). Ambient chrome (filters, footers,
   meters, mode chips, avatars) never accent.
3. `Running` ≠ brand: running = spinner/`InfoDim` glyph + muted verb.
4. Style precedence: semantic role (danger/warning) composes OVER
   selection/cursor chrome via `.patch`, never `if/else` chains that drop one.
5. Charts: data = `ChartSeries1..4` only.

## Site inventory (leads verified by audit at `605217aa`; re-locate by rg)

Whole-row/бar status paint → glyph-only:
- `trace_waterfall.rs:1165-1177` (span names in status role), `:1225-1240`
  (solid `█` bars → `ChartSeries`-family or dim block ramp with status only
  on the letter).
- `log_stream.rs:1136-1145` (whole line in level role), `event_stream.rs:1063-1065`,
  `error_state.rs:722-757` (glyph+summary one danger string → glyph danger,
  summary `TextStrong`), `badge.rs:393-431,452-476` (variant color on glyph +
  delimiters; label `Text`), `callout.rs:367-378,407,443` (tone on rail+glyph;
  border stays `Role::Border`), `progress_steps.rs:126-134,768-780`,
  `model_mode_selectors.rs:1185,1608,1649`, `file_picker.rs:1494-1495`,
  `status_bar.rs:114-124,830-846` (slot text → `StatusBar`/`TextMuted`;
  role on glyph; ≤1 accent slot), `connectivity.rs:589`,
  `working_state_card` / `integration_status` / `background_task_panel` /
  `prompt_queue` / `subagent_card` analogues (patterns → plan 010; widget-side
  helpers here). Find: `rg -n "\.role\(\)\)" crates/termrock/src/widgets/`.
- `semantic_status.rs:81-91`: `Running => Role::InfoDim` (was Accent); give
  Running a distinct glyph from Online (`◐` family), keep `Success ≠ Accent`.
- `spinner.rs:607` `ActivityIndicator` default role → `TextMuted` (glyph may
  keep `Info`); `status_indicator.rs:350-364` label → `Text`, role on glyph.
- `view_state.rs` `LoadingView`/`Banner`: spinner glyph `Info`, label
  `TextMuted`; banner glyph carries severity, message `Text`.
- `toast.rs:1038-1050`: inner `│` rail → `Role::Border` (or drop rail);
  status color on icon only (`:1067,1097` already correct).
- `progress.rs:120-125,1035-1051,1105`: running fill → `ChartSeries1`/`Info`;
  accent only when the owning operation is focused/primary (add a
  `primary(bool)` builder if absent).

Accent-budget sites → muted:
- `highlighted_text.rs:65-77`: `Match => TextStrong` (+`HoverTint` bg opt),
  `Focused => Accent` (the one sanctioned accent), drop Warning.
- `hex_viewer.rs:1245-1255`, `trace_waterfall.rs:1051-1060`,
  `dependency_graph.rs:1224-1233` filter rows → `Sunken` well + `TextMuted` +
  accent on cursor cell only; same for `file_tree.rs:1098-1126` filter/rename
  rows and `tree_navigation.rs:908-918` (one shared inline-chrome-row helper:
  prefix glyph + muted body; danger only for confirm rows which keep `!`).
- `context_meter.rs:721,908-918`; `checkpoint_timeline.rs:1236-1237,1272`;
  `empty_state.rs:113` FirstUse → `TextStrong`; `diagnostic.rs:126`
  Hint/Help → `Info`; `pagination.rs:920-923,1146-1150` reverse-in-color-path
  → mono-only.
- `search_input.rs:840-861` filter chips → Tag/Chip composition or muted
  bordered chips; accent only on the active chip; `search_input.rs:874-879`
  NoResults → `TextMuted` + `∅` glyph.
- `menu_bar.rs:1410-1412` mnemonic mode → highlight the mnemonic char only;
  `menu_bar.rs:1631-1638` destructive rows compose Danger over active recipe
  + leading glyph (rule 4).
- `input_group.rs:288-292` suffix addons → `TextMuted`, accent only when
  targetable.
- Action chips: `action_bar.rs:184-196` — `ActionFocused` chip ONLY for the
  default confirm; cursor = bracket/gutter + `TextStrong`; make `Action::style`
  a patch not an override (fixes `permission.rs:1728-1735` losing cursor
  state); `button_group.rs:697-706` — fix black-on-tint contrast bug (tint
  under, legible fg over); `fullscreen_viewer.rs:1244-1250` +
  `tool_call_card.rs:986-991` route through `button_recipe`;
  `form_wizard.rs:1244-1252` nav → `ActionBar`.
- `jump_overlay.rs:803-822`: labels = bracketed accent fg on `Sunken` chip
  (not `ActionFocused` slabs); implement the documented prefix masking
  (`[·b]`); dim non-target content while active (Backdrop from plan 004).
- Agent surfaces: `transcript.rs:806-815` selected block keeps `kind_style`
  base + gutter/tint (no Accent flood); `accent_rail.rs:98-121` add quiet
  tier (~0.4 blend) for non-active blocks, full brightness only
  active/selected; `message_thread.rs:1078` footer — accent only while
  search input is live; `agent_blocks.rs:180-185` mode chip muted unless
  ribbon owns focus; `prompt_composer.rs:1987-1994` `›` accent only when
  `accepts_input`; busy verb per spec (`streaming`); `review.rs:1590-1799`
  cursor=gutter, multi-select=muted `☑`, stats/marks `TextMuted`, draft row
  muted, summary accent only when focused; `loading_overlay.rs:555-572`
  wash via `Role::Backdrop` style patch WITHOUT `set_char` (preserve
  content), labels `TextMuted`; `agent.rs:62-83` TokenMeter compact form
  (`▰▰▰▱▱ 64%`) — never clip numbers mid-token
  (`prompt_composer.rs:2062-2070` width plumbing).
- Identity: `identity.rs:95-101,290-295` → `Actor*` roles + ChartSeries hash
  palette (never Warning/Success/Accent as identity hues).
- Charts: `charts.rs:303-315` series = `ChartSeries1..4` cycle only;
  defaults `:353,1162,1180,1903` → `ChartSeries1`; bipolar `:1929,1941` →
  series roles + `+/−` prefix; legends `:771-788,2391-2407` per-series
  marker color + `TextMuted` labels.
- Glyph literal hygiene (sites found by data audit): `error_state.rs:775,795,797`,
  `scroll_area.rs:795-802`, `context_meter.rs:910`, `connectivity.rs:970`,
  `separator.rs:441,444,458,460` (heavy/double → `rule()/rule_v()/rule_strong()`;
  emphasis via role), `table.rs:1685-1686` ASCII arm, `review.rs:1784` `💬`
  → catalog glyph; route through `system.glyphs.resolve(...)`.
- Precedence chain bugs (rule 4): `dropdown_menu.rs:1283-1305`,
  `model_mode_selectors.rs:1645-1653`, `question_flow.rs:1252-1275`,
  `badge outline` (`badge.rs:63-71` Outline label → `TextMuted`, delimiters
  `Border`), `date_time_picker` handled in plan 008.
- Selected-row information loss: `search_results.rs:1294-1313` (highlights
  painted on selected rows too), `diff.rs:1504-1524` (cursor line keeps word
  diff), `log_stream.rs:174-175,1158-1167` (styled spans painted span-wise,
  not flattened), `streaming_markdown.rs:815,836-847` (forward colorless;
  reserve error-strip row), `diagnostic.rs:806-812,878-881` (caret color from
  the diagnostic's severity).

## Commands

| Purpose | Command | Expected |
|---|---|---|
| Fast gate | `mise run check` | exit 0 |
| Full gate | `mise run gate` | exit 0 |

## Scope

**In scope**: files listed above (widgets only); a shared
`widgets/row_chrome.rs` or `status_paint` helper for glyph-vs-body split;
`crates/termrock/tests/design_gate.rs` (accent-budget test);
`migrations/0288-*.md` + `MIGRATING.md`.

**Out of scope**: patterns (plan 010), overlay container chrome (plan 009),
inputs (plan 008), lookbook (plan 011).

## Git workflow

`main`; commits per cluster (`status-to-glyph`, `accent-budget`,
`charts`, `agent-surfaces`); `git commit -s`.

## Steps

1. **Shared status-split helper**: `paint_status_row(glyph, glyph_role, body, base_style, …)`
   or equivalent — one place that renders `<glyph in role> <body in base>`.
   Migrate the whole-row status sites onto it (first inventory cluster).
   **Verify**: log_stream/event_stream/trace_waterfall/progress_steps tests
   show body cells without status fg.
2. **Feedback widgets** (toast rail, banner, loading view/overlay, spinner
   family, status bar, progress). **Verify**: per-widget tests; status_bar
   default render has ≤1 accent-styled slot.
3. **Accent-budget sweep** (filters, footers, meters, chips, mnemonic,
   addons, empty-state titles). **Verify**: rg spot checks per site.
4. **Action chips + precedence fixes** (action_bar patch-compose,
   button_group contrast, jump overlay, dropdown/model/question chains).
   **Verify**: a destructive dropdown row under cursor still shows Danger;
   permission Allow chip shows cursor state.
5. **Agent surfaces** (transcript, rail quiet tier, composer, review,
   message thread, token meter, semantic status). **Verify**: transcript
   selected block retains actor rail hue; composer at rest ≤2 accent regions
   (count in test render).
6. **Charts + identity + glyph hygiene + info-loss fixes**. **Verify**:
   charts render 4-series without Accent; search_results selected row shows
   highlights; diff cursor line shows word diff.
7. **Gate + migration**: add `design_gate.rs::accent_budget()` — render 6
   flagship stories (list, table, metrics tile row, transcript, composer,
   status bar) under phosphor; count cells whose fg or bg equals the Accent
   green; assert per-render budget (pick the threshold from the actual
   post-fix renders and record it). `migrations/0289-*.md` + `MIGRATING.md`.
   `mise run gate` → exit 0.

## Test plan

Per-step verifies; the accent_budget gate; update all churned snapshots.
Model helper tests on `row_chrome.rs` tests from plan 006.

## Done criteria

- [ ] `mise run gate` exits 0.
- [ ] `rg -n "Role::Accent" crates/termrock/src/widgets/ | wc -l` drops sharply; every remaining site is intent-gated (focused/live/primary) — list them in the migration.
- [ ] `design_gate.rs::accent_budget` green.
- [ ] Semantic `Running` no longer maps to `Role::Accent` anywhere: `rg -n "Running.*Accent|Accent.*Running" crates/termrock/src` → 0.
- [ ] Migration + `MIGRATING.md`; `plans/README.md` updated.

## STOP conditions

- A site's status color removal leaves two states indistinguishable in mono
  (glyphs identical) — report the glyph gap instead of re-adding color.
- `SemanticStatus::role()` change breaks public consumers beyond repair-by-
  sweep — list them.
- The accent_budget threshold can't go below 2 regions for a story because a
  widget outside this plan's scope still sprays accent — record it for the
  owning plan; don't expand scope.

## Maintenance notes

- The accent_budget number is the long-term regression guard for design-
  language law 1.1; reviewers treat raising it as a design decision.
- Patterns inherit these helpers in plan 010.
