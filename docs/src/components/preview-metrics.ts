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

/**
 * Quantize `devicePixelRatio` for crisp Ghostty-class cell paint.
 * Fractional DPR (1.25 / 1.33 / 2.75) is rounded to 0.25 steps so the canvas
 * backing store aligns better with integer cell metrics.
 */
export function paintDpr(raw: number): number {
  if (!(raw > 0) || !Number.isFinite(raw)) return 1
  const q = Math.round(raw * 4) / 4
  return Math.max(1, q)
}

/**
 * Visible host viewport for size remap (Ghostty window-step feel).
 * Prefer `clientWidth`/`clientHeight` over ResizeObserver `contentRect` —
 * with overflow:auto stages, contentRect can track the wide canvas and stick
 * remaps on large packs while the visible box is narrow.
 */
export function hostViewportSize(
  clientW: number,
  clientH: number,
  contentW: number,
  contentH: number,
): { width: number; height: number } {
  const w = clientW > 0 ? clientW : contentW
  const h = clientH > 0 ? clientH : contentH
  return {
    width: Math.max(0, Number.isFinite(w) ? w : 0),
    height: Math.max(0, Number.isFinite(h) ? h : 0),
  }
}

/**
 * True when the visible viewport moved enough to warrant a size-pack re-pick.
 * Pure — host uses this to skip no-op reconciles.
 */
export function materialViewportChange(
  prevW: number,
  prevH: number,
  nextW: number,
  nextH: number,
  thresholdPx = 1,
): boolean {
  const t = Math.max(0, thresholdPx)
  return Math.abs(nextW - prevW) >= t || Math.abs(nextH - prevH) >= t
}

/**
 * Ghostty window viewport for remap: min of stage + chrome **width**.
 * Height stays stage-only (chrome includes title/status bars).
 * When overflow leaves stage.clientWidth stuck wide, a narrowed chrome
 * host still drives the correct smaller pack.
 */
export function combinedHostViewport(
  stageClientW: number,
  stageClientH: number,
  chromeClientW: number,
  stageContentW = 0,
  stageContentH = 0,
): { width: number; height: number } {
  const stage = hostViewportSize(
    stageClientW,
    stageClientH,
    stageContentW,
    stageContentH,
  )
  const chromeW =
    chromeClientW > 0 && Number.isFinite(chromeClientW) ? chromeClientW : 0
  const width =
    chromeW > 0 && stage.width > 0
      ? Math.min(stage.width, chromeW)
      : stage.width > 0
        ? stage.width
        : chromeW
  return { width: Math.max(0, width), height: Math.max(0, stage.height) }
}

/**
 * Pure TUI nav key → step action used by TerminalPreview host.
 * - `+1` / `-1`: relative step
 * - `'first'` / `'last'`: Home/End (and Esc → first)
 * - `null`: not a preview nav key (host ignores)
 */
export type NavStepAction = 1 | -1 | 'first' | 'last'

export function stepDeltaFromNavKey(rawKey: string): NavStepAction | null {
  if (!rawKey) return null
  // Normalize single-char keys to lowercase for vim-style j/k/h/l.
  const key = rawKey.length === 1 ? rawKey.toLowerCase() : rawKey
  if (
    key === 'ArrowDown' ||
    key === 'ArrowRight' ||
    key === 'j' ||
    key === 'l' ||
    key === 'PageDown' ||
    key === ' ' ||
    key === 'Spacebar'
  ) {
    return 1
  }
  if (
    key === 'ArrowUp' ||
    key === 'ArrowLeft' ||
    key === 'k' ||
    key === 'h' ||
    key === 'PageUp'
  ) {
    return -1
  }
  if (key === 'Home' || key === 'Escape') return 'first'
  if (key === 'End') return 'last'
  return null
}

/**
 * Apply a nav action to the current step (pure clamp for host/tests).
 */
export function applyNavStepAction(
  step: number,
  maxStep: number,
  action: NavStepAction,
): number {
  if (action === 'first') return 0
  if (action === 'last') return clampStep(maxStep, maxStep)
  return clampStep(step + action, maxStep)
}

/**
 * Map pointer position over the terminal canvas to a pack step index.
 * Prefer body-row mapping (lists/tours); short wide grids (tabs) use column fraction.
 * Pure — drives the real host path (no reimplementation in tests).
 */
export function stepFromPointer(
  yCss: number,
  xCss: number,
  cellH: number,
  cellW: number,
  pad: number,
  maxStep: number,
  rows: number,
  cols: number,
  storyCols: number,
): number {
  const ch = Math.max(1, cellH)
  const cw = Math.max(1, cellW)
  const p = Math.max(0, pad | 0)
  const max = Math.max(0, maxStep | 0)
  const row = Math.floor(yCss / ch)
  const col = Math.floor(xCss / cw)
  const bodyRow = Math.max(0, row - p)
  const bodyCol = Math.max(0, col - p)
  let next = Math.min(max, bodyRow)
  // Short tab strips / horizontal controls: map x across story width.
  if (rows <= 4 && cols > 8) {
    const sc = Math.max(1, storyCols | 0)
    const colStep = Math.floor((bodyCol / sc) * (max + 1))
    next = Math.min(max, Math.max(0, colStep))
  }
  return clampStep(next, max)
}

