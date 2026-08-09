# TerminalCapabilities and CapabilityBoundary

| Field | Value |
|-------|-------|
| **Status** | Binding |
| **Migration** | `0091-v0.13.0-terminal-capabilities.md` |
| **Module** | `capability` |
| **CLI** | `termrock doctor` |
| **Studio** | `capability/*` stories |

## Preserve / migrate / split / delete

| Surface | Fate |
|---------|------|
| `CapabilityProfile` Modern/Compatible/Minimal/Inline/Headless | **Preserve** |
| `CapabilitySet` / `CapabilityKind` / fallbacks | **Preserve** |
| `resolve_capabilities` / detect / doctor | **Preserve** |
| `EffectiveCapabilities` | **Alias** → `TerminalCapabilities` |
| Env reads inside widget paint | **Forbidden** — use `CapabilityBoundary` |
| Hidden hard deps on mouse/images/truecolor | **Forbidden** |

## Mission

Model terminal capabilities explicitly; progressive enhancement is a first-class
contract. Detect or inject: color depth, Unicode/ASCII, keyboard, mouse,
bracketed paste, hyperlinks, clipboard, sync output, images, text sizing,
alt-screen, inline, mux/SSH/ConPTY.

## API

```rust
// Resolve
TerminalCapabilities::detect()
TerminalCapabilities::resolve(profile, overrides)
TerminalCapabilities::for_profile(profile)           // pure
TerminalCapabilities::with_hints(hints, profile, o)  // PTY tests
resolve_from_detection(report, profile, overrides)

// Progressive enhancement for widgets
let boundary = caps.boundary(); // CapabilityBoundary
boundary.colorless() / ascii_glyphs() / allow_mouse()
boundary.component_hints()
boundary.project_system(system) / project_palette(palette)
boundary.session_flags() // → Session / crossterm options
boundary.fallback(CapabilityKind::Mouse)

// Detection pure
detect_from_hints(EnvHints::fixture("xterm-256color", Some("truecolor"), true))
```

## Profiles

| Profile | Color | Glyphs | Mouse | Alt screen | Notes |
|---------|-------|--------|-------|------------|-------|
| Modern | Truecolor | Unicode | on | on | Kitty/Wez/Ghostty class |
| Compatible | 256 | Unicode | on | on | SSH/mux safe |
| Minimal | Mono | ASCII | off | off | dumb / NO_COLOR class |
| Inline | 256 | Unicode | off | **off** | scrollback-friendly |
| Headless | buffer styles | Unicode | off | off | no raw mode |

## Laws

1. **NO_COLOR** → monochrome unless explicit color override.
2. Optional features never block paint.
3. Hosts resolve once; widgets use `CapabilityBoundary`.
4. Doctor lists every `CapabilityKind` + fallback story id.
