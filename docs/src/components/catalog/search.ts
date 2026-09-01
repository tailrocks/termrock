export type CatalogSearchDocument = Readonly<{
  title: string
  purpose: string
  tags: readonly string[]
  aliases: readonly string[]
}>

export function normalizeCatalogQuery(value: string): string {
  return value.normalize('NFKD').toLocaleLowerCase().replaceAll(/\p{Diacritic}/gu, '')
}

export function scoreCatalogDocument(entry: CatalogSearchDocument, query: string): number {
  const terms = normalizeCatalogQuery(query).split(/\s+/u).filter(Boolean)
  if (terms.length === 0) return 0

  const title = normalizeCatalogQuery(entry.title)
  const purpose = normalizeCatalogQuery(entry.purpose)
  const tags = entry.tags.map(normalizeCatalogQuery)
  const aliases = entry.aliases.map(normalizeCatalogQuery)
  const searchable = [title, purpose, ...tags, ...aliases].join(' ')
  if (!terms.every((term) => searchable.includes(term))) return -1

  return terms.reduce((total, term) => {
    if (title === term) return total + 32
    if (title.startsWith(term)) return total + 20
    if (aliases.some((alias) => alias === term)) return total + 16
    if (aliases.some((alias) => alias.startsWith(term))) return total + 12
    if (tags.some((tag) => tag === term)) return total + 8
    if (title.includes(term)) return total + 6
    if (purpose.includes(term)) return total + 2
    return total + 1
  }, 0)
}
