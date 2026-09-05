/goal You are leading a long-running implementation campaign to completely redesign and rewrite TermRock using `terminal-components-claude` as the canonical visual and behavioral reference.

## Mission

Transform:

https://github.com/tailrocks/termrock

so that its entire TUI design system, components, widgets, examples, interactions, visual language, and behavior match:

https://github.com/donbeave/terminal-components-claude

as closely to **one-to-one identical** as technically possible.

Canonical design specification:

https://raw.githubusercontent.com/donbeave/terminal-components-claude/refs/heads/main/DESIGN.md

Canonical visual references:

https://github.com/donbeave/terminal-components-claude/tree/main/shots

The reference project is the source of truth.

Do **not** redesign, reinterpret, simplify, modernize, "improve", or introduce an alternative visual direction where the reference already defines an answer.

When the reference defines something, copy its design and behavior faithfully.

"Similar", "inspired by", "close enough", and "same general style" are failures.

The target is one-to-one visual and behavioral fidelity.

---

## Execution Model

Use subagents for everything.

Use `Grok 4.6` with `xhigh` reasoning for every subagent and every task where model selection is available.

The primary agent is an orchestrator, coordinator, integrator, and final verifier.

Delegate investigation, design analysis, implementation, testing, visual comparison, code review, architecture review, documentation, and verification to fresh subagents.

Spawn as many parallel subagents as useful when their scopes do not conflict.

Prefer many small, independently verifiable workstreams over one large sequential task.

Never allow the primary agent to become the implementation bottleneck when work can be delegated.

For important design decisions, use multiple independent agents and compare their conclusions before committing to an interpretation.

For important completed implementations, use fresh independent verifier agents that did not implement the work.

Do not allow an implementation agent to be its own final reviewer.

---

## Never Block

Never stop merely because something is difficult, ambiguous, undocumented, broken, missing, or unexpected.

Never wait indefinitely for human input.

If blocked:

1. spawn subagents to investigate the blocker independently;
2. search the repositories, history, tests, screenshots, examples, documentation, and implementation for evidence;
3. generate multiple possible solutions;
4. have independent agents compare the alternatives;
5. select the solution best supported by the reference implementation and design system;
6. continue execution.

Escalate to the human only when proceeding would require an irreversible decision for which no defensible answer can be derived from the repositories or project requirements.

A blocker is a reason to increase investigation and parallelism, not a reason to stop.

---

# Source of Truth

Treat sources in this priority order:

1. actual rendered behavior of `terminal-components-claude`;
2. screenshots in `terminal-components-claude/shots`;
3. actual source implementation of `terminal-components-claude`;
4. `terminal-components-claude/DESIGN.md`;
5. repeated design patterns inferred from the reference project;
6. only then, professional TUI design judgment.

When sources appear inconsistent, investigate the actual rendered implementation and determine the intended behavior using multiple independent agents.

Do not preserve an existing TermRock behavior merely because it already exists if it conflicts with the canonical reference.

---

# Phase 1 — Exhaustive Reference Analysis

Before substantial implementation, create a complete inventory of `terminal-components-claude`.

Use parallel subagents to independently analyze:

* global visual language;
* layout system;
* spacing;
* padding;
* margins;
* alignment;
* borders;
* separators;
* corners;
* shadows;
* depth;
* colors;
* semantic color roles;
* typography assumptions;
* text hierarchy;
* iconography;
* Phosphor usage;
* focus states;
* hover states where supported;
* selected states;
* disabled states;
* pressed/active states;
* keyboard interaction;
* navigation;
* mouse interaction;
* scrolling;
* empty states;
* loading states;
* errors;
* warnings;
* destructive actions;
* overlays;
* dialogs;
* menus;
* command palettes;
* forms;
* inputs;
* tables;
* lists;
* trees;
* tabs;
* panes;
* headers;
* footers;
* status information;
* help text;
* shortcuts;
* animation or transition-like behavior achievable in a terminal;
* responsive behavior under different terminal sizes.

Inventory every reusable component, widget, pattern, primitive, composite component, and example.

Do not start from assumptions about what the design system is.

Derive it from evidence.

---

# Phase 2 — TermRock Gap Analysis

Independently inventory everything currently present in TermRock.

For every TermRock component, widget, primitive, example application, and interaction, classify it as one of:

* direct reference equivalent exists;
* partial reference equivalent exists;
* reference pattern can be composed;
* no direct reference exists and must be derived from the design system;
* obsolete/inconsistent implementation that should be replaced.

Build a complete reference-to-TermRock mapping.

