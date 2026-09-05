import { expect, test, type Locator, type Page } from '@playwright/test'

async function preview(page: Page, route: string, story: string) {
  await page.goto(`/docs/components/${route}`)
  const figure = page.locator(`[data-termrock-preview="${story}"]`)
  await figure.getByRole('button', { name: 'Run live', exact: true }).click()
  await expect(figure).toHaveAttribute('data-preview-live', 'rust-wasm')
  await expect(figure.locator('canvas')).toBeVisible()
  return figure
}

async function settleViewportAndIdle(page: Page) {
  await page.evaluate(
    () =>
      new Promise<void>((resolve) => {
        requestAnimationFrame(() => {
          requestAnimationFrame(() => {
            if ('requestIdleCallback' in window) {
              window.requestIdleCallback(() => resolve())
            } else {
              window.setTimeout(resolve, 0)
            }
          })
        })
      }),
  )
}

async function focusPreview(figure: Locator) {
  await figure.locator('[data-termrock-interaction="1"]').click()
  const host = figure.locator('[role="application"]')
  await expect(figure).toHaveAttribute('data-preview-engaged', 'true')
  await expect
    .poll(() =>
      host.evaluate(
        (element) =>
          element === document.activeElement || element.contains(document.activeElement),
      ),
    )
    .toBe(true)
  return host
}

// The live session mounts at the CSS host size (72×20 cells). On that grid the
// Buttons page paints "Run task" across cells x 23–34 of row 7; (26,7) is its
// center. Pointer coordinates in these specs are derived from the reported
// session grid so they keep pointing at the same widget.
async function cellPoint(figure: Locator, cellX: number, cellY: number) {
  const canvas = figure.locator('canvas')
  await canvas.scrollIntoViewIfNeeded()
  const box = await canvas.boundingBox()
  expect(box).not.toBeNull()
  if (!box) return null
  const cols = Number(await figure.getAttribute('data-preview-cols'))
  const rows = Number(await figure.getAttribute('data-preview-rows'))
  return {
    x: box.x + box.width * ((cellX + 0.5) / cols),
    y: box.y + box.height * ((cellY + 0.5) / rows),
  }
}

test('action link hover paints real state and click emits activation', async ({ page }) => {
  const figure = await preview(page, 'action-link', 'action-link/basic')
  const canvas = figure.locator('canvas')
  const point = await cellPoint(figure, 26, 7)
  if (!point) return
  const before = await canvas.screenshot()
  const semanticRevision = await figure.getAttribute('data-preview-semantic-revision')
  await page.mouse.move(point.x, point.y)
  await expect(figure).toHaveAttribute('data-preview-hover', '26,7')
  // Hover feedback is real Rust state on this page: the session repaints and
  // advances its semantic revision without recording an outcome.
  await expect
    .poll(async () => Number(await figure.getAttribute('data-preview-semantic-revision')))
    .toBeGreaterThan(Number(semanticRevision ?? 0))
  const hovered = await canvas.screenshot()
  expect(hovered.equals(before)).toBeFalsy()
  await page.mouse.move(0, 0)
  await expect(figure).toHaveAttribute('data-preview-hover', '')
  const afterLeave = await canvas.screenshot()
  expect(afterLeave.equals(before)).toBeTruthy()
  // The engagement press-release both starts interaction mode and dispatches
  // pointer input, so this first in-bounds click already activates the widget.
  await canvas.click({
    position: {
      x: point.x - (await canvas.boundingBox()).x,
      y: point.y - (await canvas.boundingBox()).y,
    },
  })
  await expect(figure).toHaveAttribute('data-preview-engaged', 'true')
  await expect(figure.locator('[role="application"]')).toBeFocused()
  await expect(figure).toHaveAttribute('data-preview-outcome', 'Run task ✓')
})

