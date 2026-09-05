import type { ReactElement } from 'react'
import { CatalogBrowser, type CatalogBrowseEntry } from '@/components/catalog/CatalogBrowser'
import { catalogPatternFamilies, catalogPatterns } from '@/generated/catalog'

const entries: readonly CatalogBrowseEntry[] = catalogPatterns.map((pattern) => ({
  id: pattern.id,
  group: pattern.family,
  href: pattern.href,
  title: pattern.id,
  purpose: pattern.purpose,
  story: pattern.story,
  eyebrow: pattern.family,
  tags: [...pattern.tags, ...pattern.uses],
  aliases: pattern.aliases,
}))

export function PatternGallery(): ReactElement {
  return (
    <CatalogBrowser
      entries={entries}
      groups={catalogPatternFamilies}
      noun="pattern"
      placeholder="Search by workflow, purpose, component, or alias…"
    />
  )
}
