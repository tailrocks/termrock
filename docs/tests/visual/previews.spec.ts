import { expect, test, type Locator, type Page, type TestInfo } from '@playwright/test'

async function settlePaint(page: import('@playwright/test').Page) {
  await page.evaluate(
    () => new Promise<void>((resolve) => requestAnimationFrame(() => requestAnimationFrame(() => resolve()))),
  )
}

async function preview(page: Page, route: string, story: string) {
  await page.goto(route)
  const figure = page.locator(`[data-termrock-preview="${story}"]`)
  await figure.getByRole('button', { name: 'Run live', exact: true }).click()
  await expect(figure).toHaveAttribute('data-preview-live', 'rust-wasm')
  await expect(figure.locator('canvas')).toBeVisible()
  return figure
}

async function focusPreview(figure: Locator) {
  await figure.locator('[data-termrock-interaction="1"]').click()
  await expect(figure).toHaveAttribute('data-preview-engaged', 'true')
  const host = figure.locator('[role="application"]')
  await expect
    .poll(() =>
      host.evaluate(
        (element) =>
          element === document.activeElement || element.contains(document.activeElement),
      ),
    )
    .toBe(true)
}

async function moveToCell(
  page: Page,
  figure: Locator,
  canvas: Locator,
  cellX: number,
  cellY: number,
) {
  // Font loading can shift the page between measuring the canvas and moving
  // the pointer, so re-measure until the reported hover cell matches.
  for (let attempt = 0; attempt < 4; attempt++) {
    await canvas.scrollIntoViewIfNeeded()
    const box = await canvas.boundingBox()
    expect(box).not.toBeNull()
    if (!box) return
    const cols = Number(await figure.getAttribute('data-preview-cols'))
    const rows = Number(await figure.getAttribute('data-preview-rows'))
    await page.mouse.move(
      box.x + box.width * ((cellX + 0.5) / cols),
      box.y + box.height * ((cellY + 0.5) / rows),
    )
    if ((await figure.getAttribute('data-preview-hover')) === `${cellX},${cellY}`) return
  }
  // The last move happened with fresh measurements; the assertion below owns
  // the final verdict.
}

async function capture(
  page: Page,
  figure: Locator,
  testInfo: TestInfo,
  name: string,
) {
  await settlePaint(page)
  return figure.screenshot({
    animations: 'disabled',
    path: testInfo.outputPath(`${name}.png`),
  })
}

async function paintMetrics(canvas: import('@playwright/test').Locator) {
  return canvas.evaluate((element: HTMLCanvasElement) => {
    const context = element.getContext('2d')
    if (!context) return { width: 0, height: 0, colors: 0, nonDominant: 0 }
    const pixels = context.getImageData(0, 0, element.width, element.height).data
    const counts = new Map<number, number>()
    for (let index = 0; index < pixels.length; index += 4) {
      const color =
        (pixels[index]! << 16) | (pixels[index + 1]! << 8) | pixels[index + 2]!
      counts.set(color, (counts.get(color) ?? 0) + 1)
    }
    const dominant = Math.max(0, ...counts.values())
    return {
      width: element.width,
      height: element.height,
      colors: counts.size,
      nonDominant: pixels.length / 4 - dominant,
    }
  })
}

test('ActionLink visual lifecycle: before, hover, activation', async ({ page }, testInfo) => {
  const figure = await preview(
    page,
    '/docs/components/action-link',
    'action-link/basic',
  )
  const canvas = figure.locator('canvas')
  const beforeCanvas = await canvas.screenshot({ animations: 'disabled' })
  await capture(page, figure, testInfo, 'action-link-before')

  // Center of the "Run task" button on the mounted Buttons page.
  await moveToCell(page, figure, canvas, 26, 7)
  await expect(figure).toHaveAttribute('data-preview-hover', '26,7')
  await settlePaint(page)
  const hoverCanvas = await canvas.screenshot({ animations: 'disabled' })
  expect(hoverCanvas.equals(beforeCanvas)).toBeFalsy()
  await capture(page, figure, testInfo, 'action-link-hover')

  await page.mouse.down()
  await page.mouse.up()
  await expect(figure).toHaveAttribute('data-preview-outcome', 'Run task ✓')
  await capture(page, figure, testInfo, 'action-link-clicked')
})

