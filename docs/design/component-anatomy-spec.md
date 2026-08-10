# TermRock component anatomy, behavior, variants, and state specification

**Status:** binding design target for all component work  
**Audience:** implementers, lookbook authors, migration writers  
**Design system:** [`terminal-design-system.md`](./terminal-design-system.md)  
**Agent prompts:** [`component-prompt-library.md`](./component-prompt-library.md) — 164 implementable prompts (global contract + per-component tasks)  
**Agent collection:** [`termrock-agent.md`](./termrock-agent.md) — `@termrock/agent` source-owned components + AgentWorkbench (patterns extracted from agent TUIs; provider policy stays consumer-owned)  
**Inventory baseline:** public surface of `termrock::widgets` + `patterns` on the experience-layer line  
**Policy:** A component is **not** complete because it paints. Interaction design, focus ownership, contraction (narrow/tiny), capability ladders (unicode/ascii/colorless), typed outcomes, stories, and tests are part of the component.  
**Quality gate:** [`component-quality-standard.md`](./component-quality-standard.md) — mandatory axes, evidence, design lints, machine-readable contracts, CI.

---

## A. Current inventory → proposed taxonomy

**HEAD inventory** (`COMPONENTS.md` / public API): ActionBar, ApprovalCard, Backdrop, Badge, Banner, BarSeries, Button, Callout, Checkbox, ChoiceDialog, CodeBlock, CommandPalette, CompletionMenu, DataTable, DesignInspector, DetailTable, Dialog, DiffView, Drawer, EmptyState, ErrorView, Form, FormWizard, Heading, HintBar, ImageSurface, JumpOverlay, Kbd, List, LoadingView, LogPane, MarkdownView, Menu, MessageDialog, ModeRibbon, Panel, Paragraph, PermissionPrompt, Picker, PlanReview, Popover, Progress, PromptBox, PromptComposer, QuestionFlow, SegmentedMeter, SeparatorLine, SessionPicker, Skeleton, Sparkline, SplitPane, StatusBar, StreamView, Surface, Table, Tabs, TaskRail, TextArea, TextInput, ThemePicker, ThinkingBlock, Timeline, Toast, TokenMeter, ToolCard, Transcript, Tree, Viewport, VirtualGrid (+ patterns: agent_shell, ops_dashboard, resource_browser, studio_shell).

| Proposed name | Taxonomy | HEAD mapping | Status |
|---------------|----------|--------------|--------|
| Button, IconButton | Primitives | Button exists; IconButton incomplete | Harden / add |
| Badge, Tag, Chip, Kbd | Primitives | Badge, Kbd exist; Tag/Chip partial | Harden / add |
| Separator | Primitives | SeparatorLine | Rename recipe-only ok |
| Spinner | Primitives | Inside Progress/Loading | Promote first-class |
| Heading, Paragraph, Markdown, CodeBlock | Content | Heading, Paragraph, MarkdownView, CodeBlock | Rename Markdown |
| Surface, Section, Callout, Alert | Content/Layout | Surface, Panel, Callout, Banner | Section new; Banner→Alert |
| ScrollArea, WorkspacePane | Layout | Viewport, SplitPane | Evolve names |
| Tabs, Sidebar, Breadcrumbs, Menu, ContextMenu | Navigation | Tabs, Menu exist; Sidebar/Breadcrumbs/ContextMenu gaps | Fill |
| ActionBar, HintBar, StatusBar | Navigation | Exist | Contract stories |
| TextInput, TextArea, Checkbox, RadioGroup, Switch, Form | Forms | Input/Area/Checkbox/Form exist; Radio/Switch gaps | Fill |
| Select, MultiSelect, Combobox | Selection | Picker → Combobox; Select/MultiSelect thin | Fill |
| List, Tree, CompletionMenu | Selection | Exist | Full part×state recipes |
| Toast, Progress, Skeleton, EmptyState, LoadingView, ErrorView | Feedback | Exist | Spinner + Alert polish |
| Dialog, Drawer, Popover, Tooltip, CommandPalette | Overlays | Most exist; Tooltip thin | Fill |
| Table, DataTable, ObjectInspector, LogStream, Timeline, DiffReview | Data | Table/DataTable/DetailTable/LogPane/Timeline/DiffView | Rename + harden |
| Charts | Data | Sparkline, BarSeries, SegmentedMeter | Keep |
| ThemePicker, DesignInspector | Dev tools | Exist | Studio only defaults |
| PromptComposer, PermissionPrompt, QuestionFlow, PlanReview, ToolCallCard, TaskRail, SessionPicker | AI-agent | Exist (ToolCard/ApprovalCard lineage) | Harden safety + recipes |
| ThinkingBlock, TokenMeter, Transcript | AI-agent | Exist | Transcript engine |
| AgentWorkbench, OpsDashboard, ResourceBrowser, SettingsShell, FormWizard | Blocks | Patterns + FormWizard | Elevate contracts |

**Completeness rule:** every axis **1–24** is specified, story-covered, and tested—or explicitly **N/A with reason**. Rendering alone never completes a component.

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

1. **Purpose:** Canonical primary action; consumer maps outcome → effects.  
2. **Anatomy:** `root` · `leading` · `label` · `trailing` · optional confirm mark  
3. **Public properties:** label, variant, size, leading/trailing, full_width, accessible_label, ascii, colorless, `system`  
4. **State:** `ActivationState` (enabled, loading, accepts_input, armed, pending confirm).  
5. **Variants:** primary · secondary · quiet · outline · destructive · link · success · command  
6. **Sizes:** compact / normal pad; always 1 row.  
7. **Visual states:** default, accepts_input, armed, disabled, loading, confirm-armed.  
8. **Interaction states:** idle · pressed/armed · confirm_required · activated once.  
9. **Keyboard:** `default_button_intent` (Enter/Space → Activate); Repeat ignored; Space arm/release.  
10. **Mouse:** Down arms; Up inside hit activates; drag-off cancels.  
11. **Focus:** host `accepts_input`; Destructive not safe default.  
12. **Disabled:** no activate; ActionDisabled.  
13. **Loading:** distinct Info/muted + `…`; no activate.  
14. **Error:** N/A.  
15. **Narrow:** drop trailing then leading.  
16. **Tiny:** label start; min hit 3.  
17. **Unicode/ASCII:** grapheme-safe label; ASCII brackets/loading.  
18. **Colorless:** strong primary/danger; outline `[ ]`.  
19. **Composition:** ActionBar, Dialog footer, forms.  
20. **Outcomes:** `Activated` · `ConfirmRequired` · `Pressed` · `Ignored`  
21. **Stories:** `button/{activation,variants,destructive,toolbar,icon,disabled,loading,narrow,unicode}`  
22. **Snapshots:** variants + loading vs disabled.  
23. **Interaction tests:** Enter once; Space arm/release; pending confirm; disabled/loading.  
24. **Perf:** O(1) paint.

## IconButton

1. **Purpose:** Icon-only action; **accessible_label required**.  
2. **Anatomy:** `root` · glyph (via Button leading)  
3. **Public properties:** glyph, accessible_label, variant, ascii, colorless, `system`  
4. **State:** same ActivationState / ButtonState.  
5. **Variants:** quiet / primary / destructive common.  
6. **Sizes:** min hit 3×1.  
7–14. Same activation laws as Button.  
15–16. Tiny: glyph only.  
17–18. Catalog glyph + ASCII.  
19. **Composition:** toolbars, dialog chrome.  
20. **Outcomes:** same ActivationOutcome.  
21. **Stories:** `button/icon`.  
22–24. Via Button tests.

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
13. **Loading:** N/A.  
14. **Error:** N/A.  
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
7. **Visual states:** as Menu (open path, disabled items).  
8. **Interaction states:** open at · navigate · activate · dismiss.  
9. **Keyboard:** arrows, Enter activate, Esc dismiss (Menu keys when open).  
10. **Mouse:** right-click open; outside click dismiss; click item.  
11. **Focus:** trap while open; restore prior focus on close.  
12. **Disabled:** target may suppress open; disabled items skip.  
13. **Loading:** optional spinner on async item (rare).  
14. **Error:** N/A.  
15. **Narrow:** flip/clamp placement (CompletionMenu rules).  
16. **Tiny:** may refuse open if &lt;10 cols free—fallback to full-width Menu panel.  
17. **Unicode/ASCII:** Menu item glyphs catalog.  
18. **Colorless:** selected item reverse; disabled dim.  
19. **Composition:** List/Table/Tree row actions.  
20. **Outcomes:** Menu outcomes ∪ `OpenAt { x, y }`  
21. **Stories:** `context-menu/basic`, `context-menu/clamp`  
22. **Snapshots:** placement clamp.  
23. **Interaction tests:** outside dismiss; Esc.  
24. **Perf:** O(items).

## ActionBar