For each item determine:

* current TermRock implementation;
* canonical reference;
* visual differences;
* behavioral differences;
* API implications;
* reusable primitive dependencies;
* required implementation changes;
* required tests;
* required visual verification.

Do not silently omit components.

The campaign is complete only when the full TermRock surface has been reviewed.

---

# Phase 3 — Extract the Canonical Design System

Convert the reference implementation into explicit reusable design primitives inside TermRock.

Avoid scattering copied magic constants throughout widgets.

Extract reusable semantic concepts where technically appropriate, including:

* palette and semantic colors;
* spacing scale;
* borders;
* surfaces;
* text styles;
* focus styling;
* selected styling;
* disabled styling;
* interactive-state styling;
* icon conventions;
* layout primitives;
* overlays;
* panels;
* navigation patterns;
* table styles;
* input styles;
* status patterns;
* keyboard conventions.

The goal is not merely to make screenshots match once.

The goal is to make the reference design the native reusable design language of TermRock so future components naturally look correct.

Do not introduce abstractions that materially change the rendered result.

Fidelity comes before abstraction purity.

---

# Phase 4 — Reimplement Direct Matches One-to-One

Whenever `terminal-components-claude` contains a direct equivalent of a TermRock component, reproduce it faithfully.

Match all observable aspects including, where applicable:

* geometry;
* width;
* height;
* spacing;
* padding;
* alignment;
* border characters;
* foreground colors;
* background colors;
* emphasis;
* dimming;
* icons;
* labels;
* typography treatment;
* separators;
* shadows;
* active state;
* selected state;
* focus state;
* disabled state;
* hover behavior;
* keyboard behavior;
* mouse behavior;
* scrolling;
* state transitions;
* navigation;
* error presentation;
* help presentation;
* layout behavior;
* terminal resize behavior.

Do not approximate values when exact values can be extracted from the reference.

---

# Phase 5 — Components Without Direct References

Some TermRock components may not exist in `terminal-components-claude`.

Do not invent an unrelated design.

For each such component:

1. have multiple design-analysis agents independently study analogous reference components;
2. identify the relevant design rules from `DESIGN.md`;
3. identify repeated patterns in the reference implementation;
4. propose how the missing component should look and behave;
5. compare proposals;
6. obtain independent agreement on the interpretation;
7. implement the component using the established TermRock design primitives;
8. perform fresh visual/design verification afterward.

The result must look as though the missing component had originally been designed by the same designer who created `terminal-components-claude`.

---

# Architecture Requirements

TermRock is a reusable Rust TUI component library, not a screenshot recreation.

Preserve or improve the reusable library architecture necessary for components to work across multiple applications.

Prefer:

* reusable primitives;
* composable components;
* explicit state models;
* clear public APIs;
* predictable event handling;
* strong Rust types;
* low duplication;
* separation between design tokens and widget logic;
* separation between state, rendering, and interaction when appropriate;
* idiomatic Rust;
* idiomatic Ratatui patterns where applicable;
* testable behavior.

Do not create application-specific hacks simply to reproduce one screenshot.

However, do not sacrifice visible fidelity merely to preserve an existing abstraction.

When an existing architecture prevents correct implementation, refactor it.

This is a redesign/reimplementation campaign, not a compatibility-preservation exercise.

Avoid maintaining obsolete legacy paths unless they are genuinely required.

---

# Visual Verification Is Mandatory

Every significant visual implementation must be verified against the reference.

Use the material in:

`terminal-components-claude/shots`

as canonical comparison evidence.

Where possible, create equivalent TermRock rendering scenarios and produce deterministic screenshots/images for comparison.

Compare at minimum:

* overall composition;
* component geometry;
* spacing;
* alignment;
* text;
* icons;
* borders;
* backgrounds;
* foregrounds;
* state;
* visual hierarchy;
* selected/focused behavior.

Use programmatic image comparison where practical.

Do not rely only on an agent saying "this looks correct."

Use objective evidence whenever possible.

If the rendering environment introduces unavoidable differences, isolate those differences and verify everything under TermRock's control.

---

# Behavioral Verification Is Mandatory

Visual similarity alone is insufficient.

For every interactive component verify:

* focus traversal;
* keyboard commands;
* selection behavior;
* mouse behavior where supported;
* hover behavior where supported;
* scrolling;
* boundaries;
* disabled states;
* state transitions;
* escape/back behavior;
* confirmation behavior;
* navigation;
* resize behavior;
* invalid input;
* empty content;
* large content.

Where the reference exhibits behavior, that behavior is canonical.

---

