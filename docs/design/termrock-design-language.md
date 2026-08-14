# TermRock design language — the "expensive terminal"

| Field | Value |
|-------|-------|
| **Status** | **Binding design SoT for interaction styling & focus grammar** (living). Consolidates and extends the paint audit. On conflict about focus/selection/active/underline paint, this file wins; [`terminal-design-system.md`](./terminal-design-system.md) stays SoT for token taxonomy; [`phosphor-obsidian-visual-direction.md`](./phosphor-obsidian-visual-direction.md) stays SoT for the phosphor palette values. |
| **Date** | 2026-08-14 |
| **Perspective** | Designer / craft, not architecture. Answers *why it looks cheap* and *what makes it read expensive*. |
| **References** | Grok Build (same stack, studied from source), Amp (ampcode.com), Jackin (Tailrocks consumer, `=0.11.0`), shadcn/ui, Linear |
| **Coordinates (does not duplicate)** | [`phosphor-obsidian-visual-direction.md`](./phosphor-obsidian-visual-direction.md) = token/paint audit; [`component-visual-richness-plan.md`](./component-visual-richness-plan.md) = implementation plan; [`terminal-design-system.md`](./terminal-design-system.md) = token taxonomy. This file = the **language** and the **underline-free focus grammar**, plus the full component map. |

---

## 0. TL;DR

TermRock's *idea* of itself is right; the *paint* betrays it. The current default
reads as **neon CRT cosplay**: one phosphor green does selection, focus, accent,
success, scroll, tabs, and links at once; surfaces are empty (no depth); spacing
tokens are dead code (text kisses borders); chrome is hand-drawn in ~140 files;
and **underline is scattered across ~40 files / 86 sites** — focus/selection/active
in ~30 widgets, inconsistent and cheap-feeling (the rest is legitimate content
rendering: ANSI SGR-4 parsing, markdown emphasis, diff markers).

This document defines the design language that fixes that — from a designer's
lens — in three parts:

1. **What makes a TUI read as expensive** (the craft principles, ported from
   shadcn/Linear/Grok Build).
2. **One styling system** (color ladder, spacing rhythm, border language, glyph
   catalog, motion) with an **underline-free focus grammar** as a first-class rule.
3. **A per-component improvement map** for all 48 listed surfaces.

The diagnosis is not "48 widgets to restyle." It is **6 load-bearing defects**
(propagated everywhere) + **one focus-grammar sweep**. Fix the foundation and the
shared chrome path first; most widgets inherit the new look without per-widget
work. The implementation order lives in the richness plan; this file owns the
*look-and-feel contract*.

---

## 1. What "expensive" means in a terminal (designer lens)

Most terminal apps look cheap because they violate a small set of craft laws the
best web/native products take for granted. Grok Build, Amp, btop, Linear, and
shadcn/ui all obey the same laws in different media. Port them to cell physics.

### 1.1 Restraint is the luxury signal

Cheap UIs scream. Expensive UIs whisper and reserve volume for the *one thing
that matters right now*. In a terminal that means: **graphite structure, rare jewel
accent.** Grok Build's near-black gray ramp (`#0a0a0a → #363636`) reads expensive
precisely because color is scarce — small moments (a 1-col accent rail, a live
badge, a focused border) pop against calm neutrals. Phosphor green on every
interactive row is the opposite: volume everywhere = nothing matters.

> **Law — accent budget:** ≤ **2** accent-forward regions per viewport (e.g. one
> focused border + one live status). Everything else is graphite and muted text.

### 1.2 Depth, not boxes

Cheap TUIs float boxes on void black. Expensive ones stack **surfaces**: a canvas,
a panel body, a raised card, an elevated dialog, a sunken input well — each one
step lighter/darker than the last, so the eye reads containment without a border
doing all the work. shadcn's surface ladder (`background/card/popover`) and
Linear's layered canvases are the same idea. TermRock has the ladder as types
(`Canvas/Surface/Raised/Elevated/Sunken`) but ships most of it **empty** — so depth
never renders.

### 1.3 One scale governs everything

shadcn's genius move is a single `--radius` that scales the whole system; Linear
"snap[s] every spacing value and radius to a defined scale — no off-scale
paddings." The terminal analog is **density** → spacing → corner shape → glyph
weight, all derived from one number. TermRock has `Density` + `SpacingScale` but
they are consumed by ~2 of ~160 widget files; `Panel` hard-codes `.padding(0,0)`.
Until one scale actually drives padding, the rhythm is ad hoc and looks it.

