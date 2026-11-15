# Layer focus policy for shared shell layers (#21)

`LayerSpec::restore_focus` and `LayerSpec::inert_below` were declared but
ignored. `ShellLayers::open` read only `initial_focus` and always called the
unconditional `ModalStack::push`, and every close path restored the saved
focus. A popover that moved focus to another valid control before closing had
that choice overwritten, and a passive tooltip cleared focus on open.

## Decision

- `ModalStack::push_with_focus_policy(modal, focus, FocusOnOpen, restore_focus)`
  stores `restore_focus` on the entry when it opens. A later change to the
  layer's spec does not change that entry's return contract.
  `FocusOnOpen::Set(target)` moves focus on open (`Set(None)` clears it);
  `FocusOnOpen::Preserve` leaves it alone.
- `ModalStack::push` keeps its signature and behavior: it delegates with
  `Set(initial)` and `restore_focus = true`.
- `ShellLayers::open` reads the spec once. An initial target maps to
  `Set(Some(id))`. No target on an `inert_below` layer maps to `Set(None)`,
  and the shared fallback picks a control on the next upkeep. No target on a
  non-inert layer maps to `Preserve`.
- `pop` restores the saved focus only for a restoring entry. Result, dismissal
  and `Request::Close` all pop, so they share the policy.
- `clear` computes the final destination of repeated top-down pops (the saved
  focus of the lowest restoring entry, or the current focus when none
  restores) and applies it once. Removed layers' controls receive no
  intermediate focus or blur transition.
- `retain` keeps its signature and does not touch focus. Every surviving entry
  above the first removed entry loses its saved focus (it becomes `None`),
  because the snapshot may name a control inside a removed layer. A later
  restoring pop then clears focus and the next upkeep chooses a valid control.
  This is conservative and deliberate.

## Consumers

The Showcase opened page layers through `push` and read `initial_focus` itself.
It now calls `ShellLayers::open`, and its `page_under_layer` asks
`ShellLayers::hides_page_focus` so a passive tooltip layer keeps the page's
focus indicator visible. TablePro, Holla and Jackin already open through
`open` and inherit the policy; none of them sets `restore_focus(false)` or a
non-inert layer today.

The Showcase "Focus & navigation" page gains a "Layer focus" card with a
passive tip layer, a restoring popover and a non-restoring popover. The
Showcase layer routes every key to the top layer, so the tip closes with Esc.
That routing is the Showcase's, not a library rule.

## Limits

- `clear` and `retain` do not validate the destination against the focus ring.
  Shell upkeep does that on the next frame through the existing fallback.
- Closing a layer does not emit focus changes of its own. Blur reaches layers
  and pages through the existing `Focus::take_changes` settlement.
