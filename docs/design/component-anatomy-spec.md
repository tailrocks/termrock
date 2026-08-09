# TermRock component anatomy, behavior, variants, and state specification

**Status:** binding design target for all component work  
**Audience:** implementers, lookbook authors, migration writers  
**Design system:** [`terminal-design-system.md`](./terminal-design-system.md)  
**Agent prompts:** [`component-prompt-library.md`](./component-prompt-library.md) — 164 implementable prompts (global contract + per-component tasks)  
**Inventory baseline:** public surface of `termrock::widgets` + `patterns` on the experience-layer line  
**Policy:** A component is **not** complete because it paints. Interaction design, focus ownership, contraction (narrow/tiny), capability ladders (unicode/ascii/colorless), typed outcomes, stories, and tests are part of the component.

---

## A. Current inventory → proposed taxonomy

| Current TermRock surface | Taxonomy | Status |
|--------------------------|----------|--------|
| *(none as first-class)* Button, IconButton, Badge, Tag, Chip, Kbd | Primitives | **New** (ActionBar paints ad-hoc actions today) |
| Panel, Backdrop | Layout / Primitives | Evolve → **Surface** (+ keep Panel as bordered Surface recipe) |
| Implicit `─` dividers | Primitives | **Separator** |
| Heading, Paragraph | Content | **New** |
| MarkdownView, CodeBlock, DiffView | Content / Data | Rename Markdown; Diff → **DiffReview** |
| Progress (incl. indeterminate), Skeleton, EmptyState, ErrorView, Banner, Toast, LoadingView | Feedback | Banner → **Callout** / **Alert**; Spinner **promoted** |
| TextInput, TextArea, Form | Forms | + Checkbox, RadioGroup, Switch, Select, MultiSelect |
| List, Tree, Table, VirtualGrid, DetailTable, Picker, CompletionMenu, Selection | Selection / Data | Picker → **Combobox**; DetailTable → **ObjectInspector** |
| Tabs, ActionBar, HintBar, StatusBar | Navigation | + Sidebar, Breadcrumbs, Menu |
| Dialog, MessageDialog, ChoiceDialog, CommandPalette, JumpOverlay, Toast | Overlays | + ContextMenu, Popover, Tooltip, Drawer |
| SplitPane, WorkSurface, Viewport | Layout | → Workspace tree (plan 042); Viewport → ScrollArea |
| StreamView, ToolCard, ApprovalCard, PromptBox, Timeline, ThinkingBlock, TokenMeter | AI-agent | + QuestionFlow, PlanReview, TaskRail, SessionPicker; harden PermissionPrompt |
| Sparkline, BarSeries, SegmentedMeter | Data presentation | Keep under charts |
| ThemePicker, ImageSurface | Dev tools / content | ThemePicker = developer tools |
| agent_shell, ops_dashboard, resource_browser | Application blocks | Elevate: AgentWorkbench, OpsDashboard, ResourceBrowser, … |

**Completeness rule:** every claimed axis in the contract table is specified, story-covered, and tested—or explicitly marked N/A with reason.

---

## B. Shared cross-cutting contracts

Apply to every interactive component unless a section marks N/A.

| Concern | TermRock rule |
|---------|----------------|
| **State ownership** | Widget state = interaction only (focus, selection, scroll, hover, open, cursor). Domain values, validation copy, async results = consumer projections. |
| **IDs** | Stable IDs for focus, selection, and hit geometry when identity survives reorder. |
| **Borrowed data** | Render from borrowed slices/lines; no hidden clone of large datasets. |
| **Focus chrome** | Single-line border + design tokens; **never** border weight/double-line for focus. |
| **Esc** | Routed by InteractionScene layer policy; local cancel only when the scene owns the layer. |
| **Intents** | Prefer `UiIntent` / scene actions; raw keys only in keymap adapters. |
| **Density** | Comfortable / Compact / Dashboard from DesignSystem. |
| **Narrow** | Drop named parts by priority before grapheme-unsafe truncate. |
| **Tiny** (≤20×5) | Primary label + focus cue; overflow ellipsis. |
| **Unicode** | Grapheme-safe clip; never split wide cells. |
| **ASCII** | Glyph catalog fallbacks, not ad-hoc literals. |
| **Colorless / NO_COLOR** | Modifiers + glyphs carry state. |
| **Outcomes** | Typed enums; **no** side effects (no I/O, no process spawn). |
| **Perf** | Paint O(visible); virtualized surfaces never full-scan data in render. |

### Shared visual states
`default` · `hover` · `focus-visible` · `active/pressed` · `selected` · `disabled` · `loading` · `invalid` · `read-only`

### Shared keyboard primitives (via keymap → intents)
Move prev/next · Page · Home/End · Activate · Toggle · Cancel · Submit · Open/Close · Expand/Collapse · Select-all-request

### Density → cell chrome (default)

| Token | Comfortable | Compact | Dashboard |
|-------|-------------|---------|-----------|
| row pad x | 1–2 | 0–1 | 0 |
| stack gap | 1 | 0–1 | 0 |
| panel inset | (2,1) | (1,0) | (0,0) |
| min interactive hit | 1 row | 1 row | 1 row |

---

## C. Taxonomy (target)

1. **Primitives** — Button, IconButton, Badge, Tag, Chip, Kbd, Separator, Spinner  
2. **Content** — Heading, Paragraph, Markdown, CodeBlock, Callout, Alert, Surface, Section  
3. **Layout** — Surface, Section, ScrollArea, Split/WorkspacePane  
4. **Navigation** — Tabs, Sidebar, Breadcrumbs, Menu, ActionBar, HintBar, StatusBar  
5. **Forms and input** — TextInput, TextArea, Checkbox, RadioGroup, Switch, Form  
6. **Selection** — Select, MultiSelect, Combobox, List, Tree, CompletionMenu  
7. **Feedback** — Toast, Progress, Skeleton, EmptyState, Spinner, LoadingView, ErrorView  
8. **Overlays** — Dialog, Drawer, Popover, Tooltip, ContextMenu, CommandPalette, JumpOverlay, Backdrop  
9. **Data presentation** — Table, DataTable, ObjectInspector, LogStream, Timeline, DiffReview, Charts  
10. **Developer tools** — ThemePicker, DesignInspector  
11. **AI-agent** — PromptComposer, PermissionPrompt, QuestionFlow, PlanReview, ToolCallCard, TaskRail, SessionPicker, ThinkingBlock, TokenMeter, Transcript  
12. **Application blocks** — AgentWorkbench, OpsDashboard, ResourceBrowser, SettingsShell, FormWizard  

---

## D. Spec template

Every component below uses sections **1–24**:

1. Purpose  
2. Anatomy and named parts  
3. Public properties  
4. Controlled and uncontrolled state  
5. Variants  
6. Sizes and density behavior  
7. Visual states  
8. Interaction states  
9. Keyboard behavior  
10. Mouse behavior  
11. Focus behavior  
12. Disabled behavior  
13. Loading and asynchronous behavior  
14. Error and validation behavior  
15. Narrow-terminal behavior  
16. Tiny-terminal fallback  
17. Unicode and ASCII behavior  
18. Colorless behavior  
19. Composition rules  
20. Expected messages or outcomes  
21. Stories  
22. Snapshot tests  
23. Interaction tests  
24. Performance expectations  

---

# 1. Primitives

## Button

1. **Purpose:** Domain-neutral control that invokes one action; consumer maps outcome → effects.  
2. **Anatomy:** `root` · `leading_icon` · `label` · `trailing_icon` · `kbd_hint`  
3. **Public properties:** `id`, `label: &str`, `leading`, `trailing`, `variant`, `size`, `enabled`, `loading`, `danger` (or variant), `design: &DesignSystem`  
4. **State:** Uncontrolled: hover, pressed (frame-local). Controlled: none required. Activation is an outcome, not stored “clicked.”  
5. **Variants:** `primary` · `secondary` · `ghost` · `danger` · `link`  
6. **Sizes/density:** height always 1 row; pad_x Comfortable=2, Compact=1, Dashboard=1; `sm`/`md` only change pad.  
7. **Visual states:** default, hover, focus-visible, pressed, disabled, loading.  
8. **Interaction states:** idle · armed (Space down) · activated (emit once).  
9. **Keyboard:** Enter / Space activate; no arrow keys (not a group).  
10. **Mouse:** press/release on root hit; drag-off cancels activation.  
11. **Focus:** tab stop when enabled; focus-visible via recipe (underline or border role, not weight).  
12. **Disabled:** no activate; dim recipe; skip in tab order when `skip_disabled`.  
13. **Loading:** Spinner replaces `leading_icon`; ignore activate until consumer clears loading.  
14. **Error/validation:** N/A (use Form/Callout).  
15. **Narrow:** drop priority: `kbd_hint` → `trailing_icon` → `leading_icon`; never drop `label` first.  
16. **Tiny:** label only, min 3 cols, ellipsis.  
17. **Unicode/ASCII:** label grapheme-safe; icons from glyph catalog.  
18. **Colorless:** primary = bold/underline; danger = `[!]` prefix + bold.  
19. **Composition:** ActionBar, Dialog footer, Menu item paint, EmptyState action.  
20. **Outcomes:** `Activated(Id)` · `Ignored`  
21. **Stories:** `button/primary`, `button/danger-loading`, `button/narrow`, `button/mono`, `button/disabled`  
22. **Snapshots:** each variant × {default, focus, disabled, loading} at fixed width.  
23. **Interaction tests:** Enter/Space once; disabled ignores; loading ignores; drag-off no activate.  
24. **Perf:** O(1) paint.

## IconButton

1. **Purpose:** Icon-only action; accessible label required for hints/a11y.  
2. **Anatomy:** `root` · `icon` · `badge_dot` (optional)  
3. **Public properties:** `id`, `icon`, `aria_label` (required string), `variant`, `enabled`, `loading`, `design`  
4. **State:** hover/pressed only.  
5. **Variants:** `ghost` · `solid` · `danger`  
6. **Sizes/density:** min hit 3×1 cells even if glyph is 1 cell.  
7. **Visual states:** default, hover, focus-visible, pressed, disabled, loading.  
8. **Interaction states:** idle · armed · activated.  
9. **Keyboard:** Enter / Space.  
10. **Mouse:** click root.  
11. **Focus:** tab stop; when focused, HintBar may show `aria_label`.  
12. **Disabled:** no activate; dim icon.  
13. **Loading:** spinner glyph replaces icon.  
14. **Error:** N/A.  
15. **Narrow:** drop `badge_dot` first.  
16. **Tiny:** single glyph.  
17. **Unicode/ASCII:** catalog icon + ASCII fallback.  
18. **Colorless:** reverse on focus-visible.  
19. **Composition:** toolbars, Tabs trailing, Dialog chrome.  
20. **Outcomes:** `Activated(Id)`  
21. **Stories:** `icon-button/basic`, `icon-button/loading`, `icon-button/badge`  
22. **Snapshots:** ghost/solid/danger × focus.  
23. **Interaction tests:** activate + disabled + loading.  
24. **Perf:** O(1).

## Badge

