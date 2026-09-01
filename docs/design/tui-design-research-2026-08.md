# TUI design research 2026 — reference languages, design law v2, component improvement plan

**Status:** binding shared design laws; historical component matrix
**Audience:** design, implementers
**Builds on:** `phosphor-obsidian-visual-direction.md`, `terminal-design-system.md`,
`component-visual-richness-plan.md`, `product-audit.md`
**Supersedes:** every rule in earlier docs that prescribes **underline as a focus,
selection, or tab cue** (see §5.3 — underline is retired from the state vocabulary)
**Scope:** every public surface inherits the shared laws. The §6 matrix is a
historical 48-surface implementation sample, not component inventory; the Rust
inventory and generated docs catalog own membership and counts.
**Companion SoTs:** [`tui-app-deep-analysis.md`](./tui-app-deep-analysis.md)
(per-app cell-level extraction from 15 reference apps),
[`tui-motion-system.md`](./tui-motion-system.md) (transitions/effects/pipeline),
[`tui-design-specs-2026-08.md`](./tui-design-specs-2026-08.md) (DESIGN.md specs,
Monospace standard, Clack/Fresh/Superfile/FrankenTUI),
[`tui-theme-gallery.md`](./tui-theme-gallery.md) (color scheme gallery, palette→role
mapping, quantization).

---

## 1. Method and sources

Research ran along three axes in August 2026:

1. **Reference products** — Grok Build (same Ratatui/crossterm stack, studied from
   source), Amp CLI (chronicle, manual, community reviews), Jackin (consumer built on
   TermRock; its workarounds are our missing features).
2. **Component-library ecosystems** — Charm stack (bubbletea/lipgloss/bubbles/huh),
   Textual (Python), ink + ink-ui + kud/ink-ui (React), OpenTUI (Zig/TS), the Ratatui
   third-party ecosystem (throbber-widgets, tui-textarea, tachyonfx, …), shadcn/ui
   itself for the discipline being ported.
3. **Best-designed terminal apps** — Crush, opencode, lazygit, k9s, helix, zellij,
   fzf, posting, harlequin — what their chrome actually does, with theme/skin files
   as evidence.

This file keeps only what is **decision-relevant** for TermRock. Prior deep dives
(`component-visual-richness-plan.md` §3, `experience-research-2026.md`,
`competitive-tui-research.md`) hold the full evidence.

---

## 2. What the references do (condensed)

### 2.1 Grok Build (Ratatui, from source)

- **Neutral graphite ramp, jewel accents.** Six-step near-black ramp
  (`#0a0a0a → #363636`) makes every small color moment read expensive; the ramp
  survives 256/16-color quantization.
- **Accent rail per scrollback block** (`┃`, 1 col, colored per actor: user gray,
  assistant magenta, plan golden, error red) instead of boxes. Scannable, cheap,
  animatable.
- **Width-invariant glyph catalog** with per-glyph fallback, every glyph tested to
  exactly 1 column: `❯ ◆◇◈ ●○◉◎ ✓✗ ›‹ ▸▾ ⧉ ↗`, braille spinners, quiet dot-pulse
  for background tasks.
- **Motion = status, never decoration.** sin² wave down active rails, sin² pulse for
  single icons, logo shimmer — capped 30 fps, span-run merged.
- **Rounded borders uniformly; focus = border brightness.** Same law as TermRock's
  single-line border rule, but executed with a consistent rounded glyph set.
- **Quantize-at-startup** for all colors including runtime-generated; truecolor-only
  themes hidden from the picker on lesser terminals.

### 2.2 Amp CLI

- Rebuilt as "a proper terminal user interface" (Sept 2025, *Look Ma, No Flicker*):
  full-screen flicker-free rendering is table stakes, not a feature.
- **Command palette replaced slash commands** (Oct 2025) — palette-first interaction
  model; TermRock's `CommandPalette`/`QuickOpen` must be the primary command surface,
  not an extra.
- Inline diff review + staging inside the TUI (June 2026) — diffs are a first-class
  review surface with actions, not a pager dump.
