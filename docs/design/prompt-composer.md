# PromptComposer (flagship agent input)

**Status:** implemented foundation (`termrock::widgets::PromptComposer`)  
**Package role:** kernel widget now; opinionated skins may ship via `@termrock/agent` later  
**Supersedes for agents:** thin `PromptBox` remains for simple embeds

---

## 1. State model

| Bucket | Owns | Survives overlay takeover? |
|--------|------|----------------------------|
| **Text editing** | `TextAreaState`, undo/redo stacks, history, selection anchor | **Yes** (draft never cleared on blur) |
| **Tokens & attachments** | `chips: Vec<ComposerChip>` | Yes |
| **Completion** | `CompletionQuery` (slash / file / symbol) | Closed on Esc; draft kept |
| **Presentation** | compact/normal/expanded/fullscreen, density, ascii | Yes |
| **Policy** | `SubmitPolicy`, `busy`, `connection`, queue | Consumer-set |

```text
Draft text ──blur──► still held  ◄── focus returns from Permission/Plan/Palette
```

---

## 2. Anatomy

```
┌ chips: [📎 file] [📋 paste ×] … ─────────────────────┐
│ multiline editor (grapheme-safe TextArea)              │
│ mode · model · busy · queue:N          [ctx meter]     │
│ validation error (optional)                            │
└────────────────────────────────────────────────────────┘
```

---

## 3. Public API (summary)

```rust
use termrock::widgets::{
    PromptComposer, PromptComposerState, PromptComposerOutcome,
    SubmitPolicy, ComposerPresentation, ComposerConnection,
    ModeIndicator, ModelIndicator, ContextEstimate, ComposerChip,
};

let mut state = PromptComposerState::new();
state.set_mode(Some(ModeIndicator { label: "PLAN".into(), warning: false }));
state.set_model(Some(ModelIndicator { label: "grok".into() }));
state.set_context(ContextEstimate { used: 1_000, limit: 128_000 });
state.set_busy(true); // Enter → Queued

match state.handle_key(key) {
    PromptComposerOutcome::Submit { text, chip_ids } => { /* run agent */ }
    PromptComposerOutcome::Queued { entry } => { /* queue */ }
    PromptComposerOutcome::Completion { query } => { /* open menu */ }
    PromptComposerOutcome::Cancel | PromptComposerOutcome::Interrupt => { /* stop */ }
    _ => {}
}

// Overlay takeover: do NOT clear_draft
state.set_focused(false);
// … permission layer …
state.set_focused(true);
```

Overlay helpers: `open_completion_overlay`, `dismiss_completion_overlay`, `place_completion`.

---

## 4. Typed messages

See `PromptComposerOutcome`: Submit, Queued, QueueRemoved, Cancel, Interrupt, DismissRequest, ExternalEditor, Completion, CompletionClosed, CompletionCommitted, ModeMenu, ModelMenu, ChipRemoved/Activated, AttachRequest, ValidationFailed, PresentationChanged, Blur, Changed, Ignored.

---

## 5. Overlay integration

- Completion uses `OverlayKind::Completion` + id `termrock.prompt_completion`.  
- Fullscreen presentation may use `termrock.prompt_fullscreen`.  
- Esc closes completion **one layer** before bubbling `DismissRequest`.  
- Draft preserved while Permission / Question / Plan / Session / Palette own focus.

---

## 6. Focus integration

- `set_focused(false)` → editor unfocused; draft intact.  
- Scene: register composer control id on root or prompt layer.  
- Chip focus via BackTab strip; Esc returns to editor.

---

## 7. Rendering strategy

1. Layout chips / editor / status / validation without nested scroll chrome.  
2. Editor = `TextArea` (cursor, mouse, paste).  
3. Status = mode · model · busy · queue + optional `TokenMeter`.  
4. Compact skips heavy panel border when short.

---

## 8. Performance

- Undo = bounded text snapshots (64), not per-grapheme ropes.  
- History capped (100).  
- Large paste → chip (no multi-MB insert into editor).  
- Paint O(visible editor rows + chips).

---

## 9–10. Tests & stories

Lib tests: blur draft, submit, queue, disconnect, paste chip, slash/@ detect, undo, history, completion apply, overlay open.

Lookbook: `prompt-composer/basic`, `busy-queue`, `compact` + interactor.

---

## 11. Implementation plan (phased)

| Phase | Work |
|-------|------|
| **P0** ✅ | State split, submit/queue/busy, chips, paste threshold, completion detect, undo/history, indicators, blur draft, tests, stories |
| **P1** | Wire real CompletionMenu/Slash rows in workbench; commit inserts |
| **P2** | True multi-line selection + cut/copy via edit_core ranges |
| **P3** | Fullscreen overlay promote + external editor round-trip API |
| **P4** | Shift-selection, richer mouse drag-select |
| **P5** | Studio scenarios: permission takeover, matrix mono/ascii |

`PromptBox` remains for minimal embeds; agents should prefer `PromptComposer`.
