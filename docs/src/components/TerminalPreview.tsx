'use client'

import {
  useCallback,
  useEffect,
  useId,
  useRef,
  useState,
  type CSSProperties,
  type KeyboardEvent as ReactKeyboardEvent,
  type PointerEvent as ReactPointerEvent,
  type WheelEvent as ReactWheelEvent,
} from 'react'
import {
  PREVIEW_MONO_STACK,
  baselineForCell,
  boldFontWeight,
  boxStrokeForGlyph,
  boxStrokeGeometry,
  cellAtPointer,
  fontSizeForCell,
  formatCellProbe,
  glyphCellSpan,
  glyphDrawX,
  isBoxOrBlockGlyph,
  paintDpr,
  resolvePaintFg,
  underlineMetrics,
  underlineSpans,
} from '@/components/preview-metrics'

/** Cell payload from the shared Rust demo runtime (truecolor RGB). */
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

const DEFAULT_CELL_W = 9
const DEFAULT_CELL_H = 18
const PAD = 1
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

function rgb(c: [number, number, number]): string {
  return `rgb(${c[0]}, ${c[1]}, ${c[2]})`
}

/** Dim + Ghostty min-contrast vs cell bg (pure resolvePaintFg). */
function paintFg(cell: FrameCell): string {
  return rgb(resolvePaintFg(cell.fg, cell.bg, cell.dim))
}

