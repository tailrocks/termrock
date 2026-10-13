# Plan 008: Make `Theme` consumer-constructible and route every widget through semantic roles

> **Executor instructions**: Follow this plan step by step. Run every
> verification command and confirm the expected result before moving to the
> next step. If anything in the "STOP conditions" section occurs, stop and
> report — do not improvise. When done, update the status row for this plan
> in `plans/README.md`.
>
> **Drift check (run first)**: `git diff --stat da54a03..HEAD -- crates/termrock/src/style/ crates/termrock/src/widgets/tabs.rs crates/termrock/src/widgets/action_bar.rs crates/termrock/src/widgets/hint_bar.rs crates/termrock/src/widgets/status_bar.rs crates/termrock/src/widgets/diff.rs crates/termrock/src/widgets/viewport.rs`
> On mismatch with "Current state" excerpts, STOP.

## Status

- **Priority**: P1
- **Effort**: M
- **Risk**: MED (changes rendered output paths for 6 widgets; lookbook SVG determinism checks and render tests are the net)
- **Depends on**: none (Plan 009 depends on THIS)
- **Category**: tech-debt
- **Planned at**: commit `da54a03`, 2026-07-16

## Why this matters

TermRock's stated purpose (AGENTS.md) is a product-neutral component library whose consumers re-theme it for their own brands — and the repo's new "Modern-first, pre-stable API" section explicitly requires the default phosphor design to "never prevent the library from being product-neutral, fully re-themable". Today that is impossible twice over: (1) `Theme` has a private `roles: [Style; 22]` array and exactly one constructor, `tailrocks_phosphor()`; (2) even with a constructible Theme, six widgets never consult it — `Tabs` hardcodes `TAB_BG_*`/`WHITE`/`GREEN` constants, `HintBar`'s `styled_hint_spans` hardcodes `WHITE`/`PHOSPHOR_GREEN`/`PHOSPHOR_DIM`/`BORDER_GRAY`, `ActionBar` hardcodes `reversed()`/`dim()` fallbacks, and `StatusBar`/`DiffView`/`Viewport` take raw per-field `Style`s with no semantic meaning. This plan gives `Theme` a public builder and threads roles through all six bypassing widgets, with the phosphor palette surviving as the default.

## Current state

- `crates/termrock/src/style/mod.rs` — the `Role` enum (22 variants, exact order is ABI for the array):

```rust
pub enum Role {
    Canvas, Surface, Elevated, Backdrop,
    Text, TextMuted, TextDisabled,
    Border, BorderFocused,
    Selection, Focus, Accent,
    Success, Warning, Danger, Info,
    Link, LinkHover,
    Input, InputInvalid,
    ScrollTrack, ScrollThumb,
}
```

- `Theme` (same file):

```rust
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Theme {
    roles: [Style; 22],
}

impl Theme {
    #[must_use]
    pub fn tailrocks_phosphor() -> Self {
        Self { roles: [ /* 22 positional styles */ ] }
    }

    #[must_use]
    pub const fn style(&self, role: Role) -> Style {
        self.roles[role as usize]
    }
}

impl Default for Theme {
    fn default() -> Self { Self::tailrocks_phosphor() }
}
```

- Widgets already role-driven (leave alone): `list`, `tree`, `text_input`, `dialog`, `detail_table`, `form`, `panel`, `toast`, `split_pane` — all carry `theme: &Theme` and call `theme.style(Role::…)`.
- The six bypassers (excerpts):
  - `widgets/tabs.rs` — `pub struct Tabs<'a, Id> { pub tabs: &'a [Tab<'a, Id>], pub gap: u16 }` (no theme field); render matches `(selected, hovered)` to `TAB_BG_ACTIVE_HOVER / TAB_BG_ACTIVE / TAB_BG_INACTIVE_HOVER / TAB_BG_INACTIVE`, text `Style::new().fg(WHITE)`, focused underline `GREEN` vs `Style::new().fg(WHITE)`.
  - `widgets/hint_bar.rs` — `styled_hint_spans(spans, remap)` builds `key = WHITE+BOLD`, `text = PHOSPHOR_GREEN`, `dim = PHOSPHOR_DIM`, `sep = BORDER_GRAY` from `crate::style::` constants.
  - `widgets/action_bar.rs` — `pub struct ActionBar<'a, Id> { pub actions: …, pub gap: &'a str }`; per-action fallback `Style::new().dim()` (disabled) / `.reversed()` (focused).
  - `widgets/status_bar.rs` — `pub struct StatusBar<'a, Id> { pub left, pub right, pub style: Style, pub alpha: f32 }`.
  - `widgets/diff.rs` — `pub struct DiffView<'a> { pub lines, pub added_style: Style, pub removed_style: Style }`; style/mod.rs already has `DIFF_ADDED_BG/FG`, `DIFF_REMOVED_BG/FG` constants nothing threads through.
  - `widgets/viewport.rs` — 7 raw `Style` fields (`content_style`, `border_style`, `title_style`, `scroll_track_style`, `scroll_thumb_style`, …).
