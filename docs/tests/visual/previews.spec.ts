import { expect, test } from '@playwright/test'

async function settlePaint(page: import('@playwright/test').Page) {
  await page.evaluate(
    () => new Promise<void>((resolve) => requestAnimationFrame(() => requestAnimationFrame(() => resolve()))),
  )
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
