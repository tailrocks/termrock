# TermRock component anatomy, behavior, variants, and state specification

**Status:** binding design target for component work (post–kernel plans 039–044)  
**Inspected inventory:** 46 public widgets (lookbook + COMPONENTS.md) on `feat/experience-layer-shadcn-tui`  
**Design system dependency:** [`terminal-design-system.md`](./terminal-design-system.md) (`DesignSystem`, recipes, capability)  
**Policy:** Rendering alone is incomplete. Interaction, contraction, focus, and outcomes are part of “done.” Breaking renames/API reshapes are allowed.

---

## A. Current inventory → taxonomy mapping

| Current TermRock | Taxonomy home | Evolution |
|------------------|---------------|-----------|
| Panel, Backdrop | Layout / Primitives | → Surface, Section |
| Separator (implicit `─` in lists) | Primitives | → Separator |
| Heading/Paragraph (none) | Content | **New** |
| MarkdownView, CodeBlock, DiffView | Content | Rename Markdown; Diff → DiffReview |
| Spinner (via Progress indeterminate) | Feedback | **Promote** Spinner |
| Progress, Skeleton, EmptyState, ErrorView, Banner, Toast, LoadingView | Feedback | Banner → Callout/Alert split |
| TextInput, TextArea, Form | Forms | + Checkbox, Radio, Switch, Select… |
| List, Tree, Table, VirtualGrid, DetailTable, Picker, CompletionMenu | Selection / Data | Picker → Combobox; MultiSelect modes |
| Tabs, ActionBar, HintBar, StatusBar, Sidebar (none) | Navigation | + Breadcrumbs, Sidebar |
| Dialog, MessageDialog, ChoiceDialog, CommandPalette, JumpOverlay, Toast | Overlays | + Menu, ContextMenu, Popover, Tooltip, Drawer |
| SplitPane, WorkSurface, Viewport | Layout | → workspace tree (plan 042) |
| StreamView, ToolCard, ApprovalCard, PromptBox, Timeline, ThinkingBlock, TokenMeter | AI-agent | + QuestionFlow, PlanReview, TaskRail, SessionPicker, LogStream |
| Sparkline, BarSeries, SegmentedMeter | Data viz | under Data presentation |
| ThemePicker, ImageSurface | Dev tools / content | ThemePicker = Developer tools |
| agent_shell / ops / resource patterns | Application blocks | Elevate to full blocks |

**Completeness rule:** A component is **not** complete when it paints. It is complete when all rows in its contract table below are specified, tested, and story-covered for claimed axes.

---

## B. Shared cross-cutting contracts

Apply to every interactive component unless marked N/A.

| Concern | Default TermRock rule |
|---------|----------------------|
| **State ownership** | Widget state = interaction only (focus, selection, scroll, hover, open). Domain values, validation messages, async results = consumer-owned projections. |
| **IDs** | Stable IDs for focus/selection/hit where identity survives reorder. |
| **Borrowed data** | Render from `&[…]` / borrowed lines; no hidden clone of datasets. |
| **Focus** | Single-line border + `DesignSystem` focus tokens; never border weight. |
| **Esc** | Routed via InteractionScene layer policy (plan 040); local “cancel” only when scene owns layer. |
| **Intents** | Prefer `UiIntent` / scene actions; raw keys only in keymap adapters. |
| **Density** | Comfortable / Compact / Dashboard from `DesignSystem`. |
| **Narrow** | Drop parts by recipe priority before grapheme-unsafe truncate. |
| **Tiny** (≤20×5) | Collapse to primary label + focus cue; overflow ellipsis. |
| **Unicode** | Grapheme-safe clip; wide chars never split. |
| **ASCII** | Glyph catalog fallback. |
| **Colorless** | Modifiers + glyphs carry state. |
| **Outcomes** | Typed enums; **no** side effects (no I/O, no process). |
| **Perf** | Paint O(visible); no full-data scan in render hot path for virtualized surfaces. |

### Shared visual states
`default` · `hover` · `focus-visible` · `active/pressed` · `selected` · `disabled` · `loading` · `invalid` · `read-only`

### Shared keyboard primitives (mapped via keymap)
Move prev/next · Page · Home/End · Activate · Toggle · Cancel · Submit · Open/Close · Expand/Collapse

---

## C. Taxonomy

1. **Primitives** — Button, IconButton, Badge, Tag, Chip, Kbd, Separator, Spinner  
2. **Content** — Heading, Paragraph, Markdown, CodeBlock, Callout, Alert, Surface, Section  
3. **Layout** — Surface (shared), Section, Split/Workspace (from plan 042), ScrollArea/Viewport  
4. **Navigation** — Tabs, Sidebar, Breadcrumbs, Menu (bar), ActionBar, HintBar, StatusBar  
5. **Forms & input** — TextInput, TextArea, Checkbox, RadioGroup, Switch, Form  
6. **Selection** — Select, MultiSelect, Combobox, List, Tree, CompletionMenu  
7. **Feedback** — Toast, Progress, Skeleton, EmptyState, Spinner, LoadingView  
8. **Overlays** — Dialog, Drawer, Popover, Tooltip, ContextMenu, CommandPalette, JumpOverlay, Backdrop  
9. **Data presentation** — Table, DataTable, ObjectInspector, LogStream, Timeline, DiffReview, charts  
10. **Developer tools** — ThemePicker, DesignSystem inspector (studio), public-api stories  
11. **AI-agent** — PromptComposer, PermissionPrompt, QuestionFlow, PlanReview, ToolCallCard, TaskRail, SessionPicker, ThinkingBlock, TokenMeter, Transcript (plan 041)  
12. **Application blocks** — AgentWorkbench, OpsDashboard, ResourceBrowser, SettingsShell, FormWizard  

