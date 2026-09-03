/**
 * Deterministic checks for the retained terminal painter and live Rust/WASM host.
 */
import init, {
  demo_frame,
  dispatch_demo,
  mount_demo,
  unmount_demo,
} from '../src/generated/termrock-preview/termrock_catalog_web.js'
import {
  baselineForCell,
  boldFontWeight,
  boxStrokeForGlyph,
  boxStrokeGeometry,
  cellAtPointer,
  contrastRatio,
  ensureMinContrast,
  fontSizeForCell,
  formatCellProbe,
  glyphCellSpan,
  glyphDrawX,
  isBoxOrBlockGlyph,
  paintDpr,
  resolvePaintFg,
  underlineMetrics,
  underlineSpans,
  type ProbeCell,
} from '../src/components/preview-metrics'
import {
  PreviewKeyAliases,
  decidePreviewKey,
  shouldCapturePreviewKey,
} from '../src/components/preview/input'
import type {
  DemoDescriptor,
  DemoUpdate,
  TerminalFrame,
} from '../src/components/preview/model'
import { previewClockPolicy } from '../src/components/preview/motion'

function assert(condition: unknown, message: string): asserts condition {
  if (!condition) throw new Error(message)
}

assert(fontSizeForCell(18) === 14, '18px cells use a 14px font')
assert(baselineForCell(18) === 14, '18px cells use the accepted baseline')
assert(boldFontWeight(true) === '700', 'bold cells use weight 700')
assert(paintDpr(2.8) === 2.75, 'device scale is quarter-step quantized for crisp cells')
assert(glyphCellSpan(20, 9) === 2, 'wide grapheme occupies two cells')
assert(glyphDrawX(18, 9, 5, '─') === 18, 'box glyphs meet the cell edge')
assert(isBoxOrBlockGlyph('┌'), 'panel corner uses vector geometry')
assert(!isBoxOrBlockGlyph('A'), 'text remains font-painted')

const horizontal = boxStrokeForGlyph('─')
assert(horizontal?.kind === 'h', 'horizontal border has a vector plan')
const geometry = boxStrokeGeometry(horizontal, 0, 0, 9, 18)
assert(
  geometry.segs.some((segment) => segment.x1 === 0 && segment.x2 === 9),
  'horizontal border spans the complete cell',
)

assert(contrastRatio([255, 255, 255], [0, 0, 0]) > 20, 'contrast math')
const corrected = ensureMinContrast([20, 20, 20], [0, 0, 0], 4.5)
assert(contrastRatio(corrected, [0, 0, 0]) >= 4.5, 'minimum contrast correction')
assert(
  resolvePaintFg([80, 80, 80], [0, 0, 0], true)[0] <
    resolvePaintFg([80, 80, 80], [0, 0, 0], false)[0],
  'dim modifier changes resolved foreground',
)

const empty: ProbeCell = { ch: ' ', fg: [255, 255, 255], bg: [0, 0, 0] }
const cells = Array.from({ length: 8 }, () => ({ ...empty }))
cells[1] = { ...empty, ch: 'A', underline: true }
cells[2] = { ...empty, ch: 'B', underline: true }
assert(underlineSpans(cells).length === 1, 'adjacent underlines become one stroke')
assert(underlineMetrics(18).thickness >= 1, 'underline stays visible')
assert(cellAtPointer(10, 20, 9, 18, 4, 2)?.x === 1, 'pointer maps to exact cell')
assert(formatCellProbe(1, 0, cells[1]!).includes('A'), 'cell probe describes paint')

const passive: DemoDescriptor = {
  id: 'accent-rail/actors',
  title: 'Accent rail actors',
  component: 'AccentRail',
  description: '',
  cols: 40,
  rows: 8,
  interactive: false,
  interactionKind: 'passive-paint',
  hints: [],
}
assert(!shouldCapturePreviewKey('ArrowDown', passive), 'passive preview does not trap navigation')
assert(!shouldCapturePreviewKey('x', passive), 'passive preview does not trap typing')
const editor = { ...passive, interactive: true, interactionKind: 'editor-form' }
assert(shouldCapturePreviewKey('ArrowLeft', editor), 'editor captures cursor movement')
assert(shouldCapturePreviewKey('λ', editor), 'editor captures Unicode text')

