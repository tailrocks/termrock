# TermRock inventory — junie-tui design-fidelity campaign

Scope: `/Users/donbeave/Projects/tailrocks/termrock` at branch
`experimental/component-catalog-docs-2026-09-02`, HEAD `f51d0ba8`.
Inventory only — no design decisions.

---

## 0. Workspace shape

`crates/`:

| Crate | Role | Notes |
|---|---|---|
| `termrock` | core lib | ~296k LOC src. The whole design system + widget catalog. |
| `termrock-cli` | binary `termrock` | registry install + capability doctor + contract lint. **Not** a renderer. |
| `termrock-lookbook` | binary + stories | 1090 stories, SVG/PNG/poster renderer, goldens, PNG baselines. |
| `termrock-lookbook-web` | wasm cdylib | mounts lookbook demos, serializes frames to JSON. |
| `termrock-showcase` | binary | single full-screen agent workbench app, 7 scenarios. |
| `termrock-raster` | lib | Buffer → PNG via tiny-skia + swash, vendored JetBrains Mono. |

Workspace deps: ratatui 0.30.2 (`default-features=false`), `ratatui-core` 0.1.2,
`ratatui-widgets` 0.3.2, `crossterm` 0.29, `tui-scrollbar` 0.2.7, `tiny-skia`
0.12, `swash` 0.2.10, `wasm-bindgen`. MSRV `1.97.1`. `missing_docs = deny`,
`unsafe_code = forbid`.

---

## 1. `crates/termrock/src` module tree

Top level (`lib.rs:16-34`): `ansi_text`, `capability`, `context`, `input`,
`interaction`, `keymap`, `layout`, `osc`, `patterns`, `perf`, `registry`,
`runtime`, `scroll`, `style`, `text`, `widgets`, and (feature-gated) `crossterm`.

There is **no `interactors/` dir in `termrock/src`** — interactors live in
`crates/termrock-lookbook/src/interactors/`.

### 1.1 `style/` — the design system (chokepoint)

| File | LOC | Purpose |
|---|---|---|
| `mod.rs` | 1184 | `Role` (63 variants) + `RolePalette`; `color()`, `faded()`, `blend_toward` re-export; `PHOSPHOR_GREEN`/`PHOSPHOR_DARK`/`PREVIEW_CARD` truecolor swatches (web/SVG only, never runtime). |
| `palette.rs` | 107 | `Rgb`, WCAG `relative_luminance`, `contrast_ratio`, `lift()` (hover-lift). Private consts `PHOSPHOR_GREEN = (0,255,65)`, `PHOSPHOR_DARK = (0,80,18)`, `PREVIEW_CARD = (28,28,28)`. |
| `tokens.rs` | 2067 | `GlyphSet`, `SelectionChrome`, `FocusEmphasis`, `SurfaceFamily`, `RecipeFamily`, `NonColorCue`, `AccentUsage`, `MotionSemantics`, `FamilyRecipe`, `BorderShape`, `SpacingScale`, `ContentInset`, `SpacerBand`, `KvSeparator`, `ListRowVisualState`, `PanelChrome`, `Elevation`, `BreakpointScale`, `ButtonRecipeVariant`, `ControlState`, `ButtonRecipe`, `InputRecipe`, `ThemePackage`, **`DesignSystem`**, `ListRowRecipe`. |
| `density.rs` | 90 | `Density::{Comfortable,Compact,Dashboard}` → `padding_x/padding_y/gap/hint_rows/tree_indent`. |
| `glyph.rs` | 924 | `Glyph` semantic catalog (~90 names, 6 `GlyphGroup`s), ramps (`BLOCK_RAMP`, `LEFT_BLOCK_RAMP`, `SHADE_RAMP`, `BRAILLE_RAMP`), `SPINNER_BRAILLE_FRAMES`, `SPINNER_DOT_PULSE_FRAMES`, `MASK_CELLS=8`, `GLYPH_CONTEXTS`. |
| `motion.rs` | 959 | `MotionPolicy::{Full,Basic,Off}`, `Easing`, `MotionChannel`, `effective_alpha`, `shimmer_at`, `wave_brightness`, `pulse_brightness`, `coalesce_cells`, `edge_fade`, `fade_style`. |
| `quantize.rs` | 599 | `ColorCapability::{Truecolor,Indexed256,Ansi16,Monochrome}`, `Ansi16Color` (named 16), `quantize_color`, `quantize_palette`, `rgb_to_xterm256`, `detect_from_env`. |
| `appearance.rs` | 169 | `Appearance::{Dark,Light,Unknown}` detection (`TERMROCK_APPEARANCE`/`COLORFGBG`/macOS `defaults`), `AppearanceThemeMap` (dark=`"phosphor"`, light=`"paper"`), `palette_for_appearance`. |
| `contrast_floor.rs` | 482 | `#[cfg(test)]`-only computed contrast barrier over every painted fg/bg pair, with `KNOWN_SHORTFALLS`. |
| `preview_host.rs` | 598 | `CapabilityPreviewHost`, `PreviewSurface`, media-session commands. |

### 1.2 `widgets/` — 151 files, 210 public identities

Every file's first doc line (source of truth):

