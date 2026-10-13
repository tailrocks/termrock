# TermRock component inventory

The public widget set is derived from the reviewed API report and currently contains `Accordion`, `ActionBar`, `ActionLink`, `Alert`, `AlertDialog`, `AvatarGlyph`, `Backdrop`, `Badge`, `Banner`, `BarSeries`, `Breadcrumbs`, `Button`, `ButtonGroup`, `Callout`, `Card`, `Chart`, `Checkbox`, `CheckpointTimeline`, `ChoiceDialog`, `CodeBlock`, `Collapsible`, `CommandPalette`, `CompletionMenu`, `DataTable`, `DateTimePicker`, `Description`, `DesignInspector`, `DetailTable`, `DiagnosticView`, `Dialog`, `DiffReview`, `DiffView`, `Drawer`, `DropdownMenu`, `EmptyState`, `ErrorState`, `EventStream`, `FieldCaption`, `FilePicker`, `Form`, `FormWizard`, `FullscreenViewer`, `Gauge`, `Heading`, `HexViewer`, `HighlightedText`, `HintBar`, `Histogram`, `HistoryPicker`, `Icon`, `Identity`, `ImageSurface`, `JumpOverlay`, `Kbd`, `KeyValueTable`, `KeybindingRecorder`, `KeyboardHelp`, `Label`, `Link`, `List`, `LoadingView`, `LogPane`, `LogStream`, `MarkdownView`, `Menu`, `MenuBar`, `MessageDialog`, `MetricRadar`, `ModeRibbon`, `MultiSelect`, `NavigationList`, `NotificationCenter`, `NumberInput`, `ObjectInspector`, `OfflineBanner`, `Pagination`, `Panel`, `Paragraph`, `PasswordInput`, `PathInput`, `PermissionPrompt`, `Picker`, `Popover`, `PreviewCard`, `ProgressBar`, `ProgressSteps`, `PromptComposer`, `QuestionFlow`, `QuickOpen`, `RangeSlider`, `ResizablePanelGroup`, `SearchInput`, `Section`, `SegmentedControl`, `SegmentedMeter`, `Select`, `Separator`, `ShortcutHint`, `Sidebar`, `Skeleton`, `Slider`, `Sparkline`, `Spinner`, `SplitPane`, `StatusBar`, `StatusIndicator`, `Stepper`, `Surface`, `Switch`, `Table`, `Tabs`, `TerminalOutput`, `Text`, `TextArea`, `TextInput`, `ThemePicker`, `ThinkingBlock`, `Timeline`, `Toast`, `Toggle`, `ToggleGroup`, `TokenField`, `TokenMeter`, `ToolCard`, `Toolbar`, `Tooltip`, `Transcript`, `Tree`, `TreeNavigation`, `TreeTable`, `Viewport`, `VirtualGrid`, and `VirtualList`.

`BackgroundTaskPanel` monitors detached jobs/watchers/servers (bounded output, reconnect/lost, request-only control).


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

`FileTree` is a **filesystem explorer** on `Tree`: host projects
`FileTreeEntry` rows (kind, git status, hidden/ignored, lazy dirs, symlink
targets, permission errors). State owns filter, hidden/ignored toggles, rename
draft, multi-select, and safe multi-delete confirm. Outcomes are typed
requests (open, preview, load children, create/rename/delete, copy path,
QuickOpen, breadcrumbs) — no FS/Git I/O. Bridges: `breadcrumbs_from_path`,
`file_tree_to_quick_open_items`.

`ProcessTable` is a **process / task monitor** (flat + tree). Host projects
`ProcessRow` snapshots keyed by `ProcessKey` (pid + start marker — never PID
alone). State owns selection, multi-check, sort, filters, view mode, refresh
cadence chrome, and safe TERM/KILL/INT confirm. Outcomes are typed requests
(signal, refresh, details, copy command) — TermRock never enumerates processes
or sends signals. `TreeTable` remains the generic hierarchy+columns substrate;
`process_column_model` supplies default columns.

