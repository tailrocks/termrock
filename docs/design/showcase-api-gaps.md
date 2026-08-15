# Showcase API gaps log

**Rule:** When `termrock-showcase` needs a private workaround, log it here and
fix TermRock instead. Never paper over with private app-only chrome.

**SoT:** `docs/design/showcase-workbench.md` (elevated 2026-08-09).

**Burn-down 2026-08-15 (plans/019).** The showcase now exists and runs. Seven
gaps closed — five because the library shipped the primitive while the suite
ran, one (GAP-MT-1) because the projection belongs to the host and always did.
One of those (GAP-WB-2) closed the way the SoT said it could: with a
showcase-owned split built from public `layout`, not a new pattern slot. One
gap was *found* by building it: `centered_modal` panicked on any terminal
narrower than a modal's minimum, which took the whole application down at 20
columns. Fixed in `patterns/agent_workbench.rs` with a regression test, which
is the gap protocol working in the direction it was designed for.

| Id | Symptom | Missing primitive / API | Severity | Home | Delivery | Status |
|----|---------|-------------------------|----------|------|----------|--------|
| GAP-WB-1 | Workbench still PromptBox/ApprovalCard | Elevate to PromptComposer + PermissionPrompt + OverlayStack | P0 | `patterns/agent_workbench.rs` | G1 / agent A1b | **closed (0236)** |
| GAP-WB-2 | No files pane / activity slot in layout | Workspace slots in pattern **or** showcase-owned Workspace (interim) | P1 | pattern + layout | G1b | **closed by the sanctioned interim (SoT §2.2.1)** — the showcase splits a 26-column files rail off frames ≥100 columns with public layout and paints `Panel` + `Tree` into it; the pattern needs no files slot |
| GAP-MT-1 | Nested tool under message awkward | MessageThread project-to-lines helpers | P0 | widgets + agent pack | G2 | **closed by composition (S2)** — the showcase projects its own model to `TranscriptBlock` lines per frame (`app::project_blocks`); no library helper was needed and none is missing |
| GAP-MD-1 | Stream fences break wrap | StreamingMarkdown incomplete-fence | P0 | `markdown.rs` | G2 | open |
| GAP-TC-1 | Tool card not full inspectable run | ToolCallCard elevation | P0 | `agent.rs` | G3 | **closed** — `ToolCard` ships; showcase projects tool runs into transcript tool blocks |
| GAP-TR-1 | Shell stdout not first-class | TerminalRunCard (LogPane compose) | P0 | review/log + agent | G3 | **closed** — `TerminalRunCard` + `TerminalOutput` ship (patterns/terminal_run_card.rs) |
| GAP-PL-1 | Plan accept chords unsafe (`a` accept; Enter not bound) | PlanReview action-focus + default safe focus | P1 | `plan_review.rs` (0228) | G4 | closed |
| GAP-DF-1 | No file accept/reject; multi-file nav missing | `FileAccepted` / `FileRejected` (+ multi-file) on `DiffReviewOutcome` | P1 | `review.rs` | G4 | open — showcase ships §18.5 interim (hunk nav + Esc), per SKD-18 |
| GAP-SUB-1 | Custom subagent paint | SubagentCard | P1 | agent pack / blocks | G5a | **closed** — `SubagentCard` ships (patterns/subagent_card.rs) |
| GAP-ACT-1 | Active tools row hand-rolled | ActivityShelf | P1 | agent pack | G5b | **closed** — `ActivityShelf` ships and the workbench takes `activities` |
| GAP-CM-1 | Context only TokenMeter | ContextMeter | P1 | agent elevate | G5c | open — `TokenMeter` covers the showcase's need; a separate ContextMeter is not yet justified by a real gap |
| GAP-CP-1 | Checkpoints not interactive (`Timeline` paint-only) | CheckpointTimeline | P1 | `checkpoint_timeline.rs` (0229) | G5d | closed |
| GAP-REC-1 | No Studio rec format wired | recording schema + check CLI | P2 | studio + showcase | S8 | **closed as scripted tests** — `crates/termrock-showcase/tests/scenes.rs` replays every scenario headlessly and asserts the design law; a JSON recording format is not needed to gate behaviour |

### Severity

- **P0** — blocks core demo scenario  
- **P1** — degrades quality vs mockups  
- **P2** — polish / edge  

### Protocol

1. Reproduce in showcase without private chrome.
2. Append or update a row here (append-only history; status may move open → in-progress → closed).
3. Ship library fix as a G-unit (or library PR on main) before re-enabling the demo path.
4. Close the gap only when public API + story/tests land.