1. **Purpose:** Non-interactive count or status mark.  
2. **Anatomy:** `root` · `label`  
3. **Public properties:** `text`, `tone` (`neutral`/`info`/`success`/`warning`/`danger`), `max_chars`, `design`  
4. **State:** none.  
5. **Variants:** `soft` · `solid` · `outline`  
6. **Sizes/density:** always 1 row; pad 0–1 by density.  
7. **Visual states:** default only (no hover).  
8. **Interaction states:** N/A.  
9–12. **Keyboard/Mouse/Focus/Disabled:** N/A (not focusable).  
13–14. **Loading/Error:** N/A.  
15. **Narrow:** numeric overflow `9+` / `99+` style when `max_chars` exceeded.  
16. **Tiny:** single `•` if width &lt; 2.  
17. **Unicode/ASCII:** plain text; no emoji required.  
18. **Colorless:** tone → prefix `i`/`!`/`x`/`ok`.  
19. **Composition:** List trailing, Tabs, IconButton, Sidebar items.  
20. **Outcomes:** none.  
21. **Stories:** `badge/tones`, `badge/overflow`  
22. **Snapshots:** all tones × variants.  
23. **Interaction tests:** N/A.  
24. **Perf:** O(1).

## Tag

1. **Purpose:** Categorical label, optionally removable.  
2. **Anatomy:** `root` · `label` · `remove`  
3. **Public properties:** `id`, `label`, `removable`, `enabled`, `tone?`, `design`  
4. **State:** Controlled: membership in parent collection. Uncontrolled: N/A.  
5. **Variants:** `default` · `accent`  
6. **Sizes/density:** 1 row; pad by density.  
7. **Visual states:** default, hover (remove), disabled.  
8. **Interaction states:** remove armed.  
9. **Keyboard:** if focused and removable: Backspace/Delete → remove.  
10. **Mouse:** click `remove` hit region only (not whole tag unless configured).  
11. **Focus:** tab stop only when `removable`.  
12. **Disabled:** no remove; dim.  
13. **Loading:** N/A.  
14. **Error:** N/A.  
15. **Narrow:** drop `remove` before truncating label.  
16. **Tiny:** first grapheme of label.  
17. **Unicode/ASCII:** remove glyph `×` / `x`.  
18. **Colorless:** dim label; reverse remove hit.  
19. **Composition:** MultiSelect chip row, filter bars.  
20. **Outcomes:** `Remove(Id)`  
21. **Stories:** `tag/static`, `tag/removable`  
22. **Snapshots:** removable focused.  
23. **Interaction tests:** Delete/Backspace and click remove.  
24. **Perf:** O(1).

## Chip

1. **Purpose:** Compact toggleable filter/choice in a group.  
2. **Anatomy:** `root` · `leading` · `label`  
3. **Public properties:** `id`, `label`, `selected`, `enabled`, `design`  
4. **State:** Controlled `selected` (or group-controlled). Uncontrolled only if group owns selection.  
5. **Variants:** `filter` · `choice`  
6. **Sizes/density:** 1 row; pad by density.  
7. **Visual states:** selected, unselected, hover, disabled.  
8. **Interaction states:** idle · toggled.  
9. **Keyboard:** Enter / Space toggle.  
10. **Mouse:** click toggle.  
11. **Focus:** tab stop when enabled.  
12. **Disabled:** no toggle.  
13. **Loading:** optional spinner replaces leading.  
14. **Error:** N/A.  
15. **Narrow:** truncate label; keep selected cue.  
16. **Tiny:** selected `*` prefix + first grapheme.  
17. **Unicode/ASCII:** `[x]`/`[ ]` or `[*]`/`[ ]`.  
18. **Colorless:** reverse when selected.  
19. **Composition:** filter bars, MultiSelect.  
20. **Outcomes:** `Toggled { id, selected }`  
21. **Stories:** `chip/filter-group`, `chip/choice`  
22. **Snapshots:** selected vs not.  
23. **Interaction tests:** toggle outcomes; disabled.  
24. **Perf:** O(1).

## Kbd

1. **Purpose:** Render a key chord for hints and docs.  
2. **Anatomy:** `root` · `keys[]` · `separator`  
3. **Public properties:** `chord: &str` or structured keys, `size`, `design`  
4. **State:** none.  
5. **Variants:** `plain` · `raised` (sunken surface)  
6. **Sizes/density:** 1 row.  
7. **Visual states:** default only.  
8–14. **Interaction/loading/error:** N/A.  
15. **Narrow:** collapse to last key.  
16. **Tiny:** single char.  
17. **Unicode/ASCII:** “Ctrl” vs `^`; “⌘” vs `Cmd`/`M-`.  
18. **Colorless:** reverse video key caps.  
19. **Composition:** HintBar, Button trailing, docs Markdown.  
20. **Outcomes:** none.  
21. **Stories:** `kbd/chords`, `kbd/ascii`  
22. **Snapshots:** multi-key chords.  
23. **Interaction tests:** N/A.  
24. **Perf:** O(keys).

## Separator

1. **Purpose:** Visual section break; never communicates focus.  
2. **Anatomy:** `root` · optional `label`  
3. **Public properties:** `orientation` (h/v), `label`, `variant`, `design`  
4. **State:** none.  
5. **Variants:** `line` · `dashed` · `section` (`═`/`=` — still not focus weight)  
6. **Sizes/density:** fills parent axis.  
7. **Visual states:** default.  
8–14. N/A.  
15. **Narrow:** label centered with line fill; truncate label.  
16. **Tiny:** single `-` or `|`.  
17. **Unicode/ASCII:** `─`/`-`, `│`/`|`.  
18. **Colorless:** dim line.  
19. **Composition:** Section stacks, Menu item groups, Dialog body.  
20. **Outcomes:** none.  
21. **Stories:** `separator/labeled`, `separator/vertical`  
22. **Snapshots:** labeled horizontal.  
23. **Interaction tests:** N/A.  
24. **Perf:** O(width or height).

## Spinner

