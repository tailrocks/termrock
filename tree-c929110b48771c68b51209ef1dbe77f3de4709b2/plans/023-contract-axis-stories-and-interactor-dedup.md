# Plan 023: Make the contract axes real — narrow/unicode story variants — and deduplicate interactor plumbing

> **Executor instructions**: Follow this plan step by step. Run every
> verification command and confirm the expected result before moving to the
> next step. If anything in the "STOP conditions" section occurs, stop and
> report — do not improvise. When done, update the status row for this plan
> in `plans/README.md`.
>
> **Drift check (run first)**: `git diff --stat c51e11c..HEAD -- crates/termrock-lookbook/src/ docs/api/component-contracts.json docs/scripts/check-catalog.ts`
> Plan 022 legitimately touches svg.rs/main.rs/lib.rs — verify its row, read
> live code. Other structural surprises = STOP.

## Status

- **Priority**: P2
- **Effort**: M
- **Risk**: LOW-MED (new stories enlarge the golden set; interactor extraction is behavior-preserving)
- **Depends on**: plans/022-lookbook-svg-and-pipeline-correctness.md (regenerate goldens once, on fixed colors)
- **Category**: tests
- **Planned at**: commit `c51e11c`, 2026-07-16

## Why this matters

`docs/api/component-contracts.json` claims `narrowTerminal: "covered"` for nearly every component and `unicode: "covered"` for most — but the gate (`check-catalog.ts`) only validates enum values and preview-file existence, and every story renders once at a single generous fixed size. Only two stories contain any non-ASCII; none render narrow; none exercise a monochrome path. The "covered" claims are self-attested, decoupled from any render. A component can regress its narrow/unicode behavior with everything green. Separately, the interactors hand-copy a ~30-line mouse-plumbing block three times (List/Tree/Form), and split-pane magic bounds `12`/`16` appear in three places — story and interactor can silently disagree.

## Current state

- Contract file shape (`docs/api/component-contracts.json`):

```json
"ActionBar": { "keyboard": "caller-owned", "mouse": "covered", "focus": "covered", "nonColor": "covered", "unicode": "covered", "narrowTerminal": "covered" },
```

- Gate (`docs/scripts/check-catalog.ts:34-45`): validates each axis ∈ {covered, caller-owned, not-applicable}; requires each story id mentioned in some MDX and a preview SVG to exist. Nothing ties an axis claim to a story.
- Stories (`crates/termrock-lookbook/src/stories.rs` ~109-280): `Story::new(id, title, component, description, width, height, render_fn)` — e.g. `("panel/focused", ..., 48, 7, panel)`. All sizes generous; unicode only in `tree` ("Wide 🧪 notes") and `viewport` ("gamma: 🧪 Unicode").
- Interactor duplication (`crates/termrock-lookbook/src/interactors.rs`): the block below appears near-identically at ~113-139 (List), ~214-243 (Tree), ~282-311 (Form):

```rust
    fn handle_mouse(&mut self, mouse: MouseEvent, preview_area: Rect) -> bool {
        let position = ratatui::layout::Position::new(mouse.column, mouse.row);
        if !preview_area.contains(position) {
            let changed = self.state.hovered.is_some();
            self.state.hover(position);
            return changed;
        }
        match mouse.kind {
            crossterm::event::MouseEventKind::Moved => { self.state.hover(position); true }
            crossterm::event::MouseEventKind::Down(crossterm::event::MouseButton::Left) => { ... click ... }
            crossterm::event::MouseEventKind::Drag(...) => { ... scroll_to_position ... }
            crossterm::event::MouseEventKind::ScrollUp => { self.state.scroll_by(-1, ...) }
            crossterm::event::MouseEventKind::ScrollDown => { self.state.scroll_by(1, ...) }
            _ => false,
        }
    }
```

- Split-pane magic: `SplitPane::new(SplitDirection::Horizontal, 12, 16, ...)` at `interactors.rs:334`, `:343`, and `stories.rs:417`.
- `FormInteractor::handle_key` rebuilds `form_fields()`+sections per keystroke (~271-279) mirroring `render` (~262-267).
- Catalog counting: `catalog.test.ts` (if still present — Plan 027 may wire or delete it) asserts "18 public components with 18 stories"; adding VARIANT stories for existing components keeps component count but raises story count — check how `check-catalog.ts` treats multiple stories per component (it builds a Set of story components; extra stories per component are fine as long as each id appears in MDX and has a preview).
- Repo conventions: every new story needs its id mentioned in `docs/content/docs/components.mdx` (the gate enforces `docs.includes(story.id)`) and a committed preview SVG; Conventional Commits + DCO.

## Commands you will need

| Purpose | Command | Expected on success |
|---|---|---|
| Lookbook tests | `cargo test -p termrock-lookbook` | all pass |
| Render + check | `cargo run -p termrock-lookbook -- render --out docs/public/component-previews && cargo run -p termrock-lookbook -- check --dir docs/public/component-previews` | exit 0 |
| Catalog gate | `cd docs && bun run build` | exit 0 |
| Workspace | `cargo test --workspace --all-features --locked` | all pass |

## Scope

