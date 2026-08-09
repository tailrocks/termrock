# TermRock Complete Component Prompt Library

Repository: `https://github.com/tailrocks/termrock`

## How to use this library

Each component prompt below is designed to be combined with the **Global implementation contract**. Paste the global contract first, then paste one component prompt. This avoids repeating several pages of non-negotiable requirements while keeping every task precise.

The prompts intentionally permit breaking changes. A component is not complete merely because it renders: its interaction model, responsive contraction, terminal capability fallbacks, tests, stories, documentation, and source-installation experience are part of the component.

Priorities:

- **P0 — Foundation:** required before a coherent component system can exist.
- **P1 — Core:** expected in the first high-quality public release.
- **P2 — Advanced:** important for serious developer tools and full applications.
- **P3 — Specialized:** differentiators, flagship blocks, and ecosystem expansion.

## Global implementation contract

```text
You are working directly on TermRock:
https://github.com/tailrocks/termrock

Mission:
Make TermRock the shadcn/ui equivalent for TUI and CLI applications: a source-owned, terminal-native design system with Radix-level interaction rigor, excellent Rust APIs, exceptional visual quality, and production-grade behavior on top of Ratatui.

Before changing code, inspect the complete repository: public APIs, implementations, state types, examples, documentation, tests, screenshots, lookbook stories, theme architecture, focus handling, mouse routing, Unicode behavior, responsive behavior, and every call site of the target component.

Breaking changes are allowed. Do not preserve a weak API merely for compatibility.

Non-negotiable architecture rules:
- Keep terminal lifecycle, focus, geometry, Unicode, capabilities, and other stable infrastructure in compiled crates.
- Prefer source-installed, user-owned code for opinionated styled components and application blocks.
- Keep application side effects and business policy outside reusable components.
- Components consume semantic UI intents rather than hardcoded physical keys.
- Components return typed messages/outcomes and coordination requests.
- Every interactive element registers semantic role, label, state, actions, rectangle, and focusability.
- Use the shared DesignSystem and component recipes; do not hardcode visual styling.
- Focus and selection must be distinct concepts and distinct visuals.
- Mouse actions must have keyboard equivalents.
- Color must never be the only carrier of meaning.
- Every glyph requires an ASCII fallback.
- Every component must contract intentionally at narrow and tiny terminal sizes.
- Optional terminal capabilities must use progressive enhancement with deterministic fallbacks.
- Idle components must not animate or trigger unnecessary redraws.
- Avoid web-style box soup; borders indicate ownership, focus, or interaction boundaries.
- Do not add a generic abstraction unless at least two real components benefit from it.

Required process:
1. Audit the existing implementation and related primitives.
2. Research equivalent components in shadcn/ui, Radix UI, Ratatui applications, Textual, Bubble Tea/Bubbles, Ink, prompt-toolkit, FTXUI, and respected production TUIs where relevant.
3. Write a concise design specification before implementation.
4. Define anatomy, state machine, public API, typed outcomes, focus behavior, keyboard behavior, mouse behavior, responsive contraction, capability fallbacks, and performance expectations.
5. Implement the component and migrate every call site.
6. Add realistic Studio/lookbook stories for normal, focused, selected, disabled, loading, empty, error, narrow, tiny, Unicode, ASCII, no-color, reduced-color, and stress states where applicable.
7. Add buffer snapshots, semantic-scene snapshots, interaction traces, property tests, PTY tests, fuzzing, and benchmarks as appropriate.
8. Update documentation with anatomy, examples, keyboard map, mouse behavior, composition, theming, common mistakes, migration notes, and performance notes.
9. Run formatting, linting, tests, and relevant benchmarks.
10. Perform a second visual and interaction review after the first implementation pass.

Definition of done:
- The component feels intentional and premium rather than like a widget demo.
- The public API is understandable, composable, and difficult to misuse.
- Keyboard-only, mouse-only, no-color, ASCII, narrow-terminal, and Unicode experiences are credible.
- Escape closes exactly one conceptual layer when overlays are involved.
- The component can be installed or represented through the TermRock registry with provenance, stories, dependencies, and migration metadata.
- No known interaction or visual defect is hidden behind “future work” when it is essential to the component.

At completion report:
- Design decisions and rejected alternatives.
- Breaking changes and migration examples.
- Files changed.
- Stories and tests added.
- Commands run.
- Performance results where relevant.
- Remaining non-blocking limitations.
- A strict quality score from 1–10, with concrete work required for any score below 9.
```

---


# A. Foundational interaction and design primitives

These are not decorative widgets. They are the contracts that prevent every visible component from reinventing focus, input, overlays, semantics, responsiveness, and capability handling.

## 1. DesignSystem and component recipes — P0

```text
Using the TermRock Global implementation contract, redesign and implement: DesignSystem and component recipes.

Component mission:
Replace the narrow theme abstraction with a complete terminal design system.

Component-specific requirements:
- Define semantic foreground, surface, intent, syntax, diff, chart, border, spacing, density, glyph, motion, breakpoint, and elevation tokens.
- Support component-part recipes, state variants, partial overrides, inheritance, runtime switching, and user-owned theme packages.
- Ship Phosphor Obsidian, Slate, Paper, ANSI, terminal-adaptive, and high-contrast presets.
- Guarantee truecolor, 256-color, ANSI 8/16, no-color, Unicode, and ASCII behavior.
- Migrate all existing style roles and remove component-local hardcoded styles.

Research direction:
Study shadcn token organization, Radix state anatomy, Textual themes, Lip Gloss style composition, and the visual restraint of Glow, btop, and Posting.

Before implementation, state which existing TermRock types and call sites will be preserved, migrated, split, or deleted. Produce concrete Rust API sketches, implement the component, migrate its usages, add Studio stories and the appropriate visual, semantic, interaction, PTY, fuzz, and performance tests, then perform an independent second-pass design review.
```

## 2. UiContext — P0

```text
Using the TermRock Global implementation contract, redesign and implement: UiContext.

Component mission:
Create the per-frame coordination object shared by all TermRock components without replacing Ratatui's immediate-mode rendering.

Component-specific requirements:
- Expose design system, capabilities, semantic intents, keymap, focus graph, overlay stack, semantic scene, frame clock, and diagnostics.
- Keep Frame and Buffer directly usable; do not create a mandatory retained DOM.
- Define borrowing and lifetime ergonomics that remain pleasant in real applications.
- Make nested component composition and testing straightforward.
- Provide adapters for current render and event APIs.

Research direction:
Compare Ratatui's immediate-mode model with Textual's app context, Elm-style update loops, and Radix context composition.

Before implementation, state which existing TermRock types and call sites will be preserved, migrated, split, or deleted. Produce concrete Rust API sketches, implement the component, migrate its usages, add Studio stories and the appropriate visual, semantic, interaction, PTY, fuzz, and performance tests, then perform an independent second-pass design review.
```

## 3. SemanticScene — P0

```text
Using the TermRock Global implementation contract, redesign and implement: SemanticScene.

Component mission:
Build a lightweight semantic tree rebuilt every frame alongside rendering.

Component-specific requirements:
- Register element IDs, parent relationships, roles, labels, descriptions, states, actions, rectangles, focusability, and disabled status.
- Use it for hit-testing, focus navigation, generated help, jump mode, inspection, semantic snapshots, remote clients, and AI-readable UI state.
- Define stable identity and collision diagnostics.
- Keep construction cheap enough for large virtualized views.
- Add a Studio inspector and semantic snapshot format.

Research direction:
Use accessibility-tree concepts and browser devtools semantics as inspiration, while remaining terminal-native and frame-local.

Before implementation, state which existing TermRock types and call sites will be preserved, migrated, split, or deleted. Produce concrete Rust API sketches, implement the component, migrate its usages, add Studio stories and the appropriate visual, semantic, interaction, PTY, fuzz, and performance tests, then perform an independent second-pass design review.
```

## 4. UiIntent and Keymap — P0

```text
Using the TermRock Global implementation contract, redesign and implement: UiIntent and Keymap.

Component mission:
Normalize raw terminal input into semantic intentions so components never own application-specific physical keys.

Component-specific requirements:
- Model navigation, activation, toggle, open, close, cancel, submit, edit, search, help, fullscreen, jump, and application commands.
- Support default, Vim-style, Emacs-style editing, context-sensitive, and user-remapped keymaps.
- Generate footer hints, keyboard help, command-palette entries, and conflict diagnostics from one source of truth.
- Support conventional and enhanced keyboard protocols.
- Migrate representative existing components and remove direct key matching from them.

Research direction:
Study Zellij modes, lazygit contextual keys, Textual bindings, prompt-toolkit keymaps, and editor command systems.

Before implementation, state which existing TermRock types and call sites will be preserved, migrated, split, or deleted. Produce concrete Rust API sketches, implement the component, migrate its usages, add Studio stories and the appropriate visual, semantic, interaction, PTY, fuzz, and performance tests, then perform an independent second-pass design review.
```

## 5. EventResult and typed component outcomes — P0

```text
Using the TermRock Global implementation contract, redesign and implement: EventResult and typed component outcomes.

Component mission:
Standardize how components report consumed input, messages, redraw needs, focus requests, overlay requests, and semantic actions.

Component-specific requirements:
- Design an ergonomic generic EventResult or equivalent without forcing a global application architecture.
- Differentiate domain messages from framework coordination requests.
- Support bubbling, capture, cancellation, and nested composites.
- Make outcomes deterministic and easy to interaction-test.
- Migrate several current components to prove the abstraction.

Research direction:
Compare Elm messages, Bubble Tea commands, Textual messages, and Radix event contracts without copying their architecture wholesale.

Before implementation, state which existing TermRock types and call sites will be preserved, migrated, split, or deleted. Produce concrete Rust API sketches, implement the component, migrate its usages, add Studio stories and the appropriate visual, semantic, interaction, PTY, fuzz, and performance tests, then perform an independent second-pass design review.
```

## 6. FocusGraph — P0

```text
Using the TermRock Global implementation contract, redesign and implement: FocusGraph.

Component mission:
Evolve focus handling into a predictable graph supporting complex workbenches.

Component-specific requirements:
- Support linear Tab order, directional spatial navigation, focus zones, parent-child scopes, modal traps, opener restoration, and focus history.
- Preserve roving focus inside collections while exposing one external focus target when appropriate.
- Handle disabled reconciliation, dynamic registration, virtualization, and programmatic requests.
- Add Focus Lens and debug visualization.
- Define exact rules for focus versus selection and pointer focus.

Research direction:
Study Radix focus scopes, Zellij pane navigation, terminal editor focus modes, and current TermRock focus behavior.

Before implementation, state which existing TermRock types and call sites will be preserved, migrated, split, or deleted. Produce concrete Rust API sketches, implement the component, migrate its usages, add Studio stories and the appropriate visual, semantic, interaction, PTY, fuzz, and performance tests, then perform an independent second-pass design review.
```

## 7. RovingFocusGroup — P0

```text
Using the TermRock Global implementation contract, redesign and implement: RovingFocusGroup.

Component mission:
Create a reusable behavior primitive for menus, tabs, radio groups, toolbars, segmented controls, and collections.

Component-specific requirements:
- Support orientation, wrapping, disabled items, Home/End, typeahead, directionality, and stable active item IDs.
- Separate active descendant state from external focus ownership.
- Work with virtualized children and dynamic insertion/removal.
- Expose semantic scene information and generated key hints.
- Provide property tests for item changes and disabled reconciliation.

Research direction:
Use Radix RovingFocusGroup as a behavioral reference, adapted to terminal navigation and immediate-mode registration.

Before implementation, state which existing TermRock types and call sites will be preserved, migrated, split, or deleted. Produce concrete Rust API sketches, implement the component, migrate its usages, add Studio stories and the appropriate visual, semantic, interaction, PTY, fuzz, and performance tests, then perform an independent second-pass design review.
```

## 8. CollectionState — P0

```text
Using the TermRock Global implementation contract, redesign and implement: CollectionState.

Component mission:
Define a reusable headless collection model for lists, menus, command palettes, pickers, trees, and tables.

Component-specific requirements:
- Support stable IDs, ordering, disabled items, filtering, typeahead, active item, current item, and virtualization metadata.
- Avoid storing borrowed display data in long-lived state.
- Define reconciliation when items appear, disappear, reorder, or become disabled.
- Keep hierarchy optional rather than forcing tree semantics onto flat collections.
- Prove reuse across at least three components.

Research direction:
Compare Radix collections, React Aria collection models, Textual widgets, and Ratatui state patterns.

Before implementation, state which existing TermRock types and call sites will be preserved, migrated, split, or deleted. Produce concrete Rust API sketches, implement the component, migrate its usages, add Studio stories and the appropriate visual, semantic, interaction, PTY, fuzz, and performance tests, then perform an independent second-pass design review.
```

## 9. SelectionModel — P0

```text
Using the TermRock Global implementation contract, redesign and implement: SelectionModel.

Component mission:
Create consistent single, multiple, range, row, cell, and hierarchical selection behavior.

Component-specific requirements:
- Separate focus, active cursor, current item, checked state, and selected state.
- Support anchor-based range selection, select-all, inversion, disabled items, and application-controlled selection.
- Define typed selection changes and stable-ID semantics.
- Handle virtualization and filtered views without corrupting selection.
- Provide visual recipe requirements that do not rely on color alone.

Research direction:
Study desktop list/table selection, VisiData, lazygit, file managers, and accessible web selection semantics.

Before implementation, state which existing TermRock types and call sites will be preserved, migrated, split, or deleted. Produce concrete Rust API sketches, implement the component, migrate its usages, add Studio stories and the appropriate visual, semantic, interaction, PTY, fuzz, and performance tests, then perform an independent second-pass design review.
```

## 10. ScrollArea — P0

```text
Using the TermRock Global implementation contract, redesign and implement: ScrollArea.

Component mission:
Create the canonical scrolling primitive used by every scrollable TermRock component.

Component-specific requirements:
- Support vertical and horizontal axes, wheel routing, page movement, scrollbars, follow-tail, paused follow-tail, and new-content indicators.
- Define stable anchors across streamed content, resize, reflow, and item insertion.
- Support nested scroll areas and explicit scroll chaining policy.
- Expose visible ranges for virtualization and semantic inspection.
- Add huge-content benchmarks and Unicode wrapping tests.

Research direction:
Study browser scroll anchoring, Textual scroll views, terminal log viewers, Yazi, k9s, and current TermRock scrolling.

Before implementation, state which existing TermRock types and call sites will be preserved, migrated, split, or deleted. Produce concrete Rust API sketches, implement the component, migrate its usages, add Studio stories and the appropriate visual, semantic, interaction, PTY, fuzz, and performance tests, then perform an independent second-pass design review.
```

## 11. Virtualizer — P0

```text
Using the TermRock Global implementation contract, redesign and implement: Virtualizer.

Component mission:
Build a reusable one- and two-dimensional virtualizer for large collections and grids.

Component-specific requirements:
- Support fixed and variable item extents, overscan, sticky regions, stable item IDs, and visible-range queries.
- Handle viewport resize, insertions, deletions, filtering, and anchor preservation.
- Avoid rendering or measuring offscreen content unnecessarily.
- Integrate with semantic scene registration without allocating millions of nodes.
- Benchmark million-row logical datasets.

Research direction:
Use TermRock VirtualGrid, VisiData, large Textual data tables, and virtualized web collection algorithms as references.

Before implementation, state which existing TermRock types and call sites will be preserved, migrated, split, or deleted. Produce concrete Rust API sketches, implement the component, migrate its usages, add Studio stories and the appropriate visual, semantic, interaction, PTY, fuzz, and performance tests, then perform an independent second-pass design review.
```

## 12. OverlayStack — P0

```text
Using the TermRock Global implementation contract, redesign and implement: OverlayStack.

Component mission:
Create one overlay system for dialogs, menus, completion, popovers, drawers, palettes, and fullscreen viewers.

Component-specific requirements:
- Own z-order, anchors, placement, collision handling, flipping, clamping, focus traps, opener restoration, backdrop, pointer routing, wheel routing, and nested dismissal.
- Support queued modal requests and fullscreen promotion.
- Establish the rule that Escape closes exactly one conceptual layer.
- Handle resize and tiny-terminal fallback deterministically.
- Migrate existing Dialog, CompletionMenu, and Picker-like components.

Research direction:
Study Radix layers, Grok Build overlay state, Textual screens/modals, and robust popover placement algorithms.

Before implementation, state which existing TermRock types and call sites will be preserved, migrated, split, or deleted. Produce concrete Rust API sketches, implement the component, migrate its usages, add Studio stories and the appropriate visual, semantic, interaction, PTY, fuzz, and performance tests, then perform an independent second-pass design review.
```

## 13. DismissableLayer — P0

```text
Using the TermRock Global implementation contract, redesign and implement: DismissableLayer.

Component mission:
Extract reusable dismissal behavior for transient interactive surfaces.

Component-specific requirements:
- Support Escape, outside click, focus leaving, parent overlay closure, explicit dismissal, and non-dismissable critical flows.
- Define capture and bubbling across nested layers.
- Prevent accidental double dismissal from one input event.
- Support pointer press/release sequences and drag cancellation.
- Add exhaustive nested-layer interaction tests.

Research direction:
Use Radix DismissableLayer as a conceptual reference, translated to terminal event semantics.

Before implementation, state which existing TermRock types and call sites will be preserved, migrated, split, or deleted. Produce concrete Rust API sketches, implement the component, migrate its usages, add Studio stories and the appropriate visual, semantic, interaction, PTY, fuzz, and performance tests, then perform an independent second-pass design review.
```

## 14. Responsive and contraction system — P0

```text
Using the TermRock Global implementation contract, redesign and implement: Responsive and contraction system.

Component mission:
Create terminal-native responsive anatomy rather than ad hoc truncation.

Component-specific requirements:
- Let component parts declare essential, important, optional, and decorative priority.
- Support compact spacing, shortened metadata, hidden secondary content, overflow actions, pane collapse, drawer replacement, and line-mode fallback.
- Define width and height breakpoints as recipes rather than global CSS-like assumptions.
- Provide responsive inspectors and fixed-width Studio stories.
- Migrate representative Table, Dialog, Tabs, and Form components.

Research direction:
Study responsive master-detail apps, terminal file managers, modern agent TUIs, and current TermRock narrow-layout contracts.

Before implementation, state which existing TermRock types and call sites will be preserved, migrated, split, or deleted. Produce concrete Rust API sketches, implement the component, migrate its usages, add Studio stories and the appropriate visual, semantic, interaction, PTY, fuzz, and performance tests, then perform an independent second-pass design review.
```

## 15. TerminalCapabilities and CapabilityBoundary — P0

```text
Using the TermRock Global implementation contract, redesign and implement: TerminalCapabilities and CapabilityBoundary.

Component mission:
Model terminal capabilities explicitly and make progressive enhancement a first-class contract.

Component-specific requirements:
- Detect and override color depth, Unicode, keyboard protocol, mouse, bracketed paste, hyperlinks, clipboard, synchronized output, images, text sizing, alternate screen, and inline mode.
- Define Modern, Compatible, Minimal, Inline, and Headless profiles.
- Provide deterministic component fallbacks and a `termrock doctor` experience.
- Honor NO_COLOR and ASCII-only operation.
- Add capability emulation stories and PTY tests.

Research direction:
Study Kitty protocols, terminal multiplexers, NO_COLOR, Crossterm behavior, and progressive enhancement in Yazi and modern TUIs.

Before implementation, state which existing TermRock types and call sites will be preserved, migrated, split, or deleted. Produce concrete Rust API sketches, implement the component, migrate its usages, add Studio stories and the appropriate visual, semantic, interaction, PTY, fuzz, and performance tests, then perform an independent second-pass design review.
```

## 16. FrameClock, Presence, and motion — P1

