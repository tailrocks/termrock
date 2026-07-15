# First-publish audit

Checkpoint prepared from the filtered donor boundary recorded in `provenance.toml`.

- The standalone workspace has no Tailrocks product dependency and the base feature graph has no Tokio or Crossterm.
- The `crossterm` feature contains adapters and scoped terminal ownership only.
- Public widgets use borrowed render data, stable IDs, and typed OSC requests.
- The donor component facade and consumer-owned widgets are absent.
- The neutral lookbook registry has unique IDs and deterministic committed renders.
- All post-boundary commits carry DCO sign-offs; the full-history secret and object-size scans recorded during bootstrap found no findings.
- No tags or secondary branches are part of the first publish.

CI/CD status is intentionally deferred to the extraction program's final aggregate verification phase.
