# ActionBar — Old rev vs HEAD comparison

Part of `roadmap/jackin-termrock-parity`. Produced by plan 008. The verdict
below is recorded and applied via plan 009 — never filled by an executor.

- **Family**: ActionBar
- **Old rev**: `5ff94ee117fd4a1b72fdd0d1b1847815055a93ac`
- **HEAD at comparison**: `5bcaac4b`
- **States covered**: 1 compared, 4 uncomparable, 0 HEAD-only
- **Produced by**: dedicated subagent run

## Compared states

### action-bar/basic

| Old rev | HEAD |
|---------|------|
| ![action-bar/basic old](img/action-bar-basic--old.png) | ![action-bar/basic HEAD](img/action-bar-basic--head.png) |

Differences — every visible difference named, each classed:

| # | Difference | Class |
|---|------------|-------|
| 1 | Selected `Accept` fill changed from white to bright phosphor green. | palette-level |
| 2 | Unselected `Cancel` label changed from bright white to a dim neutral gray-green. | palette-level |
| 3 | Action-row background changed from black to a subtly green-tinted near-black. | palette-level |

## Uncomparable states

States with a HEAD story but no Old-rev construction path, from the
old-rev harness uncomparable list (reasons verbatim):

| Story id | Reason |
|----------|--------|
| action-bar/disabled | - `action-bar/disabled` (ActionBar): component exists at 5ff94ee1 but this state has no Old-rev story counterpart and no equivalent public-constructor setup was defined at the pin |
| action-bar/focused | - `action-bar/focused` (ActionBar): component exists at 5ff94ee1 but this state has no Old-rev story counterpart and no equivalent public-constructor setup was defined at the pin |
| action-bar/narrow | - `action-bar/narrow` (ActionBar): component exists at 5ff94ee1 but this state has no Old-rev story counterpart and no equivalent public-constructor setup was defined at the pin |
| action-bar/unicode | - `action-bar/unicode` (ActionBar): component exists at 5ff94ee1 but this state has no Old-rev story counterpart and no equivalent public-constructor setup was defined at the pin |

## HEAD-only states

HEAD states with no Old-rev render and no uncomparable entry (added after
the harness ran) — visible here, not compared:

None.

## Verdict

**Verdict**: _pending_
<!-- Allowed values: merge | restore | accept (merge = expected default: jackin-era base, current improvements kept on top; restore = Old-rev look; accept = record the divergence). The user rules (D1): replace `_pending_` with exactly one value — nothing else on the line. Plan 009 appends an `**Applied**: <date>` line below after application. -->
