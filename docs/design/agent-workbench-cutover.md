# Agent Workbench cutover — dual chrome kill (Break J partial)

| Field | Value |
|-------|-------|
| **Status** | Binding for this change set |
| **Migration** | `0063-v0.13.0-agent-dual-cutover.md` |
| **Related** | `pre-1.0-api-redesign.md` Break J, `showcase-workbench.md` GAP-WB-1, `permission-trust.md`, `prompt-composer.md` |

## Problem

TermRock shipped two agent stacks:

| Concern | Legacy (delete) | Canonical (keep) |
|---------|-----------------|------------------|
| Prompt | `PromptBox` | `PromptComposer` |
| Permission | `ApprovalCard` (+ `y`→AllowOnce) | `PermissionPrompt` (default-deny, no grant-on-`y`) |
| Pattern seed | workbench used legacy | workbench uses canonical only |

Lookbook and docs taught both. That fails the premium component standard and fail-safe trust law.

## Decisions

1. **Sole public prompt** for agent input: `PromptComposer` / `PromptComposerState` / `PromptComposerOutcome`.
2. **Sole public permission** for trust gates: `PermissionPrompt` / `PermissionPromptState` / `PermissionRequest` / queue / provenance.
3. **AgentWorkbench** composes only canonical types. Scene layer id: `permission` (was `approval`).
4. **Delete** public `ApprovalCard*`, `ApprovalDecision*`, `ApprovalRisk`, `PromptBox*`.
5. **StreamView** stays this MS (separate cutover); workbench stream remains `Transcript`.
6. **No generic abstraction** invented for one pattern — workbench is a pattern, not a new widget framework.
7. **Intents:** `default_permission_intent` + `PermissionPromptState::handle_intent` for Activate/Cancel/Move; composer keeps existing key routing (intent pack follows).
8. **Focus:** host gates via `InteractionScene`; composer `set_focused` reflects scene ownership; list uses `.focused(bool)`.
9. **Esc:** scene peels `permission` then `question`; does not resolve grants (host must call permission Esc / cancel).
10. **Responsive:** existing workspace collapse; permission modal fraction clamps on narrow; tiny widths keep geometry contained.
11. **ASCII / no-color:** `PermissionPrompt::ascii`, `PromptComposerState::set_ascii_fallback` / `set_colorless` supported on workbench paint path via consumer state.

## Foundational fixes (not workarounds)

| Fix | Why not workaround |
|-----|--------------------|
| Delete dual types | Two fail-safe laws cannot coexist |
| Workbench API break | Seeding legacy permanently trains the wrong stack |
| Policy tests denylist | Prevents dual re-entry in workbench + lookbook |

## Out of scope this MS

- StreamView deletion / Transcript-only stream law
- Lookbook HostFrame / host_focus delete
- ModalStack public kill
- Full intent keymap for composer chords

## Acceptance

- [ ] No `ApprovalCard` / `PromptBox` in public API or lookbook stories
- [ ] Workbench paint + Esc + narrow tests green on PermissionPrompt + PromptComposer
- [ ] Permission intent tests + workbench interaction tests
- [ ] Migration 0063 + COMPONENTS / handbook updated
- [ ] `cargo test -p termrock --all-features` and lookbook green
