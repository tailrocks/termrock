---
status: strategic design SoT
date: 2026-08-09
kind: product architecture + experience research
policy: quality over compatibility; breaking preferred
related:
  - experience-research-2026.md
  - competitive-tui-research.md
  - pre-1.0-api-redesign.md
  - source-owned-registry.md
  - termrock-agent.md
  - termrock-studio.md
note: >
  Strategic brief for TermRock as the shadcn-class terminal design system.
  Some recommendations may already be partially landed in tree (InteractionScene,
  OverlayStack, DesignSystem, agent surfaces, registry CLI, capability profiles);
  treat residual dual authorities and incomplete distribution gravity as open work.
  Do not copy product-specific Grok/Amp/OpenCode chrome or proprietary source.
---

# TermRock: from a strong Ratatui crate to the shadcn/ui of terminal software

## Executive verdict

**TermRock already has a better interaction foundation than most TUI component collections.** Stable identities, focus management, modal scopes, opener restoration, hit geometry, grapheme-safe editing, terminal-session rollback, narrow-layout contracts, virtualization, performance budgets, and a real lookbook are difficult problems—and TermRock has deliberately addressed them. ([GitHub][1])

However, **TermRock is not yet the shadcn/ui of terminal applications**. It is currently closer to a carefully designed dependency crate. The defining advantage of shadcn/ui is not its Button or Dialog; it is that shadcn is a **distribution system for inspectable source code**, with registries, blocks, dependencies, themes, namespaces, CLI installation, and upstream diffing. Users receive code they can own, understand, and modify. ([Shadcn][2])

The correct ambition is:

> **TermRock should become an open-source terminal design system, source registry, interaction kernel, and collection of high-quality application blocks for Ratatui.**

A fitting product line would be:

> **Beautiful, inspectable terminal components you own.**

The highest-priority changes are:

1. Split TermRock into a small stable kernel and a source-owning component registry.
2. Replace the current color-role theme with a complete terminal design system.
3. Introduce semantic input intents, a unified overlay stack, and a per-frame semantic UI scene.
4. Build foundational primitives before adding more high-level widgets.
5. Make AI-agent interfaces the flagship component pack.
6. Turn the lookbook into a world-class component studio, inspector, test runner, and registry browser.
7. Support graceful capability reduction instead of assuming only modern truecolor terminals.

Compatibility should not block this redesign. TermRock’s own documentation already treats it as pre-release software and explicitly prioritizes better APIs over compatibility shims, which makes this the ideal moment for a foundational break. ([GitHub][3])

---

## 1. Current-state assessment

| Area                           |     Assessment | Strategic interpretation                                                 |
| ------------------------------ | -------------: | ------------------------------------------------------------------------ |
| Terminal lifecycle correctness |  **Excellent** | A genuine moat; preserve it in a stable core                             |
| Focus, hit-testing, scrolling  |  **Excellent** | Stronger than many visually richer TUI libraries                         |
| Unicode and narrow layouts     |     **Strong** | Continue expanding capability tests                                      |
| Data-oriented widgets          |     **Strong** | Table, Tree, VirtualGrid, List and LogPane are valuable foundations      |
| Component composition          |   **Moderate** | APIs are still more widget-like than design-system-like                  |
| Visual hierarchy               |      **Early** | Current phosphor identity is memorable but overly flat                   |
| Theming                        | **Too narrow** | Colors/styles exist; spacing, density, glyphs, motion and recipes do not |
| shadcn-style distribution      |    **Missing** | The most important product gap                                           |
| Higher-order blocks            |    **Missing** | Needed for users to build excellent applications quickly                 |
| AI-native UI                   |    **Missing** | The largest opportunity for differentiation                              |
| Terminal compatibility         |     **Narrow** | Modern-first is fine; modern-only will limit adoption                    |
| Lookbook and documentation     |  **Promising** | Should evolve into TermRock Studio                                       |

### What is already unusually good

TermRock’s public inventory covers roughly two dozen useful widgets, including tables, trees, virtual grids, lists, forms, text editing, pickers, completion menus, dialogs, logs, diffs, split panes, status surfaces, toasts and navigation. Its components generally use borrowed render data and stable IDs rather than hiding application state or introducing unnecessary cloning. ([GitHub][1])

The runtime boundary is especially strong. `Session` owns raw mode, alternate screen, mouse capture, bracketed paste, wrapping and cursor lifecycle, with rollback and idempotent restoration. TermRock also keeps side effects and domain policy in the application rather than embedding them inside visual components. This is exactly the right ownership model for an adaptable source component system. ([GitHub][1])

The interaction work is also mature. `FocusRing` supports per-frame registration, disabled-item reconciliation, pointer focus, modal trapping and opener restoration. Hover does not silently steal keyboard focus, and composites can expose a single external focus target while managing internal state themselves. 

The performance direction is sound: visible-window rendering, a two-dimensional `VirtualGrid`, resident-cell modeling and an explicit performance budget for large trees. That is much closer to production infrastructure than a typical decorative widget collection. ([GitHub][1])

Finally, the lookbook is not a fake screenshot gallery. It renders public APIs with real state, typed stories, narrow and Unicode variants, SVG generation and a contract matrix. That is an excellent foundation for a serious design-system studio. ([GitHub][4])

---

## 2. The main things holding TermRock back

### 2.1 It distributes abstractions, not owned source

Today, the primary experience is adding TermRock as a crate dependency. That is appropriate for lifecycle, focus, geometry, Unicode and low-level behavior. It is less appropriate for opinionated components and application blocks.

A source-owned model would let a developer install `command-palette`, inspect it, alter its state model, replace its visual recipe, add application-specific fields and still retain TermRock’s underlying interaction contracts. That is the essential shadcn experience.

The right answer is not to eliminate the crate. It is to use a **hybrid model**:

* Stable, compiled infrastructure remains in crates.
* Styled components, recipes and application blocks are copied into the application.
* A manifest records provenance, dependencies, installed versions and local modifications.
* The CLI can show upstream differences without overwriting user code.

### 2.2 The current visual identity is memorable but one-dimensional

The fixed theme currently contains 38 semantic roles, which is a good beginning. But in the default phosphor theme, `Canvas`, `Surface`, `Elevated` and `Backdrop` are effectively empty styles, while selection, focus, accent and several semantic states converge on the same phosphor green. The Slate preset demonstrates more surface hierarchy, but the default experience does not. ([GitHub][5])

The practical result is:

* Weak distinction between canvas, container, overlay and selected content.
* Bright selection fills that overwhelm surrounding content.
* Too little visual depth despite already having semantic elevation roles.
* A strong “hacker terminal” brand, but not yet a restrained luxury interface.

