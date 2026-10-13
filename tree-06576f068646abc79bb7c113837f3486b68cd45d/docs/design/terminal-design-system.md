# TermRock terminal-native design system

**Status:** specification for implementation (plan 043+)  
**Supersedes:** treating `Theme` / `Role` alone as the design system  
**Builds on:** `Theme`, `Role`, `Density`, `Motion`, `GlyphSet`, `SelectionChrome`,
`SpacingScale`, `DesignTokens`, `ColorCapability`, `Appearance`  
**Constraint:** terminal-native (cells, glyphs, modifiers, capability ladders)—not CSS-in-Rust

---

## 1. Design principles

1. **Quiet canvas, bright intent.** Structure is calm (surfaces, muted text, gray borders). Phosphor (or brand accent) appears for *current* intent: keyboard focus, primary action, live/running—not every selected row.
2. **Cells are the unit.** Spacing, insets, gaps, breakpoints, and dimensions are integer terminal cells. Never rem, never px.
3. **Non-color always carries state.** Focus, selection, disabled, loading, success/danger remain legible under monochrome / `NO_COLOR` via glyphs, underline, reverse, bold/dim.
4. **Semantic tokens over raw RGB in components.** Widgets resolve `Role` / recipes; only theme authors touch palette numbers.
5. **Recipes own parts and states.** Components paint through resolved recipes, not ad-hoc `theme.style(Role::…)` soup.
6. **Capability is progressive.** Truecolor is the design target; 256 / ANSI / monochrome are first-class projections, not afterthoughts.
7. **Density is a product mode**, not optional padding. Comfortable / Compact / Dashboard change insets, gaps, chrome height.
8. **Glyphs are themable.** Unicode box-drawing is default; ASCII fallback is a token set, not scattered string literals.
9. **Motion is optional.** Spinners and soft transitions respect reduced-motion; zero motion remains correct.
10. **Single-line borders for focus.** Border *weight* never communicates focus (AGENTS.md). Focus is role/style/underline/gutter—not double-line boxes.
11. **Terminal default background is sacred.** Backdrop and canvas may use `Color::Reset` so app chrome respects the user’s terminal profile.
12. **App overrides are surgical.** Customize one list recipe without forking the whole theme.

---

## 2. Token taxonomy

```text
DesignSystem
├── capability: ColorCapability          # truecolor | 256 | ansi16 | mono
├── polarity: Appearance                 # dark | light | unknown
├── density: Density                     # comfortable | compact | dashboard
├── motion: Motion                       # full | reduced | off
├── glyphs: GlyphCatalog                 # unicode | ascii (+ custom)
├── space: SpaceScale                    # cell scale + density resolution
├── break: Breakpoints                   # cols/rows thresholds
├── type: TypeScale                      # terminal “typography”
├── color: ColorTokens                   # semantic color roles → Style
├── chrome: ChromeTokens                 # borders, dividers, focus, selection, elevation
├── viz: VizTokens                       # charts
├── syntax: SyntaxTokens                 # code / diff
├── dim: DimensionTokens                 # min widths/heights for chrome
└── recipes: RecipeBook                  # per-component part×state → paint plan
```

### 2.1 Semantic colors (`ColorTokens`)

Map **meaning** → `Style` (fg/bg/modifiers). Not CSS variables: each role is a full cell style.

| Group | Tokens |
|-------|--------|
| **Surface** | `canvas`, `surface`, `surfaceRaised`, `elevated`, `sunken`, `overlay`, `backdrop` |
| **Foreground** | `fg`, `fgStrong`, `fgMuted`, `fgFaint`, `fgDisabled`, `fgOnAccent`, `fgOnDanger` |
| **Border** | `border`, `borderMuted`, `borderFocused`, `borderDanger` |
| **Intent** | `accent`, `accentMuted`, `accentEmphasis` |
| **Status** | `success`, `successMuted`, `warning`, `warningMuted`, `danger`, `dangerMuted`, `info`, `infoMuted` |
| **Interactive** | `link`, `linkHover`, `selection`, `selectionMuted`, `hover`, `focusRing` |
| **Input** | `input`, `inputPlaceholder`, `inputInvalid`, `cursor` |
| **Chrome** | `statusBar`, `hintKey`, `hintText`, `hintDim`, `scrollTrack`, `scrollThumb` |
| **Tab** | `tabActive`, `tabInactive`, `tabActiveHover`, `tabInactiveHover`, `tabUnderlineFocused`, `tabUnderlineQuiet` |
| **Action** | `action`, `actionFocused`, `actionDisabled` |