| File | Purpose |
|---|---|
| `accent_rail.rs` | Semantic one-column accent rail for composed blocks. |
| `accordion.rs` | Accordion — single- or multi-open disclosure groups. |
| `action_bar.rs` | Semantic hierarchy for an ActionBar item. |
| `agent.rs` | Agent-era widgets: tool cards, thinking, meters (`ThinkingBlock`, `TokenMeter`, `ToolCard`). |
| `agent_blocks.rs` | `ModeRibbon` / workbench mode blocks. |
| `alert_dialog.rs` | AlertDialog — specialized high-risk confirmation surface. |
| `attachment_chips.rs` | AttachmentChip / PasteChip. |
| `badge.rs` | Badge — compact status/category indicator. |
| `blocks.rs` | Generic block chrome helper (`BlockChrome`). |
| `breadcrumbs.rs` | Location context and ancestor navigation. |
| `button_group.rs` | ButtonGroup — grouped actions, priority overflow, roving focus. |
| `callout.rs` | Callout and Alert — inline feedback messages. |
| `card.rs` | Card — raised container composed from Panel + Surface. |
| `carousel.rs` | Carousel — multi-slide panel. |
| `charts.rs` | Sparkline, Chart, Gauge, Histogram, BarSeries, MetricRadar, PieChart family. |
| `checkpoint_timeline.rs` | Rewindable session history. |
| `chrome_row.rs` | ChromeRow — one-line strips a pane grows when busy. |
| `citation.rs` | SourceCitation / CitationList. |
| `code_block.rs` | CodeBlock — production code/command rendering. |
| `collapsible.rs` | Collapsible — accessible disclosure. |
| `combobox.rs` | Combobox / Autocomplete — editable input + suggestions. |
| `command_palette.rs` | CommandPalette — universal command surface. |
| `completion_menu.rs` | CompletionMenu — anchored suggestion surface. |
| `composed_row.rs` | Named-part row projection for priority contraction. |
| `confirm_prompt.rs` | ConfirmPrompt — last question before something irreversible. |
| `connectivity.rs` | Offline / ReconnectingState surfaces. |
| `content.rs` | Heading and paragraph primitives. |
| `context_meter.rs` | ContextMeter — token/resource budget display. |
| `controls.rs` | Checkbox, radio, switch, select, multiselect, combobox controls. |
| `data_table.rs` | DataTable — interactive/virtualized grid. |
| `data_view.rs` | Shared data-presentation abstractions. |
| `date_time_picker.rs` | Date, time, range selection. |
| `dependency_graph.rs` | DependencyGraph — constrained graph viewer. |
| `design_inspector.rs` | Studio-oriented design inspector (lookbook/debug). |
| `detail_table.rs` | DetailTable activation capabilities. |
| `diagnostic.rs` | Diagnostic + CodeFrame — structured diagnostics. |
| `dialog.rs` | Dialog — canonical modal interaction surface. |
| `diff.rs` | DiffView — unified/side-by-side diff renderer. |
| `drawer.rs` | Drawer — edge-mounted secondary surfaces. |
| `dropdown_menu.rs` | DropdownMenu / ContextMenu — nesting, state, shortcuts. |
| `edit_core.rs` | Shared single-line grapheme editing primitives. |
| `empty_state.rs` | EmptyState — useful empty/first-run surfaces. |
| `error_state.rs` | ErrorState + Recovery. |
| `event_stream.rs` | EventStream — high-volume structured-event viewer. |
| `field_message.rs` | One way to say what is wrong with a field. |
| `field_row.rs` | Composed label/value rows for forms. |
| `file_picker.rs` | File and directory browser. |
| `file_tree.rs` | FileTree — filesystem-specialized Tree. |
| `form.rs` | Field, Fieldset, Form. |
| `form_wizard.rs` | Multi-step form flow. |
| `fullscreen_viewer.rs` | FullscreenViewer + SemanticZoom. |
| `hex_viewer.rs` | HexViewer — virtualized binary inspector. |
| `highlighted_text.rs` | HighlightedText + MatchRanges. |
| `hint_bar.rs` | One footer-hint span shared by terminal surfaces. |
| `history_picker.rs` | HistoryPicker — recent-history selector. |
| `icon.rs` | Icon — paint a semantic Glyph with width alignment. |
| `identity.rs` | AvatarGlyph + Identity. |
| `image_surface.rs` | Optional terminal image surface protocol. |
| `input_group.rs` | InputGroup — prefix/field/suffix chrome. |
| `input_otp.rs` | InputOtp — one-time/PIN entry. |
| `jump_overlay.rs` | JumpMode — direct navigation over SemanticScene. |
| `kbd.rs` | Kbd + ShortcutHint — keyboard chord display. |
| `key_value_list.rs` | KeyValueList — compact metadata. |
| `key_value_table.rs` | KeyValueTable — dense interactive detail table. |
| `keybinding_recorder.rs` | Captures/validates user keybindings. |
| `keyboard_help.rs` | KeyboardHelp — generated keyboard help. |
| `label.rs` | Label + Description — field captions. |
| `link.rs` | Link + ActionLink — hyperlinks and inline actions. |
| `list.rs` | List — composable collection view. |
| `loading_overlay.rs` | LoadingOverlay + BusyBoundary. |
| `log_pane.rs` | Append-oriented scrollback. |
| `log_stream.rs` | LogStream — continuous professional log viewer. |
| `markdown.rs` | Editorial, streaming-capable Markdown projection. |
| `mention.rs` | FileMention / EntityMention inline tokens. |
| `menu_bar.rs` | MenuBar — nested menus, mnemonics, overlay cascade. |
| `message_thread.rs` | MessageThread — virtualized conversation transcript. |
| `metric_tile.rs` | MetricTile — one measured number, stated well. |
| `model_mode_selectors.rs` | ModelSelector / AgentModeSelector. |
| `multi_select.rs` | Searchable multiple-choice selector. |
| `notification_center.rs` | NotificationCenter — persistent history surface. |
| `number_input.rs` | Numeric field, draft text separate from committed value. |
| `object_inspector.rs` | ObjectInspector — expandable typed inspector. |
| `pagination.rs` | Page navigation for bounded result sets. |
| `panel.rs` | Panel — composable panel chrome, variants, body modes. |
| `password_input.rs` | Secure single-line secret entry. |
| `path_input.rs` | Filesystem-aware path field. |
| `permission.rs` | PermissionPrompt — signature trust surface. |
| `picker.rs` | Default overlay id for select/picker popups. |
| `popover.rs` | Popover — anchored interactive surface. |
| `preview_card.rs` | PreviewCard — contextual preview. |
| `primitives.rs` | Activation law + `Button`, `IconButton`, `ActivationState`. |
| `progress.rs` | ProgressBar — determinate/indeterminate. |
| `progress_steps.rs` | ProgressSteps — pipeline/phase progress. |
| `prompt_composer.rs` | PromptComposer — flagship agent input surface. |
| `prompt_queue_model.rs` | Prompt-queue identity types. |
| `question_flow.rs` | QuestionFlow — multi-question human-in-the-loop. |
| `quick_open.rs` | QuickOpen — fuzzy resource opener. |
| `resizable_panel_group.rs` | ResizablePanelGroup. |
| `review.rs` | DiffReview — interactive review over DiffView. |
| `row_chrome.rs` | **One selection language for collections** (`RowChrome`). |
| `scroll_area.rs` | ScrollArea — canonical shared scrolling primitive. |
| `search_input.rs` | Search field: query, status, clear, history, filters. |
| `search_results.rs` | SearchResults — grouped, navigable results. |
| `section.rs` | Section — editorial grouping without a border box. |
| `segmented_control.rs` | SegmentedControl — compact mutually exclusive selector. |
| `select.rs` | Single-choice Select: opener + list. |
| `selection.rs` | List multi-select membership facade. |
| `semantic_status.rs` | Shared SemanticStatus vocabulary. |
| `separator.rs` | Separator — semantic rules with optional labels. |
| `sidebar.rs` | NavigationList + Sidebar shell. |
| `skeleton.rs` | Skeleton — low-noise structural placeholders. |
| `slash_command_menu.rs` | SlashCommandMenu — prompt-composer completion. |
| `slider.rs` | Slider + RangeSlider. |
| `spinner.rs` | Spinner + ActivityIndicator. |
| `split_pane.rs` | SplitPane. |
| `status_bar.rs` | StatusBar — low-noise status surface. |
| `status_indicator.rs` | StatusIndicator — compact semantic status. |
| `status_strip.rs` | StatusStrip — a row of short facts. |
| `stepper.rs` | Stepper — multi-step progress/navigation. |
| `streaming_markdown.rs` | StreamingMarkdown — token-by-token output. |
| `surface.rs` | **Lowest-level visual ownership: fill, padding, border, clip, hit.** |
| `table.rs` | Table — static/moderate columnar presentation. |
| `table_chrome.rs` | Shared header language for the table family. |
| `tabs.rs` | Tabs — composable tab strip (`TabsActiveCue`). |
| `tag_chip.rs` | Tag + Chip — removable/selectable tokens. |
| `terminal_cell_grid.rs` | Borrowed terminal-cell projection into a buffer. |
| `terminal_output.rs` | TerminalOutput — safe command output. |
| `text.rs` | Text — canonical styled text primitive. |
| `text_area.rs` | Multi-line grapheme-safe editing, two-axis viewport. |
| `text_input.rs` | Production-grade single-line text editor. |
| `theme_picker.rs` | Live theme picker. |
| `tiered_row.rs` | One row, several tiers. |
| `timeline.rs` | Timeline — chronological events. |
| `toast.rs` | Toast — transient notifications. |
| `toggle.rs` | Toggle + ToggleGroup. |
| `token_field.rs` | Editable token/chip collection. |
| `tool_call_card.rs` | ToolCallCard — agent tool execution card. |
| `toolbar.rs` | Toolbar — roving-focus action strip. |
| `tooltip.rs` | Tooltip — delayed contextual help. |
| `trace_waterfall.rs` | TraceWaterfall — span/latency visualization. |
| `transcript.rs` | Variable-height streaming transcript. |
| `tree.rs` | Tree — hierarchical collection. |
| `tree_navigation.rs` | Hierarchical route navigation. |
| `tree_table.rs` | TreeTable — hierarchical rows + columns. |
| `view_state.rs` | Banner + LoadingView. |
| `viewport.rs` | Scrollable view over borrowed terminal lines. |
| `virtual_grid.rs` | Virtualized two-axis grid. |
| `virtual_list.rs` | VirtualList — extremely large/streaming sets. |
| `virtualizer.rs` | Canonical 1D/2D virtualizer. |
| `field_row.rs`, `composed_row.rs`, `tiered_row.rs`, `chrome_row.rs`, `status_strip.rs` | Composed-row family. |

### 1.3 `patterns/` — 35 copyable application recipes (36 files)

`activity_shelf`, `agent_shell`, `agent_status_header`, `agent_workbench`,
`app_dashboard`, `app_shell`, `approval_queue`, `auth_entry`,
`background_task_panel`, `connection_manager`, `database_workbench`,
`error_recovery`, `file_manager`, `git_workbench`, `help_center`,
`integration_status`, `metrics_dashboard`, `observability_dashboard`,
`ops_dashboard`, `plan_review`, `process_table`, `project_launcher`,
`prompt_queue`, `query_editor`, `resource_browser`, `result_grid`,
`schema_browser`, `session_picker`, `settings_screen`, `setup_wizard`,
`studio_shell`, `subagent_card`, `task_rail`, `terminal_run_card`,
`working_state_card`.

