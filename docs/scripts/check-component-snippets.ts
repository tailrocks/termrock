import { readdir } from 'node:fs/promises'

const root = `${import.meta.dir}/../..`
const docs = `${root}/docs/content/docs/components`
const scratch = `${root}/target/component-doc-snippets`
const mkdir = Bun.spawnSync(['mkdir', '-p', `${scratch}/src`])
if (mkdir.exitCode !== 0) throw new Error(mkdir.stderr.toString())

const files = (await readdir(docs)).filter((name) => name.endsWith('.mdx')).toSorted()
if (files.length !== 165) throw new Error(`canonical component page drift: ${files.length}`)
const stories = await Bun.file(`${root}/crates/termrock-lookbook/src/stories.rs`).text()

for (const file of files) {
  const body = await Bun.file(`${docs}/${file}`).text()
  const usage = body.match(/^## Usage\n([\s\S]*?)(?=^## |(?![\s\S]))/m)?.[1]
  const source = usage?.match(/```rust\s*\n([\s\S]*?)\n```/)?.[1]?.trim()
  if (!source || !source.includes('// Exact paint setup used by the shared catalog story.')) {
    throw new Error(`${file}: canonical Usage is not the exact shared story setup`)
  }
  const functionSource = source.slice(source.indexOf('\nfn ') + 1)
  if (!functionSource.startsWith('fn ') || !stories.includes(functionSource)) {
    throw new Error(`${file}: displayed story function differs from compiled catalog source`)
  }
}

// The exact displayed functions compile in termrock-lookbook. Separately prove
// every public widget named by the canonical inventory remains importable by a
// consumer crate, rather than accepting source-text resemblance alone.
const api = await Bun.file(`${root}/docs/api/public-api.txt`).text()
const names = [
  ...new Set(
    [...api.matchAll(/^impl.*ratatui_core::widgets::(?:widget::Widget|stateful_widget::StatefulWidget) for &?termrock::widgets::([A-Z][A-Za-z0-9_]*)/gm)]
      .map((match) => match[1]!),
  ),
].toSorted()
if (names.length !== 135) throw new Error(`public widget inventory drift: ${names.length}`)

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
use termrock::widgets::{${names.join(',')}};
fn main() {}
`,
)
const consumer = Bun.spawnSync(
  ['cargo', 'check', '--quiet', '--manifest-path', `${scratch}/Cargo.toml`],
  { cwd: root, stdout: 'inherit', stderr: 'inherit' },
)
if (consumer.exitCode !== 0) throw new Error('canonical public widget imports do not compile')
const catalog = Bun.spawnSync(
  ['cargo', 'check', '--quiet', '-p', 'termrock-lookbook', '--all-targets'],
  { cwd: root, stdout: 'inherit', stderr: 'inherit' },
)
if (catalog.exitCode !== 0) throw new Error('compiled shared story source is invalid')
console.log(`verified 165 exact catalog snippets and ${names.length} public widget imports`)
