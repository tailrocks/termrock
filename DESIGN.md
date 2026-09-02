<!--
SPDX-FileCopyrightText: 2026 Alexey Zhokhov
SPDX-License-Identifier: Apache-2.0
-->

# TermRock design language

This is the implemented TermRock design system. It is a one-to-one port of the
Junie TUI language (`terminal-components-claude`). Tokens, glyphs, spacing,
state grammar, and interaction rules below are **the** system. Widgets ask
`DesignSystem` / `JunieTheme` for a style given state and never spell RGB.
Both are read-only views of one system. `RolePalette::junie()` is the
Role-indexed view of `JunieTheme::for_level`; hosts do not ship a second
palette.

TermRock mapping:

| Junie | TermRock |
|---|---|
| `Theme` | `termrock::style::JunieTheme` + `RolePalette::junie()` |
| `Theme::row/lift/gutter/button/field_style/...` | `DesignSystem::{row,lift,gutter,...}` and `JunieTheme` resolvers |
| Button | `widgets::Button` (`primitives.rs`) |
| Checkbox/radio/toggle | `widgets::Checkbox`, `RadioGroup`, `Switch`/`Toggle` |
| Text input / textarea | `widgets::TextInput`, `TextArea` |
| Select / chips / picker | `widgets::Select`, `Tag`/`Chip`, `QuickOpen`/`CommandPalette` |
| List / tree / table / grid | `widgets::List`, `Tree`, `Table`, `VirtualGrid`/`DataTable` |
| Dialog | `widgets::Dialog` / `AlertDialog` |
| Progress / empty / keyhint | `widgets::ProgressBar`, `EmptyState`, `HintBar` |
| Panel/card/frame | `widgets::Panel`, `Card`, `Surface` |
| Footer status (no toasts) | `HintBar` + `StatusBar`; `Toast` paints as a footer sentence |

Colour-capability fallbacks: `--color truecolor|256|16|none` and `NO_COLOR`.
Visual verification: `verify/junie` cell-grid compare against fresh `junie-tui` captures.
Current gate: 40 equivalent showcase crops PASS at `text_cells: 0` / `color_cells: 0`; 5 TablePro product-shell scenes SKIP (campaign non-goal, not a widget gap).

Shipped contracts that the lookbook must consume (no page-local forks):

- Table reverse cell cursor is `TableState::cell_nav`. `focused_column` without that flag is not a cursor; Left/Right stay row-select. `cell_nav` with no column seeds the first visible column on paint (junie `cursor_col` starts at 0).
- Line overflow thumbs (`Panel`, `Picker`, `List`, `Tree`, `TextArea`, `Select`) use `scroll::overflow_thumb` / `paint_overflow_scrollbar`: `len = (viewport * track) / content`.
- Picker search footer paints junie's spelled `Alt+Enter`, not Emacs `A-↵`. Searchable pickers type `j`/`k`/Space into the query; Tab is `PickerOutcome::NextScope`; Alt+Enter is `PickerOutcome::ActivatedAlt`.

The YAML token block and prose that follow are the Junie specification TermRock implements.

---
version: alpha
name: Junie TUI
description: Terminal-native design system extracted from the junie-tui Ratatui implementation (design-system showcase and TablePro workbench). Tokens are exact; prose explains how the implementation uses them.
omitted:
  - section: typography
    reason: "The terminal emulator owns font family, size, line height and letter spacing. The application controls only modifiers (bold, italic, underline, strikethrough), tone and layout, which are specified in the Typography prose."
  - section: rounded
    reason: "Corners are Unicode box-drawing glyphs on a character grid (╭ ╮ ╰ ╯), not a geometric radius. The Shapes prose specifies the glyph sets."
colors:
  primary: "#48e054"
  accent-hover: "#3ab343"
  accent-pressed: "#2b8632"
  accent-tint: "#0f2e13"
  on-accent: "#19191c"
  canvas: "#000000"
  surface: "#111111"
  surface-elevated: "#18181b"
  surface-overlay: "#27272a"
  popover: "#3f3f46"
  field: "#1e1e22"
  field-hover: "#232328"
  border-subtle: "#262626"
  border-strong: "#4d4d4d"
  text-primary: "#ffffff"
  text-secondary: "#b3b3b3"
  text-muted: "#808080"
  text-faint: "#4d4d4d"
  text-ghost: "#262626"
  error: "#e44545"
  warning: "#f59e09"
spacing:
  gutter: 1
  inline: 1
  gap: 2
  column-gap: 2
  form-gap: 4
  card-inset: 2
  frame-inset: 3
  dialog-inset: 3
  tree-indent: 2
  field-height: 3
  tabs-height: 2
  min-width: 72
  min-height: 20