---

## D. Spec template (used below)

For each component, sections **1–24** follow the user’s list. Where behavior is N/A, stated explicitly.

---

# Primitives

## Button

1. **Purpose:** Primary interactive control to invoke a single domain-neutral action (consumer maps to effects).  
2. **Anatomy:** `root` · `leading_icon` · `label` · `trailing_icon` · `kbd_hint`  
3. **Public properties:** `id`, `label`, `leading`, `trailing`, `variant`, `size`, `enabled`, `loading`, `danger`, `design: &DesignSystem`  
4. **State:** Controlled: none required. Uncontrolled widget state: `pressed` (frame-local), `hovered`. Activation is outcome, not stored “clicked.”  
5. **Variants:** `primary` · `secondary` · `ghost` · `danger` · `link`  
6. **Sizes/density:** `sm`/`md` height 1 row; density changes pad_x (Comfortable 2, Compact 1, Dashboard 1).  
7. **Visual states:** default, hover, focus-visible, pressed, disabled, loading.  
8. **Interaction states:** idle, armed (space down), activated.  
9. **Keyboard:** Enter/Space activate; no arrows (not a group).  
10. **Mouse:** click/press on root hit region.  
11. **Focus:** Tab stop when enabled; focus-visible ring via recipe.  
12. **Disabled:** no activation; dim recipe; excluded from focus if `skip_disabled`.  
13. **Loading:** spinner replaces leading; no double-activate until consumer clears loading.  
14. **Error/validation:** N/A (use Callout/Form).  
15. **Narrow:** drop `kbd_hint` then `trailing` then `leading`; never drop label before those.  
16. **Tiny:** label only, min 3 cols ellipsis.  
17. **Unicode/ASCII:** label grapheme-safe; icons from glyph catalog.  
18. **Colorless:** bold/underline for primary; `[!]` prefix for danger.  
19. **Composition:** usable inside ActionBar, Dialog footer, Menu items (as shared paint).  
20. **Outcomes:** `Activated(Id)` · `Ignored`.  
21. **Stories:** `button/primary`, `button/danger-loading`, `button/narrow`, `button/mono`.  
22. **Snapshots:** each variant×state at md width.  
23. **Interaction tests:** Enter/Space activate once; disabled ignores; loading ignores.  
24. **Perf:** O(1) paint.

## IconButton

1. Icon-only action (tooltip/label for a11y).  
2. `root` · `icon` · `badge_dot` (optional)  
3. `id`, `icon`, `aria_label` (required string for hints), `variant`, `enabled`, `loading`  
4. hover/pressed only; no value state.  
5. `ghost` · `solid` · `danger`  
6. hit target min 3×1 cells even if glyph 1 cell (pad).  
7–8. same as Button.  
9. Enter/Space.  
10. click.  
11. focus-visible; screen readers via label in HintBar when focused.  
12–14. as Button.  
15. keep icon; badge_dot drops first.  
16. single glyph.  
17–18. glyph ASCII fallback.  
19. toolbars, tabs trailing.  
20. `Activated(Id)`.  
21. `icon-button/basic`, `icon-button/loading`.  
22–23. activate + disabled.  
24. O(1).

## Badge

1. Non-interactive count/status mark.  
2. `root` · `label`  
3. `text`, `tone` (neutral/info/success/warn/danger), `max_chars`  
4. none.  
5. `soft` · `solid` · `outline`  
6. always 1 row; density pad 0–1.  
7. default only (no hover).  
8. N/A interactive.  
9–12. N/A (not focusable).  
13–14. N/A.  
15. truncate with `9+` style if numeric.  
16. single char `•` if &lt;2 cols.  
17–18. tone → mono prefix `i`/`!`/`x`.  
19. inside List/Tab/Button trailing.  
20. none.  
21. `badge/tones`.  
22. snapshot tones.  
23. N/A.  
24. O(1).

## Tag

1. Removable or static categorical label.  
2. `root` · `label` · `remove`  
3. `id`, `label`, `removable`, `enabled`  
4. Controlled: presence in parent list. Uncontrolled: N/A.  
5. `default` · `accent`  
6. 1 row.  
7. hover on remove, disabled.  
8. remove armed.  
9. if focused: Backspace/Delete remove; Enter no-op unless activatable.  
10. click remove hit region.  
11. optional tab stop when removable.  
12. no remove.  
13. N/A.  
14. N/A.  
15. drop remove before truncating label.  
16. first grapheme only.  
17–18. `x` ascii remove.  
19. MultiSelect chips, filters.  
20. `Remove(Id)`.  
21. `tag/removable`.  
22–23. remove key/click.  
24. O(1).

## Chip

