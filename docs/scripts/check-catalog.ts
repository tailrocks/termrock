import { componentSlug } from './component-doc-utils'

const root = `${import.meta.dir}/../..`
const result = Bun.spawnSync(
  ['cargo', 'run', '-q', '-p', 'termrock-lookbook', '--', 'list', '--format', 'json'],
  { cwd: root },
)
if (result.exitCode !== 0) throw new Error(result.stderr.toString())
const stories = JSON.parse(result.stdout.toString()) as Array<{ id: string; component: string }>
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

const storyComponents = new Set(stories.map((story) => story.component))
const missingStories = [...publicComponents].filter((component) => !storyComponents.has(component))
if (missingStories.length) throw new Error(`public components without stories: ${missingStories.join(', ')}`)
const unknownStories = [...storyComponents].filter((component) => !publicComponents.has(component))
if (unknownStories.length) throw new Error(`stories without public components: ${unknownStories.join(', ')}`)

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
const contractValues = new Set(['covered', 'caller-owned', 'not-applicable'])
for (const [component, review] of Object.entries(contracts)) {
  for (const contract of contractNames) {
    const value = review[contract]
    if (!value || !contractValues.has(value)) {
      throw new Error(`${component} has no valid ${contract} contract`)
    }
  }
}

// Base stories for these widgets already demonstrate the axis by construction;
// all other `covered` claims require a named axis story.
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
  return stories.some((story) => story.component === component && story.id.split('/').includes(axis))
}
for (const [component, review] of Object.entries(contracts)) {
  if (
    review['narrowTerminal'] === 'covered' &&
    !NARROW_EXEMPT.has(component) &&
    !hasAxisStory(component, 'narrow')
  ) {
    throw new Error(`${component} claims narrowTerminal covered without a /narrow story`)
  }
  if (
    review['unicode'] === 'covered' &&
    !UNICODE_EXEMPT.has(component) &&
    !hasAxisStory(component, 'unicode')
  ) {
    throw new Error(`${component} claims unicode covered without a /unicode story`)
  }
}

const docsDir = `${root}/docs/content/docs`
let docs = ''
for await (const name of new Bun.Glob('**/*.mdx').scan({ cwd: docsDir })) {
  docs += `${await Bun.file(`${docsDir}/${name}`).text()}\n`
}
for (const component of publicComponents) {
  const page = `${docsDir}/components/${componentSlug(component)}.mdx`
  if (!(await Bun.file(page).exists())) throw new Error(`missing component reference page ${page}`)
}
for (const story of stories) {
  if (!docs.includes(`\`${story.id}\``)) throw new Error(`missing docs for story ${story.id}`)
  const preview = `${root}/docs/public/component-previews/${story.id.replaceAll('/', '-')}.svg`
  if (!Bun.file(preview).size) throw new Error(`missing preview ${preview}`)
}
console.log(`catalog covers ${publicComponents.size} public components with ${stories.length} stories`)
