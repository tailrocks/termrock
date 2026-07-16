# Contributing

Use Conventional Commits and `git commit -s`. Neutral defects reproducible by
a TermRock story or fixture belong here; reports requiring consumer state,
wording, or effects belong in that consumer. Cross-repository reports link both
sides. All work is committed and pushed directly to `main`; do not create
feature branches or pull requests.

Changes to public APIs must update the API report, component documentation, and
relevant guides in the same commit. Breaking changes also require a versioned
`MIGRATING.md` entry with explicit old-to-new mappings and downstream ownership
changes. After `v0.6.0`, incompatible changes require an intentional version
decision. Compatibility shims are not a substitute for migration documentation.