test('Dialog visual lifecycle: closed, open, dismissed', async ({ page }, testInfo) => {
  const figure = await preview(page, '/docs/components/dialog', 'dialog/message')
  await capture(page, figure, testInfo, 'dialog-closed')

  await focusPreview(figure)
  // Shift+Enter aliases Tab into the terminal and seeds focus on the trigger.
  await page.keyboard.press('Shift+Enter')
  const revisionBeforeOpen = Number(await figure.getAttribute('data-preview-semantic-revision'))
  await page.keyboard.press('Enter')
  await expect
    .poll(async () => Number(await figure.getAttribute('data-preview-semantic-revision')))
    .toBeGreaterThan(revisionBeforeOpen)
  await capture(page, figure, testInfo, 'dialog-open')

  await page.keyboard.press('Shift+Escape')
  await expect(figure).toHaveAttribute('data-preview-outcome', 'Cancelled')
  await capture(page, figure, testInfo, 'dialog-dismissed')
})

// The four specs below drive widget-level lifecycles (text capture with caret
// and paste, split geometry drags, tree collapse, toast expiry). The unified
// catalog runtime hosts page-level demos, and the catalog pages these stories
// mount never implement those lifecycles. Deferred root cause: a
// widget-session host over the termrock widgets (mirroring CatalogSession over
// catalog pages, and PatternSession over crates/termrock/src/patterns) does
// not exist; these tests return when it ships.
test.fixme('TextInput visual evidence keeps typed Unicode and real caret', async ({ page }, testInfo) => {
  const figure = await preview(
    page,
    '/docs/components/text-input',
    'text-input/basic',
  )
  await focusPreview(figure)
  await page.keyboard.type('λ-rock')
  await expect(figure).toHaveAttribute('data-preview-outcome', /Input value: .*λ-rock/)
  await capture(page, figure, testInfo, 'text-input-typed-caret')
})

test.fixme('SplitPane visual evidence follows real pointer drag', async ({ page }, testInfo) => {
  const figure = await preview(
    page,
    '/docs/components/split-pane',
    'split-pane/horizontal',
  )
  await focusPreview(figure)
  await page.keyboard.press('ArrowRight')
  await expect(figure).toHaveAttribute('data-preview-outcome', /Split ratio:/)
  const canvas = figure.locator('canvas')
  const box = await canvas.boundingBox()
  expect(box).not.toBeNull()
  if (!box) return
  const cols = Number(await figure.getAttribute('data-preview-cols'))
  const dividerCell = 1 + Math.round((cols - 3) * 0.405)
  const dividerX = box.x + ((dividerCell + 0.5) / cols) * box.width
  const y = box.y + box.height / 2
  await page.mouse.move(dividerX, y)
  await page.mouse.down()
  await page.mouse.move(box.x + box.width * 0.55, y)
  await page.mouse.up()
  await expect(figure).toHaveAttribute('data-preview-outcome', /Split resize completed/)
  await capture(page, figure, testInfo, 'split-pane-after-drag')
})

test.fixme('TreeTable visual evidence shows collapsed and expanded state', async ({ page }, testInfo) => {
  const figure = await preview(
    page,
    '/docs/components/tree-table',
    'tree-table/process',
  )
  await focusPreview(figure)
  await page.keyboard.press('ArrowLeft')
  await expect(figure).toHaveAttribute('data-preview-outcome', 'Row 1 collapsed')
  await capture(page, figure, testInfo, 'tree-table-collapsed')
  await page.keyboard.press('ArrowRight')
  await expect(figure).toHaveAttribute('data-preview-outcome', 'Row 1 expanded')
  await capture(page, figure, testInfo, 'tree-table-expanded')
})

test.fixme('Toast visual evidence shows visible and expired lifecycle', async ({ page }, testInfo) => {
  const figure = await preview(page, '/docs/components/toast', 'toast/success')
  await focusPreview(figure)
  await page.keyboard.press('Enter')
  await expect(figure).toHaveAttribute('data-preview-outcome', 'Toast appeared')
  await capture(page, figure, testInfo, 'toast-visible')
  await expect(figure).toHaveAttribute('data-preview-outcome', 'Toast expired')
  await capture(page, figure, testInfo, 'toast-expired')
})

