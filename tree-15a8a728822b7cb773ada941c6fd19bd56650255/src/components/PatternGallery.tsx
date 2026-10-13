'use client'

import { useEffect, useMemo, useRef, useState } from 'react'
import catalog from '../../api/pattern-catalog.json'
import { paintCanvas, type TerminalFrame } from '@/components/TerminalPreview'

type Pattern = (typeof catalog)[number]

const GROUPS: Array<{ key: Pattern['classification']; title: string; note: string }> = [
  {
    key: 'application',
    title: 'Applications',
    note: 'Complete multi-surface samples with persistent navigation and overlays.',
  },
  {
    key: 'composite',
    title: 'Composites',
    note: 'Product-shaped recipes assembled from public building blocks.',
  },
  {
    key: 'layout-helper',
    title: 'Layout helpers',
    note: 'Geometry helpers demonstrated inside their canonical parent application.',
  },
]

function PatternPoster({ demo, label }: { demo: string; label: string }) {
  const ref = useRef<HTMLCanvasElement>(null)
  useEffect(() => {
    let cancelled = false
    const slug = demo.replaceAll('/', '-')
    void fetch(`/preview-posters/${slug}.json`)
      .then((response) => {
        if (!response.ok) throw new Error(`poster ${response.status}`)
        return response.json() as Promise<TerminalFrame>
      })
      .then((frame) => {
        if (cancelled || !ref.current) return
        paintCanvas(ref.current, frame, 5, 10, 1)
        ref.current.style.width = '100%'
        ref.current.style.height = 'auto'
      })
      .catch(() => undefined)
    return () => {
      cancelled = true
    }
  }, [demo])
  return <canvas ref={ref} role="img" aria-label={`${label} terminal preview`} />
}

function card(pattern: Pattern) {
  return (
    <a
      key={pattern.slug}
      href={`/docs/patterns/${pattern.slug}`}
      style={{
        display: 'grid',
        gridTemplateRows: '150px auto',
        overflow: 'hidden',
        border: '1px solid #263126',
        borderRadius: 10,
        color: 'inherit',
        textDecoration: 'none',
        background: '#090c09',
      }}
    >
      <div style={{ overflow: 'hidden', background: '#050705' }}>
        <PatternPoster demo={pattern.demo} label={pattern.pattern} />
      </div>
      <span style={{ display: 'grid', gap: 7, padding: 12 }}>
        <strong style={{ color: '#d8ffd8' }}>{pattern.pattern}</strong>
        <span style={{ color: '#91a091', fontSize: 13 }}>
          {pattern.buildingBlocks.slice(0, 4).join(' · ')}
        </span>
        <code style={{ color: '#39ff14', fontSize: 11 }}>{pattern.demo}</code>
      </span>
    </a>
  )
}

export function PatternGallery() {
  const [query, setQuery] = useState('')
  const filtered = useMemo(() => {
    const needle = query.trim().toLowerCase()
    if (!needle) return catalog
    return catalog.filter((pattern) =>
      [pattern.pattern, pattern.slug, pattern.demo, ...pattern.buildingBlocks]
        .join(' ')
        .toLowerCase()
        .includes(needle),
    )
  }, [query])

  return (
    <div className="not-prose" style={{ display: 'grid', gap: 24 }}>
      <label style={{ display: 'grid', gap: 7, maxWidth: 520 }}>
        <span style={{ color: '#a8b8a8', fontSize: 13 }}>Find a pattern or building block</span>
        <input
          type="search"
          value={query}
          onChange={(event) => setQuery(event.target.value)}
          placeholder="Search workbench, dialog, table…"
          style={{
            border: '1px solid #334033',
            borderRadius: 8,
            background: '#080b08',
            color: '#d8e8d8',
            padding: '10px 12px',
            font: 'inherit',
          }}
        />
      </label>
      <span aria-live="polite" style={{ color: '#91a091', fontSize: 13 }}>
        {filtered.length} of {catalog.length} patterns
      </span>
      {GROUPS.map((group) => {
        const entries = filtered.filter((pattern) => pattern.classification === group.key)
        if (!entries.length) return null
        return (
          <section key={group.key} style={{ display: 'grid', gap: 12 }}>
            <div>
              <h2 style={{ margin: 0, color: '#d8ffd8', fontSize: 20 }}>{group.title}</h2>
              <p style={{ margin: '4px 0 0', color: '#91a091', fontSize: 13 }}>{group.note}</p>
            </div>
            <div
              style={{
                display: 'grid',
                gridTemplateColumns: 'repeat(auto-fit, minmax(240px, 1fr))',
                gap: 14,
              }}
            >
              {entries.map(card)}
            </div>
          </section>
        )
      })}
    </div>
  )
}
