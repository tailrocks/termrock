# Public API review

The reviewed report confirms stable-ID interaction surfaces, borrowed render data, pure rendering and OSC encoding, empty default features, no executor dependency, and Crossterm isolation. Application nouns and policies are absent from public signatures. The post-`v0.6.0` migration deliberately moves dialog layout, focus/hover/modal lifecycle, scroll rendering, and the modal backdrop into their canonical `layout`, `interaction`, `scroll`, and `widgets` namespaces; the report must not reintroduce their former donor-shaped component submodules.

## Deterministic regeneration

Both the manifest generator and rustdoc schema are pinned. Changing either pin
is an explicit manifest-format migration: regenerate twice and require
byte-identical output before committing the snapshot.

```bash
rustup toolchain install nightly-2026-09-01 --profile minimal
RUSTUP_TOOLCHAIN=nightly-2026-09-01 mise x cargo:cargo-public-api@0.52.0 -- \
  cargo public-api -p termrock --all-features > docs/api/public-api.txt
```
