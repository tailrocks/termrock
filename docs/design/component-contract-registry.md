# ComponentContract and registry metadata

| Field | Value |
|-------|-------|
| **Status** | Binding |
| **Migration** | `0093-v0.13.0-component-contract-registry.md` |
| **Module** | `termrock::registry` |
| **CLI** | `termrock contract list\|check` |
| **Studio** | `registry/contracts` |

## Preserve / migrate / split / delete

| Surface | Fate |
|---------|------|
| `docs/api/component-contracts.json` (v1 six-axis) | **Preserve** for quality CI |
| `component-contracts.v2*` quality axes | **Preserve** (quality evidence) |
| CLI `plan/add/diff/check` fixture entries | **Preserve** (schema 1 install) |
| Ad-hoc COMPONENTS.md-only inventory | **Migrate** toward `ComponentContract` |
| Kernel as npm-style copy of focus/Esc | **Forbidden** |

## Mission

One machine-readable contract every inventory item can satisfy:

files · dependencies · capabilities · anatomy · semantic roles · variants ·
outcomes · stories · tests · migration · provenance · source hash · license ·
kind (primitive / component / behavior / block / theme / keymap / template).

Enables linting, CI, docs, Studio browse, private registries, source-owned updates.

## API

```rust
RegistryItemKind::{Primitive, Component, Behavior, Block, Theme, Keymap, Template}
ComponentContract { schema: 3, id, kind, files, dependencies, capabilities, … }
validate_contract / validate_contracts → ValidationReport
official_kernel_contracts() / official_contract("Panel")
```

## CLI

```bash
termrock contract list
termrock contract check
```

## Laws

1. Kernel engines stay in the crate; registry may copy **composition/brand**.
2. Contracts are untrusted when loaded from remote registries (CLI path law).
3. `complete=true` requires stories + tests evidence.
4. No `..` path segments in contract files.