1. Toggleable filter/choice in a group (compact).  
2. `root` · `label` · `leading`  
3. `id`, `label`, `selected`, `enabled`  
4. Controlled `selected`; or group-controlled.  
5. `filter` · `choice`  
6. density pad.  
7. selected/unselected/hover/disabled.  
8. toggled.  
9. Enter/Space toggle.  
10. click toggle.  
11. tab stop.  
12. no toggle.  
13. optional loading spinner.  
14. N/A.  
15. truncate label.  
16. selected `*` prefix mono.  
17–18. ascii `[*]`/`[ ]`.  
19. filter bars.  
20. `Toggled { id, selected }`.  
21. `chip/filter-group`.  
22–23. toggle outcomes.  
24. O(1).

## Kbd

1. Display a key chord for hints/docs.  
2. `root` · `keys[]` · `separator`  
3. `chord: &str` or structured `KeyChord`, `size`  
4. none.  
5. `plain` · `raised` (sunken bg)  
6. 1 row.  
7. default only.  
8–14. N/A.  
15. collapse to last key.  
16. single char.  
17. display “Ctrl” vs “^” ascii mode.  
18. reverse video.  
19. HintBar, Button trailing, docs.  
20. none.  
21. `kbd/chords`.  
22. snapshot.  
23. N/A.  
24. O(keys).

## Separator

1. Visual section break.  
2. `root` (line) · optional `label`  
3. `orientation` (h/v), `label`, `variant`  
4. none.  
5. `line` · `dashed` (ascii `- `) · `thick` (still single-line glyph `═`/ `=`, not double border focus)  
6. full width/height of parent.  
7. default.  
8–14. N/A.  
15. label centered with line fill.  
16. single `-` or `|`.  
17–18. ascii fallbacks.  
19. between sections.  
20. none.  
21. `separator/labeled`.  
22. snapshot.  
23. N/A.  
24. O(width).

## Spinner

