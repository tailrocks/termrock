import { mkdir, mkdtemp, rm } from 'node:fs/promises'
import { tmpdir } from 'node:os'
import { join } from 'node:path'
import { loadCatalog } from './catalog-data'

const root = join(import.meta.dir, '..', '..')
const value: unknown = await Bun.file(join(root, 'docs', 'public', 'demo-code.json')).json()
if (typeof value !== 'object' || value === null || Array.isArray(value)) {
  throw new Error('demo-code.json must be an object')
}
const snippets = new Map<string, string>()
for (const [story, snippet] of Object.entries(value)) {
  if (typeof snippet !== 'string' || !snippet.trim()) throw new Error(`${story}: empty demo snippet`)
  snippets.set(story, snippet)
}

const catalog = await loadCatalog()
const canonicalStories = new Set(catalog.stories.map((story) => story.id))
const missing = [...canonicalStories].filter((story) => !snippets.has(story)).sort()
const extra = [...snippets.keys()].filter((story) => !canonicalStories.has(story)).sort()
if (missing.length || extra.length) {
  throw new Error(`demo snippet set drift: missing [${missing.join(', ')}]; extra [${extra.join(', ')}]`)
}
for (const entry of [...catalog.components, ...catalog.patterns]) {
  if (!snippets.has(entry.story)) throw new Error(`${entry.id}: missing representative demo code`)
  const directory = entry.entryKind === 'component' ? 'components' : 'patterns'
  const page = join(root, 'docs', 'content', 'docs', directory, `${entry.slug}.mdx`)
  const body = await Bun.file(page).text()
  if (/^```(?:rust|rs)(?:[\s,].*)?$/mu.test(body)) {
    throw new Error(`${entry.id}: authored Rust fence duplicates canonical compiled demo-code`)
  }
}

function moduleName(story: string, index: number): string {
  const slug = story.replaceAll(/[^A-Za-z0-9]+/gu, '_').replaceAll(/^_+|_+$/gu, '').toLowerCase()
  return `snippet_${index}_${slug}`
}

function indent(source: string): string {
  return source.split('\n').map((line) => `        ${line}`).join('\n')
}

const harness = [
  '#![allow(dead_code, unreachable_code, unused_imports, unused_mut, unused_variables)]',
  '',
  ...[...snippets.entries()].sort(([left], [right]) => left.localeCompare(right)).flatMap(
    ([story, snippet], index) => [
      `// ${story}`,
      `mod ${moduleName(story, index)} {`,
      '    pub fn compile() {',
      indent(snippet),
      '    }',
      '}',
      '',
    ],
  ),
  'fn main() {}',
  '',
].join('\n')

const temporary = await mkdtemp(join(tmpdir(), 'termrock-doc-snippets-'))
try {
  await mkdir(join(temporary, 'src'))
  const manifest = [
    '[package]',
    'name = "termrock-doc-snippet-check"',
    'version = "0.0.0"',
    'edition = "2024"',
    'publish = false',
    '',
    '[workspace]',
    '',
    '[dependencies]',
    `termrock = { path = ${JSON.stringify(join(root, 'crates', 'termrock'))} }`,
    `termrock-catalog = { path = ${JSON.stringify(join(root, 'crates', 'termrock-catalog'))}, default-features = false }`,
    '',
  ].join('\n')
  await Bun.write(join(temporary, 'Cargo.toml'), manifest)
  await Bun.write(join(temporary, 'src', 'main.rs'), harness)
  const compiled = Bun.spawnSync(
    ['cargo', 'check', '--quiet', '--offline'],
    {
      cwd: temporary,
      env: {
        ...process.env,
        CARGO_TARGET_DIR: join(root, 'target', 'docs-snippet-check'),
      },
      stdout: 'inherit',
      stderr: 'inherit',
    },
  )
  if (compiled.exitCode !== 0) throw new Error('generated demo-code Rust harness failed')
} finally {
  await rm(temporary, { recursive: true, force: true })
}

console.log(
  `snippets: ${snippets.size} compiled canonical demos; no duplicate authored Rust; ${catalog.components.length} components; ${catalog.patterns.length} patterns`,
)
