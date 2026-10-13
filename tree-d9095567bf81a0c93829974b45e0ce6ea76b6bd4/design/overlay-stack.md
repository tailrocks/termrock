# OverlayStack (sole overlay authority)

| Field | Value |
|-------|-------|
| **Status** | Binding |
| **Migration** | `0087-v0.13.0-overlay-stack-premium.md` |
| **Module** | `interaction::overlay_stack` |
| **Studio** | `overlay/*` stories |

## Preserve / migrate / split / delete

| Surface | Fate |
|---------|------|
| `OverlayStack` / `OverlaySpec` / kinds / policies | **Preserve & expand** (queue, pointer route, placement diag) |
| `place_overlay` | **Preserve**; add `place_overlay_detailed` |
| `open_*_overlay` (Dialog, Completion, Palette, Drawer, …) | **Preserve** |
| Picker local popup geometry | **Migrate** → `open_picker_overlay` / Select kind |
| Public `ModalStack` | **Already deleted** (0065); crate-private only |
| Dual Esc handlers (scene + stack both dismiss) | **Forbidden** — stack first, then scene |

## Mission

One overlay system for dialogs, menus, completion, popovers, drawers, palettes,
and fullscreen viewers: z-order, anchors, placement (flip/clamp/collision),
focus traps, opener restore, backdrop, pointer/wheel routing, nested
dismissal, modal queue, fullscreen promotion, deterministic tiny-terminal
fallback. **Escape closes exactly one conceptual layer.**

## API (premium)

```rust
OpenMode::{Stack, Queue, Replace}
OverlayOutcome::{Ignored, Opened, Queued, Dismissed, UnhandledEscape}
PointerRoute::{Empty, Top, OutsideTop, Lower}
PlacementResult { rect, flipped_*, clamped, fullscreen_promoted, hidden }

stack.open(bounds, spec)                    // Stack
stack.open_with(bounds, spec, OpenMode::Queue)
stack.enqueue(bounds, spec)
stack.drain_queue() / drain_queue_all()
stack.handle_escape()                        // one layer
stack.handle_outside_click / handle_pointer / route_pointer
stack.promote_top_fullscreen / demote_top_fullscreen
stack.reflow(bounds)
stack.sync_scene_layers(scene)

place_overlay / place_overlay_detailed
open_picker_overlay / dismiss_picker_overlay / place_picker
```

## Laws

1. Esc peels **only** the top dismissible layer; Trap → `Ignored` (protects below).
2. Dismiss removes **transitive** descendants (parent → children → grandchildren).
3. `OpenMode::Queue` defers a blocking modal behind a blocking top; drain on dismiss.
4. Placement flips when short of space; completion never covers anchor.
5. Narrow fallback is kind policy (hide / center / fullscreen / clamp).
6. Host paints backdrop only when `backdrop_policy() != None`.

## Research anchors

| Source | Takeaway |
|--------|----------|
| Radix layers / DismissableLayer | Outside + Esc + focus scope |
| Textual screens/modals | Z-order + exclusive input |
| Popover placement algs | Flip + clamp + prefer |
| TermRock 0065 sole authority | One stack at shell |

## Related: native-feel inputs

Floating UI alone is not enough. Multi-line **PromptComposer / TextArea** viewports
must use the same [`ScrollAreaState`](./scroll-area.md) engine so agent prompts
scroll like Grok Build / Amp editors (caret reveal, wheel steps, dual-axis bars),
not like raw terminal line mode. See migration `0088`.
