const output = `${import.meta.dir}/../dist/client`
const required = ['index.html', '404.html', 'docs/index.html', 'docs/components/index.html']

for (const relative of required) {
  if (!(await Bun.file(`${output}/${relative}`).exists())) {
    throw new Error(`static docs output missing ${relative}`)
  }
}

// Spot-check core component reference pages that must always prerender.
const componentChecks = [
  ['action-bar', 'action-bar-basic'],
  ['list', 'list-selection'],
  ['viewport', 'viewport-both-axes'],
] as const
for (const [component, preview] of componentChecks) {
  const page = `${output}/docs/components/${component}/index.html`
  if (!(await Bun.file(page).exists())) {
    throw new Error(`static docs output missing docs/components/${component}/index.html`)
  }
  const html = await Bun.file(page).text()
  if (
    !html.includes(preview) ||
    !html.includes('Interaction contract') ||
    !html.includes('class="line"')
  ) {
    throw new Error(
      `${component} reference page is missing its preview, contract, or Rust usage`,
    )
  }
}

const components = await Bun.file(`${output}/docs/components/index.html`).text()
const siteBase = Bun.env['GITHUB_ACTIONS'] === 'true' ? '/termrock' : ''
if (!components.includes(`href="${siteBase}/docs/components/action-bar"`)) {
  throw new Error('components overview link does not include the configured site base')
}
if (components.includes('/termrock/termrock/')) {
  throw new Error('Pages base path was applied twice')
}

console.log('static docs smoke: shell + core component reference pages OK')
