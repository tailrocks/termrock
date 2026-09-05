# Phase 3 proposal A — junie-tui's exact visual system as TermRock's only rendering

Date 2026-09-02. Branch `experimental/component-catalog-docs-2026-09-02`.
Target `/Users/donbeave/Projects/tailrocks/termrock`. Reference
`/Users/donbeave/Projects/terminal-components-claude` (`junie-tui`), SoT
`src/theme.rs` + `DESIGN.md` (Colors / Pairings and states / Layout rhythm).
Read-only on both repos; this file is the only write.

## 0. Governing call

Three moves, all inside the existing chokepoint (`style/tokens.rs::DesignSystem`,
144/151 widget files, 1613 references). No widget gains a color.

1. **New `crates/termrock/src/style/junie.rs`** — verbatim port of reference
   `src/theme.rs`: `palette` consts, `JunieTheme::junie()` (27 tokens),
   `ColorLevel`, `downgrade` / `nearest_256` / `nearest_16` / mono 4-bucket,
   `ColorLevel::detect`, and every resolver (`row`, `lift`, `gutter`, `button`,
   `field_style`, `placeholder`, `selection`, `scrollbar_track`,
   `scrollbar_thumb`, `tone`, `syntax`, `badge`, `backdrop`).
2. **`RolePalette::junie()` becomes the only constructor.** `tailrocks_phosphor`,
   `terminal_native`, `slate`, `paper`, `ansi`, `high_contrast` are deleted
   (style/mod.rs:345–770). `Role` collapses 63 → 56 (§1.5/§1.6).
3. **State logic moves to the resolvers** (§3). `Role` becomes a pure
   projection of `JunieTheme`; hover, press, focus, editing are *computed*,
   not stored.

A role that junie does not have a value for is either derived by junie's rules
or deleted. No synonym tiers, no dormant tokens (project no-legacy law).

## 1. Role mapping table (TrueColor)

`Rgb` values are the reference's; every one is a `palette` const in
`style/junie.rs`.

### 1.1 Planes (bg-carrying)

| Role | today (style/mod.rs:345) | becomes | note |
|---|---|---|---|
| `Canvas` | `bg(Color::Reset)` | `#000000` | junie canvas is a literal black; `Reset` is gone (§4). |
| `Surface` | `bg(Ansi16 Black)` | `#111111` | card plane. |
| `Elevated` | `bg(Reset)` + marker | `#18181b` | dialogs, popups, picker. |
| `Sunken` | `bg(Black)` | `#1e1e22` | = junie `field`; the input well. |
| `Popover` | — **new** | `#3f3f46` | strongest neutral: text/range selection, secondary/danger button fill, current find match. |
| `Raised` | hover/section surface | **DELETED** | junie has no such token; hover is `lift(bg)` computed at paint (§3). 22 sites → `lift()` or `Elevated`. |
| `HoverTint` | `bg(Black)` | **DELETED** | same reason; 15 sites → `DesignSystem::lift(bg)`. |
| `BackdropWash` | `bg(Black)` | **DELETED** | junie's modal dim is the per-cell `backdrop(style)` walk, not a fill. 11 sites → `DesignSystem::backdrop_style(Style)`. |
| `Backdrop` | `fg(DarkGray)` | `bg #000000`, `fg #262626` | survives as the walk's residual branch only. |
| `StatusBar` | `fg(Gray) bg(Black)` | `fg #b3b3b3` on `Canvas` | junie footer: one row, text-secondary, right-edge status. |
| `Input` | `fg(Gray) bg(Black)` | `fg #ffffff`, `bg #1e1e22` | value/well pair; well never changes hue while editing. |
| `InputInvalid` | `fg(Red) bg(Black)` | `fg #e44545`, `bg #1e1e22` | invalid keeps the field plane + error underline + trailing bold `!`. |
| `Selection` | `bg(Green) fg(Black)` | `fg #ffffff`, `bg #3f3f46` | = `Theme::selection()`. Now means **text/range selection only**, never row membership (60 sites audited). |
| `SelectionTint` | `bg(DarkGray)` | `bg #0f2e13` | = `accent_bg`; selected **and** focused rows only. |

Plane order is junie's, monotonic: `000000 → 111111 → 18181b → 1e1e22 → 3f3f46`.
`#27272a` (`surface_overlay`) and `#232328` (`field_hover`) stay tokens inside
`JunieTheme`, reached only through `lift()` and `button_recipe`.

