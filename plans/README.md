# Look-and-feel overhaul — verified archive

Generated on 2026-08-14 at `605217aa`. Executed on PR #26.

All 22 plans are complete and acceptance-verified. No claims, active lanes,
deferred implementation items, or follow-up queue remain here. Individual
plan files stay as design history because implementation comments and design
documents cite their decisions; migrations are the consumer-facing record.

## Completion evidence

| Plan | Result | Durable evidence |
|------|--------|------------------|
| 001 | VERIFIED | Binding underline grammar plus `interaction_underline_is_dead` |
| 002 | VERIFIED | Migrations 0283, 0284, 0287; palette and contrast tests |
| 003 | VERIFIED | Migrations 0282, 0326; capability, quantize, monochrome, glyph tests |
| 004 | VERIFIED | Migration 0288; recipe-authority design gates |
| 005 | VERIFIED | Migrations 0295, 0304; global underline gate |
| 006 | VERIFIED | Migration 0296; collection-gutter and selection gates |
| 007 | VERIFIED | Migrations 0298, 0300, 0301; accent-budget gate |
| 008 | VERIFIED | Migration 0312; shared input-chrome gates |
| 009 | VERIFIED | Migrations 0302, 0307–0311, 0326; overlay tests and neon-fill gate |
| 010 | VERIFIED | Migration 0315; pattern-composition gates |
| 011 | VERIFIED | Migration 0321; preset, SVG, inspector, and golden tests |
| 012 | VERIFIED | Migration 0313; row-anatomy gate and table tests |
| 013 | VERIFIED | Migration 0319; empty-state and one-column-glyph gates |
| 014 | VERIFIED | Migrations 0289, 0297, 0299, 0303, 0305, 0306, 0320, 0325; motion-policy tests |
| 015 | VERIFIED | Migration 0316; chip, tab, keycap, and bold-budget gates |
| 016 | VERIFIED | Migration 0314; patterns-only-compose and charter gates |
| 017 | VERIFIED | Migrations 0283, 0287, 0290, 0324, 0326; contrast and information-budget gates |
| 018 | VERIFIED | Generated in-app scenes, application links, docs contracts, docs build |
| 019 | VERIFIED | `termrock-showcase`; migration 0326; public-API, trust, stream-layout, scene, and tiny-frame tests |
| 020 | VERIFIED | Migrations 0286, 0326; chord, ellipsis, and microcopy gates |
| 021 | VERIFIED | Migrations 0317, 0325; state-matrix and action-feedback tests |
| 022 | VERIFIED | Migrations 0281, 0285, 0291–0294, 0318, 0325, 0326; geometry, truncation, scrollbar gates |

## Final verification

Run from repository root:

```sh
mise run gate
```

The gate covers formatting, clippy, all workspace tests and features,
deterministic preview goldens, minimal/default/crossterm/example builds, wasm,
rustdoc warnings, public API inventory, feature powerset, dependency policy,
package construction, preview posters, and the documentation build.

The implementation plans, [coverage map](COVERAGE.md), and
[designer checklist](DESIGNER-CHECKLIST.md) are frozen evidence, not an active
work queue. New work requires a new plan instead of reopening this archive.