# Continuous Independent Review

After each substantial workstream, spawn fresh verifier subagents.

Verifier responsibilities:

* inspect the reference independently;
* inspect the implementation;
* compare screenshots;
* compare behavior;
* identify discrepancies;
* identify missing states;
* identify API or architectural regressions;
* classify findings by severity.

Do not merely ask:

"Does this look good?"

Ask:

"What is different from the canonical reference?"

Treat every unexplained difference as a defect until proven otherwise.

For difficult components, use several independent reviewers.

Continue fixing and reverifying until meaningful discrepancies are eliminated.

---

# Regression Safety

Maintain strong deterministic verification throughout the campaign.

At appropriate checkpoints run all relevant:

* formatting;
* linting;
* compilation;
* unit tests;
* integration tests;
* interaction tests;
* snapshot tests;
* rendering tests;
* screenshot comparisons;
* examples;
* workspace checks.

Never knowingly leave the repository in a broken intermediate state for longer than necessary.

When shared foundations change, proactively identify all components affected and reverify them.

---

# Example Applications

Every TermRock example/demo application is part of the deliverable.

Examples must also use the canonical design system.

Do not leave old TermRock styling inside examples after components are redesigned.

Examples should demonstrate the new component APIs and canonical visual system consistently.

Use examples as visual regression fixtures whenever useful.

---

# Cleanup

Remove or replace stale design implementations that conflict with the canonical system.

After migrations:

* eliminate redundant styling;
* remove dead design constants;
* remove superseded implementations;
* remove obsolete compatibility code when safe;
* consolidate duplicated design primitives;
* update examples;
* update tests;
* update documentation.

Do not leave two competing TermRock design systems in the repository.

There should be one coherent canonical system.

---

# Documentation

Update TermRock documentation so future contributors understand that the implemented design system is deliberate and reusable.

Document:

* design primitives;
* component conventions;
* interaction conventions;
* extension rules;
* how new components should inherit the design system;
* visual verification practices.

Where appropriate, maintain a TermRock `DESIGN.md` that accurately captures the implemented design language derived from the canonical reference.

Do not replace implementation work with documentation.

Documentation must describe what was actually implemented.

---

# Completion Standard

Do not declare success because:

* the project builds;
* most components were migrated;
* screenshots look approximately similar;
* one reviewer approved;
* tests pass while visual differences remain.

Success requires all of the following:

* every existing TermRock component has been inventoried;
* every component has been mapped to the canonical reference or derived design rules;
* all direct equivalents have been reimplemented with one-to-one fidelity;
* unmatched components have been redesigned consistently with the same system;
* reusable design primitives are established;
* every example uses the new design;
* visual regression verification has been performed;
* behavioral verification has been performed;
* independent reviewers have inspected the result;
* discrepancies identified by reviewers have been resolved or proven unavoidable;
* formatting passes;
* linting passes;
* compilation passes;
* tests pass;
* examples run;
* no stale competing visual system remains;
* documentation reflects the resulting implementation.

---

# Final Acceptance Review

Before finishing, spawn fresh subagents that have not participated in implementation.

Assign independent roles such as:

* reference design auditor;
* pixel/visual comparison reviewer;
* interaction reviewer;
* Rust architecture reviewer;
* component API reviewer;
* completeness auditor;
* regression/test reviewer.

Ask each verifier to actively search for reasons the campaign should **not** be accepted.

Aggregate their findings.

Resolve actionable discrepancies.

Run verification again.

Repeat until no material unresolved discrepancies remain.

---

# Final Report

At completion provide a concise evidence-based report containing:

1. what was redesigned/reimplemented;
2. component-by-component coverage;
3. reusable design-system primitives created;
4. behavioral changes;
5. architecture changes;
6. tests added or updated;
7. visual comparisons performed;
8. independent verification performed;
9. commands/checks executed and their results;
10. any remaining differences from `terminal-components-claude`, with concrete technical reasons why they are unavoidable.

Do not claim exact parity without evidence.

If an observable difference remains under our control, continue working rather than documenting it as acceptable.

---

## Governing Principle

`terminal-components-claude` is not inspiration.

It is the canonical implementation and design reference.

For anything explicitly represented there:

**COPY, DO NOT INTERPRET.**

For anything not explicitly represented there:

**DERIVE, DO NOT INVENT.**

For every implementation:

**COMPARE, VERIFY, FIX, AND VERIFY AGAIN.**

Continue the campaign autonomously until TermRock behaves and renders as though both projects were created from the same design system, with one-to-one fidelity wherever a direct reference exists.

