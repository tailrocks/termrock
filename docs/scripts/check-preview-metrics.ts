/**
 * Unit check of shipped Ghostty preview metric helpers (no DOM canvas required).
 * Invoked from docs quality path / local verify.
 */
import {
  adjacentSteps,
  allSteps,
  baselineForCell,
  cellAtPointer,
  clampStep,
  cursorCellForStep,
  fontSizeForCell,
  formatCellProbe,
  formatRgbHex,
  glyphCellSpan,
  glyphDrawX,
  inferCursorFromFrame,
  isLoadStillCurrent,
  scrollThumbMetrics,
  shouldAcceptKeyEvent,
  stepDeltaFromWheel,
  stepFromPointer,
  stepFromScrollRatio,
  type ProbeCell,
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

// Cell pointer mapping + RGB probe (Ghostty-class inspector).
assert(cellAtPointer(-1, 0, 9, 18, 40, 10) === null, 'outside left null')
assert(cellAtPointer(0, 0, 9, 18, 40, 10)?.x === 0, 'origin cell')
assert(cellAtPointer(9 * 3 + 1, 18 * 2 + 1, 9, 18, 40, 10)?.x === 3, 'col 3')
assert(cellAtPointer(9 * 3 + 1, 18 * 2 + 1, 9, 18, 40, 10)?.y === 2, 'row 2')
assert(cellAtPointer(9 * 50, 0, 9, 18, 40, 10) === null, 'past cols null')
assert(formatRgbHex([0, 255, 65]) === '#00ff41', `hex ${formatRgbHex([0, 255, 65])}`)
assert(formatRgbHex([255, 0, 16]) === '#ff0010', 'hex pad')
const probeCell: ProbeCell = {
  ch: 'A',
  fg: [0, 255, 65],
  bg: [28, 28, 28],
  underline: true,
}
assert(
  formatCellProbe(3, 2, probeCell) === '3,2 · A · #00ff41/#1c1c1c',
  `probe ${formatCellProbe(3, 2, probeCell)}`,
)
assert(formatCellProbe(0, 0, { ch: ' ', fg: [0, 0, 0], bg: [0, 0, 0] }).includes('·'), 'space → ·')

// Infer cursor from underline/selection paint (not only step heuristic).
const cols = 6
const rows = 4
const empty: ProbeCell = { ch: ' ', fg: [255, 255, 255], bg: [0, 0, 0] }
const grid: ProbeCell[] = Array.from({ length: cols * rows }, () => ({ ...empty }))
// pad=1 body: put underline at (2,2)
grid[2 * cols + 2] = { ch: 'A', fg: [0, 255, 65], bg: [28, 28, 28], underline: true }
const inferred = inferCursorFromFrame(grid, cols, rows, 1, 0)
assert(inferred.x === 2 && inferred.y === 2, `infer underline → 2,2 got ${inferred.x},${inferred.y}`)
// no cues → fallback step 1 pad 1 → y=2, x=1
const plain = inferCursorFromFrame(
  Array.from({ length: cols * rows }, () => ({ ...empty })),
  cols,
  rows,
  1,
  1,
)
assert(plain.x === 1 && plain.y === 2, `fallback step1 → 1,2 got ${plain.x},${plain.y}`)
// selection bar glyph
const barGrid: ProbeCell[] = Array.from({ length: cols * rows }, () => ({ ...empty }))
barGrid[1 * cols + 1] = { ch: '▌', fg: [0, 255, 65], bg: [0, 0, 0] }
const barCur = inferCursorFromFrame(barGrid, cols, rows, 0, 9)
assert(barCur.x === 1 && barCur.y === 1, `bar glyph cursor got ${barCur.x},${barCur.y}`)

console.log('preview-metrics: ok')
