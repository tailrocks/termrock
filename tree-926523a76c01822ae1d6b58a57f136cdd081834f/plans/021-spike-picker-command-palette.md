# Plan 021 (spike): Design the `Picker` — the filterable list / command-palette composition every consumer rebuilds

> **Executor instructions**: DESIGN SPIKE with a working prototype story.
> Deliverable = design doc + prototype + recommendation. Honor STOP
> conditions. Update the plans/README.md row when done.
>
> **Drift check (run first)**: `git diff --stat da54a03..HEAD -- crates/termrock/src/widgets/list.rs crates/termrock/src/widgets/text_input.rs crates/termrock-lookbook/src/stories.rs`
> Plans 011/013 reshaped these APIs — design against the CURRENT signatures;
> read them as you go.

## Status

- **Priority**: P3
- **Effort**: M (coarse — spike scope)
- **Risk**: LOW-MED (additive widget; main risk = ownership-boundary creep)
- **Depends on**: plans/011-event-model-convergence.md (design against the final event contract)
- **Category**: direction
- **Planned at**: commit `da54a03`, 2026-07-16

## Why this matters

The fuzzy picker / command palette is the canonical TUI capability, and TermRock already demonstrates the seam by hand: the lookbook registers story `text-input/filter` literally titled "Filter composition" — a `TextInput` wired to a filtered `List` by consumer glue. `TextInput` ships a `placeholder` used as "Type to filter" in both stories and interactors. Every terminal app re-derives this wiring: query state → filter projection → selection reconciliation as the visible set shrinks/grows → activate. The mechanics (query editing, selection stability across re-filters, viewport, empty-state) are product-neutral and belong in TermRock per AGENTS.md ("Assume a visual or interaction pattern belongs in TermRock unless it is provably specific to a consumer's product domain"); the *matching policy* (fuzzy scoring, ranking) is consumer-owned. The design problem is drawing exactly that line.

## Current state

- Evidence of the hand-rolled pattern: `crates/termrock-lookbook/src/lib.rs` (~lines 52-54) registers `text-input/filter` "Filter composition" (component `TextInput`); `stories.rs` (~560) and `interactors.rs` (~161) use placeholder "Type to filter". Read the interactor — it IS the reference wiring this spike absorbs (query edit → filter rows → render List).
- Building blocks already canonical:
  - `TextInputState` — validated editing, cursor, placeholder (`text_input.rs`; post-011 `handle_key(&mut self, key) -> TextInputOutcome`).
  - `ListState<Id>` — selection/hover/viewport/regions, plus the indexed-picker methods migration 0002 introduced: `ListState::<usize>::for_count`, `cycle_index`, `move_index`, `reconcile_count`, `selected_item` — reconciliation across count changes is EXACTLY the shrinking-filter problem, already solved for the index-addressed case.
- Ownership doctrine to honor (COMPONENTS.md): "callers retain hierarchy, filtering, lazy loading" (Tree) / "Consumers own labels, validation, filtering, lifecycle" — filtering stays caller-owned; the picker owns query mechanics + selection reconciliation + layout.
- Post-011 event contract: state-owned `handle_key(data, key)`, `Outcome` vocabulary.

## Commands you will need

| Purpose | Command | Expected on success |
|---|---|---|
| Tests | `cargo test --workspace --all-features --locked` | all pass |
| Gallery | `cargo run -p termrock-lookbook` | prototype story interactive |
| Previews | `cargo run -p termrock-lookbook -- check --dir docs/public/component-previews` | exit 0 |

## Scope

**In scope (spike)**:
- Design doc `plans/021-picker-design.md`
- Prototype `PickerState` + composition INSIDE the lookbook first (replacing the hand-rolled `text-input/filter` interactor wiring) — library-crate placement only in the follow-up build plan
- API definition for the build plan

