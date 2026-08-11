# Permission & trust surface system

| Field | Value |
|-------|-------|
| **Status** | Implemented foundation + design SoT (`termrock::widgets::permission`) |
| **Goal** | Clearer and safer than generic “Allow?” dialogs for terminal AI agents |
| **Code** | `crates/termrock/src/widgets/permission.rs` |
| **Companion** | Prefer `PermissionPrompt` over `ApprovalCard` for agent trust (KD-25 agent ban) |
| **Related** | `termrock-agent.md` KD-5/20/26, `overlay-stack.md`, plan 046 |

---

## Principles

1. **Default focus never grants.** Deny (or non-grant) is selected first for every risk tier.  
2. **Esc cancels** without Allow (widget path or host gate-cancel on Dismissible peel — KD-26).  
3. **Stale responses ignored** — confirm must match live head `generation`.  
4. **Provenance is visible** — main agent → subagent → plugin / MCP leaf.  
5. **Risk language is explicit** — destructive + data-egress banners; labels not color-only.  
6. **No side effects** — pure outcomes; consumer owns policy store, process, network.  
7. **`y` is unbound** — no accidental Allow chord (historical bug class on `ApprovalCard`).  
8. **Queue is FIFO** — nested provenance is *data* on each request, not a parallel UI stack.

---

## Ownership split

| TermRock | Consumer agent |
|----------|----------------|
| Domain types, queue, generations, audit buffer | Whether a tool may run |
| Default-deny focus, action strip order | Sandbox, network, secret policy |
| Render + keys + mouse hits | Persistence of “Always” grants |
| Overlay id + open helpers | Effect of Allow/Deny |
| Warning copy templates | Redaction of secrets in detail lines |

---

## Domain types

### `PermissionRequest` — every prompt can communicate

| Field | Communicates |
|-------|----------------|
| `provenance` | Who initiated (chain) |
| `provenance` hops | Main / subagent / plugin / MCP / user / system |
| `action` + `action_kind` | Exact action (`bash`, `read_file`, …) |
| `target` | Exact target path/URL/resource |
| `location` | Execution location (local, sandbox, remote) |
| `data.accessed` | Data being accessed |
| `data.destination` + `data.egress` | Destination / egress flag |
| `expected_result` | Expected result |
| `risk` | Risk level Low→Critical |
| `reversible` | Whether operation is reversible |
| `requested_scope` | Requested permission scope |
| `prior_grant` | Whether a similar permission was previously granted |
| `command_preview` / `pattern_preview` | Editable command / pattern |
| `detail_lines` | Expandable inspect payload |
| `generation` | Stale-response token (queue-assigned) |

### Provenance

```text
InitiatorKind: MainAgent | Subagent | Plugin | McpServer | User | System
ProvenanceHop { kind, id, label }
PermissionProvenance { chain: outer → leaf }
  display_path() → "agent:main > subagent:reviewer > mcp:filesystem"
  leaf() / has_subagent()
```

### Risk classification

| Risk | Glyph | Role | Default focus | Typical content |
|------|-------|------|---------------|-----------------|
| Low | `i` | Info | Deny | Read-only |
| Medium | `!` | Warning | Deny | Careful local write |
| High | `!!` | Danger | Deny | Destructive / hard reverse |
| Critical | `X` | Danger | Deny | Data egress / secrets / external |

`PermissionRisk::default_focus` **always** returns a non-granting action.

### Scopes

`Once` · `Session` · `Project` · `Always`  
Cycle with `[` / `]`. Consumer enforces persistence; UI only records chosen scope on grant.

### Outcomes (`PermissionAction`)

| Action | Grants? | Role |
|--------|---------|------|
| `Allow` | yes | Full allow at selected scope |
| `Deny` | no | Reject |
| `AllowEdited` | yes | Allow with edited command |
| `AllowRestricted` | yes | Allow with restricted pattern/scope |
| `RequestChanges` | no | Ask agent to change plan |
| `InspectDetails` | no | Toggle detail expansion |

### Typed messages (`PermissionOutcome`)

`Ignored` · `SelectionChanged` · `DetailsToggled` · `EditStarted` · `EditChanged` · `EditCancelled` · `Decided { request_id, generation, action, scope, edited }` · `Cancelled` · `QueueChanged` · `StaleIgnored`

