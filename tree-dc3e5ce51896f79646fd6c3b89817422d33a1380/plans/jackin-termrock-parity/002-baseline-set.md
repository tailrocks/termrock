# Plan 002: Commit the initial PNG baseline set for the jackin-used subset

> **Executor instructions**: Follow this plan step by step. Run the
> preconditions first. Run every verification command and confirm the
> expected result before moving on. If anything in "STOP conditions"
> occurs, stop and report — do not improvise. When done, update this
> plan's status row in `plans/jackin-termrock-parity/README.md`.

## Status

- **Priority**: P1
- **Effort**: M
- **Risk**: MED (depends on plan 001's freshly landed rasterizer API; renders 97 stories through interactor mounts whose determinism is asserted, not yet proven fleet-wide)
- **Depends on**: plans/jackin-termrock-parity/001-*.md (termrock-raster crate)
- **Covers**: spec/baselines.md "Baseline set for the jackin-used subset" · F5(set), D4, D6, N2
- **Guardrails**: N2
- **Research basis**: research/tui-png-baselines/03-termrock-seams-and-old-rev.md (Q1, Q6), research/tui-png-baselines/04-determinism-ci-storage.md (§5), research/tui-png-baselines/05-ci-placement-and-commands.md (Q2, Q3)
- **Planned at**: commit `41cf3d0b`, 2026-08-16

## Why this matters

TermRock has no committed image record of what its widgets look like: the
lookbook emits SVGs and lossy frame JSON, and CI only proves renders equal
*themselves*. This plan renders every current lookbook story of the 16
jackin-used widget families through the new `termrock-raster` engine
(phosphor theme only) and commits the PNGs in plain git under
`crates/termrock-lookbook/baselines/png/`. After it lands, the repo holds the
design record that plan 003's bless-required gate will protect and that
reviewers will diff as images in PRs. Plain git (never LFS) is what makes the
PR image diff visible at all.

## Preconditions — run before anything else

All commands run from the repository root
`/Users/donbeave/Projects/tailrocks/termrock`. Any failed precondition is a
STOP.

- Plan 001 landed — crate exists:
  `test -f crates/termrock-raster/src/lib.rs && echo RASTER-CRATE-OK`
  → prints `RASTER-CRATE-OK`
- Plan 001 landed — its tests pass:
  `cargo nextest run -p termrock-raster --all-features --locked`
  → exit 0, all tests pass
- Plan 001 marked DONE: its row in `plans/jackin-termrock-parity/README.md`
  reads DONE.
- Toolchain present: `cargo --version` → prints a version (workspace pins
  rust 1.97.1 via `rust-toolchain`/workspace `rust-version`); `mise --version`
  → prints a version.
- Drift check (this plan edits pre-existing code):
  `git diff --stat 41cf3d0b..HEAD -- crates/termrock-lookbook/`
  On any change to `crates/termrock-lookbook/src/frame.rs`,
  `crates/termrock-lookbook/src/main.rs`,
  `crates/termrock-lookbook/src/lib.rs`,
  `crates/termrock-lookbook/src/stories.rs`, or
  `crates/termrock-lookbook/Cargo.toml`, compare the "Starting state"
  excerpts below against live code; a mismatch is a STOP.
- No baseline dir exists yet:
  `test ! -e crates/termrock-lookbook/baselines && echo FRESH`
  → prints `FRESH` (if the dir exists, another session already started this
  plan — STOP and report).

## Spec contract

The requirement this plan implements, inlined verbatim from
`plans/jackin-termrock-parity/spec/baselines.md` — do not read `spec/`:

### Requirement: Baseline set for the jackin-used subset

The repo SHALL commit, in plain git (N2: never LFS), one PNG per lookbook
story belonging to the jackin-used subset's 16 widget families, rendered by
`termrock-raster` with the phosphor `RolePalette` (D6) at the story's
registered geometry, under
`crates/termrock-lookbook/baselines/png/<story-id-with-dashes>.png`
(filename scheme mirroring the SVG exporter's `svg.rs:104`). Coverage is the
subset only (D4) — no catalog-wide baselines.
Covers: F5, D4, D6, N2 · Evidence: ch. 03 Q6 (87 subset stories at HEAD), ch. 04 §5 (plain-git size math)

#### Scenario: Every subset story has a baseline
- **GIVEN** the registered story list filtered to the 16 subset components
- **WHEN** the baseline directory is listed
- **THEN** every such story id has exactly one committed PNG and no non-subset story does

#### Scenario: Baseline is reproducible
- **GIVEN** any committed baseline PNG
- **WHEN** its story is re-rendered on the same commit
- **THEN** decoded pixels are identical to the committed file

Done means these scenarios hold; the test plan below exercises them.

**Plan annotation on the story count (verified live at commit `41cf3d0b`,
not spec text):** the normative filter is "the registered story list
filtered to the 16 subset components", i.e. exact match on each story's
`component` field. At the planned-at commit that filter yields **97** story
ids, not 87: 87 static `Story::new` registrations (the count the spec's
evidence note cites, from research ch. 03 Q6) **plus 10 generated
`*/in-app` variants** that `stories()` appends with subset component names
(`in_app_stories`, `stories.rs:470-486`; scene table `stories.rs:279`). The
in-app ten: `list/in-app`, `panel/in-app`, `status-bar/in-app`,
`tabs/in-app`, `detail-table/in-app`, `diff-view/in-app`, `toast/in-app`,
`progress/in-app`, `text-input/in-app`, `dialog/in-app`. The research
chapter itself records that these generated variants exist on top of the 87.
This plan implements the normative filter — all 97 — because the in-app
variants are registered lookbook stories of the subset families. The
component-field filter is also why `capability/ascii-glyphs` (component
`List`) and `panel-stack/omission` (component `Panel`) are IN the set while
`progress-bar/*` (component `ProgressBar`), `virtual-list/*`
(`VirtualList`), and `alert-dialog/*` (`AlertDialog`) are OUT — never
filter by id prefix.

## Must NOT

Guardrails inlined verbatim from the must-not registry
(`plans/jackin-termrock-parity/spec/README.md`), with reasons. These
override anything a step seems to imply:

- **N2**: Baselines MUST NOT be stored in git-LFS — pointer-only PR diffs
  defeat the reviewer-sees-image-diff requirement (research ch. 04 §5). At
  the planned-at commit the repo has **no** `.gitattributes` file anywhere;
  do not create one, and do not run `git lfs` commands. Step 7 proves the
  absence of any LFS filter on the committed PNGs.

Plan-local boundaries (from the manifest scope, equally binding):

- Do NOT write the gate test, `TERMROCK_BLESS_PNGS` bless mode, or any
  mise task — that is plan 003's territory.
- Do NOT add, rename, or modify any story registration in `stories.rs` —
  state-gap stories are plan 004's territory. `stories.rs` is read-only for
  this plan.
- Do NOT touch `.github/workflows/` — CI needs no change (research ch. 05:
  the future gate rides workspace nextest).
- Do NOT modify `termrock-raster` — if its API cannot render a
  `Buffer` + `RolePalette` to PNG, that is a STOP, not a patch.

## Inputs to provide

None — fully self-contained. The one fact the plan cannot pin today is the
exact name of `termrock-raster`'s public render entry point (plan 001 lands
it). It is derivable: read `crates/termrock-raster/src/lib.rs` in step 1;
plan 001's contract requires a public path from a ratatui `Buffer` plus a
`RolePalette` to PNG bytes (9×18 px cells, phosphor-capable). If no such
public API exists, STOP — do not add one.

## Starting state

Facts verified at commit `41cf3d0b` (all paths from the repository root):

- **The full-fidelity seam** is the unresolved `ratatui::Buffer` inside
  `paint_story_frame`, one line before JSON encoding —
  `crates/termrock-lookbook/src/frame.rs:274`
  (`let buffer = terminal.backend().buffer().clone();`). The frame JSON
  after it is lossy: `resolve_cell_paint` (`frame.rs:173-199`) drops
  ITALIC and CROSSED_OUT, pre-swaps REVERSED into fg/bg, and pre-darkens
  DIM. **Baselines must be rasterized from the Buffer, never from
  `FrameCell`/`TerminalFrame` JSON.**
- `paint_story_frame(story, theme, cols, rows)` (`frame.rs:238-288`)
  builds a `TestBackend` terminal at
  `story_cols = cols.unwrap_or(story.width).max(1)` (same for rows) plus
  `STORY_PAD` (= 1, `frame.rs:27`) on each side, paints a `PREVIEW_CARD`
  charcoal ground over the whole area, `Clear`s the inner rect, sets the
  inner style to the palette's `Role::Canvas` via
  `crate::design::lookbook_system(theme.clone())`, then calls
  `story.make_interactor()` → `interactor.set_theme(theme.clone())` →
  `interactor.render(frame, inner)`. It hard-labels the theme `"phosphor"`
  and callers pass `RolePalette::default()` (which is phosphor — see
  `main.rs:148`, where `--theme phosphor` maps to `RolePalette::default()`).
- **Story registry**: `pub struct Story` has `id`, `title`,
  `component: &'static str`, `description`, `width`, `height`,
  `interactive`, private `render`/`interactor`
  (`crates/termrock-lookbook/src/stories.rs:168-185`); `Story` is `Copy`.
  `pub fn stories() -> Vec<Story>` (`stories.rs:743`) returns 1133 stories:
  1066 `Story::new` registrations plus generated in-app variants
  (`in_app_stories`, `stories.rs:470-486`, copies the host story with a new
  `id`/`component`). `story_by_id` (`frame.rs:230-234`) searches
  `stories()`.
- **CLI** (`crates/termrock-lookbook/src/main.rs`):
  `const USAGE: &str = "usage: termrock-lookbook <terminal|list|render|check|frame|export-posters>";`
  (`main.rs:23`). Subcommand style to mirror: `render` parses
  `--theme <phosphor|slate> --out <dir>` with a local `usage` string and a
  `while let Some(flag) = args.next()` loop (`main.rs:139-160`);
  `export-posters --out <dir> --story <id>...` creates the dir, renders via
  `paint_story_frame`, writes `format!("{slug}.json")` with
  `let slug = id.replace('/', "-");`, prints each written path
  (`main.rs:243-284`). `list --format json` prints the demo catalog
  (`main.rs:121-137`), whose entries carry `id` and `component`
  (`DemoDescriptor`: `id` at `crates/termrock-lookbook/src/demo.rs:51`,
  `component` at `demo.rs:55`).
- **Filename scheme to mirror** (`crates/termrock-lookbook/src/svg.rs:101-105`):

  ```rust
  /// Canonical filename for a story's SVG preview.
  #[must_use]
  pub(crate) fn story_svg_filename(story: Story) -> String {
      format!("{}.svg", story.id.replace('/', "-"))
  }
  ```

- **Crate layout**: the lookbook is lib + bin. `src/lib.rs` exports
  `pub mod demo; pub mod design; pub mod frame; pub mod interactors;
  pub mod knobs; pub mod palette256; pub mod stories;` — no cfg gates.
  `svg.rs` and `app.rs` are bin-only modules (declared in `main.rs`).
  `crates/termrock-lookbook/Cargo.toml` features:
  `default = ["native"]`, `native = ["termrock/crossterm", "ratatui/crossterm"]`;
  the `[[bin]]` has `required-features = ["native"]`; dependencies are
  `termrock = { version = "0.11.0", path = "../termrock" }`, `ratatui`,
  `serde`, `serde_json`. **`termrock-lookbook-web` consumes the lib with
  `default-features = false`** (`crates/termrock-lookbook-web/Cargo.toml:18`)
  and must keep compiling for `wasm32-unknown-unknown`
  (gate: `cargo check -p termrock-lookbook-web --target wasm32-unknown-unknown --locked`,
  `mise.toml` `[tasks.gate]`) — so the raster dependency must be optional
  and tied to the `native` feature, never unconditional.
- **Storage facts** (research ch. 04 §5): terminal-grid phosphor PNGs are
  flat-color, ~5–50 KB each; ~100–300 PNGs ≈ 0.5–15 MB total — far below
  every GitHub plain-git limit. No PNG is committed anywhere in the repo
  today, and there is no `.gitattributes` (verified: file absent), so no
  LFS filter can apply.
- **Convention exemplar for committed render baselines**:
  `crates/termrock-lookbook/tests/goldens.rs` + `crates/termrock-lookbook/goldens/*.txt`
  (committed text cell-dumps, diffed on every PR via workspace nextest).
  This plan commits the images only; the analogous PNG gate test is plan
  003's.
- **Determinism evidence**: `.github/workflows/docs.yml:115-118` already
  double-renders all story SVGs and `diff -r`s the trees on every PR;
  `frame.rs`'s test `poster_frame_uses_the_same_mounted_demo_as_native_and_web_hosts`
  proves two independent interactor mounts paint identically for a pattern
  story. Plan 001 ships termrock-raster's own double-render identity test.
  Coverage ledger assumption A1 ("`png` crate emits deterministic bytes at
  a fixed version") underwrites the byte-level re-render check in step 6.

## Commands you will need

Task definitions live in `mise.toml` (research ch. 05 Q3). PR CI runs the
repo's mise tasks via a pinned external reusable workflow (not inspectable
in-repo); the local mise commands in this table are authoritative for this
plan.

| Purpose | Command | Expected on success |
|---------|---------|---------------------|
| Build + full check | `mise run ci` | exit 0 |
| Pre-push gate (required before any push) | `mise run gate` (`mise.toml:44-67`) | exit 0 |
| Tests | `mise run test` | exit 0, all pass |
| Lint | `mise run lint` | exit 0 |
| Format | `mise run fmt` | exit 0 |
| Targeted lookbook tests | `cargo nextest run -p termrock-lookbook --all-features` | exit 0 |
| Generate baselines | `cargo run -q -p termrock-lookbook -- render-png --out crates/termrock-lookbook/baselines/png` | prints 97 paths, exit 0 |

## Scope

**In scope** (the only files to create or modify):

- `crates/termrock-lookbook/Cargo.toml` (optional `termrock-raster` dep +
  `native` feature extension)
- `Cargo.lock` (mechanical update from the dependency edge)
- `crates/termrock-lookbook/src/frame.rs` (extract the public Buffer seam)
- `crates/termrock-lookbook/src/lib.rs` (declare the new module)
- `crates/termrock-lookbook/src/png.rs` (new: subset filter + PNG writer)
- `crates/termrock-lookbook/src/main.rs` (new `render-png` subcommand +
  USAGE string)
- `crates/termrock-lookbook/baselines/png/*.png` (new: the 97 baselines)

**Out of scope** (do NOT touch, even though related):

- `crates/termrock-lookbook/tests/`, `mise.toml` — gate test, bless mode,
  and task wiring are plan 003's.
- `crates/termrock-lookbook/src/stories.rs`, `src/interactors.rs` — new
  state stories are plan 004's; this plan renders what exists.
- `crates/termrock-raster/**` — plan 001's crate is a fixed dependency here.
- `.github/workflows/**`, `.gitattributes` (must not exist), `deny.toml`,
  `REUSE.toml`, `LICENSES/` — licensing was plan 001's; CI needs nothing.
- `crates/termrock/**`, `docs/**`, `migrations/`, `MIGRATING.md` — no
  termrock public API changes here (the lookbook is `publish = false` and
  outside the tracked `cargo public-api` surface; additive lookbook lib
  functions need no migration file).

The hub `plans/jackin-termrock-parity/README.md` status row is
protocol-writable and never listed in scope: update it and stage it in the
same commit as this plan's final work (commit 2). Roadmap item + index
writes are owned by the hub's Executor protocol (first-started-plan /
package-completion events only) — follow the hub protocol for those; this
plan never writes them directly.

