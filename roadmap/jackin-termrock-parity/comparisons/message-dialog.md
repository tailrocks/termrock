# MessageDialog — Old rev vs HEAD comparison

Part of `roadmap/jackin-termrock-parity`. Produced by plan 008. The verdict
below is recorded and applied via plan 009 — never filled by an executor.

- **Family**: MessageDialog
- **Old rev**: `5ff94ee117fd4a1b72fdd0d1b1847815055a93ac`
- **HEAD at comparison**: `5bcaac4b`
- **States covered**: 1 compared, 2 uncomparable, 0 HEAD-only
- **Produced by**: dedicated subagent run

## Compared states

### message-dialog/details

| Old rev | HEAD |
|---------|------|
| ![message-dialog/details old](img/message-dialog-details--old.png) | ![message-dialog/details HEAD](img/message-dialog-details--head.png) |

Differences — every visible difference named, each classed:

| # | Difference | Class |
|---|------------|-------|
| 1 | Dialog interior changed from black to a dark green-tinted surface; surrounding canvas also shifted from neutral charcoal to a green-tinted dark tone. | palette-level |
| 2 | Body copy changed from bright white to muted light gray. | palette-level |
| 3 | Detail labels changed from phosphor green to muted gray. | palette-level |
| 4 | `Ready` value changed from bright white to muted light gray. | palette-level |
| 5 | Content gained left padding: body copy and detail rows start two cells farther right. | widget-level |
| 6 | Content moved one row downward, adding blank space below the title border. | widget-level |
| 7 | Colon separators between detail labels and values were removed. | widget-level |
| 8 | Detail value column moved right and its alignment changed relative to the labels. | widget-level |
| 9 | Old dialog's black right-and-bottom drop shadow was removed. | widget-level |

## Uncomparable states

States with a HEAD story but no Old-rev construction path, from the
old-rev harness uncomparable list (reasons verbatim):

| Story id | Reason |
|----------|--------|
| message-dialog/narrow | - `message-dialog/narrow` (MessageDialog): component exists at 5ff94ee1 but this state has no Old-rev story counterpart and no equivalent public-constructor setup was defined at the pin |
| message-dialog/unicode | - `message-dialog/unicode` (MessageDialog): component exists at 5ff94ee1 but this state has no Old-rev story counterpart and no equivalent public-constructor setup was defined at the pin |

## HEAD-only states

HEAD states with no Old-rev render and no uncomparable entry (added after
the harness ran) — visible here, not compared:

None.

## Verdict

**Verdict**: _pending_
<!-- Allowed values: merge | restore | accept (merge = expected default: jackin-era base, current improvements kept on top; restore = Old-rev look; accept = record the divergence). The user rules (D1): replace `_pending_` with exactly one value — nothing else on the line. Plan 009 appends an `**Applied**: <date>` line below after application. -->