components:
  row:
    backgroundColor: "{colors.canvas}"
    textColor: "{colors.text-primary}"
  row-hover:
    backgroundColor: "{colors.surface-elevated}"
    textColor: "{colors.text-primary}"
  row-selected-focused:
    backgroundColor: "{colors.accent-tint}"
    textColor: "{colors.text-primary}"
  row-pressed:
    backgroundColor: "{colors.text-primary}"
    textColor: "{colors.canvas}"
  row-disabled:
    textColor: "{colors.text-faint}"
  focus-gutter:
    textColor: "{colors.primary}"
    width: "{spacing.gutter}"
  button-primary:
    backgroundColor: "{colors.primary}"
    textColor: "{colors.on-accent}"
  button-primary-hover:
    backgroundColor: "{colors.accent-hover}"
    textColor: "{colors.on-accent}"
  button-primary-pressed:
    backgroundColor: "{colors.accent-pressed}"
    textColor: "{colors.on-accent}"
  button-secondary:
    backgroundColor: "{colors.surface-overlay}"
    textColor: "{colors.text-primary}"
  button-secondary-hover:
    backgroundColor: "{colors.popover}"
    textColor: "{colors.text-primary}"
  button-subtle:
    textColor: "{colors.text-secondary}"
  button-danger:
    backgroundColor: "{colors.surface-overlay}"
    textColor: "{colors.error}"
  button-danger-pressed:
    backgroundColor: "{colors.error}"
    textColor: "{colors.text-primary}"
  field:
    backgroundColor: "{colors.field}"
    textColor: "{colors.text-primary}"
    height: "{spacing.field-height}"
  field-hover:
    backgroundColor: "{colors.field-hover}"
    textColor: "{colors.text-primary}"
  field-placeholder:
    backgroundColor: "{colors.field}"
    textColor: "{colors.text-muted}"
  card:
    backgroundColor: "{colors.surface}"
    textColor: "{colors.text-primary}"
    padding: "{spacing.card-inset}"
  frame:
    backgroundColor: "{colors.canvas}"
    textColor: "{colors.text-primary}"
    padding: "{spacing.frame-inset}"
  frame-border:
    textColor: "{colors.border-subtle}"
  frame-border-focused:
    textColor: "{colors.border-strong}"
  dialog:
    backgroundColor: "{colors.surface-elevated}"
    textColor: "{colors.text-primary}"
    padding: "{spacing.dialog-inset}"
  popup:
    backgroundColor: "{colors.surface-elevated}"
    textColor: "{colors.text-primary}"
  text-selection:
    backgroundColor: "{colors.popover}"
    textColor: "{colors.text-primary}"
  badge-edit:
    backgroundColor: "{colors.primary}"
    textColor: "{colors.on-accent}"
  tabs:
    height: "{spacing.tabs-height}"
    textColor: "{colors.text-secondary}"
  status-error:
    textColor: "{colors.error}"
  status-warning:
    textColor: "{colors.warning}"
  status-success:
    textColor: "{colors.primary}"
  text-secondary:
    textColor: "{colors.text-secondary}"
  text-muted:
    textColor: "{colors.text-muted}"
  text-faint:
    textColor: "{colors.text-faint}"
  backdrop-ghost:
    textColor: "{colors.text-ghost}"
  key-hint-key:
    textColor: "{colors.text-primary}"
  key-hint-action:
    textColor: "{colors.text-muted}"
  screen-minimum:
    width: "{spacing.min-width}"
    height: "{spacing.min-height}"
  action-row:
    padding: "{spacing.gap}"
  dialog-actions:
    padding: "{spacing.inline}"
  table-header:
    textColor: "{colors.text-muted}"
    padding: "{spacing.column-gap}"
  form-columns:
    padding: "{spacing.form-gap}"
  tree-row:
    textColor: "{colors.text-primary}"
    padding: "{spacing.tree-indent}"
---

# Junie TUI

## Overview

Junie TUI is the terminal translation of the Junie visual language: a near-black
canvas, a small number of dark planes, white text stepped down an opacity
ladder, and one green used only where the user's attention belongs. Two
applications share the system, a component showcase and a database workbench
(TablePro), and both look like the same product because they share tokens,
widgets, glyphs, and one interaction grammar.

What makes a screen recognisably Junie TUI:

- **One hue.** Green (`primary`, `#48e054`) marks keyboard focus, the primary
  action, the current or chosen item, the active document tab, the editing
  badge and live activity. Everything else is achromatic. Red and amber exist
  only as safety tones (error, dirty or risky).
- **State is geometry before colour.** Focus is a `▎` bar in the first column
  plus bold text. Hover lifts the background one plane and never touches
  focus. Selection is a marker glyph (`›`, `✓`). Editing is the hardware
  cursor plus an accent underline plus an `EDIT` badge in the footer. Errors
  add a bold `!` and a message. Every state survives a monochrome terminal.
- **Whitespace groups; borders bound.** Cards are filled planes with no
  border. A rounded frame appears only where a pane needs an edge (explorer,
  tab body, modal surfaces). One blank row separates sections; nothing is
  boxed twice.
- **Dense rows, quiet chrome.** Data rows are one cell high with two-cell
  column gaps; headers are muted; identity and status live in one-line strips.
  Density comes from rows, not from shrinking insets.
- **Keyboard first, mouse equal.** Every control is a Tab stop in reading
  order; composite widgets (lists, trees, grids, tab strips) are one stop with
  an internal cursor. Mouse hover, click, drag, wheel and scrollbar work on the
  same widgets without stealing keyboard focus.
- **Contextual discoverability.** The footer shows only the hints for the
  focused control, most important first, and drops from the right. Modals
  replace the hints. There is no permanent shortcut wall.
- **Safety proportional to risk.** Routine actions are green, destructive
  actions start on Cancel, and irreversible database statements require the
  target's name to be typed before the confirming button enables.
- **Feedback is quiet and timed.** A status sentence on the footer's right
  edge for 4–5 seconds, a spinner while something runs, a `✓` when a job
  finishes. No toasts, no flashing.

A new screen belongs to this product when a reader can find the keyboard
destination in under a second, the only green on the screen is where the
rules above allow it, and nothing is framed that a blank row could separate.

## Colors

The palette is a black canvas, five neutral planes, a five-step white ladder,
two border strengths, one accent with two darker steps and a tint, and two
safety tones. Every value lives in the theme module; widgets ask the theme for
a style given their state and never spell an RGB value.

### Planes

- **Canvas (`#000000`)** is the page. Pane bodies inside frames and the
  identity strip sit directly on it.
- **Surface (`#111111`)** is the card plane: titled cards, the form card,
  the detail card. It is the default container for grouped content.
- **Surface elevated (`#18181b`)** is one plane up: dialogs, popups, the
  picker, and the hover lift of a canvas row.
- **Surface overlay (`#27272a`)** is the rest fill of secondary, toggle and
  danger buttons, and the hover lift of a surface or elevated row.
- **Popover (`#3f3f46`)** is the strongest neutral: text selection, range
  selection in grids, the current find match, and the hover fill of a
  secondary button.
- **Field (`#1e1e22`)** is the body of every text entry; **field hover
  (`#232328`)** is its hover, applied only while not editing.

Hover always lifts exactly one plane: canvas → elevated, surface or elevated →
overlay, field → field hover, anything else → popover. It never changes hue.

### Text ladder

White at 100 / 70 / 50 / 30 / 15 percent over black:

- **text-primary (`#ffffff`)** content, focused labels, key names, titles.
- **text-secondary (`#b3b3b3`)** supporting text, unfocused labels,
  busy labels, strings and numbers in code, active progress fill.
- **text-muted (`#808080`)** metadata, placeholders, helper text, hint
  actions, column headers, `NULL`, operators and punctuation in code.
- **text-faint (`#4d4d4d`)** panel meta, row numbers, comments, disabled
  content. It is also the disabled foreground and the strong border.
- **text-ghost (`#262626`)** appears only under a modal backdrop; never for
  live content.

### Borders

- **border-subtle (`#262626`)** unfocused frames, the tab-strip baseline, the
  progress track, the scrollbar track.
- **border-strong (`#4d4d4d`)** focused frames, the quiet white rule under
  secondary-level tabs, and the neutral underline (hover on an editable cell,
  bracket match, the current line while editing a text area or code).

### Accent