## Git workflow

- **No branch, no PR**: all TermRock work lands directly on `main`
  (repo CLAUDE.md law).
- Conventional Commits with DCO sign-off — always `git commit -s`.
- Two commits, each independently verified:
  1. `feat(lookbook): add render-png exporter over termrock-raster` —
     steps 1–4 (code only, tests green).
  2. `feat(lookbook): commit initial jackin-subset PNG baselines` —
     steps 5–7 (the 97 PNGs) plus the hub status flip.
- Push `main` only after `mise run gate` exits 0 on the final tree — the
  documented pre-push gate (`mise.toml:44-67`). `mise run ci`/`test`/
  `lint`/`fmt` verify work along the way but do not authorize a push.

## Steps

### Step 1: Wire termrock-raster into the lookbook as a native-only dependency

Read `crates/termrock-raster/src/lib.rs` and identify the public entry
point that renders a ratatui `Buffer` plus a `RolePalette` to PNG bytes
(plan 001's contract; the exact symbol name is whatever 001 shipped — e.g.
a free function or a renderer type with an encode step). Record the exact
call shape; if none exists, STOP.

**Recorded bindings** (fill in from the live crate at execution time; later
steps refer to these names, never to guessed symbols):

- `R_COMPARE` = the actual public pixel-compare helper path/symbol
  discovered by reading `crates/termrock-raster/src/` (plan 001 ships one;
  its plan names `src/compare.rs` with `compare_png_pixels`). If no such
  public helper exists → STOP (plan-001 contract gap).