1. Indeterminate progress glyph.  
2. `root` · `frame` · `label`  
3. `tick: u64` or `FrameTick`, `label`, `motion` from system  
4. frame derived from tick (uncontrolled animation input from app clock).  
5. `inline` · `block`  
6. 1 row.  
7. animating / static (motion off).  
8. N/A.  
9–12. N/A unless labeled button.  
13. **is** loading affordance.  
14. N/A.  
15. drop label.  
16. one braille/ascii frame.  
17. unicode frames vs `|/-\`.  
18. same.  
19. Button loading, EmptyState, ToolCallCard.  
20. none.  
21. `spinner/frames`, `spinner/motion-off`.  
22. fixed tick snapshots.  
23. motion-off stable frame.  
24. O(1).

---

# Content

## Heading

1. Title hierarchy without font sizes.  
2. `root` · `text` · optional `eyebrow`  
3. `level: 1..=4`, `text`, `truncation`  
4. none.  
5. levels map to type scale (bold + fg_strong).  
6. density only affects margin recipe if in Section.  
7. default.  
8–14. N/A.  
15. truncate end.  
16. level1 → bold single line.  
17–18. bold/dim only.  
19. Section header, Dialog title.  
20. none.  
21. `heading/levels`.  
22. snapshots.  
23. N/A.  
24. O(len).

## Paragraph

1. Body prose wrapping.  
2. `root` · `lines`  
3. `text`, `wrap`, `max_lines`, `tone` (default/muted)  
4. none (pure view).  
5. `body` · `muted`  
6. wrap width = area.  
7. default.  
8–14. N/A.  
15. max_lines + ellipsis.  
16. first line only.  
17. grapheme wrap.  
18. muted = dim.  
19. Markdown blocks, Dialog body.  
20. none.  
21. `paragraph/wrap`.  
22. wrap snapshots.  
23. N/A.  
24. O(visible lines).

## Markdown

1. Projected markdown blocks (consumer may parse; widget paints kinds).  
2. `root` · `block[]` (heading/p/list/code/quote/rule)  
3. `blocks`, `first`, `width_policy`  
4. scroll offset controlled by parent or internal `offset` if stateful.  
5. `reader` · `compact`  
6. density line gap 0–1.  
7. default.  
8. scroll interaction if stateful.  
9. optional j/k page if focusable viewport.  
10. wheel scroll.  
11. optional focus for scroll.  
12. N/A.  
13. streaming: append blocks; keep anchor (plan 041).  
14. N/A.  
15. reflow wrap.  
16. plain text dump.  
17. code still mono-safe.  
18. headings bold; lists `*` ascii.  
19. Transcript, docs.  
20. `Scrolled` if stateful.  
21. `markdown/basic`, `markdown/narrow`.  
22. block kind snapshots.  
23. scroll tests if stateful.  
24. O(visible blocks).

## CodeBlock

1. Source listing with optional gutter and syntax roles.  
2. `root` · `gutter` · `line[]` · `language_label`  
3. `lines`, `first_line`, `show_line_numbers`, `highlighter`, `language`  
4. scroll/cursor optional state.  
5. `plain` · `numbered`  
6. gutter width from digit count.  
7. default.  
8. select line optional.  
9. arrows if selectable.  
10. wheel.  
11. focus for keyboard scroll.  
12. N/A.  
13. streaming append lines.  
14. N/A.  
15. hide language label; shrink gutter.  
16. no gutter.  
17. no mid-grapheme clip.  
18. syntax → bold/dim only.  
19. Transcript, Diff side.  
20. `Scrolled` / `LineActivated`.  
21. `code-block/basic`, `code-block/unicode`.  
22. numbered snapshot.  
23. scroll bounds.  
24. O(visible lines); no highlight full file each frame without cache.

## Surface

1. Elevated/sunken/canvas container (replaces ad-hoc bg).  
2. `root` · `content_slot`  
3. `elevation`, `border`, `padding` from density  
4. none.  
5. `canvas` · `surface` · `raised` · `elevated` · `sunken`  
6. inset from DesignSystem.  
7. default.  
8–14. N/A.  
15. reduce inset.  
16. no pad.  
17–18. border ascii.  
19. wraps any children.  
20. none.  
21. `surface/elevations`.  
22. snapshot ladder.  
23. N/A.  
24. O(area).

## Section

1. Surface + heading + optional description + body.  
2. `root` · `heading` · `description` · `body` · `header_actions`  
3. `title`, `description`, `actions[]`  
4. collapsed optional controlled.  
5. `default` · `collapsible`  
6. density gaps.  
7. collapsed/expanded.  
8. toggle.  
9. if collapsible: Enter toggle when header focused.  
10. click header.  
11. header focusable if collapsible.  
12. N/A.  
13. body loading skeleton.  
14. N/A.  
15. drop description then actions.  
16. title only.  
17–18. disclosure glyphs.  
19. settings pages.  
20. `Toggled { expanded }`.  
21. `section/collapsible`.  
22–23. expand tests.  
24. O(1)+children.

## Callout

1. Inline informational banner (non-modal).  
2. `root` · `icon` · `title` · `body` · `actions`  
3. `tone`, `title`, `body`, `dismissible`  
4. visible controlled by parent.  
5. `info` · `success` · `warning` · `danger` · `neutral`  
6. 2–n lines wrap.  
7. default/hover dismiss.  
8. dismiss.  
9. if dismissible and focused: Esc dismiss only if scene allows.  
10. click dismiss.  
11. optional focus.  
12. N/A.  
13. N/A.  
14. tone=danger for errors.  
15. single line title.  
16. icon+title.  
17–18. tone glyphs `i`/`!`/`x`.  
19. above forms.  
20. `Dismissed`.  
21. `callout/tones`.  
22–23. dismiss.  
24. O(text).

## Alert

1. Stronger than Callout; may require acknowledgement.  
2. Anatomy as Callout + `primary_action`.  
3. `tone`, `title`, `body`, `action_label`  
4. open controlled.  
5. `inline` · `banner`  
6–18. similar Callout; mono stronger bold.  
19. not for permissions (use PermissionPrompt).  
20. `Acknowledged` · `Dismissed`.  
21. `alert/danger`.  
22–23. ack tests.  
24. O(text).

---

# Layout

*(Surface/Section above.)*

## ScrollArea (Viewport evolution)

1. Scrollable viewport over borrowed content height.  
2. `root` · `viewport` · `scrollbar_y` · `scrollbar_x`  
3. `content_size`, `show_bars`, `follow` policy  
4. Controlled offsets or internal `ScrollState`.  
5. `auto_bars` · `always` · `never`  
6. bar thickness 1 cell.  
7. scrolling/following.  
8. drag thumb.  
9. page/arrows when focused.  
10. wheel, click track, drag.  
11. focusable.  
12. N/A.  
13. follow tail for logs.  
14. N/A.  
15. hide bars if no overflow.  
16. no bars; clip.  
17–18. bar glyphs ascii `#`/`|`.  
19. wraps Markdown/Log/Table body.  
20. `Scrolled { x, y }`.  
21. `scroll-area/both-axes`.  
22–23. wheel/page bounds.  
24. O(1) chrome + child paint.

## Split / WorkspacePane (plan 042)

1. Resizable multi-pane tree.  
2. `root` · `pane[]` · `divider` · `tab_strip?`  
3. tree model, ratios, collapse  
4. ratio/collapse/zoom state.  
5. horizontal/vertical/tabs/stack.  
6. min pane sizes from DesignSystem.dim.  
7. dragging divider.  
8. focus leaf.  
9. remap pane focus; divider keyboard resize.  
10. drag divider.  
11. leaf focus; divider focusable.  
12. N/A.  
13. N/A.  
14. N/A.  
15. collapse by priority.  
16. single leaf only.  
17–18. divider `|`/`-`.  
19. AgentWorkbench, ResourceBrowser.  
20. `RatioChanged` · `Collapsed` · `FocusPane`.  
21. `workspace/split-resize`, `workspace/narrow-collapse`.  
22–23. resize clamp.  
24. layout O(panes); no thrash alloc.

---

# Navigation

## Tabs

