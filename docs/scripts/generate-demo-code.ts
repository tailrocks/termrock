import { readdir } from 'node:fs/promises'
import { join } from 'node:path'
import { frontmatter, scalar } from './doc-frontmatter'

const root = join(import.meta.dir, '..', '..')
const docsRoot = join(root, 'docs', 'content', 'docs')
const output = join(root, 'docs', 'public', 'demo-code.json')
const applications = await Bun.file(
  join(root, 'crates', 'termrock-lookbook', 'src', 'interactors', 'applications.rs'),
).text()
const composites = await Bun.file(
  join(root, 'crates', 'termrock-lookbook', 'src', 'interactors', 'composites.rs'),
).text()
const stories = await Bun.file(
  join(root, 'crates', 'termrock-lookbook', 'src', 'stories.rs'),
).text()

function itemUntil(source: string, start: number, next: RegExp): string {
  const tail = source.slice(start)
  const match = tail.slice(1).match(next)
  return tail.slice(0, match?.index == null ? undefined : match.index + 1).trim()
}

function exactPatternDemo(demo: string): string | undefined {
  const escaped = demo.replace(/[.*+?^${}()|[\]\\]/g, '\\$&')
  const app = applications.match(
    new RegExp(`"${escaped}"\\s*=>\\s*Some\\(Box::new\\((\\w+)::new\\(\\)\\)\\)`),
  )
  if (app?.[1]) {
    const start = applications.indexOf(`struct ${app[1]} {`)
    if (start >= 0) {
      return [
        `// Exact long-lived shared demo for ${demo}.`,
        '// It calls only public termrock::patterns and termrock::widgets APIs;',
        '// host-owned effects are represented by visible typed outcomes.',
        itemUntil(applications, start, /\nstruct [A-Z]/),
      ].join('\n')
    }
  }

  const line = composites.match(new RegExp(`^\\s*"${escaped}"\\s*=>.*$`, 'm'))?.[0]?.trim()
  if (!line) {
    const story = stories.match(new RegExp(`Story::new\\(\\s*"${escaped}"[\\s\\S]*?\\n\\s*([a-z][a-z0-9_]+),\\s*\\n\\s*\\)`))
    const renderFn = story?.[1]
    if (!renderFn) return undefined
    const start = stories.indexOf(`fn ${renderFn}(`)
    if (start < 0) return undefined
    return [
      `// Exact shared story setup for ${demo}.`,
      itemUntil(stories, start, /\nfn /),
    ].join('\n')
  }
  const helper = line.match(/\b([a-z][a-z0-9_]+_state)\(\)/)?.[1]
  let fixture = ''
  if (helper) {
    const start = composites.indexOf(`fn ${helper}(`)
    if (start >= 0) fixture = itemUntil(composites, start, /\n(?:fn |impl )/)
  }
  return [
    `// Exact persistent-state registration for ${demo}.`,
    `// crates/termrock-lookbook/src/interactors/composites.rs`,
    line,
    fixture,
    '// The shared CompositePatternDemo forwards render, key, pointer, paste,',
    '// resize, focus, Escape, and deterministic time to this public state.',
  ].filter(Boolean).join('\n\n')
}

async function mdxFiles(directory: string): Promise<string[]> {
  const entries = await readdir(directory, { withFileTypes: true })
  const nested = await Promise.all(entries.map(async (entry) => {
    const path = join(directory, entry.name)
    if (entry.isDirectory()) return mdxFiles(path)
    return entry.isFile() && entry.name.endsWith('.mdx') ? [path] : []
  }))
  return nested.flat()
}

const snippets: Record<string, string> = {}
for (const path of await mdxFiles(docsRoot)) {
  const body = await Bun.file(path).text()
  const meta = frontmatter(body)
  const demo = scalar(meta, 'demo')
  if (!demo) continue
  const rust = body.match(/```rust\s*\n([\s\S]*?)\n```/)
  const publicType = scalar(meta, 'component') || scalar(meta, 'pattern')
  const source = scalar(meta, 'source')
  const pattern = scalar(meta, 'pattern')
  const snippet = (pattern ? exactPatternDemo(demo) : undefined) || rust?.[1]?.trim() || [
    `use termrock::patterns::${publicType};`,
    '',
    `// Shared live demo: ${demo}`,
    `// Public recipe source: ${source}`,
    '// The mounted Rust catalog owns persistent state and calls this recipe’s',
    '// public render and input APIs with deterministic in-memory fixtures.',
  ].join('\n')
  if (pattern || !snippets[demo] || snippet.length > snippets[demo].length) {
    snippets[demo] = snippet
  }
}

// Variant controls can select any catalog story, not only the 200 canonical
// component/pattern routes. Give every variant its exact compiled paint setup.
for (const match of stories.matchAll(/Story::new\(\s*"([^"]+)"/g)) {
  const demo = match[1]!
  if (snippets[demo]) continue
  const start = match.index!
  const next = stories.indexOf('Story::new(', start + 10)
  const registration = stories.slice(start, next < 0 ? undefined : next)
  const functions = [...registration.matchAll(/^\s*([a-z][a-z0-9_]+),\s*$/gm)]
  const renderFn = functions.at(-1)?.[1]
  if (!renderFn) continue
  const functionAt = stories.indexOf(`fn ${renderFn}(`)
  if (functionAt < 0) continue
  snippets[demo] = [
    'use ratatui::prelude::*;',
    'use termrock::{input::*, interaction::*, runtime::*, style::*, widgets::*};',
    '',
    '// Exact paint setup used by the selected catalog variant.',
    itemUntil(stories, functionAt, /\nfn /),
  ].join('\n')
}

const serialized = `${JSON.stringify(Object.fromEntries(Object.entries(snippets).sort()), null, 2)}\n`
if (process.argv.includes('--check')) {
  const current = await Bun.file(output).text().catch(() => '')
  if (current !== serialized) {
    throw new Error('demo-code.json is stale; run bun docs/scripts/generate-demo-code.ts')
  }
} else {
  await Bun.write(output, serialized)
}

console.log(`${Object.keys(snippets).length} canonical demo code samples`)
