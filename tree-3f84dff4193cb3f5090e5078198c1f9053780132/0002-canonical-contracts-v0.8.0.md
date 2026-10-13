# 0002 — Canonical contracts (`v0.8.0`)

Apply [migration 0001](0001-canonical-namespaces-v0.7.0.md) first.

## Before

The first canonical widgets still left several reusable contracts in consumers:
dialog axes shared one margin policy, dialog presentation could bypass semantic
theme roles, Toast callers calculated placement, indexed pickers managed their
own selection arithmetic, and consumers manually split bottom chrome rows.

## After

TermRock owns these domain-neutral contracts completely.

| Before | After |
|---|---|
| one dialog margin concept | `DialogSpec::{horizontal_margin, vertical_margin}` |
| caller-styled dialog shell | `Dialog` with `Theme` and semantic `PanelEmphasis` |
| caller-positioned toast rectangle | `Toast::new(theme, message, severity)` plus `.anchor(...)` and `.margins(...)` |
| local `usize` picker selection/cycling | `ListState::<usize>::for_count`, `cycle_index`, `move_index`, `reconcile_count`, and `selected_item` |
| local bottom-row arithmetic | `layout::bottom_rows` |
| local detail/status body geometry | canonical `DetailTable` and `StatusBar` rendering/state |

## Consumer actions

1. Replace dialog specifications with independent horizontal and vertical
   margins and pass the shared theme plus semantic emphasis to `Dialog`.
2. Delete toast sizing and anchoring helpers; construct `Toast` over the full
   outer area and let it resolve its rectangle.
3. Replace local indexed-picker clamping, wrapping, and selection helpers with
   `ListState<usize>` methods.
4. Replace `row_from_bottom`-style arithmetic with `layout::bottom_rows` so
   tiny-terminal collapse behavior is shared.
5. Delete local Toast, detail-table, status-footer, and neutral dialog-rendering
   copies. Retain only product-specific facts, wording, effects, and policy.

No compatibility facade is provided.
