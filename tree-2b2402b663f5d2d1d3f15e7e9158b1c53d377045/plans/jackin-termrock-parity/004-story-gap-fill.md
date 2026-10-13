# Plan 004: Fill the focused/disabled/hover story gaps for the jackin-used subset and bless their PNG baselines

> **Executor instructions**: Follow this plan step by step. Run the
> preconditions first. Run every verification command and confirm the
> expected result before moving on. If anything in "STOP conditions"
> occurs, stop and report — do not improvise. When done, update this
> plan's status row in `plans/jackin-termrock-parity/README.md`.

## Status

- **Priority**: P2
- **Effort**: M
- **Risk**: LOW (purely additive: new stories + new baseline PNGs + regenerated
  demo-code entries; no existing render path is touched)
- **Depends on**: plans/003-*.md (PNG gate test + mise wiring; transitively 001
  raster crate and 002 initial baseline set)
- **Covers**: spec/baselines.md "All-states story gap fill" · B3, F5
- **Guardrails**: N1 (inlined below — new stories must not change existing
  rendering)
- **Research basis**: research/tui-png-baselines/03-termrock-seams-and-old-rev.md
  (Q6), research/tui-png-baselines/05-ci-placement-and-commands.md (Q3)
- **Planned at**: commit `41cf3d0b`, 2026-08-16

## Why this matters

The jackin-used subset's baseline set (plan 002) can only protect states that
have stories. Research ch. 03 Q6 recorded the exact gap: **no focused/disabled
variant stories exist for TextInput, Tabs, Toast, StatusBar, ActionBar, and no
hover-variant story id exists for any subset component** (hover APIs exist and
are exercised only incidentally inside other components' stories, e.g.
`stories.rs:16200`, `22628`, `22847`). After this plan lands, every state those
five widgets can honestly paint through public API has a registered story and a
committed, gate-protected PNG baseline — so B3's "all states" holds and plan
008's old-vs-new comparison reports have complete HEAD-side material.

## Preconditions — run before anything else

Run each from the repository root `/Users/donbeave/Projects/tailrocks/termrock`.
Any failure is a STOP.

1. Clean tree: `git status --porcelain` → empty output.
2. Plan 003 landed (mise wiring): `grep -c "bless-pngs" mise.toml` → ≥ 1
   match, and `grep -c "png-baselines" mise.toml` → ≥ 1 match
   (spec/ci-gate.md mandates exactly these task names).
