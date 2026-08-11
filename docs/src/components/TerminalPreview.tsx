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
  type WheelEvent as ReactWheelEvent,
} from 'react'
import {
  PREVIEW_MONO_STACK,
  adjacentSteps,
  allSteps,
  baselineForCell,
  clampStep,
  fontSizeForCell,
  glyphCellSpan,
  glyphDrawX,
  isLoadStillCurrent,
  stepDeltaFromWheel,
} from '@/components/preview-metrics'

export {
  PREVIEW_MONO_STACK,
  adjacentSteps,
  allSteps,
  baselineForCell,
  clampStep,
  fontSizeForCell,
  glyphCellSpan,
  glyphDrawX,
  isLoadStillCurrent,
  stepDeltaFromWheel,
} from '@/components/preview-metrics'

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
  /** Exported size keys e.g. `40x8` (story cols×rows). */
  sizes?: string[]
  defaultSize?: string
  padCells?: number
  /** Preferred nav key used when baking interactor steps. */
  stepKey?: string | null
  /** Multi-scene composite tour story ids (optional). */
  tour?: string[] | null
}

const DEFAULT_CELL_W = 9
const DEFAULT_CELL_H = 18
const PAD = 1
const DEFAULT_SIZES = ['28x6', '40x8', '56x12', '72x16', '80x24']
/** Ghostty-class surface fill under cells (matches PREVIEW_CARD charcoal). */
const SURFACE_BG = '#0a0a0a'

/** Mirrors `frame::cols_for_css_width` (total terminal cols). */
export function colsForCssWidth(cssPx: number, cellW = DEFAULT_CELL_W): number {
  const cell = Math.max(1, cellW)
  return Math.max(8, Math.floor(cssPx / cell))
}

/** Mirrors `frame::rows_for_css_height` (total terminal rows). */
export function rowsForCssHeight(cssPx: number, cellH = DEFAULT_CELL_H): number {
  const cell = Math.max(1, cellH)
  return Math.max(4, Math.floor(cssPx / cell))
}

/** Mirrors `frame::story_size_for_css_host` (inner story size). */
export function storySizeForCssHost(
  cssW: number,
  cssH: number,
  cellW = DEFAULT_CELL_W,
  cellH = DEFAULT_CELL_H,
): { storyCols: number; storyRows: number } {
  const totalC = colsForCssWidth(cssW, cellW)
  const totalR = rowsForCssHeight(cssH, cellH)
  return {
    storyCols: Math.max(8, totalC - PAD * 2),
    storyRows: Math.max(4, totalR - PAD * 2),
  }
}

/** Mirrors `frame::pick_size_key`. */
export function pickSizeKey(
  wantCols: number,
  wantRows: number,
  sizes: string[] = DEFAULT_SIZES,
): string {
  let best = sizes[0] ?? '40x8'
  let bestScore = Number.POSITIVE_INFINITY
  for (const key of sizes) {
    const [cs, rs] = key.split('x').map((n) => Number(n))
    if (!cs || !rs) continue
    const dc = cs - wantCols
    const dr = rs - wantRows
    const score = Math.abs(dc) * 2 + Math.abs(dr)
    if (score < bestScore) {
      bestScore = score
      best = key
    }
  }
  return best
}

function rgb(c: [number, number, number]): string {
  return `rgb(${c[0]}, ${c[1]}, ${c[2]})`
}

/** Dim/faint: bake remaining dim flag on already-encoded RGB for host safety. */
function paintFg(cell: FrameCell): string {
  if (!cell.dim) return rgb(cell.fg)
  return `rgb(${Math.round(cell.fg[0] * 0.7)},${Math.round(cell.fg[1] * 0.7)},${Math.round(cell.fg[2] * 0.7)})`
}

function basePath(): string {
  if (typeof document === 'undefined') return ''
  const base = import.meta.env.BASE_URL ?? '/'
  return base.endsWith('/') ? base.slice(0, -1) : base
}

async function loadJson<T>(url: string): Promise<T> {
  const res = await fetch(url)
  if (!res.ok) throw new Error(`failed to load ${url}: ${res.status}`)
  return res.json() as Promise<T>
}