**Out of scope**:
- Fuzzy-matching algorithms (consumer-supplied predicate/projection — hard boundary).
- Overlay/modal palette chrome (compose with existing `Dialog`/`Backdrop`; note the composition, don't build new chrome).
- Async/streamed candidate sources (design note only).

## Steps

### Step 1: Specify the contract

In the doc, define (sketch — refine against real signatures):

```rust
pub struct PickerState<Id> {
    pub query: TextInputState,
    pub list: ListState<Id>,
    // focus: which sub-widget receives keys (query vs list)? DECIDE: the
    // classic palette gives arrows/Enter to the list and text to the query
    // simultaneously — no explicit focus toggle. Evaluate against dialog.rs's
    // key routing.
}
impl<Id: Clone + PartialEq> PickerState<Id> {
    /// `visible`: the caller-filtered, caller-ordered projection for THIS frame.
    pub fn handle_key(&mut self, visible: &[ListRow<'_, Id>], key: KeyEvent) -> PickerOutcome<Id>;
    pub fn query_text(&self) -> &str;
    /// Call after re-filtering: keeps selection on the same Id when it
    /// survived the filter, else nearest index (reuse ListState reconciliation).
    pub fn reconcile(&mut self, visible: &[ListRow<'_, Id>]);
}
pub enum PickerOutcome<Id> { Ignored, QueryChanged, Activated(Id), Cancelled }
```

Answer explicitly: key-routing table (chars/Backspace/Home/End → query; Up/Down/PageUp/PageDown → list; Enter → activate; Esc → cancel-or-clear-query two-stage? pin it); selection stability semantics (Id-sticky, index-fallback — write the exact rule); empty-result rendering (reserve a "no matches" line? caller-supplied?); does a `Picker` *widget* (rendering query band + list + count) ship, or only the state + a documented composition? Recommendation: ship both state and a default widget (strong defaults doctrine), overridable layout later.

### Step 2: Prototype in the lookbook

Rebuild the `text-input/filter` story/interactor on `PickerState` (locally in the lookbook crate). The story must exercise: typing narrows rows (caller filter = simple `contains`), selection sticks to a surviving Id, selection falls to nearest when its row is filtered out, Enter yields the activated Id, Esc behavior as pinned.

**Verify**: interactor works in the gallery; `cargo test -p termrock-lookbook` (add prototype tests for the reconciliation rules — table-driven, ~5 cases); previews unchanged unless the story's default render intentionally changed (if so, re-render goldens in the same commit and say why).

### Step 3: Design doc + build-plan stub

`plans/021-picker-design.md`: the contract from Step 1 with the routing table and stability rule as normative text; prototype findings (what the lookbook wiring revealed); the library placement plan (new `widgets/picker.rs`, story ids, contract-matrix row, preview — remember AGENTS.md requires inventory/contract/story/preview for every public widget in the same change); open questions (multi-select? preview pane à la telescope? both deferred — record as future options with evidence needs).

**Verify**: doc exists; README row updated.

## Done criteria

- [ ] `plans/021-picker-design.md` with normative routing table + stability rule + placement plan
- [ ] Lookbook `text-input/filter` runs on the prototype `PickerState`; ≥5 reconciliation tests pass
- [ ] `cargo test --workspace --all-features --locked` green; previews consistent
- [ ] No `crates/termrock/src/` changes (library lands in the build plan)
- [ ] `plans/README.md` status row updated

## STOP conditions

- Plan 011 not landed — the contract would be written against dead signatures; stop.
- Simultaneous key routing (query + list without focus toggle) conflicts with the post-011 contract in a way that needs a new event concept — that's a real design finding; document and stop prototyping past it.
- The prototype needs `ListState` internals not publicly reachable — list the missing accessors in the doc (they become part of the build plan), don't hack visibility.

## Maintenance notes

- The follow-up build plan must ship widget + story + contract row + preview together (catalog gate enforces most of this once `public-api.txt` regenerates).
- Matching/scoring stays consumer-owned FOREVER per the ownership doctrine — the doc should quote COMPONENTS.md's "callers retain … filtering" line at the top so future contributors don't absorb a fuzzy-scorer "for convenience".
