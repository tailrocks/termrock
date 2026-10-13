<!--
SPDX-FileCopyrightText: 2026 Alexey Zhokhov
SPDX-License-Identifier: Apache-2.0
-->

# termrock-showcase

The flagship workbench: a real terminal application built out of **public
TermRock only**. Transcript, prompt composer, trust gate, plan and diff
review, task rail, status bar — the same widgets a consumer imports, composed
the way a consumer would compose them.

The agent behind it is scripted. No provider, no network, no shell: the demo is
deterministic, runs anywhere, and touches nothing on your disk.

## Two minutes to the wow

```sh
cargo run -p termrock-showcase
```

1. **Type anything and press Enter.** The reply streams in token by token; the
   composer stays live while it does.
2. **Press `^n`** to move to the next scenario, then Enter again:
   - *Run a tool* — a tool card opens, its output streams, it exits green.
   - *Ask for a high-risk permission* — the trust gate takes the screen. The
     agent **stops**. `Esc` dismisses it, which is never a grant; the draft you
     were writing is still there afterwards.
   - *Propose a plan* / *Propose a diff* — review surfaces with real hunk
     navigation.
   - *Spawn subagents* — parallel work reported in the rail.
3. **Widen past 100 columns** and a files rail appears beside the workbench —
   a showcase-owned split, built from public `layout`, showing what a host adds
   around the pattern.
4. **Resize the terminal** to 40 columns. The layout contracts; submit and read
   both survive. Try `20×5`: it still paints something usable.
5. **`^q`** quits.

## Keys

| Key | What it does |
|---|---|
| `Enter` | Submit the draft (queues it when the agent is busy) |
| `Esc` | Peel exactly one layer — overlay, then review, then nothing |
| `Tab` / `Shift+Tab` | Cycle panes |
| `^n` | Next scenario |
| `^q` | Quit |

## What it proves

- **Public API is sufficient.** Every surface is `termrock::{widgets,
  patterns, style, layout, runtime, input, interaction}`. There is no private
  reach-in, no forked widget, and no local approval card — a missing capability
  is filed in `docs/design/showcase-api-gaps.md` and shipped in the library, not
  worked around here.
- **Trust is honest.** Dismissing a permission never grants it, a high-risk
  request never defaults to Allow, and the agent does not stream behind its own
  gate.
- **Continuity holds.** A half-written prompt survives an overlay, a review,
  and a cancel.
- **The design law applies to applications.** `tests/scenes.rs` renders every
  scenario and asserts the accent budget, the one-focused-border rule, and
  usable paint at 120×32, 80×24, 40×16 and 20×5.

## Not goals

Not published to crates.io, not a product, not an agent. It is the standing
dogfood: every future library redesign runs this demo as its human acceptance
test.