- **primary (`#48e054`)** focus gutter, primary button fill, `›`/`✓` markers
  on the focused row, the active document-tab underline, the `EDIT` badge,
  spinners, the indeterminate sweep, completed progress, the required-field
  `*`, the selected tree label, and the `✓` of a checked box.
- **accent-hover (`#3ab343`)** and **accent-pressed (`#2b8632`)** exist only
  for the primary button's hover and press.
- **accent-tint (`#0f2e13`)** is the background of a row that is both
  selected and focused. It never appears on an unfocused selected row, and a
  hover lift replaces it.
- **on-accent (`#19191c`)** is the text on any green fill.

### Safety tones

- **error (`#e44545`)** invalid fields and cells, the `!` glyph, error
  messages, diagnostics, failed progress, the danger button's label at rest and
  its fill when pressed, the reversed cursor on an erroneous cell.
- **warning (`#f59e09`)** changed-but-unsaved values, the `•` modified marker,
  pending counts, warning diagnostics, the Safe Mode token when a production
  connection runs Silent, the `▲` cost marker in EXPLAIN plans.

Amber is never used as a second accent and red is never used for routine
destructive affordances at rest beyond the label of a danger button.

Four approved pairings sit below the 4.5:1 AA ratio and are kept on purpose:
the primary button while pressed (`on-accent` on `accent-pressed`, visible
for 140 ms), the danger button's red label on overlay and its white-on-red
press, and the placeholder (`text-muted` on `field`). None of them carries
information that is not also present as a glyph, a label, or the value that
replaces the placeholder.

### Pairings and states

| State | Treatment |
|---|---|
| default | text-primary on the container plane |
| hovered | same text, plane lifted one step; suppressed on disabled controls and while editing a field |
| focused | `▎` in primary, bold text; containers brighten their frame or show the bar in the title row |
| selected, unfocused | marker glyph only, text-primary, no tint |
| selected + focused | marker glyph, bold, accent-tint background |
| pressed | reversed: canvas text on text-primary, 140 ms after activation |
| disabled | text-faint; no bar, no hover, not in the Tab ring |
| error | error text, trailing bold `!`, message in error |
| editing | field plane keeps its colour; accent underline under the text; hardware cursor |
| busy | spinner in primary, label in text-secondary |

### Colour-level fallback

`COLORTERM=truecolor` selects the palette above; a `TERM` containing
`256color`, `ghostty` or `kitty` maps every token to the nearest xterm-256
value; other terminals get 16 named colours; `NO_COLOR` forces monochrome.
Both binaries accept `--color truecolor|256|16|none`. What must survive at
every level: the `▎` bar, bold for focus, underline for editing, the reversed
cursor cell, `!`, `›`, `✓` and `•`. At 16 colours the accent is LightGreen and
error is LightRed; in monochrome all hue is gone and the glyph and modifier
language carries the state alone.

### Declared but dormant

Junie's `Theme` struct still carries `accent_bg_subtle`, `error_bg` and
`info` (`#8787ff`); no resolver uses them. TermRock `JunieTheme` omits those
fields. They are not part of the system; do not introduce them into new
screens. The Overview Tokens page copies the reference swatch list, including
the dormant `info` hex, as page content — not as a `Role`.

## Typography

The terminal owns the font. The application controls weight, slant,
underline, strikethrough, tone, case, punctuation and alignment, and builds
its whole hierarchy from those.

### Modifiers and their meanings

- **Bold** means "this is where the keyboard is" or "this is the heading":
  focused rows and labels, panel titles, the active tab, key names in hints,
  keywords in code, matched characters in a fuzzy match, the current line
  number in the editor, the reversed cursor cell. Bold is removed on purpose
  from chrome inside a focused row (tab prefixes, kind glyphs, row numbers,
  completion details) so the row's content stays the loudest element.
- **Italic** means absence of a value: `NULL` and `DEFAULT` cells, comments in
  code. Nothing else is italic.
- **Underline** carries three meanings distinguished by colour: accent
  underline = editing here (single-line input, the picker query, the find
  needle, an in-place cell editor; it turns red when the edit fails
  validation); border-strong underline = quiet affordance (hover on an
  editable cell, hover on a sortable header, bracket match, other find hits,
  the current line while editing multi-line text); error or warning underline
  = a diagnostic range in code.
- **Strikethrough** appears only on a grid row queued for deletion, with faint
  text; its markers and row number stay legible so it can be undone.
- Dim and reverse-video attributes are never used. Dimming is done by stepping
  down the ladder; "reversed" is drawn explicitly as canvas-on-white so it
  degrades predictably.

### Hierarchy

| Role | Treatment |
|---|---|
| Screen identity | `▪` mark, product name bold, breadcrumb in text-secondary, one line |
| Panel or card title | bold text-primary when focused, text-secondary otherwise; meta right-aligned in text-faint on the same row |
| Section heading (sidebar groups) | text-faint, sentence case, one blank row before |
| Field label | text-secondary; bold text-primary when its field has focus; required `*` in primary; `optional` suffix in text-faint only when it fits whole |
| Value / content | text-primary |
| Metadata (counts, sizes, timestamps) | text-muted, right-aligned in the row |
| Helper text | text-muted below the field |
| Hint | bold key + muted action, `Esc Cancel` |
| Status message | text-secondary, right edge of the footer |
| Warning / dirty | warning tone plus `•` or `▲` |
| Error | error tone plus bold `!` |
| Disabled | text-faint, no modifiers |
| Selected content | `›` or `✓` marker; the label keeps text-primary |
| Code | keywords bold; identifiers plain; strings and numbers text-secondary; operators muted; comments faint italic |

### Case and punctuation

- Sentence case everywhere: titles, buttons, hints, labels. No Title Case, no
  uppercase headings. The only uppercase element is the `EDIT` badge; SQL
  keywords and identifiers such as `DDL` are literals.
- A trailing `…` on a button means "opens a dialog or needs more input":
  `Delete…`, `Rename task…`. Terminal actions have none: `Cancel`, `Save`.
- ` · ` (space, middle dot, space) joins clauses on one line: `3 d ago · 5 ms`,
  `Cancelled · nothing was executed`. The clause after the dot is lower case.
- ` › ` is hierarchy: `acme_prod › public`, `public › orders`. Never a peer
  separator.
- `…` (one glyph) truncates; `truncate_middle` keeps the tail of identifiers.
  In-progress statuses end with it: `Saving…`, `Opening SSH tunnel…`.
- Ranges use an en dash: `rows 1–27 of 500`, `12–24 of 120`.
- Numbers are grouped with thousands separators; durations carry a spaced
  unit: `1.903 ms`, `1.2 M rows`.
