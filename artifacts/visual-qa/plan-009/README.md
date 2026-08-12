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
