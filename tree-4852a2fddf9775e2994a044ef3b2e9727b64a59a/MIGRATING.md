# TermRock migration index

TermRock optimizes for the best current API and architecture, not backward
compatibility. Consumers pin reviewed full Git revisions and migrate forward
without compatibility shims or parallel legacy paths. TermRock keeps executor,
output, validation, wording, and application models consumer-owned unless a
migration explicitly changes that boundary.

Migration versions correspond to immutable Git tags from `v0.7.0` onward.
Release-boundary rules and tag ownership are documented in
[`RELEASING.md`](RELEASING.md).

Apply every migration after the consumer's pinned version in numeric order:

| Sequence | Version | Migration |
|---:|---|---|
| 0001 | `v0.7.0` | [Canonical namespaces](migrations/0001-v0.7.0-canonical-namespaces.md) |
| 0002 | `v0.8.0` | [Canonical widget contracts](migrations/0002-v0.8.0-canonical-widget-contracts.md) |
| 0003 | `v0.9.0` | [Styled tab glyphs](migrations/0003-v0.9.0-styled-tab-glyphs.md) |
| 0004 | `v0.9.0` | [Typed OSC requests](migrations/0004-v0.9.0-typed-osc-requests.md) |
| 0005 | `v0.9.0` | [Unknown key handling](migrations/0005-v0.9.0-unknown-key-handling.md) |
| 0006 | `v0.9.0` | [Unified key vocabulary](migrations/0006-v0.9.0-unified-key-vocabulary.md) |
| 0007 | `v0.9.0` | [Constructible theme](migrations/0007-v0.9.0-constructible-theme.md) |
| 0008 | `v0.9.0` | [Semantic theme palette](migrations/0008-v0.9.0-semantic-theme-palette.md) |
| 0009 | `v0.9.0` | [Neutral event contract](migrations/0009-v0.9.0-neutral-event-contract.md) |
| 0010 | `v0.9.0` | [Canonical module homes](migrations/0010-v0.9.0-canonical-module-homes.md) |
| 0011 | `v0.10.0` | [Trailing metadata cells](migrations/0011-v0.10.0-trailing-metadata-cells.md) |
| 0012 | `v0.10.0` | [Widget construction and growth](migrations/0012-v0.10.0-widget-construction-and-growth.md) |
| 0013 | `v0.10.0` | [Content measurement revisions](migrations/0013-v0.10.0-content-measurement-revisions.md) |
| 0014 | `v0.10.0` | [Scroll and hover unification](migrations/0014-v0.10.0-scroll-and-hover-unification.md) |
| 0015 | `v0.10.0` | [Independent terminal session options](migrations/0015-v0.10.0-independent-session-options.md) |
| 0016 | `v0.11.0` | [Ordinary vs strong text and Viewport emphasis](migrations/0016-v0.11.0-text-strong-and-viewport-emphasis.md) |
| 0017 | `v0.11.0` | [First-class scrollable block helpers](migrations/0017-v0.11.0-scrollable-block-helpers.md) |
| 0018 | `v0.11.0` | [Theme-explicit scroll and typed dialog input](migrations/0018-v0.11.0-theme-explicit-scroll.md) |
| 0019 | `v0.11.0` | [Bounded LogPane scrollback](migrations/0019-v0.11.0-bounded-log-pane-scrollback.md) |
| 0020 | `v0.11.0` | [Explicit LogPane oldest navigation](migrations/0020-v0.11.0-log-pane-oldest-navigation.md) |
| 0021 | `v0.11.0` | [Responsive Progress percentage](migrations/0021-v0.11.0-responsive-progress-percentage.md) |

Each breaking or dramatic public change adds the next zero-padded file and an
index row in the same commit. Existing migration files describe historical
boundaries and are not rewritten to describe a later API. Agents encountering
an incompatibility should locate the consumer's pinned version, then walk these
files sequentially until reaching the target revision.
