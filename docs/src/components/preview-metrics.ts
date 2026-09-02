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
 * True for Unicode Box Drawing / Block Elements (and common terminal line glyphs).
 * These must paint flush to the cell edge so panel borders join without hairline gaps.
 */
export function isBoxOrBlockGlyph(ch: string): boolean {
  if (!ch) return false
  const cp = ch.codePointAt(0)
  if (cp === undefined) return false
  // Box Drawing U+2500–257F, Block Elements U+2580–259F
  if (cp >= 0x2500 && cp <= 0x259f) return true
  // Half/selection bars used by TermRock list chrome (outside block range when combined)
  if (ch === '▌' || ch === '▐' || ch === '│' || ch === '┃') return true
  return false
}

/**
 * Horizontal draw origin for a glyph inside a cell.
 * Centers single-width text glyphs; box/block glyphs flush-left so seams join
 * (Ghostty/TUI panel chrome). Wide/unmeasured glyphs fall back to left+0.5.
 */
export function glyphDrawX(
  cellPx: number,
  cellW: number,
  textWidth: number,
  ch?: string,
): number {
  if (ch && isBoxOrBlockGlyph(ch)) {
    // Flush to cell origin — no +0.5 hairline; borders abut neighboring cells.
    return cellPx
  }
  const w = Math.max(1, cellW)
  if (!(textWidth > 0) || textWidth >= w - 0.25) {
    return cellPx + 0.5
  }
  return cellPx + (w - textWidth) / 2
}

/** Canvas font-weight for cell bold flag (Ghostty-class mono bold). */
export function boldFontWeight(bold: boolean | undefined): '400' | '700' {
  return bold ? '700' : '400'
}

/**
 * Underline geometry inside a cell (CSS px, top-origin).
 * Thickness scales with cell height (~10%) so selection chrome stays visible
 * on large grids; sits near the bottom with a 1px air gap when possible.
 */
export function underlineMetrics(cellH: number): {
  offsetFromTop: number
  thickness: number
} {
  const h = Math.max(1, cellH | 0)
  const thickness = Math.max(1, Math.round(h * 0.1))
  const offsetFromTop = Math.max(0, h - thickness - 1)
  return { offsetFromTop, thickness }
}

/**
 * Merge consecutive underlined cells on one row into continuous spans.
 * `end` is exclusive. Pure — host draws one stroke per span (Ghostty selection).
 */
export function underlineSpans(
  rowCells: ReadonlyArray<{ underline?: boolean } | null | undefined>,
): Array<{ start: number; end: number }> {
  const spans: Array<{ start: number; end: number }> = []
  let i = 0
  const n = rowCells.length
  while (i < n) {
    if (!rowCells[i]?.underline) {
      i++
      continue
    }
    const start = i
    while (i < n && rowCells[i]?.underline) i++
    spans.push({ start, end: i })
  }
  return spans
}

/** sRGB channel → linear (WCAG). */
function srgbToLinear(c: number): number {
  const x = Math.max(0, Math.min(255, c)) / 255
  return x <= 0.03928 ? x / 12.92 : Math.pow((x + 0.055) / 1.055, 2.4)
}

/** Relative luminance 0..1 (WCAG). */
export function relativeLuminance(rgb: [number, number, number]): number {
  const r = srgbToLinear(rgb[0])
  const g = srgbToLinear(rgb[1])
  const b = srgbToLinear(rgb[2])
  return 0.2126 * r + 0.7152 * g + 0.0722 * b
}

/** Contrast ratio ≥ 1 (WCAG). */
export function contrastRatio(
  a: [number, number, number],
  b: [number, number, number],
): number {
  const l1 = relativeLuminance(a)
  const l2 = relativeLuminance(b)
  const hi = Math.max(l1, l2)
  const lo = Math.min(l1, l2)
  return (hi + 0.05) / (lo + 0.05)
}

/**
 * Ghostty-like minimum-contrast: nudge fg toward black/white until ratio ≥ min.
 * Used after dim so faint muted text stays readable on near-bg cells.
 */
