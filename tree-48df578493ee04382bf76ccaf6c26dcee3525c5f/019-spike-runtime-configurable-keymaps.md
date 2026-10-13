# Plan 019 (spike): Design runtime-configurable keymaps (user-rebindable keys) without losing the dispatch/hint single-source guarantee

> **Executor instructions**: DESIGN SPIKE. Deliverable = design doc + a
> compile-proven prototype of the chosen ownership model + recommendation.
> Honor STOP conditions. Update the plans/README.md row when done.
>
> **Drift check (run first)**: `git diff --stat da54a03..HEAD -- crates/termrock/src/keymap.rs`
> Plan 006 (vocabulary unification) must be DONE first — this spike designs on
> the unified `KeyCode`/`KeyModifiers` types.

## Status

- **Priority**: P3
- **Effort**: M (coarse — spike scope)
- **Risk**: MED for the eventual build (touches dispatch core); LOW for the spike
- **Depends on**: plans/006-key-vocabulary-unification.md
- **Category**: direction
- **Planned at**: commit `da54a03`, 2026-07-16

## Why this matters

Every keymap in TermRock is compile-time static: `Keymap<A> { bindings: &'static [KeyBinding<A>] }`, `KeyBinding.chords: &'static [KeyChord]`, `hint: Option<&'static str>`, `glyph: Option<&'static str>`, `const fn new(&'static [...])`. A general-purpose TUI library adopted by multiple products will be asked for user-remappable keys and config-loaded binding sets (vim/emacs presets); today a consumer would have to fork the dispatch layer — which also silently forfeits the module's core guarantee that key dispatch and hint-bar advertisement can never drift (they derive from the same table). The design problem: introduce runtime-owned bindings while keeping (a) the const/static zero-cost path for defaults, (b) the dispatch=hints invariant, (c) serde-loadability.

## Current state

- `crates/termrock/src/keymap.rs` (~lines 228-260, verbatim):

```rust
pub struct KeyBinding<A> {
    pub chords: &'static [KeyChord],
    pub action: A,
    pub hint: Option<&'static str>,
    pub visibility: Visibility,
    pub glyph: Option<&'static str>,
}

pub struct Keymap<A: 'static> {
    bindings: &'static [KeyBinding<A>],
}

impl<A: Copy + 'static> Keymap<A> {
    pub const fn new(bindings: &'static [KeyBinding<A>]) -> Self { ... }
    pub fn bindings(&self) -> &[KeyBinding<A>] { ... }
```

- Module doc bills itself as the "single source of truth coupling key dispatch and hint advertisement".
- Binding tables in the wild: `static` tables in `crates/termrock-lookbook/src/main.rs` (~line 62) and keymap's own test tables (~544). Hint derivation: `chord_glyph` + `KeyBinding.glyph` override; `HintBar` renders from the same table.
- Post-006, `KeyChord { key: input::KeyCode, mods: input::KeyModifiers }` — plain `Copy` data, serde-friendly shape.
- `Visibility { Shown, ... }` controls hint-bar advertisement.
- Repo conventions: forward-only redesign preferred over parallel old/new; any accepted design lands as ONE breaking change + migration file.

## Commands you will need

| Purpose | Command | Expected on success |
|---|---|---|
| Keymap tests | `cargo test -p termrock keymap` | all pass |
| Workspace | `cargo test --workspace --all-features --locked` | all pass |

## Scope

**In scope (spike)**:
- Design doc `plans/019-keymap-design.md`
- A branch-local prototype of the chosen model compiled + tested inside `keymap.rs` (may temporarily live behind `#[cfg(test)]` or a private module — no public surface change yet)

**Out of scope**:
- Shipping the redesign (follow-up build plan).
- Key-recording UI, chord-conflict resolution UX (consumer domain; the design should EXPOSE conflict detection, not decide policy).
- serde formats beyond a serde derive proof (which config format is consumer choice).

## Steps

### Step 1: Decide the ownership model — evaluate exactly three candidates

Write up (with sketched Rust for each; compile the winner):

1. **Owned-everything**: `Keymap<A> { bindings: Vec<KeyBinding<A>> }`, `KeyBinding { chords: Vec<KeyChord>, hint: Option<Cow<'static, str>>, ... }`. Statics become `Keymap::from_static(&'static [...])` copying into Vecs at startup. Cost: loses `const` construction; every app pays Vec at init (trivial); simplest single-path model (forward-only friendly).
2. **Cow-layered**: fields become `Cow<'static, [KeyChord]>`/`Cow<'static, str>`; `const fn new` survives for the borrowed variant IF const-constructing `Cow::Borrowed` in const context compiles on MSRV 1.95 (**prototype this first — it's the make-or-break fact**).
3. **Base + override layer**: `Keymap` stays static; a new `KeymapOverrides { remaps: Vec<(A, Vec<KeyChord>)>, disabled: Vec<A> }` merges at dispatch/hint time via a `ResolvedKeymap` view. Keeps statics untouched; two types to reason about; hints must render from the RESOLVED view (invariant preserved by construction).

Evaluation criteria (score each in the doc): dispatch=hints invariant preservation, const-default cost, serde story, API surface added, forward-only cleanliness (AGENTS.md dislikes parallel paths — candidate 3 is two paths; say so honestly).

### Step 2: Prototype the winner

Implement enough to compile and pass: construct a default table, apply a runtime remap (rebind quit from `q` to `Ctrl+C`), dispatch both old (miss) and new (hit) chords, and derive hints reflecting the remap. Add a conflict-detection fn (`fn conflicts(&self) -> Vec<(&A, &A, KeyChord)>` — two actions sharing a chord). Prove serde: `#[cfg_attr(feature = "serde", ...)]` on `KeyChord` + the owned binding type compiles under `--features serde` (Plan 013 adds the feature; if not landed, prove with a local dev-only derive and note it).

**Verify**: prototype tests pass under `cargo test -p termrock keymap`; no public API changed yet (`cargo public-api` diff empty if run).

### Step 3: Design doc + build-plan stub

`plans/019-keymap-design.md`: the three candidates with scores, the winner's full public API (types, methods, the `from_static` bridge), the hint-derivation data flow diagram (text form), conflict-detection contract, serde example (a TOML snippet mapping to bindings — illustrative only), migration sketch for existing static tables, open questions (per-widget vs app-global maps; do widget-internal keys — List's j/k — join the keymap system or stay hardcoded? Cite `list.rs:80`'s hardcoded vim keys in the question).

**Verify**: doc exists; README row updated with the winner in one line.

## Done criteria

- [ ] `plans/019-keymap-design.md` with three-candidate evaluation + winner + full API sketch
- [ ] Winner prototype compiles + ≥4 prototype tests pass (remap hit, old-chord miss, hint reflects remap, conflict detection)
- [ ] Const-Cow feasibility fact recorded (candidate 2's make-or-break) even if another candidate wins
- [ ] No public surface changed; workspace tests green
- [ ] `plans/README.md` status row updated

## STOP conditions

- Plan 006 not landed (two vocabularies would double the design) — stop, dependency.
- The winner requires `A: 'static + Copy` bounds to loosen in ways that break `Keymap::dispatch` callers — document the bound change as a build-plan cost, don't force it in the spike.

## Maintenance notes

- The follow-up build plan must update the module doc's "single source of truth" paragraph to describe the runtime path, and migrate the lookbook's static tables as the reference consumer.
- Widget-internal hardcoded keys (List/Tree j/k arrows) joining the keymap system is a SEPARATE future decision — the doc's open-questions section carries it; don't fold it into the first build.
