import { expect, test } from '@playwright/test'
import { TERMROCK_INSTALL_COMMAND } from '../../src/lib/install'

test('component detail copies the pinned repository install command', async ({ page }) => {
  await page.addInitScript(`
    Object.defineProperty(navigator, 'clipboard', {
      configurable: true,
      value: { writeText: async (value) => { window.__termrockCopied = value } },
    })
  `)
  await page.goto('/docs/components/action-bar')

  const install = page.locator('.doc-detail-command')
  await expect(install.getByText(TERMROCK_INSTALL_COMMAND, { exact: true })).toBeVisible()
  await install.getByRole('button', { name: 'Copy command' }).click()
  await expect(install.getByRole('button', { name: 'Copied' })).toBeVisible()
  await expect.poll(() =>
    page.evaluate(() =>
      (window as typeof window & { __termrockCopied?: string }).__termrockCopied,
    ),
  ).toBe(TERMROCK_INSTALL_COMMAND)
})