export function ensureMinContrast(
  fg: [number, number, number],
  bg: [number, number, number],
  minRatio = 1.6,
): [number, number, number] {
  const min = minRatio > 1 ? minRatio : 1
  if (contrastRatio(fg, bg) >= min) {
    return [fg[0] | 0, fg[1] | 0, fg[2] | 0]
  }
  const bgLum = relativeLuminance(bg)
  // Push toward white on dark bg, black on light bg.
  const target: [number, number, number] = bgLum < 0.5 ? [255, 255, 255] : [0, 0, 0]
  let out: [number, number, number] = [fg[0], fg[1], fg[2]]
  // Binary-ish mix toward target until ratio met (max 12 steps).
  for (let i = 0; i < 12; i++) {
    if (contrastRatio(out, bg) >= min) break
    const t = 0.2 + i * 0.07
    out = [
      Math.round(out[0] + (target[0] - out[0]) * t),
      Math.round(out[1] + (target[1] - out[1]) * t),
      Math.round(out[2] + (target[2] - out[2]) * t),
    ]
  }
  // Final clamp if still short — snap to target.
  if (contrastRatio(out, bg) < min) return target
  return [
    Math.max(0, Math.min(255, out[0])),
    Math.max(0, Math.min(255, out[1])),
    Math.max(0, Math.min(255, out[2])),
  ]
}

/**
 * Resolve paint RGB for a cell fg given dim flag and Ghostty min-contrast vs bg.
 */
export function resolvePaintFg(
  fg: [number, number, number],
  bg: [number, number, number],
  dim?: boolean,
  minRatio = 1.6,
): [number, number, number] {
  let r: [number, number, number] = [fg[0], fg[1], fg[2]]
  if (dim) {
    r = [
      Math.round(r[0] * 0.7),
      Math.round(r[1] * 0.7),
      Math.round(r[2] * 0.7),
    ]
  }
  return ensureMinContrast(r, bg, minRatio)
}

/**
 * Vector stroke plan for common terminal box/block glyphs.
 * When non-null, paint geometry instead of font glyphs so borders join exactly
 * at cell edges (Ghostty grid chrome). Unknown box glyphs return null → font path.
 *
 * Block partials use `eighths` in 1..8 (Unicode eighths ladder). Full █ is fill.
 * Shade ░▒▓ stay on the font path (null).
 */
export type BoxStrokePlan =
  | { kind: 'h'; heavy: boolean }
  | { kind: 'v'; heavy: boolean }
  | { kind: 'corner'; n: boolean; s: boolean; e: boolean; w: boolean }
  | { kind: 'fill' }
  | { kind: 'lower'; eighths: number }
  | { kind: 'upper'; eighths: number }
  | { kind: 'left'; eighths: number }
  | { kind: 'right'; eighths: number }
  /** Full-cell shade fill; density is paint alpha (░≈0.25 ▒≈0.5 ▓≈0.75). */
  | { kind: 'shade'; density: number }

/** Clamp Unicode eighths ladder to 1..8. */
export function clampEighths(n: number): number {
  if (!(n > 0) || !Number.isFinite(n)) return 1
  if (n >= 8) return 8
  return n | 0
}

export function boxStrokeForGlyph(ch: string): BoxStrokePlan | null {
  if (!ch) return null
  switch (ch) {
    case '─':
    case '┄':
    case '┈':
      return { kind: 'h', heavy: false }
    case '━':
    case '┅':
    case '┉':
      return { kind: 'h', heavy: true }
    case '│':
    case '┊':
    case '┆':
      return { kind: 'v', heavy: false }
    case '┃':
    case '┋':
    case '┇':
      return { kind: 'v', heavy: true }
    case '┌':
      return { kind: 'corner', n: false, s: true, e: true, w: false }
    case '┐':
      return { kind: 'corner', n: false, s: true, e: false, w: true }
    case '└':
      return { kind: 'corner', n: true, s: false, e: true, w: false }
    case '┘':
      return { kind: 'corner', n: true, s: false, e: false, w: true }
    case '├':
      return { kind: 'corner', n: true, s: true, e: true, w: false }
    case '┤':
      return { kind: 'corner', n: true, s: true, e: false, w: true }
    case '┬':
      return { kind: 'corner', n: false, s: true, e: true, w: true }
    case '┴':
      return { kind: 'corner', n: true, s: false, e: true, w: true }
    case '┼':
      return { kind: 'corner', n: true, s: true, e: true, w: true }
    // Full block only — partials use eighths ladders (sparklines / meters).
    case '█':
      return { kind: 'fill' }
    // Lower blocks (bottom-aligned): ▁▂▃▄▅▆▇
    case '▁':
      return { kind: 'lower', eighths: 1 }
    case '▂':
      return { kind: 'lower', eighths: 2 }
    case '▃':
      return { kind: 'lower', eighths: 3 }
    case '▄':
      return { kind: 'lower', eighths: 4 }
    case '▅':
      return { kind: 'lower', eighths: 5 }
    case '▆':
      return { kind: 'lower', eighths: 6 }
    case '▇':
      return { kind: 'lower', eighths: 7 }
    // Upper half
    case '▀':
      return { kind: 'upper', eighths: 4 }
    // Left blocks: ▏▎▍▌▋▊▉
    case '▏':
      return { kind: 'left', eighths: 1 }
    case '▎':
      return { kind: 'left', eighths: 2 }
    case '▍':
      return { kind: 'left', eighths: 3 }
    case '▌':
      return { kind: 'left', eighths: 4 }
    case '▋':
      return { kind: 'left', eighths: 5 }
    case '▊':
      return { kind: 'left', eighths: 6 }
    case '▉':
      return { kind: 'left', eighths: 7 }
    case '▐':
      return { kind: 'right', eighths: 4 }
    // Shade blocks — density for alpha fill (not solid █).
    case '░':
      return { kind: 'shade', density: 0.25 }
    case '▒':
      return { kind: 'shade', density: 0.5 }
    case '▓':
      return { kind: 'shade', density: 0.75 }
    default:
      return null
  }
}

