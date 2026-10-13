# Testing

`mise run check` — formatting, clippy (warnings denied), all-feature nextest,
doctests. Run before every commit.

`mise run gate` — the full trunk gate: everything in `check` plus
no-default-features and examples checks, rustdoc with warnings denied,
feature powerset (cargo-hack), dependency policy (cargo-deny: advisories,
bans, licenses, sources), unused-dependency check (cargo-shear), packaging,
and the flagship preview baselines (`mise run preview-goldens` — a real diff
against committed cell dumps, not a render diffed against itself; bless an
intended change with `mise run bless-previews`). CI additionally verifies the
Rust 1.97.1 (toolchain-pinned latest stable), and REUSE compliance. Run before every
push.
