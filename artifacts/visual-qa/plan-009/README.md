# Plan 009 visual QA

## Table and DataTable row recipes

Validated the DataTable handbook with `agent-browser` at 375×812, 768×1024,
and 1440×900 plus paper/reduced-motion desktop. Keyboard scene navigation
worked; page overflow stayed zero and wide terminal content remained inside its
preview scroller. Consoles and page errors were clean.

Designer verdict — Table/DataTable: **pass** — the raised muted header creates
a stable scanning tier; tint plus gutter makes selection clear without a neon
slab; hover and zebra remain subordinate. Dense data, Unicode, narrow clipping,
paper host, and reduced-motion states remain legible. Evidence:
[desktop](data-table-desktop-1440x900.png),
[paper](data-table-paper-reduced-motion-1440x900.png),
[mobile](data-table-mobile-375x812.png), and
[tablet](data-table-tablet-768x1024.png).

## TreeTable recipe rows

Validated the generated TreeTable component page with `agent-browser` at
375×812, 768×1024, and 1440×900 plus paper/reduced-motion desktop. Keyboard
navigation and pointer hover advanced the interactive preview; the page had no
horizontal overflow, preview overflow stayed contained, internal links and all
frame requests returned successfully, and console/page errors were clean.

Designer verdict — TreeTable: **pass after 1 iteration** — the first capture
exposed a transient state-tour fill variant, so review returned to the default
phosphor scene. The final default has a clear raised header, quiet hierarchy
indent, full-row tint plus gutter, distinct muted/loading tiers, and a hover
wash subordinate to selection. Narrow content scrolls inside the terminal;
paper and reduced-motion remain readable and deterministic. Evidence:
[desktop](tree-table-desktop-1440x900.png),
[paper](tree-table-paper-reduced-motion-1440x900.png),
[mobile](tree-table-mobile-375x812.png), and
[tablet](tree-table-tablet-768x1024.png).

## Overlay and picker recipe rows

Validated each component page with `agent-browser` at 375×812, 768×1024,
and 1440×900 plus light/reduced-motion desktop. Every page stayed at zero page
overflow; narrow terminal overflow remained inside its preview. Keyboard state
tours responded, internal links resolved, frame requests succeeded, and
console/page errors were clean. Browser review triggered two shared iterations:
selected text first erased tint backgrounds, then palette reconstruction exposed
the stale Fill default. Both enabling conditions were removed in the recipe.

- Designer verdict — MenuBar: **pass after 2 shared iterations** — quiet bar,
  canonical floating shell, clear mnemonic/focus hierarchy. [desktop](menu-bar-desktop-1440x900.png)
- Designer verdict — DropdownMenu: **pass after 2 shared iterations** — overlay
  layering, shortcuts, disabled rows, and selection remain distinct. [desktop](dropdown-menu-desktop-1440x900.png)
- Designer verdict — CommandPalette: **pass after 2 shared iterations** — overlay
  focal point, sunken query, grouped results, calm tint, and accent matches read
  as one refined command surface. [desktop](command-palette-desktop-1440x900.png)
- Designer verdict — QuickOpen: **pass after 2 shared iterations** — provider,
  result, detail, and selection tiers scan cleanly under narrow clipping. [desktop](quick-open-desktop-1440x900.png)
- Designer verdict — HistoryPicker: **pass after 2 shared iterations** — pinned,
  redacted, metadata, and active states retain disciplined hierarchy. [desktop](history-picker-desktop-1440x900.png)
- Designer verdict — FilePicker: **pass after 2 shared iterations** — hierarchy,
  preview layering, error tone, and tinted row focus remain legible. [desktop](file-picker-desktop-1440x900.png)
- Designer verdict — Select: **pass after 2 shared iterations** — trigger, group,
  checked, disabled, and active states are distinct without reverse video. [desktop](select-desktop-1440x900.png)
- Designer verdict — MultiSelect: **pass after 2 shared iterations** — checked
  state stays independent from active tint; filter and hint tiers remain calm. [desktop](multi-select-desktop-1440x900.png)

Each component also has adjacent `mobile-375x812`, `tablet-768x1024`, and
`paper-reduced-motion-1440x900` screenshots in this directory.

Designer verdict — shared SelectionTint cascade: **pass after 2 iterations** —
the corrected label recipe affected 166 catalog components. All 135 directly
mapped generated component pages were browser-validated at mobile, tablet,
desktop, and paper/reduced-motion; keyboard previews responded, page overflow
and page errors stayed zero. Designer sampling across data, navigation, forms,
dialogs, agent surfaces, and virtualized views confirmed the tint remains a
quiet secondary plane while gutters/text preserve focus. The full 540-image
review set is in [selection-tint-cascade](selection-tint-cascade/); grouped
catalog entries without standalone pages are covered by their generated frames
and owning composite pages.