```text
Using the TermRock Global implementation contract, redesign and implement: FrameClock, Presence, and motion.

Component mission:
Provide deterministic animation and timed-presence primitives without encouraging gratuitous motion.

Component-specific requirements:
- Support spinner cadence, progress pulses, toast lifetimes, delayed tooltip appearance, and reduced-motion mode.
- Never redraw idle screens solely for decorative animation.
- Make time injectable for tests and replay.
- Define enter/exit presence without retaining hidden focusable elements.
- Integrate with Studio recordings and deterministic snapshots.

Research direction:
Study animation scheduling in Textual, Bubble Tea ticks, and terminal redraw constraints.

Before implementation, state which existing TermRock types and call sites will be preserved, migrated, split, or deleted. Produce concrete Rust API sketches, implement the component, migrate its usages, add Studio stories and the appropriate visual, semantic, interaction, PTY, fuzz, and performance tests, then perform an independent second-pass design review.
```

## 17. ComponentContract and registry metadata — P0

```text
Using the TermRock Global implementation contract, redesign and implement: ComponentContract and registry metadata.

Component mission:
Define the machine-readable contract every TermRock registry component must satisfy.

Component-specific requirements:
- Describe files, dependencies, capabilities, anatomy, semantic roles, variants, outcomes, stories, tests, migration, provenance, source hash, and license.
- Enable design linting, CI validation, documentation generation, and Studio browsing.
- Support private registries and source-owned updates.
- Distinguish primitive, component, behavior, block, theme, keymap, and template items.
- Create validation tooling and several real registry entries.

Research direction:
Use shadcn registry concepts, Storybook metadata, package manifests, and TermRock's current component contracts as references.

Before implementation, state which existing TermRock types and call sites will be preserved, migrated, split, or deleted. Produce concrete Rust API sketches, implement the component, migrate its usages, add Studio stories and the appropriate visual, semantic, interaction, PTY, fuzz, and performance tests, then perform an independent second-pass design review.
```


# B. Layout, surface, and application-shell components

These components create visual rhythm, spatial hierarchy, and predictable large-screen composition without turning the terminal into a grid of decorative boxes.

## 18. AppShell — P1

```text
Using the TermRock Global implementation contract, redesign and implement: AppShell.

Component mission:
Create the canonical top-level composition for full-screen and inline TermRock applications.

Component-specific requirements:
- Support header, sidebar, main workspace, inspector rail, footer/status area, overlays, and optional command surface.
- Define focus zones and responsive collapse from multi-pane to single-pane or drawer layouts.
- Handle terminal lifecycle states, disconnected/offline states, and tiny-terminal fallback.
- Expose slots rather than application policy.
- Ship several recipes: workbench, dashboard, master-detail, and minimal.

Research direction:
Study Zellij layouts, Posting, OpenCode, Grok Build, database clients, and IDE workbenches.

Before implementation, state which existing TermRock types and call sites will be preserved, migrated, split, or deleted. Produce concrete Rust API sketches, implement the component, migrate its usages, add Studio stories and the appropriate visual, semantic, interaction, PTY, fuzz, and performance tests, then perform an independent second-pass design review.
```

## 19. Surface — P1

```text
Using the TermRock Global implementation contract, redesign and implement: Surface.

Component mission:
Create the lowest-level visual ownership primitive for backgrounds, padding, borders, clipping, and stateful chrome.

Component-specific requirements:
- Support canvas, inset, raised, overlay, interactive, focused, selected, warning, and destructive recipes.
- Keep background use compatible with terminal-default colors and no-color mode.
- Allow named parts and composition without nested box soup.
- Define clipping and hit-region semantics.
- Migrate existing ad hoc Blocks and Panels where appropriate.

Research direction:
Study shadcn surfaces/cards conceptually, Glow's restraint, and btop's controlled use of panels.

Before implementation, state which existing TermRock types and call sites will be preserved, migrated, split, or deleted. Produce concrete Rust API sketches, implement the component, migrate its usages, add Studio stories and the appropriate visual, semantic, interaction, PTY, fuzz, and performance tests, then perform an independent second-pass design review.
```

## 20. Panel and Card — P1

```text
Using the TermRock Global implementation contract, redesign and implement: Panel and Card.

Component mission:
Redesign the existing panel concept into a composable container with meaningful anatomy.

Component-specific requirements:
- Support title, subtitle, leading status, badges, header actions, body, footer, loading, empty, error, and collapsible states.
- Provide bordered, divider-only, quiet, interactive, and selected recipes.
- Define responsive removal of low-priority metadata and actions.
- Ensure focus belongs to real interactive descendants unless the whole panel is actionable.
- Add agent tool-card and dashboard examples.

Research direction:
Compare shadcn Card anatomy, Textual containers, btop panels, and Grok Build task/tool surfaces.

Before implementation, state which existing TermRock types and call sites will be preserved, migrated, split, or deleted. Produce concrete Rust API sketches, implement the component, migrate its usages, add Studio stories and the appropriate visual, semantic, interaction, PTY, fuzz, and performance tests, then perform an independent second-pass design review.
```

## 21. Stack and Inline — P1

```text
Using the TermRock Global implementation contract, redesign and implement: Stack and Inline.

Component mission:
Create ergonomic vertical and horizontal layout primitives with terminal-cell spacing tokens.

Component-specific requirements:
- Support alignment, distribution, wrapping, fill, intrinsic sizing, min/max constraints, and responsive direction changes.
- Avoid allocations and complex retained layout state.
- Integrate semantic grouping and hit geometry.
- Define clear behavior when children exceed available cells.
- Use them to simplify several existing component implementations.

Research direction:
Study flex layouts in Ink, Textual, FTXUI, and CSS concepts while respecting Ratatui's explicit Rect model.

Before implementation, state which existing TermRock types and call sites will be preserved, migrated, split, or deleted. Produce concrete Rust API sketches, implement the component, migrate its usages, add Studio stories and the appropriate visual, semantic, interaction, PTY, fuzz, and performance tests, then perform an independent second-pass design review.
```

## 22. Grid — P1

```text
Using the TermRock Global implementation contract, redesign and implement: Grid.

Component mission:
Create a predictable terminal grid for dashboards, forms, settings, and card collections.

Component-specific requirements:
- Support fixed, fractional, intrinsic, and minmax-like tracks; row/column gaps; spans; and responsive templates.
- Provide deterministic overflow and contraction rules.
- Keep layout computation transparent and debuggable in Studio.
- Support keyboard spatial navigation where requested, without making every grid interactive.
- Benchmark large but realistic layouts.

Research direction:
Study Textual Grid, CSS Grid concepts, dashboard TUIs, and Ratatui constraint layouts.

Before implementation, state which existing TermRock types and call sites will be preserved, migrated, split, or deleted. Produce concrete Rust API sketches, implement the component, migrate its usages, add Studio stories and the appropriate visual, semantic, interaction, PTY, fuzz, and performance tests, then perform an independent second-pass design review.
```

## 23. Center — P2

```text
Using the TermRock Global implementation contract, redesign and implement: Center.

Component mission:
Create a small but rigorous primitive for centering and constrained content.

Component-specific requirements:
- Support horizontal, vertical, both-axis, max-width, max-height, and safe minimums.
- Avoid underflow on tiny terminal sizes.
- Register only the child semantics rather than creating a fake interactive node.
- Use it in empty states, dialogs, onboarding, and failure screens.
- Add property tests across all terminal dimensions.

Research direction:
Reference common Ratatui centering helpers but turn them into a tested design-system primitive.

Before implementation, state which existing TermRock types and call sites will be preserved, migrated, split, or deleted. Produce concrete Rust API sketches, implement the component, migrate its usages, add Studio stories and the appropriate visual, semantic, interaction, PTY, fuzz, and performance tests, then perform an independent second-pass design review.
```

## 24. Section — P1

```text
Using the TermRock Global implementation contract, redesign and implement: Section.

Component mission:
Create an editorial grouping primitive that communicates hierarchy without requiring a full border.

Component-specific requirements:
- Support heading, description, actions, divider, indentation, status, and nested sections.
- Define spacing and text hierarchy recipes.
- Contract actions into overflow on narrow widths.
- Work in forms, settings, documentation, object inspectors, and dashboards.
- Provide quiet and emphasized variants.

Research direction:
Study Glow, settings screens, shadcn section patterns, and good CLI help output.

Before implementation, state which existing TermRock types and call sites will be preserved, migrated, split, or deleted. Produce concrete Rust API sketches, implement the component, migrate its usages, add Studio stories and the appropriate visual, semantic, interaction, PTY, fuzz, and performance tests, then perform an independent second-pass design review.
```

## 25. Separator — P1

```text
Using the TermRock Global implementation contract, redesign and implement: Separator.

Component mission:
Create semantic horizontal and vertical separators with optional labels.

Component-specific requirements:
- Support quiet divider, strong divider, section break, labeled divider, and focus-zone boundary.
- Guarantee ASCII and no-color output.
- Use spacing recipes rather than embedded arbitrary gaps.
- Do not register interactive semantics unless labeled navigation behavior exists.
- Test all one-cell and tiny-size edge cases.

Research direction:
Study editorial terminal layouts and shadcn Separator behavior.

Before implementation, state which existing TermRock types and call sites will be preserved, migrated, split, or deleted. Produce concrete Rust API sketches, implement the component, migrate its usages, add Studio stories and the appropriate visual, semantic, interaction, PTY, fuzz, and performance tests, then perform an independent second-pass design review.
```

## 26. Toolbar — P1

```text
Using the TermRock Global implementation contract, redesign and implement: Toolbar.

Component mission:
Create a roving-focus action strip for contextual commands.

Component-specific requirements:
- Support buttons, toggle groups, separators, overflow menu, labels, icons, and key hints.
- Provide horizontal and compact vertical variants.
- Move low-priority actions into overflow responsively.
- Integrate generated command metadata and semantic intents.
- Distinguish application toolbar focus from content selection.

Research direction:
Study desktop toolbars, Radix Toolbar, Posting, database clients, and terminal editors.

Before implementation, state which existing TermRock types and call sites will be preserved, migrated, split, or deleted. Produce concrete Rust API sketches, implement the component, migrate its usages, add Studio stories and the appropriate visual, semantic, interaction, PTY, fuzz, and performance tests, then perform an independent second-pass design review.
```

## 27. StatusBar — P1

```text
Using the TermRock Global implementation contract, redesign and implement: StatusBar.

Component mission:
Create a low-noise status surface for mode, connection, selection, context, and contextual shortcuts.

Component-specific requirements:
- Support left, center, and right regions with responsive priorities.
- Show semantic status text and symbols rather than unexplained color.
- Integrate generated key hints and active focus zone.
- Handle transient messages without displacing critical persistent state.
- Provide minimal, compact, and rich recipes.

Research direction:
Study Zellij's mode bar, Vim/Helix status lines, OpenCode, Grok Build, and btop.

Before implementation, state which existing TermRock types and call sites will be preserved, migrated, split, or deleted. Produce concrete Rust API sketches, implement the component, migrate its usages, add Studio stories and the appropriate visual, semantic, interaction, PTY, fuzz, and performance tests, then perform an independent second-pass design review.
```

## 28. SplitPane — P1

```text
Using the TermRock Global implementation contract, redesign and implement: SplitPane.

Component mission:
Redesign split panes around accessible keyboard resizing, mouse dragging, focus zones, and responsive collapse.

Component-specific requirements:
- Support horizontal and vertical splits, min/max sizes, preferred ratios, collapse, restore, and persisted layouts.
- Provide visible but restrained resize affordances.
- Handle tiny terminals without negative or zero-width child errors.
- Expose typed resize and collapse outcomes.
- Add nested-split and rapid-resize tests.

Research direction:
Study Zellij panes, IDE split views, Yazi, Posting, and current TermRock split behavior.

Before implementation, state which existing TermRock types and call sites will be preserved, migrated, split, or deleted. Produce concrete Rust API sketches, implement the component, migrate its usages, add Studio stories and the appropriate visual, semantic, interaction, PTY, fuzz, and performance tests, then perform an independent second-pass design review.
```

## 29. ResizablePanelGroup — P2

```text
Using the TermRock Global implementation contract, redesign and implement: ResizablePanelGroup.

Component mission:
Create a higher-order multi-panel layout built on SplitPane.

Component-specific requirements:
- Support multiple handles, constrained redistribution, collapse thresholds, keyboard resizing, and saved presets.
- Preserve focused content and scroll state during layout changes.
- Offer responsive recipes that replace side panels with drawers.
- Avoid coupling panel content to the layout engine.
- Provide workbench and dashboard stories.

Research direction:
Study desktop workbench layouts and Zellij's pane management.

Before implementation, state which existing TermRock types and call sites will be preserved, migrated, split, or deleted. Produce concrete Rust API sketches, implement the component, migrate its usages, add Studio stories and the appropriate visual, semantic, interaction, PTY, fuzz, and performance tests, then perform an independent second-pass design review.
```

## 30. Collapsible — P1

```text
Using the TermRock Global implementation contract, redesign and implement: Collapsible.

Component mission:
Create an accessible disclosure primitive for optional detail.

Component-specific requirements:
- Support controlled/uncontrolled open state, trigger, content, disabled state, and nested disclosures.
- Use semantic expand/collapse intents and stable focus behavior.
- Preserve child state while closed only when policy requests it.
- Provide compact inline and section variants.
- Ensure glyph and no-color fallbacks communicate state.

Research direction:
Study Radix Collapsible, tree disclosures, and tool-detail expansion in agent TUIs.

Before implementation, state which existing TermRock types and call sites will be preserved, migrated, split, or deleted. Produce concrete Rust API sketches, implement the component, migrate its usages, add Studio stories and the appropriate visual, semantic, interaction, PTY, fuzz, and performance tests, then perform an independent second-pass design review.
```

## 31. Accordion — P1

```text
Using the TermRock Global implementation contract, redesign and implement: Accordion.

Component mission:
Build single- or multi-open disclosure groups using Collapsible and roving focus.

Component-specific requirements:
- Support Home/End, disabled items, typeahead, stable IDs, and controlled state.
- Differentiate navigation focus from expanded state.
- Handle large content with nested scroll areas.
- Provide section, settings, logs, and FAQ recipes.
- Test dynamic item insertion/removal and narrow layouts.

Research direction:
Study Radix Accordion, Textual collapsibles, and settings/help interfaces.

Before implementation, state which existing TermRock types and call sites will be preserved, migrated, split, or deleted. Produce concrete Rust API sketches, implement the component, migrate its usages, add Studio stories and the appropriate visual, semantic, interaction, PTY, fuzz, and performance tests, then perform an independent second-pass design review.
```


# C. Content, typography, and identity components

These components create editorial hierarchy, readable technical content, and compact identity signals. Terminal typography is achieved through spacing, weight, color, indentation, and glyph discipline—not CSS imitation.

## 32. Text — P1

```text
Using the TermRock Global implementation contract, redesign and implement: Text.

Component mission:
Create the canonical styled text primitive for semantic content.

Component-specific requirements:
- Support semantic roles, spans, emphasis, dimming, wrapping, truncation, alignment, selectable text policy, and copy-safe rendering.
- Handle graphemes, combining marks, CJK, emoji, tabs, control characters, and ambiguous widths.
- Preserve terminal-default background where possible.
- Expose syntax-independent inline annotations and highlights.
- Replace ad hoc styled Span construction in representative components.

Research direction:
Study Rich Text, Textual Static, Glow typography, and Ratatui text primitives.

Before implementation, state which existing TermRock types and call sites will be preserved, migrated, split, or deleted. Produce concrete Rust API sketches, implement the component, migrate its usages, add Studio stories and the appropriate visual, semantic, interaction, PTY, fuzz, and performance tests, then perform an independent second-pass design review.
```

## 33. Heading and Paragraph — P1

```text
Using the TermRock Global implementation contract, redesign and implement: Heading and Paragraph.

Component mission:
Create editorial heading and prose components for documentation, dialogs, plans, help, and empty states.

Component-specific requirements:
- Define heading levels through spacing, weight, dividers, glyphs, and optional modern-terminal enhancements.
- Support paragraph wrapping, indentation, hanging prefixes, quotes, lists, and selectable text.
- Provide compact and reading-mode recipes.
- Guarantee graceful ASCII and no-color hierarchy.
- Integrate with Markdown rendering without duplicating layout logic.

Research direction:
Use Glow, Rich, man pages, and high-quality CLI help as primary references.

Before implementation, state which existing TermRock types and call sites will be preserved, migrated, split, or deleted. Produce concrete Rust API sketches, implement the component, migrate its usages, add Studio stories and the appropriate visual, semantic, interaction, PTY, fuzz, and performance tests, then perform an independent second-pass design review.
```

## 34. Label and Description — P1

```text
Using the TermRock Global implementation contract, redesign and implement: Label and Description.

Component mission:
Create consistent labeling primitives for fields, settings, controls, and metadata.

Component-specific requirements:
- Associate labels and descriptions semantically with target component IDs.
- Support required, optional, disabled, invalid, warning, and help states.
- Define inline, stacked, and compact layout recipes.
- Ensure descriptions contract before primary labels.
- Generate useful semantic-scene descriptions for inspection and help.

Research direction:
Study accessible form labeling in Radix/shadcn and terminal settings interfaces.

Before implementation, state which existing TermRock types and call sites will be preserved, migrated, split, or deleted. Produce concrete Rust API sketches, implement the component, migrate its usages, add Studio stories and the appropriate visual, semantic, interaction, PTY, fuzz, and performance tests, then perform an independent second-pass design review.
```

## 35. Icon and Glyph — P1

```text
Using the TermRock Global implementation contract, redesign and implement: Icon and Glyph.

Component mission:
Create a semantic glyph system rather than scattered Unicode literals.

Component-specific requirements:
- Map semantic names to Unicode, ASCII, and optional enhanced/Nerd Font representations.
- Define width guarantees and alignment behavior.
- Prevent glyphs from being the only representation of critical meaning.
- Support directional, status, file-type, action, and disclosure glyph groups.
- Add a Studio glyph browser and capability fallback tests.

Research direction:
Study Lucide's semantic consistency conceptually, terminal icon packs, Yazi, and Zellij.

Before implementation, state which existing TermRock types and call sites will be preserved, migrated, split, or deleted. Produce concrete Rust API sketches, implement the component, migrate its usages, add Studio stories and the appropriate visual, semantic, interaction, PTY, fuzz, and performance tests, then perform an independent second-pass design review.
```

## 36. Badge — P1

```text
Using the TermRock Global implementation contract, redesign and implement: Badge.

Component mission:
Create a compact status or category indicator with strong semantic discipline.

Component-specific requirements:
- Support neutral, informational, success, warning, destructive, outline, and count variants.
- Define selected, focused, interactive, and disabled behavior only when the badge is actionable.
- Handle narrow layouts, clipping, large counts, and no-color symbols.
- Avoid excessive background fills that dominate dense views.
- Provide table, task, and settings examples.

Research direction:
Study shadcn Badge, issue labels, btop indicators, and agent task statuses.

Before implementation, state which existing TermRock types and call sites will be preserved, migrated, split, or deleted. Produce concrete Rust API sketches, implement the component, migrate its usages, add Studio stories and the appropriate visual, semantic, interaction, PTY, fuzz, and performance tests, then perform an independent second-pass design review.
```

## 37. Tag and Chip — P1

```text
Using the TermRock Global implementation contract, redesign and implement: Tag and Chip.

Component mission:
Create removable/selectable compact tokens for filters, attachments, entities, and structured input.

Component-specific requirements:
- Support static tag, interactive chip, selected chip, removable chip, error chip, and loading chip.
- Define internal focus between label and remove action without awkward Tab explosion.
- Support horizontal scrolling, wrapping, and overflow summaries.
- Provide ASCII fallbacks and explicit removal text for screen/semantic inspection.
- Use shared behavior in TokenField, attachments, filters, and paste chips.

Research direction:
Study shadcn-style badges, token inputs, Grok Build paste/file chips, and desktop filter controls.

Before implementation, state which existing TermRock types and call sites will be preserved, migrated, split, or deleted. Produce concrete Rust API sketches, implement the component, migrate its usages, add Studio stories and the appropriate visual, semantic, interaction, PTY, fuzz, and performance tests, then perform an independent second-pass design review.
```