### 1.4 Hierarchy by weight + value, not decoration

Expensive type systems step down: strong → body → muted → faint. Linear does this
with negative tracking at large sizes and a disciplined muted ladder. TermRock has
`TextStrong/Text/TextMuted/TextDisabled` but stories paint secondary metadata the
same white as primary, so there is no real hierarchy — everything competes.

### 1.5 Color is the last channel, never the only one

Cheap status UIs are color-only. Expensive ones encode meaning in **glyph + weight
+ word + position**, then add color. Grok Build's status dots `●○◉◎`, check glyphs
`✓✗`, actor rails, and `+`/`-` diff markers all carry meaning before any hue.
Monochrome, `NO_COLOR`, narrow, and SSH/mux survival all depend on this.

### 1.6 Motion = status, not decoration

Cheap motion spins for show. Expensive motion *is* the status: Grok Build's sin²
wave flows down the rail of an *active* block; a dot-pulse marks a *running* task;
a shimmer marks *live* presence. Static when idle, alive when working, and
**reduced-motion collapses cleanly to static accents**.

### 1.7 Craft in the small

Width-invariant, 1-column-tested glyphs with per-glyph ASCII fallback. Display-
column-correct truncation. Sub-cell ramps (` ▁▂▃▄▅▆▇█`) in progress/meters.
Consistent gutters, consistent pill shapes, consistent chip rhythm. Each is tiny;
together they are the difference between "demo" and "product."

### Why the references, specifically

| Reference | What to steal (the expensive move) | What NOT to copy |
|-----------|--------------------------------------|------------------|
| **Grok Build** | Neutral ground + jewel accents; per-actor 1-col accent rail; width-tested glyph catalog; motion-as-status; rounded borders + focus-by-brightness; quantize-at-startup. | Brand, slash vocab, model routing, sandbox policy. |
| **Amp** | Thread/session as object; mode dial always visible; config-as-panel not YAML folklore; anticipatory completion; one runtime → TUI or headless. | Sourcegraph branding, allowlists. |
| **Jackin** | 3–5 segment composed rows; spacing-as-chrome (intentional bands, not gaps); five-slot dialog rhythm; reference-width sizing; `truncate_cols`. | Product nouns (auth/editor fields stay consumer). |
| **shadcn/ui** | Token discipline: surface/foreground pairing, single radius source of truth, variant ladders, muted/accent state tokens. | JSX, Tailwind, CSS, the web DOM. |
| **Linear** | Snap-everything-to-scale; quietly luxurious density; high signal-to-noise; restrained accent. | Pixel-GUI assumptions. |

---

## 2. The current mess, named (6 root causes, not 160 widgets)

Consolidated from the paint audit. Every "cheap" symptom traces to one of these.

1. **No surface ladder.** `Canvas/Surface/Elevated/Backdrop/StatusBar` ship as
   empty `Style::new()`; the fill path drops empty styles, so every panel/card/
   dialog/toast/menu fill is a runtime no-op. No depth → boxes on void.
2. **Dead spacing.** `SpacingScale` + `Density::padding_*` touched by ~2 files;
   `Panel` hard-codes zero padding; body text sits flush on border glyphs. No
   rhythm → cramped demo feel.
3. **Accent collapse.** `BorderFocused`, `Focus`, `Accent`, `Success`, `HintText`,
   `ScrollThumb`, `TabUnderlineFocused`, `ChartSeries1` are the **same green**.
   Meaning collapses to "green = important." Selection also mis-wires to fg-only
   roles so tint washes can't render.
4. **Hand-rolled chrome.** ~137 of ~160 widgets draw their own `┌` literals and
   `style.bg = None` (13 sites). No fill/pad/elevation policy can reach them, and
   the recipes (`button_recipe`, `input_recipe`, `panel_recipe`, `elevation`) have
   **zero** callers.
5. **Neon selection.** `Selection` is a full-row phosphor fill with black ink; it
   competes with focus and success and is unsustainable for 8-hour sessions.