1. **Purpose:** Indeterminate progress glyph driven by app clock.  
2. **Anatomy:** `root` · `frame` · `label`  
3. **Public properties:** `tick: u64` / `FrameTick`, `label`, `design` (reads Motion)  
4. **State:** frame derived from tick (uncontrolled animation input).  
5. **Variants:** `inline` · `block`  
6. **Sizes/density:** 1 row.  
7. **Visual states:** animating · static (Motion::Off/reduced).  
8–12. N/A unless embedded in a button.  
13. **Loading:** this *is* the loading affordance.  
14. N/A.  
15. **Narrow:** drop label.  
16. **Tiny:** one frame glyph.  
17. **Unicode/ASCII:** braille/dots vs `|/-\`.  
18. **Colorless:** same glyphs.  
19. **Composition:** Button loading, EmptyState, ToolCallCard, LoadingView.  
20. **Outcomes:** none.  
21. **Stories:** `spinner/frames`, `spinner/motion-off`  
22. **Snapshots:** fixed tick → deterministic frame.  
23. **Interaction tests:** Motion::Off stable frame across ticks.  
24. **Perf:** O(1).

---

# 2. Content components

## Heading

1. **Purpose:** Title hierarchy without font sizes (modifiers + roles).  
2. **Anatomy:** `root` · `text` · optional `eyebrow`  
3. **Public properties:** `level: 1..=4`, `text`, `truncation`, `design`  
4. **State:** none.  
5. **Variants:** levels map to type scale (`display`/`title`/`label`).  
6. **Sizes/density:** margin only when inside Section.  
7. **Visual states:** default.  
8–14. N/A.  
15. **Narrow:** end truncate.  
16. **Tiny:** bold single line.  
17. **Unicode/ASCII:** grapheme truncate.  
18. **Colorless:** bold = level cue.  
19. **Composition:** Section header, Dialog title, Sidebar header.  
20. **Outcomes:** none.  
21. **Stories:** `heading/levels`  
22. **Snapshots:** L1–L4.  
23. **Interaction tests:** N/A.  
24. **Perf:** O(len).

## Paragraph

1. **Purpose:** Body prose with wrap.  
2. **Anatomy:** `root` · `lines`  
3. **Public properties:** `text`, `wrap`, `max_lines`, `tone` (`default`/`muted`), `design`  
4. **State:** none (pure view).  
5. **Variants:** `body` · `muted`  
6. **Sizes/density:** wrap width = area; gap 0.  
7. **Visual states:** default.  
8–14. N/A.  
15. **Narrow:** wrap; `max_lines` + ellipsis.  
16. **Tiny:** first line only.  
17. **Unicode:** grapheme-aware wrap.  
18. **Colorless:** muted = dim.  
19. **Composition:** Markdown body blocks, Dialog body, Callout body.  
20. **Outcomes:** none.  
21. **Stories:** `paragraph/wrap`, `paragraph/max-lines`  
22. **Snapshots:** wrap at fixed widths.  
23. **Interaction tests:** N/A.  
24. **Perf:** O(visible lines).

## Markdown

1. **Purpose:** Paint projected markdown blocks (parse is consumer-side or adapter).  
2. **Anatomy:** `root` · `block[]` (`heading`/`paragraph`/`list`/`code`/`quote`/`rule`)  
3. **Public properties:** `blocks`, `first`/`offset`, `width_policy`, `design`  
4. **State:** optional scroll `offset` (controlled preferred for transcript).  
5. **Variants:** `reader` · `compact`  
6. **Sizes/density:** line gap 0–1 by density.  
7. **Visual states:** default; focused viewport if scrollable.  
8. **Interaction states:** scrolling.  
9. **Keyboard:** j/k/page when focusable scroll surface.  
10. **Mouse:** wheel scroll.  
11. **Focus:** optional focus for keyboard scroll.  
12. **Disabled:** N/A.  
13. **Loading/async:** streaming append; preserve anchor (transcript engine).  
14. **Error:** N/A.  
15. **Narrow:** reflow wrap all blocks.  
16. **Tiny:** plain text dump of first blocks.  
17. **Unicode/ASCII:** lists `•`/`*`; rules `─`/`-`.  
18. **Colorless:** headings bold; quotes dim.  
19. **Composition:** Transcript, docs panes, PlanReview detail.  
20. **Outcomes:** `Scrolled { offset }` if stateful.  
21. **Stories:** `markdown/basic`, `markdown/narrow`, `markdown/code-fence`  
22. **Snapshots:** each block kind.  
23. **Interaction tests:** scroll bounds when stateful.  
24. **Perf:** O(visible blocks); no re-parse in paint.

## CodeBlock

1. **Purpose:** Source listing with optional gutter and syntax roles.  
2. **Anatomy:** `root` · `gutter` · `line[]` · `language_label`  
3. **Public properties:** `lines`, `first_line`, `show_line_numbers`, `highlighter`, `language`, `design`  
4. **State:** optional scroll/cursor.  
5. **Variants:** `plain` · `numbered`  
6. **Sizes/density:** gutter width from digit count.  
7. **Visual states:** default; optional selected line.  
8. **Interaction states:** scrolling · line-select.  
9. **Keyboard:** arrows/page if focusable.  
10. **Mouse:** wheel; click line if selectable.  
11. **Focus:** for keyboard scroll/select.  
12. **Disabled:** N/A.  
13. **Loading:** streaming append lines.  
14. **Error:** N/A.  
15. **Narrow:** hide language; shrink gutter.  
16. **Tiny:** no gutter; clip lines.  
17. **Unicode:** no mid-grapheme clip.  
18. **Colorless:** syntax → bold/dim only.  
19. **Composition:** Markdown code fence, Transcript, Diff side.  
20. **Outcomes:** `Scrolled` · `LineActivated(n)`  
21. **Stories:** `code-block/basic`, `code-block/numbered`, `code-block/unicode`  
22. **Snapshots:** numbered + unicode line.  
23. **Interaction tests:** scroll bounds.  
24. **Perf:** O(visible lines); highlighter cache; no full-file rehighlight each frame.

## Surface

1. **Purpose:** Semantic container elevation (canvas/surface/raised/elevated/sunken).  
2. **Anatomy:** `root` · `content_slot` · optional `border`  
3. **Public properties:** `elevation`, `bordered`, `padding` override, `design`  
4. **State:** none.  
5. **Variants:** `canvas` · `surface` · `raised` · `elevated` · `sunken`  
6. **Sizes/density:** inset from DesignSystem density.  
7. **Visual states:** default; focused only if Panel-like bordered interactive owner uses BorderFocused.  
8–14. N/A (non-interactive shell).  
15. **Narrow:** reduce inset.  
16. **Tiny:** zero pad.  
17. **Unicode/ASCII:** border glyphs from catalog.  
18. **Colorless:** elevation via dim/bold border, not color fill alone.  
19. **Composition:** wraps any child; Panel = Surface + title + focus emphasis.  
20. **Outcomes:** none.  
21. **Stories:** `surface/elevations`  
22. **Snapshots:** elevation ladder.  
23. **Interaction tests:** N/A.  
24. **Perf:** O(area) fill.

## Section

1. **Purpose:** Surface + heading + optional description + body.  
2. **Anatomy:** `root` · `heading` · `description` · `body` · `header_actions` · `disclosure`  
3. **Public properties:** `title`, `description`, `actions[]`, `collapsible`, `expanded`, `design`  
4. **State:** Controlled `expanded` preferred for collapsible.  
5. **Variants:** `default` · `collapsible`  
6. **Sizes/density:** stack gap from density.  
7. **Visual states:** expanded/collapsed; action hover.  
8. **Interaction states:** toggle collapse.  
9. **Keyboard:** when header focused and collapsible: Enter/Space toggle.  
10. **Mouse:** click header toggle.  
11. **Focus:** header tab stop if collapsible; actions separate stops.  
12. **Disabled:** N/A body; actions respect disabled.  
13. **Loading:** body may be Skeleton.  
14. **Error:** body may be ErrorView.  
15. **Narrow:** drop description → actions → collapse body by default optional.  
16. **Tiny:** title only.  
17. **Unicode/ASCII:** disclosure `▾`/`▸` vs `v`/`>`.  
18. **Colorless:** bold title; disclosure always visible.  
19. **Composition:** settings pages, Form sections.  
20. **Outcomes:** `Toggled { expanded }` · action `Activated`  
21. **Stories:** `section/default`, `section/collapsible`  
22. **Snapshots:** collapsed/expanded.  
23. **Interaction tests:** keyboard toggle.  
24. **Perf:** O(1) chrome + children.

## Callout

1. **Purpose:** Inline informational banner (non-modal).  
2. **Anatomy:** `root` · `icon` · `title` · `body` · `actions` · `dismiss`  
3. **Public properties:** `tone`, `title`, `body`, `dismissible`, `design`  
4. **State:** visibility controlled by parent.  
5. **Variants:** `info` · `success` · `warning` · `danger` · `neutral`  
6. **Sizes/density:** wrap 2–n lines; pad by density.  
7. **Visual states:** default; dismiss hover.  
8. **Interaction states:** dismiss.  
9. **Keyboard:** if dismissible and focused: Esc only when scene allows local dismiss.  
10. **Mouse:** click dismiss.  
11. **Focus:** optional tab stop when dismissible or actions present.  
12. **Disabled:** N/A.  
13. **Loading:** N/A.  
14. **Error:** tone=danger for soft errors (hard → ErrorView/Alert).  
15. **Narrow:** single-line title + icon.  
16. **Tiny:** icon + title.  
17. **Unicode/ASCII:** tone glyphs `ℹ`/`!`/`✗` → `i`/`!`/`x`.  
18. **Colorless:** tone prefix + bold title.  
19. **Composition:** above forms, inside Section body.  
20. **Outcomes:** `Dismissed` · `ActionActivated`  
21. **Stories:** `callout/tones`, `callout/dismissible`  
22. **Snapshots:** all tones.  
23. **Interaction tests:** dismiss.  
24. **Perf:** O(text).

## Alert

1. **Purpose:** Stronger than Callout; may require acknowledgement.  
2. **Anatomy:** Callout anatomy + `primary_action`  
3. **Public properties:** `tone`, `title`, `body`, `action_label`, `design`  
4. **State:** open/visible controlled.  
5. **Variants:** `inline` · `banner`  
6. **Sizes/density:** as Callout.  
7. **Visual states:** danger/warning emphasis stronger than Callout.  
8. **Interaction states:** acknowledge · dismiss.  
9. **Keyboard:** Enter on primary action when focused.  
10. **Mouse:** click action/dismiss.  
11. **Focus:** primary action preferred initial focus if required.  
12. **Disabled:** N/A.  
13. **Loading:** action loading optional.  
14. **Error:** primary hard-inline error surface (still not PermissionPrompt).  
15. **Narrow:** stack action under body.  
16. **Tiny:** title + action label.  
17. **Unicode/ASCII:** as Callout.  
18. **Colorless:** bold + `!` prefix.  
19. **Composition:** page tops; **not** for agent permissions.  
20. **Outcomes:** `Acknowledged` · `Dismissed`  
21. **Stories:** `alert/danger`, `alert/banner`  
22. **Snapshots:** banner width.  
23. **Interaction tests:** acknowledge path.  
24. **Perf:** O(text).

---

# 3. Layout components

## ScrollArea (Viewport evolution)

1. **Purpose:** Scrollable viewport over borrowed content metrics.  
2. **Anatomy:** `root` · `viewport` · `scrollbar_y` · `scrollbar_x`  
3. **Public properties:** `content_size`, `show_bars` policy, `follow` policy, `design`  
4. **State:** Controlled offsets preferred; or internal scroll state.  
5. **Variants:** `auto_bars` · `always` · `never`  
6. **Sizes/density:** bar thickness 1 cell.  
7. **Visual states:** idle · scrolling · following-tail.  
8. **Interaction states:** drag thumb · wheel.  
9. **Keyboard:** arrows/page/home/end when focused.  
10. **Mouse:** wheel, track click, thumb drag.  
11. **Focus:** focusable when keyboard scroll enabled.  
12. **Disabled:** N/A.  
13. **Loading:** follow tail for streams.  
14. **Error:** N/A.  
15. **Narrow:** hide bars if no overflow.  
16. **Tiny:** no bars; clip.  
17. **Unicode/ASCII:** thumb `█`/`#`, track `│`/`|`.  
18. **Colorless:** reverse thumb.  
19. **Composition:** wraps Markdown, LogStream, Table body.  
20. **Outcomes:** `Scrolled { x, y }`  
21. **Stories:** `scroll-area/both-axes`, `scroll-area/follow`  
22. **Snapshots:** overflow bars.  
23. **Interaction tests:** page bounds; follow disables on manual scroll.  
24. **Perf:** O(1) chrome + child paint.

## WorkspacePane / Split (plan 042)

1. **Purpose:** Resizable multi-pane tree for application shells.  
2. **Anatomy:** `root` · `pane[]` · `divider` · optional `tab_strip`  
3. **Public properties:** pane tree, ratios, min sizes, collapse policy, `design`  
4. **State:** ratios, collapsed flags, focused leaf, zoom.  
5. **Variants:** horizontal · vertical · tabs · stack.  
6. **Sizes/density:** min pane sizes from DimensionTokens.  
7. **Visual states:** dragging divider · focused leaf border.  
8. **Interaction states:** resize · focus move · collapse.  
9. **Keyboard:** pane focus cycle; keyboard resize when divider focused.  
10. **Mouse:** drag divider; click pane.  
11. **Focus:** leaf owns content focus; divider optional.  
12. **Disabled:** N/A.  
13–14. N/A.  
15. **Narrow:** collapse by priority list.  
16. **Tiny:** single leaf only.  
17. **Unicode/ASCII:** divider `│`/`|`, `─`/`-`.  
18. **Colorless:** focused leaf border bold.  
19. **Composition:** AgentWorkbench, ResourceBrowser.  
20. **Outcomes:** `RatioChanged` · `Collapsed` · `FocusPane` · `Zoomed`  
21. **Stories:** `workspace/split-resize`, `workspace/narrow-collapse`  
22. **Snapshots:** 2-pane + collapsed.  
23. **Interaction tests:** resize clamp to mins.  
24. **Perf:** layout O(panes); no thrash alloc per frame.

*(Panel remains the bordered Surface with title + PanelEmphasis until fully aliased to Surface recipes.)*

---

# 4. Navigation

## Tabs

1. **Purpose:** Switch among peer views.  
2. **Anatomy:** `root` · `tab[]` · `label` · `icon` · `badge` · `underline` · `close`  
3. **Public properties:** `tabs`, `selected_id`, `closable`, `design`  
4. **State:** Controlled `selected_id` preferred; hover internal.  
5. **Variants:** `line` · `enclosed`  
6. **Sizes/density:** strip height 1–2 rows by density.  
7. **Visual states:** active, inactive, hover, disabled, focus-visible underline.  
8. **Interaction states:** selecting · closing.  
9. **Keyboard:** Left/Right, Home/End, Enter; optional 1–9.  
10. **Mouse:** click tab; click close.  
11. **Focus:** roving tabindex; one focused tab in strip.  
12. **Disabled:** skip disabled tabs.  
13. **Loading:** badge spinner on tab.  
14. **Error:** N/A (badge tone optional).  
15. **Narrow:** truncate labels; drop badge → icon; horizontal scroll strip.  
16. **Tiny:** active tab first grapheme only.  
17. **Unicode/ASCII:** underline focus; close `×`/`x`.  
18. **Colorless:** active = bold + underline.  
19. **Composition:** Workspace tab_strip, Dialog multi-page.  
20. **Outcomes:** `Selected(Id)` · `Close(Id)`  
21. **Stories:** `tabs/status`, `tabs/closable`, `tabs/narrow`  
22. **Snapshots:** active underline; narrow truncate.  
23. **Interaction tests:** keyboard roving; close.  
24. **Perf:** O(tabs).

## Sidebar

1. **Purpose:** Vertical navigation for app sections.  
2. **Anatomy:** `root` · `header` · `item[]` · `icon` · `label` · `badge` · `footer`  
3. **Public properties:** `items`, `selected`, `collapsed` (rail), `design`  
4. **State:** Controlled selected; collapsed controlled or internal.  
5. **Variants:** `expanded` · `rail`  
6. **Sizes/density:** width tokens ~16–28 expanded; rail 3.  
7. **Visual states:** selected, hover, disabled.  
8. **Interaction states:** activate · collapse toggle.  
9. **Keyboard:** Up/Down/Enter; optional `[` rail toggle.  
10. **Mouse:** click item; click collapse control.  
11. **Focus:** list-like roving within sidebar.  
12. **Disabled:** skip items.  
13. **Loading:** section skeleton / item loading.  
14. **Error:** N/A.  
15. **Narrow:** force rail.  
16. **Tiny:** rail only.  
17. **Unicode/ASCII:** icons catalog.  
18. **Colorless:** selected gutter `>` + reverse.  
19. **Composition:** + Breadcrumbs content area.  
20. **Outcomes:** `Selected(Id)` · `ToggledCollapse` · `Activated(Id)`  
21. **Stories:** `sidebar/expanded`, `sidebar/rail`  
22. **Snapshots:** rail vs expanded.  
23. **Interaction tests:** nav + collapse.  
24. **Perf:** O(visible items).