**35 of 36 files reference `DesignSystem`. Zero hardcoded `Color::Rgb` and
zero hardcoded ANSI named colors.** Patterns are pure composition over
`crate::widgets`.

### 1.4 Other modules

| Module | Files | Purpose |
|---|---|---|
| `layout/` | `mod` 294, `center` 880, `dialog` 104, `grid` 1263, `panel_stack` 208, `responsive` 1756, `stack` 1325, `work_surface` 240, `workspace` 400 | Stack/Inline flex packing; Center; Grid templates; PanelStack; responsive priority-tier contraction; WorkSurface (lazygit/k9s shells); Workspace split/dock tree. |
| `interaction/` | `mod` 158, `collection` 524, `dismissable` 704, `event_result` 497, `focus_graph` 1055, `intent` 699, `keymap_bridge` 109, `modal` 28, `overlay_stack` 2905, `roving` 676, `scene` 2102, `selection_model` 925 | Semantic intents; headless collection model; dismissal layers; typed `EventResult`; `FocusGraph`; overlay stack (sole z-order/Escape authority); roving focus; per-frame `InteractionScene`; `SelectionModel`. |
| `input/` | `mod` 43, `event` 356 | Backend-neutral key/mouse vocabulary, logical chords/bindings. |
| `keymap.rs` | 715 (+`keymap/tests.rs` 582) | `Keymap<A>` — single SoT coupling dispatch and hint advertisement. |
| `scroll/` | `mod` 772, `render` 484, `tests` 258 + `render/tests` 229 | Scrollbar state/metrics adapters over `tui-scrollbar`; consumers own rendering. |
| `text/` | `mod` 917 | Display-column measurement, sanitization, windows, `paint_text`. |
| `registry/` | `mod` 38, `catalog` 2155, `contract` 299, `inventory` 1903, `pattern_inventory` 436, `validate` 359 | `ComponentContract` schema; `official_kernel_contracts()` (255 ids, hand-written); `PublicUiId` (210 variants) + `public_ui_inventory()`; `PatternId`; `ValidationReport`. |
| `runtime/` | `mod` 31, `animate` 445, `motion` 530, `presenter` 613, `runner` 472, `subscription` 74, `time` 134 | `FrameTick`/`FrameClock`, `Presence`, typed value animation, dirty coalescing + backpressure presenter, runner. |
| `capability/` | `mod` 30, `boundary` 235, `detect` 256, `doctor` 274, `profile` 599, `set` 377 | `TerminalCapabilities`, `CapabilityBoundary`, `CapabilityProfile`, env detection, `DoctorReport`. |
| `crossterm/` | `mod` 6, `session` 568 | Optional backend + scoped `Session` (alt-screen, raw mode, restore). |
| `osc/` | `mod` 9, `encode` 186, `request` 92 | Typed terminal requests, pure OSC encoders. |
| `perf/` | `mod` 23, `budget` 343, `follow` 179, `stream` 320 | CI-gated hot-path budgets; follow-tail/anchors; batched streaming. |
| `ansi_text.rs` | 1158 (+tests 263) | Safe ANSI SGR parse + paint for untrusted terminal output. |
| `context.rs` | 712 | Per-frame `UiContext` — coordination without a retained DOM. |
| `keymap.rs` | 715 | Keybinding registry. |
| `text/` | — | see above. |

`lib.rs` also contains compile-time policy modules: `root_export_policy`,
`focus_authority_policy`, `paint_authority_policy`, `overlay_authority_policy`
— structural gates that fail the build on forbidden imports.

---

## 2. Central styling architecture

### 2.1 The chokepoint

**`DesignSystem` is the sole paint authority** (`style/tokens.rs:832-874`):
> One object owns palette, density, glyphs, spacing, selection, capability,
> motion, and breakpoints. Widgets take `&DesignSystem` only.

Fields: `palette: RolePalette`, `density: Density`, `motion: MotionPolicy`,
`glyphs: GlyphSet`, `spacing: SpacingScale`, `selection: SelectionChrome`,
`border_shape: BorderShape`, `capability: ColorCapability`,
`breakpoints: BreakpointScale`, `kv_separator: KvSeparator`,
`focus: [FocusEmphasis; 6]` (private), `tick: Option<FrameTick>` (private).

**Measured inheritance — this is the headline finding:**

| Metric | Value |
|---|---|
| Widget files referencing `DesignSystem` | **144 of 151** (`widgets/` has 151 non-test `.rs`) |
| `DesignSystem` occurrences in `widgets/` | 1613 |
| Pattern files referencing `DesignSystem` | 35 of 36 |
| Hardcoded `Color::Rgb(...)` in `widgets/` (non-test) | **6** — all in `terminal_cell_grid.rs` (3), `surface.rs` (2), `accent_rail.rs` (1); every one is a test-module line or a `matches!(…)` guard, **zero** are paint literals |
| Hardcoded ANSI named colors in `widgets/` (non-test) | **34**, all inside `#[cfg(test)]` modules except `code_block.rs:180-188` (4 sites: a fallback syntax palette used when the caller supplies no tokenizer) |
| Hardcoded colors in `patterns/`, `layout/`, `interaction/` | **0** |
| Where literals live at all | `style/mod.rs` (124 — the five presets), `style/quantize.rs` (11), `style/motion.rs` (4), `style/palette.rs` (3), `style/appearance.rs` (1) |

Conclusion: **~99% of the visual surface inherits from central tokens.** A
retheme is a palette swap plus a handful of recipe edits, not 160 file edits.

### 2.2 How the layers compose

```
Role (63 semantic variants)
  └─ RolePalette { roles: [Style; 63] }          ← color authority
       ├─ tailrocks_phosphor()   ← DEFAULT (ANSI-16 named only)
       ├─ terminal_native()      ← phosphor, surfaces = empty style
       ├─ slate() / paper() / ansi() / high_contrast()
       └─ with_role() / merge() / from_fn()

Density (Comfortable|Compact|Dashboard)
  └─ SpacingScale::from_density → {pad_x, pad_y, gap, min_row_height}
       └─ DesignSystem.content_inset(bordered) → ContentInset {x, y}

GlyphSet (Unicode|Ascii|Enhanced)  → GlyphSet::resolve(Glyph) → GlyphResolved
DesignSystem.border_set() → ratatui border::Set  (PLAIN | ROUNDED | ASCII `+-|`)

ColorCapability → quantize_palette(palette, capability)   [quantize-at-edge]
MotionPolicy    → MotionSemantics::animates(policy)
FrameTick       → DesignSystem.at(tick) → elapsed_ms() (ambient phase)
```

Resolution path for a component:

1. `DesignSystem::family_recipe(RecipeFamily)` → `FamilyRecipe`
   (surface role, primary/secondary roles, border role, focus emphasis,
   selection chrome, required `NonColorCue`, `AccentUsage` budget,
   `MotionSemantics`). 7 families: Action, Input, Collection, Overlay, Status,
   Data, Layout (`tokens.rs:972-1059`).
2. Family recipe → a concrete recipe fn:
   - `button_recipe(variant, state)` → `ButtonRecipe` (`tokens.rs:1378`)
   - `input_recipe(state, invalid)` / `_at(..., settled)` → `InputRecipe` (`tokens.rs:1508`)
   - `resolve_list_row(ListRowVisualState)` → `ListRowRecipe` (`tokens.rs:1583`)
   - `panel_recipe(emphasis, elevation)` / `_at(..., settled)` → `PanelRecipe` (`tokens.rs:1304`)
   - `surface_recipe(SurfaceRecipe)` → `SurfacePaintPlan` (`widgets/surface.rs:481`)
3. Recipe → paint. Widgets never call `palette.style(Role::X)` directly for
   chrome — they ask for a recipe. (Direct `style()` calls do happen for
   content roles.)

### 2.3 Theme selection and override

- Built-in packages: `ThemePackage::builtins()` → 6 entries
  `phosphor`, `slate`, `paper`, `ansi`, `high-contrast`, `adaptive`
  (`tokens.rs:812-826`).