The phosphor identity should not be discarded. It should become **rarer and more intentional**.

A better principle is:

> **Quiet canvas, bright intent.**

Use phosphor green for the active cursor, primary action, live state and critical focus—not for every selected row and nearly every semantic role.

### 2.3 Theme is being asked to do the job of a design system

A `[Style; 38]` theme can answer “what color is a focused border?” It cannot answer:

* How much horizontal padding does a compact menu row have?
* Does selection use a gutter, a tint or a full fill?
* Are borders single, heavy, rounded or absent?
* What glyph represents expansion under ASCII fallback?
* How quickly does a spinner advance?
* How does a dialog contract at 50 columns?
* What is the minimum touch target for mouse interaction?
* Does the user prefer comfortable, compact or dense layouts?
* How do Button, DropdownMenu and Tab each interpret the same semantic accent?

TermRock needs design tokens and **component recipes**, not only semantic colors.

### 2.4 Interaction APIs are not yet consistently semantic

TermRock already has a keymap system intended to serve dispatch, hints, glyphs, conflicts and runtime remapping. But some widgets still interpret concrete keys directly. For example, the current List implementation handles arrows, `j`/`k`, Home, End, page navigation, Enter, Space and Escape internally. That makes it harder to offer globally consistent Vim, Emacs, default and application-specific mappings. 

The framework should normalize raw terminal events into semantic intentions:

```rust
pub enum UiIntent {
    Move(NavigationMove),
    Page(PageMove),
    Activate,
    Toggle,
    Open,
    Close,
    Cancel,
    Submit,
    Edit(EditIntent),
    Command(CommandId),
}
```

Widgets should consume `UiIntent`, not hardcoded terminal keys. The keymap should remain application-configurable and become the single source for:

* Input dispatch.
* Contextual footer hints.
* Command-palette entries.
* Shortcut conflict detection.
* Generated help.
* User remapping.

### 2.5 Existing APIs are sometimes too structurally narrow

The current `Panel` is essentially a titled bordered block with a small emphasis enum. That is useful as a low-level primitive, but a design-system panel needs slots for title, subtitle, leading status, badges, header actions, footer actions, loading state and different chrome recipes. 

Likewise, production-grade list and table rows need compositional anatomy:

```text
[leading icon] [primary label] [secondary metadata] [badge] [shortcut] [status]
```

Those parts need independent styling and responsive priority. A narrow screen should remove low-priority metadata before truncating the primary label.

### 2.6 The compatibility baseline is too restrictive for the category goal

The current documented target is essentially a modern truecolor, Ghostty-class terminal on Linux or macOS, with no reduced-color mode, `NO_COLOR`, Windows or RTL/BiDi support. That is defensible for an early project, but not for a library aspiring to become the default TUI design system. ([GitHub][3])

“Quality first” should mean **progressive enhancement**, not “refuse to run gracefully outside the ideal environment.”

---

## 3. What successful terminal applications consistently get right

There is no single universally recognized equivalent of the Apple Design Awards for terminal software. AwesomeTUI’s 2026 awards are best treated as a current, curated community signal rather than a formal industry standard. Their selections are still useful: btop won overall, OpenCode won Best Developer Tool, Glow won Best Terminal UX, and micro won Best Daily Driver. The older Awesome TUIs list also deliberately limits itself to maintained, genuinely interactive applications rather than wrappers around other commands. ([Awesome TUI][6])

| Reference      | Why the experience stands out                                                                                                                                                                                                                                      | What TermRock should extract                                                                                         |
| -------------- | ------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------ | -------------------------------------------------------------------------------------------------------------------- |
| **Grok Build** | Treats planning, interviews, permissions, task management, rewinding and execution as distinct interaction modes. Its recent updates repeatedly refine focus restoration, overlays, selection, paste chips, context visibility and task hierarchy. ([SpaceXAI][7]) | Agent-native blocks; explicit mode changes; structured permissions; hierarchical activity rail; semantic status text |
| **Amp**        | Its rebuilt CLI is remote-controllable, compaction-first and plugin-powered. Sessions can be controlled across surfaces, while plugins can request notifications, confirmation, text input and selection UI. ([Ampcode][8])                                        | Separate session engine from terminal client; serializable plugin UI schema; prompt queue and cancellation           |
| **OpenCode**   | Uses terminal, desktop and IDE clients; exposes plan/build agents, sessions, themes, configurable keybindings and programmatic/server-style operation. ([OpenCode][9])                                                                                             | Client/server-compatible architecture; visible agent modes; transactional history; theme and keymap systems          |
| **btop**       | Combines exceptional information density with direct mouse interaction, keyboard navigation, autoscaling graphs, process trees, details and configurable visual presets. ([GitHub][10])                                                                            | Dense but calm dashboards; terminal-native charts; direct manipulation; view presets                                 |
| **lazygit**    | Turns complex Git operations into focused, context-sensitive workflows, down to lines and ranges rather than only whole files. Recent work has also focused on maintaining responsiveness during concurrent operations. ([GitHub][11])                             | Contextual actions, semantic granularity, progressive disclosure and safe destructive workflows                      |
| **Yazi**       | Treats asynchronous operations and task scheduling as core product concepts. It also supports several terminal image protocols and plugin-owned UI. ([GitHub][12])                                                                                                 | First-class background activity model, media abstraction, instant high-frequency navigation                          |
| **Zellij**     | Makes modes visible, provides layouts, floating and stacked panes, plugins and collaborative sessions while retaining strong defaults. ([GitHub][13])                                                                                                              | Mode ribbon, shareable layouts, pane recipes and plugin surfaces                                                     |
| **Glow**       | Demonstrates that excellent terminal typography comes from hierarchy, whitespace, wrapping and editorial restraint—not from surrounding everything with borders. It was selected as AwesomeTUI’s 2026 Best Terminal UX. ([Charm][14])                              | High-quality Markdown and prose rendering; fewer boxes; editorial content hierarchy                                  |
| **Posting**    | Combines a multi-pane workbench, command palette, jump navigation, autocomplete, syntax highlighting, custom keybindings and themes. ([GitHub][15])                                                                                                                | Jump mode, universal command palette, polished multi-pane application blocks                                         |

The recurring pattern is important: **the best TUIs are not merely attractive**. They make system state legible, expose the right action at the right moment, minimize navigation cost, preserve terminal correctness and use visual decoration with restraint.

---

## 4. Grok Build is especially valuable as a source-level reference

Grok Build’s current public repository contains the Rust source for its full-screen TUI and agent runtime. The TUI is separated from the shell/runtime, supports interactive, headless and editor-integrated operation, and uses an Elm-like Action → Effect structure internally. ([GitHub][16])