**In scope**:
- `crates/termrock-lookbook/src/{stories.rs, interactors.rs}`
- `docs/content/docs/components.mdx` (story-id mentions for new variants)
- `docs/public/component-previews/` (new goldens)
- `docs/scripts/check-catalog.ts` (Step 3's axis-story linkage — additive)

**Out of scope**:
- Widget source changes — if a narrow/unicode story reveals a rendering bug, capture the SVG as-is, mark the story description "documents current behavior", and report; the fix is separate work.
- The monochrome/non-color axis: the library is truecolor-only by decree (README: no reduced-color path); `nonColor` in the contract means non-color CUES (markers like `*`/`⊘`/underline), which existing stories already show. Do not build a monochrome render mode. Relabel nothing.
- Knobs/theme switching (Plan 020).

## Git workflow

- Directly on `main`; `git commit -s -m "test(lookbook): exercise narrow and unicode contract axes in stories"` + `refactor(lookbook): share interactor pointer plumbing`.

## Steps

### Step 1: Extract the shared pointer plumbing

In `interactors.rs`, define one helper the three copies delegate to. The three widget states share the method names (`hover`, `click`, `scroll_by`, `scroll_to_position` — verify Form's exact set; Tree/List confirmed) but not a trait — introduce a small internal trait:

```rust
trait PointerTarget {
    fn hover_at(&mut self, position: Position) -> bool;      // returns changed
    fn click_at(&mut self, position: Position) -> bool;      // returns handled
    fn drag_to(&mut self, position: Position) -> bool;
    fn wheel(&mut self, delta: isize) -> bool;
}
fn route_pointer(target: &mut impl PointerTarget, mouse: MouseEvent, preview_area: Rect) -> bool { /* the shared block */ }
```

Implement `PointerTarget` per interactor (each impl is 4 one-line delegations closing over its rows/len). Also hoist the split-pane bounds: `const SPLIT_PANE_MIN: u16 = 12; const SPLIT_PANE_MAX: u16 = 16;` in one place (stories.rs, `pub(crate)`) used by all three sites. Fix the Form fixture rebuild: build fields/sections once per interactor construction where the borrow structure allows; if borrowed lifetimes force per-call rebuild, keep it and note why in a comment.

**Verify**: `cargo test -p termrock-lookbook` → the 2 existing interactor tests still pass; manual gallery spot-check optional. `grep -c "MouseEventKind::ScrollUp" crates/termrock-lookbook/src/interactors.rs` → 1.

### Step 2: Add narrow + unicode story variants

For each component whose contract claims `narrowTerminal: "covered"`, decide whether the EXISTING story demonstrates it (none do — sizes are generous). Add variants for the highest-value set (don't do all 18 blindly — pick the components where narrow behavior is nontrivial): `list/narrow` (width 14), `tabs/narrow` (width 16 — clipping + hit regions), `form/narrow` (width 24 — single-column collapse, the responsive behavior COMPONENTS.md documents), `status-bar/narrow` (width 20 — slot elision), `dialog/narrow` (width 20), `toast/narrow` (width 16). For unicode: `list/unicode` (CJK + emoji + combining-mark rows), `text-input/unicode` (wide-char value with cursor mid-string), `detail-table/unicode` (CJK labels, emoji values — exercises `display_cols_slice` wrap).

Each variant: `Story::new("component/axis-variant", ...)` + one-line mention in `components.mdx` (the gate requires the id in backticks) + rendered preview.

**Verify**: `render` + `check` green; `cd docs && bun run build` → exit 0 (all new ids mentioned + previews exist); visually spot-open two new SVGs (with Plan 022's fix, content must be visible).

### Step 3: Tie axis claims to stories in the gate

Extend `check-catalog.ts` (additive): for each component whose contract says `narrowTerminal: "covered"`, require at least one story whose id matches `/narrow/` OR the component appears in an explicit allowlist `NARROW_EXEMPT` (seeded with the components whose base story is already narrow-safe by construction — e.g. `Backdrop`); same for `unicode`. Keep it a warning-free hard error like the checker's other rules. This makes the self-attestation structural.

**Verify**: `cd docs && bun run build` → exit 0 with the new rule active; temporarily remove one `narrow` story id locally → build fails with the new error → restore.

## Test plan

- Step 1: existing interactor tests as the behavior net + the grep-count done-criterion.
- Step 2: new stories ARE tests (deterministic goldens); the render/check/determinism gates validate them.
- Step 3: negative test performed manually (gate fails when a required story is removed) — state the observed error in your report.

## Done criteria

- [x] One `route_pointer` implementation; duplicated mouse blocks gone
- [x] `SPLIT_PANE_MIN/MAX` constants; `grep -rn "12, 16" crates/termrock-lookbook/src/` → no raw-magic hits at the three sites
- [x] ≥6 narrow + ≥3 unicode story variants committed with previews and MDX mentions
- [x] `check-catalog.ts` enforces axis-story linkage (with documented exempt list)
- [x] All gates green (lookbook tests, render/check, determinism, bun build, workspace tests)
- [x] `plans/README.md` status row updated

## STOP conditions

- A narrow/unicode story render PANICS — that's a library bug (geometry math): report widget + size + input immediately; do not commit a panicking story.
- A narrow render is visually broken (overlap, garbage) — commit is allowed ONLY with the "documents current behavior" description + a report entry; if it's egregious (unreadable), report first.
- `check-catalog.ts`'s story-component model can't express variants (e.g. duplicate-component stories rejected) — read the checker; it builds Sets so duplicates are fine; if reality differs, report.

## Maintenance notes

- New-widget checklist gains: narrow + unicode variant stories when claiming those axes (the gate now enforces it).
- Plan 020's knobs may later subsume static variants (a width knob replaces `x/narrow` stories) — fine; keep the gate rule pointed at whatever demonstrates the axis.
- The `PointerTarget` trait is lookbook-internal; if Plan 011's neutral events land first, `route_pointer` should consume `input::MouseEvent` instead of crossterm's — small adaptation, note it.