## Breadcrumbs

1. **Purpose:** Path hierarchy navigation.  
2. **Anatomy:** `root` · `crumb[]` · `separator` · `overflow`  
3. **Public properties:** `items`, `max_visible`, `design`  
4. **State:** none for path (parent owns); internal overflow open optional.  
5. **Variants:** `default`  
6. **Sizes/density:** 1 row.  
7. **Visual states:** hover non-current; current muted/bold.  
8. **Interaction states:** activate ancestor · open overflow.  
9. **Keyboard:** Left/Right among crumbs; Enter activate.  
10. **Mouse:** click crumb.  
11. **Focus:** roving among crumbs or single widget + index.  
12. **Disabled:** N/A.  
13–14. N/A.  
15. **Narrow:** collapse middle to `…` overflow.  
16. **Tiny:** last crumb only.  
17. **Unicode/ASCII:** sep `/` or `›`/`>`.  
18. **Colorless:** current bold; others dim.  
19. **Composition:** ResourceBrowser header.  
20. **Outcomes:** `Activated(Id)` · `OverflowOpened`  
21. **Stories:** `breadcrumbs/overflow`, `breadcrumbs/deep`  
22. **Snapshots:** overflow ellipsis.  
23. **Interaction tests:** activate non-current; overflow.  
24. **Perf:** O(items).

## Menu

1. **Purpose:** Hierarchical command menu (menubar or panel).  
2. **Anatomy:** `root` · `item[]` · `label` · `shortcut` · `submenu_marker` · `separator` · `check`  
3. **Public properties:** `items` tree, `open_path`, `design`  
4. **State:** open path (indices/ids); highlight index.  
5. **Variants:** `bar` · `panel`  
6. **Sizes/density:** item height 1; pad by density.  
7. **Visual states:** open, hover/highlight, disabled, checked.  
8. **Interaction states:** submenu open · activate.  
9. **Keyboard:** arrows navigate; Right open submenu; Left close; Enter activate; Esc close one level.  
10. **Mouse:** click; optional hover-open delay.  
11. **Focus:** highlight follows focus; trap while open panel.  
12. **Disabled:** skip; no activate.  
13. **Loading:** N/A.  
14. **Error:** N/A.  
15. **Narrow:** drop shortcuts first.  
16. **Tiny:** labels only, clip.  
17. **Unicode/ASCII:** submenu `▸`/`>`; check `✓`/`*`.  
18. **Colorless:** reverse highlight.  
19. **Composition:** ContextMenu shares item model; ActionBar overflow.  
20. **Outcomes:** `Activated(Id)` · `Cancelled` · `OpenChanged`  
21. **Stories:** `menu/nested`, `menu/disabled-items`  
22. **Snapshots:** nested open path.  
23. **Interaction tests:** Esc peels one level; Enter activates leaf.  
24. **Perf:** O(visible items).

## ContextMenu

1. **Purpose:** Pointer-triggered Menu at a point.  
2. **Anatomy:** Menu anatomy + `anchor`  
3. **Public properties:** `items`, `position` / anchor rect, `design`  
4. **State:** open boolean + Menu open path.  
5. **Variants:** `default`  
6. **Sizes/density:** as Menu; clamp to parent.  
7. **Visual states:** as Menu.  
8. **Interaction states:** open at · dismiss.  
9. **Keyboard:** as Menu when open.  
10. **Mouse:** right-click open; outside click dismiss; click item.  
11. **Focus:** trap while open; restore prior focus on close.  
12. **Disabled:** target may suppress open.  
13–14. N/A.  
15. **Narrow:** flip/clamp placement (CompletionMenu rules).  
16. **Tiny:** may refuse open if &lt;10 cols free—document fallback to Menu panel.  
17–18. as Menu.  
19. **Composition:** List/Table/Tree row actions.  
20. **Outcomes:** Menu outcomes ∪ `OpenAt { x, y }`  
21. **Stories:** `context-menu/basic`, `context-menu/clamp`  
22. **Snapshots:** placement clamp.  
23. **Interaction tests:** outside dismiss; Esc.  
24. **Perf:** O(items).

## ActionBar / HintBar / StatusBar

Supporting navigation chrome (exist today).

| Component | Purpose summary | Key contraction |
|-----------|-----------------|-----------------|
| **ActionBar** | Horizontal Button/IconButton group | Drop labels → icons → overflow Menu |
| **HintBar** | Keymap-driven Kbd+labels | Priority drop right-to-left |
| **StatusBar** | Slots with priority | Drop low-priority slots first |

Full Button/Kbd contracts apply to parts. Stories: existing + density/narrow. Outcomes: `Activated` from actions; HintBar none; StatusBar optional slot activate.

---

# 5. Forms and input

## TextInput

1. **Purpose:** Single-line grapheme-safe editor.  
2. **Anatomy:** `root` · `label` · `value` · `placeholder` · `prefix` · `suffix` · `clear` · `message`  
3. **Public properties:** value (controlled or state buffer), `placeholder`, `validation` projection, `password`, `design`  
4. **State:** Uncontrolled common: buffer, cursor, scroll, selection. Controlled value optional.  
5. **Variants:** `default` · `password` · `search`  
6. **Sizes/density:** height 1; pad by density.  
7. **Visual states:** default, focus, invalid, disabled, read-only.  
8. **Interaction states:** editing · clearing.  
9. **Keyboard:** edit keys; Enter → Submit; Esc → Cancel when configured; Ctrl-U clear optional.  
10. **Mouse:** click set cursor; optional drag select.  
11. **Focus:** caret visible; focus-visible border/underline.  
12. **Disabled:** no edit; dim.  
13. **Loading:** N/A (async validation is consumer).  
14. **Error/validation:** invalid style + message below (consumer text); `TextInputValidity`.  
15. **Narrow:** drop label → prefix/suffix → clear.  
16. **Tiny:** value only.  
17. **Unicode:** grapheme cursor and delete.  
18. **Colorless:** underline invalid; reverse selection.  
19. **Composition:** Form fields, Combobox query, CommandPalette query.  
20. **Outcomes:** `Changed` · `Submit` · `Cancelled` · `Cleared`  
21. **Stories:** `text-input/basic`, `text-input/unicode`, `text-input/invalid`, `text-input/password`  
22. **Snapshots:** invalid + password mask.  
23. **Interaction tests:** grapheme delete; submit; disabled.  
24. **Perf:** O(width) paint.

## TextArea

1. **Purpose:** Multi-line grapheme-safe editor.  
2. **Anatomy:** `root` · `gutter?` · `lines` · `scrollbar` · `placeholder`  
3. **Public properties:** text state, title, placeholder, wrap policy, `design`  
4. **State:** buffer, cursor, scroll, selection (uncontrolled typical).  
5. **Variants:** `default` · `code`  
6. **Sizes/density:** min height Comfortable=3, Compact=2, Dashboard=2.  
7. **Visual states:** focus, disabled, read-only, invalid.  
8. **Interaction states:** editing · scrolling.  
9. **Keyboard:** multiline edit; Esc cancel policy; optional submit chord.  
10. **Mouse:** click cursor; wheel scroll; drag select.  
11. **Focus:** caret; focus chrome.  
12. **Disabled:** no edit.  
13. **Loading:** N/A.  
14. **Error:** invalid border + message.  
15. **Narrow:** reflow wrap; hide gutter.  
16. **Tiny:** min 2 rows if possible else single-line fallback guidance.  
17. **Unicode:** grapheme ops.  
18. **Colorless:** underline invalid.  
19. **Composition:** PromptComposer, Form long fields.  
20. **Outcomes:** `Changed` · `Scrolled` · `Cancelled` · `Submit` (if enabled)  
21. **Stories:** existing + `text-area/wrap`, `text-area/invalid`  
22. **Snapshots:** multi-line unicode.  
23. **Interaction tests:** enter newline; scroll bounds; undo if exposed.  
24. **Perf:** O(visible lines); edit ops amortized O(line).

## Checkbox

1. **Purpose:** Boolean field with label.  
2. **Anatomy:** `root` · `box` · `label` · `description`  
3. **Public properties:** `id`, `checked`, `indeterminate`, `enabled`, `label`, `description`, `design`  
4. **State:** Controlled `checked` preferred.  
5. **Variants:** `default`  
6. **Sizes/density:** 1 row; +description → 2.  
7. **Visual states:** checked, unchecked, indeterminate, disabled, focus, invalid.  
8. **Interaction states:** toggle.  
9. **Keyboard:** Space toggle; Enter optional same.  
10. **Mouse:** click root.  
11. **Focus:** tab stop on control.  
12. **Disabled:** no toggle.  
13. **Loading:** N/A.  
14. **Error:** invalid group message (Form).  
15. **Narrow:** drop description.  
16. **Tiny:** box only.  
17. **Unicode/ASCII:** `☑`/`☐` vs `[x]`/`[ ]`/`[-]`.  
18. **Colorless:** `x` mark always.  
19. **Composition:** Form, List leading, MultiSelect options.  
20. **Outcomes:** `Toggled { id, checked }`  
21. **Stories:** `checkbox/states`, `checkbox/indeterminate`  
22. **Snapshots:** three check states.  
23. **Interaction tests:** Space toggles; disabled.  
24. **Perf:** O(1).

## RadioGroup

1. **Purpose:** Exactly one choice among options.  
2. **Anatomy:** `root` · `legend` · `option[]` · `radio` · `label`  
3. **Public properties:** `options`, `selected_id`, `enabled`, `design`  
4. **State:** Controlled `selected_id`.  
5. **Variants:** `vertical` · `horizontal`  
6. **Sizes/density:** gap by density.  
7. **Visual states:** selected option, disabled options, focus, invalid group.  
8. **Interaction states:** move · select.  
9. **Keyboard:** arrows move; Space select (if move≠select); Home/End.  
10. **Mouse:** click option.  
11. **Focus:** roving tabindex within group.  
12. **Disabled:** group or per-option; skip.  
13. **Loading:** N/A.  
14. **Error:** required validation consumer message.  
15. **Narrow:** force vertical.  
16. **Tiny:** selected option only + change opens full group guidance.  
17. **Unicode/ASCII:** `(●)`/`( )` vs `(*)`/`( )`.  
18. **Colorless:** `*` selected.  
19. **Composition:** Form, Permission decision row alternative, QuestionFlow.  
20. **Outcomes:** `Selected(Id)`  
21. **Stories:** `radio-group/basic`, `radio-group/horizontal`  
22. **Snapshots:** selected middle.  
23. **Interaction tests:** arrow wrap policy; disabled skip.  
24. **Perf:** O(options).

## Switch