## 38. Kbd and ShortcutHint — P1

```text
Using the TermRock Global implementation contract, redesign and implement: Kbd and ShortcutHint.

Component mission:
Create a canonical keyboard chord and shortcut display component.

Component-specific requirements:
- Render platform-aware modifier names, sequences, alternatives, and semantic commands.
- Derive display from the active keymap rather than hardcoded labels.
- Support compact footer form, inline documentation form, and keycap-like form.
- Handle ASCII/no-color and narrow contraction.
- Use it throughout menus, dialogs, help, and toolbars.

Research direction:
Study shadcn Kbd, editor shortcut UIs, Textual bindings display, and Zellij help.

Before implementation, state which existing TermRock types and call sites will be preserved, migrated, split, or deleted. Produce concrete Rust API sketches, implement the component, migrate its usages, add Studio stories and the appropriate visual, semantic, interaction, PTY, fuzz, and performance tests, then perform an independent second-pass design review.
```

## 39. Link and ActionLink — P2

```text
Using the TermRock Global implementation contract, redesign and implement: Link and ActionLink.

Component mission:
Create terminal-safe links and lightweight inline actions.

Component-specific requirements:
- Support OSC-8 hyperlinks when available, visible URL fallback, copy action, and application-routed links.
- Differentiate navigation links from button-like actions.
- Provide focus, hover, visited/session state if useful, and disabled behavior.
- Never hide destination or risk for external links.
- Add no-hyperlink and no-color stories.

Research direction:
Study Rich hyperlinks, terminal hyperlink protocols, and CLI documentation conventions.

Before implementation, state which existing TermRock types and call sites will be preserved, migrated, split, or deleted. Produce concrete Rust API sketches, implement the component, migrate its usages, add Studio stories and the appropriate visual, semantic, interaction, PTY, fuzz, and performance tests, then perform an independent second-pass design review.
```

## 40. CodeBlock — P1

```text
Using the TermRock Global implementation contract, redesign and implement: CodeBlock.

Component mission:
Create a production-grade code and command rendering component.

Component-specific requirements:
- Support syntax highlighting, line numbers, highlighted ranges, selection, horizontal scrolling, wrapping policy, copy, and source metadata.
- Render control characters and tabs intentionally.
- Support streaming/unfinished code fences and large-file virtualization.
- Provide ANSI and no-color syntax fallbacks.
- Compose with Diagnostic, DiffReview, PlanReview, and TerminalOutput.

Research direction:
Study Glow, bat, delta, Rich Syntax, and code views in lazygit and agent TUIs.

Before implementation, state which existing TermRock types and call sites will be preserved, migrated, split, or deleted. Produce concrete Rust API sketches, implement the component, migrate its usages, add Studio stories and the appropriate visual, semantic, interaction, PTY, fuzz, and performance tests, then perform an independent second-pass design review.
```

## 41. Markdown — P1

```text
Using the TermRock Global implementation contract, redesign and implement: Markdown.

Component mission:
Create an editorial, streaming-capable Markdown renderer for terminal applications.

Component-specific requirements:
- Support headings, paragraphs, lists, task lists, quotes, tables, links, code, syntax fences, thematic breaks, and inline emphasis.
- Handle incremental input with unfinished blocks without layout thrashing.
- Provide selectable text, source anchors, link activation, and responsive tables.
- Use whitespace and indentation before borders.
- Benchmark long AI responses and documentation files.

Research direction:
Use Glow as the visual benchmark, plus Rich Markdown and streaming agent output.

Before implementation, state which existing TermRock types and call sites will be preserved, migrated, split, or deleted. Produce concrete Rust API sketches, implement the component, migrate its usages, add Studio stories and the appropriate visual, semantic, interaction, PTY, fuzz, and performance tests, then perform an independent second-pass design review.
```

## 42. AnsiText — P1

```text
Using the TermRock Global implementation contract, redesign and implement: AnsiText.

Component mission:
Safely parse and render ANSI-styled terminal output inside TermRock surfaces.

Component-specific requirements:
- Support SGR colors/styles, resets, carriage returns, tabs, backspaces where appropriate, hyperlinks, and malformed sequences.
- Prevent escape-sequence injection from affecting the host terminal.
- Provide stripping and no-color modes.
- Support incremental streaming and bounded history.
- Integrate with TerminalOutput and LogStream.

Research direction:
Study terminal emulators, ansi-to-tui parsers, Rich, and command-output components in agent tools.

Before implementation, state which existing TermRock types and call sites will be preserved, migrated, split, or deleted. Produce concrete Rust API sketches, implement the component, migrate its usages, add Studio stories and the appropriate visual, semantic, interaction, PTY, fuzz, and performance tests, then perform an independent second-pass design review.
```

## 43. KeyValueList — P1

```text
Using the TermRock Global implementation contract, redesign and implement: KeyValueList.

Component mission:
Create a compact metadata presentation component for settings, object summaries, dialogs, and details panes.

Component-specific requirements:
- Support aligned keys, wrapped values, nested groups, copyable values, status values, links, and secret redaction.
- Contract from two-column to stacked anatomy on narrow terminals.
- Preserve primary values before secondary annotations.
- Support interactive rows only when actions exist.
- Add dense and reading-mode recipes.

Research direction:
Study system information TUIs, detail panels, and shadcn DescriptionList-style patterns.

Before implementation, state which existing TermRock types and call sites will be preserved, migrated, split, or deleted. Produce concrete Rust API sketches, implement the component, migrate its usages, add Studio stories and the appropriate visual, semantic, interaction, PTY, fuzz, and performance tests, then perform an independent second-pass design review.
```

## 44. AvatarGlyph and Identity — P3

```text
Using the TermRock Global implementation contract, redesign and implement: AvatarGlyph and Identity.

Component mission:
Create a terminal-native identity marker for users, agents, services, and collaborators.

Component-specific requirements:
- Support initials, semantic glyphs, status, role badge, and deterministic fallback patterns without relying on raster images.
- Guarantee one- and two-cell widths across capability profiles.
- Provide compact, normal, and presence variants.
- Keep identity understandable in no-color mode.
- Use in message threads, subagents, and collaboration surfaces.

Research direction:
Study chat identity systems and agent TUIs, adapted to monospaced terminals.

Before implementation, state which existing TermRock types and call sites will be preserved, migrated, split, or deleted. Produce concrete Rust API sketches, implement the component, migrate its usages, add Studio stories and the appropriate visual, semantic, interaction, PTY, fuzz, and performance tests, then perform an independent second-pass design review.
```

## 45. HighlightedText and MatchRanges — P2

```text
Using the TermRock Global implementation contract, redesign and implement: HighlightedText and MatchRanges.

Component mission:
Create a reusable text-match renderer for search, fuzzy matching, completion, and command palettes.

Component-specific requirements:
- Support multiple match ranges, grapheme-safe indices, overlapping annotations, focused/selected states, and no-color emphasis.
- Avoid recomputing expensive fuzzy metadata during render.
- Preserve source text for copy and semantic labels.
- Provide truncation that keeps important matches visible.
- Use across QuickOpen, SearchResults, Combobox, and CommandPalette.

Research direction:
Study fuzzy finders such as fzf, television, and command palettes.

Before implementation, state which existing TermRock types and call sites will be preserved, migrated, split, or deleted. Produce concrete Rust API sketches, implement the component, migrate its usages, add Studio stories and the appropriate visual, semantic, interaction, PTY, fuzz, and performance tests, then perform an independent second-pass design review.
```


# D. Actions, fields, and form components

These prompts build the daily interaction vocabulary of TermRock. Each control must be keyboard-first, mouse-complete, semantically inspectable, and visually coherent across dense and comfortable modes.

## 46. Button — P1

```text
Using the TermRock Global implementation contract, redesign and implement: Button.

Component mission:
Create the canonical primary action component.

Component-specific requirements:
- Support primary, secondary, quiet, outline, destructive, link-like, success, and command variants.
- Support compact/normal sizes, leading/trailing glyphs, loading, disabled, pending confirmation, and full-width behavior.
- Define activation, press/release, repeat policy, focus, pointer, and double-trigger prevention.
- Do not rely on bracket decoration alone for affordance.
- Add realistic dialog, toolbar, form, and inline-action stories.

Research direction:
Study shadcn Button anatomy, Radix behavior, Gum prompts, Textual buttons, and polished agent dialogs.

Before implementation, state which existing TermRock types and call sites will be preserved, migrated, split, or deleted. Produce concrete Rust API sketches, implement the component, migrate its usages, add Studio stories and the appropriate visual, semantic, interaction, PTY, fuzz, and performance tests, then perform an independent second-pass design review.
```

## 47. IconButton — P1

```text
Using the TermRock Global implementation contract, redesign and implement: IconButton.

Component mission:
Create a compact glyph action with mandatory accessible labeling.

Component-specific requirements:
- Support tooltip/help integration, toggle state, badges, loading, disabled, destructive, and compact toolbar recipes.
- Require semantic label even when only a glyph is rendered.
- Provide safe target sizing and pointer hit slop without distorting visual layout.
- Use ASCII glyph fallback and optional text fallback at low capability.
- Integrate with Toolbar, Panel actions, and data-row actions.

Research direction:
Study desktop toolbar buttons and shadcn icon buttons, adapted to terminal cell constraints.

Before implementation, state which existing TermRock types and call sites will be preserved, migrated, split, or deleted. Produce concrete Rust API sketches, implement the component, migrate its usages, add Studio stories and the appropriate visual, semantic, interaction, PTY, fuzz, and performance tests, then perform an independent second-pass design review.
```

## 48. ButtonGroup — P1

```text
Using the TermRock Global implementation contract, redesign and implement: ButtonGroup.

Component mission:
Create grouped actions with shared borders, priority, overflow, and focus behavior.

Component-specific requirements:
- Support connected and separated visual recipes, primary/secondary ordering, overflow menu, and destructive separation.
- Use roving focus where appropriate without hiding individual command semantics.
- Contract secondary actions into overflow at narrow widths.
- Handle loading/disabled children and default action submission.
- Use in dialogs, review flows, and toolbars.

Research direction:
Study shadcn button groups, desktop dialog action bars, and terminal prompt action rows.

Before implementation, state which existing TermRock types and call sites will be preserved, migrated, split, or deleted. Produce concrete Rust API sketches, implement the component, migrate its usages, add Studio stories and the appropriate visual, semantic, interaction, PTY, fuzz, and performance tests, then perform an independent second-pass design review.
```

## 49. Toggle and ToggleGroup — P1

```text
Using the TermRock Global implementation contract, redesign and implement: Toggle and ToggleGroup.

Component mission:
Create single and grouped pressable state controls distinct from checkboxes and tabs.

Component-specific requirements:
- Support pressed, unpressed, indeterminate if justified, disabled, icon-only, text, single-select, and multi-select groups.
- Use roving focus and typed state changes.
- Make pressed state understandable without color.
- Support responsive overflow and compact toolbar recipes.
- Document when ToggleGroup should not be used.

Research direction:
Study Radix Toggle/ToggleGroup, editor toolbars, and mode controls.

Before implementation, state which existing TermRock types and call sites will be preserved, migrated, split, or deleted. Produce concrete Rust API sketches, implement the component, migrate its usages, add Studio stories and the appropriate visual, semantic, interaction, PTY, fuzz, and performance tests, then perform an independent second-pass design review.
```

## 50. Checkbox — P1

```text
Using the TermRock Global implementation contract, redesign and implement: Checkbox.

Component mission:
Create a robust binary or tri-state selection control.

Component-specific requirements:
- Support checked, unchecked, indeterminate, disabled, invalid, read-only, and mixed-group states.
- Associate label and description semantics.
- Define Space/Activate behavior, pointer hit area, and form integration.
- Provide ASCII symbols and no-color state distinction.
- Test dynamic controlled state and list/table composition.

Research direction:
Study Radix Checkbox, Textual Checkbox, Huh forms, and terminal setup wizards.

Before implementation, state which existing TermRock types and call sites will be preserved, migrated, split, or deleted. Produce concrete Rust API sketches, implement the component, migrate its usages, add Studio stories and the appropriate visual, semantic, interaction, PTY, fuzz, and performance tests, then perform an independent second-pass design review.
```

## 51. RadioGroup — P1

```text
Using the TermRock Global implementation contract, redesign and implement: RadioGroup.

Component mission:
Create a single-choice group with predictable roving navigation.

Component-specific requirements:
- Support vertical/horizontal orientation, disabled options, descriptions, badges, typeahead, and controlled state.
- Decide and document whether focus movement commits selection or requires activation, with configurable policy if necessary.
- Handle long labels and narrow stacked anatomy.
- Guarantee no-color and ASCII clarity.
- Use in settings, permissions, and question flows.

Research direction:
Study Radix RadioGroup, native desktop radio behavior, Huh, and Grok Build choices.

Before implementation, state which existing TermRock types and call sites will be preserved, migrated, split, or deleted. Produce concrete Rust API sketches, implement the component, migrate its usages, add Studio stories and the appropriate visual, semantic, interaction, PTY, fuzz, and performance tests, then perform an independent second-pass design review.
```

## 52. Switch — P1

```text
Using the TermRock Global implementation contract, redesign and implement: Switch.

Component mission:
Create an immediate on/off setting control distinct from a checkbox.

Component-specific requirements:
- Support checked, unchecked, disabled, read-only, loading, and error states.
- Pair the visual switch with explicit On/Off text where ambiguity exists.
- Define keyboard and pointer behavior and prevent accidental toggles in scrollable rows.
- Provide compact settings-row anatomy.
- Document appropriate versus inappropriate use.

Research direction:
Study shadcn/Radix Switch and terminal settings UIs.

Before implementation, state which existing TermRock types and call sites will be preserved, migrated, split, or deleted. Produce concrete Rust API sketches, implement the component, migrate its usages, add Studio stories and the appropriate visual, semantic, interaction, PTY, fuzz, and performance tests, then perform an independent second-pass design review.
```

## 53. SegmentedControl — P1

```text
Using the TermRock Global implementation contract, redesign and implement: SegmentedControl.

Component mission:
Create a compact mutually exclusive view or mode selector.

Component-specific requirements:
- Support text, icons, badges, disabled segments, overflow, and responsive collapse into Select.
- Use roving focus and typed selected-value changes.
- Distinguish it from Tabs by content relationship and from RadioGroup by visual density.
- Make active state clear without a full neon fill.
- Use for view modes, density, model modes, and filters.

Research direction:
Study desktop segmented controls, shadcn patterns, and mode selectors in developer tools.

Before implementation, state which existing TermRock types and call sites will be preserved, migrated, split, or deleted. Produce concrete Rust API sketches, implement the component, migrate its usages, add Studio stories and the appropriate visual, semantic, interaction, PTY, fuzz, and performance tests, then perform an independent second-pass design review.
```

## 54. Slider and RangeSlider — P2

```text
Using the TermRock Global implementation contract, redesign and implement: Slider and RangeSlider.

Component mission:
Create terminal-native numeric sliders for bounded settings and filters.

Component-specific requirements:
- Support single and range values, step size, marks, labels, direct numeric entry, disabled/read-only, and vertical orientation if justified.
- Provide keyboard increments, page increments, Home/End, and pointer dragging.
- Make handles visible in ASCII and no-color modes.
- Handle tiny widths by falling back to numeric input.
- Avoid using sliders where exact precision is required without a paired value field.

Research direction:
Study Radix Slider, TUI volume controls, btop settings, and Textual sliders.

Before implementation, state which existing TermRock types and call sites will be preserved, migrated, split, or deleted. Produce concrete Rust API sketches, implement the component, migrate its usages, add Studio stories and the appropriate visual, semantic, interaction, PTY, fuzz, and performance tests, then perform an independent second-pass design review.
```

## 55. Field, Fieldset, and Form — P0

```text
Using the TermRock Global implementation contract, redesign and implement: Field, Fieldset, and Form.

Component mission:
Build the compositional form architecture shared by every input control.

Component-specific requirements:
- Support labels, descriptions, required/optional state, validation, warnings, async validation, help, status, and grouped fields.
- Define controlled value ownership and typed field outcomes without hidden application state.
- Support form-level submission, reset, dirty state, touched state, error summary, and first-invalid focus.
- Provide stacked, inline, compact, and responsive layouts.
- Migrate the current Form implementation deliberately.

Research direction:
Study shadcn form composition, React Hook Form concepts, Huh, Textual forms, and desktop settings panels.

Before implementation, state which existing TermRock types and call sites will be preserved, migrated, split, or deleted. Produce concrete Rust API sketches, implement the component, migrate its usages, add Studio stories and the appropriate visual, semantic, interaction, PTY, fuzz, and performance tests, then perform an independent second-pass design review.
```

## 56. TextInput — P1

```text
Using the TermRock Global implementation contract, redesign and implement: TextInput.

Component mission:
Create a production-grade single-line text editor.

Component-specific requirements:
- Support grapheme-safe editing, selection, clipboard integration, undo/redo, word movement, Home/End, horizontal scrolling, placeholder, prefix/suffix, clear action, and validation.
- Support Emacs/Vim-compatible intents through keymaps without hardcoded keys.
- Handle bracketed paste, control characters, IME limitations, and mouse cursor placement.
- Provide disabled, read-only, loading, invalid, and secret-adjacent recipes.
- Add Unicode fuzzing and long-input benchmarks.

Research direction:
Study terminal line editors, prompt-toolkit, Reedline, Textual Input, and current TermRock text editing.

Before implementation, state which existing TermRock types and call sites will be preserved, migrated, split, or deleted. Produce concrete Rust API sketches, implement the component, migrate its usages, add Studio stories and the appropriate visual, semantic, interaction, PTY, fuzz, and performance tests, then perform an independent second-pass design review.
```

## 57. TextArea — P1

```text
Using the TermRock Global implementation contract, redesign and implement: TextArea.

Component mission:
Create a multi-line editor suitable for forms, notes, prompts, and comments.

Component-specific requirements:
- Support grapheme-safe editing, selection, undo/redo, line/word movement, indentation, configurable wrapping, horizontal scroll, line numbers, and external editor.
- Integrate ScrollArea, semantic intents, completion overlays, and responsive fullscreen promotion.
- Preserve cursor and scroll through resize and reflow.
- Support read-only and review-comment variants.
- Benchmark large but realistic documents.

Research direction:
Study tui-textarea, prompt-toolkit, terminal editors, and agent prompt composers.

Before implementation, state which existing TermRock types and call sites will be preserved, migrated, split, or deleted. Produce concrete Rust API sketches, implement the component, migrate its usages, add Studio stories and the appropriate visual, semantic, interaction, PTY, fuzz, and performance tests, then perform an independent second-pass design review.
```

## 58. PasswordInput — P1

```text
Using the TermRock Global implementation contract, redesign and implement: PasswordInput.

Component mission:
Create a secure secret-entry component derived from TextInput without leaking value through rendering or diagnostics.

Component-specific requirements:
- Support masking, reveal-on-hold or explicit reveal policy, paste controls, strength/status hooks, and confirmation pairing.
- Redact values from semantic scene, logs, snapshots, debug output, and replay recordings.
- Define clipboard and copy policy explicitly.
- Support disabled, read-only, invalid, and pending states.
- Add security-focused tests for accidental exposure.

Research direction:
Study secure CLI prompts, password managers, and desktop secret fields.

Before implementation, state which existing TermRock types and call sites will be preserved, migrated, split, or deleted. Produce concrete Rust API sketches, implement the component, migrate its usages, add Studio stories and the appropriate visual, semantic, interaction, PTY, fuzz, and performance tests, then perform an independent second-pass design review.
```

## 59. NumberInput — P1

