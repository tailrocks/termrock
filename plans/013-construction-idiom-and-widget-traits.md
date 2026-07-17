# Plan 013: One construction idiom, `#[non_exhaustive]` growth room, owned Widget impls, and a serde feature

> **Executor instructions**: Follow this plan step by step. Run every
> verification command and confirm the expected result before moving to the
> next step. If anything in the "STOP conditions" section occurs, stop and
> report — do not improvise. When done, update the status row for this plan
> in `plans/README.md`.
>
> **Drift check (run first)**: `git diff --stat da54a03..HEAD -- crates/termrock/src/widgets/ crates/termrock/Cargo.toml`
> Plans 008/011 legitimately reshaped widget fields/methods. Verify their
> status in plans/README.md; reconcile excerpts against live code; STOP on
> unexplained mismatch.

## Status

- **Priority**: P2
- **Effort**: M
- **Risk**: MED (constructor churn across all widgets; mechanical, compiler-guided)
- **Depends on**: plans/008-theme-constructor-and-role-threading.md, plans/011-event-model-convergence.md (field sets must be final first)
- **Category**: tech-debt
- **Planned at**: commit `da54a03`, 2026-07-16

## Why this matters

Four construction idioms coexist across 16 widgets: plain pub-field structs (`List { rows, theme }`, `Tabs`, `Dialog`, `StatusBar`, `DiffView`, `Viewport`, `DetailTable`, `ActionBar`, `TextInput`), builder-over-public-fields (`Panel::new(theme).title(..).emphasis(..)`), builder-over-private-fields (`Toast::new(theme, message, severity).anchor(..).margins(..)`), and private-struct+`new()` (`Form::new(sections, theme)`, `SplitPane::new(..)`). A consumer cannot predict any widget's shape. Zero types in the crate carry `#[non_exhaustive]`, so with pub-field structs every added field is an instant compile-break with no `..Default::default()` escape. Separately, every `Widget`/`StatefulWidget` impl exists only for `&T` — consumers must write `frame.render_widget(&toast, area)`; idiomatic owned rendering (`frame.render_widget(toast, area)`) fails. And none of the plain-data state types (`TextInputState`, `SplitPaneState`, `DiffState`, `ListState`, …) can be persisted — no serde support exists even optionally.

## Current state

- Idiom examples (verbatim):
  - `widgets/panel.rs` — builder over **public** fields:

```rust
pub struct Panel<'a> {
    pub title: Option<&'a str>,
    pub emphasis: PanelEmphasis,
    pub style: Option<Style>,
    pub theme: &'a Theme,
}
impl<'a> Panel<'a> {
    pub const fn new(theme: &'a Theme) -> Self { ... }
    pub const fn title(mut self, title: &'a str) -> Self { ... }
    pub const fn emphasis(mut self, emphasis: PanelEmphasis) -> Self { ... }
    pub const fn style(mut self, style: Style) -> Self { ... }
```

  - `widgets/toast.rs` — builder over **private** fields (`message, severity, anchor, style, horizontal_margin, vertical_margin, theme` all private; `new(theme, message, severity)` + `anchor/margins/style` builders).
  - `widgets/list.rs` — plain pub fields (`List { rows, theme }`), constructed with struct literals everywhere (see `examples/support/mod.rs`: `&List { rows: &rows, theme: &theme }`).
  - `widgets/form.rs` — `Form::new(sections, theme)`-style private construction (`Form` fields private, `form.rs:290-301`).
- `grep -rn "non_exhaustive" crates/ --include="*.rs"` → zero hits.
- All 18 render impls are reference-only (verified list): `impl StatefulWidget for &List/&Tree/&Form/&DetailTable/&Tabs/&ActionBar/&StatusBar/&SplitPane/&TextInput/&DiffView/&Viewport/&ChoiceDialog/&MessageDialog`, `impl Widget for &Panel/&Toast/&HintBar/&Dialog/&Backdrop`.
- Features (`crates/termrock/Cargo.toml`): only `crossterm`. Dependencies do not include serde.
- Migration 0002 blessed the `Toast` builder shape by name (`Toast::new(theme, message, severity).anchor(...)`) — the most recent deliberate construction decision in the repo.
- Repo conventions: forward-only + migration file; `public-api.txt` regen.