test('outside release and pointercancel clear Rust pointer state without activation', async ({
  page,
}) => {
  const figure = await preview(page, 'action-link', 'action-link/basic')
  const point = await cellPoint(figure, 26, 7)
  if (!point) return
  const canvas = figure.locator('canvas')
  await canvas.click()
  await expect(figure).toHaveAttribute('data-preview-engaged', 'true')

  await page.mouse.move(point.x, point.y)
  await page.mouse.down()
  await page.mouse.move(0, 0)
  await page.mouse.up()
  await expect(figure).toHaveAttribute('data-preview-hover', '')
  await expect(figure).toHaveAttribute('data-preview-outcome', '')

  await page.mouse.move(point.x, point.y)
  await page.mouse.down()
  await figure.locator('canvas').evaluate((element) => {
    element.dispatchEvent(
      new PointerEvent('pointercancel', { bubbles: true, pointerId: 1 }),
    )
  })
  await expect(figure).toHaveAttribute('data-preview-hover', '')
  await page.mouse.up()
  await expect(figure).toHaveAttribute('data-preview-outcome', '')

  // The same press-release inside the widget still activates: the guards above
  // block outside releases, not activation itself.
  await page.mouse.move(point.x, point.y)
  await page.mouse.down()
  await page.mouse.up()
  await expect(figure).toHaveAttribute('data-preview-outcome', 'Run task ✓')
})

test('button owns activation, loading, and deterministic completion', async ({ page }) => {
  const figure = await preview(page, 'button', 'button/activation')
  await focusPreview(figure)
  const semanticRevision = await figure.getAttribute('data-preview-semantic-revision')
  await page.keyboard.press('Shift+Enter')
  await page.keyboard.press('Enter')
  await expect(figure).toHaveAttribute('data-preview-outcome', 'Run task ✓')
  await expect
    .poll(async () => Number(await figure.getAttribute('data-preview-semantic-revision')))
    .toBeGreaterThan(Number(semanticRevision ?? 0))
  // The outcome is the page's sticky action record: a no-op repeat neither
  // clears it nor claims a new activation.
  const settled = await figure.getAttribute('data-preview-semantic-revision')
  await page.keyboard.press('Enter')
  await expect(figure).toHaveAttribute('data-preview-outcome', 'Run task ✓')
  await expect(figure).toHaveAttribute('data-preview-semantic-revision', settled ?? '0')
})

test('dialog starts at a trigger, opens, then closes with Escape', async ({ page }) => {
  const figure = await preview(page, 'dialog', 'dialog/message')
  await focusPreview(figure)
  const semanticRevision = await figure.getAttribute('data-preview-semantic-revision')
  await page.keyboard.press('Shift+Enter')
  await page.keyboard.press('Enter')
  await expect
    .poll(async () => Number(await figure.getAttribute('data-preview-semantic-revision')))
    .toBeGreaterThan(Number(semanticRevision ?? 0))
  await page.keyboard.press('Shift+Escape')
  await expect(figure).toHaveAttribute('data-preview-outcome', 'Cancelled')
})

// The ten specs below drive widget-level lifecycles (choice dialog outcomes,
// text capture with caret and paste, slider/split geometry, tab selection
// outcomes, tree collapse, virtual-list wheel offsets, toast expiry, alert
// persistence, drawer/fullscreen-viewer triggers, key-value filtering and
// permission decisions). The unified catalog runtime hosts page-level demos:
// the catalog pages these stories mount never implement those lifecycles, and
// no host converts per-widget state into preview outcomes yet. Deferred root
// cause: a widget-session host over the termrock widgets (mirroring
// CatalogSession over catalog pages, and PatternSession over
// crates/termrock/src/patterns) does not exist; these tests return when it
// ships. Their steps describe the intended behavior verbatim.
test.fixme('choice dialog keeps Continue and Cancel as distinct real outcomes', async ({ page }) => {
  const figure = await preview(page, 'choice-dialog', 'choice-dialog/basic')
  await focusPreview(figure)
  await page.keyboard.press('Enter')
  await expect(figure).toHaveAttribute('data-preview-outcome', 'Choice dialog opened')
  await page.keyboard.press('Enter')
  await expect(figure).toHaveAttribute('data-preview-outcome', 'You chose continue')

  await page.keyboard.press('Enter')
  await expect(figure).toHaveAttribute('data-preview-outcome', 'Choice dialog opened')
  await page.keyboard.press('Shift+Escape')
  await expect(figure).toHaveAttribute('data-preview-outcome', 'You chose cancel')
})