### Queue & audit

```text
PermissionQueue
  push(req) → generation N (monotonic)
  head() only live confirmable generation
  resolve(N, action, scope, edited) → Ok(req) + audit | Err(Stale)
  dismiss_head(N) → cancel without grant
  remove_id(id) → drop cancelled tool
  audit(): PermissionAuditEntry { request_id, generation, action, scope, edited, initiator }
```

`initiator` audit field = **leaf** provenance label (e.g. MCP server name).

### Stale-response protection

```
enqueue A → gen 1
enqueue B → gen 2
resolve(2) while head is 1 → StalePermission::Superseded { live: 1 }
async host must re-check generation before side effects
surface confirm after external resolve → StaleIgnored
```

---

## Permission queue & nested subagents

- One visual surface shows **head only**.  
- Nested subagent is **not** a second focus stack: it is provenance chrome on the request.  
- When head is decided/cancelled, queue advances; next head re-syncs default Deny focus.  
- Upstream tool cancel → `remove_id` (may invalidate non-head without focusing it).

---

## Detail expansion

- `d` or `InspectDetails` + Enter toggles `details_expanded`.  
- Shows `detail_lines`, full command, pattern, prior grant, data movement.  
- Collapsed by default on narrow terminals and High/Critical (banner stays visible).

---

## Command & pattern editing

| Key | Field | Confirm |
|-----|-------|---------|
| `e` | Command (`command_preview`) | Enter → `AllowEdited` + buffer |
| `p` | Pattern (`pattern_preview`) | Enter → `AllowRestricted` + buffer |
| Esc | — | `EditCancelled` (queue intact) |

No I/O: edited string is payload on `Decided.edited`.

---

## Keyboard flow

| Key | Action |
|-----|--------|
| ←/→ · Tab / BackTab | Move among actions |
| [ / ] | Scope Once→Session→Project→Always |
| Enter | Confirm selected (Inspect toggles details) |
| Esc | Cancel head (no grant), advance queue |
| n | Deny + confirm |
| d | Toggle details |
| e / p | Start command / pattern edit |
| **y** | **Not bound** |

---

## Mouse flow

- Hover selects action region.  
- Click confirms once (same as Enter on that action).  
- Regions republished each paint in `action_regions`.  
- Host should not treat outside-click as Allow.

---

## Narrow-terminal layout

```
width ≥ 60  multi-column action strip + one-line provenance
width 40–59 stack lines; clamp action labels
width < 40  essential: risk glyph + action + target + Deny/Allow row
            details collapsed; scope on own line
```

ASCII mode: risk glyphs only (`i`/`!`/`!!`/`X`); no emoji.

---

## Destructive language & data-egress warnings

`PermissionRequest::warning_text()`:

1. Custom `destructive_notice` if set.  
2. Else if `data.egress` → `DATA EGRESS: {accessed} → {destination}`.  
3. Else if `!reversible || risk.is_destructive()` → `DESTRUCTIVE: {kind} may be hard to undo`.

Banner uses Danger/Warning roles; **words remain** under monochrome themes.

---

## Overlay integration

| Id | Kind |
|----|------|
| `termrock.permission` (`PERMISSION_OVERLAY_ID`) | `Dialog` Low/Medium; `AlertDialog` (Trap) High/Critical |

```rust
state.open_overlay(&mut stack, bounds, Some(opener_focus));
// High/Critical: Trap → Esc goes to widget → Cancelled
// Low/Medium: Dismissible peel → host must permission_gate_cancel (KD-26)
```

---

## Textual mockups

### Low risk — file read (prior grant)

```
┌─ i low risk · read_file ─────────────────────────────┐
│ from agent:agent                                     │
│ read → src/lib.rs                                    │
│ at local · ~/proj                                    │
│ expect: file contents for analysis                   │
│ prior: Session · src/** previously Session           │
│ scope: Once · [] · q:1                               │
│ [ Deny ]  [ Details ]  [ Change ]  [ Allow ]         │
└──────────────────────────────────────────────────────┘
  focus: Deny   Enter → Deny (never accidental Allow)
```

### Destructive — shell delete (nested subagent + MCP)

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
  focus: Deny
  e → edit command → Enter → AllowEdited
  y → Ignored (no grant)
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
  Details: payload size, redacted headers, …
