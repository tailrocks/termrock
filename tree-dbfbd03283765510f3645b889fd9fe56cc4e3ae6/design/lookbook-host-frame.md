# Lookbook HostFrame (Break K partial / Host on library authorities)

| Field | Value |
|-------|-------|
| **Status** | Binding for this change set |
| **Migration** | `0066-v0.13.0-lookbook-host-frame.md` |
| **Related** | Break C/D/K, OverlayStack sole (`0065`), InteractionScene sole focus |

## Problem

Lookbook dogfooded a **vendored FocusRing** (`host_focus.rs` ~500 LOC) after M3 unexported library FocusRing. That freezes dual focus authority and blocks shadcn-like “host uses only public APIs.”

## Decisions

1. **HostFrame** (lookbook-private): `DesignSystem` source + `InteractionScene<FocusId, LayerId, ()>` + `OverlayStack<()>`.
2. **Delete** `host_focus.rs` and FocusRing type alias.
3. **Focus ids** stay lookbook-local enums (`FocusId`, `LayerId`).
4. **Panel chrome:** `scene.focused() == Some(&id)` → `PanelChrome::Focused`.
5. **Tab/Esc:** `scene.handle_key_tab_esc` / layer peel; overlay Esc via `OverlayStack` first when modal open.
6. **Modal prototype:** OverlayStack entry + scene `LayerId::Modal` with `owns_input`; action targets registered on that layer.
7. **Stories** still `RenderFn` + interactors this MS (full `Story` trait = M12).
8. **No new library FocusRing.**

## Foundational fixes

| Fix | Why |
|-----|-----|
| Delete host_focus | Dual focus is not a temporary adapter |
| Scene layers for modal | Scope stacks on FocusRing re-taught wrong model |

## Acceptance

- [x] `host_focus.rs` gone
- [x] lookbook compiles/tests green
- [x] denylist: no FocusRing / host_focus in lookbook sources
- [x] focus trap modal still works (Tab, Esc, pointer)
- [x] narrow shell geometry tests
- [x] focused panel chrome buffer evidence
