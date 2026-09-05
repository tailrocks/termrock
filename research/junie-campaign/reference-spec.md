# junie-tui Canonical Reference Specification

- **Date:** 2026-09-02
- **Source repo:** `/Users/donbeave/Projects/terminal-components-claude` (crate `junie-tui`)
- **Source commit:** `e43cf670d6cb793e5761819e8778600797bbf1aa` (branch `main`, clean tree)
- **Stack:** Ratatui 0.30 (`features = ["crossterm_0_29"]`), crossterm 0.29, `unicode-width 0.2`, `unicode-segmentation 1`, edition 2024, rust 1.88 (`Cargo.toml:1-30`)

**Authority order.** `src/theme.rs` is the only source of colour truth. Rendered truth is `shots/*.txt` (deterministic tmux captures, `tools/capture.sh`) plus `tests/showcase_baseline.txt`. Where `DESIGN.md` prose disagrees with source or shots, **source + shots win**; every such conflict is listed in §8. Line numbers refer to the commit above.

**Notation.** Glyphs are given as literal characters. `BOLD`, `ITALIC`, `UNDERLINED`, `CROSSED_OUT` are Ratatui modifiers. "col 0 / col 1 / col 3" are offsets from the row's own `x`.

---

## 1. Palette and colour system

### 1.1 Raw palette (`src/theme.rs:67-88`, module `palette`)

| Constant | Hex | Role |
|---|---|---|
| `BLACK` | `#000000` | canvas |
| `CHROME` | `#111111` | surface (chrome plane) |
| `CARD` | `#18181b` | surface_elevated |
| `INPUT` | `#1e1e22` | field |
| `INPUT_HOVER` | `#232328` | field_hover |
| `OVERLAY` | `#27272a` | surface_overlay |
| `POPOVER` | `#3f3f46` | popover |
| `WHITE` | `#ffffff` | text_primary |
| `WHITE_70` | `#b3b3b3` | text_secondary |
| `WHITE_50` | `#808080` | text_muted |
| `WHITE_30` | `#4d4d4d` | text_faint, border_strong, disabled |
| `WHITE_15` | `#262626` | text_ghost, border_subtle |
| `GREEN` | `#48e054` | accent, focus, success |
| `GREEN_80` | `#3ab343` | accent_hover |
| `GREEN_60` | `#2b8632` | accent_pressed |
| `GREEN_20` | `#0f2e13` | accent_bg |
| `GREEN_10` | `#0a1c0c` | accent_bg_subtle (dormant, §1.5) |
| `ON_GREEN` | `#19191c` | text_on_accent |
| `RED` | `#e44545` | error |
| `RED_20` | `#2e0f0f` | error_bg (dormant, §1.5) |
| `AMBER` | `#f59e09` | warning |
| `PURPLE` | `#8787ff` | info (dormant, §1.5) |

### 1.2 Role mapping — `Theme::junie()` (`src/theme.rs:131-162`)

canvas=BLACK, surface=CHROME, surface_elevated=CARD, surface_overlay=OVERLAY, field=INPUT, field_hover=INPUT_HOVER, popover=POPOVER, border_subtle=WHITE_15, border_strong=WHITE_30, text_primary=WHITE, text_secondary=WHITE_70, text_muted=WHITE_50, text_faint=WHITE_30, text_ghost=WHITE_15, text_on_accent=ON_GREEN, accent=GREEN, accent_hover=GREEN_80, accent_pressed=GREEN_60, accent_bg=GREEN_20, accent_bg_subtle=GREEN_10, focus=GREEN, disabled=WHITE_30, error=RED, error_bg=RED_20, warning=AMBER, success=GREEN, info=PURPLE.

### 1.3 Capability downgrade (`src/theme.rs:30-43`, `166-201`)

`ColorLevel::detect()`: `NO_COLOR` set → `Mono`; `COLORTERM` ∈ {truecolor,24bit} → `TrueColor`; `TERM` contains `256color`|`ghostty`|`kitty` → `Ansi256`; else `Ansi16`. `Theme::for_level(level)` applies `downgrade()` to **all 27** colour fields. Ansi256 uses nearest cube-or-gray-232; Ansi16 uses luminance + hue; Mono collapses to white/gray/black. Test: at Ansi16, accent=`LightGreen`, error=`LightRed`, canvas=`Black` (`src/theme.rs:632-635`).

### 1.4 Style resolvers (exact behaviour, `src/theme.rs:206-508`)

Base helpers: `base()` = fg(text_primary).bg(canvas); `on(bg)` = fg(text_primary).bg(bg); `primary/secondary/muted/faint()` = fg(text_{primary,secondary,muted,faint}); `accent_fg()` = fg(accent); `error_fg()` = fg(error); `title()` = fg(text_primary)+BOLD; `label(focused)` = focused ? `title()` : `secondary()`; `key_hint_key()` = fg(text_primary)+BOLD; `key_hint_action()` = fg(text_muted); `border(focused)` = fg(focused ? border_strong : border_subtle); `selection()` = fg(text_primary).bg(popover); `scrollbar_track()` = fg(border_subtle); `scrollbar_thumb(focused,hovered)` = fg(text_primary) if focused else fg(text_secondary) if hovered else fg(text_muted); `tone(Tone)` maps Normal/Secondary/Muted/Faint→text ladder, Error/Warning/Success→error/warning/success — **never the accent** (`theme.rs:458-468`).

**`row(s, bg)` — the universal row resolver (`theme.rs:308-340`).** Applied in this order, later steps override:

1. `disabled` → `fg(disabled).bg(bg)` (return).
2. base `fg(text_primary).bg(bg)`.
3. `selected && focused` → `bg(accent_bg)` (selection tint only where the keyboard is; elsewhere the marker glyph alone carries "selected").
4. `hovered` → `bg(lift(bg))` — hover is always exactly one plane up, never a colour.
5. `error` → `fg(error)`.
6. `busy` → `fg(text_secondary)`.
7. `focused` → `+BOLD`.
8. `pressed` → **full replacement**: `fg(canvas).bg(text_primary)+BOLD` (reversal).

**`lift(bg)` (`theme.rs:341-352`)** — the hover ladder: canvas→surface_elevated; surface|surface_elevated→surface_overlay; field→field_hover; anything else→popover.

**`button(kind, s, bg)` (`theme.rs:366-431`)**

| Kind | Idle | Hover | Focused | Pressed |
|---|---|---|---|---|
| Primary | fg(text_on_accent).bg(accent)+BOLD | bg(accent_hover) | (no extra; already BOLD) | bg(accent_pressed) |
| Secondary, Toggle | fg(text_primary).bg(surface_overlay) | bg(popover) | +BOLD | replaced: fg(canvas).bg(text_primary) |
| Subtle | fg(text_secondary).bg(bg) | fg(text_primary).bg(lift(bg)) | fg(text_primary)+BOLD | replaced: fg(canvas).bg(text_primary) |
| Danger | fg(error).bg(surface_overlay) | bg(popover) | +BOLD | replaced: fg(text_primary).bg(error) |

Disabled (all kinds): `fg(disabled).bg(bg)` for Subtle, `fg(disabled).bg(lift(bg))` otherwise.

**`field_style(s)` (`theme.rs:433-443`)** — disabled → fg(disabled).bg(field); bg = field_hover when `hovered && !editing` else field; fg(text_primary). **`placeholder(s)`** = `field_style(s).fg(disabled|text_muted)`.

**`gutter(s, bg, on_accent)` (`theme.rs:355-365`)** — fg = bg (invisible) when `!focused`; text_primary when `on_accent`; otherwise focus.

