# Archived TermRock 48-surface designer audit — web-premium redesign

| Field | Value |
|-------|-------|
| **Status** | Archived 2026-08 snapshot. Not component inventory or current coverage authority; use the Rust inventory and generated docs catalog. |
| **Date** | 2026-08-14 |
| **Bar** | [`web-premium-tui-law.md`](./web-premium-tui-law.md) — "feels like it's NOT a terminal" (Kimi/Grok/Amp/Claude.ai) under phosphor. Properties P1–P15, rules §4, vocab §5, composer §6, toast §7. |
| **Feeds** | the `plans/` execution system. Foundations first, then per-widget, then decisions. |
| **Method** | 9 parallel research agents, source-verified (`file:line` read against HEAD), one per cluster. English only. Docs only — no code. |

> This doc turns the law into component-level work. Read **§1 Foundations** first: those
> ten shared primitives unblock the most widgets and are the real P0s. §2 is the per-component
> audit. §3 consolidates every open decision. §4 is the execution order.

---

## 1. Foundations — build these first (they unblock the most widgets)

The audit's central finding: most component defects share a handful of **missing structural
primitives**. Fixing them once heals dozens of widgets; fixing widgets first re-creates the
same bug class downstream. This is the "first find why the architecture allowed the bug class"
discipline applied.

### F1. Surface ladder is collapsed — depth never renders (P0 foundation)
- `widgets/surface.rs:447-448` — `Raised` and `Overlay` recipes **both** resolve to
  `Role::Elevated` (identical fill). `Role::Raised` (`style/mod.rs:126`) is never reached.
- `surface.rs:38-58` — `SurfaceRecipe` has **no `Sunken` variant**; `Role::Sunken` unreachable
  from any recipe. The 5-step ladder (Canvas→Sunken→Surface→Raised→Elevated) is physically
  impossible to express.
- `surface.rs:443-471` — `surface_recipe` never references `Role::Backdrop`; overlays paint
  Elevated+Border with **zero dim** toward Canvas.
- **Fix:** add `SurfaceRecipe::Sunken`→`Role::Sunken`; split `Raised`→`Role::Raised` (distinct),
  reserve `Elevated` for `Overlay` only. Overlay = `Elevated` fill + faint `BackdropWash` border
  + **mandatory backdrop dim** (`blend_toward(Canvas,0.6)`) over the occluded area before render.
  Add `ladder_is_monotonic` test over the five Role hexes. (Canvas = `Color::Reset` exception.)
- **Unblocks:** every depth consumer — Panel, Sidebar=Raised, composer=Raised, input well=Sunken
  (§6), code-block wells (P15), Toast fake-depth rear-pebble (P12).

### F2. `FocusEmphasis` enum + underline removal (P0 — kills the bug class)
- Root: `style/tokens.rs:816` `show_focus_underline: state.focused && state.selected` —
  structural root for the whole collection cluster's underline-as-focus.
- Kernel: `widgets/text_input.rs:994-1203` paints **no border**; focus = `Modifier::UNDERLINED`
  label (`text_input.rs:1023-1025`). `InputRecipe.border` (`tokens.rs:352`) has **zero readers**.
- ~14 widgets hand-roll `UNDERLINED` for focus/active/conflict: Tabs (`tabs.rs:1176,1178,1228`),
  Breadcrumbs (`breadcrumbs.rs:871-875`), Tag (`tag_chip.rs:364`), Chip (`tag_chip.rs:849`),
  KeyboardHelp (`keyboard_help.rs:1203`), Slider (`slider.rs:736-738`), RadioGroup
  (`controls.rs:1140-1142,1292,1304`), Stepper (`stepper.rs:860-862,881`), SearchInput
  (`search_input.rs:808-812`), NumberInput (`number_input.rs:1042-1044`), TokenField
  (`token_field.rs:1003-1005`), Combobox (`combobox.rs:944-952`), Select (`select.rs:829`).
- **Fix:** add `FocusEmphasis{BrightBorder, SelectionFill, FocusTint, Reversed, BoldKey,
  PillGlyph}` on `DesignSystem`. Delete `show_focus_underline`. Wire `TextInput::paint` to
  consume `InputRecipe.border` (border swap, never underline) — heals
  Number/Password/Search/Token/Combobox in one change. Three proven mechanisms: border swap
  (`Border`→`BorderFocused`), glyph ladder `○→◎→●`, bg wash (`HoverTint`/`SelectionTint`).
- **Underline survives only:** hovered links, ANSI SGR-4 / markdown / diff (content), cursor
  fallback.

### F3. `MotionChannel` + `shimmer_cells` primitive (P0 — perceived aliveness)
- `style/motion.rs:13-105` has only `pulse_brightness` + `wave_brightness`. **No `shimmer_cells`.**
- Skeleton uses a **global 4-step pulse** toggling the whole block
  (`skeleton.rs:498-504`) — not the traveling sweep the law requires.
- `LoadingView` (`view_state.rs:28-34`) takes a **static frame**; spinner never animates.
  `Spinner` has no Stream/Done phase (`spinner.rs:50-61`); reduced-motion fallback is `●`
  (collides with "done"). `StatusIndicator` has no motion at all (`status_indicator.rs:341-367`)
  — only `BOLD`. Timeline/LogStream/Toast all rebuild sweep ad hoc.
- **Fix:** add `MotionChannel{Work, Wait, Stream, Live, Static}` + a **period table** (Work
  ~80ms / Wait ~240ms / Stream ~120ms / Live breathe ~2s / heartbeat ~5s) + `shimmer_cells(tick,
  cols, period)` traveling sweep in `motion.rs`. Errors = Static (gravity). Add the
  `skeleton ≠ spinner` test (`shimmer_implies_no_spinner_frames`). Reduced-motion collapses to
  static with zero info loss (guaranteed by shape-before-color).
