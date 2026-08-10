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
| 11 | chart-bar-default | covered | `BarSeries` / `Histogram` | Solid bars + labels |
| 12 | chart-bar-horizontal | covered | `BarSeries` | Horizontal track |
| 13 | chart-bar-multiple | covered | multi `BarDatum` | |
| 14 | chart-bar-stacked | covered | `BarDatum::stacked` (0253) | Multi-segment bands, not SegmentedMeter alone |
| 15 | chart-bar-label | covered | bar labels | |
| 16 | chart-bar-label-custom | partial | host label strings | registry id label-custom |
| 17 | chart-bar-mixed | covered | mixed magnitudes | |
| 18 | chart-bar-active | covered | `.selected` | |
| 19 | chart-bar-negative | covered | bipolar domain + zero tick (0253) | |
| 20 | chart-bar-interactive | covered | selection | |
| 21 | chart-line-default | covered | `Chart` / `Sparkline` | Nearest columns |
| 22 | chart-line-linear | covered | `Chart::linear()` (0254) | Lerp between samples |
| 23 | chart-line-step | covered | `Chart::step()` (0254) | Hold floor sample |
| 24 | chart-line-multiple | covered | multi `ChartSeries` | Distinct markers on plot |
| 25 | chart-line-dots | covered | series markers | |
| 26 | chart-line-dots-custom | partial | glyph ladder | registry id dots-custom |
| 27 | chart-line-dots-colors | covered | series roles | No-color markers |
| 28 | chart-line-label | covered | title/legend | |
| 29 | chart-line-label-custom | partial | host strings | registry label-custom |
| 30 | chart-line-interactive | covered | selection highlight | |
| 31 | chart-pie-simple | covered | `SegmentedMeter` | Proportional track |
| 32 | chart-pie-separator-none | covered | continuous (default) | |
| 33 | chart-pie-label | covered | `show_labels` | |
| 34 | chart-pie-label-custom | partial | host strings | registry label-custom |
| 35 | chart-pie-label-list | partial | host list | |
| 36 | chart-pie-legend | covered | labels | |
| 37 | chart-pie-donut | partial | linear track | True circular hole N/A |
| 38 | chart-pie-donut-active | covered | `.selected` (0255) | |
| 39 | chart-pie-donut-text | covered | `.center` (0255) | |
| 40 | chart-pie-stacked | partial | nested meters host | |
| 41 | chart-pie-interactive | covered | `.selected` | |
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
| covered | 36 |
| partial | 18 |
| missing | 0 |
| N/A | 14 |
| **Total** | **68** |

## Port decisions (0252)

| Gap | Decision |
|-----|----------|
| Area / stacked area | `ChartFill::Area` / `AreaStacked` + `Chart::area()` / `area_stacked()` |
| Line / multi-series / selection | Existing `Chart` |
| Bar / histogram | `BarSeries` / `Histogram`; stacked + negative on BarSeries (0253) |
| Pie proportions | `SegmentedMeter` + selection/center (0255) |
| Radial | `Gauge` |
| Radar polar | N/A |
| Continuous gradient | N/A |

## Validation

```bash
rtk cargo test -p termrock --lib charts
rtk cargo check -p termrock
```