**`backdrop(style)` (`theme.rs:276-301`)** — modal dimming. bg kept for canvas/surface/surface_elevated; field/field_hover→surface_elevated; any other fill→surface_overlay; `None`→canvas. fg: a glyph whose fg equals its own bg stays = bg (hidden gutter stays hidden); canvas|surface→bg; text_primary|accent|error|warning→text_muted; text_secondary|text_on_accent→text_faint; else text_ghost.

**`syntax(SyntaxTone)` (`theme.rs:470-499`)** — Keyword: fg(text_primary)+BOLD; Ident|Plain: fg(text_primary); Str|Number: fg(text_secondary); Operator|Punct: fg(text_muted); Comment: fg(text_faint)+ITALIC.

**`badge(BadgeKind::Edit)` (`theme.rs:500-508`)** = fg(text_on_accent).bg(accent)+BOLD. This is the only badge variant.

### 1.5 Dormant tokens (verified by grep over all resolvers)

`accent_bg_subtle` (GREEN_10), `error_bg` (RED_20) and `info` (PURPLE) are **never read by any resolver**; `info` appears only in the Overview token swatch list (`src/bin/showcase/pages/overview.rs:21-43`). `focus` and `accent` are the same green; `success` likewise.

---

## 2. Surfaces, borders, padding

### 2.1 Surface ladder

