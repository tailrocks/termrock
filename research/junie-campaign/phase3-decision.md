# Phase 3 Decision — Canonical Junie System in TermRock

Date 2026-09-02. Synthesis of phase3-proposal-A.md and phase3-proposal-B.md,
arbitrated against reference source (`src/theme.rs`, `src/widgets/grid.rs`) and
reference-spec.md. This file is the binding implementation contract.

## D1. Core architecture (A+B agree)

- New `crates/termrock/src/style/junie.rs`: verbatim port of reference
  `theme.rs` — 24 active tokens + `ColorLevel` + `downgrade/nearest_256/
  nearest_16/mono` + every resolver (`row`, `lift`, `gutter`, `button`,
  `field_style`, `placeholder`, `selection`, `scrollbar_track/_thumb`, `tone`,
  `syntax`, `badge`, `backdrop`, `border(focused)`, `title`, `label(focused)`,
  `key_hint_key/_action`).
- `RolePalette::junie()` is the ONLY palette. Delete presets: `tailrocks_
  phosphor`, `terminal_native`, `slate`, `paper`, `ansi`, `high_contrast`,
  `faded`, Appearance auto-mapper (`style/appearance.rs` deleted entirely),
  `ThemePackage::builtins()` reduced to the single canonical package,
  lookbook theme-picker swap arm.
- Capability: tokens CONSTRUCTED per ColorLevel using the reference downgrade
  algorithm. Delete `quantize_palette` math, `separate_elevation`, mono
  REVERSED substitution (mono = 4 gray buckets), and the
  `TERM contains truecolor` detect arm. Exact-match vector tests:
  `#262626`→DarkGray@16, `#f59e09`→Yellow@16, `#48e054`→Indexed(78)@256,
  `#111111`→232@256 (assert per reference algorithm output).
- Hover/press/thumb-focus leave the Role enum: computed by resolvers
  (`lift(bg)`: canvas→`#18181b`, surface/elevated→`#27272a`, field→`#232328`,
  else popover `#3f3f46`).

## D2. Role surgery (63 → 57)

Delete: `Raised`, `HoverTint`, `BackdropWash`, `ActionConstructive`,
`InfoStrong`, `InfoDim`, `TabActiveHovered`, `TabInactiveHovered`,
`DisclosureHeader`. (Info itself: see D4 — deleted.)

Add: `Popover` (`#3f3f46`), `TextSecondary` (`#b3b3b3`), `TextGhost`
(`#262626`, backdrop-only), `TextOnAccent` (`#19191c`).

Canonical values (TrueColor):

| Role | Value | | Role | Value |
|---|---|---|---|---|
| Canvas | `#000000` bg | | Border | `#262626` |
| Surface | `#111111` bg | | BorderFocused | `#4d4d4d` (no bold) |
| Elevated | `#18181b` bg | | Focus | `#48e054` |
| Field | `#1e1e22` bg | | Accent | `#48e054` |
| FieldHover* | `#232328` (resolver-only) | | Success | `#48e054` |
| SurfaceOverlay* | `#27272a` (resolver-only) | | Warning | `#f59e09` |
| Popover | `#3f3f46` bg | | Danger | `#e44545` (no bold) |
| Backdrop | per `backdrop()` | | Disabled | `#4d4d4d` |
| Text | `#ffffff` | | Selection | `#ffffff` on `#3f3f46` |
| TextStrong | `#ffffff` + BOLD | | SelectionTint | `#0f2e13` bg |
| TextSecondary | `#b3b3b3` | | ScrollTrack | `#262626` |
| TextMuted | `#808080` | | ScrollThumb | `#808080` (hover `#b3b3b3`, focused `#ffffff` — resolver) |
| TextFaint | `#4d4d4d` | | TabActive | underline `━` accent |
| TextDisabled | `#4d4d4d` | | TabInactive | `#b3b3b3` |
| TextGhost | `#262626` | | HintKey | `#ffffff` + BOLD |
| TextOnAccent | `#19191c` | | HintText | `#808080` |
| Input | fg `#ffffff` bg `#1e1e22` | | ActionFocused | `▎` accent + BOLD (not bg fill) |
| StatusBar | fg `#b3b3b3` bg canvas | | ActionDisabled | `#4d4d4d`, no DIM |

