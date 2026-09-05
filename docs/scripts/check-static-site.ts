import { catalogEntries } from '../src/generated/catalog'
import { TERMROCK_INSTALL_COMMAND } from '../src/lib/install'

const output = `${import.meta.dir}/../dist/client`
const required = [
  'index.html',
  '404.html',
  'docs/index.html',
  'docs/components/index.html',
  'docs/interaction/index.html',
  'docs/advanced-composition/index.html',
]

for (const relative of required) {
  if (!(await Bun.file(`${output}/${relative}`).exists())) {
    throw new Error(`static docs output missing ${relative}`)
  }
}

const retiredSlug = ['interaction', 'notes'].join('-')
if (await Bun.file(`${output}/docs/${retiredSlug}/index.html`).exists()) {
  throw new Error(`static docs output still publishes retired ${retiredSlug} route`)
}

const rootHtml = await Bun.file(`${output}/index.html`).text()
const rootMarkers = [
  '<main id="main-content"',
  'Build terminal software that feels finished.',
  'data-termrock-preview="agent-workbench/basic"',
] as const
const missingRootMarkers = rootMarkers.filter((marker) => !rootHtml.includes(marker))
if (missingRootMarkers.length > 0) {
  throw new Error(`static root is missing ${missingRootMarkers.join(', ')}`)
}
const fallbackHtml = await Bun.file(`${output}/404.html`).text()
if (rootHtml === fallbackHtml) {
  throw new Error('static root is a copied SPA fallback shell')
}

for (const entry of catalogEntries) {
  const page = `${output}${entry.href}/index.html`
  if (!(await Bun.file(page).exists())) {
    throw new Error(`static docs output missing ${entry.href}/index.html`)
  }
  const html = await Bun.file(page).text()
  const required = [
    entry.story,
    'class="doc-detail"',
    'data-termrock-preview',
    'What the mounted story proves',
    'Minimal implementation',
    'API, tokens, accessibility',
    TERMROCK_INSTALL_COMMAND,
  ] as const
  const missing = required.filter((marker) => !html.includes(marker))
  if (missing.length > 0) {
    throw new Error(`${entry.id}: static detail page missing ${missing.join(', ')}`)
  }
}

const components = await Bun.file(`${output}/docs/components/index.html`).text()
if (!components.includes('href="/docs/components/action-bar"')) {
  throw new Error('components overview link does not use the custom-domain root')
}
if (components.includes('/termrock/')) {
  throw new Error('legacy GitHub project Pages base path remains in static output')
}

console.log(`static docs smoke: shell + ${catalogEntries.length} detail pages OK`)
