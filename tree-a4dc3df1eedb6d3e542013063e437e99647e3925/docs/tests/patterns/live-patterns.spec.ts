import { expect, test, type Page } from '@playwright/test'

async function pattern(page: Page, slug: string, story: string) {
  await page.goto(`/docs/patterns/${slug}`)
  const figure = page.locator(`[data-termrock-preview="${story}"]`)
  await expect(figure).toHaveAttribute('data-preview-live', 'rust-wasm')
  await expect(figure.locator('canvas')).toBeVisible()
  return figure
}

for (const [slug, story] of [
  ['auth-entry', 'auth-entry/basic'],
  ['connection-manager', 'connection-manager/full'],
  ['app-shell', 'app-shell/workbench'],
  ['file-manager', 'file-manager/basic'],
  ['git-workbench', 'git-workbench/basic'],
  ['setup-wizard', 'setup-wizard/welcome'],
] as const) {
  test(`${slug} mounts one effect-free application flow`, async ({ page }) => {
    const figure = await pattern(page, slug, story)
    const host = figure.locator('[role="application"]')
    await host.focus()
    await page.keyboard.press('s')
    await expect(figure).toHaveAttribute('data-preview-outcome', 'Sidebar region collapsed')
    await page.keyboard.press('Enter')
    await expect(figure).toHaveAttribute('data-preview-outcome', 'Sample application action opened')
    await page.keyboard.press('Escape')
    await expect(figure).toHaveAttribute('data-preview-outcome', 'Sample application action cancelled')
  })
}

test('full preview resizes the same mounted session', async ({ page }) => {
  const figure = await pattern(page, 'connection-manager', 'connection-manager/full')
  const before = await figure.getAttribute('data-preview-cols')
  await figure.getByRole('button', { name: 'Full preview' }).click()
  await expect(figure.getByRole('button', { name: 'Exit full preview' })).toBeVisible()
  await expect.poll(() => figure.getAttribute('data-preview-cols')).not.toBe(before)
  await expect(figure).toHaveAttribute('data-preview-live', 'rust-wasm')
})