1. Switch among peer views.  
2. `root` · `tab[]` · `label` · `icon` · `badge` · `underline` · `close?`  
3. `tabs`, `selected_id`, `closable`  
4. Controlled selected_id preferred.  
5. `line` · `enclosed`  
6. tab height 1–2 by density.  
7. active/inactive/hover/disabled.  
8. selecting.  
9. Left/Right, Home/End, Enter; 1–9 optional.  
10. click tab.  
11. roving tabindex within strip; one tab focus.  
12. skip disabled.  
13. badge loading.  
14. N/A.  
15. scroll/truncate labels; drop badge then icon.  
16. active only char.  
17–18. underline focus.  
19. with workspace.  
20. `Selected(Id)` · `Close(Id)`.  
21. `tabs/status`, `tabs/narrow`.  
22–23. keyboard nav.  
24. O(tabs).

## Sidebar

1. Vertical nav list for app sections.  
2. `root` · `header` · `item[]` · `footer`  
3. `items`, `selected`, `collapsed` (icon rail)  
4. controlled selected.  
5. `expanded` · `rail`  
6. width tokens 16–28 / rail 3.  
7. selected/hover.  
8. activate.  
9. up/down/enter.  
10. click.  
11. focus within.  
12. disabled items.  
13. section loading.  
14. N/A.  
15. force rail.  
16. rail only.  
17–18. icons ascii.  
19. + Breadcrumbs content.  
20. `Selected` · `ToggledCollapse`.  
21. `sidebar/rail`.  
22–23. nav tests.  
24. O(items visible).

## Breadcrumbs

1. Path hierarchy navigation.  
2. `root` · `crumb[]` · `separator` · `overflow_menu`  
3. `items`, `max_visible`  
4. none.  
5. `default`  
6. 1 row.  
7. hover/current.  
8. activate non-current.  
9. optional left/right.  
10. click crumb.  
11. each crumb focusable or single widget with internal index.  
12. N/A.  
13. N/A.  
14. N/A.  
15. collapse middle to `…`.  
16. last crumb only.  
17–18. sep `/` ascii.  
19. ResourceBrowser header.  
20. `Activated(Id)`.  
21. `breadcrumbs/overflow`.  
22–23. overflow activate.  
24. O(items).

## Menu

1. Hierarchical command menu (menubar or standalone).  
2. `root` · `item[]` · `submenu` · `shortcut` · `separator`  
3. `items` tree, `open_path`  
4. open path state.  
5. `bar` · `panel`  
6. item height 1.  
7. open/hover/disabled/checked.  
8. submenu open.  
9. arrows, enter, esc closes one level.  
10. click; hover open optional.  
11. focus moves with highlight.  
12. skip.  
13. N/A.  
14. N/A.  
15. drop shortcuts.  
16. labels only.  
17–18. `>` submenu ascii.  
19. ContextMenu shares items model.  
20. `Activated(Id)` · `Cancelled`.  
21. `menu/nested`.  
22–23. esc levels.  
24. O(visible items).

## ContextMenu

1. Pointer-triggered Menu at point.  
2. Menu anatomy + `anchor`  
3. `items`, `position`  
4. open boolean.  
5. default.  
6–18. as Menu; open clamps in parent.  
19. list/table rows.  
20. same Menu + `OpenAt`.  
21. `context-menu/basic`.  
22–23. outside click dismiss.  
24. O(items).

## CommandPalette

1. Fuzzy command launcher (exists; formalize).  
2. `root` · `query` · `list` · `empty` · `footer_hints`  
3. `commands` projection, filter consumer-owned  
4. query + list selection state (Picker-like).  
5. `default`  
6. min width from dim.palette.  
7. empty/loading.  
8. query vs results focus.  
9. type→query; up/down results; enter activate; esc clear then close.  
10. click row.  
11. trap focus in overlay layer.  
12. disabled commands skip.  
13. async: consumer swaps rows; show Skeleton/Empty.  
14. N/A.  
15. full width; drop footer.  
16. query only.  
17–18. as List.  
19. scene layer.  
20. `Activated` · `QueryChanged` · `Cancelled`.  
21. existing + `command-palette/loading`.  
22–23. two-stage esc.  
24. filter O(n) consumer; paint O(visible).

## ActionBar / HintBar / StatusBar

*(Existing — keep)* Spec abbreviated: ActionBar = horizontal Buttons; HintBar = Kbd+labels from Keymap; StatusBar = slots with priority drop on narrow. Full 1–24 as current contracts + DesignSystem recipes.

---

# Forms and input

## TextInput

1. Single-line grapheme-safe edit (exists).  
2. `root` · `label` · `value` · `placeholder` · `prefix` · `suffix` · `clear`  
3. value controlled or state-owned buffer; validation projection  
4. cursor, focus, scroll in state; value often state.  
5. `default` · `password` (mask) · `search`  
6. height 1; pad by density.  
7. invalid/disabled/read-only.  
8. editing.  
9. edit keys, enter submit optional, esc cancel optional.  
10. click set cursor; drag select if supported.  
11. focus caret.  
12. no edit.  
13. N/A async.  
14. invalid style + message below (consumer text).  
15. drop label.  
16. value only.  
17. grapheme cursor.  
18. underline invalid.  
19. Form fields, Combobox query.  
20. `Changed` · `Submit` · `Cancelled`.  
21. existing unicode stories.  
22–23. grapheme tests.  
24. O(width).

## TextArea

