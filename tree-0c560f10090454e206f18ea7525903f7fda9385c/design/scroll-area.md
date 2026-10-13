# ScrollArea (canonical scrolling primitive)

| Field | Value |
|-------|-------|
| **Status** | Binding |
| **Migration** | `0085-v0.13.0-scroll-area.md` |
| **Module** | `widgets::scroll_area` |
| **Studio** | `scroll-area/follow-paused` |

## Preserve / migrate / split / delete

| Surface | Fate |
|---------|------|
| `scroll::{max_offset, apply_delta_u16, render_scrollbar}` | **Preserve** |
| `scroll::TailScroll` / `DialogScroll` | **Preserve** specialized helpers |
| `perf::FollowMode` / `ScrollAnchor` / `NewContentIndicator` | **Wire into** ScrollAreaState |
| `widgets::ScrollAreaState` | **Expand** to canonical model |
| `LogStreamState` dual `follow` bool | **Delete** — sole `scroll.is_following()` |
| Widget-local offset pairs without state | **Migrate** reps to ScrollAreaState |
| Color-only “new content” | **Forbidden** — glyph/count indicator |

## Mission

One scroll engine for logs, transcripts, lists, dialogs, nested panes:
axes, wheel, page, bars, follow/pause/unseen, anchors, nesting policy,
visible ranges for virt + semantic inspection.

## Research anchors

| Source | Takeaway |
|--------|----------|
| Browser scroll anchoring | Prefer stable content ids; re-resolve after reflow |
| Textual / terminal log viewers | Follow-tail + paused + “N new” chip |
| k9s / nested panes | Explicit chain at edge, never silent dual capture |
| TermRock `perf::follow` | Reuse `FollowMode` / `ScrollAnchor` / indicator kit |

## API (shipped)

```rust
ScrollChain::{Capture, Parent, NestedPreferChild}
ScrollOutcome::{Ignored, Scrolled, FollowChanged, ChainToParent}
VisibleRange { start, end }  // half-open
ScrollAreaState {
  offset_x/y, content, viewport,
  follow: FollowMode, indicator: NewContentIndicator,
  anchor: Option<ScrollAnchor>,
  chain: ScrollChain,
  wheel_step_y/x, axis_y/x,
}
  set_content_size / set_viewport / clamp
  scroll_by / page / home / end / follow_tail / pause_follow
  set_offset_y (user, pauses) / set_offset_y_quiet (reveal)
  on_content_grown(appended)  // follow or note unseen
  apply_anchor(resolve_id)
  visible_range_y() / visible_range_x()
  handle_key / handle_mouse / handle_intent
ScrollArea::render_bars / render_new_content / body_area
```

## Tests

| Kind | Filter / id |
|------|-------------|
| Unit | `widgets::scroll_area` (clamp, follow, chain, anchors, unicode width, huge O(1)) |
| Visual paint | `visual_bars_and_new_content_paint` |
| Migration rep | `widgets::review` LogStream / ObjectInspector / DiffReview |
| Studio | `scroll-area/follow-paused` |