- Partial override: `DesignSystem::with_role(Role, Style)` and
  `DesignSystem::merge(&RolePalette)` (skips empty styles → inheritance).
- Per-family focus override: `DesignSystem::with_focus_emphasis(family, cue)`.
- Auto polarity: `Appearance::detect()` → `palette_for_appearance`
  (Light→`paper`, Dark/Unknown→`tailrocks_phosphor`).
- Degrade: `DesignSystem::quantize(cap)` (edge quantization),
  `no_color()` (= `quantize(Monochrome)` + `GlyphSet::Ascii`), `mono()`.
- Runtime switch is a host concern: lookbook binds `Ctrl+Alt+T` to swap
  palettes via `set_system(lookbook_system(...))`; the lone example binds `t`.

### 2.4 The DEFAULT rendered theme — exact construction site

`RolePalette::Default` → `tailrocks_phosphor()` (`style/mod.rs:814-818`).
`DesignSystem::Default` → `phosphor()` (`style/tokens.rs:870-874`), which is
`from_palette(RolePalette::tailrocks_phosphor()).capability(Ansi16).selection(Gutter)`.

**Critical: the default theme is ANSI-16 *named* colors, not RGB.** A test
(`style/mod.rs:907-918`, `phosphor_baseline_uses_named_ansi_only`) fails the
build if any role carries `Color::Rgb` or `Color::Indexed`. The operator's
terminal owns the actual RGB. Exact default role map (`style/mod.rs:345-418`):

| Role | Default |
|---|---|
| `Canvas` | `bg(Color::Reset)` — **terminal-native, not black** |
| `Surface`, `Raised`, `Sunken` | `bg(Ansi16::Black)` |
| `Elevated` | `bg(Color::Reset)` + frame/title marker |
| `Backdrop` | `fg(DarkGray)` |
| `Text` | `fg(Gray)` |
| `TextStrong` | `fg(White).bold()` |
| `TextMuted` | `fg(DarkGray).dim()` |
| `TextDisabled` | `fg(White) + DIM|CROSSED_OUT` |
| `TextFaint` | `fg(DarkGray).italic()` |
| `Border` | `fg(DarkGray)` |
| `BorderFocused` | `fg(LightGreen)` |
| `Selection` | `bg(Green).fg(Black)` (opt-in only) |
| `SelectionTint` | `bg(DarkGray)` |
| `HoverTint` | `bg(Black)` |
| `Focus` | `fg(LightGreen)` |
| `Accent` | `fg(Ansi16::Green)` |
| `Success` | `fg(LightGreen)` |
| `Warning` / `DisclosureHeader` | `fg(Yellow)` |
| `Danger` | `fg(Red).bold()` |
| `Info` / `Link` | `fg(Cyan)` |
| `Input` | `fg(Gray).bg(Black)` |
| `ScrollTrack` / `ScrollThumb` | `fg(Black)` / `fg(DarkGray)` |
| `TabActive` / `TabInactive` | `fg(White).bold()` / `fg(Gray)` |
| `HintKey` / `HintText` / `HintSeparator` | `White+bold` / `Gray` / `DarkGray` |
| `ActionFocused` | `fg(Black).bg(Green).bold()` |
| `ActionDisabled` | `fg(DarkGray).dim()` |
| `StatusBar` | `fg(Gray).bg(Black)` |
| `DiffAdded` / `DiffRemoved` | `fg(LightGreen)` / `fg(Red)` |
| Syntax | Keyword=Magenta, String=LightGreen, Comment=DarkGray, Number=Yellow, Function=Cyan |
| Actors | User=Gray, Assistant=Magenta, Thinking=LightMagenta, Tool=DarkGray, Plan=Yellow, System=Cyan |
| Charts | 1=LightGreen, 2=Cyan, 3=Yellow, 4=Magenta, Axis=Gray, Grid=DarkGray |
| `BackdropWash` | `bg(Black)` |

Other defaults: `Density::Comfortable` (pad 2,1 gap 1),
`MotionPolicy::Full`, `GlyphSet::Unicode`, `SelectionChrome::Gutter`,
`BorderShape::Square`, `ColorCapability::Truecolor` on the enum (but
`DesignSystem::phosphor()` pins `Ansi16`), breakpoints 20/40/80/120,
`KvSeparator::Gutter`.

Truecolor constants exist for web/SVG export only (`style/mod.rs:60-64`):
`PHOSPHOR_GREEN = rgb(0,255,65)`, `PHOSPHOR_DARK = rgb(0,80,18)`,
`PREVIEW_CARD = rgb(28,28,28)`.

---

## 3. Borders / padding / focus / selection today

### 3.1 Shared helpers

| Helper | File | Owns |
|---|---|---|
| `Surface` | `widgets/surface.rs:194` | Fill + border + padding + clip + hit geometry for one region. `plan()` → `SurfacePaintPlan{fill, border, pad_x, pad_y, recipe, family}`. `layout()` → `SurfaceParts{root, content, hit, clip, has_border}`. One Surface per region; no nesting. |
| `SurfaceRecipe` | `widgets/surface.rs:38` | 12 variants: Canvas, Inset (default), Sunken, Raised, Overlay, OverlayFocused, OverlayDanger, Interactive, Focused, Selected, Warning, Destructive. |
| `DesignSystem::border_set()` | `style/tokens.rs:1146` | Single source of corner glyphs: `GlyphSet::Ascii` → `+-|`, else `BorderShape::Rounded` → `ROUNDED`, else `PLAIN` (default). |
| `DesignSystem::content_inset(bordered)` | `style/tokens.rs:1173` | Bordered chrome floors at **1 column** at every density (`pad_x-1`, min 1), insets **0 rows** — the border owns vertical rhythm. Unbordered uses density `pad_x`/`pad_y`. |
| `PanelRecipe` / `panel_recipe_at` | `style/tokens.rs:1304` | Border style (+BOLD when focused), title style, pad, surface fill, `title_prefix` glyph (`Warning` for Danger, `FocusDiamond` for Focused). Cross-fades border on focus change via `blend_role`. |
| `RowChrome` | `widgets/row_chrome.rs:56` | **One selection language for all whole-row collections.** Canonicalizes every historical `SelectionChrome` to gutter+tint. |
| `TableChrome` | `widgets/table_chrome.rs` | Shared header language for the table family. |
| `paint_status_glyph` | `widgets/row_chrome.rs:31` | Status color lives on the glyph cell only; preserves row bg so a selection wash survives. |
| `FocusEmphasis` | `style/tokens.rs:189` | Per-`SurfaceFamily` cue: Container/Field→`BrightBorder`, Row→`FocusTint`, Cell→`Reversed`, Token→`PillGlyph`, Chord→`BoldKey`. |
| `normalize_content_band` | `widgets/surface.rs:449` | Projects caller content onto the capability contract (mono → Reset + BOLD; else Ansi16 quantize). |

### 3.2 Representative widgets

**Button** (`widgets/primitives.rs:396-820`, `Button::paint` at :638)
- Anatomy: single **1-row** band, no box border in the common case.
  `ButtonParts { root, label }` — both the same rect.
- Padding: `self.size.pad_cols()` spaces on each side (`" ".repeat(pad)`);
  `ButtonSize::Compact` = 1, `Normal` = 2. Recipe floor is
  `pad_x = spacing.pad_x.max(1)` (`tokens.rs:1502`).
- Variants: Primary, Secondary, Quiet, Outline, Destructive, Link, Success,
  Command. Recipe maps Primary→`Role::ActionFocused` fill, Destructive→
  `Role::Danger` border, Link→`Role::Link`.
- Non-color cues: Primary/Success/Command add `BOLD` (+`REVERSED` on focus);
  Link adds `UNDERLINED`; Command prefixes `›` (`>` in mono); Destructive
  prefixes `!` in mono; icon-only without a11y label paints `⚠`/`!` in Danger.
- States: Focused → `BorderFocused` + BOLD border, bordered forced on (unless
  Quiet/Link); Hovered → `HoverTint` wash, or `palette::lift()` on a filled
  control; Pressed → `SelectionTint` fill + `BOLD|REVERSED`; Disabled →
  `ActionDisabled`; Loading → `DIM` + leading `Glyph::Loading`.
- No corners, no rounding — the button is a styled string, not a bordered box.

