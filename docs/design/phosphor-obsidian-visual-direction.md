# Phosphor Obsidian — visual audit and direction

**Status:** design SoT (audit of HEAD lookbook + `Theme::tailrocks_phosphor` + recipes)  
**Audience:** product, design, implementers  
**Related:** [`terminal-design-system.md`](./terminal-design-system.md), component anatomy, AGENTS cross-surface consistency  
**Not:** a CSS mood board. Specs are terminal-cell paint rules.

---

## 1. Executive verdict (current system)

| Criterion | Score (1–5) | Verdict |
|-----------|-------------|---------|
| Premium | **2** | Reads as toolkit demo + neon CRT cosplay, not a product skin. |
| Cohesive | **2** | List/Table/Tabs/dialogs share green but not a surface ladder or rhythm. |
| Intentional | **2** | Accent, selection, focus, success often **same** phosphor green. |
| Calm | **1** | Full-row `#00ff41` fills dominate; hard to rest eyes for hours. |
| Legible | **3** | High contrast on black works; selected rows reverse to black-on-neon (harsh). |
| Distinctive | **3** | Phosphor brand is memorable; overuse makes it generic “hacker TUI.” |
| Terminal-native | **3** | Glyphs/cells OK; many stories still look like framed widget islands. |
| Long sessions | **1** | Neon selection bars are not sustainable for ops/agent workdays. |

**One-line diagnosis:** TermRock’s *idea* of phosphor is strong; the *implementation* of phosphor paints intent on almost every interactive row. Quiet canvas never ships because Canvas/Surface roles are empty and Selection is a solid neon slab.

### Evidence from HEAD

**Theme** (`Theme::tailrocks_phosphor`):

- `Canvas` / `Surface` / `Elevated` / `Backdrop` → `Style::new()` (no ladder).
- `Selection` → **bg phosphor green, fg black**.
- `BorderFocused`, `Focus`, `Accent`, `Success`, `ActionFocused`, scroll thumb, tab underline → **same green family**.
- Lookbook SVG canvas is pure `#000000`.

**Lookbook SVGs (catalog):**

- `list-selection.svg`: selected “Beta” is a **full-width neon bar**; multi-check also phosphor; idle rows white on `#1c1c1c` gutters.
- `table-basic.svg`: selected row is a **full neon slab** with black ink; disclosure glyph inverted on that fill.
- Primary content often competes with chrome; secondary metadata is often same white as primary (no real muted ladder in stories).

**Code path gap:** `DesignSystem::phosphor()` prefers `SelectionChrome::Gutter`, but many stories/widgets still resolve paint through `Theme` selection **fill** or older recipes, and **published SVGs still show neon fill**. Visual truth for users is the lookbook/catalog, not the intent in design docs.

---

## 2. Analysis axes (current state)

### Visual hierarchy
Primary labels, checks, selection, and accents fight for the same green/white band. Headers rarely step down with bold-only or muted; separators are white rules, not quiet structure.

### Surface hierarchy
**Broken.** No durable canvas → surface → elevated difference in default phosphor. Everything floats on void black or ad-hoc `#1c1c1c` cells. Dialogs clear-and-border but do not sit on a distinct elevated field.

### Foreground hierarchy
White body + dim gray disabled. **Missing:** strong / muted / faint as a disciplined ladder for title vs meta vs timestamps.

### Accent usage
**Overused.** Accent = selection = focus border = success = active action. Meaning collapses to “green means important.”

### Selection vs focus
**Not distinct.** Selected row = neon fill. Focus often same green border or same fill. Keyboard focus-visible and multi-select membership blend into one “lit” look.

### Border usage
Mixed: some panels single-line (good); lookbook frames every story as a boxed demo. Risk of “boxes everywhere” when every pane always borders. Ownership (who has keys) should light **one** border, not all.

### Spacing / density / alignment
Cell density is mostly 1-row chrome (good for TUI). Rhythm is inconsistent: list gutter reserved sometimes, table selection steals the first cells with glyphs, badges right-align inconsistently across widgets.

### Component anatomy
Composed parts exist in code (leading/primary/secondary/badge/shortcut) but **paint hierarchy is weak**: secondary often same style as primary; loading/error not consistently quiet.

### Information grouping
Stories isolate single widgets → **widget demo feel**. Workbench patterns exist but catalog lead images are components in isolation.

### Typography-like emphasis
Bold used for strong and for selection fill ink. Underline used for focus in some recipes but drowned by neon fill.

### Status communication
Danger/warning/info exist as roles; success collides with brand accent. Destructive scope (what will be deleted) rarely framed—often just red label.

### Empty space
Black void without graphite surface makes empty states feel abandoned, not calm.

