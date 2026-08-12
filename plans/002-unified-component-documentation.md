# Plan 002: Merge the handbook into canonical interactive component pages

> **Executor instructions:** Complete Plan 001 first. Follow this plan in order,
> run every verification, and update `plans/README.md`. Preserve useful handbook
> prose, but never preserve a duplicate route or second component catalog.
>
> **Drift check (run first):**
> `rtk git diff --stat 26457206..HEAD -- docs/content/docs docs/scripts/gen-component-pages.ts docs/scripts/component-docs.ts docs/scripts/check-catalog.ts docs/api/component-contracts.json`
> Plan 001 should not materially change these content sources. Any other
> mismatch with the evidence below is a STOP condition.

## Status

- **Execution:** IN PROGRESS on `feat/live-interactive-docs`
- **Priority:** P1
- **Effort:** L
- **Risk:** MED; large content move with generated-route checks
- **Depends on:** `plans/001-live-preview-runtime.md`
- **Category:** docs / information architecture
- **Planned at:** commit `26457206`, 2026-08-12

## Why this matters

The site currently presents Components and Component handbook as different
destinations even though both explain and preview the same building blocks.
The generated page is shallow while the handbook is partial and inconsistently
navigated. One canonical component page must combine live behavior, exact code,
state ownership, interaction, variants, and deeper guidance.

## Current state

- `docs/content/docs/meta.json:2-8` exposes both `components` and `handbook`.
- There are 165 generated component pages and 84 handbook MDX files. Handbook
  navigation lists 69 entries, so authored files and visible navigation differ.
- `docs/scripts/gen-component-pages.ts:91-155` emits one preview, usage, contract,
  and a stories table. Its copy claims click/keys explore transitions, but Plan
  001 proves the old host only scrubbed frames.
- `docs/scripts/component-docs.ts:1-5` stores only description, primary story,
  and usage. It cannot describe a real demo's actions or outcomes.
- `docs/scripts/check-catalog.ts:149-196` requires exactly one preview and a
  stories table on both component and handbook pages. It validates duplication
  instead of rejecting it.
- Handbook mixes primitives and product composites (`agent-workbench`,
  `settings-screen`, `setup-wizard`). This violates the repository's required
  `widgets` versus `patterns` boundary.
- The public inventory and `component-contracts.json` already provide strong
  coverage anchors; retain them.

## Target page

Exactly one route exists for each public widget:
`/docs/components/<component-slug>`. It contains, in this order:

1. Name and one-sentence purpose.
2. Large live demo with Preview/Code, Reset, current action hints, and last
   outcome. Code is the exact public-API setup used by the demo.
3. Real-world behavior recipe: what to click/type/drag and expected result.
4. State ownership and typed outcomes: widget-owned versus host-owned.
5. Keyboard, mouse, focus, cursor/caret, narrow, Unicode, non-color, and motion
   contracts, showing `not-applicable` honestly for passive widgets.
6. Explicit variant/configuration controls. A variant may reset/remount the
   demo; it must not masquerade as interaction.
7. API usage, common mistakes, test recipe, source links, and related components.

## Commands you will need

| Purpose | Command | Expected on success |
|---|---|---|
| Component inventory | `rtk bun docs/scripts/check-catalog.ts` | reports all public components covered |
| Component page checks | `rtk bun docs/scripts/check-component-pages.ts` | 165 canonical routes, zero duplicate/orphan routes |
| Snippets | `rtk bun --cwd docs run check:snippets` | all displayed Rust accepted |
| Site build | `rtk bun --cwd docs run build` | all routes prerender |
| Full gate | `rtk mise run gate` | exit 0 |

## Scope

**In scope:**

- `docs/content/docs/components.mdx`, `docs/content/docs/components/*.mdx`
- all current `docs/content/docs/handbook/*.mdx` and its metadata (remove after
  content mapping)
- `docs/content/docs/meta.json`
- `docs/scripts/component-docs.ts`, `component-doc-utils.ts`,
  `gen-component-pages.ts`, `check-catalog.ts`, snippet/static-site checkers,
  and a new focused component-page checker
- `docs/api/component-contracts.json` and a temporary/retained route migration map
- `docs/design/component-documentation-standard.md`
- internal links in `README.md`, crate READMEs, and documentation content

**Out of scope:**

- Widget or pattern Rust behavior
- A second docs route for advanced/deep component material
- Application pattern implementation (Plan 003)
- Changing the visual preview renderer established by Plan 001

## Git workflow

The user's execution instruction supersedes the original workflow: all three
plans ship from `feat/live-interactive-docs` in one PR to `main`. Commits use
Conventional Commits, DCO sign-off, and
`Co-authored-by: Codex <codex@openai.com>`.

## Steps

### Step 1: Create a complete migration and demo contract inventory

1. Enumerate every handbook file into a machine-readable map with exactly one
   destination: canonical component, application pattern, concept, or delete
   only when its full content is proven duplicate.
2. Extend canonical component metadata/frontmatter with `demo`, interaction
   kind, required actions, expected outcomes, capability tags, source path, and
   related components. Values must come from the shared catalog, not prose-only
   promises.
3. Use these interaction families for coverage: passive paint, activation,
   choice/toggle, selection/navigation, editor/form, disclosure/overlay,
   scrolling/virtualization, drag/continuous value, and timed state.
