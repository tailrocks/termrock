import type { ReactElement } from 'react'
import { CatalogCard } from '@/components/catalog/CatalogCard'
import { catalogPatterns } from '@/generated/catalog'

function includesComponent(uses: readonly string[], component: string): boolean {
  return uses.includes(component)
}

export function SeenInApplications({ component }: Readonly<{ component: string }>): ReactElement {
  const patterns = catalogPatterns.filter((pattern) => includesComponent(pattern.uses, component))

  if (patterns.length === 0) {
    return (
      <p className="text-[var(--tr-text-muted)]">
        No catalog pattern uses this component yet. This is an example-coverage gap, not a component limitation.{' '}
        <a href="/docs/patterns">Browse all patterns</a>.
      </p>
    )
  }

  return (
    <div className="not-prose grid gap-4 [grid-template-columns:repeat(auto-fill,minmax(14rem,1fr))]">
      {patterns.map((pattern) => (
        <CatalogCard
          key={pattern.id}
          href={pattern.href}
          title={pattern.id}
          purpose={pattern.purpose}
          story={pattern.story}
          eyebrow={pattern.family}
          posterSize="pattern"
        />
      ))}
    </div>
  )
}