Its source tree exposes a remarkably useful inventory of real-world AI-agent surfaces:

* Prompt composer.
* Completion and slash-command dropdowns.
* File search.
* Plan approval.
* Question/interview flow.
* Permission view.
* Task and subagent panes.
* Context and credit bars.
* Rewind and session pickers.
* Extension, MCP and settings modals.
* Timeline and turn status.
* Overlay and fullscreen behavior. ([GitHub][17])

### The product behaviors worth extracting

**Permission requests are queued, not replaced.** Only the front request is interactive, preserving order when several requests arrive. Permission state distinguishes option navigation, follow-up text input and editable command patterns. It also models command scope, MCP tool/server scope, expandable details, subagent provenance and stale-response protection. 

**Question flows are richer than a generic Select dialog.** Grok Build supports single and multiple selection, free-form “other” input, multiple questions as tabs, per-question cursor and scroll positions, fullscreen expansion, prompt stashing and context-dependent bottom actions. 

**Plan approval is a review workflow.** It distinguishes preview, prompt and comment focus, lets the user attach feedback to selected line ranges, preserves source references and produces structured approve, revise or abandon outcomes. 

**The task pane is a unified activity model**, grouping workflows, subagents, ordinary background tasks and recurring watchers. It supports collapsible groups, type-aware actions, running/completed visual treatments, syntax-highlighted commands and compact activity metadata. 

**Overlay behavior is centralized.** Visibility, focus and fullscreen form an explicit state machine with consistent handling for Tab, Escape, `q`, Space and fullscreen toggling. 

### The implementation weakness is TermRock’s opportunity

Several of the highest-value Grok views are more than 3,000 lines each: permissions, questions, tasks and the prompt composer. They contain excellent product behavior, but they are tightly integrated with Grok-specific state, input, styling and domain policy. ([GitHub][18])

TermRock should **not copy these files as monoliths**. It should extract their reusable interaction concepts:

| Grok Build surface            | TermRock extraction                                                        |
| ----------------------------- | -------------------------------------------------------------------------- |
| `permission_view`             | `PermissionState`, `PermissionScope`, `PermissionPrompt` block             |
| `question_view`               | `QuestionFlowState`, `ChoiceGroup`, `FreeformChoice`, `QuestionFlow` block |
| `plan_approval_view`          | `ReviewDocument`, `InlineCommentState`, `PlanReview` block                 |
| `tasks_pane`                  | `ActivityModel`, `ActivityGroup`, `TaskRail` block                         |
| `prompt_widget`               | `ComposerState`, `TokenElement`, `AttachmentChip`, `PromptComposer`        |
| `overlay`                     | General `OverlayStack`, placement, focus trap and dismissal policies       |
| `session_picker` and `rewind` | `SessionPicker`, `CheckpointTimeline`, `HistoryNavigator`                  |

The low-level state machines should be product-neutral. Grok-, Amp- or OpenCode-specific policy should remain in application code or source-owned block recipes.

---

## 5. The correct product architecture

TermRock should become a hybrid of compiled infrastructure and copied source:

```text
┌──────────────────────────────────────────────────────────────┐
│                       User application                       │
│                                                              │
│  src/ui/                src/blocks/           src/themes/     │
│  copied components      copied screens        owned recipes   │
└──────────────────────────────▲───────────────────────────────┘
                               │ termrock add / diff / update
┌──────────────────────────────┴───────────────────────────────┐
│                    TermRock registries                       │
│ primitives · components · blocks · themes · keymaps · apps   │
└──────────────────────────────▲───────────────────────────────┘
                               │
┌──────────────────────────────┴───────────────────────────────┐
│                       Stable crates                          │
│ core · runtime · input · focus · geometry · capability       │
└──────────────────────────────────────────────────────────────┘
```

### Recommended package structure

| Package             | Responsibility                                                              |
| ------------------- | --------------------------------------------------------------------------- |
| `termrock-core`     | IDs, geometry, semantic events, focus, scrolling, selection, state helpers  |
| `termrock-runtime`  | Crossterm lifecycle, capability detection, event cadence, terminal requests |
| `termrock-render`   | Text, ANSI, wrapping, clipping, Markdown and syntax infrastructure          |
| `termrock-widgets`  | A conservative baseline of compiled headless or low-level widgets           |
| `termrock-registry` | Registry schema, resolution, dependencies and source metadata               |
| `termrock-cli`      | `init`, `add`, `search`, `view`, `diff`, `update`, `doctor`, `migrate`      |
| `termrock-studio`   | Stories, previews, event inspector, visual tests and registry browser       |
| `@termrock/agent`   | Source registry for AI-agent components and complete blocks                 |
| `@termrock/data`    | Data tables, inspectors, query/results and database workbench blocks        |
| `@termrock/ops`     | Logs, metrics, tasks, process and observability blocks                      |

The existing `termrock` crate can remain as an umbrella dependency.

### Registry item types

TermRock should support more than “component”:

```text
primitive
component
behavior
layout
block
template
theme
glyph-set
keymap
capability-profile
test-fixture
```

A registry item should describe:

* Files and target paths.
* Cargo dependencies and feature flags.
* Dependencies on other registry items.
* Minimum TermRock core and Ratatui versions.
* Required and optional terminal capabilities.
* Stories and preview states.
* Interaction contracts.
* Migrations.
* Source hash and license metadata.

### CLI experience

```bash
termrock init
termrock search command
termrock view @termrock/agent/prompt-composer
termrock add button badge command-palette
termrock add @termrock/agent/workbench
termrock diff command-palette
termrock update --interactive
termrock doctor
termrock story
termrock migrate
```

`termrock init` should itself be an excellent TermRock application with live theme, density and glyph previews.

### Manifest

A `termrock.toml` should record design choices and installed sources:

```toml
schema = 1
style = "phosphor-obsidian"
density = "compact"
glyphs = "unicode"
capabilities = "auto"

[registries]
official = "@termrock"
company = "@tailrocks"

[paths]
components = "src/ui"
blocks = "src/blocks"
stories = "stories"
```

Each installed item should also record origin, version, source hash and local modification state.

### Updating locally owned code

The update experience must never become “overwrite my files.”

`termrock diff` should compare:

1. The version originally installed.
2. The current upstream version.
3. The locally modified version.

An interactive update can then apply clean changes, present conflicts and preserve local ownership. This is the Rust equivalent of the source-first model that makes shadcn practical rather than simply being another dependency bundle. ([Shadcn][19])

---

## 6. A breaking API architecture for TermRock 1.0

