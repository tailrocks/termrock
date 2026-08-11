# shadcn/ui tooltip charts → TermRock TUI coverage

**Source:** [ui.shadcn.com/charts/tooltip](https://ui.shadcn.com/charts/tooltip)  
**Locked crawl:** 2026-08-11 live page + registry (HTTP 200). Exact **9** `chart-tooltip-*` ids (all registry 200), including **`chart-tooltip-label-custom`**.

**TermRock surface:** keyboard **selection / highlight** on `Chart` (`selected_series` + `selected_index`), `Sparkline::selected`, `BarSeries::selected`, plus `Chart::thresholds` / `Sparkline::threshold` as **indicator-line** peers. Floating HTML/Recharts hover tooltips are **not** ported.

**Statuses:** `covered` | `partial` | `missing` | `N/A`

## Notes

- Hover cursor + floating tooltip DOM → TUI-honest peer is focus/selection that changes in-chart paint.
- Host owns label/value formatting strings (formatters, custom label keys) — partial when React-only.
- Series icons → monochrome `series_marker` peer (partial vs React icon components).
- Advanced multi-line formatter + totals → host overlay / status line (partial).

## Matrix

| # | Demo id | Status | TermRock surface(s) | Notes |
|---|---------|--------|---------------------|-------|
| 1 | chart-tooltip-default | covered | `Chart::selected_series` + `selected_index` | Highlight sample (ASCII `X`) |
| 2 | chart-tooltip-indicator-line | covered | `Chart::thresholds` / `Sparkline::threshold` | Horizontal tick indicator line |
| 3 | chart-tooltip-indicator-none | covered | selection only | No threshold required |
| 4 | chart-tooltip-label-none | covered | selection highlight | Host may omit labels |
| 5 | chart-tooltip-label-formatter | partial | host formats selection chrome | React `labelFormatter` N/A |
| 6 | chart-tooltip-label-custom | partial | host label keys | React `labelKey` / chartConfig N/A |
| 7 | chart-tooltip-formatter | partial | host value format | React `formatter` N/A |
| 8 | chart-tooltip-icons | partial | `series_marker` monochrome | React icon components N/A |
| 9 | chart-tooltip-advanced | partial | host overlay / status | Floating multi-line total N/A |

## Counts

| Status | Count |
|--------|------:|
| covered | 4 |
| partial | 5 |
| missing | 0 |
| N/A | 0 |
| **Total** | **9** |

## Port decisions

| Gap | Decision |
|-----|----------|
| Inspect sample under keyboard | `selected_series` + `selected_index` changes glyph/style |
| Indicator line | Shared threshold marks across plot width |
| Indicator / label none | Selection alone still paints |
| Formatters / custom labels | Host responsibility → **partial** |
| Icons / advanced overlay | Markers + host chrome → **partial** |
| Floating Recharts hover | **Not covered** (no DOM) |

**Consolidated charts SoT:** tooltip rows in `shadcn-charts-coverage.md` match this matrix (9 ids).

## Validation

```bash
rtk cargo test -p termrock --lib chart_selection
rtk cargo test -p termrock --lib chart_threshold
rtk cargo test -p termrock --lib sparkline_autoscale
rtk cargo check -p termrock
```