- `@` fuzzy file mentions, message queueing, edit/restore/fork of prior messages —
  composer-level richness consumers expect from an agent surface.

### 2.3 Jackin (TermRock consumer — its workarounds are our backlog)

- **Row-level composed typography**: every row is 3–5 styled segments (cursor gutter,
  marker column, fixed-width label, typed value, dim annotation). Our composed row
  anatomy exists but doesn't produce this look out of the box.
- **Spacing as chrome**: intentional blank spacer bands, five-slot dialog rhythm,
  content-sized panel stacks, dialogs sized against a virtual 160-col reference.
- **Missing semantic roles it invented**: constructive-action accent (mint),
  disclosure-header accent (amber), live-tier cyan pair. (Now adopted as
  `ActionConstructive`, `DisclosureHeader`, `InfoStrong/InfoDim`.)
- **Per-character animation primitives**: cell-run coalescing, brightness ripple,
  alpha fade, age ramps.
- **Brand-law conflict**: Jackin mandates square caps; Grok Build rounds everything.
  → **Border shape is a theme token, never a library law.**

### 2.4 Charm stack (lipgloss v2, bubbles, huh)

- Styles as **immutable value objects** with CSS-like spacing shorthand
  (`Padding(1,4)`), `GetFrameSize()` layout math, border glyph sets as data with
  per-side overrides.
- **LightDark closure**: OSC 11 background detection → `lightDark(light, dark)`
  pairs; v2 removed ambient adaptation — constructors take `isDark` explicitly.
- bubbles `list`: fuzzy filter, pagination, help, status, spinner, and an
  **ItemDelegate seam** — the composition hole that keeps the core simple.
- huh: `Form → Group → Field`, inline validation, 5 themes, and an
  **accessible mode** that degrades the whole form to plain stdin prompts for screen
  readers. Steal this degradation path.

### 2.5 Textual (the most complete terminal design system)

- **11 base tokens** (`primary, secondary, foreground, background, surface, panel,
  boost, warning, error, success, accent`); everything else is **derived**:
  `-lighten-1..3`, `-darken-1..3`, `-muted` (70% blend to background),
  `text-<role>` re-tinted per surface. A theme is a partial override over a
  generator, not a 60-row table.
- **Component variables** layered on top: `border` / `border-blurred`,
  `block-cursor-{fg,bg}` + blurred variants, `input-selection-background`,
  scrollbar triple, `link-*`, `button-focus-text-style` — each overridable, each
  defaulting to a derivation.
- **Focused vs blurred is a token pair**, not a boolean in widget code.
- Pseudo-classes as first-class state: `:hover :focus :blur :focus-within
  :disabled :dark` — `:focus-within` = container reflects descendant focus.
- 37 widgets incl. `MaskedInput`, `OptionList` vs `SelectionList` (single-choice vs
  multi-check as separate primitives), `Rule`, `Digits`, Toast, CommandPalette with
  runtime theme switching.
- Apps proving the ceiling: posting (API client), harlequin (SQL IDE).

### 2.6 OpenTUI + ink ecosystem

- OpenTUI: Zig core, Yoga flexbox, tree-sitter Code/Diff/Markdown as **library
  primitives**, dedicated `@opentui/keymap` command engine, SSH-served sessions.
  Powers opencode in production.
- ink-ui official: per-component style slots overridden through theme context — the
  closest JS analog to our per-widget recipes.
- kud/ink-ui design claims worth adopting verbatim: **"state is signalled by shape,
  case, and glyph, never colour alone"**; spacing scale as data; behavior hooks
  (`useTabs`, `useListCursor`) split from presentational components.

### 2.7 App chrome evidence

