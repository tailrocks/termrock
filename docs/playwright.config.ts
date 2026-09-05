import { defineConfig, devices } from '@playwright/test'

const testPort = Number(process.env.TERMROCK_TEST_PORT ?? 4179)
if (!Number.isInteger(testPort) || testPort < 1 || testPort > 65_535) {
  throw new Error('TERMROCK_TEST_PORT must be an integer from 1 through 65535')
}
const testBaseUrl = `http://127.0.0.1:${testPort}`

export default defineConfig({
  testDir: './tests',
  timeout: 45_000,
  expect: { timeout: 15_000 },
  fullyParallel: true,
  forbidOnly: Boolean(process.env.CI),
  retries: process.env.CI ? 2 : 0,
  workers: process.env.CI ? 2 : undefined,
  reporter: 'line',
  use: {
    baseURL: testBaseUrl,
    trace: 'retain-on-failure',
    screenshot: 'only-on-failure',
    video: 'retain-on-failure',
  },
  projects: [
    {
      name: 'chromium',
      use: { ...devices['Desktop Chrome'] },
    },
  ],
  webServer: {
    command: `bun run dev --host 127.0.0.1 --port ${testPort}`,
    url: `${testBaseUrl}/docs`,
    reuseExistingServer: !process.env.CI,
    timeout: 120_000,
  },
})
