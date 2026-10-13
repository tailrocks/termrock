# Plan 005: Overlay elevation — dialogs on elevated fill, dimmed backdrop, status-bar band, toast chrome

> **Executor instructions**: Follow step by step; verify each step; STOP
> conditions are binding. Update `plans/README.md` when done.
>
> **Drift check (run first)**: `git diff --stat 539e7d03..HEAD -- crates/termrock/src/widgets/dialog.rs crates/termrock/src/widgets/toast.rs crates/termrock/src/widgets/status_bar.rs`
> Widget excerpts below must still match (style/ churn from 001–004 is
> expected). On mismatch, STOP.

## Status

- **Priority**: P1
- **Effort**: M
- **Risk**: MED (modal visuals change for every consumer)
- **Depends on**: plans/001-surface-ladder-and-role-expansion.md, plans/003-spacing-activation.md
- **Category**: tech-debt (visual foundation)
- **Planned at**: commit `539e7d03`, 2026-08-12

## Why this matters

Overlays have zero elevation cues: dialogs `Clear` to the terminal default
background and bypass the `Surface` fill path entirely, the backdrop never
dims (`Backdrop::from_tokens` discards its tokens and returns Reset), toasts
hand-draw a severity-colored frame on a fill that is a no-op, and the status
bar's own role is empty so "the bar has no bar". Modal layering is the
single most recognizable mark of a rich application; this plan makes
elevation real using the roles from plan 001.

## Current state

Verified excerpts (`539e7d03`):

`widgets/dialog.rs:1258` — `Clear.render(area, buffer);` then
`dialog.rs:1292-1294`:

```rust
let block = panel.block();
let inner = block.inner(area);
block.render(area, buffer);
```

— the dialog renders `Panel::block()` directly, **bypassing** `Panel::paint`
and therefore any Surface fill.

`widgets/dialog.rs:507-548` — Backdrop:

```rust
impl Default for Backdrop {
    fn default() -> Self {
        Self {
            symbol: ' ',
            style: Style::new().fg(Color::Reset).bg(crate::style::DIALOG_BACKDROP),
        }
    }
}
...
pub fn dim_wash(ascii: bool) -> Self {
    Self {
        symbol: if ascii { '.' } else { '░' },
        style: Style::new().fg(Color::DarkGray).bg(crate::style::DIALOG_BACKDROP)
            .add_modifier(ratatui_core::style::Modifier::DIM),
    }
}
pub fn from_tokens(tokens: &DesignSystem) -> Self {
    let _ = tokens;
    Self::reset()
}
```

`DIALOG_BACKDROP = Color::Reset` (`style/mod.rs:63`) with a comment
explaining Reset-bg policy for terminal-theme compatibility — **keep the
Reset bg**; dimming comes from the wash glyph+fg, not from painting a solid
backdrop color.

`widgets/toast.rs:1036-1049` — fill loop writes `Role::Elevated` (a no-op
before plan 001, real after) and a hand-drawn frame colored by severity:

```rust
let fill = system.style(Role::Elevated);
for y in area.y..area.bottom() { ... buffer[(x, y)].set_style(fill); ... }
let border = system.style(kind.role());
let (tl, tr, bl, br, h, v) = if ascii { ("+","+","+","+","-","|") }
    else { ("┌","┐","└","┘","─","│") };
```

`widgets/status_bar.rs:797-800`:

```rust
buffer.set_style(
    area,
    fade_style(self.system.style(Role::StatusBar), self.alpha),
);
```

— mechanism works; role was empty before plan 001. After 001 the band
exists; this plan adds slot separators and zone chrome.

Design constraints: design SoT (`docs/design/component-visual-richness-plan.md`
§5): dialogs/popovers/menus/toasts paint `Elevated` fill; backdrop dims by
default; toast severity carried by icon + accent rail, border stays muted;
phosphor-obsidian doc: "canvas Reset behind backdrop; dialog `elevated`
fill".