- Status sentences are past tense, no period: `Cell saved`, `Changes
  discarded`. A colon introduces the reason for a refusal: `Nothing to run:
  the cursor is between blocks`.
- Numeric cells right-align; text left-aligns; rows stay one cell high.

## Layout

All measurements are terminal cells. The spacing tokens are unitless cell
counts.

### Shell

Both applications use the same shell: a one-row header, a blank row, the
body, a blank row, a one-row footer. The showcase body spans the full width;
TablePro's body has a one-cell margin on each side.

- **Showcase**: navigation sidebar (`19` columns, `24` from `110` columns
  wide) + `2` gap + main pane; an optional inspector (`30` columns) appears at
  `100` columns and wider. Below `25` rows the sidebar drops its section
  labels and blank rows and becomes one contiguous list.
- **TablePro**: identity strip (segments), a two-row tab strip, then the body:
  a framed explorer (`body ÷ 4`, clamped `28–40`) + `1` gap + a framed tab
  body. Below `100` columns the explorer becomes a drawer: it covers the whole
  body while it has focus and disappears when focus leaves (`0` opens it,
  Tab or opening an object puts it away).
- **Connections screen**: list pane (`width ÷ 3`, clamped `26–40`) + `2` gap
  + detail or form card; a single list below `80` columns.

### Rhythm

| Token | Cells | Use |
|---|---:|---|
| `gutter` | 1 | the `▎` column at the start of every row and control |
| `inline` | 1 | gap between buttons inside dialogs and between tabs; the editor/results split |
| `gap` | 2 | gap between panes, columns of cards, buttons in forms and action rows |
| `column-gap` | 2 | between table and grid columns; segments in a strip |
| `form-gap` | 4 | between the two columns of a form |
| `card-inset` | 2 | card horizontal padding; vertical padding is 1 row and the title occupies it |
| `frame-inset` | 3 | content start inside a rounded frame (border + 2), one spare column on the right |
| `dialog-inset` | 3 | dialog horizontal padding inside its frame; vertical padding is 2 rows |
| `tree-indent` | 2 | per depth level |
| `field-height` | 3 | label row, field row, help/error row (inputs and selects) |
| `tabs-height` | 2 | label row and underline row |
| section break | 1 row | between related blocks; never replaced by a border |

Row anatomy is universal: `▎` at column 0, a one-cell marker slot at column 1,
content from column 3. Buttons are `label + 2` wide (one cell each side, the
left cell becoming `▎` on focus), `+ 2` more when they carry a toggle marker or
spinner. Text areas are `rows + 2` high. Props blocks align values at the
widest label `+ 2`.

### Compositions

- **Sidebar + workspace** (showcase): navigation list on the left, one page on
  the right. Use when the product is a set of peer screens.
- **Explorer + tabbed workspace** (TablePro): a framed tree plus a framed tab
  body with document tabs in a strip above. Use when many objects are open at
  once.
- **Master/detail** (connections, history): list on the left, detail card on
  the right; the list wins when the width cannot hold both.
- **Editor + results**: vertical split, `38 %` editor with minima `4/6`, one
  blank row between; `Ctrl+↑/↓` resizes, `z` maximises either half.
- **Table + contextual controls**: mode tabs (Data / Structure), an optional
  filter chip row, the grid, and a one-line status; pending changes add a
  two-row bar at the bottom of the grid.
- **Searchable list**: a plain-label search field above a list; the field
  yields width to a scope readout before either truncates.
- **Modal workflow**: dialog or picker centred over a dimmed page.
- **Command palette / quick switcher**: the picker, centred in the upper
  third, with query, grouped rows, scope and its own hint row.

### Responsive rules

Minimum size is `72×20`. Below it both apps show a centred four-line notice
(product name, `Terminal too small`, `Need 72×20, have W×H`, `q Quit`) and
nothing else. Representative sizes are `80×24`, `100×30`, `120×40`, `160×50`.

When space runs out, things leave in this order:

1. Low-priority segments of the identity strip (capability and size first,
   then counts, then help, then the schema path; identity, connection and
   running state last; ties drop from the right).
2. Footer hints from the right; a hint is never cut mid-word. The status
   message always keeps its space.
3. Row metadata, all or none per view (tree and list), rather than per row.
4. Status-line parts by priority (row range last, column range first).
5. Secondary panes: the explorer becomes a drawer; a split gives the whole
   area to one side when both minima cannot fit.
6. Truncation with `…`, then overflow controls: `‹ ›` in a tab strip, `‹N N›`
   hidden-column counts on a grid, `…` at the edges of a table.

Labels are never shrunk to fit; controls are never overlapped.

### Scrolling and clipping

Scrolling belongs to the container. Arrows and `j/k` move one row, page keys
one viewport, `Home/End` and `g/G` jump, and the cursor is kept visible with
the smallest scroll. A one-column scrollbar (`│` track, `┃` thumb) appears only
on overflow, brightens when its container is focused, and can be clicked or
dragged. The wheel scrolls the topmost container under the pointer by three
rows and never moves focus. Position reads `12–24 of 120`; grids add
`rows 1–27 of 500 loaded · 1,203,338 total · cols 2–3 of 14`. Wide tables and
grids keep column widths and expose the hidden columns rather than squeezing
every column; the next column is drawn clipped so the pane never ends in blank
space.

## Elevation & Depth

There are no shadows. Depth is tonal, structural and modal:

1. **Tonal planes.** Canvas, surface, elevated, overlay and popover form the
   vertical scale. A card is a filled surface on the canvas; a dialog is an
   elevated surface; a selection is a popover patch. Hover lifts a background
   one plane and only for as long as the pointer stays.
2. **Frames.** A rounded frame is drawn only where a region needs a hard edge:
   panes that scroll independently, and every floating surface (dialog,
   picker, popup, completion). The frame brightens from border-subtle to
   border-strong when its content has focus, and its title switches from
   text-secondary to bold.
3. **Titles.** Cards have no border; their focus shows as `▎` in the title
   row's left inset and a bold title. Meta (`14 cols`, `1–21 of 24`) sits
   right-aligned on the same row in text-faint.
4. **Backdrop.** A modal dims everything below it by walking each cell two
   steps down the ladder: primary, accent, error and warning text become muted;
   secondary becomes faint; the rest becomes ghost; field fills become
   elevated; any coloured fill becomes overlay; all modifiers are cleared. The
   page stays readable but inert. The footer row is excluded so the modal's
   own hints stay live.
5. **Anchored popups** (select options, completion) are elevated surfaces
   with a rounded frame placed below their anchor, flipped above when there is
   no room, then clamped to the screen. They are drawn last so they sit above
   later siblings.