- `*` = exists inside resolvers/lift only; if kept as enum variants for
  compile-compat during surgery they must carry exact values and no resolver
  may bypass them.
- TextMuted site audit REQUIRED: every existing TextMuted paint site is either
  "supporting text" (→ TextSecondary `#b3b3b3`) or "metadata/placeholder/
  header" (stays `#808080`), per junie ladder semantics. Audit table goes in
  the PR description.

## D3. Derived buckets (no junie counterpart) — binding interpretations

- **Charts**: achromatic. Series 1..n walk the ladder `#ffffff → #b3b3b3 →
  #808080 → #4d4d4d`; axis `#808080`; grid `#262626`. Weight/markers
  differentiate; no hue. ( junie: "One hue"; charts are not exempt.)
- **Actors** (user/assistant/tool/...): ladder + glyphs + labels, NOT hue.
  Default content `#ffffff`; meta `#808080`; distinct roles carried by the
  existing label/glyph chrome (junie "glyphs carry meaning"). No purple.
- **Diff/change semantics — COPY from `grid.rs` (same semantic class)**:
  inserted → text_secondary `#b3b3b3` with `+`; deleted → text_muted `#808080`
  with `−`, whole deleted row `#4d4d4d` + CROSSED_OUT; modified/dirty →
  warning `#f59e09` with `•`; error → `#e44545` + BOLD `!`. DiffAdded role =
  `#b3b3b3`, DiffRemoved role = `#808080` (+ strikethrough at row chrome),
  any DiffChanged/Warn role = `#f59e09`. (Both proposals' green/red calls
  rejected: reference reserves green; renders changes via ladder+glyph.)
- **Link**: `#b3b3b3` + UNDERLINED (junie underline = affordance; links are
  affordances). No cyan.
- **Syntax** (code_block fallback): junie `syntax()`: keyword `#ffffff` BOLD,
  ident/plain `#ffffff`, string+number `#b3b3b3`, operator+punct `#808080`,
  comment `#4d4d4d` ITALIC.

## D4. Dormant tokens: NOT ported

`accent_bg_subtle #0a1c0c`, `error_bg #2e0f0f`, `info #8787ff` stay out of the
system entirely (reference: "not part of the system"). `Role::Info` deleted;
its 65 sites re-derived: informational status → TextSecondary; actor System →
glyph+label distinction. No `#8787ff` anywhere in the repo.

## D5. Modifiers law

