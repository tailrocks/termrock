# TUI design research — implementation annex (per-component, source-grounded)

| Field | Value |
|-------|-------|
| **Status** | Research annex (evidence + implementation-grade findings). Not a law doc. |
| **Date** | 2026-08-14 |
| **Method** | 7 parallel research agents, one per component family, each grounded in (a) actual TermRock source (`file:line` verified) and (b) best-in-class TUI/CLI + frontend libraries with cited patterns. |
| **Companion to** | [`tui-design-research-2026-08.md`](./tui-design-research-2026-08.md) — design-law v2 plus an archived 48-surface sample and token deltas. This file is historical evidence, not inventory. The Rust inventory and generated docs catalog own current membership. |
| **Feeds** | `plans/001` (underline SoT), `plans/002` (role palette), and the future code plans (TextInput kernel, glyph catalog, FocusEmphasis). |
| **Constraint honored** | User dislikes underline for focus/active; accent-restraint aesthetic (Grok Build / Amp / Jackin / shadcn / Linear). |

> Read [`tui-design-research-2026-08.md`](./tui-design-research-2026-08.md) §5 (the binding law) first.
> This annex adds only what that doc lacks: **current-code defects with line numbers**,
> **proven-in-repo replacement mechanisms**, **named external-library patterns at the
> detail that makes each component premium**, **glyph/token catalog gaps**, and a few
> **divergences** worth a human decision.

---

## 1. The single highest-leverage defect (unblocks the most widgets)

**`TextInput` paints no border at all.** Focus is communicated only by
`Role::Focus + Modifier::UNDERLINED` on the optional label row
(`widgets/text_input.rs:1016-1025`). **Every input widget delegates to
`TextInput::paint()`** — `NumberInput`, `PasswordInput`, `PathInput`, `SearchInput`,
`Combobox`, `TokenField`, `InputGroup`. So one kernel defect propagates the
underline-as-focus look across the entire input family.

The fix the design law already specifies — sunken well + `Border`→`BorderFocused`
border swap — is **already an unused contract**: `InputRecipe.border`
(`style/tokens.rs:730-759`) exists and has zero readers. Wire `TextInput` to consume
its own recipe's border and delete the label underline, and the input cluster heals
in one change. This is the P0 beneath P0.

> This sharpens `tui-design-research-2026-08.md` §7 item 2: the underline retirement
> is not 30 independent widget edits — it is *fix the kernel + kill the shared flag*.

---

## 2. Converged structural findings (apply across many widgets)

### 2.1 Underline root cause is structural, not per-widget

The underline problem has **three load-bearing sources**, not 30:

1. `ListRowRecipe.show_focus_underline` (`style/tokens.rs:816,856`) — bakes underline
   in as *the* row focus primitive; 14 collection widgets inherit it.
2. The `TextInput` kernel (§1) — propagates to 7 input widgets.
3. **No central underline-free focus-emphasis vocabulary** — so ~10 remaining widgets
   invent their own ad-hoc `Modifier::UNDERLINED` (tabs, toggle, stepper, segmented,
   badge, tag/chip remove, keyboard_help, citation, breadcrumbs, link).

**Proposed structural fix:** add a `FocusEmphasis` enum on `DesignSystem`
(`{BrightBorder, SelectionFill, FocusTint, Reversed, BoldKey, PillGlyph}`) and route
all focus emphasis through it. Widgets stop reaching for `UNDERLINED` because the
vocabulary no longer offers it. This removes the bug *class*, not the symptoms — and
it makes `plans/001`'s doc reconciliation enforceable in code.

Underline survives only where `plans/001` already allows it: ANSI SGR-4 / markdown /
diff (content), OSC-8 links, and the cursor fallback.

### 2.2 The three underline-free mechanisms — all already proven in-repo

Every external reference (shadcn, Radix, Mantine, Linear, Textual, huh, fzf) uses
**border swap or ring, never underline**, for focus. TermRock already has the tokens
to do the same — they are just under-consumed:

