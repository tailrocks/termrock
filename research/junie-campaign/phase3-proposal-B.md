# Phase 3 — Proposal B (independent designer, adversarial self-review)

Date 2026-09-02. Reference: `junie-tui` at `/Users/donbeave/Projects/terminal-components-claude`
(`src/theme.rs` 678 lines, `DESIGN.md` 987 lines). Target: termrock. Written before reading any
other phase-3 proposal. Read set: campaign-plan.md, termrock-inventory.md, verification-infra.md,
reference theme.rs + DESIGN.md, termrock `style/{mod,tokens,palette,appearance,contrast_floor,
quantize,glyph,density,motion}.rs`, `tests/design_gate.rs`.

## 0. Mapping principles (stated before the table, so the table is checkable)

1. **TermRock's Role enum is a transport, not a design system.** junie's system is 26 tokens +
   8 state resolvers. I keep the role *addresses* widgets already call, re-point every value to a
   junie token, and move everything junie expresses as a *resolver over state×background*
   (hover, press, focus, disabled, thumb-focus) out of roles into `DesignSystem` methods.
2. **A role with no junie token and no derivable job is deleted, not invented.** junie's
   "explicitly absent" list (toast, badge, shadows, dim/reverse, Title Case) plus "declared but
   dormant" (`accent_bg_subtle`, `error_bg`, `info`) bound what I may map.
