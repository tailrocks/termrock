import { readFileSync, readdirSync } from 'node:fs'
import { join } from 'node:path'

const root = join(import.meta.dirname, '..', '..')
const process = Bun.spawnSync(['cargo', 'run', '-q', '-p', 'termrock-lookbook', '--', 'list', '--format', 'json'], { cwd: root })
if (process.exitCode !== 0) throw new Error(process.stderr.toString())
const stories = JSON.parse(process.stdout.toString()) as Array<{ id: string; component: string }>
const ids = new Set(stories.map((story) => story.id))
if (ids.size !== stories.length) throw new Error('duplicate story ID')

const docsDir = join(root, 'docs', 'content', 'docs')
const docs = readdirSync(docsDir).filter((name) => name.endsWith('.mdx')).map((name) => readFileSync(join(docsDir, name), 'utf8')).join('\n')
for (const story of stories) {
  if (!docs.includes(`\`${story.id}\``)) throw new Error(`missing docs for story ${story.id}`)
  const preview = join(root, 'docs', 'public', 'component-previews', `${story.id.replaceAll('/', '-')}.svg`)
  if (!Bun.file(preview).size) throw new Error(`missing preview ${preview}`)
}
console.log(`catalog covers ${stories.length} stories`)