1. **Purpose:** Horizontal or vertical group of primary actions.  
2. **Anatomy:** `root` · `action[]` · cursor chrome  
3. **Public properties:** actions projection, `system`, `ascii`, `colorless`, `vertical`  
4. **State:** `cursor: Option<Id>` (not scene focus); hit regions.  
5. **Variants:** horizontal · vertical stack  
6. **Sizes/density:** 1 row or N rows when stacked.  
7. **Visual states:** cursor / disabled via Role tokens.  
8. **Interaction states:** host/dialog owns activate.  
9. **Keyboard:** owner (ChoiceDialog / host) moves cursor.  
10. **Mouse:** hit regions for owner click.  
11. **Focus:** paint cursor only; scene focus is host.  
12. **Disabled:** skip disabled in regions.  
13–14. Loading/error: host.  
15. **Narrow:** vertical stack from ChoiceDialog.  
16. **Tiny:** clip width.  
17. **Unicode/ASCII:** `[label]` cursor when ascii.  
18. **Colorless:** TextStrong cursor; `›label‹`.  
19. **Composition:** Dialog footer, workbench chrome.  
20. **Outcomes:** none (paint + regions only).  
21. **Stories:** action-bar via ChoiceDialog stories.  
22. **Snapshots:** cursor glyph.  
23. **Interaction tests:** via ChoiceDialog.  
24. **Perf:** O(actions).

## HintBar

1. **Purpose:** Keymap-driven hints (Kbd + short labels).  
2. **Anatomy:** `root` · `hint[]` (`kbd` · `label`) · `separator`  
3. **Public properties:** hints projection with priority, `design`  
4. **State:** none (or hover highlight).  
5. **Variants:** `inline` · `stacked` (density).  
6. **Sizes/density:** Comfortable 2 rows allowed; Compact/Dashboard 1.  
7. **Visual states:** default.  
8–14. **Interactive/loading/error:** N/A (not focusable by default).  
15. **Narrow:** drop lowest priority right-to-left.  
16. **Tiny:** highest priority single hint.  
17. **Unicode/ASCII:** Kbd rules.  
18. **Colorless:** kbd reverse; label dim.  
19. **Composition:** footer of panels, Dialog, workbench.  
20. **Outcomes:** none.  
21. **Stories:** `hint-bar/priority`, `hint-bar/narrow`  
22. **Snapshots:** priority drop matrix.  
23. **Interaction tests:** N/A.  
24. **Perf:** O(hints).

## StatusBar

1. **Purpose:** Persistent status slots with priority.  
2. **Anatomy:** `root` · `slot[]` · optional `separator`  
3. **Public properties:** slots (`id`, content, priority, interactive?), `design`  
4. **State:** optional focus slot if interactive.  
5. **Variants:** `default`  
6. **Sizes/density:** always 1 row.  
7. **Visual states:** slot tones (ok/warn/error).  
8. **Interaction states:** activate interactive slot.  
9. **Keyboard:** optional Left/Right + Enter if interactive slots.  
10. **Mouse:** click interactive slot.  
11. **Focus:** only if any slot interactive.  
12. **Disabled:** N/A.  
13. **Loading:** spinner slot.  
14. **Error:** danger tone slot.  
15. **Narrow:** drop low-priority slots first.  
16. **Tiny:** single highest-priority slot.  
17. **Unicode/ASCII:** slot glyphs.  
18. **Colorless:** danger/warn prefixes.  
19. **Composition:** workbench south/status.  
20. **Outcomes:** `Activated(Id)` optional.  
21. **Stories:** `status-bar/priority`, `status-bar/interactive`  
22. **Snapshots:** priority drop.  
23. **Interaction tests:** activate interactive.  
24. **Perf:** O(slots).

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
2. **Anatomy:** `root` · `lines` · `scrollbar` · `placeholder` · caret  
3. **Public properties:** title, placeholder, ascii, colorless, `system`  
4. **State:** buffer, caret, scroll, accepts_input, read_only.  
5. **Variants:** default editor surface.  
6. **Sizes/density:** min height 2–3 rows.  
7. **Visual states:** accepts_input chrome, caret, empty placeholder.  
8. **Interaction states:** editing · scrolling · read-only nav.  
9. **Keyboard:** intents for Home/End/Page/Esc; Up/Down/chars on handle_key.  
10. **Mouse:** wheel → Scrolled; scrollbar press/drag.  
11. **Focus:** host accepts_input; caret local.  
12. **Disabled/read-only:** no edit when read_only or !accepts_input.  
13–14. Loading/Error: N/A (Form wraps invalid).  
15. **Narrow:** horizontal scroll.  
16. **Tiny:** min body.  
17. **Unicode:** grapheme ops.  
18. **Colorless:** TextStrong caret; ASCII scroll `|`.  
19. **Composition:** PromptComposer (propagates accepts_input), Form.  
20. **Outcomes:** `Changed` · `Scrolled` · `Cancelled` · `Ignored`  
21. **Stories:** `text-area/{basic,narrow,unicode,empty,scrolled}`  
22. **Snapshots:** multi-line unicode.  
23. **Interaction tests:** accepts_input gate; read_only; grapheme edits.  
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
17. **Unicode/ASCII:** required `*` and disabled `⊘` markers; child control glyphs.  
18. **Colorless:** invalid field bold `!` prefix; required `*` always.  
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
17. **Unicode/ASCII:** option glyphs + chevron from catalog; labels grapheme-safe.  
18. **Colorless:** open menu bold border; selected option reverse/gutter `>`.  
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
17. **Unicode/ASCII:** checkboxes `[x]`/`[ ]`; chip remove `×`/`x`.  
18. **Colorless:** selected options reverse; chips bold label.  
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
17. **Unicode/ASCII:** query grapheme-safe; list gutters catalog.  
18. **Colorless:** cursor cell reverse; selected result gutter/reverse.  
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

1. **Purpose:** Anchored candidate list for editors (completion popup).  
2. **Anatomy:** `frame` · `row[]` · `kind` · `label` · `detail` · `scrollbar`  
3. **Public properties:** candidates projection (stable IDs), selected, `design`  
4. **State:** selected_id, scroll offset; open controlled by editor.  
5. **Variants:** `default` · `rich` (kind + detail)  
6. **Sizes/density:** size planner + `place_completion_menu` clamp/flip.  
7. **Visual states:** selected, empty.  
8. **Interaction states:** navigate · accept · dismiss.  
9. **Keyboard:** Up/Down; Enter/Tab `Accepted`; Esc `Dismissed`.  
10. **Mouse:** click row; wheel.  
11. **Focus:** not a global tab stop; receives keys while editor owns completion mode.  
12. **Disabled:** skip disabled candidates.  
13. **Loading:** Skeleton rows or spinner footer.  
14. **Error:** N/A.  
15. **Narrow:** drop detail then kind.  
16. **Tiny:** label only; clamp inside parent.  
17. **Unicode/ASCII:** kind glyphs catalog.  
18. **Colorless:** selected reverse/gutter.  
19. **Composition:** TextInput/TextArea/PromptComposer completion.  
20. **Outcomes:** `Accepted(Id)` · `Dismissed` · `Changed`  
21. **Stories:** existing completion + `completion-menu/place-flip`  
22. **Snapshots:** placement above/below anchor.  
23. **Interaction tests:** never covers anchor cell; Esc dismiss.  
24. **Perf:** O(visible candidates).

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
17. **Unicode/ASCII:** severity glyphs catalog; close `×`/`x`.  
18. **Colorless:** severity letter prefix (`i`/`!`/`x`/`ok`) + bold message.  
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
8. **Interaction states:** N/A.  
9. **Keyboard:** N/A.  
10. **Mouse:** N/A.  
11. **Focus:** not a tab stop.  
12. **Disabled:** N/A.  
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
8. **Interaction states:** N/A.  
9. **Keyboard:** N/A.  
10. **Mouse:** N/A.  
11. **Focus:** not a tab stop.  
12. **Disabled:** N/A.  
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

## LoadingView

1. **Purpose:** Full-region or inline loading surface.  
2. **Anatomy:** `root` · `spinner` · `label`  
3. **Public properties:** `tick`/`FrameTick`, `label`, `variant` (`block`/`inline`), `design`  
4. **State:** frame from tick + Motion.  
5. **Variants:** `block` · `inline`  
6. **Sizes/density:** block centers in parent; inline 1 row.  
7. **Visual states:** animating / static (Motion off).  
8–12. **Interaction/focus/disabled:** non-interactive; not a tab stop.  
13. **Loading:** this *is* loading UI.  
14. **Error:** N/A (use ErrorView).  
15. **Narrow:** truncate label.  
16. **Tiny:** spinner only.  
17. **Unicode/ASCII:** Spinner catalog.  
18. **Colorless:** same glyphs; dim label.  
19. **Composition:** panel body, dialog body, workbench center.  
20. **Outcomes:** none.  
21. **Stories:** `loading-view/block`, `loading-view/motion-off`  
22. **Snapshots:** fixed tick.  
23. **Interaction tests:** Motion::Off stable.  
24. **Perf:** O(1).

## ErrorView