Whitespace ranks above all of these: use a blank row before a card, a card
before a frame, a frame before an overlay.

## Shapes

Shape is glyph vocabulary on a character grid.

### Borders and rules

- Rounded box drawing (`╭ ╮ ╰ ╯ ─ │`) is the only frame set: framed panes,
  dialogs, pickers, popups, the filter editor. Subtle when idle, strong when
  focused.
- `─` is a quiet rule: the tab-strip baseline, the empty progress track, the
  body of a toggle switch.
- `━` is an active rule: the active document tab's underline (primary), a
  secondary-level tab's underline (border-strong), the filled part of a
  progress bar, the sweeping indeterminate segment.
- `│` / `┃` are the scrollbar track and thumb.
- Tables, lists, trees and grids draw no row boxes and no column walls; two
  blank cells separate columns.
- Cards have no border at all.

### Markers and indicators

| Glyph | Meaning | Where |
|---|---|---|
| `▎` | keyboard focus | first column of every focused row or control; title row of a focused card |
| `›` | current or chosen item | lists, tables, selects, the current block in the editor; also the path separator |
| `✓` | checked, selected row, completed | checkboxes, multi-select lists, grid row selection, finished progress |
| `•` | modified, pending | dirty grid rows, dirty tabs, the pending bar |
| `+` / `−` | inserted / deleted row | grid change slot |
| `!` | error, diagnostic | after a field, in a tab, in a grid row, in the editor gutter, after a failed progress bar |
| `▲` | warning weight | cost share above 50 % in a plan |
| `▸` / `▾` | collapsed / expanded | tree disclosure; a spinner replaces it while children load |
| `▴` / `▾` | sort ascending / descending | header suffix; `▾` / `▴` also closed / open on a select |
| `∇` | filter applied to this column | header suffix |
| `▪` | identity mark; primary key | the product mark in the header and strip; the pk header prefix |
| `→` | follows a reference | trailing cell of a foreign-key column |
| `↓` | more rows available | the fetch-more virtual row |
| `‹` `›` | hidden content in that direction | tab strip overflow; `‹N` / `N›` hidden-column counts |
| `…` | truncated or clipped | ends of text, horizontal scroll edges, collapsed JSON |
| `×` | close / remove | tabs, chips; faint until hovered |
| `● ○` | on / off | toggle button, radio `(●)`, switch knob |
| `[✓] [ ]` | checked / unchecked | checkbox |
| `◆ ◇` | production / staging | environment identity in the strip and connection list |
| `⠋⠙⠹⠸⠼⠴⠦⠧⠇⠏` | activity | ten-frame spinner, primary tone, 80 ms tick |

Glyphs carry meaning; nothing is decorative. Reuse the table above before
adding a symbol, and never assign a second meaning to a glyph already listed
(the `▾` collision between tree and select is disambiguated by context and
should not be extended).

## Components

### Interaction grammar

Global keys shared by both applications:

- `Tab` / `Shift+Tab` move focus through every enabled control in reading
  order and wrap. Composite widgets are one stop.
- Arrow keys and `h j k l` move the cursor inside the focused widget;
  `PgUp/PgDn` page; `Home/End` and `g/G` jump.
- `Enter` activates or starts editing; `Space` activates, toggles, or selects
  a row.
- `Esc` climbs a ladder: cancel single-line editing → finish multi-line
  editing → close the popup or completion → cancel the modal → leave local
  mode for the containing navigation (showcase: to the sidebar; TablePro:
  cancel a running query → un-maximise → tab strip → explorer, clearing the
  explorer filter on the way).
- `0` jumps to the navigation (sidebar or explorer). `[` `]` switch pages or
  tabs. `?` opens help. `q` quits (`Ctrl+C` always; in TablePro it first
  cancels a running query and then asks when work is unsaved).
- Chords that would collide with typing (`Ctrl+L`, `Ctrl+N`, `Ctrl+D`, `q`,
  `z`) are ignored while a control is editing.

Screen-specific chords (TablePro workbench): `Ctrl+R`/`F5` run the statement
under the cursor, `Alt+R` run all, `Ctrl+X`/`Alt+X` explain / explain analyze,
`Ctrl+T`/`Ctrl+W` new / close tab, `Ctrl+O`/`Ctrl+P` open quickly, `Ctrl+G`
tab list, `Ctrl+Y` history, `Ctrl+B` explorer, `Ctrl+L` Safe Mode level,
`Ctrl+D` Data ⇄ Structure, `Ctrl+S` save pending changes, `Ctrl+F` find or
filter, `z` maximise, `Ctrl+↑/↓` resize the split. The connections screen adds
`Ctrl+N` new connection and `/` filter.

Contextual keys belong to the focused widget and appear in the footer (`s`
sort, `f` filter, `p` preview, `x` close, `u` undo, `y` copy, `*`/`-` expand
or collapse all). Future screens reuse these letters for the same verbs.
Application chords are handled before the focused widget sees a key, so a
widget key that collides with a chord (the grid's `Ctrl+D` duplicate under
TablePro's `Ctrl+D` Data ⇄ Structure) is unreachable in that application.

Editing keys are shared by every text control: `Ctrl+A/E` line start/end,
`Ctrl/Alt+←→` and `Alt+B/F` by word, `Shift+arrows` select, `Ctrl+U/K` delete
to start/end, `Ctrl+W` delete word, `Ctrl+L` select all, `Ctrl+Home/End`
document start/end. Single-line controls: `Enter` commits, `Esc` reverts,
`Tab` commits and moves on (to the next field, or the next editable cell).
Multi-line controls: `Enter` inserts a newline, `Esc` finishes and keeps the
text. Losing focus commits. Paste inserts only into the control that is
editing.

Mouse: hover previews; the first click focuses, the second click on an
already-focused field or the current cell starts editing; a click on a header
sorts; a click on a tab activates it and on its `×` closes it; drag selects
text or a cell range or moves a scrollbar thumb; the wheel scrolls the
container under the pointer; clicking outside a cancelable dialog, picker or
open select closes it. Any key press suppresses hover until the pointer moves
again, so a stale lift never competes with the focus bar. Mouse-down records
the press, mouse-up activates only if it lands on the same target; the
pressed flash lasts 140 ms for keyboard and mouse alike.

### Focus model

The focus ring is rebuilt every frame in render order, so Tab order is reading
order and deterministic. Disabled controls register a hit region but no ring
entry. Modals push a barrier into both the ring and the hit registry so
nothing below is reachable; anchored popups push only a hit barrier and leave
keyboard focus with their owner. Opening a dialog saves the current focus,
clears hover and press, and sets the dialog's initial focus (confirm: the
primary action; destructive: Cancel; prompt: the field; typed
acknowledgement: the field). Closing restores the saved focus; a focus that no
longer exists snaps to the first reachable stop. Hidden-but-reachable stops
(the explorer drawer, the tab body behind it) register with an empty area so
Tab still reaches them.

