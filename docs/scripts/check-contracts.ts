/**
 * TermRock component quality contract validator (v2 + static design lints).
 *
 * - Always validates docs/api/component-contracts.v2.example.json structurally.
 * - If docs/api/component-contracts.v2.json exists, validates full catalog rules.
 * - Runs a lightweight hardcoded-KeyCode scan under widgets/ (warn by default;
 *   set TERMrock_LINT_KEYS=error to fail).
 *
 * Usage: bun run docs/scripts/check-contracts.ts
 */

const root = `${import.meta.dir}/../..`

const REQUIRED_AXES = [
  'visual_states',
  'keyboard',
  'mouse',
  'focus',
  'disabled',
  'loading',
  'error',
  'empty',
  'overlay',
  'escape',
  'responsive',
  'tiny_terminal',
  'unicode',
  'cjk',
  'combining',
  'emoji',
  'ascii_fallback',
  'no_color',
  'color_ladder',
  'streaming',
  'large_data',
  'resize',
  'panic_safety',
] as const

const STATUS = new Set([
  'covered',
  'partial',
  'caller_owned',
  'not_applicable',
  'missing',
])

const LINT_STATUS = new Set(['pass', 'fail', 'waived', 'not_run'])

type Evidence = {
  stories?: string[]
  tests?: string[]
  snapshots?: string[]
  recordings?: string[]
  benches?: string[]
  notes?: string
}

type AxisCell = {
  status: string
  evidence?: Evidence
  reason?: string
  waiver?: { ticket?: string; expires?: string }
}

type V2Component = {
  schema: 2
  component: string
  module?: string
  kind: string
  complete: boolean
  axes: Record<string, AxisCell>
  lints?: Record<string, string>
}

type V2Document = {
  schema: 2
  version?: string
  components: V2Component[]
}

const errors: string[] = []
const warnings: string[] = []

function fail(msg: string) {
  errors.push(msg)
}

function warn(msg: string) {
  warnings.push(msg)
}

function evidenceNonEmpty(ev?: Evidence): boolean {
  if (!ev) return false
  return Boolean(
    (ev.stories && ev.stories.length) ||
      (ev.tests && ev.tests.length) ||
      (ev.snapshots && ev.snapshots.length) ||
      (ev.recordings && ev.recordings.length) ||
      (ev.benches && ev.benches.length),
  )
}

async function loadStoryIds(): Promise<Set<string>> {
  const result = Bun.spawnSync(
    ['cargo', 'run', '-q', '-p', 'termrock-lookbook', '--', 'list', '--format', 'json'],
    { cwd: root },
  )
  if (result.exitCode !== 0) {
    warn(
      `lookbook list failed (story evidence checks skipped): ${result.stderr.toString().slice(0, 200)}`,
    )
    return new Set()
  }
  try {
    const stories = JSON.parse(result.stdout.toString()) as Array<{ id: string }>
    return new Set(stories.map((s) => s.id))
  } catch {
    warn('lookbook list produced invalid JSON; story evidence checks skipped')
    return new Set()
  }
}

async function fileExists(rel: string): Promise<boolean> {
  return Bun.file(`${root}/${rel}`).exists()
}

function validateComponent(
  c: V2Component,
  storyIds: Set<string>,
  opts: { requireAllAxes: boolean },
) {
  if (c.schema !== 2) fail(`${c.component}: schema must be 2`)
  if (!/^[A-Z][A-Za-z0-9_]*$/.test(c.component)) {
    fail(`${c.component}: invalid component name`)
  }
  if (!['interactive', 'static', 'overlay', 'container', 'pattern'].includes(c.kind)) {
    fail(`${c.component}: invalid kind ${c.kind}`)
  }
  if (!c.axes || typeof c.axes !== 'object') {
    fail(`${c.component}: missing axes`)
    return
  }

  for (const axis of REQUIRED_AXES) {
    const cell = c.axes[axis]
    if (!cell) {
      if (opts.requireAllAxes) fail(`${c.component}: missing axis ${axis}`)
      continue
    }
    if (!STATUS.has(cell.status)) {
      fail(`${c.component}.${axis}: invalid status ${cell.status}`)
      continue
    }
    if (
      cell.status === 'not_applicable' ||
      cell.status === 'caller_owned' ||
      cell.status === 'missing' ||
      cell.status === 'partial'
    ) {
      if (!cell.reason || !cell.reason.trim()) {
        fail(`${c.component}.${axis}: status ${cell.status} requires reason`)
      }
    }
    if (cell.status === 'covered') {
      if (!evidenceNonEmpty(cell.evidence)) {
        fail(`${c.component}.${axis}: covered requires evidence (stories|tests|snapshots|…)`)
      }
    }
    if (cell.status === 'covered' || cell.status === 'partial') {
      const stories = cell.evidence?.stories ?? []
      for (const id of stories) {
        if (storyIds.size > 0 && !storyIds.has(id)) {
          fail(`${c.component}.${axis}: unknown story id "${id}"`)
        }
      }
      const snaps = cell.evidence?.snapshots ?? []
      for (const path of snaps) {
        // checked async below via queue
        void path
      }
    }
  }

  if (c.complete) {
    for (const axis of REQUIRED_AXES) {
      const cell = c.axes[axis]
      if (!cell) continue
      if (cell.status === 'missing') {
        fail(`${c.component}: complete=true but axis ${axis} is missing`)
      }
      if (cell.status === 'partial' && !cell.waiver?.ticket) {
        fail(`${c.component}: complete=true but axis ${axis} is partial without waiver.ticket`)
      }
    }
    if (c.lints) {
      for (const [lint, status] of Object.entries(c.lints)) {
        if (status === 'fail') {
          fail(`${c.component}: complete=true but lint ${lint} is fail`)
        }
      }
    }
  }

  if (c.lints) {
    for (const [lint, status] of Object.entries(c.lints)) {
      if (!LINT_STATUS.has(status)) {
        fail(`${c.component}: lint ${lint} has invalid status ${status}`)
      }
    }
  }
}

