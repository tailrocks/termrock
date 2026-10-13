const root = `${import.meta.dir}/../..`
const scratch = `${root}/target/component-doc-snippets`
const mkdir = Bun.spawnSync(['mkdir', '-p', `${scratch}/src`])
if (mkdir.exitCode !== 0) throw new Error(mkdir.stderr.toString())

// Smoke: every documented component must still resolve as a public import.
// Full usage snippets lag HEAD APIs; detailed compile gates live in
// crates/termrock/tests/documentation_examples.rs and lookbook stories.
const api = await Bun.file(`${root}/docs/api/public-api.txt`).text()
const names = [
  ...new Set(
    [...api.matchAll(/^impl.*ratatui_core::widgets::(?:widget::Widget|stateful_widget::StatefulWidget) for &?termrock::widgets::([A-Z][A-Za-z0-9_]*)/gm)]
      .map((match) => match[1]!),
  ),
].toSorted()
if (names.length !== 135) throw new Error(`public widget inventory drift: ${names.length}`)
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
console.log(`resolved ${names.length} public widget imports from canonical API inventory`)