- The lookbook (`crates/termrock-lookbook/src/stories.rs`, `interactors.rs`) constructs `Theme::default()` throughout and is the only in-repo consumer; the SVG preview check (`cargo run -p termrock-lookbook -- check --dir docs/public/component-previews`) pins rendered output.
- Repo conventions: breaking changes welcome + migration file same commit; public API changes must regenerate `docs/api/public-api.txt` if Plan 003's gate is live; render-behavior changes must keep the lookbook previews deterministic (re-render + commit them in the same change).

## Commands you will need

| Purpose | Command | Expected on success |
|---|---|---|
| Tests | `cargo test --workspace --all-features --locked` | all pass |
| Clippy | `cargo clippy --workspace --all-targets --all-features --locked -- -D warnings` | exit 0 |
| Re-render previews | `cargo run -p termrock-lookbook -- render --out docs/public/component-previews` | exit 0 |
| Preview check | `cargo run -p termrock-lookbook -- check --dir docs/public/component-previews` | exit 0 |
| API report regen (if Plan 003 landed) | see plans/003 Step 1 | diff shows only intended additions |

## Scope

**In scope**:
- `crates/termrock/src/style/mod.rs` (Role additions, Theme builder)
- `crates/termrock/src/widgets/{tabs,action_bar,hint_bar,status_bar,diff,viewport}.rs`
- Lookbook call sites the compiler flags (constructor changes)
- `docs/public/component-previews/` (re-rendered)
- `docs/api/component-contracts.json` only if a contract classification changes (unlikely — theming is not a contract axis)
- `migrations/000N-*.md` + `MIGRATING.md`

**Out of scope**:
- Deleting/moving the raw color constants — Plan 009 (do not purge yet; the phosphor Theme still reads them).
- The already-role-driven widgets.
- `Backdrop` in `dialog.rs` (uses `DIALOG_BACKDROP = Color::Reset` — that is a deliberate terminal-default-background decision documented in style/mod.rs comments; not a theming bypass).
- Second theme preset — Plan 010.

## Git workflow

- Directly on `main`; `git commit -s -m "feat(style)!: constructible theme with full role coverage"`; migration file + re-rendered previews in the same commit.

## Steps

### Step 1: Extend `Role` with the missing semantic slots

Append variants (append-only — existing `as usize` indices must not shift): `TabActive`, `TabInactive`, `TabActiveHovered`, `TabInactiveHovered`, `TabUnderlineFocused`, `TabUnderlineUnfocused`, `HintKey`, `HintText`, `HintDim`, `HintSeparator`, `ActionFocused`, `ActionDisabled`, `StatusBar`, `DiffAdded`, `DiffRemoved`. Update `Theme.roles` to `[Style; 37]` and extend `tailrocks_phosphor()` positionally with the styles currently hardcoded at the bypass sites:

- `TabActive` = `Style::new().fg(WHITE).bg(TAB_BG_ACTIVE)`, hovered variants likewise; `TabUnderlineFocused` = `GREEN`; `TabUnderlineUnfocused` = `Style::new().fg(WHITE)`.
- `HintKey` = `Style::new().fg(WHITE).add_modifier(Modifier::BOLD)`; `HintText` = `GREEN`; `HintDim` = `DIM`; `HintSeparator` = `Style::new().fg(BORDER_GRAY)`.
- `ActionFocused` = `Style::new().reversed()`; `ActionDisabled` = `Style::new().dim()`.
- `StatusBar` = the default `StatusBar.style` observed in lookbook stories (grep `StatusBar {` in `stories.rs` for the current value; if none, `Style::new()`).
- `DiffAdded` = `Style::new().fg(DIFF_ADDED_FG).bg(DIFF_ADDED_BG)`; `DiffRemoved` mirror.

Keep bold/underline modifiers applied *by the widget* for state (selected/hovered) where they are structural non-color cues, not palette.

**Verify**: `cargo test --workspace --all-features --locked` → all pass (nothing consumes new roles yet).

### Step 2: Add the Theme builder

In `style/mod.rs`:

```rust
impl Theme {
    /// Start from an existing theme (usually the default) and override roles.
    #[must_use]
    pub fn with_role(mut self, role: Role, style: Style) -> Self {
        self.roles[role as usize] = style;
        self
    }

    /// Build a theme by answering every role from a function.
    #[must_use]
    pub fn from_fn(f: impl Fn(Role) -> Style) -> Self { /* iterate all roles */ }

    /// All roles, in stable order (for `from_fn` and introspection).
    #[must_use]
    pub const fn roles() -> [Role; 37] { /* explicit array */ }
}
```

