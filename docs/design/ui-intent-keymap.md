# UiIntent + Keymap (semantic input)

| Field | Value |
|-------|-------|
| **Status** | Binding |
| **Migration** | `0080-v0.13.0-ui-intent-keymap.md` |

## Preserve / migrate / delete

| Surface | Fate |
|---------|------|
| `UiIntent` core (Move/Page/Activate/…) | **Extend** |
| `default_*_intent` free fns | **Preserve** as thin bridges over `Keymap` tables |
| `Keymap` / `KeyBinding` / `Conflict` / hints | **Preserve & extend** |
| `KeyChord` + `raw_bytes_to_chord` | **Preserve** (conventional + CSI/kitty subset) |
| `dispatch_keymap_action` | **Preserve** |
| Widget `handle_intent` | **Preserve** primary path |
| Widget raw `match key.code` for nav | **Migrate** reps → intent |
| Product-only chords (permission `e`/`p`) | **Keep** on handle_key until command maps |

## API sketch

```rust
pub struct AppCommandId(pub &'static str);

pub enum UiIntent {
    // nav / activation (existing) + …
    FocusNext, FocusPrevious,
    JumpStart, JumpLabel(char),
    Edit, Delete, Backspace,
    Search, Help, Fullscreen, OpenCommandPalette,
    AppCommand(AppCommandId),
}

pub enum KeymapContext { Global, Surface(&'static str), Zone(&'static str), Overlay(&'static str) }
pub struct KeymapStack<A> { /* top-wins layers */ }
pub enum KeymapProfile { Default, Vim, Emacs }

// Presets
Keymap::<UiIntent>::list_default() / list_vim()
Keymap::<UiIntent>::global_chrome()
KeymapStack::profile(KeymapProfile::Default)
```

## Laws

1. Widgets implement `handle_intent`; `handle_key` only bridges defaults.
2. Hints, help, palette rows, and dispatch share binding tables.
3. Conflicts are diagnosable (`Keymap::conflicts`, stack merge diagnostics).
4. UiIntent stays `Copy` (static AppCommandId only).