`QueryEditor` is a **code-oriented query workbench** (SQL / logs / search DSLs).
It embeds `TextAreaState` for the draft, preserves cursor across result focus,
and emits run/stop/format/save/history/completion requests only. Integrates
`CompletionMenu`, `Diagnostic`/`CodeFrame`, `KeyboardHelp`, `HistoryPicker`,
and a results slot for host `ResultGrid` / `DataTable`. Compact / normal /
fullscreen modes. No language servers or DB drivers inside TermRock.

`ResultGrid` is the **typed query-result grid** on `DataTable`: nulls, binary
summaries, secret redaction, large-text clamp, row numbers, streaming/partial
status, column stats chrome, export/inspect/page outcomes, and ObjectInspector
bridges. Host projects only the visible window for wide schemas and unknown
totals. No SQL drivers or file export IO inside TermRock.

`SchemaBrowser` is a **hierarchical catalog navigator** on `Tree`: connections,
databases, schemas, tables/views, columns, indexes, constraints, routines.
Lazy expansion, connection health chrome, filter with ancestors, expanded-id
preserve across refresh/reconnect, side-pane/drawer/fullscreen presentation,
QuickOpen and Breadcrumbs bridges. Outcomes are typed metadata/query requests
— no SQL drivers inside TermRock.

`SearchResults` is a **grouped search hit list** for files, logs, objects,
commands, and docs: match ranges + keep-match snippet truncation, group
collapse, n/N match walk, streaming/partial/stale generation gates, and
open/preview/fullscreen/QuickOpen outcomes. Host owns search I/O; compose with
`SearchInput` for the query field.

`MetricsDashboard` is an **observability block**: metric cards with
sparklines/gauges (public chart APIs), thresholds, comparison deltas, alerts,
time-range/refresh chrome, spatial keyboard nav, command-palette action ids,
and narrow vertical summary. Per-tile health supports partial failure. Host owns
scrape/query.

`TraceWaterfall` visualizes **nested spans** (traces / agent tools) with a name
column and duration bars on a shared time axis, critical-path emphasis, status
letters, filter with ancestors, zoom/pan independent of vertical virtualization,
and Hierarchy vs Timeline nav modes. Bridges to ObjectInspector and Timeline.
Host owns trace fetch.

`DependencyGraph` is a **constrained dependency map** for packages, services,
schemas, and tasks: deterministic layered layout, ASCII connectors, pan
navigation, Graph/Tree/List views, and auto TreeTable-shaped fallback when the
canvas is too narrow or large. Host owns resolution; ObjectInspector bridge for
details.

`PromptComposer` is the **flagship agent input surface**: grapheme-safe
multiline `TextArea`, selection, undo/redo, submit history, attachments and
paste chips, slash/@/# completion triggers, mode/model/context chrome,
queue-while-busy, submit/interrupt/cancel, and external-editor outcomes. Draft
survives permission/plan/session/palette takeover via `accepts_input` only.
Buckets stay separated (edit · tokens · completion · presentation · policy).
Bridges: `CompletionMenu` / OverlayStack, `KeyboardHelp`, `HistoryPicker`,
`TokenMeter`. Host owns providers and candidate search.

`AttachmentChip` and `PasteChip` are **structured attachment tokens** for files,
images, URLs, selected code, and large pastes: type/name/size/lines, status
(upload/index/error), validation, remove/open/preview/retry outcomes, progress
marks, and sensitive-safe semantic summaries (never paste bodies). Large pastes
collapse with expand/copy-by-id. Strips use `TokenStrip` wrap/scroll/`+N`.
PromptComposer paints chips through these widgets; `ComposerChip` remains the
list model with conversion bridges.