1. Multi-line editor (exists).  
2. `root` · `gutter?` · `lines` · `scrollbar`  
3. text state, title, placeholder  
4. buffer+cursor+scroll state.  
5. `default` · `code`  
6. min height 3 comfortable.  
7–18. as today + DesignSystem; wrap policy.  
19. PromptComposer body.  
20. `Changed` · `Scrolled` · `Cancelled`.  
21–24. existing + hot-path.

## Checkbox

1. Boolean toggle with label.  
2. `root` · `box` · `label` · `description`  
3. `checked`, `enabled`, `indeterminate`  
4. controlled checked preferred.  
5. default.  
6. 1–2 rows if description.  
7. checked/indeterminate.  
8. toggle.  
9. space toggle.  
10. click.  
11. focus on box.  
12. no toggle.  
13. N/A.  
14. invalid group message.  
15. drop description.  
16. box only.  
17–18. `[x]`/`[ ]`/`[-]`.  
19. Form, List leading.  
20. `Toggled { checked }`.  
21. `checkbox/states`.  
22–23. space.  
24. O(1).

## RadioGroup

1. Single choice among options.  
2. `root` · `option[]` · `label` · `radio`  
3. `options`, `selected_id`  
4. controlled selected.  
5. `vertical` · `horizontal`  
6. density gap.  
7. selected.  
8. move+select.  
9. arrows move; space select.  
10. click.  
11. roving focus.  
12. skip disabled.  
13. N/A.  
14. required validation consumer.  
15. vertical force.  
16. selected only.  
17–18. `(*)`/`( )`.  
19. forms, Permission choices.  
20. `Selected(Id)`.  
21. `radio-group/basic`.  
22–23. arrow wrap.  
24. O(options).

## Switch

1. Immediate binary setting.  
2. `root` · `track` · `thumb` · `label`  
3. `on`, `enabled`  
4. controlled on.  
5. default.  
6. width 4–6 cells.  
7. on/off.  
8. toggle.  
9. space.  
10. click.  
11. focus.  
12. no.  
13. loading optional.  
14. N/A.  
15. label then switch.  
16. `ON`/`OFF` text ascii.  
17–18. `[=]`/`[ ]`.  
19. settings.  
20. `Toggled { on }`.  
21. `switch/basic`.  
22–23. toggle.  
24. O(1).

## Select

1. Single select from list in overlay.  
2. `root` · `trigger` · `value` · `menu` · `option[]`  
3. `options`, `selected`, `placeholder`  
4. open + selected.  
5. `default`  
6. trigger height 1.  
7. open/closed/invalid.  
8. open menu.  
9. enter open; up/down; enter choose; esc close.  
10. click trigger/option.  
11. focus trigger; trap in menu.  
12. no open.  
13. async options → loading in menu.  
14. invalid trigger.  
15. menu full width.  
16. value only.  
17–18. as List.  
19. Form.  
20. `Selected` · `Opened` · `Cancelled`.  
21. `select/basic`.  
22–23. esc.  
24. O(visible options).

## MultiSelect

1. Many selected values.  
2. Select + `chip_row` · check options  
3. `selected: &[Id]`  
4. controlled selection set.  
5. `checkbox` · `chips`  
6–18. chips narrow drop.  
19. filters.  
20. `SelectionChanged`.  
21. `multi-select/chips`.  
22–23. toggle membership.  
24. O(visible).

## Combobox

1. Filterable select (Picker evolution).  
2. `root` · `input` · `list` · `empty`  
3. query + filtered projection  
4. as Picker today.  
5. `select_only` · `free_text`  
6–24. as current Picker contracts + intents split query/results.

## Form

1. Labeled fields layout + validation display (exists).  
2. sections/fields/error/help  
3–24. existing Form + DesignSystem; keyboard field nav.

---

# Selection (collections)

## List

1. Selectable rows with optional multi-check (exists; extend composed parts).  
2. `container` · `row` · `leading` · `primary` · `secondary` · `badge` · `shortcut` · `selection_indicator` · `scrollbar`  
3. rows projection, selection, multi  
4. selected_id, multi Selection, offset, hover, focus  
5. `single` · `multi`  
6. density pad; recipe.  
7–8. selected/hover/disabled/loading.  
9. intents move/page/activate/toggle/cancel.  
10. click/wheel.  
11. list focus; row cursor.  
12. skip disabled.  
13. row loading flag.  
14. N/A.  
15. part priority drop.  
16. primary only.  
17–18. gutter ascii.  
19. CommandPalette body, Sidebar.  
20. `Outcome` as today.  
21. existing + composed-row stories.  
22–23. intent tests.  
24. O(visible).

## Tree

1. Hierarchy expand/collapse (exists).  
2. row + `disclosure` · indent  
3–24. + Expand/Collapse intents; ascii disclosure; loading node status.

## Table / DataTable

1. **Table:** columnar data (exists). **DataTable:** opinionated product table (sort headers, pin, density) built on Table.  
2. `header` · `cell` · `row` · `scrollbar` · `sort_icon`  
3. columns, rows, sort keys  
4. selection, offset, sort request state  
5. Table `plain`; DataTable `striped` · `compact`  
6. column collapse right-first (existing).  
7–18. as Table + recipes.  
19. ObjectInspector uses DetailTable.  
20. `SortRequested` · `Activated` · selection outcomes.  
21. existing + `datatable/sort`.  
22–23. sort request only (no sort exec).  
24. visible window only.