export type BoxStrokeSeg = {
  x1: number
  y1: number
  x2: number
  y2: number
  width: number
}

export type BoxFillRect = { x: number; y: number; w: number; h: number }

/**
 * Geometry for a stroke plan in cell-local CSS pixels (absolute canvas coords).
 * Horizontal segs span the full cell width; vertical segs span full height so
 * neighboring cells' strokes abut with no gap. Partial blocks are fractional fills.
 */
export function boxStrokeGeometry(
  plan: BoxStrokePlan,
  px: number,
  py: number,
  cellW: number,
  cellH: number,
): { segs: BoxStrokeSeg[]; fills: BoxFillRect[] } {
  const w = Math.max(1, cellW)
  const h = Math.max(1, cellH)
  const mx = px + w / 2
  const my = py + h / 2
  const light = Math.max(1, Math.min(w, h) * 0.12)
  const heavy = Math.max(light * 1.6, Math.min(w, h) * 0.2)
  const segs: BoxStrokeSeg[] = []
  const fills: BoxFillRect[] = []

  if (plan.kind === 'fill') {
    fills.push({ x: px, y: py, w, h })
    return { segs, fills }
  }
  if (plan.kind === 'shade') {
    // Full cell; paint path applies density as alpha.
    fills.push({ x: px, y: py, w, h })
    return { segs, fills }
  }
  if (plan.kind === 'lower') {
    const e = clampEighths(plan.eighths)
    const fh = Math.max(1, Math.round((h * e) / 8))
    fills.push({ x: px, y: py + h - fh, w, h: fh })
    return { segs, fills }
  }
  if (plan.kind === 'upper') {
    const e = clampEighths(plan.eighths)
    const fh = Math.max(1, Math.round((h * e) / 8))
    fills.push({ x: px, y: py, w, h: fh })
    return { segs, fills }
  }
  if (plan.kind === 'left') {
    const e = clampEighths(plan.eighths)
    const fw = Math.max(1, Math.round((w * e) / 8))
    fills.push({ x: px, y: py, w: fw, h })
    return { segs, fills }
  }
  if (plan.kind === 'right') {
    const e = clampEighths(plan.eighths)
    const fw = Math.max(1, Math.round((w * e) / 8))
    fills.push({ x: px + w - fw, y: py, w: fw, h })
    return { segs, fills }
  }
  if (plan.kind === 'h') {
    const lw = plan.heavy ? heavy : light
    segs.push({ x1: px, y1: my, x2: px + w, y2: my, width: lw })
    return { segs, fills }
  }
  if (plan.kind === 'v') {
    const lw = plan.heavy ? heavy : light
    segs.push({ x1: mx, y1: py, x2: mx, y2: py + h, width: lw })
    return { segs, fills }
  }
  // corner / junction
  const lw = light
  if (plan.n) segs.push({ x1: mx, y1: py, x2: mx, y2: my, width: lw })
  if (plan.s) segs.push({ x1: mx, y1: my, x2: mx, y2: py + h, width: lw })
  if (plan.w) segs.push({ x1: px, y1: my, x2: mx, y2: my, width: lw })
  if (plan.e) segs.push({ x1: mx, y1: my, x2: px + w, y2: my, width: lw })
  return { segs, fills }
}

/** Monospace stack matching docs --font-mono / Ghostty-class host. */
export const PREVIEW_MONO_STACK =
  '"JetBrains Mono", "SF Mono", "Cascadia Mono", ui-monospace, Menlo, Consolas, monospace'

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
 * Example: `3,2 · A · #48e054/#111111`
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
