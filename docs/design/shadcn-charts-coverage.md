# shadcn/ui charts → TermRock TUI coverage

**Source:** [ui.shadcn.com/charts](https://ui.shadcn.com/charts) (+ `/charts/area`, bar, line, pie, radar, radial, tooltip)  
**Locked crawl:** 2026-08 (registry-style `chart-{family}-{variant}` ids; live fetch may be SSRF-blocked).

**TermRock surface:** `widgets::charts` — `Sparkline`, `Chart` (+ **`ChartFill` area/stacked**, migration **0252**), `BarSeries`, `Histogram`, `Gauge`, `SegmentedMeter`.

**Statuses:** `covered` | `partial` | `missing` | `N/A`

## Notes

- Official demos are Recharts chrome variants of the same terminal jobs (scale, multi-series, fill, selection). Terminal physics collapse many onto one peer + builder flags.
- Host owns network data, floating HTML tooltips, continuous gradients.
- **Radar** polar geometry is **N/A** (no honest cell polar). Use multi-metric bars.
- **Tooltip** demos → selected index / focus highlight (keyboard, not hover-only).

## Matrix (68 locked demos)

| # | Demo id | Status | TermRock surface(s) | Notes |
|---|---------|--------|---------------------|-------|
| 1 | chart-area-default | covered | `Chart::area()` | Fill under series |
| 2 | chart-area-linear | covered | `Chart` + area | Linear samples host-projected |
| 3 | chart-area-step | partial | `Chart` + area | Step interpolation host; column samples |
| 4 | chart-area-legend | covered | `show_legend` | |
| 5 | chart-area-stacked | covered | `Chart::area_stacked()` | Cumulative domain |
| 6 | chart-area-stacked-expand | partial | `area_stacked` | % expand = host normalize |
| 7 | chart-area-icons | partial | legend markers | Icon chrome host |
| 8 | chart-area-gradient | N/A | none | Continuous CSS gradient theater |
| 9 | chart-area-axes | covered | `show_axes` | |
| 10 | chart-area-interactive | covered | `selected_series` / `selected_index` | Keyboard selection |
| 11 | chart-bar-default | covered | `BarSeries` / `Histogram` | |
| 12 | chart-bar-horizontal | covered | horizontal histogram/bars | |
| 13 | chart-bar-multiple | covered | multi `BarDatum` | |
| 14 | chart-bar-stacked | covered | `SegmentedMeter` / stacked bars | |
| 15 | chart-bar-label | covered | bar labels | |
| 16 | chart-bar-custom-label | partial | host label strings | |
| 17 | chart-bar-mixed | covered | mixed values | |
| 18 | chart-bar-active | covered | `.selected` | |
| 19 | chart-bar-negative | partial | host scale for negatives | |
| 20 | chart-bar-interactive | covered | selection | |
| 21 | chart-line-default | covered | `Chart` / `Sparkline` | |
| 22 | chart-line-linear | covered | `Chart` | |
| 23 | chart-line-step | partial | sample columns | Step curve host |
| 24 | chart-line-multiple | covered | multi `ChartSeries` | |
| 25 | chart-line-dots | covered | series markers | |
| 26 | chart-line-custom-dots | partial | glyph ladder | |
| 27 | chart-line-dots-colors | covered | series roles | No-color uses markers |
| 28 | chart-line-label | covered | title/legend | |
| 29 | chart-line-custom-label | partial | host strings | |
| 30 | chart-line-interactive | covered | selection | |
| 31 | chart-pie-simple | covered | `SegmentedMeter` | Proportions in cells |
| 32 | chart-pie-separator-none | covered | continuous meter | |
| 33 | chart-pie-label | covered | segment labels | |
| 34 | chart-pie-custom-label | partial | host | |
| 35 | chart-pie-label-list | partial | legend | |
| 36 | chart-pie-legend | covered | labels | |
| 37 | chart-pie-donut | partial | gauge + hole metaphor | True donut N/A |
| 38 | chart-pie-donut-active | partial | selected segment | |
| 39 | chart-pie-donut-text | partial | center label host | |
| 40 | chart-pie-stacked | partial | nested meters | |
| 41 | chart-pie-interactive | covered | selection | |
| 42 | chart-radar-default | N/A | multi-metric bars | Polar radar not TUI-honest |
| 43 | chart-radar-dots | N/A | multi-metric bars | |
| 44 | chart-radar-lines-only | N/A | multi-metric bars | |
| 45 | chart-radar-label-custom | N/A | multi-metric bars | |
| 46 | chart-radar-grid-none | N/A | multi-metric bars | |
| 47 | chart-radar-grid-circle | N/A | multi-metric bars | |
| 48 | chart-radar-grid-circle-fill | N/A | multi-metric bars | |
| 49 | chart-radar-grid-circle-nofill | N/A | multi-metric bars | |
| 50 | chart-radar-grid-fill | N/A | multi-metric bars | |
| 51 | chart-radar-grid-custom | N/A | multi-metric bars | |
| 52 | chart-radar-multiple | N/A | multi-metric bars | |
| 53 | chart-radar-legend | N/A | multi-metric bars | |
| 54 | chart-radar-icons | N/A | multi-metric bars | |
| 55 | chart-radial-simple | covered | `Gauge` | |
| 56 | chart-radial-label | covered | gauge label | |
| 57 | chart-radial-grid | partial | thresholds | |
| 58 | chart-radial-text | covered | unit/label | |
| 59 | chart-radial-shape | partial | glyph fill | Custom SVG shapes N/A |
| 60 | chart-radial-stacked | partial | segmented gauge host | |
| 61 | chart-tooltip-default | covered | `selected_index` highlight | Keyboard focus peer |
| 62 | chart-tooltip-indicator-line | partial | threshold lines | |
| 63 | chart-tooltip-indicator-none | covered | selection only | |
| 64 | chart-tooltip-label-none | covered | selection | |
| 65 | chart-tooltip-label-formatter | partial | host formats | |
| 66 | chart-tooltip-formatter | partial | host | |
| 67 | chart-tooltip-icons | partial | markers | |
| 68 | chart-tooltip-advanced | partial | host overlay | No hover DOM |

## Counts

| Status | Count |
|--------|------:|
| covered | 32 |
| partial | 22 |
| missing | 0 |
| N/A | 14 |
| **Total** | **68** |

## Port decisions (0252)

| Gap | Decision |
|-----|----------|
| Area / stacked area | `ChartFill::Area` / `AreaStacked` + `Chart::area()` / `area_stacked()` |
| Line / multi-series / selection | Existing `Chart` |
| Bar / histogram | `BarSeries` / `Histogram` |
| Pie proportions | `SegmentedMeter` |
| Radial | `Gauge` |
| Radar polar | N/A |
| Continuous gradient | N/A |

## Validation

```bash
rtk cargo test -p termrock --lib charts
rtk cargo check -p termrock
```
