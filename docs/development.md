# Development

Install pinned tools with `mise install` (Rust 1.99.1 and the tool
pins in `mise.toml`), then install the repository hooks with
`mise exec -- hk install --mise`. Run both commands from the workspace
root.

## Commands

| Task | Command |
| --- | --- |
| Run an app | `cargo run --bin showcase` (also `tablepro`, `holla`) |
| Run the jackin preview | `cargo run --bin jackin-preview --features demo` |
| Demo and PTY tests | `cargo nextest run --locked --workspace --all-features --profile functional --no-tests fail` |
| Product-shape tests | `cargo nextest run --locked -p jackin-console --no-default-features --features product --profile ci --no-tests fail` |
| Lint | `cargo clippy --locked --all-targets --all-features -- -D warnings` |
| Lint, product shape | `cargo clippy --locked -p jackin-console --all-targets --no-default-features --features product -- -D warnings` |
| Format check | `cargo fmt --all --check` |

The workspace default members are `junie-tui`, `termrock`, the four app
packages (`apps/holla`, `apps/jackin`, `apps/showcase`, `apps/tablepro`) and
`functional-cross`, so these commands cover all of them. The `functional`
profile selects the `functional` target of `junie-tui`, `jackin-console` and
`functional-cross`. To run the root journeys against freshly built candidates,
use `python3 scripts/run_functional.py` (see
[functional testing](functional-testing.md)).

The independent consumer fixtures under `tests/consumers/` are separate
workspaces, so no workspace command covers them. Run
`bash scripts/check-consumers.sh` to build and run all four, each in its own
Cargo invocation and target directory.

`jackin-console` has the feature `demo`, off by default. Without it the
`fixtures` and `sim` modules, the `jackin-preview` binary and the
`functional` target of the crate are not built, so a command that does
not pass `--all-features` skips the demo tests without a failure. CI
runs `--all-features` in `Rust / <crate>` and the product shape in
`Rust / <crate> (product)`; `suite-functional` runs the default features
(see [CI](ci.md#the-demo-feature)). Check that a change did not drop a
test with `cargo nextest list --locked --workspace --all-features`.

All test commands use `cargo nextest`, never `cargo test`. Run the
functional suite, clippy and fmt before reporting work as done; see
[functional testing](functional-testing.md) for the test system and
[CI](ci.md) for how the gates run remotely.

## Conventions

- Keep Rust comments to safety invariants, legally required notices,
  or non-obvious behaviour constraints; contracts and rationale live
  in these docs.
- Any `unsafe` operation must have its invariant understandable at
  the operation and its callers; do not claim a safety concern is
  resolved without a specific review and evidence.
- `CLAUDE.md` is a symlink to `AGENTS.md`; edit `AGENTS.md` only.
