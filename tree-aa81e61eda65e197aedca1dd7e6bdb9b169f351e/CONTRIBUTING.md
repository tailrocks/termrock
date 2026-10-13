# Contributing

Use Conventional Commits and `git commit -s`. Neutral defects reproducible by
a TermRock story or fixture belong here; reports requiring consumer state,
wording, or effects belong in that consumer. Cross-repository reports link both
sides. Reviewed green checkpoints push directly to `main`.

All work is trunk-only: use `main`, never create or publish another branch, and
never open a pull request. Keep changes small and forward-only, run the relevant
gates, push every green commit immediately, and verify local `HEAD` equals
`origin/main`. Never rewrite published history.

Changes to public APIs must update the API report and component documentation in
the same commit. Breaking or dramatic changes must also update `MIGRATING.md`
with an old-to-new surface map, required consumer edits, before/after examples,
removed concepts, and validation commands. After `v0.6.0`, `semver-candidate`,
`rust-required`, and `docs-required` are release gates; incompatible changes
require an intentional version decision. Prefer the best forward design over
backward compatibility; migration documentation replaces compatibility shims,
deprecated aliases, and parallel legacy implementations.