1. **Purpose:** Immediate binary setting (settings-style).  
2. **Anatomy:** `root` · `track` · `thumb` · `label`  
3. **Public properties:** `id`, `on`, `enabled`, `label`, `design`  
4. **State:** Controlled `on`.  
5. **Variants:** `default`  
6. **Sizes/density:** track width 4–6 cells.  
7. **Visual states:** on/off, disabled, focus, loading.  
8. **Interaction states:** toggle.  
9. **Keyboard:** Space toggle.  
10. **Mouse:** click.  
11. **Focus:** tab stop.  
12. **Disabled:** no toggle.  
13. **Loading:** thumb spinner / ignore toggle.  
14. **Error:** N/A.  
15. **Narrow:** label then switch (priority: keep switch).  
16. **Tiny:** text `ON`/`OFF`.  
17. **Unicode/ASCII:** `[=]`/`[ ]` or `●─`/`─●`.  
18. **Colorless:** reverse track when on.  
19. **Composition:** settings Section rows.  
20. **Outcomes:** `Toggled { id, on }`  
21. **Stories:** `switch/basic`, `switch/loading`  
22. **Snapshots:** on/off.  
23. **Interaction tests:** Space.  
24. **Perf:** O(1).

## Form

1. **Purpose:** Labeled fields layout, focus order, validation display (exists).  
2. **Anatomy:** `root` · `section[]` · `field[]` · `label` · `control` · `help` · `error`  
3. **Public properties:** sections/fields projection, `design`  
4. **State:** focused field id, scroll to field; values consumer-owned.  
5. **Variants:** `default` · `compact`  
6. **Sizes/density:** field gap by density.  
7. **Visual states:** field invalid, section collapsed if used.  
8. **Interaction states:** field navigate · submit request.  
9. **Keyboard:** Tab / shift-tab fields; Enter submit if configured.  
10. **Mouse:** click field focus.  
11. **Focus:** one field control.  
12. **Disabled:** per-field.  
13. **Loading:** submit loading on primary action.  
14. **Error:** field-level + form-level messages.  
15. **Narrow:** stack label above control.  
16. **Tiny:** focused field only.  
17–18. child contracts.  
19. **Composition:** TextInput/TextArea/Checkbox/Select…  
20. **Outcomes:** `FocusField` · `SubmitRequested` · child outcomes  
21. **Stories:** existing form stories + invalid.  
22. **Snapshots:** multi-field invalid.  
23. **Interaction tests:** tab order; submit.  
24. **Perf:** O(visible fields).

---

# 6. Selection

## Select

1. **Purpose:** Single value from options in an overlay list.  
2. **Anatomy:** `root` · `trigger` · `value` · `chevron` · `menu` · `option[]`  
3. **Public properties:** `options`, `selected`, `placeholder`, `enabled`, `design`  
4. **State:** open + selected; highlight in menu.  
5. **Variants:** `default`  
6. **Sizes/density:** trigger height 1.  
7. **Visual states:** closed/open, invalid, disabled.  
8. **Interaction states:** open menu · choose.  
9. **Keyboard:** Enter/Space open; arrows; Enter choose; Esc close; typeahead optional.  
10. **Mouse:** click trigger/option; outside closes.  
11. **Focus:** trigger; trap in menu while open.  
12. **Disabled:** no open.  
13. **Loading:** async options → Skeleton/Empty in menu.  
14. **Error:** invalid trigger chrome.  
15. **Narrow:** menu full width of parent.  
16. **Tiny:** value only; menu may full-screen.  
17–18. as List options.  
19. **Composition:** Form fields.  
20. **Outcomes:** `Selected(Id)` · `Opened` · `Cancelled`  
21. **Stories:** `select/basic`, `select/loading-options`  
22. **Snapshots:** open menu.  
23. **Interaction tests:** Esc closes without change; Enter selects.  
24. **Perf:** O(visible options).

## MultiSelect

1. **Purpose:** Many selected values with chips and/or checks.  
2. **Anatomy:** Select anatomy + `chip_row` · check options  
3. **Public properties:** `options`, `selected: &[Id]`, `design`  
4. **State:** Controlled selection set; open.  
5. **Variants:** `checkbox` · `chips`  
6. **Sizes/density:** chip row wrap.  
7. **Visual states:** chips selected; menu checks.  
8. **Interaction states:** toggle membership.  
9. **Keyboard:** as Select; Space toggles option without close.  
10. **Mouse:** click option toggle; chip remove.  
11. **Focus:** trigger + chip removes + menu.  
12. **Disabled:** no change.  
13. **Loading:** as Select.  
14. **Error:** invalid if required empty.  
15. **Narrow:** chip row scroll/drop trailing chips + count Badge.  
16. **Tiny:** count Badge only + open.  
17–18. Checkbox + Tag rules.  
19. **Composition:** filters, Form.  
20. **Outcomes:** `SelectionChanged { selected }` · `Cancelled`  
21. **Stories:** `multi-select/chips`, `multi-select/checkbox`  
22. **Snapshots:** multi chips.  
23. **Interaction tests:** toggle membership; chip remove.  
24. **Perf:** O(visible options + chips).

## Combobox

1. **Purpose:** Filterable select / free-text hybrid (Picker evolution).  
2. **Anatomy:** `root` · `input` · `list` · `empty` · `footer`  
3. **Public properties:** options projection, filter consumer-owned or helper, `mode`, `design`  
4. **State:** query, list selection, open (PickerState lineage).  
5. **Variants:** `select_only` · `free_text`  
6. **Sizes/density:** input 1 + list min 3.  
7. **Visual states:** empty, loading, no-match.  
8. **Interaction states:** typing · navigating results.  
9. **Keyboard:** type → query; Up/Down results; Enter activate; Esc clear query then close.  
10. **Mouse:** click row; wheel list.  
11. **Focus:** input vs list policy (document: arrows move to list).  
12. **Disabled:** no edit/open.  
13. **Loading:** Skeleton rows.  
14. **Error:** invalid free_text validation.  
15. **Narrow:** full width overlay.  
16. **Tiny:** input only.  
17–18. TextInput + List.  
19. **Composition:** SessionPicker, resource filter, CommandPalette cousin.  
20. **Outcomes:** `Activated(Id)` · `QueryChanged` · `FreeTextSubmit(String)` · `Cancelled`  
21. **Stories:** existing picker + `combobox/free-text`, `combobox/empty`  
22. **Snapshots:** no-match empty.  
23. **Interaction tests:** two-stage Esc; activate.  
24. **Perf:** filter O(n) consumer; paint O(visible).

## List

1. **Purpose:** Selectable rows with optional multi-check; primary collection primitive.  
2. **Anatomy:** `container` · `row` · `leading` · `primary` · `secondary` · `badge` · `shortcut` · `selection_indicator` · `scrollbar`  
3. **Public properties:** rows projection, selection mode, `design`  
4. **State:** selected_id, multi `Selection`, offset, hover, focus.  
5. **Variants:** `single` · `multi`  
6. **Sizes/density:** row pad from density/recipes.  
7. **Visual states:** selected, hover, disabled, loading row.  
8. **Interaction states:** navigate · activate · toggle.  
9. **Keyboard:** intents Move/Page/Home/End/Activate/Toggle/Cancel.  
10. **Mouse:** click row; wheel.  
11. **Focus:** list focus; cursor row.  
12. **Disabled:** skip rows.  
13. **Loading:** per-row loading flag.  
14. **Error:** N/A.  
15. **Narrow:** part drop priority: shortcut → badge → secondary → leading → primary (never drop primary first).  
16. **Tiny:** primary only + selection cue.  
17. **Unicode/ASCII:** gutter glyphs.  
18. **Colorless:** gutter `>` + reverse/underline.  
19. **Composition:** CommandPalette body, Sidebar, Combobox list.  
20. **Outcomes:** `Changed` · `Activated(Id)` · `Toggled` · `Cancelled` · `Ignored`  
21. **Stories:** existing + `list/composed-row`, `list/narrow`, `list/multi`  
22. **Snapshots:** part contraction matrix.  
23. **Interaction tests:** intents; disabled skip; multi toggle.  
24. **Perf:** O(visible rows).

## Tree

1. **Purpose:** Hierarchical expand/collapse navigation.  
2. **Anatomy:** List-like row + `disclosure` · `indent` · `status`  
3. **Public properties:** nodes projection, `design`  
4. **State:** expanded set, selected, offset.  
5. **Variants:** `default` · `directory`  
6. **Sizes/density:** indent cells by density (2/1/1).  
7. **Visual states:** expanded/collapsed, loading node, selected.  
8. **Interaction states:** expand · collapse · activate.  
9. **Keyboard:** Left/Right expand/collapse; arrows move; Enter activate.  
10. **Mouse:** click disclosure; click row.  
11. **Focus:** tree focus + cursor node.  
12. **Disabled:** skip.  
13. **Loading:** `TreeNodeStatus::Loading`.  
14. **Error:** N/A.  
15. **Narrow:** reduce indent; drop secondary.  
16. **Tiny:** primary + disclosure.  
17. **Unicode/ASCII:** `▾`/`▸` vs `v`/`>`.  
18. **Colorless:** disclosure always; selected reverse.  
19. **Composition:** ResourceBrowser west pane.  
20. **Outcomes:** `Expanded` · `Collapsed` · `Activated` · `Changed`  
21. **Stories:** existing tree stories + loading node.  
22. **Snapshots:** deep indent.  
23. **Interaction tests:** left on expanded collapses; on root no-op.  
24. **Perf:** O(visible nodes).

## CompletionMenu

Supporting selection overlay (exists): candidate list + size planner + place helper. Contracts as Combobox list layer; outcomes `Accepted(Id)` · `Cancelled`. Stories/tests existing.

---

# 7. Feedback

## Toast

1. **Purpose:** Transient non-modal notification.  
2. **Anatomy:** `root` · `icon` · `message` · `close`  
3. **Public properties:** severity, lifetime, anchor, message, `design`  
4. **State:** ToastState TTL queue.  
5. **Variants:** by Severity; Anchor positions.  
6. **Sizes/density:** max width ~48; 1–3 lines.  
7. **Visual states:** entering/visible/leaving (motion optional).  
8. **Interaction states:** dismiss.  
9. **Keyboard:** optional focus for close; not default trap.  
10. **Mouse:** click close.  
11. **Focus:** usually not stealing; sticky toast may be focusable.  
12. **Disabled:** N/A.  
13. **Loading:** N/A.  
14. **Error:** severity danger.  
15. **Narrow:** full width bottom.  
16. **Tiny:** message only one line.  
17–18. severity glyphs.  
19. **Composition:** OverlayHost non-blocking layer.  
20. **Outcomes:** `Dismissed` · `Expired`  
21. **Stories:** existing toast stories.  
22. **Snapshots:** severities.  
23. **Interaction tests:** TTL expire; manual dismiss.  
24. **Perf:** O(toasts visible).

## Progress

