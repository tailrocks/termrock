# Responsive layout system

**Status:** implemented foundation (migration `0044`)  
**Rule:** Responsive TUI design is **not** “truncate every label.”  
**Law:** Primary labels and primary actions survive longer than decorative or secondary information.

## Priority tiers

| Tier | Role | Survival |
|------|------|----------|
| **Essential** | Primary labels, primary actions | Always (while surface is shown) |
| **Important** | Key columns, secondary labels, secondary panes | Until line-mode |
| **Optional** | Badges, shortcuts, tertiary columns, meta | Full → shorten secondary |
| **Decorative** | Flourish chrome | Full → compact only |

## Contraction progression

1. **Full anatomy** — all parts, comfortable density  
2. **Compact spacing** — tighter density tokens  
3. **Shortened secondary labels** — abbreviate non-primary text  
4. **Hidden low-priority metadata** — drop optional meta  
5. **Collapsed secondary actions** — keep primary actions only  
6. **Single-pane layout** — multi-pane / multi-column → one region  
7. **Drawer or overlay replacement** — docked secondary → drawer/overlay  
8. **Inline / line-mode fallback** — essential-only tiny terminal  

## Declared per component

- Essential / important / optional content  
- Secondary actions  
- Preferred size + minimum usable size (`SizeBudget`)  
- Contraction strategies (stage thresholds)  
- Alternative compact anatomy (`AdaptiveAnatomy`)  
- Tiny-terminal fallback (`LineMode`)  
- Overflow behavior (`Ellipsis` | `Clip` | `Scroll` | `Wrap` | `Hide`)  

## Rust API (summary)

```rust
use termrock::{
    AdaptiveAnatomy, AnatomyPart, ContentPriority, ContractionStage,
    ResponsiveSurface, ViewportClass, WIDTH_LADDER, contract_parts,
};

// Global width → stage
let stage = ContractionStage::from_width(80);

// Per-surface classification
let class = ResponsiveSurface::AppShell.classify(80, 24);
assert!(class.anatomy.essential);
assert!(!class.anatomy.optional_meta); // depends on surface thresholds

// Anatomy parts with priority
let parts = [
    AnatomyPart::essential("title", 24, 4),
    AnatomyPart::optional("badge", 6),
];
let shown = contract_parts(&parts, 40, class.anatomy);
// primary/essential retained; badge dropped when budget or stage requires

// Form columns
let cols = ResponsiveSurface::Form.form_columns(width);
```

## Surface policies

| Surface | Multi-pane | Drawer | Notes |
|---------|------------|--------|-------|
| App shell | yes | yes | Sidebar → drawer under pressure |
| Sidebar | no | yes | Collapses to overlay host |
| Tabs | no | no | Scroll overflow for labels |
| Table / Tree | no | no | Composed-row drop order |
| DataTable | no | no | Hide low-priority columns first |
| Form | yes (columns) | no | 2-col → 1-col |
| Dialog | no | yes | Narrow → fullscreen-ish |
| Command palette | no | yes | Fullscreen promote when tiny |
| Prompt composer | no | no | Height compresses first |
| Task rail | no | yes | Drawer on narrow shell |
| Permission prompt | no | yes | Primary Accept/Deny last |
| Plan review | yes | yes | Steps essential |
| Diff review | yes | no | Side-by-side → unified |
| Log viewer | no | no | Scroll overflow |
| Status bar | no | no | Primary slot survives |

## Width test matrix

Canonical samples: **160, 120, 100, 80, 60, 40, 20** (`WIDTH_LADDER`).

Invariants per cell:

1. `anatomy.essential == true`  
2. Stage severity is non-decreasing as width decreases  
3. Optional never outlives important  
4. Line-mode implies no secondary actions / no multi-pane  
5. `contract_parts` retains essential primary labels  

## Relationship to existing systems

| Existing | Role after this design |
|----------|------------------------|
| `Density` | Spacing tokens suggested by stage |
| `ComposedRow::parts_for_width` | Row-local drop order (aligned with optional→essential) |
| `PanelSlots::for_width` | Chrome slot contraction |
| `Workspace` collapse_priority | Pane-level single-pane step |
| `OverlayStack` narrow fallback | Drawer/overlay + fullscreen stages |

New work should **query** `ResponsiveSurface` / `ViewportClass` instead of inventing ad-hoc width cutoffs.
