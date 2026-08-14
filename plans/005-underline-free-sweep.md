# Plan 005: Underline-free interaction sweep — remove every focus/selection/active underline, keep content + links

> **Executor instructions**: Follow this plan step by step. Run every
> verification command and confirm the expected result before moving to the
> next step. If anything in the "STOP conditions" section occurs, stop and
> report — do not improvise. When done, update the status row for this plan
> in `plans/README.md`.
>
> **Drift check (run first)**: `git diff --stat 605217aa..HEAD -- crates/termrock/src/widgets/ crates/termrock/src/style/`
> Plans 002-004 are expected to have landed (that IS drift from `605217aa`).
> For every site this plan edits, first re-locate it with the given `rg`
> pattern; if a listed site no longer matches its description, skip it and
> record it in your report rather than improvising.

## Status

- **Priority**: P1 (the user's headline complaint)
- **Effort**: L
- **Risk**: MED (wide snapshot churn; every change is a deletion + already-present secondary cue)
- **Depends on**: plans/001 (doc grammar), plans/004 (recipe field removed, cue vocabulary)
- **Category**: design / tech-debt
- **Planned at**: commit `605217aa`, 2026-08-14

## Why this matters

`Modifier::UNDERLINED` appears at ~86 sites across 44 widget files meaning
focus, selection, hover, current-item, sort, severity, syntax-class, match
highlight, and button affordance — while `Role::Link` itself is NOT underlined
in any color palette. Underline currently means everything except "link".
The binding grammar (`docs/design/termrock-design-language.md` §5, made
binding by plan 001): **a border or a gutter or a glyph — never an underline**;
underline survives only in content rendering, monochrome links, and as an
explicit cursor fallback.

## The replacement rules (apply mechanically)

| Old meaning | Replacement |
|---|---|
| focus on label/control/chrome row | delete the underline; the site already switches to `Role::Focus`/`TextStrong` — keep that. If focus becomes invisible (no other cue changes), use `REVERSED` on the mark/glyph cell, not the label |
| hover | delete; hover = `HoverTint` wash where a rect exists, else nothing |
| selection (row/block) | gutter glyph + `TextStrong` (+ `SelectionTint` bg where the widget already paints row rects) |
| current item (tab/page/step/crumb/segment) | `BOLD` + `TextStrong`/`Accent` fg (already present at most sites — pure deletion) |
| cell cursor (table/grid/otp) | `REVERSED` on the one cell |
| severity / mono severity | glyph prefix (`!`, `x`) + `BOLD`; never underline |
| search/match highlight | `BOLD` + accent fg; focused match = `REVERSED`; mono match = `REVERSED` |
| syntax-class / internal type tag | real enum (Step 7) |
| emphasis (markdown `*em*`) | `ITALIC`; underline only under a no-italics capability flag (Step 6) |
| link | `LinkStyle` policy (Step 8); mono palettes keep underline |

## Current state (mechanism anchors — verified first-hand at `605217aa`)

- `widgets/list.rs:1350-1355` + `widgets/tree.rs:1142-1153` — row focus/
  selection/hover underline (removed by plan 004 Step 2; verify gone).
- `widgets/primitives.rs:678` — `let surface = accepts_input() && !disabled
  && !loading` gates underline on EVERY enabled Outline/Quiet/Secondary
  button (`:699,:710`), `armed` (`:715`), IconButton (`:1291-1295`);
  `Destructive`+mono (`:705`). `ButtonVariant::Link` (`:695`) is the only
  legit arm.
- `widgets/text.rs:530-537` — `TextEmphasis::Emphasis` adds
  `ITALIC | UNDERLINED` unconditionally; `Code` adds bare `UNDERLINED`;
  `:543-549` `span.highlight` = accent fg + `UNDERLINED | BOLD`.
- `widgets/tabs.rs:1175-1179` — label `UNDERLINED | REVERSED` when
  focused-not-selected; `UNDERLINED` on hover. The separate rule row at
  `:1229-1240` is the legit form and stays.
- `widgets/markdown.rs:1626-1639` — `underline_row()` ORs `UNDERLINED` +
  `Role::Selection` fg into every cell; callers `:857,:967,:1024,:1887`.
- `widgets/key_value_list.rs:816-822` — full-row selected underline loop;
  `:843-845` href underline is LEGIT (until Step 8 routes it via LinkStyle).
- `widgets/text_input.rs:1014-1026` — focused label = `Role::Focus` +
  `UNDERLINED` (the pattern copied by 8 sibling inputs).
- `widgets/controls.rs:468-482` — Checkbox label: focused → `Role::Focus` +
  `UNDERLINED`; hovered → `UNDERLINED` (pattern repeated for Radio/Switch/legend).

## Site inventory (leads verified by audit; re-locate each with rg before editing)

Enumerate live sites first: `rg -n "UNDERLINED|underlined\(" crates/termrock/src crates/termrock-lookbook/src -g '*.rs'`

DELETE-underline sites (mechanism → files):
1. Input field labels: `text_input.rs:1024`, `number_input.rs:1043`,
   `search_input.rs:809`, `select.rs:830`, `combobox.rs:950`,
   `multi_select.rs:879`, `token_field.rs:1004`, `path_input.rs:1013`,
   `slider.rs:738` (+ `path_input.rs:1203` destructive-field underline →
   danger border/glyph per rules).
2. Controls: `controls.rs:438,474,480,1141,1292,1308,2052,2089,2095`;
   `toggle.rs:479,481,489`; `segmented_control.rs:524,526,543`.
3. Buttons: `primitives.rs:699,705,710,715,1292,1294` + docs `:286,:376,:788`.
4. Tables/cells: `table.rs:1352`, `data_table.rs:1534`, `tree_table.rs:1370`
   → `REVERSED` cell cursor.
5. Navigation/current: `breadcrumbs.rs:870-872`, `stepper.rs:862,882`,
   `pagination.rs:927`, `tabs.rs:1176,1178`, `fullscreen_viewer.rs:1181,1211,1305`,
   `callout.rs:461`, `badge.rs:461`, `tag_chip.rs:364,849`, `charts.rs:2462`,
   `date_time_picker.rs:2144`.
6. Row/block selection: `markdown.rs` `underline_row` + 4 callers →
   `SelectionTint` bg wash preserving cell modifiers + left gutter glyph;
   `key_value_list.rs:816-822` → same treatment.
7. Match/highlight: `text.rs:544-549` (highlight → accent fg + BOLD; drop
   underline), `highlighted_text.rs:953-960` (mono → `REVERSED`; focused
   match → `REVERSED`; update test `:1095-1102`), `code_block.rs:889`
   Search arm (→ `Role::Warning`? NO — use `HoverTint` bg + BOLD),
   `code_block.rs:901` diff fallback (→ BOLD, matching `diff.rs:1571-1574`
   colorless branch), `diff.rs:1581,1584` word-diff color path (→ rely on
   DiffAdded/DiffRemoved bg tints + BOLD).
8. Mono severity/status: `identity.rs:563,569,590`, `keyboard_help.rs:1203`,
   `stepper.rs:862`, `code_block.rs:1683` mono SyntaxString → apply the mono
   cue ladder (BOLD/DIM/REVERSED/glyph; underline = link only).
9. OTP cursor: `input_otp.rs:406` → `REVERSED` cell (drop `UNDERLINED`,
   keep at most one of Accent/BOLD).
10. Dead blocks: `input_group.rs:272-275`, `code_block.rs:906-909`,
    `highlighted_text.rs:963` → delete blocks + stale comments (+ now-unused
    imports).
11. Link-focus underline: `link.rs:424,773` — remove the `|| state.focused`
    trigger (BOLD already carries focus); variant behavior handled in Step 8.
12. Lookbook copy: `crates/termrock-lookbook/src/stories.rs:1584,2366,4639,5295,6517,6652,6816`
    — update story descriptions that advertise underline cues.

KEEP (whitelist — do not touch): `ansi_text.rs` SGR-4/OSC-8 passthrough;
`diagnostic.rs` caret rows (glyph rows, not SGR); `primitives.rs:292,695`
Link button variant (NOTE: the tabs `━` rule row is deleted by plan 002 —
if it still exists when this plan runs, delete it here);
`citation.rs:741-749` (link affordance; hover/visited variants);
`markdown.rs:1654` inline links; `key_value_list.rs:843-845` href;
`style/mod.rs` mono-palette Link underline (`:741-742`);
`hint_bar.rs:220-221` + `style/motion.rs:75-76` (underline_color remaps only);
lookbook `frame.rs` serialization; `code_block.rs:889` `Diagnostic` arm ONLY
(squiggle substitute — see Step 6 note).

## Commands

| Purpose | Command | Expected |
|---------|---------|----------|
| Enumerate | `rg -n "UNDERLINED" crates/termrock/src -g '*.rs'` | shrinking list |
| Fast gate | `mise run check` | exit 0 |
| Full gate | `mise run gate` | exit 0 |

## Scope

**In scope**: the widget files listed above; `crates/termrock/src/style/mod.rs`
(link palette tweaks in Step 8 only); `crates/termrock-lookbook/src/stories.rs`
(copy + assertions only); tests asserting `UNDERLINED`
(`tests/tree.rs:113,300,316`, `link.rs:1008-1014`, `highlighted_text.rs:1095`,
others found by `rg -n "UNDERLINED" crates/termrock -g '*test*' -g '*/tests/*'`);
`migrations/0284-*.md` + `MIGRATING.md`.

**Out of scope**: selection-chrome structural unification (plan 006), status
color discipline (plan 007), input wells/focus cue (plan 008), overlay chrome
(plan 009). Where this plan deletes an underline whose replacement belongs to
those plans, the interim state must still satisfy "state visibly
distinguishable" (the rules table's minimal cue), never "cue removed, nothing
added".

## Git workflow

`main`; ideally 3 commits (mechanical deletes / match-highlight+emphasis /
link policy + token-kind), each `git commit -s`, Conventional Commits
(`refactor(widgets)!: …`).

## Steps

### Step 1: Verify plan-004 preconditions

`rg -n "show_focus_underline" crates/` → 0. `rg -n "UNDERLINED" crates/termrock/src/widgets/list.rs crates/termrock/src/widgets/tree.rs` → 0. If not, STOP (plan 004 incomplete).

### Step 2: Mechanical deletions — clusters 1,2,3,5,9,10,11

Apply the replacement rules table. For buttons (`primitives.rs`): underline
survives ONLY in the `ButtonVariant::Link` arm; `armed` → `BOLD` (+
`REVERSED` under mono); IconButton `toggled` → `REVERSED` face, enabled →
no modifier; fix the three doc comments. For each edited file run the
widget's tests before moving on.

**Verify** after cluster: `rg -n "UNDERLINED" <file>` → only whitelisted lines remain; `mise run check` at the end of the step.

### Step 3: Row/block selection — cluster 6

Replace `markdown.rs::underline_row` body: apply `Role::SelectionTint` bg to
each cell (preserving fg + modifiers) and paint `glyphs.selection_gutter()`
in `Role::Accent` at the row's left margin column; rename fn to
`select_row`. Same for `key_value_list.rs:816-822` loop.

**Verify**: `rg -n "underline_row" crates/` → 0; markdown/kv-list tests updated + green.

### Step 4: Tables cell cursor — cluster 4

`table.rs:1352`, `tree_table.rs:1370`, `data_table.rs:1534`: replace
`UNDERLINED` (and `UNDERLINED|BOLD`) with `REVERSED`. data_table edit-draft
inherits `REVERSED` — verify the draft text is legible (fg/bg swap of the
cell style).

**Verify**: table/tree_table/data_table tests updated + green.

### Step 5: Match highlight — cluster 7

Per rules table. `text.rs` `span.highlight`: accent fg + `BOLD` only.
`highlighted_text.rs`: mono → `REVERSED`; `MatchKind::Focused` → `REVERSED`
(color path too); adjust `no_color_underline_matches` test name/body.
`code_block.rs:889`: split arms — `Search` → `HoverTint` bg + `BOLD`;
`Diagnostic` → keep `UNDERLINED` and add a `// sanctioned: squiggle
substitute (design-language §5.9)` comment. `code_block.rs:901`, `diff.rs:1581,1584`
per rules.

Also append one sentence to `docs/design/termrock-design-language.md` §5.9
(added by plan 001): "Diagnostic-span underline inside code blocks is
sanctioned content annotation (squiggle substitute), matching the caret-row
semantics of `Diagnostic`."

**Verify**: `cargo nextest run -p termrock highlighted_text code_block diff text::` green.

### Step 6: Emphasis fallback honesty

`text.rs:530-537`: `Emphasis` emits `ITALIC` only; add
`DesignSystem`-sourced fallback — if a no-italics flag is available on
`DesignSystem`/capability, emit `UNDERLINED` instead; if no such flag exists
yet, emit `ITALIC` only and record the missing capability flag in the
migration + README follow-ups (do NOT invent a new public field here).
`Code` → drop underline; use `Role::Info` fg patch (keep "no filled
background" promise at `:131`).

**Verify**: text/markdown tests green; markdown emphasis snapshot no longer underlined.

### Step 7: Kill the underline type-tag in the token highlighter

`code_block.rs:1587-1594` + `:1608-1626` + decoder `:1745-1765`: make the
internal tokenizer return a token kind (private enum or `Role`) instead of
encoding "string" as `Green+UNDERLINED`; `syntax_role_style` stays the only
presentation point. If `highlight_line`'s public signature must change,
that's allowed (pre-1.0, breaking free) — document in the migration.

**Verify**: `rg -n "contains\(Modifier::UNDERLINED\)" crates/termrock/src/widgets/code_block.rs` → 0; code_block tests green.

### Step 8: LinkStyle policy

In `link.rs`: add `LinkStyle { Color (default), UnderlineOnHover, AlwaysUnderline }`
per design-language §5.7. Default rendering: `Link` color + trailing `↗`/`›`
chevron glyph (from glyph catalog), no underline; hover = `LinkHover` color;
variants map onto the policy. Keep `LinkVariant::Underline` working as
`AlwaysUnderline` or fold it into `LinkStyle` (one enum — prefer the fold,
document in migration). Mono: link underline stays (palette-driven).
`citation.rs`: keep underline (it's a link) but route through the same
policy so consumers can opt colorless citations out.

**Verify**: link/citation tests updated + green.

### Step 9: Global verification + migration

- `rg -n "UNDERLINED|underlined\(" crates/termrock/src -g '*.rs'` — every
  remaining match is on the KEEP whitelist. Paste the final list into the
  migration file.
- `migrations/0285-*.md` (next free): the sweep — every removed site grouped
  by mechanism, replacement cue per group, LinkStyle introduction,
  highlight/emphasis/token-kind changes. `MIGRATING.md` row.
- `mise run gate` → exit 0.

## Test plan

Update every test asserting `UNDERLINED` on a non-whitelisted surface (list
in Scope); add: `design_gate.rs` gains
`fn interaction_underline_is_dead()` — source scan over `widgets/` asserting
`UNDERLINED` appears only in whitelisted files (encode the whitelist in the
test).

## Done criteria

- [ ] `mise run gate` exits 0.
- [ ] `rg -n "UNDERLINED" crates/termrock/src/widgets/ | rg -v "link|citation|ansi_text|code_block|kbd|tabs"` → manually confirm each remaining line is whitelisted; the design_gate source-scan test encodes it.
- [ ] Focused form controls, selected rows, active tabs/pages/steps/crumbs, sorted headers: zero SGR underline (spot-render via lookbook stories).
- [ ] Migration file + `MIGRATING.md` row.
- [ ] `plans/README.md` updated.

## STOP conditions

- Deleting an underline leaves two states pixel-identical (e.g.
  `segmented_control` focused-selected vs focused) and no rule-table cue
  applies cleanly — report the state pair instead of inventing a new cue.
- A whitelisted file's underline turns out to be an interaction cue on
  inspection — report, don't guess.
- `tabs.rs` rule row regressed by plan 002's role rename (paints nothing) —
  report.

## Maintenance notes

- The no-italics capability flag (Step 6) is deliberately deferred.
- design_gate whitelist is the regression barrier; reviewers should reject
  any PR that grows it.
