import { readdir } from 'node:fs/promises'
import { join } from 'node:path'
import { frontmatter, scalar } from './doc-frontmatter'

type Pattern = {
  pattern: string
  slug: string
  source: string
  demo: string
  interaction: string
  actions: string[]
  classification: 'application' | 'composite' | 'layout-helper'
  buildingBlocks: string[]
  defaultDimensions: { cols: number; rows: number }
}

type Demo = {
  id: string
  interactionKind: string
  hints: string[]
}

const root = join(import.meta.dir, '..', '..')
const docs = join(root, 'docs', 'content', 'docs')
const patterns = JSON.parse(
  await Bun.file(join(root, 'docs', 'api', 'pattern-catalog.json')).text(),
) as Pattern[]
const catalogResult = Bun.spawnSync(
  ['cargo', 'run', '-q', '-p', 'termrock-lookbook', '--', 'list', '--format', 'json'],
  { cwd: root },
)
if (catalogResult.exitCode !== 0) throw new Error(catalogResult.stderr.toString())
const demos = new Map(
  (JSON.parse(catalogResult.stdout.toString()) as Demo[]).map((demo) => [demo.id, demo]),
)
if (patterns.length !== 35) throw new Error(`pattern catalog drift: ${patterns.length}`)
if (new Set(patterns.map((entry) => entry.slug)).size !== patterns.length) {
  throw new Error('duplicate pattern slug')
}

const api = await Bun.file(join(root, 'docs', 'api', 'public-api.txt')).text()

const modules = (await readdir(join(root, 'crates', 'termrock', 'src', 'patterns')))
  .filter((name) => name.endsWith('.rs') && name !== 'mod.rs')
const sources = new Set(patterns.map((entry) => entry.source.split('/').pop()))
const missingModules = modules.filter((name) => !sources.has(name))
if (missingModules.length || sources.size !== modules.length) {
  throw new Error(`pattern module/catalog mismatch: ${missingModules.join(', ')}`)
}

const pages = (await readdir(join(docs, 'patterns')))
  .filter((name) => name.endsWith('.mdx'))
  .map((name) => name.replace(/\.mdx$/, ''))
  .sort()
const expected = patterns.map((entry) => entry.slug).sort()
if (JSON.stringify(pages) !== JSON.stringify(expected)) {
  throw new Error('pattern pages differ from pattern catalog')
}

for (const pattern of patterns) {
  const path = join(docs, 'patterns', `${pattern.slug}.mdx`)
  const body = await Bun.file(path).text()
  const meta = frontmatter(body)
  if (scalar(meta, 'pattern') !== pattern.pattern || scalar(meta, 'demo') !== pattern.demo) {
    throw new Error(`${pattern.slug}: frontmatter differs from pattern catalog`)
  }
  if (!(await Bun.file(join(root, pattern.source)).exists())) {
    throw new Error(`${pattern.slug}: missing Rust source`)
  }
  const demo = demos.get(pattern.demo)
  if (!demo) throw new Error(`${pattern.slug}: unknown demo ${pattern.demo}`)
  if (pattern.interaction !== demo.interactionKind) {
    throw new Error(`${pattern.slug}: interaction differs from shared demo`)
  }
  if (JSON.stringify(pattern.actions) !== JSON.stringify(demo.hints)) {
    throw new Error(`${pattern.slug}: actions differ from shared demo hints`)
  }
  for (const action of pattern.actions) {
    if (!body.includes('`' + action + '`')) {
      throw new Error(`${pattern.slug}: workflow omits ${action}`)
    }
  }
  if (!['application', 'composite', 'layout-helper'].includes(pattern.classification)) {
    throw new Error(`${pattern.slug}: invalid classification`)
  }
  if (!pattern.buildingBlocks.length || pattern.defaultDimensions.cols < 1 || pattern.defaultDimensions.rows < 1) {
    throw new Error(`${pattern.slug}: incomplete composition/dimensions`)
  }
  if (new RegExp(`termrock::widgets::${pattern.pattern}\\b`).test(api)) {
    throw new Error(`${pattern.slug}: pattern is incorrectly exported as a widget`)
  }
  const previews = [...body.matchAll(/<TerminalPreview\s+[^>]*story=["']([^"']+)["']/g)]
  if (previews.length !== 1 || previews[0]?.[1] !== pattern.demo) {
    throw new Error(`${pattern.slug}: needs exactly one matching live preview`)
  }
  for (const heading of ['## Live application preview', '## Classification and composition', '## Workflow', '## Ownership', '## Source']) {
    if (!body.includes(heading)) throw new Error(`${pattern.slug}: missing ${heading}`)
  }
}

console.log(`pattern gallery: ${patterns.length}/35 pattern modules documented`)
