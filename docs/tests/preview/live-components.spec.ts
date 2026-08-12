import { expect, test, type Locator, type Page } from '@playwright/test'

async function preview(page: Page, route: string, story: string) {
  await page.goto(`/docs/components/${route}`)
  const figure = page.locator(`[data-termrock-preview="${story}"]`)
  await expect(figure).toHaveAttribute('data-preview-live', 'rust-wasm')
  await expect(figure.locator('canvas')).toBeVisible()
  return figure
}

async function focusPreview(figure: Locator) {
  const host = figure.locator('[role="application"]')
  await host.focus()
  return host
}

test('action link hover paints real state and click emits activation', async ({ page }) => {
  const figure = await preview(page, 'action-link', 'action-link/basic')
  const canvas = figure.locator('canvas')
  const before = await canvas.screenshot()
  await canvas.hover({ position: { x: 30, y: 27 } })
  await expect(figure).toHaveAttribute('data-preview-hover', /\d+,\d+/)
  const hovered = await canvas.screenshot()
  expect(hovered.equals(before)).toBeFalsy()
  await canvas.click({ position: { x: 30, y: 27 } })
  await expect(figure).toHaveAttribute(
    'data-preview-outcome',
    'Action activated: cargo test',
  )
})

test('button owns activation, loading, and deterministic completion', async ({ page }) => {
  const figure = await preview(page, 'button', 'button/activation')
  await focusPreview(figure)
  await page.keyboard.press('Enter')
  await expect(figure).toHaveAttribute('data-preview-outcome', 'Save started')
  await expect(figure).toHaveAttribute('data-preview-outcome', 'Saved successfully')
})

test('dialog starts at a trigger, opens, then closes with Escape', async ({ page }) => {
  const figure = await preview(page, 'dialog', 'dialog/message')
  await focusPreview(figure)
  await page.keyboard.press('Enter')
  await expect(figure).toHaveAttribute('data-preview-outcome', 'Dialog opened')
  await page.keyboard.press('Escape')
  await expect(figure).toHaveAttribute(
    'data-preview-outcome',
    'Dialog closed: Escape; focus restored to Open dialog',
  )
})

test('choice dialog keeps Continue and Cancel as distinct real outcomes', async ({ page }) => {
  const figure = await preview(page, 'choice-dialog', 'choice-dialog/basic')
  await focusPreview(figure)
  await page.keyboard.press('Enter')
  await expect(figure).toHaveAttribute('data-preview-outcome', 'Choice dialog opened')
  await page.keyboard.press('Enter')
  await expect(figure).toHaveAttribute('data-preview-outcome', 'You chose continue')

  await page.keyboard.press('Enter')
  await expect(figure).toHaveAttribute('data-preview-outcome', 'Choice dialog opened')
  await page.keyboard.press('Escape')
  await expect(figure).toHaveAttribute('data-preview-outcome', 'You chose cancel')
})

test('text input accepts real Unicode typing, caret movement, and paste', async ({
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

test('slider and split pane respond to keys and pointer drag', async ({ page }) => {
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
    const y = splitBox.y + splitBox.height / 2
    await page.mouse.move(splitBox.x + splitBox.width * 0.38, y)
    await page.mouse.down()
    await page.mouse.move(splitBox.x + splitBox.width * 0.55, y)
    await page.mouse.up()
    await expect(split).toHaveAttribute('data-preview-outcome', /Split resize completed/)
  }
})

test('tabs change by their own keys, never by page-scroll substitution', async ({ page }) => {
  const figure = await preview(page, 'tabs', 'tabs/status')
  await focusPreview(figure)
  const scrollBefore = await page.evaluate(() => window.scrollY)
  await page.keyboard.press('ArrowRight')
  await expect(figure).toHaveAttribute('data-preview-outcome', /Tab selected: details/)
  await expect.poll(() => page.evaluate(() => window.scrollY)).toBe(scrollBefore)
})

test('tree table collapses and virtual list consumes real wheel scrolling', async ({ page }) => {
  const tree = await preview(page, 'tree-table', 'tree-table/process')
  await focusPreview(tree)
  await page.keyboard.press('ArrowLeft')
  await expect(tree).toHaveAttribute('data-preview-outcome', 'Row 1 collapsed')
  await page.keyboard.press('ArrowRight')
  await expect(tree).toHaveAttribute('data-preview-outcome', 'Row 1 expanded')

  const list = await preview(page, 'virtual-list', 'virtual-list/million')
  const canvas = list.locator('canvas')
  await canvas.hover()
  await page.mouse.wheel(0, 120)
  await expect(list).toHaveAttribute('data-preview-outcome', /Viewport offset: 250001/)
})

test('toast appears, dismisses, and expires in one mounted session', async ({ page }) => {
  const figure = await preview(page, 'toast', 'toast/success')
  await focusPreview(figure)
  await page.keyboard.press('Enter')
  await expect(figure).toHaveAttribute('data-preview-outcome', 'Toast appeared')
  await page.keyboard.press('Escape')
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

test('passive paint does not trap page input or invent a cursor', async ({ page }) => {
  const figure = await preview(page, 'accent-rail', 'accent-rail/actors')
  await expect(figure).toHaveAttribute('data-preview-interactive', 'false')
  await expect(figure.locator('[role="img"]')).toHaveAttribute('tabindex', '-1')
  await expect(figure.locator('[data-termrock-hints="1"]')).toContainText('No input')
})