### 6.1 Replace `Theme` with `DesignSystem`

Keep semantic roles, but place them inside a broader system:

```rust
pub struct DesignSystem {
    pub colors: ColorTokens,
    pub text: TextTokens,
    pub spacing: SpacingScale,
    pub density: DensityScale,
    pub borders: BorderTokens,
    pub glyphs: GlyphSet,
    pub motion: MotionTokens,
    pub breakpoints: Breakpoints,
    pub layers: LayerTokens,
    pub components: ComponentRecipes,
}
```

Important additions:

* **Spacing:** zero, compact, normal and spacious scales in cell units.
* **Density:** global default with per-component override.
* **Borders:** none, divider, light, strong, focused and destructive.
* **Glyphs:** Unicode, ASCII and optional enhanced/Nerd Font packs.
* **Motion:** spinner cadence, transition cadence, reduced-motion mode.
* **Breakpoints:** responsive policies rather than ad hoc width checks.
* **Component recipes:** part-level and state-level styling.

A component recipe should address anatomy, not only a single style:

```rust
pub struct ListRecipe {
    pub container: StateStyles,
    pub row: StateStyles,
    pub leading: StateStyles,
    pub label: StateStyles,
    pub metadata: StateStyles,
    pub indicator: StateStyles,
    pub selection_visual: SelectionVisual,
    pub row_padding: Insets,
}
```

Allow partial theme patches. A product should be able to replace only Menu selection or Dialog chrome without rebuilding all 38 semantic roles.

### 6.2 Introduce a per-frame `UiContext`

TermRock should remain compatible with Ratatui’s immediate-mode approach. It should **not** introduce a mandatory retained DOM or hide `Frame` and `Buffer`. Ratatui’s immediate rendering and buffer-diff model are strengths, while its ecosystem deliberately leaves application architecture to users. ([Ratatui][20])

A per-frame context can add the missing coordination:

```rust
pub struct UiContext<'a> {
    pub design: &'a DesignSystem,
    pub capabilities: &'a Capabilities,
    pub keymap: &'a Keymap,
    pub focus: &'a mut FocusGraph,
    pub overlays: &'a mut OverlayStack,
    pub scene: &'a mut SemanticScene,
    pub clock: FrameClock,
}
```

### 6.3 Evolve `FocusRing` into `FocusGraph`

Keep linear Tab order, but add:

* Focus zones.
* Parent/child scopes.
* Directional spatial navigation.
* Roving focus for collections.
* Modal trapping.
* Opener restoration.
* Focus history.
* Programmatic requests.
* Jump labels.

For a workbench, Tab should move between major regions while arrows navigate inside the region:

```text
Prompt → Scrollback → Task rail → Prompt
```

This is substantially more predictable than allowing every component to invent Tab behavior.

### 6.4 Add a semantic scene without adding a retained renderer

Each rendered interactive element should register lightweight semantics:

```rust
Element {
    id,
    parent,
    role,
    rect,
    label,
    description,
    state,
    actions,
    focusable,
}
```

The scene is rebuilt each frame, matching Ratatui’s immediate model.

This one structure enables:

* Hit-testing.
* Directional focus.
* A visual focus inspector.
* Jump mode.
* Automatic contextual hints.
* Command-palette action discovery.
* Accessibility-oriented text export.
* Interaction snapshots.
* Remote or web clients in the future.
* AI-readable UI state.

This could become one of TermRock’s strongest differentiators.

### 6.5 Standardize event results

Components should return typed messages and coordination requests without performing effects:

```rust
pub struct EventResult<M> {
    pub consumed: bool,
    pub message: Option<M>,
    pub redraw: Redraw,
    pub focus: Option<FocusRequest>,
    pub overlay: Option<OverlayRequest>,
}
```

The application remains responsible for database access, network requests, shell execution, persistence and other side effects.

### 6.6 Create one real `OverlayStack`

Dialogs, completion menus, popovers, tooltips, command palettes and fullscreen viewers should not each reinvent layering.

`OverlayStack` should own:

* Z-order.
* Placement and flipping.
* Screen-edge clamping.
* Backdrop policy.
* Focus trapping.
* Opener restoration.
* Pointer routing.
* Wheel routing.
* Outside-click dismissal.
* Escape behavior.
* Nested overlay unwinding.
* Fullscreen promotion.

The rule should be:

> **Escape closes exactly one conceptual layer.**

### 6.7 Separate headless behavior from styled views

Examples:

```text
CollectionState      → List, Menu, CommandPalette
ChoiceState          → RadioGroup, Select, QuestionFlow
SelectionModel       → Table, Tree, VirtualGrid
TextEditorState      → TextInput, TextArea, PromptComposer
OverlayState         → Popover, Dialog, Drawer, FullscreenViewer
ReviewState          → DiffReview, PlanReview
ActivityModel        → Progress, ToolCard, TaskRail
```

The source registry can then combine those behaviors into opinionated components without growing an enormous generic crate API.

---

## 7. Terminal-native luxury design direction

A luxurious terminal interface should not imitate a web dashboard cell-for-cell. It should exploit the terminal’s strengths:

* Monospaced alignment.
* Extremely fast keyboard operation.
* High information density.
* Streaming content.
* Fixed spatial relationships.
* Rich code and structured text.
* Direct access to commands.
* Low visual latency.

### Proposed visual laws

| Law                                       | Meaning                                                                               |
| ----------------------------------------- | ------------------------------------------------------------------------------------- |
| **Quiet canvas, bright intent**           | Accent appears at active decisions and live state, not everywhere                     |
| **Borders indicate ownership**            | Do not box every section; use borders when content has independent lifecycle or focus |
| **Selection and focus are different**     | Selected data can remain selected when another region has focus                       |
| **Every spinner has a verb**              | Show “Waiting for permission” or “Running tests,” not an unexplained animation        |
| **Every destructive action states scope** | “Delete session” is weaker than “Delete this local session and its checkpoints”       |
| **Primary text survives contraction**     | Remove metadata and actions before truncating the core label                          |
| **Color is supplementary**                | State must remain understandable through symbols, labels, weight or shape             |
| **No hidden hover-only behavior**         | Everything available through mouse must be available through keyboard                 |
| **Motion earns its redraw**               | No idle animation with no information value                                           |
| **One screen, one dominant action**       | Make the next meaningful decision visually obvious                                    |

### Default visual family

I would replace the current default with **Phosphor Obsidian**:

* Terminal-default or near-black canvas.
* Very subtle neutral surface and elevated layers.
* Slightly warm high-contrast primary text.
* Muted gray secondary text.
* Phosphor green reserved for focus, primary action and active/live state.
* Blue or cyan for links and informational context.
* Amber for waiting/review.
* Red only for destructive or failed state.
* Selection represented by a left rail plus soft tint, not a solid neon slab.