### 1.2 Text ladder (junie: white at 100/70/50/30/15 over black)

| Role | today | becomes |
|---|---|---|
| `Text` | `fg(Gray)` | `#ffffff` |
| `TextStrong` | `fg(White) bold` | `#ffffff` BOLD |
| `TextMuted` | `fg(DarkGray) dim` | `#808080` (DIM purged, §4) |
| `TextFaint` | `fg(DarkGray) italic` | `#4d4d4d`; ITALIC only for code comments |
| `TextDisabled` | `fg(White) DIM\|CROSSED_OUT` | `#4d4d4d`, **no modifier** (junie `disabled = text_faint`) |
| `HintKey` | `White bold` | `#ffffff` BOLD |
| `HintText` | `fg(Gray)` | `#808080` |
| `HintDim` | `fg(DarkGray) dim` | `#4d4d4d` |
| `HintSeparator` | `fg(DarkGray)` | `#4d4d4d` |
| `TextGhost` | — | not a role; lives in `JunieTheme.text_ghost = #262626`, backdrop only |

### 1.3 Borders, accent, safety

| Role | today | becomes | note |
|---|---|---|---|
| `Border` | `fg(DarkGray)` | `#262626` (`border_subtle`) | |
| `BorderFocused` | `fg(LightGreen)` | `#4d4d4d` (`border_strong`) + BOLD | **the accent leaves the frame.** junie `border(focused)` → `border_strong`; green is reserved for the gutter/fill/badge. |
| `Focus` | `fg(LightGreen)` | `#48e054` | the `▎` gutter only. |
| `Accent` | `fg(Green)` | `#48e054` | gutter, primary fill, `›`/`✓` markers, active tab underline, `EDIT` badge, spinner, indeterminate sweep, completed progress, required `*`, selected tree label, checked box. |
| `Success` | `fg(LightGreen)` | `#48e054` | junie `success = accent`. |
| `Warning` | `fg(Yellow)` | `#f59e09` | unsaved changes, `•` modified, pending counts, warning diagnostics, `▲` cost. |
| `Danger` | `fg(Red) bold` | `#e44545` | no bold at rest; bold arrives only via the resolver. |
| `ActionFocused` | `fg(Black) bg(Green) bold` | `fg #19191c`, `bg #48e054`, BOLD | primary button at rest (`on_accent` on `accent`). |
| `ActionDisabled` | `fg(DarkGray) dim` | `#4d4d4d` | |
| `ActionConstructive` | `fg(LightGreen) bold` | **DELETED** | "constructive" is not in junie's green budget; 6 sites → `Accent` only when it is the primary commit, else `Text`. |
| `DisclosureHeader` | `fg(Yellow) bold` | **DELETED** | disclosure = `TextStrong` + `▸/▾`; 6 sites. |
| `Link` | `fg(Cyan)` | `#b3b3b3` | junie has no link tone: unfocused-label tier; UNDERLINED only in 16/mono. 24 sites. |
| `LinkHover` | `fg(LightCyan)` | `#ffffff` on `lift(bg)` | |
| `ScrollTrack` | `fg(Black)` | `#262626`, glyph `│` | |
| `ScrollThumb` | `fg(DarkGray)` | `#808080`; hovered `#b3b3b3`, focused `#ffffff`, glyph `┃` | only when overflowing. |
| `TabActive` | `fg(White) bold` | `#ffffff` BOLD + **accent underline row** | junie active document tab = accent underline; `tabs-height 2`. Kills `tab_palette_roles_are_underline_free`. |
| `TabInactive` | `fg(Gray)` | `#808080` | |
| `TabActiveHovered` / `TabInactiveHovered` | White bold / Gray | unchanged values | hover is the plane lift (`lift(bg)`), not a new fg. |

### 1.4 Derived roles (junie has no counterpart — one-line justification each)