| Mechanism | Tokens (proven) | Replaces underline in |
|-----------|-----------------|-----------------------|
| **Glyph ladder** `○`→`◎`→`●` (idle→focused/preview→committed) | `RadioOff`, `StatusDotRing` `◎`, `RadioOn` `●`, `Busy` `◉` — all cataloged in `glyph.rs` | RadioGroup, Slider thumb, Stepper node, Checkbox, Toggle, Segmented |
| **Border color shift** `Border`(#505050)→`BorderFocused`(#00FF41) | `Role::Border`/`Role::BorderFocused` — the Panel focus law | containers, inputs, chips, segmented |
| **Background wash** `HoverTint`(#1A221C) / `SelectionTint`(#14331A) | both bg-carrying roles | rows, hover, selected chips, focused segments |

The **glyph ladder is strictly more informative than underline**: the `◎` preview pip
shows *where the cursor is before you commit*. That is the headline replacement
pattern for every control/choice widget.

### 2.3 Motion as status — a 5-channel vocabulary (extends law §5.7)

The feedback/status cluster proposed one `MotionChannel` vocabulary so every widget
maps its "why animate?" to one channel instead of inventing its own pulse:

| Channel | Signal | Glyph/efx |
|---------|--------|-----------|
| **Work** | compute happening | braille `⠋⠙⠹⠸⠼⠴⠦⠧⠇⠏` ~80ms |
| **Wait** | breathing/thinking | dot-pulse `⋅:⸬⁙` ~240ms |
| **Stream** | data arriving (not compute) | shimmer/water `∻≈∿〜` or traveling band |
| **Live** | alive presence | `pulse_brightness` ~2s breathe |
| **Static** | gravity states (done/failed/offline) | never move |

Plus: **`shimmer_cells` should be a first-class primitive in `style/motion.rs`**
(today only `wave_brightness`/`pulse_brightness` exist). One shimmer primitive powers
skeleton sweep, streaming-log arrival, progress-toast bar, live timeline rail —
stopping each widget rebuilding it ad hoc (the cross-surface-consistency defect the
contributor rules warn against).

**Skeleton ≠ spinner rule** (load-bearing, currently undocumented): skeleton =
"space reserved" and **must not** spin/pulse (inert); spinner = "work happening".
A skeleton that spins reads as broken. Promote into the anatomy spec.

### 2.4 Shape-before-color ladder (accessibility + premium)

Encode liveness/weight **before** hue, enforced cluster-wide:

- **Filled** `●` = terminal/solid (success, failed, online)
- **Ring** `◎`/`◉` = in-flight (running, waiting, focused-preview)
- **Hollow** `○` = idle (idle, offline, unknown)
- **Diamond** `◇` = checkpoint/marker/now-edge

Codify as a test mirroring the existing `color_alone_insufficient`: assert every
animated status has a distinct **static-glyph fallback** so reduced-motion still reads.

### 2.5 Catalog & token gaps to file (one PR each, enables everything)

These are missing or bypassed today — each blocks a cluster:

1. **Left-half block ramp** `▏▎▍▌▋▊▉` (U+258F–2589) — absent. Powers Slider/
   RangeSlider sub-cell motion + Histogram 1/8 bar-tops (Rich precision). ASCII `|`.
2. **Shade blocks** `░▒▓` (U+2591–2593) — absent. Soft fills, Switch trailing-edge,
   backdrop stipple. ASCII `:./#`.
3. **Slider bypasses the glyph catalog** — `widgets/slider.rs:193-203` hardcodes
   `●`/`━`/`─`/`┊` locally, ignoring `GlyphSet` Unicode/ASCII/Enhanced encodings. Only
   control widget that bypasses it. Promote to named `Glyph::{SliderThumb, SliderFill,
   SliderRail, SliderTick}`.
4. **Mask glyph is split three ways** — `*` (TextInput/PasswordInput default),
   `●` (PasswordInput `.mask()`), `•` (InputOtp). Unify to `●` (U+25CF) library-wide,
   ASCII `*`. One constant.
5. **`▌` selection-gutter glyph collision** — `tree_table.rs:1297-1303` and
   `data_table.rs:1477` steal `▌` to mean *checked* (else `›` only when focused, else
   blank). Same glyph = two meanings; and a selected row in an unfocused panel renders
   as **blank** (selection vanishes). Fix: `▌` = selected unconditionally (survives
   unfocus; focus is the border's job); move checked to a catalog checkbox glyph
   (`✓`/`☐`).
6. **`multi_select.rs:1142-1146`** hand-rolls checkbox literals `"[✓]"`/`"[ ]"`/`"[x]"`
   — bypasses catalog. Route through `Glyph::CheckOn/Off`.
7. **Missing roles** (proposed): `Role::TextPlaceholder` (placeholder ≠ secondary
   text), `Role::InputBorderInvalid` (invalid must recolor the **border**, not the fill
   — shadcn/Linear restraint vs Mantine aggression), `Role::Spinner`, optional
   `Role::InputFocused` (the "lit well" wash).

### 2.6 One-rule vocabulary fixes (consistency sweep)

Each is a single-rule change that makes the cluster read as one system:

| Fix | Today | Should be |
|-----|-------|-----------|
| Separator row | Select hardcodes `─`, MultiSelect hardcodes `-`, both ignore ascii toggle | `─` everywhere, `-` in ascii (respect the toggle) |
| Affordance-glyph color | `▾▴✓×⌕◀▶⌫` painted foreground or inconsistent | `TextMuted` rest → `BorderFocused`/`Accent` active → `TextDisabled` disabled (shadcn opacity-50 equivalent) |
| Ellipsis width | ascii `...` = 3 cells, unicode `…` = 1 | 1-cell ascii stand-in, or reserve 3 in both |
| Spinner wiring | `…`/`...` static everywhere a spinner belongs (TextInput loading, Password pending, Search searching, Select/Combobox async) | wire the existing braille frames, FrameTick-driven ~80ms |
| Combobox chevron | computed but never painted (dead `_chev`) | paint `▾`/`▴` like Select |

---

## 3. Open decisions (research diverges from `tui-design-research-2026-08.md`)

These are real disagreements surfaced by the deep research. Flagging, not resolving —
each changes defaults and needs a human call.

### D1 — Danger chrome: red border, or red button only?

- **Operator doc** (`§6` ChoiceDialog, `§5`): danger variant = `danger border + ! + scope body`.
  TermRock today red-borders destructive dialogs.
- **Research** (shadcn AlertDialog, Amp, Linear, Grok Build consensus): **danger lives
  on the ACTION button only; chrome stays neutral.** Target echoed in muted body;
  cancel-left / destructive-right; destructive is *not* default.

**Proposed:** `DangerChrome::{Quiet (default), Loud (opt-in)}`. Quiet = neutral border
(`BorderFocused`, modal owns interaction), `!` glyph + "Delete" word carry danger
(glyph+word first, color last — on-brand), panel fill stays `Surface`; red appears
**only** on the solid confirm chip (`bg Danger / fg Black / BOLD`). Loud = current
`Danger` border for the irreversible-of-irreversible. This aligns TermRock with the
premium north star while preserving glyph-first danger. The scoping body + Safe-Enter
(default=cancel, phrase gate, countdown, Esc-trap) is already best-in-class — keep all.

### D2 — Active-tab cue: connected-fill, accent pill, or marker?

Three variants in play across the docs:

| Source | Active-tab cue |
|--------|----------------|
| `tui-design-research-2026-08.md` §5.3 | **Connected-tab**: active on `surface` fill + open bottom edge into content, `fg-strong` bold |
| `termrock-design-language.md` §5.2 | bold Accent + `▸` marker; bottom rule opt-in/off |
| Nav-cluster research (Linear/shadcn) | bold + **faint accent-fill pill** + accent fg; marker reserved for data state |

**Proposed:** make it a `TabsActiveCue::{Connected, AccentPill, Marker, Rule}` token;
default to **AccentPill** (most legible without any line, Linear lineage), `Connected`
as the app-shell option, `Rule` opt-in. Retire `TabUnderline*` roles regardless (all
three agree: never SGR underline on the label).

### D3 — Border-shape default: Square (phosphor identity) or Rounded (modernity)?

- **Data-cluster research:** flip Panel default Square→Rounded — "single biggest
  perceived-modernity lever" (Rich, btop round everything).
- **`phosphor-obsidian-visual-direction.md` + project CLAUDE.md:** Square caps are the
  loved phosphor/Jackin brand identity; Rounded is a theme token for Grok-class
  consumers.

**Proposed:** keep **Square as the phosphor default** (brand law is binding per
CLAUDE.md — "current phosphor design concept is loved and stays the default"), but (a)
ensure `BorderShape::Rounded` is one trivial switch, (b) ship a documented "modern"
preset that selects Rounded for consumers who want the Grok look. Do not flip the
default — that would break the loved identity. (This is the one place the research
recommendation is overruled by an explicit project constraint.)

### D4 — `Role::Selection` neon removal timeline

Both docs agree: kill the neon fill default. Operator §7 item 1 proposes removing
`SelectionChrome::Fill` after one migration window. Research confirms the tint/gutter
path is fully wired (`resolve_list_row`, 14 consumers). **Confirm and execute** — no
decision needed, just sequencing.

---

## 4. Per-component findings (deltas beyond the operator matrix)

Only what `tui-design-research-2026-08.md` §6 does not already say. Format:
**Defect → Move (source lib: detail).**

### 4.1 Inputs

- **TextInput** — Defect: no border, label underline (`text_input.rs:1016-1025`),
  static `…` loading, no prefix/suffix slots. Move: consume `InputRecipe.border`
  (border swap, not underline); optional "lit well" wash on focus (Textual 5%-fg lift);
  ghost-history prefill when focused+empty (Helix `ui.text.inactive`); wire braille
  spinner for loading; `TextPlaceholder` dedicated role.
- **TextArea** — Defect: placeholder only for single-empty-line. Move: placeholder for
  any empty doc; **active-line = `Sunken`/`raised` bg wash, NOT underline**
  (ratatui-textarea uses underline for active *line* — the one legit case, but user
  dislikes, so wash instead); active gutter cell emphasized (Helix `line-number.current`);
  optional matching-bracket bg tint (Textual); status `Ln/Col` faint in title slot.
- **NumberInput** — Defect: no hold-to-repeat. Move: keep typographically-correct `−`
  (U+2212, better than Mantine chevrons); **hold-to-repeat with acceleration**
  `interval=max(1000/t²,25)` (Mantine — no terminal lib has this); clamp-on-blur
  (Mantine); steppers `TextMuted`→`BorderFocused`→`TextDisabled`.
- **PasswordInput** — Defect: mask split (`*`/`●`); reveal glyphs `"o"`/`"*"` (vocab
  break); strength = text label; default `RevealPolicy::Never`. Move: unify mask `●`;
  reveal eye pair `◑`/`◌` (or `👁`/`⊝`); **block-ramp strength meter**
  `▏▎▍▌▋▊▉` Danger→Warning→Info→Success (not a text label); default
  `RevealPolicy::Hold` (press-to-peek, modern secure default); mismatch → border danger
  (not fill).
- **SearchInput** — Defect: bare integer count, no `/total`; static `…`; width
  inconsistency (`…` 1 cell vs `...` 3). Move: **`{count}/{total}` count** right-aligned
  (fzf `12/340`); streaming braille spinner; `⌕` muted→`BorderFocused`; `─` separator
  between query and results (fzf); `[esc]` clear kbd-pill when query non-empty.
- **Select / Combobox** — Defect: closed trigger uses fill-ladder, open list uses
  `Panel` border — **two focus vocabularies in one widget**; separator ignores ascii;
  Combobox chevron dead code. Move: trigger = bordered well (same recipe as TextInput),
  list = `Panel` BorderFocused — now one widget; **right-aligned `✓` check column**
  (shadcn/Radix — aligns all selected regardless of label length); chevron `▾`/`▴` at
  `TextMuted` (opacity-50 move); one highlight (accent fill) + check-glyph-only for
  selected; paint Combobox chevron.
- **TokenField** — Defect: delegates to TextInput underline; chip shape ad hoc. Move:
  bordered well (multi-row if wrap); chips `⟨ label × ⟩` (angle brackets, design-law
  spec) ASCII `< label × >`; `×` `TextMuted`→`Danger` on chip focus; `⌫` hint when token
  is active zone; duplicate → flash existing chip `Warning` (no banner).

### 4.2 Controls

- **Slider / RangeSlider** — Defect: hardcoded glyphs bypass catalog; no sub-cell. Move:
  catalog the glyphs; `─` rail `Border` / `━` fill `Accent` (weight contrast); thumb
  ladder `○`→`◉`(fisheye focus)→`◉`+`Focus` drag; **left-half-block sub-cell ramp** at
  thumb; value chip `Selection` style only while focused/dragging; RangeSlider
  collision → single `◆` `Warning`. (Source: shadcn/Radix hairline track + ring thumb;
  Rich 1/8 fractional tops.)
- **Toggle/Switch** — Defect: `[ON ]`/`[OFF]` text; toggle underline focus. Move:
  **sliding-thumb model** `OFF ●` → `● ON` (Textual 0.3s), knob passes through label;
  keep redundant ON/OFF text (accessibility — TermRock instinct correct); focus =
  border brighten + knob `●`→`◉`; loading → knob pulses amber.
- **RadioGroup** — the **`○→◎→●` ladder** (idle `○` / focused-preview `◎` / selected
  `●` / focused+selected `◉`); selected row `SelectionTint`, focused row `HoverTint`;
  two distinct bg cues, no underline. (Source: shadcn ring+dot; Textual `BUTTON_INNER`.)
- **Stepper** — Defect: underline on current step. Move: **connector ramp carries
  progress** — filled `━` Accent up to current, `─` Border after; node ladder `○`(future)
  → `◉`(current, Focus+BOLD) → `✓`(complete, dimmed) → `✕`(error). (Source: shadcn/MUI
  connector-fill stepper.)
- **SegmentedControl** — Defect: `[ ]` brackets for active + **UNDERLINE focus**
  (`segmented_control.rs:521-527`). Move: rounded-pill container; selected = `Selection`
  fill (remove brackets); focused-roving = `HoverTint` + `BorderFocused` segment edge
  (independent of selected); slide-fill animation. (Source: shadcn Toggle Group; iOS.)

### 4.3 Collections

- **`▌`/checked collision** (tree_table, data_table) — §2.5 #5. Highest-value
  consistency fix: one glyph = one concept; selection survives unfocus.
- **VirtualGrid** — Defect: 0 calls to `resolve_list_row`, no selection contract (tracks
  cursor/range in state, renders nothing). Move: cell-native contract — `SelectionTint`
  fill + `TextStrong` on selected cell; optional `BorderFocused` 1-cell frame variant;
  no row to hang `▌` on, so fill is correct (mirrors yazi grids).
- **Shared `FuzzyHighlight` painter** — absent. Move: one painter in
  `highlighted_text.rs` (matched = `TextStrong`+`Accent`+optional `SelectionTint` bg;
  unmatched = `TextMuted`; no underline) consumed by picker, quick_open, combobox,
  search_input, multi_select search, slash_command_menu, mention. (Source: fzf
  `hl/current-hl` separate from `pointer`/`marker`/`selection`.)
- **Trailing metadata slot** — `ListRowRecipe` has no `meta: Option<Span>`. Move: add
  right-aligned `TextMuted` secondary slot; standardizes k9s/btop-style right-edge
  metrics across List/Table/Tree.
- **Table sticky header + tabular numerics + `↑` sort glyph** (operator §6 names sort
  glyph; add: right-aligned numerics, sticky header under scroll).
- **QuickOpen** — operator §6 names palette-on-elevated; add: grouped results,
  recency bias, two-line rows, right-aligned `[⏎]` kbd hints, two-line path `TextFaint`.

### 4.4 Navigation / layout

- **Tabs** — see D2. Connected-fill vs accent-pill vs marker; retire `TabUnderline*`.
- **Sidebar** — two-tier focus (panel `BorderFocused` border; row faint accent-fill +
  `▌`); tree `▸▾`; uppercase `FgMuted` section headers + optional faint `─` rule; auto-
  collapse to icon rail <48 cols; single left `│` quiet, not full box. (Source:
  shadcn `data-active:bg-sidebar-accent`; Linear tight density.)
- **MenuBar** — accelerator cue = **color, not underline** (hotkey letter in `Accent`,
  rest `Fg`); open = accent pill + `Elevated` dropdown; checked `●`/unchecked `○`;
  submenu `▸`; narrow → `≡` button. (Source: shadcn Menubar; macOS.)
- **Toolbar** — ghost buttons default; primary = chip; group separators 2-space or
  faint `│`; **omit disabled actions entirely** (lazygit discipline — no greyed clutter);
  left=contextual, right=mode `FgMuted`. (Source: lazygit/zellij keybinding bars.)
- **SplitPane / ResizablePanelGroup** — one divider recipe: 1-col `│`/`─` faint, bright
  `BorderFocused` + grip `⠇`/`⠺` on focus/drag; pane title inlaid `┤ Title ├`; live `%`
  during drag only; double-click = equalize. (Source: shadcn Resizable `withHandle`;
  zellij/tmux title-inlay.)
- **Breadcrumbs** — `›`/`/` `Border` muted; current `FgStrong`; collapse middle `src › …
  › tabs.rs` (never cut leaf); clickable crumb = `▸` prefix, not underline.

### 4.5 Feedback / status (motion channel + shape ladder from §2.3–2.4)

- **Toast** — already Sonner-class (rail+icon, muted border, TTL 4s). Add: fake depth
  (back-stack overlaps 1 row + dims + shortens rail — can't scale cells); icon-only
  "pebble" tier `✓ saved`; Presence-phase slide-in; **errors never animate** (gravity);
  success = one brightness pop then static. (Source: Sonner collapsed-stack mechanics.)
- **StatusIndicator** — motion-as-status on the glyph: Running `◉` breathe ~2s, Waiting
  `◐` slow dot-pulse, Online `●` heartbeat ~5s; gravity states static. Shape ladder
  (§2.4).
- **Spinner** — phase→channel map (Work/Wait/Stream); add stream glyph set
  `["∻","≈","∿","〜"]` for token streaming (distinct from compute spinner); activity
  rail (Grok sin² wave = throughput); completion morph braille→`✓`.
- **LoadingView** — 3 modes: Spinner (cold), Skeleton shimmer (caller passes ShapeSpec;
  shimmer from `lighten`/`hsl_shift` `ping_pong`; **skeleton never spins**), Optimistic
  stale (dim content + `↻ updating`).
- **LogPane/LogStream** — aligned timestamp+source fixed columns (Detailed); follow-tail
  "now" rail 1-cell `┃ Accent` (vertical, not underline); optional severity rail `│`;
  soft level filter (dim to `TextDisabled`, don't hide); live-arrival 1-tick pulse on
  append.
- **Timeline** — running segment rail = `wave_brightness` travels toward active event;
  completed = steady muted `┃`; failed = rail breaks `┊`/`✕`; now-marker `◇` at live
  edge; elapsed on Running right-aligned faint; recency age-fade.

### 4.6 Data / overlays

- **KeyValueTable / DetailTable** — key `TextMuted` fixed-col / value `Fg`; zebra
  `HoverTint` opt-in (Comfortable); type badge `#%@·` leading 1-char (visidata) vs full
  column wide; promote selected gutter `›`→`▌`; drop colon default (`label    value`);
  frozen identity col `Sunken`; link affordance via brackets/`⧉` never underline; dot-
  leader on wrap.
- **Histogram** — **1/8 sub-cell bar tops** (`▁▂▃▄▅▆▇` + fractional `▏▎▍▌▋▊▉`) so short
  bars aren't quantized (Rich precision — biggest perceived-quality jump); faint `─`
  baseline + optional y-tick margin; **monochrome multi-series via hatch outline** (`▒`
  outline vs `█` solid) — glyph before color; `HistColorMode::{Flat, Magnitude}`;
  selected `▼` under label.
- **ChoiceDialog** — see D1 (danger-on-button). Keep scoping + Safe-Enter.
- **Surface** — de-dup `Raised` (fill `RAISED` + `Border`) vs `Overlay` (fill `Elevated`
  + `Border`) so dialog reads one step above card; add `Sunken` recipe (recessed wells);
  overlay border optional faint `BackdropWash` tint.
- **Panel** — consume `PanelRecipe` (zero readers today); left-aligned border-title
  default (`┌[ Title]─────┐`); `DividerOnly` rules → faint `ChartGrid` not full `Border`.
- **Section** — add `RuleBeside` title variant (`Section  ────────` faint — rule is a
  separate fill, not under the glyphs, underline-safe); explicit
  `SectionRhythm::{Tight,Normal,Loose}`; status → glyph+role prefix `[✕ failed]`; cap
  nesting indent at depth 2.
- **Collapsible** — keep leading `▸▾` (valid terminal house style, Finder/ranger/lazygit);
  body flush to trigger text (no self-indent — shadcn consensus); hover = `HoverTint`
  row (underline-free); optional stepwise settle; `border-b` rule after collapsed
  siblings.

### 4.7 Meta / composites

- **Tag / Chip** — Defect: remove-part focus `BOLD|UNDERLINED` (`tag_chip.rs:364,849`).
  Move: invert remove region (`Danger` fg + `Surface` bg + `BOLD`, or `REVERSED`);
  bracket family `⟨tag ×⟩` (Tag, angle=neutral) / `[● label]` (Chip, square=interactive);
  selection ramp glyph>weight>fill>color (drop fill before radio mark under narrow
  pressure; mark invariant); selected+focused distinct from selected via `Focus` fg on
  mark only.
- **ShortcutHint / kbd** — **already the cleanest widget** (no underline; `KbdVariant`
  Compact/Keycap/Inline; platform `⌃⌥⇧⌘`; special `↵⇥⌫↑`). Formalize the keycap chip:
  `[ C-s ]` space-padded interior, brackets `Border` faint, chord `HintKey`+`BOLD`,
  `Raised` bg — brackets faint not key-colored. Separator inside chord = space (not `+`,
  collides with shift semantics); `·` between hints. Focused kbd hint (rare) =
  `REVERSED`, never underline.
- **KeyboardHelp** — Defect: underline colorless conflict cue (`keyboard_help.rs:1203`).
  Move: `REVERSED` or `BOLD`+`⚠`/`!`; fixed-width chord column right-aligned (lazygit);
  faint `─` rule under category headers; `[12]` result-count chip.
- **ThemePicker** — already live-preview + quantize annotation. Add: **swatch row**
  `[▆▆▆▆]` of the preset's own `Accent/Success/Danger/BorderFocused` (terminal color
  swatch); render swatches through *active terminal* color resolution, `~` hatch if
  truecolor-only on ANSI; `↑↓ preview · Enter apply · Esc cancel` footer; group
  Recommended vs Variants.
- **Setup wizard** — Defect: submit buried in footer chord. Move: Stepper connector ramp
  (D2/§4.2); one primary `Button` "Continue/Finish" right + `Esc` back ghost left
  (Jackin five-slot); alternate body `Surface`/`Raised` bands; gate failure as `Danger`
  field annotation.
- **Settings screen** — Defect: Theme/Keybind body modes render raw (break the one-
  bright-border invariant). Move: wrap all body modes in focused `Panel`; route every
  setting through `FieldRow`; StatusBar with `⟨● modified⟩` Badge + kbd chips (not plain
  text); dirty/apply primary chip when dirty.
- **Metrics dashboard** — Defect: host pre-formats strings (hierarchy not guaranteed).
  Move: push value-typography into `MetricTile` (value `TextStrong`+`BOLD`, unit
  `TextMuted`, title `Text`, delta right-aligned smaller); delta **direction glyph
  `▲`/`▼` before color** (ASCII `^`/`v`); sparkline muted baseline + threshold faint;
  consider full `BorderFocused` perimeter on focused tile (top-edge-only can read as
  separator).
- **Auth Entry** — Defect: fields bespoke, not `FieldRow`; submit chord-only. Move:
  migrate fields to `FieldRow` anatomy (label band + `*`/`!` marker + `Masked` value +
  annotation); one primary `Button` "Sign in" right (Enter triggers); secondaries as
  ghost chips below; validation as `Danger` annotation; keep single bright border.

---

## 5. Recommended next plans (feed the `plans/` system)

Ordered by unblock value:

1. **TextInput kernel border** (P0) — consume `InputRecipe.border`, delete label
   underline. Heals 7 widgets. Depends on nothing.
2. **Glyph catalog gaps** (P0) — add left-half blocks `▏▎▍▌▋▊▉`, shade blocks `░▒▓`,
   promote slider hardcoded glyphs, add `Checkbox` glyphs, unify mask `●`. Enables
   Slider/RangeSlider/Histogram/controls.
3. **`FocusEmphasis` enum** (P0) — central underline-free vocab on `DesignSystem`;
   delete `ListRowRecipe.show_focus_underline`; migrate widgets. Removes the bug class.
4. **`▌`/checked collision fix** (P1) — tree_table + data_table; selection survives
   unfocus; checked → catalog checkbox.
5. **Shared `FuzzyHighlight` + trailing `meta` slot** (P1) — one painter, 8 consumers.
6. **`shimmer_cells` primitive + `MotionChannel` vocab** (P1) — motion.rs; skeleton≠
   spinner rule into anatomy spec.
7. **New roles** (P1) — `TextPlaceholder`, `InputBorderInvalid`, `Spinner`, optional
   `InputFocused`. (Coordinates with `plans/002` role-palette foundation.)
8. **Divergence decisions D1–D3** (decision-gated) — implement per the chosen default
   once decided.

---

## 6. Sources (per cluster, non-exhaustive)

- **Inputs:** shadcn Input/Select/Combobox/Textarea; Radix Select; Mantine
  Input/NumberInput/TagsInput (hold-to-repeat, Pill); Textual Input/MaskedInput;
  Charm huh/Bubbles; fzf; ratatui-textarea; helix/kakoune.
- **Controls:** shadcn Slider/Switch/RadioGroup/Toggle/Slider; Radix; Mantine; Textual
  Switch/RadioButton (`BUTTON_INNER`); huh; btop sliders.
- **Collections:** k9s, lazygit, yazi, visidata, fzf/television, shadcn DataTable/Command.
- **Nav/layout:** shadcn Tabs/Sidebar/Breadcrumb/Resizable/Menubar; Radix; zellij
  (tab/status-bar); tmux; helix.
- **Feedback/status:** [Sonner](https://emilkowal.ski/ui/building-a-toast-component);
  [tachyonfx](https://github.com/junkdog/tachyonfx);
  [cli-spinners](https://github.com/sindresorhus/cli-spinners); btop; shadcn
  Skeleton/Spinner/Toast; Rich logging.
- **Data/overlays:** shadcn Dialog/AlertDialog/Sheet; Radix (`shadow-6`, black-alpha
  scrim); Rich Panel/Table/Bar (1/8 fractional tops); Textual DataTable; visidata; Amp;
  Grok Build.
- **Meta/composites:** [shadcn Badge](https://ui.shadcn.com/docs/components/badge),
  [shadcn Kbd](https://ui.shadcn.com/docs/components/kbd),
  [HeroUI Kbd](https://heroui.com/docs/react/components/kbd),
  [huh](https://github.com/charmbracelet/huh),
  [gum](https://github.com/charmbracelet/gum).

*(Design-language analysis only. No proprietary source reuse. All `file:line`
references verified against TermRock HEAD at research time.)*
