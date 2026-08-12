import { expect, test } from '@playwright/test'

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
    const before = await canvas.screenshot({
      animations: 'disabled',
      path: testInfo.outputPath(`${story.replaceAll('/', '-')}.png`),
    })
    expect(before.length).toBeGreaterThan(5_000)
    await preview.getByRole('button', { name: 'Reset' }).click()
    await expect(preview).toHaveAttribute('data-preview-outcome', 'Demo reset')
    const after = await canvas.screenshot({ animations: 'disabled' })
    expect(after.equals(before)).toBeTruthy()
  })
}