Retain these presets:

* `phosphor-obsidian`
* `slate`
* `paper`
* `ansi`
* `terminal-adaptive`

`terminal-adaptive` should preserve the user’s terminal background and derive only the foreground hierarchy and accents.

### Avoid “box soup”

Glow’s editorial clarity is an important reference: headings, indentation, whitespace, quote rails and carefully wrapped text often communicate hierarchy better than borders. ([Charm][14])

Reserve full boxes for:

* Modal ownership.
* Independent pane focus.
* Editable regions.
* Security or permission boundaries.
* Scrollable contained objects.
* Destructive confirmation.

Use headings, gutters, separators and alignment everywhere else.

### Responsive anatomy, not mere truncation

A responsive component should progressively change form:

1. Full label, metadata, trailing actions and help.
2. Compact label and shortened metadata.
3. Hide low-priority metadata.
4. Replace secondary actions with overflow/command palette.
5. Collapse multi-pane layout to a single region.
6. Fall back to an inline or line-mode interaction.

TermRock’s existing tiny-terminal contracts are an excellent starting point; the next step is to encode contraction as component recipes rather than one-off logic. ([GitHub][1])

---

## 8. Component collection TermRock should build

### Foundation pack

| Category     | Components                                                                   |
| ------------ | ---------------------------------------------------------------------------- |
| Content      | `Text`, `Heading`, `Label`, `Paragraph`, `Markdown`, `CodeBlock`, `AnsiText` |
| Identity     | `Icon`, `Badge`, `Tag`, `Chip`, `AvatarGlyph`, `Kbd`                         |
| Structure    | `Surface`, `Separator`, `Section`, `Callout`, `Alert`, `Well`                |
| Feedback     | `Spinner`, `ProgressBar`, `ProgressSteps`, `Skeleton`, `StatusDot`, `Toast`  |
| Empty states | `EmptyState`, `ErrorState`, `NoResults`, `OfflineState`                      |
| Layout       | `Stack`, `Inline`, `Grid`, `Center`, `Spacer`, `ScrollArea`, `AppShell`      |

### Input and navigation pack

| Category   | Components                                                                         |
| ---------- | ---------------------------------------------------------------------------------- |
| Actions    | `Button`, `IconButton`, `ButtonGroup`, `ActionMenu`                                |
| Selection  | `Checkbox`, `RadioGroup`, `Switch`, `SegmentedControl`                             |
| Entry      | `TextInput`, `TextArea`, `PasswordInput`, `NumberInput`, `PathInput`, `TokenField` |
| Choice     | `Select`, `MultiSelect`, `Combobox`, `Autocomplete`, `Picker`                      |
| Navigation | `Tabs`, `Sidebar`, `Breadcrumbs`, `Pagination`, `TreeNav`, `HistoryPicker`         |
| Discovery  | `CommandPalette`, `QuickOpen`, `JumpMode`, `KeyboardHelp`                          |

### Overlay pack

```text
Dialog
AlertDialog
Popover
Tooltip
HoverCard
ContextMenu
Menu
Drawer / Sheet
CompletionMenu
FullscreenViewer
```

### Data and developer pack

| Component         | Important behavior                                                               |
| ----------------- | -------------------------------------------------------------------------------- |
| `DataTable`       | Sticky headers, pinning, sorting, filtering, column resizing, row/cell selection |
| `TreeTable`       | Hierarchy plus columns, lazy loading and expansion                               |
| `ObjectInspector` | JSON/YAML/TOML/tree data with type-aware values                                  |
| `KeyValueTable`   | Dense metadata and settings                                                      |
| `LogStream`       | Follow-tail, pause, filter, search, ANSI, new-lines indicator                    |
| `Timeline`        | Grouped events, durations, status and expansion                                  |
| `DiffReview`      | File tree, hunks, syntax, line comments and decisions                            |
| `Diagnostic`      | Severity, source span, code frame, fix and documentation link                    |
| `Metrics`         | Sparkline, bar, histogram, gauge and time series                                 |
| `HexView`         | Offsets, bytes, text and selection                                               |
| `TerminalOutput`  | stdout/stderr distinction, exit status, elapsed time and truncation              |

The current Table, Tree, VirtualGrid, DetailTable, LogPane and DiffView should be evolved rather than discarded. They already provide much of the difficult state and geometry work.

---

## 9. The flagship: `@termrock/agent`

AI-agent UI should be TermRock’s most visible differentiator. Generic TUI libraries already provide lists, forms and tables. Very few provide reusable, polished **agent interaction blocks**.

### Prompt and conversation

| Block               | Required behavior                                                                                                       |
| ------------------- | ----------------------------------------------------------------------------------------------------------------------- |
| `PromptComposer`    | Multiline editing, mode/model indicator, slash completion, file mentions, history, queue, attachments and submit/cancel |
| `AttachmentChip`    | File, image, pasted text, URL or selected code with remove/open actions                                                 |
| `PasteChip`         | Collapses large pasted content while preserving inspectability                                                          |
| `MessageThread`     | User, assistant, tool and system entries with stable anchors                                                            |
| `StreamingMarkdown` | Partial Markdown, unfinished code fences, incremental wrapping and stable scrolling                                     |
| `ContextMeter`      | Token budget, compaction state, included sources and expandable breakdown                                               |

### Agent activity

| Block                 | Required behavior                                                               |
| --------------------- | ------------------------------------------------------------------------------- |
| `ToolCallCard`        | Queued, running, waiting, success and failure; compact and expanded views       |
| `TerminalRunCard`     | Live stdout/stderr, elapsed time, exit code, cancel, detach and open externally |
| `ActivityShelf`       | Compact list of currently active operations                                     |
| `TaskRail`            | Workflows, subagents, tasks and watchers with grouping and filtering            |
| `SubagentCard`        | Role, task, context, elapsed time, output preview, steer and cancel             |
| `BackgroundTaskPanel` | Persistent output, follow-tail, pause, open and terminate                       |

### Human decision surfaces

| Block                | Required behavior                                                                 |
| -------------------- | --------------------------------------------------------------------------------- |
| `PlanReview`         | Markdown plan, line selection, inline comments, approve, revise and abandon       |
| `QuestionFlow`       | Multiple questions, tabs, single/multiple choice, free-form answer and validation |
| `PermissionPrompt`   | Origin, exact operation, target, risk, scope, details and allow/deny choices      |
| `DiffReview`         | File navigation, hunk decisions, comments, external editor and apply/reject       |
| `CheckpointTimeline` | Rewindable turns, file states, labels and irreversible-boundary warnings          |
| `SessionPicker`      | Search, recency, project, status, branch and resume/delete actions                |

