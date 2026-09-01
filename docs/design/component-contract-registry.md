# Component contracts and registry metadata

| Field | Value |
|---|---|
| Status | Binding |
| Public UI authority | `termrock::registry::{public_ui_inventory, pattern_inventory}` |
| Docs projection | `termrock-lookbook inventory --format json` |
| Quality ledger | `docs/api/component-contracts.v2.json` |
| Schema | `docs/api/component-contract.schema.json` |
| Generated render evidence | `docs/api/component-render-evidence.json` |

## Separation of authority

- Rust owns exact public identities, rendering kind, family, canonical route,
  and representative story.
- Typed pattern inventory owns all pattern identities and optional links to an
  exact public visual owner.
- Canonical MDX owns purpose, source link, tags, composition, and deep guidance.
- The mandatory v2 ledger owns quality status, reason, evidence, waivers, and lints.
- Axis applicability is explicit: `required` is universal or proven by the
  mounted interactive owner; `conditional` stays incomplete until focused
  evidence establishes coverage, caller ownership, or non-applicability.
- Evidence names exact Rust symbols and exact canonical source, story, poster,
  and check paths. A source path alone never proves coverage.
- `docs/contract-audit` mounts all 227 representative stories from the joined
  catalog. It records exact size, capability-ladder, no-color, ASCII, character,
  and host-event outcomes without writing or comparing visual goldens.
- `docs/scripts/audit-render-contracts.ts` exact-joins that output back to ID,
  canonical source, story, render kind, family, and interaction metadata. Any
  omitted, duplicate, or drifted identity fails the contract gate.
- `docs/scripts/contract-owner-evidence.ts` records reviewed owner-local axis
  claims. Duplicate ID/axis claims, unknown IDs, missing test files, or missing
  exact test symbols fail synchronization or catalog validation.
- `docs/scripts/catalog-data.ts` owns contract parsing and generates the published
  JSON Schema; the schema is never a second hand-maintained validator.
- `docs/src/generated/catalog.ts` is the deterministic joined projection. It is
  consumed, never hand-edited.

No parallel manifest, compatibility route, or inferred coverage map is valid.
One exact public symbol has one route and one representative mounted story.

The current 210-entry public visual inventory contains 192 component-doc owners
and 18 exact public owners of patterns. The 35-entry pattern inventory adds 17
composed-only pattern concepts, producing 227 canonical ledger entries. The
deleted flat `Menu` duplicated canonical `DropdownMenu`; it is not an omitted
228th owner and must not be restored.

## Quality law

1. Every catalog entry has every v2 axis and design lint.
2. `covered` and `partial` require concrete evidence.
3. `missing`, `partial`, `caller_owned`, and `not_applicable` require reasons.
4. `complete=true` rejects missing axes, unwaived partial axes, failed lints,
   and lints that were not run.
5. Evidence story IDs and paths must exist; test and check references require
   an exact symbol anchor, and source/poster references must match the joined
   canonical entry.
6. Waiver expiry values are valid ISO 8601 calendar dates.
7. Remote registry data remains untrusted at its own CLI boundary; it never
   becomes documentation authority.

## Axis applicability

| Axis group | `required` when | `not_applicable` when |
|---|---|---|
| Visual, responsive, tiny, ASCII, no-color, color ladder, resize, panic | Every canonical owner | Never |
| Keyboard, focus | Typed representative is interactive | Typed interaction kind is `passive-paint` |
| Mouse | Typed hints declare click, drag, pointer, mouse, or wheel input | No typed pointer hint owns that path |
| Escape | Typed interaction kind is `disclosure-overlay` | Owner has no dismissal path |
| Disabled | Interactive action, input, navigation, or overlay owner | Owner does not own disabled interaction state |
| Overlay | Overlay family or disclosure interaction | Owner does not own an overlay |
| Loading, error, empty, streaming | An exact owner story declares and renders that state | Exact typed owner story set declares no such state |
| Large data | Exact large-data story, or scrolling data owner | Owner does not own large-data rendering |
| Unicode, CJK, combining, emoji | Exact character story, or text-bearing non-behavior owner | Typed behavior/layout-helper owns no text-bearing fixture |

Generated success is `partial`, never `covered`: it proves one mounted state and
one probe path. Owner-local `covered` claims require an exact executable test
symbol; reviewed caller-owned and not-applicable claims retain their typed source
and representative story. Static owners never receive interaction or focus
completion from the generated probe. Failed required probes stay `missing`; the
generator never converts them into waivers or caller-owned claims.