6. **Underline as focus — inconsistent and cheap.** `Modifier::UNDERLINED` is
   sprinkled across ~40 files / 86 sites. The ~30 **focus/selection/active** sites
   (tabs, list row, toggle, stepper, link, citation, breadcrumbs, text/number/
   search/path inputs, token field, OTP, combobox, select, multi-select, tree,
   tree_table, date picker, controls, keyboard help, badge) contradict TermRock's
   own law ("focus = theme role, never border weight or underline decoration") and
   read as legacy CLI. The remaining sites are legitimate **content** underline
   (ANSI SGR-4 parsing in `ansi_text.rs`, markdown emphasis, diff markers) and must
   stay. **This document makes underline-free the default *interaction* grammar**
   (§5) while leaving content underline intact.

These are foundation defects. Fix the tokens, plug in the recipes, route chrome
through `Surface`, and run the focus sweep — most widgets inherit the new look.
The per-component map (§6) then handles the residuals.

---

## 3. Binding design principles (the language)

1. **Quiet canvas, rare jewel accent.** Structure is graphite and mute. Phosphor
   appears for *current intent only*: the focused owner, the one live action, the
   running badge — never every selected row.
2. **Depth before borders.** Containment reads from the surface ladder; borders are
   structure, not the only container signal.
3. **One scale, everywhere.** `Density` drives spacing, corner shape, and glyph
   weight. No off-scale paddings.
4. **Hierarchy steps down.** Strong → body → muted → faint, always. Secondary
   metadata is never the same white as primary.
5. **Selection ≠ focus.** Selection = gutter glyph + tint + strong text (calm).
   Focus = the owner's bright border + the row gutter (precise). Two different
   marks, always distinguishable.
6. **Color is the last channel.** Glyph + weight + word + position carry meaning
   first; hue reinforces. Survives mono / `NO_COLOR` / narrow.
7. **Borders mark ownership, single-line only.** Inactive = quiet gray; the one
   focused owner = phosphor. Never double-line, never a border on every row.
8. **Motion is status.** Alive when working, static when idle; reduced-motion
   collapses to static accents with zero information loss.
9. **Restraint over richness.** If a cue can be quieter and still legible, it must
   be. Premium = signal-to-noise, not feature count.
10. **Phosphor identity stays.** Green-on-obsidian, square caps, single-line
    borders, focus-by-color — the loved default. Neutrality means others can
    retheme fully, not that defaults go bland.

---

## 4. The styling system (one reference)

The concrete tokens. Values are the design-SoT targets; the richness plan owns
wiring them into `tailrocks_phosphor`.

### 4.1 Color — surface ladder + foreground ladder (shadcn pairing)

Every surface **pairs with a foreground**. Borrowed discipline from shadcn's
`background/foreground`, `card`, `popover`, `muted`, `input`, `ring`.

| Role | Truecolor | shadcn analog |
|------|-----------|---------------|
| `Canvas` | `#0a0c0a` (or `Reset`) | `background` |
| `Surface` | `#121612` | `card` |
| `Raised` | `#1a1f1a` | hover/section |
| `Elevated` | `#1e2620` | `popover` (dialogs/palettes) |
| `Sunken` | `#0d100d` | `input`/well/code |
| `Backdrop` | dim wash toward Canvas | overlay scrim |
| `StatusBar` | filled band | — |
| `Fg` | `#d6e0d6` | `foreground` |
| `FgStrong` | `#f0f5f0` + bold | titles / selected primary |
| `FgMuted` | `#7a8a7a` | secondary |
| `FgFaint` | `#4a574a` | meta / timestamps |
| `Border` | `#2a332c` | quiet structure |
| `BorderFocused` | `#00ff41` | **owner only** |
| `SelectionTint` | `#14331a` (bg) | row wash — not neon |
| `HoverTint` | `#1a221c` (bg) | hover wash |
| `Selection gutter` | `▌` + Accent fg | default selection mark |

**De-collapse accents:** give `BorderFocused`, `Focus`, `Accent`, `Success`,
`HintText`, `ScrollThumb`, `TabAccent`, `ChartSeries1..5` distinct values. Split
`Success` (softer mint `#5dffa0`) from brand `Accent` (`#00ff41`).

### 4.2 Spacing rhythm (Linear "snap to scale")