```text
Using the TermRock Global implementation contract, redesign and implement: NumberInput.

Component mission:
Create a numeric field with parsing, validation, and optional stepper actions.

Component-specific requirements:
- Support integers, decimals, units, min/max, step, locale-independent storage, empty state, and invalid intermediate input.
- Keep editing text separate from committed numeric value.
- Provide increment/decrement intents and optional mouse controls.
- Handle overflow and precision safely.
- Compose with Slider and settings forms.

Research direction:
Study shadcn numeric inputs conceptually, Textual numeric fields, and robust desktop form behavior.

Before implementation, state which existing TermRock types and call sites will be preserved, migrated, split, or deleted. Produce concrete Rust API sketches, implement the component, migrate its usages, add Studio stories and the appropriate visual, semantic, interaction, PTY, fuzz, and performance tests, then perform an independent second-pass design review.
```

## 60. SearchInput — P1

```text
Using the TermRock Global implementation contract, redesign and implement: SearchInput.

Component mission:
Create a specialized search field with query, status, clear, history, and filter affordances.

Component-specific requirements:
- Support debounced application messages without embedding async work.
- Show result count, searching, no-results, error, and active-filter state.
- Integrate history, completion, command syntax, and Escape semantics.
- Contract metadata before the query text.
- Use in tables, logs, quick open, command palette, and object inspectors.

Research direction:
Study fzf, television, browser search, VisiData, and editor search bars.

Before implementation, state which existing TermRock types and call sites will be preserved, migrated, split, or deleted. Produce concrete Rust API sketches, implement the component, migrate its usages, add Studio stories and the appropriate visual, semantic, interaction, PTY, fuzz, and performance tests, then perform an independent second-pass design review.
```

## 61. PathInput — P1

```text
Using the TermRock Global implementation contract, redesign and implement: PathInput.

Component mission:
Create a filesystem-aware text input without coupling it to a specific filesystem implementation.

Component-specific requirements:
- Support path completion, tilde/environment presentation, existence/type status, relative/base path context, history, and browse action.
- Separate UI behavior from async filesystem lookup policy.
- Handle Windows and Unix path semantics where supported.
- Display potentially destructive targets clearly.
- Compose with FilePicker and connection/setup flows.

Research direction:
Study shell completion, file pickers, Yazi, and CLI setup tools.

Before implementation, state which existing TermRock types and call sites will be preserved, migrated, split, or deleted. Produce concrete Rust API sketches, implement the component, migrate its usages, add Studio stories and the appropriate visual, semantic, interaction, PTY, fuzz, and performance tests, then perform an independent second-pass design review.
```

## 62. TokenField — P1

```text
Using the TermRock Global implementation contract, redesign and implement: TokenField.

Component mission:
Create an editable collection of chips/tokens with free text and completion.

Component-specific requirements:
- Support add, remove, reorder, select, multi-select, duplicate policy, validation, and overflow.
- Define cursor movement between text and tokens without creating excessive Tab stops.
- Handle paste of multiple values and async suggestions.
- Use grapheme-safe editing and stable token IDs.
- Apply to recipients, filters, tags, file mentions, and command arguments.

Research direction:
Study token inputs, email recipient fields, and agent attachment/file mention composers.

Before implementation, state which existing TermRock types and call sites will be preserved, migrated, split, or deleted. Produce concrete Rust API sketches, implement the component, migrate its usages, add Studio stories and the appropriate visual, semantic, interaction, PTY, fuzz, and performance tests, then perform an independent second-pass design review.
```

## 63. Select — P1

```text
Using the TermRock Global implementation contract, redesign and implement: Select.

Component mission:
Create a single-choice select built from CollectionState, Popover, and roving focus.

Component-specific requirements:
- Support placeholder, labels/descriptions, disabled options, groups, separators, typeahead, search for long collections, and controlled value.
- Preserve opener focus and distinguish highlighted option from selected value.
- Handle tiny terminals by promoting the list to fullscreen.
- Provide inline, form, and compact toolbar recipes.
- Test dynamic option changes and nested overlays.

Research direction:
Study Radix Select, Huh select prompts, Textual Select, and terminal pickers.

Before implementation, state which existing TermRock types and call sites will be preserved, migrated, split, or deleted. Produce concrete Rust API sketches, implement the component, migrate its usages, add Studio stories and the appropriate visual, semantic, interaction, PTY, fuzz, and performance tests, then perform an independent second-pass design review.
```

## 64. MultiSelect — P1

```text
Using the TermRock Global implementation contract, redesign and implement: MultiSelect.

Component mission:
Create a searchable multiple-choice selector with clear selected-value presentation.

Component-specific requirements:
- Support check state, select all, groups, disabled options, maximum selection, chips, overflow summary, and clear/reset.
- Keep focus highlight distinct from checked items.
- Support range selection where meaningful.
- Provide compact summary and fullscreen selection modes.
- Use in filters, permissions, task selection, and schema tools.

Research direction:
Study modern multi-select controls, Huh, and terminal fuzzy pickers.

Before implementation, state which existing TermRock types and call sites will be preserved, migrated, split, or deleted. Produce concrete Rust API sketches, implement the component, migrate its usages, add Studio stories and the appropriate visual, semantic, interaction, PTY, fuzz, and performance tests, then perform an independent second-pass design review.
```

## 65. Combobox and Autocomplete — P1

```text
Using the TermRock Global implementation contract, redesign and implement: Combobox and Autocomplete.

Component mission:
Create an editable input plus suggestion collection for free-form or constrained values.

Component-specific requirements:
- Support async suggestions, loading, empty, error, groups, fuzzy highlights, creatable values, recent values, and exact-value validation.
- Define Enter, Tab, Escape, arrow, pointer, and blur semantics precisely.
- Keep typed text, active suggestion, and committed value separate.
- Use OverlayStack and CompletionMenu rather than private popup logic.
- Add race-condition tests for stale async results.

Research direction:
Study Radix Combobox patterns, prompt-toolkit completion, editor completion menus, and command palettes.

Before implementation, state which existing TermRock types and call sites will be preserved, migrated, split, or deleted. Produce concrete Rust API sketches, implement the component, migrate its usages, add Studio stories and the appropriate visual, semantic, interaction, PTY, fuzz, and performance tests, then perform an independent second-pass design review.
```

## 66. FilePicker — P2

```text
Using the TermRock Global implementation contract, redesign and implement: FilePicker.

Component mission:
Create a reusable file and directory selection component built from PathInput, Tree/List, and overlays.

Component-specific requirements:
- Support files/directories, single/multiple selection, hidden files, filters, sorting, breadcrumbs, preview, path entry, and permission errors.
- Keep filesystem operations application-provided and cancellable.
- Support keyboard-first navigation and optional mouse double-click/open behavior.
- Contract to fullscreen on small terminals.
- Provide Unix, Windows, SSH-like provider, and no-preview stories.

Research direction:
Study Yazi, ranger, lf, broot, desktop file dialogs, and terminal fuzzy finders.

Before implementation, state which existing TermRock types and call sites will be preserved, migrated, split, or deleted. Produce concrete Rust API sketches, implement the component, migrate its usages, add Studio stories and the appropriate visual, semantic, interaction, PTY, fuzz, and performance tests, then perform an independent second-pass design review.
```

## 67. DateTimePicker — P3

```text
Using the TermRock Global implementation contract, redesign and implement: DateTimePicker.

Component mission:
Create date, time, and range selection only where terminal applications genuinely benefit.

Component-specific requirements:
- Support direct text entry, calendar grid, time list, range selection, min/max, timezone display, and validation.
- Provide locale-independent storage with explicit presentation formatting.
- Use keyboard navigation, typeahead, and tiny-terminal list fallback.
- Make selected, today, focused, and unavailable states distinct without color.
- Document when a plain text field is the better choice.

Research direction:
Study shadcn Calendar/DatePicker concepts and Textual calendar widgets.

Before implementation, state which existing TermRock types and call sites will be preserved, migrated, split, or deleted. Produce concrete Rust API sketches, implement the component, migrate its usages, add Studio stories and the appropriate visual, semantic, interaction, PTY, fuzz, and performance tests, then perform an independent second-pass design review.
```

## 68. KeybindingRecorder — P2

```text
Using the TermRock Global implementation contract, redesign and implement: KeybindingRecorder.

Component mission:
Create a settings control for capturing and validating user keybindings.

Component-specific requirements:
- Capture chords/sequences across conventional and enhanced keyboard protocols.
- Show normalized semantic representation, conflicts, reserved bindings, and platform limitations.
- Support cancel, clear, restore default, and alternate bindings.
- Never trap the user without an escape path.
- Integrate with Keymap and KeyboardHelp.

Research direction:
Study editor keybinding settings and terminal protocol limitations.

Before implementation, state which existing TermRock types and call sites will be preserved, migrated, split, or deleted. Produce concrete Rust API sketches, implement the component, migrate its usages, add Studio stories and the appropriate visual, semantic, interaction, PTY, fuzz, and performance tests, then perform an independent second-pass design review.
```

## 69. FormWizard — P2

```text
Using the TermRock Global implementation contract, redesign and implement: FormWizard.

Component mission:
Create a multi-step form flow for setup, onboarding, connections, and migrations.

Component-specific requirements:
- Support step navigation, validation gates, optional steps, saved progress, review screen, async checks, and cancellation.
- Preserve data when moving backward and focus the first relevant field.
- Provide Stepper integration and narrow single-step layouts.
- Keep side effects in the application through typed outcomes.
- Add failure/retry and resume stories.

Research direction:
Study Huh forms, installers, cloud CLIs, and polished onboarding wizards.

Before implementation, state which existing TermRock types and call sites will be preserved, migrated, split, or deleted. Produce concrete Rust API sketches, implement the component, migrate its usages, add Studio stories and the appropriate visual, semantic, interaction, PTY, fuzz, and performance tests, then perform an independent second-pass design review.
```


# E. Navigation, menus, and overlay components

These components determine whether a complex terminal application feels discoverable and coherent. They must share semantic commands, focus rules, placement, and dismissal behavior.

## 70. Tabs — P1

```text
Using the TermRock Global implementation contract, redesign and implement: Tabs.

Component mission:
Create a composable tab system for views, documents, and question groups.

Component-specific requirements:
- Support manual/automatic activation, horizontal/vertical orientation, badges, status, close actions, overflow, reordering hooks, and disabled tabs.
- Use roving focus and preserve each panel's internal state.
- Contract into scrolling tabs, overflow menu, or Select on narrow widths.
- Distinguish tabs from segmented controls and navigation lists.
- Migrate current TermRock tabs and status stories.

Research direction:
Study Radix Tabs, terminal editors, Zellij, Posting, and browser tab overflow.

Before implementation, state which existing TermRock types and call sites will be preserved, migrated, split, or deleted. Produce concrete Rust API sketches, implement the component, migrate its usages, add Studio stories and the appropriate visual, semantic, interaction, PTY, fuzz, and performance tests, then perform an independent second-pass design review.
```

## 71. Sidebar and NavigationList — P1

```text
Using the TermRock Global implementation contract, redesign and implement: Sidebar and NavigationList.

Component mission:
Create primary application navigation with sections, hierarchy, badges, status, and responsive collapse.

Component-specific requirements:
- Support active route, focus, disabled items, groups, collapsible sections, contextual actions, and searchable large navigation.
- Collapse from full labels to compact rail, drawer, or command palette.
- Keep route state distinct from keyboard focus.
- Integrate with AppShell and semantic commands.
- Provide database, settings, and agent workbench examples.

Research direction:
Study IDE sidebars, Yazi panes, Posting, OpenCode, and shadcn sidebar patterns.

Before implementation, state which existing TermRock types and call sites will be preserved, migrated, split, or deleted. Produce concrete Rust API sketches, implement the component, migrate its usages, add Studio stories and the appropriate visual, semantic, interaction, PTY, fuzz, and performance tests, then perform an independent second-pass design review.
```

## 72. TreeNavigation — P2

```text
Using the TermRock Global implementation contract, redesign and implement: TreeNavigation.

Component mission:
Create hierarchical navigation distinct from a generic data Tree.

Component-specific requirements:
- Support routes, expansion, active ancestors, lazy children, badges, status, typeahead, and context actions.
- Define arrow semantics, parent navigation, and activation clearly.
- Preserve route selection through filtering and dynamic updates.
- Provide compact indentation and narrow fallback.
- Use in project, schema, settings, and documentation navigation.

Research direction:
Study file explorers, VS Code trees, Yazi, broot, and database navigators.

Before implementation, state which existing TermRock types and call sites will be preserved, migrated, split, or deleted. Produce concrete Rust API sketches, implement the component, migrate its usages, add Studio stories and the appropriate visual, semantic, interaction, PTY, fuzz, and performance tests, then perform an independent second-pass design review.
```

## 73. Breadcrumbs — P1

```text
Using the TermRock Global implementation contract, redesign and implement: Breadcrumbs.

Component mission:
Create location context and ancestor navigation for files, routes, schemas, and object paths.

Component-specific requirements:
- Support separators, ellipsis collapse, editable path mode, overflow menu, status, and current-item semantics.
- Allow keyboard navigation among ancestors without producing too many global Tab stops.
- Preserve first/root and current segments under contraction.
- Provide ASCII glyphs and no-color clarity.
- Compose with FilePicker, SchemaBrowser, and master-detail blocks.

Research direction:
Study desktop breadcrumbs, terminal file managers, and shadcn Breadcrumb.

Before implementation, state which existing TermRock types and call sites will be preserved, migrated, split, or deleted. Produce concrete Rust API sketches, implement the component, migrate its usages, add Studio stories and the appropriate visual, semantic, interaction, PTY, fuzz, and performance tests, then perform an independent second-pass design review.
```

## 74. Pagination — P2

```text
Using the TermRock Global implementation contract, redesign and implement: Pagination.

Component mission:
Create page navigation for remote datasets and bounded result sets without confusing it with scrolling.

Component-specific requirements:
- Support previous/next, first/last, page numbers, unknown totals, page size, loading, and disabled states.
- Provide compact summaries and direct page entry.
- Expose typed page requests while the application owns fetching.
- Contract intelligently on narrow widths.
- Document when virtualization is preferable.

Research direction:
Study shadcn Pagination, database clients, and API result browsers.

Before implementation, state which existing TermRock types and call sites will be preserved, migrated, split, or deleted. Produce concrete Rust API sketches, implement the component, migrate its usages, add Studio stories and the appropriate visual, semantic, interaction, PTY, fuzz, and performance tests, then perform an independent second-pass design review.
```

## 75. MenuBar — P2

```text
Using the TermRock Global implementation contract, redesign and implement: MenuBar.

Component mission:
Create desktop-style top-level menus for command-rich applications.

Component-specific requirements:
- Support nested menus, mnemonics, disabled/checked/radio items, separators, shortcuts, recent items, and dynamic commands.
- Integrate command metadata, OverlayStack, roving focus, and pointer behavior.
- Define platform-neutral Alt/mnemonic behavior and safe fallbacks.
- Support narrow replacement with CommandPalette.
- Add nested dismissal and focus restoration tests.

Research direction:
Study desktop menu bars, Textual menus, terminal editors, and Radix menus.

Before implementation, state which existing TermRock types and call sites will be preserved, migrated, split, or deleted. Produce concrete Rust API sketches, implement the component, migrate its usages, add Studio stories and the appropriate visual, semantic, interaction, PTY, fuzz, and performance tests, then perform an independent second-pass design review.
```

## 76. CommandPalette — P1

```text
Using the TermRock Global implementation contract, redesign and implement: CommandPalette.

Component mission:
Build a flagship universal command surface for every TermRock application.

Component-specific requirements:
- Support command search, fuzzy highlighting, groups, recent commands, contextual actions, disabled reasons, shortcuts, nested pages, arguments, and previews.
- Generate entries from the command/keymap system and semantic scene where appropriate.
- Support async results, stale-result cancellation, history, and direct execution.
- Promote to fullscreen on small terminals.
- Provide exceptionally polished empty, no-result, and loading states.

Research direction:
Study VS Code, Textual command palette, Posting, Zellij, television, and modern agent TUIs.

Before implementation, state which existing TermRock types and call sites will be preserved, migrated, split, or deleted. Produce concrete Rust API sketches, implement the component, migrate its usages, add Studio stories and the appropriate visual, semantic, interaction, PTY, fuzz, and performance tests, then perform an independent second-pass design review.
```

## 77. QuickOpen — P1

```text
Using the TermRock Global implementation contract, redesign and implement: QuickOpen.

Component mission:
Create a high-performance fuzzy resource opener for files, symbols, sessions, tables, commands, and arbitrary providers.

Component-specific requirements:
- Support provider switching, async streaming results, fuzzy ranges, recent items, previews, query syntax, and cancellation.
- Keep provider IO outside the component behind typed requests/results.
- Handle millions of logical candidates through incremental indexing or application-provided search.
- Preserve query and selection when switching providers.
- Integrate with FullscreenViewer and JumpMode.

Research direction:
Study fzf, television, VS Code Quick Open, Yazi, and command-line launchers.

Before implementation, state which existing TermRock types and call sites will be preserved, migrated, split, or deleted. Produce concrete Rust API sketches, implement the component, migrate its usages, add Studio stories and the appropriate visual, semantic, interaction, PTY, fuzz, and performance tests, then perform an independent second-pass design review.
```

## 78. JumpMode and FocusLens — P2

```text
Using the TermRock Global implementation contract, redesign and implement: JumpMode and FocusLens.

Component mission:
Create a terminal-native direct navigation overlay driven by the SemanticScene.

Component-specific requirements:
- Label focusable or actionable regions with short generated keys.
- Support filtering by role/action, nested targets, collision-free labels, and cancellation.
- Do not modify component implementations to participate beyond semantic registration.
- Provide visual treatment that remains legible over dense content and no-color mode.
- Add deterministic labeling and replay tests.

Research direction:
Study Vim easymotion, browser keyboard navigation extensions, Posting jump navigation, and accessibility focus inspectors.

Before implementation, state which existing TermRock types and call sites will be preserved, migrated, split, or deleted. Produce concrete Rust API sketches, implement the component, migrate its usages, add Studio stories and the appropriate visual, semantic, interaction, PTY, fuzz, and performance tests, then perform an independent second-pass design review.
```

## 79. Stepper — P1

```text
Using the TermRock Global implementation contract, redesign and implement: Stepper.

Component mission:
Create progress/navigation for multi-step flows.

Component-specific requirements:
- Support current, complete, error, optional, disabled, and future steps; horizontal and vertical variants; labels and descriptions.
- Allow navigation only when flow policy permits.
- Contract to compact numeric status or dropdown on narrow terminals.
- Keep step status understandable without color.
- Integrate with FormWizard, onboarding, plans, and migrations.

Research direction:
Study shadcn-inspired steppers, installers, and CI pipeline views.

Before implementation, state which existing TermRock types and call sites will be preserved, migrated, split, or deleted. Produce concrete Rust API sketches, implement the component, migrate its usages, add Studio stories and the appropriate visual, semantic, interaction, PTY, fuzz, and performance tests, then perform an independent second-pass design review.
```

## 80. HistoryPicker — P2

```text
Using the TermRock Global implementation contract, redesign and implement: HistoryPicker.

Component mission:
Create a reusable recent-history selector for commands, prompts, searches, sessions, and values.

Component-specific requirements:
- Support recency, pinning, search, deletion, metadata, grouping, and preview.
- Define privacy/redaction hooks for sensitive history.
- Preserve current draft when opening and cancelling.
- Use typed outcomes and application-owned persistence.
- Support compact popover and fullscreen variants.

Research direction:
Study shell history search, prompt histories, session pickers, and command palettes.

Before implementation, state which existing TermRock types and call sites will be preserved, migrated, split, or deleted. Produce concrete Rust API sketches, implement the component, migrate its usages, add Studio stories and the appropriate visual, semantic, interaction, PTY, fuzz, and performance tests, then perform an independent second-pass design review.
```

## 81. KeyboardHelp — P1