3. **junie spends one hue on exactly 11 things** (DESIGN.md Colors→Accent + Do/Don't): focus
   gutter `▎`, primary button fill, `›`/`✓` markers **on the focused row**, active document-tab
   `━`, ` EDIT ` badge, spinner + indeterminate sweep, completed progress `✓`, required `*`,
   selected tree label, checked `[✓]`. Nothing else may resolve to `#48e054`.
4. **Modifiers: junie uses BOLD, ITALIC, UNDERLINED (3 meanings), CROSSED_OUT (deleted grid rows)
   — never DIM, never REVERSED** ("Dim and reverse-video attributes are never used"; pressed is
   *drawn* canvas-on-white). Every `.dim()` and every `Modifier::REVERSED` in the palette dies.
5. **Italic means absence of value only** (`NULL`, `DEFAULT`, code comments) — a text *tier* may
   not carry it, so `TextFaint` loses its `.italic()`.
6. Planes are ranked by real luminance: `#000000 < #111111 < #18181b < #1e1e22 < #232328 <
   #27272a < #3f3f46`. termrock's ladder order (Raised between Surface and Elevated) does not
   survive contact with these numbers; I re-order rather than revalue.

## 1. Role → junie token map (all 63; 55 survive, 8 deleted)

| # | Role | junie token | Exact value | Note |
|---|---|---|---|---|
| 1 | Canvas | canvas | bg `#000000` | was `Reset` — rewrite |
| 2 | Surface | surface | bg `#111111` | card plane |
| 3 | Raised | — | **DELETE** | junie has no third in-flow plane; cards are `#111111`; `Elevation::Raised` → `Surface` |
| 4 | Elevated | surface_elevated | bg `#18181b` | dialogs, popups, picker |
| 5 | Sunken | field | bg `#1e1e22` | the input well; junie's `field` is exactly this |
| 6 | Backdrop | text_ghost | fg `#262626` | dimmed-cell answer tier |
| 7 | Text | text_primary | fg `#ffffff` | body/value = white, no modifier |
| 8 | TextStrong | `title()` | fg `#ffffff` + BOLD | titles, focused labels, key names |
| 9 | TextMuted | text_secondary | fg `#b3b3b3` | supporting text, unfocused labels, busy labels, tab labels, code str/num, active progress fill |
| 10 | TextDisabled | disabled (= text_faint) | fg `#4d4d4d`, **no modifiers** | distinct by structure (no hit, no ring), not chroma |
| 11 | Border | border_subtle | fg `#262626` | |
| 12 | BorderFocused | border_strong | fg `#4d4d4d`, **no BOLD** | focused frame is a brighter *gray*; green goes to the `▎` bar. Title still bolds |
| 13 | Selection | `selection()` | fg `#ffffff` bg `#3f3f46` | becomes **text/range selection only** (popover patch); never row selection |
| 14 | Focus | focus = accent | fg `#48e054` | the `▎` bar color |
| 15 | Accent | accent | fg `#48e054` | the 11-item budget (§0.3) |
| 16 | Success | success = accent | fg `#48e054` | junie aliases it deliberately |
| 17 | Warning | warning | fg `#f59e09` | `•`/`▲` carriers, pending, changed values |
| 18 | Danger | error | fg `#e44545`, **no bold** | bold belongs to the trailing `!`, not the tone |
| 19 | Info | info (dormant) | fg `#8787ff` | exists in the reference struct; no new consumers |
| 20 | Link | — (no link component) | fg `#b3b3b3` | derivation: interactive text = secondary; underline is its affordance |
| 21 | LinkHover | border-strong underline class | fg `#ffffff` + UNDERLINED | "quiet affordance" family |
| 22 | Input | `field_style` | fg `#ffffff` bg `#1e1e22` | |
| 23 | InputInvalid | error tone, plane unchanged | fg `#e44545` bg `#1e1e22` | junie: the field plane **never** changes for invalid; underline turns error |
| 24 | ScrollTrack | border_subtle | fg `#262626` | `│` |
| 25 | ScrollThumb | `scrollbar_thumb(f,h)` rest | fg `#808080` | focused `#ffffff`, hovered `#b3b3b3` become resolver args |
| 26 | TabActive | tabs textColor + `title()` | fg `#ffffff` + BOLD | green lives in the `━` underline glyph, not the label |
| 27 | TabInactive | tabs textColor | fg `#b3b3b3` | DESIGN.md frontmatter says `text-secondary` |
| 28 | TabActiveHovered | — | **DELETE** | junie tabs do not lift; only `×` brightens from faint |
| 29 | TabInactiveHovered | — | **DELETE** | |
| 30 | HintKey | `key_hint_key()` | fg `#ffffff` + BOLD | |
| 31 | HintText | `key_hint_action()` | fg `#808080` | |
| 32 | HintDim | text_faint | fg `#4d4d4d` | |
| 33 | HintSeparator | text_faint | fg `#4d4d4d` | ghost `#262626` is backdrop-only, so this is the quietest live tone |
| 34 | ActionFocused | primary button | fg `#19191c` bg `#48e054` + BOLD | numerically = junie's primary recipe; hover `#3ab343`, press `#2b8632` are resolver states, not roles |
| 35 | ActionDisabled | disabled | fg `#4d4d4d`, no dim | subtle variant keeps its container bg |
| 36 | StatusBar | status message tone on canvas | fg `#b3b3b3` bg `#000000` | footer carries no fill in junie and is excluded from backdrop |
| 37 | DiffAdded | success/accent (marker only) | fg `#48e054` | derived: `+` marker green, **content text stays on the ladder** |
| 38 | DiffRemoved | error (marker only) | fg `#e44545` | derived: `−` marker red; removed content `#4d4d4d` + CROSSED_OUT |
| 39 | SyntaxKeyword | `syntax(Keyword)` | fg `#ffffff` + BOLD | |
| 40 | SyntaxString | `syntax(Str)` | fg `#b3b3b3` | |
| 41 | SyntaxComment | `syntax(Comment)` | fg `#4d4d4d` + ITALIC | |
| 42 | SyntaxNumber | `syntax(Number)` | fg `#b3b3b3` | junie: numbers share the string tier |
| 43 | SyntaxFunction | `syntax(Ident)` | fg `#ffffff` | junie has no Function tone |
| 44 | SelectionTint | accent_bg | bg `#0f2e13` | selected **and focused** rows only (§3c) |
| 45 | HoverTint | — | **DELETE** | hover = `DesignSystem::lift(bg)`, one plane up (§2) |
| 46 | ActionConstructive | — | **DELETE** | creation is not in the accent budget |
| 47 | DisclosureHeader | section heading | fg `#4d4d4d`, no bold | junie: sidebar section headings are text-faint, sentence case |
| 48 | InfoStrong | — | **DELETE** | junie has no strong/dim variants; dimming is the ladder |
| 49 | InfoDim | — | **DELETE** | |
| 50 | ActorUser | text_primary | fg `#ffffff` | derived: actors separate by label+glyph first, ladder second |
| 51 | ActorAssistant | text_secondary | fg `#b3b3b3` | |
| 52 | ActorThinking | text_muted | fg `#808080` | no italic (italic = absence of value) |
| 53 | ActorTool | text_faint | fg `#4d4d4d` | |
| 54 | ActorPlan | warning | fg `#f59e09` | a plan is pending work = junie's warning class |
| 55 | ActorSystem | info | fg `#8787ff` | the only remaining declared hue; weakest derivation (§7) |
| 56 | ChartSeries1 | text_primary | fg `#ffffff` | derived: charts are achromatic; ramps separate series in mono |
| 57 | ChartSeries2 | text_secondary | fg `#b3b3b3` | |
| 58 | ChartSeries3 | text_muted | fg `#808080` | |
| 59 | ChartSeries4 | text_faint | fg `#4d4d4d` | |
| 60 | ChartAxis | text_muted | fg `#808080` | headers/meta tone |
| 61 | ChartGrid | border_subtle | fg `#262626` | |
| 62 | TextFaint | text_muted | fg `#808080`, no italic | metadata, counts, timestamps, helpers, headers |
| 63 | BackdropWash | — | **DELETE** | junie dims per cell with `Theme::backdrop()`; there is no wash fill |

Unmapped junie tokens and where they live instead: `field_hover #232328` and `popover #3f3f46`
are `lift()` arms; `accent_hover #3ab343` / `accent_pressed #2b8632` are primary-button states;
`accent_bg_subtle #0a1c0c`, `error_bg #2e0f0f` are **not ported** (junie: "not part of the
system; do not introduce them").

`ROLE_COUNT` 63 → 55. Recipe structs (`ButtonRecipe`, `InputRecipe`, `ListRowRecipe`,
`PanelRecipe`, `SurfacePaintPlan`) stay as the widget-facing shape — 144 widget files already
consume them — but their bodies become literal ports of `theme.rs::{row,button,field_style,
placeholder,gutter,scrollbar_thumb,selection,border,lift,backdrop,tone,syntax,badge}`.

## 2. Capability ladder — replace termrock's algorithm with the reference's, verbatim

**Decision: delete `quantize.rs`'s mapping math, port `theme.rs:552-621` byte-for-byte.**

Evidence (same input, two algorithms):

| Input | junie `nearest_256` | termrock `rgb_to_xterm256` | junie `nearest_16` | termrock `rgb_to_ansi16` |
|---|---|---|---|---|
| accent `#48e054` | `Indexed(78)` | `Indexed(77)` | LightGreen | LightGreen |
| border_subtle `#262626` | 232 | 233 | **DarkGray** | **Black** — border invisible on black canvas |
| surface `#111111` | 232 | 233 | DarkGray | DarkGray |
| warning `#f59e09` | (cube) | (cube) | **Yellow** | **LightYellow** |

Two of the four canonical tokens land on the wrong value today, and one of them (the border)
erases chrome entirely at 16 colors. termrock's algorithm is tuned for a phosphor ladder that no
longer exists (`NEAR_GRAY_SPREAD`, `NEUTRAL_CHROME/MUTED/BODY` cuts at 48/120/208,
`CHROMATIC_FLOOR`, `BRIGHT_FLOOR`), and `separate_elevation` exists only because that ladder
collided on the gray ramp. junie's planes are ≥1 ramp step apart under its own mapping
(`#000000`→16, `#111111`→232, `#18181b`→233, `#1e1e22`→234, `#27272a`→236, `#3f3f46`→238), so
`separate_elevation` and `ELEVATION_LADDER` are dead code under the new palette → delete.

Port, unchanged: `downgrade`, `nearest_256`, `nearest_16`, the mono 4-bucket
(`≤40 Black / ≤110 DarkGray / ≤190 Gray / else White`), `ColorLevel::detect()` ordering
(`NO_COLOR` **non-empty** → Mono; `COLORTERM=truecolor|24bit`; `TERM` containing
`256color|ghostty|kitty`; else Ansi16), and `Theme::for_level` mapping over the whole token set.

Delete with it:
- `quantize_style`'s `Modifier::REVERSED` substitution — junie's mono is a **gray ladder**, not
  `Reset`, and "reversed is drawn explicitly as canvas-on-white". Fills that lose their bg keep
  their shape via the ladder + the non-color cue, same as the reference.
- termrock `Monochrome ⇒ Color::Reset` — replaced by the 4-bucket named grays.
- `Ansi16Color`'s role in authoring: the canonical palette is RGB; names appear only as
  `downgrade` output. `Ansi16Color` survives purely as the name→`Color` table the downgrade
  returns through (it is the same 16 names ratatui already has — consider deleting the enum and
  returning `Color::Black` etc. directly).
- `detect_from_env`'s `TERM contains "truecolor" → Indexed256` arm (junie has no such arm).

Keep: `ColorCapability` (rename fields to junie's `TrueColor/Ansi256/Ansi16/Mono` is optional;
the *ladder semantics* are what must match), `quantize_palette`'s loop over roles, and
`degrade_chrome` reduced to "Mono ⇒ nothing chromatic" (no `GlyphSet::Ascii` arm — §4).

Gate that proves it: a literal vector table ported from the reference's own tests plus the four
rows above (`#48e054`→`Indexed(78)`/LightGreen/Gray-mono, `#e44545`→LightRed, `#000000`→Black,
`#262626`→DarkGray-at-16), asserted at all four levels for every role.

## 3. Gates, tests, and docs that contradict the junie system

**Rewrite-to-junie-law** unless marked **DELETE**. "Rewrite" = keep the harness, change the law.

| Site | Contradiction | Action |
|---|---|---|
| `style/mod.rs:907` `phosphor_baseline_uses_named_ansi_only` | forbids the exact RGB that IS the system | **Rewrite** → `junie_baseline_is_exact_rgb`: every role carries `Color::Rgb` from the table in §1, and `for_level` downgrade matches the §2 vector table |
| `style/mod.rs:933` `accents_are_distinct` | asserts Success/DiffAdded/Focus ≠ Accent; junie aliases all three to `#48e054` | **Rewrite** → the remaining arms (ScrollThumb `#808080`, HintText `#808080`, TabActive `#ffffff`, ChartSeries1 `#ffffff`, Border `#262626`) plus the §0.3 budget list as the new law |
| `style/mod.rs:1029` `terminal_native_inherits_terminal_background` | terminal-native surfaces contradict a token canvas | **DELETE** with `RolePalette::terminal_native()` |
| `style/mod.rs:853` `default_separates_ordinary_and_strong_text`, `:1084` `phosphor_preset_pins_load_bearing_role_values` | pin ANSI-16 named values + DIM/italic modifiers | **Rewrite** to the §1 values; DIM/CROSSED_OUT/italic assertions removed from tiers |
| `style/mod.rs:1133` `slate_preset_pins_load_bearing_role_values` | second design system | **DELETE** with the slate preset |
| `style/mod.rs:921` `faded_named_ansi_stays_in_named_terminal_space` | `faded()` blends toward black; junie dims by stepping the ladder | **DELETE** with `faded()`; consumers re-point to the ladder |
| `style/appearance.rs:126-168` (4 tests) + `mod.rs:19` re-export | light/dark auto-mapper; junie ships one dark theme | **DELETE** module. Verified: `Appearance`/`palette_for_appearance` have **zero** consumers outside `style/` (only unrelated "Appearance" UI strings) |
| `style/tokens.rs:812` `ThemePackage::builtins()` (6 presets), `:879-930` `phosphor/slate/paper/ansi/high_contrast/terminal_native/adaptive` | six design systems; junie = one theme × 4 capability levels | **DELETE** slate/paper/ansi/high-contrast/adaptive/terminal_native; ship one package + `--color truecolor\|256\|16\|none` |
| `style/tokens.rs:1146` `border_set()` Ascii `+-|` branch | junie's frames are always `╭╮╰╯` | **Rewrite** → `ROUNDED` always; delete the Ascii branch with `GlyphSet::Ascii` |
| `style/tokens.rs:1551` `input_recipe` prompt glyph `❯` + `:1532` focused border | junie's field cue is `▎` + bold label + accent underline; plane and border never change | **Rewrite**: drop `prompt`, border constant `border_subtle`, focus = `▎` + BOLD label + accent underline |
| `style/tokens.rs:1417-1443` button Focused/Hovered/Pressed | termrock focus = BOLD border + REVERSED label; pressed = SelectionTint | **Rewrite** to `theme.rs:367-431`: primary hover `#3ab343`, press `#2b8632`; secondary/subtle press = explicit `#000000` on `#ffffff`; danger press = `#ffffff` on `#e44545`; disabled ignores hover |
| `style/tokens.rs:1583` `resolve_list_row` | selection = gutter+tint whenever selected | **Rewrite** to `theme.rs:309-339`: tint iff selected**&&focused**; marker-only otherwise; hover = `lift(bg)`; pressed = canvas-on-white; busy = secondary |
| `style/tokens.rs:1328-1356` panel `title_prefix` FocusDiamond `◊`, border BOLD | junie cards show focus as `▎` in the title inset + bold title; frames brighten subtle→strong | **Rewrite**: `title_prefix` = `▎` (Focused) / `!` (Danger); drop border BOLD |
| `style/palette.rs:81` `lift()` (luminance lighten/darken + ANSI bright twins) | junie's lift is a **plane ladder**, never a colour computation | **Rewrite** → `theme.rs:342-352`: canvas→`#18181b`, surface/elevated→`#27272a`, field→`#232328`, else→`#3f3f46` |
| `style/contrast_floor.rs:56` `KNOWN_SHORTFALLS` | lists paper/high-contrast ladder rows of deleted presets | **Rewrite** → exactly junie's four declared sub-AA pairings (below), presets table → one entry |
| `style/contrast_floor.rs:453` `disabled_and_faint_tiers_stay_distinguishable` | survives numerically (`#808080` 5.32 vs `#4d4d4d` 2.48 → gap 2.84) | keep, re-valued |
| `style/quantize.rs:200` `separate_elevation`, `:233` REVERSED substitution, `:514`/`:536`/`:555` tests | phosphor-ladder repair; mono-as-Reset | **DELETE** |
| `widgets/surface.rs:544` `canvas_uses_terminal_default_fill`, `:746` `phosphor_surfaces_keep_semantic_elevation` | Canvas = `Reset`; rung order Surface<Raised<Elevated | **Rewrite**: Canvas = `#000000`; rungs = `#111111` (Surface) < `#18181b` (Elevated/overlay) with `#27272a` as the *hover/secondary-fill* plane via `lift()`, `#1e1e22` well; assert the exact hex + the `lift()` ladder instead of "distinct rungs" |
| `tests/design_gate.rs:443` `collections_share_one_gutter_glyph` | `▌` on **selected** rows | **Rewrite** → `focused_rows_carry_the_bar_and_selected_rows_carry_a_marker`: `▎` on the focused row of List/Table/Tree/Timeline; `›` on selected, `✓` on checked |
| `tests/design_gate.rs:543` `interaction_underline_is_dead` (7-entry whitelist) | junie *requires* underline for editing (accent), quiet affordance (border-strong), diagnostics (error/warning) | **Rewrite** to junie's three-colour underline law; whitelist grows to the editing/affordance/diagnostic sites (text_input, text_area, data_table cells, completion, code editor) |
| `tests/design_gate.rs:594` `tab_palette_roles_are_underline_free` | still correct (junie's tab rule is a `━` glyph, not SGR underline) | keep, values updated |
| `tests/design_gate.rs:696` `no_widget_paints_selection_fill_by_default` | junie **does** fill the selected+focused row | **Rewrite** → new invariant (next block) |
| `tests/design_gate.rs:387` `a_focused_field_says_so`, `:1462` `inputs_share_field_chrome` | expect well change + prompt cue + BorderFocused | **Rewrite** to `▎`+bold label+accent underline, well constant `#1e1e22` |
| `tests/design_gate.rs:1394` `accent_budget` (cell count) | junie's budget is a *use list*, not a count | **Rewrite**: accent cells allowed only in the gutter column of a focused row, the marker slot, a primary-button rect, the active-tab underline row, the EDIT badge, spinner/progress cells, `*`, selected tree label, `[✓]` — assert by position, keep a count ceiling as a tripwire |
| `tests/design_gate.rs:1831` `bold_budget_per_row` | junie **removes** bold from chrome inside a focused row | **Rewrite**: exactly one bold run per focused row (the label); tab prefixes/kind glyphs/row numbers/completion details non-bold |
| `tests/design_gate.rs:299/:334` motion gates | termrock transitions/ambient loops; junie has two motion facts | **Rewrite**: 140 ms pressed flash, 80 ms spinner tick, nothing else animates |
| `tests/design_gate.rs:353` `spinner_frames_one_column` | survives | keep (termrock's braille frames == junie's `⠋⠙⠹⠸⠼⠴⠦⠧⠇⠏`) |
| `tests/design_gate.rs:1872` `state_matrix_distinct` | pressed/disabled expectations | **Rewrite** values (§1, §3 button rows) |
| `tests/design_gate.rs:2094` `no_wide_emoji_in_chrome` | survives; trivially once `GlyphSet::Enhanced` dies | keep |
| `tests/design_gate.rs:1548` `data_rows_have_ladder`, `:983` `pattern_style_diversity`, `:2182` monochrome evidence, `:2252` reduced motion, `:2354` button focus without color | ladder tones, mono = Reset+REVERSED, `MotionPolicy::Basic` cap | **Rewrite** to ladder values + gray-ladder mono + Full/Off |
| `lookbook/tests/design_gate.rs` CROSSED_OUT probe | mono is now a gray ladder | **Rewrite** probe to the ladder + junie survival list (`▎`, bold, edit underline, reversed cursor cell, `! › ✓ •`) |
| `docs/design/termrock-design-language.md` (§5 underline-free binding law, §10 phosphor identity), `terminal-design-system.md` §9 (`#00ff41` swatches), `phosphor-obsidian-visual-direction.md`, `web-premium-tui-law.md` §4.1 | four binding docs describe a different system | **DELETE all four**, ship one `docs/design/DESIGN.md` = junie's DESIGN.md re-based on termrock's widget inventory |
| `crates/termrock-lookbook/src/main.rs:154-163` `--theme phosphor\|slate`, `app.rs:577` Ctrl+Alt+T, `examples/showcase.rs` `t`, `widgets/theme_picker.rs` | theme gallery | **DELETE** slate arm + picker; flag becomes `--color` (junie's own surface) |
| `.github/workflows/docs.yml:119` slate render | renders a deleted theme | **DELETE** step |

### The precise replacement invariant for selection fill

`selected && focused` ⇒ row bg = `Role::SelectionTint` (`#0f2e13`), label `text_primary` + BOLD,
`▎` in column 0 (`#48e054`), marker `›`/`✓` in column 1 (`#48e054`).
`selected && !focused` ⇒ **no fill**, label `text_primary`, marker `›`/`✓` in `text_primary`,
gutter cell painted in the row's own bg (hidden, `theme.rs:287`).
`hovered` ⇒ bg = `lift(row_bg)` and **hover replaces the tint** (a hovered selected+focused row
lifts instead of tinting, `theme.rs:319-321`). `pressed` ⇒ explicit `#000000` on `#ffffff` + BOLD.
`disabled` ⇒ `#4d4d4d`, no bar, no hover. The fill is bounded to the row rect the widget owns —
it never bleeds into gutters it does not own. Name the rewritten gate
`selection_tint_requires_focus_and_hover_wins`.

### contrast_floor `KNOWN_SHORTFALLS` (new, exact, measured)

```
("junie", "button Primary/Pressed on_accent on accent_pressed"),  // #19191c on #2b8632 ≈ 3.72
("junie", "button Danger/Default label on overlay"),              // #e44545 on #27272a ≈ 3.71
("junie", "button Danger/Pressed text_primary on error"),         // #ffffff on #e44545 ≈ 4.01
("junie", "input placeholder text_muted on field"),               // #808080 on #1e1e22 ≈ 4.21
```
Same exactness contract as today: a pair that starts passing must be removed in the same commit.
junie's justification carries over verbatim — none of the four carries information not also
present as a glyph, a label, or the value that replaces the placeholder.

## 4. Glyph law

junie's table is closed: `▎ › ✓ • + − ! ▲ ▸ ▾ ▴ ▾ ∇ ▪ → ↓ ‹ › … × ● ○ [✓] [ ] ◆ ◇ ─ ━ │ ┃ ╭╮╰╯`
+ braille `⠋⠙⠹⠸⠼⠴⠦⠧⠇⠏` @80 ms. TermRock's `Glyph` (68 variants) re-pointed:

**Re-point (value changes only):** `SelectionGutter` `▌`→`▎` (and rename → `FocusBar`: it is the
keyboard-focus bar; selection is a marker) · `SelectionMarker` `▸`→ delete, selected rows use
`›` (`ChevronRight`) · `ChevronUp` `ˆ`→`▴`, `ChevronDown` `ˇ`→`▾` (junie sort/select) ·
`Success` `✓` ✓ keep · `Error` `✕`→`!` · `Warning` `!`→`•` (junie's modified/pending mark) ·
`Close` `✕`→`×` · `Info` `·` → **delete** (status is tone+label) · `Loading` `◔`→`⠋` (spinner
frame 0; the spinner is the only activity glyph) · `Busy` `◐` → delete · `CheckOn` `☑`→`[✓]`,
`CheckOff` `☐`→`[ ]` · `RadioOn` `●`/`RadioOff` `○` keep (junie paints the parens literally:
`▎(●) option`) · `RuleHStrong` `═`→`━` (junie's active rule) · `Remove` `−`, `Add` `+`,
`Bullet`/`ModeDot` `●`, `EmptyCircle`/`StatusDotHollow` `○`, `DiamondFilled` `◆`, `NowEdge` `◇`,
`Mask` `●`, `SliderThumb` `●`, `SliderFill` `━`, `SliderRail` `─`, `DividerVertical` `│`,
`DividerVerticalActive`/`RailHeavy` `┃`, `DividerHorizontal` `─`, `DividerHorizontalActive` `━`,
`MetaSeparator` `·`, `Ellipsis` `…`, `DisclosureOpen` `▾`, `DisclosureClosed` `▸`,
`ChevronLeft` `‹`, `ChevronRight` `›`, `ArrowDown` `↓`, `ArrowUp` `↑` — all already junie glyphs.

**Delete (no junie glyph, and the derived widgets must not invent one):** `File` `▫`,
`Folder` `▪`, `FolderOpen` `▨` (junie's tree uses *kind letters* `D S T V ƒ #`; `▪` is reserved
for the identity mark / pk) · `Search` `/` (a key, not a glyph) · `Settings` `⚙` · `Edit` `✎`
(the edit affordance is the ` EDIT ` badge) · `Copy` `⧉` (`y` is a key) · `Play` `▶`, `Stop` `■`
(progress suffixes are `✓ ! ‖`) · `CheckMixed` `▣` (no tri-state in junie; show `N of M`) ·
`RuleTeeLeft/Right` `├┤` (no tees; frames are `╭╮╰╯`, no column walls) · `Connection` `◍` ·
`SelectionMark` `▮` · `FocusDiamond` `◊` · `DisabledMark` `⊘` (disabled = faint text, no glyph) ·
`DiamondDouble` `◈`, `StatusDotTarget` `◉`, `StatusDotRing` `◎`, `RailCollapsed` `❙`,
`SliderTick` `┊`, `DividerVerticalHint` `┋`, `DividerHorizontalHint` `┅`, `Token` `◧`,
`SPINNER_DOT_PULSE_FRAMES`.

**Profiles: delete `GlyphSet::Ascii` and `GlyphSet::Enhanced`.** junie has neither, and it states
the opposite law — the glyph table is fixed and only *colour* degrades ("What must survive at
every level: the `▎` bar, bold, underline, the reversed cursor cell, `!`, `›`, `✓`, `•`"). I
argue keep-behind-flag and reject it: (a) there is no reference behavior to preserve behind the
flag, so the flag preserves an unverifiable invention; (b) its existence is what lets two
encodings per glyph and the `GLYPH_CONTEXTS` collision machinery exist; (c) `Enhanced`'s emoji
(`📁📄🔍`) already violate termrock's own `no_wide_emoji_in_chrome`. Consequences:
`GlyphSet` becomes a marker type (or dies; keep it as the `DesignSystem` field for API shape),
`mono()` loses its `is_ascii()` arm, `no_color()` = `quantize(Mono)` only, `degrade_chrome`
loses two of its three branches, `border_set()` is always `ROUNDED`, and ~30 widgets drop their
`ascii_glyphs()` branches. `GLYPH_CONTEXTS` is rewritten to junie's law: one glyph may carry
several meanings across *non-co-occurring* contexts (`●○` = toggle/radio/switch; `▾` = tree and
select, "disambiguated by context and should not be extended"); the enforced invariant stays
"no two meanings inside one context", now with junie's declared exception documented instead of
engineered away.

## 5. Density, motion, appearance

**Density: delete the enum, keep tokens.** junie has one density and answers pressure by
*prioritised dropping*, never by re-spacing. `Density::{Comfortable,Compact,Dashboard}` →
deleted; `SpacingScale` becomes junie's named token table with canonical values as pub fields:
`gutter 1, inline 1, gap 2, column_gap 2, form_gap 4, card_inset 2, frame_inset 3,
dialog_inset_x 3, dialog_inset_y 2, tree_indent 2, field_height 3, tabs_height 2, min 72×20`.
Tunables-with-canonical-defaults: yes for the token *values* (a host may override a field), no
for inventing new tokens ("Don't invent a spacing value"). Add the 72×20 four-line
"Terminal too small" notice as a gated behavior (`a_scrolled_region…`-style test).

**Motion: two facts, two consts.** `ACTION_FLASH_MS` 1_000 → **140**; spinner tick stays **80**
(`MotionChannel::Work.period_ms()` already 80 ✓). Delete: `MotionPolicy::Basic` +
`BASIC_TRANSITION_CAP` (a 120 ms cap contradicts a 140 ms flash), `MotionChannel::{Live,Wait,
Stream}` (no breathe/shimmer/dot-pulse in junie), `Easing`/`smoothstep`, `shimmer_at`,
`wave_brightness`, `pulse_brightness`, `edge_fade`, `fade_style`, `effective_alpha`,
`coalesce_cells`, `blend_toward`/`blend_role` cross-fades (junie's focus border snaps subtle→
strong; nothing transitions). `MotionPolicy` = `{Full, Off}`; `MotionSemantics::StateTransition`
→ `Static`. Keep as canonical consts, not tunables: `PRESS_FLASH_MS = 140`,
`SPINNER_TICK_MS = 80`, plus `STATUS_MESSAGE_MS = 5_000` for the footer status TTL. `FrameTick`
stays (the spinner needs it). Gates `motion_policy_off_is_static` / `..._actually_animates` are
rewritten to assert exactly those two channels.

**Appearance: delete** (`style/appearance.rs`, 169 LOC + re-export + 4 tests). Zero consumers
verified; junie has no light theme and no auto-mapping.

## 6. Blast radius and ordered migration (verification at every step)

Touch set: `style/` (9 files), 144 widget files, 35 pattern files, lookbook `design.rs`/`main.rs`
/`app.rs`/`demo.rs`/`frame.rs`/`stories.rs` (Enhanced + preset refs), `examples/showcase.rs`,
`theme_picker.rs` (delete), `termrock-raster` (unchanged — it paints whatever the buffer says),
docs (4 design .md deleted, 1 added, 193 component pages, catalog.ts, 227 posters), baselines
(15 goldens, 123 PNGs).

| # | Step | Verification at the checkpoint |
|---|---|---|
| 0 | Regenerate reference truth at junie HEAD (`verify/junie/bin/ref_capture.sh --all`, 40 scenes, `--color` × 4 levels) | `python3 verify/junie/bin/run.py --layer text --precomputed-reference` → 0 drift vs committed grids |
| 1 | Port `downgrade`/`nearest_256`/`nearest_16`/`detect`; delete `separate_elevation`, REVERSED-mono | new vector-table test + `cargo nextest run -p termrock --test capability_pty`; color-layer compare at 256/16 catches any drift |
| 2 | Re-point 55 roles, delete 8, delete presets + appearance + `faded` | `cargo test -p termrock --lib` (`junie_baseline_is_exact_rgb`, rewritten `accents_are_distinct`); **this is where fidelity can silently regress**: a role left unmapped still compiles and paints the old value — the exact-rgb gate is what catches it |
| 3 | Glyph re-point/deletes; delete `GlyphSet::{Ascii,Enhanced}` | `collections_share_one_gutter_glyph` (rewritten) + `every_catalog_encoding_matches_declared_width` + `no_wide_emoji_in_chrome`; `mise run preview-goldens` fails by design → **diff against reference `.txt` per scenario before blessing** |
| 4 | Recipes → junie resolvers; `lift()` plane ladder; underline law | `state_matrix_distinct`, `a_focused_field_says_so`, `inputs_share_field_chrome`, rewritten selection gate, `interaction_underline_is_dead` |
| 5 | Motion: 140/80, delete channels | rewritten motion gates; `cargo test -p termrock --test design_gate` |
| 6 | Widget + pattern sweep (179 files, mechanical: role/glyph re-pointing) | full `design_gate` + lookbook `design_gate`; `mise run check` |
| 7 | Bless downstream, in this order | `mise run bless-previews` (15) → `mise run bless-pngs` (123; **never** bless a render-twice determinism failure) → `bun run build:preview-posters` (227) → `bun run generate-catalog` |
| 8 | Docs: delete 4 design .md, ship one, re-render posters/pages | `bun run check:component-pages`, `check:preview-posters`, `check-preview-metrics`, remove `docs.yml:119` |

**Checkpoints where fidelity silently regresses, and the command that catches it:**

1. **Capability projections** (256/16/mono) — today they have zero render coverage; a wrong
   downgrade ships invisibly. Catch: new capability goldens (one text golden per scenario at
   `Ansi256`/`Ansi16`/`Mono`) + `run.py --layer color` against reference `.ansi` captured with
   `--color 256` and `--color 16`.
2. **Mass bless** — 15 goldens + 123 PNGs + 227 posters change in one commit, which hides one
   wrong glyph among 4000. Catch: `python3 verify/junie/bin/run.py --layer text` must pass per
   scenario *before* `mise run bless-*`; never bless a scenario whose text layer fails.
3. **Poster-only drift** — posters are JSON painted client-side; goldens do not cover them.
   Catch: `bun run check:preview-posters` in the same commit as the bless.
4. **Accent leakage** — a widget that quietly spends green shows only on big frames. Catch:
   the rewritten position-based `accent_budget`.
5. **Focus-bar width** — `▎` vs `▌` differ by one sub-cell and are nearly invisible in a PNG at
   9×18. Catch: `collections_share_one_gutter_glyph` (exact symbol assert) + text-layer diff at
   column 0.

## 7. Self-critique — the three weakest points, and what would change my mind

1. **`TextMuted` = `#b3b3b3` (junie *secondary*) while its name says "muted" and many of its
   paint sites (headers, placeholder, helper) match junie's `#808080`, which I gave to
   `TextFaint`.** If the paint-site audit (`grep -c TextMuted src/widgets`) shows headers and
   placeholders dominate over supporting text, the two assignments should swap, and every
   surface shifts one ladder step. Evidence to change my mind: that audit, plus a side-by-side
   render of a form and a table against `shots/f_forms*.txt`.
2. **Deleting `GlyphSet::Ascii`/`Enhanced`.** This is the one call that can make the UI
   *unreadable* rather than merely less faithful, on a terminal without box-drawing or braille —
   and junie's evidence for it is an argument ("must survive at every level"), not a measurement.
   Evidence to change my mind: a capture on a real `TERM=vt220`/`linux` console showing broken
   frames, at which point the honest fix is a *capability*-gated substitution computed at
   `detect()` (not a second authoring profile) — still not the current enum.
3. **Achromatic charts + the 6-actor ladder on a 4-step neutral scale.** junie has no charts and
   no agent domain; four series and six actors on four neutrals are ambiguous without glyphs or
   labels, and `ActorSystem = #8787ff` resurrects a token junie explicitly calls dormant.
   Evidence to change my mind: a mono render of a 4-series chart and a 6-actor transcript where
   the ramps/labels fail to disambiguate — then I would allow exactly one non-accent hue
   (`#8787ff`) for series-2/system and re-run the contrast floor, rather than re-introducing a
   per-series palette.

**Single riskiest call:** deleting the ASCII glyph profile (§7.2) — it is the only decision that
trades *legibility on legacy terminals* for fidelity to a reference that never tested them.
Everything else in this proposal degrades gracefully; that one does not.