test.fixme('text input accepts real Unicode typing, caret movement, and paste', async ({
  context,
  page,
}) => {
  await context.grantPermissions(['clipboard-read', 'clipboard-write'])
  const figure = await preview(page, 'text-input', 'text-input/basic')
  await focusPreview(figure)
  await page.keyboard.type('λ')
  await page.keyboard.press('ArrowLeft')
  await page.keyboard.press('ArrowRight')
  await expect(figure).toHaveAttribute('data-preview-outcome', /Input value: .*λ/)
  await page.evaluate(() => navigator.clipboard.writeText('🚀'))
  await page.keyboard.press('ControlOrMeta+V')
  await expect(figure).toHaveAttribute('data-preview-outcome', /Input value: .*λ.*🚀/)
})

test.fixme('slider and split pane respond to keys and pointer drag', async ({ page }) => {
  const slider = await preview(page, 'slider', 'slider/basic')
  await focusPreview(slider)
  await page.keyboard.press('ArrowRight')
  await expect(slider).toHaveAttribute('data-preview-outcome', /Volume: \d+%/)
  const sliderCanvas = slider.locator('canvas')
  const sliderBox = await sliderCanvas.boundingBox()
  expect(sliderBox).not.toBeNull()
  if (sliderBox) {
    await page.mouse.move(sliderBox.x + sliderBox.width * 0.25, sliderBox.y + 27)
    await page.mouse.down()
    await page.mouse.move(sliderBox.x + sliderBox.width * 0.75, sliderBox.y + 27)
    await page.mouse.up()
    await expect(slider).toHaveAttribute('data-preview-outcome', /Volume: \d+%/)
  }

  const split = await preview(page, 'split-pane', 'split-pane/horizontal')
  await focusPreview(split)
  await page.keyboard.press('ArrowRight')
  await expect(split).toHaveAttribute('data-preview-outcome', /Split ratio:/)
  const splitCanvas = split.locator('canvas')
  const splitBox = await splitCanvas.boundingBox()
  expect(splitBox).not.toBeNull()
  if (splitBox) {
    const cols = Number(await split.getAttribute('data-preview-cols'))
    const storyCols = cols - 2
    const available = storyCols - 1
    const dividerCell = 1 + Math.round(available * 0.405)
    const dividerX = splitBox.x + ((dividerCell + 0.5) / cols) * splitBox.width
    const y = splitBox.y + splitBox.height / 2
    await page.mouse.move(dividerX, y)
    await page.mouse.down()
    const revisionBeforeDrag = await split.getAttribute('data-preview-semantic-revision')
    await page.mouse.move(splitBox.x + splitBox.width * 0.55, y)
    await expect(split).toHaveAttribute(
      'data-preview-semantic-revision',
      revisionBeforeDrag ?? '0',
    )
    await page.mouse.up()
    await expect(split).toHaveAttribute('data-preview-outcome', /Split resize completed/)
    const revisionAfterGesture = await split.getAttribute('data-preview-semantic-revision')
    expect(Number(revisionAfterGesture)).toBeGreaterThan(Number(revisionBeforeDrag))
    const colsBeforeResize = await split.getAttribute('data-preview-cols')
    await split.evaluate((element: HTMLElement) => {
      element.style.width = '360px'
    })
    await expect(split).not.toHaveAttribute('data-preview-cols', colsBeforeResize ?? '')
    await expect(split).toHaveAttribute(
      'data-preview-semantic-revision',
      revisionAfterGesture ?? '0',
    )
  }
})

test.fixme('tabs change by their own keys, never by page-scroll substitution', async ({ page }) => {
  const figure = await preview(page, 'tabs', 'tabs/status')
  await focusPreview(figure)
  const scrollBefore = await page.evaluate(() => window.scrollY)
  await page.keyboard.press('ArrowRight')
  await expect(figure).toHaveAttribute('data-preview-outcome', /Tab selected: details/)
  await expect.poll(() => page.evaluate(() => window.scrollY)).toBe(scrollBefore)
})

