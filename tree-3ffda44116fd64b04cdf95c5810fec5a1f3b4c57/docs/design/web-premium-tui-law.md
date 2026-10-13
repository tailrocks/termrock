# Web-premium → TUI translation law

| Field | Value |
|-------|-------|
| **Status** | Binding design law for the "feels like it's NOT a terminal" quality bar. |
| **Date** | 2026-08-14 |
| **Question answered** | How do TermRock widgets reach the perceived quality of premium web AI interfaces — **Kimi, Grok, Amp, Claude.ai, ChatGPT, Linus, v0, Raycast** — *as a Rust TUI on Ratatui, under the phosphor style?* |
| **Relationship** | Sits *above* [`tui-design-research-2026-08.md`](./tui-design-research-2026-08.md) (design law v2) and [`termrock-design-language.md`](./termrock-design-language.md) (interaction SoT). This file defines the **quality target and its terminal translation**; those define the grammar and tokens. The [`tui-design-research-implementation-annex.md`](./tui-design-research-implementation-annex.md) then applies this law component-by-component. |
| **Method** | Grounded in live fetches + teardowns of the named products (see §8), not generic design theory. |

> Read §1 (the target), §2 (what makes them feel non-terminal), §3 (the 15 binding
> property translations), §4 (the cross-cutting rules), then the per-component annex.
> Everything below is terminal-physics-grounded: every web property gets a concrete
> Ratatui rendering rule under phosphor.

---

## 1. The target — verified real data from the products

Captured live (metadata, rendered DOM, teardowns):

