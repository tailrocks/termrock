# Phase 3 Audit 1 — style core vs junie-tui reference

Independent verifier, 2026-09-02. Target: working tree after style-core implementation.
Verdict: ported core (`style/junie.rs`) verbatim-faithful; widget layer does not
consume it — resolvers dead, widgets re-derive law with deviations.

## Verified CONTRACT-OK

- Palette hexes 19/19 match reference (`junie.rs:49-86` vs `theme.rs:67-88`).
- Dormant tokens (`#0a1c0c`, `#2e0f0f`, `#8787ff`) absent repo-wide.
- Downgrade vectors independently recomputed: all 256/16/mono values reproduce
  the reference algorithm exactly (`#48e054`→78, `#f59e09`→214, `#111111`→232,
  `#262626`→DarkGray@16 …). `#1e1e22`/`#232328` both→234 is canonical collapse.
- All 15 resolvers ported with exact ordering (disabled early-return,
  hover-after-tint, pressed last-wins, disabled-ignores-hover, hover
  suppressed while editing).
- D2 role surgery complete (`ROLE_COUNT=57`, no deleted-role references).
- D3 charts/actors/link/diff role values; D7 spacing consts ×13, flash 140,
  spinner 80, min 72×20; success-green law present (`SemanticStatus:86-88`,
  `CalloutTone:84`).

## Defects

### BLOCKER

| # | Where | Defect |
|---|---|---|
| B1 | 44 files, ~55 sites (e.g. `widgets/table.rs:1321`, `data_table.rs:1521`, `virtual_grid.rs:1003`, `tree_table.rs:1331`, `chrome_row.rs:173`, `patterns/task_rail.rs:1457`, `interaction/focus_graph.rs:820`) | `Modifier::REVERSED` painted. Reference: zero occurrences; reversal is explicit `fg(canvas).bg(text_primary)+BOLD`. D5 ban. |
| B2 | 12 files, ~21 sites (`controls.rs:422,429,449,2009,2041`, `code_block.rs:177,1258,1776,1782`, `toggle.rs:484`, `tree.rs:1119`, `identity.rs:593`, `text.rs:544`, `markdown.rs:1086`, `text_area.rs:1617`, `loading_overlay.rs:545`, `integration_status.rs:1426`) | `Modifier::DIM` painted. Reference: zero; dimming = one ladder step down. D5 ban. |
| B3 | `widgets/text_input.rs:1161,1177` | Caret double-inversion: recipe cursor already explicit reversal, `+REVERSED` re-swaps → caret white-on-black, invisible. |

### MAJOR