### Permission UX should become a signature TermRock capability

A permission prompt should communicate:

```text
WHO        Main agent / subagent / plugin / MCP server
WHAT       Exact command, file operation, network request or tool
WHERE      Local machine, remote service, project or global config
DATA       Files, arguments or secrets involved
SCOPE      Once / this session / this project / always
RISK       Informational / review / destructive / external data transfer
RESULT     What will become possible after approval
```

Recent reporting about Grok Build uploading more repository data than users expected reinforces why data movement and destination must be explicit UI concepts rather than hidden behind a generic “allow tool?” question. ([Axios][21])

TermRock could introduce reusable types such as:

```rust
pub enum PermissionScope {
    Once,
    Session,
    Project,
    Global,
}

pub enum RiskLevel {
    Informational,
    Review,
    Destructive,
    DataEgress,
}

pub struct Provenance {
    pub actor: ActorId,
    pub parent: Option<ActorId>,
    pub plugin: Option<PluginId>,
}
```

This is more than visual polish. It is trust architecture.

---

## 10. Five signature interaction ideas that could make TermRock distinctive

| Signature         | Experience                                                                                                       |
| ----------------- | ---------------------------------------------------------------------------------------------------------------- |
| **Focus Lens**    | A temporary overlay labels focusable regions and shows the most important action for each                        |
| **Semantic Zoom** | Space or Enter expands the selected object inline; another action promotes it to fullscreen without losing state |
| **Action Lens**   | Command palette automatically narrows to actions valid for the current focus and selection                       |
| **Trust Surface** | Permissions and external operations always reveal origin, scope, destination and risk                            |
| **Replayable UI** | Interactions can be recorded and deterministically replayed for tests, bug reports, demos and documentation      |

### Focus Lens

Posting’s jump-style navigation and Zellij’s visible mode model suggest a powerful TermRock feature: press a configurable leader chord and temporarily place one- or two-character labels over all relevant semantic scene nodes. Selecting a label jumps directly to that region. ([GitHub][15])

Because the semantic scene already knows IDs, rectangles, roles and available actions, this would not require every application to implement custom jump navigation.

### Semantic Zoom

A task, tool call, diff, log entry or object inspector should support the same progression:

```text
compact row → expanded inline detail → fullscreen viewer
```

Focus, selection and scroll anchor should survive transitions. Grok Build’s repeated fullscreen and overlay refinements show how valuable this interaction is in dense agent interfaces. ([SpaceXAI][22])

### Replayable UI

TermRock already has deterministic frame ticks and a real lookbook. Add an event recorder:

```text
terminal capabilities
initial state
resize events
key/mouse/paste events
frame ticks
application messages
buffer snapshots
semantic scene snapshots
```

A bug report could become:

```bash
termrock replay issue-142.trock
```

The same recording could generate an animated documentation example and a regression test.

---

## 11. A north-star application: TermRock Workbench

TermRock needs one flagship that proves the system can create a category-leading experience. An AI workbench is the best candidate because it exercises nearly every hard interaction.

```text
 project / branch       BUILD       model       context 63%       connected
──────────────────────────────────────────────────────────────────────────────
│                                                                            │
│  You                                                                       │
│  Refactor the authorization layer and add tests.                           │
│                                                                            │
│  Agent                                                                     │
│  I found three call paths that bypass the policy cache…                    │
│                                                                            │
│  ┃ Running tests                                           18s             │
│  ┃ cargo test -p auth                                                     │
│  ┃ 127 passed · 1 failed                                  [open] [stop]    │
│                                                                            │
│  Proposed changes                                                         │
│  M src/auth/policy.rs                                     +42 -18          │
│  M src/auth/cache.rs                                      +11  -4          │
│                                                                            │
├──────────────────────────────────────────────────────┬─────────────────────┤
│ [src/auth/policy.rs] [pasted text ×]                 │ TASKS               │
│                                                     │ ▾ Subagents 2       │
│ Ask anything…                                      │   ◌ Explore… 12s    │
│                                                     │   ✓ Tests    31s    │
│ BUILD · Enter send · Shift+Enter newline · / command│ ▾ Tasks 1           │
└──────────────────────────────────────────────────────┴─────────────────────┘
```

Interaction rules:

* Tab cycles Composer → Scrollback → Task rail.
* Arrows navigate within a focused region.
* Escape closes one overlay or exits one nested interaction mode.
* `?` opens contextual help.
* The command palette contains all discoverable actions.
* Jump mode exposes direct focus labels.
* Space performs semantic zoom.
* Tool output does not steal scroll position when the user is reading history.
* New streamed content displays a “new content” indicator when follow-tail is paused.
* Task rail collapses into a drawer at medium widths and becomes a status strip on narrow terminals.
* The composer always preserves draft text when a permission, plan or question temporarily replaces it.

This should be a complete registry block, not a tightly coupled framework application.

---

## 12. Multi-surface and plugin architecture

Amp, OpenCode and Grok Build all point toward the same architectural direction: the terminal UI increasingly acts as one client of a longer-lived session or agent engine rather than owning the entire process. Amp emphasizes remote control and plugin-provided UI; OpenCode serves terminal, desktop and IDE experiences; Grok Build supports headless and ACP-based clients. ([Ampcode][8])

TermRock should not build an agent engine. It should make its blocks compatible with this architecture.

A longer-term `termrock-protocol` could serialize high-level interaction requests:

```rust
pub enum UiRequest {
    Notify(Notification),
    Confirm(Confirmation),
    Input(TextPrompt),
    Select(SelectPrompt),
    ReviewDiff(DiffReviewRequest),
    ReviewPlan(PlanReviewRequest),
    RequestPermission(PermissionRequest),
}
```

This could support:

* Plugin-contributed UI.
* Remote terminal clients.
* Web companions.
* Test harnesses.
* Headless automation.
* IDE integration.

The protocol should describe meaningful components and outcomes, not remote-control raw terminal cells.

---

## 13. Capability profiles and graceful reduction

TermRock should introduce explicit capability profiles:

| Profile      | Expected environment                                     | Behavior                                              |
| ------------ | -------------------------------------------------------- | ----------------------------------------------------- |
| `Modern`     | Truecolor, Unicode, mouse, OSC, modern keyboard protocol | Full visual treatment and optional media              |
| `Compatible` | 256 colors, Unicode, conventional keyboard and mouse     | Reduced palette and conservative glyphs               |
| `Minimal`    | ANSI 8/16 or no color, ASCII, no mouse                   | Text, symbols, spacing and labels carry all meaning   |
| `Inline`     | Normal shell scrollback; no alternate screen             | Prompts, forms, progress, pickers and compact widgets |
| `Headless`   | Non-interactive output or machine client                 | Structured/static rendering and serializable outcomes |