```text
Using the TermRock Global implementation contract, redesign and implement: KeyboardHelp.

Component mission:
Create contextual, generated keyboard and interaction help.

Component-specific requirements:
- Generate commands from active keymap, focus zone, overlays, and current component actions.
- Support compact footer hints, categorized modal help, search, conflicts, and user-remapped bindings.
- Include mouse equivalents and semantic action descriptions.
- Never show stale hardcoded shortcuts.
- Provide no-color and tiny-terminal layouts.

Research direction:
Study Zellij help, lazygit keybindings, Vim help, and Textual binding displays.

Before implementation, state which existing TermRock types and call sites will be preserved, migrated, split, or deleted. Produce concrete Rust API sketches, implement the component, migrate its usages, add Studio stories and the appropriate visual, semantic, interaction, PTY, fuzz, and performance tests, then perform an independent second-pass design review.
```

## 82. Tooltip — P1

```text
Using the TermRock Global implementation contract, redesign and implement: Tooltip.

Component mission:
Create delayed contextual help for truncated labels, icon buttons, statuses, and unfamiliar controls.

Component-specific requirements:
- Support focus-triggered and pointer-triggered display, delay, placement, clamping, dismissal, and disabled behavior.
- Never make essential information available only on hover.
- Avoid stealing focus or intercepting unrelated input.
- Provide plain text, shortcut, and rich compact variants.
- Disable or simplify animation under reduced motion.

Research direction:
Study Radix Tooltip and desktop tooltips, adapted to terminals with limited hover semantics.

Before implementation, state which existing TermRock types and call sites will be preserved, migrated, split, or deleted. Produce concrete Rust API sketches, implement the component, migrate its usages, add Studio stories and the appropriate visual, semantic, interaction, PTY, fuzz, and performance tests, then perform an independent second-pass design review.
```

## 83. Popover — P1

```text
Using the TermRock Global implementation contract, redesign and implement: Popover.

Component mission:
Create an anchored non-modal interactive surface for settings, filters, pickers, and details.

Component-specific requirements:
- Support anchors, placement, collision handling, focus entry, outside dismissal, nested overlays, and opener restoration.
- Define modal versus non-modal behavior explicitly.
- Contract to drawer/fullscreen when content cannot fit.
- Expose header/body/footer slots without forcing a Panel.
- Use OverlayStack rather than component-private geometry.

Research direction:
Study Radix Popover, terminal pickers, and Textual overlays.

Before implementation, state which existing TermRock types and call sites will be preserved, migrated, split, or deleted. Produce concrete Rust API sketches, implement the component, migrate its usages, add Studio stories and the appropriate visual, semantic, interaction, PTY, fuzz, and performance tests, then perform an independent second-pass design review.
```

## 84. DropdownMenu and ContextMenu — P1

```text
Using the TermRock Global implementation contract, redesign and implement: DropdownMenu and ContextMenu.

Component mission:
Create command menus with nested items, stateful items, shortcuts, and contextual placement.

Component-specific requirements:
- Support normal, checkbox, radio, submenu, separator, label, disabled reason, destructive, loading, and custom-preview items.
- Use roving focus, typeahead, semantic commands, and generated shortcut hints.
- Handle pointer opening, keyboard opening, right-click/context key, and nested dismissal.
- Promote deep or oversized menus to command-palette style pages.
- Add exhaustive nested overlay tests.

Research direction:
Study Radix menus, desktop context menus, Textual, lazygit, and file managers.

Before implementation, state which existing TermRock types and call sites will be preserved, migrated, split, or deleted. Produce concrete Rust API sketches, implement the component, migrate its usages, add Studio stories and the appropriate visual, semantic, interaction, PTY, fuzz, and performance tests, then perform an independent second-pass design review.
```

## 85. CompletionMenu — P1

```text
Using the TermRock Global implementation contract, redesign and implement: CompletionMenu.

Component mission:
Redesign completion as a reusable anchored suggestion surface for editors and inputs.

Component-specific requirements:
- Support groups, fuzzy ranges, kind glyphs, details, documentation preview, async updates, loading, empty, stale results, and commit characters.
- Preserve editor focus while navigating suggestions via active descendant semantics.
- Define Tab/Enter/Escape behavior through semantic intents.
- Clamp, flip, and promote to fullscreen on small terminals.
- Migrate the current TermRock completion component.

Research direction:
Study LSP completion UIs, prompt-toolkit, terminal editors, and Grok Build prompt completion.

Before implementation, state which existing TermRock types and call sites will be preserved, migrated, split, or deleted. Produce concrete Rust API sketches, implement the component, migrate its usages, add Studio stories and the appropriate visual, semantic, interaction, PTY, fuzz, and performance tests, then perform an independent second-pass design review.
```

## 86. Dialog — P1

```text
Using the TermRock Global implementation contract, redesign and implement: Dialog.

Component mission:
Create the canonical modal interaction surface.

Component-specific requirements:
- Support title, description, body, actions, close policy, focus trap, initial focus, opener restoration, scrolling content, loading, and validation.
- Provide normal, compact, wide, fullscreen, and destructive-adjacent recipes.
- Define Enter/default action and Escape behavior without accidental submission.
- Handle nested popovers and tiny terminals.
- Migrate current dialogs and remove duplicated modal logic.

Research direction:
Study Radix Dialog, Textual modals, Grok Build flows, and desktop dialog conventions.

Before implementation, state which existing TermRock types and call sites will be preserved, migrated, split, or deleted. Produce concrete Rust API sketches, implement the component, migrate its usages, add Studio stories and the appropriate visual, semantic, interaction, PTY, fuzz, and performance tests, then perform an independent second-pass design review.
```

## 87. AlertDialog — P1

```text
Using the TermRock Global implementation contract, redesign and implement: AlertDialog.

Component mission:
Create a specialized high-risk confirmation surface distinct from a generic Dialog.

Component-specific requirements:
- Communicate exact scope, consequences, reversibility, target, and safer alternatives.
- Support typed confirmation, countdown only when justified, destructive default policy, and non-dismissable critical state.
- Choose safe initial focus and prevent Enter-key accidents.
- Provide delete, overwrite, terminate, and data-egress examples.
- Test every dismissal and focus path.

Research direction:
Study Radix AlertDialog, database destructive actions, cloud consoles, and permission surfaces.

Before implementation, state which existing TermRock types and call sites will be preserved, migrated, split, or deleted. Produce concrete Rust API sketches, implement the component, migrate its usages, add Studio stories and the appropriate visual, semantic, interaction, PTY, fuzz, and performance tests, then perform an independent second-pass design review.
```

## 88. Drawer and Sheet — P1

```text
Using the TermRock Global implementation contract, redesign and implement: Drawer and Sheet.

Component mission:
Create edge-mounted secondary surfaces for responsive inspectors, task rails, filters, and details.

Component-specific requirements:
- Support left/right/top/bottom placement, modal/non-modal policy, resizable width, focus trap, opener restoration, and nested overlays.
- Use as a responsive replacement for sidebars and inspector panes.
- Provide compact handles and no-motion fallback.
- Handle full-screen and tiny terminal promotion.
- Preserve underlying view selection and scroll.

Research direction:
Study shadcn Sheet, mobile drawers, Zellij floating panes, and agent task sidebars.

Before implementation, state which existing TermRock types and call sites will be preserved, migrated, split, or deleted. Produce concrete Rust API sketches, implement the component, migrate its usages, add Studio stories and the appropriate visual, semantic, interaction, PTY, fuzz, and performance tests, then perform an independent second-pass design review.
```

## 89. FullscreenViewer and SemanticZoom — P1

```text
Using the TermRock Global implementation contract, redesign and implement: FullscreenViewer and SemanticZoom.

Component mission:
Create a reusable promotion path from compact row to inline detail to fullscreen inspection.

Component-specific requirements:
- Preserve component state, selection, focus, scroll anchor, and source context across promotion/demotion.
- Support title, breadcrumbs, actions, search, help, and close/restore behavior.
- Allow different content views without copying application state.
- Integrate with CodeBlock, DiffReview, logs, objects, tasks, and media.
- Define exact nested overlay Escape semantics.

Research direction:
Study Grok Build fullscreen overlays, file previews, IDE inspectors, and terminal pagers.

Before implementation, state which existing TermRock types and call sites will be preserved, migrated, split, or deleted. Produce concrete Rust API sketches, implement the component, migrate its usages, add Studio stories and the appropriate visual, semantic, interaction, PTY, fuzz, and performance tests, then perform an independent second-pass design review.
```

## 90. PreviewCard — P2

```text
Using the TermRock Global implementation contract, redesign and implement: PreviewCard.

Component mission:
Create a non-essential contextual preview for selected resources.

Component-specific requirements:
- Support delayed pointer/focus preview, metadata, rich content, loading, error, and pin-to-open actions.
- Never hide required information exclusively in the preview.
- Avoid focus theft and excessive redraw while selection changes rapidly.
- Use application-provided async data with stale-result cancellation.
- Provide file, command, symbol, and session examples.

Research direction:
Study IDE quick previews, hover cards, Yazi previews, and QuickOpen panels.

Before implementation, state which existing TermRock types and call sites will be preserved, migrated, split, or deleted. Produce concrete Rust API sketches, implement the component, migrate its usages, add Studio stories and the appropriate visual, semantic, interaction, PTY, fuzz, and performance tests, then perform an independent second-pass design review.
```


# F. Feedback, progress, and system-state components

Feedback components must communicate what is happening, why it is happening, whether the user can act, and what happens next. Decorative spinners are not enough.

## 91. Alert and Callout — P1

```text
Using the TermRock Global implementation contract, redesign and implement: Alert and Callout.

Component mission:
Create inline informational, success, warning, error, and destructive-context messages.

Component-specific requirements:
- Support title, description, details, actions, dismissibility, source, and status glyph.
- Use border/gutter/text hierarchy rather than huge background fills.
- Provide compact inline and prominent section recipes.
- Ensure state is understandable in no-color and ASCII modes.
- Compose with forms, diagnostics, permissions, and empty states.

Research direction:
Study shadcn Alert, Glow quote rails, CLI warnings, and system diagnostics.

Before implementation, state which existing TermRock types and call sites will be preserved, migrated, split, or deleted. Produce concrete Rust API sketches, implement the component, migrate its usages, add Studio stories and the appropriate visual, semantic, interaction, PTY, fuzz, and performance tests, then perform an independent second-pass design review.
```

## 92. Toast — P1

```text
Using the TermRock Global implementation contract, redesign and implement: Toast.

Component mission:
Redesign transient notifications around priority, actions, lifecycle, and non-disruptive placement.

Component-specific requirements:
- Support informational, success, warning, error, progress, undo, persistent, and grouped notifications.
- Manage queueing, deduplication, replacement, timeout pause, focus, and announcement semantics.
- Do not cover critical content or steal keyboard focus unexpectedly.
- Provide a route to NotificationCenter for missed items.
- Migrate current TermRock toast stories and timing.

Research direction:
Study shadcn/Sonner concepts, desktop notifications, Textual notifications, and agent task updates.

Before implementation, state which existing TermRock types and call sites will be preserved, migrated, split, or deleted. Produce concrete Rust API sketches, implement the component, migrate its usages, add Studio stories and the appropriate visual, semantic, interaction, PTY, fuzz, and performance tests, then perform an independent second-pass design review.
```

## 93. NotificationCenter — P2

```text
Using the TermRock Global implementation contract, redesign and implement: NotificationCenter.

Component mission:
Create a persistent history and action surface for application notifications.

Component-specific requirements:
- Support unread state, grouping, filtering, timestamps, actions, progress, source, dismissal, and clear-all.
- Keep persistence application-owned.
- Integrate with Toast without duplicating data models.
- Provide drawer and full-page recipes.
- Handle high-volume deduplication and accessibility.

Research direction:
Study desktop notification centers, CI dashboards, and task histories.

Before implementation, state which existing TermRock types and call sites will be preserved, migrated, split, or deleted. Produce concrete Rust API sketches, implement the component, migrate its usages, add Studio stories and the appropriate visual, semantic, interaction, PTY, fuzz, and performance tests, then perform an independent second-pass design review.
```

## 94. Spinner and ActivityIndicator — P1

```text
Using the TermRock Global implementation contract, redesign and implement: Spinner and ActivityIndicator.

Component mission:
Create semantic activity indicators with deterministic cadence.

Component-specific requirements:
- Every indicator must be paired with a meaningful verb or label unless embedded in a clearly labeled control.
- Support indeterminate, waiting, queued, reconnecting, and compact inline variants.
- Use capability-aware glyph sequences and reduced-motion fallback.
- Stop frame ticks when not visible or active.
- Add timing and idle-redraw tests.

Research direction:
Study terminal spinners, Textual loading indicators, and polished AI tool states.

Before implementation, state which existing TermRock types and call sites will be preserved, migrated, split, or deleted. Produce concrete Rust API sketches, implement the component, migrate its usages, add Studio stories and the appropriate visual, semantic, interaction, PTY, fuzz, and performance tests, then perform an independent second-pass design review.
```

## 95. ProgressBar — P1

```text
Using the TermRock Global implementation contract, redesign and implement: ProgressBar.

Component mission:
Create determinate and indeterminate progress with meaningful numeric and textual context.

Component-specific requirements:
- Support percentage, units, rate, ETA, phases, buffering, paused, cancelled, complete, and failed states.
- Provide compact, detailed, and multi-line recipes.
- Render accurately at tiny widths and without Unicode/color.
- Throttle updates and avoid unnecessary redraws.
- Integrate with task and transfer models.

Research direction:
Study Rich Progress, indicatif, btop bars, and download/build TUIs.

Before implementation, state which existing TermRock types and call sites will be preserved, migrated, split, or deleted. Produce concrete Rust API sketches, implement the component, migrate its usages, add Studio stories and the appropriate visual, semantic, interaction, PTY, fuzz, and performance tests, then perform an independent second-pass design review.
```

## 96. ProgressSteps — P1

```text
Using the TermRock Global implementation contract, redesign and implement: ProgressSteps.

Component mission:
Create pipeline and phase progress for builds, imports, migrations, and agent plans.

Component-specific requirements:
- Support queued, running, waiting, complete, skipped, warning, failed, retrying, and cancelled steps.
- Show durations, current verb, optional details, and retry actions.
- Distinguish interactive navigation from passive progress.
- Contract into compact summary on narrow terminals.
- Compose with Timeline and TaskRail.

Research direction:
Study CI pipelines, installers, and agent task plans.

Before implementation, state which existing TermRock types and call sites will be preserved, migrated, split, or deleted. Produce concrete Rust API sketches, implement the component, migrate its usages, add Studio stories and the appropriate visual, semantic, interaction, PTY, fuzz, and performance tests, then perform an independent second-pass design review.
```

## 97. Skeleton — P2

```text
Using the TermRock Global implementation contract, redesign and implement: Skeleton.

Component mission:
Create low-noise structural placeholders for content that will arrive asynchronously.

Component-specific requirements:
- Support text lines, rows, cards, tables, and custom shapes using terminal cells.
- Respect reduced motion and avoid constant shimmering by default.
- Preserve final layout to reduce jumping.
- Use only when structure is known; otherwise prefer explicit loading state.
- Add capability and tiny-size stories.

Research direction:
Study shadcn Skeleton and terminal loading placeholders, while avoiding gratuitous web mimicry.

Before implementation, state which existing TermRock types and call sites will be preserved, migrated, split, or deleted. Produce concrete Rust API sketches, implement the component, migrate its usages, add Studio stories and the appropriate visual, semantic, interaction, PTY, fuzz, and performance tests, then perform an independent second-pass design review.
```

## 98. StatusIndicator — P1

```text
Using the TermRock Global implementation contract, redesign and implement: StatusIndicator.

Component mission:
Create a compact semantic status primitive used across connections, tasks, agents, rows, and services.

Component-specific requirements:
- Support online, offline, idle, queued, running, waiting, success, warning, failed, paused, and unknown.
- Combine glyph, label, and style; color alone is insufficient.
- Provide dot-like compact, labeled, and elapsed-time variants.
- Use a shared status vocabulary to prevent component inconsistency.
- Add mapping tests across capability profiles.

Research direction:
Study btop, process monitors, collaboration presence, and agent status surfaces.

Before implementation, state which existing TermRock types and call sites will be preserved, migrated, split, or deleted. Produce concrete Rust API sketches, implement the component, migrate its usages, add Studio stories and the appropriate visual, semantic, interaction, PTY, fuzz, and performance tests, then perform an independent second-pass design review.
```

## 99. EmptyState — P1

```text
Using the TermRock Global implementation contract, redesign and implement: EmptyState.

Component mission:
Create useful empty and first-run states rather than blank boxes.

Component-specific requirements:
- Support title, explanation, primary action, secondary action, example, shortcut, illustration glyph, and contextual details.
- Differentiate first use, no data, no results, filtered-out, and permission-limited states.
- Contract to concise inline form inside small panes.
- Keep primary action dominant and safe.
- Provide table, logs, sessions, projects, and search examples.

Research direction:
Study shadcn empty-state patterns, IDE welcome screens, and polished CLI onboarding.

Before implementation, state which existing TermRock types and call sites will be preserved, migrated, split, or deleted. Produce concrete Rust API sketches, implement the component, migrate its usages, add Studio stories and the appropriate visual, semantic, interaction, PTY, fuzz, and performance tests, then perform an independent second-pass design review.
```

## 100. ErrorState and Recovery — P1

```text
Using the TermRock Global implementation contract, redesign and implement: ErrorState and Recovery.

Component mission:
Create structured recoverable failure presentation.

Component-specific requirements:
- Support summary, human explanation, technical details, source, retry, alternative action, copy diagnostics, and report issue.
- Differentiate validation, network, permission, not-found, conflict, crash, and unsupported-capability errors.
- Hide technical detail initially without making it inaccessible.
- Preserve user work and explain whether retry is safe.
- Provide inline, pane, dialog, and full-screen recipes.

Research direction:
Study browser/IDE error surfaces, cloud CLIs, and terminal crash recovery.

Before implementation, state which existing TermRock types and call sites will be preserved, migrated, split, or deleted. Produce concrete Rust API sketches, implement the component, migrate its usages, add Studio stories and the appropriate visual, semantic, interaction, PTY, fuzz, and performance tests, then perform an independent second-pass design review.
```

## 101. LoadingOverlay and BusyBoundary — P2

```text
Using the TermRock Global implementation contract, redesign and implement: LoadingOverlay and BusyBoundary.

Component mission:
Create a coordinated loading state that can block only the affected region rather than freezing an entire application.

Component-specific requirements:
- Support non-blocking busy state, blocking operation, cancellable operation, optimistic state, and stale-content presentation.
- Preserve readable content where safe and explain what is unavailable.
- Manage focus and input routing explicitly.
- Avoid overlay abuse for short operations.
- Add nested region and cancellation tests.

Research direction:
Study async UI boundaries, Textual workers, and command execution in agent tools.

Before implementation, state which existing TermRock types and call sites will be preserved, migrated, split, or deleted. Produce concrete Rust API sketches, implement the component, migrate its usages, add Studio stories and the appropriate visual, semantic, interaction, PTY, fuzz, and performance tests, then perform an independent second-pass design review.
```

## 102. Offline and ReconnectingState — P2

```text
Using the TermRock Global implementation contract, redesign and implement: Offline and ReconnectingState.

Component mission:
Create a specialized connectivity state for remote sessions, databases, agents, and services.

Component-specific requirements:
- Show connection target, last successful time, retry state, queued actions, offline capabilities, and manual actions.
- Distinguish disconnected, reconnecting, authentication required, and server unavailable.
- Preserve local drafts and selection.
- Provide unobtrusive banner plus full error variant.
- Integrate with StatusBar and NotificationCenter.

Research direction:
Study remote IDEs, database clients, SSH tools, and collaborative agent interfaces.

Before implementation, state which existing TermRock types and call sites will be preserved, migrated, split, or deleted. Produce concrete Rust API sketches, implement the component, migrate its usages, add Studio stories and the appropriate visual, semantic, interaction, PTY, fuzz, and performance tests, then perform an independent second-pass design review.
```


