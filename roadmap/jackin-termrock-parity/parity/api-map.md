# Jackin old-to-HEAD API parity map

This R2 map resolves every inventory API row against TermRock HEAD.

- Jackin: `9e211559` (clean tree)
- TermRock: `e68ed5a6`
- Captured: 2026-08-17
- Recount: 1,067 matching lines across 134 Rust files.
- Resolution: current public API first; then ordered migration evidence; then current source fallback.
- GAP result: no gaps found. Every old capability remains public or has an explicit forward replacement.

## Theme

| Jackin usage (old) | Current HEAD API | Evidence | Status |
|---|---|---|---|
| `termrock::Theme` | `termrock::style::RolePalette` | `migrations/0060-v0.13.0-root-reexport-purge.md`; `migrations/0061-v0.13.0-design-system-sole-paint.md:7`; `crates/termrock/src/style/mod.rs:355` | RENAMED |
| `Theme::default()` | `RolePalette::default()` or `RolePalette::tailrocks_phosphor()` | `docs/api/public-api.txt:30920` | RENAMED |
| `Theme::with_role` | `RolePalette::with_role` | `docs/api/public-api.txt:30921` | RENAMED |

## `style`

| Jackin usage (old) | Current HEAD API | Evidence | Status |
|---|---|---|---|
| `termrock::style::Role` | `termrock::style::Role` | `docs/api/public-api.txt:11071` | SAME |
| `termrock::style::faded` | `termrock::style::faded` | `docs/api/public-api.txt:31139` | SAME |

## `scroll`

| Jackin usage (old) | Current HEAD API | Evidence | Status |
|---|---|---|---|
| `termrock::scroll::DialogScroll` | `termrock::scroll::DialogScroll` | `docs/api/public-api.txt:9156` | SAME |
| `termrock::scroll::ScrollAxes` | `termrock::scroll::ScrollAxes` | `docs/api/public-api.txt:2193` | SAME |
| `termrock::scroll::ScrollAxis` | `termrock::scroll::ScrollAxis` | `docs/api/public-api.txt:28280` | SAME |
| `termrock::scroll::ScrollDelta` | `termrock::scroll::ScrollDelta` | `docs/api/public-api.txt:28566` | SAME |
| `termrock::scroll::ScrollbarGeometry` | `termrock::scroll::ScrollbarGeometry` | `docs/api/public-api.txt:28656` | SAME |
| `termrock::scroll::ScrollbarSpec` | `termrock::scroll::ScrollbarSpec` | `docs/api/public-api.txt:28700` | SAME |
| `termrock::scroll::TailScroll` | `termrock::scroll::TailScroll` | `docs/api/public-api.txt:28745` | SAME |
| `termrock::scroll::apply_scroll_delta` | `termrock::scroll::apply_scroll_delta` | `docs/api/public-api.txt:28803` | SAME |
| `termrock::scroll::apply_scroll_delta_unclamped` | `termrock::scroll::apply_scroll_delta_unclamped` | `docs/api/public-api.txt:28804` | SAME |
| `termrock::scroll::apply_term_width_scroll_delta` | `termrock::scroll::apply_term_width_scroll_delta` | `docs/api/public-api.txt:28805` | SAME |
| `termrock::scroll::clamp_scroll_offset` | `termrock::scroll::clamp_scroll_offset` | `docs/api/public-api.txt:28807` | SAME |
| `termrock::scroll::cursor_follow_offset` | `termrock::scroll::cursor_follow_offset` | `docs/api/public-api.txt:28808` | SAME |
| `termrock::scroll::dialog_scroll_axes` | `termrock::scroll::dialog_scroll_axes` | `docs/api/public-api.txt:28809` | SAME |
| `termrock::scroll::effective_offset` | `termrock::scroll::effective_offset` | `docs/api/public-api.txt:28810` | SAME |
| `termrock::scroll::horizontal_scrollbar_area` | `termrock::scroll::horizontal_scrollbar_area` | `docs/api/public-api.txt:28812` | SAME |
| `termrock::scroll::is_scrollable` | `termrock::scroll::is_scrollable` | `docs/api/public-api.txt:28813` | SAME |
| `termrock::scroll::max_line_width` | `termrock::scroll::max_line_width` | `docs/api/public-api.txt:28814` | SAME |
| `termrock::scroll::max_offset` | `termrock::scroll::max_offset` | `docs/api/public-api.txt:28815` | SAME |
| `termrock::scroll::max_offset_u16` | `termrock::scroll::max_offset_u16` | `docs/api/public-api.txt:28816` | SAME |
| `termrock::scroll::mouse_scroll_delta` | `termrock::scroll::mouse_scroll_delta` | `docs/api/public-api.txt:28817` | SAME |
| `termrock::scroll::mouse_scroll_delta_with_step` | `termrock::scroll::mouse_scroll_delta_with_step` | `docs/api/public-api.txt:28818` | SAME |
| `termrock::scroll::offset_for_track_position_u16` | `termrock::scroll::offset_for_track_position_u16` | `docs/api/public-api.txt:28820` | SAME |
| `termrock::scroll::render_line_with_fixed_prefix_scroll` | `termrock::scroll::render_line_with_fixed_prefix_scroll` | `docs/api/public-api.txt:28824` | SAME |
| `termrock::scroll::render_lines_with_offset_in_area` | `termrock::scroll::render_lines_with_offset_in_area` | `docs/api/public-api.txt:28825` | SAME |
| `termrock::scroll::render_scrollbar` | `termrock::scroll::render_scrollbar` | `docs/api/public-api.txt:28826` | SAME |
| `termrock::scroll::scroll_hint_spans` | `termrock::scroll::scroll_hint_spans` | `docs/api/public-api.txt:28827` | SAME |
| `termrock::scroll::scrollbar_offset_for_track_position` | `termrock::scroll::scrollbar_offset_for_track_position` | `docs/api/public-api.txt:28829` | SAME |
| `termrock::scroll::tail_vertical_thumb` | `termrock::scroll::tail_vertical_thumb` | `docs/api/public-api.txt:28830` | SAME |
| `termrock::scroll::vertical_scrollbar_area` | `termrock::scroll::vertical_scrollbar_area` | `docs/api/public-api.txt:28831` | SAME |
| `termrock::scroll::viewport_height` | `termrock::scroll::viewport_height` | `docs/api/public-api.txt:28832` | SAME |
| `termrock::scroll::viewport_width` | `termrock::scroll::viewport_width` | `docs/api/public-api.txt:28833` | SAME |

