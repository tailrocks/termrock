# TUI design specs and tools — source research (2026-08)

**Status:** design SoT (reference evidence + binding steals)
**Audience:** design, implementers
**Method:** deep read of source repos, spec files, theme files, docs (August
2026). Sources cited inline.
**Related:** [`tui-design-research-2026-08.md`](./tui-design-research-2026-08.md)
(styling system), [`tui-app-deep-analysis.md`](./tui-app-deep-analysis.md) (hero
apps), [`tui-theme-gallery.md`](./tui-theme-gallery.md) (color schemes),
[`terminal-aesthetics-landscape-2026-08.md`](./terminal-aesthetics-landscape-2026-08.md)

---

## 1. awesome-tui-design (cola-runner) — reverse-engineered DESIGN.md specs

[repo](https://github.com/cola-runner/awesome-tui-design)

Curated collection of `DESIGN.md` files reverse-engineered from real source of
top TUIs. Each spec follows the same 9-section skeleton: overview, palette
(truecolor + 256 + 16 with exact hexes/indices), typography, layout/grid,
component anatomy, states, motion, Do/Don't lists, and an agent prompt guide
(how to instruct an AI to reproduce the look). This format is itself the most
important finding — see §1.2.

### 1.1 Cross-spec laws (extracted from all 16 specs)

Every spec — Minimal, Claude Code, Codex CLI, Gemini CLI, Lazygit, k9s, btop,
and the scheme entries — converges on the same structure:

| Law | Evidence across specs |
|---|---|
| **Universal 10-role palette** | bg / fg / primary / secondary / accent / success / warning / error / muted / surface — every spec defines exactly these, different hues |
| **One accent discipline** | ≤1–2 accent hues per viewport region; accent = brand, not emphasis-by-default |
| **Focus = border color, never border weight** | matches TermRock focus law exactly |
| **Selection = reverse video or dim tint, never bright fill** | TermRock `SelectionChrome` law confirmed independently |
| **Min-width floor 80 cols** | below 80 → compact mode, never broken layout |
| **Titles in border** | `┤ title ├` as the panel label convention (btop, lazygit, k9s) |
| **No view transitions** | none of the praised specs animate screen swaps; state changes only |
| **Spinner taxonomy 80–120 ms** | braille or dot frames, one glyph column, fixed interval |
| **Muted text = hue-shifted gray, not dim-bright** | e.g. Claude Code uses desaturated terracotta-gray, Codex uses neutral gray ramp |

Representative extracted values (all verified against app source or theme
files by the spec authors):

| App | Accent | Background | Muted text | Selection |
|---|---|---|---|---|
| Minimal (Vercel/Linear) | `#0070f3` blue | `#000000` | `#888888` | reverse |
| Claude Code | `#D77757` terracotta / `#FD5DB1` hot pink | terminal default | desat. terracotta-gray | dim tint |
| Codex CLI | adaptive monochrome (fg only) | terminal default | gray ramp | reverse |
| Lazygit | user theme (default green) | terminal default | `#666666`-class | bold when unfocused |
| btop | gradient ramps per metric | `#0e0e10`-class | per-theme | `selected_bg` |

### 1.2 The DESIGN.md format itself is a distribution model

Why the format works, and why TermRock should adopt it:

1. **Executable specificity.** Every claim carries a value: hex, cell count,
   ms duration, codepoint. An implementer (human or AI) cannot drift.
2. **Three color tiers side by side** (truecolor / 256 / 16) force the author
   to decide degradation up front — TermRock's quantization strategy should be
   documented in exactly this shape per shipped theme.
3. **ASCII fallback per glyph.** Each Unicode glyph lists its ASCII
   replacement in a table. Same shape as TermRock's planned capability-gated
   glyph catalog.
4. **Do/Don't lists** carry the taste, not just the values ("don't use double
   borders for focus", "don't color more than one panel at a time").
5. **Agent Prompt Guide section** — a canned prompt that reproduces the design
   in an AI coding session. Distribution to AI-assisted consumers, which is
   TermRock's stated adoption path (CLAUDE.md: "AI-assisted consumers can
   migrate quickly").
6. **One file per design**, self-contained, diffable, fork-adaptable — the
   text-native equivalent of shadcn's copy-and-adapt model.
7. **Reverse-engineered from source**, not aspirational — values are what the
   app actually ships, which keeps the spec honest.
8. **Small**: each spec is 200–400 lines. No prose essays; tables dominate.
9. **Scheme entries use the same skeleton as app entries** — a color scheme
   is documented with the same rigor as an application.

**Steal:** publish a `DESIGN.md` per shipped TermRock theme (phosphor first)
using this skeleton, including the agent prompt guide. It doubles as the
registry-format prototype CLAUDE.md anticipates ("design APIs that can later
support registry or copy-and-adapt distribution").

---

## 2. awesome-terminal-aesthetics (kud) — covered

The curated index (~60 tools, 8 categories) is extracted in
[`terminal-aesthetics-landscape-2026-08.md`](./terminal-aesthetics-landscape-2026-08.md)
§S2. Top signal for this doc: the praised set skews to tools with declarative
theme files, ported palettes (Catppuccin/Tokyo Night/Gruvbox everywhere), and
one-accent discipline — the same laws as §1.1.

---

## 3. Monospace Design TUI standard — the auditable-rule format

[repo](https://github.com/coreyt/monospace-design-tui) · v0.3.0 · CC BY-SA 4.0

A prescriptive "HIG for TUIs": falsifiable MUST-rules, auditable by inspection.
Concrete rules worth importing or deliberately rejecting:

| Domain | Rule (quoted/summarized) | TermRock verdict |
|---|---|---|
| Spacing | scale `0 1 2 3 4 6 8` cells only; 5, 7 forbidden | **Adopt** as TermRock spacing tokens |
| Grid | 80×24 functional floor; 120×40 design target; breakpoints Compact 40–79 / Standard 80–119 / Expanded 120–159 / Wide 160+ | **Adopt** breakpoint names + floor |
| Focus | "exactly one interactive element MUST hold focus at all times"; six mandatory states (Enabled, Focused, Hovered, Pressed, Selected, Disabled) + conditional Error | **Already TermRock law** — cite standard in docs |
| Color | role-assigned never literal; "color MUST NOT be the sole indicator"; truecolor→256→16→mono ladder, "MUST remain fully functional in monochrome"; contrast ≥4.5:1 body | **Adopt** as contract-test requirements |
| Borders | L1/L2 single-line, L3/L4 dialogs double-line `═║` | **Reject**: conflicts with TermRock law (border weight never encodes elevation or focus). Steal the *format*, keep single-line + semantic roles |
| Rounded corners | `╭╮╰╯` only for decorative non-interactive containers | **Adopt** |
| Typography | exactly 4 treatments (Display/Title/Body/Label); max 2 SGR attributes per span | **Adopt** the cap; treatments map to existing text roles |
| Motion | tiers Instant 0 / Fast 50–100 / Standard 150–300 / Slow 300–500 ms; nothing >500 ms; `NO_MOTION=1` forces Instant | **Adopt** — aligns with `tui-motion-system.md` budgets |
| Keyboard | dual-binding (F-key + common key); single-letter suppressed while text input focused; footer shows lowercase only; key meaning stable across screens | **Adopt** as keyboard contract |
| Footer | bottom 1–2 rows always, `F1 Help  F5 Refresh  / Filter  q Quit` format | Matches `ShortcutHint` direction |

**Steal:** the *auditable-rule format* itself. TermRock's design law should
keep moving toward falsifiable MUST/MUST NOT statements (Monospace-style) over
principle prose, because rules are contract-testable and principle prose is
not. Ship monochrome-must-work and non-color-cue requirements as tests.

---

## 4. Fresh — terminal IDE

[repo](https://github.com/sinelaw/fresh) · [docs](https://sinelaw-fresh.mintlify.app/introduction)
· Rust core (~283k SLoC) + TypeScript plugins on sandboxed QuickJS · ~7.4k
stars · GPL-2.0

Community praise centers on *discoverability and GUI ergonomics in terminal*,
not chrome: "intuitive like VS Code… even recognises the mouse", "feels and
works like a GUI, but works in terminal", "nailed the discoverability —
everything works intuitively". (No X post where @TheNoamLewis praises another
tool's design — he is Fresh's author; the praise flows toward Fresh.)

Extracted mechanics:

- **Prefix-mode command palette**: one input, prefix switches mode — none =
  file finder, `>` = commands, `#` = buffers, `:` = go-to-line. Hints line at
  palette bottom lists prefixes. Fuzzy supports acronym match (`fge` →
  features/groups/editor.tsx) and space-separated multi-term. `Tab` accepts
  top suggestion.
- **Theme model**: JSON, sections `editor` / `ui` / `syntax` / `diagnostic` /
  `search`; colors as RGB array **or** named (`"DarkGray"`, `"Default"`) —
  dual representation; `"inherits": "dark"` delta themes;
  `use_terminal_bg: true` transparency; config-watch live reload; in-app theme
  picker applies instantly. 7 built-ins (high-contrast default, dark, light,
  nord, dracula, nostalgia, solarized-dark). Theme distribution via plugin
  registry.
- **Explorer contract**: collapse glyphs `▸/▾`; git status as a decoration
  layer over a neutral tree (plugin-supplied badges), not baked into the tree.
- Menu bar + tabs + sidebar (default width 30, `Ctrl+B` toggle) + splits +
  integrated terminal; full settings UI and interactive keybinding editor
  in-app; mouse everywhere; non-modal VS Code-style keys.

**Steal:** (a) prefix-mode palette grammar — maps to `QuickOpen`/command
palette upgrade, already queued in `tui-app-deep-analysis.md` §17.6; (b) JSON
theme shape with `inherits` + dual color repr + `use_terminal_bg` — proven
shape for TermRock theme files; (c) decoration-layer-over-neutral-tree as the
tree/git integration model (matches widgets/patterns boundary: neutral tree in
widgets, git badges a pattern-side projection).

---

## 5. Superfile — GUI file-manager anatomy, Bubble Tea polish

[repo](https://github.com/yorukot/superfile) · [superfile.dev](https://superfile.dev/)
· Go + Bubble Tea · ~17.7k stars

- **Layout**: 3-column GUI-FM anatomy — sidebar (pinned dirs) / file panels
  (`n` spawns extra panels) / preview panel (images, video) + metadata bar +
  processes panel + clipboard.
- **Help on demand**: `?` overlay cheat-sheet; no persistent footer bar —
  contrast with lazygit's ambient options bar. Both work; overlay wins when
  chrome budget is tight.
- **Themes as flat files**: one TOML of semantic color keys per theme, 20+
  bundled (Catppuccin ×4 flavors default mocha, Nord, Tokyo Night, Dracula,
  Gruvbox, Rosé Pine…). Low authoring ceremony → high community theme
  contribution. Adoption evidence for the gallery plan.
- **Nerd Fonts assumed** — docs recommend installing one; icons are opt-in by
  font capability, not graceful-degradation-first. Contrast with Monospace
  standard's monochrome-must-work.

**Steal:** (a) sidebar + panels + preview as a `patterns/` file-manager
composite recipe, not widgets; (b) `?` cheat-sheet overlay generated from the
same binding table that drives dispatch — single source of truth for keys
(TermRock's `KeyboardHelp` should render from the focus/intent graph);
(c) theme = one flat file of semantic keys, ship curated named ports;
(d) glyph capability gate is a real decision: TermRock law = monochrome-safe
defaults with capability-gated rich glyphs (Nerd Font optional, never
required).

---

## 6. FrankenTUI — kernel discipline, marketing inflation

[repo](https://github.com/Dicklesworthstone/frankentui) ·
[frankentui.com](https://frankentui.com/) · Rust nightly · `ftui = "0.5"` ·
single author

Real and verified: Buffer → Diff → Presenter → ANSI kernel ("no hidden I/O");
**one-writer rule** (`TerminalWriter` serializes stdout); **RAII session
restore even on panic**; golden-checksum snapshot/time-travel harness; 80+
widgets; 46 demo screens doubling as snapshot targets; Web/WASM backend.

**First-class inline mode** (`ScreenMode::Inline { ui_height }`) preserves
scrollback — a legitimate differentiator; Ratatui leaves inline rendering to
each app's own hacks.

Inflated: "Bayesian diff strategy", "BOCPD resize coalescing",
"conformal-prediction alerts" — marketing names on ordinary heuristics; stat
contradictions on the site ("50 days" vs "5 days"); stable API explicitly
post-v1.

**Steal:** (a) demo-showcase-as-test-suite — every lookbook/gallery screen
doubles as a snapshot-test target (TermRock lookbook already close; formalize);
(b) first-class inline mode preserving scrollback as a session-mode contract —
worth a kernel RFC; (c) one-writer rule + RAII restore stated as kernel
invariants like TermRock's focus law; (d) **skip** probabilistic diffing —
deterministic diff + resize coalescing gets the same UX without the
math-cosplay.

---

## 7. Clack — prompt chrome grammar

[repo](https://github.com/bombshell-dev/clack) ·
[docs](https://bomb.sh/docs/clack/packages/prompts/) · JS; ports in Go
(go-clack), Ruby, Python

The cleanest prompt-session visual language in the survey. Exact grammar from
`packages/prompts/src/common.ts`:

- **Symbols** (unicode / ASCII fallback via `unicodeOr`): `◆/*` step-active,
  `■/x` cancel, `▲/x` error, `◇/o` submit, `┌ │ └` session frame (`└` closes
  the session), `●/○` radio, `◻/◼` checkbox off/on, `▪` password mask,
  `╭╮╰╯` note box.
- **State → color law** (one function, applied to glyph *and* guide bar):
  active → cyan `◆`; submit → green `◇`; error → yellow `▲`; cancel → red `■`.
- **Why it reads clean**: (1) one vertical guide bar threads the whole
  session — every prompt line hangs off `│`, and its color *is* the state, so
  a finished prompt visibly "settles" cyan `◆` → green `◇`; (2) active/submit
  are the same diamond filled/hollow — completion is a shape change, not just
  color (works without color); (3) everything else dim, hints in parens;
  (4) capability detection with hard ASCII fallbacks.

**Steal:** (a) one shared `prompt_chrome(state) -> (glyph, role)` function
across all prompt/form widgets — TermRock's `TextInput`, `PasswordInput`,
`Select`, `Checkbox`, wizard steps should all resolve chrome through it;
(b) guide-bar threading for any session-shaped surface (setup wizard, form,
multi-step dialog) — instant visual grouping, trivial in Ratatui;
(c) filled/hollow glyph pairs for active→done transitions as a non-color cue;
(d) `unicodeOr`-style fallback table as the core glyph catalog contract.

---

## 8. TUI Studio — the .tui insight

[tui.studio](https://tui.studio/) ·
[repo](https://github.com/jalonsogo/tui-studio) · **Alpha**

Figma-like canvas: drag-drop with live ANSI preview, zoom, layers, undo,
command palette; 20+ components; Absolute/Flexbox/Grid layout with CSS-like
property panel; 8 built-in themes. Export to 6 frameworks planned, **not
functional yet**; no Ratatui target.

The durable idea: projects save as `.tui` = plain JSON
`{version, meta:{name, theme, savedAt}, tree:{...}}` — a serializable widget
**tree** plus a named theme reference, git-friendly.

**Steal:** (a) JSON-serializable widget tree + named theme is the
interop/distribution format TermRock's registry future can adopt (CLAUDE.md
already anticipates registry/copy-and-adapt); (b) its component list is a
catalog parity checklist — Breadcrumb, Popover, Tooltip, Spacer are the easy
gaps to audit; (c) do **not** build a canvas — expose deterministic preview +
theme switching (TermRock lookbook direction already correct).

---

## 9. SilkCircuit — intensity variants of one palette

[repo](https://github.com/hyperb1iss/silkcircuit-nvim) · cyberpunk theme +
ports (Ghostty, Starship, Tmux, fzf, bat, delta)

Canonical palette: bg `#0a0a0f` near-black violet, fg `#e0e0e0`, electric
purple `#e135ff` (signature), hot pink `#ff79c6`, neon cyan `#80ffea`, green
`#50fa7b`, yellow `#f1fa8c`, orange `#ffb86c`. Dracula-derived with
violet-shifted purple/cyan.

The transferable idea is not the hues — it is the **variant system**: one
semantic palette × 5 intensity/ambient variants — Neon (100% sat), Vibrant
(85%), Soft (70%, "extended sessions"), Glow (ultra-dark bg + pure neon fg,
OLED), Dawn (light). One design, five rooms.

**Steal:** ship phosphor variants by intensity/ambient instead of N unrelated
themes: e.g. phosphor (default), phosphor-soft (long sessions), phosphor-glow
(OLED/marketing), phosphor-dawn (light). Same role IDs, different slot values —
the exact mechanism `tui-theme-gallery.md` §6 specifies for dark/light pairs.

---

## 10. Binding deltas for TermRock (ranked)

1. **`DESIGN.md` per shipped theme** (cola-runner format: 3-tier color tables,
   ASCII fallbacks, Do/Don't, agent prompt guide) — doubles as registry-format
   prototype. Start with phosphor.
2. **Monospace-standard contracts**: spacing scale `0 1 2 3 4 6 8`, breakpoint
   names, ≤2 SGR attributes per span, rounded corners only on decorative
   containers, monochrome-must-work + non-color-cue contract tests. Reject its
   double-line elevation rule (conflicts with focus law).
3. **Clack `prompt_chrome(state)` recipe** — one state→(glyph, role) function
   + guide-bar threading across all prompt/form/wizard surfaces; filled/hollow
   pairs as non-color completion cue.
4. **Fresh theme-file shape**: JSON, `inherits`, dual color repr (RGB |
   named | `Default`), `use_terminal_bg`, config-watch reload — adopt for
   TermRock theme loading.
5. **Prefix-mode palette** (`>`/`#`/`:`) with bottom hints line — upgrade
   `QuickOpen`.
6. **`KeyboardHelp`/`ShortcutHint` rendered from the binding table** (superfile
   `?` overlay + lazygit ambient bar as two density modes of one component).
7. **Inline mode preserving scrollback** as first-class session mode
   (FrankenTUI evidence) — kernel RFC.
8. **Lookbook screens = snapshot tests, formalized** (FrankenTUI showcase
   model).
9. **Phosphor intensity variants** (SilkCircuit model): phosphor / -soft /
   -glow / -dawn share role IDs.
10. **`.tui`-style serializable widget tree + named theme** as the future
    registry interop format; audit catalog against TUI Studio's component list
    (Breadcrumb, Popover, Tooltip, Spacer).

Per repo law: primitives land in `widgets`; assembled surfaces (file-manager
composite, wizard session, palette) are `patterns` composites of them.
