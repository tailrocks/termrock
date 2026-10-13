# TUI app deep analysis — what the best terminal apps actually paint

**Status:** design SoT (reference evidence + binding steals)
**Audience:** design, implementers
**Method:** cell-level extraction from repos, theme/skin files, docs, screenshots
(August 2026). Sources cited inline.
**Related:** [`tui-design-research-2026-08.md`](./tui-design-research-2026-08.md)
(styling system), [`tui-motion-system.md`](./tui-motion-system.md) (transitions),
[`experience-research-2026.md`](./experience-research-2026.md) (cluster DNA)

---

## 1. btop — density without clutter

[repo](https://github.com/aristocratos/btop) ·
[themes](https://github.com/aristocratos/btop/tree/main/themes)

- **Bracketed gradient meters**: every scalar = label + meter + value on one row.
  Meter fill uses block elements `█▉▊▋▌▎▏` (1/8-cell resolution); filled cells
  interpolate fg along a 3-point ramp (`cpu_start/mid/end`), empty cells one dark
  `meter_bg`. Position = magnitude, hue = severity — two encodings, zero extra
  space.
- **Braille sparklines** (2×4 dots/cell) with block/tty fallbacks; gradient mapped
  to vertical position so amplitude reads as color at 1-cell height.
- **Process list depth gradient** — rows darken down the list; recency is
  pre-attentive. Selected = `selected_bg/fg`; followed = `followed_bg`.
- **Chrome**: single-line boxes, title in border (`┤ CPU ├`), per-box border tint,
  separation by border + tinted title, never by bg fill.
- Theme = ~50 keys: 15 semantic core + gradient triplets per metric family.

**Steal:** (a) 1/8-cell `Meter` with 3-point ramp + explicit track color;
(b) braille `Sparkline` with position→color mapping; (c) theme schema = semantic
core + gradient triplets; (d) title-in-border as *the* panel label convention.

## 2. yazi — async file manager

[repo](https://github.com/sxyazi/yazi) ·
[theme docs](https://yazi-rs.github.io/docs/configuration/theme)

- **Miller columns**: parent/current/preview; hovered row strong fill, context rows
  dimmed. Spatial context survives lateral motion.
- **Image preview capability ladder**: Kitty placeholders → iTerm2 → Sixel →
  Überzug++ → Chafa ASCII. Best available renderer, never a blank pane.
- **which-key popup** (`[which]`: `mask/cand/rest/desc/separator`) after chord
  prefix: key accent, description dim, separator between groups.
- **Mode block in status bar** (`[mode]`: `normal_main/normal_alt`, …) — solid
  mode-colored block + `_alt` segment, airline-style two-tone.
- **Overlay style vocabulary**: every overlay type (`confirm, input, pick, cmp,
  help, notify, spot, tasks`) has a `{border,title,body,…}` style triple.
- Selection = marker glyphs + count badges in mode block (`marker_copied`,
  `count_copied`); filetype colors semantic (url/mime rules), not extension-based.

**Steal:** which-key overlay with `cand/rest/desc` split; two-tone mode block;
per-overlay style triples; marker-glyph + count-badge dual channel.

## 3. lazygit — key hints as ambient furniture

[repo](https://github.com/jesseduffield/lazygit)

- **Options bar**: bottom line, key in accent, description dim, **updates per
  focused panel** — a which-key that never pops up. Self-documenting spatially.
- **Title bars as status**: branch, filter counts, search state in panel titles;
  focus = `activeBorderColor`, search sub-mode = third border color. Border color
  alone carries focus *and* sub-mode.
- **Two-tier selection**: fill when panel focused, **bold only** when unfocused —
  cursor stays visible without competing fills.
- Theme = 11 keys; color values accept `reverse`/`bold` modifiers; border modes
  `rounded|single|double|hidden` — borderless first-class.

**Steal:** context-bound key-hint footer; two-tier selection; minimum viable theme
(`activeBorder/inactiveBorder/options/selectedLineBg/defaultFg` ≈ 5 roles).

## 4. k9s — operational dashboard grammar

[skins](https://k9scli.io/topics/skins/)

- **Title bar as query surface**: `Pods(default)[10] /label=app` — name, scope,
  live counter, active filter, each its own color role (`frame.title.{counter,
  filter,highlight}`).
- **Namespaced skin taxonomy** `k9s.frame.border.{fg,focus}` /
  `k9s.views.table.header.sorter` — region → component → property; the best
  precedent for a design-token tree.
- **Status pulse line**: verb-colored flash reporting last operation
  (`status.{new,modify,add,error,kill,completed}`).
- Full-screen overlay swap (xray) instead of stacked floats — one paint authority.

**Steal:** title composition `Name(scope)[count] <filter>` with per-segment roles;
verb-colored transient status; namespaced token tree; drill-in overlay swap.

## 5. helix — modes as the product

[themes](https://docs.helix-editor.com/themes.html)

- **Mode-shaped cursor**: block NORMAL / bar INSERT / underline SELECT, themed per
  mode (`ui.cursor.*`), echoed by statusline color (`color-modes`). State is
  embodied, never read.
- **Picker anatomy**: prompt + separator + results + **live preview pane**;
  selection always previews before commit. Columned pickers highlight the cursor's
  column (`ui.picker.header.column.active`).
- **`ui.*` scope tree (~40 scopes), longest-match resolution** + `inherits =`
  delta themes — coarse-tune or fine-tune from the same grammar.
- Severity as texture: `diagnostic.*` with `line|curl|dashed|dotted|double_line`.
- Borderless splits: `ui.window` = 1-cell separator only.

**Steal:** longest-match token resolution; per-mode cursor + statusline pairs;
picker = prompt/separator/list/live-preview; severity texture styles.

## 6. zellij — multiplexer chrome that teaches

[themes](https://zellij.dev/documentation/themes)

- **One ribbon component** for mode, tabs, and key hints (`ribbon_selected` /
  `ribbon_unselected`) — bar, mode, tabs are one visual system.
- **Frame state = focus × mode**: focused frame recolors *again* in non-base modes
  (`frame_highlight`) — chrome teaches modal state.
- **Per-component fixed emphasis ladder `emphasis_0..3`** (used e.g. for fuzzy
  match ranks) — better abstraction than ad-hoc bright/dim per widget.
- Floating panes = same frame grammar, spatially distinct; pane titles editable in
  top frame.

**Steal:** `emphasis_0..3` ladder per component; frame = f(focus, mode); one
ribbon family for mode/tabs/hints.

## 7. posting — Postman mental model, terminal physics

[repo](https://github.com/darrenburns/posting)

- **Jump mode**: 1–2 letter reverse-video labels overlaid on every focusable
  widget; type to teleport focus. Zero-memory navigation across dense forms.
- **Verb chips**: GET green / POST yellow / DELETE red — the only saturated hues on
  screen; everything else muted ramp.
- **Caret-anchored autocomplete** (env vars) with typed icon column.
- Palette rows = action + dim keybinding column.

**Steal:** jump-label overlay navigation; semantic verb/status chips with fixed
color mapping; caret-anchored completion.

## 8. harlequin — desktop IDE compressed

[repo](https://github.com/tconbeer/harlequin)

- **Catalog tree with right-aligned dim type annotations** (`INT`, `VARCHAR`) in
  the row — metadata in the tree removes round-trips.
- **Truncation honesty banner**: "Showing 10,000 of N records" as table footer.
- Buffer tabs above editor; locale-aware number formatting; F1 restyles the app as
  documentation.
- Theme = variable sheet passed to all widgets (`--theme`), one paint authority.

**Steal:** right-aligned dim metadata column in trees; truncation banner rows;
theme-as-variable-sheet distribution.

## 9. opencode — theme system as design tokens

[themes](https://opencode.ai/docs/themes/)

- **3-rung ladders**: surfaces `background/backgroundPanel/backgroundElement`;
  borders `borderSubtle/border/borderActive`; text `text/textMuted`. The most
  complete role taxonomy in the survey.
- **Adaptive values** `{dark, light}` per key + `none` for terminal-default
  transparency; `defs` named palette referenced by semantic keys.
- **Layered theme precedence**: builtin → user → project → cwd.
- Leader-key command layer (`ctrl+x …`); session sidebar; live-preview `/theme`
  picker; full `diff*`/`markdown*`/`syntax*` role families.

**Steal:** 3-rung surface/border/text ladders; `{dark,light}` adaptive values +
`none`; `defs` + semantic indirection; layered theme directories.

## 10. Crush — honest agent state

[repo](https://github.com/charmbracelet/crush)

- **Tri-state inline permission**: `Allow? [y/n/always]` — once/never/always in one
  line, allow-list persists by tool. Minimal ceremony, auditable.
- **Collapsed tool-call rows**: icon + verb + target on one line, expand on demand.
- **Live state badges**: `IsBusy`, `AttachedClients` on session rows.
- **Compact density toggle** as first-class layout option.

**Steal:** tri-state permission primitive; collapsible status rows; live badges on
list items; density toggle.

## 11. glow — markdown as typesetting

[repo](https://github.com/charmbracelet/glow) ·
[glamour](https://github.com/charmbracelet/glamour)

- **Heading→modifier ladder** (no font sizes): H1 colored+bold → H2 bold → H3
  italic. Blockquotes behind `▌` bar; hanging-indent lists; code blocks padded with
  bg fill; links as `text (url)` dim suffix.
- **Per-element JSON stylesheet** — a themable widget-recipe system, exactly.
- Auto dark/light background detection.

**Steal:** markdown typographic ladder; blockquote bar + hanging indents;
per-element stylesheet schema (`markdown.*` role family).

## 12. delta / gitui — diffs are a rendering problem

[delta](https://github.com/dandavison/delta) ·
[gitui](https://github.com/gitui-org/gitui)

- **Two-intensity changed-line backgrounds**: changed line = muted bg, changed
  *token* = stronger bg (word-level emphasis), foreground stays syntax-highlighted.
- **Decorated headers** (commit/file/hunk): box/line/underline styles as theme
  options — the header layer makes raw diffs feel designed.
- Dual dim line-number gutters; OSC8 hyperlinks; side-by-side mode.
- gitui: context-filtered help bar (only keys valid in focused tab); theme ≈ 10
  roles — proof a git TUI needs no more.

**Steal:** dual-intensity bg + syntax fg; header decoration variants; dual gutters;
context-filtered hint bar.

## 13. Single-screen utilities — mprocs, gping, duf, dust

[mprocs](https://github.com/pvolok/mprocs) · [gping](https://github.com/orf/gping)
· [duf](https://github.com/muesli/duf) · [dust](https://github.com/bootandy/dust)

- mprocs: status-glyph process column + interactive pane + removable keymap strip
  + zoom-to-pane key.
- gping: full-screen braille graph, per-series color, stat readout line
  (last/min/max/avg), `--simple-graphics` dot fallback.
- duf: **threshold-colored inline cell bars** in tables; grouped sections under
  plain headers; auto light/dark; re-flows to terminal width.
- dust: **dual-channel bars** (length = value, shade = depth); `-R`
  screen-reader mode swaps bars for explicit depth column — non-color cue done
  right.

**Steal:** threshold-colored cell bars; dual-channel length+shade bars;
accessibility mode trading glyphs for columns; removable hint strip; zoom key.

## 14. Agent TUIs — Claude Code, codex, gemini-cli, Grok Build

[codex-rs](https://github.com/openai/codex) ·
[gemini-cli](https://github.com/google-gemini/gemini-cli) ·
[grok-build](https://github.com/xai-org/grok-build)

Same visual grammar across four frameworks (Ratatui ×2, OpenTUI, Ink) — proof the
patterns are framework-independent:

- **Composer**: bottom multi-line input, bordered, dim placeholder, mode/agent chip
  attached; expand-on-paste.
- **Transcript**: borderless full-width flow; tool calls collapse to one status
  line (spinner + verb + target, expandable); markdown ladder for assistant turns.
- **Permission prompt**: the category's defining widget — payload rendered with
  full syntax/diff fidelity + numbered options incl. persist.
- **Spinner-with-verb**: braille/dot spinner + rotating gerund + elapsed + `esc to
  interrupt`. The verb vocabulary is the polish.
- **Status line**: cwd / branch / model / context %; left-right split.

**Steal:** spinner verb slot + elapsed + interrupt hint; collapsible tool-call row;
permission dialog schema (title, payload region, options with persist);
transcript/composer separation (borderless flow above, bordered input below).

## 15. Dashboards & pickers — alpha / snacks / telescope

[snacks.nvim](https://github.com/folke/snacks.nvim) ·
[telescope](https://github.com/nvim-telescope/telescope.nvim)

- **Dashboard = declarative sections** (header art, keys, recent files, startup
  timer) + pane columns; **whitespace-first**: centered, 40–60 cols, gaps as
  structure.
- **3-part shortcut row** `icon | desc (fixed width) | [key]` — clean row anatomy,
  named highlight groups per part.
- **Telescope picker**: floating three-part frame (prompt with prefix + border
  title / results with `>` caret + match-char highlight / preview), pluggable
  layouts `horizontal|vertical|center|dropdown|cursor|ivy`.

**Steal:** 3-part menu row anatomy; picker layout variants; match-char highlight
separate from selection; whitespace-first dashboard composition.

---

## 16. Cross-app pattern table

| Pattern | Apps |
|---|---|
| Focus = border color only (inactive neutral) | lazygit, k9s, zellij, posting, harlequin, opencode, crush — **TermRock law confirmed** |
| Title-in-border as label/status surface | btop, lazygit, k9s (`[count]`+filter), zellij, posting, harlequin, telescope |
| Key-hint footer bound to focus | lazygit, gitui, mprocs, glow, Textual footer |
| Which-key overlay after prefix | yazi, helix, opencode leader-key |
| Two-tier selection (fill focused / bold unfocused) | lazygit, k9s, yazi, helix |
| Ramp meters + braille graphs | btop, gping, dust, duf |
| Fixed emphasis ladder per component | zellij `emphasis_0..3`, opencode 3-rung ladders, btop title/hi/main/inactive |
| Namespaced token tree `region.component.prop` | k9s, helix `ui.*`, yazi, opencode, zellij |
| Overlay style triple {border,title,body} | yazi, helix popups, agent permission dialogs |
| Mode as color + shape | helix cursor, yazi mode block, zellij ribbons, posting verb chips |
| Preview-before-commit in pickers | helix, telescope, opencode `/theme`, yazi, glow |
| Dual-intensity diff bg + syntax fg | delta, crush, opencode, Claude Code/codex |
| Collapsible one-line status rows (verb+target) | crush, Claude Code, codex, opencode, mprocs, k9s |
| Spinner with verb + elapsed + interrupt hint | Claude Code, codex, gemini-cli, opencode, crush |
| Tri-state permission + allow-list | crush, Claude Code, codex, gemini-cli, grok-build |
| Markdown typographic ladder | glow, opencode `markdown*`, all agent transcripts |
| Capability ladder for graphics | yazi, snacks, btop, gping |
| Accessibility/non-color cues | dust `-R`, btop tty mode, lazygit `reverse`/`bold` modifiers |
| Whitespace-first centered layouts | alpha, snacks, telescope dropdown |
| Layered theme precedence (builtin→user→project→cwd) | opencode, yazi, helix inherits, zellij |
| Adaptive dark/light + auto-detect | opencode, glow, duf, delta, crush, helix |
| Live state badges on list items | crush, mprocs, k9s counters, zellij tabs |
| Truncation honesty banners | harlequin, telescope counts, yazi tasks |
| Jump-label navigation | posting, helix jump motions |

## 17. Binding steals for TermRock (ranked by leverage)

1. **Namespaced token tree with longest-match resolution** (k9s/helix/opencode) —
   supersedes the flat 63-role array as the *authoring* surface; compiled runtime
   representation stays.
2. **3-rung ladders (surface/border/text) + per-component `emphasis_0..3`**
   (opencode/zellij) — adopt as `ChromeTokens`/recipe extension.
3. **Two-tier selection + focus-by-border-role only** (lazygit) — already TermRock
   law; enforce in every collection recipe, delete stray fill paths.
4. **Context key-hint footer + which-key overlay as one component family**
   (lazygit/yazi) — `ShortcutHint`/`KeyboardHelp` merge into one system bound to
   the focus graph.
5. **Block-element `Meter` + braille `Sparkline` with gradient triplets** (btop) —
   new dataviz primitives; threshold-colored variant for tables (duf).
6. **Picker anatomy**: prompt + separator + list + **live preview** + pluggable
   layouts (helix/telescope) — upgrade `QuickOpen`/`Picker`/`CommandPalette`.
7. **Agent widget set**: collapsible tool-call row, spinner-with-verb, tri-state
   permission (crush/Claude Code/codex) — mostly exist; align anatomy to this
   grammar.
8. **Title-bar composition `Name(scope)[count] <filter>`** (k9s) — standard
   `Panel` title recipe with per-segment roles.
9. **Overlay style triple {border,title,body} per overlay type** (yazi) — one
   recipe family for dialog/popover/toast/menu/picker.
10. **Adaptive `{dark,light}` values + layered theme precedence** (opencode) —
    `RolePalette` authoring + loading model.
11. **Dual-channel bars** (dust: length = value, shade = depth) — tree/table bar
    recipe.
12. **Capability ladder for media** (yazi) — `ImageSurface` renderer chain.

Per repo law: primitives above land in `widgets`; assembled product surfaces
(agent transcript, dashboard, wizard) are `patterns` composites of them.