## `widgets`

| Jackin usage (old) | Current HEAD API | Evidence | Status |
|---|---|---|---|
| `termrock::widgets::Action` | `termrock::widgets::Action` | `docs/api/public-api.txt:57667` | SAME |
| `termrock::widgets::ActionBar` | `termrock::widgets::ActionBar` | `docs/api/public-api.txt:57706` | SAME |
| `termrock::widgets::ActionBarState` | `termrock::widgets::ActionBarState` | `docs/api/public-api.txt:57719` | SAME |
| `termrock::widgets::Anchor` | `termrock::widgets::Anchor` | `docs/api/public-api.txt:32618` | SAME |
| `termrock::widgets::Backdrop` | `termrock::widgets::Backdrop` | `docs/api/public-api.txt:59070` | SAME |
| `termrock::widgets::ChoiceDialog` | `termrock::widgets::ChoiceDialog` | `docs/api/public-api.txt:61267` | SAME |
| `termrock::widgets::ChoiceDialogState` | `termrock::widgets::ChoiceDialogState` | `docs/api/public-api.txt:61278` | SAME |
| `termrock::widgets::DetailCapability` | `termrock::widgets::DetailCapability` | `docs/api/public-api.txt:38029` | SAME |
| `termrock::widgets::DetailRow` | `termrock::widgets::DetailRow` | `docs/api/public-api.txt:23518` | SAME |
| `termrock::widgets::DetailTable` | `termrock::widgets::DetailTable` | `docs/api/public-api.txt:64956` | SAME |
| `termrock::widgets::DetailTableOutcome` | `termrock::widgets::DetailTableOutcome` | `docs/api/public-api.txt:38078` | SAME |
| `termrock::widgets::DetailTableState` | `termrock::widgets::DetailTableState` | `docs/api/public-api.txt:25929` | SAME |
| `termrock::widgets::Dialog` | `termrock::widgets::Dialog` | `docs/api/public-api.txt:61272` | SAME |
| `termrock::widgets::DiffKind` | `termrock::widgets::DiffKind` | `docs/api/public-api.txt:38712` | SAME |
| `termrock::widgets::DiffLine` | `termrock::widgets::DiffLine` | `docs/api/public-api.txt:20768` | SAME |
| `termrock::widgets::DiffState` | `termrock::widgets::DiffState` | `docs/api/public-api.txt:92006` | SAME |
| `termrock::widgets::DiffView` | `termrock::widgets::DiffView` | `docs/api/public-api.txt:66218` | SAME |
| `termrock::widgets::EditAction` | `termrock::widgets::EditAction` | `docs/api/public-api.txt:39492` | SAME |
| `termrock::widgets::HintSpan` | `termrock::widgets::HintSpan` | `docs/api/public-api.txt:2192` | SAME |
| `termrock::widgets::List` | `termrock::widgets::List` | `docs/api/public-api.txt:72363` | SAME |
| `termrock::widgets::ListRow` | `termrock::widgets::ListRow` | `docs/api/public-api.txt:18901` | SAME |
| `termrock::widgets::ListState` | `termrock::widgets::ListState` | `docs/api/public-api.txt:18889` | SAME |
| `termrock::widgets::MessageDialog` | `termrock::widgets::MessageDialog` | `docs/api/public-api.txt:74226` | SAME |
| `termrock::widgets::Panel` | `termrock::widgets::Panel` | `docs/api/public-api.txt:76387` | SAME |
| `termrock::widgets::PanelEmphasis` | `termrock::style::PanelChrome` (also re-exported from `widgets`) | `migrations/0061-v0.13.0-design-system-sole-paint.md:9` | MOVED |
| `termrock::widgets::Progress` | `termrock::widgets::Progress` | `docs/api/public-api.txt:92012`; public alias of `ProgressBar`, preserved by `migrations/0177-v0.13.0-progress-bar.md` | SAME |
| `termrock::widgets::ProgressKind` | `termrock::widgets::ProgressKind` | `docs/api/public-api.txt:48039` | SAME |
| `termrock::widgets::RowRole` | `termrock::widgets::RowRole` | `docs/api/public-api.txt:49469` | SAME |
| `termrock::widgets::Severity` | `termrock::widgets::Severity` | `docs/api/public-api.txt:38244` | SAME |
| `termrock::widgets::StatusBar` | `termrock::widgets::StatusBar` | `docs/api/public-api.txt:84450` | SAME |
| `termrock::widgets::StatusBarState` | `termrock::widgets::StatusBarState` | `docs/api/public-api.txt:20360` | SAME |
| `termrock::widgets::StatusSlot` | `termrock::widgets::StatusSlot` | `docs/api/public-api.txt:18651` | SAME |
| `termrock::widgets::TAB_GAP` | `termrock::widgets::TAB_GAP` | `docs/api/public-api.txt:91586`; identifier contains “GAP” but is not a gap; generic?: no | SAME |
| `termrock::widgets::Tab` | `termrock::widgets::Tab` | `docs/api/public-api.txt:85735` | SAME |
| `termrock::widgets::Tabs` | `termrock::widgets::Tabs` | `docs/api/public-api.txt:86085` | SAME |
| `termrock::widgets::TabsState` | `termrock::widgets::TabsState` | `docs/api/public-api.txt:86091` | SAME |
| `termrock::widgets::TextInput` | `termrock::widgets::TextInput` | `docs/api/public-api.txt:86872` | SAME |
| `termrock::widgets::TextInputOutcome` | `termrock::widgets::TextInputOutcome` | `docs/api/public-api.txt:42265` | SAME |
| `termrock::widgets::TextInputState` | `termrock::widgets::TextInputState` | `docs/api/public-api.txt:62886` | SAME |
| `termrock::widgets::Toast` | `termrock::widgets::Toast` | `docs/api/public-api.txt:87717` | SAME |
| `termrock::widgets::Validation` | `termrock::widgets::Validation` | `docs/api/public-api.txt:56771` | SAME |
| `termrock::widgets::Viewport` | `termrock::widgets::Viewport` | `docs/api/public-api.txt:90597` | SAME |
| `termrock::widgets::lay_out_tabs` | `termrock::widgets::lay_out_tabs` | `docs/api/public-api.txt:91839` | SAME |
| `termrock::widgets::render_hint_bar` | `termrock::widgets::render_hint_bar` | `docs/api/public-api.txt:91958` | SAME |
| `termrock::widgets::tab_at_column` | `termrock::widgets::tab_at_column` | `docs/api/public-api.txt:91986` | SAME |
| `termrock::widgets::wrapped_hint_lines` | `termrock::widgets::wrapped_hint_lines` | `docs/api/public-api.txt:91996` | SAME |

