# Plan 001: Make the underline-free interaction grammar the single binding design SoT

> **Executor instructions**: Follow this plan step by step. Run every
> verification command and confirm the expected result before moving to the
> next step. If anything in the "STOP conditions" section occurs, stop and
> report — do not improvise. When done, update the status row for this plan
> in `plans/README.md`.
>
> **Drift check (run first)**: `git diff --stat 605217aa..HEAD -- docs/design/`
> If any in-scope file changed since this plan was written, compare the
> "Current state" excerpts against the live lines before proceeding; on a
> mismatch, treat it as a STOP condition.

## Status

- **Priority**: P1
- **Effort**: M
- **Risk**: LOW
- **Depends on**: none
- **Category**: docs (design SoT alignment; prerequisite for all code plans)
- **Planned at**: commit `605217aa`, 2026-08-14

## Why this matters

Four design docs claim authority over the same paint rules and disagree about
underline. `docs/design/termrock-design-language.md` (newest, 2026-08-14)
defines an **underline-free focus grammar**; `terminal-design-system.md` and
`phosphor-obsidian-visual-direction.md` (both marked "design SoT") still
*prescribe* underline for row focus, form focus, sorted columns, mono
fallbacks, and grid cursors. Every code plan that removes interaction underline
(plans 002+) would contradict a doc marked SoT unless the docs are reconciled
first. This plan makes the grammar unambiguous in one binding place and amends
every contradicting line, so later executors and CI contract text have exactly
one rule to follow.

This is a docs-only plan. **Do not modify any `.rs` file.**

## The grammar being installed (decision record — copy into the doc verbatim where §5 policy lines are added)

Underline (`Modifier::UNDERLINED`) is allowed ONLY for:

1. **Hyperlinks in monochrome projection** — on mono/`NO_COLOR`, `Role::Link`
   keeps underline (it is the only reliable link cue without color). In color
   modes the default link affordance is `Link` color + trailing `↗`/`›`
   chevron, no underline; underline is opt-in via `LinkStyle`
   (`Color` default | `UnderlineOnHover` | `AlwaysUnderline`).
2. **Content rendering** — faithful passthrough: ANSI SGR-4 in `ansi_text`,
   OSC-8 hyperlink segments, markdown emphasis *fallback* when italics are
   unavailable, diff/word-diff only where the content itself is underlined.
3. **Cursor fallback** — the text/grid cursor is a block/reverse cell by
   default; underline-cursor is permitted only as an explicit fallback where
   reverse video is unavailable.

Underline is FORBIDDEN for: focus (container, row, field, label, control,
chrome section), selection, hover, active/current item (tab, page, step,
crumb, segment), sort indicators, severity/status, search/match highlight,
syntax classes, and button affordance.

The mono (colorless) cue ladder replacing it: **BOLD** = strong/current,
**DIM** = muted/disabled, **REVERSED** = selected row / cursor cell /
focused-chrome, **glyph prefix** (`!`, `x`, `>`, `*`) = severity/selection,
**UNDERLINED** = link only.

## Current state (verified excerpts at `605217aa`)

- `docs/design/termrock-design-language.md:5` — Status field says
  `Design direction (living). Consolidates and extends the paint audit.` §5
  (lines 251–348) already defines the underline-free grammar, five focus cues,
  per-family replacement cues, and enumerated removal sites. This doc is
  correct; it just isn't marked binding.