**Foreground hierarchy (terminal):**

| Level | Means | Typical terminal means |
|-------|--------|-------------------------|
| Strong | Titles, selected primary when not filled | Bold + brighter fg |
| Default | Body | Normal fg |
| Muted | Secondary metadata | Dimmer RGB or dim modifier |
| Faint | Timestamps, de-emphasized | Further dim |
| Disabled | Unavailable | Dim + muted hue or dim only (mono) |

**Surface hierarchy:**

| Level | Use |
|-------|-----|
| Canvas | App background (often Reset) |
| Surface | Panel body |
| SurfaceRaised | Nested card inside panel |
| Elevated | Dialog, popover, command palette |
| Sunken | Input wells, code blocks |
| Overlay | Floating menus |
| Backdrop | Modal scrim (often Reset + glyph wash) |

### 2.2 Typography-like scale (`TypeScale`)

Terminals lack font size. Hierarchy uses **modifiers + role + optional prefix glyphs**:

| Style | Modifiers / cues |
|-------|------------------|
| `display` | Bold + `fgStrong` (rare, dialog titles) |
| `title` | Bold + `fgStrong` |
| `body` | Default |
| `label` | Default or bold |
| `meta` | `fgMuted` |
| `code` | `fg` on `sunken` bg |
| `kbd` | Reverse or bordered single-cell look via recipe |

### 2.3 Spacing (`SpaceScale`) — cells only

```text
space.0 = 0
space.1 = 1 cell
space.2 = 2
space.3 = 3
space.4 = 4
space.6 = 6   # rare, dialog outer margin
```

Resolved **insets/gaps** depend on density:

| Token | Comfortable | Compact | Dashboard |
|-------|-------------|---------|-----------|
| `inset.panel` | (2,1) | (1,0) | (0,0) |
| `inset.row` | (1,0) | (0,0) | (0,0) |
| `inset.dialog` | (2,1) | (1,1) | (1,0) |
| `gap.section` | 1 | 1 | 0 |
| `gap.inline` | 1 | 1 | 1 |
| `gap.stack` | 1 | 0 | 0 |
| `gutter.selection` | 2 | 1 | 1 |
| `min_touch` (mouse row) | 1 | 1 | 1 |

### 2.4 Borders & dividers (`ChromeTokens`)

| Token | Glyph source | Style |
|-------|--------------|-------|
| `border.normal` | single box | `border` |
| `border.focused` | **same single box** | `borderFocused` (color/underline—not weight) |
| `border.danger` | single | `borderDanger` |
| `divider.horizontal` | `─` / `-` | `borderMuted` |
| `divider.vertical` | `│` / `\|` | `borderMuted` |
| `divider.section` | `═` / `=` optional | muted |

### 2.5 Focus & selection

| Mechanism | Default (Phosphor Obsidian) | Mono fallback |
|-----------|----------------------------|---------------|
| **Focus (container)** | `borderFocused` accent on single-line border | Bold border glyph side or title `*` |
| **Focus (row)** | underline on primary label OR left gutter accent | Underline / reverse cell |
| **Selection** | Gutter `▌` + muted surface tint (not full phosphor fill) | Gutter `>` + reverse |
| **Hover** | Subtle surfaceRaised / muted fg | Underline |
| **Keyboard vs pointer** | Selection ≠ hover; focus-visible only for keyboard owner | Same |

### 2.6 Overlay elevation

| Level | Surface token | Backdrop |
|-------|---------------|----------|
| 0 in-flow | surface | — |
| 1 popover/menu | overlay | none or light |
| 2 dialog | elevated | backdrop wash `░` optional |
| 3 toast | elevated + border | none |

Backdrop: prefer `Color::Reset` + optional dim glyph field (not opaque black that fights terminal theme).

### 2.7 Glyph catalog

```text
disclosure.open / .closed
selection.gutter
bullet / check.on / check.off / radio.on / radio.off
status.success / warning / danger / info / running / pending / cancelled
spinner.frames[]
border.* (tl,tr,bl,br,h,v)
scrollbar.track / thumb
fold.marker
```

