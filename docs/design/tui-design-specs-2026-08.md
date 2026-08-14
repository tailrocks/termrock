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

[repo](https://github.com/cola-runner/awesome-tui-design) · MIT · re-verified
2026-08-14 at commit `445d993e` (2026-04-05): still 16 specs, unchanged since
the first extraction. This re-read adds full per-spec value extraction (§1.3),
the authoring template (§1.4), and the sibling tui-art asset format (§1.5).

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
| **Titles in border** | `┤ title ├` as the panel label convention (btop, lazygit, k9s); btop inverts the tees — `┐cpu┌` — as its signature "notch" |
| **No view transitions** | none of the praised specs animate screen swaps; state changes only |
| **Spinner taxonomy 80–120 ms** | braille or dot frames, one glyph column, fixed interval |
| **Muted text = hue-shifted gray, not dim-bright** | e.g. Claude Code uses desaturated terracotta-gray, Codex uses neutral gray ramp |
| **Corner style is a theme slot; weight is not** | rounded `╭╮╰╯` in 6/9 schemes + lazygit/btop/gemini/codex; straight `┌┐└┘` in k9s/gruvbox/nord/minimal; double `╔═╗` only as deliberate retro identity (cyberpunk, retro) — weight never varies by state within a spec |
| **Elevation = background ramp, 3–4 steps** | ocean: shallow/mid/deep/abyss; catppuccin: base/mantle/crust; rose-pine: base/surface/overlay + highlight low/med/high — never heavier borders or fake shadows |
| **Tool-block border color = state channel** | gemini: running blue / pending yellow / focused green / done gray; claude-code: pink tools / lavender permission; k9s + lazygit: search-mode border accent — extends the focus-only border pair for process blocks |

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

### 1.3 Per-app signature mechanics (full value extraction)

The laws in §1.1 are the intersection; these are the differentiators — the one
mechanic per app that makes it recognizable. Values verified against the specs
(reverse-engineered from app source by the spec authors).

**Claude Code** — per-surface border color coding. Input box: dashed ASCII
(`-` / `|`), muted gray, shimmer between `#888888` and `#a6a6a6` — deliberately
*not* box-drawing ("casual, not corporate"). Tool blocks: hot pink `#fd5db1`.
Permission dialogs: lavender `#b1b9f9`. Auto-accept mode: purple `#af87ff`.
Thinking spinner: reverse-mirror cycle `· ✢ ✳ ✶ ✻ ✽ ✻ ✶ ✳ ✢` at 120 ms,
terracotta `#d77757` shimmering to `#eb9f7f`, paired with a random whimsical
verb from ~184 options ("Percolating...", "Shenaniganing..."). Diff tints:
added bg `#225c2b`, removed bg `#7a2936`. Subagents each get one of 8 palette
colors. Hard rule: "don't colorize AI response body text — white for trust and
readability."

**Codex CLI** — adaptive monochrome via runtime background detection. Queries
the terminal's actual bg with OSC 11 and blends everything toward it (dark
*and* light terminals, continuously — not a dark/light pair). Signature
shimmer: cosine wave `brightness = cos(position − time)` sweeps status text,
per-character fg blended toward the detected bg; TrueColor required, falls
back to `•`/`◦` alternating blink. Gutter replaces boxing for tool output:
`▌` block start, `│` continuation, `└` end marker. Startup: 10 ASCII-art
variants × 36 frames at 80 ms in block chars (`▄ ▀ █ ▐ ▌ ▝ ▘`). GitHub-style
diff tints in dark/light pairs (`#213a2b`/`#4a221d`, `#dafbe1`/`#ffebe9`).
Deliberately unbranded: green only for success, red only for errors. Built in
Rust + ratatui.

**Gemini CLI** — semantic token theme architecture. Tokens like `ui.active`,
`ui.focus`, `status.ok`, `status.warning`, `status.error`, `border.default`,
`text.primary`, `text.secondary` map to colors per theme — this indirection is
why 15+ themes ship out of the box. Border color *is* tool state: running
shell blue `#87afff`, pending tool yellow `#ffffaf`, focused shell green
`#d7ffd7`, completed gray `#808080`. Gradient spinner: braille dots
(`⠋⠙⠹⠸⠼⠴⠦⠧⠇⠏`) color-cycled through the 6-color brand
gradient (`#4796E4 → #847ACE → #C3677F` family) at 33 fps via `tinygradient`.
Italic status text with live elapsed timer `(3.2s)` and persistent
`(esc to cancel)` hint. Tool status icon set: `✓ x ⊷ ? o -`. React Ink.

**Lazygit** — border style is user config: `rounded` (default) / `single` /
`double` / `bold` / `hidden`. Panel chrome packs metadata into the border
itself: title left, item count `(3)`, position `1 of 3` right. Zero-gap
shared borders between panels; `portraitMode: auto` stacks vertically below
100 cols. Commit graph lines colored per author. `animateExplosion: true`
(default) plays an explosion on `git clean -fd` — destructive operations get
drama as an easter egg. Nerd Fonts off by default; opt-in (`"2"`/`"3"`) adds
700+ per-filetype colored icons.