## Commands you will need

| Purpose | Command | Expected on success |
|---|---|---|
| Tests | `cargo test --workspace --all-features --locked` | all pass |
| Feature powerset | `cargo hack check --workspace --feature-powerset --all-targets --locked` | exit 0 |
| Clippy | `cargo clippy --workspace --all-targets --all-features --locked -- -D warnings` | exit 0 |
| Previews | `cargo run -p termrock-lookbook -- check --dir docs/public/component-previews` | exit 0 |

## Scope

**In scope**:
- All widget structs + their state/outcome/enum types in `crates/termrock/src/widgets/`
- `crates/termrock/Cargo.toml` + workspace `Cargo.toml` (serde optional dep)
- Call sites (lookbook, examples, tests) — compiler-guided
- `migrations/000N-*.md` + `MIGRATING.md`; `public-api.txt` regen

**Out of scope**:
- Method signatures (Plan 011 finished them).
- Theming fields (Plan 008 finished them).
- serde for `Theme`/`Style` (ratatui `Style` serde is a ratatui feature question — defer; state types only).

## Git workflow

- Directly on `main`; `git commit -s -m "feat(widgets)!: canonical construction idiom and owned render impls"`; migration file same commit.

## Steps

### Step 1: Adopt the canonical idiom — `new(required...)` + const builder methods, private fields

Decision (matches the most recent deliberate shape, Toast/migration-0002, and survives field growth without `non_exhaustive` noise in patterns): every widget becomes **private fields + `const fn new(<required>)` + const builder methods for optionals**. Required = the data slice + `theme` (post-008 every themed widget has one). Examples of target shapes:

```rust
let list = List::new(&rows, &theme);                       // no optionals today
let tabs = Tabs::new(&tabs, &theme).gap(2);
let viewport = Viewport::new(&lines, &theme).title("Log");
let status = StatusBar::new(&left, &right, &theme).alpha(0.8);
```

Port order (mechanical, one widget per commit or batched): `List`, `Tabs`, `Dialog`, `StatusBar`, `DiffView`, `Viewport`, `DetailTable`, `ActionBar`, `TextInput`, and align `Panel` (fields go private; builders already exist). `Toast`, `Form`, `SplitPane` already match — verify only. Keep every builder `const fn` where the current builders are (`Panel`/`Toast` prove it works).

Data-carrying row/config structs that consumers construct per item (`ListRow`, `TreeNode`, `Tab`, `StatusSlot`, `FormField`, `FormSection`, `DetailRow`, `DiffLine`, `Hint`, `Action`) **stay pub-field structs** (struct-literal ergonomics matter for projection code) — they get `#[non_exhaustive]` treatment in Step 2 via a different mechanism: add `#[non_exhaustive]` ONLY where a `new()` helper also exists, otherwise leave exhaustive and accept the break-on-grow (document the choice in the migration file). Decide per type by projected growth: `FormField` (7 fields, likely to grow) gets `new()`+`non_exhaustive`; `DiffLine { text, kind }` stays a bare literal.

**Verify**: after each widget, `cargo check --workspace --all-features --locked` → exit 0. Full suite at the end of the step.

### Step 2: `#[non_exhaustive]` on public enums

Add `#[non_exhaustive]` to public enums that will grow: `Role`, `Severity`, `Anchor`, `PanelEmphasis`, all `*Outcome` enums, `interaction::Outcome`, `input::{KeyCode, MouseEventKind, Event}`, `osc::{PointerShape, Request}`, `DiffKind`, `RowRole`, `TreeNodeStatus`. NOTE: in-repo `match` statements on these stay exhaustive-checked (same crate); external consumers gain a forced `_` arm — that is the point. Do NOT add it to `KeyEventKind` (fixed three-state physical fact) or two-variant flags unlikely to grow.

**Verify**: `cargo test --workspace --all-features --locked` → all pass.

### Step 3: Owned render impls

For each `impl Widget for &T` / `impl StatefulWidget for &T`, add the owned delegate:

```rust
impl<Id: Clone + PartialEq> StatefulWidget for List<'_, Id> {
    type State = ListState<Id>;
    fn render(self, area: Rect, buffer: &mut Buffer, state: &mut Self::State) {
        StatefulWidget::render(&self, area, buffer, state);
    }
}
```

All 18 impl sites. Keep the `&T` impls (re-render without rebuild stays possible).

**Verify**: add one test in `widgets/tests.rs` rendering an owned `Panel` via `frame`-style call (`Widget::render(panel, area, &mut buffer)`) → compiles and matches the `&`-render output buffer.

### Step 4: Optional `serde` feature

Workspace `Cargo.toml`: add `serde = { version = "1", features = ["derive"], default-features = false }` to `[workspace.dependencies]`. Crate `Cargo.toml`: `serde = { workspace = true, optional = true }`, feature `serde = ["dep:serde"]`. Gate derives on plain-data state types:

```rust
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct DiffState { ... }
```

Apply to: `DiffState`, `SplitRatio` + `SplitPaneState`'s persistable core (inspect: if it holds `Rect` hit-geometry caches, split persistable vs runtime state is out of scope — derive only where ALL fields are plain data; skip states holding `Rect` regions and note them in the migration file as intentionally non-serde), `ListState<Id>`/`TreeState<Id>`/`FormState<Id>` only if `Id: Serialize` bounds work cleanly with the same all-plain-fields rule; `TextInputState`'s value/cursor core similarly. Be conservative: derive where it compiles without bound gymnastics; list the skips.

**Verify**: `cargo check -p termrock --features serde --locked` → exit 0; `cargo hack check --workspace --feature-powerset --all-targets --locked` → exit 0 (powerset now covers crossterm×serde).

### Step 5: Migration file + regen

Old→new construction table for every widget whose literal construction broke (`List { rows, theme }` → `List::new(&rows, &theme)`, etc.), the `non_exhaustive` list, owned-impl note ("`frame.render_widget(w, area)` now works; `&w` still works"), serde feature doc. Regenerate `public-api.txt`.

**Verify**: migration indexed; `mise run gate` → exit 0.

## Test plan

- Owned-impl equivalence test (Step 3).
- serde round-trip test for one state type under `--features serde`: `serde_json` as dev-dependency? NO — avoid adding dev-deps silently; use `serde_test`? Simplest: gate a test on the feature using `serde::Serialize` trait bounds compile-check only (`fn assert_serde<T: serde::Serialize + serde::de::DeserializeOwned>() {}`). Report if a real round-trip test is wanted (needs a serde format dev-dep — maintainer choice).
- Existing suite + previews = regression net (construction changes must not alter rendering).

## Done criteria

- [x] Every widget constructs via `new(required)` + builders; `grep -rn "List {\|Tabs {\|Viewport {\|DiffView {\|StatusBar {" crates/ --include="*.rs"` (as struct literals) → no non-test matches
- [x] `grep -rc "non_exhaustive" crates/termrock/src/` → ≥ 15
- [x] Owned + reference render impls both exist for all 23 current public widgets (the original 18 plus later graduations)
- [x] `serde` feature compiles standalone and in powerset
- [x] Full suite + previews green; migration indexed; `public-api.txt` regenerated and byte-fresh
- [x] `plans/README.md` status row updated

## STOP conditions

- A builder method can't be `const fn` (non-const field init) — drop const for that method silently is fine ONCE; if it happens on >3 widgets, the const-builder convention is wrong — report.
- serde bounds on `Id`-generic states cascade into public API bounds changes — skip that type, list it, move on.
- Lookbook stories construct widgets in ways the new idiom can't express (e.g. mutating a pub field post-construction) — report the story; a setter-builder is the likely answer but confirm intent.

## Maintenance notes

- New-widget checklist addition: private fields, `new(required)`, const builders, `#[non_exhaustive]` on its enums, owned+ref render impls, serde derive if plain-data.
- Adding a field to any widget is now non-breaking (builder default) — the payoff for this churn.
- If ratatui later stabilizes serde on `Style`, revisit `Theme` serialization (deliberately out of scope here).