1. **Purpose:** Determinate or indeterminate progress.  
2. **Anatomy:** `track` · `fill` · `label` · `percent`  
3. **Public properties:** `kind` (determinate/indeterminate), value 0..=1, label, `design`  
4. **State:** none (value controlled).  
5. **Variants:** `bar` · `meter`  
6. **Sizes/density:** height 1.  
7. **Visual states:** partial/full/indeterminate.  
8–12. N/A interactive.  
13. **Loading:** indeterminate uses Spinner frames when Motion on.  
14. **Error:** optional danger tone on failure value.  
15. **Narrow:** drop percent then label.  
16. **Tiny:** bar only min 3 cols.  
17. **Unicode/ASCII:** blocks `█`/`#`.  
18. **Colorless:** fill reverse/bold density.  
19. **Composition:** ToolCallCard, downloads, TokenMeter cousin.  
20. **Outcomes:** none.  
21. **Stories:** existing progress stories.  
22. **Snapshots:** 0/50/100 + indeterminate fixed tick.  
23. **Interaction tests:** N/A.  
24. **Perf:** O(width).

## Skeleton

1. **Purpose:** Placeholder bones while content loads.  
2. **Anatomy:** `root` · `bone[]`  
3. **Public properties:** `rows`, `variant` (`text`/`card`/`table`), `design`  
4. **State:** none.  
5. **Variants:** `text` · `card` · `table`  
6. **Sizes/density:** bone height/gap from density.  
7. **Visual states:** pulse if Motion full; static if off.  
8–12. N/A.  
13. **Loading:** this *is* loading UI.  
14. **Error:** N/A (ErrorView).  
15. **Narrow:** fewer/shorter bones.  
16. **Tiny:** one bone.  
17. **Unicode/ASCII:** `░`/`~`.  
18. **Colorless:** dim only.  
19. **Composition:** List/Table body placeholder.  
20. **Outcomes:** none.  
21. **Stories:** `skeleton/text`, `skeleton/table`, `skeleton/motion-off`  
22. **Snapshots:** variants; motion-off deterministic.  
23. **Interaction tests:** N/A.  
24. **Perf:** O(bones).

## EmptyState

1. **Purpose:** Zero-data explanation with optional recovery action.  
2. **Anatomy:** `root` · `glyph` · `title` · `description` · `action`  
3. **Public properties:** title, description, glyph, action?, `design`  
4. **State:** action Button state only.  
5. **Variants:** `default` · `search` · `error-lite`  
6. **Sizes/density:** vertical pad; max description ~48 cols.  
7. **Visual states:** default; action focus/hover.  
8. **Interaction states:** action activate.  
9. **Keyboard:** Tab action; Enter activate.  
10. **Mouse:** click action.  
11. **Focus:** action if present; else not a stop.  
12. **Disabled:** action disabled respected.  
13. **Loading:** N/A (Skeleton/LoadingView).  
14. **Error:** prefer ErrorView for hard failures.  
15. **Narrow:** stack parts.  
16. **Tiny:** glyph + title.  
17. **Unicode/ASCII:** glyph catalog.  
18. **Colorless:** bold title; dim description.  
19. **Composition:** List/Table empty slot.  
20. **Outcomes:** `ActionActivated(Id)`  
21. **Stories:** `empty-state/basic`, `empty-state/with-action`, `empty-state/narrow`  
22. **Snapshots:** with/without action.  
23. **Interaction tests:** action Enter/click.  
24. **Perf:** O(text).

## LoadingView / ErrorView

| | LoadingView | ErrorView |
|--|-------------|-----------|
| 1 Purpose | Full-region loading | Hard failure surface |
| 2 Anatomy | spinner · label | glyph · title · detail · action |
| 3–6 | tick, block/inline | title/detail, default/compact |
| 9–12 | non-interactive | action like EmptyState |
| 13–14 | is loading | is error |
| 15–16 | label truncate / spinner only | stack / title+action |
| 20 | none | ActionActivated |
| 21–24 | motion-off stories | retry action tests |

---

# 8. Overlays

## Dialog

1. **Purpose:** Modal container with title/body/footer.  
2. **Anatomy:** `backdrop` · `frame` · `title` · `body` · `footer` · `close`  
3. **Public properties:** open, size constraints, title, `design`  
4. **State:** open controlled; focus trap via InteractionScene.  
5. **Variants:** `default` · `danger`  
6. **Sizes/density:** min from DimensionTokens; inset density.  
7. **Visual states:** open; danger border role.  
8. **Interaction states:** dismiss · confirm.  
9. **Keyboard:** Esc per scene policy; Tab cycle trap; Enter default footer.  
10. **Mouse:** footer clicks; optional outside-click per policy.  
11. **Focus:** trap inside; initial focus primary or first control.  
12. **Disabled:** N/A shell.  
13. **Loading:** footer primary loading.  
14. **Error:** danger variant + body Callout.  
15. **Narrow:** full width; stack footer.  
16. **Tiny:** title + footer only.  
17. **Unicode/ASCII:** single-line border glyphs.  
18. **Colorless:** bold title; danger `!`.  
19. **Composition:** MessageDialog / ChoiceDialog specialize; body any.  
20. **Outcomes:** `Closed` · `Confirmed` · footer `Activated`  
21. **Stories:** existing dialog + choice + message.  
22. **Snapshots:** danger title.  
23. **Interaction tests:** Esc dismiss; focus trap.  
24. **Perf:** O(area) + children.

## Drawer

1. **Purpose:** Edge-docked modal panel.  
2. **Anatomy:** `backdrop` · `panel` · `title` · `body` · `footer?`  
3. **Public properties:** `side: Left|Right|Bottom`, size fraction, open, `design`  
4. **State:** open controlled.  
5. **Variants:** side variants.  
6. **Sizes/density:** width/height % of workspace.  
7. **Visual states:** open; focused border.  
8. **Interaction states:** dismiss.  
9. **Keyboard:** Esc dismiss; Tab trap.  
10. **Mouse:** outside on backdrop dismiss if allowed.  
11. **Focus:** trap.  
12–14. as Dialog.  
15. **Narrow:** full width/height.  
16. **Tiny:** full screen panel.  
17–18. as Dialog.  
19. **Composition:** settings, TaskRail expanded.  
20. **Outcomes:** `Closed`  
21. **Stories:** `drawer/right`, `drawer/bottom-narrow`  
22. **Snapshots:** right dock.  
23. **Interaction tests:** Esc.  
24. **Perf:** O(area).

## Popover

1. **Purpose:** Non-modal anchored floating content.  
2. **Anatomy:** `frame` · `arrow?` · `content`  
3. **Public properties:** anchor rect, placement preference, open, `design`  
4. **State:** open.  
5. **Variants:** `default`  
6. **Sizes/density:** auto size to content max.  
7. **Visual states:** open.  
8. **Interaction states:** dismiss.  
9. **Keyboard:** Esc dismiss; no full app trap if non-modal policy.  
10. **Mouse:** outside click dismiss.  
11. **Focus:** optional initial focus inside.  
12–14. N/A.  
15. **Narrow:** flip/clamp (CompletionMenu placement).  
16. **Tiny:** may become Dialog fallback—document.  
17–18. elevated surface mono bold border.  
19. **Composition:** Select menu, rich Tooltip.  
20. **Outcomes:** `Closed`  
21. **Stories:** `popover/flip`, `popover/clamp`  
22. **Snapshots:** flip placements.  
23. **Interaction tests:** outside dismiss.  
24. **Perf:** O(content).

## Tooltip

1. **Purpose:** Delayed label on hover/focus-visible.  
2. **Anatomy:** `frame` · `text`  
3. **Public properties:** `text`, `delay_ms`, `design`  
4. **State:** visible internal + delay timer (FrameTick).  
5. **Variants:** `default`  
6. **Sizes/density:** max ~40 cols; 1–2 wrap.  
7. **Visual states:** hidden/shown.  
8. **Interaction states:** show/hide.  
9. **Keyboard:** show on focus-visible.  
10. **Mouse:** show on hover after delay.  
11. **Focus:** follows owner focus-visible.  
12. **Disabled:** owner disabled → no show.  
13–14. N/A.  
15. **Narrow:** clamp inside window.  
16. **Tiny:** hide if &lt;10 free cols.  
17–18. plain text.  
19. **Composition:** IconButton, truncated Table cells.  
20. **Outcomes:** none.  
21. **Stories:** `tooltip/basic`, `tooltip/delay`  
22. **Snapshots:** shown state.  
23. **Interaction tests:** delay with fake ticks; no show disabled.  
24. **Perf:** O(1).

## CommandPalette

1. **Purpose:** Fuzzy command launcher overlay.  
2. **Anatomy:** `root` · `query` · `list` · `empty` · `footer_hints`  
3. **Public properties:** commands projection, filter consumer-owned, `design`  
4. **State:** query + list selection (palette state).  
5. **Variants:** `default`  
6. **Sizes/density:** min width DimensionTokens; center elevated.  
7. **Visual states:** empty, loading, results.  
8. **Interaction states:** query vs results.  
9. **Keyboard:** type query; up/down; Enter activate; Esc clear then close.  
10. **Mouse:** click row.  
11. **Focus:** trap in overlay layer.  
12. **Disabled:** skip disabled commands.  
13. **Loading:** Skeleton/Empty swap by consumer.  
14. **Error:** N/A.  
15. **Narrow:** full width; drop footer.  
16. **Tiny:** query only.  
17–18. List rules.  
19. **Composition:** scene modal/palette layer.  
20. **Outcomes:** `Activated(Id)` · `QueryChanged` · `Cancelled`  
21. **Stories:** existing + `command-palette/loading`  
22. **Snapshots:** empty query.  
23. **Interaction tests:** two-stage Esc.  
24. **Perf:** filter O(n) consumer; paint O(visible).

## Backdrop / JumpOverlay

| | Backdrop | JumpOverlay |
|--|----------|-------------|
| Purpose | Occlusion wash | Letter-jump targets |
| Interactive | no | badge assign + jump |
| Outcomes | none | JumpOutcome |
| Notes | Reset bg sacred | scene registration |

---

# 9. Data presentation

## Table

1. **Purpose:** Columnar grid with borrowed cells, stable row IDs; consumer owns sort/filter execution.  
2. **Anatomy:** `container` · `header_cell[]` · `sort_icon` · `body_row[]` · `cell[]` · `scrollbar_y` · `empty`  
3. **Public properties:** columns, rows, selection mode, width policies, `design`  
4. **State:** selected_row, offset, hover, focused region; sort **request** only.  
5. **Variants:** `plain` · `bordered`  
6. **Sizes/density:** row height 1; cell inset by density; columns collapse right-first.  
7. **Visual states:** sorted header, selected row, disabled row, empty.  
8. **Interaction states:** navigate · sort-requested · activate.  
9. **Keyboard:** move/page/home/end; Enter activate; sort when header focused.  
10. **Mouse:** row click; header click → SortRequested; wheel.  
11. **Focus:** table; optional header vs body.  
12. **Disabled:** rows skip.  
13. **Loading:** skeleton rows projection.  
14. **Error:** consumer empty/error surface.  
15. **Narrow:** collapse columns right-first; keep identity column.  
16. **Tiny:** first column + selected row.  
17. **Unicode/ASCII:** grapheme cells; sort icons catalog.  
18. **Colorless:** bold sort header; reverse selected.  
19. **Composition:** ScrollArea/Workspace; no nested tables.  
20. **Outcomes:** `Changed` · `Activated(RowId)` · `SortRequested { column, direction }` · `Ignored`  
21. **Stories:** `table/basic`, `table/sorted`, `table/narrow`, `table/unicode`, `table/empty`  
22. **Snapshots:** sort icons, narrow collapse.  
23. **Interaction tests:** keyboard select; sort is request only.  
24. **Perf:** O(visible rows × cols).