## ObjectInspector

1. Key/value / nested object view (DetailTable evolution).  
2. `root` · `row` · `key` · `value` · `action`  
3. fields projection  
4. selected field  
5. `flat` · `nested`  
6–24. DetailTable contracts + copy/link capabilities.

---

# Feedback

## Toast

1. Transient notification (exists).  
2. `root` · `icon` · `message` · `close`  
3. severity, lifetime  
4. ToastState TTL  
5–24. existing + DesignSystem tones; mono glyphs.

## Progress

1. Determinate/indeterminate (exists).  
2. track/fill/label/%  
3–24. existing + Spinner split for indeterminate-only.

## Skeleton / EmptyState / LoadingView / ErrorView

As current view_state widgets; map EmptyState/ErrorView/Skeleton/Spinner stories; colorless glyphs required.

---

# Overlays

## Dialog

1. Modal container (exists).  
2. `backdrop` · `frame` · `title` · `body` · `footer` · `close`  
3. open, size  
4. open controlled; focus trap via scene  
5. `default` · `danger`  
6. min sizes from dim  
7–18. esc policy Trap/Dismiss; tiny = title+footer.  
19. MessageDialog, ChoiceDialog specialize.  
20. `Closed` · footer `Activated`.  
21–24. existing + scene integration.

## Drawer

1. Edge-docked modal panel.  
2. `backdrop` · `panel` · `title` · `body`  
3. `side: Left|Right|Bottom`, size fraction  
4. open  
5. side variants  
6. width/height %  
7–18. like Dialog; narrow = full width.  
19. mobile-like settings.  
20. `Closed`.  
21. `drawer/right`.  
22–23. esc.  
24. O(area).

## Popover

1. Non-modal anchored bubble.  
2. `frame` · `arrow?` · `content`  
3. anchor rect, placement  
4. open  
5. default  
6. auto flip (as completion menu).  
7–18. click outside dismiss; esc.  
19. Select, tooltips rich.  
20. `Closed`.  
21. `popover/flip`.  
22–23. placement clamp.  
24. O(content).

## Tooltip

1. Delayed label on hover/focus.  
2. `frame` · `text`  
3. text, delay_ms  
4. visible internal  
5. default  
6. max 40 cols wrap 1–2  
7–11. show on focus-visible/hover.  
12. no show disabled.  
13–14. N/A.  
15. clamp.  
16. hide if &lt;10 cols free.  
17–18. plain.  
19. IconButton.  
20. none.  
21. `tooltip/basic`.  
22. snapshot.  
23. delay with FrameTick.  
24. O(1).

## Backdrop

1. Occlusion layer (exists).  
2. fill glyph/style  
3–24. Reset bg + optional wash; not focusable.

## JumpOverlay

1. Letter jump (exists).  
2. badges on targets  
3–24. existing + scene registration.

---

# Data presentation

## LogStream

1. Append-only log with follow (LogPane evolution).  
2. `root` · `line[]` · `scrollbar` · `follow_chip`  
3. lines, bound history  
4. offset, follow  
5. `plain` · `structured` (level colors)  
6–24. LogPane contracts; streaming append O(1); hot-path.

## Timeline

1. Temporal events (exists).  
2. event · bullet · time · text  
3–24. existing + density.

## DiffReview

1. Reviewable diff (DiffView + navigation).  
2. hunk headers · lines · gutter +/−  
3. lines, cursor hunk  
4. offset, selected hunk  
5. `unified` · `split` (if width)  
6–18. syntax tokens; narrow force unified.  
19. PR review agent.  
20. `HunkActivated` · `Scrolled`.  
21. `diff-review/hunks`.  
22–23. hunk nav.  
24. O(visible lines).

## Charts (Sparkline, BarSeries, SegmentedMeter)

Existing; document viz tokens; colorless use glyph density only.

---

# Developer tools

## ThemePicker

1. Select DesignSystem preset (exists as Theme).  
2. list of presets · preview swatch  
3–24. switch DesignSystem; stories live preview.

## DesignInspector (new, studio)

1. Debug focus/scene/tokens.  
2. panels for layers, focus id, capability  
3–24. lookbook-only; not production default.

---

# AI-agent components

## PromptComposer

1. Multi-line prompt + attachments chrome (PromptBox evolution).  
2. `root` · `editor` · `attach_chip[]` · `mode_badge` · `send` · `footer_hints`  
3. text state, mode (plan/build), placeholder  
4. editor state + focus  
5. `simple` · `with_mode`  
6. min height 3  
7. empty/nonempty.  
8. editing vs send.  
9. Enter submit (config); Alt/Ctrl+Enter newline; esc cancel policy.  
10. click.  
11. focus editor.  
12. read-only when running.  
13. disabled send while streaming.  
14. N/A.  
15. drop chips then mode.  
16. single-line input fallback.  
17–18. as TextArea.  
19. AgentWorkbench south.  
20. `Submitted { text }` · `Changed` · `Cancelled`.  
21. `prompt-composer/submit`.  
22–23. enter vs newline.  
24. O(visible lines).

## PermissionPrompt