## Commands you will need

| Purpose | Command | Expected |
|---|---|---|
| Check | `mise run check` | exit 0 |
| Gate | `mise run gate` | exit 0 |
| Targeted | `cargo nextest run -p termrock dialog toast status_bar popover dropdown_menu completion_menu` | pass |

## Scope

**In scope**:

- `crates/termrock/src/widgets/dialog.rs` (incl. `alert_dialog.rs` if it
  paints its own shell — check `grep -n "Clear.render\|block().render" crates/termrock/src/widgets/alert_dialog.rs`)
- `crates/termrock/src/widgets/toast.rs`
- `crates/termrock/src/widgets/status_bar.rs`
- `crates/termrock/src/widgets/popover.rs`, `dropdown_menu.rs`,
  `completion_menu.rs`, `notification_center.rs`, `drawer.rs` (Surface
  adoption for their shells — these carry hand-drawn `"┌"` literals, verified
  by grep at `539e7d03`)
- `crates/termrock-lookbook/src/stories.rs`
- `migrations/0265-*.md` + `MIGRATING.md`
- `plans/README.md`

**Out of scope**:

- `menu_bar.rs`, `callout.rs`, `log_pane.rs`, `preview_card.rs`,
  `fullscreen_viewer.rs`, `image_surface.rs` and other box-literal carriers —
  plan 010 sweep.
- Overlay stacking/focus logic (`interaction::OverlayStack`) — paint only.

## Git workflow

`main`, `git commit -s`. Suggested:
`feat(widgets)!: real overlay elevation — elevated fills, dimmed backdrop, status band`.

## Steps

### Step 1: Dialog paints an elevated surface

In `dialog.rs` `paint` (line 1246 region): replace `Clear.render` +
bare-block with:

1. `Clear.render(area, buffer)` stays (occlusion).
2. Paint `Surface::new(self.tokens).recipe(SurfaceRecipe::Overlay).bordered(false).padding(0,0)`
   over `area` (Overlay resolves `Role::Elevated` fill — verified
   `surface.rs:431`). Padding 0 here because plan 003's interior rhythm owns
   inner spacing.
3. Then the existing `panel.block().render(...)` border/title on top.

**Verify**: new test `dialog_paints_elevated_fill`: interior cell bg ==
`style(Role::Elevated).bg`; `cargo nextest run -p termrock dialog` → pass.

### Step 2: Backdrop dims by default

- `Backdrop::from_tokens(tokens)` (dialog.rs:545): stop discarding tokens —
  return the dim wash: symbol `░` (`.` for `GlyphSet::Ascii` — read glyph
  set from `tokens.glyphs`), fg from `Role::Backdrop` (plan 001 gives it a
  dark-gray fg), bg stays `DIALOG_BACKDROP` (Reset) per the documented
  policy. Add `Backdrop::reset()` as the explicit opt-out (already exists,
  line 527 — keep).
- `Backdrop::default()` also becomes the dim wash (`Self::from_tokens` needs
  tokens — make `default()` delegate to a token-free dim wash using
  `Color::DarkGray`, and have overlay call sites prefer `from_tokens`).
  Find backdrop call sites: `grep -rn "Backdrop::" crates/termrock/src` and
  route the ones with a `DesignSystem` in scope through `from_tokens`.
- Check `Motion`/reduced-motion is irrelevant here (static wash), but the
  `dim_wash` DIM modifier must survive `NO_COLOR` (mono story renders `░`
  field) — confirm in the mono lookbook story.

**Verify**: test `backdrop_from_tokens_dims`: symbol == `░`, fg ==
`style(Role::Backdrop).fg`; grep shows no `let _ = tokens;` left in
`dialog.rs`.

### Step 3: Toast chrome through Surface

