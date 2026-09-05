import { readdir } from 'node:fs/promises'
import { join } from 'node:path'
import { frontmatter, list, scalar } from './doc-frontmatter'

export const REQUIRED_AXES = [
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

export const REQUIRED_LINTS = [
  'color_only_state',
  'invisible_keyboard_focus',
  'primary_clipped_before_secondary',
  'interactive_without_semantic_role',
  'mouse_without_keyboard',
  'unpredictable_overlay_dismiss',
  'hardcoded_key_handling',
  'missing_ascii_fallback',
  'focus_selection_indistinguishable',
  'idle_animation_redraw',
  'zero_area_panic',
  'missing_contract_evidence',
  'stale_contract_component',
] as const

const AXIS_STATUSES = ['covered', 'partial', 'caller_owned', 'not_applicable', 'missing'] as const
export const AXIS_APPLICABILITIES = ['required', 'conditional'] as const
const LINT_STATUSES = ['pass', 'fail', 'waived', 'not_run'] as const
const ENTRY_KINDS = ['component', 'pattern'] as const
const EVIDENCE_REQUIRED_STATUSES = ['covered', 'partial'] as const
const REASON_REQUIRED_STATUSES = ['partial', 'caller_owned', 'not_applicable', 'missing'] as const
const COMPLETE_AXIS_STATUSES = ['covered', 'partial', 'caller_owned', 'not_applicable'] as const
const COMPLETE_LINT_STATUSES = ['pass', 'waived'] as const
const COMPLETE_WAIVER_STATUS = 'partial' as const

export type AxisName = (typeof REQUIRED_AXES)[number]
export type AxisStatus = (typeof AXIS_STATUSES)[number]
export type AxisApplicability = (typeof AXIS_APPLICABILITIES)[number]
export type LintStatus = (typeof LINT_STATUSES)[number]

export type Evidence = Readonly<{
  stories: readonly string[]
  tests: readonly string[]
  recordings: readonly string[]
  benches: readonly string[]
  posters: readonly string[]
  sources: readonly string[]
  checks: readonly string[]
  notes?: string
}>

export type AxisCell = Readonly<{
  applicability: AxisApplicability
  status: AxisStatus
  evidence: Evidence
  reason?: string
  waiver?: Readonly<{ ticket?: string; expires?: string }>
}>

export type Contract = Readonly<{
  schema: 2
  id: string
  entryKind: 'component' | 'pattern'
  complete: boolean
  axes: Readonly<Record<AxisName, AxisCell>>
  lints: Readonly<Record<(typeof REQUIRED_LINTS)[number], LintStatus>>
}>

export type ContractDocument = Readonly<{
  version: string
  entries: readonly Contract[]
}>

export type PublicUi = Readonly<{
  publicUi: string
  kind: 'widget' | 'paint' | 'layout' | 'behavior'
  family: string
  documentationKind: 'component' | 'pattern'
  docsSlug: string
  docsPath: string
  representativeStory: string
}>

export type Pattern = Readonly<{
  pattern: string
  docsSlug: string
  docsPath: string
  representativeStory: string
  publicUi: string | null
}>

export type Story = Readonly<{
  id: string
  title: string
  component: string
  description: string
  cols: number
  rows: number
  interactive: boolean
  interactionKind: string
  hints: readonly string[]
}>

type Family = Readonly<{
  id: string
  title: string
  description: string
  order: number
}>

type Editorial = Readonly<{
  id: string
  slug: string
  title: string
  description: string
  source: string
  tags: readonly string[]
  aliases: readonly string[]
  classification: string | undefined
  uses: readonly string[]
  supportingTypes: readonly string[]
  body: string
}>

export type CatalogEntry = Readonly<{
  id: string
  entryKind: 'component' | 'pattern'
  publicUi: string | null
  renderKind: PublicUi['kind'] | null
  family: string
  slug: string
  href: string
  title: string
  purpose: string
  tags: readonly string[]
  aliases: readonly string[]
  source: string
  story: string
  storyComponent: string
  storyTitle: string
  interactive: boolean
  interactionKind: string
  hints: readonly string[]
  dimensions: Readonly<{ cols: number; rows: number }>
  uses: readonly string[]
  supportingTypes: readonly string[]
  coverage: Readonly<{
    complete: boolean
    covered: number
    partial: number
    missing: number
    total: number
  }>
  authoredGuidance: boolean
}>

export type Catalog = Readonly<{
  componentFamilies: readonly Family[]
  patternFamilies: readonly Family[]
  components: readonly CatalogEntry[]
  patterns: readonly CatalogEntry[]
  contracts: readonly Contract[]
  stories: readonly Story[]
}>

const root = join(import.meta.dir, '..', '..')
const docs = join(root, 'docs', 'content', 'docs')
const forbiddenAuthorities = [
  'docs/api/component-routes.json',
  'docs/api/pattern-catalog.json',
  'docs/api/component-contracts.json',
  'docs/api/component-contracts.v2.example.json',
  'docs/scripts/sync-component-contracts.ts',
  'docs/scripts/sync-pattern-actions.ts',
] as const

export function contractSchema(): Readonly<Record<string, unknown>> {
  const axisProperties = Object.fromEntries(
    REQUIRED_AXES.map((axis) => [axis, { $ref: '#/$defs/axis' }]),
  )
  const completeAxisProperties = Object.fromEntries(
    REQUIRED_AXES.map((axis) => [axis, { $ref: '#/$defs/completeAxis' }]),
  )
  const lintProperties = Object.fromEntries(
    REQUIRED_LINTS.map((lint) => [lint, { $ref: '#/$defs/lintStatus' }]),
  )
  const completeLintProperties = Object.fromEntries(
    REQUIRED_LINTS.map((lint) => [lint, { enum: COMPLETE_LINT_STATUSES }]),
  )
  return {
    $schema: 'https://json-schema.org/draft/2020-12/schema',
    $id: 'https://termrock.dev/schemas/component-contract.schema.json',
    title: 'TermRock catalog quality contracts',
    description: 'Complete evidence ledger for every canonical component and pattern documentation entry.',
    type: 'object',
    required: ['schema', 'version', 'entries'],
    additionalProperties: false,
    properties: {
      schema: { const: 2 },
      version: { $ref: '#/$defs/nonBlankString' },
      entries: { type: 'array', minItems: 1, items: { $ref: '#/$defs/entry' } },
    },
    $defs: {
      status: { type: 'string', enum: AXIS_STATUSES },
      applicability: { type: 'string', enum: AXIS_APPLICABILITIES },
      lintStatus: { type: 'string', enum: LINT_STATUSES },
      nonBlankString: { type: 'string', minLength: 1, pattern: '\\S' },
      stringList: { type: 'array', items: { $ref: '#/$defs/nonBlankString' } },
      evidence: {
        type: 'object',
        required: ['stories', 'tests', 'recordings', 'benches'],
        additionalProperties: false,
        properties: {
          stories: { $ref: '#/$defs/stringList' },
          tests: { $ref: '#/$defs/stringList' },
          recordings: { $ref: '#/$defs/stringList' },
          benches: { $ref: '#/$defs/stringList' },
          posters: { $ref: '#/$defs/stringList' },
          sources: { $ref: '#/$defs/stringList' },
          checks: { $ref: '#/$defs/stringList' },
          notes: { $ref: '#/$defs/nonBlankString' },
        },
      },
      waiver: {
        type: 'object',
        additionalProperties: false,
        properties: {
          ticket: { $ref: '#/$defs/nonBlankString' },
          expires: { type: 'string', format: 'date' },
        },
      },
      axis: {
        type: 'object',
        required: ['applicability', 'status', 'evidence'],
        additionalProperties: false,
        properties: {
          applicability: { $ref: '#/$defs/applicability' },
          status: { $ref: '#/$defs/status' },
          evidence: { $ref: '#/$defs/evidence' },
          reason: { $ref: '#/$defs/nonBlankString' },
          waiver: { $ref: '#/$defs/waiver' },
        },
        allOf: [
          {
            if: { properties: { status: { enum: REASON_REQUIRED_STATUSES } } },
            then: { required: ['reason'] },
          },
          {
            if: { properties: { status: { enum: EVIDENCE_REQUIRED_STATUSES } } },
            then: {
              properties: {
                evidence: {
                  anyOf: [
                    { properties: { stories: { minItems: 1 } } },
                    { properties: { tests: { minItems: 1 } } },
                    { properties: { recordings: { minItems: 1 } } },
                    { properties: { benches: { minItems: 1 } } },
                    { properties: { posters: { minItems: 1 } } },
                    { properties: { checks: { minItems: 1 } } },
                  ],
                },
              },
            },
          },
        ],
      },
      completeAxis: {
        allOf: [
          { $ref: '#/$defs/axis' },
          { properties: { status: { enum: COMPLETE_AXIS_STATUSES } } },
          {
            if: { properties: { applicability: { const: 'required' } } },
            then: { properties: { status: { enum: ['covered', 'partial'] } } },
          },
          {
            if: { properties: { status: { const: COMPLETE_WAIVER_STATUS } } },
            then: {
              required: ['waiver'],
              properties: { waiver: { required: ['ticket'] } },
            },
          },
        ],
      },
      axes: {
        type: 'object',
        additionalProperties: false,
        required: REQUIRED_AXES,
        properties: axisProperties,
      },
      completeAxes: {
        type: 'object',
        additionalProperties: false,
        required: REQUIRED_AXES,
        properties: completeAxisProperties,
      },
      lints: {
        type: 'object',
        additionalProperties: false,
        required: REQUIRED_LINTS,
        properties: lintProperties,
      },
      completeLints: {
        type: 'object',
        additionalProperties: false,
        required: REQUIRED_LINTS,
        properties: completeLintProperties,
      },
      entry: {
        type: 'object',
        required: ['schema', 'id', 'entryKind', 'complete', 'axes', 'lints'],
        additionalProperties: false,
        properties: {
          schema: { const: 2 },
          id: { type: 'string', pattern: '^[A-Z][A-Za-z0-9_]*$' },
          entryKind: { type: 'string', enum: ENTRY_KINDS },
          complete: { type: 'boolean' },
          axes: { $ref: '#/$defs/axes' },
          lints: { $ref: '#/$defs/lints' },
        },
        allOf: [{
          if: { properties: { complete: { const: true } } },
          then: {
            properties: {
              axes: { $ref: '#/$defs/completeAxes' },
              lints: { $ref: '#/$defs/completeLints' },
            },
          },
        }],
      },
    },
  }
}

export function contractSchemaSource(): string {
  return `${JSON.stringify(contractSchema(), null, 2)}\n`
}

function object(value: unknown, label: string): Record<string, unknown> {
  if (typeof value !== 'object' || value === null || Array.isArray(value)) {
    throw new Error(`${label}: expected object`)
  }
  return value as Record<string, unknown>
}

function array(value: unknown, label: string): readonly unknown[] {
  if (!Array.isArray(value)) throw new Error(`${label}: expected array`)
  return value
}

function text(value: unknown, label: string): string {
  if (typeof value !== 'string' || !value.trim()) throw new Error(`${label}: expected non-empty string`)
  return value
}

function integer(value: unknown, label: string): number {
  if (typeof value !== 'number' || !Number.isInteger(value) || value < 0) {
    throw new Error(`${label}: expected non-negative integer`)
  }
  return value
}

function boolean(value: unknown, label: string): boolean {
  if (typeof value !== 'boolean') throw new Error(`${label}: expected boolean`)
  return value
}

function optionalText(value: unknown, label: string): string | undefined {
  return value === undefined ? undefined : text(value, label)
}

function nullableOptionalText(value: unknown, label: string): string | undefined {
  return value === undefined || value === null ? undefined : text(value, label)
}

function isoDate(value: unknown, label: string): string {
  const source = text(value, label)
  const match = /^(\d{4})-(\d{2})-(\d{2})$/u.exec(source)
  if (!match) throw new Error(`${label}: expected ISO 8601 calendar date`)
  const year = Number(match[1])
  const month = Number(match[2])
  const day = Number(match[3])
  const parsed = new Date(0)
  parsed.setUTCHours(0, 0, 0, 0)
  parsed.setUTCFullYear(year, month - 1, day)
  if (
    parsed.getUTCFullYear() !== year
    || parsed.getUTCMonth() !== month - 1
    || parsed.getUTCDate() !== day
  ) {
    throw new Error(`${label}: invalid ISO 8601 calendar date ${source}`)
  }
  return source
}

function textArray(value: unknown, label: string): readonly string[] {
  return array(value, label).map((item, index) => text(item, `${label}[${index}]`))
}

function enumValue<const Values extends readonly string[]>(
  value: unknown,
  values: Values,
  label: string,
): Values[number] {
  const candidate = text(value, label)
  if (!values.includes(candidate)) throw new Error(`${label}: invalid value ${candidate}`)
  return candidate
}

function contains(values: readonly string[], value: string): boolean {
  return values.some((candidate) => candidate === value)
}

export type HardcodedKeyMatch = Readonly<{
  literal: string
  line: number
}>

export function hardcodedKeyMatches(source: string): readonly HardcodedKeyMatch[] {
  const tests = /^\s*mod tests\s*\{/mu.exec(source)
  const production = tests ? source.slice(0, tests.index) : source
  const rawKey = /KeyCode::Char\(\s*'[^']+'(?:\s*\|\s*'[^']+')*\s*\)|KeyCode::(?:F\d+|Esc|Enter|Tab)\b/gu
  return [...production.matchAll(rawKey)].map((match) => ({
    literal: match[0],
    line: production.slice(0, match.index).split('\n').length,
  }))
}

function unique<T>(values: readonly T[], key: (value: T) => string, label: string): void {
  const seen = new Set<string>()
  for (const value of values) {
    const id = key(value)
    if (seen.has(id)) throw new Error(`${label}: duplicate ${id}`)
    seen.add(id)
  }
}

function exactKeys(value: Record<string, unknown>, allowed: readonly string[], label: string): void {
  const allowedKeys = new Set(allowed)
  const unknown = Object.keys(value).filter((key) => !allowedKeys.has(key))
  if (unknown.length) throw new Error(`${label}: unknown fields ${unknown.join(', ')}`)
}

function parsePublicUi(value: unknown, index: number): PublicUi {
  const item = object(value, `inventory.publicUi[${index}]`)
  return {
    publicUi: text(item['publicUi'], `inventory.publicUi[${index}].publicUi`),
    kind: enumValue(item['kind'], ['widget', 'paint', 'layout', 'behavior'] as const, `inventory.publicUi[${index}].kind`),
    family: text(item['family'], `inventory.publicUi[${index}].family`),
    documentationKind: enumValue(item['documentationKind'], ['component', 'pattern'] as const, `inventory.publicUi[${index}].documentationKind`),
    docsSlug: text(item['docsSlug'], `inventory.publicUi[${index}].docsSlug`),
    docsPath: text(item['docsPath'], `inventory.publicUi[${index}].docsPath`),
    representativeStory: text(item['representativeStory'], `inventory.publicUi[${index}].representativeStory`),
  }
}

function parsePattern(value: unknown, index: number): Pattern {
  const item = object(value, `inventory.patterns[${index}]`)
  return {
    pattern: text(item['pattern'], `inventory.patterns[${index}].pattern`),
    docsSlug: text(item['docsSlug'], `inventory.patterns[${index}].docsSlug`),
    docsPath: text(item['docsPath'], `inventory.patterns[${index}].docsPath`),
    representativeStory: text(item['representativeStory'], `inventory.patterns[${index}].representativeStory`),
    publicUi: nullableOptionalText(item['publicUi'], `inventory.patterns[${index}].publicUi`) ?? null,
  }
}

function parseStory(value: unknown, index: number): Story {
  const item = object(value, `stories[${index}]`)
  return {
    id: text(item['id'], `stories[${index}].id`),
    title: text(item['title'], `stories[${index}].title`),
    component: text(item['component'], `stories[${index}].component`),
    description: text(item['description'], `stories[${index}].description`),
    cols: integer(item['cols'], `stories[${index}].cols`),
    rows: integer(item['rows'], `stories[${index}].rows`),
    interactive: boolean(item['interactive'], `stories[${index}].interactive`),
    interactionKind: text(item['interactionKind'], `stories[${index}].interactionKind`),
    hints: textArray(item['hints'], `stories[${index}].hints`),
  }
}

function parseFamily(value: unknown, label: string): Family {
  const item = object(value, label)
  return {
    id: text(item['id'], `${label}.id`),
    title: text(item['title'], `${label}.title`),
    description: text(item['description'], `${label}.description`),
    order: integer(item['order'], `${label}.order`),
  }
}

function parseEvidence(value: unknown, label: string): Evidence {
  const item = object(value, label)
  exactKeys(item, ['stories', 'tests', 'recordings', 'benches', 'posters', 'sources', 'checks', 'notes'], label)
  const evidence: Evidence = {
    stories: textArray(item['stories'], `${label}.stories`),
    tests: textArray(item['tests'], `${label}.tests`),
    recordings: textArray(item['recordings'], `${label}.recordings`),
    benches: textArray(item['benches'], `${label}.benches`),
    posters: item['posters'] === undefined ? [] : textArray(item['posters'], `${label}.posters`),
    sources: item['sources'] === undefined ? [] : textArray(item['sources'], `${label}.sources`),
    checks: item['checks'] === undefined ? [] : textArray(item['checks'], `${label}.checks`),
  }
  const notes = optionalText(item['notes'], `${label}.notes`)
  return notes ? { ...evidence, notes } : evidence
}

function parseContract(value: unknown, index: number): Contract {
  const label = `contracts[${index}]`
  const item = object(value, label)
  exactKeys(item, ['schema', 'id', 'entryKind', 'complete', 'axes', 'lints'], label)
  if (item['schema'] !== 2) throw new Error(`${label}.schema: expected 2`)
  const axesValue = object(item['axes'], `${label}.axes`)
  const axes = Object.create(null) as Record<AxisName, AxisCell>
  for (const axis of REQUIRED_AXES) {
    const cellValue = object(axesValue[axis], `${label}.axes.${axis}`)
    exactKeys(cellValue, ['applicability', 'status', 'evidence', 'reason', 'waiver'], `${label}.axes.${axis}`)
    const applicability = enumValue(
      cellValue['applicability'],
      AXIS_APPLICABILITIES,
      `${label}.axes.${axis}.applicability`,
    )
    const status = enumValue(
      cellValue['status'],
      AXIS_STATUSES,
      `${label}.axes.${axis}.status`,
    )
    const evidence = parseEvidence(cellValue['evidence'], `${label}.axes.${axis}.evidence`)
    const reason = optionalText(cellValue['reason'], `${label}.axes.${axis}.reason`)
    const evidenceCount = evidence.stories.length + evidence.tests.length + evidence.posters.length
      + evidence.checks.length + evidence.recordings.length + evidence.benches.length
    if (contains(EVIDENCE_REQUIRED_STATUSES, status) && evidenceCount === 0) {
      throw new Error(`${label}.axes.${axis}: ${status} requires concrete evidence`)
    }
    if (contains(REASON_REQUIRED_STATUSES, status) && !reason) {
      throw new Error(`${label}.axes.${axis}: ${status} requires reason`)
    }
    const waiverValue = cellValue['waiver']
    const waiver = waiverValue === undefined ? undefined : (() => {
      const parsed = object(waiverValue, `${label}.axes.${axis}.waiver`)
      exactKeys(parsed, ['ticket', 'expires'], `${label}.axes.${axis}.waiver`)
      const ticket = optionalText(parsed['ticket'], `${label}.axes.${axis}.waiver.ticket`)
      const expires = parsed['expires'] === undefined
        ? undefined
        : isoDate(parsed['expires'], `${label}.axes.${axis}.waiver.expires`)
      return {
        ...(ticket ? { ticket } : {}),
        ...(expires ? { expires } : {}),
      }
    })()
    axes[axis] = {
      applicability,
      status,
      evidence,
      ...(reason ? { reason } : {}),
      ...(waiver ? { waiver } : {}),
    }
  }
  const unexpectedAxes = Object.keys(axesValue).filter((axis) => !REQUIRED_AXES.includes(axis as AxisName))
  if (unexpectedAxes.length) throw new Error(`${label}.axes: unknown axes ${unexpectedAxes.join(', ')}`)

  const lintsValue = object(item['lints'], `${label}.lints`)
  const lints = Object.create(null) as Record<(typeof REQUIRED_LINTS)[number], LintStatus>
  for (const lint of REQUIRED_LINTS) {
    lints[lint] = enumValue(lintsValue[lint], LINT_STATUSES, `${label}.lints.${lint}`)
  }
  const unexpectedLints = Object.keys(lintsValue).filter((lint) => !REQUIRED_LINTS.includes(lint as (typeof REQUIRED_LINTS)[number]))
  if (unexpectedLints.length) throw new Error(`${label}.lints: unknown lints ${unexpectedLints.join(', ')}`)

  const id = text(item['id'], `${label}.id`)
  if (!/^[A-Z][A-Za-z0-9_]*$/u.test(id)) throw new Error(`${label}.id: invalid catalog id ${id}`)
  const contract: Contract = {
    schema: 2,
    id,
    entryKind: enumValue(item['entryKind'], ENTRY_KINDS, `${label}.entryKind`),
    complete: boolean(item['complete'], `${label}.complete`),
    axes,
    lints,
  }
  if (contract.complete) {
    for (const [axis, cell] of Object.entries(contract.axes)) {
      if (!contains(COMPLETE_AXIS_STATUSES, cell.status)) {
        throw new Error(`${contract.id}: complete contract has ${cell.status} ${axis}`)
      }
      if (cell.status === COMPLETE_WAIVER_STATUS && !cell.waiver?.ticket) {
        throw new Error(`${contract.id}: complete contract has unwaived partial ${axis}`)
      }
      if (
        cell.applicability === 'required'
        && (cell.status === 'caller_owned' || cell.status === 'not_applicable')
      ) {
        throw new Error(`${contract.id}: required ${axis} cannot be ${cell.status}`)
      }
    }
    for (const [lint, status] of Object.entries(contract.lints)) {
      if (!contains(COMPLETE_LINT_STATUSES, status)) {
        throw new Error(`${contract.id}: complete contract has ${status} lint ${lint}`)
      }
    }
  }
  return contract
}

export function parseContractDocument(value: unknown): ContractDocument {
  const item = object(value, 'contracts')
  exactKeys(item, ['schema', 'version', 'entries'], 'contracts')
  if (item['schema'] !== 2) throw new Error('contracts.schema: expected 2')
  return {
    version: text(item['version'], 'contracts.version'),
    entries: array(item['entries'], 'contracts.entries').map(parseContract),
  }
}

async function json(path: string): Promise<unknown> {
  const source = await Bun.file(path).text()
  return JSON.parse(source) as unknown
}

async function rustJson(command: readonly string[]): Promise<unknown> {
  const result = Bun.spawnSync([...command], { cwd: root })
  if (result.exitCode !== 0) throw new Error(result.stderr.toString())
  return JSON.parse(result.stdout.toString()) as unknown
}

export async function loadRustAuthority(): Promise<Readonly<{
  publicUi: readonly PublicUi[]
  patterns: readonly Pattern[]
  stories: readonly Story[]
}>> {
  const inventoryValue = object(await rustJson(
    ['cargo', 'run', '-q', '-p', 'termrock-catalog', '--', 'authority', '--format', 'json'],
  ), 'inventory')
  return {
    publicUi: array(inventoryValue['publicUi'], 'inventory.publicUi').map(parsePublicUi),
    patterns: array(inventoryValue['patterns'], 'inventory.patterns').map(parsePattern),
    stories: array(await rustJson(
      ['cargo', 'run', '-q', '-p', 'termrock-catalog', '--', 'scenarios', '--format', 'json'],
    ), 'stories').map(parseStory),
  }
}

async function editorial(directory: 'components' | 'patterns'): Promise<readonly Editorial[]> {
  const folder = join(docs, directory)
  const names = (await readdir(folder)).filter((name) => name.endsWith('.mdx')).sort()
  return Promise.all(names.map(async (name) => {
    const body = await Bun.file(join(folder, name)).text()
    const metadata = frontmatter(body)
    if (/<TerminalPreview\b/u.test(body.replace(metadata, ''))) {
      throw new Error(`${directory}/${name}: preview identity belongs to the shared detail layout`)
    }
    const forbidden = directory === 'components'
      ? ['component', 'demo', 'interaction', 'actions', 'expectedOutcomes']
      : ['pattern', 'demo', 'interaction', 'actions', 'buildingBlocks']
    for (const key of forbidden) {
      if (new RegExp(`^${key}:`, 'm').test(metadata)) throw new Error(`${directory}/${name}: forbidden generated field ${key}`)
    }
    for (const key of ['catalogId', 'title', 'description', 'source', 'tags', 'aliases']) {
      if (!new RegExp(`^${key}:`, 'm').test(metadata)) throw new Error(`${directory}/${name}: missing ${key}`)
    }
    if (directory === 'patterns') {
      for (const key of ['classification', 'uses', 'supportingTypes']) {
        if (!new RegExp(`^${key}:`, 'm').test(metadata)) throw new Error(`${directory}/${name}: missing ${key}`)
      }
    }
    return {
      id: scalar(metadata, 'catalogId') ?? '',
      slug: name.replace(/\.mdx$/u, ''),
      title: scalar(metadata, 'title') ?? '',
      description: scalar(metadata, 'description') ?? '',
      source: scalar(metadata, 'source') ?? '',
      tags: list(metadata, 'tags'),
      aliases: list(metadata, 'aliases'),
      classification: scalar(metadata, 'classification'),
      uses: list(metadata, 'uses'),
      supportingTypes: list(metadata, 'supportingTypes'),
      body,
    }
  }))
}

function exactSet(actual: Iterable<string>, expected: Iterable<string>, label: string): void {
  const actualSet = new Set(actual)
  const expectedSet = new Set(expected)
  const missing = [...expectedSet].filter((value) => !actualSet.has(value)).sort()
  const extra = [...actualSet].filter((value) => !expectedSet.has(value)).sort()
  if (missing.length || extra.length) {
    throw new Error(`${label}: missing [${missing.join(', ')}]; extra [${extra.join(', ')}]`)
  }
}

function hasAuthoredGuidance(body: string): boolean {
  return body.replace(/^---\n[\s\S]*?\n---\n?/u, '').trim().length > 0
}

function coverage(contract: Contract): CatalogEntry['coverage'] {
  const values = Object.values(contract.axes)
  return {
    complete: contract.complete,
    covered: values.filter((axis) => axis.status === 'covered').length,
    partial: values.filter((axis) => axis.status === 'partial').length,
    missing: values.filter((axis) => axis.status === 'missing').length,
    total: values.length,
  }
}

export async function loadCatalog(): Promise<Catalog> {
  for (const path of forbiddenAuthorities) {
    if (await Bun.file(join(root, path)).exists()) throw new Error(`legacy catalog authority exists: ${path}`)
  }
  const { publicUi, patterns, stories } = await loadRustAuthority()
  const familiesValue = object(await json(join(root, 'docs', 'catalog', 'families.json')), 'families')
  if (familiesValue['schema'] !== 1) throw new Error('families.schema: expected 1')
  const componentFamilies = array(familiesValue['components'], 'families.components')
    .map((value, index) => parseFamily(value, `families.components[${index}]`))
  const patternFamilies = array(familiesValue['patterns'], 'families.patterns')
    .map((value, index) => parseFamily(value, `families.patterns[${index}]`))
  const contracts = parseContractDocument(
    await json(join(root, 'docs', 'api', 'component-contracts.v2.json')),
  ).entries
  const componentEditorial = await editorial('components')
  const patternEditorial = await editorial('patterns')

  unique(publicUi, (entry) => entry.publicUi, 'inventory public UI')
  unique(publicUi, (entry) => entry.docsSlug, 'inventory public UI slug')
  unique(patterns, (entry) => entry.pattern, 'pattern inventory')
  unique(patterns, (entry) => entry.docsSlug, 'pattern inventory slug')
  unique(stories, (entry) => entry.id, 'story catalog')
  unique(componentFamilies, (entry) => entry.id, 'component family')
  unique(patternFamilies, (entry) => entry.id, 'pattern family')
  unique(contracts, (entry) => entry.id, 'contract')
  unique([...componentEditorial, ...patternEditorial], (entry) => entry.id, 'editorial catalog id')
  unique([...componentEditorial, ...patternEditorial], (entry) => entry.slug, 'editorial route slug')

  const publicUiById = new Map(publicUi.map((entry) => [entry.publicUi, entry]))
  const storyById = new Map(stories.map((entry) => [entry.id, entry]))
  const contractById = new Map(contracts.map((entry) => [entry.id, entry]))
  const componentFamilyIds = new Set(componentFamilies.map((entry) => entry.id))
  const patternFamilyIds = new Set(patternFamilies.map((entry) => entry.id))
  const componentInventory = publicUi.filter((entry) => entry.documentationKind === 'component')
  const patternPublicUi = publicUi.filter((entry) => entry.documentationKind === 'pattern')

  exactSet(componentFamilies.map((entry) => entry.id), componentInventory.map((entry) => entry.family), 'component families')
  exactSet(patterns.flatMap((entry) => entry.publicUi ? [entry.publicUi] : []), patternPublicUi.map((entry) => entry.publicUi), 'linked pattern public UI')
  exactSet(componentEditorial.map((entry) => entry.id), componentInventory.map((entry) => entry.publicUi), 'component pages')
  exactSet(patternEditorial.map((entry) => entry.id), patterns.map((entry) => entry.pattern), 'pattern pages')
  exactSet(patternFamilies.map((entry) => entry.id), patternEditorial.flatMap((entry) => entry.classification ? [entry.classification] : []), 'pattern families')
  exactSet(contracts.map((entry) => entry.id), [...componentInventory.map((entry) => entry.publicUi), ...patterns.map((entry) => entry.pattern)], 'contract coverage')

  const componentIds = new Set(componentInventory.map((entry) => entry.publicUi))
  const componentEntries = componentEditorial.map((metadata): CatalogEntry => {
    const inventory = publicUiById.get(metadata.id)
    if (!inventory || inventory.documentationKind !== 'component') throw new Error(`${metadata.id}: missing component inventory`)
    if (metadata.slug !== inventory.docsSlug || inventory.docsPath !== `/docs/components/${metadata.slug}`) {
      throw new Error(`${metadata.id}: component route differs from Rust inventory`)
    }
    const story = storyById.get(inventory.representativeStory)
    if (!story || story.component !== metadata.id) throw new Error(`${metadata.id}: representative story identity mismatch`)
    if (!componentFamilyIds.has(inventory.family)) throw new Error(`${metadata.id}: unknown component family ${inventory.family}`)
    unique(metadata.tags, (tag) => tag, `${metadata.id} tag`)
    unique(metadata.aliases, (alias) => alias, `${metadata.id} alias`)
    if (!Bun.file(join(root, metadata.source)).size) throw new Error(`${metadata.id}: missing source ${metadata.source}`)
    const contract = contractById.get(metadata.id)
    if (!contract || contract.entryKind !== 'component') throw new Error(`${metadata.id}: missing component contract`)
    return {
      id: metadata.id,
      entryKind: 'component',
      publicUi: metadata.id,
      renderKind: inventory.kind,
      family: inventory.family,
      slug: metadata.slug,
      href: inventory.docsPath,
      title: metadata.title,
      purpose: metadata.description,
      tags: metadata.tags,
      aliases: metadata.aliases,
      source: metadata.source,
      story: story.id,
      storyComponent: story.component,
      storyTitle: story.title,
      interactive: story.interactive,
      interactionKind: story.interactionKind,
      hints: story.hints,
      dimensions: { cols: story.cols, rows: story.rows },
      uses: [],
      supportingTypes: [],
      coverage: coverage(contract),
      authoredGuidance: hasAuthoredGuidance(metadata.body),
    }
  }).sort((left, right) => left.title.localeCompare(right.title))

  const patternById = new Map(patterns.map((entry) => [entry.pattern, entry]))
  const patternEntries = patternEditorial.map((metadata): CatalogEntry => {
    const inventory = patternById.get(metadata.id)
    if (!inventory) throw new Error(`${metadata.id}: missing pattern inventory`)
    if (metadata.slug !== inventory.docsSlug || inventory.docsPath !== `/docs/patterns/${metadata.slug}`) {
      throw new Error(`${metadata.id}: pattern route differs from Rust inventory`)
    }
    const publicEntry = inventory.publicUi ? publicUiById.get(inventory.publicUi) : undefined
    if (inventory.publicUi && (!publicEntry || publicEntry.documentationKind !== 'pattern')) {
      throw new Error(`${metadata.id}: invalid linked public UI`)
    }
    const story = storyById.get(inventory.representativeStory)
    if (!story) throw new Error(`${metadata.id}: unknown representative story`)
    if (story.component !== (inventory.publicUi ?? metadata.id)) {
      throw new Error(`${metadata.id}: representative story identity mismatch`)
    }
    if (!metadata.classification || !patternFamilyIds.has(metadata.classification)) {
      throw new Error(`${metadata.id}: unknown pattern classification ${metadata.classification ?? ''}`)
    }
    if (!Bun.file(join(root, metadata.source)).size) throw new Error(`${metadata.id}: missing source ${metadata.source}`)
    for (const used of metadata.uses) if (!componentIds.has(used)) throw new Error(`${metadata.id}: unknown component use ${used}`)
    for (const supporting of metadata.supportingTypes) {
      if (componentIds.has(supporting)) throw new Error(`${metadata.id}: canonical component ${supporting} belongs in uses`)
    }
    unique(metadata.tags, (tag) => tag, `${metadata.id} tag`)
    unique(metadata.aliases, (alias) => alias, `${metadata.id} alias`)
    unique(metadata.uses, (used) => used, `${metadata.id} use`)
    unique(metadata.supportingTypes, (supporting) => supporting, `${metadata.id} supporting type`)
    const contract = contractById.get(metadata.id)
    if (!contract || contract.entryKind !== 'pattern') throw new Error(`${metadata.id}: missing pattern contract`)
    return {
      id: metadata.id,
      entryKind: 'pattern',
      publicUi: inventory.publicUi ?? null,
      renderKind: publicEntry?.kind ?? null,
      family: metadata.classification,
      slug: metadata.slug,
      href: inventory.docsPath,
      title: metadata.title,
      purpose: metadata.description,
      tags: metadata.tags,
      aliases: metadata.aliases,
      source: metadata.source,
      story: story.id,
      storyComponent: story.component,
      storyTitle: story.title,
      interactive: story.interactive,
      interactionKind: story.interactionKind,
      hints: story.hints,
      dimensions: { cols: story.cols, rows: story.rows },
      uses: metadata.uses,
      supportingTypes: metadata.supportingTypes,
      coverage: coverage(contract),
      authoredGuidance: hasAuthoredGuidance(metadata.body),
    }
  }).sort((left, right) => left.title.localeCompare(right.title))

  const canonicalIds = new Set([...componentEntries, ...patternEntries].map((entry) => entry.id))
  const canonicalSlugs = new Set([...componentEntries, ...patternEntries].map((entry) => entry.slug))
  unique(
    [...componentEntries, ...patternEntries],
    (entry) => entry.story,
    'documentation representative story',
  )
  for (const entry of [...componentEntries, ...patternEntries]) {
    const visualEvidence = contractById.get(entry.id)?.axes.visual_states.evidence.stories ?? []
    if (!visualEvidence.includes(entry.story)) {
      throw new Error(`${entry.id}: visual_states evidence must include representative story ${entry.story}`)
    }
  }
  for (const entry of [...componentEntries, ...patternEntries]) {
    for (const alias of entry.aliases) {
      if (canonicalIds.has(alias) || canonicalSlugs.has(alias)) throw new Error(`${entry.id}: alias shadows canonical route ${alias}`)
    }
  }

  const catalogEntryById = new Map(
    [...componentEntries, ...patternEntries].map((entry) => [entry.id, entry]),
  )
  for (const contract of contracts) {
    const entry = catalogEntryById.get(contract.id)
    if (!entry) throw new Error(`${contract.id}: missing canonical catalog entry`)
    for (const [axis, cell] of Object.entries(contract.axes)) {
      for (const story of cell.evidence.stories) {
        const [family, variant, ...extra] = story.split('/')
        if (!family || !variant || extra.length > 0) {
          throw new Error(`${contract.id}.${axis}: invalid canonical scenario evidence ${story}`)
        }
      }
      for (const poster of cell.evidence.posters) {
        if (!poster.startsWith('docs/public/preview-posters/') || !poster.endsWith('.json')) {
          throw new Error(`${contract.id}.${axis}: invalid poster evidence ${poster}`)
        }
        if (!Bun.file(join(root, poster)).size) throw new Error(`${contract.id}.${axis}: missing poster evidence ${poster}`)
        const expectedPoster = `docs/public/preview-posters/${entry.story.replaceAll('/', '-')}.json`
        if (poster !== expectedPoster) {
          throw new Error(`${contract.id}.${axis}: poster differs from canonical story ${poster}`)
        }
      }
      for (const source of cell.evidence.sources) {
        if (source !== entry.source) {
          throw new Error(`${contract.id}.${axis}: source evidence differs from canonical source ${source}`)
        }
      }
      for (const evidence of [
        ...cell.evidence.tests,
        ...cell.evidence.checks,
        ...cell.evidence.benches,
      ]) {
        const [path, anchor] = evidence.split('#')
        if (!path || !Bun.file(join(root, path)).size) throw new Error(`${contract.id}.${axis}: missing evidence ${evidence}`)
        if (!anchor) throw new Error(`${contract.id}.${axis}: evidence needs an exact symbol anchor ${evidence}`)
        const source = await Bun.file(join(root, path)).text()
        const escaped = anchor.replace(/[.*+?^${}()|[\]\\]/gu, '\\$&')
        const declaration = new RegExp(`(?:fn|function)\\s+${escaped}\\b|(?:const|let)\\s+${escaped}\\b`, 'u')
        if (!declaration.test(source)) {
          throw new Error(`${contract.id}.${axis}: evidence symbol does not exist ${evidence}`)
        }
      }
      for (const evidence of cell.evidence.recordings) {
        if (!Bun.file(join(root, evidence)).size) {
          throw new Error(`${contract.id}.${axis}: missing recording evidence ${evidence}`)
        }
      }
    }
  }

  return {
    componentFamilies,
    patternFamilies,
    components: componentEntries,
    patterns: patternEntries,
    contracts,
    stories,
  }
}
