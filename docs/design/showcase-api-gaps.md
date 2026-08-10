# Showcase API gaps log

**Rule:** When `termrock-showcase` needs a private workaround, log it here and
fix TermRock instead. Never paper over with private app-only chrome.

**SoT:** `docs/design/showcase-workbench.md` (elevated 2026-08-09).

| Id | Symptom | Missing primitive / API | Severity | Home | Delivery | Status |
|----|---------|-------------------------|----------|------|----------|--------|
| GAP-WB-1 | Workbench still PromptBox/ApprovalCard | Elevate to PromptComposer + PermissionPrompt + OverlayStack | P0 | `patterns/agent_workbench.rs` | G1 / agent A1b | **closed (0236)** |
| GAP-WB-2 | No files pane / activity slot in layout | Workspace slots in pattern **or** showcase-owned Workspace (interim) | P1 | pattern + layout | G1b | open |
| GAP-MT-1 | Nested tool under message awkward | MessageThread project-to-lines helpers | P0 | widgets + agent pack | G2 | open |
| GAP-MD-1 | Stream fences break wrap | StreamingMarkdown incomplete-fence | P0 | `markdown.rs` | G2 | open |
| GAP-TC-1 | Tool card not full inspectable run | ToolCallCard elevation | P0 | `agent.rs` | G3 | open |
| GAP-TR-1 | Shell stdout not first-class | TerminalRunCard (LogPane compose) | P0 | review/log + agent | G3 | open |
| GAP-PL-1 | Plan accept chords unsafe (`a` accept; Enter not bound) | PlanReview action-focus + default safe focus | P1 | `plan_review.rs` (0228) | G4 | closed |
| GAP-DF-1 | No file accept/reject; multi-file nav missing | `FileAccepted` / `FileRejected` (+ multi-file) on `DiffReviewOutcome` | P1 | `review.rs` | G4 | open |
| GAP-SUB-1 | Custom subagent paint | SubagentCard | P1 | agent pack / blocks | G5a | open |
| GAP-ACT-1 | Active tools row hand-rolled | ActivityShelf | P1 | agent pack | G5b | open |
| GAP-CM-1 | Context only TokenMeter | ContextMeter | P1 | agent elevate | G5c | open |
| GAP-CP-1 | Checkpoints not interactive (`Timeline` paint-only) | CheckpointTimeline | P1 | `checkpoint_timeline.rs` (0229) | G5d | closed |
| GAP-REC-1 | No Studio rec format wired | recording schema + check CLI | P2 | studio + showcase | S8 | open |

### Severity

- **P0** — blocks core demo scenario  
- **P1** — degrades quality vs mockups  
- **P2** — polish / edge  

### Protocol

1. Reproduce in showcase without private chrome.
2. Append or update a row here (append-only history; status may move open → in-progress → closed).
3. Ship library fix as a G-unit (or library PR on main) before re-enabling the demo path.
4. Close the gap only when public API + story/tests land.
