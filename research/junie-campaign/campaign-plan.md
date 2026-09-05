# Junie-TUI Fidelity Campaign — Orchestration Plan

Date: 2026-09-02. Branch: `experimental/component-catalog-docs-2026-09-02`.
Canonical reference: `/Users/donbeave/Projects/terminal-components-claude` (crate `junie-tui`).
Target: `/Users/donbeave/Projects/tailrocks/termrock`.

## Mission

TermRock's entire TUI design system must render and behave one-to-one like
junie-tui. Direct matches are copied exactly; widgets without a reference are
derived from the same tokens/patterns. "Similar" = failure.

## Source of truth (priority order)

1. Rendered behavior of junie-tui (bins + `shots/` artifacts: `.txt` = exact
   text grid, `.ansi` = colored escape stream, `.png` raster).
2. FRESH renders under `verify/junie/reference/scenes/` (committed `shots/`
   are STALE vs reference HEAD `e43cf67` — newer pages exist; trust fresh
   captures over the committed `shots/` directory).
3. Source (`src/theme.rs`, `src/widgets/*`, `src/core/*`, `src/ui/*`, bins).
4. `DESIGN.md`.
5. Repeated patterns.
6. Professional judgment (last resort only).

## Extracted canonical facts (verified by primary agent from source)

### Palette (TrueColor; see theme.rs `palette` mod)

| Token | Hex | | Token | Hex |
|---|---|---|---|---|
| canvas | #000000 | | accent | #48e054 |
| surface | #111111 | | accent_hover | #3ab343 |
| surface_elevated | #18181b | | accent_pressed | #2b8632 |
| surface_overlay | #27272a | | accent_bg | #0f2e13 |
| field | #1e1e22 | | accent_bg_subtle | #0a1c0c (dormant) |
| field_hover | #232328 | | on_accent | #19191c |
| popover | #3f3f46 | | focus = accent | |
| border_subtle | #262626 | | error | #e44545 |
| border_strong | #4d4d4d | | error_bg | #2e0f0f (dormant) |
| text_primary | #ffffff | | warning | #f59e09 |
| text_secondary | #b3b3b3 | | success = accent | |
| text_muted | #808080 | | info | #8787ff (dormant) |
| text_faint | #4d4d4d | | disabled = text_faint | |
| text_ghost | #262626 | | | |

Fallback: 256 = nearest xterm; 16 = named (accent LightGreen, error LightRed);
mono = 4-bucket gray. `NO_COLOR` = mono. Both bins accept `--color`.

### State grammar (theme.rs resolvers)

- `row(state, bg)`: selected+focused → `accent_bg` tint; hover → `lift(bg)`
  (never hue); focused → BOLD; pressed → inverted (canvas on text_primary);
  error → error fg; busy → secondary fg; disabled → faint, no hover.
- `lift(bg)`: canvas→elevated; surface/elevated→overlay; field→field_hover;
  else popover. Hover = exactly one plane up.
- Focus = `▎` gutter glyph (focus color) + BOLD text. Focus bar color =
  accent; hidden when unfocused (fg = bg).
- Buttons: primary on-accent/accent bold (hover accent_hover, pressed
  accent_pressed); secondary/toggle on surface_overlay (hover popover, pressed
  inverted); subtle text_secondary on container (hover lift+primary); danger
  error on surface_overlay (pressed text_primary on error). Pressed flash
  140 ms. Disabled ignores hover.
- Editing = field plane unchanged + accent underline + hardware cursor +
  ` EDIT ` badge; invalid edit underline turns error.
- Backdrop (modal): ladder-walk per cell (primary/accent/error/warning text →
  muted; secondary/on_accent → faint; else ghost; field bg → elevated;
  colored fill → overlay; modifiers cleared). Footer excluded.
- Selection (text/range) = text_primary on popover.
- Scrollbar: track border_subtle `│`, thumb `┃` muted / hovered secondary /
  focused primary. Only on overflow.
- Syntax: keyword bold primary; ident/plain primary; str+number secondary;
  operator+punct muted; comment faint italic. No hue.

### Spacing (DESIGN.md Layout)

