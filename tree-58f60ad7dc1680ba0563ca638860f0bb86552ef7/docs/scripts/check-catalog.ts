import { readdir } from 'node:fs/promises'
import { join } from 'node:path'

type Demo = {
  id: string
  component: string
  interactive: boolean
  interactionKind: string
  hints: string[]
}

const root = join(import.meta.dir, '..', '..')
const docs = join(root, 'docs', 'content', 'docs')
const result = Bun.spawnSync(
  ['cargo', 'run', '-q', '-p', 'termrock-lookbook', '--', 'list', '--format', 'json'],
  { cwd: root },
)
if (result.exitCode !== 0) throw new Error(result.stderr.toString())
const catalog = JSON.parse(result.stdout.toString()) as Demo[]
const ids = new Set(catalog.map((entry) => entry.id))
if (ids.size !== catalog.length) throw new Error('duplicate demo id')

const api = await Bun.file(join(root, 'docs', 'api', 'public-api.txt')).text()
const publicWidgets = new Set(
  [...api.matchAll(/^impl.*ratatui_core::widgets::(?:widget::Widget|stateful_widget::StatefulWidget) for &?termrock::widgets::([A-Z][A-Za-z0-9_]*)/gm)]
    .map((match) => match[1]!),
)
if (publicWidgets.size !== 136) throw new Error(`public widget inventory drift: ${publicWidgets.size}`)
const storyComponents = new Set(catalog.map((entry) => entry.component))
const missingStories = [...publicWidgets].filter((component) => !storyComponents.has(component))
if (missingStories.length) throw new Error(`public widgets without demos: ${missingStories.join(', ')}`)

const contracts = JSON.parse(
  await Bun.file(join(root, 'docs', 'api', 'component-contracts.json')).text(),
) as Record<string, Record<string, string>>
const contractComponents = new Set(Object.keys(contracts))
const missingContracts = [...publicWidgets].filter((component) => !contractComponents.has(component))
const staleContracts = [...contractComponents].filter((component) => !publicWidgets.has(component))
if (missingContracts.length || staleContracts.length) {
  throw new Error(`contract inventory mismatch; missing=${missingContracts.join(', ')} stale=${staleContracts.join(', ')}`)
}

async function mdxFiles(directory: string): Promise<string[]> {
  const entries = await readdir(directory, { withFileTypes: true })
  return (
    await Promise.all(
      entries.map(async (entry) => {
        const path = join(directory, entry.name)
        if (entry.isDirectory()) return mdxFiles(path)
        return entry.isFile() && entry.name.endsWith('.mdx') ? [path] : []
      }),
    )
  ).flat()
}

const embedded = new Set<string>()
for (const path of await mdxFiles(docs)) {
  const body = await Bun.file(path).text()
  if (body.includes('component-previews/') || /!\[[^\]]*\]\([^)]*\.svg\)/.test(body)) {
    throw new Error(`${path}: SVG catalog previews are forbidden`)
  }
  const previews = [...body.matchAll(/<TerminalPreview\s+[^>]*story=["']([^"']+)["']/g)]
  if (previews.length > 1) throw new Error(`${path}: more than one live terminal focus`)
  for (const preview of previews) embedded.add(preview[1]!)
}

if (await Bun.file(join(root, 'docs', 'public', 'preview-frames')).exists()) {
  throw new Error('multi-step preview frame packs are forbidden')
}
for (const id of embedded) {
  if (!ids.has(id)) throw new Error(`unknown embedded demo ${id}`)
  const poster = join(root, 'docs', 'public', 'preview-posters', `${id.replaceAll('/', '-')}.json`)
  if (!(await Bun.file(poster).exists()) || (await Bun.file(poster).size) === 0) {
    throw new Error(`missing one-frame fallback poster ${poster}`)
  }
}

const routes = JSON.parse(
  await Bun.file(join(root, 'docs', 'api', 'component-routes.json')).text(),
) as unknown[]
const patterns = JSON.parse(
  await Bun.file(join(root, 'docs', 'api', 'pattern-catalog.json')).text(),
) as unknown[]
console.log(
  `catalog: ${publicWidgets.size}/136 public widgets, ${routes.length}/166 component routes, ${patterns.length}/35 patterns, ${embedded.size} live demos, ${catalog.length} stories`,
)