### State grammar

| Situation | Treatment |
|---|---|
| Initial / no data | `EmptyState`: centred muted title, one blank row, faint wrapped hint that names the key which fills it (`Ctrl+N creates one`) |
| Loading | spinner in primary with a secondary label (`Loading rows…`, `Opening SSH tunnel…`); busy buttons show the spinner, drop bold and refuse activation; lazy tree nodes show the spinner in the disclosure slot |
| Loaded | rows; a muted status line with position and totals |
| Partial data | `↓ 500 loaded · Enter fetches more` virtual row; `~` before estimated totals; `N loaded · M total` |
| No matches | the same empty state with `No matches` / `No matching queries` |
| Recoverable error | red `!` + message where it happened (field help row, cell, tab, editor gutter and footer); the control keeps focus and the value |
| Failed operation | the result tab gets `!`, the status reads `failed · 2 ms`, an `Error` card shows the message and a trimmed detail; the editor underlines the offending token |
| Disconnected / connection failed | `!` + message + detail on the connection card; the action relabels to `Reconnect` and a `Retry` button appears |
| Disabled action | text-faint, no bar, not in the ring; a nearby helper explains when it matters (`Fixed by the connection`) |
| Read-only | content fully readable, mutation absent, the reason in the status line (`read-only: …`) |
| Destructive operation | `Dialog::destructive` starting on Cancel; database writes use the facts dialog (Action, Target, Scope, Risk, Reversible, Safe Mode, the SQL) with a typed target name gating the confirming button |
| Background operation | `⠋ running` segment in the strip, `⠋ running 320 ms · Esc cancels` in the results area, `Esc` or `Ctrl+C` cancels, cancelled runs show `Cancelled` |
| Pending changes | `•` per row, amber values, `• 2 pending · 1 update · 1 delete` bar with Preview SQL / Discard / Save, `• 2 pending` in the strip |
| Success | footer status in text-secondary for 4 s (showcase) or 5 s (TablePro): `Saved 2 changes to public.orders`; long jobs end with `✓` |

### Component catalogue

#### Button

- **Purpose**: one action or a persistent toggle.
- **Anatomy**: `▎label ` on one row; toggle variant prefixes `●`/`○`; busy
  variant prefixes the spinner. Width `label + 2` (`+ 2` with marker).
- **Styling**: primary = on-accent on primary, bold; secondary and toggle =
  text-primary on overlay; subtle = text-secondary on the container; danger =
  error on overlay. Hover lifts (primary → accent-hover, secondary → popover,
  subtle → one plane). Focus adds `▎` and bold (primary is already bold; its
  focus is the bar alone). Pressed reverses (primary → accent-pressed, danger →
  white on error).
- **States**: default, hover, focus, pressed (140 ms), toggled, disabled
  (faint, no hover), busy (spinner, secondary label, no activation).
- **Keys**: `Enter` or `Space`. **Mouse**: click.
- **Usage**: one primary button per decision; destructive actions use the
  danger variant and end in `…` when a dialog follows. **Avoid**: boxing
  buttons, more than one primary in a row, using a button for a persistent
  setting (use a toggle).

#### Checkbox, radio group, toggle

- **Anatomy**: `▎[✓] label`, `▎(●) option` rows under a label, `▎──● label on`.
  Marks are primary when on, muted when off.
- **Behaviour**: `Space`/`Enter` toggles; a radio group is one focus stop whose
  `↑↓`/`j k` move and select at once; option rows are click targets only.
- **Usage**: checkbox for independent flags, radio for one-of-few with all
  options visible, toggle for an on/off setting, select for one-of-many.

#### Text input

- **Anatomy**: three rows: label (required `*` in primary, `optional` suffix in
  faint only when whole), field on the field plane starting at column 2 with
  `…` overflow markers, help or error row. A trailing bold `!` marks an error.
- **States**: navigation (bar, bold label), editing (accent underline,
  hardware cursor, hover suppressed), hover (field-hover plane), disabled,
  error (`!` + message replaces help), placeholder (muted).
- **Keys**: `Enter`/`F2` edit; single-line editing rules above; validation on
  commit. **Mouse**: first click focuses, second click edits at the pointer.
- **Avoid**: changing the field plane to signal editing (the signal is
  underline + cursor); clipping the `optional` suffix; validating on every
  keystroke unless the field is already in error.

#### Text area

- **Anatomy**: label, `rows` body lines with a two-cell inset, optional
  scrollbar column, footer with help/error left and `ln 3/12` or `a–b of N`
  right. The current line shows a border-strong underline while editing.
- **Keys**: `Enter`/`F2` edit; `↑↓`/`j k` scroll in navigation; multi-line
  editing rules; `Esc` finishes and keeps the text.
- **Avoid**: promising `Esc` reverts a text area; it does not.

#### Select

- **Anatomy**: three rows like an input; the field shows the value and a
  trailing `▾` (`▴` when open). Open, an elevated rounded popup lists options
  with `›` on the selected one, `12–40` wide, at most `10` rows, below the
  field or flipped above.
- **Keys**: closed `↑↓←→` change the value without opening, `Enter`/`Space`
  opens; open `↑↓`/`j k` move, `Enter`/`Space` commit, `Esc` reverts the cursor
  and closes. Losing focus closes. Clicking outside must be routed by the
  owner.
- **Usage**: bounded choices that need not all be visible. Render the open
  select last so its popup sits above later siblings.

#### Chip bar

- **Anatomy**: optional lead (`match all ▾`), chips `▎label ×` (toggle style;
  faint when disabled; error tone when invalid), a subtle `+ Add filter` stop,
  `…` when chips overflow. One focus stop with a logical cursor.
- **Keys**: `←→`/`h l` move; `Enter` edits or adds; `Space` toggles;
  `x`/`Delete`/`Backspace` removes; `+` adds; `X` clears all.
- **Usage**: active filters and tags above a data set.

#### List

- **Anatomy**: `▎`, marker (`›` chosen, `✓` checked), label, right-aligned
  muted meta (all or none when the label would drop below 12 cells),
  scrollbar. Empty shows a centred muted sentence.
