# Termrock

Termrock is a Rust terminal UI component library with four
deterministic demonstration applications: Showcase, TablePro, Holla
and Jackin. The library owns interaction and state; the applications
prove the components compose in realistic screens.

In Showcase, multi-search picker filters accept spaces as text; use
Ctrl+Space to toggle the focused result and Enter to confirm the chosen
set.

## Use the library

Depend on the `termrock` facade, with the features you need. The default is
empty. Termrock is not published to crates.io, so use a path or git dependency:

```toml
[dependencies]
termrock = { git = "https://github.com/tailrocks/terminal-components-baseline", default-features = false, features = ["terminal"] }
```

Import from `termrock::` (`termrock::widgets`, `termrock::ui`, `termrock::core`,
`termrock::prelude`). The repository root package `junie-tui` is the
implementation behind it and keeps working for existing consumers. The
supported imports, features, ownership rules and the v0.1 migration are in the
[API contract](docs/api-contract.md).

Working consumers, each an independent package that compiles from its declared
features alone (`bash scripts/check-consumers.sh` runs all four):

- [`tests/consumers/minimal`](tests/consumers/minimal/src/lib.rs): no
  features. A form of an input and a list; a validated input; typed grid
  actions in order; nested scroll views that bubble a key; and a bounded
  external-message channel that returns its overflow.
- [`tests/consumers/keyed-modal`](tests/consumers/keyed-modal/src/lib.rs): a
  keyed list borrowed from a provider, under a modal layer.
- [`tests/consumers/terminal`](tests/consumers/terminal/src/lib.rs): the
  `terminal` feature and the public viewport.
- [`tests/consumers/jackin-host`](tests/consumers/jackin-host/src/lib.rs): a
  host of `jackin-console` with the demo off.

The applications are separate packages that import only the facade:
`cargo run -p termrock-showcase`, `-p termrock-tablepro`, `-p termrock-holla`
and `cargo run -p jackin-console --features demo --bin jackin-preview`.

## Generated-workflow snapshot bounds

The generated-workflow guard accepts recursive Git tree listings up to 8 MiB and 8,192 entries, with a 16 MiB maximum per blob and a 512 MiB maximum total of materialized blob occurrences. The measured baseline HEAD contains 608 blobs totaling 191,075,553 raw Git blob bytes; its largest blob is 1,876,744 bytes. The limits apply to raw blob bytes, count repeated object IDs and symlink targets once per tree entry, and leave room above that measured snapshot. Future or historical snapshots beyond these limits are rejected.

The pre-commit and pre-push guard requires one explicit setup from reviewed checkout code: `bash scripts/install-generated-workflow-guard.sh`. Setup installs a digest-pinned checker under Git common metadata and registers it in the effective hook directory. It qualifies the pinned Python and Mise artifacts, prepares the approved tool/dependency cache, and leaves any foreign hook registration untouched by failing closed. See [generated-workflow guard setup](docs/generated-workflow-guard.md) for the setup and update details.

When a validated selected snapshot is missing blobs but its commit and tree metadata are available, the guard retrieves only those blob IDs from the fixed official GitHub blob API and verifies the declared identity, decoded bytes, and Git object hash before publishing them to a private verified-blob store. The trusted-seed reader and selected-snapshot reader share this blob-only store while keeping their private tree-object stores separate. Across one checker invocation, hydration is bounded to 1,024 missing objects, 4 MiB per blob, 256 MiB total decoded bytes, and 15 minutes. Repeated pre-push records reuse already verified objects and receive separate bounded response scratch directories. The API must have each object, so unpublished local-only blobs remain unavailable. Missing commits and trees, and missing blobs in SHA-256 repositories, fail closed. These hydration limits are narrower than the 16 MiB and 512 MiB materialization limits above.

## Documentation

- [Current state](docs/current-state.md) — architecture and
  structure as they exist today.
- [API contract](docs/api-contract.md) — supported imports, features,
  ownership and migration for a consumer.
- [Future architecture](docs/architecture.md) — the refactoring
  target: crate structure, ownership contracts, migration gates.
- [Product contract](docs/product.md) — visual and interaction
  authority.
- [Requirements](docs/requirements.md) — conceptual acceptance
  surface per app and journey.
- [Functional testing](docs/functional-testing.md) — the single
  test estate and its gates.
- [Development](docs/development.md) · [CI](docs/ci.md) ·
  [Dependencies](docs/dependencies.md) · [Decisions](docs/decisions.md)

Agent working rules are in `AGENTS.md` (`CLAUDE.md` is a symlink to
it).
