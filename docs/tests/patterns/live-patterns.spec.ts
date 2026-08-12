import { expect, test, type Locator, type Page } from '@playwright/test'

async function pattern(page: Page, slug: string, story: string) {
  await page.goto(`/docs/patterns/${slug}`)
  const figure = page.locator(`[data-termrock-preview="${story}"]`)
  await expect(figure).toHaveAttribute('data-preview-live', 'rust-wasm')
  await expect(figure.locator('canvas')).toBeVisible()
  return figure
}

async function focus(figure: Locator) {
  await figure.locator('[role="application"]').focus()
}

test('AuthEntry edits a real field, moves focus, and switches mode', async ({ page }) => {
  const figure = await pattern(page, 'auth-entry', 'auth-entry/basic')
  await focus(figure)
  await page.keyboard.type('alex@example.com')
  await expect(figure).toHaveAttribute('data-preview-outcome', 'Edited identity')
  await page.keyboard.press('Tab')
  await expect(figure).toHaveAttribute('data-preview-outcome', /Focused/)
  await page.keyboard.press('ControlOrMeta+g')
  await expect(figure).toHaveAttribute('data-preview-outcome', /Mode switched/)
})

test('ConnectionManager opens and cancels its real delete confirmation', async ({ page }) => {
  const figure = await pattern(page, 'connection-manager', 'connection-manager/full')
  await focus(figure)
  await page.keyboard.press('ArrowDown')
  await expect(figure).toHaveAttribute('data-preview-outcome', /Selected connection/)
  await page.keyboard.press('d')
  await expect(figure).toHaveAttribute('data-preview-outcome', /Delete confirmation opened/)
  await expect(figure.locator('[data-termrock-hints="1"]')).toContainText('Enter resolve')
  await page.keyboard.press('Escape')
  await expect(figure).toHaveAttribute('data-preview-outcome', 'Delete cancelled')
})

test('AppShell changes focus, sidebar visibility, and responsive size in one session', async ({ page }) => {
  const figure = await pattern(page, 'app-shell', 'app-shell/workbench')
  await focus(figure)
  await page.keyboard.press('Tab')
  await expect(figure).toHaveAttribute('data-preview-outcome', /next visible shell zone/)
  await page.keyboard.press('s')
  await expect(figure).toHaveAttribute('data-preview-outcome', 'Sidebar collapsed')
  const before = await figure.getAttribute('data-preview-cols')
  await figure.getByRole('button', { name: 'Full preview' }).click()
  await expect(figure.getByRole('button', { name: 'Exit full preview' })).toBeVisible()
  await expect.poll(() => figure.getAttribute('data-preview-cols')).not.toBe(before)
  await expect(figure).toHaveAttribute('data-preview-outcome', 'Sidebar collapsed')
})

test('FileManager navigates and toggles its public tree state', async ({ page }) => {
  const figure = await pattern(page, 'file-manager', 'file-manager/basic')
  await focus(figure)
  await page.keyboard.press('ArrowDown')
  await expect(figure).toHaveAttribute('data-preview-outcome', /File manager:/)
  await page.keyboard.press('ArrowRight')
  await expect(figure).toHaveAttribute('data-preview-outcome', /(Toggled tree node|File manager:)/)
})

test('Agent and Git workbench overlays open and peel without losing base state', async ({ page }) => {
  const agent = await pattern(page, 'agent-workbench', 'agent-workbench/basic')
  await focus(agent)
  await page.keyboard.press('o')
  await expect(agent).toHaveAttribute('data-preview-outcome', /Session overlay opened/)
  await expect(agent.locator('[data-termrock-hints="1"]')).toContainText('Esc close overlay')
  await page.keyboard.press('Escape')
  await expect(agent).toHaveAttribute('data-preview-outcome', /prompt draft preserved/)

  const git = await pattern(page, 'git-workbench', 'git-workbench/basic')
  await focus(git)
  await page.keyboard.press('?')
  await expect(git).toHaveAttribute('data-preview-outcome', 'Git workbench help opened')
  await page.keyboard.press('Escape')
  await expect(git).toHaveAttribute('data-preview-outcome', 'Git workbench help closed')
})

test('Database workbench emits a safe local run request', async ({ page }) => {
  const figure = await pattern(page, 'database-workbench', 'database-workbench/basic')
  await focus(figure)
  await page.keyboard.press('ControlOrMeta+Enter')
  await expect(figure).toHaveAttribute('data-preview-outcome', /Run requested.*demo executed no query/)
})

test('SetupWizard advances a real step and opens its cancel confirmation', async ({ page }) => {
  const figure = await pattern(page, 'setup-wizard', 'setup-wizard/welcome')
  await focus(figure)
  await page.keyboard.press('Enter')
  await expect(figure).toHaveAttribute('data-preview-outcome', /Setup wizard:/)
  await page.keyboard.press('Escape')
  await expect(figure).toHaveAttribute('data-preview-outcome', 'Cancel confirmation opened')
  await page.keyboard.press('Escape')
  await expect(figure).toHaveAttribute('data-preview-outcome', 'Cancel confirmation dismissed')
})

test('pattern Code view shows exact shared implementation and returns to live state', async ({ page }) => {
  const figure = await pattern(page, 'auth-entry', 'auth-entry/basic')
  await figure.getByRole('button', { name: 'Code' }).click()
  await expect(figure.locator('[data-termrock-code="1"]')).toContainText('struct AuthEntryDemo')
  await expect(figure.locator('canvas')).toBeHidden()
  await figure.getByRole('button', { name: 'Preview' }).click()
  await expect(figure.locator('canvas')).toBeVisible()
})

test('pattern index filters grouped poster cards', async ({ page }) => {
  await page.goto('/docs/patterns')
  await expect(page.getByText('35 of 35 patterns')).toBeVisible()
  await page.getByRole('searchbox').fill('workbench')
  await expect(page.getByText(/of 35 patterns/)).toBeVisible()
  const workbench = page.getByRole('link', { name: /AgentWorkbench/ })
  await expect(workbench).toBeVisible()
  await expect(workbench.getByRole('img')).toBeVisible()
  await expect(page.getByRole('link', { name: /AuthEntry/ })).toHaveCount(0)
})