`NO_COLOR` is an established hint that applications should suppress color, so it should become a first-class profile override rather than an unsupported environment. ([No Color][23])

Modern protocols should be detected rather than assumed. The Kitty keyboard protocol explicitly supports capability detection; its graphics protocol enables raster media; and its newer text-sizing protocol offers optional typographic hierarchy. These should be progressive enhancements, never baseline dependencies. ([Kovid Goyal][24])

Recommended capabilities model:

```rust
pub struct Capabilities {
    pub color: ColorDepth,
    pub unicode: UnicodeLevel,
    pub keyboard: KeyboardProtocol,
    pub mouse: MouseCapabilities,
    pub clipboard: ClipboardCapabilities,
    pub hyperlinks: bool,
    pub images: ImageProtocol,
    pub text_sizing: bool,
    pub synchronized_output: bool,
    pub alternate_screen: bool,
}
```

Every optional capability needs:

* Detection.
* Explicit user override.
* A fallback.
* A story in TermRock Studio.
* A contract test.

`termrock doctor` should display the negotiated result and render a live sample of each fallback.

---

## 14. Fullscreen TUI, inline TUI and ordinary CLI output

TermRock should not define “terminal UI” as only an alternate-screen application.

Bubble Tea supports inline and full-window modes, while Rich demonstrates how much value comes from excellent non-interactive tables, Markdown, progress, syntax and diagnostics. Textual adds reactive widgets, workers, themes, command palettes and strong developer tooling. Ink demonstrates familiar component composition over terminal layout. ([GitHub][25])

TermRock should support:

```rust
pub enum RenderMode {
    Static,
    Inline,
    Fullscreen,
}
```

This unlocks:

* Interactive installation wizards.
* Inline fuzzy pickers.
* Confirmation prompts.
* Progress and task views that preserve shell history.
* Fullscreen workbenches.
* CI-friendly static diagnostics.
* The same design language across CLI and TUI modes.

The source registry can provide variants of the same conceptual component rather than forcing one renderer into every environment.

---

## 15. TermRock Studio: the developer experience moat

The existing lookbook should evolve into **TermRock Studio**.

Each component page should expose:

* Visual anatomy.
* Source code.
* Public API.
* Registry installation command.
* All variants and sizes.
* Interactive state controls.
* Keyboard and mouse map.
* Narrow-layout behavior.
* Unicode and ASCII comparison.
* Truecolor, 256-color, ANSI and no-color modes.
* Focus-order visualization.
* Hit-region visualization.
* Semantic-scene inspector.
* Event and message log.
* Performance metrics.
* Buffer snapshot.
* Upstream/local diff.
* Contract status.

### Studio modes

```text
Browse      component and block registry
Inspect     focus, hit zones, semantics and events
Theme       edit tokens and component recipes live
Resize      test widths and heights
Capability  emulate terminal features
Record      produce deterministic demos and tests
Compare     visual and semantic snapshot diff
```

The Studio should itself use only public TermRock APIs. Any special internal hook needed by Studio is a sign that the public developer experience is incomplete.

---

## 16. Quality contracts should become a defining feature

TermRock already documents keyboard, mouse, focus, narrow, Unicode and non-color expectations. That is an unusually good foundation. ([GitHub][26])

Expand the matrix to include:

| Axis        | Required cases                                                               |
| ----------- | ---------------------------------------------------------------------------- |
| Color       | None, ANSI 8/16, 256, truecolor, dark and light                              |
| Glyphs      | ASCII, Unicode, emoji, CJK, combining marks, ambiguous width                 |
| Input       | Conventional keys, enhanced keyboard protocol, paste, mouse, wheel and drag  |
| Environment | Local, SSH, tmux/screen, alternate-screen and inline                         |
| Platform    | Linux, macOS and Windows/ConPTY                                              |
| Layout      | Wide, standard, narrow, tiny and live resize                                 |
| Motion      | Normal and reduced                                                           |
| Streaming   | Slow, bursty, very large and interrupted output                              |
| Focus       | Keyboard, pointer, disabled reconciliation, modal nesting and opener restore |
| Selection   | Text selection, row/cell selection and native terminal copy                  |
| Failure     | Panics, terminal disconnect, partial initialization and restoration          |

### Testing layers

1. **Buffer snapshots** for exact cells and styles.
2. **ANSI snapshots** for emitted sequences.
3. **SVG previews** for documentation.
4. **Semantic snapshots** for roles, labels, actions and focus order.
5. **Interaction traces** for state transitions.
6. **PTY tests** for real terminal lifecycle and key behavior.
7. **Property tests** for layout bounds, scrolling and selection.
8. **Unicode fuzzing** for editing and wrapping.
9. **Performance budgets** for large lists, logs, diffs and streaming Markdown.
10. **Design linting** for color-only states, invisible focus, missing glyph fallback and clipped primary labels.

Grok Build’s repository includes a dedicated PTY harness package alongside its pager, which is a good indication of how seriously production terminal software must treat real terminal integration rather than relying only on buffer tests. ([GitHub][27])

---

## 17. Recommended implementation roadmap

| Phase                         | Outcome                                                                                                   |
| ----------------------------- | --------------------------------------------------------------------------------------------------------- |
| **1. Category reset**         | Publish the product definition, registry model, design principles and breaking architecture RFC           |
| **2. Core v2**                | `DesignSystem`, `UiIntent`, `EventResult`, `UiContext`, `FocusGraph`, `OverlayStack`, capability profiles |
| **3. Registry and CLI**       | `init`, `add`, `search`, `view`, dependency resolution, manifests and private namespaces                  |
| **4. Visual rebuild**         | Phosphor Obsidian, Slate, Paper and ANSI themes; component recipes; responsive density                    |
| **5. Foundation components**  | Content, surfaces, actions, selection, menus, command palette, popover, tooltip and drawer                |
| **6. Studio**                 | Inspector, themes, capability simulation, event traces, snapshots and registry browsing                   |
| **7. Agent pack**             | Composer, messages, tool cards, terminal cards, permissions, plans, questions, tasks, diffs and sessions  |
| **8. Flagship application**   | TermRock Workbench built entirely with public APIs and registry blocks                                    |
| **9. Ecosystem**              | Community/private registries, interactive updates, source provenance and registry validation              |
| **10. Broader compatibility** | Windows/ConPTY, inline mode, no-color, improved multiplexers and remote clients                           |

