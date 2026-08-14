# Plan 011: The catalog tells the truth — lookbook host parity, faithful exporters, visual baselines

> **Executor instructions**: Follow this plan step by step. Run every
> verification command and confirm the expected result before moving to the
> next step. If anything in the "STOP conditions" section occurs, stop and
> report — do not improvise. When done, update the status row for this plan
> in `plans/README.md`.
>
> **Drift check (run first)**: plans 002-010 DONE in `plans/README.md`.
> Re-locate every site with `rg` before editing.

## Status

- **Priority**: P2 (final; locks everything in)
- **Effort**: L
- **Risk**: MED (every preview artifact changes — intended)
- **Depends on**: plans/002-010
- **Category**: dx / design-infrastructure
- **Planned at**: commit `605217aa`, 2026-08-14

## Why this matters

"Visual truth for users is the lookbook/catalog" — and today the catalog
lies. Every lookbook render path builds `DesignSystem::from_palette(...)`,
which inherits `SelectionChrome::default()` (was `Fill`; plan 004 made it
`Gutter` — the lookbook must still adopt the PRESET, not the raw default);
every preview host `Clear`s the story rect to raw black so the surface
ladder never shows; the SVG exporter silently drops `BOLD` and `UNDERLINED`;
`Color::Indexed` renders as white, making quantized profiles unpreviewable;
the design inspector hardcodes `selection_chrome: "gutter"` and
`theme: "phosphor"` regardless of reality; the "deterministic preview check"
diffs a render against itself (no baseline anywhere in the repo); and the
docs' pixel-histogram helper already measures phosphor area but asserts only
lower bounds. Without this plan, plans 002-010 are invisible and
unenforced.

## Current state (leads verified by audit at `605217aa`)

- Host system: `crates/termrock-lookbook/src/host_frame.rs:59-61`
  (`HostFrame::system()` — "Sole paint authority"), `svg.rs:43-45`
  (`render_story_to_buffer`), `demo.rs:202` (`RolePalette::default()`), and
  **165** interactor sites `DesignSystem::from_palette(self.theme.clone())`
  (`interactors.rs` 64, `interactors/catalog.rs` 26, `remaining.rs` 19,
  `viewers.rs` 19, `applications.rs` 16, `extended.rs` 11, `workflows.rs` 9,
  `composites.rs` 1). Only `stories.rs:10358,10388` use
  `DesignSystem::phosphor()`.
- Per-story selection overrides (delete once host is right):
  `stories.rs:12304,12327,12363,12380,12404,13281,13327`.
- Clear-to-black: `svg.rs:70-74` (`Clear` + comment), `app.rs:315` (Clear
  after `app.rs:282-285` painted Surface), `demo.rs:336-342`; SVG page bg
  hardcoded `#000000` (`svg.rs:177,180,236`); `frame.rs:203-208` Reset→black.
- Exporter fidelity: `svg.rs:249-277` handles only `REVERSED`/`DIM`;
  `svg.rs:206-210` emits `<text>` without font-weight/text-decoration;
  `svg.rs:237` `Color::Indexed(_) => "#ffffff"`;
  `frame.rs:226-231` Indexed → white/black; both hardcode phosphor-flavored
  ANSI slots (`svg.rs:222`, `frame.rs:212`). The live path is correct:
  `frame.rs:151-191` + `docs/src/components/TerminalPreview.tsx:48-51,181,204-209`.
- Inspector lies: `app.rs:339-348` hardcodes
  `selection_chrome: "gutter"`, `capability: Truecolor`, `density`,
  `recipes`; `demo.rs:356` hardcodes `theme: "phosphor"`;
  `widgets/design_inspector.rs:68,103-109,117-128` never reads the system
  and shows no active tab.
- Story paint opt-outs: `stories.rs:13187-13199` (hint_bar blanks four Hint
  roles under phosphor), `:17884-17890` (backdrop story fakes the role),
  `:15979-15990` (status_bar hand-rolls modifiers), `:16991` (`Color::Black`
  literal).
- Gates: `.github/workflows/docs.yml:110-114` renders twice and diffs
  (self-diff; slate render never asserted); `mise.toml:44-66` gate has no
  lookbook step though `TESTING.md:10` claims it; `svg.rs:113-145`
  `check_svgs` is filename-inventory only (`let _ = theme;` at `:129`) with
  a misleading error message; docs checks (`docs/package.json:29`) assert
  painter mechanics, not language; `docs/tests/visual/previews.spec.ts:33-52`
  `paintMetrics` histogram used only for lower bounds (`:219-222`); no
  committed baseline anywhere (`docs/.gitignore:5`; old SVGs deleted in
  `eb02ba18`).

## Commands

| Purpose | Command | Expected |
|---|---|---|
| Fast gate | `mise run check` | exit 0 |
| Lookbook render | `cargo run -p termrock-lookbook -- render --out target/render-a` | exit 0 |
| Docs checks | `cd docs && bun run build` | exit 0 |
| Full gate | `mise run gate` | exit 0 |

## Scope

**In scope**: `crates/termrock-lookbook/src/**`,
`crates/termrock-lookbook-web/src/**` (only if frame encoding shared),
`crates/termrock/src/widgets/design_inspector.rs`,
`docs/tests/visual/previews.spec.ts`, `docs/scripts/` (new bless script if
chosen), `.github/workflows/docs.yml` (gate wiring), `mise.toml` (gate step),
`TESTING.md` (make the claim true), `migrations/0295-*.md` + `MIGRATING.md`
(if any public surface changes — likely none; then skip migration and say so
in the commit body).

