# Dependencies

Keep dependencies on their latest releases (crates, the Rust
toolchain, Tuiscotti, velnor-actions); upgrade at least monthly. A
version held below latest is recorded here with its blocker. Never
vendor or patch an upstream crate to get around a pin.

| Dependency | Pin | Notes |
| --- | --- | --- |
| Rust | 1.99.1 (`rust-toolchain.toml`, `mise.toml`) | Held below latest: 1.100.0 (Renovate PR #484) is blocked because velnor-actions, which generates CI, qualifies Rust only up to 1.99.1 (its version policy sets `rust = "1.99.1"`). CI fails on 1.100.0 in Plan, Build suite (functional) and Suite (performance). Blocker: [tailrocks/velnor-new#1625](https://github.com/tailrocks/velnor-new/issues/1625). No `v0.1.x` velnor-actions release can add it: upstream removed that release workflow, and 0.1.81 is the last release. Bump only after CI runs on a generator that qualifies 1.100.0, either a final velnor-actions release or a migration to velnor-gen. That route is the open question on #1625. |
| `termpane` | git rev `d05d114bdce504d184b00836376a350851806f61` | The `terminal` feature's emulator (`junie_tui::terminal`), replacing `vt100`. Pinned to a git commit, not a crates.io release: the commit is termpane `main` after PR #41 (unicode-width 0.2.3; DECSTR restores auto-wrap, keeps the cursor and bracketed paste and homes the saved cursor like xterm; `?1049` saves and restores the cursor with its deferred wrap and no longer homes it on entry; a wide lead gets its continuation when the grid widens; the half-width voiced marks U+FF9E and U+FF9F take a cell of their own; the serializer repaints a cell a wide print narrowed), PR #40 (the reuse of OSC 8 links), PR #39 (the clamp of the `CSI L`, `M`, `S` and `T` counts and the shared OSC 8 hyperlink) and PR #37 (the cluster cap), which also holds `EraseDisplayPolicy` (PR #35, merged as f1c2dfde). `TerminalState` sets `EraseDisplayPolicy::Discard` so `ESC[2J` no longer moves the visible rows into history, as `vt100` behaved; 0.1.0 has no such switch. termpane caps the characters that join one cell at 16, so no byte stream can grow a cell; the feed guard keeps its own count as a second layer. The tip also clamps the count of `CSI L`, `M`, `S` and `T` to the rows they can move, and shares one OSC 8 hyperlink between all the cells written under it (a URI over 8 KiB or an id over 256 bytes is refused), so neither a count nor a long link multiplies the work or the memory per input byte. The pin becomes `=0.1.1` when 0.1.1 is published on crates.io (the operator says when). The `46cc0977` to `d05d114` bump (PRs #42 to #45) changes only CI, pinned tooling and lint configuration in termpane, no source. Default features only; `process` and `pty` spawn local PTYs and stay off. Brings `vte`, `compact_str` 0.10 and `smallvec`. |
| `tuiscotti-tui-test` (`tui_test`) | `=0.2.1` | Live PTY driver for functional tests. |
| `tuiscotti` (`tuiscotti-latest`) | git rev `f1fb3f276b8d49948881c07b3bbc619840b6cc93` | Offline `FrozenStore` visual gate; bump to newest `main` rev deliberately when it moves. |
| `toml` (dev-dependency) | `1.2.1` | Parses `.velnor/config.toml` in `tests/ci_default_branch.rs`, so the default-branch guard reads the real TOML grammar instead of a hand-written line scanner (#13). Latest release at last review; tests only, never linked into a library or app. |
| velnor-actions | 0.1.78 (`mise.toml`) | Generates CI from `.velnor/config.toml`; see [CI](ci.md). |
| mr-boxington (`mbx`) | 1.30.0 (`mise.toml`, `postinstall = "mbx setup --yes"`) | Cargo wrapper via mise (`[wrappers.cargo]`, `mr_boxington = true` on the rust tool); CI uses the velnor `mbx` compile driver. |

Reviewed 2026-11-05. Testing-stack rationale is in
[functional testing](functional-testing.md).
