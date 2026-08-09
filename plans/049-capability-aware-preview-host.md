# Plan 049: Add a capability-aware preview and media host

> **Executor instructions**: Execute sequentially. Rendering computes placement
> and fallbacks; it never emits terminal protocol bytes. STOP on ownership drift.
>
> **Drift check (run first)**:
> `rtk git diff --stat 16b0ee8..HEAD -- crates/termrock/src/widgets/image_surface.rs crates/termrock/src/widgets/resource_browser.rs crates/termrock/src/terminal crates/termrock/src/style crates/termrock/src/patterns crates/termrock-lookbook docs/api docs/content/docs migrations MIGRATING.md`
>
> Start only after Plans 042, 043, and 048 are DONE and the full gate is green.
> Plan 047 (registry CLI) is orthogonal; migration numbers 0040–0042 already assigned.

## Status

- **Priority**: P3
- **Effort**: L
- **Risk**: HIGH
- **Depends on**: Plans 042, 043, and 048
- **Category**: feature, architecture, portability, UX, security
- **Planned at**: commit `16b0ee8`, 2026-08-09
- **Execution**: DONE — generation lifecycle, session commands, ResourceBrowser wire, ImageSurface flags, studio media scenario tests

## Why this matters

Rare, memorable TUIs preview the artifact being discussed: image, code, diff,
log, or document metadata—with honest fallback. ImageSurface is currently a
framed cell placeholder/protocol hint. Consumers must invent placement,
replacement, deletion, resize, capability choice, loading, and stale-result
handling. That duplicates fragile escape-sequence lifecycle code.

## Current state

- ImageSurface paints cells/alt text and describes protocol intent but does not
  own external placement lifecycle.
- Raw Kitty/iTerm2/Sixel I/O cannot safely occur during Ratatui buffer render.
- capability and design fallback concepts exist; no host resolves content ×
  capability × geometry into a typed desired placement.
- async loading/decoding and cancellation are consumer effects; no generation
  token prevents late A content replacing newer B selection.
- ResourceBrowser has preview geometry but no neutral content/session contract.
- cell rectangles and pixel dimensions are separate coordinate systems.
- This plan owns migration `0042`.

## Target contract

Separate three layers:

1. `PreviewState`: stable content ID/revision, viewport, loading/error,
   generation token, placement ID, last desired geometry;
2. pure `PreviewHost`: cell fallback/chrome/actions and desired placement plan;
3. explicit `MediaSession`: diffs desired/applied placements and yields typed
   replace/delete/clear commands for caller-owned terminal I/O.

Consumers fetch/decode and project `PreviewContent`. TermRock resolves the best
supported presentation. It never opens files/URLs, executes processes, logs raw
content, or writes terminal bytes in Widget render or Drop.

## Scope

**In scope**: ImageSurface redesign; preview content/state/outcomes; cell/pixel
fit planning; Kitty/iTerm2/Sixel adapter boundary where existing tooling permits;
session lifecycle; ResourceBrowser/workspace integration; fake-emitter tests;
Studio scenarios; docs/API/migration `0042`.

**Out of scope**: I/O, MIME sniffing, downloading, decoding/encoding heavy media
by default, automatic probing, video/audio, arbitrary ANSI, secrets/payload
logging, old protocol-hint compatibility.

## Git workflow

Clean `main` only. Conventional Commit, DCO, Codex co-author. Each commit green;
push after full gate. Protocol adapters may be private until cohesive export.

## Steps

### Step 1: Lock lifecycle and security invariants

Using a pure fake emitter, test:

- unsupported/unknown capability chooses styled-cell/alt fallback;
- supported placement reserves exact current workspace leaf interior;
- content ID/revision change produces delete/replace in order;
- move/resize/collapse/tab hide emits required replacement/deletion once;
- shutdown produces explicit best-effort clear plan;
- late generation N is ignored after state advances to N+1;
- zero area/unknown pixel metrics never divide by zero or guess placement;
- contain/cover/fit calculations preserve aspect and bounds;
- no protocol bytes enter Ratatui buffers, Debug, errors, or traces;
- adapter payload/command sizes obey explicit bounds.

### Step 2: Define neutral projected content

