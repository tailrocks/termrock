# shadcn/ui pie charts → TermRock TUI coverage

**Source:** [ui.shadcn.com/charts/pie](https://ui.shadcn.com/charts/pie)  
**Locked crawl:** 2026-08 (registry-style `chart-pie-*`; live fetch may be SSRF-blocked).

**TermRock surface:** `SegmentedMeter` / `MeterSegment` (+ selection, center caption, separators, honest width alloc — migration **0255**). Linear part-to-whole track; **not** SVG pie arcs.

**Statuses:** `covered` | `partial` | `missing` | `N/A`

## Notes

- True circular pies / donut rings are not TUI-honest; proportions map to a full-width segmented track.
- Hover tooltips / Recharts animation → host or keyboard selection highlight.
- Registry spellings preserved (e.g. not swapped name order).

## Matrix

| # | Demo id | Status | TermRock surface(s) | Notes |
|---|---------|--------|---------------------|-------|
| 1 | chart-pie-simple | covered | `SegmentedMeter` | Proportional multi-segment track |
| 2 | chart-pie-separator-none | covered | continuous (default) | No gaps between segments |
| 3 | chart-pie-label | covered | `show_labels(true)` | Label row under bar |
| 4 | chart-pie-label-custom | partial | host label strings | React custom label N/A |
| 5 | chart-pie-label-list | partial | host list + labels | Side list layout host |
| 6 | chart-pie-legend | covered | labels / host legend | Segment labels |
| 7 | chart-pie-donut | partial | center caption metaphor | True circular hole N/A |
| 8 | chart-pie-donut-active | covered | `.selected(i)` | Active segment highlight |
| 9 | chart-pie-donut-text | covered | `.center("…")` | Center caption under track |
| 10 | chart-pie-stacked | partial | nested meters host | Multi-ring stack host |
| 11 | chart-pie-interactive | covered | `.selected(i)` | Keyboard-driven highlight |

## Counts

| Status | Count |
|--------|------:|
| covered | 7 |
| partial | 4 |
| missing | 0 |
| N/A | 0 |
| **Total** | **11** |

## Port decisions (0255)

| Gap | Decision |
|-----|----------|
| Proportions + full track | `allocate_segment_widths` + SegmentedMeter |
| Zero-weight | 0 columns (no invented mass) |
| Selection / active | selected glyph `X` + bold/reversed |
| Donut text | `center` caption on row 2 |
| Separators optional | `separators(true)` vs continuous default |
| True SVG pie/donut | partial / N/A |

## Validation

```bash
rtk cargo test -p termrock --lib segmented_meter
rtk cargo test -p termrock --lib allocate_segment
rtk cargo check -p termrock
```