/**
 * Ghostty-class cell paint: full 24-bit RGB bg+fg, monospaced metrics, no smoothing.
 * Always fills every cell background (including pure black) so truecolor is preserved.
 * Text glyphs are centered in-cell. Common box/block glyphs are vector-stroked
 * full-cell so panel borders join exactly at edges (Ghostty grid chrome).
 * Underlines are drawn as continuous row spans (selection chrome), not per-cell hairlines.
 * Cursor/caret paint comes only from the Rust widget buffer. The host never
 * invents a cursor for passive or non-editable demos.
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
  const ul = underlineMetrics(cellH)
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
        const shadeAlpha =
          strokePlan.kind === 'shade'
            ? Math.max(0.05, Math.min(1, strokePlan.density))
            : 1
        ctx.globalAlpha = shadeAlpha
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
        ctx.globalAlpha = 1
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
          x += 1 // skip continuation cell
          continue
        }
      }
      ctx.fillText(ch, drawX, drawY)
    }
  }
  // Continuous underlines (Ghostty selection chrome) — one stroke per run.
  ctx.lineCap = 'butt'
  for (let y = 0; y < frame.rows; y++) {
    const rowStart = y * frame.cols
    const row = frame.cells.slice(rowStart, rowStart + frame.cols)
    const spans = underlineSpans(row)
    if (spans.length === 0) continue
    const py = y * cellH + ul.offsetFromTop + ul.thickness / 2
    ctx.lineWidth = ul.thickness
    for (const span of spans) {
      const first = row[span.start]
      if (!first) continue
      ctx.strokeStyle = paintFg(first)
      ctx.beginPath()
      ctx.moveTo(span.start * cellW, py)
      ctx.lineTo(span.end * cellW, py)
      ctx.stroke()
    }
  }
}

export type DemoDescriptor = {
  id: string
  title: string
  component: string
  description: string
  cols: number
  rows: number
  interactive: boolean
  interactionKind: string
  hints: string[]
}

export type DemoUpdate = {
  changed: boolean
  outcome: string | null
  hints: string[]
  interactive: boolean
  capturesTextInput: boolean
  nextDeadlineMs: number | null
}

type DemoEvent =
  | {
      type: 'key'
      key: string
      kind: 'press' | 'repeat' | 'release'
      shift?: boolean
      ctrl?: boolean
      alt?: boolean
      meta?: boolean
    }
  | {
      type: 'pointer'
      kind: 'move' | 'down' | 'up' | 'drag'
      x: number
      y: number
      button?: 'left' | 'right' | 'middle'
    }
  | { type: 'wheel'; deltaX: number; deltaY: number; x: number; y: number }
  | { type: 'paste'; text: string }
  | { type: 'resize'; cols: number; rows: number }
  | { type: 'focus'; focused: boolean }
  | { type: 'tick'; elapsedMs: number }

type PreviewRuntime = typeof import('@/generated/termrock-preview/termrock_lookbook_web.js')

let runtimePromise: Promise<PreviewRuntime> | null = null

export function loadPreviewRuntime(): Promise<PreviewRuntime> {
  runtimePromise ??= import(
    '@/generated/termrock-preview/termrock_lookbook_web.js'
  ).then(async (runtime) => {
    await runtime.default()
    return runtime
  })
  return runtimePromise
}

export function shouldCapturePreviewKey(
  key: string,
  descriptor: DemoDescriptor | null,
  capturesTextInput = false,
): boolean {
  if (!descriptor?.interactive) return false
  const navigation = new Set([
    'ArrowUp',
    'ArrowDown',
    'ArrowLeft',
    'ArrowRight',
    'Enter',
    'Escape',
    'Tab',
    'Backspace',
    'Delete',
    'Home',
    'End',
    'PageUp',
    'PageDown',
    ' ',
  ])
  if (navigation.has(key)) return true
  return (
    key.length === 1 &&
    (capturesTextInput ||
      ['editor-form', 'activation', 'selection-navigation'].includes(
        descriptor.interactionKind,
      ))
  )
}

export function pointerCell(
  clientX: number,
  clientY: number,
  canvas: HTMLCanvasElement,
  frame: TerminalFrame,
  cellW = DEFAULT_CELL_W,
  cellH = DEFAULT_CELL_H,
): { x: number; y: number } | null {
  const rect = canvas.getBoundingClientRect()
  const scaleX = rect.width > 0 ? canvas.clientWidth / rect.width : 1
  const scaleY = rect.height > 0 ? canvas.clientHeight / rect.height : 1
  return cellAtPointer(
    (clientX - rect.left) * scaleX,
    (clientY - rect.top) * scaleY,
    cellW,
    cellH,
    frame.cols,
    frame.rows,
  )
}

export type TerminalPreviewProps = {
  story: string
  caption?: string
  maxHeight?: number
  interactive?: boolean
}

/**
 * Live terminal host. Rust owns persistent demo state and behavior; React only
 * translates browser events and paints returned Ratatui cells.
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
  const runtimeRef = useRef<PreviewRuntime | null>(null)
  const handleRef = useRef<number | null>(null)
  const frameRef = useRef<TerminalFrame | null>(null)
  const descriptorRef = useRef<DemoDescriptor | null>(null)
  const updateRef = useRef<DemoUpdate | null>(null)
  const dragRef = useRef(false)
  const visibleRef = useRef(true)
  const mountedAtRef = useRef(0)
  const lastGridRef = useRef({ cols: 0, rows: 0 })
  const [frame, setFrame] = useState<TerminalFrame | null>(null)
  const [descriptor, setDescriptor] = useState<DemoDescriptor | null>(null)
  const [update, setUpdate] = useState<DemoUpdate | null>(null)
  const [focused, setFocused] = useState(false)
  const [hover, setHover] = useState<{ x: number; y: number } | null>(null)
  const [error, setError] = useState<string | null>(null)
  const [loading, setLoading] = useState(true)
  const [poster, setPoster] = useState(false)
  const [zen, setZen] = useState(false)
  const labelId = useId()

  frameRef.current = frame
  descriptorRef.current = descriptor
  updateRef.current = update

  const renderCurrent = useCallback(() => {
    const runtime = runtimeRef.current
    const handle = handleRef.current
    if (!runtime || handle === null) return
    const next = JSON.parse(runtime.demo_frame(handle)) as TerminalFrame
    frameRef.current = next
    setFrame(next)
  }, [])

  const dispatch = useCallback(
    (event: DemoEvent, alwaysRender = false): DemoUpdate | null => {
      const runtime = runtimeRef.current
      const handle = handleRef.current
      if (!runtime || handle === null) return null
      try {
        const next = JSON.parse(
          runtime.dispatch_demo(handle, JSON.stringify(event)),
        ) as DemoUpdate
        updateRef.current = next
        setUpdate(next)
        if (next.changed || alwaysRender) renderCurrent()
        return next
      } catch (reason) {
        setError(reason instanceof Error ? reason.message : String(reason))
        return null
      }
    },
    [renderCurrent],
  )

  useEffect(() => {
    let cancelled = false
    let mountedHandle: number | null = null
    setLoading(true)
    setError(null)
    setFrame(null)
    setUpdate(null)
    setDescriptor(null)
    setPoster(false)
    void loadPreviewRuntime()
      .then((runtime) => {
        if (cancelled) return
        const catalog = JSON.parse(runtime.catalog_json()) as DemoDescriptor[]
        const nextDescriptor = catalog.find((entry) => entry.id === story)
        if (!nextDescriptor) throw new Error('unknown TermRock demo: ' + story)
        const handle = runtime.mount_demo(
          story,
          Math.max(8, nextDescriptor.cols),
          Math.max(4, nextDescriptor.rows),
        )
        if (cancelled) {
          runtime.unmount_demo(handle)
          return
        }
        mountedHandle = handle
        runtimeRef.current = runtime
        handleRef.current = handle
        descriptorRef.current = nextDescriptor
        setDescriptor(nextDescriptor)
        mountedAtRef.current = performance.now()
        const initial = JSON.parse(runtime.demo_frame(handle)) as TerminalFrame
        frameRef.current = initial
        lastGridRef.current = {
          cols: initial.story_cols,
          rows: initial.story_rows,
        }
        setFrame(initial)
        const initialUpdate = JSON.parse(
          runtime.dispatch_demo(
            handle,
            JSON.stringify({
              type: 'resize',
              cols: initial.story_cols,
              rows: initial.story_rows,
            }),
          ),
        ) as DemoUpdate
        setUpdate(initialUpdate)
        setLoading(false)
      })
      .catch(async (reason: unknown) => {
        if (cancelled) return
        try {
          const slug = story.replaceAll('/', '-')
          const response = await fetch(`/preview-posters/${slug}.json`)
          if (!response.ok) throw new Error(`poster ${response.status}`)
          const fallback = (await response.json()) as TerminalFrame
          if (cancelled) return
          const fallbackDescriptor: DemoDescriptor = {
            id: story,
            title: fallback.title,
            component: fallback.component,
            description: 'Static fallback for environments without WebAssembly.',
            cols: fallback.story_cols,
            rows: fallback.story_rows,
            interactive: false,
            interactionKind: 'passive-paint',
            hints: [],
          }
          frameRef.current = fallback
          descriptorRef.current = fallbackDescriptor
          setFrame(fallback)
          setDescriptor(fallbackDescriptor)
          setPoster(true)
          setLoading(false)
          setError(null)
        } catch {
          if (!cancelled) {
            setLoading(false)
            setError(reason instanceof Error ? reason.message : String(reason))
          }
        }
      })
    return () => {
      cancelled = true
      const runtime = runtimeRef.current
      if (runtime && mountedHandle !== null) {
        try {
          runtime.unmount_demo(mountedHandle)
        } catch {
          // A remount can already have released the handle.
        }
      }
      if (handleRef.current === mountedHandle) handleRef.current = null
    }
  }, [story])

  useEffect(() => {
    const canvas = canvasRef.current
    if (!canvas || !frame) return
    let cancelled = false
    const paint = () => {
      if (cancelled) return
      const raw = window.devicePixelRatio || 1
      paintCanvas(canvas, frame, DEFAULT_CELL_W, DEFAULT_CELL_H, paintDpr(raw))
    }
    paint()
    void document.fonts?.ready.then(paint)
    void document.fonts
      ?.load(String(fontSizeForCell(DEFAULT_CELL_H)) + 'px JetBrains Mono')
      .then(paint)
      .catch(() => undefined)
    return () => {
      cancelled = true
    }
  }, [frame])

  useEffect(() => {
    const stage = stageRef.current
    if (!stage || !descriptor) return
    let raf = 0
    const resize = () => {
      const width = Math.max(72, stage.clientWidth - 24)
      const height = Math.max(72, Math.min(maxHeight, stage.clientHeight) - 24)
      const { storyCols, storyRows } = storySizeForCssHost(
        width,
        height,
        DEFAULT_CELL_W,
        DEFAULT_CELL_H,
      )
      if (
        lastGridRef.current.cols === storyCols &&
        lastGridRef.current.rows === storyRows
      ) {
        return
      }
      lastGridRef.current = { cols: storyCols, rows: storyRows }
      dispatch({ type: 'resize', cols: storyCols, rows: storyRows }, true)
    }
    const schedule = () => {
      cancelAnimationFrame(raf)
      raf = requestAnimationFrame(resize)
    }
    const observer = new ResizeObserver(schedule)
    observer.observe(stage)
    schedule()
    return () => {
      cancelAnimationFrame(raf)
      observer.disconnect()
    }
  }, [descriptor, dispatch, maxHeight])

  useEffect(() => {
    const host = hostRef.current
    if (!host) return
    const observer = new IntersectionObserver(
      ([entry]) => {
        visibleRef.current = Boolean(entry?.isIntersecting)
      },
      { rootMargin: '120px' },
    )
    observer.observe(host)
    return () => observer.disconnect()
  }, [])

  useEffect(() => {
    if (descriptor?.interactionKind !== 'timed-state') return
    const reduced = window.matchMedia('(prefers-reduced-motion: reduce)').matches
    if (reduced) return
    let raf = 0
    let previousStep = -1
    const animate = (now: number) => {
      if (visibleRef.current && document.visibilityState === 'visible') {
        const elapsedMs = Math.max(0, Math.floor(now - mountedAtRef.current))
        const step = Math.floor(elapsedMs / 100)
        if (step !== previousStep) {
          previousStep = step
          dispatch({ type: 'tick', elapsedMs }, true)
        }
      }
      raf = requestAnimationFrame(animate)
    }
    raf = requestAnimationFrame(animate)
    return () => cancelAnimationFrame(raf)
  }, [descriptor, dispatch])

  useEffect(() => {
    const host = hostRef.current
    if (!host) return
    const beforeInput = (event: InputEvent) => {
      if (
        (!updateRef.current?.capturesTextInput &&
          descriptorRef.current?.interactionKind !== 'editor-form') ||
        !event.data
      ) {
        return
      }
      event.preventDefault()
      for (const value of event.data) {
        dispatch({ type: 'key', key: value, kind: 'press' })
      }
    }
    const paste = (event: ClipboardEvent) => {
      if (
        !updateRef.current?.capturesTextInput &&
        descriptorRef.current?.interactionKind !== 'editor-form'
      ) return
      const text = event.clipboardData?.getData('text')
      if (!text) return
      event.preventDefault()
      dispatch({ type: 'paste', text })
    }
    host.addEventListener('beforeinput', beforeInput)
    host.addEventListener('paste', paste)
    return () => {
      host.removeEventListener('beforeinput', beforeInput)
      host.removeEventListener('paste', paste)
    }
  }, [dispatch])

  const canInteract = Boolean(interactive && descriptor?.interactive)
  const animated = descriptor?.interactionKind === 'timed-state'

  const keyEvent = (
    event: ReactKeyboardEvent,
    kind: 'press' | 'repeat' | 'release',
  ) => {
    if (
      !canInteract ||
      !shouldCapturePreviewKey(
        event.key,
        descriptor,
        update?.capturesTextInput ?? false,
      )
    ) return
    event.preventDefault()
    event.stopPropagation()
    dispatch({
      type: 'key',
      key: event.key,
      kind,
      shift: event.shiftKey,
      ctrl: event.ctrlKey,
      alt: event.altKey,
      meta: event.metaKey,
    })
  }

  const eventCell = (
    event: { clientX: number; clientY: number },
  ): { x: number; y: number } | null => {
    const canvas = canvasRef.current
    const current = frameRef.current
    if (!canvas || !current) return null
    return pointerCell(
      event.clientX,
      event.clientY,
      canvas,
      current,
      DEFAULT_CELL_W,
      DEFAULT_CELL_H,
    )
  }

  const pointerMove = (event: ReactPointerEvent) => {
    const cell = eventCell(event)
    if (!cell) {
      setHover(null)
      return
    }
    setHover(cell)
    if (!canInteract) return
    dispatch({
      type: 'pointer',
      kind: dragRef.current ? 'drag' : 'move',
      x: cell.x,
      y: cell.y,
    })
  }

  const pointerDown = (event: ReactPointerEvent) => {
    if (!canInteract || event.button !== 0) return
    const cell = eventCell(event)
    if (!cell) return
    hostRef.current?.focus()
    event.currentTarget.setPointerCapture(event.pointerId)
    dragRef.current = true
    dispatch({ type: 'pointer', kind: 'down', x: cell.x, y: cell.y })
  }

  const pointerUp = (event: ReactPointerEvent) => {
    if (!canInteract || !dragRef.current) return
    const cell = eventCell(event)
    dragRef.current = false
    if (!cell) return
    dispatch({ type: 'pointer', kind: 'up', x: cell.x, y: cell.y })
  }

  const wheel = (event: ReactWheelEvent) => {
    if (!canInteract) return
    const cell = eventCell(event)
    if (!cell || (event.deltaX === 0 && event.deltaY === 0)) return
    event.preventDefault()
    event.stopPropagation()
    dispatch({
      type: 'wheel',
      deltaX: Math.sign(event.deltaX),
      deltaY: Math.sign(event.deltaY),
      x: cell.x,
      y: cell.y,
    })
  }

  const reset = () => {
    const runtime = runtimeRef.current
    const handle = handleRef.current
    if (!runtime || handle === null) return
    try {
      const next = JSON.parse(runtime.reset_demo(handle)) as TerminalFrame
      mountedAtRef.current = performance.now()
      frameRef.current = next
      setFrame(next)
      setUpdate({
        changed: true,
        outcome: 'Demo reset',
        hints: descriptor?.hints ?? [],
        interactive: descriptor?.interactive ?? false,
        capturesTextInput: false,
        nextDeadlineMs: animated ? 100 : null,
      })
    } catch (reason) {
      setError(reason instanceof Error ? reason.message : String(reason))
    }
  }

  const hoverProbe =
    hover && frame
      ? formatCellProbe(
          hover.x,
          hover.y,
          frame.cells[hover.y * frame.cols + hover.x] ?? null,
        )
      : ''

  const chrome: CSSProperties = {
    borderRadius: 12,
    border: focused ? '1px solid #39ff14' : '1px solid #1e261e',
    background: 'linear-gradient(180deg, #222622 0%, #121512 14%, #070907 100%)',
    boxShadow: focused
      ? '0 0 0 1px rgba(57,255,20,0.28), 0 0 24px rgba(57,255,20,0.08), 0 16px 48px rgba(0,0,0,0.6)'
      : '0 1px 0 rgba(255,255,255,0.04) inset, 0 16px 48px rgba(0,0,0,0.5)',
    overflow: 'hidden',
    width: '100%',
    maxWidth: '100%',
    outline: 'none',
    filter: focused ? 'none' : 'brightness(0.94)',
    transition: 'border-color 120ms ease, box-shadow 120ms ease, filter 120ms ease',
    ...(zen
      ? {
          position: 'fixed',
          inset: 12,
          zIndex: 1000,
          width: 'auto',
          maxWidth: 'none',
          display: 'flex',
          flexDirection: 'column',
        }
      : {}),
  }

  return (
    <figure
      className="not-prose my-6"
      data-termrock-preview={story}
      data-preview-live={poster ? 'static-poster' : 'rust-wasm'}
      data-preview-interactive={canInteract ? 'true' : 'false'}
      data-preview-focused={focused ? 'true' : 'false'}
      data-preview-cols={frame?.cols ?? ''}
      data-preview-rows={frame?.rows ?? ''}
      data-preview-outcome={update?.outcome ?? ''}
      data-preview-hover={hover ? String(hover.x) + ',' + String(hover.y) : ''}
    >
      <div
        ref={hostRef}
        role={canInteract ? 'application' : 'img'}
        tabIndex={canInteract ? 0 : -1}
        aria-labelledby={labelId}
        aria-label={(canInteract ? 'Interactive' : 'Rendered') + ' terminal preview: ' + story}
        onKeyDown={(event) =>
          keyEvent(event, event.repeat ? 'repeat' : 'press')
        }
        onKeyUp={(event) => keyEvent(event, 'release')}
        onPointerMove={pointerMove}
        onPointerLeave={() => setHover(null)}
        onPointerDown={pointerDown}
        onPointerUp={pointerUp}
        onPointerCancel={() => {
          dragRef.current = false
        }}
        onWheel={wheel}
        onFocus={() => {
          setFocused(true)
          dispatch({ type: 'focus', focused: true })
        }}
        onBlur={() => {
          setFocused(false)
          dispatch({ type: 'focus', focused: false })
        }}
        style={chrome}
      >
        <div
          id={labelId}
          style={{
            display: 'flex',
            alignItems: 'center',
            gap: 8,
            padding: '9px 12px',
            borderBottom: '1px solid #1a201a',
            background: 'linear-gradient(180deg, #181b18 0%, #0e100e 100%)',
            fontFamily: PREVIEW_MONO_STACK,
            fontSize: 12,
            color: '#8a9a8a',
            userSelect: 'none',
          }}
        >
          <span style={{ display: 'flex', gap: 6 }} aria-hidden>
            {['#ff5f57', '#febc2e', '#28c840'].map((color) => (
              <span
                key={color}
                style={{ width: 10, height: 10, borderRadius: 999, background: color }}
              />
            ))}
          </span>
          <span style={{ color: '#c8d6c8' }}>Ghostty</span>
          <span>· TermRock —</span>
          <span style={{ color: '#39ff14' }}>{story}</span>
          <span style={{ marginLeft: 'auto', fontSize: 11 }}>
            {frame
              ? String(frame.cols) + '×' + String(frame.rows) + ' · RGB24'
              : 'mounting Rust…'}
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
            maxHeight: zen ? 'calc(100vh - 116px)' : maxHeight,
            flex: zen ? 1 : undefined,
            minHeight: 120,
            width: '100%',
            display: 'flex',
            justifyContent: 'center',
            alignItems: 'flex-start',
            boxSizing: 'border-box',
          }}
        >
          {error ? (
            <pre style={{ color: '#ff5e7a', fontSize: 12, whiteSpace: 'pre-wrap' }}>
              {error}
            </pre>
          ) : loading || !frame ? (
            <span style={{ color: '#6a7a6a', fontFamily: PREVIEW_MONO_STACK }}>
              Mounting shared Rust demo…
            </span>
          ) : (
            <canvas
              ref={canvasRef}
              data-termrock-canvas="1"
              data-canvas-cols={frame.cols}
              data-canvas-rows={frame.rows}
              style={{ display: 'block', imageRendering: 'pixelated', flexShrink: 0 }}
            />
          )}
        </div>
        <div
          data-termrock-status="1"
          style={{
            display: 'flex',
            alignItems: 'center',
            flexWrap: 'wrap',
            gap: 10,
            padding: '6px 12px',
            borderTop: '1px solid #1a201a',
            fontFamily: PREVIEW_MONO_STACK,
            fontSize: 11,
            color: '#718071',
            background: '#0c0e0c',
            userSelect: 'none',
          }}
        >
          <span style={{ color: canInteract || animated ? '#39ff14' : '#6a7a6a' }}>
            {poster
              ? '○ static fallback'
              : animated
                ? '◐ timed Rust demo'
                : canInteract
                  ? '● live Rust demo'
                  : '○ live paint'}
          </span>
          {update?.outcome ? (
            <span data-termrock-outcome="1" style={{ color: '#b4e8b4' }}>
              {update.outcome}
            </span>
          ) : null}
          {hoverProbe ? <span data-termrock-cell-probe="1">{hoverProbe}</span> : null}
          <span data-termrock-hints="1" style={{ marginLeft: 'auto' }}>
            {(update?.hints ?? descriptor?.hints ?? []).join(' · ') ||
              'No input — rendered state only'}
          </span>
          <button
            type="button"
            onClick={(event) => {
              event.stopPropagation()
              setZen((value) => !value)
            }}
            style={{
              border: '1px solid #334033',
              borderRadius: 5,
              background: '#121712',
              color: '#a8b8a8',
              padding: '2px 7px',
              font: 'inherit',
              cursor: 'pointer',
            }}
          >
            {zen ? 'Exit full preview' : 'Full preview'}
          </button>
          {!poster ? <button
            type="button"
            onClick={(event) => {
              event.stopPropagation()
              reset()
            }}
            style={{
              border: '1px solid #334033',
              borderRadius: 5,
              background: '#121712',
              color: '#a8b8a8',
              padding: '2px 7px',
              font: 'inherit',
              cursor: 'pointer',
            }}
          >
            Reset
          </button> : null}
        </div>
      </div>
      {caption ? (
        <figcaption
          style={{
            marginTop: 8,
            fontSize: 13,
            color: '#8a9a8a',
            fontFamily: PREVIEW_MONO_STACK,
          }}
        >
          {caption}
        </figcaption>
      ) : null}
    </figure>
  )
}

export default TerminalPreview