| Role | becomes | derivation |
|---|---|---|
| `DiffAdded` | `#ffffff` | green is budgeted to gutter/action/marker/tab/badge/live-activity only; a diff add is none of them, so the `+` glyph and position carry the fact (hue-free ladder). |
| `DiffRemoved` | `#e44545` | safety tone: removal is the one destructive fact in a diff. |
| `SyntaxKeyword` | `#ffffff` BOLD | `SyntaxTone::Keyword`. |
| `SyntaxString` | `#b3b3b3` | `SyntaxTone::Str`. |
| `SyntaxNumber` | `#b3b3b3` | `SyntaxTone::Number`. |
| `SyntaxComment` | `#4d4d4d` ITALIC | `SyntaxTone::Comment` (the only italic in the system). |
| `SyntaxFunction` | `#ffffff` | `SyntaxTone::Ident`. |
| `ActorUser` | `#ffffff` | the human is primary content. |
| `ActorAssistant` | `#ffffff` BOLD | weight, not hue, separates the speaker. |
| `ActorThinking` | `#808080` | meta commentary = muted tier. |
| `ActorTool` | `#b3b3b3` | supporting text = secondary tier. |
| `ActorPlan` | `#b3b3b3` | pending is stated by the glyph, which may use `#f59e09`. |
| `ActorSystem` | `#808080` | ambient system text = muted tier. |
| `ChartSeries1..4` | `#ffffff`, `#b3b3b3`, `#808080`, `#4d4d4d` | "everything else the white ladder": series separate by a value ramp, never hue. |
| `ChartAxis` | `#808080` | metadata tier. |
| `ChartGrid` | `#262626` | `border_subtle`. |
| `Info` | **DELETED** | junie declares `info #8787ff` dormant — "not part of the system". 47 sites → `#b3b3b3` (annotation) / `#ffffff`. |
| `InfoStrong` | **DELETED** | → `#ffffff` BOLD (7 sites). |
| `InfoDim` | **DELETED** | → `#808080` (11 sites). |

**Dormant-but-present decision:** `accent_bg_subtle #0a1c0c`, `error_bg
#2e0f0f`, `info #8787ff` are **not ported at all**. `Role` never grows them.
Keeping a token that no resolver reads is exactly the second-system rot the
campaign forbids; junie's own doc says "do not introduce them into new screens".

**`Role` count: 63 − 8 (`Raised`, `HoverTint`, `BackdropWash`, `Info`,
`InfoStrong`, `InfoDim`, `ActionConstructive`, `DisclosureHeader`) + 1
(`Popover`) = 56.** ~125 call sites migrate mechanically (grep `Role::<name>`).

### 1.5 GlyphSet and shape re-pointing (same commit)

- `Glyph::SelectionGutter` `▌` → `▎` (junie row anatomy: col 0 gutter, col 1
  marker slot, content col 3).
- Spinner = ten-frame braille `⠋⠙⠹⠸⠼⠴⠦⠧⠇⠏` @ 80 ms; `SPINNER_DOT_PULSE_FRAMES`
  deleted (`style/glyph.rs`).
- `FocusDiamond` title prefix deleted (junie: a focused container brightens its
  frame or shows the bar in the title row); `PanelChrome::Danger` keeps `▲`? No —
  junie uses no warning glyph on frames; `title_prefix` survives only for
  `Danger` → none. Delete the field.
- `BorderShape::default()` `Square` → `Rounded` (`╭╮╰╯`, junie frame-inset 3).
- Modifiers allowed: BOLD, ITALIC, REVERSED, UNDERLINED. **DIM and CROSSED_OUT
  are purged** (junie uses neither).
- `SpacingScale` from `Density::Comfortable` → junie rhythm: gutter 1, inline 1,
  gap 2, column-gap 2, form-gap 4, card-inset 2, frame-inset 3, dialog-inset 3h/2v,
  tree-indent 2, field-height 3, tabs-height 2, min 72×20; button width
  `label + 2` (+2 with marker/spinner).

## 2. Capability fallback — port the reference's own downgrade

Replace `style/quantize.rs`'s machinery (`quantize_color`, `quantize_style`,
`quantize_palette`, `rgb_to_xterm256`, `rgb_to_ansi16`, `indexed_to_ansi16`,
`separate_elevation`) with the reference's four functions, copied unchanged:
`downgrade`, `nearest_256`, `nearest_16`, mono `(r+g+b)/3 → Black|DarkGray|Gray|White`.

- `ColorCapability` stays as the public enum; `Truecolor|Indexed256|Ansi16|
  Monochrome` ↔ `ColorLevel::TrueColor|Ansi256|Ansi16|Mono`.
