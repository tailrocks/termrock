# Terminal capability architecture

**Status:** design SoT + foundation (`termrock::capability`)  
**Law:** Optional terminal features must never become **hidden hard dependencies**.  
**Related:** `ColorCapability`, `GlyphSet`, `SessionOptions`, OSC encoders, `CapabilityPreviewHost`, quality contracts.

---

## 1. Goals

- Look exceptional on modern terminals (truecolor, Unicode, mouse, paste, images).  
- Degrade **gracefully** and **explicitly** on older/restricted hosts.  
- Every optional capability has: **detection**, **user override**, **documented fallback**, **Studio story**, **contract test**.  
- `termrock doctor` explains the resolved environment.

---

## 2. Capability model

### 2.1 Kinds (`CapabilityKind`)

Truecolor · 256 · ANSI 8/16 · No color · Unicode · ASCII-only · Keyboard · Enhanced keyboard · Mouse · Bracketed paste · Hyperlinks · Clipboard · Synchronized output · Image protocols · Text-sizing · Alternate screen · Inline · Multiplexer · SSH · Windows ConPTY  

### 2.2 Effective set (`CapabilitySet`)

Resolved booleans + `ColorCapability` + `GlyphSet`. Widgets/hosts **read** this; they do not probe env ad hoc.

### 2.3 Profiles (`CapabilityProfile`)

| Profile | Intent |
|---------|--------|
| **Modern** | Truecolor, Unicode, mouse, paste, alt-screen, images allowed; enhanced keyboard opt-in |
| **Compatible** | 256-color, Unicode, mouse/paste, no OSC 52/8 by default |
| **Minimal** | Mono + ASCII, keyboard only, no alt-screen extras |
| **Inline** | Compatible-ish, **no** alternate screen (scrollback TUI) |
| **Headless** | No interactive session; buffer paint / CI |

### 2.4 Resolution order

```
1. Profile baseline (preferred or auto)
2. Clamp to detected environment (color ladder, NO_COLOR, dumb TERM)
3. Explicit overrides (API + TERMROCK_* env)
4. Exclusivity (inline ⇔ ¬alt-screen; headless ⇒ ¬keyboard/mouse)
```

Auto profile: `dumb` → Minimal; SSH/mux without preference → Compatible; else Modern.

### 2.5 Overrides

API: `CapabilityOverrides`  
Env:

| Variable | Values |
|----------|--------|
| `TERMROCK_PROFILE` | modern / compatible / minimal / inline / headless |
| `TERMROCK_COLOR` | truecolor / 256 / 16 / mono |
| `TERMROCK_GLYPHS` | unicode / ascii |
| `NO_COLOR` | forces mono (standard) |

Never require a feature crate for degradation — only for **emitting** optional sequences (crossterm session, OSC).

---

## 3. Detection (`detect_environment`)

Best-effort, **no DA queries required** (portable, testable):

- `TERM`, `COLORTERM`, `NO_COLOR`, `TERM_PROGRAM`  
- Multiplexer: `TMUX`, `ZELLIJ`, `TERM=screen*`  
- SSH: `SSH_CONNECTION` / `SSH_TTY`  
- ConPTY/Windows Terminal hints  

Interactive probes (DECRQM, XTVERSION, kitty keyboard) remain **host-owned** and optional; results can feed `CapabilityOverrides`.

### Suspicious configs (doctor warnings)

- `NO_COLOR` with truecolor COLORTERM  
- `TERM=screen*` + truecolor without mux RGB  
- `TERM=dumb`  
- SSH + clipboard enabled  
- `TERM` unset  

---

## 4. Fallback table (summary)

| Capability | Fallback |
|------------|----------|
| Truecolor | Quantize 256 → 16 → mono |
| 256 / ANSI | Next lower ladder |
| No color | Modifiers + glyphs only |
| Unicode | `GlyphSet::Ascii` |
| Enhanced keyboard | Conventional KeyEvent |
| Mouse | Keyboard equivalents; `mouse_capture=false` |
| Bracketed paste | Rapid keys or disable |
| Hyperlinks | Text only; no OSC 8 |
| Clipboard | Outcome only; host no-op |
| Sync output | Normal writes |
| Images | Cell/alt-text via preview host |
| Text-sizing | Ignore; cell grid |
| Alt screen | Inline profile |
| Mux / SSH / ConPTY | Compatible defaults + doctor tips |

