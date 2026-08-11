/**
 * Unit check of shipped Ghostty preview metric helpers (no DOM canvas required).
 * Invoked from docs quality path / local verify.
 */
import { existsSync, readdirSync, readFileSync } from 'node:fs'
import { join } from 'node:path'
import {
  adjacentSteps,
  allSteps,
  applyNavStepAction,
  baselineForCell,
  boldFontWeight,
  boxStrokeForGlyph,
  boxStrokeGeometry,
  cellAtPointer,
  contrastRatio,
  ensureMinContrast,
  resolvePaintFg,
  clampStep,
  cursorCellForStep,
  fontSizeForCell,
  formatCellProbe,
  formatRgbHex,
  glyphCellSpan,
  glyphDrawX,
  inferCursorFromFrame,
  isBoxOrBlockGlyph,
  isLoadStillCurrent,
  combinedHostViewport,
  hostViewportSize,
  materialViewportChange,
  paintDpr,
  scrollThumbMetrics,
  shouldAcceptKeyEvent,
  stepDeltaFromNavKey,
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

// Box/block glyphs flush-left (no center hairline gaps in panel chrome).
assert(isBoxOrBlockGlyph('─'), 'box light horizontal')
assert(isBoxOrBlockGlyph('│'), 'box light vertical')
assert(isBoxOrBlockGlyph('┌'), 'box corner')
assert(isBoxOrBlockGlyph('█'), 'full block')
assert(isBoxOrBlockGlyph('▌'), 'left half block / selection bar')
assert(isBoxOrBlockGlyph('━'), 'heavy horizontal')
assert(!isBoxOrBlockGlyph('A'), 'latin not box')
assert(!isBoxOrBlockGlyph(''), 'empty not box')
assert(glyphDrawX(18, cellW, 5, '─') === 18, `box flush-left got ${glyphDrawX(18, cellW, 5, '─')}`)
assert(glyphDrawX(18, cellW, 5, 'A') !== 18, 'text still centered, not flush')
assert(boldFontWeight(true) === '700', 'bold weight 700')
assert(boldFontWeight(false) === '400', 'normal weight 400')
assert(boldFontWeight(undefined) === '400', 'undef weight 400')

// Real pack box chars must be classified as box (panel borders).
{
  const path = join(import.meta.dir, '..', 'public', 'preview-frames', 'panel-variants', '40x8', '0.json')
  const fr = JSON.parse(readFileSync(path, 'utf8')) as { cells: { ch: string }[] }
  const boxCells = fr.cells.filter((c) => c.ch && isBoxOrBlockGlyph(c.ch))
  assert(boxCells.length > 50, `panel pack box cells ${boxCells.length}`)
  for (const ch of ['─', '┌', '┐', '└', '┘']) {
    assert(
      fr.cells.some((c) => c.ch === ch),
      `panel pack contains ${ch}`,
    )
    assert(isBoxOrBlockGlyph(ch), `pack glyph ${ch} is box`)
    assert(glyphDrawX(9, 9, 4, ch) === 9, `pack ${ch} flush drawX`)
    const plan = boxStrokeForGlyph(ch)
    assert(plan, `vector plan for pack ${ch}`)
  }
  // Vector geometry: horizontal ─ spans full cell; corners reach edges.
  const hPlan = boxStrokeForGlyph('─')
  assert(hPlan?.kind === 'h', '─ is h plan')
  const hGeo = boxStrokeGeometry(hPlan!, 10, 20, 9, 18)
  assert(hGeo.segs.length === 1, '─ one segment')
  assert(hGeo.segs[0]!.x1 === 10 && hGeo.segs[0]!.x2 === 19, `─ full width segs ${JSON.stringify(hGeo.segs[0])}`)
  assert(hGeo.segs[0]!.y1 === hGeo.segs[0]!.y2, '─ horizontal')
  // Two adjacent ─ cells abut: cell0 ends at 19, cell1 starts at 19
  const hGeo2 = boxStrokeGeometry(hPlan!, 19, 20, 9, 18)
  assert(hGeo.segs[0]!.x2 === hGeo2.segs[0]!.x1, 'adjacent ─ abut')
  const vPlan = boxStrokeForGlyph('│')
  assert(vPlan?.kind === 'v', '│ is v')
  const vGeo = boxStrokeGeometry(vPlan!, 0, 0, 9, 18)
  assert(vGeo.segs[0]!.y1 === 0 && vGeo.segs[0]!.y2 === 18, '│ full height')
  const tl = boxStrokeGeometry(boxStrokeForGlyph('┌')!, 0, 0, 9, 18)
  assert(tl.segs.length === 2, '┌ two arms')
  const fill = boxStrokeGeometry(boxStrokeForGlyph('█')!, 0, 0, 9, 18)
  assert(fill.fills.length === 1 && fill.fills[0]!.w === 9, '█ fill cell')
  const half = boxStrokeGeometry(boxStrokeForGlyph('▌')!, 0, 0, 10, 18)
  assert(half.fills[0]!.w === 5, '▌ left half width (4/8)')
  assert(boxStrokeForGlyph('A') === null, 'latin no stroke plan')

  // Partial blocks must NOT be full fill (sparkline / metrics bars).
  const lowerPlans: Array<[string, number]> = [
    ['▁', 1],
    ['▂', 2],
    ['▃', 3],
    ['▄', 4],
    ['▅', 5],
    ['▆', 6],
    ['▇', 7],
  ]
  for (const [ch, e] of lowerPlans) {
    const plan = boxStrokeForGlyph(ch)
    assert(plan?.kind === 'lower' && plan.eighths === e, `${ch} lower ${e} got ${JSON.stringify(plan)}`)
    const geo = boxStrokeGeometry(plan!, 0, 0, 8, 16)
    assert(geo.fills.length === 1, `${ch} one fill`)
    const fh = geo.fills[0]!.h
    const expect = Math.max(1, Math.round((16 * e) / 8))
    assert(fh === expect, `${ch} height ${fh} want ${expect}`)
    assert(geo.fills[0]!.y === 16 - fh, `${ch} bottom-aligned`)
    assert(fh < 16 || e === 8, `${ch} not full cell (e=${e})`)
  }
  assert(boxStrokeForGlyph('▉')?.kind === 'left', '▉ left partial')
  assert(boxStrokeForGlyph('▉') && (boxStrokeForGlyph('▉') as { eighths: number }).eighths === 7, '▉ 7/8')
  const leftGeo = boxStrokeGeometry(boxStrokeForGlyph('▉')!, 0, 0, 16, 8)
  assert(leftGeo.fills[0]!.w === Math.round((16 * 7) / 8), '▉ width 7/8')
  assert(leftGeo.fills[0]!.w < 16, '▉ not full width')
  // █ alone is full fill
  assert(boxStrokeForGlyph('█')?.kind === 'fill', '█ is fill')

  // Real sparkline pack: ▅ must be lower-5, never full fill.
  {
    const sp = join(import.meta.dir, '..', 'public', 'preview-frames', 'sparkline-basic', '40x8', '0.json')
    const fr = JSON.parse(readFileSync(sp, 'utf8')) as { cells: { ch: string }[] }
    const bars = fr.cells.filter((c) => c.ch === '▅' || c.ch === '▂' || c.ch === '▁')
    assert(bars.length > 0, 'sparkline has partial bars')
    for (const c of bars) {
      const plan = boxStrokeForGlyph(c.ch)
      assert(plan && plan.kind === 'lower', `spark ${c.ch} lower plan got ${JSON.stringify(plan)}`)
      assert(plan.kind !== 'fill', `spark ${c.ch} must not full-fill`)
    }
  }
  // metrics-dashboard larger size uses ▅▆▇
  {
    const mp = join(
      import.meta.dir,
      '..',
      'public',
      'preview-frames',
      'metrics-dashboard-basic',
      '56x12',
      '0.json',
    )
    const fr = JSON.parse(readFileSync(mp, 'utf8')) as { cells: { ch: string }[] }
    for (const ch of ['▅', '▆', '▇'] as const) {
      assert(fr.cells.some((c) => c.ch === ch), `metrics pack has ${ch}`)
      const plan = boxStrokeForGlyph(ch)
      assert(plan?.kind === 'lower', `metrics ${ch} lower got ${JSON.stringify(plan)}`)
    }
  }

  // Shade blocks ░▒▓ — density fill, not solid █
  assert(boxStrokeForGlyph('░')?.kind === 'shade', '░ shade')
  assert(
    boxStrokeForGlyph('░') && (boxStrokeForGlyph('░') as { density: number }).density === 0.25,
    '░ density 0.25',
  )
  assert(
    boxStrokeForGlyph('▒') && (boxStrokeForGlyph('▒') as { density: number }).density === 0.5,
    '▒ density 0.5',
  )
  assert(
    boxStrokeForGlyph('▓') && (boxStrokeForGlyph('▓') as { density: number }).density === 0.75,
    '▓ density 0.75',
  )
  const shadeGeo = boxStrokeGeometry(boxStrokeForGlyph('░')!, 0, 0, 9, 18)
  assert(shadeGeo.fills.length === 1 && shadeGeo.fills[0]!.w === 9, '░ full-cell fill geom')
}

// Ghostty min-contrast: dim near-bg fg is boosted.
{
  const darkBg: [number, number, number] = [0, 0, 0]
  const nearBg: [number, number, number] = [10, 10, 10]
  const raw = contrastRatio(nearBg, darkBg)
  assert(raw < 1.6, `near-bg contrast low got ${raw}`)
  const boosted = ensureMinContrast(nearBg, darkBg, 1.6)
  assert(contrastRatio(boosted, darkBg) >= 1.6, `boosted ratio ${contrastRatio(boosted, darkBg)}`)
  // Already-contrasting phosphor green unchanged enough to stay green-ish.
  const green: [number, number, number] = [0, 255, 65]
  const kept = ensureMinContrast(green, darkBg, 1.6)
  assert(kept[1] >= 200, `green stays bright g=${kept[1]}`)
  // Dim + contrast path
  const dimmed = resolvePaintFg([20, 20, 20], [0, 0, 0], true, 1.6)
  assert(contrastRatio(dimmed, [0, 0, 0]) >= 1.6, `dim resolve contrast ${contrastRatio(dimmed, [0, 0, 0])}`)
  // Real pack: dim cells exist and resolve to readable fg
  {
    const path = join(import.meta.dir, '..', 'public', 'preview-frames', 'list-selection', '40x8', '0.json')
    const fr = JSON.parse(readFileSync(path, 'utf8')) as {
      cells: { fg: [number, number, number]; bg: [number, number, number]; dim?: boolean }[]
    }
    const dimCells = fr.cells.filter((c) => c.dim)
    // If pack has dim, each resolve meets min contrast
    for (const c of dimCells.slice(0, 20)) {
      const fg = resolvePaintFg(c.fg, c.bg, true, 1.6)
      assert(
        contrastRatio(fg, c.bg) >= 1.6,
        `dim cell contrast ${contrastRatio(fg, c.bg)}`,
      )
    }
  }
  // Real pack shade ░ present in catalog
  {
    let found = false
    const root = join(import.meta.dir, '..', 'public', 'preview-frames')
    for (const name of readdirSync(root)) {
      const manPath = join(root, name, 'manifest.json')
      if (!existsSync(manPath)) continue
      const man = JSON.parse(readFileSync(manPath, 'utf8')) as {
        defaultSize?: string
        sizes?: string[]
      }
      const size = man.defaultSize ?? man.sizes?.[0] ?? '40x8'
      const frPath = join(root, name, size, '0.json')
      if (!existsSync(frPath)) continue
      const fr = JSON.parse(readFileSync(frPath, 'utf8')) as { cells: { ch: string }[] }
      if (fr.cells.some((c) => c.ch === '░')) {
        found = true
        assert(boxStrokeForGlyph('░')?.kind === 'shade', 'pack ░ shade plan')
        break
      }
    }
    assert(found, 'catalog has at least one ░ shade cell for fixture')
  }
}

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
// Leftmost-body selection bar ▌ only (pad col).
const barGrid: ProbeCell[] = Array.from({ length: cols * rows }, () => ({ ...empty }))
barGrid[2 * cols + 1] = { ch: '▌', fg: [0, 255, 65], bg: [0, 0, 0] }
const barCur = inferCursorFromFrame(barGrid, cols, rows, 1, 9)
assert(barCur.x === 1 && barCur.y === 2, `leftmost ▌ cursor got ${barCur.x},${barCur.y}`)
// Mid-body ▌ is not a selection cue → fallback
const midBar: ProbeCell[] = Array.from({ length: cols * rows }, () => ({ ...empty }))
midBar[2 * cols + 3] = { ch: '▌', fg: [0, 255, 65], bg: [0, 0, 0] }
const midCur = inferCursorFromFrame(midBar, cols, rows, 1, 0)
assert(midCur.x === 1 && midCur.y === 1, `mid ▌ ignored → fallback got ${midCur.x},${midCur.y}`)
// Panel scrollbar █ (right edge) must NOT pin cursor — form pack pattern.
const scrollGrid: ProbeCell[] = Array.from({ length: cols * rows }, () => ({ ...empty }))
for (let y = 1; y <= 2; y++) {
  scrollGrid[y * cols + (cols - 1)] = { ch: '█', fg: [0, 255, 65], bg: [0, 0, 0] }
}
const scrollCur = inferCursorFromFrame(scrollGrid, cols, rows, 1, 0)
assert(
  !(scrollCur.x === cols - 1),
  `scrollbar █ must not win cursor, got ${scrollCur.x},${scrollCur.y}`,
)
assert(scrollCur.x === 1 && scrollCur.y === 1, `scrollbar █ → fallback step0 pad1`)
// Decorative › (transcript/prompt) must NOT pin cursor — workbench pattern.
const decoGrid: ProbeCell[] = Array.from({ length: cols * rows }, () => ({ ...empty }))
decoGrid[2 * cols + 2] = { ch: '›', fg: [255, 255, 255], bg: [0, 0, 0] }
const decoCur = inferCursorFromFrame(decoGrid, cols, rows, 1, 0)
assert(
  !(decoCur.x === 2 && decoCur.y === 2),
  `decorative › must not win, got ${decoCur.x},${decoCur.y}`,
)
assert(decoCur.x === 1 && decoCur.y === 1, `› → fallback step0 pad1`)

// Real frame packs: drive shipped inferCursorFromFrame on exported cells.
function loadPackCells(slug: string, size: string, step: number): {
  cells: ProbeCell[]
  cols: number
  rows: number
} {
  const path = join(import.meta.dir, '..', 'public', 'preview-frames', slug, size, `${step}.json`)
  const fr = JSON.parse(readFileSync(path, 'utf8')) as {
    cells: ProbeCell[]
    cols: number
    rows: number
  }
  return { cells: fr.cells, cols: fr.cols, rows: fr.rows }
}

// form-responsive: right-edge █ scrollbar + decorative › — must fall back, not (40,1)
{
  const { cells, cols: fc, rows: fr } = loadPackCells('form-responsive', '40x8', 0)
  const hasBar = cells.some((c) => c.ch === '█')
  const hasDeco = cells.some((c) => c.ch === '›')
  assert(hasBar, 'form pack fixture has █ scrollbar cells')
  assert(hasDeco, 'form pack fixture has › glyph')
  const cur = inferCursorFromFrame(cells, fc, fr, 1, 0)
  assert(cur.x !== 40, `form must not pin to scrollbar col 40, got ${cur.x},${cur.y}`)
  // No underline/reversed/leftmost-▌ in step0 form → step+pad fallback (1,1)
  assert(cur.x === 1 && cur.y === 1, `form step0 fallback (1,1) got ${cur.x},${cur.y}`)
}

// agent-workbench: decorative › in transcript — must not sticky (5,2)
{
  const { cells, cols: wc, rows: wr } = loadPackCells('agent-workbench-basic', '72x16', 0)
  const gt = cells.findIndex((c) => c.ch === '›')
  assert(gt >= 0, 'workbench pack has decorative ›')
  const gx = gt % wc
  const gy = Math.floor(gt / wc)
  const cur = inferCursorFromFrame(cells, wc, wr, 1, 0)
  assert(
    !(cur.x === gx && cur.y === gy),
    `workbench must not pin to › at (${gx},${gy}), got ${cur.x},${cur.y}`,
  )
  assert(cur.x === 1 && cur.y === 1, `workbench step0 fallback (1,1) got ${cur.x},${cur.y}`)
}

// list-selection step1: underline on label + leftmost ▌ — underline leftmost on row wins
{
  const { cells, cols: lc, rows: lr } = loadPackCells('list-selection', '40x8', 1)
  const cur = inferCursorFromFrame(cells, lc, lr, 1, 1)
  assert(cells[cur.y * lc + cur.x]?.underline, `list cursor on underline cell got ${cur.x},${cur.y}`)
  // First underline on selection row is col 5 ('A') in exported pack
  assert(cur.y === 2 && cur.x === 5, `list step1 underline start (5,2) got ${cur.x},${cur.y}`)
}

// HiDPI paint DPR quantize (Ghostty-class crisp cells).
assert(paintDpr(1) === 1, 'dpr 1')
assert(paintDpr(2) === 2, 'dpr 2')
assert(paintDpr(1.25) === 1.25, 'dpr 1.25')
assert(Math.abs(paintDpr(1.33) - 1.25) < 1e-9, `dpr 1.33 → ${paintDpr(1.33)}`)
assert(paintDpr(0) === 1, 'dpr 0 → 1')
assert(paintDpr(-2) === 1, 'dpr neg → 1')
assert(paintDpr(2.75) === 2.75, `dpr 2.75 → ${paintDpr(2.75)}`)
assert(paintDpr(1.1) === 1 || paintDpr(1.1) === 1.25, `dpr 1.1 → ${paintDpr(1.1)}`)

// Prefer client viewport over overflow contentRect (wide canvas trap).
{
  const vis = hostViewportSize(260, 200, 678, 400)
  assert(vis.width === 260 && vis.height === 200, `client wins got ${vis.width}x${vis.height}`)
  const fallback = hostViewportSize(0, 0, 400, 300)
  assert(fallback.width === 400 && fallback.height === 300, 'content fallback')
  const zero = hostViewportSize(0, 0, 0, 0)
  assert(zero.width === 0 && zero.height === 0, 'all zero')
}

// Combined chrome+stage: narrowed Ghostty chrome wins over stuck-wide stage.
{
  const stuck = combinedHostViewport(678, 400, 260)
  assert(stuck.width === 260, `chrome min width got ${stuck.width}`)
  assert(stuck.height === 400, `stage height kept got ${stuck.height}`)
  const both = combinedHostViewport(240, 180, 300)
  assert(both.width === 240, `stage narrower than chrome got ${both.width}`)
  const onlyChrome = combinedHostViewport(0, 0, 200, 500, 300)
  // stage falls back to content 500x300; min with chrome 200
  assert(onlyChrome.width === 200, `chrome vs content got ${onlyChrome.width}`)
  assert(onlyChrome.height === 300, `content height got ${onlyChrome.height}`)
}

assert(!materialViewportChange(100, 50, 100, 50), 'no change')
assert(materialViewportChange(100, 50, 101, 50), 'width delta 1')
assert(materialViewportChange(100, 50, 100, 52), 'height delta')
assert(!materialViewportChange(100, 50, 100.4, 50, 1), 'sub-threshold')
assert(materialViewportChange(100, 50, 102, 50, 2), 'at threshold 2')

// Pure nav key map drives host path (not reimplemented).
assert(stepDeltaFromNavKey('ArrowDown') === 1, 'down')
assert(stepDeltaFromNavKey('j') === 1, 'j')
assert(stepDeltaFromNavKey('J') === 1, 'J lowercased')
assert(stepDeltaFromNavKey('ArrowUp') === -1, 'up')
assert(stepDeltaFromNavKey('k') === -1, 'k')
assert(stepDeltaFromNavKey('Home') === 'first', 'Home')
assert(stepDeltaFromNavKey('Escape') === 'first', 'Esc')
assert(stepDeltaFromNavKey('End') === 'last', 'End')
assert(stepDeltaFromNavKey('PageDown') === 1, 'PgDn')
assert(stepDeltaFromNavKey('PageUp') === -1, 'PgUp')
assert(stepDeltaFromNavKey(' ') === 1, 'space')
assert(stepDeltaFromNavKey('Enter') === null, 'Enter not step')
assert(stepDeltaFromNavKey('Tab') === null, 'Tab not step')
assert(stepDeltaFromNavKey('') === null, 'empty')
assert(applyNavStepAction(2, 5, 1) === 3, 'apply +1')
assert(applyNavStepAction(0, 5, -1) === 0, 'apply -1 clamp')
assert(applyNavStepAction(3, 5, 'first') === 0, 'apply first')
assert(applyNavStepAction(3, 5, 'last') === 5, 'apply last')
// One physical keystroke path: same key within dedupe window still one accept.
assert(shouldAcceptKeyEvent('ArrowDown', 100, null), 'nav first accept')
assert(
  !shouldAcceptKeyEvent('ArrowDown', 120, { key: 'ArrowDown', t: 100 }),
  'nav duplicate rejected',
)

console.log('preview-metrics: ok')
