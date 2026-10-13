# Responsive and contraction system

| Field | Value |
|-------|-------|
| **Status** | Binding |
| **Migration** | `0090-v0.13.0-responsive-contraction.md` |
| **Module** | `layout::responsive` |
| **Studio** | `responsive/ladder-inspector` |

## Preserve / migrate / split / delete

| Surface | Fate |
|---------|------|
| `ContentPriority` / `ContractionStage` / `AdaptiveAnatomy` | **Preserve** (core law) |
| `ResponsiveSurface` + `SurfaceResponsivePolicy` | **Preserve** |
| `contract_parts` / anatomy helpers | **Preserve** |
| Magic `width < N` in Table/Dialog/Tabs | **Migrate** → surface helpers |
| Global CSS-like breakpoints | **Forbidden** — use **recipes** or surface policy |
| Ad-hoc truncation without priority | **Forbidden** |

## Mission

Terminal-native responsive anatomy: parts declare **Essential / Important /
Optional / Decorative**; contraction stages drop lowest first; surfaces and
**named recipes** define width/height bands — not one global media query.

## Progression

1. Full → 2. Compact spacing → 3. Shorten secondary → 4. Hide optional meta  
5. Collapse secondary actions → 6. Single pane → 7. Drawer/overlay → 8. Line mode  

## API (premium)

```rust
WIDTH_LADDER / HEIGHT_LADDER
ContentPriority / ContractionStage / AdaptiveAnatomy
ResponsiveSurface::classify / form_columns / anatomy_for_width
ResponsiveRecipe::{DEFAULT, AGENT_SHELL, DATA_DENSE}
Breakpoint / OverflowAction
ResponsiveSnapshot::for_surface / for_recipe / lines()
contract_parts / table_row_shows_optional / dialog_stack_actions
tabs_show_status_glyphs
```

## Widget migrations

| Widget | Change |
|--------|--------|
| Form | `form_columns` + multi_pane / line_mode gate |
| Table | leading/badge via `table_row_shows_optional` |
| Dialog (Choice) | action stack from measured `ActionBar` width vs real content slot |
| Tabs | status glyphs via `tabs_show_status_glyphs` |

## Laws

1. Essential labels/actions survive through line-mode.
2. Optional drops before important; decorative first.
3. Breakpoints are **recipe- or surface-local**, not global CSS.
4. Height can force line-mode / single-pane independently of width.