- `docs/design/terminal-design-system.md:3` — "Status: design SoT". Lines that
  prescribe interaction underline:
  - `:30` principle 3: "…legible under monochrome / `NO_COLOR` via glyphs, underline, reverse, bold/dim."
  - `:37` principle 10: "Focus is role/style/underline/gutter—not double-line boxes."
  - `:77` token table: `tabUnderlineFocused`, `tabUnderlineQuiet`.
  - `:145` border table: "`border.focused` | **same single box** | `borderFocused` (color/underline—not weight)".
  - `:156` focus table: "**Focus (row)** | underline on primary label OR left gutter accent | Underline / reverse cell".
  - `:158` "**Hover** | Subtle surfaceRaised / muted fg | Underline".
  - `:444` `pub focus_underline: Option<bool>,` (ListStatePatch spec).
  - `:483` `pub focus_underline: bool,` (ResolvedListRow spec).
  - `:509` "Selected + focused (list owns keys) | Gutter + underline on primary + accent gutter color".
  - `:651` mono projection: "keep **modifiers** (bold/dim/underline/reverse)".
  - `:656` mono rules: "- Focus: underline".
  - `:710` "**Focus row:** underline primary when focused+selected; gutter always if selected."
  - `:775` test-list row: `capability_mono_keeps_underline_bold`.
  - `:796` story row: "`focus/border-vs-row` | Container focus + row underline".
