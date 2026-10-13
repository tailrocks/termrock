# Baselines

## Purpose

The committed PNG baseline set: one PNG per lookbook story of the
jackin-used subset, phosphor theme only, plain git. This is the design
record the CI gate protects and the artifact reviewers diff in PRs.
Anchors: F5(set), B3, D4, D6, N2 · Evidence: research/tui-png-baselines/03-termrock-seams-and-old-rev.md, research/tui-png-baselines/04-determinism-ci-storage.md

## Requirements

### Requirement: Baseline set for the jackin-used subset
The repo SHALL commit, in plain git (N2: never LFS), one PNG per lookbook
story belonging to the jackin-used subset's 16 widget families, rendered by
`termrock-raster` with the phosphor `RolePalette` (D6) at the story's
registered geometry, under
`crates/termrock-lookbook/baselines/png/<story-id-with-dashes>.png`
(filename scheme mirroring the SVG exporter's `svg.rs:104`). Coverage is the
subset only (D4) — no catalog-wide baselines.
Covers: F5, D4, D6, N2 · Evidence: ch. 03 Q6 (87 subset stories at HEAD), ch. 04 §5 (plain-git size math)

#### Scenario: Every subset story has a baseline
- **GIVEN** the registered story list filtered to the 16 subset components
- **WHEN** the baseline directory is listed
- **THEN** every such story id has exactly one committed PNG and no non-subset story does

#### Scenario: Baseline is reproducible
- **GIVEN** any committed baseline PNG
- **WHEN** its story is re-rendered on the same commit
- **THEN** decoded pixels are identical to the committed file

### Requirement: All-states story gap fill
The lookbook SHALL gain stories closing the recorded state gaps for the
subset so B3's "all states" holds: focused and disabled variants for
TextInput, Tabs, Toast, StatusBar, and ActionBar (the components ch. 03 Q6
names as lacking them) wherever the widget actually models that state. Where
a widget does not model a state (no focus/disabled/hover API or distinct
paint — e.g. Toast is never focusable, `toast.rs:12-13`), the story-set note
SHALL record why instead — the honesty rule of the hover scenario applies to
every state. Each new story registers under the existing component id scheme
and thereby joins the baseline set automatically.
Covers: B3, F5 · Evidence: ch. 03 Q6 (gap list; no hover-variant story exists for any subset component)

#### Scenario: TextInput focused story exists
- **WHEN** `termrock-lookbook list` runs after the gap fill
- **THEN** a `text-input/focused` (and `text-input/disabled`) story id appears

#### Scenario: Gap fill is honest about hover
- **GIVEN** hover is a state the design system models for some widgets
- **WHEN** a subset widget exposes a hover style API (e.g. `hover_style`)
- **THEN** a hover-variant story exists for it, or the story-set doc records why hover is not a paintable story for that widget