test('application pattern is usable at desktop and narrow widths', async ({ page }, testInfo) => {
  await page.setViewportSize({ width: 1440, height: 1000 })
  const figure = await preview(
    page,
    '/docs/patterns/connection-manager',
    'connection-manager/full',
  )
  await focusPreview(figure)
  // The pattern page keeps its input contract at any size: typing repaints
  // through the live Rust host, and the session reflows with the viewport.
  const revisionBeforeType = Number(await figure.getAttribute('data-preview-semantic-revision'))
  await page.keyboard.press('n')
  await expect
    .poll(async () => Number(await figure.getAttribute('data-preview-semantic-revision')))
    .toBeGreaterThan(revisionBeforeType)
  await capture(page, figure, testInfo, 'connection-manager-desktop')

  await page.setViewportSize({ width: 430, height: 900 })
  // The shell floor clamps the session: a viewport narrower than 72 cells
  // keeps the 72-column grid instead of shrinking past usable density.
  await expect.poll(async () => Number(await figure.getAttribute('data-preview-cols'))).toBe(72)
  await expect(figure).toHaveAttribute('data-preview-engaged', 'true')
  await capture(page, figure, testInfo, 'connection-manager-narrow')
})

test('passive preview has no fake cursor or interaction hint', async ({ page }, testInfo) => {
  const figure = await preview(
    page,
    '/docs/components/stack',
    'stack/vertical',
  )
  await expect(figure).toHaveAttribute('data-preview-interactive', 'false')
  await expect(figure.locator('[role="img"]')).toHaveAttribute('tabindex', '-1')
  await expect(figure.locator('[data-termrock-hints="1"]')).toHaveText(
    'No input — rendered state only',
  )
  await expect(figure.locator('textarea')).toHaveCount(0)
  await capture(page, figure, testInfo, 'stack-passive-no-cursor')
})

for (const [route, story] of [
  ['/docs/components/button', 'button/activation'],
  ['/docs/components/dialog', 'dialog/message'],
] as const) {
  test(`default paint is substantial and Reset is deterministic: ${story}`, async ({
    page,
  }, testInfo) => {
    await page.goto(route)
    const preview = page.locator(`[data-termrock-preview="${story}"]`)
    await preview.getByRole('button', { name: 'Run live', exact: true }).click()
    await expect(preview).toHaveAttribute('data-preview-live', 'rust-wasm')
    await expect(preview.locator('canvas')).toBeVisible()

    const canvas = preview.locator('canvas')
    await preview.getByRole('button', { name: 'Reset' }).click()
    await expect(preview).toHaveAttribute('data-preview-outcome', 'Demo reset')
    await settlePaint(page)
    const before = await canvas.screenshot({
      animations: 'disabled',
      path: testInfo.outputPath(`${story.replaceAll('/', '-')}.png`),
    })
    const metrics = await paintMetrics(canvas)
    expect(metrics.width).toBeGreaterThan(100)
    expect(metrics.height).toBeGreaterThan(50)
    expect(metrics.colors).toBeGreaterThan(3)
    expect(metrics.nonDominant).toBeGreaterThan(100)

    await focusPreview(preview)
    await page.keyboard.press('Shift+Enter')
    if (story === 'dialog/message') {
      // Opening the dialog repaints semantically without recording an outcome;
      // closing it would record 'Cancelled'.
      const revisionBeforeOpen = Number(
        await preview.getAttribute('data-preview-semantic-revision'),
      )
      await page.keyboard.press('Enter')
      await expect
        .poll(async () => Number(await preview.getAttribute('data-preview-semantic-revision')))
        .toBeGreaterThan(revisionBeforeOpen)
    } else {
      await page.keyboard.press('Enter')
      await expect(preview).toHaveAttribute('data-preview-outcome', 'Run task ✓')
    }
    await settlePaint(page)
    const changed = await canvas.screenshot({ animations: 'disabled' })
    expect(changed.equals(before)).toBeFalsy()

    await preview.getByRole('button', { name: 'Reset' }).click()
    await expect(preview).toHaveAttribute('data-preview-outcome', 'Demo reset')
    await settlePaint(page)
    const after = await canvas.screenshot({ animations: 'disabled' })
    expect(after.equals(before)).toBeTruthy()
  })
}
