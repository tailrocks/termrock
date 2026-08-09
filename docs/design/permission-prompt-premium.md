# PermissionPrompt premium redesign

| Field | Value |
|-------|-------|
| **Status** | Binding |
| **Migration** | `0077-v0.13.0-permission-prompt-premium.md` |

## Preserve / migrate / delete

| Surface | Fate |
|---------|------|
| Request/queue/provenance/risk/scope models | **Preserve** |
| Default-deny + stale generations + no `y` grant | **Preserve** (safety law) |
| Overlay open trap High/Critical | **Preserve** |
| `PermissionOutcome::SelectionChanged` | **Migrate** → `ActionCursorMoved` / `ScopeChanged` |
| Ungated keys | **Migrate** → `accepts_input` |
| Raw key-first routing | **Migrate** → intent-first + product chords |
| Always Focused panel | **Migrate** → surface `focused` + accepts_input |

## Laws

1. Default action cursor is never a granting action.
2. Esc cancels without grant; dismiss advances queue.
3. Enter activates **selected** action (Deny by default).
4. `y` never in default_permission_intent.
5. Focus (surface accepts_input) ≠ action cursor (selected decision).
6. Color never sole risk cue — glyphs + labels always present.