**k9s** — selected row is black-on-aqua reverse (`#000000` on `#00ffff`). The
7-state Kubernetes status map is the industry standard for infra dashboards:
New `#87cefa` LightSkyBlue, Running `#adff2f` GreenYellow, Pending `#ff8c00`
DarkOrange, Error `#ff4500` OrangeRed, Terminating `#9370db` MediumPurple,
Completed `#5f9ea0` CadetBlue, Unknown gray. Palette vocabulary is named CSS
colors, not brand hexes. Command/filter prompt overlays at the *top*, pushing
content down. Toggle indicators: LimeGreen `#32cd32` ON / gray OFF.

**btop** — inverted-corner title notch: `┐cpu┌` on the top edge (tees flipped
outward), unique in the corpus. Graph fidelity ladder, three render modes
under one gradient: braille (2×4 dot matrix per cell, highest), half-block
(`▄ ▀ █ ▟ ▙`), tty (`░ ▒ ▓ █` four densities, max compatibility) — all
colored green `#77ca9b` → yellow `#cbc06c` → red `#dc4c4c` by value. Per-box
accent hues (cpu `#556d59`, mem `#6c6c4b`, net `#5c588d`, proc `#805252`).
Dotted vertical divider `╎` (U+254E) between columns. Zero padding; adjacent
boxes share edges.

**Minimal (Vercel/Linear-inspired)** — the neutral scale does all hierarchy:
50 `#1a1a1a` → 100 `#2a2a2a` (borders) → 200 `#444444` (disabled) → 300
`#666666` (placeholder) → 400 `#888888` (secondary) → 500 `#ededed` (body).
Focus = reverse video + `▸` prefix. Inputs are bottom-border-only. Progress
bar caps `▕ … ▏`. Laws: no emoji (inconsistent widths break alignment), "don't
animate anything except spinners and progress bars", "don't use background
colors for emphasis".

**Retro** — CRT phosphor variants in one theme file: amber (fg `#ffb000`,
muted `#665500`, accent `#ff6600`) and green phosphor (fg `#33ff00`, muted
`#1a7700`, accent `#66ff33`). Direct evidence that a phosphor-monochrome
family with slot-swapped foregrounds is a recognized theme shape. Typewriter
reveal (30 ms/char) for important messages; error flicker (dim→bright→dim,
50 ms × 3 cycles).

### 1.4 TEMPLATE.md — the authoring skeleton (binding for TermRock theme specs)

The repo's `TEMPLATE.md` (256 lines) is the exact contract every shipped spec
follows — the uniformity is what makes 16 specs cross-comparable. TermRock's
per-theme `DESIGN.md` files (binding delta §10.1) must follow this skeleton:

1. **Theme Overview** — mood / density (compact|balanced|spacious) / target /
   terminal minimum (16 | 256 | truecolor).
2. **Color Palette** — the 10 semantic roles × (hex, ANSI 256, ANSI 16, usage)
   plus a neutral scale table (steps 50–500 with assigned usages).
3. **Typography & ASCII Art** — figlet header font if any; 6-level hierarchy
   (H1/H2/H3/Body/Caption/Label) with style per level.
4. **Borders & Box Drawing** — primary + secondary border, 11-part parts table
   (corners, horizontal/vertical, cross, four tees), dividers (horizontal,
   vertical, section break).
5. **Components** — buttons (focused/unfocused/disabled), inputs
   (active/inactive/error), tables, lists/menus, panels, tabs, status bar —
   each with an ASCII rendering example.
6. **Layout & Spacing** — min/ideal width, panel padding, component gap,
   indent, alignment principles.
7. **Icons & Indicators** — purpose / icon / ASCII-fallback table.
8. **Animation & Motion** — spinners (exact frames + ms interval), transition
   policy, progress bar spec (glyphs, %/ETA policy).
9. **Agent Prompt Guide** — quick-reference value block + ≥3 copy-paste
   prompts that reproduce the design in an AI coding session.

Then a **Do / Don't** list carrying taste rules. Note what the template does
*not* have: no prose rationale sections, no history, no marketing. Tables and
ASCII renderings only.

### 1.5 tui-art (`docs/format-spec.md` + `docs/brainstorm.md`) — glyph assets as data

The repo's `docs/` (Chinese-language, v0.1, concept stage 2026-04 — no
implementation) specifies **tui-art**, a terminal visual-asset platform. The
insight transfers directly to TermRock's planned capability-gated glyph
catalog: glyphs should be *distributable data with declared variants*, not
hard-coded per widget.

The TOML asset format:

- **One TOML file per asset**, compiled to a JSON index + per-asset JSON +
  category bundles. `[meta]` carries a dotted semantic ID
  (`icon.status.success`, `component.border.rounded`, `spinner.dots`),
  category (`icon | component | logo | sprite | animation | font`), tags,
  license; `[size]` declares cell width/height.
