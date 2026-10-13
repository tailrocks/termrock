# Official TermRock kernel contracts

Machine-readable inventory for kernel-hosted items lives primarily as
`termrock::registry::official_kernel_contracts()` (validated in CI).

This directory holds **source-owned / private registry** style packages and
fixtures. Offline install still uses `registry/fixtures/*/entry.json` via:

```bash
termrock check registry/fixtures/tiny-component
termrock contract list
termrock contract check
```

Schema: `ComponentContract` schema version **3** in `termrock::registry`.
