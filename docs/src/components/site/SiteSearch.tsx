'use client'

import {
  createContext,
  useContext,
  useMemo,
  useState,
  type ReactNode,
} from 'react'
import type { ReactSortedResult } from 'fumadocs-core/search'
import {
  SearchDialog,
  SearchDialogClose,
  SearchDialogContent,
  SearchDialogFooter,
  SearchDialogHeader,
  SearchDialogIcon,
  SearchDialogInput,
  SearchDialogList,
  SearchDialogListItem,
  SearchDialogOverlay,
  type SharedProps,
} from 'fumadocs-ui/components/dialog/search'
import { scoreCatalogDocument } from '@/components/catalog/search'
import { catalogComponents, catalogPatterns } from '@/generated/catalog'

type SearchEntry = Readonly<{
  id: string
  label: string
  url: string
  group: 'Component' | 'Pattern' | 'Guide'
  purpose: string
  tags: readonly string[]
  aliases: readonly string[]
}>

export type GuideSearchEntry = SearchEntry & Readonly<{ group: 'Guide' }>

const GuideSearchContext = createContext<readonly GuideSearchEntry[]>([])

export function GuideSearchProvider({
  entries,
  children,
}: Readonly<{ entries: readonly GuideSearchEntry[]; children: ReactNode }>) {
  return <GuideSearchContext value={entries}>{children}</GuideSearchContext>
}

const COMPONENT_ENTRIES: readonly SearchEntry[] = catalogComponents.map((component) => ({
  id: `component-${component.slug}`,
  label: component.id,
  url: component.href,
  group: 'Component',
  purpose: component.purpose,
  tags: [...component.tags, component.family, component.renderKind, component.story],
  aliases: component.aliases,
}))

const PATTERN_ENTRIES: readonly SearchEntry[] = catalogPatterns.map((pattern) => ({
  id: `pattern-${pattern.slug}`,
  label: pattern.id,
  url: pattern.href,
  group: 'Pattern',
  purpose: pattern.purpose,
  tags: [...pattern.tags, ...pattern.uses, ...pattern.supportingTypes, pattern.story],
  aliases: pattern.aliases,
}))

function toResult(entry: SearchEntry): ReactSortedResult {
  return {
    id: entry.id,
    type: 'page',
    url: entry.url,
    content: entry.label,
    breadcrumbs: [entry.group],
  }
}

function renderPlainText(value: string): ReactNode {
  return <span>{value}</span>
}

export function SiteSearch({ open, onOpenChange }: SharedProps) {
  const [query, setQuery] = useState('')
  const guides = useContext(GuideSearchContext)
  const entries = useMemo<readonly SearchEntry[]>(
    () => [...guides, ...COMPONENT_ENTRIES, ...PATTERN_ENTRIES],
    [guides],
  )
  const initialEntries = useMemo<readonly SearchEntry[]>(
    () =>
      [
        guides[0],
        COMPONENT_ENTRIES.find((entry) => entry.id === 'component-button'),
        COMPONENT_ENTRIES.find((entry) => entry.id === 'component-data-table'),
        PATTERN_ENTRIES.find((entry) => entry.id === 'pattern-agent-workbench'),
      ].filter((entry): entry is SearchEntry => entry !== undefined),
    [guides],
  )
  const results = useMemo<ReactSortedResult[]>(() => {
    if (!query.trim()) return initialEntries.map(toResult)

    return entries.map((entry) => ({
      entry,
      score: scoreCatalogDocument({
        title: entry.label,
        purpose: entry.purpose,
        tags: entry.tags,
        aliases: entry.aliases,
      }, query),
    }))
      .filter((candidate) => candidate.score >= 0)
      .sort(
        (left, right) =>
          right.score - left.score || left.entry.label.localeCompare(right.entry.label),
      )
      .slice(0, 24)
      .map(({ entry }) => toResult(entry))
  }, [entries, initialEntries, query])

  const changeOpen = (nextOpen: boolean) => {
    if (!nextOpen) setQuery('')
    onOpenChange(nextOpen)
  }

  return (
    <SearchDialog
      open={open}
      onOpenChange={changeOpen}
      search={query}
      onSearchChange={setQuery}
    >
      <SearchDialogOverlay />
      <SearchDialogContent>
        <SearchDialogHeader>
          <SearchDialogIcon />
          <SearchDialogInput
            aria-label="Search TermRock"
            placeholder="Search components, patterns, and guides"
          />
          <SearchDialogClose />
        </SearchDialogHeader>
        <SearchDialogList
          items={results}
          Item={({ item, onClick }) => (
            <SearchDialogListItem
              item={item}
              onClick={onClick}
              renderMarkdown={renderPlainText}
            />
          )}
        />
        <SearchDialogFooter>
          <span className="site-search__hint">Components, application patterns, and every guide</span>
        </SearchDialogFooter>
      </SearchDialogContent>
    </SearchDialog>
  )
}
