/**
 * Unit check of shipped Ghostty preview metric helpers (no DOM canvas required).
 * Invoked from docs quality path / local verify.
 */
import {
  adjacentSteps,
  allSteps,
  baselineForCell,
  clampStep,
  fontSizeForCell,
  glyphCellSpan,
  glyphDrawX,
  isLoadStillCurrent,
  stepDeltaFromWheel,
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

assert(stepDeltaFromWheel(0) === 0, 'dead zone zero')
assert(stepDeltaFromWheel(3) === 0, 'dead zone small')
assert(stepDeltaFromWheel(20) === 1, 'scroll down advances')
assert(stepDeltaFromWheel(-20) === -1, 'scroll up retreats')
assert(clampStep(-1, 5) === 0, 'clamp low')
assert(clampStep(9, 5) === 5, 'clamp high')
assert(JSON.stringify(adjacentSteps(0, 5)) === JSON.stringify([1]), 'adj start')
assert(JSON.stringify(adjacentSteps(3, 5)) === JSON.stringify([2, 4]), 'adj mid')
assert(JSON.stringify(adjacentSteps(5, 5)) === JSON.stringify([4]), 'adj end')

assert(glyphCellSpan(5, 9) === 1, 'narrow span')
assert(glyphCellSpan(18, 9) === 2, 'wide span')
assert(glyphCellSpan(0, 9) === 1, 'empty span')
assert(isLoadStillCurrent(3, 3), 'current load')
assert(!isLoadStillCurrent(2, 3), 'stale load rejected')
assert(JSON.stringify(allSteps(2)) === JSON.stringify([0, 1, 2]), 'all steps')

console.log('preview-metrics: ok')
