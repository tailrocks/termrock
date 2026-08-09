# OverlayStack architecture

**Status:** implemented (migrations `0043`, helpers expanded with drawer/popover/tooltip/jump)  
**Law:** **Escape closes exactly one conceptual interaction layer.**  
**Module:** `termrock::interaction::overlay_stack`

---

## 1. Problem

Dialogs, completion menus, popovers, command palettes, tooltips, context menus,
drawers, jump modes, and fullscreen viewers must not each invent:

- placement / anchoring / flip / clamp  
- z-order and parent–child lifetime  
- backdrops  
- focus traps and opener restoration  
- Esc / outside-click / wheel routing  
- narrow-terminal fallback and fullscreen promotion  
- resize reflow  

---

## 2. Solution

[`OverlayStack`](../../crates/termrock/src/interaction/overlay_stack.rs) is the
**sole owner** of open overlay durability and resolved geometry.

| Concern | Behavior |
|---------|----------|
| **Z-order** | Open order; last entry is top |
| **Parent–child** | `parent` id; dismissing a parent drops all descendants |
| **Placement** | `place_overlay` + kind policy prefer/flip/clamp/min–max |
| **Anchoring** | Optional `anchor` rect; `cover_anchor` per kind |
| **Collision / flip** | Anchored menus flip above/left when needed |
| **Screen-edge** | Final clamp inside bounds |
| **Min/max sizes** | `OverlaySize` |
| **Backdrops** | `BackdropPolicy::{None,Dim,Occlude}` (topmost wins) |
| **Focus traps** | `focus_trap` + pair with InteractionScene/FocusGraph |
| **Opener restore** | `opener_focus` returned on `Dismissed` |
| **Keyboard** | Top `owns_input` / trap; Esc via `handle_escape` |
| **Pointer** | `handle_outside_click`; `pointer_hits_top` |
| **Wheel** | `wheel_captures` + `wheel_captured(position)` |
| **Nested dismiss** | Children removed with parent |
| **Fullscreen** | Kind, narrow promote, or `promote_top_fullscreen` |
| **Resize** | `reflow(bounds)` re-places from stored size/anchor |
| **Narrow** | `NarrowFallback::{Clamp,Center,Fullscreen,Hide}` |
| **Animation** | Optional consumer FrameTick; stack is geometry-only |

### Escape law

```text
handle_escape():
  if stack empty → UnhandledEscape (app quit policy)
  if top.esc == Trap → Ignored (protect layers beneath)
  if top.esc == Ignore → (should not be top input owner)
  if top.esc == Dismissible → dismiss top only (+ descendants of that top)
```

One Esc → one conceptual peel. Nested menus: child first, then parent.

---

## 3. Policy table

| Kind | Esc | Outside | Owns input | Trap | Wheel | Backdrop | Prefer | Cover anchor | Narrow |
|------|-----|---------|------------|------|-------|----------|--------|--------------|--------|
| **Tooltip** | Ignore | Dismiss | no | no | no | None | AboveStart | no | Hide |
| **Popover** | Dismiss | Dismiss | yes | no | yes | None | BelowStart | no | Center |
| **Menu** | Dismiss | Dismiss | yes | yes | yes | None | BelowStart | no | Clamp |
| **ContextMenu** | Dismiss | Dismiss | yes | yes | yes | None | AtOrigin | yes | Clamp |
| **Completion** | Dismiss | Dismiss | yes | no | yes | None | BelowStart | **no** | Clamp |
| **Select** | Dismiss | Dismiss | yes | yes | yes | None | BelowStart | no | Clamp |
| **Dialog** | Dismiss | **Trap** | yes | yes | yes | Dim | Center | yes | Fullscreen |
| **AlertDialog** | **Trap** | **Trap** | yes | yes | yes | Occlude | Center | yes | Fullscreen |
| **Drawer** | Dismiss | Dismiss | yes | yes | yes | Dim | DrawerEnd | yes | Fullscreen |
| **CommandPalette** | Dismiss | Dismiss | yes | yes | yes | Dim | Center | yes | Fullscreen |
| **Fullscreen** | Dismiss | Trap | yes | yes | yes | Occlude | Fullscreen | yes | Fullscreen |

Jump mode uses **Fullscreen** placement with Dismissible Esc (see `open_jump_overlay`).

---

## 4. Concrete Rust API

