# DesignSystem + component recipes (complete terminal design system)

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
| `PanelRecipe`, `ListRowRecipe` | **Preserve**; add Button/Input recipes |
| `ColorCapability` + quantize | **Preserve**; wire adaptive presets |
| `RolePalette::{tailrocks_phosphor, slate}` | **Preserve**; alias Obsidian |
| Public phosphor RGB marketing consts | **Preserve** (crate default builder) |
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

// Capability
system.quantize(ColorCapability::Ansi16)
system.no_color()  // monochrome + ASCII glyphs
system.ascii()     // GlyphSet::Ascii only
```

## Laws

1. Widgets take `&DesignSystem` only for paint.
2. Color never sole meaning — glyphs + density + modifiers.
3. Every GlyphSet has Unicode + ASCII.
4. Quantize is progressive: truecolor → 256 → 16 → mono.
