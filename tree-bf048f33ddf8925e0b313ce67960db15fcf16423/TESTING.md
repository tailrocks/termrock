# Testing

`mise run check` — formatting, clippy (warnings denied), all-feature nextest,
doctests. Run before every commit.

`mise run gate` — the full trunk gate: everything in `check` plus
no-default-features and examples checks, rustdoc with warnings denied,
feature powerset (cargo-hack), dependency policy (cargo-deny: advisories,
bans, licenses, sources), unused-dependency check (cargo-shear), packaging,
and the deterministic lookbook preview check. CI additionally verifies the
Rust 1.95 MSRV and REUSE compliance. Run before every push.