`canvas` (black) → `surface` (#111) → `surface_elevated` (#18181b) → `surface_overlay` (#27272a) → `field` (#1e1e22)/`field_hover` (#232328) → `popover` (#3f3f46). A control never sits more than one plane above its container: hover = `lift`, selection tint = `accent_bg`, reversal (pressed / cell cursor) = white bg + canvas fg.

### 2.2 Border language

| Surface | Fill | Border | Inset to content |
|---|---|---|---|
| Canvas page | `t.canvas` | none | shell-defined |
| Card (`Panel::card`) | `t.surface` | **none** (surface change is the border) | `Margin::new(2,1)` → content at x+2, y+1 (`src/widgets/panel.rs:80-118`) |
| Framed panel (`Panel::framed`) | `t.canvas` | `Rounded` ALL, `t.border(focused)` | frame-inset 3: `Margin::new(1,1)` then x+2, width−3 (`panel.rs:120-186`) |
| Dialog / popup surface | `surface_elevated` | `Rounded` ALL, `t.border(true)` (`src/ui/popup.rs`) | `Margin::new(1,1)` from `popup::surface()`; dialogs use `Margin::new(3,2)` (`src/widgets/dialog.rs:387`) |
| Picker | `surface_elevated` | `Rounded` ALL + `border(true)` | `Margin::new(2,1)` (`src/widgets/picker.rs:201+`) |
| Field body | `field_style` | no border; editing = `UNDERLINED` with `underline_color(accent)` | text at field.x+2 |
| Popover/completion/select popup | `surface_elevated` | `Rounded` | 1,1 |

Titles: `Panel` title row at `area.x+2`, width−4, style `t.title()` when focused else `t.secondary()`; framed panels wrap title/meta in `" {} "`; meta is right-aligned in `t.faint()`; badge `" {badge} "` in `t.badge(kind)`. Titled cards return an inner rect shifted +1 row (`panel.rs:80-186`).

### 2.3 Spacing tokens (DESIGN.md:413-427, confirmed in source)

| Token | Value | Evidence |
|---|---|---|
| gutter | 1 col (`▎`) | every row control |
| inline | 1 | `x = area.x + 1` in header/footer/segments |
| gap | 2 | `segments::render` sep (`src/widgets/segments.rs:63`), grid column gap (`src/widgets/grid.rs:1454`), most button rows |
| column-gap | 2 | table `Layout::horizontal(...).spacing(2)` (`src/widgets/table.rs:521`) |
| form-gap | 4 | showcase forms/inputs/textareas/settings splits |
| card-inset | 2 | `Panel::card` Margin::new(2,1) |
| frame-inset | 3 | `Panel::framed` |
| dialog-inset | 3 | `Margin::new(3,2)` in dialog.rs:387 |
| tree-indent | 2 | `indent = depth*2` (`src/widgets/tree.rs:502`) |
| field-height | 3 | `TextInput::HEIGHT = 3` (`input.rs:138`), `Select::HEIGHT = 3` (`select.rs:33`) |
| tabs-height | 2 | tab strip occupies 2 rows |
| min window | 72 × 20 | `MIN_WIDTH`/`MIN_HEIGHT` in both apps |

Derived: `Panel::card` inner content x+2 / y+1; panel title row consumes 1 extra row; table body reserve = 1 row (header) and 1 col (gutter) + 5 cols (`area.width - 5`) (`table.rs:561-570`).

---

## 3. Typography

### 3.1 Modifiers per role

| Role | Style |
|---|---|
| Page/product title | fg(text_primary)+BOLD (`title()`) |
| Panel title, focused | `title()`; unfocused `secondary()` |
| Header identity word (`Junie`, `TablePro`) | `title()` |
| Key in a key hint | fg(text_primary)+BOLD |
| Hint action word | fg(text_muted) |
| Label of a focused field | `title()`; otherwise `secondary()` |
| Row under keyboard focus | row style + BOLD |
| Sort/keyword emphasis | BOLD only; no colour change |
| Comment (syntax) | text_faint + ITALIC |
| NULL / DEFAULT cell | text_muted + ITALIC (`grid.rs:1807-1826`) |
| Deleted row | text_faint + CROSSED_OUT (`grid.rs:1711-1763`) |
| Matched fuzzy chars | +BOLD over the base style |
| Sortable header under pointer | +UNDERLINED, underline_color(border_strong) |
| Editing text | +UNDERLINED, underline_color(accent) (error colour when validation fails) |
| Badge ` EDIT ` | fg(text_on_accent).bg(accent)+BOLD |

**No underline, strikethrough, italic or colour is ever used for hover** — hover is strictly one surface plane up.

### 3.2 Text helpers (`src/ui/text.rs`)

- `truncate(s, max)` — append `…` when it does not fit.
- `truncate_middle(s, max)` — `keep_end = (max-1)/3`; `very_long_identifier_name` at 12 → `very_lon…ame`. Used for ids and long identifiers (`grid.rs:1969-2005`).
- `fit(s,w)` / `fit_right(s,w)` — pad/truncate to exactly `w` cells (left / right aligned).
- `wrap(s, w)` — word wrap with hard fallback; used by dialogs, cards, props.
- `thousands(n)` — `,` grouping.
- `width(s)` — unicode display width; wide chars count 2.
- `fuzzy(needle, hay)` — penalty tiers: prefix 0, boundary substring 10, substring 30, subsequence `60 + idx`; returns matched byte offsets so callers can BOLD the matched characters.
- Position labels: `scrollbar::position_label` = `"{start+1}–{end} of {len}"` (en dash, `src/widgets/scrollbar.rs:53-59`); grid `rows A–B of T · cols C–D of N` (`grid.rs:1389-1420`); code editor `ln {l+1}/{n} · col {c+1}`.

---

## 4. Widget specifications

Every widget is a plain state struct with `render(area, buf, ctx, bg)` that **draws and registers** hit/focus regions, plus `on_key` / `on_click` / `on_drag` / `on_wheel` returning `Outcome::{Ignored, Consumed, Changed}` (Changed dominates, `or()`). `bg` is the container background so a widget renders correctly on canvas, surface or dialog.

### 4.1 Button (`src/widgets/button.rs`)

- Geometry: 1 row. `width() = label + 2 + (2 if toggle || busy)` (:62).
- Anatomy: `▎` at x (gutter, `t.gutter(s, bg, on_accent)` with `on_accent = kind==Primary && !disabled`), label from x+1, one trailing space.
- Toggle prefix `●` / `○` (U+25CF/U+25CB) at x+1 — accent when on, text_muted when off, **style unchanged when pressed**; toggle is `ButtonKind::Toggle` so it shares Secondary colours.
- Busy prefix: `spinner_frame(tick) + ' '` in accent; the rest of the label loses BOLD and becomes text_secondary (:101-165).
- `can_activate()` = `!disabled && !busy` (:67). Keys: Enter/Space → `(Outcome, bool activated)`.
- Layout helpers: `row_layout(area, widths, gap)` left-aligned, `row_layout_right(area, widths, gap)` right-aligned (:167-190). Dialogs and the grid pending bar use `row_layout_right(..., gap = 1)`; page action rows use gap 2.

### 4.2 Chip bar (`src/widgets/chips.rs`)

- Chip width `1 + label + 1 + (2 if removable) + 1`; removable `×` at `x+2+label_w`, idle text_muted, hover text_primary+BOLD.
- Disabled chip = fg(text_faint); error chip = fg(error); enabled chips use `ButtonKind::Toggle`, disabled ones `Secondary`.
- Overflow → single `…` in text_muted.
- Lead segment rendered `" {lead} "` muted; hover → primary + lift. Default add label `"+ Add filter"`.
- Keys: Left/h, Right/l, Enter (Activate|Add), Space (Toggle), Delete/Backspace/x (Remove), `+` (Add), `X` (ClearAll). Events `ChipEvent::{Activate,Toggle,Remove,Add,Lead,ClearAll}`.

### 4.3 Checkbox / RadioGroup / Toggle (`src/widgets/choice.rs`)

- Checkbox `HEIGHT` = 1: `"[✓]"` / `"[ ]"` at x+1 (accent / text_muted), label at x+5.
- RadioGroup: single focus stop; label at area.x+2, options from area.y+1, `"(●)"` / `"( )"` at row.x+1, option text at row.x+5; height = options+1; **Up/k and Down/j move and select**; Space/Enter selects; each option has its own hit id `option_id(i)`.
- Toggle (switch): `HEIGHT` = 1, glyph `──●` (on) / `○──` (off) at x+1 (accent / text_muted), label at x+5, trailing `"on"`/`"off"` at `x+6+label_w` in text_muted.

### 4.4 Completion popup (`src/widgets/completion.rs`)

- `width = (max_label_w + max_detail_w + 8).clamp(24, 48)`, `max_rows = 8`, height = rows+2. Anchored with `place(.., Placement::Below)`.
- Row: `▎` at row.x, glyph at row.x+1 (text_primary when focused else text_muted, BOLD removed), label from row.x+3 with matched chars BOLD, detail right-aligned in text_muted shown only when `avail > label+detail+2`.
- Keys: Down|Ctrl+N, Up|Ctrl+P, PageUp/PageDown by `max_rows`, Tab|Enter = Accept, Esc = Dismiss.

### 4.5 Dialog (`src/widgets/dialog.rs`)

Constructors: `confirm` (w 54, subtle Cancel + primary, initial focus = confirm, `cancel_index = Some(0)`) (:58); `destructive` (w 54, secondary Cancel + danger, focus = Cancel) (:75); `prompt` (w 54, takes a `TextInput`, focus = input) (:92); `facts` (w 66, `Vec<Prop>`, optional code block, optional `AckInput{input,token}`) (:110). `.with_actions(actions, cancel_index)` overrides the pair; help dialogs reuse `confirm` with the primary removed (:showcase app.rs:553-558).

- `armed()` = ack text trimmed == token (:139). The confirming action is disabled until armed; ack label is `"Type {tok} to confirm"` with `.plain_label()`.
- `height(w) = 2 + 1 + 1 + 1 + body_h + 1 + 1 + 1` (:168-186). Facts body = `facts + (code.min(6) + 1) + (ack ? HEIGHT+1 : 0)`; code preview capped at 6 lines, the last line becomes `"{truncated} … {n} more"`.
- Render (:357+): backdrop dims `screen.height - 1` rows (footer excluded) via `t.backdrop` **and clears modifiers**; `begin_modal()`; `width = min(self.width, screen.width - 4).max(20)`; `inner = area.inner(Margin::new(3, 2))`; title at inner.x/inner.y in `t.title()`; body_y = inner.y+2; text body wrapped in `t.secondary()`; actions_y = `area.bottom() - 3`; actions laid out with `row_layout_right(..., gap = 1)`.
- Keys: Esc → the `cancel_index` action (or `Cancelled`); Tab/BackTab traverse; Left/h and Right/l move between **enabled** actions only; `y` = first enabled Primary|Danger action (text bodies only); `n` = cancel; Enter inside the ack input moves focus forward and **never** submits; Enter on a button activates. Click outside cancels when cancelable. `Enter` never activates while an action is disabled.
- Surface is registered as a hit region, then controls re-registered on top, so clicks inside do not fall through.

### 4.6 EmptyState (`src/widgets/empty.rs`)

`title` in `t.muted()` at y0; `hint` wrapped to `max(8, area.width - 4)` in `t.faint()` at y0+2+i; `y0 = area.y + (area.height - total)/2`. No borders, no glyph.

### 4.7 TextInput (`src/widgets/input.rs`)

- `HEIGHT = 3` (:138). Label at area.x+2 (`label(focused)`); `required` appends `" *"` with the `*` repainted in accent at `area.x+2+name_w+1`; optional label appends `"  optional"` in faint only when it fits (`name_w + 2 + 2 + 8 <= width`).
- Field row = `(area.x, area.y+1, area.width, 1)` filled `field_style(s)`; `▎` gutter at field.x; text inner starts field.x+2; right reserve = 3, plus 2 more when in error (`inner.width = width - 3 - (2 if error)`).
- Scroll indicators: `…` at inner.x when scrolled left, at `inner.right()-1` when clipped right (text_muted).
- Editing → `UNDERLINED`, `underline_color(accent)`; selection → `t.selection()`; hardware cursor via `ctx.set_cursor`; error → `"!"` at `field.right()-2` in error+BOLD.
- Help row at area.y+2: error text in error_fg when invalid else help in muted. Validation: `live_validate` only re-runs while already in error; commit on focus loss; default message `"Required"`.
- Keys: shared `edit_key` (§7.4). Enter/F2 = Commit, Esc = Cancel, Tab = Commit+Tab.

### 4.8 TextArea (`src/widgets/textarea.rs`)

- height = rows+2; label at area.x+2; body = `(area.x, area.y+1, area.width, rows)`; `▎` on **every** body row; inner = (body.x+2, body.width−4).
- Editing cursor line gets border_strong `UNDERLINE` across inner; error `"!"` at body.right()−2.
- Footer on body.bottom(): help/error left, position right (`ln {l+1}/{n}` while editing, else `position_label`) in faint; scrollbar at body.right()−1.
- Keys: nav Up/k, Down/j, PageUp/PageDown always scroll; Enter/F2 begin edit; Esc commits (keeps text).

### 4.9 Select (`src/widgets/select.rs`)

- `HEIGHT = 3`; label at area.x+2; value truncated to `field.width - 5` at field.x+2; `▴` when open / `▾` when closed at `field.right()-2` in text_secondary (disabled colour when disabled); help at area.y+2.
- Popup: `h = min(options+2, 10)`, `w = field.width.clamp(12, 40)`, `Placement::Below` (flip-then-clamp); rows on surface_elevated with `▎`, `›` in accent at row.x+1 when selected, label at row.x+3.
- Closed: Up/Left = previous, Down/Right = next (**changes value without opening**). Enter/Space opens. Open: Up/k, Down/j, Enter/Space commit, Esc reverts cursor and closes. Auto-closes on blur or disable.

### 4.10 ListBox (`src/widgets/list.rs`)

- Row anatomy: `▎` at x; marker at x+1 — `›` (single, chosen) / `✓` (multi, checked) / `" "`; label `fit` at x+3.
- Meta is right-aligned at `row.right() - meta_w - 1` in text_muted and is **hidden when `label_w - (meta_w+2) < 12`** — the label is never starved.
- Marker accent when focused||hovered else text_secondary; empty list renders `empty_text` (default `"Nothing here yet"`) centred muted at `area.y + height/2`.
- The container registers `control` **first** so individual rows win hit-testing.
- Keys: Up/k/K, Down/j/J, PageUp/PageDown (viewport), Home/g, End/G, Enter/Space activate/toggle, `a` toggle-all (multi, skips disabled), Shift+Up/Down extends a range from an anchor (multi only).

### 4.11 TreeView (`src/widgets/tree.rs`)

- `indent = depth*2` (:502); disclosure at `rect.x + 1 + indent`: `▾` expanded / `▸` collapsed / `" "` leaf / `spinner_frame` while busy (accent when busy, text_secondary when has_children).
- Disclosure hit region is **2 cells wide and registered after the row** so it wins hit-testing (:522-524, 571-574).
- Label after disclosure +2; optional kind glyph occupies 2 cells in text_muted with BOLD removed; note rows are text_muted, BOLD removed, not selectable.
- Meta is all-or-nothing across visible rows (`show_meta` requires `1 + depth*2 + 2 + (2 if glyph) + label + 2 + meta + 1 <= row_w` for every visible row) and hidden when `avail - (meta_w+2) < 10`.
- First level expanded by default; API: `set_children` (lazy load), `set_busy`, `set_filter` (case-insensitive substring, ancestors kept, matching folders auto-open, scroll+cursor reset), `reveal`, `expand_all`, `collapse_all`.
- Keys: Up/k, Down/j, PageUp/PageDown, Home/g, End/G, Right/l (expand, else step in), Left/h (collapse, else step out to parent), Enter/Space (toggle folder / activate leaf), `*` expand all, `-` collapse all. Events `TreeEvent::{Expand(path), Activate(path)}`.

### 4.12 Tabs (`src/widgets/tabs.rs`)

- Two rows: baseline of `─` in border_subtle across the full width at y+1 (:249-258).
- `tab_width = 1 + label + 2 (+ prefix_w + 1) (+2 if dirty|busy|error) (+2 if closable)` (:225-237).
- Overflow reserves 3 cells each side: `" ‹ "` / `" › "` (secondary when more content, faint otherwise, lift on hover); the blank case renders `"   "` (:296-306, 405-414).
- `" + "` (3 cells) at the right edge in muted, primary + lift on hover → `TabEvent::New` (:425-436).
- Tab fg: text_primary when active||hovered else text_secondary; BOLD when active||focused; lift on hover when not active; `▎` gutter at x; prefix at x+1 in text_muted with BOLD removed.
- State slot (2 cols) at cx+1: busy → `spinner_frame(tick)` in accent; else error → `"!"` in error; else dirty → `"•"` in warning (:359-373).
- Closable: `×` (U+00D7) at cx+1 in text_faint, hover text_primary + lift; 1-cell hit region re-registered **after** the tab so it wins.
- Active underline: `━` from x+1..x+w−1 at y+1 in accent, or border_strong when `quiet` (:385-394) — exactly one accent underline per screen; secondary strips set `quiet = true`.
- Keys: Left/h, Right/l, digits 1-9 (0 excluded), Enter/Space activate, x/Delete close (closable only), `n` new (allow_new only).

### 4.13 Picker (`src/widgets/picker.rs`)

- Defaults: `width = 64`, `max_rows = 12`, `placeholder = "Type to search…"`, `empty_text = "No matches"` (:63-80). Apps override width freely (48/80/88/112).
- Render (:201+): dims `screen.height - 1` rows + clears modifiers, `begin_modal()`, `h = (2 + 1 + query_rows + rows + 2).min(screen.height - 2)` (query_rows = 2 when searchable), `w = self.width.min(screen.width - 4)`, `place(Center)` (upper third), fill surface_elevated + Rounded border(true), inner `Margin::new(2,1)`.
- Title in `t.title()` with the scope readout right-aligned in `t.muted()` on the same row.
- Query field filled `field_style(focused+editing)` with `▎` in focus; placeholder muted; query UNDERLINED accent; hardware cursor at `field.x + 2 + query_w`.
- Columns are computed over **all** items so they never shift while scrolling: `label_col = max_label.clamp(6, (row_w*45/100).max(6))`; group and tag columns likewise.
- Row: `▎`, glyph at x+1 (text_primary when focused else text_muted, BOLD removed), label from x+3 with matched BOLD, group right-aligned in text_faint on group change, tag right-aligned in text_secondary, detail at `x+3+label_col+2` in text_muted when `room >= 4`.
- Hints line at `inner.bottom()-1` in faint, e.g. `"↑↓ Move · Enter Open · Alt+Enter New tab · Tab Scope · Esc Clear / Close"`.
- Keys: Esc (clears the query first, then Cancels), Enter (Chosen, or ChosenAlt with Alt), Down/Up, Ctrl+N/J, Ctrl+P/K, PageUp/PageDown by `max_rows`, Tab = NextScope, Delete = Secondary on the cursor row, Backspace, Ctrl+U clear, plain chars append. Non-searchable pickers still move with j/k. `step` skips disabled items.

### 4.14 Progress / spinner (`src/widgets/progress.rs`)

- `SPINNER: ["⠋","⠙","⠹","⠸","⠼","⠴","⠦","⠧","⠇","⠏"]`, `spinner_frame(tick) = SPINNER[tick % 10]` (:8-12).
- `render_bar(area, label, ratio, status, bg)`: `pct = format!("{:>4}", "{n}%")` (5 cells); label only when `area.width > label_w + 8`, then `x += label_w + 2`; fixed 2-cell suffix column keeps percentages aligned — `" ✓"` Done, `" !"` Error, `" ‖"` Paused, `"  "` Active (:48-53); `track_w = right - x - (pct_w + 2)`; when `track_w < 6` only the percentage is drawn; fill `━` (Active = text_secondary, Done = success, Error = error, Paused = text_muted), track `─` in border_subtle. **Green is reserved for completion; a running bar is white 70 %.**
- `render_indeterminate`: segment `(track/5).clamp(2, 8)` of accent `━` sweeping a `─` track.
- `render_spinner(label)`: frame in accent_fg at x, label at x+2 in secondary.

### 4.15 Segments (`src/widgets/segments.rs`)

`render(area, buf, ctx, left, right, bg)`; separator 2 cells; lowest `priority` drops first (left compares `<`, right `<=`, so the right side wins ties) (:83-101); clickable segments render `" {text} "` and lift (bg=lift, fg=text_primary) on hover; non-clickable render bare; left starts at area.x+1, right ends at area.right()−1. Used for identity strips and status lines.

### 4.16 Keyhint footer (`src/widgets/keyhint.rs`)

Starts at area.x+1; each hint consumes `key_w + 1 + action_w + 2`; key in `key_hint_key()` (BOLD), action at x+key_w+1 in `key_hint_action()`; whole hints are dropped from the right when they would collide with the status; the status is right-aligned at `right - w - 1` in secondary when `width > w + 2` (reserves w+3). Optional leading badge `" {text} "` in `t.badge(kind)` then x += w+2.

### 4.17 Scrollbar (`src/widgets/scrollbar.rs`)

`TRACK = "│"`, `THUMB = "┃"` (:8-9). Draws nothing when `!overflows()`. Thumb colour `scrollbar_thumb(focused, hovered)`, track `scrollbar_track()`. Hit id = `container.sub("scrollbar")`; the whole track is clickable and draggable. `offset_for_click(track, pos, scroll)` maps a pointer y to an offset.

### 4.18 DataTable (`src/widgets/table.rs`)

- `Column{title, width: Constraint, align, editable, sortable}`; `min_width()` = Length/Min value else 6 (:53-58).
- Sorting is a permutation `order: Vec<usize>` so edits and selection survive re-sorting; `sort_by` cycles asc → desc → none and keeps the cursor on the same **source** row (:236-259). Numeric columns sort with `parse_num` (non-numeric cells first).
- Header: sorted → `t.primary()` else `t.muted()`; hovered+sortable → text_primary + UNDERLINED; suffix `" ▴"` / `" ▾"` (:599-603); only sortable headers get a hit region.
- Geometry: gutter 1 + columns + scrollbar 1; `cols_area = (area.x+3, area.y, width-5-(1 if sb), height)` (:565-570); column gap 2; horizontal overflow indicators `…` in faint at area.x+1 and `cols_area.right()+1`.
- Row: `▎` at x, `›` at x+1 when selected (accent if focused else text_secondary); any cell hover counts as row hover; `empty_text` (default `"No rows"`) centred muted.
- Cell cursor (cell_nav) = `fg(canvas).bg(text_primary)+BOLD`, bg(error) with fg(text_primary) on cell error; hover on an editable cell in cell_nav → UNDERLINED border_strong; cell error → fg(error) and `"!"` at right−1 BOLD.
- Inline edit: `field_style(editing)` fill + UNDERLINED accent (error colour when invalid) + `"!"` + hardware cursor; Tab moves to the next editable cell and emits `LeaveForward`/`LeaveBackward` at the ends; focus loss commits; clicking the already-current editable cell starts editing (double-click emulation); header click commits then sorts.
- Events: `Committed{row,col}`, `Cancelled`, `Activated(row)`, `LeaveForward`, `LeaveBackward`.

### 4.19 CodeEditor (`src/widgets/code.rs`)

- Gutter: `num_w = max(2, digits(line_count))`, `gutter_w = 1 + 1 + num_w + 1 + 1` (:633-635); `▎` at area.x **only on the cursor line**; marker at area.x+1: `" "` / running spinner (accent) / `"›"` for the current block (accent when focused else text_secondary) / `"!"` for a diagnostic (error|warning + BOLD); line number right-fitted at area.x+3 (primary+BOLD on the focused cursor line, secondary inside the running block, muted otherwise, faint when read_only); text at area.x+gutter_w.
- body_h = height−1; `…` at text_area.x when hscrolled and at the right edge when clipped.
- Styling: per-char `fs.patch(t.syntax(tone_at(off)))`; selection bg(popover); find current match bg(popover), other matches UNDERLINED border_strong; diagnostics UNDERLINED error|warning; bracket pair BOLD|UNDERLINED border_strong; editing cursor line UNDERLINED border_strong; placeholder muted.
- Footer: find bar (`"find "` muted + needle UNDERLINED accent while editing + `{cur+1}/{n}` or `"no matches"`) or the nearest diagnostic truncated to `width - pos_w - 3` (min 8); position right `ln {l+1}/{n} · col {c+1}` in faint.
- Keys: Enter/i edit, `a` edit + move right, j/k, Left/h hscroll−8, Right/l hscroll+8, PageUp/PageDown, Home/g, End/G, `{`/`}` jump between blocks, `/` find, `n`/`N` next/prev match, Esc closes find; while editing Tab indents (2 spaces) / BackTab dedents, or commits+leaves when `tab_leaves`; wheel horizontal = hscroll ± 4·delta. Find is case-sensitive iff the needle contains an uppercase letter.
- Extension points: `Highlighter = fn(&str) -> Vec<(Range<usize>, SyntaxTone)>`, `Segmenter = fn(&str) -> Vec<Range<usize>>` — the language lives entirely in the caller.

### 4.20 DataGrid (`src/widgets/grid.rs`)

- Row anatomy: `▎` focus gutter, `✓` selection, change slot, row number, cells. `num_w = max(2, digits(rows.len()))`, `gutter_w = 3 + num_w + 1`; `cols_area.x = grid_area.x + gutter_w`, `cols_area.width = grid_area.width - gutter_w - 4 - (1 if sb)` (:1545-1559).
- Change glyphs at x+2: Modified `"•"` warning, Inserted `"+"` text_secondary, Deleted `"−"` text_muted, Error `"!"` error+BOLD, Clean `" "` (:1739-1751).
- Row number right-fitted at x+3 (text_secondary when the row is focused else text_faint); selected `"✓"` at x+1 (accent if focused else text_secondary); a deleted row is fg(text_faint)+CROSSED_OUT.
- Cells: `NULL`/`DEFAULT` text_muted+ITALIC; empty text `''` faint; primary column text_secondary when the grid is not focused; selection range bg(popover); dirty fg(warning) −ITALIC; cell error fg(error) −ITALIC; cursor = `fg(canvas).bg(text_primary)+BOLD` (bg error + fg text_primary on error; fg text_muted on a deleted row); hover on an editable cell → UNDERLINED border_strong; FK reference `"→"` at right−1 (canvas when it is the cursor else muted), clickable → `FollowReference`.
- Header: `"▪ "` prefix (overdrawn by `"⚷"` at the same cell for primary keys, so the rendered glyph is `⚷`), name, `" ∇"` when filtered, `" ▴"`/`" ▾"` when sorted; `"⚷"` is drawn separately at rect.x (or at right for right-aligned columns) in text_faint; header text_primary when sorted||filtered||(cursor column && focused) else muted; hovered+sortable → text_primary + UNDERLINED border_strong.
- Horizontal overflow: `‹{offset}` at grid_area.x+1 and `{hidden}›` at cols_area.right()+1 (faint; primary + lift on hover, clickable).
- Fetch-more row: `"{spinner} fetching…"` or `"↓ {n} loaded · Enter fetches more"` in muted at x+gutter_w; loading empty state = `render_spinner("Loading rows…")` at body.x+3, body.y+height/2, otherwise an `EmptyState`.
- Column widths: p95 of the first 200 rows clamped to `[min(max,min_w), max(max_w, header)]` with `header = width(name) + (2 if primary) + 2`; `fit_header_marks` widens sorted/filtered columns.
- Pending bar (2 reserved rows): `"• {n} pending"` in `t.primary().fg(t.warning)` at area.x+1; detail `"· {msg}"` in error_fg when the cursor row has a row error, else `"· {pending_label}"` muted; buttons right-aligned with gap 1 — `Preview SQL` (subtle), `Discard` (subtle), `Save` (primary) (:399-404, 1886-1923). `pending_label()` = `"N update(s) · N insert(s) · N delete(s)"`.
- `position_label()` = `"rows A–B of T · cols C–D of N"`; totals `"N loaded · M total"` / `"N loaded · ~M total"` / `"~M"`.
- Keys: Up/Down/k/j/K/J (Shift extends the range), Ctrl+Left/Right column page, Left/h/H, Right/l/L, Ctrl+Home/Ctrl+End row, Home/End column, PageUp/PageDown, g/G, Enter/F2 edit (Bool cycles true→false→NULL→true; Json opens the viewer; Text longer than 2× column width opens the viewer), Space row select, Esc clears range+selection, Delete/Backspace sets NULL (or reports `"{col} is NOT NULL"`) or deletes selected rows, `-` toggle delete, `+` insert row, Ctrl+D duplicate, y/Y copy, Ctrl+S CommitRequested, `s` sort, `S` clear sort, `f` FilterOnCell, `/` OpenFilters, `F` ClearFilters, `r`/F5 Refresh, `u` undo, `U` DiscardRequested, `p` PreviewSql, Ctrl+] FollowReference. With 0 rows only `+`, `r`/F5, `f`, `F` are accepted.
- `CellValue::Num` renders `format!("{:.2}")`; `cell_text` sanitises `\n`→`↵`, `\t`→`⇥`, other control chars→`·`, caps at 10 000 chars; Id kind uses `truncate_middle`; Json narrower than 8 cells collapses to `"[…]"`/`"{…}"`.