`from_fn` + `roles()` requires an explicit const array of all variants — write it out; add a unit test asserting `Theme::roles().len() == 37` and that `roles()[i] as usize == i` for all i (guards the positional-array invariant permanently).

**Verify**: `cargo test -p termrock style` → new tests pass.

### Step 3: Thread `Theme` through the six widgets

For each, add a `theme: &'a Theme` field (matching the existing pattern — see `list.rs`'s `List { rows, theme }` shape) and replace hardcoded styles with `self.theme.style(Role::…)`:

- `Tabs`: add `theme`; background/underline selection via the six Tab roles.
- `HintBar`/`styled_hint_spans`: change signature to take `theme: &Theme` (keep the `remap` closure — it serves the alpha-fade path; apply remap to the role-derived colors).
- `ActionBar`: keep the per-action `style: Option<Style>` escape hatch; fallbacks come from `ActionFocused`/`ActionDisabled`.
- `StatusBar`: replace `pub style: Style` with `theme` + keep `alpha`; base style = `Role::StatusBar`. Keep `StatusSlot`'s per-slot style/hover-style fields (genuine one-off escapes).
- `DiffView`: replace `added_style`/`removed_style` fields with `theme`; styles from `DiffAdded`/`DiffRemoved`.
- `Viewport`: replace the five chrome style fields with `theme` (content/border/title/scroll-track/scroll-thumb map to `Text`/`Border`/`Text`+BOLD/`ScrollTrack`/`ScrollThumb`); keep `content_style` as an optional override only if a lookbook story requires it (check `stories.rs` usage first — prefer deleting per forward-only design).

Fix all compiler-flagged call sites (lookbook stories/interactors, widget tests).

**Verify**: `cargo test --workspace --all-features --locked` → all pass. Some render tests assert exact colors (e.g. `widgets/tests.rs` asserts `PHOSPHOR_GREEN` on a tab underline) — they must STILL pass, because the phosphor theme reproduces the same styles. A color-assertion failure means a role got mis-mapped: fix the mapping, not the test.

### Step 4: Re-render previews + migration file

```
cargo run -p termrock-lookbook -- render --out docs/public/component-previews
cargo run -p termrock-lookbook -- check --dir docs/public/component-previews
git diff --stat docs/public/component-previews
```

Expected: **zero SVG diffs** (identical styles through a new indirection). Any diff = a mis-mapped role — investigate before committing. Write the next-numbered migration file (removed: `StatusBar.style`, `DiffView.added_style/removed_style`, `Viewport` style fields, `styled_hint_spans` old signature; replacement: `theme` + roles table; before/after example for one widget), link from `MIGRATING.md`.

**Verify**: preview check exits 0 with no diffs; migration indexed.

## Test plan

- New: `style` tests from Step 2 (roles-array invariant), plus one behavioral test: build `Theme::default().with_role(Role::TabActive, Style::new().bg(Color::Blue))`, render `Tabs`, assert the active tab cell bg is blue (proves the override path end-to-end). Put it in `widgets/tests.rs` following its existing buffer-assertion style (e.g. the test asserting `buffer[(0,0)].symbol() == "┌"`).
- Existing render tests + lookbook SVG determinism = regression net.

## Done criteria

- [ ] `grep -n "TAB_BG_\|PHOSPHOR_GREEN\|PHOSPHOR_DIM\|BORDER_GRAY\|WHITE" crates/termrock/src/widgets/tabs.rs crates/termrock/src/widgets/hint_bar.rs` → no matches (constants no longer referenced by widget bodies)
- [ ] All 6 widgets have a `theme` field; `grep -L "Theme" crates/termrock/src/widgets/*.rs` lists only `mod.rs`/`tests.rs` (and files with no styling)
- [ ] `Theme::with_role`/`from_fn`/`roles()` public and tested
- [ ] `cargo test --workspace --all-features --locked` → all pass
- [ ] `cargo run -p termrock-lookbook -- check --dir docs/public/component-previews` → exit 0, zero SVG changes
- [ ] Migration file exists and is indexed
- [ ] `plans/README.md` status row updated

## STOP conditions

- Preview SVGs diff after Step 3 despite phosphor mappings — a role mapping is wrong; report which widget/story if you cannot locate it within two attempts.
- The `remap`-closure interaction in `styled_hint_spans` (alpha fading) conflicts with role indirection in a way that changes StatusBar fade rendering — report with the before/after cell values.
- `Role as usize` is used anywhere outside `style/mod.rs` (grep first) — extra coupling to the array layout must be reported before extending the array.

## Maintenance notes

- Every future widget MUST take `theme: &Theme` and style exclusively via roles — reviewers should reject raw `crate::style::` constants in widget bodies (Plan 009 makes most of them non-public anyway).
- Adding a `Role` variant is append-only until/unless the array-position invariant is replaced with an index-independent map.
- Plan 009 (constant purge) and Plan 010 (second preset) build directly on this.
