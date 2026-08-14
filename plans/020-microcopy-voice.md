# Plan 020: One voice — the microcopy standard (case, keys, ellipsis, error copy)

> **Executor instructions**: Follow this plan step by step. Run every
> verification command. STOP conditions binding. Update `plans/README.md`.
>
> **Drift check (run first)**: `git diff --stat d09bd2fe..HEAD -- crates/termrock/src`
> Re-locate every cited string with `rg` before editing (strings move).

## Status

- **Priority**: P2 (cheap, wide, designer-visible)
- **Effort**: M
- **Risk**: LOW (string-only changes + two gates; snapshot churn)
- **Depends on**: none hard; run before/alongside 010/013 so sweeps stop
  preserving whichever voice they encounter
- **Category**: design / docs
- **Planned at**: commit `d09bd2fe`, 2026-08-14

## Why this matters

Two microcopy systems coexist with no arbitration: a terse terminal voice
(lowercase labels, fragment errors, bare key names) and an app voice
(Title-case labels, full sentences, `Ctrl+S`). The split cuts through both
`widgets/` and `patterns/` and sometimes one string. No design doc rules on
copy, so every other plan will faithfully preserve the inconsistency. Case,
punctuation, and key notation are hierarchy channels on a cell grid —
designers spend them deliberately.

## The standard (decision record — copy verbatim into the law doc)

1. **Labels/buttons/titles: sentence case.** `Cancel`, `Sign in`,
   `Git output`, `Search settings…`. Never ALL-CAPS as structure (uppercase
   allowed ONLY where a SoT names it, e.g. sidebar section headers).
   Panel titles capitalize the first word: `Procs` not `procs`.
2. **Hints: lowercase action verbs, keys as chips.** `[esc] cancel ·
   [enter] open`. `·` separates hint pairs only — never key from label.
   One verb per key repo-wide where meaning matches: `esc` = cancel
   (dismiss one layer), `enter` = confirm/open. No trailing periods.
3. **Key notation: one system.** Modifier-dash, lowercase key: `C-s`,
   `S-tab`, `M-x`; bare keys spelled `esc enter tab space ↑↓←→` (ASCII
   `up/down/left/right`). `Ctrl+S`/`⌘K` forms are forbidden in painted
   strings (docs prose may spell "Ctrl+S" when explaining). Chord separator
   inside a keycap = space (audit F4).
4. **Ellipsis: `…` gated to `...` via the glyph catalog** — never a bare
   literal in painted strings; both modes width-checked at the call site.
5. **Error copy: fragment + cause + one recovery.** Pattern:
   `<what failed> — <recovery>` in sentence case, no terminal period for
   one-liners: `Could not reach the API — check connectivity and retry`.
   The `error · message` / `error: message` prefix idioms die; severity is
   carried by the glyph/role, not the word "error".
6. **Placeholders: sentence case, conversational, ellipsis:**
   `Search settings…`, `Filter projects…`.
7. **Running verbs: lowercase gerund** beside the spinner: `⠹ running
   tests`, `streaming…` (matches design-language §5.10).
8. **OK is `OK`** (never `Ok`).

## Current state (verified at `d09bd2fe`; leads for the sweep)

- Ellipsis drift: `widgets/completion_menu.rs:499` `"Loading…"` vs
  `:1154,:1418` `"Loading..."`; `widgets/object_inspector.rs:1420/1422`
  both marks in adjacent arms; `quick_open.rs:1588` `"[...] searching"`,
  `command_palette.rs:1520` `"[...] loading"`; `tabs.rs:1024` pads `" … "`
  but not `"..."`. Correct gated idiom exists at `data_table.rs:1182`,
  `tree_table.rs:1043`, `menu_bar.rs:1567`, `pagination.rs:939`,
  `diagnostic.rs:731,861`.
- Case drift: `working_state_card.rs:455` `"Cancel"` vs
  `file_manager.rs:320` `"cancel"` (+ `hint_bar.rs:329,358` and ~6 more);
  titles `"Files"`/`"Git output"` vs `"procs"`/`"catalog"`/`"ops"`/`"sql"`;
  placeholders `"Search settings…"` (`settings_screen.rs:713`) vs
  `"filter projects…"` (`project_launcher.rs:1375,1380`).
- Key notation: `"C-s"` (`toolbar.rs:1198`, `dropdown_menu.rs:1557`, kbd)
  vs `"Ctrl+S save"` (`settings_screen.rs:800,1043-1047` + ≥40 occurrences
  across ~10 files) vs `"⌘K"` (`list.rs:1968`).