# G. Data, developer-tool, and observability components

These components are central to TermRock's developer-tool identity. They require virtualization, precise selection, copy behavior, search, responsive column priority, and exceptional performance.

## 103. List — P1

```text
Using the TermRock Global implementation contract, redesign and implement: List.

Component mission:
Redesign the current List as a composable collection view rather than a label-only widget.

Component-specific requirements:
- Support leading content, primary label, secondary metadata, badge, status, trailing actions, shortcut, group headers, separators, and custom row rendering.
- Use CollectionState, SelectionModel, RovingFocusGroup, ScrollArea, and semantic intents.
- Support single/multiple/range selection, typeahead, search, disabled rows, loading, empty, and virtualization.
- Define compact and comfortable density plus narrow priority rules.
- Migrate current direct key handling.

Research direction:
Study lazygit lists, Yazi, Textual ListView, shadcn command items, and current TermRock List.

Before implementation, state which existing TermRock types and call sites will be preserved, migrated, split, or deleted. Produce concrete Rust API sketches, implement the component, migrate its usages, add Studio stories and the appropriate visual, semantic, interaction, PTY, fuzz, and performance tests, then perform an independent second-pass design review.
```

## 104. VirtualList — P1

```text
Using the TermRock Global implementation contract, redesign and implement: VirtualList.

Component mission:
Create a high-performance list for extremely large or streaming collections.

Component-specific requirements:
- Use the shared Virtualizer with stable IDs, overscan, variable heights where justified, sticky headers, and anchor preservation.
- Support async page loading, placeholders, follow-tail, filtering, and live updates.
- Keep semantic registration limited to visible/near-visible items.
- Expose visible ranges and item measurement diagnostics.
- Benchmark million-row logical datasets.

Research direction:
Study Textual virtual lists, VisiData, logs, and current TermRock virtualization.

Before implementation, state which existing TermRock types and call sites will be preserved, migrated, split, or deleted. Produce concrete Rust API sketches, implement the component, migrate its usages, add Studio stories and the appropriate visual, semantic, interaction, PTY, fuzz, and performance tests, then perform an independent second-pass design review.
```

## 105. Tree — P1

```text
Using the TermRock Global implementation contract, redesign and implement: Tree.

Component mission:
Redesign the current Tree for files, schemas, tasks, settings, and object hierarchies.

Component-specific requirements:
- Support stable IDs, lazy children, loading/error child state, expansion, active cursor, selection, check state, icons, metadata, context actions, and typeahead.
- Define left/right parent-child navigation and collapse semantics precisely.
- Support filtering while retaining ancestor context.
- Virtualize large expanded trees and preserve scroll anchors.
- Provide ASCII indentation/glyph fallbacks.

Research direction:
Study file explorers, broot, Yazi, VS Code trees, and current TermRock Tree.

Before implementation, state which existing TermRock types and call sites will be preserved, migrated, split, or deleted. Produce concrete Rust API sketches, implement the component, migrate its usages, add Studio stories and the appropriate visual, semantic, interaction, PTY, fuzz, and performance tests, then perform an independent second-pass design review.
```

## 106. Table — P1

```text
Using the TermRock Global implementation contract, redesign and implement: Table.

Component mission:
Redesign the existing Table as a polished static or moderately sized data presentation component.

Component-specific requirements:
- Support headers, alignment, widths, wrapping/truncation, row/cell focus, row selection, empty/loading/error states, and responsive column priorities.
- Provide quiet, bordered, striped-by-symbol/spacing, and compact recipes without visual noise.
- Support sticky header and horizontal scrolling.
- Separate display model from interactive DataTable behavior.
- Improve current screenshot hierarchy and selection treatment.

Research direction:
Study Rich tables, Glow tables, database clients, btop, and current TermRock Table.

Before implementation, state which existing TermRock types and call sites will be preserved, migrated, split, or deleted. Produce concrete Rust API sketches, implement the component, migrate its usages, add Studio stories and the appropriate visual, semantic, interaction, PTY, fuzz, and performance tests, then perform an independent second-pass design review.
```

## 107. DataTable — P1

```text
Using the TermRock Global implementation contract, redesign and implement: DataTable.

Component mission:
Create a category-leading interactive and virtualized table for professional developer tools.

Component-specific requirements:
- Support sorting, filtering, search, column resizing, visibility, pinning, reordering hooks, grouping, row/cell/range selection, inline editing, copy, and context actions.
- Support sticky headers/columns, remote loading, partial data, unknown totals, and million-row logical datasets.
- Define keyboard navigation modes and pointer resizing/selection precisely.
- Use responsive column priorities and fullscreen promotion.
- Add serious benchmarks and VisiData-like usage stories.

Research direction:
Study VisiData, Textual DataTable, database clients, k9s, btop, and spreadsheet interaction patterns.

Before implementation, state which existing TermRock types and call sites will be preserved, migrated, split, or deleted. Produce concrete Rust API sketches, implement the component, migrate its usages, add Studio stories and the appropriate visual, semantic, interaction, PTY, fuzz, and performance tests, then perform an independent second-pass design review.
```

## 108. TreeTable — P2

```text
Using the TermRock Global implementation contract, redesign and implement: TreeTable.

Component mission:
Combine hierarchical rows with columns without producing confusing navigation.

Component-specific requirements:
- Support expansion, lazy children, sticky headers, sortable columns where semantically valid, selection, grouping, and aggregate rows.
- Define whether left/right controls hierarchy, horizontal scroll, or cells based on interaction mode.
- Virtualize visible expanded rows.
- Use responsive column priorities and compact hierarchy indentation.
- Provide process tree, schema, task, and dependency examples.

Research direction:
Study process-tree views, file trees with metadata, IDE outlines, and database schema browsers.

Before implementation, state which existing TermRock types and call sites will be preserved, migrated, split, or deleted. Produce concrete Rust API sketches, implement the component, migrate its usages, add Studio stories and the appropriate visual, semantic, interaction, PTY, fuzz, and performance tests, then perform an independent second-pass design review.
```

## 109. KeyValueTable — P1

```text
Using the TermRock Global implementation contract, redesign and implement: KeyValueTable.

Component mission:
Create a dense interactive detail table for metadata, settings, headers, and object properties.

Component-specific requirements:
- Support key, value, type, source, status, copy, edit, secret redaction, nested groups, and validation.
- Contract from columns to stacked rows.
- Allow row navigation and per-value actions without excessive focus targets.
- Provide compare/diff mode.
- Use in HTTP, database, process, permission, and agent detail panels.

Research direction:
Study inspector panels, HTTP clients, cloud consoles, and current TermRock detail tables.

Before implementation, state which existing TermRock types and call sites will be preserved, migrated, split, or deleted. Produce concrete Rust API sketches, implement the component, migrate its usages, add Studio stories and the appropriate visual, semantic, interaction, PTY, fuzz, and performance tests, then perform an independent second-pass design review.
```

## 110. ObjectInspector — P1

```text
Using the TermRock Global implementation contract, redesign and implement: ObjectInspector.

Component mission:
Create an expandable typed inspector for JSON, YAML, TOML, structured logs, Rust debug data, and arbitrary application trees.

Component-specific requirements:
- Support objects, arrays, scalar types, paths, type-aware formatting, lazy expansion, search, copy path/value, edit hooks, and diff/compare.
- Preserve expansion across updates using stable paths or IDs.
- Handle huge/deep structures through virtualization and depth limits.
- Provide compact inline preview and fullscreen inspection.
- Guarantee escaped control characters and safe secret redaction.

Research direction:
Study browser devtools, jq viewers, fx, Textual trees, and database JSON inspectors.

Before implementation, state which existing TermRock types and call sites will be preserved, migrated, split, or deleted. Produce concrete Rust API sketches, implement the component, migrate its usages, add Studio stories and the appropriate visual, semantic, interaction, PTY, fuzz, and performance tests, then perform an independent second-pass design review.
```

## 111. Timeline — P1

```text
Using the TermRock Global implementation contract, redesign and implement: Timeline.

Component mission:
Create chronological event presentation for sessions, tasks, deployments, traces, and agent turns.

Component-specific requirements:
- Support timestamps, relative time, duration, actor, status, grouping, expansion, correlation, filters, and live streaming.
- Preserve reading position while new events arrive.
- Offer compact rail, detailed list, and grouped-day recipes.
- Use symbols and labels in no-color mode.
- Compose with CheckpointTimeline, EventStream, and task history.

Research direction:
Study Git history, CI timelines, observability tools, and agent session views.

Before implementation, state which existing TermRock types and call sites will be preserved, migrated, split, or deleted. Produce concrete Rust API sketches, implement the component, migrate its usages, add Studio stories and the appropriate visual, semantic, interaction, PTY, fuzz, and performance tests, then perform an independent second-pass design review.
```

## 112. EventStream — P2

```text
Using the TermRock Global implementation contract, redesign and implement: EventStream.

Component mission:
Create a generic high-volume structured-event viewer distinct from plain logs.

Component-specific requirements:
- Support event type, timestamp, severity, actor/source, structured fields, correlation IDs, filtering, grouping, pause/follow, and details.
- Allow pluggable row summaries and inspector detail.
- Handle bursty streams with batching and backpressure indicators.
- Preserve stable anchors and unread counts.
- Benchmark sustained event rates.

Research direction:
Study observability event consoles, Kubernetes events, and agent activity streams.

Before implementation, state which existing TermRock types and call sites will be preserved, migrated, split, or deleted. Produce concrete Rust API sketches, implement the component, migrate its usages, add Studio stories and the appropriate visual, semantic, interaction, PTY, fuzz, and performance tests, then perform an independent second-pass design review.
```

## 113. LogStream — P1

```text
Using the TermRock Global implementation contract, redesign and implement: LogStream.

Component mission:
Redesign the log viewer for continuous professional use.

Component-specific requirements:
- Support follow-tail, pause, unread/new-lines indicator, timestamps, source, severity, ANSI, wrapping, horizontal scroll, search, filters, bookmarks, selection, copy, and export outcomes.
- Handle bounded history, dropped-line indicators, burst batching, and reconnects.
- Use virtualization and stable anchors.
- Provide compact and detailed line recipes.
- Migrate current LogPane behavior deliberately.

Research direction:
Study k9s, stern-style workflows, Textual logs, btop, and current TermRock LogPane.

Before implementation, state which existing TermRock types and call sites will be preserved, migrated, split, or deleted. Produce concrete Rust API sketches, implement the component, migrate its usages, add Studio stories and the appropriate visual, semantic, interaction, PTY, fuzz, and performance tests, then perform an independent second-pass design review.
```

## 114. DiffView — P1

```text
Using the TermRock Global implementation contract, redesign and implement: DiffView.

Component mission:
Create a high-quality read-only unified and side-by-side diff renderer.

Component-specific requirements:
- Support files, hunks, line numbers, syntax, additions/deletions/context, whitespace markers, word-level changes, folding, search, and navigation.
- Handle narrow terminals by choosing unified mode automatically.
- Virtualize large diffs and preserve file/hunk anchors.
- Use semantic diff tokens and no-color prefixes.
- Migrate and improve current TermRock DiffView.

Research direction:
Study delta, lazygit, GitUI, review tools, and current TermRock diff screenshots.

Before implementation, state which existing TermRock types and call sites will be preserved, migrated, split, or deleted. Produce concrete Rust API sketches, implement the component, migrate its usages, add Studio stories and the appropriate visual, semantic, interaction, PTY, fuzz, and performance tests, then perform an independent second-pass design review.
```

## 115. DiffReview — P1

```text
Using the TermRock Global implementation contract, redesign and implement: DiffReview.

Component mission:
Build interactive review behavior on top of DiffView.

Component-specific requirements:
- Support file tree, hunk and line-range selection, comments, approve/reject/apply decisions, staging-like actions, external editor, and review summary.
- Separate application-specific version-control policy from reusable review state.
- Preserve comments and selection across mode/resize changes.
- Provide safe destructive language and undo where possible.
- Use in Git, plan changes, and AI-agent code review.

Research direction:
Study GitHub reviews, lazygit staging, Grok Build plan review, and agent diff approval.

Before implementation, state which existing TermRock types and call sites will be preserved, migrated, split, or deleted. Produce concrete Rust API sketches, implement the component, migrate its usages, add Studio stories and the appropriate visual, semantic, interaction, PTY, fuzz, and performance tests, then perform an independent second-pass design review.
```

## 116. Diagnostic and CodeFrame — P1

```text
Using the TermRock Global implementation contract, redesign and implement: Diagnostic and CodeFrame.

Component mission:
Create structured diagnostics with source context and actionable fixes.

Component-specific requirements:
- Support severity, code, message, source, range, related locations, notes, help, documentation link, suggested fixes, and copyable details.
- Render single- and multi-line spans, tabs, Unicode, overlapping diagnostics, and truncated files correctly.
- Provide list, inline, and full code-frame recipes.
- Do not use color alone for severity.
- Integrate with build output, editors, forms, and ErrorState.

Research direction:
Study Rust compiler diagnostics, miette, Rich tracebacks, IDE problems panels, and code frames.

Before implementation, state which existing TermRock types and call sites will be preserved, migrated, split, or deleted. Produce concrete Rust API sketches, implement the component, migrate its usages, add Studio stories and the appropriate visual, semantic, interaction, PTY, fuzz, and performance tests, then perform an independent second-pass design review.
```

## 117. TerminalOutput — P1

```text
Using the TermRock Global implementation contract, redesign and implement: TerminalOutput.

Component mission:
Create a safe terminal command execution/output presentation component.

Component-specific requirements:
- Support command, working directory, environment summary/redaction, stdout/stderr distinction, live streaming, exit status, signal, duration, cancel, detach, retry, and copy.
- Parse ANSI safely and support raw/plain modes.
- Preserve scroll when user is reading while output continues.
- Provide compact card, pane, and fullscreen modes.
- Never execute commands itself; emit typed control requests.

Research direction:
Study Grok Build, Amp, OpenCode, terminal emulators, and CI command logs.

Before implementation, state which existing TermRock types and call sites will be preserved, migrated, split, or deleted. Produce concrete Rust API sketches, implement the component, migrate its usages, add Studio stories and the appropriate visual, semantic, interaction, PTY, fuzz, and performance tests, then perform an independent second-pass design review.
```

## 118. HexViewer — P2

```text
Using the TermRock Global implementation contract, redesign and implement: HexViewer.

Component mission:
Create a virtualized binary inspector.

Component-specific requirements:
- Support offsets, configurable bytes per row, hex, ASCII/Unicode interpretation, selection, search, bookmarks, endianness-aware value inspector, and copy/export outcomes.
- Handle massive files through application-provided paging.
- Provide clear active byte and selected range visuals without color dependence.
- Support tiny-terminal compact mode.
- Add correctness/property tests for offsets and widths.

Research direction:
Study hex editors, xxd, and binary-analysis tools.

Before implementation, state which existing TermRock types and call sites will be preserved, migrated, split, or deleted. Produce concrete Rust API sketches, implement the component, migrate its usages, add Studio stories and the appropriate visual, semantic, interaction, PTY, fuzz, and performance tests, then perform an independent second-pass design review.
```

## 119. Sparkline, Chart, Gauge, and Histogram — P2

```text
Using the TermRock Global implementation contract, redesign and implement: Sparkline, Chart, Gauge, and Histogram.

Component mission:
Create a coherent terminal data-visualization family rather than unrelated graph widgets.

Component-specific requirements:
- Support time series, bars, stacked bars, histogram buckets, gauges, thresholds, labels, legends, missing data, and selected points.
- Use braille/block/ASCII capability fallbacks with consistent scale semantics.
- Provide autoscale, fixed scale, logarithmic where justified, and time-window behavior.
- Keep charts readable in no-color mode through line styles, glyphs, labels, and ordering.
- Benchmark streaming updates and tiny dimensions.

Research direction:
Study btop, bottom, gping, Ratatui charts, and observability dashboards.

Before implementation, state which existing TermRock types and call sites will be preserved, migrated, split, or deleted. Produce concrete Rust API sketches, implement the component, migrate its usages, add Studio stories and the appropriate visual, semantic, interaction, PTY, fuzz, and performance tests, then perform an independent second-pass design review.
```

## 120. FileTree — P1

```text
Using the TermRock Global implementation contract, redesign and implement: FileTree.

Component mission:
Create a file-system-specialized Tree with status, filtering, and file operations expressed as typed requests.

Component-specific requirements:
- Support git status, file types, hidden files, ignored files, lazy directories, search, reveal active file, multi-select, rename/create/delete requests, and preview integration.
- Keep filesystem and Git IO outside the component.
- Handle symlinks, permission errors, huge directories, and path normalization.
- Provide Yazi-like keyboard efficiency and safe destructive flows.
- Integrate with QuickOpen and Breadcrumbs.

Research direction:
Study Yazi, ranger, lf, broot, VS Code, and lazygit file lists.

Before implementation, state which existing TermRock types and call sites will be preserved, migrated, split, or deleted. Produce concrete Rust API sketches, implement the component, migrate its usages, add Studio stories and the appropriate visual, semantic, interaction, PTY, fuzz, and performance tests, then perform an independent second-pass design review.
```

## 121. ProcessTable — P2

```text
Using the TermRock Global implementation contract, redesign and implement: ProcessTable.

Component mission:
Create a process and task monitoring table with tree and flat modes.

Component-specific requirements:
- Support PID, command, CPU, memory, status, user, elapsed time, hierarchy, search, sort, filters, details, signals, and refresh cadence.
- Preserve selection across refresh and process churn.
- Use stable identity carefully when PIDs are reused.
- Provide safe signal/terminate/kill confirmation flows.
- Benchmark frequent updates and thousands of rows.

Research direction:
Study btop, bottom, htop, procs, and process explorers.

Before implementation, state which existing TermRock types and call sites will be preserved, migrated, split, or deleted. Produce concrete Rust API sketches, implement the component, migrate its usages, add Studio stories and the appropriate visual, semantic, interaction, PTY, fuzz, and performance tests, then perform an independent second-pass design review.
```

## 122. QueryEditor — P2

```text
Using the TermRock Global implementation contract, redesign and implement: QueryEditor.

Component mission:
Create a code-oriented editor for SQL, logs, search languages, and structured queries.

Component-specific requirements:
- Support syntax, multiline editing, completion, diagnostics, parameters, history, run/stop, selection execution, formatting request, and saved queries.
- Keep language services and execution application-provided.
- Integrate CompletionMenu, CodeFrame, KeybindingHelp, and ResultGrid.
- Preserve draft and cursor across result focus changes.
- Provide compact and fullscreen modes.

Research direction:
Study TablePlus-like query editors, database TUIs, Grafana-like query workflows, and terminal editors.

Before implementation, state which existing TermRock types and call sites will be preserved, migrated, split, or deleted. Produce concrete Rust API sketches, implement the component, migrate its usages, add Studio stories and the appropriate visual, semantic, interaction, PTY, fuzz, and performance tests, then perform an independent second-pass design review.
```

## 123. ResultGrid — P2

```text
Using the TermRock Global implementation contract, redesign and implement: ResultGrid.

Component mission:
Create a database/query result component built on DataTable with data-specific behaviors.

Component-specific requirements:
- Support typed cells, nulls, binary values, large text, row numbers, copy/export, column statistics, editable cells, pagination/streaming, and query status.
- Provide cell detail and object inspection for structured values.
- Handle very wide schemas and unknown row counts.
- Use safe display/redaction for secrets and binary data.
- Integrate with QueryEditor and schema context.

Research direction:
Study database clients, VisiData, TablePlus, and SQL terminal tools.

Before implementation, state which existing TermRock types and call sites will be preserved, migrated, split, or deleted. Produce concrete Rust API sketches, implement the component, migrate its usages, add Studio stories and the appropriate visual, semantic, interaction, PTY, fuzz, and performance tests, then perform an independent second-pass design review.
```