**Out of scope**: widget/pattern paint (done), docs site content authoring.

## Git workflow

`main`; commits per step; `git commit -s`.

## Steps

### Step 1: One host system, preset-true

Add `fn lookbook_system(theme: RolePalette) -> DesignSystem` (in
`termrock-lookbook`): `DesignSystem::phosphor()` shape — i.e. the PRESET's
selection/density/motion — with the palette swapped
(`DesignSystem::from_palette(theme)` replaced by
`DesignSystem::phosphor().palette-swap`; if no palette-swap API exists, use
`from_palette(theme).selection(DesignSystem::phosphor().selection)` and note
it). Mechanically replace the 165 `from_palette(self.theme.clone())` sites +
`host_frame.rs:59-61` + `svg.rs:43-45` + `demo.rs:202`. Delete the 7
per-story selection overrides. Add test: `HostFrame::system().selection ==
DesignSystem::phosphor().selection`.

**Verify**: `rg -n "from_palette" crates/termrock-lookbook/src | wc -l` → ≤2 (the helper itself); test green.

### Step 2: Stories sit on the ladder

Replace the three `Clear` calls (`svg.rs:70-74`, `app.rs:315`,
`demo.rs:336-342`) with a `Role::Canvas` fill from the story's system; SVG
page background derives from the palette's Canvas (not `#000000`);
`frame.rs:203-208` Reset-bg encodes the palette Canvas RGB. Delete the three
story paint opt-outs (`hint_bar`, `backdrop`, `status_bar` hand-rolls) —
whatever they reveal about `HintDim`/`HintSeparator`/`Backdrop` under
phosphor, fix in the palette (plan 002's values should already be sane;
if a role still looks broken in the story, STOP and report the role).

**Verify**: `rg -n "render_widget\(Clear" crates/termrock-lookbook/src` → 0; hint_bar/backdrop stories render role-true.

### Step 3: Faithful exporters

`svg.rs`: emit `font-weight="700"` for BOLD and underline (text-decoration
or run-merged rect matching `TerminalPreview.tsx` `underlineSpans`); share
one `xterm256_to_rgb` table between `svg.rs` and `frame.rs` for
`Color::Indexed`; derive ANSI-16 slot colors from the active palette instead
of hardcoded phosphor hexes. `check_svgs`: reword failure to "story
inventory out of date"; drop `let _ = theme;`.

**Verify**: SVG of a bold+underlined cell contains the attributes (unit test
on the emitted string); Indexed256-quantized story renders non-white.

### Step 4: Inspector honesty

`design_inspector.rs`: `from_system` populates selection_chrome/density/
capability from the passed system; active tab = bold + `▸` marker
(underline-free). `app.rs:339-348` builds the frame from
`self.host.system()`; `demo.rs:356` theme field from the session palette id.

**Verify**: inspector story under the lookbook host shows the host's actual
chrome (test asserts the string matches `system.selection`).

### Step 5: Real gates

- Buffer-level goldens: commit JSON cell dumps (via `frame.rs::encode_buffer`)
  for ~15 flagship stories (list/selection, table, tabs, dialog, form,
  toast, transcript, composer, metrics tile row, permission, command
  palette, sidebar, statusbar, quick_open, setup_wizard step) under
  `crates/termrock-lookbook/goldens/`; a `mise run bless-previews` task
  regenerates; a test diffs current render vs golden.
- Phosphor budget: `previews.spec.ts` `paintMetrics` returns the
  `0x00ff41`-family pixel count; assert per-story upper bound (derive from
  post-fix renders; commit alongside goldens).
- Wire the lookbook determinism check + golden diff into `mise run gate`
  (making `TESTING.md:10` true) and keep the a/b determinism diff in CI.

**Verify**: `mise run gate` runs the golden diff; intentionally corrupt one
golden locally → gate fails → restore.

### Step 6: Regenerate + close

Regenerate all previews/artifacts the repo tracks (`cargo run -p
termrock-lookbook -- render`, docs pipeline); update story descriptions still
referencing old chrome; final `mise run gate`.

**Verify**: gate green; `git status` clean of unintended files.

## Test plan

Step-level tests above; golden set + bless task; budget assertions.

## Done criteria

- [ ] `mise run gate` exits 0 and includes the lookbook golden diff.
- [ ] `rg -n "from_palette" crates/termrock-lookbook/src` ≤ 2.
- [ ] SVG exporter emits bold/underline; Indexed mapped via shared table.
- [ ] Inspector reports live values.
- [ ] Goldens + budget thresholds committed; bless task documented in TESTING.md.
- [ ] `plans/README.md` updated (this is the last plan — mark the set complete).

## STOP conditions

- No palette-swap API and the `from_palette().selection(...)` fallback still
  diverges from the preset in other fields (density/motion) — report the
  field list.
- Golden diffs are platform-unstable (font metrics leak into cell dumps —
  they shouldn't; dumps are cell-level) — report with a diff sample.
- A story renders a role that still looks broken after plan 002 values —
  report the role; do not patch the story.

## Maintenance notes

- Goldens are the standing screenshot-identity gate (experience law
  "screenshot identity"); bless intentionally, in the same PR as the visual
  change, with the design rationale in the commit body.
- Registry publishing (`registry/official/`) should sequence behind this
  plan so first shipped items inherit truthful previews.
