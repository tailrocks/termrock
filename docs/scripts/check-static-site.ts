const output = `${import.meta.dir}/../dist/client`
const required = ['index.html', '404.html', 'docs/index.html', 'docs/components/index.html']

for (const relative of required) {
  if (!(await Bun.file(`${output}/${relative}`).exists())) {
    throw new Error(`static docs output missing ${relative}`)
  }
}

// Spot-check core component reference pages that must always prerender.
// Ghostty path: MDX embeds `story="…/…"` (slash form), not SVG-era hyphenated filenames.
const componentChecks = [
  ['action-bar', 'action-bar/basic'],
  ['list', 'list/selection'],
  ['viewport', 'viewport/both-axes'],
] as const
for (const [component, storyId] of componentChecks) {
  const page = `${output}/docs/components/${component}/index.html`
  if (!(await Bun.file(page).exists())) {
    throw new Error(`static docs output missing docs/components/${component}/index.html`)
  }
  const html = await Bun.file(page).text()
  // SSR/prerender may keep the attribute as story="…" or HTML-escaped story=&#34;…&#34;.
  const storyNeedle = storyId
  const hasStory =
    html.includes(`story="${storyNeedle}"`) ||
    html.includes(`story='${storyNeedle}'`) ||
    html.includes(`story=&#34;${storyNeedle}&#34;`) ||
    html.includes(storyNeedle)
  if (
    !hasStory ||
    !html.includes('Interaction contract') ||
    !html.includes('class="line"')
  ) {
    throw new Error(
      `${component} reference page is missing its Ghostty story embed, contract, or Rust usage`,
    )
  }
}

const components = await Bun.file(`${output}/docs/components/index.html`).text()
if (!components.includes('href="/docs/components/action-bar"')) {
  throw new Error('components overview link does not use the custom-domain root')
}
if (components.includes('/termrock/')) {
  throw new Error('legacy GitHub project Pages base path remains in static output')
}

console.log('static docs smoke: shell + core component reference pages OK')