```

---

## Public API (summary)

```rust
use termrock::widgets::{
    PermissionPrompt, PermissionPromptState, PermissionRequest, PermissionRisk,
    PermissionProvenance, ProvenanceHop, InitiatorKind, PermissionScope,
    PermissionAction, PermissionOutcome, PermissionQueue, PERMISSION_OVERLAY_ID,
};

let mut ui = PermissionPromptState::new();
ui.enqueue(
    PermissionRequest::new("r1", "bash", "workspace")
        .risk(PermissionRisk::High)
        .action_kind(PermissionActionKind::Shell)
        .command("rm -rf build/")
        .expected("remove build artifacts")
        .location("local", Some("sandbox:off".into()))
        .irreversible()
        .provenance(
            PermissionProvenance::main_agent("run", "main")
                .push(ProvenanceHop::new(InitiatorKind::Subagent, "s", "reviewer"))
                .push(ProvenanceHop::new(InitiatorKind::McpServer, "mcp", "filesystem")),
        ),
);

match ui.handle_key(key) {
    PermissionOutcome::Decided { generation, action, scope, edited, .. } => {
        if ui.queue.is_live(generation) { /* should already be consumed */ }
        apply_policy(action, scope, edited);
    }
    PermissionOutcome::StaleIgnored { .. } => {}
    PermissionOutcome::Cancelled { .. } => {}
    _ => {}
}
```

---

## Tests (lib) — especially queue + nested provenance

| Test | Proves |
|------|--------|
| `default_focus_is_never_allow` | All risk tiers |
| `enqueue_defaults_selection_to_deny` | High-risk shell |
| `enter_on_default_denies_not_allows` | Enter ≠ Allow |
| `nested_provenance_display_includes_subagent_and_mcp` | Path + leaf |
| `queue_fifo_and_stale_protection` | Generations |
| `surface_stale_confirm_after_external_dismiss` | UI StaleIgnored |
| `queued_requests_advance_after_decide` | FIFO advance |
| `nested_subagent_queue_preserves_provenance_across_advance` | Nested across queue |
| `three_queued_stale_after_head_resolved_externally` | Multi-stale |
| `esc_cancels_without_grant_and_advances_queue` | Esc law |
| `y_is_not_bound_to_allow` | No y-grant |
| `command_edit_allow_edited_outcome` / `pattern_edit_allow_restricted` | Edits |
| `edit_esc_cancels_without_resolving` | Edit cancel |
| `scope_cycle` / `allow_with_project_scope_records_audit` | Scopes + audit |
| `audit_records_nested_initiator_label` | Leaf = MCP |
| `egress_warning_text` / `request_fields_cover_trust_checklist` | Content |
| `mouse_confirm_uses_hit_regions` | Mouse |
| `permission_overlay_opens_alert_for_high_risk` | OverlayStack |
| `remove_id_invalidates_non_head_without_stale_grant` | Upstream cancel |

---

## Lookbook stories

| Story | Content |
|-------|---------|
| `permission-prompt/basic` | High-risk shell |
| `permission-prompt/low-read` | Low risk + prior grant |
| `permission-prompt/destructive-nested` | Nested provenance + DESTRUCTIVE |
| `permission-prompt/egress` | Critical data egress |
| `permission-prompt/narrow` | 22-col contraction |
| `permission-prompt/unicode` | Unicode-safe paint |

---

## Implementation plan

| Phase | Work | Status |
|-------|------|--------|
| **T0** | Domain types, queue, generations, audit, default-deny prompt | **Done** |
| **T1** | Nested provenance, edit command/pattern, scope cycle, mouse hits | **Done** |
| **T2** | Destructive/egress warnings, narrow layout, colorless glyphs | **Done** |
| **T3** | Overlay helpers + AlertDialog High/Critical; expanded tests | **Done** (this pass) |
| **T4** | AgentWorkbench sole path (A1b); kill ApprovalCard agent use | Next |
| **T5** | Studio matrix: mono, narrow, triple queue, KD-26 dismiss cancel | Next |
| **T6** | Optional: host-side Always store adapter docs only | Later |

---

## Migration

Additive public surface (`PERMISSION_OVERLAY_ID`, builders). `ApprovalCard` remains for non-agent embeds; agent workbench must use `PermissionPrompt` (see `termrock-agent.md` KD-25).
