import { expect, test } from '@playwright/test'
import { TERMROCK_INSTALL_COMMAND } from '../../src/lib/install'

test('landing exposes the complete journey and generated catalog proof', async ({ page }) => {
  await page.goto('/')

  const main = page.getByRole('main')
  await expect(main).toHaveCount(1)
  await expect(main.getByRole('heading', { level: 1 })).toHaveText(
    'Build terminal software that feels finished.',
  )
  await expect(main.locator('[data-termrock-preview="agent-workbench/basic"]')).toBeVisible()
  await expect(main.getByText(TERMROCK_INSTALL_COMMAND, { exact: true })).toBeVisible()
  await expect(main.getByRole('heading', { name: 'A UI capability layer, not an application framework.' })).toBeVisible()
  await expect(main.getByRole('heading', { name: 'Choose the smallest capable primitive.' })).toBeVisible()
  await expect(main.getByRole('heading', { name: 'Change the system, not every widget.' })).toBeVisible()
  await expect(main.getByRole('heading', { name: 'Compose screens without surrendering ownership.' })).toBeVisible()
  await expect(main.getByRole('link', { name: /Command Palette/ })).toHaveAttribute(
    'href',
    '/docs/components/command-palette',
  )
  await expect(main.getByRole('link', { name: /Terminal Run Card/ })).toHaveAttribute(
    'href',
    '/docs/patterns/terminal-run-card',
  )
})

test('install command supports keyboard copy and reduced motion', async ({ page }) => {
  await page.addInitScript(`
    Object.defineProperty(navigator, 'clipboard', {
      configurable: true,
      value: { writeText: async (value) => { window.__termrockCopied = value } },
    })
  `)
  await page.emulateMedia({ reducedMotion: 'reduce' })
  await page.goto('/')

  const copy = page.getByRole('button', { name: 'Copy command' })
  await copy.focus()
  await expect(copy).toBeFocused()
  await page.keyboard.press('Enter')
  const copiedButton = page.getByRole('button', { name: 'Copied' })
  await expect(copiedButton).toBeVisible()
  await expect(page.locator('.home-copy__status')).toHaveText(
    'Install command copied to clipboard.',
  )
  await expect.poll(() =>
    page.evaluate(() =>
      (window as typeof window & { __termrockCopied?: string }).__termrockCopied,
    ),
  ).toBe(TERMROCK_INSTALL_COMMAND)

  const transitionDuration = await copiedButton.evaluate(
    (element) => getComputedStyle(element).transitionDuration,
  )
  expect(['0s', '1e-05s']).toContain(transitionDuration)
})

for (const viewport of [
  { width: 1440, height: 1000 },
  { width: 1024, height: 900 },
  { width: 430, height: 932 },
  { width: 375, height: 812 },
] as const) {
  test(`landing fits ${viewport.width}px without page overflow`, async ({ page }) => {
    await page.setViewportSize(viewport)
    await page.goto('/')

    await expect(page.getByRole('heading', { level: 1 })).toBeVisible()
    await expect(page.locator('[data-termrock-preview="agent-workbench/basic"]')).toBeVisible()
    expect(
      await page.evaluate(() => document.documentElement.scrollWidth <= window.innerWidth),
    ).toBe(true)
  })
}

test('docs home exposes the five-step information architecture', async ({ page }) => {
  await page.goto('/docs')

  await expect(page).toHaveTitle('Documentation — TermRock')
  const path = page.locator('.docs-home__path a')
  await expect(path).toHaveCount(5)
  await expect(path.locator('strong')).toHaveText([
    'Installation',
    'Getting Started',
    'Customization',
    'API',
    'Advanced',
  ])
  await expect(path.nth(0)).toHaveAttribute('href', '/docs/installation')
  await expect(path.nth(1)).toHaveAttribute('href', '/docs/getting-started')
  await expect(path.nth(3)).toHaveAttribute('href', '/docs/components')
  await expect(path.nth(4)).toHaveAttribute('href', '/docs/advanced-composition')
})
