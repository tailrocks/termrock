import { expect, test } from '@playwright/test'

test('home shell owns one main landmark and exposes primary actions', async ({ page }) => {
  await page.goto('/')

  await expect(page).toHaveTitle('Terminal UI components for Rust — TermRock')
  await expect(page.locator('html')).toHaveAttribute('lang', 'en')
  await expect(page.locator('meta[name="theme-color"]')).toHaveCount(0)

  const main = page.getByRole('main')
  await expect(main).toHaveCount(1)
  const skipLink = page.getByRole('link', { name: 'Skip to main content' })
  await expect(skipLink).toHaveAttribute('href', '#main-content')

  const header = page.locator('header.site-shell')
  await expect(header).toHaveCount(1)
  await expect(main.getByRole('banner')).toHaveCount(0)
  await expect(header.getByRole('link', { name: 'TermRock', exact: true })).toHaveAttribute(
    'href',
    '/',
  )
  await expect(header.getByRole('navigation', { name: 'Primary navigation' })).toBeVisible()
  await expect(header.getByRole('link', { name: 'Components' })).toBeVisible()
  await expect(header.getByRole('link', { name: 'Patterns' })).toBeVisible()
  await expect(header.getByRole('link', { name: 'Docs' })).toBeVisible()
  await expect(header.getByRole('link', { name: 'TermRock source on GitHub' })).toBeVisible()
  await expect(header.getByRole('button', { name: /Search/ })).toBeVisible()
  await expect(header.locator('[aria-current="page"]')).toHaveCount(1)

  await skipLink.focus()
  await page.keyboard.press('Enter')
  await expect(main).toBeFocused()
})

test('catalog search navigates without a search backend', async ({ page }) => {
  await page.goto('/')
  await page.getByRole('button', { name: /Search/ }).click()
  await page.getByRole('textbox', { name: 'Search TermRock' }).fill('button')

  const result = page.getByRole('button', { name: /Component Button$/ })
  await expect(result).toBeVisible()
  await result.click()
  await expect(page).toHaveURL(/\/docs\/components\/button\/?$/)
  await expect(page).toHaveTitle('Button — TermRock')

  const sidebar = page.locator('#nd-sidebar')
  await expect(page.locator('[aria-current="page"]')).toHaveCount(1)
  await expect(sidebar.getByRole('link', { name: /^Components/ })).toHaveAttribute(
    'aria-current',
    'location',
  )

  await page.goto('/docs/components')
  await expect(page.locator('[aria-current="page"]')).toHaveCount(1)
  await expect(sidebar.getByRole('link', { name: /^Components/ })).toHaveAttribute(
    'aria-current',
    'page',
  )
  await expect(page.locator('[aria-current="location"]')).toHaveCount(0)
})

test('guide search is derived from the documentation tree', async ({ page }) => {
  await page.goto('/')
  await page.getByRole('button', { name: /Search/ }).click()
  await page.getByRole('textbox', { name: 'Search TermRock' }).fill('installation')

  const result = page.getByRole('button', { name: /Guide Installation$/ })
  await expect(result).toBeVisible()
  await result.click()
  await expect(page).toHaveURL(/\/docs\/installation\/?$/)
  await expect(page).toHaveTitle('Installation — TermRock')
})

test('docs tree is the only section navigation owner and exposes current ancestry', async ({
  page,
}) => {
  await page.goto('/docs')

  const sidebar = page.locator('#nd-sidebar')
  await expect(sidebar.getByRole('link', { name: /^Components/ })).toHaveCount(1)
  await expect(sidebar.getByRole('link', { name: /^Application patterns/ })).toHaveCount(1)
  await expect(sidebar.getByRole('button', { name: /^Actions/ })).toBeHidden()

  await page.goto('/docs/components/button')
  await expect(page.locator('[aria-current="page"]')).toHaveCount(1)
  await expect(sidebar.getByRole('link', { name: 'Button', exact: true })).toHaveAttribute(
    'aria-current',
    'page',
  )
  await expect(sidebar.getByRole('link', { name: /^Components/ })).toHaveAttribute(
    'aria-current',
    'location',
  )
  await expect(sidebar.getByRole('button', { name: /^Actions/ })).toHaveAttribute(
    'aria-current',
    'location',
  )
  await expect(sidebar.locator('.site-sidebar__folder-trigger')).toHaveCount(9)
})

test('unknown routes keep the branded shell and a real skip target', async ({ page }) => {
  await page.goto('/outside-the-workshop')

  await expect(page.locator('header.site-shell')).toHaveCount(1)
  const main = page.locator('main#main-content')
  await expect(main).toHaveCount(1)
  await expect(main.getByRole('heading', { level: 1 })).toHaveText(
    'This path is outside the workshop.',
  )
  const skipLink = page.getByRole('link', { name: 'Skip to main content' })
  await skipLink.focus()
  await page.keyboard.press('Enter')
  await expect(main).toBeFocused()
})

test('mobile docs shell keeps search and sidebar navigation reachable', async ({ page }) => {
  await page.setViewportSize({ width: 390, height: 844 })
  await page.goto('/docs/runtime')

  await expect(page).toHaveTitle('Runtime integration — TermRock')
  const main = page.getByRole('main')
  await expect(main).toHaveCount(1)
  const header = page.locator('header.site-shell')
  await expect(header).toHaveCount(1)
  await expect(main.getByRole('banner')).toHaveCount(0)
  await expect(header.getByRole('link', { name: 'TermRock', exact: true })).toBeVisible()
  await expect(header.getByRole('button', { name: /Search/ })).toBeVisible()
  await expect(header.locator('[aria-current]')).toHaveCount(0)

  const skipLink = page.getByRole('link', { name: 'Skip to main content' })
  await skipLink.focus()
  await page.keyboard.press('Enter')
  await expect(main).toBeFocused()

  const menu = header.getByRole('button', { name: 'Open documentation navigation' })
  await expect(menu).toBeVisible()
  await menu.click()
  const drawer = page.locator('aside[data-state="open"]')
  await expect(drawer).toBeVisible()
  await expect(drawer.locator('[aria-current="page"]')).toHaveCount(1)
  await expect(drawer.getByRole('link', { name: 'Runtime integration' })).toHaveAttribute(
    'aria-current',
    'page',
  )
})

test('mobile home menu keeps its GitHub destination visible', async ({ page }) => {
  await page.setViewportSize({ width: 390, height: 844 })
  await page.goto('/')

  const header = page.locator('header.site-shell')
  await expect(header.getByRole('link', { name: 'TermRock source on GitHub' })).toBeHidden()
  await header.locator('summary[aria-label="Open primary navigation"]').click()

  const mobileNav = header.getByRole('navigation', { name: 'Mobile navigation' })
  await expect(mobileNav.getByRole('link', { name: 'GitHub', exact: true })).toBeVisible()
})