### 4.21 Props (`src/widgets/props.rs`)

`Prop{label, value, tone, wrap}`; `render` returns the rows used; `label_w = max label width + 2`; labels in `t.muted().bg(bg)`; value at x+label_w in `fg(t.tone(tone))`; wrap uses `ui::text::wrap(vw.max(4))`.

### 4.22 ScrollPanel (`src/widgets/panel.rs:188+`)

Keys Up/k −1, Down/j +1, PageUp/PageDown, Home/g jump_start, End/G jump_end + `follow = true`, `f` toggles follow (jump_end on enable); any manual offset change clears follow. `text_w = area.width - 2`; scrollbar at area.right()−1; takes `style_line: fn(&Theme, &str) -> Style` for per-line colouring.

---

## 5. Composition conventions

### 5.1 Header (identity strip)

Showcase (`src/bin/showcase/app.rs:802-841`), all on row 0: `"▪"` in accent_fg at x+1; `x += 2`; product name in `title()`; `x += 6`; product subtitle in secondary; breadcrumb `"/ {section} / {label}"` in muted; right side built right-to-left — `" ? Help "`, `" i Inspector "` / `" i Inspector · on "` (1-cell gap between them, hover = `t.primary().bg(t.surface)`), then the capability readout `"{level.label()} · {w}×{h}"` in faint, drawn only when `rx > x + crumb_w + cw + 2`.