Edit `crates/termrock-lookbook/Cargo.toml`:

```toml
[features]
default = ["native"]
native = ["termrock/crossterm", "ratatui/crossterm", "dep:termrock-raster"]

[dependencies]
termrock-raster = { version = "<read from crates/termrock-raster/Cargo.toml>", path = "../termrock-raster", optional = true }
```

For `version`, read the actual `version` field from
`crates/termrock-raster/Cargo.toml` at execution time and mirror the
dependency shape the lookbook already uses for `termrock` (path + version,
`crates/termrock-lookbook/Cargo.toml:24`) — never hardcode a version from
this plan. (Keep every existing entry; only add the optional dep and extend
the `native` list.) Then run `cargo check -p termrock-lookbook` once to
update `Cargo.lock`.

**Verify**:
`cargo check -p termrock-lookbook --all-features` → exit 0, and
`cargo check -p termrock-lookbook-web --target wasm32-unknown-unknown --locked`
→ exit 0 (raster stays out of the wasm graph). If the wasm target is not
installed: `rustup target add wasm32-unknown-unknown` first.

### Step 2: Extract the public Buffer seam in frame.rs

In `crates/termrock-lookbook/src/frame.rs`, split `paint_story_frame`
(currently `frame.rs:238-288`) into:

```rust
/// Paint a story once (static path) and return the unresolved [`Buffer`].
///
/// This is the full-fidelity seam: every modifier (ITALIC, CROSSED_OUT
/// included) is still present here; [`encode_buffer`] / frame JSON below
/// this point is lossy.
#[must_use]
pub fn paint_story_buffer(
    story: Story,
    theme: &RolePalette,
    cols: Option<u16>,
    rows: Option<u16>,
) -> Buffer
```