const idleUpdate: DemoUpdate = {
  changed: false,
  outcome: null,
  hints: [],
  interactive: true,
  capturesTextInput: false,
  nextDeadlineMs: null,
  deadlineKind: null,
  semanticRevision: 0,
}
const tabAlias = decidePreviewKey('Enter', true, true, editor, idleUpdate)
assert(tabAlias.kind === 'dispatch' && tabAlias.key === 'Tab', 'Shift+Enter aliases Tab')
const aliases = new PreviewKeyAliases()
aliases.remember('Enter', tabAlias)
const aliasedRelease = aliases.release('Enter')
assert(
  aliasedRelease?.key === 'Tab' && !aliasedRelease.shiftKey,
  'keyup retains its keydown alias after Shift is released',
)

const functionalUpdate = {
  ...idleUpdate,
  nextDeadlineMs: 900,
  deadlineKind: 'functional',
} satisfies DemoUpdate
assert(
  previewClockPolicy(functionalUpdate, true) === 'functional-deadline',
  'reduced motion preserves functional deadlines',
)
const visualUpdate = {
  ...functionalUpdate,
  deadlineKind: 'visual-motion',
} satisfies DemoUpdate
assert(
  previewClockPolicy(visualUpdate, true) === 'stopped',
  'reduced motion suppresses decorative clocks',
)

const wasm = await Bun.file(
  new URL(
    '../src/generated/termrock-preview/termrock_catalog_web_bg.wasm',
    import.meta.url,
  ),
).arrayBuffer()
await init({ module_or_path: wasm })

function frame(handle: number): TerminalFrame {
  return JSON.parse(demo_frame(handle)) as TerminalFrame
}

const tabs = mount_demo('tabs/status', 80, 24)
const tabsBefore = frame(tabs)
JSON.parse(
  dispatch_demo(
    tabs,
    JSON.stringify({ type: 'key', key: 'Tab', kind: 'press' }),
  ),
) as { changed: boolean; hints: string[] }
const tabsUpdate = JSON.parse(
  dispatch_demo(
    tabs,
    JSON.stringify({ type: 'key', key: 'ArrowRight', kind: 'press' }),
  ),
) as { changed: boolean; hints: string[] }
const tabsAfter = frame(tabs)
assert(tabsUpdate.changed, 'real Tabs state accepts ArrowRight')
assert(JSON.stringify(tabsBefore.cells) !== JSON.stringify(tabsAfter.cells), 'Tabs paint changes')
assert(tabsUpdate.hints.includes('← →'), 'Tabs publishes exact navigation hint')
unmount_demo(tabs)

const spinner = mount_demo('spinner/labeled', 80, 24)
const spinnerStarted = JSON.parse(
  dispatch_demo(spinner, JSON.stringify({ type: 'tick', elapsedMs: 0 })),
) as DemoUpdate
assert(spinnerStarted.deadlineKind === 'visual-motion', 'Rust owns visual deadline kind')
const spinnerBefore = frame(spinner)
const spinnerAdvanced = JSON.parse(
  dispatch_demo(spinner, JSON.stringify({ type: 'tick', elapsedMs: 880 })),
) as DemoUpdate
const spinnerAfter = frame(spinner)
assert(
  JSON.stringify(spinnerBefore.cells) !== JSON.stringify(spinnerAfter.cells),
  'host-controlled time advances the Rust Spinner',
)
assert(
  spinnerAdvanced.semanticRevision === spinnerStarted.semanticRevision,
  'decorative Rust ticks preserve semantic revision',
)
unmount_demo(spinner)

const button = mount_demo('button/activation', 80, 24)
JSON.parse(
  dispatch_demo(
    button,
    JSON.stringify({ type: 'key', key: 'Tab', kind: 'press' }),
  ),
) as DemoUpdate
const buttonActivated = JSON.parse(
  dispatch_demo(button, JSON.stringify({ type: 'key', key: 'Enter', kind: 'press' })),
) as DemoUpdate
assert(buttonActivated.deadlineKind === 'functional', 'Rust owns functional deadline kind')
assert(buttonActivated.semanticRevision > 0, 'Rust advances semantic activation revision')
unmount_demo(button)

const paintedOnly = mount_demo('accent-rail/actors', 40, 8)
const passiveUpdate = JSON.parse(
  dispatch_demo(
    paintedOnly,
    JSON.stringify({ type: 'key', key: 'ArrowDown', kind: 'press' }),
  ),
) as { changed: boolean; hints: string[] }
assert(!passiveUpdate.changed, 'passive Rust demo ignores fake interaction')
assert(passiveUpdate.hints.length > 0, 'Rust host preserves owning-page hints')
unmount_demo(paintedOnly)

let rejected = false
try {
  demo_frame(4_294_967_295)
} catch {
  rejected = true
}
assert(rejected, 'unknown WASM handles return an error')

console.log('live preview painter + Rust/WASM runtime checks passed')