- Hint verb drift: `esc cancel/clear/demote/close/dismiss` across
  loading_overlay/empty_state/fullscreen_viewer/dialog/popover/drawer.
- Error voice: `error_state.rs:1163` full sentence with period vs
  `search_results.rs:190` `format!("error · {message}")` vs
  `project_launcher.rs:1468` `format!("error: {err}")` vs
  `connection_manager.rs:2359` `"cannot connect"`.
- `"Ok"` in `notification_center.rs:1436` (test fixture) vs `"OK"` in
  dialog fixtures.
- Mask placeholder `"••••••••"` at `auth_entry.rs:813,830` — must be the
  catalog mask `●`/`*` (plan 003 unifies the glyph; this plan fixes the
  literal).

## Scope

**In scope**: painted string literals + hint/label constants across
`crates/termrock/src/{widgets,patterns}`; `docs/design/web-premium-tui-law.md`
(§4 gains the copy clause); `design_gate.rs`; test fixtures whose strings
are asserted (update alongside). **Out of scope**: docs-site MDX prose;
lookbook story descriptions (sweep only where they assert painted output);
localization (none exists).

## Steps

### Step 1: Law + helper

Append the standard above as one clause block to
`docs/design/web-premium-tui-law.md` §4 (rule 16, "One voice"). Add
`GlyphSet::ellipsis()` (or confirm one exists) so `…/...` always resolves
via catalog; add `fmt_key(chord) -> String` helper (or extend the kbd
recipe) as the single chord-formatter.

**Verify**: `rg -n "One voice" docs/design/web-premium-tui-law.md` → 1.

### Step 2: Mechanical sweeps (one commit each)

a. Ellipsis: `rg -n '"[^"]*\.\.\.[^"]*"' crates/termrock/src -g '*.rs'` —
   every painted literal → catalog resolution; fix `[...]` idioms to
   spinner-verb form (`⠹ searching` per plan 014 channel or static gated
   `…` pre-014); equalize tabs overflow padding both modes.
b. Key notation: `rg -n '"[^"]*(Ctrl|Cmd|⌘|Alt)\+' crates/termrock/src -g '*.rs'`
   → `fmt_key` / `C-…` form.
c. Labels/titles/placeholders case per standard;
   `rg -n '"(cancel|confirm|delete|save|apply)"' ` for label constants;
   panel title literals first-word capitalized.
d. Hint verbs: normalize `esc` to `cancel` (transient/loading) or the
   layer-true verb where cancel is wrong (report oddballs); strip trailing
   periods in hints.
e. Error strings: kill `error ·`/`error:` prefixes (severity via
   glyph/role); reshape to `<what> — <recovery>` where a recovery exists;
   `Ok`→`OK`; auth mask literal → catalog mask.

**Verify** after each: targeted `rg` returns 0 (or only whitelisted docs
comments); `mise run check` green (snapshot updates included).

### Step 3: Gates

`design_gate.rs`: `no_bare_ellipsis_in_paint` (source scan for `...` inside
string literals in widgets/patterns, whitelist comments/tests),
`one_chord_notation` (scan for `Ctrl+`/`⌘` in painted literals). Wire the
scans with the same mechanism as existing source-scan gates.

**Verify**: gates green; corrupting one string fails locally.

### Step 4: Migration note

Copy changes are visible defaults → one `migrations/` file (next free)
listing the standard + notable renames (`Ctrl+S`→`C-s` chips etc.),
`MIGRATING.md` row.

## Done criteria

- [ ] `mise run gate` exits 0; both new gates green.
- [ ] Law doc carries rule 16.
- [ ] `rg -n '"(Ctrl|Cmd)\+' crates/termrock/src -g '*.rs'` → 0 painted sites.
- [ ] `rg -n '\.\.\.' crates/termrock/src/widgets crates/termrock/src/patterns -g '*.rs' | rg '"'` → only gated/whitelisted.
- [ ] Migration + `MIGRATING.md`; README row updated.

## STOP conditions

- A key-notation change collides with the kbd-recipe work (plan 015 Step 2)
  mid-flight — coordinate: the RECIPE owns rendering; this plan only fixes
  raw literals; if 015 landed, route through its recipe instead of strings.
- A hint verb can't normalize without lying about behavior (esc does
  something layer-specific) — keep the truthful verb, list it in the
  migration as a sanctioned exception.
- Fixture updates exceed ~200 assertions in one commit — split by widget
  family; don't bulk-regex asserted strings blind.

## Maintenance notes

- New strings follow rule 16; reviewers reject `Ctrl+`/bare `...`/`error:`
  on sight — the gates catch the first two mechanically.
- If localization ever lands, rule 16 becomes the source-language style
  guide.