- **Unblocks:** skeleton sweep, streaming-log arrival, Toast success-pop, Timeline running-rail,
  StatusIndicator breathe, Slider thumb hover, Spinner Stream/Done morph.

### F4. `TokenRecipe` — one chip family (P0)
- `Tag.paint` (`tag_chip.rs:293`) and `Chip.paint` (`tag_chip.rs:768`) are **near-duplicate
  paint bodies** — parallel path (§4 rule 13). Both use `BOLD|UNDERLINED` remove-focus
  (`364`, `849`); both `Role::Selection` neon (`532`).
- Kbd keycap: brackets painted with key style, not split faint (`kbd.rs:562-564`); two divergent
  keycap renderings (`kbd.rs:535/557` vs `779`); `+` separator in Spelled mode (`kbd.rs:251-268`).
- **Fix:** one `TokenRecipe{BracketStyle{Angle, Square}, mark: Option<Glyph>, label, removable,
  status, state}` — Tag = angle `⟨ label × ⟩` (neutral), Chip = square `[● label ×]`
  (interactive), kbd = `[ C-s ]` (space-padded, brackets `Border` faint, label `HintKey+BOLD`).
  Remove-region = **invert** (`Danger` fg + `Surface` bg + `BOLD`, or `REVERSED` on ANSI-16) —
  never underline. Selection ramp **glyph>weight>fill>color** (drop fill before the mark when
  narrow). Chord separator = space (not `+`); `·` between hints.

### F5. Glyph catalog gaps (P0 — enabling)
- **Left-half blocks `▏▎▍▌▋▊▉` absent** (`glyph.rs:17` has only bottom-block `▁▂▃▄▅▆▇█`). Powers
  Histogram 1/8 bar-tops, Slider sub-cell thumb, Switch trailing-edge, Password strength meter.
- **Shade blocks `░▒▓` absent** (only inline literals at `charts.rs:1248,1758`). Powers
  monochrome multi-series, soft fills, backdrop stipple.
- **Mask split 3 ways:** `*` (TextInput/Password default), `●` (Password `.mask()`), `•`
  (InputOtp). Unify to `●` (U+25CF), ASCII `*`.
- **Slider bypasses catalog** (`slider.rs:187-203` — hardcoded `●/━/─/┊`). **SplitPane**
  (`split_pane.rs:382-393`) + **ResizablePanelGroup** (`:889-910`) bypass too.
- **Glyph collisions:** `▌` = checked **and** selected (TreeTable `tree_table.rs:1297-1303`,
  DataTable), selected row renders **blank on unfocus**; `▸` = disclosure **and** selected
  (DetailTable `detail_table.rs:17`).
- **Checkbox literals** `"[✓]"/"[ ]"/"[x]"` bypass catalog (`multi_select.rs:1142-1146`,
  `menu_bar.rs:1641-1657`).
- **Fix:** catalog all; enforce one-glyph-one-concept. `▌`=selected (survives unfocus), checked
  → `Glyph::CheckOn/Off` in a separate column, `▸/▾`=disclosure only, `◇`=now-edge/checkpoint
  (free from Info, reassign Info→`·`).

