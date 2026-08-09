# Component documentation standard

**Status:** binding for public component pages  
**Bar:** comparable to [shadcn/ui docs](https://ui.shadcn.com/docs) — purpose-first, copy-paste examples, ownership clarity  
**Rule:** Do not restate field names. Explain **why** and **when** to use each pattern.

---

## 1. Page template (required sections)

Every flagship component page uses this order:

| # | Section | Content rules |
|---|---------|----------------|
| 1 | **Title + one-line purpose** | What problem it solves in a terminal UI |
| 2 | **When to use / when not** | Alternatives (e.g. List vs Table vs Picker) |
| 3 | **Installation** | Crate pin and/or `termrock add …` when registry |
| 4 | **Source files** | Paths installed (crate module or registry files) |
| 5 | **Basic example** | Minimal public API; compiles in CI |
| 6 | **Interactive example** | Key/mouse outcomes; compiles in CI |
| 7 | **Anatomy** | Named parts (primary survives contraction) |
| 8 | **Public API** | Constructors, important methods — *with intent* |
| 9 | **State ownership** | TermRock vs consumer |
| 10 | **Typed outcomes** | Enum variants + who handles effects |
| 11 | **Variants** | Modes / emphasis / kinds |
| 12 | **Sizes** | Min usable, preferred, fullscreen |
| 13 | **Density** | Comfortable / compact / dashboard behavior |
| 14 | **Keyboard** | Intents/chords; Esc law |
| 15 | **Mouse** | Hits, wheel, drag |
| 16 | **Focus** | Entry/exit, trap, opener restore |
| 17 | **Responsive** | Narrow/tiny contraction order |
| 18 | **Accessibility / colorless** | Non-color cues |
| 19 | **Unicode** | CJK, combining, emoji, ASCII fallback |
| 20 | **Composition** | How it nests with Panel/Overlay/Workbench |
| 21 | **Theming** | Roles used |
| 22 | **Custom recipe** | Tokens/recipe override example |
| 23 | **Common mistakes** | Anti-patterns |
| 24 | **Performance** | Viewport, alloc, stream notes |
| 25 | **Testing** | Story ids, unit tests, contracts |
| 26 | **Migration** | Link `migrations/00xx` if any |
| 27 | **Related** | Sibling components |
| 28 | **Complete application sketch** | Small shell loop, no product domain |

Optional frontmatter: preview SVG, registry item id, quality contract status.

---

## 2. Writing rules

1. **Why before what.** Open with a scenario, not a struct dump.  
2. **Public API only** in fenced `rust` blocks that CI compiles.  
3. **Borrowing:** show `&rows`, not fake owned mega-structs.  
4. **Outcomes ≠ effects:** “returns `Activated(id)`; consumer navigates.”  
5. **Ownership table** on every page.  
6. **Stories:** list lookbook/Studio ids with backticks for catalog CI.  
7. **No changelog filler** in the body; put history under Migration.  
8. **Density/responsive** describe *what drops first*, not “has a width field.”

---

## 3. File layout

| Artifact | Path |
|----------|------|
| Standard (this file) | `docs/design/component-documentation-standard.md` |
| Flagship handbook pages | `docs/content/docs/handbook/*.mdx` |
| Generated thin pages | `docs/content/docs/components/*.mdx` (API inventory; keep short) |
| Compilable examples | `crates/termrock/tests/documentation_examples.rs` (+ handbook snippets via snippets check when linked) |

Generated pages stay inventory-facing. **Handbook pages** are the shadcn-depth docs.

---

## 4. Installation wording

**Crate (today):**

```toml
termrock = { git = "https://github.com/tailrocks/termrock.git", rev = "PIN" }
```

**Registry (when item exists):**

```bash
termrock add termrock/<item>
```

List installed paths relative to `termrock.toml` `ui_root`.

---

## 5. CI

| Check | Ensures |
|-------|---------|
| `documentation_examples` tests | Handbook-critical snippets compile |
| `check-component-snippets` | `component-docs.ts` usage blocks compile |
| `check-catalog` | Story ids appear in docs; previews exist |
| Future | Handbook section presence lint for public widgets |

---

## 6. Applied handbook set (initial)

Button (ActionBar pattern) · List · DataTable (Table + data_view) · Dialog · CommandPalette · PromptComposer · PermissionPrompt  

See `docs/content/docs/handbook/`.
