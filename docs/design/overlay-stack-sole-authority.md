# OverlayStack sole authority (Break D / M4)

| Field | Value |
|-------|-------|
| **Status** | Binding for this change set |
| **Migration** | `0065-v0.13.0-overlay-stack-sole.md` |
| **Related** | pre-1.0 Break D, OverlayStack helpers 0059 |

## Problem

Public `ModalStack` + private `OverlayHost`/`EscCascade`/`OverlayController` + lookbook dual taught a second Esc/z-order path. Quality and Esc law live only on `OverlayStack`.

## Decisions

1. **Sole public overlay authority:** `OverlayStack` (+ widget openers).
2. **Unexport** `ModalStack`, `ModalClickResult`, `classify_click`.
3. **Keep** `render_backdrop` / paint helper for hosts that paint when `stack.backdrop_policy() != None` (stack-driven).
4. **Delete** private `overlay.rs`, `esc_cascade.rs`, `overlay_controller.rs`.
5. **FocusRing** (crate-private / lookbook fork): modal scope helpers without ModalStack.
6. **Lookbook:** `OverlayStack` + domain `Option` for prototype ChoiceDialog; Esc/outside via stack.
7. **No new generic** overlay framework.

## Foundational fixes

| Fix | Why |
|-----|-----|
| Delete dual OverlayId types | Two ids = wrong dismiss |
| Lookbook on OverlayStack | Dogfood sole path |
| Scope helpers without ModalStack | Focus must not reintroduce dual |

## Acceptance

- [ ] public-api free of ModalStack
- [ ] lookbook free of ModalStack import
- [ ] OverlayStack Esc / outside tests still green
- [ ] Policy test denylist ModalStack in public re-exports
