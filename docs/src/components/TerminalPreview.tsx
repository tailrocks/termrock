'use client'

import {
  useCallback,
  useEffect,
  useId,
  useMemo,
  useRef,
  useState,
  type CSSProperties,
  type KeyboardEvent as ReactKeyboardEvent,
  type PointerEvent as ReactPointerEvent,
} from 'react'

/** Cell payload from termrock-lookbook frame export (truecolor RGB). */
export type FrameCell = {
  ch: string
  fg: [number, number, number]
  bg: [number, number, number]
  bold?: boolean
  dim?: boolean
  underline?: boolean
  reversed?: boolean
}

export type TerminalFrame = {
  story_id: string
  title: string
  component: string
  cols: number
  rows: number
  story_cols: number
  story_rows: number
  cells: FrameCell[]
  interactive: boolean
  theme: string
}

export type FrameManifest = {
  storyId: string
  title: string
  component: string
  interactive: boolean
  steps: number
  cellWidthPx: number
  cellHeightPx: number
}

const DEFAULT_CELL_W = 9
const DEFAULT_CELL_H = 18

function rgb(c: [number, number, number]): string {
  return `rgb(${c[0]}, ${c[1]}, ${c[2]})`
}

function basePath(): string {
  if (typeof document === 'undefined') return ''
  // Vite base for GitHub Pages is /termrock/
  const base = import.meta.env.BASE_URL ?? '/'
  return base.endsWith('/') ? base.slice(0, -1) : base
}

async function loadJson<T>(url: string): Promise<T> {
  const res = await fetch(url)
  if (!res.ok) throw new Error(`failed to load ${url}: ${res.status}`)
  return res.json() as Promise<T>
}

function paintCanvas(
  canvas: HTMLCanvasElement,
  frame: TerminalFrame,
  cellW: number,
  cellH: number,
  dpr: number,
) {
  const w = frame.cols * cellW
  const h = frame.rows * cellH
  canvas.width = Math.max(1, Math.floor(w * dpr))
  canvas.height = Math.max(1, Math.floor(h * dpr))
  canvas.style.width = `${w}px`
  canvas.style.height = `${h}px`
  const ctx = canvas.getContext('2d')
  if (!ctx) return
  ctx.setTransform(dpr, 0, 0, dpr, 0, 0)
  ctx.imageSmoothingEnabled = false
  // Ghostty-class black stage
  ctx.fillStyle = '#0a0a0a'
  ctx.fillRect(0, 0, w, h)
  const fontSize = Math.max(11, Math.floor(cellH * 0.78))
  const baseline = Math.floor(cellH * 0.78)
  for (let y = 0; y < frame.rows; y++) {
    for (let x = 0; x < frame.cols; x++) {
      const cell = frame.cells[y * frame.cols + x]
      if (!cell) continue
      const px = x * cellW
      const py = y * cellH
      if (cell.bg[0] || cell.bg[1] || cell.bg[2]) {
        ctx.fillStyle = rgb(cell.bg)
        ctx.fillRect(px, py, cellW, cellH)
      }
      const ch = cell.ch
      if (!ch || ch === ' ' || ch === '\u00a0') continue
      const weight = cell.bold ? '600' : '400'
      ctx.font = `${weight} ${fontSize}px "JetBrains Mono", "SF Mono", "Cascadia Mono", ui-monospace, Menlo, Consolas, monospace`
      ctx.fillStyle = rgb(cell.fg)
      ctx.textBaseline = 'alphabetic'
      ctx.fillText(ch, px + 0.5, py + baseline)
      if (cell.underline) {
        ctx.strokeStyle = rgb(cell.fg)
        ctx.beginPath()
        ctx.moveTo(px, py + cellH - 2)
        ctx.lineTo(px + cellW, py + cellH - 2)
        ctx.stroke()
      }
    }
  }
}

export type TerminalPreviewProps = {
  /** Lookbook story id, e.g. list/selection */
  story: string
  /** Optional caption under the chrome */
  caption?: string
  /** Max CSS height for the host (responsive width fills container) */
  maxHeight?: number
  /** Prefer interactive pack when available */
  interactive?: boolean
}

