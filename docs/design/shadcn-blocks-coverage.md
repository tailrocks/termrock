# shadcn/ui first-party blocks → TermRock TUI coverage (consolidated)

**Source:** [ui.shadcn.com/blocks](https://ui.shadcn.com/blocks) + category indexes  
**Locked crawl:** 2026-08 (web snippets; live fetch may be SSRF-blocked).

**Family detail docs (non-drift peers):**
- Signup: `docs/design/shadcn-signup-blocks-coverage.md`
- Login: `docs/design/shadcn-login-blocks-coverage.md`
- Sidebar detail: `docs/design/shadcn-sidebar-blocks-coverage.md` (01…16)

**Statuses:** `covered` | `partial` | `missing` | `N/A`

## First-party lock set

Official featured + linked families only (not third-party marketplaces).  
**Re-lock (2026-08 registry/category crawl + skeptic recheck):** includes `sidebar-16`
(sticky header) and `signup-05` (social providers).

| Family | IDs locked |
|--------|------------|
| Dashboard | `dashboard-01` |
| Sidebar | `sidebar-01` … `sidebar-16` |
| Login | `login-01` … `login-05` |
| Signup | `signup-01` … `signup-05` |

**Total rows:** 1 + 16 + 5 + 5 = **27**

## Matrix

| # | Block id | Status | TermRock surface(s) | Notes |
|---|----------|--------|---------------------|-------|
| 1 | dashboard-01 | covered | **`AppDashboard`** (0251), `AppShell`, `Sidebar`, charts/DataTable host panes | Sidebar + metrics/main/footer; host paints charts/table |
| 2 | sidebar-01 | covered | `Sidebar` / sectioned nav | See sidebar family doc |
| 3 | sidebar-02 | covered | Collapsible sections + `filter_nav_collapsed` | |
| 4 | sidebar-03 | covered | Nested depth / submenus | |
| 5 | sidebar-04 | partial | Drawer presentation | No CSS floating shadow |
| 6 | sidebar-05 | covered | Collapsible submenus | |
| 7 | sidebar-06 | partial | `ContextMenuRequested` | Host menu overlay |
| 8 | sidebar-07 | covered | Rail / icon collapse | |
| 9 | sidebar-08 | covered | Inset + secondary sections | Same section model |
| 10 | sidebar-09 | covered | Nested collapsible | `filter_nav_collapsed` |
| 11 | sidebar-10 | partial | Popover → Drawer / palette | Host overlay |
| 12 | sidebar-11 | partial | File tree → `Tree` / deep `NavItem` | Host projects tree rows |
| 13 | sidebar-12 | partial | Calendar in rail → `DateTimePicker` host pane | Not embedded web calendar |
| 14 | sidebar-13 | partial | Sidebar in dialog → `Dialog` / drawer | Host chrome |
| 15 | sidebar-14 | covered | Right rail | Host places `AppShell` zone |
| 16 | sidebar-15 | partial | Dual sidebars | Sidebar + inspector slots |
| 17 | sidebar-16 | covered | Sticky header band | `AppShell` header + `Sidebar`/`Panel` title chrome (no CSS sticky) |
| 18 | login-01 | covered | `AuthEntry::sign_in` | |
| 19 | login-02 | partial | AuthEntry + aside | Image N/A |
| 20 | login-03 | covered | AuthEntry + theme surface | |
| 21 | login-04 | partial | AuthEntry + aside | Image N/A |
| 22 | login-05 | covered | `AuthEntry::email_only` | |
| 23 | signup-01 | covered | `AuthEntry` SignUp | |
| 24 | signup-02 | partial | AuthEntry + aside | Image N/A |
| 25 | signup-03 | covered | AuthEntry + theme | |
| 26 | signup-04 | partial | AuthEntry + aside | Image N/A |
| 27 | signup-05 | partial | AuthEntry `SecondaryAction` oauth ids | Social brand button grid N/A; host maps provider ids |

## Counts

| Status | Count |
|--------|------:|
| covered | 15 |
| partial | 12 |
| missing | 0 |
| N/A | 0 |
| **Total** | **27** |

## Port decisions (this pass)

| Gap | Decision |
|-----|----------|
| dashboard-01 | Ship `AppDashboard` — keyboard shell: sidebar ↔ main, rail, host main paint |
| sidebar-08…16 | Map onto Sidebar + AppShell; sticky header = shell header band |
| login/signup | Prior AuthEntry work (0248–0249); signup-05 = oauth secondary actions |
| sidebar-01…07 | Prior Sidebar work (0250) |

## Validation

```bash
rtk cargo test -p termrock --lib app_dashboard
rtk cargo test -p termrock --lib auth_entry
rtk cargo test -p termrock --lib sidebar
rtk cargo check -p termrock
```
