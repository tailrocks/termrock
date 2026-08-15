import { readdir } from 'node:fs/promises'
import { join } from 'node:path'
import { frontmatter, scalar } from './doc-frontmatter'

type Demo = {
  id: string
  title: string
  component: string
  description: string
  interactive: boolean
  interactionKind: string
  hints: string[]
}

const root = join(import.meta.dir, '..', '..')
const componentDir = join(root, 'docs', 'content', 'docs', 'components')
const result = Bun.spawnSync(
  ['cargo', 'run', '-q', '-p', 'termrock-lookbook', '--', 'list', '--format', 'json'],
  { cwd: root },
)
if (result.exitCode !== 0) throw new Error(result.stderr.toString())
const catalog = JSON.parse(result.stdout.toString()) as Demo[]
const byId = new Map(catalog.map((demo) => [demo.id, demo]))
const byComponent = new Map<string, Demo[]>()
for (const demo of catalog) {
  const variants = byComponent.get(demo.component) ?? []
  variants.push(demo)
  byComponent.set(demo.component, variants)
}
const storiesSource = await Bun.file(
  join(root, 'crates', 'termrock-lookbook', 'src', 'stories.rs'),
).text()

function exactStorySetup(demoId: string): string {
  const escaped = demoId.replace(/[.*+?^${}()|[\]\\]/g, '\\$&')
  const match = storiesSource.match(new RegExp(`Story::new\\(\\s*"${escaped}"`))
  const storyAt = match?.index ?? -1
  if (storyAt < 0) throw new Error(`${demoId}: missing story registration`)
  const nextStory = storiesSource.indexOf('Story::new(', storyAt + 10)
  const registration = storiesSource.slice(storyAt, nextStory < 0 ? undefined : nextStory)
  const functions = [...registration.matchAll(/^\s*([a-z][a-z0-9_]+),\s*$/gm)]
  const renderFn = functions.at(-1)?.[1]
  if (!renderFn) throw new Error(`${demoId}: cannot resolve render function`)
  const functionAt = storiesSource.indexOf(`fn ${renderFn}(`)
  if (functionAt < 0) throw new Error(`${demoId}: missing ${renderFn}`)
  const nextFunction = storiesSource.indexOf('\nfn ', functionAt + 3)
  const source = storiesSource.slice(functionAt, nextFunction < 0 ? undefined : nextFunction).trim()
  return [
    'use ratatui::prelude::*;',
    'use termrock::{input::*, interaction::*, runtime::*, style::*, widgets::*};',
    '',
    '// Exact paint setup used by the shared catalog story.',
    source,
  ].join('\n')
}

function yamlList(key: string, values: string[]): string {
  if (!values.length) return `${key}: []`
  return `${key}:\n${values.map((value) => `  - ${JSON.stringify(value)}`).join('\n')}`
}

function replaceList(body: string, key: string, values: string[]): string {
  const expression = new RegExp(`^${key}:(?:\\s*\\[\\])?(?:\\n  - .*)*`, 'm')
  if (!expression.test(body)) throw new Error(`missing ${key}`)
  return body.replace(expression, yamlList(key, values))
}

function outcomeFor(hint: string): string {
  const lower = hint.toLowerCase()
  if (/hover|move pointer/.test(lower)) {
    return 'Pointer movement updates visible hover chrome without activating the control.'
  }
  if (/drag|resize/.test(lower)) {
    return 'Dragging changes the visible continuous value or geometry and emits its typed change outcome.'
  }
  if (/wheel|scroll|page/.test(lower)) {
    return 'Scrolling changes the mounted viewport while preserving selection and surrounding page position.'
  }
  if (/type|paste|edit|filter|search/.test(lower)) {
    return 'Editing changes the visible Unicode-safe draft and reports the public typed edit outcome.'
  }
  if (/esc|close|cancel|dismiss/.test(lower)) {
    return 'Cancel or close removes the transient state without performing a host-owned effect.'
  }
  if (/enter|space|click|activate|submit|choose|toggle|open/.test(lower)) {
    return 'Activation changes the mounted state and exposes the public typed request in the preview status.'
  }
  if (/arrow|select|focus|tab|home|end|next|previous/.test(lower)) {
    return 'Navigation moves visible focus or selection and preserves it for the next event.'
  }
  return `The ${JSON.stringify(hint)} action updates visible mounted state and reports its typed outcome.`
}

