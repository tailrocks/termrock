import { expect, test, type Locator, type Page, type TestInfo } from '@playwright/test'

async function settlePaint(page: import('@playwright/test').Page) {
  await page.evaluate(
    () => new Promise<void>((resolve) => requestAnimationFrame(() => requestAnimationFrame(() => resolve()))),
  )
}

async function preview(page: Page, route: string, story: string) {
  await page.goto(route)
  const figure = page.locator(`[data-termrock-preview="${story}"]`)
  await expect(figure).toHaveAttribute('data-preview-live', 'rust-wasm')
  await expect(figure.locator('canvas')).toBeVisible()
  return figure
}

async function focusPreview(figure: Locator) {
  await figure.locator('[role="application"]').focus()
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

  await canvas.hover({ position: { x: 30, y: 27 } })
  await expect(figure).toHaveAttribute('data-preview-hover', /\d+,\d+/)
  await settlePaint(page)
  const hoverCanvas = await canvas.screenshot({ animations: 'disabled' })
  expect(hoverCanvas.equals(beforeCanvas)).toBeFalsy()
  await capture(page, figure, testInfo, 'action-link-hover')

  await canvas.click({ position: { x: 30, y: 27 } })
  await expect(figure).toHaveAttribute(
    'data-preview-outcome',
    'Action activated: cargo test',
  )
  await capture(page, figure, testInfo, 'action-link-clicked')
})

test('Dialog visual lifecycle: closed, open, dismissed', async ({ page }, testInfo) => {
  const figure = await preview(page, '/docs/components/dialog', 'dialog/message')
  await capture(page, figure, testInfo, 'dialog-closed')

  await focusPreview(figure)
  await page.keyboard.press('Enter')
  await expect(figure).toHaveAttribute('data-preview-outcome', 'Dialog opened')
  await capture(page, figure, testInfo, 'dialog-open')

  await page.keyboard.press('Escape')
  await expect(figure).toHaveAttribute(
    'data-preview-outcome',
    'Dialog closed: Escape; focus restored to Open dialog',
  )
  await capture(page, figure, testInfo, 'dialog-dismissed')
})

test('TextInput visual evidence keeps typed Unicode and real caret', async ({ page }, testInfo) => {
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

test('SplitPane visual evidence follows real pointer drag', async ({ page }, testInfo) => {
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

test('TreeTable visual evidence shows collapsed and expanded state', async ({ page }, testInfo) => {
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

test('Toast visual evidence shows visible and expired lifecycle', async ({ page }, testInfo) => {
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
  await page.keyboard.press('ArrowDown')
  await expect(figure).toHaveAttribute('data-preview-outcome', /Selected connection/)
  const desktopCols = Number(await figure.getAttribute('data-preview-cols'))
  await capture(page, figure, testInfo, 'connection-manager-desktop')

  await page.setViewportSize({ width: 430, height: 900 })
  await expect.poll(async () => Number(await figure.getAttribute('data-preview-cols'))).toBeLessThan(
    desktopCols,
  )
  await expect(figure).toHaveAttribute('data-preview-outcome', /Selected connection/)
  await capture(page, figure, testInfo, 'connection-manager-narrow')
})

test('passive preview has no fake cursor or interaction hint', async ({ page }, testInfo) => {
  const figure = await preview(
    page,
    '/docs/components/accent-rail',
    'accent-rail/actors',
  )
  await expect(figure).toHaveAttribute('data-preview-interactive', 'false')
  await expect(figure.locator('[role="img"]')).toHaveAttribute('tabindex', '-1')
  await expect(figure.locator('[data-termrock-hints="1"]')).toHaveText(
    'No input — rendered state only',
  )
  await expect(figure.locator('textarea')).toHaveCount(0)
  await capture(page, figure, testInfo, 'accent-rail-passive-no-cursor')
})

for (const [route, story] of [
  ['/docs/components/button', 'button/activation'],
  ['/docs/components/dialog', 'dialog/message'],
  ['/docs/components/text-input', 'text-input/basic'],
  ['/docs/patterns/connection-manager', 'connection-manager/full'],
] as const) {
  test(`default paint is substantial and Reset is deterministic: ${story}`, async ({
    page,
  }, testInfo) => {
    await page.goto(route)
    const preview = page.locator(`[data-termrock-preview="${story}"]`)
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

    const host = preview.locator('[role="application"]')
    await host.focus()
    if (story === 'text-input/basic') await page.keyboard.type('λ')
    else if (story === 'connection-manager/full') await page.keyboard.press('ArrowDown')
    else await page.keyboard.press('Enter')
    await expect(preview).not.toHaveAttribute('data-preview-outcome', 'Demo reset')
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