whose body is exactly the current `paint_story_frame` logic from the
`story_cols` computation through `terminal.backend().buffer().clone()`
(lines 244–274), returning the cloned `Buffer`. Rewrite
`paint_story_frame` as a wrapper: recompute
`let story_cols = cols.unwrap_or(story.width).max(1);` and
`let story_rows = rows.unwrap_or(story.height).max(1);`, call
`paint_story_buffer(story, theme, cols, rows)`, then `encode_buffer(&buffer)`
and build the same `TerminalFrame` literal as today (theme label
`"phosphor"` unchanged). Zero behavior change; `paint_story_after_keys` is
untouched.

**Verify**: `cargo nextest run -p termrock-lookbook --all-features` →
exit 0; in particular the existing tests
`paint_story_frame_nonempty_for_list_selection` and
`poster_frame_uses_the_same_mounted_demo_as_native_and_web_hosts` still
pass (the latter proves the refactor did not perturb the paint path).

### Step 3: Add the `png` lib module — subset filter + PNG writer

Create `crates/termrock-lookbook/src/png.rs` and declare it in
`crates/termrock-lookbook/src/lib.rs` as
`#[cfg(feature = "native")]\npub mod png;` (after `pub mod palette256;`,
keeping the list alphabetical-ish; the cfg keeps the wasm lib clean).
Match the repo's file conventions: SPDX header (see `frame.rs:1-2`), `//!`
module doc, `#[must_use]` on pure fns, rustdoc on every public item,
placeholder-free — `mise run docs-quality` greps for a list of banned
placeholder rustdoc phrasings (e.g. `` Documentation for `…` ``); see
`mise.toml` `[tasks.docs-quality]` for the exact list.