| App | Steal-worthy pattern |
|-----|----------------------|
| **Crush** | Theme = role **ramps**: `fgBase→fgMostSubtle`, `bgBase→bgMostVisible`, per-status 3-step subtlety ladder (`info/infoMoreSubtle/infoMostSubtle`); all 16 ANSI slots remapped to theme so child-process output matches; zero hex literals in theme logic |
| **lazygit** | Selected line = **reverse video** — palette-free emphasis surviving every theme; users run it borderless; panel title bars double as key-hint surfaces |
| **k9s** | Skin taxonomy: `frame.border.{fg,focusColor}`, `frame.crumbs.*`, `frame.title.{highlight,counter,filter}` — **title bar as status surface** (counts, active filter, sort indicator inline); `status.{new,modify,add,error,kill,completed}` resource-state roles |
| **helix** | Flat scope map over a named `[palette]`, `inherits = "base"` delta themes, mode-specific cursor scopes, `ui.window` split borders |
| **zellij** | Whole theme = **10 colors**; chrome maps roles onto them. Small fixed palette + role mapping beats 100 named slots |
| **fzf** | State grammar for pickers: `pointer` (cursor glyph), `marker` (multi-select glyph), `hl/current-hl/selected-hl` (match highlighting **separate from selection**); per-region border colors; `--style full|default|minimal` density presets; `bw` no-color scheme shipped as a first-class scheme |

### 2.8 shadcn/ui (the discipline being ported)

- Two layers: **headless behavior primitives** (focus, keyboard, semantics) +
  **style layer** (tokens + variants). TermRock's kernel is the headless layer;
  recipes are the style layer.
- **CVA variants**: one typed variant axis set per component (`default / secondary /
  destructive / outline / ghost`), defaults in the theme, extendable without touching
  call sites.
- **`cn()` semantics**: consumer styles override defaults by conflict resolution,
  not concatenation → ratatui `Style::patch` with later-wins per field.
- **Compound components / named parts**: every anatomy part separately addressable
  in the theme (Textual component variables and k9s skins already prove the terminal
  version).
- Every surface token **pairs with a foreground token**; dark mode = token override.

---

## 3. Why most TUIs look cheap (synthesis)

Cross-cutting failure tells, ranked by visual damage:

1. **Border soup** — every panel double-bordered regardless of focus; four bright
   borders in one viewport; no borderless rest state.
2. **Color soup** — raw ANSI16 everywhere, no muted ramp, accent = selection =
   focus = success = brand collapsed into one hue.
3. **Neon selection slabs** — full-row inverted/saturated fills as the only selected
   state; unreadable for long sessions.
4. **No surface ladder** — everything floats on void black; dialogs don't sit on an
   elevated field; inputs aren't recessed.
5. **No spacing discipline** — text flush against border glyphs; gaps accidental.
6. **Invisible interactivity** — no affordances; focus expressed only by color, or
   by a blanket underline that makes every focused region look like a hyperlink farm.
7. **Typography anarchy** — no muted/strong/faint ladder; metadata as loud as
   primary content; ALL-CAPS and emoji used as structure.
8. **Glyph roulette** — nerd-font icons breaking on machines without the font, no
   ASCII fallback, wide-glyph misalignment.
9. **Decorative motion** — animation eating function (btop-style fades); spinners
   that don't name the verb.
10. **Unstyled edges** — empty states, errors, loading, and monochrome/no-color
    modes left as afterthoughts.

---

## 4. State of TermRock (HEAD, August 2026)

Done since the first audit:

- Surface ladder **filled** in `RolePalette::tailrocks_phosphor`
  (Canvas `#0a0c0a` … Sunken) — commit `c91d71d5`.
- Success split from brand accent (`SUCCESS_GREEN_RGB` mint vs phosphor).
- Constructive / disclosure / live-tier / actor roles adopted.
- `SelectionChrome::Gutter` available; `SelectionTint`/`HoverTint` washes exist.

Still broken or regressive:

1. **`Role::Selection` is still a neon slab** — `bg(PHOSPHOR_GREEN).fg(INK)`
   (`style/mod.rs:376`), and `tailrocks_phosphor()` still resolves
   `SelectionChrome::Tint` while lookbook rows still paint fill.
2. **Accent meaning collapse persists**: `BorderFocused`, `Focus`, `Accent` are the
   same green; `Focus` has no independent resolution.
