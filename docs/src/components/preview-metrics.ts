/**
 * Pure cell metrics for Ghostty-class docs TerminalPreview paint.
 * Kept free of DOM so unit checks can drive the real helpers.
 */

/** Font size (CSS px) fitted to a cell height — ~Ghostty body scale. */
export function fontSizeForCell(cellH: number): number {
  return Math.max(11, Math.floor(Math.max(1, cellH) * 0.78))
}

/** Alphabetic baseline offset within a cell (from top). */
export function baselineForCell(cellH: number): number {
  return Math.floor(Math.max(1, cellH) * 0.78)
}

/**
 * Horizontal draw origin for a glyph inside a cell.
 * Centers single-width glyphs; falls back to left+0.5 when wide or unmeasured.
 */
export function glyphDrawX(cellPx: number, cellW: number, textWidth: number): number {
  const w = Math.max(1, cellW)
  if (!(textWidth > 0) || textWidth >= w - 0.25) {
    return cellPx + 0.5
  }
  return cellPx + (w - textWidth) / 2
}

/** Monospace stack matching docs --font-mono / Ghostty-class host. */
export const PREVIEW_MONO_STACK =
  '"JetBrains Mono", "SF Mono", "Cascadia Mono", ui-monospace, Menlo, Consolas, monospace'

/**
 * Map wheel delta to a step delta for TUI state/tour navigation.
 * Positive deltaY (scroll down) advances; negative goes back. Dead-zone ignores noise.
 */
export function stepDeltaFromWheel(deltaY: number, deadZone = 4): number {
  if (!(Math.abs(deltaY) > deadZone)) return 0
  return deltaY > 0 ? 1 : -1
}

/** Clamp step index into [0, maxStep]. */
export function clampStep(step: number, maxStep: number): number {
  const max = Math.max(0, maxStep)
  if (step < 0) return 0
  if (step > max) return max
  return step | 0
}

/** Adjacent step indices to prefetch for snappy Ghostty-style interaction. */
export function adjacentSteps(step: number, maxStep: number): number[] {
  const cur = clampStep(step, maxStep)
  const out: number[] = []
  if (cur > 0) out.push(cur - 1)
  if (cur < maxStep) out.push(cur + 1)
  return out
}

/**
 * How many terminal cells a measured glyph should occupy (1 or 2).
 * Wide CJK / emoji advances ~2 mono cells in Ghostty-class grids.
 */
export function glyphCellSpan(textWidth: number, cellW: number): number {
  const w = Math.max(1, cellW)
  if (!(textWidth > 0)) return 1
  // Round ratio; cap at 2 — terminal cells rarely paint beyond double-width.
  const span = Math.round(textWidth / w)
  if (span <= 1) return 1
  return 2
}

/** True when this async load generation is still the latest request. */
export function isLoadStillCurrent(requestId: number, latestId: number): boolean {
  return requestId === latestId
}

/** All step indices for a size pack (prefetch whole interactive graph). */
export function allSteps(maxStep: number): number[] {
  const max = Math.max(0, maxStep)
  const out: number[] = []
  for (let i = 0; i <= max; i++) out.push(i)
  return out
}

export type KeyStrokeStamp = { key: string; t: number }

/**
 * Deduplicate window-capture + React onKeyDown for the same physical keystroke.
 * Without this, Ghostty host advances two steps per ArrowDown when focused.
 */
export function shouldAcceptKeyEvent(
  key: string,
  nowMs: number,
  last: KeyStrokeStamp | null,
  windowMs = 32,
): boolean {
  if (!key) return false
  if (last && last.key === key && nowMs - last.t < windowMs) return false
  return true
}

/**
 * Ghostty-style block cursor cell for interactive previews.
 * Maps selection/tour step onto the padded body row (list/tour demos).
 */
export function cursorCellForStep(
  step: number,
  pad: number,
  cols: number,
  rows: number,
): { x: number; y: number } {
  const p = Math.max(0, pad | 0)
  const c = Math.max(1, cols | 0)
  const r = Math.max(1, rows | 0)
  const maxY = Math.max(0, r - 1)
  const bodyMax = Math.max(p, maxY - p)
  const y = Math.min(bodyMax, Math.max(p, p + Math.max(0, step | 0)))
  const x = Math.min(c - 1, p)
  return { x, y }
}

/** Phosphor block-cursor fill used when the preview host is focused. */
export const CURSOR_BLOCK_RGB: [number, number, number] = [0x00, 0xff, 0x41]
