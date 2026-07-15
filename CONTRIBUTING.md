# Contributing

Use Conventional Commits and `git commit -s`. Neutral defects reproducible by
a TermRock story or fixture belong here; reports requiring consumer state,
wording, or effects belong in that consumer. Cross-repository reports link both
sides. Until the first tag, reviewed green bootstrap checkpoints may push
directly to `main`; afterward all changes use protected-branch pull requests.

Changes to public APIs must update the API report and component documentation in the same pull request. After `v0.6.0`, `semver-candidate`, `rust-required`, and `docs-required` are release gates; incompatible changes require an intentional version decision.