`FileMention` and `EntityMention` are **inline structured mention tokens** for
files, symbols, agents, tools, sessions, and resources: label, canonical id/path,
type glyph, validity (stale/missing/ambiguous), preview, and remove. Atomic
cursor movement in `MentionDraft`; completion via `CompletionMenu` projections;
disambiguation lists for ambiguous names. Host owns resolution — no provider
lookup. Semantic descriptions redact sensitive paths.

`SlashCommandMenu` is the **caret-anchored slash command surface** for prompt
composers: name/aliases/description/arguments/shortcut/source, recent bias,
nested argument completion, fuzzy filter, async plugin generation gates, and
draft-preserving token replace. Bridges global `CommandEntry` catalogs while
keeping composer-only commands. Host executes — outcomes only.

`ModelSelector` and `AgentModeSelector` are **compact composer selectors** for
model id (provider, capabilities, cost/latency/context, availability, recent)
and agent safety mode (Ask/Plan/Edit/Auto/FullAuto) plus execution policy.
Separated controls with optional `ComposerSelectors` strip. FullAuto and
high-risk policy use warning roles; provider data is host-owned. Bridges to
`ModelIndicator` / `ModeIndicator` and ModeRibbon.

`MessageThread` is the **virtualized agent conversation pack** over `Transcript`:
user/assistant/system/tool/status/event/error entries, grouping, timestamps,
actors, search, copy, semantic zoom, collapsed tools, follow-tail with unread
indicator, and checkpoint-preserving compact. Project-to-lines v1 (no nested
widgets). Editorial prefixes — not chat bubbles.

`StreamingMarkdown` is the **token-stream Markdown** host over `MarkdownView`:
stable committed prefix + reparsed tail, incomplete fences/tables without
full-doc reparse on hot path, coalesce batching, citations/tool insertions,
scroll follow while streaming, and plain-line projection for MessageThread.

`SourceCitation` and `CitationList` are **inline citations and expandable source
lists**: title, type, path/URL, range, confidence/provenance, open/preview/copy,
unavailable/offline, and duplicate grouping. Raw destinations stay visible for
external and sensitive sources. Bridges StreamingMarkdown `StreamCitation` and
Markdown `SourceAnchor`; host owns open/OSC.

`ToolCallCard` is the **interactive agent tool execution card**: full lifecycle
statuses (queued → preparing → running → waiting input/permission → streaming →
success/warning/failure/cancelled/detached), verb/actor/args/duration/result/
risk/egress, inline expand + fullscreen zoom, secret redaction, and typed
outcomes (cancel/retry/permission/copy/diff/log). Host executes tools; card never
binds a provider protocol. Thin `ToolCard` remains the paint-only summary.

`TerminalRunCard` specializes shell/terminal runs over the `TerminalOutput`
substrate: proposed vs executed command, edited approval, provenance, env
redaction, stdout/stderr, exit/signal, follow/scroll while streaming, stop/
detach/retry/copy/fullscreen/permission outcomes. Bridges to `ToolCall` for
MessageThread. Host owns PTY/process.

`ActivityShelf` is the **compact concurrent activity strip**: status, elapsed,
actor, progress, waiting reason, jump/open, with blocked/action-required first.
Narrow → summary or badge. Projects into StatusBar slots and NotificationCenter.
Does not replace TaskRail.

`TaskRail` is the **unified task/agent activity side panel** over application-neutral
`ActivityModel`: workflows, subagents, foreground/background, watchers, completed
history; collapse/filter/search/zoom; needs-input priority; Drawer/StatusBar
collapse; bridges to ActivityShelf and List projection.

`SubagentCard` is the **delegated agent run** card (live vs artifact, nested provenance, request-only control).





`TerminalRunCard` specializes shell/terminal runs over the `TerminalOutput`
substrate: proposed vs executed command, edited approval, provenance, env
redaction, stdout/stderr, exit/signal, follow/scroll while streaming, stop/
detach/retry/copy/fullscreen/permission outcomes. Bridges to `ToolCall` for
MessageThread. Host owns PTY/process.