TablePro (`src/bin/tablepro/app.rs:2188-2281`) uses `segments::render` on row 0 with left = `▪` (Success, p9), `TablePro` (Normal, bold, p9), screen segment, connection name `truncate_middle(name, 18)` clickable p9, environment `◆ production` / `◇ staging` / `development` / `local` p8, scope `"{db} › {schema}"` clickable p7, safe-mode token clickable p8, and right = `"{spinner} running"` p9 / `"• {n} pending"` (Warning, p8), capability readout (Faint, p1), `"? help"` (Muted, p4, clickable).

### 5.2 Footer / key hints

Row `h-1`. Priority ladder (showcase `app.rs:998-1022`): dialog+editing → `Enter Confirm`, `Esc Cancel`; dialog → `← → Choose`, `Enter Confirm`, `Esc Cancel` (+ `y / n Quick answer` for text bodies); nav focused → `↑ ↓ Move`, `Enter Open`, `Tab Into page`, `q Quit`; else the page's own hints + `Tab Next` when not editing. The ` EDIT ` badge is inserted **before** the hints when the page is editing and no dialog is open. Right reserve = status width + 3, or **14 cells** when there is no status. Status expiry: showcase 4 s, TablePro 5 s.

### 5.3 Layering

Order per frame: page → drawers/side panes → popovers (select, completion) → picker/modal dialogs → filter editor. Every modal calls `begin_modal()` which sets a focus barrier; popovers register `push_barrier()` in the hit registry so earlier (lower) regions stop resolving. Clicking outside a dialog/picker cancels it. Dialogs dim with `t.backdrop` + modifier clearing over `screen.height - 1` rows, leaving the footer readable.

