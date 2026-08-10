# TermRock component inventory

The public widget set is derived from the reviewed API report and currently contains `Accordion`, `ActionBar`, `ActionLink`, `AvatarGlyph`, `AnsiText`, `Backdrop`, `Badge`, `Banner`, `BarSeries`, `Button`, `ButtonGroup`, `Callout`, `Card`, `Checkbox`, `Chip`, `ChoiceDialog`, `CodeBlock`, `Collapsible`, `CommandPalette`, `Combobox`, `Autocomplete`, `CompletionMenu`, `DataTable`, `DesignInspector`, `Description`, `DetailTable`, `Dialog`, `DiffView`, `Drawer`, `EmptyState`, `ErrorView`, `Field`, `Fieldset`, `FieldCaption`, `Form`, `FormWizard`, `Heading`, `HighlightedText`, `HistoryPicker`, `HintBar`, `Icon`, `IconButton`, `Identity`, `ImageSurface`, `JumpOverlay`, `JumpMode`, `FocusLens`, `Kbd`, `KeyValueList`, `Label`, `Link`, `List`, `LoadingView`, `LogPane`, `MarkdownView`, `Menu`, `MenuBar`, `MessageDialog`, `ModeRibbon`, `Panel`, `Paragraph`, `PasswordInput`, `NumberInput`, `SearchInput`, `PathInput`, `TokenField`, `Select`, `MultiSelect`, `PermissionPrompt`, `Picker`, `PlanReview`, `Popover`, `Progress`, `PromptComposer`, `QuickOpen`, `ResizablePanelGroup`, `QuestionFlow`, `RangeSlider`, `ScrollArea`, `Section`, `SegmentedControl`, `SegmentedMeter`, `Separator`, `SessionPicker`, `ShortcutHint`, `Slider`, `Skeleton`, `Sparkline`, `SplitPane`, `Stepper`, `StatusBar`, `Surface`, `Table`, `Tabs`, `Tag`, `TaskRail`, `Text`, `TextArea`, `TextInput`, `ThemePicker`, `ThinkingBlock`, `Timeline`, `Toast`, `Toggle`, `ToggleGroup`, `TokenStrip`, `Toolbar`, `TokenMeter`, `ToolCard`, `Transcript`, `Tree`, `Viewport`, and `VirtualGrid`.

`ScrollArea` / `ScrollAreaState` is the canonical scrolling primitive: dual-axis
offsets, wheel/page/intents, scrollbar chrome, follow-tail with paused unseen
indicator, scroll anchors, nested `ScrollChain` policy, and `VisibleRange` for
virtualization. Specialized `scroll::TailScroll` / `DialogScroll` helpers remain
for buffer-local and dialog dual-axis math; new surfaces should prefer
`ScrollAreaState`.

`Virtualizer` / `Virtualizer2D` is the canonical large-collection window engine:
fixed or sparse variable extents, overscan, sticky regions, anchors, and
semantic budgets that never allocate O(logical_len). `VirtualWindow` remains the
fixed unit-slot facade for DataTable; `VirtualGridState` embeds `Virtualizer2D`.

With the optional `crossterm` feature, `Session` is the sole terminal lifecycle
owner. Its forward default acquires raw mode, alternate screen, mouse capture,
bracketed paste, disabled line wrapping, and hidden cursor state. Failed entry
rolls back every acquired mode; explicit restore and `Drop` restore in reverse
order and remain idempotent. Cursor hiding and line-wrap disabling are
independent options that default on; inline integrations may enable either
without owning the alternate screen, while alternate-screen integrations may
disable either. Screens and widgets never emit lifecycle commands.

`SplitPane` maps an integer remembered ratio, horizontal/vertical direction,
and caller minimums into bounded first/divider/second rectangles. Tiny areas
degrade proportionally without escaping the input rectangle. `SplitPaneState`
owns ratio, divider focus/hover/drag, collapse side, and last painted geometry;
render alone publishes direction-tagged pointer hit geometry. Keyboard resize
and pointer methods emit semantic ratio/focus outcomes; explicit
`collapse`/`expand` methods preserve the remembered ratio. The caller maps
collapse bindings and owns pane content, persistence, focus routing, and
collapse policy.