1. **Purpose:** Hard failure surface with optional recovery action.  
2. **Anatomy:** `root` · `glyph` · `title` · `detail` · `action`  
3. **Public properties:** title, detail, action?, `design`  
4. **State:** action Button state only.  
5. **Variants:** `default` · `compact`  
6. **Sizes/density:** compact drops detail padding.  
7. **Visual states:** danger tone; action states.  
8. **Interaction states:** action activate.  
9. **Keyboard:** Tab action; Enter activate.  
10. **Mouse:** click action.  
11. **Focus:** action if present.  
12. **Disabled:** action disabled respected.  
13. **Loading:** N/A.  
14. **Error:** this *is* error UI.  
15. **Narrow:** stack parts.  
16. **Tiny:** title + action.  
17. **Unicode/ASCII:** danger glyph catalog.  
18. **Colorless:** `!`/`x` prefix + bold title.  
19. **Composition:** workbench center, dialog body, empty failure.  
20. **Outcomes:** `ActionActivated(Id)`  
21. **Stories:** `error-view/basic`, `error-view/retry`  
22. **Snapshots:** compact vs default.  
23. **Interaction tests:** retry activate.  
24. **Perf:** O(text).

---

# 8. Overlays

## Dialog

1. **Purpose:** Modal container with title/body/footer.  
2. **Anatomy:** `backdrop` · `frame` · `title` · `body` · `footer` · action bar  
3. **Public properties:** title, body, variant, footer_hint, loading, `system`  
4. **State:** open on OverlayStack; ChoiceDialog holds action **cursor** + regions.  
5. **Variants:** `default` · `danger` · `info`  
6. **Sizes/density:** DialogSize density table + OverlayPolicy.  
7. **Visual states:** open; danger border + `!` title; loading glyph.  
8. **Interaction states:** cancel · activate · action cursor move.  
9. **Keyboard:** Esc cancel; Enter activate; Left/Right cursor; **Tab = host scene**.  
10. **Mouse:** action hit regions; outside-click per OverlayStack policy.  
11. **Focus:** overlay trap on stack; action cursor local (host may project scene).  
12. **Disabled:** loading blocks activate; `accepts_input` gate.  
13. **Loading:** title busy glyph + ChoiceDialogState::set_loading.  
14. **Error:** danger variant + body Callout.  
15. **Narrow:** full-width promote; vertical action stack.  
16. **Tiny:** title + border only (height &lt; 3).  
17. **Unicode/ASCII:** single-line border; action `[label]` / `›label‹`.  
18. **Colorless:** bold title; danger `!`; strong action cursor.  
19. **Composition:** MessageDialog / ChoiceDialog specialize; body any.  
20. **Outcomes:** `Cancelled` · `Activated` · `Changed` (cursor) · `Ignored`  
21. **Stories:** dialog + choice + message (+ narrow/unicode).  
22. **Snapshots:** danger title; action cursor; narrow stack.  
23. **Interaction tests:** Esc; skip disabled; accepts_input; Tab ignored locally.  
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
12. **Disabled:** N/A shell; body children respect own disabled.  
13. **Loading:** footer primary loading as Dialog.  
14. **Error:** danger border + body Callout as Dialog.  
15. **Narrow:** full width/height.  
16. **Tiny:** full screen panel.  
17. **Unicode/ASCII:** single-line border glyphs.  
18. **Colorless:** bold title; danger `!` when risk.  
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
12. **Disabled:** N/A shell.  
13. **Loading:** content may be Skeleton.  
14. **Error:** N/A (content Callout).  
15. **Narrow:** flip/clamp (CompletionMenu placement).  
16. **Tiny:** may become Dialog fallback—document.  
17. **Unicode/ASCII:** elevated single-line border glyphs.  
18. **Colorless:** bold border; elevated dim surface via reverse edge.  
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
13. **Loading:** N/A.  
14. **Error:** N/A.  
15. **Narrow:** clamp inside window.  
16. **Tiny:** hide if &lt;10 free cols.  
17. **Unicode/ASCII:** plain text; grapheme-safe wrap.  
18. **Colorless:** reverse or bold tooltip frame.  
19. **Composition:** IconButton, truncated Table cells.  
20. **Outcomes:** none.  
21. **Stories:** `tooltip/basic`, `tooltip/delay`  
22. **Snapshots:** shown state.  
23. **Interaction tests:** delay with fake ticks; no show disabled.  
24. **Perf:** O(1).

## CommandPalette

1. **Purpose:** Fuzzy command launcher overlay.  
2. **Anatomy:** `root` · `query` · `list` · `empty` · `footer_hints`  
3. **Public properties:** rows projection, `system`, `focused`, `ascii`, `colorless`, footer/empty copy  
4. **State:** PickerState (query + list cursor + accepts_input).  
5. **Variants:** default chrome; unfocused Normal panel.  
6. **Sizes/density:** CommandPaletteSize + OverlayPolicy promote.  
7. **Visual states:** empty, results, unfocused surface.  
8. **Interaction states:** query edit vs list cursor.  
9. **Keyboard:** type query; j/k page; Enter; Esc clear then close.  
10. **Mouse:** click row (picker).  
11. **Focus:** OverlayStack trap; host accepts_input; list cursor local.  
12. **Disabled:** skip disabled rows in list.  
13. **Loading:** consumer swaps empty message / rows.  
14. **Error:** N/A.  
15. **Narrow:** drop footer; fullscreen promote.  
16. **Tiny:** query-first; short empty.  
17. **Unicode/ASCII:** query grapheme-safe; empty `∅`/`[ ]`.  
18. **Colorless:** muted empty; list selection gutters.  
19. **Composition:** OverlayStack CommandPalette kind.  
20. **Outcomes:** `Activated` · `QueryChanged` · `CursorMoved` · `Cancelled` · `Ignored`  
21. **Stories:** `command-palette/{basic,empty,ascii,narrow,unicode}`  
22. **Snapshots:** empty, footer, unfocused border.  
23. **Interaction tests:** two-stage Esc; accepts_input; overlay open.  
24. **Perf:** filter O(n) consumer; paint O(visible).

## Backdrop

1. **Purpose:** Occlusion wash behind modal content.  
2. **Anatomy:** `root` (full rect) · optional `wash_glyph` field  
3. **Public properties:** `design` (reads backdrop token)  
4. **State:** none.  
5. **Variants:** `reset` · `dim` (glyph wash)  
6. **Sizes/density:** fills parent.  
7. **Visual states:** visible only.  
8–14. **Interaction/loading/error:** non-interactive; clicks may bubble as outside-dismiss to owner.  
15–16. **Narrow/tiny:** full area still.  
17. **Unicode/ASCII:** optional `░`/` `.  
18. **Colorless:** Reset bg; no color required.  
19. **Composition:** Dialog, Drawer, CommandPalette.  
20. **Outcomes:** optional `OutsideClick` to owner.  
21. **Stories:** `backdrop/reset`, `backdrop/dim`  
22. **Snapshots:** dim wash.  
23. **Interaction tests:** outside click forwarding.  
24. **Perf:** O(area) fill.

## JumpOverlay

1. **Purpose:** Letter-jump targets over registered hit regions.  
2. **Anatomy:** `badge[]` over scene geometry  
3. **Public properties:** registered targets from InteractionScene, `design`  
4. **State:** typed prefix buffer.  
5. **Variants:** `default`  
6. **Sizes/density:** badge 1–2 cells.  
7. **Visual states:** candidate / matched / eliminated.  
8. **Interaction states:** type filter · jump · cancel.  
9. **Keyboard:** printable accumulates; Esc cancel; exact match jumps.  
10. **Mouse:** click badge jumps.  
11. **Focus:** overlay layer owns keys until done.  
12–14. N/A.  
15–16. clamp badges inside window; drop if no space.  
17. **Unicode/ASCII:** ASCII letters preferred for typing.  
18. **Colorless:** reverse badges.  
19. **Composition:** scene overlay registration.  
20. **Outcomes:** `Jumped(Id)` · `Cancelled`  
21. **Stories:** existing jump-overlay.  
22. **Snapshots:** multi-badge.  
23. **Interaction tests:** type disambiguation; Esc.  
24. **Perf:** O(targets).

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
17. **Unicode/ASCII:** Table cell graphemes; toolbar icon catalog.  
18. **Colorless:** stripe via dim alternate rows; bulk bar reverse header; selected reverse.  
19. **Composition:** wraps Table; toolbar Buttons.  
20. **Outcomes:** Table ∪ `BulkAction` ∪ `SelectAllRequested` ∪ `ClearBulk`  
21. **Stories:** `datatable/striped`, `datatable/bulk`, `datatable/narrow-toolbar`  
22. **Snapshots:** stripe, bulk bar, pin edge.  
23. **Interaction tests:** bulk toggle; select-all does not silently fill virtual unload.  
24. **Perf:** Table + bulk set O(selected).

## ObjectInspector

