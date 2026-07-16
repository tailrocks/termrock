# Migrating TermRock consumers

TermRock optimizes for the best forward design rather than API compatibility.
Breaking changes expose only the new model: there are no deprecated aliases,
compatibility facades, or transition shims.

Pin a reviewed full Git revision and commit the Cargo lockfile. Before changing
that revision, apply every migration after the consumer's current version in
the order listed below. Each migration records the previous model, the new
model, and the downstream action required. TermRock keeps executor, output,
validation, wording, and application models consumer-owned unless a migration
explicitly changes that boundary.

## Ordered migrations

1. [`0001 — canonical namespaces (v0.7.0)`](migrations/0001-canonical-namespaces-v0.7.0.md)
2. [`0002 — canonical contracts (v0.8.0)`](migrations/0002-canonical-contracts-v0.8.0.md)

New breaking changes append one numbered file and one link to this list. Never
rewrite an older migration to describe a later API: agents must be able to walk
the history sequentially and identify exactly which transition resolves an
incompatibility.
