import { join } from 'node:path'
import { loadCatalog, type Story } from './catalog-data'

const root = join(import.meta.dir, '..', '..')
const output = join(root, 'docs', 'public', 'demo-code.json')
const catalog = await loadCatalog()
const storyById = new Map(catalog.stories.map((story) => [story.id, story]))

function exactStorySnippet(story: Story): string {
  return [
    'use termrock::style::RolePalette;',
    'use termrock_lookbook::{',
    '    design::lookbook_system,',
    '    frame::{paint_story_frame, story_by_id},',
    '};',
    '',
    `let story = story_by_id(${JSON.stringify(story.id)}).expect("canonical story");`,
    `assert_eq!(story.component(), ${JSON.stringify(story.component)});`,
    'let frame = paint_story_frame(',
    '    story,',
    '    &lookbook_system(RolePalette::default()),',
    '    None,',
    '    None,',
    ');',
    `assert_eq!(frame.story_id, ${JSON.stringify(story.id)});`,
  ].join('\n')
}

const snippets: Record<string, string> = {}
for (const story of catalog.stories) snippets[story.id] ??= exactStorySnippet(story)
const exactStoryIds = new Set(catalog.stories.map((story) => story.id))
const stale = Object.keys(snippets).filter((story) => !exactStoryIds.has(story))
if (stale.length) throw new Error(`stale demo-code stories: ${stale.join(', ')}`)
for (const entry of [...catalog.components, ...catalog.patterns]) {
  const story = storyById.get(entry.story)
  if (!story || !snippets[entry.story]) throw new Error(`${entry.id}: missing representative story code`)
}

const serialized = `${JSON.stringify(Object.fromEntries(Object.entries(snippets).sort()), null, 2)}\n`
if (process.argv.includes('--check')) {
  const current = await Bun.file(output).text().catch(() => '')
  if (current !== serialized) {
    throw new Error('demo-code.json is stale; run bun run generate:demo-code')
  }
} else {
  await Bun.write(output, serialized)
}

console.log(`${Object.keys(snippets).length} canonical demo code samples`)
