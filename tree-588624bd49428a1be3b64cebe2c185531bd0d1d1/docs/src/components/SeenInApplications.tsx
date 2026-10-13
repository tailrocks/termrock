'use client'

import { useEffect, useMemo, useRef } from 'react'
import catalog from '../../api/pattern-catalog.json'
import { paintCanvas, type TerminalFrame } from '@/components/TerminalPreview'

type Pattern = (typeof catalog)[number]

/**
 * Where a component is actually used.
 *
 * A component page could tell you what a widget does but not where anyone
 * puts it, so every reader had to guess whether a primitive was load-bearing
 * or decorative. The answer already exists in `pattern-catalog.json`
 * (`buildingBlocks`), which stays the single source: this reads it, nobody
 * hand-maintains a list (plans/018 Step 1).
 */
function usedBy(component: string): Pattern[] {
  return catalog.filter((pattern) => pattern.buildingBlocks.includes(component))
}

function Poster({ demo, label }: { demo: string; label: string }) {
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
        paintCanvas(ref.current, frame, 4, 8, 1)
        ref.current.style.width = '100%'
        ref.current.style.height = 'auto'
      })
      .catch(() => undefined)
    return () => {
      cancelled = true
    }
  }, [demo])
  return <canvas ref={ref} role="img" aria-label={`${label} preview`} />
}

export function SeenInApplications({ component }: { component: string }) {
  const patterns = useMemo(() => usedBy(component), [component])

  if (!patterns.length) {
    return (
      <p style={{ color: '#91a091' }}>
        Not yet composed in a shipped example. That is a coverage gap in the examples, not a
        limitation of the component — see <a href="/docs/patterns">the pattern catalog</a>.
      </p>
    )
  }

  return (
    <div
      className="not-prose"
      style={{
        display: 'grid',
        gap: 12,
        gridTemplateColumns: 'repeat(auto-fill, minmax(210px, 1fr))',
      }}
    >
      {patterns.map((pattern) => (
        <a
          key={pattern.slug}
          href={`/docs/patterns/${pattern.slug}`}
          style={{
            display: 'grid',
            gridTemplateRows: '96px auto',
            overflow: 'hidden',
            border: '1px solid #263126',
            borderRadius: 10,
            color: 'inherit',
            textDecoration: 'none',
            background: '#090c09',
          }}
        >
          <div style={{ overflow: 'hidden', background: '#050705' }}>
            <Poster demo={pattern.demo} label={pattern.pattern} />
          </div>
          <span style={{ display: 'grid', gap: 4, padding: 10 }}>
            <strong style={{ color: '#d8ffd8' }}>{pattern.pattern}</strong>
            <span style={{ color: '#91a091', fontSize: 12 }}>{pattern.classification}</span>
          </span>
        </a>
      ))}
    </div>
  )
}
