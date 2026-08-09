# Plan 041: Build a variable-height, streaming transcript engine

> **Executor instructions**: Execute sequentially. Verify every step. STOP on
> semantic drift; do not keep one-row transcript APIs as compatibility paths.
>
> **Drift check (run first)**:
> `rtk git diff --stat 16b0ee8..HEAD -- crates/termrock/src/widgets/agent.rs crates/termrock/src/widgets/markdown.rs crates/termrock/src/widgets/code_block.rs crates/termrock/src/widgets/diff.rs crates/termrock/src/widgets/text_area.rs crates/termrock/src/widgets/text_input.rs crates/termrock/src/text crates/termrock/tests crates/termrock-lookbook docs/api docs/content/docs MIGRATING.md migrations`
>
> Compare changed files against this plan. Begin only after Plan 040 is DONE
> and the repository gate is green.

## Status

- **Priority**: P1
- **Effort**: L
- **Risk**: HIGH
- **Depends on**: Plan 040
- **Category**: feature, architecture, performance, UX, Unicode
- **Planned at**: commit `16b0ee8`, 2026-08-09
- **Execution**: DONE — Transcript engine (migration 0035)

## Why this matters

AI-agent TUIs feel exceptional when long, streaming, mixed-content sessions
remain anchored, searchable, foldable, and responsive. TermRock's current
stream view treats every item as one row, ignores its stable ID, and cannot
virtualize markdown/code/diff/tool blocks of variable height. Consumers would
have to rebuild the hardest part of an Amp/Grok-style experience themselves.

## Current state

- `widgets/agent.rs` renders stream items as one clipped line and scrolls by
  item index. `StreamItem::id` is not the anchor used by state.
- MarkdownView and CodeBlock provide useful presentation primitives but no
  shared measurement, folding, search, or viewport contract.
- ToolCard/thinking/approval content has separate local rendering contracts.
- resize, streamed append, and folding can move the viewport because state has
  no `(stable block id, intra-block display row)` anchor.
- text clipping helpers split at Unicode scalar/display-column boundaries and
  can cut extended grapheme clusters.
- PromptBox accepts only Press and documents Alt+Enter insertion, while the
  implementation forwards a different contract; held navigation and paste are
  inconsistent across text widgets.
- Effects, model transport, parser policy, token accounting, and transcript
  wording remain consumer-owned.
- This plan owns migration `0034` after Plans 039–040 claim `0032–0033`.

## Target contract

Introduce a product-neutral transcript surface composed from borrowed blocks:

- `TranscriptBlock` exposes stable ID, revision, semantic kind, measurement at
  width/fold state, clipped rendering, and plain-text projection.
- `TranscriptState<Id>` owns selection, display-row viewport, stable anchor,
  follow mode, folds, search cursor, and bounded measurement cache.
- Rendering visits only blocks intersecting the viewport plus small overscan.
- append/upsert/reconcile invalidates only changed measurements.
- resize, fold, and streaming updates preserve the stable visual anchor.
- interactions return neutral outcomes: activate, fold, copy projection,
  selection changed, follow changed, or search match changed.

Prefer a generic borrowed slice or consumer enum implementation. Do not require
boxed trait objects, cloned transcript strings, or library-owned model storage.

## Scope

**In scope**:

- new transcript module under `crates/termrock/src/widgets/`;
- StreamView, MarkdownView, CodeBlock, Diff, ToolCard, thinking and approval
  adapters/refactors needed for one block contract;
- shared Unicode clipping and text-input event/paste correctness;
- InteractionScene registration and typed transcript actions;
- hot-path/allocation tests, lookbook stories/scripts, component docs,
  generated inventory, migration `0034`.

**Out of scope**:

- LLM/network/runtime integration, process execution, persistence, or secrets;
- mandatory CommonMark parser or syntax highlighter;
- image protocol lifecycle (Plan 044);
- domain-specific message schema or branded agent wording;
- compatibility aliases for one-row StreamView.

## Git workflow

- Work directly on `main`; STOP otherwise.
- Conventional Commit, `rtk git commit -s`, plus
  `Co-authored-by: Codex <codex@openai.com>`.
- Each commit independently passes `rtk proxy mise run check`; push only after
  `rtk proxy mise run gate`.
- If staged implementation needs multiple commits, land private foundations
  first without exporting incomplete public APIs; the breaking public commit
  must include migration/docs/catalog.

## Steps

### Step 1: Define behavior with a reference layout model

Create a test-only slow reference model and fixture blocks with controlled
heights. Lock these invariants:

1. zero blocks and zero-area renders are no-ops;
2. viewport maps display rows across variable-height block boundaries;
3. known stable anchor survives append above/below, reorder, width resize, and
   fold/unfold;
4. follow mode stays at tail during streaming; manual upward scroll detaches;
5. explicit End/Follow reattaches without jumping beyond content;
6. removing the anchor reconciles to nearest surviving stable block;
7. search skips folded/hidden content only according to one documented rule;
8. disabled/non-interactive blocks never activate;
9. clipped rendering never splits an extended grapheme cluster;
10. visible output from optimized and reference models matches across random
    block heights, widths, folds, and scroll operations.

Use deterministic seeded cases if no property framework exists.

**Verify**: focused transcript tests fail because the optimized engine does not
yet exist; existing termrock tests remain green.

### Step 2: Implement stable measurement and viewport indexing

Define a public block projection with these concepts (names may adjust):

- stable `Id: Eq` supplied by caller;
- monotonically meaningful `revision`/cache key supplied by caller;
- product-neutral `TranscriptKind` for semantics and default recipes;
- `measure(width, fold) -> DisplayRows`;
- `render_rows(frame, area, row_range, context)`;
- `write_plain_text(&mut impl fmt::Write)` for copy/search without mandatory
  intermediate allocation;