4. Change the docs standard to require one canonical route and an executable
   behavior recipe. Remove the old assumption that a stories table is itself an
   interactive demonstration.

**Verify:** checker reports 165/165 public components classified and 84/84
handbook files mapped exactly once. No demo ID is missing from the shared catalog.

### Step 2: Make component MDX the canonical content source

Stop overwriting rich pages from a parallel TypeScript prose source. Migrate
`component-docs.ts` data into canonical MDX frontmatter/body, then delete it.
Replace `gen-component-pages.ts` with two explicit tools:

- `scaffold-component-page.ts --component <Type>` creates only a missing page
  and refuses to overwrite one.
- `check-component-pages.ts` compares public API inventory to canonical MDX
  frontmatter and validates the page/demo contract during every build.

The checked-in canonical MDX contains the content users read. Migrate every
existing description, usage snippet, and contract without loss. Build checks,
not human memory, compare public API inventory to frontmatter.

Each page embeds one live demo instance. Multiple variants are controls on that
instance or clearly labeled independent examples below it; never a frame-step
carousel. Passive components remain honest: theme/density/content controls are
allowed, fake activation is not.

**Verify:** regenerate/check mode is idempotent; editing canonical prose no
longer gets overwritten; inventory check still finds exactly 165 pages.

### Step 3: Replace representative demos with real-world flows

Implement and test the shared demo scenarios by interaction family before bulk
conversion. Minimum exemplars:

- ActionLink: hover color, keyboard/click activation, visible destination/action.
- Button: press/release, loading, deterministic completion, disabled behavior.
- Alert/Dialog/ChoiceDialog/Dropdown/Popover/Toast: trigger, focus, choose or
  dismiss, outcome feedback, and correct disappearance.
- TextInput/PasswordInput/TextArea/FormWizard: focus, type, Unicode, paste,
  edit, step navigation, validation, and submit outcome.
- Slider/RangeSlider/SplitPane/ResizablePanelGroup: keys and real drag.
- Tabs/Sidebar/Menu/List/Table/TreeTable: real selection, activation,
  expand/collapse, and no generic page-scroll substitution.
- VirtualList/VirtualGrid: actual viewport scrolling over enough rows to prove
  virtualization.
- Spinner/Progress/Skeleton/loading surfaces: injected-time animation with a
  reduced-motion state.

After each exemplar passes, convert its entire family using the same shared
abstraction. Do not create one-off React behavior or duplicate widget logic.

**Verify:** one deterministic Rust trace and one browser test per family; all
native/web parity assertions from Plan 001 remain green.

### Step 4: Merge handbook content and remove the duplicate section

For each migration-map entry:

- Merge component-specific prose into its canonical component page.
- Move product-noun compositions to `/docs/patterns/<slug>` for Plan 003.
- Move genuine cross-component architecture to an existing/new concept page.
- Update all internal links. Add static redirects only if the production build
  can verify them; never keep duplicate content as an alias.

Then remove `docs/content/docs/handbook/`, remove Handbook from root metadata,
and delete checker exceptions that treat it as a valid second component system.
The public navigation should say Components and Application patterns, not
Components and Component handbook.

**Verify:**

```sh
find docs/content/docs/handbook -type f 2>/dev/null | wc -l
rg -n '/docs/handbook|\]\([^)]*handbook/' README.md crates docs/content docs/src
```

Expected: `0` files and no stale internal links (redirect configuration, if
implemented, is allowed only in its dedicated migration map).

### Step 5: Enforce experience, not only presence

Update `check-catalog.ts` and add focused tests that reject:

- a public component without one canonical page or live demo
- an active component marked passive
- required actions absent from demo hints
- a behavior recipe whose expected outcome is not emitted by its demo test
- editor caret metadata on a passive/non-editor demo
- generic `click`, `wheel`, or arrow hints unsupported by current state
- a stories table presented as live behavior
- a second route claiming to be the same component documentation

Run snippets, site build, then the full gate.

## Done criteria

- [ ] Exactly 165 public components have one canonical route and one shared demo.
- [ ] All 84 old handbook files are mapped and their useful content preserved.
- [ ] No Component handbook section, orphan page, or duplicate component route remains.
- [ ] Every active component has real actions and machine-tested outcomes.
- [ ] Passive components make no false interaction or cursor claims.
- [ ] Preview code matches the shared Rust demo's public API.
- [ ] Component, snippet, site, parity, and full repository gates pass.
- [ ] `plans/README.md` marks Plan 002 `DONE`.

## STOP conditions

- Plan 001 is incomplete or a component page still receives baked steps.
- A handbook page cannot be classified under the repository's building-block
  versus example-composite law.
- A required demo needs private API or a web-only behavior implementation.
- A canonical code example cannot be compiled or tied to the mounted demo.
- Static routing cannot remove or redirect old routes without broken links;
  report the exact framework limit before choosing a fallback.
- Any verification fails twice after a focused correction.

## Maintenance notes

A new public widget is incomplete until one component page, one shared demo,
real action/outcome metadata, contracts, source links, and deterministic tests
land together. The catalog checker must fail the same commit that introduces an
unrepresented widget.