Each entry: `{ unicode: &str, ascii: &str }`.

### 2.8 Motion

| Token | Full | Reduced | Off |
|-------|------|---------|-----|
| `spinner.advance_every_n_ticks` | 1 | 4 | never |
| `progress.pulse` | yes | no | no |
| `toast.fade` | optional dim steps | instant | instant |

### 2.9 Responsive breakpoints (columns)

| Name | Cols | Behavior |
|------|------|----------|
| `xs` | ≤ 40 | Single column; drop secondary/badge/shortcut |
| `sm` | 41–60 | Secondary optional |
| `md` | 61–100 | Full row anatomy |
| `lg` | 101–140 | Comfortable multi-pane |
| `xl` | ≥ 141 | Wide workbench |

Row recipes take `cols: u16` and apply **part priority** (drop order).

### 2.10 Component dimensions

| Token | Comfortable | Compact | Dashboard |
|-------|-------------|---------|-----------|
| `row.height` | 1 | 1 | 1 |
| `input.height` | 1 | 1 | 1 |
| `tab.height` | 2 | 2 | 1–2 |
| `status.height` | 1 | 1 | 1 |
| `dialog.min_width` | 40 | 36 | 32 |
| `palette.min_width` | 48 | 40 | 36 |
| `panel.min_width` | 20 | 16 | 12 |

### 2.11 Visualization tokens

| Token | Use |
|-------|-----|
| `viz.series[0..7]` | Chart series hues |
| `viz.grid` | Faint grid |
| `viz.positive` / `viz.negative` | Up/down |
| `viz.fill` / `viz.empty` | Bar glyphs █ ░ |
| `viz.spark.max` | Peak accent |

### 2.12 Syntax & diff tokens

| Token | Use |
|-------|-----|
| `syntax.default` / `keyword` / `string` / `comment` / `number` / `type` / `function` / `operator` | Highlighter roles |
| `diff.added.fg/bg` / `diff.removed.fg/bg` / `diff.context` / `diff.hunk` | DiffView |

---

## 3. Rust structures and APIs

```rust
// crates/termrock/src/style/system.rs (target)

use ratatui_core::style::{Color, Modifier, Style};

/// Entry point applications hold and pass into widgets.
#[derive(Debug, Clone, PartialEq)]
pub struct DesignSystem {
    pub capability: ColorCapability,
    pub polarity: Appearance,
    pub density: Density,
    pub motion: Motion,
    pub glyphs: GlyphCatalog,
    pub space: SpaceScale,
    pub breaks: Breakpoints,
    pub typography: TypeScale,
    pub color: ColorTokens,
    pub chrome: ChromeTokens,
    pub viz: VizTokens,
    pub syntax: SyntaxTokens,
    pub dim: DimensionTokens,
    pub recipes: RecipeBook,
}

impl DesignSystem {
    pub fn phosphor_obsidian() -> Self { /* flagship dark */ }
    pub fn phosphor_day() -> Self { /* light */ }
    pub fn slate() -> Self { /* existing slate spirit */ }
    pub fn high_contrast(polarity: Appearance) -> Self { /* … */ }
    pub fn adaptive() -> Self {
        let pol = Appearance::detect();
        match pol {
            Appearance::Light => Self::phosphor_day(),
            _ => Self::phosphor_obsidian(),
        }
    }

    /// Project all colors through the capability ladder (pure; clone-based).
    pub fn with_capability(self, cap: ColorCapability) -> Self { /* quantize color tokens */ }

    pub fn with_density(mut self, d: Density) -> Self {
        self.density = d;
        self.space = SpaceScale::for_density(d);
        self.dim = DimensionTokens::for_density(d);
        self
    }

    /// Surgical override: replace one semantic color style.
    pub fn with_color(mut self, key: ColorKey, style: Style) -> Self { … }

    /// Surgical override: replace one recipe part.
    pub fn with_list_recipe(mut self, f: impl FnOnce(ListRecipe) -> ListRecipe) -> Self { … }

    pub fn resolve_list_row(&self, state: ListRowState, cols: u16) -> ResolvedListRow { … }
    pub fn resolve_panel(&self, state: PanelState) -> ResolvedPanel { … }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SpaceScale {
    pub s0: u16, pub s1: u16, pub s2: u16, pub s3: u16, pub s4: u16, pub s6: u16,
    pub inset_panel: Inset,
    pub inset_row: Inset,
    pub inset_dialog: Inset,
    pub gap_section: u16,
    pub gap_inline: u16,
    pub gap_stack: u16,
    pub gutter_selection: u16,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Inset { pub x: u16, pub y: u16 }

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Breakpoints {
    pub xs: u16, // 40
    pub sm: u16, // 60
    pub md: u16, // 100
    pub lg: u16, // 140
}

impl Breakpoints {
    pub fn class(self, cols: u16) -> BreakClass { … }
}

#[derive(Debug, Clone, PartialEq)]
pub struct ColorTokens {
    // each field: Style
    pub canvas: Style,
    pub surface: Style,
    pub surface_raised: Style,
    pub elevated: Style,
    pub sunken: Style,
    pub overlay: Style,
    pub backdrop: Style,
    pub fg: Style,
    pub fg_strong: Style,
    pub fg_muted: Style,
    pub fg_faint: Style,
    pub fg_disabled: Style,
    pub fg_on_accent: Style,
    pub border: Style,
    pub border_focused: Style,
    pub accent: Style,
    pub selection: Style,
    pub selection_muted: Style,
    pub hover: Style,
    pub focus_ring: Style,
    pub success: Style,
    pub warning: Style,
    pub danger: Style,
    pub info: Style,
    // … full set from taxonomy
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GlyphCatalog {
    pub set: GlyphSetKind, // Unicode | Ascii
    pub disclosure_open: CellGlyph,
    pub disclosure_closed: CellGlyph,
    pub selection_gutter: CellGlyph,
    pub check_on: CellGlyph,
    pub check_off: CellGlyph,
    pub status_success: CellGlyph,
    pub status_danger: CellGlyph,
    pub status_running: CellGlyph,
    pub spinner_frames: &'static [&'static str],
    // …
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct CellGlyph {
    pub unicode: &'static str,
    pub ascii: &'static str,
}

impl CellGlyph {
    pub fn get(self, set: GlyphSetKind) -> &'static str { … }
}
```