gutter 1, inline 1, gap 2, column-gap 2, form-gap 4, card-inset 2,
frame-inset 3 (border+2), dialog-inset 3 (h) / 2 rows (v), tree-indent 2,
field-height 3, tabs-height 2, min 72×20. Row anatomy: `▎` col 0, marker slot
col 1, content col 3. Buttons `label+2` (+2 with marker). Section break = 1
blank row. Two-cell column gaps in tables/grids (no column walls, no row
boxes). Cards = borderless filled surface.

### Glyphs (complete table in DESIGN.md Shapes)

`▎ › ✓ • + − ! ▲ ▸ ▾ ▴ ▾ ∇ ▪ → ↓ ‹ › … × ● ○ [✓] [ ] ◆ ◇ ─ ━ │ ┃ ╭╮╰╯` and
ten-frame braille spinner `⠋⠙⠹⠸⠼⠴⠦⠧⠇⠏` @80 ms. Sentence case everywhere;
only uppercase = `EDIT` badge. ` · ` clause join, ` › ` hierarchy, en dash
ranges, `…` truncation + middle truncation for identifiers.

### Interaction grammar

Tab/Shift+Tab reading-order ring (rebuilt per frame in render order);
composite widgets = one stop with internal cursor. Arrows+hjkl, PgUp/Dn,
Home/End, g/G. Enter/Space activate. Esc ladder per app. `0` nav, `[` `]`
switch, `?` help, `q` quit. Editing keys shared (Ctrl+A/E, Ctrl/Alt+arrows,
Alt+B/F, Shift+arrows, Ctrl+U/K/W/L, Ctrl+Home/End). Mouse: first click
focus, second click edits, header click sorts, drag select/scrollbar, wheel
3 rows without focus steal, outside-click closes cancelable surfaces, any key
suppresses hover, mouse-down/up same-target activation, 140 ms pressed flash.
Modal = begin_modal barrier (ring + hit); anchored popup = hit barrier only,
drawn last. Destructive = Cancel focused; irreversible writes need typed
target name. Status = footer right edge 4–5 s. No toasts.

### Widgets (direct matches; 24)

button, chips, choice (checkbox/radio/toggle), code (editor), completion,
dialog, empty, grid (data grid), input, keyhint, list, panel (card/frame/
scroll), picker, progress, props (via field_common/props.rs), scrollbar,
segments, select, table, tabs, textarea, tree. Apps: showcase (sidebar+page+
inspector layout), tablepro (identity strip, tab strip, explorer/body split,
connections screen).

### Explicitly absent from reference (do not invent)

Toast, context menu, diff viewer, generic badge, shadows, dim/reverse
modifiers, Title Case, column walls, framed cards, nested frames.

## Workstreams

| # | Stream | Depends on |
|---|---|---|
| 1 | Reference extraction (agents → reference-spec.md) | — |
| 2 | Termrock inventory (agent → termrock-inventory.md) | — |
| 3 | Verification infra design (agent → verification-infra.md) | — |
| 4 | Canonical tokens/theme in termrock (junie defaults at chokepoints) | 1,2 |
| 5 | Direct-match widget ports (batched per-widget workstreams) | 4 |
| 6 | Derived-widget design (multi-proposal, agree, implement) | 4 |
| 7 | Example apps migration (showcase/lookbook/cli) | 4,5,6 |
| 8 | Visual+behavioral verification vs shots | 3,5 |
| 9 | Docs (termrock DESIGN.md), cleanup, single system | 5–8 |
| 10 | Independent acceptance review (fresh agents) | 9 |

## Evidence artifacts

- `research/junie-campaign/reference-spec.md` — canonical extraction.
- `research/junie-campaign/termrock-inventory.md` — surface + classification.
- `research/junie-campaign/verification-infra.md` — harness design.
- Comparison renders per scenario; check logs for fmt/clippy/test.

## Non-negotiables

- No RGB literals in widgets; all through theme resolvers (reference rule 3).
- No second design system left in repo; stale paths removed (project law).
- Every meaningful visual change verified against reference render, not
  reviewer vibes.
- Breaking API changes allowed and preferred over fidelity loss.
