import { join } from 'node:path'
import { componentSlug } from './component-doc-utils'

const args = Bun.argv.slice(2)
const flag = args.indexOf('--component')
const component = flag >= 0 ? args[flag + 1] : undefined
if (!component || !/^[A-Z][A-Za-z0-9]+$/.test(component)) {
  throw new Error('usage: bun run scripts/scaffold-component-page.ts --component <Type>')
}

const root = join(import.meta.dir, '..', '..')
const path = join(root, 'docs', 'content', 'docs', 'components', `${componentSlug(component)}.mdx`)
if (await Bun.file(path).exists()) throw new Error(`refusing to overwrite ${path}`)

const result = Bun.spawnSync(
  ['cargo', 'run', '-q', '-p', 'termrock-lookbook', '--', 'list', '--format', 'json'],
  { cwd: root },
)
if (result.exitCode !== 0) throw new Error(result.stderr.toString())
const catalog = JSON.parse(result.stdout.toString()) as Array<{ id: string; component: string }>
const demo = catalog.find((entry) => entry.component === component)
if (!demo) throw new Error(`${component}: add a shared Lookbook demo before scaffolding docs`)

await Bun.write(path, `---
title: ${component}
description: TODO
component: ${component}
demo: ${demo.id}
interaction: TODO
actions: []
expectedOutcomes: []
source: TODO
---

## Live terminal

<TerminalPreview story="${demo.id}" interactive />

## Usage

\`\`\`rust
// TODO: exact public API used by the demo.
\`\`\`

## Interaction contract

TODO

## Stories

TODO

## Try it

TODO

## State and typed outcomes

TODO

## Common mistakes

TODO

## Test recipe

TODO

## Source and related material

TODO
`)
console.log(`created ${path}`)