`Form` consumes caller-owned borrowed sections and stable-ID fields. It renders
required, disabled, help, and validation-error states in responsive one/two
column layouts. `FormState` owns only active focus, hover, viewport offset,
column count, and painted field/scrollbar geometry. Partially clipped fields
retain a union hit region plus optional visible label/value/support subregions.
Required and disabled states reserve the neutral non-color markers `*` and `⊘`.
Keyboard, click, wheel, and scrollbar-position methods expose semantic
focus/activation or bounded scroll; callers retain field values, wording,
editing, validation, submission, and lifecycle.

`Tree` consumes a caller-flattened borrowed node projection with stable IDs,
depth, disclosure, enabled, and loading/error facts. `TreeState` owns only
focus, selection, hover, viewport offset, and painted row/disclosure/scrollbar
hit regions. Keyboard, wheel, click, and scrollbar-position methods return or
apply semantic selection/toggle/activation/scroll outcomes; callers retain
hierarchy, filtering, lazy loading, and expansion policy.

`List` owns selection, hover, focus, viewport offset, painted regions, and a
reserved proportional-scrollbar gutter. Stable-ID rows use the general
navigation contract. Index-addressed pickers use the `ListState<usize>` count,
wrap-navigation, bounded-gesture, reconciliation, and selected-item methods so
consumers do not retain a second list-state crate or generic picker helpers.

`Table` consumes caller-owned borrowed columns, styled cells, and stable-ID
rows. Fixed/minimum/fill policies resolve deterministic visible widths;
rightmost columns collapse first at impossible widths without phantom gaps.
`TableState` owns selection, hover, vertical offset, resolved geometry, and
painted header/row regions. Keyboard and pointer methods emit typed row
selection/activation or column sort requests. Callers own sorting execution,
row ordering, data loading, wording, and effects. Rendering scans only the
visible body window and reuses state-owned layout scratch buffers.

`VirtualGrid` is a two-axis virtualized grid over caller-projected resident
cells. Callers supply column specs, a window of borrowed cells with absolute
row indices, optional known totals, and pending markers for non-resident
data. `VirtualGridState` owns two-axis viewport origin, cursor cell, optional
range anchor, column widths, focus, and painted header/cell hit regions.
Keyboard (arrows/page/home/end, shift-range), wheel, click, and drag emit
semantic cursor/range/viewport/activate outcomes. The grid never fetches or
edits data and never allocates the full dataset; render cost is bounded by
the painted viewport.

`TextArea` owns a normalized nonempty line buffer, grapheme-boundary cursor,
selection, undo/redo, word/indent motion, remembered vertical goal column, and
two-axis `ScrollArea` viewport with optional soft wrap and line-number gutter.
Enter inserts a newline; consumer keymaps retain submission policy. Host
outcomes cover clipboard, external editor, and fullscreen promotion. Paste
normalizes CRLF, LF, and CR; review/read-only variants mute chrome. Callers
retain validation, effects, syntax policy, persistence, and submission.

`PasswordInput` is the secure secret-entry surface: it composes `TextInput`
editing, always masks paint unless reveal policy allows, redacts `Debug` and
semantic descriptions, never embeds secrets in outcomes, gates clipboard via
`ClipboardPolicy`, supports strength/status hooks and `PasswordConfirmState`
pairing, and best-effort `secure_clear` on drop. `TextInput::secret` remains
paint-only for demos.

`NumberInput` is the typed numeric field: draft text is separate from a
locale-independent committed `f64`, with `NumberKind` (integer/decimal),
min/max/step via `NumberConstraints` (including `from_slider` /
`to_slider_value`), intermediate invalid typing, steppers/wheel, unit suffix,
and empty-allowed state.