## `text`

| Jackin usage (old) | Current HEAD API | Evidence | Status |
|---|---|---|---|
| `termrock::text::display_cols` | `termrock::text::display_cols` | `docs/api/public-api.txt:31735` | SAME |
| `termrock::text::display_cols_slice` | `termrock::text::display_cols_slice` | `docs/api/public-api.txt:31736` | SAME |
| `termrock::text::leading_space_cols` | `termrock::text::leading_space_cols` | `docs/api/public-api.txt:31742` | SAME |
| `termrock::text::sanitize_terminal_title` | `termrock::text::sanitize_terminal_title` | `docs/api/public-api.txt:31751` | SAME |
| `termrock::text::take_display_cols` | `termrock::text::take_display_cols` | `docs/api/public-api.txt:31755` | SAME |

## `osc`

| Jackin usage (old) | Current HEAD API | Evidence | Status |
|---|---|---|---|
| `termrock::osc::ClipboardSelection` | `termrock::osc::ClipboardSelection` | `docs/api/public-api.txt:9164` | SAME |
| `termrock::osc::ClipboardWrite` | `termrock::osc::ClipboardWrite` | `docs/api/public-api.txt:9258` | SAME |
| `termrock::osc::PointerShape` | `termrock::osc::PointerShape` | `docs/api/public-api.txt:9208` | SAME |
| `termrock::osc::encode_clipboard` | `termrock::osc::encode_clipboard` | `docs/api/public-api.txt:9394` | SAME |
| `termrock::osc::encode_hyperlink_close` | `termrock::osc::encode_hyperlink_close` | `docs/api/public-api.txt:9395` | SAME |
| `termrock::osc::encode_hyperlink_open` | `termrock::osc::encode_hyperlink_open` | `docs/api/public-api.txt:9396` | SAME |
| `termrock::osc::encode_pointer` | `termrock::osc::encode_pointer` | `docs/api/public-api.txt:9397` | SAME |

