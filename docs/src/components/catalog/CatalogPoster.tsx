'use client'

import { useEffect, useRef, useState, type ReactElement } from 'react'
import { paintCanvas } from '@/components/preview/painter'
import { loadPosterFrame } from '@/components/preview/poster'

type PosterState = 'idle' | 'loading' | 'ready' | 'failed'

export type CatalogPosterProps = Readonly<{
  story: string
  label: string
  cellWidth?: number
  cellHeight?: number
}>

export function CatalogPoster({
  story,
  label,
  cellWidth = 4,
  cellHeight = 8,
}: CatalogPosterProps): ReactElement {
  const hostRef = useRef<HTMLDivElement>(null)
  const canvasRef = useRef<HTMLCanvasElement>(null)
  const [state, setState] = useState<PosterState>('idle')

  useEffect(() => {
    const host = hostRef.current
    if (!host) return

    const abort = new AbortController()
    const observer = new IntersectionObserver(
      (entries) => {
        if (!entries.some((entry) => entry.isIntersecting)) return
        observer.disconnect()
        setState('loading')
        void loadPosterFrame(story, abort.signal)
          .then((frame) => {
            if (!canvasRef.current) return
            paintCanvas(canvasRef.current, frame, cellWidth, cellHeight, 1)
            setState('ready')
          })
          .catch((error: unknown) => {
            if (error instanceof DOMException && error.name === 'AbortError') return
            setState('failed')
          })
      },
      { rootMargin: '240px 0px' },
    )
    observer.observe(host)

    return () => {
      observer.disconnect()
      abort.abort()
    }
  }, [cellHeight, cellWidth, story])

  return (
    <div ref={hostRef} className="relative size-full min-h-24 overflow-hidden bg-[var(--tr-graphite-950)]">
      <canvas
        ref={canvasRef}
        className="block h-auto min-w-full"
        role="img"
        aria-label={`${label} terminal preview`}
      />
      {state === 'idle' || state === 'loading' ? (
        <span className="absolute inset-0 grid place-items-center text-xs text-[var(--tr-text-muted)]">
          Loading preview
        </span>
      ) : null}
      {state === 'failed' ? (
        <span className="absolute inset-0 grid place-items-center text-xs text-[var(--tr-text-muted)]">
          Preview unavailable
        </span>
      ) : null}
    </div>
  )
}