### Responsive contraction
Narrow stories exist; contraction is technical. Visual language doesn’t prescribe “tiny = primary + one cue only” as a calm rule across all surfaces.

### Consistency
List recipes and table paint diverge; Theme default vs DesignSystem phosphor diverge; SVG gate lags recipe work → **implementation-first drift**.

---

## 3. Failure modes (where it currently feels like…)

| Failure mode | Where it shows |
|--------------|----------------|
| **Widget demo** | Catalog SVGs: isolated List/Table/Form on pure black, no product chrome or task context. |
| **Generic hacker terminal** | Neon fill selection, green-on-black everything, CRT cosplay without restraint. |
| **Unrelated components** | Tabs use filled chips; list uses bars; dialog uses border-only; toast severity uses different chrome languages. |
| **Implementation-first UI** | Role array + widget-local styles; surface ladder empty; dual Theme vs DesignSystem. |
| **Web UI copied to terminal** | Full-row “selected button” fills like Material list rows; tab pills with solid active backgrounds. |
| **Excessive borders** | Every panel/demo framed; multi-pane workbenches risk four bright borders. |
| **Excessive accent** | Selection + focus + success + checks all phosphor green in one viewport. |

---

## 4. Phosphor Obsidian — visual direction

### Principles (binding)

1. **Quiet canvas, bright intent.** Structure is graphite and mute. Phosphor appears for *current intent only*: keyboard focus owner, primary live action, running/live badge—not every selected row.
2. **Accent is rare and meaningful.** Default count of phosphor-heavy regions per viewport ≤ **2** (e.g. focused panel border + one live status).
3. **Selection ≠ focus.** Selection = gutter glyph and/or deep green tint + strong text. Focus-visible = underline and/or focused border on the **owner** container.
4. **Borders mark ownership.** Single-line only. Inactive = gray; focused owner = phosphor border. No double-line. No border on every list row.
5. **Primary content dominates.** White/soft-green body for primary; muted for secondary; faint for meta.
6. **Active operations show state + verb.** e.g. `running · apply patch`, not only a green spinner.
7. **Destructive states show scope.** Danger border + body names the target (`Delete 3 files`), not only a red button.
8. **Color is never the only cue.** Glyphs, bold/dim, underline, reverse (mono), status letters always present.

### Palette (truecolor targets — reaffirm)

| Token | Hex / policy | Use |
|-------|----------------|-----|
| Canvas | Reset or `#0a0c0a` | App void |
| Surface | `#121612` | Panel body |
| Raised | `#1a1f1a` | Nested card |
| Elevated | `#1e2620` | Dialog / palette |
| Sunken | `#0d100d` | Input / code |
| Border | `#2a332c` | Quiet structure |
| Border focused | `#00ff41` | **Owner only** |
| Fg | `#d6e0d6` | Body |
| Fg strong | `#f0f5f0` + bold | Title / selected primary |
| Fg muted | `#7a8a7a` | Secondary |
| Fg faint | `#4a574a` | Meta / timestamps |
| Selection tint | `#14331a` | Optional row wash — **not** neon |
| Selection gutter | `▌` + accent fg | Default selection |
| Hover | `#1a221c` | Subtle |
| Danger | `#ff5e7a` | Error / destructive |
| Warning | `#f0c040` | Caution |
| Info | `#5ec8ff` | Informational |
| Success | `#5dffa0` | Done — **not** brand accent green slab |

### Global paint rules

| Situation | Paint |
|-----------|--------|
| Idle row | fg body; no fill |
| Selected (list unfocused) | gutter `▌` + fg strong; optional tint bg |
| Selected + list focused | gutter accent + **underline** primary; no full neon fill |
| Multi-checked | `☑` muted accent; not full-row green |
| Focused panel | single border `border_focused` |
| Inactive panel | single border `border` |
| Primary action | solid accent chip **only** when it is the default confirm |
| Secondary action | ghost / border / muted |
| Running | spinner glyph + muted verb text |
| Error | danger fg + `!` / `x` prefix |

Default `SelectionChrome::Gutter` everywhere collections paint. **Delete neon fill as default.**

---

## 5. Before / after by component (with ANSI mockups)

Legend for mockups:

- `[g]` = phosphor gutter `▌`
- dim text shown as lowercase-ish description; use comments for color
- `▌` green, body soft white, meta gray

### 5.1 List

**Before (catalog truth):** full neon bar on selected row; checks also neon; looks like a selected web list item.

```text
  [x] Alpha                         12 ms
▌[ ] Beta ████████████████████████ 28 ms   ← entire row #00ff41 / black ink
  [ ] Gamma                         …
```

**After (Obsidian):**

```text
  ☑  Alpha                          12 ms     ← checked muted, not selected
▌  Beta                             28 ms     ← gutter + bold/underline if focused
   Gamma                            4 ms      ← idle soft white; meta muted
```

