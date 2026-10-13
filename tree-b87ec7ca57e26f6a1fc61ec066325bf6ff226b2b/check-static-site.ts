const output = `${import.meta.dir}/../dist/client`
const content = `${import.meta.dir}/../content/docs`
const required = ['index.html', '404.html']
const pages: Array<{ output: string; title: string }> = []

for await (const file of new Bun.Glob('**/*.mdx').scan({ cwd: content })) {
  const slug = file.replace(/\.mdx$/, '')
  const pageOutput =
    slug === 'index' ? 'docs/index.html' : `docs/${slug}/index.html`
  const source = await Bun.file(`${content}/${file}`).text()
  const titleLine = source.split('\n').find((line) => line.startsWith('title: '))

  if (titleLine === undefined) {
    throw new Error(`${file} is missing a frontmatter title`)
  }

  required.push(pageOutput)
  pages.push({ output: pageOutput, title: titleLine.slice('title: '.length) })
}

for (const relative of required) {
  if (!(await Bun.file(`${output}/${relative}`).exists())) {
    throw new Error(`static docs output missing ${relative}`)
  }
}

for (const page of pages) {
  const html = await Bun.file(`${output}/${page.output}`).text()
  if (!html.includes(page.title)) {
    throw new Error(`${page.output} did not prerender its title`)
  }
}

const docsIndex = await Bun.file(`${output}/docs/index.html`).text()
for (const page of pages) {
  if (!docsIndex.includes(page.title)) {
    throw new Error(`docs navigation is missing ${page.title}`)
  }
}

const componentChecks = [
  ['action-bar', 'action-bar-basic'],
  ['list', 'list-selection'],
  ['viewport', 'viewport-both-axes'],
] as const
for (const [component, preview] of componentChecks) {
  const html = await Bun.file(`${output}/docs/components/${component}/index.html`).text()
  if (!html.includes(preview) || !html.includes('Interaction contract') || !html.includes('class="line"')) {
    throw new Error(`${component} reference page is missing its preview, contract, or Rust usage`)
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

console.log(`static docs contain ${required.length} prerendered routes`)
