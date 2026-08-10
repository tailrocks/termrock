# shadcn/ui line charts → TermRock TUI coverage

**Source:** [ui.shadcn.com/charts/line](https://ui.shadcn.com/charts/line)  
**Locked crawl:** 2026-08 (registry-style `chart-line-*`; live fetch may be SSRF-blocked).

**TermRock surface:** `Chart` / `ChartSeries` / `Sparkline` (+ **`ChartInterpolation`** linear/step, migration **0254**).

**Statuses:** `covered` | `partial` | `missing` | `N/A`

## Notes

- Web demos differ mainly by Recharts chrome (dot style, custom labels, animation).
- Terminal line jobs: multi-series markers, legend/axes, selection highlight, missing samples, linear vs step X mapping.
- Continuous CSS gradients / hover tooltips → N/A or host.

## Matrix

| # | Demo id | Status | TermRock surface(s) | Notes |
|---|---------|--------|---------------------|-------|
| 1 | chart-line-default | covered | `Chart` / `Sparkline` | Nearest sample columns |
| 2 | chart-line-linear | covered | `Chart::linear()` | Lerp between samples (0254) |
| 3 | chart-line-step | covered | `Chart::step()` | Hold floor sample (0254) |
| 4 | chart-line-multiple | covered | multi `ChartSeries` | Distinct series markers |
| 5 | chart-line-dots | covered | series markers | Multi-row plot glyphs |
| 6 | chart-line-dots-custom | partial | glyph ladder / host | registry id dots-custom (not custom-dots) |
| 7 | chart-line-dots-colors | covered | series roles + no-color markers | |
| 8 | chart-line-label | covered | title / legend | |
| 9 | chart-line-label-custom | partial | host strings | registry `label-custom` spelling |
| 10 | chart-line-interactive | covered | `selected_series` / `selected_index` | Keyboard highlight |

## Counts

| Status | Count |
|--------|------:|
| covered | 8 |
| partial | 2 |
| missing | 0 |
| N/A | 0 |
| **Total** | **10** |

## Port decisions (0254)

| Gap | Decision |
|-----|----------|
| Linear vs step path | `ChartInterpolation::{Linear,Step}` + `Chart::linear()` / `step()` |
| Multi-series + selection | Existing Chart; honest occupancy tests |
| Custom dots / labels | partial host |

**Consolidated SoT:** line rows in `shadcn-charts-coverage.md` aligned.

## Validation

```bash
rtk cargo test -p termrock --lib charts
rtk cargo check -p termrock
```