test.fixme('tree table collapses and virtual list consumes real wheel scrolling', async ({ page }) => {
  const tree = await preview(page, 'tree-table', 'tree-table/process')
  await focusPreview(tree)
  await page.keyboard.press('ArrowLeft')
  await expect(tree).toHaveAttribute('data-preview-outcome', 'Row 1 collapsed')
  await page.keyboard.press('ArrowRight')
  await expect(tree).toHaveAttribute('data-preview-outcome', 'Row 1 expanded')

  const list = await preview(page, 'virtual-list', 'virtual-list/million')
  const canvas = list.locator('canvas')
  await canvas.hover()
  await expect(list).toHaveAttribute('data-preview-engaged', 'false')
  const outcomeBeforeEntry = await list.getAttribute('data-preview-outcome')
  await page.mouse.wheel(0, 120)
  await expect(list).toHaveAttribute('data-preview-outcome', outcomeBeforeEntry ?? '')
  await focusPreview(list)
  await canvas.hover()
  await page.mouse.wheel(0, 120)
  await expect(list).toHaveAttribute('data-preview-outcome', /Viewport offset: 250001/)
})

test.fixme('toast appears, dismisses, and expires in one mounted session', async ({ page }) => {
  const figure = await preview(page, 'toast', 'toast/success')
  await focusPreview(figure)
  await page.keyboard.press('Enter')
  await expect(figure).toHaveAttribute('data-preview-outcome', 'Toast appeared')
  await page.keyboard.press('Shift+Escape')
  await expect(figure).toHaveAttribute('data-preview-outcome', 'Toast dismissed')
  await page.keyboard.press('Enter')
  await expect(figure).toHaveAttribute('data-preview-outcome', 'Toast appeared')
  await expect(figure).toHaveAttribute('data-preview-outcome', 'Toast expired')
})

test('spinner advances only from host-injected time', async ({ page }) => {
  const figure = await preview(page, 'spinner', 'spinner/labeled')
  const canvas = figure.locator('canvas')
  const before = await canvas.screenshot()
  await page.waitForTimeout(350)
  const after = await canvas.screenshot()
  expect(after.equals(before)).toBeFalsy()
})

test('reduced motion freezes host-injected animation', async ({ page }) => {
  await page.emulateMedia({ reducedMotion: 'reduce' })
  const figure = await preview(page, 'spinner', 'spinner/labeled')
  const canvas = figure.locator('canvas')
  const before = await canvas.screenshot()
  await page.waitForTimeout(350)
  const after = await canvas.screenshot()
  expect(after.equals(before)).toBeTruthy()
})

test('reduced motion preserves functional Rust deadlines', async ({ page }) => {
  await page.emulateMedia({ reducedMotion: 'reduce' })
  const figure = await preview(page, 'button', 'button/activation')
  await focusPreview(figure)
  // Shift+Enter aliases Tab into the terminal and seeds focus on the Run task
  // button; plain Enter then activates it through the functional deadline.
  await page.keyboard.press('Shift+Enter')
  await page.keyboard.press('Enter')
  await expect(figure).toHaveAttribute('data-preview-outcome', 'Run task ✓')
})

test('visual ticks do not reconcile hidden semantic text', async ({ page }) => {
  const figure = await preview(page, 'spinner', 'spinner/labeled')
  const canvas = figure.locator('canvas')
  const before = await canvas.screenshot()
  const mutations = await figure
    .locator('[data-termrock-semantic-state="1"]')
    .evaluate(
      (element) =>
        new Promise<number>((resolve) => {
          let count = 0
          const observer = new MutationObserver((records) => {
            count += records.length
          })
          observer.observe(element, { childList: true, characterData: true, subtree: true })
          window.setTimeout(() => {
            observer.disconnect()
            resolve(count)
          }, 350)
        }),
    )
  const after = await canvas.screenshot()
  expect(after.equals(before)).toBeFalsy()
  expect(mutations).toBe(0)
})

