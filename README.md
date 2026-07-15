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