- **Keys**: `↑↓`/`j k` (`Shift` extends a range in multi-select), page keys,
  `Home/End`, `g/G`, `Enter`/`Space` choose or toggle, `a` toggles all
  (multi). Disabled rows are skipped for selection.
- **Mouse**: click chooses, wheel scrolls, scrollbar drags.

#### Tree

- **Anatomy**: `▎`, `depth × 2` indent, `▾`/`▸` (or spinner while loading), an
  optional kind glyph (`D S T V ƒ #` in TablePro), label (accent green when
  selected, muted for notes), right-aligned meta.
- **Keys**: `↑↓`/`j k`; `→`/`l` expands or steps in; `←`/`h` collapses or
  steps out; `Enter`/`Space` toggles a folder or activates a leaf; `*` expands
  all, `-` collapses all. A filter keeps ancestors visible and opens matches.
- **Mouse**: row click, a separate two-cell disclosure target, wheel,
  scrollbar.
- **Usage**: hierarchies (files, schema objects). **Avoid**: ASCII connector
  lines; indentation and disclosure glyphs carry the structure.

#### Tabs

- **Anatomy**: two rows; each tab is `▎ prefix label ✕`, where the state slot
  after the label holds one of spinner, `!` or `•`; a `─` baseline; `━` under
  the active tab in primary for document tabs, in border-strong for secondary
  levels (Data / Structure, structure sections); `‹ ›` when the strip
  overflows; `+` for new.
- **Keys**: `←→`/`h l` move and activate; `1–9` jump; `x`/`Delete` close;
  `n` new. **Mouse**: tab, `×`, `‹ ›`, `+`.
- **Rule**: one accent underline per screen. Nested tab levels use the quiet
  white rule. Labels never shrink; the strip scrolls.

#### Panel (card and frame) and scroll panel

- **Card**: filled surface, `2×1` inset, title in the inset row, meta on the
  right, `▎` in the title inset when the card is the focus stop. Default
  container.
- **Frame**: rounded border on the canvas, title inline in the top edge, meta
  and badge on the same edge, content starting three cells in. Strong border
  and bold title when focused. Use for panes and floating surfaces only.
- **Scroll panel**: read-only text inside either, one focus stop, `f` toggles
  follow-tail, `End`/`G` enables it, any manual scroll disables it.
- **Avoid**: a frame around a card, a card inside a card, a bar on both a
  container and its child.

#### Table

- **Purpose**: general in-memory table with sorting and optional inline
  editing.
- **Anatomy**: muted header (sorted column primary, `▴`/`▾` suffix, underline
  on hover), rows with `▎` and `›`, two-cell gaps, `…` at the edges for hidden
  columns, scrollbar; the current cell is reversed (white background, canvas
  text, bold).
- **Keys**: cursor keys and vim keys; `s` cycles sort asc → desc → none;
  `Enter`/`Space` selects a row or, in cell mode, `Enter`/`F2` edits; `Tab`
  commits and hops to the next editable cell.
- **Mouse**: header click sorts, second click on the current cell edits.
- **Rule**: sorting permutes a source order so selection and edits survive it.

#### Data grid

