# Catalog API gaps log

**Rule:** When `termrock-catalog` needs a private workaround, log it here and
fix TermRock instead. Never paper over with private app-only chrome.

**SoT:** the shared coordination ledger and the public TermRock API.

**Historical burn-down completed 2026-08-16 (plans/019).** The former workbench
projection existed, but it is no longer a competing preview product. The
unified catalog has no open API gap. Missing generic capabilities shipped in
TermRock; product projections stayed in the catalog host. The GAP-WB-2
composition uses public `layout`, not a new pattern slot. One gap was *found*
by building it: `centered_modal` panicked on any terminal narrower than a
modal's minimum, which took the whole application down at 20 columns. Fixed in
`patterns/agent_workbench.rs` with a regression test, which is the gap protocol
working in the direction it was designed for.

| Id | Symptom | Missing primitive / API | Severity | Home | Delivery | Status |
|----|---------|-------------------------|----------|------|----------|--------|
| GAP-WB-1 | Workbench still PromptBox/ApprovalCard | Elevate to PromptComposer + PermissionPrompt + OverlayStack | P0 | `patterns/agent_workbench.rs` | G1 / agent A1b | **closed (0236)** |
| GAP-WB-2 | No files pane / activity slot in layout | Workspace slots in pattern **or** catalog-owned Workspace composition (interim) | P1 | pattern + layout | G1b | **closed by the sanctioned interim** — the catalog splits a 26-column files rail off frames ≥100 columns with public layout and paints `Panel` + `Tree` into it; the pattern needs no files slot |
| GAP-MT-1 | Nested tool under message awkward | MessageThread project-to-lines helpers | P0 | widgets + agent pack | G2 | **closed by composition (S2)** — the catalog projects its own model to `TranscriptBlock` lines per frame (`app::project_blocks`); no library helper was needed and none is missing |
| GAP-MD-1 | Stream fences break wrap | StreamingMarkdown incomplete-fence | P0 | `markdown.rs` | G2 | **closed (0326)** — `StreamingMarkdown::with_blocks` and `measure_height` expose the exact paint projection; open-fence row-map parity is tested |
| GAP-TC-1 | Tool card not full inspectable run | ToolCallCard elevation | P0 | `agent.rs` | G3 | **closed** — `ToolCard` ships; the catalog projects tool runs into transcript tool blocks |
| GAP-TR-1 | Shell stdout not first-class | TerminalRunCard (LogPane compose) | P0 | review/log + agent | G3 | **closed** — `TerminalRunCard` + `TerminalOutput` ship (patterns/terminal_run_card.rs) |
| GAP-PL-1 | Plan accept chords unsafe (`a` accept; Enter not bound) | PlanReview action-focus + default safe focus | P1 | `plan_review.rs` (0228) | G4 | closed |
| GAP-DF-1 | No file accept/reject; multi-file nav missing | File units through the typed review outcome | P1 | `review.rs` | G4 | **closed (0326)** — file-tree focus resolves to `DiffReviewUnitKind::File`; approve/reject returns the existing typed unit outcome instead of adding parallel file-only variants |
| GAP-SUB-1 | Custom subagent paint | SubagentCard | P1 | agent pack / blocks | G5a | **closed** — `SubagentCard` ships (patterns/subagent_card.rs) |
| GAP-ACT-1 | Active tools row hand-rolled | ActivityShelf | P1 | agent pack | G5b | **closed** — `ActivityShelf` ships and the workbench takes `activities` |
| GAP-CM-1 | Context only TokenMeter | ContextMeter | P1 | agent elevate | G5c | **closed by correct ownership** — `TokenMeter` covers the catalog's measured need; no distinct generic capability or behavior exists to justify a duplicate ContextMeter |
| GAP-CP-1 | Checkpoints not interactive (`Timeline` paint-only) | CheckpointTimeline | P1 | `checkpoint_timeline.rs` (0229) | G5d | closed |
| GAP-REC-1 | No catalog recording format wired | recording schema + check CLI | P2 | catalog | S8 | **closed as scripted tests** — `crates/termrock-catalog/tests/` replays deterministic scenarios and asserts the design law; a JSON recording format is not needed to gate behaviour |

### Severity

- **P0** — blocks core demo scenario  
- **P1** — degrades quality vs mockups  
- **P2** — polish / edge  

### Protocol

1. Reproduce in the catalog without private chrome.
2. Append or update a row here (append-only history; status may move open → in-progress → closed).
3. Ship library fix as a G-unit (or library PR on main) before re-enabling the demo path.
4. Close the gap only when public API + catalog scenario/tests land.