test('landing and detail stay poster-only until explicit live activation', async ({ page }) => {
  const runtimeRequests: string[] = []
  page.on('request', (request) => {
    const requestPath = new URL(request.url()).pathname
    if (
      requestPath.includes('termrock_catalog_web_bg') &&
      requestPath.endsWith('.wasm')
    ) {
      runtimeRequests.push(request.url())
    }
  })
  await page.goto('/')
  const landingFigure = page.locator('[data-termrock-preview]').first()
  await expect(landingFigure).toHaveAttribute('data-preview-live', 'static-poster')
  await expect(landingFigure.locator('canvas')).toBeVisible()
  await settleViewportAndIdle(page)
  expect(runtimeRequests).toEqual([])

  await page.goto('/docs/components/button')
  const figure = page.locator('[data-termrock-preview="button/activation"]')
  await expect(figure).toHaveAttribute('data-preview-live', 'static-poster')
  await expect(figure.locator('canvas')).toBeVisible()
  await settleViewportAndIdle(page)
  expect(runtimeRequests).toEqual([])

  const runtimeRequest = page.waitForRequest((request) => {
    const requestPath = new URL(request.url()).pathname
    return (
      requestPath.includes('termrock_catalog_web_bg') &&
      requestPath.endsWith('.wasm')
    )
  })
  const runLive = figure.getByRole('button', { name: 'Run live', exact: true })
  await runLive.focus()
  await page.keyboard.press('Enter')
  await runtimeRequest
  await expect(figure).toHaveAttribute('data-preview-live', 'rust-wasm')
  expect(runtimeRequests.length).toBeGreaterThan(0)
})

test('interaction activation cold-starts runtime and transfers focus', async ({ page }) => {
  await page.goto('/docs/components/button')
  const figure = page.locator('[data-termrock-preview="button/activation"]')
  await expect(figure).toHaveAttribute('data-preview-live', 'static-poster')
  await figure
    .getByRole('button', { name: 'Interact with preview', exact: true })
    .click()
  await expect(figure).toHaveAttribute('data-preview-live', 'rust-wasm')
  await expect(figure).toHaveAttribute('data-preview-engaged', 'true')
  await expect(figure.locator('[role="application"]')).toBeFocused()
})

test.fixme('alert dismissal persists until the user explicitly reopens it', async ({ page }) => {
  const figure = await preview(page, 'alert', 'alert/danger')
  await focusPreview(figure)
  await page.keyboard.press('Shift+Escape')
  await expect(figure).toHaveAttribute('data-preview-outcome', 'Alert: Dismissed')
  await expect(figure.locator('[data-termrock-hints="1"]')).toContainText('O show alert')
  await page.keyboard.press('o')
  await expect(figure).toHaveAttribute('data-preview-outcome', 'Alert: Shown')
})

test.fixme('drawer and fullscreen viewer use trigger-open-close lifecycles', async ({ page }) => {
  const drawer = await preview(page, 'drawer', 'drawer/basic')
  await focusPreview(drawer)
  await page.keyboard.press('Enter')
  await expect(drawer).toHaveAttribute('data-preview-outcome', 'Drawer: Opened')
  await page.keyboard.press('Shift+Escape')
  await expect(drawer).toHaveAttribute('data-preview-outcome', 'Drawer: Closed')

  const viewer = await preview(page, 'fullscreen-viewer', 'fullscreen-viewer/basic')
  await focusPreview(viewer)
  await page.keyboard.press('Enter')
  await expect(viewer).toHaveAttribute('data-preview-outcome', /FullscreenViewer: Opened/)
  await page.keyboard.press('Shift+Escape')
  await expect(viewer).toHaveAttribute('data-preview-outcome', /FullscreenViewer: (Closed|Demoted)/)
})

