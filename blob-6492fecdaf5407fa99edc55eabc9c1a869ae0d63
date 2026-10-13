import { mkdir, readdir, readFile, rm } from 'node:fs/promises'
import { join } from 'node:path'

const root = join(import.meta.dir, '..', '..')
const docs = join(root, 'docs', 'content', 'docs')
const output = join(root, 'docs', 'public', 'preview-posters')

async function mdxFiles(directory: string): Promise<string[]> {
  const entries = await readdir(directory, { withFileTypes: true })
  const nested = await Promise.all(
    entries.map(async (entry) => {
      const path = join(directory, entry.name)
      if (entry.isDirectory()) return mdxFiles(path)
      return entry.isFile() && entry.name.endsWith('.mdx') ? [path] : []
    }),
  )
  return nested.flat()
}

const stories = new Set<string>()
for (const file of await mdxFiles(docs)) {
  const body = await readFile(file, 'utf8')
  for (const match of body.matchAll(/<TerminalPreview\s+[^>]*story=["']([^"']+)["']/g)) {
    if (match[1]) stories.add(match[1])
  }
}

if (stories.size === 0) throw new Error('no TerminalPreview stories found')

await rm(output, { recursive: true, force: true })
await mkdir(output, { recursive: true })
const args = [
  'cargo',
  'run',
  '-q',
  '-p',
  'termrock-lookbook',
  '--',
  'export-posters',
  '--out',
  output,
]
for (const story of [...stories].sort()) args.push('--story', story)

const result = Bun.spawnSync(args, { cwd: root, stdout: 'ignore', stderr: 'inherit' })
if (result.exitCode !== 0) throw new Error('poster export failed')

console.log(`exported ${stories.size} static preview posters`)