3. Plan 003 landed (gate test): `grep -rl "TERMROCK_BLESS_PNGS" crates/termrock-lookbook/tests/`
   → at least one test file (the PNG baseline gate; the env var name is fixed
   by spec/ci-gate.md's bless scenario).
4. Plan 002 landed (baselines exist): `ls crates/termrock-lookbook/baselines/png/*.png | wc -l`
   → ≥ 80 (research counted 87 subset stories at planning time; 002 commits one
   PNG per subset story).
5. Gate green on the clean tree: `mise run png-baselines` → exit 0, no drifted
   or missing story ids reported.
6. Drift check (this plan touches pre-existing code):
   `git diff --stat 41cf3d0b..HEAD -- crates/termrock-lookbook/src/stories.rs crates/termrock/src/widgets/text_input.rs crates/termrock/src/widgets/tabs.rs crates/termrock/src/widgets/toast.rs crates/termrock/src/widgets/status_bar.rs crates/termrock/src/widgets/action_bar.rs`
   → expected empty (plans 001–003 do not touch these files). If anything
   changed, re-read every excerpt quoted under "Starting state" against the
   live files (line numbers may shift; the quoted content must still exist).
   A content mismatch is a STOP.

## Spec contract

Inlined verbatim from `plans/jackin-termrock-parity/spec/baselines.md` — the
executor does not read `spec/`:

### Requirement: All-states story gap fill
The lookbook SHALL gain stories closing the recorded state gaps for the
subset so B3's "all states" holds: focused and disabled variants for
TextInput, Tabs, Toast, StatusBar, and ActionBar (the components ch. 03 Q6
names as lacking them). Each new story registers under the existing
component id scheme and thereby joins the baseline set automatically.
Covers: B3, F5 · Evidence: ch. 03 Q6 (gap list; no hover-variant story exists for any subset component)

#### Scenario: TextInput focused story exists
- **WHEN** `termrock-lookbook list` runs after the gap fill
- **THEN** a `text-input/focused` (and `text-input/disabled`) story id appears

#### Scenario: Gap fill is honest about hover
- **GIVEN** hover is a state the design system models for some widgets
- **WHEN** a subset widget exposes a hover style API (e.g. `hover_style`)
- **THEN** a hover-variant story exists for it, or the story-set doc records why hover is not a paintable story for that widget

Done means these scenarios hold; the test plan below exercises them.

### Contract reconciliation (planning-time finding — follow this, do not improvise)

The requirement names focused **and** disabled variants for all five widgets.
Planning-time code inspection (citations under "Starting state") found that two
of the five widgets do not model those states in any public paint API:

- **Toast** is never focusable by design (`toast.rs:12-13` module doc: "Toasts
  are **never focusable**"; `ToastState::is_focusable()` hard-returns `false`
  at `toast.rs:509-513`; test `never_focusable` at `toast.rs:1802`) and has no
  disabled or hover surface at all.
- **StatusBar** has no focused state (`StatusBarState` fields are `hovered`,
  `regions`, `transient`, mode-fade only — `status_bar.rs:357-371`), and a
  slot with `.enabled(false)` is **omitted from allocation entirely**
  (`status_bar.rs:979-981`) rather than painted in a disabled style — there is
  no disabled visual to record.

The lookbook's hard law (`crates/termrock-lookbook/CLAUDE.md`) forbids stories
that bypass public API, so `toast/focused`, `toast/disabled`,
`status-bar/focused`, and `status-bar/disabled` cannot be painted without
lying. This plan resolves the tension with the same honesty mechanism the
spec's hover scenario establishes: **every state a widget can honestly paint
gets a story; every state it cannot is recorded, with the code reason, in the
story-set note** (Step 1). If you believe this reconciliation is wrong after
reading the cited code, STOP and report — do not invent a paint path.

### Final story id list (8 new stories)

| New story id | State painted via | Honest? |
|--------------|-------------------|---------|
| `text-input/focused` | `TextInputState::set_focused(true)` → `ControlState::Focused` | yes |
| `text-input/disabled` | `TextInputState::set_enabled(false)` → `ControlState::Disabled` | yes |
| `tabs/focused` | `TabsState::set_focused(true)` + roving focus cue | yes |
| `tabs/disabled` | `Tab::new(..).enabled(false)` → `Role::TextDisabled` | yes |
| `tabs/hover` | `TabsState.hovered` (pub field) → hover roles + `Role::HoverTint` | yes (hover API exists) |
| `status-bar/hover` | `StatusSlot::hover_style` + `StatusBarState.hovered` (pub field) | yes (hover API exists) |
| `action-bar/focused` | `ActionBarState.cursor` → `Role::ActionFocused` | yes |
| `action-bar/disabled` | `Action { enabled: false, .. }` → `Role::ActionDisabled` | yes |

No hover stories for TextInput, Toast, ActionBar (no hover API exists in their
widget files — recorded in the story-set note). No new Toast stories and no
status-bar focused/disabled stories (reconciliation above — recorded in the
story-set note).

Baseline filenames follow the id scheme `<id with '/' → '-'>.png` (the SVG
exporter's canonical slug, `crates/termrock-lookbook/src/svg.rs:103-105`
`story.id.replace('/', "-")`; spec/baselines.md fixes the PNG names to mirror
it): `text-input-focused.png`, `text-input-disabled.png`, `tabs-focused.png`,
`tabs-disabled.png`, `tabs-hover.png`, `status-bar-hover.png`,
`action-bar-focused.png`, `action-bar-disabled.png` — all under
`crates/termrock-lookbook/baselines/png/`.

## Must NOT

Guardrails inlined verbatim; these override anything a step seems to imply.

- **N1** (spec/README.md must-not registry, verbatim): "The repo MUST NOT ship
  any unreviewed visual divergence from the jackin-era look: every difference
  is restored, merged, or explicitly accepted by a recorded per-component
  verdict" — reason: "item §Must not; nothing drifts silently". Manifest
  application to this plan: **new stories must not change existing
  rendering**. Operationally: no existing story function, registration, widget
  source, committed baseline PNG, or golden `.txt` may change; after blessing,
  `git status` under `crates/termrock-lookbook/baselines/png/` and
  `crates/termrock-lookbook/goldens/` must show only the 8 new `??` files and
  zero modifications.
- **No widget behavior changes** (manifest scope): do not edit anything under
  `crates/termrock/src/`. If a state cannot be painted through the existing
  public API, that is a story-set-note fact or a STOP — never a widget patch
  in this plan.
- **Lookbook public-API law** (`crates/termrock-lookbook/CLAUDE.md`, binding):
  "Every story and interactor **must** call the same public API downstream
  applications use." No `pub(crate)` access, no reimplemented key dispatch, no
  raw ratatui Block/Paragraph construction the widget normally owns.
- **N2 context** (owned by 002/003, still binding here): baselines are plain
  git files — never git-LFS. `mise run bless-pngs` writes ordinary files; do
  not add any LFS attribute.

## Inputs to provide

None — fully self-contained.

## Starting state

All line numbers are at commit `41cf3d0b` (run the Precondition 6 drift check;
content governs if lines shifted).

### Story registration shape and id convention

Stories register in the literal `catalog` vec inside `stories()`
(`crates/termrock-lookbook/src/stories.rs:743-744`). Exemplar state-variant
registration, `stories.rs:1052-1061`:

```rust
Story::new(
    "panel/focused",
    "Focused panel",
    "Panel",
    "A semantically focused bordered panel.",
    48,
    7,
    panel,
)
.with_interactor(panel_interactor),
```

Argument order: id, title, component, description, cols, rows, render fn.
Disabled-variant exemplar `"list/disabled"` at `stories.rs:4284-4292` (fn
`list_disabled` at `:13528`). Id convention is
`<component-kebab>/<state-word>` (`panel/focused`, `list/disabled`); **no
`/hover` id exists anywhere yet** (ch. 03 Q6) — this plan introduces the first
two using the same single-word convention.

Existing ids for the five components (collision check — none of the 8 new ids
exists): action-bar/{basic,narrow,unicode}; tabs/{status,overflow,vertical,
manual,closable,narrow,unicode}; status-bar/{basic,minimal,transient,rich,
narrow,unicode}; toast/{success,kinds,stack,persistent,narrow,unicode};
text-input/{basic,secret,invalid,prefix,narrow,unicode}; plus generated
`*/in-app` entries from `IN_APP_SCENES` (`stories.rs:279`).

The literal vec closes at `stories.rs:10442` (`];`) — its last entry is
`text-input/prefix` (`:10433-10441`) — followed by
`catalog.extend(in_app_stories(&catalog));` at `:10449`. The file's top-level
`use ratatui::{ layout::{Constraint, Layout, Rect}, .. }` (`stories.rs:9-11`)
and existing story fns make `TextInput`, `TextInputState`, `Tab`, `Tabs`,
`TabsState`, `TabsActivation`, `StatusBar`, `StatusBarState`, `Action`,
`ActionBar`, `ActionBarState`, `Style`, `Span`, `Role` available unqualified;
`StatusSlot`/`StatusRegion` are imported locally inside status-bar fns
(`stories.rs:16197`). The file ends with `fn transcript_ascii_colorless` — the
EOF is the insertion anchor for new fns.

### Sibling story functions to mirror

`text_input_basic_story` (`stories.rs:25256-25263`):

```rust
fn text_input_basic_story(frame: &mut Frame<'_>, area: Rect, system: &DesignSystem) {
    let mut state = TextInputState::new("filter term");
    state.set_focused(true);
    let _ = TextInput::new("Query", system)
        .placeholder("Search…")
        .show_clear(true)
        .paint(area, frame.buffer_mut(), &mut state);
}
```

`fn tabs` (tabs/status, `stories.rs:13301-13337`) builds `Tab { .. }` literals,
`TabsState::new().with_selected("overview")`, `state.set_focused(true)`, then
`frame.render_stateful_widget(&Tabs::new(&items, system).gap(1), area, &mut state)`.
`tabs_manual_story` (`stories.rs:13371-13392`) is the roving-focus exemplar:
`TabsState::new().with_selected("a").with_activation(TabsActivation::Manual)`,
`set_focused(true)`, then
`let _ = state.handle_key(termrock::input::KeyEvent::new(termrock::input::KeyCode::Right, termrock::input::KeyModifiers::NONE), &items);`
before `Tabs::new(&items, system).ascii(true).paint(area, frame.buffer_mut(), &mut state);`.

`fn status_bar` (status-bar/basic, `stories.rs:16196-16216`) already passes
`.hover_style(Style::new().bold().reversed())` / `.hover_style(Style::new().bold())`
on its mode and selection slots (the ch. 03 Q6 `stories.rs:16200` citation) but
never sets `state.hovered` — so no hover story id exists.

`fn action_bar` (action-bar/basic, `stories.rs:12234-12258`) builds
`Action { id: "accept", label: "Accept", enabled: true, style: None }` literals
and `ActionBarState { cursor: Some("accept"), ..ActionBarState::default() }`,
rendering via `frame.render_stateful_widget(&ActionBar::new(&actions, system).gap("  "), area, &mut state)`.

Layout destructure exemplar (`stories.rs:11720`):
`Layout::vertical([Constraint::Length(1), Constraint::Length(1)]).areas(area);`.

### Widget state APIs the new stories call (all public)

- **TextInput** (`crates/termrock/src/widgets/text_input.rs`):
  `TextInputState::new(value)` defaults `focused: false`, `enabled: true`
  (`:229-249`); `set_enabled(&mut self, on: bool)` (`:273`); `set_focused`
  (`:288`); paint resolves `ControlState::Disabled` when `!state.enabled`,
  `Focused` when `state.focused` (`:1102-1109`); cursor painted only when
  `state.focused && state.enabled` (`:1185`);
  `TextInput::new(label, system)` (`:912`),
  `paint(area, buffer, &mut state) -> TextInputParts` (`:1006-1011`). No hover
  API exists anywhere in the file.
- **Tabs** (`crates/termrock/src/widgets/tabs.rs`): `Tab::new(id, label)` with
  builder `enabled(false)` (`:291`); `TabsState::new()` (`:396`),
  `with_selected` (`:447`), `with_activation`, `set_focused` (`:493`),
  `handle_key(key, &tabs)` (`:583`), public field `pub hovered: Option<Id>`
  (`:358`). Paint: hover selects `Role::TabActiveHovered`/`TabInactiveHovered`
  (`:1248-1253`) plus `Role::HoverTint` bg (`:1314-1315`); roving focus on a
  **non-selected** tab patches `Role::BorderFocused` + BOLD (`:1304-1311`) —
  focus on the selected tab adds nothing beyond the selected BOLD (`:1312-1313`),
  which is why the focused story must move roving focus off the selection;
  a tab with `enabled: false` paints `Role::TextDisabled` (`:1317-1321`);
  `paint(&self, area, buffer, state)` (`:918`).
- **Toast** (`crates/termrock/src/widgets/toast.rs`): "Toasts are **never
  focusable**" (`:12-13`), `is_focusable() -> false` (`:509-513`), test
  `never_focusable` (`:1802`); no disabled or hover API in the file.
- **StatusBar** (`crates/termrock/src/widgets/status_bar.rs`):
  `StatusSlot::mode/selection/shortcut/connection/focus_zone` constructors
  (`:265` for `focus_zone`), builders `.enabled(bool)` (`:303`),
  `.style(Style)` (`:310`), `.hover_style(Style)` (`:318`);
  `StatusBarState` public field `pub hovered: Option<Id>` (`:359`) and
  pointer-driven `hover(&mut self, position)` (`:423`). `resolve_style`
  returns the slot's `hover_style` when hovered (`:949-954`); a slot with
  `enabled: false` is dropped from allocation (`:979-981`). No focused state.
- **ActionBar** (`crates/termrock/src/widgets/action_bar.rs`): `Action` public
  fields `{ id, label, enabled, style }` (`:17-26`); `ActionBarState` public
  field `cursor: Option<Id>` (`:33-38`; `set_focused` exists only as a
  deprecated alias `:56-60` — use the `cursor` field); paint selects
  `Role::ActionDisabled` when `!action.enabled`, `Role::ActionFocused` when on
  cursor (`:188-198`). No hover API.

### Gate and verification environment (from plans 002/003 + ch. 05)

- Baselines live at `crates/termrock-lookbook/baselines/png/<slug>.png`
  (plain git, N2). The gate test (003) renders every subset story via
  termrock-raster, compares decoded pixels at zero tolerance (N3), **fails on
  a missing baseline naming the story ids and instructing
  `mise run bless-pngs`** (spec/ci-gate.md "Missing baseline fails"), and
  rewrites PNGs when `TERMROCK_BLESS_PNGS=1` is set. The five components in
  this plan are all inside the 16-family subset, so new stories "join the
  baseline set automatically" (spec).
- The gate runs on every PR through `cargo nextest run --workspace` via
  `mise run ci`/`test` (ch. 05 Q2/Q3) — no workflow edits exist or are needed.
- Docs-side checks at planning time: `docs/scripts/check-component-pages.ts`
  validates page→catalog only (`:91-93`, `:135-137`), `check-catalog.ts`
  requires ≥1 story per public widget, and `check-contracts.ts` fails only on
  *unknown* story ids (`:173-174`) — those three are additive-safe. **Not**
  additive-safe: `docs/scripts/generate-demo-code.ts` emits one snippet per
  `Story::new` registration in stories.rs (`generate-demo-code.ts:106-126`)
  into `docs/public/demo-code.json` (output path `:5-7`), and its `--check`
  mode (`:129-133`) — run by the docs `check:snippets` script inside the docs
  `build` (docs/package.json), which `mise run gate` executes — fails while
  the committed JSON lacks the 8 new entries. Regenerating that one file
  (Step 2) is the single required docs-side write; the rest of `docs/**`
  stays untouched.
- Flagship text goldens (`crates/termrock-lookbook/tests/goldens.rs:20-36`)
  cover a fixed 15-id `FLAGSHIP` list; none of the 8 new ids is in it, and no
  existing golden may change.

## Commands you will need

Proven by research ch. 05 (Q2/Q3) against `mise.toml` at `41cf3d0b`; the
`bless-pngs`/`png-baselines` names are fixed by spec/ci-gate.md "Mise task
wiring" (delivered by plan 003).

| Purpose | Command | Expected on success |
|---------|---------|---------------------|
| List story ids | `cargo run -q -p termrock-lookbook -- list` | one `id<TAB>title` line per story (`main.rs:130-134`) |
| PNG gate diff | `mise run png-baselines` | exit 0 when baselines match |
| Bless PNGs | `mise run bless-pngs` | writes missing/changed baseline PNGs |
| Format (apply) | `cargo fmt --all` | exit 0 |
| Format (check) | `mise run fmt` (`cargo fmt --all -- --check`, mise.toml:41-42) | exit 0 |
| Lint | `mise run lint` (clippy `-D warnings`, mise.toml:38-39) | exit 0 |
| Tests | `mise run test` (`cargo nextest run --workspace --all-features --locked`, mise.toml:35-36) | all pass |
| Regenerate docs demo snippets | `bun docs/scripts/generate-demo-code.ts` (or `(cd docs && bun run generate:demo-code)`) | rewrites `docs/public/demo-code.json` |
| Demo-snippet freshness | `bun docs/scripts/generate-demo-code.ts --check` | exit 0 when the committed JSON is current |
| Pre-push proof | `mise run gate` (mise.toml:44-67; supersets `check` and runs the docs `build`, whose `check:snippets` runs `generate-demo-code.ts --check`) | exit 0 |

## Suggested executor toolkit

- Read `crates/termrock-lookbook/CLAUDE.md` before writing story code — it is
  the story-authoring law this plan's bodies already comply with.
- `crates/termrock-lookbook/src/stories.rs` sibling functions cited above are
  the only style reference needed.

## Scope

**In scope** (the only files to create or modify):

- `crates/termrock-lookbook/src/stories.rs` — 8 new story fns, 1 story-set
  note comment block, 8 new `Story::new` registrations.
- `crates/termrock-lookbook/baselines/png/` — exactly 8 new PNGs written by
  `mise run bless-pngs` (filenames listed under "Final story id list").
- `docs/public/demo-code.json` — regenerated by
  `bun docs/scripts/generate-demo-code.ts` (Step 2); the script emits one
  snippet per `Story::new` registration (`generate-demo-code.ts:106-126`), so
  the 8 new registrations add 8 entries. Never hand-edit this file.

**Out of scope** (do NOT touch, even though related):

- `crates/termrock/src/**` — widget behavior/paint changes are forbidden here
  (visual changes are plan 009's verdict-application territory).
- `crates/termrock-raster/**`, the gate test file, `mise.toml` — plans 001/003
  territory.
- Existing baselines and `crates/termrock-lookbook/goldens/*.txt` — N1.
- `docs/**` other than the regenerated `docs/public/demo-code.json` above,
  plus `migrations/`, `MIGRATING.md` — no public termrock API changes in
  this plan, so no migration file is due.
- Old-rev harness and comparison reports — plans 007/008.

The hub `plans/jackin-termrock-parity/README.md` status row is a protocol
write staged in this plan's final commit. Roadmap item + index writes are
owned by the hub's Executor protocol (first-started-plan / package-completion
events only) and are never part of this plan's commit.

## Git workflow

- Branch: none — repo law: "All TermRock work happens directly on `main`. Do
  not create feature branches or pull requests" (CLAUDE.md).
- One commit carrying stories + blessed baselines + regenerated
  `docs/public/demo-code.json` + hub status row **together** (the manifest
  requires baselines blessed in the same change). Conventional Commits with
  DCO sign-off:
  `git commit -s -m "feat(lookbook): fill focused/disabled/hover story gaps for the jackin subset"`.
- Push `main` only after `mise run gate` exits 0 in this session
  (mise.toml:44-67 — the documented pre-push gate; its docs `build` step is
  what enforces demo-code.json freshness. Repo law: push only when the
  documented gate is green).

## Steps

### Step 1: Append the story-set note and the 8 story functions

At the **end of** `crates/termrock-lookbook/src/stories.rs` (after
`fn transcript_ascii_colorless`), append the following block. The comment is
the **story-set note** the spec's honesty scenario requires — it is the
recorded reason a state has no story, so keep its content (wording may be
tightened, facts may not be dropped):

```rust
// ── State-variant gap fill (spec/baselines.md "All-states story gap fill", B3) ──
//
// Story-set note — state coverage per jackin-subset widget:
// - TextInput: focused/disabled paint via ControlState (text_input.rs). No
//   hover API exists in widgets/text_input.rs, so hover is not a paintable
//   story for TextInput.
// - Tabs: hover IS modeled (TabsState::hovered → Role::TabInactiveHovered /
//   Role::HoverTint) — tabs/hover paints it. Focused paint is the roving
//   focus cue on a non-selected tab; per-tab enabled(false) is the disabled
//   paint.
// - Toast: never focusable by design (widgets/toast.rs: "Toasts are never
//   focusable"; ToastState::is_focusable() == false) and exposes no disabled
//   or hover API — no focused/disabled/hover story can be painted through
//   public API.
// - StatusBar: hover IS modeled (StatusSlot::hover_style +
//   StatusBarState::hovered) — status-bar/hover paints it. The bar has no
//   focused state, and a slot with enabled(false) is omitted from allocation
//   rather than painted disabled — no focused/disabled story exists to paint.
// - ActionBar: cursor (Role::ActionFocused) and Action.enabled == false
//   (Role::ActionDisabled) are the paintable states. No hover API exists in
//   widgets/action_bar.rs, so hover is not a paintable story for ActionBar.

fn text_input_focused_story(frame: &mut Frame<'_>, area: Rect, system: &DesignSystem) {
    let [blurred, _, focused] = Layout::vertical([
        Constraint::Length(2),
        Constraint::Length(1),
        Constraint::Length(2),
    ])
    .areas(area);
    let mut blurred_state = TextInputState::new("resting value");
    let _ = TextInput::new("Blurred", system).paint(blurred, frame.buffer_mut(), &mut blurred_state);
    let mut focused_state = TextInputState::new("editing value");
    focused_state.set_focused(true);
    let _ = TextInput::new("Focused", system).paint(focused, frame.buffer_mut(), &mut focused_state);
}

fn text_input_disabled_story(frame: &mut Frame<'_>, area: Rect, system: &DesignSystem) {
    let mut state = TextInputState::new("locked value");
    state.set_enabled(false);
    let _ = TextInput::new("Disabled", system).paint(area, frame.buffer_mut(), &mut state);
}

fn tabs_focused_story(frame: &mut Frame<'_>, area: Rect, system: &DesignSystem) {
    let items = [
        Tab::new("overview", "Overview"),
        Tab::new("details", "Details"),
        Tab::new("logs", "Logs"),
    ];
    let [blurred, _, focused] = Layout::vertical([
        Constraint::Length(2),
        Constraint::Length(1),
        Constraint::Length(2),
    ])
    .areas(area);
    let mut blurred_state = TabsState::new().with_selected("overview");
    Tabs::new(&items, system).paint(blurred, frame.buffer_mut(), &mut blurred_state);
    let mut focused_state = TabsState::new()
        .with_selected("overview")
        .with_activation(TabsActivation::Manual);
    focused_state.set_focused(true);
    // Roving focus on a non-selected tab: the focus cue the strip actually has.
    let _ = focused_state.handle_key(
        termrock::input::KeyEvent::new(
            termrock::input::KeyCode::Right,
            termrock::input::KeyModifiers::NONE,
        ),
        &items,
    );
    Tabs::new(&items, system).paint(focused, frame.buffer_mut(), &mut focused_state);
}

fn tabs_disabled_story(frame: &mut Frame<'_>, area: Rect, system: &DesignSystem) {
    let items = [
        Tab::new("overview", "Overview"),
        Tab::new("archive", "Archive").enabled(false),
        Tab::new("logs", "Logs"),
    ];
    let mut state = TabsState::new().with_selected("overview");
    state.set_focused(true);
    Tabs::new(&items, system).paint(area, frame.buffer_mut(), &mut state);
}

fn tabs_hover_story(frame: &mut Frame<'_>, area: Rect, system: &DesignSystem) {
    let items = [
        Tab::new("overview", "Overview"),
        Tab::new("details", "Details"),
        Tab::new("logs", "Logs"),
    ];
    let mut state = TabsState::new().with_selected("overview");
    state.hovered = Some("details");
    Tabs::new(&items, system).paint(area, frame.buffer_mut(), &mut state);
}

fn status_bar_hover_story(frame: &mut Frame<'_>, area: Rect, system: &DesignSystem) {
    use termrock::widgets::{StatusRegion, StatusSlot};
    let left = [StatusSlot::mode("mode", "NOR")
        .style(Style::new().reversed())
        .hover_style(Style::new().bold().reversed())];
    let center = [StatusSlot::focus_zone("focus", "main")];
    let right = [
        StatusSlot::selection("sel", "3/12")
            .style(Style::new().dim())
            .hover_style(Style::new().bold()),
        StatusSlot::shortcut("hint", "? help").region(StatusRegion::Right),
    ];
    let mut state = StatusBarState::default();
    state.hovered = Some("sel");
    frame.render_stateful_widget(
        &StatusBar::with_center(&left, &center, &right, system)
            .rich()
            .alpha(1.0),
        area,
        &mut state,
    );
}

fn action_bar_focused_story(frame: &mut Frame<'_>, area: Rect, system: &DesignSystem) {
    let actions = [
        Action {
            id: "accept",
            label: "Accept",
            enabled: true,
            style: None,
        },
        Action {
            id: "cancel",
            label: "Cancel",
            enabled: true,
            style: None,
        },
    ];
    let [resting, _, focused] = Layout::vertical([
        Constraint::Length(1),
        Constraint::Length(1),
        Constraint::Length(1),
    ])
    .areas(area);
    let mut resting_state = ActionBarState::default();
    frame.render_stateful_widget(
        &ActionBar::new(&actions, system).gap("  "),
        resting,
        &mut resting_state,
    );
    let mut cursor_state = ActionBarState {
        cursor: Some("accept"),
        ..ActionBarState::default()
    };
    frame.render_stateful_widget(
        &ActionBar::new(&actions, system).gap("  "),
        focused,
        &mut cursor_state,
    );
}

fn action_bar_disabled_story(frame: &mut Frame<'_>, area: Rect, system: &DesignSystem) {
    let actions = [
        Action {
            id: "accept",
            label: "Accept",
            enabled: true,
            style: None,
        },
        Action {
            id: "delete",
            label: "Delete",
            enabled: false,
            style: None,
        },
    ];
    let mut state = ActionBarState::default();
    frame.render_stateful_widget(&ActionBar::new(&actions, system).gap("  "), area, &mut state);
}
```

Design intent (for judgment calls): the focused stories paint a resting
instance **and** the state instance so the baseline PNG records the delta —
precedent: the ladder stories `design-system/button-recipes`
("… × default/focused/disabled", `stories.rs:781-789`) and the FieldRow states
story (`stories.rs:22623-22631`). Single-state stories (disabled, hover)
paint one instance because the state chrome is legible alone. If the compiler
rejects any call here, the quoted API in "Starting state" has drifted — STOP.

In the same edit, register the 8 stories: insert the following entries at the
end of the literal catalog vec, immediately **after** the `text-input/prefix`
entry and **before** the closing `];` (`stories.rs:10441-10442` at planning):

```rust
Story::new(
    "text-input/focused",
    "TextInput focused",
    "TextInput",
    "Blurred field above focused field: cursor and focus chrome delta.",
    40,
    5,
    text_input_focused_story,
),
Story::new(
    "text-input/disabled",
    "TextInput disabled",
    "TextInput",
    "Disabled input; edits blocked and cursor suppressed.",
    40,
    2,
    text_input_disabled_story,
),
Story::new(
    "tabs/focused",
    "Tabs focused",
    "Tabs",
    "Blurred strip above focused strip with roving focus cue.",
    52,
    5,
    tabs_focused_story,
),
Story::new(
    "tabs/disabled",
    "Tabs disabled",
    "Tabs",
    "A disabled tab muted among enabled ones.",
    48,
    2,
    tabs_disabled_story,
),
Story::new(
    "tabs/hover",
    "Tabs hover",
    "Tabs",
    "Pointer hover tint on an inactive tab.",
    52,
    2,
    tabs_hover_story,
),
Story::new(
    "status-bar/hover",
    "Status bar hover",
    "StatusBar",
    "Hovered slot painted with its hover style.",
    64,
    1,
    status_bar_hover_story,
),
Story::new(
    "action-bar/focused",
    "Action bar focused",
    "ActionBar",
    "Bar without cursor above bar with action cursor.",
    48,
    3,
    action_bar_focused_story,
),
Story::new(
    "action-bar/disabled",
    "Action bar disabled",
    "ActionBar",
    "Disabled action muted beside an enabled one.",
    48,
    2,
    action_bar_disabled_story,
),
```

Then run `cargo fmt --all`.

**Verify**:
`TAB=$(printf '\t'); cargo run -q -p termrock-lookbook -- list | grep -cE "^(text-input/(focused|disabled)|tabs/(focused|disabled|hover)|status-bar/hover|action-bar/(focused|disabled))${TAB}"` → prints `8`.
Also confirm nothing else changed: `git status --porcelain` → exactly one
modified file, `crates/termrock-lookbook/src/stories.rs`.

### Step 2: Regenerate the docs demo-code snapshot

`docs/scripts/generate-demo-code.ts` emits one snippet per `Story::new`
registration (`generate-demo-code.ts:106-126`), so Step 1 made the committed
`docs/public/demo-code.json` stale. Regenerate it from the repository root:

`bun docs/scripts/generate-demo-code.ts`

(equivalently `(cd docs && bun run generate:demo-code)`; the script resolves
every path from its own file location, `generate-demo-code.ts:5-7`, so both
invocations write the same file).

**Verify** (both):

1. `bun docs/scripts/generate-demo-code.ts --check` → exit 0 (the same check
   the docs `check:snippets` script runs inside the docs `build`,
   docs/package.json — the path `mise run gate` executes in Step 6).
2. `git status --porcelain` → exactly two modified files:
   `crates/termrock-lookbook/src/stories.rs` and `docs/public/demo-code.json`.

### Step 3: Prove the gate sees the new stories as missing baselines

Run `mise run png-baselines`.

**Verify**: the run **fails**, naming missing baselines drawn from the 8 new
story ids with the `mise run bless-pngs` instruction (spec/ci-gate.md scenario
"Missing baseline fails"). Do **not** require a single run to enumerate all 8
ids — the gate may report a partial list per run; completeness is proven by
the Step 4 file count instead. Two deviations are STOPs: (a) the run passes —
the gate's subset filter did not pick up the new stories (plan 003 defect);
(b) any run's failure names an id **outside** the 8 — existing rendering
drifted (N1 violation or dirty starting state).

### Step 4: Bless the new baselines and check N1

Run `mise run bless-pngs`, then `mise run png-baselines`. If the gate still
reports missing baselines, iterate bless → gate until the gate passes.

**Verify** (all four):

1. `git status --porcelain crates/termrock-lookbook/baselines/png/` → exactly
   8 lines, all `??` (untracked additions), matching the filename list under
   "Final story id list". **Zero `M` lines** — any modified existing baseline
   is an N1 violation and a STOP.
2. All 8 new PNGs exist:
   `ls crates/termrock-lookbook/baselines/png/ | grep -E '^(text-input|tabs|status-bar|action-bar)-(focused|disabled|hover)\.png$'`
   → 8 lines (the pattern matches exactly the "Final story id list" filenames
   and no pre-existing baseline).
3. `git status --porcelain crates/termrock-lookbook/goldens/` → empty.
4. `mise run png-baselines` → now exits 0 (spec/ci-gate.md scenario "Bless
   rewrites and the PR carries the diff": bless then pass).

### Step 5: Full verification on the working tree

Run in order:

1. `mise run fmt` → exit 0.
2. `mise run lint` → exit 0.
3. `mise run test` → all tests pass (workspace nextest includes the PNG gate
   test and the flagship goldens; both must be green).

**Verify**: all three commands exit 0. A goldens failure means an existing
story's paint changed — N1 STOP.

### Step 6: Commit, gate, push

1. Update this plan's status row in `plans/jackin-termrock-parity/README.md`
   (protocol write; roadmap item + index writes are owned by the hub's
   Executor protocol — first-started-plan / package-completion events only —
   and are not part of this commit).
2. Stage exactly: `crates/termrock-lookbook/src/stories.rs`, the 8 new PNGs
   under `crates/termrock-lookbook/baselines/png/`,
   `docs/public/demo-code.json`, and the hub README status row.
3. After staging, `git status --porcelain` → nothing modified or untracked
   outside the staged set (a leftover `M`/`??` line is a scope violation —
   STOP).
4. `git commit -s -m "feat(lookbook): fill focused/disabled/hover story gaps for the jackin subset"`
   (stories + baselines + regenerated demo-code + hub row in the same commit,
   per manifest).
5. `mise run gate` → exit 0 (mise.toml:44-67; pushing before this exits 0 is
   forbidden).
6. `git push origin main`.

**Verify**: `git show --stat HEAD` lists only the staged files above;
`git status --porcelain` → empty; push accepted, and only after the gate's
exit 0.

## Test plan

No new test files: the manifest scopes this plan to stories.rs additions and
blessed baselines, and the durable enforcement is the plan-003 gate test —
each committed PNG pins its story's pixels on every PR via workspace nextest
(ch. 05 Q3). Scenario coverage, each checked by a command above:

- Scenario "TextInput focused story exists" → Step 1 verification: the `list`
  output contains `text-input/focused` and `text-input/disabled` (grep count 8
  includes both).
- Scenario "Gap fill is honest about hover" → hover stories exist for the two
  widgets exposing hover APIs (`tabs/hover`, `status-bar/hover` in the Step 1
  grep), and the story-set note records why TextInput, Toast, and ActionBar
  have none: `grep -n "Story-set note" crates/termrock-lookbook/src/stories.rs`
  → 1 hit in the new comment block.
- Requirement "joins the baseline set automatically" → Step 3 (gate fails
  naming missing baselines from the 8) then Step 4 (bless iterations add
  exactly 8 PNGs, the `ls` filename count prints 8 lines, gate passes). The
  expected values (the 8 ids) come from this plan, not from the code under
  test, so the check is independent.
- N1 protection → Step 4 checks 1 and 3 (additions only) and Step 5 check 3
  (goldens green).

## Done criteria

Machine-checkable. ALL must hold (current-session command output only):

- [ ] `cargo run -q -p termrock-lookbook -- list` output contains all 8 new
      ids and the Step 1 grep count prints `8`.
- [ ] `mise run png-baselines` exits 0.
- [ ] `bun docs/scripts/generate-demo-code.ts --check` exits 0 (the committed
      `docs/public/demo-code.json` carries the 8 new story entries).
- [ ] `mise run test` exits 0 (includes PNG gate + flagship goldens).
- [ ] `mise run lint` and `mise run fmt` exit 0.
- [ ] Exactly 8 new files under `crates/termrock-lookbook/baselines/png/`
      (`git show --stat HEAD` shows them as added), named per the "Final story
      id list" table; **no existing file under that directory or under
      `crates/termrock-lookbook/goldens/` modified** (N1).
- [ ] `grep -c "Story-set note" crates/termrock-lookbook/src/stories.rs` ≥ 1.
- [ ] No files outside the in-scope list modified (`git show --stat HEAD`) —
      excluding the hub `plans/jackin-termrock-parity/README.md` status row,
      staged in the same final commit; roadmap item + index writes are owned
      by the hub's Executor protocol (first-started-plan / package-completion
      events only) and never appear in this commit.
- [ ] `mise run gate` (mise.toml:44-67) exits 0 before the push; commit is
      signed off (`git log -1 --format=%B` ends with a `Signed-off-by:`
      trailer) and pushed to `origin/main` only after that exit 0.
- [ ] `plans/jackin-termrock-parity/README.md` status row updated.

## STOP conditions

Stop and report back (do not improvise) if:

- Any precondition fails, or a "Starting state" excerpt no longer matches the
  live code (API drift since `41cf3d0b`).
- Step 3's gate run **passes** right after adding the stories (subset filter
  missed them — plan 003 defect), or any of its failure runs names a story id
  other than the 8 new ones (pre-existing pixel drift — N1).
