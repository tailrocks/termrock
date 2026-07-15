import { expect, test } from 'bun:test'

test('catalog checker covers every published story', () => {
  const result = Bun.spawnSync(['bun', 'run', 'scripts/check-catalog.ts'], {
    cwd: new URL('..', import.meta.url).pathname,
  })
  expect(result.stderr.toString()).toBe('')
  expect(result.exitCode).toBe(0)
  expect(result.stdout.toString()).toContain('catalog covers 12 stories')
})