```rust
use termrock::interaction::{
    OverlayStack, OverlaySpec, OverlaySize, OverlayKind, OverlayOutcome,
    place_overlay, OverlayPolicy,
};

let mut stack = OverlayStack::<FocusId>::new();
let bounds = terminal_size;

// Open
stack.open(bounds, OverlaySpec::dialog("confirm", OverlaySize::dialog(48, 12), Some(opener)));
stack.open(bounds, OverlaySpec::completion("cmp", anchor, size, Some(opener)));
stack.open(bounds, OverlaySpec::command_palette("palette", size, Some(opener)));
stack.open(bounds, OverlaySpec::tooltip("tip", anchor, size, None));
stack.open(bounds, OverlaySpec::popover("pop", anchor, size, Some(opener)));
stack.open(bounds, OverlaySpec::drawer("drawer", size, Some(opener)));
stack.open(bounds, OverlaySpec::fullscreen("viewer", Some(opener)));
stack.open(bounds, OverlaySpec::menu("select", anchor, size, Some(opener)).with_parent("dialog"));

// Frame
stack.reflow(new_bounds); // on resize
stack.sync_scene_layers(&mut scene);

// Input
match stack.handle_escape() { /* Dismissed { focus } | Ignored | UnhandledEscape */ }
match stack.handle_outside_click(pos) { … }
if stack.wheel_captured(pos) { /* don't scroll parent */ }

// Paint
for entry in stack.entries() {
    // paint chrome in entry.rect; respect entry.kind / backdrop_policy()
}
```

### Widget helpers (migrated)

| Widget | Helpers |
|--------|---------|
| **Dialog** | `place_dialog`, `open_dialog_overlay`, `open_alert_dialog_overlay`, `dismiss_dialog_overlay` |
| **CompletionMenu** | `place_completion_menu`, `open_completion_overlay`, `dismiss_completion_overlay` |
| **CommandPalette** | `place_command_palette`, `open_command_palette_overlay`, `dismiss_…` |
| **Drawer / Popover / Tooltip** | `place_*`, `open_*_overlay`, `dismiss_drawer_overlay` |
| **JumpOverlay** | `open_jump_overlay`, `dismiss_jump_overlay`, `JumpOverlayState::open_on_stack` |
| **Menu / ContextMenu** | `OverlaySpec::menu` / `context_menu` via menu state helpers |
| **PromptComposer** | completion open via `OverlaySpec` + `place_overlay` |

**Rule:** New floating UI **must** open through `OverlayStack` + `OverlayKind` policy. Local `Rect` math only for non-floating layout.

---

## 5. Consumer shell loop

1. `stack.open(bounds, OverlaySpec::…)` when opening chrome  
2. Each frame: register root, `stack.sync_scene_layers(&mut scene)`, register controls  
3. Esc → `stack.handle_escape()` then scene / quit  
4. Outside click → `stack.handle_outside_click`  
5. Resize → `stack.reflow(new_bounds)`  
6. Paint backdrop (`stack.backdrop_policy()`) then each `entry.rect`  
7. On `Dismissed { focus }` → restore FocusGraph / scene focus  

---

## 6. Stories / tests (required coverage)

Implemented as unit tests in `overlay_stack::tests` (`story_*`) plus widget open tests:

| Scenario | Test / story |
|----------|----------------|
| Nested overlays + one Esc peel | `story_nested_overlays_escape_one_layer` |
| Placement near every edge | `story_placement_near_every_screen_edge` |
| Tiny terminal fallback | `story_tiny_terminal_fallback` |
| Mouse dismissal | `story_mouse_dismissal` |
| Keyboard-only + Esc law | `story_keyboard_only_navigation_and_esc_law` |
| Opener restoration | `story_opener_focus_restoration` |
| Resize while open | `story_resize_while_overlay_open` |
| Multiple queued dialogs | `story_multiple_queued_dialogs` |
| Fullscreen promotion | `story_fullscreen_promotion` |
| Policy table all kinds | `story_policy_table_covers_all_kinds` |
| Drawer edge | `story_drawer_edge_placement` |
| Widget migration | completion / dialog / palette / drawer / popover / tooltip / jump open tests |

Lookbook: paint stories remain per-widget; stack behavior is enforced in lib tests (deterministic, no TUI required).

---

## 7. Migration notes for apps

```rust
// Before — ad-hoc dialog rect
let rect = center(area, 40, 12);
// Esc handled inside dialog only; no stack; no opener restore

// After
stack.open(area, OverlaySpec::dialog("d", OverlaySize::dialog(40, 12), Some(FOCUS_MAIN)));
// paint Dialog into stack.top().rect
// Esc: stack.handle_escape() → restore FOCUS_MAIN
```

Completion must use `OverlayKind::Completion` so menus **never cover** the anchor cell.

---

## 8. Success criteria

1. No new floating surface with private placement that duplicates `place_overlay`.  
2. Esc always peels exactly one stack layer (or traps alert).  
3. Opener focus restored via `Dismissed.focus`.  
4. Narrow terminals promote dialogs/palettes; tooltips hide.  
5. Nested dismiss removes children.  
6. Resize reflow keeps ids and re-places.

---

## 9. Related

- Semantic intents / FocusGraph: [`semantic-interaction-architecture.md`](./semantic-interaction-architecture.md)  
- Migration `0043` historical boundary; new helpers are additive API expansions.