- `mise run bless-pngs` modifies any tracked baseline PNG or any golden
  `.txt` (N1: only 8 additions are legal).
- A story body cannot be written against the public API as quoted (lookbook
  law forbids workarounds; a missing API is a finding for the hub notes, not
  a patch in this plan).
- You conclude the "Contract reconciliation" is wrong — i.e. you find a public
  API that paints Toast focused/disabled or StatusBar focused/disabled that
  this plan missed. Report it; do not silently add or drop stories.
- A step's verification fails twice after a reasonable fix attempt.
- The work would require touching an out-of-scope file or violating a Must
  NOT.

## Maintenance notes

- Plan 008 pairs these baselines with old-rev renders. The old rev has no
  counterpart stories for any of the 8 ids (ch. 03 Q6: old TextInput has only
  `text-input/unicode`), so 008's reports must list them as
  new-state-without-old-counterpart, not skip them.
- Plan 009 verdicts may re-bless these PNGs; the story code itself should not
  need edits then.
- If a hover API is later added to TextInput, Toast, or ActionBar, the
  story-set note's corresponding line must be replaced by a real hover story
  in the same change (cross-surface consistency law) — a stale "no hover API"
  note is a defect.
- Reviewer scrutiny: the Step 4 `git status` capture is the N1 evidence —
  confirm the commit adds exactly 8 PNGs and modifies zero. Also confirm the
  8 PNGs render the states named (GitHub image view): focused stories show a
  resting and a state instance; `tabs/hover` and `status-bar/hover` may look
  close to their basic siblings if the phosphor hover tint is subtle — that is
  the honest record of the theme, not a defect.
- Deferred: `text-input/basic` and `tabs/status` render focused-by-default
  (`stories.rs:25258`, `:13335`), so the catalog now has both a
  focused-by-default basic story and an explicit focused-contrast story per
  widget. Rationalizing basic stories to resting state is a deliberate visual
  change requiring bless + review — out of scope here (N1), noted for a future
  design pass.
