import { join } from 'node:path'
import { catalogEntries } from '../src/generated/catalog'
import {
  REQUIRED_AXES,
  REQUIRED_LINTS,
  hardcodedKeyMatches,
  type AxisName,
} from './catalog-data'
import { ownerEvidence, type OwnerAxisEvidence } from './contract-owner-evidence'

const root = join(import.meta.dir, '..', '..')
const contractPath = join(root, 'docs', 'api', 'component-contracts.v2.json')
const renderEvidencePath = join(root, 'docs', 'api', 'component-render-evidence.json')
const exhaustiveInventoryTest =
  'crates/termrock-lookbook/src/demo.rs#typed_inventory_joins_every_representative_story'
const posterCheck = 'docs/scripts/export-preview-posters.ts#validate'
const renderAuditCheck = 'docs/scripts/audit-render-contracts.ts#runRenderAudit'

const universalRenderAxes = new Set<AxisName>([
  'visual_states',
  'responsive',
  'tiny_terminal',
  'ascii_fallback',
  'no_color',
  'color_ladder',
  'resize',
  'panic_safety',
])
const interactionAxes = new Set<AxisName>(['keyboard', 'mouse', 'focus', 'escape'])
const characterAxes = new Set<AxisName>(['unicode', 'cjk', 'combining', 'emoji'])
const stateAxisKey = {
  disabled: 'disabled',
  loading: 'loading',
  error: 'error',
  empty: 'empty',
  overlay: 'overlay',
  streaming: 'streaming',
  large_data: 'largeData',
} as const satisfies Partial<Record<AxisName, string>>
const checkKey = {
  responsive: 'responsive',
  tiny_terminal: 'tinyTerminal',
  ascii_fallback: 'asciiFallback',
  no_color: 'noColor',
  color_ladder: 'colorLadder',
  resize: 'resize',
  panic_safety: 'panicSafety',
  keyboard: 'keyboard',
  mouse: 'mouse',
  focus: 'focus',
  escape: 'escape',
} as const satisfies Partial<Record<AxisName, string>>

type CanonicalEntry = (typeof catalogEntries)[number]
type RenderCheck = 'canonicalNonempty' | 'responsive' | 'tinyTerminal' | 'resize'
  | 'panicSafety' | 'colorLadder' | 'noColor' | 'asciiFallback'
  | 'keyboard' | 'mouse' | 'focus' | 'escape'
type CharacterKey = 'unicode' | 'cjk' | 'combining' | 'emoji'
type StateKey = 'disabled' | 'loading' | 'empty' | 'error' | 'streaming' | 'largeData' | 'overlay'
type RenderEvidence = Readonly<{
  id: string
  story: string
  source: string
  renderKind: string | null
  family: string
  interactive: boolean
  interactionKind: string
  checks: Readonly<Record<RenderCheck, boolean>>
  characterStories: Readonly<Record<CharacterKey, readonly string[]>>
  stateStories: Readonly<Record<StateKey, readonly string[]>>
}>

const ownerEvidenceById = new Map<string, Readonly<Partial<Record<AxisName, OwnerAxisEvidence>>>>()
for (const entry of ownerEvidence) {
  const current = ownerEvidenceById.get(entry.id) ?? {}
  for (const axis of Object.keys(entry.axes) as AxisName[]) {
    if (current[axis]) throw new Error(`${entry.id}.${axis}: duplicate owner evidence`)
    const claim = entry.axes[axis]
    if (!claim) throw new Error(`${entry.id}.${axis}: empty owner evidence`)
    if ((claim.status === 'covered' || claim.status === 'partial') && claim.tests.length === 0) {
      throw new Error(`${entry.id}.${axis}: ${claim.status} owner evidence requires an exact test`)
    }
    if (
      claim.applicability === 'required'
      && (claim.status === 'caller_owned' || claim.status === 'not_applicable')
    ) {
      throw new Error(`${entry.id}.${axis}: required owner evidence cannot be ${claim.status}`)
    }
  }
  ownerEvidenceById.set(entry.id, { ...current, ...entry.axes })
}

function object(value: unknown, label: string): Record<string, unknown> {
  if (typeof value !== 'object' || value === null || Array.isArray(value)) {
    throw new Error(`${label}: expected object`)
  }
  return value as Record<string, unknown>
}

function text(value: unknown, label: string): string {
  if (typeof value !== 'string' || !value.trim()) throw new Error(`${label}: expected string`)
  return value
}

function textArray(value: unknown, label: string): readonly string[] {
  if (!Array.isArray(value)) throw new Error(`${label}: expected array`)
  return value.map((item, index) => text(item, `${label}[${index}]`))
}

function boolean(value: unknown, label: string): boolean {
  if (typeof value !== 'boolean') throw new Error(`${label}: expected boolean`)
  return value
}