Allowed: BOLD, ITALIC (comments, NULL/DEFAULT only), UNDERLINED/UNDERLINE
(3-color law: accent = editing here; border-strong `#4d4d4d` = quiet
affordance/hover-editable/current line; error/warning = diagnostic range),
CROSSED_OUT (deleted rows only — junie's only strikethrough), REVERSED
implemented EXPLICITLY as fg(canvas).bg(text_primary), never Modifier::REVERSED.
Banned: DIM, and Modifier::REVERSED itself.

## D6. Glyphs

- `▌`→`▎` everywhere (rename to FocusBar semantics). `❯` prompt deleted.
  `☑/☐`→`[✓]/[ ]`, spinner frames → braille `⠋⠙⠹⠸⠼⠴⠦⠧⠇⠏` @80 ms tick,
  `═`→`━`, `✕`→`×`, warning glyph → `•`/`!` per junie table, sort `▴/▾`,
  tree `▸/▾`, filtered `∇`, pk `⚷` over `▪`, env `◆ ◇`.
- DELETE glyph variants with no junie counterpart: File, Folder, FolderOpen,
  Search, Settings, Edit, Copy, Play, Stop, CheckMixed, RuleTee*, Connection,
  FocusDiamond, DisabledMark, DiamondDouble, StatusDot*, RailCollapsed,
  SliderTick, Token, dot-pulse spinner.
- **Delete `GlyphSet::Ascii` and `GlyphSet::Enhanced`** (B's call, adopted):
  the reference has exactly one glyph vocabulary; state must survive monochrome
  via glyphs+modifiers, which the vocabulary guarantees. `border_set()` always
  ROUNDED `╭ ╮ ╰ ╯ ─ │`. `GLYPH_CONTEXTS` rewritten to junie's
  context-disambiguation law.

## D7. Density / motion / spacing

- Delete `Density` enum. `SpacingScale` holds junie's named tokens as const
  values: gutter 1, inline 1, gap 2, column_gap 2, form_gap 4, card_inset 2,
  frame_inset 3, dialog_inset 3, tree_indent 2, field_height 3, tabs_height 2,
  min 72×20.
- `MotionPolicy` = {Full, Off}. `ACTION_FLASH_MS` 1000→140. Spinner 80 ms.
  Delete MotionChannel::{Live,Wait,Stream}, Basic cap, easing/shimmer/wave/
  pulse/edge_fade/blend. Status expiry 4 s (host-configurable knob remains,
  default 4 s).
- `BorderShape::Rounded` canonical; Square variant deleted with GlyphSet
  profiles (no second system).

## D8. Selection law (replaces old gates)

New invariant `selection_tint_requires_focus_and_hover_wins`:
- tint `#0f2e13` bg iff selected && focused;
- selected && !focused: marker glyph only, no fill;
- hovered: `lift(bg)` replaces tint (hover plane wins);
- pressed: `#000000` on `#ffffff`;
- text/range selection: `#ffffff` on `#3f3f46` (popover).

## D9. Gates (design_gate.rs + style tests) — rewrite in same commit

Rewrite to junie law: `phosphor_baseline_uses_named_ansi_only` →
`junie_truecolor_tokens_exact` (exact-RGB table + downgrade vectors);
`accents_are_distinct` → junie green-budget invariant (green only: focus
gutter, primary fill, `›`/`✓` on focused rows, active document-tab underline,
EDIT badge, spinner/live activity, completed progress, required `*`, selected
tree label, checked ✓); `no_widget_paints_selection_fill_by_default` →
replaced by D8 invariant; `phosphor_surfaces_keep_semantic_elevation` →
strictly-increasing ladder + KNOWN_SHORTFALLS extension; ladder floor gate →
junie's measured steps (canvas→surface 1.11:1 etc. are CANONICAL, gate asserts
exact values not ratios); `interaction_underline_is_dead` → 3-color underline
law; `accent_budget` → position-based classes (focus-gutter cells, fill cells,
marker cells budgeted separately); `bold_budget_per_row` → one bold run per
focused row (+title/keyword exemptions); `state_matrix_distinct` → junie state
table (default/hover/focus/selected±focused/pressed/disabled/error/editing/
busy); contrast_floor KNOWN_SHORTFALLS → junie's four declared pairings
(primary-pressed 3.72, danger-label 3.71-ish, white-on-error press,
placeholder muted-on-field — measure exact ratios in-repo); motion gates →
140 ms flash + 80 ms tick + {Full,Off}; `collections_share_one_gutter_glyph`
→ `▎`; `canvas_uses_terminal_default_fill` → canvas is `#000000`.
Delete gates for deleted features (Ascii profile, Density, appearance, hover
roles, dim). All 41 gates must either encode junie law or be deleted with a
one-line justification in the PR.

## D10. Ordered execution (each step must compile+test green before next)

1. `junie.rs` port + palette construction + capability downgrade (+ vector
   tests). Old presets still present but unwired.
2. Role surgery (D2) + resolver rewiring in tokens.rs recipes (button, input,
   list row, panel, surface) as literal theme.rs resolver ports. Fix all
   call sites. Compile green.
3. Glyph surgery (D6), spacing/motion (D7). Compile green.
4. Delete dead code: presets, appearance.rs, quantize math, Ascii/Enhanced,
   Density, hover roles, theme picker arms. `cargo clippy --workspace
   --all-targets -- -D warnings` green.
5. Gates rewrite (D9) in the same PR series. `cargo test -p termrock --test
   design_gate` green.
6. Bless: lookbook goldens (15), PNG baselines (123), docs preview-posters,
   catalog JSON, public-api snapshot. Review diffs for sanity (geometry
   unchanged except rounded borders/▎/spinner — color deltas everywhere).
7. Capability coverage: add goldens/color-layer scenarios for 256/16/mono
   (harness `run.py --layer color`) — today zero render coverage below
   TrueColor.
8. Docs: docs/design/* collapse to one TermRock DESIGN.md derived from the
   reference (separate workstream, Phase 7).

Steps 1–3 one implementation agent (style core). Step 5 a SECOND agent in
parallel against the D9 contract (tests only, no src edits); integrator
arbitrates mismatches. Step 6 a third agent. Fidelity authority:
reference-spec.md + campaign-plan.md + this file.
