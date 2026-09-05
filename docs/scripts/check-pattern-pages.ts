import { join } from 'node:path'
import { loadCatalog } from './catalog-data'
import { assertEditorialPage, assertSharedDetailLayout } from './doc-page-structure'

const root = join(import.meta.dir, '..', '..')
const catalog = await loadCatalog()
await assertSharedDetailLayout()

for (const pattern of catalog.patterns) {
  const path = join(root, 'docs', 'content', 'docs', 'patterns', `${pattern.slug}.mdx`)
  const body = await Bun.file(path).text()
  assertEditorialPage(body, pattern.id)
}

console.log(`pattern docs: ${catalog.patterns.length} shared-layout editorial pages`)