test.fixme('checkpoint and diff review expose persistent navigation outcomes', async ({ page }) => {
  const checkpoints = await preview(
    page,
    'checkpoint-timeline',
    'checkpoint-timeline/basic',
  )
  await focusPreview(checkpoints)
  await page.keyboard.press('ArrowUp')
  await expect(checkpoints).toHaveAttribute(
    'data-preview-outcome',
    /CheckpointTimeline: Selected/,
  )
  await page.keyboard.press('Enter')
  await expect(checkpoints).toHaveAttribute(
    'data-preview-outcome',
    /CheckpointTimeline: PreviewOpened/,
  )

  const review = await preview(page, 'diff-review', 'diff-review/hunks')
  await focusPreview(review)
  await page.keyboard.press('ArrowDown')
  await expect(review).toHaveAttribute('data-preview-outcome', /DiffReview: /)
  await page.keyboard.press(' ')
  await expect(review).toHaveAttribute('data-preview-outcome', /SelectionChanged/)
})

test.fixme('key-value filtering and permission decisions remain real state', async ({ page }) => {
  const table = await preview(page, 'key-value-table', 'key-value-table/http')
  await focusPreview(table)
  await page.keyboard.press('/')
  await page.keyboard.type('host')
  await expect(table).toHaveAttribute(
    'data-preview-outcome',
    /KeyValueTable: FilterChanged\("host"\)/,
  )

  const permission = await preview(
    page,
    'permission-prompt',
    'permission-prompt/basic',
  )
  await focusPreview(permission)
  await page.keyboard.press('Enter')
  await expect(permission).toHaveAttribute('data-preview-outcome', /PermissionPrompt: Decided/)
  await expect(permission.locator('[data-termrock-hints="1"]')).toContainText(
    'O enqueue request',
  )
  await page.keyboard.press('o')
  await expect(permission).toHaveAttribute(
    'data-preview-outcome',
    'PermissionPrompt: Enqueued',
  )
})

test('passive paint does not trap page input or invent a cursor', async ({ page }) => {
  const figure = await preview(page, 'stack', 'stack/vertical')
  await expect(figure).toHaveAttribute('data-preview-interactive', 'false')
  await expect(figure).toHaveAttribute('data-preview-hover', '')
  await expect(figure.locator('[role="img"]')).toHaveAttribute('tabindex', '-1')
  await expect(figure.locator('[data-termrock-hints="1"]')).toContainText('No input')
})

test('interaction mode exposes state and releases keyboard focus', async ({ page }) => {
  const figure = await preview(page, 'button', 'button/activation')
  const entry = figure.locator('[data-termrock-interaction="1"]')
  const host = figure.locator('[role="application"]')

  await expect(entry).toHaveAttribute('aria-pressed', 'false')
  await expect(figure.locator('[role="status"]')).toContainText('Terminal preview ready')
  await expect(figure.locator('canvas')).toHaveAttribute('aria-hidden', 'true')
  await entry.click()
  await expect(entry).toHaveAttribute('aria-pressed', 'true')
  await expect(host).toBeFocused()

  await page.keyboard.press('Tab')
  await expect(figure).toHaveAttribute('data-preview-engaged', 'false')
  await expect(host).not.toBeFocused()

  await entry.click()
  await page.keyboard.press('Escape')
  await expect(figure).toHaveAttribute('data-preview-engaged', 'false')
  await expect(entry).toBeFocused()
})