**List** (`widgets/list.rs`, paint at :1049, row projection at :1150)
- Calls `system.resolve_list_row(visual_state)` **and**
  `RowChrome::resolve(system, visual_state)`.
- Default (`SelectionChrome::Gutter`): selected row = **leading gutter glyph**
  (`Glyph::SelectionGutter` = `▌`) + `Role::TextStrong` label (bold), **no
  background wash**. Gutter tone = `Role::Accent` when the list owns focus,
  `Role::TextMuted` when parked (`tokens.rs:1615-1628`).
- `RowChrome::resolve` **ignores** a theme's `SelectionChrome::Fill`/`Marker`
  and canonicalizes to gutter+tint (`row_chrome.rs:66-89`) — enforced by test
  `configured_fill_cannot_replace_collection_gutter_and_tint`.
- Gutter slot is **always reserved** (`show_gutter_slot: true` for all four
  chrome variants) so rows never shift when selection moves.
- Hover: `Role::HoverTint` wash + `TextStrong` label — explicitly never link
  styling (`tokens.rs:1690-1695`).
- Padding: `recipe.pad_x = spacing.pad_x` (2 at Comfortable).

**Table** (`widgets/table.rs`, paint at :886, cell render at :1137, chrome at :1268/:1360)
- Header language from `table_chrome.rs`; header cells use `Role::TextMuted`.
- Grid/border lines painted with `Role::Border`; the focused table uses
  `Role::BorderFocused` on its own frame, never a per-row border.
- Cell cursor = **`FocusEmphasis::Reversed`** (one reversed cell), per
  `SurfaceFamily::Cell`.
- Row selection goes through the same `RowChrome` (`table.rs:1036`) →
  gutter + `SelectionTint`, same as List.
- Sort/disabled markers: `Role::Accent` for active sort, `Role::TextDisabled`
  for disabled cells.

**Tabs** (`widgets/tabs.rs:836-858`)
- `TabsActiveCue::{AccentPill, Connected, Marker, Rule}`, **default `Rule`**:
  selection wash + a semantic border row under the active label, using
  `Role::Accent` while the strip owns focus and `Role::Border` otherwise.
  No underline anywhere (enforced by `design_gate::tab_palette_roles_are_underline_free`).

---

## 4. Examples / demo surface

### 4.1 `crates/termrock-showcase` — flagship app, not a gallery
Files: `lib.rs`, `app.rs` (660), `demo_runtime.rs` (524), `model.rs` (239),
`main.rs` (38), `tests/scenes.rs` (217). **No `stories.rs`.**

Scenes = `Scenario` enum, 7 variants (`demo_runtime.rs:113-132`), cycled with
`^n`: `hello-stream`, `tool-run`, `permission-high`, `plan-build`,
`diff-review`, `question`, `multi-subagent`. Keys: Enter submit, `^n` next,
Esc peel layer, `^q` quit.

Theme: **hardcoded at boot** — `DesignSystem::from_palette(RolePalette::tailrocks_phosphor())`
(`app.rs:79`). No picker, no flag, no runtime swap. A `DesignSystem` is
threaded into every widget (`Transcript::new(&blocks, &self.system)`, etc.).

`tests/scenes.rs` asserts design law on painted buffers: ≤3 accent rows/scene,
≤2 long focused-border runs, renders at 120x32 / 80x24 / 40x16 / 20x5, no
banned private-API imports.

### 4.2 `crates/termrock-lookbook` — the component catalog
**1090 stories** across 239 id prefixes; 954 passive + 136 mounted. All
declared inline in `src/stories.rs` (~27k lines, `stories()` at :900-10716).
`Story { id, title, identity, description, width, height, spec }`; spec is
`Fixture(FixturePaint)` or `Mounted(Box<dyn StoryInteraction>)`.

- `src/interactors.rs` (4460 LOC) + `src/interactors/{applications,catalog,composites,extended,remaining,viewers,workflows}.rs` — 70+ interactors.
- `src/design.rs:20` — `lookbook_system(theme: RolePalette) -> DesignSystem`: takes `DesignSystem::phosphor()`'s shape and swaps only the palette. **Every story gets a `DesignSystem`.**
- Host `src/app.rs:577` starts on `RolePalette::default()`, `Ctrl+Alt+T` toggles default ⇄ slate ⇄ phosphor.

CLI (`src/main.rs:23`):
```
termrock-lookbook <terminal|list|inventory|render|render-png|check|frame|export-posters>
  terminal [--story <id>]                       full TUI gallery
  list [--format json]                          id/title or demo catalog JSON
  inventory --format json                       typed public-UI + pattern inventory
  render [--theme phosphor|slate] --out <dir>   SVG per story
  render-png --out <dir>                        Jackin-subset PNGs
  check --dir <dir>                             verify committed SVGs current
  frame --story <id> [--cols N] [--rows N] [--keys k1,k2]   one frame as JSON
  export-posters --out <dir> --story <id> ...   one JSON poster per story
```

### 4.3 `crates/termrock-lookbook-web` — WASM host
`src/lib.rs` (137 LOC), `crate-type = ["cdylib","rlib"]`. Mounts persistent
`DemoSession`s from `termrock_lookbook::demo` in a `thread_local` store and
serializes truecolor frames to JSON. Exports: `catalog_json`, `mount_demo`,
`dispatch_demo`, `demo_frame`, `reset_demo`, `unmount_demo`.

### 4.4 `crates/termrock-cli` — registry + doctor, **not a renderer**
Binary name is `termrock`. Usage (`src/main.rs:14-31`):
```
termrock doctor [--profile modern|compatible|minimal|inline|headless]
termrock contract list
termrock contract check
termrock plan  <entry-dir> [--workspace DIR]
termrock add   <entry-dir> [--workspace DIR] [--force]
termrock diff  <entry-dir> [--workspace DIR]
termrock check <entry-dir>
```
`src/lib.rs` (951 LOC) holds `RegistryFile`/`InstallPlan`/`plan_install`/
`apply_plan`/`diff_installed`/`sha256_hex`. `tests/install_blocks_compile.rs`
installs 6 registry fixtures into a temp consumer crate and runs `cargo check`.

### 4.5 `crates/termrock-raster` — PNG only
Backend: `tiny-skia` (pixel surface) + `swash` 0.2.10 (outline rendering).
Public API (`src/lib.rs:14-24`): `render_pixmap`, `render_png`,
`compare_png_pixels`, `PixelDiff`. Metrics `CELL_WIDTH_PX=9`,
`CELL_HEIGHT_PX=18`, `FONT_SIZE_PX=14.0`, `BASELINE_PX=14`.
Fonts: vendored `JetBrainsMono-{Regular,Bold,Italic}.ttf` `include_bytes!`'d in
`src/fonts.rs:7-9`, hashes pinned by test. Modifiers: REVERSED swaps fg/bg,
DIM scales to 60%, UNDERLINED draws rows 15-16, CROSSED_OUT rows 8-9; wide
graphemes span 2 cells. **No SVG, no text output.**

### 4.6 `crates/termrock/examples/`
Exactly one: `showcase.rs` (211 LOC), `required-features = ["crossterm"]`.
Session lifecycle, `Keymap` static bindings, Tabs+Panel+List+HintBar+StatusBar+
Toast composition, theme toggle on `t` between `RolePalette::default()` and
`slate()`, list hover/click, `bottom_rows`.

---

## 5. Verification infrastructure

### 5.1 mise tasks (`mise.toml`) — the real gates

```
mise run check   # docs-quality + fmt + clippy -D warnings + nextest workspace
mise run test    # cargo nextest run --workspace --all-features --locked
mise run lint    mise run fmt
mise run gate    # check + preview-goldens + png-baselines + no-default-features
                 # + examples + crossterm + wasm32 + rustdoc -D warnings
                 # + cargo-public-api diff vs docs/api/public-api.txt
                 # + cargo hack --feature-powerset + cargo deny + cargo shear
                 # + cargo package + docs preview-posters + docs build
mise run preview-goldens   # nextest -p termrock-lookbook --test goldens
mise run bless-previews    # TERMROCK_BLESS_PREVIEWS=1 ...
mise run png-baselines     # nextest -p termrock-lookbook --all-features --test png_baselines
mise run bless-pngs        # TERMROCK_BLESS_PNGS=1 ... --no-capture
```

