# Button premium redesign (canonical primary action)

| Field | Value |
|-------|-------|
| **Status** | Binding |
| **Migration** | `0076-v0.13.0-button-premium.md` |

## Preserve / migrate / delete

| Surface | Fate |
|---------|------|
| `ActivationState` / `ActivationOutcome` | **Preserve & extend** (shared by Button + IconButton) |
| `Button` / `ButtonState` / `button_hit` | **Migrate** API |
| `IconButton` | **Migrate** — require `accessible_label` |
| `Action` / `ActionBar` | **Preserve** multi-id toolbar rows |
| `primary(bool)` | **Preserve** as compat helper → `ButtonVariant::Primary` |
| Raw Enter/Space in `ActivationState::handle_key` | **Delete** — use `default_button_intent` |

## API sketch

```rust
pub enum ButtonVariant { Primary, Secondary, Quiet, Outline, Destructive, Link, Success, Command }
pub enum ButtonSize { Compact, Normal }
pub enum ActivationOutcome { Ignored, Activated, ConfirmRequired, Pressed }

Button::new(label, &system)
  .variant(ButtonVariant::Primary)
  .size(ButtonSize::Compact)
  .leading("✓").trailing("⌘S")
  .full_width(true)
  .accessible_label("Save document") // required if label empty
  .ascii(true).colorless(true);

// Host
state.activation.set_accepts_input(scene_owns);
state.activation.set_pending_confirmation(destructive);
// Do NOT set_accepts_input(true) as dialog default for Destructive
assert!(!btn.is_safe_default_focus());
```

## Laws

1. Activate via intents (`default_button_intent` → Activate); Enter Press once; Space arm/release; Repeat ignored.
2. Loading ≠ disabled (paint + `is_loading` / `is_enabled`).
3. Destructive: `is_safe_default_focus() == false`.
4. Icon-only: accessible_label required.
5. Affordance from Role + pad + outline/link cues — not brackets alone.
