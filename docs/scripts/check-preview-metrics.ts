/**
 * Unit check of shipped Ghostty preview metric helpers (no DOM canvas required).
 * Invoked from docs quality path / local verify.
 */
import {
  baselineForCell,
  fontSizeForCell,
  glyphDrawX,
} from '../src/components/preview-metrics'

function assert(cond: unknown, msg: string): asserts cond {
  if (!cond) throw new Error(msg)
}

assert(fontSizeForCell(18) === 14, `fontSizeForCell(18)=${fontSizeForCell(18)}`)
assert(fontSizeForCell(1) === 11, 'fontSize floor at 11')
assert(baselineForCell(18) === 14, `baselineForCell(18)=${baselineForCell(18)}`)

// Narrow glyph centers; wide glyph left+0.5
const cellW = 9
const left = glyphDrawX(0, cellW, 0)
assert(left === 0.5, `empty measure falls back to 0.5, got ${left}`)
const centered = glyphDrawX(10, cellW, 5)
assert(
  Math.abs(centered - (10 + (cellW - 5) / 2)) < 1e-9,
  `center narrow glyph, got ${centered}`,
)
const wide = glyphDrawX(0, cellW, 20)
assert(wide === 0.5, `wide glyph left-aligned, got ${wide}`)

console.log('preview-metrics: ok')
