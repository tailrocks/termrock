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
