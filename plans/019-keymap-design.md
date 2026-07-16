# Runtime-configurable keymaps: one Cow-backed source

## Decision

Use one Cow-layered `Keymap`. Static defaults remain allocation-free and const
constructible; the first runtime edit clones the binding table, after which
only edited fields become owned. Dispatch, hints, glyph lookup, and conflict
inspection continue reading the same resolved binding slice.

The prototype lives under `#[cfg(test)]` in `keymap.rs`. It remaps Quit from
`q` to `Ctrl+C`, proves the old chord misses and the new chord dispatches,
derives the new hint, reports a conflict, and compile-checks an owned serde
wire shape. The const Cow construction test also passes on the declared Rust
1.95 MSRV, not only the active 1.97 toolchain.

## Candidate evaluation

Scores are 1 (poor) through 5 (strong).

| Candidate | Dispatch = hints | Const defaults | Serde | Surface size | Forward-only coherence | Total |
|---|---:|---:|---:|---:|---:|---:|
| Owned everything | 5 | 1 | 5 | 4 | 5 | 20 |
| Cow layered | 5 | 5 | 4 | 4 | 5 | **23** |
| Static base + overrides | 4 | 5 | 4 | 2 | 1 | 16 |

### 1. Owned everything

```rust
pub struct KeyBinding<A> {
    pub chords: Vec<KeyChord>,
    pub action: A,
    pub hint: Option<String>,
    pub visibility: Visibility,
    pub glyph: Option<String>,
}
pub struct Keymap<A> { bindings: Vec<KeyBinding<A>> }
```

This is the simplest runtime model and serde derives naturally. Static tables
must be copied at startup and cannot remain `const`. The cost is small, but it
needlessly removes a useful property from every default map.

### 2. Cow layered — chosen

```rust
pub struct KeyBinding<A: Clone + 'static> {
    chords: Cow<'static, [KeyChord]>,
    action: A,
    hint: Option<Cow<'static, str>>,
    visibility: Visibility,
    glyph: Option<Cow<'static, str>>,
}

pub struct Keymap<A: Clone + 'static> {
    bindings: Cow<'static, [KeyBinding<A>]>,
}
```

`Cow::Borrowed` is const-constructible for both slices and strings on Rust
1.95. `Cow::to_mut` gives mutation a single copy-on-write boundary. Deserialized
maps naturally occupy the owned variants. This is one model, not parallel
static/runtime dispatch paths.

### 3. Static base plus overrides

```rust
pub struct KeymapOverrides<A> {
    remaps: Vec<(A, Vec<KeyChord>)>,
    disabled: Vec<A>,
}
pub struct ResolvedKeymap<'a, A> { base: &'a Keymap<A>, overrides: &'a KeymapOverrides<A> }
```

Defaults remain untouched, but every operation must use `ResolvedKeymap` or
risk reading stale base hints. Two representations and a mandatory resolved
view conflict with the repository's single-path, forward-only direction.

## Full proposed API

```rust
impl<A: Clone + 'static> KeyBinding<A> {
    pub const fn borrowed(
        chords: &'static [KeyChord],
        action: A,
        hint: Option<&'static str>,
        visibility: Visibility,
        glyph: Option<&'static str>,
    ) -> Self;

    pub fn owned(
        chords: Vec<KeyChord>,
        action: A,
        hint: Option<String>,
        visibility: Visibility,
        glyph: Option<String>,
    ) -> Self;
}

impl<A: Clone + Copy + PartialEq + 'static> Keymap<A> {
    pub const fn from_static(bindings: &'static [KeyBinding<A>]) -> Self;
    pub fn from_owned(bindings: Vec<KeyBinding<A>>) -> Self;
    pub fn bindings(&self) -> &[KeyBinding<A>];
    pub fn remap(&mut self, action: A, chords: Vec<KeyChord>) -> bool;
    pub fn replace(&mut self, action: A, binding: KeyBinding<A>) -> bool;
    pub fn disable(&mut self, action: A) -> bool;
    pub fn dispatch(&self, chord: KeyChord) -> Option<A>;
    pub fn hint_spans(&self) -> Vec<HintSpan<'static>>;
    pub fn conflicts(&self) -> Vec<Conflict<'_, A>>;
}

pub struct Conflict<'a, A> {
    pub first: &'a A,
    pub second: &'a A,
    pub chord: KeyChord,
}
```

The eventual build should add serde derives to `KeyCode`, `KeyModifiers`,
`KeyChord`, `Visibility`, owned `KeyBinding`, and owned `Keymap` under the
existing `serde` feature. Deserialization must produce owned Cow variants.
`A: Clone` is the copy-on-write cost; current callers use small Copy enums, so
the stronger mutation path does not invalidate dispatch callers.

## Data flow and invariant

```text
static borrowed table ─┐
                       ├─> Keymap.bindings (Cow) ─> dispatch(chord)
config-owned table ────┘                         ├─> hint_spans()
runtime remap ──to_mut───────────────────────────├─> glyph_for(action)
                                                 └─> conflicts()
```

There is no independent hint registry and no override lookup beside the
binding table. A remap replaces `binding.chords`; every consumer immediately
observes the same resolved data.

`conflicts()` is diagnostic, not policy. It returns declaration-order action
pairs for each shared chord. Dispatch remains first-binding-wins until the
consumer rejects, warns about, or intentionally accepts the conflict.

## Illustrative serde configuration

```toml
[[bindings]]
action = "quit"
chords = [{ key = "c", modifiers = ["control"] }]
hint = "quit"
visibility = "shown"
glyph = "Ctrl-C"
```

The exact TOML spelling belongs to the consumer. TermRock supplies serializable
plain data, not file discovery, merge policy, or conflict UX.

## Migration sketch

Static tables move from public-field literals to the const constructor:

```rust
// Before
KeyBinding { chords: QUIT, action: Quit, hint: Some("quit"), visibility: Shown, glyph: None }

// After
KeyBinding::borrowed(QUIT, Quit, Some("quit"), Visibility::Shown, None)
```

`Keymap::new(BINDINGS)` becomes `Keymap::from_static(BINDINGS)`. Existing
dispatch/hint calls stay conceptually identical. Runtime consumers call
`remap` on the same map before entering the event loop.

## Follow-up build-plan stub

1. Replace the static-only structs with the Cow model in one breaking change.
2. Add serde derives and owned deserialization tests.
3. Add conflict, remap, disable, axis-filter, glyph, and first-binding-wins
   tests against both borrowed and owned maps.
4. Migrate every TermRock and lookbook static table through `borrowed` and use
   the lookbook for one runtime-remap demonstration.
5. Add migration documentation and regenerate the public API inventory.

## Open questions

- Should maps be app-global, per focus scope, or composed? Start per context as
  today; global merge policy needs consumer evidence.
- Should `List`, `Tree`, and other widget-internal arrow/j/k handling consume
  injected keymaps? Not in the first build. Those keys are currently hardcoded
  interaction semantics and need a separate cross-widget design.
- Should duplicate chords inside one action be normalized? Diagnostics can
  ignore self-duplicates; mutation need not reorder user input.
- Is first-binding-wins acceptable after conflicts are exposed, or should
  dispatch return an ambiguity? Preserve deterministic current behavior until
  a consumer demonstrates stricter policy.
