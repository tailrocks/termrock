# TermRock

**Beautiful, inspectable terminal components you own.**

TermRock is the **source-owned design system for building exceptional terminal
software** on [Ratatui](https://ratatui.rs/)—not merely another widget dump.
Category direction (kernel + registry + Studio + agent pack):
[`docs/design/shadcn-tui-strategic-brief.md`](docs/design/shadcn-tui-strategic-brief.md).

It is a **hybrid terminal design system**: a stable interaction kernel
(session lifecycle, focus, overlays, semantic intents, design tokens), product-
neutral widgets, and composition patterns—inspired by the open, inspectable
source model of [shadcn/ui](https://ui.shadcn.com/docs), adapted to Rust,
Ratatui, and terminal constraints. Architecture foundation:
[`docs/design/architecture-foundation.md`](docs/design/architecture-foundation.md).
Experience research:
[`docs/design/experience-research-2026.md`](docs/design/experience-research-2026.md).

Reusable visual and interaction behavior belongs here. Applications keep only
their domain state and wording, effects, process policy, secrets, executor
choices, and projections into TermRock components. During this pre-release
period, shared design quality takes priority over API compatibility; consumers
pin exact revisions and adapt to deliberate breaking changes using
[`MIGRATING.md`](MIGRATING.md).

The repository is in its bootstrap extraction period. Consumers pin exact Git
revisions; crates.io publication is not part of the initial migration.

The **design** baseline is latest stable Rust (1.97.1+) on Linux and macOS with truecolor terminals
in the Ghostty class. Optional requests cover OSC 8 hyperlinks, OSC 22 pointer
shapes, and OSC 52 clipboard writes. **Runtime progressive enhancement** is
supported via `ColorCapability` (including `NO_COLOR` → monochrome),
`Appearance` detection, `GlyphSet::Ascii`, and `Motion` reduction—not
truecolor-only forever.

```toml
termrock = { git = "https://github.com/tailrocks/termrock.git", rev = "FULL_COMMIT_SHA" }
```

Default features are empty. Enable `crossterm` only for its event, backend, and scoped-session adapters.

## Compatibility

| Surface | Baseline |
|---|---|
| Rust | 1.97.1 (latest stable; toolchain-pinned) |
| Operating systems | Linux and macOS |
| Ratatui | `ratatui-core 0.1.2`, `ratatui-widgets 0.3.2`, optional `ratatui-crossterm 0.1.2` |
| Crossterm | optional `0.29.0` adapter feature |
| Terminal | UTF-8, truecolor, modern VT behavior; Ghostty-class baseline |
| Optional OSC | OSC 8 hyperlinks, OSC 22 pointer shape, OSC 52 clipboard write; consumers own emission policy |

The exact first-consumer revision and reproduction commands live in [`compatibility.toml`](compatibility.toml). Reduced-color, `NO_COLOR`, Windows, and RTL/BiDi support are not claimed by this revision line.