`Density::Comfortable` → pad `(2,1)`, gap `1`; `Compact`/`Dashboard` → tighter. One
scale drives: `Panel` inset, `Stack`/`Grid` gaps, list gutters, dialog five-slot
rhythm (leading spacer / body / mid spacer / actions / trailing spacer — Jackin
rebuilt exactly this product-side), chip internal pad. **No off-scale paddings.**

### 4.3 Border language

Single-line only. Shape is a **theme token** (`BorderShape::Square` default =
Jackin/Phosphor identity; `Rounded` for Grok-Build-class consumers; ASCII → `+`).
Ownership = the **one** bright border per scene layer. Never double-line, never
heavy, never per-row.

### 4.4 Glyph catalog discipline

Lucide-named semantic glyphs, width-tested to exactly 1 column, with per-glyph
ASCII/CP437 fallback (already the catalog design). Status dots `●○◉◎`, diamond
family `◆◇◈`, checks `✓✗`, disclosure `▸▾/›‹`, sub-cell ramps
` ▁▂▃▄▅▆▇█`, braille + dot-pulse spinners `⠋⠙⠹` / `⋅:⸬⁙`. Glyphs never the sole
carrier of meaning. **Promote `BLOCK_RAMP` out of charts into `Progress`/meters.**

### 4.5 Motion ladder

Sin² `wave_brightness` (rail of active block), `pulse_brightness` (single icon),
raised-cosine shimmer (live presence), all **30fps-capped** with span-run
coalescing. `Motion::Reduced`/`Off` collapse every effect to a static accent.
Motion signals *activity*, nothing else.

### 4.6 Iconography

One `Icon` system (Lucide-named). Status carried by glyph + accent rail/color, not
by whole-frame severity floods.

---

## 5. Focus grammar — **underline-free** (the new rule)

The user dislike is correct and load-bearing: underlined-text-as-focus reads as
legacy CLI and is applied inconsistently across ~30 interaction sites (~40 files /
86 statements total, but ~10 of those are legitimate content rendering — ANSI
SGR-4, markdown emphasis, diff markers — which stay). This section replaces the
**interaction** underline with a coherent, layered vocabulary. **Default TermRock
has no text underline for focus, selection, active state, or tabs.** Underline
survives only as an *opt-in, hover-only, dim* link affordance (§5.7) and in content
rendering — and consumers can disable the link one.

### 5.1 The five focus cues (no underline)

| Situation | Cue (default) | Role / glyph |
|-----------|---------------|--------------|
| **Container owner** (panel/dialog/pane/composer) | One **bright border** on the owner only | `BorderFocused` |
| **Collection row** (list/table/tree/grid/rail/palette/menu) | `▌` **gutter** + **bold `FgStrong`** primary + optional `SelectionTint` | `Accent` gutter, `FgStrong` label |
| **Inline control** (input/select/combobox/token/OTP/search) | Field is a **Sunken well**; focus = **bright border** on the box (+ optional `›` prompt glyph) | `Sunken` fill, `BorderFocused` border |
| **Cursor** (text caret, grid cell) | **Block/reverse cell** `█` or reverse | `Focus` |
| **Active option** (tab, segmented, toggle on, radio on, slider thumb, stepper) | **Bold `FgStrong`/Accent label + a non-line marker** (see 5.2–5.6) | `FgStrong`/`Accent` + glyph |

Rule of thumb: **a border or a gutter or a glyph — never an underline.**

### 5.2 Tabs — active marker, not an underline

Remove `Modifier::UNDERLINED` from `tabs.rs` (currently `UNDERLINED \| REVERSED`).
Active tab default cue:

```text
 ▸ Files    Search    Git        ← bold Accent label + ▸ leading marker; quiet inactive
```

- Active = **bold + `FgStrong`/Accent** + leading `▸` (or `◆`) marker.
- Inactive = `FgMuted`.
- When the strip owns keyboard focus: add `▌` Accent gutter on the active tab.
- Optional (theme token, **off by default**): a bottom **accent rule line** — a run
  of `─` border cells in `Accent` under the active tab. This is a *border glyph*,
  full-width, visually distinct from character underline. Keep off by default to
  honor the underline aversion; expose as `TabsActiveCue::{Marker, Rule, Both}`.

### 5.3 Inputs (text/number/password/search/path/token/OTP/date)