### 5.2 Test targets

| Command | What |
|---|---|
| `cargo test -p termrock --test design_gate` | 41 mechanical design-law gates (see 5.3) |
| `cargo test -p termrock --test capability_pty` | capability resolution (no real PTY) |
| `cargo nextest run -p termrock-lookbook --all-features --test goldens --locked` | 15 text cell-dump goldens |
| `cargo nextest run -p termrock-lookbook --all-features --test png_baselines --locked` | 123 PNG pixel-exact baselines |
| `cargo test -p termrock-lookbook --all-features --test design_gate` | every public identity paints its `NonColorCue` without color |

Other `crates/termrock/tests/`: `detail_table.rs`, `detail_table_hot_path.rs`,
`direction_research_brief.rs`, `documentation_examples.rs`, `form.rs`,
`generic_state_defaults.rs`, `input_adapter.rs`, `log_pane_hot_path.rs`,
`picker_hot_path.rs`, `split_pane.rs`, `table_hot_path.rs`,
`text_area_hot_path.rs`, `tree.rs`, `tree_hot_path.rs`, `viewport_hot_path.rs`.

### 5.3 `crates/termrock/tests/design_gate.rs` (~83 KB, 41 tests) — the important one

Two mechanisms, both **cell/text based — no PTY, no pixels, no PNG**:

1. **Static source scan** of every `.rs` under `src/widgets/` and
   `src/patterns/`, truncated at the first `#[cfg(test)]`, comments stripped,
   with a `payload_mask` exemption for `example_*` payload literals. Gates:
   `no_bare_ellipsis_in_paint`, `one_overflow_note`, `one_chord_notation`,
   `pattern_hint_copy_budget`, `gates_detect_their_own_violations`.
   Rules come from `docs/design/web-premium-tui-law.md` §4.1.

2. **Painted-buffer checks** via `painted()` and `priority_pattern_frames()`:
   `motion_policy_off_is_static`, `motion_policy_full_actually_animates`,
   `spinner_frames_one_column`, `a_focused_field_says_so`,
   `collections_share_one_gutter_glyph`, `interaction_underline_is_dead`,
   `tab_palette_roles_are_underline_free`,
   `selection_chrome_is_not_overridden_in_widget_paint`,
   `no_widget_paints_selection_fill_by_default`, `pattern_style_diversity`,
   `pattern_hint_budget`, `text_never_touches_borders`,
   `bordered_overlays_reserve_their_gutters`, `one_scrollbar_language`,
   `a_scrolled_region_says_it_continues`, `truncation_has_ellipsis`,
   `flagship_widgets_survive_tiny_and_random_geometry` (deterministic `lcg`
   fuzz, 1200 iterations).

**This is the harness that will fight a token change.** Any redesign that
alters gutter glyphs, adds selection fills, or touches border/underline law
must update these assertions in the same change.

### 5.4 `crates/termrock-lookbook/tests/design_gate.rs`
One test, `every_public_ui_representative_paints_its_recipe_cue_without_color`.
For every `public_ui_inventory()` entry it renders the representative story
twice with `lookbook_system(RolePalette::default()).no_color()` (second pass
tags every role `CROSSED_OUT` as a probe), then asserts the family's enforced
`NonColorCue` is visible in real cells and that the widget consumed the
supplied `DesignSystem` rather than hard-coding style.

### 5.5 `crates/termrock/tests/capability_pty.rs` (74 LOC)
**Does not spawn a PTY.** Injects `EnvHints::fixture(...)` /
`EnvHints::fixture_ssh_tmux()` into `TerminalCapabilities::with_hints`. 6
tests: `no_color_forces_monochrome_boundary`, `dumb_term_resolves_minimal_without_preferred`,
`ssh_tmux_prefers_compatible`, `profile_override_beats_ssh_auto`,
`all_profiles_have_session_and_boundary`,
`color_override_wins_over_no_color_env_when_explicit`.

### 5.6 Pixel diff
`termrock_raster::compare_png_pixels` (`crates/termrock-raster/src/compare.rs:41`)
— decodes both with `tiny_skia::Pixmap::decode_png`, checks dimensions, scans
RGBA row-major, returns `PixelDiff::FirstDifference{x,y,a,b}`. **Zero
tolerance, no fuzzy threshold, no HTML report, no `png_diff` binary.** Used
only by `crates/termrock-lookbook/tests/png_baselines.rs`, which also renders
twice in-process and treats any double-render mismatch as a pipeline bug
("do NOT resolve it by blessing", line 43-46).

Baselines: **123 committed PNGs** at
`crates/termrock-lookbook/baselines/png/`, named
`{story-id with / → -}.png` (`story_png_filename`, `lookbook/src/png.rs:52`).
Subset = `JACKIN_SUBSET_COMPONENTS` — 17 families (`png.rs:19-37`): ActionBar,
Backdrop, ChoiceDialog, DetailTable, Dialog, DiffView, HintBar, List,
MessageDialog, Panel, ProgressBar, StatusBar, Tabs, TerminalCellGrid,
TextInput, Toast, Viewport. Test asserts `subset.len() >= 87`.

Goldens: **15 text dumps** at `crates/termrock-lookbook/goldens/`
(`list__selection.txt`, `table__basic.txt`, `tabs__status.txt`,
`dialog__destructive.txt`, `form__validation.txt`, `toast__stack.txt`,
`transcript__basic.txt`, `prompt-composer__basic.txt`,
`metrics-dashboard__basic.txt`, `tool-call-card__permission.txt`,
`command-palette__basic.txt`, `sidebar__settings.txt`, `status-bar__basic.txt`,
`quick-open__basic.txt`, `setup-wizard__capability.txt`). Format
(`render_golden`, `goldens.rs:51`): header `"<id> <cols>x<rows> (story <w>x<h>)"`
then one plain-text row per terminal row, **chars only, no colors**. Coverage
floor `covered >= 8`.

Design rationale for the PNG gate lives in `research/tui-png-baselines/`
(6-chapter dossier, not code): compare decoded pixels not PNG bytes, plain git
not LFS, workspace test not a new workflow, cross-arch bit-identity already
measured for aarch64 vs x86_64/Rosetta; macOS↔Linux is open ledger assumption A3.

### 5.7 Docs-side checks
`docs/scripts/` (21 TS tools, `bun run`): `generate-catalog.ts --check`
(regenerates `docs/src/generated/catalog.ts` from the Rust registry, fails on
drift), `check-contracts.ts` (v2 contract schema),
`audit-render-contracts.ts`, `check-component-pages.ts` (193 component pages),
`check-building-block-boundary.ts` (product-noun composites must not be
`termrock::widgets` pub-uses), `check-preview-metrics.ts`,
`export-preview-posters.ts --check`, `check-static-site.ts`, and more.

**Posters:** the recent commits (`8ca7ebb2`, `57f2fe43`, `f51d0ba8`) refer to
`docs/public/preview-posters/*.json` produced by
`docs/scripts/export-preview-posters.ts` (`bun run build:preview-posters`,
`--check` mode) and by `termrock-lookbook export-posters`. These are **JSON
frame data**, not PNG.

Playwright: `docs/playwright.config.ts` sets `screenshot: 'only-on-failure'`
with **no `toMatchSnapshot`/`toHaveScreenshot` config** — `docs/tests/visual/previews.spec.ts`
asserts DOM attributes and paint metrics, not pixels. Run `bun run test:browser`
/ `test:visual` / `test:preview`.

CI: `.github/workflows/ci.yml` is generated and only delegates to pinned
`velnor-actions` reusable workflows — the actual cargo commands are the mise
tasks above. `.github/workflows/docs.yml` does a render-twice `diff -r`
SVG self-consistency check (:115-118) plus a slate-theme render (:119).

---

## 6. Widget classification vs a junie-style reference

Reference bucket (a) = the ~24-widget junie set: button, chips, select, input,
textarea, list, table, tabs, tree, dialog, picker, progress, scrollbar, panel,
segments, code, completion, keyhint/hint, empty, grid.

### (a) Likely direct match — port 1:1 (34)