## 124. SchemaBrowser — P2

```text
Using the TermRock Global implementation contract, redesign and implement: SchemaBrowser.

Component mission:
Create a hierarchical database/schema navigator.

Component-specific requirements:
- Support connections, databases, schemas, tables, views, columns, indexes, constraints, routines, loading/error states, search, status, and context actions.
- Use lazy expansion and application-owned metadata fetching.
- Provide detail previews and QuickOpen integration.
- Preserve expanded state across refresh and reconnect.
- Contract from side pane to drawer/fullscreen.

Research direction:
Study TablePlus, DataGrip, pgcli ecosystems, and file-tree navigation.

Before implementation, state which existing TermRock types and call sites will be preserved, migrated, split, or deleted. Produce concrete Rust API sketches, implement the component, migrate its usages, add Studio stories and the appropriate visual, semantic, interaction, PTY, fuzz, and performance tests, then perform an independent second-pass design review.
```

## 125. SearchResults — P1

```text
Using the TermRock Global implementation contract, redesign and implement: SearchResults.

Component mission:
Create grouped, navigable search results for files, logs, objects, commands, and documentation.

Component-specific requirements:
- Support result groups, match ranges, snippets, source metadata, status, pagination/streaming, selection, preview, and open action.
- Keep important matched text visible under truncation.
- Support keyboard next/previous match and group collapse.
- Handle stale async searches and cancellation.
- Compose with SearchInput, QuickOpen, and FullscreenViewer.

Research direction:
Study ripgrep UIs, IDE search, fzf previews, and documentation search.

Before implementation, state which existing TermRock types and call sites will be preserved, migrated, split, or deleted. Produce concrete Rust API sketches, implement the component, migrate its usages, add Studio stories and the appropriate visual, semantic, interaction, PTY, fuzz, and performance tests, then perform an independent second-pass design review.
```

## 126. MetricsDashboard — P2

```text
Using the TermRock Global implementation contract, redesign and implement: MetricsDashboard.

Component mission:
Create a reusable dashboard block composed from metric cards, charts, tables, alerts, and time controls.

Component-specific requirements:
- Support time range, refresh, comparison, thresholds, drill-down, loading, partial failure, and responsive grid layouts.
- Prioritize trend and exception readability over decoration.
- Provide keyboard spatial navigation and command palette actions.
- Contract into a vertical summary on narrow terminals.
- Use only public component APIs.

Research direction:
Study btop, Grafana concepts, observability TUIs, and operating dashboards.

Before implementation, state which existing TermRock types and call sites will be preserved, migrated, split, or deleted. Produce concrete Rust API sketches, implement the component, migrate its usages, add Studio stories and the appropriate visual, semantic, interaction, PTY, fuzz, and performance tests, then perform an independent second-pass design review.
```

## 127. TraceWaterfall — P3

```text
Using the TermRock Global implementation contract, redesign and implement: TraceWaterfall.

Component mission:
Create a hierarchical span and latency visualization for distributed traces and agent/tool execution.

Component-specific requirements:
- Support nested spans, duration bars, critical path, status, service/actor, search, filters, zoom, selection, and details.
- Provide readable ASCII fallback and exact time labels.
- Virtualize large traces and preserve horizontal time navigation.
- Distinguish hierarchy navigation from timeline scrolling.
- Compose with ObjectInspector and Timeline.

Research direction:
Study trace viewers, Chrome devtools waterfall, and agent activity timelines.

Before implementation, state which existing TermRock types and call sites will be preserved, migrated, split, or deleted. Produce concrete Rust API sketches, implement the component, migrate its usages, add Studio stories and the appropriate visual, semantic, interaction, PTY, fuzz, and performance tests, then perform an independent second-pass design review.
```

## 128. DependencyGraph — P3

```text
Using the TermRock Global implementation contract, redesign and implement: DependencyGraph.

Component mission:
Create a constrained graph viewer for package, service, schema, and task dependencies.

Component-specific requirements:
- Support nodes, edges, direction, status, selection, search, filtering, grouping, details, and alternative list/tree representation.
- Do not promise arbitrary graph layout quality beyond terminal constraints.
- Provide deterministic layouts, pan/zoom-like navigation, and ASCII connectors.
- Fallback to TreeTable when the graph is unreadable.
- Benchmark moderate real-world graphs.

Research direction:
Study terminal graph tools, dependency trees, service maps, and FTXUI canvases.

Before implementation, state which existing TermRock types and call sites will be preserved, migrated, split, or deleted. Produce concrete Rust API sketches, implement the component, migrate its usages, add Studio stories and the appropriate visual, semantic, interaction, PTY, fuzz, and performance tests, then perform an independent second-pass design review.
```


# H. AI-agent and autonomous-workflow components

This collection should become TermRock's signature differentiator. These are reusable, provider-neutral interaction patterns extracted from excellent agent TUIs rather than monolithic product-specific views.

## 129. PromptComposer — P0

```text
Using the TermRock Global implementation contract, redesign and implement: PromptComposer.

Component mission:
Build the flagship input surface for terminal AI agents.

Component-specific requirements:
- Support multiline grapheme-safe editing, selection, undo/redo, history, attachments, paste chips, slash commands, file/symbol mentions, completion, model/mode indicators, queueing, submit, interrupt, cancel, and external editor.
- Preserve draft when permissions, questions, plans, sessions, or command palette temporarily take over.
- Separate text editor, token model, completion, presentation, and application submission policy.
- Provide compact, normal, expanded, and fullscreen modes.
- Benchmark large prompts, streaming completion updates, and repeated paste.

Research direction:
Study Grok Build prompt widget, Amp, OpenCode, Claude Code, prompt-toolkit, and terminal editors.

Before implementation, state which existing TermRock types and call sites will be preserved, migrated, split, or deleted. Produce concrete Rust API sketches, implement the component, migrate its usages, add Studio stories and the appropriate visual, semantic, interaction, PTY, fuzz, and performance tests, then perform an independent second-pass design review.
```

## 130. AttachmentChip and PasteChip — P1

```text
Using the TermRock Global implementation contract, redesign and implement: AttachmentChip and PasteChip.

Component mission:
Create structured compact representations of files, images, URLs, selected code, and large pasted text.

Component-specific requirements:
- Support type, name, size/line count, status, validation, remove, open/preview, retry, and upload/indexing progress.
- Collapse large pasted text while preserving inspectability and copy behavior.
- Use stable IDs and avoid exposing sensitive content in semantic summaries or recordings.
- Support wrapping, horizontal scrolling, and overflow summaries.
- Compose with PromptComposer and permission/data-egress flows.

Research direction:
Study Grok Build paste/file chips, modern chat attachments, and terminal prompt composers.

Before implementation, state which existing TermRock types and call sites will be preserved, migrated, split, or deleted. Produce concrete Rust API sketches, implement the component, migrate its usages, add Studio stories and the appropriate visual, semantic, interaction, PTY, fuzz, and performance tests, then perform an independent second-pass design review.
```

## 131. FileMention and EntityMention — P1

```text
Using the TermRock Global implementation contract, redesign and implement: FileMention and EntityMention.

Component mission:
Create inline structured tokens for files, symbols, agents, tools, sessions, and resources.

Component-specific requirements:
- Support display label, canonical ID/path, type glyph, validity, stale/missing state, preview, and removal.
- Integrate completion, keyboard navigation, copy, and semantic descriptions.
- Keep cursor movement intuitive across text/token boundaries.
- Support ambiguous names and disambiguation.
- Avoid embedding provider-specific resource lookup.

Research direction:
Study editor mentions, chat mentions, and agent file-reference syntax.

Before implementation, state which existing TermRock types and call sites will be preserved, migrated, split, or deleted. Produce concrete Rust API sketches, implement the component, migrate its usages, add Studio stories and the appropriate visual, semantic, interaction, PTY, fuzz, and performance tests, then perform an independent second-pass design review.
```

## 132. SlashCommandMenu — P1

```text
Using the TermRock Global implementation contract, redesign and implement: SlashCommandMenu.

Component mission:
Create a command completion surface optimized for prompt composers.

Component-specific requirements:
- Support command name, aliases, description, arguments, shortcut, provider/plugin source, disabled reason, recent commands, and nested argument completion.
- Integrate the global command system while allowing composer-specific commands.
- Provide fuzzy ranges, async plugin commands, loading, empty, and error states.
- Preserve draft text and replace only the intended token range.
- Support compact and fullscreen modes.

Research direction:
Study Grok Build, OpenCode, Claude Code, terminal shells, and command palettes.

Before implementation, state which existing TermRock types and call sites will be preserved, migrated, split, or deleted. Produce concrete Rust API sketches, implement the component, migrate its usages, add Studio stories and the appropriate visual, semantic, interaction, PTY, fuzz, and performance tests, then perform an independent second-pass design review.
```

## 133. ModelSelector and AgentModeSelector — P1

```text
Using the TermRock Global implementation contract, redesign and implement: ModelSelector and AgentModeSelector.

Component mission:
Create compact selectors for model, reasoning effort, agent mode, and execution policy.

Component-specific requirements:
- Support current choice, provider, capabilities, cost/latency/context metadata, availability, warnings, and recent choices.
- Separate model selection from mode selection but allow composed presentation.
- Contract to concise status text in the composer and expand into searchable selection.
- Show consequential changes clearly, especially permissions or cost.
- Keep provider data application-owned.

Research direction:
Study Amp, OpenCode, Grok Build, and model pickers in AI tools.

Before implementation, state which existing TermRock types and call sites will be preserved, migrated, split, or deleted. Produce concrete Rust API sketches, implement the component, migrate its usages, add Studio stories and the appropriate visual, semantic, interaction, PTY, fuzz, and performance tests, then perform an independent second-pass design review.
```

## 134. MessageThread — P1

```text
Using the TermRock Global implementation contract, redesign and implement: MessageThread.

Component mission:
Create a virtualized conversation and activity transcript for long-running agent sessions.

Component-specific requirements:
- Support user, assistant, system, tool, status, compact event, and error entries; stable anchors; grouping; timestamps; actors; actions; selection; copy; and search.
- Preserve reading position while streaming; show new-content indicator when not following tail.
- Support collapsed tool/activity entries and semantic zoom.
- Virtualize very long sessions and compact old content without losing checkpoints.
- Avoid chat-bubble web imitation.

Research direction:
Study Amp, OpenCode, Grok Build, Claude Code, and editorial Markdown TUIs.

Before implementation, state which existing TermRock types and call sites will be preserved, migrated, split, or deleted. Produce concrete Rust API sketches, implement the component, migrate its usages, add Studio stories and the appropriate visual, semantic, interaction, PTY, fuzz, and performance tests, then perform an independent second-pass design review.
```

## 135. StreamingMarkdown — P0

```text
Using the TermRock Global implementation contract, redesign and implement: StreamingMarkdown.

Component mission:
Create a streaming-safe Markdown renderer optimized for token-by-token AI output.

Component-specific requirements:
- Handle unfinished paragraphs, lists, tables, links, and code fences without flicker or complete reparse when avoidable.
- Preserve scroll anchors and text selection while content grows.
- Support citations/source anchors, partial syntax highlighting, and tool/status insertions.
- Batch updates to balance latency and redraw cost.
- Add adversarial streaming fixtures and performance budgets.

Research direction:
Study Glow-quality rendering combined with real streaming behavior in leading agent CLIs.

Before implementation, state which existing TermRock types and call sites will be preserved, migrated, split, or deleted. Produce concrete Rust API sketches, implement the component, migrate its usages, add Studio stories and the appropriate visual, semantic, interaction, PTY, fuzz, and performance tests, then perform an independent second-pass design review.
```

## 136. SourceCitation and CitationList — P2

```text
Using the TermRock Global implementation contract, redesign and implement: SourceCitation and CitationList.

Component mission:
Create compact inline citations and expandable source lists for agent output.

Component-specific requirements:
- Support source title, type, path/URL, range, confidence/provenance, open, preview, copy, unavailable state, and duplicate grouping.
- Keep raw destinations visible for external or sensitive sources.
- Integrate with Markdown anchors and fullscreen previews.
- Provide keyboard navigation without fragmenting the reading flow.
- Support no-hyperlink and offline states.

Research direction:
Study research assistants, IDE references, and terminal hyperlink capabilities.

Before implementation, state which existing TermRock types and call sites will be preserved, migrated, split, or deleted. Produce concrete Rust API sketches, implement the component, migrate its usages, add Studio stories and the appropriate visual, semantic, interaction, PTY, fuzz, and performance tests, then perform an independent second-pass design review.
```

## 137. ToolCallCard — P0

```text
Using the TermRock Global implementation contract, redesign and implement: ToolCallCard.

Component mission:
Create a reusable compact-to-expanded representation of agent tool execution.

Component-specific requirements:
- Support queued, preparing, running, waiting for input, waiting for permission, success, warning, failure, cancelled, and detached states.
- Show tool name, meaningful verb, actor/provenance, arguments summary, duration, result summary, risk, and actions.
- Allow inline expansion and fullscreen semantic zoom.
- Redact secrets and make data egress explicit.
- Never couple to a specific agent provider or tool protocol.

Research direction:
Study Grok Build, Amp, OpenCode, and Claude Code tool presentations.

Before implementation, state which existing TermRock types and call sites will be preserved, migrated, split, or deleted. Produce concrete Rust API sketches, implement the component, migrate its usages, add Studio stories and the appropriate visual, semantic, interaction, PTY, fuzz, and performance tests, then perform an independent second-pass design review.
```

## 138. TerminalRunCard — P0

```text
Using the TermRock Global implementation contract, redesign and implement: TerminalRunCard.

Component mission:
Specialize ToolCallCard for shell/terminal commands and live output.

Component-specific requirements:
- Show exact command, cwd, environment/redaction summary, provenance, status, elapsed time, stdout/stderr, exit code/signal, and actions.
- Support stop, detach, retry, copy, open fullscreen, and permission boundary.
- Preserve user scroll while output streams.
- Clearly distinguish proposed command from executed command and edited approval.
- Use safe ANSI parsing.

Research direction:
Study agent CLIs, CI command cards, and terminal output panes.

Before implementation, state which existing TermRock types and call sites will be preserved, migrated, split, or deleted. Produce concrete Rust API sketches, implement the component, migrate its usages, add Studio stories and the appropriate visual, semantic, interaction, PTY, fuzz, and performance tests, then perform an independent second-pass design review.
```

## 139. ActivityShelf — P1

```text
Using the TermRock Global implementation contract, redesign and implement: ActivityShelf.

Component mission:
Create a compact persistent summary of currently active or blocked operations.

Component-specific requirements:
- Support multiple concurrent tasks, statuses, elapsed time, actor, progress, waiting reason, and jump/open actions.
- Prioritize blocked and user-action-required items.
- Contract to one-line summary or badge in narrow layouts.
- Do not duplicate the full TaskRail model.
- Integrate with StatusBar and notifications.

Research direction:
Study agent activity indicators, build queues, and IDE background task surfaces.

Before implementation, state which existing TermRock types and call sites will be preserved, migrated, split, or deleted. Produce concrete Rust API sketches, implement the component, migrate its usages, add Studio stories and the appropriate visual, semantic, interaction, PTY, fuzz, and performance tests, then perform an independent second-pass design review.
```

## 140. TaskRail — P0

```text
Using the TermRock Global implementation contract, redesign and implement: TaskRail.

Component mission:
Create a unified task and agent activity side panel.

Component-specific requirements:
- Group workflows, subagents, foreground tasks, background tasks, watchers, and completed history.
- Support collapse, filter, search, selection, semantic zoom, status, elapsed time, progress, dependencies, and contextual actions.
- Prioritize requests requiring user input.
- Collapse into Drawer or StatusBar summary responsively.
- Use an application-neutral ActivityModel.

Research direction:
Study Grok Build tasks pane, Amp sessions, OpenCode agents, CI task lists, and Zellij panes.

Before implementation, state which existing TermRock types and call sites will be preserved, migrated, split, or deleted. Produce concrete Rust API sketches, implement the component, migrate its usages, add Studio stories and the appropriate visual, semantic, interaction, PTY, fuzz, and performance tests, then perform an independent second-pass design review.
```

## 141. SubagentCard — P1

```text
Using the TermRock Global implementation contract, redesign and implement: SubagentCard.

Component mission:
Create a reusable representation of delegated agent work.

Component-specific requirements:
- Show role, task, parent/provenance, status, model/mode, context, elapsed time, progress, latest summary, output preview, and actions.
- Support steer, message, inspect, cancel, retry, detach, and promote result outcomes without implementing agent control itself.
- Distinguish live work from completed artifact/result.
- Provide compact row, card, and fullscreen view.
- Make nested delegation understandable.

Research direction:
Study multi-agent products, Grok Build subagents, and task orchestration dashboards.

Before implementation, state which existing TermRock types and call sites will be preserved, migrated, split, or deleted. Produce concrete Rust API sketches, implement the component, migrate its usages, add Studio stories and the appropriate visual, semantic, interaction, PTY, fuzz, and performance tests, then perform an independent second-pass design review.
```

## 142. BackgroundTaskPanel — P1

```text
Using the TermRock Global implementation contract, redesign and implement: BackgroundTaskPanel.

Component mission:
Create persistent monitoring and control for detached commands, watchers, servers, and long jobs.

Component-specific requirements:
- Support live output, status, restart count, ports/resources, elapsed time, follow/pause, stop, restart, detach, open, and notifications.
- Handle reconnect and lost-process states.
- Preserve output history with bounded storage and dropped-line indicators.
- Provide compact rail row and full pane.
- Keep process control application-owned.

Research direction:
Study IDE task terminals, process supervisors, Grok Build watchers, and Zellij sessions.

Before implementation, state which existing TermRock types and call sites will be preserved, migrated, split, or deleted. Produce concrete Rust API sketches, implement the component, migrate its usages, add Studio stories and the appropriate visual, semantic, interaction, PTY, fuzz, and performance tests, then perform an independent second-pass design review.
```

## 143. ContextMeter — P1

```text
Using the TermRock Global implementation contract, redesign and implement: ContextMeter.

Component mission:
Create a trustworthy context/token/resource budget display.

Component-specific requirements:
- Support used/available, compaction threshold, model limit, included sources, cached content, pending attachments, warning, and expandable breakdown.
- Avoid false precision when estimates are approximate.
- Provide concise composer/status form and detailed popover.
- Show what action will reduce usage or trigger compaction.
- Support non-token resource budgets through generic measurement types.

Research direction:
Study Amp compaction, OpenCode context displays, and AI chat token meters.

Before implementation, state which existing TermRock types and call sites will be preserved, migrated, split, or deleted. Produce concrete Rust API sketches, implement the component, migrate its usages, add Studio stories and the appropriate visual, semantic, interaction, PTY, fuzz, and performance tests, then perform an independent second-pass design review.
```

## 144. PermissionPrompt — P0

```text
Using the TermRock Global implementation contract, redesign and implement: PermissionPrompt.

Component mission:
Build a signature trust surface substantially better than a generic Allow dialog.

Component-specific requirements:
- Show who initiated the action, provenance chain, exact operation, target, execution location, accessed/transmitted data, destination, expected result, reversibility, risk, and requested scope.
- Support Allow once/session/project/always, deny, edit command/pattern, restrict scope, inspect details, and request changes.
- Queue concurrent requests and protect against stale responses.
- Use safe initial focus and explicit destructive/data-egress language.
- Add exhaustive security and nested-subagent tests.

Research direction:
Study Grok Build permissions, Amp plugin prompts, browser permissions, sudo, and security review UIs.

Before implementation, state which existing TermRock types and call sites will be preserved, migrated, split, or deleted. Produce concrete Rust API sketches, implement the component, migrate its usages, add Studio stories and the appropriate visual, semantic, interaction, PTY, fuzz, and performance tests, then perform an independent second-pass design review.
```