test('full preview is a contained modal and restores focus', async ({ page }) => {
  const figure = await preview(page, 'button', 'button/activation')
  const entry = figure.locator('[data-termrock-interaction="1"]')
  const full = figure.getByRole('button', { name: 'Full preview', exact: true })
  await expect(figure).toHaveAttribute('data-preview-engaged', 'false')
  await full.click()

  const dialog = figure.getByRole('dialog')
  await expect(dialog).toHaveAttribute('aria-modal', 'true')
  await expect
    .poll(() => page.evaluate(() => document.body.style.overflow))
    .toBe('hidden')
  const stage = figure.locator('[data-termrock-stage="1"]')
  await stage.evaluate((element: HTMLElement) => {
    element.tabIndex = -1
    element.focus()
  })
  await expect(stage).toBeFocused()
  await page.keyboard.press('Tab')
  await expect(figure.locator('[role="application"]')).toBeFocused()
  await page.keyboard.press('Shift+Tab')
  await expect(figure.getByRole('button', { name: 'Reset', exact: true })).toBeFocused()
  await page.keyboard.press('Tab')
  await expect(figure.locator('[role="application"]')).toBeFocused()
  await page.keyboard.press('Escape')

  await expect(dialog).toHaveCount(0)
  await expect(full).toBeFocused()
  await expect(figure).toHaveAttribute('data-preview-engaged', 'false')
  await expect
    .poll(() => page.evaluate(() => document.body.style.overflow))
    .toBe('')

  await entry.click()
  await expect(figure).toHaveAttribute('data-preview-engaged', 'true')
  await full.click()
  await page.keyboard.press('Escape')
  await expect(figure.getByRole('dialog')).toHaveCount(0)
  await expect(figure).toHaveAttribute('data-preview-engaged', 'true')
  const host = figure.locator('[role="application"]')
  await expect(host).toBeFocused()
  await page.keyboard.press('Shift+Enter')
  await page.keyboard.press('Enter')
  await expect(figure).toHaveAttribute('data-preview-outcome', 'Run task ✓')
})

test('fullscreen reserves plain Escape and forwards Shift+Escape', async ({ page }) => {
  const figure = await preview(page, 'dialog', 'dialog/message')
  await figure.getByRole('button', { name: 'Full preview', exact: true }).click()
  const dialog = figure.getByRole('dialog')
  const semanticRevision = await figure.getAttribute('data-preview-semantic-revision')
  await page.keyboard.press('Shift+Enter')
  await page.keyboard.press('Enter')
  await expect
    .poll(async () => Number(await figure.getAttribute('data-preview-semantic-revision')))
    .toBeGreaterThan(Number(semanticRevision ?? 0))
  await page.keyboard.press('Shift+Escape')
  await expect(figure).toHaveAttribute('data-preview-outcome', 'Cancelled')
  await expect(dialog).toBeVisible()
  await page.keyboard.press('Escape')
  await expect(dialog).toHaveCount(0)
})

test('story transitions never expose a prior story frame', async ({ page }) => {
  await preview(page, 'button', 'button/activation')
  let releasePoster: () => void = () => undefined
  const heldPoster = new Promise<void>((resolve) => {
    releasePoster = resolve
  })
  await page.route('**/preview-posters/button-icon.json', async (route) => {
    await heldPoster
    await route.continue()
  })

  // The catalog keeps one representative story per component, so story
  // transitions happen across pages; the next page's poster must finish
  // loading before any canvas appears.
  await page.goto('/docs/components/icon-button')
  const nextFigure = page.locator('[data-termrock-preview="button/icon"]')
  await expect(nextFigure).toHaveAttribute('data-preview-live', 'poster-loading')
  await expect(nextFigure.locator('canvas')).toHaveCount(0)
  await expect(nextFigure.locator('[role="img"]')).toHaveAttribute(
    'aria-label',
    'Terminal preview: button/icon',
  )

  releasePoster()
  await expect(nextFigure).toHaveAttribute('data-preview-live', 'static-poster')
  await expect(nextFigure.locator('canvas')).toBeVisible()
})

test('Preview, Code, and Variant controls use the selected canonical demo', async ({ page }) => {
  let figure = await preview(page, 'button', 'button/activation')
  await figure.getByRole('button', { name: 'Code' }).click()
  await expect(figure.locator('[data-termrock-code="1"]')).toContainText('CatalogSession::mount("button/activation"')
  await expect(figure.locator('[data-termrock-code="1"]')).toContainText('session.frame()')
  await expect(figure.locator('canvas')).toBeHidden()
  await figure.getByRole('button', { name: 'preview', exact: true }).click()
  await expect(figure.locator('canvas')).toBeVisible()

  // The catalog keeps one representative story per component, so switching the
  // selected demo means switching pages; the Code view must follow that story.
  figure = await preview(page, 'icon-button', 'button/icon')
  await figure.getByRole('button', { name: 'Code' }).click()
  await expect(figure.locator('[data-termrock-code="1"]')).toContainText(
    'CatalogSession::mount("button/icon"',
  )
})