1. **Purpose:** Key/value and nested object inspection (DetailTable evolution).  
2. **Anatomy:** `root` · `row` · `key` · `value` · cursor gutter · empty mark  
3. **Public properties:** fields projection, `system`, `focused`, `ascii`, `colorless`  
4. **State:** field cursor + scroll; `accepts_input` host gate. Nested expand is projection.  
5. **Variants:** `flat` · `nested` (via `depth`)  
6. **Sizes/density:** responsive key/value line recipes.  
7. **Visual states:** cursor row (surface-focused vs unfocused marks).  
8. **Interaction states:** cursor move · activate · wheel · click.  
9. **Keyboard:** j/k arrows Home/End page; Enter/Space activate (`default_inspector_intent`).  
10. **Mouse:** click row → cursor; second click → activate; wheel moves cursor.  
11. **Focus:** scene owns surface; inspector owns field cursor.  
12. **Disabled:** `accepts_input = false` ignores keys/mouse.  
13. **Loading:** host projects skeleton fields.  
14. **Error:** host projects error value tones.  
15. **Narrow:** `key=value` compact.  
16. **Tiny:** cursor shows value; others key.  
17. **Unicode/ASCII:** cursor `›`/`>`; empty `∅`/`[ ]`; keys grapheme-safe.  
18. **Colorless:** cursor uses TextStrong; empty mark always present.  
19. **Composition:** ResourceBrowser east; OpsDashboard detail; ToolCallCard.  
20. **Outcomes:** `CursorMoved` · `Activate { index }` · `Scrolled` · `Ignored`  
21. **Stories:** `object-inspector/{flat,nested,empty,narrow,ascii}`  
22. **Snapshots:** cursor gutter, empty, narrow, cursor-follow.  
23. **Interaction tests:** intent, accepts_input, mouse, page, home/end.  
24. **Perf:** O(visible fields).

## LogStream

1. **Purpose:** Continuous professional log viewer (stern/k9s-class; LogPane projects into it).  
2. **Anatomy:** `root` · `title?` · `reconnect_banner?` · `line[]` · `follow_chip` · `level_glyph` · empty mark  
3. **Public properties:** `LogLine` projection (id, level, text, timestamp?, source?, styled?, batch), `system`, `focused`, `ascii`, `colorless`, `title`  
4. **State:** `ScrollAreaState` follow/unread, cursor, multi-select, bookmarks, search, level floor, wrap/h-scroll, recipe, dropped/reconnect/batch, anchors, regions, `accepts_input`.  
5. **Variants:** levels Trace…Error; recipes Compact | Detailed; wrap Clip | Wrap.  
6. **Sizes/density:** virtualized window; chip last row when height ≥ 2; detailed vs compact recipes.  
7. **Visual states:** following · pinned · unread · dropped · reconnect · search/filter chip · selection · bookmark.  
8. **Interaction states:** scroll · follow · select · search · filter · bookmark · copy/export · h-scroll · wrap/recipe.  
9. **Keyboard:** j/k page Home/End; `f` follow; Space select; `/` search; `[` level; `m` bookmark; `c` copy; `C-e` export; `b` ack; `w`/`d` wrap/recipe; h/l h-scroll.  
10. **Mouse:** wheel scrolls + detaches; click chip re-follows; click line sets cursor; Shift+click multi-select.  
11. **Focus:** scene owns surface; stream owns scroll/follow/cursor.  
12. **Disabled:** `accepts_input = false`.  
13. **Loading/async:** `on_append` O(1) rejoin when following; host reports drop/batch/reconnect.  
14. **Error:** error-level Danger / colorless strong; reconnect banner Warning.  
15. **Narrow:** severity + body (drop timestamp/source).  
16. **Tiny:** body only.  
17. **Unicode/ASCII:** glyphs `i!x` vs `IWE`; chip `↓`/`v`; bookmark `★`/`*`.  
18. **Colorless:** letter marks + strong/muted.  
19. **Composition:** OpsDashboard; LogPane via `log_lines_from_plain`; EventStream for structured.  
20. **Outcomes:** `Scrolled` · `Follow` · `Detach` · `SelectionChanged` · `Copy` · `Export` · `BookmarkToggled` · `SearchChanged` · `LevelFilter` · `HScrolled` · `Cancelled` · `AckDropped` · `Ignored`  
21. **Stories:** `log-stream/{follow,structured,filter,dropped,empty,narrow,ascii}`  
22. **Snapshots:** levels, chip, empty, narrow, filter, drop.  
23. **Interaction tests:** append keeps follow; scroll detaches; chip/f re-follow; search/level; copy/bookmark; anchors.  
24. **Perf:** O(visible lines); `log_stream_bench`; sustained paint test; host projects window.

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
11. **Focus:** optional list focus.  
12. **Disabled:** skip disabled events.  
13. **Loading:** pending event Skeleton row.  
14. **Error:** failed event danger tone.  
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

## FileTree

1. **Purpose:** FS-specialized Tree with typed file-op requests.  
2. **Anatomy:** title? · filter? · rename draft? · tree body · confirm banner.  
3. **Public properties:** `FileTreeEntry[]` (kind, git, path, lazy, errors).  
4. **State:** embeds `TreeState`; filter; hidden/ignored; draft; confirm.  
5. **Variants:** file/dir/symlink; git M/A/D/?; lazy/loading/error.  
6–10. Yazi-like chords; multi-delete confirm.  
11–14. No FS/Git IO; host pages huge dirs.  
15–18. ASCII kind glyphs; status letters.  
19. **Composition:** Tree paint; QuickOpen; Breadcrumbs.  
20. **Outcomes:** Open/Create/Rename/Delete/Preview/LoadChildren/…  
21. **Stories:** `file-tree/{basic,filter,hidden,confirm,empty,narrow,ascii}`  
22–24. Filter ancestor retention; path normalize tests.

## ProcessTable

1. **Purpose:** Process/task monitor with tree and flat modes.  
2. **Anatomy:** title? · filter? · column header · body rows · signal confirm.  
3. **Public properties:** `ProcessRow[]` (key, parent, depth, cmd, cpu, mem, status, user, elapsed).  
4. **State:** selection + multi-check; view mode; sort; filters; confirm; refresh_ms; `VirtualWindow`.  
5. **Variants:** Flat / Tree; status R/S/D/T/Z/X.  
6–10. htop/btop chords; TERM/KILL confirm; multi-signal.  
11–14. No process enum/kill; host refresh + signal.  
15–18. ASCII selection/tree; status letter roles.  
19. **Composition:** column model kit; TreeTable remains generic substrate.  
20. **Outcomes:** SignalRequested · ConfirmRequired · Refresh · Details · Sort/View/Filter · CopyCommand.  
21. **Stories:** `process-table/{basic,tree,filter,confirm,empty,narrow,ascii}`  
22–24. PID-reuse reconcile; 5k-row sort/paint bench constants.

## QueryEditor

1. **Purpose:** Code-oriented SQL/logs/search query workbench.  
2. **Anatomy:** chrome · params? · editor · diagnostics? · results slot · footer.  
3. **Public properties:** language, parameters, diagnostics projection, result summary.  
4. **State:** embeds `TextAreaState`; focus zones; run status; mode; completion; slots.  
5. **Variants:** Compact / Normal / Fullscreen; Idle/Running/Success/Failed.  
6–10. Ctrl chords for run/stop/format/save/history/complete/focus.  
11–14. No language server / DB driver / formatter execution.  
15–18. ASCII focus marks; severity letters on diagnostics.  
19. **Composition:** TextArea · CompletionMenu · Diagnostic/CodeFrame · KeyboardHelp · HistoryPicker · ResultGrid/DataTable in results slot.  
20. **Outcomes:** Run/Stop/Format/Save/History/Completion/JumpToDiagnostic/Focus/Mode.  
21. **Stories:** `query-editor/{basic,running,diagnostics,parameters,compact,empty,narrow,ascii}`  
22–24. Draft survives focus; large-draft paint; no-exec guard.

## ResultGrid