## `layout`

| Jackin usage (old) | Current HEAD API | Evidence | Status |
|---|---|---|---|
| `termrock::layout::DialogSpec` | `termrock::layout::DialogSpec` | `docs/api/public-api.txt:7793` | SAME |
| `termrock::layout::Placement` | `termrock::layout::Placement` | `docs/api/public-api.txt:7042` | SAME |
| `termrock::layout::bottom_rows` | `termrock::layout::bottom_rows` | `docs/api/public-api.txt:9131` | SAME |
| `termrock::layout::render_dialog_shell` | `termrock::layout::render_dialog_shell` | `docs/api/public-api.txt:9155` | SAME |
| `termrock::layout::render_scrollable_dialog_body` | `termrock::layout::render_scrollable_dialog_body` | `docs/api/public-api.txt:9156` | SAME |
| `termrock::layout::resolve_dialog` | `termrock::layout::resolve_dialog` | `docs/api/public-api.txt:9157` | SAME |

## `input`

| Jackin usage (old) | Current HEAD API | Evidence | Status |
|---|---|---|---|
| `termrock::input::KeyBinding` | `termrock::input::KeyBinding` | `docs/api/public-api.txt:1901` | SAME |
| `termrock::input::KeyChord` | `termrock::input::KeyChord` | `docs/api/public-api.txt:1955` | SAME |
| `termrock::input::KeyCode` | `termrock::input::KeyCode` | `docs/api/public-api.txt:1579` | SAME |
| `termrock::input::KeyEvent` | `termrock::input::KeyEvent` | `docs/api/public-api.txt:1437` | SAME |
| `termrock::input::KeyModifiers` | `termrock::input::KeyModifiers` | `docs/api/public-api.txt:1957` | SAME |
| `termrock::input::Keymap` | `termrock::input::Keymap` | `docs/api/public-api.txt:2186` | SAME |
| `termrock::input::MouseEventKind` | `termrock::input::MouseEventKind` | `docs/api/public-api.txt:1797` | SAME |
| `termrock::input::Visibility` | `termrock::input::Visibility` | `docs/api/public-api.txt:1851` | SAME |

## `keymap`

