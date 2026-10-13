# shadcn/ui radial charts → TermRock TUI coverage

**Source:** [ui.shadcn.com/charts/radial](https://ui.shadcn.com/charts/radial)  
**Locked crawl:** 2026-08-11 live page + registry (HTTP 200). Exact **6** `chart-radial-*` ids (all registry 200).

**TermRock surface:** `Gauge` (single-value progress + label/unit + thresholds), `BarSeries` (multi-category radial bars → linear category bars), `SegmentedMeter` (stacked radial part-to-whole). Linear / block-glyph peers — **not** SVG circular rings.

**Statuses:** `covered` | `partial` | `missing` | `N/A`

## Notes

- Recharts `RadialBarChart` arcs/polar grids are not TUI-honest; map jobs to linear gauges, category bars, and segmented tracks.
- Multi-item radial bars (simple/label/grid) compare categories under one domain → `BarSeries` (or multi-row gauges).
- Single-value radial (text/shape) → `Gauge` with label/unit and capability-aware fill.
- Stacked radial (two series on one arc) → `SegmentedMeter` proportional segments + optional center caption.
- Polar grid chrome / custom arc endAngle shapes stay residual.

## Matrix

| # | Demo id | Status | TermRock surface(s) | Notes |
|---|---------|--------|---------------------|-------|
| 1 | chart-radial-simple | covered | `BarSeries` / multi `Gauge` rows | Multi-category values; linear bars peer |
| 2 | chart-radial-label | covered | `BarSeries` labels / `Gauge::label` | Category labels on track |
| 3 | chart-radial-grid | partial | `Gauge::thresholds` ticks | PolarGrid circle chrome N/A |
| 4 | chart-radial-text | covered | `Gauge` + `.label` / `.unit` | Center-text job → value/unit paint |
| 5 | chart-radial-shape | partial | glyph ladder fill | Custom SVG endAngle/shape N/A |
| 6 | chart-radial-stacked | covered | `SegmentedMeter` | Stacked series → part-to-whole segments |

## Counts

| Status | Count |
|--------|------:|
| covered | 4 |
| partial | 2 |
| missing | 0 |
| N/A | 0 |
| **Total** | **6** |

## Port decisions

| Gap | Decision |
|-----|----------|
| Single-value progress | `Gauge::percent` / `Gauge::new` + scale |
| Label / unit text | `Gauge::label` · `Gauge::unit` |
| Threshold “grid” marks | `Gauge::thresholds` tick glyphs (polar grid N/A) |
| Multi-category radial bars | `BarSeries` shared domain |
| Stacked radial | `SegmentedMeter` + honest width alloc (0255) |
| Custom circular shapes | **partial** — capability glyphs only |
| Continuous circular SVG | **N/A** (not claimed covered) |

**Consolidated charts SoT:** radial rows in `shadcn-charts-coverage.md` match this matrix.

## Validation

```bash
rtk cargo test -p termrock --lib gauge_
rtk cargo test -p termrock --lib segmented_meter
rtk cargo test -p termrock --lib bar_series
rtk cargo check -p termrock
```
