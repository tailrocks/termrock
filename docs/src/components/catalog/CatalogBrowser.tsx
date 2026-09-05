'use client'

import { useMemo, type ReactElement } from 'react'
import { CatalogCard } from '@/components/catalog/CatalogCard'
import { useCatalogFilters, type CatalogFilters } from '@/components/catalog/CatalogFilters'
import { scoreCatalogDocument } from '@/components/catalog/search'

export type CatalogGroup = Readonly<{
  id: string
  title: string
  description: string
  order: number
}>

export type CatalogBrowseEntry = Readonly<{
  id: string
  group: string
  href: string
  title: string
  purpose: string
  story: string
  eyebrow: string
  tags: readonly string[]
  aliases: readonly string[]
}>

export type CatalogBrowserProps = Readonly<{
  entries: readonly CatalogBrowseEntry[]
  groups: readonly CatalogGroup[]
  noun: 'component' | 'pattern'
  placeholder: string
}>

export function CatalogBrowser({
  entries,
  groups,
  noun,
  placeholder,
}: CatalogBrowserProps): ReactElement {
  const { filters, updateFilters } = useCatalogFilters()

  const orderedGroups = useMemo(
    () => [...groups].sort((left, right) => left.order - right.order),
    [groups],
  )
  const ranked = useMemo(
    () => entries
      .filter((entry) => !filters.family || entry.group === filters.family)
      .map((entry) => ({ entry, score: scoreCatalogDocument(entry, filters.query) }))
      .filter((candidate) => candidate.score >= 0)
      .sort(
        (left, right) =>
          right.score - left.score || left.entry.title.localeCompare(right.entry.title),
      ),
    [entries, filters],
  )

  const change = (next: CatalogFilters): void => updateFilters(next)

  return (
    <div className="not-prose grid gap-8">
      <div className="grid gap-3 md:grid-cols-[minmax(0,1fr)_15rem]">
        <label className="grid gap-2">
          <span className="text-xs font-semibold uppercase tracking-[0.12em] text-[var(--tr-text-muted)]">
            Find a {noun}
          </span>
          <input
            type="search"
            value={filters.query}
            onChange={(event) => change({ ...filters, query: event.target.value })}
            placeholder={placeholder}
            className="min-h-11 rounded-[var(--tr-radius-sm)] border border-[var(--tr-stroke-strong)] bg-[var(--tr-surface-raised)] px-3 text-[var(--tr-text-strong)] placeholder:text-[var(--tr-text-muted)]"
          />
        </label>
        <label className="grid gap-2">
          <span className="text-xs font-semibold uppercase tracking-[0.12em] text-[var(--tr-text-muted)]">
            Family
          </span>
          <select
            value={filters.family}
            onChange={(event) => change({ ...filters, family: event.target.value })}
            className="min-h-11 rounded-[var(--tr-radius-sm)] border border-[var(--tr-stroke-strong)] bg-[var(--tr-surface-raised)] px-3 text-[var(--tr-text-strong)]"
          >
            <option value="">All families</option>
            {orderedGroups.map((group) => (
              <option key={group.id} value={group.id}>{group.title}</option>
            ))}
          </select>
        </label>
      </div>

      <p
        aria-live="polite"
        className="m-0 text-sm text-[var(--tr-text-muted)]"
        data-catalog-count={`${ranked.length}/${entries.length}`}
      >
        {ranked.length} of {entries.length} {noun}s
      </p>

      {ranked.length === 0 ? (
        <div className="rounded-[var(--tr-radius-md)] border border-[var(--tr-stroke-subtle)] bg-[var(--tr-surface-muted)] p-6">
          <strong className="text-[var(--tr-text-strong)]">No {noun}s found</strong>
          <p className="mb-0 mt-2 text-sm text-[var(--tr-text-muted)]">
            Clear the search or choose another family.
          </p>
        </div>
      ) : (
        orderedGroups.map((group) => {
          const groupEntries = ranked
            .filter(({ entry }) => entry.group === group.id)
            .map(({ entry }) => entry)
          if (groupEntries.length === 0) return null
          return (
            <section key={group.id} className="grid gap-4" aria-labelledby={`catalog-${noun}-${group.id}`}>
              <div className="grid gap-1">
                <h2 id={`catalog-${noun}-${group.id}`} className="m-0 text-xl text-[var(--tr-text-strong)]">
                  {group.title}
                </h2>
                <p className="m-0 text-sm text-[var(--tr-text-muted)]">{group.description}</p>
              </div>
              <div className="grid gap-4 [grid-template-columns:repeat(auto-fill,minmax(14rem,1fr))]">
                {groupEntries.map((entry) => (
                  <CatalogCard
                    key={entry.id}
                    href={entry.href}
                    title={entry.title}
                    purpose={entry.purpose}
                    story={entry.story}
                    eyebrow={entry.eyebrow}
                    posterSize={noun}
                  />
                ))}
              </div>
            </section>
          )
        })
      )}
    </div>
  )
}