Remove underline focus from `text_input.rs:1024`, `number_input`, `password_input`,
`search_input.rs`, `path_input.rs`, `token_field.rs:1004`, `input_otp.rs:406`,
`combobox.rs:950`, `select.rs`, `date_time_picker.rs:2144`, `multi_select.rs:879`.

- Field = `Sunken` well fill + `Border` border.
- Focus = `BorderFocused` on the field box (bright border), **not** text underline.
- Optional `›` prompt glyph or `▎` left rail for extra presence.
- Cursor = block/reverse cell in `Focus`.
- Invalid = `InputInvalid` border + `!` glyph + danger text (never underline-only).

### 5.4 Toggle / switch / radio / checkbox / stepper / slider

Remove underline from `toggle.rs:479-489`, `stepper.rs:862,882`, `slider`-family.

- Toggle/switch: state = glyph (`●/○` or `◉/◯`) with Accent only on **on**; focus =
  bright outline + bold label.
- Radio/checkbox: `◉/○`, `☑/☐/▣`; focus = bold label + bright outline, never
  underline.
- Stepper: focus = bold current step + `▸`/`◀ ▶` markers + bright track border.
- Slider/range: thumb = `█`/`◉` Accent; focus = bright track border + bold value.

### 5.5 Lists / tables / trees / grids / menus / palette

Remove `show_focus_underline` from `ListRowRecipe` and the `list.rs:1353` underline.
Use the collection-row grammar: `▌` gutter + `FgStrong` primary + `SelectionTint`.
Selected-but-unfocused = gutter `FgMuted` + tint; selected-and-focused = gutter
`Accent` + bold. Menu/dropdown/palette reuse the same row grammar (remove
`controls.rs:438,474` underline).

### 5.6 Breadcrumbs / citations / keyboard-help / badge / tag / chip

Remove underline from `breadcrumbs.rs`, `citation.rs:741-749`, `keyboard_help.rs:1203`,
`badge.rs:461`, `tag_chip`.

- Breadcrumbs: separators are `›` / `/` in `FgFaint`; current crumb = `FgStrong`.
  Never underlined.
- Citation: `[1]` chip in Accent; hover = brighter chip, not underline.
- Keyboard help: keys render as **kbd chips** (bordered cells), never underlined
  text.
- Badge/tag/chip: pill = bracketed cell (`⟨tag⟩` / `[tag]` / rounded `❪ ❫`);
  severity via glyph + accent rail/color, never underline.

### 5.7 Links — the only place underline survives, opt-in

Links conventionally underline. Honor "dislike *mostly* underline": default link =
`Link` color + trailing `↗`/`›` chevron, **no underline**. On hover: brighter
`LinkHover` color; underline **off by default**, exposed as an opt-in `LinkStyle`
(`Color` default, `UnderlineOnHover`, `AlwaysUnderline`) so a consumer who wants
classic web links can enable it. Remove `link.rs:424,773` default underline.

**Monochrome projection is the exception:** on mono / `NO_COLOR` the `Link` role
*keeps* `Modifier::UNDERLINED` regardless of `LinkStyle`, because underline is the
only reliable link cue once color is gone. This is the only interaction underline
that survives a colorless projection; every other cue degrades to bold / dim /
reverse / glyph per §5.9.

### 5.8 Migration note for the sweep

The underline removal is a visible default change → sequential `migrations/` file
+ `MIGRATING.md` entry when implemented (per repo law). Enumerate the ~30 sites
and the replacement cue in that file. This document defines the target grammar;
the migration records the mechanical edits.

### 5.9 Grammar clauses (binding)

`Modifier::UNDERLINED` is allowed ONLY for:

1. **Hyperlinks in monochrome projection** — on mono / `NO_COLOR`, `Role::Link`
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

---

## 6. Component improvement map (all 48)

Each row: **current cheap signal → target direction.** Grouped by family.
Foundation fixes (§2) lift most of these automatically; residuals listed here.

### 6.1 Containers & surfaces

| # | Component | Cheap signal now | Target direction |
|---|-----------|------------------|------------------|
| 17 | **Surface** | Fill is a no-op (empty role). | Authority for fill+border+pad+elevation; every container routes through it. |
| 35 | **Panel** | Zero padding; text kisses border; every box same weight. | `(2,1)` inset; `panel_recipe`; one bright border = owner. |
| 27 | **Section** | Header competes with body; loose on void. | `FgStrong` title + `FgFaint` rule + `Raised` surface band. |
| 45 | **Collapsible** | Disclosure glyph inconsistent; underline header. | `▸/▾` disclosure + bold header; no underline; smooth expand (motion-as-status). |