**Spec:** `SelectionChrome::Gutter`; primary strong when selected; secondary/meta `fg_muted`; shortcut `fg_faint` or hint role; never `Role::Selection` full-row fill by default; multi-check uses glyph catalog not neon text.

---

### 5.2 Table

**Before:** selected row solid phosphor; process name sometimes green without being selection.

```text
  PID  Process          Region     CPU    State
  101  termrock         東京🧪     82.4%  run
▌422  bun-docs          us-east    9.2%  wait   ← full green slab
  509  shell            東京       4.4%  done
```

**After:**

```text
  PID  Process          Region     CPU    State
  101  termrock         東京       82.4%  run     ← success state glyph, not brand fill
▌ 422  bun-docs         us-east     9.2%  wait    ← gutter; primary bold; meta muted
  509  shell            東京        4.4%  done
```

**Spec:** header row `fg_muted` or bold once; sorted column: underline or `↑` muted, not green header fill; selected row gutter + optional tint; numeric columns faint/muted; status column uses status tokens + letter (`R`/`W`/`D`) for mono.

---

### 5.3 Tabs

**Before:** filled active tab background (web pill).

```text
 [ Files ]  Search   Git
 ████████
```

**After:** underline ownership; inactive quiet.

```text
  Files    Search    Git
  ─────                         ← underline border_focused / accent only under active
```

**Spec:** active = bold + underline (or bottom rule cell); inactive = muted; no solid tab chip default; optional left `▌` only if tab strip owns focus.

---

### 5.4 Dialog

**Before:** border often same as every other box; body white; no elevated field; danger not scoped.

```text
┌──────── Notice ────────┐
│ The operation completed│
│         [ OK ]         │  ← may be neon action
└────────────────────────┘
```

**After:** elevated surface; one focused border; danger names scope.

```text
┌────── ! Delete files ──────┐     ← danger border + ! non-color
│ Remove 3 paths from disk:  │     ← body soft white
│   src/old.rs               │     ← scope listed, muted paths
│   …                        │
│  esc cancel    [ Delete ]  │     ← primary danger chip only on confirm
└────────────────────────────┘
```

**Spec:** canvas Reset behind backdrop; dialog `elevated` fill; default OK = accent only if primary; danger variant = danger border + `!` title prefix; footer hint muted; loading = title `…` not spinning whole frame green.

---

### 5.5 Form

**Before:** fields look like free-floating labels; invalid often color-only; dense white.

```text
 Name *
 [________________]
 Email
 [________________]   ← invalid maybe red border only
```

**After:**

```text
 Name *
 › ________________        ← sunken well; cursor underline
   help text muted

 Email  ! required
 › ________________
   error: use work address   ← danger text + !; mono keeps !
```

**Spec:** labels strong or body; help faint; error line danger + glyph; required `*` always; disabled `⊘` + dim; focus = field underline or border_focused on active field only, not all fields.

---

### 5.6 Toast

**Before:** severity may use large green/red blocks; competes with selection language.

```text
┌──────────────────┐
│ ✓ Saved          │  green slab
└──────────────────┘
```

**After:**

```text
  ✓  Saved to disk          ← glyph + muted border; success fg on icon only
```

**Spec:** one-line preferred; icon carries status color; message body soft white; border muted not accent; auto-dismiss no neon flash.

---

### 5.7 Diff

**Before:** OK-ish bgs but may lean loud; markers must remain.

**After:**

```text
  @@ -12,4 +12,5 @@
    keep context muted
 -  removed line              ← danger fg on deep red-black tint
 +  added line                ← success fg on deep green-black tint
```

**Spec:** never rely on bg alone; always `+`/`-`/` `; hunk header faint; selected hunk gutter not neon fill of whole hunk.

---

### 5.8 VirtualGrid

**Before:** risk of every cell looking selected/hoverable like a spreadsheet skin.

**After:**

```text
      A        B        C
 1   10       20       30
 2   11   ▌   21_      31     ← cursor cell: gutter or underline, not full neon cell
 3   12       22       32
```

**Spec:** cursor = underline or reverse one cell + optional gutter; range selection = tint only; headers muted; O(visible) paint unchanged.

---

### 5.9 Command palette

**Before:** may look like a second neon list in a box.

**After:**

```text
┌─ Commands ─────────────────────┐  elevated
│ > git █                        │  sunken input
│ ▌ git status           ⌘↩      │  gutter select; shortcut faint
│   git commit                   │
│   git push                     │
│  type to filter · esc close    │  muted footer
└────────────────────────────────┘
```

**Spec:** query sunken; results use List Obsidian recipe; no fill selection; empty “No commands” muted centered.

---

