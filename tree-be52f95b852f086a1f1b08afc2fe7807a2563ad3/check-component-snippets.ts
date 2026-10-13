import { componentDocs } from './component-docs'

const root = `${import.meta.dir}/../..`
const scratch = `${root}/target/component-doc-snippets`
const mkdir = Bun.spawnSync(['mkdir', '-p', `${scratch}/src`])
if (mkdir.exitCode !== 0) throw new Error(mkdir.stderr.toString())

// Smoke: every documented component must still resolve as a public import.
// Full usage snippets lag HEAD APIs; detailed compile gates live in
// crates/termrock/tests/documentation_examples.rs and lookbook stories.
const names = Object.keys(componentDocs).toSorted()
const importList = names.join(',\n    ')

await Bun.write(
  `${scratch}/Cargo.toml`,
  `[package]
name = "termrock-component-doc-snippets"
version = "0.0.0"
edition = "2024"
publish = false

[workspace]

[dependencies]
termrock = { path = "${root}/crates/termrock" }
`,
)
await Bun.write(
  `${scratch}/src/main.rs`,
  `#![allow(unused_imports)]

use termrock::widgets::{
    ${importList},
};

fn main() {
    let _count = ${names.length};
}
`,
)

const result = Bun.spawnSync(
  ['cargo', 'check', '--quiet', '--manifest-path', `${scratch}/Cargo.toml`],
  { cwd: root, stdout: 'inherit', stderr: 'inherit' },
)
if (result.exitCode !== 0) {
  throw new Error(
    'component doc names do not all resolve as termrock::widgets imports',
  )
}
console.log(`resolved ${names.length} component widget imports from component-docs`)