async function validateSnapshots(c: V2Component) {
  for (const axis of Object.values(c.axes)) {
    for (const path of axis.evidence?.snapshots ?? []) {
      if (!(await fileExists(path))) {
        fail(`${c.component}: missing snapshot file ${path}`)
      }
    }
  }
}

async function validateV2Document(
  path: string,
  storyIds: Set<string>,
  opts: { requireAllAxes: boolean; label: string },
) {
  const text = await Bun.file(path).text()
  let doc: V2Document
  try {
    doc = JSON.parse(text) as V2Document
  } catch (e) {
    fail(`${opts.label}: invalid JSON: ${e}`)
    return
  }
  if (doc.schema !== 2) fail(`${opts.label}: schema must be 2`)
  if (!Array.isArray(doc.components) || doc.components.length === 0) {
    fail(`${opts.label}: components must be a non-empty array`)
    return
  }
  const names = new Set<string>()
  for (const c of doc.components) {
    if (names.has(c.component)) fail(`${opts.label}: duplicate component ${c.component}`)
    names.add(c.component)
    validateComponent(c, storyIds, opts)
    await validateSnapshots(c)
  }
}

/** Static lint: raw KeyCode product chords in widgets (heuristic). */
async function lintHardcodedKeys() {
  const widgetsDir = `${root}/crates/termrock/src/widgets`
  const allowName = /(keymap|key_map|input|event|binding)/i
  const re =
    /KeyCode::Char\(['"][a-zA-Z]['"]\)|KeyCode::(F\d+|Esc|Enter|Tab)/g
  const glob = new Bun.Glob('**/*.rs')
  let hits = 0
  for await (const name of glob.scan({ cwd: widgetsDir })) {
    if (allowName.test(name)) continue
    // Interaction hosts are expected to match keys; still count for visibility.
    const text = await Bun.file(`${widgetsDir}/${name}`).text()
    // Skip pure re-exports / tiny glue
    if (!text.includes('KeyCode::')) continue
    const matches = text.match(re)
    if (matches && matches.length > 8) {
      // Heuristic: dense KeyCode matching may be local keymap debt
      hits += 1
      warn(
        `hardcoded_key_handling: ${name} has dense KeyCode matches (${matches.length}); prefer Keymap/UiIntent where product chords live`,
      )
    }
  }
  if (hits === 0) {
    // no dense files — ok
  }
  if (process.env.TERMROCK_LINT_KEYS === 'error' && warnings.some((w) => w.includes('hardcoded_key'))) {
    for (const w of warnings.filter((x) => x.includes('hardcoded_key'))) {
      fail(w)
    }
  }
}

async function main() {
  // Schema file present
  if (!(await fileExists('docs/api/component-contract.schema.json'))) {
    fail('missing docs/api/component-contract.schema.json')
  }

  const storyIds = await loadStoryIds()

  const example = `${root}/docs/api/component-contracts.v2.example.json`
  if (!(await Bun.file(example).exists())) {
    fail('missing docs/api/component-contracts.v2.example.json')
  } else {
    await validateV2Document(example, storyIds, {
      requireAllAxes: true,
      label: 'component-contracts.v2.example.json',
    })
  }

  const catalog = `${root}/docs/api/component-contracts.v2.json`
  if (await Bun.file(catalog).exists()) {
    await validateV2Document(catalog, storyIds, {
      requireAllAxes: true,
      label: 'component-contracts.v2.json',
    })
  } else {
    warn('docs/api/component-contracts.v2.json not present yet (v1 catalog still authoritative)')
  }

  await lintHardcodedKeys()

  for (const w of warnings) {
    console.warn(`warn: ${w}`)
  }
  if (errors.length) {
    for (const e of errors) console.error(`error: ${e}`)
    console.error(`check-contracts: ${errors.length} error(s), ${warnings.length} warning(s)`)
    process.exit(1)
  }
  console.log(
    `check-contracts: ok (${warnings.length} warning(s); v2 example validated${
      (await Bun.file(catalog).exists()) ? '; v2 catalog validated' : ''
    })`,
  )
}

await main()
