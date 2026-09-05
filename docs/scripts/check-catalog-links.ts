import { join, relative } from 'node:path'
import { loadCatalog } from './catalog-data'

const root = join(import.meta.dir, '..', '..')
const docs = join(root, 'docs', 'content', 'docs')
const catalog = await loadCatalog()
const componentSlugs = new Set(catalog.components.map((entry) => entry.slug))
const patternSlugs = new Set(catalog.patterns.map((entry) => entry.slug))
const errors: string[] = []

function lineAt(source: string, index: number): number {
  return source.slice(0, index).split('\n').length
}

for await (const name of new Bun.Glob('**/*.mdx').scan({ cwd: docs })) {
  const path = join(docs, name)
  const source = await Bun.file(path).text()
  for (const match of source.matchAll(/\/docs\/(components|patterns)\/([a-z0-9-]+)/gu)) {
    const kind = match[1]
    const slug = match[2]!
    const slugs = kind === 'components' ? componentSlugs : patternSlugs
    if (!slugs.has(slug)) errors.push(`${relative(root, path)}:${lineAt(source, match.index!)}: broken ${kind} link ${slug}`)
  }
  const collection = name.startsWith('components/')
    ? componentSlugs
    : name.startsWith('patterns/')
      ? patternSlugs
      : undefined
  if (!collection) continue
  for (const match of source.matchAll(/\]\(\.\/([a-z0-9-]+)\)/gu)) {
    const slug = match[1]!
    if (!collection.has(slug)) errors.push(`${relative(root, path)}:${lineAt(source, match.index!)}: broken relative catalog link ${slug}`)
  }
}

if (errors.length) throw new Error(`catalog link failures:\n${errors.join('\n')}`)
console.log(`catalog links: ${catalog.components.length} component and ${catalog.patterns.length} pattern targets`)
