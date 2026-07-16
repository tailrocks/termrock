# TermRock migration index

TermRock optimizes for the best current API and architecture, not backward
compatibility. Consumers pin reviewed full Git revisions and migrate forward
without compatibility shims or parallel legacy paths. TermRock keeps executor,
output, validation, wording, and application models consumer-owned unless a
migration explicitly changes that boundary.

Apply every migration after the consumer's pinned version in numeric order:

| Sequence | Version | Migration |
|---:|---|---|
| 0001 | `v0.7.0` | [Canonical namespaces](migrations/0001-v0.7.0-canonical-namespaces.md) |
| 0002 | `v0.8.0` | [Canonical widget contracts](migrations/0002-v0.8.0-canonical-widget-contracts.md) |
| 0003 | `v0.9.0` | [Styled tab glyphs](migrations/0003-v0.9.0-styled-tab-glyphs.md) |

Each breaking or dramatic public change adds the next zero-padded file and an
index row in the same commit. Existing migration files describe historical
boundaries and are not rewritten to describe a later API. Agents encountering
an incompatibility should locate the consumer's pinned version, then walk these
files sequentially until reaching the target revision.