### Component recipes (List example)

```rust
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ListRowState {
    pub selected: bool,
    pub focused: bool,   // list owns keyboard focus AND this row is cursor
    pub hovered: bool,
    pub disabled: bool,
    pub loading: bool,
    pub checked: bool,   // multi-select
}

/// Authoring-time recipe (stored in DesignSystem).
#[derive(Debug, Clone, PartialEq)]
pub struct ListRecipe {
    pub container: ContainerRecipe,
    pub row: PartRecipe,
    pub leading: PartRecipe,
    pub primary: PartRecipe,
    pub secondary: PartRecipe,
    pub badge: PartRecipe,
    pub shortcut: PartRecipe,
    pub selection_indicator: SelectionIndicatorRecipe,
    // state patches
    pub when_selected: ListStatePatch,
    pub when_focused: ListStatePatch,
    pub when_hovered: ListStatePatch,
    pub when_disabled: ListStatePatch,
    pub when_loading: ListStatePatch,
    /// Drop order for narrow terminals (first dropped first).
    pub part_priority: [ListPart; 6],
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ListPart { Secondary, Badge, Shortcut, Leading, SelectionIndicator, Primary }

#[derive(Debug, Clone, PartialEq)]
pub struct PartRecipe {
    pub fg: ColorKey,          // semantic key into ColorTokens
    pub bg: Option<ColorKey>,
    pub modifiers: Modifier,
    pub prefix: Option<GlyphKey>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct SelectionIndicatorRecipe {
    pub kind: SelectionChrome, // Gutter | Fill | Tint | None
    pub glyph: GlyphKey,
    pub color: ColorKey,       // accent by default
}

/// Paint-ready plan for one row at a given width.
#[derive(Debug, Clone, PartialEq)]
pub struct ResolvedListRow {
    pub pad_x: u16,
    pub fill: Option<Style>,
    pub parts: Vec<ResolvedPart>, // already priority-culled for cols
    pub focus_underline: bool,
}

#[derive(Debug, Clone, PartialEq)]
pub struct ResolvedPart {
    pub kind: ListPart,
    pub style: Style,
    pub min_cols: u16,
}
```

Widgets take `&DesignSystem` (or `&RecipeBook` + colors) instead of bare `&Theme` over time.