- optional fold/search/activate capability flags.

Build an indexed height structure or prefix-sum cache that supports viewport
lookup without scanning every historical block each frame. Invalidation is by
ID + revision + width + fold state. Bound or compact stale cache entries when
blocks disappear. No unsafe code is needed.

Choose/document complexity targets:

- append/upsert: amortized logarithmic or localized rebuild;
- display-row lookup: logarithmic;
- render: proportional to visible blocks/rows;
- warmed unchanged frame: zero allocations.

Do not cache rendered `Line` clones for the entire history.

### Step 3: Implement state, anchor, follow, folds, and search

`TranscriptState` must own only interaction state:

- selected stable block and optional intra-block range;
- top anchor `(Id, display_row_within_block)` plus visual offset;
- follow state with explicit detached/attached outcomes;
- fold state keyed by stable ID;
- query/search cursor and match navigation state;
- measurement/index scratch with documented bounds.

Register block/part geometry and available actions with InteractionScene.
Pointer and keyboard use the same stable identities. Selection/copy outcomes
carry IDs/ranges, not cloned domain objects. Consumers perform clipboard and
search-index effects.

Add sticky user-prompt/block headers only as an optional generic block-header
policy. Avoid assistant/product-specific semantics.

### Step 4: Compose rich standard blocks

Provide adapters or standard block parts for:

- plain/styled text;
- MarkdownView;
- CodeBlock with horizontal clipping and fold state;
- unified/split Diff as width permits;
- tool/activity card and progressive disclosure;
- thinking/details block;
- ApprovalCard from Plan 039.

One transcript may mix these without consumer-specific render switching. Keep
parsing and syntax tokenization projected by callers or optional features.
Replace the one-row StreamView public path rather than retaining duplicate
render bodies.

### Step 5: Repair shared Unicode and input foundations

Replace column truncation/fixed-prefix helpers with extended-grapheme-safe
iteration while preserving display-width bounds. Test combining marks, ZWJ
emoji, flags, skin tones, wide CJK, zero-width sequences, and width 0/1.

Unify text event policy:

- navigation/edit Repeat is accepted; Release ignored;
- submission is Press-only;
- use the binding component contract consistently: plain Enter submits;
  Alt+Enter and Ctrl+Enter insert newline when decoded by the backend; Shift+
  Enter has one explicitly tested policy rather than accidental delegation;
- batch paste as one edit/undo operation without per-character quadratic work;
- malformed/unsupported chords return ignored, never mutate draft.

This is a structural prerequisite for transcript search/composition, not an
unrelated cleanup.

### Step 6: Prove scale and scripted experience

Add a benchmark-style test with at least 10,000 mixed-height blocks and a
40-row viewport. After warmup, unchanged render/route must allocate zero and
visit only the visible neighborhood. Test append streaming, resize invalidation,
search stepping, fold, and follow detach/rejoin.

Lookbook scripts must demonstrate:

- streamed delta while following;
- manual scroll detaches, End rejoins;
- folded tool/code content;
- search next/previous;
- selection/copy outcome;
- widths 20/40/80/120 with Unicode content.

Record deterministic visible output and semantic traces using stable IDs.

### Step 7: Document migration and validate

Write `migrations/0034-v0.12.0-transcript-engine.md` covering removed
StreamView API, block projection, state ownership, cache revision rules,
before/after consumer examples, input contract, and commands. Update
`MIGRATING.md`, components, catalog contracts, stories/previews, and API
inventory.

**Verify**:

- `rtk cargo test -p termrock transcript --all-features --locked` → pass.
- `rtk cargo test -p termrock --test table_hot_path --all-features --locked`
  and the new transcript hot-path target → pass.
- `rtk cargo run -p termrock-lookbook -- check --dir docs/public/component-previews` → pass.
- `rtk proxy mise run check` and `rtk proxy mise run gate` → both exit 0.

## Test plan

- Reference-vs-index model tests over seeded block operations.
- Unit tests for anchor reconciliation, follow, fold, search, and ID stability.
- Render tests for every standard block and narrow breakpoint.
- Grapheme safety and paste/input-event tests.
- 10k-block allocation/complexity regression test.
- Lookbook deterministic scripts and contract evidence.

## Done criteria

- [ ] Variable-height blocks virtualize by display row.
- [ ] Stable anchors survive append, resize, fold, reorder, and removal.
- [ ] Follow mode detaches/rejoins predictably.
- [ ] Search, fold, activate, selection, and copy are neutral typed outcomes.
- [ ] Standard markdown/code/diff/tool/thinking/approval blocks compose.
- [ ] Unicode clipping preserves extended grapheme clusters.
- [ ] Text Repeat/Release/paste/submit behavior is coherent.
- [ ] 10k-block warmed viewport meets zero-allocation/local-work target.
- [ ] Migration `0034`, docs, scripts, contracts, previews, and inventory are
      fresh; old one-row public path is gone.
- [ ] Full gates pass.

## STOP conditions

Stop and report if:

- Plan 040 is not DONE, branch is not `main`, tree is dirty, or `0034` is
  claimed.
- Transcript requires TermRock-owned domain messages, network/model state, or
  clipboard/process effects.
- A mandatory parser, decoder, or syntax engine would materially expand the
  default dependency surface; keep projection optional instead.
- Stable IDs/revisions cannot be supplied by the caller projection.
- Performance assertions are nondeterministic after removing terminal timing.
- Any verification fails twice after a reasonable correction.

## Maintenance notes

- Workspace integration in Plan 042 should host Transcript directly; it must
  not wrap it in another scroll truth.
- Preview blocks in Plan 044 must use the same ID/revision/anchor contracts.
- When adding future block kinds, prefer adapters and semantic parts over
  expanding a closed product-specific enum.