### 6.2 Inputs

| # | Component | Cheap signal now | Target direction |
|---|-----------|------------------|------------------|
| 13 | **TextInput** | Underline focus; flush placeholder. | Sunken well + bright border focus; `›` prompt; `FgFaint` placeholder. |
| 14 | **TextArea** | Same as TextInput, no scroll chrome. | Sunken well; `BorderFocused` focus; quiet scroll gutter; line-ruler gutter. |
| 36 | **NumberInput** | Underline focus; raw digits. | Bright border focus; `◀ ▶`/`- +` steppers; `FgMuted` unit suffix; clamp glyph. |
| 34 | **PasswordInput** | Underline focus; reveal toggle noisy. | Sunken well; ` obscured` + `👁` reveal chip; strength meter via `BLOCK_RAMP`. |
| 28 | **SearchInput** | Underline focus; no result count. | `⌕` leading glyph; bright border focus; `FgFaint` "n results"; `esc` clear chip. |
| 9 | **TokenField** | Underline focus; chips uneven. | Sunken well; bracketed chips `⟨x⟩`; focus border on field; `⌫` remove hint. |
| 26 | **Select** | Underline focus; chevron only. | Sunken well + `▾`; bright border focus; popup = Elevated; row gutter grammar. |
| (combobox) | **Combobox** | Underline focus. | Same as Select + inline filter; completion shares row grammar. |
| 23 | **Slider** | Plain track; underline thumb. | `BLOCK_RAMP` track on `Sunken`; `█`/`◉` Accent thumb; bright border focus + bold value. |
| 30 | **RangeSlider** | Two thumbs, unclear active. | Same track; active thumb = Accent + bold value; inactive thumb `FgMuted`. |
| (toggle) | **Toggle/Switch** | Underline on focus. | `●/○` glyph, Accent on **on**; bright outline + bold label focus. |
| 31 | **RadioGroup** | Mixed focus cue. | `◉/○`; bold active label; bright outline focus. |
| 18 | **Stepper** | Underline on current step. | `▸` marker + bold `FgStrong` current + `◀ ▶`; bright track border. |

### 6.3 Collections

| # | Component | Cheap signal now | Target direction |
|---|-----------|------------------|------------------|
| 40 | **List** | Neon fill selection; underline focus; flat meta. | `▌` gutter + tint + `FgStrong`/`FgMuted`/`FgFaint` ladder; no fill, no underline. |
| 5 | **VirtualList** | Same as List at scale. | Same grammar; O(visible) paint; skeleton rows while loading. |
| 6 | **VirtualGrid** | Risk of every cell "selected". | Cell cursor = `█`/reverse one cell + optional gutter; range = tint only. |
| 7 | **TreeTable** | Underline focus; neon row. | Row gutter grammar; `▸/▾` disclosure; depth tint; header `FgMuted`. |
| 8 | **Toolbar** | Button soup; underline states. | Ghost buttons default; primary = chip; group separators `FgFaint`; focus = bright outline. |
| 11 | **Timeline** | Color-only events. | Glyph per event type + Accent rail; `FgFaint` timestamps; now-marker `▌`. |

### 6.4 Data

| # | Component | Cheap signal now | Target direction |
|---|-----------|------------------|------------------|
| 42 | **KeyValueTable** | Flat white; no alignment. | Key `FgMuted` / value `Fg`; fixed column; monospace value well for code. |
| 44 | **DetailTable** | Border-only; loose. | `Raised` surface; `FgStrong` field labels; sectioned groups with `FgFaint` rules. |
| 43 | **Histogram** | Loud bars. | `BLOCK_RAMP`/braille bins; `ChartSeries` (de-collapsed); `FgMuted` axes; hover = Accent bin. |
| (charts) | **Charts** | Single green series. | Distinct `ChartSeries1..5`; `FgMuted` grid; Accent only on hover/active. |

### 6.5 Navigation