function nullableText(value: unknown, label: string): string | null {
  if (value === null) return null
  return text(value, label)
}

function exactKeys(
  value: Record<string, unknown>,
  expected: readonly string[],
  label: string,
): void {
  const actual = Object.keys(value).sort()
  const wanted = [...expected].sort()
  if (actual.join('\0') !== wanted.join('\0')) {
    throw new Error(`${label}: expected keys [${wanted.join(', ')}], found [${actual.join(', ')}]`)
  }
}

function booleanRecord<K extends string>(
  value: unknown,
  keys: readonly K[],
  label: string,
): Readonly<Record<K, boolean>> {
  const item = object(value, label)
  exactKeys(item, keys, label)
  return Object.fromEntries(keys.map((key) => [key, boolean(item[key], `${label}.${key}`)])) as Record<K, boolean>
}

function storyRecord<K extends string>(
  value: unknown,
  keys: readonly K[],
  label: string,
): Readonly<Record<K, readonly string[]>> {
  const item = object(value, label)
  exactKeys(item, keys, label)
  return Object.fromEntries(keys.map((key) => [key, textArray(item[key], `${label}.${key}`)])) as Record<K, readonly string[]>
}

async function loadRenderEvidence(): Promise<ReadonlyMap<string, RenderEvidence>> {
  const document = object(await Bun.file(renderEvidencePath).json() as unknown, 'render evidence')
  if (document['schema'] !== 1 || !Array.isArray(document['entries'])) {
    throw new Error('render evidence: expected schema 1 entries')
  }
  const checkKeys: readonly RenderCheck[] = [
    'canonicalNonempty', 'responsive', 'tinyTerminal', 'resize', 'panicSafety',
    'colorLadder', 'noColor', 'asciiFallback', 'keyboard', 'mouse', 'focus', 'escape',
  ]
  const characterKeys: readonly CharacterKey[] = ['unicode', 'cjk', 'combining', 'emoji']
  const stateKeys: readonly StateKey[] = ['disabled', 'loading', 'empty', 'error', 'streaming', 'largeData', 'overlay']
  const entries = document['entries'].map((value, index): RenderEvidence => {
    const item = object(value, `render evidence.entries[${index}]`)
    exactKeys(item, [
      'id', 'story', 'source', 'renderKind', 'family', 'interactive', 'interactionKind',
      'checks', 'characterStories', 'stateStories',
    ], `render evidence.entries[${index}]`)
    return {
      id: text(item['id'], `render evidence.entries[${index}].id`),
      story: text(item['story'], `render evidence.entries[${index}].story`),
      source: text(item['source'], `render evidence.entries[${index}].source`),
      renderKind: nullableText(item['renderKind'], `render evidence.entries[${index}].renderKind`),
      family: text(item['family'], `render evidence.entries[${index}].family`),
      interactive: boolean(item['interactive'], `render evidence.entries[${index}].interactive`),
      interactionKind: text(item['interactionKind'], `render evidence.entries[${index}].interactionKind`),
      checks: booleanRecord(item['checks'], checkKeys, `render evidence.entries[${index}].checks`),
      characterStories: storyRecord(item['characterStories'], characterKeys, `render evidence.entries[${index}].characterStories`),
      stateStories: storyRecord(item['stateStories'], stateKeys, `render evidence.entries[${index}].stateStories`),
    }
  })
  const byId = new Map(entries.map((entry) => [entry.id, entry]))
  if (byId.size !== entries.length) throw new Error('render evidence: duplicate IDs')
  return byId
}

function unique(values: readonly string[]): readonly string[] {
  return [...new Set(values)]
}

function expectsPointer(entry: CanonicalEntry): boolean {
  return entry.hints.some((hint) => /click|drag|mouse|pointer|wheel/iu.test(hint))
}

function stateStories(report: RenderEvidence, axis: AxisName): readonly string[] {
  const key = stateAxisKey[axis as keyof typeof stateAxisKey]
  return key === undefined ? [] : report.stateStories[key]
}

function applies(entry: CanonicalEntry, report: RenderEvidence, axis: AxisName): boolean {
  if (universalRenderAxes.has(axis)) return true
  if (characterAxes.has(axis)) {
    if (report.characterStories[axis as CharacterKey].length) return true
    return entry.renderKind !== 'behavior' && entry.family !== 'layout-helper'
  }
  if (axis === 'keyboard' || axis === 'focus') return entry.interactive
  if (axis === 'mouse') return entry.interactive && expectsPointer(entry)
  if (axis === 'escape') return entry.interactive && entry.interactionKind === 'disclosure-overlay'
  if (axis === 'disabled') {
    return entry.interactive && ['action', 'input', 'navigation', 'overlay'].includes(entry.family)
  }
  if (axis === 'overlay') return entry.family === 'overlay' || entry.interactionKind === 'disclosure-overlay'
  if (axis === 'large_data') {
    return stateStories(report, axis).length > 0
      || (entry.family === 'data' && entry.interactionKind === 'scrolling-virtualization')
  }
  return stateStories(report, axis).length > 0
}