- **Variant ladder per asset**: `[variants.ascii]`, `[variants.unicode]`,
  `[variants.halfblock]`, `[variants.braille]` — runtime selects the richest
  the terminal supports; fallback priority braille → halfblock → unicode →
  ascii. Same asset, four fidelities, declared side by side.
- **Color separated from glyph**: per-tier `[colors.16|256|truecolor]` maps —
  a character-aligned mask over the glyph art where each character keys into
  a per-tier palette (`.`/space = terminal default). Glyph shape and coloring
  evolve independently across the same quantization ladder as palettes.
- **Animation = frames + timing**: `[[animation.frames]]` each with its own
  variant set, plus `interval_ms` and `loop`.
- **Component assets are named-part sets**: a border asset declares
  `top_left / top_right / bottom_left / bottom_right / horizontal / vertical`
  (with per-variant overrides) — the machine-readable form of the DESIGN.md
  parts table.

The brainstorm doc adds the rationale and the detection chain: **capability
detection Nerd Fonts → Unicode extensions → box-drawing → pure ASCII**;
semantic naming by purpose, never codepoint; one asset definition generating
Go/Rust/Python/TS bindings. Its Nerd Fonts critique is structural: PUA glyphs
need a manually installed patched font, programs cannot detect at runtime
whether the user has it, some glyphs render with wrong widths, and only small
icons are covered — larger assets (logos, spinners, progress, sprites) are
unserved. Market gap confirmed by the whole §1 corpus: every spec hand-rolls
the same fallback tables.

**Steal:** adopt the tui-art TOML shape as the file format for TermRock's
glyph catalog — dotted semantic IDs, the four-rung variant ladder, per-tier
char-aligned color masks, frames+interval animation, named-part components,
TOML→JSON compile. TermRock supplies what tui-art lacks: the runtime
capability ladder (`terminal-capabilities.md`), contract tests, and
monochrome-safe defaults as law rather than convention.

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
   prototype. Start with phosphor. Authoring contract: the TEMPLATE.md
   skeleton in §1.4, followed exactly so TermRock theme specs stay
   cross-comparable with the 16-spec corpus.
2. **Glyph catalog as data** (tui-art TOML shape, §1.5): dotted semantic IDs,
   ascii/unicode/halfblock/braille variant ladder, per-tier char-aligned color
   masks, frames+`interval_ms` animation, named-part component assets,
   TOML→JSON compile — replacing hand-rolled per-widget fallback tables with
   one catalog the capability ladder quantizes.
3. **Monospace-standard contracts**: spacing scale `0 1 2 3 4 6 8`, breakpoint
   names, ≤2 SGR attributes per span, rounded corners only on decorative
   containers, monochrome-must-work + non-color-cue contract tests. Reject its
   double-line elevation rule (conflicts with focus law).
4. **Clack `prompt_chrome(state)` recipe** — one state→(glyph, role) function
   + guide-bar threading across all prompt/form/wizard surfaces; filled/hollow
   pairs as non-color completion cue.
5. **Fresh theme-file shape**: JSON, `inherits`, dual color repr (RGB |
   named | `Default`), `use_terminal_bg`, config-watch reload — adopt for
   TermRock theme loading.
6. **Prefix-mode palette** (`>`/`#`/`:`) with bottom hints line — upgrade
   `QuickOpen`.
7. **`KeyboardHelp`/`ShortcutHint` rendered from the binding table** (superfile
   `?` overlay + lazygit ambient bar as two density modes of one component).
8. **Tool-block border color as state channel** (§1.1, gemini/claude-code
   evidence): extend the `Border`/`BorderFocused` pair with process-state
   roles — running / pending / done plus a permission accent — for tool, diff,
   and execution blocks. `permission-trust.md` already accents permission
   chrome; generalize to all process blocks. Focus stays border-color-only;
   this adds *state*, not weight.
9. **Elevation = background ramp, 3–4 steps, never borders or shadows**
   (§1.1): adopt the ocean depth-scale / catppuccin base-mantle-crust /
   rose-pine base-surface-overlay shape as the surface-elevation law in
   `terminal-design-system.md`.
10. **Corner style is a theme slot; weight is not** (§1.1): themes choose
    rounded vs straight corners (retro identities may opt into double-line as
    *identity*, applied uniformly); state and focus never change glyph weight.
    Encode as a theme token next to the border roles.
11. **Inline mode preserving scrollback** as first-class session mode
    (FrankenTUI evidence) — kernel RFC.
12. **Lookbook screens = snapshot tests, formalized** (FrankenTUI showcase
    model).
13. **Phosphor intensity variants** (SilkCircuit model): phosphor / -soft /
    -glow / -dawn share role IDs. Retro spec (§1.3) adds evidence that a
    phosphor family with slot-swapped foregrounds (amber/green CRT) is a
    recognized theme shape — the variant mechanism covers it.
14. **`.tui`-style serializable widget tree + named theme** as the future
    registry interop format; audit catalog against TUI Studio's component list
    (Breadcrumb, Popover, Tooltip, Spacer).

Per repo law: primitives land in `widgets`; assembled surfaces (file-manager
composite, wizard session, palette) are `patterns` composites of them.
