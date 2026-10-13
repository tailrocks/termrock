# Terminal capability architecture

| Field | Value |
|-------|-------|
| **Status** | Design SoT + foundation (`termrock::capability`) |
| **Law** | Optional terminal features must never become **hidden hard dependencies** |
| **Code** | `crates/termrock/src/capability/{detect,profile,set,doctor}.rs` |
| **Paint adapters** | `ColorCapability`, `GlyphSet`, `CapabilityPreviewHost`, OSC helpers |
| **Related** | SessionOptions, quality contracts (`color_ladder`, `ascii_fallback`, `no_color`) |

---

## 1. Goals

1. Look exceptional on modern terminals (truecolor, Unicode, mouse, paste, images).  
2. Degrade **gracefully** and **explicitly** on older, restricted, muxed, or remote hosts.  
3. Every optional capability has: **detection · override · fallback · Studio story · contract test**.  
4. `termrock doctor` is the user-facing truth surface.  
5. Library code **never panics** because a feature is off — only emission paths are gated.

---

## 2. Capability model

### 2.1 Kinds (`CapabilityKind`) — 20

| Kind | Class |
|------|--------|
| Truecolor · Color256 · AnsiColor · NoColor | Color ladder |
| Unicode · AsciiOnly | Glyphs |
| Keyboard · EnhancedKeyboard | Input |
| Mouse · BracketedPaste | Pointer / paste |
| Hyperlinks · Clipboard · SynchronizedOutput | OSC / host |
| ImageProtocols · TextSizing | Extensions |
| AlternateScreen · InlineRendering | Session mode |
| Multiplexer · Ssh · WindowsConPty | Environment context |

### 2.2 Effective set (`CapabilitySet`)

Resolved booleans + `ColorCapability` + `GlyphSet`.  

**Law:** widgets and session hosts **read** `CapabilitySet` (or DesignSystem derived from it). They do **not** probe `env` ad hoc at paint time.

```rust
pub struct CapabilitySet {
    pub color: ColorCapability,      // Truecolor | Indexed256 | Ansi16 | Monochrome
    pub glyphs: GlyphSet,            // Unicode | Ascii
    pub mouse: bool,
    pub bracketed_paste: bool,
    pub hyperlinks: bool,
    pub clipboard: bool,
    pub enhanced_keyboard: bool,
    pub synchronized_output: bool,
    pub image_protocols: bool,
    pub text_sizing: bool,
    pub alternate_screen: bool,
    pub inline: bool,
    pub keyboard: bool,
    pub multiplexer: bool,  // informational
    pub ssh: bool,
    pub windows_conpty: bool,
}
```

### 2.3 Profiles (`CapabilityProfile`)

| Profile | Intent | Baseline highlights |
|---------|--------|---------------------|
| **Modern** | Best modern TUI | Truecolor, Unicode, mouse, paste, alt-screen; images allowed; enhanced keyboard opt-in |
| **Compatible** | Mux / unknown hosts | 256-color, Unicode, mouse/paste; OSC 52/8 **off** by default |
| **Minimal** | Broken / dumb TERM | Mono + ASCII, keyboard only, no alt-screen extras |
| **Inline** | Scrollback-friendly agent embed | Compatible-ish, **`alternate_screen = false`**, `inline = true` |
| **Headless** | CI / buffer paint | No interactive session; keyboard/mouse off |

### 2.4 Resolution order

```
1. Profile baseline (preferred API or auto)
2. Clamp to detected environment (color ladder, NO_COLOR, dumb TERM)
3. Explicit overrides (CapabilityOverrides + TERMROCK_* / NO_COLOR)
4. Exclusivity rules:
     - inline  ⇒  ¬alternate_screen
     - headless ⇒  ¬keyboard ∧ ¬mouse ∧ ¬paste extras
     - NoColor  ⇒  Monochrome (wins over COLORTERM)
```

**Auto profile heuristics:**

| Hint | Profile |
|------|---------|
| `TERM=dumb` / no TTY | Minimal or Headless |
| SSH and/or multiplexer, no user preference | Compatible |
| Windows ConPTY without WT truecolor proof | Compatible |
| Otherwise | Modern |

### 2.5 Overrides (never hidden)

**API:** `CapabilityOverrides { profile, color, glyphs, mouse, paste, hyperlinks, clipboard, … }`  
**Env:**