/**
 * Ghostty-class cell paint: full 24-bit RGB bg+fg, monospaced metrics, no smoothing.
 * Always fills every cell background (including pure black) so truecolor is preserved.
 * Glyphs are centered in-cell when the mono advance fits (terminal-grid feel).
 */
export function paintCanvas(
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
  // Fixed CSS pixel size — do not stretch with max-width (keeps integer cell metrics).
  canvas.style.width = `${w}px`
  canvas.style.height = `${h}px`
  canvas.style.maxWidth = 'none'
  const ctx = canvas.getContext('2d')
  if (!ctx) return
  ctx.setTransform(dpr, 0, 0, dpr, 0, 0)
  ctx.imageSmoothingEnabled = false
  ctx.fillStyle = SURFACE_BG
  ctx.fillRect(0, 0, w, h)
  const fontSize = fontSizeForCell(cellH)
  const baseline = baselineForCell(cellH)
  for (let y = 0; y < frame.rows; y++) {
    for (let x = 0; x < frame.cols; x++) {
      const cell = frame.cells[y * frame.cols + x]
      if (!cell) continue
      const px = x * cellW
      const py = y * cellH
      // Always paint truecolor background (including [0,0,0]).
      ctx.fillStyle = rgb(cell.bg)
      ctx.fillRect(px, py, cellW, cellH)
      const ch = cell.ch
      if (!ch || ch === ' ' || ch === '\u00a0') continue
      const weight = cell.bold ? '600' : '400'
      ctx.font = `${weight} ${fontSize}px ${PREVIEW_MONO_STACK}`
      ctx.fillStyle = paintFg(cell)
      ctx.textBaseline = 'alphabetic'
      const tw = ctx.measureText(ch).width
      const span = glyphCellSpan(tw, cellW)
      // Double-width glyphs: paint across two cells when the next cell is empty.
      if (span === 2 && x + 1 < frame.cols) {
        const next = frame.cells[y * frame.cols + x + 1]
        if (next && (!next.ch || next.ch === ' ' || next.ch === '\u00a0')) {
          ctx.fillStyle = rgb(cell.bg)
          ctx.fillRect(px + cellW, py, cellW, cellH)
          ctx.fillStyle = paintFg(cell)
          ctx.fillText(ch, px + 0.5, py + baseline)
          if (cell.underline) {
            ctx.strokeStyle = paintFg(cell)
            ctx.lineWidth = 1
            ctx.beginPath()
            ctx.moveTo(px, py + cellH - 2)
            ctx.lineTo(px + cellW * 2, py + cellH - 2)
            ctx.stroke()
          }
          x += 1 // skip continuation cell
          continue
        }
      }
      ctx.fillText(ch, glyphDrawX(px, cellW, tw), py + baseline)
      if (cell.underline) {
        ctx.strokeStyle = paintFg(cell)
        ctx.lineWidth = 1
        ctx.beginPath()
        ctx.moveTo(px, py + cellH - 2)
        ctx.lineTo(px + cellW, py + cellH - 2)
        ctx.stroke()
      }
    }
  }
}

export type TerminalPreviewProps = {
  story: string
  caption?: string
  maxHeight?: number
  interactive?: boolean
}

/**
 * Ghostty-class interactive terminal surface for TermRock docs.
 * Host ResizeObserver remaps story cols×rows via the same helpers as
 * `termrock-lookbook` export (`storySizeForCssHost` / `pickSizeKey`) and
 * loads the matching size pack — not letterbox-only.
 */
