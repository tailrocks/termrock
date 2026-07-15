# TermRock

Product-neutral Ratatui components, interaction foundations, a lookbook, and
generated component documentation for terminal applications.

The repository is in its bootstrap extraction period. Consumers pin exact Git
revisions; crates.io publication is not part of the initial migration.

The supported baseline is Rust 1.95 on Linux and macOS with truecolor terminals in the Ghostty class. Optional requests cover OSC 8 hyperlinks, OSC 22 pointer shapes, and OSC 52 clipboard writes. TermRock intentionally has no reduced-color or `NO_COLOR` degradation path.

```toml
termrock = { git = "https://github.com/tailrocks/termrock.git", rev = "FULL_COMMIT_SHA" }
```

Default features are empty. Enable `crossterm` only for its event, backend, and scoped-session adapters.

## Compatibility

| Surface | Baseline |
|---|---|
| Rust | 1.95 minimum; 1.97 tested |
| Operating systems | Linux and macOS |
| Ratatui | `ratatui-core 0.1.2`, `ratatui-widgets 0.3.2`, optional `ratatui-crossterm 0.1.2` |
| Crossterm | optional `0.29.0` adapter feature |
| Terminal | UTF-8, truecolor, modern VT behavior; Ghostty-class baseline |
| Optional OSC | OSC 8 hyperlinks, OSC 22 pointer shape, OSC 52 clipboard write; consumers own emission policy |

The exact first-consumer revision and reproduction commands live in [`compatibility.toml`](compatibility.toml). Reduced-color, `NO_COLOR`, Windows, and RTL/BiDi support are not claimed by this revision line.
