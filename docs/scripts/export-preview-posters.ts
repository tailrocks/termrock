import { mkdir, mkdtemp, readdir, rename, rm } from 'node:fs/promises'
import { dirname, join } from 'node:path'
import { loadCatalog } from './catalog-data'

type PosterFrame = Readonly<{
  storyId: string
  component: string
  cols: number
  rows: number
  storyCols: number
  storyRows: number
  cells: readonly unknown[]
  value: unknown
}>

const root = join(import.meta.dir, '..', '..')
const output = join(root, 'docs', 'public', 'preview-posters')
const catalog = await loadCatalog()
const entries = [...catalog.components, ...catalog.patterns]
const expectedByStory = new Map(entries.map((entry) => [entry.story, entry]))
const stories = [...expectedByStory.keys()].sort()
const filenames = stories.map((story) => `${story.replaceAll('/', '-')}.json`)
if (stories.length !== entries.length) throw new Error('catalog representative stories must be unique')
if (new Set(filenames).size !== filenames.length) throw new Error('poster filename collision')

function object(value: unknown, label: string): Record<string, unknown> {
  if (typeof value !== 'object' || value === null || Array.isArray(value)) {
    throw new Error(`${label}: expected object`)
  }
  return value as Record<string, unknown>
}

function text(value: unknown, label: string): string {
  if (typeof value !== 'string' || !value) throw new Error(`${label}: expected string`)
  return value
}

function dimension(value: unknown, label: string): number {
  if (typeof value !== 'number' || !Number.isSafeInteger(value) || value <= 0) {
    throw new Error(`${label}: expected positive integer`)
  }
  return value
}

function posterFrame(value: unknown, label: string): PosterFrame {
  const item = object(value, label)
  const cols = dimension(item['cols'], `${label}.cols`)
  const rows = dimension(item['rows'], `${label}.rows`)
  const cells = item['cells']
  if (!Array.isArray(cells) || cells.length !== cols * rows) {
    throw new Error(`${label}.cells: expected full ${cols}×${rows} frame`)
  }
  return {
    storyId: text(item['story_id'], `${label}.story_id`),
    component: text(item['component'], `${label}.component`),
    cols,
    rows,
    storyCols: dimension(item['story_cols'], `${label}.story_cols`),
    storyRows: dimension(item['story_rows'], `${label}.story_rows`),
    cells,
    value,
  }
}

async function exactFiles(directory: string): Promise<void> {
  const actual = (await readdir(directory)).filter((name) => name.endsWith('.json')).sort()
  const missing = filenames.filter((name) => !actual.includes(name))
  const extra = actual.filter((name) => !filenames.includes(name))
  if (missing.length || extra.length) {
    throw new Error(`poster set drift in ${directory}: missing [${missing.join(', ')}]; extra [${extra.join(', ')}]`)
  }
}

async function validate(directory: string): Promise<void> {
  await exactFiles(directory)
  for (const story of stories) {
    const filename = `${story.replaceAll('/', '-')}.json`
    const frame = posterFrame(await Bun.file(join(directory, filename)).json(), filename)
    const entry = expectedByStory.get(story)
    if (!entry) throw new Error(`${story}: missing catalog entry`)
    if (frame.storyId !== story || frame.component !== entry.storyComponent) {
      throw new Error(`${filename}: poster identity differs from canonical story`)
    }
    if (frame.storyCols !== entry.dimensions.cols || frame.storyRows !== entry.dimensions.rows) {
      throw new Error(`${filename}: story dimensions differ from canonical story`)
    }
  }
}

async function exportStories(directory: string): Promise<void> {
  await mkdir(directory, { recursive: true })
  const args = [
    'cargo',
    'run',
    '-q',
    '-p',
    'termrock-lookbook',
    '--',
    'export-posters',
    '--out',
    directory,
  ]
  for (const story of stories) args.push('--story', story)
  const result = Bun.spawnSync(args, { cwd: root, stdout: 'ignore', stderr: 'pipe' })
  if (result.exitCode !== 0) throw new Error(`poster export failed:\n${result.stderr.toString()}`)
  await validate(directory)
}

async function compare(rendered: string): Promise<void> {
  await validate(output)
  for (const story of stories) {
    const filename = `${story.replaceAll('/', '-')}.json`
    const expected = posterFrame(await Bun.file(join(rendered, filename)).json(), `${filename} rendered`)
    const actual = posterFrame(await Bun.file(join(output, filename)).json(), `${filename} checked-in`)
    if (JSON.stringify(actual.value) !== JSON.stringify(expected.value)) {
      throw new Error(`${filename}: checked-in full frame, dimensions, or cells differ from canonical render`)
    }
  }
}

const temporary = await mkdtemp(join(dirname(output), '.preview-posters-'))
try {
  await exportStories(temporary)
  if (process.argv.includes('--check')) {
    await compare(temporary)
    console.log(`posters: ${stories.length} exact canonical frames`)
  } else {
    await rm(output, { recursive: true, force: true })
    await rename(temporary, output)
    console.log(`exported ${stories.length} exact canonical frames`)
  }
} finally {
  await rm(temporary, { recursive: true, force: true })
}