export function TerminalPreview({
  story,
  caption,
  maxHeight = 420,
  interactive = true,
}: TerminalPreviewProps) {
  const canvasRef = useRef<HTMLCanvasElement>(null)
  const hostRef = useRef<HTMLDivElement>(null)
  const stageRef = useRef<HTMLDivElement>(null)
  const [frame, setFrame] = useState<TerminalFrame | null>(null)
  const [manifest, setManifest] = useState<FrameManifest | null>(null)
  const [step, setStep] = useState(0)
  const [sizeKey, setSizeKey] = useState('40x8')
  const [error, setError] = useState<string | null>(null)
  const [focused, setFocused] = useState(false)
  const [caretOn, setCaretOn] = useState(true)
  const [stepPulse, setStepPulse] = useState(0)
  const labelId = useId()
  const stepRef = useRef(0)
  const sizeKeyRef = useRef(sizeKey)
  const maxStepRef = useRef(0)
  const frameCacheRef = useRef<Map<string, TerminalFrame>>(new Map())
  const loadGenRef = useRef(0)
  stepRef.current = step
  sizeKeyRef.current = sizeKey

  const slug = useMemo(() => story.replaceAll('/', '-'), [story])
  const packBase = `${basePath()}/preview-frames/${slug}`

  const cellW = manifest?.cellWidthPx ?? DEFAULT_CELL_W
  const cellH = manifest?.cellHeightPx ?? DEFAULT_CELL_H
  const sizes = manifest?.sizes?.length ? manifest.sizes : DEFAULT_SIZES
  const canInteract = Boolean(interactive && manifest?.interactive)
  const maxStep = Math.max(0, (manifest?.steps || 1) - 1)
  maxStepRef.current = maxStep

  const cacheKey = (size: string, n: number) => `${size}:${n}`

  const fetchFrame = useCallback(
    async (size: string, n: number): Promise<TerminalFrame> => {
      const key = cacheKey(size, n)
      const hit = frameCacheRef.current.get(key)
      if (hit) return hit
      let f: TerminalFrame
      try {
        f = await loadJson<TerminalFrame>(`${packBase}/${size}/${n}.json`)
      } catch {
        f = await loadJson<TerminalFrame>(`${packBase}/${n}.json`)
      }
      frameCacheRef.current.set(key, f)
      return f
    },
    [packBase],
  )

  const prefetchSizePack = useCallback(
    (size: string) => {
      // Warm every step in this size so tours/arrows feel instantaneous.
      for (const s of allSteps(maxStepRef.current)) {
        void fetchFrame(size, s).catch(() => {
          /* optional pack hole */
        })
      }
    },
    [fetchFrame],
  )

  const prefetchAdjacent = useCallback(
    (size: string, n: number) => {
      for (const s of adjacentSteps(n, maxStepRef.current)) {
        void fetchFrame(size, s).catch(() => {
          /* optional adjacent pack */
        })
      }
    },
    [fetchFrame],
  )

  const loadFrame = useCallback(
    async (size: string, n: number) => {
      const reqId = ++loadGenRef.current
      try {
        const next = clampStep(n, maxStepRef.current)
        const f = await fetchFrame(size, next)
        // Drop stale responses so rapid nav never rewinds the visible step.
        if (!isLoadStillCurrent(reqId, loadGenRef.current)) return
        setFrame(f)
        setStep(next)
        setSizeKey(size)
        setError(null)
        setStepPulse((p) => p + 1)
        prefetchAdjacent(size, next)
        prefetchSizePack(size)
      } catch (e) {
        if (!isLoadStillCurrent(reqId, loadGenRef.current)) return
        setError(e instanceof Error ? e.message : String(e))
      }
    },
    [fetchFrame, prefetchAdjacent, prefetchSizePack],
  )

  // Drop cache + invalidate in-flight loads when pack story changes.
  useEffect(() => {
    frameCacheRef.current = new Map()
    loadGenRef.current += 1
  }, [packBase])

  useEffect(() => {
    let cancelled = false
    ;(async () => {
      try {
        const m = await loadJson<FrameManifest>(`${packBase}/manifest.json`)
        if (cancelled) return
        setManifest(m)
        const initial = m.defaultSize ?? m.sizes?.[1] ?? '40x8'
        await loadFrame(initial, 0)
      } catch (e) {
        if (!cancelled) setError(e instanceof Error ? e.message : String(e))
      }
    })()
    return () => {
      cancelled = true
    }
  }, [packBase, loadFrame])

  // Paint after JetBrains Mono is ready so advances match Ghostty mono metrics
  // (first paint may use fallback mono; fonts.ready triggers a true repaint).
  useEffect(() => {
    const canvas = canvasRef.current
    if (!canvas || !frame) return
    let cancelled = false
    const paint = () => {
      if (cancelled) return
      const dpr = typeof window !== 'undefined' ? window.devicePixelRatio || 1 : 1
      paintCanvas(canvas, frame, cellW, cellH, dpr)
    }
    paint()
    const fonts = typeof document !== 'undefined' ? document.fonts : undefined
    if (fonts?.ready) {
      void fonts.ready.then(paint)
    }
    if (fonts?.load) {
      void fonts
        .load(`${fontSizeForCell(cellH)}px JetBrains Mono`)
        .then(paint)
        .catch(() => {
          /* optional webfont */
        })
    }
    return () => {
      cancelled = true
    }
  }, [frame, cellW, cellH])

  // Ghostty-style caret blink while focused (visual only; selection is frame-backed).
  useEffect(() => {
    if (!focused || !canInteract) {
      setCaretOn(true)
      return
    }
    const id = window.setInterval(() => setCaretOn((v) => !v), 530)
    return () => window.clearInterval(id)
  }, [focused, canInteract])

  // Real remap: host CSS size → story cols/rows → nearest exported pack → load.
  // sizeKeyRef avoids re-binding ResizeObserver on every size change.
  useEffect(() => {
    const stage = stageRef.current
    if (!stage || !manifest) return
    let timer: ReturnType<typeof setTimeout> | null = null
    const apply = (cssW: number, cssH: number) => {
      const { storyCols, storyRows } = storySizeForCssHost(cssW, cssH, cellW, cellH)
      const nextKey = pickSizeKey(storyCols, storyRows, sizes)
      stage.dataset['hostCssW'] = String(Math.round(cssW))
      stage.dataset['hostCssH'] = String(Math.round(cssH))
      stage.dataset['wantStoryCols'] = String(storyCols)
      stage.dataset['wantStoryRows'] = String(storyRows)
      stage.dataset['sizeKey'] = nextKey
      if (nextKey !== sizeKeyRef.current) {
        void loadFrame(nextKey, stepRef.current)
      }
    }
    const ro = new ResizeObserver((entries) => {
      const entry = entries[0]
      if (!entry) return
      const { width, height } = entry.contentRect
      if (timer) clearTimeout(timer)
      timer = setTimeout(() => apply(width, height), 50)
    })
    ro.observe(stage)
    const rect = stage.getBoundingClientRect()
    apply(rect.width, rect.height)
    return () => {
      ro.disconnect()
      if (timer) clearTimeout(timer)
    }
  }, [manifest, cellW, cellH, sizes, loadFrame])

  const goStep = useCallback(
    (n: number) => {
      void loadFrame(sizeKey, clampStep(n, maxStep))
    },
    [loadFrame, maxStep, sizeKey],
  )

  const onWheel = (e: ReactWheelEvent) => {
    if (!canInteract) return
    const delta = stepDeltaFromWheel(e.deltaY)
    if (!delta) return
    e.preventDefault()
    e.stopPropagation()
    hostRef.current?.focus()
    setFocused(true)
    goStep(stepRef.current + delta)
  }

  /** Shared nav mapping for React onKeyDown + window capture (agent-browser reliability). */
  const handleNavKey = useCallback(
    (rawKey: string): boolean => {
      if (!canInteract) return false
      const map: Record<string, string> = {
        ArrowDown: 'ArrowDown',
        ArrowUp: 'ArrowUp',
        ArrowLeft: 'ArrowLeft',
        ArrowRight: 'ArrowRight',
        Enter: 'Enter',
        Escape: 'Escape',
        Tab: 'Tab',
        Home: 'Home',
        End: 'End',
        PageDown: 'PageDown',
        PageUp: 'PageUp',
        Backspace: 'Backspace',
        ' ': ' ',
      }
      const key = map[rawKey] ?? (rawKey.length === 1 ? rawKey : null)
      if (!key) return false
      if (
        key === 'ArrowDown' ||
        key === 'ArrowRight' ||
        key === 'j' ||
        key === 'l' ||
        key === 'PageDown' ||
        key === ' '
      ) {
        goStep(stepRef.current + 1)
        return true
      }
      if (
        key === 'ArrowUp' ||
        key === 'ArrowLeft' ||
        key === 'k' ||
        key === 'h' ||
        key === 'PageUp'
      ) {
        goStep(stepRef.current - 1)
        return true
      }
      if (key === 'Home' || key === 'Escape') {
        goStep(0)
        return true
      }
      if (key === 'End') {
        goStep(maxStep)
        return true
      }
      return false
    },
    [canInteract, goStep, maxStep],
  )

  const onKeyDown = (e: ReactKeyboardEvent) => {
    if (handleNavKey(e.key)) {
      e.preventDefault()
      e.stopPropagation()
    }
  }

  // Capture phase on window while focused — keys still reach the TUI host when
  // focus is slightly wrong (common under headless agent-browser injection).
  useEffect(() => {
    if (!focused || !canInteract) return
    const onWin = (e: KeyboardEvent) => {
      if (handleNavKey(e.key)) {
        e.preventDefault()
        e.stopPropagation()
      }
    }
    window.addEventListener('keydown', onWin, true)
    return () => window.removeEventListener('keydown', onWin, true)
  }, [focused, canInteract, handleNavKey])

  const onPointerDown = (e: ReactPointerEvent) => {
    hostRef.current?.focus()
    setFocused(true)
    if (!canInteract) return
    if (e.button !== 0) return
    const canvas = canvasRef.current
    if (canvas && frame) {
      const rect = canvas.getBoundingClientRect()
      const scaleY = rect.height > 0 ? canvas.clientHeight / rect.height : 1
      const scaleX = rect.width > 0 ? canvas.clientWidth / rect.width : 1
      const yCss = (e.clientY - rect.top) * scaleY
      const xCss = (e.clientX - rect.left) * scaleX
      const row = Math.floor(yCss / cellH)
      const col = Math.floor(xCss / cellW)
      const bodyRow = Math.max(0, row - PAD)
      const bodyCol = Math.max(0, col - PAD)
      // Prefer row mapping; for short tab strips also allow col-based step.
      let next = Math.min(maxStep, bodyRow)
      if (frame.rows <= 4 && frame.cols > 8) {
        const colStep = Math.floor((bodyCol / Math.max(1, frame.story_cols)) * (maxStep + 1))
        next = Math.min(maxStep, Math.max(0, colStep))
      }
      goStep(next)
      return
    }
    goStep(step + 1)
  }

  const chrome: CSSProperties = {
    borderRadius: 12,
    border: focused ? '1px solid #39ff14' : '1px solid #1e261e',
    background:
      'linear-gradient(180deg, #222622 0%, #121512 14%, #070907 100%)',
    boxShadow: focused
      ? '0 0 0 1px rgba(57,255,20,0.28), 0 0 24px rgba(57,255,20,0.08), 0 16px 48px rgba(0,0,0,0.6)'
      : '0 1px 0 rgba(255,255,255,0.04) inset, 0 16px 48px rgba(0,0,0,0.5)',
    overflow: 'hidden',
    maxWidth: '100%',
    filter: focused ? 'none' : 'brightness(0.92)',
    transition: 'border-color 120ms ease, box-shadow 120ms ease, filter 120ms ease',
  }

  const titleBar: CSSProperties = {
    display: 'flex',
    alignItems: 'center',
    gap: 8,
    padding: '9px 12px',
    borderBottom: '1px solid #1a201a',
    background: focused
      ? 'linear-gradient(180deg, #1c221c 0%, #121612 100%)'
      : 'linear-gradient(180deg, #181b18 0%, #0e100e 100%)',
    fontFamily: PREVIEW_MONO_STACK,
    fontSize: 12,
    color: focused ? '#9aaa9a' : '#6a766a',
    userSelect: 'none',
  }

  const statusBar: CSSProperties = {
    display: 'flex',
    alignItems: 'center',
    gap: 10,
    padding: '5px 12px',
    borderTop: '1px solid #1a201a',
    fontFamily: PREVIEW_MONO_STACK,
    fontSize: 11,
    color: '#6a7a6a',
    background:
      stepPulse > 0
        ? 'linear-gradient(90deg, rgba(57,255,20,0.12), #0c0e0c 40%)'
        : '#0c0e0c',
    transition: 'background 160ms ease',
    userSelect: 'none',
  }

  const gridLabel = frame
    ? `${frame.cols}×${frame.rows} · story ${frame.story_cols}×${frame.story_rows} · ${sizeKey}`
    : sizeKey

  const tour = manifest?.tour
  const sceneId =
    tour && tour.length > 0
      ? (tour[Math.min(step, tour.length - 1)] ?? story)
      : story
  const sceneTitle = frame?.title ?? sceneId

  const stepLabel = canInteract
    ? tour && tour.length > 1
      ? `scene ${step + 1}/${maxStep + 1}`
      : `step ${step + 1}/${maxStep + 1}`
    : 'static'

  const hint = canInteract
    ? tour && tour.length > 1
      ? '↑↓/wheel cycle states · j/k · click · Home/End'
      : '↑↓/←→ · wheel · j/k · click · Home/End'
    : 'read-only frame'

  return (
    <figure
      className="not-prose my-6"
      data-termrock-preview={story}
      data-preview-step={step}
      data-preview-size={sizeKey}
      data-preview-cols={frame?.cols ?? ''}
      data-preview-rows={frame?.rows ?? ''}
      data-preview-interactive={canInteract ? 'true' : 'false'}
      data-preview-focused={focused ? 'true' : 'false'}
      data-preview-truecolor="rgb24"
      data-preview-scene={sceneId}
      data-preview-tour={tour && tour.length > 1 ? 'true' : 'false'}
      data-preview-pulse={String(stepPulse)}
    >
      <div
        ref={hostRef}
        role="application"
        tabIndex={0}
        aria-labelledby={labelId}
        aria-label={`Interactive terminal preview: ${story}`}
        onKeyDown={onKeyDown}
        onPointerDown={onPointerDown}
        onWheel={onWheel}
        onFocus={() => setFocused(true)}
        onBlur={() => setFocused(false)}
        style={{ ...chrome, outline: 'none', cursor: canInteract ? 'text' : 'default' }}
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
          <span style={{ color: '#c8d6c8' }}>Ghostty</span>
          <span style={{ opacity: 0.55 }}>·</span>
          <span style={{ color: '#8a9a8a' }}>TermRock</span>
          <span style={{ opacity: 0.55 }}>—</span>
          <span style={{ color: '#39ff14' }}>{story}</span>
          {tour && tour.length > 1 ? (
            <>
              <span style={{ opacity: 0.55 }}>·</span>
              <span style={{ color: '#c8e6c8' }} title={sceneId}>
                {sceneTitle}
              </span>
            </>
          ) : null}
          <span
            aria-hidden
            style={{
              marginLeft: 4,
              width: 7,
              height: 12,
              background: focused && caretOn ? '#39ff14' : 'transparent',
              boxShadow: focused && caretOn ? '0 0 6px #39ff14' : undefined,
              display: 'inline-block',
              verticalAlign: 'middle',
            }}
          />
          <span style={{ marginLeft: 'auto', fontSize: 11, opacity: 0.85 }}>
            {gridLabel}
          </span>
        </div>
        <div
          ref={stageRef}
          data-termrock-stage="1"
          style={{
            padding: 12,
            background:
              'radial-gradient(120% 80% at 50% 0%, #121412 0%, #0a0a0a 55%, #050505 100%)',
            boxShadow: 'inset 0 0 0 1px #151915, inset 0 12px 28px rgba(0,0,0,0.45)',
            overflow: 'auto',
            maxHeight,
            minHeight: 120,
            width: '100%',
            display: 'flex',
            justifyContent: 'center',
            alignItems: 'flex-start',
            boxSizing: 'border-box',
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
              data-termrock-canvas="1"
              data-canvas-cols={frame?.cols ?? ''}
              data-canvas-rows={frame?.rows ?? ''}
              style={{
                display: 'block',
                imageRendering: 'pixelated',
                // Crisp integer cells — stage scrolls when host is narrower.
                flexShrink: 0,
              }}
            />
          )}
        </div>
        <div style={statusBar} data-termrock-status="1">
          <span style={{ color: canInteract ? '#39ff14' : '#6a7a6a' }}>
            {canInteract
              ? tour && tour.length > 1
                ? '● state tour'
                : '● live pack'
              : '○ snapshot'}
          </span>
          <span>{stepLabel}</span>
          {tour && tour.length > 1 ? (
            <span style={{ color: '#8aba8a' }} data-preview-scene-label={sceneId}>
              {sceneId}
            </span>
          ) : null}
          <span style={{ opacity: 0.8 }}>{sizeKey}</span>
          {frame ? (
            <span style={{ opacity: 0.75 }}>
              RGB24 · {frame.cols}×{frame.rows}
            </span>
          ) : null}
          <span style={{ marginLeft: 'auto', opacity: 0.85 }}>{hint}</span>
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
