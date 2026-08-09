# OverlayStack architecture

**Status:** implemented (migration `0043`)  
**Law:** Escape closes exactly one conceptual interaction layer.

## Problem

Dialogs, completion menus, popovers, command palettes, tooltips, context menus,
drawers, and fullscreen viewers must not each invent placement, focus,
dismissal, and input routing.

## Solution

[`OverlayStack`](../../crates/termrock/src/interaction/overlay_stack.rs) owns:

| Concern | Behavior |
|---------|----------|
| Z-order | Open order; last entry is top |
| Parent–child | `parent` id; dismissing a parent drops descendants |
| Placement | Prefer / flip / clamp / min–max size |
| Anchoring | Optional `anchor` rect; cover policy per kind |
| Collision / flip | Anchored menus flip above/left when needed |
| Screen-edge | Final clamp inside bounds |
| Backdrops | `BackdropPolicy::{None,Dim,Occlude}` on stack |
| Focus traps | Policy flag; pair with `InteractionScene` |
| Opener restore | `opener_focus` returned on dismiss |
| Keyboard | Top `owns_input` / `focus_trap` |
| Pointer / wheel | Outside-click + `wheel_captured` |
| Escape | Single-layer dismiss or trap/ignore |
| Nested dismiss | Children removed when parent dismissed |
| Fullscreen | Kind, narrow promote, or `promote_top_fullscreen` |
| Resize | `reflow(bounds)` re-places from stored size/anchor |
| Narrow terminal | `NarrowFallback::{Clamp,Center,Fullscreen,Hide}` |

## Policy kinds

Tooltip, Popover, Menu, ContextMenu, Completion, Select, Dialog, AlertDialog,
Drawer, CommandPalette, Fullscreen, Custom — see `OverlayPolicy::for_kind` and
migration `0043`.

## Consumer shell loop

1. `stack.open(bounds, OverlaySpec::…)` when opening chrome  
2. `stack.sync_scene_layers(&mut scene)` each frame after root  
3. Route Esc → `stack.handle_escape()` then scene / quit  
4. Route outside click → `stack.handle_outside_click`  
5. On resize → `stack.reflow(new_bounds)`  
6. Paint each `entry.rect` with widget payloads  

## Migrated widgets

CompletionMenu, CommandPalette, Dialog — helpers `place_*` / `open_*_overlay`.