### 5.4 Table chrome

Header row 0 (`muted`, sorted column `primary` + `▴`/`▾`), body from row 1, gutter `▎` col 0, selection marker `›` col 1, columns from col 3 with gap 2, `…` overflow markers at both horizontal edges, scrollbar in the last column.

### 5.5 Grid chrome

Gutter `3 + num_w + 1` with focus bar, `✓`, change glyph and row number; header with `⚠`-free `▪`/`⚷` primary mark, `∇` filter and `▴`/`▾` sort; `‹n`/`n›` h-scroll edges; `↓ {n} loaded · Enter fetches more` fetch row; 2-row pending bar with `• {n} pending`, detail, and `Preview SQL / Discard / Save`.

### 5.6 Picker / completion / select popups

Picker = modal (dims + `begin_modal`, centred upper third). Completion = `Placement::Below` anchored, flips above, clamps. Select = `Placement::Below`, `clamp(12,40)` wide, ≤10 rows. All three use `surface_elevated` + `Rounded` + `border(true)`.

---

## 6. Showcase and TablePro application conventions

### 6.1 Shared shell rules (both apps)

- `MIN_WIDTH 72`, `MIN_HEIGHT 20`; below that a centred 4-line notice: product name (`title()`), `"Terminal too small"` (secondary), `"Need 72×20, have {w}×{h}"` (muted), `"q Quit"` (faint, dropped one extra row). Only `q` works.
- Tick 80 ms when animating else 400 ms; press flash 140 ms; `Interaction.pressed(id)` = (pressed==id && hover==id) || flash==id.
- Fresh `HitRegistry` + `FocusRing` every frame; after render focus is re-validated (no modal → snap to `ring.first()` when stale; modal → `ensure_valid`).
- Wheel delta is **3 rows** per event (showcase `app.rs:655-667`, TablePro `app.rs:2025-2028`); horizontal wheel ignored.
- Hover is suppressed after any key press until the pointer moves.

### 6.2 Showcase (`src/bin/showcase/`)

- Layout (`app.rs:713-754`): header row 0, blank row 1, body rows 2..h-2, blank, footer last. Sidebar 19 cols, or **24 when width ≥ 110**; gap 2; inspector 30 cols when toggled with `i` **and** width ≥ 100 (2-col gaps on both sides).
- Sidebar: section label at x+3 in faint; rows fill `t.row(s, bg)`; `▎` gutter, `›` at x+1 for the current page, label at x+3 truncated to width−4; compact mode below `NAV_ENTRIES.len() + sections*2 - 1` rows (drops section labels and blank rows).
- 20 nav entries in 3 sections (`Foundations`, `Components`, `Screens`) — `NAV_ENTRIES` order, not the `PageId` enum order.
- Keys: global `Tab`/`BackTab`, `q`, `?` help, `i` inspector, `[`/`]` prev/next page, `0` jump to nav, `Esc` returns focus to nav (Consumed when already there). Ctrl+C always quits. Enter/Space on a non-editing `Changed` outcome sets the 140 ms flash.
- Help dialog = mutated `Dialog::confirm` (primary removed, remaining action demoted to Secondary, `cancel_index = Some(0)`, **width 70**).
- Page trait: `title`, `blurb`, `render`, `handle(PageEvent)`, `hints(focus)`, `editing()`, `animating()`. Page title at `area.x/area.y` in `title()`, blurb at `x+title_w+2` in muted (only when `width > title_w + 4`), body from `y+2`.
- Split helpers: `rows(area, &[u16])` fixed heights with the last taking the remainder; `columns(area, left_w, gap)` stacks vertically when `width < left_w + gap + 20`.
- Page split ratios: editor 62 % (min 40), trees 60 % (min 30), chips props 55 % (min 36), forms/inputs/textareas/settings `width/2 - 2` with gap 4, panels `width/2 - 1` with gap 2, taskrunner left 30.
- Simulated durations: buttons busy 2200 ms, forms busy 1800 ms, grid commit 4 ticks, editor run 10 ticks (`40 + (runs*37) % 90` ms), progress +0.006/tick, taskrunner speed `0.012 + (i%3)*0.006`, log every 9 ticks, ≤2 concurrent tasks.
- Determinism: `showcase_visual_baseline` renders 20 pages × {(120,40),(80,24)} with one Tab press and FNV-1a hashes every cell **except the sidebar** (`tests/showcase_baseline.txt`, 40 lines). Live capture: `tools/capture.sh` drives the binary in tmux and emits `shots/<name>.{ansi,txt,html,png,cursor}`.

