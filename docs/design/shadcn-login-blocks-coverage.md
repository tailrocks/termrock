# shadcn/ui login blocks → TermRock TUI coverage

**Source:** [ui.shadcn.com/blocks/login](https://ui.shadcn.com/blocks/login)  
**Locked crawl:** 2026-08 (web snippets; live fetch may be SSRF-blocked).

**TermRock surface:** `patterns::AuthEntry` — `SignIn` / `EmailOnly` (migrations
0248 + **0249** email-only path) + `TextInput`, `PasswordInput`, `Panel`,
secondary-action outcomes.

**Statuses:** `covered` | `partial` | `missing` | `N/A`

## Notes

- Login web blocks differ mainly by **layout chrome** (centered form, muted
  surface, two-column cover image). Terminal physics collapse those to one
  keyboard-first **auth-entry** gate: identity + password, or **email-only**
  passwordless request.
- Prefer extending `AuthEntry` over a dual login composition.
- Host owns network auth, OAuth, magic-link delivery, CAPTCHA, and vaults.
- Continuous cover imagery is **partial** (optional aside text only).

## Matrix

| # | shadcn block | Status | TermRock surface(s) | Notes |
|---|--------------|--------|---------------------|-------|
| 1 | login-01 — simple login form | covered | **`AuthEntry::sign_in`** | Identity + password + submit/cancel |
| 2 | login-02 — two-column / cover | partial | `AuthEntry` SignIn + aside text | Cover image N/A; aside copy optional |
| 3 | login-03 — muted background | covered | `AuthEntry` SignIn + design-system surface | Theme roles, not a second API |
| 4 | login-04 — form and image | partial | `AuthEntry` SignIn + aside text | Image N/A; same fielded gate |
| 5 | login-05 — email-only | covered | **`AuthEntry::email_only`** (0249) | Identity submit → host magic-link |

## Counts

| Status | Count |
|--------|------:|
| covered | 3 |
| partial | 2 |
| missing | 0 |
| N/A | 0 |
| **Total** | **5** |

## Port decisions

| Gap | Decision |
|-----|----------|
| Simple / muted password login | `AuthEntry::sign_in` (0248) |
| Email-only / magic-link request | `AuthEntryMode::EmailOnly` + `email_only()` (0249) |
| Image / cover columns | partial — aside strings only |
| OAuth button grids | `SecondaryAction { id: "oauth:…" }` host maps |
| Forgot password | `SecondaryAction { id: "forgot-password" }` (Ctrl+F) |

## Validation

```bash
rtk cargo test -p termrock --lib auth_entry
rtk cargo check -p termrock
```
