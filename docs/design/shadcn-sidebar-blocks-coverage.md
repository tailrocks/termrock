# shadcn/ui sidebar blocks → TermRock TUI coverage

**Source:** [ui.shadcn.com/blocks/sidebar](https://ui.shadcn.com/blocks/sidebar)  
**Locked crawl:** 2026-08 (web snippets; live fetch may be SSRF-blocked).

**TermRock surface:** `widgets::Sidebar` / `NavigationList` / `NavItem` (+ AppShell
rail slots). Collapse filter: [`filter_nav_collapsed`] (migration **0250**).

**Statuses:** `covered` | `partial` | `missing` | `N/A`

## Notes

- Web variants differ mainly by layout chrome (floating, inset, dropdown
  flyouts). Terminal physics collapse them onto one **keyboard-first rail**:
  sectioned items, route ≠ focus, collapsible groups, rail/drawer presentation.
- Host owns routes, deep-linking, and open-section persistence after
  `ExpandToggled`.
- Continuous floating CSS shadow / hover-only flyouts without keyboard parity
  are **partial** or host overlay (`ContextMenuRequested` / `OpenDrawer`).

## Matrix

| # | shadcn block | Status | TermRock surface(s) | Notes |
|---|--------------|--------|---------------------|-------|
| 1 | sidebar-01 — sectioned nav | covered | `Sidebar` + `NavItem::section` / `example_sectioned_sidebar_nav` | Groups by section |
| 2 | sidebar-02 — collapsible sections | covered | `ExpandToggled` + `filter_nav_collapsed` | Left/Right / Enter on section |
| 3 | sidebar-03 — submenus | covered | Nested `depth` + group expand | Keyboard nested leaves |
| 4 | sidebar-04 — floating + submenus | partial | `SidebarPresentation::Drawer` / overlay | No CSS float; drawer peer |
| 5 | sidebar-05 — collapsible submenus | covered | Group collapse + filter | Host updates `expanded` |
| 6 | sidebar-06 — submenus as dropdowns | partial | `ContextMenuRequested` (Ctrl+M / Shift+Space) | Host paints menu overlay |
| 7 | sidebar-07 — collapse to icons | covered | `SidebarPresentation::Rail` / `[` toggle / `apply_width` | Icon rail |

## Counts

| Status | Count |
|--------|------:|
| covered | 5 |
| partial | 2 |
| missing | 0 |
| N/A | 0 |
| **Total** | **7** |

## Port decisions

| Gap | Decision |
|-----|----------|
| Sectioned + collapsible + nested | Existing Sidebar; harden `filter_nav_collapsed` (0250) |
| Icon rail | `ToggleRail` / width policy |
| Floating / dropdown chrome | partial → Drawer / ContextMenuRequested host overlay |
| Dual Sidebar paint fork | Forbidden |

## Validation

```bash
rtk cargo test -p termrock --lib sidebar
rtk cargo check -p termrock
```