| # | Component | Cheap signal now | Target direction |
|---|-----------|------------------|------------------|
| 16 | **Tabs** | Filled pill / underline active. | Bold Accent + `▸` marker; `▌` when strip focused; rule-line opt-in (off default). |
| 48 | **Breadcrumbs** | Underlined current. | `›` separators `FgFaint`; current = `FgStrong`; no underline. |
| 24 | **Sidebar** | Inconsistent selection; border-heavy. | Gutter grammar; `Raised` groups; collapse = `▸`; section headers `FgMuted`. |
| 37 | **MenuBar** | Mixed focus cue. | Row gutter grammar; `▸` submenu marker; accelerator keys as kbd chips. |

### 6.6 Feedback & status

| # | Component | Cheap signal now | Target direction |
|---|-----------|------------------|------------------|
| 10 | **Toast** | Severity color block; competes w/ selection. | One-line; icon carries status color; `Fg` body; muted border; Accent rail optional. |
| 19 | **StatusIndicator** | Color-only dots. | Glyph (`●○◉⚠✓✗`) + color + dot-pulse when live; never underline. |
| 22 | **Spinner** | Single frame set. | Braille + dot-pulse tiers; reduced-motion → static `•`/glyph. |
| 39 | **LoadingView** | Full-frame spin or blank. | Skeleton shade ramp in content shape; verb text (`loading…`); no whole-frame green. |
| (skeleton) | **Skeleton** | Plain gray bar. | Shimmer band on `Sunken`; matches final content shape. |
| 38 | **LogPane** | Raw ANSI dump. | Level glyph + `FgFaint` timestamp; follow-tail gutter; filter chips; `Sunken` well. |

### 6.7 Overlays & dialogs

| # | Component | Cheap signal now | Target direction |
|---|-----------|------------------|------------------|
| 46 | **ChoiceDialog** | Neon OK; danger unscoped. | `Elevated` fill; dim backdrop; danger border + `!` + scope body; primary danger chip only on confirm. |
| (dialog) | **Dialog** | Clears to terminal default; no elevation. | `Elevated` fill; backdrop dims; five-slot rhythm; reference-width (160-col) sizing. |
| (popover) | **Popover/Drawer** | No elevation. | `Elevated` fill + bright border; Arrow glyph; dismiss hint `FgFaint`. |
| 32 | **QuickOpen** | Neon list in a box. | Elevated; sunken query; row gutter grammar; `FgFaint` path; kbd-chip shortcut. |
| 33 | **Picker** | Neon selection. | Same as QuickOpen; preview pane via `PreviewHost`. |

### 6.8 Layout

| # | Component | Cheap signal now | Target direction |
|---|-----------|------------------|------------------|
| 20 | **Stack / Inline** | Gap 0; density constructors unused. | `Density`-derived gap; intentional bands not voids; `FgFaint` optional dividers. |
| 21 | **SplitPane** | Bright borders both sides. | One bright border = owner; `FgFaint` resize handle `│`; drag hint. |
| 29 | **ResizablePanelGroup** | Inconsistent handles. | `FgFaint` `│`/`▍` handles; one owner border; sizes in content units. |

### 6.9 Meta / chrome

| # | Component | Cheap signal now | Target direction |
|---|-----------|------------------|------------------|
| 25 | **ShortcutHint (kbd)** | Underlined text keys. | **kbd chips** (bordered cells, `FgStrong` key, `FgFaint` border); `+`/`·` separators. |
| 41 | **KeyboardHelp** | Underlined keys; dense. | kbd chips; grouped sections with `FgMuted` headers; action = `Fg`. |
| 15 | **Tag** | Underline; uneven pill. | Bracketed pill `⟨tag⟩`; Accent only for active tag; `FgMuted` quiet tags. |
| 47 | **Chip** | Same as Tag. | Same pill grammar; removable `×`; severity via glyph + rail. |
| 12 | **ThemePicker** | Static swatch list. | Live stage preview (Studio); quantize-safe swatches; truecolor-only hidden on lesser terminals. |

### 6.10 Example composites (patterns — recipe polish, not core widgets)

| # | Component | Direction |
|---|-----------|-----------|
| 1 | **Setup wizard** | Stepper + form density; progress `BLOCK_RAMP`; one bright border on active step; calm confirm. |
| 2 | **Settings screen** | Nav tree (sidebar grammar) + form (`FieldRow` composed rows) + apply footer; sections `Raised`. |
| 3 | **Metrics dashboard** | MetricTile + sparkline (`ChartSeries`); density art without color-only state; `FgFaint` axes. |
| 4 | **Auth entry** | `FieldRow` (label + marker col + typed value + annotation) + identity glyph + primary chip only on submit. |