`Sparkline`, `Chart`, `Gauge`, `Histogram`, `BarSeries`, and `SegmentedMeter`
form one **visualization family** with shared `ScaleMode` (auto/fixed/log),
`VizGlyphSet` (block/braille/ASCII), missing-data marks, thresholds, selection
marks, and streaming `window` helpers. No-color mode uses ordered series
markers and density glyphs rather than hue alone. `TokenMeter` remains the
token-usage specialist. `ContextMeter` elevates budgets with approximate precision, breakdown, compaction actions, and non-token units. `QuestionFlow` is multi-question HITL with structured answers, review, queue/provenance; never clears composer draft. `PlanReview` is interactive plan-document review (Markdown, comments, version diff) with safe action focus; Approve never default. `SessionPicker` elevates session create/resume/search/rename/archive/delete; cancel preserves draft. `ConnectionManager` manages DB/SSH/API/service connection inventory (launcher + full, safe secrets, Offline/diagnostic projection; host owns protocol and vault). `DatabaseWorkbench` (pattern) composes ConnectionManager, SchemaBrowser, QueryEditor, ResultGrid, ObjectInspector, history, StatusBar, and CommandPalette with density collapse and typed run/cancel/export messages. `GitWorkbench` (pattern) composes FileTree, DiffReview, CheckpointTimeline, TerminalOutput, conflict diagnostics, branches, StatusBar, and KeyboardHelp with staging/discard confirms and fullscreen diff; host owns Git I/O. `ObservabilityDashboard` (pattern) composes SearchInput, LogStream, EventStream, MetricsDashboard, ObjectInspector, and StatusBar with live/pause, dropped/reconnect, bookmarks, and drill-down; host owns acquisition. `FileManager` (pattern) composes Breadcrumbs, SearchInput, FileTree, PreviewCard, QuickOpen, operation queue, StatusBar, and confirm/conflict dialogs with density multi→single-pane/drawer and typed copy/move/delete/rename/new requests; host owns FS I/O. `ProjectLauncher` (pattern) composes SearchInput, grouped project List, SessionPicker, PreviewCard, QuickOpen, ConnectionStatus chrome, EmptyState onboarding, and StatusBar with home/inline modes and typed open/new/import/favorite/session requests; host owns discovery and persistence. `HelpCenter` (pattern) composes SearchInput, topic nav, KeyboardHelp (live HelpEntry/keymap SoT), command reference from HelpEntry, MarkdownView, DoctorReport diagnostics, and StatusBar with full/compact modes; host owns markdown and command execution. `ErrorRecovery` (pattern) composes ErrorState summary, preserved-work cue, recovery action list, redacted crash-report diagnostics, and StatusBar with full and inline-fallback modes; host owns restart, session restore, logs, issue trackers, and panic hooks. `InputOtp` is fixed-slot PIN/OTP entry (shadcn Input OTP peer). `Carousel` is keyboard multi-slide panels. `InputGroup` composes prefix/suffix addons around TextInput. `PromptQueue` manages queued prompts (compact/expanded) without auto-drain on fail. `AgentStatusHeader` is top agent/session chrome with StatusBar contraction. `IntegrationStatus` manages MCP/plugin health with safe egress language. `WorkingStateCard` summarizes current agent work without CoT. `ApprovalQueue` unifies pending human decisions without bulk high-risk approve.

`HexViewer` is a **virtualized binary inspector**. Host projects a paged
`HexWindow` (`base_offset` + `data` + `total_len`). State owns absolute cursor
and selection, bookmarks, search needle, endian, ASCII mode, and auto/fixed
bytes-per-row. Paint marks active byte with brackets and selection with braces
(non-color). Inspector strip decodes u8..u64 LE/BE. Outcomes: copy hex/ASCII,
export dump, `PageNeeded` for host re-paging. Property-tested offset/row math.

