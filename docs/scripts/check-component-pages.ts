import { join } from 'node:path'
import { loadCatalog } from './catalog-data'
import { assertEditorialPage, assertSharedDetailLayout } from './doc-page-structure'

const root = join(import.meta.dir, '..', '..')
const catalog = await loadCatalog()
await assertSharedDetailLayout()

for (const component of catalog.components) {
  const path = join(root, 'docs', 'content', 'docs', 'components', `${component.slug}.mdx`)
  const body = await Bun.file(path).text()
  assertEditorialPage(body, component.id)
}

console.log(`component docs: ${catalog.components.length} shared-layout editorial pages`)
