import { join } from 'node:path'
import { catalogEntries } from '../src/generated/catalog'

const root = join(import.meta.dir, '..', '..')
const outputPath = join(root, 'docs', 'api', 'component-render-evidence.json')
const manifestPath = join(root, 'docs', 'contract-audit', 'Cargo.toml')

const booleanKeys = [
  'canonicalNonempty',
  'responsive',
  'tinyTerminal',
  'resize',
  'panicSafety',
  'colorLadder',
  'noColor',
  'asciiFallback',
] as const
const characterKeys = ['unicode', 'cjk', 'combining', 'emoji'] as const
const interactionKeys = ['keyboard', 'mouse', 'focus', 'escape'] as const
const stateKeys = ['disabled', 'loading', 'empty', 'error', 'streaming', 'largeData', 'overlay'] as const

type BooleanKey = (typeof booleanKeys)[number]
type CharacterKey = (typeof characterKeys)[number]
type InteractionKey = (typeof interactionKeys)[number]
type StateKey = (typeof stateKeys)[number]
type AuditEntry = Readonly<{
  id: string
  story: string
  source: string
  renderKind: string | null
  family: string
  interactive: boolean
  interactionKind: string
  checks: Readonly<Record<BooleanKey | InteractionKey, boolean>>
  characterStories: Readonly<Record<CharacterKey, readonly string[]>>
  stateStories: Readonly<Record<StateKey, readonly string[]>>
}>

function parseBoolean(value: string | undefined, label: string): boolean {
  if (value === 'true') return true
  if (value === 'false') return false
  throw new Error(`${label}: expected true or false, found ${value ?? 'missing'}`)
}

function parseStories(value: string | undefined, label: string): readonly string[] {
  if (value === undefined) throw new Error(`${label}: missing`)
  if (value === '-') return []
  const stories = value.split(',')
  if (stories.some((story) => !story || !story.includes('/'))) {
    throw new Error(`${label}: invalid story list ${value}`)
  }
  return stories
}

function parseLine(line: string, index: number): Readonly<{
  story: string
  checks: Readonly<Record<BooleanKey | InteractionKey, boolean>>
  characterStories: Readonly<Record<CharacterKey, readonly string[]>>
  stateStories: Readonly<Record<StateKey, readonly string[]>>
}> {
  const columns = line.split('\t')
  if (columns.length !== 24) {
    throw new Error(`render audit row ${index + 1}: expected 24 columns, found ${columns.length}`)
  }
  const story = columns[0]
  if (!story) throw new Error(`render audit row ${index + 1}: missing story`)
  const checks = Object.fromEntries([
    ...booleanKeys.map((key, offset) => [key, parseBoolean(columns[offset + 1], `${story}.${key}`)]),
    ...interactionKeys.map((key, offset) => [key, parseBoolean(columns[offset + 13], `${story}.${key}`)]),
  ]) as Record<BooleanKey | InteractionKey, boolean>
  const characterStories = Object.fromEntries(
    characterKeys.map((key, offset) => [key, parseStories(columns[offset + 9], `${story}.${key}`)]),
  ) as Record<CharacterKey, readonly string[]>
  const stateStories = Object.fromEntries(
    stateKeys.map((key, offset) => [key, parseStories(columns[offset + 17], `${story}.${key}`)]),
  ) as Record<StateKey, readonly string[]>
  return { story, checks, characterStories, stateStories }
}

export function runRenderAudit(): readonly AuditEntry[] {
  const stories = catalogEntries.map((entry) => entry.story)
  if (new Set(stories).size !== stories.length) {
    throw new Error('render audit requires one exact representative story per contract')
  }
  const result = Bun.spawnSync([
    'cargo',
    'run',
    '--quiet',
    '--manifest-path',
    manifestPath,
    '--',
    ...stories,
  ], {
    cwd: root,
    env: { ...process.env, CARGO_TARGET_DIR: join(root, 'target') },
    stdout: 'pipe',
    stderr: 'pipe',
  })
  if (result.exitCode !== 0) {
    throw new Error(`contract render audit failed:\n${result.stderr.toString()}`)
  }
  const rows = result.stdout.toString().trim().split('\n').filter(Boolean).map(parseLine)
  const rowByStory = new Map(rows.map((row) => [row.story, row]))
  if (rows.length !== catalogEntries.length || rowByStory.size !== catalogEntries.length) {
    throw new Error(`render audit expected ${catalogEntries.length} unique rows, found ${rowByStory.size}`)
  }
  return catalogEntries.map((entry) => {
    const row = rowByStory.get(entry.story)
    if (!row) throw new Error(`${entry.id}: render audit omitted ${entry.story}`)
    return {
      id: entry.id,
      story: entry.story,
      source: entry.source,
      renderKind: entry.renderKind,
      family: entry.family,
      interactive: entry.interactive,
      interactionKind: entry.interactionKind,
      checks: row.checks,
      characterStories: row.characterStories,
      stateStories: row.stateStories,
    }
  })
}

const source = `${JSON.stringify({ schema: 1, entries: runRenderAudit() }, null, 2)}\n`
if (process.argv.includes('--check')) {
  const current = await Bun.file(outputPath).text().catch(() => '')
  if (current !== source) {
    throw new Error('component-render-evidence.json is stale; run bun run generate:contract-render-evidence')
  }
} else {
  await Bun.write(outputPath, source)
}
console.log(`render evidence: ${catalogEntries.length} exact representative stories`)
