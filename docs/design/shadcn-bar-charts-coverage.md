# shadcn/ui bar charts → TermRock TUI coverage

**Source:** [ui.shadcn.com/charts/bar](https://ui.shadcn.com/charts/bar)  
**Locked crawl:** 2026-08 (registry-style `chart-bar-*`; live fetch may be SSRF-blocked).

**TermRock surface:** `BarSeries` / `BarDatum` (+ **stacked segments** + **bipolar negative**, migration **0253**), `Histogram` (vertical).

**Statuses:** `covered` | `partial` | `missing` | `N/A`

## Notes

- Web demos differ mainly by Recharts chrome (active style, custom label components, animation).
- Terminal bars: categorical rows, scale, selection, stacked multi-segment, bipolar negatives with zero tick.
- Continuous gradients / hover tooltips → N/A or host.

## Matrix

| # | Demo id | Status | TermRock surface(s) | Notes |
|---|---------|--------|---------------------|-------|
| 1 | chart-bar-default | covered | `BarSeries` / `Histogram` vertical | Solid bars + labels |
| 2 | chart-bar-horizontal | covered | `BarSeries` (horizontal) | Default orientation |
| 3 | chart-bar-multiple | covered | multi `BarDatum` rows | One series of categories |
| 4 | chart-bar-stacked | covered | `BarDatum::stacked` + multi-segment paint | Distinct segment glyphs |
| 5 | chart-bar-label | covered | bar labels left of track | |
| 6 | chart-bar-custom-label | partial | host label strings | React custom label N/A |
| 7 | chart-bar-mixed | covered | mixed positive magnitudes | Scale Auto |
| 8 | chart-bar-active | covered | `.selected(i)` bold fill | |
| 9 | chart-bar-negative | covered | bipolar domain + zero tick + neg glyph | **0253** |
| 10 | chart-bar-interactive | covered | selection | Keyboard host selects index |

## Counts

| Status | Count |
|--------|------:|
| covered | 9 |
| partial | 1 |
| missing | 0 |
| N/A | 0 |
| **Total** | **10** |

## Port decisions (0253)

| Gap | Decision |
|-----|----------|
| Stacked multi-segment | `BarDatum::stacked(label, &[..])` solid segments with series markers |
| Negative / bipolar | Domain includes 0; fill left/right of zero tick |
| Vertical columns | Existing `Histogram` |
| Custom React labels | partial |

**Consolidated charts SoT:** update bar rows in `shadcn-charts-coverage.md` to match.

## Validation

```bash
rtk cargo test -p termrock --lib bar_series
rtk cargo test -p termrock --lib charts
rtk cargo check -p termrock
```