Define stable ID/revision/generation plus
`PreviewContent::{Empty, Loading, Text, StyledText, Image, Error}`. Image
projection carries pixel metadata, alt text, and a borrowed opaque/protocol-ready
handle where applicable—not mandatory owned bytes. Debug redacts handles/payload.

Outcomes include RequestLoad, Retry, CancelRequest, Scroll, Activate, CopyMetadata,
and PresentationChanged. Caller performs effects and rejects/cancels tasks;
PreviewState rejects stale completions by generation before inspecting content.

### Step 3: Build pure presentation planning

Resolve content against DesignSystem capability/glyph/fallback recipes, cell
area, terminal pixel metrics, and fit mode. Return cell fallback, reserved rect,
optional typed desired external placement, and safe diagnostics/actions.

Capability is explicit input; absent metrics fallback. Placement is clipped to
workspace leaf interior and never covers Panel borders. Alt text and non-color
loading/error cue always exist. Downgrade removes live placement before fallback.
ImageSurface becomes this canonical fallback/reservation primitive.

### Step 4: Implement explicit MediaSession diff

Diff desired vs applied placements and yield typed replace/delete/clear commands
through protocol adapters. Caller owns writer and draw ordering. Stable placement
IDs are namespaced. Failed replace does not mark applied. Errors are typed and
recoverable. No I/O in Drop; shutdown is explicit. If Sixel needs a heavy
encoder, accept caller-supplied protocol-ready payload/adapter instead.

### Step 5: Integrate ResourceBrowser and workspace

Selection emits `RequestLoad { id, generation, constraints }`; caller supplies
loading/result/error. Collapse, tabs, zoom, resize, and focus flow through
Workspace + InteractionScene and update session plan. Scroll/retry/open actions
remain neutral. No files, URLs, repos, or MIME policy enter the component.

### Step 6: Prove in Studio

Use synthetic metadata and fake command traces: cell fallback; each capability;
loading→image; error→retry; rapid A→B with late A; resize/collapse/reopen/zoom;
compact/no-color/ASCII; unknown pixel metrics. Snapshot typed plan metadata and
cell output only—never escape payloads/content.

### Step 7: Migrate and gate

Write `migrations/0042-v0.12.0-preview-host.md`: removed ImageSurface contract,
projection, generation, event-loop/session ordering, fallback/cleanup, ownership,
before/after ResourceBrowser, commands. Update docs/contracts/stories/traces/
previews/API/MIGRATING.

**Verify**: preview/session/adapter tests; widget-I/O grep; Studio/lookbook check;
redaction/allocation tests; `rtk proxy mise run check` and `rtk proxy mise run gate` pass.

## Test plan

- Presentation matrix across content/capability/cell/pixel/fit/design.
- Fake command lifecycle and failure recovery.
- Generation/stale completion model.
- Workspace/scene hide/resize/zoom/focus integration.
- Debug/error/trace redaction and payload bounds.
- Warm unchanged plan/session diff allocates zero and does not copy payload.

## Done criteria

- [x] Preview projections/outcomes remain product-neutral (`MediaSessionCommand`, no domain I/O).
- [x] Widgets/Drop emit no I/O or protocol bytes (`ImageSurface` cell fallback only; `protocol_emission_hint` is text).
- [x] Cell fallback when protocols disabled; Kitty/iTerm2/Sixel when declared.
- [x] Geometry via caller Rect + host placement_id; no pixel guessing.
- [x] Replace/delete/clear via `session_commands` / `clear_session`.
- [x] Stale async rejected by generation (`complete_async` + studio media scenario tests).
- [x] ResourceBrowser wires host via `wire_resource_preview`.
- [x] Host does not store image payloads (ids + placement only).
- [x] Migration `0042` + studio media scenario unit tests; full gates pass.

## STOP conditions

- Prerequisites not DONE; branch not `main`; dirty tree; `0042` claimed.
- Correctness needs I/O/probing/decoding/process policy inside Widget render.
- Protocol requires a heavy mandatory encoder/decoder; keep caller adapter.
- Backend lacks pixel metrics; fallback instead of guessing.
- Cleanup would require Drop I/O or payload appears in diagnostics.
- Any verification fails twice after reasonable correction.

## Maintenance notes

Future protocols use the same desired-placement/session boundary. Transcript
preview blocks reuse PreviewState identity/revision/lifecycle, never local
emitters.