| TermRock widget | Why it maps |
|---|---|
| `Button` (+`IconButton`) | Same control; junie primary/secondary/ghost ≈ `ButtonVariant::{Primary,Secondary,Quiet}`. |
| `ButtonGroup` | Grouped actions ≈ junie button row. |
| `Tag` / `Chip` (`tag_chip.rs`) | junie "chips" = Tag/Chip pill grammar. |
| `Badge` | Compact status chip ≈ junie chip variant. |
| `Select` | Single-choice opener + list. |
| `MultiSelect` | Multiple-choice with chip summary. |
| `Combobox` | Editable input + suggestion list. |
| `TextInput` | Single-line field. |
| `NumberInput` | Numeric field — superset, subsettable. |
| `PasswordInput` | Secret field — superset. |
| `SearchInput` | Search field — superset. |
| `PathInput` | Path field — superset. |
| `TextArea` | Multi-line editor. |
| `List` | Flat collection. |
| `VirtualList` | List at scale — same row grammar. |
| `Table` | Columnar. |
| `DataTable` | Interactive table — superset. |
| `DetailTable` / `KeyValueTable` / `KeyValueList` | Key/value detail — maps to a junie key-value surface if present. |
| `Tabs` | Tab strip. |
| `SegmentedControl` | junie "segments". |
| `Tree` / `TreeTable` / `TreeNavigation` / `FileTree` | junie "tree". |
| `Dialog` / `AlertDialog` / `ChoiceDialog` | junie "dialog". |
| `Picker` / `QuickOpen` / `HistoryPicker` | junie "picker". |
| `ProgressBar` | junie "progress". |
| `ScrollArea` + `scroll/` | junie "scrollbar". |
| `Panel` / `Card` / `Surface` | junie "panel". |
| `CodeBlock` | junie "code". |
| `CompletionMenu` / `SlashCommandMenu` | junie "completion". |
| `HintBar` / `Kbd` / `ShortcutHint` / `KeyboardHelp` | junie "keyhint/hint". |
| `EmptyState` | junie "empty". |
| `Grid` (`layout/grid.rs`) + `VirtualGrid` | junie "grid". |
| `Separator` | Structural divider present in every TUI kit. |
| `Spinner` | Universal activity affordance. |

Justification: these all have a 1:1 junie analogue in **name and role**; the
work is retokenizing, not restructuring. Every one already routes through
`DesignSystem`, so the port is a palette/recipe change plus an anatomy
comparison.

### (b) Partial match — right idea, different anatomy or extra state (26)

| Widget | Gap vs a junie-scale reference |
|---|---|
| `Toggle` / `ToggleGroup`, `Checkbox`/`RadioGroup` (in `controls.rs`) | junie may have a simple switch; TermRock's carry group/roving semantics + rich glyph ladders. |
| `Slider` / `RangeSlider` | Often absent from minimal TUI kits; needs sub-cell ramp re-derivation. |
| `Select` popup chrome | Uses `OverlayStack` + `Elevated` — junie popups are usually a simpler list box. |
| `CommandPalette` | Flagship, 2286 LOC — a junie "picker" is a strict subset. |
| `PromptComposer` | Agent-domain; no junie analogue, but built from input + hint + list. |
| `PromptQueue`, `QuestionFlow`, `FormWizard`, `Form` | Multi-step/multi-field composites. |
| `Tooltip` / `Popover` / `Drawer` | junie dialogs only; these need elevation/backdrop work. |
| `Toast` / `NotificationCenter` | Transient layers with lifecycle. |
| `MarkdownView` / `StreamingMarkdown` | Rich text — junie kits usually ship plain text. |
| `DiffView` / `DiffReview` | Two-axis diff renderer. |
| `MenuBar` / `DropdownMenu` / `ContextMenu` | Nested menus with mnemonics. |
| `Sidebar` / `Breadcrumbs` | Shell navigation. |
| `StatusBar` / `StatusStrip` / `ChromeRow` / `TieredRow` / `ComposedRow` | Composed-row family — same job, different part model. |
| `Skeleton` / `LoadingOverlay` | Async placeholders. |
| `Pagination` / `Stepper` / `ProgressSteps` | Paged/stepped chrome. |
| `Callout` / `Alert` / `FieldMessage` / `ErrorState` / `Diagnostic` | Feedback family — richer than a minimal kit's "alert". |

Justification: the role exists in a junie-style system but TermRock's version
carries extra state machines, overlays, or streaming concerns. Retokenize
first; then decide whether to trim state.

### (c) No reference — must be re-derived from tokens/patterns (large remainder)

Agent/AI domain: `Transcript`, `MessageThread`, `ToolCallCard`, `ToolCard`,
`ThinkingBlock`, `TokenMeter`, `ModeRibbon`, `ModelSelector`,
`AgentModeSelector`, `PermissionPrompt`, `SlashCommandMenu`, `Mention`
(`FileMention`/`EntityMention`), `ContextMeter`, `CheckpointTimeline`,
`PromptQueue`, `SubagentCard`, `AgentStatusHeader` (pattern), `TaskRail`
(pattern).

Data/observability domain: `Sparkline`, `Chart` (line/area), `Gauge`,
`Histogram`, `BarSeries`, `MetricRadar`, `MetricTile`, `TraceWaterfall`,
`DependencyGraph`, `EventStream`, `LogStream`, `LogPane`, `HexViewer`,
`TerminalOutput`, `TerminalCellGrid`, `Viewport`, `ObjectInspector`,
`SearchResults`, `ResultGrid`, `ProcessTable`, `DataView`.

Identity/meta: `AvatarGlyph`, `Identity`, `SourceCitation`/`CitationList`,
`Icon`, `JumpOverlay`, `FocusLens`, `SemanticZoomBadge`, `DesignInspector`,
`ThemePicker`, `KeybindingRecorder`, `ConnectionManager` (pattern),
`Offline*`/`Connectivity`.

Layout/shell: `WorkSurface`, `Workspace`, `Stack`/`Inline`, `Center`,
`SplitPane`, `ResizablePanelGroup`, `PanelStack`, `Carousel`, `Accordion`,
`Collapsible`, `Section`, `Heading`/`Paragraph` (`content.rs`), `Text`,
`Label`/`Description`, `HighlightedText`, `AnsiText`, `ImageSurface`,
`PreviewCard`, `FullscreenViewer`, `Virtualizer`, `OverlayStack`, `Backdrop`,
`FocusGraph`, `SemanticScene`, `RovingFocusGroup`, `DismissableLayer`,
`InteractionScene`, `UiContext`.

Plus all 35 `patterns/*` application composites — no junie reference exists
for a connection manager or an observability dashboard; these must be
re-derived from whatever tokens the new system lands on.

**Counts: (a) 34 direct, (b) 26 partial, (c) ~150 identities + 35 patterns.**
The (c) bucket is large because TermRock is a 210-identity kernel, not a
24-widget kit — but almost all of it inherits from the same `DesignSystem`, so
deriving (c) is mostly "re-resolve the same recipes under new tokens."

---

## 7. Design-language docs

### 7.1 Which docs exist and who wins

`docs/design/` has 87 flat `.md` files. Declared precedence
(`termrock-design-language.md:5`):

- **`termrock-design-language.md`** (569 lines, 2026-08-14) — *binding SoT for
  interaction styling and focus grammar*. Wins on focus/selection/active/underline.
- **`terminal-design-system.md`** (894 lines) — SoT for token taxonomy.
- **`phosphor-obsidian-visual-direction.md`** — SoT for phosphor palette values.
- `component-visual-richness-plan.md` — implementation waves.
- `web-premium-tui-law.md` — source of the `design_gate.rs` §4.1 rules.

### 7.2 Current canonical language, compressed

1. **Accent budget:** ≤2 accent-forward regions per viewport. Everything else
   graphite and muted text.
2. **Depth before borders:** surface ladder Canvas → Surface → Raised →
   Elevated → Sunken; each rung one step apart. Borders are structure, not the
   only containment signal.
3. **One scale:** `Density` drives padding, gap, corner shape, glyph weight.
   No off-scale paddings.
4. **Hierarchy steps down:** Strong → body → muted → faint, always. Secondary
   metadata never the same white as primary.
5. **Selection ≠ focus, two different marks:**
   - selection = `▌` gutter + `SelectionTint` + bold `FgStrong` (calm)
   - focus = the owner's bright `BorderFocused` + the row gutter (precise)