```rust
pub struct List<'a, Id> {
    rows: &'a [ComposedRow<'a, Id>],
    system: &'a DesignSystem,
}

pub struct ComposedRow<'a, Id> {
    pub id: Id,
    pub leading: Option<Line<'a>>,
    pub primary: Line<'a>,
    pub secondary: Option<Line<'a>>,
    pub badge: Option<Line<'a>>,
    pub shortcut: Option<&'a str>,
    pub enabled: bool,
    pub loading: bool,
}
```

---

## 4. Theme inheritance and partial overrides

```rust
impl DesignSystem {
    /// Deep clone then apply patch (no shared mutable global).
    pub fn extend(&self, patch: DesignSystemPatch) -> Self { … }
}

#[derive(Default)]
pub struct DesignSystemPatch {
    pub density: Option<Density>,
    pub motion: Option<Motion>,
    pub glyphs: Option<GlyphSetKind>,
    pub colors: Vec<(ColorKey, Style)>,
    pub list: Option<ListRecipePatch>,
    pub panel: Option<PanelRecipePatch>,
    // …
}

// Builder sugar
let app = DesignSystem::phosphor_obsidian()
    .with_density(Density::Compact)
    .with_color(ColorKey::Accent, Style::new().fg(Color::Rgb(0, 200, 255)))
    .with_list_recipe(|mut list| {
        list.selection_indicator.kind = SelectionChrome::Gutter;
        list
    });
```

**Inheritance rules:**

1. Start from a **named preset** (complete system).
2. Patches only override specified keys.
3. Capability projection runs **last** (`with_capability`) so authors author in truecolor.
4. No ambient global theme—pass `&DesignSystem` explicitly (testable, multi-window safe).

---

## 5. Component-level recipe overrides

Applications customize **one component** without rebuilding colors:

```rust
// Only lists become gutter-selected; dialogs keep defaults.
let mut system = DesignSystem::phosphor_obsidian();
system.recipes.list.selection_indicator.kind = SelectionChrome::Gutter;
system.recipes.list.when_selected.primary_modifiers = Modifier::BOLD;

// Or per-widget instance override (does not mutate global system):
List::new(&rows, &system)
    .recipe_override(|r| {
        r.part_priority = [
            ListPart::Shortcut,
            ListPart::Badge,
            ListPart::Secondary,
            ListPart::Leading,
            ListPart::SelectionIndicator,
            ListPart::Primary,
        ];
    });
```

Instance overrides merge: `instance_patch` over `system.recipes.list` over hardcoded defaults.

---

## 6. Runtime theme switching

```rust
pub struct ThemeController {
    active: DesignSystem,
    capability: ColorCapability,
}

impl ThemeController {
    pub fn set_preset(&mut self, preset: SystemPreset) {
        let base = match preset {
            SystemPreset::PhosphorObsidian => DesignSystem::phosphor_obsidian(),
            SystemPreset::PhosphorDay => DesignSystem::phosphor_day(),
            SystemPreset::Slate => DesignSystem::slate(),
            SystemPreset::HighContrastDark => DesignSystem::high_contrast(Appearance::Dark),
            SystemPreset::Adaptive => DesignSystem::adaptive(),
        };
        self.active = base.with_capability(self.capability);
    }

    pub fn set_capability(&mut self, cap: ColorCapability) {
        self.capability = cap;
        // re-project from last unquantized snapshot OR re-build preset
    }

    pub fn system(&self) -> &DesignSystem { &self.active }
}
```

Lookbook / ThemePicker: live-preview by swapping `ThemeController` and redrawing; no widget internal caches of absolute RGB without invalidation.

`FrameTick` + `motion` drive spinner frame index:

```rust
let frame = system.glyphs.spinner_frame(tick.frame_index(), system.motion);
```

---

## 7. Truecolor · 256 · ANSI · no-color

| Capability | Projection |
|------------|------------|
| **Truecolor** | Authored RGB as-is |
| **Indexed256** | `rgb_to_xterm256` on all RGB roles (existing) |
| **Ansi16** | Map to nearest ANSI; prefer hue families (danger→red, accent→green/cyan) |
| **Monochrome / NO_COLOR** | fg/bg → Reset; keep **modifiers** (bold/dim/underline/reverse); force glyph cues on |

**Mono rules (terminal-native a11y):**

