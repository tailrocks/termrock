# DesignSystem + component recipes (complete terminal design system)

> **Superseded (2026-09-02).** Token taxonomy, spacing, glyphs, and state grammar live in [`DESIGN.md`](../../DESIGN.md). This file is historical.

| Field | Value |
|-------|-------|
| **Status** | Binding |
| **Migration** | `0079-v0.13.0-design-system-recipes.md` |

## Preserve / migrate / delete

| Surface | Fate |
|---------|------|
| `DesignSystem` sole paint root | **Preserve & extend** |
| `RolePalette` Role→Style map | **Extend** (syntax + chart roles) |
| `Density`, `Motion`, `GlyphSet`, `SpacingScale`, `SelectionChrome` | **Preserve** |
| `PanelRecipe`, `ListRowRecipe`, Button/Input recipes | **Preserve**; route through family contracts |
| `ColorCapability` + quantize | **Preserve**; wire adaptive presets |
| `RolePalette::{tailrocks_phosphor, slate}` | **Preserve**; no aliases |
| Runtime phosphor palette | **Named ANSI-16 only**; terminal owns actual RGB |
| Public phosphor RGB swatches | **Export-only** for web/SVG; never recipe authority |
| Component-local RGB in widgets | **Migrate** to roles/recipes when found |

## API sketch

```rust
// Presets
DesignSystem::phosphor()           // Phosphor Obsidian
DesignSystem::slate()
DesignSystem::paper()              // light
DesignSystem::ansi()               // 16-color native
DesignSystem::high_contrast()
DesignSystem::adaptive()           // env capability ladder

// Overrides / packages
system.with_role(Role::Accent, style)
system.merge(partial)              // override non-default roles from package
ThemePackage { id, label, system }

// Tokens
system.breakpoints                 // tiny/narrow/comfortable widths
system.elevation(Elevation::Raised) // Surface/Elevated mapping
system.button_recipe(variant, state)
system.panel_recipe / list_row_recipe
system.family_recipe(RecipeFamily::Action)

// Enforced families
RecipeFamily::{Action, Input, Collection, Overlay, Status, Data, Layout}

// Capability
system.quantize(ColorCapability::Ansi16)
system.no_color()  // monochrome + ASCII glyphs
system.ascii()     // GlyphSet::Ascii only
```

## Laws

1. Widgets take `&DesignSystem` only for paint.
2. Color is never sole meaning. Each family declares a required non-color cue:
   label weight, prompt glyph, selection gutter, framed title, status glyph +
   label, tiered text, or structural boundary.
3. Every GlyphSet has Unicode + ASCII.
4. Quantize is progressive: truecolor → 256 → 16 → mono.
5. Brand green is spent only on primary intent, focus, or a small semantic mark.
6. `Basic` retains short state transitions but freezes activity; `Off` freezes all motion.
7. Primary focus toggles the reverse-video state plus a bold focus border; it
   cannot collapse into the idle `ActionFocused` paint, including no-color.
8. Named-ANSI transitions are symmetric and remain named in both directions.
