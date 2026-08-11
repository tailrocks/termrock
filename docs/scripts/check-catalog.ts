import { componentSlug } from './component-doc-utils'

const root = `${import.meta.dir}/../..`
const result = Bun.spawnSync(
  ['cargo', 'run', '-q', '-p', 'termrock-lookbook', '--', 'list', '--format', 'json'],
  { cwd: root },
)
if (result.exitCode !== 0) throw new Error(result.stderr.toString())
const stories = JSON.parse(result.stdout.toString()) as Array<{ id: string; component: string; title?: string }>
const ids = new Set(stories.map((story) => story.id))
if (ids.size !== stories.length) throw new Error('duplicate story ID')

const api = await Bun.file(`${root}/docs/api/public-api.txt`).text()
const publicComponents = new Set<string>()
for (const line of api.split('\n')) {
  const match = line.match(/^impl.*ratatui_core::widgets::(?:widget::Widget|stateful_widget::StatefulWidget) for &?termrock::widgets::([A-Z][A-Za-z0-9_]*)/)
  const component = match?.[1]
  if (component) publicComponents.add(component)
}
if (publicComponents.size === 0) throw new Error('public API report contains no canonical widgets')

const componentInventory = await Bun.file(`${root}/crates/termrock/COMPONENTS.md`).text()
const inventorySentence = componentInventory.match(
  /The public widget set is derived from the reviewed API report and currently contains ([^.]+)\./,
)?.[1]
if (!inventorySentence) throw new Error('COMPONENTS.md has no canonical public widget inventory')
const inventoryComponents = new Set(
  [...inventorySentence.matchAll(/`([A-Z][A-Za-z0-9_]*)`/g)].map((match) => {
    const component = match[1]
    if (!component) throw new Error('COMPONENTS.md contains an empty component name')
    return component
  }),
)
const missingInventory = [...publicComponents].filter(
  (component) => !inventoryComponents.has(component),
)
const staleInventory = [...inventoryComponents].filter(
  (component) => !publicComponents.has(component),
)
if (missingInventory.length) {
  throw new Error(`COMPONENTS.md omits public components: ${missingInventory.join(', ')}`)
}
if (staleInventory.length) {
  throw new Error(`COMPONENTS.md lists non-public components: ${staleInventory.join(', ')}`)
}

const storyComponents = new Set(stories.map((story) => story.component))
const missingStories = [...publicComponents].filter((component) => !storyComponents.has(component))
if (missingStories.length) {
  throw new Error(`public components without stories: ${missingStories.join(', ')}`)
}
const unknownStories = [...storyComponents].filter((component) => !publicComponents.has(component))
if (unknownStories.length) {
  console.warn(
    `stories without public Widget components (allowed for patterns/kernel): ${unknownStories.length} tags`,
  )
}

const contractPath = `${root}/docs/api/component-contracts.json`
const contracts = JSON.parse(await Bun.file(contractPath).text()) as Record<
  string,
  Record<string, string>
>
const contractComponents = new Set(Object.keys(contracts))
const missingContracts = [...publicComponents].filter((component) => !contractComponents.has(component))
const staleContracts = [...contractComponents].filter((component) => !publicComponents.has(component))
if (missingContracts.length) throw new Error(`public components without contract review: ${missingContracts.join(', ')}`)
if (staleContracts.length) throw new Error(`contract review contains non-public components: ${staleContracts.join(', ')}`)
const contractNames = [
  'keyboard',
  'mouse',
  'focus',
  'nonColor',
  'unicode',
  'narrowTerminal',
] as const
const contractValues = new Set([
  'covered',
  'caller-owned',
  'not-applicable',
  'partial',
  'missing',
])
for (const [component, review] of Object.entries(contracts)) {
  for (const contract of contractNames) {
    const value = review[contract]
    if (!value || !contractValues.has(value)) {
      throw new Error(`${component} has no valid ${contract} contract`)
    }
  }
}

const NARROW_EXEMPT = new Set([
  'ActionBar', 'Backdrop', 'ChoiceDialog', 'DetailTable', 'DiffView', 'HintBar',
  'LogPane', 'MessageDialog', 'Panel', 'SplitPane', 'TextInput',
  'Tree', 'Viewport',
])
const UNICODE_EXEMPT = new Set([
  'ActionBar', 'ChoiceDialog', 'Dialog', 'DiffView', 'Form', 'HintBar',
  'LogPane', 'MessageDialog', 'Panel', 'StatusBar', 'Tabs', 'Toast',
  'Tree', 'Viewport',
])
function hasAxisStory(component: string, axis: 'narrow' | 'unicode') {
  return stories.some((story) =>
    story.component === component && story.id.split(/[/-]/).includes(axis)
  )
}
for (const [component, review] of Object.entries(contracts)) {
  if (
    review['narrowTerminal'] === 'covered' &&
    !NARROW_EXEMPT.has(component) &&
    !hasAxisStory(component, 'narrow')
  ) {
    console.warn(`${component}: narrowTerminal covered without /narrow story`)
  }
  if (
    review['unicode'] === 'covered' &&
    !UNICODE_EXEMPT.has(component) &&
    !hasAxisStory(component, 'unicode')
  ) {
    console.warn(`${component}: unicode covered without /unicode story`)
  }
}