/**
 * Map a vertical scrollbar track ratio in [0, 1] to a step (Ghostty-like jump).
 * 0 → first step, 1 → last step.
 */
export function stepFromScrollRatio(ratio: number, maxStep: number): number {
  const max = Math.max(0, maxStep | 0)
  if (max === 0) return 0
  const r = Number.isFinite(ratio) ? ratio : 0
  const t = r <= 0 ? 0 : r >= 1 ? 1 : r
  return clampStep(Math.round(t * max), max)
}

/**
 * Overlay scrollbar thumb geometry in track-normalized units [0, 1].
 * Thumb height shrinks with more steps (min 12% of track); top follows step.
 */
export function scrollThumbMetrics(
  step: number,
  maxStep: number,
): { top: number; height: number; ratio: number } {
  const max = Math.max(0, maxStep | 0)
  const cur = clampStep(step, max)
  if (max === 0) {
    return { top: 0, height: 1, ratio: 0 }
  }
  const height = Math.max(0.12, 1 / (max + 1))
  const travel = 1 - height
  const ratio = cur / max
  const top = ratio * travel
  return { top, height, ratio }
}

/** Minimal cell shape for hover/cursor inference (matches FrameCell fields used). */
export type ProbeCell = {
  ch: string
  fg: [number, number, number]
  bg: [number, number, number]
  underline?: boolean
  reversed?: boolean
}

/**
 * Map canvas CSS pointer coords to a grid cell, or null if outside the grid.
 */
export function cellAtPointer(
  xCss: number,
  yCss: number,
  cellW: number,
  cellH: number,
  cols: number,
  rows: number,
): { x: number; y: number } | null {
  const cw = Math.max(1, cellW)
  const ch = Math.max(1, cellH)
  const c = Math.max(1, cols | 0)
  const r = Math.max(1, rows | 0)
  if (!(xCss >= 0) || !(yCss >= 0)) return null
  const x = Math.floor(xCss / cw)
  const y = Math.floor(yCss / ch)
  if (x < 0 || y < 0 || x >= c || y >= r) return null
  return { x, y }
}

/** Format RGB triple as #rrggbb (lowercase). */
export function formatRgbHex(rgb: [number, number, number]): string {
  const h = (n: number) => {
    const v = Math.max(0, Math.min(255, n | 0))
    return v.toString(16).padStart(2, '0')
  }
  return `#${h(rgb[0])}${h(rgb[1])}${h(rgb[2])}`
}

/**
 * Status-bar probe string for a hovered cell (Ghostty-class cell inspector).
 * Example: `3,2 · A · #00ff41/#1c1c1c`
 */
export function formatCellProbe(
  x: number,
  y: number,
  cell: ProbeCell | null | undefined,
): string {
  if (!cell) return `${x | 0},${y | 0}`
  const ch = cell.ch && cell.ch !== ' ' && cell.ch !== '\u00a0' ? cell.ch : '·'
  return `${x | 0},${y | 0} · ${ch} · ${formatRgbHex(cell.fg)}/${formatRgbHex(cell.bg)}`
}

/**
 * Infer block-cursor cell from real frame paint.
 *
 * Cues (strict — chrome glyphs must NOT win):
 * 1. Underline / reverse video (list focus, selected fields)
 * 2. Leftmost-body selection bar `▌` only (not full-block `█` scrollbars,
 *    not decorative `›` / `❯` / `▶` in transcripts/prompts)
 *
 * Falls back to step+pad heuristic when no safe cue is found.
 */
export function inferCursorFromFrame(
  cells: readonly ProbeCell[],
  cols: number,
  rows: number,
  pad: number,
  fallbackStep: number,
): { x: number; y: number } {
  const c = Math.max(1, cols | 0)
  const r = Math.max(1, rows | 0)
  const p = Math.max(0, pad | 0)
  const x0 = Math.min(c - 1, p)
  const x1 = Math.max(x0, c - 1 - p)
  const y0 = Math.min(r - 1, p)
  const y1 = Math.max(y0, r - 1 - p)
  // Body left column for selection bars (pad cell). Never treat right-edge █
  // panel scrollbars or decorative › as cursor.
  const barCol = x0
  const rank = (cell: ProbeCell | undefined, x: number): number => {
    if (!cell) return 0
    if (cell.underline) return 3
    if (cell.reversed) return 2
    // Only half-block selection bar in the leftmost body column.
    if (cell.ch === '▌' && x === barCol) return 1
    return 0
  }
  let best: { x: number; y: number; score: number } | null = null
  for (let y = y0; y <= y1; y++) {
    for (let x = x0; x <= x1; x++) {
      const cell = cells[y * c + x]
      const score = rank(cell, x)
      if (score <= 0) continue
      // Higher score wins; ties prefer topmost then leftmost (scan order).
      if (
        !best ||
        score > best.score ||
        (score === best.score && (y < best.y || (y === best.y && x < best.x)))
      ) {
        best = { x, y, score }
      }
    }
  }
  if (best) return { x: best.x, y: best.y }
  return cursorCellForStep(fallbackStep, p, c, r)
}
