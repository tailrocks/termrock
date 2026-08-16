# ChoiceDialog — Old rev vs HEAD comparison

Part of `roadmap/jackin-termrock-parity`. Produced by plan 008. The verdict
below is recorded and applied via plan 009 — never filled by an executor.

- **Family**: ChoiceDialog
- **Old rev**: `5ff94ee117fd4a1b72fdd0d1b1847815055a93ac`
- **HEAD at comparison**: `5bcaac4b`
- **States covered**: 1 compared, 2 uncomparable, 0 HEAD-only
- **Produced by**: dedicated subagent run

## Compared states

### choice-dialog/basic

| Old rev | HEAD |
|---------|------|
| ![choice-dialog/basic old](img/choice-dialog-basic--old.png) | ![choice-dialog/basic HEAD](img/choice-dialog-basic--head.png) |

Differences — every visible difference named, each classed:

| # | Difference | Class |
|---|------------|-------|
| 1 | The surrounding canvas shifts from black to charcoal gray. | palette-level |
| 2 | The Old-rev green single-line border is removed, leaving an unframed dark content rectangle in HEAD. | widget-level |
| 3 | The inline `Choose` border title becomes a wider solid-green `Choose action` header label with black text. | widget-level |
| 4 | The `Continue with this operation?` prompt is absent in HEAD. | widget-level |
| 5 | The selected `Continue` button and adjacent `Cancel` action are both absent in HEAD. | widget-level |

## Uncomparable states

States with a HEAD story but no Old-rev construction path, from the
old-rev harness uncomparable list (reasons verbatim):

| Story id | Reason |
|----------|--------|
| choice-dialog/narrow | - `choice-dialog/narrow` (ChoiceDialog): component exists at 5ff94ee1 but this state has no Old-rev story counterpart and no equivalent public-constructor setup was defined at the pin |
| choice-dialog/unicode | - `choice-dialog/unicode` (ChoiceDialog): component exists at 5ff94ee1 but this state has no Old-rev story counterpart and no equivalent public-constructor setup was defined at the pin |

## HEAD-only states

HEAD states with no Old-rev render and no uncomparable entry (added after
the harness ran) — visible here, not compared:

None.

## Verdict

**Verdict**: _pending_
<!-- Allowed values: merge | restore | accept (merge = expected default: jackin-era base, current improvements kept on top; restore = Old-rev look; accept = record the divergence). The user rules (D1): replace `_pending_` with exactly one value — nothing else on the line. Plan 009 appends an `**Applied**: <date>` line below after application. -->
