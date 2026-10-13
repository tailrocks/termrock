# Component documentation standard

| Field | Value |
|-------|-------|
| **Status** | Binding for public component pages |
| **Bar** | Comparable to [shadcn/ui docs](https://ui.shadcn.com/docs) — purpose-first, copy-paste examples, ownership clarity |
| **Rule** | Do **not** restate field names. Explain **why** and **when** to use each pattern |
| **Applied** | Button · List · DataTable · Dialog · CommandPalette · PromptComposer · PermissionPrompt |

---

## 1. Completeness law

A component documentation page is complete only when:

1. Every **required section** below is present (or explicitly N/A with one line).  
2. **Basic** and **interactive** Rust examples use **public APIs only**.  
3. Those examples are mirrored in `crates/termrock/tests/documentation_examples.rs` and pass CI.  
4. Lookbook/Studio story ids are named for visual proof.  
5. Writing answers *when/why*, not only *what exists*.  
6. **Exactly one** Ghostty-class `<TerminalPreview story="…/…" />` embed per handbook
   page (primary lookbook story + multi-step interactivity). **Never** SVG,
   `component-previews/`, or multi-image galleries.

Thin generated inventory pages (`docs/content/docs/components/`) are **not** a substitute for handbook pages.

---

## 2. Required page template (order fixed)

| # | Section | Content rules |
|---|---------|----------------|
| 1 | **Purpose** | One-line + short paragraph: problem solved in a terminal UI |
| 1b | **Live terminal (Ghostty-class)** | One `<TerminalPreview>` for the primary story; no SVG |
| 2 | **When to use / when not** | Table vs alternatives |
| 3 | **Installation** | Crate pin and/or `termrock add …` |
| 4 | **Source files** | Crate paths or registry install list |
| 5 | **Basic example** | Minimal public API; compiles in CI |
| 6 | **Interactive example** | Key/mouse → outcome; compiles in CI |
| 7 | **Anatomy** | Named parts; primary survives contraction |
| 8 | **Public API** | Constructors/methods **with intent** (why call them) |
| 9 | **State ownership** | TermRock vs consumer table |
| 10 | **Typed outcomes** | Variants + who runs effects |
| 11 | **Variants** | Modes / emphasis / kinds |
| 12 | **Sizes** | Min usable, preferred, fullscreen/overlay |
| 13 | **Density** | Comfortable / compact / dashboard |
| 14 | **Keyboard** | Chords/intents; Esc law |
| 15 | **Mouse** | Hits, wheel, drag |
| 16 | **Focus** | Entry/exit, trap, opener restore |
| 17 | **Responsive** | Narrow/tiny drop order |
| 18 | **Accessibility / colorless** | Non-color cues |
| 19 | **Unicode** | CJK, combining, emoji, ASCII fallback |
| 20 | **Composition** | Nesting with Panel/Overlay/Workbench |
| 21 | **Theming** | Roles used (not hex dumps) |
| 22 | **Custom recipe** | Tokens/recipe override with intent |
| 23 | **Common mistakes** | Anti-patterns (why they break) |
| 24 | **Performance** | Viewport, alloc, stream notes |
| 25 | **Testing** | Story ids, unit tests, contracts |
| 26 | **Migration** | Link `migrations/00xx` or “none yet” |
| 27 | **Related components** | Siblings with one-line reason |
| 28 | **Complete application example** | Small shell loop sketch |

Frontmatter: `title`, `description` (purpose-oriented).

---

## 3. Markdown skeleton (copy for new pages)

```markdown
---
title: ComponentName
description: Purpose-first one-liner.
---

# ComponentName

**Purpose.** …

## When to use / when not
| Use | Prefer instead |
|-----|----------------|

## Installation
## Source files
## Basic example
## Interactive example
## Anatomy
## Public API
## State ownership
## Typed outcomes
## Variants
## Sizes
## Density
## Keyboard
## Mouse
## Focus
## Responsive
## Accessibility / colorless
## Unicode
## Composition
## Theming
## Custom recipe
## Common mistakes
## Performance
## Testing
## Migration
## Related
## Complete application example
```

---

## 4. Writing rules

1. **Why before what.** Open with a scenario, not a struct dump.  
2. **Public API only** in fenced `rust` blocks that CI compiles.  
3. **Borrowing:** show `&rows` / projections, not fake owned mega-models.  
4. **Outcomes ≠ effects:** “returns `Activated(id)`; consumer navigates.”  
5. **Ownership table** on every page.  
6. **Stories** listed with backticks for catalog CI (`list/narrow`).  
7. **No changelog filler** in body — history under Migration.  
8. **Density/responsive** describe *what drops first*, not “has a width field.”  
9. Prefer **tables** for parallel comparisons; prose for judgment.

---

## 5. File layout

| Artifact | Path |
|----------|------|
| Standard | `docs/design/component-documentation-standard.md` |
| Handbook (shadcn depth) | `docs/content/docs/handbook/*.mdx` |
| Generated inventory | `docs/content/docs/components/*.mdx` |
| Compilable examples | `crates/termrock/tests/documentation_examples.rs` |
| Thin usage snippets | `docs/scripts/component-docs.ts` → `check-component-snippets.ts` |

---

## 6. Installation wording

**Crate (today):**

```toml
termrock = { git = "https://github.com/tailrocks/termrock.git", rev = "PIN" }
```

**Registry (when item exists):**

```bash
termrock add termrock/<item>
```

List installed paths relative to `termrock.toml` `ui_root` when documenting registry skins.

---

## 7. CI enforcement

| Check | Ensures |
|-------|---------|
| `cargo test -p termrock --test documentation_examples` | Handbook-critical snippets compile |
| `bun run docs/scripts/check-component-snippets.ts` | `component-docs.ts` usage blocks compile |
| `bun run docs/scripts/check-catalog.ts` | Story ids referenced in docs exist |
| Lookbook check | SVG previews for primary stories |

Handbook authors: when adding a new fenced example that “must compile,” add a sibling test under `documentation_examples.rs`.

---

## 8. Applied handbook set

| Page | Public surface |
|------|----------------|
| [Button](../content/docs/handbook/button.mdx) | `Button` / `ButtonState` (+ `ActionBar` for toolbars) |
| [List](../content/docs/handbook/list.mdx) | `List` / `ListRow` / `ListState` |
| [DataTable](../content/docs/handbook/data-table.mdx) | `DataTable` / `Table` + `data_view` kits |
| [Dialog](../content/docs/handbook/dialog.mdx) | `Dialog` / `ChoiceDialog` + place helpers |
| [Command palette](../content/docs/handbook/command-palette.mdx) | `CommandPalette` + OverlayStack |
| [Prompt composer](../content/docs/handbook/prompt-composer.mdx) | `PromptComposer` |
| [Permission prompt](../content/docs/handbook/permission-prompt.mdx) | `PermissionPrompt` + queue |

---

## 9. Decision summary

1. Handbook is the depth layer; generated components stay inventory.  
2. Template order is fixed for scannability (shadcn-class).  
3. Examples are code, not aspirational pseudocode.  
4. Ownership and outcomes are first-class sections.  
5. Mistakes and performance prevent cargo-cult copy-paste.