Replace the hand-drawn frame (`toast.rs:1044-1076`) with
`Surface::new(system).recipe(SurfaceRecipe::Overlay).bordered(true)` +
border style override: border color = `Role::Border` (muted), **not**
`kind.role()`. Severity moves to: leading icon in `kind.role()` color plus a
1-col accent rail (left column cells styled `kind.role()`) — per the design
SoT. Progress strip (`toast.rs:1128` area) adopts the plan-004 ramp+track
treatment. Keep ASCII fallback via the Surface/GlyphSet path.

**Verify**: test `toast_border_is_muted_severity_on_icon_and_rail`: border
cell style == `Role::Border`, left rail cell fg == `kind.role()` fg;
`cargo nextest run -p termrock toast` → pass.

### Step 4: StatusBar band chrome

`status_bar.rs`: with `Role::StatusBar` now filled (plan 001), add:

- Slot separators between adjacent slots (glyph `·` from the glyph catalog,
  `Role::TextMuted`) — locate the placement loop (`status_bar.rs:802-830`,
  verified) and paint separators between placements on each side.
- Left/right zone division already exists via `placement.side`; no logic
  change — chrome only.

**Verify**: `cargo nextest run -p termrock status_bar` → pass (update
content-width expectations for separator cells); band bg asserted by
existing/new test `status_bar_paints_band`.

### Step 5: Popover/menu/drawer/notification shells

For `popover.rs`, `dropdown_menu.rs`, `completion_menu.rs`,
`notification_center.rs`, `drawer.rs`: find each hand-drawn box section
(search `"┌"` in each file) and replace the shell paint with
`Surface::new(...).recipe(SurfaceRecipe::Overlay).bordered(true)` keeping
each widget's existing border-color choice (focused menus keep
`BorderFocused`). Interior content code unchanged — only the shell. Each
widget keeps its measured geometry; Surface must paint the same rect the
hand-drawn frame did.

**Verify**: per-widget: `grep -n '"┌"' crates/termrock/src/widgets/popover.rs`
(and the other four) → 0 matches each; `cargo nextest run -p termrock popover
dropdown_menu completion_menu notification_center drawer` → pass.

### Step 6: Stories, migration, gate

- Lookbook stories: dialog/toast/status-bar/menu stories updated for
  elevated fills + dimmed backdrop (story expectations, sizes).
- `migrations/0265-v0.13.0-overlay-elevation.md`: dialog elevated fill,
  backdrop dim default (+ `Backdrop::reset()` opt-out), toast muted border +
  severity rail, status separators, Surface adoption list; before/after and
  validation commands. Link from `MIGRATING.md`.

**Verify**: `mise run check` → 0; `mise run gate` → 0. Commit.

## Test plan

New tests named in Steps 1–4 (4 minimum) in each widget's existing test
module; expectation updates in existing dialog/toast/status-bar tests.
Mono/ASCII stories re-checked for each touched widget (capability law).

## Done criteria

- [ ] `mise run check` + `mise run gate` exit 0
- [ ] `grep -n "let _ = tokens" crates/termrock/src/widgets/dialog.rs` → 0 matches
- [ ] `grep -c '"┌"' crates/termrock/src/widgets/{popover,dropdown_menu,completion_menu,notification_center,drawer,toast}.rs` → 0 each
- [ ] 4 new tests pass
- [ ] `migrations/0265-*.md` exists, linked
- [ ] `plans/README.md` updated

## STOP conditions

- Surface cannot express a shell some widget needs (e.g. drawer's partial
  border) → report the gap; do not fork a second chrome path.
- Backdrop dimming double-paints with `OverlayStack`'s own backdrop handling
  (if the interaction layer also paints) → report before changing overlay
  internals.
- Alert-dialog shell turns out independent of `dialog.rs` → add it to scope
  explicitly in your report, don't silently expand.

## Maintenance notes

- Plan 010 sweeps the remaining box-literal widgets to Surface — after this
  plan, Surface is proven for overlay shells.
- Reviewers: check modal stories in `paper` (light) preset — Elevated must
  read *above* Surface in light mode too (ladder direction flips luminance).