- `docs/design/phosphor-obsidian-visual-direction.md` — "Status: design SoT".
  Underline-prescribing lines: `:74` (audit prose, keep as history), `:110`
  ("Focus-visible = underline and/or focused border"), `:115` (mono channel
  list includes underline), `:146` ("Selected + list focused | gutter accent +
  **underline** primary"), `:181` (list mockup "underline if focused"), `:209`
  (table sorted column "underline or `↑` muted"), `:222–229` (tabs
  "underline ownership" / "active = bold + underline (or bottom rule cell)"),
  `:274` (form mockup "cursor underline"), `:282` ("focus = field underline or
  border_focused"), `:332`/`:336` (VirtualGrid "cursor cell: gutter or
  underline").
- `docs/design/component-quality-standard.md:81` — focus axis pass row:
  "Entry paints focus-visible chrome (`Role::BorderFocused` / gutter / underline)".
- `docs/design/component-anatomy-spec.md` — Tabs `:586` anatomy part
  `underline`; `:591` "focus-visible underline"; `:601` "underline focus;
  close `×`/`x`"; `:602` "active = bold + underline"; `:606` "active
  underline; narrow truncate". TextInput `:864` "focus-visible
  border/underline"; `:869` "underline invalid; reverse selection". List
  `:1115` "gutter `>` + reverse/underline". AgentWorkbench `:2703` "pane focus
  underline/border role".
- `docs/design/component-visual-richness-plan.md:60,164` — mention role name
  `TabUnderlineFocused` in de-collapse lists (keep the mentions; they name a
  current code identifier — plan 002+ renames it in code first).

Repo convention: design docs are plain Markdown under `docs/design/`; no build
step consumes them. Edits are text-only; keep each file's existing table
formatting and heading structure.

## Commands you will need

| Purpose | Command | Expected on success |
|---------|---------|---------------------|
| Locate lines | `rg -n "<pattern>" docs/design/<file>` | shows the target line |
| Verify sweep | see per-step and Done criteria `rg` commands | exact counts below |

No compile/test gate applies (docs only). Do not run `mise run gate`.

## Scope

**In scope** (the only files you may modify):
- `docs/design/termrock-design-language.md`
- `docs/design/terminal-design-system.md`
- `docs/design/phosphor-obsidian-visual-direction.md`
- `docs/design/component-quality-standard.md`
- `docs/design/component-anatomy-spec.md`

**Out of scope** (do NOT touch):
- Any `.rs` file, any lookbook/story source, `migrations/`, `MIGRATING.md`
  (doc amendment is not a public API break — no migration file).
- `docs/design/component-visual-richness-plan.md` and
  `component-prompt-library.md` — their `TabUnderlineFocused` mentions name a
  live code identifier; renaming happens with the code in a later plan.
- Historical audit prose that *describes* today's defects (e.g.
  phosphor-obsidian `:74`, design-language §2 item 6) — history stays.

## Git workflow

- Work directly on `main` (repo law: no feature branches). Single commit,
  Conventional Commits + DCO sign-off, e.g.:
  `git commit -s -m "docs(design): bind underline-free interaction grammar across SoT docs"`
- Do NOT push unless the operator instructed it.

## Steps

### Step 1: Promote `termrock-design-language.md` to binding and add the three missing clauses

1. In the header table (line ~5), change the Status cell to:
   `**Binding design SoT for interaction styling & focus grammar** (living). Consolidates and extends the paint audit. On conflict about focus/selection/active/underline paint, this file wins; terminal-design-system.md stays SoT for token taxonomy; phosphor-obsidian-visual-direction.md stays SoT for the phosphor palette values.`
2. At the end of §5.7 (after line ~341), append a short subsection `### 5.9
   Grammar clauses (binding)` containing the three-clause allowed list and the
   forbidden list and the mono cue ladder from "The grammar being installed"
   above, verbatim.
3. In §5.7, adjust the link ruling to state explicitly: monochrome projection
   keeps `Role::Link` underlined (only mono underline besides content).

**Verify**: `rg -n "Binding design SoT for interaction styling" docs/design/termrock-design-language.md` → 1 match; `rg -n "5.9 Grammar clauses" docs/design/termrock-design-language.md` → 1 match.

### Step 2: Amend `terminal-design-system.md`

Edit exactly these lines (find by content, not line number, if drifted):

| Where | Change |
|-------|--------|
| `:3` Status | append: `Interaction underline rules are superseded by termrock-design-language.md §5 (binding).` |
| `:30` principle 3 | `via glyphs, underline, reverse, bold/dim` → `via glyphs, reverse, bold/dim (underline = links only; see termrock-design-language.md §5)` |
| `:37` principle 10 | `Focus is role/style/underline/gutter` → `Focus is role/style/border/gutter` |
| `:77` Tab tokens | `tabUnderlineFocused`, `tabUnderlineQuiet` → `tabAccent`, `tabAccentQuiet` and add note `(rule-row cue, opt-in, off by default)` |
| `:145` | `(color/underline—not weight)` → `(color—not weight, never underline)` |
| `:156` Focus (row) | `underline on primary label OR left gutter accent` → `left gutter accent + bold primary`; mono cell `Underline / reverse cell` → `Reverse cell` |
| `:158` Hover | mono cell `Underline` → `Reverse or dim` |
| `:444` | delete the `focus_underline: Option<bool>,` spec line (leave a note `// removed: underline is not a focus cue`) |
| `:483` | same for `focus_underline: bool,` |
| `:509` | `Gutter + underline on primary + accent gutter color` → `Accent gutter + bold primary (no underline)` |
| `:651` | `keep **modifiers** (bold/dim/underline/reverse)` → `keep **modifiers** (bold/dim/reverse; underline only on links)` |
| `:656` | `- Focus: underline` → `- Focus: reverse cell or gutter '>'` |
| `:775` | test name `capability_mono_keeps_underline_bold` → `capability_mono_keeps_bold_reverse` |
| `:796` | `Container focus + row underline` → `Container focus + row gutter` |

**Verify**: `rg -c "underline" docs/design/terminal-design-system.md` → count drops from 12 to ≤4, and `rg -n "underline" docs/design/terminal-design-system.md` shows only: the principle-3 "links only" line, the Status supersession note, and lines that say "never underline"/"no underline"/"underline only on links".

### Step 3: Amend `phosphor-obsidian-visual-direction.md`

| Where | Change |
|-------|--------|
| `:3` Status | append supersession note as in Step 2 |
| `:110` | `Focus-visible = underline and/or focused border on the **owner** container.` → `Focus-visible = focused border on the **owner** container (never underline).` |
| `:115` | drop `underline,` from the channel list |
| `:146` | `gutter accent + **underline** primary; no full neon fill` → `gutter accent + **bold** primary; no full neon fill, no underline` |
| `:181` | `gutter + bold/underline if focused` → `gutter + bold if focused` |
| `:209` | `sorted column: underline or `↑` muted` → `sorted column: `↑` muted` |
| `:222–229` tabs | rewrite the After block + Spec: active = bold Accent label + leading `▸` marker; optional opt-in bottom rule line of `─` border cells (off by default); never SGR underline on the label |
| `:274` | `cursor underline` → `block cursor` |
| `:282` | `focus = field underline or border_focused on active field only` → `focus = border_focused on the active field only` |
| `:332` | `cursor cell: gutter or underline, not full neon cell` → `cursor cell: reverse cell + optional gutter, not full neon cell` |
| `:336` | `cursor = underline or reverse one cell` → `cursor = reverse one cell` |

Leave `:74` (audit history) untouched.

**Verify**: `rg -n "underline" docs/design/phosphor-obsidian-visual-direction.md` → remaining matches are only `:74`-area audit history, the supersession note, and "never underline"/"no underline" phrasings.

### Step 4: Amend `component-quality-standard.md:81`

`Entry paints focus-visible chrome (`Role::BorderFocused` / gutter / underline)` →
`Entry paints focus-visible chrome (`Role::BorderFocused` / gutter / reverse — never underline; termrock-design-language.md §5)`

**Verify**: `rg -n "never underline" docs/design/component-quality-standard.md` → 1 match; `rg -n "gutter / underline" docs/design/component-quality-standard.md` → 0 matches.

### Step 5: Amend `component-anatomy-spec.md` (five component specs)

| Where | Change |
|-------|--------|
| Tabs `:586` | anatomy part `underline` → `active_marker` |
| Tabs `:591` | `focus-visible underline` → `focus-visible marker + reverse` |
| Tabs `:601` | `underline focus; close ×/x` → `reverse focus; close ×/x` |
| Tabs `:602` | `active = bold + underline` → `active = bold + ▸ marker` |
| Tabs `:606` | `active underline; narrow truncate` → `active marker; narrow truncate` |
| TextInput `:864` | `focus-visible border/underline` → `focus-visible border` |
| TextInput `:869` | `underline invalid; reverse selection` → `! prefix + bold invalid; reverse selection` |
| List `:1115` | `gutter > + reverse/underline` → `gutter > + reverse` |
| AgentWorkbench `:2703` | `pane focus underline/border role` → `pane focus border role` |

**Verify**: `rg -n "underline" docs/design/component-anatomy-spec.md` → 1 remaining match maximum (Diagnostic/CodeFrame `:1888` "underlines `^`/`-`" — caret glyph rows, content-legit, keep).

### Step 6: Cross-check no doc now contradicts the grammar

**Verify**: `rg -in "focus.{0,40}underline|underline.{0,40}focus" docs/design/ --glob '!competitive-tui-research.md' --glob '!experience-research-2026.md' --glob '!component-prompt-library.md' --glob '!*coverage*.md'` → every match is either a "never/no underline" phrasing, §2/§5 of termrock-design-language.md (defect description / removal instructions), or phosphor-obsidian `:74` history.

## Test plan

Docs only — verification is the `rg` gates above. No unit tests.

## Done criteria

- [ ] All Step 1–6 verify commands pass with the stated counts.
- [ ] `git diff --stat` touches only the five in-scope files.
- [ ] `git status` shows no other modified files.
- [ ] `plans/README.md` status row updated.

## STOP conditions

- Any "Current state" excerpt no longer matches the live doc line (drift).
- A line to amend appears more than once in a file and the plan's table does
  not disambiguate which occurrence (report both line numbers).
- You find an additional doc line that *prescribes* interaction underline and
  is not listed here — report it; do not silently expand scope.

## Maintenance notes

- Plans 002+ implement the code side (recipe flag removal, widget sweeps,
  Role rename `TabUnderlineFocused` → `TabAccent`). When those land, the code
  identifiers mentioned in `component-visual-richness-plan.md:60,164` get
  renamed with them.
- Reviewers should check that the amended lines keep table column counts
  intact (Markdown tables break silently).
