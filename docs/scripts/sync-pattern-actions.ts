import { join } from 'node:path'

type Pattern = { slug: string; demo: string; actions: string[] }
type Demo = { id: string; hints: string[] }

const root = join(import.meta.dir, '..', '..')
const manifestPath = join(root, 'docs', 'api', 'pattern-catalog.json')
const patterns = JSON.parse(await Bun.file(manifestPath).text()) as Pattern[]
const result = Bun.spawnSync(
  ['cargo', 'run', '-q', '-p', 'termrock-lookbook', '--', 'list', '--format', 'json'],
  { cwd: root },
)
if (result.exitCode !== 0) throw new Error(result.stderr.toString())
const demos = new Map(
  (JSON.parse(result.stdout.toString()) as Demo[]).map((demo) => [demo.id, demo]),
)

function instruction(hint: string): string {
  const lower = hint.toLowerCase()
  if (/type|paste|edit|filter|search/.test(lower)) return 'focus the real field and edit its persistent Unicode-safe draft'
  if (/drag|resize/.test(lower)) return 'change the same mounted layout/value; no alternate screenshot is mounted'
  if (/wheel|scroll|page/.test(lower)) return 'move the pattern-owned viewport without scrolling the documentation page'
  if (/esc|close|cancel|dismiss/.test(lower)) return 'peel or cancel the current transient layer and preserve underlying state'
  if (/enter|space|click|activate|submit|open|approve|deny|stop|retry|clear|run/.test(lower)) return 'invoke the focused public state-machine action and inspect its typed request'
  if (/arrow|select|focus|tab|next|previous|pane|tree/.test(lower)) return 'move visible focus, selection, pane, or hierarchy state for the next action'
  return 'forward this normalized action to the mounted public pattern state'
}

for (const pattern of patterns) {
  const demo = demos.get(pattern.demo)
  if (!demo) throw new Error(`${pattern.slug}: missing demo ${pattern.demo}`)
  pattern.actions = demo.hints
  const path = join(root, 'docs', 'content', 'docs', 'patterns', `${pattern.slug}.mdx`)
  const body = await Bun.file(path).text()
  const start = body.indexOf('## Workflow\n')
  const end = body.indexOf('\n## Ownership', start)
  if (start < 0 || end < 0) throw new Error(`${pattern.slug}: malformed Workflow section`)
  const actions = demo.hints.map((hint) => `- \`${hint}\`: ${instruction(hint)}.`).join('\n')
  const replacement = `## Workflow\n\n${actions}\n\nAccepted actions update the same persistent pattern instance in web docs and native Lookbook. Typed requests appear in the status line; the fixtures never authenticate, connect, execute, persist, or mutate external state.\n`
  await Bun.write(path, body.slice(0, start) + replacement + body.slice(end))
}

await Bun.write(manifestPath, `${JSON.stringify(patterns, null, 2)}\n`)
console.log(`synchronized ${patterns.length} pattern action contracts from the Rust catalog`)