`SearchInput` is the specialized search field: TextInput-backed query, clear,
history recall, leading filter-chip metadata before the query, trailing
status (searching / counts / no-results / error), command/filter/goto syntax
detection, Tab completion request, Esc clear-then-cancel, and debounce
signals via `poll(FrameTick)` without embedding async work.

`PathInput` is the filesystem-aware path field without FS coupling: pure path
helpers (separators, absolute detection, join, tilde/env expansion via host
data), host-projected `PathFsStatus` / `PathRisk` / `PathExpect`, base/relative
context, history, Tab completion and Ctrl+O browse outcomes, destructive-target
chrome, for setup flows and future FilePicker composition.

`TokenField` owns an editable token list plus free-text draft: commit on Enter/
separators, paste multi-value, Backspace-remove, Left/Right across tokens and
draft (one surface focus), Alt-reorder, multi-select, duplicate policy,
overflow `+N`, and completion requests for async suggestions. `TokenStrip`
remains the projection-only strip.

`Select` is the single-choice form control: closed trigger + open list on
`CollectionState` (value ≠ highlight), groups/separators, optional search,
typeahead, recipes (inline/form/compact), and popover vs fullscreen for tiny
terminals. Host places overlays from `Opened` / `PresentationChanged` outcomes.

`MultiSelect` is the multi-choice counterpart: ordered `Selection` membership
(check ≠ highlight), select-all / clear, max selection, Shift-range, search,
chip summary with `+N` overflow, and the same popover/fullscreen presentation
policy as Select.

`Combobox` / `Autocomplete` pair editable `TextInput` draft with
`CompletionMenu` suggestions: draft ≠ active suggestion ≠ committed value;
async results apply only when generation matches; host owns OverlayStack
placement via `open_completion_overlay`. Autocomplete defaults to creatable
free text; Combobox defaults to exact-match constrained commit.

`Picker` composes query editing, a caller-filtered stable-ID `List`,
ID-sticky/index-fallback reconciliation, empty state, and semantic outcomes.
Printable and cursor-editing keys route to the query while vertical/page keys
route to results; Escape clears a non-empty query before cancelling. Pointer
activation delegates to painted List geometry. Consumers retain matching,
scoring, ordering, candidate lifecycle, labels, overlays, and async policy.

`CompletionMenu` is a popup candidate list with stable caller-owned IDs,
selected ID, scroll, and keyboard/mouse routing. Geometry is placed by
`place_completion_menu` so the menu never covers the anchor cell and
flips/clamps inside a caller-supplied bounding rect. Enter/Tab report
`Committed(id)`; Escape reports `Dismissed`. Consumers own ranking,
filtering, label text, kind annotations, and commit policy (token replace).

The `tree_hot_path` evidence renders a warmed 40-row viewport over 10,000
borrowed nodes 100 times in the Cargo test/debug profile, asserts bounded
painted regions, and enforces named budgets `tree_viewport_10k` /
`tree_viewport_10k_alloc` via `termrock::perf` (`check_batch_budget` 250 ms,
`check_zero_alloc_steady`). Full budget table:
`docs/design/streaming-performance.md` and `termrock::perf::budgets()`.
The 2026-07-16 baseline was 45.09 ms on an Apple M1 Max; raising a budget
requires a deliberate PR with new measurement evidence.

Every component uses borrowed render data and stable IDs where interaction identity is required. Consumers own labels, validation, filtering, lifecycle, output, and domain models. Canonical neutral stories and SVG previews are maintained by `termrock-lookbook`; the catalog coverage check derives the component inventory from `docs/api/public-api.txt`, requires at least one typed story, documented story ID, and deterministic preview per public widget, and requires an exact keyboard/mouse/focus/non-color/Unicode/narrow-terminal classification in `docs/api/component-contracts.json`.

Cross-widget focus uses `FocusRing`: consumers register stable IDs, enabled
state, and optional painted rectangles every frame. TermRock owns ordered
Tab/BackTab traversal, dynamic reconciliation, pointer transfer, semantic
panel emphasis, modal trapping, and opener restoration. Composite widgets
remain one screen target and retain their domain-neutral internal navigation.
