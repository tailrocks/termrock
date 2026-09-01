# shadcn/ui → TermRock TUI coverage matrix

**Source of shadcn names:** [ui.shadcn.com/docs/components](https://ui.shadcn.com/docs/components)
(All Components index, 2026-08 crawl; 64 named components including New: Questionnaire).

**TermRock inventory:** `crates/termrock/src/widgets/*`, `patterns/*`, `registry/catalog.rs`.

**Statuses:** `covered` | `partial` | `missing` | `N/A`

**Second-pass notes**

- Prefer mapping peers over dual APIs with the same terminal job.
- Web-only geometry/DOM: **Aspect Ratio**, **Direction**, **Native Select** → N/A.
- Hover-only web chrome must map to keyboard/focus paths in TUI.
- Ports closed this pass: **Input OTP**, **Carousel**, **Input Group** (migration 0247).
- Residual partials: Hover Card (PreviewCard/Popover), Item (ComposedRow/List), Typography (Heading/content vs full type scale), Bubble (message_thread chrome).
- **Marker** is **N/A** (map-pin geo UX), not partial — status glyphs/`SemanticStatus` cover non-map markers.

| # | shadcn | Status | TermRock surface(s) | Notes |
|---|--------|--------|---------------------|-------|
| 1 | Accordion | covered | `Accordion` | Expandable sections |
| 2 | Alert | covered | `Callout`, `ErrorState` | Callout for banners; ErrorState for failures |
| 3 | Alert Dialog | covered | `AlertDialog` | Confirm gates, typed phrase |
| 4 | Aspect Ratio | N/A | none | Continuous CSS aspect ratio has no honest cell geometry |
| 5 | Attachment | covered | `AttachmentChips` | File/attachment chips |
| 6 | Avatar | covered | `AvatarGlyph`, `Identity` | Glyph/initials + presence (no raster required) |
| 7 | Badge | covered | `Badge`, `TagChip` | Status/count badges |
| 8 | Breadcrumb | covered | `Breadcrumbs` | Path trail + edit |
| 9 | Bubble | partial | `MessageThread`, chat cards | Residual: dedicated chat-bubble recipe API |
| 10 | Button | covered | `Button`, primitives | Actions |
| 11 | Button Group | covered | `ButtonGroup` | Grouped actions |
| 12 | Calendar | covered | `DateTimePicker` calendar surface | Month grid + range |
| 13 | Card | covered | `Card`, `Surface` | Chrome containers |
| 14 | Carousel | covered | **`Carousel`** (0247) | Keyboard prev/next, indicators, auto tick |
| 15 | Chart | covered | `charts` family | Sparkline, gauge, histogram, etc. |
| 16 | Checkbox | covered | `Checkbox` | Form checked state |
| 17 | Collapsible | covered | `Collapsible`, `Section` | Disclosure |
| 18 | Combobox | covered | `Combobox` | Searchable select |
| 19 | Command | covered | `CommandPalette`, `QuickOpen` | Command menu |
| 20 | Context Menu | covered | `DropdownMenu` | Keyboard/context menus |
| 21 | Data Table | covered | `DataTable`, `DataView`, `ResultGrid` | Virtualized tables |
| 22 | Date Picker | covered | `DateTimePicker` | Date/time/range |
| 23 | Dialog | covered | `Dialog`, overlay stack | Modal chrome |
| 24 | Direction | N/A | none | Document LTR/RTL is web layout; terminal is host locale |
| 25 | Drawer | covered | `Drawer` | Edge panels |
| 26 | Dropdown Menu | covered | `DropdownMenu` | Menus |
| 27 | Empty | covered | `EmptyState` | Empty collections |
| 28 | Field | covered | `Field`, `Fieldset`, `Form` | Label+control+error |
| 29 | Hover Card | partial | `PreviewCard`, `Popover`, `Tooltip` | Residual: no separate HoverCard type; use PreviewCard + focus |
| 30 | Input | covered | `TextInput` | Single-line entry |
| 31 | Input Group | covered | **`InputGroup`** (0247) | Multi-addon + actions; simple chrome → `TextInput` prefix/suffix |
| 32 | Input OTP | covered | **`InputOtp`** (0247) | Fixed slot PIN/OTP |
| 33 | Item | partial | `ComposedRow`, `ListRow` | Residual: no shadcn-named Item type; composed rows cover job |
| 34 | Kbd | covered | `Kbd` | Keycap chrome |
| 35 | Label | covered | `Label` | Field labels |
| 36 | Marker | N/A | status glyphs / `SemanticStatus` | Map-marker UX is geo/web; use status markers |
| 37 | Menubar | covered | `MenuBar` | Top menus |
| 38 | Message | covered | `MessageThread`, agent cards | Chat messages |
| 39 | Message Scroller | covered | `Transcript`, `ScrollArea`, virtual lists | Message virtualization |
| 40 | Native Select | N/A | `Select` | Browser `<select>`; use TermRock Select |
| 41 | Navigation Menu | covered | `NavigationList`, `Sidebar` | Site nav peer |
| 42 | Pagination | covered | `Pagination` | Page chrome |
| 43 | Popover | covered | `Popover` | Anchored overlay |
| 44 | Progress | covered | `ProgressBar` | Determinate/indeterminate |
| 45 | Questionnaire | covered | `QuestionFlow` | Multi-question HITL |
| 46 | Radio Group | covered | `RadioGroup` | Exclusive options |
| 47 | Resizable | covered | `ResizablePanelGroup`, `SplitPane` | Split resize |
| 48 | Scroll Area | covered | `ScrollArea` | Scroll viewport |
| 49 | Select | covered | `Select` | Option list |
| 50 | Separator | covered | `Separator` | Dividers |
| 51 | Sheet | covered | `Drawer` with bottom placement | Edge sheet |
| 52 | Sidebar | covered | `Sidebar` | App rail |
| 53 | Skeleton | covered | `Skeleton` | Loading placeholders |
| 54 | Slider | covered | `Slider` | Range control |
| 55 | Spinner | covered | `Spinner` | Busy indicator |
| 56 | Switch | covered | `Switch` | On/off preference |
| 57 | Table | covered | `Table`, `DetailTable` | Grid tables |
| 58 | Tabs | covered | `Tabs` | Tab panels |
| 59 | Textarea | covered | `TextArea` | Multiline |
| 60 | Toast | covered | `Toast`, `NotificationCenter` | Ephemeral notices |
| 61 | Toggle | covered | `Toggle` | Sticky press |
| 62 | Toggle Group | covered | `ToggleGroup`, `SegmentedControl` | Tool mode groups |
| 63 | Tooltip | covered | `Tooltip` | Focus/hover hints |
| 64 | Typography | partial | `Heading`, `content`, `MarkdownView` | Residual: no full type-scale token pack named Typography |

## Counts (this revision)

| Status | Count |
|--------|------:|
| covered | 56 |
| partial | 4 |
| missing | 0 |
| N/A | 4 |
| **Total** | **64** |

## Port decisions (0247)

| Gap | Decision |
|-----|----------|
| Input OTP | Ship `InputOtp` — 2FA/PIN is a real TUI job |
| Carousel | Ship `Carousel` — keyboard slides without Embla |
| Input Group | Ship `InputGroup` — prefix/suffix around TextInput |
| Hover Card | Keep partial → PreviewCard + Popover (keyboard focus path exists) |
| Item | Keep partial → ComposedRow/ListRow |
| Bubble | Keep partial → MessageThread |
| Typography | Keep partial → Heading/Markdown |

## Rejected ports

- Aspect Ratio, Direction, Native Select, map Marker — not TUI-honest without theater.
- Full CSS type scale as “Typography” widget — prefer markdown + heading roles.

## Validation

```bash
rtk cargo test -p termrock --lib input_otp
rtk cargo test -p termrock --lib carousel
rtk cargo test -p termrock --lib input_group
rtk cargo check -p termrock
```
