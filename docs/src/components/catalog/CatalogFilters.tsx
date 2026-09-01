'use client'

import { createContext, useContext, type ReactElement, type ReactNode } from 'react'

export type CatalogFilters = Readonly<{
  query: string
  family: string
}>

type CatalogFiltersContextValue = Readonly<{
  filters: CatalogFilters
  updateFilters: (filters: CatalogFilters) => void
}>

const CatalogFiltersContext = createContext<CatalogFiltersContextValue | null>(null)

export function CatalogFiltersProvider({
  children,
  filters,
  updateFilters,
}: CatalogFiltersContextValue & Readonly<{ children: ReactNode }>): ReactElement {
  return (
    <CatalogFiltersContext value={{ filters, updateFilters }}>
      {children}
    </CatalogFiltersContext>
  )
}

export function useCatalogFilters(): CatalogFiltersContextValue {
  const value = useContext(CatalogFiltersContext)
  if (!value) throw new Error('Catalog filters require docs route ownership')
  return value
}
