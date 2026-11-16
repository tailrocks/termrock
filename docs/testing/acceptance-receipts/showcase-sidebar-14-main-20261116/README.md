# Showcase sidebar refresh, merge with main (#14)

Merging main 986cd378 into the #14 branch conflicted on the five Finder baselines
that main's hosted-Finder work (#64) moved: `functional/showcase_page_finder`,
`pre_refactor/s09_finder/sc_finder_options`, `_120x40`, `_80x24` and the new
`pre_refactor/s09_finder/opt_finder_painter`. Each took main's version. #14's own
receipts for the four it had already refreshed were dropped, because they were
accepted against the older body, and the five were accepted again from a fresh run
of the merged tree.

Each accepted sample differs from main's baseline in exactly one cell: the last
digit of the Components count (54 to 59) in the sidebar header, row 2, column 23
(column 18 at 80x24). `tuiscotti check` lists that cell, and `scripts/accept-baseline.py`
re-ran the check before it accepted. The Finder body is main's.

Per capture, `<test_id>.accept.json` is the Tuiscotti transaction receipt,
`<test_id>.provenance.json` is the acceptance body, and `superseded/<path>.tui.pb`
is the baseline it replaced. `docs/testing/baseline-refreshes-20261108.tsv` has one
appended chain row each, and `docs/testing/binary-baselines-20261108.tsv` points
`functional/showcase_page_finder` and `opt_finder_painter` at the new receipt.