---

## 7. Execution alignment

This file is the *look-and-feel contract*. Implementation order, token wiring,
`Surface` adoption, recipe plugging, and the migration files live in the
[`component-visual-richness-plan.md`](./component-visual-richness-plan.md) waves.
Add one explicit pass:

- **Wave 0b — underline sweep (new):** remove all `Modifier::UNDERLINED` focus/
  selection/active uses per §5; replace with the grammar; one migration file
  enumerating every site and its replacement cue. Runs alongside Wave 1 (foundation
  visible everywhere) since both touch the same files.

Sequencing rationale: foundation first (palette ladder + spacing + `Surface` +
recipes) so the underline replacements have real borders/gutters/tints to land on;
then the sweep; then the per-component residuals in Waves 2–4.

---

## 8. "Done" / acceptance (designer-visible)

1. **Side-by-side:** default lookbook Panel/Dialog/List/Button/Tabs visibly layered
   vs `v0.11.0` — surface depth, breathing padding, tinted selection, chip primary,
   marker-based tabs. Obvious at a glance.
2. **Accent pixel budget:** average Accent-green area per viewport drops sharply;
   ≤2 accent-forward regions enforced.
3. **Two-marks test:** an untrained user can point to "where is focus" and "what is
   selected" as **two different marks** (bright border vs gutter+tint).
4. **Zero underline focus:** no `Modifier::UNDERLINED` encodes focus, selection, or
   active state; surviving underline is only content rendering (ANSI/markdown/diff)
   or opt-in link hover. `grep` of interaction sites clean.
5. **8-hour session:** ops/agent dashboard screenshot with no neon fatigue; calm at
   long exposure.
6. **Degrade:** mono / `NO_COLOR` / ASCII / narrow still legible (glyphs + weight +
   words carry meaning without hue).
7. **Consumer proof:** Jackin deletes its workaround list (five-slot dialog, tree
   renderer, FieldRow-equivalents, hint-bar reflow, truncation helpers) on a new pin.

---

## 9. Sources

- TermRock SoTs: [`phosphor-obsidian-visual-direction.md`](./phosphor-obsidian-visual-direction.md),
  [`component-visual-richness-plan.md`](./component-visual-richness-plan.md),
  [`terminal-design-system.md`](./terminal-design-system.md),
  [`competitive-tui-research.md`](./competitive-tui-research.md),
  [`experience-research-2026.md`](./experience-research-2026.md).
- [shadcn/ui theming](https://ui.shadcn.com/docs/theming) — radius source of truth,
  surface/foreground pairing, variant ladders.
- [Shadcn design principles (gist)](https://gist.github.com/eonist/c1103bab5245b418fe008643c08fa272) —
  minimalism, 1rem padding, hairline borders.
- [Linear DESIGN.md](https://github.com/voltagent/awesome-design-md/blob/main/design-md/linear.app/DESIGN.md),
  [Linear on DesignLang](https://www.designlang.app/gallery/linear-app) — snap-to-scale,
  quietly luxurious density, high signal-to-noise.
- [Space in Design Systems (EightShapes)](https://medium.com/eightshapes-llc/space-in-design-systems-188bcbae0d62) —
  inset/stack/grid density dials.
- [Why Does a Design Look Good? (NN/g)](https://www.nngroup.com/articles/why-does-design-look-good/) —
  typography, spacing, alignment as polish.
- [awesome-tui-design DESIGN.md](https://github.com/cola-runner/awesome-tui-design/blob/master/designs/minimal/DESIGN.md),
  [pi-zentui](https://pi.dev/packages/pi-zentui) — focus-indicator alternatives
  (bright border, accent rail, gutter, cursor presence) beyond underline.
- Grok Build (same Rust/Ratatui/crossterm stack, studied from source), Amp
  ([ampcode.com](https://ampcode.com/), [manual](https://ampcode.com/manual)),
  Jackin (Tailrocks consumer, `=0.11.0`).

*(Design-language analysis and direction only. No proprietary source reuse.)*