### 5.10 Prompt composer

**Before:** chrome-heavy; mode badges neon; send always green.

**After:**

```text
┌ compose ──────────────── plan ─┐  mode badge muted/outline unless active mode owns intent
│ Fix the list selection fill…█  │  body dominant
│                                │
│ 📎 2 files   streaming…   ⏎ send│  chips muted; send accent only when enabled & focused
└────────────────────────────────┘
```

**Spec:** editor sunken or surface; border_focused only when composer owns keys; streaming lock dims send + shows verb `streaming`; attachments as quiet chips not neon.

---

### 5.11 Permission request

**Before:** risk of allow-looking green primary; default selection unsafe if index-based.

**After:**

```text
┌─ ! High risk ──────────────────┐  danger border
│ Write ~/.ssh/config            │  scope explicit
│ Agent: apply_patch             │  meta muted
│                                │
│  ( ) Allow once                │
│  ( ) Allow session             │
│  (•) Deny                      │  default High → Deny; non-color (•)
│                                │
│  ← → select · ⏎ confirm        │
└────────────────────────────────┘
```

**Spec:** decision identity not allow-first; High default Deny; accent never on Allow for High; danger border + `!`; outcomes pure messages.

---

### 5.12 Task rail

**Before:** every task status green/red noise; selected neon.

**After:**

```text
 Tasks
▌ ● running  apply list recipes    ← selected gutter; status glyph; verb in label
  ○ pending  rewrite table paint
  ✓ done     tree density indent
  × failed   dialog tokens migrate
```

**Spec:** one selected gutter; status via glyphs (`●○✓×`) + muted color; title body; no full-row fill; running uses spinner only on that row.

---

## 6. Cross-component cohesion rules (product)

1. **Max one focused container border** per scene layer.
2. **Max one neon-forward control** (primary action or live badge).
3. **Selection gutter language shared** by List, Tree, Table, VirtualGrid, Task rail, palette results.
4. **Meta always muted/faint** — never same white as primary.
5. **Catalog stories** must re-render under Obsidian tokens so SVGs match recipes (stale neon SVGs are product bugs).
6. **Lookbook “hero”** should show AgentWorkbench-class composition, not only isolated widgets—reduces demo feel.
7. **Slate / Day / HC** keep the same hierarchy rules; only hue polarity changes.

---

## 7. Implementation checklist (for engineers)

| Priority | Work |
|----------|------|
| P0 | Author `Theme::phosphor_obsidian` / `DesignSystem::phosphor_obsidian` with full surface ladder; Selection = tint or strong text, **not** neon bg fill. |
| P0 | Force default `SelectionChrome::Gutter` on all collection paint paths; kill default Fill. |
| P0 | Split Success from brand Accent (success softer mint). |
| P1 | Table/List/Tree/Grid/Tabs/Dialog/Form/Toast/Diff/Palette/Composer/Permission/TaskRail recipes apply Obsidian rules. |
| P1 | Regenerate lookbook SVGs + catalog; gate fails if selection fill is full-row phosphor. |
| P2 | Workbench flagship story as visual gold master. |
| P2 | Long-session review: 10-minute ops dashboard screenshot without neon fatigue. |

---

## 8. Acceptance mock (flagship viewport)

Target Agent Workbench feel (compact):

```text
┌ files ┐┌ transcript ───────────────────────── focused ┐┌ tasks ─┐
│src    ││ user                                      ││▌● apply │
│▌lib.rs││  quiet soft white                         ││ ○ review│
│ tree  ││                                           ││ ✓ done  │
│       ││ assistant                                 ││         │
│       ││  … explanation muted meta 12:04           ││         │
└───────┘│ tool  ● running  cargo test               │└─────────┘
         │   output sunken                           │
         ├───────────────────────────────────────────┤
         │ › fix selection fill █           plan  ⏎  │  composer owns focus border
         └───────────────────────────────────────────┘
  status  ready · 42 tok · branch main                 muted bar
```

Only **one** bright border (composer or transcript—whoever owns keys). Selection gutters quiet. No full neon rows.

---

## 9. What “done” looks like for design quality

- Screenshot test: average phosphor green pixel area drops sharply vs today’s list/table SVGs.
- Untrained user can point to “where is focus” and “what is selected” as **two different marks**.
- 8-hour ops session: no full-row neon.
- Catalog still demo-able but **same language** as product workbench.
- Distinctive without cosplay: graphite + rare phosphor, not Matrix screensaver.

---

## 10. Relationship to existing docs

- Token taxonomy and recipes: [`terminal-design-system.md`](./terminal-design-system.md)  
- This file: **visual audit + Obsidian paint authority + component mockups**  
- Implementation must update Theme roles and lookbook artifacts; design text alone does not fix catalog neon.