| Variable | Values |
|----------|--------|
| `TERMROCK_PROFILE` | `modern` · `compatible` · `minimal` · `inline` · `headless` |
| `TERMROCK_COLOR` | `truecolor` · `256` · `16` · `mono` |
| `TERMROCK_GLYPHS` | `unicode` · `ascii` |
| `NO_COLOR` | any → mono (standard) |

Feature crates (crossterm session, OSC emit) only **emit** sequences when flags are true — degradation never requires a missing crate.

---

## 3. Detection (`detect_environment`)

Best-effort, **no DA queries required** in the library (portable + unit-testable):

| Source | Signals |
|--------|---------|
| Env | `TERM`, `COLORTERM`, `NO_COLOR`, `TERM_PROGRAM` |
| Mux | `TMUX`, `ZELLIJ`, `TERM=screen*` / `tmux*` |
| SSH | `SSH_CONNECTION`, `SSH_TTY` |
| Windows | ConPTY / Windows Terminal program hints |

Interactive probes (DECRQM, XTVERSION, kitty keyboard, image query) remain **host-owned**. Results may feed `CapabilityOverrides` after probe.

### Suspicious configs (doctor warnings)

| Pattern | Why |
|---------|-----|
| `NO_COLOR` set + `COLORTERM=truecolor` | Conflicting user intent |
| `TERM=screen*` + truecolor without mux RGB | Broken colors in tmux |
| `TERM=dumb` | Minimal posture expected |
| SSH + clipboard enabled | OSC 52 often blocked |
| `TERM` unset | Unreliable detection |
| Alt-screen + Inline both requested | Exclusivity fix applied |

---

## 4. Fallback table (every capability)

| Capability | Fallback when off / unavailable |
|------------|----------------------------------|
| Truecolor | Quantize theme 256 → 16 → mono (`ColorCapability`) |
| 256 | Ansi16 or mono |
| ANSI 8/16 | Monochrome + modifiers |
| No color | Roles become mono styles; glyphs carry status |
| Unicode | `GlyphSet::Ascii` |
| ASCII-only | Forced even if terminal Unicode-capable |
| Keyboard | Headless / no session — no raw mode |
| Enhanced keyboard | Conventional `KeyEvent` mapping |
| Mouse | Keyboard equivalents; `mouse_capture=false` |
| Bracketed paste | Rapid-key paste or disabled; `Event::Paste` may be empty |
| Hyperlinks | Plain text URLs; no OSC 8 |
| Clipboard | Outcome only (`CopyPayload`); host no-ops OSC 52 |
| Synchronized output | Normal sequential writes |
| Images | Cell/alt-text via `CapabilityPreviewHost` / ImageSurface |
| Text-sizing | Ignore; cell grid geometry only |
| Alternate screen | Use **Inline** profile (main buffer) |
| Inline | Default when alt-screen off |
| Multiplexer | Compatible defaults + doctor tips |
| SSH | Compatible; clipboard/hyperlinks off |
| ConPTY | Compatible keyboard/mouse; host owns PTY quirks |

Canonical table in code: `fallback_policies()` — **must stay length = `CapabilityKind::ALL`**.

---

## 5. Per-capability contract (five requirements)

For **each** optional capability:

| # | Requirement | Where |
|---|-------------|--------|
| 1 | **Detection** | `detect_environment` and/or host probe hook |
| 2 | **User override** | `CapabilityOverrides` + env |
| 3 | **Documented fallback** | `FallbackPolicy` |
| 4 | **Studio story** | id in `FallbackPolicy.story` |
| 5 | **Contract test** | unit / quality axis |

**CI (phase C2+):** every `CapabilityKind` has a fallback entry; story ids exist when claimed covered; doctor lists all kinds.

---

## 6. `termrock doctor`

### Behavior

1. Detect environment → resolve profile + effective set  
2. Explain **every** capability on/off + fallback text  
3. Show chosen profile and resolution sources  
4. Describe live visual sample (host paints Role swatches + glyph row)  
5. Flag suspicious TERM / mux / SSH  
6. Recommend safe `TERMROCK_*` / `NO_COLOR` overrides  

### Crate API (I/O-free)

```rust
use termrock::{
    CapabilityOverrides, CapabilityProfile, build_doctor_report, format_doctor_text,
};

let report = build_doctor_report(None, CapabilityOverrides::from_env());
print!("{}", format_doctor_text(&report));
// Live sample: Theme::default().quantized(report.effective.set.color)
//              + GlyphSet from report.effective.set.glyphs
```

### CLI