Contents (shape, not necessarily line-for-line):

```rust
/// The 16 jackin-used widget families, by exact `Story::component` string.
pub const JACKIN_SUBSET_COMPONENTS: [&str; 16] = [
    "ActionBar", "Backdrop", "ChoiceDialog", "DetailTable", "Dialog",
    "DiffView", "HintBar", "List", "MessageDialog", "Panel", "Progress",
    "StatusBar", "Tabs", "TextInput", "Toast", "Viewport",
];

/// Registered stories of the subset families, sorted by id.
#[must_use]
pub fn subset_stories() -> Vec<Story> { /* stories() filtered by
    JACKIN_SUBSET_COMPONENTS.contains(&story.component), sort_by id */ }

/// Canonical baseline filename (mirrors the SVG scheme, svg.rs).
#[must_use]
pub fn story_png_filename(story: Story) -> String {
    format!("{}.png", story.id.replace('/', "-"))
}

/// Render one story to PNG bytes at its registered geometry, phosphor only.
#[must_use]
pub fn render_story_png(story: Story) -> Vec<u8> { /* paint via
    crate::frame::paint_story_buffer(story, &RolePalette::default(), None, None)
    then the termrock-raster entry point from step 1 */ }

/// Write every subset baseline into `out_dir`, creating it; returns paths.
pub fn write_story_pngs(out_dir: impl AsRef<Path>) -> io::Result<Vec<PathBuf>>
```

