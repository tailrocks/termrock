# Permission & trust surface

**Status:** implemented foundation (`termrock::widgets::permission`)  
**Goal:** clearer and safer than generic “Allow?” dialogs for terminal AI agents  
**Companion:** `ApprovalCard` remains a minimal decision strip; prefer `PermissionPrompt` for agent trust UX.

---

## Principles

1. **Default focus never grants.** Deny (or non-grant) is selected first.  
2. **Esc cancels** without Allow.  
3. **Stale responses ignored** — generation must match queue head.  
4. **Provenance is visible** — main agent → subagent → plugin/MCP.  
5. **Risk language is explicit** — destructive + data-egress banners.  
6. **No side effects** — outcomes only; consumer enforces policy.

---

## Domain types

| Type | Role |
|------|------|
| `PermissionRequest` | Full request snapshot |
| `PermissionProvenance` / `ProvenanceHop` / `InitiatorKind` | Who initiated |
| `PermissionRisk` | Low → Critical |
| `PermissionActionKind` | read/write/shell/network/mcp/… |
| `PermissionTarget` / `ExecutionLocation` / `DataMovement` | What/where/egress |
| `PermissionScope` | Once · Session · Project · Always |
| `PermissionAction` | Allow · Deny · AllowEdited · AllowRestricted · RequestChanges · InspectDetails |
| `PermissionQueue` | FIFO + generation + audit |
| `PermissionPromptState` | UI state for head request |
| `PermissionOutcome` | Typed messages |
| `PermissionAuditEntry` | Local audit trail |

### Request fields (every prompt can show)

- Who / provenance chain  
- Exact action + kind  
- Exact target  
- Execution location  
- Data accessed / destination / egress flag  
- Expected result  
- Risk + reversible  
- Requested scope  
- Prior grant hint  
- Command + pattern previews (editable)

---

## Queue & stale protection

```
push(req) → assign generation N
head is only confirmable generation
resolve(N, …) ok → audit + pop
resolve(old, …) → StalePermission / StaleIgnored
```

Nested subagent provenance is data on the request; queue order is still FIFO unless consumer reorders.

---

## Keyboard

| Key | Action |
|-----|--------|
| ←/→ · Tab | Move among actions |
| \[ / \] | Cycle scope Once→Session→Project→Always |
| Enter | Confirm selected (Inspect toggles details) |
| Esc | Cancel head (no grant), advance queue |
| n | Deny + confirm |
| d | Toggle details |
| e | Edit command (if present) |
| p | Edit pattern (if present) |
| **y** | **Not bound** (no accidental allow) |

Edit mode: type/backspace; Enter → AllowEdited/AllowRestricted; Esc cancel edit.

## Mouse

- Hover selects action  
- Click confirms once  
- Hit regions published each frame  

## Narrow layout

- Stack provenance → action/target → warning → one action row (scroll/clamp labels)  
- Details collapsed by default  
- Scope on its own line  

## Colorless / ASCII

- Risk glyphs `i` / `!` / `!!` / `X`  
- Labels always present  

---

## Textual mockups

### Low risk — file read

```
┌─ i low risk · read_file ─────────────────────────────┐
│ from agent:main                                      │
│ read → src/lib.rs                                    │
│ at local · ~/proj                                    │
│ expect: file contents for analysis                   │
│ prior: Session grant on src/**                       │
│ scope: Once · [] · q:1                               │
│ [ Deny ]  [ Details ]  [ Change ]  [ Allow ]         │
└──────────────────────────────────────────────────────┘
  focus: Deny
```

### Destructive — shell delete (nested subagent)

```
┌─ !! high risk · bash ────────────────────────────────┐
│ from agent:main > subagent:reviewer > mcp:filesystem │
│ shell → workspace                                    │
│ at local · sandbox:off                               │
│ DESTRUCTIVE: shell may be hard to undo               │
│ expect: remove build artifacts                       │
│ $ rm -rf build/                                      │
│ scope: Once · [] · q:2                               │
│ [ Deny ] [ Details ] [ Edit&Allow ] [ Change ]       │
│ [ Restrict ] [ Allow ]                               │
└──────────────────────────────────────────────────────┘
  focus: Deny   (Allow is last; Enter does not approve)
```

### Data egress — critical network

```
┌─ X critical · http_post ─────────────────────────────┐
│ from agent:main > subagent:reviewer > mcp:filesystem │
│ network → api.example.com                            │
│ at local                                             │
│ DATA EGRESS: src/** + .env → https://api.example.com │
│ expect: upload diagnostics payload                   │
│ scope: Once · [] · q:1                               │
│ [ Deny ] [ Details ] [ Change ] [ Restrict ] [ Allow]│
└──────────────────────────────────────────────────────┘
  focus: Deny
  Details expands: payload size, redacted headers, …
```

---

## Integration

```rust
let mut ui = PermissionPromptState::new();
ui.enqueue(request);
// OverlayStack open PermissionPrompt policy (Alert-like trap optional)

match ui.handle_key(key) {
    PermissionOutcome::Decided { action, scope, edited, generation, .. } => {
        // re-check generation if async
        apply_policy(action, scope, edited);
    }
    PermissionOutcome::StaleIgnored { .. } => {}
    PermissionOutcome::Cancelled { .. } => {}
    _ => {}
}
```

Pair with `OverlayStack` + `LayerDismissPolicy::Trap` for alert-class blocks when required.

---

## Tests (lib)

- Default focus never Allow  
- Nested provenance path  
- FIFO queue + stale resolve  
- Surface stale after external dismiss  
- Esc cancel advances queue  
- `y` does not grant  
- Command edit → AllowEdited  
- Scope cycle  
- Mouse hit confirm  
- Audit initiator = leaf MCP label  

---

## Migration

`0046` — additive public surface; `ApprovalCard` unchanged.