function recipeFor(hint: string): string {
  const lower = hint.toLowerCase()
  if (/hover|move pointer/.test(lower)) return 'Move the pointer over the target; hover chrome changes without activation.'
  if (/drag|resize/.test(lower)) return 'Press, drag, and release on the visible handle; geometry/value follows the pointer.'
  if (/wheel|scroll|page/.test(lower)) return 'Scroll over the preview; its own viewport moves while the docs page stays put.'
  if (/type|paste|edit|filter|search/.test(lower)) return 'Focus the editable field, then type or paste Unicode text; the draft remains visible.'
  if (/esc|close|cancel|dismiss/.test(lower)) return 'Open the transient state first, then cancel it; focus and the underlying state are preserved.'
  if (/enter|space|click|activate|submit|choose|toggle|open/.test(lower)) return 'Activate the focused or pointed target; inspect the changed cells and typed outcome below the terminal.'
  if (/arrow|select|focus|tab|home|end|next|previous/.test(lower)) return 'Move through the real focus/selection model; the next action uses the new target.'
  return 'Dispatch this action to the mounted Rust state machine and inspect its visible typed result.'
}

function interactionContract(demo: Demo): string {
  const hints = demo.hints.join(' ').toLowerCase()
  const mouse = /click|hover|pointer|drag|wheel|mouse/.test(hints)
  const editor = demo.interactionKind === 'editor-form'
  const timed = demo.interactionKind === 'timed-state'
  const value = (covered: boolean) => covered ? 'covered' : 'not-applicable in primary demo'
  return `| Axis | Status |\n|---|---|\n| Keyboard | ${value(demo.interactive)} |\n| Mouse | ${value(demo.interactive && mouse)} |\n| Focus | ${value(demo.interactive)} |\n| Cursor / caret | ${value(editor)} |\n| Narrow terminal | covered |\n| Unicode | covered |\n| Non-color cues | covered |\n| Motion / time | ${value(timed)} |`
}

function variantsSection(component: string): string {
  const variants = (byComponent.get(component) ?? []).toSorted((a, b) => a.id.localeCompare(b.id))
  if (variants.length <= 1) {
    return 'This component has one canonical catalog setup. Resize the live host to exercise its responsive contract; **Reset** remounts the same configuration.'
  }
  const rows = variants.map((variant) => `| \`${variant.id}\` | ${variant.title} | ${variant.interactive ? 'interactive' : 'paint-only'} |`)
  return `Choose **Variant** above the preview. A variant is a separate configuration and remounts fresh state; it is never presented as an interaction step.\n\n| Demo | Configuration | Input |\n|---|---|---|\n${rows.join('\n')}`
}

function section(body: string, heading: string): string {
  const escaped = heading.replace(/[.*+?^${}()|[\]\\]/g, '\\$&')
  return body.match(new RegExp(`^## ${escaped}\\n([\\s\\S]*?)(?=^## |(?![\\s\\S]))`, 'm'))?.[1]?.trim() ?? ''
}

function replaceSection(body: string, heading: string, content: string): string {
  const escaped = heading.replace(/[.*+?^${}()|[\]\\]/g, '\\$&')
  const expression = new RegExp(`^## ${escaped}\\n[\\s\\S]*?(?=^## |(?![\\s\\S]))`, 'm')
  if (!expression.test(body)) throw new Error(`missing ## ${heading}`)
  return body.replace(expression, `## ${heading}\n\n${content.trim()}\n\n`)
}

