import { join } from 'node:path'
import {
  hardcodedKeyMatches,
  loadRustAuthority,
  REQUIRED_AXES,
  REQUIRED_LINTS,
} from './catalog-data'

function argument(name: string): string | undefined {
  const index = Bun.argv.indexOf(name)
  return index < 0 ? undefined : Bun.argv[index + 1]
}

const component = argument('--component')
const description = argument('--description')
const source = argument('--source')
if (!component || !description || !source) {
  throw new Error('usage: bun run scaffold:component --component <PublicUiId> --description <purpose> --source <relative Rust path>')
}

const root = join(import.meta.dir, '..', '..')
if (!(await Bun.file(join(root, source)).exists())) throw new Error(`missing source ${source}`)
const authority = await loadRustAuthority()
const inventory = authority.publicUi.find((entry) => entry.publicUi === component)
if (!inventory || inventory.documentationKind !== 'component') {
  throw new Error(`${component}: not a component-doc PublicUiId`)
}
const story = authority.stories.find((entry) => entry.id === inventory.representativeStory)
if (!story || story.component !== component) throw new Error(`${component}: representative story identity mismatch`)
const tags = [...new Set([inventory.family, inventory.kind])]

const path = join(root, 'docs', 'content', 'docs', 'components', `${inventory.docsSlug}.mdx`)
if (await Bun.file(path).exists()) throw new Error(`refusing to overwrite ${path}`)

const page = `---
title: ${JSON.stringify(component.replaceAll(/([a-z0-9])([A-Z])/gu, '$1 $2'))}
description: ${JSON.stringify(description)}
catalogId: ${component}
source: ${source}
tags:
${tags.map((tag) => `  - ${JSON.stringify(tag)}`).join('\n')}
aliases: []
---

## State and typed outcomes

Document the public state and typed outcomes. Keep domain effects caller-owned.

## Minimal implementation

Add a compiling minimal use of the exact public \`${component}\` API.

## Common mistakes

- Do not substitute a similarly named recipe or alias.

## Test recipe

Start with \`${story.id}\`; add focused evidence before changing any contract status.

## Source and related material

- Public definition: \`${source}\`
- Representative story: \`${story.id}\`

## Seen in applications

<SeenInApplications component="${component}" />
`

const contractPath = join(root, 'docs', 'api', 'component-contracts.v2.json')
const contractValue: unknown = await Bun.file(contractPath).json()
if (typeof contractValue !== 'object' || contractValue === null || Array.isArray(contractValue)) {
  throw new Error('invalid v2 contract document')
}
const document = contractValue as Record<string, unknown>
if (document['schema'] !== 2 || !Array.isArray(document['entries'])) throw new Error('invalid v2 contract document')
if (document['entries'].some((value) => (
  typeof value === 'object' && value !== null && 'id' in value && value.id === component
))) throw new Error(`${component}: contract already exists`)

const emptyEvidence = { stories: [], tests: [], recordings: [], benches: [] }
const interactionAxes = new Set(['keyboard', 'mouse', 'focus', 'escape'])
const universallyRequired = new Set([
  'visual_states',
  'responsive',
  'tiny_terminal',
  'unicode',
  'cjk',
  'combining',
  'emoji',
  'ascii_fallback',
  'no_color',
  'color_ladder',
  'resize',
  'panic_safety',
])
const poster = `docs/public/preview-posters/${story.id.replaceAll('/', '-')}.json`
const axes = Object.fromEntries(REQUIRED_AXES.map((axis) => [
  axis,
  axis === 'visual_states'
    ? {
        applicability: 'required',
        status: 'partial',
        evidence: {
          ...emptyEvidence,
          stories: [story.id],
          tests: ['crates/termrock-catalog/src/catalog.rs#representative_scenarios_cover_component_and_pattern_registries'],
          posters: [poster],
          sources: [source],
          checks: ['docs/scripts/export-preview-posters.ts#validate'],
        },
        reason: 'Canonical source, mounted story, exact poster, and exhaustive identity test prove one representative state; remaining visual states are unproven.',
      }
    : axis === 'panic_safety'
      ? {
        applicability: 'required',
        status: 'partial',
        evidence: {
          ...emptyEvidence,
          stories: [story.id],
          posters: [poster],
          sources: [source],
          checks: ['docs/scripts/export-preview-posters.ts#validate'],
        },
        reason: 'Canonical poster export renders one representative state at one canonical size; zero-area and full state-space safety remain unproven.',
      }
      : axis === 'no_color'
        ? {
          applicability: 'required',
          status: 'partial',
          evidence: {
            ...emptyEvidence,
            tests: ['crates/termrock/tests/design_gate.rs#public_ui_inventory_has_exact_recipe_and_monochrome_evidence'],
            sources: [source],
          },
          reason: 'Exhaustive public-UI inventory evidence proves a non-color cue and monochrome hierarchy recipe for this exact ID; rendered states remain unproven.',
        }
        : {
          applicability: universallyRequired.has(axis)
            || (story.interactive && interactionAxes.has(axis))
            ? 'required'
            : 'conditional',
          status: 'missing',
          evidence: emptyEvidence,
          reason: `Add focused ${axis} evidence and cite it before claiming coverage.`,
        },
]))
const sourceKeys = hardcodedKeyMatches(await Bun.file(join(root, source)).text())
const lints = Object.fromEntries(REQUIRED_LINTS.map((lint) => {
  if (lint === 'hardcoded_key_handling') return [lint, sourceKeys.length ? 'fail' : 'pass']
  if (lint === 'stale_contract_component') return [lint, 'pass']
  if (lint === 'missing_contract_evidence') return [lint, 'fail']
  return [lint, 'not_run']
}))
document['entries'].push({ schema: 2, id: component, entryKind: 'component', complete: false, axes, lints })
document['entries'].sort((left, right) => {
  const leftId = typeof left === 'object' && left !== null && 'id' in left && typeof left.id === 'string' ? left.id : ''
  const rightId = typeof right === 'object' && right !== null && 'id' in right && typeof right.id === 'string' ? right.id : ''
  return leftId.localeCompare(rightId)
})

await Bun.write(path, page)
await Bun.write(contractPath, `${JSON.stringify(document, null, 2)}\n`)
console.log(`created ${path}; seeded truthful incomplete v2 contract`)
