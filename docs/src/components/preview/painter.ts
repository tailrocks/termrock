import {
  PREVIEW_MONO_STACK,
  baselineForCell,
  boldFontWeight,
  boxStrokeForGlyph,
  boxStrokeGeometry,
  cellAtPointer,
  fontSizeForCell,
  glyphCellSpan,
  glyphDrawX,
  isBoxOrBlockGlyph,
  resolvePaintFg,
  underlineMetrics,
  underlineSpans,
} from '@/components/preview-metrics'
import type { FrameCell, TerminalFrame } from '@/components/preview/model'

export const DEFAULT_CELL_W = 9
export const DEFAULT_CELL_H = 18

const PAD = 1
/** Ghostty-class surface fill under cells. */
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
): { readonly storyCols: number; readonly storyRows: number } {
  const totalC = colsForCssWidth(cssW, cellW)
  const totalR = rowsForCssHeight(cssH, cellH)
  return {
    storyCols: Math.max(8, totalC - PAD * 2),
    storyRows: Math.max(4, totalR - PAD * 2),
  }
}

function rgb(color: [number, number, number]): string {
  return `rgb(${color[0]}, ${color[1]}, ${color[2]})`
}

function paintFg(cell: FrameCell): string {
  return rgb(resolvePaintFg(cell.fg, cell.bg, cell.dim))
}

/** Convert painted terminal cells into screen-reader-readable row text. */
export function frameToText(frame: TerminalFrame): string {
  const rows: string[] = []
  for (let y = 0; y < frame.rows; y++) {
    let row = ''
    for (let x = 0; x < frame.cols; x++) {
      row += frame.cells[y * frame.cols + x]?.ch || ' '
    }
    rows.push(row.trimEnd())
  }
  return rows.join('\n').trimEnd()
}

/** Ghostty-class full truecolor cell paint. */
export function paintCanvas(
  canvas: HTMLCanvasElement,
  frame: TerminalFrame,
  cellW: number,
  cellH: number,
  dpr: number,
): void {
  const width = frame.cols * cellW
  const height = frame.rows * cellH
  canvas.width = Math.max(1, Math.floor(width * dpr))
  canvas.height = Math.max(1, Math.floor(height * dpr))
  canvas.style.width = `${width}px`
  canvas.style.height = `${height}px`
  canvas.style.maxWidth = 'none'
  canvas.style.imageRendering = 'pixelated'
  canvas.style.fontFeatureSettings = '"liga" 0, "calt" 0'
  const context = canvas.getContext('2d')
  if (!context) return
  context.setTransform(dpr, 0, 0, dpr, 0, 0)
  context.imageSmoothingEnabled = false
  context.fillStyle = SURFACE_BG
  context.fillRect(0, 0, width, height)
  const fontSize = fontSizeForCell(cellH)
  const baseline = baselineForCell(cellH)
  const underline = underlineMetrics(cellH)
  for (let y = 0; y < frame.rows; y++) {
    for (let x = 0; x < frame.cols; x++) {
      const cell = frame.cells[y * frame.cols + x]
      if (!cell) continue
      const px = x * cellW
      const py = y * cellH
      context.fillStyle = rgb(cell.bg)
      context.fillRect(px, py, cellW, cellH)
      const ch = cell.ch
      if (!ch || ch === ' ' || ch === '\u00a0') continue
      const strokePlan = boxStrokeForGlyph(ch)
      if (strokePlan) {
        const { segs, fills } = boxStrokeGeometry(strokePlan, px, py, cellW, cellH)
        const foreground = paintFg(cell)
        context.globalAlpha =
          strokePlan.kind === 'shade'
            ? Math.max(0.05, Math.min(1, strokePlan.density))
            : 1
        context.fillStyle = foreground
        for (const fill of fills) context.fillRect(fill.x, fill.y, fill.w, fill.h)
        context.strokeStyle = foreground
        context.lineCap = 'butt'
        context.lineJoin = 'miter'
        for (const segment of segs) {
          context.lineWidth = segment.width
          context.beginPath()
          context.moveTo(segment.x1, segment.y1)
          context.lineTo(segment.x2, segment.y2)
          context.stroke()
        }
        context.globalAlpha = 1
        continue
      }
      const weight = boldFontWeight(cell.bold)
      context.font = `${weight} ${fontSize}px ${PREVIEW_MONO_STACK}`
      context.fillStyle = paintFg(cell)
      context.textBaseline = isBoxOrBlockGlyph(ch) ? 'middle' : 'alphabetic'
      const textWidth = context.measureText(ch).width
      const span = isBoxOrBlockGlyph(ch) ? 1 : glyphCellSpan(textWidth, cellW)
      const drawY = isBoxOrBlockGlyph(ch) ? py + cellH / 2 : py + baseline
      const drawX = glyphDrawX(px, cellW, textWidth, ch)
      if (span === 2 && x + 1 < frame.cols) {
        const next = frame.cells[y * frame.cols + x + 1]
        if (next && (!next.ch || next.ch === ' ' || next.ch === '\u00a0')) {
          context.fillStyle = rgb(cell.bg)
          context.fillRect(px + cellW, py, cellW, cellH)
          context.fillStyle = paintFg(cell)
          context.fillText(ch, px + 0.5, drawY)
          x += 1
          continue
        }
      }
      context.fillText(ch, drawX, drawY)
    }
  }
  context.lineCap = 'butt'
  for (let y = 0; y < frame.rows; y++) {
    const rowStart = y * frame.cols
    const row = frame.cells.slice(rowStart, rowStart + frame.cols)
    const spans = underlineSpans(row)
    if (spans.length === 0) continue
    const py = y * cellH + underline.offsetFromTop + underline.thickness / 2
    context.lineWidth = underline.thickness
    for (const span of spans) {
      const first = row[span.start]
      if (!first) continue
      context.strokeStyle = paintFg(first)
      context.beginPath()
      context.moveTo(span.start * cellW, py)
      context.lineTo(span.end * cellW, py)
      context.stroke()
    }
  }
}

export function pointerCell(
  clientX: number,
  clientY: number,
  canvas: HTMLCanvasElement,
  frame: TerminalFrame,
  cellW = DEFAULT_CELL_W,
  cellH = DEFAULT_CELL_H,
): { readonly x: number; readonly y: number } | null {
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

