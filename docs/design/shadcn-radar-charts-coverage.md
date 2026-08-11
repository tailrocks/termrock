# shadcn/ui radar charts → TermRock TUI coverage

**Source:** [ui.shadcn.com/charts/radar](https://ui.shadcn.com/charts/radar)  
**Locked crawl:** 2026-08-11 live page + registry (HTTP 200). Page lists **12** `chart-radar-*` demos; registry also serves **`chart-radar-icons`** (200) → **13** locked ids. Registry spelling: `chart-radar-grid-circle-no-lines` (not `…-nofill`, 404).

**TermRock surface:** `MetricRadar` / `MetricAxis` / `MetricSeries` (migration **0256**). Multi-axis comparison as **grouped horizontal bars per metric** — not polar SVG radar.

**Statuses:** `covered` | `partial` | `missing` | `N/A`

## Notes

- True polar radar / radial grid / filled spider polygons are not TUI-honest (cell grid, no SVG).
- Job preserved: compare several metrics across one or more series with shared scale, legend, selection.
- Each axis is a labeled row; each series owns a horizontal bar slot on that row (grouped).
- Monochrome uses series markers (not color alone). Missing values paint the shared missing glyph.

## Matrix

| # | Demo id | Status | TermRock surface(s) | Notes |
|---|---------|--------|---------------------|-------|
| 1 | chart-radar-default | covered | `MetricRadar` + axes/series | Grouped bars per metric axis |
| 2 | chart-radar-dots | covered | legend + monochrome `series_marker` | Dot peer = marker glyphs, not polar dots |
| 3 | chart-radar-lines-only | partial | bars (not polylines); host `Chart` for line series | Spider lines N/A |
| 4 | chart-radar-label-custom | partial | axis `label` strings; host custom chrome | React custom label N/A |
| 5 | chart-radar-grid-none | N/A | — | Polar grid chrome not ported |
| 6 | chart-radar-grid-circle | N/A | — | Circular grid N/A |
| 7 | chart-radar-grid-circle-fill | N/A | — | Filled circular grid N/A |
| 8 | chart-radar-grid-circle-no-lines | N/A | — | Registry spelling (was nofill 404) |
| 9 | chart-radar-grid-fill | N/A | — | Polar fill N/A |
| 10 | chart-radar-grid-custom | N/A | — | Custom polar grid N/A |
| 11 | chart-radar-multiple | covered | multi `MetricSeries` | Grouped bar slots per series |
| 12 | chart-radar-legend | covered | `.show_legend(true)` default | Marker + label row |
| 13 | chart-radar-icons | partial | series markers as icon peer | On registry (200); React icons N/A |

## Counts

| Status | Count |
|--------|------:|
| covered | 4 |
| partial | 3 |
| missing | 0 |
| N/A | 6 |
| **Total** | **13** |

## Port decisions (0256)

| Gap | Decision |
|-----|----------|
| Multi-metric comparison | `MetricRadar` axes × series |
| Shared scale | `ScaleMode::Auto` / `Fixed` via existing `resolve_domain` |
| Multi-series | Grouped horizontal slots + gap |
| Legend / dots | Legend markers; monochrome series_marker fill |
| Selection | `selected_axis` / `selected_series` (ASCII `X`) |
| Missing | NaN → missing glyph |
| Polar grid / spider / fill | **N/A** — no polar theater |

**Consolidated charts SoT:** update radar rows in `shadcn-charts-coverage.md` to match.

## Validation

```bash
rtk cargo test -p termrock --lib metric_radar
rtk cargo check -p termrock
```