/**
 * Ghostty-class interactive terminal surface for TermRock docs.
 * Loads truecolor cell frames from `public/preview-frames/{story}/`
 * (exported by `termrock-lookbook export-frames`).
 */
export function TerminalPreview({
  story,
  caption,
  maxHeight = 420,
  interactive = true,
}: TerminalPreviewProps) {
  const canvasRef = useRef<HTMLCanvasElement>(null)
  const hostRef = useRef<HTMLDivElement>(null)
  const [frame, setFrame] = useState<TerminalFrame | null>(null)
  const [manifest, setManifest] = useState<FrameManifest | null>(null)
  const [step, setStep] = useState(0)
  const [error, setError] = useState<string | null>(null)
  const [focused, setFocused] = useState(false)
  const labelId = useId()

  const slug = useMemo(() => story.replaceAll('/', '-'), [story])
  const packBase = `${basePath()}/preview-frames/${slug}`

  const cellW = manifest?.cellWidthPx ?? DEFAULT_CELL_W
  const cellH = manifest?.cellHeightPx ?? DEFAULT_CELL_H

  const loadStep = useCallback(
    async (n: number) => {
      try {
        const f = await loadJson<TerminalFrame>(`${packBase}/${n}.json`)
        setFrame(f)
        setStep(n)
        setError(null)
      } catch (e) {
        setError(e instanceof Error ? e.message : String(e))
      }
    },
    [packBase],
  )

  useEffect(() => {
    let cancelled = false
    ;(async () => {
      try {
        const m = await loadJson<FrameManifest>(`${packBase}/manifest.json`)
        if (cancelled) return
        setManifest(m)
        await loadStep(0)
      } catch (e) {
        if (!cancelled) setError(e instanceof Error ? e.message : String(e))
      }
    })()
    return () => {
      cancelled = true
    }
  }, [packBase, loadStep])

  useEffect(() => {
    const canvas = canvasRef.current
    if (!canvas || !frame) return
    const dpr = typeof window !== 'undefined' ? window.devicePixelRatio || 1 : 1
    paintCanvas(canvas, frame, cellW, cellH, dpr)
  }, [frame, cellW, cellH])

  // Responsive: when host resizes, we keep native cell metrics (Ghostty-like fixed
  // cell size) and letterbox; true row/col remap needs re-export. CSS scales host.
  useEffect(() => {
    const host = hostRef.current
    if (!host || !frame) return
    const ro = new ResizeObserver(() => {
      // Re-paint for DPR / CSS size; cell grid stays story-native for fidelity.
      const canvas = canvasRef.current
      if (!canvas) return
      const dpr = window.devicePixelRatio || 1
      paintCanvas(canvas, frame, cellW, cellH, dpr)
    })
    ro.observe(host)
    return () => ro.disconnect()
  }, [frame, cellW, cellH])

  const onKeyDown = (e: ReactKeyboardEvent) => {
    if (!interactive || !manifest?.interactive) return
    const map: Record<string, string> = {
      ArrowDown: 'ArrowDown',
      ArrowUp: 'ArrowUp',
      ArrowLeft: 'ArrowLeft',
      ArrowRight: 'ArrowRight',
      Enter: 'Enter',
      Escape: 'Escape',
      Tab: 'Tab',
      ' ': ' ',
    }
    const key = map[e.key] ?? (e.key.length === 1 ? e.key : null)
    if (!key) return
    e.preventDefault()
    e.stopPropagation()
    // Step graph: Down advances selection frames; Up goes back; Home resets.
    if (key === 'ArrowDown' || key === 'j') {
      const next = Math.min((manifest.steps || 1) - 1, step + 1)
      void loadStep(next)
      return
    }
    if (key === 'ArrowUp' || key === 'k') {
      const next = Math.max(0, step - 1)
      void loadStep(next)
      return
    }
    if (key === 'Home' || key === 'Escape') {
      void loadStep(0)
    }
  }

  const onPointerDown = (e: ReactPointerEvent) => {
    hostRef.current?.focus()
    setFocused(true)
    if (!interactive || !manifest?.interactive) return
    // Click advances selection — feels like activating rows in a list TUI.
    if (e.button === 0) {
      const next = Math.min((manifest.steps || 1) - 1, step + 1)
      void loadStep(next)
    }
  }

  const chrome: CSSProperties = {
    borderRadius: 10,
    border: focused ? '1px solid #39ff14' : '1px solid #273027',
    background:
      'linear-gradient(180deg, #1a1c1a 0%, #0c0e0c 12%, #050705 100%)',
    boxShadow: focused
      ? '0 0 0 1px rgba(57,255,20,0.25), 0 12px 40px rgba(0,0,0,0.55)'
      : '0 12px 40px rgba(0,0,0,0.45)',
    overflow: 'hidden',
    maxWidth: '100%',
  }

  const titleBar: CSSProperties = {
    display: 'flex',
    alignItems: 'center',
    gap: 8,
    padding: '8px 12px',
    borderBottom: '1px solid #1e241e',
    fontFamily:
      '"JetBrains Mono", "SF Mono", "Cascadia Mono", ui-monospace, monospace',
    fontSize: 12,
    color: '#8a9a8a',
    userSelect: 'none',
  }

  return (
    <figure
      className="not-prose my-6"
      data-termrock-preview={story}
      data-preview-step={step}
      data-preview-interactive={manifest?.interactive ? 'true' : 'false'}
    >
      <div
        ref={hostRef}
        role="application"
        tabIndex={0}
        aria-labelledby={labelId}
        aria-label={`Interactive terminal preview: ${story}`}
        onKeyDown={onKeyDown}
        onPointerDown={onPointerDown}
        onFocus={() => setFocused(true)}
        onBlur={() => setFocused(false)}
        style={{ ...chrome, outline: 'none', cursor: 'text' }}
      >
        <div style={titleBar} id={labelId}>
          <span style={{ display: 'flex', gap: 6 }} aria-hidden>
            <span
              style={{
                width: 10,
                height: 10,
                borderRadius: 999,
                background: '#ff5f57',
              }}
            />
            <span
              style={{
                width: 10,
                height: 10,
                borderRadius: 999,
                background: '#febc2e',
              }}
            />
            <span
              style={{
                width: 10,
                height: 10,
                borderRadius: 999,
                background: '#28c840',
              }}
            />
          </span>
          <span style={{ color: '#c8d6c8' }}>Ghostty · TermRock</span>
          <span style={{ opacity: 0.7 }}>—</span>
          <span style={{ color: '#39ff14' }}>{story}</span>
          {manifest?.interactive ? (
            <span style={{ marginLeft: 'auto', fontSize: 11, opacity: 0.8 }}>
              ↑↓ select · click · Esc reset
            </span>
          ) : (
            <span style={{ marginLeft: 'auto', fontSize: 11, opacity: 0.65 }}>
              truecolor · responsive host
            </span>
          )}
        </div>
        <div
          style={{
            padding: 10,
            background: '#0a0a0a',
            overflow: 'auto',
            maxHeight,
            display: 'flex',
            justifyContent: 'center',
          }}
        >
          {error ? (
            <pre
              style={{
                color: '#ff5e7a',
                fontSize: 12,
                whiteSpace: 'pre-wrap',
                margin: 0,
              }}
            >
              {error}
              {'\n'}
              Run:{' '}
              {`cargo run -p termrock-lookbook -- export-frames --out docs/public/preview-frames --story ${story}`}
            </pre>
          ) : (
            <canvas
              ref={canvasRef}
              style={{
                display: 'block',
                imageRendering: 'pixelated',
                maxWidth: '100%',
                height: 'auto',
              }}
            />
          )}
        </div>
      </div>
      {caption ? (
        <figcaption
          style={{
            marginTop: 8,
            fontSize: 13,
            color: '#8a9a8a',
            fontFamily:
              '"JetBrains Mono", "SF Mono", ui-monospace, monospace',
          }}
        >
          {caption}
        </figcaption>
      ) : null}
    </figure>
  )
}

export default TerminalPreview