## DataTable

1. **Purpose:** Opinionated product table on Table—stripes, toolbar, bulk selection—still no fetch/sort execution.  
2. **Anatomy:** Table + `toolbar` · `bulk_bar` · `pinned_shadow`  
3. **Public properties:** Table props + `striped`, `show_toolbar`, `bulk_actions[]`, `pinned_leading_cols`  
4. **State:** Table state + optional bulk `Selection`.  
5. **Variants:** `default` · `striped` · `compact`  
6. **Sizes/density:** maps to cell pad + toolbar visibility.  
7. **Visual states:** stripe, bulk bar visible, pinned edge.  
8. **Interaction states:** bulk mode.  
9. **Keyboard:** Table + Space bulk-toggle; Ctrl-A → SelectAllRequested only.  
10. **Mouse:** Table + toolbar hits.  
11. **Focus:** toolbar separate stops; body table focus.  
12. **Disabled:** bulk actions disabled if empty selection.  
13. **Loading:** toolbar spinner + skeleton body.  
14. **Error:** composed Callout above.  
15. **Narrow:** toolbar labels→icons; column collapse; bulk bar stacks.  
16. **Tiny:** document List fallback or single-column Table.  
17–18. Table + stripe via dim alternate.  
19. **Composition:** wraps Table; toolbar Buttons.  
20. **Outcomes:** Table ∪ `BulkAction` ∪ `SelectAllRequested` ∪ `ClearBulk`  
21. **Stories:** `datatable/striped`, `datatable/bulk`, `datatable/narrow-toolbar`  
22. **Snapshots:** stripe, bulk bar, pin edge.  
23. **Interaction tests:** bulk toggle; select-all does not silently fill virtual unload.  
24. **Perf:** Table + bulk set O(selected).

## ObjectInspector

1. **Purpose:** Key/value and nested object inspection (DetailTable evolution).  
2. **Anatomy:** `root` · `row` · `key` · `value` · `action` · `disclosure`  
3. **Public properties:** fields projection, capabilities (copy/link), `design`  
4. **State:** selected field; expanded nested paths.  
5. **Variants:** `flat` · `nested`  
6. **Sizes/density:** key column min width tokens.  
7. **Visual states:** selected row; action hover.  
8. **Interaction states:** select · activate action · expand nested.  
9. **Keyboard:** up/down; Enter action; Left/Right expand.  
10. **Mouse:** click row/action.  
11. **Focus:** inspector focus + row cursor.  
12. **Disabled:** actions per capability.  
13. **Loading:** skeleton fields.  
14. **Error:** field error tone.  
15. **Narrow:** stack key above value.  
16. **Tiny:** selected key=value one line.  
17–18. mono keys bold; values default.  
19. **Composition:** ResourceBrowser east; ToolCallCard detail.  
20. **Outcomes:** `Selected` · `Action(Id)` · `Copied` request · `ToggledExpand`  
21. **Stories:** `object-inspector/flat`, `object-inspector/nested`  
22. **Snapshots:** nested expand.  
23. **Interaction tests:** capability-gated actions.  
24. **Perf:** O(visible fields).

## LogStream

1. **Purpose:** Append-only log with follow (LogPane evolution).  
2. **Anatomy:** `root` · `line[]` · `scrollbar` · `follow_chip` · `level_glyph`  
3. **Public properties:** lines (bounded history projection), `design`  
4. **State:** offset, follow bool.  
5. **Variants:** `plain` · `structured` (level roles).  
6. **Sizes/density:** line 1.  
7. **Visual states:** following · pinned historical.  
8. **Interaction states:** scroll · toggle follow.  
9. **Keyboard:** page/arrows; optional `f` follow.  
10. **Mouse:** wheel breaks follow; click chip.  
11. **Focus:** focusable scroll surface.  
12. **Disabled:** N/A.  
13. **Loading/async:** append O(1) path; auto-scroll if follow.  
14. **Error:** error-level lines tone.  
15. **Narrow:** drop timestamp meta first.  
16. **Tiny:** last line only.  
17. **Unicode:** grapheme safe lines.  
18. **Colorless:** level prefix `E`/`W`/`I`.  
19. **Composition:** OpsDashboard; ToolCallCard expanded log.  
20. **Outcomes:** `Scrolled` · `FollowChanged`  
21. **Stories:** `log-stream/follow`, `log-stream/structured`  
22. **Snapshots:** levels.  
23. **Interaction tests:** append keeps follow; manual scroll clears follow.  
24. **Perf:** O(visible lines); append must not repaint full history buffers.

## Timeline

1. **Purpose:** Temporal event list.  
2. **Anatomy:** `root` · `event` · `bullet` · `time` · `text`  
3. **Public properties:** events projection, `design`  
4. **State:** optional selected + offset.  
5. **Variants:** `default` · `compact`  
6. **Sizes/density:** gap by density.  
7. **Visual states:** selected event.  
8. **Interaction states:** select.  
9. **Keyboard:** up/down.  
10. **Mouse:** click event.  
11. **Focus:** optional.  
12–14. N/A.  
15. **Narrow:** drop time to secondary line.  
16. **Tiny:** text only.  
17. **Unicode/ASCII:** bullet `•`/`*`.  
18. **Colorless:** bold time.  
19. **Composition:** agent history sidebar.  
20. **Outcomes:** `Selected` · `Activated`  
21. **Stories:** existing timeline.  
22. **Snapshots:** multi events.  
23. **Interaction tests:** select.  
24. **Perf:** O(visible events).

## DiffReview

1. **Purpose:** Reviewable unified/split diff with hunk navigation.  
2. **Anatomy:** `root` · `hunk_header` · `line[]` · `gutter` · `marker` (+/−)  
3. **Public properties:** lines, hunk index, `design`  
4. **State:** offset, selected hunk, cursor line.  
5. **Variants:** `unified` · `split` (wide only)  
6. **Sizes/density:** gutter width fixed.  
7. **Visual states:** add/del/context roles.  
8. **Interaction states:** hunk nav · scroll.  
9. **Keyboard:** n/p hunk; page; optional stage request outcome.  
10. **Mouse:** wheel; click hunk.  
11. **Focus:** focusable.  
12. **Disabled:** N/A.  
13. **Loading:** streaming patch append.  
14. **Error:** N/A.  
15. **Narrow:** force unified.  
16. **Tiny:** current hunk header + few lines.  
17. **Unicode:** no mid-grapheme; markers `+`/`-`/` `.  
18. **Colorless:** `+`/`-` prefix always; bold add.  
19. **Composition:** PlanReview, PR agent tools.  
20. **Outcomes:** `HunkActivated` · `Scrolled` · `StageRequested` (optional)  
21. **Stories:** `diff-review/hunks`, `diff-review/narrow-unified`  
22. **Snapshots:** add/del lines.  
23. **Interaction tests:** hunk next/prev bounds.  
24. **Perf:** O(visible lines).

## Charts (Sparkline, BarSeries, SegmentedMeter)

Viz tokens; non-interactive by default; colorless = glyph density only; narrow drops labels; perf O(points visible). Stories exist.

---

# 10. Developer tools

## ThemePicker

1. **Purpose:** Select DesignSystem/theme preset with preview.  
2. **Anatomy:** `root` · `list` · `swatch` · `name` · `preview`  
3. **Public properties:** presets, selected, `design`  
4. **State:** selection index.  
5. **Variants:** `list` · `grid`  
6–12. List-like navigation.  
13–14. N/A.  
15–16. list only on narrow/tiny.  
17–18. swatch mono → name only.  
19. Lookbook / settings.  
20. **Outcomes:** `Selected(PresetId)` · `Preview(PresetId)`  
21–24. existing ThemePicker stories/tests; live preview host (plan 049).

## DesignInspector (studio)

1. **Purpose:** Debug focus, scene layers, tokens, capabilities.  
2. **Anatomy:** panels for layers, focus id, capability, recipes.  
3. **Public properties:** scene snapshot, tokens, `design`  
4. **State:** selected panel tab.  
5–18. lookbook-only; density compact.  
19. Not production default.  
20. none (or copy token outcome).  
21. `studio/inspector`  
22–23. focus id updates.  
24. O(layers).

---

# 11. AI-agent components

## PromptComposer

1. **Purpose:** Multi-line prompt + attachments chrome + send (PromptBox evolution).  
2. **Anatomy:** `root` · `editor` · `attach_chip[]` · `mode_badge` · `send` · `footer_hints`  
3. **Public properties:** text state, mode (`plan`/`build`/…), placeholder, `design`  
4. **State:** editor state + focus; mode controlled.  
5. **Variants:** `simple` · `with_mode`  
6. **Sizes/density:** min height 3 comfortable.  
7. **Visual states:** empty/nonempty; streaming-locked.  
8. **Interaction states:** editing · send.  
9. **Keyboard:** Enter submit (configurable); Alt/Ctrl+Enter newline; Esc cancel policy.  
10. **Mouse:** click editor/send/chips.  
11. **Focus:** editor default.  
12. **Disabled:** read-only while agent running if consumer sets.  
13. **Loading:** disable send while streaming; spinner on send.  
14. **Error:** N/A (Callout above).  
15. **Narrow:** drop chips → mode → hints.  
16. **Tiny:** single-line input fallback.  
17–18. TextArea rules.  
19. **Composition:** AgentWorkbench south.  
20. **Outcomes:** `Submitted { text }` · `Changed` · `Cancelled` · `ModeChanged` · `Detach(Id)`  
21. **Stories:** `prompt-composer/submit`, `prompt-composer/newline`, `prompt-composer/streaming-locked`  
22. **Snapshots:** with mode badge.  
23. **Interaction tests:** Enter vs newline; locked ignores submit.  
24. **Perf:** O(visible editor lines).

## PermissionPrompt

1. **Purpose:** Risk-aware permission card; **no side effects** (ApprovalCard hardened).  
2. **Anatomy:** `frame` · `risk_glyph` · `title` · `detail` · `decision_row`  
3. **Public properties:** risk, title, detail, decisions set, `design`  
4. **State:** **selected decision identity** (not raw index into allow-first array). Default by risk: **High → Deny**; Low/Medium → safest non-destructive documented default (Deny or Defer—not Allow).  
5. **Variants:** `card` · `inline`  
6. **Sizes/density:** full width; decisions wrap/stack.  
7. **Visual states:** risk tones.  
8. **Interaction states:** move decision · confirm.  
9. **Keyboard:** Left/Right; Enter confirm; `n` Deny; `y` only if risk allows explicit allow shortcut; never silent allow on High default.  
10. **Mouse:** click decision.  
11. **Focus:** trap; initial focus = default decision.  
12. **Disabled:** N/A.  
13–14. N/A.  
15. **Narrow:** stack decisions vertically.  
16. **Tiny:** title + Deny/Allow only.  
17. **Unicode/ASCII:** risk glyphs catalog.  
18. **Colorless:** risk prefix always.  
19. **Composition:** scene modal layer above workbench.  
20. **Outcomes:** `Decided(ApprovalDecision)` only—consumer executes policy.  
21. **Stories:** `permission-prompt/high-default-deny`, `permission-prompt/medium`, `permission-prompt/narrow`  
22. **Snapshots:** High default selection = Deny.  
23. **Interaction tests:** Enter on High emits Deny if default; Left/Right bounds; Esc → Deny or Cancelled per scene.  
24. **Perf:** O(1).

