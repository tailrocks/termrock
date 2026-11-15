# Showcase sidebar refresh (#14)

The five pages that `HIDDEN_ENTRIES` kept out of the sidebar (Fixed controls,
Fixed dialogs, List detail, Terminal pane, Terminal viewport) now sit in their
groups in `NAV_ENTRIES`. Two deliberate visual changes follow in every Showcase
capture that shows the sidebar: the Components count rises from 54 to 59, and an
expanded group lists its new pages. A taller sidebar also grows a scrollbar in
short terminals. Captures that hide the sidebar do not change.

Each baseline was accepted by `scripts/accept-baseline.py` in candidate mode from
the exact sample of a current-source functional run, with the expected baseline
digest enforced by `tuiscotti accept`. Each accepted sample differs from the
baseline it replaces only in the sidebar column and the page header, which was
checked cell by cell with `tuiscotti check` against the superseded sample. For
every accepted capture:

- `<test_id>.accept.json` is the Tuiscotti transaction receipt.
- `<test_id>.provenance.json` is the acceptance body.
- `superseded/<path>.tui.pb` is the baseline it replaced.
- `docs/testing/baseline-refreshes-20261108.tsv` has one appended chain row, and
  `docs/testing/binary-baselines-20261108.tsv` points at the new receipt where the
  test is one of the 105 binary baselines.

## Merge with origin/train/all

Merging origin/train/all into the #14 branch conflicted on 133 baselines that
both sides had moved. Each took train/all's version, and #14's receipts for those
captures were dropped. Where the merged tree still draws the sidebar in a
capture, the baseline was accepted again from a fresh run of the merged tree,
with the same procedure, 135 captures in all (a test that checks several captures needs one run per capture), and
`baseline-refreshes-20261108.tsv` lists each. Every accepted sample
differs from train/all's baseline only in sidebar columns 0 to 23 (18 at 80x24),
checked cell by cell with `tuiscotti check`. The one exception,
`functional/showcase_page_finder`, is accepted under
`showcase-sidebar-14-merge-20261116`: train/all's Finder page (#257) draws a
different body than train/all's baseline holds, so that sample carries #257's
body and #14's sidebar together.
