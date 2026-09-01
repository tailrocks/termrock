import { join } from 'node:path'
import {
  contractSchemaSource,
  hardcodedKeyMatches,
  loadCatalog,
  type HardcodedKeyMatch,
} from './catalog-data'

const root = join(import.meta.dir, '..', '..')
const publishedSchema = await Bun.file(join(root, 'docs', 'api', 'component-contract.schema.json'))
  .text()
  .catch(() => '')
if (publishedSchema !== contractSchemaSource()) {
  throw new Error('component-contract.schema.json is stale; run bun run generate:catalog')
}

const catalog = await loadCatalog()
const contractById = new Map(catalog.contracts.map((contract) => [contract.id, contract]))
const entryById = new Map([...catalog.components, ...catalog.patterns].map((entry) => [entry.id, entry]))
const keyOwners: Array<Readonly<{
  id: string
  source: string
  matches: readonly HardcodedKeyMatch[]
}>> = []
const sourceMatches = new Map<string, readonly HardcodedKeyMatch[]>()

for (const entry of [...catalog.components, ...catalog.patterns]) {
  let matches = sourceMatches.get(entry.source)
  if (!matches) {
    matches = hardcodedKeyMatches(await Bun.file(join(root, entry.source)).text())
    sourceMatches.set(entry.source, matches)
  }
  const expected = matches.length ? 'fail' : 'pass'
  const actual = contractById.get(entry.id)?.lints.hardcoded_key_handling
  if (actual !== expected) {
    throw new Error(
      `${entry.id}: hardcoded_key_handling is ${actual ?? 'missing'}; exact canonical source requires ${expected}`,
    )
  }
  if (matches.length) keyOwners.push({ id: entry.id, source: entry.source, matches })
}

const incomplete = catalog.contracts
  .filter((contract) => !contract.complete)
  .map((contract) => ({
    id: contract.id,
    source: entryById.get(contract.id)?.source ?? 'missing-source',
    axes: Object.entries(contract.axes)
      .filter(([, cell]) => cell.status === 'missing')
      .map(([axis]) => axis),
    lints: Object.entries(contract.lints)
      .filter(([, status]) => status === 'fail' || status === 'not_run')
      .map(([lint]) => lint),
  }))

const missingAxes = incomplete.flatMap((entry) => entry.axes)
const genericRenderAxes = new Set([
  'responsive', 'tiny_terminal', 'ascii_fallback', 'no_color', 'color_ladder', 'resize', 'panic_safety',
])
const behaviorAxes = new Set(['keyboard', 'mouse', 'focus', 'disabled', 'overlay', 'escape'])
const characterAxes = new Set(['unicode', 'cjk', 'combining', 'emoji'])
const missingClass = {
  genericRender: missingAxes.filter((axis) => genericRenderAxes.has(axis)).length,
  behavior: missingAxes.filter((axis) => behaviorAxes.has(axis)).length,
  characterProfile: missingAxes.filter((axis) => characterAxes.has(axis)).length,
  stateData: missingAxes.filter((axis) => !genericRenderAxes.has(axis) && !behaviorAxes.has(axis) && !characterAxes.has(axis)).length,
}

if (process.argv.includes('--report')) {
  const keySources = new Map<string, typeof keyOwners>()
  for (const owner of keyOwners) {
    const group = keySources.get(owner.source) ?? []
    group.push(owner)
    keySources.set(owner.source, group)
  }
  for (const [source, owners] of [...keySources].sort(([left], [right]) => left.localeCompare(right))) {
    const ids = owners.map((owner) => owner.id).sort()
    const matches = owners[0]?.matches ?? []
    const locations = matches.map((match) => `${match.line}:${match.literal}`).join(',')
    console.log(`hardcoded-key-owner ${source} ids=[${ids.join(',')}] matches=[${locations}]`)
  }
  const gapSources = new Map<string, typeof incomplete>()
  for (const entry of incomplete.filter((item) => item.axes.length || item.lints.length)) {
    const group = gapSources.get(entry.source) ?? []
    group.push(entry)
    gapSources.set(entry.source, group)
  }
  for (const [source, entries] of [...gapSources].sort(([left], [right]) => left.localeCompare(right))) {
    const axisCounts = new Map<string, number>()
    const lintCounts = new Map<string, number>()
    for (const entry of entries) {
      for (const axis of entry.axes) axisCounts.set(axis, (axisCounts.get(axis) ?? 0) + 1)
      for (const lint of entry.lints) lintCounts.set(lint, (lintCounts.get(lint) ?? 0) + 1)
    }
    const counts = (items: ReadonlyMap<string, number>): string => [...items]
      .sort(([left], [right]) => left.localeCompare(right))
      .map(([name, count]) => `${name}:${count}`)
      .join(',')
    console.log(
      `gap-owner ${source} ids=[${entries.map((entry) => entry.id).sort().join(',')}] `
      + `axes=[${counts(axisCounts)}] lints=[${counts(lintCounts)}]`,
    )
  }
}

const complete = catalog.contracts.filter((contract) => contract.complete).length
console.log(
  `contracts: ${catalog.contracts.length} canonical entries; ${complete} complete; `
  + `${incomplete.length} incomplete; ${missingAxes.length} required missing `
  + `(generic-render ${missingClass.genericRender}, behavior ${missingClass.behavior}, `
  + `character-profile ${missingClass.characterProfile}, state/data ${missingClass.stateData}); `
  + `${keyOwners.length} source-key owner(s)`,
)
