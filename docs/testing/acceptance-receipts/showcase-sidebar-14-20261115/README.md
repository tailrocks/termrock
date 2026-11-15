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