1. Risk-aware approval (ApprovalCard hardened).  
2. `frame` · `risk_glyph` · `title` · `detail` · `decision_row`  
3. risk, title, detail, decisions set  
4. **selected decision** (not raw index); default by risk (**Deny** if High)  
5. `card` · `inline`  
6. full width decisions wrap/stack on narrow  
7. risk tones.  
8. move decision; confirm.  
9. left/right; enter confirm; y/n shortcuts optional; **never** default allow on high.  
10. click decision.  
11. trap focus.  
12. N/A.  
13. N/A.  
14. N/A.  
15. stack decisions vertically.  
16. title + Deny/Allow only.  
17–18. risk glyphs.  
19. scene modal layer.  
20. `Decided(ApprovalDecision)` — **no side effects**.  
21. `permission-prompt/high-default-deny`, `permission-prompt/narrow`.  
22–23. enter on high → Deny if default.  
24. O(1).

## QuestionFlow

1. Multi-step agent questions (interview).  
2. `root` · `progress` · `question` · `options` · `nav`  
3. steps projection  
4. step_index, answers controlled  
5. `single` · `multi` questions  
6–18. Radio/MultiSelect composition.  
19. plan mode.  
20. `Answered { step, values }` · `Back` · `Skip`.  
21. `question-flow/basic`.  
22–23. back/next bounds.  
24. O(options).

## PlanReview

1. Present plan steps for accept/edit.  
2. `root` · `step_list` · `detail` · `actions` (accept/reject)  
3. steps, selected step  
4. selection  
5. default  
6–18. List+Markdown composition.  
19. agent plan mode.  
20. `Accepted` · `Rejected` · `StepSelected`.  
21. `plan-review/basic`.  
22–23. accept.  
24. O(visible steps).

## ToolCallCard

1. Mutable tool invocation display (exists ToolCard).  
2. `frame` · `status_glyph` · `name` · `summary` · `body` · `expand`  
3. status, name, summary, detail, expanded  
4. expanded controlled/uncontrolled  
5. `compact` · `expanded`  
6–18. status glyphs; streaming detail append.  
19. Transcript.  
20. `ToggledExpand` · `Activated` (open full).  
21. `tool-call-card/running`.  
22–23. expand.  
24. O(visible detail lines).

## TaskRail

1. Side list of tasks/subagents (Grok dashboard DNA).  
2. `root` · `task_row[]` · `status` · `title` · `meta`  
3. tasks projection  
4. selected task  
5. `compact`  
6–18. List composition + status colors.  
19. AgentWorkbench east/west.  
20. `Selected` · `Activated`.  
21. `task-rail/statuses`.  
22–23. select.  
24. O(visible).

## SessionPicker

1. Resume/pick agent sessions.  
2. `root` · `query?` · `session_row[]` · `time` · `title` · `preview`  
3. sessions projection  
4. selected, query  
5. `list` · `combobox`  
6–18. Combobox/List.  
19. startup overlay.  
20. `Picked(Id)` · `Cancelled`.  
21. `session-picker/basic`.  
22–23. pick.  
24. O(visible).

## ThinkingBlock / TokenMeter / Timeline / Stream→Transcript

Existing widgets; re-spec under DesignSystem + plan 041 transcript engine replacing StreamView for variable height.

---

# Application blocks

## AgentWorkbench

1. Flagship composition: TaskRail + Transcript + PromptComposer + overlays (Permission/Plan/Question) + Status/Token.  
2. Named panes via Workspace.  
3. layout config, child projections  
4. focus pane, open overlays  
5. `ide` · `compact`  
6–18. workspace collapse; scene owns esc.  
19. only block; no domain I/O.  
20. unions of child outcomes.  
21. `blocks/agent-workbench`.  
22–23. esc peel approval → focus prompt.  
24. sum of children; no extra O(n²).

## OpsDashboard / ResourceBrowser / SettingsShell / FormWizard

Compose charts+logs; tree+detail+preview; sections+forms; stepper+forms. Spec: geometry from workspace tree; all child contracts apply.

---

## E. Rust sketch — composed row + outcomes

```rust
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

pub enum SelectOutcome<Id> {
    Ignored,
    Changed,
    Activated(Id),
    Toggled { id: Id, checked: bool },
    Cancelled,
}
```

---

## F. Implementation phasing (components)

| Phase | Deliver |
|-------|---------|
| Now | Harden existing (List, Table, Dialog, …) to this contract language |
| 039–040 | PermissionPrompt, scene-backed overlays |
| 041 | Transcript replaces StreamView |
| 042 | Workspace; Sidebar/Drawer natural |
| 043–044 | Recipes + intents on all collections |
| Next | Button family, Menu, Select, Checkbox/Radio/Switch |
| Flagship | AgentWorkbench block + stories |

---

## G. Definition of done (any component)

- [ ] Anatomy named and recipe-backed  
- [ ] Outcomes typed; no side effects  
- [ ] Keyboard + mouse + focus documented and tested  
- [ ] Disabled / loading / invalid as claimed  
- [ ] Narrow + tiny + unicode + ascii + colorless stories or contract `not-applicable`  
- [ ] Snapshot + interaction tests  
- [ ] Perf note + test if virtualized  
- [ ] Lookbook story IDs registered  

**Rendering alone never checks the box.**