function generatedEvidence(
  entry: CanonicalEntry,
  additions: Readonly<{
    stories?: readonly string[]
    tests?: readonly string[]
    posters?: readonly string[]
    checks?: readonly string[]
  }> = {},
): Record<string, unknown> {
  return {
    stories: unique(additions.stories ?? [entry.story]),
    tests: unique(additions.tests ?? []),
    recordings: [],
    benches: [],
    ...(additions.posters?.length ? { posters: unique(additions.posters) } : {}),
    sources: [entry.source],
    ...(additions.checks?.length ? { checks: unique(additions.checks) } : {}),
  }
}

function generatedEvidenceCell(
  value: unknown,
  entry: CanonicalEntry,
  report: RenderEvidence,
  axis: AxisName,
): Record<string, unknown> {
  object(value, `${entry.id}.${axis}`)
  const poster = `docs/public/preview-posters/${entry.story.replaceAll('/', '-')}.json`
  const required = applies(entry, report, axis)
  const applicability = required ? 'required' : 'conditional'

  if (axis === 'visual_states') {
    return {
      applicability,
      status: 'partial',
      evidence: generatedEvidence(entry, {
        stories: [entry.story],
        tests: [exhaustiveInventoryTest],
        posters: [poster],
        checks: [posterCheck],
      }),
      reason: 'Canonical source, mounted story, exact poster, and exhaustive identity test prove one representative state; remaining visual states are unproven.',
    }
  }

  if (interactionAxes.has(axis)) {
    if (!required) {
      const reason = !entry.interactive
        ? `Typed story metadata declares ${entry.interactionKind}; this static owner accepts no input.`
        : axis === 'mouse'
          ? 'Typed interaction hints declare no pointer, drag, or wheel ownership.'
          : 'Typed interaction kind does not own this dismissal path.'
      return {
        applicability,
        status: 'not_applicable',
        evidence: generatedEvidence(entry, { checks: [renderAuditCheck] }),
        reason,
      }
    }
    const key = checkKey[axis as keyof typeof checkKey]
    if (key === undefined) throw new Error(`${entry.id}.${axis}: missing render check key`)
    const proved = report.checks[key]
    return {
      applicability,
      status: proved ? 'partial' : 'missing',
      evidence: generatedEvidence(entry, { checks: [renderAuditCheck] }),
      reason: proved
        ? `Exact mounted representative changed state or emitted an outcome through ${axis}; alternate states and paths remain unproven.`
        : `Exact mounted representative produced no observable ${axis} outcome in the exhaustive host-event probe.`,
    }
  }

  if (characterAxes.has(axis)) {
    const stories = report.characterStories[axis as CharacterKey]
    if (!required) {
      return {
        applicability,
        status: 'not_applicable',
        evidence: generatedEvidence(entry, { checks: [renderAuditCheck] }),
        reason: `Typed ${entry.renderKind ?? 'composed-only'} ${entry.family} owner declares no text-bearing ${axis} fixture capability.`,
      }
    }
    return {
      applicability,
      status: stories.length ? 'partial' : 'missing',
      evidence: generatedEvidence(entry, { stories: stories.length ? stories : [entry.story], checks: [renderAuditCheck] }),
      reason: stories.length
        ? `Exact owner story set renders ${axis} cells without panic; editing, clipping, and alternate states remain unproven.`
        : `No exact owner story renders ${axis} content; support remains unproven.`,
    }
  }

  const key = checkKey[axis as keyof typeof checkKey]
  if (key !== undefined) {
    const proved = report.checks[key]
    return {
      applicability,
      status: proved ? 'partial' : 'missing',
      evidence: generatedEvidence(entry, { checks: [renderAuditCheck] }),
      reason: proved
        ? `Exhaustive representative-story audit exercises exact ${axis} projection; full state space remains unproven.`
        : `Exact representative failed the ${axis} render audit.`,
    }
  }

  const stories = stateStories(report, axis)
  if (!required) {
    return {
      applicability,
      status: 'not_applicable',
      evidence: generatedEvidence(entry, { checks: [renderAuditCheck] }),
      reason: `Typed family, interaction kind, and exact owner story set declare no ${axis} state ownership.`,
    }
  }
  if (axis === 'overlay' && stories.length === 0) {
    return {
      applicability,
      status: 'partial',
      evidence: generatedEvidence(entry, { checks: [renderAuditCheck] }),
      reason: 'Typed overlay family or disclosure interaction and canonical mounted story prove one overlay state; nesting and alternate dismiss paths remain unproven.',
    }
  }
  return {
    applicability,
    status: stories.length ? 'partial' : 'missing',
    evidence: generatedEvidence(entry, { stories: stories.length ? stories : [entry.story], checks: [renderAuditCheck] }),
    reason: stories.length
      ? `Exact owner story metadata and non-empty render prove one ${axis} state; transitions and alternate states remain unproven.`
      : `Typed metadata makes ${axis} applicable, but no exact owner story proves the state.`,
  }
}