| Jackin usage (old) | Current HEAD API | Evidence | Status |
|---|---|---|---|
| `termrock::keymap::KeyBinding` | `termrock::keymap::KeyBinding` | `docs/api/public-api.txt:1902` | SAME |
| `termrock::keymap::KeyChord` | `termrock::keymap::KeyChord` | `docs/api/public-api.txt:1603` | SAME |
| `termrock::keymap::Keymap` | `termrock::keymap::Keymap` | `docs/api/public-api.txt:1302` | SAME |
| `termrock::keymap::SCROLL_HINT_KEYMAP` | `termrock::keymap::SCROLL_HINT_KEYMAP` | `docs/api/public-api.txt:6454` | SAME |
| `termrock::keymap::Visibility` | `termrock::keymap::Visibility` | `docs/api/public-api.txt:1855` | SAME |
| `termrock::keymap::chord_glyph` | `termrock::keymap::chord_glyph` | `docs/api/public-api.txt:6455` | SAME |
| `termrock::keymap::glyph` | `termrock::keymap::glyph` | `docs/api/public-api.txt:6130` | SAME |
| `termrock::keymap::raw_bytes_to_chord` | `termrock::keymap::raw_bytes_to_chord` | `docs/api/public-api.txt:6456` | SAME |

## `interaction`

| Jackin usage (old) | Current HEAD API | Evidence | Status |
|---|---|---|---|
| `termrock::interaction::FocusRing` | `termrock::interaction::FocusGraph` + `InteractionScene` | `migrations/0066-v0.13.0-lookbook-host-frame.md:7`; `docs/api/public-api.txt:4760` | MOVED |
| `termrock::interaction::HitRegion` | `termrock::interaction::HitRegion` | `docs/api/public-api.txt:4773` | SAME |
| `termrock::interaction::ModalClickResult` | `termrock::interaction::OverlayStack::handle_outside_click` | `migrations/0065-v0.13.0-overlay-stack-sole.md:8` | MOVED |
| `termrock::interaction::ModalStack` | host domain payload + `termrock::interaction::OverlayStack` | `migrations/0065-v0.13.0-overlay-stack-sole.md:7` | MOVED |
| `termrock::interaction::Outcome` | `termrock::interaction::Outcome` | `docs/api/public-api.txt:3148` | SAME |
| `termrock::interaction::classify_click` | `termrock::interaction::OverlayStack::handle_outside_click` | `migrations/0065-v0.13.0-overlay-stack-sole.md:8` | MOVED |

## `ansi_text`

| Jackin usage (old) | Current HEAD API | Evidence | Status |
|---|---|---|---|
| `termrock::ansi_text::line_from_ansi` | `termrock::ansi_text::line_from_ansi` | `docs/api/public-api.txt:345` | SAME |

## Sixteen Jackin-used widget families at HEAD

| Family | Current export | Evidence |
|---|---|---|
| Action / ActionBar | `termrock::widgets::ActionBar` | `docs/api/public-api.txt:57706` |
| Backdrop | `termrock::widgets::Backdrop` | `docs/api/public-api.txt:59070` |
| ChoiceDialog | `termrock::widgets::ChoiceDialog` | `docs/api/public-api.txt:61267` |
| Dialog | `termrock::widgets::Dialog` | `docs/api/public-api.txt:61272` |
| MessageDialog | `termrock::widgets::MessageDialog` | `docs/api/public-api.txt:74226` |
| DetailTable | `termrock::widgets::DetailTable` | `docs/api/public-api.txt:64956` |
| DiffView | `termrock::widgets::DiffView` | `docs/api/public-api.txt:66218` |
| HintBar / HintSpan | `termrock::widgets::HintSpan` | `docs/api/public-api.txt:2192` |
| List | `termrock::widgets::List` | `docs/api/public-api.txt:72363` |
| Panel | `termrock::widgets::Panel` | `docs/api/public-api.txt:76387` |
| Progress | `termrock::widgets::Progress` (alias; canonical `ProgressBar`) | `docs/api/public-api.txt:92012` |
| StatusBar | `termrock::widgets::StatusBar` | `docs/api/public-api.txt:84450` |
| Tabs | `termrock::widgets::Tabs` | `docs/api/public-api.txt:86085` |
| TextInput | `termrock::widgets::TextInput` | `docs/api/public-api.txt:86872` |
| Toast | `termrock::widgets::Toast` | `docs/api/public-api.txt:87717` |
| Viewport | `termrock::widgets::Viewport` | `docs/api/public-api.txt:90597` |