### F6. `ListRowRecipe` — trailing-meta slot + breathing rows (P0 — 14 widgets)
- `tokens.rs:810` `trailing:secondary` shares `TextMuted` with secondary text; `list.rs:1234`
  `badge.or(status).or(trailing)` makes them **mutually exclusive** (k9s metric + status glyph
  can't coexist).
- `list.rs:72` row height = 1 cell for every density unless Comfortable+secondary — **no
  intentional Canvas breathing bands**.
- **Fix:** add `Role::TextFaint` (P4 4th level, below Muted) for trailing meta as an
  **independent right-aligned column** (timestamps/sizes) left of the status/badge slot;
  `LIST_NARROW_DROP_ORDER` still governs survival. Insert **1 blank Canvas spacer row** between
  sections under Comfortable (governed by `Density→SpacingScale`). `NARROW` drops trailing first.

### F7. `BackdropWash` + one overlay family (P1)
- `style/mod.rs:79` `DIALOG_BACKDROP: Color = Color::Reset`; `dialog.rs:546-553` paints `░`
  stipple + `DIM` + Reset bg — a sparse dot pattern, **not** the `blend_toward(Canvas,0.6)`
  dim. Non-black-background terminals get **no dim at all**.
- QuickOpen paints on `Panel::Focused` not `SurfaceRecipe::Overlay` (`quick_open.rs:1382-1390`);
  Picker has **no wrapper at all** (`picker.rs:421-487`); Select trigger = fill, list = border
  (**two vocabularies in one widget**, `select.rs:898-966`).
- **Fix:** add `Role::BackdropWash = blend_toward(Canvas, 0.6)` as a bg-carrying role; all
  overlays (Dialog/ChoiceDialog/QuickOpen/Picker/Select popover/Combobox menu/MenuBar/Toast) →
  `Elevated` + `BackdropWash` dim. One overlay recipe (`SurfaceRecipe::Overlay`).

### F8. `DangerChrome` + primary-button discipline (P1)
- ChoiceDialog red-borders **whole chrome** (`dialog.rs:1223-1231`); persists through
  interaction (`1271-1282`). Research consensus (shadcn/Amp/Linear/Grok) = danger on **button
  only**. `title_for_paint` keeps `!` prefix (good).
- 3/4 composites have **no primary `Button`** — submit is chord-only (SetupWizard
  `form_wizard.rs:1145-1180`, SettingsScreen `:472-478`, AuthEntry `:534-536`); all hand-paint
  footers as `set_stringn` text. `ButtonVariant::{Primary,Quiet,Outline,Destructive}`
  (`primitives.rs:378`) exists unused.
- **Fix:** `DangerChrome::{Quiet default, Loud opt-in}` — Quiet = neutral border, `!` + word
  carry danger, red only on the solid confirm chip (`bg Danger / fg INK / BOLD`); Loud =
  red border for irreversible-of-irreversible. Every shippable action = **one `Button{Primary}`,
  Enter-triggered, omitted-not-greyed when disabled** (P6/P7). Footers compose
  `StatusBar`+`Badge`+`Kbd` (the unified recipe).

### F9. `LoadingMode` enum (P1)
- Loading split across `LoadingView` (cold spinner, `view_state.rs`), `LoadingOverlay`
  (Optimistic/Stale, `loading_overlay.rs:56-118`), `Skeleton` (inert, `skeleton.rs`). No shared
  vocabulary.
- **Fix:** one `LoadingMode{Spinner(verb), Skeleton(ShapeSpec), Optimistic}`. Spinner = Work +
  verb beside it; Skeleton = inert + `shimmer_cells` sweep (**never spins**); Optimistic = dim
  content + `↻ updating`. `LoadingOverlay` composes `LoadingView` as its body renderer.

### F10. Density defaults + `Stack` gap + Panel inset (P1)
- `StackSpec::default/vertical/horizontal` all hardcode `gap: 0` (`layout/stack.rs:236-285`);
  wrap `row_h: u16 = 1` (`:1032`); no Spacer primitive.
- `Panel` pad_x/pad_y collapse to 0 under narrow (`panel.rs:719-728`); text touches the border.
- **Fix:** Stack gap reads from `SpacingScale::from_density` (Comfortable>Compact); `.gap(0)`
  explicit opt-out. Add `Stack::spacer(n)` painting `Canvas` fill (breathing band). Enforce
  **min 1-cell Panel inset** at all widths (content width contracts, inset never zero).

---

## 2. Historical per-surface audit

Format: **Defect** (`file:line`) → **Redesign** → **P**riority. Dense by design.

### Cluster A — Inputs

**13. TextInput** (kernel, heals 6) — No border (`text_input.rs:994-1203`); `InputRecipe.border`
zero readers; label underline (`:1023-1025`); static `…` loading (`glyph.rs:471`); placeholder
reuses TextMuted. → Consume `InputRecipe.border`: single-line Square `Border`→`BorderFocused`,
Sunken well + optional lit-well wash on focus. Delete underline. Braille Work spinner ~80ms.
Add `Role::TextPlaceholder`. Prefix/suffix/clear inside the well. **P0**.
*Decision: focused-input 1-cell `▎` left rail (opt-in) or border-swap alone? Propose border-swap default.*

**14. TextArea** — Placeholder gated to single-empty-line (`text_area.rs:1623,1700`); no
active-line cue beyond 1-cell caret; no Ln/Col; no matching-bracket. → Placeholder on empty
active-line too. Active logical line on **Sunken wash** (NOT underline — user dislikes). Gutter
cell TextStrong+BOLD (Helix). `Ln/Col` FgFaint inlaid title. Taller-on-focus +1 row. **P1**.
*Decision: active-line wash default-on (Editor) or variant token (off in Review)?*

**28. SearchInput** — Bare int count, no `/total` (`search_input.rs:957`); static `…`
(`:950-955`); label underline (`:808-812`); static `⌕`; neon REVERSED chips (`:851-859`); no
separator, no `[esc]`. → `{count}/{total}` FgFaint; Work spinner + verb `searching`; `⌕`
TextMuted→BorderFocused; one chip recipe; `─` separator; `[esc]` keycap when query non-empty.
**P1**. *Decision: adding `total` to `SearchStatus::Results` is breaking.*

**34. PasswordInput** — Mask default `*` not `●` (`password_input.rs:713,760`); reveal glyphs
`o`/`*` (`:797`); strength = text label (`:811-830`); `RevealPolicy::Never` default (`:44`);
mismatch `is_mismatch()` never painted. → Unify mask `●`; eye pair `◑`/`◌`; **BLOCK_RAMP meter**
`▏▎▍▌▋▊▉` Danger→Warning→Info→Success; default `Hold` (press-to-peek); mismatch → border
`InputBorderInvalid`. **P0**. *Decision: catalog eye glyphs; is Hold safe if backends lack key
Release? (fall back to Explicit toggle).*

**36. NumberInput** — Label underline (`number_input.rs:1042-1044`); flat 2-color steppers
(`:1076-1094`); no hold-to-repeat (`:614-622`); clamp-on-blur inconsistent; asymmetric `−`/`+`
(`:1070,1088`). → Kernel border swap; stepper ladder TextDisabled→TextMuted→BorderFocused→
Accent+BOLD; **hold-to-repeat** `interval=max(1000/t²,25ms)`; universal clamp-on-focus-loss,
Esc=cancel-restore; symmetrize glyphs via catalog. **P1**.

**9. TokenField** — Label underline (`token_field.rs:1003-1005`); single-row only (`:1016`);
no `⌫` hint; no duplicate flash; chip paint delegated to broken Tag/Chip. → Kernel well; multi-row
wrap (`max_rows` opt-in); chips via unified `TokenRecipe`; `⌫` FgFaint on focused token;
duplicate → Warning border flash ≤150ms (interruptible). **P1**.

### Cluster B — Controls

**23. Slider** — Glyph-catalog bypass (`slider.rs:187-203`); label underline (`:736-738`); no
sub-cell thumb; no fisheye ladder; no value chip. → Catalog `Glyph::{SliderThumb/Fill/Rail/Tick}`;
thumb ladder `○`→`◎`(fisheye)→`●`; rail `Border`/fill `Accent`; **left-half-block ramp** at thumb;
value chip `[value]` Selection-fill only while focused/dragging; delete underline. **P0**.
*Decision: value chip = unified recipe or minimal token?*

**30. RangeSlider** — Two thumbs share one `●` (`slider.rs:1347-1349`); no collision glyph;
inherits bypass. → Start thumb `◇`, end `●` (distinct on overlap); collision `◆` Warning;
fill-between Accent (already correct); value chip `[lo–hi]`. **P1**. *Decision: distinct
per-thumb glyphs vs single + active ring.*

**31. RadioGroup** — `◎` preview pip **never painted** (`controls.rs:1037-1057`); legend/option/
hover underline (`:1140,1292,1304`); no two-bg-cue. → Full ladder `○`→`◎`(preview)→`●`→`◉`;
SelectionTint (selected) / HoverTint (focused) two-bg-cue; delete all underline. **P1**. *(The
`◎` preview pip is the headline pattern for every choice widget.)*

**18. Stepper** — Connector carries no progress (`stepper.rs:906-910`); bracketed text marks
(`:86-103`); current = REVERSED; error/roving underline (`:860,881`). → Connector ramp `━`
Accent up to current / `─` Border after; node ladder `○`→`◉`→`✓`→`✕` (current `◇` when running);
roving = HoverTint row; delete underline. **P1**. *Decision: connector fill = Accent (liveness)
or Success (done-ness)?*

**22. Spinner** — No Stream phase / no Done morph (`spinner.rs:50-61`); reduced-motion `●`
collides with done; no `Role::Spinner`. → Explicit phase→channel map; add `Streaming`→Stream
shimmer `∻≈∿〜`; Done→Static morph braille→`✓`; reduced-motion fallback `○` (not `●`); add
`Role::Spinner`. **P1**. *Decision: Stream on Spinner or a dedicated StreamingCaret widget?*

### Cluster C — Collections / data

**40. List** (recipe SoT, 14 widgets) — `show_focus_underline` root (`tokens.rs:816`);
trailing=secondary + mutually-exclusive slots (`tokens.rs:810`, `list.rs:1234`); no breathing
(`list.rs:72`). → (F2 + F6). **P0**. *Decision: breathing rows default-on Comfortable (migration
cost) or opt-in?*

**5. VirtualList** — Sticky rows bypass `resolve_list_row` (`virtual_list.rs:864-886`), bare
TextStrong, no gutter/tint. → Route sticky through the recipe; faint `─` rule under sticky-lead;
breathing row. **P1**. *Decision: sticky = navigable item or non-selectable header?*

**6. VirtualGrid** — **Zero** `resolve_list_row` calls; cursor + range both `Role::Accent`
(`virtual_grid.rs:1040,1042`) — indistinguishable + dies in monochrome; pending = static `…`.
→ Cell-native contract: cursor `SelectionTint`+TextStrong+BOLD+optional `BorderFocused` frame;
range `SelectionTint`+TextStrong; pending → skeleton shimmer. **P1**. *Decision: extend
ListRowRecipe or new `resolve_grid_cell`?*

**7. TreeTable** — `▌`=checked, `›`=selected+focused, **` `=selected+unfocused (blank!)**
(`tree_table.rs:1297-1303`); `focused: surface_focused && selected` (`:1252`). → `▌`=selected
**unconditionally** (survives unfocus) via catalog; checked→`CheckOn/Off` separate column;
decouple focused from selected. **P1**.

**42. KeyValueTable** — Gutter `›` not `▌` (`key_value_table.rs:1228`); hand-rolled 3-branch
selection (`:1243-1260`); separator `"  "` vs DetailTable's ` : ` (`:524`); full 8-wide type col
(`:44`). → `▌` gutter; `resolve_list_row`; **dot-leader** `········` Border between key/value
library-wide; width-gated 1-char type badge (`#%@·`); tabular numerics right-aligned. **P1**.
*Decision: type badge mode — compact-only, full-only, or width-gated both?*

**43. Histogram** — Integer-row bars quantize (`charts.rs:1519`); `BLOCK_RAMP` lacks left-half
ramp (`glyph.rs:17`); selection color-only → invisible in mono (`:1531`); inline `░▒` uncataloged
(`:1248,1758`); no baseline. → **1/8 sub-cell bar-tops** `▏▎▍▌▋▊▉`; catalog left-half ramp + shade
blocks; mono multi-series via **hatch** (selected `█`BOLD / unselected `░`); faint `─` baseline.
**P1**. *Decision: half-block orientation — left-half (bar tops/slider) vs bottom (density)?*

**44. DetailTable** — `SELECTED_MARKER="▸ "` disclosure collision (`detail_table.rs:17`);
separator `" : "` (`:19`); hand-rolled selection no recipe (`:586-613`). → `▌` gutter; dot-leader;
resolve_list_row; **frozen identity col** (Sunken) on horizontal scroll; tabular numerics; sticky
header. **P1**.

### Cluster D — Layout primitives

**17. Surface** — (F1). Raised/Overlay collapse, no Sunken, no backdrop dim. **P1** (foundation P0).
*Decision: Sunken fill-only or optional hairline border?*

**20. Stack/Inline** — `gap: 0` default (`layout/stack.rs:236-285`); wrap `row_h: 1` (`:1032`);
no Spacer. → (F10). **P1**. *Decision: Spacer paints Canvas fill or transparent?*

**21. SplitPane** — Glyph literals bypass catalog (`split_pane.rs:382-393`); no grip on focus;
no inlaid title; no equalize; no live `%`; collapse arrows burn Accent. → Catalog divider glyphs;
idle `Border` faint / focused `BorderFocused` + grip `⠇`/`⠺` + title `┤ Title ├`; live `%` while
dragging; `Equalized` outcome (dbl-click / Ctrl+Enter); arrows → BorderFocused. **P1**.
*Decision: grip = braille `⠇`/`⠺` (house style) vs half-block ramp?*

**29. ResizablePanelGroup** — Duplicate glyph literals (`:889-910`); no grip/title/`%`; no
equalize; sizes lost on panel-count change (`:402-430`); static min (`:991-1060`); 1-cell hit no
padding. → Reuse SplitPane's divider helper (lockstep); `Equalized` outcome; **`PanelSizeStore`
trait** for persistence; content-driven min; ±1-cell hit-expand on hover. **P1**.
*Decision: widget-owned `PanelSizeStore` trait vs host-only?*

**35. Panel** — `PanelRecipe.surface` zero readers (`tokens.rs:224`, `panel.rs:862-883`);
recipe computed twice; pad→0 narrow (`:719-728`); DividerOnly full `Border` weight (`:1118`);
bare-space title; Selected gutter Accent. → One recipe path; **min 1-cell inset**; DividerOnly
→`ChartGrid` faint; title bracket `[ Title ]` → `┌[ Title]─────┐` (brackets drop first <14);
wire or delete dead surface field; Selected gutter → TextMuted. **P1**. *Decision: bracket title
default-on (2-cell cost) or bare?*

**27. Section** — Status as trailing text (`section.rs:528-534`); divider full row below
(`:442-453`); no rhythm enum; unbounded depth (`:297-308`); divider `Border` weight. → Status as
**leading** `[ ✕ failed ]` glyph prefix (shape ladder); `RuleBeside` (title + faint `ChartGrid`
rule same row, **not under glyphs**); `SectionRhythm{Tight,Normal,Loose}`; **cap depth 2**;
divider → ChartGrid; depth role ladder. **P1**. *Decision: RuleBeside fill-to-end or
fill-to-actions?*

### Cluster E — Nav / chrome

**16. Tabs** (P0) — `━` underline row (`tabs.rs:1228-1242`); focused/hovered underline
(`:1176,1178`); `TabUnderlineFocused/Unfocused` roles (`style/mod.rs:180-182`); locked test
(`:1378`); `paint_select` BOLD|REVERSED; 4-role ladder. → Retire `TabUnderline*`; delete `━` row;
`TabsActiveCue::{AccentPill, Connected, Marker, Rule(default)}` — active = focus-aware rule+
TextStrong+BOLD; roving brightens leading edge BorderFocused. **P0**.
*Decision D2: Rule is the canonical default; alternate cues remain explicit.*

**48. Breadcrumbs** (P0) — Current crumb `UNDERLINED` (`breadcrumbs.rs:871-875`, comment
"underline current for no-color"); focus REVERSED+BOLD; clickable TextMuted (too dim); separator
TextMuted not Border; leaf truncatable; no `▸` prefix. → Delete underline; current TextStrong+BOLD;
separator `Border` faint; clickable Text + optional `▸` Accent prefix; focus HoverTint; never cut
leaf. **P0**.

**24. Sidebar** — Focus REVERSED (`sidebar.rs:1219-1229`); route `Role::Selection` neon
(`:1218`); literal `›/•` gutter (`:1238`); literal `─` (`:1197`); headers not uppercase; literal
chevrons (`:1255`); floats on Canvas no Raised (`:1334`); no auto-collapse <48 cols. → Default-on
Raised + single left `│`; route `SelectionTint`+`▌` gutter; focus HoverTint+TextStrong; uppercase
headers + faint rule; catalog chevrons; auto-collapse to icon rail <48 cols. **P1**.

**37. MenuBar** — Mnemonic `(F)ile` parens text-only (`menu_bar.rs:1753-1771`); open top =
Selection neon (`:1407`); narrow REVERSED (`:1359`); `✓`/`●` literals bypass catalog
(`:1641-1663`); submenu `›` (= active marker); no backdrop dim; disabled greyed. → Mnemonic
letter Accent+BOLD (color not underline; parens ASCII-only fallback); AccentPill open; catalog
check/radio + shape ladder; submenu `▸`; backdrop dim; omit disabled. **P1**.

**8. Toolbar** — Disabled greyed not omitted (`toolbar.rs:1080-1087`); roving = solid Accent fill
(breaks one-primary) (`:1082-1087`); ad-hoc `[..]` cursor + `(C-s)` parens; no contextual/mode
slots; no primary flag. → Ghost default; roving HoverTint+Text; `primary` flag → one solid Accent
chip; omit disabled; unified keycap; `ToolbarSlot::{Contextual, Mode}`. **P1**.

**45. Collapsible** — No HoverTint (`collapsible.rs:514-523`, no `hovered` field); Section rule
**inverted** (rule on open not collapsed) (`:535-549`); no border-b after collapsed siblings;
binary height swap no settle. → Keep `▸▾` (already correct) + flush body (already correct); add
`hovered`+HoverTint; `RuleAfter::Collapsed`; optional stepwise settle (defer to motion primitive).
**P2**. *(Mostly already right.)*

### Cluster F — Overlays / picker

**26. Select** — Two vocabularies: trigger fill-ladder no border (`select.rs:898-912`) vs list
Panel border (`:958-966`); separator ignores ascii (`:1024-1035`); check = left-inline prefix
(`:1087-1096`); selected vanishes when highlight moves. → Trigger = InputRecipe well (one vocab);
list = Elevated + BackdropWash; one separator helper; **right-aligned `✓` column**; one highlight
+ check-glyph-only selected. **P1**.

**Combobox** (Select sibling) — Label underline (`combobox.rs:944-952`); **dead chevron** `_chev`
(`:1018-1028` — never painted, so indistinguishable from TextInput); static `…`; no committed
`✓`; no shared fuzzy painter. → Kernel border; **paint chevron** `▾`/`▴`; Work spinner;
committed `✓` FgMuted; route menu through shared `HighlightedText`. **P1**.

**32. QuickOpen** — Panel not Overlay (`quick_open.rs:1382`); single-line rows (`:1632-1746`);
plain-text footer (`:1499`); neon Selection active tab (`:1558`); static `[...] searching`
(`:1586`); recency sort-only. → `SurfaceRecipe::Overlay`; **two-line rows** (label + path FgFaint)
+ `[⏎]` active-row hint; grouped headers (Recent/Files/Symbols) uppercase + faint rule; AccentPill
tab; Work spinner + verb; footer keycap chips; `{shown}/{total}` count. **P1**.
*Decision: two-line Comfortable / single-line Compact, or always two-line?*

**33. Picker** — **No wrapper at all** (`picker.rs:421-487`); discards focused/ascii/colorless
(`:484`); third empty-cue variant (`:455-473`); dead TextMuted-both-arms branch (`:462`); no
preview/footer. → Wrap in Overlay + Focused chrome; route rows through `HighlightedText`; adopt
QuickOpen empty cue; optional footer kbd chips. **P1**. *Decision: Picker=light (no preview) /
QuickOpen=heavy, or converge? Recommend split, share primitives only.*

**46. ChoiceDialog** — (F8). Red-border-on-chrome; backdrop = Reset+stipple not dim; no
`DangerChrome`; phrase-gate/countdown absent despite "best-in-class" claim. → Quiet default
(neutral border, red confirm only); BackdropWash dim; add phrase-gate + countdown on Loud+irreversible;
default cursor to cancel. **P1**. *Decision D1: confirm Quiet default.*

### Cluster G — Feedback / status

**10. Toast** — `TOAST_STACK_GAP=0` (`toast.rs:48`); uniform stack no pebble tier (`:1371-1413`);
full rail all entries (`:1047`); static no entrance (`:1023`); `MAX_VISIBLE=5` (violates 3-layer);
push-only API no `toast()` (`:748`); no focus-loss pause; no error-gravity / success-pop; fixed
offset no retarget (`:1370`). → (law §7): front full Elevated + rail + icon+body; rear = 1-row
lower + dimmer fill + **no body, icon-only pebble**; cap 3; FrameTick row-offset tween (retarget);
≤120ms entrance; errors skip tween; success one pulse then static; TTL 4s + focus-loss pause;
observer `toast()`. **P1**. *Decision: `toast()` = process-global singleton vs host-injected delegate.*

**11. Timeline** — No vertical rail (`timeline.rs:954-1036`); `◇`=Info collides with now-edge
(`:113-134`); duration inlined not right-aligned (`:1079`); no FrameTick/wave (`:413-439`); footer
`↓ live` chip not `◇` now-marker (`:1131`); no age-fade. → 1-cell rail: completed `┃` Border /
running `wave_brightness` toward active / failed `┊`+`✕` / now-marker `◇` Accent at live edge;
reassign Info→`·`; duration FgFaint trailing slot; recency age-fade. **P1**. *Decision: Info
marker `·` or `○`?*

**19. StatusIndicator** — Static glyph only (`status_indicator.rs:341-367`); "aliveness" = BOLD
only; no FrameTick/MotionChannel; elapsed inlined; `busy` claim not painted. → Motion-as-status:
Running `◉` Live breathe ~2s; Waiting `◐` Wait dot-pulse; Online `●` heartbeat ~5s; gravity
static; reduced-motion zero-loss; elapsed FgFaint slot. **P1**. *Decision: Waiting = glyph swap
◐↔◑ or brightness pulse?*

**38. LogPane/LogStream** — Follow chip `⇣ following` horizontal not vertical `┃` rail
(`log_pane.rs:319`); no fixed columns (`log_stream.rs:1176`); follow `↓ follow` bottom text
(`:1250`); hard-reject filter hides below-floor (`:905`); inline severity glyph no `│` rail
(`:1146`); no live-arrival pulse. → Fixed columns (ts/src/severity-rail/body); vertical `┃` Accent
now-rail (= Timeline, one `NowRail` recipe); **soft filter dim-not-hide** (+ collapse consecutive
identical below-floor `· ×N`); severity rail `│` by level; 1-tick arrival pulse on append. **P1**.
*Decision: soft-filter pure-dim vs dim+collapse-identical under burst?*

**39. LoadingView** — (F9). Single cold-spinner mode; spinner+label concatenated; static frame.
→ `LoadingMode{Spinner,Skeleton,Optimistic}`; Work+verb; skeleton inert sweep; optimistic dim+
`↻ updating`. **P1**. *Decision: `LoadingMode` on LoadingView (LoadingOverlay composes it) vs
shared enum each widget reads?*

### Cluster H — Meta / chips

**15. Tag** (P0) — Square `[ body ]` not angle `⟨⟩` (`tag_chip.rs:306`); `BOLD|UNDERLINED` remove
(`:364`); Selection neon (`:532`). → (F4). **P0**.

**47. Chip** (P0) — Duplicate paint vs Tag (`tag_chip.rs:768` vs `293`); `BOLD|UNDERLINED`
(`:849`); inverted selection ramp + neon (`:532`); mark doesn't escalate on ramp. → (F4): collapse
to one `TokenRecipe`; shape-ladder mark `○`→`●`→`◉`; remove-region invert. **P0**.
*Decision: focused+selected mark `◉` or focus on bracket only (one-bright-border)?*

**25. ShortcutHint/Kbd** — Brackets painted key-style not faint (`kbd.rs:562-564`); two keycap
renderings (`535/557` vs `779`); `+` separator Spelled mode (`:251-268`). → (F4): one keycap recipe
`[ C-s ]` space-padded, brackets Border faint, label HintKey+BOLD; space separator inside chip
(keep `+` in prose); focused = REVERSED. **P1**. *Decision: Spelled separator space globally or
chip-only?*

**41. KeyboardHelp** (P0) — `UNDERLINED` colorless conflict cue (`keyboard_help.rs:1203`); neon
active fill (`:1208`); split conflict glyph `!` vs `⚠` (`:1178` vs `1032`); left-aligned chord
(`:1188`); no header rule; no `[12]` count (`:594,1168`). → Drop underline → `⚠`/`!` Danger+BOLD;
SelectionTint+`▌`; unify conflict glyph; **right-aligned** chord column `{:>10}`; faint header
rule; `[12]` live count chip. **P0**.

**12. ThemePicker** — Literal `›` marker bypasses catalog (`theme_picker.rs:267`); Selection neon
(`:274`); flat `· truecolor` (`:268`); ungrouped, no swatch, no footer. → `▌` gutter; SelectionTint;
**`[▆▆▆▆]` swatch band** through active terminal color resolution (`~` hatch if truecolor-only on
ANSI-16) + `[~ tc]` chip; Recommended/Variants groups + faint rule; kbd footer. **P1**.
*Decision: ThemePicker = generic PresetPicker (widgets) or composite (patterns)?*

### Cluster I — Composites (patterns/)

**1. Setup wizard** — All nav delegated to FormWizard which hand-paints `< Back`/`Next` text
(`form_wizard.rs:1145-1180`); bespoke capability/summary painters; cancel = REVERSED strip not
Dialog; gate failure inline Danger text. → **Compose, don't paint**: one `Button{Primary}`
Continue→Finish (morphs by phase) bottom-right + Back/Esc ghost (omitted at step 0); capability
rows via `FieldRow` (marker `!`); alternate Surface/Raised bands; cancel = ChoiceDialog; push
statuses through Stepper (connector ramp). **P1**. *Decision: ramp in Stepper building block (rec)
or wizard-local band?*

**2. Settings screen** — Only `Form` mode wrapped in Panel (`settings_screen.rs:748`); Theme/
Keybinding/NoResults raw (break one-bright-border); plain-text footer (`:798`); StatusBar with
empty slots (`:811`); bespoke banner; Form/Fieldset not FieldRow; save chord-only. → Wrap ALL
body modes in one Panel; FieldRow anatomy; `StatusBar`+`Badge{● modified}`+kbd chips + one
`Button{Primary} Apply` (omitted when clean); banner = Badge widgets. **P1**. *Decision: block
auto-synthesizes footer when host passes empty slots, or requires host slots?*

**3. Metrics dashboard** — Tile = host-preformatted strings (`metrics_dashboard.rs:264-289`);
value+unit one Text string (`:1404`); delta color-only no glyph (`:1421`); top-edge-only focus
(`:1370`); sparkline saturated by health (`:1471`); static `loading…` (`:1456`); plain toolbar/
footer. → Split value(TextStrong+BOLD)/unit(TextMuted)/title(Text); **`▲`/`▼` before color**
(ASCII `^`/`v`); full Panel BorderFocused perimeter on focused tile; sparkline Border baseline +
faint threshold; Work spinner + verb; StatusBar+kbd. **P1**. *Decision: delta glyph widget-owned
(typed field, rec) or host-side string?*

**4. Auth Entry** — Bespoke field layout not FieldRow (`auth_entry.rs:796-833`); validation as
Danger header (`:757-769`); submit chord-only (`:534`); footer plain text (`:858`); secondaries
plain-text chords; `•` vs `●` mask. → FieldRow (`FieldRowValue::Masked` exists for passwords) with
per-field annotation; one `Button{Primary} Sign in` bottom-right (Enter triggers, Spinner+verb
`verifying` while pending); secondaries as ghost chips below; kbd footer; keep the already-compliant
Panel border; unify mask `●`. **P1**.

---

## 3. Open decisions (consolidated — need a human call)

These recur across clusters. Each changes defaults or public API.

| ID | Decision | Research recommendation | Blocks |
|----|----------|------------------------|-------|
| **D1** | Danger chrome: red **border** on dialog, or red **button only**? | **Button only** (Quiet default) — shadcn/Amp/Linear/Grok consensus; north star "not a terminal" overrides the older red-border spec. | ChoiceDialog, AlertDialog, destructive Button |
| **D2** | Active-tab cue: AccentPill / Connected / Marker / Rule? | **AccentPill default** (Linear lineage, most legible without any line); Connected for app-shells. Retire `TabUnderline*` regardless. | Tabs, Sidebar, MenuBar, QuickOpen tabs |
| **D3** | Panel border shape default: Square or Rounded? | **Square** (loved phosphor identity, CLAUDE.md binding). Rounded = easy preset. Research's "flip to Rounded" is overruled by explicit project constraint. | Panel, Surface |
| **D4** | `Role::Selection` neon removal | Execute — tint/gutter fully proven. No decision, just sequencing. | List, Sidebar, MenuBar, KeyboardHelp, Tag, Chip, ThemePicker |
| **D5** | `toast()` API: process-global singleton or host delegate? | Flag — affects threading/test isolation. | Toast |
| **D6** | Spinner Stream phase on Spinner or dedicated StreamingCaret? | Law §5 separates `▎` caret from spinner. Flag. | Spinner, streaming caret |
| **D7** | `LoadingMode` location: on LoadingView (overlay composes) vs shared enum? | On LoadingView, overlay composes it (fewer types). | LoadingView/Overlay/Skeleton |
| **D8** | List breathing rows: default-on Comfortable (migration cost) or opt-in? | Default-on is premium-correct but real migration cost. Sequencing call. | List, 14 widgets |
| **D9** | Histogram/Slider half-block orientation convention? | Need both; pick which "fractional" means in shared APIs. | Histogram, Slider, RangeSlider |
| **D10** | VirtualGrid recipe: extend ListRowRecipe or new `resolve_grid_cell`? | New `resolve_grid_cell` (keep row recipe focused). | VirtualGrid |
| **D11** | RangeSlider thumbs: distinct `◇`/`●` or single + active ring? | Distinct (legible on overlap). | RangeSlider |
| **D12** | Stepper connector fill: Accent (liveness) or Success (done-ness)? | Flag — shadcn uses neutral-completed + accent-current. | Stepper |
| **D13** | Log soft-filter: pure dim, or dim + collapse-identical under burst? | Collapse-identical (stern-style) for readability. | LogStream |
| **D14** | Info status glyph reassignment (`◇`→now-edge): Info becomes `·` or `○`? | `·` (LogStream family). | Timeline, StatusIndicator |
| **D15** | SplitPane/Resizable grip: braille `⠇`/`⠺` or half-block ramp? | Braille (house style; reserve ramp for slider). | SplitPane, ResizablePanelGroup |
| **D16** | `PanelSizeStore` persistence: widget-owned trait or host-only? | Widget-owned trait (product-neutral). | ResizablePanelGroup |
| **D17** | ThemePicker: generic PresetPicker (widgets) or composite (patterns)? | Swatch+row primitives in widgets, branded chooser in patterns. | ThemePicker |
| **D18** | Tag/Chip focused+selected mark `◉` or focus on bracket only? | Bracket carries focus (one-bright-border); mark stays `●`. | Tag, Chip |
| **D19** | Panel title bracket `[ Title ]` default-on (2-cell) or bare? | Default-on (premium "labelled container" tell), drops first <14. | Panel |
| **D20** | Section `RuleBeside`: fill-to-end or fill-to-actions? | Fill-to-actions (Linear/Notion calm). | Section |
| **D21** | Password `RevealPolicy::Hold` default safe if backends lack key Release? | Fall back to Explicit toggle if crossterm Release unreliable. | PasswordInput |
| **D22** | QuickOpen rows: two-line Comfortable / single-line Compact, or always two-line? | Density-gated (two Comfortable, single Compact). | QuickOpen, Picker |
| **D23** | Composites: block auto-synthesizes default footer, or host must provide slots? | Block-synthesized default + host-override (strong-defaults north star). | SettingsScreen, others |

---

## 4. Execution order (feeds `plans/`)

Ordered by unblock value. Each is a plan file. Docs-only constraint ends here — these are code
plans the operator's `plans/` system will execute.

**Wave 0 — foundations (do first, in order):**
1. **F1 Surface ladder** — Sunken recipe, split Raised/Elevated, `BackdropWash` role + dim,
   `ladder_is_monotonic` test. (Surface.rs)
2. **F5 Glyph catalog gaps** — left-half blocks, shade blocks, mask `●`, slider/divider/checkbox
   glyphs, `▌`/`▸` collision resolution, `◇` reassignment. (glyph.rs)
3. **F2 `FocusEmphasis` + TextInput kernel** — delete `show_focus_underline`, consume
   `InputRecipe.border`, add enum. Heals input cluster + unblocks the underline sweep.
4. **F3 `MotionChannel` + `shimmer_cells`** — channel vocab + period table + sweep primitive.
   Unblocks feedback cluster.
5. **F6 ListRowRecipe** — `TextFaint` trailing slot + breathing rows. Heals 14 widgets.
6. **F4 `TokenRecipe`** — collapse Tag/Chip/kbd. Heals chip cluster.
7. **F7 overlay family** — all overlays Elevated + BackdropWash dim + one recipe.
8. **F10 Stack gap + Panel inset** — Density-driven gap, Spacer, min 1-cell inset.

**Wave 1 — per-cluster sweeps (after foundations, parallelizable):**
9. Underline sweep (Tabs P0, Breadcrumbs P0, all remaining `UNDERLINED` sites) — rides F2.
10. `Role::Selection` neon sweep (D4) — rides F1/F6.
11. Collections selection contract (TreeTable/DataTable/DetailTable/List/VirtualList/VirtualGrid) —
    rides F5/F6.
12. Controls glyph ladders (Slider/RadioGroup/Stepper/RangeSlider) — rides F5.
13. Feedback motion (Toast/Timeline/StatusIndicator/LogPane/LoadingView) — rides F3.
14. Overlay convergence (Select/Combobox/QuickOpen/Picker/ChoiceDialog) — rides F7 + F8.
15. Composites → FieldRow + primary Button + StatusBar (SetupWizard/Settings/Metrics/AuthEntry) —
    rides F8.

**Wave 2 — decisions (D1–D23) + polish:** execute after the relevant decisions land.

---

## 5. Cross-surface consistency map (what MUST land together)

Per CLAUDE.md cross-surface-consistency law — these recipes change once and cascade:

- **One chip recipe** (F4): Tag, Chip, TokenField token, kbd, suggestion, attachment chips,
  ThemePicker truecolor chip.
- **One overlay recipe** (F7): Dialog, ChoiceDialog, QuickOpen, Picker, Select popover, Combobox
  menu, MenuBar menu, Toast.
- **One row recipe** (F6): List, Select/MultiSelect/Combobox list, TreeTable body, VirtualList,
  QuickOpen results, Picker, KeyboardHelp, KeyValueTable, DetailTable.
- **One focus vocabulary** (F2): every focusable widget.
- **One motion vocabulary** (F3): every animated widget.
- **One `NowRail`/`StatusRail` recipe**: LogPane follow, LogStream follow, Timeline live-edge,
  EventStream.
- **One `paint_bracketed` helper**: Tag/Chip brackets, kbd brackets, KeyValueTable/DetailTable
  separators.
- **One danger vocabulary** (F8): ChoiceDialog, AlertDialog, destructive Button, PermissionPrompt.

Inconsistency is a defect. A local win that invents a second way is incomplete work.

---

*All `file:line` references verified against HEAD by the 9 audit agents. Design-language analysis
only — no proprietary source reuse. Component redesigns compose existing public building blocks
(`FieldRow`, `Button`, `Badge`, `StatusBar`, `Panel`, `Stepper`) wherever possible; no new widgets
where composition suffices.*