| # | Where | Defect |
|---|---|---|
| M1 | `widgets/primitives.rs:675`; `button_recipe` | Idle-enabled buttons map to `ControlState::Focused` → every enabled button BOLD; no `▎` gutter slot painted anywhere in widgets (`▎` literal only in `style/glyph.rs`). Reference: single focus owner, `t.gutter()` bar. |
| M2 | `widgets/primitives.rs:722,1284` | Armed press = reversal `+BOLD\|REVERSED` — cancels explicit reversal; pressed invisible. Reference pressed = full style replacement, no modifier. |
| M3 | `style/tokens.rs:1404` vs `widgets/row_chrome.rs` | Two row resolvers disagree (gutter tone secondary vs muted; membership `selected\|\|focused` vs `selected`). `junie.rs row()` is authority, 0 callers. |
| M4 | `widgets/row_chrome.rs:150-159` | selected&&focused tint wins over hover fill; D8: hover plane wins. (`tokens.rs:1461` encodes correctly — unused.) |
| M5 | `widgets/row_chrome.rs:74-88` | Parked selected row paints visible muted `▎`; reference parks gutter invisible (fg=bg), membership via `›`/`✓` at col 1 `text_secondary`. |
| M6 | `style/tokens.rs:1353-1363`, `:1342-1346` | Editing underline = `border_strong`; D5 law: accent. Invalid value repaints whole value error instead of underline only. |
| M7 | `widgets/diff.rs` | No `CROSSED_OUT` (deleted rows must be `#4d4d4d`+CROSSED_OUT); no dirty→Warning path; word insert/delete BOLD instead of ladder+glyph. |
| M8 | `widgets/spinner.rs:38-43`, `progress.rs:32-34` | Second activity vocabularies: dot-pulse, reverse-braille, ASCII frames, 8-frame truncation. D6/D7: one 10-frame braille cadence @80 ms. |
| M9 | `widgets/code_block.rs:165-190` | Reachable `TokenSyntax` paints raw Green/Magenta/Cyan/Blue + DIM — second syntax palette. |
| M10 | `patterns/approval_queue.rs:1044`, `widgets/connectivity.rs:93`, `patterns/prompt_queue.rs:788`, `session_picker.rs:1241-1242`, `plan_review.rs:1684`, `connection_manager.rs:2163`, `streaming_markdown.rs:509,515` | Banned glyphs `☑ ☐ ✕ ▌` still painted. |
| M11 | `widgets/list.rs:1128`, `patterns/approval_queue.rs:1057-1064` | Green budget: whole row label Accent (checked-unselected) / Accent+BOLD selected. Reference: `›`/`✓` marker + tint, never green rows. |
| M12 | `style/junie.rs` | Dead resolvers (0 production callers): `row()`, `scrollbar_track()`, `scrollbar_thumb()`, `key_hint_key()`, `key_hint_action()`, `tone()`, `badge()`. Scrollbar states unreachable; hints bypass; EDIT on-accent badge never painted. |
| M13 | `style/tokens.rs:1299-1307`, `widgets/primitives.rs` | Busy button keeps idle pair; reference busy = accent spinner prefix, label loses BOLD → text_secondary. |

### MINOR

| # | Where | Defect |
|---|---|---|
| N1 | `style/tokens.rs:1422-1428` | Busy applied after focus BOLD replaces style → focused+busy rows lose weight. `ListRowVisualState` lacks error/pressed fields. |
| N2 | `style/tokens.rs:1274` | Buttons pinned `ground = surface`; reference passes container bg (dialog ≠ surface). |
| N3 | `widgets/tabs.rs:1337` | Active-tab underline `─` not `━`, only when focused. |
| N4 | `style/quantize.rs:69-85` | `NO_COLOR=""`→Mono (reference: non-empty only); extra `TERM=dumb` arm; lowercased COLORTERM. |
| N5 | `style/mod.rs:308` | Static `Role::Backdrop` = muted/canvas; reference unstyled-cell backdrop = ghost on canvas. |
| N6 | `widgets/theme_picker.rs:50-54`, `style/tokens.rs:931,1490`, `registry/catalog.rs:327-340` | Legacy shims: 14 dead preset names alias junie; catalog advertises phosphor. No-alias law. |
| N7 | `widgets/primitives.rs:770,1231`, `ansi_text.rs:1071` | `⚠` icon (vocabulary `•`/`!`); link fallback `Color::Blue`. |
| N8 | `style/tokens.rs:1064-1067`, `style/glyph.rs:36-38`, `lookbook/src/stories.rs:4424,6863` | Doc residue: deleted MotionChannel link; orphaned dot-pulse doc; Ascii profile copy. |
| N9 | `widgets/charts.rs:203-213,1237,1742` | `VizGlyphSet::Ascii` + `-` fill — second glyph profile relocated into charts. |
| N10 | misc | ~40 pub `_ascii` unused params; `pub glyph_ascii()` fns; local density enums (WorkbenchDensity…); dead `if mono \|\| false` (`primitives.rs:744`); single-variant `GlyphSet`; `BorderShape` Rounded-only; `SelectionChrome::Tint`; `FocusEmphasis::{SelectionFill,Reversed}`; `quantize()` drops `with_role` overrides. |

Totals: BLOCKER 3 · MAJOR 13 · MINOR 10 · NIT (folded into N10).
Structural fix: make `JunieTheme`/recipes the only reachable style authority;
delete modifier-level escapes.