### The first six concrete PRs

| PR    | Scope                                                                                |
| ----- | ------------------------------------------------------------------------------------ |
| **1** | RFC defining TermRock as core + registry + Studio rather than only a crate           |
| **2** | Replace `Theme` with `DesignSystem`; ship Phosphor Obsidian and migration tooling    |
| **3** | Add `UiIntent`; migrate List and one other collection away from hardcoded keys       |
| **4** | Add `OverlayStack`; migrate Dialog, CompletionMenu and Picker placement              |
| **5** | Introduce `termrock.toml`, registry schema and `termrock init/add/diff`              |
| **6** | Build `PromptComposer` plus a minimal Agent Workbench block as the architecture test |

Do not begin by adding 40 miscellaneous widgets. The design system, registry model, semantic input and overlay architecture must come first, or every additional component will increase migration cost.

---

## 18. Things TermRock should explicitly avoid

1. **Do not copy web-component aesthetics literally.** Rounded-card imitation and excessive padding waste terminal space.
2. **Do not make every component a mandatory crate abstraction.** Opinionated source should be locally owned.
3. **Do not force Elm, actors or another application architecture.** Provide clean outcomes and adapters.
4. **Do not copy Grok Build’s product-specific monoliths.** Extract small behaviors and reusable blocks.
5. **Do not use accent color as the only hierarchy tool.**
6. **Do not require Nerd Fonts for core meaning.**
7. **Do not capture mouse input without explaining native text-selection behavior.**
8. **Do not animate at idle.**
9. **Do not treat a screenshot as component quality.** Interaction contracts are part of design.
10. **Do not preserve an early API solely because it already exists.**

---

## Final recommendation

TermRock’s strongest asset is not its phosphor aesthetic or even its current widget count. It is the fact that the project already understands the invisible parts of terminal quality: identity, focus, lifecycle, Unicode, geometry, resizing, virtualization and testing.

The winning strategy is to retain that work as a **small, extremely dependable interaction kernel**, then build a shadcn-style source ecosystem above it:

```text
Stable kernel
    +
Owned source components
    +
Opinionated application blocks
    +
A complete terminal design system
    +
World-class Studio and contracts
    +
An industry-leading AI-agent component pack
```

That combination would occupy a space that Ratatui, Textual, Bubble Tea, Ink and current application-specific AI TUIs do not fully address. Ratatui provides rendering primitives; Textual provides an integrated application framework; Grok, Amp and OpenCode provide polished products. **TermRock can become the reusable design and interaction layer between those categories.**

The most important immediate decision is therefore not which widget to add next. It is to commit to the category:

> **TermRock is not another Ratatui widget crate. It is the source-owned design system for building exceptional terminal software.**

[1]: https://raw.githubusercontent.com/tailrocks/termrock/main/crates/termrock/COMPONENTS.md "https://raw.githubusercontent.com/tailrocks/termrock/main/crates/termrock/COMPONENTS.md"
[2]: https://ui.shadcn.com/docs/registry?utm_source=chatgpt.com "Introduction - Shadcn UI"
[3]: https://raw.githubusercontent.com/tailrocks/termrock/main/README.md "https://raw.githubusercontent.com/tailrocks/termrock/main/README.md"
[4]: https://github.com/tailrocks/termrock/tree/main/crates/termrock-lookbook "termrock/crates/termrock-lookbook at main · tailrocks/termrock · GitHub"
[5]: https://raw.githubusercontent.com/tailrocks/termrock/main/crates/termrock/src/style/mod.rs "https://raw.githubusercontent.com/tailrocks/termrock/main/crates/termrock/src/style/mod.rs"
[6]: https://awesometui.com/awards "https://awesometui.com/awards"
[7]: https://x.ai/cli "https://x.ai/cli"
[8]: https://ampcode.com/news/neo "https://ampcode.com/news/neo"
[9]: https://opencode.ai/ "https://opencode.ai/"
[10]: https://github.com/aristocratos/btop?utm_source=chatgpt.com "aristocratos/btop: A monitor of resources"
[11]: https://github.com/jesseduffield/lazygit?utm_source=chatgpt.com "jesseduffield/lazygit: simple terminal UI for git commands"
[12]: https://github.com/sxyazi/yazi?utm_source=chatgpt.com "sxyazi/yazi: 💥 Blazing fast terminal file manager written in ..."
[13]: https://github.com/zellij-org/zellij?utm_source=chatgpt.com "zellij-org/zellij: A terminal workspace with batteries included"
[14]: https://charm.sh/?utm_source=chatgpt.com "Charm"
[15]: https://github.com/darrenburns/posting?utm_source=chatgpt.com "darrenburns/posting: The modern API client that lives in ..."
[16]: https://github.com/xai-org/grok-build "https://github.com/xai-org/grok-build"
[17]: https://github.com/xai-org/grok-build/tree/main/crates/codegen/xai-grok-pager/src/views "grok-build/crates/codegen/xai-grok-pager/src/views at main · xai-org/grok-build · GitHub"
[18]: https://github.com/xai-org/grok-build/blob/main/crates/codegen/xai-grok-pager/src/views/permission_view.rs "grok-build/crates/codegen/xai-grok-pager/src/views/permission_view.rs at main · xai-org/grok-build · GitHub"
[19]: https://ui.shadcn.com/docs/registry/getting-started?utm_source=chatgpt.com "Getting Started - Shadcn UI"
[20]: https://ratatui.rs/concepts/rendering/ "https://ratatui.rs/concepts/rendering/"
[21]: https://www.axios.com/newsletters/axios-future-of-cybersecurity-9168e100-7af2-11f1-bc32-bbfb768a7518 "https://www.axios.com/newsletters/axios-future-of-cybersecurity-9168e100-7af2-11f1-bc32-bbfb768a7518"
[22]: https://x.ai/build/changelog "https://x.ai/build/changelog"
[23]: https://no-color.org/ "https://no-color.org/"
[24]: https://sw.kovidgoyal.net/kitty/keyboard-protocol/ "https://sw.kovidgoyal.net/kitty/keyboard-protocol/"
[25]: https://github.com/charmbracelet/bubbletea/blob/main/README.md?utm_source=chatgpt.com "README.md - charmbracelet/bubbletea"
[26]: https://raw.githubusercontent.com/tailrocks/termrock/main/docs/api/component-contracts.v2.json "TermRock catalog quality contracts"
[27]: https://github.com/xai-org/grok-build/tree/main/crates/codegen "https://github.com/xai-org/grok-build/tree/main/crates/codegen"
