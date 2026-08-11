/**
 * Unit check of shipped Ghostty preview metric helpers (no DOM canvas required).
 * Invoked from docs quality path / local verify.
 */
import {
  adjacentSteps,
  allSteps,
  baselineForCell,
  clampStep,
  cursorCellForStep,
  fontSizeForCell,
  glyphCellSpan,
  glyphDrawX,
  isLoadStillCurrent,
  scrollThumbMetrics,
  shouldAcceptKeyEvent,
  stepDeltaFromWheel,
  stepFromPointer,
  stepFromScrollRatio,
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

assert(shouldAcceptKeyEvent('ArrowDown', 100, null), 'first key ok')
assert(
  !shouldAcceptKeyEvent('ArrowDown', 110, { key: 'ArrowDown', t: 100 }),
  'same key within window rejected',
)
assert(
  shouldAcceptKeyEvent('ArrowDown', 200, { key: 'ArrowDown', t: 100 }),
  'same key after window ok',
)
assert(
  shouldAcceptKeyEvent('ArrowUp', 105, { key: 'ArrowDown', t: 100 }),
  'different key ok',
)

const c0 = cursorCellForStep(0, 1, 42, 10)
assert(c0.x === 1 && c0.y === 1, `cursor step0 pad1 → (1,1) got ${c0.x},${c0.y}`)
const c3 = cursorCellForStep(3, 1, 42, 10)
assert(c3.y === 4, `cursor step3 pad1 → y=4 got ${c3.y}`)
const cHi = cursorCellForStep(99, 1, 42, 10)
assert(cHi.y === 8, `cursor clamp bottom got ${cHi.y}`)

// Pointer → step (list body rows after pad).
// cellH=18, pad=1 → yCss for body row 2 is (1+2)*18 + 1 = mid of row 3
assert(
  stepFromPointer(18 + 9, 0, 18, 9, 1, 5, 10, 42, 40) === 0,
  'pointer pad row maps body 0',
)
assert(
  stepFromPointer(18 * 3 + 9, 0, 18, 9, 1, 5, 10, 42, 40) === 2,
  'pointer body row 2 → step 2',
)
assert(
  stepFromPointer(9999, 0, 18, 9, 1, 5, 10, 42, 40) === 5,
  'pointer clamps to maxStep',
)
// Short wide grid uses column fraction (tabs).
assert(
  stepFromPointer(10, 9 * 20, 18, 9, 1, 3, 3, 40, 20) === 3,
  'horizontal strip uses col mapping → last step',
)

assert(stepFromScrollRatio(0, 5) === 0, 'scroll ratio 0 → step 0')
assert(stepFromScrollRatio(1, 5) === 5, 'scroll ratio 1 → max')
assert(stepFromScrollRatio(0.5, 4) === 2, 'scroll ratio mid')
assert(stepFromScrollRatio(-1, 5) === 0, 'scroll ratio clamp low')
assert(stepFromScrollRatio(2, 5) === 5, 'scroll ratio clamp high')
assert(stepFromScrollRatio(0.5, 0) === 0, 'scroll ratio static pack')

const th0 = scrollThumbMetrics(0, 5)
assert(th0.top === 0 && th0.ratio === 0, `thumb step0 top=0 got ${th0.top}`)
assert(th0.height >= 0.12 && th0.height <= 1, `thumb height ${th0.height}`)
const thLast = scrollThumbMetrics(5, 5)
assert(
  Math.abs(thLast.top + thLast.height - 1) < 1e-9,
  `thumb last sits at bottom, top+h=${thLast.top + thLast.height}`,
)
assert(thLast.ratio === 1, `thumb last ratio 1 got ${thLast.ratio}`)
const thStatic = scrollThumbMetrics(0, 0)
assert(thStatic.height === 1 && thStatic.top === 0, 'static thumb full track')

console.log('preview-metrics: ok')