1. **Purpose:** Typed query result grid on DataTable.  
2. **Anatomy:** status · stats? · DataTable body (row# + schema columns).  
3. **Public properties:** `ResultColumn[]`, `ResultRow[]` (typed cells), status, redaction.  
4. **State:** embeds `DataTableState<u64,String>`; schema; stats; redaction; row_numbers.  
5. **Variants:** Idle/Streaming/Ready/Failed; Safe vs RevealSecrets.  
6–10. DataTable nav + export/inspect/page/stats chords.  
11–14. No query execution; host pages unknown totals.  
15–18. ASCII NULL; secret mask; blob(N).  
19. **Composition:** DataTable paint; QueryEditor summary bridge; ObjectInspector fields.  
20. **Outcomes:** Export · Inspect · CellDetail · Page · Edit · Sort · RevealSecrets.  
21. **Stories:** `result-grid/{basic,streaming,stats,wide,empty,error,narrow,ascii}`  
22–24. Wide schema project; 500-row page paint; no-driver guard.

## SchemaBrowser

1. **Purpose:** Hierarchical DB catalog navigator.  
2. **Anatomy:** title · filter? · Tree body.  
3. **Public properties:** `SchemaBrowserEntry[]` (kind, path, lazy/error, type, key badges).  
4. **State:** embeds `TreeState`; filter; presentation; expanded preserve set.  
5. **Variants:** SidePane/Drawer/Fullscreen; conn status; Lazy/Loading/Error.  
6–10. Tree nav + query/describe/refresh/reconnect/QuickOpen.  
11–14. No catalog SQL; host lazy-loads children.  
15–18. ASCII kind glyphs; connection letters.  
19. **Composition:** Tree paint; QuickOpen; Breadcrumbs; QueryEditor open.  
20. **Outcomes:** Open/LoadChildren/Refresh/Reconnect/ContextAction/QuickOpen.  
21. **Stories:** `schema-browser/{basic,lazy,filter,error,drawer,empty,narrow,ascii}`  
22–24. Expand preserve + filter ancestors; 5k-object filter paint.

## SearchResults

1. **Purpose:** Grouped navigable search hits (files/logs/objects/commands/docs).  
2. **Anatomy:** status · group bands · item rows (title + snippet).  
3. **Public properties:** groups, items with MatchRange on title/snippet, source, line.  
4. **State:** cursor, VirtualWindow, generation, collapsed, match_walk, status.  
5. **Variants:** Idle/Loading/Partial/Ready/Empty/Error/Stale/Cancelled.  
6–10. j/k open preview; n/N match walk; group toggle; page.  
11–14. No search I/O; host begin_search/apply_results generation gate.  
15–18. Keep-first-match truncate; ASCII marks.  
19. **Composition:** HighlightedText; SearchInput host; QuickOpen/FullscreenViewer.  
20. **Outcomes:** Open/Preview/MatchWalk/GroupToggled/Cancel/Page/Fullscreen.  
21. **Stories:** `search-results/{basic,loading,empty,stale,collapsed,streaming,narrow,ascii}`  
22–24. Generation stale tests; 2k-hit paint; match keep-visible.

## MetricsDashboard

1. **Purpose:** Observability dashboard from public Sparkline/Gauge APIs.  
2. **Anatomy:** toolbar · metric tile grid/summary · alerts · footer.  
3. **Public properties:** MetricTile[] (value, samples, thresholds, health), alerts.  
4. **State:** time_range, comparison, focus zone/tile, layout override, pause.  
5. **Variants:** Grid vs Summary (≤48 cols); tile health Ok/Warn/Danger/Failed.  
6–10. Spatial hjkl; Tab zones; Ctrl+R/T/D/K commands.  
11–14. No scrape/query IO; partial failure per tile.  
15–18. Health letters; ASCII sparkline/gauge glyphs.  
19. **Composition:** Sparkline · Gauge · CommandPalette entries.  
20. **Outcomes:** DrillDown · Refresh · TimeRange · Comparison · AlertActivated.  
21. **Stories:** `metrics-dashboard/{basic,narrow,partial-fail,paused,empty,ascii}`  
22–24. Layout grid/summary; 24-tile paint; public-API-only guard.

## TraceWaterfall

1. **Purpose:** Hierarchical span latency waterfall (traces / agent tools).  
2. **Anatomy:** chrome · filter? · time ruler · name col · bar axis.  
3. **Public properties:** TraceSpan[] (start/duration/depth/status/critical/service).  
4. **State:** selection, VirtualWindow, time window, nav mode, filter, expanded.  
5. **Variants:** Hierarchy vs Timeline nav; critical-only filter; ASCII bars.  
6–10. j/k select; h/l expand or pan; zoom Ctrl+=−0; Shift+wheel time.  
11–14. No OTLP/fetch; host projects flattened expanded spans.  
15–18. Exact duration labels; status letters; critical marks.  
19. **Composition:** ObjectInspector fields; Timeline export bridge.  
20. **Outcomes:** Selection · Expand · Details · Filter · TimeWindow · CriticalPath.  
21. **Stories:** `trace-waterfall/{basic,error,critical,zoomed,empty,narrow,ascii}`  
22–24. Bar clamp math; 2k-span paint; expand/zoom tests.

## DependencyGraph

1. **Purpose:** Constrained package/service/schema/task dependency map.  
2. **Anatomy:** chrome · filter? · graph canvas **or** tree/list body.  
3. **Public properties:** DepNode[] · DepEdge[] (directed kind/status).  
4. **State:** preferred/effective view, pan, selection, filter, force_tree.  
5. **Variants:** Graph / Tree / List; auto tree when narrow or large.  
6–10. j/k select; pan [ ] / Ctrl+hjkl; Ctrl+V view cycle.  
11–14. No package resolve; deterministic layered layout only.  
15–18. ASCII connectors; status letters; kind glyphs.  
19. **Composition:** TreeTable-shaped projection; ObjectInspector fields.  
20. **Outcomes:** Selection · Details · EdgeSelected · ViewChanged · Panned.  
21. **Stories:** `dependency-graph/{basic,tree,list,filter,narrow,ascii}`  
22–24. Deterministic layout; 80-node auto-tree; no cargo metadata.

## Charts (Sparkline / Chart / Gauge / Histogram)

1. **Purpose:** Coherent terminal data-viz family.  
2. **Anatomy:** optional title/legend · plot · axes · threshold marks · selection.  
3. **Public properties:** samples/series/buckets, `ScaleMode`, `VizGlyphSet`, thresholds, selection.  
4. **State:** stateless paint (host owns series buffers).  
5. **Variants:** Auto/Fixed/Log; Block/Braille/ASCII; vertical/horizontal hist.  
6–8. Tiny: 1-row sparkline/gauge; multi-row chart/hist.  
9–10. Non-interactive paint (host may wrap selection).  
11–14. No process I/O.  
15–16. Readable at 8×1.  
17–18. No-color markers + density.  
19. **Composition:** dashboards, StatusBar, TokenMeter cousin.  
20. **Outcomes:** none (pure paint).  
21. **Stories:** sparkline/chart/gauge/histogram/bar-series/segmented-meter.  
22–24. Streaming window + scale property tests.

## HexViewer

1. **Purpose:** Virtualized binary inspector (host-paged).  
2. **Anatomy:** title? · search? · rows(offset\|hex\|ascii) · inspector.  
3. **Public properties:** `HexWindow`, title, focused/ascii/colorless.  
4. **State:** absolute cursor/selection, bpr, endian, ascii mode, bookmarks, search.  
5. **Variants:** LE/BE; Ascii/Unicode/Dots; bpr 4–64 or auto.  
6–8. Tiny collapses to compact hex; wide shows full xxd-like rows.  
9. **Keyboard:** hjkl · select · /search · b bookmark · e endian · c/x/y copy.  
10. **Mouse:** wheel · click byte column.  
11–14. Scene focus; host pages on `PageNeeded`.  
15–16. Width &lt; 28 compact.  
17–18. Brackets/braces/bookmarks without color.  
19. **Composition:** FullscreenViewer body.  
20. **Outcomes:** Cursor/Selection · Copy · Export · PageNeeded · SearchHit.  
21. **Stories:** `hex-viewer/{basic,selection,inspector,search,empty,narrow,ascii}`  
22–24. Property tests for offsets/widths; O(visible rows).

## TerminalOutput

1. **Purpose:** Safe command-run presentation (never executes).  
2. **Anatomy:** status header · command · cwd? · env? · body lines · follow chip.  
3. **Public properties:** `TerminalCommandMeta`, `TerminalLine[]`, title, focused/ascii.  
4. **State:** follow via `ScrollAreaState`, recipe, paint mode, filters, env, cursor, regions.  
5. **Variants:** status Pending…Detached; streams stdout/stderr/system.  
6–8. Compact/pane/fullscreen; ANSI/no-color/plain/raw.  
9. **Keyboard:** f follow · c cancel/copy · r retry · d detach · e env · m/p modes.  
10. **Mouse:** wheel · chip re-follow · click line.  
11–14. Scene focus; host owns process; outcomes are requests.  
15–16. Narrow drops cwd; tiny status+cmd.  
17–18. ASCII glyphs; stream prefixes.  
19. **Composition:** AnsiText/AnsiLine; ToolCard summary.  
20. **Outcomes:** Cancel/Retry/DetachProcess/Copy*/Follow/Detach …  
21. **Stories:** `terminal-output/{running,failed,compact,env,pinned,empty,narrow,ascii}`  
22–24. Follow/unread tests; no process API; sustained paint.

## Diagnostic and CodeFrame

1. **Purpose:** Structured diagnostics + source code frames (rustc/miette-class).  
2. **Anatomy:** list row · severity letter · code · message · code_frame · notes · fixes · summary empty.  
3. **Public properties:** `Diagnostic` projection, recipes List/Inline/Full, optional `CodeFrameLine` window.  
4. **State:** cursor, expand-by-id, fix cursor, scroll, regions.  
5. **Variants:** Error/Warning/Info/Hint/Note/Help (letter+glyph always).  
6–8. List dense; full expands frame; inline one line.  
9. **Keyboard:** j/k · Space expand · c copy · a apply fix · d docs · Enter activate.  
10. **Mouse:** click select · wheel.  
11–14. Scene focus; host apply/copy/open.  
15–16. Narrow list clips path; tiny inline.  
17–18. ASCII E/W/I; underlines `^`/`-`.  
19. **Composition:** CodeBlock highlights; ErrorState plain copy.  
20. **Outcomes:** CopyDetails · ApplyFixRequested · OpenDocsRequested · Activated …  
21. **Stories:** `diagnostic/{list,full,inline,code-frame,empty,narrow,ascii}`  
22–24. Spans/tabs/Unicode tests; O(visible) list paint.

## DiffView

1. **Purpose:** High-quality read-only unified/side-by-side diff (delta/GitUI-class).  
2. **Anatomy:** `root` · `title?` · `line[]` · `status_chip` · empty mark  
3. **Public properties:** `DiffLine` projection, hunks, files, `system`, focused/ascii/colorless/title  
4. **State:** mode Auto|Unified|Split, scroll, cursor, hunk cursor, search, folds, line-nos/word/ws flags, anchors, regions.  
5. **Variants:** kinds Context|Added|Removed|FileHeader|HunkHeader|Meta; word spans; syntax spans.  
6. **Sizes/density:** line numbers; split when width ≥ 56.  
7. **Visual states:** cursor · active hunk · search/fold chip · trailing-ws marker.  
8. **Interaction states:** scroll · hunk nav · mode · search · fold · h-scroll.  
9. **Keyboard:** n/p hunks; j/k page; s mode; / search; z fold; l/w/. toggles; Enter activate.  
10. **Mouse:** wheel; click line; Ctrl+click activate.  
11. **Focus:** scene owns surface; DiffView owns scroll/cursor/hunk.  
12. **Disabled:** `accepts_input = false`.  
13. **Loading:** host projects window (virtualize large patches).  
14. **Error:** N/A.  
15. **Narrow:** force unified.  
16. **Tiny:** drop numbers; body + prefix.  
17. **Unicode/ASCII:** gutter `›`/`>`; divider `│`/`|`; empty `∅`/`[ ]`.  
18. **Colorless:** strong add/remove; bold word inserts/deletes.  
19. **Composition:** DiffReview veneer; CodeBlock for single snippets.  
20. **Outcomes:** full `DiffViewOutcome` set.  
21. **Stories:** `diff/{basic,split,word,search,narrow,unicode}`  
22. **Snapshots:** unified, split, word, search, narrow.  
23. **Interaction tests:** mode, hunk, fold, search, mouse, anchors, sustained paint.  
24. **Perf:** O(visible); `diff_bench`; host window.

## DiffReview

1. **Purpose:** Interactive patch review on DiffView (Git / plan / agent).  
2. **Anatomy:** `file_tree?` · `diff_body` · `confirm?` · `comment_draft?` · `summary`  
3. **Public properties:** lines, hunks, `DiffReviewFileRow[]`, title, tree/summary flags.  
4. **State:** embeds `DiffViewState`; region focus; selection; decisions; comments; undo; confirm.  
5. **Variants:** decisions Pending/Approved/Rejected/Staged/Unstaged/Applied/Skipped.  
6. **Sizes:** tree when width ≥ 48; summary when height ≥ 4.  
7. **Visual states:** decision glyphs · selection marks · comment markers · confirm banner.  
8. **Interaction:** select · decide · comment · editor · undo · region tab.  
9. **Keyboard:** a/r/t/T/x · Space select · c comment · e editor · u undo · v mode · Tab regions.  
10. **Mouse:** tree row · summary · DiffView body hits.  
11. **Focus:** scene surface; review owns region + DiffView scroll/cursor.  
12. **Disabled:** `accepts_input = false`.  
13. **Loading:** host projects windows.  
14. **Error:** N/A (policy on host).  
15. **Narrow:** hide tree; unified DiffView.  
16. **Tiny:** summary + body only.  
17. **Unicode/ASCII:** ✓/A · ✗/R · ●/S · comment @/💬.  
18. **Colorless:** strong marks + safe text banners.  
19. **Composition:** DiffView; PlanReview for step lists.  
20. **Outcomes:** request set (stage/apply/approve/reject/editor/comment/confirm/undo).  
21. **Stories:** `diff-review/{hunks,decisions,comments,confirm,empty,narrow,ascii}`  
22. **Snapshots:** tree, decisions, confirm, draft.  
23. **Tests:** approve/undo, multi-reject confirm, comment, selection vs mode, tree, editor, paint.  
24. **Perf:** O(visible); host virtualizes files/hunks; `diff_review_bench`.

## Sparkline

1. **Purpose:** Compact 1-row series viz.  
2. **Anatomy:** `root` · `points` · optional `label`  
3. **Public properties:** series `&[f64]`, `design` (viz tokens)  
4–14. Non-interactive controlled data; no focus.  
15. **Narrow:** drop label; fewer columns sample.  
16. **Tiny:** min 3 cells density glyphs.  
17. **Unicode/ASCII:** block levels vs `#*.-`.  
18. **Colorless:** glyph height only.  
19. **Composition:** StatusBar, TokenMeter cousin, dashboards.  
20. **Outcomes:** none.  
21. **Stories:** existing sparkline.  
22. **Snapshots:** fixed series.  
23. **Interaction tests:** N/A.  
24. **Perf:** O(width).

## BarSeries

1. **Purpose:** Multi-bar chart in cells.  
2. **Anatomy:** `root` · `bar[]` · `axis` · `label[]`  
3. **Public properties:** bars, labels, `design`  
4–14. Non-interactive.  
15. **Narrow:** drop axis labels then collapse bars.  
16. **Tiny:** single bar summary.  
17–18. viz fill glyphs; mono density.  
19. **Composition:** OpsDashboard.  
20–24. stories/snapshots existing; O(bars×height).

## SegmentedMeter

1. **Purpose:** Segmented usage/quota meter.  
2. **Anatomy:** `track` · `segment[]` · `label`  
3. **Public properties:** segments (value+tone), `design`  
4–14. Non-interactive.  
15. **Narrow:** drop labels.  
16. **Tiny:** track only.  
17–18. fill glyphs; mono hatch per segment.  
19. **Composition:** TokenMeter, StatusBar.  
20–24. existing stories; O(width).

---

# 10. Developer tools

## ThemePicker

1. **Purpose:** Select DesignSystem/theme preset with preview.  
2. **Anatomy:** `root` · `list` · `swatch` · `name` · `preview`  
3. **Public properties:** presets, selected, `design`  
4. **State:** selection index (List-like).  
5. **Variants:** `list` · `grid`  
6. **Sizes/density:** preview pane min width; density compact in studio.  
7. **Visual states:** selected row; swatch samples.  
8. **Interaction states:** navigate · select · preview.  
9. **Keyboard:** List keys; Enter select.  
10. **Mouse:** click row/swatch.  
11. **Focus:** list focus.  
12. **Disabled:** skip disabled presets.  
13. **Loading:** N/A.  
14. **Error:** N/A.  
15. **Narrow:** list only; hide preview.  
16. **Tiny:** name list only.  
17. **Unicode/ASCII:** swatch blocks; names plain.  
18. **Colorless:** name only (no swatch reliance).  
19. **Composition:** lookbook / settings shell.  
20. **Outcomes:** `Selected(PresetId)` · `Preview(PresetId)`  
21. **Stories:** existing ThemePicker + capability host.  
22. **Snapshots:** list + swatch mono.  
23. **Interaction tests:** select emits preset id.  
24. **Perf:** O(visible presets) + preview paint.

## DesignInspector (studio)

1. **Purpose:** Debug focus, scene layers, tokens, capabilities (not production default).  
2. **Anatomy:** `tabs` · `layers` · `focus_id` · `capability` · `recipe` panels  
3. **Public properties:** scene snapshot, tokens, `design`  
4. **State:** selected panel tab.  
5. **Variants:** `docked` · `overlay`  
6. **Sizes/density:** compact density forced.  
7. **Visual states:** active tab; selected layer.  
8. **Interaction states:** tab switch · select layer.  
9. **Keyboard:** Tabs keys; list nav in panels.  
10. **Mouse:** click tabs/rows.  
11. **Focus:** inspector root; internal stops.  
12. **Disabled:** N/A.  
13–14. N/A.  
15. **Narrow:** single panel stack.  
16. **Tiny:** focus_id only.  
17. **Unicode/ASCII:** tree/list glyphs.  
18. **Colorless:** selected reverse.  
19. **Composition:** TermRock Studio / lookbook only.  
20. **Outcomes:** optional `CopyToken` request.  
21. **Stories:** `studio/inspector`  
22. **Snapshots:** layers panel.  
23. **Interaction tests:** focus id updates with scene.  
24. **Perf:** O(layers).

---

# 11. AI-agent components

## FileMention

1. **Purpose:** Inline file/path/symbol mention tokens with completion + disambiguation.  
2. **Anatomy:** type glyph · label · validity mark · optional remove; disambiguation list.  
3. **Public properties:** `FileMention` / `MentionRef` (label, canonical, validity, disambiguators).  
4. **State:** `FileMentionState` (query open) · `InlineMentionState` · `MentionDraft` cursor.  
5. **Variants:** Valid / Stale / Missing / Ambiguous; file · directory · symbol.  
6–10. Atomic ←/→ · Backspace removes token; `@`/`#` detect; CompletionMenu commit insert.  
11–14. Host ranks paths; no fs/LSP in TermRock; semantic redaction when sensitive.  
15–18. ASCII type letters; markup `@[kind:id|label]`.  
19. **Composition:** CompletionMenu, PromptComposer triggers, Tag chrome.  
20. **Outcomes:** Activated · Removed · Copy · Preview · Disambiguate · DisambiguationSelected.  
21. **Stories:** `file-mention/{basic,missing,ambiguous}`, `mention-draft/atomic`.  
22–24. Atomic cursor tests; filter bench; no lookup I/O.

## EntityMention

1. **Purpose:** Inline agent/tool/session/resource/user mention tokens.  
2. **Anatomy:** same as FileMention with entity glyphs.  
3. **Public properties:** `EntityMention` wrapping `MentionRef`.  
4. **State:** `EntityMentionState` + shared draft/inline state.  
5. **Variants:** Agent · Tool · Session · Resource · User; validity marks.  
6–10. Same keyboard/completion model; family filter Entity.  
11–14. Host owns registry of agents/tools; outcomes only.  
15–18. ASCII A/T/H/R/U letters.  
19. **Composition:** CompletionMenu; permission/session hosts.  
20. **Outcomes:** shared `InlineMentionOutcome`.  
21. **Stories:** `entity-mention/{agent-tool,stale}`.  
22–24. Stale paint; family filter tests.

## AttachmentChip

1. **Purpose:** Compact file/image/URL/code attachment token for composers and egress flows.  
2. **Anatomy:** type glyph · name · meta/size/lines · progress · remove.  
3. **Public properties:** `AttachmentItem` (kind, name, bytes, lines, status, validation, sensitive).  
4. **State:** `AttachmentChipState` (Tag Body/Remove focus).  
5. **Variants:** Ready / Pending / Uploading / Indexing / Error / Invalid.  
6–10. Enter activate or remove; Ctrl+O open; Ctrl+P preview; Ctrl+R retry; Delete remove.  
11–14. Host owns path/upload; outcomes only; sensitive semantic summaries redact paths.  
15–18. ASCII type letters F/I/U/C; TokenStrip wrap/scroll/+N.  
19. **Composition:** PromptComposer chips; Tag chrome; permission strips.  
20. **Outcomes:** Activated · Removed · Open · Preview · Retry · PartChanged.  
21. **Stories:** `attachment-chip/{file,broken-path,upload}`, `attachment-strip/wrap`.  
22–24. Redaction tests; strip paint bench; no network I/O.

## PasteChip

1. **Purpose:** Collapsed large-paste token; body out of editor path until insert/submit.  
2. **Anatomy:** PASTE badge · preview · bytes/lines · remove; optional expanded lines.  
3. **Public properties:** `PastePayload` (preview, bytes, lines, body?, binary, status, sensitive).  
4. **State:** `PasteChipState` (expanded + Tag focus).  
5. **Variants:** text paste · binary · error/progress.  
6–10. Enter expand; Esc collapse; Ctrl+C copy-by-id; Ctrl+I insert (confirm binary); Delete remove.  
11–14. Semantic summary never includes body; CopyRequested carries id only.  
15–18. Threshold aligned with PromptComposer; ASCII `P` badge.  
19. **Composition:** PromptComposer paste chips; TokenStrip; permission egress.  
20. **Outcomes:** Expanded · Collapsed · Removed · Copy · Insert · Preview · Retry.  
21. **Stories:** `paste-chip/{large,binary,expanded}`.  
22–24. Body-not-in-summary; binary confirm; expand/esc tests.

## PromptComposer

1. **Purpose:** Flagship terminal AI agent input surface.  
2. **Anatomy:** chips · TextArea editor · status (mode/model/busy/queue/ctx) · validation.  
3. **Public properties:** chips, presentation, density, ascii/colorless, placeholder, policy.  
4. **State:** five buckets — editor/undo/history/selection · chips · CompletionQuery · presentation/indicators · policy/busy/queue/connection + `accepts_input`.  
5. **Variants:** compact / normal / expanded / fullscreen; paste/file/mention chips.  
6–10. Enter submit/queue; Alt|Ctrl|Shift+Enter newline; Ctrl Z/Y/A/C/U/E; Ctrl+Shift O/F; Up/Down history; Esc peel; mouse editor/chips/mode/model.  
11–14. Host `accepts_input` gate; draft never cleared by overlay takeover; busy queue; ValidationFailed.  
15–18. `contract_for_width`; grapheme editor; ASCII marks; colorless reverse selection.  
19. **Composition:** AgentWorkbench south; CompletionMenu + OverlayStack; HistoryPicker + KeyboardHelp bridges; TokenMeter.  
20. **Outcomes:** Submit · Queued · Interrupt · Cancel · Completion* · ExternalEditor · chips · fullscreen · SelectionCopied.  
21. **Stories:** `prompt-composer/{basic,busy-queue,compact,paste-chip,disconnected,fullscreen,narrow,unicode}`  
22–24. Draft gate tests; large-prompt + repeated-paste + streaming completion bench; no provider I/O.

## PermissionPrompt

1. **Purpose:** Risk-aware permission card; **no side effects**.  
2. **Anatomy:** `frame` · `risk_glyph` · `provenance` · `detail` · `scope` · `decision_row`  
3. **Public properties:** system, ascii, colorless, focused  
4. **State:** queue + **action_cursor** (default Deny) + scope + accepts_input.  
5. **Variants:** card via OverlayStack (Alert trap High/Critical).  
6. **Sizes/density:** Compact density; dialog size clamp.  
7. **Visual states:** risk panel; empty ∅; action cursor marks.  
8. **Interaction states:** move action · scope · edit · confirm · cancel.  
9. **Keyboard:** intent map + product `n`/`e`/`p`/`[]`; **no y grant**.  
10. **Mouse:** hit regions; click selects/confirms per law.  
11. **Focus:** surface accepts_input; action_cursor local; Esc one layer.  
12. **Disabled:** accepts_input false.  
13. **Loading:** N/A (sync decide).  
14. **Error:** risk is presentation, not validation.  
15. **Narrow:** vertical action stack.  
16. **Tiny:** risk title + actions.  
17. **Unicode/ASCII:** risk glyphs + ›/`>`.  
18. **Colorless:** strong cursor; risk glyphs remain.  
19. **Composition:** AgentWorkbench overlay; PromptComposer gate.  
20. **Outcomes:** Decided / Cancelled / ActionCursorMoved / ScopeChanged / StaleIgnored / …  
21. **Stories:** `permission-prompt/{basic,low-read,destructive-nested,egress,narrow,unicode}`  
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
17. **Unicode/ASCII:** radio/check glyphs; progress bar blocks catalog.  
18. **Colorless:** selected option reverse; progress via fill density.  
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
12. **Disabled:** accept/reject disabled while consumer marks pending.  
13. **Loading:** step detail Skeleton when plan streams.  
14. **Error:** rejected step tone; invalid plan Callout.  
15. **Narrow:** stack detail under list.  
16. **Tiny:** selected step title + Accept/Reject.  
17. **Unicode/ASCII:** step status glyphs; markdown body rules.  
18. **Colorless:** accepted check prefix; selected reverse.  
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
17. **Unicode/ASCII:** task status glyphs catalog.  
18. **Colorless:** status letter + selected reverse.  
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
12. **Disabled:** skip disabled sessions.  
13. **Loading:** Skeleton list while sessions fetch.  
14. **Error:** EmptyState error-lite or ErrorView.  
15. **Narrow:** drop preview.  
16. **Tiny:** title only rows.  
17. **Unicode/ASCII:** timestamps plain; list gutters.  
18. **Colorless:** selected reverse; empty bold title.  
19. **Composition:** startup overlay.  
20. **Outcomes:** `Picked(Id)` · `Cancelled` · `QueryChanged`  
21. **Stories:** `session-picker/basic`, `session-picker/empty`  
22. **Snapshots:** list rows.  
23. **Interaction tests:** pick/cancel.  
24. **Perf:** O(visible).

## ThinkingBlock

1. **Purpose:** Collapsible agent reasoning stream.  
2. **Anatomy:** `root` · `header` · `status` · `body` · `expand`  
3. **Public properties:** text/lines, expanded, streaming, `design`  
4. **State:** expanded controlled or uncontrolled.  
5. **Variants:** `inline` · `card`  
6. **Sizes/density:** collapsed 1 row; expanded variable.  
7. **Visual states:** streaming, collapsed, expanded.  
8. **Interaction states:** toggle expand.  
9. **Keyboard:** Enter toggle when focused.  
10. **Mouse:** click header.  
11. **Focus:** optional tab stop in Transcript.  
12. **Disabled:** N/A.  
13. **Loading/async:** streaming append; no full rebuffer.  
14. **Error:** N/A.  
15. **Narrow:** collapsed default; drop header chrome.  
16. **Tiny:** status glyph only until expand forced full.  
17. **Unicode/ASCII:** disclosure glyphs.  
18. **Colorless:** dim body; bold header when expanded.  
19. **Composition:** Transcript items.  
20. **Outcomes:** `ToggledExpand`  
21. **Stories:** `thinking-block/stream`, `thinking-block/collapsed`  
22. **Snapshots:** expanded body dim.  
23. **Interaction tests:** toggle.  
24. **Perf:** O(visible body lines).

## TokenMeter

1. **Purpose:** Context/token usage meter.  
2. **Anatomy:** SegmentedMeter parts + `label` · `ratio`  
3. **Public properties:** used, limit, segments?, `design`  
4–12. Non-interactive by default.  
13. **Loading:** optional indeterminate.  
14. **Error:** over-limit danger tone.  
15. **Narrow:** drop labels keep track.  
16. **Tiny:** track only.  
17–18. SegmentedMeter glyphs; mono hatch.  
19. **Composition:** StatusBar, workbench chrome.  
20. **Outcomes:** none (or `Activated` if clickable).  
21. **Stories:** existing token-meter.  
22. **Snapshots:** near-limit / over.  
23. **Interaction tests:** N/A default.  
24. **Perf:** O(width).

## Transcript

1. **Purpose:** Variable-height agent conversation surface (StreamView evolution).  
2. **Anatomy:** `root` · `item[]` · `sticky_user?` · `scrollbar` · `follow`  
3. **Public properties:** items projection (Markdown, ToolCallCard, Thinking, Permission inline, …), `design`  
4. **State:** offset, follow, focused item; virtualized window.  
5. **Variants:** `chat` · `log`  
6. **Sizes/density:** item pad by density.  
7. **Visual states:** streaming tail; follow chip.  
8. **Interaction states:** scroll · activate item · expand child.  
9. **Keyboard:** page/arrows; item intents when focused.  
10. **Mouse:** wheel (breaks follow); click items.  
11. **Focus:** transcript surface + optional item focus.  
12. **Disabled:** N/A.  
13. **Loading/async:** append/stream without full scan; skeleton pending.  
14. **Error:** ErrorView item or banner.  
15. **Narrow:** drop secondary item parts via child recipes.  
16. **Tiny:** last item + composer sibling.  
17. **Unicode/ASCII:** child rules.  
18. **Colorless:** child rules + follow chip bold.  
19. **Composition:** AgentWorkbench center; children pure.  
20. **Outcomes:** `Scrolled` · `ItemActivated` · child unions · `FollowToggled`  
21. **Stories:** `transcript/stream`, `transcript/mixed-items`, `transcript/follow`  
22. **Snapshots:** mixed ToolCall + Markdown.  
23. **Interaction tests:** follow breaks on wheel; virtualize bounds.  
24. **Perf:** O(visible items × item paint); never O(total history) in render.

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
17. **Unicode/ASCII:** child glyph catalogs.  
18. **Colorless:** child non-color cues; pane focus underline/border role.  
19. **Composition:** only block; **no domain I/O**.  
20. **Outcomes:** union of children.  
21. **Stories:** `blocks/agent-workbench`, `blocks/agent-workbench-narrow`  
22. **Snapshots:** default layout.  
23. **Interaction tests:** Esc peel approval → prompt focus.  
24. **Perf:** sum of children; no O(n²) layout thrash.

## OpsDashboard

1. **Purpose:** Ops overview block: metrics + logs + status.  
2. **Anatomy:** `root` · `charts` · `log` · `status`  
3. **Public properties:** chart series, log lines, status slots, `design`  
4. **State:** focus region; log follow.  
5. **Variants:** `default`  
6. **Sizes/density:** dashboard density default.  
7–14. Child visual/interaction/loading.  
15. **Narrow:** drop charts first; keep LogStream + StatusBar.  
16. **Tiny:** StatusBar + last log lines.  
17–18. child rules.  
19. **Composition:** Workspace geometry.  
20. **Outcomes:** child unions.  
21. **Stories:** `blocks/ops-dashboard`, `blocks/ops-dashboard-narrow`  
22–24. snapshots + O(visible).

## ResourceBrowser

1. **Purpose:** Hierarchical resource explorer.  
2. **Anatomy:** `breadcrumbs` · `tree` · `inspector` · `preview?`  
3. **Public properties:** nodes, fields, preview, `design`  
4. **State:** tree selection + inspector path.  
5. **Variants:** `default`  
6. **Sizes/density:** tree rail width tokens.  
7–14. Tree + ObjectInspector contracts.  
15. **Narrow:** tree rail; hide preview.  
16. **Tiny:** tree only.  
17–18. child rules.  
19. **Composition:** Workspace.  
20. **Outcomes:** Tree ∪ ObjectInspector.  
21. **Stories:** `blocks/resource-browser`  
22–24. selection sync tests; O(visible).

## SettingsShell

1. **Purpose:** Settings navigation + form body.  
2. **Anatomy:** `sidebar` · `section` · `form`  
3. **Public properties:** nav items, form sections, `design`  
4. **State:** selected nav; form focus.  
5–14. Sidebar + Form.  
15. **Narrow:** sidebar rail.  
16. **Tiny:** form only with Breadcrumbs back.  
17–24. child rules; stories `blocks/settings-shell`.

## FormWizard

1. **Purpose:** Multi-step form flow.  
2. **Anatomy:** `stepper` · `form` · `nav` (back/next/finish)  
3. **Public properties:** steps, current, form fields, `design`  
4. **State:** step index; form state.  
5. **Variants:** `linear`  
6. **Sizes/density:** stepper 1 row.  
7. **Visual states:** step complete/current/upcoming.  
8. **Interaction states:** next/back/finish.  
9. **Keyboard:** Form keys; next/back bindings.  
10. **Mouse:** stepper + nav buttons.  
11. **Focus:** form then nav.  
12. **Disabled:** next until valid.  
13. **Loading:** step transition skeleton.  
14. **Error:** field validation blocks next.  
15. **Narrow:** stepper titles truncate.  
16. **Tiny:** step title + form only.  
17–18. Form + Button.  
19. **Composition:** Dialog or full workspace.  
20. **Outcomes:** `StepChanged` · `Finished` · Form outcomes.  
21. **Stories:** `blocks/form-wizard`  
22. **Snapshots:** mid-step.  
23. **Interaction tests:** required blocks next.  
24. **Perf:** O(visible fields).

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

---

## H. Required component coverage matrix

Every row must have axes **1–24** specified in this document (interactive axes may be N/A with reason).

| Taxonomy | Components |
|----------|------------|
| Primitives | Button, IconButton, Badge, Tag, Chip, Kbd, Separator, Spinner |
| Content | Heading, Paragraph, Markdown, CodeBlock, Surface, Section, Callout, Alert |
| Layout | Surface, Section, ScrollArea, WorkspacePane/Split |
| Navigation | Tabs, Sidebar, Breadcrumbs, Menu, ContextMenu, ActionBar, HintBar, StatusBar |
| Forms | TextInput, TextArea, Checkbox, RadioGroup, Switch, Form |
| Selection | Select, MultiSelect, Combobox, List, Tree, CompletionMenu |
| Feedback | Toast, Progress, Skeleton, EmptyState, LoadingView, ErrorView, Spinner |
| Overlays | Dialog, Drawer, Popover, Tooltip, CommandPalette, Backdrop, JumpOverlay |
| Data | Table, DataTable, ObjectInspector, LogStream, Timeline, DiffReview, Sparkline, BarSeries, SegmentedMeter |
| Dev tools | ThemePicker, DesignInspector |
| AI-agent | PromptComposer, PermissionPrompt, QuestionFlow, PlanReview, ToolCallCard, TaskRail, SessionPicker, ThinkingBlock, TokenMeter, Transcript |
| Blocks | AgentWorkbench, OpsDashboard, ResourceBrowser, SettingsShell, FormWizard |

### Axis legend (1–24)

1 Purpose · 2 Anatomy · 3 Public properties · 4 Controlled/uncontrolled state · 5 Variants · 6 Sizes/density · 7 Visual states · 8 Interaction states · 9 Keyboard · 10 Mouse · 11 Focus · 12 Disabled · 13 Loading/async · 14 Error/validation · 15 Narrow · 16 Tiny · 17 Unicode/ASCII · 18 Colorless · 19 Composition · 20 Outcomes · 21 Stories · 22 Snapshots · 23 Interaction tests · 24 Performance

### Cross-links

- Tokens/recipes: [`terminal-design-system.md`](./terminal-design-system.md)  
- Quality gate: [`component-quality-standard.md`](./component-quality-standard.md)  
- Agent implement prompts: [`component-prompt-library.md`](./component-prompt-library.md)  
- Agent pack: [`termrock-agent.md`](./termrock-agent.md)  
- Contracts JSON: `docs/api/component-contracts.json` (machine inventory)
