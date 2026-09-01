import type { ReactElement } from 'react'
import { CatalogPoster } from '@/components/catalog/CatalogPoster'

export type CatalogCardProps = Readonly<{
  href: string
  title: string
  purpose: string
  story: string
  eyebrow: string
  posterSize?: 'component' | 'pattern'
}>

export function CatalogCard({
  href,
  title,
  purpose,
  story,
  eyebrow,
  posterSize = 'component',
}: CatalogCardProps): ReactElement {
  const posterHeight = posterSize === 'pattern' ? 'h-40' : 'h-28'
  const cellWidth = posterSize === 'pattern' ? 5 : 4
  const cellHeight = posterSize === 'pattern' ? 10 : 8

  return (
    <a
      href={href}
      className="group grid min-w-0 overflow-hidden rounded-[var(--tr-radius-md)] border border-[var(--tr-stroke-subtle)] bg-[var(--tr-surface-raised)] text-inherit no-underline transition-[border-color,transform] duration-[var(--tr-duration-fast)] ease-[var(--tr-ease-enter)] hover:-translate-y-0.5 hover:border-[var(--tr-stroke-strong)]"
    >
      <div className={`${posterHeight} overflow-hidden`}>
        <CatalogPoster
          story={story}
          label={title}
          cellWidth={cellWidth}
          cellHeight={cellHeight}
        />
      </div>
      <span className="grid gap-2 p-4">
        <span className="font-mono text-[0.6875rem] uppercase tracking-[0.12em] text-[var(--tr-text-muted)]">
          {eyebrow}
        </span>
        <strong className="text-base text-[var(--tr-text-strong)]">{title}</strong>
        <span className="line-clamp-2 text-sm leading-6 text-[var(--tr-text-muted)]">
          {purpose}
        </span>
      </span>
    </a>
  )
}