6. **Color is the last channel.** Glyph + weight + word + position first;
   hue reinforces. Must survive mono / `NO_COLOR` / narrow / SSH.
7. **Borders single-line only.** Inactive = quiet gray; the one focused owner
   = phosphor. Never double-line, never per-row. Shape is a theme token
   (`BorderShape::Square` default = phosphor identity; `Rounded` for
   Grok-Build-class consumers; `+` for ASCII).
8. **Motion is status.** Alive when working, static when idle; reduced-motion
   collapses to static accents with zero information loss.
9. **Underline-free focus grammar (binding, §5):** five cues — bright border
   (container), gutter+tint (row), sunken well + bright border + `›` prompt
   (field), block/reverse cell (cursor), bold label + non-line marker (active
   option). Underline is allowed **only** for: monochrome hyperlinks, faithful
   content rendering (ANSI SGR-4, OSC-8, markdown fallback, diff), and cursor
   fallback where reverse is unavailable.
10. **Glyph catalog:** Lucide-named, width-tested to exactly 1 column, per-glyph
    ASCII fallback. Status dots `●○◉◎`, diamonds `◆◇◈`, checks `✓✗`,
    disclosure `▸▾/›‹`, sub-cell ramp ` ▁▂▃▄▅▆▇█`, braille + dot-pulse spinners.
    Glyphs never the sole carrier of meaning.
11. **Phosphor identity stays** (principle 10): green-on-obsidian, square caps,
    single-line borders, focus-by-color. Neutrality means others can retheme
    fully, not that the default goes bland.

Truecolor targets (`terminal-design-system.md` §9):
Canvas `Reset` else `#0a0c0a`; Surface `#121612`; Raised `#1a1f1a`; Elevated
`#1e2620`; Sunken `#0d100d`; Border `#2a332c`; BorderFocused `#00ff41`; Fg
`#d6e0d6`; FgStrong `#f0f5f0`; FgMuted `#7a8a7a`; FgFaint `#4a574a`; FgDisabled
`#3a453a`; Accent `#00ff41`; AccentMuted `#009928`; SelectionTint `#14331a`;
HoverTint `#1a221c`; Success `#5dffa0`; Warning `#f0c040`; Danger `#ff5e7a`;
Info `#5ec8ff`.

Runtime authority, however, is **named ANSI-16** (`phosphor-obsidian` §4):
Canvas `Reset`, Surface/Raised/Sunken `Black`, Elevated `Reset`+marker, Border
`DarkGray`, BorderFocused `LightGreen`+bold, Fg `Gray`, FgStrong `White`+bold,
FgMuted `DarkGray`+dim, FgFaint `DarkGray`+italic, SelectionTint `DarkGray`,
Selection gutter `▌`+`Green`, Hover `Black` wash, Danger `Red`, Warning
`Yellow`, Info `Cyan`, Success `LightGreen`.

### 7.3 Conflicts with an exact-token single dark theme (primary `#48e054`, black canvas, `#111` surfaces)

| # | junie token | Current canonical language | Conflict severity |
|---|---|---|---|
| 1 | **Canvas = pure black** | `Canvas = Color::Reset` (terminal-native). Design law prefers Reset, with `#0a0c0a` only as a truecolor fallback. A test asserts the dark canvas bg is `Reset`. | **Direct.** Painting literal black changes `style/mod.rs:349`, breaks `default_separates_ordinary_and_strong_text`-adjacent assumptions, and `Appearance::palette_for_appearance` dark branch, and `light_appearance_is_actually_light`. |
| 2 | **Primary `#48e054`** | Phosphor identity is `#00ff41` (truecolor) / `Ansi16::Green` (runtime). Two docs and one const pin it: `palette.rs:68`, `terminal-design-system.md` §9, `phosphor-obsidian` §4. | **Direct.** Also `PHOSPHOR_GREEN` is used by the SVG/poster export path, so the docs site and PNG baselines shift with it. |
| 3 | **Surfaces `#111`** | Surface ladder has **five rungs** (`#121612` / `#1a1f1a` / `#1e2620` / `#0d100d`) in truecolor docs, all collapsed to `Black` at runtime ANSI-16. A single `#111` surface erases the ladder. | **Structural.** The ladder is a load-bearing principle (§1.2 "Depth, not boxes"); `phosphor_surfaces_keep_semantic_elevation` and `surface.rs` tests assert Raised ≠ Surface ≠ Elevated ≠ Sunken. Collapsing to one surface value requires either a different mechanism for depth (border/inset) or deleting the ladder. |
| 4 | **Rounded borders** | `BorderShape::Square` is the default and is called the "phosphor identity default"; `Rounded` exists as an *alternate product theme* (`tokens.rs:447-453`). | **Trivial.** `DesignSystem::border_shape(Rounded)` already works and routes through `border_set()`. This is a one-token flip — but it is a *default* change, so every PNG baseline and every `design_gate` text golden with corners shifts. |
| 5 | **Exact tokens = single theme** | The codebase ships **6 preset packages** and an appearance-driven auto-mapper (dark→phosphor, light→paper). `phosphor_preset_pins_load_bearing_role_values` and `slate_preset_pins_load_bearing_role_values` pin exact styles. | **Architectural.** Locking to one exact theme means deprecating `paper`, `slate`, `ansi`, `high-contrast`, `adaptive` as *defaults* (they can stay as retheme proof, per principle 10) and removing the `Appearance` auto-mapper, or re-pointing `AppearanceThemeMap::{dark,light}` at the single theme. |
| 6 | **Exact RGB as runtime paint** | Runtime authority is deliberately **named ANSI-16 only**; `phosphor_baseline_uses_named_ansi_only` fails the build on any `Color::Rgb` in a role, and `faded_named_ansi_stays_in_named_terminal_space` guards the fade path. The docs are explicit: truecolor is "web/SVG export swatches, never TUI paint authority." | **The hardest conflict.** Adopting `#48e054`/`#111` as *runtime* values means either (a) relaxing that gate and accepting that 16-color terminals get a quantized approximation, or (b) keeping the ANSI-16 baseline and using the exact tokens only for Truecolor capability — which `ColorCapability` already supports (`quantize_at_edge`). |
| 7 | **Accent budget ≤2** | junie's primary-green-everywhere tendency (primary buttons, selected rows, focus ring, scrollbar thumb all the same green) is exactly what this doc calls "neon CRT cosplay" (§0) and forbids. | **Direct.** `Role::Accent`, `Role::Focus`, `Role::Success`, `Role::BorderFocused`, `Role::ScrollThumb`, `Role::HintText`, `Role::ChartSeries1` are deliberately **distinct**; `accents_are_distinct` asserts `ScrollThumb`, `DiffAdded`, `TabActive`, `Border` do **not** paint the brand accent. A single-accent theme must re-derive these or weaken that test. |
| 8 | **Selection = full-row fill** (common junie pattern) | `SelectionChrome::Gutter` is the shipped default; `Fill` and `Marker` are documented as "opt-in only; never a default", and `RowChrome` **ignores** a theme's `Fill`/`Marker` entirely. `no_widget_paints_selection_fill_by_default` is a design gate. | **Direct.** Deriving a junie-style filled selection requires either changing `RowChrome::resolve`'s canonicalization or removing that gate. |
| 9 | **`#48e054` luminance** | `contrast_floor.rs` measures every painted fg/bg pair against documented floors (body ≥7.0, secondary ≥4.5, borders ≥1.3) with an exact `KNOWN_SHORTFALLS` list that may only shrink. | **Gate.** New token values must clear the floors or be added to `KNOWN_SHORTFALLS` with a plan — and any pair that starts passing must be removed from the list. |
| 10 | **Docs + baselines are downstream** | 193 component pages, `docs/src/generated/catalog.ts` (generated), 15 text goldens, 123 PNG baselines all encode the current look. | **Cost.** `mise run gate` regenerates and diffs all of it; `generate-catalog.ts --check` fails on drift. A token change is a mass-bless event by construction. |

**Net:** the architecture is fully ready for a retheme — the chokepoint is
real and near-total. The blockers are *policy*, not plumbing: the
ANSI-16-only runtime law (#6), the five-rung surface ladder (#3), the
accent-de-collapse rule (#7), and the gutter-not-fill selection default (#8).
Each is enforced by a named test, so each is a deliberate, reviewable
decision rather than a refactor.