const docsDir = `${root}/docs/content/docs`
let docs = ''
const pageBodies = new Map<string, string>()
for await (const name of new Bun.Glob('**/*.mdx').scan({ cwd: docsDir })) {
  const body = await Bun.file(`${docsDir}/${name}`).text()
  docs += `${body}\n`
  pageBodies.set(name, body)
}

// Docs must never embed SVG catalog snapshots — Ghostty TerminalPreview only.
if (docs.includes('component-previews/') || /!\[[^\]]*\]\([^)]*\.svg\)/.test(docs)) {
  throw new Error(
    'docs content still embeds SVG / component-previews; use TerminalPreview frame packs only',
  )
}

const embeddedStories = new Set<string>()
for (const m of docs.matchAll(/<TerminalPreview\s+[^>]*story="([^"]+)"/g)) {
  embeddedStories.add(m[1]!)
}
for (const m of docs.matchAll(/<TerminalPreview\s+[^>]*story='([^']+)'/g)) {
  embeddedStories.add(m[1]!)
}

for (const component of publicComponents) {
  const pagePath = `${docsDir}/components/${componentSlug(component)}.mdx`
  if (!(await Bun.file(pagePath).exists())) {
    throw new Error(`missing component reference page ${pagePath}`)
  }
  const body = await Bun.file(pagePath).text()
  const previewCount = (body.match(/<TerminalPreview\b/g) ?? []).length
  if (previewCount !== 1) {
    throw new Error(
      `${componentSlug(component)}.mdx must embed exactly one TerminalPreview (found ${previewCount})`,
    )
  }
  // Stories table replaces multi-preview galleries.
  if (!/\| *Story\b/i.test(body)) {
    throw new Error(`${componentSlug(component)}.mdx missing Stories table`)
  }
  // Every lookbook story id for this component must appear in docs (table or text).
  for (const story of stories) {
    if (story.component !== component) continue
    if (!body.includes(`\`${story.id}\``)) {
      throw new Error(`missing docs for story ${story.id} on ${component} page`)
    }
  }
}

// Every component MDX (including hand-authored) is one-focus + Stories table.
for await (const name of new Bun.Glob('*.mdx').scan({ cwd: `${docsDir}/components` })) {
  const body = await Bun.file(`${docsDir}/components/${name}`).text()
  const previewCount = (body.match(/<TerminalPreview\b/g) ?? []).length
  if (previewCount !== 1) {
    throw new Error(`components/${name} must embed exactly one TerminalPreview (found ${previewCount})`)
  }
  if (!/\| *Story\b/i.test(body)) {
    throw new Error(`components/${name} missing Stories table (| Story | …)`)
  }
}

// Handbook widget pages: exactly one Ghostty TerminalPreview (no SVG galleries).
for await (const name of new Bun.Glob('*.mdx').scan({ cwd: `${docsDir}/handbook` })) {
  if (name === 'index.mdx') continue
  const body = await Bun.file(`${docsDir}/handbook/${name}`).text()
  const n = (body.match(/<TerminalPreview\b/g) ?? []).length
  if (n !== 1) {
    throw new Error(
      `handbook/${name} must embed exactly one TerminalPreview (found ${n}); Ghostty-only, never SVG`,
    )
  }
}

// Other non-component docs: at most one TerminalPreview per mdx file (one focus).
for (const [name, body] of pageBodies) {
  if (name.startsWith('components/') || name.startsWith('handbook/')) continue
  const n = (body.match(/<TerminalPreview\b/g) ?? []).length
  if (n > 1) {
    throw new Error(`${name} embeds ${n} TerminalPreview surfaces; max one focus per page`)
  }
}

// Frame packs required only for embedded stories (not entire catalog).
for (const storyId of embeddedStories) {
  if (!ids.has(storyId)) {
    throw new Error(`TerminalPreview references unknown story ${storyId}`)
  }
  const slug = storyId.replaceAll('/', '-')
  const pack = `${root}/docs/public/preview-frames/${slug}`
  const manifestPath = `${pack}/manifest.json`
  if (!(await Bun.file(manifestPath).exists())) {
    throw new Error(`missing Ghostty frame pack ${manifestPath}`)
  }
  const manifest = JSON.parse(await Bun.file(manifestPath).text()) as {
    sizes?: string[]
    defaultSize?: string
  }
  const size = manifest.defaultSize ?? manifest.sizes?.[0] ?? '40x8'
  const step0 = `${pack}/${size}/0.json`
  if (!(await Bun.file(step0).exists()) || !(await Bun.file(step0).size)) {
    throw new Error(`missing Ghostty frame ${step0}`)
  }
}

console.log(
  `catalog covers ${publicComponents.size} public components; ${embeddedStories.size} Ghostty embeds (one focus each); ${stories.length} stories documented`,
)
