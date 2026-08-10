# shadcn/ui signup blocks → TermRock TUI coverage

**Source:** [ui.shadcn.com/blocks/signup](https://ui.shadcn.com/blocks/signup)  
**Locked crawl:** 2026-08 (web snippets + registry ids; live fetch may be SSRF-blocked).

**TermRock surface:** `patterns::AuthEntry` (migration 0248) + existing `TextInput`,
`PasswordInput` / `PasswordConfirmState`, `Checkbox`, `Panel`, `Callout`.

**Statuses:** `covered` | `partial` | `missing` | `N/A`

## Notes

- Web signup blocks differ mainly by **layout chrome** (centered card, muted
  surface, two-column cover image). Terminal physics collapse those to one
  keyboard-first **auth-entry** composition: fields + validate + submit/cancel
  + mode switch (sign-up ↔ sign-in) + optional terms.
- Host owns network auth, OAuth, CAPTCHA, email verify, and secret vaults.
- Continuous cover imagery and marketing hero splits are **N/A** (no cell
  geometry theater). Optional host-projected **aside text** lines map the
  “second column copy” job without images.

## Matrix

| # | shadcn block | Status | TermRock surface(s) | Notes |
|---|--------------|--------|---------------------|-------|
| 1 | signup-01 — simple signup form | covered | **`AuthEntry`** (SignUp) | Identity + password (+ confirm) + submit/cancel |
| 2 | signup-02 — two-column with cover image | partial | `AuthEntry` + optional aside text | Cover image N/A; aside copy optional |
| 3 | signup-03 — muted background page | covered | `AuthEntry` + design-system surface | Muted chrome via theme roles, not a second API |
| 4 | signup-04 — form and image | partial | `AuthEntry` + optional aside text | Image N/A; same fielded gate as signup-01 |
| 5 | signup-05 — social providers | partial | `AuthEntry` `SecondaryAction` (e.g. `oauth:…`) | Brand OAuth button grid N/A; host maps provider ids (Ctrl+O peer) |

## Counts

| Status | Count |
|--------|------:|
| covered | 2 |
| partial | 3 |
| missing | 0 |
| N/A | 0 |
| **Total** | **5** |

## Port decision

| Gap | Decision |
|-----|----------|
| Simple / muted form pages | Ship `AuthEntry` — CLI/TUI credential gate |
| Image / cover columns | Keep partial — optional aside strings only |
| OAuth / social (signup-05) | Host secondary actions via `SecondaryAction` ids; no fake brand paint |
| Full blocks catalog (login siblings, dashboards) | See consolidated `shadcn-blocks-coverage.md` |

## Validation

```bash
rtk cargo test -p termrock --lib auth_entry
rtk cargo check -p termrock
```
