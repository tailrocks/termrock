import {
  catalogComponentFamilies,
  catalogComponents,
  catalogEntries,
  catalogPatternFamilies,
  catalogPatterns,
} from '@/generated/catalog'

export type DocCatalogEntry = Readonly<{
  id: string
  entryKind: 'component' | 'pattern'
  publicUi: string | null
  renderKind: 'widget' | 'paint' | 'layout' | 'behavior' | null
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

export type RelatedDoc = Readonly<{
  href: string
  title: string
  relationship: string
}>

const entries: readonly DocCatalogEntry[] = catalogEntries
const components: readonly DocCatalogEntry[] = catalogComponents
const patterns: readonly DocCatalogEntry[] = catalogPatterns
const familyTitles = new Map<string, string>(
  [...catalogComponentFamilies, ...catalogPatternFamilies].map((family) => [
    family.id,
    family.title,
  ]),
)

export function catalogEntryById(id: string): DocCatalogEntry | null {
  return entries.find((entry) => entry.id === id) ?? null
}

export function familyTitle(entry: DocCatalogEntry): string {
  return familyTitles.get(entry.family) ?? entry.family
}

export function relatedDocs(entry: DocCatalogEntry): readonly RelatedDoc[] {
  const primary =
    entry.entryKind === 'component'
      ? patterns
          .filter((pattern) => pattern.uses.includes(entry.id))
          .map((pattern) => ({
            href: pattern.href,
            title: pattern.title,
            relationship: 'Uses this component',
          }))
      : components
          .filter((component) => entry.uses.includes(component.id))
          .map((component) => ({
            href: component.href,
            title: component.title,
            relationship: 'Building block',
          }))

  const peers = (entry.entryKind === 'component' ? components : patterns)
    .filter((candidate) => candidate.id !== entry.id && candidate.family === entry.family)
    .map((candidate) => ({
      href: candidate.href,
      title: candidate.title,
      relationship: `Also in ${familyTitle(entry)}`,
    }))

  const seen = new Set<string>()
  return [...primary, ...peers].filter((candidate) => {
    if (seen.has(candidate.href) || seen.size >= 6) return false
    seen.add(candidate.href)
    return true
  })
}