**Grok** — `theme-color` `#f9f8f7` (warm off-white light) / `#1e1f22` (near-black cool
dark). Font **UniversalSans**, weight ladder **400 / 550** (the "550" semi-bold — *not*
600/700 — is the premium tell: hierarchy stepped by subtle weight, not heavy jumps).
Split into **Display** vs **Text** cuts. Next.js/Vercel. `interactive-widget=resizes-content`
(composer grows the viewport, doesn't shrink it).
- **Composer** (assistant-ui clone, verified): max-w-3xl (~768px), pill container
  (`rounded-4xl`), surface `#f8f8f8` light / `#212121` dark, **1px INSET ring** border
  `#e5e5e5`/`#2a2a2a` — *recessed* depth, not flat outline. **Three discrete states**
  (empty / typing / running) drive everything; children morph via `scale-0`/`opacity-0`/
  `max-w-0`.
- **Send** = 36px circle, **INVERTED contrast** (dark fill light-mode / white dark-mode) —
  the only strong signal of a shippable state. Right-rail cycles **Mic → Send → Cancel**
  in ONE slot (single-button morphing). Model-tier label **retreats (collapses width) as
  you type** (contextual collapse). Per-message edit/copy/reload appear **only on hover**.
  Streaming metrics (first-token, tokens/sec) in a **hover tooltip**, never the body.

**Kimi** — two-panel: ~240–260px sidebar (New Chat + `Ctrl K` kbd chip → Plugins /
Scheduled Tasks / Collapse → Kimi Work / Code / Claw → app/about/lang/help), centered
max-width composer, minimal top bar. Composer placeholder text is conversational ("Ask anything, or do an Agent task…").
(conversational — "Ask anything, or do an Agent task"). Empty state = centered SVG scene + an "Explore inspiration — slide to explore" prompt + suggestion
cards. Component DNA from JS chunks reveals the premium primitives: **RunningText**
(animated text), **RecommendPromptList** (suggestion cards), **RiveImg** (Rive motion),
PopoverMenu, Tooltip, ImageThumbnail, Modal, Loading — each a separate, modular chunk.

**Shared premium DNA** (Kimi + Grok + Amp + Claude.ai + setproduct anatomy study):
content column hard-capped **720–768px** (~72 chars, WCAG cap 80), line-height **~1.6**;
**7 honest message states** (Queued/Thinking/Streaming/Complete/Error/Regenerating/
Stopped); **flat full-width messages with actor differentiation, NOT SMS bubbles**
("bubbles undermine the tool framing"); streaming **caret** is the cheapest alive signal;
**optimistic** user-message render + **first-token <800ms** target; auto-scroll locks when
user scrolls up + floating "jump to latest"; **one** morphing primary, **one** jewel accent
on near-monochrome structure.

**The fidelity axis** (Signature-Flicker research): the #1 "cheap TUI" tell is
**flicker** from poor incremental re-rendering. Claude Code rewrote its renderer (kept
React model); Amp went alt-screen (flicker-gone but loses native select/find/scroll); pi
proves flicker-free differential *and* terminal-native can coexist. **Ratatui's Buffer
diff IS the surgical-update mechanism** — preserve it, never clear-then-redraw.

---

## 2. What makes them feel "not a terminal" (the signals to reproduce)

1. **Radical calm default** — mostly empty calm surface, ONE focal element; no mode chips,
   no vocabulary noise; you can just act.
2. **Depth from surface fill steps, not borders** — containers sit on a lighter/darker
   field than their parent; the fill does the containing, not a box.
3. **Flat, confident hierarchy** — few levels, stepped by weight + value, not dramatic
   size jumps.
4. **Restraint as luxury** — near-monochrome structure, ONE jewel accent reserved for the
   single current intent; color scarcity makes small moments read as expensive.
5. **Contextual disclosure** — actions/chrome appear only on hover or when the element owns
   intent; the default view is maximally clean, power features one step away.
6. **Single morphing primary** — the one shippable affordance changes identity in place
   rather than showing three controls.
7. **Honest state visibility** — model/state/progress shown before cost, not buried;
   streaming performance transparent but non-cluttering.
8. **Seamless, interruptible transitions** — state changes retarget mid-flight, no flash,
   no jank, no full repaint.
9. **Latency is hidden** — optimistic render + first-token masking; never a frozen frame.
10. **Fake material depth without real shadows** — stacked elements suggest z-order via
    offset/overlap/dim.
11. **Pill/chip consistency** — every small interactive unit shares one rounded grammar.
12. **Sustained readability** — capped width, generous line separation, body never competes
    with chrome.
13. **Flicker-free surgical updates** — nothing visibly repaints that didn't change; the
    surface feels solid. **The single largest cheap-vs-premium tell in a terminal.**
14. **Glyph/type craft** — precise alignment, consistent weight, width-correct truncation,
    no misaligned wide chars — exactness accumulates into "made with care."

---

## 3. The 15 property translations (binding)

Each maps a web-premium property → a concrete terminal rendering rule under phosphor.
Tokens referenced: `Role` enum, `GlyphSet`, `SpacingScale`, `Density`, recipes
(`PanelRecipe`/`InputRecipe`/`ButtonRecipe`/`ListRowRecipe`).

| # | Web property | Terminal rule under phosphor |
|---|--------------|------------------------------|
| **P1** | Generous whitespace (max-width 720–768, line-height ~1.6, 65–72 chars) | Cap content column at `BreakpointScale::comfortable` (~80 cols), center in pane, leave **2-cell Canvas gutters**; refuse edge-to-edge body text. Vertical rhythm = `pad_y` + **1 intentional blank Canvas spacer row** between sections. Composer docked with **≥2 blank Canvas rows** above last message. Text **never touches a border glyph** (min 1-cell inset via `SpaceScale`). |
| **P2** | Restraint — near-monochrome + ONE jewel accent | **Accent budget ≤2** accent-forward regions/viewport (focused owner's `BorderFocused` + ONE live element). Structure = graphite fill ladder + Fg/Muted/Faint. Only the primary confirm may be solid Accent (bg Accent / fg INK / BOLD); secondaries ghost/outline. Status hues muted-leaning, never brand green. |
| **P3** | Surface depth ladder (Kimi base / tinted sidebar / elevated card) | Ship the phosphor ladder as **FILLED** roles: Canvas `#0a0c0a` → Surface `#121612` → Raised `#1a1f1a` → Elevated `#1e2620` → Sunken `#0d100d`. Each step +6–8 luminance, monotonic (test `ladder_is_monotonic`). Sidebar=Raised, content=Surface, composer=Raised, dialog/palette/toast=Elevated, input well=Sunken. Borders hairline `#2a332c`, never the container signal. **Backdrop dims 60% toward Canvas under every overlay.** |
| **P4** | Flat confident typography (Kimi 20–24 semibold; Grok 400/550) | **Four-level Fg ladder only** (no real px in a cell grid): Strong `#f0f5f0`+BOLD / Body `#d6e0d6` / Muted `#7a8a7a` / Faint `#4a574a`. Titles/selected=Strong; content=Body; secondary=Muted; timestamps/shortcuts=Faint. **Max ONE BOLD run/row.** Metadata NEVER Body-white. "Size" illusion via 1-cell spacer band above titles + `BLOCK_RAMP` for metric values. Placeholder gets own role `TextPlaceholder` (proposed). |
| **P5** | Hairline borders / inset rings, never heavy elevation | **Single-line Square-cap** (loved default), hairline `#2a332c`; the ONE owner → `#00ff41`. NEVER double/heavy/mixed. Real shadows impossible — depth = fill step + backdrop dim. Rounded = theme token (`BorderShape::Rounded`, `┌┐└┘`↔`╭╮╰╯`) for Grok-class consumers, **not the default**. |
| **P6** | Hover-revealed contextual actions | Default rows: primary label + trailing meta only. Hover (`HoverTint`)/focus reveal action glyphs (`⧉✎×`) FgMuted→BorderFocused. **Disabled actions OMITTED, not greyed** (lazygit). Suggestion chips dismissible, never block composer. |
| **P7** | Single morphing primary (Mic→Send→Cancel) | Composer send = **ONE cell-slot** morphing by state: empty `⏎` FgMuted → typing `⏎` solid Accent chip → running `⏹` Danger. Glyph+role swap on **same coordinates**, not three buttons. Only enabled/primary uses solid Accent fill. |
| **P8** | Contextual collapse (label retreats as you type) | While composer owns focus + has content, secondary chrome (mode badges, tier label, chips) **collapses to a 1-cell icon or dims to Faint**; re-expands when empty. Never removed — always 1 cell away. |
| **P9** | Honest state visibility | Model/mode label = FgMuted **trailing text on every assistant message**. Running state shows the **verb** (`running tests`) beside the spinner. Performance facts = FgFaint status strip / hover tooltip, never body. Never silently truncate context. |
| **P10** | Interruptible seamless motion (150–300ms, retargets mid-flight) | Every animation = FrameTick brightness/alpha tween **capped 30fps**, span-coalesced; entrance/exit **≤150ms** fade; state changes **retarget** the tween on same cells, never restart. `Motion::Reduced/Off` collapses to static accent with **zero info loss** (shape-before-color guarantees it). No decorative loops. |
| **P11** | Latency masking (optimistic + first-token <800ms) | On submit: render user message optimistically (actor-painted) + Queued placeholder (**Wait dot-pulse `⁙` + shimmer line, NEVER % bar**) in assistant slot. First token → morph Queued→Streaming (Stream shimmer caret). Token paints batched into **30–60ms** `coalesce_cells` windows. |
| **P12** | Fake depth without real shadows (Sonner stack) | Real per-cell scale impossible → fake by: (a) **fill-step** (overlay=Elevated over Backdrop dimmed 60%); (b) toast stacks: rear toasts 1 row lower, on **dimmer fill** (one step toward Canvas), **shorter rail, no icon** ("pebble" `✓`). **Max 3 depth layers** (Canvas→Surface→Elevated), no fourth tier. |
| **P13** | Pill/chip grammar consistency | **ONE chip recipe** across Tag/Chip/Token/kbd/suggestion: `⟨ label × ⟩` (angle=neutral Tag, ASCII `<…>`), `[● label]` (square=interactive Chip), `[ C-s ]` (space-padded kbd). Brackets=Border FgMaint faint; label=Fg/FgStrong; `×`=FgMuted→Danger on chip-focus. Selection ramp **glyph>weight>fill>color**. Mask unified to `●`, ASCII `*`. |
| **P14** | Flicker-free differential rendering | Ratatui Buffer diff IS the surgical path — **preserve it, never force full-frame clears mid-stream**. Never blank-then-paint visible content. Overlays occlude via fill, not clear. Streaming caret = localized cell tween, not frame-wide redraw. **Load-bearing for "feels solid."** |
| **P15** | Flat full-width messages, not bubbles | Messages full-width within centered column, differentiated by **actor rail `┃`** + Fg ladder, NOT bubble fills. Assistant/user body=Fg; code blocks on **Sunken** well. No rounded bubbles, no saturated fills. |

---

## 4. Cross-cutting binding rules (every widget obeys)

1. **Accent budget ≤2** per viewport (focused owner + ONE live element).
2. **One bright border per scene layer** — only the interaction owner uses `BorderFocused`.
3. **Surface ladder is FILLED** — no empty `Style::new()` on Canvas/Surface/Raised/Elevated/Sunken. Depth = fill steps. Backdrop dims 60% under overlays.
4. **Text never touches a border** — min 1-cell inset; Panel padding non-zero.
5. **Hierarchy by weight + value only** — four Fg levels; max ONE BOLD run/row; metadata never Body-white.
6. **Underline-free focus** — border swap + glyph ladder + bg wash; underline survives only on hovered links.
7. **Color is the LAST channel** — glyph + weight + word + position first; every state survives monochrome/NO_COLOR (ladder ○◎●◇).
8. **Motion = status, never decoration** — `MotionChannel{Work,Wait,Stream,Live,Static}`; 30fps, coalesced; reduced→static zero-loss; skeleton never spins; errors never animate.
9. **Flicker-free** — preserve Ratatui Buffer diff; no clear-then-redraw mid-stream.
10. **Honest state visibility** — show model/mode/verb/perf; no generic "loading"/"something went wrong"; errors name cause + one recovery; never fake-throttle a fast model.
11. **Contextual disclosure** — default maximally clean; actions on hover/focus; secondary collapses to 1 cell; disabled omitted not greyed.
12. **One scale governs all** — `Density → SpacingScale → pad/gap/corner/glyph`. Breathing rows are intentional Canvas bands.
13. **One recipe per family** — one chip, one overlay, one row, one kbd recipe; no parallel paint paths (inconsistency is a defect).
14. **Flat full-width messages** — actor rail + Fg ladder, not bubbles.
15. **Content column ~80 cols centered** — no edge-to-edge body; Canvas side-bands.
16. **One voice** — case, punctuation, and key notation are hierarchy channels on a cell grid, spent deliberately. See §4.1.

### 4.1 One voice — the microcopy standard (binding)

Two microcopy systems used to coexist with no arbitration: a terse terminal
voice and an app voice, sometimes inside one string. This clause arbitrates.

1. **Labels, buttons, titles: sentence case.** `Cancel`, `Sign in`,
   `Git output`, `Search settings…`. ALL-CAPS is never structure (allowed only
   where a SoT names it, e.g. sidebar section headers). Panel titles capitalize
   the first word: `Procs`, not `procs`.
2. **Hints: lowercase action verbs, keys as chips.** `[esc] cancel ·
   [enter] open`. `·` separates hint pairs only — never a key from its label.
   One verb per key where the meaning matches: `esc` = cancel (dismiss one
   layer), `enter` = confirm/open. No trailing periods.
3. **Key notation: one system.** Modifier-dash, lowercase key: `C-s`, `S-tab`,
   `M-x`; bare keys spelled `esc enter tab space ↑↓←→` (ASCII
   `up/down/left/right`). Painted strings never hardcode `Ctrl+S` or `⌘K` —
   they route through `widgets::kbd::format_chord` / `Kbd`, which owns platform
   and ASCII fallbacks. Documentation prose may spell "Ctrl+S" when explaining
   a binding. A chord separator inside one keycap is a space.
4. **Ellipsis resolves through the glyph catalog** (`GlyphSet::ellipsis()` /
   `Glyph::Ellipsis`), never as a bare `...` literal in a painted string, so
   both the `…` and `...` forms stay width-checked at the call site.
5. **Error copy: what failed, then one recovery.** `<what failed> — <recovery>`
   in sentence case, no terminal period for one-liners: `Could not reach the
   API — check connectivity and retry`. The `error · message` and
   `error: message` prefixes are dead: severity is carried by the glyph and
   role, not by the word "error".
6. **Placeholders: sentence case, conversational, ellipsis** —
   `Search settings…`, `Filter projects…`.
7. **Running verbs: lowercase gerund** beside the spinner — `⠹ running tests`,
   `streaming…`.
8. **`OK` is `OK`**, never `Ok`.

Two gates enforce the mechanical half: `design_gate.rs::no_bare_ellipsis_in_paint`
and `design_gate.rs::one_chord_notation`.

### 4.2 The information budget — radical calm, measured (binding)

A default frame shows **≤3 content zones**, spends **≤1 hint row** carrying
**≤5 chords**, keeps **metadata to ≤⅓ of visible rows**, and speaks in **≤8
content hues** — a hue that paints a status *glyph* is not a content hue, which
is the whole point of putting status in the glyph.

Everything else is **one keypress away** (focus, expand, or overlay) **and its
affordance is visible in the frame**: `i details`, `d details`, `s segments`.
Removing information from the default frame requires providing that keypress
path in the same commit. Detail is never deleted, only moved.

Safety information does not bend to the budget. A permission prompt's risk
line, a destructive confirmation's consequence, and an error's cause stay in
the default frame however crowded it gets.

Charts are exempt from the hue budget: series colors are data, not decoration.
Keyboard-path parity is exempt from the chord budget: where a surface advertises
the keyboard path for every pointer action, the list is an accessibility
contract and stays complete.

Three gates enforce the mechanical half: `design_gate.rs::pattern_hint_budget`,
`design_gate.rs::pattern_style_diversity`, and the contrast floor in
`style/contrast_floor.rs`, which keeps a quieted tone from becoming an
unreadable one.

---

## 5. The vocabulary (referenced throughout the component annex)

- **Focus grammar (underline-free):** `FocusEmphasis{BrightBorder, SelectionFill, FocusTint, Reversed, BoldKey, PillGlyph}` on `DesignSystem`. Three mechanisms: **border swap** (`Border`→`BorderFocused`), **glyph ladder** `○→◎→●` (idle→preview→committed), **bg wash** (`HoverTint`/`SelectionTint`). Bright-border-on-owner law.
- **MotionChannel:** Work (braille `⠋⠙⠹…` ~80ms + verb) / Wait (dot-pulse `⁙` ~240ms) / Stream (shimmer `∻≈∿〜` / caret `▎`) / Live (breathe ~2s) / Static (gravity: done/failed/offline).
- **Shape-before-color ladder:** Filled `●` (terminal) / Ring `◎◉` (in-flight) / Hollow `○` (idle) / Diamond `◇` (checkpoint/now-edge).
- **Surface ladder:** Canvas → Sunken → Surface → Raised → Elevated; Backdrop = `blend_toward(Canvas,0.6)`.
- **Density modes:** Comfortable / Compact / Dashboard → `SpacingScale`.
- **Loading modes (never conflated):** Skeleton (inert shape + shimmer sweep, **never spins**) / Spinner (Work + verb) / Optimistic-stale (dim content + `↻ updating`).
- **7 message states:** Queued / Thinking / Streaming / Complete / Error / Regenerating / Stopped.
- **Glyph catalog gaps to close:** left-half blocks `▏▎▍▌▋▊▉`, shade blocks `░▒▓`, unified mask `●`, slider glyphs promoted to catalog, checkbox `✓/☐`, selection gutter `▌`.

---

## 6. The composer (signature element — full spec)

Surface/Raised card with a **Sunken text well** inside, single-line Square border →
`BorderFocused` when composer owns keys. **Taller-on-focus** = +1 row (Kimi grow-on-focus),
capped then scrollable. Embedded action row on bottom border line: left = attach `+` ghost
chip (morphs `×` when menu open), trailing-right = the **ONE morphing send slot** (empty
`⏎` FgMuted → typing `⏎` solid Accent → running `⏹` Danger). Model-tier label = FgMuted
trailing text **collapsing to 1-cell icon when content present**. kbd chips `[⏎ send]`/
`[⌘↵]` in keycap recipe. Streaming: send→Danger-stop, mode badge muted, verb `streaming…`
FgMuted. Attachments = quiet chips (Raised + paired fg + faint `×`). Composer **docked
with ≥2 blank Canvas rows above** — never floats over last message. Placeholder =
conversational FgFaint. State machine **empty/typing/running** is the single driver; all
children read state and morph in place on the same coordinates.

## 7. Toast (Sonner-class, full spec)

Real per-cell scale impossible → fake depth: rear toasts **1 row lower**, **dimmer fill**
(one step toward Canvas), **shorter rail, no body** — icon-only "pebble" `✓ saved`. Front =
full Elevated + icon + body + rail. One-line preferred; severity by **icon glyph + fg only**
(`✓ ! i ⚠`), border muted (not accent), body soft Fg. Stacked presence: rear toasts
tween position via FrameTick (**interruptible** — retarget row-offset, don't restart).
Entrance ≤120ms fade/slide from edge; **no neon flash**. **Errors never animate** (gravity).
Success = ONE brightness pop then static. TTL 4s, pause when terminal lacks focus.
Dismiss = `esc`/`×`; **no swipe** (no pointer analog). **Observer-pattern API** — plain
importable `toast()`, no provider. Anchored bottom-right/center on Elevated over
non-dimmed Canvas (toasts float, don't dim the scene).

---

## 8. Sources studied (live fetches + teardowns)

- **Grok** — live `theme-color` `#f9f8f7`/`#1e1f22`, UniversalSans 400/550 (Display+Text), `interactive-widget=resizes-content`.
- **[Grok composer teardown (aiuxplayground)](https://aiuxplayground.com/teardowns/grok/composer)** — radical calm default, unified pill grammar, single-button morphing, contextual collapse, honest model visibility, filled-black send.
- **[Grok interface clone (assistant-ui)](https://www.assistant-ui.com/examples/grok)** — composer max-w-3xl, `rounded-4xl`, `#f8f8f8`/`#212121`, inset ring `#e5e5e5`/`#2a2a2a`, 3 states, inverted 36px send.
- **[AI chat interface anatomy (setproduct)](https://www.setproduct.com/blog/ai-chat-interface-ui-design)** — 720–768px cap, line-height ~1.6, 7 message states, caret signal, optimistic render, flat-not-bubbles.
- **[Sonner toast (emilkowal.ski)](https://emilkowal.ski/ui/building-a-toast-component)** — fake depth (translateY + scale), interruptible transitions, hover-expand, swipe momentum, frictionless `toast()` API.
- **[Signature Flicker (steipete)](https://steipete.me/posts/2025/signature-flicker)** — flicker = #1 cheap TUI tell; alt-screen vs incremental; Claude Code renderer rewrite; Amp alt-screen tradeoffs; pi gold standard.
- **Kimi** — live render: two-panel sidebar, conversational placeholder, SVG empty state, modular component chunks (RunningText, RecommendPromptList, RiveImg, PopoverMenu, Tooltip, Modal, Loading).
- **TermRock SoTs** — phosphor palette values, Square-cap binding default, underline-free grammar, MotionChannel, Role enum, recipes (read for grounding, not restated).

## 9. Open tensions (web patterns that cannot map 1:1 — chosen compromise)

- **Real radius** impossible at cell granularity → `BorderShape::{Square,Rounded}` glyph swap; Square stays loved default; no fake shading.
- **Real shadows** impossible → fill-ladder step + Backdrop dim, never border-weight/shading.
- **Scroll momentum** has no pointer analog → discreet keyboard scroll + quiet scrollbar; lock + "jump to latest".
- **Per-element transform** impossible → toast depth via row-offset + dimmer fill + shorter rail (not scale); "taller-on-focus" via +1 row; morph via glyph+role swap on same coords.
- **Hover** absent in most sessions → hover actions also reveal on focus + keyboard; never hover-only.
- **Per-cell alpha** unavailable → distinct role hexes on tuned ladder + `DIM`, not real alpha; shimmer = discrete brightness steps.
- **Font weight/size** is whatever the terminal renders → hierarchy = `BOLD` + Fg ladder; treat four-level Fg as the contract.
- **60fps** fragile → 30fps-capped FrameTick + span coalesce; `Motion::Reduced/Off` static zero-loss; animation never load-bearing for meaning.
- **Native select/find/paste** broken by alt-screen → TermRock (Ratatui full-screen) accepts the trade and **rebuilds equivalents** (custom selection, search, copy), document the choice, keep renderer diff-based.
- **Continuous gradients** quantize on 256/16-color → quantize-at-startup; truecolor-only subtle effects hidden on lesser terminals, replaced by nearest flat fill; ladder survives ANSI-16.

---

*Design-language analysis only. No proprietary source reuse. All hex/token values are
from public metadata, teardowns, or TermRock's own SoTs. Component-by-component application
lives in [`tui-design-research-implementation-annex.md`](./tui-design-research-implementation-annex.md).*
