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
  CURSOR_BLOCK_RGB,
  PREVIEW_MONO_STACK,
  adjacentSteps,
  allSteps,
  applyNavStepAction,
  baselineForCell,
  boldFontWeight,
  boxStrokeForGlyph,
  boxStrokeGeometry,
  cellAtPointer,
  clampStep,
  cursorCellForStep,
  fontSizeForCell,
  formatCellProbe,
  glyphCellSpan,
  glyphDrawX,
  inferCursorFromFrame,
  isBoxOrBlockGlyph,
  isLoadStillCurrent,
  combinedHostViewport,
  hostViewportSize,
  materialViewportChange,
  paintDpr,
  scrollThumbMetrics,
  shouldAcceptKeyEvent,
  stepDeltaFromNavKey,
  stepDeltaFromWheel,
  stepFromPointer,
  stepFromScrollRatio,
  type KeyStrokeStamp,
} from '@/components/preview-metrics'

export {
  CURSOR_BLOCK_RGB,
  PREVIEW_MONO_STACK,
  adjacentSteps,
  allSteps,
  applyNavStepAction,
  baselineForCell,
  boldFontWeight,
  boxStrokeForGlyph,
  boxStrokeGeometry,
  cellAtPointer,
  clampStep,
  combinedHostViewport,
  cursorCellForStep,
  fontSizeForCell,
  formatCellProbe,
  formatRgbHex,
  glyphCellSpan,
  glyphDrawX,
  hostViewportSize,
  inferCursorFromFrame,
  isBoxOrBlockGlyph,
  isLoadStillCurrent,
  materialViewportChange,
  paintDpr,
  scrollThumbMetrics,
  shouldAcceptKeyEvent,
  stepDeltaFromNavKey,
  stepDeltaFromWheel,
  stepFromPointer,
  stepFromScrollRatio,
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

export type PaintCursor = {
  x: number
  y: number
  /** When false, cursor is in the off phase of blink. */
  on: boolean
}

/**
 * Ghostty-class cell paint: full 24-bit RGB bg+fg, monospaced metrics, no smoothing.
 * Always fills every cell background (including pure black) so truecolor is preserved.
 * Text glyphs are centered in-cell. Common box/block glyphs are vector-stroked
 * full-cell so panel borders join exactly at edges (Ghostty grid chrome).
 * Optional block cursor overlays the active selection row when the host is focused.
 */
export function paintCanvas(
  canvas: HTMLCanvasElement,
  frame: TerminalFrame,
  cellW: number,
  cellH: number,
  dpr: number,
  cursor?: PaintCursor | null,
) {
  const w = frame.cols * cellW
  const h = frame.rows * cellH
  canvas.width = Math.max(1, Math.floor(w * dpr))
  canvas.height = Math.max(1, Math.floor(h * dpr))
  // Fixed CSS pixel size — do not stretch with max-width (keeps integer cell metrics).
  canvas.style.width = `${w}px`
  canvas.style.height = `${h}px`
  canvas.style.maxWidth = 'none'
  canvas.style.imageRendering = 'pixelated'
  canvas.style.fontFeatureSettings = '"liga" 0, "calt" 0'
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
      const strokePlan = boxStrokeForGlyph(ch)
      if (strokePlan) {
        const { segs, fills } = boxStrokeGeometry(strokePlan, px, py, cellW, cellH)
        const fg = paintFg(cell)
        ctx.fillStyle = fg
        for (const fr of fills) {
          ctx.fillRect(fr.x, fr.y, fr.w, fr.h)
        }
        ctx.strokeStyle = fg
        ctx.lineCap = 'butt'
        ctx.lineJoin = 'miter'
        for (const seg of segs) {
          ctx.lineWidth = seg.width
          ctx.beginPath()
          ctx.moveTo(seg.x1, seg.y1)
          ctx.lineTo(seg.x2, seg.y2)
          ctx.stroke()
        }
        if (cell.underline) {
          ctx.lineWidth = 1
          ctx.beginPath()
          ctx.moveTo(px, py + cellH - 2)
          ctx.lineTo(px + cellW, py + cellH - 2)
          ctx.stroke()
        }
        continue
      }
      const weight = boldFontWeight(cell.bold)
      ctx.font = `${weight} ${fontSize}px ${PREVIEW_MONO_STACK}`
      ctx.fillStyle = paintFg(cell)
      ctx.textBaseline = isBoxOrBlockGlyph(ch) ? 'middle' : 'alphabetic'
      const tw = ctx.measureText(ch).width
      const span = isBoxOrBlockGlyph(ch) ? 1 : glyphCellSpan(tw, cellW)
      const drawY = isBoxOrBlockGlyph(ch) ? py + cellH / 2 : py + baseline
      const drawX = glyphDrawX(px, cellW, tw, ch)
      // Double-width glyphs: paint across two cells when the next cell is empty.
      if (span === 2 && x + 1 < frame.cols) {
        const next = frame.cells[y * frame.cols + x + 1]
        if (next && (!next.ch || next.ch === ' ' || next.ch === '\u00a0')) {
          ctx.fillStyle = rgb(cell.bg)
          ctx.fillRect(px + cellW, py, cellW, cellH)
          ctx.fillStyle = paintFg(cell)
          ctx.fillText(ch, px + 0.5, drawY)
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
      ctx.fillText(ch, drawX, drawY)
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
  // Block cursor (Ghostty default): solid phosphor cell when blink-on.
  if (cursor?.on) {
    const cx = Math.max(0, Math.min(frame.cols - 1, cursor.x | 0))
    const cy = Math.max(0, Math.min(frame.rows - 1, cursor.y | 0))
    const px = cx * cellW
    const py = cy * cellH
    const under = frame.cells[cy * frame.cols + cx]
    ctx.fillStyle = rgb(CURSOR_BLOCK_RGB)
    ctx.globalAlpha = 0.92
    ctx.fillRect(px, py, cellW, cellH)
    ctx.globalAlpha = 1
    // Invert glyph under cursor when present.
    if (under?.ch && under.ch !== ' ' && under.ch !== '\u00a0') {
      const weight = boldFontWeight(under.bold)
      ctx.font = `${weight} ${fontSize}px ${PREVIEW_MONO_STACK}`
      ctx.fillStyle = '#0a0a0a'
      const box = isBoxOrBlockGlyph(under.ch)
      ctx.textBaseline = box ? 'middle' : 'alphabetic'
      const tw = ctx.measureText(under.ch).width
      const dy = box ? py + cellH / 2 : py + baseline
      ctx.fillText(under.ch, glyphDrawX(px, cellW, tw, under.ch), dy)
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
  const [loading, setLoading] = useState(false)
  const [hover, setHover] = useState<{ x: number; y: number } | null>(null)
  const labelId = useId()
  const stepRef = useRef(0)
  const sizeKeyRef = useRef(sizeKey)
  const maxStepRef = useRef(0)
  const frameCacheRef = useRef<Map<string, TerminalFrame>>(new Map())
  const loadGenRef = useRef(0)
  const lastKeyRef = useRef<KeyStrokeStamp | null>(null)
  const lastViewportRef = useRef({ w: 0, h: 0 })
  const reconcileViewportRef = useRef<(() => void) | null>(null)
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
        const key = cacheKey(size, next)
        const warm = frameCacheRef.current.has(key)
        if (!warm) setLoading(true)
        const f = await fetchFrame(size, next)
        // Drop stale responses so rapid nav never rewinds the visible step.
        if (!isLoadStillCurrent(reqId, loadGenRef.current)) return
        setFrame(f)
        setStep(next)
        setSizeKey(size)
        setError(null)
        setLoading(false)
        setStepPulse((p) => p + 1)
        prefetchAdjacent(size, next)
        prefetchSizePack(size)
      } catch (e) {
        if (!isLoadStillCurrent(reqId, loadGenRef.current)) return
        setLoading(false)
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
  // Cursor blink re-enters this effect via caretOn.
  useEffect(() => {
    const canvas = canvasRef.current
    if (!canvas || !frame) return
    let cancelled = false
    const cursor =
      focused && canInteract
        ? {
            ...inferCursorFromFrame(frame.cells, frame.cols, frame.rows, PAD, step),
            on: caretOn,
          }
        : null
    const paint = () => {
      if (cancelled) return
      const raw = typeof window !== 'undefined' ? window.devicePixelRatio || 1 : 1
      paintCanvas(canvas, frame, cellW, cellH, paintDpr(raw), cursor)
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
  }, [frame, cellW, cellH, focused, canInteract, step, caretOn])

  // Ghostty-style caret blink while focused (visual only; selection is frame-backed).
  useEffect(() => {
    if (!focused || !canInteract) {
      setCaretOn(true)
      return
    }
    const id = window.setInterval(() => setCaretOn((v) => !v), 530)
    return () => window.clearInterval(id)
  }, [focused, canInteract])

  // Real remap: visible CSS size → story cols/rows → nearest exported pack → load.
  // Observe stage AND chrome host; re-check on window resize and pointerenter.
  // combinedHostViewport mins chrome width so overflow:auto stages cannot stick
  // on wide canvas when the Ghostty window chrome is narrowed.
  useEffect(() => {
    const stage = stageRef.current
    const chrome = hostRef.current
    if (!stage || !manifest) return
    let timer: ReturnType<typeof setTimeout> | null = null
    const apply = (cssW: number, cssH: number, force = false) => {
      if (
        !force &&
        !materialViewportChange(
          lastViewportRef.current.w,
          lastViewportRef.current.h,
          cssW,
          cssH,
        )
      ) {
        return
      }
      lastViewportRef.current = { w: cssW, h: cssH }
      const { storyCols, storyRows } = storySizeForCssHost(cssW, cssH, cellW, cellH)
      const nextKey = pickSizeKey(storyCols, storyRows, sizes)
      stage.dataset['hostCssW'] = String(Math.round(cssW))
      stage.dataset['hostCssH'] = String(Math.round(cssH))
      stage.dataset['wantStoryCols'] = String(storyCols)
      stage.dataset['wantStoryRows'] = String(storyRows)
      stage.dataset['sizeKey'] = nextKey
      if (chrome) {
        chrome.dataset['chromeCssW'] = String(Math.round(chrome.clientWidth))
      }
      if (nextKey !== sizeKeyRef.current) {
        prefetchSizePack(nextKey)
        void loadFrame(nextKey, stepRef.current)
      }
    }
    const readViewport = (contentW = 0, contentH = 0) =>
      combinedHostViewport(
        stage.clientWidth,
        stage.clientHeight,
        chrome?.clientWidth ?? 0,
        contentW,
        contentH,
      )
    const schedule = (contentW = 0, contentH = 0, force = false) => {
      const { width, height } = readViewport(contentW, contentH)
      const { storyCols, storyRows } = storySizeForCssHost(width, height, cellW, cellH)
      const pendingKey = pickSizeKey(storyCols, storyRows, sizes)
      if (pendingKey !== sizeKeyRef.current) {
        prefetchSizePack(pendingKey)
      }
      if (timer) clearTimeout(timer)
      timer = setTimeout(() => apply(width, height, force), 40)
    }
    const reconcile = () => schedule(0, 0, true)
    reconcileViewportRef.current = reconcile
    const ro = new ResizeObserver((entries) => {
      const entry = entries[0]
      if (!entry) return
      schedule(entry.contentRect.width, entry.contentRect.height, false)
    })
    ro.observe(stage)
    if (chrome) ro.observe(chrome)
    const onWinResize = () => schedule(0, 0, false)
    window.addEventListener('resize', onWinResize)
    // Initial force so first paint picks the real visible pack.
    schedule(0, 0, true)
    return () => {
      reconcileViewportRef.current = null
      ro.disconnect()
      window.removeEventListener('resize', onWinResize)
      if (timer) clearTimeout(timer)
    }
  }, [manifest, cellW, cellH, sizes, loadFrame, prefetchSizePack])

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
      const action = stepDeltaFromNavKey(rawKey)
      if (action === null) return false
      // Dedupe identity is the raw key (window + React both fire).
      const now =
        typeof performance !== 'undefined' ? performance.now() : Date.now()
      if (!shouldAcceptKeyEvent(rawKey, now, lastKeyRef.current)) {
        return true // swallow duplicate without double-stepping
      }
      lastKeyRef.current = { key: rawKey, t: now }
      goStep(applyNavStepAction(stepRef.current, maxStep, action))
      setCaretOn(true) // reset blink on nav like a real terminal
      return true
    },
    [canInteract, goStep, maxStep],
  )

  /** Ghostty-like focus-follows-mouse: hover focuses so keys work without click. */
  const onPointerEnterHost = () => {
    if (!canInteract) return
    hostRef.current?.focus()
    setFocused(true)
    // Reconcile size when pointer enters — catches style-driven shrinks RO missed.
    reconcileViewportRef.current?.()
  }

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

  const pointerCssOnCanvas = (
    e: { clientX: number; clientY: number },
    canvas: HTMLCanvasElement,
  ): { xCss: number; yCss: number } => {
    const rect = canvas.getBoundingClientRect()
    const scaleY = rect.height > 0 ? canvas.clientHeight / rect.height : 1
    const scaleX = rect.width > 0 ? canvas.clientWidth / rect.width : 1
    return {
      xCss: (e.clientX - rect.left) * scaleX,
      yCss: (e.clientY - rect.top) * scaleY,
    }
  }

  const onPointerMove = (e: ReactPointerEvent) => {
    const canvas = canvasRef.current
    if (!canvas || !frame) {
      if (hover) setHover(null)
      return
    }
    const t = e.target as HTMLElement | null
    if (t?.closest?.('[data-termrock-scrollbar]')) {
      if (hover) setHover(null)
      return
    }
    const { xCss, yCss } = pointerCssOnCanvas(e, canvas)
    const next = cellAtPointer(xCss, yCss, cellW, cellH, frame.cols, frame.rows)
    if (!next) {
      if (hover) setHover(null)
      return
    }
    if (!hover || hover.x !== next.x || hover.y !== next.y) setHover(next)
  }

  const onPointerLeave = () => {
    if (hover) setHover(null)
  }

  const onPointerDown = (e: ReactPointerEvent) => {
    hostRef.current?.focus()
    setFocused(true)
    if (!canInteract) return
    if (e.button !== 0) return
    // Scrollbar track/thumb owns its own handlers.
    const t = e.target as HTMLElement | null
    if (t?.closest?.('[data-termrock-scrollbar]')) return
    const canvas = canvasRef.current
    if (canvas && frame) {
      const { xCss, yCss } = pointerCssOnCanvas(e, canvas)
      const at = cellAtPointer(xCss, yCss, cellW, cellH, frame.cols, frame.rows)
      if (at) setHover(at)
      goStep(
        stepFromPointer(
          yCss,
          xCss,
          cellH,
          cellW,
          PAD,
          maxStep,
          frame.rows,
          frame.cols,
          frame.story_cols,
        ),
      )
      return
    }
    goStep(step + 1)
  }

  /** Ghostty-class overlay scrollbar: track click jumps; thumb drag scrubs steps. */
  const onScrollbarPointerDown = (e: ReactPointerEvent<HTMLDivElement>) => {
    e.stopPropagation()
    e.preventDefault()
    hostRef.current?.focus()
    setFocused(true)
    if (!canInteract || maxStep <= 0) return
    const track = e.currentTarget
    const applyFromClientY = (clientY: number) => {
      const rect = track.getBoundingClientRect()
      const ratio = rect.height > 0 ? (clientY - rect.top) / rect.height : 0
      goStep(stepFromScrollRatio(ratio, maxStep))
    }
    applyFromClientY(e.clientY)
    const onMove = (ev: PointerEvent) => applyFromClientY(ev.clientY)
    const onUp = () => {
      window.removeEventListener('pointermove', onMove)
      window.removeEventListener('pointerup', onUp)
    }
    window.addEventListener('pointermove', onMove)
    window.addEventListener('pointerup', onUp)
  }

  const thumb = canInteract && maxStep > 0 ? scrollThumbMetrics(step, maxStep) : null

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
      ? '↑↓/wheel/scroll · j/k · click · hover cell · Home/End'
      : '↑↓/←→ · wheel/scroll · j/k · click · hover cell · Home/End'
    : 'read-only frame · hover cell'

  const hoverProbe =
    hover && frame
      ? formatCellProbe(
          hover.x,
          hover.y,
          frame.cells[hover.y * frame.cols + hover.x] ?? null,
        )
      : ''

  const cursorCell = frame
    ? inferCursorFromFrame(frame.cells, frame.cols, frame.rows, PAD, step)
    : cursorCellForStep(step, PAD, 40, 8)

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
      data-preview-loading={loading ? 'true' : 'false'}
      data-preview-cursor={
        focused && canInteract && frame ? `${cursorCell.x},${cursorCell.y}` : ''
      }
      data-preview-hover={hover ? `${hover.x},${hover.y}` : ''}
      data-preview-hover-probe={hoverProbe}
      data-preview-scrollbar={thumb ? 'true' : 'false'}
      data-preview-scroll-ratio={thumb ? thumb.ratio.toFixed(4) : ''}
    >
      <div
        ref={hostRef}
        role="application"
        tabIndex={0}
        aria-labelledby={labelId}
        aria-label={`Interactive terminal preview: ${story}`}
        onKeyDown={onKeyDown}
        onPointerEnter={onPointerEnterHost}
        onPointerDown={onPointerDown}
        onPointerMove={onPointerMove}
        onPointerLeave={onPointerLeave}
        onWheel={onWheel}
        onFocus={() => {
          setFocused(true)
          reconcileViewportRef.current?.()
        }}
        onBlur={() => setFocused(false)}
        style={{
          ...chrome,
          outline: 'none',
          cursor: canInteract ? 'text' : 'default',
          // Allow chrome width constraints (agent-browser / responsive parents).
          width: '100%',
          boxSizing: 'border-box',
        }}
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
            position: 'relative',
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
          {/* Ghostty 1.3-class overlay scrollbar: position in multi-step packs. */}
          {thumb ? (
            <div
              data-termrock-scrollbar="1"
              role="scrollbar"
              aria-orientation="vertical"
              aria-valuemin={0}
              aria-valuemax={maxStep}
              aria-valuenow={step}
              aria-label="Preview step scrollbar"
              onPointerDown={onScrollbarPointerDown}
              style={{
                position: 'absolute',
                top: 16,
                right: 6,
                bottom: 16,
                width: 8,
                borderRadius: 999,
                background: focused ? 'rgba(57,255,20,0.08)' : 'rgba(80,90,80,0.12)',
                boxShadow: 'inset 0 0 0 1px rgba(57,255,20,0.12)',
                cursor: 'pointer',
                zIndex: 2,
                touchAction: 'none',
              }}
            >
              <div
                data-termrock-scroll-thumb="1"
                style={{
                  position: 'absolute',
                  left: 1,
                  right: 1,
                  top: `${thumb.top * 100}%`,
                  height: `${thumb.height * 100}%`,
                  minHeight: 14,
                  borderRadius: 999,
                  background: focused
                    ? 'linear-gradient(180deg, #55ff66 0%, #00ff41 55%, #00c832 100%)'
                    : 'linear-gradient(180deg, #4a6a4a 0%, #2a4a2a 100%)',
                  boxShadow: focused ? '0 0 8px rgba(57,255,20,0.45)' : undefined,
                  transition: 'top 80ms ease-out',
                }}
              />
            </div>
          ) : null}
        </div>
        <div style={statusBar} data-termrock-status="1">
          <span style={{ color: canInteract ? '#39ff14' : '#6a7a6a' }}>
            {loading
              ? '… loading'
              : canInteract
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
          {hoverProbe ? (
            <span
              data-termrock-cell-probe="1"
              style={{ color: focused ? '#9fef9f' : '#7a8a7a', opacity: 0.95 }}
              title="Cell under pointer (truecolor)"
            >
              {hoverProbe}
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
