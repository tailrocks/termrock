# TUI theme gallery — dominant color schemes, mapping algorithm, shipped set

**Status:** design SoT (reference evidence + binding gallery plan)
**Audience:** design, implementers
**Method:** palette values verified against official source repos
(August 2026): `catppuccin/palette`, `folke/tokyonight.nvim`, nordtheme.com
docs, `morhetz/gruvbox`, `rose-pine/palette` + `rose-pine/neovim`,
draculatheme.com spec, `sainnhe/everforest`, `rebelot/kanagawa.nvim`.
**Related:** [`tui-design-research-2026-08.md`](./tui-design-research-2026-08.md)
(token system), [`tui-design-specs-2026-08.md`](./tui-design-specs-2026-08.md)
(DESIGN.md format per theme), [`terminal-aesthetics-landscape-2026-08.md`](./terminal-aesthetics-landscape-2026-08.md)

---

## 1. Why a gallery, and why these

CLAUDE.md law: phosphor stays the default, and the default must never prevent
full re-themability. Community evidence (August 2026) shows what
"re-themable" means in practice: the same ~8 schemes appear in every praised
terminal app's theme set — Catppuccin ×4 in
[yazi-rs/flavors](https://github.com/yazi-rs/flavors) and superfile; Tokyo
Night, Gruvbox, Kanagawa, Nord, Everforest, Catppuccin ×2 as
[opencode built-ins](https://opencode.ai/docs/themes/); Dracula across
hundreds of ports. Catppuccin alone has **457 ports**. A component library
that ships these as first-class themes inherits the ecosystem's muscle
memory; one that ships only its brand palette gets themed-around anyway.

**Muted beats neon for long sessions** (the dominant family — Catppuccin,
Nord, Gruvbox, Rosé Pine, Everforest, Kanagawa — holds saturation ≈20–45%):
luminance contrast does the hierarchy work; halation/chromatic aberration on
dark terminals disappears; 256-quantization error stays invisible (quantizing
a pastel moves it less than quantizing a neon); red/green colorblind users
keep luminance separation as backup. Neon works sparse and brief:
screenshots, demos, one signature accent in a statusline. Rule for every
shipped theme: **≤2 slots above ~85% saturation**; the more data-dense the
screen, the lower the ceiling.

---

## 2. Canonical palettes (verified hexes)

### 2.1 Catppuccin — the dominant system

26 named colors per flavor, 4 flavors sharing **identical role structure** —
implement once, swap contrast. Core roles:

| Role | Mocha | Macchiato | Frappé | Latte (light) |
|---|---|---|---|---|
| base | `#1e1e2e` | `#24273a` | `#303446` | `#eff1f5` |
| mantle | `#181825` | `#1e2030` | `#292c3c` | `#e6e9ef` |
| crust | `#11111b` | `#181926` | `#232634` | `#dce0e8` |
| surface0 / 1 / 2 | `#313244` / `#45475a` / `#585b70` | `#363a4f` / `#494d64` / `#5b6078` | `#414559` / `#51576d` / `#626880` | `#ccd0da` / `#bcc0cc` / `#acb0be` |
| overlay0 / 1 / 2 | `#6c7086` / `#7f849c` / `#9399b2` | `#6e738d` / `#8087a2` / `#939ab7` | `#737994` / `#838ba7` / `#949cbb` | `#9ca0b0` / `#8c8fa1` / `#7c7f93` |
| text | `#cdd6f4` | `#cad3f5` | `#c6d0f5` | `#4c4f69` |
| subtext1 / subtext0 | `#bac2de` / `#a6adc8` | `#b8c0e0` / `#a5adcb` | `#b5bfe2` / `#a5adce` | `#5c5f77` / `#6c6f85` |
| mauve (signature) | `#cba6f7` | `#c6a0f6` | `#ca9ee6` | `#8839ef` |
| blue | `#89b4fa` | `#8aadf4` | `#8caaee` | `#1e66f5` |
| green | `#a6e3a1` | `#a6da95` | `#a6d189` | `#40a02b` |
| yellow | `#f9e2af` | `#eed49f` | `#e5c890` | `#df8e1d` |
| peach | `#fab387` | `#f5a97f` | `#ef9f76` | `#fe640b` |
| red | `#f38ba8` | `#ed8796` | `#e78284` | `#d20f39` |
| teal / sky / sapphire | `#94e2d5` / `#89dceb` / `#74c7ec` | `#8bd5ca` / `#91d7e3` / `#7dc4e4` | `#81c8be` / `#99d1db` / `#85c1dc` | `#179299` / `#04a5e5` / `#209fb5` |
| lavender | `#b4befe` | `#b7bdf8` | `#babbf1` | `#7287fd` |

Why dominant: explicit 3-step surface ladder + 3-step overlay ladder (maps
1:1 onto component elevation and border/disabled states); machine-readable
SoT (`palette.json`) + strict port style guide; pastel mid-saturation
survives screenshot compression. Warning mapping note: yellow `#f9e2af`
exceeds 80% luminance on dark base — use **peach** for warning (see §4.5).

### 2.2 Tokyo Night

| Role | Night | Storm | Day (light) |
|---|---|---|---|
| bg | `#1a1b26` | `#24283b` | `#e1e2e7` |
| bg_dark (sidebar) | `#16161e` | `#1f2335` | `#d0d5e3` |
| bg_highlight | `#292e42` | `#292e42` | `#c4c8da` |
| fg | `#c0caf5` | `#c0caf5` | `#3760bf` |
| fg_dark | `#a9b1d6` | `#a9b1d6` | `#6172b0` |
| comment (muted) | `#565f89` | `#565f89` | `#848cb5` |
| fg_gutter (faint) | `#3b4261` | `#3b4261` | `#a8aecb` |
| blue (primary) | `#7aa2f7` | `#7aa2f7` | `#2e7de9` |
| cyan | `#7dcfff` | `#7dcfff` | `#007197` |
| magenta / purple | `#bb9af7` / `#9d7cd8` | same | `#9854f1` / `#7847bd` |
| green | `#9ece6a` | `#9ece6a` | `#587539` |
| yellow (warn) | `#e0af68` | `#e0af68` | `#8c6c3e` |
| orange | `#ff9e64` | `#ff9e64` | `#b15c00` |
| red / red1 (error) | `#f7768e` / `#db4b4b` | same | `#f52a65` / `#c64343` |
| teal (hint) | `#1abc9c` | `#1abc9c` | `#118c74` |

**Selection formula worth stealing:** `bg_visual = blend(blue0 #3d59a1 over
bg at 40%)` — selection is a computed blend, never a raw slot. Git roles:
add `#449dab`, change `#6183bb`, delete `#914c54`.

### 2.3 Nord

16 colors, 4 groups. Polar Night: nord0 `#2e3440` (bg), nord1 `#3b4252`
(surface), nord2 `#434c5e` (raised), nord3 `#4c566a` (border). Snow Storm:
nord4 `#d8dee9`, nord5 `#e5e9f0`, nord6 `#eceff4` (fg ramp). Frost: nord7
`#8fbcbb`, nord8 `#88c0d0` (signature cyan), nord9 `#81a1c1`, nord10
`#5e81ac`. Aurora: nord11 `#bf616a` (danger), nord12 `#d08770`, nord13
`#ebcb8b` (warning), nord14 `#a3be8c` (success), nord15 `#b48ead`.

Lowest saturation of the set — long-session comfort benchmark; accents are
muted pastels, weak for dense charts. Dark-only officially (Snow Storm is a
text ramp, not a light theme).

### 2.4 Gruvbox

| Role | Dark | Light |
|---|---|---|
| bg0 (hard/normal/soft) | `#1d2021` / `#282828` / `#32302f` | `#f9f5d7` / `#fbf1c7` / `#f2e5bc` |
| bg1 / bg2 | `#3c3836` / `#504945` | `#ebdbb2` / `#d5c4a1` |
| bg3 / bg4 | `#665c54` / `#7c6f64` | `#bdae93` / `#a89984` |
| gray (muted) | `#928374` | `#928374` |
| fg0 / fg1 | `#fbf1c7` / `#ebdbb2` | `#3c3836` / `#504945` |
| red | `#fb4934` | `#cc241d` |
| green | `#b8bb26` | `#98971a` |
| yellow | `#fabd2f` | `#d79921` |
| blue | `#83a598` | `#458588` |
| purple | `#d3869b` | `#b16286` |
| aqua | `#8ec07c` | `#689d6a` |
| orange | `#fe8019` | `#d65d0e` |

Warm retro-earth; the only scheme whose light mode is as loved as its dark.
3 contrast levels × 2 polarities.

### 2.5 Rosé Pine

Roles named by **function** (muted/subtle/highlight ladders), not hue —
closest in spirit to a component-library role model after Catppuccin.

| Role | Main | Moon | Dawn (light) |
|---|---|---|---|
| base | `#191724` | `#232136` | `#faf4ed` |
| surface | `#1f1d2e` | `#2a273f` | `#fffaf3` |
| overlay | `#26233a` | `#393552` | `#f2e9e1` |
| hl_low / med / high | `#21202e` / `#403d52` / `#524f67` | `#2a283e` / `#44415a` / `#56526e` | `#f4ede8` / `#dfdad9` / `#cecacd` |
| text | `#e0def4` | `#e0def4` | `#464261` |
| subtle | `#908caa` | `#908caa` | `#797593` |
| muted | `#6e6a86` | `#6e6a86` | `#9893a5` |
| love (danger) | `#eb6f92` | `#eb6f92` | `#b4637a` |
| gold (warning) | `#f6c177` | `#f6c177` | `#ea9d34` |
| pine | `#31748f` | `#3e8fb0` | `#286983` |
| foam (info) | `#9ccfd8` | `#9ccfd8` | `#56949f` |
| iris (signature) | `#c4a7e7` | `#c4a7e7` | `#907aa9` |

### 2.6 Dracula

Background `#282a36`, Current Line/Selection `#44475a`, Foreground
`#f8f8f2`, Comment `#6272a4`, Cyan `#8be9fd`, Green `#50fa7b`, Orange
`#ffb86c`, Pink `#ff79c6`, Purple (signature) `#bd93f9`, Red `#ff5555`,
Yellow `#f1fa8c`.

Only **10 slots — no surface ladder, no muted text ramp**. Highest
saturation in the gallery (green `#50fa7b` is near-neon). Dark-only. Wins on
brand recognition; surface/border steps must be derived per §4.1.

### 2.7 Honorable mentions

- **Everforest**: dark bg0 `#2d353b`, ladder `#343f44/#3d484d/#475258`, fg
  `#d3c6aa`, desaturated mid-tone accents (red `#e67e80`, green `#a7c080`,
  blue `#7fbbb3`, yellow `#dbbc7f`); light medium bg0 `#fdf6e3`, fg
  `#5c6a72`. The comfort-branding "Gruvbox of the 2020s".
- **Kanagawa**: wave bg ladder `sumiInk1 #181820 → sumiInk5 #363646`, fg
  `fujiWhite #dcd7ba`, signature `oniViolet #957fb8` + `crystalBlue #7e9cd8`
  + `springGreen #98bb6c` + `waveRed #e46876`; light lotus variant. Strongest
  new momentum after Catppuccin.
- **Solarized**: historically important, dual-polarity by construction, but
  low-contrast-by-design reads muddy in dense TUIs — **skip the gallery**.
- **Cyberpunk/neon** (Synthwave '84 `#262335`/`#ff7edb`/`#36f9f6`): marketing
  and splash states only; fails as a system (§1).

---

## 3. Every scheme has the same shape

| Slot family | Catppuccin | Tokyo Night | Nord | Gruvbox | Rosé Pine | Dracula |
|---|---|---|---|---|---|---|
| bg ramp | crust/mantle/base | bg_dark1/bg_dark/bg | nord0 | bg0 hard/normal/soft | base | background |
| surface ramp | surface0–2 | bg_highlight | nord1–2 | bg1–2 | surface/overlay | — (derive) |
| border/disabled | overlay0–2 | fg_gutter | nord3 | bg3–4 | hl_med/high, muted | selection |
| fg ramp | text/subtext1/0 | fg/fg_dark/comment | nord4–6 | fg0/1, gray | text/subtle/muted | fg/comment |
| accents | 14 named | 15 named | frost+aurora | bright ×7 | 6 named | 7 |
| semantics | red/yellow/green/blue | red1/yellow/green/blue2 | aurora | red/yellow/green/blue | love/gold/pine/foam | red/yellow/green/cyan |

This convergence is the design input for TermRock's authoring surface: a
`PaletteSpec` of ~10 slots + accent set compiles into the full `Role` ladder
via §4. The 63-role runtime enum stays; the *authoring* surface shrinks.

---

## 4. Palette → semantic role mapping algorithm (binding)

Source slots to TermRock roles, in order:

1. **Luminance-sort bg slots.** `Canvas` = the intended base (never
   crust/bg_dark1 — reserve darkest for shadows/frame). Next steps →
   `Surface` (panel bg) → `Elevated` (overlay bg). If the source has one bg
   (Dracula): derive `Surface = mix(bg, text, 6–10%)`,
   `Elevated = mix(bg, text, 12–16%)` in OKLab.
2. **Fg ramp.** `Text` = primary fg; `TextMuted` = the comment/subtle slot —
   verify 4.5:1 ≤ contrast(Text) and ≥4.5:1 for muted on Canvas,
   promote/demote a ramp step if outside. `TextFaint` = gutter/disabled slot
   (≥1.8:1 to stay visible, no upper floor).
3. **Border.** Pick the neutral slot between surface-top and fg-bottom
   (overlay0, fg_gutter, nord3, bg3, hl_med). If none:
   `Border = mix(text, background, 15–25%)`. `BorderFocused` = accent (hue
   shift, never glyph weight — TermRock focus law).
4. **Accent (primary).** The scheme's signature hue as its own ports use it
   for selection/links: Catppuccin mauve (lavender if less pink wanted),
   Tokyo Night `#7aa2f7`, Nord nord8, Gruvbox `#83a598`, Rosé Pine iris,
   Dracula purple. Never let red/orange become primary.
5. **Semantics by hue bucket.** red → danger; yellow → warning **unless**
   yellow luminance >80% on dark bg — then orange/peach → warning and yellow
   stays a highlight (Catppuccin `#f9e2af` is the known offender, use peach
   `#fab387`); green → success; the blue/cyan/teal family member closest to
   500 nm that is not already accent → info.
6. **Selection/hover are computed, never raw slots.**
   `Selection = blend(accentDeep, bg, 25–40%)` (Tokyo Night's
   `bg_visual = blue0@40%` is the canonical formula); hover = surface ladder
   +1 step. This kills the neon-fill class of bug structurally.
7. **Chart series.** Order accent hues by cyclic perceptual distance starting
   at primary accent: `[accent, green, yellow, cyan, magenta, orange, red]`;
   skip hues bound to visible semantics on the same screen; require ≥30° hue
   separation; alternate light/dark luminance parity between neighbors; pull
   only from the scheme's own accent set so charts stay on-palette.
8. **Validate.** APCA/WCAG pass for text pairs; ΔE spot-check after
   256-quantization; non-color cue present for every red/green pair (repo
   law: color never the sole indicator).

---

## 5. Dark/light pairing law

Schemes with native hand-tuned light variants (Catppuccin Latte, Gruvbox
light, Rosé Pine Dawn, Tokyo Night Day, Everforest light, Kanagawa lotus)
switch polarity by **keeping identical role IDs and swapping slot values —
never derive** a light theme by inversion. Expose `light() -> Option<Theme>`.
Nord and Dracula stay dark-only rather than shipping auto-inverted lights:
inversion preserves luminance but mangles muted schemes' hue harmony (Tokyo
Night gets away with generation only because folke hand-tuned the result).

## 6. Quantization strategy (truecolor → 256 → 16)

- Ship themes as RGB SoT + **build-time-generated** quantized tables, not
  runtime math. Detection ladder: `COLORTERM=truecolor` → `Color::Rgb`; else
  256 → `Color::Indexed`; else 16 → ANSI.
- **256**: quantize each *semantic role* (not the raw source palette) to
  nearest xterm-256 index using CIELAB/OKLab distance — RGB euclidean
  mispicks blues/purples. Snap near-gray backgrounds to the grayscale ramp
  (indices 232–255) when ΔE < ~2.
- **16**: collapse to `fg, bg, muted, border, 6 accents`; accents → fixed
  ANSI hue slots with bright variants for emphasis; semantic separation must
  survive clamping (success ≠ accent, danger ≠ warning); `BorderFocused`
  falls back from accent-hue to bold/White if slots collide.
- **Test matrix**: each scheme × 3 depth tiers, snapshot-tested.
  Quantization drift is how "it looked wrong on my terminal" bugs are born.
- Monochrome (no color) must remain fully functional — non-color cues carry
  every state (Monospace standard rule, TermRock law).

## 7. Shipped gallery (binding recommendation)

| # | Theme | Dark variants | Light | Why ship |
|---|---|---|---|---|
| 1 | **phosphor** (brand default) | default + intensity variants (`-soft`, `-glow`, see below) | phosphor-dawn | repo law: default theme |
| 2 | **catppuccin** | mocha (+ macchiato/frappé cheap — same structure) | latte | ecosystem default; role-system alignment |
| 3 | **tokyo-night** | night, storm | day | modern blue-dark standard; selection-blend formula |
| 4 | **gruvbox** | dark (+hard) | light | warm retro; best-loved light mode |
| 5 | **rose-pine** | main, moon | dawn | functional role naming maps 1:1 to component roles |
| 6 | **nord** | dark | — | minimal/arctic long-session benchmark |
| 7 | **dracula** | dark | — | screenshot-culture brand; saturation outlier (≤2 slots rule enforced) |
| 8 | **kanagawa** | wave | lotus | current momentum (Everforest dark-medium/light-medium is the defensible swap if comfort branding preferred) |

Phosphor intensity variants follow the SilkCircuit model
([`tui-design-specs-2026-08.md`](./tui-design-specs-2026-08.md) §9): one
semantic palette, ambient variants by saturation/luminance — default,
`-soft` (long sessions), `-glow` (OLED/marketing), `-dawn` (light). Same
role IDs, different slot values.

Every shipped theme gets a `DESIGN.md` in the cola-runner format (3-tier
color tables, ASCII fallbacks, Do/Don't, agent prompt guide) — see
`tui-design-specs-2026-08.md` §1.2.

## 8. Implementation deltas

| Priority | Work |
|---|---|
| P0 | `PaletteSpec` authoring struct (~10 slots + accent set) → compiles to full `Role` ladder via §4 algorithm |
| P0 | Port phosphor through the algorithm (proves it round-trips; fixes `Selection` neon fill via §4.6 computed blend) |
| P1 | Ship gallery themes 2–5 (Catppuccin mocha, Tokyo Night, Gruvbox, Rosé Pine) with `light()` where native |
| P1 | Build-time quantization tables + scheme × depth snapshot matrix |
| P2 | Gallery themes 6–8; phosphor intensity variants |
| P2 | `DESIGN.md` per theme; in-app `ThemePicker` preview-before-commit (already in component plans) |
| P2 | JSON theme loading with `inherits` + `use_terminal_bg` (Fresh shape, `tui-design-specs-2026-08.md` §4) |
