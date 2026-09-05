import type { ReactElement } from 'react'
import { CatalogBrowser, type CatalogBrowseEntry } from '@/components/catalog/CatalogBrowser'
import { catalogComponentFamilies, catalogComponents } from '@/generated/catalog'

const entries: readonly CatalogBrowseEntry[] = catalogComponents.map((component) => ({
  id: component.id,
  group: component.family,
  href: component.href,
  title: component.id,
  purpose: component.purpose,
  story: component.story,
  eyebrow: `${component.family} · ${component.renderKind}`,
  tags: component.tags,
  aliases: component.aliases,
}))

export function ComponentGallery(): ReactElement {
  return (
    <CatalogBrowser
      entries={entries}
      groups={catalogComponentFamilies}
      noun="component"
      placeholder="Search by name, purpose, tag, or alias…"
    />
  )
}
