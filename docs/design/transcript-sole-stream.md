# Transcript sole stream (Break J remainder)

| Field | Value |
|-------|-------|
| **Status** | Binding for this change set |
| **Migration** | `0064-v0.13.0-transcript-sole-stream.md` |
| **Related** | Break J, agent-workbench-cutover, streaming-performance |

## Problem

Two stream widgets: `StreamView` (one-row paint shell) and `Transcript` (variable-height, anchor, follow). Dual teaching path; quality only on Transcript.

## Decisions

1. **Delete** public `StreamView`, `StreamItem`, `StreamItemKind`.
2. **Sole stream:** `Transcript` + `TranscriptState` + `TranscriptBlock` + `TranscriptKind`.
3. **Host gates keys** — `handle_key` / `handle_intent` apply when called (no silent ignore on `focused`).
4. **`focused` / `set_focused`** mean **accepts_input** for chrome + selection emphasis only.
5. **Non-color kind prefixes** (StreamView parity) + ASCII fallback + colorless paint path.
6. **Selection cue:** gutter `›` (or `>`) + role, never color-only.
7. **Empty state** distinct from zero-area.
8. **Intents:** `default_transcript_intent` + `handle_intent`.
9. **Mouse:** wheel + click-to-select + double-activate when possible.
10. **No new generic** stream framework — one widget.

## Foundational fixes

| Fix | Why |
|-----|-----|
| Delete StreamView | One-row model cannot grow variable-height law |
| Host-gate keys | Dual authority if widget ignores when unfocused while host already filtered |
| Kind glyphs | Color-only kinds fail no-color / reduced-color |

## Out of scope

- Stream coalesce kits ownership (perf module already separate)
- OverlayStack transcript fullscreen