### 6.3 TablePro (`src/bin/tablepro/`)

- Layout (`app.rs:2145-2152`): identity strip row 0, blank row 1, body rows y+2..bottom-2 **inset 1 column each side**, blank, footer. Overlays are drawn over the whole area.
- Workbench (`workbench.rs:1213-1311`): tab strip 2 rows; body from y+2. Explorer width `(body.width/4).clamp(28, 40)`; gap to main 1 col. Below **100 columns** the explorer becomes a drawer: it covers the whole body when focused (or `explorer_visible`), otherwise it collapses to a zero rect that stays a focus stop so Tab can bring it back. `Ctrl+B` toggles, `0` forces it open, `z` zooms the focused pane.
- Tab types: Table (Data/Structure mode tabs, `quiet = true`, body from y+3), Query (`Split::new(38, 4, 6)` editor/results, result tab strip 2 rows, status line, `EXPLAIN` tree + detail card at ≥110 cols), History (`Split::new(50,30,30)`, toolbar with search + `scope: … · status: …` readout, body from y+3).
- Table status line (`tabs.rs:791-838`): parts `sort {col} ▴|▾` (p4), `filtered ({n})` (p4), `rows …` (p5), `cols …` (p2), `read-only: {reason}` (p3), joined `" · "`, lowest priority dropped while too wide, drawn at body.x+1 in muted.
- Structure tab: sub-tabs Columns/Indexes/Foreign keys/Constraints/Triggers/DDL (`quiet = true`); DDL is a card whose `CREATE`/`COMMENT` lines are primary+BOLD, `CONSTRAINT` lines secondary, rest primary.
- Grid integration: read-only reasons `"These rows come from a view, which cannot be edited."` / `"This table has no primary key, so rows cannot be identified."`; `ROW_CAP = 500`; `FetchMore` → `"Fetch more: the demo engine caps results at 500 rows"`.
- Filter editor overlay: 64×15 clamped, `Placement::Center`, `Margin::new(3,1)`, `Split::new(50,16,16)` column/operator row, optional between-values row, live `"WHERE {preview}"` readout, `row_layout_right(..., 1)` buttons `Cancel` / `Update filter`|`Add filter`.
- Dialogs: defaults 54/66 plus app overrides — safety facts **74**, commit facts **78**, SQL preview **90**, JSON viewer **80**, help **78**, quit/close/discard destructive 54. Typed acknowledgement token = statement target when `deliberate || (dangerous && Production)`, else the table name when the level requires authentication.
- Esc ladder (TablePro `app.rs:779-812`): maximized → unmaximize; tab strip focused → explorer; explorer focused → clear its filter text; else → tab strip. Esc pre-empts everything to cancel a running query. Ctrl+C cancels a running query before offering quit.
- Chords: Ctrl+R/F5 run, Alt+R run all, Ctrl+X/Alt+X EXPLAIN(+ANALYZE), Ctrl+T new query, Ctrl+W close, Ctrl+O/Ctrl+P Open Quickly, Ctrl+G tab list, Ctrl+Y history, Ctrl+B explorer, Ctrl+L Safe Mode, Ctrl+D Data/Structure, Ctrl+S save pending changes, Ctrl+F filter/find, `z` zoom, `[`/`]` switch tabs, Ctrl+Up/Down grow the split by ±8.
- Status strings are quoted verbatim in the extraction appendix of this campaign (e.g. `"Saved {n} change{s} to {qualified}"`, `"Safety level set to {label} · saved to the connection"`).

---

## 7. Interaction kernel

### 7.1 Focus ring (`src/core/focus.rs`)

`FocusRing{order: Vec<WidgetId>, barrier: Option<usize>}`; `register` (append), `push_barrier` (everything registered after the barrier is unreachable while the barrier is active — modals), `reachable`, `next`/`prev` wrap around, `Focus::ensure_valid` snaps to the first entry when the stored id is stale. **Render order is Tab order**: each widget registers during `render`, so the ring is rebuilt every frame and traversal follows the visual order.

### 7.2 Hit testing (`src/core/hit.rs`)

`HitRegion{id, area, scroll_only}`; `hit()` skips `scroll_only` regions and iterates `.rev()` so the **last registered (topmost) wins**; `hit_scroll()` includes scroll_only. `push_barrier()` shadows everything registered earlier. Empty areas are never registered. Widgets that must win over their container register **after** it (tree disclosure, tab close, table cells).

### 7.3 Widget identity (`src/core/id.rs`)

`WidgetId(u64)` = FNV-1a over a path string. `WidgetId::of("app.nav")`, `.child(i)` for row/cell indices, `.sub("header")`/`.sub("scrollbar")` for sub-regions. Ids are stable across frames, which is what makes hover/pressed/flash state survive a rebuild.

### 7.4 Event flow (`src/core/event.rs`)

`Outcome::{Ignored, Consumed, Changed}` with `or()` (Changed dominates). `Input::{Key, Mouse, Resize, Paste, Tick}`. `Key{code, mods}`; `plain()` = mods minus SHIFT is empty; keys are accepted for Press|Repeat only. Mouse is left-button only: Move/Down/Up/Drag/Wheel{Up,Down,Left,Right}. Dispatch order in an app: modal (dialog/picker/filter) → focused control → page/screen → global keys → Tab/BackTab traversal.

### 7.5 Shared text-editing keymap (`src/widgets/field_common.rs`)

`edit_key(key, multiline)` returns `EditAction`:

| Input | Action |
|---|---|
| Esc | Cancel |
| Enter (multiline, plain) | Insert '\n' |
| Enter | Commit |
| Tab / BackTab | `Tab{backward}` |
| Ctrl\|Alt + Left/Right | word move (with SHIFT: select) |
| Left / Right | char move |
| Up / Down | only when multiline |
| Ctrl+Home / Ctrl+End | doc start / end |
| Home / End | line start / end |
| Ctrl\|Alt+Backspace | delete_word_left |
| Delete | delete forward |
| Ctrl+A / Ctrl+E | home / end |
| Ctrl+U / Ctrl+K | delete to line start / end |
| Ctrl+W | delete_word_left |
| Ctrl+L | select_all |
| Alt+B / Alt+F | word left / right |
| other Ctrl/Alt + char | None (app may take it) |
| plain char | Insert |

This one keymap is used by TextInput, TextArea, DataTable inline edit, DataGrid cell edit and the code editor.

### 7.6 Text model (`src/core/text.rs`)

`TextBuffer{text, cursor: byte offset, anchor, multiline}`; grapheme-aware cursor movement, word boundaries via `is_alphanumeric`, unicode-width display columns (wide char = 2), `move_up/down` preserve the display column, `insert_char` rejects '\n' when !multiline, `insert_str` filters \n/\r when !multiline, `pos_of` (col = display width), `offset_at(line, col)`, `select_range`/`selection_lines`/`select_all`, `delete_word_left`, `delete_to_line_{start,end}`. `set_text` puts the cursor at the end.