function evidenceCell(
  value: unknown,
  entry: CanonicalEntry,
  report: RenderEvidence,
  axis: AxisName,
): Record<string, unknown> {
  const generated = generatedEvidenceCell(value, entry, report, axis)
  const override = ownerEvidenceById.get(entry.id)?.[axis]
  if (!override) return generated
  const generatedEvidenceValue = object(generated['evidence'], `${entry.id}.${axis}.generated evidence`)
  const tests = unique([
    ...textArray(generatedEvidenceValue['tests'], `${entry.id}.${axis}.generated tests`),
    ...override.tests,
  ])
  return {
    applicability: override.applicability,
    status: override.status,
    evidence: {
      ...generatedEvidenceValue,
      tests,
    },
    reason: override.reason,
  }
}

async function synchronizedSource(): Promise<string> {
  const original = await Bun.file(contractPath).text()
  const renderEvidenceById = await loadRenderEvidence()
  const document = object(JSON.parse(original) as unknown, 'contracts')
  if (document['schema'] !== 2 || !Array.isArray(document['entries'])) {
    throw new Error('contracts: expected schema 2 entries')
  }
  const canonicalById = new Map<string, CanonicalEntry>(
    catalogEntries.map((entry) => [entry.id, entry]),
  )
  for (const id of ownerEvidenceById.keys()) {
    if (!canonicalById.has(id)) throw new Error(`${id}: owner evidence has no canonical contract`)
  }
  const entries = await Promise.all(document['entries'].map(async (value, index) => {
    const current = object(value, `contracts.entries[${index}]`)
    const id = text(current['id'], `contracts.entries[${index}].id`)
    const entry = canonicalById.get(id)
    if (!entry) throw new Error(`${id}: missing canonical generated catalog entry`)
    const report = renderEvidenceById.get(id)
    if (!report) throw new Error(`${id}: missing exact render evidence`)
    if (
      report.story !== entry.story
      || report.source !== entry.source
      || report.renderKind !== entry.renderKind
      || report.family !== entry.family
      || report.interactive !== entry.interactive
      || report.interactionKind !== entry.interactionKind
    ) {
      throw new Error(`${id}: render evidence identity or capability metadata drift`)
    }
    const axesValue = object(current['axes'], `${id}.axes`)
    const axes = Object.fromEntries(REQUIRED_AXES.map((axis) => [
      axis,
      evidenceCell(axesValue[axis], entry, report, axis),
    ]))
    const lintsValue = object(current['lints'], `${id}.lints`)
    const productionKeys = hardcodedKeyMatches(await Bun.file(join(root, entry.source)).text())
    const lints = Object.fromEntries(REQUIRED_LINTS.map((lint) => {
      if (lint === 'hardcoded_key_handling') return [lint, productionKeys.length ? 'fail' : 'pass']
      if (lint === 'stale_contract_component') return [lint, 'pass']
      if (lint === 'missing_contract_evidence') {
        return [lint, Object.values(axes).some((axis) => axis['status'] === 'missing') ? 'fail' : 'pass']
      }
      return [lint, text(lintsValue[lint], `${id}.lints.${lint}`)]
    }))
    return {
      schema: 2,
      id,
      entryKind: entry.entryKind,
      complete: boolean(current['complete'], `${id}.complete`),
      axes,
      lints,
    }
  }))
  if (entries.length !== canonicalById.size) {
    throw new Error(`contract coverage: expected ${canonicalById.size}, found ${entries.length}`)
  }
  if (renderEvidenceById.size !== canonicalById.size) {
    throw new Error(`render evidence coverage: expected ${canonicalById.size}, found ${renderEvidenceById.size}`)
  }
  return `${JSON.stringify({ schema: 2, version: document['version'], entries }, null, 2)}\n`
}

const synchronized = await synchronizedSource()
if (process.argv.includes('--check')) {
  const current = await Bun.file(contractPath).text()
  if (current !== synchronized) throw new Error('component-contracts.v2.json is stale; run bun run sync:contract-evidence')
} else {
  await Bun.write(contractPath, synchronized)
}
console.log(`${catalogEntries.length} contract evidence entries synchronized`)