## 145. QuestionFlow — P0

```text
Using the TermRock Global implementation contract, redesign and implement: QuestionFlow.

Component mission:
Create multi-question human-in-the-loop interaction for agents and workflows.

Component-specific requirements:
- Support single choice, multiple choice, free text, other option, validation, optional questions, multiple questions as tabs/steps, per-question scroll/cursor state, and review before submit.
- Preserve the underlying composer draft.
- Support queued questions and originating actor/provenance.
- Promote to fullscreen when content is complex.
- Return structured answers without embedding workflow policy.

Research direction:
Study Grok Build question view, form wizards, and conversational agent prompts.

Before implementation, state which existing TermRock types and call sites will be preserved, migrated, split, or deleted. Produce concrete Rust API sketches, implement the component, migrate its usages, add Studio stories and the appropriate visual, semantic, interaction, PTY, fuzz, and performance tests, then perform an independent second-pass design review.
```

## 146. PlanReview — P0

```text
Using the TermRock Global implementation contract, redesign and implement: PlanReview.

Component mission:
Create an interactive review surface for agent-generated plans.

Component-specific requirements:
- Render Markdown plan, sections, source references, tasks, risks, assumptions, and affected files.
- Support line/range comments, selection, approve, approve with conditions, request revision, edit feedback, and abandon.
- Preserve comments through plan updates where stable anchors permit.
- Show version changes between revisions.
- Use safe focus and explicit consequences.

Research direction:
Study Grok Build plan approval, code review workflows, and document annotation UIs.

Before implementation, state which existing TermRock types and call sites will be preserved, migrated, split, or deleted. Produce concrete Rust API sketches, implement the component, migrate its usages, add Studio stories and the appropriate visual, semantic, interaction, PTY, fuzz, and performance tests, then perform an independent second-pass design review.
```

## 147. CheckpointTimeline — P1

```text
Using the TermRock Global implementation contract, redesign and implement: CheckpointTimeline.

Component mission:
Create a rewindable session history for agent turns, file states, and significant actions.

Component-specific requirements:
- Support checkpoints, labels, actor, timestamp, summary, changed files, tool calls, irreversible boundaries, branch/fork, preview, rewind, and restore outcomes.
- Differentiate viewing history from mutating/rewinding state.
- Warn when local uncommitted work or external side effects cannot be restored.
- Preserve current draft and allow comparison.
- Use Timeline as a base where appropriate.

Research direction:
Study Grok Build rewind, IDE local history, Git reflog, and notebook checkpoints.

Before implementation, state which existing TermRock types and call sites will be preserved, migrated, split, or deleted. Produce concrete Rust API sketches, implement the component, migrate its usages, add Studio stories and the appropriate visual, semantic, interaction, PTY, fuzz, and performance tests, then perform an independent second-pass design review.
```

## 148. SessionPicker — P1

```text
Using the TermRock Global implementation contract, redesign and implement: SessionPicker.

Component mission:
Create a polished selector for creating, resuming, searching, renaming, archiving, and deleting agent sessions.

Component-specific requirements:
- Support project, branch, status, recency, model/mode, summary, unread/action-required state, pinning, search, and preview.
- Preserve current draft and app context when cancelled.
- Provide safe delete/archive confirmation and multi-device/remote status.
- Handle thousands of sessions through virtualization or provider search.
- Offer popover and fullscreen forms.

Research direction:
Study Amp sessions, OpenCode sessions, Grok Build picker, and project launchers.

Before implementation, state which existing TermRock types and call sites will be preserved, migrated, split, or deleted. Produce concrete Rust API sketches, implement the component, migrate its usages, add Studio stories and the appropriate visual, semantic, interaction, PTY, fuzz, and performance tests, then perform an independent second-pass design review.
```

## 149. PromptQueue — P1

```text
Using the TermRock Global implementation contract, redesign and implement: PromptQueue.

Component mission:
Create a visible, editable queue of user messages waiting behind active agent work.

Component-specific requirements:
- Support queued, sending, blocked, failed, cancelled, and sent states; reorder, edit, delete, send next, and interrupt-and-send.
- Make execution semantics clear when the agent is busy.
- Preserve attachment and mention identities.
- Provide compact composer summary and expanded management view.
- Keep queue persistence/application policy outside the component.

Research direction:
Study asynchronous chat products, agent prompt queues, and task schedulers.

Before implementation, state which existing TermRock types and call sites will be preserved, migrated, split, or deleted. Produce concrete Rust API sketches, implement the component, migrate its usages, add Studio stories and the appropriate visual, semantic, interaction, PTY, fuzz, and performance tests, then perform an independent second-pass design review.
```

## 150. AgentStatusHeader — P1

```text
Using the TermRock Global implementation contract, redesign and implement: AgentStatusHeader.

Component mission:
Create a compact top-level status surface for the current agent/session.

Component-specific requirements:
- Show project/session, branch, agent/mode/model, connection, working/waiting status, context usage, cost/time where provided, and action-required state.
- Prioritize actionable state over decorative metadata.
- Contract into StatusBar on narrow layouts.
- Provide quick actions for sessions, model, tasks, and help.
- Avoid provider-specific assumptions.

Research direction:
Study Grok Build headers, OpenCode, Amp, and IDE workspace headers.

Before implementation, state which existing TermRock types and call sites will be preserved, migrated, split, or deleted. Produce concrete Rust API sketches, implement the component, migrate its usages, add Studio stories and the appropriate visual, semantic, interaction, PTY, fuzz, and performance tests, then perform an independent second-pass design review.
```

## 151. IntegrationStatus — P2

```text
Using the TermRock Global implementation contract, redesign and implement: IntegrationStatus.

Component mission:
Create status and management surfaces for MCP servers, plugins, extensions, tools, and external integrations.

Component-specific requirements:
- Support connected, disconnected, starting, error, permission required, update available, disabled, and degraded states.
- Show source, capabilities, permissions, last error, logs, restart, enable/disable, and details outcomes.
- Separate compact status badges from full settings/diagnostic panels.
- Make third-party provenance explicit.
- Use safe permission and data-egress language.

Research direction:
Study Grok Build extension/MCP views, editor extension managers, and service health panels.

Before implementation, state which existing TermRock types and call sites will be preserved, migrated, split, or deleted. Produce concrete Rust API sketches, implement the component, migrate its usages, add Studio stories and the appropriate visual, semantic, interaction, PTY, fuzz, and performance tests, then perform an independent second-pass design review.
```

## 152. WorkingStateCard — P1

```text
Using the TermRock Global implementation contract, redesign and implement: WorkingStateCard.

Component mission:
Create a transparent but non-invasive summary of what the agent is doing now.

Component-specific requirements:
- Show current phase, concise rationale summary supplied by the application, relevant files/resources, elapsed time, next expected action, and cancellation/inspect controls.
- Do not expose or imply hidden private chain-of-thought.
- Differentiate planning, searching, editing, running, waiting, and reviewing.
- Collapse into ActivityShelf when not expanded.
- Support screen-reader/semantic descriptions and no-color state.

Research direction:
Study agent status surfaces while respecting privacy-preserving reasoning summaries.

Before implementation, state which existing TermRock types and call sites will be preserved, migrated, split, or deleted. Produce concrete Rust API sketches, implement the component, migrate its usages, add Studio stories and the appropriate visual, semantic, interaction, PTY, fuzz, and performance tests, then perform an independent second-pass design review.
```

## 153. ApprovalQueue — P2

```text
Using the TermRock Global implementation contract, redesign and implement: ApprovalQueue.

Component mission:
Create a unified surface for pending permissions, questions, plans, diffs, and other human decisions.

Component-specific requirements:
- Support priority, type, source actor, age, blocking status, summary, preview, open, approve where safe, defer, and dismiss/cancel outcomes.
- Never reduce high-risk approvals to one-click bulk actions.
- Preserve request order where protocol requires it.
- Provide compact status badge, drawer, and full view.
- Integrate with NotificationCenter and TaskRail.

Research direction:
Study agent approval flows, code review queues, and security request dashboards.

Before implementation, state which existing TermRock types and call sites will be preserved, migrated, split, or deleted. Produce concrete Rust API sketches, implement the component, migrate its usages, add Studio stories and the appropriate visual, semantic, interaction, PTY, fuzz, and performance tests, then perform an independent second-pass design review.
```

## 154. AgentWorkbench block — P0

```text
Using the TermRock Global implementation contract, redesign and implement: AgentWorkbench block.

Component mission:
Build the north-star full application block using only public TermRock APIs and registry components.

Component-specific requirements:
- Compose AppShell, MessageThread, PromptComposer, TaskRail, ActivityShelf, ToolCallCard, TerminalRunCard, PermissionPrompt, QuestionFlow, PlanReview, DiffReview, SessionPicker, and command surfaces.
- Define focus order, semantic zoom, responsive layouts, streaming behavior, draft preservation, and one-layer Escape semantics.
- Provide normal, tool-running, permission, plan, diff, multi-agent, narrow, tiny, ASCII, and no-color stories.
- Treat every workaround as a missing framework primitive to fix.
- Ship as a source-owned registry block, not a monolithic framework requirement.

Research direction:
Use Grok Build, Amp, OpenCode, Claude Code, Posting, Zellij, and Glow as experience references without cloning any one product.

Before implementation, state which existing TermRock types and call sites will be preserved, migrated, split, or deleted. Produce concrete Rust API sketches, implement the component, migrate its usages, add Studio stories and the appropriate visual, semantic, interaction, PTY, fuzz, and performance tests, then perform an independent second-pass design review.
```


# I. High-value application blocks

Blocks prove that the component system can solve real product workflows. They should be source-owned compositions, not permanently frozen framework abstractions.

## 155. SettingsScreen block — P1

```text
Using the TermRock Global implementation contract, redesign and implement: SettingsScreen block.

Component mission:
Create a complete searchable settings experience.

Component-specific requirements:
- Compose Sidebar, SearchInput, Sections, Fields, controls, validation, reset-to-default, modified indicators, conflicts, and restart-required state.
- Support categories, deep links, keyboard help, responsive drawer navigation, and no-results guidance.
- Integrate KeybindingRecorder and theme preview.
- Keep persistence and restart policy application-owned.
- Use only public registry components.

Research direction:
Study Zellij configuration UIs, btop options, editor settings, and shadcn settings layouts.

Before implementation, state which existing TermRock types and call sites will be preserved, migrated, split, or deleted. Produce concrete Rust API sketches, implement the component, migrate its usages, add Studio stories and the appropriate visual, semantic, interaction, PTY, fuzz, and performance tests, then perform an independent second-pass design review.
```

## 156. Onboarding and SetupWizard block — P1

```text
Using the TermRock Global implementation contract, redesign and implement: Onboarding and SetupWizard block.

Component mission:
Create a premium first-run flow for terminal applications.

Component-specific requirements:
- Support welcome, capability check, account/connection setup, choices, validation, permissions, theme preview, summary, and recovery.
- Use FormWizard and Stepper while allowing keyboard-only completion.
- Provide resume and safe cancellation.
- Adapt to inline and fullscreen modes.
- Avoid marketing-heavy screens that waste terminal space.

Research direction:
Study CLI installers, Huh, cloud authentication flows, and polished native onboarding.

Before implementation, state which existing TermRock types and call sites will be preserved, migrated, split, or deleted. Produce concrete Rust API sketches, implement the component, migrate its usages, add Studio stories and the appropriate visual, semantic, interaction, PTY, fuzz, and performance tests, then perform an independent second-pass design review.
```

## 157. ConnectionManager block — P2

```text
Using the TermRock Global implementation contract, redesign and implement: ConnectionManager block.

Component mission:
Create reusable management for database, SSH, API, and service connections.

Component-specific requirements:
- Support connection list, status, search, groups, add/edit/test, credentials/redaction, recent, favorites, errors, reconnect, and delete.
- Use safe secret fields and clear target/environment identity.
- Keep protocol and persistence application-provided.
- Provide compact launcher and full management views.
- Integrate with OfflineState and diagnostics.

Research direction:
Study TablePlus, SSH managers, cloud CLIs, and service dashboards.

Before implementation, state which existing TermRock types and call sites will be preserved, migrated, split, or deleted. Produce concrete Rust API sketches, implement the component, migrate its usages, add Studio stories and the appropriate visual, semantic, interaction, PTY, fuzz, and performance tests, then perform an independent second-pass design review.
```

## 158. DatabaseWorkbench block — P2

```text
Using the TermRock Global implementation contract, redesign and implement: DatabaseWorkbench block.

Component mission:
Create a source-owned database application composition demonstrating TermRock's data components.

Component-specific requirements:
- Compose connection manager, schema browser, query tabs/editor, result grid, object inspector, history, status bar, command palette, and export/copy actions.
- Define focus zones and responsive collapse.
- Support query running/cancel, errors, transaction status, and disconnected state through typed application messages.
- Provide realistic mock data stories and large-result benchmarks.
- Treat visual quality as a flagship use case.

Research direction:
Study TablePlus, DataGrip, pgcli/lazysql-style tools, and high-quality developer workbenches.

Before implementation, state which existing TermRock types and call sites will be preserved, migrated, split, or deleted. Produce concrete Rust API sketches, implement the component, migrate its usages, add Studio stories and the appropriate visual, semantic, interaction, PTY, fuzz, and performance tests, then perform an independent second-pass design review.
```

## 159. GitWorkbench block — P2

```text
Using the TermRock Global implementation contract, redesign and implement: GitWorkbench block.

Component mission:
Create a modern source-owned Git workflow block.

Component-specific requirements:
- Compose repository status, file tree/list, diff review, history timeline, branches, commits, command output, conflict diagnostics, and contextual actions.
- Support line/hunk/file selection, staging-like typed outcomes, safe destructive flows, and keyboard help.
- Provide responsive layouts and fullscreen diff promotion.
- Keep all Git execution outside components.
- Use lazygit and GitUI as interaction references while developing a distinct design.

Research direction:
Study lazygit, GitUI, delta, and IDE source-control panels.

Before implementation, state which existing TermRock types and call sites will be preserved, migrated, split, or deleted. Produce concrete Rust API sketches, implement the component, migrate its usages, add Studio stories and the appropriate visual, semantic, interaction, PTY, fuzz, and performance tests, then perform an independent second-pass design review.
```

## 160. Logs and ObservabilityDashboard block — P2

```text
Using the TermRock Global implementation contract, redesign and implement: Logs and ObservabilityDashboard block.

Component mission:
Create a complete operational monitoring composition.

Component-specific requirements:
- Compose filters, time range, LogStream, EventStream, MetricsDashboard, status summary, details inspector, alerts, and query/search.
- Support live/pause, reconnect, dropped-data warning, bookmarks, and drill-down.
- Define keyboard focus zones and responsive pane collapse.
- Provide bursty mock streams and failure stories.
- Keep data acquisition application-owned.

Research direction:
Study k9s, btop, Grafana concepts, and terminal log tools.

Before implementation, state which existing TermRock types and call sites will be preserved, migrated, split, or deleted. Produce concrete Rust API sketches, implement the component, migrate its usages, add Studio stories and the appropriate visual, semantic, interaction, PTY, fuzz, and performance tests, then perform an independent second-pass design review.
```

## 161. FileManager block — P2

```text
Using the TermRock Global implementation contract, redesign and implement: FileManager block.

Component mission:
Create a source-owned file-management composition.

Component-specific requirements:
- Compose breadcrumbs, file tree/list, preview, quick open, search, selection, operation queue, status bar, and dialogs.
- Support copy/move/delete/rename/new typed requests, progress, conflicts, and safe recovery.
- Provide mouse and keyboard interaction without hidden hover-only controls.
- Adapt from multi-pane to single-pane/drawer layouts.
- Use Yazi as a performance and interaction reference, not a clone target.

Research direction:
Study Yazi, ranger, lf, broot, and desktop file managers.

Before implementation, state which existing TermRock types and call sites will be preserved, migrated, split, or deleted. Produce concrete Rust API sketches, implement the component, migrate its usages, add Studio stories and the appropriate visual, semantic, interaction, PTY, fuzz, and performance tests, then perform an independent second-pass design review.
```

## 162. ProjectLauncher block — P2

```text
Using the TermRock Global implementation contract, redesign and implement: ProjectLauncher block.

Component mission:
Create a fast project/session launcher for developer tools.

Component-specific requirements:
- Support recent projects, favorites, workspaces, branches, sessions, remote/local status, search, grouping, preview, open/new/import, and errors.
- Integrate QuickOpen, ConnectionStatus, SessionPicker, and onboarding.
- Handle large histories and stale/missing locations.
- Provide inline quick launcher and full-screen home variants.
- Keep discovery and persistence application-owned.

Research direction:
Study IDE welcome screens, zoxide/fzf workflows, and agent session launchers.

Before implementation, state which existing TermRock types and call sites will be preserved, migrated, split, or deleted. Produce concrete Rust API sketches, implement the component, migrate its usages, add Studio stories and the appropriate visual, semantic, interaction, PTY, fuzz, and performance tests, then perform an independent second-pass design review.
```

## 163. HelpCenter and CommandReference block — P2

```text
Using the TermRock Global implementation contract, redesign and implement: HelpCenter and CommandReference block.

Component mission:
Create contextual product help generated from real component and command metadata.

Component-specific requirements:
- Support search, navigation, keyboard map, commands, tutorials, current-context help, troubleshooting, capability diagnostics, and links.
- Render with Markdown and semantic anchors.
- Avoid stale duplicated shortcut documentation.
- Provide compact help overlay and full documentation view.
- Integrate `termrock doctor` and component inspection.

Research direction:
Study Vim/Helix help, Zellij key help, CLI man pages, and command palettes.

Before implementation, state which existing TermRock types and call sites will be preserved, migrated, split, or deleted. Produce concrete Rust API sketches, implement the component, migrate its usages, add Studio stories and the appropriate visual, semantic, interaction, PTY, fuzz, and performance tests, then perform an independent second-pass design review.
```

## 164. ErrorRecovery and CrashReport block — P2

```text
Using the TermRock Global implementation contract, redesign and implement: ErrorRecovery and CrashReport block.

Component mission:
Create a graceful recovery experience for serious failures.

Component-specific requirements:
- Support human summary, preserved work, recovery options, restart/restore session, copy diagnostics, logs, environment/capabilities, report issue, and safe quit.
- Redact secrets from reports and snapshots.
- Handle terminal restoration failures and partial initialization.
- Provide inline fallback when full-screen rendering is compromised.
- Test through PTY fault injection.

Research direction:
Study crash reporters, terminal panic hooks, session restoration, and resilient CLI design.

Before implementation, state which existing TermRock types and call sites will be preserved, migrated, split, or deleted. Produce concrete Rust API sketches, implement the component, migrate its usages, add Studio stories and the appropriate visual, semantic, interaction, PTY, fuzz, and performance tests, then perform an independent second-pass design review.
```


---

# Recommended build order

The library contains **164 component and block prompts**. Do not implement them in numerical order without regard to dependencies.

A strong first sequence is:

1. DesignSystem and component recipes.
2. UiContext, SemanticScene, UiIntent, EventResult, and FocusGraph.
3. CollectionState, SelectionModel, ScrollArea, Virtualizer, and OverlayStack.
4. Responsive and capability systems.
5. Text, Surface, Stack/Inline, Button, Field/Form, TextInput, List, Dialog, and CommandPalette.
6. Studio inspection and registry metadata around those components.
7. DataTable, Tree, LogStream, DiffView, and TerminalOutput.
8. PromptComposer, StreamingMarkdown, ToolCallCard, PermissionPrompt, QuestionFlow, PlanReview, and TaskRail.
9. AgentWorkbench and the other high-value application blocks.

# Per-component review loop

```text
Design specification
    → implementation
    → realistic Studio stories
    → buffer + semantic snapshots
    → interaction and PTY tests
    → performance check
    → independent design review
    → fix every blocking issue and every quality score below 9/10
    → registry publication
```
