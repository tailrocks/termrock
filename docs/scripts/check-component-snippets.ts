import { componentDocs } from './component-docs'

const root = `${import.meta.dir}/../..`
const scratch = `${root}/target/component-doc-snippets`
const mkdir = Bun.spawnSync(['mkdir', '-p', `${scratch}/src`])
if (mkdir.exitCode !== 0) throw new Error(mkdir.stderr.toString())

const blocks = Object.entries(componentDocs)
  .toSorted(([left], [right]) => left.localeCompare(right))
  .map(([component, doc]) => `    // ${component}\n    {\n${doc.usage.split('\n').map((line) => `        ${line}`).join('\n')}\n    }`)
  .join('\n\n')

await Bun.write(
  `${scratch}/Cargo.toml`,
  `[package]
name = "termrock-component-doc-snippets"
version = "0.0.0"
edition = "2024"
publish = false

[workspace]

[dependencies]
ratatui-core = "0.1.2"
termrock = { path = "${root}/crates/termrock" }
`,
)
await Bun.write(
  `${scratch}/src/main.rs`,
  `#![allow(unused_variables)]

fn main() {
${blocks}
}
`,
)

const result = Bun.spawnSync(
  ['cargo', 'check', '--quiet', '--manifest-path', `${scratch}/Cargo.toml`],
  { cwd: root, stdout: 'inherit', stderr: 'inherit' },
)
if (result.exitCode !== 0) throw new Error('component usage snippets do not compile')
console.log(`compiled ${Object.keys(componentDocs).length} component usage snippets`)