## QuestionFlow

1. **Purpose:** Multi-step agent interview questions.  
2. **Anatomy:** `root` · `progress` · `question` · `options` · `nav` (back/skip/next)  
3. **Public properties:** steps projection, `design`  
4. **State:** step_index; answers controlled by consumer.  
5. **Variants:** `single` · `multi` per step.  
6. **Sizes/density:** progress 1 row.  
7. **Visual states:** current step; answered check.  
8. **Interaction states:** answer · navigate.  
9. **Keyboard:** as Radio/MultiSelect; `[` back; `]` next if valid.  
10. **Mouse:** option + nav buttons.  
11. **Focus:** options then nav.  
12. **Disabled:** nav next until valid if required.  
13. **Loading:** step load skeleton.  
14. **Error:** validation on next.  
15. **Narrow:** stack nav.  
16. **Tiny:** question + options only.  
17–18. Radio/MultiSelect.  
19. **Composition:** plan mode overlay.  
20. **Outcomes:** `Answered { step, values }` · `Back` · `Skip` · `Finished`  
21. **Stories:** `question-flow/basic`, `question-flow/required`  
22. **Snapshots:** mid-step.  
23. **Interaction tests:** back/next bounds; required blocks next.  
24. **Perf:** O(options).

## PlanReview

1. **Purpose:** Present plan steps for accept/edit/reject.  
2. **Anatomy:** `root` · `step_list` · `detail` · `actions` (accept/reject/edit)  
3. **Public properties:** steps, selected step, `design`  
4. **State:** selection.  
5. **Variants:** `default`  
6. **Sizes/density:** split list/detail when wide.  
7. **Visual states:** selected step; accepted checkmarks.  
8. **Interaction states:** select · accept · reject.  
9. **Keyboard:** list nav; `a` accept / `r` reject optional outcomes.  
10. **Mouse:** click step/actions.  
11. **Focus:** list or actions.  
12–14. N/A / consumer.  
15. **Narrow:** stack detail under list.  
16. **Tiny:** selected step title + Accept/Reject.  
17–18. List + Markdown.  
19. **Composition:** agent plan mode.  
20. **Outcomes:** `Accepted` · `Rejected` · `StepSelected` · `EditRequested`  
21. **Stories:** `plan-review/basic`, `plan-review/narrow`  
22. **Snapshots:** multi-step.  
23. **Interaction tests:** accept/reject.  
24. **Perf:** O(visible steps).

## ToolCallCard

1. **Purpose:** Tool invocation status card (ToolCard).  
2. **Anatomy:** `frame` · `status_glyph` · `name` · `summary` · `body` · `expand`  
3. **Public properties:** status, name, summary, detail, expanded, `design`  
4. **State:** expanded controlled or uncontrolled.  
5. **Variants:** `compact` · `expanded`  
6. **Sizes/density:** compact 1–2 rows.  
7. **Visual states:** pending/running/ok/error (ToolStatus).  
8. **Interaction states:** expand · activate full.  
9. **Keyboard:** Enter toggle expand when focused.  
10. **Mouse:** click expand/header.  
11. **Focus:** optional tab stop in transcript.  
12. **Disabled:** N/A.  
13. **Loading:** running status + spinner; streaming detail append.  
14. **Error:** error status tone + body.  
15. **Narrow:** drop summary; keep name+status.  
16. **Tiny:** status glyph + name.  
17. **Unicode/ASCII:** status glyphs.  
18. **Colorless:** status letter `R`/`E`/`OK`.  
19. **Composition:** Transcript stream items.  
20. **Outcomes:** `ToggledExpand` · `Activated`  
21. **Stories:** `tool-call-card/running`, `tool-call-card/error`, `tool-call-card/expanded`  
22. **Snapshots:** each ToolStatus.  
23. **Interaction tests:** expand toggle.  
24. **Perf:** O(visible detail lines).

## TaskRail

1. **Purpose:** Side list of tasks/subagents.  
2. **Anatomy:** `root` · `task_row[]` · `status` · `title` · `meta`  
3. **Public properties:** tasks projection, `design`  
4. **State:** selected task.  
5. **Variants:** `compact` · `detailed`  
6. **Sizes/density:** width like Sidebar.  
7. **Visual states:** status colors/glyphs; selected.  
8. **Interaction states:** select · activate.  
9. **Keyboard:** up/down/enter.  
10. **Mouse:** click.  
11. **Focus:** list focus.  
12. **Disabled:** completed optional skip policy.  
13. **Loading:** task running spinner.  
14. **Error:** failed status.  
15. **Narrow:** title only.  
16. **Tiny:** status + first grapheme.  
17–18. List composition.  
19. **Composition:** AgentWorkbench east/west.  
20. **Outcomes:** `Selected` · `Activated`  
21. **Stories:** `task-rail/statuses`  
22. **Snapshots:** mixed statuses.  
23. **Interaction tests:** select.  
24. **Perf:** O(visible).

## SessionPicker

1. **Purpose:** Resume/pick agent sessions.  
2. **Anatomy:** `root` · `query?` · `session_row[]` · `time` · `title` · `preview`  
3. **Public properties:** sessions projection, `design`  
4. **State:** selected, query.  
5. **Variants:** `list` · `combobox`  
6. **Sizes/density:** overlay min size.  
7. **Visual states:** empty, selected.  
8. **Interaction states:** pick · cancel.  
9. **Keyboard:** Combobox/List keys.  
10. **Mouse:** click row.  
11. **Focus:** trap if modal.  
12–14. N/A.  
15. **Narrow:** drop preview.  
16. **Tiny:** title only rows.  
17–18. List/Combobox.  
19. **Composition:** startup overlay.  
20. **Outcomes:** `Picked(Id)` · `Cancelled` · `QueryChanged`  
21. **Stories:** `session-picker/basic`, `session-picker/empty`  
22. **Snapshots:** list rows.  
23. **Interaction tests:** pick/cancel.  
24. **Perf:** O(visible).

## ThinkingBlock / TokenMeter / Transcript

| Component | Notes |
|-----------|--------|
| **ThinkingBlock** | Collapsible reasoning; Esc/enter expand; mono dim body; streaming append |
| **TokenMeter** | Segmented usage; colorless hatch; narrow drop labels |
| **Transcript** | Plan 041 variable-height engine replacing StreamView; virtualize; sticky user/anchor; compose Markdown, ToolCallCard, Permission inline |

Each inherits ScrollArea + content contracts; outcomes Scrolled / ItemActivated / ToggledExpand.

---

# 12. Application blocks

## AgentWorkbench

1. **Purpose:** Flagship composition: TaskRail + Transcript + PromptComposer + Permission/Plan/Question overlays + Status/Token.  
2. **Anatomy:** named panes via Workspace: `west` TaskRail, `center` Transcript, `south` PromptComposer, `overlay` stack, `status` bar.  
3. **Public properties:** layout config, child projections, `design`  
4. **State:** focused pane, open overlays, rail collapsed.  
5. **Variants:** `ide` · `compact`  
6. **Sizes/density:** workspace mins; density cascades to children.  
7. **Visual states:** pane focus borders (semantic).  
8. **Interaction states:** pane focus · overlay stack.  
9. **Keyboard:** pane cycle; Esc peels scene layers (approval → plan → focus prompt).  
10. **Mouse:** click panes/dividers.  
11. **Focus:** scene-owned; one leaf.  
12. **Disabled:** N/A block.  
13. **Loading:** transcript skeleton / stream.  
14. **Error:** ErrorView in center optional.  
15. **Narrow:** collapse TaskRail to rail; stack south.  
16. **Tiny:** Transcript + Prompt only.  
17–18. child rules.  
19. **Composition:** only block; **no domain I/O**.  
20. **Outcomes:** union of children.  
21. **Stories:** `blocks/agent-workbench`, `blocks/agent-workbench-narrow`  
22. **Snapshots:** default layout.  
23. **Interaction tests:** Esc peel approval → prompt focus.  
24. **Perf:** sum of children; no O(n²) layout thrash.

## OpsDashboard / ResourceBrowser / SettingsShell / FormWizard

| Block | Compose | Contraction |
|-------|---------|-------------|
| **OpsDashboard** | charts + LogStream + StatusBar | drop charts first |
| **ResourceBrowser** | Tree + ObjectInspector + preview + Breadcrumbs | tree rail; hide preview |
| **SettingsShell** | Sidebar + Section stack + Form | sidebar rail |
| **FormWizard** | stepper + Form + nav actions | step title + form only |

All: geometry from Workspace; child contracts apply; outcomes = child unions; stories under `blocks/*`.

---

## E. Rust sketches (target API shapes)

```rust
/// Named parts for recipe resolution (list rows, etc.).
pub struct ComposedRow<'a, Id> {
    pub id: Id,
    pub leading: Option<Line<'a>>,
    pub primary: Line<'a>,
    pub secondary: Option<Line<'a>>,
    pub badge: Option<Line<'a>>,
    pub shortcut: Option<&'a str>,
    pub enabled: bool,
    pub loading: bool,
}

/// Typical selection collection outcomes.
pub enum SelectOutcome<Id> {
    Ignored,
    Changed,
    Activated(Id),
    Toggled { id: Id, checked: bool },
    Cancelled,
}

/// Permission decisions remain pure messages.
pub enum ApprovalDecision {
    AllowOnce,
    AllowSession,
    Always,
    Deny,
    Defer,
}
```

**Safety invariant (PermissionPrompt):** selection state stores decision identity; default selection is a function of `ApprovalRisk`, never “index 0 = AllowOnce.”

---

## F. Implementation phasing

| Phase | Deliver |
|-------|---------|
| Kernel 039–040 | PermissionPrompt safety, InteractionScene, overlay Esc |
| 041 | Transcript engine |
| 042 | Workspace tree; Sidebar/Drawer natural fit |
| 043–044 | DesignSystem recipes + universal intents on collections |
| 045 | Composed row anatomy on List/Tree/Menus |
| Primitives wave | Button family, Badge/Tag/Chip/Kbd/Separator/Spinner |
| Forms wave | Checkbox, RadioGroup, Switch, Select, MultiSelect |
| Overlay wave | Menu, ContextMenu, Popover, Tooltip, Drawer |
| Flagship | AgentWorkbench + stories + contraction tests |
| Studio | DesignInspector + capability preview host (048–049) |

---

## G. Definition of done (any component)

- [ ] Anatomy named and recipe-backed  
- [ ] Public properties documented  
- [ ] Controlled vs uncontrolled state explicit  
- [ ] Variants / density / visual / interaction states listed  
- [ ] Keyboard + mouse + focus + disabled specified and tested  
- [ ] Loading / async / error as claimed  
- [ ] Narrow + tiny + unicode + ascii + colorless stories or explicit N/A  
- [ ] Composition rules + typed outcomes (no side effects)  
- [ ] Snapshot + interaction tests  
- [ ] Perf note (+ virtualization test if applicable)  
- [ ] Lookbook story IDs registered  

**Rendering alone never checks the box.**