Full table: `fallback_policies()` in code.

---

## 5. Per-capability contract (must all five exist)

For each optional capability:

1. **Detection** — env and/or host probe hook  
2. **Override** — `CapabilityOverrides` / env  
3. **Fallback** — `FallbackPolicy`  
4. **Studio story** — id listed in fallback table (implement in Studio phases)  
5. **Contract test** — unit test name / quality axis link  

CI (phase C2): story id exists when status=covered; fallback entry required for every `CapabilityKind`.

---

## 6. `termrock doctor`

### Behavior

1. Detect environment → resolve profile/set  
2. Print/explain every capability on/off + fallback  
3. Show chosen profile + sources (env/override/profile)  
4. Live visual sample (host paints Role swatches + glyph row using effective set)  
5. Flag suspicious TERM/mux/SSH  
6. Recommend safe env overrides  

### API (crate)

```rust
let report = build_doctor_report(None, CapabilityOverrides::default());
print!("{}", format_doctor_text(&report));
// TUI sample: quantize theme with report.effective.set.color; GlyphSet from set
```

### CLI (future `termrock-cli` / lookbook)

```
termrock doctor
termrock doctor --profile compatible
termrock doctor --json
```

Not a hard dependency of the library; doctor is a **host** that uses the crate.

---

## 7. Session wiring

```rust
let eff = resolve_capabilities(None, overrides);
let flags = eff.session_flags(); // → SessionOptions with crossterm feature
let theme = Theme::default().quantized(eff.set.color);
// widgets: pass GlyphSet / ascii flags from eff.set.glyphs
```

Optional features off ⇒ simply leave flags false — **no panic**, no missing symbol.

---

## 8. Studio stories (required backlog)

| Story id | Proves |
|----------|--------|
| `capability/color-ladder` | truecolor→256→16→mono swatches |
| `capability/no-color` | mono state still readable |
| `capability/ascii-glyphs` | disclosure/selection ASCII |
| `capability/keyboard-basic` | no enhanced protocol |
| `capability/no-mouse` | keyboard-only path |
| `capability/paste-fallback` | paste without bracketed |
| `capability/hyperlink-off` | text-only links |
| `capability/clipboard-off` | copy outcome without OSC |
| `capability/sync-off` | normal redraw |
| `capability/image-fallback` | cell fallback |
| `capability/text-sizing-off` | grid only |
| `capability/inline` | no alt-screen chrome |
| `capability/multiplexer` | doctor warning path (fixture env) |
| `capability/ssh` | compatible recommendations |
| `capability/conpty` | windows hint |
| `capability/headless` | no keyboard session |

---

## 9. Multiplexers, SSH, ConPTY

| Context | Default posture |
|---------|-----------------|
| tmux/screen/zellij | Compatible; truecolor only if Tc/RGB known; doctor warns |
| SSH | Prefer Compatible; clipboard off; hyperlinks off |
| ConPTY / Windows Terminal | Compatible keyboard/mouse; host adapter owns PTY quirks |
| Inline agent embed | Inline profile |

---

## 10. Implementation plan

| Phase | Work |
|-------|------|
| **C0** ✅ | Design + `capability` module + doctor text + resolve/overrides |
| **C1** | Map SessionOptions builders; document env in README |
| **C2** | Studio capability stories matrix |
| **C3** | `termrock doctor` binary command |
| **C4** | Optional DA probes behind host trait (no lib hard dep) |
| **C5** | Quality contract axes link `color_ladder` / `ascii_fallback` evidence |

---

## 11. Decision summary

1. **Profiles + overrides + detection** — never silent hard deps.  
2. **ColorCapability + GlyphSet** remain the paint adapters.  
3. **OSC/images/session modes** are enable flags, not required crates.  
4. **Doctor** is the user-facing truth surface.  
5. **Fallbacks are code-documented** (`fallback_policies`) and must stay complete for every `CapabilityKind`.