### 7.7 Scroll model (`src/core/scroll.rs`)

`ScrollState{offset, content_len, viewport_len}` is a pure model; the scrollbar derives its geometry from it. `page_up/down` = ±`viewport.max(1)`; `ensure_visible` makes the minimal move; `thumb(track)` = `(start, len)` with `len = max(1, viewport*track/content)` and `start = ((offset*(track-len)) + max_off/2) / max_off`; `offset_for_track_pos` is the inverse for track clicks/drag. The code editor keeps a second horizontal `ScrollState` stepped by 8.

### 7.8 Runtime (`src/runtime.rs`)

`Application{handle(Input)->Outcome, render(&mut Frame), should_quit(), tick_interval()}`. `run()` = `ratatui::init` + `EnableMouseCapture` + `EnableBracketedPaste`, coalesces input floods with an inner `event::poll(Duration::ZERO)` loop, redraws only on `Changed`, and restores the terminal even on error.

---

## 8. DESIGN.md vs source conflicts (source + shots win)

1. **Row anatomy is not universal.** DESIGN.md:429 "universal: `▎` at column 0, marker at column 1, content from column 3" holds for list/tree/table/grid rows, but **buttons (`button.rs:143-144`) and chips (`chips.rs:198-199`) put content at column 1 with no marker slot**.
2. **Input surface hex in the module doc.** `theme.rs:5-6` prose claims the input surface is `#27272a` (`--color-input`), but `INPUT = #1e1e22` (:70) and `#27272a` is `OVERLAY` (:72). The DESIGN.md front-matter (:21-22) agrees with the constants; the doc comment is stale.
3. **Middle truncation keeps both ends.** DESIGN.md:354/378 reads as if the tail is kept; `truncate_middle` uses `keep_end = (max-1)/3` and keeps head **and** tail (`ui/text.rs:33-64`).
4. **"pressed = reversed" is not universal.** DESIGN.md:296 states the pressed cell is reversed; that is literally true only for Secondary/Toggle/Subtle. Primary pressed = `accent_pressed` bg with on-accent text (`theme.rs:379-385`); Danger pressed = `fg(text_primary).bg(error)` (:425-427). DESIGN.md:669-670 states this correctly — the state table is the imprecise one.
5. **Tab close glyph is `×` (U+00D7), not `✕`.** DESIGN.md:761 writes `✕`; source uses `"×"` (`tabs.rs:382`) and shots confirm.
6. **Grid primary-key mark renders as `⚷`.** The header writes `"▪ "` as part of the title and then overdraws `"⚷"` at the same cell for primary columns (`grid.rs:1610, 1625`), so the rendered header is `⚷ id` (see `shots/s_grid.txt`). DESIGN.md:552/799 describes both glyphs without saying one wins.
7. **Completion row has no literal `…` between label and detail.** DESIGN.md:840 shows `▎ kind label … detail`; source right-aligns the detail with no ellipsis (`completion.rs`), and the detail is hidden when `avail <= label+detail+2`.
8. **Undocumented geometry.** Picker defaults `width = 64`, `max_rows = 12` (`picker.rs:72-73`); dialog height formula `2+1+1+1+body+1+1+1` (`dialog.rs:186`); grid `gutter_w = 3 + num_w + 1` with a 4-column right reserve (`grid.rs:1551-1558`); editor `gutter_w = 3 + num_w + 1`, `num_w >= 2` (`code.rs:633-635`); code-editor horizontal step of 8; grid `‹n` / `n›` edge markers (`grid.rs:1570, 1636`); the `⚷` overdraw above; footer's 14-cell default right reserve (showcase `app.rs:1033`); inspector's fixed 12-column value indent (showcase `app.rs:977-985`); the header capability readout's 2-cell clearance rule (showcase `app.rs:838`); the min-size notice's extra blank row before `q Quit`.
9. **Sidebar compact threshold is derived, not fixed at 25.** DESIGN.md:401 states 25 rows; source computes `NAV_ENTRIES.len() + sections*2 - 1`, so it moves as pages are added (showcase `app.rs:849-850`).
10. **Page-level split ratios are undocumented**: editor 62 %, trees 60 %, chips props 55 %, forms/inputs/textareas/settings `width/2 - 2` gap 4, panels `width/2 - 1` gap 2, taskrunner left 30, TablePro explorer `(width/4).clamp(28,40)`, connections `(width/3).clamp(26,40)`.
11. **Dialog width is a default, not a constant.** DESIGN.md documents 54/66; apps override freely (TablePro uses 74/78/80/90, showcase help 70, pickers 48/80/88/112).
12. **Status expiry differs per app**: showcase 4 s, TablePro 5 s — DESIGN.md does not distinguish.
13. **Wheel step**: DESIGN.md:479 "three rows" is implemented in the apps (±3), not in the library; widgets take a raw `delta: i32`.
14. **Chips vs tabs `×` idle colour differs**: chips use text_muted, tabs use text_faint. Preserve as-is for fidelity.
15. **Grid hint string style**: the data-grid page writes `↑↓←→` with no spaces while every other page uses `↑ ↓ ← →` (showcase `grid.rs:391`). Cosmetic inconsistency, but it is in the rendered output.
16. **`animating()` on the Progress page is hard `true`**, so the app ticks at 80 ms whenever that page is visible (`progress.rs:197-199`) — DESIGN.md mentions 80 ms only as card meta text.

Verified as **consistent** (non-conflicts): field-height 3; select popup 12-40 / ≤10 rows; completion 24-48 / ≤8 rows; dialog widths 54/66 and `Margin::new(3,2)`; the lift ladder; backdrop rules; spinner frames and the 10-frame list; `│`/`┃` scrollbar; `‹ ›` tab overflow; sidebar 19/24 + inspector 30; min 72×20 notice; ` EDIT ` badge leading; hint anatomy and right-dropping; `0`/`[`/`]`/`?`/`q` semantics.

---

## Appendix A — Deterministic renders read

`shots/`: f_overview, f_buttons_hover, f_dialog_delete, f_forms, f_inputs_edit, f_lists_hover, f_panels, f_progress, f_scrolling, f_sidebars, f_tables_hover, f_80x24_taskrunner, s_chips, s_editor, s_grid, s_grid_pending, s_grid_failed, s_grid_preview, s_pickers, s_pickers_tabs, t_connections, t_orders, t_workbench, t_80_drawer, t_dirty, t_editing_cell, t_error, t_history, t_safemode, t_danger, t_switcher. `tests/showcase_baseline.txt`: 40 lines of `<W>x<H> <PageName> <16-hex-hash>` for 20 pages × {(120,40),(80,24)}; FNV-1a offset basis `0xcbf29ce484222325`, prime `0x100000001b3`, digest input `"{symbol}|{fg:?}|{bg:?}|{modifier:?};"` per cell, sidebar excluded.

Key rendered confirmations: header ` ▪ Junie Design system / Foundations / Overview` with right side `truecolor · 120×40   i Inspector   ? Help`; footer `↑↓←→ Cell  Enter Edit  s Sort  Space Select row  + - Insert / delete  u Undo  Tab Next`; editing footer leads with the ` EDIT ` badge; TablePro strip ` ▪  TablePro  Production  ◆ production  acme_prod › public  safe`; explorer frame `╭─ Explorer ─── 6 ─╮`; mode tabs `▎Data ▎Structure` with a border-strong rule; drawer at 80 columns covers the whole body; the 80×24 sidebar drops section labels and blank rows.