- Selection: reverse or gutter ASCII `>`  
- Focus: underline  
- Danger: bold + `!` / `x` prefix from glyph catalog  
- Disabled: dim  

Detection: existing `ColorCapability::detect_from_env()`; apps may force.

---

## 8. Dark, light, adaptive, high-contrast

| Preset | Polarity | Notes |
|--------|----------|-------|
| **Phosphor Obsidian** | Dark | Flagship (below) |
| **Phosphor Day** | Light | Same accent language, light surfaces |
| **Slate** | Dark cool | Existing slate spirit, full surface ladder |
| **High Contrast Dark/Light** | Either | Max fg/bg separation; accent not sole cue |
| **Adaptive** | From `Appearance::detect()` | Maps to Obsidian or Day |

---

## 9. Flagship theme: **Phosphor Obsidian**

**Mood:** deep neutral canvas, soft graphite surfaces, **rare** phosphor green for intent. Terminal-native CRT homage without “everything is green.”

### Palette (truecolor targets)

| Token | RGB | Role |
|-------|-----|------|
| Canvas | Reset preferred, else `#0a0c0a` | App void |
| Surface | `#121612` | Panel body |
| Surface raised | `#1a1f1a` | Nested |
| Elevated | `#1e2620` | Dialogs / palette |
| Sunken | `#0d100d` | Inputs / code |
| Border | `#2a332c` | Quiet structure |
| Border focused | `#00ff41` | **Intent only** |
| Fg | `#d6e0d6` | Body |
| Fg strong | `#f0f5f0` bold | Titles |
| Fg muted | `#7a8a7a` | Secondary |
| Fg faint | `#4a574a` | Meta |
| Fg disabled | `#3a453a` dim | Disabled |
| Accent | `#00ff41` | Live / primary action / focus border |
| Accent muted | `#009928` | Checked non-cursor |
| Selection fill | `#14331a` (muted green tint) | **Not** full neon |
| Selection gutter | glyph + accent fg | Default chrome |
| Hover | `#1a221c` | Subtle |
| Success | `#5dffa0` | |
| Warning | `#f0c040` | |
| Danger | `#ff5e7a` | |
| Info | `#5ec8ff` | |
| Diff add | fg success on `#0d2818` | |
| Diff remove | fg danger on `#2a1218` | |

**Selection default:** `SelectionChrome::Gutter` (not Fill).  
**Focus container:** single-line border + `border_focused`.  
**Focus row:** underline primary when focused+selected; gutter always if selected.  
**Backdrop:** Reset + optional `░` at 10% visual weight (dim).

---

## 10. Migration from current `Theme` API

### Current

- `Theme { roles: [Style; 38] }` + `Role` enum  
- Widgets take `theme: &Theme`  
- Partial `DesignTokens` unused by paint  

### Target

- `DesignSystem` is the root  
- `Theme` / `Role` remain as **compatibility projection** for one migration window *or* collapse into `ColorTokens` immediately (preferred: **forward-only**, plan 043)

### Steps (migration `0036` / plan 043)

1. Land `DesignSystem` + token structs + Phosphor Obsidian preset (parallel module).  
2. Implement `From<&DesignSystem> for Theme` mapping ColorTokens → legacy Role array for leftover widgets.  
3. Migrate List → `DesignSystem` + recipes; delete hardcoded selection paint.  
4. Migrate Panel, Tree, Table, dialogs.  
5. Remove bare `theme: &Theme` from public constructors (breaking) **or** accept `impl IntoSystem` that wraps Theme as “colors only + defaults.”  
6. ThemePicker switches `DesignSystem` presets.  
7. Lookbook stories for density, capability, gutter vs fill, Obsidian vs Day.  
8. Delete dead role constants that forced green selection.

```rust
// Bridge during migration only
impl DesignSystem {
    pub fn legacy_theme(&self) -> Theme {
        Theme::from_fn(|role| match role {
            Role::Canvas => self.color.canvas,
            Role::Surface => self.color.surface,
            Role::Elevated => self.color.elevated,
            Role::BorderFocused => self.color.border_focused,
            Role::Selection => self.color.selection,
            Role::Accent => self.color.accent,
            // …
            _ => self.color.fg,
        })
    }
}
```

Preferred end state: widgets **only** take `&DesignSystem`; `Role` becomes internal or removed.

