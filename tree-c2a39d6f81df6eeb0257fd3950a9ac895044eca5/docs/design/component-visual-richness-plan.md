# Component Visual Richness Plan

Status: approved direction, pending implementation
Scope: `termrock` style system + widget paint layer + patterns. Companion to
`docs-site-terminal-experience-plan.md` (the docs will preview whatever this
plan ships). Implementation commits require migration files where defaults
change visibly (they will).

## 1. Problem statement and quality bar

Widgets currently read as "CLI output with borders", not as a rich terminal
application. The quality bar is set by three references:

- **Grok Build** (xAI's coding agent CLI) — open source since July 2026,
  **the exact TermRock stack** (Rust + Ratatui + crossterm), studied from
  source (clone at scratchpad `grok-build/`). Benchmark for premium agent-TUI
  presentation and motion.
- **Jackin** (Tailrocks' own TermRock consumer, pins `=0.11.0`) — its UI reads
  richer than raw TermRock defaults; every place it hand-rolls paint is a
  measured feature gap in TermRock.
- **shadcn/ui** — benchmark for the *token discipline* that makes richness
  systematic: background/foreground pairing, surface ladder, muted/accent
  state tokens, one radius source of truth. We port the discipline to
  terminal physics, not the CSS.

Three audits (2026-08-12, full reports in session research) ground everything
below in `file:line` evidence.

## 2. Diagnosis: the plainness is two policies, not 141 widgets

The audit's core finding — the cheap look originates in a handful of
load-bearing defects that propagate everywhere:

1. **The default palette has no surface ladder.**
   `RolePalette::tailrocks_phosphor()` ships `Canvas`, `Surface`, `Elevated`,
   `Backdrop` as **empty styles** (`style/mod.rs:315-319`); `Role::StatusBar`
   is empty too (`:357`). The fill path deliberately drops empty styles
   (`widgets/surface.rs:414-420,440`), so every panel/card/dialog/toast/menu
   fill in the library is a runtime no-op. A test **pins the emptiness in
   place** (`widgets/surface.rs:597-604`). The design SoT
   (`phosphor-obsidian-visual-direction.md:119-155`) already specifies the
   correct values — the palette never adopted them.
2. **Spacing tokens are dead code.** `SpacingScale` (`style/tokens.rs:143-169`)
   and `Density::padding_x/y/gap` are consumed by 2 of 141 widget files.
   `Panel` hard-codes `.padding(0, 0)` (`widgets/panel.rs:862`); `Stack`/`Grid`
   default to gap 0 and their density-aware constructors are called only from
   tests. Body text sits flush against border glyphs everywhere.
3. **The recipe system is built but unplugged.** `button_recipe`,
   `input_recipe`, `list_row_recipe`, `elevation()` have **zero** widget
   callers; `PanelRecipe.surface/pad_x/pad_y` have zero readers
   (`style/tokens.rs:572-778`).
4. **`style.bg = None` is copied into 13 widgets** (`primitives.rs:694` et
   al.), which makes the doc-specified "primary action = solid accent chip"
   unrenderable and leaves primary buttons painting near-invisible
   black-on-dark.
5. **Overlays have no elevation.** Dialogs `Clear` to terminal default and
   bypass `Surface` (`dialog.rs:1258,1292-1294`); the backdrop never dims
   (`Role::Backdrop` has zero widget readers; `dim_wash()` is test-only).
6. **Accent meaning collapse.** `BorderFocused`, `Focus`, `Accent`, `Success`,
   `HintText`, `ScrollThumb`, `TabUnderlineFocused`, `ChartSeries1` are all
   the identical green (`style/mod.rs:324-367`). Selection tint is mis-wired
   to fg-only roles so `SelectionChrome::Tint` can never produce a wash
   (`tokens.rs:766-767`).
7. **Chrome is hand-rolled.** ~137 of 141 widgets draw their own box glyphs
   instead of going through `Surface`, so no fill/pad/elevation policy can
   reach them.

Consequence for the fix strategy: **fix the tokens and the shared chrome path
first; most widgets then inherit richness instead of being restyled one by
one.**

## 3. What the references actually do (evidence to absorb)

### 3.1 Grok Build (same stack, from source)

- **Neutral ground + jewel accents:** near-black gray ramp
  (`#0a0a0a → #141414 → #1c1c1c → #242424 → #2c2c2c → #363636` for
  terminal/main/code/highlight/hover/select) so small color moments read
  expensive; ramp survives 256/16-color quantization.
- **Semantic accent-per-actor:** every scrollback block gets a 1-col colored
  accent rail `┃` (user gray, assistant magenta, plan golden `#FFDB8D`,
  error red, success green…) instead of a box. Scannable, cheap, animatable.
- **Width-invariant glyph catalog** with per-glyph legacy fallback, every
  glyph tested to exactly 1 col: `❯ ◆◇◈ ●○◉◎ ✓✗ ›‹▸▾ ⧉ ↗`, braille spinners
  `⠋⠙⠹…`, quiet dot-pulse `⋅ : ⸬ ⁙` for background tasks.
- **Motion = status, never decoration:** sin² *wave* flowing down the accent
  rail of active blocks, sin² *pulse* for single icons, logo shimmer via
  raised-cosine band, all capped 30fps with span-run merging.
- **Rounded borders uniformly** (`BorderType::Rounded`, 13 call sites);
  focus via border *brightness*, same law as TermRock's.
- **Quantize-at-startup pipeline** for all colors incl. runtime-generated;
  truecolor-only themes hidden from the picker on lesser terminals.
- Rich block styling: diamond-bullet tool rows `◆ Verb (dim details)`,
  tinted diff line backgrounds (`#420e14`/`#063806` chosen to quantize to
  red/green), thinking text blended 70% toward bg, sticky headers,
  label-in-border prompt boxes (`╰─ dispatch ──╯`).

### 3.2 Jackin (consumer workarounds = feature requests)

Jackin's richness over raw TermRock is **not color** (it reuses the phosphor
palette) — it is:

- **Row-level composed typography:** every row is 3–5 styled segments
  (cursor gutter `▸ `, source-marker column, label padded to a fixed column
  width, typed value — plain/masked/breadcrumb/danger — then an italic or dim
  annotation). TermRock's `ListRow` composed slots exist but Jackin passes
  fully pre-styled `Line`s into all 14 call sites — the composed anatomy
  doesn't produce the look it wants.
- **Spacing policy as chrome:** mandatory blank spacer rows painted as
  intentional bands (not gaps), five-slot dialog rhythm (spacer/body/spacer/
  actions/spacer) rebuilt product-side after TermRock dropped it,
  content-sized panel stacks (`(rows+2).min(cap)` per block, blocks omitted
  when empty), dialogs sized against a **virtual 160-col reference terminal**
  so they hold absolute width across resizes.
- **Missing semantic roles** it had to invent: `ACTION_ACCENT` (mint, every
  "+ Add …" constructive row), `DISCLOSURE_ACCENT` (amber, every group
  header), `CYAN`/`CYAN_DIM` live-tier pair.
- **Per-character animation primitives** it had to build: cell-run coalescing,
  smoothstep edge-fade vignette, sweeping brightness ripple, alpha fade-in on
  chrome, 6-stop age ramp for the launch rain.
- **Measured, wrapping hint bar** with a 3-pass layout convergence loop.
- Notable brand law conflict with Grok Build: Jackin mandates **square caps
  only** ("terminals cannot round corners, so neither does the mark") while
  Grok Build rounds everything → border shape must be a **theme token**, not
  a hardcoded law (§4.3).

### 3.3 shadcn/ui (the discipline)

Semantic pairs (`background/foreground`, `card`, `popover`, `primary`,
`secondary`, `muted`, `accent`, `destructive`, `border`, `input`, `ring`,
`radius` scale, `chart-1..5`, sidebar variants). Two principles to port:
**every surface token pairs with a foreground token**, and **one source of
truth scales the whole system** (radius multipliers ≈ our density/spacing
scale). Dark mode = token override ≈ our `RolePalette` swap.

## 4. Token system upgrade (the foundation fix)

### 4.1 Fill the surface ladder (P0)

Adopt the values the design doc already specifies, as the default
`tailrocks_phosphor` palette:

| Role | Value | shadcn analog |
|---|---|---|
| `Canvas` | `#0a0c0a` | `background` |
| `Surface` | `#121612` | `card` |
| `Raised` | `#1a1f1a` | — (hover/section) |
| `Elevated` | `#1e2620` | `popover` |
| `Sunken` | `#0d100d` | `input`/well |
| `Backdrop` | dim wash toward Canvas | overlay scrim |
| `StatusBar` | filled band | — |

Pair each with a foreground (shadcn pairing convention): `Fg`, `FgStrong`
(+bold), `FgMuted`, `FgFaint` ladder wired so every surface knows its text
colors. Delete the empty-surface pinning test (`surface.rs:597-604`) and
replace with tests asserting the ladder is populated and contrast-safe.
Honor Jackin's "terminal-default background" need as an explicit palette
variant (`RolePalette::terminal_native()` using `Color::Reset` surfaces), not
by keeping the default empty.

### 4.2 De-collapse accents; add missing semantic roles

- Give `BorderFocused`, `Focus`, `Accent`, `Success`, `HintText`,
  `ScrollThumb`, `TabUnderlineFocused`, `ChartSeries1` distinct values.
- Fix selection: dedicated `SelectionTint` bg role (`#14331a`) +
  `HoverTint` (`#1a221c`); rewire `ListRowRecipe.tint/hover` to bg-carrying
  roles so `SelectionChrome::Tint` actually washes rows.
- New roles (evidence: Jackin inventions, Grok Build actor system):
  - `ActionConstructive` (creation sentinels), `DisclosureHeader`
    (group headers), `InfoStrong`/`InfoDim` (live-tier pair).
  - **Actor accent slots** for agent surfaces (user/assistant/thinking/tool/
    plan/system/error/success) consumed by transcript, tool-call cards,
    plan review, subagent cards — TermRock's agent widgets are its flagship
    use case and Grok Build proves per-actor accents are the premium pattern.
- Keep the quantize-at-startup pipeline as the enforcement point — it finally
  has real colors to quantize; extend the lookbook `ColorCapability` knob
  proofs to the new roles.

### 4.3 Border shape becomes a theme token

`BorderShape { Square, Rounded }` on the theme (glyph-set aware; ASCII maps
both to `+`). Phosphor default stays **Square** (Jackin brand law, current
identity); Grok-Build-class consumers select Rounded. Focus stays
color/brightness only — the focus-visible law is unchanged.

### 4.4 Spacing becomes real

- `Panel` default padding from the anatomy spec: inset `(2, 1)` at
  Comfortable density (kill `panel.rs:862` zeroing; add pad term to
  `Panel::layout`).
- `Stack`/`Grid` defaults derive from `DesignSystem.density` (gap 1 at
  Comfortable); widget-local density enums (`ListDensity`, `EmptyDensity`,
  `KvDensity`, `DataDensity`) collapse into the single `Density` model.
- Dialog interior returns to the five-slot rhythm (leading spacer / body /
  mid spacer / actions / trailing spacer) as `Dialog` behavior — Jackin
  rebuilt exactly this product-side.
- `DialogSpec` gains reference-width policy (percent of a virtual
  `REFERENCE_COLS = 160` terminal, clamped) so dialogs hold stable width.

## 5. Chrome unification (make richness reach all widgets)

- **All container widgets route chrome through `Surface`** (fill + border +
  padding + elevation in one authority). Eliminate hand-drawn `┌`-literals in
  the ~20+ files that carry them; the remaining ~137 widgets inherit fills
  and padding the moment they adopt it.
- **Recipes become mandatory:** `Button`, inputs, list rows, panels resolve
  through their recipes; delete the 13-site `style.bg = None` pattern.
  Primary button = solid accent chip (ink on phosphor), Secondary = outlined,
  Ghost = text-only — the shadcn button variant ladder in terminal form.
- **Overlay elevation:** dialogs/popovers/menus/toasts paint `Elevated` fill,
  backdrop dims by default (`dim_wash` becomes the production path), toasts
  get muted borders with severity carried by icon + accent rail (per the
  design doc, not whole-frame severity color).
- **StatusBar gets its bar** (filled band role), slot separators, and zone
  chrome.

## 6. New primitives (absorb what consumers had to build)

| Primitive | Source of evidence |
|---|---|
| `FieldRow` — cursor gutter + marker column + fixed-width label + typed value (plain/masked/breadcrumb/danger-unset) + annotation | Jackin auth/editor rows |
| `AccentRail` block chrome — 1-col semantic rail, animatable (wave while active) | Grok Build scrollback blocks |
| `TreeList` — disclosure column, per-row tone tiers, hover fill, fixed-prefix h-scroll | Jackin workspace tree "structural exception" |
| `PanelStack` layout — per-block `(content_rows, min, max, visible)`, omit-when-empty | Jackin sidebar layout |
| `HintBar` v2 — wrapped, `measured_height(width)`, built-in leading spacer option | Jackin 3-pass footer reflow |
| Text-effect helpers — `coalesce_cells`, edge-fade vignette (smoothstep), brightness sweep/ripple, generalized `.alpha()` fade on widgets | Jackin progress rail/header; Grok Build shimmer |
| Motion kit — sin² `wave_brightness`/`pulse_brightness`, 30fps cap, `Motion` reduction mapping | Grok Build animation loop |
| Glyph catalog v2 — width-tested catalog with per-glyph ASCII/CP437 fallback; diamond family `◆◇◈`, status dots `●○◉◎`, dot-pulse spinner tier; sub-cell ramps ` ▁▂▃▄▅▆▇█` promoted from charts into `Progress`/meters | Grok Build `glyphs.rs`; audit finding #11 |
| `text::truncate_cols` — display-column-correct `…` truncation | 3 independent Jackin implementations |
| `DetailTable::measure`, dialog size registry pattern documented | Jackin operator info / modal_rects |

Boundary check (building-block law): all of the above are product-neutral
primitives → `widgets`/`style`/`layout`. Brand pill, rain, wordmark stay
product-side; a neutral `BrandPill` geometry helper is optional later.

## 7. shadcn → TUI porting matrix (design shorthand)

| shadcn affordance | Terminal translation |
|---|---|
| `background`/`card`/`popover` surface ladder | Canvas/Surface/Raised/Elevated fills (§4.1) |
| `*-foreground` pairing | per-surface fg ladder Fg/FgStrong/FgMuted/FgFaint |
| `muted` | FgMuted text + Sunken/Raised quiet surfaces |
| `accent` (hover/active) | `HoverTint`/`SelectionTint` row washes |
| `ring` (focus) | `BorderFocused` brightness, never weight |
| `radius` | `BorderShape` theme token (§4.3) |
| shadow/elevation | surface fill delta + backdrop dim (no fake shadows) |
| `destructive` | Danger role on icon/rail/action chip, muted frame |
| `input` surface | Sunken fill + placeholder ghost + cursor cell |
| button variants (default/secondary/outline/ghost) | chip/outlined/text button recipe ladder (§5) |
| `chart-1..5` | distinct ChartSeries roles (de-collapsed) |
| skeleton/spinner states | Skeleton shade ramp + braille/dot-pulse spinner tiers |

## 8. Widget upgrade pass (order)

1. **Wave 1 — foundation visible everywhere:** palette ladder + spacing +
   `Surface` adoption in `Panel`, `Dialog`, `Card`, `Toast`, `StatusBar`,
   `List` (tint selection), `Button` (chip), `Badge` (real chips),
   `Progress` (sub-cell ramp + track bg), `EmptyState` (framed surface, not
   loose text on void).
2. **Wave 2 — agent surfaces (flagship):** `Transcript`, `ToolCallCard`,
   `SubagentCard`, `PlanReview`, `PromptComposer`, `Permission`,
   `TerminalRunCard` adopt actor accents, `AccentRail`, collapsed/expanded
   states, presence motion.
3. **Wave 3 — data + overlays:** `DataTable`, `Tree`/`TreeList`, menus,
   `CommandPalette`, pickers, `Diff` (tinted line backgrounds tuned to
   quantize), `Form`/`FieldRow`.
4. **Wave 4 — cascade sweep:** remaining widgets + patterns; consistency
   audit against the contract matrix; lookbook stories updated per widget in
   the same commit (cross-surface consistency law).

Each wave: lookbook before/after side-by-side is the review artifact;
contrast checks + `NO_COLOR`/ASCII/256-color proofs via existing capability
knobs; migration file per breaking default change (filled surfaces, new
padding, button chips are all visible breaks — numbered `migrations/` +
`MIGRATING.md` entries required).

## 9. Consumer validation loop

- **Jackin is the acceptance test:** the 14 documented workarounds become the
  checklist — success means Jackin deletes product-side code (five-slot
  dialog shim, tree renderer, FieldRow-equivalents, hint-bar reflow,
  truncation helpers, spinner-frame copy) and adopts TermRock primitives on a
  new pin.
- **Grok Build parity spot-checks:** recreate 3 signature Grok Build surfaces
  (tool-call block with rail + wave, plan approval with golden accent, turn
  status line) as lookbook stories using only public TermRock APIs — if a
  recipe needs product-side paint hacks, the library is still missing a
  primitive.
- Docs site (companion plan) previews every upgraded widget live; the states
  row + knobs expose the new ladder (surface fills, density, border shape,
  motion) directly.

## 10. Success criteria

1. Default lookbook render of Panel/Dialog/List/Button is visually layered:
   surface fills, breathing padding, tinted selection, chip primary action —
   side-by-side against v0.11.0 the difference is obvious at a glance.
2. Zero widgets carry `style.bg = None`; zero widgets hand-draw box glyphs
   outside `Surface`; recipes have nonzero call sites everywhere they exist.
3. Dialogs dim the backdrop and sit on elevated fill; status bar is a band.
4. Agent surfaces express actor accents + presence motion; reduced-motion
   collapses to static accents cleanly.
5. Jackin migrates forward and deletes its workaround list; phosphor identity
   (green-on-obsidian, square caps, single-line borders, focus-by-color) is
   preserved throughout.
