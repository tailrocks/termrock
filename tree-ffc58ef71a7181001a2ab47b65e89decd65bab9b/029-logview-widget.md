# Plan 029: Build `LogView` — the follow-tail, ANSI-colored scrollback widget the primitives already imply

> **Executor instructions**: Follow this plan step by step. Run every
> verification command and confirm the expected result before moving to the
> next step. If anything in the "STOP conditions" section occurs, stop and
> report — do not improvise. When done, update the status row for this plan
> in `plans/README.md`.
>
> **Drift check (run first)**: `git diff --stat c51e11c..HEAD -- crates/termrock/src/scroll/ crates/termrock/src/ansi_text.rs crates/termrock/src/widgets/`
> Written against the POST-011/013 API (state-owned handlers, new-idiom
> construction). Verify those rows; design against LIVE signatures.

## Status

- **Priority**: P3
- **Effort**: M
- **Risk**: LOW-MED (additive widget; ownership-boundary design is the risk)
- **Depends on**: plans/011-event-model-convergence.md, plans/013-construction-idiom-and-widget-traits.md (build once, on the final contract); coordinate with 024 (keep `TailScroll`/`tail_vertical_thumb` — 024 keeps them, they're live-listed)
- **Category**: direction
- **Planned at**: commit `c51e11c`, 2026-07-16

## Why this matters

Every building block for a streaming log/console pane already ships, individually public, wired together by nobody: `TailScroll` ("tail-relative scroll offset… 0 means live tail"), `tail_vertical_thumb` (a thumb function explicitly documented "for tail-relative scrollback surfaces"), `ansi_text::styled_spans` (SGR bytes → styled spans), and `Viewport`'s bordered scroll rendering. Comments in `scroll/mod.rs` reference "the host console panels" — a consumer already hand-composed this. Streaming process output is a near-universal TUI need and central to this ecosystem's heritage. `LogView` absorbs the composition; the consumer keeps the data.

## Current state

- `crates/termrock/src/scroll/mod.rs:36-72` — `TailScroll { offset: usize }` with `scroll_by(filled, delta)`, `clamp(filled)`, `to_top_offset(content_len, viewport_len)` (tail-relative → top-relative conversion). Tested in the scroll suite.
- `scroll/mod.rs` ~583 — `tail_vertical_thumb` ("Full-cell vertical thumb for tail-relative scrollback surfaces").
- `crates/termrock/src/ansi_text.rs` — `styled_spans(input: &str, default_style: Style) -> Vec<Span>` (SGR fg/bg/modifiers incl. 256/truecolor) and `strip_bytes`. Hardened test coverage arrives via Plan 017.
- `widgets/viewport.rs` — the bordered scrollable `&[Line]` renderer (post-015 renders the visible slice; post-008 themed).
- Ownership doctrine (root AGENTS.md / COMPONENTS.md): consumers own the data ("labels, validation, filtering, lifecycle, output"); widgets consume borrowed projections with state types owning only interaction/viewport facts.
- Post-011 contract: state-owned `handle_key(data, key) -> Outcome`, `hover`/`click`/`scroll_by` naming, neutral `MouseEvent`. Post-013: `X::new(required)` + const builders, `#[non_exhaustive]`, owned+ref render impls. New-widget catalog rule (AGENTS.md): inventory + contract row + story + deterministic preview + docs in the same change (Plan 028 adds the page gate).

## Commands you will need

| Purpose | Command | Expected on success |
|---|---|---|
| Tests | `cargo test --workspace --all-features --locked` | all pass |
| Previews | `cargo run -p termrock-lookbook -- render --out docs/public/component-previews && cargo run -p termrock-lookbook -- check --dir docs/public/component-previews` | exit 0 |
| Catalog gate | `cd docs && bun run build` | exit 0 |
| Full gate | `mise run gate` | exit 0 |

## Scope

**In scope**:
- `crates/termrock/src/widgets/log_view.rs` (new) + `widgets/mod.rs` export
- Lookbook: story (`log-view/tail` + a scrolled-back variant) + interactor; preview SVGs
- `docs/api/component-contracts.json` row; `public-api.txt` regen; component page entry (post-028 generator map)
- Hot-path test file (Step 4)

**Out of scope**:
- Owning the line buffer / ring buffer: the CALLER owns lines (borrowed `&[Line]` projection). A managed ring buffer is a possible follow-up utility, not this widget.
- ANSI parsing inside the widget: `LogView` consumes `&[Line]` — the consumer (or a helper constructor, see Step 1) converts bytes via `styled_spans` at ingest time, ONCE per line, not per frame.
- Search/filter inside the log (composes with Picker-style filtering later).
- Timestamps/severity chrome (consumer projection).

## Git workflow

- Directly on `main`; `git commit -s -m "feat(widgets): log view with tail-follow scrollback"` (+ migration file ONLY if any existing surface changes — expected: none, purely additive; the catalog artifacts land in the same commit per AGENTS.md).

## Steps

### Step 1: API design (write it as rustdoc before code)

```rust
pub struct LogView<'a> { /* private: lines: &'a [Line<'a>], theme: &'a Theme, title: Option<&'a str>, wrap: bool(?) */ }
impl<'a> LogView<'a> {
    pub const fn new(lines: &'a [Line<'a>], theme: &'a Theme) -> Self;
    pub const fn title(self, t: &'a str) -> Self;
}
pub struct LogViewState {
    tail: TailScroll,           // 0 = following
    // + painted geometry for hit-testing the scrollbar, per DetailTableState precedent
}
impl LogViewState {
    pub fn handle_key(&mut self, lines_len: usize, key: KeyEvent) -> Outcome<()>;  // Up/Down/PageUp/PageDown/Home(oldest)/End(tail)
    pub fn scroll_by(&mut self, delta: isize, lines_len: usize) -> bool;
    pub fn follow(&mut self);                 // jump to tail / resume following
    pub fn is_following(&self) -> bool;       // tail.offset() == 0
}
```

Decisions to pin in rustdoc: (a) tail semantics — appending lines while `is_following()` keeps the view glued to the newest line; while scrolled back, appends do NOT move the view (TailScroll's tail-relative offset gives this for free — that's why it exists); (b) horizontal: start without horizontal scroll (long lines clipped; `wrap` deferred — record as a named non-goal); (c) non-color cue: a "▼ following" / "▲ scrolled (+N)" indicator cell in the border/title area so follow-state is visible without color.

**Verify**: design compiles as stub with `todo!()` bodies; `cargo check -p termrock` green.

### Step 2: Implement render + interaction

Render: visible window = `tail.to_top_offset(lines.len(), viewport_h)` slice (O(viewport) — the Plan-015 rule), `Panel`-style border via theme roles, `tail_vertical_thumb` scrollbar, follow indicator. Interaction per the Step-1 signatures; wheel = `scroll_by(±1)`; any upward scroll leaves follow mode; `End`/`follow()` re-enters it.

**Verify**: unit tests in the widget file (buffer-assertion style from `widgets/tests.rs`): tail-follow glues to newest after append; scrolled-back view is stable across appends; `End` resumes follow; 0-height/0-width no panic; thumb position at three offsets. ≥6 tests green.

### Step 3: Ingest helper + story

Add a free helper (in `ansi_text` or on the widget, pick `ansi_text`): `pub fn line_from_ansi(bytes: &[u8], default_style: Style) -> Line<'static>` wrapping `styled_spans` — the documented ingest path ("parse once at append time"). Lookbook: story rendering a fixed 30-line colored log at tail + a `log-view/scrolled` variant; interactor wiring wheel/keys. Contract row: keyboard covered, mouse covered, focus caller-owned, nonColor covered (the follow indicator), unicode covered (wide-char log lines in the story), narrowTerminal covered (narrow variant per Plan 023's gate).

**Verify**: previews render (content visible), catalog gate green, determinism green.

### Step 4: Hot path proof

`crates/termrock/tests/log_view_hot_path.rs` modeled on `tree_hot_path.rs` (stats_alloc): 10k borrowed lines, 40-row viewport, 100 warmed renders at tail — assert bounded allocations (O(viewport) clones only) and the batch time budget in the same spirit.

**Verify**: hot-path test green; full `mise run gate` green.

## Test plan

Steps 2 + 4 (~8 tests incl. the allocation proof); lookbook story determinism; catalog/contract gates.

## Done criteria

- [ ] `LogView`/`LogViewState` exported; post-011 signature shapes; post-013 construction
- [ ] Tail-follow semantics tested (glue, stable scrollback, resume)
- [ ] Story + preview + contract row + API report all land in the same change (catalog gate green)
- [ ] Hot-path allocation test green
- [ ] `mise run gate` → exit 0
- [ ] `plans/README.md` status row updated

## STOP conditions

- Plans 011/013 not landed — building on the old contract creates immediate migration debt; stop.
- `TailScroll`'s semantics don't actually give stable-scrollback-under-append (test reveals drift) — that's a scroll-module finding; report before working around it.
- The widget seems to need an owned line buffer to be usable (borrowed projection too awkward in the story) — STOP and report; that's an ownership-doctrine decision for the maintainer, not an implementation detail.

## Maintenance notes

- The ingest helper is the seam a future managed ring-buffer utility would slot behind — keep parse-at-ingest as the documented contract.
- Horizontal scroll / wrap is the recorded non-goal; first consumer demand decides which (wrap likely, given `Viewport` precedent).
- Plan 034's TextArea and this widget both slice-by-viewport — if a shared "line window" helper emerges, extract then, not preemptively.