3. **Underline is the focus/selection workhorse**: `show_focus_underline` fires on
   focused+selected rows (`tokens.rs:816`), tab strips lean on
   `TabUnderline{Focused,Unfocused}`, several recipes paint
   `Style::…underlined()` for accents (`style/mod.rs:501,592,741-742`). This is the
   "mostly underline" look — busy, hyperlink-noisy, and it breaks on terminals with
   curly/dotted underline rendering. **Retired by §5.3.**
4. **63 roles, no derivation ladder** — every muted/lighten variant is hand-tuned;
   theme authoring means filling a 63-slot array. Textual/Crush prove 11–16 base
   tokens + derivation is strictly better.
5. **Spacing tokens still barely consumed**; panel padding still hard-coded.
6. **Backdrop never dims**; dialogs clear to terminal default instead of sitting on
   Elevated over a dimmed scene.

---

## 5. TermRock design law v2 (binding)

These rules replace scattered per-doc rules. Where older docs conflict, this file
wins. `phosphor-obsidian-visual-direction.md` remains the palette SoT; this file is
the **interaction-state and styling-system** SoT.

### 5.1 Token model — derive, don't enumerate

Adopt the Textual/Crush model on top of the existing `Role` array:

1. **~16 base tokens** authored per theme: `canvas, surface, raised, elevated,
   sunken, backdrop, fg, fg-strong, fg-muted, fg-faint, border, border-focused,
   accent, success, warning, danger, info` (+ `accent-strong` for solid fills).
2. **Derivation ladder**: every surface/fg token gets `-muted` (blend 60–70% toward
   canvas), `-subtle`, `-strong` variants computed, not authored. Selection tint,
   hover tint, disabled fg are derivations, never separate hexes.
3. **Every surface pairs with a foreground** (`elevated` ↔ `on-elevated`); recipes
   never paint text on a surface without resolving its paired fg.
4. **Component variables** derive from bases but stay individually overridable:
   `scrollbar-{track,thumb}`, `input-{bg,cursor,selection}`, `tab-{active,inactive}`,
   `link{,-hover}`, `chart-1..5`, `syntax-*` — exactly today's tail roles,
   re-homed as derivations.
5. Theme authoring = partial override over the generator (helix `inherits` model),
   not a 63-row array literal. The `Role` array stays as the compiled runtime
   representation; the **authoring** surface shrinks.

### 5.2 Hierarchy without font size

Four emphasis levels, always available, always non-color:

| Level | Paint | Use |
|-------|-------|-----|
| Strong | `fg-strong` + **bold** | titles, selected primary label, active tab |
| Body | `fg` | primary content |
| Muted | `fg-muted` | secondary, descriptions, unfocused chrome labels |
| Faint | `fg-faint` | meta: timestamps, shortcuts, hints, counters |

Rules: max one bold run per row; metadata is never body-white; headers are
bold-only (no fills, no caps abuse).

### 5.3 Focus & selection model — underline retired

**Underline is removed from the state vocabulary.** It survives in exactly one
place: hyperlinks, and there only on hover (idle link = `link` fg, hover = fg +
underline). Everywhere else it is replaced:

| State | v2 paint (was) |
|-------|----------------|
| Row selection, list unfocused | gutter `▌` accent + primary strong; optional `selection-tint` wash (was: neon fill) |
| Row selection + focus | gutter `▌` + primary **bold/strong** + tint (was: gutter + **underline**) |
| Container focus | single-line `border-focused` (unchanged — the one border law stands) |
| Active tab | **connected tab**: active tab painted on `surface` (same fill as its content, open bottom edge), `fg-strong` bold; inactive tabs muted on canvas (was: accent underline bar) |
| Input focus | `sunken` well + visible block cursor + optional accent left caret `▎` at cursor column; bordered inputs recolor the border (was: underline) |
| Cursor cell (grids/tables) | 1-cell reverse video (lazygit's palette-free trick) + row gutter (was: underline) |
| Current match in fuzzy results | `current-hl` = **bold + accent fg on matched spans**; selection is gutter only (fzf's pointer/marker/hl separation) |

Rationale: underline reads as "link" to every modern user; on rows and tabs it
produces the noisy "underlined everything" look; several terminals render
`UNDERLINED` with inconsistent style (curly/dotted/colored) which we cannot
control. Gutter + weight + tint + reverse are all renderer-stable.

Non-color channel is preserved: gutter glyph, bold, reverse video, and case all
survive monochrome. `NO_COLOR`/mono projection: selection = reverse row, focus
border = the same single-line border (already non-color).

### 5.4 Elevation & layering without shadows

1. Scene base = `canvas`. Content = `surface`. Cards/nested = `raised`.
2. Overlays (dialog, palette, popover, toast) paint on `elevated` **and the
   backdrop dims**: every cell beneath gets blended 60% toward canvas
   (`dim_wash` graduates from test-only to the overlay stack's default).
3. Max depth: 3 layers (canvas → surface → elevated). No fourth tier; nesting
   beyond that re-uses `raised`.
4. One bright border per scene layer — ownership, not decoration.

### 5.5 Spacing & density

- All insets come from `SpaceScale` (cells): panel inset `(x:2, y:1)` comfortable,
   `(1,0)` compact; row inset `(1,0)`; dialog inset `(2,1)`; section gap 1 row.
- Text never touches a border glyph: minimum 1-cell inset on bordered chrome.
- Density is a product mode (comfortable/compact/dashboard) resolved through tokens,
   not per-widget booleans. Dashboard = zero vertical padding, gutters collapse to
   1 cell, hints inline.
- Blank spacer rows are **intentional bands** (Jackin), painted by layout, never
   accidental emptiness.

### 5.6 Color discipline

- ≤ 5 hues per theme besides the gray ramp: accent + 4 status. Status hues are
   muted-leaning (danger `#ff5e7a`, warning `#f0c040`, info `#5ec8ff`, success
   mint) — never the brand accent.
- Accent budget: ≤ 2 accent-forward regions per viewport (focused border + one
   live element). Everything else whispers via muted/fg ladder.
- ANSI-16 projection remaps the 16 terminal slots to theme values (Crush) so child
   output matches; monochrome keeps glyph/bold/reverse channels.
- `NO_COLOR`, `TERM=dumb`, and color-blind safety: state is signalled by shape,
   case, and glyph — never color alone (kud/ink-ui rule, adopted verbatim).

### 5.7 Glyphs & motion

- Glyph catalog stays the SoT; every glyph has an ASCII fallback and a measured
  1-column width test. Nerd-font-only glyphs are forbidden in library defaults.
- Spinners: braille family default, 10–12 fps, always paired with a **verb**
  (`⠹ applying patch`, never a bare spinner). Background tasks use quiet dot-pulse.
- Motion is status, capped, and respects reduced-motion: transitions are fades of
  ≤ 150 ms or nothing. No decorative loops (clig.dev anti-vibes rule).
- tachyonfx-class effects live behind the motion token as post-render garnish —
  never load-bearing.

### 5.8 Interaction grammar (adopt fzf + Textual)

- Picker state slots are separate: `pointer` (cursor row), `marker` (multi-select
  `☑`), `match` (fuzzy highlight spans), `selection` (committed selection). One
  glyph per job.
- Pseudo-state model for every widget: `hover / focus / blur / focus-within /
  disabled / error` resolved by recipes, not ad-hoc per-widget styles.
- Behavior/render split: state machines (cursor, tabs, spinner tick) external;
  widgets pure-render. One uniform component contract across the crate.
- Density/style presets: `full | default | minimal` chrome presets on collections
  (fzf `--style`), orthogonal to density.

---

## 6. Historical component improvement sample

Legend: **P0** = blocks premium feel, **P1** = cohesion, **P2** = polish.
"Chrome" = border/padding/surface; "state" = focus/selection/hover paint.
All items inherit §5 fixes automatically; below are the component-specific deltas.

### Onboarding & app surfaces

| Component | P | Improvements |
|-----------|---|--------------|
| Setup wizard | P1 | Step rail left (Stepper vertical, muted) + content right; one accent region (primary action); completed steps `✓` muted, current step strong + gutter, never all-green; final screen = summary `KeyValueTable`, not a wall of fields |
| Settings screen | P1 | Two-column: section `Sidebar` + form pane; dirty fields get `▌` constructive gutter + `●` dot, not color-only; section search via `SearchInput` filtering groups; every control row uses composed-row anatomy (label / control / hint faint) |
| Metrics dashboard | P1 | Tiles on `raised` with paired fg; one accent budget across the whole board — sparkline/histogram use `chart-*` ramp, not accent green; thresholds shown as glyph + muted label (`▲ 12% over`), no red walls; dashboard density preset |
| Auth Entry | P1 | Centered `elevated` card over dimmed backdrop; identity row = actor accent; `PasswordInput` sunken well; primary action = the only solid accent chip; error = danger fg + `!` + scope line ("3 attempts left"), never border-only red |

### Collections

| Component | P | Improvements |
|-----------|---|--------------|
| List | P0 | Kill neon fill default → gutter `▌` + strong + tint (§5.3); multi-select marker `☑` distinct from pointer; secondary/meta auto-muted via composed row; hover = `hover-tint` only |
| VirtualList | P0 | Same state paint as List (one recipe); scrollbar thumb/track derivations; overscan rows never flash unstyled; empty/loading rows use `Skeleton`, not blank space |
| VirtualGrid | P1 | Cursor cell = 1-cell reverse + row gutter; range selection = tint only; headers muted with sort glyph; column separator = 1-space, not `│` soup |
| TreeTable | P1 | Tree guides `│ ╰ ├` in `fg-faint`, disclosure chevrons `▸▾` muted, accent only on the focused node's gutter; expand/collapse never re-paints the whole row in accent |
| DataTable/Table (peer) | P1 | Header row bold-once or muted; sorted column gets `↑` glyph + strong header, not a fill; numeric columns right-aligned + muted; status column = glyph + letter (`R/W/D`) for mono |
| Picker | P0 | Adopt fzf grammar: pointer/marker/match as separate slots; match spans bold+accent, selection gutter-only; footer = count + active filter inline (k9s title-bar pattern) |
| QuickOpen | P0 | Palette on `elevated` over dimmed backdrop; query in sunken input; results = List recipe; section headers muted sticky; empty state styled ("No matching files · ↵ to search everywhere") |

### Navigation & chrome

| Component | P | Improvements |
|-----------|---|--------------|
| Tabs | P0 | **Connected-tab model** (§5.3): active = surface fill + bold fg-strong, open bottom edge into content; inactive = muted on canvas; `TabUnderline*` roles retired; overflow = `‹ 3/12 ›` pager, not wrap |
| MenuBar | P1 | Flat row on canvas, items muted, open menu = `elevated` popover with gutter selection; active item = reverse or surface fill, never underline; mnemonics shown as faint trailing letter, bolded on Alt |
| Toolbar | P1 | Ghost icon buttons (label + glyph, muted), focused = strong + gutter `▎`, primary action alone may be solid accent chip; separators = 1-cell space or faint `│`, no boxes per button |
| Sidebar | P1 | Section headers muted + bold, items body, active item gutter + strong + tint (List recipe); badges right-aligned faint; collapse = icon rail with tooltips; single left border `│` quiet, not full box |
| Breadcrumbs | P2 | Segments muted, current = fg-strong (no fill); separator `›` faint; overflow collapse `…` middle-out; k9s `frame.crumbs` active-role pairing |
| Section | P2 | Title = bold fg-strong + optional faint trailing rule `────` filling the row (Grok Build label-in-border), body inset 0; no box per section |
| Stack / Inline | P1 | Gap from `SpaceScale` by density (never 0 accidental); alignment options (start/center/end/baseline) documented; spacer bands intentional |
| Panel | P0 | Consume `PanelRecipe` (surface + pad + border role) — the recipe exists with zero readers; title-in-border pattern optional `╰─ title ──╯`; one bright border law enforced via overlay/scene |
| Surface | P0 | Sole chrome authority: every widget paints through it (fill/pad/elevation); kill the 137 hand-rolled box paths; `style.bg=None` copies removed so primary fills render |
| SplitPane | P1 | Divider = 1-cell quiet line, hover/grab = border-focused fg; size hint while dragging (`42 cols · 38%`) faint; min-size contraction rules shared |
| ResizablePanelGroup | P1 | Same divider language as SplitPane (one recipe); handle hit region ≥ 1 col + keyboard resize intents; panel chrome comes from Panel, not local boxes |
| ShortcutHint | P1 | Key = `kbd` chip (faint border or muted fg + bold), action = muted; separator `·` faint; wraps measured (Jackin's 3-pass layout); never accent-green by default |
| KeyboardHelp | P1 | Two-column kbd/action grid, grouped with muted section headers; footer variant = single faint line; search/filter when > 12 bindings |

### Inputs & forms

| Component | P | Improvements |
|-----------|---|--------------|
| TextInput | P0 | Sunken well + block cursor; focus = well stays, border (if bordered) recolors, **no underline**; placeholder faint; invalid = danger fg on value + `!` + error line beneath; selection inside text = reverse |
| TextArea | P1 | Same well/cursor language; cursor-line highlight = `raised` row wash (tui-textarea parity), optional faint line numbers gutter; status line (L4:C12) faint right |
| PasswordInput | P1 | Mask bullet `●` (not `*`) at fixed width; reveal toggle eye-glyph optional with ASCII fallback; caps-lock hint; same focus language as TextInput |
| NumberInput | P1 | Stepper `‹ ›` buttons ghost muted, value strong; drag/scrub optional; invalid range clamps with faint hint, not silent |
| SearchInput | P1 | Leading `🔍`/`/` glyph muted; match count trailing faint (`12/3,402`); clear `✕` ghost button; integrates Picker match grammar for results |
| Select | P1 | Trigger = sunken well + value strong + `▾` muted; dropdown = Picker state grammar on `elevated`; selected option `●` marker, not fill |
| RadioGroup | P1 | `(●)` selected / `( )` idle — glyph catalog; selected label strong; group label bold once; horizontal variant spaces from token |
| Slider | P1 | Track `─` faint, fill portion accent-muted (not neon), thumb `●` strong, focus = thumb accent + bold value; value bubble trailing right-aligned |
| RangeSlider | P1 | Same grammar; range fill between thumbs muted accent; min/max labels faint; keyboard step intents documented |
| TokenField | P1 | Tokens = `raised` chips with paired fg + faint `✕`; editing token gets accent left caret; overflow `+3` chip faint; input remainder sunken |
| ThemePicker | P1 | Swatch rows = 4–6 cell color blocks of the theme's own tokens (self-rendering preview); current theme `●` marker; truecolor-only themes hidden on lesser terminals (Grok Build rule); live preview applies on cursor move, commit on enter |

### Feedback & status

| Component | P | Improvements |
|-----------|---|--------------|
| Toast | P1 | One-line preferred; severity carried by **icon glyph + fg only** (`✓ ! i`), border muted, body soft white; stacked toasts on `elevated`; no neon flash on entry — 120 ms fade per motion token |
| Spinner | P1 | Braille default + verb text muted (`⠹ running tests`); quiet dot-pulse variant for background tasks; reduced-motion = static `…`; `throbber_style` separate from container |
| LoadingView | P1 | Skeleton blocks (`raised` shimmer ≤ 150 ms fade loop or static under reduced-motion) matching the layout being loaded; never full-screen spinner for > 400 ms |
| StatusIndicator | P1 | Dot `●` colored by status + **letter/word** for mono (`R run`, `W wait`); pulse only when live; label muted |
| ChoiceDialog | P1 | Elevated + dimmed backdrop; danger variant = danger border + `!` + scope body naming the target; default option never destructive; options as radio rows or ghost buttons, primary alone solid |
| Collapsible | P2 | Disclosure `▸▾` muted + title strong; `DisclosureHeader` amber only for group headers (Jackin role); body inset from token; chevron never accent-green |
| Timeline | P2 | Rail `│` faint, node glyphs `●○✓×` by status, current node = accent + bold; timestamps faint right-aligned; connecting line unbroken through muted events |
| Stepper | P1 | Completed `✓` muted, current `●` accent + bold label, upcoming `○` faint; connectors faint; vertical variant for wizard; numbers optional, never badges-on-badges |
| LogPane | P1 | Level gutter letter+color (`I W E` muted ramp), timestamps faint, message body soft white; current-line = `raised` wash; follow-mode indicator in title bar (k9s pattern), not a floating badge |
| Histogram | P1 | Bars `▄` ramp via `chart-*` tokens (derived, not hand hexes); axis labels faint; peak/current value annotated strong; overflow bucket `▸max+` explicit |

### Data display

| Component | P | Improvements |
|-----------|---|--------------|
| KeyValueTable | P2 | Keys muted fixed-column, values body; section groups with faint rules; copy affordance hint faint; no borders per row |
| DetailTable | P2 | Same key/value language; nested groups indent 2 cells; empty value = `—` faint, not blank |
| Tag | P1 | Chip = `raised` fill + paired fg, no border; status tags add glyph; removable = faint `✕`; max one accent tag per row group |
| Chip | P1 | Same chip recipe as Tag (one recipe, two names — verify no duplicate paint paths); selected chip = strong + `▌`, not inverted neon |

---

## 7. Token & API deltas required

Breaking changes, each needs a `migrations/` entry at implementation time:

1. **`Role::Selection` → gutter/tint resolution**; delete the neon fill default.
   `SelectionChrome::Fill` removed (kept only behind explicit opt-in during one
   migration window, then deleted — no permanent dual path).
2. **Retire underline from state paint**: remove `show_focus_underline` from
   `ResolvedListRow` (`tokens.rs:816,856`); remove
   `TabUnderline{Focused,Unfocused}` roles (tabs move to connected-fill model);
   strip `underlined()` from accent recipes (`style/mod.rs:501,592,741-742`);
   keep underline solely in `Link`/`LinkHover`.
3. **`Focus` role de-collapsed** from `Accent`: focus resolution = recipe-level
   (gutter + strong + tint / border-focused), not a raw color. `Role::Focus` may
   become the gutter glyph fg only.
4. **Derivation layer** in `RolePalette`: author ~16 base tokens, generate muted/
   subtle/tint/disabled variants; existing tail roles re-homed as component
   variables deriving from bases.
5. **`PanelRecipe`/`Surface` become the sole chrome path**; `style.bg = None`
   copies removed; backdrop dim wash wired into the overlay stack.
6. **Connected-tab anatomy** in `Tabs`: new part set (`tab-active-fill`,
   `tab-inactive`, `tab-divider`), variant enum (`Connected | Minimal`).
7. **Picker state slots** (`pointer/marker/match/selection`) added to
   `Picker`/`QuickOpen`/`Select` recipes — shared recipe, three consumers.
8. **Density/style presets** (`full|default|minimal`) on collections.

## 8. Rollout order

1. **P0 foundation** — selection de-neon, underline retirement, focus de-collapse,
   Surface/PanelRecipe sole chrome, backdrop dim. (One coherent break; migration
   file.) Everything visual downstream inherits.
2. **P0 collections** — List, VirtualList, Picker, QuickOpen, Tabs, TextInput to
   the new state paint; regenerate lookbook SVGs (stale neon SVGs = product bugs).
3. **P1 cohesion pass** — remaining matrix rows by family; density presets;
   derivation ladder for theme authoring.
4. **P2 polish** — motion garnish, swatch self-previews, accessible plain-prompt
   degradation mode (huh model), ANSI-16 slot remapping.

## 9. Acceptance criteria

- Viewport test: ≤ 2 accent-forward regions in any catalog scene.
- Focus vs selection: an untrained user points to two different marks; neither is
  an underline.
- Underline appears only on hovered links in the entire rendered catalog.
- 8-hour ops session screenshot: no full-row saturated fills; metadata never
  body-white.
- Monochrome projection: every state still distinguishable (glyph/bold/reverse).
- Theme authoring: a new theme = ≤ 20 overrides over the generator.
- Lookbook SVGs regenerated; gate fails on full-row phosphor fill or stray
  `UNDERLINED` outside `Link`.
