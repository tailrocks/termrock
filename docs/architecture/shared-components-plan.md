# Shared components plan

The library is components first; the four applications are consumers
that prove the components compose. Behaviour is implemented once in
`src/core`, `src/shell`, `src/ui` or `src/widgets` and reused by every
screen — a page-level workaround means the component is wrong.

## Conventions every widget follows

- Same core traits: rendering, focus, keyboard, mouse, scrolling,
  selection, hints, context menus.
- Same API shape: builders, events, outcomes; overridable behaviour
  layered as app/profile policy → per-widget setting → key remapping.
- Exactly one keyboard-focus indicator at a time (the phosphor `▎`
  gutter); selection keeps its accent in-layer and goes neutral only
  under a modal or popup.
- Floating content goes through the overlay layer
  (`RenderCtx::overlay` / `flush_overlays` / `begin_modal`); light
  dismiss is owned by the popup via its backdrop region.

## §7 Showcase coverage track

The showcase demonstrates every widget, variant, state and
configuration option. Every new or changed library capability is
demonstrated in the showcase in the same change, and the showcase
coverage gate must pass; a capability is omitted only through the
coverage allowlist with a reason. App-specific UI patterns are
extracted to the library and demonstrated in the showcase before
apps use them (exception: Jackin's brand-specific intro, outro and
cockpit-to-capsule fade). The showcase itself stays a thin consumer:
pages add wiring, demo data and option tables, never drawing,
interaction or state logic of their own. Functional proof for every
page lives in `crates/termrock-functional/tests/functional/showcase.rs`; see
[functional testing](../functional-testing.md).
