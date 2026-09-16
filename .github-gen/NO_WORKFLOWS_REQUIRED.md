# NO_WORKFLOWS_REQUIRED

Clean-room regeneration blocked: velnor static analysis fails on
`include_str!(concat!(env!("CARGO_MANIFEST_DIR"), ...))` patterns in
`crates/termrock/src/lib.rs`.

Error: `include_str! must use a plain string literal`

No workflow YAML copied. Re-enable CI after velnor analyzer supports
non-literal include_str! paths or termrock refactors test macros.