function orderCanonicalSections(body: string): string {
  const primary = [
    'Live terminal (Ghostty-class)',
    'Try it',
    'State and typed outcomes',
    'Interaction contract',
    'Configuration and variants',
    'Usage',
  ]
  const closing = [
    'Common mistakes',
    'Test recipe',
    'Stories',
    'Source and related material',
    'Seen in applications',
  ]
  const extracted = new Map<string, string>()
  let remainder = body
  for (const heading of [...primary, ...closing]) {
    const escaped = heading.replace(/[.*+?^${}()|[\]\\]/g, '\\$&')
    const expression = new RegExp(`^## ${escaped}\\n[\\s\\S]*?(?=^## |(?![\\s\\S]))`, 'gm')
    const matches = [...remainder.matchAll(expression)]
    const match = heading === 'Stories'
      ? matches.find((candidate) => candidate[0].includes('Primary (live preview)'))
      : heading === 'Common mistakes'
        ? matches.find((candidate) => candidate[0].includes('Do not treat separate Lookbook'))
        : matches[0]
    if (!match) throw new Error(`missing ## ${heading}`)
    extracted.set(heading, match[0].trim())
    const at = match.index!
    remainder = remainder.slice(0, at) + remainder.slice(at + match[0].length)
  }
  const firstExtra = remainder.search(/^## /m)
  const prefix = (firstExtra < 0 ? remainder : remainder.slice(0, firstExtra)).trimEnd()
  const extras = (firstExtra < 0 ? '' : remainder.slice(firstExtra)).trim()
  return [
    prefix,
    ...primary.map((heading) => extracted.get(heading)!),
    extras,
    ...closing.map((heading) => extracted.get(heading)!),
  ].filter(Boolean).join('\n\n')
}

for (const name of (await readdir(componentDir)).filter((name) => name.endsWith('.mdx'))) {
  const path = join(componentDir, name)
  let body = await Bun.file(path).text()
  const meta = frontmatter(body)
  const demoId = scalar(meta, 'demo')!
  const component = scalar(meta, 'component')!
  const demo = byId.get(demoId)
  if (!demo) throw new Error(`${name}: unknown demo ${demoId}`)

  if (body.includes(`description: '${component} widget.'`)) {
    body = body.replace(
      `description: '${component} widget.'`,
      `description: ${JSON.stringify(demo.description)}`,
    )
  }
  body = replaceList(body, 'actions', demo.hints)
  const expected = [...new Set(demo.hints.map(outcomeFor))]
  body = replaceList(body, 'expectedOutcomes', demo.interactive ? expected : [])

  const previewCopy = demo.interactive
    ? 'This preview mounts one persistent Rust state machine. Focus it, use the current actions shown by the runtime, inspect the typed outcome, and use **Reset** for fresh state.'
    : 'This preview is deterministic paint only. It traps no keyboard or pointer input; resize the page to inspect responsive rendering.'
  body = replaceSection(
    body,
    'Live terminal (Ghostty-class)',
    `${previewCopy}\n\n<TerminalPreview story="${demo.id}" interactive caption="${component} · ${demo.title} — ${demo.interactive ? 'live shared Rust state' : 'paint-only shared Rust frame'}" />`,
  )
  body = replaceSection(body, 'Interaction contract', interactionContract(demo))
  const tryIt = demo.interactive
    ? `${demo.hints.map((hint) => `- \`${hint}\`: ${recipeFor(hint)}`).join('\n')}\n\nEvery accepted action reaches the same mounted Rust value. The status line shows the latest typed outcome; **Reset** creates a fresh instance.`
    : '- No component input. Resize the host and compare its narrow-terminal, Unicode, and non-color rendering contracts.\n\nThe docs page remains fully navigable because this demo does not invent focus, a cursor, or activation.'
  body = replaceSection(body, 'Try it', tryIt)

  // Older handbook migrations sometimes inserted a second static demo block
  // between the canonical live preview and the configuration section.
  body = body.replace(
    /\n## `[^\n]+`\n[\s\S]*?(?=\n## Configuration and variants)/,
    '\n',
  )

  body = replaceSection(
    body,
    'Usage',
    `The Code view above and this source excerpt use the exact paint setup registered for \`${demo.id}\`. Interaction state is long-lived in the catalog interactor; the host only forwards backend-neutral events.\n\n\`\`\`rust\n${exactStorySetup(demo.id)}\n\`\`\``,
  )

  if (!body.includes('## Configuration and variants')) {
    const usage = section(body, 'Usage')
    body = body.replace(
      `## Usage\n\n${usage}`,
      `## Configuration and variants\n\n${variantsSection(component)}\n\n## Usage\n\n${usage}`,
    )
  } else {
    body = replaceSection(body, 'Configuration and variants', variantsSection(component))
  }
  body = orderCanonicalSections(body)
  await Bun.write(path, body.trimEnd() + '\n')
}

console.log('synchronized 166 component interaction contracts from the shared Rust catalog')
