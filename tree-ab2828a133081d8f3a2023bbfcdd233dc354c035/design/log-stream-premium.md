# LogStream premium redesign

| Field | Value |
|-------|-------|
| **Status** | Binding |
| **Migration** | `0071-v0.13.0-log-stream-premium.md` |

## Problem

`LogStream` is a thin follow/detach wrapper: no host input gate, no intents,
no mouse hit geometry, no scene-focus chrome, no ASCII/colorless recipes, no
empty/narrow/tiny paint, and almost no tests/stories. Below the List/Menu/
ObjectInspector premium bar while OpsDashboard already composes it.

## Decisions

1. **Scene owns surface focus** — `LogStream::focused` + `set_accepts_input`.
   No field/line cursor (scroll surface, not selection list).
2. **Outcomes** — keep `Follow` / `Detach`; add `Scrolled { offset }` when
   offset changes without a follow flip.
3. **`handle_intent` + `default_log_stream_intent`** — j/k, arrows, Home/End,
   page; `f`/Space → Toggle follow; End/`Move(Last)` → re-follow.
4. **`handle_mouse`** — wheel scrolls + detaches; click follow chip re-attaches.
5. **Paint** — `system` (not `tokens`), level glyphs, follow chip, empty mark,
   `ascii` / `colorless`, narrow (glyph+text), tiny (text only).
6. **`render(..., &mut state)`** — stores origin/viewport for hits + content size.
7. **`on_append`** — still O(1) rejoin when following.

## Not doing

- Owning bounded line history (consumer/LogPane owns buffer).
- Search / filter (consumer projects filtered lines).
- Line selection cursor (Transcript/List territory).