Hard requirements: filter by **exact `component` match** (never id prefix —
`capability/ascii-glyphs` is component `List`; `ProgressBar`/`VirtualList`/
`AlertDialog` etc. are near-name non-subset components); phosphor is
`RolePalette::default()` and no other theme parameter exists (D6);
`cols`/`rows` are `None` so the story's registered `width`/`height` apply;
render from the `Buffer` seam, never from `FrameCell`/frame JSON (frame
JSON drops ITALIC and CROSSED_OUT — full fidelity lives before encoding).
Add the unit tests from the Test plan in a `#[cfg(test)] mod tests` here.

**Verify**: `cargo nextest run -p termrock-lookbook --all-features` →
exit 0 including the new tests.

### Step 4: Add the `render-png` CLI subcommand

In `crates/termrock-lookbook/src/main.rs`:

- Extend `USAGE` (`main.rs:23`) to
  `"usage: termrock-lookbook <terminal|list|render|render-png|check|frame|export-posters>"`.
- Add a dispatch arm `if first == OsStr::new("render-png")` mirroring the
  existing `render` arm's argument loop (`main.rs:139-160`): usage string
  `"usage: termrock-lookbook render-png --out <dir>"`, sole flag `--out
  <dir>` (no `--theme` — baselines are phosphor-only by D6, matching the
  `frame` subcommand's hard-fixed phosphor). It calls
  `termrock_lookbook::png::write_story_pngs(out_dir)` and prints each
  returned path to stdout, one per line (mirroring `write_svgs`,
  `main.rs:305-314`).

**Verify**:
`cargo run -q -p termrock-lookbook -- render-png --out target/render-png-check`
→ exit 0, prints 97 paths;
`ls target/render-png-check | wc -l` → `97`;
`cargo run -q -p termrock-lookbook -- render-png` → exits nonzero with the
usage string. Commit steps 1–4 now (commit 1 in Git workflow).

### Step 5: Generate the baseline set and prove the 1:1 mapping

Generate into the canonical location:

```sh
cargo run -q -p termrock-lookbook -- render-png --out crates/termrock-lookbook/baselines/png
```

Then run the exact 1:1 check — expected set derived live from the
registered story list (the authority), never from a hardcoded list:

```sh
cargo run -q -p termrock-lookbook -- list --format json | python3 -c '
import json, sys
SUBSET = {"ActionBar","Backdrop","ChoiceDialog","DetailTable","Dialog",
          "DiffView","HintBar","List","MessageDialog","Panel","Progress",
          "StatusBar","Tabs","TextInput","Toast","Viewport"}
for d in json.load(sys.stdin):
    if d["component"] in SUBSET:
        print(d["id"].replace("/", "-") + ".png")
' | sort > target/png-expected.txt
ls crates/termrock-lookbook/baselines/png | sort > target/png-actual.txt
diff target/png-expected.txt target/png-actual.txt && echo SUBSET-1TO1-OK
```

**Verify**: prints `SUBSET-1TO1-OK`; `wc -l < target/png-expected.txt` →
`97`. Cross-check against the snapshot at the planned-at commit — the 97
ids were exactly (dashes replace `/`, `.png` appended):

action-bar/{basic,narrow,unicode} · backdrop/{basic,narrow,unicode} ·
capability/ascii-glyphs · choice-dialog/{basic,narrow,unicode} ·
detail-table/{basic,in-app,narrow,unicode} ·
dialog/{compact,destructive,in-app,message,narrow,unicode} ·
diff-view/in-app · diff/{basic,narrow,search,split,unicode,word} ·
hint-bar/{narrow,unicode,wrapped} ·
list/{ascii,comfortable,composed-row,disabled,empty,groups,in-app,loading,multi,narrow,search,selection,tiny,unicode} ·
message-dialog/{details,narrow,unicode} · panel-stack/omission ·
panel/{actions,collapsible,empty,error,focused,in-app,loading,narrow,unicode,variants} ·
progress/{detailed,determinate,failed,in-app,multi-line,narrow,unicode} ·
status-bar/{basic,in-app,minimal,narrow,rich,transient,unicode} ·
tabs/{closable,in-app,manual,narrow,overflow,status,unicode,vertical} ·
text-input/{basic,in-app,invalid,narrow,prefix,secret,unicode} ·
toast/{in-app,kinds,narrow,persistent,stack,success,unicode} ·
viewport/{both-axes,narrow,unicode}

If the live list differs from this snapshot, re-run the drift check from
Preconditions; an unexplained difference is a STOP.

### Step 6: Prove reproducibility (re-render → identical)

```sh
cargo run -q -p termrock-lookbook -- render-png --out target/render-png-b > /dev/null
diff -r crates/termrock-lookbook/baselines/png target/render-png-b && echo REPRO-OK
```

**Verify**: prints `REPRO-OK` (byte-identical trees; this is *stronger*
than the spec's decoded-pixel identity and is expected to hold per ledger
assumption A1 and plan 001's determinism self-test — the *CI gate*, plan
003, still compares decoded pixels per N3, never bytes). If any file
differs: decode-compare that pair with `R_COMPARE` (the pixel-compare
binding recorded in step 1) — pixel-different means a nondeterministic story render, which is a
STOP (A1/determinism falsified); pixel-identical-but-byte-different also a
STOP (A1 falsified — report it; plan 003's byte-level assumptions need
re-planning). This is the mirror of the docs.yml double-render precedent
(`docs.yml:115-118`).

### Step 7: Prove plain git (N2) and sanity-check size

```sh
test ! -f .gitattributes && echo NO-GITATTRIBUTES
git check-attr filter diff -- crates/termrock-lookbook/baselines/png/panel-focused.png
du -sh crates/termrock-lookbook/baselines/png
```

**Verify**: prints `NO-GITATTRIBUTES`; `git check-attr` reports
`filter: unspecified` and `diff: unspecified` (no LFS filter can apply —
N2 holds); single size rule: total baselines dir size > 30 MB → STOP and
inspect before committing; otherwise pass (research ch. 04 §5 predicts
~0.5–15 MB for a set this size).

Then flip this plan's hub status row in
`plans/jackin-termrock-parity/README.md` and stage the in-scope files and
the hub README row together
(`git add crates/termrock-lookbook/baselines/png plans/jackin-termrock-parity/README.md`);
after staging both, confirm `git status --porcelain` shows nothing outside
the in-scope list plus the hub README. Make commit 2 per Git workflow, run
`mise run gate` (`mise.toml:44-67`), and push `main` only when it exits 0.

## Test plan

New tests in `crates/termrock-lookbook/src/png.rs` (`#[cfg(test)] mod
tests`), modeled structurally on `frame.rs`'s test module
(`frame.rs:393-564`):

- `subset_filter_matches_component_field_not_id_prefix` — covers scenario
  "Every subset story has a baseline" (membership half): `subset_stories()`
  contains at least one story per each of the 16
  `JACKIN_SUBSET_COMPONENTS`; contains ids `capability/ascii-glyphs`,
  `panel-stack/omission`, and all ten in-app ids named in the Spec-contract
  annotation; contains **no** story whose component is `ProgressBar`,
  `VirtualList`, `AlertDialog`, `NavigationList`, or `KeyValueList`.
  Expected values come from this plan's independently verified registry
  facts, not from re-running the filter under test.
- `png_filename_mirrors_svg_scheme` —
  `story_png_filename(story_by_id("panel/focused").unwrap())` ==
  `"panel-focused.png"` (independent truth: the SVG exporter's documented
  scheme, `svg.rs:104`).
- `render_story_png_is_reproducible_and_nonempty` — covers scenario
  "Baseline is reproducible" at unit scale: `render_story_png` on
  `panel/focused` twice yields non-empty, byte-identical `Vec<u8>` starting
  with the PNG magic bytes `[0x89, b'P', b'N', b'G']` (magic per the PNG
  spec, an independent source).

Scenario-level verification is command-driven where the artifact is the
repo itself: "Every subset story has a baseline" (exactly, both directions)
is proven by step 5's 1:1 diff; "Baseline is reproducible" at full-set
scale by step 6. The *permanent* CI-side test for both scenarios is plan
003's gate test — deliberately out of scope here.

**Verify**: `cargo nextest run -p termrock-lookbook --all-features` → all
pass, including the 3 new tests.

## Done criteria

Machine-checkable. ALL must hold:

- [ ] `mise run ci` exits 0
- [ ] `mise run test` exits 0; the 3 new `png.rs` tests exist and pass
- [ ] `mise run lint` exits 0 and `mise run fmt` exits 0
- [ ] Step 5's 1:1 check prints `SUBSET-1TO1-OK` with 97 expected files
      (every subset story has exactly one PNG; no non-subset story has one)
- [ ] Step 6's re-render check prints `REPRO-OK`
- [ ] Step 7 prints `NO-GITATTRIBUTES` and `filter: unspecified` /
      `diff: unspecified` (N2)
- [ ] `cargo check -p termrock-lookbook-web --target wasm32-unknown-unknown --locked`
      exits 0 (raster kept out of the wasm graph)
- [ ] The 97 PNGs are committed under
      `crates/termrock-lookbook/baselines/png/`
      (`git ls-files crates/termrock-lookbook/baselines/png | wc -l` → 97)
- [ ] `mise run gate` exits 0 on the final tree (pre-push gate,
      `mise.toml:44-67`) — precondition for the push
- [ ] No files outside the in-scope list modified (`git status`, run after
      staging per step 7) — excluding the hub
      `plans/jackin-termrock-parity/README.md` status row, updated and
      staged in the same commit as this plan's final work; roadmap item +
      index writes are owned by the hub's Executor protocol
      (first-started-plan / package-completion events only) — follow the
      hub protocol for those
- [ ] `plans/jackin-termrock-parity/README.md` status row updated and
      staged in commit 2

## STOP conditions

Stop and report back (do not improvise) if:

- Any precondition fails, or "Starting state" does not match reality
  (especially `frame.rs:238-288`, the `stories()`/in-app registry shape, or
  the lookbook Cargo.toml feature layout).
- `termrock-raster` exposes no public Buffer + `RolePalette` → PNG path,
  or no public pixel-compare helper (`R_COMPARE` in step 1 stays unbound —
  plan-001 contract gap) — never patch the raster crate from this plan.
- The raster crate's cell geometry differs from 9×18. Check it: verify the
  raster crate's cell geometry constants —
  `grep -rn '9' crates/termrock-raster/src/ | grep -i 'cell'` or read its
  geometry constants directly; expected cell width 9 / cell height 18,
  matching the lookbook seam constants at
  `crates/termrock-lookbook/src/frame.rs:345-348` (`CELL_WIDTH_PX = 9`,
  `CELL_HEIGHT_PX = 18`). If the crate's constants differ from 9×18 → STOP.
- The live subset story list disagrees with the 97-id snapshot in step 5
  and the drift check shows no explaining commit.
- Step 6 finds any story whose re-render differs (pixel-different =
  nondeterministic render; byte-different-pixel-identical = ledger
  assumption A1 falsified — name the story ids either way).
- Any story panics or renders an empty buffer during `render-png`.
- A `.gitattributes` file exists or anything asks for git-LFS (N2).
- A step's verification fails twice after a reasonable fix attempt.
- The work would require touching an out-of-scope file (gate test, mise
  tasks, stories.rs, termrock-raster, workflows).

## Maintenance notes

- **Plan 003** builds the bless-required gate directly on this plan's
  surface: `termrock_lookbook::png::{JACKIN_SUBSET_COMPONENTS,
  subset_stories, story_png_filename, render_story_png}` and the committed
  `baselines/png/` dir. Keep those names stable within this plan; 003 may
  reshape them.
- **Plan 004** adds focused/disabled stories for TextInput, Tabs, Toast,
  StatusBar, ActionBar; because the filter is by component field, its new
  stories join the subset automatically and 004 blesses their baselines —
  which is why this plan must not hardcode a story count anywhere in code.
- **Reviewer scrutiny**: (1) the seam — baselines must come from
  `paint_story_buffer`'s unresolved Buffer, never frame JSON (ITALIC/
  CROSSED_OUT fidelity); (2) the wasm graph — `termrock-raster` must stay
  behind the `native` feature; (3) the in-app ten — their inclusion follows
  the spec's component-field filter, and the spec evidence note's "87"
  counts only static registrations (this plan's Spec-contract annotation
  records the discrepancy; if a later decision excludes in-app variants,
  the filter gains one `!id.ends_with("/in-app")` clause and 10 PNGs are
  deleted — a re-plan, not an executor call).
- **Deferred**: catalog-wide baselines (explicitly not wanted, D4); any
  non-phosphor theme baselines (D6); pruning-of-stale-baselines logic in
  `render-png` (plan 003's 1:1 gate makes strays fail CI).
