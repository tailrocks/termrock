const output = `${import.meta.dir}/../dist/client`
const root = Bun.file(`${output}/index.html`)
const fallback = Bun.file(`${output}/404.html`)
const legacyShell = Bun.file(`${output}/_shell.html`)

if (!(await root.exists())) {
  throw new Error('TanStack Start prerendered root is missing')
}
if (!(await fallback.exists())) {
  throw new Error('TanStack Start SPA fallback is missing')
}
if (await legacyShell.exists()) {
  throw new Error('legacy _shell.html output remains')
}

const html = await root.text()
const required = [
  '<main id="main-content"',
  'Build terminal software that feels finished.',
  'data-termrock-preview="agent-workbench/basic"',
] as const
const missing = required.filter((marker) => !html.includes(marker))
if (missing.length > 0) {
  throw new Error(`prerendered root missing ${missing.join(', ')}`)
}

console.log('static root prerender and SPA fallback verified')