`TerminalOutput` is the **safe command-run presentation** pane (agent tools, CI,
build logs). Host projects `TerminalCommandMeta` (command, cwd, env summary,
status, exit/signal, duration, pid) and `TerminalLine` rows (stdout/stderr/
system, optional pre-parsed `AnsiLine`). State owns follow/pause via
`ScrollAreaState`, recipe (compact/pane/fullscreen), paint mode (ANSI/no-color/
plain/raw), stream filters, and env panel. Outcomes are **requests only**
(cancel, retry, detach, copy) — TermRock never executes processes. Compose with
`TerminalRunCard` for agent-card chrome (proposed/executed/permission),
`ToolCard` / `ToolCallCard` for generic tools, and `AnsiStream` for ingest.

`Diagnostic` / `CodeFrame` present structured diagnostics (rustc/miette-class)
with severity letters (never color alone), codes, messages, sources, ranges,
related locations, notes, help, docs links, and suggested fixes. Recipes: list
(problems panel), inline (forms), full (code frame + notes + fixes).
`CodeFrame` paints source windows with tab expansion, multi-line/overlapping
underlines, and truncation markers. Bridges to `CodeBlock` via
`diagnostics_to_highlights` / `diagnostics_to_gutter_marks` and to `ErrorState`
via `format_diagnostics_plain`. Host owns apply/open/copy side effects.

`DiffView` is the high-quality **read-only** unified/side-by-side diff renderer
(delta/GitUI-class). Host projects `DiffLine` rows (kind, text, optional line
numbers, word spans, syntax spans, trailing-ws, file/hunk ids) plus optional
`DiffHunk` / `DiffFile` bands. State owns Auto/Unified/Split mode (narrow forces
unified), scroll virtualization, search, fold, cursor/hunk navigation, and
anchors. No-color prefixes (`+`/`-`/` `) always paint. Distinct from `CodeBlock`
(single snippet) and `LogStream` (severity lines).

`DiffReview` is interactive **patch review** on DiffView for Git, plan changes,
and AI-agent code review. Host projects file-tree rows (`DiffReviewFileRow`) plus
the same `DiffLine`/`DiffHunk` window. State owns focus regions (file tree ·
diff · comments · summary), multi-select (file/hunk/line), decisions
(`DiffDecision` by stable `DiffReviewUnit` keys), comments with anchors, draft
comment chrome, destructive-confirm banners with safe verbs, and undo/redo of
session review ops. Outcomes are **requests** only — TermRock never runs git or
apply policy. Selection and comments survive mode/resize via stable ids.

`LogStream` is the continuous **professional log viewer** (stern/k9s-class). Host
projects a window of `LogLine` (id, level, text, optional timestamp/source/
styled ANSI, batch count). State owns follow/pause/unread via `ScrollAreaState`,
cursor, multi-select, bookmarks, search, level floor, wrap/h-scroll, compact or
detailed recipes, dropped/reconnect/batch chrome, and stable anchors. Outcomes
include copy/export (host I/O). `LogPane` still owns single-buffer append/evict
and may project via `log_lines_from_plain`. Distinct from `EventStream`
(structured events) and `Timeline` (chronological recipes).

`EventStream` is the high-volume **structured** event viewer (k8s events, agent
activity, observability). Rows carry type, timestamp, severity, source, pluggable
summary, optional fields/detail/correlation, and batch counts. Follow/pause and
unread use `ScrollAreaState`; host reports backpressure (`dropped`/`batched`).
Stable anchors preserve selection across reproject. Distinct from `LogStream`
(plain lines) and `Timeline` (chronological recipes).

`Timeline` presents chronological events (sessions, deploys, traces, agent
turns) with status markers, actor/relative/duration metadata, grouping,
expansion, correlation, filters, and live-stream follow that pauses when the
user scrolls up. Recipes: `Rail`, `Detailed`, `GroupedDay`. No-color mode uses
status letters and ASCII markers. `CheckpointTimeline` is elevated session
history (browse/preview/confirm, boundaries, restore/rewind requests; draft
preserved) projecting onto Timeline when needed. Composes with LogStream and ProgressSteps.