- **Purpose**: typed, paged database rows with a pending-change queue.
- **Anatomy**: header with `▪`/`⚷` primary key, `∇` filtered, `▴▾` sort,
  `‹N N›` hidden columns; row slots `▎ ✓ • n` (focus, selection, change,
  number); cells; a fetch-more row; a two-row pending bar (`• 2 pending · 1
  update · 1 delete`, Preview SQL / Discard / Save as focus stops; the
  cursor row's rejection message replaces the breakdown).
- **Cell styling**: `NULL`/`DEFAULT` muted italic; empty string faint `''`;
  changed value in warning tone; error in error tone with `!`; reference `→`;
  cursor reversed (error background when invalid); a range in popover; deleted
  rows faint and struck through.
- **Keys**: cursor and vim keys with `Shift` ranges, `Ctrl+←→` column pages,
  `Home/End` columns, `Ctrl+Home/End` rows; `Enter`/`F2` edits (booleans cycle,
  JSON and long text open the viewer), `Space` selects a row, `Delete` sets
  NULL or deletes selected rows, `+`/`-` insert / delete, `Ctrl+D` duplicate,
  `u`/`U` undo / discard, `y`/`Y` copy cell / row, `Ctrl+S` save, `s`/`S` sort
  / clear sort, `f`/`/`/`F` filter on cell / open filters / clear, `r`/`F5`
  refresh, `p` preview SQL, `Ctrl+]` follow reference.
- **Mouse**: header sorts, row number selects, second click edits, drag
  selects a range, `→` follows, `‹ ›` step columns, wheel scrolls both axes.
- **Layout**: column widths from the 95th percentile of the first 200 rows,
  clamped per type, never narrower than the header; the last column is drawn
  clipped rather than leaving blank space.
- **Usage**: database data only; the owner runs queries and commits. **Avoid**:
  using it for static tables (use Table), saving silently, hiding the reason
  a save failed.

#### Code editor

- **Anatomy**: gutter `▎ marker  nn ` (bar on the cursor line, marker `›`
  for the current block or spinner for the running block or `!` for a
  diagnostic, line numbers bold on the cursor line and secondary inside the
  block), text with caller-supplied syntax tones, popover selection, find
  matches, bracket match, diagnostic underlines; footer with the find bar or
  the nearest diagnostic on the left and `ln 1/26 · col 18` on the right.
- **Keys**: navigation `i`/`a`/`Enter` edit, `{ }` blocks, `/` find, `n N`
  matches, `←→` horizontal scroll; editing rules above, `Tab` indents (or
  leaves when configured), `Esc` finishes.
- **Rule**: the language lives in the caller (highlighter and block
  segmenter); the editor knows nothing about SQL.

#### Completion

- **Anatomy**: anchored popup under the word, `24–48` wide, up to `8` rows of
  `▎ kind label … detail` with matched characters bold; non-modal, the editor
  keeps focus.
- **Keys**: `↑↓`, `Ctrl+N/P`, page keys; `Tab` or `Enter` accepts; `Esc`
  dismisses; other keys go to the editor.

#### Picker

- **Purpose**: modal chooser (open quickly, tab list, Safe Mode level).
- **Anatomy**: centred in the upper third over the dimmed page, rounded
  elevated surface, title with a scope readout on the right, an accent-marked
  query field when searchable, rows in fixed columns `▎ glyph label · detail ·
  tag · group` computed over all items so scrolling never shifts alignment,
  a faint hint row at the bottom.
- **Keys**: typing filters; `Esc` clears the query, then cancels; `Enter`
  chooses, `Alt+Enter` the alternate action; `Tab` cycles scope; `Delete` the
  secondary action; `Ctrl+N/P`, `Ctrl+J/K`, page keys move.
- **Rule**: the owner ranks and supplies rows on every query change.

#### Dialog

- **Variants**: confirm (subtle Cancel + primary; focus on the primary),
  destructive (secondary Cancel + danger; focus on Cancel), prompt (field;
  `Enter` submits), facts (props table, code preview capped at six lines with
  `… N more`, optional `Type orders to confirm` field whose match enables the
  last action; `Enter` in the field only moves focus).
- **Anatomy**: dimmed backdrop (footer excluded), centred rounded elevated
  surface `54` wide (`66` for facts) with a `3×2` inset, bold title, body,
  right-aligned actions with one-cell gaps.
- **Keys**: `←→`/`h l` between enabled actions, `Enter`/`Space`, `Esc` and
  `n` cancel, `y` confirms (text bodies only). Nothing leaks to the page.

#### Progress

- `label ━━━━───── 64% ✓` with a fixed two-cell status suffix (`✓` done, `!`
  error, `‖` paused). Active fill is text-secondary; green is reserved for
  completion. The track drops to a percentage below six cells. Indeterminate
  work sweeps a primary segment; compact activity is `⠋ label`.

#### Empty state

- Centred muted title, one blank row, faint wrapped hint. No glyph, no focus.
  The hint names the key that fills the space.

#### Segments strip

- One row of toned facts left and right with two-cell separators and explicit
  priorities; the lowest priority drops first. Clickable segments pad one cell
  and lift on hover. Used for the identity strip and status readouts.

#### Props

- Aligned label/value pairs: labels muted in a column as wide as the longest
  label plus two, values toned and optionally wrapped. Used for connection
  details, dialog facts and plan details.

#### Key hints and badge

- Footer pairs of bold key and muted action from the left; the ` EDIT ` badge
  (on-accent on primary) leads while a control is editing; the status message
  owns the right edge; hints drop from the right when they would collide.

#### Scrollbar

- One column, hidden without overflow, owned by its container, not a focus
  stop; click and drag map to the container offset.

### Composed patterns

- **Form**: inputs, selects, radio groups, checkboxes and toggles in one or
  two columns (`form-gap` 4), a section break per group, an action row of
  Test / Cancel / Save / Save & connect at the bottom. `Ctrl+S` submits at
  screen level; the first invalid field takes focus.
- **Explorer**: a framed tree with a plain-label filter field above it.
- **Workbench tab**: mode tabs, chips, grid, status line, pending bar.
- **Query tab**: editor over results with a result tab strip (`p` pins a
  result so the next run keeps it, `x` closes) and one status row.
- **History**: searchable list on the left; on the right a card with the
  wrapped query, facts under it, actions right after the facts.
- **Safety gate**: the facts dialog composed from the statement classifier.

There is no toast, context menu, diff viewer or generic badge component. Do
not claim one exists; add it to the showcase first if it becomes necessary.

## Do's and Don'ts

- **Do** take every colour from the theme's semantic tokens and resolvers.
  **Don't** write an RGB value in a widget or screen.
- **Do** keep green for focus, the primary action, the chosen marker on the
  focused row, the active document tab, the edit badge and live activity.
  **Don't** use green for body text, backgrounds, counts, environment names or
  a second-level tab rule.
- **Do** show focus as `▎` + bold, hover as a one-plane lift, selection as a
  marker, editing as underline + cursor + badge. **Don't** let hover move
  focus, let a cursor move create a selection, or make navigation and editing
  look alike.
- **Do** put a blank row between sections and a card around grouped content.
  **Don't** frame a card, nest frames, box every widget, or draw column walls.
- **Do** keep rows one cell high with two-cell column gaps and sentence-case
  labels. **Don't** invent a spacing value; use the tokens in Layout.
- **Do** reuse the glyph table and the contextual letters (`s` sort, `f`
  filter, `p` preview, `x` close, `u` undo, `y` copy). **Don't** add a symbol
  with a new meaning or a new key for an existing verb.
- **Do** design the empty, loading, partial, error, disabled, read-only and
  narrow states of every new screen using the state grammar. **Don't** ship
  a screen that only renders its happy path at `120×40`.
- **Do** truncate with `…`, drop metadata all-or-none, and expose hidden
  columns with `‹N N›`. **Don't** shrink labels, overlap controls or cut a
  hint mid-word.
- **Do** make destructive emphasis proportional: danger button, Cancel
  focused, typed acknowledgement for irreversible writes. **Don't** confirm
  harmless navigation or paint a production screen red.
- **Do** verify a visual change with the capture harness at `80×24`,
  `100×30`, `120×40` and `160×50` and against the showcase baseline.
  **Don't** regenerate the baseline to hide an unintended change.
- **Do** update this file when a reusable convention is added on purpose.
  **Don't** document a one-screen workaround as a system rule.

### Agent implementation guardrails

1. Inspect the showcase page and the widget module for a component before
   writing a new one; every generic component already has a page, and a new
   generic component must get one, at `120×40` and `80×24`, in the same
   change.
2. Widgets draw and register in one pass: `render` paints, registers hit
   regions (container first, rows and close affordances after) and focus
   stops (in reading order); `on_key`, `on_click`, `on_drag`, `on_wheel`
   return `Ignored`, `Consumed` or `Changed`. Follow that shape exactly.
3. Ask the theme for styles by state (`row`, `button`, `field_style`,
   `gutter`, `tone`, `syntax`, `badge`, `border`, `lift`). Never construct a
   style from colour constants.
4. Keep domain knowledge out of the library: SQL tokenising, catalog types
   and safety levels live in the application and reach widgets as functions
   and plain data.
5. A modal must call `begin_modal`; an anchored popup must be drawn after its
   siblings and route outside-clicks through its owner.
6. Ignore typing-conflicting chords while any control is editing; route
   editing keys through the shared edit keymap.
7. Responsive behaviour is prioritisation, not scaling: segments and hints
   drop by priority, metadata hides all-or-none, secondary panes become
   drawers, and only then does text truncate.
8. Every new state must be legible in monochrome: pair each colour with a
   glyph or modifier from the tables above.
9. Run `cargo fmt --check`, `cargo clippy --all-targets -- -D warnings` and
   `cargo test`; the showcase baseline must change only for the pages you
   intended to change.
10. Treat the rendered capture as the evidence: the harness in `tools/`
    produces the frames to compare against, and a change is not done until
    the frame has been looked at.
