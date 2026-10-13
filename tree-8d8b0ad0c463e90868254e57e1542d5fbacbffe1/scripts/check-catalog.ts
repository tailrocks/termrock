import { readFileSync, readdirSync } from 'node:fs'
import { join } from 'node:path'

const root = join(import.meta.dirname, '..', '..')
const process = Bun.spawnSync(['cargo', 'run', '-q', '-p', 'termrock-lookbook', '--', 'list', '--format', 'json'], { cwd: root })
if (process.exitCode !== 0) throw new Error(process.stderr.toString())
const stories = JSON.parse(process.stdout.toString()) as Array<{ id: string; component: string }>
const ids = new Set(stories.map((story) => story.id))
if (ids.size !== stories.length) throw new Error('duplicate story ID')

const api = readFileSync(join(root, 'docs', 'api', 'public-api.txt'), 'utf8')
const publicComponents = new Set<string>()
for (const line of api.split('\n')) {
  const match = line.match(/^impl.*ratatui_core::widgets::(?:widget::Widget|stateful_widget::StatefulWidget) for &?termrock::widgets::([A-Z][A-Za-z0-9_]*)/)
  if (match) publicComponents.add(match[1])
}
if (publicComponents.size === 0) throw new Error('public API report contains no canonical widgets')

const storyComponents = new Set(stories.map((story) => story.component))
const missingStories = [...publicComponents].filter((component) => !storyComponents.has(component))
if (missingStories.length) throw new Error(`public components without stories: ${missingStories.join(', ')}`)
const unknownStories = [...storyComponents].filter((component) => !publicComponents.has(component))
if (unknownStories.length) throw new Error(`stories without public components: ${unknownStories.join(', ')}`)

const contractPath = join(root, 'docs', 'api', 'component-contracts.json')
const contracts = JSON.parse(readFileSync(contractPath, 'utf8')) as Record<string, Record<string, string>>
const contractComponents = new Set(Object.keys(contracts))
const missingContracts = [...publicComponents].filter((component) => !contractComponents.has(component))
const staleContracts = [...contractComponents].filter((component) => !publicComponents.has(component))
if (missingContracts.length) throw new Error(`public components without contract review: ${missingContracts.join(', ')}`)
if (staleContracts.length) throw new Error(`contract review contains non-public components: ${staleContracts.join(', ')}`)
const contractNames = ['keyboard', 'mouse', 'focus', 'nonColor', 'unicode', 'narrowTerminal']
const contractValues = new Set(['covered', 'caller-owned', 'not-applicable'])
for (const [component, review] of Object.entries(contracts)) {
  for (const contract of contractNames) {
    if (!contractValues.has(review[contract])) throw new Error(`${component} has no valid ${contract} contract`)
  }
}

const docsDir = join(root, 'docs', 'content', 'docs')
const docs = readdirSync(docsDir).filter((name) => name.endsWith('.mdx')).map((name) => readFileSync(join(docsDir, name), 'utf8')).join('\n')
for (const story of stories) {
  if (!docs.includes(`\`${story.id}\``)) throw new Error(`missing docs for story ${story.id}`)
  const preview = join(root, 'docs', 'public', 'component-previews', `${story.id.replaceAll('/', '-')}.svg`)
  if (!Bun.file(preview).size) throw new Error(`missing preview ${preview}`)
}
console.log(`catalog covers ${publicComponents.size} public components with ${stories.length} stories`)