`ObjectInspector` is the expandable typed inspector for JSON/YAML/TOML,
structured logs, and application trees. Host projects a flattened **visible
expanded** node list; state owns cursor, expansion-by-path (sticky across
reproject), search, secret reveal, compare mode, edit draft, depth limit, and
virtual window metadata. Nodes carry stable `path`, `InspectKind`, branch/lazy
status, and optional compare values. Paint escapes control characters and
redacts secrets. Chords: expand/collapse, copy value/path, edit, reveal,
search, fullscreen. Distinct from `KeyValueTable` (flat metadata) and `Tree`
(single-column hierarchy).

`KeyValueTable` is the dense interactive detail surface for metadata and
object properties (HTTP headers, DB columns, process facts, permission claims,
agent/tool panels). Fields carry key · value · optional type/source · status ·
validation · secret · editable · compare-side. Layout contracts columns→stacked
under width pressure (`KvLayout` / `KvDensity` shared with KeyValueList). One
focus target per row; `c` copy, `e` edit, `r` reveal, `d` compare, `/` filter.
`DetailTable` remains dialog-oriented; `KeyValueList` remains the lighter
settings/summary list.

`TreeTable` combines hierarchical rows with columns (process trees, schema
browsers, tasks, dependencies). Host projects a flattened **visible-expanded**
window; TermRock paints compact indent, disclosure glyphs, sticky headers, and
data columns via `ColumnModel`. `TreeTableNavMode` makes Left/Right explicit:
Hierarchy (default expand/collapse/parent), Cell (column cursor; Shift =
hierarchy), Scroll (h-scroll only). Supports lazy/loading/error via
`TreeNodeStatus`, multi-check, group bands, aggregate rows, sort on data
columns (not the hierarchy label), filter-with-ancestors, and virtual windows
for large expanded sets. Distinct from `Tree` (single column) and `DataTable`
(flat grid).

`DataTable` is the interactive / virtualized flagship grid for professional
developer tools. Consumers project only `window.visible_range()`; TermRock never
allocates the logical universe. Kits in `data_view` supply `ColumnModel` (width,
pin, visibility, priority, resize overrides, reorder), `SelectionModel`
(row/multi/cell/range), `VirtualWindow`, `LoadState` (idle/loading/partial/ready/
empty/error), sort/filter specs, expand/group headers, and copy payloads.
`DataTableState` owns cursor, nav modes (`Cell`/`Row`/`Range`), h-scroll, sticky
header geometry, header/cell hit regions, resize drag, range anchors, and edit
draft. Outcomes cover sort, filter, selection, resize, reorder, visibility,
edit start/commit/cancel, copy, context menu, toolbar, fullscreen promotion,
and select-all **request** (visible scope only). Pointer: header sort, edge
resize, cell click/drag range, wheel, context click. Colorless: gutter markers
and ASCII sort glyphs. Display-only moderate tables use `Table`.

`Table` is the polished static / moderate-size columnar presentation surface
(display model; interactive 1M kit is `DataTable`). Caller-owned borrowed
columns, styled cells, and stable-ID rows. Fixed/minimum/fill policies plus
column **priority** resolve visible widths — lowest priority drops first under
pressure (ties rightmost). Recipes: Quiet, Bordered, Striped, Compact. Sticky
header, horizontal scroll (`h_offset`), optional cell focus, clip/ellipsis
overflow, and Ready/Loading/Error body states with host messages. Selection
uses design-system `SelectionChrome` (gutter/tint/fill) rather than a hard-coded
chevron. `TableState` owns selection, hover, vertical/horizontal offsets, cell
focus, resolved geometry, and painted header/row regions. Keyboard and pointer
methods emit typed row selection/activation or column sort requests. Callers own
sorting execution, row ordering, data loading, wording, and effects. Rendering
scans only the visible body window and reuses state-owned layout scratch buffers.

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