---

## 11. Stories and tests

### Unit tests

| Test | Asserts |
|------|---------|
| `obsidian_surfaces_differ` | canvas ≠ surface ≠ elevated styles |
| `obsidian_selection_not_equal_accent_fill` | selection bg ≠ accent solid neon when gutter mode |
| `density_insets_monotonic` | comfortable ≥ compact ≥ dashboard pad |
| `glyph_ascii_fallback` | all glyphs non-empty in ascii set |
| `capability_mono_keeps_underline_bold` | mono projection preserves modifiers |
| `list_recipe_drops_secondary_before_primary` | at cols=40 secondary absent, primary present |
| `list_gutter_vs_fill` | resolved fill option differs |
| `motion_off_spinner_static` | frame index stable |
| `patch_only_list_recipe` | color.accent unchanged when only list patched |
| `legacy_theme_bridge_roundtrip_roles` | during migration |

### Lookbook stories

| ID | Purpose |
|----|---------|
| `system/phosphor-obsidian` | Flagship gallery strip |
| `system/phosphor-day` | Light |
| `system/high-contrast` | A11y |
| `system/density-matrix` | Same list × 3 densities |
| `system/capability-mono` | NO_COLOR look |
| `system/capability-ansi16` | Quantized |
| `list/recipe-gutter` | Selection gutter |
| `list/recipe-fill` | Selection fill |
| `list/narrow-priority` | Parts drop order |
| `panel/elevation` | Surface vs elevated side-by-side |
| `focus/border-vs-row` | Container focus + row underline |

### Contract matrix additions

- `tokenThemed: covered | exempt`  
- `capability: covered` with mono/ansi stories where claimed  

---

## 12. App customization examples

### A. Brand accent only

```rust
let system = DesignSystem::phosphor_obsidian()
    .with_color(ColorKey::Accent, Style::new().fg(Color::Rgb(80, 160, 255)))
    .with_color(ColorKey::BorderFocused, Style::new().fg(Color::Rgb(80, 160, 255)));
```

### B. Only lists use quiet selection

```rust
let system = DesignSystem::phosphor_obsidian().with_list_recipe(|mut list| {
    list.selection_indicator.kind = SelectionChrome::Gutter;
    list.when_selected.fill = Some(ColorKey::SelectionMuted);
    list
});

frame.render_stateful_widget(List::new(&rows, &system), area, &mut state);
// Dialogs still use default elevated recipes from the same system
```

### C. Dashboard density + no motion

```rust
let system = DesignSystem::phosphor_obsidian()
    .with_density(Density::Dashboard)
    .motion(Motion::Off)
    .with_capability(ColorCapability::detect_from_env());
```

### D. One widget local override

```rust
List::new(&rows, &system).map_recipe(|r| {
    r.part_priority = [/* hide shortcuts first */];
    r
})
```

### E. Runtime switch

```rust
// ThemePicker confirms "day"
controller.set_preset(SystemPreset::PhosphorDay);
// next frame all widgets see controller.system()
```

---

## Implementation order (for plan 043+)

1. **Token structs + Phosphor Obsidian** (no widget migration yet) + tests.  
2. **List recipe resolution + paint migration** (delete hardcoded selection).  
3. **Panel recipe** (surface/border/title).  
4. **Capability projection on DesignSystem**.  
5. **Legacy Theme bridge** + migrate remaining chrome.  
6. **Stories** (density/capability/gutter).  
7. **Breaking:** constructors take `&DesignSystem`.  
8. Tree/Table recipes; syntax/viz tokens wired to CodeBlock/Diff/charts.

---

## Relationship to AGENTS.md

- Phosphor remains the **default language** (Obsidian is the matured phosphor).  
- Single-line borders + focus via semantic color **unchanged**.  
- Product-neutral: any brand can replace accent via patch.  
- Forward-only migration; no long-lived dual paint paths.

---

## Success criteria

1. A designer can change density and **see** list/panel chrome change without code edits per widget.  
2. Phosphor Obsidian screenshots show **muted selection** and **rare** neon.  
3. `NO_COLOR` build still shows selection and focus.  
4. An app rebrands accent in **one patch** without forking List.  
5. List narrow story drops badge before truncating primary label.  
6. ThemePicker switches full `DesignSystem`, not only `Theme` roles.
