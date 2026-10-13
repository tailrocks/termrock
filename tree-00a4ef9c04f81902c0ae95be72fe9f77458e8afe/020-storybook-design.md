# Storybook design spike

## Verdict

Graduate the gallery-local knob model. The toast slice proves live props,
focus routing, theme switching, deterministic defaults, and crate installation
without changing TermRock's public API or catalog schema. Keep knobs in the
lookbook: they describe examples, not product state.

## Knob model and render contract

```rust
enum KnobValue { Bool(bool), Choice(usize), Text(String), Number(i64) }
struct Knob {
    id: &'static str,
    label: &'static str,
    value: KnobValue,
    choices: &'static [&'static str],
}
```

Each interactor owns its descriptors and mutable values. The gallery reads
them through `StoryInteraction::knobs`, forwards control keys through
`handle_knob_key`, and asks the interactor to render a type-specific editor.
This is the live equivalent of evolving the fixed `RenderFn` to receive
`&[Knob]`, while preserving fixed render functions for generated SVGs. Every
knob declaration has a deterministic default equal to the existing static
story. Thus `render` and `check` remain stable and never persist interactive
state.

The toast slice declares:

- `severity`: Choice, Info/Success/Warning/Error; default Success.
- `anchor`: Choice, four corners; default Top right.
- `message`: Text via `TextInputState`; default `Updated`.

## Gallery interaction and layout

```text
┌ Stories ─────┐ ┌ About ──────────────────────────┐
│ catalog       │ └────────────────────────────┘
│               │ ┌ Preview ──────────┐ ┌ Controls ─────────┐
│               │ │ live story          │ │ Severity  Success  │
│               │ │                     │ │ Anchor    Top right│
│               │ │                     │ │ Message   Updated  │
└───────────────┘ └─────────────────────┘ └───────────────────┘
```

`Tab` moves Sidebar → Preview → Controls when controls exist. In Controls,
Up/Down selects a knob, Left/Right cycles choices, text keys edit text, Tab
returns to Preview, and Escape returns to Sidebar. The global `t` binding
switches phosphor/slate from every focus target and immediately updates chrome
and the active interactor. Interactive values survive theme changes but reset
on story selection.

## Story migration inventory

| Story | Candidate knobs |
|---|---|
| panel/focused | title, emphasis |
| action-bar/basic | enabled actions, focused action |
| tabs/status | selected tab, status glyphs |
| hint-bar/wrapped | width, priority set |
| list/selection | selected row, multi-select |
| tree/navigation | selected node, expansion, status |
| progress/determinate | kind, fraction, tick, label |
| log-pane/follow | follow, max lines, appended text |
| form/responsive | width, validation, disabled field |
| split-pane/horizontal | ratio, direction, collapsed pane |
| text-input/filter | value, validation, maximum length |
| detail-table/basic | wrap, selected row, capability |
| status-bar/basic | width, slot priorities |
| dialog/message | placement, title, body |
| choice-dialog/basic | selected action, disabled action |
| message-dialog/details | severity, details visibility |
| diff/basic | line kinds, scroll |
| toast/success | **implemented:** severity, anchor, message |
| backdrop/basic | symbol, style role |
| viewport/both-axes | horizontal/vertical offset, width |

Full rollout is roughly 4–6 focused implementation days: centralize typed
editor rendering, migrate four low-state stories first, then stateful stories,
add per-story interaction tests, and extend catalog validation to require knob
IDs/defaults. Do not force knobs onto stories whose contract is clearer as
separate narrow/Unicode variants.

## Dogfooding friction

- `List` provides correct stable-ID selection and trailing values, but has no
  compact property-grid row or inline editor composition.
- `TextInput` works well, but the caller must separately synchronize its state
  into a generic descriptor value.
- Choice cycling has no small reusable segmented/select control; the gallery
  owns wraparound math and hint wording.
- Focus remains application-owned. Three-pane routing is clear, but widgets do
  not expose a shared focus graph; Plan 032 should use this reproduction.
- Bool and Number need dedicated editors before full rollout. Encoding them as
  text would weaken validation.

These are evidence for future components, not reasons to put gallery concepts
in TermRock.

## Installability

Verified on 2026-07-16 with a fresh temporary `CARGO_HOME` and install root:

```sh
cargo install --git https://github.com/tailrocks/termrock termrock-lookbook --locked
```

Cargo built 74 dependencies and installed `termrock-lookbook`; its `list`
command ran successfully. Workspace/path dependencies resolve from the cloned
git workspace. Keep the binary name: it is unambiguous and avoids pretending
Cargo supports a `termrock lookbook` subcommand. An external custom theme needs
a future `--theme-file <toml>` backed by the serde direction from Plan 019;
today evaluators can use built-in `--theme phosphor|slate` for generated output
and `t` interactively. No publication action is needed.

## Open decisions

- Persistence should be explicit and opt-in, keyed by story ID plus knob ID;
  defaults and CI renders must ignore it.
- Theme is global gallery state. Per-story themes would make comparison and
  navigation surprising.
- A future theme file needs a versioned schema and complete role validation,
  not partial silent fallback.
- Decide whether Text editing should reserve `t` while focused; the spike keeps
  `t` globally authoritative as specified.
