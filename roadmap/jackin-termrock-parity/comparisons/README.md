# Comparison reports — jackin-termrock-parity

Old rev `5ff94ee117fd4a1b72fdd0d1b1847815055a93ac` vs HEAD `5bcaac4b`,
one report per jackin-used subset family. Produced by plan 008; verdict
slots are filled only by the user, via plan 009 (D1, D8).

| Family | Report | Compared | Uncomparable | HEAD-only | Verdict |
|--------|--------|----------|--------------|-----------|---------|
| ActionBar | [action-bar.md](action-bar.md) | 1 | 4 | 0 | pending |
| Backdrop | [backdrop.md](backdrop.md) | 1 | 2 | 0 | pending |
| ChoiceDialog | [choice-dialog.md](choice-dialog.md) | 1 | 2 | 0 | pending |
| DetailTable | [detail-table.md](detail-table.md) | 2 | 2 | 0 | pending |
| Dialog | [dialog.md](dialog.md) | 2 | 4 | 0 | pending |
| DiffView | [diff-view.md](diff-view.md) | 1 | 6 | 0 | pending |
| HintBar | [hint-bar.md](hint-bar.md) | 1 | 2 | 0 | pending |
| List | [list.md](list.md) | 3 | 12 | 0 | pending |
| MessageDialog | [message-dialog.md](message-dialog.md) | 1 | 2 | 0 | pending |
| Panel | [panel.md](panel.md) | 1 | 10 | 0 | pending |
| Progress | [progress.md](progress.md) | 3 | 4 | 0 | pending |
| StatusBar | [status-bar.md](status-bar.md) | 2 | 6 | 0 | pending |
| Tabs | [tabs.md](tabs.md) | 2 | 9 | 0 | pending |
| TextInput | [text-input.md](text-input.md) | 1 | 8 | 0 | pending |
| Toast | [toast.md](toast.md) | 2 | 5 | 0 | pending |
| Viewport | [viewport.md](viewport.md) | 1 | 2 | 0 | pending |

## Verdict syntax (shared contract with plan 009)

- Pending slot line, exact: `**Verdict**: _pending_`, followed by a
  comment line documenting the allowed values `merge | restore | accept`
  (merge = expected default).
- The user rules by replacing `_pending_` with exactly one value —
  `**Verdict**: merge` (or `restore` / `accept`) — nothing else on the
  line.
- After application, plan 009 appends an `**Applied**: <date>` line below
  the verdict line; plan 008 never writes it.
- Machine detection: pending = `^\*\*Verdict\*\*: _pending_`;
  ruled = `^\*\*Verdict\*\*: (merge|restore|accept)$`.

## Completion checklist

- [ ] 16 reports exist (one per subset family)
- [ ] every report has ≥1 compared pair or the explicit all-uncomparable note
- [ ] every image reference resolves to a committed file
- [ ] every byte-differing pair's block names ≥1 classified difference and
      never claims "No visible differences."
- [ ] zero verdict slots filled — every report matches the pending form
      above and none the ruled form (verdicts are the user's, D1)