```
termrock doctor
termrock doctor --profile compatible
termrock doctor --json          # future
```

Doctor is a **host** of the library — never a hard dep for widgets to compile/paint.

---

## 7. Session wiring

```rust
let eff = resolve_capabilities(None, overrides);
let flags = eff.session_flags(); // → SessionOptions when feature=crossterm
let theme = Theme::default().quantized(eff.set.color);
let glyphs = eff.set.glyphs;
// DesignSystem / DesignTokens carry glyphs + quantized theme
// Session: mouse_capture, bracketed_paste, alt_screen only if flags true
```

Optional features **off** ⇒ leave flags false — **no panic**, no missing symbols at link time for pure paint.

---

## 8. Studio stories (required matrix)

| Story id | Proves |
|----------|--------|
| `capability/color-ladder` | truecolor→256→16→mono swatches |
| `capability/no-color` | mono state still readable |
| `capability/ascii-glyphs` | disclosure/selection ASCII |
| `capability/keyboard-basic` | conventional keyboard only |
| `capability/no-mouse` | keyboard-only path |
| `capability/paste-fallback` | paste without bracketed |
| `capability/hyperlink-off` | text-only links |
| `capability/clipboard-off` | copy outcome without OSC |
| `capability/sync-off` | normal redraw |
| `capability/image-fallback` | cell fallback |
| `capability/text-sizing-off` | grid only |
| `capability/inline` | no alt-screen chrome |
| `capability/multiplexer` | doctor warning path |
| `capability/ssh` | compatible recommendations |
| `capability/conpty` | windows hint |
| `capability/headless` | no keyboard session |

Quality axes `no_color`, `color_ladder`, `ascii_fallback` point here as evidence.

---

## 9. Multiplexers, SSH, ConPTY

| Context | Default posture |
|---------|-----------------|
| tmux / screen / zellij | **Compatible**; truecolor only if Tc/RGB known; doctor warns |
| SSH | **Compatible**; clipboard off; hyperlinks off |
| ConPTY / Windows Terminal | Compatible keyboard/mouse; host adapter owns PTY quirks |
| Inline agent embed | **Inline** profile |
| CI / snapshots | **Headless** or Minimal |

---

## 10. Architecture diagram

```
┌─────────────────────────────────────────────────────────┐
│ detect_environment()     CapabilityOverrides / env      │
└─────────────┬───────────────────────┬───────────────────┘
              │                       │
              ▼                       ▼
        ┌─────────────────────────────────────┐
        │ resolve_capabilities(profile, …)    │
        │  → EffectiveCapabilities            │
        │     · CapabilitySet                 │
        │     · SessionFlags                  │
        │     · sources (for doctor)          │
        └──────────────┬──────────────────────┘
                       │
        ┌──────────────┼──────────────────────┐
        ▼              ▼                      ▼
  Theme.quantized   GlyphSet            SessionOptions
  (paint roles)     (ASCII/Unicode)     (mouse/paste/alt)
        │              │                      │
        └──────────────┴──────────┬───────────┘
                                  ▼
                         Widgets / OverlayStack
                    (never probe env at paint time)
```

---

## 11. Implementation plan

| Phase | Work | Status |
|-------|------|--------|
| **C0** | Design + `capability` module + doctor text + resolve/overrides | **Done** |
| **C1** | SessionOptions map; README env docs | Partial |
| **C2** | Studio capability stories (color-ladder, no-color, ascii, headless) | **Partial** (first stories shipped) |
| **C3** | `termrock doctor` CLI (`termrock doctor [--profile …]`) | **Done** |
| **C4** | Optional DA probes behind host trait | Later |
| **C5** | Quality contract evidence links | Later |

---

## 12. Decision summary

1. **Profiles + overrides + detection** — never silent hard dependencies.  
2. **`ColorCapability` + `GlyphSet`** are the only paint adapters for color/glyphs.  
3. **OSC / images / session modes** are enable flags, not required crates.  
4. **Doctor** is the user-facing truth surface (crate API + CLI host).  
5. **`fallback_policies()`** must stay complete for every `CapabilityKind`.  
6. Widgets read **resolved set** only — no ad hoc `std::env` in render.

---

## 13. References

- `crates/termrock/src/capability/`  
- `crates/termrock/src/style/quantize.rs`  
- `crates/termrock/src/crossterm/session.rs`  
- `crates/termrock/src/osc/`  
- `docs/design/component-quality-standard.md`  
- `docs/design/terminal-design-system.md`