- **What replaces the ANSI-16 default:** `DesignSystem::junie()` =
  `JunieTheme::for_level(level)` projected onto `Role`, where
  `level = ColorCapability::detect_from_env()` unless `--color` overrides.
  Tokens are *constructed* at the target level (junie's model); there is no
  post-hoc palette mutation, so 256/16/mono render bit-identically to the
  reference by construction.
- Pin the reference's own assertions: at Ansi256 `accent == Indexed(n)` with `n`
  pinned; at Ansi16 `accent == LightGreen`, `error == LightRed`,
  `canvas == Black`; `NO_COLOR` (non-empty) → Mono. junie also maps
  `TERM` containing `ghostty`/`kitty` → Ansi256; termrock's
  `ColorCapability::detect_from_env` gains those two, keeps `TERM=dumb` →
  Monochrome (that is a profile concern, not a color one).
- **Mono behavior change:** today `Monochrome` → `Color::Reset` + `REVERSED`
  compensation (`quantize.rs`, `surface.rs::normalize_content_band`). After the
  port, mono = 4 gray buckets with modifiers intact (junie: "the glyph and
  modifier language carries the state alone"). `normalize_content_band` keeps
  its BOLD projection but drops the Reset substitution.
- `faded()` / `blend_toward` (`style/motion.rs`) stay RGB-space and are
  downgraded at the edge like everything else;
  `faded_named_ansi_stays_in_named_terminal_space` is deleted (no named-ANSI
  space exists any more).

## 3. Recipe resolvers — termrock family fns ↔ junie resolvers

| termrock | junie | exact state logic to port |
|---|---|---|
| `DesignSystem::resolve_list_row(ListRowVisualState)` (tokens.rs:1583) | `Theme::row(s, bg)` (theme.rs:309) | disabled → `fg #4d4d4d` on `bg`, **hover ignored**; base `fg #ffffff` on `bg`; `selected && focused` → `bg #0f2e13`; `hovered` → `bg lift(bg)`; `error` → `fg #e44545`; `busy` → `fg #b3b3b3`; `focused` → BOLD; `pressed` → `fg #000000` / `bg #ffffff` BOLD, 140 ms. |
| `RowChrome::resolve` (widgets/row_chrome.rs:56) | `row()` + `gutter()` | selected+unfocused = marker glyph only, `fg #ffffff`, **no tint**; selected+focused = glyph + `#0f2e13` + BOLD; gutter `▎` `fg #48e054` when the collection owns the keyboard, `fg == bg` (hidden) when parked. Slot always reserved. |
| `DesignSystem::lift` / `palette::lift` | `Theme::lift(bg)` (theme.rs:342) | `#000000 → #18181b`; `#111111`/`#18181b → #27272a`; `#1e1e22 → #232328`; else `#3f3f46`. Never a hue change. |
| `DesignSystem::button_recipe(variant, ControlState)` (tokens.rs:1378) | `Theme::button(kind, s, bg)` (theme.rs:367) | **Primary** `#19191c` on `#48e054` BOLD; hovered `#3ab343`; pressed `#2b8632` (140 ms). **Secondary/Toggle** `#ffffff` on `#27272a`; hovered `#3f3f46`; focused BOLD; pressed `#000000` on `#ffffff`. **Quiet/Subtle** `#b3b3b3` on `bg`; hovered `#ffffff` on `lift(bg)`; focused `#ffffff` BOLD; pressed inverted. **Danger** `#e44545` on `#27272a`; hovered `#3f3f46`; focused BOLD; pressed `#ffffff` on `#e44545`. **Disabled** `#4d4d4d`, fill `bg` for Quiet else `lift(bg)`, hover ignored. **Link** `#b3b3b3` → hover `#ffffff`. |
| `DesignSystem::input_recipe(state, invalid)` (tokens.rs:1508) | `field_style` + `placeholder` + `border(focused)` | plane `#1e1e22` unchanged while editing; hover → `#232328` only when **not** editing; invalid → `fg #e44545` + error underline + trailing bold `!`; editing → accent underline + hardware cursor + ` EDIT ` badge; disabled → `#4d4d4d` on `#1e1e22`; placeholder `#808080`. **Focus is not a bright border any more** — `prompt` glyph slot and `Role::BorderFocused` in `InputRecipe` are deleted. |
| `DesignSystem::panel_recipe(emphasis, elevation)` (tokens.rs:1304) | `border(focused)` + `on(bg)` | unfocused `#262626`; focused `#4d4d4d` BOLD; danger `#e44545`; title `#ffffff` BOLD. `blend_role` border cross-fade is deleted — junie's only timed transition is the 140 ms press. |
| `SurfacePaintPlan` (widgets/surface.rs:481) | planes + `on(bg)` | `Canvas→#000000`, `Inset→#111111`, `Sunken→#1e1e22`, `Raised/Overlay→#18181b`, `OverlayFocused→#18181b` + `#4d4d4d` frame, `Interactive→ lift at hover`, `Selected→#0f2e13` only when focused, `Warning/Destructive→#f59e09/#e44545` fg on the same plane. |
| `Role::Selection` consumers (text/range/find match) | `Theme::selection()` (theme.rs:454) | `#ffffff` on `#3f3f46`. |
| `scroll::render` | `scrollbar_track/thumb` (theme.rs:458) | track `│` `#262626`; thumb `┃` `#808080` / hovered `#b3b3b3` / focused `#ffffff`; drawn only on overflow. |
| `code_block.rs` syntax (incl. the 4-site ANSI fallback at :180-188) | `Theme::syntax(SyntaxTone)` (theme.rs:486) | keyword `#ffffff` BOLD; ident/plain `#ffffff`; str/number `#b3b3b3`; operator/punct `#808080`; comment `#4d4d4d` ITALIC. The fallback literal palette is deleted — `DesignSystem::syntax()` is always available. |
| `Badge` (`widgets/badge.rs`) | `Theme::badge(BadgeKind::Edit)` (theme.rs:501) | `#19191c` on `#48e054` BOLD is the **only** filled badge in the system; every other badge is ladder text, no fill. |
| status/segment tone | `Theme::tone(Tone)` (theme.rs:472) | Normal `#ffffff`, Secondary `#b3b3b3`, Muted `#808080`, Faint `#4d4d4d`, Error `#e44545`, Warning `#f59e09`, Success `#48e054`; **never** the accent. |
| modal dim (`interaction/overlay_stack.rs`) | `Theme::backdrop(style)` (theme.rs:277) | bg: canvas/surface/elevated keep; field/field_hover → elevated `#18181b`; any colored fill → `#27272a`. fg: own-bg glyph stays hidden; canvas/surface → bg; primary/accent/error/warning → `#808080`; secondary/on_accent → `#4d4d4d`; else `#262626`. Modifiers cleared. Footer excluded. |
| focus gutter | `Theme::gutter(s, bg, on_accent)` (theme.rs:356) | `▎` `fg #48e054`; `fg == bg` when unfocused; `fg #ffffff` when the control is accent-filled. |

## 4. Gate-law rewrite list (`crates/termrock/tests/design_gate.rs`, `style/*`)

| gate | encodes | new law |
|---|---|---|
| `accent_budget` (`ACCENT_CELL_BUDGET`) | ≤2 accent regions | keep, **re-derived**: budget counts `#48e054` cells and is now per-surface-class (list frame, tab strip, footer), because junie legitimately spends green on gutter + underline + badge + progress in one viewport. Constant lives beside the new assertion. |
| `accents_are_distinct` (style/mod.rs:931) | ScrollThumb/DiffAdded/TabActive/Border ≠ accent | **rewrite**: `ScrollThumb == #808080`, `Border == #262626`, `BorderFocused == #4d4d4d` (≠ accent) still hold; `DiffAdded == #ffffff` and `TabActive == #ffffff` + accent underline are now *equal-to-primary*, so the "≠ accent" half is dropped and replaced by "accent appears only via `Accent`/`Focus`/`Success`/`ActionFocused`/`badge(Edit)`/tab underline". |
| `phosphor_baseline_uses_named_ansi_only` (style/mod.rs:907) | runtime = named ANSI-16 | **delete**; replaced by `junie_baseline_is_truecolor_and_downgrades_exactly` (§2): every role is `Color::Rgb`, and `for_level(Ansi16/Ansi256/Mono)` equals the reference's own downgrade output. |
| `faded_named_ansi_stays_in_named_terminal_space` (style/mod.rs:922) | fade stays in ANSI space | **delete** — the named space is gone. New: `blend_toward`/`faded` emit `Color::Rgb` only, and `DesignSystem::quantize` downgrades the result. |
| `no_widget_paints_selection_fill_by_default` | selection fill is opt-in | **delete**. junie: selected+unfocused = marker only, selected+focused = `#0f2e13` tint. Replaced by `selection_tint_requires_focus`: rendering a selected+unfocused row must not emit `#0f2e13`, and `Role::Selection` (`#3f3f46`) appears only for text/range selection. |
| `selection_chrome_is_not_overridden_in_widget_paint` + `row_chrome.rs:66-89` canonicalization | Fill/Marker ignored | keep the *spirit* (one selection language) but the canonical target is junie's `row()`; `SelectionChrome::{Fill,Marker}` are deleted, enum becomes `Gutter`-only and is then removed entirely (single law, no variants to pick). |
| `collections_share_one_gutter_glyph` | `▌` shared | keep; glyph literal becomes `▎`. |
| `interaction_underline_is_dead` + whitelist | underline forbidden in chrome | **narrow**: underline is legal for editing (accent underline), mono/no-color links, and content-faithful SGR/OSC-8/markdown. The whitelist shrinks to `link.rs`, `citation.rs`, `key_value_list.rs`, `markdown.rs`, `text.rs`; `primitives.rs` (Link button) and `code_block.rs` (squiggle) entries are re-judged — `code_block.rs` keeps it only as the diagnostic squiggle substitute in mono. |
| `tab_palette_roles_are_underline_free` | tabs never underline | **delete** — junie's active document tab *is* an accent underline row. Replaced by `active_tab_underline_is_accent_and_two_rows_tall` (label row + `#48e054` rule row; inactive tabs have no rule). |
| `recipe_families_are_complete_and_restrained` / `AccentUsage` | accent budget per family | `AccentUsage` variants re-pointed: `Action → PrimaryIntent`, `Input/Collection/Overlay → FocusOnly`, `Status/Data → SemanticMark`, `Layout → None`. Unchanged in shape. |
| `state_matrix_distinct` | idle/hover/pressed/disabled distinct | keep — junie satisfies it (hover = one plane, press = inverted, disabled = `#4d4d4d`, and disabled *ignores* hover, which becomes an explicit assertion copied from junie's `disabled_button_ignores_hover`). |
| `data_rows_have_ladder`, `bold_budget_per_row` | tier + weight hygiene | keep; the bold budget is re-tuned (junie bolds focus, titles, keys, keywords — more often than termrock did). |
| `inputs_share_field_chrome`, `a_focused_field_says_so` | bright border + prompt glyph | **rewrite**: a focused field is `#1e1e22` + accent underline + cursor + ` EDIT ` badge; the `›` prompt cell and `BorderFocused` border are gone. |
| `one_scrollbar_language` | only `scroll::render` touches Scroll* | keep verbatim. |
| `text_never_touches_borders`, `bordered_overlays_reserve_their_gutters`, `truncation_has_ellipsis`, `a_scrolled_region_says_it_continues`, `no_bare_ellipsis_in_paint`, `one_overflow_note`, `one_chord_notation`, `pattern_*`, `widgets_never_import_patterns`, `patterns_*`, `no_wide_emoji_in_chrome`, `modal_geometry_never_escapes_its_terminal`, `flagship_widgets_survive_tiny_and_random_geometry`, `workbench_overlays_survive_tiny_and_random_geometry`, `motion_policy_*`, `spinner_frames_one_column` | copy/geometry law | unchanged. |
| `every_button_variant_paints_distinct_focus_without_color` | focus ≠ color per variant | keep; expected styles are the junie ones (primary: BOLD only; secondary: BOLD; quiet: `#ffffff` + BOLD). |
| `one_chip_recipe` | chips share a recipe | keep; chip = ladder text, no fill (only `EDIT` is filled). |
| `public_ui_inventory_has_exact_recipe_and_monochrome_evidence` (lookbook `tests/design_gate.rs`) | per-identity cue | keep; the mono pass now runs against the gray-bucket projection. |

### `style/mod.rs` / `tokens.rs` tests

- `default_borders_use_gray_inactive_and_green_focused` →
  `default_borders_use_262626_inactive_and_4d4d4d_focused`.
- `default_separates_ordinary_and_strong_text`, `hc_and_paper_have_text_ladders`,
  `slate_preset_pins_load_bearing_role_values`, `phosphor_preset_pins…` →
  **delete with their presets**; one new test pins all 56 roles to the reference
  hexes (a table test over `RolePalette::junie()`).
- `disabled_and_faint_tiers_stay_distinguishable` (contrast_floor.rs:453) and the
  "disabled and faint must not resolve to the same style" assert
  (style/mod.rs:885) → **delete**: junie *defines* `disabled = text_faint`;
  unavailability is carried by "no hover, no Tab ring, no activation".
- `surface.rs` ladder tests asserting `Raised ≠ Surface ≠ Elevated ≠ Sunken` →
  rewrite for `000000/111111/18181b/1e1e22/3f3f46` strictly increasing.
- `Appearance` (`style/appearance.rs`): `palette_for_appearance`,
  `AppearanceThemeMap`, and its 4 tests → **delete**. junie is polarity-locked
  dark; there is no light theme to map to. `Appearance::detect` survives only if
  a host still wants the hint — it no longer picks a palette.
- `ThemePicker` (`widgets/theme_picker.rs`) + `ThemePackage::builtins()` →
  delete the widget, `theme_from_preset_id`, `system_from_preset_id`;
  `builtins()` returns the single `junie` package. A picker for one theme is
  dead UI.

### Contrast floor: port junie's four declared sub-AA pairings

Measured (WCAG, this repo's `contrast_ratio`):

| pairing | fg/bg | ratio |
|---|---|---|
| primary button pressed | `#19191c` on `#2b8632` | 3.81 |
| danger label at rest | `#e44545` on `#27272a` | 3.71 |
| danger pressed | `#ffffff` on `#e44545` | 4.01 |
| placeholder | `#808080` on `#1e1e22` | 4.21 |

New `KNOWN_SHORTFALLS` (contrast_floor.rs:56) — `presets()` becomes
`[("junie", RolePalette::junie())]`, and the entries are:

```
("junie", "button Primary/Pressed label"),   // 3.81 — 140 ms, inverted glyph too
("junie", "button Destructive/Default label"), // 3.71 on surface_overlay
("junie", "button Destructive/Pressed label"), // 4.01
("junie", "input Default/invalid=false placeholder"), // 4.21
("junie", "TextFaint on Canvas"),            // 2.48 — meta tier, glyph+label carry it
("junie", "TextFaint on Surface"),           // 2.23
("junie", "TextDisabled on Canvas"),         // 2.48
("junie", "TextDisabled on Surface"),        // 2.23
("junie", "Border on Surface"),              // 1.25
("junie", "Border on Elevated"),             // 1.02
("junie", "ladder Canvas->Surface"),         // 1.11
("junie", "ladder Surface->Raised"),         // 1.07
```

The old `paper` / `high_contrast` entries die with the presets. **The 1.15
per-step ladder floor is unrecoverable under junie's planes** (1.11 / 1.07 /
1.19) — so `palette_pairs`' ladder floor becomes a "strictly increasing" check
and the two ladder rows move into `KNOWN_SHORTFALLS` with a comment quoting
junie's intent (depth is carried by the frame + title + inset, not by luminance).
`border_subtle` on `Surface`/`Elevated` (1.25 / 1.02) is the price of a
10 %-white hairline; frames also brighten to `#4d4d4d` when focused, which is
the compensating cue. `Info on Canvas/Surface` (1.0 / 1.11 today) disappears
because `Role::Info` is deleted. `InputInvalid on its own tint` becomes
`#e44545` on `#1e1e22` = 4.14 → one more shortfall row. `recipe_pairs_pass_floor`
runs against `junie()` and asserts exactly the four declared + the faint/disabled
rows, nothing else.

## 5. Migration order and blast radius

Ordered; each step is one green `mise run check` before the next.

1. **Add `style/junie.rs`** (consts, `JunieTheme`, downgrade set, resolvers) +
   unit tests copied from `theme.rs:623-678`. No consumer yet.
2. **Tokens**: `RolePalette::junie()`; delete the five other constructors and
   `PHOSPHOR_GREEN`/`PHOSPHOR_DARK`/`PREVIEW_CARD` web swatches (style/mod.rs:60)
   — the SVG/poster export path now reads the junie hexes. Role enum surgery
   (−8/+1). `DesignSystem::default()` = `junie()`.
3. **Recipes + lift + glyphs + spacing + BorderShape::Rounded** (§1.5, §3).
   Purge DIM/CROSSED_OUT. Delete `SelectionChrome::{Fill,Marker,Tint}`,
   `TabsActiveCue`, `blend_role` border cross-fade, `FocusDiamond`.
4. **Capability**: swap quantize for `junie::downgrade`; delete
   `quantize_palette`/`separate_elevation`; rewrite `normalize_content_band`.
5. **Gates**: every row of §4 in the same change (the harness "will fight a
   token change" — it must be updated with it, not after).
6. **Presets/appearance/theme-picker removal** (`appearance.rs`,
   `widgets/theme_picker.rs`, `ThemePackage::builtins()`, lookbook
   `lookbook_system(palette)` → `lookbook_system()`; showcase `app.rs:79` →
   `DesignSystem::junie()`; example `showcase.rs` `t`-toggle deleted).
7. **Bless**: `mise run bless-previews` (15 goldens), `mise run bless-pngs`
   (123 PNGs), `bun run build:preview-posters` + `--check`, regenerate
   `docs/src/generated/catalog.ts` and `docs/api/public-api.txt`.
8. **Per-widget follow-up** (the automatic pass gets colors right; these change
   *anatomy*): `tabs.rs` (2-row accent underline), `row_chrome.rs`
   (marker-only unfocused selection), `primitives.rs` (on-accent primary, 140 ms
   press, `label + 2` width), `text_input.rs`/`text_area.rs`/`code_block.rs`
   (editing = accent underline + cursor + ` EDIT ` badge + trailing bold `!`),
   `diff.rs` (added `#ffffff` / removed `#e44545`), `badge.rs` (no fill),
   `charts.rs` (value-ramp series), `toast.rs` (**delete** — junie has no toast),
   `hint_bar.rs`/`kbd.rs` (key BOLD `#ffffff`, action `#808080`), `status_bar.rs`
   (one row, `#b3b3b3`, right-edge status 4–5 s), `surface.rs` (plane ladder +
   backdrop walk), `highlighted_text.rs` (find match `#3f3f46`).

**Automatic (color-only, no per-widget work):** the other ~135 widget files and
35 patterns — anything that only projects `Role`s or recipes
(`list.rs`, `table.rs`, `tree.rs`, `dialog.rs`, `panel.rs`, `form.rs`,
`select.rs`, `progress.rs`, `scroll_area.rs`, `markdown.rs`, `transcript.rs`,
`agent.rs`, …). **Needs follow-up:** the 13 files above + `code_block.rs`'s
deleted ANSI fallback palette.

## 6. Risks and mitigations

1. **Ladder floor collapse** (1.11/1.07/1.19 < 1.15) makes the depth principle
   unsatisfiable. *Mitigation:* rewrite the gate as "strictly increasing planes"
   + the exact `KNOWN_SHORTFALLS` above in the same commit; depth re-carried by
   frame brightening (`#4d4d4d`), title row bar, and inset.
2. **~1.1k lookbook stories** (`crates/termrock-lookbook/src/stories.rs`) all
   regenerate; `check`/`render --check`, `export-preview-posters --check`,
   `docs.yml` render-twice `diff -r` all flip at once. *Mitigation:* land steps
   1–6 in one PR, then a single mechanical bless PR (`TERMROCK_BLESS_PREVIEWS=1`,
   `TERMROCK_BLESS_PNGS=1`); never hand-edit an artifact; keep the "double-render
   mismatch is a pipeline bug, do not bless" rule from `png_baselines.rs:43`.
3. **`capability_pty` env fixtures** (`EnvHints::fixture*`) and
   `no_color_forces_monochrome_boundary` encode Reset+REVERSED mono.
   *Mitigation:* rewrite the boundary expectation to gray buckets in step 4;
   the fixtures themselves are env-independent (f51d0ba8) and stay.
4. **Public-API + catalog drift**: deleting `ThemePicker`, `Toast`,
   `AppearanceThemeMap`, five presets, and 8 `Role` variants breaks
   `docs/api/public-api.txt` (cargo-public-api) and
   `generate-catalog.ts --check` (193 pages). *Mitigation:* regenerate both in
   the same PR; the docs pages for removed identities are deleted, not
   redirected (no-legacy law).
5. **`Role::Info` deletion touches 47 sites** (+ InfoStrong/InfoDim = 65),
   mostly status/semantic-status vocabulary. *Mitigation:* mechanical
   sed-class migration (`Info → Text` for headings, `→ TextMuted` for
   annotations), gated by `cargo test -p termrock --test design_gate` +
   clippy `-D warnings`; no new "info" tone may be introduced later.
6. **DIM purge** touches muted/disabled/loading paths across ~30 files and
   `motion.rs::fade_style`. *Mitigation:* separate commit inside the same PR
   series; `fade_style` collapses to alpha-free BOLD/ITALIC/REVERSED only.
7. **Green budget inflation**: junie spends `#48e054` in more places than
   termrock's law allowed, so `accent_budget` will fail on frames that are
   *correct*. *Mitigation:* re-derive the budget per surface class (§4) before
   blessing, not by bumping the constant to make CI green.
8. **PNG bit-exactness across arch** (open assumption A3) — new values are pure
   RGB over the same vendored fonts, so raster stays deterministic; the risk is
   the *count* of baselines changing (123 → fewer once `Toast` stories go).
   *Mitigation:* bless after the deletions, so baselines are written once.
