import { join } from 'node:path'

const root = join(import.meta.dir, '..', '..')

const forbiddenEditorial = [
  /<TerminalPreview\b/u,
  /<SeenInApplications\b/u,
  /^# /mu,
  /^## Live terminal$/mu,
  /^## Live application preview$/mu,
  /^## State and typed outcomes$/mu,
  /^## Minimal implementation$/mu,
  /^## Minimal host loop$/mu,
  /^## Basic example$/mu,
  /^## Interactive example$/mu,
  /^## Classification and composition$/mu,
  /^## Workflow$/mu,
  /^## Source(?: and related material| files)?$/mu,
  /^## Migrated handbook guidance$/mu,
  /^## Common mistakes$/mu,
  /^## Test recipe$/mu,
  /^## Stories$/mu,
  /Catalog coverage for this component/u,
  /exact mounted Rust story/u,
  /The Code view above and this source excerpt/u,
] as const

export function assertEditorialPage(source: string, label: string): void {
  for (const forbidden of forbiddenEditorial) {
    if (forbidden.test(source)) throw new Error(`${label}: legacy page structure remains: ${forbidden}`)
  }
}

export async function assertSharedDetailLayout(): Promise<void> {
  const folder = join(root, 'docs', 'src', 'components', 'docs')
  const source = [
    await Bun.file(join(folder, 'DocDetailLayout.tsx')).text(),
    await Bun.file(join(folder, 'ImplementationPanel.tsx')).text(),
  ].join('\n')
  for (const required of [
    'ComponentDocLayout',
    'PatternDocLayout',
    'Live preview',
    'What it is for',
    'What the mounted story proves',
    'Minimal implementation',
    'Variants and composition',
    'API, tokens, accessibility',
    '<details',
    'Related',
  ]) {
    if (!source.includes(required)) throw new Error(`shared detail layout: missing ${required}`)
  }
}
